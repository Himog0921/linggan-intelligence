import { useInfiniteQuery } from '@tanstack/react-query';
import type { ReactNode } from 'react';
import { parseCommentDetail, parseHistory, parseRelated, type CommentDetail as Detail, type CommentHistory } from './commentDtos';
import { shortBeijingDate } from './commentModel';
import { errorText, studyTime } from './format';
import { cleanStateLabel, label, signalKindLabel, targetStateLabel, workTitleSourceLabel } from './labels';
import { StudyDialog } from './StudyDialog';
import { commentIdentity, studyKey, studyRead, useStudyRead, type CommentKey, type StudyNavigation } from './studyApi';
import { c } from './styles';

function Section({ title, children }: { title: string; children: ReactNode }) { return <section className={c('study-detail-section')}><h3>{title}</h3>{children}</section>; }
function HistoryRow({ item, nav }: { item: CommentHistory; nav: StudyNavigation }) {
  return <li><strong>{label(targetStateLabel, item.state) ?? item.state}</strong><span>{studyTime(item.createdAt)}</span><span>信号 {Number(item.signalCount || 0)} 条</span><span>方法：{item.method?.recordingState === 'recorded' ? item.method.methodName || item.method.policyRef : '历史未记录'}</span>{item.runRef ? <div className={c('study-link-actions')}>{([['targets', '查看本次目标'], ['signals', '查看本次信号'], ['method', '查看本次方法']] as const).map(([panel, title]) => <button type="button" className={c('study-link')} key={panel} data-history-run={item.runRef} data-history-panel={panel} data-history-target={panel === 'method' ? undefined : item.targetRef ?? ''} onClick={() => nav.run(item.runRef!, panel, panel === 'method' ? null : item.targetRef)}>{title}</button>)}</div> : <span>历史研究批次编号未记录，无法跳转。</span>}</li>;
}
function History({ domain, data, nav }: { domain: string; data: Detail; nav: StudyNavigation }) {
  const initial = data.studyHistory ?? { items: [], totalCount: null, page: { nextCursor: null } };
  const params = new URLSearchParams({ ...data.commentKey, limit: '50' });
  const query = useInfiniteQuery({ queryKey: studyKey(domain, `comments/history/${commentIdentity(data.commentKey)}`), initialPageParam: null as string | null, initialData: { pages: [initial], pageParams: [null] }, staleTime: Infinity,
    queryFn: ({ signal, pageParam }) => { const next = new URLSearchParams(params); if (pageParam) next.set('cursor', pageParam); return studyRead(domain, `comments/history?${next}`, parseHistory, signal); }, getNextPageParam: page => page.page.nextCursor || undefined, retry: false, gcTime: 0, refetchOnWindowFocus: false,
  });
  const items = query.data.pages.flatMap(page => page.items);
  return <Section title={`研究历史（${Number(data.studyHistory?.totalCount || 0)}）`}>{items.length ? <ol className={c('study-history-list')}>{items.map((item, i) => <HistoryRow key={item.targetRef || i} item={item} nav={nav} />)}</ol> : <p className={c('muted')}>尚无研究历史。</p>}{query.hasNextPage ? <button type="button" id="comment-history-more" disabled={query.isFetchingNextPage} onClick={() => { void query.fetchNextPage(); }}>{query.isFetchingNextPage ? '正在读取…' : query.isFetchNextPageError ? `读取失败，重试：${errorText(query.error)}` : '继续读取研究历史'}</button> : null}</Section>;
}
export function CommentDetail({ domain, commentKey, nav, close, filterWork }: { domain: string; commentKey: CommentKey; nav: StudyNavigation; close: () => void; filterWork: (key: string, label: string) => void }) {
  const params = new URLSearchParams({ ...commentKey });
  const detail = useStudyRead(domain, `comments/detail?${params}`, row => { const data = parseCommentDetail(row); if (commentIdentity(data.commentKey) !== commentIdentity(commentKey)) throw new Error('读取回执与当前范围不一致，请重新读取。'); return data; });
  const related = useInfiniteQuery({ queryKey: studyKey(domain, `comments/related/${commentIdentity(commentKey)}`), initialPageParam: null as string | null,
    queryFn: ({ signal, pageParam }) => { const next = new URLSearchParams({ ...commentKey, limit: '50' }); if (pageParam) next.set('cursor', pageParam); return studyRead(domain, `comments/related?${next}`, row => { const data = parseRelated(row); if (commentIdentity(data.commentKey) !== commentIdentity(commentKey)) throw new Error('读取回执与当前范围不一致，请重新读取。'); return data; }, signal); }, getNextPageParam: page => page.page.nextCursor || undefined, retry: false, gcTime: 0, refetchOnWindowFocus: false,
  });
  const data = detail.data, comment = data?.comment, work = data?.work, parent = data?.parentContext;
  const sourceReadable = data?.source.sourceState === 'known';
  const ready = !detail.isPending && !related.isPending;
  const signals = related.data?.pages.flatMap(page => page.signals) ?? [], relatedKnown = related.data?.pages.every(page => page.sourceState === 'known');
  const likeText = comment?.likeCount == null ? '点赞数未知' : `点赞 ${comment.likeCount.toLocaleString('zh-CN')}${comment.likeCountAsOf ? `（采集于 ${shortBeijingDate(comment.likeCountAsOf)}）` : ''}`;
  const usageText = comment?.postUsage?.state === 'used' ? `已改成帖子${comment.postUsage.markedAt ? `（${studyTime(comment.postUsage.markedAt)} 标记）` : ''}` : '未标记改帖';
  return <StudyDialog id="comment-detail-dialog" className={c('study-detail-dialog')} titleId="comment-detail-title" onClose={close}><header className={c('study-dialog-header')}><div><p className={c('study-label')}>原声证据</p><h2 id="comment-detail-title">评论详情</h2></div><button type="button" id="comment-detail-close" onClick={close}>关闭</button></header><div id="comment-detail-body" className={c('study-detail-body')} aria-live="polite">
    {!ready ? <p className={c('study-empty')}>正在读取评论详情…</p> : !data ? <p className={c('study-empty')}>评论详情读取失败：{errorText(detail.error)}</p> : <>
      {sourceReadable && comment?.commentText ? <><blockquote>{comment.commentText}</blockquote><p className={c('study-signal-meta')}>{likeText} · {usageText}</p></> : <p className={c('study-restricted')}>当前原声不可显示：{data.source.displayState || data.source.sourceState || 'unknown'}</p>}
      <Section title="所属作品"><p><strong>{work?.displayTitle || comment?.workTitle || '未命名作品'}</strong> <small>{workTitleSourceLabel[work?.displayTitleSource || ''] || work?.displayTitleSource || '标题未记录'}</small></p><button type="button" className={c('study-link')} data-detail-work={commentKey.workRef} data-detail-work-title={work?.displayTitle || comment?.workTitle || ''} onClick={() => filterWork(commentKey.workRef, work?.displayTitle || comment?.workTitle || '')}>查看该作品的评论</button></Section>
      <Section title="父评论语境">{sourceReadable && parent?.commentText ? <><blockquote>{parent.commentText}</blockquote><p className={c('study-signal-meta')}>仅作为语境，不作为当前评论的独立证据。</p>{parent.commentKey ? <button type="button" className={c('study-link')} data-detail-parent-work={parent.commentKey.workRef} data-detail-parent-id={parent.commentKey.commentExternalId} onClick={() => nav.comment(parent.commentKey!)}>查看父评论详情</button> : null}</> : <p className={c('muted')}>没有可显示的父评论语境。</p>}</Section>
      <Section title="清洗文本">{sourceReadable && comment ? <><p>{comment.researchText || ''}</p><p className={c('study-signal-meta')}>状态：{cleanStateLabel[comment.cleanState || ''] || comment.cleanState || '未记录'} · 原因：{comment.cleanReasons.join('、') || '无'}</p></> : <p className={c('study-restricted')}>清洗文本当前不可显示。</p>}</Section>
      <History domain={domain} data={data} nav={nav} />
      <Section title="相关信号与用户问题">{!sourceReadable ? <p className={c('study-restricted')}>来源当前受限，关联研究内容不显示。</p> : related.isError && !related.data ? <p className={c('study-restricted')}>关联研究读取失败：{errorText(related.error)}</p> : !relatedKnown ? <p className={c('study-restricted')}>关联研究来源状态未确认，暂不显示。</p> : <><p className={c('study-run-page-status')}>当前显示 {signals.length} 条当前有效信号{related.hasNextPage ? '，还有后续内容' : ''}。</p><div id="comment-related-signals">{signals.length ? signals.map(signal => <article className={c('study-related-signal')} key={signal.signalRef}><p><strong>{label(signalKindLabel, signal.kind) ?? signal.kind}</strong> · {signal.proposition || '归一表达未记录'}</p>{signal.evidence ? <blockquote>{signal.evidence}</blockquote> : null}<div className={c('study-link-actions')}>{signal.runRef ? <button type="button" className={c('study-link')} data-related-run={signal.runRef} data-related-target={signal.targetRef || ''} data-related-signal={signal.signalRef} onClick={() => nav.run(signal.runRef!, 'signals', signal.targetRef, signal.signalRef)}>查看本次信号</button> : null}{signal.resolvedProblemRef ? <button type="button" className={c('study-link')} data-related-problem={signal.resolvedProblemRef} onClick={() => nav.problem(signal.resolvedProblemRef!)}>查看用户问题</button> : null}</div></article>) : <p className={c('study-empty')}>当前没有有效研究信号或关联用户问题。</p>}</div>{related.hasNextPage ? <button type="button" id="comment-related-more" disabled={related.isFetchingNextPage} onClick={() => { void related.fetchNextPage().then(response => { if (response.data?.pages.at(-1)?.sourceState !== 'known') void detail.refetch(); }); }}>{related.isFetchingNextPage ? '正在读取…' : related.isFetchNextPageError ? `读取失败，重试：${errorText(related.error)}` : '加载更多关联信号'}</button> : null}</>}</Section>
    </>}
  </div></StudyDialog>;
}
