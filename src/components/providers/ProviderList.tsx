import { useState, useRef, useEffect, useMemo } from 'react';
import { createPortal } from 'react-dom';
import { useShallow } from 'zustand/react/shallow';
import { useAppStore } from '@/store/useAppStore';
import { useTranslation } from '@/i18n';
import { panelClass } from '@/components/common/panelStyles';
import { Provider, InterceptorStatus } from '@/types';
import { Switch } from '@/components/common/Switch';
import { Button } from '@/components/common/Button';
import { Select } from '@/components/common/Select';
import { ConfirmDialog } from '@/components/common/ConfirmDialog';
import { AgentConflictDialog } from '@/components/interceptor/AgentConflictDialog';
import { ProviderContextMenu } from './ProviderContextMenu';
import {
  Plus,
  LoaderCircle,
  Copy,
  Check,
  Code2,
  ChevronDown,
  GripVertical,
  Zap,
} from 'lucide-react';

import {
  ICON_SLOT,
  ProviderAvatar,
  OpenAISpiralIcon,
  ClaudeLogo,
  GeminiLogo,
  PiLogo,
} from '@/components/common/BrandIcons';

const CLIENT_LABEL: Record<InterceptorStatus['id'], string> = {
  claude_code: 'Claude Code',
  codex_cli: 'Codex CLI',
  gemini_cli: 'Gemini CLI',
  pi: 'Pi Agent',
};

function ClientMark({ id }: { id: InterceptorStatus['id'] }) {
  if (id === 'codex_cli') return <OpenAISpiralIcon className="w-full h-full" />;
  if (id === 'gemini_cli') return <GeminiLogo className="w-full h-full" />;
  if (id === 'pi') return <PiLogo className="w-full h-full" />;
  return <ClaudeLogo className="w-full h-full" />;
}

interface PressState {
  id: string;
  row: HTMLDivElement;
  startX: number;
  startY: number;
  grabOffset: number;
  rowLeft: number;
  rowWidth: number;
  listTop: number;
  rowHeight: number;
}

interface DragState extends PressState {
  pointerY: number;
  previewIds: string[];
}

interface ProviderCardProps {
  provider: Provider;
  isDragged: boolean;
  onPointerDown: (event: React.PointerEvent<HTMLDivElement>) => void;
  onRowClick: () => void;
  onContextMenu: (event: React.MouseEvent) => void;
  shouldSuppressClick: () => boolean;
}

function ProviderCard({
  provider,
  isDragged,
  onPointerDown,
  onRowClick,
  onContextMenu,
  shouldSuppressClick,
}: ProviderCardProps) {
  const { t } = useTranslation();
  const { openProviderEditor, toggleProvider, testProviderLatency } = useAppStore(
    useShallow((state) => ({
      openProviderEditor: state.openProviderEditor,
      toggleProvider: state.toggleProvider,
      testProviderLatency: state.testProviderLatency,
    })),
  );

  const [toggling, setToggling] = useState(false);
  const [testing, setTesting] = useState(false);
  const [copied, setCopied] = useState(false);

  const isSubscription =
    provider.authType === 'subscription_pool' ||
    provider.presetId === 'antigravity' ||
    provider.presetId === 'chatgpt_subscription' ||
    provider.protocol === 'chatgpt';

  const handleToggle = async () => {
    if (toggling) return;
    setToggling(true);
    try {
      await toggleProvider(provider.id, !provider.enabled);
    } finally {
      setToggling(false);
    }
  };

  const handleTestLatency = async (e: React.MouseEvent) => {
    e.stopPropagation();
    if (testing || !provider.enabled) return;
    setTesting(true);
    try {
      await testProviderLatency(provider.id);
    } catch {
      // testProviderLatency sets latencyMs: -1 on failure
    } finally {
      setTesting(false);
    }
  };

  const handleCopyBaseUrl = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (!provider.baseUrl) return;
    navigator.clipboard.writeText(provider.baseUrl);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  const displayEndpoint = provider.baseUrl
    ? provider.baseUrl.replace(/^https?:\/\//, '').replace(/\/v1\/?$/, '').replace(/\/+$/, '')
    : '';

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onRowClick}
      onContextMenu={onContextMenu}
      onClickCapture={(event) => {
        if (!shouldSuppressClick()) return;
        event.stopPropagation();
        event.preventDefault();
      }}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          openProviderEditor(provider);
        }
      }}
      onPointerDown={onPointerDown}
      className={`group relative flex items-center justify-between gap-3 px-4 py-2.5 transition-colors cursor-pointer select-none ${
        isDragged
          ? 'bg-zinc-100/70 dark:bg-zinc-800/40'
          : provider.enabled
            ? 'hover:bg-accent/50'
            : 'opacity-55 bg-zinc-50/20 dark:bg-zinc-900/20 hover:bg-zinc-100/40 dark:hover:bg-zinc-800/30'
      }`}
      aria-label={t('providers.editProviderAria', { name: provider.name })}
    >
      {isDragged ? (
        <div className="h-6 w-full" aria-hidden />
      ) : (
        <>
          <GripVertical
            aria-hidden
            className="pointer-events-none absolute left-0.5 top-1/2 h-3 w-3 -translate-y-1/2 text-zinc-400 opacity-0 transition-opacity group-hover:opacity-70 dark:text-zinc-500"
          />

          {/* 左侧：Logo + 服务商名称 + 标签 + 简化端点 */}
          <div className="flex items-center gap-2.5 min-w-0 flex-1">
            <ProviderAvatar
              name={provider.name}
              protocol={provider.protocol}
              iconKey={provider.presetId || undefined}
              sizeClass="w-6 h-6"
              dense
            />

            <div className="flex items-center gap-2 min-w-0 flex-1">
              <h3 className="shrink-0 text-[13px] font-semibold text-zinc-900 dark:text-zinc-100 truncate group-hover:text-zinc-950 dark:group-hover:text-white transition-colors">
                {provider.name}
              </h3>

              {/* 订阅池标记 */}
              {isSubscription && (
                <span
                  title={t('providers.subscriptionTitle')}
                  className="shrink-0 inline-flex items-center justify-center h-4.5 px-1.5 rounded-[4px] text-[10px] font-medium tracking-tight bg-indigo-500/[0.08] text-indigo-600 border border-indigo-500/20 dark:bg-indigo-400/[0.12] dark:text-indigo-300 dark:border-indigo-400/25 select-none"
                >
                  {t('providers.subscription')}
                </span>
              )}

              {/* 紧凑端点信息（非订阅服务商展示，空间宽裕时完整展示，空间受限时自适应截断） */}
              {!isSubscription && displayEndpoint && (
                <span
                  className="min-w-0 truncate font-mono text-[11px] text-zinc-400 dark:text-zinc-500"
                  title={provider.baseUrl}
                >
                  {displayEndpoint}
                </span>
              )}
            </div>
          </div>

          {/* 右侧：Hover 快捷操作栏 + 延迟药丸（与测速一体） + Switch 开关 */}
          <div className="shrink-0 flex items-center gap-1.5" onClick={(e) => e.stopPropagation()}>
            {/* Hover 时平滑浮现的快捷操作：复制 Base URL（仅非订阅服务商） */}
            {!isSubscription && provider.baseUrl && (
              <button
                type="button"
                onClick={handleCopyBaseUrl}
                title={copied ? t('providers.copiedBaseUrl') : t('providers.copyBaseUrl')}
                className="opacity-0 group-hover:opacity-100 p-1 rounded-md text-muted-foreground hover:text-foreground hover:bg-muted/80 transition-all cursor-pointer"
              >
                {copied ? (
                  <Check className="w-3.5 h-3.5 text-emerald-500" />
                ) : (
                  <Copy className="w-3.5 h-3.5" />
                )}
              </button>
            )}

            {/* 延迟测试药丸（仅在服务商启用时展示，停用时彻底收起不占位） */}
            {!provider.enabled ? null : testing ? (
              <div className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[11px] font-mono text-zinc-400 dark:text-zinc-500 bg-muted/50 select-none">
                <LoaderCircle className="w-3 h-3 animate-spin text-zinc-400" />
                <span>{t('providers.testingSpeed')}</span>
              </div>
            ) : provider.latencyMs && provider.latencyMs > 0 ? (
              <button
                type="button"
                onClick={handleTestLatency}
                title={t('providers.latencyTitle', { ms: provider.latencyMs })}
                className={`inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[11px] font-mono transition-colors cursor-pointer select-none ${
                  provider.latencyMs < 300
                    ? 'text-emerald-600 dark:text-emerald-400 bg-emerald-500/[0.08] hover:bg-emerald-500/[0.15]'
                    : provider.latencyMs < 1000
                      ? 'text-amber-600 dark:text-amber-400 bg-amber-500/[0.08] hover:bg-amber-500/[0.15]'
                      : 'text-orange-600 dark:text-orange-400 bg-orange-500/[0.08] hover:bg-orange-500/[0.15]'
                }`}
              >
                <span
                  className={`w-1.5 h-1.5 rounded-full ${
                    provider.latencyMs < 300
                      ? 'bg-emerald-500'
                      : provider.latencyMs < 1000
                        ? 'bg-amber-500'
                        : 'bg-orange-500'
                  }`}
                />
                <span>{provider.latencyMs}ms</span>
              </button>
            ) : provider.latencyMs === -1 ? (
              <button
                type="button"
                onClick={handleTestLatency}
                title={t('providers.connectionFailedTitle')}
                className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[11px] font-mono text-rose-600 dark:text-rose-400 bg-rose-500/[0.08] hover:bg-rose-500/[0.15] transition-colors cursor-pointer select-none"
              >
                <span className="w-1.5 h-1.5 rounded-full bg-rose-500" />
                <span>{t('providers.connectionFailed')}</span>
              </button>
            ) : (
              /* 未测速时：仅在 group-hover 时展示轻量的 ⚡ 测速按钮，常态下完全不占位！ */
              <button
                type="button"
                onClick={handleTestLatency}
                title={t('providers.testLatency')}
                className="opacity-0 group-hover:opacity-100 p-1 rounded-md text-muted-foreground hover:text-foreground hover:bg-muted/80 transition-all cursor-pointer"
              >
                <Zap className="w-3.5 h-3.5" />
              </button>
            )}

            {/* 启用/停用开关 */}
            <Switch
              checked={provider.enabled}
              disabled={toggling}
              onCheckedChange={handleToggle}
              ariaLabel={t(provider.enabled ? 'providers.disableRoutingAria' : 'providers.enableRoutingAria', { name: provider.name })}
            />
          </div>
        </>
      )}
    </div>
  );
}

function IngressCard() {
  const { t } = useTranslation();
  const { interceptors, toggleInterceptor, setInterceptorModel, providers, stats, initialLoaded } = useAppStore(
    useShallow((state) => ({
      interceptors: state.interceptors,
      toggleInterceptor: state.toggleInterceptor,
      setInterceptorModel: state.setInterceptorModel,
      providers: state.providers,
      stats: state.stats,
      initialLoaded: state.initialLoaded,
    })),
  );
  const [togglingId, setTogglingId] = useState<string | null>(null);
  const [conflictConfirmTarget, setConflictConfirmTarget] = useState<InterceptorStatus | null>(null);

  const hasEnabledProviders = providers.some((p) => p.enabled);
  const gatewayUp = stats.isRunning;

  const globalDefaultModel = useMemo(() => {
    const p = providers.find((p) => p.enabled && p.defaultModel);
    return p?.defaultModel ?? null;
  }, [providers]);

  const availableModels = useMemo(() => {
    const list: { model: string; providerName: string }[] = [];
    const seen = new Set<string>();
    for (const provider of providers) {
      if (!provider.enabled) continue;
      for (const m of provider.models) {
        if (!seen.has(m)) {
          seen.add(m);
          list.push({ model: m, providerName: provider.name });
        }
      }
    }
    return list;
  }, [providers]);

  const doToggle = async (id: InterceptorStatus['id'], enable: boolean) => {
    if (togglingId) return;
    setTogglingId(id);
    try {
      await toggleInterceptor(id, enable);
    } catch {
      // 全局错误条展示失败原因
    } finally {
      setTogglingId(null);
    }
  };

  const handleSwitch = (client: InterceptorStatus, enable: boolean) => {
    if (togglingId) return;
    if (enable && client.conflict) {
      setConflictConfirmTarget(client);
      return;
    }
    void doToggle(client.id, enable);
  };

  const notice = ((): { text: string; tone: 'fault' | 'warn' | 'hint' } | null => {
    if (!initialLoaded) return null;
    if (!gatewayUp) return { text: t('clients.portNotListening', { port: stats.port }), tone: 'fault' };
    const takenOver = interceptors.some((client) => client.isIntercepted);
    if (takenOver && !hasEnabledProviders) {
      return { text: t('clients.noUpstream'), tone: 'warn' };
    }
    const installed = interceptors.some((client) => client.installed);
    if (hasEnabledProviders && installed && !takenOver) {
      return { text: t('clients.notTakenOver'), tone: 'hint' };
    }
    return null;
  })();

  return (
    <>
      <section className={panelClass} aria-label={t('clients.title')}>
        {interceptors.length === 0 ? (
          <p className="px-4 py-6 text-center text-xs text-zinc-400">{t('clients.empty')}</p>
        ) : (
          <div className="divide-y divide-border">
            {interceptors.map((client) => {
              const label = CLIENT_LABEL[client.id] ?? client.name;
              const busy = togglingId === client.id;

              return (
                <div
                  key={client.id}
                  className={`flex items-center justify-between gap-3 px-4 py-2.5 ${!client.installed ? 'opacity-60' : ''}`}
                  title={client.configPath ?? undefined}
                >
                  <div className="flex items-center gap-2.5 min-w-0">
                    <div className={`w-6 h-6 shrink-0 flex items-center justify-center p-1 ${ICON_SLOT}`}>
                      <ClientMark id={client.id} />
                    </div>
                    <h3 className="shrink-0 text-[13px] font-semibold text-zinc-900 dark:text-zinc-100">
                      {label}
                    </h3>
                  </div>

                  <div className="shrink-0 flex items-center gap-2.5">
                    {client.installed ? (
                      <>
                        <Select
                          value={client.model ?? ''}
                          disabled={availableModels.length === 0}
                          onChange={(e) => {
                            const next = e.target.value || null;
                            void setInterceptorModel(client.id, next);
                          }}
                          containerClassName="w-32 sm:w-44 shrink-0"
                          className="h-7 text-[11px] font-mono py-0.5 pl-2 pr-6"
                          aria-label={`${label} ${t('clients.startupModel')}`}
                        >
                          <option value="">
                            {globalDefaultModel
                              ? `${t('clients.defaultModelRecommend')} (${globalDefaultModel})`
                              : t('clients.defaultModelRecommend')}
                          </option>
                          {client.model && !availableModels.some((o) => o.model === client.model) && (
                            <option value={client.model}>{client.model}</option>
                          )}
                          {availableModels.map(({ model, providerName }) => (
                            <option key={`${providerName}:${model}`} value={model}>
                              {model} ({providerName})
                            </option>
                          ))}
                        </Select>

                        {busy ? (
                          <LoaderCircle className="w-4 h-4 animate-spin text-zinc-400" />
                        ) : client.isOverwritten ? (
                          <button
                            type="button"
                            onClick={() => setConflictConfirmTarget(client)}
                            className="px-2 py-0.5 rounded text-[11px] font-medium text-amber-700 dark:text-amber-300 bg-amber-50 dark:bg-amber-950/40 border border-amber-200/80 dark:border-amber-900/50 cursor-pointer"
                          >
                            {t('clients.resolveConflict')}
                          </button>
                        ) : (
                          <Switch
                            checked={client.isIntercepted}
                            disabled={togglingId !== null}
                            onCheckedChange={(checked) => handleSwitch(client, checked)}
                            ariaLabel={t(client.isIntercepted ? 'clients.restoreClientAria' : 'clients.interceptClientAria', { name: label })}
                          />
                        )}
                      </>
                    ) : (
                      <span className="text-[11px] text-zinc-400 dark:text-zinc-500 py-1">
                        {t('clients.statusNotInstalled')}
                      </span>
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        )}

      </section>

      {/* 列表底部说明文字 */}
      <p className="text-[11px] text-zinc-400 dark:text-zinc-500 px-1 leading-relaxed">
        {notice ? (
          <span
            className={
              notice.tone === 'fault'
                ? 'text-destructive font-medium'
                : notice.tone === 'warn'
                  ? 'text-amber-600 dark:text-amber-400'
                  : ''
            }
          >
            {notice.text}
          </span>
        ) : (
          <span>{t('clients.defaultFooter')}</span>
        )}
      </p>

      <AgentConflictDialog
        open={conflictConfirmTarget !== null}
        target={conflictConfirmTarget}
        busy={togglingId !== null}
        hasEnabledProviders={hasEnabledProviders}
        onClose={() => setConflictConfirmTarget(null)}
        onConfirmTakeover={async (target) => {
          setConflictConfirmTarget(null);
          await doToggle(target.id, true);
        }}
      />
    </>
  );
}

function LoadingState() {
  const { t } = useTranslation();
  return (
    <div className="p-12 text-center">
      <LoaderCircle className="w-6 h-6 text-zinc-600 dark:text-zinc-400 animate-spin mx-auto mb-2" />
      <p className="text-xs text-zinc-400">{t('providers.loadingConfig')}</p>
    </div>
  );
}

function UniversalClientCard() {
  const { t } = useTranslation();
  const stats = useAppStore((state) => state.stats);
  const [copiedType, setCopiedType] = useState<'openai' | 'anthropic' | null>(null);
  const [expanded, setExpanded] = useState(false);

  const openaiUrl = `http://${stats.host}:${stats.port}/v1`;
  const anthropicUrl = `http://${stats.host}:${stats.port}`;

  const copyUrl = (type: 'openai' | 'anthropic', url: string) => {
    navigator.clipboard.writeText(url);
    setCopiedType(type);
    setTimeout(() => setCopiedType(null), 2000);
  };

  return (
    <section className={panelClass} aria-label={t('clients.universalCard.title')}>
      <button
        type="button"
        onClick={() => setExpanded((prev) => !prev)}
        aria-expanded={expanded}
        className={`w-full flex items-center justify-between px-4 py-2.5 text-xs cursor-pointer transition-colors hover:bg-accent/40 ${
          expanded ? 'bg-muted/30 border-b border-border' : ''
        }`}
      >
        <div className="flex items-center gap-2">
          <Code2 className="w-3.5 h-3.5 text-zinc-500" />
          <span className="font-semibold text-zinc-700 dark:text-zinc-200 tracking-wide">
            {t('clients.universalCard.title')}
          </span>
        </div>
        <ChevronDown
          className={`w-3.5 h-3.5 text-zinc-400 transition-transform ${expanded ? 'rotate-180' : ''}`}
        />
      </button>

      {expanded && (
      <div className="p-4 space-y-3 text-xs">
        <p className="text-[11px] text-zinc-500 dark:text-zinc-400 leading-relaxed">
          {t('clients.universalCard.description')}
        </p>

        <div className="grid grid-cols-1 sm:grid-cols-2 gap-2.5 pt-1">
          {/* OpenAI 兼容配置 */}
          <div className="p-3 rounded-md bg-zinc-500/[0.05] dark:bg-white/[0.05] flex flex-col justify-between gap-2.5">
            <div>
              <div className="flex items-center justify-between">
                <span className="font-semibold text-zinc-900 dark:text-zinc-100 text-[12px]">
                  {t('clients.universalCard.openaiTitle')}
                </span>
                <span className="text-[10px] text-zinc-400 font-mono">/v1/chat/completions</span>
              </div>
              <div className="font-mono text-[11px] text-zinc-600 dark:text-zinc-300 mt-1.5 truncate select-all">
                {openaiUrl}
              </div>
            </div>
            <Button
              type="button"
              variant="outline"
              size="xs"
              onClick={() => copyUrl('openai', openaiUrl)}
              className="self-start"
            >
              {copiedType === 'openai' ? (
                <Check className="w-3 h-3 text-emerald-500" />
              ) : (
                <Copy className="w-3 h-3" />
              )}
              <span>{copiedType === 'openai' ? t('clients.universalCard.copiedBaseUrl') : t('clients.universalCard.copyBaseUrl')}</span>
            </Button>
          </div>

          {/* Anthropic 兼容配置 */}
          <div className="p-3 rounded-md bg-zinc-500/[0.05] dark:bg-white/[0.05] flex flex-col justify-between gap-2.5">
            <div>
              <div className="flex items-center justify-between">
                <span className="font-semibold text-zinc-900 dark:text-zinc-100 text-[12px]">
                  {t('clients.universalCard.anthropicTitle')}
                </span>
                <span className="text-[10px] text-zinc-400 font-mono">/v1/messages</span>
              </div>
              <div className="font-mono text-[11px] text-zinc-600 dark:text-zinc-300 mt-1.5 truncate select-all">
                {anthropicUrl}
              </div>
            </div>
            <Button
              type="button"
              variant="outline"
              size="xs"
              onClick={() => copyUrl('anthropic', anthropicUrl)}
              className="self-start"
            >
              {copiedType === 'anthropic' ? (
                <Check className="w-3 h-3 text-emerald-500" />
              ) : (
                <Copy className="w-3 h-3" />
              )}
              <span>{copiedType === 'anthropic' ? t('clients.universalCard.copiedBaseUrl') : t('clients.universalCard.copyBaseUrl')}</span>
            </Button>
          </div>
        </div>
      </div>
      )}
    </section>
  );
}

export function ClientsView() {
  const dataReady = useAppStore((state) => state.initialLoaded && !state.isLoading);
  if (!dataReady) return <LoadingState />;
  return (
    <div className="space-y-3.5 animate-in fade-in duration-150">
      <IngressCard />
      <UniversalClientCard />
    </div>
  );
}

export function ProviderList() {
  const { t } = useTranslation();
  const {
    providers,
    initialLoaded,
    isLoading,
    openProviderEditor,
    reorderProviders,
    deleteProvider,
    activeTab,
    clearProviderLatencies,
  } = useAppStore(
    useShallow((state) => ({
      providers: state.providers,
      initialLoaded: state.initialLoaded,
      isLoading: state.isLoading,
      openProviderEditor: state.openProviderEditor,
      reorderProviders: state.reorderProviders,
      deleteProvider: state.deleteProvider,
      activeTab: state.activeTab,
      clearProviderLatencies: state.clearProviderLatencies,
    })),
  );

  // 离开列表页（切换 Tab 或组件卸载）时清空临时测速结果，离开后需重新测速
  useEffect(() => {
    if (activeTab !== 'providers') {
      clearProviderLatencies();
    }
  }, [activeTab, clearProviderLatencies]);

  useEffect(() => {
    return () => {
      clearProviderLatencies();
    };
  }, [clearProviderLatencies]);

  const listRef = useRef<HTMLDivElement>(null);
  const [drag, setDrag] = useState<DragState | null>(null);
  const dragRef = useRef<DragState | null>(null);
  const pressRef = useRef<PressState | null>(null);
  const suppressClickRef = useRef(false);
  const providersRef = useRef(providers);
  providersRef.current = providers;

  const [contextMenu, setContextMenu] = useState<{
    x: number;
    y: number;
    provider: Provider;
  } | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<Provider | null>(null);
  const [isDeleting, setIsDeleting] = useState(false);

  const handleRowPointerDown = (event: React.PointerEvent<HTMLDivElement>, provider: Provider) => {
    if (event.button !== 0) return;
    const target = event.target as HTMLElement;
    if (target.closest('[role="switch"], input, select, a, button')) return;

    const listRect = listRef.current?.getBoundingClientRect();
    if (!listRect) return;
    const rowRect = event.currentTarget.getBoundingClientRect();
    pressRef.current = {
      id: provider.id,
      row: event.currentTarget,
      startX: event.clientX,
      startY: event.clientY,
      grabOffset: event.clientY - rowRect.top,
      rowLeft: rowRect.left,
      rowWidth: rowRect.width,
      listTop: listRect.top,
      rowHeight: rowRect.height,
    };
  };

  const shouldSuppressClick = () => {
    if (!suppressClickRef.current) return false;
    suppressClickRef.current = false;
    return true;
  };

  const handleRowClick = (provider: Provider) => {
    openProviderEditor(provider);
  };

  useEffect(() => {
    const onMove = (event: PointerEvent) => {
      const press = pressRef.current;
      if (!press) return;

      const current = dragRef.current;
      if (!current) {
        const dx = event.clientX - press.startX;
        const dy = event.clientY - press.startY;

        if (Math.abs(dx) < 4 && Math.abs(dy) < 4) {
          return;
        }

        // 确认是整行拖动后才捕获指针，避免一开始就抢走模型条的横向滑动
        try {
          press.row.setPointerCapture(event.pointerId);
        } catch {
          // ignore
        }
        suppressClickRef.current = true;
        document.body.style.cursor = 'grabbing';
        document.body.style.userSelect = 'none';
        const started: DragState = {
          ...press,
          pointerY: event.clientY,
          previewIds: providersRef.current.map((item) => item.id),
        };
        dragRef.current = started;
        setDrag(started);
        return;
      }

      const slot = Math.max(
        0,
        Math.min(
          providersRef.current.length,
          Math.round((event.clientY - current.listTop) / current.rowHeight),
        ),
      );
      const from = current.previewIds.indexOf(current.id);
      const without = current.previewIds.filter((id) => id !== current.id);
      const insertAt = Math.max(0, Math.min(without.length, slot > from ? slot - 1 : slot));
      without.splice(insertAt, 0, current.id);

      const next = { ...current, pointerY: event.clientY, previewIds: without };
      dragRef.current = next;
      setDrag(next);
    };

    const finish = (commit: boolean) => {
      const current = dragRef.current;
      pressRef.current = null;
      dragRef.current = null;
      setDrag(null);
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
      if (commit && current) {
        const currentIds = providersRef.current.map((item) => item.id);
        const changed = current.previewIds.some((id, index) => id !== currentIds[index]);
        if (changed) {
          void reorderProviders(current.previewIds).catch(() => {
            // 全局错误条展示失败原因
          });
        }
      }
      // click 在 pointerup 之后同步派发，这里避免把拖动结束误判成打开编辑器
      setTimeout(() => {
        suppressClickRef.current = false;
      }, 0);
    };

    const onUp = () => finish(true);
    const onCancel = () => finish(false);
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') finish(false);
    };

    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
    window.addEventListener('pointercancel', onCancel);
    window.addEventListener('keydown', onKeyDown);
    return () => {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
      window.removeEventListener('pointercancel', onCancel);
      window.removeEventListener('keydown', onKeyDown);
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    };
  }, [reorderProviders]);

  const dataReady = initialLoaded && !isLoading;

  if (!dataReady) return <LoadingState />;

  const providerById = new Map(providers.map((item) => [item.id, item] as const));
  const orderedProviders = drag
    ? drag.previewIds
        .map((id) => providerById.get(id))
        .filter((item): item is Provider => Boolean(item))
    : providers;
  const draggedProvider = drag ? providerById.get(drag.id) : undefined;

  const dragIndex = drag ? drag.previewIds.indexOf(drag.id) : -1;
  const dragTotal = drag ? drag.previewIds.length : 0;
  const isFirstSlot = dragIndex === 0;
  const isLastSlot = dragIndex === dragTotal - 1;
  const isOnlySlot = dragTotal <= 1;

  const dragRoundedClass = isOnlySlot
    ? 'rounded-lg'
    : isFirstSlot
      ? 'rounded-t-lg rounded-b-none'
      : isLastSlot
        ? 'rounded-b-lg rounded-t-none'
        : 'rounded-none';

  return (
    <div className="animate-in fade-in duration-150">
      <section className={panelClass} aria-label={t('providers.title')}>
        {providers.length === 0 ? (
          <div className="px-4 py-10 text-center text-xs text-muted-foreground space-y-2">
            <p>{t('providers.emptyTitle')}</p>
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => openProviderEditor(null)}
            >
              <Plus className="w-3.5 h-3.5" />
              <span>{t('providers.addProvider')}</span>
            </Button>
          </div>
        ) : (
          <div className="divide-y divide-border" ref={listRef}>
            {orderedProviders.map((provider) => (
              <ProviderCard
                key={provider.id}
                provider={provider}
                isDragged={drag?.id === provider.id}
                onPointerDown={(event) => handleRowPointerDown(event, provider)}
                onRowClick={() => handleRowClick(provider)}
                onContextMenu={(event) => {
                  event.preventDefault();
                  setContextMenu({
                    x: event.clientX,
                    y: event.clientY,
                    provider,
                  });
                }}
                shouldSuppressClick={shouldSuppressClick}
              />
            ))}
          </div>
        )}
      </section>

      {/* 右键上下文菜单 */}
      {contextMenu && (
        <ProviderContextMenu
          x={contextMenu.x}
          y={contextMenu.y}
          provider={providers.find((p) => p.id === contextMenu.provider.id) ?? contextMenu.provider}
          onClose={() => setContextMenu(null)}
          onEdit={() => {
            const active = providers.find((p) => p.id === contextMenu.provider.id) ?? contextMenu.provider;
            openProviderEditor(active);
          }}
          onDelete={() => {
            const active = providers.find((p) => p.id === contextMenu.provider.id) ?? contextMenu.provider;
            setDeleteTarget(active);
          }}
        />
      )}

      {/* 删除服务商确认弹窗 */}
      {deleteTarget && (
        <ConfirmDialog
          open={deleteTarget !== null}
          title={t('providers.deleteConfirmTitle', { name: deleteTarget.name })}
          details={[
            t('providers.deleteConfirmDetail1'),
            t('providers.deleteConfirmDetail2'),
          ]}
          confirmLabel={t('providers.confirmDelete')}
          cancelLabel={t('common.cancel')}
          variant="danger"
          busy={isDeleting}
          onCancel={() => setDeleteTarget(null)}
          onConfirm={async () => {
            if (!deleteTarget) return;
            setIsDeleting(true);
            try {
              await deleteProvider(deleteTarget.id);
              setDeleteTarget(null);
            } finally {
              setIsDeleting(false);
            }
          }}
        />
      )}

      {drag &&
        draggedProvider &&
        createPortal(
          <div
            aria-hidden
            className={`pointer-events-none fixed z-50 overflow-hidden border border-border bg-card shadow-xl transition-[border-radius] duration-150 ease-out ${dragRoundedClass}`}
            style={{
              left: drag.rowLeft,
              width: drag.rowWidth,
              top: drag.pointerY - drag.grabOffset,
            }}
          >
            <ProviderCard
              provider={draggedProvider}
              isDragged={false}
              onPointerDown={() => {}}
              onRowClick={() => {}}
              onContextMenu={() => {}}
              shouldSuppressClick={() => false}
            />
          </div>,
          document.body,
        )}
    </div>
  );
}
