import React from 'react';
import { useAppStore } from '@/store/useAppStore';
import { Moon, Sun, Terminal, ShieldCheck, Zap } from 'lucide-react';

export const Header: React.FC = () => {
  const { stats, theme, setTheme, interceptors } = useAppStore();

  const claudeIntercepted = interceptors.find((i) => i.id === 'claude_code')?.isIntercepted;
  const codexIntercepted = interceptors.find((i) => i.id === 'codex_cli')?.isIntercepted;

  return (
    <header className="h-16 border-b border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-900/90 backdrop-blur px-6 flex items-center justify-between select-none">
      {/* Brand & Logo */}
      <div className="flex items-center gap-3">
        <div className="w-9 h-9 rounded-xl bg-gradient-to-tr from-indigo-600 to-violet-500 flex items-center justify-center text-white shadow-md shadow-indigo-500/20">
          <Zap className="w-5 h-5" />
        </div>
        <div>
          <div className="flex items-center gap-2">
            <h1 className="text-base font-bold tracking-tight text-gray-900 dark:text-white">
              OmniSwitch
            </h1>
            <span className="text-[10px] uppercase font-mono px-1.5 py-0.5 rounded bg-indigo-50 dark:bg-indigo-950/60 text-indigo-600 dark:text-indigo-400 font-semibold border border-indigo-200/50 dark:border-indigo-800/40">
              v0.1.0
            </span>
          </div>
          <p className="text-xs text-gray-500 dark:text-gray-400">
            Native AI Routing & Gateway
          </p>
        </div>
      </div>

      {/* Gateway Status Badge & Interceptor Status */}
      <div className="flex items-center gap-3">
        {/* Gateway live pill */}
        <div className="flex items-center gap-2 px-3 py-1.5 rounded-full bg-emerald-50 dark:bg-emerald-950/40 border border-emerald-200/60 dark:border-emerald-800/50 text-emerald-700 dark:text-emerald-300 text-xs font-mono">
          <span className="relative flex h-2 w-2">
            <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-emerald-400 opacity-75"></span>
            <span className="relative inline-flex rounded-full h-2 w-2 bg-emerald-500"></span>
          </span>
          <span>127.0.0.1:{stats.port}</span>
          {stats.activeRequests > 0 && (
            <span className="ml-1 px-1.5 py-0.2 bg-emerald-200 dark:bg-emerald-800 text-[10px] font-bold rounded-full">
              {stats.activeRequests} active
            </span>
          )}
        </div>

        {/* Takeover Badges */}
        <div className="hidden sm:flex items-center gap-1.5 text-xs">
          <div
            className={`flex items-center gap-1 px-2.5 py-1 rounded-md border font-mono transition-colors ${
              claudeIntercepted
                ? 'bg-amber-50 dark:bg-amber-950/40 text-amber-700 dark:text-amber-300 border-amber-300 dark:border-amber-800'
                : 'bg-gray-100 dark:bg-gray-800/60 text-gray-500 dark:text-gray-400 border-gray-200 dark:border-gray-700'
            }`}
          >
            <Terminal className="w-3 h-3" />
            <span>Claude Code: {claudeIntercepted ? 'Intercepted' : 'Direct'}</span>
          </div>

          <div
            className={`flex items-center gap-1 px-2.5 py-1 rounded-md border font-mono transition-colors ${
              codexIntercepted
                ? 'bg-amber-50 dark:bg-amber-950/40 text-amber-700 dark:text-amber-300 border-amber-300 dark:border-amber-800'
                : 'bg-gray-100 dark:bg-gray-800/60 text-gray-500 dark:text-gray-400 border-gray-200 dark:border-gray-700'
            }`}
          >
            <ShieldCheck className="w-3 h-3" />
            <span>Codex: {codexIntercepted ? 'Intercepted' : 'Direct'}</span>
          </div>
        </div>

        {/* Theme Toggle Button */}
        <button
          onClick={() => setTheme(theme === 'dark' ? 'light' : 'dark')}
          className="p-2 rounded-lg text-gray-600 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
          title="Toggle Theme"
        >
          {theme === 'dark' ? <Sun className="w-4 h-4" /> : <Moon className="w-4 h-4" />}
        </button>
      </div>
    </header>
  );
};
