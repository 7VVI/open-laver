//! s07 技能系统 — 两层加载
//! 第 1 层: 启动扫描 skills/*/SKILL.md，解析 YAML frontmatter，目录注入 system prompt
//! 第 2 层: load_skill(name) 时完整内容经 tool_result 注入当前对话

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct SkillMeta {
    pub name: String,
    pub description: String,
    pub when_to_use: Option<String>,
    pub dir: String,
}

pub struct SkillEntry {
    pub meta: SkillMeta,
    pub body: String,
    pub path: PathBuf,
}

#[derive(Default)]
pub struct SkillRegistry {
    skills: RwLock<BTreeMap<String, SkillEntry>>,
    root: RwLock<PathBuf>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 扫描技能根目录 (第 1 层)
    pub fn scan(&self, root: &Path) {
        *self.root.write().unwrap() = root.to_path_buf();
        let mut map = BTreeMap::new();
        if let Ok(rd) = std::fs::read_dir(root) {
            for entry in rd.flatten() {
                let dir = entry.path();
                if !dir.is_dir() {
                    continue;
                }
                let skill_md = dir.join("SKILL.md");
                if !skill_md.exists() {
                    continue;
                }
                if let Some(e) = parse_skill(&skill_md, &dir) {
                    map.insert(e.meta.name.clone(), e);
                }
            }
        }
        *self.skills.write().unwrap() = map;
    }

    /// 技能目录 (注入 system prompt) — 已安装即可用
    pub fn catalog(&self) -> String {
        let skills = self.skills.read().unwrap();
        let mut lines = Vec::new();
        for e in skills.values() {
            let hint = e
                .meta
                .when_to_use
                .as_deref()
                .filter(|s| !s.is_empty())
                .map(|s| format!(" (适用: {s})"))
                .unwrap_or_default();
            lines.push(format!("- {}: {}{}", e.meta.name, e.meta.description, hint));
        }
        lines.join("\n")
    }

    pub fn list(&self) -> Vec<SkillMeta> {
        self.skills
            .read()
            .unwrap()
            .values()
            .map(|e| e.meta.clone())
            .collect()
    }

    /// 读取技能完整内容 (第 2 层)
    pub fn load(&self, name: &str) -> Option<String> {
        let skills = self.skills.read().unwrap();
        skills.get(name).map(|e| {
            format!(
                "# 技能: {}\n{}\n\n(技能目录: {})",
                e.meta.name,
                e.body,
                e.path.parent().map(|p| p.display().to_string()).unwrap_or_default()
            )
        })
    }

    /// 读取技能 SKILL.md 正文 (供前端详情弹窗展示，从磁盘重读保证最新)
    pub fn read_body(&self, name: &str) -> Result<String, String> {
        let path = {
            let skills = self.skills.read().unwrap();
            skills.get(name).map(|e| e.path.clone())
        };
        let path = path.ok_or("技能不存在")?;
        let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let (_, body) = split_frontmatter(&raw);
        Ok(body.to_string())
    }

    pub fn root(&self) -> PathBuf {
        self.root.read().unwrap().clone()
    }

    /// 导入技能 zip 包 — SKILL.md 需位于根目录或单层文件夹内，解压到 skills/{name}/ 并重扫
    pub fn import_skill_zip(&self, zip_path: &Path) -> Result<String, String> {
        let root = self.root.read().unwrap().clone();
        if root.as_os_str().is_empty() {
            return Err("技能目录未初始化".into());
        }
        let file = std::fs::File::open(zip_path).map_err(|e| format!("无法打开文件: {e}"))?;
        let mut archive =
            zip::ZipArchive::new(file).map_err(|e| format!("无法读取 zip 包: {e}"))?;

        // 定位 SKILL.md: 根目录优先，其次单层顶级文件夹下
        let mut prefix: Option<String> = None;
        for i in 0..archive.len() {
            let name = archive.by_index(i).map_err(|e| e.to_string())?.name().replace('\\', "/");
            if name == "SKILL.md" {
                prefix = Some(String::new());
                break;
            }
            if let Some(dir) = name.strip_suffix("/SKILL.md") {
                if !dir.contains('/') && prefix.is_none() {
                    prefix = Some(format!("{dir}/"));
                }
            }
        }
        let prefix = prefix.ok_or("zip 包中未找到 SKILL.md (需位于根目录或单层文件夹内)")?;

        // 技能名: 顶级文件夹名，或 zip 文件名 (去扩展名)
        let name = if prefix.is_empty() {
            zip_path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default()
        } else {
            prefix.trim_end_matches('/').to_string()
        };
        let name = name.trim().to_string();
        if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains("..") {
            return Err("无法从 zip 包推断合法的技能名称".into());
        }
        let dest = root.join(&name);
        if dest.exists() {
            return Err(format!("已存在同名技能「{name}」，请先删除后重试"));
        }

        // 解压 prefix 下的全部文件 (防 zip-slip: 逐段校验路径)
        std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
        let extract = (|| -> Result<(), String> {
            for i in 0..archive.len() {
                let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
                let raw = entry.name().replace('\\', "/");
                let Some(rel) = raw.strip_prefix(prefix.as_str()) else {
                    continue;
                };
                if rel.is_empty() {
                    continue;
                }
                if rel.split('/').any(|seg| seg.is_empty() && !raw.ends_with('/') || seg == "..") {
                    return Err(format!("zip 包内存在非法路径: {raw}"));
                }
                let out = dest.join(rel);
                if entry.is_dir() {
                    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
                } else {
                    if let Some(parent) = out.parent() {
                        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                    }
                    let mut f = std::fs::File::create(&out).map_err(|e| e.to_string())?;
                    std::io::copy(&mut entry, &mut f).map_err(|e| e.to_string())?;
                }
            }
            Ok(())
        })();
        if let Err(e) = extract {
            std::fs::remove_dir_all(&dest).ok(); // 失败回滚
            return Err(e);
        }

        self.scan(&root);
        Ok(name)
    }

    /// 创建一个新技能 (写入 skills/{name}/SKILL.md 并重扫)
    pub fn create_skill(
        &self,
        name: &str,
        description: &str,
        when_to_use: &str,
        body: &str,
    ) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("技能名称不能为空".into());
        }
        if name.contains('/') || name.contains('\\') || name.contains("..") {
            return Err("技能名称不能包含路径分隔符".into());
        }
        let root = self.root.read().unwrap().clone();
        if root.as_os_str().is_empty() {
            return Err("技能目录未初始化".into());
        }
        let dir = root.join(name);
        if dir.exists() {
            return Err("已存在同名技能".into());
        }
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let mut front = format!("---\nname: {name}\ndescription: {description}\n");
        if !when_to_use.trim().is_empty() {
            front.push_str(&format!("when_to_use: {when_to_use}\n"));
        }
        front.push_str("---\n\n");
        let content = format!("{front}{body}\n");
        std::fs::write(dir.join("SKILL.md"), content).map_err(|e| e.to_string())?;
        self.scan(&root);
        Ok(())
    }

    /// 删除技能 (移除其整个文件夹并重扫)
    pub fn delete_skill(&self, name: &str) -> Result<(), String> {
        let root = self.root.read().unwrap().clone();
        let dir = {
            let skills = self.skills.read().unwrap();
            skills
                .get(name)
                .and_then(|e| e.path.parent().map(|p| p.to_path_buf()))
        };
        let dir = dir.ok_or("技能不存在")?;
        // 安全: 必须在技能根目录之下，且不能是根目录本身
        if !dir.starts_with(&root) || dir == root {
            return Err("非法的技能路径".into());
        }
        std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
        self.scan(&root);
        Ok(())
    }
}

/// 解析 SKILL.md 的 YAML frontmatter + 正文
fn parse_skill(path: &Path, dir: &Path) -> Option<SkillEntry> {
    let raw = std::fs::read_to_string(path).ok()?;
    let (front, body) = split_frontmatter(&raw);
    let mut name = dir.file_name()?.to_string_lossy().to_string();
    let mut description = String::new();
    let mut when_to_use = None;

    if let Some(front) = front {
        if let Ok(val) = serde_yaml::from_str::<serde_yaml::Value>(&front) {
            if let Some(n) = val.get("name").and_then(|v| v.as_str()) {
                name = n.to_string();
            }
            if let Some(d) = val.get("description").and_then(|v| v.as_str()) {
                description = d.to_string();
            }
            if let Some(w) = val
                .get("when_to_use")
                .or_else(|| val.get("when-to-use"))
                .and_then(|v| v.as_str())
            {
                when_to_use = Some(w.to_string());
            }
        }
    }
    // 防路径遍历: 技能名不得含分隔符
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        return None;
    }

    Some(SkillEntry {
        meta: SkillMeta {
            name,
            description,
            when_to_use,
            dir: dir.display().to_string(),
        },
        body: body.to_string(),
        path: path.to_path_buf(),
    })
}

/// 分离 `---\nYAML\n---\n正文`
fn split_frontmatter(raw: &str) -> (Option<String>, &str) {
    let trimmed = raw.trim_start_matches('\u{feff}');
    if let Some(rest) = trimmed.strip_prefix("---") {
        // 找到下一个 --- 行
        if let Some(end) = rest.find("\n---") {
            let front = &rest[..end];
            let after = &rest[end + 4..];
            let body = after.trim_start_matches(['\r', '\n']);
            return (Some(front.trim().to_string()), body);
        }
    }
    (None, raw)
}
