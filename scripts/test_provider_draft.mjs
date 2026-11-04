import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { test } from 'node:test';
import ts from 'typescript';
const source = await readFile(new URL('../src/components/providers/providerDraft.ts', import.meta.url), 'utf8');
const compiled = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 } }).outputText;
const { createProviderDraft, applyProviderPreset, prepareProvider, validateBaseUrl, editablePresetUrl, hasActualUserChanges } = await import(`data:text/javascript,${encodeURIComponent(compiled)}`);
const preset = { id: 'test', name: 'Test', protocol: 'openai', baseUrl: 'https://example.com/v1', category: '商业大模型', defaultModels: ['model-a'] };

test('重复使用预设创建独立草稿，切换服务商不带入旧密钥和模型', () => {
  const a = createProviderDraft(null, [preset]);
  const b = createProviderDraft(null, [preset]);
  assert.notEqual(a.id, b.id);
  const selected = applyProviderPreset({ ...a, apiKeys: ['old-key'], models: ['old-model'] }, preset);
  assert.equal(selected.id, a.id);
  assert.equal(selected.presetId, 'test');
  assert.deepEqual(selected.apiKeys, []);
  assert.deepEqual(selected.models, []);
  assert.equal(selected.enabled, true);
});

test('编辑和保存保留隐藏配置及停用状态，规范化密钥和模型', () => {
  const existing = { ...createProviderDraft(null, []), enabled: false, priority: 71, weight: 4, timeoutSecs: 180,
    name: '  Existing  ', baseUrl: 'https://example.com/v1/', apiKeys: [' key ', 'key', ''], models: [' org/model:tag ', 'org/model:tag'], createdAt: 'old-date' };
  const saved = prepareProvider(createProviderDraft(existing, []));
  for (const key of ['id', 'enabled', 'priority', 'weight', 'timeoutSecs', 'createdAt']) assert.equal(saved[key], existing[key]);
  assert.deepEqual(saved.apiKeys, ['key']);
  assert.deepEqual(saved.models, ['org/model:tag']);
  assert.equal(saved.baseUrl, 'https://example.com/v1');
});

test('服务商草稿规范化：默认模型与默认服务商始终重置为 null / false', () => {
  const first = createProviderDraft(null, [preset], true);
  assert.equal(first.isDefault, false);
  assert.equal(createProviderDraft(null, [preset], false).isDefault, false);

  // 选定预设会重建草稿
  assert.equal(applyProviderPreset(first, preset).isDefault, false);
  assert.equal(applyProviderPreset(first, preset).models.length, 0);

  // 编辑已有服务商时，字段均规范化
  const existing = { ...first, isDefault: true, defaultModel: 'test-model' };
  assert.equal(createProviderDraft(existing, [preset], true).isDefault, false);
  assert.equal(createProviderDraft(existing, [preset], true).defaultModel, null);
});

test('显式自定义配置不会被重新锁成预设，本地与账号专属地址可编辑', () => {
  const existing = { ...createProviderDraft(null, []), id: preset.id, name: preset.name, protocol: preset.protocol, baseUrl: preset.baseUrl, presetId: null };
  assert.equal(createProviderDraft(existing, [preset]).presetId, null);
  assert.equal(editablePresetUrl(preset), false);
  assert.equal(editablePresetUrl({ ...preset, category: '开源 / 本地部署' }), true);
  assert.equal(editablePresetUrl({ ...preset, baseUrl: 'https://example.com/YOUR_ACCOUNT_ID/v1' }), true);
  for (const address of ['file:///tmp', 'https://u:p@example.com', 'https://example.com?key=secret', 'https://example.com/v1/messages']) assert.ok(validateBaseUrl(address));
  assert.equal(validateBaseUrl('http://localhost:11434/v1'), null);
});

test('服务商保存时默认模型字段统一置空，由客户端独立模型接管', () => {
  const existing = { ...createProviderDraft(null, []), defaultModel: ' gemini-3.8-flash-high ', isDefault: true };
  assert.equal(prepareProvider(createProviderDraft(existing, [])).defaultModel, null);
  assert.equal(prepareProvider({ ...existing, defaultModel: '   ' }).defaultModel, null);
  assert.equal(prepareProvider({ ...existing, defaultModel: undefined }).defaultModel, null);
  const switched = applyProviderPreset(existing, preset);
  assert.equal(switched.defaultModel, null);
  assert.equal(switched.isDefault, false);
});

test('缺少默认路由字段的旧记录按未设置处理', () => {
  const legacy = createProviderDraft({ id: 'anthropic', name: 'Anthropic', protocol: 'anthropic',
    baseUrl: 'https://api.anthropic.com/v1', apiKeys: ['key'], models: ['claude-3-7-sonnet'] }, []);
  assert.equal(legacy.defaultModel, null);
  assert.equal(legacy.isDefault, false);
  const saved = prepareProvider(legacy);
  assert.equal(saved.defaultModel, null);
  assert.equal(saved.isDefault, false);
  assert.deepEqual(saved.models, ['claude-3-7-sonnet']);
});

test('选定预设但未输入任何内容时，不应视为实质改动', () => {
  const initial = createProviderDraft(null, [preset]);
  const chosen = applyProviderPreset(initial, preset);

  // 刚应用预设，未填 key、未选模型、未修改名称与地址 -> 无实质修改
  assert.equal(hasActualUserChanges(chosen, initial, null, preset, ''), false);

  // 输入了 key -> 有实质修改
  assert.equal(hasActualUserChanges({ ...chosen, apiKeys: ['sk-test'] }, initial, null, preset, ''), true);

  // 勾选了模型 -> 有实质修改
  assert.equal(hasActualUserChanges({ ...chosen, models: ['model-a'] }, initial, null, preset, ''), true);

  // 正在手动输入模型框中有文字 -> 有实质修改
  assert.equal(hasActualUserChanges(chosen, initial, null, preset, 'gpt-4o'), true);

  // 修改了名称 -> 有实质修改
  assert.equal(hasActualUserChanges({ ...chosen, name: 'My Custom Name' }, initial, null, preset, ''), true);

  // 修改了 Base URL -> 有实质修改
  assert.equal(hasActualUserChanges({ ...chosen, baseUrl: 'https://other.com/v1' }, initial, null, preset, ''), true);
});

