import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { test } from 'node:test';
import ts from 'typescript';

const source = await readFile(new URL('../src/store/useAppStore.ts', import.meta.url), 'utf8');
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;
let instance = 0;

async function createStore({ native = true, invoke = async () => undefined, listen = async () => () => {} } = {}) {
  const key = `omni-store-test-${instance++}`;
  globalThis[key] = { native, invoke, listen };
  globalThis.window = { location: { protocol: 'http:' } };
  globalThis.localStorage = { getItem: () => null, setItem: () => {} };
  const tauriMock = `data:text/javascript,${encodeURIComponent(`
    const mock = globalThis[${JSON.stringify(key)}];
    export const isTauri = () => mock.native;
    export const invoke = (...args) => mock.invoke(...args);
    export const listen = (...args) => mock.listen(...args);
    export const getCurrentWindow = () => ({ setTheme: async () => {} });
  `)}`;
  const moduleSource = compiled
    .replace(/from ['"]zustand['"]/g, `from ${JSON.stringify(import.meta.resolve('zustand'))}`)
    .replace(/from ['"]@tauri-apps\/api\/(?:core|event|window)['"]/g, `from ${JSON.stringify(tauriMock)}`);
  const { useAppStore } = await import(`data:text/javascript,${encodeURIComponent(moduleSource)}`);
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
  store.getState().setSettingsSection('routes');
  assert.equal(store.getState().activeTab, 'settings');
  assert.deepEqual(calls, [
    ['sync_native_tab', { tab: 'gateway' }],
    ['sync_native_tab', { tab: 'providers' }],
    ['sync_native_tab', { tab: 'routes' }],
  ]);
});

test('打开服务商编辑页统一切回列表，重复点击新增不覆盖正在编辑的草稿', async () => {
  const provider = { id: 'provider-a', name: 'Provider A', priority: 1 };
  const store = await createStore({ invoke: async (command) => command === 'get_providers' ? [provider] : [] });
  await store.getState().fetchInitialData();
  store.getState().setActiveTab('routes');
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
    if (command === 'get_routes') return [];
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

test('路由预览使用网关照常解析，结果原样返回', async () => {
  const calls = [];
  const plan = { primary: { providerId: 'antigravity', model: 'gemini-3.8-flash-high' }, fallbacks: [] };
  const store = await createStore({ invoke: async (...args) => {
    calls.push(args);
    if (args[0] === 'preview_route') return plan;
  } });
  assert.deepEqual(await store.getState().previewRoute('claude-3-7-sonnet'), plan);
  assert.deepEqual(calls, [['preview_route', { model: 'claude-3-7-sonnet' }]]);
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
