import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { parsePostUsageReceipt, type CommentItem } from './commentDtos';
import { shortBeijingDate } from './commentModel';
import { errorText, studyTime } from './format';
import { commentIdentity, studyPost } from './studyApi';
import { c } from './styles';

export function LikeCount({ item, compact = false }: { item: CommentItem; compact?: boolean }) {
  if (item.likeCount == null) return <span className={c('study-like-count study-detail-muted')} title="点赞数未知：采集时没有取得这条评论的点赞">—</span>;
  const count = item.likeCount, exact = count.toLocaleString('zh-CN'), shown = count >= 10000 ? `${(count / 10000).toFixed(1).replace(/\.0$/, '')}万` : exact, asOf = shortBeijingDate(item.likeCountAsOf);
  return <><span className={c('study-like-count')} {...(count >= 10000 ? { title: `精确值 ${exact}` } : {})}>{shown}</span>{!compact && asOf ? <small>采集于 <span className={c('study-like-date')}>{asOf}</span></small> : null}</>;
}
export function PostUsageControl({ item, domain, requestRefs, onSaved, compact = false }: { item: CommentItem; domain: string; requestRefs: Map<string, string>; onSaved: (feedback: string) => Promise<void>; compact?: boolean }) {
  const [busy, setBusy] = useState(false), [status, setStatus] = useState('');
  const guard = useRef(false), alive = useRef(true), savedCallback = useRef(onSaved);
  useLayoutEffect(() => { savedCallback.current = onSaved; }, [onSaved]);
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  const used = item.postUsage?.state === 'used', markedAt = item.postUsage?.markedAt ? studyTime(item.postUsage.markedAt) : '';
  async function save() {
    if (guard.current) return; guard.current = true; setBusy(true); setStatus('正在保存…');
    const action = used ? 'unmark' : 'mark', identity = JSON.stringify([commentIdentity(item.commentKey), action]);
    const requestRef = requestRefs.get(identity) ?? crypto.randomUUID(); requestRefs.set(identity, requestRef);
    let saved = false;
    try {
      const data = await studyPost(domain, 'comments/post-usage', { domainRef: domain, ...item.commentKey, action, requestRef }, parsePostUsageReceipt);
      if (data.contract !== 'comment-post-usage.v1' || data.requestRef !== requestRef || commentIdentity(data.commentKey) !== commentIdentity(item.commentKey) || !['applied', 'unchanged', 'replayed'].includes(data.outcome) || !['used', 'unused'].includes(data.postUsage.state)) throw new Error('保存回执不符合约定，请重试确认。');
      saved = true; requestRefs.delete(identity);
      if (alive.current) await savedCallback.current(data.outcome === 'unchanged' ? '已确认原状态，没有新增标记事件。' : data.outcome === 'replayed' ? '已确认原请求回执，没有重复记录。' : '改帖标记已保存。');
    } catch (error) { if (alive.current) setStatus(saved ? `改帖标记已保存，列表刷新失败：${errorText(error)}` : `保存未获确认：${errorText(error)}`); }
    finally { guard.current = false; if (alive.current) setBusy(false); }
  }
  return <div className={c('study-post-usage')} title={compact && used && markedAt ? `已改成帖子 · ${markedAt} 标记` : undefined}>{used && !compact ? <><span className={c('study-badge')}>已改成帖子</span>{markedAt ? <small>{markedAt} 标记</small> : null}</> : null}<button type="button" className={c('study-link')} data-post-usage={used ? 'unmark' : 'mark'} data-post-usage-work={item.commentKey.workRef} data-post-usage-id={item.commentKey.commentExternalId} disabled={busy} onClick={() => { void save(); }}>{used ? '撤销标记' : '标记已改成帖子'}</button><small className={c('study-post-usage-status')} role="status" title={status || undefined}>{status}</small></div>;
}
