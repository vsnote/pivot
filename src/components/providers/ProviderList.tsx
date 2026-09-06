import React, { useState } from 'react';
import { useAppStore } from '@/store/useAppStore';
import { Provider } from '@/types';
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
  Layers,
  Search,
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

  return (
    <div className="space-y-6">
      {/* Top Toolbar */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
        {/* Search & Filters */}
        <div className="flex items-center gap-3 flex-1 max-w-md">
          <div className="relative w-full">
            <Search className="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-gray-400" />
            <input
              type="text"
              placeholder="Search providers, models, URLs..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="w-full pl-9 pr-4 py-2 text-sm rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-500"
            />
          </div>

          <select
            value={protocolFilter}
            onChange={(e) => setProtocolFilter(e.target.value)}
            className="px-3 py-2 text-sm rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 focus:outline-none focus:ring-2 focus:ring-indigo-500/20"
          >
            <option value="all">All Protocols</option>
            <option value="anthropic">Anthropic</option>
            <option value="openai">OpenAI</option>
            <option value="gemini">Gemini</option>
            <option value="openrouter">OpenRouter</option>
            <option value="ollama">Ollama</option>
          </select>
        </div>

        {/* Action Buttons */}
        <div className="flex items-center gap-2">
          <button
            onClick={handleTestAll}
            disabled={testingId !== null}
            className="flex items-center gap-1.5 px-3 py-2 text-sm font-medium text-gray-700 dark:text-gray-200 bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg hover:bg-gray-50 dark:hover:bg-gray-700/50 transition-colors shadow-sm"
          >
            <RefreshCw className={`w-4 h-4 ${testingId !== null ? 'animate-spin' : ''}`} />
            <span>Test Latencies</span>
          </button>

          <button
            onClick={openPresetDrawer}
            className="flex items-center gap-1.5 px-3.5 py-2 text-sm font-medium text-indigo-700 dark:text-indigo-300 bg-indigo-50 dark:bg-indigo-950/60 border border-indigo-200 dark:border-indigo-800 rounded-lg hover:bg-indigo-100 dark:hover:bg-indigo-900/60 transition-colors shadow-sm"
          >
            <Sparkles className="w-4 h-4 text-indigo-600 dark:text-indigo-400" />
            <span>30+ Presets</span>
          </button>

          <button
            onClick={() => openProviderModal(null)}
            className="flex items-center gap-1.5 px-4 py-2 text-sm font-medium text-white bg-indigo-600 rounded-lg hover:bg-indigo-700 transition-colors shadow-sm shadow-indigo-600/20"
          >
            <Plus className="w-4 h-4" />
            <span>Add Provider</span>
          </button>
        </div>
      </div>

      {/* Provider Card Grid */}
      {filteredProviders.length === 0 ? (
        <div className="text-center py-16 border-2 border-dashed border-gray-200 dark:border-gray-800 rounded-xl bg-gray-50/50 dark:bg-gray-900/20">
          <Globe className="w-12 h-12 text-gray-400 mx-auto mb-3" />
          <h3 className="text-sm font-semibold text-gray-900 dark:text-gray-100">No providers found</h3>
          <p className="text-xs text-gray-500 dark:text-gray-400 mt-1 max-w-sm mx-auto">
            Get started by importing from our 30+ vendor presets (Anthropic, OpenAI, DeepSeek, Ollama...) or adding a custom provider.
          </p>
          <div className="mt-4 flex items-center justify-center gap-3">
            <button
              onClick={openPresetDrawer}
              className="inline-flex items-center gap-1.5 px-4 py-2 text-sm font-medium text-white bg-indigo-600 rounded-lg hover:bg-indigo-700 shadow-sm"
            >
              <Sparkles className="w-4 h-4" />
              <span>Explore 30+ Presets</span>
            </button>
          </div>
        </div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {filteredProviders.map((provider) => (
            <ProviderCard
              key={provider.id}
              provider={provider}
              testingId={testingId}
              copiedId={copiedId}
              onTestLatency={handleTestLatency}
              onCopy={handleCopy}
              onToggle={toggleProvider}
              onEdit={openProviderModal}
              onDelete={deleteProvider}
            />
          ))}
        </div>
      )}
    </div>
  );
};

interface ProviderCardProps {
  provider: Provider;
  testingId: string | null;
  copiedId: string | null;
  onTestLatency: (id: string) => void;
  onCopy: (id: string, text: string) => void;
  onToggle: (id: string, enabled: boolean) => void;
  onEdit: (p: Provider) => void;
  onDelete: (id: string) => void;
}

const ProviderCard: React.FC<ProviderCardProps> = ({
  provider,
  testingId,
  copiedId,
  onTestLatency,
  onCopy,
  onToggle,
  onEdit,
  onDelete,
}) => {
  const isTesting = testingId === provider.id;

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
    <div
      className={`border rounded-xl p-4 transition-all duration-200 flex flex-col justify-between ${
        provider.enabled
          ? 'bg-white dark:bg-gray-900 border-gray-200 dark:border-gray-800 shadow-sm hover:border-indigo-400 dark:hover:border-indigo-700'
          : 'bg-gray-50/70 dark:bg-gray-900/40 border-gray-200/60 dark:border-gray-800/60 opacity-75'
      }`}
    >
      <div>
        {/* Header with Title, Protocol, and Toggle */}
        <div className="flex items-start justify-between gap-2 mb-3">
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2">
              <h3 className="text-base font-semibold text-gray-900 dark:text-gray-100 truncate">
                {provider.name}
              </h3>
            </div>
            <div className="flex items-center gap-1.5 mt-1">
              <span
                className={`text-[10px] font-mono px-2 py-0.5 rounded border uppercase font-medium ${getProtocolBadge(
                  provider.protocol
                )}`}
              >
                {provider.protocol}
              </span>
              <span className="text-[11px] text-gray-500 dark:text-gray-400 font-mono">
                Pri: {provider.priority}
              </span>
            </div>
          </div>

          {/* Toggle Switch */}
          <label className="relative inline-flex items-center cursor-pointer select-none">
            <input
              type="checkbox"
              checked={provider.enabled}
              onChange={(e) => onToggle(provider.id, e.target.checked)}
              className="sr-only peer"
            />
            <div className="w-10 h-5 bg-gray-200 peer-focus:outline-none rounded-full peer dark:bg-gray-700 peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-4 after:w-4 after:transition-all dark:border-gray-600 peer-checked:bg-indigo-600"></div>
          </label>
        </div>

        {/* Base URL */}
        <div className="flex items-center justify-between text-xs bg-gray-50 dark:bg-gray-800/60 rounded-lg p-2 mb-3 border border-gray-100 dark:border-gray-800 font-mono text-gray-600 dark:text-gray-300">
          <span className="truncate flex-1">{provider.baseUrl}</span>
          <button
            onClick={() => onCopy(provider.id, provider.baseUrl)}
            className="ml-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-200"
            title="Copy URL"
          >
            {copiedId === provider.id ? (
              <Check className="w-3.5 h-3.5 text-emerald-500" />
            ) : (
              <Copy className="w-3.5 h-3.5" />
            )}
          </button>
        </div>

        {/* Keys & Models summary */}
        <div className="grid grid-cols-2 gap-2 text-xs text-gray-500 dark:text-gray-400 mb-3">
          <div className="flex items-center gap-1.5 bg-gray-50/80 dark:bg-gray-800/40 p-1.5 rounded-md border border-gray-100 dark:border-gray-800/80">
            <Key className="w-3.5 h-3.5 text-indigo-500" />
            <span className="truncate">
              {provider.apiKeys.length === 0
                ? 'No Keys'
                : `${provider.apiKeys.length} Key${provider.apiKeys.length > 1 ? 's (Pool)' : ''}`}
            </span>
          </div>

          <div className="flex items-center gap-1.5 bg-gray-50/80 dark:bg-gray-800/40 p-1.5 rounded-md border border-gray-100 dark:border-gray-800/80">
            <Layers className="w-3.5 h-3.5 text-violet-500" />
            <span className="truncate">{provider.models.length} Models</span>
          </div>
        </div>

        {/* Models Tag Pills */}
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

      {/* Footer with Latency and Action Buttons */}
      <div className="pt-3 border-t border-gray-100 dark:border-gray-800/80 flex items-center justify-between">
        {/* Latency badge */}
        <div>
          {provider.latencyMs !== undefined && provider.latencyMs !== null ? (
            provider.latencyMs >= 0 ? (
              <span
                className={`inline-flex items-center gap-1 text-xs font-mono font-medium px-2 py-0.5 rounded-md ${
                  provider.latencyMs < 300
                    ? 'bg-emerald-50 dark:bg-emerald-950/50 text-emerald-700 dark:text-emerald-300'
                    : provider.latencyMs < 800
                    ? 'bg-amber-50 dark:bg-amber-950/50 text-amber-700 dark:text-amber-300'
                    : 'bg-red-50 dark:bg-red-950/50 text-red-700 dark:text-red-300'
                }`}
              >
                <Zap className="w-3 h-3" />
                {provider.latencyMs}ms
              </span>
            ) : (
              <span className="text-xs text-red-500 font-mono">Unreachable</span>
            )
          ) : (
            <span className="text-xs text-gray-400 font-mono">-- ms</span>
          )}
        </div>

        {/* Actions */}
        <div className="flex items-center gap-1">
          <button
            onClick={() => onTestLatency(provider.id)}
            disabled={isTesting}
            className="p-1.5 rounded-md text-gray-500 hover:text-indigo-600 dark:text-gray-400 dark:hover:text-indigo-400 hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
            title="Test Latency"
          >
            <RefreshCw className={`w-3.5 h-3.5 ${isTesting ? 'animate-spin' : ''}`} />
          </button>
          <button
            onClick={() => onEdit(provider)}
            className="p-1.5 rounded-md text-gray-500 hover:text-gray-800 dark:text-gray-400 dark:hover:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
            title="Edit Provider"
          >
            <Edit2 className="w-3.5 h-3.5" />
          </button>
          <button
            onClick={() => onDelete(provider.id)}
            className="p-1.5 rounded-md text-gray-500 hover:text-red-600 dark:text-gray-400 dark:hover:text-red-400 hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
            title="Delete Provider"
          >
            <Trash2 className="w-3.5 h-3.5" />
          </button>
        </div>
      </div>
    </div>
  );
};
