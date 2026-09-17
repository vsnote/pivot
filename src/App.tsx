import React, { useEffect, useSyncExternalStore } from 'react';
import { applyAppTheme, useAppStore } from '@/store/useAppStore';
import { Header } from '@/components/layout/Header';
import { ProviderList } from '@/components/providers/ProviderList';
import { ProviderEditor } from '@/components/providers/ProviderEditor';
import { RouteModal } from '@/components/routes/RouteModal';
import { SettingsView } from '@/components/settings/SettingsView';
import './App.css';

const colorScheme = window.matchMedia('(prefers-color-scheme: dark)');
const subscribeColorScheme = (onChange: () => void) => {
  colorScheme.addEventListener('change', onChange);
  return () => colorScheme.removeEventListener('change', onChange);
};
const getSystemIsDark = () => colorScheme.matches;

export const App: React.FC = () => {
  const { activeTab, fetchInitialData, theme, isProviderEditorOpen } = useAppStore();
  const systemIsDark = useSyncExternalStore(subscribeColorScheme, getSystemIsDark);
  const isDark = theme === 'dark' || (theme === 'system' && systemIsDark);

  useEffect(() => {
    applyAppTheme(isDark ? 'dark' : 'light');
  }, [isDark]);

  useEffect(() => {
    // Initial data load
    fetchInitialData();
  }, []);

  return (
    <div className="h-screen w-screen flex flex-col overflow-hidden bg-[#f5f6f8] dark:bg-[#131416] text-gray-900 dark:text-gray-100 antialiased font-sans select-none">
      {/* 顶部优雅原生标题栏 */}
      <Header />

      {/* 主视图内容区域 */}
      {isProviderEditorOpen && (
        <div className={activeTab === 'providers' ? 'flex flex-1 min-h-0' : 'hidden'}>
          <ProviderEditor />
        </div>
      )}
      <main className={isProviderEditorOpen && activeTab === 'providers' ? 'hidden' : 'flex-1 min-h-0 w-full overflow-y-auto px-6 sm:px-8 pt-1 sm:pt-1.5 pb-6'}>
        {activeTab === 'providers' ? <ProviderList /> : <SettingsView />}
      </main>

      {/* 弹窗与抽屉 */}
      <RouteModal />
    </div>
  );
};

export default App;
