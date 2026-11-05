import React, { useState, useMemo } from 'react';
import { useAppStore } from '@/store/useAppStore';
import { useTranslation } from '@/i18n';
import { Badge } from '@/components/common/Badge';
import { ProviderAvatar } from '@/components/common/BrandIcons';
import {
  OpenAISpiralIcon,
  ClaudeLogo,
  GeminiLogo,
  PiLogo,
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
} from 'lucide-react';

type TimeRange = 'all' | 'today' | '1h';

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

export const UsageDashboard: React.FC = () => {
  const { t, locale } = useTranslation();
  const { logs, stats, providers } = useAppStore();
  const [timeRange, setTimeRange] = useState<TimeRange>('all');

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

    // 排序服务商列表 (按调用次数降序)
    const providerList = Object.values(providerMap).sort((a, b) => b.count - a.count);

    // 排序模型排行 (按调用次数降序)
    const modelList = Object.values(modelMap).sort((a, b) => b.count - a.count);

    // 排序客户端列表 (按调用次数降序)
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

  const maxHourlyCount = useMemo(() => {
    return Math.max(1, ...metrics.hourlyCounts);
  }, [metrics.hourlyCounts]);

  return (
    <div className="w-full pb-10 space-y-4 animate-in fade-in duration-150">
      {/* 顶部控制栏：标题与状态微徽章、时间跨度切换 */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 px-1">
        <div className="flex items-center gap-2.5">
          <div className="flex items-center gap-2">
            <span className="text-base font-bold text-zinc-900 dark:text-zinc-100">
              {t('header.logs')}
            </span>
            <div className="flex items-center gap-1.5 px-2 py-0.5 rounded-full text-[11px] font-mono bg-zinc-500/[0.08] dark:bg-white/[0.08] text-zinc-600 dark:text-zinc-400 border border-border">
              <span
                className={`w-1.5 h-1.5 rounded-full ${
                  stats.isRunning ? 'bg-emerald-500 animate-pulse' : 'bg-zinc-400'
                }`}
              />
              <span>
                {stats.isRunning ? `127.0.0.1:${stats.port}` : (locale === 'zh-CN' ? '网关未启动' : 'Gateway Stopped')}
              </span>
              {stats.isRunning && stats.uptimeSecs > 0 && (
                <>
                  <span className="text-zinc-300 dark:text-zinc-600">·</span>
                  <span className="text-[10px] text-zinc-400">{formatUptime(stats.uptimeSecs, t)}</span>
                </>
              )}
            </div>
          </div>
        </div>

        <div className="flex items-center gap-2">
          {/* 时间范围切换 */}
          <div className="flex items-center rounded-lg bg-zinc-500/[0.07] dark:bg-white/[0.07] p-0.5 border border-border text-xs">
            <button
              type="button"
              onClick={() => setTimeRange('all')}
              className={`px-2.5 py-1 rounded-md text-[11px] font-medium border border-transparent transition-[color,background-color,border-color,box-shadow] duration-150 cursor-pointer outline-none ${
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
              className={`px-2.5 py-1 rounded-md text-[11px] font-medium border border-transparent transition-[color,background-color,border-color,box-shadow] duration-150 cursor-pointer outline-none ${
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
              className={`px-2.5 py-1 rounded-md text-[11px] font-medium border border-transparent transition-[color,background-color,border-color,box-shadow] duration-150 cursor-pointer outline-none ${
                timeRange === '1h'
                  ? 'bg-background text-foreground shadow-2xs border-black/5 dark:border-white/10'
                  : 'text-muted-foreground hover:text-foreground'
              }`}
            >
              {t('dashboard.timeRange.oneHour')}
            </button>
          </div>
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
          {/* 1. 核心关键指标卡片 (4 列栅格) */}
          <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
            {/* 卡片 1: 总请求数 */}
            <div className="rounded-lg border border-border bg-card p-3.5 shadow-2xs flex flex-col justify-between">
              <div className="flex items-center justify-between text-zinc-400">
                <span className="text-[11px] font-medium">{t('dashboard.metrics.totalRequests')}</span>
                <Activity className="w-4 h-4 text-primary" />
              </div>
              <div className="mt-2 flex items-baseline gap-2">
                <span className="text-2xl font-mono font-bold tracking-tight text-zinc-900 dark:text-zinc-100">
                  {metrics.totalRequests.toLocaleString()}
                </span>
                <span className="text-xs text-zinc-400">{locale === 'zh-CN' ? '次' : 'reqs'}</span>
              </div>
              <div className="mt-2.5 flex items-center justify-between text-[11px] pt-2 border-t border-border/60">
                <span className="text-emerald-600 dark:text-emerald-400 font-medium flex items-center gap-1">
                  <CheckCircle2 className="w-3 h-3" />
                  {metrics.successRate.toFixed(1)}% {locale === 'zh-CN' ? '成功' : 'success'}
                </span>
                {metrics.errorCount > 0 ? (
                  <span className="text-rose-500 font-medium">
                    {metrics.errorCount} {locale === 'zh-CN' ? '次失败' : 'failed'}
                  </span>
                ) : (
                  <span className="text-zinc-400">0 {locale === 'zh-CN' ? '错误' : 'errors'}</span>
                )}
              </div>
            </div>

            {/* 卡片 2: Token 消耗 */}
            <div className="rounded-lg border border-border bg-card p-3.5 shadow-2xs flex flex-col justify-between">
              <div className="flex items-center justify-between text-zinc-400">
                <span className="text-[11px] font-medium">{t('dashboard.metrics.totalTokens')}</span>
                <Layers className="w-4 h-4 text-blue-500" />
              </div>
              <div className="mt-2 flex items-baseline gap-2">
                <span className="text-2xl font-mono font-bold tracking-tight text-zinc-900 dark:text-zinc-100">
                  {formatTokens(metrics.totalTokens)}
                </span>
                <span className="text-xs text-zinc-400">tokens</span>
              </div>
              <div className="mt-2.5 flex items-center justify-between text-[11px] pt-2 border-t border-border/60 text-zinc-400">
                <span>{locale === 'zh-CN' ? '入' : 'In'} {formatTokens(metrics.promptTokens)}</span>
                <span className="text-zinc-300 dark:text-zinc-600">/</span>
                <span>{locale === 'zh-CN' ? '出' : 'Out'} {formatTokens(metrics.completionTokens)}</span>
              </div>
            </div>

            {/* 卡片 3: 预估花费 */}
            <div className="rounded-lg border border-border bg-card p-3.5 shadow-2xs flex flex-col justify-between">
              <div className="flex items-center justify-between text-zinc-400">
                <span className="text-[11px] font-medium">{t('dashboard.metrics.estimatedCost')}</span>
                <span className="text-xs font-mono font-bold text-emerald-600 dark:text-emerald-400">
                  USD
                </span>
              </div>
              <div className="mt-2 flex items-baseline gap-2">
                <span className="text-2xl font-mono font-bold tracking-tight text-emerald-600 dark:text-emerald-400">
                  {formatCurrency(metrics.totalCost)}
                </span>
              </div>
              <div className="mt-2.5 flex items-center justify-between text-[11px] pt-2 border-t border-border/60 text-zinc-400">
                <span>{locale === 'zh-CN' ? '均次' : 'Avg'} {formatCurrency(metrics.avgCostPerReq)}</span>
                <span className="text-[10px] text-zinc-400">{locale === 'zh-CN' ? '官方标准' : 'Official rate'}</span>
              </div>
            </div>

            {/* 卡片 4: 性能与流式 */}
            <div className="rounded-lg border border-border bg-card p-3.5 shadow-2xs flex flex-col justify-between">
              <div className="flex items-center justify-between text-zinc-400">
                <span className="text-[11px] font-medium">{t('dashboard.metrics.avgLatency')}</span>
                <Clock className="w-4 h-4 text-amber-500" />
              </div>
              <div className="mt-2 flex items-baseline gap-2">
                <span className="text-2xl font-mono font-bold tracking-tight text-zinc-900 dark:text-zinc-100">
                  {metrics.avgLatency}
                </span>
                <span className="text-xs text-zinc-400">ms</span>
              </div>
              <div className="mt-2.5 flex items-center justify-between text-[11px] pt-2 border-t border-border/60 text-zinc-400">
                <span className="flex items-center gap-1">
                  <Zap className="w-3 h-3 text-amber-500" />
                  {metrics.streamRatio}% {locale === 'zh-CN' ? '流式' : 'stream'}
                </span>
                {metrics.fallbackRatio > 0 && (
                  <span className="text-amber-600 dark:text-amber-400">
                    {metrics.fallbackRatio}% {locale === 'zh-CN' ? '兜底' : 'fallback'}
                  </span>
                )}
              </div>
            </div>
          </div>

          {/* 2. 中层：服务商调用分布与模型排行 (双列网格) */}
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
            {/* 左列：服务商调用与消耗分布 */}
            <div className="rounded-lg border border-border bg-card shadow-2xs overflow-hidden">
              <div className="px-4 py-2.5 bg-muted/30 border-b border-border flex items-center justify-between text-xs">
                <div className="flex items-center gap-2 font-semibold text-zinc-800 dark:text-zinc-200">
                  <Server className="w-3.5 h-3.5 text-zinc-500" />
                  <span>{t('dashboard.charts.providerDistribution')}</span>
                </div>
                <span className="text-[11px] font-mono text-zinc-400">
                  {metrics.providerList.length} {locale === 'zh-CN' ? '家服务商' : 'providers'}
                </span>
              </div>

              <div className="p-3 divide-y divide-border/60">
                {metrics.providerList.length === 0 ? (
                  <div className="p-6 text-center text-xs text-zinc-400">{locale === 'zh-CN' ? '暂无服务商调用记录' : 'No provider records'}</div>
                ) : (
                  metrics.providerList.map((p) => {
                    const reqShare =
                      metrics.totalRequests > 0
                        ? Math.round((p.count / metrics.totalRequests) * 100)
                        : 0;
                    const pSuccessRate =
                      p.count > 0 ? Math.round((p.successCount / p.count) * 100) : 100;
                    const pAvgLat = p.count > 0 ? Math.round(p.latencySum / p.count) : 0;

                    return (
                      <div key={p.id} className="py-2.5 first:pt-1 last:pb-1 space-y-1.5">
                        <div className="flex items-center justify-between gap-2">
                          <div className="flex items-center gap-2 min-w-0">
                            <ProviderAvatar
                              name={p.name}
                              protocol={p.protocol}
                              sizeClass="w-6 h-6"
                              dense
                            />
                            <span className="text-xs font-semibold text-zinc-900 dark:text-zinc-100 truncate">
                              {p.name}
                            </span>
                            {pSuccessRate < 90 && (
                              <Badge variant="destructive" className="text-[9px] py-0 px-1">
                                {pSuccessRate}% {locale === 'zh-CN' ? '成功' : 'success'}
                              </Badge>
                            )}
                          </div>
                          <div className="flex items-center gap-2.5 shrink-0 text-xs font-mono">
                            <span className="font-bold text-zinc-900 dark:text-zinc-100">
                              {p.count} {locale === 'zh-CN' ? '次' : 'reqs'}
                            </span>
                            <span className="text-[11px] text-zinc-400 dark:text-zinc-500">
                              ({reqShare}%)
                            </span>
                          </div>
                        </div>

                        {/* 占比进度条 */}
                        <div className="w-full h-1.5 bg-zinc-500/[0.08] dark:bg-white/[0.08] rounded-full overflow-hidden">
                          <div
                            className="h-full bg-primary rounded-full transition-all duration-300"
                            style={{ width: `${Math.max(2, reqShare)}%` }}
                          />
                        </div>

                        {/* 次级指标: Token · 费用 · 延迟 */}
                        <div className="flex items-center justify-between text-[11px] text-zinc-400 font-mono">
                          <span>
                            {formatTokens(p.totalTokens)} tok
                          </span>
                          <div className="flex items-center gap-2">
                            <span>{pAvgLat} ms</span>
                            <span>·</span>
                            <span className="text-emerald-600 dark:text-emerald-400 font-medium">
                              {formatCurrency(p.cost)}
                            </span>
                          </div>
                        </div>
                      </div>
                    );
                  })
                )}
              </div>
            </div>

            {/* 右列：热门模型消耗排行 */}
            <div className="rounded-lg border border-border bg-card shadow-2xs overflow-hidden">
              <div className="px-4 py-2.5 bg-muted/30 border-b border-border flex items-center justify-between text-xs">
                <div className="flex items-center gap-2 font-semibold text-zinc-800 dark:text-zinc-200">
                  <Sparkles className="w-3.5 h-3.5 text-zinc-500" />
                  <span>{t('dashboard.charts.modelDistribution')}</span>
                </div>
                <span className="text-[11px] font-mono text-zinc-400">
                  Top {Math.min(5, metrics.modelList.length)}
                </span>
              </div>

              <div className="p-3 divide-y divide-border/60">
                {metrics.modelList.length === 0 ? (
                  <div className="p-6 text-center text-xs text-zinc-400">{locale === 'zh-CN' ? '暂无模型调用数据' : 'No model usage data'}</div>
                ) : (
                  metrics.modelList.slice(0, 6).map((m, index) => {
                    const mShare =
                      metrics.totalRequests > 0
                        ? Math.round((m.count / metrics.totalRequests) * 100)
                        : 0;

                    return (
                      <div key={m.model} className="py-2.5 first:pt-1 last:pb-1 space-y-1.5">
                        <div className="flex items-center justify-between gap-2">
                          <div className="flex items-center gap-2 min-w-0">
                            <span className="w-4 text-[11px] font-mono font-bold text-zinc-400">
                              #{index + 1}
                            </span>
                            <span
                              className="text-xs font-mono font-semibold text-zinc-900 dark:text-zinc-100 truncate max-w-[210px]"
                              title={m.model}
                            >
                              {m.model}
                            </span>
                            {m.targetModel && m.targetModel !== m.model && (
                              <span
                                className="text-[10px] font-mono text-zinc-400 truncate max-w-[110px]"
                                title={`${locale === 'zh-CN' ? '映射至' : 'Mapped to'}: ${m.targetModel}`}
                              >
                                → {m.targetModel}
                              </span>
                            )}
                          </div>
                          <div className="flex items-center gap-2 shrink-0 text-xs font-mono">
                            <span className="font-bold text-zinc-900 dark:text-zinc-100">
                              {m.count} {locale === 'zh-CN' ? '次' : 'reqs'}
                            </span>
                            <span className="text-[11px] text-zinc-400">({mShare}%)</span>
                          </div>
                        </div>

                        {/* 占比条 */}
                        <div className="w-full h-1.5 bg-zinc-500/[0.08] dark:bg-white/[0.08] rounded-full overflow-hidden">
                          <div
                            className="h-full bg-blue-500 dark:bg-blue-400 rounded-full transition-all duration-300"
                            style={{ width: `${Math.max(2, mShare)}%` }}
                          />
                        </div>

                        {/* 细分指标 */}
                        <div className="flex items-center justify-between text-[11px] text-zinc-400 font-mono">
                          <span>
                            {formatTokens(m.totalTokens)} tok ({formatTokens(m.promptTokens)} {locale === 'zh-CN' ? '入' : 'in'} / {formatTokens(m.completionTokens)} {locale === 'zh-CN' ? '出' : 'out'})
                          </span>
                          <span className="text-emerald-600 dark:text-emerald-400 font-medium">
                            {formatCurrency(m.cost)}
                          </span>
                        </div>
                      </div>
                    );
                  })
                )}
              </div>
            </div>
          </div>

          {/* 3. 下层：客户端来源分布 & 稳定性与异常诊断 (双列网格) */}
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
            {/* 左列：客户端接入统计 */}
            <div className="rounded-lg border border-border bg-card shadow-2xs overflow-hidden">
              <div className="px-4 py-2.5 bg-muted/30 border-b border-border flex items-center justify-between text-xs">
                <div className="flex items-center gap-2 font-semibold text-zinc-800 dark:text-zinc-200">
                  <Terminal className="w-3.5 h-3.5 text-zinc-500" />
                  <span>{t('dashboard.charts.clientDistribution')}</span>
                </div>
                <span className="text-[11px] font-mono text-zinc-400">
                  {metrics.clientList.length} {locale === 'zh-CN' ? '类客户端' : 'clients'}
                </span>
              </div>

              <div className="p-3 divide-y divide-border/60">
                {metrics.clientList.length === 0 ? (
                  <div className="p-6 text-center text-xs text-zinc-400">{locale === 'zh-CN' ? '暂无客户端请求' : 'No client requests'}</div>
                ) : (
                  metrics.clientList.map((c) => {
                    const cShare =
                      metrics.totalRequests > 0
                        ? Math.round((c.count / metrics.totalRequests) * 100)
                        : 0;

                    return (
                      <div key={c.type} className="py-2.5 first:pt-1 last:pb-1 space-y-1.5">
                        <div className="flex items-center justify-between gap-2">
                          <div className="flex items-center gap-2 min-w-0">
                            <div className="w-5 h-5 rounded bg-zinc-500/[0.08] dark:bg-white/[0.08] flex items-center justify-center p-0.5 shrink-0">
                              <ClientLogo type={c.type} className="w-3.5 h-3.5" />
                            </div>
                            <span className="text-xs font-semibold text-zinc-900 dark:text-zinc-100">
                              {formatClientType(c.type, t)}
                            </span>
                          </div>
                          <div className="flex items-center gap-2 text-xs font-mono">
                            <span className="font-bold text-zinc-900 dark:text-zinc-100">
                              {c.count} {locale === 'zh-CN' ? '次' : 'reqs'}
                            </span>
                            <span className="text-[11px] text-zinc-400">({cShare}%)</span>
                          </div>
                        </div>

                        <div className="w-full h-1.5 bg-zinc-500/[0.08] dark:bg-white/[0.08] rounded-full overflow-hidden">
                          <div
                            className="h-full bg-violet-500 dark:bg-violet-400 rounded-full transition-all duration-300"
                            style={{ width: `${Math.max(2, cShare)}%` }}
                          />
                        </div>

                        <div className="flex items-center justify-between text-[11px] text-zinc-400 font-mono">
                          <span>{formatTokens(c.totalTokens)} tok</span>
                          <div className="flex items-center gap-2">
                            {c.errorCount > 0 ? (
                              <span className="text-rose-500">{c.errorCount} {locale === 'zh-CN' ? '失败' : 'failed'}</span>
                            ) : (
                              <span className="text-emerald-600 dark:text-emerald-400">{t('common.normal')}</span>
                            )}
                            <span>·</span>
                            <span className="text-emerald-600 dark:text-emerald-400 font-medium">
                              {formatCurrency(c.cost)}
                            </span>
                          </div>
                        </div>
                      </div>
                    );
                  })
                )}
              </div>
            </div>

            {/* 右列：链路稳定性与异常诊断 */}
            <div className="rounded-lg border border-border bg-card shadow-2xs overflow-hidden flex flex-col justify-between">
              <div>
                <div className="px-4 py-2.5 bg-muted/30 border-b border-border flex items-center justify-between text-xs">
                  <div className="flex items-center gap-2 font-semibold text-zinc-800 dark:text-zinc-200">
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

                <div className="p-4 space-y-3.5">
                  {/* 三色状态码分段进度条 */}
                  <div className="space-y-1.5">
                    <div className="flex items-center justify-between text-[11px] font-mono text-zinc-400">
                      <span>{t('dashboard.charts.statusDistribution')}</span>
                      <span>
                        2xx: {metrics.status2xx} · 4xx: {metrics.status4xx} · 5xx: {metrics.status5xx}
                      </span>
                    </div>
                    <div className="w-full h-2 rounded-full bg-zinc-500/[0.08] dark:bg-white/[0.08] flex overflow-hidden">
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
                  <div className="pt-2 border-t border-border/60">
                    <div className="text-[11px] font-medium text-zinc-600 dark:text-zinc-400 mb-2">
                      {t('dashboard.charts.recentErrors')}
                    </div>

                    {metrics.errorDiagnostics.length === 0 ? (
                      <div className="p-3 rounded-md bg-emerald-50/60 dark:bg-emerald-950/20 border border-emerald-200/50 dark:border-emerald-900/40 flex items-center gap-2 text-xs text-emerald-700 dark:text-emerald-300">
                        <CheckCircle2 className="w-4 h-4 shrink-0 text-emerald-500" />
                        <span>{t('dashboard.charts.noErrors')}</span>
                      </div>
                    ) : (
                      <div className="space-y-1.5">
                        {metrics.errorDiagnostics.slice(0, 4).map(({ reason, count }) => (
                          <div
                            key={reason}
                            className="p-2 rounded-md bg-rose-50/50 dark:bg-rose-950/20 border border-rose-200/50 dark:border-rose-900/40 flex items-center justify-between text-xs gap-2"
                          >
                            <div className="flex items-center gap-1.5 min-w-0">
                              <AlertTriangle className="w-3.5 h-3.5 shrink-0 text-amber-500 dark:text-amber-400" />
                              <span className="font-mono text-zinc-800 dark:text-zinc-200 truncate text-[11px]">
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
              <div className="px-4 py-2 bg-muted/20 border-t border-border flex items-center justify-between text-[10px] text-zinc-400 font-mono">
                <span>{locale === 'zh-CN' ? '兜底策略：触发备用服务商' : 'Fallback triggered:'} {metrics.fallbackRatio}%</span>
                <span>{locale === 'zh-CN' ? '流式长连接 SSE:' : 'Stream SSE:'} {metrics.streamRatio}%</span>
              </div>
            </div>
          </div>

          {/* 4. 24小时活跃时段节奏条 (柱状微图) */}
          <div className="rounded-lg border border-border bg-card p-4 shadow-2xs space-y-2.5">
            <div className="flex items-center justify-between text-xs">
              <div className="flex items-center gap-2 font-semibold text-zinc-800 dark:text-zinc-200">
                <TrendingUp className="w-3.5 h-3.5 text-primary" />
                <span>{t('dashboard.charts.requestsOverTime')}</span>
              </div>
              <span className="text-[11px] text-zinc-400 font-mono">
                {locale === 'zh-CN' ? '峰值' : 'Peak'}: {maxHourlyCount > 1 ? `${maxHourlyCount} ${locale === 'zh-CN' ? '次/时' : 'reqs/h'}` : '-'}
              </span>
            </div>

            {/* 24 小时柱状条 */}
            <div className="flex items-end gap-1 h-12 pt-1 pb-1">
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
    </div>
  );
};
