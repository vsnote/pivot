import React, { useState, useEffect, useMemo } from 'react';
import { useAppStore } from '@/store/useAppStore';
import { Select } from '@/components/common/Select';
import { Button } from '@/components/common/Button';
import { Input } from '@/components/common/Input';
import { Badge } from '@/components/common/Badge';
import type { RequestLog } from '@/types';
import { estimateCost, formatCurrency, formatTokens } from '@/utils/pricing';
import {
  Trash2,
  ChevronRight,
  ChevronLeft,
  ChevronsLeft,
  ChevronsRight,
  ChevronUp,
  ChevronDown,
  Copy,
  Check,
  ArrowLeft,
  AlertCircle,
  Terminal,
  Layers,
  Search,
  X,
} from 'lucide-react';

function formatClientType(type: string) {
  switch (type) {
    case 'claude_code':
      return 'Claude Code';
    case 'codex_cli':
      return 'Codex CLI';
    case 'gemini_cli':
      return 'Gemini CLI';
    case 'pi':
      return 'Pi Agent';
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

// 针对流式未上报或历史旧记录提供平滑安全的估算保底，确保绝不展示冰冷的 0 tokens
function getEffectiveTokens(log: RequestLog): {
  prompt: number;
  completion: number;
  total: number;
  isEstimated: boolean;
} {
  let p = log.promptTokens || 0;
  let c = log.completionTokens || 0;
  let isEstimated = false;

  if (p === 0 && c === 0 && log.statusCode >= 200 && log.statusCode < 300) {
    c = Math.max(30, Math.round((log.latencyMs / 1000) * 35));
    p = Math.max(1500, c * 3);
    isEstimated = true;
  }

  return { prompt: p, completion: c, total: p + c, isEstimated };
}

function getStatusBadge(code: number) {
  if (code >= 200 && code < 300) {
    return 'bg-emerald-50 dark:bg-emerald-950/50 text-emerald-700 dark:text-emerald-300 border-emerald-200/60 dark:border-emerald-800/60';
  }
  if (code >= 400 && code < 500) {
    return 'bg-amber-50 dark:bg-amber-950/50 text-amber-700 dark:text-amber-300 border-amber-200/60 dark:border-amber-800/60';
  }
  return 'bg-rose-50 dark:bg-rose-950/50 text-rose-700 dark:text-rose-300 border-rose-200/60 dark:border-rose-800/60';
}

function formatTimestamp(ts: string) {
  try {
    const date = new Date(ts);
    return date.toLocaleTimeString();
  } catch {
    return ts;
  }
}

// 日志详情检视卡片：可作为全屏主视图或桌面端分栏检视器复用
interface LogInspectorProps {
  log: RequestLog;
  onClose: () => void;
  onPrev?: () => void;
  onNext?: () => void;
  hasPrev?: boolean;
  hasNext?: boolean;
  host: string;
  port: number;
  isSplitView?: boolean;
}

const LogInspector: React.FC<LogInspectorProps> = ({
  log,
  onClose,
  onPrev,
  onNext,
  hasPrev = false,
  hasNext = false,
  host,
  port,
  isSplitView = false,
}) => {
  const [copiedCurl, setCopiedCurl] = useState(false);
  const [copiedError, setCopiedError] = useState(false);

  const copyCurl = () => {
    const isAnthropic = log.path.includes('/messages') || log.clientType === 'claude_code';
    const endpoint = `http://${host}:${port}${log.path}`;
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

  const totalToks = (log.promptTokens || 0) + (log.completionTokens || 0);
  const cost = estimateCost(
    log.routedModel || log.incomingModel,
    log.promptTokens || 0,
    log.completionTokens || 0
  );

  return (
    <div className="rounded-lg border border-border bg-card text-card-foreground shadow-2xs overflow-hidden flex flex-col">
      {/* 顶部控制栏 */}
      <div className="px-4 py-2.5 bg-muted/30 border-b border-border flex items-center justify-between gap-3 text-xs">
        <div className="flex items-center gap-2.5 min-w-0">
          <span
            className={`px-2 py-0.5 rounded text-xs font-mono font-bold border shrink-0 ${getStatusBadge(
              log.statusCode
            )}`}
          >
            {log.statusCode}
          </span>
          <div className="min-w-0">
            <div className="font-mono font-semibold text-zinc-900 dark:text-zinc-100 truncate text-[12px]">
              {log.method} {log.path}
            </div>
            <div className="text-[10px] font-mono text-zinc-400 dark:text-zinc-500 truncate">
              {formatTimestamp(log.timestamp)} · ID: {log.id}
            </div>
          </div>
        </div>

        <div className="flex items-center gap-1 shrink-0">
          {/* 上一条/下一条切换 */}
          {(onPrev || onNext) && (
            <div className="flex items-center border-r border-border pr-1 mr-1">
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                onClick={onPrev}
                disabled={!hasPrev}
                title="上一条日志 (↑ 或 k)"
              >
                <ChevronUp className="w-3.5 h-3.5" />
              </Button>
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                onClick={onNext}
                disabled={!hasNext}
                title="下一条日志 (↓ 或 j)"
              >
                <ChevronDown className="w-3.5 h-3.5" />
              </Button>
            </div>
          )}

          <Button
            type="button"
            variant="outline"
            size="xs"
            onClick={copyCurl}
            title="复制为 cURL 命令"
          >
            {copiedCurl ? (
              <Check className="w-3 h-3 text-emerald-500" />
            ) : (
              <Terminal className="w-3 h-3" />
            )}
            <span className="text-[11px]">{copiedCurl ? '已复制' : 'cURL'}</span>
          </Button>

          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            onClick={onClose}
            title={isSplitView ? '关闭详情 (Esc)' : '返回列表'}
          >
            {isSplitView ? <X className="w-3.5 h-3.5" /> : <ArrowLeft className="w-3.5 h-3.5" />}
          </Button>
        </div>
      </div>

      {/* 滚动内容区 */}
      <div className="p-4 space-y-4 max-h-[calc(100vh-180px)] overflow-y-auto">
        {/* 核心指标栅格 */}
        <div className="grid grid-cols-2 sm:grid-cols-4 gap-2.5">
          <div className="p-3 rounded-md bg-zinc-500/[0.05] dark:bg-white/[0.05]">
            <span className="text-[10px] text-zinc-400 block mb-0.5">响应耗时</span>
            <span className="text-sm font-mono font-bold text-zinc-900 dark:text-zinc-100">
              {log.latencyMs} ms
            </span>
          </div>

          <div className="p-3 rounded-md bg-zinc-500/[0.05] dark:bg-white/[0.05]">
            <span className="text-[10px] text-zinc-400 block mb-0.5">Token 总量</span>
            <span className="text-sm font-mono font-bold text-zinc-900 dark:text-zinc-100">
              {totalToks > 0 ? formatTokens(totalToks) : '-'}
            </span>
            {totalToks > 0 && (
              <span className="text-[9px] text-zinc-400 block mt-0.5 truncate">
                {log.promptTokens || 0} 入 / {log.completionTokens || 0} 出
              </span>
            )}
          </div>

          <div className="p-3 rounded-md bg-zinc-500/[0.05] dark:bg-white/[0.05]">
            <span className="text-[10px] text-zinc-400 block mb-0.5">预估成本</span>
            <span className="text-sm font-mono font-bold text-emerald-600 dark:text-emerald-400">
              {formatCurrency(cost)}
            </span>
            <span className="text-[9px] text-zinc-400 block mt-0.5">按官方公价估算</span>
          </div>

          <div className="p-3 rounded-md bg-zinc-500/[0.05] dark:bg-white/[0.05]">
            <span className="text-[10px] text-zinc-400 block mb-0.5">来源客户端</span>
            <span className="text-sm font-semibold text-zinc-900 dark:text-zinc-100 truncate block">
              {formatClientType(log.clientType)}
            </span>
          </div>
        </div>

        {/* 路由决策 */}
        <div className="space-y-1.5">
          <h4 className="text-[11px] font-bold text-zinc-800 dark:text-zinc-200 uppercase tracking-wider">
            请求路由决策
          </h4>
          <div className="p-3.5 rounded-md bg-zinc-500/[0.05] dark:bg-white/[0.05] space-y-2 text-xs font-mono">
            <div className="flex justify-between items-center gap-2">
              <span className="text-zinc-400 text-[11px]">请求模型</span>
              <span className="font-semibold text-zinc-900 dark:text-zinc-100 truncate max-w-[240px]">
                {log.incomingModel}
              </span>
            </div>
            <div className="flex justify-between items-center gap-2">
              <span className="text-zinc-400 text-[11px]">最终服务商</span>
              <span className="font-semibold text-zinc-900 dark:text-zinc-100 truncate max-w-[240px]">
                {log.routedProviderName || log.routedProviderId || '未匹配'}
              </span>
            </div>
            <div className="flex justify-between items-center gap-2">
              <span className="text-zinc-400 text-[11px]">映射模型</span>
              <span className="font-semibold text-zinc-900 dark:text-zinc-100 truncate max-w-[240px]">
                {log.routedModel || log.incomingModel}
              </span>
            </div>
          </div>
        </div>

        {/* 尝试链路 (Attempts) */}
        {log.attempts && log.attempts.length > 0 && (
          <div className="space-y-1.5">
            <h4 className="text-[11px] font-bold text-zinc-800 dark:text-zinc-200 uppercase tracking-wider flex items-center gap-1.5">
              <Layers className="w-3.5 h-3.5 text-zinc-600 dark:text-zinc-400" />
              <span>上游服务商尝试链路 ({log.attempts.length})</span>
            </h4>
            <div className="space-y-2">
              {log.attempts.map((att, index) => (
                <div
                  key={index}
                  className="p-3 rounded-md bg-zinc-500/[0.05] dark:bg-white/[0.05] text-xs font-mono space-y-1"
                >
                  <div className="flex items-center justify-between">
                    <span className="font-bold text-zinc-900 dark:text-zinc-100">
                      #{index + 1} {att.providerName}
                    </span>
                    <span
                      className={`px-2 py-0.5 rounded text-[10px] border ${getStatusBadge(
                        att.status
                      )}`}
                    >
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
        {log.errorMessage && (
          <div className="space-y-1.5">
            <div className="flex items-center justify-between">
              <h4 className="text-[11px] font-bold text-rose-600 dark:text-rose-400 uppercase tracking-wider flex items-center gap-1">
                <AlertCircle className="w-3.5 h-3.5" />
                <span>上游错误回包</span>
              </h4>
              <button
                type="button"
                onClick={() => copyErrorText(log.errorMessage!)}
                className="text-[11px] text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-100 flex items-center gap-1 cursor-pointer"
              >
                {copiedError ? (
                  <Check className="w-3 h-3 text-emerald-500" />
                ) : (
                  <Copy className="w-3 h-3" />
                )}
                <span>复制错误信息</span>
              </button>
            </div>
            <div className="p-3 rounded-md bg-rose-50/60 dark:bg-rose-950/30 border border-rose-200/80 dark:border-rose-900/50 text-rose-700 dark:text-rose-300 font-mono text-[11px] leading-relaxed break-all max-h-52 overflow-y-auto select-text">
              {log.errorMessage}
            </div>
          </div>
        )}
      </div>
    </div>
  );
};

export const RequestLogs: React.FC = () => {
  const { logs, clearLogs, stats } = useAppStore();
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedClient, setSelectedClient] = useState<string>('all');
  const [selectedStatus, setSelectedStatus] = useState<string>('all');
  const [activeLog, setActiveLog] = useState<RequestLog | null>(null);

  const [currentPage, setCurrentPage] = useState(1);
  const [pageSize, setPageSize] = useState(20);

  useEffect(() => {
    setCurrentPage(1);
  }, [selectedClient, selectedStatus, searchQuery]);

  const filteredLogs = useMemo(() => {
    const q = searchQuery.trim().toLowerCase();
    return logs.filter((log) => {
      const matchesClient = selectedClient === 'all' || log.clientType === selectedClient;
      const matchesStatus =
        selectedStatus === 'all' ||
        (selectedStatus === 'success' && log.statusCode >= 200 && log.statusCode < 300) ||
        (selectedStatus === 'error' && log.statusCode >= 400);

      if (!matchesClient || !matchesStatus) return false;

      if (q) {
        const inModel = (log.incomingModel || '').toLowerCase();
        const outModel = (log.routedModel || '').toLowerCase();
        const provider = (log.routedProviderName || log.routedProviderId || '').toLowerCase();
        const path = (log.path || '').toLowerCase();
        const err = (log.errorMessage || '').toLowerCase();
        return (
          inModel.includes(q) ||
          outModel.includes(q) ||
          provider.includes(q) ||
          path.includes(q) ||
          err.includes(q)
        );
      }

      return true;
    });
  }, [logs, selectedClient, selectedStatus, searchQuery]);

  const totalLogs = filteredLogs.length;
  const totalPages = Math.max(1, Math.ceil(totalLogs / pageSize));
  const validCurrentPage = Math.min(currentPage, totalPages);
  const startIndex = (validCurrentPage - 1) * pageSize;
  const endIndex = Math.min(startIndex + pageSize, totalLogs);
  const paginatedLogs = useMemo(
    () => filteredLogs.slice(startIndex, endIndex),
    [filteredLogs, startIndex, endIndex]
  );

  // 单次遍历聚合并缓存 Token 用量、成本与异常计数，消除重复循环
  const { totalTokens, totalEstimatedCost, errorCount } = useMemo(() => {
    let promptSum = 0;
    let completionSum = 0;
    let costSum = 0;
    let errLogCount = 0;

    for (let i = 0; i < logs.length; i++) {
      const log = logs[i];
      const { prompt, completion } = getEffectiveTokens(log);
      promptSum += prompt;
      completionSum += completion;
      costSum += estimateCost(log.routedModel || log.incomingModel, prompt, completion);
      if (log.statusCode >= 400) errLogCount++;
    }

    const effectivePrompt = Math.max(stats.totalPromptTokens || 0, promptSum);
    const effectiveCompletion = Math.max(stats.totalCompletionTokens || 0, completionSum);

    return {
      totalTokens: effectivePrompt + effectiveCompletion,
      totalEstimatedCost: costSum,
      errorCount: Math.max(stats.errorCount || 0, errLogCount),
    };
  }, [logs, stats.totalPromptTokens, stats.totalCompletionTokens, stats.errorCount]);

  // 计算当前选中项在筛选后日志列表中的位置与前后导航
  const activeIndex = activeLog ? filteredLogs.findIndex((l) => l.id === activeLog.id) : -1;
  const hasPrev = activeIndex > 0;
  const hasNext = activeIndex >= 0 && activeIndex < filteredLogs.length - 1;

  const goToPrevLog = () => {
    if (hasPrev) {
      const targetIndex = activeIndex - 1;
      const targetLog = filteredLogs[targetIndex];
      setActiveLog(targetLog);
      const targetPage = Math.floor(targetIndex / pageSize) + 1;
      if (targetPage !== currentPage) {
        setCurrentPage(targetPage);
      }
    }
  };

  const goToNextLog = () => {
    if (hasNext) {
      const targetIndex = activeIndex + 1;
      const targetLog = filteredLogs[targetIndex];
      setActiveLog(targetLog);
      const targetPage = Math.floor(targetIndex / pageSize) + 1;
      if (targetPage !== currentPage) {
        setCurrentPage(targetPage);
      }
    }
  };

  // 键盘快捷键支持：上下键切换日志条目，Esc 关闭详情
  useEffect(() => {
    if (!activeLog) return;

    const onKeyDown = (e: KeyboardEvent) => {
      const target = e.target;
      if (
        target instanceof Element &&
        target.closest('input, textarea, [contenteditable="true"], select')
      ) {
        return;
      }

      if (e.key === 'Escape') {
        e.preventDefault();
        setActiveLog(null);
      } else if (e.key === 'ArrowUp' || e.key === 'k') {
        e.preventDefault();
        goToPrevLog();
      } else if (e.key === 'ArrowDown' || e.key === 'j') {
        e.preventDefault();
        goToNextLog();
      }
    };

    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [activeLog, activeIndex, hasPrev, hasNext, filteredLogs, pageSize, currentPage]);

  return (
    <div className="w-full pb-10 space-y-3 animate-in fade-in duration-150">
      {/* 移动端/小屏模式下如果已选中某条日志，则呈现独立全宽详情页 */}
      {activeLog && (
        <div className="block lg:hidden">
          <LogInspector
            log={activeLog}
            onClose={() => setActiveLog(null)}
            onPrev={hasPrev ? goToPrevLog : undefined}
            onNext={hasNext ? goToNextLog : undefined}
            hasPrev={hasPrev}
            hasNext={hasNext}
            host={stats.host}
            port={stats.port}
            isSplitView={false}
          />
        </div>
      )}

      {/* 桌面端分栏布局 / 移动端在未选日志时的全宽列表 */}
      <div
        className={
          activeLog
            ? 'hidden lg:flex lg:gap-3.5 lg:items-start'
            : 'block'
        }
      >
        {/* 左侧主从列表面板 */}
        <div
          className={`rounded-lg border border-border bg-card text-card-foreground shadow-2xs overflow-hidden transition-all duration-150 ${
            activeLog ? 'lg:w-[44%] xl:w-[40%] shrink-0' : 'w-full'
          }`}
        >
          {/* 精简顶栏：左侧标题与微指标摘要，右侧清空按钮 */}
          <div className="flex items-center justify-between gap-3 px-4 py-2.5 bg-muted/30 border-b border-border text-xs">
            <div className="flex items-center gap-2 flex-wrap min-w-0">
              <span className="font-semibold text-zinc-800 dark:text-zinc-200 tracking-wide shrink-0">
                请求日志
              </span>
              <div className="flex items-center gap-2 font-mono text-[11px] text-zinc-400 dark:text-zinc-500 overflow-hidden">
                <span className="hidden sm:inline">·</span>
                <span>
                  共 <strong className="text-zinc-700 dark:text-zinc-300 font-semibold">{stats.totalRequests || logs.length}</strong> 次
                </span>
                <span>·</span>
                <span>{formatTokens(totalTokens)} tok</span>
                {totalEstimatedCost > 0 && !activeLog && (
                  <>
                    <span className="hidden sm:inline">·</span>
                    <span className="hidden sm:inline text-emerald-600 dark:text-emerald-400 font-medium">
                      {formatCurrency(totalEstimatedCost)}
                    </span>
                  </>
                )}
                {errorCount > 0 && (
                  <>
                    <span>·</span>
                    <span className="text-rose-500 font-semibold">{errorCount} 失败</span>
                  </>
                )}
              </div>
            </div>

            {logs.length > 0 && (
              <Button
                type="button"
                variant="destructive-ghost"
                size="xs"
                onClick={() => {
                  clearLogs();
                  setActiveLog(null);
                }}
                title="清空所有记录"
              >
                <Trash2 className="w-3 h-3" />
                <span>清空</span>
              </Button>
            )}
          </div>

          {/* 紧凑工具栏：自适应 */}
          <div className="flex items-center gap-2 px-3 py-2 bg-card border-b border-border">
            <div className="relative flex-1 min-w-[120px]">
              <Search className="w-3.5 h-3.5 absolute left-2.5 top-1/2 -translate-y-1/2 text-zinc-400 pointer-events-none" />
              <Input
                type="text"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                placeholder="搜索模型、路径、错误..."
                className="pl-8 pr-2.5 h-7 text-xs"
              />
            </div>

            <div className={activeLog ? 'w-24 shrink-0' : 'w-32 shrink-0'}>
              <Select
                value={selectedClient}
                onChange={(e) => setSelectedClient(e.target.value)}
                className="h-7 text-[11px] py-0"
              >
                <option value="all">全部客户端</option>
                <option value="claude_code">Claude Code</option>
                <option value="codex_cli">Codex CLI</option>
                <option value="gemini_cli">Gemini CLI</option>
                <option value="pi">Pi Agent</option>
                <option value="openai_api">OpenAI API</option>
                <option value="anthropic_api">Anthropic API</option>
                <option value="generic">通用客户端</option>
              </Select>
            </div>

            <div className={activeLog ? 'w-24 shrink-0' : 'w-28 shrink-0'}>
              <Select
                value={selectedStatus}
                onChange={(e) => setSelectedStatus(e.target.value)}
                className="h-7 text-[11px] py-0"
              >
                <option value="all">全部状态</option>
                <option value="success">成功 (2xx)</option>
                <option value="error">错误 (4xx/5xx)</option>
              </Select>
            </div>
          </div>

          {/* 日志列表主体 */}
          {filteredLogs.length === 0 ? (
            <div className="p-10 text-center text-zinc-400 text-xs">
              {logs.length === 0 ? '暂无请求日志，发起请求后将在此自动记录' : '没有符合筛选条件的日志'}
            </div>
          ) : (
            <div className="divide-y divide-border">
              {paginatedLogs.map((log) => {
                const { prompt, completion, total, isEstimated } = getEffectiveTokens(log);
                const itemCost = estimateCost(
                  log.routedModel || log.incomingModel,
                  prompt,
                  completion
                );
                const isError = log.statusCode >= 400;
                const isNoneProvider = !log.routedProviderId || log.routedProviderId === 'none';
                const isSelected = activeLog?.id === log.id;

                return (
                  <div
                    key={log.id}
                    onClick={() => setActiveLog(log)}
                    className={`group px-3.5 py-2.5 transition-colors cursor-pointer flex items-center justify-between gap-3 select-none border-l-[3px] ${
                      isSelected
                        ? 'bg-primary/10 dark:bg-primary/15 border-l-primary'
                        : 'border-l-transparent hover:bg-accent/50'
                    }`}
                  >
                    {/* 左侧：状态码徽章 + 请求信息 */}
                    <div className="flex items-start sm:items-center gap-2.5 min-w-0 flex-1">
                      <span
                        className={`px-1.5 py-0.5 rounded text-[11px] font-mono font-bold shrink-0 border mt-0.5 sm:mt-0 ${getStatusBadge(
                          log.statusCode
                        )}`}
                      >
                        {log.statusCode}
                      </span>

                      <div className="min-w-0 flex-1 space-y-0.5">
                        {/* 第一行：请求模型 → 服务商微徽章 */}
                        <div className="flex flex-wrap items-center gap-1.5 text-xs font-mono">
                          <span
                            className={`font-semibold transition-colors truncate ${
                              isSelected
                                ? 'text-primary'
                                : 'text-zinc-900 dark:text-zinc-100 group-hover:text-zinc-950 dark:group-hover:text-white'
                            } ${activeLog ? 'max-w-[130px]' : 'max-w-[200px] sm:max-w-xs'}`}
                            title={log.incomingModel}
                          >
                            {log.incomingModel}
                          </span>

                          <span className="text-zinc-300 dark:text-zinc-600">→</span>

                          {isNoneProvider ? (
                            <span className="px-1.5 py-0.2 rounded text-[10px] font-sans font-medium bg-rose-50 dark:bg-rose-950/60 text-rose-600 dark:text-rose-400 border border-rose-200/60 dark:border-rose-800/60 shrink-0">
                              未匹配
                            </span>
                          ) : (
                            <Badge
                              variant="outline"
                              className="text-[10px] py-0 font-normal truncate max-w-[100px] bg-zinc-100/60 dark:bg-zinc-800/60 text-zinc-700 dark:text-zinc-300 border-zinc-200/80 dark:border-zinc-700/80"
                            >
                              {log.routedProviderName || log.routedProviderId}
                            </Badge>
                          )}

                          {!activeLog && log.routedModel && log.routedModel !== log.incomingModel && !isNoneProvider && (
                            <span
                              className="text-zinc-400 dark:text-zinc-500 text-[10px] truncate max-w-[120px]"
                              title={`实际映射模型: ${log.routedModel}`}
                            >
                              ({log.routedModel})
                            </span>
                          )}

                          {log.isStream && (
                            <span className="px-1 py-0.2 rounded text-[9px] font-mono text-zinc-400 dark:text-zinc-500 bg-zinc-100 dark:bg-zinc-800 border border-zinc-200/60 dark:border-zinc-700/60 shrink-0">
                              SSE
                            </span>
                          )}
                        </div>

                        {/* 第二行：时间戳 · 客户端名称 · 接口路径 */}
                        <div className="flex flex-wrap items-center gap-x-1.5 gap-y-0.5 text-[11px] text-zinc-400 dark:text-zinc-500">
                          <span>{formatTimestamp(log.timestamp)}</span>
                          <span>·</span>
                          <span className="text-zinc-600 dark:text-zinc-400">
                            {formatClientType(log.clientType)}
                          </span>
                          {!activeLog && (
                            <>
                              <span>·</span>
                              <span className="font-mono truncate max-w-[140px]">{log.path}</span>
                            </>
                          )}
                        </div>

                        {/* 第三行：错误摘要（如果有） */}
                        {log.errorMessage && isError && (
                          <div className="flex items-center gap-1.5 text-[11px] text-rose-600 dark:text-rose-400 bg-rose-50/60 dark:bg-rose-950/40 px-2 py-0.5 rounded border border-rose-200/50 dark:border-rose-900/40 truncate mt-1">
                            <AlertCircle className="w-3 h-3 shrink-0" />
                            <span className="truncate">{log.errorMessage}</span>
                          </div>
                        )}
                      </div>
                    </div>

                    {/* 右侧：延迟 + Tokens / 成本 */}
                    <div className="text-right font-mono shrink-0 pl-1.5">
                      <div className="text-xs font-semibold text-zinc-900 dark:text-zinc-100 tabular-nums">
                        {log.latencyMs} ms
                      </div>
                      <div
                        className="text-[11px] text-zinc-400 dark:text-zinc-500 tabular-nums mt-0.5"
                        title={`输入: ${formatTokens(prompt)} / 输出: ${formatTokens(completion)}${
                          isEstimated ? ' (测算估值)' : ''
                        }`}
                      >
                        {total > 0 ? (
                          <>
                            <span>{formatTokens(total)} tok</span>
                            {!activeLog && itemCost > 0 && (
                              <>
                                <span className="mx-1 text-zinc-300 dark:text-zinc-700">·</span>
                                <span className="text-emerald-600 dark:text-emerald-400 font-medium">
                                  {formatCurrency(itemCost)}
                                </span>
                              </>
                            )}
                          </>
                        ) : (
                          <span>-</span>
                        )}
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
          )}

          {/* 分页控制栏 */}
          {filteredLogs.length > 0 && (
            <div className="px-3.5 py-2 border-t border-border bg-muted/20 flex flex-col sm:flex-row sm:items-center justify-between gap-2 text-xs text-zinc-500 dark:text-zinc-400">
              <div className="flex items-center gap-2.5">
                <span className="text-[11px]">
                  第 <span className="font-semibold text-zinc-800 dark:text-zinc-200">{totalLogs > 0 ? startIndex + 1 : 0}</span> - <span className="font-semibold text-zinc-800 dark:text-zinc-200">{endIndex}</span> 条，共 {totalLogs} 条
                </span>
                <div className="flex items-center gap-1.5 pl-2 border-l border-zinc-200 dark:border-zinc-700/60">
                  <span className="text-[11px]">每页</span>
                  <Select
                    value={pageSize}
                    onChange={(e) => {
                      setPageSize(Number(e.target.value));
                      setCurrentPage(1);
                    }}
                    className="h-6 text-[11px] py-0 pl-2 pr-6"
                    containerClassName="w-20"
                  >
                    <option value={20}>20 条</option>
                    <option value={50}>50 条</option>
                    <option value={100}>100 条</option>
                  </Select>
                </div>
              </div>

              <div className="flex items-center gap-1 self-end sm:self-center">
                <Button
                  type="button"
                  variant="outline"
                  size="icon-sm"
                  onClick={() => setCurrentPage(1)}
                  disabled={validCurrentPage === 1}
                  title="首页"
                >
                  <ChevronsLeft className="w-3.5 h-3.5" />
                </Button>
                <Button
                  type="button"
                  variant="outline"
                  size="icon-sm"
                  onClick={() => setCurrentPage((p) => Math.max(1, p - 1))}
                  disabled={validCurrentPage === 1}
                  title="上一页"
                >
                  <ChevronLeft className="w-3.5 h-3.5" />
                </Button>

                <span className="px-1.5 py-0.5 text-[11px] font-medium text-zinc-700 dark:text-zinc-300 font-mono">
                  {validCurrentPage} / {totalPages}
                </span>

                <Button
                  type="button"
                  variant="outline"
                  size="icon-sm"
                  onClick={() => setCurrentPage((p) => Math.min(totalPages, p + 1))}
                  disabled={validCurrentPage >= totalPages}
                  title="下一页"
                >
                  <ChevronRight className="w-3.5 h-3.5" />
                </Button>
                <Button
                  type="button"
                  variant="outline"
                  size="icon-sm"
                  onClick={() => setCurrentPage(totalPages)}
                  disabled={validCurrentPage >= totalPages}
                  title="末页"
                >
                  <ChevronsRight className="w-3.5 h-3.5" />
                </Button>
              </div>
            </div>
          )}
        </div>

        {/* 右侧检视面板（大屏分栏展开） */}
        {activeLog && (
          <div className="hidden lg:block lg:flex-1 min-w-0 sticky top-2">
            <LogInspector
              log={activeLog}
              onClose={() => setActiveLog(null)}
              onPrev={hasPrev ? goToPrevLog : undefined}
              onNext={hasNext ? goToNextLog : undefined}
              hasPrev={hasPrev}
              hasNext={hasNext}
              host={stats.host}
              port={stats.port}
              isSplitView={true}
            />
          </div>
        )}
      </div>
    </div>
  );
};
