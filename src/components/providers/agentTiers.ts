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
    label: '主力编程',
    shortLabel: '主力',
    icon: Code2,
    pattern: '^(pivot/main|claude-3-7-sonnet.*|claude-sonnet.*|sonnet)$',
    description: 'Agent 主力编程槽位 (Sonnet/主力任务)',
    iconClass: 'text-blue-600 dark:text-blue-400',
  },
  fast: {
    id: 'tier-fast',
    label: '轻量快速',
    shortLabel: '快速',
    icon: Zap,
    pattern: '^(pivot/fast|claude-3-5-haiku.*|claude-haiku.*|haiku)$',
    description: 'Agent 轻量辅助槽位 (Haiku/子任务)',
    iconClass: 'text-amber-600 dark:text-amber-400',
  },
  reasoning: {
    id: 'tier-reasoning',
    label: '深度推理',
    shortLabel: '推理',
    icon: Brain,
    pattern: '^(pivot/reasoning|claude-3-opus.*|claude-opus.*|opus)$',
    description: 'Agent 深度推理槽位 (Opus/Thinking/o-series)',
    iconClass: 'text-purple-600 dark:text-purple-400',
  },
};

export const AGENT_TIER_KEYS: AgentTierKey[] = ['main', 'fast', 'reasoning'];
export const AGENT_TIER_IDS = Object.values(AGENT_TIERS).map((tier) => tier.id);
