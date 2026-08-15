//! 设计工作室 — 图标 / IP 形象 / 原型设计
//! 图像类产物优先走 OpenAI 兼容的 images/generations 接口（已配置生图模型时）；
//! 未配置生图模型（或调用失败）时，自动回退到当前对话模型生成矢量 SVG；
//! 原型类产物由当前对话模型直接生成单一 HTML 文件。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::llm::types::{ChatRequest, Message, StreamEvent, ThinkingLevel};
use crate::persistence::db::Db;
use crate::state::AppState;

/// 设计产物元数据 (与产物文件同名的 .json 存于设计目录)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignMeta {
    pub id: String,
    /// icon | ip | prototype
    pub kind: String,
    pub prompt: String,
    pub created_at: String,
    /// 产物文件名 (如 d8f1.png / d8f1.html)
    pub file: String,
    pub bytes: u64,
}

/// 图像生成配置 (密钥经系统凭据管理器保存)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignConfig {
    #[serde(default = "default_base_url")]
    pub base_url: String,
    #[serde(default = "default_model")]
    pub model: String,
}

impl Default for DesignConfig {
    fn default() -> Self {
        Self {
            base_url: default_base_url(),
            model: default_model(),
        }
    }
}

fn default_base_url() -> String {
    "https://dashscope.aliyuncs.com/compatible-mode/v1".into()
}

fn default_model() -> String {
    "wanx2.1-t2i-turbo".into()
}

pub struct DesignStore {
    db: Arc<Db>,
    dir: PathBuf,
    config: RwLock<DesignConfig>,
}

impl DesignStore {
    pub fn new(db: Arc<Db>, data_dir: &Path) -> Self {
        let dir = data_dir.join("design");
        let _ = std::fs::create_dir_all(&dir);
        let config = db
            .get_setting("design_config")
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self {
            db,
            dir,
            config: RwLock::new(config),
        }
    }

    pub fn dir(&self) -> PathBuf {
        self.dir.clone()
    }

    pub async fn config(&self) -> DesignConfig {
        self.config.read().await.clone()
    }

    pub async fn save_config(&self, cfg: DesignConfig) -> Result<(), String> {
        let json = serde_json::to_string(&cfg).map_err(|e| e.to_string())?;
        self.db.set_setting("design_config", &json)?;
        *self.config.write().await = cfg;
        Ok(())
    }

    pub fn set_key(&self, key: &str) -> Result<(), String> {
        crate::llm::credentials::set_api_key("design:image", key)
    }

    pub fn get_key(&self) -> Option<String> {
        crate::llm::credentials::get_api_key("design:image")
    }

    /// 按时间倒序列出全部设计产物
    pub fn list(&self) -> Vec<DesignMeta> {
        let mut out = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().map(|x| x == "json").unwrap_or(false) {
                    if let Ok(s) = std::fs::read_to_string(&p) {
                        if let Ok(m) = serde_json::from_str::<DesignMeta>(&s) {
                            out.push(m);
                        }
                    }
                }
            }
        }
        out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        out
    }

    pub fn get(&self, id: &str) -> Option<DesignMeta> {
        self.list().into_iter().find(|m| m.id == id)
    }

    pub fn read_file(&self, meta: &DesignMeta) -> Option<Vec<u8>> {
        std::fs::read(self.dir.join(&meta.file)).ok()
    }

    pub fn delete(&self, id: &str) -> Result<(), String> {
        if let Some(meta) = self.get(id) {
            let _ = std::fs::remove_file(self.dir.join(&meta.file));
            let _ = std::fs::remove_file(self.dir.join(format!("{id}.json")));
        }
        Ok(())
    }

    pub fn save(&self, meta: &DesignMeta, data: &[u8]) -> Result<(), String> {
        std::fs::write(self.dir.join(&meta.file), data).map_err(|e| e.to_string())?;
        let json = serde_json::to_string(meta).map_err(|e| e.to_string())?;
        std::fs::write(self.dir.join(format!("{}.json", meta.id)), json)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn next_id(&self, kind: &str, ext: &str) -> DesignMeta {
        let id = format!("d-{}", uuid::Uuid::new_v4().simple());
        let file = format!("{id}.{ext}");
        DesignMeta {
            id,
            kind: kind.to_string(),
            prompt: String::new(),
            created_at: chrono::Local::now().to_rfc3339(),
            file,
            bytes: 0,
        }
    }
}

/// 按设计类型组装图像生成提示词
pub fn image_prompt(kind: &str, prompt: &str, style: Option<&str>) -> String {
    let style = style.unwrap_or("").trim();
    let style_part = if style.is_empty() {
        String::new()
    } else {
        format!("风格：{style}。")
    };
    match kind {
        "ip" => format!(
            "设计一个品牌 IP 吉祥物：{prompt}。{style_part}要求：正面立绘、表情友善有亲和力、线条干净利落、造型独特易记忆，适合作为产品代言形象；背景使用纯色或简洁渐变，主体居中，不要出现文字。"
        ),
        _ => format!(
            "设计一款应用图标：{prompt}。{style_part}要求：居中构图、简洁易识别、圆角方形轮廓，适合作为桌面应用图标；使用扁平化矢量风格，背景纯色或简洁渐变，不要出现文字。"
        ),
    }
}

/// 调用 OpenAI 兼容图像接口生成图片，返回 (元数据, PNG 字节)
pub async fn generate_image(
    state: &AppState,
    kind: &str,
    prompt: &str,
    size: Option<&str>,
) -> Result<(DesignMeta, Vec<u8>), String> {
    let cfg = state.design.config().await;
    let api_key = state
        .design
        .get_key()
        .filter(|k| !k.is_empty())
        .ok_or_else(|| "尚未配置图像生成 API Key，请到「设置 → 设计」中填写".to_string())?;
    let base = cfg.base_url.trim_end_matches('/');
    let model = cfg.model.trim();
    if model.is_empty() {
        return Err("图像模型未配置，请到「设置 → 设计」中填写".into());
    }
    let size = size.unwrap_or("1024x1024");

    let body = serde_json::json!({
        "model": model,
        "prompt": prompt,
        "n": 1,
        "size": size,
        "response_format": "b64_json",
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{base}/images/generations"))
        .bearer_auth(&api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("请求图像接口失败: {e}"))?;
    let status = resp.status().as_u16();
    if status >= 400 {
        let text = resp.text().await.unwrap_or_default();
        return Err(format!(
            "图像接口返回 {status}: {}",
            truncate(&text, 400)
        ));
    }
    let v: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("解析图像响应失败: {e}"))?;
    let item = v
        .get("data")
        .and_then(|d| d.get(0))
        .ok_or_else(|| "图像接口未返回数据".to_string())?;

    let bytes = if let Some(b64) = item.get("b64_json").and_then(|x| x.as_str()) {
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64)
            .map_err(|e| format!("图像数据解码失败: {e}"))?
    } else if let Some(url) = item.get("url").and_then(|x| x.as_str()) {
        client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("下载图像失败: {e}"))?
            .bytes()
            .await
            .map_err(|e| format!("读取图像失败: {e}"))?
            .to_vec()
    } else {
        return Err("图像接口返回格式无法识别".into());
    };
    if bytes.is_empty() {
        return Err("图像接口返回了空数据".into());
    }

    let mut meta = state.design.next_id(kind, "png");
    meta.prompt = prompt.to_string();
    meta.bytes = bytes.len() as u64;
    state.design.save(&meta, &bytes)?;
    Ok((meta, bytes))
}

/// 调用当前对话模型生成矢量 SVG（无需生图模型，任何已配置模型均可）
pub async fn generate_vector(
    state: &AppState,
    kind: &str,
    prompt: &str,
    style: Option<&str>,
) -> Result<(DesignMeta, Vec<u8>), String> {
    let (provider, settings) = state.provider().await?;
    let style = style.unwrap_or("").trim();
    let style_part = if style.is_empty() {
        String::new()
    } else {
        format!("，风格为 {style}")
    };
    let system = "你是一名资深矢量设计师，擅长用 SVG 创作应用图标与品牌 IP 形象。你只输出有效的 SVG 代码，不输出任何解释、Markdown 围栏或其他文字。";
    let user = match kind {
        "ip" => format!(
            "请用 SVG 设计一个品牌 IP 吉祥物：{prompt}{style_part}。\n\n输出要求：\n1. 输出单一 <svg>，使用 viewBox=\"0 0 512 512\"，内容完整自包含（不要外部图片/字体）。\n2. 造型可爱有亲和力：包含头部、身体、五官、四肢等基础形状，线条干净、比例协调。\n3. 使用少量渐变与高光让形象更立体，配色和谐（蓝绿/暖色系均可）。\n4. 背景为纯色或简洁渐变圆形底，主体居中。\n5. 直接输出 <svg>…</svg> 代码本身，不要代码围栏，不要任何说明文字。"
        ),
        _ => format!(
            "请用 SVG 设计一款应用图标：{prompt}{style_part}。\n\n输出要求：\n1. 输出单一 <svg>，使用 viewBox=\"0 0 512 512\"，内容完整自包含（不要外部图片/字体）。\n2. 包含圆角方形或圆形背景（纯色或简洁渐变）与居中主体图形，2~5 层即可。\n3. 简洁易识别、扁平矢量风格，图标内不要出现文字。\n4. 配色和谐，适合作为桌面应用图标。\n5. 直接输出 <svg>…</svg> 代码本身，不要代码围栏，不要任何说明文字。"
        ),
    };

    let req = ChatRequest {
        model: settings.model,
        system: system.to_string(),
        messages: vec![Message::user_text(user)],
        tools: vec![],
        max_tokens: 8_000,
        thinking: ThinkingLevel::Off,
    };
    let out: Arc<std::sync::Mutex<String>> = Arc::new(std::sync::Mutex::new(String::new()));
    let sink_out = out.clone();
    let sink = move |ev: StreamEvent| {
        if let StreamEvent::TextDelta { text: t } = ev {
            sink_out.lock().unwrap().push_str(&t);
        }
    };
    provider
        .chat(&req, &sink)
        .await
        .map_err(|e| format!("生成矢量设计失败: {e}"))?;
    let text = out.lock().unwrap().clone();

    let svg = extract_svg(&text);
    if svg.trim().is_empty() {
        return Err("模型没有返回有效的 SVG 代码".into());
    }

    let mut meta = state.design.next_id(kind, "svg");
    meta.prompt = prompt.to_string();
    let bytes = svg.into_bytes();
    meta.bytes = bytes.len() as u64;
    state.design.save(&meta, &bytes)?;
    Ok((meta, bytes))
}

/// 调用当前对话模型生成单一 HTML 原型
pub async fn generate_prototype(
    state: &AppState,
    prompt: &str,
    platform: Option<&str>,
) -> Result<(DesignMeta, Vec<u8>), String> {
    let (provider, settings) = state.provider().await?;
    let platform = platform.unwrap_or("桌面应用").trim();
    let system = "你是一名资深产品设计师与前端工程师，擅长把产品需求转成可直接运行的高保真交互原型。你只输出完整、自包含的 HTML 代码，不输出任何解释。";
    let user = format!(
        "请为以下需求生成一个可直接在浏览器打开的高保真交互原型。\n\n平台/形态：{platform}\n\n需求描述：\n{prompt}\n\n输出要求：\n1. 输出一份完整的 HTML 文档（以 <!DOCTYPE html> 开头），所有 CSS 与 JavaScript 全部内联在同一个文件中。\n2. 界面文本使用中文，视觉风格现代简洁（浅色主题，蓝绿主色调），合理使用卡片、圆角与留白。\n3. 包含主要页面/视图结构、导航、表单、列表等核心交互元素；用内联 JS 实现切换、弹窗、输入反馈等基础交互。\n4. 适配常见桌面窗口尺寸（1280x800），不要依赖外部网络资源。\n5. 直接输出 HTML 代码本身，不要使用代码围栏（不要用 ``` 包裹），不要附加任何说明文字。"
    );

    let req = ChatRequest {
        model: settings.model,
        system: system.to_string(),
        messages: vec![Message::user_text(user)],
        tools: vec![],
        max_tokens: 12_000,
        thinking: ThinkingLevel::Off,
    };
    let out: Arc<std::sync::Mutex<String>> = Arc::new(std::sync::Mutex::new(String::new()));
    let sink_out = out.clone();
    let sink = move |ev: StreamEvent| {
        if let StreamEvent::TextDelta { text: t } = ev {
            sink_out.lock().unwrap().push_str(&t);
        }
    };
    provider
        .chat(&req, &sink)
        .await
        .map_err(|e| format!("生成原型失败: {e}"))?;
    let text = out.lock().unwrap().clone();

    let html = extract_html(&text);
    if html.trim().is_empty() {
        return Err("模型没有返回有效 HTML".into());
    }

    let mut meta = state.design.next_id("prototype", "html");
    meta.prompt = prompt.to_string();
    let bytes = html.into_bytes();
    meta.bytes = bytes.len() as u64;
    state.design.save(&meta, &bytes)?;
    Ok((meta, bytes))
}

/// 从模型输出中提取 HTML 主体 (去掉 markdown 围栏与多余说明)
fn extract_html(text: &str) -> String {
    let mut t = text.trim().to_string();
    // 去掉代码围栏
    if t.contains("```") {
        let mut out = String::new();
        let mut in_code = false;
        for line in t.lines() {
            let l = line.trim();
            if l.starts_with("```") {
                in_code = !in_code;
                continue;
            }
            if in_code {
                out.push_str(line);
                out.push('\n');
            }
        }
        if !out.trim().is_empty() {
            t = out;
        }
    }
    let t = t.trim();
    if let Some(i) = t.find("<!DOCTYPE") {
        return t[i..].to_string();
    }
    if let Some(i) = t.find("<html") {
        return t[i..].to_string();
    }
    if let Some(i) = t.find("<body") {
        return format!(
            "<!DOCTYPE html><html lang=\"zh-CN\"><head><meta charset=\"utf-8\"><title>原型预览</title></head>{}",
            t[i..].to_string()
        );
    }
    t.to_string()
}

/// 从模型输出中提取 SVG 主体
fn extract_svg(text: &str) -> String {
    let mut t = text.trim().to_string();
    // 去掉代码围栏
    if t.contains("```") {
        let mut out = String::new();
        let mut in_code = false;
        for line in t.lines() {
            let l = line.trim();
            if l.starts_with("```") {
                in_code = !in_code;
                continue;
            }
            if in_code {
                out.push_str(line);
                out.push('\n');
            }
        }
        if !out.trim().is_empty() {
            t = out;
        }
    }
    let start = t.find("<svg").unwrap_or(0);
    let body = &t[start..];
    if let Some(end) = body.find("</svg>") {
        return body[..end + "</svg>".len()].to_string();
    }
    // 兜底：原样返回（模型可能只输出了代码片段）
    if t.contains("<svg") {
        t
    } else {
        String::new()
    }
}

pub fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect::<String>() + "…"
    }
}
