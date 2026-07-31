//! s19 MCP 工具池 — 合并内置 + MCP 工具，命名 mcp__{server}__{tool}，按前缀路由

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use tokio::sync::RwLock;

use super::client::McpClient;
use super::config::{McpConfig, McpServerConfig};
use crate::llm::types::ToolSpec;

#[derive(Debug, Clone, Serialize)]
pub struct McpServerStatus {
    pub name: String,
    pub connected: bool,
    pub tool_count: usize,
    pub tools: Vec<String>,
    pub error: Option<String>,
}

#[derive(Default)]
pub struct McpPool {
    clients: RwLock<HashMap<String, Arc<McpClient>>>,
    status: RwLock<HashMap<String, McpServerStatus>>,
    config_path: RwLock<PathBuf>,
}

/// 规范化工具名: mcp__{server}__{tool}，非法字符替换为 _
pub fn qualified_name(server: &str, tool: &str) -> String {
    let norm = |s: &str| {
        s.chars()
            .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
            .collect::<String>()
    };
    format!("mcp__{}__{}", norm(server), norm(tool))
}

impl McpPool {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn set_config_path(&self, path: PathBuf) {
        *self.config_path.write().await = path;
    }

    pub async fn config_path(&self) -> PathBuf {
        self.config_path.read().await.clone()
    }

    /// 连接配置中所有 enabled 的服务器
    pub async fn connect_all(&self) {
        let path = self.config_path().await;
        let cfg = McpConfig::load(&path);
        for (name, server_cfg) in cfg.servers {
            if server_cfg.enabled {
                self.connect_one(&name, &server_cfg).await;
            }
        }
    }

    pub async fn connect_one(&self, name: &str, cfg: &McpServerConfig) {
        match McpClient::connect(name, cfg).await {
            Ok(client) => {
                let tool_names: Vec<String> =
                    client.tools.iter().map(|t| t.name.clone()).collect();
                self.status.write().await.insert(
                    name.to_string(),
                    McpServerStatus {
                        name: name.to_string(),
                        connected: true,
                        tool_count: tool_names.len(),
                        tools: tool_names,
                        error: None,
                    },
                );
                self.clients
                    .write()
                    .await
                    .insert(name.to_string(), Arc::new(client));
            }
            Err(e) => {
                self.status.write().await.insert(
                    name.to_string(),
                    McpServerStatus {
                        name: name.to_string(),
                        connected: false,
                        tool_count: 0,
                        tools: vec![],
                        error: Some(e),
                    },
                );
            }
        }
    }

    pub async fn disconnect(&self, name: &str) {
        if let Some(c) = self.clients.write().await.remove(name) {
            c.shutdown().await;
        }
        if let Some(s) = self.status.write().await.get_mut(name) {
            s.connected = false;
            s.tool_count = 0;
            s.tools.clear();
        }
    }

    /// 合并全部 MCP 工具为 ToolSpec (供 assemble_tool_pool)
    pub async fn tool_specs(&self) -> Vec<ToolSpec> {
        let clients = self.clients.read().await;
        let mut specs = Vec::new();
        for (server, client) in clients.iter() {
            for t in &client.tools {
                specs.push(ToolSpec {
                    name: qualified_name(server, &t.name),
                    description: format!("[MCP:{server}] {}", t.description),
                    input_schema: t.input_schema.clone(),
                });
            }
        }
        specs
    }

    /// 按 mcp__{server}__{tool} 路由调用
    pub async fn call(&self, qualified: &str, args: &serde_json::Value) -> Result<String, String> {
        let rest = qualified
            .strip_prefix("mcp__")
            .ok_or("非 MCP 工具名")?;
        let (server, tool) = rest.split_once("__").ok_or("MCP 工具名格式错误")?;
        // 由于 server/tool 名做过规范化，需按规范化匹配
        let clients = self.clients.read().await;
        for (sname, client) in clients.iter() {
            if qualified_name(sname, "").starts_with(&format!("mcp__{server}__")) || sname == server
            {
                // 找到匹配的原始工具名
                for t in &client.tools {
                    let norm_tool: String = t
                        .name
                        .chars()
                        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
                        .collect();
                    if norm_tool == tool {
                        return client.call_tool(&t.name, args).await;
                    }
                }
            }
        }
        Err(format!("未找到 MCP 工具: {qualified}"))
    }

    pub async fn statuses(&self) -> Vec<McpServerStatus> {
        self.status.read().await.values().cloned().collect()
    }

    pub async fn is_mcp_tool(&self, name: &str) -> bool {
        name.starts_with("mcp__")
    }
}
