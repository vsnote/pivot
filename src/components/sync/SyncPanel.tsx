import React, { useEffect, useState } from 'react';
import { useAppStore } from '@/store/useAppStore';
import { SyncConfig, SyncStatus, TestConnectionResult, StorageProviderType } from '@/types';
import { Select } from '@/components/common/Select';
import {
  Cloud,
  CloudUpload,
  CloudDownload,
  CheckCircle2,
  AlertCircle,
  RefreshCw,
  Search,
  Settings2,
  HardDrive,
  FolderGit2,
  Layers,
  ArrowUpDown,
  Sparkles,
  Terminal,
  X,
  Check,
  ChevronLeft,
  ChevronRight,
  ChevronDown,
  ChevronsLeft,
  ChevronsRight,
  Trash2,
} from 'lucide-react';

interface StoragePreset {
  id: StorageProviderType;
  name: string;
  desc: string;
  defaultEndpoint: string;
  defaultRegion: string;
  endpointHelp: string;
  needAccountId?: boolean;
}

const STORAGE_PRESETS: StoragePreset[] = [
  {
    id: 'r2',
    name: 'Cloudflare R2',
    desc: '免公网出站流量费，开发与跨设备漫游首选',
    defaultEndpoint: '',
    defaultRegion: 'auto',
    endpointHelp: 'https://<账户ID>.r2.cloudflarestorage.com',
    needAccountId: true,
  },
  {
    id: 's3',
    name: 'Amazon S3',
    desc: 'AWS 官方对象存储，支持全球各可用区',
    defaultEndpoint: 'https://s3.us-east-1.amazonaws.com',
    defaultRegion: 'us-east-1',
    endpointHelp: 'https://s3.<Region>.amazonaws.com',
  },
  {
    id: 'oss',
    name: '阿里云 OSS',
    desc: '阿里云兼容 S3 对象存储',
    defaultEndpoint: 'https://oss-cn-hangzhou.aliyuncs.com',
    defaultRegion: 'oss-cn-hangzhou',
    endpointHelp: 'https://<Region>.aliyuncs.com',
  },
  {
    id: 'cos',
    name: '腾讯云 COS',
    desc: '腾讯云兼容 S3 对象存储',
    defaultEndpoint: 'https://cos.ap-guangzhou.myqcloud.com',
    defaultRegion: 'ap-guangzhou',
    endpointHelp: 'https://cos.<Region>.myqcloud.com',
  },
  {
    id: 'minio',
    name: 'MinIO / 自建 S3',
    desc: '本地 NAS 或自建 S3 实例',
    defaultEndpoint: 'http://127.0.0.1:9000',
    defaultRegion: 'auto',
    endpointHelp: 'http://<IP或域名>:<端口>',
  },
  {
    id: 'custom',
    name: '自定义 S3 兼容',
    desc: '七牛云、华为云 OBS、B2 等任意标准 S3 协议服务',
    defaultEndpoint: '',
    defaultRegion: 'auto',
    endpointHelp: 'https://<S3-Endpoint>',
  },
];

export const SyncPanel: React.FC = () => {
  const {
    syncSessions,
    selectedSyncAgent,
    isSyncLoading,
    setSelectedSyncAgent,
    fetchSyncConfig,
    saveSyncConfig,
    testSyncConnection,
    fetchSyncSessions,
    uploadSyncSession,
    restoreSyncSession,
    syncAllSessions,
  } = useAppStore();

  const [searchQuery, setSearchQuery] = useState('');
  const [statusFilter, setStatusFilter] = useState<'all' | SyncStatus>('all');
  const [currentPage, setCurrentPage] = useState(1);
  const [pageSize, setPageSize] = useState(20);
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [testingConnection, setTestingConnection] = useState(false);
  const [connectionResult, setConnectionResult] = useState<TestConnectionResult | null>(null);
  const [operatingId, setOperatingId] = useState<string | null>(null);
  const [isBatchSyncing, setIsBatchSyncing] = useState(false);
  const [batchSyncResult, setBatchSyncResult] = useState<string | null>(null);

  useEffect(() => {
    setCurrentPage(1);
  }, [selectedSyncAgent, searchQuery, statusFilter]);

  // Form state for multi-provider S3 settings
  const [formConfig, setFormConfig] = useState<SyncConfig>({
    providerType: 'r2',
    endpoint: '',
    region: 'auto',
    accountId: '',
    bucket: '',
    accessKeyId: '',
    secretAccessKey: '',
  });

  const isConfigured = Boolean(
    formConfig.endpoint.trim() &&
    formConfig.bucket.trim() &&
    formConfig.accessKeyId.trim() &&
    formConfig.secretAccessKey.trim()
  );

  const currentPreset =
    STORAGE_PRESETS.find((p) => p.id === (formConfig.providerType || 'r2')) || STORAGE_PRESETS[0];

  useEffect(() => {
    fetchSyncConfig().then((cfg) => {
      setFormConfig({
        providerType: cfg.providerType || 'r2',
        endpoint: cfg.endpoint || '',
        region: cfg.region || 'auto',
        accountId: cfg.accountId || '',
        bucket: cfg.bucket || '',
        accessKeyId: cfg.accessKeyId || '',
        secretAccessKey: cfg.secretAccessKey || '',
      });
    });
    fetchSyncSessions(selectedSyncAgent);
  }, []);

  const handleTestConnection = async () => {
    if (!isConfigured) {
      setIsSettingsOpen(true);
      return;
    }
    setTestingConnection(true);
    setConnectionResult(null);
    try {
      const res = await testSyncConnection();
      setConnectionResult(res);
    } finally {
      setTestingConnection(false);
    }
  };

  const handleSaveSettings = async (e: React.FormEvent) => {
    e.preventDefault();
    await saveSyncConfig(formConfig);
    setIsSettingsOpen(false);
    if (isConfigured) {
      handleTestConnection();
    }
  };

  const handleClearSettings = async () => {
    const emptyConfig: SyncConfig = {
      providerType: 'r2',
      endpoint: '',
      region: 'auto',
      accountId: '',
      bucket: '',
      accessKeyId: '',
      secretAccessKey: '',
    };
    setFormConfig(emptyConfig);
    await saveSyncConfig(emptyConfig);
    setConnectionResult(null);
    setIsSettingsOpen(false);
  };

  const handleUpload = async (id: string) => {
    if (!isConfigured) {
      setIsSettingsOpen(true);
      return;
    }
    setOperatingId(id);
    try {
      await uploadSyncSession(selectedSyncAgent, id);
    } finally {
      setOperatingId(null);
    }
  };

  const handleRestore = async (id: string) => {
    if (!isConfigured) {
      setIsSettingsOpen(true);
      return;
    }
    setOperatingId(id);
    try {
      await restoreSyncSession(selectedSyncAgent, id);
    } finally {
      setOperatingId(null);
    }
  };

  const handleBatchSync = async () => {
    if (!isConfigured) {
      setIsSettingsOpen(true);
      return;
    }
    setIsBatchSyncing(true);
    setBatchSyncResult(null);
    try {
      const res = await syncAllSessions(selectedSyncAgent);
      setBatchSyncResult(`同步完成：已备份 ${res.uploaded} 个，已恢复 ${res.restored} 个`);
      setTimeout(() => setBatchSyncResult(null), 4000);
    } finally {
      setIsBatchSyncing(false);
    }
  };

  const filteredSessions = syncSessions.filter((s) => {
    if (statusFilter !== 'all' && s.syncStatus !== statusFilter) return false;
    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase();
      return (
        s.title.toLowerCase().includes(q) ||
        s.projectName.toLowerCase().includes(q) ||
        s.preview.toLowerCase().includes(q)
      );
    }
    return true;
  });

  const totalSessions = filteredSessions.length;
  const totalPages = Math.max(1, Math.ceil(totalSessions / pageSize));
  const validCurrentPage = Math.min(Math.max(1, currentPage), totalPages);
  const startIndex = (validCurrentPage - 1) * pageSize;
  const endIndex = Math.min(startIndex + pageSize, totalSessions);
  const paginatedSessions = filteredSessions.slice(startIndex, endIndex);

  const getStatusBadge = (status: SyncStatus) => {
    switch (status) {
      case 'synced':
        return (
          <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium bg-emerald-50 dark:bg-emerald-950/50 text-emerald-600 dark:text-emerald-400 border border-emerald-200/60 dark:border-emerald-800/60 shrink-0">
            <CheckCircle2 className="w-2.5 h-2.5" /> 已同步
          </span>
        );
      case 'local_only':
        return (
          <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium bg-zinc-100 dark:bg-zinc-800 text-zinc-600 dark:text-zinc-400 border border-zinc-200 dark:border-zinc-700 shrink-0">
            <HardDrive className="w-2.5 h-2.5" /> 仅本地
          </span>
        );
      case 'remote_only':
        return (
          <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium bg-sky-50 dark:bg-sky-950/50 text-sky-600 dark:text-sky-400 border border-sky-200/60 dark:border-sky-800/60 shrink-0">
            <Cloud className="w-2.5 h-2.5" /> 仅云端
          </span>
        );
      case 'modified':
        return (
          <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium bg-amber-50 dark:bg-amber-950/50 text-amber-600 dark:text-amber-400 border border-amber-200/60 dark:border-amber-800/60 shrink-0">
            <ArrowUpDown className="w-2.5 h-2.5" /> 待同步
          </span>
        );
    }
  };

  return (
    <div className="w-full pb-10 space-y-3.5 animate-in fade-in duration-150">
      {/* 顶部标题与存储状态横幅 */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 px-5 py-3.5 rounded-lg border border-zinc-200/80 dark:border-zinc-800 bg-white dark:bg-zinc-900 shadow-xs">
        <div className="flex items-center gap-3">
          <div className="w-9 h-9 rounded-md bg-sky-600 dark:bg-cyan-600 text-white flex items-center justify-center shadow-md shadow-cyan-500/25 shrink-0">
            <Cloud className="w-4 h-4" />
          </div>
          <div>
            <h2 className="text-xs sm:text-sm font-bold text-zinc-950 dark:text-white flex items-center gap-2">
              会话历史漫游与云同步
              {isConfigured ? (
                <span className="text-[10px] font-mono font-medium px-1.5 py-0.5 rounded-md bg-cyan-50 dark:bg-cyan-950/60 text-cyan-600 dark:text-cyan-400 border border-cyan-200/60 dark:border-cyan-800/60">
                  {currentPreset.name}
                </span>
              ) : (
                <span className="text-[10px] font-mono font-medium px-1.5 py-0.5 rounded-md bg-amber-50 dark:bg-amber-950/60 text-amber-700 dark:text-amber-300 border border-amber-200/60 dark:border-amber-800/60">
                  未配置云存储
                </span>
              )}
            </h2>
            <p className="text-[11px] text-zinc-500 dark:text-zinc-400 mt-0.5">
              跨设备同步会话流、执行日志与 Artifacts · 支持 S3 / R2 / MinIO / OSS / COS 等存储
            </p>
          </div>
        </div>

        <div className="flex items-center gap-2 shrink-0">
          <button
            type="button"
            onClick={handleTestConnection}
            disabled={testingConnection}
            className="inline-flex items-center gap-1.5 px-2.5 py-1.5 rounded-md text-xs font-medium border border-zinc-200 dark:border-zinc-700 bg-zinc-50 dark:bg-zinc-800/60 text-zinc-700 dark:text-zinc-300 hover:bg-zinc-100 dark:hover:bg-zinc-800 transition-colors cursor-pointer disabled:opacity-50"
          >
            <RefreshCw className={`w-3.5 h-3.5 ${testingConnection ? 'animate-spin text-cyan-600' : ''}`} />
            <span>{testingConnection ? '测试中...' : '测试连接'}</span>
          </button>
          <button
            type="button"
            onClick={() => setIsSettingsOpen(true)}
            className="inline-flex items-center gap-1.5 px-2.5 py-1.5 rounded-md text-xs font-medium border border-zinc-200 dark:border-zinc-700 bg-white dark:bg-zinc-800 text-zinc-700 dark:text-zinc-300 hover:border-zinc-300 dark:hover:border-zinc-600 transition-colors cursor-pointer"
          >
            <Settings2 className="w-3.5 h-3.5 text-zinc-500" />
            <span>存储配置</span>
          </button>
        </div>
      </div>

      {/* 未配置存储提示横幅 */}
      {!isConfigured && !connectionResult && (
        <div className="px-4 py-2.5 rounded-md bg-amber-50/70 dark:bg-amber-950/20 border border-amber-200/70 dark:border-amber-850/40 flex flex-col sm:flex-row sm:items-center justify-between gap-2.5 text-xs text-amber-800 dark:text-amber-300">
          <div className="flex items-center gap-2">
            <AlertCircle className="w-4 h-4 text-amber-500 shrink-0" />
            <span>尚未配置云端对象存储凭据。请在存储配置中绑定您的 R2、S3、MinIO 或阿里云/腾讯云对象存储。</span>
          </div>
          <button
            type="button"
            onClick={() => setIsSettingsOpen(true)}
            className="self-end sm:self-auto px-2.5 py-1 rounded-md text-[11px] font-semibold bg-amber-200/70 hover:bg-amber-200 dark:bg-amber-900/60 dark:hover:bg-amber-900/80 text-amber-900 dark:text-amber-200 transition-colors cursor-pointer shrink-0"
          >
            配置存储
          </button>
        </div>
      )}

      {/* 连通性测试结果提示 */}
      {connectionResult && (
        <div
          role="alert"
          className={`flex items-center justify-between gap-3 px-4 py-2 rounded-md text-xs border ${
            connectionResult.success
              ? 'bg-emerald-50/90 dark:bg-emerald-950/40 border-emerald-200 dark:border-emerald-800/60 text-emerald-700 dark:text-emerald-300'
              : 'bg-red-50/90 dark:bg-red-950/40 border-red-200 dark:border-red-900/60 text-red-700 dark:text-red-300'
          }`}
        >
          <div className="flex items-center gap-2">
            {connectionResult.success ? (
              <CheckCircle2 className="w-3.5 h-3.5 text-emerald-500 shrink-0" />
            ) : (
              <AlertCircle className="w-3.5 h-3.5 text-red-500 shrink-0" />
            )}
            <span className="font-medium break-all text-[11px] sm:text-xs">{connectionResult.message}</span>
          </div>
          <button
            type="button"
            onClick={() => setConnectionResult(null)}
            className="p-1 rounded-md hover:bg-black/5 dark:hover:bg-white/5"
          >
            <X className="w-3.5 h-3.5" />
          </button>
        </div>
      )}

      {/* 批量同步提示 */}
      {batchSyncResult && (
        <div className="px-4 py-2 rounded-md text-xs bg-cyan-50/90 dark:bg-cyan-950/40 border border-cyan-200 dark:border-cyan-800/60 text-cyan-700 dark:text-cyan-300 flex items-center gap-2">
          <CheckCircle2 className="w-3.5 h-3.5 text-cyan-500 shrink-0" />
          <span className="text-[11px] sm:text-xs">{batchSyncResult}</span>
        </div>
      )}

      {/* 会话列表 - 一体化面板（直线直角 + 统一边框 + 内部高密度分割） */}
      <div className="rounded-lg border border-zinc-200/80 dark:border-zinc-800 bg-white dark:bg-zinc-900 overflow-hidden shadow-xs">
        {/* 工具栏：Agent 切换 + 搜索 + 状态筛选 + 全量同步 */}
        <div className="px-4 py-2.5 border-b border-zinc-200/80 dark:border-zinc-800 bg-zinc-50/60 dark:bg-zinc-850/40 flex flex-wrap items-center justify-between gap-2.5">
          {/* Agent 选项卡 */}
          <div className="flex items-center p-0.5 rounded-md bg-zinc-200/60 dark:bg-zinc-800/60 border border-zinc-200/80 dark:border-zinc-700/80 w-fit">
            <button
              type="button"
              onClick={() => setSelectedSyncAgent('antigravity')}
              className={`flex items-center gap-1.5 px-3 py-1.5 rounded-sm text-xs font-semibold transition-all cursor-pointer ${
                selectedSyncAgent === 'antigravity'
                  ? 'bg-white dark:bg-zinc-900 text-zinc-950 dark:text-white shadow-xs'
                  : 'text-zinc-600 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-200'
              }`}
            >
              <Sparkles className="w-3.5 h-3.5 text-cyan-500" />
              <span>Antigravity IDE</span>
            </button>
            <button
              type="button"
              onClick={() => setSelectedSyncAgent('codex')}
              className={`flex items-center gap-1.5 px-3 py-1.5 rounded-sm text-xs font-semibold transition-all cursor-pointer ${
                selectedSyncAgent === 'codex'
                  ? 'bg-white dark:bg-zinc-900 text-zinc-950 dark:text-white shadow-xs'
                  : 'text-zinc-600 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-200'
              }`}
            >
              <Terminal className="w-3.5 h-3.5 text-emerald-500" />
              <span>Codex 桌面端</span>
            </button>
          </div>

          {/* 筛选与一键同步 */}
          <div className="flex items-center gap-2">
            <div className="relative">
              <Search className="w-3.5 h-3.5 absolute left-3 top-1/2 -translate-y-1/2 text-zinc-400" />
              <input
                type="text"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                placeholder="搜索会话、工程名..."
                className="pl-8 pr-3 text-xs rounded-md border border-zinc-200 dark:border-zinc-700 bg-white dark:bg-zinc-800 text-zinc-900 dark:text-zinc-100 placeholder-zinc-400 focus:outline-hidden focus:ring-1 focus:ring-cyan-500 w-40 sm:w-52 h-8"
              />
            </div>

            <Select
              value={statusFilter}
              onChange={(e) => setStatusFilter(e.target.value as any)}
            >
              <option value="all">全部状态</option>
              <option value="local_only">仅本地</option>
              <option value="synced">已同步</option>
              <option value="remote_only">仅云端</option>
              <option value="modified">待同步</option>
            </Select>

            <button
              type="button"
              onClick={handleBatchSync}
              disabled={isBatchSyncing || isSyncLoading}
              className="inline-flex items-center gap-1.5 px-3 h-8 rounded-md text-xs font-semibold bg-sky-600 hover:bg-sky-500 dark:bg-cyan-600 dark:hover:bg-cyan-500 text-white shadow-xs shadow-cyan-600/20 transition-all cursor-pointer disabled:opacity-50 shrink-0"
            >
              <RefreshCw className={`w-3.5 h-3.5 ${isBatchSyncing ? 'animate-spin' : ''}`} />
              <span>{isBatchSyncing ? '同步中...' : '全量同步'}</span>
            </button>
          </div>
        </div>

        {/* 统计子栏 */}
        <div className="px-4 py-2 border-b border-zinc-100 dark:border-zinc-800/80 flex items-center justify-between text-xs text-zinc-500 dark:text-zinc-400 font-medium bg-zinc-50/30 dark:bg-zinc-850/20">
          <div className="flex items-center gap-2">
            <Layers className="w-3.5 h-3.5 text-zinc-400" />
            {isSyncLoading ? (
              <span className="text-zinc-400 dark:text-zinc-500 animate-pulse">
                正在检索会话历史...
              </span>
            ) : (
              <span>
                共检测到 <span className="font-semibold text-zinc-900 dark:text-zinc-100">{filteredSessions.length}</span> 个会话
              </span>
            )}
          </div>
          <button
            type="button"
            onClick={() => fetchSyncSessions(selectedSyncAgent)}
            disabled={isSyncLoading}
            className="inline-flex items-center gap-1 text-xs text-cyan-600 dark:text-cyan-400 hover:underline cursor-pointer disabled:opacity-50"
          >
            <RefreshCw className={`w-3 h-3 ${isSyncLoading ? 'animate-spin' : ''}`} />
            <span>{isSyncLoading ? '扫描中...' : '刷新'}</span>
          </button>
        </div>

        {/* 加载中的顶部渐变光泽扫描线 */}
        {isSyncLoading && (
          <div className="h-[2px] w-full bg-cyan-50 dark:bg-cyan-950/40 overflow-hidden relative">
            <div className="absolute inset-y-0 left-0 w-2/5 bg-gradient-to-r from-transparent via-cyan-600 dark:via-cyan-400 to-transparent animate-shimmer" />
          </div>
        )}

        {isSyncLoading ? (
          <div className="divide-y divide-zinc-100 dark:divide-zinc-800/60 animate-in fade-in duration-200">
            {[62, 45, 75, 52, 68, 38, 58, 65].map((widthPct, idx) => (
              <div
                key={idx}
                className="px-3.5 sm:px-4 py-2 sm:py-2.5 flex items-center justify-between gap-3"
              >
                {/* 左侧：徽章骨架 + 标题骨架 + 工程名骨架 */}
                <div className="flex items-center gap-2 sm:gap-2.5 min-w-0 flex-1">
                  <div className="w-11 h-4 rounded bg-zinc-200/70 dark:bg-zinc-800 animate-pulse shrink-0" />
                  <div
                    className="h-3 rounded bg-zinc-200/90 dark:bg-zinc-800 animate-pulse"
                    style={{ width: `${widthPct}%` }}
                  />
                  <div className="hidden sm:block w-16 h-4 rounded bg-zinc-100 dark:bg-zinc-850/60 animate-pulse shrink-0" />
                </div>

                {/* 右侧：步数骨架 + 时间骨架 + 按钮骨架 */}
                <div className="flex items-center gap-2.5 sm:gap-3 shrink-0">
                  <div className="hidden md:block w-12 h-3 rounded bg-zinc-100 dark:bg-zinc-850/60 animate-pulse" />
                  <div className="w-14 h-3 rounded bg-zinc-200/60 dark:bg-zinc-800/70 animate-pulse" />
                  <div className="w-11 h-6 rounded-md bg-zinc-100 dark:bg-zinc-800/80 animate-pulse" />
                </div>
              </div>
            ))}
          </div>
        ) : filteredSessions.length === 0 ? (
          <div className="py-12 text-center text-xs text-zinc-400 flex flex-col items-center justify-center gap-2">
            <HardDrive className="w-6 h-6 text-zinc-300 dark:text-zinc-700" />
            <span>未发现符合条件的会话记录</span>
          </div>
        ) : (
          <div className="divide-y divide-zinc-100 dark:divide-zinc-800/60 animate-in fade-in duration-200">
            {paginatedSessions.map((session) => {
              const isOperating = operatingId === session.id;
              const formattedDate = session.updatedAt
                ? new Date(session.updatedAt).toLocaleString('zh-CN', {
                    month: '2-digit',
                    day: '2-digit',
                    hour: '2-digit',
                    minute: '2-digit',
                  })
                : '-';

              return (
                <div
                  key={session.id}
                  className="px-3.5 sm:px-4 py-2 sm:py-2.5 hover:bg-zinc-50/80 dark:hover:bg-zinc-800/40 transition-colors flex items-center justify-between gap-3 group"
                >
                  {/* 左侧：状态徽章 + 标题 + 工程名 */}
                  <div className="flex items-center gap-2 sm:gap-2.5 min-w-0 flex-1">
                    {getStatusBadge(session.syncStatus)}

                    <span
                      className="font-medium text-xs text-zinc-900 dark:text-zinc-100 truncate cursor-default"
                      title={session.preview ? `${session.title}\n\n预览: ${session.preview}` : session.title}
                    >
                      {session.title}
                    </span>

                    {session.projectName && (
                      <span
                        className="hidden sm:inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-mono text-zinc-500 dark:text-zinc-400 bg-zinc-100 dark:bg-zinc-800/80 border border-zinc-200/60 dark:border-zinc-700/60 shrink-0 max-w-[130px] truncate"
                        title={`工作目录/项目: ${session.projectName}`}
                      >
                        <FolderGit2 className="w-2.5 h-2.5 text-zinc-400 shrink-0" />
                        <span className="truncate">{session.projectName}</span>
                      </span>
                    )}
                  </div>

                  {/* 右侧：步数/Tokens + 时间 + 操作按钮 */}
                  <div className="flex items-center gap-2.5 sm:gap-3 shrink-0">
                    <div className="hidden md:flex items-center gap-1.5 text-[11px] text-zinc-400 dark:text-zinc-500 tabular-nums">
                      {session.stepCount > 0 && (
                        <span>{session.stepCount} 步</span>
                      )}
                      {session.tokensUsed > 0 && (
                        <>
                          <span className="text-zinc-300 dark:text-zinc-700">·</span>
                          <span>
                            {session.tokensUsed >= 1000
                              ? `${(session.tokensUsed / 1000).toFixed(1)}k`
                              : session.tokensUsed}{' '}
                            tok
                          </span>
                        </>
                      )}
                    </div>

                    <span className="text-[11px] font-mono text-zinc-400 dark:text-zinc-500 tabular-nums whitespace-nowrap">
                      {formattedDate}
                    </span>

                    {/* 操作按钮组 */}
                    <div className="flex items-center gap-1.5 shrink-0 pl-1">
                      {session.syncStatus !== 'remote_only' && (
                        <button
                          type="button"
                          onClick={() => handleUpload(session.id)}
                          disabled={isOperating || isBatchSyncing}
                          className="inline-flex items-center gap-1 h-6 px-2 rounded-md text-[11px] font-medium border border-zinc-200 dark:border-zinc-700/80 bg-white dark:bg-zinc-800 text-zinc-700 dark:text-zinc-300 hover:border-cyan-400 hover:text-cyan-600 dark:hover:text-cyan-400 hover:bg-cyan-50/50 dark:hover:bg-cyan-950/30 transition-colors cursor-pointer disabled:opacity-50"
                          title="将此会话备份打包并上传至 R2"
                        >
                          <CloudUpload className={`w-3 h-3 ${isOperating ? 'animate-pulse text-cyan-500' : 'text-cyan-500'}`} />
                          <span>备份</span>
                        </button>
                      )}

                      {session.syncStatus !== 'local_only' && (
                        <button
                          type="button"
                          onClick={() => handleRestore(session.id)}
                          disabled={isOperating || isBatchSyncing}
                          className="inline-flex items-center gap-1 h-6 px-2 rounded-md text-[11px] font-medium border border-zinc-200 dark:border-zinc-700/80 bg-white dark:bg-zinc-800 text-zinc-700 dark:text-zinc-300 hover:border-emerald-400 hover:text-emerald-600 dark:hover:text-emerald-400 hover:bg-emerald-50/50 dark:hover:bg-emerald-950/30 transition-colors cursor-pointer disabled:opacity-50"
                          title="从云端拉取此会话并解包恢复至本机工作区"
                        >
                          <CloudDownload className={`w-3 h-3 ${isOperating ? 'animate-pulse text-emerald-500' : 'text-emerald-500'}`} />
                          <span>恢复</span>
                        </button>
                      )}
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        )}

        {/* 底部分页控制栏 */}
        {!isSyncLoading && filteredSessions.length > 0 && (
          <div className="px-4 py-2.5 border-t border-zinc-100 dark:border-zinc-800/80 bg-zinc-50/50 dark:bg-zinc-850/30 flex flex-col sm:flex-row sm:items-center justify-between gap-2.5 text-xs text-zinc-500 dark:text-zinc-400">
            <div className="flex items-center gap-3">
              <span>
                显示第 <span className="font-semibold text-zinc-800 dark:text-zinc-200">{totalSessions > 0 ? startIndex + 1 : 0}</span> - <span className="font-semibold text-zinc-800 dark:text-zinc-200">{endIndex}</span> 条，共 <span className="font-semibold text-zinc-800 dark:text-zinc-200">{totalSessions}</span> 条
              </span>
              <div className="flex items-center gap-1.5 pl-2 border-l border-zinc-200 dark:border-zinc-700/60">
                <span className="text-[11px]">每页</span>
                <div className="relative inline-flex items-center">
                  <select
                    value={pageSize}
                    onChange={(e) => {
                      setPageSize(Number(e.target.value));
                      setCurrentPage(1);
                    }}
                    className="appearance-none pl-2 pr-5 py-0.5 rounded-md border border-zinc-200 dark:border-zinc-700 bg-white dark:bg-zinc-800 text-zinc-700 dark:text-zinc-300 text-[11px] focus:outline-hidden cursor-pointer"
                  >
                    <option value={20}>20 条</option>
                    <option value={50}>50 条</option>
                    <option value={100}>100 条</option>
                  </select>
                  <ChevronDown className="w-3 h-3 text-zinc-400 dark:text-zinc-500 pointer-events-none absolute right-1.5 top-1/2 -translate-y-1/2" />
                </div>
              </div>
            </div>

            <div className="flex items-center gap-1 self-end sm:self-center">
              <button
                type="button"
                onClick={() => setCurrentPage(1)}
                disabled={validCurrentPage === 1}
                className="p-1 rounded-md border border-zinc-200 dark:border-zinc-700/80 bg-white dark:bg-zinc-800 text-zinc-600 dark:text-zinc-300 hover:bg-zinc-100 dark:hover:bg-zinc-700 disabled:opacity-40 disabled:cursor-not-allowed cursor-pointer transition-colors"
                title="首页"
              >
                <ChevronsLeft className="w-3.5 h-3.5" />
              </button>
              <button
                type="button"
                onClick={() => setCurrentPage((p) => Math.max(1, p - 1))}
                disabled={validCurrentPage === 1}
                className="p-1 rounded-md border border-zinc-200 dark:border-zinc-700/80 bg-white dark:bg-zinc-800 text-zinc-600 dark:text-zinc-300 hover:bg-zinc-100 dark:hover:bg-zinc-700 disabled:opacity-40 disabled:cursor-not-allowed cursor-pointer transition-colors"
                title="上一页"
              >
                <ChevronLeft className="w-3.5 h-3.5" />
              </button>

              <span className="px-2 py-0.5 text-[11px] font-medium text-zinc-700 dark:text-zinc-300 font-mono">
                {validCurrentPage} / {totalPages}
              </span>

              <button
                type="button"
                onClick={() => setCurrentPage((p) => Math.min(totalPages, p + 1))}
                disabled={validCurrentPage >= totalPages}
                className="p-1 rounded-md border border-zinc-200 dark:border-zinc-700/80 bg-white dark:bg-zinc-800 text-zinc-600 dark:text-zinc-300 hover:bg-zinc-100 dark:hover:bg-zinc-700 disabled:opacity-40 disabled:cursor-not-allowed cursor-pointer transition-colors"
                title="下一页"
              >
                <ChevronRight className="w-3.5 h-3.5" />
              </button>
              <button
                type="button"
                onClick={() => setCurrentPage(totalPages)}
                disabled={validCurrentPage >= totalPages}
                className="p-1 rounded-md border border-zinc-200 dark:border-zinc-700/80 bg-white dark:bg-zinc-800 text-zinc-600 dark:text-zinc-300 hover:bg-zinc-100 dark:hover:bg-zinc-700 disabled:opacity-40 disabled:cursor-not-allowed cursor-pointer transition-colors"
                title="末页"
              >
                <ChevronsRight className="w-3.5 h-3.5" />
              </button>
            </div>
          </div>
        )}
      </div>

      {/* 多存储服务商 (S3 兼容) 配置模态弹窗 */}
      {isSettingsOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/40 backdrop-blur-xs animate-in fade-in duration-150">
          <div className="bg-white dark:bg-zinc-900 border border-zinc-200 dark:border-zinc-800 rounded-lg w-full max-w-lg shadow-xl overflow-hidden animate-in zoom-in-95 duration-150 max-h-[90vh] flex flex-col">
            <div className="px-6 py-4 border-b border-zinc-100 dark:border-zinc-800 flex items-center justify-between shrink-0">
              <div className="flex items-center gap-2">
                <Cloud className="w-4 h-4 text-sky-600 dark:text-cyan-400" />
                <h3 className="text-sm font-bold text-zinc-900 dark:text-zinc-100">
                  对象存储 (S3 兼容) 云同步配置
                </h3>
              </div>
              <button
                type="button"
                onClick={() => setIsSettingsOpen(false)}
                className="p-1 rounded-md text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-200 cursor-pointer"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            <form onSubmit={handleSaveSettings} className="p-6 space-y-4 text-xs overflow-y-auto flex-1">
              {/* 存储服务提供商预设切换 */}
              <div>
                <label className="block font-medium text-zinc-700 dark:text-zinc-300 mb-1.5">
                  存储服务类型
                </label>
                <div className="grid grid-cols-2 sm:grid-cols-3 gap-1.5">
                  {STORAGE_PRESETS.map((preset) => {
                    const active = (formConfig.providerType || 'r2') === preset.id;
                    return (
                      <button
                        key={preset.id}
                        type="button"
                        onClick={() => {
                          const updated = { ...formConfig, providerType: preset.id };
                          if (preset.id === 'r2') {
                            updated.region = 'auto';
                            if (formConfig.accountId.trim()) {
                              updated.endpoint = `https://${formConfig.accountId.trim()}.r2.cloudflarestorage.com`;
                            }
                          } else if (preset.id === 's3') {
                            updated.region = formConfig.region || 'us-east-1';
                            if (!formConfig.endpoint || formConfig.endpoint.includes('cloudflarestorage') || formConfig.endpoint.includes('aliyuncs') || formConfig.endpoint.includes('myqcloud')) {
                              updated.endpoint = `https://s3.${updated.region}.amazonaws.com`;
                            }
                          } else if (preset.id === 'oss') {
                            updated.region = formConfig.region || 'oss-cn-hangzhou';
                            if (!formConfig.endpoint || formConfig.endpoint.includes('cloudflarestorage') || formConfig.endpoint.includes('amazonaws') || formConfig.endpoint.includes('myqcloud')) {
                              updated.endpoint = `https://${updated.region}.aliyuncs.com`;
                            }
                          } else if (preset.id === 'cos') {
                            updated.region = formConfig.region || 'ap-guangzhou';
                            if (!formConfig.endpoint || formConfig.endpoint.includes('cloudflarestorage') || formConfig.endpoint.includes('amazonaws') || formConfig.endpoint.includes('aliyuncs')) {
                              updated.endpoint = `https://cos.${updated.region}.myqcloud.com`;
                            }
                          } else if (preset.id === 'minio') {
                            updated.region = 'auto';
                            if (!formConfig.endpoint || formConfig.endpoint.includes('cloudflarestorage') || formConfig.endpoint.includes('amazonaws') || formConfig.endpoint.includes('aliyuncs') || formConfig.endpoint.includes('myqcloud')) {
                              updated.endpoint = 'http://127.0.0.1:9000';
                            }
                          }
                          setFormConfig(updated);
                        }}
                        className={`px-2.5 py-1.5 rounded-md border text-left text-xs transition-all cursor-pointer ${
                          active
                            ? 'border-cyan-500 bg-cyan-50/50 dark:bg-cyan-950/40 text-cyan-700 dark:text-cyan-300 font-semibold shadow-2xs'
                            : 'border-zinc-200 dark:border-zinc-700 bg-white dark:bg-zinc-800 text-zinc-600 dark:text-zinc-400 hover:border-zinc-300 dark:hover:border-zinc-600'
                        }`}
                      >
                        <div className="font-medium truncate">{preset.name}</div>
                      </button>
                    );
                  })}
                </div>
                <div className="text-[11px] text-zinc-400 dark:text-zinc-500 mt-1.5 leading-relaxed">
                  {currentPreset.desc}
                </div>
              </div>

              {/* R2 专属账户 ID */}
              {formConfig.providerType === 'r2' && (
                <div>
                  <label className="block font-medium text-zinc-700 dark:text-zinc-300 mb-1">
                    Cloudflare 账户 ID (Account ID)
                  </label>
                  <input
                    type="text"
                    value={formConfig.accountId}
                    onChange={(e) => {
                      const val = e.target.value.trim();
                      setFormConfig({
                        ...formConfig,
                        accountId: val,
                        endpoint: val ? `https://${val}.r2.cloudflarestorage.com` : formConfig.endpoint,
                      });
                    }}
                    placeholder="例如：6b33bec9ce7dc2c0e9df738ccaab1cff"
                    className="w-full px-3 py-2 rounded-md border border-zinc-200 dark:border-zinc-700 bg-zinc-50 dark:bg-zinc-800/60 text-zinc-900 dark:text-zinc-100 font-mono text-[11px] focus:outline-hidden focus:ring-1 focus:ring-cyan-500"
                  />
                  <div className="text-[10px] text-zinc-400 mt-1">
                    输入后将自动补全下方 S3 Endpoint 端点地址
                  </div>
                </div>
              )}

              {/* AWS / 阿里云 / 腾讯云 区域 */}
              {(formConfig.providerType === 's3' || formConfig.providerType === 'oss' || formConfig.providerType === 'cos') && (
                <div>
                  <label className="block font-medium text-zinc-700 dark:text-zinc-300 mb-1">
                    所属区域 (Region)
                  </label>
                  <input
                    type="text"
                    value={formConfig.region || ''}
                    onChange={(e) => {
                      const reg = e.target.value.trim();
                      let newEndpoint = formConfig.endpoint;
                      if (formConfig.providerType === 's3') newEndpoint = `https://s3.${reg || 'us-east-1'}.amazonaws.com`;
                      if (formConfig.providerType === 'oss') newEndpoint = `https://${reg || 'oss-cn-hangzhou'}.aliyuncs.com`;
                      if (formConfig.providerType === 'cos') newEndpoint = `https://cos.${reg || 'ap-guangzhou'}.myqcloud.com`;
                      setFormConfig({
                        ...formConfig,
                        region: reg,
                        endpoint: newEndpoint,
                      });
                    }}
                    placeholder={
                      formConfig.providerType === 's3'
                        ? 'us-east-1'
                        : formConfig.providerType === 'oss'
                          ? 'oss-cn-hangzhou'
                          : 'ap-guangzhou'
                    }
                    className="w-full px-3 py-2 rounded-md border border-zinc-200 dark:border-zinc-700 bg-zinc-50 dark:bg-zinc-800/60 text-zinc-900 dark:text-zinc-100 font-mono text-[11px] focus:outline-hidden focus:ring-1 focus:ring-cyan-500"
                  />
                </div>
              )}

              {/* S3 Endpoint 端点 */}
              <div>
                <label className="block font-medium text-zinc-700 dark:text-zinc-300 mb-1">
                  S3 API 端点地址 (Endpoint)
                </label>
                <input
                  type="text"
                  required
                  value={formConfig.endpoint}
                  onChange={(e) => setFormConfig({ ...formConfig, endpoint: e.target.value.trim() })}
                  placeholder={currentPreset.endpointHelp}
                  className="w-full px-3 py-2 rounded-md border border-zinc-200 dark:border-zinc-700 bg-zinc-50 dark:bg-zinc-800/60 text-zinc-900 dark:text-zinc-100 font-mono text-[11px] focus:outline-hidden focus:ring-1 focus:ring-cyan-500"
                />
              </div>

              {/* 存储桶 Bucket */}
              <div>
                <label className="block font-medium text-zinc-700 dark:text-zinc-300 mb-1">
                  存储桶名称 (Bucket)
                </label>
                <input
                  type="text"
                  required
                  value={formConfig.bucket}
                  onChange={(e) => setFormConfig({ ...formConfig, bucket: e.target.value.trim() })}
                  placeholder="例如：my-omni-backup"
                  className="w-full px-3 py-2 rounded-md border border-zinc-200 dark:border-zinc-700 bg-zinc-50 dark:bg-zinc-800/60 text-zinc-900 dark:text-zinc-100 font-mono text-[11px] focus:outline-hidden focus:ring-1 focus:ring-cyan-500"
                />
              </div>

              {/* Access Key ID */}
              <div>
                <label className="block font-medium text-zinc-700 dark:text-zinc-300 mb-1">
                  {formConfig.providerType === 'cos' ? 'SecretId (访问密钥 ID)' : 'Access Key ID'}
                </label>
                <input
                  type="text"
                  required
                  value={formConfig.accessKeyId}
                  onChange={(e) => setFormConfig({ ...formConfig, accessKeyId: e.target.value.trim() })}
                  placeholder="输入对象存储访问密钥"
                  className="w-full px-3 py-2 rounded-md border border-zinc-200 dark:border-zinc-700 bg-zinc-50 dark:bg-zinc-800/60 text-zinc-900 dark:text-zinc-100 font-mono text-[11px] focus:outline-hidden focus:ring-1 focus:ring-cyan-500"
                />
              </div>

              {/* Secret Access Key */}
              <div>
                <label className="block font-medium text-zinc-700 dark:text-zinc-300 mb-1">
                  {formConfig.providerType === 'cos' ? 'SecretKey (私有访问密钥)' : 'Secret Access Key'}
                </label>
                <input
                  type="password"
                  required
                  value={formConfig.secretAccessKey}
                  onChange={(e) => setFormConfig({ ...formConfig, secretAccessKey: e.target.value.trim() })}
                  placeholder="输入对象存储私有安全密钥"
                  className="w-full px-3 py-2 rounded-md border border-zinc-200 dark:border-zinc-700 bg-zinc-50 dark:bg-zinc-800/60 text-zinc-900 dark:text-zinc-100 font-mono text-[11px] focus:outline-hidden focus:ring-1 focus:ring-cyan-500"
                />
              </div>

              <div className="pt-3 flex items-center justify-between gap-2.5 border-t border-zinc-100 dark:border-zinc-800">
                {isConfigured ? (
                  <button
                    type="button"
                    onClick={handleClearSettings}
                    className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-md border border-rose-200 dark:border-rose-900/60 text-rose-600 dark:text-rose-400 hover:bg-rose-50 dark:hover:bg-rose-950/40 text-xs transition-colors cursor-pointer"
                  >
                    <Trash2 className="w-3.5 h-3.5" />
                    <span>清空凭证</span>
                  </button>
                ) : (
                  <div />
                )}

                <div className="flex items-center gap-2">
                  <button
                    type="button"
                    onClick={() => setIsSettingsOpen(false)}
                    className="px-4 py-2 rounded-md border border-zinc-200 dark:border-zinc-700 text-zinc-600 dark:text-zinc-400 hover:bg-zinc-100 dark:hover:bg-zinc-800 transition-colors cursor-pointer"
                  >
                    取消
                  </button>
                  <button
                    type="submit"
                    className="inline-flex items-center gap-1.5 px-4 py-2 rounded-md bg-sky-600 hover:bg-sky-500 dark:bg-cyan-600 dark:hover:bg-cyan-500 text-white font-medium shadow-xs shadow-cyan-600/20 transition-all cursor-pointer"
                  >
                    <Check className="w-3.5 h-3.5" />
                    <span>保存配置</span>
                  </button>
                </div>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
};
