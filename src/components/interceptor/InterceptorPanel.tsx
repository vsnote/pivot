import React, { useState } from 'react';
import { useAppStore } from '@/store/useAppStore';
import { InterceptorStatus } from '@/types';
import { Switch } from '@/components/common/Switch';
import { AgentConflictDialog } from '@/components/interceptor/AgentConflictDialog';
import { PiLogo } from '@/components/common/BrandIcons';
import {
  Terminal,
  ShieldCheck,
  CheckCircle2,
  AlertCircle,
  Copy,
  Check,
  Sparkles,
  AlertTriangle,
} from 'lucide-react';

const DEFAULT_INTERCEPTORS: InterceptorStatus[] = [
  {
    id: 'claude_code',
    name: 'Claude Code',
    installed: true,
    configPath: '~/.claude/settings.json',
    isIntercepted: false,
  },
  {
    id: 'codex_cli',
    name: 'Codex CLI',
    installed: true,
    configPath: '~/.codex/config.toml',
    isIntercepted: false,
  },
  {
    id: 'gemini_cli',
    name: 'Gemini CLI',
    installed: false,
    configPath: '~/.gemini/.env',
    isIntercepted: false,
  },
  {
    id: 'pi',
    name: 'Pi Agent',
    installed: false,
    configPath: '~/.pi/agent/models.json',
    isIntercepted: false,
  },
];

export const InterceptorPanel: React.FC = () => {
  const { interceptors, toggleInterceptor, providers, stats } = useAppStore();
  const [togglingId, setTogglingId] = useState<string | null>(null);
  const [copiedKey, setCopiedKey] = useState<string | null>(null);
  const [conflictConfirmTarget, setConflictConfirmTarget] = useState<InterceptorStatus | null>(null);

  const displayList = interceptors.length > 0 ? interceptors : DEFAULT_INTERCEPTORS;
  const hasEnabledProviders = providers.some((p) => p.enabled);

  const handleToggle = async (id: string, enable: boolean) => {
    const target = displayList.find((item) => item.id === id);
    if ((enable || target?.isOverwritten) && target && target.conflict) {
      setConflictConfirmTarget(target);
      return;
    }

    setTogglingId(id);
    try {
      await toggleInterceptor(id, enable);
    } finally {
      setTogglingId(null);
    }
  };

  const handleConfirmTakeover = async (target: InterceptorStatus) => {
    setConflictConfirmTarget(null);
    setTogglingId(target.id);
    try {
      await toggleInterceptor(target.id, true);
    } finally {
      setTogglingId(null);
    }
  };

  const copyToClipboard = (key: string, text: string) => {
    navigator.clipboard.writeText(text);
    setCopiedKey(key);
    setTimeout(() => setCopiedKey(null), 2000);
  };

  const claudeExport = `export ANTHROPIC_BASE_URL=http://127.0.0.1:${stats.port}`;
  const openaiExport = `export OPENAI_BASE_URL=http://127.0.0.1:${stats.port}/v1`;

  return (
    <div className="space-y-4 animate-in fade-in duration-150">
      {/* 统一网关接入地址 */}
      <div className="p-4 sm:p-5 rounded-lg border border-zinc-200/80 dark:border-zinc-800 bg-white dark:bg-zinc-900 shadow-xs space-y-3">
        <div>
          <h2 className="text-base font-bold text-zinc-950 dark:text-zinc-100">
            网关接入地址
          </h2>
          <p className="text-xs text-zinc-400 mt-1">
            适用于 Cursor、Windsurf、Aider 等第三方 Agent、插件或脚本直接接入
          </p>
        </div>

        <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 pt-1">
          <div className="p-3.5 rounded-md bg-zinc-50 dark:bg-zinc-800/60 border border-zinc-200/60 dark:border-zinc-800 flex items-center justify-between">
            <div className="min-w-0 pr-2">
              <div className="text-[11px] text-zinc-400 mb-0.5">OpenAI 兼容 Base URL</div>
              <div className="font-mono text-xs font-semibold text-zinc-900 dark:text-zinc-100 truncate">
                http://127.0.0.1:{stats.port}/v1
              </div>
            </div>
            <button
              type="button"
              onClick={() => copyToClipboard('openai-url', `http://127.0.0.1:${stats.port}/v1`)}
              className="shrink-0 rounded-md p-1.5 text-zinc-400 transition-colors hover:bg-zinc-200 hover:text-zinc-700 dark:hover:bg-zinc-700 dark:hover:text-zinc-200 cursor-pointer"
              title="复制 OpenAI 兼容地址"
            >
              {copiedKey === 'openai-url' ? (
                <Check className="w-3.5 h-3.5 text-emerald-500" />
              ) : (
                <Copy className="w-3.5 h-3.5" />
              )}
            </button>
          </div>

          <div className="p-3.5 rounded-md bg-zinc-50 dark:bg-zinc-800/60 border border-zinc-200/60 dark:border-zinc-800 flex items-center justify-between">
            <div className="min-w-0 pr-2">
              <div className="text-[11px] text-zinc-400 mb-0.5">Anthropic 兼容 Base URL</div>
              <div className="font-mono text-xs font-semibold text-zinc-900 dark:text-zinc-100 truncate">
                http://127.0.0.1:{stats.port}
              </div>
            </div>
            <button
              type="button"
              onClick={() => copyToClipboard('anthropic-url', `http://127.0.0.1:${stats.port}`)}
              className="shrink-0 rounded-md p-1.5 text-zinc-400 transition-colors hover:bg-zinc-200 hover:text-zinc-700 dark:hover:bg-zinc-700 dark:hover:text-zinc-200 cursor-pointer"
              title="复制 Anthropic 兼容地址"
            >
              {copiedKey === 'anthropic-url' ? (
                <Check className="w-3.5 h-3.5 text-emerald-500" />
              ) : (
                <Copy className="w-3.5 h-3.5" />
              )}
            </button>
          </div>
        </div>
      </div>

      {/* 本地 CLI 客户端一键代理 */}
      <div className="pb-0.5 pt-2">
        <h3 className="text-xs font-bold text-zinc-900 dark:text-zinc-100 uppercase tracking-wider">
          本地命令行客户端一键代理
        </h3>
      </div>

      {/* 精简卡片网格 */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-3.5">
        {displayList.map((tool) => {
          const isToggling = togglingId === tool.id;
          const isOverwritten = tool.isOverwritten ?? false;

          return (
            <div
              key={tool.id}
              className={`rounded-lg p-4 sm:p-5 flex flex-col justify-between transition-all duration-150 bg-white dark:bg-zinc-900 border shadow-xs ${
                isOverwritten
                  ? 'border-amber-500/60 dark:border-amber-500/60 ring-1 ring-amber-500/20'
                  : tool.isIntercepted
                    ? 'border-zinc-900/30 dark:border-zinc-100/30 ring-1 ring-zinc-900/10 dark:ring-zinc-100/10'
                    : 'border-zinc-200/80 dark:border-zinc-800 hover:border-zinc-300 dark:hover:border-zinc-700'
              }`}
            >
              <div>
                {/* 头部：图标 + 名称 + 开关 */}
                <div className="flex items-center justify-between gap-2 mb-3">
                  <div className="flex items-center gap-3">
                    <div
                      className={`w-9 h-9 rounded-md flex items-center justify-center shrink-0 ${
                        isOverwritten
                          ? 'bg-amber-600 dark:bg-amber-600 text-white shadow-xs shadow-amber-500/30'
                          : tool.isIntercepted
                            ? 'bg-zinc-900 dark:bg-zinc-100 text-white dark:text-zinc-900 shadow-2xs'
                            : 'bg-zinc-100 dark:bg-zinc-800 text-zinc-600 dark:text-zinc-300'
                      }`}
                    >
                      {tool.id === 'claude_code' ? (
                        <Terminal className="w-4 h-4" />
                      ) : tool.id === 'codex_cli' ? (
                        <ShieldCheck className="w-4 h-4" />
                      ) : tool.id === 'gemini_cli' ? (
                        <Sparkles className="w-4 h-4" />
                      ) : (
                        <PiLogo className="w-4 h-4" />
                      )}
                    </div>
                    <div>
                      <h3 className="text-sm font-bold text-zinc-950 dark:text-zinc-100">
                        {tool.name}
                      </h3>
                      <div className="text-[11px] text-zinc-400 mt-0.5 flex items-center gap-1">
                        {tool.installed ? (
                          <span className="text-emerald-600 dark:text-emerald-400 flex items-center gap-0.5 font-medium">
                            <CheckCircle2 className="w-3 h-3" />
                            已安装
                          </span>
                        ) : (
                          <span className="flex items-center gap-0.5">
                            <AlertCircle className="w-3 h-3" />
                            未检测到
                          </span>
                        )}
                      </div>
                    </div>
                  </div>

                  <Switch
                    checked={tool.isIntercepted}
                    disabled={isToggling}
                    onCheckedChange={(checked) => handleToggle(tool.id, checked)}
                    ariaLabel={`切换 ${tool.name} 代理`}
                  />
                </div>

                {/* 状态行 */}
                <div className="py-2 px-3 rounded-md bg-zinc-50/80 dark:bg-zinc-800/50 border border-zinc-200/50 dark:border-zinc-800/60 flex items-center justify-between text-xs font-mono mb-2.5">
                  <span className="text-zinc-400">状态</span>
                  <span className="font-semibold flex items-center gap-1.5">
                    <span
                      className={`w-1.5 h-1.5 rounded-full ${
                        isOverwritten
                          ? 'bg-amber-500 animate-pulse'
                          : tool.isIntercepted
                            ? 'bg-emerald-500 animate-pulse'
                            : 'bg-zinc-400'
                      }`}
                    />
                    <span
                      className={
                        isOverwritten
                          ? 'text-amber-600 dark:text-amber-400 font-medium'
                          : tool.isIntercepted
                            ? 'text-emerald-600 dark:text-emerald-400 font-medium'
                            : 'text-zinc-500 dark:text-zinc-400'
                      }
                    >
                      {isOverwritten
                        ? '外部改写覆盖 (争夺暂停)'
                        : tool.isIntercepted
                          ? `已接管 (:16888)`
                          : '官方直连'}
                    </span>
                  </span>
                </div>

                {/* 配置文件微路径 */}
                <div className="text-[11px] font-mono text-zinc-400 truncate" title={tool.configPath}>
                  {tool.configPath}
                </div>

                {/* 外部篡改争夺小提示条 */}
                {isOverwritten && (
                  <div className="mt-2.5 p-2 rounded-md bg-amber-50/90 dark:bg-amber-950/30 border border-amber-200/70 dark:border-amber-900/40 text-[11px] text-amber-900 dark:text-amber-300 flex items-center justify-between gap-2">
                    <span className="truncate flex items-center gap-1 text-[11px]">
                      <AlertTriangle className="w-3 h-3 text-amber-500 shrink-0" />
                      第三方工具写入冲突
                    </span>
                    <button
                      type="button"
                      onClick={() => setConflictConfirmTarget(tool)}
                      className="text-amber-700 dark:text-amber-300 underline font-medium hover:text-amber-950 dark:hover:text-amber-100 shrink-0 cursor-pointer text-[10px]"
                    >
                      解决冲突
                    </button>
                  </div>
                )}
              </div>
            </div>
          );
        })}
      </div>

      {/* 终端临时命令 */}
      <div className="p-4 sm:p-5 rounded-lg border border-zinc-200/80 dark:border-zinc-800 bg-white dark:bg-zinc-900 shadow-xs space-y-2.5">
        <h3 className="text-xs font-bold text-zinc-900 dark:text-zinc-100 uppercase tracking-wider">
          临时环境变量
        </h3>

        <div className="grid grid-cols-1 sm:grid-cols-2 gap-2 pt-0.5">
          {/* Claude Code */}
          <div className="flex items-center justify-between bg-zinc-950 text-zinc-100 p-2 px-3 rounded-md font-mono text-xs border border-zinc-800">
            <span className="truncate">{claudeExport}</span>
            <button
              onClick={() => copyToClipboard('claude', claudeExport)}
              className="ml-2 shrink-0 rounded-md p-1.5 text-zinc-400 transition-colors hover:bg-zinc-800 hover:text-white cursor-pointer"
              title="复制"
            >
              {copiedKey === 'claude' ? (
                <Check className="w-3.5 h-3.5 text-emerald-400" />
              ) : (
                <Copy className="w-3.5 h-3.5" />
              )}
            </button>
          </div>

          {/* OpenAI */}
          <div className="flex items-center justify-between bg-zinc-950 text-zinc-100 p-2 px-3 rounded-md font-mono text-xs border border-zinc-800">
            <span className="truncate">{openaiExport}</span>
            <button
              onClick={() => copyToClipboard('openai', openaiExport)}
              className="ml-2 shrink-0 rounded-md p-1.5 text-zinc-400 transition-colors hover:bg-zinc-800 hover:text-white cursor-pointer"
              title="复制"
            >
              {copiedKey === 'openai' ? (
                <Check className="w-3.5 h-3.5 text-emerald-400" />
              ) : (
                <Copy className="w-3.5 h-3.5" />
              )}
            </button>
          </div>
        </div>
      </div>

      {/* 冲突处理弹窗 */}
      <AgentConflictDialog
        open={conflictConfirmTarget !== null}
        target={conflictConfirmTarget}
        busy={togglingId !== null}
        hasEnabledProviders={hasEnabledProviders}
        onClose={() => setConflictConfirmTarget(null)}
        onConfirmTakeover={handleConfirmTakeover}
      />
    </div>
  );
};
