import { useEffect, useId, useState } from 'react';
import { useBlocker } from 'react-router';
import { ModalDialog } from '../../../corpus/ModalDialog';
import { ActionButton } from '../../../shared/ActionButton';
import styles from './CreatorDialogs.module.css';

/** S1 page adapter. Closing never cancels a mutation or replaces its frozen variables. */
export function useCreatorLeave({ dirty, busy, onClose }: { dirty: boolean; busy: boolean; onClose: () => void }) {
  const title = useId();
  const [closing, setClosing] = useState(false);
  const blocker = useBlocker(dirty || busy);
  const confirming = closing || blocker.state === 'blocked';
  useEffect(() => {
    if (!dirty && !busy) return;
    const warn = (event: BeforeUnloadEvent) => { event.preventDefault(); event.returnValue = ''; };
    window.addEventListener('beforeunload', warn);
    return () => window.removeEventListener('beforeunload', warn);
  }, [dirty, busy]);
  function keep() { setClosing(false); if (blocker.state === 'blocked') blocker.reset(); }
  function leave() { if (blocker.state === 'blocked') blocker.proceed(); else onClose(); }
  return {
    requestClose: () => { if (dirty || busy) setClosing(true); else onClose(); },
    confirmation: confirming ? <ModalDialog labelledBy={title} className={styles.dialog} onClose={keep}>
      <header className={styles.head}><h2 id={title}>{busy ? '提交仍在处理中' : '放弃未保存的修改？'}</h2></header>
      <div className={styles.body}><p>{busy ? '收起不会取消原请求。原请求仍按发起时的作者、领域和内容处理；回执到达后会刷新列表，不会改写之后打开的对象。' : '本次修改尚未保存。继续编辑，或明确放弃后离开。'}</p></div>
      <footer className={styles.foot}>
        <ActionButton autoFocus data-initial-focus onClick={keep}>{busy ? '等待原请求' : '继续编辑'}</ActionButton>
        <ActionButton onClick={leave}>{busy ? '收起并继续跟踪' : '放弃修改'}</ActionButton>
      </footer>
    </ModalDialog> : null,
  };
}
