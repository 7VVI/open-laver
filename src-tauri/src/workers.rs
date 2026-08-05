//! 后台工作线程: cron 调度器轮询 + 队列处理器 (空闲时交付)

use std::sync::Arc;
use std::time::Duration;

use crate::gateway::events::notice;
use crate::state::AppState;

/// 启动 cron 调度器 (每秒 tick) 与队列处理器 (空闲交付)
pub fn spawn_cron_workers(state: Arc<AppState>, app: tauri::AppHandle) {
    // 恢复 durable 任务
    for (id, title, expr, prompt, recurring, session_id) in state.db.load_durable_cron_jobs() {
        let _ = state.cron.add(crate::cron::scheduler::CronJob {
            id,
            title,
            expr,
            prompt,
            recurring,
            durable: true,
            session_id,
        });
    }

    // 调度器 tick
    {
        let state = state.clone();
        tauri::async_runtime::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            loop {
                interval.tick().await;
                state.cron.tick();
            }
        });
    }

    // 队列处理器: 消费触发的 cron，等待目标会话空闲后注入
    if let Some(mut rx) = state.cron.take_receiver() {
        let state = state.clone();
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            while let Some(fire) = rx.recv().await {
                let state2 = state.clone();
                let app2 = app.clone();
                tauri::async_runtime::spawn(async move {
                    deliver_cron(state2, app2, fire).await;
                });
            }
        });
    }
}

async fn deliver_cron(
    state: Arc<AppState>,
    app: tauri::AppHandle,
    fire: crate::cron::scheduler::CronFire,
) {
    use std::sync::atomic::Ordering;

    // 等待目标会话空闲 (最多 5 分钟)
    let mut waited = 0u64;
    loop {
        let session = state.sessions.get(&fire.session_id);
        let busy = session
            .as_ref()
            .map(|s| s.running.load(Ordering::SeqCst))
            .unwrap_or(false);
        if !busy {
            break;
        }
        if waited > 300 {
            return; // 放弃
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
        waited += 2;
    }

    // 确保会话存在
    if state.sessions.get(&fire.session_id).is_none() {
        let h = crate::agent::session::SessionHandle::new(fire.session_id.clone(), "会话".into());
        let msgs = state.db.load_messages(&fire.session_id);
        {
            let mut st = h.state.lock().await;
            st.messages = msgs;
        }
        state.sessions.insert(h);
    }

    notice(&app, "info", format!("定时任务触发: {}", fire.prompt));
    state
        .db
        .record_cron_run(&fire.job_id, &fire.title, &fire.prompt, "scheduled");
    let prompt = format!("[Scheduled] {}", fire.prompt);
    crate::agent::loop_engine::run_turn(state, app, fire.session_id, prompt).await;
}
