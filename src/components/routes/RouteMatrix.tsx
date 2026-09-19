import React, { useState } from 'react';
import { useAppStore } from '@/store/useAppStore';
import { Switch } from '@/components/common/Switch';
import { ConfirmDialog } from '@/components/common/ConfirmDialog';
import type { RoutePlan, RouteRule } from '@/types';
import {
  Plus,
  ArrowRight,
  Edit2,
  Trash2,
  Search,
  CheckCircle2,
  LoaderCircle,
  ShieldAlert,
} from 'lucide-react';

export const RouteMatrix: React.FC = () => {
  const { routes, providers, openRouteModal, saveRoutes, previewRoute } = useAppStore();

  const [pendingDelete, setPendingDelete] = useState<RouteRule | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [testModelInput, setTestModelInput] = useState('claude-3-7-sonnet-20250219');
  const [preview, setPreview] = useState<RoutePlan | null>(null);
  const [previewError, setPreviewError] = useState('');
  const [previewing, setPreviewing] = useState(false);

  const confirmDelete = async () => {
    if (!pendingDelete || deleting) return;
    setDeleting(true);
    try {
      await saveRoutes(routes.filter((r) => r.id !== pendingDelete.id));
      setPendingDelete(null);
    } catch {
      // 失败由全局错误展示
    } finally {
      setDeleting(false);
    }
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
    <div className="space-y-4 animate-in fade-in duration-150">
      {/* 极简标题与新建操作 */}
      <div className="flex items-center justify-between pb-1">
        <h2 className="text-base font-bold text-zinc-950 dark:text-zinc-100">
          路由规则
        </h2>

        <button
          onClick={() => openRouteModal(null)}
          className="flex items-center gap-1.5 px-3.5 h-8 text-xs font-medium text-white bg-indigo-600 hover:bg-indigo-700 rounded-xl shadow-xs transition-colors cursor-pointer"
        >
          <Plus className="w-3.5 h-3.5" />
          <span>新建规则</span>
        </button>
      </div>

      {/* 紧凑调试器：直奔主题 */}
      <div className="p-3.5 sm:p-4 rounded-2xl border border-zinc-200/80 dark:border-zinc-800 bg-white dark:bg-zinc-900 shadow-xs space-y-3">
        <div className="flex flex-col sm:flex-row items-stretch sm:items-center gap-2">
          <div className="relative flex-1">
            <Search className="w-3.5 h-3.5 absolute left-3 top-1/2 -translate-y-1/2 text-zinc-400 pointer-events-none" />
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
              placeholder="测试模型路由解析，如 claude-3-7-sonnet-20250219、gpt-4o"
              className="w-full pl-8.5 pr-4 py-2 text-xs font-mono rounded-xl border border-zinc-200 dark:border-zinc-800 bg-zinc-50 dark:bg-zinc-950 text-zinc-900 dark:text-zinc-100 placeholder-zinc-400 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-500"
            />
          </div>

          <button
            onClick={runPreview}
            disabled={previewing || !testModelInput.trim()}
            className="flex items-center justify-center gap-1.5 px-4 h-9 text-xs font-medium text-white bg-indigo-600 rounded-xl hover:bg-indigo-700 disabled:opacity-50 transition-colors shadow-xs shrink-0 cursor-pointer"
          >
            {previewing ? (
              <LoaderCircle className="w-3.5 h-3.5 animate-spin" />
            ) : (
              <Search className="w-3.5 h-3.5" />
            )}
            <span>解析</span>
          </button>
        </div>

        {/* 调试结果展示 */}
        {preview && (
          <div className="p-3 rounded-xl bg-zinc-50 dark:bg-zinc-850/60 border border-zinc-200/80 dark:border-zinc-800 space-y-2 animate-in fade-in">
            <div className="flex items-center gap-2 text-xs">
              <CheckCircle2 className="w-4 h-4 text-emerald-500 shrink-0" />
              <span className="text-zinc-500">分发至</span>
              <span className="font-bold text-indigo-600 dark:text-indigo-400">
                {getProviderName(preview.primary.providerId)}
              </span>
              <span className="text-zinc-400">→</span>
              <span className="font-mono bg-white dark:bg-zinc-800 px-1.5 py-0.5 rounded border border-zinc-200/60 dark:border-zinc-700 text-zinc-700 dark:text-zinc-300">
                {preview.primary.model}
              </span>
            </div>

            {preview.fallbacks.length > 0 && (
              <div className="pt-2 border-t border-zinc-200/50 dark:border-zinc-800/50 flex flex-wrap items-center gap-2 text-[11px]">
                <span className="text-zinc-400">备选链：</span>
                {preview.fallbacks.map((fb, idx) => (
                  <span
                    key={idx}
                    className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded bg-white dark:bg-zinc-800 border border-zinc-200/60 dark:border-zinc-700 text-zinc-600 dark:text-zinc-300 font-mono"
                  >
                    <span>#{idx + 1} {getProviderName(fb.providerId)}</span>
                    <span className="text-zinc-400">({fb.model})</span>
                  </span>
                ))}
              </div>
            )}
          </div>
        )}

        {previewError && (
          <div className="p-2.5 rounded-xl bg-rose-50 dark:bg-rose-950/40 border border-rose-200 dark:border-rose-900/60 text-xs text-rose-700 dark:text-rose-300 flex items-center gap-2">
            <ShieldAlert className="w-4 h-4 text-rose-500 shrink-0" />
            <span>无匹配路由：{previewError}</span>
          </div>
        )}
      </div>

      {/* 规则列表 */}
      <div className="rounded-2xl border border-zinc-200/80 dark:border-zinc-800 bg-white dark:bg-zinc-900 overflow-hidden shadow-xs">
        {routes.length === 0 ? (
          <div className="p-8 text-center text-xs text-zinc-400">
            暂无覆盖规则。所有请求将按服务商模型声明及默认模型自动路由。
          </div>
        ) : (
          <div className="divide-y divide-zinc-100 dark:divide-zinc-800">
            {routes.map((rule) => (
              <div
                key={rule.id}
                className="p-3.5 sm:px-5 flex flex-col sm:flex-row sm:items-center justify-between gap-3 hover:bg-zinc-50/50 dark:hover:bg-zinc-850/30 transition-colors"
              >
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="font-mono text-xs font-bold text-zinc-900 dark:text-zinc-100 bg-zinc-100 dark:bg-zinc-800 px-2 py-0.5 rounded-md">
                      {rule.pattern}
                    </span>
                    <ArrowRight className="w-3 h-3 text-zinc-400 shrink-0" />
                    <span className="text-xs font-semibold text-indigo-600 dark:text-indigo-400">
                      {getProviderName(rule.targetProviderId)}
                    </span>
                    {rule.targetModel && (
                      <span className="text-[11px] font-mono text-zinc-500 dark:text-zinc-400 bg-zinc-50 dark:bg-zinc-850 px-1.5 py-0.2 rounded border border-zinc-200/50 dark:border-zinc-700/50">
                        {rule.targetModel}
                      </span>
                    )}
                  </div>

                  {rule.description && (
                    <p className="text-[11px] text-zinc-400 mt-1">
                      {rule.description}
                    </p>
                  )}

                  {rule.fallbackProviderIds.length > 0 && (
                    <div className="flex items-center gap-1.5 mt-1 text-[11px] text-zinc-400">
                      <span>备选：</span>
                      <span>
                        {rule.fallbackProviderIds.map((id) => getProviderName(id)).join(' → ')}
                      </span>
                    </div>
                  )}
                </div>

                <div className="flex items-center justify-between sm:justify-end gap-2.5 shrink-0">
                  <button
                    type="button"
                    onClick={() => openRouteModal(rule)}
                    className="p-1.5 rounded-lg text-zinc-400 hover:text-zinc-700 dark:hover:text-zinc-200 hover:bg-zinc-100 dark:hover:bg-zinc-800 transition-colors cursor-pointer"
                    title="编辑"
                  >
                    <Edit2 className="w-3.5 h-3.5" />
                  </button>

                  <button
                    type="button"
                    onClick={() => setPendingDelete(rule)}
                    className="p-1.5 rounded-lg text-zinc-400 hover:text-rose-500 hover:bg-rose-50 dark:hover:bg-rose-950/40 transition-colors cursor-pointer"
                    title="删除"
                  >
                    <Trash2 className="w-3.5 h-3.5" />
                  </button>

                  <div className="pl-1.5 border-l border-zinc-200 dark:border-zinc-800 flex items-center">
                    <Switch
                      checked={rule.enabled}
                      onCheckedChange={(checked) => handleToggle(rule.id, checked)}
                      ariaLabel="启用规则"
                    />
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      <ConfirmDialog
        open={pendingDelete !== null}
        title="确认删除该规则？"
        details={[`匹配模式：“${pendingDelete?.pattern}”。`]}
        confirmLabel="删除"
        busy={deleting}
        onConfirm={confirmDelete}
        onCancel={() => setPendingDelete(null)}
      />
    </div>
  );
};
