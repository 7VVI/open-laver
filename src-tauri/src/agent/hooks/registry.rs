//! Hooks — 横切逻辑挂在循环周围，循环只调 trigger
//!
//! 四个触发点:
//! - UserPromptSubmit: 入 LLM 前
//! - PreToolUse: 工具执行前 (非 Continue 返回值阻止执行)
//! - PostToolUse: 执行后副作用
//! - Stop: 退出前 (返回消息则强制续跑)

use serde_json::Value;

use crate::agent::ctx::ToolCtx;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookEvent {
    UserPromptSubmit,
    PreToolUse,
    PostToolUse,
    Stop,
}

/// PreToolUse 结果
#[derive(Debug, Clone)]
pub enum PreToolOutcome {
    /// 放行
    Continue,
    /// 阻止执行，以此文本作为 tool_result 回填 (is_error=true)
    Block(String),
}

/// Stop 结果
#[derive(Debug, Clone)]
pub enum StopOutcome {
    /// 允许停止
    Allow,
    /// 强制续跑，注入该 user 消息
    Continue(String),
}

/// UserPromptSubmit 结果: 可改写/追加提示上下文
#[derive(Debug, Clone, Default)]
pub struct PromptContext {
    /// 追加到用户输入前的附加上下文块
    pub prepend: Vec<String>,
}

/// Hook 注册表 — 各章把自己的横切逻辑注册进来
pub struct HookRegistry;

impl HookRegistry {
    pub fn new() -> Self {
        Self
    }

    /// UserPromptSubmit: 目前用于注入 todo 提醒等 (由 loop 组合)
    pub fn on_user_prompt(&self, _prompt: &str) -> PromptContext {
        PromptContext::default()
    }

    /// PostToolUse: 记录审计 (示例，可扩展)
    pub fn on_post_tool(&self, event: HookEvent, tool: &str, _ok: bool) {
        debug_assert_eq!(event, HookEvent::PostToolUse);
        tracing::debug!(tool, "post tool use hook");
    }
}

impl Default for HookRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// PreToolUse 中权限引擎的组合判定结果的载体 (供 loop 调用权限逻辑，见 loop_engine)
pub struct PreToolInput<'a> {
    pub tool: &'a str,
    pub input: &'a Value,
    pub ctx: &'a ToolCtx,
}
