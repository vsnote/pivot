import React, { useState } from 'react';
import { useAppStore } from '@/store/useAppStore';
import { Switch } from '@/components/common/Switch';
import {
  Terminal,
  ShieldCheck,
  CheckCircle2,
  AlertCircle,
  Copy,
  Check,
  RotateCcw,
  Sparkles,
} from 'lucide-react';

const DEFAULT_INTERCEPTORS: any[] = [
  {
    id: 'claude_code',
    name: 'Claude Code',
    installed: true,
    configPath: '~/.claude.json',
    isIntercepted: false,
  },
  {
    id: 'codex_cli',
    name: 'Codex CLI',
    installed: true,
    configPath: '~/.codex/config.json',
    isIntercepted: false,
  },
  {
    id: 'gemini_cli',
    name: 'Gemini CLI',
    installed: false,
    configPath: '~/.gemini/config.json',
    isIntercepted: false,
  },
];

export const InterceptorPanel: React.FC = () => {
  const { interceptors, toggleInterceptor, stats } = useAppStore();
  const [togglingId, setTogglingId] = useState<string | null>(null);
  const [copiedKey, setCopiedKey] = useState<string | null>(null);

  const displayList = interceptors.length > 0 ? interceptors : DEFAULT_INTERCEPTORS;

  const handleToggle = async (id: string, enable: boolean) => {
    setTogglingId(id);
    try {
      await toggleInterceptor(id, enable);
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
    <div className="space-y-4 pb-8 animate-in fade-in duration-150">
      {/* 客户端卡片列表 */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-3.5">
        {displayList.map((tool) => {
          const isToggling = togglingId === tool.id;

          return (
            <div
              key={tool.id}
              className={`rounded-2xl p-4.5 flex flex-col justify-between transition-all duration-150 bg-white dark:bg-[#1a1b1e] border shadow-xs ${
                tool.isIntercepted
                  ? 'border-indigo-500/50 dark:border-indigo-500/50 ring-1 ring-indigo-500/20'
                  : 'border-gray-200/80 dark:border-white/10 hover:border-gray-300 dark:hover:border-white/20'
              }`}
            >
              <div>
                {/* Header */}
                <div className="flex items-start justify-between gap-2 mb-3.5">
                  <div className="flex items-center gap-2.5">
                    <div
                      className={`w-9 h-9 rounded-xl flex items-center justify-center ${
                        tool.isIntercepted
                          ? 'bg-indigo-600 text-white shadow-xs shadow-indigo-500/30'
                          : 'bg-gray-100 dark:bg-white/[0.06] text-gray-600 dark:text-zinc-300'
                      }`}
                    >
                      {tool.id === 'claude_code' ? (
                        <Terminal className="w-4.5 h-4.5" />
                      ) : tool.id === 'codex_cli' ? (
                        <ShieldCheck className="w-4.5 h-4.5" />
                      ) : (
                        <Sparkles className="w-4.5 h-4.5" />
                      )}
                    </div>
                    <div>
                      <h3 className="text-sm font-semibold text-gray-950 dark:text-white">
                        {tool.name}
                      </h3>
                      <div className="flex items-center gap-1.5 mt-0.5">
                        {tool.installed ? (
                          <span className="flex items-center gap-1 text-[11px] text-emerald-600 dark:text-emerald-400 font-medium">
                            <CheckCircle2 className="w-3 h-3" />
                            已检测到客户端
                          </span>
                        ) : (
                          <span className="flex items-center gap-1 text-[11px] text-gray-400">
                            <AlertCircle className="w-3 h-3" />
                            未安装或未初始化
                          </span>
                        )}
                      </div>
                    </div>
                  </div>

                  {/* Switch */}
                  <Switch
                    checked={tool.isIntercepted}
                    disabled={isToggling}
                    onCheckedChange={(checked) => handleToggle(tool.id, checked)}
                    ariaLabel={`${tool.isIntercepted ? '关闭' : '开启'} ${tool.name} 代理`}
                    size="md"
                  />
                </div>

                {/* 接管状态指示 */}
                <div className="mb-3">
                  <div
                    className={`p-2 rounded-lg border text-xs font-mono flex items-center justify-between ${
                      tool.isIntercepted
                        ? 'bg-indigo-50/70 dark:bg-indigo-950/40 border-indigo-200/60 dark:border-indigo-800/60 text-indigo-700 dark:text-indigo-300'
                        : 'bg-gray-50/70 dark:bg-white/[0.02] border-gray-100 dark:border-white/5 text-gray-500 dark:text-zinc-400'
                    }`}
                  >
                    <span>路由状态</span>
                    <span className="font-semibold">
                      {tool.isIntercepted ? `已代理至端口 ${stats.port}` : '官方直连'}
                    </span>
                  </div>
                </div>

                {/* 配置文件路径 */}
                <div className="text-[11px] text-gray-500 dark:text-zinc-400 space-y-1 font-mono mb-3">
                  <div className="text-[10.5px] uppercase tracking-wider text-gray-400 dark:text-zinc-500 font-medium">
                    配置文件：
                  </div>
                  <div className="p-2 bg-gray-50/80 dark:bg-white/[0.03] rounded-lg border border-gray-100 dark:border-white/5 break-all text-[11px] select-all">
                    {tool.configPath}
                  </div>
                </div>
              </div>

              {/* 备份提示 */}
              <div className="pt-2.5 border-t border-gray-100 dark:border-white/5 flex items-center justify-between text-[11px] text-gray-400 dark:text-zinc-500">
                <div className="flex items-center gap-1">
                  <RotateCcw className="w-3 h-3 text-emerald-500" />
                  <span>关闭时自动恢复</span>
                </div>

                {isToggling && (
                  <span className="text-indigo-600 dark:text-indigo-400 animate-pulse font-medium">
                    写入中...
                  </span>
                )}
              </div>
            </div>
          );
        })}
      </div>

      {/* 终端环境变量 */}
      <div className="p-4.5 rounded-2xl border border-gray-200/80 dark:border-white/10 bg-white dark:bg-[#1a1b1e] shadow-xs space-y-2.5">
        <div className="flex items-center gap-2">
          <Terminal className="w-4 h-4 text-indigo-600 dark:text-indigo-400" />
          <h3 className="text-xs font-semibold text-gray-950 dark:text-white">
            终端临时环境变量
          </h3>
        </div>
        <p className="text-xs text-gray-500 dark:text-zinc-400">
          如需在特定 Shell 窗口临时指向 OmniSwitch，可直接复制运行以下命令：
        </p>

        <div className="grid grid-cols-1 sm:grid-cols-2 gap-2.5 pt-0.5">
          {/* Claude Code snippet */}
          <div className="flex items-center justify-between bg-gray-950 text-gray-100 p-2.5 px-3 rounded-xl font-mono text-xs border border-gray-800/90">
            <span className="truncate">{claudeExport}</span>
            <button
              onClick={() => copyToClipboard('claude', claudeExport)}
              aria-label="复制 Anthropic 环境变量命令"
              className="ml-2 p-1 rounded hover:bg-gray-800 text-gray-400 hover:text-white transition-colors cursor-pointer"
              title="复制命令"
            >
              {copiedKey === 'claude' ? (
                <Check className="w-3.5 h-3.5 text-emerald-400" />
              ) : (
                <Copy className="w-3.5 h-3.5" />
              )}
            </button>
          </div>

          {/* OpenAI snippet */}
          <div className="flex items-center justify-between bg-gray-950 text-gray-100 p-2.5 px-3 rounded-xl font-mono text-xs border border-gray-800/90">
            <span className="truncate">{openaiExport}</span>
            <button
              onClick={() => copyToClipboard('openai', openaiExport)}
              aria-label="复制 OpenAI 环境变量命令"
              className="ml-2 p-1 rounded hover:bg-gray-800 text-gray-400 hover:text-white transition-colors cursor-pointer"
              title="复制命令"
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
    </div>
  );
};
