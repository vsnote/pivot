import React from 'react';
import { useAppStore } from '@/store/useAppStore';
import { useUpdateStore } from '@/store/useUpdateStore';
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
    isProviderEditorOpen,
  } = useAppStore();
  const hasUpdate = useUpdateStore((state) => state.hasUpdate);

  const current: TabId =
    activeTab === 'clients'
      ? 'clients'
      : activeTab === 'logs' || (activeTab === 'settings' && settingsSection === 'logs')
        ? 'logs'
        : activeTab === 'settings'
          ? 'settings'
          : 'providers';

  // 记录点击按下前的激活 Tab，防止 Radix onMouseDown 切换后导致 onClick 误判为「同 Tab 二次点击」
  const previousTabOnPointerDown = React.useRef<TabId>(current);

  const selectTab = (id: string) => {
    if (id === 'settings') {
      setSettingsSection('about');
      return;
    }
    setActiveTab(id as TabId);
  };

  const handlePointerDown = () => {
    previousTabOnPointerDown.current = current;
  };

  const handleTabClick = (id: TabId) => {
    // 只有在按下发生前本身就已经处于「服务商」且正在编辑时，才视为「再次点击已激活 Tab 退出编辑」
    if (
      id === 'providers' &&
      previousTabOnPointerDown.current === 'providers' &&
      isProviderEditorOpen
    ) {
      window.dispatchEvent(new CustomEvent('pivot:request-close-provider-editor'));
    }
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
              <TabsTrigger
                key={id}
                value={id}
                onPointerDown={handlePointerDown}
                onMouseDown={handlePointerDown}
                onClick={() => handleTabClick(id)}
              >
                <span className="inline-flex items-center">
                  <span>{t(`header.${id}`)}</span>
                  {id === 'settings' && hasUpdate && (
                    <span className="ml-1.5 flex h-2 w-2 relative" title={t('settings.newVersionFound', { version: '' })}>
                      <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-blue-400 opacity-75" />
                      <span className="relative inline-flex rounded-full h-2 w-2 bg-blue-500" />
                    </span>
                  )}
                </span>
              </TabsTrigger>
            ))}
          </TabsList>
        </Tabs>
      </div>
    </header>
  );
};
