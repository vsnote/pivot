import React from 'react';

export type PivotLogoState = 'active' | 'idle' | 'fault';
export type AnyRelayLogoState = PivotLogoState;

export interface PivotLogoProps {
  className?: string;
  state?: PivotLogoState;
  showStatusDot?: boolean;
  /**
   * - 'mark': 纯透明无底板品牌符号 (In-App Brandmark)，适用于导航栏、Header、工具栏；
   * - 'icon': macOS Squircle 规范应用图标 (App Icon)，带圆角底板与环境阴影，适用于桌面、关于面板。
   */
  variant?: 'mark' | 'icon' | 'compact' | 'detailed';
  monochrome?: boolean;
  alt?: string;
}
export type AnyRelayLogoProps = PivotLogoProps;

/**
 * Pivot 专属官方 Logo 组件
 * 规范化分离「应用内纯透明品牌标 (Brandmark)」与「系统级 Squircle 应用图标 (App Icon)」
 */
export const PivotLogo: React.FC<PivotLogoProps> = ({
  className = 'w-5 h-5',
  state = 'active',
  showStatusDot = false,
  variant = 'mark',
  alt = 'Pivot Logo',
}) => {
  const filterClass =
    state === 'fault'
      ? 'opacity-85'
      : state === 'idle'
        ? 'opacity-50'
        : 'opacity-100';

  const statusDotColor =
    state === 'fault'
      ? 'bg-rose-500 ring-rose-500/20'
      : state === 'idle'
        ? 'bg-zinc-400 ring-zinc-400/20'
        : 'bg-emerald-500 ring-emerald-500/20';

  // 1. 系统级应用图标模式: 标准 macOS 824/1024 比例圆角底板与投影
  // 浅色模式为白底黑图案，深色模式为黑底白图案
  if (variant === 'icon') {
    return (
      <div className={`relative inline-flex items-center justify-center select-none shrink-0 pointer-events-none ${className}`}>
        {/* 浅色模式: 白底，图案是黑色 */}
        <img
          src="/app-icon-light.png"
          alt={alt}
          className={`w-full h-full object-contain dark:hidden ${filterClass}`}
          draggable={false}
        />
        {/* 深色模式: 黑底，图案白色 */}
        <img
          src="/app-icon-dark.png"
          alt={alt}
          className={`w-full h-full object-contain hidden dark:block ${filterClass}`}
          draggable={false}
        />
        {showStatusDot && (
          <span
            className={`absolute -bottom-0.5 -right-0.5 w-1.5 h-1.5 rounded-full ring-2 ring-background ${statusDotColor}`}
            aria-hidden="true"
          />
        )}
      </div>
    );
  }

  // 2. 默认应用内品牌标模式: 纯透明背景
  return (
    <div className={`relative inline-flex items-center justify-center select-none shrink-0 pointer-events-none ${className}`}>
      {/* 浅色模式 */}
      <img
        src="/brand-mark-light.png"
        alt={alt}
        className={`w-full h-full object-contain dark:hidden ${filterClass}`}
        draggable={false}
      />
      {/* 深色模式 */}
      <img
        src="/brand-mark-dark.png"
        alt={alt}
        className={`w-full h-full object-contain hidden dark:block ${filterClass}`}
        draggable={false}
      />
      {showStatusDot && (
        <span
          className={`absolute -bottom-0.5 -right-0.5 w-1.5 h-1.5 rounded-full ring-2 ring-background ${statusDotColor}`}
          aria-hidden="true"
        />
      )}
    </div>
  );
};

export const AnyRelayLogo = PivotLogo;
