import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { parsePostUsageReceipt, type CommentItem } from './commentDtos';
import { errorText, studyTime } from './format';
import { commentIdentity, studyPost } from './studyApi';
import { c } from './styles';

/** 改帖标记（B-04 `.mark-button`）：只在收到服务端回执后刷新列表；未确认的请求保留同一 requestRef 重试。 */
export function PostUsageControl({ item, domain, requestRefs, onSaved, onStatus }: { item: CommentItem; domain: string; requestRefs: Map<string, string>; onSaved: (feedback: string) => Promise<void>; onStatus: (status: string) => void }) {
  const [busy, setBusy] = useState(false);
  const guard = useRef(false), alive = useRef(true), savedCallback = useRef(onSaved);
  useLayoutEffect(() => { savedCallback.current = onSaved; }, [onSaved]);
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  const used = item.postUsage?.state === 'used', markedAt = item.postUsage?.markedAt ? studyTime(item.postUsage.markedAt) : '';
  async function save() {
    if (guard.current) return; guard.current = true; setBusy(true);
    const action = used ? 'unmark' : 'mark', identity = JSON.stringify([commentIdentity(item.commentKey), action]);
    const requestRef = requestRefs.get(identity) ?? crypto.randomUUID(); requestRefs.set(identity, requestRef);
    let saved = false;
    try {
      const data = await studyPost(domain, 'comments/post-usage', { domainRef: domain, ...item.commentKey, action, requestRef }, parsePostUsageReceipt);
      if (data.contract !== 'comment-post-usage.v1' || data.requestRef !== requestRef || commentIdentity(data.commentKey) !== commentIdentity(item.commentKey) || !['applied', 'unchanged', 'replayed'].includes(data.outcome) || !['used', 'unused'].includes(data.postUsage.state)) throw new Error('保存回执不符合约定，请重试确认。');
      saved = true; requestRefs.delete(identity);
      if (alive.current) await savedCallback.current(data.outcome === 'unchanged' ? '已确认原状态，没有新增标记事件。' : data.outcome === 'replayed' ? '已确认原请求回执，没有重复记录。' : action === 'mark' ? '已添加改帖标记。' : '已取消改帖标记。');
    } catch (error) {
      if (alive.current) onStatus(saved ? `改帖标记已保存，列表刷新失败：${errorText(error)}` : `保存未获确认：${errorText(error)}`);
    }
    finally { guard.current = false; if (alive.current) setBusy(false); }
  }
  return <button type="button" className={c('mark-button')} aria-pressed={used} aria-label={`${used ? '取消' : '添加'}改帖标记`}
    title={used ? `已改成帖子${markedAt ? ` · ${markedAt} 标记` : ''}` : '标记这条评论已改成帖子'}
    data-post-usage={used ? 'unmark' : 'mark'} data-post-usage-work={item.commentKey.workRef} data-post-usage-id={item.commentKey.commentExternalId}
    disabled={busy} onClick={() => { void save(); }}>{used ? '已标' : '标记'}</button>;
}
