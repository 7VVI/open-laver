//! s08 上下文压缩管线 — 四层，顺序固定 (便宜的先跑)
//! L3 tool_result_budget -> L1 snip -> L2 micro -> (超阈值) L4 compact_history
//! + reactive_compact (prompt_too_long 应急)

use std::path::Path;
use std::sync::Arc;

use crate::constants::*;
use crate::llm::provider::LlmProvider;
use crate::llm::types::{
    estimate_messages_tokens, ChatRequest, ContentBlock, LlmError, Message, Role,
};

/// 预处理: 每轮 LLM 调用前跑 L3 -> L1 -> L2 (0 API)
pub fn preprocess(messages: &mut Vec<Message>, task_outputs_dir: &Path) {
    tool_result_budget(messages, task_outputs_dir); // L3
    snip_compact(messages); // L1
    micro_compact(messages); // L2
}

/// L3: 末条 user 消息 tool_result 总量超预算 -> 落盘，留预览 + 标记
fn tool_result_budget(messages: &mut [Message], dir: &Path) {
    let Some(last) = messages.last_mut() else {
        return;
    };
    if last.role != Role::User {
        return;
    }
    let total: usize = last
        .content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::ToolResult { content, .. } => Some(content.len()),
            _ => None,
        })
        .sum();
    if total <= TOOL_RESULT_BUDGET_BYTES {
        return;
    }
    let _ = std::fs::create_dir_all(dir);
    for b in last.content.iter_mut() {
        if let ContentBlock::ToolResult {
            tool_use_id,
            content,
            ..
        } = b
        {
            if content.len() > TOOL_RESULT_PREVIEW_CHARS {
                let fname = format!("{}-{}.txt", tool_use_id, uuid::Uuid::new_v4().simple());
                let fpath = dir.join(&fname);
                let _ = std::fs::write(&fpath, content.as_bytes());
                let preview: String = content.chars().take(TOOL_RESULT_PREVIEW_CHARS).collect();
                *content = format!(
                    "{preview}\n... [完整输出已落盘: {}，需要时用 read_file 读取]",
                    fpath.display()
                );
            }
        }
    }
}

/// L1: 消息超阈值 -> 保头 SNIP_KEEP_HEAD + 尾 SNIP_KEEP_TAIL，切口保护配对
fn snip_compact(messages: &mut Vec<Message>) {
    if messages.len() <= SNIP_THRESHOLD_MSGS {
        return;
    }
    let head = SNIP_KEEP_HEAD.min(messages.len());
    let tail_start = messages.len().saturating_sub(SNIP_KEEP_TAIL);
    let mut cut = tail_start.max(head);
    // 保护: 切口不能落在 tool_result 起始 (其对应 tool_use 在前面被裁掉会破坏配对)
    while cut < messages.len() && starts_with_tool_result(&messages[cut]) {
        cut += 1;
    }
    if cut <= head {
        return;
    }
    let removed = cut - head;
    // 索引重建: [0..head) + 省略标记 + [cut..]
    let mut rebuilt: Vec<Message> = Vec::with_capacity(head + 1 + (messages.len() - cut));
    rebuilt.extend_from_slice(&messages[..head]);
    rebuilt.push(Message::assistant_text(format!(
        "[... 省略了 {removed} 条较早的消息以节省上下文 ...]"
    )));
    rebuilt.extend_from_slice(&messages[cut..]);
    *messages = rebuilt;
}

fn starts_with_tool_result(m: &Message) -> bool {
    matches!(m.content.first(), Some(ContentBlock::ToolResult { .. }))
}

/// L2: 仅保留最近 KEEP_RECENT_TOOL_RESULTS 条 tool_result 全文，更早的替换为占位符
fn micro_compact(messages: &mut [Message]) {
    // 从后往前数 tool_result，超过 N 的置为占位
    let mut seen = 0usize;
    for m in messages.iter_mut().rev() {
        for b in m.content.iter_mut().rev() {
            if let ContentBlock::ToolResult { content, .. } = b {
                seen += 1;
                if seen > KEEP_RECENT_TOOL_RESULTS && content.len() > 200 {
                    let preview: String = content.chars().take(200).collect();
                    *content = format!("[旧工具结果已省略] {preview}...");
                }
            }
        }
    }
}

/// L4: LLM 摘要整体替换 — 返回是否成功
pub async fn compact_history(
    provider: &Arc<dyn LlmProvider>,
    model: &str,
    messages: &mut Vec<Message>,
    transcript_path: &Path,
) -> Result<(), LlmError> {
    // 写 JSONL transcript (审计/恢复)
    if let Ok(mut buf) = serde_json::to_string(&*messages) {
        buf.push('\n');
        let _ = std::fs::write(transcript_path, buf);
    }
    let convo = render_transcript(messages);
    let sys = "你是对话摘要器。将下面的 Agent 对话浓缩为结构化摘要，必须保留: 用户目标、关键决策、已完成的操作、产生的文件/结果、未完成的待办。用中文，简洁但不丢关键事实。";
    let req = ChatRequest {
        model: model.to_string(),
        system: sys.to_string(),
        messages: vec![Message::user_text(format!(
            "请摘要以下对话:\n\n{convo}"
        ))],
        tools: vec![],
        max_tokens: MAX_OUTPUT_TOKENS,
        thinking: crate::llm::types::ThinkingLevel::Off,
    };
    let noop = |_e: crate::llm::types::StreamEvent| {};
    let resp = provider.chat(&req, &noop).await?;
    let summary = resp
        .content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    if summary.trim().is_empty() {
        return Err(LlmError::Api {
            status: 0,
            message: "摘要为空".into(),
        });
    }
    *messages = vec![Message::user_text(format!(
        "[对话历史摘要 — 之前的对话已压缩]\n{summary}"
    ))];
    Ok(())
}

/// reactive: prompt_too_long 应急 — 摘要 + 尾 REACTIVE_KEEP_TAIL 条
pub async fn reactive_compact(
    provider: &Arc<dyn LlmProvider>,
    model: &str,
    messages: &mut Vec<Message>,
    transcript_path: &Path,
) -> Result<(), LlmError> {
    if messages.len() <= REACTIVE_KEEP_TAIL {
        return Ok(());
    }
    let split = messages.len() - REACTIVE_KEEP_TAIL;
    let mut head: Vec<Message> = messages[..split].to_vec();
    compact_history(provider, model, &mut head, transcript_path).await?;
    let mut new_msgs = head;
    new_msgs.extend_from_slice(&messages[split..]);
    // 保证首条为 user (API 要求)
    fix_leading_role(&mut new_msgs);
    *messages = new_msgs;
    Ok(())
}

/// 阈值判定: 估算 token 是否超过 (contextWindow - maxOutput - RESERVE)
pub fn should_compact(messages: &[Message], context_window: usize, max_output: usize) -> bool {
    let threshold = context_window
        .saturating_sub(max_output)
        .saturating_sub(COMPACT_RESERVE_TOKENS);
    estimate_messages_tokens(messages) > threshold
}

fn render_transcript(messages: &[Message]) -> String {
    let mut out = String::new();
    for m in messages {
        let role = match m.role {
            Role::User => "USER",
            Role::Assistant => "ASSISTANT",
        };
        for b in &m.content {
            match b {
                ContentBlock::Text { text } => {
                    out.push_str(&format!("[{role}] {text}\n"));
                }
                ContentBlock::ToolUse { name, input, .. } => {
                    out.push_str(&format!("[{role} 调用工具 {name}] {input}\n"));
                }
                ContentBlock::ToolResult { content, .. } => {
                    let preview: String = content.chars().take(500).collect();
                    out.push_str(&format!("[工具结果] {preview}\n"));
                }
            }
        }
    }
    out
}

fn fix_leading_role(messages: &mut Vec<Message>) {
    if let Some(first) = messages.first() {
        if first.role != Role::User {
            messages.insert(0, Message::user_text("[继续]"));
        }
    }
}
