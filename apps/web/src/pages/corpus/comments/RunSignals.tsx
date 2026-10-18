import { eligibilityLabel, label, observationRoleLabel, pairDecisionLabel, pairStateLabel, resolutionLabel, signalKindLabel } from './labels';
import type { RunSignal } from './runDtos';
import type { StudyNavigation } from './studyApi';
import { c } from './styles';

export function RunSignals({ signals, runRef, focusedTarget, focusedSignal, nav }: { signals: RunSignal[]; runRef: string; focusedTarget: string | null; focusedSignal: string | null; nav: StudyNavigation }) {
  if (!signals.length) return <p className={c('muted')}>本次运行没有可读取的研究信号。</p>;
  return <>{signals.map(signal => {
    const readable = signal.sourceState === 'known';
    return <article key={signal.signalRef} className={c('study-signal-card')} data-signal-ref={signal.signalRef} data-target-ref={signal.targetRef} data-focus={focusedSignal === signal.signalRef || focusedTarget === signal.targetRef}>
      <header><span className={c('study-badge')}>{label(signalKindLabel, signal.kind) ?? signal.kind}</span><span>{observationRoleLabel[signal.observationRole ?? ''] || '来源角色未知'} · {label(resolutionLabel, signal.resolutionState) ?? '尚未进入归并判断'}</span></header>
      {readable ? <><p className={c('study-signal-proposition')}>{signal.proposition}</p><blockquote>{signal.evidence}</blockquote></> : <p className={c('study-restricted')}>本条或父语境已受限，研究衍生文本不再显示。</p>}
      <p className={c('study-signal-meta')}>归并资格：{label(eligibilityLabel, signal.eligibilityState) ?? signal.eligibilityState}{readable && signal.eligibilityReason ? ` · ${signal.eligibilityReason}` : ''}</p>
      {readable ? signal.pairOutcomes.map((outcome, i) => {
        const state = label(pairStateLabel, outcome.state) ?? '配对状态未知', decision = outcome.decisionReason ? label(pairDecisionLabel, outcome.decisionReason) ?? '配对结论未能识别' : '历史配对未记录可分类结论';
        const recallRank = Number(outcome.selection?.recallRank), admissibleRank = Number(outcome.selection?.admissibleRank), selectionText = Number.isInteger(recallRank) && recallRank > 0 && Number.isInteger(admissibleRank) && admissibleRank > 0 ? `本次自动选择：召回候选第 ${recallRank} 位；通过独立性筛选后第 ${admissibleRank} 位。` : '历史配对未记录候选选择信息。';
        return <p className={c('study-signal-meta')} key={i}>首个候选比较：{state} · {decision}<br />{selectionText}</p>;
      }) : null}
      <div className={c('study-link-actions')}><button type="button" className={c('study-link')} data-signal-target={signal.targetRef} onClick={() => nav.run(runRef, 'targets', signal.targetRef)}>查看目标评论</button>{!readable ? <span>来源受限，关联信息已隐藏</span> : signal.resolvedProblemRef ? <button type="button" className={c('study-link')} data-signal-problem={signal.resolvedProblemRef} onClick={() => nav.problem(signal.resolvedProblemRef!)}>查看用户问题</button> : <span>尚未关联长期用户问题</span>}</div>
    </article>;
  })}</>;
}
