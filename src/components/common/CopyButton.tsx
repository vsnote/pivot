import { useEffect, useState } from 'react';
import { Check, Copy } from 'lucide-react';

interface CopyButtonProps {
  text: string;
  label: string;
}

export function CopyButton({ text, label }: CopyButtonProps) {
  const [status, setStatus] = useState<'idle' | 'copied' | 'error'>('idle');
  const [isCopying, setIsCopying] = useState(false);

  useEffect(() => {
    if (status === 'idle' || isCopying) return;
    const timer = setTimeout(() => setStatus('idle'), 2200);
    return () => clearTimeout(timer);
  }, [status, isCopying]);

  const copy = async () => {
    setIsCopying(true);
    try {
      await navigator.clipboard.writeText(text);
      setStatus('copied');
    } catch {
      setStatus('error');
    } finally {
      setIsCopying(false);
    }
  };

  return (
    <span className="inline-flex shrink-0 items-center gap-1">
      <button type="button" className="icon-button" onClick={copy} disabled={isCopying} aria-label={label} title={status === 'copied' ? '已复制' : label}>
        {status === 'copied' ? <Check size={14} /> : <Copy size={14} />}
      </button>
      <span role="status" className={status === 'error' ? 'text-[10px] text-[var(--danger)]' : 'sr-only'}>
        {status === 'copied' ? '已复制' : status === 'error' ? '复制失败，请手动复制' : ''}
      </span>
    </span>
  );
}
