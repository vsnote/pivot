import React from 'react';
import { useAppStore } from '@/store/useAppStore';
import { ActiveTab } from '@/types';
import {
  Server,
  Network,
  Cpu,
  FileText,
  BarChart3,
} from 'lucide-react';

interface TabItem {
  id: ActiveTab;
  label: string;
  icon: React.ReactNode;
  badge?: number | string;
}

export const TabsNav: React.FC = () => {
  const { activeTab, setActiveTab, providers, routes, interceptors, logs } = useAppStore();

  const activeProvidersCount = providers.filter((p) => p.enabled).length;
  const interceptedCount = interceptors.filter((i) => i.isIntercepted).length;

  const tabs: TabItem[] = [
    {
      id: 'providers',
      label: 'Providers',
      icon: <Server className="w-4 h-4" />,
      badge: `${activeProvidersCount}/${providers.length}`,
    },
    {
      id: 'routes',
      label: 'Routing Matrix',
      icon: <Network className="w-4 h-4" />,
      badge: routes.length,
    },
    {
      id: 'interceptor',
      label: 'App Interceptor',
      icon: <Cpu className="w-4 h-4" />,
      badge: interceptedCount > 0 ? `${interceptedCount} ON` : undefined,
    },
    {
      id: 'logs',
      label: 'Request Logs',
      icon: <FileText className="w-4 h-4" />,
      badge: logs.length > 0 ? logs.length : undefined,
    },
    {
      id: 'dashboard',
      label: 'Stats & Monitor',
      icon: <BarChart3 className="w-4 h-4" />,
    },
  ];

  return (
    <div className="border-b border-gray-200 dark:border-gray-800 bg-gray-50/50 dark:bg-gray-900/50 px-6">
      <nav className="flex space-x-1 sm:space-x-4">
        {tabs.map((tab) => {
          const isActive = activeTab === tab.id;
          return (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              className={`flex items-center gap-2 py-3 px-3 text-sm font-medium border-b-2 transition-all select-none ${
                isActive
                  ? 'border-indigo-600 text-indigo-600 dark:text-indigo-400 dark:border-indigo-400 font-semibold'
                  : 'border-transparent text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 hover:border-gray-300 dark:hover:border-gray-700'
              }`}
            >
              {tab.icon}
              <span>{tab.label}</span>
              {tab.badge !== undefined && (
                <span
                  className={`text-[11px] font-mono px-1.5 py-0.2 rounded-full ${
                    isActive
                      ? 'bg-indigo-100 dark:bg-indigo-950/80 text-indigo-700 dark:text-indigo-300'
                      : 'bg-gray-200/80 dark:bg-gray-800 text-gray-600 dark:text-gray-400'
                  }`}
                >
                  {tab.badge}
                </span>
              )}
            </button>
          );
        })}
      </nav>
    </div>
  );
};
