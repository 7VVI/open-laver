//! s15/s17 队友注册表 — 记录持久队友的元数据与生命周期状态

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TeammatePhase {
    Work,
    Idle,
    Shutdown,
}

#[derive(Debug, Clone, Serialize)]
pub struct TeammateInfo {
    pub name: String,
    pub role: String,
    pub phase: TeammatePhase,
    pub session_id: String,
}

pub struct TeammateHandle {
    pub info: Mutex<TeammateInfo>,
    /// 请求关闭标志 (shutdown 握手)
    pub shutdown: AtomicBool,
}

#[derive(Default)]
pub struct TeamRegistry {
    members: Mutex<HashMap<String, Arc<TeammateHandle>>>,
}

impl TeamRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, name: &str, role: &str, session_id: &str) -> Arc<TeammateHandle> {
        let handle = Arc::new(TeammateHandle {
            info: Mutex::new(TeammateInfo {
                name: name.to_string(),
                role: role.to_string(),
                phase: TeammatePhase::Work,
                session_id: session_id.to_string(),
            }),
            shutdown: AtomicBool::new(false),
        });
        self.members
            .lock()
            .unwrap()
            .insert(name.to_string(), handle.clone());
        handle
    }

    pub fn get(&self, name: &str) -> Option<Arc<TeammateHandle>> {
        self.members.lock().unwrap().get(name).cloned()
    }

    pub fn set_phase(&self, name: &str, phase: TeammatePhase) {
        if let Some(h) = self.get(name) {
            h.info.lock().unwrap().phase = phase;
        }
    }

    pub fn request_shutdown(&self, name: &str) -> bool {
        if let Some(h) = self.get(name) {
            h.shutdown.store(true, Ordering::SeqCst);
            true
        } else {
            false
        }
    }

    pub fn remove(&self, name: &str) {
        self.members.lock().unwrap().remove(name);
    }

    pub fn list(&self) -> Vec<TeammateInfo> {
        self.members
            .lock()
            .unwrap()
            .values()
            .map(|h| h.info.lock().unwrap().clone())
            .collect()
    }

    pub fn names(&self) -> Vec<String> {
        self.members.lock().unwrap().keys().cloned().collect()
    }
}
