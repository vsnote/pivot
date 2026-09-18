import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { ArrowLeft, Check, Eye, EyeOff, LoaderCircle, Plus, RefreshCw, Search, X } from 'lucide-react';
import { useAppStore } from '@/store/useAppStore';
import type { Preset, Protocol, Provider } from '@/types';
import { Select } from '@/components/common/Select';
import { Switch } from '@/components/common/Switch';
import { applyProviderPreset, createProviderDraft, editablePresetUrl, prepareProvider, uniqueValues, validateBaseUrl } from './providerDraft';

const protocols: Record<Protocol, string> = {
  openai: 'OpenAI 兼容', anthropic: 'Anthropic', gemini: 'Google Gemini', openrouter: 'OpenRouter', ollama: 'Ollama',
};
const PAGE_SIZE = 12;

export function ProviderEditor() {
  const { editingProvider, presets, closeProviderEditor, saveProvider, activeTab } = useAppStore();
  const [initial] = useState(() => createProviderDraft(editingProvider, presets));
  const [draft, setDraft] = useState(initial);
  const [choosing, setChoosing] = useState(!editingProvider);
  const [presetSearch, setPresetSearch] = useState('');
  const [showKeys, setShowKeys] = useState(false);
  const [renaming, setRenaming] = useState(!initial.name);
  const [showManual, setShowManual] = useState(false);
  const [showAuthHelp, setShowAuthHelp] = useState(false);
  const [fetching, setFetching] = useState(false);
  const [fetched, setFetched] = useState<string[] | null>(null);
  const [fetchError, setFetchError] = useState('');
  const [notice, setNotice] = useState('');
  const [modelSearch, setModelSearch] = useState('');
  const [manualModel, setManualModel] = useState('');
  const [modelPage, setModelPage] = useState(0);
  const [showAllSelected, setShowAllSelected] = useState(false);
  const [saving, setSaving] = useState(false);
  const [confirmation, setConfirmation] = useState<{ kind: 'leave' | 'preset'; preset?: Preset } | null>(null);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const requestId = useRef(0);
  const formRef = useRef<HTMLFormElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const preset = presets.find((p) => p.id === draft.presetId);
  const urlEditable = !preset || editablePresetUrl(preset);
  const dirty = JSON.stringify(draft) !== JSON.stringify(initial) || !!manualModel.trim();
  const keyInputs = draft.apiKeys.length ? draft.apiKeys : [''];
  const isLocal = preset?.category === '开源 / 本地部署' || /^https?:\/\/(localhost|127\.0\.0\.1|\[::1\])(?=[:/]|$)/.test(draft.baseUrl);

  const leave = () => {
    if (saving) return;
    if (dirty) { setConfirmation({ kind: 'leave' }); return; }
    closeProviderEditor();
  };

  useEffect(() => () => { requestId.current++; }, []);
  useEffect(() => {
    const beforeUnload = (event: BeforeUnloadEvent) => {
      if (dirty || saving) { event.preventDefault(); event.returnValue = ''; }
    };
    window.addEventListener('beforeunload', beforeUnload);
    return () => window.removeEventListener('beforeunload', beforeUnload);
  }, [dirty, saving]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== 'Escape' || activeTab !== 'providers' || event.defaultPrevented || event.target instanceof HTMLSelectElement) return;
      event.preventDefault();
      if (confirmation) setConfirmation(null); else leave();
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  });

  const invalidateModels = () => {
    requestId.current++;
    setFetching(false);
    setFetched(null);
    setFetchError('');
    setModelPage(0);
  };

  const update = (patch: Partial<Provider>) => {
    setDraft((current) => ({ ...current, ...patch }));
    setErrors({});
  };

  const choosePreset = (next?: Preset, confirmed = false) => {
    if (!confirmed && (draft.apiKeys.some((key) => key.trim()) || draft.models.length > 0 || manualModel.trim())) {
      setConfirmation({ kind: 'preset', preset: next });
      return;
    }
    setConfirmation(null);
    invalidateModels();
    setDraft(applyProviderPreset(draft, next));
    setShowKeys(false);
    setRenaming(!next);
    setShowManual(false);
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
    try { sameOrigin = new URL(baseUrl).origin === new URL(draft.baseUrl).origin; } catch { /* incomplete URL */ }
    invalidateModels();
    const clearKeys = !sameOrigin && draft.apiKeys.some((key) => key.trim());
    update({ baseUrl, latencyMs: undefined, ...(clearKeys ? { apiKeys: [] } : {}) });
    setNotice(clearKeys ? '接口主机已更改，原密钥已清除，请填写此服务商的密钥。' : '连接信息已更改，已选模型保留，请重新获取模型确认。');
  };

  const fetchModels = async () => {
    const error = validateBaseUrl(draft.baseUrl);
    if (error) { setErrors({ baseUrl: error }); return; }
    const current = ++requestId.current;
    setFetching(true);
    setFetchError('');
    try {
      const models = await invoke<string[]>('fetch_provider_models', {
        protocol: draft.protocol, baseUrl: draft.baseUrl.trim(), apiKey: uniqueValues(draft.apiKeys)[0] ?? '',
      });
      if (requestId.current !== current) return;
      setFetched(uniqueValues(models));
      setModelPage(0);
      setNotice('');
    } catch (error) {
      if (requestId.current === current) setFetchError(String(error));
    } finally {
      if (requestId.current === current) setFetching(false);
    }
  };

  const toggleModel = (model: string) => update({ models: draft.models.includes(model)
    ? draft.models.filter((m) => m !== model) : [...draft.models, model] });
  const addManualModel = () => {
    const model = manualModel.trim();
    if (!model || /[\s,，]/u.test(model)) { setErrors({ models: '请输入一个完整的模型 ID，不要包含空格或逗号' }); return; }
    update({ models: uniqueValues([...draft.models, model]) });
    setManualModel('');
    setShowManual(false);
  };

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (saving) return;
    const nextErrors: Record<string, string> = {};
    if (!draft.name.trim()) nextErrors.name = '请输入服务商名称';
    const urlError = validateBaseUrl(draft.baseUrl);
    if (urlError) nextErrors.baseUrl = urlError;
    if (!draft.models.length) nextErrors.models = '请至少选择或手动添加一个模型';
    if (manualModel.trim()) nextErrors.models = '请先添加正在输入的模型，或清空输入框';
    const defaultModel = draft.defaultModel?.trim() ?? '';
    if (/[\s,，]/u.test(defaultModel)) nextErrors.defaultModel = '请输入一个完整的模型 ID，不要包含空格或逗号';
    setErrors(nextErrors);
    if (Object.keys(nextErrors).length) {
      requestAnimationFrame(() => formRef.current?.querySelector<HTMLElement>('[aria-invalid="true"]')?.focus());
      return;
    }
    setSaving(true);
    try {
      await saveProvider(prepareProvider(draft));
      closeProviderEditor();
    } catch (error) { setErrors({ save: String(error) }); }
    finally { setSaving(false); }
  };

  const candidates = (fetched ?? []).filter((model) => model.toLowerCase().includes(modelSearch.toLowerCase()));
  const page = Math.min(modelPage, Math.max(0, Math.ceil(candidates.length / PAGE_SIZE) - 1));
  const missing = fetched === null ? [] : draft.models.filter((m) => !fetched.includes(m));
  const matchingPresets = presets.filter((p) => `${p.name} ${p.id} ${p.category}`.toLowerCase().includes(presetSearch.toLowerCase()));

  return (
    <form ref={formRef} onSubmit={submit} className={`provider-editor ${choosing ? 'editor-choosing' : 'editor-configuring'}`} aria-label={editingProvider ? '编辑服务商' : '添加服务商'}>
      <div className="editor-heading">
        <button type="button" className="button button-ghost" onClick={leave} disabled={saving}><ArrowLeft size={15} />返回</button>
        <h1>{editingProvider ? '编辑服务商' : '添加服务商'}</h1>
      </div>
      <div ref={contentRef} className="editor-scroll">
        <fieldset disabled={saving || !!confirmation} className="editor-content">
          {choosing ? (
            <section aria-label="选择服务商" className="space-y-4">
              <div className="flex items-center justify-between gap-3">
                <h2 className="text-sm font-semibold">选择服务商</h2>
                {(draft.name || draft.baseUrl) && <button type="button" className="button button-ghost" onClick={() => setChoosing(false)}>返回配置</button>}
              </div>
              <div className="search-field"><Search size={15} /><input autoFocus className="field" aria-label="搜索服务商" placeholder="搜索服务商" value={presetSearch} onChange={(e) => setPresetSearch(e.target.value)} /></div>
              <div className="preset-options">
                <button type="button" className="surface preset-option" onClick={() => choosePreset()}><Plus size={16} /><span>自定义服务商<small>兼容接口、代理或私有部署</small></span></button>
                {matchingPresets.map((p) => <button type="button" key={p.id} className="surface preset-option" onClick={() => choosePreset(p)}><span>{p.name}<small>{p.category}</small></span></button>)}
              </div>
              {presets.length === 0 && <p className="editor-help">暂未加载到预设，可以先添加自定义服务商。</p>}
              {presets.length > 0 && matchingPresets.length === 0 && <p className="editor-help">没有匹配的预设，可以添加自定义服务商。</p>}
            </section>
          ) : <>
            <section className="editor-section" aria-label="连接信息">
              <div className="editor-section-heading editor-identity">
                <h2>{draft.name || '自定义服务商'}</h2>
                <div className="flex items-center gap-4 shrink-0">
                  {!renaming && <button type="button" className="editor-secondary-action" onClick={() => setRenaming(true)}>重命名</button>}
                  {!editingProvider && <button type="button" className="editor-secondary-action" onClick={() => { setChoosing(true); contentRef.current?.scrollTo(0, 0); }}>更换</button>}
                </div>
              </div>
              {(renaming || !draft.name || errors.name) && <div>
                <label className="field-label" htmlFor="provider-name">名称</label>
                <div className="flex items-center gap-2"><input id="provider-name" autoFocus className="field" value={draft.name} onChange={(e) => update({ name: e.target.value })} onKeyDown={(e) => { if (e.key === 'Enter') { e.preventDefault(); if (draft.name.trim()) setRenaming(false); } }} aria-invalid={!!errors.name} aria-describedby={errors.name ? 'name-error' : undefined} placeholder="为此连接取一个名称" /><button type="button" className="button" disabled={!draft.name.trim()} onClick={() => setRenaming(false)}>完成</button></div>
                {errors.name && <p id="name-error" className="editor-error">{errors.name}</p>}
              </div>}
              {!preset && <div><label className="field-label" htmlFor="provider-protocol">协议</label><Select id="provider-protocol" value={draft.protocol} onChange={(e) => { invalidateModels(); update({ protocol: e.target.value as Protocol, apiKeys: [], models: [] }); setNotice('协议已更改，请重新填写密钥并选择模型。'); }} containerClassName="w-full">{Object.entries(protocols).map(([id, label]) => <option key={id} value={id}>{label}</option>)}</Select></div>}
              {urlEditable ? <div><label className="field-label" htmlFor="provider-url">接口地址</label><input id="provider-url" className="field font-mono" value={draft.baseUrl} onChange={(e) => changeUrl(e.target.value)} aria-invalid={!!errors.baseUrl} aria-describedby={errors.baseUrl ? 'url-error' : 'url-help'} placeholder="https://api.example.com/v1" autoCapitalize="none" spellCheck={false} /><p id="url-help" className="editor-help">填写基础地址，包含服务要求的版本路径（如 /v1），不要填写完整的聊天请求地址。</p>{errors.baseUrl && <p id="url-error" className="editor-error">{errors.baseUrl}</p>}</div>
              : <div className="editor-connection"><span className="badge">{protocols[draft.protocol]}</span><code title={draft.baseUrl}>{draft.baseUrl}</code><button type="button" className="editor-text-button" onClick={() => update({ presetId: null })}>自定义</button></div>}
              {preset && urlEditable && <p className="editor-help">协议：{protocols[draft.protocol]}（由预设配置）</p>}
              <div className="editor-credentials">
                <label className="field-label" htmlFor="provider-key-0">API 密钥</label>
                <div className="space-y-2">{keyInputs.map((key, index) => <div key={index} className="flex items-center gap-2">
                  <div className="editor-key-input"><input id={`provider-key-${index}`} autoFocus={!renaming && index === 0 && !editingProvider} aria-label={`API 密钥 ${index + 1}`} className="field font-mono" type={showKeys ? 'text' : 'password'} value={key} autoComplete="off" autoCapitalize="none" spellCheck={false} placeholder={index ? '另一把 API 密钥' : '输入 API 密钥'} onChange={(e) => { const keys = [...keyInputs]; keys[index] = e.target.value; invalidateModels(); update({ apiKeys: keys }); }} /><button type="button" className="icon-button" onClick={() => setShowKeys(!showKeys)} aria-label={`${showKeys ? '隐藏' : '显示'}密钥 ${index + 1}`}>{showKeys ? <EyeOff size={15} /> : <Eye size={15} />}</button></div>
                  {keyInputs.length > 1 && <button type="button" className="icon-button" aria-label={`移除密钥 ${index + 1}`} onClick={() => { invalidateModels(); update({ apiKeys: keyInputs.filter((_, i) => i !== index) }); }}><X size={14} /></button>}
                </div>)}</div>
                <div className="editor-key-actions"><button type="button" className="editor-secondary-action" onClick={() => update({ apiKeys: [...keyInputs, ''] })}>添加另一把密钥</button>{isLocal ? <span className="editor-help !mt-0">无需认证时可留空</span> : <button type="button" className="editor-secondary-action" aria-expanded={showAuthHelp} onClick={() => setShowAuthHelp(!showAuthHelp)}>密钥说明</button>}</div>
                {showAuthHelp && !isLocal && <p className="editor-help">密钥用于连接此服务商。留空时使用客户端传入的密钥；获取模型通常需要先填写密钥。</p>}
              </div>
              {notice && <p role="status" className="editor-help">{notice}</p>}
            </section>
            <section className="editor-section" aria-label="模型选择">
              <div className="editor-section-heading"><h2>模型{draft.models.length > 0 && <span className="editor-model-count">已选 {draft.models.length}</span>}</h2>
                {(fetched !== null || draft.models.length > 0) && <button type="button" className="button" onClick={fetchModels} disabled={fetching}>{fetching ? <LoaderCircle size={14} className="animate-spin" /> : <RefreshCw size={14} />}{fetching ? '获取中…' : fetchError ? '重试' : fetched === null ? '获取模型' : '刷新'}</button>}
              </div>
              {fetched === null && draft.models.length === 0 && <div className="editor-model-empty"><p>{fetchError ? '暂时无法获取模型' : '获取此服务商的模型'}</p><span className="editor-help">{isLocal ? '连接本地服务，选择需要使用的模型。' : '填写密钥后，选择需要使用的模型。'}</span><button type="button" className="button button-primary" onClick={fetchModels} disabled={fetching}>{fetching ? <LoaderCircle size={14} className="animate-spin" /> : <RefreshCw size={14} />}{fetching ? '获取中…' : fetchError ? '重试' : '获取模型'}</button></div>}
              {(fetchError || fetched !== null || fetching) && <div role="status" aria-live="polite">{fetchError && <p className="editor-error">{fetchError}</p>}{fetched !== null && !fetching && <p className="editor-help">{fetched.length ? `${fetchError ? '保留上次获取的' : '获取到'} ${fetched.length} 个模型` : '服务商未返回模型，可刷新或手动添加。'}</p>}{fetching && <span className="sr-only">正在获取模型</span>}</div>}
              {uniqueValues(draft.apiKeys).length > 1 && <p className="editor-help">使用第一把密钥获取，其他密钥的模型权限可能不同。</p>}
              {draft.models.length > 0 && <div><div className="selected-models">{(showAllSelected ? draft.models : draft.models.slice(0, 8)).map((model) => <span key={model} className="selected-model"><span>{model}</span><button type="button" aria-label={`移除模型 ${model}`} onClick={() => toggleModel(model)}><X size={13} /></button></span>)}</div>{draft.models.length > 8 && <button type="button" className="editor-text-button mt-2" aria-expanded={showAllSelected} onClick={() => setShowAllSelected(!showAllSelected)}>{showAllSelected ? '收起' : `查看全部 ${draft.models.length} 个已选模型`}</button>}</div>}
              {missing.length > 0 && <p className="editor-help">有 {missing.length} 个已选模型未出现在本次结果中，已保留，请确认是否继续使用。</p>}
              {!!fetched?.length && <>
                <div className="flex items-center gap-2"><input className="field" aria-label="搜索模型" placeholder="搜索模型 ID" value={modelSearch} onChange={(e) => { setModelSearch(e.target.value); setModelPage(0); }} /><button type="button" className="button" disabled={!candidates.length} onClick={() => update({ models: uniqueValues([...draft.models, ...candidates]) })}>全选结果</button></div>
                <div className="model-options">{candidates.slice(page * PAGE_SIZE, (page + 1) * PAGE_SIZE).map((model) => <label key={model}><input type="checkbox" checked={draft.models.includes(model)} onChange={() => toggleModel(model)} /><span>{model}</span></label>)}</div>
                {candidates.length === 0 && <p className="editor-help">没有匹配的模型。</p>}
                {candidates.length > PAGE_SIZE && <div className="flex justify-between items-center"><button type="button" className="button" disabled={page === 0} onClick={() => setModelPage(page - 1)}>上一页</button><span className="editor-help">{page + 1} / {Math.ceil(candidates.length / PAGE_SIZE)}</span><button type="button" className="button" disabled={(page + 1) * PAGE_SIZE >= candidates.length} onClick={() => setModelPage(page + 1)}>下一页</button></div>}
              </>}

              {showManual || errors.models ? <div>
                <label htmlFor="manual-model" className="field-label">模型 ID</label>
                <div className="flex gap-2"><input id="manual-model" autoFocus className="field font-mono" placeholder="输入一个完整的模型 ID" value={manualModel} onChange={(e) => setManualModel(e.target.value)} onKeyDown={(e) => { if (e.key === 'Enter') { e.preventDefault(); addManualModel(); } }} aria-invalid={!!errors.models} aria-describedby={errors.models ? 'models-error' : undefined} /><button type="button" className="button" onClick={addManualModel} disabled={!manualModel.trim()}>添加</button><button type="button" className="icon-button" aria-label="取消手动添加" onClick={() => { setManualModel(''); setShowManual(false); setErrors({}); }}><X size={14} /></button></div>
                {errors.models && <p id="models-error" className="editor-error" role="alert">{errors.models}</p>}
              </div> : <button type="button" className="editor-secondary-action self-start" onClick={() => setShowManual(true)}>手动添加模型</button>}

            </section>
            <section className="editor-section" aria-label="默认路由">
              <div className="editor-section-heading"><h2>默认路由</h2></div>
              <div>
                <label className="field-label" htmlFor="provider-default-model">默认模型</label>
                <input id="provider-default-model" className="field font-mono" placeholder="例如 gemini-3.8-flash-high" value={draft.defaultModel ?? ''} onChange={(e) => update({ defaultModel: e.target.value })} aria-invalid={!!errors.defaultModel} aria-describedby={errors.defaultModel ? 'default-model-error' : 'default-model-help'} autoComplete="off" autoCapitalize="none" spellCheck={false} />
                <p id="default-model-help" className="editor-help">请求的模型名不属于这家服务商时改用它，备用切换不会再沿用原模型名。留空时按原模型名转发。</p>
                {errors.defaultModel && <p id="default-model-error" className="editor-error">{errors.defaultModel}</p>}
              </div>
              <div className="flex items-start justify-between gap-4">
                <span>
                  <span className="field-label !mb-0">设为默认服务商</span>
                  <p className="editor-help !mt-0">没有服务商声明的模型名（例如 Claude Code 的模型名）交给这家处理；同时只能有一家。</p>
                </span>
                <Switch checked={!!draft.isDefault} onCheckedChange={(checked) => update({ isDefault: checked })} ariaLabel="设为默认服务商" />
              </div>
              {draft.isDefault && !(draft.defaultModel ?? '').trim() && (
                <p className="editor-help">建议同时填写默认模型：否则没有服务商声明的模型名会被原样转发，容易被上游拒绝。</p>
              )}
            </section>
          </>}
        </fieldset>
      </div>
      <div className="editor-footer">
        {confirmation ? <>
          <p className="editor-help !mt-0 mr-auto" role="alert">{confirmation.kind === 'leave' ? '放弃尚未保存的修改？' : '更换服务商将清除密钥、已选模型和默认路由设置。'}</p>
          <button autoFocus type="button" className="button" onClick={() => setConfirmation(null)}>继续编辑</button>
          <button type="button" className="button button-primary" onClick={() => confirmation.kind === 'leave' ? closeProviderEditor() : choosePreset(confirmation.preset, true)}>{confirmation.kind === 'leave' ? '放弃修改' : '确认更换'}</button>
        </> : <>
        {errors.save && <p className="editor-error mr-auto" role="alert">保存失败：{errors.save}</p>}
        <button type="button" className="button" onClick={leave} disabled={saving}>取消</button>
        {!choosing && <button type="submit" className="button button-primary" disabled={saving || fetching || draft.models.length === 0}>{saving ? <LoaderCircle size={14} className="animate-spin" /> : <Check size={14} />}{saving ? '保存中…' : editingProvider ? '保存修改' : '添加并启用'}</button>}
        </>}
      </div>
    </form>
  );
}
