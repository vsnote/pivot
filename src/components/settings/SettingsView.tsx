import React from 'react';
import { useAppStore } from '@/store/useAppStore';
import { Tabs, TabsList, TabsTrigger } from '@/components/common/Tabs';

const themeOptions = [
  { id: 'light', label: '浅色' },
  { id: 'dark', label: '深色' },
  { id: 'system', label: '跟随系统' },
] as const;

export const SettingsView: React.FC = () => {
  const { theme, setTheme } = useAppStore();

  return (
    <div className="w-full pb-10 space-y-4 animate-in fade-in duration-150">
      <div className="rounded-lg border border-border bg-card text-card-foreground shadow-2xs overflow-hidden divide-y divide-border">
        {/* 卡片头部 */}
        <div className="px-4 py-2.5 bg-muted/30">
          <span className="text-xs font-semibold text-zinc-700 dark:text-zinc-200 tracking-wide">
            偏好与关于
          </span>
        </div>

        {/* 外观切换项 */}
        <div className="p-4 flex items-center justify-between gap-4">
          <div className="min-w-0">
            <div className="text-xs font-medium text-zinc-900 dark:text-zinc-100">界面外观</div>
            <div className="text-[11px] text-zinc-400 dark:text-zinc-500 mt-0.5">
              选择适合的显示风格，或跟随操作系统自动切换
            </div>
          </div>

          {/* 全局统一 Tabs 组件库 */}
          <Tabs value={theme} onValueChange={(val) => setTheme(val as 'light' | 'dark' | 'system')}>
            <TabsList aria-label="主题偏好切换">
              {themeOptions.map((opt) => (
                <TabsTrigger key={opt.id} value={opt.id}>
                  {opt.label}
                </TabsTrigger>
              ))}
            </TabsList>
          </Tabs>
        </div>

        {/* 关于信息条 */}
        <div className="p-4 space-y-1 bg-muted/10">
          <div className="flex items-center gap-2">
            <h3 className="text-xs font-bold tracking-wider text-zinc-950 dark:text-white font-mono">
              PIVOT
            </h3>
            <span className="text-[10px] font-mono px-1.5 py-0.2 rounded bg-zinc-100 dark:bg-zinc-800 text-zinc-600 dark:text-zinc-400 border border-zinc-200/60 dark:border-zinc-700/60">
              v0.1.0-alpha
            </span>
          </div>
          <p className="text-[11px] text-zinc-500 dark:text-zinc-400">
            面向开发者的本地 AI 智能网关与模型中继器
          </p>
          <p className="text-[10px] text-zinc-400 dark:text-zinc-500 pt-0.5">
            纯本地运行 · 密钥与日志仅保存在本机 · 无云端遥测
          </p>
        </div>
      </div>
    </div>
  );
};
