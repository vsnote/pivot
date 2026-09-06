import React, { useEffect } from 'react';
import { useAppStore } from '@/store/useAppStore';
import { Header } from '@/components/layout/Header';
import { TabsNav } from '@/components/layout/TabsNav';
import { ProviderList } from '@/components/providers/ProviderList';
import { ProviderModal } from '@/components/providers/ProviderModal';
import { PresetDrawer } from '@/components/providers/PresetDrawer';
import { RouteMatrix } from '@/components/routes/RouteMatrix';
import { RouteModal } from '@/components/routes/RouteModal';
import { InterceptorPanel } from '@/components/interceptor/InterceptorPanel';
import { RequestLogs } from '@/components/logs/RequestLogs';
import { StatsOverview } from '@/components/stats/StatsOverview';
import './App.css';

export const App: React.FC = () => {
  const { activeTab, fetchInitialData, initEventListeners, theme } = useAppStore();

  useEffect(() => {
    // Apply theme
    const isDark =
      theme === 'dark' ||
      (theme === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches);
    if (isDark) {
      document.documentElement.classList.add('dark');
    } else {
      document.documentElement.classList.remove('dark');
    }

    // Initial data load
    fetchInitialData();

    // Event listeners
    let cleanup: (() => void) | undefined;
    initEventListeners().then((c) => {
      cleanup = c;
    });

    return () => {
      if (cleanup) cleanup();
    };
  }, []);

  return (
    <div className="min-h-screen flex flex-col bg-gray-50 dark:bg-gray-950 text-gray-900 dark:text-gray-100 antialiased font-sans">
      {/* Top Application Bar */}
      <Header />

      {/* Navigation Tabs */}
      <TabsNav />

      {/* Main Tab Content */}
      <main className="flex-1 overflow-y-auto p-6 max-w-7xl w-full mx-auto">
        {activeTab === 'providers' && <ProviderList />}
        {activeTab === 'routes' && <RouteMatrix />}
        {activeTab === 'interceptor' && <InterceptorPanel />}
        {activeTab === 'logs' && <RequestLogs />}
        {activeTab === 'dashboard' && <StatsOverview />}
      </main>

      {/* Modals & Drawers */}
      <ProviderModal />
      <PresetDrawer />
      <RouteModal />
    </div>
  );
};

export default App;
