import React, { useEffect, useRef, useState, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  Check,
  Copy,
  Eye,
  EyeOff,
  LoaderCircle,
  Plus,
  RefreshCw,
  Search,
  Trash2,
  X,
  AlertCircle,
  AlertTriangle,
  HelpCircle,
  Zap,
  Sparkles,
  ChevronDown,
  ChevronUp,
} from 'lucide-react';
import { ConfirmDialog } from '@/components/common/ConfirmDialog';
import { Button } from '@/components/common/Button';
import { Input } from '@/components/common/Input';
import { Badge } from '@/components/common/Badge';
import {
  ICON_SLOT,
  ProviderAvatar,
} from '@/components/common/BrandIcons';
import { useAppStore } from '@/store/useAppStore';
import { useTranslation } from '@/i18n';
import type { Preset, Protocol, Provider, SubscriptionAccount, AccountQuota } from '@/types';
import clsx from 'clsx';
import {
  Select,
  SelectTrigger,
  SelectValue,
  SelectContent,
  SelectItem,
} from '@/components/common/Select';
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

interface ModelCapability {
  displayName?: string;
  modalities: string[];
  contextWindow: number | null;
  maxOutput: number | null;
  reasoning: boolean;
}

const PROTOCOLS: Protocol[] = [
  'openai',
  'anthropic',
  'gemini',
  'openrouter',
  'ollama',
  'chatgpt',
];

export function ProviderEditor() {
  const { t, locale } = useTranslation();
  const { editingProvider, presets, providers, closeProviderEditor, saveProvider, deleteProvider, activeTab } =
    useAppStore();
  const [currentEditingProvider, setCurrentEditingProvider] = useState<Provider | null>(editingProvider);
  const [initial, setInitial] = useState(() => createProviderDraft(editingProvider, presets));
  const [draft, setDraft] = useState(initial);

  const findExistingProviderForPreset = (preset?: Preset) => {
    if (!preset) return undefined;
    const isSub = preset.authType === 'subscription_pool' || preset.category === '官方订阅';
    if (!isSub) return undefined;
    return providers.find(
      (prov) =>
        prov.presetId === preset.id ||
        (prov.authType === 'subscription_pool' &&
          (prov.presetId === preset.id || prov.id === preset.id || prov.name.toLowerCase() === preset.name.toLowerCase())),
    );
  };
  const currentPreset = presets.find((p) => p.id === initial.presetId);
  // 模型池：上游返回 ∪ 预设推荐 ∪ 本地已知候选池。
  // 保持模型在池中的固有位置，不因模型被选中而置顶跳位
  const [pool, setPool] = useState<string[]>(() => {
    let cached: string[] = [];
    if (initial.id) {
      try {
        const raw = localStorage.getItem(`pivot_provider_pool_${initial.id}`);
        if (raw) cached = JSON.parse(raw);
      } catch {}
    }
    const baseline = (currentPreset?.defaultModels && currentPreset.defaultModels.length > 0)
      ? currentPreset.defaultModels
      : cached;
    return uniqueValues([
      ...baseline,
      ...cached,
      ...(initial.models || []),
    ]);
  });

  useEffect(() => {
    if (draft.id) {
      try {
        localStorage.setItem(`pivot_provider_pool_${draft.id}`, JSON.stringify(pool));
      } catch {}
    }
  }, [draft.id, pool]);
  const [choosing, setChoosing] = useState(!editingProvider);
  const [presetSearch, setPresetSearch] = useState('');
  const [showKeys, setShowKeys] = useState(false);
  const [showAuthHelp, setShowAuthHelp] = useState(false);
  const [fetching, setFetching] = useState(false);
  const [fetched, setFetched] = useState<string[] | null>(null);
  const [fetchError, setFetchError] = useState('');
  const [notice, setNotice] = useState('');
  const [modelSearch, setModelSearch] = useState('');
  const [copiedModel, setCopiedModel] = useState<string | null>(null);
  const [modelFilterTab, setModelFilterTab] = useState<'all' | 'selected' | 'unselected'>('all');
  const [manualModel, setManualModel] = useState('');
  const [saving, setSaving] = useState(false);
  const [confirmation, setConfirmation] = useState<{ kind: 'leave' | 'preset'; preset?: Preset } | null>(null);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [testingConnection, setTestingConnection] = useState(false);
  const [latencyResult, setLatencyResult] = useState<{ success: boolean; ms?: number; error?: string } | null>(null);
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const [deleting, setDeleting] = useState(false);

  const isSubscriptionPool = draft.authType === 'subscription_pool' || draft.presetId === 'antigravity' || draft.presetId === 'chatgpt_subscription' || draft.protocol === 'chatgpt';
  const isChatGPT = draft.presetId === 'chatgpt_subscription' || draft.protocol === 'chatgpt';
  const [subAccounts, setSubAccounts] = useState<SubscriptionAccount[]>([]);
  const [loadingAccounts, setLoadingAccounts] = useState(false);
  const [authorizing, setAuthorizing] = useState(false);
  const [importingCodex, setImportingCodex] = useState(false);
  const [oauthError, setOauthError] = useState<string | null>(null);

  const [accountQuotas, setAccountQuotas] = useState<Record<string, AccountQuota>>({});
  const [refreshingQuotaId, setRefreshingQuotaId] = useState<string | null>(null);
  const [expandedQuotaAccounts, setExpandedQuotaAccounts] = useState<Record<string, boolean>>({});

  const toggleExpandQuota = (accountId: string) => {
    setExpandedQuotaAccounts((prev) => {
      const nextState = !prev[accountId];
      // 展开时若尚未查询额度，按需触发单账号实时查询
      if (nextState && !accountQuotas[accountId]) {
        fetchAccountQuota(accountId);
      }
      return { ...prev, [accountId]: nextState };
    });
  };

  const formatResetTime = (resetTimeStr?: string | null) => {
    if (!resetTimeStr) return null;
    const target = new Date(resetTimeStr).getTime();
    if (isNaN(target)) return null;
    const diffMs = target - Date.now();
    if (diffMs <= 0) return locale === 'zh-CN' ? '即将重置' : 'Resetting soon';
    const totalMinutes = Math.floor(diffMs / 60000);
    if (totalMinutes < 60) return locale === 'zh-CN' ? `${totalMinutes}分钟后重置` : `Resets in ${totalMinutes}m`;
    const totalHours = Math.floor(totalMinutes / 60);
    if (totalHours < 24) {
      const remMins = totalMinutes % 60;
      if (totalHours < 6 && remMins > 0) {
        return locale === 'zh-CN' ? `${totalHours}小时${remMins}分钟后重置` : `Resets in ${totalHours}h ${remMins}m`;
      }
      return locale === 'zh-CN' ? `${totalHours}小时后重置` : `Resets in ${totalHours}h`;
    }
    const days = Math.floor(totalHours / 24);
    const remHours = totalHours % 24;
    return locale === 'zh-CN' ? `${days}天${remHours}小时后重置` : `Resets in ${days}d ${remHours}h`;
  };

  const formatCreditExpiry = (isoStr: string) => {
    if (!isoStr) return locale === 'zh-CN' ? '未知' : 'Unknown';
    const d = new Date(isoStr);
    if (isNaN(d.getTime())) return isoStr;
    const year = d.getFullYear();
    const month = (d.getMonth() + 1).toString().padStart(2, '0');
    const day = d.getDate().toString().padStart(2, '0');
    const diffDays = Math.ceil((d.getTime() - Date.now()) / (1000 * 86400));
    const diffDesc = diffDays > 0
      ? (locale === 'zh-CN' ? ` (${diffDays}天后到期)` : ` (expires in ${diffDays}d)`)
      : (locale === 'zh-CN' ? ' (即将到期)' : ' (expires soon)');
    return `${year}-${month}-${day}${diffDesc}`;
  };

  const [consumingCreditAccountId, setConsumingCreditAccountId] = useState<string | null>(null);

  const handleConsumeCredit = async (accountId: string) => {
    if (!draft.id) return;
    try {
      setConsumingCreditAccountId(accountId);
      const updated = await invoke<AccountQuota>('consume_subscription_reset_credit', {
        providerId: draft.id,
        accountId,
      });
      setAccountQuotas((prev) => ({ ...prev, [accountId]: updated }));
      setNotice(locale === 'zh-CN' ? '已使用 1 张重置卡，配额已恢复！' : 'Used 1 reset card, quota restored!');
    } catch (e: any) {
      setOauthError(typeof e === 'string' ? e : e?.message || (locale === 'zh-CN' ? '使用重置卡失败' : 'Failed to use reset card'));
    } finally {
      setConsumingCreditAccountId(null);
    }
  };

  const fetchAccountQuota = async (accountId: string) => {
    if (!draft.id) return;
    try {
      setRefreshingQuotaId(accountId);
      const quota = await invoke<AccountQuota>('get_subscription_account_quota', {
        providerId: draft.id,
        accountId,
        baseUrl: draft.baseUrl || undefined,
      });
      setAccountQuotas((prev) => ({ ...prev, [accountId]: quota }));
    } catch (e) {
      console.warn(`获取账号 ${accountId} 额度失败:`, e);
    } finally {
      setRefreshingQuotaId(null);
    }
  };

  const loadSubscriptionAccounts = async () => {
    if (!draft.id) return;
    try {
      setLoadingAccounts(true);
      const list = await invoke<SubscriptionAccount[]>('get_subscription_accounts', {
        providerId: draft.id,
      });
      setSubAccounts(list);
      // 若已绑定账号且候选池较少，后台静默补充拉取一次上游模型扩充 pool（保持已选模型不变）
      if (list.length > 0 && pool.length <= draft.models.length) {
        syncSubscriptionModels(draft.baseUrl);
      }
    } catch (e) {
      console.error('加载订阅账号失败', e);
    } finally {
      setLoadingAccounts(false);
    }
  };

  useEffect(() => {
    if (isSubscriptionPool) {
      loadSubscriptionAccounts();
    }
  }, [draft.id, isSubscriptionPool]);

  const [syncingModels, setSyncingModels] = useState(false);

  const syncSubscriptionModels = async (baseUrl?: string): Promise<string[]> => {
    const defaultUrl = isChatGPT ? 'https://chatgpt.com/backend-api' : 'https://daily-cloudcode-pa.googleapis.com';
    const url = (baseUrl || draft.baseUrl || defaultUrl).trim();
    try {
      setSyncingModels(true);
      setNotice('');
      const models = await invoke<string[]>('sync_subscription_models', {
        providerId: draft.id,
        baseUrl: url,
      });
      if (models && models.length > 0) {
        const unique = uniqueValues(models);
        setPool((prev) => uniqueValues([...prev, ...unique]));
        setFetched(unique);

        // 获取模型之后，默认不选中：保留已选模型不变，新增模型放入候选池供用户挑选
        setNotice(
          locale === 'zh-CN'
            ? `已获取上游 ${unique.length} 个可用模型，请按需勾选`
            : `Fetched ${unique.length} models from upstream, select as needed`,
        );
        return unique;
      }
    } catch (e: any) {
      console.warn('同步上游模型失败，采用预设基线模型兜底', e);
    } finally {
      setSyncingModels(false);
    }

    // 容灾兜底：若上游接口暂不可用，载入官方推荐基线模型到池中，同样默认不选中
    const fallbackList = preset?.defaultModels ?? (isChatGPT ? [
      'gpt-6.1-sol',
      'gpt-6-astra',
      'gpt-6.1-terra',
      'gpt-6-sol',
      'gpt-reserve',
      'gpt-6-luna',
      'gpt-5.6-sol',
      'gpt-5.6-terra',
      'gpt-5.6-luna',
    ] : [
      'claude-sonnet-4-6',
      'claude-opus-4-6-thinking',
      'gemini-3.8-flash-high',
      'gemini-2.5-pro',
      'gemini-3-flash',
      'gemini-2.5-flash',
      'gpt-oss-120b-medium',
    ]);
    setPool((prev) => uniqueValues([...prev, ...fallbackList]));
    setNotice(
      locale === 'zh-CN'
        ? `上游模型接口受限，已载入 ${fallbackList.length} 个推荐模型到候选池`
        : `Upstream model API restricted, loaded ${fallbackList.length} recommended models into candidate pool`,
    );
    return fallbackList;
  };

  const postAccountConnect = async (successNotice: string) => {
    await loadSubscriptionAccounts();

    // 连接/导入账号后，同步上游模型到候选池（默认不全选）
    const syncedModels = await syncSubscriptionModels(draft.baseUrl);

    // 若当前已有选择的模型，保持不变；若此前完全未选过任何模型，默认仅勾选推荐首选模型（如 gpt-6-astra）
    let nextModels = draft.models.filter((m) => syncedModels.includes(m));
    if (nextModels.length === 0 && syncedModels.length > 0) {
      const primary = syncedModels.find((m) => m === 'gpt-6-astra') || syncedModels[0];
      nextModels = [primary];
    }

    replaceModels(nextModels);

    const providerToSave = prepareProvider({
      ...draft,
      models: nextModels,
      defaultModel: null,
      enabled: true,
    });
    await saveProvider(providerToSave);
    setCurrentEditingProvider(providerToSave);
    setInitial(providerToSave);
    setNotice(successNotice);
  };

  const handleConnectGoogle = async () => {
    try {
      setAuthorizing(true);
      setOauthError(null);
      await invoke<SubscriptionAccount>('start_subscription_oauth', {
        providerId: draft.id,
        accountType: 'antigravity',
      });
      await postAccountConnect(
        locale === 'zh-CN' ? '账号已连接，服务商已自动就绪并启用' : 'Account connected, provider ready and enabled',
      );
    } catch (err: any) {
      setOauthError(
        typeof err === 'string' ? err : err?.message || (locale === 'zh-CN' ? '授权失败，请重试' : 'Authorization failed, please retry'),
      );
    } finally {
      setAuthorizing(false);
    }
  };

  const handleConnectChatGPT = async () => {
    try {
      setAuthorizing(true);
      setOauthError(null);
      await invoke<SubscriptionAccount>('start_subscription_oauth', {
        providerId: draft.id,
        accountType: 'chatgpt',
      });
      await postAccountConnect(
        locale === 'zh-CN' ? 'ChatGPT 订阅账号已连接并启用' : 'ChatGPT account connected and enabled',
      );
    } catch (err: any) {
      setOauthError(
        typeof err === 'string' ? err : err?.message || (locale === 'zh-CN' ? '授权失败，请重试' : 'Authorization failed, please retry'),
      );
    } finally {
      setAuthorizing(false);
    }
  };

  const handleImportCodex = async () => {
    try {
      setImportingCodex(true);
      setOauthError(null);
      await invoke<SubscriptionAccount>('import_codex_subscription_account', {
        providerId: draft.id,
      });
      await postAccountConnect(
        locale === 'zh-CN' ? '已从本地 Codex 导入凭证并启用' : 'Codex credentials imported and enabled',
      );
    } catch (err: any) {
      setOauthError(
        typeof err === 'string'
          ? err
          : err?.message ||
              (locale === 'zh-CN'
                ? '导入失败，请确保本地已登录 Codex CLI'
                : 'Import failed, please ensure local Codex CLI is logged in'),
      );
    } finally {
      setImportingCodex(false);
    }
  };

  const handleFinishSubscription = async () => {
    if (isSubscriptionPool && draft.presetId) {
      const duplicate = providers.find(
        (prov) =>
          prov.id !== draft.id &&
          (prov.presetId === draft.presetId ||
            (prov.authType === 'subscription_pool' && prov.presetId === draft.presetId)),
      );
      if (duplicate) {
        setErrors({
          save:
            locale === 'zh-CN'
              ? `已存在同类型的官方订阅服务商（${duplicate.name}），无需重复创建。`
              : `A subscription provider of this type already exists (${duplicate.name}).`,
        });
        return;
      }
    }
    try {
      setSaving(true);
      await saveProvider(prepareProvider(draft));
      closeProviderEditor();
    } catch (e) {
      setErrors({ save: String(e) });
    } finally {
      setSaving(false);
    }
  };

  const handleDeleteAccount = async (accountId: string) => {
    try {
      await invoke('delete_subscription_account', { accountId });
      await loadSubscriptionAccounts();
    } catch (e) {
      console.error('删除账号失败', e);
    }
  };

  const handleToggleAccount = async (accountId: string, enabled: boolean) => {
    try {
      await invoke('toggle_subscription_account', {
        providerId: draft.id,
        accountId,
        enabled,
      });
      await loadSubscriptionAccounts();
    } catch (e) {
      console.error('切换账号状态失败', e);
    }
  };

  const requestId = useRef(0);
  const drawerRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);

  const preset = presets.find((p) => p.id === draft.presetId);
  const urlEditable = !preset || editablePresetUrl(preset);
  const hasUserChanges = hasActualUserChanges(
    draft,
    initial,
    currentEditingProvider,
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
    // 若为订阅服务商且已有账号：自动保存最新修改并平滑退出，零阻断
    if (isSubscriptionPool && subAccounts.length > 0) {
      void saveProvider(prepareProvider(draft));
      closeProviderEditor();
      return;
    }
    // 1. 若在预设网格页面，返回即关闭编辑器退回主页
    if (choosing) {
      closeProviderEditor();
      return;
    }
    // 2. 若是新增服务商且已进入配置页：返回应该是重选预设
    if (!currentEditingProvider) {
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
    // 若为订阅服务商且已有账号：自动保存最新修改并平滑退出，零阻断
    if (isSubscriptionPool && subAccounts.length > 0) {
      void saveProvider(prepareProvider(draft));
      closeProviderEditor();
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

  const handleCloseRef = useRef(handleClose);
  handleCloseRef.current = handleClose;

  useEffect(() => {
    const handleRequestClose = () => {
      handleCloseRef.current();
    };
    window.addEventListener('pivot:request-close-provider-editor', handleRequestClose);
    return () => {
      window.removeEventListener('pivot:request-close-provider-editor', handleRequestClose);
    };
  }, []);

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

      const target = event.target;
      if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) {
        event.preventDefault();
        if (choosing && presetSearch) {
          setPresetSearch('');
          return;
        }
        if (modelSearch) {
          setModelSearch('');
          return;
        }
        target.blur();
        return;
      }

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

    const existingSub = next ? findExistingProviderForPreset(next) : undefined;
    if (existingSub) {
      setConfirmation(null);
      invalidateModels();
      const baseline = (next?.defaultModels && next.defaultModels.length > 0)
        ? next.defaultModels
        : [];
      let cached: string[] = [];
      try {
        const raw = localStorage.getItem(`pivot_provider_pool_${existingSub.id}`);
        if (raw) cached = JSON.parse(raw);
      } catch {}
      setPool(uniqueValues([...baseline, ...cached, ...(existingSub.models || [])]));
      setCurrentEditingProvider(existingSub);
      const nextDraft = createProviderDraft(existingSub, presets);
      setInitial(nextDraft);
      setDraft(nextDraft);
      setShowKeys(false);
      setShowAuthHelp(false);
      setManualModel('');
      setModelSearch('');
      setErrors({});
      setNotice('');
      setChoosing(false);
      contentRef.current?.scrollTo(0, 0);
      return;
    }

    setConfirmation(null);
    invalidateModels();
    setPool([]);
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
    invalidateModels();
    setLatencyResult(null);
    update({ baseUrl, latencyMs: undefined });
    setNotice(
      locale === 'zh-CN'
        ? '连接信息已更改，已选模型与密钥保留，建议重新获取模型进行验证。'
        : 'Connection info changed, models and keys kept. We recommend fetching models again to verify.',
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
      const unique = uniqueValues(models);
      setFetched(unique);

      // 1. 模型池严格以上游真实返回列表为准，清理并淘汰所有不存在的模型
      setPool(unique);
      if (draft.id) {
        try {
          localStorage.setItem(`pivot_provider_pool_${draft.id}`, JSON.stringify(unique));
        } catch {}
      }

      // 2. 清理 draft.models 中不存在于真实上游列表中的模型，保持已选有效模型不变
      // 获取模型后默认不自动全选，全部可用模型放入候选池供用户自主按需挑选
      const validSelected = draft.models.filter((m) => unique.includes(m));
      const removedCount = draft.models.length - validSelected.length;

      update({
        models: validSelected,
        defaultModel: null,
      });

      if (removedCount > 0) {
        setNotice(
          locale === 'zh-CN'
            ? `已获取上游 ${unique.length} 个可用模型，已自动清理 ${removedCount} 个不存在的过期模型`
            : `Fetched ${unique.length} models from upstream, automatically removed ${removedCount} invalid models`,
        );
      } else {
        setNotice(
          locale === 'zh-CN'
            ? `已获取上游 ${unique.length} 个可用模型，请按需勾选`
            : `Fetched ${unique.length} models from upstream, select as needed`,
        );
      }
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
      defaultModel: null,
    });
  };

  const toggleModel = (model: string) => {
    replaceModels(
      draft.models.includes(model)
        ? draft.models.filter((item) => item !== model)
        : [...draft.models, model],
    );
  };

  const handleCopyModel = (e: React.MouseEvent, modelId: string) => {
    e.stopPropagation();
    navigator.clipboard.writeText(modelId);
    setCopiedModel(modelId);
    setTimeout(() => {
      setCopiedModel((prev) => (prev === modelId ? null : prev));
    }, 1500);
  };

  const handleRemoveModelFromPool = (e: React.MouseEvent | React.KeyboardEvent, model: string) => {
    e.stopPropagation();
    setPool((prev) => {
      const next = prev.filter((m) => m !== model);
      if (draft.id) {
        try {
          localStorage.setItem(`pivot_provider_pool_${draft.id}`, JSON.stringify(next));
        } catch {}
      }
      return next;
    });
    if (fetched) {
      setFetched((prev) => (prev ? prev.filter((m) => m !== model) : null));
    }
    if (draft.models.includes(model)) {
      replaceModels(draft.models.filter((m) => m !== model));
    }
  };

  const addManualModel = () => {
    const model = manualModel.trim();
    if (!model || /[\s,，]/u.test(model)) {
      setErrors({
        models:
          locale === 'zh-CN'
            ? '请输入一个完整的模型 ID，不要包含空格或逗号'
            : 'Please enter a complete model ID without spaces or commas',
      });
      return;
    }
    replaceModels([...draft.models, model]);
    setManualModel('');
  };

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (saving) return;

    const nextErrors: Record<string, string> = {};
    if (!draft.name.trim())
      nextErrors.name = locale === 'zh-CN' ? '请输入服务商名称' : 'Please enter provider name';
    const urlError = validateBaseUrl(draft.baseUrl);
    if (urlError) nextErrors.baseUrl = urlError;
    if (!draft.models.length)
      nextErrors.models =
        locale === 'zh-CN' ? '请至少选择或手动添加一个模型' : 'Please select or add at least one model';
    if (manualModel.trim())
      nextErrors.models =
        locale === 'zh-CN'
          ? '请先点击添加正在输入的模型，或清空输入框'
          : 'Please click Add to add current input or clear input box';

    setErrors(nextErrors);
    if (Object.keys(nextErrors).length) {
      requestAnimationFrame(() =>
        drawerRef.current?.querySelector<HTMLElement>('[aria-invalid="true"]')?.focus(),
      );
      return;
    }

    if (isSubscriptionPool && draft.presetId) {
      const duplicate = providers.find(
        (prov) =>
          prov.id !== draft.id &&
          (prov.presetId === draft.presetId ||
            (prov.authType === 'subscription_pool' && prov.presetId === draft.presetId)),
      );
      if (duplicate) {
        setErrors({
          save:
            locale === 'zh-CN'
              ? `已存在同类型的官方订阅服务商（${duplicate.name}），无需重复创建。`
              : `A subscription provider of this type already exists (${duplicate.name}).`,
        });
        return;
      }
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

  // 同步模型池：保持已选模型在候选池中，避免未保存的新增模型丢失
  useEffect(() => {
    setPool((prev) => {
      const next = uniqueValues([...prev, ...draft.models]);
      return next.length === prev.length ? prev : next;
    });
  }, [draft.models]);

  // 模型能力标记：目录未收录的模型不显示任何标记
  const [capabilities, setCapabilities] = useState<Record<string, ModelCapability>>({});
  useEffect(() => {
    if (pool.length === 0) return;
    let cancelled = false;
    invoke<Record<string, ModelCapability>>('get_model_capabilities', { modelNames: pool })
      .then((caps) => {
        if (!cancelled) setCapabilities(caps);
      })
      .catch(() => {
        if (!cancelled) setCapabilities({});
      });
    return () => {
      cancelled = true;
    };
  }, [pool]);

  const filteredModels = useMemo(() => {
    const q = modelSearch.trim().toLowerCase();
    return pool.filter((model) => {
      const matchesSearch = !q || model.toLowerCase().includes(q);
      if (!matchesSearch) return false;
      const isSelected = draft.models.includes(model);
      if (modelFilterTab === 'selected') return isSelected;
      if (modelFilterTab === 'unselected') return !isSelected;
      return true;
    });
  }, [pool, modelSearch, modelFilterTab, draft.models]);

  const selectAllFiltered = () => {
    const unselected = filteredModels.filter((m) => !draft.models.includes(m));
    if (unselected.length > 0) {
      replaceModels([...draft.models, ...unselected]);
    }
  };

  const clearAllFiltered = () => {
    const selectedInFiltered = filteredModels.filter((m) => draft.models.includes(m));
    if (selectedInFiltered.length > 0) {
      replaceModels(draft.models.filter((m) => !selectedInFiltered.includes(m)));
    }
  };

  const availableFamilies = useMemo(() => {
    if (pool.length < 5) return [];
    const candidates = [
      { key: 'claude', label: 'Claude', test: (m: string) => /claude/i.test(m) },
      { key: 'gpt', label: 'GPT', test: (m: string) => /gpt|o1|o3|o4/i.test(m) },
      { key: 'gemini', label: 'Gemini', test: (m: string) => /gemini/i.test(m) },
      { key: 'deepseek', label: 'DeepSeek', test: (m: string) => /deepseek/i.test(m) },
      { key: 'qwen', label: 'Qwen', test: (m: string) => /qwen/i.test(m) },
    ];
    return candidates
      .map((c) => ({ ...c, count: pool.filter(c.test).length }))
      .filter((c) => c.count >= 2);
  }, [pool]);

  const invalidModels = useMemo(() => {
    if (!fetched || fetched.length === 0) return [];
    return draft.models.filter((model) => !fetched.includes(model));
  }, [fetched, draft.models]);

  const removeInvalidModels = () => {
    if (!fetched || fetched.length === 0) return;
    replaceModels(draft.models.filter((model) => fetched.includes(model)));
  };

  return (
    <div className="w-full pb-6 animate-in fade-in duration-150">
      {/* 页面主卡片 */}
      <div
        ref={drawerRef}
        role="region"
        aria-label={currentEditingProvider ? t('providerEditor.editProvider') : t('providerEditor.newProvider')}
        className="p-4 rounded-lg border border-border bg-card text-card-foreground shadow-2xs space-y-4"
      >
        {/* 表单内容 */}
        <div ref={contentRef} className="space-y-6">
          {choosing ? (
            /* 选择服务商预设 */
            <div className="space-y-2">
              <div className="flex items-center gap-2">
                <div className="relative flex-1">
                  <Search className="w-3.5 h-3.5 absolute left-2.5 top-1/2 -translate-y-1/2 text-zinc-400 pointer-events-none" />
                  <Input
                    autoFocus
                    type="text"
                    placeholder={t('providerEditor.searchPresets')}
                    value={presetSearch}
                    onChange={(e) => setPresetSearch(e.target.value)}
                    className="pl-8 pr-8 h-8 text-xs bg-zinc-500/[0.05] dark:bg-white/[0.05] border-transparent shadow-none focus-visible:border-transparent focus-visible:ring-zinc-400/40 dark:focus-visible:ring-zinc-500/40"
                  />
                  {presetSearch && (
                    <button
                      type="button"
                      onClick={() => setPresetSearch('')}
                      className="absolute right-2 top-1/2 -translate-y-1/2 text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-200 p-0.5 cursor-pointer"
                      title={t('common.close')}
                    >
                      <X className="w-3.5 h-3.5" />
                    </button>
                  )}
                </div>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={handleClose}
                  className="h-8 px-3 text-xs shrink-0 cursor-pointer"
                >
                  {t('providerEditor.backBtn')}
                </Button>
              </div>

              {matchingPresets.length === 0 ? (
                <div className="py-10 text-center space-y-2">
                  <p className="text-xs text-zinc-400">{locale === 'zh-CN' ? '没有匹配的服务商预设' : 'No matching presets'}</p>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={() => choosePreset()}
                  >
                    <Plus className="w-3.5 h-3.5" />
                    <span>{t('providerEditor.customProvider')}</span>
                  </Button>
                </div>
              ) : (
                <div className="grid grid-cols-2 sm:grid-cols-3 gap-1.5">
                  {/* 自定义连接：与品牌条目同构，仅图标槽换为加号 */}
                  <button
                    type="button"
                    onClick={() => choosePreset()}
                    className="h-10 px-2 rounded-md flex items-center gap-2.5 text-left transition-colors cursor-pointer hover:bg-accent/60"
                  >
                    <span className={`w-6 h-6 shrink-0 flex items-center justify-center ${ICON_SLOT} text-zinc-500 dark:text-zinc-400`}>
                      <Plus className="w-3.5 h-3.5" />
                    </span>
                    <span className="text-xs font-medium text-zinc-600 dark:text-zinc-300 truncate">
                      {t('providerEditor.customProvider')}
                    </span>
                  </button>

                  {matchingPresets.map((p) => {
                    const cleanName = p.name.replace(/[\(（][^\)）]+[\)）]/g, '').trim();
                    const isSub = p.authType === 'subscription_pool' || p.category === '官方订阅';
                    const existingSub = isSub ? findExistingProviderForPreset(p) : undefined;
                    return (
                      <button
                        key={p.id}
                        type="button"
                        onClick={() => choosePreset(p)}
                        className="h-10 px-2 rounded-md flex items-center justify-between gap-2 text-left transition-colors cursor-pointer hover:bg-accent/60 group"
                      >
                        <div className="flex items-center gap-2.5 min-w-0">
                          <ProviderAvatar
                            name={cleanName}
                            protocol={p.protocol}
                            iconKey={p.icon || p.id}
                            sizeClass="w-6 h-6"
                            dense
                          />
                          <span className="text-xs font-medium text-foreground truncate">
                            {cleanName}
                          </span>
                        </div>
                        {existingSub ? (
                          <Badge
                            variant="secondary"
                            className="text-[10px] px-1.5 py-0.5 font-normal shrink-0 text-emerald-600 dark:text-emerald-400 bg-emerald-500/10 border border-emerald-500/20"
                          >
                            {locale === 'zh-CN' ? '已添加' : 'Added'}
                          </Badge>
                        ) : isSub ? (
                          <Badge variant="secondary" className="text-[10px] px-1.5 py-0.5 font-normal shrink-0">
                            {t('providerEditor.categories.official')}
                          </Badge>
                        ) : null}
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
                  {t('providerEditor.basicSettings')}
                </h3>

                {/* 服务商名称 */}
                <div>
                  <label className="block text-xs font-medium text-zinc-700 dark:text-zinc-300 mb-1">
                    {t('providerEditor.providerName')}
                  </label>
                  <Input
                    type="text"
                    value={draft.name}
                    onChange={(e) => update({ name: e.target.value })}
                    placeholder={t('providerEditor.providerNamePlaceholder')}
                  />
                  {errors.name && <p className="text-[11px] text-rose-500 mt-1">{errors.name}</p>}
                </div>

                {/* 协议与 URL：普通 API 服务商在此直接显示；订阅账号服务商收敛为状态说明，接口地址收起在底部高级设置 */}
                {isSubscriptionPool ? (
                  <div className="flex items-center justify-between p-2.5 rounded-lg bg-zinc-500/[0.04] dark:bg-white/[0.04] border border-border text-xs text-foreground">
                    <div className="flex items-center gap-2">
                      <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 shrink-0" />
                      <span className="font-medium">{t('providerEditor.categories.official')}</span>
                      <span className="text-[11px] text-muted-foreground hidden sm:inline">
                        {locale === 'zh-CN' ? '· 浏览器授权登录，支持多账号自动轮询与限流冷却' : '· Browser authorization, multi-account rotation & cooldown'}
                      </span>
                    </div>
                    <Badge variant="secondary" className="font-mono text-[10px]">
                      OAuth
                    </Badge>
                  </div>
                ) : (
                  <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
                    <div>
                      <label className="block text-xs font-medium text-zinc-700 dark:text-zinc-300 mb-1">
                        {t('providerEditor.protocol')}
                      </label>
                      {preset ? (
                        <div className="px-3 py-2 text-xs font-mono rounded-md border border-zinc-200 dark:border-zinc-800 bg-zinc-50 dark:bg-zinc-800/50 text-zinc-700 dark:text-zinc-300">
                          {t(`providerEditor.protocols.${draft.protocol}`)}
                        </div>
                      ) : (
                        <Select
                          value={draft.protocol}
                          onValueChange={(val) => {
                            invalidateModels();
                            update({
                              protocol: val as Protocol,
                              models: [],
                            });
                            setNotice(
                              locale === 'zh-CN'
                                ? '协议已更改，密钥已保留，请按需选择或获取可用模型。'
                                : 'Protocol changed, keys kept. Please select or fetch available models.',
                            );
                          }}
                        >
                          <SelectTrigger className="w-full">
                            <SelectValue />
                          </SelectTrigger>
                          <SelectContent>
                            {PROTOCOLS.map((id) => (
                              <SelectItem key={id} value={id}>
                                {t(`providerEditor.protocols.${id}`)}
                              </SelectItem>
                            ))}
                          </SelectContent>
                        </Select>
                      )}
                    </div>

                    <div className="sm:col-span-2">
                      <div className="flex items-center justify-between mb-1">
                        <label className="block text-xs font-medium text-zinc-700 dark:text-zinc-300">
                          {t('providerEditor.baseUrl')}
                        </label>
                        {draft.baseUrl.trim() && (
                          <div className="flex items-center gap-1.5 text-xs">
                            {testingConnection ? (
                              <span className="inline-flex items-center gap-1 text-[11px] text-zinc-400">
                                <LoaderCircle className="w-3 h-3 animate-spin text-zinc-500" />
                                <span>{t('providerEditor.testingConnection')}</span>
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
                                  {locale === 'zh-CN' ? '重测' : 'Retest'}
                                </button>
                              </span>
                            ) : (
                              <button
                                type="button"
                                onClick={testConnection}
                                className="text-[11px] text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100 flex items-center gap-0.5 cursor-pointer transition-colors"
                              >
                                <Zap className="w-3 h-3 shrink-0" />
                                <span>{t('providerEditor.testConnection')}</span>
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
                )}

                {/* 认证方式：官方订阅账号池 vs API 密钥 */}
                <div>
                  {isSubscriptionPool ? (
                    <div className="space-y-3">
                      <div className="flex items-center justify-between">
                        <div className="flex items-center gap-2">
                          <label className="text-xs font-bold text-foreground tracking-wider">
                            {t('providerEditor.accountPool')}
                          </label>
                          <Badge variant="secondary" className="font-mono text-[10px]">
                            {subAccounts.length > 0
                              ? t('providerEditor.accountAvailable', {
                                  available: subAccounts.filter((a) => a.enabled).length,
                                  total: subAccounts.length,
                                })
                              : t('providerEditor.notConnected')}
                          </Badge>
                          {subAccounts.length > 1 && (
                            <span className="text-[10px] text-muted-foreground hidden sm:inline select-none">
                              {t('providerEditor.autoRoundRobin')}
                            </span>
                          )}
                          {loadingAccounts && <LoaderCircle className="w-3 h-3 animate-spin text-muted-foreground" />}
                        </div>
                        <div className="flex items-center gap-2 text-[11px]">
                          {isChatGPT ? (
                            <>
                              <Button
                                type="button"
                                variant="outline"
                                size="sm"
                                disabled={importingCodex || authorizing}
                                onClick={handleImportCodex}
                              >
                                {importingCodex ? (
                                  <>
                                    <LoaderCircle className="w-3 h-3 animate-spin mr-1" />
                                    <span>{t('providerEditor.importing')}</span>
                                  </>
                                ) : (
                                  <span>{t('providerEditor.importLocalCreds')}</span>
                                )}
                              </Button>
                              <Button
                                type="button"
                                variant="outline"
                                size="sm"
                                disabled={authorizing || importingCodex}
                                onClick={handleConnectChatGPT}
                              >
                                {authorizing ? (
                                  <>
                                    <LoaderCircle className="w-3 h-3 animate-spin mr-1" />
                                    <span>{t('providerEditor.waitingBrowserAuth')}</span>
                                  </>
                                ) : (
                                  <>
                                    <Plus className="w-3 h-3 mr-1" />
                                    <span>{t('providerEditor.addAccount')}</span>
                                  </>
                                )}
                              </Button>
                            </>
                          ) : (
                            <Button
                              type="button"
                              variant="outline"
                              size="sm"
                              disabled={authorizing}
                              onClick={handleConnectGoogle}
                            >
                              {authorizing ? (
                                <>
                                  <LoaderCircle className="w-3 h-3 animate-spin mr-1" />
                                  <span>{t('providerEditor.waitingBrowserAuth')}</span>
                                </>
                              ) : (
                                <>
                                  <Plus className="w-3 h-3 mr-1" />
                                  <span>{t('providerEditor.addAccount')}</span>
                                </>
                              )}
                            </Button>
                          )}
                        </div>
                      </div>

                      {oauthError && (
                        <div className="p-2.5 rounded-lg bg-rose-50 dark:bg-rose-950/40 text-[11px] text-rose-600 dark:text-rose-400 border border-rose-200 dark:border-rose-900/60 flex items-start gap-2">
                          <AlertCircle className="w-3.5 h-3.5 mt-0.5 shrink-0" />
                          <span>{oauthError}</span>
                        </div>
                      )}

                      {subAccounts.length === 0 ? (
                        <div className="p-6 rounded-xl border border-dashed border-border text-center bg-zinc-500/[0.02]">
                          <div className="w-10 h-10 rounded-full bg-primary/10 text-primary mx-auto flex items-center justify-center mb-2.5">
                            <Zap className="w-5 h-5" />
                          </div>
                          <div className="text-xs font-semibold text-foreground mb-1">
                            {t('providerEditor.noAccountsConnected')}
                          </div>
                          <div className="text-[11px] text-muted-foreground max-w-sm mx-auto mb-3.5 leading-relaxed">
                            {isChatGPT
                              ? t('providerEditor.connectAccountChatGPTDesc')
                              : t('providerEditor.connectAccountGoogleDesc')}
                          </div>
                          {isChatGPT ? (
                            <div className="flex items-center justify-center gap-2">
                              <Button
                                type="button"
                                variant="outline"
                                size="sm"
                                disabled={importingCodex || authorizing}
                                onClick={handleImportCodex}
                              >
                                {importingCodex ? (
                                  <>
                                    <LoaderCircle className="w-3.5 h-3.5 animate-spin mr-1.5" />
                                    <span>{t('providerEditor.importing')}</span>
                                  </>
                                ) : (
                                  <span>{t('providerEditor.importFromCodexBtn')}</span>
                                )}
                              </Button>
                              <Button
                                type="button"
                                variant="default"
                                size="sm"
                                disabled={authorizing || importingCodex}
                                onClick={handleConnectChatGPT}
                              >
                                {authorizing ? (
                                  <>
                                    <LoaderCircle className="w-3.5 h-3.5 animate-spin mr-1.5" />
                                    <span>{t('providerEditor.pleaseAuthorizeInBrowser')}</span>
                                  </>
                                ) : (
                                  <>
                                    <Plus className="w-3.5 h-3.5 mr-1.5" />
                                    <span>{t('providerEditor.webAuthLogin')}</span>
                                  </>
                                )}
                              </Button>
                            </div>
                          ) : (
                            <Button
                              type="button"
                              variant="default"
                              size="sm"
                              disabled={authorizing}
                              onClick={handleConnectGoogle}
                            >
                              {authorizing ? (
                                <>
                                  <LoaderCircle className="w-3.5 h-3.5 animate-spin mr-1.5" />
                                  <span>{t('providerEditor.pleaseAuthorizeInBrowser')}</span>
                                </>
                              ) : (
                                <>
                                  <Plus className="w-3.5 h-3.5 mr-1.5" />
                                  <span>{t('providerEditor.connectAccountNow')}</span>
                                </>
                              )}
                            </Button>
                          )}
                        </div>
                      ) : (
                        <div className="space-y-2">
                          {subAccounts.map((acc) => {
                            const nowSec = Math.floor(Date.now() / 1000);
                            const isCooldown = acc.cooldownUntil > nowSec;
                            const cooldownRemaining = Math.ceil((acc.cooldownUntil - nowSec) / 60);
                            const quota = accountQuotas[acc.id];
                            const isRefreshingThis = refreshingQuotaId === acc.id;
                            const isExpanded = !!expandedQuotaAccounts[acc.id];

                            return (
                              <div
                                key={acc.id}
                                className="p-2.5 rounded-lg border border-border/80 bg-background hover:border-zinc-300 dark:hover:border-zinc-700 transition-colors shadow-2xs"
                              >
                                <div className="flex items-center justify-between gap-3">
                                  <div className="flex items-center gap-2.5 min-w-0">
                                    <span className="text-xs font-medium text-foreground truncate">
                                      {acc.email || (locale === 'zh-CN' ? `账号 ${acc.id.slice(0, 8)}` : `Account ${acc.id.slice(0, 8)}`)}
                                    </span>
                                    {!acc.enabled ? (
                                      <span className="text-[10px] text-zinc-400 dark:text-zinc-500 shrink-0">
                                        {t('providerEditor.accountDisabled')}
                                      </span>
                                    ) : isCooldown ? (
                                      <span className="text-[10px] text-amber-600 dark:text-amber-400 font-medium shrink-0">
                                        {t('providerEditor.cooldown', { minutes: cooldownRemaining })}
                                      </span>
                                    ) : (
                                      <span className="text-[10px] text-emerald-600 dark:text-emerald-400 font-medium flex items-center gap-1 shrink-0">
                                        <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 inline-block" />
                                        <span>{t('providerEditor.activeNormal')}</span>
                                      </span>
                                    )}
                                  </div>

                                  <div className="flex items-center gap-1.5 shrink-0">
                                    <button
                                      type="button"
                                      onClick={() => toggleExpandQuota(acc.id)}
                                      className="h-7 px-2 rounded-md text-[11px] font-medium text-zinc-600 dark:text-zinc-300 hover:text-zinc-900 dark:hover:text-zinc-100 hover:bg-zinc-100 dark:hover:bg-zinc-800/80 cursor-pointer transition-colors flex items-center gap-1"
                                      title={isExpanded ? t('providerEditor.collapseQuotaTooltip') : t('providerEditor.viewQuotaTooltip')}
                                    >
                                      <span>{isExpanded ? t('providerEditor.collapseQuota') : t('providerEditor.viewQuota')}</span>
                                      {isExpanded ? (
                                        <ChevronUp className="w-3 h-3 text-zinc-400" />
                                      ) : (
                                        <ChevronDown className="w-3 h-3 text-zinc-400" />
                                      )}
                                    </button>
                                    <div className="w-[1px] h-3.5 bg-border/60 mx-0.5" />
                                    <Switch
                                      checked={acc.enabled}
                                      onCheckedChange={(checked) => handleToggleAccount(acc.id, checked)}
                                      ariaLabel={t('providerEditor.enableAccountAria')}
                                    />
                                    <button
                                      type="button"
                                      onClick={() => handleDeleteAccount(acc.id)}
                                      className="p-1 rounded-md text-zinc-400 hover:text-rose-500 hover:bg-zinc-100 dark:hover:bg-zinc-800 cursor-pointer transition-colors"
                                      title={t('providerEditor.deleteAccountTitle')}
                                    >
                                      <X className="w-3.5 h-3.5" />
                                    </button>
                                  </div>
                                </div>

                                {/* 额度按需展开展示 */}
                                {isExpanded && (
                                  <div className="mt-2.5 pt-2 border-t border-border/50">
                                    <div className="flex items-center justify-between mb-2">
                                      <span className="text-[10px] font-medium text-zinc-400 dark:text-zinc-500">
                                        {isChatGPT ? t('providerEditor.chatgptQuotaTitle') : t('providerEditor.googleQuotaTitle')}
                                      </span>
                                      <button
                                        type="button"
                                        onClick={() => fetchAccountQuota(acc.id)}
                                        disabled={isRefreshingThis}
                                        className="text-[10px] text-zinc-400 hover:text-zinc-700 dark:hover:text-zinc-200 flex items-center gap-1 cursor-pointer transition-colors"
                                      >
                                        <RefreshCw className={clsx("w-3 h-3", isRefreshingThis && "animate-spin text-primary")} />
                                        <span>{isRefreshingThis ? t('providerEditor.refreshing') : t('providerEditor.refresh')}</span>
                                      </button>
                                    </div>

                                    {isRefreshingThis && !quota ? (
                                      <div className="py-2.5 flex items-center justify-center gap-2 text-[11px] text-muted-foreground">
                                        <LoaderCircle className="w-3.5 h-3.5 animate-spin" />
                                        <span>{t('providerEditor.fetchingQuota')}</span>
                                      </div>
                                    ) : quota && (quota.buckets.length > 0 || (quota.resetCredits && quota.resetCredits.length > 0)) ? (
                                      <div className="space-y-2">
                                        {quota.buckets.length > 0 && (
                                          <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
                                            {quota.buckets.map((b) => {
                                              const pct = Math.round(b.remainingFraction * 100);
                                              const resetDesc = formatResetTime(b.resetTime);
                                              const isLow = b.remainingFraction <= 0.2;
                                              const isOut = b.isExhausted;

                                              return (
                                                <div key={b.id} className="space-y-1">
                                                  <div className="flex items-center justify-between text-[10px]">
                                                    <span className="text-zinc-500 dark:text-zinc-400 font-medium truncate">{b.label}</span>
                                                    <span
                                                      className={clsx(
                                                        "font-mono shrink-0",
                                                        isOut
                                                          ? "text-rose-500 dark:text-rose-400 font-semibold"
                                                          : isLow
                                                          ? "text-amber-500 font-medium"
                                                          : "text-zinc-600 dark:text-zinc-300"
                                                      )}
                                                    >
                                                      {isOut
                                                        ? `${t('providerEditor.exhausted')}${resetDesc ? ` · ${resetDesc}` : ''}`
                                                        : `${pct}%${resetDesc ? ` · ${resetDesc}` : ''}`}
                                                    </span>
                                                  </div>
                                                  <div className="w-full h-1.5 rounded-full bg-zinc-100 dark:bg-zinc-800/80 overflow-hidden">
                                                    <div
                                                      className={clsx(
                                                        "h-full rounded-full transition-all duration-300",
                                                        isOut ? "bg-rose-500" : isLow ? "bg-amber-500" : "bg-emerald-500"
                                                      )}
                                                      style={{ width: `${Math.max(pct, isOut ? 0 : 3)}%` }}
                                                    />
                                                  </div>
                                                </div>
                                              );
                                            })}
                                          </div>
                                        )}

                                        {/* 限流重置卡 */}
                                        {quota.resetCredits && quota.resetCredits.length > 0 && (
                                          <div className="mt-2.5 pt-2 border-t border-border/50 space-y-1.5">
                                            <div className="flex items-center justify-between">
                                              <div className="flex items-center gap-1.5 text-[10px] font-medium text-zinc-500 dark:text-zinc-400">
                                                <Sparkles className="w-3 h-3 text-amber-500" />
                                                <span>{t('providerEditor.resetCredits')}</span>
                                                <Badge variant="secondary" className="text-[9px] px-1 py-0 h-4 font-mono">
                                                  {t('providerEditor.cardsAvailable', { count: quota.resetCredits.length })}
                                                </Badge>
                                              </div>
                                            </div>
                                            <div className="space-y-1">
                                              {quota.resetCredits.map((credit, idx) => (
                                                <div
                                                  key={credit.id || idx}
                                                  className="p-1.5 px-2 rounded-md bg-zinc-500/[0.03] border border-border/60 flex items-center justify-between gap-3 text-xs"
                                                >
                                                  <div className="min-w-0">
                                                    <span className="text-[10px] text-muted-foreground">
                                                      {t('providerEditor.expiryTime')}{formatCreditExpiry(credit.expiresAt)}
                                                    </span>
                                                  </div>
                                                  <Button
                                                    type="button"
                                                    variant="outline"
                                                    size="sm"
                                                    className="h-5 px-1.5 text-[10px] shrink-0 text-primary hover:text-primary hover:bg-primary/10 border-primary/30"
                                                    disabled={consumingCreditAccountId === acc.id}
                                                    onClick={() => handleConsumeCredit(acc.id)}
                                                  >
                                                    {consumingCreditAccountId === acc.id ? (
                                                      <>
                                                        <LoaderCircle className="w-2.5 h-2.5 animate-spin mr-1" />
                                                        <span>{t('providerEditor.usingCredit')}</span>
                                                      </>
                                                    ) : (
                                                      <span>{t('providerEditor.useResetCredit')}</span>
                                                    )}
                                                  </Button>
                                                </div>
                                              ))}
                                            </div>
                                          </div>
                                        )}
                                      </div>
                                    ) : (
                                      <div className="py-2 text-center text-[10px] text-muted-foreground">
                                        {t('providerEditor.noQuotaFetched')}
                                      </div>
                                    )}
                                  </div>
                                )}
                              </div>
                            );
                          })}
                        </div>
                      )}
                    </div>
                  ) : (
                    <div>
                      <div className="flex items-center justify-between mb-1.5">
                        <label className="block text-xs font-medium text-zinc-700 dark:text-zinc-300">
                          {t('providerEditor.apiKeyLabel')}
                        </label>
                        <div className="flex items-center gap-2 text-[11px]">
                          <button
                            type="button"
                            onClick={() => setShowAuthHelp(!showAuthHelp)}
                            className="text-zinc-400 hover:text-zinc-700 dark:hover:text-zinc-200 flex items-center gap-1 cursor-pointer transition-colors"
                          >
                            <HelpCircle className="w-3 h-3" />
                            <span>{t('providerEditor.keyHelp')}</span>
                          </button>
                          <span className="text-zinc-300 dark:text-zinc-700">·</span>
                          <button
                            type="button"
                            onClick={() => update({ apiKeys: [...keyInputs, ''] })}
                            className="text-zinc-500 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100 font-medium cursor-pointer transition-colors"
                          >
                            {t('providerEditor.multiKeyRotation')}
                          </button>
                        </div>
                      </div>

                      {showAuthHelp && (
                        <div className="mb-2 p-2.5 rounded-lg bg-zinc-50 dark:bg-zinc-800 text-[11px] text-zinc-600 dark:text-zinc-300 leading-relaxed border border-zinc-200/60 dark:border-zinc-700/60">
                          {t('providerEditor.keyHelpText')}
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
                                placeholder={index ? t('providerEditor.addSecondaryKey') : t('providerEditor.primaryKeyPlaceholder')}
                                className="font-mono pr-9"
                              />
                              <button
                                type="button"
                                onClick={() => setShowKeys(!showKeys)}
                                className="absolute right-2.5 top-1/2 -translate-y-1/2 text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-200 cursor-pointer"
                                aria-label={showKeys ? t('providerEditor.hideKey') : t('providerEditor.showPlainKey')}
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
                  )}
                </div>
              </div>

              {/* 模型选择与声明：一体化精简清单 */}
              <div className="space-y-3 pt-4 border-t border-border">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <h3 className="text-xs font-bold text-zinc-900 dark:text-zinc-100 uppercase tracking-wider">
                      {t('providerEditor.availableModelPool')}
                    </h3>
                    <Badge variant="secondary" className="font-mono">
                      {t('providerEditor.selectedCount', {
                        selected: draft.models.length,
                        total: pool.length > 0 ? (locale === 'zh-CN' ? `/ 共 ${pool.length}` : `/ ${pool.length}`) : '',
                      })}
                    </Badge>
                    {invalidModels.length > 0 && (
                      <Badge variant="warning" className="gap-1">
                        <AlertTriangle className="w-3 h-3 shrink-0" />
                        <span>{t('providerEditor.modelsNotProvided', { count: invalidModels.length })}</span>
                        <button
                          type="button"
                          onClick={removeInvalidModels}
                          className="underline hover:opacity-80 cursor-pointer ml-0.5"
                          title={t('providerEditor.removeInvalidTitle')}
                        >
                          {t('providerEditor.removeInvalid')}
                        </button>
                      </Badge>
                    )}
                  </div>

                  {isSubscriptionPool ? (
                    subAccounts.length > 0 ? (
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        onClick={() => syncSubscriptionModels(draft.baseUrl)}
                        disabled={syncingModels}
                        className="h-7 text-xs"
                      >
                        <RefreshCw className={`w-3.5 h-3.5 mr-1 ${syncingModels ? 'animate-spin' : ''}`} />
                        <span>{syncingModels ? t('providerEditor.syncing') : t('providerEditor.syncAccountModels')}</span>
                      </Button>
                    ) : null
                  ) : (
                    <Button
                      type="button"
                      variant="secondary"
                      size="sm"
                      onClick={fetchModels}
                      disabled={fetching}
                    >
                      <RefreshCw className={`w-3.5 h-3.5 ${fetching ? 'animate-spin' : ''}`} />
                      <span>{fetching ? t('providerEditor.fetchingModels') : fetched ? t('providerEditor.refreshUpstream') : t('providerEditor.fetchUpstreamModels')}</span>
                    </Button>
                  )}
                </div>

                {fetchError && (
                  <div className="px-3 py-2 rounded-md bg-rose-50 dark:bg-rose-950/40 border border-rose-200 dark:border-rose-900/60 text-xs text-rose-700 dark:text-rose-300 flex items-center gap-2">
                    <AlertCircle className="w-3.5 h-3.5 shrink-0 text-rose-500" />
                    <span className="truncate">{fetchError}</span>
                  </div>
                )}

                {/* 一体化清单卡片 */}
                <div className="rounded-lg border border-border/70 bg-zinc-500/[0.035] dark:bg-white/[0.035] overflow-hidden">
                  {/* 顶部搜索、视图切换与快捷操作栏 */}
                  <div className="flex flex-wrap items-center justify-between gap-2 px-3 py-2 border-b border-border/60 bg-background/50">
                    <div className="relative flex-1 min-w-[140px]">
                      <Search className="w-3.5 h-3.5 absolute left-2.5 top-1/2 -translate-y-1/2 text-zinc-400 pointer-events-none" />
                      <Input
                        type="text"
                        placeholder={t('providerEditor.searchModelPlaceholder')}
                        value={modelSearch}
                        onChange={(e) => setModelSearch(e.target.value)}
                        className="pl-8 pr-7 h-7 font-mono text-xs bg-background/80"
                      />
                      {modelSearch && (
                        <button
                          type="button"
                          onClick={() => setModelSearch('')}
                          className="absolute right-2 top-1/2 -translate-y-1/2 text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-200 p-0.5 cursor-pointer"
                          title={t('providerEditor.clearSearchTooltip')}
                        >
                          <X className="w-3 h-3" />
                        </button>
                      )}
                    </div>

                    {pool.length > 0 && (
                      <div className="flex items-center gap-2 shrink-0">
                        {/* 视图分段选择：全部 / 已选 / 未选 */}
                        <div className="inline-flex p-0.5 rounded-md bg-zinc-500/[0.08] dark:bg-white/[0.08] text-[11px] font-medium border border-border/40">
                          <button
                            type="button"
                            onClick={() => setModelFilterTab('all')}
                            className={`px-2 py-0.5 rounded border border-transparent transition-[color,background-color,border-color,box-shadow] duration-150 cursor-pointer outline-none ${
                              modelFilterTab === 'all'
                                ? 'bg-background text-foreground shadow-2xs border-black/5 dark:border-white/10'
                                : 'text-zinc-500 dark:text-zinc-400 hover:text-foreground'
                            }`}
                          >
                            {t('providerEditor.filterTabs.all')} <span className="opacity-70 font-mono text-[10px] ml-0.5">{pool.length}</span>
                          </button>
                          <button
                            type="button"
                            onClick={() => setModelFilterTab('selected')}
                            className={`px-2 py-0.5 rounded border border-transparent transition-[color,background-color,border-color,box-shadow] duration-150 cursor-pointer outline-none ${
                              modelFilterTab === 'selected'
                                ? 'bg-background text-foreground shadow-2xs border-black/5 dark:border-white/10'
                                : 'text-zinc-500 dark:text-zinc-400 hover:text-foreground'
                            }`}
                          >
                            {t('providerEditor.filterTabs.selected')} <span className="opacity-70 font-mono text-[10px] ml-0.5">{draft.models.length}</span>
                          </button>
                          <button
                            type="button"
                            onClick={() => setModelFilterTab('unselected')}
                            className={`px-2 py-0.5 rounded border border-transparent transition-[color,background-color,border-color,box-shadow] duration-150 cursor-pointer outline-none ${
                              modelFilterTab === 'unselected'
                                ? 'bg-background text-foreground shadow-2xs border-black/5 dark:border-white/10'
                                : 'text-zinc-500 dark:text-zinc-400 hover:text-foreground'
                            }`}
                          >
                            {t('providerEditor.filterTabs.unselected')} <span className="opacity-70 font-mono text-[10px] ml-0.5">{Math.max(0, pool.length - draft.models.length)}</span>
                          </button>
                        </div>

                        {/* 快捷批量操作 */}
                        <div className="flex items-center gap-1 pl-1 border-l border-border/60 text-[11px] text-zinc-500 dark:text-zinc-400">
                          <button
                            type="button"
                            onClick={selectAllFiltered}
                            className="px-1.5 py-0.5 rounded hover:bg-zinc-200/60 dark:hover:bg-zinc-800 hover:text-zinc-900 dark:hover:text-zinc-100 transition-colors cursor-pointer"
                            title={t('providerEditor.selectAllTooltip')}
                          >
                            {modelSearch.trim()
                              ? t('providerEditor.selectAllResults')
                              : modelFilterTab === 'unselected'
                              ? t('providerEditor.selectAllUnselected')
                              : t('providerEditor.selectAll')}
                          </button>
                          <button
                            type="button"
                            onClick={clearAllFiltered}
                            className="px-1.5 py-0.5 rounded hover:bg-zinc-200/60 dark:hover:bg-zinc-800 hover:text-zinc-900 dark:hover:text-zinc-100 transition-colors cursor-pointer"
                            title={t('providerEditor.clearAllTooltip')}
                          >
                            {modelSearch.trim()
                              ? t('providerEditor.clearAllResults')
                              : modelFilterTab === 'selected'
                              ? t('providerEditor.clearAllSelected')
                              : t('providerEditor.clearAll')}
                          </button>
                        </div>
                      </div>
                    )}
                  </div>

                  {/* 常用系列快捷过滤：当池子包含典型系列时提供一键聚焦 */}
                  {availableFamilies.length > 0 && (
                    <div className="flex items-center gap-1.5 px-3 py-1.5 border-b border-border/40 bg-zinc-500/[0.02] dark:bg-white/[0.02] overflow-x-auto text-[11px]">
                      {availableFamilies.map((fam) => {
                        const active = modelSearch.toLowerCase() === fam.key;
                        return (
                          <button
                            key={fam.key}
                            type="button"
                            onClick={() => setModelSearch(active ? '' : fam.key)}
                            className={`px-1.5 py-0.5 rounded-[4px] font-mono text-[10px] font-medium transition-colors cursor-pointer shrink-0 outline-none ${
                              active
                                ? 'bg-zinc-900 text-white dark:bg-zinc-100 dark:text-zinc-900 shadow-2xs'
                                : 'text-zinc-500 dark:text-zinc-400 hover:bg-zinc-200/60 dark:hover:bg-zinc-800 hover:text-zinc-900 dark:hover:text-zinc-100'
                            }`}
                          >
                            {fam.label} <span className="opacity-60 text-[9px]">{fam.count}</span>
                          </button>
                        );
                      })}
                    </div>
                  )}

                  {/* 模型列表：单列通栏列表，适配紧凑视口 */}
                  <div className="max-h-52 overflow-y-auto p-1.5">
                    {filteredModels.length === 0 ? (
                      <div className="py-7 text-center text-xs text-zinc-400 space-y-1.5">
                        {pool.length === 0 ? (
                          isSubscriptionPool ? (
                            <p>{t('providerEditor.noModelsConnectPrompt')}</p>
                          ) : (
                            <p>{t('providerEditor.noModelsEmptyPrompt')}</p>
                          )
                        ) : modelFilterTab === 'selected' ? (
                          <p>
                            {t('providerEditor.noModelsSelectedPrompt')}
                            <button
                              type="button"
                              onClick={() => setModelFilterTab('all')}
                              className="text-zinc-900 dark:text-zinc-100 underline hover:opacity-80 ml-1 cursor-pointer font-medium"
                            >
                              {t('providerEditor.goToSelectModels')}
                            </button>
                          </p>
                        ) : modelFilterTab === 'unselected' ? (
                          <p>{t('providerEditor.allModelsAlreadySelected')}</p>
                        ) : (
                          <div className="space-y-1">
                            <p>{t('providerEditor.noMatchingModels')}</p>
                            {modelSearch.trim() && (
                              <button
                                type="button"
                                onClick={() => setModelSearch('')}
                                className="text-[11px] text-zinc-500 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100 underline cursor-pointer"
                              >
                                {t('providerEditor.clearSearchKeyword')}
                              </button>
                            )}
                          </div>
                        )}
                      </div>
                    ) : (
                      <div className="space-y-0.5">
                        {filteredModels.map((model) => {
                          const isSelected = draft.models.includes(model);
                          const isInvalid = invalidModels.includes(model);
                          const cap = capabilities[model];
                          const isCopied = copiedModel === model;

                          return (
                            <div
                              key={model}
                              role="button"
                              tabIndex={0}
                              onClick={() => toggleModel(model)}
                              onKeyDown={(e) => {
                                if (e.key === ' ' || e.key === 'Enter') {
                                  e.preventDefault();
                                  toggleModel(model);
                                }
                              }}
                              aria-pressed={isSelected}
                              className={`group w-full flex items-center justify-between py-1.5 px-2 rounded-md text-xs font-mono transition-colors cursor-pointer select-none text-left ${
                                isSelected
                                  ? 'bg-zinc-100 dark:bg-zinc-800/80 text-zinc-900 dark:text-zinc-100 font-medium hover:bg-zinc-200/60 dark:hover:bg-zinc-700/60'
                                  : 'text-zinc-600 dark:text-zinc-400 hover:bg-zinc-200/50 dark:hover:bg-zinc-800/50 hover:text-zinc-900 dark:hover:text-zinc-200'
                              }`}
                            >
                              {/* 左侧：状态指示框与模型名称、紧随其后的复制按钮 */}
                              <div className="flex items-center gap-1.5 min-w-0 flex-1 mr-2">
                                <span
                                  className={`w-3.5 h-3.5 rounded-[3.5px] flex items-center justify-center shrink-0 transition-colors ${
                                    isSelected
                                      ? 'bg-zinc-900 text-white dark:bg-zinc-100 dark:text-zinc-900 group-hover:bg-zinc-800 dark:group-hover:bg-zinc-200'
                                      : 'border border-zinc-300 dark:border-zinc-700 bg-white/70 dark:bg-zinc-900/70 group-hover:border-zinc-400 dark:group-hover:border-zinc-500'
                                  }`}
                                >
                                  {isSelected && (
                                    <Check className="w-2.5 h-2.5 stroke-[3]" />
                                  )}
                                </span>
                                <span className="truncate">{cap?.displayName || model}</span>
                                <button
                                  type="button"
                                  onClick={(e) => handleCopyModel(e, model)}
                                  className={`${
                                    isCopied ? 'opacity-100 text-emerald-500' : 'opacity-0 group-hover:opacity-100 text-zinc-400 hover:text-zinc-700 dark:hover:text-zinc-200'
                                  } shrink-0 p-0.5 rounded hover:bg-zinc-200/80 dark:hover:bg-zinc-700/80 transition-all cursor-pointer`}
                                >
                                  {isCopied ? (
                                    <Check className="w-3 h-3" />
                                  ) : (
                                    <Copy className="w-3 h-3" />
                                  )}
                                </button>
                              </div>

                              {/* 右侧：标记与提示及操作 */}
                              <div className="flex items-center gap-1.5 shrink-0">
                                {isInvalid && (
                                  <span
                                    className="shrink-0 text-amber-500 flex items-center"
                                  >
                                    <AlertTriangle className="w-3 h-3" />
                                  </span>
                                )}
                                <button
                                  type="button"
                                  onClick={(e) => handleRemoveModelFromPool(e, model)}
                                  onKeyDown={(e) => e.stopPropagation()}
                                  className="opacity-0 group-hover:opacity-100 focus:opacity-100 text-zinc-400 hover:text-rose-600 dark:hover:text-rose-400 p-0.5 rounded hover:bg-rose-50 dark:hover:bg-rose-950/50 transition-all cursor-pointer"
                                  title={t('providerEditor.deleteModelFromPool')}
                                  aria-label={t('providerEditor.deleteModelFromPool')}
                                >
                                  <Trash2 className="w-3 h-3" />
                                </button>
                              </div>
                            </div>
                          );
                        })}
                      </div>
                    )}
                  </div>

                  {/* 底部紧凑手动补充条 */}
                  <div className="px-3 py-2 border-t border-border/60 flex items-center gap-2 bg-background/30">
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
                      placeholder={t('providerEditor.manualModelPlaceholder')}
                      className="font-mono h-7 text-xs bg-background/80"
                    />
                    <Button
                      type="button"
                      variant="secondary"
                      size="sm"
                      onClick={addManualModel}
                      disabled={!manualModel.trim()}
                    >
                      {t('providerEditor.addManualModel')}
                    </Button>
                  </div>
                </div>
                {errors.models && <p className="text-[11px] text-rose-500 mt-1">{errors.models}</p>}
              </div>



              {errors.save && (
                <div className="p-3 rounded-md bg-rose-50 dark:bg-rose-950/40 border border-rose-200 dark:border-rose-900/60 text-xs text-rose-700 dark:text-rose-300">
                  {t('providerEditor.saveFailed')}{errors.save}
                </div>
              )}

              {/* 表单自然底部操作区：单行自适应操作栏 */}
              <div className="flex items-center justify-between gap-2 pt-4 border-t border-border">
                <div>
                  {currentEditingProvider && (
                    <Button
                      type="button"
                      variant="destructive-ghost"
                      size="sm"
                      disabled={saving || deleting}
                      onClick={() => setConfirmingDelete(true)}
                      className="px-2 text-xs"
                    >
                      {t('providerEditor.deleteProviderBtn')}
                    </Button>
                  )}
                </div>

                <div className="flex items-center justify-end gap-2 shrink-0">
                  <span className="text-[11px] text-zinc-400 dark:text-zinc-500 truncate max-w-[140px]">
                    {fetching || syncingModels
                      ? t('providerEditor.syncingUpstreamNotice')
                      : isSubscriptionPool && subAccounts.length > 0
                        ? (
                          <span className="text-emerald-600 dark:text-emerald-400 font-medium flex items-center gap-1.5">
                            <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 inline-block shrink-0" />
                            <span className="truncate">{t('providerEditor.autoSavedNotice')}</span>
                          </span>
                        )
                        : !isSubscriptionPool && draft.models.length === 0
                          ? t('providerEditor.pleaseSelectModel')
                          : ''}
                  </span>

                  {isSubscriptionPool ? (
                    subAccounts.length > 0 ? (
                      <Button
                        type="button"
                        variant="default"
                        size="sm"
                        disabled={saving || syncingModels}
                        onClick={handleFinishSubscription}
                      >
                        {saving && (
                          <LoaderCircle className="w-3.5 h-3.5 animate-spin mr-1.5" />
                        )}
                        <span>{t('providerEditor.doneBtn')}</span>
                      </Button>
                    ) : (
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        onClick={handleBack}
                      >
                        {t('providerEditor.backBtn')}
                      </Button>
                    )
                  ) : (
                    <>
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        onClick={handleBack}
                      >
                        {t('providerEditor.backBtn')}
                      </Button>
                      <Button
                        type="submit"
                        variant="default"
                        size="sm"
                        disabled={saving || deleting || fetching || draft.models.length === 0}
                      >
                        {saving && (
                          <LoaderCircle className="w-3.5 h-3.5 animate-spin mr-1.5" />
                        )}
                        <span>{saving ? t('providerEditor.savingBtn') : t('providerEditor.saveBtn')}</span>
                      </Button>
                    </>
                  )}
                </div>
              </div>
            </form>
          )}
        </div>
      </div>

      <ConfirmDialog
        open={Boolean(confirmation)}
        title={
          confirmation?.kind === 'preset'
            ? (locale === 'zh-CN' ? '放弃已填内容并重选预设？' : 'Discard input and select preset?')
            : t('providerEditor.leaveConfirmTitle')
        }
        details={[
          confirmation?.kind === 'preset'
            ? (locale === 'zh-CN' ? '当前表单中已填写的数据将会丢失。' : 'The data filled in the current form will be lost.')
            : t('providerEditor.leaveConfirmDetail'),
        ]}
        confirmLabel={t('providerEditor.discard')}
        cancelLabel={t('providerEditor.continueEditing')}
        variant="danger"
        onConfirm={handleConfirmAbandon}
        onCancel={() => setConfirmation(null)}
      />

      {currentEditingProvider && (
        <ConfirmDialog
          open={confirmingDelete}
          title={t('providerEditor.deleteConfirmTitleWithName', { name: draft.name || currentEditingProvider.name })}
          details={[
            t('providerEditor.deleteConfirmDescReconfigure'),
          ]}
          confirmLabel={t('providerEditor.deleteAction')}
          busy={deleting}
          onConfirm={async () => {
            setDeleting(true);
            try {
              await deleteProvider(currentEditingProvider.id);
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
