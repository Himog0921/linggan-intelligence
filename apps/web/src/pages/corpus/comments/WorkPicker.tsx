import { useEffect, useRef, useState } from 'react';
import { errorText, studyTime } from './format';
import { workTitleSourceLabel } from './labels';
import { parseWorkCatalog, type SourcePreview, type StudySetup } from './methodDtos';
import { useStudyRead } from './studyApi';
import { c } from './styles';

export interface WorkRole { contentPublicRef: string; observationRole: string }
export function SourceSummary({ preview, role, id }: { preview: SourcePreview | null; role: string; id: string }) {
  if (!preview) return <p id={id} className={c(role === 'reference' ? 'study-table-status' : 'study-dialog-note')} role="status">{role}来源资格统计暂不可用。</p>;
  const labels = { workAuthorUnknown: '作品作者身份未知', commentAuthorUnknown: '评论作者身份未知，不纳入研究', creatorVoice: '作品作者本人', bodyUnavailable: '正文不可研究', sourceRestricted: '来源受限', textNotResearchable: '文本不具研究条件' };
  const reasons = Object.entries(labels).filter(([key]) => Number(preview.excludedCounts[key]) > 0).map(([key, label]) => `${label} ${preview.excludedCounts[key]} 条`);
  const unknown = Number(preview.unknownAuthorCount || 0);
  return <p id={id} className={c(role === 'reference' ? 'study-table-status' : 'study-dialog-note')} role="status">{role} · 截至 {studyTime(preview.asOf)}：共 {Number(preview.totalCommentCount || 0)} 条评论；可研究 {Number(preview.eligibleCommentCount || 0)} 条；未纳入 {Number(preview.totalCommentCount || 0) - Number(preview.eligibleCommentCount || 0)} 条{unknown && !Number(preview.excludedCounts.commentAuthorUnknown || 0) ? `；评论作者身份未知 ${unknown} 条，不纳入研究` : ''}{reasons.length ? `（${reasons.join('；')}）` : ''}。</p>;
}

export function WorkPicker({ domain, setup, kind, selectedComments, selected, onSelected, commentBudget, onCatalogLoaded }: { domain: string; setup: StudySetup; kind: 'works' | 'comments'; selectedComments: number; selected: WorkRole[]; onSelected: (rows: WorkRole[]) => void; commentBudget: number; onCatalogLoaded?: (count: number | null, failed: boolean) => void }) {
  const [search, setSearch] = useState(''), [q, setQ] = useState(''), [role, setRole] = useState('primary');
  const [history, setHistory] = useState<(string | null)[]>([]), [cursor, setCursor] = useState<string | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => () => { if (timer.current) clearTimeout(timer.current); }, []);
  const params = new URLSearchParams({ observationRole: role, limit: '50' }); if (q) params.set('q', q); if (cursor) params.set('cursor', cursor);
  const query = useStudyRead(domain, `works?${params}`, parseWorkCatalog);
  const reported = useRef('');
  useEffect(() => { const phase = query.isError ? 'error' : query.data ? 'loaded' : '';
    if (phase && phase !== reported.current && role === 'primary' && !q && !cursor) { reported.current = phase; onCatalogLoaded?.(query.data?.totalWorkCount ?? null, phase === 'error'); }
  }, [query.isError, query.data, role, q, cursor, onCatalogLoaded]);
  const rows = query.data?.items ?? [], visibleCount = rows.filter(work => selected.some(item => item.contentPublicRef === work.workRef && item.observationRole === (work.observationRole || role))).length;
  const roleLabel = role === 'primary' ? '当前领域' : '参考';
  const ready = !query.isPending && !query.isError;
  const selectedBox = useRef<HTMLInputElement>(null);
  const setAllBox = (node: HTMLInputElement | null) => { selectedBox.current = node; if (node) node.indeterminate = visibleCount > 0 && visibleCount < rows.length; };
  const select = (workRef: string, workRole: string, checked: boolean) => {
    if (checked) { if (selected.some(row => row.contentPublicRef === workRef) || selected.length < 100) onSelected([...selected.filter(row => row.contentPublicRef !== workRef), { contentPublicRef: workRef, observationRole: workRole }]); }
    else onSelected(selected.filter(row => !(row.contentPublicRef === workRef && row.observationRole === workRole)));
  };
  return <section className={c('study-workspace')} data-selection-kind={kind} aria-labelledby="work-picker-title">
    <header className={c('study-workspace-header')}><div><p className={c('study-label')}>研究输入</p><h3 id="work-picker-title">{kind === 'comments' ? '已选评论' : '选择要研究的作品'}</h3></div><div className={c('study-workspace-tools')}>
      <label className={c('study-filter')}>搜索作品目录<input id="work-filter" type="search" placeholder="按作品标题搜索" autoComplete="off" value={search} onChange={event => { const value = event.target.value; setSearch(value); if (timer.current) clearTimeout(timer.current); timer.current = setTimeout(() => { setQ(value.trim()); setCursor(null); setHistory([]); }, 250); }} /></label>
      <label className={c('study-reference-toggle')}><input id="include-reference-works" type="checkbox" disabled={setup.domainStatus !== 'active' || !Number(setup.referenceSourcePreview?.totalCommentCount)} checked={role === 'reference'} onChange={event => { setRole(event.target.checked ? 'reference' : 'primary'); setCursor(null); setHistory([]); }} />查看参考作品</label>
      <span id="selected-count" className={c('selection-count')} role="status">{kind === 'comments' ? `已选 ${selectedComments}/3000 条评论 · 本次上限 ${commentBudget || '—'} 条` : `已选 ${selected.length}/100 篇 · 当前页 ${visibleCount}/${rows.length} 篇`}</span>
    </div></header>
    <div hidden={!Number(setup.referenceSourcePreview?.totalCommentCount)}><SourceSummary id="reference-source-preview" role="reference" preview={setup.referenceSourcePreview} /></div>
    <p id="work-filter-status" className={c('study-table-status')} role="status">{query.isPending ? '正在读取可研究作品…' : query.isError ? `作品目录加载失败：${errorText(query.error)}。可重试或调整搜索条件。` : `${roleLabel}作品：当前页 ${rows.length} 篇 / 匹配 ${query.data?.totalWorkCount ?? 0} 篇；已索引评论 ${query.data?.indexCoverage?.indexedCount ?? 0}，待索引 ${query.data?.indexCoverage?.pendingCount ?? '未知'}。`}</p>
    <div className={c('study-table-wrap')}><table className={c('study-table')} aria-labelledby="work-picker-title" aria-describedby="work-filter-status"><caption className="v7-sr-only">可选择进入本次评论研究的作品及其领域角色</caption><thead><tr><th scope="col"><input ref={setAllBox} id="select-visible-works" type="checkbox" aria-label="选择当前页的作品" checked={rows.length > 0 && visibleCount === rows.length} disabled={!ready || (selected.length >= 100 && visibleCount < rows.length)} onChange={event => { const next = [...selected]; for (const work of rows) { const workRole = work.observationRole || role; const current = next.findIndex(item => item.contentPublicRef === work.workRef); if (event.target.checked && current < 0 && next.length < 100) next.push({ contentPublicRef: work.workRef, observationRole: workRole }); else if (!event.target.checked && current >= 0 && next[current]?.observationRole === workRole) next.splice(current, 1); } onSelected(next); }} /></th><th scope="col">作品</th><th scope="col">作品来源</th><th scope="col">可研究评论</th></tr></thead><tbody id="works" aria-live="polite">{query.isPending ? <tr><td colSpan={4}>正在读取作品…</td></tr> : query.isError ? <tr><td className={c('study-table-empty')} colSpan={4}>作品列表暂不可用：{errorText(query.error)} <button id="retry-work-catalog" type="button" onClick={() => { void query.refetch(); }}>重试</button></td></tr> : rows.length ? rows.map(work => { const workRole = work.observationRole || role, checked = selected.some(item => item.contentPublicRef === work.workRef && item.observationRole === workRole), title = work.displayTitle || '未命名作品'; return <tr key={work.workRef}><td><input id={`work-${work.workRef}`} type="checkbox" name="work-ref" value={work.workRef} aria-label={`选择作品：${title}`} checked={checked} disabled={!selected.some(item => item.contentPublicRef === work.workRef) && selected.length >= 100} onChange={event => select(work.workRef, workRole, event.target.checked)} /></td><td><label htmlFor={`work-${work.workRef}`}><span className={c('study-work-title')}>{title}</span><small>{workTitleSourceLabel[work.displayTitleSource ?? ''] || work.displayTitleSource || '标题未记录'}</small></label></td><td>{workRole === 'primary' ? '当前领域' : workRole === 'reference' ? '参考' : '来源未记录'}</td><td>{Number(work.eligibleCommentCount || 0)}</td></tr>; }) : <tr><td className={c('study-table-empty')} colSpan={4}>当前搜索没有匹配作品。</td></tr>}</tbody></table></div>
    <nav className={c('study-pagination')} aria-label="作品目录翻页"><button id="work-prev" type="button" disabled={!history.length} onClick={() => { setCursor(history.at(-1) ?? null); setHistory(items => items.slice(0, -1)); }}>上一页</button><span id="work-page-status" role="status">第 {history.length + 1} 页</span><button id="work-next" type="button" disabled={!query.data?.page.nextCursor} onClick={() => { setHistory(items => [...items, cursor]); setCursor(query.data?.page.nextCursor ?? null); }}>下一页</button></nav>
  </section>;
}
