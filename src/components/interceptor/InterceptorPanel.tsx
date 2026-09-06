import React, { useState } from 'react';
import { useAppStore } from '@/store/useAppStore';
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

export const InterceptorPanel: React.FC = () => {
  const { interceptors, toggleInterceptor, stats } = useAppStore();
  const [togglingId, setTogglingId] = useState<string | null>(null);
  const [copiedKey, setCopiedKey] = useState<string | null>(null);

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
    <div className="space-y-6">
      {/* Overview Banner */}
      <div className="p-5 rounded-2xl border border-indigo-200/80 dark:border-indigo-900/60 bg-gradient-to-r from-indigo-50/50 via-white to-violet-50/40 dark:from-indigo-950/20 dark:via-gray-900 dark:to-violet-950/20 shadow-sm flex flex-col md:flex-row md:items-center justify-between gap-4">
        <div>
          <div className="flex items-center gap-2">
            <h2 className="text-base font-bold text-gray-900 dark:text-gray-100">
              One-Click Client Takeover (App Interceptor)
            </h2>
            <span className="text-[11px] font-mono px-2 py-0.5 rounded bg-indigo-100 dark:bg-indigo-950 text-indigo-700 dark:text-indigo-300 font-semibold">
              Zero Config
            </span>
          </div>
          <p className="text-xs text-gray-600 dark:text-gray-400 mt-1 max-w-xl">
            Automatically modify client configurations to redirect all AI coding requests through
            OmniSwitch's high-speed local gateway. When toggled off, original configs are atomically
            restored.
          </p>
        </div>

        <div className="flex items-center gap-3">
          <div className="text-right hidden sm:block font-mono text-xs">
            <span className="text-gray-400">Gateway Target: </span>
            <span className="font-semibold text-emerald-600 dark:text-emerald-400">
              127.0.0.1:{stats.port}
            </span>
          </div>
        </div>
      </div>

      {/* Tool Cards */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-5">
        {interceptors.map((tool) => {
          const isToggling = togglingId === tool.id;

          return (
            <div
              key={tool.id}
              className={`border rounded-2xl p-5 flex flex-col justify-between transition-all duration-200 shadow-sm ${
                tool.isIntercepted
                  ? 'border-indigo-500/60 dark:border-indigo-500/60 bg-white dark:bg-gray-900 ring-1 ring-indigo-500/20'
                  : 'border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-900 hover:border-gray-300 dark:hover:border-gray-700'
              }`}
            >
              <div>
                {/* Header */}
                <div className="flex items-start justify-between gap-2 mb-4">
                  <div className="flex items-center gap-3">
                    <div
                      className={`w-10 h-10 rounded-xl flex items-center justify-center ${
                        tool.isIntercepted
                          ? 'bg-indigo-600 text-white shadow-md shadow-indigo-500/20'
                          : 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-300'
                      }`}
                    >
                      {tool.id === 'claude_code' ? (
                        <Terminal className="w-5 h-5" />
                      ) : tool.id === 'codex_cli' ? (
                        <ShieldCheck className="w-5 h-5" />
                      ) : (
                        <Sparkles className="w-5 h-5" />
                      )}
                    </div>
                    <div>
                      <h3 className="text-sm font-bold text-gray-900 dark:text-gray-100">
                        {tool.name}
                      </h3>
                      <div className="flex items-center gap-1.5 mt-0.5">
                        {tool.installed ? (
                          <span className="flex items-center gap-1 text-[11px] text-emerald-600 dark:text-emerald-400 font-medium">
                            <CheckCircle2 className="w-3 h-3" />
                            Detected
                          </span>
                        ) : (
                          <span className="flex items-center gap-1 text-[11px] text-gray-400">
                            <AlertCircle className="w-3 h-3" />
                            Standby
                          </span>
                        )}
                      </div>
                    </div>
                  </div>

                  {/* Switch */}
                  <label className="relative inline-flex items-center cursor-pointer select-none">
                    <input
                      type="checkbox"
                      checked={tool.isIntercepted}
                      disabled={isToggling}
                      onChange={(e) => handleToggle(tool.id, e.target.checked)}
                      className="sr-only peer"
                    />
                    <div className="w-11 h-6 bg-gray-200 peer-focus:outline-none rounded-full peer dark:bg-gray-700 peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all dark:border-gray-600 peer-checked:bg-indigo-600"></div>
                  </label>
                </div>

                {/* Status Indicator Pill */}
                <div className="mb-4">
                  <div
                    className={`p-2.5 rounded-xl border text-xs font-mono flex items-center justify-between ${
                      tool.isIntercepted
                        ? 'bg-indigo-50/60 dark:bg-indigo-950/40 border-indigo-200 dark:border-indigo-800/60 text-indigo-700 dark:text-indigo-300'
                        : 'bg-gray-50 dark:bg-gray-800/50 border-gray-100 dark:border-gray-800 text-gray-500 dark:text-gray-400'
                    }`}
                  >
                    <span>Status:</span>
                    <span className="font-semibold">
                      {tool.isIntercepted ? `Intercepted (: ${stats.port})` : 'Standard / Direct'}
                    </span>
                  </div>
                </div>

                {/* Config Path Info */}
                <div className="text-xs text-gray-500 dark:text-gray-400 space-y-1.5 font-mono mb-4">
                  <div className="text-[11px] uppercase tracking-wider text-gray-400 font-semibold">
                    Target Configuration:
                  </div>
                  <div className="p-2 bg-gray-50 dark:bg-gray-800/40 rounded-lg border border-gray-100 dark:border-gray-800 break-all text-[11px]">
                    {tool.configPath}
                  </div>
                </div>
              </div>

              {/* Card Footer: Backup safety notice */}
              <div className="pt-3 border-t border-gray-100 dark:border-gray-800/80 flex items-center justify-between text-[11px] text-gray-400">
                <div className="flex items-center gap-1">
                  <RotateCcw className="w-3 h-3 text-emerald-500" />
                  <span>Atomic Backup Safe</span>
                </div>

                {isToggling && (
                  <span className="text-indigo-600 dark:text-indigo-400 animate-pulse font-medium">
                    Applying...
                  </span>
                )}
              </div>
            </div>
          );
        })}
      </div>

      {/* Terminal Manual Snippets */}
      <div className="p-5 rounded-2xl border border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-900 shadow-sm space-y-3">
        <div className="flex items-center gap-2">
          <Terminal className="w-4 h-4 text-indigo-600 dark:text-indigo-400" />
          <h3 className="text-sm font-semibold text-gray-900 dark:text-gray-100">
            Manual Terminal Environment Variables (Alternative)
          </h3>
        </div>
        <p className="text-xs text-gray-500 dark:text-gray-400">
          Prefer manual terminal exports for custom scripts or transient shell sessions? Copy and run
          these in your terminal:
        </p>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-3 pt-1">
          {/* Claude Code snippet */}
          <div className="flex items-center justify-between bg-gray-900 text-gray-100 p-3 rounded-xl font-mono text-xs border border-gray-800">
            <span className="truncate">{claudeExport}</span>
            <button
              onClick={() => copyToClipboard('claude', claudeExport)}
              className="ml-2 p-1.5 rounded hover:bg-gray-800 text-gray-400 hover:text-white transition-colors"
              title="Copy export"
            >
              {copiedKey === 'claude' ? (
                <Check className="w-3.5 h-3.5 text-emerald-400" />
              ) : (
                <Copy className="w-3.5 h-3.5" />
              )}
            </button>
          </div>

          {/* OpenAI snippet */}
          <div className="flex items-center justify-between bg-gray-900 text-gray-100 p-3 rounded-xl font-mono text-xs border border-gray-800">
            <span className="truncate">{openaiExport}</span>
            <button
              onClick={() => copyToClipboard('openai', openaiExport)}
              className="ml-2 p-1.5 rounded hover:bg-gray-800 text-gray-400 hover:text-white transition-colors"
              title="Copy export"
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
