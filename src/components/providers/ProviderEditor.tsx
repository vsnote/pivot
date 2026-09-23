import React, { useEffect, useRef, useState } from 'react';
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
  HelpCircle,
} from 'lucide-react';
import {
  ProviderAvatar,
} from '@/components/common/BrandIcons';
import { useAppStore } from '@/store/useAppStore';
import type { Preset, Protocol, Provider } from '@/types';
import { Select } from '@/components/common/Select';
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
  const { editingProvider, presets, closeProviderEditor, saveProvider, activeTab } = useAppStore();
  const [initial] = useState(() => createProviderDraft(editingProvider, presets));
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
    setDraft(applyProviderPreset(draft, next));
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
    const clearKeys = !sameOrigin && draft.apiKeys.some((key) => key.trim());
    update({ baseUrl, latencyMs: undefined, ...(clearKeys ? { apiKeys: [] } : {}) });
    setNotice(
      clearKeys
        ? '接口主机已更改，原密钥已清除，请填写此服务商的新密钥。'
        : '连接信息已更改，已选模型保留，建议重新获取模型进行验证。',
    );
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

  const toggleModel = (model: string) => {
    update({
      models: draft.models.includes(model)
        ? draft.models.filter((m) => m !== model)
        : [...draft.models, model],
    });
  };

  const addManualModel = () => {
    const model = manualModel.trim();
    if (!model || /[\s,，]/u.test(model)) {
      setErrors({ models: '请输入一个完整的模型 ID，不要包含空格或逗号' });
      return;
    }
    update({ models: uniqueValues([...draft.models, model]) });
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

  const matchingPresets = presets.filter((p) =>
    `${p.name} ${p.id} ${p.category}`.toLowerCase().includes(presetSearch.toLowerCase()),
  );

  const candidates = (fetched ?? []).filter((model) =>
    model.toLowerCase().includes(modelSearch.toLowerCase()),
  );

  return (
    <div className="w-full pb-10 space-y-4 animate-in fade-in duration-150">
      {/* 顶部纯粹的「返回」栏与快捷操作 */}
      <div className="flex items-center justify-between pb-1">
        <div className="flex items-center gap-2">
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
                className="px-2.5 py-1 rounded-md text-xs font-semibold text-white bg-rose-600 hover:bg-rose-700 transition-colors cursor-pointer"
              >
                确认放弃
              </button>
              <button
                type="button"
                onClick={() => setConfirmation(null)}
                className="px-2.5 py-1 rounded-md text-xs font-medium text-zinc-600 dark:text-zinc-400 hover:bg-white dark:hover:bg-zinc-800 transition-colors cursor-pointer"
              >
                继续编辑
              </button>
            </div>
          ) : (
            <button
              type="button"
              onClick={handleBack}
              className="inline-flex items-center gap-1.5 px-2.5 py-1 -ml-2.5 rounded-lg text-xs font-medium text-zinc-500 hover:text-zinc-900 dark:text-zinc-400 dark:hover:text-zinc-100 hover:bg-zinc-100 dark:hover:bg-zinc-800 transition-colors cursor-pointer"
            >
              <ArrowLeft className="w-3.5 h-3.5" />
              <span>返回</span>
            </button>
          )}
        </div>

        <div className="flex items-center gap-2">
          {!choosing && (
            <button
              type="submit"
              form="provider-drawer-form"
              disabled={saving || fetching || draft.models.length === 0}
              className="flex items-center gap-1.5 px-3.5 py-1.5 rounded-md text-xs font-semibold text-white bg-sky-600 hover:bg-sky-700 dark:bg-cyan-600 dark:hover:bg-cyan-500 disabled:opacity-50 shadow-2xs transition-all active:scale-[0.98] cursor-pointer"
            >
              {saving ? (
                <LoaderCircle className="w-3.5 h-3.5 animate-spin" />
              ) : (
                <Check className="w-3.5 h-3.5" />
              )}
              <span>{saving ? '保存中…' : editingProvider ? '保存修改' : '添加并启用'}</span>
            </button>
          )}
          <button
            type="button"
            onClick={handleClose}
            className="p-1 rounded-lg text-zinc-400 hover:text-zinc-700 dark:hover:text-zinc-200 hover:bg-zinc-100 dark:hover:bg-zinc-800 transition-colors cursor-pointer"
            title="关闭"
          >
            <X className="w-4 h-4" />
          </button>
        </div>
      </div>

      {/* 页面主卡片 */}
      <div
        ref={drawerRef}
        role="region"
        aria-label={editingProvider ? '编辑服务商' : '添加服务商'}
        className="p-6 rounded-lg border border-zinc-200/80 dark:border-zinc-800 bg-white dark:bg-zinc-900 shadow-xs space-y-6"
      >
        {/* 表单内容 */}
        <div ref={contentRef} className="space-y-6">
          {choosing ? (
            /* 选择服务商预设 */
            <div className="space-y-4">
              <div className="relative">
                <Search className="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-zinc-400 pointer-events-none" />
                <input
                  autoFocus
                  type="text"
                  placeholder="搜索服务商预设，如 Anthropic, OpenAI, DeepSeek..."
                  value={presetSearch}
                  onChange={(e) => setPresetSearch(e.target.value)}
                  className="w-full pl-9 pr-3.5 py-2 text-xs rounded-md border border-zinc-200 dark:border-zinc-800 bg-zinc-50 dark:bg-zinc-950 text-zinc-900 dark:text-zinc-100 placeholder-zinc-400 focus:outline-none focus:ring-2 focus:ring-cyan-500/20 focus:border-cyan-500"
                />
              </div>

              <div className="grid grid-cols-2 sm:grid-cols-3 gap-2.5">
                {/* 自定义连接 */}
                <button
                  type="button"
                  onClick={() => choosePreset()}
                  className="flex items-center gap-2.5 p-3 rounded-md border border-dashed border-zinc-300 dark:border-zinc-700 hover:border-cyan-500 dark:hover:border-cyan-500 bg-zinc-50/50 dark:bg-zinc-850/30 text-left transition-all group cursor-pointer"
                >
                  <div className="w-7 h-7 rounded-md bg-cyan-50 dark:bg-cyan-950/50 border border-cyan-200/60 dark:border-cyan-800/60 text-cyan-600 dark:text-cyan-400 flex items-center justify-center shrink-0">
                    <Plus className="w-4 h-4" />
                  </div>
                  <span className="text-xs font-semibold text-zinc-900 dark:text-zinc-100 group-hover:text-cyan-600 dark:group-hover:text-cyan-400 truncate">
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
                      className="flex items-center gap-2.5 p-3 rounded-md border border-zinc-200/80 dark:border-zinc-800 hover:border-cyan-500/80 dark:hover:border-cyan-500/80 bg-white dark:bg-zinc-900/90 hover:bg-cyan-50/20 dark:hover:bg-cyan-950/20 text-left shadow-2xs hover:shadow-xs transition-all duration-150 group cursor-pointer"
                    >
                      <ProviderAvatar
                        name={cleanName}
                        protocol={p.protocol}
                        iconKey={p.icon || p.id}
                        sizeClass="w-7 h-7"
                      />
                      <span className="text-xs font-semibold text-zinc-900 dark:text-zinc-100 group-hover:text-cyan-600 dark:group-hover:text-cyan-400 truncate">
                        {cleanName}
                      </span>
                    </button>
                  );
                })}
              </div>
            </div>
          ) : (
            /* 配置表单 */
            <form id="provider-drawer-form" onSubmit={submit} className="space-y-6">
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
                  <input
                    type="text"
                    value={draft.name}
                    onChange={(e) => update({ name: e.target.value })}
                    placeholder="例如 Anthropic Claude、DeepSeek 官方"
                    className="w-full px-3 py-2 text-xs rounded-md border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-950 text-zinc-900 dark:text-zinc-100 focus:outline-none focus:ring-2 focus:ring-cyan-500/20 focus:border-cyan-500"
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
                      <div className="px-3 py-2 text-xs font-mono rounded-md border border-zinc-200 dark:border-zinc-800 bg-zinc-50 dark:bg-zinc-850/50 text-zinc-700 dark:text-zinc-300">
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
                    <label className="block text-xs font-medium text-zinc-700 dark:text-zinc-300 mb-1">
                      接口基地址 (Base URL)
                    </label>
                    <input
                      type="text"
                      value={draft.baseUrl}
                      onChange={(e) => changeUrl(e.target.value)}
                      disabled={!urlEditable}
                      placeholder="https://api.example.com/v1"
                      className="w-full px-3 py-2 text-xs font-mono rounded-md border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-950 text-zinc-900 dark:text-zinc-100 disabled:opacity-60 focus:outline-none focus:ring-2 focus:ring-cyan-500/20 focus:border-cyan-500"
                    />
                    {errors.baseUrl && (
                      <p className="text-[11px] text-rose-500 mt-1">{errors.baseUrl}</p>
                    )}
                  </div>
                </div>

                {/* API 密钥 */}
                <div>
                  <div className="flex items-center justify-between mb-1">
                    <label className="block text-xs font-medium text-zinc-700 dark:text-zinc-300">
                      API 密钥 (Key)
                    </label>
                    <div className="flex items-center gap-3">
                      <button
                        type="button"
                        onClick={() => setShowAuthHelp(!showAuthHelp)}
                        className="text-[11px] text-zinc-400 hover:text-cyan-600 dark:hover:text-cyan-400 flex items-center gap-0.5 cursor-pointer"
                      >
                        <HelpCircle className="w-3 h-3" />
                        <span>密钥说明</span>
                      </button>
                      <button
                        type="button"
                        onClick={() => update({ apiKeys: [...keyInputs, ''] })}
                        className="text-[11px] text-cyan-600 dark:text-cyan-400 hover:underline cursor-pointer"
                      >
                        + 多密钥轮询
                      </button>
                    </div>
                  </div>

                  {showAuthHelp && (
                    <div className="mb-2 p-2.5 rounded-lg bg-zinc-50 dark:bg-zinc-850 text-[11px] text-zinc-600 dark:text-zinc-300 leading-relaxed border border-zinc-200/60 dark:border-zinc-700/60">
                      密钥保存在本地 SQLite 中。留空时默认沿用客户端传入的密钥；本地模型（如 Ollama）通常无需填写。
                    </div>
                  )}

                  <div className="space-y-2">
                    {keyInputs.map((key, index) => (
                      <div key={index} className="flex items-center gap-2">
                        <div className="relative flex-1">
                          <input
                            type={showKeys ? 'text' : 'password'}
                            value={key}
                            onChange={(e) => {
                              const keys = [...keyInputs];
                              keys[index] = e.target.value;
                              invalidateModels();
                              update({ apiKeys: keys });
                            }}
                            placeholder={index ? '添加备用密钥' : 'sk-ant-... / sk-...（可留空）'}
                            className="w-full pl-3 pr-9 py-2 text-xs font-mono rounded-md border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-950 text-zinc-900 dark:text-zinc-100 focus:outline-none focus:ring-2 focus:ring-cyan-500/20 focus:border-cyan-500"
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
              </div>

              {/* 模型选择与声明 */}
              <div className="space-y-3.5 pt-4 border-t border-zinc-100 dark:border-zinc-800">
                <div className="flex items-center justify-between">
                  <h3 className="text-xs font-bold text-zinc-900 dark:text-zinc-100 uppercase tracking-wider flex items-center gap-1.5">
                    可用模型池
                    <span className="text-[11px] font-normal font-mono px-1.5 py-0.2 rounded bg-zinc-100 dark:bg-zinc-800 text-zinc-600 dark:text-zinc-400">
                      已选 {draft.models.length}
                    </span>
                  </h3>

                  <button
                    type="button"
                    onClick={fetchModels}
                    disabled={fetching}
                    className="flex items-center gap-1 px-2.5 py-1.5 rounded-lg text-xs font-medium text-cyan-600 dark:text-cyan-400 bg-cyan-50 dark:bg-cyan-950/50 hover:bg-cyan-100 dark:hover:bg-cyan-900/40 transition-colors disabled:opacity-50 cursor-pointer"
                  >
                    {fetching ? (
                      <LoaderCircle className="w-3.5 h-3.5 animate-spin" />
                    ) : (
                      <RefreshCw className="w-3.5 h-3.5" />
                    )}
                    <span>{fetching ? '获取中…' : fetched ? '刷新上游模型' : '从上游获取模型'}</span>
                  </button>
                </div>

                {fetchError && (
                  <div className="px-3 py-2 rounded-md bg-rose-50 dark:bg-rose-950/40 border border-rose-200 dark:border-rose-900/60 text-xs text-rose-700 dark:text-rose-300 flex items-center gap-2">
                    <AlertCircle className="w-3.5 h-3.5 shrink-0 text-rose-500" />
                    <span className="truncate">{fetchError}</span>
                  </div>
                )}

                {/* 已选模型 Chips */}
                {draft.models.length > 0 && (
                  <div className="p-3 rounded-md bg-zinc-50 dark:bg-zinc-850/50 border border-zinc-200/80 dark:border-zinc-800/80">
                    <div className="flex flex-wrap gap-1.5 max-h-36 overflow-y-auto">
                      {draft.models.map((model) => (
                        <span
                          key={model}
                          className="inline-flex items-center gap-1 px-2 py-0.5 rounded-md text-[11px] font-mono bg-white dark:bg-zinc-800 text-zinc-800 dark:text-zinc-200 border border-zinc-200 dark:border-zinc-700 shadow-2xs"
                        >
                          <span>{model}</span>
                          <button
                            type="button"
                            onClick={() => toggleModel(model)}
                            className="hover:text-rose-500 cursor-pointer"
                            aria-label={`移除模型 ${model}`}
                          >
                            <X className="w-3 h-3" />
                          </button>
                        </span>
                      ))}
                    </div>
                  </div>
                )}

                {/* 上游候选模型快速选择 */}
                {fetched && fetched.length > 0 && (
                  <div className="space-y-2">
                    <div className="flex items-center gap-2">
                      <div className="relative flex-1">
                        <Search className="w-3.5 h-3.5 absolute left-2.5 top-1/2 -translate-y-1/2 text-zinc-400 pointer-events-none" />
                        <input
                          type="text"
                          placeholder="在获取到的模型中快速筛选…"
                          value={modelSearch}
                          onChange={(e) => setModelSearch(e.target.value)}
                          className="w-full pl-8 pr-3 py-1.5 text-xs font-mono rounded-lg border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-950 text-zinc-900 dark:text-zinc-100 placeholder-zinc-400 focus:outline-none focus:ring-1 focus:ring-cyan-500"
                        />
                      </div>
                      {candidates.length > 0 && (
                        <button
                          type="button"
                          onClick={() => update({ models: uniqueValues([...draft.models, ...candidates]) })}
                          className="px-2.5 py-1.5 rounded-lg text-xs font-medium text-zinc-700 dark:text-zinc-300 bg-zinc-100 dark:bg-zinc-800 hover:bg-zinc-200 dark:hover:bg-zinc-700 transition-colors cursor-pointer"
                        >
                          全选结果 ({candidates.length})
                        </button>
                      )}
                    </div>

                    <div className="grid grid-cols-1 sm:grid-cols-2 gap-1.5 max-h-48 overflow-y-auto p-1.5 rounded-md border border-zinc-200/80 dark:border-zinc-800 bg-zinc-50/50 dark:bg-zinc-950/50">
                      {candidates.map((model) => {
                        const isSelected = draft.models.includes(model);
                        return (
                          <button
                            key={model}
                            type="button"
                            onClick={() => toggleModel(model)}
                            className={`flex items-center justify-between p-2 rounded-lg text-left text-xs font-mono transition-colors cursor-pointer ${
                              isSelected
                                ? 'bg-cyan-50 dark:bg-cyan-950/60 text-cyan-700 dark:text-cyan-300 border border-cyan-200/60 dark:border-cyan-800/60'
                                : 'hover:bg-white dark:hover:bg-zinc-800 text-zinc-700 dark:text-zinc-300'
                            }`}
                          >
                            <span className="truncate pr-1">{model}</span>
                            {isSelected && <Check className="w-3.5 h-3.5 shrink-0" />}
                          </button>
                        );
                      })}
                      {candidates.length === 0 && (
                        <div className="col-span-2 text-center py-4 text-xs text-zinc-400">
                          未匹配到相关模型
                        </div>
                      )}
                    </div>
                  </div>
                )}

                {/* 手动添加单个模型 */}
                <div className="flex gap-2 pt-1">
                  <input
                    type="text"
                    value={manualModel}
                    onChange={(e) => setManualModel(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter') {
                        e.preventDefault();
                        addManualModel();
                      }
                    }}
                    placeholder="手动输入模型 ID，如 claude-3-7-sonnet-20250219"
                    className="flex-1 px-3 py-2 text-xs font-mono rounded-md border border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-950 text-zinc-900 dark:text-zinc-100 focus:outline-none focus:ring-2 focus:ring-cyan-500/20 focus:border-cyan-500"
                  />
                  <button
                    type="button"
                    onClick={addManualModel}
                    disabled={!manualModel.trim()}
                    className="px-3 py-2 rounded-md text-xs font-medium text-zinc-700 dark:text-zinc-200 bg-zinc-100 dark:bg-zinc-800 hover:bg-zinc-200 dark:hover:bg-zinc-700 disabled:opacity-50 transition-colors cursor-pointer shrink-0"
                  >
                    添加模型
                  </button>
                </div>
                {errors.models && <p className="text-[11px] text-rose-500 mt-1">{errors.models}</p>}
                {errors.save && (
                  <div className="p-3 rounded-md bg-rose-50 dark:bg-rose-950/40 border border-rose-200 dark:border-rose-900/60 text-xs text-rose-700 dark:text-rose-300">
                    保存失败：{errors.save}
                  </div>
                )}
              </div>
            </form>
          )}
        </div>
      </div>
    </div>
  );
}
