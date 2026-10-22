import { useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import type { CSSProperties } from 'react';
import { Search } from 'lucide-react';
import { ProviderAvatar } from '@/components/common/BrandIcons';
import type { Provider } from '@/types';

export interface ModelPickerTarget {
  provider: Provider;
  model: string;
}

interface ModelPickerPopoverProps {
  anchor: DOMRect | null;
  isOpen: boolean;
  onClose: () => void;
  options: ModelPickerTarget[];
  onSelect: (target: ModelPickerTarget) => void;
  onClear?: () => void;
  clearLabel?: string;
  placeholder?: string;
}

const PANEL_MAX_HEIGHT = 320;

export function ModelPickerPopover({
  anchor,
  isOpen,
  onClose,
  options,
  onSelect,
  onClear,
  clearLabel = '未绑定',
  placeholder = '搜索模型或渠道...',
}: ModelPickerPopoverProps) {
  const [query, setQuery] = useState('');
  const panelRef = useRef<HTMLDivElement>(null);

  // 当弹窗打开时重置搜索
  useEffect(() => {
    if (isOpen) {
      setQuery('');
    }
  }, [isOpen]);

  // 监听 Esc 键与外部滚动
  useEffect(() => {
    if (!isOpen) return;

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose();
    };

    const onViewportChange = (event: Event) => {
      if (event.target instanceof Node && panelRef.current?.contains(event.target)) return;
      onClose();
    };

    window.addEventListener('keydown', onKeyDown);
    window.addEventListener('resize', onViewportChange);
    window.addEventListener('scroll', onViewportChange, true);
    return () => {
      window.removeEventListener('keydown', onKeyDown);
      window.removeEventListener('resize', onViewportChange);
      window.removeEventListener('scroll', onViewportChange, true);
    };
  }, [isOpen, onClose]);

  // 计算过滤后的列表
  const filteredOptions = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return options;
    return options.filter(
      (opt) =>
        opt.model.toLowerCase().includes(q) || opt.provider.name.toLowerCase().includes(q),
    );
  }, [options, query]);

  // 动态位置计算
  const panelStyle = useMemo((): CSSProperties | null => {
    if (!anchor || !isOpen) return null;

    const spaceBelow = window.innerHeight - anchor.bottom - 12;
    const spaceAbove = anchor.top - 12;
    const placeAbove = spaceBelow < 200 && spaceAbove > spaceBelow;

    const width = Math.min(Math.max(anchor.width, 320), window.innerWidth - 16);
    const left = Math.max(8, Math.min(anchor.left, window.innerWidth - width - 8));
    const maxHeight = Math.max(160, Math.min(PANEL_MAX_HEIGHT, placeAbove ? spaceAbove : spaceBelow));

    return {
      left,
      width,
      maxHeight,
      ...(placeAbove
        ? { bottom: window.innerHeight - anchor.top + 4 }
        : { top: anchor.bottom + 4 }),
    };
  }, [anchor, isOpen]);

  if (!isOpen || !anchor || !panelStyle) return null;

  return createPortal(
    <>
      <div className="fixed inset-0 z-40" onPointerDown={onClose} />
      <div
        ref={panelRef}
        role="listbox"
        style={panelStyle}
        className="fixed z-50 flex flex-col overflow-hidden rounded-lg border border-border bg-popover text-popover-foreground shadow-xl animate-in fade-in zoom-in-95 duration-100"
      >
        <div className="flex shrink-0 items-center gap-1.5 border-b border-border px-3 py-2 bg-muted/20">
          <Search className="w-3.5 h-3.5 shrink-0 text-muted-foreground" />
          <input
            autoFocus
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={placeholder}
            className="w-full bg-transparent text-xs text-foreground outline-none placeholder:text-muted-foreground/60"
          />
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto p-1.5 space-y-0.5">
          {onClear && (
            <button
              type="button"
              onClick={() => {
                onClose();
                onClear();
              }}
              className="flex h-7 w-full items-center rounded-[5px] px-2 text-[11px] text-muted-foreground hover:bg-accent hover:text-foreground transition-colors cursor-pointer"
            >
              {clearLabel}
            </button>
          )}

          {filteredOptions.map((option, idx) => {
            const firstOfProvider =
              idx === 0 || filteredOptions[idx - 1].provider.id !== option.provider.id;

            return (
              <div key={`${option.provider.id}-${option.model}`}>
                {firstOfProvider && (
                  <div className="flex items-center gap-1.5 px-2 pb-1 pt-2 text-[10px] font-semibold text-muted-foreground uppercase tracking-wider">
                    <ProviderAvatar name={option.provider.name} sizeClass="w-3.5 h-3.5" dense />
                    <span>{option.provider.name}</span>
                  </div>
                )}
                <button
                  type="button"
                  role="option"
                  onClick={() => {
                    onClose();
                    onSelect(option);
                  }}
                  className="flex h-7 w-full items-center justify-between gap-2 rounded-[5px] px-2 text-left transition-colors hover:bg-accent cursor-pointer group"
                >
                  <span className="min-w-0 truncate font-mono text-[11px] text-zinc-800 dark:text-zinc-200 group-hover:text-foreground">
                    {option.model}
                  </span>
                  <span className="text-[10px] text-muted-foreground/60 font-sans shrink-0">
                    {option.provider.name}
                  </span>
                </button>
              </div>
            );
          })}

          {filteredOptions.length === 0 && (
            <p className="px-2 py-6 text-center text-[11px] text-muted-foreground">
              未找到匹配的模型或渠道
            </p>
          )}
        </div>
      </div>
    </>,
    document.body,
  );
}
