import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  ActiveTab,
  GatewayStats,
  InterceptorStatus,
  Preset,
  Provider,
  RequestLog,
  RouteRule,
} from '@/types';

interface AppStore {
  activeTab: ActiveTab;
  setActiveTab: (tab: ActiveTab) => void;

  theme: 'dark' | 'light' | 'system';
  setTheme: (theme: 'dark' | 'light' | 'system') => void;

  stats: GatewayStats;
  providers: Provider[];
  routes: RouteRule[];
  interceptors: InterceptorStatus[];
  presets: Preset[];
  logs: RequestLog[];

  isLoading: boolean;
  error: string | null;

  // Modals
  isProviderModalOpen: boolean;
  editingProvider: Provider | null;
  openProviderModal: (p?: Provider | null) => void;
  closeProviderModal: () => void;

  isPresetDrawerOpen: boolean;
  openPresetDrawer: () => void;
  closePresetDrawer: () => void;

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
  toggleInterceptor: (id: string, enable: boolean) => Promise<void>;
  importPreset: (presetId: string) => Promise<void>;
  clearLogs: () => Promise<void>;
  initEventListeners: () => Promise<() => void>;
}

export const useAppStore = create<AppStore>((set, get) => ({
  activeTab: 'providers',
  setActiveTab: (tab) => set({ activeTab: tab }),

  theme: 'dark',
  setTheme: (theme) => {
    set({ theme });
    const isDark =
      theme === 'dark' ||
      (theme === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches);
    if (isDark) {
      document.documentElement.classList.add('dark');
    } else {
      document.documentElement.classList.remove('dark');
    }
  },

  stats: {
    isRunning: true,
    host: '127.0.0.1',
    port: 16888,
    activeRequests: 0,
    totalRequests: 0,
    totalPromptTokens: 0,
    totalCompletionTokens: 0,
    errorCount: 0,
    uptimeSecs: 0,
  },
  providers: [],
  routes: [],
  interceptors: [],
  presets: [],
  logs: [],

  isLoading: false,
  error: null,

  isProviderModalOpen: false,
  editingProvider: null,
  openProviderModal: (p = null) => set({ isProviderModalOpen: true, editingProvider: p }),
  closeProviderModal: () => set({ isProviderModalOpen: false, editingProvider: null }),

  isPresetDrawerOpen: false,
  openPresetDrawer: () => set({ isPresetDrawerOpen: true }),
  closePresetDrawer: () => set({ isPresetDrawerOpen: false }),

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
      });
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
      const providers = await invoke<Provider[]>('get_providers');
      set({ providers, isProviderModalOpen: false, editingProvider: null });
    } catch (err: unknown) {
      set({ error: String(err) });
      throw err;
    }
  },

  deleteProvider: async (id: string) => {
    try {
      await invoke('delete_provider', { id });
      set((state) => ({
        providers: state.providers.filter((p) => p.id !== id),
      }));
    } catch (err: unknown) {
      set({ error: String(err) });
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
    const updated = { ...provider, enabled };
    await get().saveProvider(updated);
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

  importPreset: async (presetId: string) => {
    try {
      const provider = await invoke<Provider>('import_preset', { presetId });
      set((state) => {
        const existingIdx = state.providers.findIndex((p) => p.id === provider.id);
        if (existingIdx >= 0) {
          const updated = [...state.providers];
          updated[existingIdx] = provider;
          return { providers: updated };
        }
        return { providers: [provider, ...state.providers] };
      });
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
    const unlisten = await listen<RequestLog>('gw-log', (event) => {
      set((state) => ({
        logs: [event.payload, ...state.logs.slice(0, 499)],
        stats: {
          ...state.stats,
          totalRequests: state.stats.totalRequests + 1,
        },
      }));
    });

    // Also set interval to refresh stats every 3 seconds
    const timer = setInterval(() => {
      get().refreshStats();
    }, 3000);

    return () => {
      unlisten();
      clearInterval(timer);
    };
  },
}));
