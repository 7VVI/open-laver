//! 后台任务 — tokio::spawn 执行慢命令，先回占位 tool_result，完成后并入下一轮

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use serde::Serialize;
use tauri::AppHandle;

use crate::agent::tools::shell::run_powershell;
use crate::gateway::events::{emit, EV_BG_UPDATE};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum BgStatus {
    Running,
    Done,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
pub struct BgTask {
    pub bg_id: String,
    pub session_id: String,
    pub command: String,
    pub status: BgStatus,
    pub output: Option<String>,
}

#[derive(Default)]
pub struct BackgroundManager {
    tasks: Mutex<HashMap<String, BgTask>>,
    /// 已完成但尚未并入对话的结果 (按 session)
    pending_notifications: Mutex<HashMap<String, Vec<String>>>,
}

impl BackgroundManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// 启动后台命令，返回 bg_id (立即)
    pub fn start(
        self: &Arc<Self>,
        app: AppHandle,
        session_id: String,
        command: String,
        cwd: PathBuf,
    ) -> String {
        let bg_id = format!("bg-{}", uuid::Uuid::new_v4().simple());
        let task = BgTask {
            bg_id: bg_id.clone(),
            session_id: session_id.clone(),
            command: command.clone(),
            status: BgStatus::Running,
            output: None,
        };
        self.tasks.lock().unwrap().insert(bg_id.clone(), task.clone());
        emit(&app, EV_BG_UPDATE, task);

        let mgr = self.clone();
        let bg_id2 = bg_id.clone();
        tauri::async_runtime::spawn(async move {
            let (output, ok) = run_powershell(&command, &cwd).await;
            let status = if ok { BgStatus::Done } else { BgStatus::Failed };
            let mut updated = BgTask {
                bg_id: bg_id2.clone(),
                session_id: session_id.clone(),
                command: command.clone(),
                status: status.clone(),
                output: Some(output.clone()),
            };
            mgr.tasks
                .lock()
                .unwrap()
                .insert(bg_id2.clone(), updated.clone());
            // 生成通知，待下一轮并入
            let note = format!(
                "<task_notification bg_id=\"{}\" status=\"{}\">\n命令: {}\n输出:\n{}\n</task_notification>",
                bg_id2,
                if ok { "done" } else { "failed" },
                command,
                output
            );
            mgr.pending_notifications
                .lock()
                .unwrap()
                .entry(session_id)
                .or_default()
                .push(note);
            updated.output = Some("(已生成通知)".into());
            emit(&app, EV_BG_UPDATE, updated);
        });

        bg_id
    }

    /// 取出并清空某 session 的待并入通知 (loop 每轮开始调用)
    pub fn take_notifications(&self, session_id: &str) -> Vec<String> {
        self.pending_notifications
            .lock()
            .unwrap()
            .remove(session_id)
            .unwrap_or_default()
    }

    pub fn list(&self, session_id: &str) -> Vec<BgTask> {
        self.tasks
            .lock()
            .unwrap()
            .values()
            .filter(|t| t.session_id == session_id)
            .cloned()
            .collect()
    }
}
