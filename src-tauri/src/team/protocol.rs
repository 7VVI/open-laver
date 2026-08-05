//! 团队协议 — request_id 贯穿的 request-response 状态机
//! shutdown 握手 + plan_approval 审批

use std::collections::HashMap;
use std::sync::Mutex;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolStatus {
    Pending,
    Approved,
    Rejected,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProtocolState {
    pub request_id: String,
    pub ptype: String, // shutdown | plan_approval
    pub sender: String,
    pub target: String,
    pub status: ProtocolStatus,
    pub payload: String,
}

#[derive(Default)]
pub struct ProtocolRegistry {
    pending: Mutex<HashMap<String, ProtocolState>>,
}

impl ProtocolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create(&self, ptype: &str, sender: &str, target: &str, payload: &str) -> String {
        let request_id = format!("req-{}", uuid::Uuid::new_v4().simple());
        let state = ProtocolState {
            request_id: request_id.clone(),
            ptype: ptype.to_string(),
            sender: sender.to_string(),
            target: target.to_string(),
            status: ProtocolStatus::Pending,
            payload: payload.to_string(),
        };
        self.pending
            .lock()
            .unwrap()
            .insert(request_id.clone(), state);
        request_id
    }

    /// 校验类型匹配 + 防重复后更新状态
    pub fn match_response(
        &self,
        request_id: &str,
        expected_type: &str,
        approved: bool,
    ) -> Result<(), String> {
        let mut map = self.pending.lock().unwrap();
        let st = map.get_mut(request_id).ok_or("未知 request_id")?;
        if st.ptype != expected_type {
            return Err(format!("协议类型不匹配: 期望 {expected_type}，实际 {}", st.ptype));
        }
        if st.status != ProtocolStatus::Pending {
            return Err("请求已被处理 (防重复)".into());
        }
        st.status = if approved {
            ProtocolStatus::Approved
        } else {
            ProtocolStatus::Rejected
        };
        Ok(())
    }

    pub fn get(&self, request_id: &str) -> Option<ProtocolState> {
        self.pending.lock().unwrap().get(request_id).cloned()
    }

    pub fn list(&self) -> Vec<ProtocolState> {
        self.pending.lock().unwrap().values().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_state_machine() {
        let reg = ProtocolRegistry::new();
        let id = reg.create("shutdown", "lead", "worker-1", "please stop");
        assert_eq!(reg.get(&id).unwrap().status, ProtocolStatus::Pending);
        // 类型不匹配被拒
        assert!(reg.match_response(&id, "plan_approval", true).is_err());
        // 正确匹配
        assert!(reg.match_response(&id, "shutdown", true).is_ok());
        assert_eq!(reg.get(&id).unwrap().status, ProtocolStatus::Approved);
        // 防重复
        assert!(reg.match_response(&id, "shutdown", true).is_err());
    }
}
