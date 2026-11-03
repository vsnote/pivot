import React from 'react';
import { useAppStore } from '@/store/useAppStore';
import { Tabs, TabsList, TabsTrigger } from '@/components/common/Tabs';
import { Plus } from 'lucide-react';

const tabs = [
  { id: 'providers', label: '服务商' },
  { id: 'routes', label: '路由' },
  { id: 'clients', label: '客户端' },
  { id: 'logs', label: '日志' },
  { id: 'settings', label: '设置' },
] as const;

type TabId = (typeof tabs)[number]['id'];

export const Header: React.FC = () => {
  const {
    activeTab,
    settingsSection,
    setActiveTab,
    setSettingsSection,
    isProviderEditorOpen,
    openProviderEditor,
  } = useAppStore();

  const current: TabId =
    activeTab === 'routes'
      ? 'routes'
      : activeTab === 'clients'
        ? 'clients'
        : activeTab === 'logs' || (activeTab === 'settings' && settingsSection === 'logs')
          ? 'logs'
          : activeTab === 'settings'
            ? 'settings'
            : 'providers';

  const selectTab = (id: string) => {
    if (id === 'settings') {
      setSettingsSection('about');
      return;
    }
    setActiveTab(id as TabId);
  };

  return (
    <header
      data-tauri-drag-region
      className="relative h-12 w-full px-4 select-none shrink-0 z-40 bg-background/80 border-b border-border backdrop-blur-md flex items-center justify-end"
      style={{ WebkitAppRegion: 'drag' } as React.CSSProperties}
    >
      {/* 居中导航 */}
      <div
        className="absolute left-1/2 -translate-x-1/2 top-1/2 -translate-y-1/2"
        style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
      >
        <Tabs value={current} onValueChange={selectTab}>
          <TabsList variant="ghost" aria-label="主要页面导航">
            {tabs.map((tab) => (
              <TabsTrigger key={tab.id} value={tab.id}>
                {tab.label}
              </TabsTrigger>
            ))}
          </TabsList>
        </Tabs>
      </div>

      {/* 右侧功能操作区 */}
      <div
        className="flex items-center justify-end"
        style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
      >
        {current === 'providers' && !isProviderEditorOpen && (
          <button
            type="button"
            onClick={() => openProviderEditor(null)}
            title="添加服务商 (⌘N)"
            aria-label="添加服务商"
            className="p-1 rounded-md text-muted-foreground hover:text-foreground hover:bg-muted/80 transition-colors cursor-pointer"
          >
            <Plus className="w-4 h-4 stroke-[2.2]" />
          </button>
        )}
      </div>
    </header>
  );
};
