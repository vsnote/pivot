import React, { useEffect, useSyncExternalStore } from 'react';
import { applyAppTheme, useAppStore } from '@/store/useAppStore';
import { Header } from '@/components/layout/Header';
import { ClientsView, ProviderList } from '@/components/providers/ProviderList';
import { ProviderEditor } from '@/components/providers/ProviderEditor';
import { RequestLogs } from '@/components/logs/RequestLogs';
import { SettingsView } from '@/components/settings/SettingsView';
import { X, AlertCircle } from 'lucide-react';
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
  } = useAppStore();
  const systemIsDark = useSyncExternalStore(subscribeColorScheme, getSystemIsDark);

  useEffect(() => {
    applyAppTheme(theme, systemIsDark);
  }, [theme, systemIsDark]);

  useEffect(() => {
    fetchInitialData();
  }, []);

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
          className="mx-4 sm:mx-6 mt-3 flex items-center justify-between gap-3 rounded-lg border border-red-200 dark:border-red-900/60 bg-red-50/90 dark:bg-red-950/50 px-4 py-2.5 text-xs text-red-700 dark:text-red-300 shadow-xs animate-in fade-in"
        >
          <div className="flex items-center gap-2 min-w-0">
            <AlertCircle className="w-4 h-4 shrink-0 text-red-500 dark:text-red-400" />
            <span className="leading-relaxed break-all">{error}</span>
          </div>
          <button
            type="button"
            onClick={clearError}
            aria-label="关闭错误提示"
            title="关闭"
            className="shrink-0 rounded-md p-1 text-red-500 hover:text-red-700 dark:text-red-400 dark:hover:text-red-200 hover:bg-red-100 dark:hover:bg-red-900/40 cursor-pointer transition-colors"
          >
            <X className="w-3.5 h-3.5" />
          </button>
        </div>
      )}

      {/* 主视图内容区域：统一定宽与内边距 */}
      <main className="flex-1 min-h-0 w-full overflow-y-auto px-4 sm:px-6 py-4">
        <div className="max-w-5xl mx-auto w-full">
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
            <RequestLogs />
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
