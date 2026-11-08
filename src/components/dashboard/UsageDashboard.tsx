import React, { useState, useMemo } from 'react';
import { useAppStore } from '@/store/useAppStore';
import { useTranslation } from '@/i18n';
import { Badge } from '@/components/common/Badge';
import { Button } from '@/components/common/Button';
import { ConfirmDialog } from '@/components/common/ConfirmDialog';
import { ProviderAvatar } from '@/components/common/BrandIcons';
import {
  OpenAISpiralIcon,
  ClaudeLogo,
  GeminiLogo,
  PiLogo,
  OpenCodeLogo,
} from '@/components/common/BrandIcons';
import { estimateCost, formatCurrency, formatTokens } from '@/utils/pricing';
import type { RequestLog, ClientType } from '@/types';
import {
  Activity,
  Layers,
  Clock,
  Sparkles,
  Server,
  Terminal,
  ShieldCheck,
  ShieldAlert,
  TrendingUp,
  Zap,
  CheckCircle2,
  AlertTriangle,
  Trash2,
  Coins,
} from 'lucide-react';

type TimeRange = 'all' | 'today' | '1h';
type SortMetric = 'count' | 'cost';

const PROVIDER_COLORS = [
  '#3b82f6', // blue
  '#8b5cf6', // violet
  '#10b981', // emerald
  '#f59e0b', // amber
  '#06b6d4', // cyan
  '#ec4899', // pink
  '#6366f1', // indigo
];

const CLIENT_COLORS: Record<string, string> = {
  claude_code: '#8b5cf6', // violet
  anthropic_api: '#8b5cf6',
  codex_cli: '#0ea5e9', // sky
  openai_api: '#10b981', // emerald
  gemini_cli: '#3b82f6', // blue
  pi: '#f97316', // orange
  opencode: '#6366f1', // indigo
  generic: '#71717a', // zinc
};

function formatClientType(type: string, t: (k: string) => string): string {
  switch (type) {
    case 'claude_code':
      return t('dashboard.charts.clientTypes.claude_code');
    case 'codex_cli':
      return t('dashboard.charts.clientTypes.codex_cli');
    case 'gemini_cli':
      return t('dashboard.charts.clientTypes.gemini_cli');
    case 'pi':
      return t('dashboard.charts.clientTypes.pi');
    case 'opencode':
      return t('dashboard.charts.clientTypes.opencode');
    case 'openai_api':
      return t('dashboard.charts.clientTypes.openai_api');
    case 'anthropic_api':
      return t('dashboard.charts.clientTypes.anthropic_api');
    case 'generic':
      return t('dashboard.charts.clientTypes.generic');
    default:
      return type;
  }
}

function ClientLogo({ type, className = 'w-4 h-4' }: { type: ClientType | string; className?: string }) {
  switch (type) {
    case 'claude_code':
    case 'anthropic_api':
      return <ClaudeLogo className={className} />;
    case 'codex_cli':
    case 'openai_api':
      return <OpenAISpiralIcon className={className} />;
    case 'gemini_cli':
      return <GeminiLogo className={className} />;
    case 'pi':
      return <PiLogo className={className} />;
    case 'opencode':
      return <OpenCodeLogo className={className} />;
    default:
      return <Terminal className={className} />;
  }
}

function getEffectiveTokens(log: RequestLog): {
  prompt: number;
  completion: number;
  total: number;
} {
  let p = log.promptTokens || 0;
  let c = log.completionTokens || 0;

  if (p === 0 && c === 0 && log.statusCode >= 200 && log.statusCode < 300) {
    c = Math.max(30, Math.round((log.latencyMs / 1000) * 35));
    p = Math.max(1500, c * 3);
  }

  return { prompt: p, completion: c, total: p + c };
}

function categorizeError(log: RequestLog, t: (k: string, p?: Record<string, string | number>) => string): string {
  const code = log.statusCode;
  const msg = (log.errorMessage || '').toLowerCase();

  if (code === 429 || msg.includes('rate_limit') || msg.includes('quota') || msg.includes('insufficient_quota')) {
    return t('dashboard.charts.errors.rateLimit');
  }
  if (code === 401 || code === 403 || msg.includes('auth') || msg.includes('key')) {
    return t('dashboard.charts.errors.authFailed');
  }
  if (code === 404 || msg.includes('model_not_found') || msg.includes('not found')) {
    return t('dashboard.charts.errors.notFound');
  }
  if (code === 502 || code === 504 || msg.includes('timeout') || msg.includes('timed out')) {
    return t('dashboard.charts.errors.timeout');
  }
  if (code >= 500) {
    return t('dashboard.charts.errors.internalError', { code });
  }
  if (log.errorMessage) {
    return log.errorMessage.slice(0, 36);
  }
  return t('dashboard.charts.errors.requestError', { code });
}

function formatUptime(secs: number, t: (k: string, p?: Record<string, string | number>) => string): string {
  if (secs <= 0) return t('dashboard.charts.uptimeFormats.justStarted');
  const hours = Math.floor(secs / 3600);
  const minutes = Math.floor((secs % 3600) / 60);
  if (hours > 0) return t('dashboard.charts.uptimeFormats.hoursMinutes', { hours, minutes });
  if (minutes > 0) return t('dashboard.charts.uptimeFormats.minutes', { minutes });
  return t('dashboard.charts.uptimeFormats.seconds', { seconds: secs });
}

function formatLatency(ms: number): { value: string; unit: string } {
  if (!ms || ms <= 0) return { value: '0', unit: 'ms' };
  if (ms < 1000) return { value: Math.round(ms).toString(), unit: 'ms' };
  if (ms < 60000) return { value: (ms / 1000).toFixed(1), unit: 's' };
  return { value: (ms / 60000).toFixed(1), unit: 'min' };
}

function parseModelDisplay(rawModel: string, targetModel?: string): {
  displayName: string;
  namespace?: string;
  targetDisplay?: string;
} {
  const slashIndex = rawModel.indexOf('/');
  let namespace: string | undefined;
  let displayName = rawModel;
  if (slashIndex > 0) {
    namespace = rawModel.slice(0, slashIndex);
    displayName = rawModel.slice(slashIndex + 1);
  }
  let targetDisplay: string | undefined;
  if (targetModel && targetModel !== rawModel && targetModel !== displayName) {
    const tSlash = targetModel.indexOf('/');
    targetDisplay = tSlash > 0 ? targetModel.slice(tSlash + 1) : targetModel;
  }
  return { displayName, namespace, targetDisplay };
}

function renderSuccessRateBadge(rate: number, count: number, locale: string) {
  if (count === 0 || rate >= 98) return null;
  if (rate >= 80) {
    return (
      <span className="inline-flex items-center px-1.5 py-0.2 rounded text-[10px] font-mono font-medium text-amber-600 dark:text-amber-400 bg-amber-500/10 border border-amber-500/25 shrink-0">
        {rate}% {locale === 'zh-CN' ? '成功' : 'success'}
      </span>
    );
  }
  return (
    <Badge variant="destructive" className="text-[10px] py-0 px-1 font-mono shrink-0">
      {rate}% {locale === 'zh-CN' ? '成功' : 'success'}
    </Badge>
  );
}

export const UsageDashboard: React.FC = () => {
  const { t, locale } = useTranslation();
  const { logs, stats, providers, clearLogs } = useAppStore();
  const [timeRange, setTimeRange] = useState<TimeRange>('all');
  const [providerSort, setProviderSort] = useState<SortMetric>('count');
  const [modelSort, setModelSort] = useState<SortMetric>('count');
  const [showClearConfirm, setShowClearConfirm] = useState(false);
  const [clearingLogs, setClearingLogs] = useState(false);

  // 根据所选时间范围筛选日志
  const filteredLogs = useMemo(() => {
    if (timeRange === 'all') return logs;
    const now = Date.now();
    if (timeRange === 'today') {
      const todayStr = new Date().toDateString();
      return logs.filter((l) => new Date(l.timestamp).toDateString() === todayStr);
    }
    if (timeRange === '1h') {
      return logs.filter((l) => now - new Date(l.timestamp).getTime() <= 3600 * 1000);
    }
    return logs;
  }, [logs, timeRange]);

  // 多维度数据聚合计算
  const metrics = useMemo(() => {
    const totalRequests = filteredLogs.length;
    let promptSum = 0;
    let completionSum = 0;
    let costSum = 0;
    let successCount = 0;
    let errorCount = 0;
    let latencySum = 0;
    let streamCount = 0;
    let fallbackCount = 0;

    // 状态码分布: 2xx, 4xx, 5xx
    let status2xx = 0;
    let status4xx = 0;
    let status5xx = 0;

    // 1. 服务商维度聚合
    const providerMap: Record<
      string,
      {
        id: string;
        name: string;
        protocol?: string;
        count: number;
        promptTokens: number;
        completionTokens: number;
        totalTokens: number;
        cost: number;
        successCount: number;
        errorCount: number;
        latencySum: number;
      }
    > = {};

    // 2. 模型使用排行聚合
    const modelMap: Record<
      string,
      {
        model: string;
        targetModel?: string;
        providerName: string;
        count: number;
        promptTokens: number;
        completionTokens: number;
        totalTokens: number;
        cost: number;
      }
    > = {};

    // 3. 客户端聚合
    const clientMap: Record<
      string,
      {
        type: string;
        count: number;
        promptTokens: number;
        completionTokens: number;
        totalTokens: number;
        cost: number;
        errorCount: number;
      }
    > = {};

    // 4. 错误诊断聚合
    const errorDiagnosticsMap: Record<string, number> = {};

    // 5. 24小时分布
    const hourlyCounts = new Array(24).fill(0);

    for (const log of filteredLogs) {
      const { prompt, completion, total } = getEffectiveTokens(log);
      promptSum += prompt;
      completionSum += completion;
      const cost = estimateCost(log.routedModel || log.incomingModel, prompt, completion);
      costSum += cost;

      const isSuccess = log.statusCode >= 200 && log.statusCode < 300;
      const isErr = log.statusCode >= 400 || Boolean(log.errorMessage);

      if (isSuccess) successCount++;
      if (isErr) errorCount++;

      if (log.statusCode >= 200 && log.statusCode < 300) status2xx++;
      else if (log.statusCode >= 400 && log.statusCode < 500) status4xx++;
      else if (log.statusCode >= 500) status5xx++;

      latencySum += log.latencyMs || 0;
      if (log.isStream) streamCount++;
      if (log.fallbackUsed || (log.attempts && log.attempts.length > 1)) fallbackCount++;

      // 1. 服务商
      const pId = log.routedProviderId || 'unknown';
      const pName =
        log.routedProviderName ||
        providers.find((p) => p.id === pId)?.name ||
        (pId === 'none' ? (locale === 'zh-CN' ? '未匹配服务商' : 'Unmatched') : pId);

      if (!providerMap[pId]) {
        const found = providers.find((p) => p.id === pId);
        providerMap[pId] = {
          id: pId,
          name: pName,
          protocol: found?.protocol,
          count: 0,
          promptTokens: 0,
          completionTokens: 0,
          totalTokens: 0,
          cost: 0,
          successCount: 0,
          errorCount: 0,
          latencySum: 0,
        };
      }
      providerMap[pId].count++;
      providerMap[pId].promptTokens += prompt;
      providerMap[pId].completionTokens += completion;
      providerMap[pId].totalTokens += total;
      providerMap[pId].cost += cost;
      providerMap[pId].latencySum += log.latencyMs || 0;
      if (isSuccess) providerMap[pId].successCount++;
      if (isErr) providerMap[pId].errorCount++;

      // 2. 模型使用排行
      const mKey = log.incomingModel || log.routedModel || 'unknown';
      if (!modelMap[mKey]) {
        modelMap[mKey] = {
          model: mKey,
          targetModel: log.routedModel || mKey,
          providerName: pName,
          count: 0,
          promptTokens: 0,
          completionTokens: 0,
          totalTokens: 0,
          cost: 0,
        };
      }
      modelMap[mKey].count++;
      modelMap[mKey].promptTokens += prompt;
      modelMap[mKey].completionTokens += completion;
      modelMap[mKey].totalTokens += total;
      modelMap[mKey].cost += cost;

      // 3. 客户端来源
      const cType = log.clientType || 'generic';
      if (!clientMap[cType]) {
        clientMap[cType] = {
          type: cType,
          count: 0,
          promptTokens: 0,
          completionTokens: 0,
          totalTokens: 0,
          cost: 0,
          errorCount: 0,
        };
      }
      clientMap[cType].count++;
      clientMap[cType].promptTokens += prompt;
      clientMap[cType].completionTokens += completion;
      clientMap[cType].totalTokens += total;
      clientMap[cType].cost += cost;
      if (isErr) clientMap[cType].errorCount++;

      // 4. 错误诊断
      if (isErr) {
        const reason = categorizeError(log, t);
        errorDiagnosticsMap[reason] = (errorDiagnosticsMap[reason] || 0) + 1;
      }

      // 5. 24小时分布
      try {
        const d = new Date(log.timestamp);
        const hour = d.getHours();
        if (hour >= 0 && hour < 24) {
          hourlyCounts[hour]++;
        }
      } catch {
        // ignore timestamp parse error
      }
    }

    // 全局兜底聚合（如果当前时间筛选为 all，融合持久化 stats 基础量）
    const effectiveTotalRequests =
      timeRange === 'all' ? Math.max(stats.totalRequests || 0, totalRequests) : totalRequests;
    const effectivePrompt =
      timeRange === 'all' ? Math.max(stats.totalPromptTokens || 0, promptSum) : promptSum;
    const effectiveCompletion =
      timeRange === 'all' ? Math.max(stats.totalCompletionTokens || 0, completionSum) : completionSum;
    const effectiveErrorCount =
      timeRange === 'all' ? Math.max(stats.errorCount || 0, errorCount) : errorCount;
    const effectiveTokens = effectivePrompt + effectiveCompletion;

    const successRate =
      effectiveTotalRequests > 0
        ? Math.max(0, Math.min(100, ((effectiveTotalRequests - effectiveErrorCount) / effectiveTotalRequests) * 100))
        : 100;

    const avgLatency = totalRequests > 0 ? Math.round(latencySum / totalRequests) : 0;
    const streamRatio = totalRequests > 0 ? Math.round((streamCount / totalRequests) * 100) : 0;
    const fallbackRatio = totalRequests > 0 ? Math.round((fallbackCount / totalRequests) * 100) : 0;

    const providerList = Object.values(providerMap);
    const modelList = Object.values(modelMap);
    const clientList = Object.values(clientMap).sort((a, b) => b.count - a.count);

    // 错误分类诊断列表 (按频次降序)
    const errorDiagnostics = Object.entries(errorDiagnosticsMap)
      .map(([reason, count]) => ({ reason, count }))
      .sort((a, b) => b.count - a.count);

    return {
      totalRequests: effectiveTotalRequests,
      promptTokens: effectivePrompt,
      completionTokens: effectiveCompletion,
      totalTokens: effectiveTokens,
      totalCost: costSum,
      avgCostPerReq: effectiveTotalRequests > 0 ? costSum / effectiveTotalRequests : 0,
      errorCount: effectiveErrorCount,
      successRate,
      avgLatency,
      streamRatio,
      fallbackRatio,
      status2xx,
      status4xx,
      status5xx,
      providerList,
      modelList,
      clientList,
      errorDiagnostics,
      hourlyCounts,
      hasRecords: effectiveTotalRequests > 0 || logs.length > 0,
    };
  }, [filteredLogs, timeRange, stats, providers, logs.length, t, locale]);

  // 根据当前排序维度动态对服务商列表排序
  const sortedProviders = useMemo(() => {
    return [...metrics.providerList].sort((a, b) =>
      providerSort === 'cost' ? b.cost - a.cost : b.count - a.count
    );
  }, [metrics.providerList, providerSort]);

  // 根据当前排序维度动态对模型列表排序
  const sortedModels = useMemo(() => {
    return [...metrics.modelList].sort((a, b) =>
      modelSort === 'cost' ? b.cost - a.cost : b.count - a.count
    );
  }, [metrics.modelList, modelSort]);

  // 服务商全宽分段比例条计算
  const providerSegments = useMemo(() => {
    const isCost = providerSort === 'cost';
    const totalVal = isCost ? metrics.totalCost : metrics.totalRequests;

    return sortedProviders.map((p, index) => {
      const val = isCost ? p.cost : p.count;
      const share = totalVal > 0 ? Math.round((val / totalVal) * 100) : 0;
      return {
        id: p.id,
        name: p.name,
        color: PROVIDER_COLORS[index % PROVIDER_COLORS.length],
        share,
        val,
      };
    });
  }, [sortedProviders, providerSort, metrics.totalCost, metrics.totalRequests]);

  // 客户端分段比例条计算
  const clientSegments = useMemo(() => {
    const totalReq = metrics.totalRequests;
    return metrics.clientList.map((c) => {
      const share = totalReq > 0 ? Math.round((c.count / totalReq) * 100) : 0;
      const color = CLIENT_COLORS[c.type] || '#71717a';
      return {
        ...c,
        share,
        color,
      };
    });
  }, [metrics.clientList, metrics.totalRequests]);

  const maxHourlyCount = useMemo(() => {
    return Math.max(1, ...metrics.hourlyCounts);
  }, [metrics.hourlyCounts]);

  const latencyFormatted = useMemo(() => {
    return formatLatency(metrics.avgLatency);
  }, [metrics.avgLatency]);

  return (
    <div className="w-full pb-8 space-y-3.5 animate-in fade-in duration-150">
      {/* 顶部控制栏：微胶囊状态与紧凑筛选器 */}
      <div className="flex items-center justify-between gap-2 px-0.5">
        <div className="flex items-center gap-1.5 px-2 py-0.5 rounded-full text-[11px] font-mono bg-zinc-500/[0.08] dark:bg-white/[0.08] text-zinc-600 dark:text-zinc-400 border border-border truncate min-w-0">
          <span
            className={`w-1.5 h-1.5 rounded-full shrink-0 ${
              stats.isRunning ? 'bg-emerald-500 animate-pulse' : 'bg-zinc-400'
            }`}
          />
          <span className="truncate">
            {stats.isRunning ? `127.0.0.1:${stats.port}` : (locale === 'zh-CN' ? '网关未启动' : 'Gateway Stopped')}
          </span>
          {stats.isRunning && stats.uptimeSecs > 0 && (
            <>
              <span className="text-zinc-300 dark:text-zinc-600">·</span>
              <span className="text-[10px] text-zinc-400 shrink-0">{formatUptime(stats.uptimeSecs, t)}</span>
            </>
          )}
        </div>

        <div className="flex items-center gap-1.5 shrink-0">
          {/* 精简时间范围切换：全部 / 今天 / 1 小时 */}
          <div className="flex items-center rounded-lg bg-zinc-500/[0.07] dark:bg-white/[0.07] p-0.5 border border-border text-xs">
            <button
              type="button"
              onClick={() => setTimeRange('all')}
              className={`px-2 py-0.5 rounded-md text-[11px] font-medium border border-transparent transition-all duration-150 cursor-pointer outline-none ${
                timeRange === 'all'
                  ? 'bg-background text-foreground shadow-2xs border-black/5 dark:border-white/10'
                  : 'text-muted-foreground hover:text-foreground'
              }`}
            >
              {t('dashboard.timeRange.all')}
            </button>
            <button
              type="button"
              onClick={() => setTimeRange('today')}
              className={`px-2 py-0.5 rounded-md text-[11px] font-medium border border-transparent transition-all duration-150 cursor-pointer outline-none ${
                timeRange === 'today'
                  ? 'bg-background text-foreground shadow-2xs border-black/5 dark:border-white/10'
                  : 'text-muted-foreground hover:text-foreground'
              }`}
            >
              {t('dashboard.timeRange.today')}
            </button>
            <button
              type="button"
              onClick={() => setTimeRange('1h')}
              className={`px-2 py-0.5 rounded-md text-[11px] font-medium border border-transparent transition-all duration-150 cursor-pointer outline-none ${
                timeRange === '1h'
                  ? 'bg-background text-foreground shadow-2xs border-black/5 dark:border-white/10'
                  : 'text-muted-foreground hover:text-foreground'
              }`}
            >
              {t('dashboard.timeRange.oneHour')}
            </button>
          </div>

          {/* 清空日志按钮 */}
          {logs.length > 0 && (
            <Button
              type="button"
              variant="ghost"
              size="sm"
              disabled={clearingLogs}
              onClick={() => setShowClearConfirm(true)}
              className="h-7 w-7 p-0 text-muted-foreground hover:text-rose-600 dark:hover:text-rose-400 cursor-pointer"
              title={t('dashboard.metrics.clearLogs')}
            >
              <Trash2 className="w-3.5 h-3.5" />
            </Button>
          )}
        </div>
      </div>

      {/* 空状态提示 */}
      {!metrics.hasRecords ? (
        <div className="rounded-lg border border-border bg-card p-12 text-center shadow-2xs">
          <div className="mx-auto w-12 h-12 rounded-full bg-zinc-500/[0.08] dark:bg-white/[0.08] flex items-center justify-center text-zinc-400 mb-3">
            <Activity className="w-6 h-6 stroke-[1.5]" />
          </div>
          <h3 className="text-sm font-semibold text-zinc-900 dark:text-zinc-100 mb-1">
            {locale === 'zh-CN' ? '暂无使用数据' : 'No Usage Data'}
          </h3>
          <p className="text-xs text-zinc-400 max-w-sm mx-auto leading-relaxed">
            {locale === 'zh-CN'
              ? '在 Claude Code、Codex CLI 或其他已配置的客户端中发起 AI 对话后，各维度的调用统计、Token 消耗与成本分析将在此实时呈现。'
              : 'Start an AI conversation in Claude Code, Codex CLI, or any configured client to see real-time usage statistics, token consumption, and cost estimates here.'}
          </p>
        </div>
      ) : (
        <>
          {/* 1. 核心关键指标栏 (超紧凑单行 4 列集成条) */}
          <div className="rounded-lg border border-border bg-card p-2 sm:p-2.5 shadow-2xs">
            <div className="grid grid-cols-4 divide-x divide-border/60">
              {/* 单元 1: 请求数 */}
              <div className="px-1.5 sm:px-2.5 flex flex-col justify-between min-w-0">
                <div className="flex items-center justify-between text-zinc-400 gap-1">
                  <span className="text-[10px] font-medium truncate">
                    {locale === 'zh-CN' ? '总请求' : 'Requests'}
                  </span>
                  <Activity className="w-3 h-3 text-primary shrink-0 opacity-80" />
                </div>
                <div className="my-0.5 flex items-baseline gap-1 truncate">
                  <span className="text-sm sm:text-base font-mono font-bold tracking-tight text-zinc-900 dark:text-zinc-100 truncate">
                    {metrics.totalRequests.toLocaleString()}
                  </span>
                  <span className="text-[10px] text-zinc-400 font-mono hidden sm:inline">
                    {locale === 'zh-CN' ? '次' : 'reqs'}
                  </span>
                </div>
                <div className="text-[10px] font-mono truncate">
                  {metrics.errorCount > 0 ? (
                    <span className="text-rose-500 font-medium truncate">
                      {metrics.successRate.toFixed(0)}% · {metrics.errorCount}{locale === 'zh-CN' ? '败' : 'err'}
                    </span>
                  ) : (
                    <span className="text-emerald-600 dark:text-emerald-400 font-medium flex items-center gap-0.5 truncate">
                      <CheckCircle2 className="w-2.5 h-2.5 shrink-0" />
                      100%
                    </span>
                  )}
                </div>
              </div>

              {/* 单元 2: Token 消耗 */}
              <div className="px-1.5 sm:px-2.5 flex flex-col justify-between min-w-0">
                <div className="flex items-center justify-between text-zinc-400 gap-1">
                  <span className="text-[10px] font-medium truncate">
                    {locale === 'zh-CN' ? '总 Token' : 'Tokens'}
                  </span>
                  <Layers className="w-3 h-3 text-blue-500 shrink-0 opacity-80" />
                </div>
                <div className="my-0.5 truncate">
                  <span className="text-sm sm:text-base font-mono font-bold tracking-tight text-zinc-900 dark:text-zinc-100 truncate">
                    {formatTokens(metrics.totalTokens)}
                  </span>
                </div>
                <div className="text-[9px] text-zinc-400 font-mono truncate">
                  ↑{formatTokens(metrics.promptTokens)} · ↓{formatTokens(metrics.completionTokens)}
                </div>
              </div>

              {/* 单元 3: 预估花费 */}
              <div className="px-1.5 sm:px-2.5 flex flex-col justify-between min-w-0">
                <div className="flex items-center justify-between text-zinc-400 gap-1">
                  <span className="text-[10px] font-medium truncate">
                    {locale === 'zh-CN' ? '预估费用' : 'Cost'}
                  </span>
                  <Coins className="w-3 h-3 text-zinc-400 shrink-0 opacity-80" />
                </div>
                <div className="my-0.5 truncate">
                  <span className="text-sm sm:text-base font-mono font-bold tracking-tight text-zinc-900 dark:text-zinc-100 truncate">
                    {formatCurrency(metrics.totalCost)}
                  </span>
                </div>
                <div className="text-[10px] text-zinc-400 font-mono truncate">
                  {locale === 'zh-CN' ? '均' : 'avg'} {formatCurrency(metrics.avgCostPerReq)}
                </div>
              </div>

              {/* 单元 4: 平均耗时 */}
              <div className="px-1.5 sm:px-2.5 flex flex-col justify-between min-w-0">
                <div className="flex items-center justify-between text-zinc-400 gap-1">
                  <span className="text-[10px] font-medium truncate">
                    {locale === 'zh-CN' ? '平均延迟' : 'Latency'}
                  </span>
                  <Clock className="w-3 h-3 text-amber-500 shrink-0 opacity-80" />
                </div>
                <div className="my-0.5 flex items-baseline gap-0.5 truncate">
                  <span className="text-sm sm:text-base font-mono font-bold tracking-tight text-zinc-900 dark:text-zinc-100 truncate">
                    {latencyFormatted.value}
                  </span>
                  <span className="text-[10px] text-zinc-400 font-mono">{latencyFormatted.unit}</span>
                </div>
                <div className="text-[10px] text-zinc-400 font-mono truncate flex items-center gap-0.5">
                  <Zap className="w-2.5 h-2.5 text-amber-500 shrink-0" />
                  <span>{metrics.streamRatio}% {locale === 'zh-CN' ? '流式' : 'stream'}</span>
                </div>
              </div>
            </div>
          </div>

          {/* 2. 中层：服务商占比 & 热门模型横向条形排行榜 (双列网格) */}
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-3.5">
            {/* 左列：服务商分发 (全宽分段条 + 紧凑行内数据) */}
            <div className="rounded-lg border border-border bg-card shadow-2xs overflow-hidden flex flex-col justify-between">
              <div>
                <div className="px-3.5 py-2 bg-muted/20 border-b border-border flex items-center justify-between text-xs">
                  <div className="flex items-center gap-1.5 font-semibold text-zinc-800 dark:text-zinc-200">
                    <Server className="w-3.5 h-3.5 text-zinc-500" />
                    <span>{t('dashboard.charts.providerDistribution')}</span>
                  </div>
                  {/* 维度切换 */}
                  <div className="flex items-center gap-0.5 bg-zinc-500/[0.08] dark:bg-white/[0.08] p-0.5 rounded-md text-[10px] font-medium border border-border/50">
                    <button
                      type="button"
                      onClick={() => setProviderSort('count')}
                      className={`px-1.5 py-0.5 rounded transition-colors cursor-pointer outline-none ${
                        providerSort === 'count'
                          ? 'bg-background text-foreground shadow-2xs font-semibold'
                          : 'text-muted-foreground hover:text-foreground'
                      }`}
                    >
                      {t('dashboard.charts.sortByRequests')}
                    </button>
                    <button
                      type="button"
                      onClick={() => setProviderSort('cost')}
                      className={`px-1.5 py-0.5 rounded transition-colors cursor-pointer outline-none ${
                        providerSort === 'cost'
                          ? 'bg-background text-foreground shadow-2xs font-semibold'
                          : 'text-muted-foreground hover:text-foreground'
                      }`}
                    >
                      {t('dashboard.charts.sortByCost')}
                    </button>
                  </div>
                </div>

                <div className="p-3 space-y-2.5">
                  {sortedProviders.length === 0 ? (
                    <div className="p-6 text-center text-xs text-zinc-400">
                      {locale === 'zh-CN' ? '暂无服务商调用记录' : 'No provider records'}
                    </div>
                  ) : (
                    <>
                      {/* 全宽分段比例条 (Stacked Segment Bar) */}
                      <div className="w-full h-2 rounded-full bg-zinc-500/[0.08] dark:bg-white/[0.08] flex overflow-hidden gap-[1px]">
                        {providerSegments.map((seg) => (
                          <div
                            key={seg.id}
                            className="h-full transition-all duration-300 first:rounded-l-full last:rounded-r-full"
                            style={{
                              width: `${Math.max(1.5, seg.share)}%`,
                              backgroundColor: seg.color,
                            }}
                            title={`${seg.name}: ${seg.share}%`}
                          />
                        ))}
                      </div>

                      {/* 服务商列表项 */}
                      <div className="divide-y divide-border/50">
                        {sortedProviders.map((p, index) => {
                          const isCost = providerSort === 'cost';
                          const share = isCost
                            ? (metrics.totalCost > 0 ? Math.round((p.cost / metrics.totalCost) * 100) : 0)
                            : (metrics.totalRequests > 0 ? Math.round((p.count / metrics.totalRequests) * 100) : 0);

                          const pSuccessRate =
                            p.count > 0 ? Math.round((p.successCount / p.count) * 100) : 100;
                          const pAvgLat = p.count > 0 ? Math.round(p.latencySum / p.count) : 0;
                          const latFmt = formatLatency(pAvgLat);
                          const color = PROVIDER_COLORS[index % PROVIDER_COLORS.length];

                          return (
                            <div key={p.id} className="py-1.5 first:pt-0 last:pb-0 space-y-1">
                              <div className="flex items-center justify-between gap-2">
                                <div className="flex items-center gap-1.5 min-w-0">
                                  <span
                                    className="w-2 h-2 rounded-full shrink-0"
                                    style={{ backgroundColor: color }}
                                  />
                                  <ProviderAvatar
                                    name={p.name}
                                    protocol={p.protocol}
                                    sizeClass="w-4 h-4"
                                    dense
                                  />
                                  <span className="text-xs font-semibold text-zinc-900 dark:text-zinc-100 truncate">
                                    {p.name}
                                  </span>
                                  {renderSuccessRateBadge(pSuccessRate, p.count, locale)}
                                </div>
                                <div className="flex items-center gap-1.5 shrink-0 text-xs font-mono">
                                  <span className="font-bold text-zinc-900 dark:text-zinc-100">
                                    {isCost ? formatCurrency(p.cost) : `${p.count} ${locale === 'zh-CN' ? '次' : 'reqs'}`}
                                  </span>
                                  <span className="text-[11px] text-zinc-400 font-normal">
                                    ({share}%)
                                  </span>
                                </div>
                              </div>

                              <div className="flex items-center justify-between text-[10px] text-zinc-400 font-mono pl-3.5">
                                <span>{formatTokens(p.totalTokens)} tok</span>
                                <div className="flex items-center gap-1.5">
                                  <span>{latFmt.value} {latFmt.unit}</span>
                                  <span>·</span>
                                  {isCost ? (
                                    <span className="text-zinc-500 dark:text-zinc-400">
                                      {p.count} {locale === 'zh-CN' ? '次' : 'reqs'}
                                    </span>
                                  ) : (
                                    <span className="text-zinc-700 dark:text-zinc-300 font-medium">
                                      {formatCurrency(p.cost)}
                                    </span>
                                  )}
                                </div>
                              </div>
                            </div>
                          );
                        })}
                      </div>
                    </>
                  )}
                </div>
              </div>
            </div>

            {/* 右列：热门模型横向条形排行榜 (Horizontal Bar Chart) */}
            <div className="rounded-lg border border-border bg-card shadow-2xs overflow-hidden flex flex-col justify-between">
              <div>
                <div className="px-3.5 py-2 bg-muted/20 border-b border-border flex items-center justify-between text-xs">
                  <div className="flex items-center gap-1.5 font-semibold text-zinc-800 dark:text-zinc-200">
                    <Sparkles className="w-3.5 h-3.5 text-zinc-500" />
                    <span>{t('dashboard.charts.modelDistribution')}</span>
                    <span className="text-[11px] font-mono text-zinc-400 font-normal">
                      (Top {Math.min(5, sortedModels.length)})
                    </span>
                  </div>
                  {/* 维度切换 */}
                  <div className="flex items-center gap-0.5 bg-zinc-500/[0.08] dark:bg-white/[0.08] p-0.5 rounded-md text-[10px] font-medium border border-border/50">
                    <button
                      type="button"
                      onClick={() => setModelSort('count')}
                      className={`px-1.5 py-0.5 rounded transition-colors cursor-pointer outline-none ${
                        modelSort === 'count'
                          ? 'bg-background text-foreground shadow-2xs font-semibold'
                          : 'text-muted-foreground hover:text-foreground'
                      }`}
                    >
                      {t('dashboard.charts.sortByRequests')}
                    </button>
                    <button
                      type="button"
                      onClick={() => setModelSort('cost')}
                      className={`px-1.5 py-0.5 rounded transition-colors cursor-pointer outline-none ${
                        modelSort === 'cost'
                          ? 'bg-background text-foreground shadow-2xs font-semibold'
                          : 'text-muted-foreground hover:text-foreground'
                      }`}
                    >
                      {t('dashboard.charts.sortByCost')}
                    </button>
                  </div>
                </div>

                <div className="p-3 space-y-2">
                  {sortedModels.length === 0 ? (
                    <div className="p-6 text-center text-xs text-zinc-400">
                      {locale === 'zh-CN' ? '暂无模型调用数据' : 'No model usage data'}
                    </div>
                  ) : (
                    sortedModels.slice(0, 5).map((m, index) => {
                      const isCost = modelSort === 'cost';
                      const mShare = isCost
                        ? (metrics.totalCost > 0 ? Math.round((m.cost / metrics.totalCost) * 100) : 0)
                        : (metrics.totalRequests > 0 ? Math.round((m.count / metrics.totalRequests) * 100) : 0);

                      const parsed = parseModelDisplay(m.model, m.targetModel);

                      // 排名微样式：优雅的纯净质感
                      const rankBadgeClass =
                        index === 0
                          ? 'bg-amber-500/15 text-amber-600 dark:text-amber-400 border border-amber-500/30'
                          : index === 1
                          ? 'bg-zinc-500/15 text-zinc-600 dark:text-zinc-300 border border-zinc-500/30'
                          : index === 2
                          ? 'bg-orange-500/15 text-orange-600 dark:text-orange-400 border border-orange-500/30'
                          : 'bg-zinc-500/10 text-zinc-400';

                      return (
                        <div key={m.model} className="space-y-1">
                          <div className="flex items-center justify-between gap-2">
                            <div className="flex items-center gap-1.5 min-w-0">
                              <span
                                className={`w-4 h-4 rounded flex items-center justify-center text-[10px] font-mono font-bold shrink-0 ${rankBadgeClass}`}
                              >
                                {index + 1}
                              </span>
                              {parsed.namespace && (
                                <span
                                  className="text-[10px] font-mono px-1 py-0.2 rounded bg-zinc-500/[0.08] dark:bg-white/[0.08] text-zinc-500 dark:text-zinc-400 shrink-0"
                                  title={`${locale === 'zh-CN' ? '命名空间' : 'Namespace'}: ${parsed.namespace}`}
                                >
                                  {parsed.namespace}
                                </span>
                              )}
                              <span
                                className="text-xs font-mono font-semibold text-zinc-900 dark:text-zinc-100 truncate"
                                title={m.model}
                              >
                                {parsed.displayName}
                              </span>
                              {parsed.targetDisplay && (
                                <span
                                  className="text-[10px] font-mono text-zinc-400 truncate shrink-0"
                                  title={`${locale === 'zh-CN' ? '目标模型' : 'Target Model'}: ${m.targetModel}`}
                                >
                                  → {parsed.targetDisplay}
                                </span>
                              )}
                            </div>
                            <div className="flex items-center gap-1.5 shrink-0 text-xs font-mono">
                              <span className="font-bold text-zinc-900 dark:text-zinc-100">
                                {isCost ? formatCurrency(m.cost) : `${m.count} ${locale === 'zh-CN' ? '次' : 'reqs'}`}
                              </span>
                              <span className="text-[11px] text-zinc-400 font-normal">({mShare}%)</span>
                            </div>
                          </div>

                          {/* 横向柱形图 (Bar Chart) */}
                          <div className="w-full h-1.5 bg-zinc-500/[0.08] dark:bg-white/[0.08] rounded-full overflow-hidden">
                            <div
                              className="h-full bg-gradient-to-r from-blue-500 to-indigo-500 rounded-full transition-all duration-300"
                              style={{ width: `${Math.max(2, mShare)}%` }}
                            />
                          </div>

                          {/* 细分指标 */}
                          <div className="flex items-center justify-between text-[10px] text-zinc-400 font-mono">
                            <span className="truncate mr-2">
                              {formatTokens(m.totalTokens)} tok
                              <span className="text-zinc-400 dark:text-zinc-500 ml-1.5">
                                (↑{formatTokens(m.promptTokens)} · ↓{formatTokens(m.completionTokens)})
                              </span>
                            </span>
                            <div className="shrink-0">
                              {isCost ? (
                                <span className="text-zinc-500 dark:text-zinc-400">
                                  {m.count} {locale === 'zh-CN' ? '次' : 'reqs'}
                                </span>
                              ) : (
                                <span className="text-zinc-700 dark:text-zinc-300 font-medium">
                                  {formatCurrency(m.cost)}
                                </span>
                              )}
                            </div>
                          </div>
                        </div>
                      );
                    })
                  )}
                </div>
              </div>
            </div>
          </div>

          {/* 3. 下层：客户端调用分段比例图 & 链路稳定性与异常诊断 (双列网格) */}
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-3.5">
            {/* 左列：客户端接入统计 (带分段比例图) */}
            <div className="rounded-lg border border-border bg-card shadow-2xs overflow-hidden">
              <div className="px-3.5 py-2 bg-muted/20 border-b border-border flex items-center justify-between text-xs">
                <div className="flex items-center gap-1.5 font-semibold text-zinc-800 dark:text-zinc-200">
                  <Terminal className="w-3.5 h-3.5 text-zinc-500" />
                  <span>{t('dashboard.charts.clientDistribution')}</span>
                </div>
                <span className="text-[11px] font-mono text-zinc-400">
                  {metrics.clientList.length} {locale === 'zh-CN' ? '类客户端' : 'clients'}
                </span>
              </div>

              <div className="p-3 space-y-2.5">
                {metrics.clientList.length === 0 ? (
                  <div className="p-6 text-center text-xs text-zinc-400">
                    {locale === 'zh-CN' ? '暂无客户端请求' : 'No client requests'}
                  </div>
                ) : (
                  <>
                    {/* 客户端分段比例条 (Stacked Segment Bar) */}
                    <div className="w-full h-2 rounded-full bg-zinc-500/[0.08] dark:bg-white/[0.08] flex overflow-hidden gap-[1px]">
                      {clientSegments.map((c) => (
                        <div
                          key={c.type}
                          className="h-full transition-all duration-300 first:rounded-l-full last:rounded-r-full"
                          style={{
                            width: `${c.share}%`,
                            backgroundColor: c.color,
                          }}
                          title={`${formatClientType(c.type, t)}: ${c.share}%`}
                        />
                      ))}
                    </div>

                    {/* 客户端条目列表 */}
                    <div className="divide-y divide-border/50">
                      {clientSegments.map((c) => (
                        <div key={c.type} className="py-1.5 first:pt-0 last:pb-0 space-y-1">
                          <div className="flex items-center justify-between gap-2">
                            <div className="flex items-center gap-1.5 min-w-0">
                              <span
                                className="w-2 h-2 rounded-full shrink-0"
                                style={{ backgroundColor: c.color }}
                              />
                              <div className="w-4 h-4 rounded bg-zinc-500/[0.08] dark:bg-white/[0.08] flex items-center justify-center p-0.5 shrink-0">
                                <ClientLogo type={c.type} className="w-3 h-3" />
                              </div>
                              <span className="text-xs font-semibold text-zinc-900 dark:text-zinc-100">
                                {formatClientType(c.type, t)}
                              </span>
                            </div>
                            <div className="flex items-center gap-1.5 text-xs font-mono">
                              <span className="font-bold text-zinc-900 dark:text-zinc-100">
                                {c.count} {locale === 'zh-CN' ? '次' : 'reqs'}
                              </span>
                              <span className="text-[11px] text-zinc-400 font-normal">({c.share}%)</span>
                            </div>
                          </div>

                          <div className="flex items-center justify-between text-[10px] text-zinc-400 font-mono pl-3.5">
                            <span>{formatTokens(c.totalTokens)} tok</span>
                            <div className="flex items-center gap-1.5">
                              {c.errorCount > 0 && (
                                <span className="text-rose-500 font-medium font-mono">
                                  {c.errorCount} {locale === 'zh-CN' ? '失败' : 'failed'}
                                </span>
                              )}
                              {c.errorCount > 0 && <span>·</span>}
                              <span className="text-zinc-700 dark:text-zinc-300 font-medium">
                                {formatCurrency(c.cost)}
                              </span>
                            </div>
                          </div>
                        </div>
                      ))}
                    </div>
                  </>
                )}
              </div>
            </div>

            {/* 右列：链路稳定性与异常诊断 */}
            <div className="rounded-lg border border-border bg-card shadow-2xs overflow-hidden flex flex-col justify-between">
              <div>
                <div className="px-3.5 py-2 bg-muted/20 border-b border-border flex items-center justify-between text-xs">
                  <div className="flex items-center gap-1.5 font-semibold text-zinc-800 dark:text-zinc-200">
                    {metrics.errorCount > 0 ? (
                      <ShieldAlert className="w-3.5 h-3.5 text-amber-500" />
                    ) : (
                      <ShieldCheck className="w-3.5 h-3.5 text-emerald-500" />
                    )}
                    <span>{t('dashboard.charts.recentErrors')}</span>
                  </div>
                  <span className="text-[11px] font-mono text-zinc-400">
                    {t('dashboard.metrics.successRate')} {metrics.successRate.toFixed(1)}%
                  </span>
                </div>

                <div className="p-3 space-y-2.5">
                  {/* 三色状态码分段进度条 */}
                  <div className="space-y-1">
                    <div className="flex items-center justify-between text-[10px] font-mono text-zinc-400">
                      <span>{t('dashboard.charts.statusDistribution')}</span>
                      <span>
                        2xx: {metrics.status2xx} · 4xx: {metrics.status4xx} · 5xx: {metrics.status5xx}
                      </span>
                    </div>
                    <div className="w-full h-1.5 rounded-full bg-zinc-500/[0.08] dark:bg-white/[0.08] flex overflow-hidden">
                      {metrics.status2xx > 0 && (
                        <div
                          className="h-full bg-emerald-500 transition-all duration-300"
                          style={{
                            width: `${(metrics.status2xx / Math.max(1, metrics.totalRequests)) * 100}%`,
                          }}
                          title={`2xx: ${metrics.status2xx}`}
                        />
                      )}
                      {metrics.status4xx > 0 && (
                        <div
                          className="h-full bg-amber-500 transition-all duration-300"
                          style={{
                            width: `${(metrics.status4xx / Math.max(1, metrics.totalRequests)) * 100}%`,
                          }}
                          title={`4xx: ${metrics.status4xx}`}
                        />
                      )}
                      {metrics.status5xx > 0 && (
                        <div
                          className="h-full bg-rose-500 transition-all duration-300"
                          style={{
                            width: `${(metrics.status5xx / Math.max(1, metrics.totalRequests)) * 100}%`,
                          }}
                          title={`5xx: ${metrics.status5xx}`}
                        />
                      )}
                    </div>
                  </div>

                  {/* 异常原因归因列表 */}
                  <div className="pt-1.5 border-t border-border/60">
                    {metrics.errorDiagnostics.length === 0 ? (
                      <div className="p-2.5 rounded-md bg-emerald-50/60 dark:bg-emerald-950/20 border border-emerald-200/50 dark:border-emerald-900/40 flex items-center gap-2 text-xs text-emerald-700 dark:text-emerald-300">
                        <CheckCircle2 className="w-3.5 h-3.5 shrink-0 text-emerald-500" />
                        <span>{t('dashboard.charts.noErrors')}</span>
                      </div>
                    ) : (
                      <div className="space-y-1">
                        {metrics.errorDiagnostics.slice(0, 3).map(({ reason, count }) => (
                          <div
                            key={reason}
                            className="p-1.5 rounded bg-rose-50/50 dark:bg-rose-950/20 border border-rose-200/50 dark:border-rose-900/40 flex items-center justify-between text-xs gap-2"
                          >
                            <div className="flex items-center gap-1.5 min-w-0">
                              <AlertTriangle className="w-3 h-3 shrink-0 text-amber-500 dark:text-amber-400" />
                              <span className="font-mono text-zinc-800 dark:text-zinc-200 truncate text-[10px]">
                                {reason}
                              </span>
                            </div>
                            <span className="text-xs font-mono font-bold text-rose-600 dark:text-rose-400 shrink-0">
                              {count} {locale === 'zh-CN' ? '次' : 'reqs'}
                            </span>
                          </div>
                        ))}
                      </div>
                    )}
                  </div>
                </div>
              </div>

              {/* 底部微信息 */}
              <div className="px-3.5 py-1.5 bg-muted/20 border-t border-border flex items-center justify-between text-[10px] text-zinc-400 font-mono">
                <span>{locale === 'zh-CN' ? '兜底触发' : 'Fallback:'} {metrics.fallbackRatio}%</span>
                <span>{locale === 'zh-CN' ? '流式长连接' : 'Stream:'} {metrics.streamRatio}%</span>
              </div>
            </div>
          </div>

          {/* 4. 24小时活跃时段节奏条 (柱状微图) */}
          <div className="rounded-lg border border-border bg-card p-3 shadow-2xs space-y-2">
            <div className="flex items-center justify-between text-xs">
              <div className="flex items-center gap-1.5 font-semibold text-zinc-800 dark:text-zinc-200">
                <TrendingUp className="w-3.5 h-3.5 text-primary" />
                <span>{t('dashboard.charts.requestsOverTime')}</span>
              </div>
              <span className="text-[10px] text-zinc-400 font-mono">
                {locale === 'zh-CN' ? '峰值' : 'Peak'}: {maxHourlyCount > 1 ? `${maxHourlyCount} ${locale === 'zh-CN' ? '次/时' : 'reqs/h'}` : '-'}
              </span>
            </div>

            {/* 24 小时柱状条 */}
            <div className="flex items-end gap-1 h-10 pt-1 pb-1">
              {metrics.hourlyCounts.map((count, hour) => {
                const heightPercent =
                  count > 0 ? Math.max(12, Math.round((count / maxHourlyCount) * 100)) : 6;

                return (
                  <div
                    key={hour}
                    className="flex-1 flex flex-col items-center justify-end h-full group relative"
                  >
                    {/* Tooltip on hover */}
                    <div className="absolute -top-7 hidden group-hover:flex items-center px-1.5 py-0.5 rounded text-[10px] font-mono bg-zinc-900 text-white dark:bg-zinc-100 dark:text-zinc-900 shadow-md whitespace-nowrap z-30 pointer-events-none">
                      {hour}:00 · {count}{locale === 'zh-CN' ? '次' : 'reqs'}
                    </div>

                    <div
                      className={`w-full rounded-xs transition-all duration-200 ${
                        count > 0
                          ? 'bg-primary/80 group-hover:bg-primary'
                          : 'bg-zinc-500/[0.08] dark:bg-white/[0.08]'
                      }`}
                      style={{ height: `${heightPercent}%` }}
                    />
                  </div>
                );
              })}
            </div>

            {/* 时间轴刻度 */}
            <div className="flex justify-between text-[9px] font-mono text-zinc-400 px-0.5">
              <span>00:00</span>
              <span>06:00</span>
              <span>12:00</span>
              <span>18:00</span>
              <span>23:00</span>
            </div>
          </div>
        </>
      )}

      {/* 清空日志二次确认弹窗 */}
      <ConfirmDialog
        open={showClearConfirm}
        title={t('dashboard.metrics.clearLogsConfirm')}
        details={[t('dashboard.metrics.clearLogsDetail')]}
        confirmLabel={t('dashboard.metrics.clearLogs')}
        cancelLabel={t('common.cancel')}
        variant="danger"
        busy={clearingLogs}
        onConfirm={async () => {
          try {
            setClearingLogs(true);
            await clearLogs();
          } finally {
            setClearingLogs(false);
            setShowClearConfirm(false);
          }
        }}
        onCancel={() => setShowClearConfirm(false)}
      />
    </div>
  );
};
