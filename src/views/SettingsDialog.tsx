import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, StorageInfo } from "../lib/api";
import ModelsView from "./ModelsView";
import MemoryView from "./MemoryView";
import McpView from "./McpView";

type Cat = "system" | "models" | "design" | "mcp" | "memory" | "help";

const svg = (paths: string) => (
  <svg viewBox="0 0 24 24" className="w-[18px] h-[18px]" fill="none" stroke="currentColor"
    strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
    {paths.split("|").map((d, i) => (
      <path key={i} d={d} />
    ))}
  </svg>
);

const CATS: { id: Cat; label: string; icon: JSX.Element }[] = [
  { id: "system", label: "系统设置", icon: svg("M12 15a3 3 0 100-6 3 3 0 000 6|M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 11-2.83 2.83l-.06-.06a1.65 1.65 0 00-2.82 1.17V21a2 2 0 11-4 0v-.09A1.65 1.65 0 006 19.4l-.06.06a2 2 0 11-2.83-2.83l.06-.06A1.65 1.65 0 004.6 15H4.5a2 2 0 110-4h.09A1.65 1.65 0 006 9.4") },
  { id: "models", label: "模型", icon: svg("M12 2l2.4 7.4H22l-6 4.6 2.3 7.4-6.3-4.6L5.7 21 8 14 2 9.4h7.6z") },
  { id: "design", label: "设计", icon: svg("M12 19l7-7 3 3-7 7-3-3z|M18 13l-1.5-7.5L2 2l3.5 14.5L13 18l5-5z|M2 2l7.586 7.586|M3.5 16.5L13 7") },
  { id: "mcp", label: "MCP", icon: svg("M9 7V4h6v3|M4 7h16v13H4z|M9 12h6") },
  { id: "memory", label: "记忆", icon: svg("M12 3a4 4 0 00-4 4v1a3 3 0 00-1 5.83V17a3 3 0 006 0|M12 3a4 4 0 014 4v1a3 3 0 011 5.83V17a3 3 0 01-6 0") },
  { id: "help", label: "帮助与反馈", icon: svg("M12 17h.01|M12 3a9 9 0 100 18 9 9 0 000-18z|M9.1 9a3 3 0 015.8 1c0 2-3 2.5-3 2.5") },
];

export default function SettingsDialog({
  initialCategory,
  onClose,
  onModelsChanged,
  onNotice,
}: {
  initialCategory?: string;
  onClose: () => void;
  onModelsChanged: () => void;
  onNotice?: (level: string, text: string) => void;
}) {
  const [cat, setCat] = useState<Cat>(
    (CATS.some((c) => c.id === initialCategory) ? initialCategory : "system") as Cat
  );

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div
      className="fixed inset-0 bg-black/30 flex items-center justify-center z-50 fade-in"
      onClick={onClose}
    >
      <div
        className="bg-white rounded-2xl shadow-2xl w-[920px] h-[640px] max-w-[94vw] max-h-[90vh] flex overflow-hidden relative"
        onClick={(e) => e.stopPropagation()}
      >
        {/* 左侧分类 */}
        <aside className="w-52 shrink-0 bg-[#f6f6f6] border-r border-slate-200 py-3 overflow-y-auto">
          <div className="px-4 pb-2 text-[13px] font-semibold text-slate-700">设置</div>
          {CATS.map((c) => (
            <button
              key={c.id}
              onClick={() => setCat(c.id)}
              className={`w-full flex items-center gap-3 px-4 py-2 text-sm transition ${
                cat === c.id
                  ? "bg-[#e0e0e0] text-[#333333] font-medium"
                  : "text-slate-600 hover:bg-slate-100"
              }`}
            >
              <span className={cat === c.id ? "text-[#333333]" : "text-slate-400"}>{c.icon}</span>
              {c.label}
            </button>
          ))}
        </aside>

        {/* 右侧内容 */}
        <div className="flex-1 min-w-0 relative">
          <button
            onClick={onClose}
            className="absolute top-3 right-3 z-10 w-8 h-8 rounded-lg flex items-center justify-center text-slate-400 hover:bg-slate-100 hover:text-slate-600"
            title="关闭"
          >
            <svg viewBox="0 0 24 24" className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round">
              <path d="M6 6l12 12M18 6L6 18" />
            </svg>
          </button>

          <div className="h-full overflow-y-auto">
            {cat === "system" && <SystemPane onNotice={onNotice} />}
            {cat === "models" && <ModelsView onChanged={onModelsChanged} />}
            {cat === "design" && <DesignPane onNotice={onNotice} />}
            {cat === "mcp" && <McpView />}
            {cat === "memory" && <MemoryView />}
            {cat === "help" && <HelpPane onNotice={onNotice} />}
          </div>
        </div>
      </div>
    </div>
  );
}

/* ---------------- 系统设置 ---------------- */

function SystemPane({ onNotice }: { onNotice?: (level: string, text: string) => void }) {
  const [workspace, setWorkspace] = useState("");
  const [storage, setStorage] = useState<StorageInfo | null>(null);

  const loadStorage = () => {
    api
      .getStorageInfo()
      .then((s) => {
        setStorage(s);
        setWorkspace(s.default_workspace);
      })
      .catch(() => {});
  };

  useEffect(() => {
    loadStorage();
  }, []);

  const pick = async () => {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir === "string") {
      await api.setDefaultWorkspace(dir);
      setWorkspace(dir);
      loadStorage();
      onNotice?.("info", "已更新默认工作空间");
    }
  };

  return (
    <div className="max-w-2xl px-8 py-8">
      <h1 className="text-xl font-semibold text-slate-800 mb-6">系统设置</h1>

      <Section title="存储">
        <div className="mb-5">
          <div className="flex items-center justify-between mb-1.5">
            <h3 className="text-sm font-medium text-slate-700">默认工作空间</h3>
            <button
              onClick={pick}
              className="bg-[#333333] hover:bg-[#111111] text-white rounded-lg px-3 py-1 text-xs"
            >
              选择
            </button>
          </div>
          <p className="text-xs text-slate-500 mb-2">
            智能体的文件操作默认限定在此目录内，目录之外的写操作会触发权限确认。
          </p>
          <div className="bg-slate-50 border border-slate-200 rounded-lg px-3 py-2 text-sm text-slate-700 truncate">
            {workspace || "加载中…"}
          </div>
          {storage && (
            <div className="text-xs text-slate-400 mt-2 truncate">
              当前工作空间（实时）：{storage.workspace}
            </div>
          )}
        </div>

        <div className="border-t border-slate-100 pt-4">
          <div className="flex items-center justify-between mb-1.5">
            <h3 className="text-sm font-medium text-slate-700">程序数据</h3>
            {storage && (
              <button
                onClick={() => api.openPath(storage.data_dir).catch(() => {})}
                className="bg-slate-100 hover:bg-slate-200 text-slate-600 rounded-lg px-3 py-1 text-xs"
              >
                打开文件夹
              </button>
            )}
          </div>
          <p className="text-xs text-slate-500 mb-2">
            应用配置、会话记录、记忆、技能等数据保存在本机此目录。
          </p>
          <div className="bg-slate-50 border border-slate-200 rounded-lg px-3 py-2 text-sm text-slate-700 truncate">
            {storage ? storage.data_dir : "加载中…"}
          </div>

          {storage && storage.total_bytes > 0 && (
            <div className="mt-5">
              <div className="flex items-baseline justify-between mb-2">
                <span className="text-xs text-slate-500">占用空间</span>
                <span className="text-sm font-semibold text-slate-700">
                  {fmtBytes(storage.total_bytes)}
                </span>
              </div>
              <div className="h-2 rounded-full bg-slate-100 overflow-hidden flex">
                {storage.items.map((it, i) => (
                  <div
                    key={it.label}
                    className="h-full"
                    style={{
                      width: pct(it.bytes, storage.total_bytes),
                      backgroundColor: BAR_COLORS[i % BAR_COLORS.length],
                    }}
                  />
                ))}
              </div>
              <div className="mt-3 space-y-2">
                {storage.items.map((it, i) => (
                  <div key={it.label} className="flex items-center gap-2">
                    <span className="w-16 shrink-0 text-xs text-slate-500">{it.label}</span>
                    <div className="flex-1 h-1.5 rounded-full bg-slate-100 overflow-hidden">
                      <div
                        className="h-full rounded-full"
                        style={{
                          width: pct(it.bytes, storage.total_bytes),
                          backgroundColor: BAR_COLORS[i % BAR_COLORS.length],
                        }}
                      />
                    </div>
                    <span className="w-20 shrink-0 text-right text-xs text-slate-500">
                      {fmtBytes(it.bytes)}
                    </span>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
      </Section>
    </div>
  );
}

const BAR_COLORS = [
  "#34c759",
  "#f59e0b",
  "#10b981",
  "#3b82f6",
  "#ef4444",
  "#1a7f37",
  "#06b6d4",
  "#64748b",
];

function fmtBytes(n: number): string {
  if (n >= 1 << 30) return (n / (1 << 30)).toFixed(2) + " GB";
  if (n >= 1 << 20) return (n / (1 << 20)).toFixed(1) + " MB";
  if (n >= 1 << 10) return Math.round(n / (1 << 10)) + " KB";
  return n + " B";
}

function pct(n: number, total: number): string {
  return total > 0 ? Math.max(0.5, (n / total) * 100) + "%" : "0%";
}

/* ---------------- 设计 / 图像模型 ---------------- */

const DESIGN_PRESETS: { label: string; base_url: string; model: string }[] = [
  {
    label: "通义万相 2.1（DashScope 推荐）",
    base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1",
    model: "wanx2.1-t2i-turbo",
  },
  {
    label: "通义千问图像（Qwen-Image）",
    base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1",
    model: "qwen-image",
  },
  {
    label: "OpenAI DALL·E 3",
    base_url: "https://api.openai.com/v1",
    model: "dall-e-3",
  },
  {
    label: "智谱 CogView-4",
    base_url: "https://open.bigmodel.cn/api/paas/v4",
    model: "cogview-4",
  },
];

function DesignPane({ onNotice }: { onNotice?: (level: string, text: string) => void }) {
  const [baseUrl, setBaseUrl] = useState("");
  const [model, setModel] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [hasKey, setHasKey] = useState(false);
  const [preset, setPreset] = useState(DESIGN_PRESETS[0].label);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [testMsg, setTestMsg] = useState<string | null>(null);

  const load = async () => {
    const cfg = await api.getDesignConfig();
    setBaseUrl(cfg.base_url);
    setModel(cfg.model);
    setHasKey(cfg.has_key);
    const p = DESIGN_PRESETS.find(
      (x) => x.base_url === cfg.base_url && x.model === cfg.model
    );
    if (p) setPreset(p.label);
  };

  useEffect(() => {
    load().catch(() => {});
  }, []);

  const applyPreset = (label: string) => {
    const p = DESIGN_PRESETS.find((x) => x.label === label);
    if (!p) return;
    setPreset(label);
    setBaseUrl(p.base_url);
    setModel(p.model);
  };

  const save = async () => {
    if (!baseUrl.trim() || !model.trim()) {
      onNotice?.("error", "接口地址与模型名称不能为空");
      return;
    }
    setSaving(true);
    try {
      await api.saveDesignConfig(
        baseUrl.trim(),
        model.trim(),
        apiKey.trim() ? apiKey.trim() : undefined
      );
      setApiKey("");
      setHasKey(hasKey || !!apiKey.trim());
      onNotice?.("info", "设计配置已保存");
      await load();
    } catch (e: any) {
      onNotice?.("error", "保存失败：" + (e?.toString() ?? ""));
    }
    setSaving(false);
  };

  const test = async () => {
    setTesting(true);
    setTestMsg(null);
    try {
      setTestMsg("✅ " + (await api.testDesign()));
    } catch (e: any) {
      setTestMsg("❌ " + (e?.toString() ?? "失败"));
    }
    setTesting(false);
  };

  return (
    <div className="max-w-2xl px-8 py-8">
      <h1 className="text-xl font-semibold text-slate-800 mb-6">设计设置</h1>

      <Section title="生图模型（可选）">
        <p className="text-xs text-slate-500 mb-4">
          用于「设计工作室」中图标与 IP 形象的位图生成，兼容 OpenAI images/generations 接口。
          <b className="text-slate-700"> 不配置也能设计：</b>
          此时会自动改用当前对话模型（任意已配置模型）生成矢量 SVG 图标/形象。
        </p>

        <div className="mb-4">
          <label className="text-xs font-medium text-slate-600 mb-1.5 block">快捷预设</label>
          <select
            value={preset}
            onChange={(e) => applyPreset(e.target.value)}
            className="w-full rounded-lg border border-slate-200 bg-white px-3 py-2 text-sm text-slate-700 focus:outline-none focus:ring-2 focus:ring-[#0f766e]/30"
          >
            {DESIGN_PRESETS.map((p) => (
              <option key={p.label} value={p.label}>
                {p.label}
              </option>
            ))}
          </select>
        </div>

        <div className="space-y-3">
          <div>
            <label className="text-xs font-medium text-slate-600 mb-1.5 block">
              接口地址
            </label>
            <input
              value={baseUrl}
              onChange={(e) => setBaseUrl(e.target.value)}
              placeholder="https://dashscope.aliyuncs.com/compatible-mode/v1"
              className="w-full rounded-lg border border-slate-200 bg-slate-50 px-3 py-2 text-sm text-slate-700 focus:outline-none focus:ring-2 focus:ring-[#0f766e]/30 font-mono"
            />
          </div>
          <div>
            <label className="text-xs font-medium text-slate-600 mb-1.5 block">模型名称</label>
            <input
              value={model}
              onChange={(e) => setModel(e.target.value)}
              placeholder="wanx2.1-t2i-turbo"
              className="w-full rounded-lg border border-slate-200 bg-slate-50 px-3 py-2 text-sm text-slate-700 focus:outline-none focus:ring-2 focus:ring-[#0f766e]/30 font-mono"
            />
          </div>
          <div>
            <label className="text-xs font-medium text-slate-600 mb-1.5 block">
              API Key
              {hasKey && <span className="ml-2 text-[11px] text-emerald-600">已配置</span>}
            </label>
            <input
              type="password"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              placeholder={hasKey ? "已保存，输入新 Key 可覆盖" : "sk-…"}
              className="w-full rounded-lg border border-slate-200 bg-slate-50 px-3 py-2 text-sm text-slate-700 focus:outline-none focus:ring-2 focus:ring-[#0f766e]/30 font-mono"
            />
          </div>
        </div>

        <div className="flex items-center gap-2 mt-4">
          <button
            onClick={save}
            disabled={saving}
            className="bg-[#333333] hover:bg-[#111111] text-white rounded-lg px-4 py-2 text-sm disabled:opacity-50"
          >
            {saving ? "保存中…" : "保存配置"}
          </button>
          <button
            onClick={test}
            disabled={testing}
            className="bg-slate-100 hover:bg-slate-200 text-slate-600 rounded-lg px-4 py-2 text-sm disabled:opacity-50"
          >
            {testing ? "测试中…" : "测试连接"}
          </button>
        </div>
        {testMsg && (
          <div className="text-sm text-slate-600 bg-slate-50 border border-slate-200 rounded-lg px-3 py-2 mt-3 whitespace-pre-wrap break-all">
            {testMsg}
          </div>
        )}
      </Section>

      <Section title="说明">
        <ul className="text-xs text-slate-500 space-y-1.5 list-disc pl-4">
          <li>配置生图模型时产出位图 PNG；未配置时自动用对话模型生成矢量 SVG，两者都可预览、下载。</li>
          <li>原型设计复用当前对话模型，直接生成可交互的 HTML 原型并在应用内预览。</li>
          <li>所有作品保存在本机数据目录的 design 文件夹，可随时删除。</li>
        </ul>
      </Section>
    </div>
  );
}

/* ---------------- 帮助与反馈 ---------------- */

function HelpPane({ onNotice }: { onNotice?: (level: string, text: string) => void }) {
  return (
    <div className="max-w-2xl px-8 py-8">
      <h1 className="text-xl font-semibold text-slate-800 mb-6">帮助与反馈</h1>

      <Section title="关于">
        <p className="text-sm text-slate-600">Laver 办公 · 桌面智能体</p>
        <p className="text-xs text-slate-400 mt-1">版本 v1.0.0</p>
        <p className="text-sm text-slate-600 mt-3">
          一个运行在本地的 AI 办公助手，支持多模型、技能、定时任务与团队协作。
        </p>
        <button
          onClick={() => onNotice?.("info", "当前已是最新版本（v1.0.0）")}
          className="mt-3 text-sm bg-slate-100 hover:bg-slate-200 rounded-lg px-3 py-1.5 text-slate-600"
        >
          检查更新
        </button>
      </Section>

      <Section title="问题反馈">
        <p className="text-sm text-slate-600">
          如有问题或建议，请通过邮箱反馈：
          <span className="text-[#1a7f37]"> support@openlaver.local</span>
        </p>
        <p className="text-xs text-slate-400 mt-1">请附上操作步骤与截图，以便我们定位问题。</p>
      </Section>
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="mb-5 bg-white border border-slate-200 rounded-xl p-5">
      <h2 className="text-sm font-semibold text-slate-700 mb-3">{title}</h2>
      {children}
    </div>
  );
}
