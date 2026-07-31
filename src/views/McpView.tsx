import { useEffect, useState } from "react";
import { api, McpServerStatus } from "../lib/api";

export default function McpView() {
  const [servers, setServers] = useState<McpServerStatus[]>([]);
  const [tab, setTab] = useState<"local" | "cloud">("local");
  const [showConfig, setShowConfig] = useState(false);
  const [configText, setConfigText] = useState("");
  const [saveMsg, setSaveMsg] = useState<string | null>(null);

  const refresh = () => api.listMcpServers().then(setServers);
  useEffect(() => {
    refresh();
    api.getMcpConfig().then((c) => setConfigText(JSON.stringify(c, null, 2)));
  }, []);

  const save = async () => {
    setSaveMsg(null);
    try {
      const parsed = JSON.parse(configText);
      await api.saveMcpConfig(parsed);
      setSaveMsg("✅ 已保存并重新连接");
      setTimeout(refresh, 1500);
    } catch (e: any) {
      setSaveMsg("❌ " + (e?.toString() ?? "配置无效"));
    }
  };

  const tabBtn = (id: "local" | "cloud", label: string) =>
    `flex items-center gap-1.5 rounded-md px-3 py-1.5 text-sm ${
      tab === id ? "bg-white shadow-sm text-slate-800" : "text-slate-500 hover:text-slate-700"
    }`;

  return (
    <div className="h-full overflow-y-auto">
      <div className="max-w-3xl mx-auto px-8 py-8">
        <h1 className="text-2xl font-bold text-slate-800 mb-4">MCP</h1>

        {/* 本地 / 云端 */}
        <div className="inline-flex bg-slate-100 rounded-lg p-0.5 mb-6">
          <button onClick={() => setTab("local")} className={tabBtn("local", "本地")}>
            <svg viewBox="0 0 24 24" className="w-3.5 h-3.5" fill="none" stroke="currentColor" strokeWidth="1.8"><rect x="3" y="4" width="18" height="12" rx="2" /><path d="M8 20h8M12 16v4" /></svg>
            本地
          </button>
          <button onClick={() => setTab("cloud")} className={tabBtn("cloud", "云端")}>
            <svg viewBox="0 0 24 24" className="w-3.5 h-3.5" fill="none" stroke="currentColor" strokeWidth="1.8"><path d="M17.5 19a4.5 4.5 0 000-9 6 6 0 00-11.7 1.5A4 4 0 006.5 19z" /></svg>
            云端
          </button>
        </div>

        {tab === "cloud" ? (
          <div className="border border-slate-200 rounded-xl py-20 text-center text-sm text-slate-400">
            云端 MCP 即将推出
          </div>
        ) : (
          <div className="border border-slate-200 rounded-xl overflow-hidden">
            {/* 管理头部 */}
            <div className="flex items-start justify-between gap-4 p-5 border-b border-slate-100">
              <div className="min-w-0">
                <div className="font-medium text-slate-800">MCP Servers 管理</div>
                <div className="text-xs text-slate-500 mt-1">
                  管理您已添加的 MCP 服务器，可启用、配置或添加新的工具能力。
                </div>
              </div>
              <button
                onClick={() => setShowConfig((v) => !v)}
                className="shrink-0 bg-slate-800 hover:bg-slate-700 text-white rounded-lg px-3.5 py-2 text-sm flex items-center gap-1"
              >
                + 添加
                <svg viewBox="0 0 24 24" className={`w-3.5 h-3.5 transition-transform ${showConfig ? "rotate-180" : ""}`} fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M6 9l6 6 6-6" /></svg>
              </button>
            </div>

            {/* 配置编辑 (点“添加”展开) */}
            {showConfig && (
              <div className="p-5 border-b border-slate-100 bg-slate-50/60">
                <p className="text-xs text-slate-400 mb-2">
                  以 JSON 配置：{`{ "mcpServers": { "名称": { "command": "npx", "args": ["-y","包名"], "enabled": true } } }`}
                </p>
                <textarea
                  value={configText}
                  onChange={(e) => setConfigText(e.target.value)}
                  rows={10}
                  spellCheck={false}
                  className="w-full font-mono text-xs bg-white border border-slate-200 rounded-lg p-3 text-slate-700 focus:outline-none focus:border-[#10a37f]"
                />
                <div className="flex items-center gap-3 mt-2">
                  <button
                    onClick={save}
                    className="bg-[#10a37f] hover:bg-[#0e9070] text-white rounded-lg px-4 py-2 text-sm"
                  >
                    保存并连接
                  </button>
                  {saveMsg && <span className="text-sm text-slate-600">{saveMsg}</span>}
                </div>
              </div>
            )}

            {/* 列表 / 空态 */}
            <div className="p-5">
              {servers.length === 0 ? (
                <div className="flex flex-col items-center justify-center py-12 text-center">
                  <div className="w-16 h-16 rounded-2xl border-2 border-slate-200 flex items-center justify-center text-slate-300 mb-3">
                    <svg viewBox="0 0 24 24" className="w-8 h-8" fill="none" stroke="currentColor" strokeWidth="1.6">
                      <rect x="3" y="8" width="18" height="8" rx="2" />
                      <circle cx="8" cy="12" r="1" fill="currentColor" />
                      <circle cx="12" cy="12" r="1" fill="currentColor" />
                      <circle cx="16" cy="12" r="1" fill="currentColor" />
                    </svg>
                  </div>
                  <div className="text-sm font-medium text-slate-600">MCP Servers</div>
                  <div className="text-xs text-slate-400 mt-1 max-w-xs">
                    还没有 MCP 服务器。点击右上角「添加」，通过 MCP 协议接入外部工具，自动合并进智能体的工具池。
                  </div>
                </div>
              ) : (
                <div className="space-y-2">
                  {servers.map((s) => (
                    <div key={s.name} className="bg-white border border-slate-200 rounded-lg p-3">
                      <div className="flex items-center gap-2">
                        <span className={s.connected ? "text-emerald-500" : "text-red-400"}>
                          {s.connected ? "●" : "○"}
                        </span>
                        <span className="font-medium text-slate-800">{s.name}</span>
                        <span className="text-xs text-slate-500">
                          {s.connected ? `${s.tool_count} 个工具` : "未连接"}
                        </span>
                      </div>
                      {s.tools.length > 0 && (
                        <div className="text-xs text-slate-500 mt-1 font-mono truncate">{s.tools.join(", ")}</div>
                      )}
                      {s.error && <div className="text-xs text-red-500 mt-1">{s.error}</div>}
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
