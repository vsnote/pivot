import React, { useState } from 'react';
import * as DialogPrimitive from '@radix-ui/react-dialog';
import {
  Check,
  Copy,
  X,
  AlertTriangle,
  ArrowRightLeft,
  Terminal,
  Info,
} from 'lucide-react';
import type { InterceptorStatus } from '../../types';
import { Button } from '@/components/common/Button';
import { useTranslation } from '@/i18n';

interface AgentConflictDialogProps {
  open: boolean;
  target: InterceptorStatus | null;
  busy?: boolean;
  hasEnabledProviders?: boolean;
  onClose: () => void;
  onConfirmTakeover: (target: InterceptorStatus) => Promise<void>;
}

export const AgentConflictDialog: React.FC<AgentConflictDialogProps> = ({
  open,
  target,
  busy = false,
  hasEnabledProviders = true,
  onClose,
  onConfirmTakeover,
}) => {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);

  if (!target) return null;

  const endpoint = target.thirdParty?.endpoint || target.originalEndpoint || 'http://127.0.0.1:8317';
  const isOverwritten = target.isOverwritten ?? false;

  // 本地网关目标地址
  let gatewayEndpoint = 'http://127.0.0.1:16888';
  let exportCommand = `export ANTHROPIC_BASE_URL=http://127.0.0.1:16888`;
  if (target.id === 'codex_cli') {
    gatewayEndpoint = 'http://127.0.0.1:16888/v1';
    exportCommand = `export OPENAI_BASE_URL=http://127.0.0.1:16888/v1`;
  } else if (target.id === 'gemini_cli') {
    gatewayEndpoint = 'http://127.0.0.1:16888';
    exportCommand = `export GOOGLE_GEMINI_BASE_URL=http://127.0.0.1:16888`;
  } else if (target.id === 'pi') {
    gatewayEndpoint = 'http://127.0.0.1:16888/v1';
    exportCommand = `export OPENAI_BASE_URL=http://127.0.0.1:16888/v1`;
  }

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(exportCommand);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // 忽略复制失败
    }
  };

  return (
    <DialogPrimitive.Root open={open} onOpenChange={(isOpen) => !isOpen && onClose()}>
      <DialogPrimitive.Portal>
        <DialogPrimitive.Overlay className="fixed inset-0 z-50 bg-black/50 backdrop-blur-sm animate-in fade-in-0 duration-150" />
        <DialogPrimitive.Content
          aria-describedby="conflict-dialog-desc"
          className="fixed left-1/2 top-1/2 z-50 w-[calc(100%-32px)] max-w-lg -translate-x-1/2 -translate-y-1/2 rounded-lg border border-zinc-200 bg-white shadow-2xl animate-in fade-in-0 zoom-in-95 duration-150 dark:border-zinc-800 dark:bg-zinc-900 overflow-hidden focus:outline-none"
        >
          {/* 顶部标题栏 */}
          <div className="flex items-start justify-between px-5 py-4 border-b border-zinc-100 dark:border-zinc-800 bg-zinc-50/50 dark:bg-zinc-900/50">
            <div className="flex items-center gap-3">
              <div
                className={`w-9 h-9 rounded-lg flex items-center justify-center shrink-0 ${
                  isOverwritten
                    ? 'bg-amber-500/10 text-amber-600 dark:text-amber-400 border border-amber-500/20'
                    : 'bg-zinc-100 text-zinc-800 dark:bg-zinc-800 dark:text-zinc-200 border border-zinc-200 dark:border-zinc-700'
                }`}
              >
                {isOverwritten ? <AlertTriangle className="w-4 h-4" /> : <ArrowRightLeft className="w-4 h-4" />}
              </div>
              <div>
                <DialogPrimitive.Title className="text-sm font-semibold text-zinc-900 dark:text-zinc-100">
                  {isOverwritten
                    ? t('agentConflict.conflictTitle', { name: target.name })
                    : t('agentConflict.takeoverTitle', { name: target.name })}
                </DialogPrimitive.Title>
                <p id="conflict-dialog-desc" className="text-xs text-zinc-500 dark:text-zinc-400 mt-0.5">
                  {isOverwritten
                    ? t('agentConflict.conflictDesc')
                    : t('agentConflict.takeoverDesc')}
                </p>
              </div>
            </div>
            <DialogPrimitive.Close asChild>
              <button
                type="button"
                disabled={busy}
                className="p-1.5 -mr-1 -mt-1 rounded-md text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-200 hover:bg-zinc-100 dark:hover:bg-zinc-800 transition-colors cursor-pointer"
              >
                <X className="w-4 h-4" />
              </button>
            </DialogPrimitive.Close>
          </div>

          {/* 内容主体 */}
          <div className="p-5 space-y-3.5 text-xs text-zinc-600 dark:text-zinc-400 max-h-[75vh] overflow-y-auto">
            {/* 端点对比 */}
            <div className="rounded-lg border border-zinc-200/80 dark:border-zinc-800 bg-zinc-50/50 dark:bg-zinc-950/40 p-3 text-xs space-y-2">
              <div className="flex items-center justify-between gap-3">
                <span className="text-zinc-500 dark:text-zinc-400 text-[11px] shrink-0 font-sans">
                  {t('agentConflict.currentEndpoint')}
                </span>
                <span className="font-mono text-zinc-800 dark:text-zinc-200 truncate text-[11px]" title={endpoint}>
                  {endpoint}
                </span>
              </div>
              <div className="flex items-center justify-between gap-3 border-t border-zinc-200/60 dark:border-zinc-800/60 pt-2">
                <span className="text-zinc-500 dark:text-zinc-400 text-[11px] shrink-0 font-sans">
                  {t('agentConflict.gatewayEndpoint')}
                </span>
                <span className="font-mono font-medium text-zinc-900 dark:text-zinc-100 font-semibold text-[11px]">
                  {gatewayEndpoint}
                </span>
              </div>
            </div>

            <div className="p-3 rounded-lg bg-zinc-50/80 dark:bg-zinc-800/50 border border-zinc-200/60 dark:border-zinc-800 text-[11px] text-zinc-600 dark:text-zinc-400 leading-relaxed">
              {t('agentConflict.descriptionNotice', { name: target.name })}
            </div>

            {/* 前置检查警示：若当前未配置任何服务商 */}
            {!hasEnabledProviders && (
              <div className="flex items-center gap-2 p-2.5 rounded-md bg-amber-50/80 dark:bg-amber-950/20 border border-amber-200/70 dark:border-amber-900/40 text-amber-800 dark:text-amber-300">
                <AlertTriangle className="w-3.5 h-3.5 shrink-0 text-amber-500" />
                <span>{t('agentConflict.noProvidersWarning')}</span>
              </div>
            )}

            {/* 辅助提醒 */}
            <div className="flex items-start gap-1.5 text-[11px] text-zinc-400 dark:text-zinc-500 leading-relaxed px-0.5">
              <Info className="w-3.5 h-3.5 shrink-0 text-zinc-400 mt-0.5" />
              <span>
                {t('agentConflict.conflictHint')}
              </span>
            </div>

            {/* 终端单次免改动命令 */}
            <div className="flex items-center justify-between px-2.5 py-1.5 rounded-md bg-zinc-100/70 dark:bg-zinc-800/40 border border-zinc-200/60 dark:border-zinc-800/80 font-mono text-[11px] text-zinc-500 dark:text-zinc-400">
              <div className="flex items-center gap-1.5 min-w-0 mr-2">
                <Terminal className="w-3 h-3 text-zinc-400 shrink-0" />
                <span className="truncate select-all">{exportCommand}</span>
              </div>
              <button
                type="button"
                onClick={handleCopy}
                className="flex items-center gap-1 text-zinc-600 dark:text-zinc-300 hover:text-zinc-900 dark:hover:text-zinc-100 shrink-0 cursor-pointer font-sans text-xs transition-colors"
              >
                {copied ? <Check className="w-3 h-3 text-emerald-500" /> : <Copy className="w-3 h-3" />}
                <span>{copied ? t('agentConflict.commandCopied') : t('agentConflict.copyCommand')}</span>
              </button>
            </div>
          </div>

          {/* 底部按钮栏 */}
          <div className="flex items-center justify-end gap-2 px-5 py-3 border-t border-zinc-100 dark:border-zinc-800 bg-zinc-50/50 dark:bg-zinc-900/50">
            <Button variant="outline" size="default" disabled={busy} onClick={onClose}>
              {t('common.cancel')}
            </Button>
            <Button
              variant="default"
              size="default"
              disabled={busy}
              onClick={() => onConfirmTakeover(target)}
            >
              {busy ? t('agentConflict.takingOver') : t('agentConflict.confirmTakeover')}
            </Button>
          </div>
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  );
};
