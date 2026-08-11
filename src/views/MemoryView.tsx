import { useEffect, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { api, MemoryItem } from "../lib/api";

const TYPE_COLOR: Record<string, string> = {
  user: "bg-[#e0e0e0] text-[#333333]",
  feedback: "bg-amber-50 text-amber-600",
  project: "bg-green-50 text-green-600",
  reference: "bg-slate-100 text-slate-500",
};

export default function MemoryView() {
  const [memories, setMemories] = useState<MemoryItem[]>([]);
  const [viewing, setViewing] = useState<MemoryItem | null>(null);

  const refresh = () => api.listMemories().then(setMemories);
  useEffect(() => {
    refresh();
    const t = setInterval(refresh, 5000);
    return () => clearInterval(t);
  }, []);

  const del = async (name: string) => {
    await api.deleteMemory(name);
    refresh();
  };

  return (
    <div className="h-full overflow-y-auto">
      <div className="max-w-3xl mx-auto px-8 py-8">
        <h1 className="text-xl font-semibold text-slate-800 mb-1">记忆库</h1>
        <p className="text-sm text-slate-500 mb-6">
          跨会话的长期记忆。智能体会自动提取用户偏好与项目约定并按需回忆。点击任意记忆可查看完整内容。
        </p>
        {memories.length === 0 ? (
          <div className="text-center text-slate-400 mt-20">
            <div className="text-4xl mb-3">🧠</div>
            <p>暂无记忆。对话中提到偏好或约定时会被自动记住。</p>
          </div>
        ) : (
          <div className="space-y-3">
            {memories.map((m) => (
              <div
                key={m.name}
                onClick={() => setViewing(m)}
                className="bg-white border border-slate-200 rounded-xl p-4 shadow-sm cursor-pointer hover:border-[#34c759]/50 hover:shadow transition-all"
              >
                <div className="flex items-center justify-between gap-3">
                  <div className="flex items-center gap-2 min-w-0">
                    <span className="font-medium text-slate-800 truncate">{m.name}</span>
                    <span className={`text-[10px] px-2 py-0.5 rounded shrink-0 ${TYPE_COLOR[m.mtype] ?? TYPE_COLOR.reference}`}>
                      {m.mtype}
                    </span>
                  </div>
                  <div className="flex items-center gap-3 shrink-0" onClick={(e) => e.stopPropagation()}>
                    <span className="text-xs text-slate-400">查看</span>
                    <button onClick={() => del(m.name)} className="text-xs text-slate-400 hover:text-red-500">
                      删除
                    </button>
                  </div>
                </div>
                {m.description && <div className="text-sm text-slate-500 mt-1">{m.description}</div>}
                <div className="text-xs text-slate-400 mt-2 line-clamp-2 leading-relaxed">{m.content}</div>
              </div>
            ))}
          </div>
        )}
      </div>

      {viewing && <MemoryDetail memory={viewing} onClose={() => setViewing(null)} />}
    </div>
  );
}

/* ---------------- 记忆详情弹窗 (Markdown 预览) ---------------- */

function MemoryDetail({ memory, onClose }: { memory: MemoryItem; onClose: () => void }) {
  return (
    <div className="fixed inset-0 bg-black/30 flex items-center justify-center z-50 fade-in" onClick={onClose}>
      <div
        className="bg-white border border-slate-200 rounded-2xl w-[720px] max-w-[92vw] max-h-[85vh] shadow-2xl flex flex-col"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="px-6 py-4 border-b border-slate-100 flex items-start justify-between gap-4">
          <div className="flex items-start gap-3 min-w-0">
            <div className="w-10 h-10 rounded-lg bg-slate-100 flex items-center justify-center text-xl shrink-0">
              🧠
            </div>
            <div className="min-w-0">
              <div className="font-semibold text-slate-800 flex items-center gap-2">
                {memory.name}
                <span className={`text-[11px] px-1.5 py-0.5 rounded font-normal ${TYPE_COLOR[memory.mtype] ?? TYPE_COLOR.reference}`}>
                  {memory.mtype}
                </span>
              </div>
              {memory.description && (
                <div className="text-sm text-slate-500 mt-0.5">{memory.description}</div>
              )}
            </div>
          </div>
          <button onClick={onClose} className="text-slate-400 hover:text-slate-600 shrink-0">✕</button>
        </div>

        <div className="flex-1 overflow-y-auto px-6 py-4">
          <div className="flex items-center gap-2 bg-blue-50 text-blue-600 text-xs rounded-lg px-3 py-2 mb-4">
            <svg viewBox="0 0 24 24" className="w-3.5 h-3.5 shrink-0" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <circle cx="12" cy="12" r="10" />
              <path d="M12 16v-4M12 8h.01" />
            </svg>
            以下内容来自该记忆的 Markdown 原文
          </div>
          <div className="md text-sm text-slate-700 leading-relaxed border border-slate-100 rounded-xl px-5 py-4">
            <ReactMarkdown remarkPlugins={[remarkGfm]}>{memory.content}</ReactMarkdown>
          </div>
        </div>

        <div className="px-6 py-3 border-t border-slate-100 flex justify-end">
          <button onClick={onClose} className="px-4 py-2 rounded-lg text-sm bg-slate-100 hover:bg-slate-200 text-slate-600 shrink-0">
            关闭
          </button>
        </div>
      </div>
    </div>
  );
}
