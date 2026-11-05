import React, { useState, useRef, useEffect, useMemo } from 'react';
import { Popover, PopoverTrigger, PopoverContent } from '@/components/common/Popover';
import { useTranslation } from '@/i18n';
import { cn } from '@/lib/utils';
import { Check, ChevronDown, Search, X, Sparkles } from 'lucide-react';

export interface GroupedModel {
  id: string;
  name: string;
  models: string[];
}

interface ClientModelSelectProps {
  value: string;
  disabled?: boolean;
  ariaLabel?: string;
  title?: string;
  placeholder?: string;
  groupedModels: GroupedModel[];
  onValueChange: (next: string) => void;
}

export const ClientModelSelect: React.FC<ClientModelSelectProps> = ({
  value,
  disabled = false,
  ariaLabel,
  title,
  placeholder,
  groupedModels,
  onValueChange,
}) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState('');
  const inputRef = useRef<HTMLInputElement>(null);

  // 浮层展开时自动聚焦搜索输入框
  useEffect(() => {
    if (open) {
      const timer = setTimeout(() => {
        inputRef.current?.focus();
      }, 40);
      return () => clearTimeout(timer);
    } else {
      setSearch('');
    }
  }, [open]);

  const isModelInPool = useMemo(() => {
    return Boolean(value && groupedModels.some((g) => g.models.includes(value)));
  }, [value, groupedModels]);

  const query = search.trim().toLowerCase();

  // 根据搜索关键字过滤模型分组
  const filteredGroups = useMemo(() => {
    if (!query) return groupedModels;
    return groupedModels
      .map((g) => ({
        ...g,
        models: g.models.filter((m) => m.toLowerCase().includes(query)),
      }))
      .filter((g) => g.models.length > 0);
  }, [groupedModels, query]);

  const hasUnlistedMatch = Boolean(
    value && !isModelInPool && (!query || value.toLowerCase().includes(query)),
  );

  const totalMatchCount =
    filteredGroups.reduce((acc, g) => acc + g.models.length, 0) + (hasUnlistedMatch ? 1 : 0);

  const handleSelect = (model: string) => {
    onValueChange(model);
    setOpen(false);
    setSearch('');
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      // 如果刚好只有1个匹配，直接回车选中
      if (hasUnlistedMatch && totalMatchCount === 1) {
        handleSelect(value);
      } else if (filteredGroups.length === 1 && filteredGroups[0].models.length === 1) {
        handleSelect(filteredGroups[0].models[0]);
      } else if (totalMatchCount === 0 && search.trim()) {
        // 无匹配时直接将输入内容作为自定义模型选中
        handleSelect(search.trim());
      }
    }
  };

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          disabled={disabled}
          aria-label={ariaLabel}
          title={title}
          className={cn(
            'flex h-7 w-fit min-w-[5.5rem] max-w-[11.5rem] sm:max-w-[18rem] items-center justify-between rounded-md border border-input bg-background px-2 py-0.5 text-[11px] font-mono text-foreground shadow-2xs transition-colors',
            'hover:bg-accent/50 focus:outline-none focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring',
            'disabled:cursor-not-allowed disabled:opacity-50 cursor-pointer select-none',
            open && 'bg-accent/60'
          )}
        >
          <span className="truncate text-left min-w-0 flex-1 mr-1.5">
            {value || placeholder}
          </span>
          <ChevronDown
            className={cn(
              'h-3.5 w-3.5 text-muted-foreground/70 shrink-0 transition-transform duration-150',
              open && 'rotate-180'
            )}
          />
        </button>
      </PopoverTrigger>

      <PopoverContent
        align="end"
        sideOffset={4}
        className="w-72 p-1.5 shadow-xl font-mono text-xs"
        onCloseAutoFocus={(e) => {
          e.preventDefault();
        }}
      >
        {/* 顶部搜索输入框 */}
        <div className="relative mb-1 flex items-center px-1">
          <Search className="w-3.5 h-3.5 absolute left-3 text-muted-foreground pointer-events-none" />
          <input
            ref={inputRef}
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={t('clients.searchModel')}
            className="w-full h-7 pl-7 pr-7 rounded-md bg-zinc-500/[0.06] dark:bg-white/[0.06] border border-border/60 text-xs placeholder:text-muted-foreground/70 focus:outline-none focus:ring-1 focus:ring-ring/40 focus:border-ring/50 transition-colors font-sans"
          />
          {search && (
            <button
              type="button"
              onClick={() => setSearch('')}
              className="absolute right-2.5 text-muted-foreground hover:text-foreground p-0.5 cursor-pointer rounded"
              title="Clear"
            >
              <X className="w-3 h-3" />
            </button>
          )}
        </div>

        {/* 模型滚动列表 */}
        <div className="max-h-64 overflow-y-auto divide-y divide-border/40 py-0.5 text-[11px]">
          {/* 未在池中的当前自定义模型 */}
          {hasUnlistedMatch && (
            <div className="py-1">
              <button
                type="button"
                onClick={() => handleSelect(value)}
                className={cn(
                  'relative flex w-full cursor-pointer select-none items-center rounded-md py-1.5 pl-7 pr-2 text-left outline-none transition-colors font-mono',
                  'hover:bg-accent hover:text-accent-foreground',
                  'bg-accent/40 font-semibold text-foreground'
                )}
              >
                <span className="absolute left-2 flex h-3.5 w-3.5 items-center justify-center">
                  <Check className="h-3.5 w-3.5 text-primary stroke-[2.5]" />
                </span>
                <span className="truncate">{value}</span>
              </button>
            </div>
          )}

          {/* 各服务商模型分组 */}
          {filteredGroups.map((group) => (
            <div key={group.id} className="py-1">
              <div className="px-2 pt-1 pb-0.5 text-[10px] font-sans font-medium uppercase tracking-wider text-muted-foreground/80">
                {group.name}
              </div>
              <div className="space-y-0.5">
                {group.models.map((model) => {
                  const isSelected = model === value;
                  return (
                    <button
                      type="button"
                      key={`${group.id}:${model}`}
                      onClick={() => handleSelect(model)}
                      className={cn(
                        'relative flex w-full cursor-pointer select-none items-center rounded-md py-1.5 pl-7 pr-2 text-left outline-none transition-colors font-mono',
                        'hover:bg-accent hover:text-accent-foreground',
                        isSelected
                          ? 'bg-accent/40 font-semibold text-foreground'
                          : 'text-zinc-700 dark:text-zinc-300'
                      )}
                    >
                      <span className="absolute left-2 flex h-3.5 w-3.5 items-center justify-center">
                        {isSelected && (
                          <Check className="h-3.5 w-3.5 text-primary stroke-[2.5]" />
                        )}
                      </span>
                      <span className="truncate">{model}</span>
                    </button>
                  );
                })}
              </div>
            </div>
          ))}

          {/* 无匹配提示及使用自定义模型操作 */}
          {totalMatchCount === 0 && (
            <div className="p-3 text-center">
              <p className="text-xs text-muted-foreground">{t('clients.noMatchingModels')}</p>
              {search.trim() && (
                <button
                  type="button"
                  onClick={() => handleSelect(search.trim())}
                  className="w-full mt-2 px-2.5 py-1.5 rounded-md text-left text-xs bg-primary/10 hover:bg-primary/20 text-primary font-mono flex items-center gap-1.5 transition-colors cursor-pointer"
                >
                  <Sparkles className="w-3.5 h-3.5 shrink-0" />
                  <span className="truncate">
                    {t('clients.useCustomModel', { name: search.trim() })}
                  </span>
                </button>
              )}
            </div>
          )}
        </div>
      </PopoverContent>
    </Popover>
  );
};
