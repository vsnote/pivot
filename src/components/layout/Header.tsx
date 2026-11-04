import React from 'react';
import { useAppStore } from '@/store/useAppStore';
import { useTranslation } from '@/i18n';
import { Tabs, TabsList, TabsTrigger } from '@/components/common/Tabs';
import { Plus } from 'lucide-react';

const tabKeys = ['providers', 'clients', 'logs', 'settings'] as const;
type TabId = (typeof tabKeys)[number];

export const Header: React.FC = () => {
  const { t } = useTranslation();
  const {
    activeTab,
    settingsSection,
    setActiveTab,
    setSettingsSection,
    isProviderEditorOpen,
    openProviderEditor,
  } = useAppStore();

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
      className="relative h-12 w-full select-none shrink-0 z-40 bg-background/80 border-b border-border backdrop-blur-md flex items-center"
      style={{ WebkitAppRegion: 'drag' } as React.CSSProperties}
    >
      {/* 居中导航：统一采用整窗中轴绝对居中，与下方主内容卡片严格对齐 */}
      <div
        className="absolute left-1/2 -translate-x-1/2 top-1/2 -translate-y-1/2"
        style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
      >
        <Tabs value={current} onValueChange={selectTab}>
          <TabsList variant="ghost" aria-label={t('header.navAria')}>
            {tabKeys.map((id) => (
              <TabsTrigger key={id} value={id}>
                {t(`header.${id}`)}
              </TabsTrigger>
            ))}
          </TabsList>
        </Tabs>
      </div>

      {/* 右侧功能操作区：脱离常规文档流绝对定位，既不干扰导航居中对称性，切换 Tab 时导航也绝不跳动 */}
      {current === 'providers' && !isProviderEditorOpen && (
        <div
          className="absolute right-3.5 top-1/2 -translate-y-1/2 flex items-center"
          style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
        >
          <button
            type="button"
            onClick={() => openProviderEditor(null)}
            title={t('header.addProviderShortcut')}
            aria-label={t('header.addProvider')}
            className="p-1 rounded-md text-muted-foreground hover:text-foreground hover:bg-muted/80 transition-colors cursor-pointer"
          >
            <Plus className="w-4 h-4 stroke-[2.2]" />
          </button>
        </div>
      )}
    </header>
  );
};
