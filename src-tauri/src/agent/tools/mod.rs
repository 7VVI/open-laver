//! 工具模块 — 注册全部内置工具 (s02-s18)

pub mod fs_tools;
pub mod orchestration;
pub mod planning;
pub mod registry;
pub mod shell;
pub mod team_tools;
pub mod tool;

use std::sync::Arc;

use registry::ToolRegistry;

/// 构建内置工具注册表 (注册顺序即导出顺序)
pub fn build_registry() -> ToolRegistry {
    let mut r = ToolRegistry::new();
    // s02 基础工具
    r.register(Arc::new(shell::ShellTool));
    r.register(Arc::new(fs_tools::ReadFileTool));
    r.register(Arc::new(fs_tools::WriteFileTool));
    r.register(Arc::new(fs_tools::EditFileTool));
    r.register(Arc::new(fs_tools::GlobTool));
    // s05 / s07 / s09
    r.register(Arc::new(planning::TodoWriteTool));
    r.register(Arc::new(planning::LoadSkillTool));
    r.register(Arc::new(planning::RememberTool));
    // s06 子代理
    r.register(Arc::new(team_tools::TaskTool));
    // s12 任务图
    r.register(Arc::new(orchestration::TaskCreateTool));
    r.register(Arc::new(orchestration::TaskListTool));
    r.register(Arc::new(orchestration::TaskClaimTool));
    r.register(Arc::new(orchestration::TaskCompleteTool));
    // s14 cron
    r.register(Arc::new(orchestration::CronScheduleTool));
    r.register(Arc::new(orchestration::CronListTool));
    r.register(Arc::new(orchestration::CronCancelTool));
    // s15-s16 团队
    r.register(Arc::new(team_tools::SpawnTeammateTool));
    r.register(Arc::new(team_tools::ShutdownTeammateTool));
    r.register(Arc::new(orchestration::SendMessageTool));
    // s18 worktree
    r.register(Arc::new(orchestration::WorktreeCreateTool));
    r.register(Arc::new(orchestration::WorktreeRemoveTool));
    r
}
