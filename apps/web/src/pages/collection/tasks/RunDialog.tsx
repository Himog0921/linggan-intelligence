import { useEffect, useRef, type ReactNode } from 'react';

export function RunDialog({ id, className = '', labelledBy, onClose, children }: { id?: string; className?: string; labelledBy: string; onClose: () => void; children: ReactNode }) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const dialog = ref.current;
    if (!dialog) return;
    const previous = document.activeElement;
    dialog.showModal(); document.documentElement.classList.add('c-run-dialog-open');
    return () => {
      dialog.close();
      if (!document.querySelector('dialog[open]')) document.documentElement.classList.remove('c-run-dialog-open');
      if (previous instanceof HTMLElement && previous.isConnected) previous.focus();
    };
  }, []);
  return <dialog id={id} ref={ref} className={`c-run-dialog ${className}`} aria-labelledby={labelledBy} onCancel={event => { event.preventDefault(); onClose(); }}>{children}</dialog>;
}
