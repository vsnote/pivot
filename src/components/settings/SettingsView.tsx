import React, { useState, useRef } from 'react';
import { useAppStore } from '@/store/useAppStore';
import { Tabs, TabsList, TabsTrigger } from '@/components/common/Tabs';
import { Button } from '@/components/common/Button';
import { formatTokens } from '@/utils/pricing';
import {
  Copy,
  Check,
  Download,
  Upload,
  Radio,
  Clock,
  Zap,
  ShieldCheck,
  CheckCircle2,
  AlertCircle,
  FileJson,
} from 'lucide-react';

const themeOptions = [
  { id: 'light', label: '浅色' },
  { id: 'dark', label: '深色' },
  { id: 'system', label: '跟随系统' },
] as const;

function formatUptime(secs: number): string {
  if (!secs || secs <= 0) return '刚启动';
  const hours = Math.floor(secs / 3600);
  const minutes = Math.floor((secs % 3600) / 60);
  if (hours > 0) {
    return `${hours} 小时 ${minutes} 分钟`;
  }
  return `${Math.max(1, minutes)} 分钟`;
}

export const SettingsView: React.FC = () => {
  const {
    theme,
    setTheme,
    stats,
    providers,
    routes,
    saveProvider,
    saveRoutes,
  } = useAppStore();

  const [copiedAddress, setCopiedAddress] = useState(false);
  const [isImporting, setIsImporting] = useState(false);
  const [backupNotice, setBackupNotice] = useState<{
    type: 'success' | 'error';
    message: string;
  } | null>(null);

  const fileInputRef = useRef<HTMLInputElement>(null);

  const gatewayAddress = `http://${stats.host}:${stats.port}`;
  const totalTokens = (stats.totalPromptTokens || 0) + (stats.totalCompletionTokens || 0);

  const copyGatewayAddress = () => {
    navigator.clipboard.writeText(gatewayAddress);
    setCopiedAddress(true);
    setTimeout(() => setCopiedAddress(false), 2000);
  };

  const handleExportBackup = () => {
    try {
      const data = {
        app: 'pivot',
        version: 1,
        exportedAt: new Date().toISOString(),
        providers,
        routes,
      };
      const jsonStr = JSON.stringify(data, null, 2);
      const blob = new Blob([jsonStr], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const dateStr = new Date().toISOString().slice(0, 10);
      const link = document.createElement('a');
      link.href = url;
      link.download = `pivot-backup-${dateStr}.json`;
      document.body.appendChild(link);
      link.click();
      document.body.removeChild(link);
      URL.revokeObjectURL(url);

      setBackupNotice({
        type: 'success',
        message: `已成功导出 ${providers.length} 个服务商与 ${routes.length} 条路由规则`,
      });
      setTimeout(() => setBackupNotice(null), 3500);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      setBackupNotice({ type: 'error', message: `导出失败: ${msg}` });
    }
  };

  const handleImportFile = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;

    try {
      setIsImporting(true);
      const text = await file.text();
      const data = JSON.parse(text);

      if (!data || !Array.isArray(data.providers)) {
        throw new Error('无效的备份文件：缺少 providers 配置列表');
      }

      let importedCount = 0;
      for (const p of data.providers) {
        if (p && p.id && p.name) {
          await saveProvider(p);
          importedCount++;
        }
      }

      if (Array.isArray(data.routes) && data.routes.length > 0) {
        await saveRoutes(data.routes);
      }

      setBackupNotice({
        type: 'success',
        message: `已恢复 ${importedCount} 个服务商与 ${data.routes?.length || 0} 条路由规则`,
      });
      setTimeout(() => setBackupNotice(null), 4000);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      setBackupNotice({ type: 'error', message: `导入失败: ${msg}` });
    } finally {
      setIsImporting(false);
      if (fileInputRef.current) {
        fileInputRef.current.value = '';
      }
    }
  };

  return (
    <div className="w-full pb-10 space-y-3.5 animate-in fade-in duration-150">
      {/* 备份导入/导出状态通知条 */}
      {backupNotice && (
        <div
          role="status"
          className={`px-4 py-2.5 rounded-lg border text-xs flex items-center justify-between gap-3 shadow-2xs animate-in fade-in duration-150 ${
            backupNotice.type === 'success'
              ? 'bg-emerald-50 dark:bg-emerald-950/40 border-emerald-200 dark:border-emerald-800/60 text-emerald-700 dark:text-emerald-300'
              : 'bg-rose-50 dark:bg-rose-950/40 border-rose-200 dark:border-rose-800/60 text-rose-700 dark:text-rose-300'
          }`}
        >
          <div className="flex items-center gap-2">
            {backupNotice.type === 'success' ? (
              <CheckCircle2 className="w-4 h-4 shrink-0 text-emerald-500" />
            ) : (
              <AlertCircle className="w-4 h-4 shrink-0 text-rose-500" />
            )}
            <span>{backupNotice.message}</span>
          </div>
          <button
            type="button"
            onClick={() => setBackupNotice(null)}
            className="text-[11px] opacity-70 hover:opacity-100 cursor-pointer"
          >
            关闭
          </button>
        </div>
      )}

      {/* 卡片 1：偏好设置（外观） */}
      <div className="rounded-lg border border-border bg-card text-card-foreground shadow-2xs overflow-hidden">
        <div className="px-4 py-2.5 bg-muted/30 border-b border-border">
          <span className="text-xs font-semibold text-zinc-700 dark:text-zinc-200 tracking-wide">
            通用偏好
          </span>
        </div>

        <div className="p-4 flex items-center justify-between gap-4">
          <div className="min-w-0">
            <div className="text-xs font-medium text-zinc-900 dark:text-zinc-100">界面外观</div>
            <div className="text-[11px] text-zinc-400 dark:text-zinc-500 mt-0.5">
              选择适合的显示风格，或跟随操作系统自动切换
            </div>
          </div>

          <Tabs value={theme} onValueChange={(val) => setTheme(val as 'light' | 'dark' | 'system')}>
            <TabsList aria-label="主题偏好切换">
              {themeOptions.map((opt) => (
                <TabsTrigger key={opt.id} value={opt.id}>
                  {opt.label}
                </TabsTrigger>
              ))}
            </TabsList>
          </Tabs>
        </div>
      </div>

      {/* 卡片 2：本地网关状态 */}
      <div className="rounded-lg border border-border bg-card text-card-foreground shadow-2xs overflow-hidden">
        <div className="px-4 py-2.5 bg-muted/30 border-b border-border flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Radio className="w-3.5 h-3.5 text-zinc-500" />
            <span className="text-xs font-semibold text-zinc-700 dark:text-zinc-200 tracking-wide">
              本地网关服务
            </span>
          </div>
          <div className="flex items-center gap-1.5 text-[11px] font-mono">
            <span
              className={`w-2 h-2 rounded-full ${
                stats.isRunning
                  ? 'bg-emerald-500 shadow-[0_0_6px_rgba(16,185,129,0.6)]'
                  : 'bg-rose-500'
              }`}
            />
            <span className={stats.isRunning ? 'text-emerald-600 dark:text-emerald-400 font-medium' : 'text-rose-500 font-medium'}>
              {stats.isRunning ? '正在运行' : '未就绪'}
            </span>
          </div>
        </div>

        <div className="p-4 space-y-4">
          {/* 地址展示与复制行 */}
          <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-2.5 p-3 rounded-md bg-muted/30 border border-border">
            <div className="min-w-0">
              <span className="text-[11px] text-zinc-400 block mb-0.5">网关服务地址</span>
              <span className="font-mono text-xs font-semibold text-zinc-900 dark:text-zinc-100 select-all">
                {gatewayAddress}
              </span>
            </div>
            <Button
              type="button"
              variant="outline"
              size="xs"
              onClick={copyGatewayAddress}
              className="self-start sm:self-center"
            >
              {copiedAddress ? (
                <Check className="w-3.5 h-3.5 text-emerald-500" />
              ) : (
                <Copy className="w-3.5 h-3.5" />
              )}
              <span>{copiedAddress ? '已复制' : '复制地址'}</span>
            </Button>
          </div>

          {/* 网关指标微栅格 */}
          <div className="grid grid-cols-2 sm:grid-cols-4 gap-2.5 text-xs">
            <div className="p-2.5 rounded-md bg-zinc-50/70 dark:bg-zinc-800/50 border border-zinc-200/60 dark:border-zinc-800">
              <span className="text-[10px] text-zinc-400 block flex items-center gap-1">
                <Clock className="w-3 h-3" />
                <span>运行时间</span>
              </span>
              <span className="font-semibold text-zinc-800 dark:text-zinc-200 mt-1 block">
                {formatUptime(stats.uptimeSecs)}
              </span>
            </div>

            <div className="p-2.5 rounded-md bg-zinc-50/70 dark:bg-zinc-800/50 border border-zinc-200/60 dark:border-zinc-800">
              <span className="text-[10px] text-zinc-400 block flex items-center gap-1">
                <Zap className="w-3 h-3" />
                <span>累计请求</span>
              </span>
              <span className="font-mono font-semibold text-zinc-800 dark:text-zinc-200 mt-1 block">
                {stats.totalRequests} 次
              </span>
            </div>

            <div className="p-2.5 rounded-md bg-zinc-50/70 dark:bg-zinc-800/50 border border-zinc-200/60 dark:border-zinc-800">
              <span className="text-[10px] text-zinc-400 block flex items-center gap-1">
                <FileJson className="w-3 h-3" />
                <span>累计 Token</span>
              </span>
              <span className="font-mono font-semibold text-zinc-800 dark:text-zinc-200 mt-1 block">
                {totalTokens > 0 ? formatTokens(totalTokens) : '0'}
              </span>
            </div>

            <div className="p-2.5 rounded-md bg-zinc-50/70 dark:bg-zinc-800/50 border border-zinc-200/60 dark:border-zinc-800">
              <span className="text-[10px] text-zinc-400 block flex items-center gap-1">
                <ShieldCheck className="w-3 h-3" />
                <span>异常请求</span>
              </span>
              <span
                className={`font-mono font-semibold mt-1 block ${
                  stats.errorCount > 0 ? 'text-rose-500' : 'text-zinc-800 dark:text-zinc-200'
                }`}
              >
                {stats.errorCount} 次
              </span>
            </div>
          </div>
        </div>
      </div>

      {/* 卡片 3：配置数据备份与恢复 */}
      <div className="rounded-lg border border-border bg-card text-card-foreground shadow-2xs overflow-hidden">
        <div className="px-4 py-2.5 bg-muted/30 border-b border-border">
          <span className="text-xs font-semibold text-zinc-700 dark:text-zinc-200 tracking-wide">
            配置备份与迁移
          </span>
        </div>

        <div className="p-4 space-y-3.5">
          <div className="text-[11px] text-zinc-500 dark:text-zinc-400 leading-relaxed">
            将当前已配置的所有服务商密钥、模型槽位及自定义路由规则导出为加密或纯净 JSON 备份文件，可在重装系统或换机时一键无缝恢复。
          </div>

          <div className="flex flex-wrap items-center gap-2.5 pt-1">
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={handleExportBackup}
              title="导出当前所有服务商与路由规则"
            >
              <Download className="w-3.5 h-3.5" />
              <span>导出配置备份 (.json)</span>
            </Button>

            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => fileInputRef.current?.click()}
              disabled={isImporting}
              title="从备份文件导入配置"
            >
              <Upload className="w-3.5 h-3.5" />
              <span>{isImporting ? '正在恢复…' : '从备份文件恢复'}</span>
            </Button>

            {/* 隐藏的文件上传 input */}
            <input
              ref={fileInputRef}
              type="file"
              accept=".json,application/json"
              onChange={handleImportFile}
              className="hidden"
            />
          </div>
        </div>
      </div>

      {/* 卡片 4：关于信息与架构理念 */}
      <div className="pt-3 text-center space-y-1 text-[11px] text-zinc-400 dark:text-zinc-500">
        <div className="flex items-center justify-center gap-1.5">
          <span className="text-xs font-bold tracking-wider text-zinc-800 dark:text-zinc-200 font-mono">
            PIVOT
          </span>
          <span className="text-[10px] font-mono px-1.5 py-0.2 rounded bg-zinc-100 dark:bg-zinc-800 text-zinc-600 dark:text-zinc-400 border border-zinc-200/60 dark:border-zinc-700/60">
            v0.1.0-alpha
          </span>
        </div>
        <p>面向开发者的本地 AI 智能网关与模型中继器</p>
        <p className="text-[10px] text-zinc-400 dark:text-zinc-600 pt-0.5">
          纯本地运行 · 密钥与配置均保存在本机 Keychain 与本地 SQLite · 无任何云端遥测
        </p>
      </div>
    </div>
  );
};
