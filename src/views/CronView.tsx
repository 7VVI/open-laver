import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, CronJob, CronRun } from "../lib/api";

/* ---------------- 频率 / cron 工具 ---------------- */

type Freq = "hourly" | "daily" | "weekday" | "mon" | "tue" | "wed" | "thu" | "fri" | "sat" | "sun" | "monthly";

const FREQ_OPTIONS: { id: Freq; label: string }[] = [
  { id: "hourly", label: "每小时" },
  { id: "daily", label: "每天" },
  { id: "weekday", label: "工作日" },
  { id: "mon", label: "每周一" },
  { id: "tue", label: "每周二" },
  { id: "wed", label: "每周三" },
  { id: "thu", label: "每周四" },
  { id: "fri", label: "每周五" },
  { id: "sat", label: "每周六" },
  { id: "sun", label: "每周日" },
  { id: "monthly", label: "每月 1 号" },
];
const DOW: Record<string, string> = { mon: "1", tue: "2", wed: "3", thu: "4", fri: "5", sat: "6", sun: "0" };

function buildCron(freq: Freq, time: string): string {
  const [hh, mm] = (time || "09:00").split(":");
  const m = parseInt(mm || "0", 10);
  const h = parseInt(hh || "9", 10);
  switch (freq) {
    case "hourly": return `${m} * * * *`;
    case "daily": return `${m} ${h} * * *`;
    case "weekday": return `${m} ${h} * * 1-5`;
    case "monthly": return `${m} ${h} 1 * *`;
    default: return `${m} ${h} * * ${DOW[freq] ?? "*"}`;
  }
}

function describeCron(expr: string): string {
  const p = expr.trim().split(/\s+/);
  if (p.length < 5) return expr;
  const [mi, ho, dom, , dow] = p;
  const t = (h: string, m: string) => `${h.padStart(2, "0")}:${m.padStart(2, "0")}`;
  if (ho === "*") return "每小时";
  if (dow === "1-5") return `工作日 ${t(ho, mi)}`;
  if (dom === "1") return `每月 1 号 ${t(ho, mi)}`;
  if (dow && dow !== "*") {
    const names: Record<string, string> = { "0": "周日", "1": "周一", "2": "周二", "3": "周三", "4": "周四", "5": "周五", "6": "周六" };
    return `每${names[dow] ?? "周"} ${t(ho, mi)}`;
  }
  return `每天 ${t(ho, mi)}`;
}

/* ---------------- 推荐案例 ---------------- */

const EXAMPLES: { title: string; prompt: string; freq: Freq; time: string; scheduleLabel: string }[] = [
  {
    title: "品牌舆情监控",
    prompt: "搜索【品牌名】品牌近 24 小时的舆情，汇总关键正负面信息并输出一份简报。",
    freq: "daily", time: "14:00", scheduleLabel: "每日下午2点",
  },
  {
    title: "每日电影推荐",
    prompt: "给我推荐一部公认的经典电影（评分较高），附简介与推荐理由。",
    freq: "daily", time: "20:00", scheduleLabel: "每日晚8点",
  },
  {
    title: "金融股票监控",
    prompt: "帮我查询【英伟达股票】今日的走势与关键消息，并给出简要分析。",
    freq: "weekday", time: "08:00", scheduleLabel: "工作日早8点",
  },
];

/* ---------------- 主视图 ---------------- */

export default function CronView({
  sessionId,
  ensureSession,
  onNotice,
}: {
  sessionId: string | null;
  ensureSession: () => Promise<string>;
  onNotice?: (level: string, text: string) => void;
}) {
  const [jobs, setJobs] = useState<CronJob[]>([]);
  const [runs, setRuns] = useState<CronRun[]>([]);
  const [tab, setTab] = useState<"jobs" | "runs">("jobs");
  const [query, setQuery] = useState("");
  const [keepAwake, setKeepAwake] = useState(false);
  const [create, setCreate] = useState<null | { title: string; prompt: string; freq: Freq; time: string }>(null);

  const refresh = () => {
    api.listCronJobs().then(setJobs);
    api.listCronRuns().then(setRuns);
  };
  useEffect(() => {
    refresh();
    const t = setInterval(refresh, 5000);
    return () => clearInterval(t);
  }, []);

  const filtered = query.trim()
    ? jobs.filter((j) => (j.title + j.prompt).toLowerCase().includes(query.trim().toLowerCase()))
    : jobs;

  const del = async (id: string) => {
    await api.cancelCronJob(id);
    refresh();
  };
  const runNow = async (id: string) => {
    try {
      await api.runCronNow(id);
      onNotice?.("info", "已手动触发定时任务");
      setTimeout(refresh, 800);
    } catch (e: any) {
      onNotice?.("error", "触发失败：" + (e?.toString() ?? ""));
    }
  };

  const openCreate = (init?: { title: string; prompt: string; freq: Freq; time: string }) =>
    setCreate(init ?? { title: "", prompt: "", freq: "daily", time: "09:00" });

  return (
    <div className="h-full overflow-y-auto">
      <div className="max-w-4xl mx-auto px-8 py-8">
        {/* 头部 */}
        <div className="flex items-start justify-between gap-4">
          <div>
            <h1 className="text-2xl font-bold text-slate-800">定时任务</h1>
            <p className="text-sm text-slate-500 mt-1">
              按计划自动执行任务，也可随时手动触发。在任意对话中描述你想定期做的事，即可快速创建。
            </p>
          </div>
          <div className="flex items-center gap-2 shrink-0">
            <button onClick={refresh} className="w-9 h-9 rounded-lg flex items-center justify-center text-slate-500 hover:bg-slate-100" title="刷新">
              <svg viewBox="0 0 24 24" className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
                <path d="M21 2v6h-6M3 12a9 9 0 0115-6.7L21 8M3 22v-6h6M21 12a9 9 0 01-15 6.7L3 16" />
              </svg>
            </button>
            <button
              onClick={() => openCreate()}
              className="text-sm bg-slate-800 hover:bg-slate-700 text-white rounded-lg px-3.5 py-2 flex items-center gap-1"
            >
              + 新建定时任务
            </button>
          </div>
        </div>

        {/* 搜索 */}
        <div className="mt-5 relative max-w-md">
          <svg viewBox="0 0 24 24" className="w-4 h-4 text-slate-400 absolute left-3 top-1/2 -translate-y-1/2" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round">
            <circle cx="11" cy="11" r="7" /><path d="M21 21l-4.3-4.3" />
          </svg>
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="搜索定时任务..."
            className="w-full bg-white border border-slate-300 rounded-lg pl-9 pr-3 py-2 text-sm text-slate-700 focus:outline-none focus:border-[#10a37f]"
          />
        </div>

        {/* 提示条 */}
        <div className="mt-4 flex items-center justify-between bg-blue-50 border border-blue-100 rounded-lg px-4 py-2.5 text-sm">
          <span className="flex items-center gap-2 text-blue-700">
            <svg viewBox="0 0 24 24" className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth="1.8"><path d="M12 16v-4M12 8h.01" /><circle cx="12" cy="12" r="9" /></svg>
            定时任务仅在电脑保持唤醒时运行
          </span>
          <label className="flex items-center gap-2 cursor-pointer text-slate-600">
            保持系统唤醒
            <span
              onClick={() => { setKeepAwake(!keepAwake); onNotice?.("info", keepAwake ? "已关闭保持唤醒" : "已开启保持唤醒（需系统电源设置配合）"); }}
              className={`w-9 h-5 rounded-full relative transition ${keepAwake ? "bg-[#10a37f]" : "bg-slate-300"}`}
            >
              <span className={`absolute top-0.5 w-4 h-4 bg-white rounded-full transition-all ${keepAwake ? "left-4.5" : "left-0.5"}`} style={{ left: keepAwake ? "18px" : "2px" }} />
            </span>
          </label>
        </div>

        {/* 推荐案例 */}
        <div className="mt-6">
          <div className="text-sm font-semibold text-slate-700 mb-3">推荐案例</div>
          <div className="grid grid-cols-3 gap-4">
            {EXAMPLES.map((ex) => (
              <button
                key={ex.title}
                onClick={() => openCreate({ title: ex.title, prompt: ex.prompt, freq: ex.freq, time: ex.time })}
                className="text-left bg-white border border-slate-200 rounded-xl p-4 hover:border-[#10a37f] hover:shadow-sm transition"
              >
                <div className="font-medium text-slate-800">{ex.title}</div>
                <div className="text-xs text-slate-500 mt-1.5 line-clamp-2 leading-relaxed">{ex.prompt}</div>
                <div className="mt-3 inline-flex items-center gap-1 text-xs text-slate-500 bg-slate-100 rounded px-2 py-1">
                  <svg viewBox="0 0 24 24" className="w-3 h-3" fill="none" stroke="currentColor" strokeWidth="1.8"><path d="M12 8v4l3 2" /><circle cx="12" cy="12" r="9" /></svg>
                  {ex.scheduleLabel}
                </div>
              </button>
            ))}
          </div>
        </div>

        {/* 我的定时任务 / 执行记录 */}
        <div className="mt-8 flex items-center gap-5 border-b border-slate-200">
          <button
            onClick={() => setTab("jobs")}
            className={`pb-2 text-sm font-medium border-b-2 -mb-px ${tab === "jobs" ? "border-[#10a37f] text-slate-800" : "border-transparent text-slate-400 hover:text-slate-600"}`}
          >
            我的定时任务
          </button>
          <button
            onClick={() => setTab("runs")}
            className={`pb-2 text-sm font-medium border-b-2 -mb-px ${tab === "runs" ? "border-[#10a37f] text-slate-800" : "border-transparent text-slate-400 hover:text-slate-600"}`}
          >
            执行记录
          </button>
        </div>

        {tab === "jobs" ? (
          filtered.length === 0 ? (
            <div className="border border-dashed border-slate-200 rounded-xl py-16 mt-6 text-center">
              <div className="w-12 h-12 rounded-full bg-slate-100 mx-auto flex items-center justify-center text-slate-400 mb-3">
                <svg viewBox="0 0 24 24" className="w-6 h-6" fill="none" stroke="currentColor" strokeWidth="1.6"><path d="M12 8v4l3 2" /><circle cx="12" cy="12" r="9" /></svg>
              </div>
              <div className="text-slate-600">暂无定时任务</div>
              <div className="text-xs text-slate-400 mt-1">创建定时任务来自动化执行周期性 AI 代理工作</div>
              <button onClick={() => openCreate()} className="mt-4 text-sm border border-slate-300 rounded-lg px-3.5 py-2 text-slate-600 hover:bg-slate-50">
                + 新建定时任务
              </button>
            </div>
          ) : (
            <div className="space-y-3 mt-6">
              {filtered.map((j) => (
                <div key={j.id} className="bg-white border border-slate-200 rounded-xl p-4 flex items-start justify-between gap-4 shadow-sm">
                  <div className="min-w-0">
                    <div className="font-medium text-slate-800">{j.title || describeCron(j.expr)}</div>
                    <div className="text-sm text-slate-500 mt-1 line-clamp-2">{j.prompt}</div>
                    <div className="flex items-center gap-2 mt-2">
                      <span className="inline-flex items-center gap-1 text-xs text-[#10a37f] bg-[#e8f5ee] rounded px-2 py-0.5">
                        <svg viewBox="0 0 24 24" className="w-3 h-3" fill="none" stroke="currentColor" strokeWidth="1.8"><path d="M12 8v4l3 2" /><circle cx="12" cy="12" r="9" /></svg>
                        {describeCron(j.expr)}
                      </span>
                      {j.recurring && <span className="text-[10px] text-slate-400">重复</span>}
                    </div>
                  </div>
                  <div className="flex items-center gap-1.5 shrink-0">
                    <button onClick={() => runNow(j.id)} className="text-xs bg-slate-100 hover:bg-slate-200 rounded-lg px-2.5 py-1.5 text-slate-600">
                      立即运行
                    </button>
                    <button onClick={() => del(j.id)} className="text-slate-400 hover:text-red-500 p-1.5" title="删除">
                      <svg viewBox="0 0 24 24" className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
                        <path d="M3 6h18M8 6V4a2 2 0 012-2h4a2 2 0 012 2v2m2 0v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6" />
                      </svg>
                    </button>
                  </div>
                </div>
              ))}
            </div>
          )
        ) : (
          <div className="mt-6">
            {runs.length === 0 ? (
              <div className="text-center text-slate-400 py-16">暂无执行记录</div>
            ) : (
              <div className="space-y-2">
                {runs.map((r, i) => (
                  <div key={i} className="bg-white border border-slate-200 rounded-lg px-4 py-3 flex items-center justify-between gap-4">
                    <div className="min-w-0">
                      <div className="text-sm text-slate-700 truncate">{r.title || r.prompt}</div>
                      <div className="text-xs text-slate-400 mt-0.5">{new Date(r.ran_at).toLocaleString("zh-CN")}</div>
                    </div>
                    <span className={`text-[10px] px-2 py-0.5 rounded shrink-0 ${r.trigger === "manual" ? "bg-amber-50 text-amber-600" : "bg-[#e8f5ee] text-[#10a37f]"}`}>
                      {r.trigger === "manual" ? "手动" : "定时"}
                    </span>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}
      </div>

      {create && (
        <CronCreateDialog
          init={create}
          onClose={() => setCreate(null)}
          onSaved={async (title, freq, time, prompt) => {
            const sid = sessionId ?? (await ensureSession());
            try {
              await api.createCronJob(sid, title, buildCron(freq, time), prompt, true);
              onNotice?.("info", "已创建定时任务");
              setCreate(null);
              refresh();
            } catch (e: any) {
              onNotice?.("error", "创建失败：" + (e?.toString() ?? ""));
            }
          }}
        />
      )}
    </div>
  );
}

/* ---------------- 新建定时任务弹窗 ---------------- */

function CronCreateDialog({
  init,
  onClose,
  onSaved,
}: {
  init: { title: string; prompt: string; freq: Freq; time: string };
  onClose: () => void;
  onSaved: (title: string, freq: Freq, time: string, prompt: string) => void;
}) {
  const [title, setTitle] = useState(init.title);
  const [freq, setFreq] = useState<Freq>(init.freq);
  const [time, setTime] = useState(init.time);
  const [prompt, setPrompt] = useState(init.prompt);
  const [saving, setSaving] = useState(false);

  const save = () => {
    if (!prompt.trim()) return;
    setSaving(true);
    onSaved(title.trim() || prompt.trim().slice(0, 20), freq, time, prompt.trim());
  };

  const pickWs = async () => {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir === "string") await api.setWorkspace(dir);
  };

  const field = "bg-white border border-slate-300 rounded-lg px-3 py-2 text-sm text-slate-800 focus:outline-none focus:border-[#10a37f]";

  return (
    <div className="fixed inset-0 bg-black/30 flex items-center justify-center z-50 fade-in" onClick={onClose}>
      <div className="bg-white rounded-2xl shadow-2xl w-[520px] max-w-[92vw]" onClick={(e) => e.stopPropagation()}>
        <div className="px-5 pt-5 pb-3 flex items-start justify-between">
          <div>
            <h2 className="font-semibold text-slate-800">新建定时任务</h2>
            <p className="text-xs text-slate-500 mt-1">按计划自动执行任务，也可随时手动触发。</p>
          </div>
          <button onClick={onClose} className="text-slate-400 hover:text-slate-600">✕</button>
        </div>

        <div className="px-5 pb-2 space-y-4">
          <div>
            <label className="block text-sm text-slate-700 mb-1.5">任务名称</label>
            <input value={title} onChange={(e) => setTitle(e.target.value)} placeholder="描述你的任务" className={`w-full ${field}`} />
          </div>

          <div>
            <label className="block text-sm text-slate-700 mb-1.5">计划时间</label>
            <div className="flex gap-2">
              <select value={freq} onChange={(e) => setFreq(e.target.value as Freq)} className={`${field} w-32`}>
                {FREQ_OPTIONS.map((f) => (
                  <option key={f.id} value={f.id}>{f.label}</option>
                ))}
              </select>
              <input
                type="time"
                value={time}
                onChange={(e) => setTime(e.target.value)}
                disabled={freq === "hourly"}
                className={`${field} flex-1 disabled:opacity-50`}
              />
            </div>
          </div>

          <div>
            <label className="block text-sm text-slate-700 mb-1.5">让智能体帮你做什么…</label>
            <textarea
              value={prompt}
              onChange={(e) => setPrompt(e.target.value)}
              rows={6}
              placeholder="让智能体帮你做什么…"
              className={`w-full ${field} leading-relaxed`}
            />
          </div>
        </div>

        <div className="px-5 py-3 flex items-center justify-between border-t border-slate-100">
          <button onClick={pickWs} className="flex items-center gap-1.5 text-xs text-slate-500 hover:text-slate-700">
            <svg viewBox="0 0 24 24" className="w-3.5 h-3.5" fill="none" stroke="currentColor" strokeWidth="1.8"><path d="M3 7a2 2 0 012-2h4l2 2h8a2 2 0 012 2v8a2 2 0 01-2 2H5a2 2 0 01-2-2z" /></svg>
            选择工作目录
          </button>
          <div className="flex gap-2">
            <button onClick={onClose} className="px-4 py-2 rounded-lg text-sm bg-slate-100 hover:bg-slate-200 text-slate-600">取消</button>
            <button
              onClick={save}
              disabled={saving || !prompt.trim()}
              className="px-5 py-2 rounded-lg text-sm bg-slate-800 hover:bg-slate-700 text-white disabled:opacity-40"
            >
              保存
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
