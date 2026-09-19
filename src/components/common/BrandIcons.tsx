import React from 'react';
import { BrandIconRenderers } from './BrandIconSvgs';

export const OpenAISpiralIcon: React.FC<{ className?: string }> = ({ className = 'w-4 h-4' }) => {
  const Component = BrandIconRenderers.openai;
  return Component ? <Component className={className} /> : null;
};

export const ClaudeLogo: React.FC<{ className?: string }> = ({ className = 'w-4 h-4' }) => {
  const Component = BrandIconRenderers.claude;
  return Component ? <Component className={className} /> : null;
};

export const AnthropicLogo: React.FC<{ className?: string }> = ({ className = 'w-4 h-4' }) => {
  const Component = BrandIconRenderers.anthropic;
  return Component ? <Component className={className} /> : null;
};

export const GeminiLogo: React.FC<{ className?: string }> = ({ className = 'w-4 h-4' }) => {
  const Component = BrandIconRenderers.gemini;
  return Component ? <Component className={className} /> : null;
};

export const DeepSeekLogo: React.FC<{ className?: string }> = ({ className = 'w-4 h-4' }) => {
  const Component = BrandIconRenderers.deepseek;
  return Component ? <Component className={className} /> : null;
};

export const OllamaLogo: React.FC<{ className?: string }> = ({ className = 'w-4 h-4' }) => {
  const Component = BrandIconRenderers.ollama;
  return Component ? <Component className={className} /> : null;
};

export const OpenRouterLogo: React.FC<{ className?: string }> = ({ className = 'w-4 h-4' }) => {
  const Component = BrandIconRenderers.openrouter;
  return Component ? <Component className={className} /> : null;
};

export type BrandType =
  | 'openai'
  | 'anthropic'
  | 'claude'
  | 'claudecode'
  | 'gemini'
  | 'deepseek'
  | 'openrouter'
  | 'ollama'
  | 'groq'
  | 'mistral'
  | 'siliconflow'
  | 'moonshot'
  | 'zhipu'
  | 'qwen'
  | 'minimax'
  | 'stepfun'
  | 'baichuan'
  | '01ai'
  | 'together'
  | 'cerebras'
  | 'perplexity'
  | 'cohere'
  | 'github'
  | 'fireworks'
  | 'xai'
  | 'novita'
  | 'deepinfra'
  | 'hyperbolic'
  | 'sambanova'
  | 'hunyuan'
  | 'wenxin'
  | 'lmstudio'
  | 'vllm'
  | 'cloudflare'
  | 'custom';

const ALIAS_MAP: Record<string, string> = {
  alibaba: 'qwen',
  google: 'gemini',
  tencent: 'hunyuan',
  baidu: 'wenxin',
  siliconcloud: 'siliconflow',
  kimi: 'moonshot',
  zeroone: '01ai',
  yi: '01ai',
  grok: 'xai',
  workersai: 'cloudflare',
  github_models: 'github',
  tencent_hunyuan: 'hunyuan',
  baidu_qianfan: 'wenxin',
  cloudflare_ai: 'cloudflare',
  lingyiwanwu: '01ai',
};

export function detectProviderBrand(
  name: string,
  protocol?: string,
  iconKey?: string,
): BrandType {
  // 1. 若显式提供 iconKey 或其映射别名，优先匹配
  if (iconKey) {
    const normKey = iconKey.toLowerCase().trim();
    const mapped = ALIAS_MAP[normKey] || normKey;
    if (mapped in BrandIconRenderers) {
      return mapped as BrandType;
    }
  }

  const n = (name || '').toLowerCase();
  const p = (protocol || '').toLowerCase();

  // 2. 识别品牌名称与协议
  if (n.includes('deepseek') || n.includes('深度求索')) return 'deepseek';
  if (n.includes('claudecode') || n.includes('claude code')) return 'claudecode';
  if (n.includes('claude')) return 'claude';
  if (n.includes('anthropic') || p === 'anthropic') return 'anthropic';
  if (n.includes('gemini') || n.includes('google') || p === 'gemini') return 'gemini';
  if (n.includes('mistral') || n.includes('codestral')) return 'mistral';
  if (n.includes('siliconflow') || n.includes('硅基流动') || n.includes('siliconcloud'))
    return 'siliconflow';
  if (n.includes('moonshot') || n.includes('月之暗面') || n.includes('kimi')) return 'moonshot';
  if (n.includes('zhipu') || n.includes('智谱') || n.includes('glm') || n.includes('codegeex'))
    return 'zhipu';
  if (
    n.includes('qwen') ||
    n.includes('通义千问') ||
    n.includes('通义') ||
    n.includes('阿里') ||
    n.includes('dashscope')
  )
    return 'qwen';
  if (n.includes('minimax') || n.includes('名之梦')) return 'minimax';
  if (n.includes('stepfun') || n.includes('阶跃星辰')) return 'stepfun';
  if (n.includes('baichuan') || n.includes('百川')) return 'baichuan';
  if (n.includes('01.ai') || n.includes('01ai') || n.includes('零一万物') || n.includes('yi-'))
    return '01ai';
  if (n.includes('together')) return 'together';
  if (n.includes('cerebras')) return 'cerebras';
  if (n.includes('perplexity') || n.includes('sonar')) return 'perplexity';
  if (n.includes('cohere') || n.includes('command-r')) return 'cohere';
  if (n.includes('github')) return 'github';
  if (n.includes('fireworks')) return 'fireworks';
  if (n.includes('xai') || n.includes('grok')) return 'xai';
  if (n.includes('novita')) return 'novita';
  if (n.includes('deepinfra')) return 'deepinfra';
  if (n.includes('hyperbolic')) return 'hyperbolic';
  if (n.includes('sambanova')) return 'sambanova';
  if (n.includes('hunyuan') || n.includes('混元') || n.includes('tencent')) return 'hunyuan';
  if (
    n.includes('wenxin') ||
    n.includes('文心') ||
    n.includes('千帆') ||
    n.includes('qianfan') ||
    n.includes('baidu')
  )
    return 'wenxin';
  if (n.includes('lmstudio') || n.includes('lm studio')) return 'lmstudio';
  if (n.includes('vllm')) return 'vllm';
  if (n.includes('cloudflare') || n.includes('workers ai') || n.includes('workersai'))
    return 'cloudflare';
  if (n.includes('groq')) return 'groq';
  if (n.includes('ollama') || p === 'ollama') return 'ollama';
  if (n.includes('openrouter') || p === 'openrouter') return 'openrouter';
  if (
    n.includes('openai') ||
    n.includes('chatgpt') ||
    (p === 'openai' && (n.includes('gpt') || n.includes('oai')))
  ) {
    return 'openai';
  }

  // 若仅是协议兼容且为自定义名称（如用户的私有中转、Work 等），视为 custom 专属字母徽章
  return 'custom';
}

const AVATAR_PALETTES = [
  'bg-indigo-500/10 dark:bg-indigo-500/20 text-indigo-600 dark:text-indigo-400 border-indigo-500/20',
  'bg-violet-500/10 dark:bg-violet-500/20 text-violet-600 dark:text-violet-400 border-violet-500/20',
  'bg-sky-500/10 dark:bg-sky-500/20 text-sky-600 dark:text-sky-400 border-sky-500/20',
  'bg-emerald-500/10 dark:bg-emerald-500/20 text-emerald-600 dark:text-emerald-400 border-emerald-500/20',
  'bg-amber-500/10 dark:bg-amber-500/20 text-amber-600 dark:text-amber-400 border-amber-500/20',
  'bg-rose-500/10 dark:bg-rose-500/20 text-rose-600 dark:text-rose-400 border-rose-500/20',
  'bg-cyan-500/10 dark:bg-cyan-500/20 text-cyan-600 dark:text-cyan-400 border-cyan-500/20',
];

function getInitialLetter(name: string): string {
  if (!name) return 'AI';
  const trimmed = name.trim();
  const firstChar = trimmed[0].toUpperCase();
  return firstChar;
}

function getAvatarPalette(name: string): string {
  let hash = 0;
  for (let i = 0; i < name.length; i++) {
    hash = (hash << 5) - hash + name.charCodeAt(i);
    hash |= 0;
  }
  const index = Math.abs(hash) % AVATAR_PALETTES.length;
  return AVATAR_PALETTES[index];
}

export const ProviderAvatar: React.FC<{
  name: string;
  protocol?: string;
  iconKey?: string;
  className?: string;
  sizeClass?: string;
}> = ({ name, protocol, iconKey, className = '', sizeClass = 'w-9 h-9' }) => {
  const brand = detectProviderBrand(name, protocol, iconKey);

  if (brand !== 'custom') {
    const IconRenderer = BrandIconRenderers[brand];
    if (IconRenderer) {
      return (
        <div
          className={`${sizeClass} rounded-xl flex items-center justify-center shrink-0 border border-zinc-200/70 dark:border-zinc-800/80 bg-zinc-50/80 dark:bg-zinc-850/60 text-zinc-850 dark:text-zinc-150 p-1.5 transition-colors ${className}`}
        >
          <IconRenderer className="w-full h-full object-contain" />
        </div>
      );
    }
  }

  // Fallback 字母徽章
  const palette = getAvatarPalette(name);
  const letter = getInitialLetter(name);
  return (
    <div
      className={`${sizeClass} rounded-xl flex items-center justify-center shrink-0 border font-semibold text-[13px] tracking-tight select-none shadow-2xs ${palette} ${className}`}
    >
      {letter}
    </div>
  );
};
