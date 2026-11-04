import React from 'react';
import { useAppStore } from '@/store/useAppStore';
import { useTranslation } from '@/i18n';
import { Tabs, TabsList, TabsTrigger } from '@/components/common/Tabs';

const tabKeys = ['providers', 'clients', 'logs', 'settings'] as const;
type TabId = (typeof tabKeys)[number];

export const Header: React.FC = () => {
  const { t } = useTranslation();
  const {
    activeTab,
    settingsSection,
    setActiveTab,
    setSettingsSection,
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
      {/* 方案 3：自然左对齐流式导航
          - 避让原生红绿灯区域（左侧留出恒定 96px 安全间距，距红绿灯约 18px 舒适留白）；
          - 中英文首个 Tab 严格对齐于相同 X 轴锚点，词条长短自然向右延伸；
          - 整个顶栏剩余区域均为原生窗口拖拽区，视觉纯净专注。
      */}
      <div
        className="flex items-center pl-[96px]"
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
    </header>
  );
};
