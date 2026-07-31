//! s03 权限引擎 — 三道闸门: DENY_LIST 硬拒绝 -> 规则匹配 -> 用户审批

use serde_json::Value;

use super::approval_memory::pattern_for;
use crate::agent::ctx::ToolCtx;

#[derive(Debug, Clone, PartialEq)]
pub enum Decision {
    Allow,
    Deny(String),
    /// 携带给用户看的说明
    Ask(String),
}

/// 闸门 1: 硬拒绝表 (Windows 风味) — 命中直接 deny，不进入审批
const DENY_LIST: &[&str] = &[
    "rm -rf /",
    "rm -rf c:",
    "format c:",
    "format d:",
    "shutdown /s",
    "shutdown -s",
    "remove-item -recurse -force c:\\",
    "remove-item -recurse -force c:/",
    "del /f /s /q c:\\",
    "rd /s /q c:\\",
    "reg delete hklm",
    "vssadmin delete shadows",
    "cipher /w",
    "diskpart",
    "bcdedit",
    "mkfs",
    ":(){ :|:& };:",
];

/// 闸门 2 规则: 危险但可经用户放行的命令特征
const ASK_PATTERNS: &[&str] = &[
    "remove-item",
    "del ",
    "rd /s",
    "rmdir",
    "rm -rf",
    "rm -r",
    "git push --force",
    "git push -f",
    "git reset --hard",
    "git clean -fd",
    "sudo ",
    "runas ",
    "taskkill",
    "stop-process",
    "stop-service",
    "sc delete",
    "netsh ",
    "set-executionpolicy",
    "curl ",
    "invoke-webrequest",
    "wget ",
    "iwr ",
    "> c:\\windows",
    "schtasks",
];

/// 敏感路径 (悟空 sensitive_paths.rs 对应物): 涉及则 ask
const SENSITIVE_PATH_HINTS: &[&str] = &[
    ".ssh",
    ".gnupg",
    "credentials",
    "id_rsa",
    ".env",
    "appdata\\local\\laver-agent",
];

/// 评估工具调用；不含审批记忆检查 (由 hook 层组合)
pub fn evaluate(tool: &str, input: &Value, ctx: &ToolCtx) -> Decision {
    match tool {
        "shell" => {
            let cmd = input["command"].as_str().unwrap_or("").to_lowercase();
            // 闸门 1
            for d in DENY_LIST {
                if cmd.contains(d) {
                    return Decision::Deny(format!("命中硬拒绝规则: {d}"));
                }
            }
            // 闸门 2 -> 转闸门 3
            for p in ASK_PATTERNS {
                if cmd.contains(p) {
                    return Decision::Ask(format!("命令包含危险特征 `{}`", p.trim()));
                }
            }
            for s in SENSITIVE_PATH_HINTS {
                if cmd.contains(s) {
                    return Decision::Ask(format!("命令涉及敏感路径 `{s}`"));
                }
            }
            Decision::Allow
        }
        "write_file" | "edit_file" => {
            let path = input["path"].as_str().unwrap_or("");
            let (abs, inside) = ctx.resolve_path(path);
            let lower = abs.to_string_lossy().to_lowercase();
            for s in SENSITIVE_PATH_HINTS {
                if lower.contains(s) {
                    return Decision::Ask(format!("写入敏感路径 `{s}`"));
                }
            }
            if !inside {
                return Decision::Ask(format!("在工作区之外写文件: {}", abs.display()));
            }
            Decision::Allow
        }
        "remove_worktree" => {
            // s18: 有未提交改动的删除由工具自身把关；此处统一 ask
            Decision::Ask("删除 git worktree".into())
        }
        _ if tool.starts_with("mcp__") => {
            // s19: 外部 MCP 工具默认询问 (描述带 destructive 标注的更应询问)
            Decision::Ask(format!("调用外部 MCP 工具 {tool}"))
        }
        _ => Decision::Allow,
    }
}

/// 生成审批记忆匹配 key (allow always 时记录)
pub fn approval_key(tool: &str, input: &Value) -> String {
    pattern_for(tool, input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deny_list_hits() {
        // 直接测试字符串匹配逻辑
        let cmd = "powershell rm -rf / --no-preserve-root".to_lowercase();
        assert!(DENY_LIST.iter().any(|d| cmd.contains(d)));
    }

    #[test]
    fn ask_pattern_hits() {
        let cmd = "git push --force origin main".to_lowercase();
        assert!(ASK_PATTERNS.iter().any(|p| cmd.contains(p)));
    }

    #[test]
    fn safe_command_passes() {
        let cmd = "ls -la".to_lowercase();
        assert!(!DENY_LIST.iter().any(|d| cmd.contains(d)));
        assert!(!ASK_PATTERNS.iter().any(|p| cmd.contains(p)));
    }
}
