//! 记忆服务 — .memory/*.md + MEMORY.md 索引，跨会话、不参与压缩
//! 索引常驻 system prompt；每轮 side-query 选相关记忆注入；轮结束提取；达阈值合并

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;

use crate::constants::{MEMORY_CONSOLIDATE_THRESHOLD, MEMORY_INJECT_MAX};
use crate::llm::provider::LlmProvider;
use crate::llm::types::{ChatRequest, ContentBlock, Message};
use crate::persistence::db::Db;

#[derive(Debug, Clone, Serialize)]
pub struct MemoryItem {
    pub name: String,
    pub description: String,
    pub mtype: String, // user | feedback | project | reference
    pub content: String,
}

pub struct MemoryService {
    db: Arc<Db>,
    dir: PathBuf,
}

impl MemoryService {
    pub fn new(db: Arc<Db>, data_dir: &Path) -> Self {
        let dir = data_dir.join(".memory");
        let _ = std::fs::create_dir_all(&dir);
        Self { db, dir }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// 全部记忆
    pub fn all(&self) -> Vec<MemoryItem> {
        let conn = self.db.list_memories();
        conn.into_iter()
            .map(|(name, description, mtype, content)| MemoryItem {
                name,
                description,
                mtype,
                content,
            })
            .collect()
    }

    /// 索引 (注入 system prompt) — 一行一条
    pub fn index_block(&self) -> String {
        let items = self.all();
        if items.is_empty() {
            return String::new();
        }
        let mut lines = vec!["已保存的长期记忆 (需要时可参考):".to_string()];
        for m in items {
            lines.push(format!("- [{}] {}: {}", m.mtype, m.name, m.description));
        }
        lines.join("\n")
    }

    pub fn has_any(&self) -> bool {
        !self.all().is_empty()
    }

    /// 关键词降级选择 (side-query 失败时)
    pub fn select_by_keywords(&self, query: &str) -> Vec<MemoryItem> {
        let q = query.to_lowercase();
        let words: Vec<&str> = q.split_whitespace().collect();
        let mut scored: Vec<(usize, MemoryItem)> = self
            .all()
            .into_iter()
            .map(|m| {
                let hay = format!("{} {} {}", m.name, m.description, m.content).to_lowercase();
                let score = words.iter().filter(|w| hay.contains(**w)).count();
                (score, m)
            })
            .filter(|(s, _)| *s > 0)
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored
            .into_iter()
            .take(MEMORY_INJECT_MAX)
            .map(|(_, m)| m)
            .collect()
    }

    /// LLM side-query 选相关记忆，失败降级关键词
    pub async fn select_relevant(
        &self,
        provider: &Arc<dyn LlmProvider>,
        model: &str,
        query: &str,
    ) -> Vec<MemoryItem> {
        let items = self.all();
        if items.is_empty() {
            return vec![];
        }
        let catalog = items
            .iter()
            .map(|m| format!("{}: {}", m.name, m.description))
            .collect::<Vec<_>>()
            .join("\n");
        let sys = "根据用户当前输入，从记忆目录中挑选最相关的条目 (最多 5 个)。只返回名称，每行一个，不要解释。无相关则返回空。";
        let req = ChatRequest {
            model: model.to_string(),
            system: sys.to_string(),
            messages: vec![Message::user_text(format!(
                "记忆目录:\n{catalog}\n\n用户输入:\n{query}"
            ))],
            tools: vec![],
            max_tokens: 512,
            thinking: crate::llm::types::ThinkingLevel::Off,
        };
        let noop = |_e: crate::llm::types::StreamEvent| {};
        match provider.chat(&req, &noop).await {
            Ok(resp) => {
                let text = resp
                    .content
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::Text { text } => Some(text.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                let names: Vec<String> = text
                    .lines()
                    .map(|l| l.trim().trim_start_matches('-').trim().to_string())
                    .filter(|l| !l.is_empty())
                    .collect();
                let selected: Vec<MemoryItem> = items
                    .into_iter()
                    .filter(|m| names.iter().any(|n| n.eq_ignore_ascii_case(&m.name)))
                    .take(MEMORY_INJECT_MAX)
                    .collect();
                if selected.is_empty() {
                    self.select_by_keywords(query)
                } else {
                    selected
                }
            }
            Err(_) => self.select_by_keywords(query),
        }
    }

    /// 渲染注入块
    pub fn render_injection(items: &[MemoryItem]) -> Option<String> {
        if items.is_empty() {
            return None;
        }
        let mut out = vec!["[相关长期记忆]".to_string()];
        for m in items {
            out.push(format!("## {}\n{}", m.name, m.content));
        }
        Some(out.join("\n\n"))
    }

    /// 写入/更新一条记忆 (md 镜像 + db)
    pub fn upsert(&self, item: &MemoryItem) -> Result<(), String> {
        let safe_name = item.name.replace(['/', '\\', ' '], "_");
        let fpath = self.dir.join(format!("{safe_name}.md"));
        let md = format!(
            "---\nname: {}\ndescription: {}\ntype: {}\n---\n{}\n",
            item.name, item.description, item.mtype, item.content
        );
        let _ = std::fs::write(&fpath, md);
        self.db
            .upsert_memory(&item.name, &item.description, &item.mtype, &item.content)?;
        self.rebuild_index();
        Ok(())
    }

    pub fn delete(&self, name: &str) -> Result<(), String> {
        let safe_name = name.replace(['/', '\\', ' '], "_");
        let _ = std::fs::remove_file(self.dir.join(format!("{safe_name}.md")));
        self.db.delete_memory(name)?;
        self.rebuild_index();
        Ok(())
    }

    /// 重建 MEMORY.md 索引文件
    fn rebuild_index(&self) {
        let items = self.all();
        let mut out = String::from("# 记忆索引\n\n");
        for m in &items {
            let safe_name = m.name.replace(['/', '\\', ' '], "_");
            out.push_str(&format!(
                "- [{}]({}.md) — {}\n",
                m.name, safe_name, m.description
            ));
        }
        let _ = std::fs::write(self.dir.join("MEMORY.md"), out);
    }

    pub fn count(&self) -> usize {
        self.all().len()
    }

    pub fn needs_consolidation(&self) -> bool {
        self.count() >= MEMORY_CONSOLIDATE_THRESHOLD
    }

    /// 清空全部记忆 (consolidate 前重置)
    pub fn db_clear(&self) -> Result<(), String> {
        for m in self.all() {
            let safe_name = m.name.replace(['/', '\\', ' '], "_");
            let _ = std::fs::remove_file(self.dir.join(format!("{safe_name}.md")));
        }
        self.db.clear_memories()
    }
}
