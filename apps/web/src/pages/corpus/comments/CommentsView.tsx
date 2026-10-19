import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { useLocation, useNavigate } from 'react-router';
import { LikeCount, PostUsageControl } from './CommentCells';
import { CommentDetail } from './CommentDetail';
import { parseComments } from './commentDtos';
import { commentEligibility, commentFilters, commentStatus, shortBeijingDate, type CommentFilters } from './commentModel';
import { errorText, studyTime } from './format';
import { signalKindLabel, workTitleSourceLabel } from './labels';
import { parseWorkCatalog } from './methodDtos';
import type { SelectedComment } from './StartResearch';
import { commentIdentity, studyRead, useStudyRead, type StudyNavigation } from './studyApi';
import { c } from './styles';

export interface CommentCounts { displayableCommentCount: number | null; eligibleCommentCount: number | null; indexedCount: number | null; pendingCount: number | null }
interface Props { domain: string; nav: StudyNavigation; selected: SelectedComment[]; onSelected: (rows: SelectedComment[]) => void; onStudy: () => void; onStart: () => void; setupStatus: { text: string; kind: string }; onCounts: (counts: CommentCounts | null) => void; requestRefs: Map<string, string>; closeDetail: () => void }
export function CommentsView({ domain, nav, selected, onSelected, onStudy, onStart, setupStatus, onCounts, requestRefs, closeDetail }: Props) {
  const location = useLocation(), navigate = useNavigate(), params = new URLSearchParams(location.search);
  const state = location.state as { commentFilter?: CommentFilters; commentDomain?: string } | null;
  const filter = commentFilters(params, state?.commentDomain === domain ? state.commentFilter : null);
  const [feedback, setFeedback] = useState(''), [postUsageFeedback, setPostUsageFeedback] = useState(''), [workError, setWorkError] = useState('');
  const [advanced, setAdvanced] = useState(() => !!(filter.voiceRole !== 'reader_and_unknown' || filter.minLikes || filter.postUsage !== 'all' || filter.workRef));
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
  useEffect(() => {
    onCounts(!query.data || query.isError || query.isPending ? null : {
      displayableCommentCount: stats?.displayableCommentCount ?? null,
      eligibleCommentCount: stats?.eligibleCommentCount ?? null,
      indexedCount: coverage?.indexedCount ?? null,
      pendingCount: coverage?.pendingCount ?? null,
    });
  }, [query.data, query.isError, query.isPending, stats, coverage, onCounts]);
  const selectedIds = new Set(selected.map(item => commentIdentity(item.commentKey))), selectedWorks = new Set(selected.map(item => item.commentKey.workRef));
  async function searchWorks(form: HTMLFormElement) {
    const q = String(new FormData(form).get('workQuery') || '').trim(), serial = ++searchSerial.current, locationKey = location.key;
    try { const data = await studyRead(domain, `works?${new URLSearchParams({ q, limit: '20' })}`, parseWorkCatalog); if (!alive.current || serial !== searchSerial.current || currentLocation.current !== locationKey) return; route({ workLabel: q, workChoices: data.items }, true); setWorkError(''); }
    catch { if (alive.current && serial === searchSerial.current && currentLocation.current === locationKey) setWorkError('作品读取失败'); }
  }
  const detailWork = params.get('commentWorkRef') || (params.get('detail') === 'comment' ? params.get('workRef') : null), detailId = params.get('commentExternalId');
  const saved = async (message: string) => { setFeedback(message); if (filter.cursor) route({ cursor: null, history: [] }, true); else { const result = await query.refetch(); if (result.error) throw result.error; } };
  const detail = detailWork && detailId ? <CommentDetail key={commentIdentity({ workRef: detailWork, commentExternalId: detailId })} domain={domain} commentKey={{ workRef: detailWork, commentExternalId: detailId }} nav={nav} close={closeDetail} filterWork={(workRef, workLabel) => route({ workRef, workLabel, workChoices: [{ workRef, displayTitle: workLabel }], cursor: null, history: [] }, false, true)} /> : null;

  const extraCount = Number(filter.voiceRole !== 'reader_and_unknown') + Number(!!filter.minLikes) + Number(filter.postUsage !== 'all') + Number(!!filter.workRef);
  return <div className={c('study-results-comments')}>
    {query.isPending ? <div className={c('study-comment-read-status')}><p className={c('study-empty')}>正在读取评论研究…</p><button type="button" id="open-study-dialog" className={c('study-primary-action')} onClick={onStart}>发起研究</button></div> : query.isError ? <div className={c('study-comment-read-status')}><p className={c('study-empty')}>评论目录读取失败：{errorText(query.error)} <button type="button" onClick={() => { void query.refetch(); }}>重试</button></p><button type="button" id="open-study-dialog" className={c('study-primary-action')} onClick={onStart}>发起研究</button></div> :
    <section className={c('study-comments')} aria-label="用户评论">
      {feedback ? <p className={c('study-detail-muted')} role="status">{feedback}</p> : null}
      {setupStatus.text ? <p id="setup-status" className={c('study-setup-status')} role="status" data-kind={setupStatus.kind}>{setupStatus.text}</p> : null}
      {postUsageFeedback ? <p className={c('study-table-status')} role="status">{postUsageFeedback}</p> : null}
      <form key={queryParams.toString()} id="comment-filter-form" className={c('study-worktable-filters')} onSubmit={event => {
        event.preventDefault();
        const form = new FormData(event.currentTarget);
        route({ q: String(form.get('q') || '').trim(), studyState: String(form.get('studyState')), signalKind: String(form.get('signalKind') || '') || null, voiceRole: String(form.get('voiceRole')), sort: String(form.get('sort')), minLikes: String(form.get('minLikes') || '') || null, postUsage: String(form.get('postUsage')), cursor: null, history: [] });
      }}>
        <div className={c('study-worktable-filter-bar')}>
          <div className={c('study-worktable-filter-fields')}>
            <label className={c('study-filter-query')}>字面搜索<input id="comment-query" name="q" type="search" defaultValue={filter.q} placeholder="搜索评论文本" /></label>
            <label>研究状态<select id="comment-study-state" name="studyState" defaultValue={filter.studyState}>{[['all','全部'],['never_studied','从未研究'],['in_progress','处理中'],['studied','已有成功结果'],['needs_context','等待语境'],['failed','最近失败']].map(([value,label])=><option key={value} value={value}>{label}</option>)}</select></label>
            <label>研究标签<select id="comment-signal-kind" name="signalKind" defaultValue={filter.signalKind || ''}>{[['','全部标签'],...Object.entries(signalKindLabel)].map(([value,label])=><option value={value} key={value}>{label}</option>)}</select></label>
            <label>排序<select id="comment-sort" name="sort" defaultValue={filter.sort}><option value="like_desc">点赞最多</option><option value="received_desc">最新收到</option></select></label>
          </div>
          <div className={c('study-worktable-filter-actions')}>
          <button type="button" className={c('study-filter-more')} aria-controls="comment-advanced-filters" aria-expanded={advanced} onClick={() => setAdvanced(old => !old)}>更多筛选{extraCount ? ' · ' + extraCount : ''} <span aria-hidden="true">{advanced ? '⌃' : '⌄'}</span></button>
          <button type="submit" className={c('study-filter-submit')}>应用筛选</button>
          <button type="button" id="open-study-dialog" className={c('study-primary-action')} onClick={onStart}>发起研究</button>
          </div>
        </div>
        <div id="comment-advanced-filters" className={c('study-filter-advanced')} hidden={!advanced}>
          <label>声音角色<select id="comment-voice-role" name="voiceRole" defaultValue={filter.voiceRole}>{[['reader_and_unknown','用户评论（默认）'],['reader','已知用户'],['unknown','身份未知'],['creator','作品作者'],['all','全部声音']].map(([value,label])=><option value={value} key={value}>{label}</option>)}</select></label>
          <label>点赞<select id="comment-min-likes" name="minLikes" defaultValue={filter.minLikes || ''}>{[['','不限'],['30','30 以上'],['100','100 以上'],['1000','1000 以上']].map(([value,label])=><option value={value} key={value}>{label}</option>)}</select></label>
          <label>改帖<select id="comment-post-usage" name="postUsage" defaultValue={filter.postUsage}>{[['all','全部'],['unused','未改'],['used','已改']].map(([value,label])=><option value={value} key={value}>{label}</option>)}</select></label>
        </div>
      </form>
      <div className={c('study-worktable-work-filter')} hidden={!advanced}>
        <form key={filter.workLabel} id="comment-work-search-form" onSubmit={event => { event.preventDefault(); void searchWorks(event.currentTarget); }}>
          <label>所属作品<input id="comment-work-search" name="workQuery" type="search" defaultValue={filter.workLabel} placeholder="搜索作品标题" /></label><button type="submit">查找作品</button>
        </form>
        <label>已匹配作品<select id="comment-work-select" value={filter.workRef ?? ''} onChange={event => {
          const ref = event.target.value || null;
          route({ workRef: ref, workLabel: filter.workChoices.find(row => row.workRef === ref)?.displayTitle || '', cursor: null, history: [] });
        }}>{workError ? <option value="">{workError}</option> : <><option value="">全部作品</option>{filter.workChoices.map(work => <option value={work.workRef} key={work.workRef}>{work.displayTitle || '未命名作品'}</option>)}</>}</select></label>
      </div>
      {selected.length ? <div className={c('study-comment-selection')} aria-label="当前所选研究范围">
        <span id="comment-selection-count" role="status">已选 {selected.length} 条 · 来自 {selectedWorks.size} 篇作品</span>
        {selected.length >= 2700 || selectedWorks.size >= 90 ? <span className={c('study-selection-limit')} title="研究选择上限：3000 条评论，100 篇作品">接近上限：最多 3000 条 / 100 作品</span> : null}
        <button id="study-selected-comments" type="button" onClick={onStudy}>研究所选评论</button>
        <button type="button" className={c('study-selection-clear')} onClick={() => onSelected([])}>清空选择</button>
      </div> : null}
      <div className={c('study-review-table-wrap')}><table className={c('study-review-table study-comments-table')}>
        <thead><tr>{['选择','原声','点赞','所属作品','研究状态','研究标签','作者','点赞采集日','观测时间','改帖'].map(title=><th scope="col" key={title}>{title}</th>)}</tr></thead>
        <tbody>{rows.length ? rows.map(item => {
          const key = commentIdentity(item.commentKey), eligible = item.studyEligibility.eligible && ['primary','reference'].includes(item.observationRole), checked = selectedIds.has(key),
            capped = !checked && (selected.length >= 3000 || (!selectedWorks.has(item.commentKey.workRef) && selectedWorks.size >= 100)),
            kinds = item.signalKinds.filter(kind => Object.hasOwn(signalKindLabel, kind)), fullKinds = kinds.map(kind => signalKindLabel[kind]).join('、');
          const eligibilityText = eligible ? '' : commentEligibility(item), statusText = commentStatus(item);
          return <tr key={key} data-selected={checked ? 'true' : undefined}>
            <td><input type="checkbox" data-select-comment data-work-ref={item.commentKey.workRef} data-comment-id={item.commentKey.commentExternalId} data-eligible={eligible} aria-label="选择这条评论进入研究" title={!eligible ? eligibilityText : capped ? '本次选择已达到评论或作品数量上限' : undefined} checked={checked} disabled={!eligible || capped} onChange={event => {
              if (event.target.checked && eligible && !capped) onSelected([...selected, { commentKey: item.commentKey, observationRole: item.observationRole }]);
              else onSelected(selected.filter(row => commentIdentity(row.commentKey) !== key));
            }} /></td>
            <td><button type="button" className={c('study-comment-open')} aria-label={item.commentText || '评论原声不可用'} title={item.commentText || '评论原声不可用'} data-comment-work={item.commentKey.workRef} data-comment-id={item.commentKey.commentExternalId} onClick={() => nav.comment(item.commentKey)}>
              <span className={c('study-comment-voice')}>{item.commentText || '评论原声不可用'}</span>{!eligible ? <span className={c('study-row-limitation')} title={eligibilityText}>不可研</span> : null}
            </button></td>
            <td><LikeCount item={item} compact /></td>
            <td><span className={c('study-work-title')} title={item.workTitle ? (item.workTitleSource === 'platform_title' ? item.workTitle : item.workTitle + ' · ' + (workTitleSourceLabel[item.workTitleSource || ''] || '标题来源未记录')) : '未命名作品'}>{item.workTitle || '未命名作品'}</span></td>
            <td><span className={c('study-row-status')} title={statusText}>{statusText}</span></td>
            <td><div className={c('study-comment-kinds')} title={fullKinds || '无当前有效标签'}>{kinds.length ? kinds.map(kind => <span className={c('study-badge')} key={kind}>{signalKindLabel[kind]}</span>) : <span className={c('study-detail-muted')} aria-label="无当前有效标签">—</span>}</div></td>
            <td><span className={c('study-row-author')} title={item.authorDisplayName || '作者未记录'}>{item.authorDisplayName || '作者未记录'}</span></td>
            <td><time className={c('study-row-time')} title={item.likeCountAsOf ? '点赞采集时间：' + studyTime(item.likeCountAsOf) : '点赞采集时间未记录'}>{shortBeijingDate(item.likeCountAsOf) || '—'}</time></td>
            <td><time className={c('study-row-time')} title={item.observedAt ? '评论观测时间' : '评论观测时间未记录'}>{item.observedAt ? studyTime(item.observedAt) : '—'}</time></td>
            <td data-post-usage-cell><PostUsageControl item={item} domain={domain} requestRefs={requestRefs} onSaved={saved} onStatus={setPostUsageFeedback} compact /></td>
          </tr>;
        }) : <tr><td colSpan={10} className={c('study-table-empty')}>当前筛选没有可显示的用户评论。</td></tr>}</tbody>
      </table></div>
      <nav className={c('study-pagination')} aria-label="用户评论翻页"><button id="comment-prev" type="button" disabled={!filter.history.length} onClick={() => route({ cursor: filter.history.at(-1) ?? null, history: filter.history.slice(0, -1) })}>上一页</button><span>第 {filter.history.length + 1} 页</span><button id="comment-next" type="button" disabled={!query.data?.page.nextCursor} onClick={() => route({ history: [...filter.history, filter.cursor], cursor: query.data?.page.nextCursor ?? null })}>下一页</button></nav>
    </section>}{detail}
  </div>;
}
