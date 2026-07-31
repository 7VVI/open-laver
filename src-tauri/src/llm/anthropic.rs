//! Anthropic Messages API 提供商 (原生 tool_use 协议, SSE 流式)

use async_trait::async_trait;
use serde_json::{json, Value};

use super::provider::{DeltaSink, LlmProvider};
use super::sse::for_each_sse_data;
use super::types::{
    ChatRequest, ContentBlock, LlmError, LlmResponse, StopReason, StreamEvent,
};

pub struct AnthropicProvider {
    base_url: String,
    api_key: String,
    client: reqwest::Client,
}

impl AnthropicProvider {
    pub fn new(base_url: String, api_key: String) -> Self {
        let base = if base_url.trim().is_empty() {
            "https://api.anthropic.com".to_string()
        } else {
            base_url.trim_end_matches('/').to_string()
        };
        Self {
            base_url: base,
            api_key,
            client: reqwest::Client::new(),
        }
    }

    fn body(req: &ChatRequest) -> Value {
        let tools: Vec<Value> = req
            .tools
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.input_schema,
                })
            })
            .collect();
        let mut body = json!({
            "model": req.model,
            "system": req.system,
            "messages": req.messages,
            "max_tokens": req.max_tokens,
            "stream": true,
        });
        if !tools.is_empty() {
            body["tools"] = Value::Array(tools);
        }
        // 思考模式 (extended thinking)
        if let Some(budget) = req.thinking.budget_tokens() {
            body["thinking"] = json!({ "type": "enabled", "budget_tokens": budget });
        }
        body
    }
}

/// 流式聚合中的部分内容块
enum PartialBlock {
    Text(String),
    ToolUse {
        id: String,
        name: String,
        json_buf: String,
    },
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    async fn chat(
        &self,
        req: &ChatRequest,
        on_event: DeltaSink<'_>,
    ) -> Result<LlmResponse, LlmError> {
        let url = format!("{}/v1/messages", self.base_url);
        let resp = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&Self::body(req))
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
                529 => LlmError::Overloaded,
                401 | 403 => LlmError::Auth(text),
                400 if lower.contains("prompt is too long")
                    || lower.contains("too many tokens") =>
                {
                    LlmError::PromptTooLong
                }
                _ if lower.contains("overloaded_error") => LlmError::Overloaded,
                _ => LlmError::Api {
                    status,
                    message: text,
                },
            });
        }

        let mut blocks: Vec<PartialBlock> = Vec::new();
        let mut stop_reason = StopReason::EndTurn;
        let mut input_tokens = None;
        let mut output_tokens = None;
        let mut stream_err: Option<LlmError> = None;

        for_each_sse_data(resp, |data| {
            let v: Value = match serde_json::from_str(data) {
                Ok(v) => v,
                Err(_) => return Ok(()),
            };
            match v["type"].as_str().unwrap_or("") {
                "message_start" => {
                    input_tokens = v["message"]["usage"]["input_tokens"].as_u64();
                }
                "content_block_start" => {
                    let cb = &v["content_block"];
                    match cb["type"].as_str().unwrap_or("") {
                        "tool_use" => {
                            let name = cb["name"].as_str().unwrap_or("").to_string();
                            on_event(StreamEvent::ToolUseStart { name: name.clone() });
                            blocks.push(PartialBlock::ToolUse {
                                id: cb["id"].as_str().unwrap_or("").to_string(),
                                name,
                                json_buf: String::new(),
                            });
                        }
                        _ => blocks.push(PartialBlock::Text(
                            cb["text"].as_str().unwrap_or("").to_string(),
                        )),
                    }
                }
                "content_block_delta" => {
                    let idx = v["index"].as_u64().unwrap_or(0) as usize;
                    let delta = &v["delta"];
                    if let Some(b) = blocks.get_mut(idx) {
                        match delta["type"].as_str().unwrap_or("") {
                            "text_delta" => {
                                let t = delta["text"].as_str().unwrap_or("");
                                if let PartialBlock::Text(s) = b {
                                    s.push_str(t);
                                }
                                on_event(StreamEvent::TextDelta {
                                    text: t.to_string(),
                                });
                            }
                            "input_json_delta" => {
                                if let PartialBlock::ToolUse { json_buf, .. } = b {
                                    json_buf
                                        .push_str(delta["partial_json"].as_str().unwrap_or(""));
                                }
                            }
                            _ => {}
                        }
                    }
                }
                "message_delta" => {
                    output_tokens = v["usage"]["output_tokens"].as_u64().or(output_tokens);
                    match v["delta"]["stop_reason"].as_str().unwrap_or("") {
                        "tool_use" => stop_reason = StopReason::ToolUse,
                        "max_tokens" => stop_reason = StopReason::MaxTokens,
                        "" => {}
                        _ => stop_reason = StopReason::EndTurn,
                    }
                }
                "error" => {
                    let etype = v["error"]["type"].as_str().unwrap_or("");
                    let msg = v["error"]["message"].as_str().unwrap_or("").to_string();
                    stream_err = Some(match etype {
                        "overloaded_error" => LlmError::Overloaded,
                        "rate_limit_error" => LlmError::RateLimited {
                            retry_after_ms: None,
                        },
                        _ => LlmError::Api {
                            status: 0,
                            message: msg,
                        },
                    });
                }
                _ => {}
            }
            Ok(())
        })
        .await?;

        if let Some(e) = stream_err {
            return Err(e);
        }

        let content: Vec<ContentBlock> = blocks
            .into_iter()
            .filter_map(|b| match b {
                PartialBlock::Text(s) => {
                    if s.is_empty() {
                        None
                    } else {
                        Some(ContentBlock::Text { text: s })
                    }
                }
                PartialBlock::ToolUse { id, name, json_buf } => {
                    let input: Value = if json_buf.trim().is_empty() {
                        json!({})
                    } else {
                        serde_json::from_str(&json_buf).unwrap_or(json!({}))
                    };
                    Some(ContentBlock::ToolUse { id, name, input })
                }
            })
            .collect();

        on_event(StreamEvent::Done);
        Ok(LlmResponse {
            content,
            stop_reason,
            input_tokens,
            output_tokens,
        })
    }
}
