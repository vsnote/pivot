import React from 'react';
import { ChevronDown } from 'lucide-react';

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
    <div className={`relative inline-flex items-center ${containerClassName}`}>
      <select
        className={`appearance-none bg-white dark:bg-[#1a1b1e] border border-gray-200/90 dark:border-gray-800 text-xs text-gray-800 dark:text-gray-200 rounded-lg pl-3 pr-8 py-1.5 h-8 font-medium hover:border-gray-300 dark:hover:border-gray-700 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-500 shadow-[0_1px_2px_rgba(0,0,0,0.02)] cursor-pointer transition-colors w-full ${className}`}
        {...props}
      >
        {children}
      </select>
      <ChevronDown className="w-3.5 h-3.5 text-gray-400 dark:text-gray-500 pointer-events-none absolute right-2.5 top-1/2 -translate-y-1/2 shrink-0" />
    </div>
  );
};
