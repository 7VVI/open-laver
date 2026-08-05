//! 统一内部消息模型 — Anthropic tool_use 与 OpenAI function calling 双协议的公共抽象

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

/// 消息内容块：与 Anthropic 的 content block 语义一致
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(default)]
        is_error: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: Vec<ContentBlock>,
}

impl Message {
    pub fn user_text(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: vec![ContentBlock::Text { text: text.into() }],
        }
    }

    pub fn assistant_text(text: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: vec![ContentBlock::Text { text: text.into() }],
        }
    }

    pub fn tool_results(results: Vec<ContentBlock>) -> Self {
        Self {
            role: Role::User,
            content: results,
        }
    }

    /// 提取全部文本内容
    pub fn text(&self) -> String {
        self.content
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn tool_uses(&self) -> Vec<(&str, &str, &Value)> {
        self.content
            .iter()
            .filter_map(|b| match b {
                ContentBlock::ToolUse { id, name, input } => {
                    Some((id.as_str(), name.as_str(), input))
                }
                _ => None,
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
}

#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub content: Vec<ContentBlock>,
    pub stop_reason: StopReason,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
}

impl LlmResponse {
    /// CC 语义: 不只看 stop_reason，内容里有 tool_use 块即需要继续 (needsFollowUp)
    pub fn needs_follow_up(&self) -> bool {
        self.stop_reason == StopReason::ToolUse
            || self
                .content
                .iter()
                .any(|b| matches!(b, ContentBlock::ToolUse { .. }))
    }
}

/// 工具定义 (JSON schema)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub model: String,
    pub system: String,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolSpec>,
    pub max_tokens: u32,
    pub thinking: ThinkingLevel,
}

/// 思考模式程度 — 控制模型推理深度 (映射到各家 API 的 reasoning 参数)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThinkingLevel {
    /// 关闭思考 (默认，兼容所有 API)
    #[default]
    Off,
    Low,
    Medium,
    High,
}

impl ThinkingLevel {
    /// 映射为思考 token 预算 (Anthropic budget_tokens / Qwen thinking_budget)
    pub fn budget_tokens(self) -> Option<u32> {
        match self {
            ThinkingLevel::Off => None,
            ThinkingLevel::Low => Some(2048),
            ThinkingLevel::Medium => Some(8192),
            ThinkingLevel::High => Some(16384),
        }
    }

    /// 映射为 OpenAI reasoning_effort 字符串
    pub fn reasoning_effort(self) -> Option<&'static str> {
        match self {
            ThinkingLevel::Off => None,
            ThinkingLevel::Low => Some("low"),
            ThinkingLevel::Medium => Some("medium"),
            ThinkingLevel::High => Some("high"),
        }
    }

    pub fn enabled(self) -> bool {
        self != ThinkingLevel::Off
    }
}

/// 流式事件 — 推送前端渲染
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StreamEvent {
    TextDelta { text: String },
    ToolUseStart { name: String },
    Done,
}

/// LLM 调用错误分类 (错误恢复的输入)
#[derive(Debug, Clone)]
pub enum LlmError {
    /// 429
    RateLimited { retry_after_ms: Option<u64> },
    /// 529 / overloaded_error
    Overloaded,
    /// 请求超出上下文窗口
    PromptTooLong,
    /// 认证失败 (401/403)
    Auth(String),
    /// 其他 API 错误
    Api { status: u16, message: String },
    /// 网络层错误
    Network(String),
}

impl std::fmt::Display for LlmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LlmError::RateLimited { .. } => write!(f, "rate limited (429)"),
            LlmError::Overloaded => write!(f, "overloaded (529)"),
            LlmError::PromptTooLong => write!(f, "prompt too long"),
            LlmError::Auth(m) => write!(f, "auth error: {m}"),
            LlmError::Api { status, message } => write!(f, "api error {status}: {message}"),
            LlmError::Network(m) => write!(f, "network error: {m}"),
        }
    }
}

impl std::error::Error for LlmError {}

/// 粗略 token 估算: ~4 字符/token (经验启发式)
pub fn estimate_tokens_str(s: &str) -> usize {
    s.chars().count() / 4 + 1
}

pub fn estimate_block_tokens(b: &ContentBlock) -> usize {
    match b {
        ContentBlock::Text { text } => estimate_tokens_str(text),
        ContentBlock::ToolUse { name, input, .. } => {
            estimate_tokens_str(name) + estimate_tokens_str(&input.to_string())
        }
        ContentBlock::ToolResult { content, .. } => estimate_tokens_str(content),
    }
}

pub fn estimate_messages_tokens(msgs: &[Message]) -> usize {
    msgs.iter()
        .map(|m| m.content.iter().map(estimate_block_tokens).sum::<usize>() + 4)
        .sum()
}
