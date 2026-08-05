import { useEffect, useState } from "react";
import { api, Task } from "../lib/api";
import { useTauriEvent, EV } from "../lib/events";

const COLUMNS = [
  { status: "pending", label: "待处理", color: "text-slate-500" },
  { status: "in_progress", label: "进行中", color: "text-[#8b5cf6]" },
  { status: "completed", label: "已完成", color: "text-violet-600" },
];

export default function TaskBoard() {
  const [tasks, setTasks] = useState<Task[]>([]);
  const refresh = () => api.listTasks().then(setTasks);
  useEffect(() => {
    refresh();
    const t = setInterval(refresh, 4000);
    return () => clearInterval(t);
  }, []);
  useTauriEvent<Task[]>(EV.TASK_UPDATE, (p) => setTasks(p));

  return (
    <div className="h-full overflow-y-auto px-8 py-8">
      <h1 className="text-xl font-semibold text-slate-800 mb-1">任务看板</h1>
      <p className="text-sm text-slate-500 mb-6">
        可持久化、按依赖驱动的任务，团队成员可自主认领。
      </p>
      {tasks.length === 0 ? (
        <Empty />
      ) : (
        <div className="grid grid-cols-3 gap-4">
          {COLUMNS.map((col) => (
            <div key={col.status} className="bg-[#f7f8fa] border border-slate-200 rounded-xl p-3">
              <h3 className={`text-xs font-semibold uppercase mb-3 ${col.color}`}>
                {col.label} ({tasks.filter((t) => t.status === col.status).length})
              </h3>
              <div className="space-y-2">
                {tasks
                  .filter((t) => t.status === col.status)
                  .map((t) => (
                    <div key={t.id} className="bg-white rounded-lg p-3 text-sm border border-slate-200 shadow-sm">
                      <div className="text-slate-800 font-medium">{t.subject}</div>
                      {t.description && (
                        <div className="text-xs text-slate-500 mt-1 line-clamp-2">{t.description}</div>
                      )}
                      <div className="flex items-center gap-2 mt-2 text-[10px] text-slate-400">
                        <span className="font-mono">{t.id.slice(0, 10)}</span>
                        {t.owner && <span className="text-[#8b5cf6]">@{t.owner}</span>}
                        {t.worktree && <span className="text-purple-500">⑂{t.worktree}</span>}
                      </div>
                      {t.blocked_by.length > 0 && (
                        <div className="text-[10px] text-amber-600 mt-1">
                          依赖: {t.blocked_by.map((b) => b.slice(0, 8)).join(", ")}
                        </div>
                      )}
                    </div>
                  ))}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function Empty() {
  return (
    <div className="text-center text-slate-400 mt-20">
      <div className="text-4xl mb-3">🗂</div>
      <p>暂无任务。在对话中让智能体拆解并创建带依赖的任务。</p>
    </div>
  );
}
