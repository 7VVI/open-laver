//! 全局常量 — 阈值与教程保持一致 (learn.shareai.run s01-s20)

/// s08 L3: 末条 user 消息 tool_result 总量预算 (字节)，超出则落盘
pub const TOOL_RESULT_BUDGET_BYTES: usize = 200_000;
/// s08 L3: 落盘后保留的预览字符数
pub const TOOL_RESULT_PREVIEW_CHARS: usize = 2000;
/// s08 L1: 消息条数超过该值触发 snip
pub const SNIP_THRESHOLD_MSGS: usize = 50;
/// s08 L1: snip 保留头部条数
pub const SNIP_KEEP_HEAD: usize = 3;
/// s08 L1: snip 保留尾部条数
pub const SNIP_KEEP_TAIL: usize = 47;
/// s08 L2: micro compact 保留最近 N 条 tool_result 全文
pub const KEEP_RECENT_TOOL_RESULTS: usize = 3;
/// s08 L4: compact_history 连续失败熔断次数
pub const COMPACT_FAILURE_FUSE: u32 = 3;
/// s08: 压缩阈值预留 token (contextWindow - maxOutput - RESERVE)
pub const COMPACT_RESERVE_TOKENS: usize = 13_000;
/// s08: reactive compact 保留尾部消息条数
pub const REACTIVE_KEEP_TAIL: usize = 5;

/// s11: 默认输出 token 上限
pub const MAX_OUTPUT_TOKENS: u32 = 8_000;
/// s11: 截断后升级的输出 token 上限 (8K -> 64K)
pub const MAX_OUTPUT_TOKENS_ESCALATED: u32 = 64_000;
/// s11: 续写重试上限
pub const MAX_CONTINUATION_ATTEMPTS: u32 = 3;
/// s11: 429/529 重试上限
pub const MAX_RETRY_ATTEMPTS: u32 = 10;
/// s11: 退避基数 (ms): min(500 * 2^n, 32000) + jitter
pub const BACKOFF_BASE_MS: u64 = 500;
pub const BACKOFF_CAP_MS: u64 = 32_000;
/// s11: 连续 529 达到该值切换备用模型
pub const CONSECUTIVE_529_FALLBACK: u32 = 3;

/// s06: 子代理循环轮数上限
pub const SUBAGENT_MAX_ROUNDS: u32 = 30;
/// s05: 连续 N 轮未调用 todo_write 注入提醒
pub const TODO_REMINDER_ROUNDS: u32 = 3;

/// s09: 记忆文件数达到该值触发 consolidate
pub const MEMORY_CONSOLIDATE_THRESHOLD: usize = 10;
/// s09: 每轮注入相关记忆上限
pub const MEMORY_INJECT_MAX: usize = 5;

/// s17: 队友 IDLE 轮询间隔 (秒)
pub const IDLE_POLL_SECS: u64 = 5;
/// s17: 队友 IDLE 超时 (秒)
pub const IDLE_TIMEOUT_SECS: u64 = 60;

/// shell 工具默认超时 (秒)
pub const SHELL_TIMEOUT_SECS: u64 = 120;
/// shell 工具输出截断 (字符)
pub const SHELL_OUTPUT_LIMIT: usize = 30_000;

/// s18: worktree 名称校验
pub const WORKTREE_NAME_RE: &str = r"^[A-Za-z0-9._-]{1,64}$";

/// s03: 权限弹窗等待超时 (秒)，超时按 deny 处理
pub const APPROVAL_TIMEOUT_SECS: u64 = 300;

/// 主循环安全上限 (防失控)
pub const MAIN_LOOP_MAX_ROUNDS: u32 = 100;

/// 默认上下文窗口 (token)，可在设置中按模型调整
pub const DEFAULT_CONTEXT_WINDOW: usize = 128_000;
