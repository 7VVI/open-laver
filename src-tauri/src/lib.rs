//! Laver Agent 库入口 — 注册状态、命令、后台工作线程

pub mod agent;
pub mod background;
pub mod constants;
pub mod cron;
pub mod design;
pub mod gateway;
pub mod llm;
pub mod mcp;
pub mod model_provider;
pub mod persistence;
pub mod seed;
pub mod state;
pub mod tasks;
pub mod team;
pub mod worktree;
pub mod workers;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tauri::Manager;

use state::AppState;

/// 程序数据目录: C:\Users\{user}\.Laver (主流 Agent 的 ~/.xxx 做法)
fn home_laver_data_dir() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("USERPROFILE") {
        return Some(PathBuf::from(p).join(".Laver"));
    }
    let drive = std::env::var_os("HOMEDRIVE")?;
    let path = std::env::var_os("HOMEPATH")?;
    Some(PathBuf::from(drive).join(path).join(".Laver"))
}

/// 递归复制目录 (首次迁移旧数据目录用, 非破坏性)
fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // 数据目录: C:\Users\{user}\.Laver (skills/.memory/laver.db 等都在其下)
            let old_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::env::temp_dir())
                .join("laver-agent");
            let data_dir = home_laver_data_dir().unwrap_or_else(|| old_dir.clone());
            // 首次迁移旧数据目录 (旧目录保留, 不删除)
            if !data_dir.exists() && old_dir.exists() && old_dir != data_dir {
                let _ = copy_dir_all(&old_dir, &data_dir);
            }
            std::fs::create_dir_all(&data_dir).ok();
            std::fs::create_dir_all(data_dir.join("skills")).ok();
            // 播种内置办公技能 (首次运行)
            seed::seed_bundled_skills(&data_dir.join("skills"));

            let registry = Arc::new(agent::tools::build_registry());
            let app_state = AppState::new(data_dir.clone(), registry);

            // MCP 配置路径 + 连接
            {
                let st = app_state.clone();
                let cfg_path = data_dir.join("mcpServers.json");
                tauri::async_runtime::spawn(async move {
                    st.mcp.set_config_path(cfg_path).await;
                    st.mcp.connect_all().await;
                });
            }

            // 启动 cron 后台工作线程
            workers::spawn_cron_workers(app_state.clone(), app.handle().clone());

            app.manage(app_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            gateway::commands::create_session,
            gateway::commands::list_sessions,
            gateway::commands::delete_session,
            gateway::commands::rename_session,
            gateway::commands::set_session_pinned,
            gateway::commands::export_session,
            gateway::commands::load_session_messages,
            gateway::commands::send_message,
            gateway::commands::cancel_turn,
            gateway::commands::is_session_running,
            gateway::commands::resolve_approval,
            gateway::commands::get_workspace,
            gateway::commands::set_workspace,
            gateway::commands::get_permission_mode,
            gateway::commands::set_permission_mode,
            gateway::commands::set_default_workspace,
            gateway::commands::get_storage_info,
            gateway::commands::list_dir_tree,
            gateway::commands::open_path,
            gateway::commands::list_models,
            gateway::commands::save_model,
            gateway::commands::delete_model,
            gateway::commands::set_active_model,
            gateway::commands::set_model_key,
            gateway::commands::set_model_runtime,
            gateway::commands::test_connection,
            gateway::commands::list_skills,
            gateway::commands::skills_root,
            gateway::commands::rescan_skills,
            gateway::commands::create_skill,
            gateway::commands::import_skill_zip,
            gateway::commands::read_skill,
            gateway::commands::delete_skill,
            gateway::commands::list_memories,
            gateway::commands::delete_memory,
            gateway::commands::list_tasks,
            gateway::commands::list_cron_jobs,
            gateway::commands::cancel_cron_job,
            gateway::commands::create_cron_job,
            gateway::commands::run_cron_now,
            gateway::commands::list_cron_runs,
            gateway::commands::list_teammates,
            gateway::commands::get_todos,
            gateway::commands::list_mcp_servers,
            gateway::commands::get_mcp_config,
            gateway::commands::save_mcp_config,
            gateway::commands::generate_design,
            gateway::commands::read_design,
            gateway::commands::list_designs,
            gateway::commands::delete_design,
            gateway::commands::get_design_config,
            gateway::commands::save_design_config,
            gateway::commands::test_design_connection,
        ])
        .run(tauri::generate_context!())
        .expect("运行 Laver Agent 失败");
}
