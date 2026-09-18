import React, { Fragment, useState } from 'react';
import { useAppStore } from '@/store/useAppStore';
import { Select } from '@/components/common/Select';
import type { RequestLog } from '@/types';
import {
  FileText,
  Trash2,
  Filter,
  ArrowRight,
  RotateCcw,
  ChevronDown,
} from 'lucide-react';

export const RequestLogs: React.FC = () => {
  const { logs, clearLogs } = useAppStore();
  const [selectedClient, setSelectedClient] = useState<string>('all');
  const [selectedStatus, setSelectedStatus] = useState<string>('all');
  const [expandedLogId, setExpandedLogId] = useState<string | null>(null);

  const filteredLogs = logs.filter((log) => {
    const matchesClient = selectedClient === 'all' || log.clientType === selectedClient;
    const matchesStatus =
      selectedStatus === 'all' ||
      (selectedStatus === 'success' && log.statusCode >= 200 && log.statusCode < 300) ||
      (selectedStatus === 'error' && log.statusCode >= 400);

    return matchesClient && matchesStatus;
  });

  const getStatusBadge = (code: number) => {
    if (code >= 200 && code < 300) {
      return 'bg-emerald-50 dark:bg-emerald-950/60 text-emerald-700 dark:text-emerald-300 border-emerald-300/40';
    }
    if (code >= 400 && code < 500) {
      return 'bg-amber-50 dark:bg-amber-950/60 text-amber-700 dark:text-amber-300 border-amber-300/40';
    }
    return 'bg-red-50 dark:bg-red-950/60 text-red-700 dark:text-red-300 border-red-300/40';
  };

  const attemptBadge = (code: number) => {
    if (code >= 200 && code < 300) {
      return 'bg-emerald-50 dark:bg-emerald-950/60 text-emerald-700 dark:text-emerald-300 border-emerald-300/40';
    }
    if (code >= 400 && code < 500) {
      return 'bg-amber-50 dark:bg-amber-950/60 text-amber-700 dark:text-amber-300 border-amber-300/40';
    }
    return 'bg-red-50 dark:bg-red-950/60 text-red-700 dark:text-red-300 border-red-300/40';
  };

  // The chain matters when more than one upstream was tried, or when the only
  // attempt failed.
  const hasChain = (log: RequestLog) => {
    const attempts = log.attempts ?? [];
    return attempts.length > 1 || (attempts.length === 1 && log.statusCode >= 400);
  };

  const formatTimestamp = (ts: string) => {
    try {
      const date = new Date(ts);
      return date.toLocaleTimeString();
    } catch {
      return ts;
    }
  };

  return (
    <div className="space-y-4 pb-8">
      {/* 筛选与操作栏 */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
        <div className="flex items-center gap-3">
          <div className="flex items-center gap-1.5 text-xs text-gray-500 dark:text-gray-400">
            <Filter className="w-3.5 h-3.5" />
            <span>筛选</span>
          </div>

          <Select
            value={selectedClient}
            onChange={(e) => setSelectedClient(e.target.value)}
            containerClassName="w-36"
          >
            <option value="all">全部客户端</option>
            <option value="claude_code">Claude Code</option>
            <option value="codex_cli">Codex CLI</option>
            <option value="gemini_cli">Gemini CLI</option>
          </Select>

          <Select
            value={selectedStatus}
            onChange={(e) => setSelectedStatus(e.target.value)}
            containerClassName="w-32"
          >
            <option value="all">全部状态</option>
            <option value="success">2xx 正常</option>
            <option value="error">4xx / 5xx 异常</option>
          </Select>
        </div>

        <button
          onClick={clearLogs}
          className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium text-gray-600 dark:text-gray-300 hover:text-red-600 dark:hover:text-red-400 bg-white dark:bg-[#1a1b1e] border border-gray-200/90 dark:border-gray-800 rounded-lg hover:bg-gray-50 dark:hover:bg-gray-800/80 transition-colors shadow-[0_1px_2px_rgba(0,0,0,0.02)]"
        >
          <Trash2 className="w-3.5 h-3.5" />
          <span>清空日志</span>
        </button>
      </div>

      {/* 日志记录表格 */}
      {filteredLogs.length === 0 ? (
        <div className="text-center py-16 border border-dashed border-gray-200 dark:border-gray-800 rounded-xl bg-white/50 dark:bg-[#1a1b1e]/50">
          <FileText className="w-10 h-10 text-gray-400 mx-auto mb-2" />
          <h3 className="text-sm font-semibold text-gray-900 dark:text-gray-100">
            暂无请求日志
          </h3>
          <p className="text-xs text-gray-500 dark:text-gray-400 mt-1 max-w-sm mx-auto">
            客户端发起请求后，这里会显示状态、耗时和路由结果。
          </p>
        </div>
      ) : (
        <div className="border border-gray-200/80 dark:border-gray-800/80 rounded-xl overflow-hidden bg-white dark:bg-[#1a1b1e] shadow-[0_1px_2px_rgba(0,0,0,0.03)]">
          <table className="w-full text-left border-collapse">
            <thead>
              <tr className="border-b border-gray-200/80 dark:border-gray-800/80 bg-gray-50/80 dark:bg-[#151619] text-[11px] font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wider">
                <th className="py-2.5 px-4">时间</th>
                <th className="py-2.5 px-4">状态</th>
                <th className="py-2.5 px-4">客户端</th>
                <th className="py-2.5 px-4">路径</th>
                <th className="py-2.5 px-4">请求模型</th>
                <th className="py-2.5 px-4">路由结果</th>
                <th className="py-2.5 px-4 text-right">耗时</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-gray-100 dark:divide-gray-800 text-xs">
              {filteredLogs.map((log) => (
                <Fragment key={log.id}>
                <tr
                  className="hover:bg-gray-50/80 dark:hover:bg-gray-800/40 transition-colors font-mono"
                >
                  <td className="py-2.5 px-4 text-gray-400 whitespace-nowrap">
                    {formatTimestamp(log.timestamp)}
                  </td>

                  <td className="py-2.5 px-4 whitespace-nowrap">
                    <span
                      className={`px-2 py-0.5 rounded text-[11px] font-semibold border ${getStatusBadge(
                        log.statusCode
                      )}`}
                    >
                      {log.statusCode}
                    </span>
                    {log.fallbackUsed && (
                      <span className="ml-1.5 px-1.5 py-0.5 rounded bg-amber-100 dark:bg-amber-950 text-amber-700 dark:text-amber-300 text-[10px] font-medium inline-flex items-center gap-0.5">
                        <RotateCcw className="w-2.5 h-2.5" />
                        故障降级
                      </span>
                    )}
                  </td>

                  <td className="py-2.5 px-4 text-gray-700 dark:text-gray-300 whitespace-nowrap">
                    {log.clientType === 'claude_code'
                      ? 'Claude Code'
                      : log.clientType === 'codex_cli'
                      ? 'Codex CLI'
                      : log.clientType}
                  </td>

                  <td className="py-2.5 px-4 text-gray-500 dark:text-gray-400 whitespace-nowrap">
                    <span className="font-semibold text-gray-700 dark:text-gray-300 mr-1">
                      {log.method}
                    </span>
                    {log.path}
                  </td>

                  <td className="py-2.5 px-4 text-indigo-600 dark:text-indigo-400 font-medium truncate max-w-[150px]">
                    {log.incomingModel}
                  </td>

                  <td className="py-2.5 px-4 text-gray-700 dark:text-gray-300 truncate max-w-[200px]">
                    {log.routedProviderName ? (
                      <div className="flex items-center gap-1">
                        <span className="font-medium">{log.routedProviderName}</span>
                        <ArrowRight className="w-3 h-3 text-gray-400" />
                        <span className="text-gray-500">{log.routedModel}</span>
                      </div>
                    ) : (
                      <span className="text-red-500 truncate" title={log.errorMessage}>{log.errorMessage || '请求失败'}</span>
                    )}
                    {hasChain(log) && (
                      <button
                        type="button"
                        aria-expanded={expandedLogId === log.id}
                        onClick={() => setExpandedLogId(expandedLogId === log.id ? null : log.id)}
                        className="mt-0.5 inline-flex items-center gap-1 text-[10px] text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 transition-colors"
                      >
                        <ChevronDown className={`w-3 h-3 transition-transform ${expandedLogId === log.id ? 'rotate-180' : ''}`} />
                        <span>{log.attempts?.length} 次尝试</span>
                      </button>
                    )}
                  </td>

                  <td className="py-2.5 px-4 text-right text-gray-600 dark:text-gray-300 whitespace-nowrap">
                    {log.latencyMs}ms
                  </td>
                </tr>
                {expandedLogId === log.id && (
                  <tr className="bg-gray-50/70 dark:bg-white/[0.02]">
                    <td colSpan={7} className="px-4 pb-3 pt-1">
                      <ol className="space-y-1 border-l border-gray-200 dark:border-gray-800 pl-3">
                        {(log.attempts ?? []).map((attempt, index) => (
                          <li key={`${log.id}-${index}`} className="flex flex-wrap items-baseline gap-x-2 gap-y-0.5 text-[11px]">
                            <span className="text-gray-400 w-3 shrink-0">{index + 1}</span>
                            <span className={`px-1.5 rounded border font-semibold shrink-0 ${attemptBadge(attempt.status)}`}>
                              {attempt.status}
                            </span>
                            <span className="font-medium text-gray-700 dark:text-gray-200">{attempt.providerName}</span>
                            <ArrowRight className="w-3 h-3 text-gray-400 self-center shrink-0" />
                            <span className="text-gray-500 dark:text-gray-400">{attempt.model}</span>
                            <span className="text-gray-400">{attempt.latencyMs}ms</span>
                            {attempt.error && (
                              <span className="text-red-500 dark:text-red-400 truncate max-w-[420px]" title={attempt.error}>
                                {attempt.error}
                              </span>
                            )}
                          </li>
                        ))}
                      </ol>
                    </td>
                  </tr>
                )}
                </Fragment>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
};
