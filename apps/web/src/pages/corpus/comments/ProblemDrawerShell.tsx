import { useEffect, useRef, type ReactNode } from 'react';
import { StudyDialog } from './StudyDialog';
import { c } from './styles';

export function ProblemDrawerShell({ children, feedback, copy, copying, close, dialogRef }: { children: ReactNode; feedback: ReactNode; copy?: () => void; copying?: boolean; close: () => void; dialogRef: React.RefObject<HTMLDialogElement | null> }) {
  const fallback = useRef<HTMLDialogElement>(null);
  const ref = dialogRef ?? fallback;
  useEffect(() => {
    const measure = () => {
      const header = parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--lgi-header-height')) || 0;
      const top = window.innerWidth > 680 ? Math.max(header, document.querySelector('[data-comment-study-toolbar]')?.getBoundingClientRect().bottom ?? document.querySelector('[class*="study-toolbar"]')?.getBoundingClientRect().bottom ?? 0) : 0;
      ref.current?.style.setProperty('--sr3-dialog-top', `${top}px`);
    };
    measure(); window.addEventListener('resize', measure, { passive: true });
    return () => window.removeEventListener('resize', measure);
  }, [ref]);
  return <StudyDialog className={c('sr3-drawer')} titleId="sr3-drawer-title" onClose={close} dialogRef={ref} portal={false} dismissBackdrop><header className={c('sr3-drawer-bar')}><span>用户问题 / 研究详情</span><button type="button" className={c('sr3-button')} data-sr3="close" onClick={close}>关闭 ×</button></header><div className={c('sr3-drawer-scroll')} data-sr3-detail>{children}</div><footer className={c('sr3-drawer-footer')}><span role="status" data-sr3-feedback>{feedback}</span>{copy ? <button type="button" className={c('sr3-button')} data-sr3="copy" disabled={copying} onClick={copy}>复制摘要与来源</button> : null}</footer></StudyDialog>;
}
