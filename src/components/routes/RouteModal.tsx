import React, { useState, useEffect } from 'react';
import { useAppStore } from '@/store/useAppStore';
import { RouteRule } from '@/types';
import { Select } from '@/components/common/Select';
import { useDialogAccessibility } from '@/components/common/useDialogAccessibility';
import { X } from 'lucide-react';

export const RouteModal: React.FC = () => {
  const { isRouteModalOpen, editingRoute, closeRouteModal, saveRoutes, routes, providers } =
    useAppStore();

  const [pattern, setPattern] = useState('');
  const [targetProviderId, setTargetProviderId] = useState('');
  const [targetModel, setTargetModel] = useState('');
  const [fallbackProviderIds, setFallbackProviderIds] = useState<string[]>([]);
  const [priority, setPriority] = useState(50);
  const [description, setDescription] = useState('');
  const [enabled, setEnabled] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const dialogRef = useDialogAccessibility(isRouteModalOpen, closeRouteModal);

  useEffect(() => {
    if (editingRoute) {
      setPattern(editingRoute.pattern);
      setTargetProviderId(editingRoute.targetProviderId);
      setTargetModel(editingRoute.targetModel || '');
      setFallbackProviderIds(editingRoute.fallbackProviderIds);
      setPriority(editingRoute.priority);
      setDescription(editingRoute.description || '');
      setEnabled(editingRoute.enabled);
    } else {
      setPattern('claude-*');
      setTargetProviderId(providers[0]?.id || 'anthropic');
      setTargetModel('');
      setFallbackProviderIds([]);
      setPriority(50);
      setDescription('');
      setEnabled(true);
    }
    setError(null);
  }, [editingRoute, isRouteModalOpen, providers]);

  if (!isRouteModalOpen) return null;

  const handleToggleFallback = (providerId: string) => {
    if (fallbackProviderIds.includes(providerId)) {
      setFallbackProviderIds(fallbackProviderIds.filter((id) => id !== providerId));
    } else {
      setFallbackProviderIds([...fallbackProviderIds, providerId]);
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!pattern.trim()) {
      setError('请输入模型匹配规则，例如 claude-*、gpt-* 或 *');
      return;
    }
    if (!targetProviderId) {
      setError('请选择目标服务商');
      return;
    }

    const rule: RouteRule = {
      id: editingRoute ? editingRoute.id : `rule_${Math.random().toString(36).substring(2, 8)}`,
      pattern: pattern.trim(),
      targetProviderId,
      targetModel: targetModel.trim() ? targetModel.trim() : undefined,
      fallbackProviderIds,
      priority,
      description: description.trim() ? description.trim() : undefined,
      enabled,
    };

    const updatedRoutes = editingRoute
      ? routes.map((r) => (r.id === editingRoute.id ? rule : r))
      : [...routes, rule];

    try {
      await saveRoutes(updatedRoutes);
      closeRouteModal();
    } catch (err: unknown) {
      setError(String(err));
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm p-4 pt-14">
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="route-modal-title"
        className="bg-white dark:bg-gray-900 rounded-2xl border border-gray-200 dark:border-gray-800 shadow-2xl w-full max-w-lg max-h-[calc(100dvh-4.5rem)] overflow-hidden flex flex-col"
      >
        {/* 弹窗头部 */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-gray-200 dark:border-gray-800">
          <h2 id="route-modal-title" className="text-base font-semibold text-gray-900 dark:text-gray-100">
            {editingRoute ? '编辑覆盖规则' : '添加覆盖规则'}
          </h2>
          <button
            type="button"
            onClick={closeRouteModal}
            aria-label="关闭覆盖规则窗口"
            className="p-1 rounded-lg text-gray-400 hover:text-gray-600 dark:hover:text-gray-200 cursor-pointer"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* 弹窗表单 */}
        <form onSubmit={handleSubmit} className="min-h-0 overflow-y-auto p-6 space-y-4">
          {error && (
            <div className="p-3 text-xs bg-red-50 dark:bg-red-950/50 border border-red-200 dark:border-red-800 text-red-700 dark:text-red-300 rounded-lg">
              {error}
            </div>
          )}

          <div>
            <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
              模型匹配规则
            </label>
            <input
              type="text"
              data-autofocus
              placeholder="例如 claude-*, gpt-4*, deepseek/*, *"
              value={pattern}
              onChange={(e) => setPattern(e.target.value)}
              className="w-full px-3 py-2 text-sm font-mono rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 focus:outline-none focus:ring-2 focus:ring-indigo-500/20"
              required
            />
            <p className="text-[11px] text-gray-400 mt-1">
              支持通配符（*、?）、前缀匹配（如 deepseek/）和精确匹配。
            </p>
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                目标服务商
              </label>
              <Select
                value={targetProviderId}
                onChange={(e) => setTargetProviderId(e.target.value)}
              >
                {providers.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name} ({p.protocol})
                  </option>
                ))}
              </Select>
            </div>

            <div>
              <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                目标模型（可选）
              </label>
              <input
                type="text"
                placeholder="留空则使用请求中的模型名称"
                value={targetModel}
                onChange={(e) => setTargetModel(e.target.value)}
                className="w-full px-3 py-2 text-sm font-mono rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 focus:outline-none focus:ring-2 focus:ring-indigo-500/20"
              />
            </div>
          </div>

          {/* 故障降级备选队列 */}
          <div>
            <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
              备用服务商（遇到 429、5xx 或超时时依次尝试）
            </label>
            <div className="grid grid-cols-2 gap-2 border border-gray-200 dark:border-gray-700 rounded-lg p-3 max-h-36 overflow-y-auto">
              {providers
                .filter((p) => p.id !== targetProviderId)
                .map((p) => {
                  const isChecked = fallbackProviderIds.includes(p.id);
                  return (
                    <label
                      key={p.id}
                      className="flex items-center gap-2 text-xs text-gray-700 dark:text-gray-300 cursor-pointer select-none"
                    >
                      <input
                        type="checkbox"
                        checked={isChecked}
                        onChange={() => handleToggleFallback(p.id)}
                        className="rounded border-gray-300 text-indigo-600 focus:ring-indigo-500"
                      />
                      <span className="truncate">{p.name}</span>
                    </label>
                  );
                })}
            </div>
            <p className="text-[11px] text-gray-400 mt-1">
              按勾选顺序尝试；每个候选使用它自己声明的模型名，无法提供该模型的服务商会自动跳过。
            </p>
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                优先级（数值越大越先匹配）
              </label>
              <input
                type="number"
                value={priority}
                onChange={(e) => setPriority(parseInt(e.target.value) || 0)}
                className="w-full px-3 py-1.5 text-sm rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100"
              />
            </div>

            <div>
              <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                备注
              </label>
              <input
                type="text"
                placeholder="例如：Claude 请求优先使用中继"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                className="w-full px-3 py-1.5 text-sm rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100"
              />
            </div>
          </div>

          <div className="flex items-center gap-2 pt-1">
            <input
              type="checkbox"
              id="enable-route"
              checked={enabled}
              onChange={(e) => setEnabled(e.target.checked)}
              className="rounded border-gray-300 text-indigo-600 focus:ring-indigo-500 cursor-pointer"
            />
            <label htmlFor="enable-route" className="text-sm text-gray-700 dark:text-gray-300 cursor-pointer select-none">
              立即启用
            </label>
          </div>

          {/* 底部按钮 */}
          <div className="flex items-center justify-end gap-3 pt-4 border-t border-gray-200 dark:border-gray-800">
            <button
              type="button"
              onClick={closeRouteModal}
              className="px-4 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg hover:bg-gray-50 dark:hover:bg-gray-700"
            >
              取消
            </button>
            <button
              type="submit"
              className="px-4 py-2 text-sm font-medium text-white bg-indigo-600 rounded-lg hover:bg-indigo-700 shadow-xs"
            >
              {editingRoute ? '保存修改' : '创建覆盖规则'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
