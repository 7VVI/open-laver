//! s02 shell 工具 — Windows 走 PowerShell，支持超时与输出截断
//! s13: run_in_background 参数交由后台管理器 (loop 层拦截，本工具执行前台命令)

use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};

use super::tool::{Tool, ToolOutput};
use crate::agent::ctx::ToolCtx;
use crate::constants::{SHELL_OUTPUT_LIMIT, SHELL_TIMEOUT_SECS};

pub struct ShellTool;

/// 执行一条 PowerShell 命令，返回 (合并输出, 是否成功)
pub async fn run_powershell(command: &str, cwd: &std::path::Path) -> (String, bool) {
    let mut cmd = tokio::process::Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", command])
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return (format!("启动进程失败: {e}"), false),
    };

    let out = match tokio::time::timeout(
        Duration::from_secs(SHELL_TIMEOUT_SECS),
        child.wait_with_output(),
    )
    .await
    {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => return (format!("进程错误: {e}"), false),
        Err(_) => {
            return (
                format!("命令超时 ({}s) 已终止", SHELL_TIMEOUT_SECS),
                false,
            )
        }
    };

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let mut combined = String::new();
    if !stdout.trim().is_empty() {
        combined.push_str(&stdout);
    }
    if !stderr.trim().is_empty() {
        if !combined.is_empty() {
            combined.push_str("\n[stderr]\n");
        }
        combined.push_str(&stderr);
    }
    let success = out.status.success();
    if !success {
        combined = format!(
            "[exit code: {}]\n{}",
            out.status.code().unwrap_or(-1),
            combined
        );
    }
    // 输出截断
    if combined.chars().count() > SHELL_OUTPUT_LIMIT {
        let truncated: String = combined.chars().take(SHELL_OUTPUT_LIMIT).collect();
        combined = format!(
            "{truncated}\n... [输出超过 {} 字符已截断]",
            SHELL_OUTPUT_LIMIT
        );
    }
    if combined.trim().is_empty() {
        combined = "(命令执行完成，无输出)".into();
    }
    (combined, success)
}

#[async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &str {
        "shell"
    }

    fn description(&self) -> &str {
        "在用户的 Windows 机器上执行一条 PowerShell 命令并返回输出。用于运行脚本、查询系统、处理文件等。设置 run_in_background=true 可将耗时命令放入后台执行。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "要执行的 PowerShell 命令"
                },
                "run_in_background": {
                    "type": "boolean",
                    "description": "是否放入后台异步执行 (适合安装依赖、长时间任务)"
                }
            },
            "required": ["command"]
        })
    }

    async fn run(&self, input: &Value, ctx: &ToolCtx) -> ToolOutput {
        let command = input["command"].as_str().unwrap_or("").trim();
        if command.is_empty() {
            return ToolOutput::err("command 不能为空");
        }
        let (output, ok) = run_powershell(command, &ctx.cwd).await;
        if ok {
            ToolOutput::ok(output)
        } else {
            ToolOutput::err(output)
        }
    }
}
