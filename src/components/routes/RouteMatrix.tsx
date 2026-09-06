import React, { useState } from 'react';
import { useAppStore } from '@/store/useAppStore';
import {
  Plus,
  ArrowRight,
  Edit2,
  Trash2,
  Sparkles,
  Search,
  CheckCircle2,
} from 'lucide-react';

export const RouteMatrix: React.FC = () => {
  const { routes, providers, openRouteModal, saveRoutes } = useAppStore();

  const [testModelInput, setTestModelInput] = useState('claude-3-7-sonnet-20250219');

  const handleDelete = async (id: string) => {
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
          matchedRule: '前缀路由分发规则',
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
          matchedRule: `命中规则 '${r.pattern}' (优先级: ${r.priority})`,
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
    <div className="space-y-6">
      {/* 顶部工具栏 */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
        <div>
          <h2 className="text-base font-semibold text-gray-900 dark:text-gray-100">
            动态路由矩阵
          </h2>
          <p className="text-xs text-gray-500 dark:text-gray-400">
            将客户端传入的模型请求按模式智能分发至目标服务商，支持多级故障自动降级队列
          </p>
        </div>

        <button
          onClick={() => openRouteModal(null)}
          className="flex items-center gap-1.5 px-4 py-2 text-sm font-medium text-white bg-indigo-600 rounded-lg hover:bg-indigo-700 transition-colors shadow-xs self-start sm:self-auto"
        >
          <Plus className="w-4 h-4" />
          <span>添加路由规则</span>
        </button>
      </div>

      {/* 实时路由匹配模拟器 */}
      <div className="p-4 rounded-xl border border-indigo-200/70 dark:border-indigo-900/60 bg-gradient-to-r from-indigo-50/40 via-white to-violet-50/30 dark:from-indigo-950/20 dark:via-gray-900 dark:to-violet-950/20 shadow-xs">
        <div className="flex items-center gap-2 mb-2 text-indigo-700 dark:text-indigo-300">
          <Sparkles className="w-4 h-4" />
          <h3 className="text-xs font-semibold uppercase tracking-wider">
            实时路由匹配模拟器
          </h3>
        </div>

        <div className="flex flex-col sm:flex-row items-stretch sm:items-center gap-3">
          <div className="relative flex-1">
            <Search className="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-gray-400" />
            <input
              type="text"
              value={testModelInput}
              onChange={(e) => setTestModelInput(e.target.value)}
              placeholder="输入模型名称测试路由匹配 (例如 claude-3-7-sonnet, openai/gpt-4o, deepseek-r1)..."
              className="w-full pl-9 pr-4 py-2 text-sm font-mono rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-indigo-500/20"
            />
          </div>

          {simulation && (
            <div className="flex items-center gap-2 text-xs bg-white dark:bg-gray-800 p-2 rounded-lg border border-gray-200 dark:border-gray-700 font-mono">
              <CheckCircle2 className="w-4 h-4 text-emerald-500 shrink-0" />
              <div className="truncate">
                <span className="text-gray-400">目标: </span>
                <span className="font-semibold text-indigo-600 dark:text-indigo-400">
                  {simulation.primary}
                </span>
                <span className="text-gray-500"> ({simulation.primaryModel})</span>
                {simulation.fallbacks.length > 0 && (
                  <span className="text-gray-400">
                    {' '}
                    &rarr; 降级备选: {simulation.fallbacks.join(', ')}
                  </span>
                )}
              </div>
            </div>
          )}
        </div>
      </div>

      {/* 规则表格 */}
      <div className="border border-gray-200 dark:border-gray-800 rounded-xl overflow-hidden bg-white dark:bg-gray-900 shadow-xs">
        <table className="w-full text-left border-collapse">
          <thead>
            <tr className="border-b border-gray-200 dark:border-gray-800 bg-gray-50/70 dark:bg-gray-800/50 text-xs font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wider">
              <th className="py-3 px-4">匹配模式 (Pattern)</th>
              <th className="py-3 px-4">目标服务商 (Target)</th>
              <th className="py-3 px-4">模型重定向 (Override)</th>
              <th className="py-3 px-4">降级备选队列 (Fallbacks)</th>
              <th className="py-3 px-4 text-center">优先级</th>
              <th className="py-3 px-4 text-center">状态</th>
              <th className="py-3 px-4 text-right">操作</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-gray-100 dark:divide-gray-800 text-sm">
            {routes.map((route) => {
              const primaryProv = providers.find((p) => p.id === route.targetProviderId);
              return (
                <tr
                  key={route.id}
                  className={`hover:bg-gray-50/80 dark:hover:bg-gray-800/40 transition-colors ${
                    !route.enabled ? 'opacity-50' : ''
                  }`}
                >
                  {/* Pattern */}
                  <td className="py-3 px-4">
                    <span className="font-mono text-xs font-semibold px-2 py-1 rounded bg-indigo-50 dark:bg-indigo-950/60 text-indigo-700 dark:text-indigo-300 border border-indigo-200/50 dark:border-indigo-800/40">
                      {route.pattern}
                    </span>
                    {route.description && (
                      <div className="text-[11px] text-gray-400 mt-1 max-w-xs truncate">
                        {route.description}
                      </div>
                    )}
                  </td>

                  {/* Target Provider */}
                  <td className="py-3 px-4">
                    <span className="font-medium text-gray-900 dark:text-gray-100">
                      {primaryProv ? primaryProv.name : route.targetProviderId}
                    </span>
                    <span className="ml-1.5 text-[10px] font-mono px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-500 uppercase">
                      {primaryProv?.protocol || 'api'}
                    </span>
                  </td>

                  {/* Target Model Override */}
                  <td className="py-3 px-4 font-mono text-xs text-gray-500 dark:text-gray-400">
                    {route.targetModel || <span className="italic text-gray-400">透传原模型</span>}
                  </td>

                  {/* Fallback Queue */}
                  <td className="py-3 px-4">
                    {route.fallbackProviderIds.length === 0 ? (
                      <span className="text-xs text-gray-400 italic">无</span>
                    ) : (
                      <div className="flex items-center gap-1.5 flex-wrap">
                        {route.fallbackProviderIds.map((fbId, idx) => (
                          <React.Fragment key={fbId}>
                            {idx > 0 && <ArrowRight className="w-3 h-3 text-gray-400" />}
                            <span className="text-xs px-2 py-0.5 rounded bg-gray-100 dark:bg-gray-800 font-mono text-gray-700 dark:text-gray-300 border border-gray-200/60 dark:border-gray-700/60">
                              {getProviderName(fbId)}
                            </span>
                          </React.Fragment>
                        ))}
                      </div>
                    )}
                  </td>

                  {/* Priority */}
                  <td className="py-3 px-4 text-center font-mono text-xs text-gray-600 dark:text-gray-400">
                    {route.priority}
                  </td>

                  {/* Toggle */}
                  <td className="py-3 px-4 text-center">
                    <input
                      type="checkbox"
                      checked={route.enabled}
                      onChange={(e) => handleToggle(route.id, e.target.checked)}
                      className="rounded border-gray-300 text-indigo-600 focus:ring-indigo-500 cursor-pointer"
                    />
                  </td>

                  {/* Actions */}
                  <td className="py-3 px-4 text-right">
                    <div className="flex items-center justify-end gap-1">
                      <button
                        onClick={() => openRouteModal(route)}
                        className="p-1 rounded text-gray-400 hover:text-gray-700 dark:hover:text-gray-200"
                        title="编辑规则"
                      >
                        <Edit2 className="w-4 h-4" />
                      </button>
                      <button
                        onClick={() => handleDelete(route.id)}
                        className="p-1 rounded text-gray-400 hover:text-red-600 dark:hover:text-red-400"
                        title="删除规则"
                      >
                        <Trash2 className="w-4 h-4" />
                      </button>
                    </div>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
};
