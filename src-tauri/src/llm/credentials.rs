//! API Key 凭证管理 — keyring 存 Windows 凭据管理器 (悟空 global_llm.rs 加密存储对应物)

const SERVICE: &str = "laver-agent";

fn account_for(kind: &str) -> String {
    format!("llm-api-key-{kind}")
}

pub fn set_api_key(kind: &str, key: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(SERVICE, &account_for(kind)).map_err(|e| e.to_string())?;
    if key.is_empty() {
        // 空 key 视为删除
        let _ = entry.delete_credential();
        return Ok(());
    }
    entry.set_password(key).map_err(|e| e.to_string())
}

pub fn get_api_key(kind: &str) -> Option<String> {
    let entry = keyring::Entry::new(SERVICE, &account_for(kind)).ok()?;
    entry.get_password().ok()
}

pub fn has_api_key(kind: &str) -> bool {
    get_api_key(kind).map(|k| !k.is_empty()).unwrap_or(false)
}
