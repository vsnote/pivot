import React, { useState, useEffect } from 'react';
import { useAppStore } from '@/store/useAppStore';
import { Provider, Protocol } from '@/types';
import { X, Plus, Trash2 } from 'lucide-react';

export const ProviderModal: React.FC = () => {
  const { isProviderModalOpen, editingProvider, closeProviderModal, saveProvider } =
    useAppStore();

  const [id, setId] = useState('');
  const [name, setName] = useState('');
  const [protocol, setProtocol] = useState<Protocol>('openai');
  const [baseUrl, setBaseUrl] = useState('');
  const [apiKeys, setApiKeys] = useState<string[]>(['']);
  const [modelsInput, setModelsInput] = useState('');
  const [priority, setPriority] = useState(10);
  const [weight, setWeight] = useState(1);
  const [timeoutSecs, setTimeoutSecs] = useState(60);
  const [enabled, setEnabled] = useState(true);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (editingProvider) {
      setId(editingProvider.id);
      setName(editingProvider.name);
      setProtocol(editingProvider.protocol);
      setBaseUrl(editingProvider.baseUrl);
      setApiKeys(editingProvider.apiKeys.length > 0 ? editingProvider.apiKeys : ['']);
      setModelsInput(editingProvider.models.join(', '));
      setPriority(editingProvider.priority);
      setWeight(editingProvider.weight);
      setTimeoutSecs(editingProvider.timeoutSecs);
      setEnabled(editingProvider.enabled);
    } else {
      setId(`prov_${Math.random().toString(36).substring(2, 8)}`);
      setName('');
      setProtocol('openai');
      setBaseUrl('https://api.openai.com/v1');
      setApiKeys(['']);
      setModelsInput('');
      setPriority(10);
      setWeight(1);
      setTimeoutSecs(60);
      setEnabled(true);
    }
    setError(null);
  }, [editingProvider, isProviderModalOpen]);

  if (!isProviderModalOpen) return null;

  const handleAddKey = () => {
    setApiKeys([...apiKeys, '']);
  };

  const handleKeyChange = (index: number, val: string) => {
    const updated = [...apiKeys];
    updated[index] = val;
    setApiKeys(updated);
  };

  const handleRemoveKey = (index: number) => {
    setApiKeys(apiKeys.filter((_, i) => i !== index));
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);

    if (!name.trim()) {
      setError('请输入服务商名称');
      return;
    }
    if (!baseUrl.trim()) {
      setError('请输入 Base URL (接口地址)');
      return;
    }

    const cleanKeys = apiKeys.map((k) => k.trim()).filter((k) => k.length > 0);
    const parsedModels = modelsInput
      .split(',')
      .map((m) => m.trim())
      .filter((m) => m.length > 0);

    const provider: Provider = {
      id: editingProvider ? editingProvider.id : id.trim(),
      name: name.trim(),
      protocol,
      baseUrl: baseUrl.trim(),
      apiKeys: cleanKeys,
      models: parsedModels,
      priority,
      weight,
      timeoutSecs,
      enabled,
      createdAt: editingProvider?.createdAt,
      updatedAt: new Date().toISOString(),
    };

    setIsSubmitting(true);
    try {
      await saveProvider(provider);
      closeProviderModal();
    } catch (err: unknown) {
      setError(String(err));
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm p-4 overflow-y-auto">
      <div className="bg-white dark:bg-gray-900 rounded-2xl border border-gray-200 dark:border-gray-800 shadow-2xl w-full max-w-xl max-h-[90vh] flex flex-col my-auto">
        {/* 弹窗头部 */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-gray-200 dark:border-gray-800">
          <div>
            <h2 className="text-base font-semibold text-gray-900 dark:text-gray-100">
              {editingProvider ? '编辑服务商' : '添加新服务商'}
            </h2>
            <p className="text-xs text-gray-500 dark:text-gray-400">
              配置上游 API 端点、账号池负载均衡与支持模型列表
            </p>
          </div>
          <button
            onClick={closeProviderModal}
            className="p-1 rounded-lg text-gray-400 hover:text-gray-600 dark:hover:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* 弹窗表单 */}
        <form onSubmit={handleSubmit} className="p-6 space-y-4 overflow-y-auto flex-1">
          {error && (
            <div className="p-3 text-xs bg-red-50 dark:bg-red-950/50 border border-red-200 dark:border-red-800 text-red-700 dark:text-red-300 rounded-lg">
              {error}
            </div>
          )}

          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                服务商名称
              </label>
              <input
                type="text"
                placeholder="例如：Anthropic 官方、DeepSeek、硅基流动"
                value={name}
                onChange={(e) => setName(e.target.value)}
                className="w-full px-3 py-2 text-sm rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 focus:outline-none focus:ring-2 focus:ring-indigo-500/20"
                required
              />
            </div>

            <div>
              <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                协议类型 (Protocol)
              </label>
              <select
                value={protocol}
                onChange={(e) => setProtocol(e.target.value as Protocol)}
                className="w-full px-3 py-2 text-sm rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 focus:outline-none focus:ring-2 focus:ring-indigo-500/20"
              >
                <option value="anthropic">Anthropic (Messages API)</option>
                <option value="openai">OpenAI (Chat Completions)</option>
                <option value="gemini">Google Gemini</option>
                <option value="openrouter">OpenRouter</option>
                <option value="ollama">Ollama (本地私有化)</option>
              </select>
            </div>
          </div>

          <div>
            <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
              Base URL (接口地址)
            </label>
            <input
              type="text"
              placeholder="例如：https://api.anthropic.com/v1"
              value={baseUrl}
              onChange={(e) => setBaseUrl(e.target.value)}
              className="w-full px-3 py-2 text-sm font-mono rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 focus:outline-none focus:ring-2 focus:ring-indigo-500/20"
              required
            />
          </div>

          {/* API Keys (账号池) */}
          <div>
            <div className="flex items-center justify-between mb-1">
              <label className="block text-xs font-medium text-gray-700 dark:text-gray-300">
                API Keys (多账号池轮询 &amp; 负载均衡)
              </label>
              <button
                type="button"
                onClick={handleAddKey}
                className="text-xs text-indigo-600 dark:text-indigo-400 hover:underline flex items-center gap-1 font-medium"
              >
                <Plus className="w-3 h-3" />
                <span>添加 Key</span>
              </button>
            </div>
            <div className="space-y-2 max-h-36 overflow-y-auto pr-1">
              {apiKeys.map((key, idx) => (
                <div key={idx} className="flex items-center gap-2">
                  <input
                    type="password"
                    placeholder={`API Key #${idx + 1}`}
                    value={key}
                    onChange={(e) => handleKeyChange(idx, e.target.value)}
                    className="w-full px-3 py-1.5 text-sm font-mono rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 focus:outline-none focus:ring-2 focus:ring-indigo-500/20"
                  />
                  {apiKeys.length > 1 && (
                    <button
                      type="button"
                      onClick={() => handleRemoveKey(idx)}
                      className="p-1.5 text-gray-400 hover:text-red-500 transition-colors"
                      title="删除此 Key"
                    >
                      <Trash2 className="w-4 h-4" />
                    </button>
                  )}
                </div>
              ))}
            </div>
          </div>

          {/* 模型列表 */}
          <div>
            <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
              支持模型列表 (英文逗号分隔)
            </label>
            <textarea
              rows={2}
              placeholder="claude-3-7-sonnet-20250219, claude-3-5-sonnet-20241022"
              value={modelsInput}
              onChange={(e) => setModelsInput(e.target.value)}
              className="w-full px-3 py-2 text-sm font-mono rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 focus:outline-none focus:ring-2 focus:ring-indigo-500/20"
            />
          </div>

          <div className="grid grid-cols-3 gap-3">
            <div>
              <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                优先级 (越小越优先)
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
                权重 (Weight)
              </label>
              <input
                type="number"
                value={weight}
                onChange={(e) => setWeight(parseInt(e.target.value) || 1)}
                className="w-full px-3 py-1.5 text-sm rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100"
              />
            </div>

            <div>
              <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                超时时间 (秒)
              </label>
              <input
                type="number"
                value={timeoutSecs}
                onChange={(e) => setTimeoutSecs(parseInt(e.target.value) || 60)}
                className="w-full px-3 py-1.5 text-sm rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100"
              />
            </div>
          </div>

          <div className="flex items-center gap-2 pt-2">
            <input
              type="checkbox"
              id="enable-provider"
              checked={enabled}
              onChange={(e) => setEnabled(e.target.checked)}
              className="rounded border-gray-300 text-indigo-600 focus:ring-indigo-500"
            />
            <label htmlFor="enable-provider" className="text-sm text-gray-700 dark:text-gray-300 cursor-pointer select-none">
              立即启用此服务商
            </label>
          </div>

          {/* 弹窗底部操作 */}
          <div className="flex items-center justify-end gap-3 pt-4 border-t border-gray-200 dark:border-gray-800">
            <button
              type="button"
              onClick={closeProviderModal}
              className="px-4 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors"
            >
              取消
            </button>
            <button
              type="submit"
              disabled={isSubmitting}
              className="px-4 py-2 text-sm font-medium text-white bg-indigo-600 rounded-lg hover:bg-indigo-700 transition-colors shadow-xs disabled:opacity-50"
            >
              {isSubmitting ? '保存中...' : editingProvider ? '保存修改' : '创建服务商'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
