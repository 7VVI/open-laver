//! s02 文件工具 — read_file / write_file / edit_file / glob，safe_path 限定工作区

use async_trait::async_trait;
use serde_json::{json, Value};

use super::tool::{Tool, ToolOutput};
use crate::agent::ctx::ToolCtx;

// ---------------- read_file ----------------

pub struct ReadFileTool;

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }
    fn description(&self) -> &str {
        "读取文本文件内容。支持相对工作区的路径或绝对路径。"
    }
    fn concurrency_safe(&self) -> bool {
        true
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "文件路径" }
            },
            "required": ["path"]
        })
    }
    async fn run(&self, input: &Value, ctx: &ToolCtx) -> ToolOutput {
        let path = input["path"].as_str().unwrap_or("");
        let (abs, _) = ctx.resolve_path(path);
        match tokio::fs::read(&abs).await {
            Ok(bytes) => {
                let text = String::from_utf8_lossy(&bytes);
                // 大文件截断保护
                let limited: String = if text.chars().count() > 60_000 {
                    let s: String = text.chars().take(60_000).collect();
                    format!("{s}\n... [文件过大已截断]")
                } else {
                    text.to_string()
                };
                ToolOutput::ok(limited)
            }
            Err(e) => ToolOutput::err(format!("读取失败 {}: {e}", abs.display())),
        }
    }
}

// ---------------- write_file ----------------

pub struct WriteFileTool;

#[async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }
    fn description(&self) -> &str {
        "创建或覆盖文件，写入给定内容。父目录不存在时自动创建。"
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "文件路径" },
                "content": { "type": "string", "description": "文件内容" }
            },
            "required": ["path", "content"]
        })
    }
    async fn run(&self, input: &Value, ctx: &ToolCtx) -> ToolOutput {
        let path = input["path"].as_str().unwrap_or("");
        let content = input["content"].as_str().unwrap_or("");
        let (abs, _) = ctx.resolve_path(path);
        if let Some(parent) = abs.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        match tokio::fs::write(&abs, content).await {
            Ok(_) => ToolOutput::ok(format!(
                "已写入 {} ({} 字节)",
                abs.display(),
                content.len()
            )),
            Err(e) => ToolOutput::err(format!("写入失败 {}: {e}", abs.display())),
        }
    }
}

// ---------------- edit_file ----------------

pub struct EditFileTool;

#[async_trait]
impl Tool for EditFileTool {
    fn name(&self) -> &str {
        "edit_file"
    }
    fn description(&self) -> &str {
        "在文件中把 old_string 精确替换为 new_string。old_string 必须唯一匹配。"
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "文件路径" },
                "old_string": { "type": "string", "description": "要替换的原文 (需唯一)" },
                "new_string": { "type": "string", "description": "替换后的新文本" }
            },
            "required": ["path", "old_string", "new_string"]
        })
    }
    async fn run(&self, input: &Value, ctx: &ToolCtx) -> ToolOutput {
        let path = input["path"].as_str().unwrap_or("");
        let old = input["old_string"].as_str().unwrap_or("");
        let new = input["new_string"].as_str().unwrap_or("");
        let (abs, _) = ctx.resolve_path(path);
        if old.is_empty() {
            return ToolOutput::err("old_string 不能为空");
        }
        let content = match tokio::fs::read_to_string(&abs).await {
            Ok(c) => c,
            Err(e) => return ToolOutput::err(format!("读取失败 {}: {e}", abs.display())),
        };
        let count = content.matches(old).count();
        if count == 0 {
            return ToolOutput::err("未找到 old_string，未做修改");
        }
        if count > 1 {
            return ToolOutput::err(format!("old_string 匹配到 {count} 处，需提供更唯一的上下文"));
        }
        let updated = content.replacen(old, new, 1);
        match tokio::fs::write(&abs, updated).await {
            Ok(_) => ToolOutput::ok(format!("已编辑 {}", abs.display())),
            Err(e) => ToolOutput::err(format!("写入失败: {e}")),
        }
    }
}

// ---------------- glob ----------------

pub struct GlobTool;

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }
    fn description(&self) -> &str {
        "按 glob 模式查找文件，例如 **/*.xlsx 或 reports/*.md。相对工作区解析。"
    }
    fn concurrency_safe(&self) -> bool {
        true
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string", "description": "glob 模式" }
            },
            "required": ["pattern"]
        })
    }
    async fn run(&self, input: &Value, ctx: &ToolCtx) -> ToolOutput {
        let pattern = input["pattern"].as_str().unwrap_or("");
        if pattern.is_empty() {
            return ToolOutput::err("pattern 不能为空");
        }
        // 相对模式挂到 cwd 下
        let full = if std::path::Path::new(pattern).is_absolute() {
            pattern.to_string()
        } else {
            ctx.cwd.join(pattern).to_string_lossy().to_string()
        };
        let mut matches = Vec::new();
        match glob::glob(&full) {
            Ok(paths) => {
                for p in paths.flatten() {
                    matches.push(p.display().to_string());
                    if matches.len() >= 500 {
                        break;
                    }
                }
            }
            Err(e) => return ToolOutput::err(format!("模式错误: {e}")),
        }
        if matches.is_empty() {
            ToolOutput::ok("(无匹配文件)".to_string())
        } else {
            ToolOutput::ok(matches.join("\n"))
        }
    }
}
