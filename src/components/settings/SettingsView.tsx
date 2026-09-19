import React from 'react';
import { useAppStore } from '@/store/useAppStore';
import { Globe, ArrowLeft } from 'lucide-react';

export const SettingsView: React.FC = () => {
  const { setActiveTab } = useAppStore();

  return (
    <div className="w-full pb-10 space-y-4 animate-in fade-in duration-150">
      {/* 顶部纯粹的「返回」栏 */}
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

      {/* 应用版本与简介卡片（纯净、已做技术脱敏） */}
      <div className="p-6 rounded-2xl border border-zinc-200/80 dark:border-zinc-800 bg-white dark:bg-zinc-900 shadow-xs space-y-6">
        <div className="flex items-center gap-3.5">
          <div className="w-10 h-10 rounded-xl bg-indigo-600 dark:bg-indigo-500 text-white flex items-center justify-center shadow-md shadow-indigo-500/25">
            <Globe className="w-5 h-5" />
          </div>
          <div>
            <h3 className="text-sm font-bold text-zinc-950 dark:text-white flex items-center gap-2">
              OmniSwitch
              <span className="text-[11px] font-mono font-medium px-2 py-0.5 rounded-full bg-indigo-50 dark:bg-indigo-950/60 text-indigo-600 dark:text-indigo-400 border border-indigo-200/60 dark:border-indigo-800/60">
                v0.1.0-alpha
              </span>
            </h3>
            <p className="text-xs text-zinc-500 dark:text-zinc-400 mt-0.5">
              面向开发者的下一代本地 AI 智能网关与模型路由器
            </p>
            <p className="text-[11px] text-zinc-400 dark:text-zinc-400 mt-1.5">
              纯本地运行 · 密钥与日志仅保存在本机 · 无云端遥测
            </p>
          </div>
        </div>
      </div>
    </div>
  );
};
