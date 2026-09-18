import { create } from 'zustand';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import {
  ActiveTab,
  SettingsSection,
  GatewayStats,
  InterceptorStatus,
  Preset,
  Provider,
  RequestLog,
  RoutePlan,
  RouteRule,
} from '@/types';

interface AppStore {
  activeTab: ActiveTab;
  setActiveTab: (tab: ActiveTab) => void;

  settingsSection: SettingsSection;
  setSettingsSection: (section: SettingsSection) => void;

  theme: 'dark' | 'light' | 'system';
  setTheme: (theme: 'dark' | 'light' | 'system') => void;

  stats: GatewayStats;
  providers: Provider[];
  routes: RouteRule[];
  interceptors: InterceptorStatus[];
  presets: Preset[];
  logs: RequestLog[];

  isLoading: boolean;
  initialLoaded: boolean;
  error: string | null;
  clearError: () => void;

  // Modals
  isProviderEditorOpen: boolean;
  editingProvider: Provider | null;
  openProviderEditor: (p?: Provider | null) => void;
  closeProviderEditor: () => void;

  isRouteModalOpen: boolean;
  editingRoute: RouteRule | null;
  openRouteModal: (r?: RouteRule | null) => void;
  closeRouteModal: () => void;

  // Actions
  fetchInitialData: () => Promise<void>;
  refreshStats: () => Promise<void>;
  saveProvider: (p: Provider) => Promise<void>;
  deleteProvider: (id: string) => Promise<void>;
  testProviderLatency: (id: string) => Promise<number>;
  toggleProvider: (id: string, enabled: boolean) => Promise<void>;
  saveRoutes: (routes: RouteRule[]) => Promise<void>;
  previewRoute: (model: string) => Promise<RoutePlan>;
  toggleInterceptor: (id: string, enable: boolean) => Promise<void>;
  clearLogs: () => Promise<void>;
  initEventListeners: () => Promise<() => void>;
}

const SETTINGS_SECTIONS: SettingsSection[] = ['clients', 'routes', 'gateway', 'logs', 'about'];
const LEGACY_TABS: Partial<Record<ActiveTab, SettingsSection>> = {
  dashboard: 'gateway',
  routes: 'routes',
  interceptor: 'clients',
  logs: 'logs',
};

const isNativeRuntime = () =>
  typeof window !== 'undefined' && (isTauri() || window.location.protocol === 'tauri:');

const getInitialTheme = (): 'dark' | 'light' | 'system' => {
  if (typeof window === 'undefined') return 'system';
  const saved = localStorage.getItem('omni_theme');
  return saved === 'dark' || saved === 'light' ? saved : 'system';
};

export const applyAppTheme = (theme: 'dark' | 'light' | 'system') => {
  const isDark =
    theme === 'dark' ||
    (theme === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches);

  document.documentElement.classList.toggle('dark', isDark);
  document.documentElement.style.colorScheme = isDark ? 'dark' : 'light';

  if (isNativeRuntime()) {
    getCurrentWindow().setTheme(isDark ? 'dark' : 'light').catch(() => {});
  }
};

export const useAppStore = create<AppStore>((set, get) => ({
  activeTab: 'providers',
  setActiveTab: (tab) => {
    const section = LEGACY_TABS[tab] ?? get().settingsSection;
    const activeTab = tab === 'providers' ? 'providers' : 'settings';
    set({ activeTab, settingsSection: section });
    if (isNativeRuntime()) {
      invoke('sync_native_tab', { tab: activeTab === 'providers' ? 'providers' : section })
        .catch((error) => console.warn('无法同步原生导航', error));
    }
  },
  settingsSection: 'clients',
  setSettingsSection: (section) => {
    set({ activeTab: 'settings', settingsSection: section });
    if (isNativeRuntime()) {
      invoke('sync_native_tab', { tab: section })
        .catch((error) => console.warn('无法同步原生导航', error));
    }
  },
  theme: getInitialTheme(),
  setTheme: (theme) => {
    set({ theme });
    localStorage.setItem('omni_theme', theme);
  },

  stats: {
    isRunning: false,
    host: '127.0.0.1',
    port: 16888,
    activeRequests: 0,
    totalRequests: 0,
    totalPromptTokens: 0,
    totalCompletionTokens: 0,
    errorCount: 0,
    uptimeSecs: 0,
  },
  // 生产初始数据必须为空：演示数据曾复用 anthropic 等真实主键，
  // 加载完成前打开编辑页会导致占位密钥覆盖真实配置。
  providers: [],
  routes: [],
  interceptors: [],
  presets: [],
  logs: [],

  isLoading: false,
  initialLoaded: false,
  error: null,
  clearError: () => set({ error: null }),

  isProviderEditorOpen: false,
  editingProvider: null,
  openProviderEditor: (p = null) => {
    if (get().isProviderEditorOpen) {
      get().setActiveTab('providers');
      return;
    }
    // 初始数据未就绪时禁止打开编辑页，避免旧快照覆盖真实配置，
    // 也避免新增记录被随后到达的初始列表覆盖。
    if (!get().initialLoaded) return;
    get().setActiveTab('providers');
    set({ isProviderEditorOpen: true, editingProvider: p });
  },
  closeProviderEditor: () => set({ isProviderEditorOpen: false, editingProvider: null }),

  isRouteModalOpen: false,
  editingRoute: null,
  openRouteModal: (r = null) => set({ isRouteModalOpen: true, editingRoute: r }),
  closeRouteModal: () => set({ isRouteModalOpen: false, editingRoute: null }),

  fetchInitialData: async () => {
    set({ isLoading: true, error: null });
    try {
      const [stats, providers, routes, interceptors, presets, logs] = await Promise.all([
        invoke<GatewayStats>('get_gateway_status').catch(() => get().stats),
        invoke<Provider[]>('get_providers').catch(() => []),
        invoke<RouteRule[]>('get_routes').catch(() => []),
        invoke<InterceptorStatus[]>('get_interceptor_status').catch(() => []),
        invoke<Preset[]>('get_presets').catch(() => []),
        invoke<RequestLog[]>('get_logs', { limit: 100, offset: 0 }).catch(() => []),
      ]);

      set({
        stats,
        providers,
        routes,
        interceptors,
        presets,
        logs,
        isLoading: false,
        initialLoaded: true,
      });
      // 真实数据到达后，正在编辑的旧草稿可能已失效，强制关闭避免误保存。
      if (get().isProviderEditorOpen) {
        set({ isProviderEditorOpen: false, editingProvider: null });
      }
    } catch (err: unknown) {
      set({ error: String(err), isLoading: false });
    }
  },

  refreshStats: async () => {
    try {
      const stats = await invoke<GatewayStats>('get_gateway_status');
      set({ stats });
    } catch {
      // ignore
    }
  },

  saveProvider: async (provider: Provider) => {
    try {
      await invoke('save_provider', { provider });
      // A successful write must not be reported as failed if a subsequent list request fails.
      set((state) => ({ providers: [...state.providers
        // 后端保证默认服务商唯一，本地列表立即跟上，避免出现两个默认标记。
        .filter((p) => p.id !== provider.id)
        .map((p) => (provider.isDefault ? { ...p, isDefault: false } : p)), provider]
        .sort((a, b) => b.priority - a.priority || a.name.localeCompare(b.name)) }));
    } catch (err: unknown) {
      set({ error: String(err) });
      throw err;
    }
  },

  deleteProvider: async (id: string) => {
    try {
      // The backend reports which rules survive the deletion, so the page never
      // shows rules that point at a provider that is already gone.
      const routes = await invoke<RouteRule[]>('delete_provider', { id });
      set((state) => ({
        providers: state.providers.filter((p) => p.id !== id),
        routes,
        error: null,
      }));
    } catch (err: unknown) {
      set({ error: `删除服务商失败：${String(err)}` });
      throw err;
    }
  },

  testProviderLatency: async (id: string) => {
    try {
      const latencyMs = await invoke<number>('test_provider_latency', { id });
      set((state) => ({
        providers: state.providers.map((p) => (p.id === id ? { ...p, latencyMs } : p)),
      }));
      return latencyMs;
    } catch (err: unknown) {
      set((state) => ({
        providers: state.providers.map((p) => (p.id === id ? { ...p, latencyMs: -1 } : p)),
      }));
      throw err;
    }
  },

  toggleProvider: async (id: string, enabled: boolean) => {
    const provider = get().providers.find((p) => p.id === id);
    if (!provider) return;
    await invoke('set_provider_enabled', { id, enabled });
    set((state) => ({ providers: state.providers.map((p) => p.id === id ? { ...p, enabled } : p) }));
  },

  saveRoutes: async (routes: RouteRule[]) => {
    try {
      await invoke('save_routes', { routes });
      set({ routes, isRouteModalOpen: false, editingRoute: null });
    } catch (err: unknown) {
      set({ error: String(err) });
      throw err;
    }
  },

  // The gateway owns model resolution, so the advanced page asks it instead of
  // guessing in the browser.
  previewRoute: (model: string) => invoke<RoutePlan>('preview_route', { model }),

  toggleInterceptor: async (id: string, enable: boolean) => {
    try {
      const status = await invoke<InterceptorStatus>('toggle_interceptor', { id, enable });
      set((state) => ({
        interceptors: state.interceptors.map((i) => (i.id === id ? status : i)),
      }));
    } catch (err: unknown) {
      set({ error: String(err) });
      throw err;
    }
  },

  clearLogs: async () => {
    try {
      await invoke('clear_logs');
      set({ logs: [] });
    } catch (err: unknown) {
      set({ error: String(err) });
    }
  },

  initEventListeners: async () => {
    try {
      if (isNativeRuntime()) {
        const unlistenLog = await listen<RequestLog>('gw-log', (event) => {
          set((state) => ({
            logs: [event.payload, ...state.logs.slice(0, 499)],
            stats: {
              ...state.stats,
              totalRequests: state.stats.totalRequests + 1,
            },
          }));
        });

        // 监听 macOS 原生 NSSegmentedControl 切换事件
        const unlistenTab = await listen<string>('native-tab-selected', (event) => {
          const tabId = event.payload;
          if (tabId === 'providers') {
            set({ activeTab: 'providers' });
          } else if (SETTINGS_SECTIONS.includes(tabId as SettingsSection)) {
            set({ activeTab: 'settings', settingsSection: tabId as SettingsSection });
          }
        });

        const unlistenAddProvider = await listen('native-add-provider', () => {
          get().openProviderEditor(null);
        });

        const unlistenTheme = await listen('native-theme-toggle', () => {
          get().setTheme(document.documentElement.classList.contains('dark') ? 'light' : 'dark');
        });

        // Also set interval to refresh stats every 3 seconds
        const timer = setInterval(() => {
          get().refreshStats();
        }, 3000);

        return () => {
          unlistenLog();
          unlistenTab();
          unlistenAddProvider();
          unlistenTheme();
          clearInterval(timer);
        };
      }
    } catch {
      // ignore
    }
    return () => {};
  },
}));
