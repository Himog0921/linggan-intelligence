import { useQuery } from '@tanstack/react-query';
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { useLocation, useNavigate } from 'react-router';
import { ActionButton } from '../../../shared/ActionButton';
import { SelectField } from '../../../shared/SelectField';
import { PostUsageControl } from './CommentCells';
import { parseComments, type CommentItem } from './commentDtos';
import { commentEligibility, commentFilters, commentStatus, commentStatusInfo, PAGE_SIZES, shortBeijingDate, type CommentFilters, type PageSize } from './commentModel';
import { CrIcon } from './CrIcon';
import { errorText, num, studyTime } from './format';
import { signalKindLabel, workTitleSourceLabel } from './labels';
import { parseWorkCatalog } from './methodDtos';
import { commentIdentity, studyRead, useStudyRead, type SelectedComment, type StudyNavigation } from './studyApi';
import { previewSelection, readProblem, type PreviewPlan } from './understandingApi';
import { c } from './styles';

const BULK_SIZES = ['100', '300', '500', '1000'] as const;
type BulkSize = typeof BULK_SIZES[number];
const COLUMNS = [26, 6, 17, 12, 8, 7, 9, 10, 5];
const MAX_SELECTED = 50, MAX_WORKS = 100;
const STATE_OPTIONS = [
  { value: 'all', label: '全部状态' }, { value: 'never_studied', label: '尚未研究' }, { value: 'in_progress', label: '进行中' },
  { value: 'studied', label: '已有结果' }, { value: 'needs_context', label: '需要语境' }, { value: 'failed', label: '最近失败' },
];
const TAG_OPTIONS = [{ value: '', label: '全部标签' }, ...Object.entries(signalKindLabel).map(([value, label]) => ({ value, label: label || value }))];
const SORT_OPTIONS = [{ value: 'like_desc', label: '点赞最多' }, { value: 'received_desc', label: '最新收到' }];
const VOICE_OPTIONS = [{ value: 'reader_and_unknown', label: '用户评论（默认）' }, { value: 'reader', label: '已知用户' }, { value: 'unknown', label: '身份未知' }, { value: 'creator', label: '作品作者' }, { value: 'all', label: '全部声音' }];
const USAGE_OPTIONS = [{ value: 'all', label: '全部评论' }, { value: 'used', label: '已标记' }, { value: 'unused', label: '未标记' }];

const canSelect = (item: CommentItem) => item.studyEligibility.eligible && ['primary', 'reference'].includes(item.observationRole);
const selectionFor = (item: CommentItem): SelectedComment => ({ sourceRef: item.sourceRef, commentKey: item.commentKey, observationRole: item.observationRole, text: item.commentText, workTitle: item.workTitle });
const slashDay = (iso: string | null) => shortBeijingDate(iso).replaceAll('-', '/');
const monthTime = (iso: string | null) => iso ? studyTime(iso).slice(5).replace('-', '/') : '—';

interface Props { domain: string; nav: StudyNavigation; selected: SelectedComment[]; onSelected: (rows: SelectedComment[]) => void; requestRefs: Map<string, string> }

/** Selection-bar estimate: the same read-only /preview used by the dialog, debounced while the selection changes. */
function usePlanEstimate(domain: string, selected: SelectedComment[]) {
  const [state, setState] = useState<{ key: string; plan: PreviewPlan | null; failed: boolean }>({ key: '', plan: null, failed: false });
  const key = selected.map(item => item.sourceRef).join(',');
  useEffect(() => {
    if (!selected.length) return;
    let live = true;
    const timer = window.setTimeout(() => {
      previewSelection(domain, selected, false).then(plan => { if (live) setState({ key, plan, failed: false }); }, () => { if (live) setState({ key, plan: null, failed: true }); });
    }, 350);
    return () => { live = false; window.clearTimeout(timer); };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- the source-ref key identifies the selection
  }, [domain, key]);
  return state.key === key ? state : { key, plan: null, failed: false };
}

export function CommentsView({ domain, nav, selected, onSelected, requestRefs }: Props) {
  const location = useLocation(), navigate = useNavigate();
  const state = location.state as { commentFilter?: CommentFilters; commentDomain?: string } | null;
  const filter = useMemo(() => commentFilters(new URLSearchParams(location.search), state?.commentDomain === domain ? state.commentFilter : null), [domain, location.search, state]);
  const [workError, setWorkError] = useState('');
  const [draft, setDraft] = useState(filter);
  const [queryDraft, setQueryDraft] = useState(filter.q);
  const [previousFilter, setPreviousFilter] = useState(filter);
  const [popover, setPopover] = useState(false);
  const [pop, setPop] = useState({ minLikes: filter.minLikes ?? '', postUsage: filter.postUsage, voiceRole: filter.voiceRole });
  const [selecting, setSelecting] = useState(selected.length > 0);
  const [bulkSize, setBulkSize] = useState<BulkSize>('300');
  const [bulkLoading, setBulkLoading] = useState(false);
  const [bulkNotice, setBulkNotice] = useState('');
  const [announcement, setAnnouncement] = useState('');
  const [jump, setJump] = useState('');
  const bulkRequest = useRef<AbortController | null>(null);
  const filterButton = useRef<HTMLButtonElement>(null), popRef = useRef<HTMLDivElement>(null), headCheck = useRef<HTMLInputElement>(null);
  // A router change replaces the local drafts; SelectField edits in the popover only apply when it closes.
  if (previousFilter !== filter) {
    // An unsubmitted search draft survives dropdown changes; only a new submitted query replaces it.
    setPreviousFilter(filter); setDraft(filter); if (previousFilter.q !== filter.q || previousFilter.searchScope !== filter.searchScope) setQueryDraft(filter.q);
    setPop({ minLikes: filter.minLikes ?? '', postUsage: filter.postUsage, voiceRole: filter.voiceRole });
  }
  const alive = useRef(true), searchSerial = useRef(0), currentLocation = useRef(location.key);
  useLayoutEffect(() => { currentLocation.current = location.key; return () => bulkRequest.current?.abort(); }, [location.key]);
  useEffect(() => { alive.current = true; return () => { alive.current = false; searchSerial.current += 1; }; }, []);

  function route(changes: Partial<CommentFilters>, replace = false) {
    const next = { ...filter, ...changes }, url = new URL(location.pathname + location.search, window.location.origin);
    url.searchParams.set('view', 'comments'); url.searchParams.set('domain', domain);
    const fields = [['q', next.q], ['searchScope', next.searchScope === 'comment' ? null : next.searchScope], ['workRef', next.workRef], ['state', next.studyState === 'all' ? null : next.studyState],
      ['signalKind', next.signalKind], ['cursor', next.cursor], ['voiceRole', next.voiceRole === 'reader_and_unknown' ? null : next.voiceRole], ['sort', next.sort === 'like_desc' ? null : next.sort],
      ['minLikes', next.minLikes], ['postUsage', next.postUsage === 'all' ? null : next.postUsage], ['page', next.page > 1 ? String(next.page) : null], ['size', next.size === '10' ? null : next.size], ['supportOf', next.supportOf]] as const;
    for (const [key, value] of fields) { if (value) url.searchParams.set(key, value); else url.searchParams.delete(key); }
    for (const key of ['problemRef', 'runRef', 'panel', 'targetRef', 'signalRef']) url.searchParams.delete(key);
    void navigate(url.pathname + url.search, { replace, state: { ...state, commentDomain: domain, commentFilter: next } });
  }
  const reset: Pick<CommentFilters, 'page' | 'cursor' | 'history'> = { page: 1, cursor: null, history: [] };
  const apply = (changes: Partial<CommentFilters>) => route({ ...changes, ...reset });

  const queryParams = new URLSearchParams({ q: filter.searchScope === 'comment' ? filter.q : '', studyState: filter.studyState, voiceRole: filter.voiceRole, sort: filter.sort, postUsage: filter.postUsage, limit: filter.size });
  // supportOf（地址参数）→ problemRef（评论接口参数，CONTRACT 第二轮 §5）：与其他筛选、offset 分页同用。
  for (const [key, value] of [['workRef', filter.workRef], ['signalKind', filter.signalKind], ['minLikes', filter.minLikes], ['problemRef', filter.supportOf]] as const) if (value) queryParams.set(key, value);
  const pageParams = new URLSearchParams(queryParams);
  // Numbered paging (CONTRACT §4) only sends offset after the server has reported totalCount; page 1 stays compatible with cursor-only servers.
  if (filter.page > 1) pageParams.set('offset', String((filter.page - 1) * Number(filter.size)));
  else if (filter.cursor) pageParams.set('cursor', filter.cursor);
  const listQuery = useStudyRead(domain, `comments?${pageParams}`, parseComments);
  // 筛选条只需要问题名称；与问题抽屉共用同一缓存读取。
  const support = useQuery({ queryKey: ['comment-understanding', domain, 'problem', filter.supportOf], enabled: Boolean(filter.supportOf),
    queryFn: ({ signal }) => readProblem(domain, filter.supportOf!, signal), retry: false });

  const supportMode = Boolean(filter.supportOf);
  const rows: CommentItem[] = listQuery.data?.items ?? [];
  const pending = listQuery.isPending, failure = listQuery.isError ? listQuery.error : null;
  const stats = listQuery.data?.summary, coverage = listQuery.data?.indexCoverage;
  const totalCount = listQuery.data?.page.totalCount ?? null;
  const numbered = totalCount != null;

  const selectedIds = new Set(selected.map(item => commentIdentity(item.commentKey))), selectedWorks = new Set(selected.map(item => item.commentKey.workRef));
  const estimate = usePlanEstimate(domain, selected);
  async function searchWorks(q: string) {
    const serial = ++searchSerial.current, locationKey = location.key;
    try { const data = await studyRead(domain, `works?${new URLSearchParams({ q, limit: '20' })}`, parseWorkCatalog); if (!alive.current || serial !== searchSerial.current || currentLocation.current !== locationKey) return; setDraft(old => ({ ...old, workChoices: data.items })); setWorkError(data.items.length ? '' : '没有找到匹配的作品标题'); }
    catch { if (alive.current && serial === searchSerial.current && currentLocation.current === locationKey) setWorkError('作品读取失败'); }
  }
  function submitSearch() {
    if (draft.searchScope === 'work' && queryDraft.trim() && !draft.workRef) { void searchWorks(queryDraft.trim()); setWorkError('请从作品候选中选择一篇作品'); return; }
    apply({ q: queryDraft.trim(), searchScope: draft.searchScope, workRef: draft.searchScope === 'work' ? draft.workRef : null, workLabel: draft.searchScope === 'work' ? draft.workLabel : '', workChoices: draft.workChoices });
  }
  const saved = async (message: string) => { nav.toast(message); const result = await listQuery.refetch(); if (result.error) throw result.error; };

  const extraCount = Number(filter.voiceRole !== 'reader_and_unknown') + Number(Boolean(filter.minLikes)) + Number(filter.postUsage !== 'all');
  function closePopover(applyChanges: boolean) {
    setPopover(false);
    if (!applyChanges) return;
    const minLikes = /^[1-9]\d{0,9}$/.test(pop.minLikes.trim()) ? pop.minLikes.trim() : null;
    if (minLikes !== filter.minLikes || pop.postUsage !== filter.postUsage || pop.voiceRole !== filter.voiceRole) apply({ minLikes, postUsage: pop.postUsage, voiceRole: pop.voiceRole });
  }
  useEffect(() => {
    if (!popover) return;
    const outside = (event: PointerEvent) => {
      const target = event.target as Element | null;
      if (!target || popRef.current?.contains(target) || filterButton.current?.contains(target) || target.closest('[role=listbox]')) return;
      closePopover(true);
    };
    document.addEventListener('pointerdown', outside, true);
    return () => document.removeEventListener('pointerdown', outside, true);
  });

  async function prepareFilteredSelection() {
    if (bulkRequest.current || selected.length) return;
    const controller = new AbortController();
    bulkRequest.current = controller; setBulkLoading(true); setBulkNotice('');
    const requested = Number(bulkSize), pickParams = new URLSearchParams(queryParams);
    pickParams.set('limit', '100');
    const selections = new Map<string, SelectedComment>();
    let scanned = 0;
    try {
      while (scanned < requested) {
        const page = await studyRead(domain, `comments?${pickParams}`, parseComments, controller.signal);
        const items = page.items.slice(0, requested - scanned);
        for (const item of items) if (canSelect(item)) selections.set(commentIdentity(item.commentKey), selectionFor(item));
        scanned += items.length;
        const next = page.page.nextCursor;
        if (items.length === 0 || !next) break;
        if (next === pickParams.get('cursor')) throw new Error('评论目录分页未前进');
        pickParams.set('cursor', next);
      }
      if (controller.signal.aborted) return;
      onSelected([...selections.values()]);
      setBulkNotice(`已检查当前筛选结果的前 ${scanned} 条，选入 ${selections.size} 条。`);
    } catch {
      if (!controller.signal.aborted) setBulkNotice('未能生成评论清单，请重试。');
    } finally {
      bulkRequest.current = null;
      if (alive.current) setBulkLoading(false);
    }
  }

  const pageEligible = rows.filter(canSelect);
  const pageSelected = pageEligible.filter(item => selectedIds.has(commentIdentity(item.commentKey))).length;
  const allPageSelected = pageEligible.length > 0 && pageSelected === pageEligible.length;
  useEffect(() => { if (headCheck.current) headCheck.current.indeterminate = pageSelected > 0 && !allPageSelected; });
  function togglePage() {
    const pageKeys = new Set(pageEligible.map(item => commentIdentity(item.commentKey)));
    if (allPageSelected) { onSelected(selected.filter(item => !pageKeys.has(commentIdentity(item.commentKey)))); return; }
    const next = [...selected], works = new Set(selectedWorks), ids = new Set(selectedIds);
    for (const item of pageEligible) {
      const key = commentIdentity(item.commentKey);
      if (ids.has(key)) continue;
      if (next.length >= MAX_SELECTED) break;
      if (!works.has(item.commentKey.workRef) && works.size >= MAX_WORKS) continue;
      next.push(selectionFor(item)); ids.add(key); works.add(item.commentKey.workRef);
    }
    onSelected(next);
  }
  function toggleSelecting() {
    if (selecting) {
      bulkRequest.current?.abort(); setBulkNotice('');
      if (selected.length) { const message = `已退出选择，清空了 ${selected.length} 条已选评论`; setAnnouncement(message); nav.toast(message); }
      onSelected([]);
    } else setAnnouncement('');
    setSelecting(old => !old);
  }

  const size = Number(filter.size), pages = numbered ? Math.max(1, Math.ceil(totalCount / size)) : 0;
  const from = numbered && totalCount ? (filter.page - 1) * size + 1 : 0, to = numbered ? Math.min(totalCount, filter.page * size) : 0;
  const pageNumbers = numbered ? [...new Set([1, filter.page - 1, filter.page, filter.page + 1, pages].filter(n => n >= 1 && n <= pages))].sort((a, b) => a - b) : [];
  const gotoPage = (n: number) => { if (n >= 1 && n <= pages && n !== filter.page) route({ page: n, cursor: null, history: [] }); };
  const visibleSelected = rows.filter(item => selectedIds.has(commentIdentity(item.commentKey))).length;
  const plan = estimate.plan;
  const notes = plan ? [
    plan.selected - plan.researchable > 0 ? `${plan.selected - plan.researchable} 条不会提交（${[plan.waiting ? `${plan.waiting} 条待作品语境` : '', plan.active ? `${plan.active} 条正在研究` : '', plan.researched ? `${plan.researched} 条已有结果` : ''].filter(Boolean).join('、') || '见预览'}）` : '',
    selected.length > visibleSelected ? `${selected.length - visibleSelected} 条不在当前页` : '',
  ].filter(Boolean) : [estimate.failed ? '计划暂时无法估算，可直接打开预览查看。' : selected.length > visibleSelected ? `${selected.length - visibleSelected} 条不在当前页` : ''].filter(Boolean);
  const supportLabel = support.data?.label ?? '用户问题';

  return <div className={c('comments-view')} data-has-selection={selected.length > 0 || undefined}>
    <form className={c('toolbar')} role="search" aria-label="筛选用户评论" onSubmit={event => { event.preventDefault(); submitSearch(); }}>
      <div className={c('field search-field')}><label className="v7-sr-only" htmlFor="comment-query">字面搜索</label>
        <div className={c('search-shell')}>
          <button type="submit" className={c('find')}>搜索</button>
          <span className={c('search-scope')}><SelectField compact label="搜索范围" value={draft.searchScope} onChange={searchScope => { setDraft(old => ({ ...old, searchScope, workRef: null, workLabel: '', workChoices: [] })); setWorkError(''); }} options={[{ value: 'comment', label: '评论文本' }, { value: 'work', label: '作品标题' }]} /></span>
          <input id="comment-query" type="search" aria-label="字面搜索" value={queryDraft} autoComplete="off" placeholder={draft.searchScope === 'work' ? '搜索作品标题' : '搜索评论原声'}
            onChange={event => { setQueryDraft(event.currentTarget.value); if (draft.searchScope === 'work') setDraft(old => ({ ...old, workRef: null, workLabel: '', workChoices: [] })); }} />
        </div>
        {draft.searchScope === 'work' && draft.workChoices.length && !draft.workRef ? <div className={c('work-choices')} role="listbox" aria-label="作品候选">{draft.workChoices.map(work =>
          <button type="button" role="option" aria-selected={draft.workRef === work.workRef} key={work.workRef} onClick={() => {
            const label = work.displayTitle || '未命名作品';
            setQueryDraft(label); setWorkError('');
            apply({ q: label, searchScope: 'work', workRef: work.workRef, workLabel: label, workChoices: draft.workChoices });
          }}>{work.displayTitle || '未命名作品'}</button>)}</div> : null}
      </div>
      <div className={c('field')}><SelectField label="研究状态" value={filter.studyState} onChange={studyState => apply({ studyState })} options={STATE_OPTIONS} /></div>
      <div className={c('field')}><SelectField label="研究标签" value={filter.signalKind || ''} onChange={signalKind => apply({ signalKind: signalKind || null })} options={TAG_OPTIONS} /></div>
      <div className={c('field')}><SelectField label="排序" value={filter.sort} onChange={sort => apply({ sort })} options={SORT_OPTIONS} /></div>
      <div className={c('filter-anchor')}>
        <ActionButton ref={filterButton} className={c('icon-gap')} aria-haspopup="dialog" aria-expanded={popover} aria-controls="comment-advanced-filters" onClick={() => popover ? closePopover(true) : setPopover(true)}>
          <CrIcon name="SlidersHorizontal" />筛选{extraCount ? <b className={c('filter-count')} aria-label={`${extraCount} 项生效`}>{extraCount}</b> : null}
        </ActionButton>
        {popover ? <div ref={popRef} id="comment-advanced-filters" className={c('filter-pop')} role="dialog" aria-label="更多筛选" onKeyDown={event => { if (event.key === 'Escape' && !event.defaultPrevented) { event.preventDefault(); closePopover(false); filterButton.current?.focus(); } }}>
          <div className={c('pop-row')}><label htmlFor="comment-min-likes">最低点赞</label><input id="comment-min-likes" type="number" min={1} step={1} inputMode="numeric" placeholder="不限" autoFocus value={pop.minLikes} onChange={event => setPop(old => ({ ...old, minLikes: event.target.value }))} /></div>
          <div className={c('pop-row')}><label>改帖标记</label><SelectField label="改帖标记" value={pop.postUsage} onChange={postUsage => setPop(old => ({ ...old, postUsage }))} options={USAGE_OPTIONS} /></div>
          <div className={c('pop-row')}><label>声音角色</label><SelectField label="声音角色" value={pop.voiceRole} onChange={voiceRole => setPop(old => ({ ...old, voiceRole }))} options={VOICE_OPTIONS} /></div>
          <div className={c('pop-foot')}>
            <ActionButton variant="quiet" className={c('quiet-small')} onClick={() => setPop({ minLikes: '', postUsage: 'all', voiceRole: 'reader_and_unknown' })}>清除筛选</ActionButton>
            <ActionButton variant="ink" className={c('small')} onClick={() => { closePopover(true); filterButton.current?.focus(); }}>完成</ActionButton>
          </div>
        </div> : null}
      </div>
      <ActionButton variant={selecting ? 'ink' : 'outline'} className={c('icon-gap')} aria-pressed={selecting} title={selecting ? '退出并清空本次选择' : '进入批量选择模式'} onClick={toggleSelecting}><CrIcon name="ListChecks" />{selecting ? '退出选择' : '选择评论'}</ActionButton>
    </form>
    {workError ? <p className={c('inline-alert')} role="status">{workError}</p> : null}
    <div className={c('result-line')}>
      <span role="status" aria-live="polite" title={coverage ? `已索引 ${num(coverage.indexedCount)} 条；待索引 ${num(coverage.pendingCount)} 条` : undefined}>
        {stats?.displayableCommentCount == null ? '评论范围待读取' : <>当前筛选 <b>{num(stats.displayableCommentCount)}</b> 条评论 · 可研究 {num(stats.eligibleCommentCount)} 条</>}
      </span>
      <span>
        {supportMode ? <span className={c('filter-chip')}><span>支持问题：{supportLabel}</span> <button type="button" className={c('text-action')} aria-label="取消问题筛选" onClick={() => apply({ supportOf: null })}>移除</button></span> : null}
        {filter.workRef ? <span className={c('filter-chip')}><span>限定作品：{filter.workLabel || filter.q || '所选作品'}</span> <button type="button" className={c('text-action')} aria-label="取消作品筛选" onClick={() => { setQueryDraft(''); apply({ workRef: null, workLabel: '', q: '', searchScope: 'comment' }); }}>移除</button></span> : null}
        {selecting && !selected.length ? <span className={c('bulk-pick')}>
          <span><SelectField compact label="检查当前筛选结果的前几条" value={bulkSize} disabled={bulkLoading} onChange={setBulkSize} options={BULK_SIZES.map(value => ({ value, label: `前 ${value} 条` }))} /></span>
          <ActionButton variant="quiet" className={c('quiet-small')} disabled={bulkLoading || !rows.length} onClick={() => void prepareFilteredSelection()}>{bulkLoading ? '正在生成清单' : '按当前筛选生成研究清单'}</ActionButton>
        </span> : null}
        {selecting && bulkNotice ? <span role="status"> {bulkNotice}</span> : null}
      </span>
    </div>
    {pending ? <p className={c('view-status')}>正在读取用户评论…</p> : failure ? <p className={c('view-status')} role="alert">评论目录读取失败：{errorText(failure)} <button type="button" className={c('text-action')} onClick={() => void listQuery.refetch()}>重试</button></p> : <>
      <div className={c('table-wrap')}><table className={c('comment-table')}><caption className="v7-sr-only">评论原声与研究结果</caption>
        <colgroup>{selecting ? <col style={{ width: 'var(--lgi-cr-check-col)' }} /> : null}{COLUMNS.map((width, index) => <col key={index} style={{ width: `${width}%` }} />)}</colgroup>
        <thead><tr>
          {selecting ? <th scope="col" className={c('check-cell')}><input ref={headCheck} className={c('table-check')} type="checkbox" aria-label="选择本页全部评论" checked={allPageSelected} disabled={bulkLoading || !pageEligible.length} onChange={togglePage} /></th> : null}
          <th scope="col">原声</th><th scope="col" className={c('numeric')}>点赞</th><th scope="col">所属作品</th><th scope="col">研究状态</th><th scope="col">研究标签</th><th scope="col">作者</th><th scope="col">点赞采集日</th><th scope="col">观测时间</th><th scope="col" className={c('center')}>改帖</th>
        </tr></thead>
        <tbody>{rows.length ? rows.map(item => {
          const key = commentIdentity(item.commentKey), eligible = canSelect(item), checked = selectedIds.has(key);
          const capped = !checked && (selected.length >= MAX_SELECTED || (!selectedWorks.has(item.commentKey.workRef) && selectedWorks.size >= MAX_WORKS));
          const status = commentStatusInfo(item), kinds = item.signalKinds.filter(kind => Object.hasOwn(signalKindLabel, kind));
          const workTitle = item.workTitle ? (item.workTitleSource === 'platform_title' ? item.workTitle : `${item.workTitle} · ${workTitleSourceLabel[item.workTitleSource || ''] || '标题来源未记录'}`) : '未命名作品';
          return <tr key={key} className={c(checked && 'selected')} data-comment-id={item.commentKey.commentExternalId}>
            {selecting ? <td className={c('check-cell')}><input className={c('table-check')} type="checkbox" aria-label="选择这条评论" data-select-comment data-eligible={eligible}
              title={!eligible ? commentEligibility(item) : capped ? '本次选择已达到评论或作品数量上限' : undefined} checked={checked} disabled={bulkLoading || !eligible || capped}
              onChange={event => { if (event.target.checked && eligible && !capped) onSelected([...selected, selectionFor(item)]); else onSelected(selected.filter(row => commentIdentity(row.commentKey) !== key)); }} /></td> : null}
            <td><button type="button" className={c('voice-link')} title={item.commentText || '评论原声不可用'} data-comment-work={item.commentKey.workRef} data-comment-id={item.commentKey.commentExternalId} onClick={() => nav.comment(item.commentKey)}>{item.commentText || '评论原声不可用'}</button></td>
            <td className={c('numeric')} title={item.likeCount == null ? '点赞数未知：采集时没有取得这条评论的点赞' : `${num(item.likeCount)} 次点赞`}>{num(item.likeCount)}</td>
            <td title={workTitle}>{item.workTitle || '未命名作品'}</td>
            <td title={eligible ? commentStatus(item) : `${commentStatus(item)} · ${commentEligibility(item)}`}><span className={c('state', status.tone)}>{status.label}</span></td>
            <td title={kinds.map(kind => signalKindLabel[kind]).join('、') || '无当前有效标签'}>{kinds[0] ? <span className={c('pill')}>{signalKindLabel[kinds[0]]}</span> : <span className={c('muted')}>—</span>}</td>
            <td title={item.authorDisplayName || '作者未记录'}>{item.authorDisplayName || '—'}</td>
            <td className={c('date-cell')} title={item.likeCountAsOf ? `点赞采集时间：${studyTime(item.likeCountAsOf)}` : '点赞采集时间未记录'}>{slashDay(item.likeCountAsOf) || '—'}</td>
            <td className={c('date-cell')} title={item.observedAt ? studyTime(item.observedAt) : '评论观测时间未记录'}>{monthTime(item.observedAt)}</td>
            <td className={c('center')}><PostUsageControl item={item} domain={domain} requestRefs={requestRefs} onSaved={saved} onStatus={nav.toast} /></td>
          </tr>;
        }) : <tr><td colSpan={selecting ? 10 : 9} className={c('empty')}><strong>没有匹配的评论</strong><p>调整搜索词或筛选条件，再试一次。</p>
          <ActionButton className={c('small')} onClick={() => { setQueryDraft(''); route({ q: '', searchScope: 'comment', workRef: null, workLabel: '', studyState: 'all', signalKind: null, minLikes: null, postUsage: 'all', voiceRole: 'reader_and_unknown', supportOf: null, ...reset }); }}>清除筛选</ActionButton></td></tr>}</tbody>
      </table></div>
      {(numbered ? totalCount > 0 : rows.length || filter.history.length) ? <div className={c('table-foot')}>
        <span className={c('page-range')}>{numbered ? `第 ${from}–${to} 条 · 共 ${num(totalCount)} 条` : `第 ${filter.history.length + 1} 页`}</span>
        <nav className={c('pager')} aria-label="评论分页">{numbered ? <>
          <button type="button" className={c('page-btn')} disabled={filter.page <= 1} aria-label="上一页" onClick={() => gotoPage(filter.page - 1)}>‹</button>
          {pageNumbers.map((n, index) => <span key={n} style={{ display: 'contents' }}>{index && n - pageNumbers[index - 1]! > 1 ? <span className={c('page-gap')} aria-hidden="true">…</span> : null}
            <button type="button" className={c('page-btn')} aria-current={n === filter.page ? 'page' : undefined} aria-label={`第 ${n} 页`} onClick={() => gotoPage(n)}>{n}</button></span>)}
          <button type="button" className={c('page-btn')} disabled={filter.page >= pages} aria-label="下一页" onClick={() => gotoPage(filter.page + 1)}>›</button>
        </> : <>
          <button type="button" className={c('page-btn')} id="comment-prev" disabled={!filter.history.length} aria-label="上一页" onClick={() => route({ cursor: filter.history.at(-1) ?? null, history: filter.history.slice(0, -1) })}>‹</button>
          <button type="button" className={c('page-btn')} id="comment-next" disabled={!listQuery.data?.page.nextCursor} aria-label="下一页" onClick={() => route({ history: [...filter.history, filter.cursor], cursor: listQuery.data?.page.nextCursor ?? null })}>›</button>
        </>}</nav>
        <div className={c('pager-tools')}>
          {numbered ? <label className={c('page-jump')}>跳至<input type="number" min={1} max={pages} step={1} inputMode="numeric" aria-label="跳转到页码" placeholder={String(filter.page)} value={jump}
            onChange={event => setJump(event.target.value)} onKeyDown={event => { if (event.key === 'Enter') { event.preventDefault(); const n = Math.round(Number(jump)); setJump(''); if (n >= 1) gotoPage(Math.min(n, pages)); } }}
            onBlur={() => { const n = Math.round(Number(jump)); setJump(''); if (n >= 1) gotoPage(Math.min(n, pages)); }} />页</label> : null}
          <span className={c('pager-size')}><SelectField label="每页条数" value={filter.size} onChange={(value: PageSize) => apply({ size: value })} options={PAGE_SIZES.map(value => ({ value, label: `${value} 条/页` }))} /></span>
        </div>
      </div> : null}
    </>}
    {selected.length ? <div className={c('selection-bar')} role="region" aria-label="已选评论">
      <div><div className={c('selection-info')}><strong>已选 {selected.length} 条评论</strong>
        <span>{plan ? <>将提交 <b>{num(plan.researchable)}</b> 条 · 预计调用 <b>{num(plan.calls)}</b> 次</> : estimate.failed ? '将提交 — 条 · 预计调用 — 次' : '正在估算提交与调用…'}</span></div>
        <div className={c('selection-detail')}>{notes.join(' · ')}</div></div>
      <div className={c('selection-actions')}>
        <ActionButton variant="quiet" onClick={() => { onSelected([]); setBulkNotice(''); setAnnouncement(`已清空 ${selected.length} 条已选评论`); }}>清空选择</ActionButton>
        <ActionButton variant="ink" className={c('icon-gap')} onClick={() => nav.preview(selected, { submitted: () => onSelected([]) })}>预览研究计划 <CrIcon name="ArrowRight" /></ActionButton>
      </div>
    </div> : null}
    <div className="v7-sr-only" role="status" aria-live="polite">{selected.length ? `已选择${selected.length}条${plan ? `，将提交${plan.researchable}条` : ''}` : announcement}</div>
  </div>;
}
