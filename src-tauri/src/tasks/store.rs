//! s12 任务图 — 文件持久化 (.tasks/{id}.json)，blockedBy 依赖驱动
//! 状态机: pending -> (claim) in_progress -> (complete) completed

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use fs4::fs_std::FileExt;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub subject: String,
    pub description: String,
    pub status: String, // pending | in_progress | completed
    pub owner: Option<String>,
    #[serde(default)]
    pub blocked_by: Vec<String>,
    #[serde(default)]
    pub worktree: Option<String>,
}

pub struct TaskStore {
    dir: PathBuf,
    lock: Mutex<()>, // 进程内串行化，配合文件锁防并发认领
}

impl TaskStore {
    pub fn new(data_dir: &Path) -> Self {
        let dir = data_dir.join(".tasks");
        let _ = std::fs::create_dir_all(&dir);
        Self {
            dir,
            lock: Mutex::new(()),
        }
    }

    fn path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }

    pub fn create(
        &self,
        subject: &str,
        description: &str,
        blocked_by: Vec<String>,
    ) -> Result<Task, String> {
        let _g = self.lock.lock().unwrap();
        let id = format!("t-{}", uuid::Uuid::new_v4().simple());
        let task = Task {
            id: id.clone(),
            subject: subject.to_string(),
            description: description.to_string(),
            status: "pending".into(),
            owner: None,
            blocked_by,
            worktree: None,
        };
        self.write(&task)?;
        Ok(task)
    }

    fn write(&self, task: &Task) -> Result<(), String> {
        let json = serde_json::to_string_pretty(task).map_err(|e| e.to_string())?;
        std::fs::write(self.path(&task.id), json).map_err(|e| e.to_string())
    }

    pub fn get(&self, id: &str) -> Option<Task> {
        let raw = std::fs::read_to_string(self.path(id)).ok()?;
        serde_json::from_str(&raw).ok()
    }

    pub fn all(&self) -> Vec<Task> {
        let mut tasks = Vec::new();
        if let Ok(rd) = std::fs::read_dir(&self.dir) {
            for e in rd.flatten() {
                if e.path().extension().and_then(|x| x.to_str()) == Some("json") {
                    if let Ok(raw) = std::fs::read_to_string(e.path()) {
                        if let Ok(t) = serde_json::from_str::<Task>(&raw) {
                            tasks.push(t);
                        }
                    }
                }
            }
        }
        tasks.sort_by(|a, b| a.id.cmp(&b.id));
        tasks
    }

    /// 所有上游是否 completed
    pub fn can_start(&self, task: &Task) -> bool {
        task.blocked_by.iter().all(|dep| {
            self.get(dep)
                .map(|t| t.status == "completed")
                .unwrap_or(true) // 依赖不存在视为已满足
        })
    }

    /// 原子认领: 文件锁 + owner 检查
    pub fn claim(&self, id: &str, owner: &str) -> Result<Task, String> {
        let _g = self.lock.lock().unwrap();
        let path = self.path(id);
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|e| format!("任务不存在: {e}"))?;
        file.lock_exclusive().map_err(|e| e.to_string())?;
        let res = (|| {
            let mut task = self.get(id).ok_or("任务读取失败")?;
            if task.status != "pending" {
                return Err(format!("任务状态为 {}，无法认领", task.status));
            }
            if task.owner.is_some() {
                return Err("任务已被认领".into());
            }
            if !self.can_start(&task) {
                return Err("上游依赖未完成，暂不可认领".into());
            }
            task.owner = Some(owner.to_string());
            task.status = "in_progress".into();
            self.write(&task)?;
            Ok(task)
        })();
        let _ = FileExt::unlock(&file);
        res
    }

    /// 完成任务，返回新解锁的下游任务
    pub fn complete(&self, id: &str) -> Result<Vec<Task>, String> {
        let _g = self.lock.lock().unwrap();
        let mut task = self.get(id).ok_or("任务不存在")?;
        task.status = "completed".into();
        self.write(&task)?;
        // 找出因此解锁的下游
        let unlocked: Vec<Task> = self
            .all()
            .into_iter()
            .filter(|t| {
                t.status == "pending"
                    && t.blocked_by.contains(&id.to_string())
                    && self.can_start(t)
            })
            .collect();
        Ok(unlocked)
    }

    pub fn bind_worktree(&self, id: &str, worktree: &str) -> Result<(), String> {
        let _g = self.lock.lock().unwrap();
        let mut task = self.get(id).ok_or("任务不存在")?;
        task.worktree = Some(worktree.to_string());
        self.write(&task)
    }

    /// s17 自主认领: 扫描可认领任务 (pending + 无 owner + can_start)
    pub fn claimable(&self) -> Vec<Task> {
        self.all()
            .into_iter()
            .filter(|t| t.status == "pending" && t.owner.is_none() && self.can_start(t))
            .collect()
    }
}
