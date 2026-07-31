//! 编排工具集: s12 任务图 / s14 cron / s15-17 团队 / s18 worktree

use async_trait::async_trait;
use serde_json::{json, Value};

use super::tool::{Tool, ToolOutput};
use crate::agent::ctx::ToolCtx;
use crate::cron::scheduler::CronJob;
use crate::gateway::events::{emit, EV_TASK_UPDATE, EV_TEAM_MESSAGE};
use crate::team::mailbox::{MailBus, MailMessage};

// ============ s12 任务图 ============

pub struct TaskCreateTool;
#[async_trait]
impl Tool for TaskCreateTool {
    fn name(&self) -> &str { "task_create" }
    fn description(&self) -> &str {
        "在任务图中创建一个可持久化、可被团队认领的任务，可声明依赖 (blocked_by 任务 id 列表)。"
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "subject": { "type": "string", "description": "任务标题" },
                "description": { "type": "string", "description": "任务详情" },
                "blocked_by": { "type": "array", "items": { "type": "string" }, "description": "依赖的任务 id" }
            },
            "required": ["subject"]
        })
    }
    async fn run(&self, input: &Value, ctx: &ToolCtx) -> ToolOutput {
        let subject = input["subject"].as_str().unwrap_or("");
        let desc = input["description"].as_str().unwrap_or("");
        let blocked_by: Vec<String> = input["blocked_by"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        match ctx.state.tasks.create(subject, desc, blocked_by) {
            Ok(t) => {
                emit(&ctx.app, EV_TASK_UPDATE, ctx.state.tasks.all());
                ToolOutput::ok(format!("已创建任务 {} — {}", t.id, t.subject))
            }
            Err(e) => ToolOutput::err(e),
        }
    }
}

pub struct TaskListTool;
#[async_trait]
impl Tool for TaskListTool {
    fn name(&self) -> &str { "task_list" }
    fn description(&self) -> &str { "列出任务图中的所有任务及其状态、owner、依赖。" }
    fn concurrency_safe(&self) -> bool { true }
    fn input_schema(&self) -> Value { json!({ "type": "object", "properties": {} }) }
    async fn run(&self, _input: &Value, ctx: &ToolCtx) -> ToolOutput {
        let tasks = ctx.state.tasks.all();
        if tasks.is_empty() {
            return ToolOutput::ok("(任务图为空)".to_string());
        }
        let lines: Vec<String> = tasks
            .iter()
            .map(|t| {
                format!(
                    "{} [{}] {}{}{}",
                    t.id,
                    t.status,
                    t.subject,
                    t.owner.as_ref().map(|o| format!(" @{o}")).unwrap_or_default(),
                    if t.blocked_by.is_empty() {
                        String::new()
                    } else {
                        format!(" (依赖: {})", t.blocked_by.join(","))
                    }
                )
            })
            .collect();
        ToolOutput::ok(lines.join("\n"))
    }
}

pub struct TaskClaimTool;
#[async_trait]
impl Tool for TaskClaimTool {
    fn name(&self) -> &str { "task_claim" }
    fn description(&self) -> &str { "认领一个 pending 且依赖已满足的任务，认领后状态转为 in_progress。" }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": { "id": { "type": "string" } },
            "required": ["id"]
        })
    }
    async fn run(&self, input: &Value, ctx: &ToolCtx) -> ToolOutput {
        let id = input["id"].as_str().unwrap_or("");
        match ctx.state.tasks.claim(id, &ctx.agent_name) {
            Ok(t) => {
                emit(&ctx.app, EV_TASK_UPDATE, ctx.state.tasks.all());
                ToolOutput::ok(format!("已认领 {} — {}", t.id, t.subject))
            }
            Err(e) => ToolOutput::err(e),
        }
    }
}

pub struct TaskCompleteTool;
#[async_trait]
impl Tool for TaskCompleteTool {
    fn name(&self) -> &str { "task_complete" }
    fn description(&self) -> &str { "标记任务为已完成，并返回因此解锁的下游任务。" }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": { "id": { "type": "string" } },
            "required": ["id"]
        })
    }
    async fn run(&self, input: &Value, ctx: &ToolCtx) -> ToolOutput {
        let id = input["id"].as_str().unwrap_or("");
        match ctx.state.tasks.complete(id) {
            Ok(unlocked) => {
                emit(&ctx.app, EV_TASK_UPDATE, ctx.state.tasks.all());
                let extra = if unlocked.is_empty() {
                    String::new()
                } else {
                    format!(
                        "\n解锁了下游任务: {}",
                        unlocked.iter().map(|t| t.id.clone()).collect::<Vec<_>>().join(", ")
                    )
                };
                ToolOutput::ok(format!("任务 {id} 已完成{extra}"))
            }
            Err(e) => ToolOutput::err(e),
        }
    }
}

// ============ s14 cron ============

pub struct CronScheduleTool;
#[async_trait]
impl Tool for CronScheduleTool {
    fn name(&self) -> &str { "cron_schedule" }
    fn description(&self) -> &str {
        "创建定时任务。到点时会自动向本会话注入 prompt 让你执行。cron 为五段式 (分 时 日 月 星期)。"
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "cron": { "type": "string", "description": "五段式 cron 表达式, 如 '0 9 * * 1' 表示每周一 9 点" },
                "prompt": { "type": "string", "description": "到点时要执行的指令" },
                "recurring": { "type": "boolean", "description": "是否重复 (默认 true)" },
                "durable": { "type": "boolean", "description": "是否持久化跨重启 (默认 true)" }
            },
            "required": ["cron", "prompt"]
        })
    }
    async fn run(&self, input: &Value, ctx: &ToolCtx) -> ToolOutput {
        let expr = input["cron"].as_str().unwrap_or("");
        let prompt = input["prompt"].as_str().unwrap_or("");
        let recurring = input["recurring"].as_bool().unwrap_or(true);
        let durable = input["durable"].as_bool().unwrap_or(true);
        let id = format!("cron-{}", uuid::Uuid::new_v4().simple());
        let title: String = prompt.lines().next().unwrap_or("定时任务").chars().take(40).collect();
        let job = CronJob {
            id: id.clone(),
            title: title.clone(),
            expr: expr.to_string(),
            prompt: prompt.to_string(),
            recurring,
            durable,
            session_id: ctx.session_id.clone(),
        };
        if let Err(e) = ctx.state.cron.add(job) {
            return ToolOutput::err(e);
        }
        if durable {
            let _ = ctx.state.db.save_cron_job(
                &id, &title, expr, prompt, recurring, durable, &ctx.session_id,
            );
        }
        ToolOutput::ok(format!("已创建定时任务 {id}: [{expr}] {prompt}"))
    }
}

pub struct CronListTool;
#[async_trait]
impl Tool for CronListTool {
    fn name(&self) -> &str { "cron_list" }
    fn description(&self) -> &str { "列出所有定时任务。" }
    fn concurrency_safe(&self) -> bool { true }
    fn input_schema(&self) -> Value { json!({ "type": "object", "properties": {} }) }
    async fn run(&self, _input: &Value, ctx: &ToolCtx) -> ToolOutput {
        let jobs = ctx.state.cron.list();
        if jobs.is_empty() {
            return ToolOutput::ok("(无定时任务)".to_string());
        }
        let lines: Vec<String> = jobs
            .iter()
            .map(|j| format!("{} [{}] {} (recurring={})", j.id, j.expr, j.prompt, j.recurring))
            .collect();
        ToolOutput::ok(lines.join("\n"))
    }
}

pub struct CronCancelTool;
#[async_trait]
impl Tool for CronCancelTool {
    fn name(&self) -> &str { "cron_cancel" }
    fn description(&self) -> &str { "取消一个定时任务。" }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": { "id": { "type": "string" } },
            "required": ["id"]
        })
    }
    async fn run(&self, input: &Value, ctx: &ToolCtx) -> ToolOutput {
        let id = input["id"].as_str().unwrap_or("");
        ctx.state.cron.remove(id);
        let _ = ctx.state.db.delete_cron_job(id);
        ToolOutput::ok(format!("已取消定时任务 {id}"))
    }
}

// ============ s15/s16 团队通信 ============

pub struct SendMessageTool;
#[async_trait]
impl Tool for SendMessageTool {
    fn name(&self) -> &str { "send_message" }
    fn description(&self) -> &str {
        "向团队中的另一个成员 (队友或 lead) 发送消息。用于协作、汇报进度、请求审批。"
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "to": { "type": "string", "description": "收件人名称 (lead 或队友名)" },
                "content": { "type": "string", "description": "消息内容" },
                "type": { "type": "string", "description": "消息类型: text/summary/plan_approval 等", "default": "text" }
            },
            "required": ["to", "content"]
        })
    }
    async fn run(&self, input: &Value, ctx: &ToolCtx) -> ToolOutput {
        let to = input["to"].as_str().unwrap_or("");
        let content = input["content"].as_str().unwrap_or("");
        let mtype = input["type"].as_str().unwrap_or("text");
        let msg = MailMessage {
            from: ctx.agent_name.clone(),
            to: to.to_string(),
            content: content.to_string(),
            mtype: mtype.to_string(),
            ts: MailBus::now_ts(),
            request_id: input["request_id"].as_str().map(String::from),
        };
        ctx.state.mailbox.send(&msg);
        emit(&ctx.app, EV_TEAM_MESSAGE, msg);
        ToolOutput::ok(format!("已发送消息给 {to}"))
    }
}

// ============ s18 worktree ============

pub struct WorktreeCreateTool;
#[async_trait]
impl Tool for WorktreeCreateTool {
    fn name(&self) -> &str { "create_worktree" }
    fn description(&self) -> &str {
        "为并行任务创建隔离的 git worktree (独立目录+分支)。名称仅限字母数字与 ._- 。"
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": { "name": { "type": "string" } },
            "required": ["name"]
        })
    }
    async fn run(&self, input: &Value, ctx: &ToolCtx) -> ToolOutput {
        let name = input["name"].as_str().unwrap_or("");
        let ws = ctx.state.workspace().await;
        let mgr = crate::worktree::manager::WorktreeManager::new(ws);
        match mgr.create(name).await {
            Ok(p) => ToolOutput::ok(format!("已创建 worktree: {}", p.display())),
            Err(e) => ToolOutput::err(e),
        }
    }
}

pub struct WorktreeRemoveTool;
#[async_trait]
impl Tool for WorktreeRemoveTool {
    fn name(&self) -> &str { "remove_worktree" }
    fn description(&self) -> &str {
        "删除 git worktree。若存在未提交改动会被拒绝，除非设 discard_changes=true。"
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "discard_changes": { "type": "boolean" }
            },
            "required": ["name"]
        })
    }
    async fn run(&self, input: &Value, ctx: &ToolCtx) -> ToolOutput {
        let name = input["name"].as_str().unwrap_or("");
        let discard = input["discard_changes"].as_bool().unwrap_or(false);
        let ws = ctx.state.workspace().await;
        let mgr = crate::worktree::manager::WorktreeManager::new(ws);
        match mgr.remove(name, discard).await {
            Ok(m) => ToolOutput::ok(m),
            Err(e) => ToolOutput::err(e),
        }
    }
}
