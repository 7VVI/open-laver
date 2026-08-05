//! SQLite 持久化层：会话消息、cron jobs、设置、记忆镜像

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection};

use crate::llm::types::Message;

pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self, String> {
        let conn = Connection::open(path).map_err(|e| e.to_string())?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                created_at TEXT NOT NULL,
                pinned INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS messages (
                session_id TEXT NOT NULL,
                seq INTEGER NOT NULL,
                json TEXT NOT NULL,
                PRIMARY KEY (session_id, seq)
            );
            CREATE TABLE IF NOT EXISTS cron_jobs (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL DEFAULT '',
                expr TEXT NOT NULL,
                prompt TEXT NOT NULL,
                recurring INTEGER NOT NULL,
                durable INTEGER NOT NULL,
                session_id TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS cron_runs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                job_id TEXT NOT NULL,
                title TEXT NOT NULL,
                prompt TEXT NOT NULL,
                trigger TEXT NOT NULL,
                ran_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS memories (
                name TEXT PRIMARY KEY,
                description TEXT NOT NULL,
                mtype TEXT NOT NULL,
                content TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            "#,
        )
        .map_err(|e| e.to_string())?;
        // 旧库迁移: cron_jobs 补充 title 列 (已存在则忽略错误)
        {
            let _ = conn.execute(
                "ALTER TABLE cron_jobs ADD COLUMN title TEXT NOT NULL DEFAULT ''",
                [],
            );
            // 旧库迁移: sessions 补充 pinned 列
            let _ = conn.execute(
                "ALTER TABLE sessions ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0",
                [],
            );
        }
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    // ---------- settings KV ----------

    pub fn get_setting(&self, key: &str) -> Option<String> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |r| r.get(0),
        )
        .ok()
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO settings(key, value) VALUES(?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = ?2",
            params![key, value],
        )
        .map(|_| ())
        .map_err(|e| e.to_string())
    }

    // ---------- sessions ----------

    pub fn upsert_session(&self, id: &str, title: &str, created_at: &str) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO sessions(id, title, created_at) VALUES(?1, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET title = ?2",
            params![id, title, created_at],
        )
        .map(|_| ())
        .map_err(|e| e.to_string())
    }

    pub fn list_sessions(&self) -> Vec<(String, String, String, bool)> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = match conn.prepare(
            "SELECT id, title, created_at, pinned FROM sessions ORDER BY pinned DESC, created_at DESC",
        ) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        stmt.query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get::<_, i64>(3)? != 0))
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    pub fn rename_session(&self, id: &str, title: &str) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE sessions SET title = ?2 WHERE id = ?1",
            params![id, title],
        )
        .map(|_| ())
        .map_err(|e| e.to_string())
    }

    pub fn set_session_pinned(&self, id: &str, pinned: bool) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE sessions SET pinned = ?2 WHERE id = ?1",
            params![id, pinned as i64],
        )
        .map(|_| ())
        .map_err(|e| e.to_string())
    }

    pub fn delete_session(&self, id: &str) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM messages WHERE session_id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM sessions WHERE id = ?1", params![id])
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// 整体重写会话消息 (每轮结束时调用)
    pub fn save_messages(&self, session_id: &str, messages: &[Message]) -> Result<(), String> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        tx.execute(
            "DELETE FROM messages WHERE session_id = ?1",
            params![session_id],
        )
        .map_err(|e| e.to_string())?;
        for (i, m) in messages.iter().enumerate() {
            let json = serde_json::to_string(m).map_err(|e| e.to_string())?;
            tx.execute(
                "INSERT INTO messages(session_id, seq, json) VALUES(?1, ?2, ?3)",
                params![session_id, i as i64, json],
            )
            .map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())
    }

    pub fn load_messages(&self, session_id: &str) -> Vec<Message> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = match conn
            .prepare("SELECT json FROM messages WHERE session_id = ?1 ORDER BY seq ASC")
        {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        stmt.query_map(params![session_id], |r| r.get::<_, String>(0))
            .map(|rows| {
                rows.filter_map(|r| r.ok())
                    .filter_map(|j| serde_json::from_str(&j).ok())
                    .collect()
            })
            .unwrap_or_default()
    }

    // ---------- cron jobs (durable) ----------

    pub fn save_cron_job(
        &self,
        id: &str,
        title: &str,
        expr: &str,
        prompt: &str,
        recurring: bool,
        durable: bool,
        session_id: &str,
    ) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO cron_jobs(id, title, expr, prompt, recurring, durable, session_id, created_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET title=?2, expr=?3, prompt=?4, recurring=?5, durable=?6",
            params![
                id,
                title,
                expr,
                prompt,
                recurring as i64,
                durable as i64,
                session_id,
                chrono::Local::now().to_rfc3339()
            ],
        )
        .map(|_| ())
        .map_err(|e| e.to_string())
    }

    pub fn delete_cron_job(&self, id: &str) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM cron_jobs WHERE id = ?1", params![id])
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// (id, title, expr, prompt, recurring, session_id)
    pub fn load_durable_cron_jobs(&self) -> Vec<(String, String, String, String, bool, String)> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = match conn.prepare(
            "SELECT id, title, expr, prompt, recurring, session_id FROM cron_jobs WHERE durable = 1",
        ) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        stmt.query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get::<_, i64>(4)? != 0,
                r.get(5)?,
            ))
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    // ---------- cron 执行记录 ----------

    pub fn record_cron_run(&self, job_id: &str, title: &str, prompt: &str, trigger: &str) {
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute(
            "INSERT INTO cron_runs(job_id, title, prompt, trigger, ran_at) VALUES(?1, ?2, ?3, ?4, ?5)",
            params![job_id, title, prompt, trigger, chrono::Local::now().to_rfc3339()],
        );
    }

    /// (job_id, title, prompt, trigger, ran_at) 最近 100 条
    pub fn list_cron_runs(&self) -> Vec<(String, String, String, String, String)> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = match conn.prepare(
            "SELECT job_id, title, prompt, trigger, ran_at FROM cron_runs ORDER BY id DESC LIMIT 100",
        ) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        stmt.query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    // ---------- memory 镜像 ----------

    pub fn upsert_memory(
        &self,
        name: &str,
        description: &str,
        mtype: &str,
        content: &str,
    ) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO memories(name, description, mtype, content, updated_at)
             VALUES(?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(name) DO UPDATE SET description=?2, mtype=?3, content=?4, updated_at=?5",
            params![
                name,
                description,
                mtype,
                content,
                chrono::Local::now().to_rfc3339()
            ],
        )
        .map(|_| ())
        .map_err(|e| e.to_string())
    }

    pub fn delete_memory(&self, name: &str) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM memories WHERE name = ?1", params![name])
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub fn clear_memories(&self) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM memories", [])
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// (name, description, mtype, content)
    pub fn list_memories(&self) -> Vec<(String, String, String, String)> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = match conn.prepare(
            "SELECT name, description, mtype, content FROM memories ORDER BY updated_at DESC",
        ) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    }
}
