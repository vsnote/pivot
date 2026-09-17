import React, { useState } from 'react';
import { useAppStore } from '@/store/useAppStore';
import { Switch } from '@/components/common/Switch';
import {
  Plus,
  ArrowRight,
  Edit2,
  Trash2,
  Search,
  CheckCircle2,
  Network,
} from 'lucide-react';

export const RouteMatrix: React.FC = () => {
  const { routes, providers, openRouteModal, saveRoutes } = useAppStore();

  const [testModelInput, setTestModelInput] = useState('claude-3-7-sonnet-20250219');

  const handleDelete = async (id: string) => {
    const route = routes.find((item) => item.id === id);
    if (!window.confirm(`确定删除路由“${route?.pattern ?? id}”吗？此操作无法撤销。`)) return;
    const updated = routes.filter((r) => r.id !== id);
    await saveRoutes(updated);
  };

  const handleToggle = async (id: string, enabled: boolean) => {
    const updated = routes.map((r) => (r.id === id ? { ...r, enabled } : r));
    await saveRoutes(updated);
  };

  const getProviderName = (providerId: string) => {
    return providers.find((p) => p.id === providerId)?.name || providerId;
  };

  // Simulate routing for the tester
  const simulateRoute = () => {
    const input = testModelInput.trim();
    if (!input) return null;

    // Check prefix
    if (input.includes('/')) {
      const [prefix, sub] = input.split('/');
      const p = providers.find((pr) => pr.id.toLowerCase() === prefix.toLowerCase());
      if (p) {
        return {
          matchedRule: '前缀路由',
          primary: p.name,
          primaryModel: sub,
          fallbacks: providers.filter((pr) => pr.id !== p.id && pr.enabled).map((pr) => pr.name),
        };
      }
    }

    // Check sorted rules
    const sorted = [...routes].sort((a, b) => b.priority - a.priority);
    for (const r of sorted) {
      if (!r.enabled) continue;
      if (r.pattern === '*' || input.toLowerCase().includes(r.pattern.replace('*', '').toLowerCase())) {
        const p = providers.find((pr) => pr.id === r.targetProviderId);
        return {
          matchedRule: `命中“${r.pattern}”（优先级 ${r.priority}）`,
          primary: p ? p.name : r.targetProviderId,
          primaryModel: r.targetModel || input,
          fallbacks: r.fallbackProviderIds.map((id: string) => getProviderName(id)),
        };
      }
    }

    return null;
  };

  const simulation = simulateRoute();

  return (
    <div className="space-y-4 pb-8 animate-in fade-in duration-150">
      {/* 顶部操作栏：模型匹配测试与添加路由 */}
      <div className="p-3.5 rounded-xl border border-gray-200/80 dark:border-white/10 bg-white dark:bg-[#1a1b1e] shadow-xs">
        <div className="flex flex-col sm:flex-row items-stretch sm:items-center gap-3">
          <div className="relative flex-1">
            <Search className="w-3.5 h-3.5 absolute left-3 top-1/2 -translate-y-1/2 text-gray-400" />
            <input
              type="text"
              value={testModelInput}
              onChange={(e) => setTestModelInput(e.target.value)}
              placeholder="输入模型名称测试路由，例如 claude-3-7-sonnet、gpt-4o"
              className="w-full pl-8.5 pr-4 py-1.5 text-xs font-mono rounded-lg border border-gray-200 dark:border-zinc-800 bg-gray-50/60 dark:bg-zinc-900/60 text-gray-900 dark:text-gray-100 placeholder-gray-400 focus:outline-none focus:ring-1 focus:ring-indigo-500/30"
            />
          </div>

          <button
            onClick={() => openRouteModal(null)}
            className="flex items-center justify-center gap-1.5 px-3 h-7 text-xs font-medium text-white bg-indigo-600 rounded-lg hover:bg-indigo-700 transition-colors shadow-xs shrink-0 cursor-pointer"
          >
            <Plus className="w-3.5 h-3.5" />
            <span>添加路由</span>
          </button>
        </div>

        {simulation && (
          <div className="mt-2.5 flex items-center gap-2 text-xs bg-indigo-50/60 dark:bg-indigo-950/30 p-2 rounded-lg border border-indigo-100 dark:border-indigo-900/40 font-mono">
            <CheckCircle2 className="w-3.5 h-3.5 text-emerald-500 shrink-0" />
            <div className="truncate text-[11.5px]">
              <span className="text-gray-500 dark:text-zinc-400">路由至：</span>
              <span className="font-semibold text-indigo-600 dark:text-indigo-400">
                {simulation.primary}
              </span>
              <span className="text-gray-500 dark:text-zinc-400">（{simulation.primaryModel}）</span>
              {simulation.fallbacks.length > 0 && (
                <span className="text-gray-400 dark:text-zinc-500">
                  {' '}
                  &rarr; 备用：{simulation.fallbacks.join('、')}
                </span>
              )}
            </div>
          </div>
        )}
      </div>


      {/* 规则卡片行列表 */}
      {routes.length === 0 ? (
        <div className="text-center py-16 border border-dashed border-gray-200 dark:border-gray-800 rounded-xl bg-white/50 dark:bg-[#1a1b1e]/50">
          <Network className="w-10 h-10 text-gray-400 mx-auto mb-2.5" />
          <h3 className="text-sm font-semibold text-gray-900 dark:text-gray-100">
            暂无路由
          </h3>
          <p className="text-xs text-gray-500 dark:text-gray-400 mt-1 max-w-sm mx-auto">
            添加路由后，可按模型名称分发请求并设置备用服务商。
          </p>
          <div className="mt-4">
            <button
              onClick={() => openRouteModal(null)}
              className="inline-flex items-center gap-1.5 px-3.5 py-1.5 text-xs font-medium text-white bg-indigo-600 rounded-lg hover:bg-indigo-700 shadow-xs"
            >
              <Plus className="w-3.5 h-3.5" />
              <span>创建路由</span>
            </button>
          </div>
        </div>
      ) : (
        <div className="space-y-2">
          {routes.map((route) => {
            const primaryProv = providers.find((p) => p.id === route.targetProviderId);

            return (
              <div
                key={route.id}
                className="group rounded-xl border border-gray-200/80 dark:border-gray-800/80 bg-white dark:bg-[#1a1b1e] p-3 transition-all duration-150 flex flex-col sm:flex-row sm:items-center justify-between gap-3 hover:border-gray-300 dark:hover:border-gray-700 shadow-[0_1px_2px_rgba(0,0,0,0.03)]"
              >
                {/* 左侧：模式、指向目标、降级 */}
                <div className="min-w-0 flex-1 space-y-1.5">
                  <div className="flex items-center gap-2 flex-wrap text-xs">
                    {/* Pattern Badge */}
                    <span className="font-mono text-xs font-semibold px-2 py-0.5 rounded-md bg-indigo-50 dark:bg-indigo-950/60 text-indigo-700 dark:text-indigo-300 border border-indigo-200/60 dark:border-indigo-800/50">
                      {route.pattern}
                    </span>

                    <ArrowRight className="w-3.5 h-3.5 text-gray-400 shrink-0" />

                    {/* Target Provider */}
                    <div className="flex items-center gap-1.5 font-medium text-gray-900 dark:text-gray-100">
                      <span>{primaryProv ? primaryProv.name : route.targetProviderId}</span>
                      <span className="text-[10px] font-mono px-1.5 py-0.2 rounded bg-gray-100 dark:bg-gray-800 text-gray-500 uppercase border border-gray-200/60 dark:border-gray-700/60">
                        {primaryProv?.protocol || 'api'}
                      </span>
                    </div>

                    {/* Target Model Override */}
                    {route.targetModel ? (
                      <span className="font-mono text-[11px] px-1.5 py-0.5 rounded bg-violet-50 dark:bg-violet-950/50 text-violet-700 dark:text-violet-300 border border-violet-200/60 dark:border-violet-800/50">
                        &rarr; {route.targetModel}
                      </span>
                    ) : (
                      <span className="text-[11px] text-gray-400 italic">
                        (透传请求模型)
                      </span>
                    )}

                    {/* Priority Badge */}
                    <span className="ml-auto sm:ml-0 text-[10px] font-mono px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-500 border border-gray-200/60 dark:border-gray-700/60">
                      优先级：{route.priority}
                    </span>
                  </div>

                  {/* 降级与说明 */}
                  <div className="flex items-center gap-3 text-xs text-gray-500 dark:text-gray-400 flex-wrap">
                    {route.description && (
                      <span className="text-gray-500 dark:text-gray-400 text-xs">
                        {route.description}
                      </span>
                    )}

                    {route.fallbackProviderIds.length > 0 && (
                      <div className="flex items-center gap-1 text-[11px] font-mono text-gray-400">
                        <span>备用：</span>
                        {route.fallbackProviderIds.map((fbId, idx) => (
                          <React.Fragment key={fbId}>
                            {idx > 0 && <span>&rarr;</span>}
                            <span className="px-1.5 py-0.2 rounded bg-amber-50 dark:bg-amber-950/40 text-amber-700 dark:text-amber-300 border border-amber-200/50 dark:border-amber-800/40">
                              {getProviderName(fbId)}
                            </span>
                          </React.Fragment>
                        ))}
                      </div>
                    )}
                  </div>
                </div>

                {/* 右侧：开关与操作 */}
                <div className="flex items-center gap-2 shrink-0 self-end sm:self-center">
                  <div className="flex items-center gap-0.5">
                    <button
                      onClick={() => openRouteModal(route)}
                      aria-label={`编辑路由 ${route.pattern}`}
                      className="p-1 rounded-md text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
                      title="编辑路由"
                    >
                      <Edit2 className="w-3.5 h-3.5" />
                    </button>
                    <button
                      onClick={() => handleDelete(route.id)}
                      aria-label={`删除路由 ${route.pattern}`}
                      className="p-1 rounded-md text-gray-400 hover:text-red-600 dark:hover:text-red-400 hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
                      title="删除路由"
                    >
                      <Trash2 className="w-3.5 h-3.5" />
                    </button>
                  </div>

                  {/* 开关 */}
                  <Switch
                    checked={route.enabled}
                    onCheckedChange={(checked) => handleToggle(route.id, checked)}
                    ariaLabel={`${route.enabled ? '停用' : '启用'}路由 ${route.pattern}`}
                    className="ml-1"
                  />
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
};
