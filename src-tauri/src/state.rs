//! 全局应用状态 (Tauri managed state) — 串联所有子系统

use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::agent::hooks::registry::HookRegistry;
use crate::agent::memory::service::MemoryService;
use crate::agent::permission::approval::ApprovalBroker;
use crate::agent::prompt::builder::PromptBuilder;
use crate::agent::session::SessionManager;
use crate::agent::skills::registry::SkillRegistry;
use crate::agent::tools::registry::ToolRegistry;
use crate::background::manager::BackgroundManager;
use crate::cron::scheduler::CronScheduler;
use crate::llm::provider::{make_provider, LlmProvider, ProviderSettings};
use crate::mcp::pool::McpPool;
use crate::model_provider::ModelStore;
use crate::persistence::db::Db;
use crate::tasks::store::TaskStore;
use crate::team::mailbox::MailBus;
use crate::team::protocol::ProtocolRegistry;
use crate::team::registry::TeamRegistry;

/// 权限模式: 默认需要审批; 完全访问自动放行 (硬拒绝清单仍生效)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionMode {
    Default,
    Full,
}

pub struct AppState {
    pub data_dir: PathBuf,
    pub workspace: RwLock<PathBuf>,
    pub default_workspace: RwLock<PathBuf>,
    pub permission_mode: RwLock<PermissionMode>,
    pub models: Arc<ModelStore>,

    pub db: Arc<Db>,
    pub sessions: Arc<SessionManager>,
    pub approvals: Arc<ApprovalBroker>,
    pub registry: Arc<ToolRegistry>,
    pub skills: Arc<SkillRegistry>,
    pub prompt_builder: Arc<PromptBuilder>,
    pub memory: Arc<MemoryService>,
    pub tasks: Arc<TaskStore>,
    pub cron: Arc<CronScheduler>,
    pub team: Arc<TeamRegistry>,
    pub mailbox: Arc<MailBus>,
    pub protocol: Arc<ProtocolRegistry>,
    pub background: Arc<BackgroundManager>,
    pub mcp: Arc<McpPool>,
    pub hooks: Arc<HookRegistry>,
}

/// 默认工作空间位置: C:\Users\{user}\Laver (取不到用户目录时回退到旧默认)
fn default_home_laver_dir() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("USERPROFILE") {
        return Some(PathBuf::from(p).join("Laver"));
    }
    let drive = std::env::var_os("HOMEDRIVE")?;
    let path = std::env::var_os("HOMEPATH")?;
    Some(PathBuf::from(drive).join(path).join("Laver"))
}

impl AppState {
    pub fn new(data_dir: PathBuf, registry: Arc<ToolRegistry>) -> Arc<Self> {
        let db = Arc::new(
            Db::open(&data_dir.join("laver.db")).expect("打开 SQLite 数据库失败"),
        );

        // 工作区: 先使用已选择的实时工作空间, 没有选择过时回落到默认工作空间
        let default_workspace = db
            .get_setting("default_workspace")
            .map(PathBuf::from)
            .unwrap_or_else(|| default_home_laver_dir().unwrap_or_else(|| data_dir.join("workspace")));
        let _ = std::fs::create_dir_all(&default_workspace);
        let workspace = db
            .get_setting("workspace")
            .map(PathBuf::from)
            .unwrap_or_else(|| default_workspace.clone());
        let _ = std::fs::create_dir_all(&workspace);
        let permission_mode = match db.get_setting("permission_mode").as_deref() {
            Some("full") => PermissionMode::Full,
            _ => PermissionMode::Default,
        };

        // 多模型存储 (自动迁移旧的单一设置)
        let models = Arc::new(ModelStore::load(db.clone()));

        let memory = Arc::new(MemoryService::new(db.clone(), &data_dir));
        let skills = Arc::new(SkillRegistry::new());
        skills.scan(&data_dir.join("skills"));

        Arc::new(Self {
            workspace: RwLock::new(workspace),
            default_workspace: RwLock::new(default_workspace),
            permission_mode: RwLock::new(permission_mode),
            models,
            db,
            sessions: Arc::new(SessionManager::default()),
            approvals: Arc::new(ApprovalBroker::default()),
            registry,
            skills,
            prompt_builder: Arc::new(PromptBuilder::new()),
            memory,
            tasks: Arc::new(TaskStore::new(&data_dir)),
            cron: Arc::new(CronScheduler::new()),
            team: Arc::new(TeamRegistry::new()),
            mailbox: Arc::new(MailBus::new(&data_dir)),
            protocol: Arc::new(ProtocolRegistry::new()),
            background: Arc::new(BackgroundManager::new()),
            mcp: Arc::new(McpPool::new()),
            hooks: Arc::new(HookRegistry::new()),
            data_dir,
        })
    }

    /// 当前工作区
    pub async fn workspace(&self) -> PathBuf {
        self.workspace.read().await.clone()
    }

    /// 默认工作空间 (仅在未选择实时工作空间时使用)
    pub async fn default_workspace(&self) -> PathBuf {
        self.default_workspace.read().await.clone()
    }

    pub async fn permission_mode(&self) -> PermissionMode {
        *self.permission_mode.read().await
    }

    pub async fn set_permission_mode(&self, mode: PermissionMode) {
        *self.permission_mode.write().await = mode;
        let _ = self.db.set_setting(
            "permission_mode",
            match mode {
                PermissionMode::Default => "default",
                PermissionMode::Full => "full",
            },
        );
    }

    /// 设置实时工作空间: 只更新当前工作区与选择记录, 不影响默认工作空间
    pub async fn set_workspace(&self, path: PathBuf) {
        let _ = std::fs::create_dir_all(&path);
        *self.workspace.write().await = path.clone();
        let _ = self
            .db
            .set_setting("workspace", &path.to_string_lossy());
    }

    /// 设置默认工作空间: 仅当尚未选择过实时工作空间时同步更新当前工作区
    pub async fn set_default_workspace(&self, path: PathBuf) {
        let _ = std::fs::create_dir_all(&path);
        let has_current = self.db.get_setting("workspace").is_some();
        let _ = self
            .db
            .set_setting("default_workspace", &path.to_string_lossy());
        *self.default_workspace.write().await = path.clone();
        if !has_current {
            *self.workspace.write().await = path.clone();
        }
    }

    pub async fn settings(&self) -> ProviderSettings {
        self.models
            .active()
            .map(|p| p.to_settings())
            .unwrap_or_default()
    }

    /// 构造当前 LLM 提供商 (从活跃模型档案 + 该模型密钥)
    pub async fn provider(&self) -> Result<(Arc<dyn LlmProvider>, ProviderSettings), String> {
        let profile = self
            .models
            .active()
            .ok_or_else(|| "尚未配置任何模型，请先添加模型".to_string())?;
        let api_key = self
            .models
            .get_key(&profile.id)
            .filter(|k| !k.is_empty())
            .ok_or_else(|| format!("模型「{}」未配置 API Key", profile.name))?;
        let settings = profile.to_settings();
        Ok((make_provider(&settings, api_key), settings))
    }

    pub fn os_info(&self) -> String {
        format!("Windows ({}) — PowerShell", std::env::consts::ARCH)
    }

    pub fn data_dir(&self) -> &PathBuf {
        &self.data_dir
    }

    pub fn task_outputs_dir(&self) -> PathBuf {
        self.data_dir.join(".task_outputs")
    }

    pub fn transcripts_dir(&self) -> PathBuf {
        self.data_dir.join(".transcripts")
    }
}
