import { useEffect, useMemo, useState } from 'react';
import type { MouseEvent } from 'react';
import { useShallow } from 'zustand/react/shallow';
import { ChevronDown } from 'lucide-react';
import { useAppStore } from '@/store/useAppStore';
import { ProviderAvatar } from '@/components/common/BrandIcons';
import { AGENT_TIERS, AGENT_TIER_IDS, AGENT_TIER_KEYS, type AgentTierKey } from '@/components/providers/agentTiers';
import { ModelPickerPopover, type ModelPickerTarget } from './ModelPickerPopover';
import type { Provider } from '@/types';

export function AgentTiersHero() {
  const { routes, providers, saveRoutes } = useAppStore(
    useShallow((state) => ({
      routes: state.routes,
      providers: state.providers,
      saveRoutes: state.saveRoutes,
    })),
  );

  const enabledProviders = useMemo(
    () => providers.filter((p) => p.enabled),
    [providers],
  );

  const [activeTierKey, setActiveTierKey] = useState<AgentTierKey | null>(null);
  const [anchor, setAnchor] = useState<DOMRect | null>(null);

  // 自动迁移旧槽位 pattern，防止宽泛正则劫持无关模型
  useEffect(() => {
    let hasOutdated = false;
    const migrated = routes.map((rule) => {
      if (AGENT_TIER_IDS.includes(rule.id)) {
        const tier = Object.values(AGENT_TIERS).find((t) => t.id === rule.id);
        if (tier && rule.pattern !== tier.pattern) {
          hasOutdated = true;
          return { ...rule, pattern: tier.pattern, description: tier.description };
        }
      }
      return rule;
    });
    if (hasOutdated) {
      void saveRoutes(migrated);
    }
  }, [routes, saveRoutes]);

  // 所有可用模型选项（供 Popover 点选）
  const pickerOptions: ModelPickerTarget[] = useMemo(() => {
    return enabledProviders.flatMap((provider) =>
      provider.models.map((model) => ({ provider, model })),
    );
  }, [enabledProviders]);

  const bindingForTier = (tierId: string): { provider: Provider; model: string } | null => {
    const route = routes.find((r) => r.id === tierId && r.enabled);
    const model = route?.targetModel;
    if (!route || !model) return null;
    const provider = enabledProviders.find(
      (p) => p.id === route.targetProviderId && p.models.includes(model),
    );
    return provider ? { provider, model } : null;
  };

  const handleOpenPicker = (tierKey: AgentTierKey, e: MouseEvent<HTMLButtonElement>) => {
    if (activeTierKey === tierKey) {
      handleClosePicker();
      return;
    }
    setAnchor(e.currentTarget.getBoundingClientRect());
    setActiveTierKey(tierKey);
  };

  const handleClosePicker = () => {
    setActiveTierKey(null);
    setAnchor(null);
  };

  const handleSelectTarget = async (target: ModelPickerTarget | null) => {
    if (!activeTierKey) return;
    const tier = AGENT_TIERS[activeTierKey];
    let next = routes.filter((r) => r.id !== tier.id);

    if (target) {
      next = next.filter(
        (r) =>
          !(
            AGENT_TIER_IDS.includes(r.id) &&
            r.targetProviderId === target.provider.id &&
            r.targetModel === target.model
          ),
      );
      next.unshift({
        id: tier.id,
        pattern: tier.pattern,
        targetProviderId: target.provider.id,
        targetModel: target.model,
        fallbackProviderIds: [],
        enabled: true,
        priority: 500,
        description: tier.description,
      });
    }

    await saveRoutes(next);
  };

  return (
    <section aria-label="Agent 核心角色槽位">
      <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
        {AGENT_TIER_KEYS.map((key) => {
          const tier = AGENT_TIERS[key];
          const Icon = tier.icon;
          const binding = bindingForTier(tier.id);
          const isOpen = activeTierKey === key;

          return (
            <button
              key={key}
              type="button"
              onClick={(e) => handleOpenPicker(key, e)}
              className={`group flex flex-col justify-between p-3 rounded-lg border text-left transition-all cursor-pointer shadow-2xs ${
                isOpen
                  ? 'border-ring bg-accent/50 ring-1 ring-ring/30'
                  : 'border-border bg-card/60 hover:bg-card hover:border-zinc-300 dark:hover:border-zinc-700'
              }`}
            >
              {/* 卡片头部：图标 + 角色名称 + 英文标识 */}
              <div className="flex items-center justify-between w-full mb-2">
                <div className="flex items-center gap-2 min-w-0">
                  <div className="flex items-center justify-center w-6 h-6 rounded-md bg-muted/60 text-muted-foreground group-hover:text-foreground shrink-0 transition-colors">
                    <Icon className={`w-3.5 h-3.5 ${tier.iconClass}`} />
                  </div>
                  <div className="min-w-0">
                    <span className="text-xs font-semibold text-zinc-900 dark:text-zinc-100 tracking-tight">
                      {tier.label}
                    </span>
                  </div>
                </div>

                <span className="text-[10px] font-mono font-medium px-1.5 py-0.5 rounded-[4px] bg-muted/50 text-muted-foreground group-hover:text-foreground transition-colors shrink-0">
                  {key.toUpperCase()}
                </span>
              </div>

              {/* 卡片主体：绑定目标 */}
              <div className="flex items-center justify-between gap-2 w-full pt-1 border-t border-border/50">
                {binding ? (
                  <div className="flex items-center gap-1.5 min-w-0 flex-1">
                    <ProviderAvatar name={binding.provider.name} sizeClass="w-3.5 h-3.5" dense />
                    <span className="text-[11px] text-muted-foreground truncate shrink-0 max-w-[70px]">
                      {binding.provider.name}
                    </span>
                    <span className="text-zinc-300 dark:text-zinc-600 shrink-0 select-none">/</span>
                    <span
                      className="font-mono text-[11px] font-medium text-foreground truncate px-1.5 py-0.5 rounded-[4px] bg-muted/70 group-hover:bg-muted transition-colors"
                      title={binding.model}
                    >
                      {binding.model}
                    </span>
                  </div>
                ) : (
                  <span className="text-[11px] text-muted-foreground/60 italic">
                    点击指定模型
                  </span>
                )}

                <ChevronDown
                  className={`w-3.5 h-3.5 text-muted-foreground/60 group-hover:text-foreground shrink-0 transition-transform ${
                    isOpen ? 'rotate-180 text-foreground' : ''
                  }`}
                />
              </div>
            </button>
          );
        })}
      </div>

      {/* 弹窗选择器 */}
      <ModelPickerPopover
        anchor={anchor}
        isOpen={activeTierKey !== null}
        onClose={handleClosePicker}
        options={pickerOptions}
        onSelect={(target) => void handleSelectTarget(target)}
        onClear={() => void handleSelectTarget(null)}
        clearLabel="清除绑定 (不拦截)"
        placeholder="搜索目标模型或渠道..."
      />
    </section>
  );
}
