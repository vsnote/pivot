import { invoke, isTauri } from '@tauri-apps/api/core';

export interface ModelRate {
  prompt: number; // USD per 1M tokens
  completion: number; // USD per 1M tokens
}

// 目录未加载或模型未收录时的保守估算
const DEFAULT_RATE: ModelRate = { prompt: 2.0, completion: 8.0 };

// 后端从 models.dev 同步的费率表：规范化模型名 -> [输入价, 输出价]
let catalogRates: Record<string, [number, number]> = {};

/** 启动时加载一次；失败时保持默认估算，不影响日志其他功能 */
export async function loadModelRates(): Promise<void> {
  if (!isTauri()) return;
  try {
    catalogRates = await invoke<Record<string, [number, number]>>('get_model_rates');
  } catch {
    // 后端不可用时沿用默认估算
  }
}

/** 与后端 catalog 一致的别名尝试：去 provider 前缀、去日期与 -latest 后缀 */
const candidateKeys = (modelName: string): string[] => {
  const base = modelName.toLowerCase().trim();
  const keys = [base];

  const slash = base.lastIndexOf('/');
  if (slash >= 0) keys.push(base.slice(slash + 1));

  for (const key of [...keys]) {
    const slim = key.replace(/-latest$/, '').replace(/-(\d{4}-\d{2}-\d{2}|\d{8})$/, '');
    if (slim !== key) keys.push(slim);
  }
  return keys;
};

export function getModelRate(modelName?: string | null): ModelRate {
  if (!modelName) return DEFAULT_RATE;

  for (const key of candidateKeys(modelName)) {
    const hit = catalogRates[key];
    if (hit) return { prompt: hit[0], completion: hit[1] };
  }
  return DEFAULT_RATE;
}

/**
 * 计算单次请求或单项具体 Prompt / Completion Token 的预估成本（美元）
 */
export function estimateCost(
  modelName: string | undefined | null,
  promptTokens: number,
  completionTokens: number
): number {
  if (promptTokens <= 0 && completionTokens <= 0) return 0;
  const rate = getModelRate(modelName);
  const promptCost = (promptTokens / 1_000_000) * rate.prompt;
  const completionCost = (completionTokens / 1_000_000) * rate.completion;
  return promptCost + completionCost;
}

/**
 * 历史会话仅有总 Token 用量时的成本估算（按 75% 输入 + 25% 输出混合费率推算）
 */
export function estimateCostFromTotalTokens(
  modelName: string | undefined | null,
  totalTokens: number
): number {
  if (totalTokens <= 0) return 0;
  const rate = getModelRate(modelName);
  const blendedRate = rate.prompt * 0.75 + rate.completion * 0.25;
  return (totalTokens / 1_000_000) * blendedRate;
}

/**
 * 货币美化显示
 */
export function formatCurrency(amount: number): string {
  if (!amount || amount <= 0) return '$0.00';
  if (amount < 0.0001) return '<$0.0001';
  if (amount < 0.01) return `$${amount.toFixed(4)}`;
  if (amount < 1) return `$${amount.toFixed(3)}`;
  return `$${amount.toFixed(2)}`;
}

/**
 * Token 用量美化显示
 */
export function formatTokens(tokens: number): string {
  if (!tokens || tokens <= 0) return '0';
  if (tokens >= 1_000_000) return `${(tokens / 1_000_000).toFixed(2)} M`;
  if (tokens >= 1_000) return `${(tokens / 1_000).toFixed(1)} K`;
  return tokens.toLocaleString();
}
