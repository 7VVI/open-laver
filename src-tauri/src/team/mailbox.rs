//! s15 团队信箱 — .mailboxes/{agent}.jsonl 文件信箱，消费式读取

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailMessage {
    pub from: String,
    pub to: String,
    pub content: String,
    #[serde(rename = "type")]
    pub mtype: String, // text | summary | shutdown_request | shutdown_ack | plan_approval | plan_decision
    pub ts: String,
    #[serde(default)]
    pub request_id: Option<String>,
}

pub struct MailBus {
    dir: PathBuf,
    lock: Mutex<()>,
}

impl MailBus {
    pub fn new(data_dir: &Path) -> Self {
        let dir = data_dir.join(".mailboxes");
        let _ = std::fs::create_dir_all(&dir);
        Self {
            dir,
            lock: Mutex::new(()),
        }
    }

    fn path(&self, agent: &str) -> PathBuf {
        let safe = agent.replace(['/', '\\', ' '], "_");
        self.dir.join(format!("{safe}.jsonl"))
    }

    /// 发送: append 到收件人信箱
    pub fn send(&self, msg: &MailMessage) {
        let _g = self.lock.lock().unwrap();
        let line = match serde_json::to_string(msg) {
            Ok(s) => s,
            Err(_) => return,
        };
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.path(&msg.to))
        {
            let _ = writeln!(f, "{line}");
        }
    }

    /// 消费式读取: 读完清空
    pub fn drain(&self, agent: &str) -> Vec<MailMessage> {
        let _g = self.lock.lock().unwrap();
        let path = self.path(agent);
        let Ok(content) = std::fs::read_to_string(&path) else {
            return vec![];
        };
        let msgs: Vec<MailMessage> = content
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();
        let _ = std::fs::write(&path, "");
        msgs
    }

    /// 非消费式查看 (前端展示)
    pub fn peek(&self, agent: &str) -> Vec<MailMessage> {
        let _g = self.lock.lock().unwrap();
        let Ok(content) = std::fs::read_to_string(self.path(agent)) else {
            return vec![];
        };
        content
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
    }

    pub fn now_ts() -> String {
        chrono::Local::now().to_rfc3339()
    }
}
