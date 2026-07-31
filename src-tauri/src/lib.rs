//! Laver Agent 库入口 — 注册状态、命令、后台工作线程

pub mod agent;
pub mod background;
pub mod constants;
pub mod cron;
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

use std::sync::Arc;

use tauri::Manager;

use state::AppState;

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
        .setup(|app| {
            // 数据目录: {app_data}/laver-agent/
            let data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::env::temp_dir())
                .join("laver-agent");
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
            gateway::commands::resolve_approval,
            gateway::commands::get_workspace,
            gateway::commands::set_workspace,
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
        ])
        .run(tauri::generate_context!())
        .expect("运行 Laver Agent 失败");
}
