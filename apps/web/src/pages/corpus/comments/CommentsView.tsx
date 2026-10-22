import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { useLocation, useNavigate } from 'react-router';
import { LikeCount, PostUsageControl } from './CommentCells';
import { ActionButton } from '../../../shared/ActionButton';
import { SelectField } from '../../../shared/SelectField';
import { CommentDetail } from './CommentDetail';
import { parseComments } from './commentDtos';
import { commentEligibility, commentFilters, commentStatus, shortBeijingDate, type CommentFilters } from './commentModel';
import { errorText, studyTime } from './format';
import { signalKindLabel, workTitleSourceLabel } from './labels';
import { parseWorkCatalog } from './methodDtos';
import type { SelectedComment } from './studyApi';
import { commentIdentity, studyRead, useStudyRead, type StudyNavigation } from './studyApi';
import { c } from './styles';

export interface CommentCounts { displayableCommentCount: number | null; eligibleCommentCount: number | null; indexedCount: number | null; pendingCount: number | null }
interface Props { domain: string; nav: StudyNavigation; selected: SelectedComment[]; onSelected: (rows: SelectedComment[]) => void; onCounts: (counts: CommentCounts | null) => void; requestRefs: Map<string, string>; closeDetail: () => void }
export function CommentsView({ domain, nav, selected, onSelected, onCounts, requestRefs, closeDetail }: Props) {
  const location = useLocation(), navigate = useNavigate(), params = new URLSearchParams(location.search);
  const state = location.state as { commentFilter?: CommentFilters; commentDomain?: string } | null;
  const filter = useMemo(() => commentFilters(new URLSearchParams(location.search), state?.commentDomain === domain ? state.commentFilter : null), [domain, location.search, state]);
  const [feedback, setFeedback] = useState(''), [postUsageFeedback, setPostUsageFeedback] = useState(''), [workError, setWorkError] = useState('');
  const [advanced, setAdvanced] = useState(() => !!(filter.voiceRole !== 'reader_and_unknown' || filter.minLikes || filter.postUsage !== 'all' || filter.workRef));
  const [draft, setDraft] = useState(filter);
  const [previousFilter, setPreviousFilter] = useState(filter);
  const [selecting, setSelecting] = useState(selected.length > 0);
  // A router change replaces the submitted draft; local SelectField edits never auto-query.
  if (previousFilter !== filter) {
    setPreviousFilter(filter);
    setDraft(filter);
  }
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

  const extraCount = Number(draft.voiceRole !== 'reader_and_unknown') + Number(!!draft.minLikes) + Number(draft.postUsage !== 'all') + Number(!!filter.workRef);
  const loadedEligible = rows.filter(item => item.studyEligibility.eligible && ['primary','reference'].includes(item.observationRole));
  const allPageSelected = loadedEligible.length > 0 && loadedEligible.every(item => selectedIds.has(commentIdentity(item.commentKey)));
  function togglePage() {
    const pageKeys = new Set(loadedEligible.map(item => commentIdentity(item.commentKey)));
    if (allPageSelected) { onSelected(selected.filter(item => !pageKeys.has(commentIdentity(item.commentKey)))); return; }
    const next = [...selected], works = new Set(selectedWorks), ids = new Set(selectedIds);
    for (const item of loadedEligible) {
      const key = commentIdentity(item.commentKey);
      if (ids.has(key)) continue;
      if (next.length >= 3000) break;
      if (!works.has(item.commentKey.workRef) && works.size >= 100) continue;
      next.push({ commentKey: item.commentKey, observationRole: item.observationRole });
      ids.add(key); works.add(item.commentKey.workRef);
    }
    onSelected(next);
  }
  return <div className={c('study-results-comments')}>
    {query.isPending ? <div className={c('study-comment-read-status')}><p className={c('study-empty')}>正在读取评论研究…</p></div> : query.isError ? <div className={c('study-comment-read-status')}><p className={c('study-empty')}>评论目录读取失败：{errorText(query.error)} <button type="button" onClick={() => { void query.refetch(); }}>重试</button></p></div> :
    <section className={c('study-comments')} aria-label="用户评论">
      {feedback ? <p className={c('study-detail-muted')} role="status">{feedback}</p> : null}
      {postUsageFeedback ? <p className={c('study-table-status')} role="status">{postUsageFeedback}</p> : null}
      <form key={queryParams.toString()} id="comment-filter-form" className={c('study-worktable-filters')} onSubmit={event => {
        event.preventDefault();
        const form = new FormData(event.currentTarget);
        route({ q: String(form.get('q') || '').trim(), studyState: draft.studyState, signalKind: draft.signalKind,
          voiceRole: draft.voiceRole, sort: draft.sort, minLikes: draft.minLikes, postUsage: draft.postUsage,
          cursor: null, history: [] });
      }}>
        <div className={c('study-worktable-filter-bar')}>
          <div className={c('study-worktable-filter-fields')}>
            <label className={c('study-filter-query')}><span className={c('study-filter-title')}>字面搜索</span><span className={c('study-filter-search-shell')}><b aria-hidden="true">FIND</b><input id="comment-query" name="q" type="search" aria-label="字面搜索" defaultValue={filter.q} placeholder="搜索评论文本" autoComplete="off" /></span></label>
            <div className={c('study-filter-control')}><span>研究状态</span><SelectField label="研究状态" value={draft.studyState} onChange={studyState => setDraft(old => ({ ...old, studyState }))} options={[
              {value:'all',label:'全部'}, {value:'never_studied',label:'从未研究'}, {value:'in_progress',label:'处理中'},
              {value:'studied',label:'已有成功结果'}, {value:'needs_context',label:'等待语境'}, {value:'failed',label:'最近失败'},
            ]} /></div>
            <div className={c('study-filter-control')}><span>研究标签</span><SelectField label="研究标签" value={draft.signalKind || ''} onChange={signalKind => setDraft(old => ({ ...old, signalKind: signalKind || null }))} options={[{value:'',label:'全部标签'},...Object.entries(signalKindLabel).map(([value,label])=>({value,label:label || value}))]} /></div>
            <div className={c('study-filter-control')}><span>排序</span><SelectField label="排序" value={draft.sort} onChange={sort => setDraft(old => ({ ...old, sort }))} options={[{value:'like_desc',label:'点赞最多'},{value:'received_desc',label:'最新收到'}]} /></div>
          </div>
          <div className={c('study-worktable-filter-actions')}>
            <ActionButton variant="quiet" type="button" aria-controls="comment-advanced-filters" aria-expanded={advanced} onClick={() => setAdvanced(old => !old)}>更多筛选{extraCount ? ' · ' + extraCount : ''}</ActionButton>
            <ActionButton variant="outline" type="submit">应用筛选</ActionButton>
          </div>
        </div>
        <div id="comment-advanced-filters" className={c('study-filter-advanced')} hidden={!advanced}>
          <div className={c('study-filter-control')}><span>声音角色</span><SelectField label="声音角色" value={draft.voiceRole} onChange={voiceRole => setDraft(old => ({ ...old, voiceRole }))} options={[
            {value:'reader_and_unknown',label:'用户评论（默认）'}, {value:'reader',label:'已知用户'}, {value:'unknown',label:'身份未知'}, {value:'creator',label:'作品作者'}, {value:'all',label:'全部声音'},
          ]} /></div>
          <div className={c('study-filter-control')}><span>点赞</span><SelectField label="点赞" value={draft.minLikes || ''} onChange={minLikes => setDraft(old => ({ ...old, minLikes: minLikes || null }))} options={[
            {value:'',label:'不限'},{value:'30',label:'30 以上'},{value:'100',label:'100 以上'},{value:'1000',label:'1000 以上'},
          ]} /></div>
          <div className={c('study-filter-control')}><span>改帖</span><SelectField label="改帖" value={draft.postUsage} onChange={postUsage => setDraft(old => ({ ...old, postUsage }))} options={[
            {value:'all',label:'全部'},{value:'unused',label:'未改'},{value:'used',label:'已改'},
          ]} /></div>
        </div>
      </form>
      <div className={c('study-worktable-work-filter')} hidden={!advanced}>
        <form key={filter.workLabel} id="comment-work-search-form" onSubmit={event => { event.preventDefault(); void searchWorks(event.currentTarget); }}>
          <label>所属作品<input id="comment-work-search" name="workQuery" type="search" defaultValue={filter.workLabel} placeholder="搜索作品标题" /></label><button type="submit">查找作品</button>
        </form>
        <div className={c('study-filter-control')}><span>已匹配作品</span><SelectField label="已匹配作品" value={filter.workRef || ''} onChange={value => {
          const ref = value || null;
          route({ workRef: ref, workLabel: filter.workChoices.find(row => row.workRef === ref)?.displayTitle || '', cursor: null, history: [] });
        }} options={workError ? [{value:'',label:workError,disabled:true}] : [{value:'',label:'全部作品'},...filter.workChoices.map(work => ({value:work.workRef,label:work.displayTitle || '未命名作品'}))]} /></div>
      </div>
      <div className={c('study-comment-result-bar')}>
        <span>{stats?.displayableCommentCount == null ? '评论范围待读取' : `当前筛选 ${stats.displayableCommentCount.toLocaleString('zh-CN')} 条评论`}</span>
        <ActionButton variant="outline" type="button" aria-pressed={selecting} onClick={() => {
          if (selecting) onSelected([]);
          setSelecting(old => !old);
        }}>{selecting ? '完成选择' : '选择评论'}</ActionButton>
      </div>
      {selecting ? <div className={c('study-comment-selection')} aria-label="当前所选评论">
        <span id="comment-selection-count" role="status">已选 {selected.length} 条 · 来自 {selectedWorks.size} 篇作品</span>
        <ActionButton variant="quiet" disabled={!loadedEligible.length} onClick={togglePage}>{allPageSelected ? '取消本页选择' : '全选当前页'}</ActionButton>
        {selected.length >= 2700 || selectedWorks.size >= 90 ? <span className={c('study-selection-limit')} title="评论选择上限：3000 条评论，100 篇作品">接近上限：最多 3000 条 / 100 作品</span> : null}
        <ActionButton variant="quiet" disabled={!selected.length} onClick={() => onSelected([])}>清空选择</ActionButton>
      </div> : null}
      <div className={c('study-review-table-wrap')}><table className={c('study-review-table study-comments-table')} data-selecting={selecting}>
        <colgroup>{selecting ? <col className={c('study-col-check')} /> : null}<col className={c('study-col-voice')} /><col className={c('study-col-likes')} /><col className={c('study-col-work')} /><col className={c('study-col-status')} /><col className={c('study-col-kinds')} /><col className={c('study-col-author')} /><col className={c('study-col-day')} /><col className={c('study-col-time')} /><col className={c('study-col-usage')} /></colgroup>
        <thead><tr>{(selecting ? ['选择'] : []).concat(['原声','点赞','所属作品','研究状态','研究标签','作者','点赞采集日','观测时间','改帖']).map(title=><th scope="col" key={title}>{title}</th>)}</tr></thead>
        <tbody>{rows.length ? rows.map(item => {
          const key = commentIdentity(item.commentKey), eligible = item.studyEligibility.eligible && ['primary','reference'].includes(item.observationRole), checked = selectedIds.has(key),
            capped = !checked && (selected.length >= 3000 || (!selectedWorks.has(item.commentKey.workRef) && selectedWorks.size >= 100)),
            kinds = item.signalKinds.filter(kind => Object.hasOwn(signalKindLabel, kind)), fullKinds = kinds.map(kind => signalKindLabel[kind]).join('、');
          const eligibilityText = eligible ? '' : commentEligibility(item), statusText = commentStatus(item);
          return <tr key={key} data-selected={checked ? 'true' : undefined}>
            {selecting ? <td className={c('study-comment-check-cell')}><label className={c('study-comment-check')}><input type="checkbox" data-select-comment data-work-ref={item.commentKey.workRef} data-comment-id={item.commentKey.commentExternalId} data-eligible={eligible} aria-label="选择这条评论" title={!eligible ? eligibilityText : capped ? '本次选择已达到评论或作品数量上限' : undefined} checked={checked} disabled={!eligible || capped} onChange={event => {
              if (event.target.checked && eligible && !capped) onSelected([...selected, { commentKey: item.commentKey, observationRole: item.observationRole }]);
              else onSelected(selected.filter(row => commentIdentity(row.commentKey) !== key));
            }} /></label></td> : null}
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
        }) : <tr><td colSpan={selecting ? 10 : 9} className={c('study-table-empty')}>当前筛选没有可显示的用户评论。</td></tr>}</tbody>
      </table></div>
      <nav className={c('study-pagination')} aria-label="用户评论翻页"><ActionButton variant="outline" id="comment-prev" disabled={!filter.history.length} onClick={() => route({ cursor: filter.history.at(-1) ?? null, history: filter.history.slice(0, -1) })}>上一页</ActionButton><span>第 {filter.history.length + 1} 页</span><ActionButton variant="outline" id="comment-next" disabled={!query.data?.page.nextCursor} onClick={() => route({ history: [...filter.history, filter.cursor], cursor: query.data?.page.nextCursor ?? null })}>下一页</ActionButton></nav>
    </section>}{detail}
  </div>;
}
