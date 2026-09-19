import fs from 'node:fs';
import path from 'node:path';

const dir = 'node_modules/@lobehub/icons-static-svg/icons';
const map = {
  anthropic: 'anthropic.svg',
  openai: 'openai.svg',
  gemini: 'gemini-color.svg',
  deepseek: 'deepseek-color.svg',
  openrouter: 'openrouter-color.svg',
  ollama: 'ollama.svg',
  groq: 'groq.svg',
  mistral: 'mistral-color.svg',
  siliconflow: 'siliconcloud-color.svg',
  moonshot: 'kimi-color.svg',
  zhipu: 'zhipu-color.svg',
  qwen: 'qwen-color.svg',
  minimax: 'minimax-color.svg',
  stepfun: 'stepfun-color.svg',
  baichuan: 'baichuan-color.svg',
  '01ai': 'zeroone-color.svg',
  together: 'together-color.svg',
  cerebras: 'cerebras-color.svg',
  perplexity: 'perplexity-color.svg',
  cohere: 'cohere-color.svg',
  github: 'github.svg',
  fireworks: 'fireworks-color.svg',
  xai: 'xai.svg',
  novita: 'novita-color.svg',
  deepinfra: 'deepinfra-color.svg',
  hyperbolic: 'hyperbolic-color.svg',
  sambanova: 'sambanova-color.svg',
  hunyuan: 'hunyuan-color.svg',
  wenxin: 'wenxin-color.svg',
  lmstudio: 'lmstudio.svg',
  vllm: 'vllm-color.svg',
  cloudflare: 'cloudflare-color.svg',
  claude: 'claude-color.svg',
  claudecode: 'claudecode-color.svg',
};

// Copy to public/brand-icons
const publicDir = 'public/brand-icons';
if (!fs.existsSync(publicDir)) fs.mkdirSync(publicDir, { recursive: true });
for (const [k, file] of Object.entries(map)) {
  fs.copyFileSync(path.join(dir, file), path.join(publicDir, `${k}.svg`));
}
console.log('Copied SVGs to', publicDir);

function toCamelCase(str) {
  return str.replace(/-([a-z])/g, (g) => g[1].toUpperCase());
}

function cleanSvgInner(svgContent, key) {
  const match = svgContent.match(/<svg[^>]*>([\s\S]*?)<\/svg>/);
  if (!match) return '';
  let inner = match[1];

  inner = inner.replace(/<title>[\s\S]*?<\/title>/gi, '');

  const attrs = [
    'fill-rule',
    'clip-rule',
    'stroke-width',
    'stroke-linecap',
    'stroke-linejoin',
    'stop-color',
    'stop-opacity',
    'fill-opacity',
    'stroke-opacity',
    'stroke-miterlimit',
    'stroke-dasharray',
    'stroke-dashoffset',
    'color-interpolation-filters',
    'clip-path',
  ];
  for (const a of attrs) {
    const camel = toCamelCase(a);
    inner = inner.replace(new RegExp(a + '=', 'g'), camel + '=');
  }

  const idRegex = /id="([^"]+)"/g;
  const ids = [];
  let m;
  while ((m = idRegex.exec(inner)) !== null) {
    ids.push(m[1]);
  }
  for (const id of ids) {
    const newId = `brand_icon_${key}_${id}`;
    inner = inner.replace(new RegExp(`id="${id}"`, 'g'), `id="${newId}"`);
    inner = inner.replace(new RegExp(`url\\(#${id}\\)`, 'g'), `url(#${newId})`);
    inner = inner.replace(new RegExp(`"url\\(#${id}\\)"`, 'g'), `"url(#${newId})"`);
    inner = inner.replace(new RegExp(`href="#${id}"`, 'g'), `href="#${newId}"`);
    inner = inner.replace(new RegExp(`xlink:href="#${id}"`, 'g'), `xlinkHref="#${newId}"`);
  }

  return inner.trim();
}

let code = `import React from 'react';\n\n`;
code += `// Official brand vector SVG icons\n`;
code += `export const BrandIconRenderers: Record<string, React.FC<{ className?: string }>> = {\n`;

for (const [k, file] of Object.entries(map)) {
  const raw = fs.readFileSync(dir + '/' + file, 'utf8');
  const inner = cleanSvgInner(raw, k);
  code += `  ${JSON.stringify(k)}: ({ className = 'w-4 h-4' }) => (\n`;
  code += `    <svg viewBox="0 0 24 24" fill="currentColor" className={className} aria-hidden="true">\n`;
  code += `      ${inner}\n`;
  code += `    </svg>\n`;
  code += `  ),\n`;
}

code += `};\n`;

fs.writeFileSync('src/components/common/BrandIconSvgs.tsx', code, 'utf8');
console.log('Generated src/components/common/BrandIconSvgs.tsx successfully!');
