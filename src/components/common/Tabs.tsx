import * as React from 'react';
import * as TabsPrimitive from '@radix-ui/react-tabs';
import { cva, type VariantProps } from 'class-variance-authority';
import { cn } from '@/lib/utils';

export const Tabs = TabsPrimitive.Root;

type TabsVariant = 'default' | 'ghost';
const TabsVariantContext = React.createContext<TabsVariant>('default');

const tabsListVariants = cva(
  'inline-flex items-center justify-center select-none text-muted-foreground',
  {
    variants: {
      variant: {
        // 表单内分段控件（如设置页）：无边界软底槽，靠内阴影而非描边呈现凹陷感
        default:
          'h-7.5 rounded-md bg-zinc-500/[0.06] dark:bg-white/[0.06] p-0.5 shadow-[inset_0_1px_2px_rgba(0,0,0,0.055)] dark:shadow-[inset_0_1px_2px_rgba(0,0,0,0.3)]',
        // 顶栏无底槽（如页面大导航）：通透无底框，幽灵浮动卡片
        ghost: 'h-8 bg-transparent p-0 gap-1',
      },
    },
    defaultVariants: {
      variant: 'default',
    },
  }
);

export interface TabsListProps
  extends React.ComponentPropsWithoutRef<typeof TabsPrimitive.List>,
    VariantProps<typeof tabsListVariants> {}

export const TabsList = React.forwardRef<
  React.ElementRef<typeof TabsPrimitive.List>,
  TabsListProps
>(({ className, variant = 'default', ...props }, ref) => (
  <TabsVariantContext.Provider value={variant || 'default'}>
    <TabsPrimitive.List
      ref={ref}
      className={cn(tabsListVariants({ variant }), className)}
      {...props}
    />
  </TabsVariantContext.Provider>
));
TabsList.displayName = TabsPrimitive.List.displayName;

export const TabsTrigger = React.forwardRef<
  React.ElementRef<typeof TabsPrimitive.Trigger>,
  React.ComponentPropsWithoutRef<typeof TabsPrimitive.Trigger>
>(({ className, ...props }, ref) => {
  const variant = React.useContext(TabsVariantContext);

  const isGhost = variant === 'ghost';

  return (
    <TabsPrimitive.Trigger
      ref={ref}
      className={cn(
        'inline-flex items-center justify-center whitespace-nowrap text-xs transition-all duration-150 cursor-pointer select-none',
        'outline-none focus:outline-none focus-visible:outline-none active:outline-none',
        'ring-0 focus:ring-0 focus-visible:ring-0 active:ring-0',
        'disabled:pointer-events-none disabled:opacity-50',
        isGhost
          ? // 顶栏幽灵风格：带透明占位边框，激活时带微边框浮动
            'h-7 rounded-md px-3 font-medium border border-transparent hover:text-foreground hover:bg-muted/60 data-[state=active]:bg-background data-[state=active]:text-foreground data-[state=active]:border-border/80 data-[state=active]:shadow-2xs font-semibold'
          : // 槽内分段控件风格：槽内纯净卡片，深浅模式下选中项都必须是“凸起”而非凹陷
            'h-6.5 rounded-[5px] px-3 font-medium hover:text-foreground data-[state=active]:bg-background dark:data-[state=active]:bg-zinc-700 data-[state=active]:text-foreground data-[state=active]:shadow-2xs',
        className
      )}
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
