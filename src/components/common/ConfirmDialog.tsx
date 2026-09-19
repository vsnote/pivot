import React from 'react';
import { AlertTriangle } from 'lucide-react';
import { useDialogAccessibility } from './useDialogAccessibility';

interface ConfirmDialogProps {
  open: boolean;
  title: string;
  /** Lines that spell out what the action removes. */
  details: string[];
  confirmLabel: string;
  variant?: 'danger' | 'warning' | 'primary';
  busy?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

/**
 * In-app confirmation. WebKit only calls through `window.confirm` when the host
 * implements the confirm panel; this app's webview does not, so a confirm dialog
 * has to live in the page or the action silently does nothing.
 */
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
  const dialogRef = useDialogAccessibility(open, onCancel);
  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm p-4 pt-14">
      <div
        ref={dialogRef}
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="confirm-dialog-title"
        className="bg-white dark:bg-zinc-900 rounded-2xl border border-zinc-200 dark:border-zinc-800 shadow-2xl w-full max-w-md"
      >
        <div className="flex items-start gap-3 px-6 py-4 border-b border-zinc-200 dark:border-zinc-800">
          <AlertTriangle className="w-5 h-5 text-amber-500 shrink-0 mt-0.5" />
          <h2 id="confirm-dialog-title" className="text-sm font-semibold text-zinc-900 dark:text-zinc-100">
            {title}
          </h2>
        </div>
        <div className="px-6 py-4 space-y-1.5">
          {details.map((line) => (
            <p key={line} className="text-xs leading-relaxed text-zinc-600 dark:text-zinc-400">
              {line}
            </p>
          ))}
        </div>
        <div className="flex items-center justify-end gap-2 px-6 py-4 border-t border-zinc-200 dark:border-zinc-800">
          <button
            type="button"
            className="px-3 py-1.5 text-xs font-medium rounded-lg border border-zinc-200 dark:border-zinc-700 text-zinc-700 dark:text-zinc-200 hover:bg-zinc-50 dark:hover:bg-zinc-800 disabled:opacity-50"
            disabled={busy}
            onClick={onCancel}
          >
            取消
          </button>
          <button
            type="button"
            data-autofocus
            className={`px-3 py-1.5 text-xs font-medium rounded-lg text-white disabled:opacity-50 transition-colors ${
              variant === 'warning'
                ? 'bg-amber-600 hover:bg-amber-700'
                : variant === 'primary'
                  ? 'bg-indigo-600 hover:bg-indigo-700'
                  : 'bg-red-600 hover:bg-red-700'
            }`}
            disabled={busy}
            onClick={onConfirm}
          >
            {busy ? '处理中…' : confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
};
