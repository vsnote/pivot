import React, { useState } from 'react';
import { useShallow } from 'zustand/react/shallow';
import { useAppStore } from '@/store/useAppStore';
import { Provider, InterceptorStatus } from '@/types';
import { Switch } from '@/components/common/Switch';
import { ConfirmDialog } from '@/components/common/ConfirmDialog';
import {
  Plus,
  Trash2,
  Zap,
  LoaderCircle,
  Boxes,
  AlertTriangle,
} from 'lucide-react';

import {
  ProviderAvatar,
  OpenAISpiralIcon,
  ClaudeLogo,
  GeminiLogo,
} from '@/components/common/BrandIcons';

function ProviderCard({ provider }: { provider: Provider }) {
  const { openProviderEditor, deleteProvider, toggleProvider, testProviderLatency } =
    useAppStore(
      useShallow((state) => ({
        openProviderEditor: state.openProviderEditor,
        deleteProvider: state.deleteProvider,
        toggleProvider: state.toggleProvider,
        testProviderLatency: state.testProviderLatency,
      })),
    );
  const [pending, setPending] = useState<'test' | 'toggle' | 'delete' | null>(null);
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  // 仅在本次测试后展示毫秒，常态不展示历史毫秒
  const [testedLatency, setTestedLatency] = useState<number | null>(null);

  const latencyColor =
    testedLatency == null
      ? 'text-zinc-400 dark:text-zinc-500'
      : testedLatency < 0
        ? 'text-rose-500 dark:text-rose-400'
        : testedLatency < 300
          ? 'text-emerald-600 dark:text-emerald-400'
          : 'text-amber-500 dark:text-amber-400';

  const latencyText =
    testedLatency == null
      ? '测速'
      : testedLatency < 0
        ? '失败'
        : `${testedLatency} ms`;

  const runAction = async (action: NonNullable<typeof pending>, e?: React.MouseEvent) => {
    if (e) e.stopPropagation();
    if (pending) return;
    setPending(action);
    try {
      if (action === 'test') {
        const ms = await testProviderLatency(provider.id);
        setTestedLatency(ms);
      }
      if (action === 'toggle') {
        await toggleProvider(provider.id, !provider.enabled);
        if (provider.enabled) {
          setTestedLatency(null);
        }
      }
      if (action === 'delete') {
        await deleteProvider(provider.id);
        setConfirmingDelete(false);
      }
    } catch {
      if (action === 'test') setTestedLatency(-1);
    } finally {
      setPending(null);
    }
  };

  const handleRowClick = () => {
    if (pending !== null) return;
    openProviderEditor(provider);
  };

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={handleRowClick}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          handleRowClick();
        }
      }}
      className={`group flex items-center justify-between gap-3.5 px-4 py-3 sm:py-3.5 transition-colors cursor-pointer select-none ${
        provider.enabled
          ? 'hover:bg-zinc-50/80 dark:hover:bg-zinc-850/50'
          : 'opacity-60 bg-zinc-50/30 dark:bg-zinc-900/30 hover:bg-zinc-100/50 dark:hover:bg-zinc-850/40'
      }`}
      aria-label={`编辑服务商 ${provider.name}`}
      aria-busy={pending !== null}
    >
      {/* 左侧身份与服务商基本信息 */}
      <div className="flex items-center gap-3.5 min-w-0 flex-1">
        <ProviderAvatar
          name={provider.name}
          protocol={provider.protocol}
          iconKey={provider.presetId || undefined}
        />

        <div className="min-w-0 flex-1 flex items-center gap-2">
          <h3 className="text-[14px] font-semibold text-zinc-900 dark:text-zinc-100 truncate">
            {provider.name}
          </h3>
          <span className="text-[11px] text-zinc-400 dark:text-zinc-500 font-mono select-none">
            · {provider.models.length} 个模型
          </span>
        </div>
      </div>

      {/* 右侧：操作区与启用开关 */}
      <div className="flex items-center gap-1.5 shrink-0">
        {/* 删除按钮：hover 时显现，排在测速左侧，避免夹在毫秒与开关中间 */}
        <button
          type="button"
          disabled={pending !== null}
          onClick={(e) => {
            e.stopPropagation();
            setConfirmingDelete(true);
          }}
          className="p-1.5 rounded-lg text-zinc-400 hover:text-rose-600 dark:hover:text-rose-400 hover:bg-rose-50 dark:hover:bg-rose-950/40 transition-all cursor-pointer opacity-0 group-hover:opacity-100 focus:opacity-100"
          title="删除服务商"
        >
          <Trash2 className="w-3.5 h-3.5" />
        </button>

        {/* 测速按钮 / 测速结果：未测试时 hover 显现测速；测试后展示结果并可再次点击重新测速 */}
        {provider.enabled && (
          <div className="flex items-center">
            {pending === 'test' ? (
              <div className="flex items-center gap-1 px-2 py-1 rounded-lg text-xs font-mono text-zinc-400">
                <LoaderCircle className="w-3.5 h-3.5 animate-spin text-indigo-500" />
                <span>…</span>
              </div>
            ) : testedLatency !== null ? (
              <button
                type="button"
                onClick={(e) => runAction('test', e)}
                className={`flex items-center gap-1 px-2 py-1 rounded-lg text-xs font-mono font-medium hover:bg-zinc-100 dark:hover:bg-zinc-800 transition-colors cursor-pointer ${latencyColor}`}
                title="点击重新测速"
              >
                <Zap className="w-3.5 h-3.5" />
                <span>{latencyText}</span>
              </button>
            ) : (
              <button
                type="button"
                onClick={(e) => runAction('test', e)}
                className="flex items-center gap-1 px-2 py-1 rounded-lg text-xs font-medium text-zinc-500 hover:text-zinc-900 dark:text-zinc-400 dark:hover:text-zinc-100 hover:bg-zinc-100 dark:hover:bg-zinc-800 transition-all cursor-pointer opacity-0 group-hover:opacity-100 focus:opacity-100"
                title="测试延迟"
              >
                <Zap className="w-3.5 h-3.5 text-zinc-400 group-hover:text-amber-500" />
                <span>测速</span>
              </button>
            )}
          </div>
        )}

        {/* 开关：常态自然对齐，去除非 hover 时的空竖线 */}
        <div
          className="ml-1 flex items-center"
          onClick={(e) => e.stopPropagation()}
        >
          <Switch
            checked={provider.enabled}
            disabled={pending !== null}
            onCheckedChange={() => runAction('toggle')}
            ariaLabel="启用服务商"
          />
        </div>
      </div>

      <ConfirmDialog
        open={confirmingDelete}
        title={`删除服务商“${provider.name}”？`}
        details={[
          '删除后需重新配置。已有请求日志不受影响。',
          ...(provider.isDefault ? ['该服务商当前是默认兜底服务商。'] : []),
        ]}
        confirmLabel="删除"
        busy={pending === 'delete'}
        onConfirm={() => runAction('delete')}
        onCancel={() => setConfirmingDelete(false)}
      />
    </div>
  );
}

const DEFAULT_INTERCEPTORS: InterceptorStatus[] = [
  {
    id: 'claude_code',
    name: 'Claude Code',
    installed: true,
    configPath: '~/.claude.json',
    isIntercepted: false,
  },
  {
    id: 'codex_cli',
    name: 'Codex CLI',
    installed: true,
    configPath: '~/.codex/config.json',
    isIntercepted: false,
  },
  {
    id: 'gemini_cli',
    name: 'Gemini CLI',
    installed: false,
    configPath: '~/.gemini/config.json',
    isIntercepted: false,
  },
];

function GatewayQuickBar() {
  const { interceptors, toggleInterceptor } = useAppStore(
    useShallow((state) => ({
      interceptors: state.interceptors,
      toggleInterceptor: state.toggleInterceptor,
    })),
  );

  const [togglingId, setTogglingId] = useState<string | null>(null);
  const [conflictConfirmTarget, setConflictConfirmTarget] = useState<InterceptorStatus | null>(null);
  const clientList = interceptors.length > 0 ? interceptors : DEFAULT_INTERCEPTORS;

  const doToggle = async (id: 'claude_code' | 'codex_cli' | 'gemini_cli', enable: boolean) => {
    if (togglingId) return;
    setTogglingId(id);
    try {
      await toggleInterceptor(id, enable);
    } catch {
      // 全局状态管理处理错误
    } finally {
      setTogglingId(null);
    }
  };

  const handleToggleClient = async (
    client: InterceptorStatus,
    e: React.MouseEvent,
  ) => {
    e.stopPropagation();
    if (togglingId) return;

    // 若尝试开启，且检测到第三方管理工具冲突（如 CC Switch），先弹窗提醒用户确认
    if (!client.isIntercepted && client.conflict) {
      setConflictConfirmTarget(client);
      return;
    }

    await doToggle(client.id, !client.isIntercepted);
  };

  // 收集当前已开启接管、且仍存在配置冲突的客户端
  const activeConflicts = clientList.filter((c) => c.isIntercepted && c.conflict);

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between gap-3 px-3.5 py-2.5 rounded-2xl border border-zinc-200/80 dark:border-zinc-800/80 bg-white dark:bg-zinc-900 shadow-xs">
        {/* CLI / Agent 自动代理胶囊 */}
        <div className="flex items-center gap-3 flex-wrap">
          <span className="text-xs font-medium text-zinc-400 dark:text-zinc-500 shrink-0 select-none">
            Agent 代理
          </span>

          <div className="flex items-center gap-1.5 flex-wrap">
            {clientList.map((client) => {
              const isBusy = togglingId === client.id;
              const active = client.isIntercepted;

              let icon = <ClaudeLogo className="w-3.5 h-3.5" />;
              let label = 'Claude';
              let activeClass =
                'bg-amber-500/10 text-amber-600 dark:text-amber-400 border-amber-500/30';

              if (client.id === 'codex_cli') {
                icon = <OpenAISpiralIcon className="w-3.5 h-3.5" />;
                label = 'Codex';
                activeClass =
                  'bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 border-emerald-500/30';
              } else if (client.id === 'gemini_cli') {
                icon = <GeminiLogo className="w-3.5 h-3.5" />;
                label = 'Gemini';
                activeClass =
                  'bg-blue-500/10 text-blue-600 dark:text-blue-400 border-blue-500/30';
              }

              const title = `${client.name}: ${
                active ? '已接管至本地网关 (点击恢复官方配置)' : '未接管 (点击一键配置代理)'
              }${client.conflict ? `\n[冲突提示] ${client.conflict}` : ''}`;

              return (
                <button
                  key={client.id}
                  type="button"
                  disabled={isBusy}
                  onClick={(e) => handleToggleClient(client, e)}
                  className={`relative flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl text-xs font-medium border transition-all duration-150 cursor-pointer ${
                    active
                      ? `${activeClass} shadow-2xs`
                      : 'bg-zinc-100/70 dark:bg-zinc-800/60 text-zinc-400 dark:text-zinc-500 border-transparent hover:text-zinc-700 dark:hover:text-zinc-300 hover:bg-zinc-200/50 dark:hover:bg-zinc-800'
                  }`}
                  title={title}
                >
                  {isBusy ? (
                    <LoaderCircle className="w-3.5 h-3.5 animate-spin" />
                  ) : (
                    icon
                  )}
                  <span className="text-[11px] font-mono">
                    {label}
                  </span>
                  {client.conflict && (
                    <span
                      title={client.conflict}
                      className="flex items-center text-amber-500 dark:text-amber-400"
                    >
                      <AlertTriangle className="w-3 h-3" />
                    </span>
                  )}
                  {active && (
                    <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 ring-2 ring-white dark:ring-zinc-900" />
                  )}
                </button>
              );
            })}
          </div>
        </div>
      </div>

      {/* 已接管状态下的冲突提示横幅 */}
      {activeConflicts.map((c) => (
        <div
          key={c.id}
          className="flex items-start gap-2 p-2.5 rounded-xl bg-amber-50/90 dark:bg-amber-950/30 border border-amber-200/70 dark:border-amber-900/50 text-xs text-amber-800 dark:text-amber-300 animate-in fade-in"
        >
          <AlertTriangle className="w-4 h-4 text-amber-500 shrink-0 mt-0.5" />
          <div className="leading-relaxed">
            <span className="font-semibold">{c.name}：</span>
            {c.conflict}
          </div>
        </div>
      ))}

      {/* 开启前的冲突拦截确认弹窗 */}
      <ConfirmDialog
        open={conflictConfirmTarget !== null}
        title={`开启 ${conflictConfirmTarget?.name} 代理提示`}
        details={[
          conflictConfirmTarget?.conflict || '检测到配置冲突，开启可能影响现有配置。',
        ]}
        confirmLabel="仍然开启"
        variant="warning"
        busy={togglingId !== null}
        onConfirm={async () => {
          if (!conflictConfirmTarget) return;
          const target = conflictConfirmTarget;
          setConflictConfirmTarget(null);
          await doToggle(target.id, true);
        }}
        onCancel={() => setConflictConfirmTarget(null)}
      />
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

  return (
    <div className="space-y-4 animate-in fade-in duration-150">
      {/* 统一网关入口与客户端快捷接管单行栏 */}
      <GatewayQuickBar />

      {!dataReady ? (
        <div className="p-12 text-center rounded-2xl border border-zinc-200/80 dark:border-zinc-800 bg-white dark:bg-zinc-900">
          <LoaderCircle className="w-6 h-6 text-indigo-600 dark:text-indigo-400 animate-spin mx-auto mb-2" />
          <p className="text-xs text-zinc-400">载入配置中…</p>
        </div>
      ) : providers.length === 0 ? (
        <div className="rounded-2xl border border-zinc-200/80 dark:border-zinc-800 bg-white dark:bg-zinc-900 p-8 text-center shadow-xs">
          <div className="w-12 h-12 rounded-2xl bg-indigo-50 dark:bg-indigo-950/60 text-indigo-600 dark:text-indigo-400 flex items-center justify-center mx-auto mb-3">
            <Boxes className="w-6 h-6" />
          </div>
          <h3 className="text-sm font-bold text-zinc-950 dark:text-zinc-100">
            暂无已配置的服务商
          </h3>
          <p className="text-xs text-zinc-400 mt-1 max-w-sm mx-auto">
            添加上游 API Key 与可用模型，即可通过 OmniSwitch 本地分发。
          </p>
          <button
            type="button"
            onClick={() => openProviderEditor(null)}
            className="mt-4 inline-flex items-center gap-1.5 px-4 py-2 rounded-xl text-xs font-medium text-white bg-indigo-600 hover:bg-indigo-700 shadow-xs cursor-pointer"
          >
            <Plus className="w-4 h-4" />
            <span>添加服务商</span>
          </button>
        </div>
      ) : (
        <div className="rounded-2xl border border-zinc-200/80 dark:border-zinc-800 bg-white dark:bg-zinc-900 shadow-xs divide-y divide-zinc-100 dark:divide-zinc-800/80 overflow-hidden">
          {providers.map((provider) => (
            <ProviderCard key={provider.id} provider={provider} />
          ))}
        </div>
      )}
    </div>
  );
}
