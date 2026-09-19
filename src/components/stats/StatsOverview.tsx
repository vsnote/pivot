import React from 'react';
import { useAppStore } from '@/store/useAppStore';
import {
  Activity,
  Zap,
  Server,
  AlertTriangle,
  Clock,
  Sparkles,
  CheckCircle2,
} from 'lucide-react';

export const StatsOverview: React.FC = () => {
  const { stats, providers } = useAppStore();

  const formatUptime = (secs: number) => {
    if (secs < 60) return `${secs}秒`;
    const hours = Math.floor(secs / 3600);
    const minutes = Math.floor((secs % 3600) / 60);
    if (hours === 0) return `${minutes}分钟`;
    return `${hours}小时 ${minutes}分钟`;
  };

  const formatTokens = (tokens: number) => {
    if (tokens >= 1_000_000) return `${(tokens / 1_000_000).toFixed(2)} M`;
    if (tokens >= 1_000) return `${(tokens / 1_000).toFixed(1)} K`;
    return tokens.toString();
  };

  const activeProvidersCount = providers.filter((p) => p.enabled).length;
  const totalTokens = stats.totalPromptTokens + stats.totalCompletionTokens;

  return (
    <div className="space-y-4 animate-in fade-in duration-150">
      {/* 极简标题 */}
      <div className="pb-1">
        <h2 className="text-base font-bold text-zinc-950 dark:text-zinc-100">
          监控概览
        </h2>
      </div>

      {/* 服务状态状态条 */}
      <div className="flex items-center justify-between p-4 rounded-2xl border border-zinc-200/80 dark:border-zinc-800 bg-white dark:bg-zinc-900 shadow-xs">
        <div className="flex items-center gap-3.5">
          <div className="relative flex items-center justify-center">
            <div
              className={`w-3 h-3 rounded-full ${stats.isRunning ? 'bg-emerald-500' : 'bg-rose-500'}`}
            />
            {stats.isRunning && (
              <div className="absolute w-5 h-5 rounded-full bg-emerald-500/20 animate-ping" />
            )}
          </div>
          <div>
            <div className="flex items-center gap-2">
              <span className="text-xs font-bold text-zinc-900 dark:text-zinc-100">
                {stats.isRunning ? '本地网关服务运行正常' : '本地网关服务未运行'}
              </span>
              <span
                className={`text-[11px] font-mono px-2 py-0.5 rounded-full font-medium border ${
                  stats.isRunning
                    ? 'bg-emerald-50 dark:bg-emerald-950/50 text-emerald-700 dark:text-emerald-300 border-emerald-200/60 dark:border-emerald-800/50'
                    : 'bg-rose-50 dark:bg-rose-950/50 text-rose-700 dark:text-rose-300 border-rose-200/60 dark:border-rose-800/50'
                }`}
              >
                127.0.0.1:{stats.port}
              </span>
            </div>
            <p className="text-xs text-zinc-500 dark:text-zinc-400 mt-0.5">
              {stats.isRunning
                ? 'Axum 高并发反向代理服务就绪，持续监控请求流量'
                : '端口可能被占用，请关闭冲突进程后重新启动 OmniSwitch'}
            </p>
          </div>
        </div>

        <div className="hidden sm:flex items-center gap-1.5 text-xs text-zinc-500 dark:text-zinc-400">
          {stats.isRunning ? (
            <CheckCircle2 className="w-4 h-4 text-emerald-500" />
          ) : (
            <AlertTriangle className="w-4 h-4 text-rose-500" />
          )}
          <span>{stats.isRunning ? '服务就绪' : '连接失败'}</span>
        </div>
      </div>

      {/* 核心指标卡片矩阵 */}
      <div className="grid grid-cols-2 lg:grid-cols-3 gap-3">
        {/* 请求总数 */}
        <div className="p-4 rounded-2xl border border-zinc-200/80 dark:border-zinc-800 bg-white dark:bg-zinc-900 shadow-xs">
          <div className="flex items-center justify-between text-xs text-zinc-500 dark:text-zinc-400">
            <span>请求总数</span>
            <Activity className="w-4 h-4 text-indigo-500" />
          </div>
          <div className="mt-2.5 flex items-baseline gap-2">
            <span className="text-2xl font-bold font-mono text-zinc-950 dark:text-white">
              {stats.totalRequests}
            </span>
            <span className="text-[11px] text-emerald-600 dark:text-emerald-400 font-medium">
              次调用
            </span>
          </div>
        </div>

        {/* Token 消耗 */}
        <div className="p-4 rounded-2xl border border-zinc-200/80 dark:border-zinc-800 bg-white dark:bg-zinc-900 shadow-xs">
          <div className="flex items-center justify-between text-xs text-zinc-500 dark:text-zinc-400">
            <span>Token 用量</span>
            <Sparkles className="w-4 h-4 text-violet-500" />
          </div>
          <div className="mt-2.5 flex items-baseline gap-2">
            <span className="text-2xl font-bold font-mono text-zinc-950 dark:text-white">
              {formatTokens(totalTokens)}
            </span>
            <span className="text-[11px] text-zinc-400">
              ({formatTokens(stats.totalPromptTokens)} 入 / {formatTokens(stats.totalCompletionTokens)} 出)
            </span>
          </div>
        </div>

        {/* 当前活跃请求 */}
        <div className="p-4 rounded-2xl border border-zinc-200/80 dark:border-zinc-800 bg-white dark:bg-zinc-900 shadow-xs">
          <div className="flex items-center justify-between text-xs text-zinc-500 dark:text-zinc-400">
            <span>进行中并发</span>
            <Zap className="w-4 h-4 text-amber-500" />
          </div>
          <div className="mt-2.5 flex items-baseline gap-2">
            <span className="text-2xl font-bold font-mono text-zinc-950 dark:text-white">
              {stats.activeRequests}
            </span>
            <span className="text-[11px] text-zinc-400">个请求</span>
          </div>
        </div>

        {/* 可用服务商 */}
        <div className="p-4 rounded-2xl border border-zinc-200/80 dark:border-zinc-800 bg-white dark:bg-zinc-900 shadow-xs">
          <div className="flex items-center justify-between text-xs text-zinc-500 dark:text-zinc-400">
            <span>可用服务商</span>
            <Server className="w-4 h-4 text-emerald-500" />
          </div>
          <div className="mt-2.5 flex items-baseline gap-2">
            <span className="text-2xl font-bold font-mono text-zinc-950 dark:text-white">
              {activeProvidersCount}
            </span>
            <span className="text-[11px] text-zinc-400">/ {providers.length} 启用</span>
          </div>
        </div>

        {/* 持续在线时长 */}
        <div className="p-4 rounded-2xl border border-zinc-200/80 dark:border-zinc-800 bg-white dark:bg-zinc-900 shadow-xs">
          <div className="flex items-center justify-between text-xs text-zinc-500 dark:text-zinc-400">
            <span>网关运行时长</span>
            <Clock className="w-4 h-4 text-blue-500" />
          </div>
          <div className="mt-2.5 flex items-baseline gap-2">
            <span className="text-xl font-bold font-mono text-zinc-950 dark:text-white">
              {formatUptime(stats.uptimeSecs)}
            </span>
          </div>
        </div>

        {/* 异常请求数 */}
        <div className="p-4 rounded-2xl border border-zinc-200/80 dark:border-zinc-800 bg-white dark:bg-zinc-900 shadow-xs">
          <div className="flex items-center justify-between text-xs text-zinc-500 dark:text-zinc-400">
            <span>异常与失败</span>
            <AlertTriangle className="w-4 h-4 text-rose-500" />
          </div>
          <div className="mt-2.5 flex items-baseline gap-2">
            <span className="text-2xl font-bold font-mono text-zinc-950 dark:text-white">
              {stats.errorCount}
            </span>
            <span className="text-[11px] text-zinc-400">次</span>
          </div>
        </div>
      </div>
    </div>
  );
};
