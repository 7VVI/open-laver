import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api } from "../lib/api";
import ModelsView from "./ModelsView";
import MemoryView from "./MemoryView";

type Cat = "system" | "models" | "memory" | "help";

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
        <aside className="w-52 shrink-0 bg-[#f7f8fa] border-r border-slate-200 py-3 overflow-y-auto">
          <div className="px-4 pb-2 text-[13px] font-semibold text-slate-700">设置</div>
          {CATS.map((c) => (
            <button
              key={c.id}
              onClick={() => setCat(c.id)}
              className={`w-full flex items-center gap-3 px-4 py-2 text-sm transition ${
                cat === c.id
                  ? "bg-[#e8f5ee] text-[#10a37f] font-medium"
                  : "text-slate-600 hover:bg-slate-100"
              }`}
            >
              <span className={cat === c.id ? "text-[#10a37f]" : "text-slate-400"}>{c.icon}</span>
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
  useEffect(() => {
    api.getWorkspace().then((w) => setWorkspace(w.workspace));
  }, []);

  const pick = async () => {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir === "string") {
      await api.setWorkspace(dir);
      setWorkspace(dir);
      onNotice?.("info", "已更新工作目录");
    }
  };

  return (
    <div className="max-w-2xl px-8 py-8">
      <h1 className="text-xl font-semibold text-slate-800 mb-6">系统设置</h1>
      <Section title="工作目录">
        <p className="text-xs text-slate-500 mb-2">
          智能体的文件操作默认限定在此目录内，目录之外的写操作会触发权限确认。
        </p>
        <div className="flex gap-2">
          <input
            readOnly
            value={workspace}
            className="flex-1 bg-slate-50 border border-slate-300 rounded-lg px-3 py-2 text-sm text-slate-700"
          />
          <button
            onClick={pick}
            className="bg-[#10a37f] hover:bg-[#0e9070] text-white rounded-lg px-4 text-sm"
          >
            选择
          </button>
        </div>
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
        <p className="text-xs text-slate-400 mt-1">版本 v0.1.0</p>
        <p className="text-sm text-slate-600 mt-3">
          一个运行在本地的 AI 办公助手，支持多模型、技能、定时任务与团队协作。
        </p>
        <button
          onClick={() => onNotice?.("info", "当前已是最新版本（v0.1.0）")}
          className="mt-3 text-sm bg-slate-100 hover:bg-slate-200 rounded-lg px-3 py-1.5 text-slate-600"
        >
          检查更新
        </button>
      </Section>

      <Section title="问题反馈">
        <p className="text-sm text-slate-600">
          如有问题或建议，请通过邮箱反馈：
          <span className="text-[#10a37f]"> support@openlaver.local</span>
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
