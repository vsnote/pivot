import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { test } from 'node:test';
import ts from 'typescript';

const source = await readFile(new URL('../src/store/useAppStore.ts', import.meta.url), 'utf8');
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
let instance = 0;

async function createStore({ native = true, invoke = async () => undefined, listen = async () => () => {}, onSetTheme } = {}) {
  const key = `omni-store-test-${instance++}`;
  globalThis[key] = { native, invoke, listen, onSetTheme };
  globalThis.window = { location: { protocol: 'http:' } };
  globalThis.localStorage = { getItem: () => null, setItem: () => {} };
  const tauriMock = `data:text/javascript,${encodeURIComponent(`
    const mock = globalThis[${JSON.stringify(key)}];
    export const isTauri = () => mock.native;
    export const invoke = (...args) => mock.invoke(...args);
    export const listen = (...args) => mock.listen(...args);
    export const getCurrentWindow = () => ({
      setTheme: async (theme) => {
        if (mock.onSetTheme) mock.onSetTheme(theme);
      },
      onThemeChanged: async () => () => {},
    });
  `)}`;
  const moduleSource = compiled
    .replace(/from ['"]zustand['"]/g, `from ${JSON.stringify(import.meta.resolve('zustand'))}`)
    .replace(/from ['"]@tauri-apps\/api\/(?:core|event|window)['"]/g, `from ${JSON.stringify(tauriMock)}`);
  const { useAppStore, applyAppTheme } = await import(`data:text/javascript,${encodeURIComponent(moduleSource)}`);
  useAppStore.applyAppTheme = applyAppTheme;
  return useAppStore;
}

test('旧导航入口映射到设置页，并保持原生导航同步', async () => {
  const calls = [];
  const store = await createStore({ invoke: async (...args) => { calls.push(args); } });
  store.getState().setActiveTab('dashboard');
  assert.equal(store.getState().activeTab, 'settings');
  assert.equal(store.getState().settingsSection, 'gateway');
  store.getState().setActiveTab('providers');
  assert.equal(store.getState().activeTab, 'providers');
  store.getState().setSettingsSection('about');
  assert.equal(store.getState().activeTab, 'settings');
  assert.deepEqual(calls, [
    ['sync_native_tab', { tab: 'gateway' }],
    ['sync_native_tab', { tab: 'providers' }],
    ['sync_native_tab', { tab: 'about' }],
  ]);
});

test('打开服务商编辑页统一切回列表，重复点击新增不覆盖正在编辑的草稿', async () => {
  const provider = { id: 'provider-a', name: 'Provider A', priority: 1 };
  const store = await createStore({ invoke: async (command) => command === 'get_providers' ? [provider] : [] });
  await store.getState().fetchInitialData();
  store.getState().setActiveTab('clients');
  store.getState().openProviderEditor(provider);
  assert.equal(store.getState().activeTab, 'providers');
  assert.equal(store.getState().editingProvider.id, provider.id);
  store.getState().openProviderEditor();
  assert.equal(store.getState().editingProvider.id, provider.id);
  store.getState().setActiveTab('logs');
  assert.equal(store.getState().isProviderEditorOpen, true);
});

test('列表开关只修改启用状态，不重新保存整个服务商或关闭编辑页', async () => {
  const calls = [];
  const provider = { id: 'provider-a', name: 'Provider A', enabled: true, priority: 1 };
  const store = await createStore({ native: false, invoke: async (...args) => {
    calls.push(args);
    if (args[0] === 'get_providers') return [provider];
    return undefined;
  } });
  await store.getState().fetchInitialData();
  calls.length = 0;
  store.getState().openProviderEditor(provider);
  await store.getState().toggleProvider(provider.id, false);
  assert.deepEqual(calls, [['set_provider_enabled', { id: provider.id, enabled: false }]]);
  assert.equal(store.getState().isProviderEditorOpen, true);
  assert.equal(store.getState().providers[0].enabled, false);
});

test('保存成功无需再次获取列表，列表刷新失败不会变成重复保存', async () => {
  const store = await createStore({ invoke: async (command) => {
    if (command === 'get_providers') throw new Error('list failed');
  } });
  const provider = { ...store.getState().providers[0], name: 'Saved' };
  await store.getState().saveProvider(provider);
  assert.equal(store.getState().providers.find((p) => p.id === provider.id).name, 'Saved');
});

test('初始数据未就绪时不能打开编辑页，真实数据到达后旧草稿关闭且真实配置不被覆盖', async () => {
  let resolveProviders;
  const providersPromise = new Promise((resolve) => { resolveProviders = resolve; });
  const calls = [];
  const store = await createStore({ invoke: async (command, args) => {
    calls.push([command, args]);
    if (command === 'get_providers') return providersPromise;
    if (command === 'get_interceptor_status') return [];
    if (command === 'get_presets') return [];
    if (command === 'get_logs') return [];
    if (command === 'get_gateway_status') return store.getState().stats;
  } });
  store.getState().openProviderEditor({ id: 'anthropic' });
  store.getState().openProviderEditor();
  assert.equal(store.getState().isProviderEditorOpen, false);
  const load = store.getState().fetchInitialData();
  assert.equal(store.getState().isLoading, true);
  resolveProviders([{ id: 'anthropic', name: 'Real', apiKeys: ['real-key'] }]);
  await load;
  assert.equal(store.getState().initialLoaded, true);
  assert.equal(store.getState().providers[0].apiKeys[0], 'real-key');
  assert.equal(store.getState().isProviderEditorOpen, false);
  assert.ok(!calls.some(([command]) => command === 'save_provider'));
});

test('路由试算使用网关照常解析，判定过程与计划原样返回', async () => {
  const calls = [];
  const explanation = {
    incoming: 'claude-3-7-sonnet',
    steps: [{ layer: 'exact', matched: true, detail: '精确模型匹配' }],
    plan: {
      primary: { providerId: 'antigravity', model: 'gemini-3.8-flash-high' },
      fallbacks: [],
    },
    error: null,
  };
  const store = await createStore({ invoke: async (...args) => {
    calls.push(args);
    if (args[0] === 'explain_route') return explanation;
  } });
  assert.deepEqual(await store.getState().explainRoute('claude-3-7-sonnet'), explanation);
  assert.deepEqual(calls, [['explain_route', { model: 'claude-3-7-sonnet' }]]);
});

test('日志事件保留每次尝试，尝试链在前端不被裁剪', async () => {
  let onLog;
  const store = await createStore({ listen: async (event, handler) => {
    if (event === 'gw-log') onLog = handler;
    return () => {};
  } });
  const stop = await store.getState().initEventListeners();
  onLog({ payload: { id: 'req-1', statusCode: 200, fallbackUsed: true, attempts: [
    { providerId: 'flaky', providerName: 'Flaky', model: 'gemini-3.8-flash-high', status: 503, error: 'not running', latencyMs: 8 },
    { providerId: 'antigravity', providerName: 'Antigravity', model: 'gemini-3.8-flash-high', status: 200, error: null, latencyMs: 120 },
  ] } });
  assert.equal(store.getState().logs[0].attempts.length, 2);
  assert.equal(store.getState().logs[0].attempts[0].status, 503);
  assert.equal(store.getState().logs[0].attempts[1].providerName, 'Antigravity');
  stop();
});

test('删除服务商后列表移除它', async () => {
  const calls = [];
  const provider = { id: 'provider-a', name: 'Provider A', enabled: true, priority: 1 };
  const store = await createStore({ native: false, invoke: async (command, args) => {
    calls.push([command, args]);
    if (command === 'get_providers') return [provider];
    if (command === 'delete_provider') return undefined;
    return undefined;
  } });
  await store.getState().fetchInitialData();
  calls.length = 0;
  await store.getState().deleteProvider(provider.id);
  assert.deepEqual(calls, [['delete_provider', { id: provider.id }]]);
  assert.deepEqual(store.getState().providers, []);
  assert.equal(store.getState().error, null);
});

test('删除失败会把原因写进全局错误并且不静默改列表', async () => {
  const provider = { id: 'provider-a', name: 'Provider A', enabled: true, priority: 1 };
  const store = await createStore({ native: false, invoke: async (command) => {
    if (command === 'get_providers') return [provider];
    if (command === 'delete_provider') throw new Error('服务商 provider-a 不存在或已被删除');
  } });
  await store.getState().fetchInitialData();
  await assert.rejects(() => store.getState().deleteProvider(provider.id));
  assert.match(store.getState().error, /删除服务商失败/);
  assert.equal(store.getState().providers.length, 1);
  store.getState().clearError();
  assert.equal(store.getState().error, null);
});

test('保存默认服务商后本地列表只有一个默认标记', async () => {
  const first = { id: 'provider-a', name: 'A', priority: 2, isDefault: true };
  const second = { id: 'provider-b', name: 'B', priority: 1 };
  const store = await createStore({ native: false, invoke: async (command) => {
    if (command === 'get_providers') return [first, second];
  } });
  await store.getState().fetchInitialData();
  await store.getState().saveProvider({ ...second, isDefault: true });
  const defaults = store.getState().providers.filter((provider) => provider.isDefault).map((provider) => provider.id);
  assert.deepEqual(defaults, ['provider-b']);
});

test('切换为跟随系统时向原生窗口传递 null，切换深浅色时传递对应主题名', async () => {
  const themeCalls = [];
  const store = await createStore({
    onSetTheme: (theme) => {
      themeCalls.push(theme);
    },
  });

  store.getState().setTheme('dark');
  assert.equal(store.getState().theme, 'dark');
  assert.equal(themeCalls.at(-1), 'dark');

  store.getState().setTheme('light');
  assert.equal(store.getState().theme, 'light');
  assert.equal(themeCalls.at(-1), 'light');

  store.getState().setTheme('system');
  assert.equal(store.getState().theme, 'system');
  assert.equal(themeCalls.at(-1), null);
});

test('applyAppTheme 在跟随系统模式下根据系统明暗应用 DOM 类名并解绑原生窗口外观锁定', async () => {
  const themeCalls = [];
  const classList = new Set();
  const style = {};
  globalThis.document = {
    documentElement: {
      classList: {
        toggle: (cls, force) => {
          if (force) classList.add(cls);
          else classList.delete(cls);
        },
        contains: (cls) => classList.has(cls),
      },
      style,
    },
  };
  globalThis.window.matchMedia = (query) => ({
    matches: query.includes('dark'),
    addEventListener: () => {},
    removeEventListener: () => {},
  });

  const store = await createStore({
    onSetTheme: (theme) => themeCalls.push(theme),
  });

  // 系统为深色时，system 主题应给 document 增加 dark 类，并向窗口传 null
  store.applyAppTheme('system', true);
  assert.equal(classList.has('dark'), true);
  assert.equal(style.colorScheme, 'dark');
  assert.equal(themeCalls.at(-1), null);

  // 系统为浅色时，system 主题应移除 dark 类，并向窗口传 null
  store.applyAppTheme('system', false);
  assert.equal(classList.has('dark'), false);
  assert.equal(style.colorScheme, 'light');
  assert.equal(themeCalls.at(-1), null);

  // 显式暗色
  store.applyAppTheme('dark');
  assert.equal(classList.has('dark'), true);
  assert.equal(style.colorScheme, 'dark');
  assert.equal(themeCalls.at(-1), 'dark');

  // 显式浅色
  store.applyAppTheme('light');
  assert.equal(classList.has('dark'), false);
  assert.equal(style.colorScheme, 'light');
  assert.equal(themeCalls.at(-1), 'light');

  delete globalThis.document;
});
