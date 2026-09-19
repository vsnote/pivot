import React, { useState, useRef, useEffect } from 'react';
import { useShallow } from 'zustand/react/shallow';
import { useAppStore } from '@/store/useAppStore';
import { Provider, InterceptorStatus } from '@/types';
import { Switch } from '@/components/common/Switch';
import { ConfirmDialog } from '@/components/common/ConfirmDialog';
import {
  Plus,
  Trash2,
  Pencil,
  Zap,
  Server,
  Network,
  LoaderCircle,
  Sparkles,
  Boxes,
  Copy,
  Check,
  ChevronDown,
  HelpCircle,
} from 'lucide-react';

const OpenAISpiralIcon: React.FC<{ className?: string }> = ({ className = 'w-4 h-4' }) => (
  <svg viewBox="0 0 24 24" fill="currentColor" className={className}>
    <path d="M22.2819 9.8211a5.9847 5.9847 0 0 0-.5157-4.9108 6.0462 6.0462 0 0 0-6.5098-2.9A6.0651 6.0651 0 0 0 4.9807 4.1818a5.9847 5.9847 0 0 0-3.9977 2.9 6.0462 6.0462 0 0 0 .7427 7.0966 5.98 5.98 0 0 0 .511 4.9107 6.051 6.051 0 0 0 6.5146 2.9001A5.9847 5.9847 0 0 0 13.2599 24a6.0557 6.0557 0 0 0 5.7718-4.2058 5.9894 5.9894 0 0 0 3.9977-2.9001 6.0557 6.0557 0 0 0-.7475-7.0729zm-9.022 12.6081a4.4755 4.4755 0 0 1-2.8764-1.0408l.1419-.0804 4.7783-2.7582a.7948.7948 0 0 0 .3927-.6813v-6.7369l2.02 1.1686a.071.071 0 0 1 .038.052v5.5826a4.5045 4.5045 0 0 1-4.4945 4.4944zm-9.6607-4.1254a4.4708 4.4708 0 0 1-.5346-3.0137l.142.0852 4.783 2.7582a.7712.7712 0 0 0 .7806 0l5.8428-3.3685v2.3324a.0804.0804 0 0 1-.0332.0615L9.74 19.9502a4.4992 4.4992 0 0 1-6.1408-1.6464zM2.3408 7.8956a4.485 4.485 0 0 1 2.3655-1.9728V11.6a.7664.7664 0 0 0 .3879.6765l5.8144 3.3543-2.0201 1.1685a.0757.0757 0 0 1-.071 0l-4.8303-2.7865A4.504 4.504 0 0 1 2.3408 7.872zm16.5963 3.8558L13.1038 8.364 15.1192 7.2a.0757.0757 0 0 1 .071 0l4.8303 2.7913a4.4944 4.4944 0 0 1-.6765 8.1042v-5.6772a.79.79 0 0 0-.407-.6667zm2.0107-3.0231l-.142-.0852-4.7735-2.7818a.7759.7759 0 0 0-.7854 0L9.409 9.2297V6.8974a.0662.0662 0 0 1 .0284-.0615l4.8303-2.7866a4.4992 4.4992 0 0 1 6.6802 4.6602zm-12.6401 4.135l-2.02-1.1639a.0804.0804 0 0 1-.038-.0567V6.0748a4.4992 4.4992 0 0 1 7.3757-3.4537l-.142.0805L6.704 5.4598a.7948.7948 0 0 0-.3927.6813zm1.0976-2.3654l2.602-1.4998 2.6069 1.4998v2.9994l-2.6069 1.4997-2.602-1.4997z"/>
  </svg>
);

const ClaudeLogo: React.FC<{ className?: string }> = ({ className = 'w-4 h-4' }) => (
  <svg viewBox="0 0 24 24" fill="currentColor" className={className}>
    <path d="m4.7144 15.9555 4.7174-2.6471.079-.2307-.079-.1275h-.2307l-.7893-.0486-2.6956-.0729-2.3375-.0971-2.2646-.1214-.5707-.1215-.5343-.7042.0546-.3522.4797-.3218.686.0608 1.5179.1032 2.2767.1578 1.6514.0972 2.4468.255h.3886l.0546-.1579-.1336-.0971-.1032-.0972L6.973 9.8356l-2.55-1.6879-1.3356-.9714-.7225-.4918-.3643-.4614-.1578-1.0078.6557-.7225.8803.0607.2246.0607.8925.686 1.9064 1.4754 2.4893 1.8336.3643.3035.1457-.1032.0182-.0728-.164-.2733-1.3539-2.4467-1.445-2.4893-.6435-1.032-.17-.6194c-.0607-.255-.1032-.4674-.1032-.7285L6.287.1335 6.6997 0l.9957.1336.419.3642.6192 1.4147 1.0018 2.2282 1.5543 3.0296.4553.8985.2429.8318.091.255h.1579v-.1457l.1275-1.706.2368-2.0947.2307-2.6957.0789-.7589.3764-.9107.7468-.4918.5828.2793.4797.686-.0668.4433-.2853 1.8517-.5586 2.9021-.3643 1.9429h.2125l.2429-.2429.9835-1.3053 1.6514-2.0643.7286-.8196.85-.9046.5464-.4311h1.0321l.759 1.1293-.34 1.1657-1.0625 1.3478-.8804 1.1414-1.2628 1.7-.7893 1.36.0729.1093.1882-.0183 2.8535-.607 1.5421-.2794 1.8396-.3157.8318.3886.091.3946-.3278.8075-1.967.4857-2.3072.4614-3.4364.8136-.0425.0304.0486.0607 1.5482.1457.6618.0364h1.621l3.0175.2247.7892.522.4736.6376-.079.4857-1.2142.6193-1.6393-.3886-3.825-.9107-1.3113-.3279h-.1822v.1093l1.0929 1.0686 2.0035 1.8092 2.5075 2.3314.1275.5768-.3218.4554-.34-.0486-2.2039-1.6575-.85-.7468-1.9246-1.621h-.1275v.17l.4432.6496 2.3436 3.5214.1214 1.0807-.17.3521-.6071.2125-.6679-.1214-1.3721-1.9246L14.38 17.959l-1.1414-1.9428-.1397.079-.674 7.2552-.3156.3703-.7286.2793-.6071-.4614-.3218-.7468.3218-1.4753.3886-1.9246.3157-1.53.2853-1.9004.17-.6314-.0121-.0425-.1397.0182-1.4328 1.9672-2.1796 2.9446-1.7243 1.8456-.4128.164-.7164-.3704.0667-.6618.4008-.5889 2.386-3.0357 1.4389-1.882.929-1.0868-.0062-.1579h-.0546l-6.3385 4.1164-1.1293.1457-.4857-.4554.0608-.7467.2307-.2429 1.9064-1.3114Z"/>
  </svg>
);

const AnthropicLogo: React.FC<{ className?: string }> = ({ className = 'w-4 h-4' }) => (
  <svg viewBox="0 0 24 24" fill="currentColor" className={className} aria-hidden="true">
    <path d="M17.3041 3.541h-3.6718l6.696 16.918H24Zm-10.6082 0L0 20.459h3.7442l1.3693-3.5527h7.0052l1.3693 3.5528h3.7442L10.5363 3.5409Zm-.3712 10.2232 2.2914-5.9456 2.2914 5.9456Z" />
  </svg>
);

const GeminiLogo: React.FC<{ className?: string }> = ({ className = 'w-4 h-4' }) => (
  <svg viewBox="0 0 24 24" fill="currentColor" className={className}>
    <path d="M11.04 19.32Q12 21.51 12 24q0-2.49.93-4.68.96-2.19 2.58-3.81t3.81-2.55Q21.51 12 24 12q-2.49 0-4.68-.93a12.3 12.3 0 0 1-3.81-2.58 12.3 12.3 0 0 1-2.58-3.81Q12 2.49 12 0q0 2.49-.96 4.68-.93 2.19-2.55 3.81a12.3 12.3 0 0 1-3.81 2.58Q2.49 12 0 12q2.49 0 4.68.96 2.19.93 3.81 2.55t2.55 3.81"/>
  </svg>
);

function getProtocolBadgeColor(protocol: string) {
  switch (protocol) {
    case 'anthropic':
      return 'bg-amber-50 dark:bg-amber-950/40 text-amber-700 dark:text-amber-300 border-amber-200/60 dark:border-amber-800/50';
    case 'openai':
      return 'bg-emerald-50 dark:bg-emerald-950/40 text-emerald-700 dark:text-emerald-300 border-emerald-200/60 dark:border-emerald-800/50';
    case 'gemini':
      return 'bg-blue-50 dark:bg-blue-950/40 text-blue-700 dark:text-blue-300 border-blue-200/60 dark:border-blue-800/50';
    case 'openrouter':
      return 'bg-violet-50 dark:bg-violet-950/40 text-violet-700 dark:text-violet-300 border-violet-200/60 dark:border-violet-800/50';
    case 'ollama':
      return 'bg-zinc-100 dark:bg-zinc-800/60 text-zinc-700 dark:text-zinc-300 border-zinc-200 dark:border-zinc-700';
    default:
      return 'bg-zinc-100 dark:bg-zinc-800/60 text-zinc-700 dark:text-zinc-300 border-zinc-200 dark:border-zinc-700';
  }
}

function ProviderCard({ provider }: { provider: Provider }) {
  const { openProviderEditor, deleteProvider, toggleProvider, testProviderLatency, routes } =
    useAppStore(
      useShallow((state) => ({
        openProviderEditor: state.openProviderEditor,
        deleteProvider: state.deleteProvider,
        toggleProvider: state.toggleProvider,
        testProviderLatency: state.testProviderLatency,
        routes: state.routes,
      })),
    );
  const [pending, setPending] = useState<'test' | 'toggle' | 'delete' | null>(null);
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const targetRules = routes.filter((route) => route.targetProviderId === provider.id);
  const fallbackRules = routes.filter((route) => route.fallbackProviderIds.includes(provider.id));
  const { latencyMs } = provider;

  const latencyColor =
    latencyMs == null
      ? 'text-zinc-400 dark:text-zinc-500'
      : latencyMs < 0
        ? 'text-rose-500 dark:text-rose-400'
        : latencyMs < 300
          ? 'text-emerald-600 dark:text-emerald-400'
          : 'text-amber-500 dark:text-amber-400';

  const latencyText =
    latencyMs == null
      ? '测试'
      : latencyMs < 0
        ? '失败'
        : `${latencyMs} ms`;

  const runAction = async (action: NonNullable<typeof pending>) => {
    if (pending) return;
    setPending(action);
    try {
      if (action === 'test') await testProviderLatency(provider.id);
      if (action === 'toggle') await toggleProvider(provider.id, !provider.enabled);
      if (action === 'delete') {
        await deleteProvider(provider.id);
        setConfirmingDelete(false);
      }
    } catch {
      // 全局错误展示
    } finally {
      setPending(null);
    }
  };

  return (
    <article
      className={`rounded-2xl border transition-all duration-150 p-3.5 sm:p-4 bg-white dark:bg-zinc-900 shadow-xs ${
        provider.enabled
          ? 'border-zinc-200/80 dark:border-zinc-800/80 hover:border-zinc-300 dark:hover:border-zinc-700'
          : 'border-zinc-200/50 dark:border-zinc-800/50 opacity-60 bg-zinc-50/40 dark:bg-zinc-900/40'
      }`}
      aria-label={provider.name}
      aria-busy={pending !== null}
    >
      <div className="flex items-center justify-between gap-3.5">
        {/* 左侧身份与服务商基本信息 */}
        <div className="flex items-center gap-3.5 min-w-0 flex-1">
          <div
            className={`w-9 h-9 rounded-xl flex items-center justify-center shrink-0 border ${
              provider.protocol === 'ollama'
                ? 'bg-zinc-100 dark:bg-zinc-800 border-zinc-200 dark:border-zinc-700 text-zinc-700 dark:text-zinc-200'
                : 'bg-indigo-50 dark:bg-indigo-950/40 border-indigo-200/50 dark:border-indigo-800/40 text-indigo-600 dark:text-indigo-400'
            }`}
          >
            {provider.protocol === 'ollama' ? <Server className="w-4 h-4" /> : <Network className="w-4 h-4" />}
          </div>

          <div className="min-w-0 flex-1 flex flex-wrap items-center gap-2">
            <h3 className="text-[14px] font-bold text-zinc-950 dark:text-zinc-50 truncate">
              {provider.name}
            </h3>
            <span
              className={`text-[10px] uppercase font-mono px-2 py-0.5 rounded-md font-semibold border ${getProtocolBadgeColor(
                provider.protocol,
              )}`}
            >
              {provider.protocol}
            </span>
            {provider.isDefault && (
              <span
                className="text-[10.5px] px-2 py-0.5 rounded-md font-medium bg-amber-50 dark:bg-amber-950/50 text-amber-700 dark:text-amber-300 border border-amber-200/60 dark:border-amber-800/50 flex items-center gap-1"
                title={provider.defaultModel ? `兜底模型: ${provider.defaultModel}` : '默认服务商'}
              >
                <Sparkles className="w-3 h-3" />
                {provider.defaultModel ? `默认 · ${provider.defaultModel}` : '默认'}
              </span>
            )}
            <span className="text-[11px] text-zinc-400 dark:text-zinc-500 font-mono">
              {provider.models.length} 个模型
            </span>
          </div>
        </div>

        {/* 右侧：测速与操作 */}
        <div className="flex items-center gap-2 shrink-0">
          {provider.enabled ? (
            <button
              type="button"
              onClick={() => runAction('test')}
              disabled={pending !== null}
              className={`flex items-center gap-1 px-2 py-1 rounded-lg text-xs font-mono font-medium hover:bg-zinc-100 dark:hover:bg-zinc-800 transition-colors cursor-pointer ${latencyColor}`}
              title="测试延迟"
            >
              {pending === 'test' ? (
                <LoaderCircle className="w-3.5 h-3.5 animate-spin" />
              ) : (
                <Zap className="w-3.5 h-3.5" />
              )}
              <span>{pending === 'test' ? '…' : latencyText}</span>
            </button>
          ) : (
            <span className="text-xs text-zinc-400 font-mono px-2 py-1">未启用</span>
          )}

          <div className="w-[1px] h-3.5 bg-zinc-200 dark:bg-zinc-800" />

          {/* 编辑与删除 */}
          <button
            type="button"
            disabled={pending !== null}
            onClick={() => openProviderEditor(provider)}
            className="p-1.5 rounded-lg text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100 hover:bg-zinc-100 dark:hover:bg-zinc-800 transition-colors cursor-pointer"
            title="编辑服务商"
          >
            <Pencil className="w-3.5 h-3.5" />
          </button>

          <button
            type="button"
            disabled={pending !== null}
            onClick={() => setConfirmingDelete(true)}
            className="p-1.5 rounded-lg text-zinc-400 hover:text-rose-600 dark:hover:text-rose-400 hover:bg-rose-50 dark:hover:bg-rose-950/40 transition-colors cursor-pointer"
            title="删除服务商"
          >
            <Trash2 className="w-3.5 h-3.5" />
          </button>

          <div className="ml-1 pl-2 border-l border-zinc-200 dark:border-zinc-800 flex items-center">
            <Switch
              checked={provider.enabled}
              disabled={pending !== null}
              onCheckedChange={() => runAction('toggle')}
              ariaLabel="启用服务商"
            />
          </div>
        </div>
      </div>

      <ConfirmDialog
        open={confirmingDelete}
        title={`删除服务商“${provider.name}”？`}
        details={[
          '删除后需重新配置。已有请求日志不受影响。',
          ...(provider.isDefault ? ['该服务商当前是默认兜底服务商。'] : []),
          ...(targetRules.length ? [`将同步删除 ${targetRules.length} 条关联路由规则。`] : []),
          ...(fallbackRules.length ? [`${fallbackRules.length} 条备选链将移除它。`] : []),
        ]}
        confirmLabel="删除"
        busy={pending === 'delete'}
        onConfirm={() => runAction('delete')}
        onCancel={() => setConfirmingDelete(false)}
      />
    </article>
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
  const { stats, interceptors, toggleInterceptor } = useAppStore(
    useShallow((state) => ({
      stats: state.stats,
      interceptors: state.interceptors,
      toggleInterceptor: state.toggleInterceptor,
    })),
  );

  const [dropdownOpen, setDropdownOpen] = useState(false);
  const [copiedKey, setCopiedKey] = useState<string | null>(null);
  const [togglingId, setTogglingId] = useState<string | null>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const port = stats.port || 16888;
  const openaiUrl = `http://127.0.0.1:${port}/v1`;
  const anthropicUrl = `http://127.0.0.1:${port}`;

  const clientList = interceptors.length > 0 ? interceptors : DEFAULT_INTERCEPTORS;

  const handleMouseEnter = () => {
    if (timerRef.current) clearTimeout(timerRef.current);
    setDropdownOpen(true);
  };

  const handleMouseLeave = () => {
    timerRef.current = setTimeout(() => {
      setDropdownOpen(false);
    }, 200);
  };

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
        setDropdownOpen(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, []);

  const copyToClipboard = (key: string, text: string, e?: React.MouseEvent) => {
    if (e) e.stopPropagation();
    navigator.clipboard.writeText(text);
    setCopiedKey(key);
    setTimeout(() => {
      setCopiedKey((curr) => (curr === key ? null : curr));
    }, 1800);
  };

  const handleToggleClient = async (
    id: 'claude_code' | 'codex_cli' | 'gemini_cli',
    currentStatus: boolean,
    e: React.MouseEvent,
  ) => {
    e.stopPropagation();
    if (togglingId) return;
    setTogglingId(id);
    try {
      await toggleInterceptor(id, !currentStatus);
    } catch {
      // 全局状态管理处理错误
    } finally {
      setTogglingId(null);
    }
  };

  return (
    <div className="relative z-30 flex items-center justify-between gap-3 px-3.5 py-2.5 rounded-2xl border border-zinc-200/80 dark:border-zinc-800/80 bg-white dark:bg-zinc-900 shadow-xs">
      {/* 左侧：网关地址胶囊与一体化展开面板 */}
      <div
        ref={dropdownRef}
        onMouseEnter={handleMouseEnter}
        onMouseLeave={handleMouseLeave}
        className="relative"
      >
          {/* 默认触发器胶囊 */}
          <div
            onClick={() => setDropdownOpen((prev) => !prev)}
            className={`flex items-center gap-2 px-2.5 py-1.5 rounded-xl border transition-all cursor-pointer select-none group ${
              dropdownOpen
                ? 'bg-transparent border-transparent opacity-0 pointer-events-none'
                : 'bg-zinc-50 dark:bg-zinc-800/60 hover:bg-zinc-100 dark:hover:bg-zinc-800 border-zinc-200/60 dark:border-zinc-750'
            }`}
          >
            <div className="w-5 h-5 rounded-md bg-emerald-500/10 dark:bg-emerald-500/20 text-emerald-600 dark:text-emerald-400 flex items-center justify-center shrink-0">
              <OpenAISpiralIcon className="w-3.5 h-3.5" />
            </div>

            <span className="font-mono text-xs font-semibold text-zinc-800 dark:text-zinc-200">
              {openaiUrl}
            </span>

            {/* 快速复制当前地址 */}
            <button
              type="button"
              onClick={(e) => copyToClipboard('openai', openaiUrl, e)}
              className="p-1 rounded-md text-zinc-400 hover:text-zinc-700 dark:hover:text-zinc-200 hover:bg-zinc-200/60 dark:hover:bg-zinc-700 transition-colors cursor-pointer"
              title="复制网关地址"
            >
              {copiedKey === 'openai' ? (
                <Check className="w-3 h-3 text-emerald-500" />
              ) : (
                <Copy className="w-3 h-3" />
              )}
            </button>

            {/* 下拉展开指示 */}
            <ChevronDown className="w-3.5 h-3.5 text-zinc-400 transition-transform duration-200" />
          </div>

          {/* 当展开时：一体化的卡片面板（包含两行网关，由真实 Icon 识别，无多余字样与按钮） */}
          {dropdownOpen && (
            <div className="absolute top-0 left-0 min-w-full rounded-xl bg-white dark:bg-zinc-900 border border-zinc-200 dark:border-zinc-700 shadow-xl shadow-zinc-950/10 dark:shadow-zinc-950/50 z-50 overflow-hidden animate-in fade-in duration-100 select-none">
              {/* 行 1：OpenAI 网关 */}
              <div
                onClick={(e) => copyToClipboard('openai', openaiUrl, e)}
                className="flex items-center justify-between gap-4 px-2.5 py-1.5 bg-zinc-50/80 dark:bg-zinc-850/70 hover:bg-zinc-100/80 dark:hover:bg-zinc-800 transition-colors cursor-pointer group"
                title="点击复制地址"
              >
                <div className="flex items-center gap-2">
                  <div className="w-5 h-5 rounded-md bg-emerald-500/10 dark:bg-emerald-500/20 text-emerald-600 dark:text-emerald-400 flex items-center justify-center shrink-0">
                    <OpenAISpiralIcon className="w-3.5 h-3.5" />
                  </div>

                  <span className="font-mono text-xs font-semibold text-zinc-900 dark:text-zinc-100">
                    {openaiUrl}
                  </span>
                </div>

                <div className="flex items-center gap-1.5">
                  <button
                    type="button"
                    onClick={(e) => copyToClipboard('openai', openaiUrl, e)}
                    className="p-1 rounded-md text-zinc-400 hover:text-zinc-700 dark:hover:text-zinc-200 hover:bg-zinc-200/60 dark:hover:bg-zinc-700 transition-colors cursor-pointer"
                    title="复制地址"
                  >
                    {copiedKey === 'openai' ? (
                      <Check className="w-3 h-3 text-emerald-500" />
                    ) : (
                      <Copy className="w-3 h-3" />
                    )}
                  </button>

                  <ChevronDown className="w-3.5 h-3.5 text-zinc-500 rotate-180" />
                </div>
              </div>

              {/* 分割线 */}
              <div className="border-t border-zinc-100 dark:border-zinc-800" />

              {/* 行 2：Anthropic 协议网关 */}
              <div
                onClick={(e) => copyToClipboard('anthropic', anthropicUrl, e)}
                className="flex items-center justify-between gap-4 px-2.5 py-1.5 hover:bg-zinc-50 dark:hover:bg-zinc-800/80 transition-colors cursor-pointer group"
                title="点击复制地址"
              >
                <div className="flex items-center gap-2">
                  <div className="w-5 h-5 rounded-md bg-zinc-900/10 dark:bg-zinc-100/10 text-zinc-800 dark:text-zinc-200 flex items-center justify-center shrink-0">
                    <AnthropicLogo className="w-3.5 h-3.5" />
                  </div>

                  <span className="font-mono text-xs font-semibold text-zinc-700 dark:text-zinc-300 group-hover:text-zinc-950 dark:group-hover:text-zinc-100">
                    {anthropicUrl}
                  </span>
                </div>

                <div className="flex items-center gap-1.5">
                  <button
                    type="button"
                    onClick={(e) => copyToClipboard('anthropic', anthropicUrl, e)}
                    className="p-1 rounded-md text-zinc-400 hover:text-zinc-700 dark:hover:text-zinc-200 hover:bg-zinc-200/60 dark:hover:bg-zinc-700 transition-colors cursor-pointer"
                    title="复制地址"
                  >
                    {copiedKey === 'anthropic' ? (
                      <Check className="w-3 h-3 text-emerald-500" />
                    ) : (
                      <Copy className="w-3 h-3" />
                    )}
                  </button>

                  {/* 占位以保证复制按钮与第一行垂直对齐 */}
                  <div className="w-3.5" />
                </div>
              </div>
            </div>
          )}
        </div>

      {/* 右侧：CLI 自动代理（含真实 Logo 与帮助提示） */}
      <div className="flex items-center gap-2 shrink-0">
        <div className="flex items-center gap-1">
          <span className="text-[11px] text-zinc-400 font-medium hidden sm:inline">
            CLI 自动代理:
          </span>
          {/* 问号帮助说明，消除“一个都不开网关是否可用”的疑虑 */}
          <div className="relative group/help">
            <HelpCircle className="w-3.5 h-3.5 text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-300 cursor-pointer" />
            <div className="absolute right-0 top-full mt-2 hidden group-hover/help:block w-72 p-3 rounded-xl bg-zinc-900/95 dark:bg-zinc-950/95 text-zinc-100 text-[11px] leading-relaxed backdrop-blur-md shadow-xl z-50 pointer-events-none border border-zinc-700/60 dark:border-zinc-800">
              <div className="absolute -top-1 right-1.5 w-2 h-2 bg-zinc-900 dark:bg-zinc-950 border-t border-l border-zinc-700/60 dark:border-zinc-800 rotate-45" />
              💡 统一网关常驻运行中，Cursor、Windsurf、Aider 等应用可直接使用左侧地址。此处的开关仅用于自动配置本机命令行（全部关闭亦不影响网关正常工作）。
            </div>
          </div>
        </div>

        <div className="flex items-center gap-1.5">
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
            }`;

            return (
              <button
                key={client.id}
                type="button"
                disabled={isBusy}
                onClick={(e) => handleToggleClient(client.id, active, e)}
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
                {active && (
                  <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 ring-2 ring-white dark:ring-zinc-900" />
                )}
              </button>
            );
          })}
        </div>
      </div>
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
        <div className="space-y-3">
          {providers.map((provider) => (
            <ProviderCard key={provider.id} provider={provider} />
          ))}
        </div>
      )}
    </div>
  );
}
