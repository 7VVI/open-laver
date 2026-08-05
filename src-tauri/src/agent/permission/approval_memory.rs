//! 审批记忆 (悟空 session_approval_memory 对应物)
//! "always allow" 决策按 pattern 记入会话状态，后续同类调用直接放行

use serde_json::Value;

/// 为工具调用生成稳定的审批模式 key。
/// shell 按命令首词 (程序名) 记忆；文件类按父目录记忆；其余按工具名。
pub fn pattern_for(tool: &str, input: &Value) -> String {
    match tool {
        "shell" => {
            let cmd = input["command"].as_str().unwrap_or("");
            let first = cmd.split_whitespace().next().unwrap_or("*");
            format!("shell:{}", first.to_lowercase())
        }
        "write_file" | "edit_file" => {
            let path = input["path"].as_str().unwrap_or("*");
            let parent = std::path::Path::new(path)
                .parent()
                .map(|p| p.to_string_lossy().to_lowercase())
                .unwrap_or_else(|| "*".into());
            format!("{tool}:{parent}")
        }
        _ => tool.to_string(),
    }
}
