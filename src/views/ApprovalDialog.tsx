import { ApprovalReq } from "../App";

export default function ApprovalDialog({
  req,
  onResolve,
}: {
  req: ApprovalReq;
  onResolve: (id: string, decision: string) => void;
}) {
  return (
    <div className="fixed inset-0 bg-black/30 flex items-center justify-center z-50 fade-in">
      <div className="bg-white border border-slate-200 rounded-2xl w-[520px] max-w-[90vw] shadow-2xl">
        <div className="p-5 border-b border-slate-100">
          <div className="flex items-center gap-2">
            <span className="text-2xl">🔐</span>
            <div>
              <h2 className="font-semibold text-slate-800">权限确认</h2>
              <p className="text-xs text-slate-500">
                {req.agent} 请求执行 <span className="font-mono">{req.tool}</span>
              </p>
            </div>
          </div>
        </div>

        <div className="p-5 space-y-3">
          <div className="text-sm text-amber-700 bg-amber-50 border border-amber-200 rounded-lg px-3 py-2">
            {req.summary}
          </div>
          <div>
            <div className="text-xs text-slate-500 mb-1">调用参数</div>
            <pre className="bg-slate-50 border border-slate-200 rounded-lg p-3 text-xs text-slate-700 overflow-x-auto max-h-48">
              {JSON.stringify(req.input, null, 2)}
            </pre>
          </div>
        </div>

        <div className="p-5 border-t border-slate-100 flex gap-2 justify-end">
          <button
            onClick={() => onResolve(req.id, "deny")}
            className="px-4 py-2 rounded-lg text-sm bg-slate-100 hover:bg-slate-200 text-slate-600"
          >
            拒绝
          </button>
          <button
            onClick={() => onResolve(req.id, "allow_once")}
            className="px-4 py-2 rounded-lg text-sm bg-slate-100 hover:bg-slate-200 text-slate-700"
          >
            允许一次
          </button>
          <button
            onClick={() => onResolve(req.id, "allow_always")}
            className="px-4 py-2 rounded-lg text-sm bg-[#8b5cf6] hover:bg-[#7c3aed] text-white"
          >
            始终允许
          </button>
        </div>
      </div>
    </div>
  );
}
