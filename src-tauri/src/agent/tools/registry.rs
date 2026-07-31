//! s02 工具注册表 — HashMap<name, Box<dyn Tool>>，加工具 = 加一条

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;

use super::tool::{Tool, ToolOutput};
use crate::agent::ctx::{AgentKind, ToolCtx};
use crate::llm::types::ToolSpec;

pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
    order: Vec<String>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            order: Vec::new(),
        }
    }

    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        let name = tool.name().to_string();
        if !self.tools.contains_key(&name) {
            self.order.push(name.clone());
        }
        self.tools.insert(name, tool);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    pub fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// 按注册顺序导出工具定义 (发给 LLM)
    pub fn specs(&self) -> Vec<ToolSpec> {
        self.order
            .iter()
            .filter_map(|n| self.tools.get(n))
            .map(|t| t.spec())
            .collect()
    }

    /// 按 Agent 类型过滤可见工具集 (s06 子代理无 task；队友精简集)
    pub fn specs_for(&self, kind: AgentKind, extra_hidden: &[&str]) -> Vec<ToolSpec> {
        self.order
            .iter()
            .filter(|n| visible_for(kind, n) && !extra_hidden.contains(&n.as_str()))
            .filter_map(|n| self.tools.get(n))
            .map(|t| t.spec())
            .collect()
    }

    /// 查表分发执行 (s02 核心)
    pub async fn dispatch(&self, name: &str, input: &Value, ctx: &ToolCtx) -> ToolOutput {
        match self.get(name) {
            Some(t) => t.run(input, ctx).await,
            None => ToolOutput::err(format!("未知工具: {name}")),
        }
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// 工具对某类 Agent 是否可见
fn visible_for(kind: AgentKind, name: &str) -> bool {
    match kind {
        AgentKind::Main => true,
        AgentKind::Sub => {
            // 子代理禁止再 spawn (防递归)，也不管理团队/定时
            !matches!(
                name,
                "task"
                    | "spawn_teammate"
                    | "send_message"
                    | "shutdown_teammate"
                    | "cron_schedule"
                    | "cron_list"
                    | "cron_cancel"
            )
        }
        AgentKind::Teammate => {
            // 队友: 基础工具 + 通信 + 任务认领
            matches!(
                name,
                "shell"
                    | "read_file"
                    | "write_file"
                    | "edit_file"
                    | "glob"
                    | "todo_write"
                    | "send_message"
                    | "task_list"
                    | "task_claim"
                    | "task_complete"
                    | "load_skill"
            )
        }
    }
}
