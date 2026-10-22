import { LoaderCircle } from 'lucide-react';
import { useAppStore } from '@/store/useAppStore';
import { AgentTiersHero } from './AgentTiersHero';

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
    <div className="animate-in fade-in duration-150">
      <AgentTiersHero />
    </div>
  );
}
