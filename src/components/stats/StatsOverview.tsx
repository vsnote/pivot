import React, { useState } from 'react';
import { useAppStore } from '@/store/useAppStore';
import {
  Activity,
  Zap,
  Server,
  AlertTriangle,
  Cpu,
  Play,
  Terminal,
} from 'lucide-react';

export const StatsOverview: React.FC = () => {
  const { stats, providers, routes } = useAppStore();
  const [diagnosticStatus, setDiagnosticStatus] = useState<string | null>(null);
  const [isRunningDiagnostic, setIsRunningDiagnostic] = useState(false);

  const formatUptime = (secs: number) => {
    const hours = Math.floor(secs / 3600);
    const minutes = Math.floor((secs % 3600) / 60);
    const seconds = secs % 60;
    return `${hours}h ${minutes}m ${seconds}s`;
  };

  const handleRunDiagnostic = async () => {
    setIsRunningDiagnostic(true);
    setDiagnosticStatus('正在向本地网关 http://127.0.0.1:16888/health 发起健康检测...');
    try {
      const res = await fetch(`http://127.0.0.1:${stats.port}/health`);
      if (res.ok) {
        const data = await res.json();
        setDiagnosticStatus(
          `健康状态正常！网关返回 200 OK: ${JSON.stringify(data)}`
        );
      } else {
        setDiagnosticStatus(`网关返回异常状态码: ${res.status}`);
      }
    } catch (err: unknown) {
      setDiagnosticStatus(`自检异常: ${String(err)}`);
    } finally {
      setIsRunningDiagnostic(false);
    }
  };

  const activeProvidersCount = providers.filter((p) => p.enabled).length;

  return (
    <div className="space-y-6">
      {/* 核心指标卡片 */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        {/* 请求总数 */}
        <div className="p-4 rounded-xl border border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-900 shadow-xs">
          <div className="flex items-center justify-between text-xs text-gray-500 dark:text-gray-400">
            <span>请求处理总数</span>
            <Activity className="w-4 h-4 text-indigo-500" />
          </div>
          <div className="mt-2 flex items-baseline gap-2">
            <span className="text-2xl font-bold font-mono text-gray-900 dark:text-gray-100">
              {stats.totalRequests}
            </span>
            <span className="text-xs text-emerald-600 dark:text-emerald-400 font-medium">
              实时流量
            </span>
          </div>
        </div>

        {/* 并发连接 */}
        <div className="p-4 rounded-xl border border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-900 shadow-xs">
          <div className="flex items-center justify-between text-xs text-gray-500 dark:text-gray-400">
            <span>当前并发请求</span>
            <Zap className="w-4 h-4 text-amber-500" />
          </div>
          <div className="mt-2 flex items-baseline gap-2">
            <span className="text-2xl font-bold font-mono text-gray-900 dark:text-gray-100">
              {stats.activeRequests}
            </span>
            <span className="text-xs text-gray-400">Tokio 异步任务</span>
          </div>
        </div>

        {/* 在线服务商 */}
        <div className="p-4 rounded-xl border border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-900 shadow-xs">
          <div className="flex items-center justify-between text-xs text-gray-500 dark:text-gray-400">
            <span>已启用服务商</span>
            <Server className="w-4 h-4 text-emerald-500" />
          </div>
          <div className="mt-2 flex items-baseline gap-2">
            <span className="text-2xl font-bold font-mono text-gray-900 dark:text-gray-100">
              {activeProvidersCount}
            </span>
            <span className="text-xs text-gray-400">/ 共 {providers.length} 个</span>
          </div>
        </div>

        {/* 异常降级 */}
        <div className="p-4 rounded-xl border border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-900 shadow-xs">
          <div className="flex items-center justify-between text-xs text-gray-500 dark:text-gray-400">
            <span>错误与自动降级</span>
            <AlertTriangle className="w-4 h-4 text-red-500" />
          </div>
          <div className="mt-2 flex items-baseline gap-2">
            <span className="text-2xl font-bold font-mono text-gray-900 dark:text-gray-100">
              {stats.errorCount}
            </span>
            <span className="text-xs text-gray-400">次自动容灾切换</span>
          </div>
        </div>
      </div>

      {/* 内核详情与自检面板 */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        <div className="p-5 rounded-2xl border border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-900 shadow-xs">
          <div className="flex items-center gap-2 mb-3">
            <Cpu className="w-5 h-5 text-indigo-600 dark:text-indigo-400" />
            <h3 className="text-sm font-semibold text-gray-900 dark:text-gray-100">
              原生网关内核架构
            </h3>
          </div>
          <div className="space-y-2 text-xs font-mono text-gray-600 dark:text-gray-300">
            <div className="flex justify-between py-1 border-b border-gray-100 dark:border-gray-800">
              <span className="text-gray-400">底层运行时:</span>
              <span>Rust Axum 0.8 + Tokio Async (独立线程 Reactor)</span>
            </div>
            <div className="flex justify-between py-1 border-b border-gray-100 dark:border-gray-800">
              <span className="text-gray-400">监听地址与端口:</span>
              <span>127.0.0.1:{stats.port}</span>
            </div>
            <div className="flex justify-between py-1 border-b border-gray-100 dark:border-gray-800">
              <span className="text-gray-400">生效路由规则:</span>
              <span>{routes.length} 条规则已加载</span>
            </div>
            <div className="flex justify-between py-1">
              <span className="text-gray-400">网关已连续运行:</span>
              <span>{formatUptime(stats.uptimeSecs)}</span>
            </div>
          </div>
        </div>

        {/* 连通性自检面板 */}
        <div className="p-5 rounded-2xl border border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-900 shadow-xs flex flex-col justify-between">
          <div>
            <div className="flex items-center gap-2 mb-2">
              <Terminal className="w-5 h-5 text-emerald-600 dark:text-emerald-400" />
              <h3 className="text-sm font-semibold text-gray-900 dark:text-gray-100">
                本地网关自检探针
              </h3>
            </div>
            <p className="text-xs text-gray-500 dark:text-gray-400 mb-3">
              对本地 127.0.0.1:16888 发起真实 HTTP 健康检查，验证回环网络连通性与协议响应。
            </p>

            {diagnosticStatus && (
              <div className="p-3 rounded-lg bg-gray-50 dark:bg-gray-800/60 border border-gray-200 dark:border-gray-700 text-xs font-mono text-gray-700 dark:text-gray-300 break-all mb-3">
                {diagnosticStatus}
              </div>
            )}
          </div>

          <button
            onClick={handleRunDiagnostic}
            disabled={isRunningDiagnostic}
            className="flex items-center justify-center gap-2 px-4 py-2 text-xs font-medium text-white bg-indigo-600 hover:bg-indigo-700 rounded-lg transition-colors shadow-xs self-start cursor-pointer"
          >
            <Play className="w-3.5 h-3.5" />
            <span>{isRunningDiagnostic ? '自检中...' : '立即运行网关自检'}</span>
          </button>
        </div>
      </div>
    </div>
  );
};
