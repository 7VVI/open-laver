//! Tauri 命令层 (悟空 gateway/commands.rs 对应物)

use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use tauri::State;

use crate::agent::loop_engine;
use crate::agent::permission::approval::ApprovalDecision;
use crate::agent::session::{SessionHandle, TodoItem};
use crate::agent::skills::registry::SkillMeta;
use crate::state::AppState;

type St<'a> = State<'a, Arc<AppState>>;

// ---------------- 会话 ----------------

#[derive(Serialize)]
pub struct SessionDto {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub pinned: bool,
}

#[tauri::command]
pub async fn create_session(state: St<'_>, title: String) -> Result<SessionDto, String> {
    let id = format!("s-{}", uuid::Uuid::new_v4().simple());
    let created_at = chrono::Local::now().to_rfc3339();
    let handle = SessionHandle::new(id.clone(), title.clone());
    state.sessions.insert(handle);
    state.db.upsert_session(&id, &title, &created_at)?;
    Ok(SessionDto {
        id,
        title,
        created_at,
        pinned: false,
    })
}

#[tauri::command]
pub async fn list_sessions(state: St<'_>) -> Result<Vec<SessionDto>, String> {
    Ok(state
        .db
        .list_sessions()
        .into_iter()
        .map(|(id, title, created_at, pinned)| SessionDto {
            id,
            title,
            created_at,
            pinned,
        })
        .collect())
}

#[tauri::command]
pub async fn delete_session(state: St<'_>, session_id: String) -> Result<(), String> {
    state.sessions.remove(&session_id);
    state.db.delete_session(&session_id)
}

/// 重命名会话
#[tauri::command]
pub async fn rename_session(state: St<'_>, session_id: String, title: String) -> Result<(), String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("会话名称不能为空".into());
    }
    state.db.rename_session(&session_id, title)
}

/// 置顶 / 取消置顶会话
#[tauri::command]
pub async fn set_session_pinned(
    state: St<'_>,
    session_id: String,
    pinned: bool,
) -> Result<(), String> {
    state.db.set_session_pinned(&session_id, pinned)
}

/// 导出会话记录为 Markdown，写入工作目录并返回路径
#[tauri::command]
pub async fn export_session(state: St<'_>, session_id: String) -> Result<String, String> {
    let title = state
        .db
        .list_sessions()
        .into_iter()
        .find(|(id, ..)| id == &session_id)
        .map(|(_, t, ..)| t)
        .unwrap_or_else(|| "会话".into());
    let msgs = state.db.load_messages(&session_id);
    let mut md = format!("# {title}\n\n");
    for m in &msgs {
        let role = match m.role {
            crate::llm::types::Role::User => "👤 用户",
            crate::llm::types::Role::Assistant => "🤖 助手",
        };
        let text = m.text();
        if text.trim().is_empty() {
            continue;
        }
        md.push_str(&format!("## {role}\n\n{text}\n\n"));
    }
    let safe: String = title
        .chars()
        .map(|c| if "\\/:*?\"<>|".contains(c) { '_' } else { c })
        .collect();
    let dir = state.workspace().await.join("exports");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("{safe}.md"));
    std::fs::write(&path, md).map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}

/// 加载会话历史消息 (前端渲染)
#[tauri::command]
pub async fn load_session_messages(
    state: St<'_>,
    session_id: String,
) -> Result<serde_json::Value, String> {
    // 确保内存中有该会话并载入历史
    let handle = match state.sessions.get(&session_id) {
        Some(h) => h,
        None => {
            let h = SessionHandle::new(session_id.clone(), "会话".into());
            let msgs = state.db.load_messages(&session_id);
            {
                let mut st = h.state.lock().await;
                st.messages = msgs;
            }
            state.sessions.insert(h.clone());
            h
        }
    };
    let st = handle.state.lock().await;
    serde_json::to_value(&st.messages).map_err(|e| e.to_string())
}

/// 发送用户消息，触发 agent turn (异步)；可携带附件文件路径
#[tauri::command]
pub async fn send_message(
    state: St<'_>,
    app: tauri::AppHandle,
    session_id: String,
    content: String,
    attachments: Option<Vec<String>>,
) -> Result<(), String> {
    // 确保会话在内存中
    let handle = match state.sessions.get(&session_id) {
        Some(h) => h,
        None => {
            let h = SessionHandle::new(session_id.clone(), "会话".into());
            let msgs = state.db.load_messages(&session_id);
            {
                let mut st = h.state.lock().await;
                st.messages = msgs;
            }
            state.sessions.insert(h.clone());
            h
        }
    };
    // 是否首条消息 (用于首次自动生成标题)
    let is_first = handle.state.lock().await.messages.is_empty();
    let raw = content.clone();
    // 将附件路径作为可读取提示附在消息末尾，供 Agent 用 read_file / 命令处理
    let mut full = content;
    if let Some(files) = attachments.filter(|f| !f.is_empty()) {
        let list = files
            .iter()
            .map(|p| format!("- {p}"))
            .collect::<Vec<_>>()
            .join("\n");
        full = format!(
            "{full}\n\n[附件] 用户提供了以下文件，需要时可用 read_file 读取或用 shell/技能处理：\n{list}"
        );
    }
    // 首条消息 -> 后台用 LLM 概括一个会话标题
    if is_first {
        let st_title = state.inner().clone();
        let app_title = app.clone();
        let sid_title = session_id.clone();
        tauri::async_runtime::spawn(async move {
            auto_title(st_title, app_title, sid_title, raw).await;
        });
    }
    let state_inner = state.inner().clone();
    tauri::async_runtime::spawn(async move {
        loop_engine::run_turn(state_inner, app, session_id, full).await;
    });
    Ok(())
}

/// 首条消息时用 LLM 总结一个简洁标题，仅当当前标题仍为默认值时覆盖
async fn auto_title(state: Arc<AppState>, app: tauri::AppHandle, session_id: String, content: String) {
    // 仅当标题仍为默认值 (新对话.../会话) 时才自动命名，不覆盖用户手动命名
    let cur = state
        .db
        .list_sessions()
        .into_iter()
        .find(|(id, ..)| id == &session_id)
        .map(|(_, t, ..)| t);
    if let Some(t) = &cur {
        if !(t.starts_with("新对话") || t == "会话") {
            return;
        }
    }
    let title = match summarize_title(&state, &content).await {
        Some(t) => t,
        None => return,
    };
    if state.db.rename_session(&session_id, &title).is_ok() {
        crate::gateway::events::emit(
            &app,
            crate::gateway::events::EV_SESSION_TITLE,
            crate::gateway::events::SessionTitlePayload { session_id, title },
        );
    }
}

/// 调用 LLM 把首条消息概括成短标题 (失败返回 None)
async fn summarize_title(state: &AppState, content: &str) -> Option<String> {
    use crate::llm::types::{ChatRequest, ContentBlock, Message, StreamEvent, ThinkingLevel};
    let (provider, settings) = state.provider().await.ok()?;
    let req = ChatRequest {
        model: settings.model.clone(),
        system: "你是标题生成助手。根据用户的第一条消息，用不超过 12 个汉字概括一个简洁的会话标题。只输出标题本身，不要引号、标点、前后缀或任何解释。".into(),
        messages: vec![Message::user_text(content)],
        tools: vec![],
        max_tokens: 32,
        thinking: ThinkingLevel::Off,
    };
    let noop = |_e: StreamEvent| {};
    let resp = provider.chat(&req, &noop).await.ok()?;
    let raw: String = resp
        .content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("");
    // 清洗: 取首行，去除引号/冒号/空白，限长
    let mut t = raw.lines().next().unwrap_or("").trim().to_string();
    t = t
        .trim_matches(|c: char| "\"'「」《》:：。".contains(c))
        .trim()
        .to_string();
    if t.chars().count() > 20 {
        t = t.chars().take(20).collect();
    }
    if t.is_empty() {
        None
    } else {
        Some(t)
    }
}

#[tauri::command]
pub async fn cancel_turn(state: St<'_>, session_id: String) -> Result<(), String> {
    if let Some(s) = state.sessions.get(&session_id) {
        s.cancel
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }
    Ok(())
}

/// 查询会话当前是否正在执行 turn (供切回会话时同步暂停键状态)
#[tauri::command]
pub async fn is_session_running(state: St<'_>, session_id: String) -> Result<bool, String> {
    Ok(state
        .sessions
        .get(&session_id)
        .map(|s| s.running.load(std::sync::atomic::Ordering::SeqCst))
        .unwrap_or(false))
}

// ---------------- 审批 ----------------

#[tauri::command]
pub async fn resolve_approval(
    state: St<'_>,
    approval_id: String,
    decision: ApprovalDecision,
) -> Result<(), String> {
    state.approvals.resolve(&approval_id, decision);
    Ok(())
}

// ---------------- 模型管理 (多模型 + 切换 + 快速添加) ----------------

use crate::llm::types::ThinkingLevel;
use crate::model_provider::{ModelProfile, ModelProfileDto};

#[derive(Serialize)]
pub struct WorkspaceDto {
    pub workspace: String,
}

#[tauri::command]
pub async fn get_workspace(state: St<'_>) -> Result<WorkspaceDto, String> {
    Ok(WorkspaceDto {
        workspace: state.workspace().await.to_string_lossy().to_string(),
    })
}

#[tauri::command]
pub async fn set_workspace(state: St<'_>, path: String) -> Result<(), String> {
    state.set_workspace(PathBuf::from(path)).await;
    Ok(())
}

#[tauri::command]
pub async fn get_permission_mode(state: St<'_>) -> Result<String, String> {
    Ok(match state.permission_mode().await {
        crate::state::PermissionMode::Default => "default".into(),
        crate::state::PermissionMode::Full => "full".into(),
    })
}

#[tauri::command]
pub async fn set_permission_mode(state: St<'_>, mode: String) -> Result<(), String> {
    let m = match mode.as_str() {
        "full" => crate::state::PermissionMode::Full,
        _ => crate::state::PermissionMode::Default,
    };
    state.set_permission_mode(m).await;
    Ok(())
}

#[tauri::command]
pub async fn set_default_workspace(state: St<'_>, path: String) -> Result<(), String> {
    state.set_default_workspace(PathBuf::from(path)).await;
    Ok(())
}

// ---------------- 存储 ----------------

#[derive(Serialize)]
pub struct StorageItemDto {
    pub label: String,
    pub path: String,
    pub bytes: u64,
}

#[derive(Serialize)]
pub struct StorageInfoDto {
    pub workspace: String,
    pub default_workspace: String,
    pub data_dir: String,
    pub total_bytes: u64,
    pub items: Vec<StorageItemDto>,
}

/// 递归统计目录占用字节数
fn dir_size(path: &std::path::Path) -> u64 {
    let mut total = 0u64;
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for entry in rd.flatten() {
                let p = entry.path();
                let meta = match entry.metadata() {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                if meta.is_dir() {
                    stack.push(p);
                } else if meta.is_file() {
                    total += meta.len();
                }
            }
        }
    }
    total
}

#[tauri::command]
pub async fn get_storage_info(state: St<'_>) -> Result<StorageInfoDto, String> {
    let workspace = state.workspace().await.to_string_lossy().to_string();
    let default_workspace = state.default_workspace().await.to_string_lossy().to_string();
    let data_dir = state.data_dir().clone();
    let mut items: Vec<StorageItemDto> = Vec::new();
    let mut other_bytes = 0u64;

    if let Ok(rd) = std::fs::read_dir(&data_dir) {
        for entry in rd.flatten() {
            let p = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let size = if p.is_dir() {
                dir_size(&p)
            } else if p.is_file() {
                entry.metadata().map(|m| m.len()).unwrap_or(0)
            } else {
                0
            };
            if size == 0 {
                continue;
            }
            let label = match name.as_str() {
                "laver.db" => "会话数据库",
                "skills" => "技能",
                ".memory" => "记忆",
                ".tasks" => "任务",
                ".mailboxes" => "邮件",
                ".task_outputs" => "任务输出",
                ".transcripts" => "对话记录",
                _ => {
                    other_bytes += size;
                    continue;
                }
            };
            items.push(StorageItemDto {
                label: label.to_string(),
                path: p.to_string_lossy().to_string(),
                bytes: size,
            });
        }
    }
    if other_bytes > 0 {
        items.push(StorageItemDto {
            label: "其他".to_string(),
            path: data_dir.to_string_lossy().to_string(),
            bytes: other_bytes,
        });
    }
    items.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    let total_bytes = items.iter().map(|i| i.bytes).sum();
    Ok(StorageInfoDto {
        workspace,
        default_workspace,
        data_dir: data_dir.to_string_lossy().to_string(),
        total_bytes,
        items,
    })
}

// ---------------- 工作目录文件树 ----------------

#[derive(Serialize)]
pub struct DirEntryDto {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub children: Option<Vec<DirEntryDto>>,
}

/// 递归列出目录树 (限制深度与条目数，跳过常见无关目录)
#[tauri::command]
pub async fn list_dir_tree(path: String) -> Result<Vec<DirEntryDto>, String> {
    const SKIP: &[&str] = &[
        "node_modules", ".git", "target", "dist", ".idea", ".next", ".cache",
        "__pycache__", ".venv", "venv", ".vscode", "build", "coverage",
    ];
    fn walk(dir: &std::path::Path, depth: usize) -> Vec<DirEntryDto> {
        if depth > 4 {
            return vec![];
        }
        let mut out = vec![];
        let Ok(entries) = std::fs::read_dir(dir) else {
            return out;
        };
        let mut items: Vec<_> = entries.filter_map(|e| e.ok()).collect();
        // 目录优先，再按名称排序
        items.sort_by_key(|e| {
            let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            (!is_dir, e.file_name().to_string_lossy().to_lowercase())
        });
        for e in items.iter().take(300) {
            let name = e.file_name().to_string_lossy().to_string();
            if SKIP.contains(&name.as_str()) {
                continue;
            }
            let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let path = e.path();
            if is_dir {
                out.push(DirEntryDto {
                    name: name.clone(),
                    path: path.to_string_lossy().to_string(),
                    is_dir: true,
                    children: Some(walk(&path, depth + 1)),
                });
            } else {
                out.push(DirEntryDto {
                    name,
                    path: path.to_string_lossy().to_string(),
                    is_dir: false,
                    children: None,
                });
            }
        }
        out
    }
    let root = std::path::Path::new(&path);
    if !root.is_dir() {
        return Err("路径不是目录".into());
    }
    Ok(walk(root, 0))
}

/// 在系统文件管理器中打开/定位路径 (Windows: explorer /select 打开目录或定位文件)
#[tauri::command]
pub async fn open_path(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(format!("/select,{path}"))
            .spawn()
            .map_err(|e| format!("打开失败: {e}"))?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = std::process::Command::new("open").arg(&path).spawn();
    }
    Ok(())
}

/// 列出所有模型档案 (含是否已配密钥、是否活跃)
#[tauri::command]
pub async fn list_models(state: St<'_>) -> Result<Vec<ModelProfileDto>, String> {
    Ok(state.models.list())
}

/// 新增/更新模型 (id 为空则新增)；可同时传入 api_key
#[tauri::command]
pub async fn save_model(
    state: St<'_>,
    profile: ModelProfile,
    api_key: Option<String>,
) -> Result<String, String> {
    let id = state.models.upsert(profile);
    if let Some(key) = api_key {
        if !key.is_empty() {
            state.models.set_key(&id, &key)?;
            // 刚配好密钥的模型自动设为当前，避免“已添加却仍用无密钥模型”
            let _ = state.models.set_active(&id);
        }
    }
    Ok(id)
}

#[tauri::command]
pub async fn delete_model(state: St<'_>, id: String) -> Result<(), String> {
    state.models.delete(&id);
    Ok(())
}

#[tauri::command]
pub async fn set_active_model(state: St<'_>, id: String) -> Result<(), String> {
    state.models.set_active(&id)
}

/// 设置模型密钥
#[tauri::command]
pub async fn set_model_key(state: St<'_>, id: String, key: String) -> Result<(), String> {
    state.models.set_key(&id, &key)
}

/// 设置运行时参数: 上下文窗口大小 + 思考程度
#[tauri::command]
pub async fn set_model_runtime(
    state: St<'_>,
    id: String,
    context_window: Option<usize>,
    thinking: Option<ThinkingLevel>,
) -> Result<(), String> {
    state.models.set_runtime(&id, context_window, thinking)
}

/// 测试连通性: 发一句简单请求
#[tauri::command]
pub async fn test_connection(state: St<'_>) -> Result<String, String> {
    use crate::llm::types::{ChatRequest, Message, StreamEvent};
    let (provider, settings) = state.provider().await?;
    let req = ChatRequest {
        model: settings.model.clone(),
        system: "你是测试助手，请用一句话回复。".into(),
        messages: vec![Message::user_text("说\"连接成功\"")],
        tools: vec![],
        max_tokens: 64,
        thinking: crate::llm::types::ThinkingLevel::Off,
    };
    let noop = |_e: StreamEvent| {};
    let resp = provider
        .chat(&req, &noop)
        .await
        .map_err(|e| e.to_string())?;
    Ok(resp
        .content
        .iter()
        .filter_map(|b| match b {
            crate::llm::types::ContentBlock::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" "))
}

// ---------------- 技能 ----------------

#[tauri::command]
pub async fn list_skills(state: St<'_>) -> Result<Vec<SkillMeta>, String> {
    Ok(state.skills.list())
}

/// 技能根目录路径 (供「通过助手创建技能」时告知保存位置)
#[tauri::command]
pub async fn skills_root(state: St<'_>) -> Result<String, String> {
    Ok(state.data_dir().join("skills").display().to_string())
}

#[tauri::command]
pub async fn rescan_skills(state: St<'_>) -> Result<Vec<SkillMeta>, String> {
    state.skills.scan(&state.data_dir().join("skills"));
    Ok(state.skills.list())
}

/// 新建技能 (写入 SKILL.md)
#[tauri::command]
pub async fn create_skill(
    state: St<'_>,
    name: String,
    description: String,
    when_to_use: Option<String>,
    body: String,
) -> Result<Vec<SkillMeta>, String> {
    state
        .skills
        .create_skill(&name, &description, &when_to_use.unwrap_or_default(), &body)?;
    Ok(state.skills.list())
}

/// 导入技能 zip 包，返回导入的技能名
#[tauri::command]
pub async fn import_skill_zip(state: St<'_>, path: String) -> Result<String, String> {
    state.skills.import_skill_zip(std::path::Path::new(&path))
}

/// 读取技能 SKILL.md 正文 (详情弹窗)
#[tauri::command]
pub async fn read_skill(state: St<'_>, name: String) -> Result<String, String> {
    state.skills.read_body(&name)
}

/// 删除技能
#[tauri::command]
pub async fn delete_skill(state: St<'_>, name: String) -> Result<Vec<SkillMeta>, String> {
    state.skills.delete_skill(&name)?;
    Ok(state.skills.list())
}

// ---------------- 记忆 ----------------

#[tauri::command]
pub async fn list_memories(
    state: St<'_>,
) -> Result<Vec<crate::agent::memory::service::MemoryItem>, String> {
    Ok(state.memory.all())
}

#[tauri::command]
pub async fn delete_memory(state: St<'_>, name: String) -> Result<(), String> {
    state.memory.delete(&name)
}

// ---------------- 任务图 ----------------

#[tauri::command]
pub async fn list_tasks(state: St<'_>) -> Result<Vec<crate::tasks::store::Task>, String> {
    Ok(state.tasks.all())
}

// ---------------- cron ----------------

#[tauri::command]
pub async fn list_cron_jobs(
    state: St<'_>,
) -> Result<Vec<crate::cron::scheduler::CronJob>, String> {
    Ok(state.cron.list())
}

#[tauri::command]
pub async fn cancel_cron_job(state: St<'_>, id: String) -> Result<(), String> {
    state.cron.remove(&id);
    state.db.delete_cron_job(&id)
}

/// 前端直接创建定时任务 (定时任务页 / “+”菜单入口)
#[tauri::command]
pub async fn create_cron_job(
    state: St<'_>,
    session_id: String,
    title: Option<String>,
    cron: String,
    prompt: String,
    recurring: Option<bool>,
) -> Result<String, String> {
    if prompt.trim().is_empty() {
        return Err("定时任务内容不能为空".into());
    }
    let recurring = recurring.unwrap_or(true);
    let title = title
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| prompt.lines().next().unwrap_or("定时任务").chars().take(40).collect());
    let id = format!("cron-{}", uuid::Uuid::new_v4().simple());
    let job = crate::cron::scheduler::CronJob {
        id: id.clone(),
        title: title.clone(),
        expr: cron.clone(),
        prompt: prompt.clone(),
        recurring,
        durable: true,
        session_id: session_id.clone(),
    };
    state.cron.add(job)?;
    let _ = state
        .db
        .save_cron_job(&id, &title, &cron, &prompt, recurring, true, &session_id);
    Ok(id)
}

/// 立即手动触发一个定时任务
#[tauri::command]
pub async fn run_cron_now(
    state: St<'_>,
    app: tauri::AppHandle,
    id: String,
) -> Result<(), String> {
    let job = state
        .cron
        .list()
        .into_iter()
        .find(|j| j.id == id)
        .ok_or("定时任务不存在")?;
    // 确保会话存在
    if state.sessions.get(&job.session_id).is_none() {
        let h = SessionHandle::new(job.session_id.clone(), "会话".into());
        let msgs = state.db.load_messages(&job.session_id);
        {
            let mut st = h.state.lock().await;
            st.messages = msgs;
        }
        state.sessions.insert(h);
    }
    state.db.record_cron_run(&job.id, &job.title, &job.prompt, "manual");
    let state_inner = state.inner().clone();
    let prompt = format!("[手动触发定时任务] {}", job.prompt);
    let sid = job.session_id.clone();
    tauri::async_runtime::spawn(async move {
        loop_engine::run_turn(state_inner, app, sid, prompt).await;
    });
    Ok(())
}

/// 列出定时任务执行记录
#[derive(Serialize)]
pub struct CronRunDto {
    pub job_id: String,
    pub title: String,
    pub prompt: String,
    pub trigger: String,
    pub ran_at: String,
}

#[tauri::command]
pub async fn list_cron_runs(state: St<'_>) -> Result<Vec<CronRunDto>, String> {
    Ok(state
        .db
        .list_cron_runs()
        .into_iter()
        .map(|(job_id, title, prompt, trigger, ran_at)| CronRunDto {
            job_id,
            title,
            prompt,
            trigger,
            ran_at,
        })
        .collect())
}

// ---------------- 团队 ----------------

#[tauri::command]
pub async fn list_teammates(
    state: St<'_>,
) -> Result<Vec<crate::team::registry::TeammateInfo>, String> {
    Ok(state.team.list())
}

#[tauri::command]
pub async fn get_todos(state: St<'_>, session_id: String) -> Result<Vec<TodoItem>, String> {
    match state.sessions.get(&session_id) {
        Some(s) => Ok(s.state.lock().await.todos.clone()),
        None => Ok(vec![]),
    }
}

// ---------------- MCP ----------------

#[tauri::command]
pub async fn list_mcp_servers(
    state: St<'_>,
) -> Result<Vec<crate::mcp::pool::McpServerStatus>, String> {
    Ok(state.mcp.statuses().await)
}

#[tauri::command]
pub async fn get_mcp_config(state: St<'_>) -> Result<serde_json::Value, String> {
    let path = state.mcp.config_path().await;
    let cfg = crate::mcp::config::McpConfig::load(&path);
    serde_json::to_value(&cfg).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_mcp_config(state: St<'_>, config: serde_json::Value) -> Result<(), String> {
    let cfg: crate::mcp::config::McpConfig =
        serde_json::from_value(config).map_err(|e| e.to_string())?;
    let path = state.mcp.config_path().await;
    cfg.save(&path)?;
    // 重新连接
    state.mcp.connect_all().await;
    Ok(())
}
