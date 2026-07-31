//! 首次运行时把内置办公技能播种到数据目录 skills/ 下

use std::path::Path;

/// (相对路径, 内容) — 编译期嵌入内置技能
const BUNDLED: &[(&str, &str)] = &[
    (
        "weekly-report/SKILL.md",
        include_str!("../bundled-skills/weekly-report/SKILL.md"),
    ),
    (
        "data-wrangling/SKILL.md",
        include_str!("../bundled-skills/data-wrangling/SKILL.md"),
    ),
    (
        "file-organizer/SKILL.md",
        include_str!("../bundled-skills/file-organizer/SKILL.md"),
    ),
    (
        "meeting-notes/SKILL.md",
        include_str!("../bundled-skills/meeting-notes/SKILL.md"),
    ),
];

/// 若技能目录中尚不存在对应技能，则写入内置版本 (不覆盖用户修改)
pub fn seed_bundled_skills(skills_dir: &Path) {
    for (rel, content) in BUNDLED {
        let dest = skills_dir.join(rel);
        if dest.exists() {
            continue;
        }
        if let Some(parent) = dest.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&dest, content);
    }
}
