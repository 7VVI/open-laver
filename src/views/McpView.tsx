import { useEffect, useState } from "react";
import { api, McpServerStatus } from "../lib/api";

export default function McpView() {
  const [servers, setServers] = useState<McpServerStatus[]>([]);
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

  return (
    <div className="h-full overflow-y-auto">
      <div className="max-w-3xl mx-auto px-8 py-8">
        <h1 className="text-xl font-semibold text-slate-800 mb-1">连接器 (MCP)</h1>
        <p className="text-sm text-slate-500 mb-6">
          通过 MCP 协议接入外部工具，自动合并进智能体的工具池。
        </p>

        <div className="mb-6">
          <h3 className="text-xs font-semibold text-slate-500 uppercase mb-3">连接状态</h3>
          {servers.length === 0 ? (
            <p className="text-sm text-slate-400">尚未配置连接器</p>
          ) : (
            <div className="space-y-2">
              {servers.map((s) => (
                <div key={s.name} className="bg-white border border-slate-200 rounded-lg p-3 shadow-sm">
                  <div className="flex items-center gap-2">
                    <span className={s.connected ? "text-emerald-500" : "text-red-500"}>
                      {s.connected ? "●" : "○"}
                    </span>
                    <span className="font-medium text-slate-800">{s.name}</span>
                    <span className="text-xs text-slate-500">
                      {s.connected ? `${s.tool_count} 个工具` : "未连接"}
                    </span>
                  </div>
                  {s.tools.length > 0 && (
                    <div className="text-xs text-slate-500 mt-1 font-mono">{s.tools.join(", ")}</div>
                  )}
                  {s.error && <div className="text-xs text-red-500 mt-1">{s.error}</div>}
                </div>
              ))}
            </div>
          )}
        </div>

        <div>
          <h3 className="text-xs font-semibold text-slate-500 uppercase mb-2">配置</h3>
          <p className="text-xs text-slate-400 mb-2">
            格式: {`{ "mcpServers": { "名称": { "command": "npx", "args": [...], "enabled": true } } }`}
          </p>
          <textarea
            value={configText}
            onChange={(e) => setConfigText(e.target.value)}
            rows={12}
            spellCheck={false}
            className="w-full font-mono text-xs bg-slate-50 border border-slate-200 rounded-lg p-3 text-slate-700 focus:outline-none focus:border-[#10a37f]"
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
      </div>
    </div>
  );
}
