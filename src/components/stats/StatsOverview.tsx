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
    <div className="space-y-4 pb-8 animate-in fade-in duration-150">
      {/* 顶部服务状态概览条 */}
      <div className="flex items-center justify-between p-3.5 px-4 rounded-xl border border-gray-200/80 dark:border-white/10 bg-white/70 dark:bg-zinc-900/60 backdrop-blur-md shadow-xs">
        <div className="flex items-center gap-3">
          <div className="relative flex items-center justify-center">
            <div className={`w-2.5 h-2.5 rounded-full ${stats.isRunning ? 'bg-emerald-500' : 'bg-rose-500'}`} />
            {stats.isRunning && <div className="absolute w-4 h-4 rounded-full bg-emerald-500/20 animate-ping" />}
          </div>
          <div>
            <div className="flex items-center gap-2">
              <span className="text-xs font-semibold text-gray-900 dark:text-zinc-100">
                {stats.isRunning ? '本地服务运行正常' : '本地服务未运行'}
              </span>
              <span className={`text-[11px] font-mono px-1.5 py-0.5 rounded font-medium border ${stats.isRunning
                ? 'bg-emerald-50 dark:bg-emerald-950/50 text-emerald-600 dark:text-emerald-400 border-emerald-200/50 dark:border-emerald-800/40'
                : 'bg-rose-50 dark:bg-rose-950/50 text-rose-600 dark:text-rose-400 border-rose-200/50 dark:border-rose-800/40'
              }`}>
                端口 {stats.port}
              </span>
            </div>
            <p className="text-[11px] text-gray-500 dark:text-zinc-400 mt-0.5">
              {stats.isRunning ? '已准备接收并分发 AI 客户端请求' : '端口可能被占用，请关闭其他实例后重启 OmniSwitch'}
            </p>
          </div>
        </div>

        <div className="flex items-center gap-1.5 text-xs text-gray-500 dark:text-zinc-400">
          {stats.isRunning
            ? <CheckCircle2 className="w-3.5 h-3.5 text-emerald-500" />
            : <AlertTriangle className="w-3.5 h-3.5 text-rose-500" />}
          <span>{stats.isRunning ? '本机连接正常' : '连接失败'}</span>
        </div>
      </div>

      {/* 核心指标卡片矩阵 */}
      <div className="grid grid-cols-2 lg:grid-cols-3 gap-3">
        {/* 请求总数 */}
        <div className="p-4 rounded-xl border border-gray-200/80 dark:border-white/10 bg-white dark:bg-[#1a1b1e] shadow-xs">
          <div className="flex items-center justify-between text-xs text-gray-500 dark:text-gray-400">
            <span>请求总数</span>
            <Activity className="w-4 h-4 text-indigo-500" />
          </div>
          <div className="mt-2.5 flex items-baseline gap-2">
            <span className="text-2xl font-bold font-mono text-gray-950 dark:text-white">
              {stats.totalRequests}
            </span>
            <span className="text-[11px] text-emerald-600 dark:text-emerald-400 font-medium">
              次调用
            </span>
          </div>
        </div>

        {/* Token 消耗 */}
        <div className="p-4 rounded-xl border border-gray-200/80 dark:border-white/10 bg-white dark:bg-[#1a1b1e] shadow-xs">
          <div className="flex items-center justify-between text-xs text-gray-500 dark:text-gray-400">
            <span>Token 用量</span>
            <Sparkles className="w-4 h-4 text-violet-500" />
          </div>
          <div className="mt-2.5 flex items-baseline gap-2">
            <span className="text-2xl font-bold font-mono text-gray-950 dark:text-white">
              {formatTokens(totalTokens)}
            </span>
            <span className="text-[11px] text-gray-400">
              (输入 {formatTokens(stats.totalPromptTokens)} / 输出 {formatTokens(stats.totalCompletionTokens)})
            </span>
          </div>
        </div>

        {/* 当前活跃请求 */}
        <div className="p-4 rounded-xl border border-gray-200/80 dark:border-white/10 bg-white dark:bg-[#1a1b1e] shadow-xs">
          <div className="flex items-center justify-between text-xs text-gray-500 dark:text-gray-400">
            <span>处理中</span>
            <Zap className="w-4 h-4 text-amber-500" />
          </div>
          <div className="mt-2.5 flex items-baseline gap-2">
            <span className="text-2xl font-bold font-mono text-gray-950 dark:text-white">
              {stats.activeRequests}
            </span>
            <span className="text-[11px] text-gray-400">进行中</span>
          </div>
        </div>

        {/* 活跃服务商 */}
        <div className="p-4 rounded-xl border border-gray-200/80 dark:border-white/10 bg-white dark:bg-[#1a1b1e] shadow-xs">
          <div className="flex items-center justify-between text-xs text-gray-500 dark:text-gray-400">
            <span>可用服务商</span>
            <Server className="w-4 h-4 text-emerald-500" />
          </div>
          <div className="mt-2.5 flex items-baseline gap-2">
            <span className="text-2xl font-bold font-mono text-gray-950 dark:text-white">
              {activeProvidersCount}
            </span>
            <span className="text-[11px] text-gray-400">/ {providers.length}</span>
          </div>
        </div>

        {/* 持续在线时长 */}
        <div className="p-4 rounded-xl border border-gray-200/80 dark:border-white/10 bg-white dark:bg-[#1a1b1e] shadow-xs">
          <div className="flex items-center justify-between text-xs text-gray-500 dark:text-gray-400">
            <span>运行时间</span>
            <Clock className="w-4 h-4 text-blue-500" />
          </div>
          <div className="mt-2.5 flex items-baseline gap-2">
            <span className="text-xl font-bold font-mono text-gray-950 dark:text-white">
              {formatUptime(stats.uptimeSecs)}
            </span>
          </div>
        </div>

        {/* 异常重试降级 */}
        <div className="p-4 rounded-xl border border-gray-200/80 dark:border-white/10 bg-white dark:bg-[#1a1b1e] shadow-xs">
          <div className="flex items-center justify-between text-xs text-gray-500 dark:text-gray-400">
            <span>异常请求</span>
            <AlertTriangle className="w-4 h-4 text-rose-500" />
          </div>
          <div className="mt-2.5 flex items-baseline gap-2">
            <span className="text-2xl font-bold font-mono text-gray-950 dark:text-white">
              {stats.errorCount}
            </span>
            <span className="text-[11px] text-gray-400">次</span>
          </div>
        </div>
      </div>
    </div>
  );
};
