//! s15/s17 队友生命周期 — WORK -> IDLE(轮询) -> SHUTDOWN，自主认领任务

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::agent::ctx::{AgentKind, ToolCtx};
use crate::agent::loop_engine::run_agent_loop;
use crate::agent::session::SessionHandle;
use crate::constants::{IDLE_POLL_SECS, IDLE_TIMEOUT_SECS};
use crate::gateway::events::{emit, notice, EV_TEAM_UPDATE};
use crate::llm::types::Message;
use crate::state::AppState;
use crate::team::mailbox::{MailBus, MailMessage};
use crate::team::registry::TeammatePhase;

/// 启动一个持久队友 (tokio task)
pub fn spawn_teammate(
    state: Arc<AppState>,
    app: tauri::AppHandle,
    lead_session_id: String,
    name: String,
    role: String,
    initial_task: String,
) {
    let handle = state.team.register(&name, &role, &lead_session_id);
    emit(&app, EV_TEAM_UPDATE, state.team.list());

    tauri::async_runtime::spawn(async move {
        let session = SessionHandle::new(format!("teammate::{name}"), format!("队友 {name}"));
        {
            let mut st = session.state.lock().await;
            if !initial_task.is_empty() {
                st.messages.push(Message::user_text(initial_task));
            }
        }
        state.sessions.insert(session.clone());

        let workspace = state.workspace().await;
        let ctx = ToolCtx {
            app: app.clone(),
            state: state.clone(),
            session_id: session.id.clone(),
            agent_name: name.clone(),
            kind: AgentKind::Teammate,
            workspace: workspace.clone(),
            cwd: workspace,
        };

        let role_prompt = format!(
            "你是团队成员 {name}，职责: {role}。\n你可以用 send_message 与 lead 或其他成员沟通，用 task_claim/task_complete 认领和完成任务图中的任务。完成阶段性工作后向 lead 汇报。",
            name = name,
            role = role
        );

        loop {
            if handle.shutdown.load(Ordering::SeqCst) {
                break;
            }
            // WORK 阶段
            state.team.set_phase(&name, TeammatePhase::Work);
            emit(&app, EV_TEAM_UPDATE, state.team.list());
            run_agent_loop(&state, &ctx, &session, AgentKind::Teammate, Some(role_prompt.clone()))
                .await;

            // IDLE 阶段: 轮询 inbox + 任务板
            state.team.set_phase(&name, TeammatePhase::Idle);
            emit(&app, EV_TEAM_UPDATE, state.team.list());
            let idle_start = Instant::now();
            let mut got_work = false;
            loop {
                if handle.shutdown.load(Ordering::SeqCst) {
                    break;
                }
                // 收到消息?
                let msgs = state.mailbox.peek(&name);
                if !msgs.is_empty() {
                    // 有 shutdown_request 优先
                    if msgs.iter().any(|m| m.mtype == "shutdown_request") {
                        handle.shutdown.store(true, Ordering::SeqCst);
                        break;
                    }
                    let drained = state.mailbox.drain(&name);
                    let mut st = session.state.lock().await;
                    for m in drained {
                        st.messages.push(Message::user_text(format!(
                            "[来自 {}]: {}",
                            m.from, m.content
                        )));
                    }
                    got_work = true;
                    break;
                }
                // 扫描可认领任务
                let claimable = state.tasks.claimable();
                if let Some(t) = claimable.first() {
                    if state.tasks.claim(&t.id, &name).is_ok() {
                        let mut st = session.state.lock().await;
                        st.messages.push(Message::user_text(format!(
                            "你认领了任务 {}: {}\n{}",
                            t.id, t.subject, t.description
                        )));
                        got_work = true;
                        break;
                    }
                }
                if idle_start.elapsed() > Duration::from_secs(IDLE_TIMEOUT_SECS) {
                    break;
                }
                tokio::time::sleep(Duration::from_secs(IDLE_POLL_SECS)).await;
            }
            if !got_work {
                break; // IDLE 超时，退出
            }
        }

        // SHUTDOWN 阶段: 汇报后退出
        state.team.set_phase(&name, TeammatePhase::Shutdown);
        let summary_msg = MailMessage {
            from: name.clone(),
            to: "lead".into(),
            content: format!("队友 {name} 已结束工作并退出。"),
            mtype: "summary".into(),
            ts: MailBus::now_ts(),
            request_id: None,
        };
        state.mailbox.send(&summary_msg);
        state.sessions.remove(&session.id);
        state.team.remove(&name);
        emit(&app, EV_TEAM_UPDATE, state.team.list());
        notice(&app, "info", format!("队友 {name} 已退出"));
    });
}
