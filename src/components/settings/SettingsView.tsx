import React from 'react';
import { useAppStore } from '@/store/useAppStore';
import { InterceptorPanel } from '@/components/interceptor/InterceptorPanel';
import { RouteMatrix } from '@/components/routes/RouteMatrix';
import { StatsOverview } from '@/components/stats/StatsOverview';
import { RequestLogs } from '@/components/logs/RequestLogs';
import { ShieldCheck, Globe, Copy, Check } from 'lucide-react';

export const SettingsView: React.FC = () => {
  const { settingsSection, stats } = useAppStore();
  const [copiedKey, setCopiedKey] = React.useState<string | null>(null);

  const copyText = (key: string, text: string) => {
    navigator.clipboard.writeText(text);
    setCopiedKey(key);
    setTimeout(() => setCopiedKey(null), 2000);
  };

  const anthropicUrl = `http://${stats.host}:${stats.port}`;
  const openaiUrl = `http://${stats.host}:${stats.port}/v1`;

  return (
    <div className="w-full pb-10">
      {settingsSection === 'clients' && <InterceptorPanel />}
      {settingsSection === 'routes' && <RouteMatrix />}
      {settingsSection === 'gateway' && <StatsOverview />}
      {settingsSection === 'logs' && <RequestLogs />}
      {settingsSection === 'about' && (
        <div className="space-y-4 animate-in fade-in duration-150">
          <div className="pb-1">
            <h2 className="text-base font-bold text-zinc-950 dark:text-zinc-100">
              设置与关于
            </h2>
          </div>

          <div className="p-6 rounded-2xl border border-zinc-200/80 dark:border-zinc-800 bg-white dark:bg-zinc-900 shadow-xs space-y-6">
            <div className="flex items-center gap-3.5">
              <div className="w-10 h-10 rounded-xl bg-indigo-600 dark:bg-indigo-500 text-white flex items-center justify-center shadow-md shadow-indigo-500/25">
                <Globe className="w-5 h-5" />
              </div>
              <div>
                <h3 className="text-sm font-bold text-zinc-950 dark:text-white flex items-center gap-2">
                  OmniSwitch
                  <span className="text-[11px] font-mono font-medium px-2 py-0.5 rounded-full bg-indigo-50 dark:bg-indigo-950/60 text-indigo-600 dark:text-indigo-400 border border-indigo-200/60 dark:border-indigo-800/60">
                    v0.1.0-alpha
                  </span>
                </h3>
              </div>
            </div>

            {/* 本地服务接入地址 */}
            <div className="space-y-2 pt-4 border-t border-zinc-100 dark:border-zinc-800">
              <h4 className="text-xs font-bold text-zinc-900 dark:text-zinc-100 uppercase tracking-wider">
                本地服务接入点
              </h4>

              <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                <div className="p-3.5 rounded-xl bg-zinc-50 dark:bg-zinc-850/60 border border-zinc-200/60 dark:border-zinc-800 flex items-center justify-between">
                  <div>
                    <div className="text-[11px] text-zinc-500 dark:text-zinc-400 mb-1">
                      Anthropic 协议地址
                    </div>
                    <div className="font-mono text-xs font-semibold text-indigo-600 dark:text-indigo-400">
                      {anthropicUrl}
                    </div>
                  </div>
                  <button
                    onClick={() => copyText('anthropic', anthropicUrl)}
                    className="p-1.5 rounded-lg text-zinc-400 hover:text-zinc-700 dark:hover:text-zinc-200 hover:bg-zinc-200 dark:hover:bg-zinc-700 transition-colors cursor-pointer"
                    title="复制接口地址"
                  >
                    {copiedKey === 'anthropic' ? (
                      <Check className="w-3.5 h-3.5 text-emerald-500" />
                    ) : (
                      <Copy className="w-3.5 h-3.5" />
                    )}
                  </button>
                </div>

                <div className="p-3.5 rounded-xl bg-zinc-50 dark:bg-zinc-850/60 border border-zinc-200/60 dark:border-zinc-800 flex items-center justify-between">
                  <div>
                    <div className="text-[11px] text-zinc-500 dark:text-zinc-400 mb-1">
                      OpenAI 兼容协议地址
                    </div>
                    <div className="font-mono text-xs font-semibold text-indigo-600 dark:text-indigo-400">
                      {openaiUrl}
                    </div>
                  </div>
                  <button
                    onClick={() => copyText('openai', openaiUrl)}
                    className="p-1.5 rounded-lg text-zinc-400 hover:text-zinc-700 dark:hover:text-zinc-200 hover:bg-zinc-200 dark:hover:bg-zinc-700 transition-colors cursor-pointer"
                    title="复制接口地址"
                  >
                    {copiedKey === 'openai' ? (
                      <Check className="w-3.5 h-3.5 text-emerald-500" />
                    ) : (
                      <Copy className="w-3.5 h-3.5" />
                    )}
                  </button>
                </div>
              </div>
            </div>

            {/* 本地数据安全承诺 */}
            <div className="p-4 rounded-xl bg-emerald-50 dark:bg-emerald-950/30 border border-emerald-200/80 dark:border-emerald-900/60 flex items-start gap-3">
              <ShieldCheck className="w-5 h-5 text-emerald-600 dark:text-emerald-400 shrink-0 mt-0.5" />
              <div className="text-xs text-emerald-900 dark:text-emerald-200 leading-relaxed">
                <strong className="font-semibold">纯本地离线隐私：</strong>
                所有 API 密钥、路由规则与日志流水完全存储于用户主目录下的本地 SQLite 数据库中（
                <code className="font-mono px-1 py-0.5 rounded bg-emerald-100/60 dark:bg-emerald-900/60">
                  ~/.omniswitch/omniswitch.db
                </code>
                ），绝无任何数据遥测或第三方云端上传。
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
