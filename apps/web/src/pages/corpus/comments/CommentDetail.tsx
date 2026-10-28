import type { ReactNode } from 'react';
import { parseCommentDetail } from './commentDtos';
import { shortBeijingDate } from './commentModel';
import { errorText, studyTime } from './format';
import { cleanStateLabel, workTitleSourceLabel } from './labels';
import { StudyDialog } from './StudyDialog';
import { CommentResearch } from './CommentResearch';
import { commentIdentity, useStudyRead, type CommentKey, type StudyNavigation } from './studyApi';
import { c } from './styles';

function Section({ title, children }: { title: string; children: ReactNode }) { return <section className={c('study-detail-section')}><h3>{title}</h3>{children}</section>; }
export function CommentDetail({ domain, commentKey, nav, close, filterWork, request }: { domain: string; commentKey: CommentKey; nav: StudyNavigation; close: () => void; filterWork: (key: string, label: string) => void; request?: string | null }) {
  const params = new URLSearchParams({ ...commentKey });
  const detail = useStudyRead(domain, `comments/detail?${params}`, row => { const data = parseCommentDetail(row); if (commentIdentity(data.commentKey) !== commentIdentity(commentKey)) throw new Error('读取回执与当前范围不一致，请重新读取。'); return data; });
  const data = detail.data, comment = data?.comment, work = data?.work, parent = data?.parentContext;
  const sourceReadable = data?.source.sourceState === 'known';
  const ready = !detail.isPending;
  const likeText = comment?.likeCount == null ? '点赞数未知' : `点赞 ${comment.likeCount.toLocaleString('zh-CN')}`;
  const observedText = comment?.observedAt ? `观测于 ${studyTime(comment.observedAt)}` : '观测时间未记录';
  const usageText = comment?.postUsage?.state === 'used' ? `已改成帖子${comment.postUsage.markedAt ? `（${studyTime(comment.postUsage.markedAt)} 标记）` : ''}` : null;
  const cleaningAddsInformation = Boolean(sourceReadable && comment && ((comment.researchText && comment.researchText !== comment.commentText) || comment.cleanReasons.length || (comment.cleanState != null && !['clean', 'direct'].includes(comment.cleanState))));
  return <StudyDialog id="comment-detail-dialog" className={c('study-detail-dialog')} titleId="comment-detail-title" onClose={close}><header className={c('study-dialog-header')}><div><p className={c('study-label')}>原声证据</p><h2 id="comment-detail-title">评论详情</h2></div><button type="button" id="comment-detail-close" onClick={close}>关闭</button></header><div id="comment-detail-body" className={c('study-detail-body')} aria-live="polite">
    {!ready ? <p className={c('study-empty')}>正在读取评论详情…</p> : !data ? <p className={c('study-empty')}>评论详情读取失败：{errorText(detail.error)}</p> : <>
      {sourceReadable && comment?.commentText ? <><blockquote>{comment.commentText}</blockquote><div className={c('study-detail-facts')}><span>{likeText}</span>{comment.likeCountAsOf ? <span>点赞采集 {shortBeijingDate(comment.likeCountAsOf)}</span> : null}<span>{observedText}</span>{usageText ? <span>{usageText}</span> : null}</div></> : <p className={c('study-restricted')}>当前原声不可显示：{data.source.displayState || data.source.sourceState || 'unknown'}</p>}
      <Section title="所属作品"><p><strong>{work?.displayTitle || comment?.workTitle || '未命名作品'}</strong> <small>{workTitleSourceLabel[work?.displayTitleSource || ''] || work?.displayTitleSource || '标题未记录'}</small></p><button type="button" className={c('study-link')} data-detail-work={commentKey.workRef} data-detail-work-title={work?.displayTitle || comment?.workTitle || ''} onClick={() => filterWork(commentKey.workRef, work?.displayTitle || comment?.workTitle || '')}>查看该作品的评论</button></Section>
      {sourceReadable && data.source.sourceRef && comment?.commentText && ['primary', 'reference'].includes(comment.observationRole) ? <CommentResearch key={request ?? data.source.sourceRef} domain={domain} work={commentKey.workRef} source={data.source.sourceRef} role={comment.observationRole} initialRequest={request} /> : null}
      {sourceReadable && parent?.commentText ? <Section title="父评论语境"><blockquote>{parent.commentText}</blockquote><p className={c('study-signal-meta')}>仅作为语境，不作为当前评论的独立证据。</p>{parent.commentKey ? <button type="button" className={c('study-link')} data-detail-parent-work={parent.commentKey.workRef} data-detail-parent-id={parent.commentKey.commentExternalId} onClick={() => nav.comment(parent.commentKey!)}>查看父评论详情</button> : null}</Section> : null}
      {cleaningAddsInformation && comment ? <Section title="清洗差异"><p>{comment.researchText || '清洗后文本未记录'}</p><p className={c('study-signal-meta')}>状态：{cleanStateLabel[comment.cleanState || ''] || comment.cleanState || '未记录'} · 原因：{comment.cleanReasons.join('、') || '未记录'}</p></Section> : null}

    </>}
  </div></StudyDialog>;
}
