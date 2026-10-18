import { createPortal } from 'react-dom';
import { useEffect, useRef, type ReactNode, type RefObject } from 'react';

/** Native dialog lifecycle and focus stay tied to the owning React component. */
export function StudyDialog({ id, className, titleId, describedBy, children, onClose, open = true, dialogRef, portal = true, dismissBackdrop = false }: { id?: string; className?: string; titleId: string; describedBy?: string; children: ReactNode; onClose: () => void; open?: boolean; dialogRef?: RefObject<HTMLDialogElement | null>; portal?: boolean; dismissBackdrop?: boolean }) {
  const ownRef = useRef<HTMLDialogElement>(null);
  const ref = dialogRef ?? ownRef;
  useEffect(() => {
    const dialog = ref.current!;
    const opener = document.activeElement;
    if (open) dialog.showModal(); else dialog.close();
    return () => { dialog.close(); if (opener instanceof HTMLElement && opener.isConnected) opener.focus({ preventScroll: true }); };
  }, [open, ref]);
  const dialog = <dialog id={id} ref={ref} className={className} aria-labelledby={titleId} aria-describedby={describedBy} onCancel={event => { event.preventDefault(); onClose(); }} onClick={event => { if (!dismissBackdrop || event.target !== event.currentTarget) return; const rect = event.currentTarget.getBoundingClientRect(); if (event.clientX < rect.left || event.clientX > rect.right || event.clientY < rect.top || event.clientY > rect.bottom) onClose(); }}>{children}</dialog>;
  return portal ? createPortal(dialog, document.body) : dialog;
}
