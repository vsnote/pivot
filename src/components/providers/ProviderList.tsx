import { useState } from 'react';
import { useShallow } from 'zustand/react/shallow';
import { useAppStore } from '@/store/useAppStore';
import { Provider } from '@/types';
import { Switch } from '@/components/common/Switch';
import { CopyButton } from '@/components/common/CopyButton';
import { Library, Trash2, Pencil, Zap, KeyRound, Server, Network, LoaderCircle } from 'lucide-react';

function ProviderRow({ provider }: { provider: Provider }) {
  const { openProviderModal, deleteProvider, toggleProvider, testProviderLatency } = useAppStore(
    useShallow((state) => ({
      openProviderModal: state.openProviderModal,
      deleteProvider: state.deleteProvider,
      toggleProvider: state.toggleProvider,
      testProviderLatency: state.testProviderLatency,
    })),
  );
  const [pending, setPending] = useState<'test' | 'toggle' | 'delete' | null>(null);
  const { latencyMs } = provider;
  const latencyStatus = latencyMs == null ? 'idle' : latencyMs < 0 ? 'error' : latencyMs < 300 ? 'good' : 'slow';
  const latencyLabel = latencyMs == null ? '测试延迟' : latencyMs < 0 ? '连接失败' : `${latencyMs} ms`;

  const runAction = async (action: NonNullable<typeof pending>) => {
    if (pending) return;
    if (action === 'delete' && !window.confirm(`确定删除服务商“${provider.name}”吗？此操作无法撤销。`)) return;
    setPending(action);
    try {
      if (action === 'test') await testProviderLatency(provider.id);
      if (action === 'toggle') await toggleProvider(provider.id, !provider.enabled);
      if (action === 'delete') await deleteProvider(provider.id);
    } catch {
      // Store 将失败原因显示在全局错误提示中，列表保持原状态。
    } finally {
      setPending(null);
    }
  };

  return (
    <article className="provider-row" aria-label={provider.name} aria-busy={pending !== null}>
      <div className="provider-identity">
        <div className="provider-icon" aria-hidden="true">{provider.protocol === 'ollama' ? <Server size={17} /> : <Network size={17} />}</div>
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <h2 className="provider-name">{provider.name}</h2>
            <span className="badge mono uppercase">{provider.protocol}</span>
          </div>
          <div className="provider-endpoint">
            <code className="mono" title={provider.baseUrl}>{provider.baseUrl}</code>
            <CopyButton text={provider.baseUrl} label={`复制 ${provider.name} 接口地址`} />
          </div>

        </div>
      </div>
      {provider.enabled ? (
        <button type="button" className="latency-button" data-status={latencyStatus} onClick={() => runAction('test')} disabled={pending !== null} title="测试接口网络延迟，不验证密钥或模型可用性" aria-label={`测试 ${provider.name} 网络延迟`}>
          {pending === 'test' ? <LoaderCircle size={13} className="animate-spin" /> : <Zap size={13} />}
          <span className="mono">{pending === 'test' ? '测试中…' : latencyLabel}</span>
        </button>
      ) : <span className="latency-button mono" aria-label="无网络延迟数据">—</span>}
      <div className="provider-actions">
        <button type="button" className="icon-button" disabled={pending !== null} onClick={() => openProviderModal(provider)} aria-label={`编辑服务商 ${provider.name}`} title="编辑服务商"><Pencil size={14} /></button>
        <button type="button" className="icon-button danger" disabled={pending !== null} onClick={() => runAction('delete')} aria-label={`删除服务商 ${provider.name}`} title="删除服务商"><Trash2 size={14} /></button>
        <span className="w-px h-4 bg-[var(--border-color)] mx-1" aria-hidden="true" />
        <Switch checked={provider.enabled} disabled={pending !== null} onCheckedChange={() => runAction('toggle')} ariaLabel={`${provider.enabled ? '停用' : '启用'}服务商 ${provider.name}`} />
      </div>
    </article>
  );
}

export function ProviderList() {
  const { providers, openPresetDrawer } = useAppStore(
    useShallow((state) => ({ providers: state.providers, openPresetDrawer: state.openPresetDrawer })),
  );

  return (
    <div className="page-stack pt-3.5">
      {providers.length === 0 ? (
        <section className="surface empty-state">
          <div className="empty-icon"><Network size={22} /></div>
          <h2>连接第一个模型服务</h2>
          <p>从预设库选择服务商，再填入密钥。也可以手动添加兼容接口或本地模型。</p>
          <button type="button" className="button button-primary mt-2" onClick={openPresetDrawer}><Library size={14} />浏览预设库</button>
        </section>
      ) : (
        <>
          <section className="surface provider-table" aria-label="服务商列表">
            <div className="provider-columns" aria-hidden="true"><span>服务商</span><span>网络延迟</span><span className="text-right">操作 / 启用</span></div>
            {providers.map((provider) => <ProviderRow key={provider.id} provider={provider} />)}
          </section>
          <p className="muted text-[11px] flex items-center gap-1.5"><KeyRound size={12} />密钥留空时使用客户端传入的密钥；网络延迟不代表模型响应速度。</p>
        </>
      )}
    </div>
  );
}
