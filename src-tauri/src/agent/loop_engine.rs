//! 主循环引擎 — 一个 while 循环，各模块机制按顺序挂载
//!
//! 每轮迭代顺序:
//! 1. UserPromptSubmit hooks
//! 2. 注入 cron 队列 + 后台通知 + 队友 inbox
//! 3. 压缩管线 (L3->L1->L2->L4)
//! 4. 组装 system prompt
//! 5. assemble_tool_pool (内置 + MCP)
//! 6. LLM 流式调用 (外包 recovery)
//! 7. 无 tool_use -> Stop hooks -> extract_memories -> 结束
//! 8. 有 tool_use -> PreToolUse(权限) -> 执行(可后台) -> PostToolUse
//! 9. tool_results 追加 -> 回到 2

use std::sync::atomic::Ordering;
use std::sync::Arc;

use serde_json::Value;

use crate::agent::compact::pipeline;
use crate::agent::ctx::{AgentKind, ToolCtx};
use crate::agent::memory::extractor;
use crate::agent::permission::approval::ApprovalDecision;
use crate::agent::permission::engine::{self, Decision};
use crate::agent::prompt::builder::PromptContext;
use crate::agent::recovery::{RecoveryAction, RecoveryState};
use crate::constants::*;
use crate::gateway::events::*;
use crate::llm::provider::LlmProvider;
use crate::llm::types::*;
use crate::state::AppState;

/// 运行一个用户输入触发的完整 turn (直到模型停止或被中断)
pub async fn run_turn(
    state: Arc<AppState>,
    app: tauri::AppHandle,
    session_id: String,
    user_input: String,
) {
    let Some(session) = state.sessions.get(&session_id) else {
        notice(&app, "error", "会话不存在");
        return;
    };
    session.running.store(true, Ordering::SeqCst);
    session.cancel.store(false, Ordering::SeqCst);

    emit(
        &app,
        EV_TURN_STATE,
        TurnStatePayload {
            session_id: session_id.clone(),
            state: "running".into(),
            detail: None,
        },
    );

    // 追加用户输入 (UserPromptSubmit: 注入 todo 提醒等)
    {
        let mut st = session.state.lock().await;
        let mut prompt = user_input.clone();
        // 连续多轮未更新 todo 则提醒
        if !st.todos.is_empty() && st.rounds_since_todo >= TODO_REMINDER_ROUNDS {
            prompt = format!(
                "<reminder>别忘了用 todo_write 更新任务进度</reminder>\n{prompt}"
            );
        }
        if !user_input.is_empty() {
            st.messages.push(Message::user_text(prompt));
        }
    }

    let ctx = ToolCtx {
        app: app.clone(),
        state: state.clone(),
        session_id: session_id.clone(),
        agent_name: "assistant".into(),
        kind: AgentKind::Main,
        workspace: state.workspace().await,
        cwd: state.workspace().await,
    };

    run_agent_loop(&state, &ctx, &session, AgentKind::Main, None).await;

    session.running.store(false, Ordering::SeqCst);
    // 持久化
    {
        let st = session.state.lock().await;
        let _ = state.db.save_messages(&session_id, &st.messages);
    }
    emit(
        &app,
        EV_TURN_STATE,
        TurnStatePayload {
            session_id: session_id.clone(),
            state: "idle".into(),
            detail: None,
        },
    );
}

/// 核心 Agent 循环 — 主 Agent / 子代理 / 队友共用
/// role_prompt: 子代理/队友的角色说明
pub async fn run_agent_loop(
    state: &Arc<AppState>,
    ctx: &ToolCtx,
    session: &Arc<crate::agent::session::SessionHandle>,
    kind: AgentKind,
    role_prompt: Option<String>,
) {
    let (provider, settings) = match state.provider().await {
        Ok(p) => p,
        Err(e) => {
            notice(&ctx.app, "error", e.clone());
            // 把原因写入对话，避免“无反应”
            push_error_message(session, &ctx.app, &e).await;
            emit(
                &ctx.app,
                EV_TURN_STATE,
                TurnStatePayload {
                    session_id: ctx.session_id.clone(),
                    state: "idle".into(),
                    detail: None,
                },
            );
            return;
        }
    };
    let mut recovery = RecoveryState::default();
    let mut rounds: u32 = 0;
    let max_rounds = match kind {
        AgentKind::Sub => SUBAGENT_MAX_ROUNDS,
        _ => MAIN_LOOP_MAX_ROUNDS,
    };

    loop {
        if session.cancel.load(Ordering::SeqCst) {
            notice(&ctx.app, "info", "已中断");
            break;
        }
        rounds += 1;
        if rounds > max_rounds {
            notice(&ctx.app, "warn", format!("达到循环上限 ({max_rounds})"));
            break;
        }

        // --- 2. 注入 cron / 后台通知 / 队友 inbox (仅主 Agent 与队友) ---
        if kind != AgentKind::Sub {
            inject_external_messages(state, ctx, session).await;
        }

        // --- 3. 压缩管线 ---
        {
            let mut st = session.state.lock().await;
            pipeline::preprocess(&mut st.messages, &state.task_outputs_dir());
            if pipeline::should_compact(
                &st.messages,
                settings.context_window,
                MAX_OUTPUT_TOKENS as usize,
            ) && st.compact_failures < COMPACT_FAILURE_FUSE
            {
                let tp = state
                    .transcripts_dir()
                    .join(format!("{}-{}.jsonl", session.id, rounds));
                let _ = std::fs::create_dir_all(state.transcripts_dir());
                match pipeline::compact_history(&provider, &settings.model, &mut st.messages, &tp)
                    .await
                {
                    Ok(_) => st.compact_failures = 0,
                    Err(_) => st.compact_failures += 1,
                }
            }
        }

        // --- 4. 组装 system prompt + 5. 工具池 ---
        let tool_specs = assemble_tool_pool(state, kind).await;
        let tool_names: Vec<String> = tool_specs.iter().map(|t| t.name.clone()).collect();
        let system = {
            let pctx = PromptContext {
                workspace: ctx.workspace.to_string_lossy().to_string(),
                os_info: state.os_info(),
                tool_names: tool_names.clone(),
                skill_catalog: state.skills.catalog(),
                memory_index: if kind == AgentKind::Main {
                    state.memory.index_block()
                } else {
                    String::new()
                },
                agent_role: role_prompt.clone(),
            };
            state.prompt_builder.build(&pctx)
        };

        // --- 6. LLM 调用 (带 recovery) ---
        let messages_snapshot = { session.state.lock().await.messages.clone() };
        let max_tokens = if recovery.has_escalated {
            MAX_OUTPUT_TOKENS_ESCALATED
        } else {
            MAX_OUTPUT_TOKENS
        };
        let model = recovery
            .current_model
            .clone()
            .unwrap_or_else(|| settings.model.clone());

        let req = ChatRequest {
            model: model.clone(),
            system,
            messages: messages_snapshot,
            tools: tool_specs,
            max_tokens,
            thinking: settings.thinking,
        };

        let resp = call_with_stream(&provider, &req, &ctx.app, &ctx.session_id, &ctx.agent_name)
            .await;

        let response = match resp {
            Ok(r) => {
                recovery.on_success();
                r
            }
            Err(e) => {
                if handle_error(&mut recovery, &e, state, &provider, &settings, session, &ctx.app)
                    .await
                {
                    continue; // 已恢复，重试
                } else {
                    break; // 放弃
                }
            }
        };

        // 输出被 max_tokens 截断 (含工具参数被截断) -> 升级上限重试，丢弃残缺回复
        if response.stop_reason == StopReason::MaxTokens {
            match recovery.on_max_tokens() {
                RecoveryAction::EscalateMaxTokens | RecoveryAction::ContinueTruncated => {
                    notice(&ctx.app, "warn", "本轮输出超出长度上限，已提高上限并重试…");
                    continue;
                }
                RecoveryAction::GiveUp(m) => {
                    push_error_message(
                        session,
                        &ctx.app,
                        &format!("生成失败：{m}。建议把大文件拆成多次写入，或缩短单次内容。"),
                    )
                    .await;
                    break;
                }
                _ => {}
            }
        }

        // 追加 assistant 回复
        {
            let mut st = session.state.lock().await;
            st.messages.push(Message {
                role: Role::Assistant,
                content: response.content.clone(),
            });
            st.rounds_since_todo = st.rounds_since_todo.saturating_add(1);
            st.turn_count += 1;
        }
        // 推送 assistant 文本
        let assistant_text = response
            .content
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text } => Some(text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        if !assistant_text.trim().is_empty() {
            emit(
                &ctx.app,
                EV_ASSISTANT_MSG,
                AssistantMsgPayload {
                    session_id: ctx.session_id.clone(),
                    agent: ctx.agent_name.clone(),
                    text: assistant_text.clone(),
                },
            );
        }

        // --- 7. 无 tool_use -> 结束 ---
        if !response.needs_follow_up() {
            // 提取记忆 (仅主 Agent)
            if kind == AgentKind::Main {
                let recent = {
                    let st = session.state.lock().await;
                    render_recent(&st.messages, 10)
                };
                extractor::extract_memories(&provider, &settings.model, &state.memory, &recent)
                    .await;
                if state.memory.needs_consolidation() {
                    extractor::consolidate(&provider, &settings.model, &state.memory).await;
                }
            }
            break;
        }

        // --- 8. 执行工具 ---
        let tool_uses: Vec<(String, String, Value)> = response
            .content
            .iter()
            .filter_map(|b| match b {
                ContentBlock::ToolUse { id, name, input } => {
                    Some((id.clone(), name.clone(), input.clone()))
                }
                _ => None,
            })
            .collect();

        let mut results: Vec<ContentBlock> = Vec::new();
        for (id, name, input) in tool_uses {
            if session.cancel.load(Ordering::SeqCst) {
                break;
            }
            let result = execute_tool(state, ctx, session, &id, &name, &input).await;
            results.push(result);
        }

        // --- 9. tool_results 追加 ---
        {
            let mut st = session.state.lock().await;
            st.messages.push(Message::tool_results(results));
        }
    }
}

/// 组装工具池 (内置 + MCP)
async fn assemble_tool_pool(state: &Arc<AppState>, kind: AgentKind) -> Vec<ToolSpec> {
    let mut specs = state.registry.specs_for(kind, &[]);
    // 子代理不接入 MCP (保持隔离简单)；主 Agent 与队友合并 MCP 工具
    if kind != AgentKind::Sub {
        specs.extend(state.mcp.tool_specs().await);
    }
    specs
}

/// LLM 流式调用，delta 推前端
async fn call_with_stream(
    provider: &Arc<dyn LlmProvider>,
    req: &ChatRequest,
    app: &tauri::AppHandle,
    session_id: &str,
    agent: &str,
) -> Result<LlmResponse, LlmError> {
    let app2 = app.clone();
    let sid = session_id.to_string();
    let ag = agent.to_string();
    let sink = move |ev: StreamEvent| {
        if let StreamEvent::TextDelta { text } = ev {
            emit(
                &app2,
                EV_DELTA,
                DeltaPayload {
                    session_id: sid.clone(),
                    agent: ag.clone(),
                    text,
                },
            );
        }
    };
    provider.chat(req, &sink).await
}

/// 错误恢复分派；返回 true 表示应重试
async fn handle_error(
    recovery: &mut RecoveryState,
    err: &LlmError,
    state: &Arc<AppState>,
    provider: &Arc<dyn LlmProvider>,
    settings: &crate::llm::provider::ProviderSettings,
    session: &Arc<crate::agent::session::SessionHandle>,
    app: &tauri::AppHandle,
) -> bool {
    // 打到终端，便于诊断
    eprintln!("[Laver][LLM 错误] {err}");
    tracing::warn!("LLM 调用错误: {err}");

    // 不可重试的致命错误：认证失败 / 4xx 客户端错误 —— 立即终止并把原因写入对话
    let fatal: Option<String> = match err {
        LlmError::Auth(m) => Some(format!(
            "认证失败（API Key 无效或无权限）：{}",
            trim_msg(m)
        )),
        LlmError::Api { status, message } if *status >= 400 && *status < 500 => Some(format!(
            "模型接口拒绝了请求（HTTP {status}）：{}",
            trim_msg(message)
        )),
        _ => None,
    };
    if let Some(msg) = fatal {
        notice(app, "error", msg.clone());
        push_error_message(session, app, &msg).await;
        return false;
    }

    let action = match err {
        LlmError::PromptTooLong => recovery.on_prompt_too_long(),
        // 到这里只剩 5xx / 网络错误 / 429 / 529 —— 属于可重试的瞬时错误
        _ => recovery.on_transient(err),
    };
    match action {
        RecoveryAction::EscalateMaxTokens => {
            notice(app, "info", "输出截断，升级 token 上限后重试");
            true
        }
        RecoveryAction::ContinueTruncated => {
            let mut st = session.state.lock().await;
            st.messages
                .push(Message::user_text("从截断处直接继续 — 不要道歉，不要重复已有内容"));
            true
        }
        RecoveryAction::ReactiveCompact => {
            notice(app, "info", "上下文超限，应急压缩后重试");
            let mut st = session.state.lock().await;
            let tp = state.transcripts_dir().join("reactive.jsonl");
            let _ = std::fs::create_dir_all(state.transcripts_dir());
            let _ =
                pipeline::reactive_compact(provider, &settings.model, &mut st.messages, &tp).await;
            true
        }
        RecoveryAction::Backoff {
            wait,
            switch_to_fallback,
        } => {
            if switch_to_fallback {
                if let Some(fb) = &settings.fallback_model {
                    recovery.current_model = Some(fb.clone());
                    notice(app, "warn", format!("多次过载，切换到备用模型 {fb}"));
                }
            }
            notice(
                app,
                "info",
                format!("服务繁忙，{}ms 后重试", wait.as_millis()),
            );
            tokio::time::sleep(wait).await;
            true
        }
        RecoveryAction::GiveUp(m) => {
            let msg = format!("多次重试后仍失败：{m}");
            notice(app, "error", msg.clone());
            push_error_message(session, app, &msg).await;
            false
        }
    }
}

/// 把错误作为一条 assistant 消息写入会话并推送前端 (持久可见)
async fn push_error_message(
    session: &Arc<crate::agent::session::SessionHandle>,
    app: &tauri::AppHandle,
    text: &str,
) {
    let full = format!("⚠️ {text}");
    {
        let mut st = session.state.lock().await;
        st.messages.push(Message::assistant_text(full.clone()));
    }
    emit(
        app,
        EV_ASSISTANT_MSG,
        AssistantMsgPayload {
            session_id: session.id.clone(),
            agent: "assistant".into(),
            text: full,
        },
    );
}

/// 截断过长的错误文本
fn trim_msg(s: &str) -> String {
    let t = s.trim();
    if t.chars().count() > 600 {
        t.chars().take(600).collect::<String>() + "…"
    } else {
        t.to_string()
    }
}

/// 注入外部消息 (cron / 后台 / inbox)
async fn inject_external_messages(
    state: &Arc<AppState>,
    ctx: &ToolCtx,
    session: &Arc<crate::agent::session::SessionHandle>,
) {
    let mut injections: Vec<String> = Vec::new();
    // 后台任务完成通知
    for note in state.background.take_notifications(&session.id) {
        injections.push(note);
    }
    // 队友 inbox (队友身份时读自己的信箱)
    let msgs = state.mailbox.drain(&ctx.agent_name);
    for m in msgs {
        injections.push(format!(
            "[来自 {} 的消息 ({})]: {}",
            m.from, m.mtype, m.content
        ));
    }
    if !injections.is_empty() {
        let mut st = session.state.lock().await;
        st.messages.push(Message::user_text(injections.join("\n\n")));
    }
}

/// 执行单个工具: PreToolUse(权限) -> 分发 -> PostToolUse
async fn execute_tool(
    state: &Arc<AppState>,
    ctx: &ToolCtx,
    session: &Arc<crate::agent::session::SessionHandle>,
    id: &str,
    name: &str,
    input: &Value,
) -> ContentBlock {
    emit(
        &ctx.app,
        EV_TOOL_START,
        ToolStartPayload {
            session_id: ctx.session_id.clone(),
            agent: ctx.agent_name.clone(),
            id: id.to_string(),
            name: name.to_string(),
            input: input.clone(),
        },
    );

    // --- 子代理 task 工具特殊处理 ---
    if name == "task" && ctx.kind != AgentKind::Sub {
        let out = crate::agent::subagent::run_subagent(state, ctx, input).await;
        return finish_tool(&ctx.app, ctx, id, name, out.content, !out.is_error);
    }

    // --- 后台执行拦截 ---
    if name == "shell" && input["run_in_background"].as_bool().unwrap_or(false) {
        let command = input["command"].as_str().unwrap_or("").to_string();
        let bg_id = state.background.start(
            ctx.app.clone(),
            ctx.session_id.clone(),
            command,
            ctx.cwd.clone(),
        );
        let msg = format!("已在后台启动任务 {bg_id}，完成后会通知你。");
        return finish_tool(&ctx.app, ctx, id, name, msg, true);
    }

    // --- 权限: PreToolUse ---
    match check_permission(state, ctx, session, name, input).await {
        Decision::Deny(reason) => {
            let msg = format!("Permission denied: {reason}");
            return finish_tool(&ctx.app, ctx, id, name, msg, false);
        }
        Decision::Allow => {}
        Decision::Ask(_) => {} // check_permission 已在内部处理 ask，走到这里说明放行
    }

    // --- MCP 路由 / 内置分发 ---
    let out = if name.starts_with("mcp__") {
        match state.mcp.call(name, input).await {
            Ok(s) => crate::agent::tools::tool::ToolOutput::ok(s),
            Err(e) => crate::agent::tools::tool::ToolOutput::err(e),
        }
    } else {
        state.registry.dispatch(name, input, ctx).await
    };

    finish_tool(&ctx.app, ctx, id, name, out.content, !out.is_error)
}

/// 权限检查 (含审批记忆 + 弹窗)
async fn check_permission(
    state: &Arc<AppState>,
    ctx: &ToolCtx,
    session: &Arc<crate::agent::session::SessionHandle>,
    name: &str,
    input: &Value,
) -> Decision {
    let decision = engine::evaluate(name, input, ctx);
    match decision {
        Decision::Allow | Decision::Deny(_) => decision,
        Decision::Ask(reason) => {
            let key = engine::approval_key(name, input);
            // 审批记忆命中 -> 直接放行
            {
                let st = session.state.lock().await;
                if st.approval_always.contains(&key) {
                    return Decision::Allow;
                }
            }
            let d = state
                .approvals
                .request(&ctx.app, &ctx.session_id, &ctx.agent_name, name, &reason, input)
                .await;
            match d {
                ApprovalDecision::AllowOnce => Decision::Allow,
                ApprovalDecision::AllowAlways => {
                    let mut st = session.state.lock().await;
                    st.approval_always.insert(key);
                    Decision::Allow
                }
                ApprovalDecision::Deny => Decision::Deny("用户拒绝".into()),
            }
        }
    }
}

fn finish_tool(
    app: &tauri::AppHandle,
    ctx: &ToolCtx,
    id: &str,
    name: &str,
    content: String,
    ok: bool,
) -> ContentBlock {
    emit(
        app,
        EV_TOOL_RESULT,
        ToolResultPayload {
            session_id: ctx.session_id.clone(),
            agent: ctx.agent_name.clone(),
            id: id.to_string(),
            name: name.to_string(),
            ok,
            output: content.clone(),
        },
    );
    ContentBlock::ToolResult {
        tool_use_id: id.to_string(),
        content,
        is_error: !ok,
    }
}

/// 渲染最近 N 条消息为文本 (供记忆提取)
fn render_recent(messages: &[Message], n: usize) -> String {
    let start = messages.len().saturating_sub(n);
    let mut out = String::new();
    for m in &messages[start..] {
        let role = match m.role {
            Role::User => "用户",
            Role::Assistant => "助手",
        };
        out.push_str(&format!("[{role}] {}\n", m.text()));
    }
    out
}
