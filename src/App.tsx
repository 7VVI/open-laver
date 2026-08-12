import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import appIcon from "./assets/app_icon.png";
import { api, SessionDto } from "./lib/api";
import { useTauriEvent, EV } from "./lib/events";
import ChatView from "./views/ChatView";
import ApprovalDialog from "./views/ApprovalDialog";
import SkillsView from "./views/SkillsView";
import CronView from "./views/CronView";
import SettingsDialog from "./views/SettingsDialog";
import ModelEditor from "./views/ModelEditor";

type Tab =
  | "chat"
  | "skills"
  | "cron";

interface NavItem {
  id: Tab;
  label: string;
  icon: JSX.Element;
}

const icon = (path: string) => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7"
    strokeLinecap="round" strokeLinejoin="round" className="w-[18px] h-[18px]">
    {path.split("|").map((d, i) => (
      <path key={i} d={d} />
    ))}
  </svg>
);

const NAV_MAIN: NavItem[] = [
  { id: "chat", label: "新任务", icon: icon("M12 5v14|M5 12h14") },
  { id: "skills", label: "技能", icon: icon("M4 7h16|M4 12h16|M4 17h10") },
  { id: "cron", label: "定时任务", icon: icon("M12 8v4l3 2|M12 3a9 9 0 100 18 9 9 0 000-18z") },
];

export interface ApprovalReq {
  id: string;
  session_id: string;
  agent: string;
  tool: string;
  summary: string;
  input: any;
}

interface Notice {
  id: number;
  level: string;
  text: string;
}

export default function App() {
  const [tab, setTab] = useState<Tab>("chat");
  const [sessions, setSessions] = useState<SessionDto[]>([]);
  const [activeSession, setActiveSession] = useState<string | null>(null);
  const [approvals, setApprovals] = useState<ApprovalReq[]>([]);
  const [notices, setNotices] = useState<Notice[]>([]);
  const [showAddModel, setShowAddModel] = useState(false);
  const [modelsRefreshKey, setModelsRefreshKey] = useState(0);
  // 当前会话是否已有对话内容 (空对话时不重复新建)
  const [activeHasContent, setActiveHasContent] = useState(false);
  // 顶栏: 侧边栏折叠 / 应用菜单 / 搜索 / 信息弹窗
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [showAppMenu, setShowAppMenu] = useState(false);
  const [showSearch, setShowSearch] = useState(false);
  const [infoDialog, setInfoDialog] = useState<{ title: string; body: string } | null>(null);
  // 设置弹窗 (含模型管理)
  const [showSettings, setShowSettings] = useState(false);
  const [settingsCategory, setSettingsCategory] = useState("system");
  // 右侧面板 (任务进度 + 工作目录文件树) 折叠状态
  const [showRightPanel, setShowRightPanel] = useState(false);
  // 外部注入到对话输入框的草稿 (如: 通过助手创建技能)
  const [chatDraft, setChatDraft] = useState<{ text: string; key: number } | null>(null);
  // 每次点击「新任务」递增, 用于重新触发主页欢迎动画并重置输入
  const [welcomeKey, setWelcomeKey] = useState(0);

  const refreshSessions = async () => {
    const list = await api.listSessions();
    setSessions(list);
  };

  // 启动进入主界面: 只加载会话列表, 不自动创建会话 (有输入内容时才创建)
  useEffect(() => {
    (async () => {
      setSessions(await api.listSessions());
    })();
  }, []);

  useTauriEvent<ApprovalReq>(EV.APPROVAL_REQUEST, (p) => {
    setApprovals((prev) => [...prev, p]);
  });

  useTauriEvent<{ level: string; text: string }>(EV.NOTICE, (p) => {
    const id = Date.now() + Math.random();
    setNotices((prev) => [...prev, { id, ...p }]);
    setTimeout(() => setNotices((prev) => prev.filter((n) => n.id !== id)), 5000);
  });

  // 首次自动生成的会话标题 -> 就地更新侧边栏
  useTauriEvent<{ session_id: string; title: string }>(EV.SESSION_TITLE, (p) => {
    setSessions((prev) =>
      prev.map((s) => (s.id === p.session_id ? { ...s, title: p.title } : s))
    );
  });

  // 新任务: 只回到主界面, 不创建会话; 有输入内容时才创建
  const newSession = () => {
    setActiveSession(null);
    setActiveHasContent(false);
    setWelcomeKey((k) => k + 1);
    setTab("chat");
  };

  // 技能页「通过 Laver 助手创建」—> 新建/复用空会话并预填创建技能的草稿
  const createSkillViaChat = async () => {
    newSession();
    let root = "";
    try {
      root = await api.skillsRoot();
    } catch {
      /* 忽略，草稿中不带路径 */
    }
    setChatDraft({
      text:
        "帮我创建一个新技能。\n\n技能需求：(请在这里描述这个技能要完成的任务)\n\n创建要求：在" +
        (root ? ` ${root} ` : "技能") +
        "目录下新建以技能英文名命名的文件夹，写入 SKILL.md（需包含 YAML frontmatter：name、description、when_to_use，正文为 Markdown 格式的操作步骤说明）。完成后告诉我技能名称，我在技能页点「重新扫描」即可使用。",
      key: Date.now(),
    });
  };

  const openChatSession = (id: string) => {
    setActiveSession(id);
    setTab("chat");
  };

  const resolveApproval = async (id: string, decision: string) => {
    await api.resolveApproval(id, decision);
    setApprovals((prev) => prev.filter((a) => a.id !== id));
  };

  // 供子组件主动弹提示 (复用底部 toast)
  const pushNotice = (level: string, text: string) => {
    const id = Date.now() + Math.random();
    setNotices((prev) => [...prev, { id, level, text }]);
    setTimeout(() => setNotices((prev) => prev.filter((n) => n.id !== id)), 5000);
  };

  const openSettings = (category: string) => {
    setSettingsCategory(category);
    setShowSettings(true);
  };

  // 确保存在一个会话 (供定时任务绑定执行会话)
  const ensureSession = async (): Promise<string> => {
    if (activeSession) return activeSession;
    const s = await api.createSession(
      "新对话 " + new Date().toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" })
    );
    await refreshSessions();
    setActiveSession(s.id);
    return s.id;
  };

  const NavButton = ({ item }: { item: NavItem }) => (
    <button
      onClick={() => (item.id === "chat" ? newSession() : setTab(item.id))}
      className={`w-full flex items-center gap-2.5 px-3 py-2 rounded-lg text-sm transition ${
        tab === item.id
          ? "bg-[#e0e0e0] text-[#333333] font-medium"
          : "text-slate-600 hover:bg-slate-100"
      }`}
    >
      <span className={tab === item.id ? "text-[#333333]" : "text-slate-400"}>{item.icon}</span>
      {item.label}
    </button>
  );

  return (
    <div className="flex flex-col h-full bg-white text-slate-800">
      {/* 顶部工具栏 */}
      <TopBar
        onToggleSidebar={() => setSidebarCollapsed((v) => !v)}
        showAppMenu={showAppMenu}
        setShowAppMenu={setShowAppMenu}
        onOpenSearch={() => setShowSearch(true)}
        rightPanelOpen={showRightPanel}
        onToggleRightPanel={() => setShowRightPanel((v) => !v)}
        onCheckUpdate={() =>
          setInfoDialog({ title: "检查更新", body: "当前已是最新版本（v0.1.0）。" })
        }
        onFeedback={() =>
          setInfoDialog({
            title: "问题反馈",
            body: "感谢使用 Laver 办公！\n\n如有问题或建议，请通过以下方式反馈：\n邮箱：support@openlaver.local\n\n请附上操作步骤与截图，以便我们定位问题。",
          })
        }
        onAbout={() =>
          setInfoDialog({
            title: "关于 Laver 办公",
            body: "Laver 办公 · 桌面智能体\n版本 v0.1.0\n\n一个运行在本地的 AI 办公助手，支持多模型、技能、定时任务与团队协作。",
          })
        }
      />

      <div className="flex flex-1 min-h-0">
      {/* 侧边栏 (宽度过渡丝滑折叠) */}
      <aside
        className={`shrink-0 bg-[#f6f6f6] border-r border-slate-200 overflow-hidden transition-[width] duration-300 ease-in-out ${
          sidebarCollapsed ? "w-0 border-r-0" : "w-60"
        }`}
      >
        <div className="w-60 h-full flex flex-col">
        {/* 品牌 */}
        <div className="px-4 pt-4 pb-3 flex items-center gap-2">
          <img
            src={appIcon}
            alt="Laver"
            className="w-7 h-7 rounded-lg object-cover"
          />
          <span className="font-semibold text-slate-800 text-[15px]">Laver 办公</span>
          <span className="text-[10px] text-slate-400 border border-slate-300 rounded px-1 py-[1px]">
            BETA
          </span>
        </div>

        {/* 导航菜单 (固定, 不随最近对话滚动) */}
        <div className="px-2 py-2 space-y-0.5">
          {NAV_MAIN.map((n) => (
            <NavButton key={n.id} item={n} />
          ))}
        </div>

        {/* 会话历史 (独立滚动区域) */}
        <div className="flex-1 min-h-0 flex flex-col px-2 pb-2">
          <div className="pt-3 pb-1 px-3 text-[11px] text-slate-400 uppercase tracking-wide">
            最近对话
          </div>
          <div className="hover-scroll flex-1 min-h-0 overflow-y-auto space-y-0.5">
          {sessions.length === 0 && (
            <div className="px-3 py-1 text-xs text-slate-400">暂无对话</div>
          )}
          {sessions.map((s) => (
            <SessionRow
              key={s.id}
              s={s}
              active={activeSession === s.id && tab === "chat"}
              onOpen={() => openChatSession(s.id)}
              onRename={async (title) => { await api.renameSession(s.id, title); refreshSessions(); }}
              onPin={async (pinned) => { await api.setSessionPinned(s.id, pinned); refreshSessions(); }}
              onExport={async () => {
                try {
                  const p = await api.exportSession(s.id);
                  pushNotice("info", "已导出对话到 " + p);
                } catch (e: any) {
                  pushNotice("error", "导出失败：" + (e?.toString() ?? ""));
                }
              }}
              onDelete={async () => {
                const wasActive = activeSession === s.id;
                await api.deleteSession(s.id);
                if (wasActive) {
                  // 删除当前会话 -> 回到主界面, 不自动创建新对话
                  await refreshSessions();
                  setActiveSession(null);
                  setActiveHasContent(false);
                  setTab("chat");
                } else {
                  refreshSessions();
                }
              }}
            />
          ))}
          </div>
        </div>

        {/* 底部: 设置 (弹窗，含模型管理) */}
        <div className="border-t border-slate-200 p-2 space-y-0.5">
          <button
            onClick={() => openSettings("system")}
            className="w-full flex items-center justify-between px-3 py-2 rounded-lg text-sm transition text-slate-600 hover:bg-slate-100"
          >
            设置
            <span className="text-slate-400">
              {icon("M21 4h-7|M10 4H3|M21 12h-9|M8 12H3|M21 20h-5|M12 20H3|M14 2v4|M8 10v4|M16 18v4")}
            </span>
          </button>
        </div>
        </div>
      </aside>

      {/* 主区 */}
      <main className="flex-1 min-w-0 relative bg-white">
        {tab === "chat" &&
          <ChatView
            sessionId={activeSession}
            onAddModel={() => setShowAddModel(true)}
            onManageModels={() => openSettings("models")}
            modelsRefreshKey={modelsRefreshKey}
            onContentChange={setActiveHasContent}
            onNotice={pushNotice}
            draft={chatDraft}
            showRightPanel={showRightPanel}
            welcomeKey={welcomeKey}
            onFirstSend={async (text, atts) => {
              const s = await api.createSession(
                "新对话 " +
                  new Date().toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" })
              );
              await refreshSessions();
              setActiveSession(s.id);
              setActiveHasContent(true);
              await api.sendMessage(s.id, text, atts.length ? atts : undefined);
              return s.id;
            }}
          />}
        {tab === "skills" && (
          <SkillsView onNotice={pushNotice} onCreateViaAssistant={createSkillViaChat} />
        )}
        {tab === "cron" && (
          <CronView
            sessionId={activeSession}
            ensureSession={ensureSession}
            onNotice={pushNotice}
          />
        )}
      </main>
      </div>

      {/* 设置弹窗 (含模型管理) */}
      {showSettings && (
        <SettingsDialog
          initialCategory={settingsCategory}
          onClose={() => setShowSettings(false)}
          onModelsChanged={() => setModelsRefreshKey((k) => k + 1)}
          onNotice={pushNotice}
        />
      )}

      {/* 搜索弹窗 (命令面板式) */}
      {showSearch && (
        <SearchDialog
          sessions={sessions}
          activeId={activeSession}
          onClose={() => setShowSearch(false)}
          onOpenSession={(id) => {
            setShowSearch(false);
            openChatSession(id);
          }}
        />
      )}

      {/* 信息弹窗: 关于 / 反馈 / 检查更新 */}
      {infoDialog && (
        <InfoDialog
          title={infoDialog.title}
          body={infoDialog.body}
          onClose={() => setInfoDialog(null)}
        />
      )}

      {/* 权限审批弹窗 */}
      {approvals.length > 0 && (
        <ApprovalDialog req={approvals[0]} onResolve={resolveApproval} />
      )}

      {/* 快速添加模型弹窗 (全局) */}
      {showAddModel && (
        <ModelEditor
          onClose={() => setShowAddModel(false)}
          onSaved={() => {
            setShowAddModel(false);
            setModelsRefreshKey((k) => k + 1);
          }}
        />
      )}

      {/* 通知 toast */}
      <div className="fixed bottom-4 right-4 space-y-2 z-50">
        {notices.map((n) => (
          <div
            key={n.id}
            className={`px-4 py-2 rounded-lg text-sm shadow-lg border fade-in ${
              n.level === "error"
                ? "bg-red-50 border-red-200 text-red-700"
                : n.level === "warn"
                ? "bg-amber-50 border-amber-200 text-amber-700"
                : "bg-white border-slate-200 text-slate-700"
            }`}
          >
            {n.text}
          </div>
        ))}
      </div>
    </div>
  );
}

/* ---------------- 会话列表项 (带三点菜单) ---------------- */

function SessionRow({
  s,
  active,
  onOpen,
  onRename,
  onPin,
  onExport,
  onDelete,
}: {
  s: SessionDto;
  active: boolean;
  onOpen: () => void;
  onRename: (title: string) => Promise<void>;
  onPin: (pinned: boolean) => Promise<void>;
  onExport: () => Promise<void>;
  onDelete: () => Promise<void>;
}) {
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const [showRename, setShowRename] = useState(false);
  const [confirmDel, setConfirmDel] = useState(false);

  const openMenu = (e: React.MouseEvent) => {
    e.stopPropagation();
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    setMenu({ x: r.right, y: r.bottom });
  };

  const menuItem =
    "w-full flex items-center gap-2.5 px-3 py-2 text-sm text-left hover:bg-slate-50";

  return (
    <>
    <div
      onClick={onOpen}
      className={`group flex items-center justify-between px-3 py-1.5 rounded-lg text-[13px] cursor-pointer ${
        active ? "bg-slate-200/70 text-slate-800" : "text-slate-500 hover:bg-slate-100"
      }`}
    >
      <span className="truncate flex items-center gap-1.5">
        {s.pinned && (
          <svg viewBox="0 0 24 24" className="w-3 h-3 shrink-0 text-[#34c759]" fill="currentColor">
            <path d="M14 4v6l3 3v2H7v-2l3-3V4z" />
          </svg>
        )}
        {s.title}
      </span>
      <button
        onClick={openMenu}
        className="opacity-0 group-hover:opacity-100 text-slate-400 hover:text-slate-700 shrink-0"
        title="更多"
      >
        <svg viewBox="0 0 24 24" className="w-4 h-4" fill="currentColor">
          <circle cx="5" cy="12" r="1.6" /><circle cx="12" cy="12" r="1.6" /><circle cx="19" cy="12" r="1.6" />
        </svg>
      </button>
      {menu && (
        <>
          <div className="fixed inset-0 z-40" onClick={(e) => { e.stopPropagation(); setMenu(null); }} />
          <div
            className="fixed z-50 w-44 bg-white border border-slate-200 rounded-xl shadow-lg py-1.5 fade-in"
            style={{ top: menu.y + 4, left: Math.max(8, menu.x - 176) }}
            onClick={(e) => e.stopPropagation()}
          >
            <button className={`${menuItem} text-slate-600`} onClick={() => { setMenu(null); setShowRename(true); }}>
              <svg viewBox="0 0 24 24" className="w-4 h-4 text-slate-400" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round"><path d="M12 20h9" /><path d="M16.5 3.5a2.1 2.1 0 013 3L7 19l-4 1 1-4z" /></svg>
              重命名
            </button>
            <button className={`${menuItem} text-slate-600`} onClick={async () => { setMenu(null); await onPin(!s.pinned); }}>
              <svg viewBox="0 0 24 24" className="w-4 h-4 text-slate-400" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round"><path d="M9 4h6v6l3 3v2H6v-2l3-3z" /><path d="M12 15v5" /></svg>
              {s.pinned ? "取消置顶" : "置顶"}
            </button>
            <button className={`${menuItem} text-slate-600`} onClick={async () => { setMenu(null); await onExport(); }}>
              <svg viewBox="0 0 24 24" className="w-4 h-4 text-slate-400" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round"><path d="M12 3v12" /><path d="M8 11l4 4 4-4" /><path d="M4 21h16" /></svg>
              导出对话记录
            </button>
            <div className="my-1 border-t border-slate-100" />
            <button className={`${menuItem} text-red-500`} onClick={() => { setMenu(null); setConfirmDel(true); }}>
              <svg viewBox="0 0 24 24" className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round"><path d="M3 6h18M8 6V4a2 2 0 012-2h4a2 2 0 012 2v2m2 0v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6" /></svg>
              删除
            </button>
          </div>
        </>
      )}
    </div>
    {showRename && (
      <RenameDialog
        initial={s.title}
        onClose={() => setShowRename(false)}
        onSave={async (t) => {
          setShowRename(false);
          if (t.trim() && t.trim() !== s.title) await onRename(t.trim());
        }}
      />
    )}
    {confirmDel && (
      <ConfirmDialog
        title="删除对话"
        message={`确定删除“${s.title}”？删除后无法恢复。`}
        confirmText="删除"
        onCancel={() => setConfirmDel(false)}
        onConfirm={async () => { setConfirmDel(false); await onDelete(); }}
      />
    )}
    </>
  );
}

/* ---------------- 重命名 / 确认 弹窗 ---------------- */

function RenameDialog({
  initial,
  onClose,
  onSave,
}: {
  initial: string;
  onClose: () => void;
  onSave: (title: string) => void;
}) {
  const [value, setValue] = useState(initial);
  return (
    <div className="fixed inset-0 bg-black/30 flex items-center justify-center z-[60] fade-in" onClick={onClose}>
      <div className="bg-white rounded-2xl shadow-2xl w-[420px] max-w-[92vw] p-6" onClick={(e) => e.stopPropagation()}>
        <h2 className="font-semibold text-slate-800 mb-4">重命名任务</h2>
        <input
          autoFocus
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={(e) => { if (e.key === "Enter") onSave(value); if (e.key === "Escape") onClose(); }}
          className="w-full bg-white border border-slate-300 rounded-lg px-3 py-2.5 text-sm text-slate-800 focus:outline-none focus:border-[#34c759]"
        />
        <div className="flex justify-end gap-2 mt-5">
          <button onClick={onClose} className="px-4 py-2 rounded-lg text-sm bg-slate-100 hover:bg-slate-200 text-slate-600">取消</button>
          <button onClick={() => onSave(value)} disabled={!value.trim()} className="px-5 py-2 rounded-lg text-sm bg-slate-800 hover:bg-slate-700 text-white disabled:opacity-40">保存</button>
        </div>
      </div>
    </div>
  );
}

function ConfirmDialog({
  title,
  message,
  confirmText,
  onCancel,
  onConfirm,
}: {
  title: string;
  message: string;
  confirmText: string;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  return (
    <div className="fixed inset-0 bg-black/30 flex items-center justify-center z-[60] fade-in" onClick={onCancel}>
      <div className="bg-white rounded-2xl shadow-2xl w-[400px] max-w-[92vw] p-6" onClick={(e) => e.stopPropagation()}>
        <h2 className="font-semibold text-slate-800">{title}</h2>
        <p className="text-sm text-slate-600 mt-3 leading-relaxed">{message}</p>
        <div className="flex justify-end gap-2 mt-6">
          <button onClick={onCancel} className="px-4 py-2 rounded-lg text-sm bg-slate-100 hover:bg-slate-200 text-slate-600">取消</button>
          <button onClick={onConfirm} className="px-5 py-2 rounded-lg text-sm bg-red-500 hover:bg-red-600 text-white">{confirmText}</button>
        </div>
      </div>
    </div>
  );
}

/* ---------------- 顶部工具栏 ---------------- */

function TopBar(props: {
  onToggleSidebar: () => void;
  showAppMenu: boolean;
  setShowAppMenu: (v: boolean) => void;
  onOpenSearch: () => void;
  onCheckUpdate: () => void;
  onFeedback: () => void;
  onAbout: () => void;
  rightPanelOpen: boolean;
  onToggleRightPanel: () => void;
}) {
  const iconBtn =
    "w-8 h-8 rounded-lg flex items-center justify-center text-slate-500 hover:bg-slate-200/70";

  // 跟踪窗口是否最大化，用于切换最大化/还原图标
  const [maximized, setMaximized] = useState(false);
  useEffect(() => {
    const w = getCurrentWindow();
    w.isMaximized().then(setMaximized).catch(() => {});
    const un = w.onResized(() => { w.isMaximized().then(setMaximized).catch(() => {}); });
    return () => { un.then((f) => f()).catch(() => {}); };
  }, []);

  return (
    <div
      className="h-10 shrink-0 flex items-center gap-0.5 px-2 border-b border-slate-200 bg-[#f6f6f6] relative"
      data-tauri-drag-region
    >
      {/* 汉堡菜单 */}
      <div className="relative">
        <button
          className={iconBtn}
          onClick={() => { props.setShowAppMenu(!props.showAppMenu); }}
          title="菜单"
        >
          <svg viewBox="0 0 24 24" className="w-[18px] h-[18px]" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round">
            <path d="M4 6h16M4 12h16M4 18h16" />
          </svg>
        </button>
        {props.showAppMenu && (
          <>
            <div className="fixed inset-0 z-30" onClick={() => props.setShowAppMenu(false)} />
            <div className="absolute top-full mt-1 left-0 w-44 bg-white border border-slate-200 rounded-xl shadow-lg py-1.5 z-40 fade-in">
              <MenuItem
                label="检查更新"
                onClick={() => { props.setShowAppMenu(false); props.onCheckUpdate(); }}
                icon="M21 2v6h-6|M3 12a9 9 0 0115-6.7L21 8|M3 22v-6h6|M21 12a9 9 0 01-15 6.7L3 16"
              />
              <MenuItem
                label="问题反馈"
                onClick={() => { props.setShowAppMenu(false); props.onFeedback(); }}
                icon="M12 17h.01|M12 3a9 9 0 100 18 9 9 0 000-18z|M9.1 9a3 3 0 015.8 1c0 2-3 2.5-3 2.5"
              />
              <div className="my-1 border-t border-slate-100" />
              <MenuItem
                label="关于"
                onClick={() => { props.setShowAppMenu(false); props.onAbout(); }}
                icon="M12 16v-4|M12 8h.01|M12 3a9 9 0 100 18 9 9 0 000-18z"
              />
            </div>
          </>
        )}
      </div>

      {/* 侧边栏折叠 */}
      <button className={iconBtn} onClick={props.onToggleSidebar} title="折叠/展开侧边栏">
        <svg viewBox="0 0 24 24" className="w-[18px] h-[18px]" fill="none" stroke="currentColor" strokeWidth="1.8">
          <rect x="3" y="4" width="18" height="16" rx="2" />
          <path d="M9 4v16" />
        </svg>
      </button>

      {/* 搜索 (打开命令面板式弹窗) */}
      <button
        className={iconBtn}
        onClick={() => { props.onOpenSearch(); props.setShowAppMenu(false); }}
        title="搜索对话"
      >
        <svg viewBox="0 0 24 24" className="w-[18px] h-[18px]" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round">
          <circle cx="11" cy="11" r="7" />
          <path d="M21 21l-4.3-4.3" />
        </svg>
      </button>

      {/* 中间可拖拽区 (拖动窗口) */}
      <div className="flex-1 self-stretch" data-tauri-drag-region />

      {/* 窗口控制 */}
      <div className="flex items-center gap-0.5">
        <button
          className={`${iconBtn} ${props.rightPanelOpen ? "text-[#34c759]" : ""}`}
          onClick={props.onToggleRightPanel}
          title={props.rightPanelOpen ? "折叠右侧面板" : "展开右侧面板"}
        >
          <svg viewBox="0 0 24 24" className="w-[16px] h-[16px]" fill="none" stroke="currentColor" strokeWidth="1.8">
            <rect x="3" y="4" width="18" height="16" rx="2" />
            <path d="M15 4v16" />
          </svg>
        </button>
        <button
          className={iconBtn}
          onClick={() => getCurrentWindow().minimize()}
          title="最小化"
        >
          <svg viewBox="0 0 24 24" className="w-[16px] h-[16px]" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round">
            <path d="M5 12h14" />
          </svg>
        </button>
        <button
          className={iconBtn}
          onClick={async () => { const w = getCurrentWindow(); await w.toggleMaximize(); setMaximized(await w.isMaximized()); }}
          title={maximized ? "还原" : "最大化"}
        >
          {maximized ? (
            <svg viewBox="0 0 24 24" className="w-[14px] h-[14px]" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
              <rect x="4" y="8" width="12" height="12" rx="2" />
              <path d="M8 8V6a2 2 0 012-2h8a2 2 0 012 2v8a2 2 0 01-2 2h-2" />
            </svg>
          ) : (
            <svg viewBox="0 0 24 24" className="w-[14px] h-[14px]" fill="none" stroke="currentColor" strokeWidth="1.8">
              <rect x="4" y="4" width="16" height="16" rx="2" />
            </svg>
          )}
        </button>
        <button
          className="w-8 h-8 rounded-lg flex items-center justify-center text-slate-500 hover:bg-red-500 hover:text-white"
          onClick={() => getCurrentWindow().close()}
          title="关闭"
        >
          <svg viewBox="0 0 24 24" className="w-[16px] h-[16px]" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round">
            <path d="M6 6l12 12M18 6L6 18" />
          </svg>
        </button>
      </div>
    </div>
  );
}

function MenuItem({
  label,
  icon,
  onClick,
}: {
  label: string;
  icon: string;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className="w-full flex items-center gap-2.5 px-3 py-2 text-sm text-slate-600 hover:bg-slate-50 text-left"
    >
      <svg viewBox="0 0 24 24" className="w-4 h-4 text-slate-400" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
        {icon.split("|").map((d, i) => (
          <path key={i} d={d} />
        ))}
      </svg>
      {label}
    </button>
  );
}

function InfoDialog({
  title,
  body,
  onClose,
}: {
  title: string;
  body: string;
  onClose: () => void;
}) {
  return (
    <div className="fixed inset-0 bg-black/30 flex items-center justify-center z-50 fade-in" onClick={onClose}>
      <div
        className="bg-white border border-slate-200 rounded-2xl w-[420px] max-w-[90vw] shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="px-5 py-4 border-b border-slate-100 flex items-center justify-between">
          <h2 className="font-semibold text-slate-800">{title}</h2>
          <button onClick={onClose} className="text-slate-400 hover:text-slate-600">✕</button>
        </div>
        <div className="p-5 text-sm text-slate-600 whitespace-pre-wrap leading-relaxed">{body}</div>
        <div className="px-5 py-4 border-t border-slate-100 flex justify-end">
          <button
            onClick={onClose}
            className="px-5 py-2 rounded-lg text-sm bg-[#333333] hover:bg-[#111111] text-white"
          >
            知道了
          </button>
        </div>
      </div>
    </div>
  );
}

/* ---------------- 搜索弹窗 (命令面板式) ---------------- */

function SearchDialog({
  sessions,
  activeId,
  onClose,
  onOpenSession,
}: {
  sessions: SessionDto[];
  activeId: string | null;
  onClose: () => void;
  onOpenSession: (id: string) => void;
}) {
  const [query, setQuery] = useState("");
  const q = query.trim().toLowerCase();
  const results = q
    ? sessions.filter((s) => s.title.toLowerCase().includes(q))
    : sessions;

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onClose();
        return;
      }
      if (e.key === "Enter" && results[0]) {
        onOpenSession(results[0].id);
        return;
      }
      if ((e.ctrlKey || e.metaKey) && /^[1-9]$/.test(e.key)) {
        const idx = parseInt(e.key, 10) - 1;
        if (results[idx]) {
          e.preventDefault();
          onOpenSession(results[idx].id);
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [results, onClose, onOpenSession]);

  const kbd =
    "px-1.5 py-0.5 bg-slate-100 rounded border border-slate-200 text-[11px] text-slate-500 font-sans";

  return (
    <div
      className="fixed inset-0 bg-black/30 flex items-start justify-center z-50 fade-in pt-[12vh]"
      onClick={onClose}
    >
      <div
        className="bg-white border border-slate-200 rounded-2xl w-[620px] max-w-[92vw] shadow-2xl overflow-hidden"
        onClick={(e) => e.stopPropagation()}
      >
        {/* 搜索输入 */}
        <div className="flex items-center gap-3 px-5 py-4 border-b border-slate-100">
          <svg viewBox="0 0 24 24" className="w-5 h-5 text-slate-400 shrink-0" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round">
            <circle cx="11" cy="11" r="7" />
            <path d="M21 21l-4.3-4.3" />
          </svg>
          <input
            autoFocus
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="搜索任务标题或会话内容..."
            className="flex-1 bg-transparent text-[15px] text-slate-800 placeholder:text-slate-400 focus:outline-none"
          />
        </div>

        {/* 计数 + 提示 */}
        <div className="flex items-center justify-between px-5 py-2 text-xs text-slate-400">
          <span>共 {results.length} 个任务</span>
          <span className="flex items-center gap-1">
            按 <kbd className={kbd}>Ctrl</kbd> + 数字 快速打开
          </span>
        </div>

        {/* 列表 */}
        <div className="max-h-[52vh] overflow-y-auto pb-2">
          {results.length === 0 ? (
            <div className="px-5 py-12 text-center text-sm text-slate-400">无匹配的对话</div>
          ) : (
            results.map((s, i) => (
              <button
                key={s.id}
                onClick={() => onOpenSession(s.id)}
                className="w-full flex items-center gap-3 px-5 py-2.5 hover:bg-slate-50 text-left"
              >
                <span
                  className={`w-2.5 h-2.5 rounded-full shrink-0 ${
                    s.id === activeId ? "bg-[#34c759]" : "border-2 border-slate-300"
                  }`}
                />
                <span className="flex-1 truncate text-sm text-slate-700">{s.title}</span>
                {i < 9 && (
                  <span className="flex items-center gap-1">
                    <kbd className={kbd}>Ctrl</kbd>
                    <kbd className={kbd}>{i + 1}</kbd>
                  </span>
                )}
              </button>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
