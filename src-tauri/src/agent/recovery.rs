//! 错误恢复 — 按错误类型分三条恢复路径

use std::time::Duration;

use rand::Rng;

use crate::constants::*;
use crate::llm::types::LlmError;

/// 恢复状态 (跨重试保留)
#[derive(Debug, Clone, Default)]
pub struct RecoveryState {
    pub has_escalated: bool,
    pub reactive_compact_used: bool,
    pub recovery_count: u32,
    pub consecutive_529: u32,
    pub retry_attempts: u32,
    pub current_model: Option<String>,
}

/// 恢复决策
#[derive(Debug, Clone)]
pub enum RecoveryAction {
    /// 升级 max_tokens 到 64K 后重试 (截断首次)
    EscalateMaxTokens,
    /// 已保存部分输出，注入续写提示后重试
    ContinueTruncated,
    /// 触发 reactive_compact 后重试
    ReactiveCompact,
    /// 退避等待后重试 (可能切换模型)
    Backoff {
        wait: Duration,
        switch_to_fallback: bool,
    },
    /// 放弃，返回错误信息
    GiveUp(String),
}

impl RecoveryState {
    /// max_tokens 截断处理
    pub fn on_max_tokens(&mut self) -> RecoveryAction {
        if !self.has_escalated {
            self.has_escalated = true;
            return RecoveryAction::EscalateMaxTokens;
        }
        if self.recovery_count < MAX_CONTINUATION_ATTEMPTS {
            self.recovery_count += 1;
            return RecoveryAction::ContinueTruncated;
        }
        RecoveryAction::GiveUp(format!(
            "输出多次截断 ({} 次续写后仍未完成)",
            MAX_CONTINUATION_ATTEMPTS
        ))
    }

    /// prompt_too_long 处理 (仅一次机会)
    pub fn on_prompt_too_long(&mut self) -> RecoveryAction {
        if !self.reactive_compact_used {
            self.reactive_compact_used = true;
            RecoveryAction::ReactiveCompact
        } else {
            RecoveryAction::GiveUp("上下文仍然超限 (reactive compact 已用尽)".into())
        }
    }

    /// 429/529 指数退避
    pub fn on_transient(&mut self, err: &LlmError) -> RecoveryAction {
        self.retry_attempts += 1;
        if self.retry_attempts > MAX_RETRY_ATTEMPTS {
            return RecoveryAction::GiveUp(format!("重试 {MAX_RETRY_ATTEMPTS} 次仍失败: {err}"));
        }

        let is_529 = matches!(err, LlmError::Overloaded);
        if is_529 {
            self.consecutive_529 += 1;
        } else {
            self.consecutive_529 = 0;
        }
        let switch = is_529 && self.consecutive_529 >= CONSECUTIVE_529_FALLBACK;

        // Retry-After 优先
        let base = if let LlmError::RateLimited {
            retry_after_ms: Some(ms),
        } = err
        {
            *ms
        } else {
            let n = self.retry_attempts.saturating_sub(1);
            (BACKOFF_BASE_MS.saturating_mul(1u64 << n.min(20))).min(BACKOFF_CAP_MS)
        };
        // 抖动 0~25%
        let jitter = rand::thread_rng().gen_range(0..=(base / 4).max(1));
        RecoveryAction::Backoff {
            wait: Duration::from_millis(base + jitter),
            switch_to_fallback: switch,
        }
    }

    /// 成功后重置瞬时计数 (保留 escalation/compact 标记直到本轮结束)
    pub fn on_success(&mut self) {
        self.retry_attempts = 0;
        self.consecutive_529 = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_tokens_escalates_then_continues_then_gives_up() {
        let mut s = RecoveryState::default();
        assert!(matches!(s.on_max_tokens(), RecoveryAction::EscalateMaxTokens));
        for _ in 0..MAX_CONTINUATION_ATTEMPTS {
            assert!(matches!(
                s.on_max_tokens(),
                RecoveryAction::ContinueTruncated
            ));
        }
        assert!(matches!(s.on_max_tokens(), RecoveryAction::GiveUp(_)));
    }

    #[test]
    fn prompt_too_long_once() {
        let mut s = RecoveryState::default();
        assert!(matches!(
            s.on_prompt_too_long(),
            RecoveryAction::ReactiveCompact
        ));
        assert!(matches!(s.on_prompt_too_long(), RecoveryAction::GiveUp(_)));
    }

    #[test]
    fn consecutive_529_switches_model() {
        let mut s = RecoveryState::default();
        let mut switched = false;
        for _ in 0..CONSECUTIVE_529_FALLBACK {
            if let RecoveryAction::Backoff {
                switch_to_fallback, ..
            } = s.on_transient(&LlmError::Overloaded)
            {
                switched = switch_to_fallback;
            }
        }
        assert!(switched);
    }
}
