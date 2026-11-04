import assert from 'node:assert/strict';
import { readFile, writeFile, unlink } from 'node:fs/promises';
import { test } from 'node:test';
import { pathToFileURL } from 'node:url';
import ts from 'typescript';

const tempFile = new URL(`./_temp_brand_test_${Date.now()}.mjs`, import.meta.url);

try {
  // 读取 BrandIcons.tsx 并保留纯 JS 逻辑
  const brandIconsSource = await readFile(
    new URL('../src/components/common/BrandIcons.tsx', import.meta.url),
    'utf8',
  );
  // 用一个 stub 替代 BrandIconSvgs 导入
  const mockedSource = brandIconsSource.replace(
    "import { BrandIconRenderers } from './BrandIconSvgs';",
    `export const BrandIconRenderers = {
      anthropic: true, openai: true, gemini: true, deepseek: true, openrouter: true,
      ollama: true, groq: true, mistral: true, siliconflow: true, moonshot: true,
      zhipu: true, qwen: true, minimax: true, stepfun: true, baichuan: true,
      '01ai': true, together: true, cerebras: true, perplexity: true, cohere: true,
      github: true, fireworks: true, xai: true, novita: true, deepinfra: true,
      hyperbolic: true, sambanova: true, hunyuan: true, wenxin: true, lmstudio: true,
      vllm: true, cloudflare: true, claude: true, claudecode: true, pi: true, antigravity: true
    };`,
  );

  const compiled = ts.transpileModule(mockedSource, {
    compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022, jsx: ts.JsxEmit.React },
  }).outputText;

  await writeFile(tempFile, compiled, 'utf8');
  const { detectProviderBrand, BrandIconRenderers } = await import(pathToFileURL(tempFile.pathname).href);

  // 同时也直接检查真实 BrandIconSvgs.tsx 文件中是否包含所有这些 key
  const brandSvgSource = await readFile(
    new URL('../src/components/common/BrandIconSvgs.tsx', import.meta.url),
    'utf8',
  );

  // 读取 Rust presets.rs 检查名称
  const presetsRs = await readFile(
    new URL('../src-tauri/src/db/presets.rs', import.meta.url),
    'utf8',
  );

  test('后端预设服务商名称全部采用官方纯净命名，无说明或翻译后缀', () => {
    const nameMatches = [...presetsRs.matchAll(/name:\s*"([^"]+)"/g)].map((m) => m[1]);
    assert.ok(nameMatches.length >= 30, '至少应有 30 个预设服务商');

    for (const name of nameMatches) {
      assert.doesNotMatch(name, /[\(（][^\)）]+[\)）]/, `预设名称 "${name}" 不应包含说明括号`);
      assert.doesNotMatch(name, /官方/, `预设名称 "${name}" 不应包含 "(官方)"`);
      assert.doesNotMatch(name, /旗舰/, `预设名称 "${name}" 不应包含翻译/说明`);
      assert.doesNotMatch(name, /本地离线/, `预设名称 "${name}" 不应包含说明`);
    }

    assert.ok(nameMatches.includes('Anthropic'));
    assert.ok(nameMatches.includes('OpenAI'));
    assert.ok(nameMatches.includes('Google Gemini'));
    assert.ok(nameMatches.includes('DeepSeek'));
    assert.ok(nameMatches.includes('Mistral AI'));
    assert.ok(nameMatches.includes('SiliconFlow'));
    assert.ok(nameMatches.includes('Moonshot AI'));
    assert.ok(nameMatches.includes('智谱 AI'));
    assert.ok(nameMatches.includes('通义千问'));
    assert.ok(nameMatches.includes('MiniMax'));
    assert.ok(nameMatches.includes('StepFun'));
    assert.ok(nameMatches.includes('百川智能'));
    assert.ok(nameMatches.includes('零一万物'));
    assert.ok(nameMatches.includes('Together AI'));
    assert.ok(nameMatches.includes('Groq'));
  });

  test('主流 AI 服务商品牌均可准确匹配到官方真实 SVG 图标渲染器', () => {
    const checks = [
      { name: 'Mistral AI', expected: 'mistral' },
      { name: 'SiliconFlow', expected: 'siliconflow' },
      { name: 'Moonshot AI', expected: 'moonshot' },
      { name: 'Kimi', expected: 'moonshot' },
      { name: '智谱 AI', expected: 'zhipu' },
      { name: '通义千问', expected: 'qwen' },
      { name: 'MiniMax', expected: 'minimax' },
      { name: 'StepFun', expected: 'stepfun' },
      { name: '阶跃星辰', expected: 'stepfun' },
      { name: '百川智能', expected: 'baichuan' },
      { name: '零一万物', expected: '01ai' },
      { name: 'Together AI', expected: 'together' },
      { name: 'Groq', expected: 'groq' },
      { name: 'DeepSeek', expected: 'deepseek' },
      { name: 'Anthropic', expected: 'anthropic' },
      { name: 'Claude', expected: 'claude' },
      { name: 'Claude Code', expected: 'claudecode' },
      { name: 'OpenAI', expected: 'openai' },
      { name: 'Google Gemini', expected: 'gemini' },
      { name: 'Ollama', expected: 'ollama' },
      { name: 'OpenRouter', expected: 'openrouter' },
      { name: 'Perplexity', expected: 'perplexity' },
      { name: 'Cohere', expected: 'cohere' },
      { name: 'GitHub Models', expected: 'github' },
      { name: 'xAI', expected: 'xai' },
      { name: 'Pi Agent', expected: 'pi' },
      { name: 'Antigravity', expected: 'antigravity' },
    ];

    for (const { name, expected } of checks) {
      const brand = detectProviderBrand(name);
      assert.equal(brand, expected, `名称 "${name}" 期望识别为 "${expected}"，实际识别为 "${brand}"`);
      assert.ok(BrandIconRenderers[brand], `品牌 "${brand}" 必须在 BrandIconRenderers 中存在映射`);
      assert.ok(
        brandSvgSource.includes(`"${expected}":`),
        `BrandIconSvgs.tsx 中必须包含 "${expected}" 的实际真实 SVG 导出`,
      );
    }
  });

  test('显式传递 iconKey 或别名时，优先精准匹配官方真实图标', () => {
    assert.equal(detectProviderBrand('自定义网关', 'openai', 'alibaba'), 'qwen');
    assert.equal(detectProviderBrand('自定义网关', 'openai', 'siliconcloud'), 'siliconflow');
    assert.equal(detectProviderBrand('自定义网关', 'openai', 'kimi'), 'moonshot');
    assert.equal(detectProviderBrand('自定义网关', 'openai', 'tencent'), 'hunyuan');
    assert.equal(detectProviderBrand('自定义网关', 'openai', 'baidu'), 'wenxin');
    assert.equal(detectProviderBrand('自定义网关', 'openai', '01ai'), '01ai');
    assert.equal(detectProviderBrand('未知自建服务', 'openai', undefined), 'custom');
    assert.equal(detectProviderBrand('anyrouter-linuxdo', 'anthropic', undefined), 'custom');
    assert.equal(detectProviderBrand('自建中转', 'gemini', undefined), 'custom');
  });
} finally {
  try {
    await unlink(tempFile);
  } catch {
    /* ignore */
  }
}
