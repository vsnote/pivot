import * as React from 'react';
import * as SwitchPrimitive from '@radix-ui/react-switch';
import { cn } from '@/lib/utils';

export interface SwitchProps {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  ariaLabel: string;
  disabled?: boolean;
  size?: 'sm' | 'md';
  className?: string;
  id?: string;
}

export const Switch: React.FC<SwitchProps> = ({
  checked,
  onCheckedChange,
  ariaLabel,
  disabled = false,
  size = 'sm',
  className = '',
  id,
}) => {
  const isSm = size === 'sm';
  const trackSize = isSm ? 'h-5 w-9' : 'h-6 w-11';
  const thumbSize = isSm ? 'h-4 w-4' : 'h-5 w-5';
  const translate = isSm
    ? 'data-[state=checked]:translate-x-4'
    : 'data-[state=checked]:translate-x-5';

  return (
    <SwitchPrimitive.Root
      id={id}
      checked={checked}
      onCheckedChange={onCheckedChange}
      disabled={disabled}
      aria-label={ariaLabel}
      className={cn(
        'peer inline-flex shrink-0 cursor-pointer items-center rounded-full border-2 border-transparent transition-colors duration-150',
        'focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring focus-visible:ring-offset-1 focus-visible:ring-offset-background',
        'disabled:cursor-not-allowed disabled:opacity-40',
        'data-[state=checked]:bg-primary data-[state=unchecked]:bg-input shadow-2xs',
        trackSize,
        className
      )}
    >
      <SwitchPrimitive.Thumb
        className={cn(
          'pointer-events-none block rounded-full bg-background shadow-xs ring-0 transition-transform duration-150 data-[state=unchecked]:translate-x-0',
          translate,
          thumbSize
        )}
      />
    </SwitchPrimitive.Root>
  );
};
