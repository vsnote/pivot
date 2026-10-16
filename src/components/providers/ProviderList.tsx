import { useState } from 'react';
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
      className={`group flex items-center justify-between gap-3 px-4 py-2.5 transition-colors cursor-pointer select-none ${
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
              <Badge variant="outline" className="text-[10px] px-1.5 py-0 font-normal bg-zinc-100/80 dark:bg-zinc-800/80 text-zinc-700 dark:text-zinc-300 border-zinc-200/90 dark:border-zinc-700/80">
                默认
              </Badge>
            )}
          </div>

          <div className="flex items-center gap-1.5 mt-1 overflow-hidden">
            {provider.models.length === 0 ? (
              <span className="text-[11px] text-zinc-400 dark:text-zinc-500">
                未配置模型
              </span>
            ) : (
              <>
                {provider.models.slice(0, 2).map((m) => (
                  <Badge
                    key={m}
                    variant="secondary"
                    className="font-mono text-[10px] font-normal truncate max-w-[160px] border border-zinc-200/60 dark:border-zinc-800"
                    title={m}
                  >
                    {m}
                  </Badge>
                ))}
                {provider.models.length > 2 && (
                  <Badge
                    variant="secondary"
                    className="font-mono text-[10px] font-normal text-zinc-400 dark:text-zinc-500"
                    title={`其他模型：${provider.models.slice(2).join(', ')}`}
                  >
                    +{provider.models.length - 2}
                  </Badge>
                )}
              </>
            )}
          </div>
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
    <Button
      type="button"
      variant="ghost"
      size="xs"
      onClick={copy}
    >
      {copied ? <Check className="w-3.5 h-3.5 text-emerald-500" /> : <Copy className="w-3.5 h-3.5" />}
      <span>{copied ? '已复制' : '复制地址'}</span>
    </Button>
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
        <div className="flex items-center justify-between gap-3 px-4 py-2.5 bg-muted/30 border-b border-border">
          <div className="flex items-center gap-2 min-w-0 text-xs">
            <span
              className={`w-1.5 h-1.5 rounded-full shrink-0 ${gatewayUp ? 'bg-emerald-500 shadow-[0_0_6px_rgba(16,185,129,0.5)]' : 'bg-rose-500'}`}
              aria-hidden
            />
            <span className="font-semibold text-zinc-700 dark:text-zinc-200 tracking-wide shrink-0">本地网关</span>
            <span className="font-mono text-zinc-400 dark:text-zinc-500 truncate select-text text-[11px]">
              {stats.host}:{stats.port}
            </span>
          </div>
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

        {/* 底部内嵌说明，去除卡片外孤立悬挂 */}
        <div className="px-4 py-2 bg-zinc-50/40 dark:bg-zinc-900/50 border-t border-zinc-100 dark:border-zinc-800/80 text-[11px] text-zinc-400 dark:text-zinc-500">
          {notice ? (
            <span
              className={
                notice.tone === 'fault'
                  ? 'text-rose-500 font-medium'
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
        </div>
      </section>

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

export function ClientsView() {
  const dataReady = useAppStore((state) => state.initialLoaded && !state.isLoading);
  if (!dataReady) return <LoadingState />;
  return (
    <div className="space-y-3 animate-in fade-in duration-150">
      <IngressCard />
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
      <section className={panelClass} aria-label="上游服务商">
        <div className="flex items-center justify-between gap-3 px-4 py-2.5 bg-muted/30 border-b border-border">
          <div className="flex items-center gap-1.5 text-xs">
            <span className="font-semibold text-zinc-800 dark:text-zinc-200 tracking-wide shrink-0">
              服务商
            </span>
            {providers.length > 0 && (
              <span className="font-mono text-muted-foreground text-[11px]">
                · 共 {providers.length} 个
              </span>
            )}
          </div>
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            onClick={() => openProviderEditor(null)}
            aria-label="添加服务商"
            title="添加服务商"
            className="text-muted-foreground hover:text-foreground"
          >
            <Plus className="w-3.5 h-3.5" />
          </Button>
        </div>

        {providers.length === 0 ? (
          <p className="px-4 py-8 text-center text-xs text-zinc-400">
            添加上游 API Key 和模型后，接管的请求会转发到这里。
          </p>
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
