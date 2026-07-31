//! LlmProvider trait 与提供商工厂 (悟空 infrastructure/shared/llm 对应物)

use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::anthropic::AnthropicProvider;
use super::openai_compat::OpenAiCompatProvider;
use super::types::{ChatRequest, LlmError, LlmResponse, StreamEvent, ThinkingLevel};

/// 流式增量回调
pub type DeltaSink<'a> = &'a (dyn Fn(StreamEvent) + Send + Sync);

#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// 流式调用；增量经 on_event 回调，完整响应聚合后返回
    async fn chat(&self, req: &ChatRequest, on_event: DeltaSink<'_>)
        -> Result<LlmResponse, LlmError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Anthropic,
    OpenaiCompat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSettings {
    pub kind: ProviderKind,
    pub base_url: String,
    pub model: String,
    pub fallback_model: Option<String>,
    pub context_window: usize,
    #[serde(default)]
    pub thinking: ThinkingLevel,
}

impl Default for ProviderSettings {
    fn default() -> Self {
        Self {
            kind: ProviderKind::OpenaiCompat,
            base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1".into(),
            model: "qwen-max".into(),
            fallback_model: Some("qwen-plus".into()),
            context_window: crate::constants::DEFAULT_CONTEXT_WINDOW,
            thinking: ThinkingLevel::Off,
        }
    }
}

/// 按设置构造提供商实例
pub fn make_provider(settings: &ProviderSettings, api_key: String) -> Arc<dyn LlmProvider> {
    match settings.kind {
        ProviderKind::Anthropic => Arc::new(AnthropicProvider::new(
            settings.base_url.clone(),
            api_key,
        )),
        ProviderKind::OpenaiCompat => Arc::new(OpenAiCompatProvider::new(
            settings.base_url.clone(),
            api_key,
        )),
    }
}
