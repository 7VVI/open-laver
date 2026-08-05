//! Cron 调度器 — croner 解析，tokio 每秒轮询 + minute_marker 防重复
//! 触发入 mpsc 队列，Agent 空闲时注入 [Scheduled] 消息；durable 持久化跨重启

use std::collections::HashMap;
use std::sync::Mutex;

use chrono::{Local, Timelike};
use croner::Cron;
use serde::Serialize;
use std::str::FromStr;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize)]
pub struct CronJob {
    pub id: String,
    #[serde(default)]
    pub title: String,
    pub expr: String,
    pub prompt: String,
    pub recurring: bool,
    pub durable: bool,
    pub session_id: String,
}

/// 触发的任务 (投递到队列)
#[derive(Debug, Clone)]
pub struct CronFire {
    pub job_id: String,
    pub title: String,
    pub session_id: String,
    pub prompt: String,
}

pub struct CronScheduler {
    jobs: Mutex<HashMap<String, CronJob>>,
    /// 防重复: job_id -> 上次触发的分钟标记
    last_fired: Mutex<HashMap<String, String>>,
    tx: mpsc::UnboundedSender<CronFire>,
    rx: Mutex<Option<mpsc::UnboundedReceiver<CronFire>>>,
}

impl CronScheduler {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            jobs: Mutex::new(HashMap::new()),
            last_fired: Mutex::new(HashMap::new()),
            tx,
            rx: Mutex::new(Some(rx)),
        }
    }

    /// 取出接收端 (只能一次) — 交给队列处理器
    pub fn take_receiver(&self) -> Option<mpsc::UnboundedReceiver<CronFire>> {
        self.rx.lock().unwrap().take()
    }

    pub fn add(&self, job: CronJob) -> Result<(), String> {
        // 校验表达式
        Cron::from_str(&job.expr).map_err(|e| format!("cron 表达式无效: {e}"))?;
        self.jobs.lock().unwrap().insert(job.id.clone(), job);
        Ok(())
    }

    pub fn remove(&self, id: &str) {
        self.jobs.lock().unwrap().remove(id);
        self.last_fired.lock().unwrap().remove(id);
    }

    pub fn list(&self) -> Vec<CronJob> {
        self.jobs.lock().unwrap().values().cloned().collect()
    }

    /// 每秒轮询: 判断哪些 job 到点，投递并处理 recurring/一次性
    pub fn tick(&self) {
        let now = Local::now();
        // 分钟标记 (date-aware，防同一分钟重复触发)
        let marker = format!("{}", now.format("%Y-%m-%d %H:%M"));
        // 仅在每分钟的第 0 秒附近判断，减少无谓匹配
        let jobs: Vec<CronJob> = self.jobs.lock().unwrap().values().cloned().collect();
        for job in jobs {
            let Ok(cron) = Cron::from_str(&job.expr) else {
                continue;
            };
            // croner: 判断当前时间是否匹配
            let matched = cron.is_time_matching(&now).unwrap_or(false);
            if !matched {
                continue;
            }
            {
                let mut lf = self.last_fired.lock().unwrap();
                if lf.get(&job.id).map(|m| m == &marker).unwrap_or(false) {
                    continue; // 本分钟已触发
                }
                lf.insert(job.id.clone(), marker.clone());
            }
            let _ = self.tx.send(CronFire {
                job_id: job.id.clone(),
                title: job.title.clone(),
                session_id: job.session_id.clone(),
                prompt: job.prompt.clone(),
            });
            // 一次性任务触发后移除
            if !job.recurring {
                self.remove(&job.id);
            }
        }
        let _ = now.second();
    }
}

impl Default for CronScheduler {
    fn default() -> Self {
        Self::new()
    }
}
