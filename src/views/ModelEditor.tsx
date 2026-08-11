import { useState } from "react";
import {
  api,
  ModelProfile,
  PROVIDER_PRESETS,
  ProviderKind,
  CONTEXT_TIERS,
} from "../lib/api";

/// 将任意值对齐到最接近的挡位 (>= 该值的最小挡位，上限 1M)
function snapTier(v: number): number {
  const t = CONTEXT_TIERS.find((x) => x.value >= v);
  return t ? t.value : CONTEXT_TIERS[CONTEXT_TIERS.length - 1].value;
}

const EMPTY: ModelProfile = {
  id: "",
  name: "",
  kind: "openai_compat",
  base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1",
  model: "qwen-max",
  context_window: 200000,
  thinking: "off",
  supports_thinking: true,
};

export default function ModelEditor({
  existing,
  onClose,
  onSaved,
}: {
  existing?: ModelProfile;
  onClose: () => void;
  onSaved: () => void;
}) {
  const [p, setP] = useState<ModelProfile>(existing ? { ...existing } : { ...EMPTY });
  const [apiKey, setApiKey] = useState("");
  const [saving, setSaving] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const isEdit = !!existing;

  const applyPreset = (label: string) => {
    const preset = PROVIDER_PRESETS.find((x) => x.label === label);
    if (!preset) return;
    setP((prev) => ({
      ...prev,
      name: prev.name || preset.label,
      kind: preset.kind,
      base_url: preset.base_url,
      model: preset.model,
      context_window: snapTier(preset.context_window),
      supports_thinking: preset.supports_thinking,
    }));
  };

  const save = async () => {
    if (!p.name.trim() || !p.model.trim()) {
      setErr("请填写模型名称与模型标识");
      return;
    }
    setSaving(true);
    setErr(null);
    try {
      await api.saveModel(p, apiKey.trim() || undefined);
      onSaved();
    } catch (e: any) {
      setErr(e?.toString() ?? "保存失败");
      setSaving(false);
    }
  };

  const field = "w-full bg-white border border-slate-300 rounded-lg px-3 py-2 text-sm text-slate-800 focus:outline-none focus:border-[#34c759]";

  return (
    <div className="fixed inset-0 bg-black/30 flex items-center justify-center z-50 fade-in">
      <div className="bg-white border border-slate-200 rounded-2xl w-[520px] max-w-[92vw] shadow-2xl">
        <div className="px-5 py-4 border-b border-slate-100 flex items-center justify-between">
          <h2 className="font-semibold text-slate-800">{isEdit ? "编辑模型" : "添加模型"}</h2>
          <button onClick={onClose} className="text-slate-400 hover:text-slate-600">
            ✕
          </button>
        </div>

        <div className="p-5 space-y-3 max-h-[70vh] overflow-y-auto">
          {!isEdit && (
            <div>
              <label className="block text-xs text-slate-500 mb-1.5">快速预设</label>
              <div className="flex flex-wrap gap-2">
                {PROVIDER_PRESETS.map((preset) => (
                  <button
                    key={preset.label}
                    onClick={() => applyPreset(preset.label)}
                    className="text-xs bg-slate-100 hover:bg-slate-200 rounded-lg px-2.5 py-1.5 text-slate-600"
                  >
                    {preset.label}
                  </button>
                ))}
              </div>
            </div>
          )}

          <Field label="显示名称">
            <input
              value={p.name}
              onChange={(e) => setP({ ...p, name: e.target.value })}
              placeholder="例如：通义千问 Max"
              className={field}
            />
          </Field>

          <div className="grid grid-cols-2 gap-3">
            <Field label="协议类型">
              <select
                value={p.kind}
                onChange={(e) => setP({ ...p, kind: e.target.value as ProviderKind })}
                className={field}
              >
                <option value="openai_compat">OpenAI 兼容</option>
                <option value="anthropic">Anthropic</option>
              </select>
            </Field>
            <Field label="模型标识">
              <input
                value={p.model}
                onChange={(e) => setP({ ...p, model: e.target.value })}
                placeholder="qwen-max"
                className={field}
              />
            </Field>
          </div>

          <Field label="Base URL">
            <input
              value={p.base_url}
              onChange={(e) => setP({ ...p, base_url: e.target.value })}
              className={field}
            />
          </Field>

          <div className="grid grid-cols-2 gap-3">
            <Field label="上下文窗口">
              <div className="grid grid-cols-3 gap-1.5">
                {CONTEXT_TIERS.map((tier) => (
                  <button
                    key={tier.value}
                    type="button"
                    onClick={() => setP({ ...p, context_window: tier.value })}
                    className={`text-sm rounded-lg py-2 border ${
                      p.context_window === tier.value
                        ? "border-[#34c759] bg-[#e0e0e0] text-[#333333]"
                        : "border-slate-300 text-slate-600 hover:bg-slate-50"
                    }`}
                  >
                    {tier.label}
                  </button>
                ))}
              </div>
            </Field>
            <Field label="支持思考模式">
              <label className="flex items-center gap-2 h-[38px] px-1">
                <input
                  type="checkbox"
                  checked={p.supports_thinking}
                  onChange={(e) => setP({ ...p, supports_thinking: e.target.checked })}
                  className="accent-[#34c759] w-4 h-4"
                />
                <span className="text-sm text-slate-600">可开启推理</span>
              </label>
            </Field>
          </div>

          <Field label={`API Key ${isEdit ? "(留空则不修改)" : ""}`}>
            <input
              type="password"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              placeholder="sk-..."
              className={field}
            />
          </Field>

          {err && <div className="text-sm text-red-600 bg-red-50 rounded-lg px-3 py-2">{err}</div>}
        </div>

        <div className="px-5 py-4 border-t border-slate-100 flex justify-end gap-2">
          <button
            onClick={onClose}
            className="px-4 py-2 rounded-lg text-sm bg-slate-100 hover:bg-slate-200 text-slate-600"
          >
            取消
          </button>
          <button
            onClick={save}
            disabled={saving}
            className="px-5 py-2 rounded-lg text-sm bg-[#333333] hover:bg-[#111111] text-white disabled:opacity-50"
          >
            {saving ? "保存中…" : "保存"}
          </button>
        </div>
      </div>
    </div>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div>
      <label className="block text-xs text-slate-500 mb-1.5">{label}</label>
      {children}
    </div>
  );
}
