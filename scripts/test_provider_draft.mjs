import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { test } from 'node:test';
import ts from 'typescript';
const source = await readFile(new URL('../src/components/providers/providerDraft.ts', import.meta.url), 'utf8');
const compiled = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 } }).outputText;
const { createProviderDraft, applyProviderPreset, prepareProvider, validateBaseUrl, editablePresetUrl } = await import(`data:text/javascript,${encodeURIComponent(compiled)}`);
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

test('显式自定义配置不会被重新锁成预设，本地与账号专属地址可编辑', () => {
  const existing = { ...createProviderDraft(null, []), id: preset.id, name: preset.name, protocol: preset.protocol, baseUrl: preset.baseUrl, presetId: null };
  assert.equal(createProviderDraft(existing, [preset]).presetId, null);
  assert.equal(editablePresetUrl(preset), false);
  assert.equal(editablePresetUrl({ ...preset, category: '开源 / 本地部署' }), true);
  assert.equal(editablePresetUrl({ ...preset, baseUrl: 'https://example.com/YOUR_ACCOUNT_ID/v1' }), true);
  for (const address of ['file:///tmp', 'https://u:p@example.com', 'https://example.com?key=secret', 'https://example.com/v1/messages']) assert.ok(validateBaseUrl(address));
  assert.equal(validateBaseUrl('http://localhost:11434/v1'), null);
});
