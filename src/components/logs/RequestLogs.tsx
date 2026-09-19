import React, { useState } from 'react';
import { useAppStore } from '@/store/useAppStore';
import { Select } from '@/components/common/Select';
import type { RequestLog } from '@/types';
import {
  FileText,
  Trash2,
  Filter,
  ArrowRight,
  ChevronRight,
  Copy,
  Check,
  X,
  AlertCircle,
  Terminal,
  Layers,
} from 'lucide-react';

export const RequestLogs: React.FC = () => {
  const { logs, clearLogs, stats } = useAppStore();
  const [selectedClient, setSelectedClient] = useState<string>('all');
  const [selectedStatus, setSelectedStatus] = useState<string>('all');
  const [activeLog, setActiveLog] = useState<RequestLog | null>(null);
  const [copiedCurl, setCopiedCurl] = useState(false);
  const [copiedError, setCopiedError] = useState(false);

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
      return 'bg-emerald-50 dark:bg-emerald-950/50 text-emerald-700 dark:text-emerald-300 border-emerald-200/60 dark:border-emerald-800/60';
    }
    if (code >= 400 && code < 500) {
      return 'bg-amber-50 dark:bg-amber-950/50 text-amber-700 dark:text-amber-300 border-amber-200/60 dark:border-amber-800/60';
    }
    return 'bg-rose-50 dark:bg-rose-950/50 text-rose-700 dark:text-rose-300 border-rose-200/60 dark:border-rose-800/60';
  };

  const formatTimestamp = (ts: string) => {
    try {
      const date = new Date(ts);
      return date.toLocaleTimeString();
    } catch {
      return ts;
    }
  };

  const copyCurl = (log: RequestLog) => {
    const isAnthropic = log.path.includes('/messages') || log.clientType === 'claude_code';
    const endpoint = `http://${stats.host}:${stats.port}${log.path}`;
    const curl = isAnthropic
      ? `curl -X POST "${endpoint}" \\\n  -H "content-type: application/json" \\\n  -H "x-api-key: your-key" \\\n  -H "anthropic-version: 2023-06-01" \\\n  -d '{"model": "${log.incomingModel}", "max_tokens": 1024, "messages": [{"role": "user", "content": "Hello"}]}'`
      : `curl -X POST "${endpoint}" \\\n  -H "content-type: application/json" \\\n  -H "authorization: Bearer your-key" \\\n  -d '{"model": "${log.incomingModel}", "messages": [{"role": "user", "content": "Hello"}]}'`;

    navigator.clipboard.writeText(curl);
    setCopiedCurl(true);
    setTimeout(() => setCopiedCurl(false), 2000);
  };

  const copyErrorText = (error: string) => {
    navigator.clipboard.writeText(error);
    setCopiedError(true);
    setTimeout(() => setCopiedError(false), 2000);
  };

  return (
    <div className="space-y-4 animate-in fade-in duration-150">
      {/* 筛选与操作栏 */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
        <div className="flex items-center gap-2.5">
          <div className="flex items-center gap-1.5 text-xs text-zinc-500 dark:text-zinc-400">
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
            <option value="success">2xx 成功</option>
            <option value="error">4xx / 5xx 异常</option>
          </Select>
        </div>

        <button
          onClick={clearLogs}
          className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium text-zinc-600 dark:text-zinc-300 hover:text-rose-600 dark:hover:text-rose-400 bg-white dark:bg-zinc-900 border border-zinc-200/90 dark:border-zinc-800 rounded-lg hover:bg-zinc-50 dark:hover:bg-zinc-800/80 transition-colors shadow-xs self-start sm:self-auto cursor-pointer"
        >
          <Trash2 className="w-3.5 h-3.5" />
          <span>清空日志</span>
        </button>
      </div>

      {/* 日志列表卡片 */}
      {filteredLogs.length === 0 ? (
        <div className="text-center py-16 border border-dashed border-zinc-200 dark:border-zinc-800 rounded-2xl bg-white/50 dark:bg-zinc-900/50">
          <FileText className="w-9 h-9 text-zinc-400 mx-auto mb-2.5" />
          <h3 className="text-sm font-semibold text-zinc-900 dark:text-zinc-100">
            暂无请求记录
          </h3>
          <p className="text-xs text-zinc-500 dark:text-zinc-400 mt-1 max-w-sm mx-auto">
            当 AI 客户端通过网关发起请求后，这里将实时更新请求状态、转发链路与 Token 统计。
          </p>
        </div>
      ) : (
        <div className="rounded-2xl border border-zinc-200/80 dark:border-zinc-800 bg-white dark:bg-zinc-900 overflow-hidden shadow-xs">
          <div className="divide-y divide-zinc-100 dark:divide-zinc-800">
            {filteredLogs.map((log) => {
              const hasFallback = (log.attempts?.length ?? 0) > 1;
              const totalTokens = (log.promptTokens || 0) + (log.completionTokens || 0);

              return (
                <div
                  key={log.id}
                  onClick={() => setActiveLog(log)}
                  className="p-3.5 sm:px-5 flex items-center justify-between gap-3 hover:bg-zinc-50 dark:hover:bg-zinc-850/50 cursor-pointer transition-colors"
                >
                  <div className="flex items-center gap-3 min-w-0 flex-1">
                    {/* 状态码 */}
                    <span
                      className={`text-xs font-mono font-bold px-2 py-0.5 rounded-md border ${getStatusBadge(
                        log.statusCode,
                      )}`}
                    >
                      {log.statusCode}
                    </span>

                    {/* 时间与客户端 */}
                    <div className="flex flex-col min-w-0 flex-1">
                      <div className="flex items-center gap-2">
                        <span className="text-xs font-mono font-medium text-zinc-900 dark:text-zinc-100 truncate">
                          {log.incomingModel}
                        </span>
                        <ArrowRight className="w-3 h-3 text-zinc-400 shrink-0" />
                        <span className="text-xs font-semibold text-indigo-600 dark:text-indigo-400 truncate">
                          {log.routedProviderName || log.routedProviderId}
                        </span>
                        {log.routedModel && log.routedModel !== log.incomingModel && (
                          <span className="text-[10px] font-mono px-1 rounded bg-zinc-100 dark:bg-zinc-800 text-zinc-500 truncate">
                            ({log.routedModel})
                          </span>
                        )}
                        {hasFallback && (
                          <span className="text-[10px] px-1.5 py-0.2 rounded bg-amber-50 dark:bg-amber-950/60 text-amber-600 dark:text-amber-400 border border-amber-200/50 font-medium">
                            重试切换
                          </span>
                        )}
                      </div>

                      <div className="flex items-center gap-3 mt-1 text-[11px] text-zinc-400 dark:text-zinc-500 font-mono">
                        <span>{formatTimestamp(log.timestamp)}</span>
                        <span>·</span>
                        <span>{log.clientType}</span>
                        <span>·</span>
                        <span>{log.path}</span>
                      </div>
                    </div>
                  </div>

                  {/* 耗时与 Token */}
                  <div className="flex items-center gap-4 shrink-0 text-right">
                    <div className="hidden sm:block">
                      <div className="text-xs font-mono font-semibold text-zinc-700 dark:text-zinc-300">
                        {log.latencyMs} ms
                      </div>
                      {totalTokens > 0 && (
                        <div className="text-[11px] text-zinc-400 font-mono">
                          {totalTokens} tokens
                        </div>
                      )}
                    </div>
                    <ChevronRight className="w-4 h-4 text-zinc-400" />
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* 日志详情抽屉 */}
      {activeLog && (
        <div className="fixed inset-0 z-50 flex justify-end bg-black/40 backdrop-blur-xs animate-fade-in">
          <div className="flex-1 cursor-pointer" onClick={() => setActiveLog(null)} />
          <div className="w-full max-w-lg h-full bg-white dark:bg-zinc-900 border-l border-zinc-200 dark:border-zinc-800 shadow-2xl flex flex-col animate-drawer-in">
            {/* 抽屉头部 */}
            <div className="px-6 py-4 border-b border-zinc-200/80 dark:border-zinc-800 flex items-center justify-between shrink-0">
              <div className="flex items-center gap-2">
                <span
                  className={`text-xs font-mono font-bold px-2.5 py-1 rounded-lg border ${getStatusBadge(
                    activeLog.statusCode,
                  )}`}
                >
                  {activeLog.statusCode}
                </span>
                <div>
                  <h3 className="text-sm font-bold text-zinc-950 dark:text-zinc-100">
                    请求详情
                  </h3>
                  <span className="text-[11px] font-mono text-zinc-400">
                    ID: {activeLog.id}
                  </span>
                </div>
              </div>

              <button
                type="button"
                onClick={() => setActiveLog(null)}
                className="p-1.5 rounded-lg text-zinc-400 hover:text-zinc-700 dark:hover:text-zinc-200 hover:bg-zinc-100 dark:hover:bg-zinc-800 transition-colors cursor-pointer"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            {/* 抽屉主体 */}
            <div className="flex-1 overflow-y-auto px-6 py-5 space-y-5">
              {/* 核心指标栅格 */}
              <div className="grid grid-cols-2 gap-2.5">
                <div className="p-3 rounded-xl bg-zinc-50 dark:bg-zinc-850/60 border border-zinc-200/60 dark:border-zinc-800">
                  <span className="text-[11px] text-zinc-400 block mb-0.5">耗时</span>
                  <span className="text-sm font-mono font-bold text-zinc-900 dark:text-zinc-100">
                    {activeLog.latencyMs} ms
                  </span>
                </div>
                <div className="p-3 rounded-xl bg-zinc-50 dark:bg-zinc-850/60 border border-zinc-200/60 dark:border-zinc-800">
                  <span className="text-[11px] text-zinc-400 block mb-0.5">Token 总用量</span>
                  <span className="text-sm font-mono font-bold text-zinc-900 dark:text-zinc-100">
                    {(activeLog.promptTokens || 0) + (activeLog.completionTokens || 0)}
                  </span>
                  <span className="text-[10px] text-zinc-400 block mt-0.5">
                    ({activeLog.promptTokens} in / {activeLog.completionTokens} out)
                  </span>
                </div>
              </div>

              {/* 路由路径 */}
              <div className="space-y-1.5">
                <h4 className="text-xs font-bold text-zinc-900 dark:text-zinc-100 uppercase tracking-wider">
                  请求路由决策
                </h4>
                <div className="p-3.5 rounded-xl bg-zinc-50 dark:bg-zinc-850/60 border border-zinc-200/60 dark:border-zinc-800 space-y-2 text-xs font-mono">
                  <div className="flex justify-between">
                    <span className="text-zinc-400">客户端类型</span>
                    <span className="font-semibold text-zinc-900 dark:text-zinc-100">
                      {activeLog.clientType}
                    </span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-zinc-400">请求路径</span>
                    <span className="font-semibold text-zinc-900 dark:text-zinc-100">
                      {activeLog.method} {activeLog.path}
                    </span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-zinc-400">请求模型</span>
                    <span className="font-semibold text-zinc-900 dark:text-zinc-100">
                      {activeLog.incomingModel}
                    </span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-zinc-400">最终分发服务商</span>
                    <span className="font-semibold text-indigo-600 dark:text-indigo-400">
                      {activeLog.routedProviderName || activeLog.routedProviderId}
                    </span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-zinc-400">最终分发模型</span>
                    <span className="font-semibold text-zinc-900 dark:text-zinc-100">
                      {activeLog.routedModel}
                    </span>
                  </div>
                </div>
              </div>

              {/* 尝试链路 (Attempts) */}
              {activeLog.attempts && activeLog.attempts.length > 0 && (
                <div className="space-y-1.5">
                  <h4 className="text-xs font-bold text-zinc-900 dark:text-zinc-100 uppercase tracking-wider flex items-center gap-1.5">
                    <Layers className="w-3.5 h-3.5 text-indigo-500" />
                    <span>上游服务商尝试链路 ({activeLog.attempts.length})</span>
                  </h4>
                  <div className="space-y-2">
                    {activeLog.attempts.map((att, index) => (
                      <div
                        key={index}
                        className="p-3 rounded-xl bg-zinc-50 dark:bg-zinc-850/50 border border-zinc-200/60 dark:border-zinc-800 text-xs font-mono space-y-1"
                      >
                        <div className="flex items-center justify-between">
                          <span className="font-bold text-zinc-900 dark:text-zinc-100">
                            #{index + 1} {att.providerName}
                          </span>
                          <span
                            className={`px-1.5 py-0.2 rounded text-[10px] border ${getStatusBadge(
                              att.status,
                            )}`}
                          >
                            {att.status} · {att.latencyMs}ms
                          </span>
                        </div>
                        <div className="text-zinc-500 text-[11px]">模型：{att.model}</div>
                        {att.error && (
                          <div className="text-rose-600 dark:text-rose-400 text-[11px] pt-1 border-t border-zinc-200/40 dark:border-zinc-700/40 break-all">
                            错误：{att.error}
                          </div>
                        )}
                      </div>
                    ))}
                  </div>
                </div>
              )}

              {/* 报错信息 */}
              {activeLog.errorMessage && (
                <div className="space-y-1.5">
                  <div className="flex items-center justify-between">
                    <h4 className="text-xs font-bold text-rose-600 dark:text-rose-400 uppercase tracking-wider flex items-center gap-1">
                      <AlertCircle className="w-3.5 h-3.5" />
                      <span>上游错误回包</span>
                    </h4>
                    <button
                      onClick={() => copyErrorText(activeLog.errorMessage!)}
                      className="text-[11px] text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-100 flex items-center gap-1 cursor-pointer"
                    >
                      {copiedError ? <Check className="w-3 h-3 text-emerald-500" /> : <Copy className="w-3 h-3" />}
                      <span>复制错误信息</span>
                    </button>
                  </div>
                  <div className="p-3 rounded-xl bg-rose-50/50 dark:bg-rose-950/30 border border-rose-200 dark:border-rose-900/50 text-rose-700 dark:text-rose-300 font-mono text-[11px] leading-relaxed break-all max-h-48 overflow-y-auto">
                    {activeLog.errorMessage}
                  </div>
                </div>
              )}
            </div>

            {/* 抽屉底部快捷 cURL 复制 */}
            <div className="px-6 py-3.5 border-t border-zinc-200/80 dark:border-zinc-800 bg-zinc-50/50 dark:bg-zinc-900/50 flex items-center justify-end shrink-0">
              <button
                onClick={() => copyCurl(activeLog)}
                className="flex items-center gap-1.5 px-4 py-2 rounded-xl text-xs font-semibold text-white bg-indigo-600 hover:bg-indigo-700 shadow-xs transition-colors cursor-pointer"
              >
                {copiedCurl ? <Check className="w-3.5 h-3.5" /> : <Terminal className="w-3.5 h-3.5" />}
                <span>{copiedCurl ? '已复制 cURL 命令' : '复制为 cURL'}</span>
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
