interface ModelRate {
  prompt: number; // USD per 1M tokens
  completion: number; // USD per 1M tokens
}

// 常见主流大模型每 100 万 Token 的官方公价（USD）
const MODEL_PRICING: Record<string, ModelRate> = {
  // Anthropic Claude
  'claude-3-7-sonnet': { prompt: 3.0, completion: 15.0 },
  'claude-3-5-sonnet': { prompt: 3.0, completion: 15.0 },
  'claude-3-5-haiku': { prompt: 0.8, completion: 4.0 },
  'claude-3-haiku': { prompt: 0.25, completion: 1.25 },
  'claude-3-opus': { prompt: 15.0, completion: 75.0 },

  // OpenAI
  'gpt-4o': { prompt: 2.5, completion: 10.0 },
  'gpt-4o-mini': { prompt: 0.15, completion: 0.6 },
  'o1': { prompt: 15.0, completion: 60.0 },
  'o1-preview': { prompt: 15.0, completion: 60.0 },
  'o1-mini': { prompt: 3.0, completion: 12.0 },
  'o3-mini': { prompt: 1.1, completion: 4.4 },
  'gpt-4-turbo': { prompt: 10.0, completion: 30.0 },
  'gpt-4': { prompt: 30.0, completion: 60.0 },
  'gpt-3.5-turbo': { prompt: 0.5, completion: 1.5 },

  // Google Gemini
  'gemini-2.5-pro': { prompt: 1.25, completion: 5.0 },
  'gemini-1.5-pro': { prompt: 1.25, completion: 5.0 },
  'gemini-2.5-flash': { prompt: 0.075, completion: 0.3 },
  'gemini-2.0-flash': { prompt: 0.075, completion: 0.3 },
  'gemini-1.5-flash': { prompt: 0.075, completion: 0.3 },
  'gemini-3.8-flash': { prompt: 0.1, completion: 0.4 },
  'gemini-3.8-flash-high': { prompt: 0.1, completion: 0.4 },

  // DeepSeek
  'deepseek-chat': { prompt: 0.14, completion: 0.28 },
  'deepseek-v3': { prompt: 0.14, completion: 0.28 },
  'deepseek-reasoner': { prompt: 0.55, completion: 2.19 },
  'deepseek-r1': { prompt: 0.55, completion: 2.19 },
};

// 默认基准模型费率（用于未收录模型）：输入 $2.0/M，输出 $8.0/M
const DEFAULT_RATE: ModelRate = { prompt: 2.0, completion: 8.0 };

export function getModelRate(modelName?: string | null): ModelRate {
  if (!modelName) return DEFAULT_RATE;
  const normalized = modelName.toLowerCase().trim();

  // 精准与前缀模糊匹配
  for (const [key, rate] of Object.entries(MODEL_PRICING)) {
    if (normalized === key || normalized.includes(key) || key.includes(normalized)) {
      return rate;
    }
  }

  // 厂商大类模糊匹配
  if (normalized.includes('haiku')) return MODEL_PRICING['claude-3-5-haiku'];
  if (normalized.includes('opus')) return MODEL_PRICING['claude-3-opus'];
  if (normalized.includes('sonnet')) return MODEL_PRICING['claude-3-7-sonnet'];
  if (normalized.includes('claude')) return MODEL_PRICING['claude-3-5-sonnet'];
  if (normalized.includes('o3')) return MODEL_PRICING['o3-mini'];
  if (normalized.includes('o1-mini')) return MODEL_PRICING['o1-mini'];
  if (normalized.includes('o1')) return MODEL_PRICING['o1'];
  if (normalized.includes('4o-mini')) return MODEL_PRICING['gpt-4o-mini'];
  if (normalized.includes('4o')) return MODEL_PRICING['gpt-4o'];
  if (normalized.includes('deepseek-r1') || normalized.includes('reasoner')) return MODEL_PRICING['deepseek-reasoner'];
  if (normalized.includes('deepseek')) return MODEL_PRICING['deepseek-chat'];
  if (normalized.includes('flash')) return MODEL_PRICING['gemini-2.0-flash'];
  if (normalized.includes('gemini')) return MODEL_PRICING['gemini-1.5-pro'];

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
