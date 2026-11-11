import { useEffect, useState } from 'react';
import { ActionButton } from '../../shared/ActionButton';
import { readCommentAssignments, type CommentAssignment, type CommentAssignmentScope } from './commentAssignmentsApi';
import { c } from './styles';
type Props = CommentAssignmentScope & { compact?: boolean; revision?: number };
const stateLabels = { not_analyzed: '尚未分析', pending: '挂靠中', failed: '挂靠失败', unavailable: '归属待更新' };
export function CommentAssignments(props: Props) {
  return <AssignmentList key={`${props.domain}:${props.work}:${props.request ?? ''}`} {...props} />;
}
function AssignmentList({ domain, work, request, compact = false, revision = 0 }: Props) {
  const [items, setItems] = useState<CommentAssignment[] | null>(null);
  const [error, setError] = useState(false);
  const [retry, setRetry] = useState(0);
  useEffect(() => {
    const controller = new AbortController();
    let timer: ReturnType<typeof setTimeout> | undefined;
    async function read() {
      try {
        const next = await readCommentAssignments({ domain, work, request }, controller.signal);
        if (controller.signal.aborted) return;
        setItems(next); setError(false);
        timer = setTimeout(() => void read(), 5000);
      } catch {
        if (!controller.signal.aborted) { setItems(null); setError(true); }
      }
    }
    void read();
    return () => { controller.abort(); if (timer) clearTimeout(timer); };
  }, [domain, work, request, revision, retry]);
  return <section className={c('lgi-tm-section')} aria-label="评论主题挂靠">
    <div className={c('lgi-tm-section-head')}><h3>评论主题挂靠</h3><ActionButton variant="quiet" onClick={() => setRetry(value => value + 1)}>刷新挂靠</ActionButton></div>
    {error ? <p role="alert">挂靠结果读取失败</p> : items === null ? <p role="status">正在读取挂靠结果…</p> : !items.length ? <p>尚无评论语义结果</p> : items.map(item =>
      <article key={item.requestRef} className={c('lgi-tm-comment')}>
        {!item.sourceAvailable ? <span>来源不可读</span> : !item.signals.length ? <span>{item.assignmentState === 'complete' ? '未提取语义信号' : stateLabels[item.assignmentState]}</span> : item.signals.map(signal => <div key={signal.ordinal}>
          {compact ? <strong>信号 {signal.ordinal + 1}</strong> : <><h4>{signal.proposition}</h4><blockquote>{signal.evidence.text}</blockquote></>}
          <div className={c('lgi-tm-row')}>
            <span>{signal.assignmentState !== 'complete' ? stateLabels[signal.assignmentState] : signal.assignment?.status === 'matched' ? '已匹配' : signal.assignment ? '未匹配' : '未形成挂靠结果'}</span>
            {signal.assignmentState === 'complete' && signal.assignment?.status === 'matched' ? signal.assignment.matches.map(match => <a key={match.definitionRef} href={`/topics?${new URLSearchParams({ tm: '1', domainRef: domain, topicRef: match.topicRef })}`}>{match.label}</a>) : null}
          </div>
        </div>)}
        {!compact ? <div className={c('lgi-tm-row')}><a href={`/corpus/comments?${new URLSearchParams({ domain, view: 'comments', commentWorkRef: item.workRef, commentExternalId: item.commentExternalId, detail: 'comment', researchRequestRef: item.requestRef })}`}>查看原评论</a><a href={`/corpus/comments?${new URLSearchParams({ domain, view: 'comments', workRef: item.workRef })}`}>查看所属作品评论</a></div> : null}
      </article>)}
  </section>;
}
