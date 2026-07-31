//! s19 MCP 客户端 — stdio JSON-RPC 子进程: initialize -> tools/list -> tools/call

use std::process::Stdio;
use std::sync::atomic::{AtomicI64, Ordering};

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::sync::Mutex;

use super::config::McpServerConfig;

/// 发现到的 MCP 工具
#[derive(Debug, Clone)]
pub struct McpToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

pub struct McpClient {
    pub server_name: String,
    child: Mutex<Child>,
    stdin: Mutex<ChildStdin>,
    stdout: Mutex<BufReader<ChildStdout>>,
    next_id: AtomicI64,
    pub tools: Vec<McpToolDef>,
}

impl McpClient {
    /// 启动子进程并完成 initialize + tools/list
    pub async fn connect(name: &str, cfg: &McpServerConfig) -> Result<Self, String> {
        let mut command = tokio::process::Command::new(&cfg.command);
        command
            .args(&cfg.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        for (k, v) in &cfg.env {
            command.env(k, v);
        }
        let mut child = command
            .spawn()
            .map_err(|e| format!("启动 MCP 服务器 {name} 失败: {e}"))?;
        let stdin = child.stdin.take().ok_or("无法获取 stdin")?;
        let stdout = child.stdout.take().ok_or("无法获取 stdout")?;

        let client = Self {
            server_name: name.to_string(),
            child: Mutex::new(child),
            stdin: Mutex::new(stdin),
            stdout: Mutex::new(BufReader::new(stdout)),
            next_id: AtomicI64::new(1),
            tools: Vec::new(),
        };

        // initialize
        client
            .request(
                "initialize",
                json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": { "name": "laver-agent", "version": "0.1.0" }
                }),
            )
            .await?;
        client.notify("notifications/initialized", json!({})).await;

        // tools/list
        let mut client = client;
        let list = client.request("tools/list", json!({})).await?;
        if let Some(arr) = list["tools"].as_array() {
            for t in arr {
                client.tools.push(McpToolDef {
                    name: t["name"].as_str().unwrap_or("").to_string(),
                    description: t["description"].as_str().unwrap_or("").to_string(),
                    input_schema: t
                        .get("inputSchema")
                        .cloned()
                        .unwrap_or_else(|| json!({"type": "object"})),
                });
            }
        }
        Ok(client)
    }

    async fn write_msg(&self, msg: &Value) -> Result<(), String> {
        let line = format!("{}\n", msg);
        let mut stdin = self.stdin.lock().await;
        stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        stdin.flush().await.map_err(|e| e.to_string())
    }

    async fn notify(&self, method: &str, params: Value) {
        let msg = json!({ "jsonrpc": "2.0", "method": method, "params": params });
        let _ = self.write_msg(&msg).await;
    }

    /// 发送请求并读取匹配 id 的响应 (行分隔 JSON-RPC)
    async fn request(&self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let msg = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        self.write_msg(&msg).await?;

        let mut reader = self.stdout.lock().await;
        loop {
            let mut line = String::new();
            let n = tokio::time::timeout(
                std::time::Duration::from_secs(30),
                reader.read_line(&mut line),
            )
            .await
            .map_err(|_| "MCP 响应超时".to_string())?
            .map_err(|e| e.to_string())?;
            if n == 0 {
                return Err("MCP 连接已关闭".into());
            }
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Ok(v) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            // 跳过通知/其他 id
            if v.get("id").and_then(|i| i.as_i64()) != Some(id) {
                continue;
            }
            if let Some(err) = v.get("error") {
                return Err(format!("MCP 错误: {err}"));
            }
            return Ok(v.get("result").cloned().unwrap_or(Value::Null));
        }
    }

    /// 调用工具
    pub async fn call_tool(&self, tool: &str, args: &Value) -> Result<String, String> {
        let result = self
            .request(
                "tools/call",
                json!({ "name": tool, "arguments": args }),
            )
            .await?;
        // content 数组 -> 拼接文本
        if let Some(content) = result["content"].as_array() {
            let text = content
                .iter()
                .filter_map(|c| c["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n");
            if !text.is_empty() {
                return Ok(text);
            }
        }
        Ok(result.to_string())
    }

    pub async fn shutdown(&self) {
        let mut child = self.child.lock().await;
        let _ = child.kill().await;
    }
}
