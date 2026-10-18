import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { useLocation, useNavigate } from 'react-router';
import { LikeCount, PostUsageControl } from './CommentCells';
import { CommentDetail } from './CommentDetail';
import { parseComments } from './commentDtos';
import { commentEligibility, commentFilters, commentStatus, type CommentFilters } from './commentModel';
import { errorText, studyTime } from './format';
import { signalKindLabel, workTitleSourceLabel } from './labels';
import { parseWorkCatalog } from './methodDtos';
import type { SelectedComment } from './StartResearch';
import { commentIdentity, studyRead, useStudyRead, type StudyNavigation } from './studyApi';
import { c } from './styles';

interface Props { domain: string; nav: StudyNavigation; selected: SelectedComment[]; onSelected: (rows: SelectedComment[]) => void; onStudy: () => void; requestRefs: Map<string, string>; closeDetail: () => void }
export function CommentsView({ domain, nav, selected, onSelected, onStudy, requestRefs, closeDetail }: Props) {
  const location = useLocation(), navigate = useNavigate(), params = new URLSearchParams(location.search);
  const state = location.state as { commentFilter?: CommentFilters; commentDomain?: string } | null;
  const filter = commentFilters(params, state?.commentDomain === domain ? state.commentFilter : null);
  const [feedback, setFeedback] = useState(''), [workError, setWorkError] = useState('');
  const alive = useRef(true), searchSerial = useRef(0), currentLocation = useRef(location.key);
  useLayoutEffect(() => { currentLocation.current = location.key; }, [location.key]);
  useEffect(() => { alive.current = true; return () => { alive.current = false; searchSerial.current += 1; }; }, []);
  function route(changes: Partial<CommentFilters>, replace = false, clearDetail = false) {
    const next = { ...filter, ...changes }, url = new URL(location.pathname + location.search, window.location.origin);
    url.searchParams.set('view', 'comments'); url.searchParams.set('domain', domain);
    const fields = [['q', next.q], ['workRef', next.workRef], ['state', next.studyState === 'all' ? null : next.studyState], ['signalKind', next.signalKind], ['cursor', next.cursor], ['voiceRole', next.voiceRole === 'reader_and_unknown' ? null : next.voiceRole], ['sort', next.sort === 'like_desc' ? null : next.sort], ['minLikes', next.minLikes], ['postUsage', next.postUsage === 'all' ? null : next.postUsage]] as const;
    for (const [key, value] of fields) { if (value) url.searchParams.set(key, value); else url.searchParams.delete(key); }
    for (const key of ['problemRef', 'runRef', 'panel', 'targetRef', 'signalRef', ...(clearDetail ? ['commentWorkRef', 'commentExternalId', 'detail'] : [])]) url.searchParams.delete(key);
    void navigate(url.pathname + url.search, { replace, state: { ...state, commentDomain: domain, commentFilter: next, ...(clearDetail ? { commentDetail: false } : {}) } });
  }
  const queryParams = new URLSearchParams({ q: filter.q, studyState: filter.studyState, voiceRole: filter.voiceRole, sort: filter.sort, postUsage: filter.postUsage, limit: '50' });
  for (const [key, value] of [['workRef', filter.workRef], ['signalKind', filter.signalKind], ['minLikes', filter.minLikes], ['cursor', filter.cursor]] as const) if (value) queryParams.set(key, value);
  const query = useStudyRead(domain, `comments?${queryParams}`, parseComments);
  const rows = query.data?.items ?? [], stats = query.data?.summary, coverage = query.data?.indexCoverage;
  const selectedIds = new Set(selected.map(item => commentIdentity(item.commentKey))), selectedWorks = new Set(selected.map(item => item.commentKey.workRef));
  async function searchWorks(form: HTMLFormElement) {
    const q = String(new FormData(form).get('workQuery') || '').trim(), serial = ++searchSerial.current, locationKey = location.key;
    try { const data = await studyRead(domain, `works?${new URLSearchParams({ q, limit: '20' })}`, parseWorkCatalog); if (!alive.current || serial !== searchSerial.current || currentLocation.current !== locationKey) return; route({ workLabel: q, workChoices: data.items }, true); setWorkError(''); }
    catch { if (alive.current && serial === searchSerial.current && currentLocation.current === locationKey) setWorkError('作品读取失败'); }
  }
  const detailWork = params.get('commentWorkRef') || (params.get('detail') === 'comment' ? params.get('workRef') : null), detailId = params.get('commentExternalId');
  const saved = async (message: string) => { setFeedback(message); if (filter.cursor) route({ cursor: null, history: [] }, true); else { const result = await query.refetch(); if (result.error) throw result.error; } };
  const detail = detailWork && detailId ? <CommentDetail key={commentIdentity({ workRef: detailWork, commentExternalId: detailId })} domain={domain} commentKey={{ workRef: detailWork, commentExternalId: detailId }} nav={nav} close={closeDetail} filterWork={(workRef, workLabel) => route({ workRef, workLabel, workChoices: [{ workRef, displayTitle: workLabel }], cursor: null, history: [] }, false, true)} /> : null;
  return <div className={c('study-results-comments')}>
    {query.isPending ? <p className={c('study-empty')}>正在读取评论研究…</p> : query.isError ? <p className={c('study-empty')}>评论目录读取失败：{errorText(query.error)} <button type="button" onClick={() => { void query.refetch(); }}>重试</button></p> : <section className={c('study-comments')} aria-labelledby="comments-title">
      {feedback ? <p className={c('study-detail-muted')} role="status">{feedback}</p> : null}<header className={c('study-comments-head')}><div><p className={c('study-label')}>评论证据库</p><h2 id="comments-title">用户评论</h2><p>当前筛选可显示 {Number(stats?.displayableCommentCount || 0)} 条；可研究 {Number(stats?.eligibleCommentCount || 0)} 条；已索引 {Number(coverage?.indexedCount || 0)}，待索引 {coverage?.pendingCount == null ? '未知' : coverage.pendingCount}。</p></div><div className={c('study-comment-selection')}><span id="comment-selection-count" role="status">已选 {selected.length}/3000 条评论，来自 {selectedWorks.size}/100 篇作品；本页可继续选择。</span><button id="study-selected-comments" type="button" disabled={!selected.length} onClick={onStudy}>研究所选评论</button></div></header>
      <form key={JSON.stringify(queryParams.toString())} id="comment-filter-form" className={c('study-comment-filters')} onSubmit={event => { event.preventDefault(); const form = new FormData(event.currentTarget); route({ q: String(form.get('q') || '').trim(), studyState: String(form.get('studyState')), signalKind: String(form.get('signalKind') || '') || null, voiceRole: String(form.get('voiceRole')), sort: String(form.get('sort')), minLikes: String(form.get('minLikes') || '') || null, postUsage: String(form.get('postUsage')), cursor: null, history: [] }); }}>
        <label>字面搜索<input id="comment-query" name="q" type="search" defaultValue={filter.q} placeholder="搜索清洗后的评论文本" /></label>
        {([
          ['studyState', 'comment-study-state', '研究状态', filter.studyState, [['all', '全部'], ['never_studied', '从未研究'], ['in_progress', '处理中'], ['studied', '已有成功结果'], ['needs_context', '等待语境'], ['failed', '最近失败']]],
          ['signalKind', 'comment-signal-kind', '研究标签', filter.signalKind || '', [['', '全部标签'], ...Object.entries(signalKindLabel)]],
          ['voiceRole', 'comment-voice-role', '声音角色', filter.voiceRole, [['reader_and_unknown', '用户评论（默认）'], ['reader', '已知用户'], ['unknown', '身份未知'], ['creator', '作品作者'], ['all', '全部声音']]],
          ['sort', 'comment-sort', '排序', filter.sort, [['received_desc', '最新收到'], ['like_desc', '点赞最多']]],
          ['minLikes', 'comment-min-likes', '点赞', filter.minLikes || '', [['', '不限'], ['30', '30 以上'], ['100', '100 以上'], ['1000', '1000 以上']]],
          ['postUsage', 'comment-post-usage', '改帖', filter.postUsage, [['all', '全部'], ['unused', '未改'], ['used', '已改']]],
        ] as const).map(([name, id, title, value, choices]) => <label key={name}>{title}<select id={id} name={name} defaultValue={value}>{choices.map(([key, label]) => <option value={key} key={key}>{label}</option>)}</select></label>)}<button type="submit">应用筛选</button>
      </form>
      <div className={c('study-comment-work-filter')}><form key={filter.workLabel} id="comment-work-search-form" onSubmit={event => { event.preventDefault(); void searchWorks(event.currentTarget); }}><label>所属作品<input id="comment-work-search" name="workQuery" type="search" defaultValue={filter.workLabel} placeholder="搜索作品标题" /></label><button type="submit">查找作品</button></form><label>已匹配作品<select id="comment-work-select" value={filter.workRef ?? ''} onChange={event => { const ref = event.target.value || null; route({ workRef: ref, workLabel: filter.workChoices.find(row => row.workRef === ref)?.displayTitle || '', cursor: null, history: [] }); }}>{workError ? <option value="">{workError}</option> : <><option value="">全部作品</option>{filter.workChoices.map(work => <option value={work.workRef} key={work.workRef}>{work.displayTitle || '未命名作品'}</option>)}</>}</select></label></div>
      <div className={c('study-review-table-wrap')}><table className={c('study-review-table study-comments-table')}><thead><tr>{['选择', '原声', '点赞', '所属作品', '研究状态', '研究标签', '作者 / 观测', '改帖'].map(title => <th scope="col" key={title}>{title}</th>)}</tr></thead><tbody>{rows.length ? rows.map(item => {
        const key = commentIdentity(item.commentKey), eligible = item.studyEligibility.eligible && ['primary', 'reference'].includes(item.observationRole), checked = selectedIds.has(key), capped = !checked && (selected.length >= 3000 || !selectedWorks.has(item.commentKey.workRef) && selectedWorks.size >= 100), kinds = item.signalKinds.filter(kind => Object.hasOwn(signalKindLabel, kind));
        return <tr key={key}><td><input type="checkbox" data-select-comment data-work-ref={item.commentKey.workRef} data-comment-id={item.commentKey.commentExternalId} data-eligible={eligible} aria-label="选择这条评论进入研究" checked={checked} disabled={!eligible || capped} onChange={event => { if (event.target.checked && eligible && !capped) onSelected([...selected, { commentKey: item.commentKey, observationRole: item.observationRole }]); else onSelected(selected.filter(row => commentIdentity(row.commentKey) !== key)); }} /></td><td><button type="button" className={c('study-comment-open')} data-comment-work={item.commentKey.workRef} data-comment-id={item.commentKey.commentExternalId} onClick={() => nav.comment(item.commentKey)}><span className={c('study-comment-voice')}>{item.commentText}</span></button>{item.studyEligibility.eligible ? null : <small>{commentEligibility(item)}</small>}</td><td><LikeCount item={item} /></td><td><span className={c('study-work-title')}>{item.workTitle || '未命名作品'}</span>{item.workTitleSource === 'platform_title' ? null : <small>{workTitleSourceLabel[item.workTitleSource || ''] || item.workTitleSource || '标题未记录'}</small>}</td><td>{commentStatus(item)}</td><td><div className={c('study-comment-kinds')}>{kinds.length ? kinds.map(kind => <span className={c('study-badge')} key={kind}>{signalKindLabel[kind]}</span>) : <span className={c('study-detail-muted')}><span aria-hidden="true">—</span><span className="v7-sr-only">无当前有效标签</span></span>}</div></td><td>{item.authorDisplayName || '作者未记录'}<small>{studyTime(item.observedAt)}</small></td><td data-post-usage-cell><PostUsageControl item={item} domain={domain} requestRefs={requestRefs} onSaved={saved} /></td></tr>;
      }) : <tr><td colSpan={8} className={c('study-table-empty')}>当前筛选没有可显示的用户评论。</td></tr>}</tbody></table></div>
      <nav className={c('study-pagination')} aria-label="用户评论翻页"><button id="comment-prev" type="button" disabled={!filter.history.length} onClick={() => route({ cursor: filter.history.at(-1) ?? null, history: filter.history.slice(0, -1) })}>上一页</button><span>第 {filter.history.length + 1} 页</span><button id="comment-next" type="button" disabled={!query.data?.page.nextCursor} onClick={() => route({ history: [...filter.history, filter.cursor], cursor: query.data?.page.nextCursor ?? null })}>下一页</button></nav>
    </section>}{detail}
  </div>;
}
