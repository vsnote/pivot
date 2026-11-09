import * as React from 'react';
import * as TabsPrimitive from '@radix-ui/react-tabs';
import { cva, type VariantProps } from 'class-variance-authority';
import { cn } from '@/lib/utils';

export const Tabs = TabsPrimitive.Root;

type TabsVariant = 'default' | 'ghost';
type TabsSize = 'default' | 'sm' | 'xs';

interface TabsContextValue {
  variant: TabsVariant;
  size: TabsSize;
}

const TabsContext = React.createContext<TabsContextValue>({
  variant: 'default',
  size: 'default',
});

const tabsListVariants = cva(
  'inline-flex items-center justify-center select-none text-muted-foreground',
  {
    variants: {
      variant: {
        // 表单内分段控件（如设置页）：精致内嵌底槽，带微边框与内凹感
        default:
          'rounded-md bg-zinc-500/[0.08] dark:bg-white/[0.08] p-0.5 border border-border/40 shadow-[inset_0_1px_1px_rgba(0,0,0,0.04)] dark:shadow-[inset_0_1px_2px_rgba(0,0,0,0.2)]',
        // 顶栏无底槽（如页面大导航）：通透无底框，幽灵浮动卡片
        ghost: 'bg-transparent p-0 gap-1',
      },
      size: {
        default: 'h-7.5',
        sm: 'h-6.5',
        xs: 'h-5.5',
      },
    },
    compoundVariants: [
      {
        variant: 'ghost',
        size: 'default',
        className: 'h-8',
      },
    ],
    defaultVariants: {
      variant: 'default',
      size: 'default',
    },
  }
);

export interface TabsListProps
  extends React.ComponentPropsWithoutRef<typeof TabsPrimitive.List>,
    VariantProps<typeof tabsListVariants> {}

export const TabsList = React.forwardRef<
  React.ElementRef<typeof TabsPrimitive.List>,
  TabsListProps
>(({ className, variant = 'default', size = 'default', ...props }, ref) => (
  <TabsContext.Provider
    value={{
      variant: variant || 'default',
      size: size || 'default',
    }}
  >
    <TabsPrimitive.List
      ref={ref}
      className={cn(tabsListVariants({ variant, size }), className)}
      {...props}
    />
  </TabsContext.Provider>
));
TabsList.displayName = TabsPrimitive.List.displayName;

const tabsTriggerVariants = cva(
  'inline-flex items-center justify-center whitespace-nowrap transition-[color,background-color,border-color,box-shadow] duration-150 cursor-pointer select-none outline-none focus:outline-none focus-visible:outline-none active:outline-none ring-0 focus:ring-0 focus-visible:ring-0 active:ring-0 disabled:pointer-events-none disabled:opacity-50 border border-transparent',
  {
    variants: {
      variant: {
        ghost:
          'hover:text-foreground hover:bg-muted/60 data-[state=active]:bg-background data-[state=active]:text-foreground data-[state=active]:border-border/80 data-[state=active]:shadow-2xs',
        default:
          'text-zinc-600 dark:text-zinc-400 hover:text-foreground data-[state=active]:bg-white dark:data-[state=active]:bg-zinc-700 data-[state=active]:text-zinc-900 dark:data-[state=active]:text-zinc-100 data-[state=active]:border-black/5 dark:data-[state=active]:border-white/10 data-[state=active]:shadow-xs',
      },
      size: {
        default: 'h-6.5 rounded-[5px] px-3 font-medium text-xs',
        sm: 'h-5.5 rounded px-2 font-medium text-[11px]',
        xs: 'h-4.5 rounded px-1.5 font-medium text-[10px]',
      },
    },
    compoundVariants: [
      {
        variant: 'ghost',
        size: 'default',
        className: 'h-7 rounded-md px-2.5 font-medium text-xs',
      },
    ],
    defaultVariants: {
      variant: 'default',
      size: 'default',
    },
  }
);

export const TabsTrigger = React.forwardRef<
  React.ElementRef<typeof TabsPrimitive.Trigger>,
  React.ComponentPropsWithoutRef<typeof TabsPrimitive.Trigger>
>(({ className, ...props }, ref) => {
  const { variant, size } = React.useContext(TabsContext);

  return (
    <TabsPrimitive.Trigger
      ref={ref}
      className={cn(tabsTriggerVariants({ variant, size }), className)}
      {...props}
    />
  );
});
TabsTrigger.displayName = TabsPrimitive.Trigger.displayName;

export const TabsContent = React.forwardRef<
  React.ElementRef<typeof TabsPrimitive.Content>,
  React.ComponentPropsWithoutRef<typeof TabsPrimitive.Content>
>(({ className, ...props }, ref) => (
  <TabsPrimitive.Content
    ref={ref}
    className={cn(
      'outline-none focus:outline-none focus-visible:outline-none',
      className
    )}
    {...props}
  />
));
TabsContent.displayName = TabsPrimitive.Content.displayName;
