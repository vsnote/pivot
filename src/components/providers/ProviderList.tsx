import React, { useState } from 'react';
import { useAppStore } from '@/store/useAppStore';
import {
  Plus,
  Sparkles,
  RefreshCw,
  Trash2,
  Edit2,
  Copy,
  Check,
  Zap,
  Globe,
  Key,
  Search,
  LayoutList,
  LayoutGrid,
} from 'lucide-react';

export const ProviderList: React.FC = () => {
  const {
    providers,
    openProviderModal,
    openPresetDrawer,
    deleteProvider,
    toggleProvider,
    testProviderLatency,
  } = useAppStore();

  const [searchQuery, setSearchQuery] = useState('');
  const [protocolFilter, setProtocolFilter] = useState<string>('all');
  const [testingId, setTestingId] = useState<string | null>(null);
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const [viewMode, setViewMode] = useState<'list' | 'card'>('list');

  const handleCopy = (id: string, text: string) => {
    navigator.clipboard.writeText(text);
    setCopiedId(id);
    setTimeout(() => setCopiedId(null), 2000);
  };

  const handleTestLatency = async (id: string) => {
    setTestingId(id);
    try {
      await testProviderLatency(id);
    } finally {
      setTestingId(null);
    }
  };

  const handleTestAll = async () => {
    for (const p of providers) {
      if (p.enabled) {
        setTestingId(p.id);
        try {
          await testProviderLatency(p.id);
        } catch {
          // ignore
        }
      }
    }
    setTestingId(null);
  };

  const filteredProviders = providers.filter((p) => {
    const matchesSearch =
      p.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
      p.baseUrl.toLowerCase().includes(searchQuery.toLowerCase()) ||
      p.models.some((m) => m.toLowerCase().includes(searchQuery.toLowerCase()));

    const matchesProtocol =
      protocolFilter === 'all' || p.protocol.toLowerCase() === protocolFilter.toLowerCase();

    return matchesSearch && matchesProtocol;
  });

  const getProtocolBadge = (proto: string) => {
    switch (proto.toLowerCase()) {
      case 'anthropic':
        return 'bg-amber-100 dark:bg-amber-950/60 text-amber-800 dark:text-amber-300 border-amber-300/40';
      case 'openai':
        return 'bg-emerald-100 dark:bg-emerald-950/60 text-emerald-800 dark:text-emerald-300 border-emerald-300/40';
      case 'gemini':
        return 'bg-blue-100 dark:bg-blue-950/60 text-blue-800 dark:text-blue-300 border-blue-300/40';
      case 'openrouter':
        return 'bg-purple-100 dark:bg-purple-950/60 text-purple-800 dark:text-purple-300 border-purple-300/40';
      default:
        return 'bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300 border-gray-300/40';
    }
  };

  return (
    <div className="space-y-4">
      {/* 顶部工具栏 */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
        {/* 搜索与协议过滤 */}
        <div className="flex items-center gap-2.5 flex-1 max-w-lg">
          <div className="relative w-full">
            <Search className="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-gray-400" />
            <input
              type="text"
              placeholder="搜索服务商名称、模型或接口地址..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="w-full pl-9 pr-4 py-2 text-sm rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-500"
            />
          </div>

          <select
            value={protocolFilter}
            onChange={(e) => setProtocolFilter(e.target.value)}
            className="px-3 py-2 text-sm rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 shrink-0"
          >
            <option value="all">全部协议</option>
            <option value="anthropic">Anthropic</option>
            <option value="openai">OpenAI</option>
            <option value="gemini">Google Gemini</option>
            <option value="openrouter">OpenRouter</option>
            <option value="ollama">Ollama (本地)</option>
          </select>
        </div>

        {/* 视图切换与主操作按钮组 */}
        <div className="flex items-center gap-2">
          {/* 视图切换 (默认列表) */}
          <div className="flex items-center bg-gray-100 dark:bg-gray-800 p-0.5 rounded-lg border border-gray-200 dark:border-gray-700">
            <button
              onClick={() => setViewMode('list')}
              className={`p-1.5 rounded-md transition-colors ${
                viewMode === 'list'
                  ? 'bg-white dark:bg-gray-700 text-indigo-600 dark:text-indigo-400 shadow-xs'
                  : 'text-gray-500 hover:text-gray-700 dark:hover:text-gray-300'
              }`}
              title="列表视图 (推荐)"
            >
              <LayoutList className="w-4 h-4" />
            </button>
            <button
              onClick={() => setViewMode('card')}
              className={`p-1.5 rounded-md transition-colors ${
                viewMode === 'card'
                  ? 'bg-white dark:bg-gray-700 text-indigo-600 dark:text-indigo-400 shadow-xs'
                  : 'text-gray-500 hover:text-gray-700 dark:hover:text-gray-300'
              }`}
              title="卡片视图"
            >
              <LayoutGrid className="w-4 h-4" />
            </button>
          </div>

          <button
            onClick={handleTestAll}
            disabled={testingId !== null}
            className="flex items-center gap-1.5 px-3 py-2 text-sm font-medium text-gray-700 dark:text-gray-200 bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg hover:bg-gray-50 dark:hover:bg-gray-700/50 transition-colors shadow-xs"
          >
            <RefreshCw className={`w-4 h-4 ${testingId !== null ? 'animate-spin' : ''}`} />
            <span>测速</span>
          </button>

          <button
            onClick={openPresetDrawer}
            className="flex items-center gap-1.5 px-3.5 py-2 text-sm font-medium text-indigo-700 dark:text-indigo-300 bg-indigo-50 dark:bg-indigo-950/60 border border-indigo-200 dark:border-indigo-800 rounded-lg hover:bg-indigo-100 dark:hover:bg-indigo-900/60 transition-colors shadow-xs"
          >
            <Sparkles className="w-4 h-4 text-indigo-600 dark:text-indigo-400" />
            <span>30+ 厂商预设库</span>
          </button>

          <button
            onClick={() => openProviderModal(null)}
            className="flex items-center gap-1.5 px-3.5 py-2 text-sm font-medium text-white bg-indigo-600 rounded-lg hover:bg-indigo-700 transition-colors shadow-xs shadow-indigo-600/20"
          >
            <Plus className="w-4 h-4" />
            <span>添加服务商</span>
          </button>
        </div>
      </div>

      {/* 列表模式展示 (参考 cc-switch 的设计) */}
      {filteredProviders.length === 0 ? (
        <div className="text-center py-16 border-2 border-dashed border-gray-200 dark:border-gray-800 rounded-xl bg-gray-50/50 dark:bg-gray-900/20">
          <Globe className="w-12 h-12 text-gray-400 mx-auto mb-3" />
          <h3 className="text-sm font-semibold text-gray-900 dark:text-gray-100">未检索到任何服务商</h3>
          <p className="text-xs text-gray-500 dark:text-gray-400 mt-1 max-w-sm mx-auto">
            您可以一键导入 30+ 官方与主流厂商预设（Anthropic、OpenAI、DeepSeek、Ollama 等），或手动添加自定义服务商。
          </p>
          <div className="mt-4 flex items-center justify-center gap-3">
            <button
              onClick={openPresetDrawer}
              className="inline-flex items-center gap-1.5 px-4 py-2 text-sm font-medium text-white bg-indigo-600 rounded-lg hover:bg-indigo-700 shadow-xs"
            >
              <Sparkles className="w-4 h-4" />
              <span>打开 30+ 厂商预设库</span>
            </button>
          </div>
        </div>
      ) : viewMode === 'list' ? (
        <div className="border border-gray-200 dark:border-gray-800 rounded-xl overflow-hidden bg-white dark:bg-gray-900 shadow-xs">
          <table className="w-full text-left border-collapse">
            <thead>
              <tr className="border-b border-gray-200 dark:border-gray-800 bg-gray-50/80 dark:bg-gray-800/60 text-[11px] font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wider select-none">
                <th className="py-3 px-4 w-12 text-center">状态</th>
                <th className="py-3 px-4">服务商名称</th>
                <th className="py-3 px-4">协议</th>
                <th className="py-3 px-4">接口地址 (Base URL)</th>
                <th className="py-3 px-4">账号池 Keys</th>
                <th className="py-3 px-4">包含模型</th>
                <th className="py-3 px-4 text-center">延迟</th>
                <th className="py-3 px-4 text-right">操作</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-gray-100 dark:divide-gray-800 text-sm">
              {filteredProviders.map((provider) => {
                const isTesting = testingId === provider.id;

                return (
                  <tr
                    key={provider.id}
                    className={`hover:bg-gray-50/80 dark:hover:bg-gray-800/40 transition-colors ${
                      !provider.enabled ? 'opacity-60 bg-gray-50/30 dark:bg-gray-900/40' : ''
                    }`}
                  >
                    {/* 激活开关 */}
                    <td className="py-3.5 px-4 text-center align-middle">
                      <label className="relative inline-flex items-center cursor-pointer select-none">
                        <input
                          type="checkbox"
                          checked={provider.enabled}
                          onChange={(e) => toggleProvider(provider.id, e.target.checked)}
                          className="sr-only peer"
                        />
                        <div className="w-9 h-5 bg-gray-200 peer-focus:outline-none rounded-full peer dark:bg-gray-700 peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-4 after:w-4 after:transition-all dark:border-gray-600 peer-checked:bg-indigo-600"></div>
                      </label>
                    </td>

                    {/* 服务商名称 */}
                    <td className="py-3.5 px-4 font-medium text-gray-900 dark:text-gray-100 whitespace-nowrap">
                      <div className="flex items-center gap-2">
                        <span className="font-semibold text-sm">{provider.name}</span>
                        {provider.priority > 0 && (
                          <span className="text-[10px] text-gray-400 font-mono">
                            Pri: {provider.priority}
                          </span>
                        )}
                      </div>
                    </td>

                    {/* 协议标签 */}
                    <td className="py-3.5 px-4 whitespace-nowrap">
                      <span
                        className={`text-[11px] font-mono px-2 py-0.5 rounded border uppercase font-medium ${getProtocolBadge(
                          provider.protocol
                        )}`}
                      >
                        {provider.protocol}
                      </span>
                    </td>

                    {/* Base URL */}
                    <td className="py-3.5 px-4">
                      <div className="flex items-center gap-1.5 font-mono text-xs text-gray-600 dark:text-gray-300 max-w-xs truncate">
                        <span className="truncate">{provider.baseUrl}</span>
                        <button
                          onClick={() => handleCopy(provider.id, provider.baseUrl)}
                          className="text-gray-400 hover:text-gray-600 dark:hover:text-gray-200 shrink-0"
                          title="复制接口地址"
                        >
                          {copiedId === provider.id ? (
                            <Check className="w-3.5 h-3.5 text-emerald-500" />
                          ) : (
                            <Copy className="w-3.5 h-3.5" />
                          )}
                        </button>
                      </div>
                    </td>

                    {/* 账号池 Key */}
                    <td className="py-3.5 px-4 whitespace-nowrap">
                      <div className="flex items-center gap-1 text-xs">
                        <Key className="w-3.5 h-3.5 text-indigo-500" />
                        {provider.apiKeys.length === 0 ? (
                          <span className="text-gray-400 italic">未配置 (使用客户端 Key)</span>
                        ) : provider.apiKeys.length === 1 ? (
                          <span className="text-gray-700 dark:text-gray-300 font-mono">1 个 API Key</span>
                        ) : (
                          <span className="text-indigo-600 dark:text-indigo-400 font-mono font-medium">
                            {provider.apiKeys.length} 个 Key (账号池轮询)
                          </span>
                        )}
                      </div>
                    </td>

                    {/* 支持模型 */}
                    <td className="py-3.5 px-4">
                      <div className="flex items-center gap-1 flex-wrap max-w-xs">
                        {provider.models.slice(0, 2).map((m) => (
                          <span
                            key={m}
                            className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-300 truncate max-w-[130px]"
                          >
                            {m}
                          </span>
                        ))}
                        {provider.models.length > 2 && (
                          <span className="text-[10px] font-mono px-1 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-400">
                            +{provider.models.length - 2}
                          </span>
                        )}
                      </div>
                    </td>

                    {/* 延迟 */}
                    <td className="py-3.5 px-4 text-center whitespace-nowrap">
                      {provider.latencyMs !== undefined && provider.latencyMs !== null ? (
                        provider.latencyMs >= 0 ? (
                          <span
                            className={`inline-flex items-center gap-1 text-xs font-mono font-medium px-2 py-0.5 rounded-md ${
                              provider.latencyMs < 300
                                ? 'bg-emerald-50 dark:bg-emerald-950/50 text-emerald-700 dark:text-emerald-300 border border-emerald-200/50 dark:border-emerald-800/40'
                                : provider.latencyMs < 800
                                ? 'bg-amber-50 dark:bg-amber-950/50 text-amber-700 dark:text-amber-300 border border-amber-200/50 dark:border-amber-800/40'
                                : 'bg-red-50 dark:bg-red-950/50 text-red-700 dark:text-red-300 border border-red-200/50 dark:border-red-800/40'
                            }`}
                          >
                            <Zap className="w-3 h-3" />
                            {provider.latencyMs}ms
                          </span>
                        ) : (
                          <span className="text-xs text-red-500 font-mono">离线 / 超时</span>
                        )
                      ) : (
                        <span className="text-xs text-gray-400 font-mono">-- ms</span>
                      )}
                    </td>

                    {/* 操作 */}
                    <td className="py-3.5 px-4 text-right whitespace-nowrap">
                      <div className="flex items-center justify-end gap-1">
                        <button
                          onClick={() => handleTestLatency(provider.id)}
                          disabled={isTesting}
                          className="p-1.5 rounded-md text-gray-400 hover:text-indigo-600 dark:hover:text-indigo-400 hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
                          title="测速"
                        >
                          <RefreshCw className={`w-3.5 h-3.5 ${isTesting ? 'animate-spin' : ''}`} />
                        </button>
                        <button
                          onClick={() => openProviderModal(provider)}
                          className="p-1.5 rounded-md text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
                          title="编辑服务商"
                        >
                          <Edit2 className="w-3.5 h-3.5" />
                        </button>
                        <button
                          onClick={() => deleteProvider(provider.id)}
                          className="p-1.5 rounded-md text-gray-400 hover:text-red-600 dark:hover:text-red-400 hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
                          title="删除服务商"
                        >
                          <Trash2 className="w-3.5 h-3.5" />
                        </button>
                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      ) : (
        /* 卡片备选视图 */
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {filteredProviders.map((provider) => {
            const isTesting = testingId === provider.id;
            return (
              <div
                key={provider.id}
                className={`border rounded-xl p-4 transition-all duration-200 flex flex-col justify-between ${
                  provider.enabled
                    ? 'bg-white dark:bg-gray-900 border-gray-200 dark:border-gray-800 shadow-xs hover:border-indigo-400 dark:hover:border-indigo-700'
                    : 'bg-gray-50/70 dark:bg-gray-900/40 border-gray-200/60 dark:border-gray-800/60 opacity-75'
                }`}
              >
                <div>
                  <div className="flex items-start justify-between gap-2 mb-3">
                    <div className="flex-1 min-w-0">
                      <h3 className="text-base font-semibold text-gray-900 dark:text-gray-100 truncate">
                        {provider.name}
                      </h3>
                      <div className="flex items-center gap-1.5 mt-1">
                        <span
                          className={`text-[10px] font-mono px-2 py-0.5 rounded border uppercase font-medium ${getProtocolBadge(
                            provider.protocol
                          )}`}
                        >
                          {provider.protocol}
                        </span>
                        <span className="text-[11px] text-gray-500 dark:text-gray-400 font-mono">
                          优先级: {provider.priority}
                        </span>
                      </div>
                    </div>

                    <label className="relative inline-flex items-center cursor-pointer select-none">
                      <input
                        type="checkbox"
                        checked={provider.enabled}
                        onChange={(e) => toggleProvider(provider.id, e.target.checked)}
                        className="sr-only peer"
                      />
                      <div className="w-9 h-5 bg-gray-200 peer-focus:outline-none rounded-full peer dark:bg-gray-700 peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-4 after:w-4 after:transition-all dark:border-gray-600 peer-checked:bg-indigo-600"></div>
                    </label>
                  </div>

                  <div className="flex items-center justify-between text-xs bg-gray-50 dark:bg-gray-800/60 rounded-lg p-2 mb-3 border border-gray-100 dark:border-gray-800 font-mono text-gray-600 dark:text-gray-300">
                    <span className="truncate flex-1">{provider.baseUrl}</span>
                    <button
                      onClick={() => handleCopy(provider.id, provider.baseUrl)}
                      className="ml-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-200"
                      title="复制地址"
                    >
                      {copiedId === provider.id ? (
                        <Check className="w-3.5 h-3.5 text-emerald-500" />
                      ) : (
                        <Copy className="w-3.5 h-3.5" />
                      )}
                    </button>
                  </div>

                  <div className="flex flex-wrap gap-1 mb-4">
                    {provider.models.slice(0, 3).map((model) => (
                      <span
                        key={model}
                        className="text-[11px] font-mono px-2 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-300 truncate max-w-[140px]"
                      >
                        {model}
                      </span>
                    ))}
                    {provider.models.length > 3 && (
                      <span className="text-[11px] font-mono px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-500">
                        +{provider.models.length - 3}
                      </span>
                    )}
                  </div>
                </div>

                <div className="pt-3 border-t border-gray-100 dark:border-gray-800/80 flex items-center justify-between">
                  <div>
                    {provider.latencyMs !== undefined && provider.latencyMs !== null ? (
                      <span className="text-xs font-mono font-medium text-emerald-600">
                        {provider.latencyMs}ms
                      </span>
                    ) : (
                      <span className="text-xs text-gray-400 font-mono">-- ms</span>
                    )}
                  </div>
                  <div className="flex items-center gap-1">
                    <button
                      onClick={() => handleTestLatency(provider.id)}
                      disabled={isTesting}
                      className="p-1.5 rounded-md text-gray-400 hover:text-indigo-600"
                    >
                      <RefreshCw className={`w-3.5 h-3.5 ${isTesting ? 'animate-spin' : ''}`} />
                    </button>
                    <button
                      onClick={() => openProviderModal(provider)}
                      className="p-1.5 rounded-md text-gray-400 hover:text-gray-700"
                    >
                      <Edit2 className="w-3.5 h-3.5" />
                    </button>
                    <button
                      onClick={() => deleteProvider(provider.id)}
                      className="p-1.5 rounded-md text-gray-400 hover:text-red-600"
                    >
                      <Trash2 className="w-3.5 h-3.5" />
                    </button>
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
};
