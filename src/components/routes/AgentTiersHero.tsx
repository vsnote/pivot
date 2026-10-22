import { useEffect, useMemo, useState } from 'react';
import type { MouseEvent } from 'react';
import { useShallow } from 'zustand/react/shallow';
import { Brain, ChevronDown, Code2, Compass, Zap } from 'lucide-react';
import { useAppStore } from '@/store/useAppStore';
import { ProviderAvatar } from '@/components/common/BrandIcons';
import { panelClass } from '@/components/common/panelStyles';
import { AGENT_TIERS, AGENT_TIER_IDS, type AgentTierKey } from '@/components/providers/agentTiers';
import { ModelPickerPopover, type ModelPickerTarget } from './ModelPickerPopover';
import type { Provider } from '@/types';

type SlotKey = AgentTierKey | 'default';

interface SlotDefinition {
  key: SlotKey;
  label: string;
  badge: string;
  description: string;
  icon: typeof Code2;
  iconClass: string;
}

const SLOTS: SlotDefinition[] = [
  {
    key: 'default',
    label: '默认模型',
    badge: 'DEFAULT',
    description: '未指定模型或未知请求时的通用承接模型',
    icon: Compass,
    iconClass: 'text-emerald-600 dark:text-emerald-400',
  },
  {
    key: 'main',
    label: '主力模型',
    badge: 'MAIN',
    description: '核心工作负载与复杂编码 (pivot/main)',
    icon: Code2,
    iconClass: 'text-blue-600 dark:text-blue-400',
  },
  {
    key: 'fast',
    label: '快速模型',
    badge: 'FAST',
    description: '低延迟子任务与轻量编辑 (pivot/fast)',
    icon: Zap,
    iconClass: 'text-amber-600 dark:text-amber-400',
  },
  {
    key: 'reasoning',
    label: '推理模型',
    badge: 'REASONING',
    description: '深度逻辑与思考规划 (pivot/reasoning)',
    icon: Brain,
    iconClass: 'text-purple-600 dark:text-purple-400',
  },
];

export function AgentTiersHero() {
  const { routes, providers, saveRoutes, saveProvider } = useAppStore(
    useShallow((state) => ({
      routes: state.routes,
      providers: state.providers,
      saveRoutes: state.saveRoutes,
      saveProvider: state.saveProvider,
    })),
  );

  const enabledProviders = useMemo(
    () =>
      [...providers]
        .filter((p) => p.enabled)
        .sort((a, b) => b.priority - a.priority || a.name.localeCompare(b.name)),
    [providers],
  );

  const [activeSlotKey, setActiveSlotKey] = useState<SlotKey | null>(null);
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

  // 当存在已启用服务商但尚未设置默认模型时，自动默认绑定优先级最高渠道的第 1 个模型
  const defaultProvider = useMemo(
    () => enabledProviders.find((p) => p.isDefault),
    [enabledProviders],
  );

  useEffect(() => {
    if (enabledProviders.length === 0) return;
    const hasDefault = enabledProviders.some((p) => p.isDefault && p.defaultModel);
    if (!hasDefault) {
      const topProvider = enabledProviders[0];
      const firstModel = topProvider?.models[0];
      if (topProvider && firstModel) {
        void saveProvider({
          ...topProvider,
          isDefault: true,
          defaultModel: firstModel,
        });
      }
    }
  }, [enabledProviders, saveProvider]);

  // 所有可用模型选项（供 Popover 点选）
  const pickerOptions: ModelPickerTarget[] = useMemo(() => {
    return enabledProviders.flatMap((provider) =>
      provider.models.map((model) => ({ provider, model })),
    );
  }, [enabledProviders]);

  // 获取某个 Agent 槽位的当前绑定
  const bindingForTier = (tierId: string): { provider: Provider; model: string } | null => {
    const route = routes.find((r) => r.id === tierId && r.enabled);
    const model = route?.targetModel;
    if (!route || !model) return null;
    const provider = enabledProviders.find(
      (p) => p.id === route.targetProviderId && p.models.includes(model),
    );
    return provider ? { provider, model } : null;
  };

  // 获取默认模型的当前绑定
  const defaultBinding = useMemo((): { provider: Provider; model: string } | null => {
    if (!defaultProvider) return null;
    const model = defaultProvider.defaultModel || defaultProvider.models[0];
    if (!model) return null;
    return { provider: defaultProvider, model };
  }, [defaultProvider]);

  const handleOpenPicker = (slotKey: SlotKey, e: MouseEvent<HTMLDivElement | HTMLButtonElement>) => {
    if (activeSlotKey === slotKey) {
      handleClosePicker();
      return;
    }
    setAnchor(e.currentTarget.getBoundingClientRect());
    setActiveSlotKey(slotKey);
  };

  const handleClosePicker = () => {
    setActiveSlotKey(null);
    setAnchor(null);
  };

  const handleSelectTarget = async (target: ModelPickerTarget | null) => {
    if (!activeSlotKey) return;

    // 1. 处理默认模型槽位
    if (activeSlotKey === 'default') {
      handleClosePicker();
      if (target) {
        await saveProvider({
          ...target.provider,
          isDefault: true,
          defaultModel: target.model,
        });
      } else if (defaultProvider) {
        await saveProvider({
          ...defaultProvider,
          isDefault: false,
        });
      }
      return;
    }

    // 2. 处理 Agent 3 角色槽位
    const tier = AGENT_TIERS[activeSlotKey];
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
    <section className={panelClass} aria-label="模型与角色调度">
      {/* 4 核心调度槽位列表（纯粹无头列表） */}
      <div className="divide-y divide-border">
        {SLOTS.map((slot) => {
          const Icon = slot.icon;
          const binding =
            slot.key === 'default'
              ? defaultBinding
              : bindingForTier(AGENT_TIERS[slot.key].id);
          const isOpen = activeSlotKey === slot.key;

          return (
            <div
              key={slot.key}
              onClick={(e) => handleOpenPicker(slot.key, e)}
              className={`group flex items-center justify-between gap-3 px-4 min-h-[44px] py-2 transition-colors hover:bg-accent/40 cursor-pointer ${
                isOpen ? 'bg-accent/50' : ''
              }`}
            >
              {/* 左侧：图标 + 角色名称 + 说明说明 */}
              <div className="flex items-center gap-2.5 min-w-0">
                <div className="flex items-center justify-center w-6 h-6 rounded-md bg-muted/60 text-muted-foreground group-hover:text-foreground shrink-0 transition-colors">
                  <Icon className={`w-3.5 h-3.5 ${slot.iconClass}`} />
                </div>
                <div className="flex items-baseline gap-2 min-w-0">
                  <span className="text-xs font-semibold text-zinc-800 dark:text-zinc-200 shrink-0">
                    {slot.label}
                  </span>
                  <span className="text-[10px] text-muted-foreground/80 truncate hidden sm:inline">
                    · {slot.description}
                  </span>
                </div>
              </div>

              {/* 右侧：当前绑定模型卡片 + 下拉箭头 */}
              <div className="flex items-center gap-2 shrink-0">
                {binding ? (
                  <div className="flex items-center gap-1.5 h-6 px-2 rounded-[5px] bg-background/90 border border-border/80 group-hover:border-zinc-300 dark:group-hover:border-zinc-600 transition-colors shadow-2xs">
                    <ProviderAvatar name={binding.provider.name} sizeClass="w-3.5 h-3.5" dense />
                    <span className="text-[11px] font-medium text-muted-foreground truncate max-w-[80px]">
                      {binding.provider.name}
                    </span>
                    <span className="text-zinc-300 dark:text-zinc-600 select-none">/</span>
                    <span
                      className="font-mono text-[11px] font-semibold text-foreground truncate max-w-[130px] sm:max-w-[180px]"
                      title={binding.model}
                    >
                      {binding.model}
                    </span>
                  </div>
                ) : (
                  <div className="flex items-center h-6 px-2 text-[11px] text-muted-foreground/70 italic">
                    {slot.key === 'default' ? '未指定 (报错拒绝)' : '未指定'}
                  </div>
                )}

                <ChevronDown
                  className={`w-3.5 h-3.5 text-muted-foreground/70 group-hover:text-foreground transition-transform shrink-0 ${
                    isOpen ? 'rotate-180 text-foreground' : ''
                  }`}
                />
              </div>
            </div>
          );
        })}
      </div>

      {/* 弹窗选择器（4 个槽位共用） */}
      <ModelPickerPopover
        anchor={anchor}
        isOpen={activeSlotKey !== null}
        onClose={handleClosePicker}
        options={pickerOptions}
        onSelect={(target) => void handleSelectTarget(target)}
        onClear={() => void handleSelectTarget(null)}
        clearLabel={
          activeSlotKey === 'default'
            ? '未指定 (请求未知模型将报错拒绝)'
            : '清除绑定 (不拦截)'
        }
        placeholder={
          activeSlotKey === 'default'
            ? '搜索默认模型或渠道...'
            : '搜索目标模型或渠道...'
        }
      />
    </section>
  );
}
