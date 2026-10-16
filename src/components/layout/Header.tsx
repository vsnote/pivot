import React from 'react';
import { useAppStore } from '@/store/useAppStore';
import { Tabs, TabsList, TabsTrigger } from '@/components/common/Tabs';
import { Button } from '@/components/common/Button';
import { Plus } from 'lucide-react';

const tabs = [
  { id: 'providers', label: '服务商' },
  { id: 'clients', label: '客户端' },
  { id: 'logs', label: '日志' },
  { id: 'settings', label: '设置' },
] as const;

type TabId = (typeof tabs)[number]['id'];

export const Header: React.FC = () => {
  const { activeTab, settingsSection, setActiveTab, setSettingsSection, openProviderEditor } =
    useAppStore();

  const current: TabId =
    activeTab === 'clients'
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
      className="relative h-11 sm:h-12 w-full px-4 select-none shrink-0 z-40 bg-background/80 border-b border-border backdrop-blur-md flex items-center justify-between"
      style={{ WebkitAppRegion: 'drag' } as React.CSSProperties}
    >
      {/* 左侧：macOS 原生红绿灯避让留白区 (保持与右侧对称) */}
      <div className="w-16 shrink-0 pointer-events-none" />

      {/* 中间：居中导航 */}
      <div style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}>
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

      {/* 右侧：留白且与背景完全融为一体，仅在服务商页提供轻量新增入口 */}
      <div
        className="w-16 shrink-0 flex items-center justify-end"
        style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
      >
        {current === 'providers' && (
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            onClick={() => openProviderEditor(null)}
            aria-label="添加服务商"
            title="添加服务商"
            className="text-muted-foreground hover:text-foreground"
          >
            <Plus className="w-4 h-4" />
          </Button>
        )}
      </div>
    </header>
  );
};
