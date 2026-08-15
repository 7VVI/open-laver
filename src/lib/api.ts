// Tauri 命令封装 + 类型定义
import { invoke } from "@tauri-apps/api/core";

export type Role = "user" | "assistant";

export interface ContentBlock {
  type: "text" | "tool_use" | "tool_result";
  text?: string;
  id?: string;
  name?: string;
  input?: any;
  tool_use_id?: string;
  content?: string;
  is_error?: boolean;
}

export interface Message {
  role: Role;
  content: ContentBlock[];
}

export interface DirEntry {
  name: string;
  path: string;
  is_dir: boolean;
  children?: DirEntry[];
}

export interface SessionDto {
  id: string;
  title: string;
  created_at: string;
  pinned: boolean;
}

export interface StorageItem {
  label: string;
  path: string;
  bytes: number;
}

export interface StorageInfo {
  workspace: string;
  default_workspace: string;
  data_dir: string;
  total_bytes: number;
  items: StorageItem[];
}

export type ProviderKind = "anthropic" | "openai_compat";
export type ThinkingLevel = "off" | "low" | "medium" | "high";

export interface ModelProfile {
  id: string;
  name: string;
  kind: ProviderKind;
  base_url: string;
  model: string;
  context_window: number;
  thinking: ThinkingLevel;
  supports_thinking: boolean;
}

export interface ModelProfileDto extends ModelProfile {
  has_key: boolean;
  active: boolean;
}

export interface TodoItem {
  content: string;
  status: string;
}

export interface SkillMeta {
  name: string;
  description: string;
  when_to_use: string | null;
  dir: string;
}

export interface MemoryItem {
  name: string;
  description: string;
  mtype: string;
  content: string;
}

export interface Task {
  id: string;
  subject: string;
  description: string;
  status: string;
  owner: string | null;
  blocked_by: string[];
  worktree: string | null;
}

export interface CronJob {
  id: string;
  title: string;
  expr: string;
  prompt: string;
  recurring: boolean;
  durable: boolean;
  session_id: string;
}

export interface CronRun {
  job_id: string;
  title: string;
  prompt: string;
  trigger: string;
  ran_at: string;
}

export interface TeammateInfo {
  name: string;
  role: string;
  phase: "work" | "idle" | "shutdown";
  session_id: string;
}

export interface McpServerStatus {
  name: string;
  connected: boolean;
  tool_count: number;
  tools: string[];
  error: string | null;
}

export type DesignKind = "icon" | "ip" | "prototype";

export interface DesignItem {
  id: string;
  kind: DesignKind;
  prompt: string;
  created_at: string;
  path: string;
  bytes: number;
  mode: "image" | "vector" | "html";
  note?: string | null;
}

export interface DesignConfig {
  base_url: string;
  model: string;
  has_key: boolean;
}

export const api = {
  createSession: (title: string) => invoke<SessionDto>("create_session", { title }),
  listSessions: () => invoke<SessionDto[]>("list_sessions"),
  deleteSession: (sessionId: string) => invoke("delete_session", { sessionId }),
  renameSession: (sessionId: string, title: string) =>
    invoke("rename_session", { sessionId, title }),
  setSessionPinned: (sessionId: string, pinned: boolean) =>
    invoke("set_session_pinned", { sessionId, pinned }),
  exportSession: (sessionId: string) => invoke<string>("export_session", { sessionId }),
  loadMessages: (sessionId: string) =>
    invoke<Message[]>("load_session_messages", { sessionId }),
  sendMessage: (sessionId: string, content: string, attachments?: string[]) =>
    invoke("send_message", { sessionId, content, attachments: attachments ?? null }),
  cancelTurn: (sessionId: string) => invoke("cancel_turn", { sessionId }),
  isSessionRunning: (sessionId: string) => invoke<boolean>("is_session_running", { sessionId }),
  resolveApproval: (approvalId: string, decision: string) =>
    invoke("resolve_approval", { approvalId, decision }),

  getWorkspace: () => invoke<{ workspace: string }>("get_workspace"),
  setWorkspace: (path: string) => invoke("set_workspace", { path }),
  setDefaultWorkspace: (path: string) => invoke("set_default_workspace", { path }),
  getPermissionMode: () => invoke<string>("get_permission_mode"),
  setPermissionMode: (mode: string) => invoke("set_permission_mode", { mode }),
  getStorageInfo: () => invoke<StorageInfo>("get_storage_info"),
  listDirTree: (path: string) => invoke<DirEntry[]>("list_dir_tree", { path }),
  openPath: (path: string) => invoke("open_path", { path }),

  // 模型管理
  listModels: () => invoke<ModelProfileDto[]>("list_models"),
  saveModel: (profile: ModelProfile, apiKey?: string) =>
    invoke<string>("save_model", { profile, apiKey: apiKey ?? null }),
  deleteModel: (id: string) => invoke("delete_model", { id }),
  setActiveModel: (id: string) => invoke("set_active_model", { id }),
  setModelKey: (id: string, key: string) => invoke("set_model_key", { id, key }),
  setModelRuntime: (id: string, contextWindow?: number, thinking?: ThinkingLevel) =>
    invoke("set_model_runtime", {
      id,
      contextWindow: contextWindow ?? null,
      thinking: thinking ?? null,
    }),
  testConnection: () => invoke<string>("test_connection"),

  listSkills: () => invoke<SkillMeta[]>("list_skills"),
  skillsRoot: () => invoke<string>("skills_root"),
  rescanSkills: () => invoke<SkillMeta[]>("rescan_skills"),
  createSkill: (name: string, description: string, whenToUse: string, body: string) =>
    invoke<SkillMeta[]>("create_skill", { name, description, whenToUse: whenToUse || null, body }),
  readSkill: (name: string) => invoke<string>("read_skill", { name }),
  importSkillZip: (path: string) => invoke<string>("import_skill_zip", { path }),
  deleteSkill: (name: string) => invoke<SkillMeta[]>("delete_skill", { name }),

  listMemories: () => invoke<MemoryItem[]>("list_memories"),
  deleteMemory: (name: string) => invoke("delete_memory", { name }),

  listTasks: () => invoke<Task[]>("list_tasks"),
  listCronJobs: () => invoke<CronJob[]>("list_cron_jobs"),
  cancelCronJob: (id: string) => invoke("cancel_cron_job", { id }),
  createCronJob: (
    sessionId: string,
    title: string,
    cron: string,
    prompt: string,
    recurring?: boolean
  ) =>
    invoke<string>("create_cron_job", {
      sessionId,
      title: title || null,
      cron,
      prompt,
      recurring: recurring ?? null,
    }),
  runCronNow: (id: string) => invoke("run_cron_now", { id }),
  listCronRuns: () => invoke<CronRun[]>("list_cron_runs"),
  listTeammates: () => invoke<TeammateInfo[]>("list_teammates"),
  getTodos: (sessionId: string) => invoke<TodoItem[]>("get_todos", { sessionId }),

  listMcpServers: () => invoke<McpServerStatus[]>("list_mcp_servers"),
  getMcpConfig: () => invoke<any>("get_mcp_config"),
  saveMcpConfig: (config: any) => invoke("save_mcp_config", { config }),

  // 设计工作室
  generateDesign: (kind: DesignKind, prompt: string, style?: string, size?: string) =>
    invoke<DesignItem>("generate_design", {
      kind,
      prompt,
      style: style ?? null,
      size: size ?? null,
    }),
  readDesign: (id: string) => invoke<string>("read_design", { id }),
  listDesigns: () => invoke<DesignItem[]>("list_designs"),
  deleteDesign: (id: string) => invoke("delete_design", { id }),
  getDesignConfig: () => invoke<DesignConfig>("get_design_config"),
  saveDesignConfig: (baseUrl: string, model: string, apiKey?: string) =>
    invoke("save_design_config", {
      baseUrl,
      model,
      apiKey: apiKey ?? null,
    }),
  testDesign: () => invoke<string>("test_design_connection"),
};

export const THINKING_LABELS: Record<ThinkingLevel, string> = {
  off: "不思考",
  low: "轻度思考",
  medium: "中度思考",
  high: "深度思考",
};

/// 上下文窗口挡位 (仅允许选择固定档位)
export const CONTEXT_TIERS: { label: string; value: number }[] = [
  { label: "200K", value: 200000 },
  { label: "400K", value: 400000 },
  { label: "1M", value: 1000000 },
];

export function formatContext(v: number): string {
  if (v >= 1000000) return `${(v / 1000000).toFixed(v % 1000000 === 0 ? 0 : 1)}M`;
  return `${Math.round(v / 1000)}K`;
}

export const PROVIDER_PRESETS: {
  label: string;
  kind: ProviderKind;
  base_url: string;
  model: string;
  context_window: number;
  supports_thinking: boolean;
}[] = [
  {
    label: "通义千问 Max",
    kind: "openai_compat",
    base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1",
    model: "qwen-max",
    context_window: 128000,
    supports_thinking: true,
  },
  {
    label: "通义千问 Plus",
    kind: "openai_compat",
    base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1",
    model: "qwen-plus",
    context_window: 128000,
    supports_thinking: true,
  },
  {
    label: "DeepSeek Chat",
    kind: "openai_compat",
    base_url: "https://api.deepseek.com/v1",
    model: "deepseek-chat",
    context_window: 64000,
    supports_thinking: false,
  },
  {
    label: "DeepSeek Reasoner",
    kind: "openai_compat",
    base_url: "https://api.deepseek.com/v1",
    model: "deepseek-reasoner",
    context_window: 64000,
    supports_thinking: true,
  },
  {
    label: "Kimi (Moonshot)",
    kind: "openai_compat",
    base_url: "https://api.moonshot.cn/v1",
    model: "moonshot-v1-32k",
    context_window: 32000,
    supports_thinking: false,
  },
  {
    label: "Claude 3.5 Sonnet",
    kind: "anthropic",
    base_url: "https://api.anthropic.com",
    model: "claude-3-5-sonnet-20241022",
    context_window: 200000,
    supports_thinking: false,
  },
  {
    label: "Claude 3.7 Sonnet (thinking)",
    kind: "anthropic",
    base_url: "https://api.anthropic.com",
    model: "claude-3-7-sonnet-20250219",
    context_window: 200000,
    supports_thinking: true,
  },
];
