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

export const PiLogo: React.FC<{ className?: string }> = ({ className = 'w-4 h-4' }) => {
  const Component = BrandIconRenderers.pi;
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
  | 'pi'
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
  if (n.includes('pi agent') || n === 'pi' || n.startsWith('pi ') || n.startsWith('pi-')) return 'pi';
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

function getInitialLetter(name: string): string {
  if (!name) return 'AI';
  return name.trim()[0].toUpperCase();
}

// 无边界的软底槽：用 7% 中性底 + 1px 内描边替代硬边框，避免在浅色卡片上出现生硬的轮廓线
export const ICON_SLOT =
  'rounded-md bg-zinc-500/[0.07] dark:bg-white/[0.07] ' +
  'shadow-[inset_0_0_0_1px_rgba(0,0,0,0.045)] dark:shadow-[inset_0_0_0_1px_rgba(255,255,255,0.07)]';

export const AVATAR_SHELL = `flex items-center justify-center shrink-0 transition-colors ${ICON_SLOT}`;

export const ProviderAvatar: React.FC<{
  name: string;
  protocol?: string;
  iconKey?: string;
  className?: string;
  sizeClass?: string;
  /** 24px 及以下的小槽位：收紧内边距与字母字号，避免图形被挤压、字母撑满 */
  dense?: boolean;
}> = ({ name, protocol, iconKey, className = '', sizeClass = 'w-9 h-9', dense = false }) => {
  const brand = detectProviderBrand(name, protocol, iconKey);

  if (brand !== 'custom') {
    const IconRenderer = BrandIconRenderers[brand];
    if (IconRenderer) {
      return (
        <div
          className={`${sizeClass} ${AVATAR_SHELL} ${dense ? 'p-[3px]' : 'p-1.5'} text-zinc-900 dark:text-zinc-100 ${className}`}
        >
          <IconRenderer className="w-full h-full object-contain" />
        </div>
      );
    }
  }

  // Fallback 字母徽章：与品牌图标共用同一底槽，保持列表内视觉重量一致
  return (
    <div
      className={`${sizeClass} ${AVATAR_SHELL} font-mono font-medium ${dense ? 'text-[11px]' : 'text-[12px]'} select-none text-zinc-600 dark:text-zinc-400 ${className}`}
    >
      {getInitialLetter(name)}
    </div>
  );
};
