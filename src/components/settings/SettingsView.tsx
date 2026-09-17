import React from 'react';
import { useAppStore } from '@/store/useAppStore';
import { InterceptorPanel } from '@/components/interceptor/InterceptorPanel';
import { RouteMatrix } from '@/components/routes/RouteMatrix';
import { StatsOverview } from '@/components/stats/StatsOverview';
import { RequestLogs } from '@/components/logs/RequestLogs';
import { ShieldCheck, Globe } from 'lucide-react';

export const SettingsView: React.FC = () => {
  const { settingsSection, stats } = useAppStore();

  return (
    <div className="max-w-5xl mx-auto pb-12 pt-2 animate-in fade-in duration-200">
      {settingsSection === 'clients' && <InterceptorPanel />}
      {settingsSection === 'routes' && <RouteMatrix />}
      {settingsSection === 'gateway' && <StatsOverview />}
      {settingsSection === 'logs' && <RequestLogs />}
      {settingsSection === 'about' && (
        <div className="space-y-4 animate-in fade-in duration-150">
          <div className="p-6 rounded-2xl border border-gray-200/80 dark:border-white/10 bg-white/70 dark:bg-zinc-900/60 backdrop-blur-md shadow-xs">
            <div className="flex items-center gap-3.5 mb-4">
              <div className="w-11 h-11 rounded-2xl bg-indigo-600 dark:bg-indigo-500 text-white flex items-center justify-center shadow-md shadow-indigo-500/25">
                <Globe className="w-6 h-6" />
              </div>
              <div>
                <h3 className="text-base font-bold text-gray-950 dark:text-white flex items-center gap-2">
                  OmniSwitch
                  <span className="text-[11px] font-mono font-medium px-2 py-0.5 rounded-full bg-indigo-50 dark:bg-indigo-950/60 text-indigo-600 dark:text-indigo-400 border border-indigo-200/60 dark:border-indigo-800/60">
                    v0.1.0-alpha
                  </span>
                </h3>
                <p className="text-xs text-gray-500 dark:text-zinc-400 mt-0.5">
                  面向 AI 编程客户端的轻量低延迟路由网关
                </p>
              </div>
            </div>

            {/* 核心服务地址 */}
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 pt-4 border-t border-gray-100 dark:border-white/5">
              <div className="p-3.5 rounded-xl bg-gray-50/80 dark:bg-white/[0.03] border border-gray-100 dark:border-white/5">
                <div className="text-xs text-gray-500 dark:text-zinc-400 mb-1">
                  Anthropic 接口地址
                </div>
                <div className="font-mono text-xs font-semibold text-indigo-600 dark:text-indigo-400">
                  http://{stats.host}:{stats.port}
                </div>
              </div>

              <div className="p-3.5 rounded-xl bg-gray-50/80 dark:bg-white/[0.03] border border-gray-100 dark:border-white/5">
                <div className="text-xs text-gray-500 dark:text-zinc-400 mb-1">
                  OpenAI 兼容接口
                </div>
                <div className="font-mono text-xs font-semibold text-indigo-600 dark:text-indigo-400">
                  http://{stats.host}:{stats.port}/v1
                </div>
              </div>
            </div>

            {/* 数据安全说明 */}
            <div className="mt-4 p-4 rounded-xl bg-emerald-500/5 dark:bg-emerald-500/10 border border-emerald-500/20 flex items-start gap-3">
              <ShieldCheck className="w-5 h-5 text-emerald-600 dark:text-emerald-400 shrink-0 mt-0.5" />
              <div className="text-xs text-emerald-900 dark:text-emerald-200 leading-relaxed">
                <strong className="font-semibold">本地数据：</strong>
                API 密钥、路由配置和请求记录仅存储在本地 SQLite 数据库中，不经过第三方服务器。
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
