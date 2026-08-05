//! task (声明) + spawn_teammate / shutdown_teammate 工具
//! task 的实际执行在 loop_engine::execute_tool 中拦截 (需要 spawn 子循环)

use async_trait::async_trait;
use serde_json::{json, Value};

use super::tool::{Tool, ToolOutput};
use crate::agent::ctx::ToolCtx;

// task 工具仅提供声明；执行被 loop_engine 拦截
pub struct TaskTool;
#[async_trait]
impl Tool for TaskTool {
    fn name(&self) -> &str { "task" }
    fn description(&self) -> &str {
        "派生一个一次性子代理来独立完成某个子任务。子代理有独立的上下文，完成后只返回最终结论。适合需要大量中间步骤但你只关心结果的子任务。"
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "description": { "type": "string", "description": "交给子代理的完整任务描述" }
            },
            "required": ["description"]
        })
    }
    async fn run(&self, _input: &Value, _ctx: &ToolCtx) -> ToolOutput {
        // 不会被直接调用 (loop_engine 拦截)
        ToolOutput::err("task 应由循环引擎处理")
    }
}

// spawn_teammate
pub struct SpawnTeammateTool;
#[async_trait]
impl Tool for SpawnTeammateTool {
    fn name(&self) -> &str { "spawn_teammate" }
    fn description(&self) -> &str {
        "创建一个持久的团队成员 (队友)，它会持续运行、可与你双向通信、并能自主认领任务图中的任务。适合需要并行、长期协作的场景。"
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "description": "队友名称 (唯一)" },
                "role": { "type": "string", "description": "队友的职责描述" },
                "initial_task": { "type": "string", "description": "交给队友的初始任务 (可选)" }
            },
            "required": ["name", "role"]
        })
    }
    async fn run(&self, input: &Value, ctx: &ToolCtx) -> ToolOutput {
        let name = input["name"].as_str().unwrap_or("").trim().to_string();
        let role = input["role"].as_str().unwrap_or("").to_string();
        let initial = input["initial_task"].as_str().unwrap_or("").to_string();
        if name.is_empty() {
            return ToolOutput::err("name 不能为空");
        }
        if ctx.state.team.get(&name).is_some() {
            return ToolOutput::err(format!("队友 {name} 已存在"));
        }
        crate::team::autonomous::spawn_teammate(
            ctx.state.clone(),
            ctx.app.clone(),
            ctx.session_id.clone(),
            name.clone(),
            role,
            initial,
        );
        ToolOutput::ok(format!("已创建队友 {name}"))
    }
}

// shutdown_teammate (体面关机握手)
pub struct ShutdownTeammateTool;
#[async_trait]
impl Tool for ShutdownTeammateTool {
    fn name(&self) -> &str { "shutdown_teammate" }
    fn description(&self) -> &str { "请求某个队友完成当前工作后退出 (发送 shutdown_request)。" }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": { "name": { "type": "string" } },
            "required": ["name"]
        })
    }
    async fn run(&self, input: &Value, ctx: &ToolCtx) -> ToolOutput {
        let name = input["name"].as_str().unwrap_or("");
        if ctx.state.team.get(name).is_none() {
            return ToolOutput::err(format!("队友 {name} 不存在"));
        }
        // 通过协议发起 shutdown + 发消息
        let req_id = ctx
            .state
            .protocol
            .create("shutdown", &ctx.agent_name, name, "请完成当前工作后退出");
        let msg = crate::team::mailbox::MailMessage {
            from: ctx.agent_name.clone(),
            to: name.to_string(),
            content: "请完成当前工作后退出".into(),
            mtype: "shutdown_request".into(),
            ts: crate::team::mailbox::MailBus::now_ts(),
            request_id: Some(req_id.clone()),
        };
        ctx.state.mailbox.send(&msg);
        ctx.state.team.request_shutdown(name);
        ToolOutput::ok(format!("已请求队友 {name} 关机 (request_id={req_id})"))
    }
}
