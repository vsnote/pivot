import { create } from 'zustand';
import { check, type Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { openUrl } from '@tauri-apps/plugin-opener';

export type UpdateState =
  | 'idle'
  | 'checking'
  | 'available'
  | 'up-to-date'
  | 'downloading'
  | 'ready'
  | 'error';

export const GITHUB_RELEASE_URL = 'https://github.com/vsnote/pivot/releases/latest';

interface UpdateStore {
  updateState: UpdateState;
  updateInstance: Update | null;
  downloadProgress: number;
  updateError: string;
  updateVersion: string;
  updateBody: string;
  hasUpdate: boolean;
  lastCheckedTime: number | null;

  checkUpdate: (manual?: boolean) => Promise<void>;
  downloadAndInstall: () => Promise<void>;
  relaunch: () => Promise<void>;
  openReleasePage: () => Promise<void>;
  clearError: () => void;
  dismiss: () => void;
}

const formatUpdateError = (err: unknown): string => {
  const msg = err instanceof Error ? err.message : String(err);
  const lower = msg.toLowerCase();

  if (lower.includes('could not fetch a valid release json') || lower.includes('404')) {
    return '获取更新信息失败（未找到发布版本或网络无法访问）';
  }
  if (lower.includes('timed out') || lower.includes('timeout') || lower.includes('504')) {
    return '连接 GitHub 更新服务器超时，建议检查网络代理或手动下载 DMG';
  }
  if (lower.includes('signature')) {
    return '升级包签名校验失败，建议前往 GitHub 手动下载最新 DMG';
  }
  if (lower.includes('failed to send request') || lower.includes('network')) {
    return '网络请求失败，请检查网络连接或代理设置';
  }
  return msg || '检查更新失败，请稍后重试或手动下载安装包';
};

export const useUpdateStore = create<UpdateStore>((set, get) => ({
  updateState: 'idle',
  updateInstance: null,
  downloadProgress: 0,
  updateError: '',
  updateVersion: '',
  updateBody: '',
  hasUpdate: false,
  lastCheckedTime: null,

  checkUpdate: async (manual = true) => {
    // 正在下载或准备重启时不重复检查
    const current = get().updateState;
    if (current === 'downloading' || current === 'ready') return;

    try {
      set({
        updateState: 'checking',
        updateError: '',
      });

      const update = await check();
      const now = Date.now();

      if (update && update.available) {
        set({
          updateState: 'available',
          updateInstance: update,
          updateVersion: update.version ?? '',
          updateBody: update.body ?? '',
          hasUpdate: true,
          lastCheckedTime: now,
        });
      } else {
        set({
          updateState: 'up-to-date',
          updateInstance: null,
          hasUpdate: false,
          lastCheckedTime: now,
        });

        // 3.5秒后将 up-to-date 自动切回 idle
        setTimeout(() => {
          if (get().updateState === 'up-to-date') {
            set({ updateState: 'idle' });
          }
        }, 3500);
      }
    } catch (err: unknown) {
      console.warn('检查更新失败:', err);
      if (manual) {
        set({
          updateState: 'error',
          updateError: formatUpdateError(err),
        });
      } else {
        // 静默检查失败不向用户抛出错误弹层，保持 idle
        set({
          updateState: 'idle',
        });
      }
    }
  },

  downloadAndInstall: async () => {
    const { updateInstance } = get();
    if (!updateInstance) return;

    try {
      set({
        updateState: 'downloading',
        downloadProgress: 0,
        updateError: '',
      });

      let downloaded = 0;
      let total = 0;

      await updateInstance.downloadAndInstall((event) => {
        if (event.event === 'Started') {
          total = event.data.contentLength || 0;
        } else if (event.event === 'Progress') {
          downloaded += event.data.chunkLength;
          if (total > 0) {
            set({ downloadProgress: Math.min(100, Math.round((downloaded / total) * 100)) });
          }
        } else if (event.event === 'Finished') {
          set({ downloadProgress: 100 });
        }
      });

      set({ updateState: 'ready' });
    } catch (err: unknown) {
      console.error('下载或安装更新失败:', err);
      set({
        updateState: 'error',
        updateError: formatUpdateError(err),
      });
    }
  },

  relaunch: async () => {
    try {
      await relaunch();
    } catch (err) {
      console.error('重启应用失败:', err);
    }
  },

  openReleasePage: async () => {
    try {
      await openUrl(GITHUB_RELEASE_URL);
    } catch {
      if (typeof window !== 'undefined') {
        window.open(GITHUB_RELEASE_URL, '_blank');
      }
    }
  },

  clearError: () => {
    set({
      updateState: get().hasUpdate ? 'available' : 'idle',
      updateError: '',
    });
  },

  dismiss: () => {
    set({
      updateState: 'idle',
      updateError: '',
    });
  },
}));
