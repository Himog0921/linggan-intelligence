import { useQuery } from '@tanstack/react-query';
import { Fragment, type ReactNode } from 'react';
import { ActionButton } from '../../../shared/ActionButton';
import { CommentAssignments } from '../../topics/CommentAssignments';
import { parseCommentDetail } from './commentDtos';
import { CrIcon } from './CrIcon';
import { Drawer } from './Drawer';
import { errorText, num, studyTime } from './format';
import { cleanStateLabel, signalKindLabel, workTitleSourceLabel } from './labels';
import { commentIdentity, useStudyRead, type CommentKey, type StudyNavigation } from './studyApi';
import { readLatest, why } from './understandingApi';
import { c } from './styles';

function Section({ title, label, children }: { title: string; label?: string; children: ReactNode }) {
  return <section className={c('drawer-section')} aria-label={label}><h3>{title}</h3>{children}</section>;
}

/** Highlight the first research quote that appears verbatim in the comment; never alters the text itself. */
function Quote({ text, quotes }: { text: string; quotes: string[] }) {
  const quote = quotes.find(item => item && text.includes(item));
  if (!quote) return <blockquote className={c('quote')}>{text}</blockquote>;
  const at = text.indexOf(quote);
  return <><blockquote className={c('quote')}>{text.slice(0, at)}<mark title="研究引用的证据原文">{quote}</mark>{text.slice(at + quote.length)}</blockquote>
    <p className={c('quote-note')}><i aria-hidden="true" />标出部分是研究引用的证据原文</p></>;
}

const ACTIVE = new Set(['pending', 'queued', 'running']);

export function CommentDrawer({ domain, commentKey, nav, close, back, filterWork, request }: {
  domain: string; commentKey: CommentKey; nav: StudyNavigation; close: () => void; back: (() => void) | null;
  filterWork: (workRef: string, label: string) => void; request?: string | null;
}) {
  const detail = useStudyRead(domain, `comments/detail?${new URLSearchParams({ ...commentKey })}`, row => {
    const data = parseCommentDetail(row);
    if (commentIdentity(data.commentKey) !== commentIdentity(commentKey)) throw new Error('读取回执与当前范围不一致，请重新读取。');
    return data;
  });
  const data = detail.data, comment = data?.comment, work = data?.work, parent = data?.parentContext;
  const readable = data?.source.sourceState === 'known' && Boolean(comment?.commentText);
  const researchable = Boolean(readable && data?.source.sourceRef && comment && ['primary', 'reference'].includes(comment.observationRole));
  const sourceRef = data?.source.sourceRef ?? '';
  const latestQuery = useQuery({
    queryKey: ['comment-understanding', domain, commentKey.workRef, sourceRef, request ?? ''], enabled: researchable,
    queryFn: ({ signal }) => readLatest(domain, commentKey.workRef, sourceRef, signal), retry: false, gcTime: 0,
    refetchInterval: query => query.state.data && ACTIVE.has(query.state.data.state) ? 3000 : false,
  });
  const latest = latestQuery.isError ? null : latestQuery.data ?? null, result = latest?.result ?? null;
  const running = Boolean(latest && ACTIVE.has(latest.state)), failed = latest?.state === 'failed';
  const workTitle = work?.displayTitle || comment?.workTitle || '';
  const cleaningAddsInformation = Boolean(readable && comment && ((comment.researchText && comment.researchText !== comment.commentText) || comment.cleanReasons.length || (comment.cleanState != null && !['clean', 'direct'].includes(comment.cleanState))));
  const selection = comment && sourceRef ? { sourceRef, commentKey, observationRole: comment.observationRole, text: comment.commentText, workTitle } : null;
  const action = !researchable || !selection ? null
    : result && !running ? <ActionButton onClick={() => nav.preview([selection], { rerun: true })}>重新研究</ActionButton>
    : <ActionButton variant="ink" disabled={running || latestQuery.isPending} onClick={() => nav.preview([selection])}>{running ? '研究中' : failed ? '重试研究' : '研究此条'}</ActionButton>;

  return <Drawer id="comment-detail" title="评论详情" closeLabel="关闭评论详情" close={close} back={back} note={commentKey.commentExternalId}
    actions={<>{latest ? <ActionButton onClick={() => nav.records(latest.batchRef)}>查看研究记录</ActionButton> : null}{action}</>}>
    {detail.isPending ? <p className={c('view-status')}>正在读取评论详情…</p> : !data ? <p className={c('view-status')} role="alert">评论详情读取失败：{errorText(detail.error)} <button type="button" className={c('text-action')} onClick={() => void detail.refetch()}>重试读取</button></p> : <>
      <Section title="原始评论">
        {readable && comment?.commentText ? <Quote text={comment.commentText} quotes={result?.signals.map(signal => signal.quote) ?? []} />
          : <p className={c('notice')}>当前原声不可显示：{data.source.displayState || data.source.sourceState || '来源状态未知'}</p>}
        {comment ? <div className={c('facts')}>
          <span>{comment.authorDisplayName || '作者未记录'}</span>
          <span>{comment.likeCount == null ? '点赞未知' : `点赞 ${num(comment.likeCount)}`}</span>
                    <span>{comment.observedAt ? `观测 ${studyTime(comment.observedAt).replaceAll('-', '/')}` : '观测时间未记录'}</span>
          {comment.postUsage?.state === 'used' ? <span>已改成帖子{comment.postUsage.markedAt ? `（${studyTime(comment.postUsage.markedAt)}）` : ''}</span> : null}
        </div> : null}
      </Section>
      <Section title="所属作品 · 作品语境">
        <div className={c('work-context')}><strong>{workTitle || '未命名作品'}</strong>
          <p>{result?.summary || `${workTitleSourceLabel[work?.displayTitleSource || ''] || '标题来源未记录'}；作品语境会随研究结果一并显示。`}</p></div>
        <button type="button" className={c('text-action')} data-detail-work={commentKey.workRef} onClick={() => filterWork(commentKey.workRef, workTitle)}>查看该作品的评论 <CrIcon name="ArrowRight" /></button>
      </Section>
      {researchable ? <Section title="评论研究" label="研究这条评论">
        {latestQuery.isPending ? <p className={c('view-status')}>正在读取研究结果…</p>
          : latestQuery.isError ? <p className={c('view-status')} role="alert">研究结果读取失败。 <button type="button" className={c('text-action')} onClick={() => void latestQuery.refetch()}>重试读取</button></p> : <>
          {running ? <div className={c('notice info')}>正在研究这条评论。{result ? '已有有效结果继续保留。' : '完成后会更新结果。'}</div> : null}
          {failed ? <div className={c('notice')}><strong>最近一次执行失败</strong><br />{latest?.failureCode ? why(latest.failureCode) : '未取得新的研究结果。'}{result ? <><br />以下仍显示此前的有效结果。</> : null}</div> : null}
          {result ? <>
            <div className={c('result-title')}>
              <span className={c('state', result.outcome === 'needs_context' ? 'wait' : 'good')}>{result.outcome === 'needs_context' ? '研究完成 · 需要语境' : result.outcome === 'no_signal' ? '研究完成 · 无信号' : '已有有效结果'}</span>
              {result.signals[0] ? <span className={c('pill')}>{signalKindLabel[result.signals[0].kind] ?? '研究信号'}</span> : null}
            </div>
            <div className={c('research-result')}>
              {result.signals.length ? result.signals.map((signal, index) => <Fragment key={index}>
                <p>{result.signals.length > 1 ? `${signalKindLabel[signal.kind] ?? '研究信号'} · ` : ''}{signal.proposition}</p>
                {signal.problem ? <p><span className={c('muted')}>问题归属</span> <button type="button" className={c('text-action')} onClick={() => nav.problem(signal.problem!.problemRef, { related: true })}>{signal.problem.label} <CrIcon name="ArrowUpRight" /></button></p> : null}
              </Fragment>) : <p>{result.outcome === 'needs_context' ? '需要更多语境。' : '未发现可研究信号。'}{result.reason}</p>}
            </div>
            {result.signals.length ? <CommentAssignments domain={domain} work={commentKey.workRef} request={result.requestRef} compact /> : null}
          </> : !running && !failed ? <p className={c('view-status')}>尚未产生有效研究结果。</p> : null}
          {result && !running ? <p className={c('threshold-note')}>重新研究会生成新结果；现有结果保留到新结果完成为止。</p> : null}
        </>}
      </Section> : null}
      {readable && parent?.commentText ? <Section title="父评论语境"><blockquote className={c('parent-quote')}>{parent.commentText}</blockquote>
        <p className={c('threshold-note')}>仅作为语境，不作为当前评论的独立证据。</p>
        {parent.commentKey ? <button type="button" className={c('text-action')} onClick={() => nav.comment(parent.commentKey!, { related: true })}>查看父评论 <CrIcon name="ArrowUpRight" /></button> : null}</Section> : null}
      {cleaningAddsInformation && comment ? <Section title="清洗差异"><p className={c('detail-desc')}>{comment.researchText || '清洗后文本未记录'}</p>
        <p className={c('threshold-note')}>状态：{cleanStateLabel[comment.cleanState || ''] || comment.cleanState || '未记录'} · 原因：{comment.cleanReasons.join('、') || '未记录'}</p></Section> : null}
    </>}
  </Drawer>;
}
