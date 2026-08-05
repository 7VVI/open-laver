//! 子代理 — task 工具 spawn 子循环: 全新 messages、无 task 工具、仅回传最终文本

use std::sync::Arc;

use serde_json::Value;

use crate::agent::ctx::{AgentKind, ToolCtx};
use crate::agent::loop_engine::run_agent_loop;
use crate::agent::session::SessionHandle;
use crate::agent::tools::tool::ToolOutput;
use crate::llm::types::{Message, Role};
use crate::state::AppState;

/// 运行一个一次性子代理，返回其最终文本结论
pub async fn run_subagent(state: &Arc<AppState>, parent: &ToolCtx, input: &Value) -> ToolOutput {
    let description = input["description"].as_str().unwrap_or("").trim();
    if description.is_empty() {
        return ToolOutput::err("task 需要 description 参数");
    }

    // 全新会话状态 (上下文隔离)
    let sub_id = format!("{}::sub-{}", parent.session_id, uuid::Uuid::new_v4().simple());
    let sub_session = SessionHandle::new(sub_id.clone(), format!("子任务: {description}"));
    {
        let mut st = sub_session.state.lock().await;
        st.messages.push(Message::user_text(description));
    }
    state.sessions.insert(sub_session.clone());

    let role = "你是一个专注的子代理，负责独立完成被交付的单一子任务。完成后用简洁的文本给出结论 (结果、产出文件路径、关键发现)。你无法再派生新的子代理。";

    let sub_ctx = ToolCtx {
        app: parent.app.clone(),
        state: state.clone(),
        session_id: sub_id.clone(),
        agent_name: format!("subagent-{}", &sub_id[sub_id.len().saturating_sub(6)..]),
        kind: AgentKind::Sub,
        workspace: parent.workspace.clone(),
        cwd: parent.cwd.clone(),
    };

    // Box::pin 打断 async 递归 (run_agent_loop -> execute_tool -> run_subagent -> run_agent_loop)
    Box::pin(run_agent_loop(
        state,
        &sub_ctx,
        &sub_session,
        AgentKind::Sub,
        Some(role.to_string()),
    ))
    .await;

    // 仅回传最后一条 assistant 文本
    let conclusion = {
        let st = sub_session.state.lock().await;
        st.messages
            .iter()
            .rev()
            .find(|m| m.role == Role::Assistant && !m.text().trim().is_empty())
            .map(|m| m.text())
            .unwrap_or_else(|| "(子代理未产生文本结论)".to_string())
    };
    state.sessions.remove(&sub_id);

    ToolOutput::ok(format!("子代理完成:\n{conclusion}"))
}
