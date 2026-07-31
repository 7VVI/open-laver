//! 后端 -> 前端事件流 (悟空 agui_stream 对应物)

use serde::Serialize;
use tauri::{AppHandle, Emitter};

pub const EV_DELTA: &str = "agent://delta";
pub const EV_TOOL_START: &str = "agent://tool-start";
pub const EV_TOOL_RESULT: &str = "agent://tool-result";
pub const EV_ASSISTANT_MSG: &str = "agent://assistant-message";
pub const EV_TURN_STATE: &str = "agent://turn-state";
pub const EV_APPROVAL_REQUEST: &str = "agent://approval-request";
pub const EV_TODO_UPDATE: &str = "agent://todo-update";
pub const EV_TEAM_UPDATE: &str = "agent://team-update";
pub const EV_TEAM_MESSAGE: &str = "agent://team-message";
pub const EV_TASK_UPDATE: &str = "agent://task-update";
pub const EV_NOTICE: &str = "agent://notice";
pub const EV_BG_UPDATE: &str = "agent://background-update";
pub const EV_SESSION_TITLE: &str = "session://title";

pub fn emit<T: Serialize + Clone>(app: &AppHandle, event: &str, payload: T) {
    let _ = app.emit(event, payload);
}

#[derive(Debug, Clone, Serialize)]
pub struct DeltaPayload {
    pub session_id: String,
    pub agent: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolStartPayload {
    pub session_id: String,
    pub agent: String,
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolResultPayload {
    pub session_id: String,
    pub agent: String,
    pub id: String,
    pub name: String,
    pub ok: bool,
    pub output: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AssistantMsgPayload {
    pub session_id: String,
    pub agent: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TurnStatePayload {
    pub session_id: String,
    pub state: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NoticePayload {
    pub level: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionTitlePayload {
    pub session_id: String,
    pub title: String,
}

pub fn notice(app: &AppHandle, level: &str, text: impl Into<String>) {
    emit(
        app,
        EV_NOTICE,
        NoticePayload {
            level: level.into(),
            text: text.into(),
        },
    );
}
