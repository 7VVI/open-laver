# Laver Agent

基于 **Tauri 2 + Rust + React** 的桌面办公 Agent 智能体。

完整实现了 Agent Harness 的各个核心机制（主循环 / 工具 / 权限 / Hooks / 技能 / 压缩 / 记忆 / 任务图 / cron / 团队 / worktree / MCP），并借鉴悟空 Agent 的模块划分（loop_engine / real_tools / memory / cron / skills / mcp / gateway / model_provider / process_manager）。

设计哲学：**循环只有一个，机制分层叠加**。

---

## 快速开始

前置：Rust (stable) 与 Node.js 18+。

```powershell
cd laver-agent
npm install                 # 安装前端依赖 (若 esbuild 被拦截: npm rebuild esbuild)
npm run tauri dev           # 开发模式 (启动 Vite + Tauri)
npm run tauri build         # 打包 Windows 安装包
```

首次运行后，进入 **设置** 页：
1. 选择工作目录（Agent 的文件操作默认限定其内）。
2. 选择模型提供商预设（通义千问 / DeepSeek / Kimi / Anthropic），填入 API Key，点「测试连接」。

数据目录：`%APPDATA%/laver-agent/`（SQLite、skills/、.memory/、.tasks/、.mailboxes/、.task_outputs/、mcpServers.json）。

---

## 架构总览

```
src-tauri/src/
├── main.rs / lib.rs          入口，注册 Tauri 命令 + AppState + 后台工作线程
├── constants.rs              全局阈值常量
├── state.rs                  AppState — 串联所有子系统
├── workers.rs                cron 调度器 tick + 队列交付
├── seed.rs                   首次运行播种内置办公技能
├── llm/                      多提供商抽象 (悟空 shared/llm)
│   ├── provider.rs           trait LlmProvider + 工厂
│   ├── anthropic.rs          Anthropic Messages (原生 tool_use, SSE)
│   ├── openai_compat.rs      OpenAI 兼容 (通义/DeepSeek/Kimi, function calling)
│   ├── types.rs              统一消息模型 + 双协议转换 + token 估算
│   ├── sse.rs                SSE 流解析 (UTF-8 安全)
│   └── credentials.rs        API Key -> Windows 凭据管理器 (keyring)
├── agent/
│   ├── loop_engine.rs        主循环 (9 步挂载顺序)
│   ├── session.rs            会话状态 + 管理器
│   ├── ctx.rs                ToolCtx + safe_path 工作区校验
│   ├── recovery.rs           错误恢复状态机
│   ├── subagent.rs           子代理
│   ├── tools/                工具注册表 + 全部内置工具
│   ├── permission/           三闸门权限 + 审批经纪人 + 审批记忆
│   ├── hooks/                hook 触发点
│   ├── skills/               技能扫描与加载
│   ├── compact/              四层压缩管线
│   ├── memory/               记忆服务 + 提取/合并
│   └── prompt/               分段 system prompt 组装 + 缓存
├── tasks/                    任务图 (文件持久化 + 文件锁)
├── background/               后台任务管理器
├── cron/                     croner 调度器
├── team/                     信箱 / 协议 / 队友生命周期
├── worktree/                 git worktree 隔离
├── mcp/                      stdio JSON-RPC 客户端 + 工具池
├── persistence/              SQLite (会话/cron/记忆/设置)
└── gateway/                  Tauri 命令层 + 事件流 (悟空 agui_stream)

src/ (React + TS)
├── App.tsx                   侧边栏导航 + 会话管理 + 审批弹窗 + 通知
├── lib/                      invoke 封装 + 事件订阅 hooks
└── views/                    Chat/Approval/TaskBoard/Team/Skills/Memory/Cron/Mcp/Settings
```

---

## 主循环挂载顺序

`loop_engine::run_agent_loop` 每轮迭代：

1. UserPromptSubmit hooks + todo 提醒注入
2. 注入 cron 队列 / 后台任务通知 / 队友 inbox
3. 压缩管线 L3→L1→L2→(超阈值)L4
4. 组装 system prompt: identity+tools+workspace+memory 索引+skills 目录
5. assemble_tool_pool: 内置 + MCP
6. LLM 流式调用，外包 recovery 错误恢复
7. 无 tool_use → Stop hooks → extract_memories → 结束
8. 有 tool_use → PreToolUse(权限) → 执行(可后台/子代理) → PostToolUse
9. tool_results 追加 → 回到 2

---

## 内置办公技能

首次运行自动播种到 `skills/`：
- **weekly-report** — 生成结构化工作周报
- **data-wrangling** — CSV/Excel 数据清洗与统计 (PowerShell)
- **file-organizer** — 批量文件归类与重命名
- **meeting-notes** — 会议纪要与行动项提炼

在「技能」页可启用/禁用；自定义技能只需在 `skills/` 下新建含 `SKILL.md` 的文件夹。

---

## 测试

```powershell
cd src-tauri
cargo test --lib          # 单元测试: 权限/恢复/协议/worktree 校验
cargo check               # 全量类型检查
```

## 说明

- 阈值常量集中在 `constants.rs` (200KB / 50 条 / 3 条 tool_result / 10 个记忆文件 / 退避公式等)。
- shell 工具在 Windows 上通过 `powershell -NoProfile -NonInteractive -Command` 执行，超时 120s、输出截断 30K 字符。
