import { useEffect, useRef, useState } from 'react';
import { errorText, shortRef } from './format';
import { parseControl, parseRecovery, type StudyRun } from './runDtos';
import { studyPost } from './studyApi';
import type { ControlAction } from './RunActions';

export function useRunControl({ domain, refresh, created, recoveryRefs }: { domain: string; refresh: () => Promise<void>; created: (ref: string) => void; recoveryRefs: Map<string, string> }) {
  const [modal, setModal] = useState<{ run: StudyRun; action: 'stop' | 'recover' } | null>(null), [feedback, setFeedback] = useState(''), [error, setError] = useState(''), [busy, setBusy] = useState(false);
  const guard = useRef(false), alive = useRef(true);
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  async function control(run: StudyRun, action: Exclude<ControlAction, 'recover'>) {
    if (guard.current || run.controlVersion == null) return;
    guard.current = true; setBusy(true); setError('');
    if (action !== 'stop') setFeedback(action === 'pause' ? '正在暂停派发…' : '正在恢复派发…');
    let confirmed = false; let text: string;
    try {
      const result = await studyPost(domain, `runs/${encodeURIComponent(run.runRef)}/${action}`, { expectedControlVersion: run.controlVersion }, parseControl);
      if (result.runRef !== run.runRef || result.controlVersion !== run.controlVersion + 1 || result.dispatchState !== ({ pause: 'paused', resume: 'enabled', stop: 'stopped' })[action] || result.dispatchReason !== ({ pause: 'user_paused', resume: null, stop: 'user_stopped' })[action]) throw new Error('服务回执与本次控制命令不匹配。');
      confirmed = true; text = `已按服务端回执${action === 'pause' ? '暂停派发' : action === 'resume' ? '恢复派发' : '停止本次运行'}。`;
    } catch (cause) { text = action === 'stop' ? `停止未获确认：${errorText(cause)}；已重新读取当前运行状态，请核对后再操作。` : `控制未获确认：${errorText(cause)}`; }
    if (alive.current) {
      setModal(null); setFeedback(text);
      try { await refresh(); }
      catch (cause) { if (alive.current) setFeedback(`${text}${confirmed ? ' 运行列表刷新失败；以上为本次命令回执，最新运行状态未重新读取。' : ' 运行列表刷新也失败；命令结果未获确认，最新运行状态未重新读取。'} ${errorText(cause)}`); }
    }
    guard.current = false; if (alive.current) setBusy(false);
  }
  async function recover(run: StudyRun) {
    if (guard.current) return; guard.current = true; setBusy(true); setError('正在重新核对并创建运行…');
    const requestRef = recoveryRefs.get(run.runRef) ?? crypto.randomUUID(); recoveryRefs.set(run.runRef, requestRef);
    try {
      const receipt = await studyPost(domain, `runs/${encodeURIComponent(run.runRef)}/recover`, { domainRef: domain, requestRef }, parseRecovery);
      if (receipt.requestRef !== requestRef || !['created', 'index_pending', 'no_work'].includes(receipt.outcome) || (receipt.outcome === 'created' ? !receipt.runRef || !(Number(receipt.targetCount) > 0) : receipt.runRef != null || receipt.targetCount !== 0)) throw new Error('补跑回执与本次请求不一致');
      recoveryRefs.delete(run.runRef);
      if (alive.current) {
        const text = receipt.outcome === 'created' ? `已按服务端回执创建补跑批次 ${shortRef(receipt.runRef)}，本次冻结 ${Number(receipt.targetCount)} 条。` : '本次没有仍可恢复的评论；未创建空运行。';
        setModal(null); setFeedback(text); if (receipt.runRef) created(receipt.runRef);
        try { await refresh(); } catch (cause) { if (alive.current) setFeedback(`${text} 运行列表刷新失败，最新状态未重新读取：${errorText(cause)}`); }
      }
    } catch (cause) { if (alive.current) setError(`补跑未获确认：${errorText(cause)}。重试将沿用本次请求编号。`); }
    finally { guard.current = false; if (alive.current) setBusy(false); }
  }
  return { modal, feedback, error, busy, dismiss: () => { if (!guard.current) { setModal(null); setError(''); } }, act: (run: StudyRun, action: ControlAction) => { if (guard.current) return; if (action === 'stop' || action === 'recover') { setModal({ run, action }); setError(''); } else void control(run, action); }, confirm: () => { if (modal?.action === 'stop') void control(modal.run, 'stop'); else if (modal) void recover(modal.run); } };
}
