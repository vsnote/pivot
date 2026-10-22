import { LoaderCircle } from 'lucide-react';
import { useAppStore } from '@/store/useAppStore';
import { AgentTiersHero } from './AgentTiersHero';
import { ModelRoutingCard } from './ModelRoutingCard';

export function RoutesView() {
  const dataReady = useAppStore((state) => state.initialLoaded && !state.isLoading);

  if (!dataReady) {
    return (
      <div className="p-12 text-center">
        <LoaderCircle className="w-6 h-6 text-zinc-600 dark:text-zinc-400 animate-spin mx-auto mb-2" />
        <p className="text-xs text-zinc-400">载入配置中…</p>
      </div>
    );
  }

  return (
    <div className="space-y-3 animate-in fade-in duration-150">
      {/* 1. Agent 核心角色 3 宫格控制台 */}
      <AgentTiersHero />

      {/* 2. 模型统一路由表 (集成主备决策、实时路由试算 Trace、别名抽屉与未知兜底) */}
      <ModelRoutingCard />
    </div>
  );
}
