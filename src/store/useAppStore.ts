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
  RouteExplanation,
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

  // Actions
  fetchInitialData: () => Promise<void>;
  refreshStats: () => Promise<void>;
  fetchInterceptors: () => Promise<void>;
  saveProvider: (p: Provider) => Promise<void>;
  reorderProviders: (orderedIds: string[]) => Promise<void>;
  deleteProvider: (id: string) => Promise<void>;
  testProviderLatency: (id: string) => Promise<number>;
  clearProviderLatencies: () => void;
  toggleProvider: (id: string, enabled: boolean) => Promise<void>;
  explainRoute: (model: string) => Promise<RouteExplanation>;
  toggleInterceptor: (id: string, enable: boolean) => Promise<void>;
  setInterceptorModel: (id: string, model: string | null) => Promise<void>;
  importThirdPartyProvider: (id: string) => Promise<void>;
  clearLogs: () => Promise<void>;
  initEventListeners: () => Promise<() => void>;
}

const SETTINGS_SECTIONS: SettingsSection[] = ['clients', 'gateway', 'logs', 'about'];

// 连续拖拽时只认最后一次请求，避免先发后到的响应覆盖新顺序
let reorderSeq = 0;
const LEGACY_TABS: Partial<Record<ActiveTab, SettingsSection>> = {
  dashboard: 'gateway',
  interceptor: 'clients',
  logs: 'logs',
};

const isNativeRuntime = () =>
  typeof window !== 'undefined' && (isTauri() || window.location.protocol === 'tauri:');

const getInitialTheme = (): 'dark' | 'light' | 'system' => {
  if (typeof window === 'undefined') return 'system';
  const saved = localStorage.getItem('pivot_theme') || localStorage.getItem('omni_theme');
  return saved === 'dark' || saved === 'light' ? saved : 'system';
};

export const applyAppTheme = (theme: 'dark' | 'light' | 'system', systemIsDark?: boolean) => {
  if (typeof window === 'undefined') return;

  const resolvedSystemIsDark =
    systemIsDark ?? Boolean(window.matchMedia?.('(prefers-color-scheme: dark)').matches);

  const isDark = theme === 'dark' || (theme === 'system' && resolvedSystemIsDark);

  if (typeof document !== 'undefined') {
    document.documentElement.classList.toggle('dark', isDark);
    document.documentElement.style.colorScheme = isDark ? 'dark' : 'light';
  }

  if (isNativeRuntime()) {
    getCurrentWindow()
      .setTheme(theme === 'system' ? null : theme)
      .catch(() => {});
  }
};

export const useAppStore = create<AppStore>((set, get) => ({
  activeTab: 'providers',
  setActiveTab: (tab) => {
    const section = LEGACY_TABS[tab] ?? (tab === 'logs' ? 'logs' : get().settingsSection);
    const isTopLevel =
      tab === 'providers' || tab === 'clients' || tab === 'logs';
    const activeTab = isTopLevel ? tab : 'settings';
    if (get().activeTab !== activeTab) {
      get().clearProviderLatencies();
    }
    set({ activeTab, settingsSection: section });
    if (isNativeRuntime()) {
      invoke('sync_native_tab', {
        tab: activeTab === 'settings' ? section : activeTab,
      }).catch((error) => console.warn('无法同步原生导航', error));
    }
  },
  settingsSection: 'about',
  setSettingsSection: (section) => {
    if (get().activeTab !== 'settings') {
      get().clearProviderLatencies();
    }
    set({ activeTab: 'settings', settingsSection: section });
    if (isNativeRuntime()) {
      invoke('sync_native_tab', { tab: section })
        .catch((error) => console.warn('无法同步原生导航', error));
    }
  },
  theme: getInitialTheme(),
  setTheme: (theme) => {
    set({ theme });
    localStorage.setItem('pivot_theme', theme);
    applyAppTheme(theme);
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
    get().clearProviderLatencies();
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
  closeProviderEditor: () => {
    get().clearProviderLatencies();
    set({ isProviderEditorOpen: false, editingProvider: null });
  },

  fetchInitialData: async () => {
    set({ isLoading: true, error: null });
    try {
      const [stats, providers, interceptors, presets, logs] = await Promise.all([
        invoke<GatewayStats>('get_gateway_status').catch(() => get().stats),
        invoke<Provider[]>('get_providers').catch(() => []),
        invoke<InterceptorStatus[]>('get_interceptor_status').catch(() => []),
        invoke<Preset[]>('get_presets').catch(() => []),
        invoke<RequestLog[]>('get_logs', { limit: 500, offset: 0 }).catch(() => []),
      ]);

      set({
        stats,
        providers: providers.map((p) => ({ ...p, latencyMs: undefined })),
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

  fetchInterceptors: async () => {
    try {
      const interceptors = await invoke<InterceptorStatus[]>('get_interceptor_status');
      set({ interceptors });
    } catch {
      // ignore
    }
  },

  refreshStats: async () => {
    try {
      const [stats, interceptors] = await Promise.all([
        invoke<GatewayStats>('get_gateway_status').catch(() => get().stats),
        invoke<InterceptorStatus[]>('get_interceptor_status').catch(() => get().interceptors),
      ]);
      set({ stats, interceptors });
    } catch {
      // ignore
    }
  },

  saveProvider: async (provider: Provider) => {
    try {
      await invoke('save_provider', { provider });
      // 原地更新，保持当前列表顺序：编辑服务商不应让它在列表里跳动。
      // 列表顺序只在加载时由后端决定，新建的服务商追加到末尾。
      set((state) => {
        const exists = state.providers.some((p) => p.id === provider.id);
        const next = exists
          ? state.providers.map((p) => (p.id === provider.id ? provider : p))
          : [...state.providers, provider];
        // 后端保证默认服务商与默认模型唯一，本地列表立即跟上，避免出现多个默认标记。
        const isClaimingDefault = !!provider.isDefault;
        const hasDefaultModel = Boolean(provider.defaultModel);
        return {
          providers: next.map((p) => {
            if (p.id === provider.id) return provider;
            return {
              ...p,
              ...(isClaimingDefault ? { isDefault: false } : {}),
              ...(hasDefaultModel ? { defaultModel: null } : {}),
            };
          }),
        };
      });
    } catch (err: unknown) {
      set({ error: String(err) });
      throw err;
    }
  },

  reorderProviders: async (orderedIds: string[]) => {
    const seq = ++reorderSeq;
    const previous = get().providers;
    // 先按释放顺序就地更新，避免等待后端时列表跳回旧顺序
    const byId = new Map(previous.map((provider) => [provider.id, provider] as const));
    set({
      providers: orderedIds
        .map((id) => byId.get(id))
        .filter((provider): provider is Provider => Boolean(provider)),
    });
    try {
      // 列表顺序即路由优先级，后端重写 priority 后返回权威顺序
      const providers = await invoke<Provider[]>('reorder_providers', { ids: orderedIds });
      if (seq !== reorderSeq) return;
      set({ providers, error: null });
    } catch (err: unknown) {
      if (seq !== reorderSeq) return;
      set({ providers: previous, error: `调整顺序失败：${String(err)}` });
      throw err;
    }
  },

  deleteProvider: async (id: string) => {
    try {
      await invoke('delete_provider', { id });
      set((state) => ({
        providers: state.providers.filter((p) => p.id !== id),
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

  clearProviderLatencies: () => {
    set((state) => ({
      providers: state.providers.map((p) =>
        p.latencyMs !== undefined ? { ...p, latencyMs: undefined } : p,
      ),
    }));
  },

  toggleProvider: async (id: string, enabled: boolean) => {
    const provider = get().providers.find((p) => p.id === id);
    if (!provider) return;
    await invoke('set_provider_enabled', { id, enabled });
    set((state) => ({ providers: state.providers.map((p) => p.id === id ? { ...p, enabled } : p) }));
  },

  // The gateway owns model resolution, so the routing page asks it instead of
  // guessing in the browser.
  explainRoute: (model: string) => invoke<RouteExplanation>('explain_route', { model }),

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

  setInterceptorModel: async (id: string, model: string | null) => {
    try {
      const status = await invoke<InterceptorStatus>('set_interceptor_model', { id, model });
      set((state) => ({
        interceptors: state.interceptors.map((i) => (i.id === id ? status : i)),
      }));
    } catch (err: unknown) {
      set({ error: String(err) });
      throw err;
    }
  },

  importThirdPartyProvider: async (id: string) => {
    try {
      const [provider, status] = await invoke<[Provider, InterceptorStatus]>('import_third_party_provider', { id });
      set((state) => {
        const exists = state.providers.some((p) => p.id === provider.id);
        const newProviders = exists
          ? state.providers.map((p) => (p.id === provider.id ? provider : p))
          : [provider, ...state.providers];
        return {
          providers: newProviders,
          interceptors: state.interceptors.map((i) => (i.id === id ? status : i)),
        };
      });
    } catch (err: unknown) {
      set({ error: String(err) });
      throw err;
    }
  },

  clearLogs: async () => {
    try {
      await invoke('clear_logs');
      set((state) => ({
        logs: [],
        stats: {
          ...state.stats,
          totalRequests: 0,
          totalPromptTokens: 0,
          totalCompletionTokens: 0,
          errorCount: 0,
        },
      }));
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
          if (tabId === 'providers' || tabId === 'clients') {
            set({ activeTab: tabId });
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

        let unlistenThemeChanged: (() => void) | undefined;
        try {
          unlistenThemeChanged = await getCurrentWindow().onThemeChanged(() => {
            if (get().theme === 'system') {
              applyAppTheme('system');
            }
          });
        } catch {
          // ignore
        }

        // Also set interval to refresh stats every 3 seconds
        const timer = setInterval(() => {
          get().refreshStats();
        }, 3000);

        return () => {
          unlistenLog();
          unlistenTab();
          unlistenAddProvider();
          unlistenTheme();
          unlistenThemeChanged?.();
          clearInterval(timer);
        };
      }
    } catch {
      // ignore
    }
    return () => {};
  },
}));
