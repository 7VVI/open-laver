//! 记忆提取 — 每轮结束对照已有记忆提取新条目 (extract_memories)

use std::sync::Arc;

use serde_json::Value;

use super::service::{MemoryItem, MemoryService};
use crate::llm::provider::LlmProvider;
use crate::llm::types::{ChatRequest, ContentBlock, Message};

/// 从最近对话中提取值得长期保存的记忆
pub async fn extract_memories(
    provider: &Arc<dyn LlmProvider>,
    model: &str,
    service: &MemoryService,
    recent: &str,
) {
    let existing = service
        .all()
        .iter()
        .map(|m| m.name.clone())
        .collect::<Vec<_>>()
        .join(", ");
    let sys = r#"你是记忆提取器。从对话中找出值得跨会话长期保存的事实，例如: 用户偏好、项目约定、稳定的环境信息、重要反馈。
只提取明确、稳定、可复用的信息，不要提取一次性的临时内容。
以 JSON 数组返回，每项为 {"name","description","type","content"}，type 取值 user/feedback/project/reference。
若无值得保存的内容，返回 []。不要输出多余文字。"#;
    let req = ChatRequest {
        model: model.to_string(),
        system: sys.to_string(),
        messages: vec![Message::user_text(format!(
            "已存在的记忆名称: [{existing}]\n\n最近对话:\n{recent}"
        ))],
        tools: vec![],
        max_tokens: 1024,
        thinking: crate::llm::types::ThinkingLevel::Off,
    };
    let noop = |_e: crate::llm::types::StreamEvent| {};
    let Ok(resp) = provider.chat(&req, &noop).await else {
        return;
    };
    let text = resp
        .content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    let json_str = extract_json_array(&text);
    let Ok(items) = serde_json::from_str::<Vec<Value>>(&json_str) else {
        return;
    };
    for it in items {
        let name = it["name"].as_str().unwrap_or("").trim().to_string();
        if name.is_empty() {
            continue;
        }
        let item = MemoryItem {
            name,
            description: it["description"].as_str().unwrap_or("").to_string(),
            mtype: it["type"].as_str().unwrap_or("reference").to_string(),
            content: it["content"].as_str().unwrap_or("").to_string(),
        };
        let _ = service.upsert(&item);
    }
}

/// consolidate — 达阈值时去重合并 (简化: 交给 LLM 归并)
pub async fn consolidate(
    provider: &Arc<dyn LlmProvider>,
    model: &str,
    service: &MemoryService,
) {
    let items = service.all();
    if items.len() < crate::constants::MEMORY_CONSOLIDATE_THRESHOLD {
        return;
    }
    let dump = items
        .iter()
        .map(|m| format!("{}|{}|{}|{}", m.name, m.mtype, m.description, m.content))
        .collect::<Vec<_>>()
        .join("\n");
    let sys = r#"你是记忆整理器。下面是现有记忆 (格式: name|type|description|content)。
合并语义重复的条目，删除过时或矛盾的信息，保持每条聚焦单一主题。
以 JSON 数组返回整理后的完整记忆集合，每项 {"name","description","type","content"}。不要输出多余文字。"#;
    let req = ChatRequest {
        model: model.to_string(),
        system: sys.to_string(),
        messages: vec![Message::user_text(dump)],
        tools: vec![],
        max_tokens: 4096,
        thinking: crate::llm::types::ThinkingLevel::Off,
    };
    let noop = |_e: crate::llm::types::StreamEvent| {};
    let Ok(resp) = provider.chat(&req, &noop).await else {
        return;
    };
    let text = resp.content.iter().filter_map(|b| match b {
        ContentBlock::Text { text } => Some(text.clone()),
        _ => None,
    }).collect::<Vec<_>>().join("\n");
    let json_str = extract_json_array(&text);
    let Ok(new_items) = serde_json::from_str::<Vec<Value>>(&json_str) else {
        return;
    };
    if new_items.is_empty() {
        return;
    }
    // 用整理后的集合替换
    let _ = service.db_clear();
    for it in new_items {
        let name = it["name"].as_str().unwrap_or("").trim().to_string();
        if name.is_empty() {
            continue;
        }
        let item = MemoryItem {
            name,
            description: it["description"].as_str().unwrap_or("").to_string(),
            mtype: it["type"].as_str().unwrap_or("reference").to_string(),
            content: it["content"].as_str().unwrap_or("").to_string(),
        };
        let _ = service.upsert(&item);
    }
}

fn extract_json_array(text: &str) -> String {
    if let (Some(start), Some(end)) = (text.find('['), text.rfind(']')) {
        if end > start {
            return text[start..=end].to_string();
        }
    }
    "[]".to_string()
}
