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
    setDiagnosticStatus('Pinging local gateway on http://127.0.0.1:16888/health...');
    try {
      const res = await fetch(`http://127.0.0.1:${stats.port}/health`);
      if (res.ok) {
        const data = await res.json();
        setDiagnosticStatus(
          `Healthy! Gateway responded with 200 OK: ${JSON.stringify(data)}`
        );
      } else {
        setDiagnosticStatus(`Received status ${res.status} from gateway.`);
      }
    } catch (err: unknown) {
      setDiagnosticStatus(`Diagnostic error: ${String(err)}`);
    } finally {
      setIsRunningDiagnostic(false);
    }
  };

  const activeProvidersCount = providers.filter((p) => p.enabled).length;

  return (
    <div className="space-y-6">
      {/* Metric Cards Grid */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        {/* Total Requests */}
        <div className="p-4 rounded-xl border border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-900 shadow-sm">
          <div className="flex items-center justify-between text-xs text-gray-500 dark:text-gray-400">
            <span>Total Requests</span>
            <Activity className="w-4 h-4 text-indigo-500" />
          </div>
          <div className="mt-2 flex items-baseline gap-2">
            <span className="text-2xl font-bold font-mono text-gray-900 dark:text-gray-100">
              {stats.totalRequests}
            </span>
            <span className="text-xs text-emerald-600 dark:text-emerald-400 font-medium">
              active stream
            </span>
          </div>
        </div>

        {/* Active Streams */}
        <div className="p-4 rounded-xl border border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-900 shadow-sm">
          <div className="flex items-center justify-between text-xs text-gray-500 dark:text-gray-400">
            <span>Concurrent Streams</span>
            <Zap className="w-4 h-4 text-amber-500" />
          </div>
          <div className="mt-2 flex items-baseline gap-2">
            <span className="text-2xl font-bold font-mono text-gray-900 dark:text-gray-100">
              {stats.activeRequests}
            </span>
            <span className="text-xs text-gray-400">Tokio tasks</span>
          </div>
        </div>

        {/* Active Providers */}
        <div className="p-4 rounded-xl border border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-900 shadow-sm">
          <div className="flex items-center justify-between text-xs text-gray-500 dark:text-gray-400">
            <span>Active Providers</span>
            <Server className="w-4 h-4 text-emerald-500" />
          </div>
          <div className="mt-2 flex items-baseline gap-2">
            <span className="text-2xl font-bold font-mono text-gray-900 dark:text-gray-100">
              {activeProvidersCount}
            </span>
            <span className="text-xs text-gray-400">/ {providers.length} total</span>
          </div>
        </div>

        {/* Error Count */}
        <div className="p-4 rounded-xl border border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-900 shadow-sm">
          <div className="flex items-center justify-between text-xs text-gray-500 dark:text-gray-400">
            <span>Error &amp; Failover</span>
            <AlertTriangle className="w-4 h-4 text-red-500" />
          </div>
          <div className="mt-2 flex items-baseline gap-2">
            <span className="text-2xl font-bold font-mono text-gray-900 dark:text-gray-100">
              {stats.errorCount}
            </span>
            <span className="text-xs text-gray-400">failovers</span>
          </div>
        </div>
      </div>

      {/* Gateway Kernel Details */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        <div className="p-5 rounded-2xl border border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-900 shadow-sm">
          <div className="flex items-center gap-2 mb-3">
            <Cpu className="w-5 h-5 text-indigo-600 dark:text-indigo-400" />
            <h3 className="text-sm font-semibold text-gray-900 dark:text-gray-100">
              Built-in Gateway Kernel
            </h3>
          </div>
          <div className="space-y-2 text-xs font-mono text-gray-600 dark:text-gray-300">
            <div className="flex justify-between py-1 border-b border-gray-100 dark:border-gray-800">
              <span className="text-gray-400">Runtime:</span>
              <span>Rust Axum 0.8 + Tokio Async (Native)</span>
            </div>
            <div className="flex justify-between py-1 border-b border-gray-100 dark:border-gray-800">
              <span className="text-gray-400">Host &amp; Port:</span>
              <span>127.0.0.1:{stats.port}</span>
            </div>
            <div className="flex justify-between py-1 border-b border-gray-100 dark:border-gray-800">
              <span className="text-gray-400">Active Routing Rules:</span>
              <span>{routes.length} Rules Configured</span>
            </div>
            <div className="flex justify-between py-1">
              <span className="text-gray-400">Gateway Uptime:</span>
              <span>{formatUptime(stats.uptimeSecs)}</span>
            </div>
          </div>
        </div>

        {/* Self Diagnostic Panel */}
        <div className="p-5 rounded-2xl border border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-900 shadow-sm flex flex-col justify-between">
          <div>
            <div className="flex items-center gap-2 mb-2">
              <Terminal className="w-5 h-5 text-emerald-600 dark:text-emerald-400" />
              <h3 className="text-sm font-semibold text-gray-900 dark:text-gray-100">
                Gateway Self-Diagnostic
              </h3>
            </div>
            <p className="text-xs text-gray-500 dark:text-gray-400 mb-3">
              Verify local gateway loopback connection, health endpoint, and latency.
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
            className="flex items-center justify-center gap-2 px-4 py-2 text-xs font-medium text-white bg-indigo-600 hover:bg-indigo-700 rounded-lg transition-colors shadow-sm self-start"
          >
            <Play className="w-3.5 h-3.5" />
            <span>{isRunningDiagnostic ? 'Testing...' : 'Run Gateway Self-Check'}</span>
          </button>
        </div>
      </div>
    </div>
  );
};
