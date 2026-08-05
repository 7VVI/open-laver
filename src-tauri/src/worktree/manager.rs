//! Worktree 隔离 — git worktree add，为并行任务提供独立目录+分支
//! 名称校验防路径穿越；events.jsonl 审计

use std::path::PathBuf;

use regex::Regex;

use crate::agent::tools::shell::run_powershell;
use crate::constants::WORKTREE_NAME_RE;

pub struct WorktreeManager {
    workspace: PathBuf,
}

impl WorktreeManager {
    pub fn new(workspace: PathBuf) -> Self {
        Self { workspace }
    }

    fn valid_name(name: &str) -> bool {
        Regex::new(WORKTREE_NAME_RE)
            .map(|re| re.is_match(name))
            .unwrap_or(false)
    }

    fn worktrees_dir(&self) -> PathBuf {
        self.workspace.join(".worktrees")
    }

    fn audit(&self, event: &str) {
        use std::io::Write;
        let dir = self.worktrees_dir();
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("events.jsonl"))
        {
            let _ = writeln!(
                f,
                "{{\"ts\":\"{}\",\"event\":\"{}\"}}",
                chrono::Local::now().to_rfc3339(),
                event.replace('"', "'")
            );
        }
    }

    /// 创建 worktree，返回其绝对路径
    pub async fn create(&self, name: &str) -> Result<PathBuf, String> {
        if !Self::valid_name(name) {
            return Err(format!("非法 worktree 名称: {name}"));
        }
        let path = self.worktrees_dir().join(name);
        let path_str = path.to_string_lossy().to_string();
        let cmd = format!(
            "git worktree add \"{}\" -b wt/{}",
            path_str, name
        );
        let (out, ok) = run_powershell(&cmd, &self.workspace).await;
        if !ok {
            return Err(format!("创建 worktree 失败: {out}"));
        }
        self.audit(&format!("create {name}"));
        Ok(path)
    }

    /// 删除 worktree；有未提交改动时默认拒绝，除非 discard=true
    pub async fn remove(&self, name: &str, discard: bool) -> Result<String, String> {
        if !Self::valid_name(name) {
            return Err(format!("非法 worktree 名称: {name}"));
        }
        let path = self.worktrees_dir().join(name);
        let path_str = path.to_string_lossy().to_string();
        // 检查未提交改动
        let status_cmd = format!("git -C \"{}\" status --porcelain", path_str);
        let (status_out, _) = run_powershell(&status_cmd, &self.workspace).await;
        if !status_out.trim().is_empty() && !discard {
            return Err(format!(
                "worktree {name} 存在未提交改动，拒绝删除 (如需强制请设 discard_changes=true):\n{status_out}"
            ));
        }
        let force = if discard { "--force" } else { "" };
        let cmd = format!("git worktree remove {} \"{}\"", force, path_str);
        let (out, ok) = run_powershell(&cmd, &self.workspace).await;
        if !ok {
            return Err(format!("删除 worktree 失败: {out}"));
        }
        self.audit(&format!("remove {name} discard={discard}"));
        Ok(format!("已删除 worktree {name}"))
    }

    pub async fn list(&self) -> String {
        let (out, _) = run_powershell("git worktree list", &self.workspace).await;
        out
    }

    pub fn path_of(&self, name: &str) -> Option<PathBuf> {
        if !Self::valid_name(name) {
            return None;
        }
        let p = self.worktrees_dir().join(name);
        if p.exists() {
            Some(p)
        } else {
            None
        }
    }
}

pub fn is_valid_worktree_name(name: &str) -> bool {
    WorktreeManager::valid_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_validation() {
        assert!(is_valid_worktree_name("feature-1"));
        assert!(is_valid_worktree_name("task_2.0"));
        assert!(!is_valid_worktree_name("../escape"));
        assert!(!is_valid_worktree_name("has/slash"));
        assert!(!is_valid_worktree_name("has space"));
        assert!(!is_valid_worktree_name(""));
    }
}
