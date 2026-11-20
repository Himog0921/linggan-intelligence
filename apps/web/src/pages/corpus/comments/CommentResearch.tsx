import { useQuery } from '@tanstack/react-query';
import { CommentAssignments } from '../../topics/CommentAssignments';
import { readLatest, why } from './understandingApi';
import type { StudyNavigation } from './studyApi';
import { CommentResearchSelection } from './CommentResearchSelection';
import { signalKindLabel } from './labels';
import { STATES } from './researchModel';
import { c } from './styles';

export function CommentResearch({ domain, work, source, role, nav }: { domain: string; work: string; source: string; role: string; nav: StudyNavigation }) {
  const comment = useQuery({
    queryKey: ['comment-understanding', domain, work, source],
    queryFn: ({ signal }) => readLatest(domain, work, source, signal),
    retry: false, gcTime: 0,
  });
  const latest = comment.isError ? null : comment.data;
  const selection = { sourceRef: source, commentKey: { workRef: work, commentExternalId: '' }, observationRole: role };
  return <section className={c('study-detail-section')} aria-label="研究这条评论">
    <h3>研究记录</h3>
    {comment.isPending ? <p>正在读取评论研究…</p> : comment.isError ? <p role="alert">研究记录读取失败。 <button type="button" onClick={() => void comment.refetch()}>重试读取</button></p> :
      latest ? <p>最近一次执行 · {STATES[latest.state] ?? '状态待核对'}{latest.failureCode ? ` · ${why(latest.failureCode)}` : ''}</p> : <p>尚未发起评论理解。</p>}
    {latest ? <button className={c('study-link')} type="button" onClick={() => nav.run?.(latest.batchRef)}>查看运行回执</button> : null}
    {latest && !latest.result && latest.state === 'succeeded' ? <p>当前没有可读取的研究结果。</p> : null}
    {latest?.result ? <>
      {latest.result.summary ? <><h4>作品 L1 语境</h4><p>{latest.result.summary}</p></> : null}
      <h4>当前研究结果</h4>
      {latest.result.outcome === 'no_signal' ? <p>未发现可研究信号。{latest.result.reason}</p> : null}
      {latest.result.outcome === 'needs_context' ? <p>需要更多语境。{latest.result.reason}</p> : null}
      {latest.result.signals.length ? <ul>{latest.result.signals.map((signal, index) => <li key={index}>
        <p>{signalKindLabel[signal.kind] ?? '研究信号'} · {signal.proposition}</p>
        <blockquote>{signal.quote}</blockquote>
        {signal.problem ? <button className={c('study-link')} type="button" onClick={() => nav.problem?.(signal.problem!.problemRef)}>
          {signal.problem.label} · {signal.problem.state === 'formal' ? '正式' : '候选'} · {signal.problem.authors} 位作者
        </button> : null}
      </li>)}</ul> : null}
      {latest.result.signals.length ? <CommentAssignments domain={domain} work={work} request={latest.result.requestRef} compact /> : null}
    </> : null}
    <CommentResearchSelection domain={domain} selected={[selection]} nav={nav} />
    <button className={c('study-link')} type="button" disabled={comment.isFetching} onClick={() => void comment.refetch()}>刷新研究状态</button>
  </section>;
}
