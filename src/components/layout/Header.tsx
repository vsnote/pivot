import React from 'react';
import { useAppStore } from '@/store/useAppStore';
import { Plus, ScrollText, Cloud, Settings } from 'lucide-react';

export const Header: React.FC = () => {
  const {
    activeTab,
    setActiveTab,
    settingsSection,
    setSettingsSection,
    openProviderEditor,
    isProviderEditorOpen,
  } = useAppStore();

  const isLogsActive = activeTab === 'logs' || (activeTab === 'settings' && settingsSection === 'logs');
  const isSettingsActive = activeTab === 'settings' && (settingsSection === 'about' || settingsSection === 'clients');
  const isSyncActive = activeTab === 'sync';

  const handleToggleLogs = () => {
    if (isLogsActive) {
      setActiveTab('providers');
    } else {
      setActiveTab('logs');
      setSettingsSection('logs');
    }
  };

  const handleToggleSync = () => {
    if (isSyncActive) {
      setActiveTab('providers');
    } else {
      setActiveTab('sync');
    }
  };

  const handleToggleSettings = () => {
    if (isSettingsActive) {
      setActiveTab('providers');
    } else {
      setActiveTab('settings');
      setSettingsSection('about');
    }
  };

  return (
    <header
      data-tauri-drag-region
      className="relative h-13 sm:h-14 w-full px-4 sm:px-6 select-none shrink-0 z-40 bg-white/80 dark:bg-zinc-950/80 border-b border-zinc-200/80 dark:border-zinc-800/80 backdrop-blur-xl flex items-center justify-between"
      style={{ WebkitAppRegion: 'drag' } as React.CSSProperties}
    >
      {/* 左侧：避开系统红绿灯(约76px)的原生工具栏纯净拖拽区 */}
      <div className="flex items-center pl-[76px] h-full" data-tauri-drag-region />

      {/* 右侧：极简图标操作区（顺序：添加 -> 会话云同步 -> 日志 -> 设置） */}
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
            isProviderEditorOpen && activeTab === 'providers'
              ? 'bg-cyan-50 dark:bg-cyan-950/60 text-cyan-600 dark:text-cyan-400 border border-cyan-200/80 dark:border-cyan-800 shadow-2xs'
              : 'text-zinc-500 dark:text-zinc-400 hover:text-cyan-600 dark:hover:text-cyan-400 hover:bg-zinc-100 dark:hover:bg-zinc-800'
          }`}
          aria-label="添加服务商"
          title="添加服务商"
        >
          <Plus className="w-4 h-4" />
        </button>

        {/* 2. 会话云同步（调换后在日志前面） */}
        <button
          onClick={handleToggleSync}
          className={`w-8 h-8 flex items-center justify-center rounded-lg transition-colors cursor-pointer ${
            isSyncActive
              ? 'bg-cyan-50 dark:bg-cyan-950/60 text-cyan-600 dark:text-cyan-400 border border-cyan-200/80 dark:border-cyan-800 shadow-2xs'
              : 'text-zinc-500 dark:text-zinc-400 hover:text-cyan-600 dark:hover:text-cyan-400 hover:bg-zinc-100 dark:hover:bg-zinc-800'
          }`}
          aria-label="会话云同步"
          title="会话云同步"
        >
          <Cloud className="w-4 h-4" />
        </button>

        {/* 3. 日志与指标 */}
        <button
          onClick={handleToggleLogs}
          className={`w-8 h-8 flex items-center justify-center rounded-lg transition-colors cursor-pointer ${
            isLogsActive
              ? 'bg-cyan-50 dark:bg-cyan-950/60 text-cyan-600 dark:text-cyan-400 border border-cyan-200/80 dark:border-cyan-800 shadow-2xs'
              : 'text-zinc-500 dark:text-zinc-400 hover:text-cyan-600 dark:hover:text-cyan-400 hover:bg-zinc-100 dark:hover:bg-zinc-800'
          }`}
          aria-label="请求日志"
          title="请求日志"
        >
          <ScrollText className="w-4 h-4" />
        </button>

        {/* 4. 系统设置（包含外观切换与关于） */}
        <button
          onClick={handleToggleSettings}
          className={`w-8 h-8 flex items-center justify-center rounded-lg transition-colors cursor-pointer ${
            isSettingsActive
              ? 'bg-cyan-50 dark:bg-cyan-950/60 text-cyan-600 dark:text-cyan-400 border border-cyan-200/80 dark:border-cyan-800 shadow-2xs'
              : 'text-zinc-500 dark:text-zinc-400 hover:text-cyan-600 dark:hover:text-cyan-400 hover:bg-zinc-100 dark:hover:bg-zinc-800'
          }`}
          aria-label="系统设置"
          title="系统设置"
        >
          <Settings className="w-4 h-4" />
        </button>
      </div>
    </header>
  );
};
