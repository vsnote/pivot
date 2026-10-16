import React from 'react';
import * as DialogPrimitive from '@radix-ui/react-dialog';
import { AlertTriangle } from 'lucide-react';
import { Button } from '@/components/common/Button';

interface ConfirmDialogProps {
  open: boolean;
  title: string;
  details: string[];
  confirmLabel: string;
  variant?: 'danger' | 'warning' | 'primary';
  busy?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

export const ConfirmDialog: React.FC<ConfirmDialogProps> = ({
  open,
  title,
  details,
  confirmLabel,
  variant = 'danger',
  busy = false,
  onConfirm,
  onCancel,
}) => {
  return (
    <DialogPrimitive.Root open={open} onOpenChange={(isOpen) => !isOpen && onCancel()}>
      <DialogPrimitive.Portal>
        <DialogPrimitive.Overlay className="fixed inset-0 z-50 bg-black/50 backdrop-blur-sm animate-in fade-in-0 duration-150" />
        <DialogPrimitive.Content
          aria-describedby={details.length > 0 ? 'confirm-dialog-desc' : undefined}
          className="fixed left-1/2 top-1/2 z-50 w-full max-w-md -translate-x-1/2 -translate-y-1/2 rounded-lg border border-zinc-200 bg-white shadow-2xl animate-in fade-in-0 zoom-in-95 duration-150 dark:border-zinc-800 dark:bg-zinc-900 focus:outline-none"
        >
          <div className="flex items-start gap-3 border-b border-zinc-200 px-6 py-4 dark:border-zinc-800">
            <AlertTriangle className="mt-0.5 h-5 w-5 shrink-0 text-amber-500" />
            <DialogPrimitive.Title className="text-sm font-semibold text-zinc-900 dark:text-zinc-100">
              {title}
            </DialogPrimitive.Title>
          </div>

          {details.length > 0 && (
            <div id="confirm-dialog-desc" className="space-y-1.5 px-6 py-4">
              {details.map((line) => (
                <p key={line} className="text-xs leading-relaxed text-zinc-600 dark:text-zinc-400">
                  {line}
                </p>
              ))}
            </div>
          )}

          <div className="flex items-center justify-end gap-2 border-t border-zinc-200 px-6 py-4 dark:border-zinc-800">
            <Button variant="outline" size="default" disabled={busy} onClick={onCancel}>
              取消
            </Button>
            <Button
              variant={variant === 'primary' ? 'default' : 'destructive'}
              size="default"
              autoFocus
              disabled={busy}
              onClick={onConfirm}
            >
              {busy ? '处理中…' : confirmLabel}
            </Button>
          </div>
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  );
};
