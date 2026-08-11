import { useEffect, useState } from "react";
import { api, TeammateInfo } from "../lib/api";
import { useTauriEvent, EV } from "../lib/events";

interface TeamMsg {
  from: string;
  to: string;
  content: string;
  type: string;
  ts: string;
}

const PHASE: Record<string, { label: string; color: string }> = {
  work: { label: "工作中", color: "bg-[#e0e0e0] text-[#333333]" },
  idle: { label: "空闲轮询", color: "bg-amber-50 text-amber-600" },
  shutdown: { label: "已关闭", color: "bg-slate-100 text-slate-500" },
};

export default function TeamView() {
  const [teammates, setTeammates] = useState<TeammateInfo[]>([]);
  const [messages, setMessages] = useState<TeamMsg[]>([]);
  const refresh = () => api.listTeammates().then(setTeammates);
  useEffect(() => {
    refresh();
    const t = setInterval(refresh, 3000);
    return () => clearInterval(t);
  }, []);
  useTauriEvent<TeammateInfo[]>(EV.TEAM_UPDATE, (p) => setTeammates(p));
  useTauriEvent<TeamMsg>(EV.TEAM_MESSAGE, (p) => setMessages((prev) => [...prev.slice(-100), p]));

  return (
    <div className="h-full flex">
      <div className="flex-1 overflow-y-auto px-8 py-8">
        <h1 className="text-xl font-semibold text-slate-800 mb-1">团队协作</h1>
        <p className="text-sm text-slate-500 mb-6">
          持久成员并行工作、通过信箱通信，并能自主认领任务。
        </p>
        {teammates.length === 0 ? (
          <div className="text-center text-slate-400 mt-20">
            <div className="text-4xl mb-3">👥</div>
            <p>暂无团队成员。在对话中让智能体创建协作成员。</p>
          </div>
        ) : (
          <div className="grid grid-cols-2 gap-4">
            {teammates.map((tm) => (
              <div key={tm.name} className="bg-white border border-slate-200 rounded-xl p-4 shadow-sm">
                <div className="flex items-center justify-between">
                  <span className="font-semibold text-slate-800">🤝 {tm.name}</span>
                  <span className={`text-[10px] px-2 py-0.5 rounded ${PHASE[tm.phase]?.color ?? ""}`}>
                    {PHASE[tm.phase]?.label ?? tm.phase}
                  </span>
                </div>
                <p className="text-xs text-slate-500 mt-2">{tm.role}</p>
              </div>
            ))}
          </div>
        )}
      </div>

      <aside className="w-80 shrink-0 border-l border-slate-200 p-4 overflow-y-auto bg-[#fafbfc]">
        <h3 className="text-xs font-semibold text-slate-500 uppercase mb-3">信箱消息</h3>
        <div className="space-y-2">
          {messages.length === 0 && <p className="text-xs text-slate-400">尚无团队消息</p>}
          {messages.map((m, i) => (
            <div key={i} className="text-xs bg-white border border-slate-200 rounded-lg p-2">
              <div className="flex items-center gap-1 text-slate-500">
                <span className="text-[#1a7f37]">{m.from}</span>→
                <span className="text-[#666666]">{m.to}</span>
                <span className="ml-auto text-[9px] px-1.5 rounded bg-slate-100 text-slate-500">{m.type}</span>
              </div>
              <div className="text-slate-700 mt-1">{m.content}</div>
            </div>
          ))}
        </div>
      </aside>
    </div>
  );
}
