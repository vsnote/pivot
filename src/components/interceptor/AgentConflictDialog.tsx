import React, { useState } from 'react';
import { Layers, Terminal, Check, Copy, ArrowRight, ShieldCheck, X } from 'lucide-react';
import type { InterceptorStatus } from '../../types';
import { useDialogAccessibility } from '../common/useDialogAccessibility';

interface AgentConflictDialogProps {
  open: boolean;
  target: InterceptorStatus | null;
  busy?: boolean;
  onClose: () => void;
  onImportAndToggle: (target: InterceptorStatus) => Promise<void>;
  onJustToggle: (target: InterceptorStatus) => Promise<void>;
}

export const AgentConflictDialog: React.FC<AgentConflictDialogProps> = ({
  open,
  target,
  busy = false,
  onClose,
  onImportAndToggle,
  onJustToggle,
}) => {
  const dialogRef = useDialogAccessibility(open, onClose);
  const [copied, setCopied] = useState(false);

  if (!open || !target) return null;

  const endpoint = target.thirdParty?.endpoint || target.originalEndpoint || 'http://127.0.0.1:8317';
  const suggestedName = target.thirdParty?.suggestedName || (endpoint.includes('8317') ? 'CC-Switch 本地通道 (8317)' : '本地第三方代理');

  // 生成临时的终端导出命令
  let exportCommand = `export ANTHROPIC_BASE_URL=http://127.0.0.1:16888`;
  if (target.id === 'codex_cli') {
    exportCommand = `export OPENAI_BASE_URL=http://127.0.0.1:16888/v1`;
  } else if (target.id === 'gemini_cli') {
    exportCommand = `export GOOGLE_GEMINI_BASE_URL=http://127.0.0.1:16888`;
  }

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(exportCommand);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // 忽略复制失败
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-xs p-4 pt-10">
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="conflict-dialog-title"
        className="bg-white dark:bg-zinc-900 rounded-2xl border border-zinc-200 dark:border-zinc-800 shadow-2xl w-full max-w-lg overflow-hidden animate-in fade-in zoom-in-95 duration-150"
      >
        {/* 顶部标题栏 */}
        <div className="flex items-center justify-between px-5 py-3.5 border-b border-zinc-200/80 dark:border-zinc-800/80 bg-zinc-50/50 dark:bg-zinc-900/50">
          <div className="flex items-center gap-2.5">
            <div className="w-7 h-7 rounded-lg bg-indigo-500/10 dark:bg-indigo-500/20 text-indigo-600 dark:text-indigo-400 flex items-center justify-center">
              <Layers className="w-4 h-4" />
            </div>
            <div>
              <h2 id="conflict-dialog-title" className="text-sm font-semibold text-zinc-900 dark:text-zinc-100">
                检测到第三方端点：开启 {target.name} 代理
              </h2>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            disabled={busy}
            className="p-1 rounded-lg text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-200 hover:bg-zinc-100 dark:hover:bg-zinc-800 transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* 内容主体 */}
        <div className="p-5 space-y-4">
          {/* 检测到的端点详情信息 */}
          <div className="rounded-xl p-3.5 bg-amber-50/80 dark:bg-amber-950/20 border border-amber-200/70 dark:border-amber-900/40 space-y-2">
            <div className="flex items-center justify-between text-xs">
              <span className="text-amber-800 dark:text-amber-300 font-medium">
                检测到当前配置了第三方端点
              </span>
              <span className="px-2 py-0.5 rounded-md font-mono text-[11px] font-medium bg-amber-200/60 dark:bg-amber-900/60 text-amber-900 dark:text-amber-200">
                {endpoint}
              </span>
            </div>
            <p className="text-xs text-amber-700/90 dark:text-amber-400/90 leading-relaxed">
              您的 {target.name} 目前正在使用该外部端点。您可以选择直接将此端点作为上游渠道接入，双应用协同工作无冲突。
            </p>
          </div>

          {/* 选项一：级联代理导入（强烈推荐） */}
          <button
            type="button"
            disabled={busy}
            onClick={() => onImportAndToggle(target)}
            className="w-full text-left p-3.5 rounded-xl border-2 border-indigo-500/40 hover:border-indigo-500 bg-indigo-500/5 hover:bg-indigo-500/10 dark:border-indigo-500/30 dark:hover:border-indigo-500 dark:bg-indigo-500/10 transition-all cursor-pointer group"
          >
            <div className="flex items-start justify-between gap-2">
              <div className="space-y-1">
                <div className="flex items-center gap-2">
                  <span className="text-xs font-semibold text-indigo-700 dark:text-indigo-300">
                    接入为上游服务商并代理
                  </span>
                  <span className="text-[10px] px-1.5 py-0.5 font-medium rounded-full bg-indigo-500/20 text-indigo-600 dark:text-indigo-300">
                    推荐 · 协同共存
                  </span>
                </div>
                <p className="text-xs text-zinc-600 dark:text-zinc-400 leading-relaxed">
                  将该端点（<span className="font-mono font-medium text-zinc-800 dark:text-zinc-200">{suggestedName}</span>）接入为 OmniSwitch 的服务商之一。原工具的所有账号与模型继续生效，享受统一路由与监控。
                </p>
              </div>
              <ArrowRight className="w-4 h-4 text-indigo-500 group-hover:translate-x-0.5 transition-transform mt-0.5 shrink-0" />
            </div>
          </button>

          {/* 选项二：仅接管并备份 */}
          <button
            type="button"
            disabled={busy}
            onClick={() => onJustToggle(target)}
            className="w-full text-left p-3 rounded-xl border border-zinc-200 dark:border-zinc-800 hover:border-zinc-300 dark:hover:border-zinc-700 bg-zinc-50/50 dark:bg-zinc-800/40 hover:bg-zinc-100/50 dark:hover:bg-zinc-800/80 transition-all cursor-pointer group"
          >
            <div className="flex items-center justify-between">
              <div className="space-y-0.5">
                <div className="text-xs font-medium text-zinc-800 dark:text-zinc-200 flex items-center gap-1.5">
                  <ShieldCheck className="w-3.5 h-3.5 text-zinc-500" />
                  仅接管代理（自动备份原配置）
                </div>
                <p className="text-[11px] text-zinc-500 dark:text-zinc-400">
                  安全备份原有端点配置，直接由 OmniSwitch 接管。关闭代理时完整恢复原配置。
                </p>
              </div>
              <ArrowRight className="w-3.5 h-3.5 text-zinc-400 group-hover:translate-x-0.5 transition-transform shrink-0" />
            </div>
          </button>

          {/* 选项三：免改动配置文件的环境变量提示 */}
          <div className="pt-2 border-t border-zinc-100 dark:border-zinc-800/80">
            <div className="flex items-center justify-between text-[11px] text-zinc-500 dark:text-zinc-400 mb-1.5">
              <span className="flex items-center gap-1">
                <Terminal className="w-3 h-3" />
                或者：单次终端免改动代理（零冲突）
              </span>
              <button
                type="button"
                onClick={handleCopy}
                className="flex items-center gap-1 text-indigo-600 dark:text-indigo-400 hover:underline cursor-pointer"
              >
                {copied ? <Check className="w-3 h-3 text-emerald-500" /> : <Copy className="w-3 h-3" />}
                {copied ? '已复制' : '复制命令'}
              </button>
            </div>
            <div className="flex items-center justify-between px-2.5 py-1.5 rounded-lg bg-zinc-100/80 dark:bg-zinc-800/60 font-mono text-[11px] text-zinc-600 dark:text-zinc-300 select-all overflow-x-auto">
              <code>{exportCommand}</code>
            </div>
          </div>
        </div>

        {/* 底部按钮栏 */}
        <div className="flex items-center justify-end px-5 py-3 border-t border-zinc-200 dark:border-zinc-800 bg-zinc-50/50 dark:bg-zinc-900/50">
          <button
            type="button"
            className="px-3.5 py-1.5 text-xs font-medium rounded-lg text-zinc-600 dark:text-zinc-300 hover:bg-zinc-200/60 dark:hover:bg-zinc-800 transition-colors cursor-pointer"
            disabled={busy}
            onClick={onClose}
          >
            取消
          </button>
        </div>
      </div>
    </div>
  );
};
