import React, { useEffect, useSyncExternalStore } from 'react';
import { applyAppTheme, useAppStore } from '@/store/useAppStore';
import { useTranslation } from '@/i18n';
import { Header } from '@/components/layout/Header';
import { ClientsView, ProviderList } from '@/components/providers/ProviderList';
import { ProviderEditor } from '@/components/providers/ProviderEditor';
import { UsageDashboard } from '@/components/dashboard/UsageDashboard';
import { SettingsView } from '@/components/settings/SettingsView';
import { X, AlertCircle } from 'lucide-react';
import { loadModelRates } from '@/utils/pricing';
import './App.css';

const subscribeColorScheme = (onChange: () => void) => {
  if (typeof window === 'undefined' || !window.matchMedia) return () => {};
  const mql = window.matchMedia('(prefers-color-scheme: dark)');
  const listener = () => onChange();

  if (mql.addEventListener) {
    mql.addEventListener('change', listener);
  } else if (mql.addListener) {
    mql.addListener(listener);
  }

  return () => {
    if (mql.removeEventListener) {
      mql.removeEventListener('change', listener);
    } else if (mql.removeListener) {
      mql.removeListener(listener);
    }
  };
};

const getSystemIsDark = () =>
  typeof window !== 'undefined' && Boolean(window.matchMedia?.('(prefers-color-scheme: dark)').matches);

export const App: React.FC = () => {
  const {
    activeTab,
    settingsSection,
    fetchInitialData,
    theme,
    isProviderEditorOpen,
    error,
    clearError,
    setActiveTab,
    setSettingsSection,
    openProviderEditor,
  } = useAppStore();
  const { t } = useTranslation();
  const systemIsDark = useSyncExternalStore(subscribeColorScheme, getSystemIsDark);

  useEffect(() => {
    applyAppTheme(theme, systemIsDark);
  }, [theme, systemIsDark]);

  useEffect(() => {
    fetchInitialData();
    loadModelRates();
  }, []);

  // 全局快捷键：⌘1-4 切换主要页面，⌘N 快速打开新增服务商
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target;
      if (
        target instanceof Element &&
        target.closest('input, textarea, [contenteditable="true"], select')
      ) {
        return;
      }

      const isMac = navigator.platform.toUpperCase().includes('MAC');
      const modKey = isMac ? event.metaKey : event.ctrlKey;

      if (modKey && !event.shiftKey && !event.altKey) {
        if (event.key === '1') {
          event.preventDefault();
          setActiveTab('providers');
        } else if (event.key === '2') {
          event.preventDefault();
          setActiveTab('clients');
        } else if (event.key === '3') {
          event.preventDefault();
          setActiveTab('logs');
        } else if (event.key === '4') {
          event.preventDefault();
          setSettingsSection('about');
          setActiveTab('settings');
        } else if (event.key.toLowerCase() === 'n') {
          event.preventDefault();
          if (activeTab !== 'providers') {
            setActiveTab('providers');
          }
          openProviderEditor(null);
        }
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [activeTab, setActiveTab, setSettingsSection, openProviderEditor]);

  useEffect(() => {
    const onContextMenu = (event: MouseEvent) => {
      const target = event.target;
      if (target instanceof Element && target.closest('input, textarea, [contenteditable="true"]')) {
        return;
      }
      event.preventDefault();
    };
    document.addEventListener('contextmenu', onContextMenu);
    return () => document.removeEventListener('contextmenu', onContextMenu);
  }, []);

  return (
    <div className="h-screen w-screen flex flex-col overflow-hidden bg-background text-foreground antialiased font-sans select-none">
      {/* 顶部现代化原生标题栏与导航 */}
      <Header />

      {/* 操作失败提示横幅 */}
      {error && (
        <div
          role="alert"
          className="mx-4 mt-3 flex items-center justify-between gap-3 rounded-lg border border-red-200 dark:border-red-900/60 bg-red-50/90 dark:bg-red-950/50 px-4 py-2.5 text-xs text-red-700 dark:text-red-300 shadow-xs animate-in fade-in"
        >
          <div className="flex items-center gap-2 min-w-0">
            <AlertCircle className="w-4 h-4 shrink-0 text-red-500 dark:text-red-400" />
            <span className="leading-relaxed break-all">{error}</span>
          </div>
          <button
            type="button"
            onClick={clearError}
            aria-label={t('common.close')}
            title={t('common.close')}
            className="shrink-0 rounded-md p-1 text-red-500 hover:text-red-700 dark:text-red-400 dark:hover:text-red-200 hover:bg-red-100 dark:hover:bg-red-900/40 cursor-pointer transition-colors"
          >
            <X className="w-3.5 h-3.5" />
          </button>
        </div>
      )}

      {/* 主视图内容区域：统一定宽与内边距 */}
      <main className="flex-1 min-h-0 w-full overflow-y-auto no-scrollbar px-4 py-4">
        <div
          className={`mx-auto w-full transition-[max-width] duration-150 ${
            activeTab === 'logs' || (activeTab === 'settings' && settingsSection === 'logs')
              ? 'max-w-6xl'
              : 'max-w-5xl'
          }`}
        >
          <div className={activeTab === 'providers' ? 'block' : 'hidden'}>
            {isProviderEditorOpen ? <ProviderEditor /> : <ProviderList />}
          </div>
          <div className={activeTab === 'clients' ? 'block' : 'hidden'}>
            <ClientsView />
          </div>
          <div
            className={
              activeTab === 'logs' || (activeTab === 'settings' && settingsSection === 'logs')
                ? 'block'
                : 'hidden'
            }
          >
            <UsageDashboard />
          </div>
          <div className={activeTab === 'settings' && settingsSection !== 'logs' ? 'block' : 'hidden'}>
            <SettingsView />
          </div>
        </div>
      </main>
    </div>
  );
};

export default App;
