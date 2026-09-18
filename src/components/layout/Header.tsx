import React, { useRef, useState, useEffect } from 'react';
import { isTauri } from '@tauri-apps/api/core';
import { useAppStore } from '@/store/useAppStore';
import { SettingsSection } from '@/types';
import { Moon, Sun, Plus } from 'lucide-react';

export const Header: React.FC = () => {
  const {
    activeTab,
    setActiveTab,
    settingsSection,
    setSettingsSection,
    theme,
    setTheme,
    openProviderEditor,
  } = useAppStore();

  const isSettings = activeTab === 'settings';

  const navRef = useRef<HTMLElement>(null);
  const [indicatorStyle, setIndicatorStyle] = useState<{ left: number; width: number; opacity: number }>({
    left: 0,
    width: 0,
    opacity: 0,
  });

  const settingsTabs: { id: SettingsSection; label: string }[] = [
    { id: 'clients', label: '客户端' },
    { id: 'routes', label: '高级设置' },
    { id: 'gateway', label: '监控' },
    { id: 'logs', label: '日志' },
    { id: 'about', label: '关于' },
  ];

  // 动态测量设置二级 Tab 项，驱动 Apple 物理级 Spring 滑块平滑无级跟随
  useEffect(() => {
    const updateIndicator = () => {
      if (!isSettings || !navRef.current) {
        setIndicatorStyle((prev) => ({ ...prev, opacity: 0 }));
        return;
      }
      const activeEl = navRef.current.querySelector<HTMLButtonElement>(`[data-tab-id="${settingsSection}"]`);
      if (activeEl) {
        setIndicatorStyle({
          left: activeEl.offsetLeft,
          width: activeEl.offsetWidth,
          opacity: 1,
        });
      }
    };

    updateIndicator();
    const raf = requestAnimationFrame(updateIndicator);
    window.addEventListener('resize', updateIndicator);
    return () => {
      cancelAnimationFrame(raf);
      window.removeEventListener('resize', updateIndicator);
    };
  }, [isSettings, settingsSection]);

  const isNative = isTauri() || window.location.protocol === 'tauri:';

  return (
    <header
      data-tauri-drag-region
      className="relative h-11 w-full px-6 sm:px-8 select-none shrink-0 z-30 bg-transparent"
      style={{ WebkitAppRegion: 'drag' } as React.CSSProperties}
    >
      {/* 左翼：为原生 macOS 红黄绿交通灯预留舒适呼吸空间与拖拽区域 */}
      <div
        data-tauri-drag-region
        className="flex items-center pl-18 sm:pl-20 shrink-0 h-full"
      />

      {/* 中央：当处于 Web 调试环境时显示 Web 灵动岛；原生环境下由 AppKit NSSegmentedControl 接管 */}
      {!isNative && (
        <div
          className="absolute left-1/2 -translate-x-1/2 top-1/2 -translate-y-1/2 flex items-center justify-center transition-all duration-300 ease-[cubic-bezier(0.16,1,0.3,1)]"
          style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
        >
        {/* 灵动岛主体：全圆角胶囊形态，彻底杜绝耳片接缝与漏白 */}
        <div
          className={`relative h-[32px] bg-white/90 dark:bg-zinc-900/90 text-gray-900 dark:text-white rounded-full shadow-[0_2px_10px_rgba(0,0,0,0.04),0_1px_2px_rgba(0,0,0,0.02)] dark:shadow-[0_4px_20px_rgba(0,0,0,0.6),inset_0_1px_0_rgba(255,255,255,0.08)] border border-gray-200/80 dark:border-white/10 flex items-center backdrop-blur-xl transition-all duration-300 ease-[cubic-bezier(0.16,1,0.3,1)] ${
            isSettings ? 'pl-2.5 pr-1.5 gap-1.5' : 'px-3.5 gap-2'
          }`}
        >
          {/* Logo 区域：首页为状态胶囊，设置页为可点击返回按钮 */}
          {isSettings ? (
            <button
              onClick={() => setActiveTab('providers')}
              className="flex items-center gap-1.5 select-none shrink-0 whitespace-nowrap rounded-full px-2 py-1 text-gray-600 dark:text-zinc-400 hover:text-gray-900 dark:hover:text-white hover:bg-black/[0.05] dark:hover:bg-white/[0.08] transition-all duration-150 cursor-pointer"
              title="返回服务商"
            >
              <div className="relative flex items-center justify-center">
                <div className="w-2.5 h-2.5 rounded-full bg-emerald-500/20 flex items-center justify-center">
                  <div className="w-1.5 h-1.5 rounded-full bg-emerald-500" />
                </div>
              </div>
              <span className="text-[11.5px] font-semibold tracking-tight font-sans">
                OmniSwitch
              </span>
            </button>
          ) : (
            <div className="flex items-center gap-2 select-none shrink-0 whitespace-nowrap cursor-default">
              <div className="relative flex items-center justify-center">
                <div className="w-3.5 h-3.5 rounded-full border-[1.5px] border-emerald-500/80 dark:border-emerald-400 bg-emerald-500/10 dark:bg-emerald-400/10 flex items-center justify-center shadow-[0_0_8px_rgba(16,185,129,0.25)]">
                  <div className="w-1.5 h-1.5 rounded-full bg-emerald-500 dark:bg-emerald-400 animate-pulse" />
                </div>
              </div>
              <span className="text-[12px] font-semibold tracking-tight font-sans text-gray-800 dark:text-zinc-100">
                OmniSwitch
              </span>
            </div>
          )}

          {/* 设置模式下展开的细微分割线与二级 Tab 导航 */}
          {isSettings && (
            <>
              {/* 细腻微分割线 */}
              <div className="w-[1px] h-3 bg-gray-200 dark:bg-white/15 shrink-0" />

              {/* 二级 Tab 导航 */}
              <nav
                ref={navRef}
                className="relative flex items-center gap-0.5 h-6 shrink-0 animate-in fade-in duration-200"
              >
                {/* 动态平滑滑动的流体胶囊 (Apple Fluid Spring Indicator) */}
                <div
                  className="absolute left-0 top-0 bottom-0 rounded-full pointer-events-none transition-all duration-260 ease-[cubic-bezier(0.16,1,0.3,1)] will-change-[transform,width] bg-black/[0.06] dark:bg-white/[0.16] shadow-[0_1px_2px_rgba(0,0,0,0.04)] dark:shadow-[inset_0_1px_0_rgba(255,255,255,0.35),0_2px_8px_rgba(0,0,0,0.4)]"
                  style={{
                    transform: `translateX(${indicatorStyle.left}px)`,
                    width: `${indicatorStyle.width}px`,
                    opacity: indicatorStyle.opacity,
                  }}
                />

                {settingsTabs.map((tab) => {
                  const isActive = settingsSection === tab.id;
                  return (
                    <button
                      key={tab.id}
                      data-tab-id={tab.id}
                      onClick={() => setSettingsSection(tab.id)}
                      className={`relative z-10 px-2.5 h-6 text-[11px] flex items-center justify-center rounded-full transition-colors duration-150 cursor-pointer active:scale-95 whitespace-nowrap shrink-0 ${
                        isActive
                          ? 'text-gray-950 dark:text-white font-semibold'
                          : 'text-gray-500 dark:text-white/60 hover:text-gray-900 dark:hover:text-white font-medium'
                      }`}
                    >
                      {tab.label}
                    </button>
                  );
                })}
              </nav>
            </>
          )}
        </div>
      </div>
      )}

      {/* Web 调试环境保留同等功能，原生窗口交由 NSToolbar 渲染 */}
      {!isNative && (
        <div
          className="absolute right-6 sm:right-8 top-1/2 -translate-y-1/2 flex h-8 items-center gap-0.5 rounded-full border border-gray-200/80 bg-white/90 p-0.5 shadow-[0_2px_10px_rgba(0,0,0,0.04),0_1px_2px_rgba(0,0,0,0.02)] backdrop-blur-xl dark:border-white/10 dark:bg-zinc-900/90 dark:shadow-[0_4px_20px_rgba(0,0,0,0.6),inset_0_1px_0_rgba(255,255,255,0.08)]"
          style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
        >
          <button
            onClick={() => openProviderEditor(null)}
            aria-label="添加服务商"
            title="添加服务商"
            className="inline-flex size-7 items-center justify-center rounded-full bg-indigo-600 text-white shadow-xs shadow-indigo-600/20 transition-[background-color,transform,box-shadow] duration-150 hover:bg-indigo-700 active:scale-[0.96] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 focus-visible:ring-offset-2 focus-visible:ring-offset-white cursor-pointer dark:focus-visible:ring-indigo-400 dark:focus-visible:ring-offset-zinc-900"
          >
            <Plus className="size-3.5" />
          </button>

          <button className="inline-flex size-7 items-center justify-center rounded-full" aria-label="切换外观" title="切换外观" onClick={() => setTheme(document.documentElement.classList.contains('dark') ? 'light' : 'dark')}>
            {theme === 'dark' ? <Sun size={17} /> : <Moon size={17} />}
          </button>
        </div>
      )}
    </header>
  );
};
