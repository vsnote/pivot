import { useEffect, useMemo, useState } from 'react';
import { useShallow } from 'zustand/react/shallow';
import {
  ArrowRight,
  ChevronDown,
  Plus,
  RotateCcw,
  Search,
  X,
} from 'lucide-react';
import { useAppStore } from '@/store/useAppStore';
import { ProviderAvatar } from '@/components/common/BrandIcons';
import { panelClass } from '@/components/common/panelStyles';
import { AGENT_TIER_IDS } from '@/components/providers/agentTiers';
import { ModelPickerPopover, type ModelPickerTarget } from './ModelPickerPopover';
import type { Provider, RouteExplanation, RouteRule } from '@/types';

interface ModelRoutingItem {
  modelName: string;
  availableProviders: Provider[];
  primaryProviderId: string;
  fallbackProviderIds: string[];
  hasCustomOverride: boolean;
  ruleId?: string;
}

type FilterTab = 'all' | 'conflicts' | 'custom';

export function ModelRoutingCard() {
  const { routes, providers, saveRoutes, saveProvider, explainRoute } = useAppStore(
    useShallow((state) => ({
      routes: state.routes,
      providers: state.providers,
      saveRoutes: state.saveRoutes,
      saveProvider: state.saveProvider,
      explainRoute: state.explainRoute,
    })),
  );

  const [search, setSearch] = useState('');
  const [filterTab, setFilterTab] = useState<FilterTab>('all');
  const [addingFallbackFor, setAddingFallbackFor] = useState<string | null>(null);

  // 默认模型选择器状态
  const [isPickingDefault, setIsPickingDefault] = useState(false);
  const [defaultPickerAnchor, setDefaultPickerAnchor] = useState<DOMRect | null>(null);

  // 实时路由试算状态
  const [liveTrace, setLiveTrace] = useState<RouteExplanation | null>(null);

  const enabledProviders = useMemo(
    () =>
      [...providers]
        .filter((p) => p.enabled)
        .sort((a, b) => b.priority - a.priority || a.name.localeCompare(b.name)),
    [providers],
  );

  const defaultProvider = useMemo(
    () => enabledProviders.find((p) => p.isDefault),
    [enabledProviders],
  );

  // 所有可用模型选项供点选
  const allModelOptions: ModelPickerTarget[] = useMemo(() => {
    return enabledProviders.flatMap((provider) =>
      provider.models.map((model) => ({ provider, model })),
    );
  }, [enabledProviders]);

  // 当前生效的默认模型（服务商 + 具体模型）
  const currentDefaultTarget = useMemo((): ModelPickerTarget | null => {
    if (!defaultProvider) return null;
    const model = defaultProvider.defaultModel || defaultProvider.models[0];
    if (!model) return null;
    return { provider: defaultProvider, model };
  }, [defaultProvider]);

  const providerById = useMemo(
    () => new Map(providers.map((p) => [p.id, p] as const)),
    [providers],
  );

  // 汇聚所有启用渠道声明的模型，计算其首选渠道与回退链路
  const modelItems: ModelRoutingItem[] = useMemo(() => {
    const modelToProviders = new Map<string, Provider[]>();

    for (const provider of enabledProviders) {
      for (const model of provider.models) {
        const trimmed = model.trim();
        if (!trimmed) continue;
        const list = modelToProviders.get(trimmed) || [];
        list.push(provider);
        modelToProviders.set(trimmed, list);
      }
    }

    const items: ModelRoutingItem[] = [];

    for (const [modelName, availableProviders] of modelToProviders.entries()) {
      // 检查是否有针对该模型的精确路由规则（非 Agent 角色槽位，pattern 为纯模型名）
      const existingRule = routes.find(
        (r) =>
          !AGENT_TIER_IDS.includes(r.id) &&
          r.pattern.toLowerCase() === modelName.toLowerCase() &&
          r.enabled,
      );

      if (existingRule) {
        items.push({
          modelName,
          availableProviders,
          primaryProviderId: existingRule.targetProviderId,
          fallbackProviderIds: existingRule.fallbackProviderIds || [],
          hasCustomOverride: true,
          ruleId: existingRule.id,
        });
      } else {
        // 默认自然顺位：声明该模型的最高优先级服务商为首选
        const primary = availableProviders[0]?.id || '';
        items.push({
          modelName,
          availableProviders,
          primaryProviderId: primary,
          fallbackProviderIds: [],
          hasCustomOverride: false,
        });
      }
    }

    return items.sort((a, b) => a.modelName.localeCompare(b.modelName));
  }, [enabledProviders, routes]);

  // 计算过滤后的模型条目
  const filteredItems = useMemo(() => {
    let list = modelItems;

    if (filterTab === 'conflicts') {
      list = list.filter((item) => item.availableProviders.length > 1);
    } else if (filterTab === 'custom') {
      list = list.filter((item) => item.hasCustomOverride);
    }

    const q = search.trim().toLowerCase();
    if (q) {
      list = list.filter(
        (item) =>
          item.modelName.toLowerCase().includes(q) ||
          item.availableProviders.some((p) => p.name.toLowerCase().includes(q)),
      );
    }

    return list;
  }, [modelItems, filterTab, search]);

  const conflictsCount = useMemo(
    () => modelItems.filter((i) => i.availableProviders.length > 1).length,
    [modelItems],
  );

  const customCount = useMemo(
    () => modelItems.filter((i) => i.hasCustomOverride).length,
    [modelItems],
  );

  // 实时搜索即时试算防抖
  useEffect(() => {
    const q = search.trim();
    if (!q) {
      setLiveTrace(null);
      return;
    }

    const timer = setTimeout(() => {
      explainRoute(q)
        .then((res) => setLiveTrace(res))
        .catch(() => setLiveTrace(null));
    }, 150);

    return () => clearTimeout(timer);
  }, [search, explainRoute]);

  const persistModelRule = async (
    item: ModelRoutingItem,
    primaryId: string,
    fallbackIds: string[],
  ) => {
    const cleanRoutes = routes.filter((r) => r.id !== item.ruleId && r.pattern !== item.modelName);
    const defaultPrimaryId = item.availableProviders[0]?.id;
    const isBackToDefault = primaryId === defaultPrimaryId && fallbackIds.length === 0;

    if (isBackToDefault) {
      await saveRoutes(cleanRoutes);
      return;
    }

    const newRule: RouteRule = {
      id: item.ruleId || `model-rule-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 6)}`,
      pattern: item.modelName,
      targetProviderId: primaryId,
      targetModel: item.modelName,
      fallbackProviderIds: fallbackIds,
      enabled: true,
      priority: 400,
    };

    await saveRoutes([...cleanRoutes, newRule]);
  };

  // 更新某模型的首选服务商
  const handleSetPrimary = async (item: ModelRoutingItem, newPrimaryId: string) => {
    if (newPrimaryId === item.primaryProviderId) return;
    const fallbacks = item.fallbackProviderIds.filter((id) => id !== newPrimaryId);
    await persistModelRule(item, newPrimaryId, fallbacks);
  };

  // 添加备用回退渠道
  const handleAddFallback = async (item: ModelRoutingItem, fallbackId: string) => {
    setAddingFallbackFor(null);
    if (!fallbackId || item.fallbackProviderIds.includes(fallbackId)) return;
    await persistModelRule(item, item.primaryProviderId, [...item.fallbackProviderIds, fallbackId]);
  };

  // 移除备用回退渠道
  const handleRemoveFallback = async (item: ModelRoutingItem, fallbackId: string) => {
    const fallbacks = item.fallbackProviderIds.filter((id) => id !== fallbackId);
    await persistModelRule(item, item.primaryProviderId, fallbacks);
  };

  // 恢复系统默认
  const handleResetToDefault = async (item: ModelRoutingItem) => {
    if (!item.hasCustomOverride) return;
    const cleanRoutes = routes.filter((r) => r.id !== item.ruleId && r.pattern !== item.modelName);
    await saveRoutes(cleanRoutes);
  };

  // 选择默认承接模型（具体服务商 + 具体模型）
  const handleSelectDefaultModel = async (target: ModelPickerTarget) => {
    setIsPickingDefault(false);
    setDefaultPickerAnchor(null);
    try {
      await saveProvider({
        ...target.provider,
        isDefault: true,
        defaultModel: target.model,
      });
    } catch {
      // 失败由全局错误条展示
    }
  };

  // 清除默认模型（未知模型将报错拒绝）
  const handleClearDefaultModel = async () => {
    setIsPickingDefault(false);
    setDefaultPickerAnchor(null);
    if (!defaultProvider) return;
    try {
      await saveProvider({
        ...defaultProvider,
        isDefault: false,
      });
    } catch {
      // 失败由全局错误条展示
    }
  };

  // 提取 liveTrace 命中的层级说明
  const liveTraceMatchedStep = useMemo(() => {
    if (!liveTrace) return null;
    return liveTrace.steps.find((s) => s.matched);
  }, [liveTrace]);

  return (
    <section className={panelClass} aria-label="模型统一路由表">
      {/* 顶栏：标题 + 筛选 Filter Chips + 搜索框 */}
      <div className="px-4 py-2.5 border-b border-border flex flex-col sm:flex-row sm:items-center justify-between gap-2.5">
        <div className="flex items-center gap-3 min-w-0">
          <span className="text-xs font-semibold text-zinc-700 dark:text-zinc-200 tracking-wide shrink-0">
            模型统一路由表
          </span>

          {/* 快速过滤 Chips */}
          <div className="flex items-center gap-1 bg-muted/40 p-0.5 rounded-[5px] text-[10px]">
            <button
              type="button"
              onClick={() => setFilterTab('all')}
              className={`px-2 py-0.5 rounded-[3px] transition-colors cursor-pointer ${
                filterTab === 'all'
                  ? 'bg-background text-foreground shadow-2xs font-medium'
                  : 'text-muted-foreground hover:text-foreground'
              }`}
            >
              全部 ({modelItems.length})
            </button>
            <button
              type="button"
              onClick={() => setFilterTab('conflicts')}
              className={`flex items-center gap-1 px-2 py-0.5 rounded-[3px] transition-colors cursor-pointer ${
                filterTab === 'conflicts'
                  ? 'bg-background text-foreground shadow-2xs font-medium'
                  : conflictsCount > 0
                    ? 'text-amber-700 dark:text-amber-400 font-medium hover:text-foreground'
                    : 'text-muted-foreground hover:text-foreground'
              }`}
            >
              <span>多渠道重合 ({conflictsCount})</span>
              {conflictsCount > 0 && filterTab !== 'conflicts' && (
                <span className="w-1.5 h-1.5 rounded-full bg-amber-500 shrink-0" />
              )}
            </button>
            {customCount > 0 && (
              <button
                type="button"
                onClick={() => setFilterTab('custom')}
                className={`px-2 py-0.5 rounded-[3px] transition-colors cursor-pointer ${
                  filterTab === 'custom'
                    ? 'bg-background text-foreground shadow-2xs font-medium'
                    : 'text-muted-foreground hover:text-foreground'
                }`}
              >
                已定制 ({customCount})
              </button>
            )}
          </div>
        </div>

        {/* 搜索框（支持实时试算） */}
        <div className="relative w-full sm:w-56 shrink-0">
          <Search className="w-3 h-3 text-zinc-400 pointer-events-none absolute left-2 top-1/2 -translate-y-1/2" />
          <input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="搜索或输入模型测试路由..."
            className="h-6 w-full pl-6 pr-2 font-mono text-[11px] rounded-[5px] border border-input bg-background/60 text-foreground placeholder:text-muted-foreground/60 outline-none focus:ring-1 focus:ring-ring"
          />
        </div>
      </div>

      {/* 实时路由试算反馈条 (Live Trace Feedback) */}
      {liveTrace && (
        <div className="px-4 py-2 bg-accent/40 border-b border-border flex items-center justify-between text-xs animate-in fade-in duration-150">
          <div className="flex items-center gap-2 min-w-0 flex-1">
            <span className="text-[10px] uppercase font-mono px-1.5 py-0.5 rounded bg-muted text-muted-foreground font-semibold shrink-0">
              路由试算
            </span>
            <span className="font-mono text-[11px] font-medium text-foreground truncate max-w-[140px]">
              {liveTrace.incoming}
            </span>
            <ArrowRight className="w-3 h-3 text-muted-foreground shrink-0" />
            {liveTrace.plan ? (
              <div className="flex items-center gap-1.5 min-w-0 truncate">
                <span
                  className={`text-[10px] px-1.5 py-0.5 rounded font-medium shrink-0 ${
                    liveTraceMatchedStep?.layer === 'role'
                      ? 'bg-blue-500/10 text-blue-600 dark:text-blue-400'
                      : liveTraceMatchedStep?.layer === 'exact'
                        ? 'bg-emerald-500/10 text-emerald-600 dark:text-emerald-400'
                        : liveTraceMatchedStep?.layer === 'prefix'
                          ? 'bg-purple-500/10 text-purple-600 dark:text-purple-400'
                          : 'bg-zinc-500/10 text-zinc-600 dark:text-zinc-400'
                  }`}
                >
                  {liveTraceMatchedStep?.layer === 'role'
                    ? '命中 Agent 角色'
                    : liveTraceMatchedStep?.layer === 'exact'
                      ? '命中已知模型'
                      : liveTraceMatchedStep?.layer === 'prefix'
                        ? '走渠道前缀路由'
                        : '命中默认模型'}
                </span>

                <div className="flex items-center gap-1 min-w-0">
                  <ProviderAvatar
                    name={
                      providerById.get(liveTrace.plan.primary.providerId)?.name ||
                      liveTrace.plan.primary.providerId
                    }
                    sizeClass="w-3.5 h-3.5"
                    dense
                  />
                  <span className="font-medium text-[11px] text-foreground">
                    {providerById.get(liveTrace.plan.primary.providerId)?.name ||
                      liveTrace.plan.primary.providerId}
                  </span>
                  <span className="text-zinc-300 dark:text-zinc-600 select-none">/</span>
                  <span className="font-mono text-[11px] text-foreground">
                    {liveTrace.plan.primary.model}
                  </span>
                </div>

                {liveTrace.plan.fallbacks.length > 0 && (
                  <span className="text-[10px] text-muted-foreground shrink-0 hidden sm:inline">
                    (备用:{' '}
                    {liveTrace.plan.fallbacks
                      .map((f) => providerById.get(f.providerId)?.name || f.providerId)
                      .join(', ')}
                    )
                  </span>
                )}
              </div>
            ) : (
              <span className="text-[11px] text-rose-500 font-medium truncate">
                无可用渠道 · {liveTrace.error || '请求将被拒绝'}
              </span>
            )}
          </div>

          <button
            type="button"
            onClick={() => {
              setSearch('');
              setLiveTrace(null);
            }}
            className="text-[10px] text-muted-foreground hover:text-foreground shrink-0 ml-2 cursor-pointer"
          >
            清空
          </button>
        </div>
      )}

      {/* 模型统一路由表内容 */}
      {modelItems.length === 0 ? (
        <div className="px-4 py-8 text-center text-xs text-muted-foreground space-y-1">
          <p>暂无可用模型</p>
          <p className="text-[11px] text-zinc-400 dark:text-zinc-500">
            请先在「服务商」页添加并启用服务商渠道。
          </p>
        </div>
      ) : filteredItems.length === 0 ? (
        <div className="px-4 py-8 text-center text-xs text-muted-foreground">
          未找到匹配「{search}」的模型
        </div>
      ) : (
        <div>
          {/* 表头：严格的三列网格对齐 */}
          <div className="grid grid-cols-12 gap-3 px-4 py-1.5 border-b border-border/60 text-[10px] font-medium text-zinc-400 dark:text-zinc-500 uppercase tracking-wider bg-muted/15 select-none">
            <div className="col-span-5 sm:col-span-4">模型名称</div>
            <div className="col-span-4 sm:col-span-4">首选渠道 (PRIMARY)</div>
            <div className="col-span-3 sm:col-span-4">备用回退 (FALLBACK)</div>
          </div>

          {/* 表格内容行 */}
          <div className="divide-y divide-border">
            {filteredItems.map((item) => {
              const isAddingFallback = addingFallbackFor === item.modelName;
              const hasMultipleProviders = item.availableProviders.length > 1;

              // 备用回退渠道只应从真正声明/拥有该模型的其他可用渠道中选择
              const candidateFallbacks = item.availableProviders.filter(
                (p) => p.id !== item.primaryProviderId && !item.fallbackProviderIds.includes(p.id),
              );

              const primaryProvider = providerById.get(item.primaryProviderId);

              return (
                <div
                  key={item.modelName}
                  className={`group grid grid-cols-12 gap-3 items-center px-4 py-2 text-xs transition-colors hover:bg-accent/40 ${
                    hasMultipleProviders
                      ? 'bg-amber-500/[0.015] dark:bg-amber-400/[0.015]'
                      : ''
                  }`}
                >
                  {/* 第 1 列：模型名称 + 供应标记 */}
                  <div className="col-span-5 sm:col-span-4 flex items-center gap-1.5 min-w-0 pr-1">
                    <span
                      className="font-mono text-[11px] font-medium text-zinc-900 dark:text-zinc-100 truncate"
                      title={item.modelName}
                    >
                      {item.modelName}
                    </span>

                    {hasMultipleProviders && (
                      <span
                        className="shrink-0 text-[9px] px-1.5 py-0.5 rounded font-sans bg-amber-500/10 dark:bg-amber-400/10 text-amber-700 dark:text-amber-400 border border-amber-500/20 font-medium"
                        title={`支持该模型的渠道: ${item.availableProviders.map((p) => p.name).join(', ')}`}
                      >
                        {item.availableProviders.length} 渠道支持
                      </span>
                    )}

                    {item.hasCustomOverride && (
                      <button
                        type="button"
                        onClick={() => void handleResetToDefault(item)}
                        title="当前为手动定制，点击恢复为默认顺位"
                        className="shrink-0 text-[10px] text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-200 transition-colors ml-auto sm:ml-0 cursor-pointer"
                      >
                        <RotateCcw className="w-2.5 h-2.5" />
                      </button>
                    )}
                  </div>

                  {/* 第 2 列：首选渠道（统一采用 Avatar + 厂商名 + 内嵌箭头） */}
                  <div className="col-span-4 sm:col-span-4 flex items-center gap-1 min-w-0">
                    {hasMultipleProviders ? (
                      <div className="relative inline-flex items-center min-w-0 max-w-[170px] w-full group/sel">
                        <div
                          className="flex items-center justify-between gap-1.5 h-6 pl-1.5 pr-2 rounded-[5px] border border-input bg-background/90 group-hover/sel:bg-accent/60 transition-colors shadow-2xs text-[11px] font-medium text-zinc-800 dark:text-zinc-200 min-w-0 w-full"
                          title="点击切换该模型的首选承接渠道"
                        >
                          <div className="flex items-center gap-1.5 min-w-0 flex-1">
                            <ProviderAvatar
                              name={primaryProvider?.name || item.primaryProviderId}
                              sizeClass="w-3.5 h-3.5"
                              dense
                            />
                            <span className="truncate">
                              {primaryProvider?.name || item.primaryProviderId}
                            </span>
                          </div>
                          <ChevronDown className="w-3 h-3 text-muted-foreground group-hover/sel:text-foreground shrink-0 transition-colors" />
                        </div>

                        <select
                          value={item.primaryProviderId}
                          onChange={(e) => void handleSetPrimary(item, e.target.value)}
                          className="absolute inset-0 opacity-0 w-full h-full cursor-pointer"
                          title="选择该模型的首选承接渠道"
                        >
                          {item.availableProviders.map((p) => (
                            <option key={p.id} value={p.id}>
                              {p.name}
                            </option>
                          ))}
                        </select>
                      </div>
                    ) : (
                      <div className="flex items-center gap-1.5 min-w-0 max-w-[170px]">
                        <ProviderAvatar
                          name={primaryProvider?.name || item.primaryProviderId}
                          sizeClass="w-3.5 h-3.5"
                          dense
                        />
                        <span
                          className="text-[11px] font-medium text-zinc-700 dark:text-zinc-300 truncate"
                          title="唯一支持渠道"
                        >
                          {primaryProvider?.name || item.primaryProviderId}
                        </span>
                      </div>
                    )}
                  </div>

                  {/* 第 3 列：备用回退（竖向严格对齐） */}
                  <div className="col-span-3 sm:col-span-4 flex flex-wrap items-center gap-1.5 min-w-0">
                    {item.fallbackProviderIds.map((fId) => {
                      const pName = providerById.get(fId)?.name || fId;
                      return (
                        <span
                          key={fId}
                          className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded-[4px] bg-zinc-500/[0.05] dark:bg-white/[0.05] border border-border/70 text-[10px] text-zinc-700 dark:text-zinc-300"
                        >
                          <ArrowRight className="w-2.5 h-2.5 text-zinc-400 shrink-0" />
                          <ProviderAvatar name={pName} sizeClass="w-3 h-3" dense />
                          <span className="truncate max-w-[70px]">{pName}</span>
                          <button
                            type="button"
                            onClick={() => void handleRemoveFallback(item, fId)}
                            title="移除此备用回退"
                            className="text-zinc-400 hover:text-rose-500 transition-colors cursor-pointer"
                          >
                            <X className="w-2.5 h-2.5" />
                          </button>
                        </span>
                      );
                    })}

                    {/* 多渠道时支持添加备用；单渠道时展示静音降噪标签 */}
                    {candidateFallbacks.length > 0 ? (
                      isAddingFallback ? (
                        <div className="flex items-center gap-1">
                          <div className="relative inline-flex items-center">
                            <select
                              autoFocus
                              value=""
                              onChange={(e) => {
                                if (e.target.value) {
                                  void handleAddFallback(item, e.target.value);
                                }
                              }}
                              className="h-5 text-[10px] pl-1.5 pr-5 rounded-[4px] border border-input bg-background text-foreground outline-none appearance-none cursor-pointer"
                            >
                              <option value="">选择备用...</option>
                              {candidateFallbacks.map((cp) => (
                                <option key={cp.id} value={cp.id}>
                                  {cp.name}
                                </option>
                              ))}
                            </select>
                            <ChevronDown className="w-2.5 h-2.5 text-muted-foreground pointer-events-none absolute right-1.5 top-1/2 -translate-y-1/2 shrink-0" />
                          </div>
                          <button
                            type="button"
                            onClick={() => setAddingFallbackFor(null)}
                            className="text-zinc-400 hover:text-zinc-600 p-0.5 cursor-pointer"
                          >
                            <X className="w-3 h-3" />
                          </button>
                        </div>
                      ) : (
                        <button
                          type="button"
                          onClick={() => setAddingFallbackFor(item.modelName)}
                          title="配置故障转移回退渠道"
                          className="flex items-center gap-0.5 text-[10px] text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-100 border border-dashed border-border/80 hover:border-zinc-400 dark:hover:border-zinc-500 px-1.5 py-0.5 rounded-[4px] transition-colors cursor-pointer"
                        >
                          <Plus className="w-2.5 h-2.5" />
                          <span>备用</span>
                        </button>
                      )
                    ) : item.fallbackProviderIds.length === 0 ? (
                      <span className="text-[10px] text-zinc-400/70 dark:text-zinc-500/70 select-none">
                        单渠道独占
                      </span>
                    ) : null}
                  </div>
                </div>
              );
            })}
          </div>

          {/* 表底栏：默认模型 */}
          <div className="px-4 py-2.5 border-t border-border bg-muted/15 flex flex-col sm:flex-row sm:items-center justify-between gap-3">
            <div className="flex items-center gap-2 min-w-0">
              <span className="shrink-0 text-[10px] font-medium px-1.5 py-0.5 rounded bg-zinc-500/10 dark:bg-white/10 text-zinc-600 dark:text-zinc-300">
                默认
              </span>
              <div className="flex items-baseline gap-1.5 min-w-0">
                <span className="text-xs font-semibold text-zinc-800 dark:text-zinc-200 shrink-0">
                  默认模型
                </span>
                <span className="text-[11px] text-zinc-400 dark:text-zinc-500 truncate hidden md:inline">
                  — 客户端未指定模型或请求未知模型时自动由此承接；未指定时报错拒绝
                </span>
              </div>
            </div>

            <div className="shrink-0">
              <button
                type="button"
                onClick={(e) => {
                  setDefaultPickerAnchor(e.currentTarget.getBoundingClientRect());
                  setIsPickingDefault((prev) => !prev);
                }}
                className={`flex items-center justify-between gap-2 h-7 px-2.5 rounded-[5px] border transition-colors shadow-2xs text-[11px] font-medium cursor-pointer min-w-0 max-w-[260px] sm:w-auto ${
                  isPickingDefault
                    ? 'border-ring bg-accent/70 ring-1 ring-ring/30'
                    : 'border-input bg-background/90 hover:bg-accent/60 text-zinc-800 dark:text-zinc-200'
                }`}
                title="点击选择默认承接模型"
              >
                {currentDefaultTarget ? (
                  <div className="flex items-center gap-1.5 min-w-0">
                    <ProviderAvatar
                      name={currentDefaultTarget.provider.name}
                      sizeClass="w-3.5 h-3.5"
                      dense
                    />
                    <span className="text-muted-foreground truncate max-w-[80px]">
                      {currentDefaultTarget.provider.name}
                    </span>
                    <span className="text-zinc-300 dark:text-zinc-600 select-none">/</span>
                    <span className="font-mono text-foreground font-semibold truncate max-w-[130px]">
                      {currentDefaultTarget.model}
                    </span>
                  </div>
                ) : (
                  <span className="text-muted-foreground/70 font-normal">
                    未指定 (报错拒绝)
                  </span>
                )}
                <ChevronDown
                  className={`w-3 h-3 text-muted-foreground transition-transform shrink-0 ${
                    isPickingDefault ? 'rotate-180 text-foreground' : ''
                  }`}
                />
              </button>
            </div>
          </div>
        </div>
      )}

      {/* 默认模型选择 Popover */}
      <ModelPickerPopover
        anchor={defaultPickerAnchor}
        isOpen={isPickingDefault}
        onClose={() => {
          setIsPickingDefault(false);
          setDefaultPickerAnchor(null);
        }}
        options={allModelOptions}
        onSelect={(target) => void handleSelectDefaultModel(target)}
        onClear={() => void handleClearDefaultModel()}
        clearLabel="未指定 (请求未知模型将报错拒绝)"
        placeholder="搜索默认模型或渠道..."
      />
    </section>
  );
}
