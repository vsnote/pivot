import React from 'react';
import { useAppStore } from '@/store/useAppStore';
import {
  ArrowLeft,
  Sun,
  Moon,
  Laptop,
  Palette,
  ShieldCheck,
} from 'lucide-react';
import { PivotLogo } from '@/components/common/PivotLogo';

export const SettingsView: React.FC = () => {
  const { setActiveTab, theme, setTheme } = useAppStore();

  return (
    <div className="w-full pb-10 space-y-3 animate-in fade-in duration-150">
      {/* 顶部纯粹的「返回」栏与页面标题 */}
      <div className="flex items-center justify-between pb-1">
        <button
          type="button"
          onClick={() => setActiveTab('providers')}
          className="inline-flex items-center gap-1.5 px-2.5 py-1 -ml-2.5 rounded-lg text-xs font-medium text-zinc-500 hover:text-zinc-900 dark:text-zinc-400 dark:hover:text-zinc-100 hover:bg-zinc-100 dark:hover:bg-zinc-800 transition-colors cursor-pointer"
        >
          <ArrowLeft className="w-3.5 h-3.5" />
          <span>返回</span>
        </button>
      </div>

      {/* 外观与个性化偏好面板 */}
      <div className="rounded-lg border border-zinc-200/80 dark:border-zinc-800 bg-white dark:bg-zinc-900 shadow-xs overflow-hidden">
        <div className="px-5 py-3 border-b border-zinc-100 dark:border-zinc-800/80 bg-zinc-50/50 dark:bg-zinc-850/30 flex items-center justify-between">
          <div className="flex items-center gap-2 text-xs font-bold text-zinc-900 dark:text-zinc-100">
            <Palette className="w-4 h-4 text-cyan-600 dark:text-cyan-400" />
            <span>界面与外观</span>
          </div>
        </div>

        <div className="p-5 flex flex-col sm:flex-row sm:items-center justify-between gap-3">
          <div>
            <div className="text-xs font-semibold text-zinc-900 dark:text-zinc-100">主题外观</div>
            <div className="text-[11px] text-zinc-400 dark:text-zinc-500 mt-0.5">
              选择适合您的显示风格，或跟随操作系统自动切换
            </div>
          </div>

          {/* 三段式主题切换器 */}
          <div className="flex items-center p-0.5 rounded-md bg-zinc-100 dark:bg-zinc-800 border border-zinc-200/80 dark:border-zinc-700/80 w-fit shrink-0">
            <button
              type="button"
              onClick={() => setTheme('light')}
              className={`flex items-center gap-1.5 px-3 py-1.5 rounded-sm text-xs font-medium transition-all cursor-pointer ${
                theme === 'light'
                  ? 'bg-white text-zinc-950 shadow-xs font-semibold'
                  : 'text-zinc-500 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100'
              }`}
            >
              <Sun className="w-3.5 h-3.5 text-amber-500" />
              <span>浅色</span>
            </button>
            <button
              type="button"
              onClick={() => setTheme('dark')}
              className={`flex items-center gap-1.5 px-3 py-1.5 rounded-sm text-xs font-medium transition-all cursor-pointer ${
                theme === 'dark'
                  ? 'bg-zinc-900 text-white shadow-xs font-semibold'
                  : 'text-zinc-500 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100'
              }`}
            >
              <Moon className="w-3.5 h-3.5 text-sky-400" />
              <span>深色</span>
            </button>
            <button
              type="button"
              onClick={() => setTheme('system')}
              className={`flex items-center gap-1.5 px-3 py-1.5 rounded-sm text-xs font-medium transition-all cursor-pointer ${
                theme === 'system'
                  ? 'bg-white dark:bg-zinc-900 text-zinc-950 dark:text-white shadow-xs font-semibold'
                  : 'text-zinc-500 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100'
              }`}
            >
              <Laptop className="w-3.5 h-3.5 text-emerald-500" />
              <span>跟随系统</span>
            </button>
          </div>
        </div>
      </div>

      {/* 应用版本与关于卡片 */}
      <div className="rounded-lg border border-zinc-200/80 dark:border-zinc-800 bg-white dark:bg-zinc-900 shadow-xs overflow-hidden">
        <div className="px-5 py-3 border-b border-zinc-100 dark:border-zinc-800/80 bg-zinc-50/50 dark:bg-zinc-850/30 flex items-center justify-between">
          <div className="flex items-center gap-2 text-xs font-bold text-zinc-900 dark:text-zinc-100">
            <ShieldCheck className="w-4 h-4 text-cyan-600 dark:text-cyan-400" />
            <span>关于 Pivot</span>
          </div>
        </div>

        <div className="p-5 flex items-start gap-4">
          <div className="w-11 h-11 flex items-center justify-center shrink-0 mt-0.5">
            <PivotLogo className="w-full h-full drop-shadow-md" variant="icon" state="active" />
          </div>
          <div className="space-y-1">
            <div className="flex items-center gap-2">
              <h3 className="text-sm font-bold text-zinc-950 dark:text-white">
                Pivot
              </h3>
              <span className="text-[10px] font-mono font-medium px-1.5 py-0.2 rounded-md bg-cyan-50 dark:bg-cyan-950/60 text-cyan-600 dark:text-cyan-400 border border-cyan-200/60 dark:border-cyan-800/60">
                v0.1.0-alpha
              </span>
            </div>
            <p className="text-xs text-zinc-500 dark:text-zinc-400">
              面向开发者的下一代本地 AI 智能网关与模型中继器
            </p>
            <p className="text-[11px] text-zinc-400 dark:text-zinc-500 pt-1">
              纯本地运行 · 密钥与日志仅保存在本机 · 无云端遥测
            </p>
          </div>
        </div>
      </div>
    </div>
  );
};
