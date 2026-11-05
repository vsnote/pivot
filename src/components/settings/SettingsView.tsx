import React, { useState, useRef, useEffect } from 'react';
import { useAppStore } from '@/store/useAppStore';
import { useTranslation, type Locale } from '@/i18n';
import { Tabs, TabsList, TabsTrigger } from '@/components/common/Tabs';
import { Button } from '@/components/common/Button';
import { PivotLogo } from '@/components/common/PivotLogo';
import { check, type Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { getVersion } from '@tauri-apps/api/app';
import { openUrl } from '@tauri-apps/plugin-opener';
import {
  Download,
  Upload,
  CheckCircle2,
  AlertCircle,
  RefreshCw,
  Sparkles,
  ArrowUpCircle,
  ExternalLink,
} from 'lucide-react';

const themeKeys = ['light', 'dark', 'system'] as const;

export const SettingsView: React.FC = () => {
  const { t, locale, setLocale } = useTranslation();
  const {
    theme,
    setTheme,
    providers,
    saveProvider,
  } = useAppStore();

  const [isImporting, setIsImporting] = useState(false);
  const [backupNotice, setBackupNotice] = useState<{
    type: 'success' | 'error';
    message: string;
  } | null>(null);

  const fileInputRef = useRef<HTMLInputElement>(null);

  // 应用版本信息
  const [appVersion, setAppVersion] = useState<string>('');

  useEffect(() => {
    getVersion()
      .then(setAppVersion)
      .catch(() => {
        setAppVersion('0.2.4');
      });
  }, []);

  // 自动更新状态管理
  const [updateState, setUpdateState] = useState<
    'idle' | 'checking' | 'available' | 'up-to-date' | 'downloading' | 'ready' | 'error'
  >('idle');
  const [updateInstance, setUpdateInstance] = useState<Update | null>(null);
  const [downloadProgress, setDownloadProgress] = useState(0);
  const [updateError, setUpdateError] = useState('');

  const formatUpdateError = (err: unknown): string => {
    const msg = err instanceof Error ? err.message : String(err);
    const lower = msg.toLowerCase();

    if (lower.includes('could not fetch a valid release json') || lower.includes('404')) {
      return locale === 'zh-CN'
        ? '获取更新信息失败（未找到有效发布版本或网络无法访问）'
        : 'Failed to fetch update info (No release found or network unreachable)';
    }
    if (lower.includes('timed out') || lower.includes('timeout') || lower.includes('connect')) {
      return locale === 'zh-CN'
        ? '连接 GitHub 更新服务器超时，请检查网络'
        : 'Connection to GitHub update server timed out, please check your network';
    }
    if (lower.includes('signature')) {
      return locale === 'zh-CN'
        ? '升级包签名校验失败，安全终止'
        : 'Update signature verification failed, aborted for security';
    }
    if (lower.includes('failed to send request') || lower.includes('network')) {
      return locale === 'zh-CN'
        ? '网络请求失败，请检查网络连接或代理'
        : 'Network request failed, please check network connection or proxy';
    }
    return msg || (locale === 'zh-CN' ? '检查更新失败，请稍后重试' : 'Check for updates failed, please try again later');
  };

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
        setUpdateError(formatUpdateError(err));
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
      setUpdateError(formatUpdateError(err));
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

  // 打开 GitHub 仓库
  const handleOpenRepo = async () => {
    try {
      await openUrl('https://github.com/vsnote/pivot');
    } catch {
      window.open('https://github.com/vsnote/pivot', '_blank');
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
        message: t('settings.exportSuccess', { count: providers.length }),
      });
      setTimeout(() => setBackupNotice(null), 3500);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      setBackupNotice({ type: 'error', message: t('settings.exportFailed', { error: msg }) });
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
        throw new Error(locale === 'zh-CN' ? '无效的备份文件：缺少 providers 配置列表' : 'Invalid backup: missing providers array');
      }

      let importedCount = 0;
      for (const p of data.providers) {
        if (p && p.id && p.name) {
          await saveProvider(p);
          importedCount++;
        }
      }

      setBackupNotice({
        type: 'success',
        message: t('settings.restoreSuccess', { count: importedCount }),
      });
      setTimeout(() => setBackupNotice(null), 4000);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      setBackupNotice({ type: 'error', message: t('settings.restoreFailed', { error: msg }) });
    } finally {
      setIsImporting(false);
      if (fileInputRef.current) {
        fileInputRef.current.value = '';
      }
    }
  };

  return (
    <div className="w-full space-y-3.5 animate-in fade-in duration-150">
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
            {t('common.close')}
          </button>
        </div>
      )}

      {/* 卡片 1：偏好设置（外观与语言） */}
      <div className="rounded-lg border border-border bg-card text-card-foreground shadow-2xs overflow-hidden">
        <div className="px-4 py-2.5 bg-muted/30 border-b border-border">
          <span className="text-xs font-semibold text-zinc-700 dark:text-zinc-200 tracking-wide">
            {t('settings.generalPreferences')}
          </span>
        </div>

        {/* 界面外观 */}
        <div className="px-4 py-3 flex items-center justify-between gap-3">
          <span className="text-xs font-medium text-zinc-900 dark:text-zinc-100">
            {t('settings.appearance')}
          </span>

          <Tabs value={theme} onValueChange={(val) => setTheme(val as 'light' | 'dark' | 'system')} className="shrink-0">
            <TabsList aria-label={t('settings.appearance')}>
              {themeKeys.map((id) => (
                <TabsTrigger key={id} value={id} className="px-3">
                  {t(`settings.themes.${id}`)}
                </TabsTrigger>
              ))}
            </TabsList>
          </Tabs>
        </div>

        {/* 分隔线 */}
        <div className="border-t border-border/60" />

        {/* 界面语言 */}
        <div className="px-4 py-3 flex items-center justify-between gap-3">
          <span className="text-xs font-medium text-zinc-900 dark:text-zinc-100">
            {t('settings.language')}
          </span>

          <Tabs value={locale} onValueChange={(val) => setLocale(val as Locale)} className="shrink-0">
            <TabsList aria-label={t('settings.language')}>
              <TabsTrigger value="zh-CN" className="px-3">
                {t('settings.languages.zhCN')}
              </TabsTrigger>
              <TabsTrigger value="en" className="px-3">
                {t('settings.languages.en')}
              </TabsTrigger>
            </TabsList>
          </Tabs>
        </div>
      </div>

      {/* 卡片 2：配置数据备份与恢复 */}
      <div className="rounded-lg border border-border bg-card text-card-foreground shadow-2xs overflow-hidden">
        <div className="px-4 py-2.5 bg-muted/30 border-b border-border">
          <span className="text-xs font-semibold text-zinc-700 dark:text-zinc-200 tracking-wide">
            {t('settings.backupSection')}
          </span>
        </div>

        <div className="p-4 space-y-3.5">
          <div className="text-[11px] text-zinc-500 dark:text-zinc-400 leading-relaxed">
            {t('settings.backupDesc')}
          </div>

          <div className="flex flex-wrap items-center gap-2.5 pt-1">
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={handleExportBackup}
              title={t('settings.exportBackup')}
            >
              <Download className="w-3.5 h-3.5" />
              <span>{t('settings.exportBackup')}</span>
            </Button>

            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => fileInputRef.current?.click()}
              disabled={isImporting}
              title={t('settings.restoreBackup')}
            >
              <Upload className="w-3.5 h-3.5" />
              <span>{isImporting ? t('settings.restoring') : t('settings.restoreBackup')}</span>
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

      {/* 卡片 3：关于 Pivot 与更新 */}
      <div className="rounded-lg border border-border bg-card text-card-foreground shadow-2xs overflow-hidden">
        <div className="px-4 py-2.5 bg-muted/30 border-b border-border flex items-center justify-between">
          <span className="text-xs font-semibold text-zinc-700 dark:text-zinc-200 tracking-wide">
            {t('settings.aboutPivot')}
          </span>
          <span className="text-[10px] font-mono text-zinc-400">
            {appVersion ? `v${appVersion}` : ''}
          </span>
        </div>

        <div className="p-4 space-y-3.5">
          {/* Logo 与主定位 */}
          <div className="flex items-start gap-3">
            <PivotLogo variant="icon" className="w-9 h-9 rounded-lg shadow-xs shrink-0 mt-0.5" />
            <div className="min-w-0 flex-1">
              <div className="text-xs font-semibold text-zinc-900 dark:text-zinc-100">
                Pivot
              </div>
              <p className="text-[11px] text-zinc-500 dark:text-zinc-400 mt-0.5 leading-relaxed">
                {t('settings.pivotDesc')}
              </p>
            </div>
          </div>

          {/* 分隔线 */}
          <div className="border-t border-border/60" />

          {/* 软件更新状态与操作行 */}
          <div className="flex items-center justify-between gap-3">
            <div className="min-w-0">
              <div className="text-xs font-medium text-zinc-900 dark:text-zinc-100 flex items-center gap-1.5 flex-wrap">
                <span>{t('settings.softwareUpdate')}</span>
                {updateState === 'available' && (
                  <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium bg-blue-500/10 text-blue-600 dark:text-blue-400 border border-blue-500/20">
                    <Sparkles className="w-2.5 h-2.5" />
                    {t('settings.newVersionFound', { version: updateInstance?.version ?? '' })}
                  </span>
                )}
                {updateState === 'up-to-date' && (
                  <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium bg-emerald-500/10 text-emerald-600 dark:text-emerald-400">
                    <CheckCircle2 className="w-2.5 h-2.5" />
                    {t('settings.upToDate')}
                  </span>
                )}
              </div>
            </div>

            <div className="flex items-center gap-2 shrink-0">
              {updateState === 'ready' ? (
                <Button
                  type="button"
                  size="sm"
                  onClick={handleRelaunch}
                  className="bg-emerald-600 hover:bg-emerald-500 text-white cursor-pointer"
                >
                  <RefreshCw className="w-3.5 h-3.5" />
                  <span>{t('settings.restartToApply')}</span>
                </Button>
              ) : updateState === 'available' ? (
                <Button
                  type="button"
                  size="sm"
                  onClick={handleDownloadAndInstall}
                  className="bg-primary text-primary-foreground hover:bg-primary/90 cursor-pointer"
                >
                  <ArrowUpCircle className="w-3.5 h-3.5" />
                  <span>{t('settings.updateToVersion', { version: updateInstance?.version ?? '' })}</span>
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
                  <span>{updateState === 'checking' ? t('settings.checkingUpdate') : t('settings.checkUpdate')}</span>
                </Button>
              )}
            </div>
          </div>

          {/* 下载进度条 */}
          {updateState === 'downloading' && (
            <div className="p-3 rounded-md bg-muted/40 border border-border space-y-1.5 animate-in fade-in">
              <div className="flex items-center justify-between text-[11px]">
                <span className="text-zinc-600 dark:text-zinc-300">{t('settings.downloadingUpdate')}</span>
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
              <div className="font-medium text-zinc-700 dark:text-zinc-300">{t('settings.releaseNotesSummary')}</div>
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
                {t('settings.ignoreError')}
              </button>
            </div>
          )}

          {/* 底部存储与外链 */}
          <div className="pt-2 border-t border-border/60 flex flex-col sm:flex-row sm:items-center justify-between gap-1.5 text-[11px] text-zinc-400 dark:text-zinc-500">
            <span className="font-mono text-[10px]">
              {t('settings.localDatabaseNotice')}
            </span>
            <button
              type="button"
              onClick={handleOpenRepo}
              className="inline-flex items-center gap-1 text-primary hover:underline cursor-pointer text-[11px] self-start sm:self-auto"
            >
              <span>{t('settings.githubRepo')}</span>
              <ExternalLink className="w-3 h-3" />
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};
