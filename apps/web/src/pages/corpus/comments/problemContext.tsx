import { useEffect, useRef, useState } from 'react';
import { parseSourceContext } from './commentSourceDto';
import { errorText } from './format';
import { useStudyRead, type CommentKey } from './studyApi';
import { c } from './styles';

export function ProblemContext({ domain, commentKey, sourceUnavailable }: { domain: string; commentKey: CommentKey; sourceUnavailable: () => void }) {
  const [open, setOpen] = useState(false);
  return <><div className={c('sr3-evidence-bottom')}><span>作品 {commentKey.workRef.slice(0, 12)}</span><button type="button" className={c('sr3-button')} data-sr3="context" aria-expanded={open} onClick={() => setOpen(value => !value)}>展开作品与父语境</button></div><div className={c('sr3-context')} hidden={!open}>{open ? <ContextBody domain={domain} commentKey={commentKey} sourceUnavailable={sourceUnavailable} /> : null}</div></>;
}
function ContextBody({ domain, commentKey, sourceUnavailable }: { domain: string; commentKey: CommentKey; sourceUnavailable: () => void }) {
  const notified = useRef(false);
  const params = new URLSearchParams({ ...commentKey });
  const query = useStudyRead(domain, `comments/detail?${params}`, row => {
    const value = parseSourceContext(row);
    if (value.commentKey.workRef !== commentKey.workRef || value.commentKey.commentExternalId !== commentKey.commentExternalId) throw new Error('读取回执与当前范围不一致，请重新读取。');
    return value;
  });
  useEffect(() => { if (query.data && !query.isFetching && query.data.sourceState !== 'known' && !notified.current) { notified.current = true; sourceUnavailable(); } }, [query.data, query.isFetching, sourceUnavailable]);
  return query.isFetching ? <>正在核对原声与语境…</> : query.isError ? <>{errorText(query.error)} 收起后可重试。</> : query.data?.sourceState !== 'known' ? <>原声当前不可读取，正在重新核对问题及引用。</> : <><span className={c('sr3-caption')}>所属作品</span><h4>{query.data.workTitle || '作品标题未记录'}</h4><p className={c('sr3-caption')}>此处只取得作品标题，不把标题推断成作品完整观点。</p><span className={c('sr3-caption')}>父评论语境</span>{query.data.parentText?.trim() ? <blockquote>{query.data.parentText}</blockquote> : <p>没有可显示的父评论语境；不推断为主评论。</p>}<p className={c('sr3-caption')}>父评论只用于理解这条表达，不计为它的独立支持依据。</p></>;
}
