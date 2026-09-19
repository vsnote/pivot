import React from 'react';
import { useAppStore } from '@/store/useAppStore';
import { SettingsSection } from '@/types';
import { Moon, Sun, Plus } from 'lucide-react';

interface NavItem {
  id: 'providers' | SettingsSection;
  label: string;
}

export const Header: React.FC = () => {
  const {
    activeTab,
    setActiveTab,
    settingsSection,
    setSettingsSection,
    theme,
    setTheme,
    openProviderEditor,
  } = useAppStore();

  const currentTabId: 'providers' | SettingsSection =
    activeTab === 'providers' ? 'providers' : settingsSection;

  const navTabs: NavItem[] = [
    { id: 'providers', label: '首页' },
    { id: 'routes', label: '路由' },
    { id: 'logs', label: '日志' },
    { id: 'gateway', label: '监控' },
    { id: 'about', label: '设置' },
  ];

  const handleSelectTab = (id: 'providers' | SettingsSection) => {
    if (id === 'providers') {
      setActiveTab('providers');
    } else {
      setSettingsSection(id);
    }
  };

  return (
    <header
      data-tauri-drag-region
      className="relative h-13 sm:h-14 w-full px-5 sm:px-7 select-none shrink-0 z-40 bg-white/80 dark:bg-zinc-950/80 border-b border-zinc-200/80 dark:border-zinc-800/80 backdrop-blur-xl flex items-center justify-between"
      style={{ WebkitAppRegion: 'drag' } as React.CSSProperties}
    >
      {/* 左侧：macOS 交通灯预留安全区（~80px）与纯净应用名称 */}
      <div className="flex items-center pl-20 sm:pl-22" data-tauri-drag-region>
        <button
          type="button"
          onClick={() => setActiveTab('providers')}
          className="text-[14px] font-bold tracking-tight text-zinc-900 dark:text-zinc-100 hover:text-indigo-600 dark:hover:text-indigo-400 transition-colors select-none cursor-pointer"
          style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
          title="返回首页"
        >
          OmniSwitch
        </button>
      </div>

      {/* 中央：舒展对称的 Tab 导航分段器 */}
      <div
        className="flex items-center justify-center"
        style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
      >
        <nav
          className="flex items-center gap-1 p-1 bg-zinc-100/90 dark:bg-zinc-900/90 border border-zinc-200/80 dark:border-zinc-800/80 rounded-xl shadow-2xs"
        >
          {navTabs.map((tab) => {
            const isActive = currentTabId === tab.id;
            return (
              <button
                key={tab.id}
                data-nav-id={tab.id}
                onClick={() => handleSelectTab(tab.id)}
                className={`px-3.5 sm:px-4 h-7 text-xs sm:text-[13px] flex items-center justify-center rounded-lg transition-all duration-150 cursor-pointer whitespace-nowrap shrink-0 ${
                  isActive
                    ? 'bg-white dark:bg-zinc-800 text-zinc-950 dark:text-white shadow-xs font-semibold'
                    : 'text-zinc-500 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100 font-medium'
                }`}
              >
                {tab.label}
              </button>
            );
          })}
        </nav>
      </div>

      {/* 右侧：极简快捷操作 */}
      <div
        className="flex items-center gap-2"
        style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
      >
        {/* 添加服务商快捷按钮 */}
        <button
          onClick={() => openProviderEditor(null)}
          className="w-8 h-8 flex items-center justify-center rounded-lg text-zinc-600 dark:text-zinc-300 hover:text-indigo-600 dark:hover:text-indigo-400 hover:bg-zinc-100 dark:hover:bg-zinc-850 border border-zinc-200/70 dark:border-zinc-800 transition-colors cursor-pointer"
          aria-label="添加服务商"
          title="添加服务商"
        >
          <Plus className="w-4 h-4" />
        </button>

        {/* 外观切换 */}
        <button
          onClick={() => setTheme(document.documentElement.classList.contains('dark') ? 'light' : 'dark')}
          className="w-8 h-8 flex items-center justify-center rounded-lg text-zinc-500 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100 hover:bg-zinc-100 dark:hover:bg-zinc-850 border border-zinc-200/70 dark:border-zinc-800 transition-colors cursor-pointer"
          aria-label="切换明暗主题"
          title="切换明暗主题"
        >
          {theme === 'dark' ? <Sun className="w-4 h-4" /> : <Moon className="w-4 h-4" />}
        </button>
      </div>
    </header>
  );
};
