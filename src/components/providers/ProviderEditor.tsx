import React, { useEffect, useRef, useState, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  ArrowLeft,
  Check,
  Eye,
  EyeOff,
  LoaderCircle,
  Plus,
  RefreshCw,
  Search,
  X,
  AlertCircle,
  AlertTriangle,
  HelpCircle,
  Zap,
} from 'lucide-react';
import { ConfirmDialog } from '@/components/common/ConfirmDialog';
import { Button } from '@/components/common/Button';
import { Input } from '@/components/common/Input';
import { Badge } from '@/components/common/Badge';
import {
  ProviderAvatar,
} from '@/components/common/BrandIcons';
import { useAppStore } from '@/store/useAppStore';
import type { Preset, Protocol, Provider } from '@/types';
import { Select } from '@/components/common/Select';
import { Switch } from '@/components/common/Switch';
import {
  applyProviderPreset,
  createProviderDraft,
  editablePresetUrl,
  hasActualUserChanges,
  prepareProvider,
  uniqueValues,
  validateBaseUrl,
} from './providerDraft';

const protocols: Record<Protocol, string> = {
  openai: 'OpenAI 兼容',
  anthropic: 'Anthropic',
  gemini: 'Google Gemini',
  openrouter: 'OpenRouter',
  ollama: 'Ollama',
};

export function ProviderEditor() {
  const { editingProvider, presets, providers, closeProviderEditor, saveProvider, deleteProvider, activeTab } =
    useAppStore();
  // 还没有可用的默认服务商时，新添加的第一个服务商默认承担该角色。
  // 路由只在已启用的服务商里找默认目标，所以停用的默认不算数；
  // 与后端第三方导入路径 is_default: existing.is_empty() 保持一致。
  const [autoDefault] = useState(
    () => !editingProvider && !providers.some((provider) => provider.isDefault && provider.enabled),
  );
  const [initial] = useState(() => createProviderDraft(editingProvider, presets, autoDefault));
  const [draft, setDraft] = useState(initial);
  const [choosing, setChoosing] = useState(!editingProvider);
  const [presetSearch, setPresetSearch] = useState('');
  const [showKeys, setShowKeys] = useState(false);
  const [showAuthHelp, setShowAuthHelp] = useState(false);
  const [fetching, setFetching] = useState(false);
  const [fetched, setFetched] = useState<string[] | null>(null);
  const [fetchError, setFetchError] = useState('');
  const [notice, setNotice] = useState('');
  const [modelSearch, setModelSearch] = useState('');
  const [manualModel, setManualModel] = useState('');
  const [saving, setSaving] = useState(false);
  const [confirmation, setConfirmation] = useState<{ kind: 'leave' | 'preset'; preset?: Preset } | null>(null);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [testingConnection, setTestingConnection] = useState(false);
  const [latencyResult, setLatencyResult] = useState<{ success: boolean; ms?: number; error?: string } | null>(null);
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const [deleting, setDeleting] = useState(false);

  const requestId = useRef(0);
  const drawerRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);

  const preset = presets.find((p) => p.id === draft.presetId);
  const urlEditable = !preset || editablePresetUrl(preset);
  const hasUserChanges = hasActualUserChanges(
    draft,
    initial,
    editingProvider,
    preset,
    manualModel,
  );

  const keyInputs = draft.apiKeys.length ? draft.apiKeys : [''];

  const handleBack = () => {
    if (saving) return;
    if (confirmation) {
      setConfirmation(null);
      return;
    }
    // 1. 若在预设网格页面，返回即关闭编辑器退回主页
    if (choosing) {
      closeProviderEditor();
      return;
    }
    // 2. 若是新增服务商且已进入配置页：返回应该是重选预设
    if (!editingProvider) {
      if (!hasUserChanges) {
        setChoosing(true);
        return;
      }
      setConfirmation({ kind: 'preset' });
      return;
    }
    // 3. 若是编辑已有服务商
    if (!hasUserChanges) {
      closeProviderEditor();
      return;
    }
    setConfirmation({ kind: 'leave' });
  };

  const handleClose = () => {
    if (saving) return;
    if (confirmation) {
      setConfirmation(null);
      return;
    }
    if (!hasUserChanges) {
      closeProviderEditor();
      return;
    }
    setConfirmation({ kind: 'leave' });
  };

  const handleConfirmAbandon = () => {
    if (!confirmation) return;
    if (confirmation.kind === 'preset') {
      setConfirmation(null);
      setChoosing(true);
    } else {
      setConfirmation(null);
      closeProviderEditor();
    }
  };

  useEffect(() => () => {
    requestId.current++;
  }, []);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (
        activeTab !== 'providers' ||
        event.key !== 'Escape' ||
        event.defaultPrevented ||
        event.target instanceof HTMLSelectElement
      )
        return;
      event.preventDefault();
      if (confirmation) setConfirmation(null);
      else handleBack();
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  });

  const invalidateModels = () => {
    requestId.current++;
    setFetching(false);
    setFetched(null);
    setFetchError('');
  };

  const update = (patch: Partial<Provider>) => {
    setDraft((current) => ({ ...current, ...patch }));
    setErrors({});
  };

  const choosePreset = (next?: Preset, confirmed = false) => {
    if (
      !confirmed &&
      (draft.apiKeys.some((key) => key.trim()) || draft.models.length > 0 || manualModel.trim())
    ) {
      setConfirmation({ kind: 'preset', preset: next });
      return;
    }
    setConfirmation(null);
    invalidateModels();
    setDraft(applyProviderPreset(draft, next, autoDefault));
    setShowKeys(false);
    setShowAuthHelp(false);
    setManualModel('');
    setModelSearch('');
    setErrors({});
    setNotice('');
    setChoosing(false);
    contentRef.current?.scrollTo(0, 0);
  };

  const changeUrl = (baseUrl: string) => {
    let sameOrigin = false;
    try {
      sameOrigin = new URL(baseUrl).origin === new URL(draft.baseUrl).origin;
    } catch {
      /* ignore */
    }
    invalidateModels();
    setLatencyResult(null);
    const clearKeys = !sameOrigin && draft.apiKeys.some((key) => key.trim());
    update({ baseUrl, latencyMs: undefined, ...(clearKeys ? { apiKeys: [] } : {}) });
    setNotice(
      clearKeys
        ? '接口主机已更改，原密钥已清除，请填写此服务商的新密钥。'
        : '连接信息已更改，已选模型保留，建议重新获取模型进行验证。',
    );
  };

  const testConnection = async () => {
    const error = validateBaseUrl(draft.baseUrl);
    if (error) {
      setErrors((prev) => ({ ...prev, baseUrl: error }));
      return;
    }
    setTestingConnection(true);
    setLatencyResult(null);
    try {
      const ms = await invoke<number>('test_endpoint_latency', {
        protocol: draft.protocol,
        baseUrl: draft.baseUrl.trim(),
        apiKey: uniqueValues(draft.apiKeys)[0] ?? null,
      });
      setLatencyResult({ success: true, ms });
    } catch (err) {
      setLatencyResult({ success: false, error: String(err) });
    } finally {
      setTestingConnection(false);
    }
  };

  const fetchModels = async () => {
    const error = validateBaseUrl(draft.baseUrl);
    if (error) {
      setErrors({ baseUrl: error });
      return;
    }
    const current = ++requestId.current;
    setFetching(true);
    setFetchError('');
    try {
      const models = await invoke<string[]>('fetch_provider_models', {
        protocol: draft.protocol,
        baseUrl: draft.baseUrl.trim(),
        apiKey: uniqueValues(draft.apiKeys)[0] ?? '',
      });
      if (requestId.current !== current) return;
      setFetched(uniqueValues(models));
      setNotice('');
    } catch (error) {
      if (requestId.current === current) setFetchError(String(error));
    } finally {
      if (requestId.current === current) setFetching(false);
    }
  };

  const replaceModels = (models: string[]) => {
    const next = uniqueValues(models);
    update({
      models: next,
      defaultModel: draft.defaultModel && next.includes(draft.defaultModel) ? draft.defaultModel : null,
    });
  };

  const toggleModel = (model: string) => {
    replaceModels(
      draft.models.includes(model)
        ? draft.models.filter((item) => item !== model)
        : [...draft.models, model],
    );
  };

  const addManualModel = () => {
    const model = manualModel.trim();
    if (!model || /[\s,，]/u.test(model)) {
      setErrors({ models: '请输入一个完整的模型 ID，不要包含空格或逗号' });
      return;
    }
    replaceModels([...draft.models, model]);
    setManualModel('');
  };

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (saving) return;

    const nextErrors: Record<string, string> = {};
    if (!draft.name.trim()) nextErrors.name = '请输入服务商名称';
    const urlError = validateBaseUrl(draft.baseUrl);
    if (urlError) nextErrors.baseUrl = urlError;
    if (!draft.models.length) nextErrors.models = '请至少选择或手动添加一个模型';
    if (manualModel.trim()) nextErrors.models = '请先点击添加正在输入的模型，或清空输入框';
    const defaultModel = draft.defaultModel?.trim() ?? '';
    if (/[\s,，]/u.test(defaultModel))
      nextErrors.defaultModel = '请输入一个完整的模型 ID，不要包含空格或逗号';

    setErrors(nextErrors);
    if (Object.keys(nextErrors).length) {
      requestAnimationFrame(() =>
        drawerRef.current?.querySelector<HTMLElement>('[aria-invalid="true"]')?.focus(),
      );
      return;
    }

    setSaving(true);
    try {
      await saveProvider(prepareProvider(draft));
      closeProviderEditor();
    } catch (error) {
      setErrors({ save: String(error) });
    } finally {
      setSaving(false);
    }
  };

  const matchingPresets = useMemo(() => {
    const q = presetSearch.trim().toLowerCase();
    if (!q) return presets;
    return presets.filter((p) => `${p.name} ${p.id}`.toLowerCase().includes(q));
  }, [presets, presetSearch]);

  // 模型池总览：包含从上游拉取的所有模型以及当前已选/手动添加的模型
  const allModels = useMemo(
    () => uniqueValues([...(fetched ?? []), ...draft.models]),
    [fetched, draft.models]
  );

  const filteredModels = useMemo(() => {
    const q = modelSearch.toLowerCase();
    return allModels.filter((model) => model.toLowerCase().includes(q));
  }, [allModels, modelSearch]);

  const selectAllFiltered = () => {
    replaceModels([...draft.models, ...filteredModels]);
  };

  const clearAllFiltered = () => {
    replaceModels(draft.models.filter((m) => !filteredModels.includes(m)));
  };

  const invalidModels = useMemo(() => {
    if (!fetched || fetched.length === 0) return [];
    return draft.models.filter((model) => !fetched.includes(model));
  }, [fetched, draft.models]);

  const removeInvalidModels = () => {
    if (!fetched || fetched.length === 0) return;
    replaceModels(draft.models.filter((model) => fetched.includes(model)));
  };

  const otherDefault = providers.find((provider) => provider.isDefault && provider.id !== draft.id);

  return (
    <div className="w-full pb-6 space-y-3 animate-in fade-in duration-150">
      {/* 顶部面包屑返回导航：去除非弹窗的关闭按钮，清晰指示层级 */}
      <div className="flex items-center justify-between pb-1">
        {confirmation ? (
          <div className="flex items-center gap-2 px-2.5 py-1 -ml-1 rounded-md bg-rose-50 dark:bg-rose-950/40 border border-rose-200/80 dark:border-rose-900/50 animate-in fade-in duration-150">
            <span className="text-xs font-medium text-rose-700 dark:text-rose-300">
              {confirmation.kind === 'preset'
                ? '放弃已填内容并重选预设？'
                : '存在未保存的修改，确认放弃？'}
            </span>
            <button
              type="button"
              onClick={handleConfirmAbandon}
              className="inline-flex h-7 items-center justify-center rounded-md bg-rose-600 px-2.5 text-xs font-semibold text-white transition-colors hover:bg-rose-700 cursor-pointer"
            >
              确认放弃
            </button>
            <button
              type="button"
              onClick={() => setConfirmation(null)}
              className="inline-flex h-7 items-center justify-center rounded-md px-2.5 text-xs font-medium text-zinc-600 transition-colors hover:bg-white dark:text-zinc-400 dark:hover:bg-zinc-800 cursor-pointer"
            >
              继续编辑
            </button>
          </div>
        ) : (
          <nav aria-label="页面层级" className="flex items-center gap-1.5 text-xs">
            <button
              type="button"
              onClick={handleBack}
              className="-ml-2 inline-flex h-7 items-center gap-1 rounded-md px-2 text-zinc-500 hover:text-zinc-900 dark:text-zinc-400 dark:hover:text-zinc-100 hover:bg-zinc-100 dark:hover:bg-zinc-800 transition-colors cursor-pointer"
              title="返回服务商列表"
            >
              <ArrowLeft className="w-3.5 h-3.5" />
              <span>服务商</span>
            </button>
            <span className="text-zinc-300 dark:text-zinc-700 select-none">/</span>
            <span className="font-semibold text-zinc-800 dark:text-zinc-200 truncate max-w-[260px]">
              {choosing
                ? '选择预设'
                : draft.name.trim() || (editingProvider ? '未命名服务商' : '新增服务商')}
            </span>
          </nav>
        )}
      </div>

      {/* 页面主卡片 */}
      <div
        ref={drawerRef}
        role="region"
        aria-label={editingProvider ? '编辑服务商' : '添加服务商'}
        className="p-5 rounded-lg border border-border bg-card text-card-foreground shadow-2xs space-y-4"
      >
        {/* 表单内容 */}
        <div ref={contentRef} className="space-y-6">
          {choosing ? (
            /* 选择服务商预设 */
            <div className="space-y-3">
              <div className="relative">
                <Search className="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-zinc-400 pointer-events-none" />
                <Input
                  autoFocus
                  type="text"
                  placeholder="搜索服务商预设，如 Anthropic, OpenAI, DeepSeek..."
                  value={presetSearch}
                  onChange={(e) => setPresetSearch(e.target.value)}
                  className="pl-9 pr-8 h-8.5 text-xs"
                />
                {presetSearch && (
                  <button
                    type="button"
                    onClick={() => setPresetSearch('')}
                    className="absolute right-2.5 top-1/2 -translate-y-1/2 text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-200 p-0.5 cursor-pointer"
                    title="清空"
                  >
                    <X className="w-3.5 h-3.5" />
                  </button>
                )}
              </div>

              {matchingPresets.length === 0 ? (
                <div className="p-8 text-center space-y-2 rounded-lg border border-border bg-muted/10">
                  <p className="text-xs text-zinc-400">没有匹配的服务商预设</p>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={() => choosePreset()}
                  >
                    <Plus className="w-3.5 h-3.5" />
                    <span>创建自定义服务商</span>
                  </Button>
                </div>
              ) : (
                <div className="grid grid-cols-2 sm:grid-cols-3 gap-2">
                  {/* 自定义连接 */}
                  <button
                    type="button"
                    onClick={() => choosePreset()}
                    className="h-11 px-3 rounded-lg border border-dashed border-border hover:border-zinc-400 dark:hover:border-zinc-500 bg-muted/20 hover:bg-muted/40 flex items-center gap-2.5 text-left transition-colors cursor-pointer group"
                  >
                    <div className="w-7 h-7 rounded-md bg-muted flex items-center justify-center shrink-0 text-muted-foreground group-hover:text-foreground">
                      <Plus className="w-4 h-4" />
                    </div>
                    <span className="text-xs font-medium text-foreground truncate">
                      自定义服务商
                    </span>
                  </button>

                  {matchingPresets.map((p) => {
                    const cleanName = p.name.replace(/[\(（][^\)）]+[\)）]/g, '').trim();
                    return (
                      <button
                        key={p.id}
                        type="button"
                        onClick={() => choosePreset(p)}
                        className="h-11 px-3 rounded-lg border border-border bg-card hover:bg-accent/50 hover:border-zinc-300 dark:hover:border-zinc-700 flex items-center gap-2.5 text-left transition-colors cursor-pointer group shadow-2xs"
                      >
                        <ProviderAvatar
                          name={cleanName}
                          protocol={p.protocol}
                          iconKey={p.icon || p.id}
                          sizeClass="w-7 h-7"
                        />
                        <span className="text-xs font-medium text-foreground truncate">
                          {cleanName}
                        </span>
                      </button>
                    );
                  })}
                </div>
              )}
            </div>
          ) : (
            /* 配置表单 */
            <form id="provider-drawer-form" onSubmit={submit} className="space-y-4">
              {/* 基础连接设置 */}
              <div className="space-y-3.5">
                <h3 className="text-xs font-bold text-zinc-900 dark:text-zinc-100 uppercase tracking-wider">
                  基础连接信息
                </h3>

                {/* 服务商名称 */}
                <div>
                  <label className="block text-xs font-medium text-zinc-700 dark:text-zinc-300 mb-1">
                    服务商名称
                  </label>
                  <Input
                    type="text"
                    value={draft.name}
                    onChange={(e) => update({ name: e.target.value })}
                    placeholder="例如 Anthropic Claude、DeepSeek 官方"
                  />
                  {errors.name && <p className="text-[11px] text-rose-500 mt-1">{errors.name}</p>}
                </div>

                {/* 协议与 URL */}
                <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
                  <div>
                    <label className="block text-xs font-medium text-zinc-700 dark:text-zinc-300 mb-1">
                      协议格式
                    </label>
                    {preset ? (
                      <div className="px-3 py-2 text-xs font-mono rounded-md border border-zinc-200 dark:border-zinc-800 bg-zinc-50 dark:bg-zinc-800/50 text-zinc-700 dark:text-zinc-300">
                        {protocols[draft.protocol]}
                      </div>
                    ) : (
                      <Select
                        value={draft.protocol}
                        onChange={(e) => {
                          invalidateModels();
                          update({
                            protocol: e.target.value as Protocol,
                            apiKeys: [],
                            models: [],
                          });
                          setNotice('协议已更改，请重新填写密钥并选择可用模型。');
                        }}
                        containerClassName="w-full"
                      >
                        {Object.entries(protocols).map(([id, label]) => (
                          <option key={id} value={id}>
                            {label}
                          </option>
                        ))}
                      </Select>
                    )}
                  </div>

                  <div className="sm:col-span-2">
                    <div className="flex items-center justify-between mb-1">
                      <label className="block text-xs font-medium text-zinc-700 dark:text-zinc-300">
                        接口基地址 (Base URL)
                      </label>
                      {draft.baseUrl.trim() && (
                        <div className="flex items-center gap-1.5 text-xs">
                          {testingConnection ? (
                            <span className="inline-flex items-center gap-1 text-[11px] text-zinc-400">
                              <LoaderCircle className="w-3 h-3 animate-spin text-zinc-500" />
                              <span>测试中…</span>
                            </span>
                          ) : latencyResult ? (
                            <span
                              className={`inline-flex items-center gap-1 text-[11px] font-mono ${
                                latencyResult.success
                                  ? 'text-emerald-600 dark:text-emerald-400 font-medium'
                                  : 'text-rose-500 dark:text-rose-400'
                              }`}
                            >
                              {latencyResult.success ? (
                                <>
                                  <Zap className="w-3 h-3 shrink-0" />
                                  <span>{latencyResult.ms} ms</span>
                                </>
                              ) : (
                                <span title={latencyResult.error} className="truncate max-w-[130px]">
                                  {latencyResult.error}
                                </span>
                              )}
                              <button
                                type="button"
                                onClick={testConnection}
                                className="text-[10px] text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-200 underline ml-0.5 cursor-pointer"
                              >
                                重测
                              </button>
                            </span>
                          ) : (
                            <button
                              type="button"
                              onClick={testConnection}
                              className="text-[11px] text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100 flex items-center gap-0.5 cursor-pointer transition-colors"
                            >
                              <Zap className="w-3 h-3 shrink-0" />
                              <span>测试连接</span>
                            </button>
                          )}
                        </div>
                      )}
                    </div>
                    <Input
                      type="text"
                      value={draft.baseUrl}
                      onChange={(e) => changeUrl(e.target.value)}
                      disabled={!urlEditable}
                      placeholder="https://api.example.com/v1"
                      className="font-mono"
                    />
                    {errors.baseUrl && (
                      <p className="text-[11px] text-rose-500 mt-1">{errors.baseUrl}</p>
                    )}
                  </div>
                </div>

                {/* API 密钥 */}
                <div>
                  <div className="flex items-center justify-between mb-1.5">
                    <label className="block text-xs font-medium text-zinc-700 dark:text-zinc-300">
                      API 密钥 (Key)
                    </label>
                    <div className="flex items-center gap-2 text-[11px]">
                      <button
                        type="button"
                        onClick={() => setShowAuthHelp(!showAuthHelp)}
                        className="text-zinc-400 hover:text-zinc-700 dark:hover:text-zinc-200 flex items-center gap-1 cursor-pointer transition-colors"
                      >
                        <HelpCircle className="w-3 h-3" />
                        <span>密钥说明</span>
                      </button>
                      <span className="text-zinc-300 dark:text-zinc-700">·</span>
                      <button
                        type="button"
                        onClick={() => update({ apiKeys: [...keyInputs, ''] })}
                        className="text-zinc-500 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100 font-medium cursor-pointer transition-colors"
                      >
                        + 多密钥轮询
                      </button>
                    </div>
                  </div>

                  {showAuthHelp && (
                    <div className="mb-2 p-2.5 rounded-lg bg-zinc-50 dark:bg-zinc-800 text-[11px] text-zinc-600 dark:text-zinc-300 leading-relaxed border border-zinc-200/60 dark:border-zinc-700/60">
                      密钥保存在本地 SQLite 中。留空时默认沿用客户端传入的密钥；本地模型（如 Ollama）通常无需填写。
                    </div>
                  )}

                  <div className="space-y-2">
                    {keyInputs.map((key, index) => (
                      <div key={index} className="flex items-center gap-2">
                        <div className="relative flex-1">
                          <Input
                            type={showKeys ? 'text' : 'password'}
                            value={key}
                            onChange={(e) => {
                              const keys = [...keyInputs];
                              keys[index] = e.target.value;
                              invalidateModels();
                              update({ apiKeys: keys });
                            }}
                            placeholder={index ? '添加备用密钥' : 'sk-ant-... / sk-...（可留空）'}
                            className="font-mono pr-9"
                          />
                          <button
                            type="button"
                            onClick={() => setShowKeys(!showKeys)}
                            className="absolute right-2.5 top-1/2 -translate-y-1/2 text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-200 cursor-pointer"
                            aria-label={showKeys ? '隐藏密钥' : '显示明文密钥'}
                          >
                            {showKeys ? <EyeOff className="w-3.5 h-3.5" /> : <Eye className="w-3.5 h-3.5" />}
                          </button>
                        </div>

                        {keyInputs.length > 1 && (
                          <button
                            type="button"
                            onClick={() => {
                              invalidateModels();
                              update({ apiKeys: keyInputs.filter((_, i) => i !== index) });
                            }}
                            className="p-2 rounded-lg text-zinc-400 hover:text-rose-500 hover:bg-zinc-100 dark:hover:bg-zinc-800 cursor-pointer transition-colors"
                          >
                            <X className="w-3.5 h-3.5" />
                          </button>
                        )}
                      </div>
                    ))}
                  </div>

                  {notice && <p className="text-[11px] text-amber-600 dark:text-amber-400 mt-1">{notice}</p>}
                </div>

                {/* 设为全局默认服务商 */}
                <div className="pt-3 border-t border-border">
                  <div className="flex items-center justify-between gap-4">
                    <div>
                      <span className="text-xs font-medium text-zinc-800 dark:text-zinc-200">
                        设为全局默认服务商
                      </span>
                      <span className="text-[11px] text-zinc-400 dark:text-zinc-500 ml-1.5">
                        {otherDefault && !draft.isDefault
                          ? `(当前默认: ${otherDefault.name})`
                          : '(未命中任何规则或声明模型时默认路由至此)'}
                      </span>
                    </div>
                    <Switch
                      checked={!!draft.isDefault}
                      onCheckedChange={(checked) => update({ isDefault: checked, defaultModel: null })}
                      ariaLabel="设为全局默认服务商"
                    />
                  </div>
                </div>
              </div>

              {/* 模型选择与声明：一体化精简清单 */}
              <div className="space-y-3 pt-4 border-t border-border">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <h3 className="text-xs font-bold text-zinc-900 dark:text-zinc-100 uppercase tracking-wider">
                      可用模型池
                    </h3>
                    <Badge variant="secondary" className="font-mono">
                      已选 {draft.models.length} {allModels.length > 0 ? `/ 共 ${allModels.length}` : ''}
                    </Badge>
                    {invalidModels.length > 0 && (
                      <Badge variant="warning" className="gap-1">
                        <AlertTriangle className="w-3 h-3 shrink-0" />
                        <span>{invalidModels.length} 个未在上游提供</span>
                        <button
                          type="button"
                          onClick={removeInvalidModels}
                          className="underline hover:opacity-80 cursor-pointer ml-0.5"
                          title="从已选模型中移除未在上游提供的模型"
                        >
                          移除
                        </button>
                      </Badge>
                    )}
                  </div>

                  <Button
                    type="button"
                    variant="secondary"
                    size="sm"
                    onClick={fetchModels}
                    disabled={fetching}
                  >
                    <RefreshCw className={`w-3.5 h-3.5 ${fetching ? 'animate-spin' : ''}`} />
                    <span>{fetching ? '获取中…' : fetched ? '刷新上游' : '获取上游模型'}</span>
                  </Button>
                </div>

                {fetchError && (
                  <div className="px-3 py-2 rounded-md bg-rose-50 dark:bg-rose-950/40 border border-rose-200 dark:border-rose-900/60 text-xs text-rose-700 dark:text-rose-300 flex items-center gap-2">
                    <AlertCircle className="w-3.5 h-3.5 shrink-0 text-rose-500" />
                    <span className="truncate">{fetchError}</span>
                  </div>
                )}

                {/* 一体化清单卡片 */}
                <div className="rounded-md border border-border bg-background overflow-hidden shadow-2xs">
                  {/* 顶部搜索与快捷操作 */}
                  <div className="flex items-center justify-between gap-2 px-3 py-2 border-b border-border bg-muted/30">
                    <div className="relative flex-1">
                      <Search className="w-3.5 h-3.5 absolute left-2.5 top-1/2 -translate-y-1/2 text-zinc-400 pointer-events-none" />
                      <Input
                        type="text"
                        placeholder="搜索或过滤模型…"
                        value={modelSearch}
                        onChange={(e) => setModelSearch(e.target.value)}
                        className="pl-8 pr-3 h-7 font-mono"
                      />
                    </div>

                    {allModels.length > 0 && (
                      <div className="flex items-center gap-1 shrink-0 text-xs text-zinc-500 dark:text-zinc-400">
                        <button
                          type="button"
                          onClick={selectAllFiltered}
                          className="px-2 py-1 rounded hover:bg-zinc-200/60 dark:hover:bg-zinc-800 hover:text-zinc-900 dark:hover:text-zinc-100 transition-colors cursor-pointer"
                        >
                          全选
                        </button>
                        <span className="text-zinc-300 dark:text-zinc-700">|</span>
                        <button
                          type="button"
                          onClick={clearAllFiltered}
                          className="px-2 py-1 rounded hover:bg-zinc-200/60 dark:hover:bg-zinc-800 hover:text-zinc-900 dark:hover:text-zinc-100 transition-colors cursor-pointer"
                        >
                          清空
                        </button>
                      </div>
                    )}
                  </div>

                  {/* 模型紧凑流式列表：按内容自适应宽度，拒绝强行通栏拉长 */}
                  <div className="max-h-40 overflow-y-auto p-2">
                    {filteredModels.length === 0 ? (
                      <div className="py-6 text-center text-xs text-zinc-400">
                        {allModels.length === 0
                          ? '暂无模型，点击右上角「获取上游模型」或在下方手动补充'
                          : '未匹配到相关模型'}
                      </div>
                    ) : (
                      <div className="flex flex-wrap gap-1.5">
                        {filteredModels.map((model) => {
                          const isSelected = draft.models.includes(model);
                          const isInvalid = invalidModels.includes(model);

                          return (
                            <button
                              key={model}
                              type="button"
                              onClick={() => toggleModel(model)}
                              className={`inline-flex items-center gap-2 px-2.5 py-1 rounded-md text-xs font-mono transition-colors cursor-pointer select-none border ${
                                isSelected
                                  ? 'bg-zinc-100/90 dark:bg-zinc-800/90 text-zinc-900 dark:text-zinc-100 border-zinc-300/90 dark:border-zinc-700 font-medium shadow-2xs'
                                  : 'bg-white/80 dark:bg-zinc-900/60 text-zinc-600 dark:text-zinc-400 border-zinc-200/80 dark:border-zinc-800 hover:border-zinc-300 dark:hover:border-zinc-700 hover:text-zinc-900 dark:hover:text-zinc-200'
                              }`}
                            >
                              {/* 正统 shadcn 高反差极简复选框 */}
                              <span
                                className={`w-3.5 h-3.5 rounded-[3px] border flex items-center justify-center shrink-0 transition-colors ${
                                  isSelected
                                    ? 'bg-zinc-900 border-zinc-900 text-white dark:bg-zinc-100 dark:border-zinc-100 dark:text-zinc-900'
                                    : 'border-zinc-300/80 dark:border-zinc-600 bg-white dark:bg-zinc-800'
                                }`}
                              >
                                {isSelected && <Check className="w-2.5 h-2.5 stroke-[3]" />}
                              </span>
                              <span>{model}</span>
                              {isInvalid && (
                                <span title="该模型未在上游接口返回的列表中" className="shrink-0 text-amber-500">
                                  <AlertTriangle className="w-3 h-3" />
                                </span>
                              )}
                            </button>
                          );
                        })}
                      </div>
                    )}
                  </div>

                  {/* 底部紧凑手动补充条 */}
                  <div className="px-3 py-2 border-t border-border bg-muted/20 flex items-center gap-2">
                    <Input
                      type="text"
                      value={manualModel}
                      onChange={(e) => setManualModel(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter') {
                          e.preventDefault();
                          addManualModel();
                        }
                      }}
                      placeholder="手动补充模型 ID，如 claude-3-7-sonnet（回车添加）"
                      className="font-mono h-7"
                    />
                    <Button
                      type="button"
                      variant="secondary"
                      size="sm"
                      onClick={addManualModel}
                      disabled={!manualModel.trim()}
                    >
                      添加
                    </Button>
                  </div>
                </div>
                {errors.models && <p className="text-[11px] text-rose-500 mt-1">{errors.models}</p>}
              </div>



              {errors.save && (
                <div className="p-3 rounded-md bg-rose-50 dark:bg-rose-950/40 border border-rose-200 dark:border-rose-900/60 text-xs text-rose-700 dark:text-rose-300">
                  保存失败：{errors.save}
                </div>
              )}

              {/* 表单自然底部操作区：纯正 shadcn/ui 按钮层级 */}
              <div className="flex items-center justify-between pt-5 border-t border-border">
                <div>
                  {editingProvider && (
                    <Button
                      type="button"
                      variant="destructive-ghost"
                      size="default"
                      disabled={saving || deleting}
                      onClick={() => setConfirmingDelete(true)}
                    >
                      删除此服务商
                    </Button>
                  )}
                </div>

                <div className="flex items-center gap-2.5">
                  <span className="text-[11px] text-zinc-400 dark:text-zinc-500">
                    {fetching
                      ? '正在获取上游模型…'
                      : draft.models.length === 0
                        ? '请先选择或添加至少一个模型'
                        : ''}
                  </span>
                  <Button
                    type="button"
                    variant="outline"
                    size="default"
                    onClick={handleClose}
                  >
                    取消
                  </Button>
                  <Button
                    type="submit"
                    variant="default"
                    size="default"
                    disabled={saving || deleting || fetching || draft.models.length === 0}
                  >
                    {saving ? (
                      <LoaderCircle className="w-3.5 h-3.5 animate-spin" />
                    ) : (
                      <Check className="w-3.5 h-3.5" />
                    )}
                    <span>{saving ? '保存中…' : editingProvider ? '保存修改' : '添加并启用'}</span>
                  </Button>
                </div>
              </div>
            </form>
          )}
        </div>
      </div>

      {editingProvider && (
        <ConfirmDialog
          open={confirmingDelete}
          title={"删除服务商“" + (draft.name || editingProvider.name) + "”？"}
          details={[
            '删除后需重新配置。已有请求日志不受影响。',
            ...(draft.isDefault ? ['该服务商当前是默认服务商，删除后需要重新指定。'] : []),
          ]}
          confirmLabel="删除"
          busy={deleting}
          onConfirm={async () => {
            setDeleting(true);
            try {
              await deleteProvider(editingProvider.id);
              closeProviderEditor();
            } catch {
              setDeleting(false);
            }
          }}
          onCancel={() => setConfirmingDelete(false)}
        />
      )}
    </div>
  );
}
