import React, { useState, useRef, useEffect } from 'react';
import { useAppStore } from '@/store/useAppStore';
import { Tabs, TabsList, TabsTrigger } from '@/components/common/Tabs';
import { Button } from '@/components/common/Button';
import { PivotLogo } from '@/components/common/PivotLogo';
import { check, type Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import {
  Download,
  Upload,
  CheckCircle2,
  AlertCircle,
  RefreshCw,
  Sparkles,
  ArrowUpCircle,
} from 'lucide-react';

const themeOptions = [
  { id: 'light', label: '浅色' },
  { id: 'dark', label: '深色' },
  { id: 'system', label: '跟随系统' },
] as const;

export const SettingsView: React.FC = () => {
  const {
    theme,
    setTheme,
    providers,
    routes,
    saveProvider,
    saveRoutes,
  } = useAppStore();

  const [isImporting, setIsImporting] = useState(false);
  const [backupNotice, setBackupNotice] = useState<{
    type: 'success' | 'error';
    message: string;
  } | null>(null);

  const fileInputRef = useRef<HTMLInputElement>(null);

  // 自动更新状态管理
  const [updateState, setUpdateState] = useState<
    'idle' | 'checking' | 'available' | 'up-to-date' | 'downloading' | 'ready' | 'error'
  >('idle');
  const [updateInstance, setUpdateInstance] = useState<Update | null>(null);
  const [downloadProgress, setDownloadProgress] = useState(0);
  const [updateError, setUpdateError] = useState('');

  // 检查更新
  const handleCheckUpdate = async (isManual = true) => {
    try {
      setUpdateState('checking');
      setUpdateError('');
      const update = await check();
      if (update && update.available) {
        setUpdateInstance(update);
        setUpdateState('available');
      } else {
        setUpdateState('up-to-date');
        setTimeout(() => {
          setUpdateState((prev) => (prev === 'up-to-date' ? 'idle' : prev));
        }, 3500);
      }
    } catch (err: unknown) {
      if (isManual) {
        setUpdateState('error');
        const msg = err instanceof Error ? err.message : String(err);
        setUpdateError(msg.includes('404') ? '未检测到新发布版本' : msg);
      } else {
        setUpdateState('idle');
      }
    }
  };

  // 下载并安装更新
  const handleDownloadAndInstall = async () => {
    if (!updateInstance) return;
    try {
      setUpdateState('downloading');
      setDownloadProgress(0);
      let downloaded = 0;
      let total = 0;

      await updateInstance.downloadAndInstall((event) => {
        if (event.event === 'Started') {
          total = event.data.contentLength || 0;
        } else if (event.event === 'Progress') {
          downloaded += event.data.chunkLength;
          if (total > 0) {
            setDownloadProgress(Math.round((downloaded / total) * 100));
          }
        } else if (event.event === 'Finished') {
          setDownloadProgress(100);
        }
      });

      setUpdateState('ready');
    } catch (err: unknown) {
      setUpdateState('error');
      setUpdateError(err instanceof Error ? err.message : String(err));
    }
  };

  // 重启应用
  const handleRelaunch = async () => {
    try {
      await relaunch();
    } catch (err) {
      console.error('重启应用失败:', err);
    }
  };

  // 启动时在后台静默检测一次是否有新版本
  useEffect(() => {
    const timer = setTimeout(() => {
      handleCheckUpdate(false);
    }, 2000);
    return () => clearTimeout(timer);
  }, []);

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
          className={`px-4 py-2.5 rounded-lg text-xs flex items-center justify-between gap-3 animate-in fade-in duration-150 ${
            backupNotice.type === 'success'
              ? 'bg-emerald-500/10 text-emerald-700 dark:text-emerald-300'
              : 'bg-rose-500/10 text-rose-700 dark:text-rose-300'
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
                <TabsTrigger key={opt.id} value={opt.id} className="w-[76px]">
                  {opt.label}
                </TabsTrigger>
              ))}
            </TabsList>
          </Tabs>
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
            导出全部服务商、密钥与路由规则为单个 JSON 文件，换机或重装后可据此恢复。备份是明文的，含密钥，请妥善保管。
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

      {/* 卡片：软件更新 */}
      <div className="rounded-lg border border-border bg-card text-card-foreground shadow-2xs overflow-hidden">
        <div className="px-4 py-2.5 bg-muted/30 border-b border-border flex items-center justify-between">
          <span className="text-xs font-semibold text-zinc-700 dark:text-zinc-200 tracking-wide">
            软件更新
          </span>
          <span className="text-[10px] font-mono text-zinc-400">当前版本 v0.1.0</span>
        </div>

        <div className="p-4 space-y-3">
          <div className="flex items-center justify-between gap-4">
            <div className="min-w-0">
              <div className="text-xs font-medium text-zinc-900 dark:text-zinc-100 flex items-center gap-1.5">
                <span>自动检查与升级</span>
                {updateState === 'available' && (
                  <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium bg-blue-500/10 text-blue-600 dark:text-blue-400 border border-blue-500/20">
                    <Sparkles className="w-2.5 h-2.5" />
                    发现新版本 v{updateInstance?.version}
                  </span>
                )}
                {updateState === 'up-to-date' && (
                  <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium bg-emerald-500/10 text-emerald-600 dark:text-emerald-400">
                    <CheckCircle2 className="w-2.5 h-2.5" />
                    已是最新版本
                  </span>
                )}
              </div>
              <div className="text-[11px] text-zinc-400 dark:text-zinc-500 mt-0.5">
                直接从 GitHub Releases 获取经过签名验证的安全升级包
              </div>
            </div>

            <div className="flex items-center gap-2">
              {updateState === 'ready' ? (
                <Button
                  type="button"
                  size="sm"
                  onClick={handleRelaunch}
                  className="bg-emerald-600 hover:bg-emerald-500 text-white cursor-pointer"
                >
                  <RefreshCw className="w-3.5 h-3.5" />
                  <span>重启应用生效</span>
                </Button>
              ) : updateState === 'available' ? (
                <Button
                  type="button"
                  size="sm"
                  onClick={handleDownloadAndInstall}
                  className="bg-primary text-primary-foreground hover:bg-primary/90 cursor-pointer"
                >
                  <ArrowUpCircle className="w-3.5 h-3.5" />
                  <span>更新至 v{updateInstance?.version}</span>
                </Button>
              ) : (
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={() => handleCheckUpdate(true)}
                  disabled={updateState === 'checking' || updateState === 'downloading'}
                >
                  <RefreshCw className={`w-3.5 h-3.5 ${updateState === 'checking' ? 'animate-spin' : ''}`} />
                  <span>{updateState === 'checking' ? '正在检查…' : '检查更新'}</span>
                </Button>
              )}
            </div>
          </div>

          {/* 下载进度条 */}
          {updateState === 'downloading' && (
            <div className="p-3 rounded-md bg-muted/40 border border-border space-y-1.5 animate-in fade-in">
              <div className="flex items-center justify-between text-[11px]">
                <span className="text-zinc-600 dark:text-zinc-300">正在下载升级包…</span>
                <span className="font-mono text-zinc-500">{downloadProgress}%</span>
              </div>
              <div className="w-full h-1.5 bg-zinc-200 dark:bg-zinc-700 rounded-full overflow-hidden">
                <div
                  className="h-full bg-primary transition-all duration-200"
                  style={{ width: `${downloadProgress}%` }}
                />
              </div>
            </div>
          )}

          {/* 发现新版本时的更新日志摘要 */}
          {updateState === 'available' && updateInstance?.body && (
            <div className="p-3 rounded-md bg-muted/40 border border-border text-[11px] space-y-1 animate-in fade-in max-h-36 overflow-y-auto">
              <div className="font-medium text-zinc-700 dark:text-zinc-300">更新内容摘要：</div>
              <div className="text-zinc-500 dark:text-zinc-400 whitespace-pre-wrap leading-relaxed">
                {updateInstance.body}
              </div>
            </div>
          )}

          {/* 错误提示 */}
          {updateState === 'error' && updateError && (
            <div className="px-3 py-2 rounded-md bg-rose-500/10 text-rose-700 dark:text-rose-300 text-[11px] flex items-center justify-between gap-2">
              <div className="flex items-center gap-1.5 min-w-0">
                <AlertCircle className="w-3.5 h-3.5 shrink-0 text-rose-500" />
                <span className="truncate">{updateError}</span>
              </div>
              <button
                type="button"
                onClick={() => setUpdateState('idle')}
                className="opacity-70 hover:opacity-100 text-[10px] shrink-0"
              >
                忽略
              </button>
            </div>
          )}
        </div>
      </div>

      {/* 卡片 4：关于信息与架构理念 */}
      <div className="pt-3 text-center space-y-1.5 text-[11px] text-zinc-400 dark:text-zinc-500">
        <div className="flex items-center justify-center mb-1">
          <PivotLogo variant="icon" className="w-8 h-8 rounded-lg shadow-xs" />
        </div>
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
          纯本地运行 · 密钥与配置保存在本机 ~/.pivot/pivot.db（仅当前系统用户可读写） · 无任何云端遥测
        </p>
      </div>
    </div>
  );
};
