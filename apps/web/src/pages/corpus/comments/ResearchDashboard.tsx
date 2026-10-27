import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { requestJson } from '../../../shared/api';
import { bool, decode, list, num, optStr, str } from '../../../shared/decode';
import type { StudyNavigation } from './studyApi';
import { c } from './styles';
import { DomainWordCloud } from './DomainWordCloud';
import type { Category } from './DomainWordCloud';

import { PHASES, STATES, REASONS } from './researchModel';
export function ResearchDashboard({ domain, view, nav }: { domain: string; view: 'overview' | 'runs' | 'problems'; nav: StudyNavigation }) {
  const [notice, setNotice] = useState('');
  const [days, setDays] = useState(56), [category, setCategory] = useState<Category>('all');
  const runs = useQuery({ queryKey: ['comment-research', domain, 'runs'], queryFn: ({ signal }) => requestJson(`/api/local/comment-research/runs?${new URLSearchParams({ domainRef: domain })}`, { signal }).then(value => decode(value, root => list(root, 'runs', row => ({ requestRef: str(row, 'requestRef'), workRef: str(row, 'workRef'), commentExternalId: str(row, 'commentExternalId'), state: str(row, 'state'), available: bool(row, 'sourceAvailable'), stages: list(row, 'stages', stage => ({ phase: num(stage, 'stageNumber'), state: str(stage, 'state'), failure: optStr(stage, 'failureCode') })) })))), retry: false });
  const problems = useQuery({ queryKey: ['comment-research', domain, 'problems'], queryFn: ({ signal }) => requestJson(`/api/local/comment-research/problems?${new URLSearchParams({ domainRef: domain })}`, { signal }).then(value => decode(value, root => ({ problems: list(root, 'problems', row => ({ problemRef: str(row, 'problemRef'), label: str(row, 'label'), actor: str(row, 'actor'), goal: str(row, 'goal'), barrier: str(row, 'barrier'), context: optStr(row, 'context'), evidence: list(row, 'evidence', entry => ({ requestRef: str(entry, 'requestRef'), ordinal: num(entry, 'signalOrdinal'), workRef: str(entry, 'workRef'), commentExternalId: str(entry, 'commentExternalId'), quote: str(entry, 'quote'), proposition: str(entry, 'proposition') })) })), candidates: list(root, 'candidates', row => ({ requestRef: str(row, 'requestRef'), ordinal: num(row, 'signalOrdinal'), workRef: str(row, 'workRef'), commentExternalId: str(row, 'commentExternalId'), quote: str(row, 'quote') })) }))), retry: false });
  async function cancel(request: string) { try { await requestJson('/api/local/comment-research/cancel', { method: 'POST', body: { commandRef: crypto.randomUUID(), domainRef: domain, requestRef: request } }); setNotice('已请求取消，正在发送的调用将保留回执，但不会接纳新的问题归属。'); await runs.refetch(); } catch { setNotice('取消回执未知，请刷新后核对。'); } }
  return <section className={c('study-detail-section')} aria-label="评论研究进度">
    <h2>{view === 'problems' ? '用户问题' : view === 'runs' ? '研究记录' : '评论研究'}</h2>
    <p>作品研究底稿和父评论提供语境；目标评论提供原声依据。原声提取后经过问题归并与独立核验，通过后进入用户问题。</p>
    <p>各阶段模型、提示词和 Skill 在 AI 设置分别管理。任务提交时固定做法，进行中的研究沿用提交时版本。</p>
    <button type="button" disabled={runs.isFetching || problems.isFetching} onClick={() => { void runs.refetch(); void problems.refetch(); }}>刷新研究记录与问题</button>
    {view === 'overview' ? <button type="button" onClick={() => nav.view('comments')}>选择评论开始研究</button> : null}
    {notice ? <p role="status">{notice}</p> : null}
    {view !== 'problems' ? runs.isPending ? <p>正在读取研究记录…</p> : runs.isError ? <p role="alert">研究记录读取失败，请重试刷新。</p> : <><p>最近最多 100 条研究记录；{runs.data.length} 条可见。完成状态表示流程回执，研究内容仍须人工判断。</p>{!runs.data.length ? <p>尚无新流程的研究记录。</p> : runs.data.map(run => <article key={run.requestRef}><h3>{STATES[run.state] ?? run.state}</h3>{!run.available ? <p>来源当前不可读取，研究内容隐藏。</p> : <button type="button" onClick={() => nav.research(run.requestRef, run)}>查看评论与研究依据</button>}<ol>{run.stages.map(stage => <li key={stage.phase}>{PHASES[stage.phase]}：{STATES[stage.state] ?? stage.state}{stage.failure ? ` · ${REASONS[stage.failure] ?? '请查看 AI 调用记录'}` : ''}</li>)}</ol>{['running', 'needs_support'].includes(run.state) ? <button type="button" onClick={() => void cancel(run.requestRef)}>{run.state === 'needs_support' ? '撤销待支持候选' : '取消余下阶段'}</button> : null}</article>)}</> : null}
    {view !== 'runs' ? problems.isPending ? <p>正在读取用户问题…</p> : problems.isError ? <p role="alert">问题读取失败，请重试刷新。</p> : <><h3>经核验的用户问题</h3>{!problems.data.problems.length ? <p>尚无通过归并核验且来源可读的问题。</p> : problems.data.problems.map(problem => <article key={problem.problemRef}><h4>{problem.label}</h4><dl><dt>谁</dt><dd>{problem.actor}</dd><dt>想完成什么</dt><dd>{problem.goal}</dd><dt>遇到什么阻碍</dt><dd>{problem.barrier}</dd>{problem.context ? <><dt>情境</dt><dd>{problem.context}</dd></> : null}</dl>{problem.evidence.map(entry => <div key={entry.requestRef + ':' + entry.ordinal}><p>{entry.proposition}</p><blockquote>{entry.quote}</blockquote><button type="button" onClick={() => nav.comment(entry)}>查看原声</button></div>)}</article>)}</> : null}
    {view !== 'runs' && problems.data?.candidates.length ? <section aria-label="待支持的问题候选"><h3>等待独立作者支持</h3><p>单条原声只形成候选。需要另一位作者的原声支持同一问题，并通过独立核验，才会进入用户问题。</p>{problems.data.candidates.map(entry => <article key={entry.requestRef + ':' + entry.ordinal}><blockquote>{entry.quote}</blockquote><button type="button" onClick={() => nav.research(entry.requestRef, entry)}>查看候选研究依据</button></article>)}</section> : null}
    {view === 'overview' ? <><div role="group" aria-label="词频观察时段">{[7, 28, 56].map(value => <button type="button" key={value} aria-pressed={days === value} onClick={() => setDays(value)}>近 {value} 天</button>)}</div><DomainWordCloud domain={domain} days={days} nav={nav} category={category} onCategory={setCategory} /></> : null}
  </section>;
}
