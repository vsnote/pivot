import React, { useState } from 'react';
import { useAppStore } from '@/store/useAppStore';
import { Select } from '@/components/common/Select';
import type { RequestLog } from '@/types';
import {
  Trash2,
  ChevronRight,
  Copy,
  Check,
  ArrowLeft,
  AlertCircle,
  Terminal,
  Layers,
  Activity,
  Sparkles,
  Zap,
  AlertTriangle,
} from 'lucide-react';

function formatClientType(type: string) {
  switch (type) {
    case 'claude_code':
      return 'Claude Code';
    case 'codex_cli':
      return 'Codex CLI';
    case 'gemini_cli':
      return 'Gemini CLI';
    case 'openai_api':
      return 'OpenAI API';
    case 'anthropic_api':
      return 'Anthropic API';
    case 'generic':
      return '通用客户端';
    default:
      return type;
  }
}

export const RequestLogs: React.FC = () => {
  const { logs, clearLogs, stats, setActiveTab } = useAppStore();
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

  const formatTokens = (tokens: number) => {
    if (tokens >= 1_000_000) return `${(tokens / 1_000_000).toFixed(2)} M`;
    if (tokens >= 1_000) return `${(tokens / 1_000).toFixed(1)} K`;
    return tokens.toString();
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

  const copyErrorText = (errorText: string) => {
    navigator.clipboard.writeText(errorText);
    setCopiedError(true);
    setTimeout(() => setCopiedError(false), 2000);
  };

  const totalTokens = stats.totalPromptTokens + stats.totalCompletionTokens;

  // ==========================================
  // 视图 2：全尺寸页面级「日志详情」视图
  // ==========================================
  if (activeLog) {
    return (
      <div className="w-full pb-10 space-y-4 animate-in fade-in duration-150">
        {/* 顶部返回导航栏 */}
        <div className="flex items-center justify-between pb-1">
          <button
            type="button"
            onClick={() => setActiveLog(null)}
            className="inline-flex items-center gap-1.5 px-2.5 py-1 -ml-2.5 rounded-lg text-xs font-medium text-zinc-500 hover:text-zinc-900 dark:text-zinc-400 dark:hover:text-zinc-100 hover:bg-zinc-100 dark:hover:bg-zinc-800 transition-colors cursor-pointer"
          >
            <ArrowLeft className="w-3.5 h-3.5" />
            <span>返回</span>
          </button>

          <div className="flex items-center gap-2">
            <button
              onClick={() => copyCurl(activeLog)}
              className="inline-flex items-center gap-1.5 px-3 py-1 rounded-lg text-xs font-medium text-white bg-sky-600 hover:bg-sky-500 dark:bg-cyan-600 dark:hover:bg-cyan-500 shadow-2xs transition-colors cursor-pointer"
            >
              {copiedCurl ? <Check className="w-3.5 h-3.5" /> : <Terminal className="w-3.5 h-3.5" />}
              <span>{copiedCurl ? '已复制 cURL' : '复制为 cURL'}</span>
            </button>
          </div>
        </div>

        {/* 详情主卡片 */}
        <div className="p-6 rounded-lg border border-zinc-200/80 dark:border-zinc-800 bg-white dark:bg-zinc-900 shadow-xs space-y-6">
          {/* 请求概览头 */}
          <div className="flex items-center justify-between pb-4 border-b border-zinc-100 dark:border-zinc-800">
            <div className="flex items-center gap-3">
              <span className={`px-2.5 py-0.5 rounded-md text-xs font-mono font-bold border ${getStatusBadge(activeLog.statusCode)}`}>
                {activeLog.statusCode}
              </span>
              <div>
                <h3 className="text-sm font-bold text-zinc-950 dark:text-zinc-50">
                  {activeLog.method} {activeLog.path}
                </h3>
                <span className="text-[11px] font-mono text-zinc-400">
                  {formatTimestamp(activeLog.timestamp)} · ID: {activeLog.id}
                </span>
              </div>
            </div>
          </div>

          {/* 核心指标栅格 */}
          <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
            <div className="p-3.5 rounded-md bg-zinc-50 dark:bg-zinc-850/60 border border-zinc-200/60 dark:border-zinc-800">
              <span className="text-[11px] text-zinc-400 block mb-0.5">耗时</span>
              <span className="text-base font-mono font-bold text-zinc-900 dark:text-zinc-100">
                {activeLog.latencyMs} ms
              </span>
            </div>
            <div className="p-3.5 rounded-md bg-zinc-50 dark:bg-zinc-850/60 border border-zinc-200/60 dark:border-zinc-800">
              <span className="text-[11px] text-zinc-400 block mb-0.5">Token 总用量</span>
              <span className="text-base font-mono font-bold text-zinc-900 dark:text-zinc-100">
                {(activeLog.promptTokens || 0) + (activeLog.completionTokens || 0)}
              </span>
              <span className="text-[10px] text-zinc-400 block mt-0.5">
                ({activeLog.promptTokens || 0} 输入 / {activeLog.completionTokens || 0} 输出)
              </span>
            </div>
            <div className="p-3.5 rounded-md bg-zinc-50 dark:bg-zinc-850/60 border border-zinc-200/60 dark:border-zinc-800">
              <span className="text-[11px] text-zinc-400 block mb-0.5">客户端</span>
              <span className="text-base font-semibold text-zinc-900 dark:text-zinc-100">
                {formatClientType(activeLog.clientType)}
              </span>
            </div>
          </div>

          {/* 路由决策 */}
          <div className="space-y-2">
            <h4 className="text-xs font-bold text-zinc-900 dark:text-zinc-100 uppercase tracking-wider">
              请求路由决策
            </h4>
            <div className="p-4 rounded-md bg-zinc-50 dark:bg-zinc-850/60 border border-zinc-200/60 dark:border-zinc-800 space-y-2.5 text-xs font-mono">
              <div className="flex justify-between">
                <span className="text-zinc-400">请求模型 (Incoming Model)</span>
                <span className="font-semibold text-zinc-900 dark:text-zinc-100">
                  {activeLog.incomingModel}
                </span>
              </div>
              <div className="flex justify-between">
                <span className="text-zinc-400">最终服务商 (Routed Provider)</span>
                <span className="font-semibold text-sky-600 dark:text-cyan-400">
                  {activeLog.routedProviderName || activeLog.routedProviderId}
                </span>
              </div>
              <div className="flex justify-between">
                <span className="text-zinc-400">最终分发模型 (Routed Model)</span>
                <span className="font-semibold text-zinc-900 dark:text-zinc-100">
                  {activeLog.routedModel}
                </span>
              </div>
            </div>
          </div>

          {/* 尝试链路 (Attempts) */}
          {activeLog.attempts && activeLog.attempts.length > 0 && (
            <div className="space-y-2">
              <h4 className="text-xs font-bold text-zinc-900 dark:text-zinc-100 uppercase tracking-wider flex items-center gap-1.5">
                <Layers className="w-3.5 h-3.5 text-sky-600 dark:text-cyan-400" />
                <span>上游服务商尝试链路 ({activeLog.attempts.length})</span>
              </h4>
              <div className="space-y-2">
                {activeLog.attempts.map((att, index) => (
                  <div
                    key={index}
                    className="p-3.5 rounded-md bg-zinc-50 dark:bg-zinc-850/50 border border-zinc-200/60 dark:border-zinc-800 text-xs font-mono space-y-1.5"
                  >
                    <div className="flex items-center justify-between">
                      <span className="font-bold text-zinc-900 dark:text-zinc-100">
                        #{index + 1} {att.providerName}
                      </span>
                      <span className={`px-2 py-0.5 rounded text-[10px] border ${getStatusBadge(att.status)}`}>
                        {att.status} · {att.latencyMs}ms
                      </span>
                    </div>
                    <div className="text-zinc-500 text-[11px]">模型：{att.model}</div>
                    {att.error && (
                      <div className="text-rose-600 dark:text-rose-400 text-[11px] pt-1.5 border-t border-zinc-200/40 dark:border-zinc-700/40 break-all">
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
            <div className="space-y-2">
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
              <div className="p-3.5 rounded-md bg-rose-50/50 dark:bg-rose-950/30 border border-rose-200 dark:border-rose-900/50 text-rose-700 dark:text-rose-300 font-mono text-[11px] leading-relaxed break-all max-h-60 overflow-y-auto">
                {activeLog.errorMessage}
              </div>
            </div>
          )}
        </div>
      </div>
    );
  }

  // ==========================================
  // 视图 1：日志主列表（上方整合统计卡片组）
  // ==========================================
  return (
    <div className="w-full pb-10 space-y-4 animate-in fade-in duration-150">
      {/* 顶部纯粹的「返回」栏 */}
      <div className="flex items-center justify-between pb-1">
        <button
          type="button"
          onClick={() => setActiveTab('providers')}
          className="inline-flex items-center gap-1.5 px-2.5 py-1 -ml-2.5 rounded-lg text-xs font-medium text-zinc-500 hover:text-zinc-900 dark:text-zinc-400 dark:hover:text-zinc-100 hover:bg-zinc-100 dark:hover:bg-zinc-800 transition-colors cursor-pointer"
        >
          <ArrowLeft className="w-3.5 h-3.5" />
          <span>返回</span>
        </button>
      </div>

      {/* 紧凑一体化指标条（直线直角 + 零间距 + 精细分割） */}
      <div className="rounded-lg border border-zinc-200/80 dark:border-zinc-800 bg-white dark:bg-zinc-900 shadow-xs grid grid-cols-2 sm:grid-cols-4 divide-y sm:divide-y-0 sm:divide-x divide-zinc-100 dark:divide-zinc-800/80 overflow-hidden">
        {/* 请求总数 */}
        <div className="px-4 py-2.5 flex items-center justify-between">
          <div className="min-w-0">
            <span className="text-[11px] text-zinc-400 dark:text-zinc-500">请求总数</span>
            <div className="text-sm sm:text-base font-bold font-mono text-zinc-950 dark:text-white mt-0.5">
              {stats.totalRequests}
            </div>
          </div>
          <Activity className="w-4 h-4 text-cyan-500 shrink-0" />
        </div>

        {/* Token 用量 */}
        <div className="px-4 py-2.5 flex items-center justify-between">
          <div className="min-w-0">
            <span className="text-[11px] text-zinc-400 dark:text-zinc-500">Token 用量</span>
            <div className="text-sm sm:text-base font-bold font-mono text-zinc-950 dark:text-white mt-0.5">
              {formatTokens(totalTokens)}
            </div>
          </div>
          <Sparkles className="w-4 h-4 text-sky-500 shrink-0" />
        </div>

        {/* 进行中并发 */}
        <div className="px-4 py-2.5 flex items-center justify-between">
          <div className="min-w-0">
            <span className="text-[11px] text-zinc-400 dark:text-zinc-500">进行中并发</span>
            <div className="text-sm sm:text-base font-bold font-mono text-zinc-950 dark:text-white mt-0.5">
              {stats.activeRequests}
            </div>
          </div>
          <Zap className="w-4 h-4 text-amber-500 shrink-0" />
        </div>

        {/* 异常与失败 */}
        <div className="px-4 py-2.5 flex items-center justify-between">
          <div className="min-w-0">
            <span className="text-[11px] text-zinc-400 dark:text-zinc-500">异常与失败</span>
            <div className="text-sm sm:text-base font-bold font-mono text-zinc-950 dark:text-white mt-0.5">
              {stats.errorCount}
            </div>
          </div>
          <AlertTriangle className="w-4 h-4 text-rose-500 shrink-0" />
        </div>
      </div>

      {/* 日志一体化控制面板 */}
      <div className="rounded-lg border border-zinc-200/80 dark:border-zinc-800 bg-white dark:bg-zinc-900 shadow-xs overflow-hidden">
        {/* 顶部筛选与清空工具栏 */}
        <div className="flex flex-wrap items-center justify-between gap-3 px-4 py-2.5 bg-zinc-50/60 dark:bg-zinc-850/40 border-b border-zinc-200/80 dark:border-zinc-800">
          <div className="flex items-center gap-2">
            <Select
              value={selectedClient}
              onChange={(e) => setSelectedClient(e.target.value)}
            >
              <option value="all">全部客户端</option>
              <option value="claude_code">Claude Code</option>
              <option value="codex_cli">Codex CLI</option>
              <option value="gemini_cli">Gemini CLI</option>
              <option value="openai_api">OpenAI API 直连</option>
              <option value="anthropic_api">Anthropic API 直连</option>
              <option value="generic">通用客户端</option>
            </Select>

            <Select
              value={selectedStatus}
              onChange={(e) => setSelectedStatus(e.target.value)}
            >
              <option value="all">全部状态</option>
              <option value="success">成功 (2xx)</option>
              <option value="error">错误 (4xx/5xx)</option>
            </Select>
          </div>

          {logs.length > 0 && (
            <button
              onClick={() => clearLogs()}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-medium text-zinc-600 dark:text-zinc-400 hover:text-rose-600 dark:hover:text-rose-400 hover:bg-rose-50 dark:hover:bg-rose-950/30 border border-zinc-200/80 dark:border-zinc-800 transition-colors cursor-pointer"
            >
              <Trash2 className="w-3.5 h-3.5" />
              <span>清空日志</span>
            </button>
          )}
        </div>

        {/* 日志列表主体 */}
        {filteredLogs.length === 0 ? (
          <div className="p-12 text-center text-zinc-400 text-xs">
            {logs.length === 0 ? '暂无请求日志，发起请求后将在此自动记录' : '没有符合筛选条件的日志'}
          </div>
        ) : (
          <div className="divide-y divide-zinc-100 dark:divide-zinc-800">
            {filteredLogs.map((log) => (
              <div
                key={log.id}
                onClick={() => setActiveLog(log)}
                className="group p-3.5 sm:p-4 hover:bg-zinc-50/80 dark:hover:bg-zinc-800/50 transition-colors cursor-pointer flex items-center justify-between gap-4 select-none"
              >
                <div className="flex items-center gap-3.5 min-w-0">
                  <span className={`px-2 py-0.5 rounded text-xs font-mono font-bold shrink-0 border ${getStatusBadge(log.statusCode)}`}>
                    {log.statusCode}
                  </span>

                  <div className="min-w-0">
                    <div className="flex items-center gap-1.5 text-xs font-mono truncate">
                      <span className="font-semibold text-zinc-900 dark:text-zinc-100 group-hover:text-cyan-600 dark:group-hover:text-cyan-400 transition-colors truncate">
                        {log.incomingModel}
                      </span>
                      <span className="text-zinc-400">→</span>
                      <span className="text-sky-600 dark:text-cyan-400 truncate">
                        {log.routedProviderName || log.routedProviderId}
                      </span>
                      {log.routedModel && log.routedModel !== log.incomingModel && (
                        <span className="text-zinc-400 text-[11px] truncate">
                          ({log.routedModel})
                        </span>
                      )}
                    </div>
                    <div className="flex items-center gap-2 mt-0.5 text-[11px] text-zinc-400">
                      <span>{formatTimestamp(log.timestamp)}</span>
                      <span>·</span>
                      <span>{formatClientType(log.clientType)}</span>
                      <span>·</span>
                      <span className="font-mono">{log.path}</span>
                    </div>
                  </div>
                </div>

                <div className="flex items-center gap-3 shrink-0">
                  <div className="text-right font-mono">
                    <div className="text-xs font-semibold text-zinc-900 dark:text-zinc-100">
                      {log.latencyMs} ms
                    </div>
                    <div className="text-[11px] text-zinc-400">
                      {(log.promptTokens || 0) + (log.completionTokens || 0)} tokens
                    </div>
                  </div>
                  <ChevronRight className="w-4 h-4 text-zinc-300 dark:text-zinc-600 group-hover:text-zinc-500 dark:group-hover:text-zinc-300 group-hover:translate-x-0.5 transition-all" />
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
};
