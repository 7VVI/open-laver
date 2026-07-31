//! 多模型管理 (悟空 model_provider 对应物)
//! 用户可保存多个模型配置、快速切换、快速添加；每个模型独立存密钥。

use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

use crate::llm::provider::{ProviderKind, ProviderSettings};
use crate::llm::types::ThinkingLevel;
use crate::persistence::db::Db;

/// 单个模型配置档案
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelProfile {
    pub id: String,
    /// 展示名 (如 "通义千问 Max")
    pub name: String,
    pub kind: ProviderKind,
    pub base_url: String,
    /// 实际模型标识 (如 qwen-max)
    pub model: String,
    #[serde(default = "default_ctx")]
    pub context_window: usize,
    #[serde(default)]
    pub thinking: ThinkingLevel,
    /// 该模型是否支持思考模式 (前端据此显示开关)
    #[serde(default)]
    pub supports_thinking: bool,
}

fn default_ctx() -> usize {
    crate::constants::DEFAULT_CONTEXT_WINDOW
}

impl ModelProfile {
    pub fn to_settings(&self) -> ProviderSettings {
        ProviderSettings {
            kind: self.kind,
            base_url: self.base_url.clone(),
            model: self.model.clone(),
            fallback_model: None,
            context_window: self.context_window,
            thinking: self.thinking,
        }
    }
}

/// 前端展示用 (含是否已配置密钥)
#[derive(Debug, Clone, Serialize)]
pub struct ModelProfileDto {
    #[serde(flatten)]
    pub profile: ModelProfile,
    pub has_key: bool,
    pub active: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ModelStoreData {
    profiles: Vec<ModelProfile>,
    active_id: Option<String>,
}

pub struct ModelStore {
    db: Arc<Db>,
    data: RwLock<ModelStoreData>,
}

const KEY_STORE: &str = "model_store";

impl ModelStore {
    pub fn load(db: Arc<Db>) -> Self {
        let mut data: ModelStoreData = db
            .get_setting(KEY_STORE)
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        // 首次使用: 迁移旧的单一 provider_settings，或播种一个默认档案
        if data.profiles.is_empty() {
            let seed = db
                .get_setting("provider_settings")
                .and_then(|s| serde_json::from_str::<ProviderSettings>(&s).ok())
                .unwrap_or_default();
            let id = new_id();
            data.profiles.push(ModelProfile {
                id: id.clone(),
                name: default_name(&seed.model),
                kind: seed.kind,
                base_url: seed.base_url,
                model: seed.model,
                context_window: seed.context_window,
                thinking: seed.thinking,
                supports_thinking: true,
            });
            data.active_id = Some(id);
        }
        if data.active_id.is_none() {
            data.active_id = data.profiles.first().map(|p| p.id.clone());
        }

        let store = Self {
            db,
            data: RwLock::new(data),
        };
        store.persist();
        store
    }

    fn persist(&self) {
        let data = self.data.read().unwrap();
        if let Ok(json) = serde_json::to_string(&*data) {
            let _ = self.db.set_setting(KEY_STORE, &json);
        }
    }

    pub fn list(&self) -> Vec<ModelProfileDto> {
        let data = self.data.read().unwrap();
        data.profiles
            .iter()
            .map(|p| ModelProfileDto {
                profile: p.clone(),
                has_key: crate::llm::credentials::has_api_key(&key_account(&p.id)),
                active: data.active_id.as_deref() == Some(p.id.as_str()),
            })
            .collect()
    }

    pub fn active(&self) -> Option<ModelProfile> {
        let data = self.data.read().unwrap();
        let id = data.active_id.as_ref()?;
        data.profiles.iter().find(|p| &p.id == id).cloned()
    }

    pub fn active_id(&self) -> Option<String> {
        self.data.read().unwrap().active_id.clone()
    }

    /// 新增/更新一个模型档案 (id 为空则新增)
    pub fn upsert(&self, mut profile: ModelProfile) -> String {
        if profile.id.trim().is_empty() {
            profile.id = new_id();
        }
        {
            let mut data = self.data.write().unwrap();
            if let Some(existing) = data.profiles.iter_mut().find(|p| p.id == profile.id) {
                *existing = profile.clone();
            } else {
                data.profiles.push(profile.clone());
                if data.active_id.is_none() {
                    data.active_id = Some(profile.id.clone());
                }
            }
        }
        self.persist();
        profile.id
    }

    pub fn delete(&self, id: &str) {
        {
            let mut data = self.data.write().unwrap();
            data.profiles.retain(|p| p.id != id);
            if data.active_id.as_deref() == Some(id) {
                data.active_id = data.profiles.first().map(|p| p.id.clone());
            }
        }
        let _ = crate::llm::credentials::set_api_key(&key_account(id), "");
        self.persist();
    }

    pub fn set_active(&self, id: &str) -> Result<(), String> {
        let mut data = self.data.write().unwrap();
        if !data.profiles.iter().any(|p| p.id == id) {
            return Err("模型不存在".into());
        }
        data.active_id = Some(id.to_string());
        drop(data);
        self.persist();
        Ok(())
    }

    /// 更新运行时参数 (上下文窗口 + 思考程度) — 作用于指定模型
    pub fn set_runtime(
        &self,
        id: &str,
        context_window: Option<usize>,
        thinking: Option<ThinkingLevel>,
    ) -> Result<(), String> {
        {
            let mut data = self.data.write().unwrap();
            let p = data
                .profiles
                .iter_mut()
                .find(|p| p.id == id)
                .ok_or("模型不存在")?;
            if let Some(cw) = context_window {
                p.context_window = cw.clamp(4000, 2_000_000);
            }
            if let Some(t) = thinking {
                p.thinking = t;
            }
        }
        self.persist();
        Ok(())
    }

    pub fn set_key(&self, id: &str, key: &str) -> Result<(), String> {
        crate::llm::credentials::set_api_key(&key_account(id), key)
    }

    pub fn get_key(&self, id: &str) -> Option<String> {
        crate::llm::credentials::get_api_key(&key_account(id))
    }
}

fn key_account(id: &str) -> String {
    format!("model:{id}")
}

fn new_id() -> String {
    format!("m-{}", uuid::Uuid::new_v4().simple())
}

fn default_name(model: &str) -> String {
    if model.is_empty() {
        "默认模型".to_string()
    } else {
        model.to_string()
    }
}
