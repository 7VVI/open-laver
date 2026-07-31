//! Agent 执行上下文 — 工具执行时携带的环境信息

use std::path::PathBuf;

use tauri::AppHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentKind {
    /// 主 Agent (完整工具集)
    Main,
    /// s06 一次性子代理 (无 task 工具，防递归)
    Sub,
    /// s15 持久队友 (精简工具集 + send_message)
    Teammate,
}

#[derive(Clone)]
pub struct ToolCtx {
    pub app: AppHandle,
    pub state: std::sync::Arc<crate::state::AppState>,
    pub session_id: String,
    pub agent_name: String,
    pub kind: AgentKind,
    /// 用户选定的办公工作目录
    pub workspace: PathBuf,
    /// 实际执行目录 (worktree 绑定时被覆盖, s18)
    pub cwd: PathBuf,
}

impl ToolCtx {
    /// safe_path: 解析相对路径并校验是否在允许范围内。
    /// 返回 (绝对路径, 是否在工作区内)
    pub fn resolve_path(&self, p: &str) -> (PathBuf, bool) {
        let raw = PathBuf::from(p);
        let abs = if raw.is_absolute() {
            raw
        } else {
            self.cwd.join(raw)
        };
        // 词法归一化 (不要求路径存在)
        let mut normalized = PathBuf::new();
        for comp in abs.components() {
            match comp {
                std::path::Component::ParentDir => {
                    normalized.pop();
                }
                std::path::Component::CurDir => {}
                c => normalized.push(c.as_os_str()),
            }
        }
        let inside = normalized.starts_with(&self.workspace) || normalized.starts_with(&self.cwd);
        (normalized, inside)
    }
}
