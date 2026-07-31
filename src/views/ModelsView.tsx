import { useEffect, useState } from "react";
import { api, ModelProfileDto, THINKING_LABELS, formatContext } from "../lib/api";
import ModelEditor from "./ModelEditor";

export default function ModelsView({ onChanged }: { onChanged?: () => void }) {
  const [models, setModels] = useState<ModelProfileDto[]>([]);
  const [editing, setEditing] = useState<ModelProfileDto | null>(null);
  const [adding, setAdding] = useState(false);
  const [testMsg, setTestMsg] = useState<string | null>(null);
  const [testing, setTesting] = useState(false);

  const refresh = async () => {
    setModels(await api.listModels());
    onChanged?.();
  };
  useEffect(() => {
    refresh();
  }, []);

  const activate = async (id: string) => {
    await api.setActiveModel(id);
    refresh();
  };
  const del = async (id: string) => {
    await api.deleteModel(id);
    refresh();
  };
  const test = async () => {
    setTesting(true);
    setTestMsg(null);
    try {
      setTestMsg("✅ " + (await api.testConnection()));
    } catch (e: any) {
      setTestMsg("❌ " + (e?.toString() ?? "失败"));
    }
    setTesting(false);
  };

  return (
    <div className="h-full overflow-y-auto">
      <div className="max-w-3xl mx-auto px-8 py-8">
        <div className="flex items-center justify-between mb-1">
          <h1 className="text-xl font-semibold text-slate-800">模型管理</h1>
          <div className="flex items-center gap-2">
            <button
              onClick={test}
              disabled={testing}
              className="text-sm bg-slate-100 hover:bg-slate-200 rounded-lg px-3 py-1.5 text-slate-600 disabled:opacity-50"
            >
              {testing ? "测试中…" : "测试当前模型"}
            </button>
            <button
              onClick={() => setAdding(true)}
              className="text-sm bg-[#10a37f] hover:bg-[#0e9070] text-white rounded-lg px-3 py-1.5"
            >
              + 添加模型
            </button>
          </div>
        </div>
        <p className="text-sm text-slate-500 mb-2">
          管理多个模型配置，切换当前使用的模型；密钥安全存储于系统凭据管理器。
        </p>
        {testMsg && (
          <div className="text-sm text-slate-600 bg-slate-50 border border-slate-200 rounded-lg px-3 py-2 mb-4">
            {testMsg}
          </div>
        )}

        <div className="space-y-3 mt-4">
          {models.length === 0 && (
            <div className="text-center text-slate-400 py-16">
              还没有模型，点击右上角「添加模型」开始。
            </div>
          )}
          {models.map((m) => (
            <div
              key={m.id}
              className={`rounded-xl border p-4 ${
                m.active ? "border-[#10a37f] bg-[#f2fbf7]" : "border-slate-200 bg-white"
              }`}
            >
              <div className="flex items-start justify-between gap-3">
                <div className="min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="font-medium text-slate-800">{m.name}</span>
                    {m.active && (
                      <span className="text-[10px] bg-[#10a37f] text-white rounded px-1.5 py-0.5">
                        当前使用
                      </span>
                    )}
                    {!m.has_key && (
                      <span className="text-[10px] bg-amber-100 text-amber-700 rounded px-1.5 py-0.5">
                        未配密钥
                      </span>
                    )}
                  </div>
                  <div className="text-xs text-slate-500 mt-1 font-mono">
                    {m.model} · {formatContext(m.context_window)} 上下文
                    {m.supports_thinking && m.thinking !== "off"
                      ? ` · ${THINKING_LABELS[m.thinking]}`
                      : ""}
                  </div>
                  <div className="text-[11px] text-slate-400 mt-0.5 truncate">{m.base_url}</div>
                </div>
                <div className="flex items-center gap-1.5 shrink-0">
                  {!m.active && (
                    <button
                      onClick={() => activate(m.id)}
                      className="text-xs bg-slate-100 hover:bg-slate-200 rounded-lg px-2.5 py-1.5 text-slate-600"
                    >
                      设为当前
                    </button>
                  )}
                  <button
                    onClick={() => setEditing(m)}
                    className="text-xs text-slate-500 hover:text-slate-800 px-2 py-1.5"
                  >
                    编辑
                  </button>
                  <button
                    onClick={() => del(m.id)}
                    className="text-xs text-slate-400 hover:text-red-500 px-2 py-1.5"
                  >
                    删除
                  </button>
                </div>
              </div>
            </div>
          ))}
        </div>
      </div>

      {adding && (
        <ModelEditor
          onClose={() => setAdding(false)}
          onSaved={() => {
            setAdding(false);
            refresh();
          }}
        />
      )}
      {editing && (
        <ModelEditor
          existing={editing}
          onClose={() => setEditing(null)}
          onSaved={() => {
            setEditing(null);
            refresh();
          }}
        />
      )}
    </div>
  );
}
