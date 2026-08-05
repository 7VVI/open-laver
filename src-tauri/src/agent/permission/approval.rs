//! 闸门 3: 用户审批 — 事件推送前端弹窗，oneshot 等待决策

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::AppHandle;
use tokio::sync::oneshot;

use crate::constants::APPROVAL_TIMEOUT_SECS;
use crate::gateway::events::{emit, EV_APPROVAL_REQUEST};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    AllowOnce,
    AllowAlways,
    Deny,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApprovalRequestPayload {
    pub id: String,
    pub session_id: String,
    pub agent: String,
    pub tool: String,
    pub summary: String,
    pub input: Value,
}

/// 审批经纪人: 挂起的 oneshot 等待前端 resolve
#[derive(Default)]
pub struct ApprovalBroker {
    pending: Mutex<HashMap<String, oneshot::Sender<ApprovalDecision>>>,
}

impl ApprovalBroker {
    /// 发起审批请求并等待 (超时按 Deny)
    pub async fn request(
        &self,
        app: &AppHandle,
        session_id: &str,
        agent: &str,
        tool: &str,
        summary: &str,
        input: &Value,
    ) -> ApprovalDecision {
        let id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id.clone(), tx);

        emit(
            app,
            EV_APPROVAL_REQUEST,
            ApprovalRequestPayload {
                id: id.clone(),
                session_id: session_id.into(),
                agent: agent.into(),
                tool: tool.into(),
                summary: summary.into(),
                input: input.clone(),
            },
        );

        let decision =
            match tokio::time::timeout(Duration::from_secs(APPROVAL_TIMEOUT_SECS), rx).await {
                Ok(Ok(d)) => d,
                _ => ApprovalDecision::Deny,
            };
        self.pending.lock().unwrap().remove(&id);
        decision
    }

    /// 前端回传决策 (gateway command 调用)
    pub fn resolve(&self, id: &str, decision: ApprovalDecision) -> bool {
        if let Some(tx) = self.pending.lock().unwrap().remove(id) {
            tx.send(decision).is_ok()
        } else {
            false
        }
    }
}
