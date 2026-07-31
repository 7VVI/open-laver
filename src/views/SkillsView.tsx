import { useEffect, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { open } from "@tauri-apps/plugin-dialog";
import { api, SkillMeta } from "../lib/api";

export default function SkillsView({
  onNotice,
  onCreateViaAssistant,
}: {
  onNotice?: (level: string, text: string) => void;
  onCreateViaAssistant?: () => void;
}) {
  const [skills, setSkills] = useState<SkillMeta[]>([]);
  const [adding, setAdding] = useState(false);
  const [addMenu, setAddMenu] = useState(false);
  const [importing, setImporting] = useState(false);
  const [confirmDel, setConfirmDel] = useState<string | null>(null);
  const [viewing, setViewing] = useState<SkillMeta | null>(null);
  const [scanning, setScanning] = useState(false);

  const refresh = () => api.listSkills().then(setSkills);
  useEffect(() => {
    refresh();
  }, []);

  // 重新扫描: 按钮展示扫描中状态 + 完成后 toast 反馈数量变化
  const rescan = async () => {
    if (scanning) return;
    setScanning(true);
    const before = skills.length;
    try {
      // 扫描很快，补一个最短展示时长让状态可感知
      const [list] = await Promise.all([
        api.rescanSkills(),
        new Promise((r) => setTimeout(r, 400)),
      ]);
      setSkills(list);
      const diff = list.length - before;
      const change =
        diff > 0 ? `，新增 ${diff} 个` : diff < 0 ? `，移除 ${-diff} 个` : "，无变化";
      onNotice?.("info", `扫描完成，共 ${list.length} 个技能${change}`);
    } catch (e: any) {
      onNotice?.("error", "扫描失败：" + (e?.toString() ?? ""));
    } finally {
      setScanning(false);
    }
  };

  const del = async (name: string) => {
    setSkills(await api.deleteSkill(name));
    setConfirmDel(null);
  };

  // 上传技能 zip 包并导入
  const importZip = async () => {
    setAddMenu(false);
    const picked = await open({
      multiple: false,
      filters: [{ name: "技能包", extensions: ["zip"] }],
    });
    if (!picked || typeof picked !== "string") return;
    setImporting(true);
    try {
      const name = await api.importSkillZip(picked);
      await refresh();
      onNotice?.("info", `技能「${name}」导入成功`);
    } catch (e: any) {
      onNotice?.("error", "导入失败：" + (e?.toString() ?? ""));
    } finally {
      setImporting(false);
    }
  };

  return (
    <div className="h-full overflow-y-auto">
      <div className="max-w-3xl mx-auto px-8 py-8">
        <div className="flex items-center justify-between mb-1">
          <h1 className="text-xl font-semibold text-slate-800">技能</h1>
          <div className="flex items-center gap-2">
            <button
              onClick={rescan}
              disabled={scanning}
              className="text-sm bg-slate-100 hover:bg-slate-200 rounded-lg px-3 py-1.5 text-slate-600 disabled:opacity-60 flex items-center gap-1.5"
            >
              <svg
                viewBox="0 0 24 24"
                className={`w-3.5 h-3.5 ${scanning ? "animate-spin" : ""}`}
                fill="none"
                stroke="currentColor"
                strokeWidth="1.8"
                strokeLinecap="round"
                strokeLinejoin="round"
              >
                <path d="M21 2v6h-6" />
                <path d="M3 12a9 9 0 0115-6.7L21 8" />
                <path d="M3 22v-6h6" />
                <path d="M21 12a9 9 0 01-15 6.7L3 16" />
              </svg>
              {scanning ? "扫描中…" : "重新扫描"}
            </button>
            <div className="relative">
              <button
                onClick={() => setAddMenu((v) => !v)}
                disabled={importing}
                className="text-sm bg-[#10a37f] hover:bg-[#0e9070] text-white rounded-lg px-3 py-1.5 disabled:opacity-50"
              >
                {importing ? "导入中…" : "+ 添加技能"}
              </button>
              {addMenu && (
                <>
                  <div className="fixed inset-0 z-40" onClick={() => setAddMenu(false)} />
                  <div className="absolute top-full right-0 mt-1.5 w-64 bg-white border border-slate-200 rounded-xl shadow-lg py-1.5 z-50 fade-in">
                    <AddMenuItem
                      title="通过 Laver 助手创建"
                      desc="在对话中描述需求，由助手生成个性化技能"
                      icon="M12 3a9 9 0 100 18 9 9 0 000-18z|M8 12h8|M12 8v8"
                      onClick={() => {
                        setAddMenu(false);
                        onCreateViaAssistant?.();
                      }}
                    />
                    <AddMenuItem
                      title="上传技能"
                      desc="导入技能 zip 包，仅本地化安装"
                      icon="M12 15V4|M8 8l4-4 4 4|M4 20h16|M4 15v5|M20 15v5"
                      onClick={importZip}
                    />
                    <div className="my-1 border-t border-slate-100" />
                    <AddMenuItem
                      title="手动创建"
                      desc="填写名称与 Markdown 说明"
                      icon="M12 20h9|M16.5 3.5a2.1 2.1 0 013 3L7 19l-4 1 1-4z"
                      onClick={() => {
                        setAddMenu(false);
                        setAdding(true);
                      }}
                    />
                  </div>
                </>
              )}
            </div>
          </div>
        </div>
        <p className="text-sm text-slate-500 mb-6">
          安装的技能会告知智能体其能力，需要时自动加载完整操作说明。
        </p>

        {skills.length === 0 ? (
          <div className="text-center text-slate-400 mt-20">
            <div className="text-4xl mb-3">🧩</div>
            <p>暂无技能，点击右上角「添加技能」创建。</p>
          </div>
        ) : (
          <div className="space-y-3">
            {skills.map((s) => (
              <div
                key={s.name}
                onClick={() => setViewing(s)}
                className="bg-white border border-slate-200 rounded-xl p-4 flex items-start justify-between gap-4 shadow-sm cursor-pointer hover:border-[#10a37f]/50 hover:shadow transition-all"
              >
                <div className="min-w-0">
                  <div className="font-medium text-slate-800">{s.name}</div>
                  <div className="text-sm text-slate-500 mt-1">{s.description}</div>
                  {s.when_to_use && (
                    <div className="text-xs text-slate-400 mt-1">适用: {s.when_to_use}</div>
                  )}
                </div>
                <div className="flex items-center gap-3 shrink-0" onClick={(e) => e.stopPropagation()}>
                  <button
                    onClick={() => setConfirmDel(s.name)}
                    className="text-slate-400 hover:text-red-500"
                    title="删除技能"
                  >
                    <svg viewBox="0 0 24 24" className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
                      <path d="M3 6h18M8 6V4a2 2 0 012-2h4a2 2 0 012 2v2m2 0v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6" />
                    </svg>
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {viewing && <SkillDetail skill={viewing} onClose={() => setViewing(null)} />}

      {confirmDel && (
        <div
          className="fixed inset-0 bg-black/30 flex items-center justify-center z-[60] fade-in"
          onClick={() => setConfirmDel(null)}
        >
          <div
            className="bg-white rounded-2xl shadow-2xl w-[400px] max-w-[92vw] p-6"
            onClick={(e) => e.stopPropagation()}
          >
            <h2 className="font-semibold text-slate-800">删除技能</h2>
            <p className="text-sm text-slate-600 mt-3 leading-relaxed">
              确定删除技能「{confirmDel}」？将移除其整个文件夹，删除后无法恢复。
            </p>
            <div className="flex justify-end gap-2 mt-6">
              <button
                onClick={() => setConfirmDel(null)}
                className="px-4 py-2 rounded-lg text-sm bg-slate-100 hover:bg-slate-200 text-slate-600"
              >
                取消
              </button>
              <button
                onClick={() => del(confirmDel)}
                className="px-5 py-2 rounded-lg text-sm bg-red-500 hover:bg-red-600 text-white"
              >
                删除
              </button>
            </div>
          </div>
        </div>
      )}

      {adding && (
        <SkillEditor
          onClose={() => setAdding(false)}
          onSaved={(list) => {
            setSkills(list);
            setAdding(false);
          }}
        />
      )}
    </div>
  );
}

/* ---------------- 添加方式菜单项 ---------------- */

function AddMenuItem({
  title,
  desc,
  icon,
  onClick,
}: {
  title: string;
  desc: string;
  icon: string;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className="w-full flex items-start gap-3 px-3.5 py-2.5 hover:bg-slate-50 text-left"
    >
      <svg
        viewBox="0 0 24 24"
        className="w-[18px] h-[18px] text-slate-500 shrink-0 mt-0.5"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.7"
        strokeLinecap="round"
        strokeLinejoin="round"
      >
        {icon.split("|").map((d, i) => (
          <path key={i} d={d} />
        ))}
      </svg>
      <span className="min-w-0">
        <span className="block text-sm text-slate-700">{title}</span>
        <span className="block text-xs text-slate-400 mt-0.5">{desc}</span>
      </span>
    </button>
  );
}

/* ---------------- 技能详情弹窗 (SKILL.md 预览) ---------------- */

function SkillDetail({ skill, onClose }: { skill: SkillMeta; onClose: () => void }) {
  const [body, setBody] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    api
      .readSkill(skill.name)
      .then(setBody)
      .catch((e) => setErr(e?.toString() ?? "读取失败"));
  }, [skill.name]);

  return (
    <div className="fixed inset-0 bg-black/30 flex items-center justify-center z-50 fade-in" onClick={onClose}>
      <div
        className="bg-white border border-slate-200 rounded-2xl w-[720px] max-w-[92vw] max-h-[85vh] shadow-2xl flex flex-col"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="px-6 py-4 border-b border-slate-100 flex items-start justify-between gap-4">
          <div className="flex items-start gap-3 min-w-0">
            <div className="w-10 h-10 rounded-lg bg-slate-100 flex items-center justify-center text-xl shrink-0">
              🧩
            </div>
            <div className="min-w-0">
              <div className="font-semibold text-slate-800 flex items-center gap-2">
                {skill.name}
                <span className="text-[11px] px-1.5 py-0.5 rounded font-normal bg-emerald-50 text-emerald-600">
                  可用
                </span>
              </div>
              <div className="text-sm text-slate-500 mt-0.5">{skill.description}</div>
              {skill.when_to_use && (
                <div className="text-xs text-slate-400 mt-0.5">适用: {skill.when_to_use}</div>
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
            以下内容来自该技能的 SKILL.md 原文
          </div>
          {err ? (
            <div className="text-sm text-red-600 bg-red-50 rounded-lg px-3 py-2">{err}</div>
          ) : body === null ? (
            <div className="text-sm text-slate-400 py-8 text-center">加载中…</div>
          ) : (
            <div className="md text-sm text-slate-700 leading-relaxed border border-slate-100 rounded-xl px-5 py-4">
              <ReactMarkdown remarkPlugins={[remarkGfm]}>{body}</ReactMarkdown>
            </div>
          )}
        </div>

        <div className="px-6 py-3 border-t border-slate-100 flex justify-between items-center">
          <span className="text-xs text-slate-400 truncate" title={skill.dir}>{skill.dir}</span>
          <button onClick={onClose} className="px-4 py-2 rounded-lg text-sm bg-slate-100 hover:bg-slate-200 text-slate-600 shrink-0">
            关闭
          </button>
        </div>
      </div>
    </div>
  );
}

/* ---------------- 添加技能弹窗 ---------------- */

function SkillEditor({
  onClose,
  onSaved,
}: {
  onClose: () => void;
  onSaved: (list: SkillMeta[]) => void;
}) {
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [whenToUse, setWhenToUse] = useState("");
  const [body, setBody] = useState(
    "# 技能说明\n\n描述这个技能的用途，以及智能体应如何一步步完成任务。"
  );
  const [saving, setSaving] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  const field =
    "w-full bg-white border border-slate-300 rounded-lg px-3 py-2 text-sm text-slate-800 focus:outline-none focus:border-[#10a37f]";

  const save = async () => {
    if (!name.trim() || !description.trim()) {
      setErr("请填写技能名称与简介");
      return;
    }
    if (/[\\/]/.test(name)) {
      setErr("技能名称不能包含斜杠");
      return;
    }
    setSaving(true);
    setErr(null);
    try {
      const list = await api.createSkill(name.trim(), description.trim(), whenToUse.trim(), body);
      onSaved(list);
    } catch (e: any) {
      setErr(e?.toString() ?? "创建失败");
      setSaving(false);
    }
  };

  return (
    <div className="fixed inset-0 bg-black/30 flex items-center justify-center z-50 fade-in" onClick={onClose}>
      <div
        className="bg-white border border-slate-200 rounded-2xl w-[560px] max-w-[92vw] shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="px-5 py-4 border-b border-slate-100 flex items-center justify-between">
          <h2 className="font-semibold text-slate-800">添加技能</h2>
          <button onClick={onClose} className="text-slate-400 hover:text-slate-600">✕</button>
        </div>

        <div className="p-5 space-y-3 max-h-[70vh] overflow-y-auto">
          <div>
            <label className="block text-xs text-slate-500 mb-1.5">技能名称（英文/数字，作为文件夹名）</label>
            <input value={name} onChange={(e) => setName(e.target.value)} placeholder="weekly-report" className={field} />
          </div>
          <div>
            <label className="block text-xs text-slate-500 mb-1.5">简介</label>
            <input value={description} onChange={(e) => setDescription(e.target.value)} placeholder="一句话说明这个技能能做什么" className={field} />
          </div>
          <div>
            <label className="block text-xs text-slate-500 mb-1.5">适用场景（可选）</label>
            <input value={whenToUse} onChange={(e) => setWhenToUse(e.target.value)} placeholder="用户需要生成周报时" className={field} />
          </div>
          <div>
            <label className="block text-xs text-slate-500 mb-1.5">技能内容（Markdown，指导智能体如何完成）</label>
            <textarea
              value={body}
              onChange={(e) => setBody(e.target.value)}
              rows={8}
              className={`${field} font-mono text-xs leading-relaxed`}
            />
          </div>
          {err && <div className="text-sm text-red-600 bg-red-50 rounded-lg px-3 py-2">{err}</div>}
        </div>

        <div className="px-5 py-4 border-t border-slate-100 flex justify-end gap-2">
          <button onClick={onClose} className="px-4 py-2 rounded-lg text-sm bg-slate-100 hover:bg-slate-200 text-slate-600">
            取消
          </button>
          <button
            onClick={save}
            disabled={saving}
            className="px-5 py-2 rounded-lg text-sm bg-[#10a37f] hover:bg-[#0e9070] text-white disabled:opacity-50"
          >
            {saving ? "保存中…" : "创建"}
          </button>
        </div>
      </div>
    </div>
  );
}
