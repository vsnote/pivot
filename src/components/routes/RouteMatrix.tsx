import React, { useState } from 'react';
import { useAppStore } from '@/store/useAppStore';
import { Switch } from '@/components/common/Switch';
import type { RoutePlan } from '@/types';
import {
  Plus,
  ArrowRight,
  Edit2,
  Trash2,
  Search,
  CheckCircle2,
  Network,
  LoaderCircle,
  Sparkles,
} from 'lucide-react';

const RESOLUTION_ORDER = ['provider/model 前缀', '命中的规则', '精确模型名匹配', '默认服务商的默认模型'];

export const RouteMatrix: React.FC = () => {
  const { routes, providers, openRouteModal, saveRoutes, previewRoute } = useAppStore();

  const [testModelInput, setTestModelInput] = useState('claude-3-7-sonnet-20250219');
  const [preview, setPreview] = useState<RoutePlan | null>(null);
  const [previewError, setPreviewError] = useState('');
  const [previewing, setPreviewing] = useState(false);

  const handleDelete = async (id: string) => {
    const route = routes.find((item) => item.id === id);
    if (!window.confirm(`确定删除覆盖规则“${route?.pattern ?? id}”吗？此操作无法撤销。`)) return;
    await saveRoutes(routes.filter((r) => r.id !== id));
  };

  const handleToggle = async (id: string, enabled: boolean) => {
    await saveRoutes(routes.map((r) => (r.id === id ? { ...r, enabled } : r)));
  };

  const getProviderName = (providerId: string) => {
    return providers.find((p) => p.id === providerId)?.name || providerId;
  };

  const runPreview = async () => {
    const model = testModelInput.trim();
    if (!model || previewing) return;
    setPreviewing(true);
    setPreviewError('');
    try {
      setPreview(await previewRoute(model));
    } catch (error) {
      setPreview(null);
      setPreviewError(String(error));
    } finally {
      setPreviewing(false);
    }
  };

  return (
    <div className="space-y-4 pb-8 animate-in fade-in duration-150">
      <section className="p-4 rounded-xl border border-gray-200/80 dark:border-white/10 bg-white dark:bg-[#1a1b1e] shadow-xs space-y-3">
        <div className="flex items-center gap-2">
          <Sparkles className="w-4 h-4 text-indigo-500 shrink-0" />
          <h2 className="text-sm font-semibold text-gray-900 dark:text-gray-100">
            模型路由由网关自动完成
          </h2>
        </div>
        <p className="text-xs leading-relaxed text-gray-600 dark:text-gray-400">
          只在「服务商」页配置接口、密钥、模型和默认模型即可。网关按
          {RESOLUTION_ORDER.map((step, index) => (
            <React.Fragment key={step}>
              {index > 0 && <span className="text-gray-400 dark:text-gray-500"> → </span>}
              <span className="font-medium text-gray-700 dark:text-gray-300">{step}</span>
            </React.Fragment>
          ))}
          的顺序解析模型名，并对每个候选使用它自己能提供的模型名，服务商不可用时自动换下一家。
        </p>
        <p className="text-xs leading-relaxed text-gray-500 dark:text-gray-400">
          下面的规则是高级覆盖项，命中后优先于自动解析。只有需要强制改写模型名、固定服务商或指定备用顺序时才添加。
        </p>
      </section>

      {/* 路由预览：由网关真实解析，避免与后端行为不一致 */}
      <div className="p-3.5 rounded-xl border border-gray-200/80 dark:border-white/10 bg-white dark:bg-[#1a1b1e] shadow-xs">
        <div className="flex flex-col sm:flex-row items-stretch sm:items-center gap-3">
          <div className="relative flex-1">
            <Search className="w-3.5 h-3.5 absolute left-3 top-1/2 -translate-y-1/2 text-gray-400" />
            <input
              type="text"
              value={testModelInput}
              onChange={(e) => {
                setTestModelInput(e.target.value);
                setPreview(null);
                setPreviewError('');
              }}
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  e.preventDefault();
                  runPreview();
                }
              }}
              placeholder="输入模型名称查看网关会怎么路由，例如 claude-3-7-sonnet、gpt-4o"
              className="w-full pl-8.5 pr-4 py-1.5 text-xs font-mono rounded-lg border border-gray-200 dark:border-zinc-800 bg-gray-50/60 dark:bg-zinc-900/60 text-gray-900 dark:text-gray-100 placeholder-gray-400 focus:outline-none focus:ring-1 focus:ring-indigo-500/30"
            />
          </div>

          <button
            onClick={runPreview}
            disabled={previewing || !testModelInput.trim()}
            className="flex items-center justify-center gap-1.5 px-3 h-7 text-xs font-medium text-white bg-indigo-600 rounded-lg hover:bg-indigo-700 disabled:opacity-50 transition-colors shadow-xs shrink-0 cursor-pointer"
          >
            {previewing ? <LoaderCircle className="w-3.5 h-3.5 animate-spin" /> : <Search className="w-3.5 h-3.5" />}
            <span>预览路由</span>
          </button>

          <button
            onClick={() => openRouteModal(null)}
            className="flex items-center justify-center gap-1.5 px-3 h-7 text-xs font-medium text-indigo-600 dark:text-indigo-400 border border-indigo-200 dark:border-indigo-900/60 rounded-lg hover:bg-indigo-50 dark:hover:bg-indigo-950/40 transition-colors shrink-0 cursor-pointer"
          >
            <Plus className="w-3.5 h-3.5" />
            <span>添加覆盖规则</span>
          </button>
        </div>

        {previewError && (
          <div className="mt-2.5 flex items-start gap-2 text-xs bg-red-50/70 dark:bg-red-950/30 p-2 rounded-lg border border-red-100 dark:border-red-900/40">
            <span className="text-red-600 dark:text-red-400 leading-relaxed">{previewError}</span>
          </div>
        )}

        {preview && (
          <div className="mt-2.5 flex items-start gap-2 text-xs bg-indigo-50/60 dark:bg-indigo-950/30 p-2 rounded-lg border border-indigo-100 dark:border-indigo-900/40">
            <CheckCircle2 className="w-3.5 h-3.5 text-emerald-500 shrink-0 mt-0.5" />
            <div className="text-[11.5px] leading-relaxed">
              <span className="text-gray-500 dark:text-zinc-400">首选：</span>
              <span className="font-semibold text-indigo-600 dark:text-indigo-400">
                {getProviderName(preview.primary.providerId)}
              </span>
              <span className="text-gray-500 dark:text-zinc-400">（{preview.primary.model}）</span>
              {preview.fallbacks.length > 0 && (
                <div className="text-gray-400 dark:text-zinc-500 mt-0.5">
                  依次备用：
                  {preview.fallbacks.map((candidate, index) => (
                    <React.Fragment key={`${candidate.providerId}-${index}`}>
                      {index > 0 && <ArrowRight className="w-3 h-3 inline -mt-0.5 mx-0.5" />}
                      <span className="text-gray-600 dark:text-gray-300">
                        {getProviderName(candidate.providerId)}
                      </span>
                      <span>（{candidate.model}）</span>
                    </React.Fragment>
                  ))}
                </div>
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
            没有覆盖规则
          </h3>
          <p className="text-xs text-gray-500 dark:text-gray-400 mt-1 max-w-md mx-auto">
            这是正常状态：模型名由服务商配置自动解析。需要强制改写模型或固定服务商时，再添加规则。
          </p>
          <div className="mt-4">
            <button
              onClick={() => openRouteModal(null)}
              className="inline-flex items-center gap-1.5 px-3.5 py-1.5 text-xs font-medium text-white bg-indigo-600 rounded-lg hover:bg-indigo-700 shadow-xs"
            >
              <Plus className="w-3.5 h-3.5" />
              <span>添加覆盖规则</span>
            </button>
          </div>
        </div>
      ) : (
        <div className="space-y-2">
          {routes.map((route) => {
            const primaryProv = providers.find((p) => p.id === route.targetProviderId);
            const providerEnabled = primaryProv?.enabled ?? false;

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

                  {!providerEnabled && (
                    <p className="text-[11px] text-amber-600 dark:text-amber-400">
                      目标服务商未启用，这条规则当前不会生效，请求会继续按自动解析处理。
                    </p>
                  )}
                </div>

                {/* 右侧：开关与操作 */}
                <div className="flex items-center gap-2 shrink-0 self-end sm:self-center">
                  <div className="flex items-center gap-0.5">
                    <button
                      onClick={() => openRouteModal(route)}
                      aria-label={`编辑覆盖规则 ${route.pattern}`}
                      className="p-1 rounded-md text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
                      title="编辑覆盖规则"
                    >
                      <Edit2 className="w-3.5 h-3.5" />
                    </button>
                    <button
                      onClick={() => handleDelete(route.id)}
                      aria-label={`删除覆盖规则 ${route.pattern}`}
                      className="p-1 rounded-md text-gray-400 hover:text-red-600 dark:hover:text-red-400 hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
                      title="删除覆盖规则"
                    >
                      <Trash2 className="w-3.5 h-3.5" />
                    </button>
                  </div>
                  <Switch
                    checked={route.enabled}
                    onCheckedChange={(checked) => handleToggle(route.id, checked)}
                    ariaLabel={`${route.enabled ? '停用' : '启用'}覆盖规则 ${route.pattern}`}
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
