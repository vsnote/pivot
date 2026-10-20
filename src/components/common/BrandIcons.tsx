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
  'from-zinc-100 to-zinc-200/80 dark:from-zinc-800 dark:to-zinc-900 text-zinc-800 dark:text-zinc-200 border-zinc-200/90 dark:border-zinc-700/80',
  'from-slate-100 to-slate-200/80 dark:from-slate-800 dark:to-slate-900 text-slate-800 dark:text-slate-200 border-slate-200/90 dark:border-slate-700/80',
  'from-stone-100 to-stone-200/80 dark:from-stone-800 dark:to-stone-900 text-stone-800 dark:text-stone-200 border-stone-200/90 dark:border-stone-700/80',
  'from-neutral-100 to-neutral-200/80 dark:from-neutral-800 dark:to-neutral-900 text-neutral-800 dark:text-neutral-200 border-neutral-200/90 dark:border-neutral-700/80',
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
          className={`${sizeClass} rounded-md flex items-center justify-center shrink-0 border border-zinc-200/80 dark:border-zinc-700/80 bg-zinc-100/90 dark:bg-zinc-800 text-zinc-900 dark:text-zinc-100 p-1.5 shadow-[0_1px_2px_rgba(0,0,0,0.04)] dark:shadow-[0_1px_2px_rgba(0,0,0,0.25)] transition-colors ${className}`}
        >
          <IconRenderer className="w-full h-full object-contain drop-shadow-[0_0.5px_1px_rgba(0,0,0,0.08)]" />
        </div>
      );
    }
  }

  // Fallback 字母徽章：精工微渐变中性底槽，低饱和度，与品牌 Logo 和谐同构
  const palette = getAvatarPalette(name);
  const letter = getInitialLetter(name);
  return (
    <div
      className={`${sizeClass} rounded-md flex items-center justify-center shrink-0 border bg-gradient-to-b font-mono font-semibold text-[13px] tracking-tight select-none shadow-[0_1px_2px_rgba(0,0,0,0.04)] dark:shadow-[0_1px_2px_rgba(0,0,0,0.25)] ${palette} ${className}`}
    >
      {letter}
    </div>
  );
};
