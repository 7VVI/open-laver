import { useEffect, useState } from "react";
import { api, MemoryItem } from "../lib/api";

const TYPE_COLOR: Record<string, string> = {
  user: "bg-[#f0e9ff] text-[#8b5cf6]",
  feedback: "bg-amber-50 text-amber-600",
  project: "bg-violet-50 text-violet-600",
  reference: "bg-slate-100 text-slate-500",
};

export default function MemoryView() {
  const [memories, setMemories] = useState<MemoryItem[]>([]);
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
          跨会话的长期记忆。智能体会自动提取用户偏好与项目约定并按需回忆。
        </p>
        {memories.length === 0 ? (
          <div className="text-center text-slate-400 mt-20">
            <div className="text-4xl mb-3">🧠</div>
            <p>暂无记忆。对话中提到偏好或约定时会被自动记住。</p>
          </div>
        ) : (
          <div className="space-y-3">
            {memories.map((m) => (
              <div key={m.name} className="bg-white border border-slate-200 rounded-xl p-4 shadow-sm">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <span className="font-medium text-slate-800">{m.name}</span>
                    <span className={`text-[10px] px-2 py-0.5 rounded ${TYPE_COLOR[m.mtype] ?? TYPE_COLOR.reference}`}>
                      {m.mtype}
                    </span>
                  </div>
                  <button onClick={() => del(m.name)} className="text-xs text-slate-400 hover:text-red-500">
                    删除
                  </button>
                </div>
                {m.description && <div className="text-sm text-slate-500 mt-1">{m.description}</div>}
                <div className="text-xs text-slate-500 mt-2 whitespace-pre-wrap">{m.content}</div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
