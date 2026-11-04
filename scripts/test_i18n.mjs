import assert from 'node:assert/strict';
import { test } from 'node:test';
import { readFile } from 'node:fs/promises';
import ts from 'typescript';

// Transpile i18n files
const zhSource = await readFile(new URL('../src/i18n/locales/zh-CN.ts', import.meta.url), 'utf8');
const enSource = await readFile(new URL('../src/i18n/locales/en.ts', import.meta.url), 'utf8');

const zhCompiled = ts.transpileModule(zhSource, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;

const enCompiled = ts.transpileModule(enSource, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
}).outputText;

const { zhCN } = await import(`data:text/javascript,${encodeURIComponent(zhCompiled)}`);
const { en } = await import(`data:text/javascript,${encodeURIComponent(enCompiled)}`);

function getKeys(obj, prefix = '') {
  const keys = [];
  for (const [k, v] of Object.entries(obj)) {
    const fullKey = prefix ? `${prefix}.${k}` : k;
    if (v && typeof v === 'object' && !Array.isArray(v)) {
      keys.push(...getKeys(v, fullKey));
    } else {
      keys.push(fullKey);
    }
  }
  return keys;
}

test('多语言字典完整性：zh-CN 与 en 词条完全对齐', () => {
  const zhKeys = getKeys(zhCN).sort();
  const enKeys = getKeys(en).sort();

  const missingInEn = zhKeys.filter((k) => !enKeys.includes(k));
  const missingInZh = enKeys.filter((k) => !zhKeys.includes(k));

  assert.deepEqual(missingInEn, [], 'en 字典缺少 zh-CN 字典的词条');
  assert.deepEqual(missingInZh, [], 'zh-CN 字典缺少 en 字典的词条');
  assert.ok(zhKeys.length > 100, '词条总数应覆盖系统主要界面');
});

test('铁律验证：严禁出现默认服务商开关，标识统一使用「默认模型」', () => {
  const allZhStrings = JSON.stringify(zhCN);
  const allEnStrings = JSON.stringify(en);

  // 绝不能有“默认服务商”或“首选模型”
  assert.equal(allZhStrings.includes('默认服务商'), false, '字典中严禁出现「默认服务商」');
  assert.equal(allZhStrings.includes('首选模型'), false, '字典中禁止出现「首选模型」混淆叫法');
  assert.equal(allZhStrings.includes('默认模型'), true, '中文标识必须包含「默认模型」');

  assert.equal(allEnStrings.includes('Default Provider'), false, '英文字典严禁出现 Default Provider');
  assert.equal(allEnStrings.includes('Default Model'), true, '英文标识必须为 Default Model');
});

test('多语言插值逻辑验证', () => {
  function interpolate(text, params) {
    if (!params) return text;
    return text.replace(/{(\w+)}/g, (_, k) => (k in params ? String(params[k]) : `{${k}}`));
  }

  const zhTemplate = zhCN.providerEditor.accountAvailable;
  assert.equal(interpolate(zhTemplate, { available: 2, total: 3 }), '2 / 3 可用');

  const enTemplate = en.providerEditor.accountAvailable;
  assert.equal(interpolate(enTemplate, { available: 2, total: 3 }), '2 / 3 available');
});

test('全量源码 t(...) 静态多语言调用键全覆盖验证', async () => {
  const fs = await import('node:fs');
  const path = await import('node:path');

  function getNested(obj, p) {
    const parts = p.split('.');
    let curr = obj;
    for (const part of parts) {
      if (curr == null || typeof curr !== 'object') return undefined;
      curr = curr[part];
    }
    return curr;
  }

  const missingInZh = [];
  const missingInEn = [];

  function walk(dir) {
    for (const item of fs.readdirSync(dir)) {
      const full = path.join(dir, item);
      if (fs.statSync(full).isDirectory()) {
        if (item !== 'node_modules' && item !== 'dist' && item !== '.git') {
          walk(full);
        }
      } else if (full.endsWith('.ts') || full.endsWith('.tsx')) {
        const code = fs.readFileSync(full, 'utf8');
        // Match t('domain.key') but exclude false positives
        const matches = code.matchAll(/\bt\(\s*['"]([a-zA-Z0-9_]+(?:\.[a-zA-Z0-9_]+)+)['"]/g);
        for (const m of matches) {
          const k = m[1];
          if (getNested(zhCN, k) === undefined) {
            missingInZh.push({ file: path.relative(process.cwd(), full), key: k });
          }
          if (getNested(en, k) === undefined) {
            missingInEn.push({ file: path.relative(process.cwd(), full), key: k });
          }
        }
      }
    }
  }

  walk(new URL('../src', import.meta.url).pathname);

  assert.deepEqual(missingInZh, [], `源码中存在未在 zh-CN 字典中定义的键: ${JSON.stringify(missingInZh)}`);
  assert.deepEqual(missingInEn, [], `源码中存在未在 en 字典中定义的键: ${JSON.stringify(missingInEn)}`);
});
