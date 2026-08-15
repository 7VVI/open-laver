import { useEffect, useRef, useState } from "react";
import { api, DesignConfig, DesignItem, DesignKind } from "../lib/api";

interface ModeMeta {
  id: DesignKind;
  label: string;
  desc: string;
  placeholder: string;
  examples: string[];
}

const MODES: ModeMeta[] = [
  {
    id: "icon",
    label: "图标设计",
    desc: "生成应用/产品图标",
    placeholder: "例如：一个 AI 编程助手的应用图标，绿色渐变，代码括号元素，扁平风格…",
    examples: [
      "AI 编程助手图标，蓝色渐变，终端与代码括号元素",
      "笔记应用图标，圆角纸张与笔，暖黄色",
      "记账应用图标，圆形算盘与绿色叶片",
    ],
  },
  {
    id: "ip",
    label: "IP 形象",
    desc: "生成品牌吉祥物 / 虚拟形象",
    placeholder: "例如：一只圆润的小浣熊，戴圆框眼镜、蓝色围巾，可爱亲和…",
    examples: [
      "圆润小浣熊，戴圆框眼镜和蓝色围巾，可爱",
      "科技感机器人吉祥物，圆润机身，大眼睛，蓝白配色",
      "国潮风小老虎，红色围脖，微笑表情",
    ],
  },
  {
    id: "prototype",
    label: "原型设计",
    desc: "生成可交互的高保真原型",
    placeholder: "例如：一个个人记账桌面应用，包含总览仪表盘、收支明细、分类统计…",
    examples: [
      "个人记账桌面应用：总览仪表盘、收支明细、分类统计",
      "项目管理后台：任务看板、成员列表、数据图表",
      "电商 App 首页：搜索、轮播、商品瀑布流、底部导航",
    ],
  },
];

const ICON_STYLES = ["扁平", "渐变", "3D", "拟物", "像素", "插画", "极简", "手绘"];
const IP_STYLES = ["可爱", "科技", "国潮", "简约", "3D", "卡通", "像素", "扁平"];
const PLATFORMS = ["桌面应用", "移动 App", "网页", "后台管理", "数据看板"];
const SIZES = ["1024x1024", "1280x720", "720x1280"];

const KIND_LABEL: Record<DesignKind, string> = {
  icon: "图标",
  ip: "IP 形象",
  prototype: "原型",
};

function kindBadgeCls(kind: DesignKind): string {
  switch (kind) {
    case "icon":
      return "bg-sky-100 text-sky-700";
    case "ip":
      return "bg-violet-100 text-violet-700";
    case "prototype":
      return "bg-emerald-100 text-emerald-700";
  }
}

function KindGlyph({ kind }: { kind: DesignKind }) {
  const d =
    kind === "icon"
      ? "M12 2l2.4 7.4H22l-6 4.6 2.3 7.4-6.3-4.6L5.7 21 8 14 2 9.4h7.6z"
      : kind === "ip"
      ? "M12 12a4 4 0 100-8 4 4 0 000 8z|M4 21c0-4 3.6-6 8-6s8 2 8 6"
      : "M3 3h18v18H3z|M3 9h18|M9 21V9";
  return (
    <svg
      viewBox="0 0 24 24"
      className="w-5 h-5"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      {d.split("|").map((p, i) => (
        <path key={i} d={p} />
      ))}
    </svg>
  );
}

export default function DesignView({
  onNotice,
  onOpenSettings,
}: {
  onNotice?: (level: string, text: string) => void;
  onOpenSettings?: () => void;
}) {
  const [mode, setMode] = useState<DesignKind>("icon");
  const [prompt, setPrompt] = useState("");
  const [style, setStyle] = useState("");
  const [platform, setPlatform] = useState(PLATFORMS[0]);
  const [size, setSize] = useState(SIZES[0]);
  const [generating, setGenerating] = useState(false);
  const [items, setItems] = useState<DesignItem[]>([]);
  const [selected, setSelected] = useState<DesignItem | null>(null);
  const [preview, setPreview] = useState<{
    kind: DesignKind;
    url: string;
    isHtml: boolean;
  } | null>(null);
  const [config, setConfig] = useState<DesignConfig | null>(null);
  const previewUrlRef = useRef<string | null>(null);

  const meta = MODES.find((m) => m.id === mode)!;

  const refreshItems = async () => {
    try {
      setItems(await api.listDesigns());
    } catch {
      /* 忽略 */
    }
  };

  useEffect(() => {
    api
      .getDesignConfig()
      .then(setConfig)
      .catch(() => {});
    refreshItems();
    return () => {
      if (previewUrlRef.current) URL.revokeObjectURL(previewUrlRef.current);
    };
  }, []);

  const loadPreview = async (item: DesignItem) => {
    const dataUrl = await api.readDesign(item.id);
    if (item.kind === "prototype") {
      const b64 = dataUrl.slice(dataUrl.indexOf(",") + 1);
      const bin = atob(b64);
      const bytes = new Uint8Array(bin.length);
      for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
      const text = new TextDecoder("utf-8").decode(bytes);
      const url = URL.createObjectURL(new Blob([text], { type: "text/html" }));
      if (previewUrlRef.current) URL.revokeObjectURL(previewUrlRef.current);
      previewUrlRef.current = url;
      setPreview({ kind: "prototype", url, isHtml: true });
    } else {
      if (previewUrlRef.current) URL.revokeObjectURL(previewUrlRef.current);
      previewUrlRef.current = null;
      setPreview({ kind: item.kind, url: dataUrl, isHtml: false });
    }
  };

  const selectItem = async (item: DesignItem) => {
    setSelected(item);
    try {
      await loadPreview(item);
    } catch (e: any) {
      onNotice?.("error", "预览失败：" + (e?.toString() ?? ""));
    }
  };

  const generate = async () => {
    const text = prompt.trim();
    if (!text || generating) return;
    setGenerating(true);
    try {
      const item = await api.generateDesign(
        mode,
        text,
        mode === "prototype" ? platform : style || undefined,
        mode === "prototype" ? undefined : size
      );
      setItems((prev) => [item, ...prev]);
      setSelected(item);
      await loadPreview(item);
      if (item.note) {
        onNotice?.("info", item.note);
      } else {
        onNotice?.("info", "设计生成完成");
      }
    } catch (e: any) {
      onNotice?.("error", e?.toString?.() ?? "生成失败");
    } finally {
      setGenerating(false);
    }
  };

  const remove = async (item: DesignItem) => {
    try {
      await api.deleteDesign(item.id);
      setItems((prev) => prev.filter((i) => i.id !== item.id));
      if (selected?.id === item.id) {
        setSelected(null);
        if (previewUrlRef.current) URL.revokeObjectURL(previewUrlRef.current);
        previewUrlRef.current = null;
        setPreview(null);
      }
      onNotice?.("info", "已删除");
    } catch (e: any) {
      onNotice?.("error", "删除失败：" + (e?.toString() ?? ""));
    }
  };

  const openFile = (item: DesignItem) => {
    api.openPath(item.path).catch(() => onNotice?.("error", "无法打开文件"));
  };

  const styles = mode === "icon" ? ICON_STYLES : IP_STYLES;

  return (
    <div className="h-full flex flex-col bg-white">
      {/* 顶部 */}
      <header className="px-6 pt-5 pb-3 border-b border-slate-100 flex items-center justify-between shrink-0">
        <div>
          <h1 className="text-xl font-semibold text-slate-800">设计工作室</h1>
          <p className="text-xs text-slate-400 mt-0.5">
            图标 · IP 形象 · 原型设计，AI 一键生成，灵感随时落地
          </p>
        </div>
        <button
          onClick={onOpenSettings}
          className="text-sm bg-slate-100 hover:bg-slate-200 text-slate-600 rounded-lg px-3 py-1.5 flex items-center gap-1.5"
        >
          <svg
            viewBox="0 0 24 24"
            className="w-4 h-4"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.7"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <path d="M12 15a3 3 0 100-6 3 3 0 000 6z|M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 11-2.83 2.83l-.06-.06a1.65 1.65 0 00-2.82 1.17V21a2 2 0 11-4 0v-.09A1.65 1.65 0 006 19.4l-.06.06a2 2 0 11-2.83-2.83l.06-.06A1.65 1.65 0 004.6 15H4.5a2 2 0 110-4h.09A1.65 1.65 0 006 9.4" />
          </svg>
          生图模型设置（可选）
        </button>
      </header>

      <div className="flex-1 min-h-0 flex">
        {/* 左侧控制区 */}
        <aside className="w-[340px] shrink-0 border-r border-slate-200 flex flex-col">
          <div className="p-4 space-y-4 overflow-y-auto flex-1 min-h-0">
            {/* 模式切换 */}
            <div className="grid grid-cols-3 gap-1.5">
              {MODES.map((m) => (
                <button
                  key={m.id}
                  onClick={() => {
                    setMode(m.id);
                    setPrompt("");
                    setStyle("");
                  }}
                  className={`rounded-lg px-2 py-2 text-center transition ${
                    mode === m.id
                      ? "bg-[#0f766e] text-white shadow-sm"
                      : "bg-slate-100 text-slate-600 hover:bg-slate-200"
                  }`}
                >
                  <div className="text-[13px] font-medium">{m.label}</div>
                  <div
                    className={`text-[10px] mt-0.5 ${
                      mode === m.id ? "text-teal-100" : "text-slate-400"
                    }`}
                  >
                    {m.desc}
                  </div>
                </button>
              ))}
            </div>

            {/* 需求输入 */}
            <div>
              <label className="text-xs font-medium text-slate-600 mb-1.5 block">
                设计需求
              </label>
              <textarea
                value={prompt}
                onChange={(e) => setPrompt(e.target.value)}
                placeholder={meta.placeholder}
                rows={5}
                className="w-full resize-none rounded-xl border border-slate-200 bg-slate-50 px-3 py-2.5 text-sm text-slate-800 placeholder:text-slate-400 focus:outline-none focus:ring-2 focus:ring-[#0f766e]/30 focus:border-[#0f766e]"
              />
              <div className="mt-2 flex flex-wrap gap-1.5">
                {meta.examples.map((ex) => (
                  <button
                    key={ex}
                    onClick={() => setPrompt(ex)}
                    className="text-[11px] bg-slate-100 hover:bg-slate-200 text-slate-500 rounded-full px-2.5 py-1 transition"
                  >
                    {ex.length > 18 ? ex.slice(0, 18) + "…" : ex}
                  </button>
                ))}
              </div>
            </div>

            {/* 风格 / 平台 */}
            {mode !== "prototype" ? (
              <div>
                <label className="text-xs font-medium text-slate-600 mb-1.5 block">
                  风格
                </label>
                <div className="flex flex-wrap gap-1.5">
                  {styles.map((s) => (
                    <button
                      key={s}
                      onClick={() => setStyle(style === s ? "" : s)}
                      className={`text-xs rounded-full px-3 py-1 transition ${
                        style === s
                          ? "bg-[#0f766e] text-white"
                          : "bg-slate-100 text-slate-600 hover:bg-slate-200"
                      }`}
                    >
                      {s}
                    </button>
                  ))}
                </div>
              </div>
            ) : (
              <div>
                <label className="text-xs font-medium text-slate-600 mb-1.5 block">
                  目标形态
                </label>
                <div className="flex flex-wrap gap-1.5">
                  {PLATFORMS.map((p) => (
                    <button
                      key={p}
                      onClick={() => setPlatform(p)}
                      className={`text-xs rounded-full px-3 py-1 transition ${
                        platform === p
                          ? "bg-[#0f766e] text-white"
                          : "bg-slate-100 text-slate-600 hover:bg-slate-200"
                      }`}
                    >
                      {p}
                    </button>
                  ))}
                </div>
              </div>
            )}

            {/* 尺寸 */}
            {mode !== "prototype" && (
              <div>
                <label className="text-xs font-medium text-slate-600 mb-1.5 block">
                  画布尺寸
                </label>
                <div className="flex gap-1.5">
                  {SIZES.map((s) => (
                    <button
                      key={s}
                      onClick={() => setSize(s)}
                      className={`text-xs rounded-lg px-3 py-1.5 transition ${
                        size === s
                          ? "bg-[#0f766e] text-white"
                          : "bg-slate-100 text-slate-600 hover:bg-slate-200"
                      }`}
                    >
                      {s}
                    </button>
                  ))}
                </div>
              </div>
            )}

            {/* 生成按钮 */}
            <button
              onClick={generate}
              disabled={!prompt.trim() || generating}
              className={`w-full rounded-xl py-3 text-sm font-medium transition flex items-center justify-center gap-2 ${
                !prompt.trim() || generating
                  ? "bg-slate-100 text-slate-400 cursor-not-allowed"
                  : "bg-[#0f766e] hover:bg-[#115e59] text-white shadow-sm"
              }`}
            >
              {generating ? (
                <>
                  <svg
                    className="w-4 h-4 animate-spin"
                    viewBox="0 0 24 24"
                    fill="none"
                  >
                    <circle
                      className="opacity-25"
                      cx="12"
                      cy="12"
                      r="10"
                      stroke="currentColor"
                      strokeWidth="3"
                    />
                    <path
                      className="opacity-90"
                      fill="currentColor"
                      d="M4 12a8 8 0 018-8v4a4 4 0 00-4 4H4z"
                    />
                  </svg>
                  正在生成…
                </>
              ) : (
                <>
                  <svg
                    viewBox="0 0 24 24"
                    className="w-4 h-4"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="1.8"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  >
                    <path d="M12 5v14|M5 12h14" />
                  </svg>
                  开始设计
                </>
              )}
            </button>

            {/* 模型配置状态 */}
            <div className="rounded-xl bg-slate-50 border border-slate-200 px-3 py-2.5">
              <div className="flex items-center justify-between">
                <span className="text-[11px] text-slate-500">生图模型（可选）</span>
                <span
                  className={`text-[11px] rounded px-1.5 py-0.5 ${
                    config?.has_key
                      ? "bg-emerald-100 text-emerald-700"
                      : "bg-slate-200 text-slate-600"
                  }`}
                >
                  {config?.has_key ? "已配置" : "自动模式"}
                </span>
              </div>
              <div className="text-[11px] text-slate-400 mt-1 font-mono truncate">
                {config?.has_key
                  ? `${config.model} · ${config.base_url}`
                  : "未配置时自动用对话模型生成矢量图"}
              </div>
              <button
                onClick={onOpenSettings}
                className="text-[11px] text-[#0f766e] hover:underline mt-1"
              >
                管理生图模型 →
              </button>
            </div>
          </div>
        </aside>

        {/* 右侧预览 + 历史 */}
        <section className="flex-1 min-w-0 flex flex-col">
          {/* 预览区 */}
          <div className="flex-1 min-h-0 flex items-center justify-center bg-[repeating-conic-gradient(#f1f5f9_0%_25%,#ffffff_0%_50%)] bg-[length:24px_24px] p-6 relative">
            {preview ? (
              <div className="w-full h-full min-h-0 flex flex-col items-center gap-3">
                {preview.isHtml ? (
                  <iframe
                    src={preview.url}
                    title="原型预览"
                    className="flex-1 min-h-0 w-full bg-white rounded-xl border border-slate-200 shadow-lg"
                    sandbox="allow-scripts allow-modals allow-forms"
                  />
                ) : (
                  <img
                    src={preview.url}
                    alt="设计预览"
                    className="max-w-full max-h-[62vh] object-contain rounded-xl shadow-lg bg-white"
                  />
                )}
                {selected && (
                  <div className="flex items-center gap-2 bg-white/90 backdrop-blur rounded-xl border border-slate-200 px-3 py-2 shadow">
                    <button
                      onClick={() => openFile(selected)}
                      className="text-xs text-slate-600 hover:text-slate-900 px-2.5 py-1.5 rounded-lg hover:bg-slate-100"
                    >
                      打开文件
                    </button>
                    <button
                      onClick={() => {
                        const a = document.createElement("a");
                        a.href = preview.url;
                        a.download = selected.path.split(/[\\/]/).pop() ?? "design";
                        document.body.appendChild(a);
                        a.click();
                        document.body.removeChild(a);
                      }}
                      className="text-xs text-slate-600 hover:text-slate-900 px-2.5 py-1.5 rounded-lg hover:bg-slate-100"
                    >
                      下载
                    </button>
                    <button
                      onClick={() => remove(selected)}
                      className="text-xs text-slate-400 hover:text-red-500 px-2.5 py-1.5 rounded-lg hover:bg-red-50"
                    >
                      删除
                    </button>
                  </div>
                )}
              </div>
            ) : (
              <div className="text-center text-slate-400">
                <div className="mx-auto w-16 h-16 rounded-2xl bg-white border border-slate-200 shadow-sm flex items-center justify-center text-slate-300 mb-3">
                  <svg
                    viewBox="0 0 24 24"
                    className="w-7 h-7"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="1.5"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  >
                    <path d="M12 19l7-7 3 3-7 7-3-3z|M18 13l-1.5-7.5L2 2l3.5 14.5L13 18l5-5z|M2 2l7.586 7.586|M3.5 16.5L13 7" />
                  </svg>
                </div>
                <p className="text-sm font-medium text-slate-500">输入需求，开始你的设计</p>
                <p className="text-xs mt-1 max-w-xs">
                  生成结果会保存在本机设计目录，可随时预览、打开或删除
                </p>
              </div>
            )}
            {generating && (
              <div className="absolute inset-0 bg-white/60 backdrop-blur-[2px] flex flex-col items-center justify-center gap-3">
                <div className="flex items-center gap-1.5">
                  {[0, 1, 2].map((i) => (
                    <span
                      key={i}
                      className="w-2.5 h-2.5 rounded-full bg-[#0f766e] flow-dot"
                      style={{ animationDelay: `${i * 0.18}s` }}
                    />
                  ))}
                </div>
                <p className="text-sm text-slate-500">
                  {mode === "prototype" ? "正在生成交互原型…" : "正在绘制设计稿…"}
                </p>
                <p className="text-xs text-slate-400">通常需要 10~60 秒，请稍候</p>
              </div>
            )}
          </div>

          {/* 历史 */}
          <div className="shrink-0 border-t border-slate-200 bg-white">
            <div className="px-4 pt-3 pb-1.5 flex items-center justify-between">
              <span className="text-xs font-medium text-slate-500">设计历史</span>
              <span className="text-[11px] text-slate-400">{items.length} 个作品</span>
            </div>
            <div className="px-3 pb-3 flex gap-2 overflow-x-auto hover-scroll">
              {items.length === 0 && (
                <div className="text-xs text-slate-400 px-2 py-3">
                  还没有设计作品，生成后会自动出现在这里
                </div>
              )}
              {items.map((it) => (
                <div
                  key={it.id}
                  onClick={() => selectItem(it)}
                  className={`shrink-0 w-48 rounded-xl border p-2.5 cursor-pointer transition hover:shadow-sm ${
                    selected?.id === it.id
                      ? "border-[#0f766e] bg-teal-50/60"
                      : "border-slate-200 bg-white hover:border-slate-300"
                  }`}
                >
                  <div className="flex items-center justify-between">
                    <span
                      className={`inline-flex items-center gap-1 text-[10px] rounded px-1.5 py-0.5 ${kindBadgeCls(
                        it.kind
                      )}`}
                    >
                      <KindGlyph kind={it.kind} />
                      {KIND_LABEL[it.kind]}
                    </span>
                    <span className="text-[10px] text-slate-400 flex items-center gap-1">
                      {it.mode === "vector" && (
                        <span className="text-slate-400 bg-slate-100 rounded px-1 py-px">矢量</span>
                      )}
                      {fmtTime(it.created_at)}
                    </span>
                  </div>
                  <div className="text-xs text-slate-600 mt-1.5 line-clamp-2 min-h-[32px]">
                    {it.prompt}
                  </div>
                  <div className="flex items-center justify-between mt-1.5">
                    <span className="text-[10px] text-slate-400">{fmtBytes(it.bytes)}</span>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        remove(it);
                      }}
                      className="text-[10px] text-slate-400 hover:text-red-500 px-1"
                    >
                      删除
                    </button>
                  </div>
                </div>
              ))}
            </div>
          </div>
        </section>
      </div>
    </div>
  );
}

function fmtTime(iso: string): string {
  try {
    const d = new Date(iso);
    return d.toLocaleString("zh-CN", {
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
    });
  } catch {
    return "";
  }
}

function fmtBytes(n: number): string {
  if (n >= 1 << 20) return (n / (1 << 20)).toFixed(1) + " MB";
  if (n >= 1 << 10) return Math.round(n / (1 << 10)) + " KB";
  return n + " B";
}
