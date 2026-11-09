import React, { useState, useRef, useEffect } from 'react';
import { useAppStore } from '@/store/useAppStore';
import { useUpdateStore } from '@/store/useUpdateStore';
import { useTranslation, type Locale } from '@/i18n';
import { Tabs, TabsList, TabsTrigger } from '@/components/common/Tabs';
import { Button } from '@/components/common/Button';
import { PivotLogo } from '@/components/common/PivotLogo';
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

  // 全局自动更新状态管理
  const {
    updateState,
    updateVersion,
    updateBody,
    downloadProgress,
    updateError,
    checkUpdate,
    downloadAndInstall,
    relaunch,
    openReleasePage,
    clearError: clearUpdateError,
  } = useUpdateStore();

  // 打开 GitHub 仓库
  const handleOpenRepo = async () => {
    try {
      await openUrl('https://github.com/vsnote/pivot');
    } catch {
      window.open('https://github.com/vsnote/pivot', '_blank');
    }
  };

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

      {/* 卡片：常用快捷键 */}
      <div className="rounded-lg border border-border bg-card text-card-foreground shadow-2xs overflow-hidden">
        <div className="px-4 py-2.5 bg-muted/30 border-b border-border">
          <span className="text-xs font-semibold text-zinc-700 dark:text-zinc-200 tracking-wide">
            {t('settings.shortcutsSection')}
          </span>
        </div>

        <div className="divide-y divide-border/60 text-xs">
          <div className="px-4 py-2.5 flex items-center justify-between gap-3">
            <span className="text-zinc-700 dark:text-zinc-300">
              {t('settings.shortcuts.switchViews')}
            </span>
            <kbd className="px-2 py-0.5 rounded border border-border bg-muted/40 font-mono text-[11px] text-zinc-600 dark:text-zinc-300 shadow-2xs">
              {typeof navigator !== 'undefined' && navigator.platform.toUpperCase().includes('MAC') ? '⌘ 1 ~ 4' : 'Ctrl + 1 ~ 4'}
            </kbd>
          </div>

          <div className="px-4 py-2.5 flex items-center justify-between gap-3">
            <span className="text-zinc-700 dark:text-zinc-300">
              {t('settings.shortcuts.newProvider')}
            </span>
            <kbd className="px-2 py-0.5 rounded border border-border bg-muted/40 font-mono text-[11px] text-zinc-600 dark:text-zinc-300 shadow-2xs">
              {typeof navigator !== 'undefined' && navigator.platform.toUpperCase().includes('MAC') ? '⌘ N' : 'Ctrl + N'}
            </kbd>
          </div>

          <div className="px-4 py-2.5 flex items-center justify-between gap-3">
            <span className="text-zinc-700 dark:text-zinc-300">
              {t('settings.shortcuts.backOrCancel')}
            </span>
            <kbd className="px-2 py-0.5 rounded border border-border bg-muted/40 font-mono text-[11px] text-zinc-600 dark:text-zinc-300 shadow-2xs">
              Esc
            </kbd>
          </div>
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

      {/* 卡片 3：关于 Pivot 与版本管理 */}
      <div className="rounded-lg border border-border bg-card text-card-foreground shadow-2xs overflow-hidden">
        <div className="px-4 py-2.5 bg-muted/30 border-b border-border flex items-center justify-between">
          <span className="text-xs font-semibold text-zinc-700 dark:text-zinc-200 tracking-wide">
            {t('settings.aboutPivot')}
          </span>
        </div>

        <div className="p-4 space-y-3.5">
          {/* Logo、主定位与版本操作区 */}
          <div className="flex items-start justify-between gap-3">
            <div className="flex items-center gap-3 min-w-0">
              <PivotLogo variant="icon" className="w-10 h-10 rounded-xl shadow-xs shrink-0" />
              <div className="min-w-0">
                <div className="flex items-center gap-2 flex-wrap">
                  <span className="text-sm font-semibold text-zinc-900 dark:text-zinc-100">
                    Pivot
                  </span>
                  <span className="px-1.5 py-0.5 rounded text-[10px] font-mono font-medium bg-zinc-100 dark:bg-zinc-800 text-zinc-600 dark:text-zinc-300 border border-zinc-200/80 dark:border-zinc-700/80">
                    {appVersion ? `v${appVersion}` : 'v0.2.10'}
                  </span>
                  {updateState === 'up-to-date' && (
                    <span className="inline-flex items-center gap-1 text-[10px] text-emerald-600 dark:text-emerald-400 font-medium animate-in fade-in">
                      <CheckCircle2 className="w-2.5 h-2.5" />
                      {t('settings.upToDate')}
                    </span>
                  )}
                </div>
                <p className="text-[11px] text-zinc-500 dark:text-zinc-400 mt-0.5 leading-relaxed">
                  {t('settings.pivotDesc')}
                </p>
              </div>
            </div>

            <div className="shrink-0 flex items-center pt-0.5">
              {updateState === 'ready' ? (
                <Button
                  type="button"
                  size="sm"
                  onClick={relaunch}
                  className="bg-emerald-600 hover:bg-emerald-500 text-white cursor-pointer h-7 text-xs"
                >
                  <RefreshCw className="w-3.5 h-3.5" />
                  <span>{t('settings.restartToApply')}</span>
                </Button>
              ) : updateState === 'available' ? (
                <Button
                  type="button"
                  size="sm"
                  onClick={downloadAndInstall}
                  className="bg-primary text-primary-foreground hover:bg-primary/90 cursor-pointer h-7 text-xs"
                >
                  <ArrowUpCircle className="w-3.5 h-3.5" />
                  <span>{t('settings.updateToVersion', { version: updateVersion })}</span>
                </Button>
              ) : (
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={() => checkUpdate(true)}
                  disabled={updateState === 'checking' || updateState === 'downloading'}
                  className="h-7 text-xs text-zinc-600 dark:text-zinc-300 cursor-pointer"
                >
                  <RefreshCw className={`w-3 h-3 ${updateState === 'checking' ? 'animate-spin' : ''}`} />
                  <span>{updateState === 'checking' ? t('settings.checkingUpdate') : t('settings.checkUpdate')}</span>
                </Button>
              )}
            </div>
          </div>

          {/* 发现新版本时的更新卡片与更新日志 */}
          {updateState === 'available' && (
            <div className="p-3 rounded-md bg-blue-500/10 border border-blue-500/20 text-[11px] space-y-2 animate-in fade-in">
              <div className="flex items-center gap-1.5 font-medium text-blue-700 dark:text-blue-300">
                <Sparkles className="w-3.5 h-3.5 text-blue-500" />
                <span>{t('settings.newVersionFound', { version: updateVersion })}</span>
              </div>
              {updateBody && (
                <div className="text-zinc-600 dark:text-zinc-300 whitespace-pre-wrap leading-relaxed max-h-36 overflow-y-auto text-[10px] bg-background/60 p-2 rounded border border-border/40">
                  {updateBody}
                </div>
              )}
            </div>
          )}

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

          {/* 错误提示与降级通道 */}
          {updateState === 'error' && updateError && (
            <div className="p-3 rounded-md bg-rose-500/10 border border-rose-500/20 text-rose-700 dark:text-rose-300 text-[11px] space-y-2">
              <div className="flex items-start justify-between gap-2">
                <div className="flex items-center gap-1.5 min-w-0">
                  <AlertCircle className="w-3.5 h-3.5 shrink-0 text-rose-500" />
                  <span className="font-medium">{updateError}</span>
                </div>
                <button
                  type="button"
                  onClick={clearUpdateError}
                  className="opacity-70 hover:opacity-100 text-[10px] shrink-0 cursor-pointer"
                >
                  {t('settings.ignoreError')}
                </button>
              </div>

              <p className="text-[10px] text-rose-600/80 dark:text-rose-300/80 leading-relaxed">
                {t('settings.manualDownloadHint')}
              </p>

              <div className="flex items-center gap-2 pt-0.5">
                <Button
                  type="button"
                  size="sm"
                  variant="outline"
                  onClick={() => checkUpdate(true)}
                  className="h-6 px-2 text-[10px] border-rose-300 dark:border-rose-800 text-rose-700 dark:text-rose-300 hover:bg-rose-500/10"
                >
                  <RefreshCw className="w-3 h-3" />
                  <span>{t('settings.retryUpdate')}</span>
                </Button>
                <Button
                  type="button"
                  size="sm"
                  onClick={openReleasePage}
                  className="h-6 px-2 text-[10px] bg-rose-600 hover:bg-rose-500 text-white cursor-pointer"
                >
                  <ExternalLink className="w-3 h-3" />
                  <span>{t('settings.downloadManually')}</span>
                </Button>
              </div>
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
