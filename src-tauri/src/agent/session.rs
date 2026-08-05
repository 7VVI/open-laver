//! 会话状态 (messages + 各模块叠加的状态字段)

use std::collections::{HashMap, HashSet};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::llm::types::Message;

/// Todo 项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub content: String,
    pub status: String, // pending | in_progress | completed
}

/// 会话内可变状态
#[derive(Default)]
pub struct SessionState {
    pub messages: Vec<Message>,
    /// 当前 todo 清单
    pub todos: Vec<TodoItem>,
    /// 距上次 todo_write 的轮数
    pub rounds_since_todo: u32,
    /// "always allow" 审批记忆 (工具名或 工具名:模式)
    pub approval_always: HashSet<String>,
    /// 压缩 L4: compact 连续失败计数 (熔断)
    pub compact_failures: u32,
    /// 轮次计数
    pub turn_count: u32,
}

pub struct SessionHandle {
    pub id: String,
    pub title: std::sync::Mutex<String>,
    pub state: Mutex<SessionState>,
    /// 当前是否有 turn 在跑 (cron 队列处理器据此判断空闲)
    pub running: AtomicBool,
    /// 用户请求取消
    pub cancel: AtomicBool,
}

impl SessionHandle {
    pub fn new(id: String, title: String) -> Arc<Self> {
        Arc::new(Self {
            id,
            title: std::sync::Mutex::new(title),
            state: Mutex::new(SessionState::default()),
            running: AtomicBool::new(false),
            cancel: AtomicBool::new(false),
        })
    }
}

/// 会话管理器 (悟空 session/session_manager 对应物)
#[derive(Default)]
pub struct SessionManager {
    pub sessions: std::sync::RwLock<HashMap<String, Arc<SessionHandle>>>,
}

impl SessionManager {
    pub fn get(&self, id: &str) -> Option<Arc<SessionHandle>> {
        self.sessions.read().unwrap().get(id).cloned()
    }

    pub fn insert(&self, handle: Arc<SessionHandle>) {
        self.sessions
            .write()
            .unwrap()
            .insert(handle.id.clone(), handle);
    }

    pub fn remove(&self, id: &str) {
        self.sessions.write().unwrap().remove(id);
    }

    /// 任一会话空闲即可交付 cron (返回第一个空闲会话)
    pub fn first_idle(&self) -> Option<Arc<SessionHandle>> {
        self.sessions
            .read()
            .unwrap()
            .values()
            .find(|s| !s.running.load(std::sync::atomic::Ordering::SeqCst))
            .cloned()
    }
}
