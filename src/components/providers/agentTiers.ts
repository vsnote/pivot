import { Brain, Code2, Zap } from 'lucide-react';

export type AgentTierKey = 'main' | 'fast' | 'reasoning';

export interface AgentTier {
  id: string;
  label: string;
  shortLabel: string;
  icon: typeof Code2;
  pattern: string;
  description: string;
  iconClass: string;
}

export const AGENT_TIERS: Record<AgentTierKey, AgentTier> = {
  main: {
    id: 'tier-main',
    label: '主力模型',
    shortLabel: '主力',
    icon: Code2,
    pattern: '^(pivot/main|main)$',
    description: '核心工作负载与复杂编码 (pivot/main)',
    iconClass: 'text-blue-600 dark:text-blue-400',
  },
  fast: {
    id: 'tier-fast',
    label: '快速模型',
    shortLabel: '快速',
    icon: Zap,
    pattern: '^(pivot/fast|fast)$',
    description: '低延迟子任务与轻量编辑 (pivot/fast)',
    iconClass: 'text-amber-600 dark:text-amber-400',
  },
  reasoning: {
    id: 'tier-reasoning',
    label: '推理模型',
    shortLabel: '推理',
    icon: Brain,
    pattern: '^(pivot/reasoning|reasoning)$',
    description: '深度逻辑与思考规划 (pivot/reasoning)',
    iconClass: 'text-purple-600 dark:text-purple-400',
  },
};

export const AGENT_TIER_KEYS: AgentTierKey[] = ['main', 'fast', 'reasoning'];
export const AGENT_TIER_IDS = Object.values(AGENT_TIERS).map((tier) => tier.id);
