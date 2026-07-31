//! s05 TodoWrite 工具 + s07 load_skill + s08 compact 触发 + 记忆写入

use async_trait::async_trait;
use serde_json::{json, Value};

use super::tool::{Tool, ToolOutput};
use crate::agent::ctx::ToolCtx;
use crate::agent::session::TodoItem;
use crate::gateway::events::{emit, EV_TODO_UPDATE};

// ---------------- s05 todo_write ----------------

pub struct TodoWriteTool;

#[derive(serde::Serialize, Clone)]
struct TodoUpdatePayload {
    session_id: String,
    todos: Vec<TodoItem>,
}

#[async_trait]
impl Tool for TodoWriteTool {
    fn name(&self) -> &str {
        "todo_write"
    }
    fn description(&self) -> &str {
        "维护当前任务的待办清单。开始多步骤任务时先列出全部步骤，随进展更新每项状态 (pending/in_progress/completed)。保持只有一项 in_progress。"
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "todos": {
                    "type": "array",
                    "description": "完整的待办清单 (每次传全量)",
                    "items": {
                        "type": "object",
                        "properties": {
                            "content": { "type": "string" },
                            "status": {
                                "type": "string",
                                "enum": ["pending", "in_progress", "completed"]
                            }
                        },
                        "required": ["content", "status"]
                    }
                }
            },
            "required": ["todos"]
        })
    }
    async fn run(&self, input: &Value, ctx: &ToolCtx) -> ToolOutput {
        let items: Vec<TodoItem> = match input.get("todos").and_then(|v| v.as_array()) {
            Some(arr) => arr
                .iter()
                .filter_map(|t| {
                    Some(TodoItem {
                        content: t["content"].as_str()?.to_string(),
                        status: t["status"].as_str().unwrap_or("pending").to_string(),
                    })
                })
                .collect(),
            None => return ToolOutput::err("todos 必须是数组"),
        };

        // 写入会话状态 + 重置提醒计数
        if let Some(session) = ctx.state.sessions.get(&ctx.session_id) {
            let mut st = session.state.lock().await;
            st.todos = items.clone();
            st.rounds_since_todo = 0;
        }
        emit(
            &ctx.app,
            EV_TODO_UPDATE,
            TodoUpdatePayload {
                session_id: ctx.session_id.clone(),
                todos: items.clone(),
            },
        );

        let summary = items
            .iter()
            .map(|t| {
                let mark = match t.status.as_str() {
                    "completed" => "[x]",
                    "in_progress" => "[~]",
                    _ => "[ ]",
                };
                format!("{mark} {}", t.content)
            })
            .collect::<Vec<_>>()
            .join("\n");
        ToolOutput::ok(format!("待办已更新:\n{summary}"))
    }
}

// ---------------- s07 load_skill ----------------

pub struct LoadSkillTool;

#[async_trait]
impl Tool for LoadSkillTool {
    fn name(&self) -> &str {
        "load_skill"
    }
    fn description(&self) -> &str {
        "加载指定技能的完整操作说明。当任务匹配某个技能目录中的能力时调用，加载后按其步骤执行。"
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "description": "技能名称" }
            },
            "required": ["name"]
        })
    }
    async fn run(&self, input: &Value, ctx: &ToolCtx) -> ToolOutput {
        let name = input["name"].as_str().unwrap_or("");
        match ctx.state.skills.load(name) {
            Some(content) => ToolOutput::ok(content),
            None => ToolOutput::err(format!("未找到技能 '{name}' 或该技能未启用")),
        }
    }
}

// ---------------- s09 remember (主动写记忆) ----------------

pub struct RememberTool;

#[async_trait]
impl Tool for RememberTool {
    fn name(&self) -> &str {
        "remember"
    }
    fn description(&self) -> &str {
        "将一条需要跨会话长期记住的事实存入记忆 (用户偏好、项目约定、稳定信息)。"
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "description": "简短标识名" },
                "description": { "type": "string", "description": "一句话说明" },
                "type": { "type": "string", "enum": ["user", "feedback", "project", "reference"] },
                "content": { "type": "string", "description": "记忆正文" }
            },
            "required": ["name", "content"]
        })
    }
    async fn run(&self, input: &Value, ctx: &ToolCtx) -> ToolOutput {
        let item = crate::agent::memory::service::MemoryItem {
            name: input["name"].as_str().unwrap_or("").to_string(),
            description: input["description"].as_str().unwrap_or("").to_string(),
            mtype: input["type"].as_str().unwrap_or("reference").to_string(),
            content: input["content"].as_str().unwrap_or("").to_string(),
        };
        if item.name.is_empty() {
            return ToolOutput::err("name 不能为空");
        }
        match ctx.state.memory.upsert(&item) {
            Ok(_) => ToolOutput::ok(format!("已记住: {}", item.name)),
            Err(e) => ToolOutput::err(e),
        }
    }
}
