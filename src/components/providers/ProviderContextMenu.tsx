import React, { useState, useRef, useEffect, useMemo } from 'react';
import { createPortal } from 'react-dom';
import { invoke } from '@tauri-apps/api/core';
import { useShallow } from 'zustand/react/shallow';
import { useAppStore } from '@/store/useAppStore';
import { ProviderAvatar } from '@/components/common/BrandIcons';
import type { Provider, SubscriptionAccount, AccountQuota, QuotaBucket } from '@/types';
import {
  Zap,
  Copy,
  Check,
  Settings,
  Trash2,
  Search,
  RefreshCw,
  LoaderCircle,
} from 'lucide-react';

interface ProviderContextMenuProps {
  x: number;
  y: number;
  provider: Provider;
  onClose: () => void;
  onEdit: () => void;
  onDelete: () => void;
}

function cleanBucketLabel(label: string): string {
  if (label.includes('Claude') && label.includes('5小时')) return 'Claude (5h)';
  if (label.includes('Claude') && (label.includes('周') || label.includes('week'))) return 'Claude (周)';
  if (label.includes('Gemini') && label.includes('5小时')) return 'Gemini (5h)';
  if (label.includes('Gemini') && (label.includes('周') || label.includes('week'))) return 'Gemini (周)';
  if (label.includes('Pro Lite') || label.includes('GPT')) {
    return label.replace('周期配额', '').replace('配额', '').trim();
  }
  return label.replace('周期配额', '').replace('配额', '').trim();
}

function formatShortResetTime(resetTimeStr?: string | null): string | null {
  if (!resetTimeStr) return null;
  const target = new Date(resetTimeStr).getTime();
  if (isNaN(target)) return null;
  const diffMs = target - Date.now();
  if (diffMs <= 0) return '即将';
  const totalMinutes = Math.floor(diffMs / 60000);
  if (totalMinutes < 60) return `${totalMinutes}m`;
  const totalHours = Math.floor(totalMinutes / 60);
  if (totalHours < 24) return `${totalHours}h`;
  const days = Math.floor(totalHours / 24);
  return `${days}d`;
}

function formatFullResetTime(resetTimeStr?: string | null): string {
  if (!resetTimeStr) return '';
  const target = new Date(resetTimeStr).getTime();
  if (isNaN(target)) return '';
  const diffMs = target - Date.now();
  if (diffMs <= 0) return '即将重置';
  const totalMinutes = Math.floor(diffMs / 60000);
  if (totalMinutes < 60) return `${totalMinutes}分钟后重置`;
  const totalHours = Math.floor(totalMinutes / 60);
  if (totalHours < 24) {
    const remMins = totalMinutes % 60;
    return remMins > 0 ? `${totalHours}小时${remMins}分钟后重置` : `${totalHours}小时后重置`;
  }
  const days = Math.floor(totalHours / 24);
  const remHours = totalHours % 24;
  return `${days}天${remHours}小时后重置`;
}

export const ProviderContextMenu: React.FC<ProviderContextMenuProps> = ({
  x,
  y,
  provider: initialProvider,
  onClose,
  onEdit,
  onDelete,
}) => {
  const { testProviderLatency, currentProvider } = useAppStore(
    useShallow((state) => ({
      testProviderLatency: state.testProviderLatency,
      currentProvider: state.providers.find((p) => p.id === initialProvider.id),
    })),
  );

  const provider = currentProvider ?? initialProvider;

  const menuRef = useRef<HTMLDivElement>(null);
  const [coords, setCoords] = useState({ x, y });
  const [modelSearch, setModelSearch] = useState('');
  const [copiedUrl, setCopiedUrl] = useState(false);
  const [copiedModel, setCopiedModel] = useState<string | null>(null);
  const [testingLatency, setTestingLatency] = useState(false);
  const [capabilities, setCapabilities] = useState<Record<string, { displayName?: string }>>({});

  // 加载模型显示名称
  useEffect(() => {
    if (provider.models.length === 0) return;
    let cancelled = false;
    invoke<Record<string, { displayName?: string }>>('get_model_capabilities', {
      modelNames: provider.models,
    })
      .then((caps) => {
        if (!cancelled) setCapabilities(caps);
      })
      .catch(() => {
        if (!cancelled) setCapabilities({});
      });
    return () => {
      cancelled = true;
    };
  }, [provider.models]);

  // 订阅账号与配额状态
  const isSubscription =
    provider.authType === 'subscription_pool' ||
    provider.presetId === 'antigravity' ||
    provider.presetId === 'chatgpt_subscription' ||
    provider.protocol === 'chatgpt';

  const [subAccounts, setSubAccounts] = useState<SubscriptionAccount[]>([]);
  const [accountQuotas, setAccountQuotas] = useState<Record<string, AccountQuota>>({});
  const [selectedAccountId, setSelectedAccountId] = useState<string | null>(null);
  const [loadingQuota, setLoadingQuota] = useState(false);

  // 视口边缘自适应贴边与动态尺寸监听（防遮挡防溢出）
  useEffect(() => {
    const el = menuRef.current;
    if (!el) return;

    const updatePosition = () => {
      const rect = el.getBoundingClientRect();
      const padding = 12;
      let nextX = x;
      let nextY = y;

      if (x + rect.width > window.innerWidth - padding) {
        nextX = Math.max(padding, window.innerWidth - rect.width - padding);
      }
      if (y + rect.height > window.innerHeight - padding) {
        nextY = Math.max(padding, window.innerHeight - rect.height - padding);
      }

      setCoords({ x: nextX, y: nextY });
    };

    updatePosition();

    // 关键：当内容异步加载更新导致高度变动时，即刻重新吸附
    const observer = new ResizeObserver(() => {
      updatePosition();
    });
    observer.observe(el);

    return () => observer.disconnect();
  }, [x, y]);

  // 监听键盘 Esc 关闭
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [onClose]);

  // 订阅账号配额拉取
  const fetchSubscriptionQuotas = async (showLoading = true) => {
    if (!isSubscription) return;
    if (showLoading) setLoadingQuota(true);
    try {
      const accounts = await invoke<SubscriptionAccount[]>('get_subscription_accounts', {
        providerId: provider.id,
      });
      setSubAccounts(accounts);
      if (accounts.length > 0 && !selectedAccountId) {
        setSelectedAccountId(accounts[0].id);
      }

      const quotaResults = await Promise.all(
        accounts
          .filter((a) => a.enabled)
          .map(async (acc) => {
            try {
              const q = await invoke<AccountQuota>('get_subscription_account_quota', {
                providerId: provider.id,
                accountId: acc.id,
                baseUrl: provider.baseUrl || undefined,
              });
              return [acc.id, q] as const;
            } catch {
              return [acc.id, null] as const;
            }
          }),
      );

      const quotaMap: Record<string, AccountQuota> = {};
      for (const [id, q] of quotaResults) {
        if (q) quotaMap[id] = q;
      }
      setAccountQuotas(quotaMap);
    } catch (e) {
      console.warn('获取订阅账号额度失败', e);
    } finally {
      if (showLoading) setLoadingQuota(false);
    }
  };

  useEffect(() => {
    if (isSubscription) {
      void fetchSubscriptionQuotas(true);
    }
  }, [provider.id, isSubscription]);

  // 复制 Base URL
  const handleCopyBaseUrl = () => {
    if (!provider.baseUrl) return;
    navigator.clipboard.writeText(provider.baseUrl);
    setCopiedUrl(true);
    setTimeout(() => {
      setCopiedUrl(false);
      onClose();
    }, 800);
  };

  // 测速
  const handleTestLatency = async () => {
    if (testingLatency || !provider.enabled) return;
    setTestingLatency(true);
    try {
      await testProviderLatency(provider.id);
    } finally {
      setTestingLatency(false);
      onClose();
    }
  };

  // 复制模型 ID（用于 API 请求配置）
  const handleCopyModel = (e: React.MouseEvent, modelId: string) => {
    e.stopPropagation();
    navigator.clipboard.writeText(modelId);
    setCopiedModel(modelId);
    setTimeout(() => {
      setCopiedModel((prev) => (prev === modelId ? null : prev));
    }, 1200);
  };

  const filteredModels = useMemo(() => {
    const q = modelSearch.trim().toLowerCase();
    if (!q) return provider.models;
    return provider.models.filter((m) => {
      const matchId = m.toLowerCase().includes(q);
      const displayName = capabilities[m]?.displayName;
      const matchName = displayName ? displayName.toLowerCase().includes(q) : false;
      return matchId || matchName;
    });
  }, [provider.models, modelSearch, capabilities]);

  // 当前选中的订阅账号与对应配额
  const activeAccount = useMemo(() => {
    if (subAccounts.length === 0) return null;
    return subAccounts.find((a) => a.id === selectedAccountId) || subAccounts[0];
  }, [subAccounts, selectedAccountId]);

  const activeQuota = activeAccount ? accountQuotas[activeAccount.id] : null;

  return createPortal(
    <div
      className="fixed inset-0 z-50 overflow-hidden"
      onClick={onClose}
      onContextMenu={(e) => {
        e.preventDefault();
        onClose();
      }}
    >
      <div
        ref={menuRef}
        onClick={(e) => e.stopPropagation()}
        style={{ left: `${coords.x}px`, top: `${coords.y}px` }}
        className="fixed z-50 w-72 max-w-[92vw] max-h-[calc(100vh-24px)] overflow-y-auto no-scrollbar rounded-lg border border-border bg-popover/95 backdrop-blur-md shadow-xl text-popover-foreground text-xs p-1 select-none animate-in fade-in-0 zoom-in-95 duration-100"
      >
        {/* 顶部服务商信息标题 */}
        <div className="flex items-center gap-2 px-2.5 py-1.5 border-b border-border/60">
          <ProviderAvatar
            name={provider.name}
            protocol={provider.protocol}
            iconKey={provider.presetId || undefined}
            sizeClass="w-4 h-4"
            dense
          />
          <div className="min-w-0 flex-1">
            <div className="font-semibold text-foreground truncate text-[12px] flex items-center gap-1.5">
              <span>{provider.name}</span>
              {isSubscription && (
                <span className="shrink-0 inline-flex items-center justify-center h-4.5 px-1.5 rounded-[4px] text-[10px] font-medium tracking-tight bg-indigo-500/[0.08] text-indigo-600 border border-indigo-500/20 dark:bg-indigo-400/[0.12] dark:text-indigo-300 dark:border-indigo-400/25 select-none">
                  订阅
                </span>
              )}
            </div>
          </div>
        </div>

        {/* 订阅配额数据展示区（全新紧凑微面板结构） */}
        {isSubscription && (
          <div className="py-1.5 px-1 border-b border-border/60 space-y-1.5">
            {/* 顶栏：标签 + 多账号分段器 + 刷新按钮 */}
            <div className="flex items-center justify-between px-1.5 text-[11px]">
              <div className="flex items-center gap-1.5 min-w-0">
                <span className="font-medium text-muted-foreground shrink-0">订阅配额</span>
                {/* 多账号时呈现水平切换药丸 */}
                {subAccounts.length > 1 && (
                  <div className="flex items-center gap-1 overflow-x-auto no-scrollbar py-0.5">
                    {subAccounts.map((acc, idx) => {
                      const isSelected = activeAccount?.id === acc.id;
                      const isCooldown = acc.cooldownUntil > Date.now() / 1000;
                      const prefix = acc.email ? acc.email.split('@')[0] : `账号${idx + 1}`;

                      return (
                        <button
                          key={acc.id}
                          type="button"
                          onClick={() => setSelectedAccountId(acc.id)}
                          title={acc.email || acc.id}
                          className={`px-1.5 py-0.2 rounded text-[10px] font-mono shrink-0 transition-colors cursor-pointer ${
                            isSelected
                              ? 'bg-zinc-200 dark:bg-zinc-800 text-foreground font-semibold shadow-2xs'
                              : 'text-muted-foreground hover:text-foreground hover:bg-muted/50'
                          }`}
                        >
                          <span>{prefix}</span>
                          {isCooldown && <span className="text-amber-500 ml-0.5">●</span>}
                        </button>
                      );
                    })}
                  </div>
                )}
              </div>

              <button
                type="button"
                onClick={() => void fetchSubscriptionQuotas(true)}
                disabled={loadingQuota}
                title="刷新配额"
                className="text-zinc-400 hover:text-foreground transition-colors p-0.5 rounded cursor-pointer disabled:opacity-50 shrink-0"
              >
                <RefreshCw
                  className={`w-3 h-3 ${loadingQuota ? 'animate-spin text-primary' : ''}`}
                />
              </button>
            </div>

            {/* 账号活跃与冷却状态 */}
            {activeAccount && (
              <div className="flex items-center justify-between px-1.5 text-[10px] text-muted-foreground">
                <span className="font-mono truncate max-w-[170px]" title={activeAccount.email || activeAccount.id}>
                  {activeAccount.email || `账号 ${activeAccount.id.slice(0, 8)}`}
                </span>
                {activeAccount.cooldownUntil > Date.now() / 1000 ? (
                  <span className="text-amber-600 dark:text-amber-400 font-medium shrink-0">
                    429 冷却中 ({Math.ceil((activeAccount.cooldownUntil - Date.now() / 1000) / 60)}m)
                  </span>
                ) : !activeAccount.enabled ? (
                  <span className="text-zinc-400 dark:text-zinc-500 shrink-0">已停用</span>
                ) : (
                  <span className="text-emerald-600 dark:text-emerald-400 flex items-center gap-1 shrink-0 font-medium">
                    <span className="w-1.5 h-1.5 rounded-full bg-emerald-500" />
                    <span>正常</span>
                  </span>
                )}
              </div>
            )}

            {/* 配额核心：双列紧凑指标网格 (2-Column Metric Grid) */}
            {loadingQuota && subAccounts.length === 0 ? (
              <div className="py-2.5 flex items-center justify-center gap-1.5 text-[11px] text-muted-foreground">
                <LoaderCircle className="w-3 h-3 animate-spin text-zinc-400" />
                <span>查询配额中…</span>
              </div>
            ) : subAccounts.length === 0 ? (
              <div className="py-1 px-2 text-center text-[10px] text-muted-foreground/80">
                尚未绑定订阅账号
              </div>
            ) : activeQuota && activeQuota.buckets.length > 0 ? (
              <div
                className={`grid gap-1 px-0.5 ${
                  activeQuota.buckets.length === 1 ? 'grid-cols-1' : 'grid-cols-2'
                }`}
              >
                {activeQuota.buckets.map((b: QuotaBucket) => {
                  const pct = Math.round(b.remainingFraction * 100);
                  const isLow = b.remainingFraction <= 0.2;
                  const isOut = b.isExhausted;
                  const shortReset = formatShortResetTime(b.resetTime);
                  const fullReset = formatFullResetTime(b.resetTime);
                  const title = cleanBucketLabel(b.label);

                  return (
                    <div
                      key={b.id}
                      title={`${b.label}: ${isOut ? '已耗尽' : `${pct}%`}${fullReset ? ` (${fullReset})` : ''}`}
                      className="p-1.5 rounded-md bg-muted/40 border border-border/40 flex flex-col justify-between min-h-[38px] transition-colors"
                    >
                      <div className="text-[10px] text-muted-foreground/90 font-sans truncate">
                        {title}
                      </div>
                      <div className="flex items-baseline justify-between gap-1 mt-0.5">
                        <span
                          className={`font-mono text-[11px] font-semibold leading-none ${
                            isOut
                              ? 'text-rose-500'
                              : isLow
                                ? 'text-amber-500'
                                : 'text-foreground'
                          }`}
                        >
                          {isOut ? '耗尽' : `${pct}%`}
                        </span>
                        {shortReset && (
                          <span className="font-mono text-[9px] text-zinc-400 dark:text-zinc-500 shrink-0 leading-none">
                            {shortReset}
                          </span>
                        )}
                      </div>
                    </div>
                  );
                })}
              </div>
            ) : loadingQuota ? (
              <div className="py-1 text-[10px] text-muted-foreground text-center">
                更新配额中…
              </div>
            ) : null}
          </div>
        )}

        {/* 模型列表区 */}
        <div className="py-1">
          <div className="flex items-center justify-between px-2.5 py-1 text-[11px] font-medium text-muted-foreground">
            <span>可用模型 ({provider.models.length})</span>
          </div>

          {/* 搜索框：仅在模型较多时展示 */}
          {provider.models.length > 5 && (
            <div className="px-2 pb-1">
              <div className="flex items-center gap-1.5 px-2 py-1 rounded bg-muted/50 border border-border/50 text-[11px]">
                <Search className="w-3 h-3 text-muted-foreground shrink-0" />
                <input
                  type="text"
                  value={modelSearch}
                  onChange={(e) => setModelSearch(e.target.value)}
                  placeholder="过滤模型..."
                  className="w-full bg-transparent outline-none placeholder:text-muted-foreground/60 text-foreground font-mono"
                  onClick={(e) => e.stopPropagation()}
                />
              </div>
            </div>
          )}

          {/* 滚动模型列表 */}
          <div className="max-h-36 overflow-y-auto no-scrollbar space-y-0.5 px-1">
            {filteredModels.length === 0 ? (
              <div className="px-2 py-2 text-center text-muted-foreground/70 text-[11px]">
                {provider.models.length === 0 ? '未配置任何模型' : '无匹配模型'}
              </div>
            ) : (
              filteredModels.map((model) => {
                const cap = capabilities[model];
                const modelName = cap?.displayName || model;
                const isCopied = copiedModel === model;

                return (
                  <div
                    key={model}
                    className="group flex items-center justify-between gap-1.5 px-2 py-1 rounded-md hover:bg-accent/60 transition-colors text-[11px]"
                  >
                    {/* 左侧：模型名称及紧随其后的复制按钮 */}
                    <div className="flex items-center gap-1 min-w-0 flex-1">
                      <span className="text-foreground/90 select-text cursor-text truncate font-medium">
                        {modelName}
                      </span>
                      <button
                        type="button"
                        onClick={(e) => handleCopyModel(e, model)}
                        className={`${
                          isCopied ? 'opacity-100 text-emerald-500' : 'opacity-0 group-hover:opacity-100 text-muted-foreground hover:text-foreground'
                        } shrink-0 p-0.5 rounded hover:bg-muted/80 transition-all cursor-pointer`}
                      >
                        {isCopied ? (
                          <Check className="w-3 h-3" />
                        ) : (
                          <Copy className="w-3 h-3" />
                        )}
                      </button>
                    </div>
                  </div>
                );
              })
            )}
          </div>
        </div>

        {/* 分割线 */}
        <div className="my-1 border-t border-border/60" />

        {/* 服务商管理快捷操作区 */}
        <div className="space-y-0.5">
          {/* 测试延迟 */}
          <button
            type="button"
            disabled={testingLatency || !provider.enabled}
            onClick={handleTestLatency}
            className="w-full flex items-center justify-between px-2.5 py-1.5 rounded-md hover:bg-accent hover:text-accent-foreground text-left transition-colors cursor-pointer disabled:opacity-40 disabled:pointer-events-none"
          >
            <div className="flex items-center gap-2">
              <Zap
                className={`w-3.5 h-3.5 ${testingLatency ? 'animate-pulse text-amber-500' : 'text-zinc-500'}`}
              />
              <span>{testingLatency ? '正在测试延迟…' : '测试端点延迟'}</span>
            </div>
            {provider.latencyMs && provider.latencyMs > 0 && (
              <span className="font-mono text-[10px] text-zinc-400">{provider.latencyMs}ms</span>
            )}
          </button>

          {/* 复制 Base URL（仅非订阅服务商） */}
          {!isSubscription && provider.baseUrl && (
            <button
              type="button"
              onClick={handleCopyBaseUrl}
              className="w-full flex items-center justify-between px-2.5 py-1.5 rounded-md hover:bg-accent hover:text-accent-foreground text-left transition-colors cursor-pointer"
            >
              <div className="flex items-center gap-2">
                {copiedUrl ? (
                  <Check className="w-3.5 h-3.5 text-emerald-500" />
                ) : (
                  <Copy className="w-3.5 h-3.5 text-zinc-500" />
                )}
                <span>{copiedUrl ? '已复制接口地址' : '复制 Base URL'}</span>
              </div>
            </button>
          )}

          {/* 编辑与删除 */}
          <button
            type="button"
            onClick={() => {
              onClose();
              onEdit();
            }}
            className="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-md hover:bg-accent hover:text-accent-foreground text-left transition-colors cursor-pointer"
          >
            <Settings className="w-3.5 h-3.5 text-zinc-500" />
            <span>编辑服务商配置...</span>
          </button>

          <button
            type="button"
            onClick={() => {
              onClose();
              onDelete();
            }}
            className="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-md hover:bg-destructive/10 text-destructive text-left transition-colors cursor-pointer"
          >
            <Trash2 className="w-3.5 h-3.5" />
            <span>删除服务商</span>
          </button>
        </div>
      </div>
    </div>,
    document.body,
  );
};