import React from 'react';
import { useAppStore } from '@/store/useAppStore';
import { Moon, Sun, Plus, ScrollText, Info } from 'lucide-react';

export const Header: React.FC = () => {
  const {
    activeTab,
    setActiveTab,
    settingsSection,
    setSettingsSection,
    theme,
    setTheme,
    openProviderEditor,
    closeProviderEditor,
    isProviderEditorOpen,
  } = useAppStore();

  const isLogsActive = activeTab === 'logs' || (activeTab === 'settings' && settingsSection === 'logs');
  const isSettingsActive = activeTab === 'settings' && (settingsSection === 'about' || settingsSection === 'clients');

  const handleToggleLogs = () => {
    if (isLogsActive) {
      setActiveTab('providers');
      closeProviderEditor();
    } else {
      setActiveTab('logs');
      setSettingsSection('logs');
      closeProviderEditor();
    }
  };

  const handleToggleSettings = () => {
    if (isSettingsActive) {
      setActiveTab('providers');
      closeProviderEditor();
    } else {
      setActiveTab('settings');
      setSettingsSection('about');
      closeProviderEditor();
    }
  };

  const handleHome = () => {
    setActiveTab('providers');
    closeProviderEditor();
  };

  return (
    <header
      data-tauri-drag-region
      className="relative h-13 sm:h-14 w-full px-4 sm:px-6 select-none shrink-0 z-40 bg-white/80 dark:bg-zinc-950/80 border-b border-zinc-200/80 dark:border-zinc-800/80 backdrop-blur-xl flex items-center justify-between"
      style={{ WebkitAppRegion: 'drag' } as React.CSSProperties}
    >
      {/* 左侧：避开交通灯(约70px)并排对齐的应用名称 */}
      <div className="flex items-center pl-[76px]" data-tauri-drag-region>
        <button
          type="button"
          onClick={handleHome}
          className="text-[14px] font-bold tracking-tight text-zinc-900 dark:text-zinc-100 hover:text-indigo-600 dark:hover:text-indigo-400 transition-colors select-none cursor-pointer"
          style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
          title="返回首页"
        >
          OmniSwitch
        </button>
      </div>

      {/* 右侧：极简图标操作区（顺序：添加 -> 日志 -> 外观 -> 关于） */}
      <div
        className="flex items-center gap-2"
        style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
      >
        {/* 1. 添加服务商 */}
        <button
          onClick={() => {
            setActiveTab('providers');
            openProviderEditor(null);
          }}
          className={`w-8 h-8 flex items-center justify-center rounded-lg transition-colors cursor-pointer ${
            isProviderEditorOpen
              ? 'bg-indigo-50 dark:bg-indigo-950/60 text-indigo-600 dark:text-indigo-400 border border-indigo-200/80 dark:border-indigo-800 shadow-2xs'
              : 'text-zinc-500 dark:text-zinc-400 hover:text-indigo-600 dark:hover:text-indigo-400 hover:bg-zinc-100 dark:hover:bg-zinc-800'
          }`}
          aria-label="添加服务商"
          title="添加服务商"
        >
          <Plus className="w-4 h-4" />
        </button>

        {/* 2. 日志与指标 */}
        <button
          onClick={handleToggleLogs}
          className={`w-8 h-8 flex items-center justify-center rounded-lg transition-colors cursor-pointer ${
            isLogsActive
              ? 'bg-indigo-50 dark:bg-indigo-950/60 text-indigo-600 dark:text-indigo-400 border border-indigo-200/80 dark:border-indigo-800 shadow-2xs'
              : 'text-zinc-500 dark:text-zinc-400 hover:text-indigo-600 dark:hover:text-indigo-400 hover:bg-zinc-100 dark:hover:bg-zinc-800'
          }`}
          aria-label="请求日志"
          title="请求日志"
        >
          <ScrollText className="w-4 h-4" />
        </button>

        {/* 3. 外观切换 */}
        <button
          onClick={() => setTheme(document.documentElement.classList.contains('dark') ? 'light' : 'dark')}
          className="w-8 h-8 flex items-center justify-center rounded-lg text-zinc-500 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100 hover:bg-zinc-100 dark:hover:bg-zinc-800 transition-colors cursor-pointer"
          aria-label="切换明暗主题"
          title="切换明暗主题"
        >
          {theme === 'dark' ? <Sun className="w-4 h-4" /> : <Moon className="w-4 h-4" />}
        </button>

        {/* 4. 关于（最右侧） */}
        <button
          onClick={handleToggleSettings}
          className={`w-8 h-8 flex items-center justify-center rounded-lg transition-colors cursor-pointer ${
            isSettingsActive
              ? 'bg-indigo-50 dark:bg-indigo-950/60 text-indigo-600 dark:text-indigo-400 border border-indigo-200/80 dark:border-indigo-800 shadow-2xs'
              : 'text-zinc-500 dark:text-zinc-400 hover:text-indigo-600 dark:hover:text-indigo-400 hover:bg-zinc-100 dark:hover:bg-zinc-800'
          }`}
          aria-label="关于 OmniSwitch"
          title="关于 OmniSwitch"
        >
          <Info className="w-4 h-4" />
        </button>
      </div>
    </header>
  );
};
