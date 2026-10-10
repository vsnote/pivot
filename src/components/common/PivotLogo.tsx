import React from 'react';

export type PivotLogoState = 'active' | 'idle' | 'fault';
export type AnyRelayLogoState = PivotLogoState;

export interface PivotLogoProps {
  className?: string;
  state?: PivotLogoState;
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
  variant = 'mark',
  alt = 'Pivot Logo',
}) => {
  const filterClass =
    state === 'fault'
      ? 'hue-rotate-180 brightness-110'
      : state === 'idle'
        ? 'grayscale opacity-75'
        : '';

  // 1. 系统级应用图标模式: 带有标准 macOS Squircle 磨砂底板与柔和环境阴影
  if (variant === 'icon') {
    return (
      <img
        src="/app-icon.png"
        alt={alt}
        className={`object-contain select-none shrink-0 pointer-events-none drop-shadow-md ${filterClass} ${className}`}
        draggable={false}
      />
    );
  }

  // 2. 默认应用内品牌标模式: 纯透明背景，绝无方块底板，纯净优雅呼应文字
  return (
    <div className={`relative inline-flex items-center justify-center select-none shrink-0 pointer-events-none ${className}`}>
      {/* 浅色模式: 纯透明深钴蓝至电气青立体微光 */}
      <img
        src="/brand-mark-light.png"
        alt={alt}
        className={`w-full h-full object-contain dark:hidden ${filterClass}`}
        draggable={false}
      />
      {/* 深色模式: 纯透明晶莹发光白青霓虹 */}
      <img
        src="/brand-mark-dark.png"
        alt={alt}
        className={`w-full h-full object-contain hidden dark:block drop-shadow-[0_0_8px_rgba(6,182,212,0.45)] ${filterClass}`}
        draggable={false}
      />
    </div>
  );
};

export const AnyRelayLogo = PivotLogo;
