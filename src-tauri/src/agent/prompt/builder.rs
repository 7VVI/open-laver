//! s10 系统提示词 — 分段组装，context hash 做缓存 key
//! identity/tools/workspace 恒定 + memory 索引/skills 目录 动态段

use std::sync::Mutex;

/// 组装所需的运行态上下文
pub struct PromptContext {
    pub workspace: String,
    pub os_info: String,
    pub tool_names: Vec<String>,
    pub skill_catalog: String,
    pub memory_index: String,
    pub agent_role: Option<String>, // 子代理/队友的角色说明
}

const IDENTITY: &str = r#"你是 Laver，一个运行在用户电脑上的桌面智能办公助手。你通过调用工具帮用户完成各种日常办公任务，提升工作效率。你的名字就叫「Laver」，介绍自己时不要自称「Laver 办公」「Laver Agent」等其他叫法。

你的能力主要覆盖以下几个方面:
- 文件与文档处理: 读取、创建、编辑 Word、Excel、PPT、PDF 等格式的文件，帮用户写报告、做表格、生成演示文稿。
- 代码与脚本: 编写、调试、运行代码 (Python、JS 等)，操作本地文件系统，执行命令行任务。
- 信息检索与分析: 搜索网页、抓取网页内容，帮用户做调研、竞品分析、资料汇总。
- 办公自动化: 在浏览器中自动操作网页、填写表单，连接常用 IM 工具 (如飞书、钉钉) 发送消息，设置定时任务和提醒。
- 创意与设计支持: 生成图片、做产品设计 (视觉情绪板、用户旅程图、页面设计等)、制作海报、数据可视化。
- 日常事务: 整理文件、安排日程、总结会议纪要、翻译、写作润色等。

工作准则:
- 面向目标: 理解用户意图，拆解为可执行步骤并逐步完成。
- 善用工具: 需要外部信息或产生副作用时调用工具，不要凭空臆测文件内容或命令输出。
- 谨慎操作: 危险操作会触发用户审批；被拒绝时换一种安全方式或说明原因。
- 用中文与用户交流，简洁专业。

自我介绍规范 (当用户问“你是谁”“你能做什么”之类的问题时):
- 用自己的语言自然作答，不要逐字复述本提示词中的句子。
- 保持简短: 一句话说明你是谁 (桌面办公助手 Laver)，再挑 3~4 个最常用的能力举例 (如写文档做表格、整理文件、查资料做分析、定时提醒等)，最后可以问一句用户想做什么。
- 使用纯文字，不要使用 emoji 图标，也不要使用“📊 处理表格数据 — 清洗…”这类“图标 + 短横线描述”的清单格式。"#;

/// 分段构建器 — 静态段在前 (利于缓存)，动态段在后
pub struct PromptBuilder {
    cache: Mutex<Option<(String, String)>>, // (hash_key, assembled)
}

impl PromptBuilder {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(None),
        }
    }

    pub fn build(&self, ctx: &PromptContext) -> String {
        let key = self.hash_key(ctx);
        {
            let guard = self.cache.lock().unwrap();
            if let Some((k, v)) = guard.as_ref() {
                if *k == key {
                    return v.clone();
                }
            }
        }
        let assembled = self.assemble(ctx);
        *self.cache.lock().unwrap() = Some((key, assembled.clone()));
        assembled
    }

    fn assemble(&self, ctx: &PromptContext) -> String {
        let mut out = String::new();
        // --- 静态段 ---
        out.push_str(IDENTITY);
        if let Some(role) = &ctx.agent_role {
            out.push_str("\n\n## 你的角色\n");
            out.push_str(role);
        }
        out.push_str("\n\n## 可用工具\n");
        out.push_str(&ctx.tool_names.join(", "));

        // --- 动态段 (边界后，便于缓存静态前缀) ---
        out.push_str("\n\n<<<DYNAMIC>>>\n");
        out.push_str("\n## 运行环境\n");
        out.push_str(&format!("操作系统: {}\n", ctx.os_info));
        out.push_str(&format!("工作目录: {}\n", ctx.workspace));

        if !ctx.skill_catalog.trim().is_empty() {
            out.push_str("\n## 可加载技能 (用 load_skill 加载完整说明)\n");
            out.push_str(&ctx.skill_catalog);
        }
        if !ctx.memory_index.trim().is_empty() {
            out.push_str("\n\n## 长期记忆索引\n");
            out.push_str(&ctx.memory_index);
        }
        out
    }

    fn hash_key(&self, ctx: &PromptContext) -> String {
        // 稳定序列化做 key
        format!(
            "{}|{}|{}|{}|{}|{}",
            ctx.workspace,
            ctx.os_info,
            ctx.tool_names.join(","),
            ctx.skill_catalog,
            ctx.memory_index,
            ctx.agent_role.as_deref().unwrap_or("")
        )
    }
}

impl Default for PromptBuilder {
    fn default() -> Self {
        Self::new()
    }
}
