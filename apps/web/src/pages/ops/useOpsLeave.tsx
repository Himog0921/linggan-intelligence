import { useEffect, useState } from 'react';
import { ActionButton } from '../../shared/ActionButton';
import { Dialog } from './ui';
import { c } from './styles';

/** S1 Ops adapter. Dismissal never aborts or replaces the original target request. */
export function useOpsLeave({ dirty, pending = false, onClose }: { dirty: boolean; pending?: boolean; onClose: () => void }) {
  const [intent, setIntent] = useState<'close' | 'switch_period'>('close');
  const [action, setAction] = useState<(() => void) | null>(null);
  useEffect(() => {
    if (!dirty && !pending) return;
    const protect = (event: BeforeUnloadEvent) => { event.preventDefault(); event.returnValue = ''; };
    window.addEventListener('beforeunload', protect);
    return () => window.removeEventListener('beforeunload', protect);
  }, [dirty, pending]);
  const keep = () => setAction(null);
  const requestLeave = (next: () => void = onClose, purpose: 'close' | 'switch_period' = 'close') => { if (dirty || pending) { setIntent(purpose); setAction(() => next); } else next(); };
  return {
    requestClose: () => requestLeave(), requestLeave,
    confirmation: action ? <Dialog label={pending ? '原请求仍待核实' : '放弃未应用的修改？'} onClose={keep}>
      <div className={c('ops-dialog-body')}><h2>{pending ? '原请求仍待核实' : '放弃未应用的修改？'}</h2>
        <p>{pending ? intent === 'switch_period' ? '切换周期不会停止原请求。返回原周期后继续核实同一个请求，不重复建立目标修改。' : '收起不会停止原请求。重新打开后继续核实同一个请求，不重复建立目标修改。' : '修改尚未应用；继续编辑会保留当前输入。'}</p>
      </div>
      <footer><ActionButton autoFocus onClick={keep}>继续编辑</ActionButton><ActionButton onClick={() => { setAction(null); action(); }}>{intent === 'switch_period' ? pending ? '切换并继续跟踪' : '放弃修改并切换' : pending ? '收起并继续跟踪' : '放弃修改'}</ActionButton></footer>
    </Dialog> : null,
  };
}
