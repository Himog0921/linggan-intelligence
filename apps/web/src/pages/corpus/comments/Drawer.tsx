import type { ReactNode } from 'react';
import { ActionButton } from '../../../shared/ActionButton';
import { CrIcon } from './CrIcon';
import { StudyDialog } from './StudyDialog';
import { c } from './styles';

/** Right-hand inspector (B-04 `.inspector`): modal dialog below the shared header, ink rail on the left. */
export function Drawer({ id, title, closeLabel, close, back, note, actions, children }: {
  id: string; title: string; closeLabel: string; close: () => void; back: (() => void) | null; note: ReactNode; actions: ReactNode; children: ReactNode;
}) {
  return <StudyDialog id={id} className={c('dialog inspector')} titleId={`${id}-title`} onClose={close} dismissBackdrop>
    <header className={c('dialog-head')}><div><h2 id={`${id}-title`} tabIndex={-1}>{title}</h2></div>
      <button type="button" className={c('icon-btn')} aria-label={closeLabel} onClick={close}><CrIcon name="X" /></button></header>
    <div className={c('dialog-body')} aria-live="polite">{children}</div>
    <footer className={c('dialog-footer')}>
      {back ? <ActionButton variant="quiet" className={c('quiet-small')} onClick={back}>返回上一项</ActionButton> : <span className={c('footer-note')}>{note}</span>}
      <div className={c('actions')}>{actions}</div>
    </footer>
  </StudyDialog>;
}
