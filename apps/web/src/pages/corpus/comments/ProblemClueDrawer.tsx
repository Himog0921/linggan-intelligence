import { useRef } from 'react';
import { errorText } from './format';
import { pairDecisionLabel, pairStateLabel } from './labels';
import { ProblemEvidenceRow } from './ProblemDrawer';
import { ProblemDrawerShell } from './ProblemDrawerShell';
import { parseSourceContext } from './commentSourceDto';
import type { ProblemClue } from './problemDtos';
import { candidateStates } from './problemModel';
import { useStudyRead, type StudyNavigation } from './studyApi';
import { c } from './styles';

export function ProblemClueDrawer({ domain, clue, close, nav }: { domain: string; clue: ProblemClue; close: () => void; nav: StudyNavigation }) {
  const dialog = useRef<HTMLDialogElement>(null);
  const params = new URLSearchParams({ ...clue.commentKey });
  const query = useStudyRead(domain, `comments/detail?${params}`, row => { const value = parseSourceContext(row); if (value.commentKey.workRef !== clue.commentKey.workRef || value.commentKey.commentExternalId !== clue.commentKey.commentExternalId) throw new Error('读取回执与当前范围不一致，请重新读取。'); return value; });
  const fresh = query.data, can = !query.isFetching && fresh?.sourceState === 'known' && Boolean(fresh.commentText?.trim()), unchanged = can && fresh?.commentText === clue.commentText;
  return <ProblemDrawerShell dialogRef={dialog} close={close} feedback={query.isPending ? '只读研究结果' : '线索可阅读，不需要人工确认才能展示。'}>
    {query.isPending ? <h2 id="sr3-drawer-title">正在核对线索来源…</h2> : query.isError ? <><h2 id="sr3-drawer-title">线索暂时不可读取</h2><div className={c('sr3-message')} role="status"><strong>{errorText(query.error)}</strong><button type="button" className={c('sr3-button')} onClick={() => { void query.refetch(); }}>重新读取</button></div></> : <>
      <section className={c('sr3-detail-intro')}><span className={c('sr3-eyebrow')}>未归并线索 / 不作为已建档问题</span><h2 id="sr3-drawer-title">{unchanged ? clue.proposition || '归一表达未记录' : can ? '原声已变化，旧研究表达不再显示' : '来源当前不可读取'}</h2><p>{candidateStates[clue.state] || '处理状态未记录'}</p></section>
      <section className={c('sr3-detail-section')}><ProblemEvidenceRow domain={domain} row={{ ...clue, commentText: can ? fresh?.commentText ?? null : null, sourceState: can ? 'known' : 'restricted', authorDisplayName: clue.authorDisplayName, addedAt: null, signals: [] }} index={0} sourceUnavailable={() => { void query.refetch(); }} /><h3>当前候选比较记录</h3>{unchanged && clue.pairOutcomes.length ? clue.pairOutcomes.map((pair, i) => <p key={i}>{pairStateLabel[pair.state || ''] || pair.state} · {pairDecisionLabel[pair.decisionReason || ''] || pair.decisionReason || '原因未记录'}</p>) : <p>没有当前可读取的比较结论，不能据此判断是否已有独立依据。</p>}{clue.runRef ? <button type="button" className={c('sr3-button')} data-sr3="run" onClick={() => { close(); nav.run(clue.runRef!, 'signals'); }}>查看来源研究批次</button> : null}</section>
    </>}
  </ProblemDrawerShell>;
}
