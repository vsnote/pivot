import React from 'react';
import { ChevronDown } from 'lucide-react';
import { cn } from '@/lib/utils';

export interface SelectProps extends React.SelectHTMLAttributes<HTMLSelectElement> {
  containerClassName?: string;
}

export const Select: React.FC<SelectProps> = ({
  className = '',
  containerClassName = '',
  children,
  ...props
}) => {
  return (
    <div className={cn('relative inline-flex items-center w-full', containerClassName)}>
      <select
        className={cn(
          'flex h-8 w-full appearance-none rounded-md border border-input bg-background pl-2.5 pr-8 py-1 text-xs text-foreground shadow-2xs transition-colors cursor-pointer font-medium',
          'hover:bg-accent/50',
          'focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring focus-visible:border-ring',
          'disabled:cursor-not-allowed disabled:opacity-50',
          className
        )}
        {...props}
      >
        {children}
      </select>
      <ChevronDown className="w-3.5 h-3.5 text-muted-foreground pointer-events-none absolute right-2.5 top-1/2 -translate-y-1/2 shrink-0" />
    </div>
  );
};
