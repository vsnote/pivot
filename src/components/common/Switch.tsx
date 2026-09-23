import React from 'react';

interface SwitchProps {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  ariaLabel: string;
  disabled?: boolean;
  size?: 'sm' | 'md';
  className?: string;
}

export const Switch: React.FC<SwitchProps> = ({
  checked,
  onCheckedChange,
  ariaLabel,
  disabled = false,
  size = 'sm',
  className = '',
}) => {
  const trackSize = size === 'md' ? 'h-6 w-10' : 'h-5 w-9';
  const thumbSize = size === 'md' ? 'h-5 w-5' : 'h-4 w-4';

  return (
    <label
      className={`relative inline-flex shrink-0 select-none ${
        disabled ? 'cursor-not-allowed opacity-50' : 'cursor-pointer'
      } ${className}`}
    >
      <input
        type="checkbox"
        role="switch"
        aria-label={ariaLabel}
        aria-checked={checked}
        checked={checked}
        disabled={disabled}
        onChange={(event) => onCheckedChange(event.target.checked)}
        className="peer sr-only"
      />
      <span
        className={`${trackSize} rounded-full bg-zinc-200 p-0.5 transition-colors peer-checked:bg-sky-600 dark:peer-checked:bg-cyan-500 peer-focus-visible:ring-2 peer-focus-visible:ring-cyan-500/40 peer-focus-visible:ring-offset-2 dark:bg-zinc-700 dark:peer-focus-visible:ring-offset-zinc-900`}
      >
        <span
          className={`block ${thumbSize} rounded-full border border-black/5 bg-white shadow-sm transition-transform ${
            checked ? 'translate-x-4' : 'translate-x-0'
          }`}
        />
      </span>
    </label>
  );
};
