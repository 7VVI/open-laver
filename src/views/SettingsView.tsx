import { useEffect, useState } from "react";
import { api } from "../lib/api";
import { open } from "@tauri-apps/plugin-dialog";

export default function SettingsView() {
  const [workspace, setWorkspace] = useState("");

  useEffect(() => {
    api.getWorkspace().then((w) => setWorkspace(w.workspace));
  }, []);

  const pickWorkspace = async () => {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir === "string") {
      await api.setWorkspace(dir);
      setWorkspace(dir);
    }
  };

  return (
    <div className="h-full overflow-y-auto">
      <div className="max-w-2xl mx-auto px-8 py-8">
        <h1 className="text-xl font-semibold text-slate-800 mb-6">设置</h1>

        <Section title="工作目录">
          <p className="text-xs text-slate-500 mb-2">
            智能体的文件操作默认限定在此目录内，目录之外的写操作会触发权限确认。
          </p>
          <div className="flex gap-2">
            <input
              readOnly
              value={workspace}
              className="flex-1 bg-slate-50 border border-slate-300 rounded-lg px-3 py-2 text-sm text-slate-700"
            />
            <button
              onClick={pickWorkspace}
              className="bg-[#333333] hover:bg-[#111111] text-white rounded-lg px-4 text-sm"
            >
              选择
            </button>
          </div>
        </Section>

        <Section title="模型与密钥">
          <p className="text-sm text-slate-600">
            模型配置、切换与密钥管理已移至左下角「模型管理」，也可在对话框顶部快速切换 / 添加模型。
          </p>
        </Section>

        <Section title="关于">
          <p className="text-sm text-slate-600">Laver 办公 · 桌面智能体</p>
          <p className="text-xs text-slate-400 mt-1">版本 0.1.0</p>
        </Section>
      </div>
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="mb-5 bg-white border border-slate-200 rounded-xl p-5">
      <h2 className="text-sm font-semibold text-slate-700 mb-3">{title}</h2>
      {children}
    </div>
  );
}
