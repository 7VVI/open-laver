//! 工具抽象 — Tool trait + 分发注册表 (查表替代硬编码)

use async_trait::async_trait;
use serde_json::Value;

use crate::agent::ctx::ToolCtx;
use crate::llm::types::ToolSpec;

/// 工具执行结果
pub struct ToolOutput {
    pub content: String,
    pub is_error: bool,
}

impl ToolOutput {
    pub fn ok(s: impl Into<String>) -> Self {
        Self {
            content: s.into(),
            is_error: false,
        }
    }
    pub fn err(s: impl Into<String>) -> Self {
        Self {
            content: s.into(),
            is_error: true,
        }
    }
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    /// JSON schema (input_schema)
    fn input_schema(&self) -> Value;
    /// 是否并发安全 (只读工具通常安全) — 供未来并行执行参考
    fn concurrency_safe(&self) -> bool {
        false
    }
    async fn run(&self, input: &Value, ctx: &ToolCtx) -> ToolOutput;

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: self.name().to_string(),
            description: self.description().to_string(),
            input_schema: self.input_schema(),
        }
    }
}
