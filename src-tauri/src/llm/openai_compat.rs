//! OpenAI 兼容提供商 (通义千问/DeepSeek/Kimi/Moonshot 等, function calling, SSE 流式)

use async_trait::async_trait;
use serde_json::{json, Value};

use super::provider::{DeltaSink, LlmProvider};
use super::sse::for_each_sse_data;
use super::types::{
    ChatRequest, ContentBlock, LlmError, LlmResponse, Message, Role, StopReason, StreamEvent,
};

pub struct OpenAiCompatProvider {
    base_url: String,
    api_key: String,
    client: reqwest::Client,
}

impl OpenAiCompatProvider {
    pub fn new(base_url: String, api_key: String) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            client: reqwest::Client::new(),
        }
    }

    /// 内部消息模型 -> OpenAI chat messages
    fn convert_messages(system: &str, messages: &[Message]) -> Vec<Value> {
        let mut out = Vec::new();
        if !system.is_empty() {
            out.push(json!({ "role": "system", "content": system }));
        }
        for m in messages {
            match m.role {
                Role::Assistant => {
                    let text = m.text();
                    let tool_calls: Vec<Value> = m
                        .content
                        .iter()
                        .filter_map(|b| match b {
                            ContentBlock::ToolUse { id, name, input } => Some(json!({
                                "id": id,
                                "type": "function",
                                "function": {
                                    "name": name,
                                    "arguments": input.to_string(),
                                }
                            })),
                            _ => None,
                        })
                        .collect();
                    let mut msg = json!({ "role": "assistant" });
                    msg["content"] = if text.is_empty() {
                        Value::Null
                    } else {
                        Value::String(text)
                    };
                    if !tool_calls.is_empty() {
                        msg["tool_calls"] = Value::Array(tool_calls);
                    }
                    out.push(msg);
                }
                Role::User => {
                    // tool_result 块 -> role:"tool"; 文本块 -> role:"user"
                    let mut user_texts: Vec<String> = Vec::new();
                    for b in &m.content {
                        match b {
                            ContentBlock::ToolResult {
                                tool_use_id,
                                content,
                                is_error,
                            } => {
                                let body = if *is_error {
                                    format!("[ERROR] {content}")
                                } else {
                                    content.clone()
                                };
                                out.push(json!({
                                    "role": "tool",
                                    "tool_call_id": tool_use_id,
                                    "content": body,
                                }));
                            }
                            ContentBlock::Text { text } => user_texts.push(text.clone()),
                            ContentBlock::ToolUse { .. } => {}
                        }
                    }
                    if !user_texts.is_empty() {
                        out.push(json!({ "role": "user", "content": user_texts.join("\n") }));
                    }
                }
            }
        }
        out
    }
}

#[derive(Default)]
struct PartialCall {
    id: String,
    name: String,
    arguments: String,
}

#[async_trait]
impl LlmProvider for OpenAiCompatProvider {
    async fn chat(
        &self,
        req: &ChatRequest,
        on_event: DeltaSink<'_>,
    ) -> Result<LlmResponse, LlmError> {
        let url = format!("{}/chat/completions", self.base_url);
        let tools: Vec<Value> = req
            .tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.input_schema,
                    }
                })
            })
            .collect();
        let mut body = json!({
            "model": req.model,
            "messages": Self::convert_messages(&req.system, &req.messages),
            "max_tokens": req.max_tokens,
            "stream": true,
        });
        if !tools.is_empty() {
            body["tools"] = Value::Array(tools);
        }
        // 思考模式：Qwen/DashScope 用 enable_thinking + thinking_budget；
        // 同时带上通用 reasoning_effort (OpenAI o 系列)。关闭时不加任何字段以兼容所有 API。
        if req.thinking.enabled() {
            body["enable_thinking"] = Value::Bool(true);
            if let Some(budget) = req.thinking.budget_tokens() {
                body["thinking_budget"] = Value::from(budget);
            }
            if let Some(effort) = req.thinking.reasoning_effort() {
                body["reasoning_effort"] = Value::from(effort);
            }
        }

        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| LlmError::Network(e.to_string()))?;

        let status = resp.status().as_u16();
        if status >= 400 {
            let retry_after = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .map(|s| s * 1000);
            let text = resp.text().await.unwrap_or_default();
            let lower = text.to_lowercase();
            return Err(match status {
                429 => LlmError::RateLimited {
                    retry_after_ms: retry_after,
                },
                401 | 403 => LlmError::Auth(text),
                _ if status >= 500 && lower.contains("overload") => LlmError::Overloaded,
                400 if lower.contains("context_length")
                    || lower.contains("context length")
                    || lower.contains("maximum context")
                    || lower.contains("too long")
                    || lower.contains("input length") =>
                {
                    LlmError::PromptTooLong
                }
                _ => LlmError::Api {
                    status,
                    message: text,
                },
            });
        }

        let mut text_buf = String::new();
        let mut calls: Vec<PartialCall> = Vec::new();
        let mut finish: Option<String> = None;
        let mut input_tokens = None;
        let mut output_tokens = None;

        for_each_sse_data(resp, |data| {
            let v: Value = match serde_json::from_str(data) {
                Ok(v) => v,
                Err(_) => return Ok(()),
            };
            if let Some(u) = v.get("usage").filter(|u| !u.is_null()) {
                input_tokens = u["prompt_tokens"].as_u64().or(input_tokens);
                output_tokens = u["completion_tokens"].as_u64().or(output_tokens);
            }
            let Some(choice) = v["choices"].as_array().and_then(|c| c.first()) else {
                return Ok(());
            };
            if let Some(fr) = choice["finish_reason"].as_str() {
                finish = Some(fr.to_string());
            }
            let delta = &choice["delta"];
            if let Some(t) = delta["content"].as_str() {
                if !t.is_empty() {
                    text_buf.push_str(t);
                    on_event(StreamEvent::TextDelta {
                        text: t.to_string(),
                    });
                }
            }
            if let Some(tcs) = delta["tool_calls"].as_array() {
                for tc in tcs {
                    let idx = tc["index"].as_u64().unwrap_or(0) as usize;
                    while calls.len() <= idx {
                        calls.push(PartialCall::default());
                    }
                    let slot = &mut calls[idx];
                    if let Some(id) = tc["id"].as_str() {
                        if !id.is_empty() {
                            slot.id = id.to_string();
                        }
                    }
                    if let Some(n) = tc["function"]["name"].as_str() {
                        if !n.is_empty() && slot.name.is_empty() {
                            slot.name = n.to_string();
                            on_event(StreamEvent::ToolUseStart {
                                name: n.to_string(),
                            });
                        }
                    }
                    if let Some(a) = tc["function"]["arguments"].as_str() {
                        slot.arguments.push_str(a);
                    }
                }
            }
            Ok(())
        })
        .await?;

        let mut content: Vec<ContentBlock> = Vec::new();
        if !text_buf.is_empty() {
            content.push(ContentBlock::Text { text: text_buf });
        }
        for (i, c) in calls.into_iter().enumerate() {
            if c.name.is_empty() {
                continue;
            }
            let input: Value = if c.arguments.trim().is_empty() {
                json!({})
            } else {
                serde_json::from_str(&c.arguments).unwrap_or(json!({}))
            };
            let id = if c.id.is_empty() {
                format!("call_{i}_{}", uuid::Uuid::new_v4().simple())
            } else {
                c.id
            };
            content.push(ContentBlock::ToolUse {
                id,
                name: c.name,
                input,
            });
        }

        let has_tool_use = content
            .iter()
            .any(|b| matches!(b, ContentBlock::ToolUse { .. }));
        let stop_reason = match finish.as_deref() {
            Some("length") => StopReason::MaxTokens,
            Some("tool_calls") => StopReason::ToolUse,
            _ if has_tool_use => StopReason::ToolUse,
            _ => StopReason::EndTurn,
        };

        on_event(StreamEvent::Done);
        Ok(LlmResponse {
            content,
            stop_reason,
            input_tokens,
            output_tokens,
        })
    }
}
