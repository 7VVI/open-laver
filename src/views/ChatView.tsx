import { Fragment, useEffect, useRef, useState, type MouseEvent as ReactMouseEvent } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { open } from "@tauri-apps/plugin-dialog";
import {
  api,
  Message,
  TodoItem,
  ModelProfileDto,
  ThinkingLevel,
  THINKING_LABELS,
  CONTEXT_TIERS,
  formatContext,
  SkillMeta,
  DirEntry,
} from "../lib/api";
import { useTauriEvent, EV } from "../lib/events";
import agentIcon from "../assets/agent_icon_holo.png";

interface ToolCall {
  id: string;
  name: string;
  input: any;
  ok?: boolean;
  output?: string;
}
type UiBlock =
  | { kind: "text"; text: string }
  | { kind: "tools"; tools: ToolCall[] };

interface UiMsg {
  role: "user" | "assistant";
  text: string;
  blocks: UiBlock[];
  tools?: ToolCall[];
  streaming?: boolean;
  attachments?: string[];
}

function greeting() {
  const h = new Date().getHours();
  if (h < 6) return "凌晨好";
  if (h < 12) return "上午好";
  if (h < 14) return "中午好";
  if (h < 18) return "下午好";
  return "晚上好";
}

// 定时任务常用频率预设 (五段式 cron)
const CRON_PRESETS: { label: string; cron: string }[] = [
  { label: "每小时", cron: "0 * * * *" },
  { label: "每天 09:00", cron: "0 9 * * *" },
  { label: "每周一 09:00", cron: "0 9 * * 1" },
  { label: "每月 1 号 09:00", cron: "0 9 1 * *" },
];

export default function ChatView({
  sessionId,
  onAddModel,
  onManageModels,
  modelsRefreshKey,
  onContentChange,
  onNotice,
  draft,
  showRightPanel,
}: {
  sessionId: string;
  onAddModel: () => void;
  onManageModels: () => void;
  modelsRefreshKey: number;
  onContentChange?: (hasContent: boolean) => void;
  onNotice?: (level: string, text: string) => void;
  draft?: { text: string; key: number } | null;
  showRightPanel: boolean;
}) {
  const [messages, setMessages] = useState<UiMsg[]>([]);
  const [input, setInput] = useState("");
  const [running, setRunning] = useState(false);
  const [todos, setTodos] = useState<TodoItem[]>([]);
  const [models, setModels] = useState<ModelProfileDto[]>([]);
  const [skills, setSkills] = useState<SkillMeta[]>([]);
  const [workspace, setWorkspace] = useState<string>("");
  const [attachments, setAttachments] = useState<string[]>([]);
  // 消息队列: 当前任务运行时发送会排队，结束后自动发下一条
  const [queue, setQueue] = useState<{ text: string; attachments: string[] }[]>([]);
  // 当前任务是否已结束: 复制按钮仅在任务结束后显示
  const [taskDone, setTaskDone] = useState(true);
  const queueRef = useRef<{ text: string; attachments: string[] }[]>([]);
  const scrollRef = useRef<HTMLDivElement>(null);
  const streamingText = useRef("");
  // 状态兜底类更新 (任务结束 idle) 不触发自动滚动，避免输出结束后内容突然跳动
  const skipScrollRef = useRef(false);
  // 用户是否位于消息流底部: 仅在底部时新内容才自动滚动，上翻历史时保持不动
  const nearBottomRef = useRef(true);
  // 程序自动滚动标记: 区分“程序滚动”与“用户手动滚动”，
  // 避免自动滚动后触发 scroll 事件误判 nearBottomRef
  const autoScrollRef = useRef(false);
  // 每个会话独立保存输入草稿 (切换会话时不串提)
  const draftsRef = useRef<Record<string, string>>({});
  const inputRef = useRef(input);
  inputRef.current = input;
  const prevSessionRef = useRef(sessionId);

  const active = models.find((m) => m.active);
  const hasMessages = messages.length > 0;
  const lastMsg = messages[messages.length - 1];
  const streamingNow = !!lastMsg && lastMsg.role === "assistant" && !!lastMsg.streaming;
  // 处理中指示器：延迟出现 + 即时消失
  // - 任务进行中超过 500ms 才显示（过滤掉收尾窗口的瞬间闪烁）
  // - 一旦条件不满足立即隐藏（无延迟）
  const [showProc, setShowProc] = useState(false);
  const showProcRef = useRef(false);
  useEffect(() => {
    const shouldShow = running && !streamingNow;
    showProcRef.current = shouldShow;
    if (shouldShow) {
      // 延迟出现：收尾窗口短于 500ms 时根本不显示
      const t = setTimeout(() => {
        if (showProcRef.current) setShowProc(true);
      }, 500);
      return () => clearTimeout(t);
    }
    // 即时消失
    setShowProc(false);
  }, [running, streamingNow]);

  const setQueueBoth = (updater: (q: { text: string; attachments: string[] }[]) => { text: string; attachments: string[] }[]) => {
    queueRef.current = updater(queueRef.current);
    setQueue([...queueRef.current]);
  };

  // 向 App 报告当前会话是否有内容 (用于“空对话不重复新建”)
  useEffect(() => {
    onContentChange?.(messages.length > 0);
  }, [messages.length, onContentChange]);

  const refreshModels = () => api.listModels().then(setModels);

  // 外部注入草稿 (如技能页「通过助手创建」) -> 填入输入框
  useEffect(() => {
    if (draft) setInput(draft.text);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [draft?.key]);

  useEffect(() => {
    refreshModels();
    api.getWorkspace().then((w) => setWorkspace(w.workspace));
    api.listSkills().then(setSkills);
  }, [modelsRefreshKey]);

  useEffect(() => {
    // 每个会话独立的输入草稿: 切走时存旧会话、切入时载入新会话 (没有则空)
    if (prevSessionRef.current !== sessionId) {
      draftsRef.current[prevSessionRef.current] = inputRef.current;
    }
    setInput(draftsRef.current[sessionId] ?? "");
    prevSessionRef.current = sessionId;
    setMessages([]);
    streamingText.current = "";
    setRunning(false);
    setTaskDone(true);
    setQueueBoth(() => []);
    let alive = true;
    // 切回正在执行的会话时，同步运行态，保证暂停键可用
    api.isSessionRunning(sessionId).then((r) => { if (alive) { setRunning(r); setTaskDone(!r); } }).catch(() => {});
    api.loadMessages(sessionId).then((raw: Message[]) => {
      if (!alive) return;
      const ui: UiMsg[] = [];
      for (const m of raw) {
        // 按消息内内容顺序构建有序块: 连续文本合并为一个 text 块、连续工具合并为一个 tools 块
        const blocks: UiBlock[] = [];
        let curText = "";
        let curTools: ToolCall[] | null = null;
        for (const b of m.content) {
          if (b.type === "text") {
            if (curTools) { blocks.push({ kind: "tools", tools: curTools }); curTools = null; }
            curText = curText ? curText + "\n" + (b.text ?? "") : (b.text ?? "");
          } else if (b.type === "tool_use") {
            if (curText) { blocks.push({ kind: "text", text: curText }); curText = ""; }
            if (!curTools) curTools = [];
            curTools.push({ id: b.id!, name: b.name!, input: b.input });
          }
        }
        if (curText) blocks.push({ kind: "text", text: curText });
        if (curTools) blocks.push({ kind: "tools", tools: curTools });
        const text = blocks.filter((x) => x.kind === "text").map((x) => x.text).join("\n\n");
        const tools = blocks.flatMap((x) => (x.kind === "tools" ? x.tools : []));
        const results = m.content.filter((b) => b.type === "tool_result");
        if (results.length) {
          // tool_result 可能因上下文压缩(compact)与 tool_use 不再相邻：
          // 先匹配本条消息内的工具，再回退匹配历史消息中的工具 (从后往前)
          for (const r of results) {
            let tc = tools.find((t) => t.id === r.tool_use_id);
            if (!tc) {
              for (let i = ui.length - 1; i >= 0 && !tc; i--) {
                for (const blk of ui[i].blocks ?? []) {
                  if (blk.kind === "tools") {
                    tc = blk.tools.find((t) => t.id === r.tool_use_id);
                    if (tc) break;
                  }
                }
              }
            }
            if (tc) { tc.output = r.content; tc.ok = !r.is_error; }
          }
        }
        if (!blocks.length) continue;
        const last = ui[ui.length - 1];
        if (m.role === "assistant" && last && last.role === "assistant") {
          // 合并连续助手消息: 相邻同类块合并，保持原有顺序
          for (const blk of blocks) {
            const lb = last.blocks[last.blocks.length - 1];
            if (blk.kind === "text" && lb && lb.kind === "text") lb.text += "\n\n" + blk.text;
            else if (blk.kind === "tools" && lb && lb.kind === "tools") lb.tools.push(...blk.tools);
            else last.blocks.push(blk);
          }
          last.text = last.blocks.filter((x) => x.kind === "text").map((x) => x.text).join("\n\n");
        } else {
          ui.push({ role: m.role, text, blocks, tools: tools.length ? tools : undefined });
        }
      }
      setMessages(ui);
    });
    api.getTodos(sessionId).then((t) => { if (alive) setTodos(t); });
    return () => { alive = false; };
  }, [sessionId]);

  // 监听滚动位置，记录用户是否停留在底部 (阈值 80px)
  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    nearBottomRef.current = true;
    const onScroll = () => {
      // 程序自动滚动不更新 nearBottomRef，避免 ReactMarkdown 回流后误判
      if (autoScrollRef.current) return;
      nearBottomRef.current = el.scrollHeight - el.scrollTop - el.clientHeight < 80;
    };
    el.addEventListener("scroll", onScroll, { passive: true });
    return () => el.removeEventListener("scroll", onScroll);
  }, [hasMessages]);

  useEffect(() => {
    if (skipScrollRef.current) {
      skipScrollRef.current = false;
      return;
    }
    // 仅在用户位于底部时自动贴底；流式输出期间高频增量也可靠跟随
    if (!nearBottomRef.current) return;
    const el = scrollRef.current;
    if (!el) return;
    autoScrollRef.current = true;
    el.scrollTop = el.scrollHeight;
    // 下一帧重置标记，允许用户滚动事件恢复判定
    requestAnimationFrame(() => { autoScrollRef.current = false; });
  }, [messages]);

  useTauriEvent<{ session_id: string; text: string }>(EV.DELTA, (p) => {
    if (p.session_id !== sessionId) return;
    streamingText.current += p.text;
    setMessages((prev) => {
      const copy = [...prev];
      const last = copy[copy.length - 1];
      if (last && last.role === "assistant" && last.streaming) {
        // 流式增量: 更新末尾 text 块，保持块顺序
        const b = last.blocks[last.blocks.length - 1];
        if (b && b.kind === "text") b.text = streamingText.current;
        else last.blocks.push({ kind: "text", text: streamingText.current });
      } else {
        copy.push({
          role: "assistant",
          text: streamingText.current,
          blocks: [{ kind: "text", text: streamingText.current }],
          streaming: true,
        });
      }
      return copy;
    });
  });
  useTauriEvent<{ session_id: string; id: string; name: string; input: any }>(EV.TOOL_START, (p) => {
    if (p.session_id !== sessionId) return;
    // streamingText 只跟踪当前轮的文本段，不继承历史轮的文本（旧文本已在 blocks 中）
    streamingText.current = "";
    setMessages((prev) => {
      const copy = [...prev];
      let last = copy[copy.length - 1];
      // 复用当前任务的助手气泡（流式中或已含工具步骤），避免中间文本定稿后
      // 新建气泡，导致后续步骤被顶到文本之后；仅当最后一条是已定稿的纯文本
      // 回复（上一个任务的结论）时才新建气泡
      const hasTools = (last?.blocks ?? []).some((b) => b.kind === "tools");
      if (!last || last.role !== "assistant" || (last.streaming === false && !hasTools)) {
        last = { role: "assistant", text: "", blocks: [], streaming: true };
        copy.push(last);
      } else {
        last.streaming = true;
      }
      // 工具步骤按到达顺序追加为 tools 块（或并入末尾 tools 块）
      const tool: ToolCall = { id: p.id, name: p.name, input: p.input };
      const tb = last.blocks[last.blocks.length - 1];
      if (tb && tb.kind === "tools") tb.tools.push(tool);
      else last.blocks.push({ kind: "tools", tools: [tool] });
      if (!last.tools) last.tools = [];
      last.tools.push(tool);
      return copy;
    });
  });
  useTauriEvent<{ session_id: string; id: string; ok: boolean; output: string }>(EV.TOOL_RESULT, (p) => {
    if (p.session_id !== sessionId) return;
    setMessages((prev) => {
      const copy = [...prev];
      for (let i = copy.length - 1; i >= 0; i--) {
        let found = false;
        for (const blk of copy[i].blocks ?? []) {
          if (blk.kind === "tools") {
            const tc = blk.tools.find((t) => t.id === p.id);
            if (tc) { tc.ok = p.ok; tc.output = p.output; found = true; break; }
          }
        }
        if (found) break;
      }
      return copy;
    });
  });
  useTauriEvent<{ session_id: string; text: string }>(EV.ASSISTANT_MSG, (p) => {
    if (p.session_id !== sessionId) return;
    streamingText.current = "";
    setMessages((prev) => {
      const copy = [...prev];
      const last = copy[copy.length - 1];
      if (last && last.role === "assistant") {
        const blocks = [...(last.blocks ?? [])];
        if (p.text && p.text.trim()) {
          const b = blocks[blocks.length - 1];
          if (b && b.kind === "text") {
            // 定稿: 若文本相同或互为子串则替换，避免重复追加
            if (b.text === p.text || b.text.endsWith(p.text) || p.text.endsWith(b.text)) {
              b.text = p.text;
            } else {
              b.text = b.text + "\n\n" + p.text;
            }
          } else {
            blocks.push({ kind: "text", text: p.text });
          }
        }
        copy[copy.length - 1] = { ...last, blocks, streaming: false };
      } else if (p.text && p.text.trim()) {
        copy.push({
          role: "assistant",
          text: p.text,
          blocks: [{ kind: "text", text: p.text }],
          streaming: false,
        });
      }
      return copy;
    });
  });
  useTauriEvent<{ session_id: string; state: string }>(EV.TURN_STATE, (p) => {
    if (p.session_id !== sessionId) return;
    if (p.state === "running") {
      setRunning(true);
      setTaskDone(false);
      return;
    }
    // idle: 定稿流式气泡，然后自动发送队列中的下一条；
    // 同时把未收到结果事件的工具兜底标记为完成，避免 icon 一直显示处理中
    // (兜底属状态同步，跳过自动滚动，避免输出结束后内容突然跳动)
    skipScrollRef.current = true;
    setMessages((prev) =>
      prev.map((m) => {
        const blocks = m.blocks?.map((b) =>
          b.kind === "tools"
            ? { ...b, tools: b.tools.map((t) => (t.ok === undefined ? { ...t, ok: true } : t)) }
            : b
        );
        return m.streaming
          ? { ...m, blocks, streaming: false }
          : blocks
          ? { ...m, blocks }
          : m;
      })
    );
    if (queueRef.current.length > 0) {
      const next = queueRef.current[0];
      setQueueBoth((q) => q.slice(1));
      void sendComposed(next.text, next.attachments);
    } else {
      setRunning(false);
      setTaskDone(true);
    }
  });
  useTauriEvent<{ session_id: string; todos: TodoItem[] }>(EV.TODO_UPDATE, (p) => {
    if (p.session_id === sessionId) setTodos(p.todos);
  });

  // 实际发送一条消息 (不判断队列)
  const sendComposed = async (text: string, atts: string[]) => {
    setMessages((prev) => [...prev, { role: "user", text, attachments: atts, blocks: [] }]);
    streamingText.current = "";
    setRunning(true);
    setTaskDone(false);
    await api.sendMessage(sessionId, text, atts.length ? atts : undefined);
  };

  // 用户点发送/回车: 运行中则入队，否则立即发送
  const send = () => {
    const content = input.trim();
    if (!content && attachments.length === 0) return;
    const atts = attachments;
    setInput("");
    setAttachments([]);
    if (running) {
      setQueueBoth((q) => [...q, { text: content, attachments: atts }]);
    } else {
      void sendComposed(content, atts);
    }
  };

  const pickFiles = async () => {
    const sel = await open({ multiple: true });
    if (!sel) return;
    const arr = Array.isArray(sel) ? sel : [sel];
    setAttachments((prev) => [...prev, ...arr.filter((a) => !prev.includes(a))]);
  };

  // 选择技能: 向输入框插入一个使用该技能的提示
  const pickSkill = (name: string) => {
    setInput((v) => (v.trim() ? `${v.trimEnd()} ` : "") + `使用技能「${name}」`);
  };

  // 选择定时频率: 以当前输入为执行内容创建定时任务
  const createCron = async (cron: string, label: string) => {
    const promptText = input.trim();
    if (!promptText) {
      onNotice?.("warn", "请先在输入框描述要定时执行的内容");
      return;
    }
    try {
      await api.createCronJob(sessionId, "", cron, promptText, true);
      setInput("");
      onNotice?.("info", `已创建定时任务（${label}）`);
    } catch (e: any) {
      onNotice?.("error", "创建定时任务失败：" + (e?.toString() ?? ""));
    }
  };

  const composer = (
    <Composer
      input={input}
      setInput={setInput}
      onSend={send}
      onCancel={() => api.cancelTurn(sessionId)}
      running={running}
      models={models}
      active={active}
      skills={skills}
      attachments={attachments}
      queue={queue}
      onRemoveQueue={(i) => setQueueBoth((q) => q.filter((_, idx) => idx !== i))}
      onAttach={pickFiles}
      onPickSkill={pickSkill}
      onCreateCron={createCron}
      onRemoveAttach={(p) => setAttachments((prev) => prev.filter((x) => x !== p))}
      onSwitch={async (id) => { await api.setActiveModel(id); refreshModels(); }}
      onRuntime={async (modelId, cw, th) => { await api.setModelRuntime(modelId, cw, th); refreshModels(); }}
      onAddModel={onAddModel}
      onManageModels={onManageModels}
      workspace={workspace}
      showWorkspace={!hasMessages}
      onPickWorkspace={async () => {
        const dir = await open({ directory: true, multiple: false });
        if (typeof dir === "string") { await api.setWorkspace(dir); setWorkspace(dir); }
      }}
    />
  );

  return (
    <div className="h-full flex">
      <div className="flex-1 flex flex-col min-w-0">
        {hasMessages ? (
          <>
            <div className="relative flex-1 min-h-0">
              <div ref={scrollRef} className="h-full overflow-y-auto px-6 py-6">
                <div className="max-w-3xl mx-auto space-y-5">
                  {messages.map((m, i) => (
                    <MessageBubble key={i} msg={m} taskDone={taskDone} todos={todos} workspace={workspace} />
                  ))}
                  {showProc && (
                    <div className="flex items-center gap-2 text-sm text-slate-400">
                      <span className="w-3.5 h-3.5 rounded-full border-2 border-slate-300 border-t-[#8b5cf6] animate-spin shrink-0" />
                      处理中
                      <span className="flex gap-0.5">
                        {[0, 1, 2].map((i) => (
                          <span
                            key={i}
                            className="flow-dot w-1 h-1 rounded-full bg-slate-400"
                            style={{ animationDelay: `${i * 0.2}s` }}
                          />
                        ))}
                      </span>
                    </div>
                  )}
                </div>
              </div>
              {/* 底部渐白遮罩: 越靠近输入框越模糊白，高度约一行文字 (置于滚动容器外，必定显示在内容之上) */}
              <div
                className="pointer-events-none absolute bottom-0 left-0 right-0 h-6"
                style={{
                  backgroundImage:
                    "linear-gradient(to top, #ffffff 0%, rgba(255,255,255,0.65) 55%, rgba(255,255,255,0) 100%)",
                }}
              />
            </div>
            <div className="px-6 pb-5">
              <div className="max-w-[800px] mx-auto">{composer}</div>
            </div>
          </>
        ) : (
          <div className="flex-1 flex flex-col items-center justify-center px-6">
            <div className="w-full max-w-3xl">
              <div key={sessionId} className="greeting-enter greeting-hover flex flex-col items-start mb-6">
                <img
                  src={agentIcon}
                  alt="Laver 智能体形象"
                  className="greeting-avatar w-24 h-24 rounded-2xl object-cover mb-4 select-none"
                  draggable={false}
                />
                <h1 className="text-[26px] font-bold text-slate-800 leading-tight text-left">
                  {greeting()}
                  <br />
                  有什么需要我帮你搞定的？
                </h1>
              </div>
            </div>
            <div className="w-full max-w-[800px]">{composer}</div>
          </div>
        )}
      </div>

      {showRightPanel && (
        <aside className="w-64 shrink-0 border-l border-slate-200 p-4 overflow-y-auto bg-[#fafbfc]">
          {/* 工作目录文件树: 右键可添加到对话上下文或复制路径 */}
          <FileTree
            workspace={workspace}
            onAddAttachment={(p) => setAttachments((prev) => (prev.includes(p) ? prev : [...prev, p]))}
          />
        </aside>
      )}
    </div>
  );
}

/* ---------------- Composer (输入卡片 + 工具栏) ---------------- */

function Composer(props: {
  input: string;
  setInput: (s: string) => void;
  onSend: () => void;
  onCancel: () => void;
  running: boolean;
  models: ModelProfileDto[];
  active?: ModelProfileDto;
  skills: SkillMeta[];
  attachments: string[];
  queue: { text: string; attachments: string[] }[];
  onRemoveQueue: (i: number) => void;
  onAttach: () => void;
  onPickSkill: (name: string) => void;
  onCreateCron: (cron: string, label: string) => void;
  onRemoveAttach: (p: string) => void;
  onSwitch: (id: string) => void;
  onRuntime: (modelId: string, cw?: number, th?: ThinkingLevel) => void;
  onAddModel: () => void;
  onManageModels: () => void;
  workspace: string;
  showWorkspace: boolean;
  onPickWorkspace: () => void;
}) {
  const [showModels, setShowModels] = useState(false);
  const [editModelId, setEditModelId] = useState<string | null>(null);
  const [showPlus, setShowPlus] = useState(false);
  const [plusSub, setPlusSub] = useState<null | "skills" | "cron">(null);
  const closePlus = () => { setShowPlus(false); setPlusSub(null); };
  const editModel = editModelId ? props.models.find((m) => m.id === editModelId) : undefined;
  const wsName = props.workspace ? props.workspace.split(/[\\/]/).pop() : "选择工作目录";

  // 输入框随内容自动增高，最多 7 行，超出则内部滚动
  const taRef = useRef<HTMLTextAreaElement>(null);
  const snapTimer = useRef<number | null>(null);
  useEffect(() => {
    const ta = taRef.current;
    if (!ta) return;
    ta.style.height = "auto";
    const cs = getComputedStyle(ta);
    const lh = parseFloat(cs.lineHeight) || 24;
    const pad = parseFloat(cs.paddingTop) + parseFloat(cs.paddingBottom);
    const maxH = lh * 7 + pad;
    // 有内容时额外预留一行高度，方便继续输入；空输入保持单行
    const extra = props.input.trim() ? lh : 0;
    ta.style.height = Math.min(ta.scrollHeight + extra, maxH) + "px";
    ta.style.overflowY = ta.scrollHeight + extra > maxH + 1 ? "auto" : "hidden";
  }, [props.input]);

  return (
    <div>
      {/* 消息队列 (显示在输入框上方，上一任务结束后自动发送) */}
      {props.queue.length > 0 && (
        <div className="mb-2 space-y-1.5">
          <div className="text-[11px] text-slate-400 px-1">
            队列中·{props.queue.length} 条，将在当前任务结束后自动发送
          </div>
          {props.queue.map((q, i) => (
            <div
              key={i}
              className="flex items-center gap-2 bg-[#f6f3ff] border border-[#e0d9fa] rounded-lg px-3 py-1.5 text-sm text-slate-600"
            >
              <svg viewBox="0 0 24 24" className="w-3.5 h-3.5 shrink-0 text-[#8b5cf6]" fill="none" stroke="currentColor" strokeWidth="1.8">
                <path d="M12 8v4l3 2" />
                <path d="M12 3a9 9 0 100 18 9 9 0 000-18z" />
              </svg>
              <span className="truncate flex-1">
                {q.text || "(仅附件)"}
                {q.attachments.length > 0 && (
                  <span className="text-[11px] text-slate-400 ml-1">📎{q.attachments.length}</span>
                )}
              </span>
              <button
                onClick={() => props.onRemoveQueue(i)}
                className="text-slate-400 hover:text-red-500 shrink-0"
              >
                ✕
              </button>
            </div>
          ))}
        </div>
      )}

    <div className={`border border-slate-200 shadow-[0_2px_16px_rgba(0,0,0,0.05)] bg-white ${props.showWorkspace ? "rounded-t-2xl" : "rounded-2xl"}`}>
      {/* 附件 chips */}
      {props.attachments.length > 0 && (
        <div className="flex flex-wrap gap-2 px-3 pt-3">
          {props.attachments.map((p) => (
            <span
              key={p}
              className="flex items-center gap-1.5 max-w-[220px] bg-slate-100 border border-slate-200 rounded-lg pl-2 pr-1.5 py-1 text-xs text-slate-600"
              title={p}
            >
              <svg viewBox="0 0 24 24" className="w-3.5 h-3.5 shrink-0" fill="none" stroke="currentColor" strokeWidth="1.8">
                <path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" />
                <path d="M14 2v6h6" />
              </svg>
              <span className="truncate">{p.split(/[\\/]/).pop()}</span>
              <button
                onClick={() => props.onRemoveAttach(p)}
                className="text-slate-400 hover:text-red-500 shrink-0"
              >
                ✕
              </button>
            </span>
          ))}
        </div>
      )}

      <div className="px-4 pt-3.5 pb-2">
        <textarea
          ref={taRef}
          value={props.input}
          onChange={(e) => props.setInput(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); props.onSend(); }
          }}
          onScroll={(e) => {
            // 自然滚动；停下后 (防抖) 再平滑吸附到最近整行，避免切半行
            const ta = e.currentTarget;
            if (snapTimer.current) window.clearTimeout(snapTimer.current);
            snapTimer.current = window.setTimeout(() => {
              const lh = 24;
              const snapped = Math.round(ta.scrollTop / lh) * lh;
              if (Math.abs(snapped - ta.scrollTop) > 1) {
                ta.style.scrollBehavior = "smooth";
                ta.scrollTop = snapped;
                window.setTimeout(() => { ta.style.scrollBehavior = "auto"; }, 220);
              }
            }, 140);
          }}
          placeholder="描述任务，输入 / 调用技能…"
          rows={1}
          className="block w-full resize-none bg-transparent text-[15px] leading-6 text-slate-800 placeholder:text-slate-400 focus:outline-none"
        />
      </div>

      <div className="flex items-center gap-1.5 px-3 pb-2.5 relative">
        {/* “+” 添加上下文菜单 */}
        <div className="relative shrink-0">
          <button
            onClick={() => { setShowPlus(!showPlus); setPlusSub(null); setShowModels(false); }}
            className="w-8 h-8 rounded-full border border-slate-200 text-slate-500 hover:bg-slate-100 flex items-center justify-center"
            title="添加上下文"
          >
            <svg viewBox="0 0 24 24" className="w-[18px] h-[18px]" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round">
              <path d="M12 5v14M5 12h14" />
            </svg>
          </button>
          {showPlus && (
            <>
              <div className="fixed inset-0 z-30" onClick={closePlus} />
              <div className="absolute bottom-full mb-2 left-0 w-52 bg-white border border-slate-200 rounded-xl shadow-lg py-1.5 z-40 fade-in">
                {/* 添加文件 */}
                <button
                  onClick={() => { props.onAttach(); closePlus(); }}
                  className="w-full flex items-center gap-2.5 px-3 py-2 text-sm text-slate-600 hover:bg-slate-50 text-left"
                >
                  <svg viewBox="0 0 24 24" className="w-4 h-4 text-slate-400" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
                    <path d="M21.44 11.05l-9.19 9.19a5.5 5.5 0 01-7.78-7.78l9.19-9.19a3.5 3.5 0 014.95 4.95l-9.2 9.19a1.5 1.5 0 01-2.12-2.12l8.49-8.49" />
                  </svg>
                  添加文件
                </button>

                {/* 技能 (子菜单) */}
                <div className="relative">
                  <button
                    onClick={() => setPlusSub(plusSub === "skills" ? null : "skills")}
                    className={`w-full flex items-center gap-2.5 px-3 py-2 text-sm text-left ${plusSub === "skills" ? "bg-slate-50 text-slate-800" : "text-slate-600 hover:bg-slate-50"}`}
                  >
                    <svg viewBox="0 0 24 24" className="w-4 h-4 text-slate-400" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
                      <path d="M12 3l2.5 5 5.5.8-4 3.9.9 5.5L12 21l-4.9 2.6.9-5.5-4-3.9 5.5-.8z" />
                    </svg>
                    <span className="flex-1">技能</span>
                    <span className="text-slate-300">›</span>
                  </button>
                  {plusSub === "skills" && (
                    <div className="absolute left-full ml-1 bottom-0 w-52 bg-white border border-slate-200 rounded-xl shadow-lg py-1.5 max-h-64 overflow-y-auto">
                      {props.skills.length === 0 ? (
                        <div className="px-3 py-2 text-xs text-slate-400">暂无启用的技能</div>
                      ) : (
                        props.skills.map((s) => (
                          <button
                            key={s.name}
                            onClick={() => { props.onPickSkill(s.name); closePlus(); }}
                            className="w-full text-left px-3 py-2 text-sm text-slate-600 hover:bg-slate-50 truncate"
                            title={s.description}
                          >
                            {s.name}
                          </button>
                        ))
                      )}
                    </div>
                  )}
                </div>

                <div className="my-1 border-t border-slate-100" />

                {/* 定时任务 (子菜单) */}
                <div className="relative">
                  <button
                    onClick={() => setPlusSub(plusSub === "cron" ? null : "cron")}
                    className={`w-full flex items-center gap-2.5 px-3 py-2 text-sm text-left ${plusSub === "cron" ? "bg-slate-50 text-slate-800" : "text-slate-600 hover:bg-slate-50"}`}
                  >
                    <svg viewBox="0 0 24 24" className="w-4 h-4 text-slate-400" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
                      <path d="M12 8v4l3 2" />
                      <path d="M12 3a9 9 0 100 18 9 9 0 000-18z" />
                    </svg>
                    <span className="flex-1">定时任务</span>
                    <span className="text-slate-300">›</span>
                  </button>
                  {plusSub === "cron" && (
                    <div className="absolute left-full ml-1 bottom-0 w-44 bg-white border border-slate-200 rounded-xl shadow-lg py-1.5">
                      {CRON_PRESETS.map((c) => (
                        <button
                          key={c.cron}
                          onClick={() => { props.onCreateCron(c.cron, c.label); closePlus(); }}
                          className="w-full text-left px-3 py-2 text-sm text-slate-600 hover:bg-slate-50"
                        >
                          {c.label}
                        </button>
                      ))}
                    </div>
                  )}
                </div>
              </div>
            </>
          )}
        </div>

        <div className="flex-1" />

        {/* 模型切换 + 编辑配置 (发送键左侧) */}
        <div className="relative shrink-0">
          <button
            onClick={() => { setShowModels((v) => !v); setEditModelId(null); setShowPlus(false); setPlusSub(null); }}
            className="flex items-center gap-1.5 text-sm text-slate-700 hover:bg-slate-100 rounded-full px-3 py-1.5"
          >
            {props.active ? props.active.name : "未选择模型"}
            <Chevron />
          </button>
          {showModels && (
            <>
              <div className="fixed inset-0 z-10" onClick={() => { setShowModels(false); setEditModelId(null); }} />
              <div className="absolute bottom-full mb-2 right-0 w-60 bg-white border border-slate-200 rounded-xl shadow-lg z-20 fade-in">
                {/* 左: 模型列表 (行右侧“编辑”打开配置) */}
                <div className="py-1.5 max-h-80 overflow-y-auto">
                  <div className="px-3 py-1 text-[11px] text-slate-400">切换模型</div>
                  {props.models.length === 0 && (
                    <div className="px-3 py-2 text-sm text-slate-400">暂无模型</div>
                  )}
                  {props.models.map((m) => (
                    <div
                      key={m.id}
                      className={`group flex items-center justify-between pl-3 pr-2 py-2 text-sm hover:bg-slate-50 ${editModelId === m.id ? "bg-slate-50" : ""}`}
                    >
                      <button onClick={() => props.onSwitch(m.id)} className="flex items-center gap-2 min-w-0 flex-1 text-left">
                        {m.active ? (
                          <span className="text-[#8b5cf6] text-xs w-3 shrink-0">✓</span>
                        ) : (
                          <span className="w-3 shrink-0" />
                        )}
                        <span className="truncate text-slate-700">{m.name}</span>
                        {!m.has_key && <span className="text-[10px] text-amber-600 shrink-0">无密钥</span>}
                      </button>
                      <button
                        onClick={() => setEditModelId(editModelId === m.id ? null : m.id)}
                        className={`ml-2 shrink-0 flex items-center gap-1 text-xs hover:text-[#8b5cf6] ${editModelId === m.id ? "text-[#8b5cf6]" : "text-slate-400"}`}
                        title="配置上下文与思考模式"
                      >
                        <svg viewBox="0 0 24 24" className="w-3.5 h-3.5" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round"><path d="M12 20h9" /><path d="M16.5 3.5a2.1 2.1 0 013 3L7 19l-4 1 1-4z" /></svg>
                        编辑
                      </button>
                    </div>
                  ))}
                  <div className="border-t border-slate-100 mt-1 pt-1">
                    <button
                      onClick={() => { props.onAddModel(); setShowModels(false); }}
                      className="w-full flex items-center gap-2 px-3 py-2 text-sm text-slate-600 hover:bg-slate-50 text-left"
                    >
                      <svg viewBox="0 0 24 24" className="w-4 h-4 text-slate-400 shrink-0" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="3" /><path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 11-2.83 2.83l-.06-.06a1.65 1.65 0 00-2.82 1.17V21a2 2 0 11-4 0v-.09A1.65 1.65 0 006 19.4l-.06.06a2 2 0 11-2.83-2.83l.06-.06A1.65 1.65 0 004.6 15H4.5a2 2 0 110-4h.09A1.65 1.65 0 006 9.4" /></svg>
                      添加模型
                    </button>
                  </div>
                </div>
                {/* 右: 纵向配置浮层 (点“编辑”后自动出现在列表右侧，不改变列表尺寸/位置) */}
                {editModel && (
                  <div className="absolute left-full top-0 ml-2 w-52 bg-white border border-slate-200 rounded-xl shadow-lg py-1.5 max-h-80 overflow-y-auto">
                    <div className="px-3 py-1 text-sm font-medium text-slate-800 truncate">{editModel.name}</div>
                    <div className="px-3 pt-1.5 pb-0.5 text-[11px] text-slate-400">上下文窗口</div>
                    {CONTEXT_TIERS.map((tier) => (
                      <button
                        key={tier.value}
                        onClick={() => props.onRuntime(editModel.id, tier.value, undefined)}
                        className="w-full flex items-center justify-between px-3 py-1.5 text-sm text-slate-700 hover:bg-slate-50"
                      >
                        <span>{tier.label}</span>
                        {editModel.context_window === tier.value && <span className="text-[#8b5cf6] text-xs">✓</span>}
                      </button>
                    ))}
                    <div className="border-t border-slate-100 my-1" />
                    <div className="flex items-center justify-between px-3 py-1.5">
                      <span className="text-[11px] text-slate-400">思考模式</span>
                      <span
                        onClick={() => props.onRuntime(editModel.id, undefined, editModel.thinking === "off" ? "medium" : "off")}
                        className={`w-9 h-5 rounded-full relative cursor-pointer transition ${editModel.thinking !== "off" ? "bg-[#8b5cf6]" : "bg-slate-300"}`}
                      >
                        <span className="absolute top-0.5 w-4 h-4 bg-white rounded-full transition-all" style={{ left: editModel.thinking !== "off" ? "18px" : "2px" }} />
                      </span>
                    </div>
                    {editModel.thinking !== "off" &&
                      (["low", "medium", "high"] as ThinkingLevel[]).map((lv) => (
                        <button
                          key={lv}
                          onClick={() => props.onRuntime(editModel.id, undefined, lv)}
                          className="w-full flex items-center justify-between px-3 py-1.5 text-sm text-slate-700 hover:bg-slate-50"
                        >
                          <span>{THINKING_LABELS[lv]}</span>
                          {editModel.thinking === lv && <span className="text-[#8b5cf6] text-xs">✓</span>}
                        </button>
                      ))}
                    {!editModel.supports_thinking && (
                      <div className="px-3 pt-1 text-[11px] text-amber-600">该模型未标记支持思考</div>
                    )}
                  </div>
                )}
              </div>
            </>
          )}
        </div>

        {/* 发送 / 停止 / 入队 */}
        {props.running && !props.input.trim() && props.attachments.length === 0 ? (
          <button
            onClick={props.onCancel}
            className="w-9 h-9 rounded-full bg-red-500 hover:bg-red-600 text-white flex items-center justify-center"
            title="停止当前任务"
          >
            <span className="w-3 h-3 bg-white rounded-[2px]" />
          </button>
        ) : (
          <button
            onClick={props.onSend}
            className="w-9 h-9 rounded-full bg-[#8b5cf6] hover:bg-[#7c3aed] text-white flex items-center justify-center disabled:opacity-40"
            disabled={!props.input.trim() && props.attachments.length === 0}
            title={props.running ? "加入队列，任务结束后自动发送" : "发送"}
          >
            {props.running ? (
              <svg viewBox="0 0 24 24" className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round">
                <path d="M12 5v14M5 12h14" />
              </svg>
            ) : (
              <svg viewBox="0 0 24 24" className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
                <path d="M12 19V5M5 12l7-7 7 7" />
              </svg>
            )}
          </button>
        )}
      </div>
    </div>

      {/* 工作目录 (仅空会话显示; 一旦对话过就隐藏，不再允许切换) */}
      {props.showWorkspace && (
        <div className="bg-slate-100 rounded-b-2xl px-3 py-2 flex items-center">
          <button
            onClick={props.onPickWorkspace}
            className="flex items-center gap-1.5 text-xs text-slate-500 hover:text-slate-700"
            title={props.workspace}
          >
            <svg viewBox="0 0 24 24" className="w-3.5 h-3.5" fill="none" stroke="currentColor" strokeWidth="1.8">
              <path d="M3 7a2 2 0 012-2h4l2 2h8a2 2 0 012 2v8a2 2 0 01-2 2H5a2 2 0 01-2-2z" />
            </svg>
            {wsName}
            <Chevron />
          </button>
        </div>
      )}
    </div>
  );
}

function Chevron() {
  return (
    <svg viewBox="0 0 24 24" className="w-3.5 h-3.5" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M6 9l6 6 6-6" />
    </svg>
  );
}

/* ---------------- 消息气泡 ---------------- */

// 把文本中指向工作区的绝对路径转为可点击链接（显示相对路径，点击打开目录）
function pathToLinks(text: string, workspace: string): string {
  if (!workspace) return text;
  const ws = workspace.replace(/[\\/]+$/, "").toLowerCase();
  // 匹配 Windows/Unix 绝对路径：允许被反引号包裹（行内代码），但生成链接时剥掉反引号，
  // 避免链接仍被包在行内代码里导致无法解析
  const re = /`*[A-Za-z]:[\\/][^\s"'<>|?*，。；、：（）()【】`]+`*/g;
  let changed = false;
  const out = text.replace(re, (raw) => {
    const abs = raw.replace(/`/g, "");
    const norm = abs.replace(/[\\/]+/g, "\\");
    if (!norm.toLowerCase().startsWith(ws)) return raw;
    const rel = norm.slice(ws.length).replace(/^[\\/]+/, "");
    changed = true;
    // 使用 open: 协议 (opaque 形式，编码后任意字符均合法)
    return `[${rel}](open:${encodeURIComponent(abs)})`;
  });
  return changed ? out : text;
}

function MessageBubble({ msg, taskDone, todos, workspace }: { msg: UiMsg; taskDone: boolean; todos: TodoItem[]; workspace: string }) {
  const [copied, setCopied] = useState<null | "text" | "md">(null);
  const textBlocks = msg.blocks.filter((b) => b.kind === "text").map((b) => b.text);
  const hasText = textBlocks.length > 0;
  // 待办折叠行只显示在最后一个“更新待办”工具步骤下方，位置固定不随新内容移动
  const lastTodoIdx = msg.blocks.reduce(
    (acc, b, i) => (b.kind === "tools" && b.tools.some((t) => t.name === "todo_write") ? i : acc),
    -1
  );
  const copy = async (kind: "text" | "md") => {
    const val = textBlocks.join("\n\n");
    try {
      await navigator.clipboard.writeText(val);
      setCopied(kind);
      setTimeout(() => setCopied(null), 1500);
    } catch {
      /* 忽略复制失败 */
    }
  };

  if (msg.role === "user") {
    return (
      <div className="flex justify-end">
        <div className="max-w-3xl flex flex-col items-end gap-1.5">
          {msg.attachments && msg.attachments.length > 0 && (
            <div className="flex flex-wrap gap-1.5 justify-end">
              {msg.attachments.map((p) => (
                <span
                  key={p}
                  className="flex items-center gap-1 bg-[#f0e9ff] text-[#6d28d9] rounded-lg px-2 py-1 text-xs max-w-[220px]"
                  title={p}
                >
                  <svg viewBox="0 0 24 24" className="w-3 h-3 shrink-0" fill="none" stroke="currentColor" strokeWidth="1.8">
                    <path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" />
                    <path d="M14 2v6h6" />
                  </svg>
                  <span className="truncate">{p.split(/[\\/]/).pop()}</span>
                </span>
              ))}
            </div>
          )}
          {msg.text && (
            <div className="bg-[#eeeeee] text-slate-900 rounded-2xl rounded-br-md px-4 py-2.5 text-sm whitespace-pre-wrap">
              {msg.text}
            </div>
          )}
        </div>
      </div>
    );
  }
  return (
    <div className="flex justify-start">
      <div className="max-w-full w-full space-y-2">
        {/* 按到达顺序渲染内容块: 文本与工具步骤交替展示，保持处理顺序 */}
        {msg.blocks.map((b, i) =>
          b.kind === "tools" ? (
            <Fragment key={i}>
              <ToolActivity tools={b.tools} />
              {i === lastTodoIdx && todos.length > 0 && <TodoPanel todos={todos} />}
            </Fragment>
          ) : (
            <div key={i} className="text-[15px] text-slate-800 md leading-relaxed">
              <ReactMarkdown
                remarkPlugins={[remarkGfm]}
                urlTransform={(url) => url}
                components={{
                  a: ({ href, children }) => {
                    // 工作区文件链接: 蓝色显示，点击在系统文件管理器中打开/定位
                    if (href?.startsWith("open:")) {
                      const p = decodeURIComponent(href.slice(5));
                      return (
                        <a
                          href="#"
                          className="text-blue-600 hover:underline cursor-pointer break-all"
                          onClick={(e) => {
                            e.preventDefault();
                            e.stopPropagation();
                            void api.openPath(p);
                          }}
                        >
                          {children}
                        </a>
                      );
                    }
                    return <a href={href}>{children}</a>;
                  },
                }}
              >
                {pathToLinks(b.text, workspace)}
              </ReactMarkdown>
            </div>
          )
        )}
        {/* AI 回复末尾: 复制文本 / 复制 Markdown (任务结束后才显示，淡入避免突兀) */}
        {hasText && !msg.streaming && taskDone && (
          <div className="flex items-center gap-1 pt-0.5 fade-in">
            <button
              onClick={() => copy("text")}
              title="复制文本"
              className="w-7 h-7 rounded-md flex items-center justify-center text-slate-400 hover:text-slate-700 hover:bg-slate-100"
            >
              {copied === "text" ? (
                <svg viewBox="0 0 24 24" className="w-4 h-4 text-[#8b5cf6]" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M20 6L9 17l-5-5" /></svg>
              ) : (
                <svg viewBox="0 0 24 24" className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round"><rect x="9" y="9" width="13" height="13" rx="2" /><path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1" /></svg>
              )}
            </button>
            <button
              onClick={() => copy("md")}
              title="复制 Markdown"
              className="w-7 h-7 rounded-md flex items-center justify-center text-slate-400 hover:text-slate-700 hover:bg-slate-100"
            >
              {copied === "md" ? (
                <svg viewBox="0 0 24 24" className="w-4 h-4 text-[#8b5cf6]" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M20 6L9 17l-5-5" /></svg>
              ) : (
                <svg viewBox="0 0 16 16" className="w-4 h-4" fill="currentColor"><path d="M14.85 3H1.15C.52 3 0 3.52 0 4.15v7.69C0 12.48.52 13 1.15 13h13.69c.64 0 1.15-.52 1.15-1.15v-7.7C16 3.52 15.48 3 14.85 3zM9 11H7V8L5.5 9.92 4 8v3H2V5h2l1.5 2L7 5h2v6zm2.99.5L9.5 8H11V5h2v3h1.5l-2.51 3.5z" /></svg>
              )}
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

/* ---------------- 待办清单面板 (消息流内展示，替代 JSON 文本) ---------------- */

function TodoPanel({ todos }: { todos: TodoItem[] }) {
  const [open, setOpen] = useState(false);
  const done = todos.filter((t) => t.status === "completed").length;
  return (
    <div>
      <button
        onClick={() => setOpen((v) => !v)}
        className="flex items-center gap-1.5 text-slate-500 hover:text-slate-700 py-0.5"
      >
        <svg viewBox="0 0 24 24" className="w-3.5 h-3.5 shrink-0 text-[#8b5cf6]" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><path d="M9 6h11M9 12h11M9 18h11" /><path d="M3 6l1 1 2-2M3 12l1 1 2-2M3 18l1 1 2-2" /></svg>
        <span className="text-sm">更新待办 {todos.length} 项</span>
        <span className="text-[10px] text-slate-400 ml-auto">{done}/{todos.length} 完成</span>
        <svg viewBox="0 0 24 24" className={`w-3.5 h-3.5 text-slate-400 transition-transform shrink-0 ${open ? "" : "-rotate-90"}`} fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M6 9l6 6 6-6" /></svg>
      </button>
      {open && (
        <div className="mt-1 ml-[7px] border-l-2 border-slate-100 pl-3 space-y-1.5">
          {todos.map((t, i) => (
            <div key={i} className="flex items-start gap-2 text-sm">
              <span className="mt-0.5 shrink-0">
                {t.status === "in_progress" ? (
                  <span className="inline-block w-3.5 h-3.5 rounded-full border-2 border-slate-300 border-t-[#8b5cf6] animate-spin" />
                ) : t.status === "completed" ? (
                  <span className="inline-flex w-3.5 h-3.5 rounded-full bg-[#8b5cf6] items-center justify-center">
                    <svg viewBox="0 0 24 24" className="w-2.5 h-2.5 text-white" fill="none" stroke="currentColor" strokeWidth="3.5" strokeLinecap="round" strokeLinejoin="round"><path d="M20 6L9 17l-5-5" /></svg>
                  </span>
                ) : (
                  <span className="inline-block w-3.5 h-3.5 rounded-full border-2 border-slate-300" />
                )}
              </span>
              <span
                className={
                  t.status === "completed"
                    ? "text-slate-400 line-through"
                    : t.status === "in_progress"
                    ? "text-[#8b5cf6] font-medium"
                    : "text-slate-600"
                }
              >
                {t.content}
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

/* ---------------- 工作目录文件树 ---------------- */

function FileTree({
  workspace,
  onAddAttachment,
}: {
  workspace: string;
  onAddAttachment: (path: string) => void;
}) {
  const [tree, setTree] = useState<DirEntry[] | null>(null);
  // 右键菜单: 位置 + 目标节点
  const [menu, setMenu] = useState<{ x: number; y: number; path: string; isDir: boolean } | null>(null);
  useEffect(() => {
    setTree(null);
    if (!workspace) return;
    api.listDirTree(workspace).then(setTree).catch(() => setTree(null));
  }, [workspace]);
  // 点击/右键其他区域时关闭菜单
  useEffect(() => {
    if (!menu) return;
    const close = () => setMenu(null);
    window.addEventListener("click", close);
    window.addEventListener("contextmenu", close);
    return () => {
      window.removeEventListener("click", close);
      window.removeEventListener("contextmenu", close);
    };
  }, [menu]);
  const handleContext = (e: ReactMouseEvent, node: DirEntry) => {
    e.preventDefault();
    e.stopPropagation();
    setMenu({
      x: Math.min(e.clientX, window.innerWidth - 170),
      y: Math.min(e.clientY, window.innerHeight - 96),
      path: node.path,
      isDir: node.is_dir,
    });
  };
  return (
    <div>
      <h3 className="text-xs font-semibold text-slate-500 uppercase mb-2">工作目录</h3>
      {!workspace ? (
        <div className="text-xs text-slate-400">未选择工作目录</div>
      ) : !tree ? (
        <div className="text-xs text-slate-400">加载中…</div>
      ) : tree.length === 0 ? (
        <div className="text-xs text-slate-400">空目录</div>
      ) : (
        <div className="space-y-0.5">
          {tree.map((n) => (
            <TreeNode key={n.path} node={n} depth={0} onContext={handleContext} />
          ))}
        </div>
      )}
      {/* 右键菜单 */}
      {menu && (
        <div
          className="fixed z-50 bg-white border border-slate-200 rounded-lg shadow-lg py-1 min-w-[150px]"
          style={{ left: menu.x, top: menu.y }}
        >
          {!menu.isDir && (
            <button
              onClick={() => {
                onAddAttachment(menu.path);
                setMenu(null);
              }}
              className="w-full text-left px-3 py-1.5 text-xs text-slate-600 hover:bg-slate-100"
            >
              添加到对话上下文
            </button>
          )}
          <button
            onClick={async () => {
              try {
                await navigator.clipboard.writeText(menu.path);
              } catch {
                /* 忽略复制失败 */
              }
              setMenu(null);
            }}
            className="w-full text-left px-3 py-1.5 text-xs text-slate-600 hover:bg-slate-100"
          >
            复制路径
          </button>
        </div>
      )}
    </div>
  );
}

function TreeNode({
  node,
  depth,
  onContext,
}: {
  node: DirEntry;
  depth: number;
  onContext: (e: ReactMouseEvent, node: DirEntry) => void;
}) {
  const [open, setOpen] = useState(false);
  return (
    <div>
      <button
        onClick={() => node.is_dir && setOpen((v) => !v)}
        onContextMenu={(e) => onContext(e, node)}
        className={`w-full flex items-center gap-1 text-xs py-0.5 rounded hover:bg-slate-100 ${node.is_dir ? "text-slate-600" : "text-slate-500"}`}
        style={{ paddingLeft: depth * 14 }}
        title={node.path}
      >
        {node.is_dir ? (
          <>
            <svg viewBox="0 0 24 24" className={`w-2.5 h-2.5 text-slate-400 transition-transform shrink-0 ${open ? "" : "-rotate-90"}`} fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M6 9l6 6 6-6" /></svg>
            <svg viewBox="0 0 24 24" className="w-3.5 h-3.5 shrink-0 text-[#8b5cf6]" fill="none" stroke="currentColor" strokeWidth="1.8"><path d="M3 7a2 2 0 012-2h4l2 2h8a2 2 0 012 2v8a2 2 0 01-2 2H5a2 2 0 01-2-2z" /></svg>
          </>
        ) : (
          <>
            <span className="w-2.5 shrink-0" />
            <svg viewBox="0 0 24 24" className="w-3.5 h-3.5 shrink-0 text-slate-400" fill="none" stroke="currentColor" strokeWidth="1.8"><path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" /><path d="M14 2v6h6" /></svg>
          </>
        )}
        <span className="truncate">{node.name}</span>
      </button>
      {node.is_dir && open && node.children && node.children.length > 0 && (
        <div>
          {node.children.map((c) => (
            <TreeNode key={c.path} node={c} depth={depth + 1} onContext={onContext} />
          ))}
        </div>
      )}
    </div>
  );
}

/* ---------------- 工具活动列表 (紧凑) ---------------- */

const truncTxt = (s: string, n: number) => (s.length > n ? s.slice(0, n) + "…" : s);
const baseName = (p?: string) => (p ? p.split(/[\\/]/).pop() : "");

function toolLabel(t: ToolCall): string {
  const i: any = t.input || {};
  switch (t.name) {
    case "read_file": return `读取文件 ${baseName(i.path) ?? ""}`.trim();
    case "write_file": return `写入文件 ${baseName(i.path) ?? ""}`.trim();
    case "edit_file": return `编辑文件 ${baseName(i.path) ?? ""}`.trim();
    case "glob": return `查找文件 ${i.pattern ?? ""}`.trim();
    case "shell": return "执行命令" + (i.command ? `：${truncTxt(String(i.command), 48)}` : "");
    case "todo_write": return Array.isArray(i.todos) ? `更新待办 ${i.todos.length} 项` : "更新待办";
    case "load_skill": return `加载技能 ${i.name ?? ""}`.trim();
    case "remember": return "更新记忆";
    case "task_create":
    case "task": return "创建任务" + (i.description ? `：${truncTxt(String(i.description), 36)}` : "");
    case "task_list": return "查看任务列表";
    case "task_claim": return "认领任务";
    case "task_complete": return "完成任务";
    case "cron_schedule": return "设置定时任务";
    case "cron_list": return "查看定时任务";
    case "send_message": return "发送消息";
    case "spawn_teammate": return "创建协作队友";
    case "create_worktree": return "创建工作区";
    default: return t.name;
  }
}

function ToolStatusIcon({ status }: { status: "running" | "ok" | "err" }) {
  if (status === "running")
    return <span className="w-3.5 h-3.5 rounded-full border-2 border-slate-300 border-t-[#8b5cf6] animate-spin shrink-0" />;
  if (status === "err")
    return (
      <svg viewBox="0 0 24 24" className="w-3.5 h-3.5 text-amber-500 shrink-0" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round"><path d="M18 6L6 18M6 6l12 12" /></svg>
    );
  return (
    <svg viewBox="0 0 24 24" className="w-3.5 h-3.5 text-slate-400 shrink-0" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round"><path d="M20 6L9 17l-5-5" /></svg>
  );
}

function ToolActivity({ tools }: { tools: ToolCall[] }) {
  const [open, setOpen] = useState(true);
  const running = tools.some((t) => t.ok === undefined);
  const done = tools.filter((t) => t.ok !== undefined).length;
  return (
    <div className="text-sm">
      <button
        onClick={() => setOpen((v) => !v)}
        className="flex items-center gap-1.5 text-slate-500 hover:text-slate-700 py-0.5"
      >
        {running ? (
          <span className="w-3.5 h-3.5 rounded-full border-2 border-slate-300 border-t-[#8b5cf6] animate-spin" />
        ) : (
          <svg viewBox="0 0 24 24" className="w-3.5 h-3.5 text-[#8b5cf6]" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round"><path d="M20 6L9 17l-5-5" /></svg>
        )}
        <span>{running ? `已完成 ${done} 个步骤…` : `已完成 ${tools.length} 个步骤`}</span>
        <svg viewBox="0 0 24 24" className={`w-3.5 h-3.5 transition-transform ${open ? "" : "-rotate-90"}`} fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M6 9l6 6 6-6" /></svg>
      </button>
      {open && (
        <div className="mt-1 ml-[7px] border-l-2 border-slate-100 pl-3 space-y-0.5">
          {tools.map((t) => (
            <ToolLine key={t.id} tool={t} />
          ))}
        </div>
      )}
    </div>
  );
}

function ToolLine({ tool }: { tool: ToolCall }) {
  const [open, setOpen] = useState(false);
  const status = tool.ok === undefined ? "running" : tool.ok ? "ok" : "err";
  // todo_write 的 JSON 参数与长文案不在界面展开，避免输出原始 JSON
  const hideDetail = tool.name === "todo_write";
  const hasDetail = !hideDetail && ((tool.input && Object.keys(tool.input).length > 0) || !!tool.output);
  return (
    <div>
      <button
        onClick={() => hasDetail && setOpen((v) => !v)}
        className={`w-full flex items-center gap-2 py-0.5 text-left text-slate-600 ${hasDetail ? "hover:text-slate-800 cursor-pointer" : "cursor-default"}`}
      >
        <ToolStatusIcon status={status} />
        <span className="truncate">{toolLabel(tool)}</span>
      </button>
      {open && hasDetail && (
        <div className="ml-[22px] my-1 rounded-lg bg-slate-50 border border-slate-100 p-2 text-xs text-slate-500 space-y-1 max-h-64 overflow-auto">
          {tool.input && Object.keys(tool.input).length > 0 && (
            <pre className="whitespace-pre-wrap break-all">{JSON.stringify(tool.input, null, 2)}</pre>
          )}
          {tool.output && (
            <pre className="whitespace-pre-wrap break-all border-t border-slate-200 pt-1">{truncTxt(tool.output, 4000)}</pre>
          )}
        </div>
      )}
    </div>
  );
}

function ToolCard({ tool }: { tool: ToolCall }) {
  const [open, setOpen] = useState(false);
  const status = tool.ok === undefined ? "running" : tool.ok ? "ok" : "err";
  return (
    <div className="bg-slate-50 border border-slate-200 rounded-lg text-xs overflow-hidden">
      <button onClick={() => setOpen(!open)} className="w-full flex items-center gap-2 px-3 py-2 hover:bg-slate-100">
        <span>{status === "running" ? "⏳" : status === "ok" ? "🔧" : "⚠️"}</span>
        <span className="font-mono text-slate-700">{tool.name}</span>
        <span className="text-slate-400 truncate flex-1 text-left">{summarizeInput(tool.input)}</span>
        <span className="text-slate-400">{open ? "▲" : "▼"}</span>
      </button>
      {open && (
        <div className="px-3 pb-3 space-y-2 border-t border-slate-200">
          <div>
            <div className="text-slate-400 mt-2 mb-1">参数</div>
            <pre className="bg-white border border-slate-200 rounded p-2 overflow-x-auto text-slate-700">
              {JSON.stringify(tool.input, null, 2)}
            </pre>
          </div>
          {tool.output !== undefined && (
            <div>
              <div className="text-slate-400 mb-1">结果</div>
              <pre className={`bg-white border border-slate-200 rounded p-2 overflow-x-auto max-h-64 ${tool.ok ? "text-slate-700" : "text-red-600"}`}>
                {tool.output}
              </pre>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function summarizeInput(input: any): string {
  if (!input) return "";
  if (input.command) return input.command;
  if (input.path) return input.path;
  if (input.pattern) return input.pattern;
  if (input.name) return input.name;
  if (input.subject) return input.subject;
  return JSON.stringify(input).slice(0, 80);
}
