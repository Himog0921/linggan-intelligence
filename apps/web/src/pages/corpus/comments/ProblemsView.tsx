import { useCallback, useState } from 'react';
import { useLocation, useNavigate } from 'react-router';
import { errorText, num } from './format';
import { ProblemClueCard, ProblemResultCard } from './ProblemCards';
import { ProblemClueDrawer } from './ProblemClueDrawer';
import { ProblemDrawer } from './ProblemDrawer';
import { parseProblemCards, parseProblemClues, type ProblemClue } from './problemDtos';
import { candidateStates } from './problemModel';
import { useStudyRead, type StudyNavigation } from './studyApi';
import { c } from './styles';

interface SavedResults { domain: string; view: string; q: string; order: string; cursor: string | null; candidate: string; density: string }
export function ProblemsView({ domain, nav, active = true }: { domain: string; nav: StudyNavigation; active?: boolean }) {
  const location = useLocation(), navigate = useNavigate(), params = new URLSearchParams(location.search);
  const state = location.state as { problemResults?: SavedResults } | null;
  const saved = state?.problemResults?.domain === domain ? state.problemResults : null;
  const view = ['all', 'changes', 'clues'].includes(params.get('state') || '') ? params.get('state')! : saved?.view || 'all';
  const q = params.get('q') ?? saved?.q ?? '', order = ['recent', 'created', 'support'].includes(params.get('sort') || '') ? params.get('sort')! : saved?.order || 'recent';
  const cursor = params.get('cursor') ?? saved?.cursor ?? null, candidate = saved?.candidate && candidateStates[saved.candidate] ? saved.candidate : 'all';
  const density = saved?.density === 'compact' ? 'compact' : 'standard';
  const [scope, setScope] = useState(false), [clue, setClue] = useState<ProblemClue | null>(null);
  const [restricted, setRestricted] = useState<Set<string>>(() => new Set());
  const availability = useCallback((ref: string, readable: boolean) => setRestricted(old => { if (old.has(ref) === !readable) return old; const next = new Set(old); if (readable) next.delete(ref); else next.add(ref); return next; }), []);
  const problemRef = params.get('problemRef'), clues = view === 'clues';
  function route(change: Partial<SavedResults> & { problemRef?: string | null } = {}, replace = false) {
    const next: SavedResults = { domain, view, q, order, cursor, candidate, density, ...change };
    const url = new URL(location.pathname + location.search, window.location.origin); url.searchParams.set('domain', domain); url.searchParams.set('view', 'problems');
    for (const field of ['runRef', 'panel', 'signalRef', 'targetRef', 'workRef', 'commentWorkRef', 'commentExternalId', 'detail', 'signalKind', 'voiceRole', 'minLikes', 'postUsage']) url.searchParams.delete(field);
    for (const [key, value] of [['state', next.view], ['q', next.q], ['sort', next.order], ['cursor', next.cursor], ['problemRef', 'problemRef' in change ? change.problemRef : problemRef]] as const) { if (value) url.searchParams.set(key, value); else url.searchParams.delete(key); }
    void navigate(url.pathname + url.search, { replace, state: { ...state, problemResults: next } });
  }
  const queryParams = new URLSearchParams({ limit: '12', q, view: view === 'changes' ? 'changes' : 'all', order }); if (cursor) queryParams.set('cursor', cursor);
  const clueParams = new URLSearchParams({ limit: '12', state: candidate }); if (cursor) clueParams.set('cursor', cursor);
  const cards = useStudyRead(domain, `problem-cards?${queryParams}`, parseProblemCards, active && !clues);
  const expressions = useStudyRead(domain, `problem-candidates?${clueParams}`, parseProblemClues, active && clues);
  const current = clues ? expressions : cards, items = (cards.data?.problems ?? []).map(row => restricted.has(row.problemRef) ? { ...row, definitionReadable: false, title: null, definition: null, preview: null, stableIdentity: null, includeCriteria: null, excludeCriteria: null } : row), clueItems = expressions.data?.expressions ?? [], next = (clues ? expressions.data : cards.data)?.page.nextCursor;
  const loading = current.isPending, failed = current.isError;
  const emptyLabel = clues ? '当前状态下没有可读取的未归并线索' : view === 'changes' ? '近 28 天没有新的研究记录变化' : q ? '没有匹配的问题' : '尚无已建档问题，未归并表达仍可查看';
  const empty = <div className={c('sr3-empty')}><span className={c('sr3-empty-mark')}>[ — ]</span><h3>{emptyLabel}</h3><p>{q ? '试试更短的关键词。' : '已有材料不会因为缺少新结论而被隐藏，也不会用演示内容补齐。'}</p><button type="button" className={c('sr3-button')} data-sr3={q ? 'clear' : 'view'} data-value={clues ? 'all' : 'clues'} onClick={() => route(q ? { q: '', cursor: null } : { view: clues ? 'all' : 'clues', cursor: null, q: '', problemRef: null })}>{q ? '清除搜索' : clues ? '查看全部问题' : '查看未归并线索'}</button></div>;
  const grid = (rows: typeof items) => <div className={c('sr3-grid')}>{rows.map(problem => <ProblemResultCard key={problem.problemRef} problem={problem} onOpen={() => route({ problemRef: problem.problemRef })} />)}</div>;
  return <div className={c('study-problems-workbench')}>
    <div className={c('sr3-heading')}><div><span className={c('sr3-eyebrow')}>评论研究 / 成果</span><h2>用户问题</h2></div><span className={c('sr3-heading-note')}>问题长期积累，原声随时可查</span></div>
    <div className={c('sr3-toolbar')}><nav className={c('sr3-tabs')} aria-label="问题阅读范围">{([['all', '全部问题'], ['changes', '最近变化'], ['clues', '未归并线索']] as const).map(([value, label]) => <button type="button" data-sr3="view" data-value={value} aria-pressed={view === value} key={value} onClick={() => { setClue(null); route({ view: value, cursor: null, q: '', problemRef: null }); }}>{label}</button>)}</nav>
      <form key={q} data-sr3-search className={c('sr3-search')} role="search" hidden={clues} onSubmit={event => { event.preventDefault(); route({ q: String(new FormData(event.currentTarget).get('q') || '').trim(), cursor: null }); }}><label className="v7-sr-only" htmlFor="sr3-query">搜索问题标题与定义</label><input id="sr3-query" name="q" type="search" maxLength={300} autoComplete="off" defaultValue={q} placeholder="搜索问题标题与定义" /><button type="submit" aria-label="搜索问题">搜索</button></form>
      <label className={c('sr3-select')} hidden={clues}>排序<select data-sr3-select="order" value={order} onChange={event => route({ order: event.target.value, cursor: null })}>{Object.entries({ recent: '最近记录更新', created: '最近建档', support: '关联原声最多' }).map(([value, label]) => <option value={value} key={value}>{label}</option>)}</select></label><div className={c('sr3-density')} aria-label="卡片密度">{([['standard', '阅读卡'], ['compact', '紧凑卡']] as const).map(([value, label]) => <button type="button" className={c('sr3-button')} data-sr3="density" data-value={value} aria-pressed={density === value} key={value} onClick={() => { route({ density: value }, true); }}>{label}</button>)}</div>
    </div>
    <div className={c('sr3-range')}><span data-sr3-status role="status">{loading ? '正在读取当前领域…' : failed ? '未取得本次查询结果' : clues ? `本页 ${clueItems.length} 条线索${next ? ' · 还有后续内容' : ' · 已到末页'} · 未计入问题总数` : `匹配 ${num(cards.data?.totalCount)} 个问题 · 本页 ${items.length} 个${view === 'changes' ? ' · 近 28 天的记录变化' : ''}`}</span><div><button type="button" className={c('sr3-button')} data-sr3="scope" aria-expanded={scope} onClick={() => setScope(open => !open)}>研究范围</button><button type="button" className={c('sr3-button')} data-sr3="refresh" onClick={() => { if (cursor) route({ cursor: null }, true); else void current.refetch(); }}>刷新</button></div></div>
    <section className={c('sr3-scope')} data-sr3-scope hidden={!scope}><strong>这些数字指向库内研究材料，不代表市场规模。</strong><p>原声按作品与评论标识去重，账号按平台与账号标识去重，不等于核验后的独立自然人。最近变化按建档、定义版本或原声首次关联时间统计近 28 天；补入历史评论不表示需求增长。</p><p>关联原声预览按首次加入时间选取，不保证代表全部情境；它不是整段定义的语义验证。读取基线约束翻页新增材料，来源可读性在每次读取时重新核对。</p></section>
    <div data-sr3-candidate-controls hidden={!clues}><label className={c('sr3-select')}>处理状态<select data-sr3-select="candidate" value={candidate} onChange={event => route({ candidate: event.target.value, cursor: null })}>{Object.entries(candidateStates).map(([value, label]) => <option value={value} key={value}>{label}</option>)}</select></label></div>
    <div data-sr3-list data-density={density}>{loading ? <div className={c('sr3-loading')} role="status">正在读取研究成果…</div> : failed ? <><div className={c('sr3-message')} role="status"><strong>{errorText(current.error)}</strong><button type="button" className={c('sr3-button')} data-sr3="retry" onClick={() => { void current.refetch(); }}>重新读取</button></div>{cursor ? <button type="button" className={c('sr3-button')} onClick={() => route({ cursor: null }, true)}>从第一页重新读取</button> : null}</> : clues ? clueItems.length ? <div className={c('sr3-grid')}>{clueItems.map(row => <ProblemClueCard key={row.signalRef} clue={row} onOpen={() => setClue(row)} />)}</div> : empty : !items.length ? empty : view === 'changes' ? ([['定义记录', '首次建档与已有定义的新版本；不等于模型已经验证结论。', items.filter(row => row.changeKind !== 'evidence_added')], ['仅材料补入', '同一问题补入依据，不据此生成新的研究结论。', items.filter(row => row.changeKind === 'evidence_added')]] as const).filter(([, , rows]) => rows.length).map(([title, note, rows]) => <section className={c('sr3-group')} key={title}><div className={c('sr3-group-head')}><h3>{title}</h3><span>{note}</span></div>{grid(rows)}</section>) : grid(items)}</div>
    <nav className={c('sr3-pagination')} aria-label="问题翻页" data-sr3-pages>{!loading && !failed ? <>{cursor ? <button type="button" className={c('sr3-button')} onClick={() => route({ cursor: null }, true)}>回到第一页</button> : null}<span>{clues ? '未归并表达按最近处理记录排列' : '检索、排序与分页由服务端执行'}</span>{next ? <button type="button" className={c('sr3-button')} data-sr3="next" onClick={() => route({ cursor: next })}>下一页 →</button> : null}</> : null}</nav>
    {active && problemRef ? <ProblemDrawer key={problemRef} domain={domain} problemRef={problemRef} onAvailability={availability} close={() => route({ problemRef: null }, true)} refreshList={() => { void cards.refetch(); }} /> : null}
    {active && clue ? <ProblemClueDrawer key={clue.signalRef} domain={domain} clue={clue} close={() => setClue(null)} nav={nav} /> : null}
  </div>;
}
