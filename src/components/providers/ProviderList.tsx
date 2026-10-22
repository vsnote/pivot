import { useState, useRef, useEffect } from 'react';
import { createPortal } from 'react-dom';
import { useShallow } from 'zustand/react/shallow';
import { useAppStore } from '@/store/useAppStore';
import { AGENT_TIERS, AGENT_TIER_KEYS, AGENT_TIER_IDS } from './agentTiers';
import { panelClass } from '@/components/common/panelStyles';
import { Provider, InterceptorStatus, RouteRule } from '@/types';
import { Switch } from '@/components/common/Switch';
import { Button } from '@/components/common/Button';
import { AgentConflictDialog } from '@/components/interceptor/AgentConflictDialog';
import {
  Plus,
  LoaderCircle,
  Copy,
  Check,
  Code2,
  ChevronDown,
  GripVertical,
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

function displayPath(path: string): string {
  const home = path.match(/^\/(?:Users|home)\/[^/]+/);
  return home ? `~${path.slice(home[0].length)}` : path;
}

function ClientMark({ id }: { id: InterceptorStatus['id'] }) {
  if (id === 'codex_cli') return <OpenAISpiralIcon className="w-full h-full" />;
  if (id === 'gemini_cli') return <GeminiLogo className="w-full h-full" />;
  if (id === 'pi') return <PiLogo className="w-full h-full" />;
  return <ClaudeLogo className="w-full h-full" />;
}

function clientStatus(client: InterceptorStatus): { label: string; className: string } {
  if (!client.installed) return { label: '未安装', className: 'text-zinc-400 dark:text-zinc-500' };
  if (client.isOverwritten) {
    return { label: '配置已被其他工具改写', className: 'text-amber-600 dark:text-amber-400' };
  }
  if (client.isIntercepted) return { label: '已接管', className: 'text-emerald-600 dark:text-emerald-400' };
  return { label: '未接管', className: 'text-zinc-500 dark:text-zinc-400' };
}

function ModelChip({
  provider,
  model,
  routes,
}: {
  provider: Provider;
  model: string;
  routes: RouteRule[];
}) {
  const activeTier = AGENT_TIER_KEYS.find((tierKey) => {
    const tier = AGENT_TIERS[tierKey];
    return routes.some(
      (r) =>
        r.id === tier.id &&
        r.enabled &&
        r.targetProviderId === provider.id &&
        r.targetModel === model,
    );
  });

  const tierInfo = activeTier ? AGENT_TIERS[activeTier] : null;
  const TierIcon = tierInfo ? tierInfo.icon : null;

  return (
    <span
      className="inline-flex shrink-0 items-center gap-1 px-1.5 py-px rounded-[5px] text-[11px] leading-4 font-mono select-none bg-zinc-500/[0.055] text-zinc-600 dark:bg-white/[0.06] dark:text-zinc-400"
      title={tierInfo ? `${model} · 承担${tierInfo.label}槽位` : model}
    >
      {tierInfo && TierIcon && (
        <span className="shrink-0 flex items-center" title={tierInfo.description}>
          <TierIcon className={`w-3 h-3 ${tierInfo.iconClass} stroke-[2.2]`} />
        </span>
      )}
      <span className="truncate max-w-[180px]">{model}</span>
    </span>
  );
}

function ProviderModelList({ provider }: { provider: Provider }) {
  const routes = useAppStore((state) => state.routes);
  const scrollRef = useRef<HTMLDivElement>(null);
  const [scrollMask, setScrollMask] = useState<{ canLeft: boolean; canRight: boolean }>({
    canLeft: false,
    canRight: false,
  });
  const [isOverflow, setIsOverflow] = useState(false);
  const [isDragging, setIsDragging] = useState(false);

  // 鼠标拖拽平滑滚动与动量惯性
  const isDownRef = useRef(false);
  const startXRef = useRef(0);
  const startScrollLeftRef = useRef(0);
  const hasDraggedRef = useRef(false);
  const lastXRef = useRef(0);
  const lastTimeRef = useRef(0);
  const velocityRef = useRef(0);
  const rafIdRef = useRef<number | null>(null);

  const isPriorityModel = (m: string) => {
    return routes.some(
      (r) =>
        AGENT_TIER_IDS.includes(r.id) &&
        r.enabled &&
        r.targetProviderId === provider.id &&
        r.targetModel === m,
    );
  };

  const sortedModels = [...provider.models].sort((a, b) => {
    const aPri = isPriorityModel(a) ? 1 : 0;
    const bPri = isPriorityModel(b) ? 1 : 0;
    return bPri - aPri;
  });

  const stopMomentum = () => {
    if (rafIdRef.current !== null) {
      cancelAnimationFrame(rafIdRef.current);
      rafIdRef.current = null;
    }
  };

  const startMomentum = (initialVelocity: number) => {
    stopMomentum();
    let v = initialVelocity;
    const step = () => {
      const el = scrollRef.current;
      if (!el || Math.abs(v) < 0.04) {
        stopMomentum();
        return;
      }
      el.scrollLeft -= v * 16;
      v *= 0.92;
      updateScrollState();
      rafIdRef.current = requestAnimationFrame(step);
    };
    rafIdRef.current = requestAnimationFrame(step);
  };

  const updateScrollState = () => {
    const el = scrollRef.current;
    if (!el) return;
    const canLeft = el.scrollLeft > 4;
    const canRight = el.scrollLeft + el.clientWidth < el.scrollWidth - 4;
    const overflow = el.scrollWidth > el.clientWidth + 2;
    setIsOverflow(overflow);
    setScrollMask((prev) => {
      if (prev.canLeft === canLeft && prev.canRight === canRight) return prev;
      return { canLeft, canRight };
    });
  };

  useEffect(() => {
    updateScrollState();
    const el = scrollRef.current;
    if (!el) return;

    const observer = new ResizeObserver(() => {
      updateScrollState();
    });
    observer.observe(el);

    window.addEventListener('resize', updateScrollState);
    return () => {
      observer.disconnect();
      window.removeEventListener('resize', updateScrollState);
      stopMomentum();
    };
  }, [sortedModels.length]);

  const handleWheel = (e: React.WheelEvent<HTMLDivElement>) => {
    const el = scrollRef.current;
    if (!el) return;
    if (Math.abs(e.deltaY) > Math.abs(e.deltaX) && el.scrollWidth > el.clientWidth) {
      e.stopPropagation();
      stopMomentum();
      el.scrollLeft += e.deltaY;
      updateScrollState();
    }
  };

  const handlePointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return; // 仅主键（左键）拖动
    const el = scrollRef.current;
    if (!el || el.scrollWidth <= el.clientWidth + 2) return;

    stopMomentum();
    isDownRef.current = true;
    startXRef.current = e.clientX;
    startScrollLeftRef.current = el.scrollLeft;
    hasDraggedRef.current = false;
    lastXRef.current = e.clientX;
    lastTimeRef.current = performance.now();
    velocityRef.current = 0;
  };

  const handlePointerMove = (e: React.PointerEvent<HTMLDivElement>) => {
    if (!isDownRef.current) return;
    const el = scrollRef.current;
    if (!el) return;

    const dx = e.clientX - startXRef.current;
    if (!hasDraggedRef.current && Math.abs(dx) > 4) {
      hasDraggedRef.current = true;
      setIsDragging(true);
      try {
        e.currentTarget.setPointerCapture(e.pointerId);
      } catch {
        // ignore
      }
    }

    if (hasDraggedRef.current) {
      el.scrollLeft = startScrollLeftRef.current - dx;
      updateScrollState();

      const now = performance.now();
      const dt = now - lastTimeRef.current;
      if (dt > 8) {
        velocityRef.current = (e.clientX - lastXRef.current) / dt;
        lastXRef.current = e.clientX;
        lastTimeRef.current = now;
      }
    }
  };

  const handlePointerUp = (e: React.PointerEvent<HTMLDivElement>) => {
    if (!isDownRef.current) return;
    isDownRef.current = false;

    if (hasDraggedRef.current) {
      try {
        if (e.currentTarget.hasPointerCapture(e.pointerId)) {
          e.currentTarget.releasePointerCapture(e.pointerId);
        }
      } catch {
        // ignore
      }

      setIsDragging(false);

      const now = performance.now();
      if (now - lastTimeRef.current < 100 && Math.abs(velocityRef.current) > 0.15) {
        startMomentum(velocityRef.current);
      }

      setTimeout(() => {
        hasDraggedRef.current = false;
      }, 50);
    }
  };

  const handleClickCapture = (e: React.MouseEvent) => {
    if (hasDraggedRef.current) {
      e.stopPropagation();
      e.preventDefault();
      hasDraggedRef.current = false;
    }
  };

  if (provider.models.length === 0) {
    return (
      <span className="text-[11px] text-muted-foreground/60">
        未配置模型
      </span>
    );
  }

  let maskStyle: React.CSSProperties = {};
  if (scrollMask.canLeft && scrollMask.canRight) {
    maskStyle = {
      maskImage: 'linear-gradient(to right, transparent 0%, black 20px, black calc(100% - 24px), transparent 100%)',
      WebkitMaskImage: 'linear-gradient(to right, transparent 0%, black 20px, black calc(100% - 24px), transparent 100%)',
    };
  } else if (scrollMask.canRight) {
    maskStyle = {
      maskImage: 'linear-gradient(to right, black calc(100% - 24px), transparent 100%)',
      WebkitMaskImage: 'linear-gradient(to right, black calc(100% - 24px), transparent 100%)',
    };
  } else if (scrollMask.canLeft) {
    maskStyle = {
      maskImage: 'linear-gradient(to right, transparent 0%, black 20px)',
      WebkitMaskImage: 'linear-gradient(to right, transparent 0%, black 20px)',
    };
  }

  return (
    <div className="relative min-w-0 flex-1">
      <div
        ref={scrollRef}
        data-no-row-drag=""
        onScroll={updateScrollState}
        onWheel={handleWheel}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerUp}
        onPointerCancel={handlePointerUp}
        onClickCapture={handleClickCapture}
        style={maskStyle}
        className={`flex flex-nowrap items-center gap-1.5 overflow-x-auto no-scrollbar py-0.5 ${
          isDragging
            ? 'select-none cursor-grabbing [&_*]:cursor-grabbing [&_*]:select-none'
            : isOverflow
              ? 'cursor-grab'
              : ''
        }`}
      >
        {sortedModels.map((m) => (
          <ModelChip key={m} provider={provider} model={m} routes={routes} />
        ))}
      </div>
    </div>
  );
}

interface PressState {
  id: string;
  row: HTMLDivElement;
  startX: number;
  startY: number;
  inScroll: boolean;
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
  shouldSuppressClick: () => boolean;
}

function ProviderCard({
  provider,
  isDragged,
  onPointerDown,
  onRowClick,
  shouldSuppressClick,
}: ProviderCardProps) {
  const { openProviderEditor, toggleProvider } = useAppStore(
    useShallow((state) => ({
      openProviderEditor: state.openProviderEditor,
      toggleProvider: state.toggleProvider,
    })),
  );
  const [toggling, setToggling] = useState(false);

  const handleToggle = async () => {
    if (toggling) return;
    setToggling(true);
    try {
      await toggleProvider(provider.id, !provider.enabled);
    } finally {
      setToggling(false);
    }
  };

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onRowClick}
      onClickCapture={(event) => {
        // 拖动结束后浏览器仍会补发 click，这里拦掉复制/开关等附带动作
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
      className={`group relative flex items-center justify-between gap-3 px-4 py-2 transition-colors cursor-pointer select-none ${
        provider.enabled
          ? 'hover:bg-accent/50'
          : 'opacity-55 bg-zinc-50/20 dark:bg-zinc-900/20 hover:bg-zinc-100/40 dark:hover:bg-zinc-800/30'
      } ${isDragged ? 'opacity-25' : ''}`}
      aria-label={`编辑服务商 ${provider.name}`}
    >
      <GripVertical
        aria-hidden
        className="pointer-events-none absolute left-0.5 top-1/2 h-3 w-3 -translate-y-1/2 text-zinc-400 opacity-0 transition-opacity group-hover:opacity-70 dark:text-zinc-500"
      />

      <div className="flex items-center gap-2.5 min-w-0 flex-1">
        <ProviderAvatar
          name={provider.name}
          protocol={provider.protocol}
          iconKey={provider.presetId || undefined}
          sizeClass="w-6 h-6"
          dense
        />

        <div className="flex items-center gap-2 min-w-0 flex-1">
          <h3 className="min-w-0 shrink text-[13px] font-semibold text-zinc-900 dark:text-zinc-100 truncate group-hover:text-zinc-950 dark:group-hover:text-white transition-colors">
            {provider.name}
          </h3>
          {provider.isDefault && (
            <span
              title={
                provider.enabled
                  ? `未在任何服务商声明的模型会转发到 ${provider.defaultModel || provider.models[0] || '它的默认模型'}`
                  : '默认服务商已停用，未声明的模型将无法路由'
              }
              className={`shrink-0 text-[10px] leading-none ${
                provider.enabled
                  ? 'text-zinc-400 dark:text-zinc-500'
                  : 'text-amber-600 dark:text-amber-400'
              }`}
            >
              默认
            </span>
          )}

          <ProviderModelList provider={provider} />
        </div>
      </div>

      <div className="shrink-0 flex items-center" onClick={(e) => e.stopPropagation()}>
        <Switch
          checked={provider.enabled}
          disabled={toggling}
          onCheckedChange={handleToggle}
          ariaLabel={provider.enabled ? `停止 ${provider.name} 参与路由` : `让 ${provider.name} 参与路由`}
        />
      </div>
    </div>
  );
}

function IngressCard() {
  const { interceptors, toggleInterceptor, providers, stats, initialLoaded } = useAppStore(
    useShallow((state) => ({
      interceptors: state.interceptors,
      toggleInterceptor: state.toggleInterceptor,
      providers: state.providers,
      stats: state.stats,
      initialLoaded: state.initialLoaded,
    })),
  );
  const [togglingId, setTogglingId] = useState<string | null>(null);
  const [conflictConfirmTarget, setConflictConfirmTarget] = useState<InterceptorStatus | null>(null);

  const hasEnabledProviders = providers.some((p) => p.enabled);
  const gatewayUp = stats.isRunning;

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
    if (!gatewayUp) return { text: `${stats.port} 未在监听，接管不会生效。`, tone: 'fault' };
    const takenOver = interceptors.some((client) => client.isIntercepted);
    if (takenOver && !hasEnabledProviders) {
      return { text: '已接管的请求还没有可转发的上游。', tone: 'warn' };
    }
    const installed = interceptors.some((client) => client.installed);
    if (hasEnabledProviders && installed && !takenOver) {
      return { text: '没有客户端被接管，请求仍直连官方；手动接入请展开下方指南。', tone: 'hint' };
    }
    return null;
  })();

  return (
    <>
      <section className={panelClass} aria-label="客户端列表">
        {interceptors.length === 0 ? (
          <p className="px-4 py-6 text-center text-xs text-zinc-400">没有读到本机客户端。</p>
        ) : (
          <div className="divide-y divide-border">
            {interceptors.map((client) => {
              const label = CLIENT_LABEL[client.id] ?? client.name;
              const status = clientStatus(client);
              const busy = togglingId === client.id;
              const path = client.installed && client.configPath ? displayPath(client.configPath) : '';

              return (
                <div key={client.id} className="flex items-center justify-between gap-3 px-4 py-2">
                  <div className="flex items-center gap-2.5 min-w-0">
                    <div className={`w-6 h-6 shrink-0 flex items-center justify-center p-1 ${ICON_SLOT}`}>
                      <ClientMark id={client.id} />
                    </div>
                    <h3 className="shrink-0 text-[13px] font-semibold text-zinc-900 dark:text-zinc-100">
                      {label}
                    </h3>
                    <p className="min-w-0 text-[11px] truncate">
                      <span className={status.className}>{status.label}</span>
                      {path && (
                        <span className="text-zinc-400 dark:text-zinc-500 select-text font-mono text-[10px]" title={client.configPath}>
                          {' '}
                          · {path}
                        </span>
                      )}
                    </p>
                  </div>

                  <div className="shrink-0">
                    {busy ? (
                      <LoaderCircle className="w-4 h-4 animate-spin text-zinc-400" />
                    ) : client.isOverwritten ? (
                      <button
                        type="button"
                        onClick={() => setConflictConfirmTarget(client)}
                        className="px-2 py-0.5 rounded text-[11px] font-medium text-amber-700 dark:text-amber-300 bg-amber-50 dark:bg-amber-950/40 border border-amber-200/80 dark:border-amber-900/50 cursor-pointer"
                      >
                        解决冲突
                      </button>
                    ) : client.installed ? (
                      <Switch
                        checked={client.isIntercepted}
                        disabled={togglingId !== null}
                        onCheckedChange={(checked) => handleSwitch(client, checked)}
                        ariaLabel={client.isIntercepted ? `恢复 ${label}` : `接管 ${label}`}
                      />
                    ) : null}
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
          <span>接管后客户端请求将流经 Pivot 分发；未接管则走官方直连。</span>
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
  return (
    <div className="p-12 text-center">
      <LoaderCircle className="w-6 h-6 text-zinc-600 dark:text-zinc-400 animate-spin mx-auto mb-2" />
      <p className="text-xs text-zinc-400">载入配置中…</p>
    </div>
  );
}

function UniversalClientCard() {
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
    <section className={panelClass} aria-label="通用客户端手动接入">
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
            通用客户端手动接入
          </span>
        </div>
        <ChevronDown
          className={`w-3.5 h-3.5 text-zinc-400 transition-transform ${expanded ? 'rotate-180' : ''}`}
        />
      </button>

      {expanded && (
      <div className="p-4 space-y-3 text-xs">
        <p className="text-[11px] text-zinc-500 dark:text-zinc-400 leading-relaxed">
          在任意支持自定义 API 的开发工具中填入下方 Base URL，API Key 可填任意字符串（如 <code className="font-mono px-1 py-0.5 rounded bg-muted text-foreground">pivot</code>），Pivot 将根据请求的模型智能分发到对应的上游服务商。
        </p>

        <div className="grid grid-cols-1 sm:grid-cols-2 gap-2.5 pt-1">
          {/* OpenAI 兼容配置 */}
          <div className="p-3 rounded-md bg-zinc-500/[0.05] dark:bg-white/[0.05] flex flex-col justify-between gap-2.5">
            <div>
              <div className="flex items-center justify-between">
                <span className="font-semibold text-zinc-900 dark:text-zinc-100 text-[12px]">
                  OpenAI 兼容协议
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
              <span>{copiedType === 'openai' ? '已复制 Base URL' : '复制 Base URL'}</span>
            </Button>
          </div>

          {/* Anthropic 兼容配置 */}
          <div className="p-3 rounded-md bg-zinc-500/[0.05] dark:bg-white/[0.05] flex flex-col justify-between gap-2.5">
            <div>
              <div className="flex items-center justify-between">
                <span className="font-semibold text-zinc-900 dark:text-zinc-100 text-[12px]">
                  Anthropic 兼容协议
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
              <span>{copiedType === 'anthropic' ? '已复制 Base URL' : '复制 Base URL'}</span>
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
  const { providers, initialLoaded, isLoading, openProviderEditor, reorderProviders } = useAppStore(
    useShallow((state) => ({
      providers: state.providers,
      initialLoaded: state.initialLoaded,
      isLoading: state.isLoading,
      openProviderEditor: state.openProviderEditor,
      reorderProviders: state.reorderProviders,
    })),
  );

  const listRef = useRef<HTMLDivElement>(null);
  const [drag, setDrag] = useState<DragState | null>(null);
  const dragRef = useRef<DragState | null>(null);
  const pressRef = useRef<PressState | null>(null);
  const suppressClickRef = useRef(false);
  const providersRef = useRef(providers);
  providersRef.current = providers;

  const handleRowPointerDown = (event: React.PointerEvent<HTMLDivElement>, provider: Provider) => {
    if (event.button !== 0) return;
    // 开关等表单控件保持原生语义；模型条里的标签允许以纵向拖动启动整行拖动
    const target = event.target as HTMLElement;
    const inScroll = Boolean(target.closest('[data-no-row-drag]'));
    if (target.closest('[role="switch"], input, select, a')) return;
    if (!inScroll && target.closest('button')) return;

    const listRect = listRef.current?.getBoundingClientRect();
    if (!listRect) return;
    const rowRect = event.currentTarget.getBoundingClientRect();
    pressRef.current = {
      id: provider.id,
      row: event.currentTarget,
      startX: event.clientX,
      startY: event.clientY,
      inScroll,
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

        if (press.inScroll) {
          // 模型条上的手势：横向交给它自己滚动，纵向才接管为整行拖动
          if (Math.abs(dx) > 4 && Math.abs(dx) > Math.abs(dy)) {
            pressRef.current = null;
            return;
          }
          if (Math.abs(dy) <= 6 || Math.abs(dy) < Math.abs(dx)) return;
        } else if (Math.abs(dx) < 4 && Math.abs(dy) < 4) {
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

  const hasDefault = providers.some((p) => p.isDefault && p.enabled);
  const anyEnabled = providers.some((p) => p.enabled);

  return (
    <div className="animate-in fade-in duration-150">
      <section className={panelClass} aria-label="服务商">
        {providers.length === 0 ? (
          <div className="px-4 py-10 text-center text-xs text-muted-foreground space-y-2">
            <p>尚未配置任何服务商渠道</p>
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => openProviderEditor(null)}
            >
              <Plus className="w-3.5 h-3.5" />
              <span>添加服务商</span>
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
                shouldSuppressClick={shouldSuppressClick}
              />
            ))}
          </div>
        )}
      </section>

      {anyEnabled && !hasDefault && (
        <p className="px-1 pt-2 text-[11px] text-amber-600 dark:text-amber-400">
          没有启用的默认服务商：所有服务商都未声明的模型将无法路由。
        </p>
      )}

      {drag &&
        draggedProvider &&
        createPortal(
          <div
            aria-hidden
            className="pointer-events-none fixed z-50 overflow-hidden rounded-lg border border-border bg-card shadow-xl"
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
              shouldSuppressClick={() => false}
            />
          </div>,
          document.body,
        )}
    </div>
  );
}
