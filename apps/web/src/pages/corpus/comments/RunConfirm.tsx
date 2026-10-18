import { shortRef } from './format';
import type { StudyRun } from './runDtos';
import { StudyDialog } from './StudyDialog';
import { c } from './styles';

export function RunConfirm({ run, action, error, busy, confirm, close }: { run: StudyRun; action: 'stop' | 'recover'; error: string; busy: boolean; confirm: () => void; close: () => void }) {
  const stop = action === 'stop', prefix = stop ? 'study-stop' : 'study-recover';
  return <StudyDialog id={`${prefix}-dialog`} titleId={`${prefix}-title`} describedBy={stop ? 'study-stop-run-identity study-stop-description' : 'study-recover-description'} onClose={() => { if (!busy) close(); }}><div className={c('study-stop-dialog-body')}><p className={c('study-label')}>{stop ? '运行控制' : '有界恢复'}</p><h2 id={`${prefix}-title`}>{stop ? '停止本次研究？' : '补跑未完成评论？'}</h2>{stop ? <p id="study-stop-run-identity" className={c('study-stop-run-identity')}>研究批次 {run.runRef} · 当前未终态 {run.pendingCount == null ? '未知' : `${run.pendingCount} 条`} · 目标总数 {Number(run.targetCount)} 条</p> : null}
    <p id={`${prefix}-description`}>{stop ? '尚未外发的目标会标记为取消，已接纳结果会保留。已经开始的调用无法保证中断，可能仍产生费用；获准的在途结果仍会结算。停止后不能恢复原研究批次；可从该批次显式补跑未完成评论。' : `原研究批次 ${shortRef(run.runRef)}。仅从本次未完成评论中重新核对当前资格，跳过后续已成功、在途、受限或仍不可恢复的评论；沿用原方法与本次最多 ${Number(run.limits?.commentBudget)} 条、${Number(run.limits?.tokenLimit)} 词元预算。新的研究批次会保留与原批次的关联，原记录不改写。`}</p><p id={`${prefix}-error`} className={c('study-stop-error')} role="status" aria-live="polite">{error}</p><div className={c('study-stop-actions')}><button id={`${prefix}-confirm`} type="button" className={stop ? c('study-danger-button') : undefined} disabled={busy} onClick={confirm}>{stop ? busy ? '正在提交…' : '确认停止' : '确认创建新运行'}</button><button id={`${prefix}-dismiss`} type="button" autoFocus disabled={busy} onClick={close}>返回</button></div>
  </div></StudyDialog>;
}
