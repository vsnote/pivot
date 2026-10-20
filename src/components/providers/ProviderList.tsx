import { useState, useRef, useEffect } from 'react';
import { createPortal } from 'react-dom';
import { useShallow } from 'zustand/react/shallow';
import { useAppStore } from '@/store/useAppStore';
import { Provider, InterceptorStatus } from '@/types';
import { Switch } from '@/components/common/Switch';
import { Button } from '@/components/common/Button';
import { Badge } from '@/components/common/Badge';
import { AgentConflictDialog } from '@/components/interceptor/AgentConflictDialog';
import {
  Plus,
  LoaderCircle,
  Copy,
  Check,
  Code2,
  Zap,
  Brain,
} from 'lucide-react';

import {
  ProviderAvatar,
  OpenAISpiralIcon,
  ClaudeLogo,
  GeminiLogo,
} from '@/components/common/BrandIcons';

const panelClass =
  'rounded-lg border border-border bg-card text-card-foreground shadow-2xs overflow-hidden';

const CLIENT_LABEL: Record<InterceptorStatus['id'], string> = {
  claude_code: 'Claude Code',
  codex_cli: 'Codex CLI',
  gemini_cli: 'Gemini CLI',
};

function displayPath(path: string): string {
  const home = path.match(/^\/(?:Users|home)\/[^/]+/);
  return home ? `~${path.slice(home[0].length)}` : path;
}

function ClientMark({ id }: { id: InterceptorStatus['id'] }) {
  if (id === 'codex_cli') return <OpenAISpiralIcon className="w-full h-full" />;
  if (id === 'gemini_cli') return <GeminiLogo className="w-full h-full" />;
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

type AgentTierKey = 'main' | 'fast' | 'reasoning';

const AGENT_TIERS: Record<
  AgentTierKey,
  {
    id: string;
    label: string;
    shortLabel: string;
    icon: typeof Code2;
    pattern: string;
    description: string;
    chipClass: string;
    iconClass: string;
  }
> = {
  main: {
    id: 'tier-main',
    label: '主力编程',
    shortLabel: '主力',
    icon: Code2,
    pattern: '.*(sonnet|gpt-4o|claude-3-7|claude-4|fable).*',
    description: 'Agent 主力编程槽位 (Sonnet/4o/主力任务)',
    chipClass: 'bg-blue-50/80 text-blue-900 border-blue-200/90 dark:bg-blue-950/40 dark:text-blue-200 dark:border-blue-800/70 font-medium',
    iconClass: 'text-blue-600 dark:text-blue-400',
  },
  fast: {
    id: 'tier-fast',
    label: '轻量快速',
    shortLabel: '快速',
    icon: Zap,
    pattern: '.*(haiku|flash|mini|small|instant).*',
    description: 'Agent 轻量辅助槽位 (Haiku/Flash/子任务)',
    chipClass: 'bg-amber-50/80 text-amber-900 border-amber-200/90 dark:bg-amber-950/40 dark:text-amber-200 dark:border-amber-800/70 font-medium',
    iconClass: 'text-amber-600 dark:text-amber-400',
  },
  reasoning: {
    id: 'tier-reasoning',
    label: '深度推理',
    shortLabel: '推理',
    icon: Brain,
    pattern: '.*(opus|thinking|reasoner|r1|o1|o3).*',
    description: 'Agent 深度推理槽位 (Opus/Thinking/o-series)',
    chipClass: 'bg-purple-50/80 text-purple-900 border-purple-200/90 dark:bg-purple-950/40 dark:text-purple-200 dark:border-purple-800/70 font-medium',
    iconClass: 'text-purple-600 dark:text-purple-400',
  },
};

interface PopoverPortalProps {
  open: boolean;
  onClose: () => void;
  triggerRect: DOMRect | null;
  children: React.ReactNode;
}

function PopoverPortal({ open, onClose, triggerRect, children }: PopoverPortalProps) {
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const handlePointerDown = (e: MouseEvent | TouchEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    const handleScroll = (e: Event) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    document.addEventListener('pointerdown', handlePointerDown);
    window.addEventListener('scroll', handleScroll, true);
    return () => {
      document.removeEventListener('pointerdown', handlePointerDown);
      window.removeEventListener('scroll', handleScroll, true);
    };
  }, [open, onClose]);

  if (!open || !triggerRect) return null;

  const menuWidth = 220;
  const spaceBelow = window.innerHeight - triggerRect.bottom;
  const spaceAbove = triggerRect.top;

  // 只要下方空间不足且上方空间更大，向上弹出
  const showAbove = spaceBelow < 180 && spaceAbove > spaceBelow;
  const maxAvailable = showAbove ? spaceAbove - 16 : spaceBelow - 16;
  const maxHeight = Math.max(140, Math.min(360, maxAvailable));

  const top = showAbove ? undefined : triggerRect.bottom + 6;
  const bottom = showAbove ? window.innerHeight - triggerRect.top + 6 : undefined;

  let left = triggerRect.left;
  if (left + menuWidth > window.innerWidth - 12) {
    left = window.innerWidth - menuWidth - 12;
  }
  if (left < 12) left = 12;

  return createPortal(
    <div
      ref={menuRef}
      role="menu"
      onClick={(e) => e.stopPropagation()}
      style={{
        position: 'fixed',
        top: top !== undefined ? `${top}px` : undefined,
        bottom: bottom !== undefined ? `${bottom}px` : undefined,
        left: `${left}px`,
        maxHeight: `${maxHeight}px`,
        zIndex: 9999,
      }}
      className="w-55 rounded-lg border border-border/80 bg-popover text-popover-foreground shadow-lg p-1 animate-in fade-in zoom-in-95 duration-100 text-xs text-left overflow-hidden no-scrollbar"
    >
      {children}
    </div>,
    document.body
  );
}

function ModelChip({ provider, model }: { provider: Provider; model: string }) {
  const { routes, saveRoutes } = useAppStore(
    useShallow((state) => ({
      routes: state.routes,
      saveRoutes: state.saveRoutes,
    })),
  );

  const [open, setOpen] = useState(false);
  const [triggerRect, setTriggerRect] = useState<DOMRect | null>(null);
  const [copied, setCopied] = useState(false);
  const buttonRef = useRef<HTMLButtonElement>(null);

  const activeTier = (Object.keys(AGENT_TIERS) as AgentTierKey[]).find((tierKey) => {
    const tier = AGENT_TIERS[tierKey];
    return routes.some(
      (r) =>
        r.id === tier.id &&
        r.enabled &&
        r.targetProviderId === provider.id &&
        r.targetModel === model,
    );
  });

  const handleOpen = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (buttonRef.current) {
      setTriggerRect(buttonRef.current.getBoundingClientRect());
    }
    setOpen((prev) => !prev);
  };

  // 严格互斥与原子性槽位分配
  const handleToggleTier = async (tierKey: AgentTierKey) => {
    const tier = AGENT_TIERS[tierKey];
    const isCurrentlyThisTier = activeTier === tierKey;
    const allTierIds = Object.values(AGENT_TIERS).map((t) => t.id);

    // 1. 移除目标槽位原有的绑定（槽位全局单一）
    // 2. 移除当前模型在其他槽位的绑定（模型单槽位）
    const nextRoutes = routes.filter((r) => {
      if (r.id === tier.id) return false;
      if (allTierIds.includes(r.id) && r.targetProviderId === provider.id && r.targetModel === model) {
        return false;
      }
      return true;
    });

    if (!isCurrentlyThisTier) {
      nextRoutes.unshift({
        id: tier.id,
        pattern: tier.pattern,
        targetProviderId: provider.id,
        targetModel: model,
        fallbackProviderIds: [],
        enabled: true,
        priority: 500,
        description: tier.description,
      });
    }

    try {
      await saveRoutes(nextRoutes);
      setOpen(false);
    } catch {
      // 错误由 store 统一捕获
    }
  };

  const copyModelName = () => {
    void navigator.clipboard.writeText(model).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  };

  const tierInfo = activeTier ? AGENT_TIERS[activeTier] : null;
  const TierIcon = tierInfo ? tierInfo.icon : null;

  return (
    <div className="relative inline-flex shrink-0" onClick={(e) => e.stopPropagation()}>
      <button
        ref={buttonRef}
        type="button"
        onClick={handleOpen}
        className={`group/chip inline-flex shrink-0 items-center gap-1.5 px-2 py-0.5 rounded-[5px] text-[11px] font-mono transition-all cursor-pointer border select-none ${
          tierInfo
            ? `${tierInfo.chipClass} shadow-2xs`
            : 'bg-zinc-100/70 hover:bg-zinc-100/90 text-zinc-600 hover:text-zinc-900 border-zinc-200/70 hover:border-zinc-300 dark:bg-zinc-800/40 dark:hover:bg-zinc-800/70 dark:text-zinc-400 dark:hover:text-zinc-200 dark:border-zinc-700/50 dark:hover:border-zinc-600'
        } ${open ? 'ring-1 ring-ring/80 border-ring' : ''}`}
        title={`点击管理 ${model} 槽位或复制模型名称`}
      >
        {tierInfo && TierIcon && (
          <span className="shrink-0 flex items-center" title={tierInfo.description}>
            <TierIcon className={`w-3 h-3 ${tierInfo.iconClass} stroke-[2.2]`} />
          </span>
        )}
        <span className="truncate max-w-[180px]">{model}</span>
      </button>

      <PopoverPortal open={open} onClose={() => setOpen(false)} triggerRect={triggerRect}>
        {/* 顶部标题与一键复制模型名称 */}
        <div className="px-2.5 py-1.5 border-b border-border/60 flex items-center justify-between gap-2">
          <div className="min-w-0">
            <div className="font-mono font-medium text-[11px] truncate text-foreground leading-tight">
              {model}
            </div>
            <div className="text-[10px] text-muted-foreground truncate mt-0.5">
              {provider.name} · {provider.protocol}
            </div>
          </div>
          <button
            type="button"
            onClick={copyModelName}
            title="复制模型名称"
            className="shrink-0 p-1 rounded hover:bg-accent text-muted-foreground hover:text-foreground transition-colors cursor-pointer"
          >
            {copied ? (
              <Check className="w-3.5 h-3.5 text-emerald-500" />
            ) : (
              <Copy className="w-3.5 h-3.5" />
            )}
          </button>
        </div>

        {/* 分配 Agent 角色槽位 */}
        <div className="p-1 space-y-0.5">
          <div className="px-2 pt-1 pb-0.5 text-[10px] font-medium text-muted-foreground">
            分配 Agent 槽位
          </div>
          {(Object.keys(AGENT_TIERS) as AgentTierKey[]).map((tierKey) => {
            const tier = AGENT_TIERS[tierKey];
            const isSelected = activeTier === tierKey;
            const Icon = tier.icon;

            return (
              <button
                key={tierKey}
                type="button"
                onClick={() => handleToggleTier(tierKey)}
                className={`w-full flex items-center justify-between px-2 py-1.5 rounded-md text-left transition-colors cursor-pointer ${
                  isSelected
                    ? 'bg-accent font-medium text-accent-foreground'
                    : 'hover:bg-accent/60 text-muted-foreground hover:text-foreground'
                }`}
              >
                <div className="flex items-center gap-2 text-[11px] min-w-0">
                  <Icon className={`w-3.5 h-3.5 shrink-0 ${isSelected ? tier.iconClass : 'text-muted-foreground'}`} />
                  <span className="truncate">{tier.label}槽位</span>
                </div>
                {isSelected ? (
                  <Check className="w-3.5 h-3.5 shrink-0 text-foreground stroke-[2.5]" />
                ) : (
                  <span className="text-[10px] text-muted-foreground/50 shrink-0">点击绑定</span>
                )}
              </button>
            );
          })}
        </div>
      </PopoverPortal>
    </div>
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

  const allTierIds = Object.values(AGENT_TIERS).map((t) => t.id);
  const isPriorityModel = (m: string) => {
    return routes.some(
      (r) =>
        allTierIds.includes(r.id) &&
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
    <div className="relative min-w-0 max-w-full mt-1.5" onClick={(e) => e.stopPropagation()}>
      <div
        ref={scrollRef}
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
          <ModelChip key={m} provider={provider} model={m} />
        ))}
      </div>
    </div>
  );
}

function ProviderCard({ provider }: { provider: Provider }) {
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
      onClick={() => openProviderEditor(provider)}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          openProviderEditor(provider);
        }
      }}
      className={`group flex items-center justify-between gap-3 px-4 py-3 transition-colors cursor-pointer select-none ${
        provider.enabled
          ? 'hover:bg-accent/50'
          : 'opacity-55 bg-zinc-50/20 dark:bg-zinc-900/20 hover:bg-zinc-100/40 dark:hover:bg-zinc-800/30'
      }`}
      aria-label={`编辑服务商 ${provider.name}`}
    >
      <div className="flex items-center gap-3 min-w-0 flex-1">
        <ProviderAvatar
          name={provider.name}
          protocol={provider.protocol}
          iconKey={provider.presetId || undefined}
          sizeClass="w-8 h-8"
        />

        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-1.5">
            <h3 className="text-[13px] font-semibold text-zinc-900 dark:text-zinc-100 truncate group-hover:text-zinc-950 dark:group-hover:text-white transition-colors">
              {provider.name}
            </h3>
            {provider.isDefault && (
              <Badge
                variant="outline"
                className="text-[10px] px-1.5 py-0 font-normal bg-zinc-100/90 dark:bg-zinc-800/90 text-zinc-600 dark:text-zinc-400 border-zinc-200/90 dark:border-zinc-700/80"
              >
                {provider.enabled ? '默认服务商' : '默认 (已停用)'}
              </Badge>
            )}
          </div>

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

function CopyAddressButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false);

  const copy = () => {
    void navigator.clipboard.writeText(text).then(
      () => {
        setCopied(true);
        window.setTimeout(() => setCopied(false), 1600);
      },
      () => {},
    );
  };

  return (
    <button
      type="button"
      onClick={copy}
      title={copied ? "已复制" : `复制完整地址 (${text})`}
      aria-label="复制网关地址"
      className="inline-flex items-center gap-1 p-1 -my-1 rounded text-muted-foreground hover:text-foreground hover:bg-muted/80 transition-colors cursor-pointer"
    >
      {copied ? (
        <>
          <Check className="w-3.5 h-3.5 text-emerald-500" />
          <span className="text-[10px] text-emerald-600 dark:text-emerald-400 font-sans select-none">
            已复制
          </span>
        </>
      ) : (
        <Copy className="w-3.5 h-3.5" />
      )}
    </button>
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
  const address = `http://${stats.host}:${stats.port}`;
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
      return { text: 'CLI 仍走官方。手动接入使用上面的网关地址。', tone: 'hint' };
    }
    return null;
  })();

  return (
    <>
      <section className={panelClass} aria-label="网关与客户端">
        <div className="flex items-center gap-2 px-4 py-2.5 bg-muted/30 border-b border-border text-xs min-w-0">
          <span
            className={`w-1.5 h-1.5 rounded-full shrink-0 ${gatewayUp ? 'bg-emerald-500 shadow-[0_0_6px_rgba(16,185,129,0.5)]' : 'bg-rose-500'}`}
            aria-hidden
          />
          <span className="font-semibold text-zinc-700 dark:text-zinc-200 tracking-wide shrink-0">本地网关</span>
          <span className="font-mono text-zinc-400 dark:text-zinc-500 truncate select-text text-[11px]">
            {stats.host}:{stats.port}
          </span>
          <CopyAddressButton text={address} />
        </div>

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
                <div key={client.id} className="flex items-center justify-between gap-3 px-4 py-2.5">
                  <div className="flex items-center gap-3 min-w-0">
                    <div className="w-8 h-8 rounded-md flex items-center justify-center shrink-0 border border-zinc-200/70 dark:border-zinc-700/60 bg-zinc-50/80 dark:bg-zinc-800/80 p-1.5">
                      <ClientMark id={client.id} />
                    </div>
                    <div className="min-w-0">
                      <h3 className="text-[13px] font-semibold text-zinc-900 dark:text-zinc-100 truncate">
                        {label}
                      </h3>
                      <p className="text-[11px] truncate mt-0.5">
                        <span className={status.className}>{status.label}</span>
                        {path && (
                          <span className="text-zinc-400 dark:text-zinc-500 select-text font-mono text-[10px]" title={client.configPath}>
                            {' '}
                            · {path}
                          </span>
                        )}
                      </p>
                    </div>
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

  const openaiUrl = `http://${stats.host}:${stats.port}/v1`;
  const anthropicUrl = `http://${stats.host}:${stats.port}`;

  const copyUrl = (type: 'openai' | 'anthropic', url: string) => {
    navigator.clipboard.writeText(url);
    setCopiedType(type);
    setTimeout(() => setCopiedType(null), 2000);
  };

  return (
    <section className={panelClass} aria-label="通用客户端手动接入">
      <div className="flex items-center justify-between px-4 py-2.5 bg-muted/30 border-b border-border text-xs">
        <div className="flex items-center gap-2">
          <Code2 className="w-3.5 h-3.5 text-zinc-500" />
          <span className="font-semibold text-zinc-700 dark:text-zinc-200 tracking-wide">
            通用客户端接入指南
          </span>
        </div>
        <span className="text-[11px] text-zinc-400 font-mono">Cursor / Aider / Continue / 脚本</span>
      </div>

      <div className="p-4 space-y-3 text-xs">
        <p className="text-[11px] text-zinc-500 dark:text-zinc-400 leading-relaxed">
          在任意支持自定义 API 的开发工具中填入下方 Base URL，API Key 可填任意字符串（如 <code className="font-mono px-1 py-0.5 rounded bg-muted text-foreground">pivot</code>），Pivot 将根据请求的模型智能分发到对应的上游服务商。
        </p>

        <div className="grid grid-cols-1 sm:grid-cols-2 gap-2.5 pt-1">
          {/* OpenAI 兼容配置 */}
          <div className="p-3 rounded-md bg-muted/30 border border-border flex flex-col justify-between gap-2.5">
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
          <div className="p-3 rounded-md bg-muted/30 border border-border flex flex-col justify-between gap-2.5">
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
  const { providers, initialLoaded, isLoading, openProviderEditor } = useAppStore(
    useShallow((state) => ({
      providers: state.providers,
      initialLoaded: state.initialLoaded,
      isLoading: state.isLoading,
      openProviderEditor: state.openProviderEditor,
    })),
  );

  const dataReady = initialLoaded && !isLoading;

  if (!dataReady) return <LoadingState />;

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
          <div className="divide-y divide-border">
            {providers.map((provider) => (
              <ProviderCard key={provider.id} provider={provider} />
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
