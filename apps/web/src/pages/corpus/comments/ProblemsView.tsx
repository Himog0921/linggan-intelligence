import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { useLocation, useNavigate } from 'react-router';
import { ActionButton } from '../../../shared/ActionButton';
import { SelectField } from '../../../shared/SelectField';
import { ParticleWake, PixelCode } from './problemArt';
import { archiveId, bannerCode, emblemFor } from './researchModel';
import { readProblems, type ProblemCatalogRow } from './understandingApi';
import type { StudyNavigation } from './studyApi';
import { errorText } from './format';
import { c } from './styles';

type Status = 'all' | 'formal' | 'candidate';
type Sort = 'recent' | 'authors' | 'comments' | 'works';
const SORTS: readonly { value: Sort; label: string }[] = [{ value: 'recent', label: '最新研究' }, { value: 'authors', label: '作者最多' }, { value: 'comments', label: '评论最多' }, { value: 'works', label: '作品最多' }];
const pad = (value: number) => String(value).padStart(2, '0');
const day = (value: string | null) => {
  const date = new Date(value ?? '');
  return Number.isNaN(date.valueOf()) ? '未记录' : new Intl.DateTimeFormat('zh-CN', { timeZone: 'Asia/Shanghai', year: 'numeric', month: '2-digit', day: '2-digit' }).format(date);
};

function Card({ problem, open }: { problem: ProblemCatalogRow; open: () => void }) {
  const formal = problem.state === 'formal', filled = Math.min(8, problem.authors), id = `problem-title-${problem.problemRef}`;
  return <article className={c('pa-card')} data-emblem={emblemFor(problem.problemRef)} aria-labelledby={id}>
    <div className={c('pa-banner')} aria-hidden="true">
      <span className={c('pa-corner tl')} /><span className={c('pa-corner tr')} /><span className={c('pa-corner bl')} /><span className={c('pa-corner br')} />
      <div className={c('pa-banner-label')}><i className={c('pa-signal-dot')} /><span>LI / PROBLEM ARCHIVE</span></div>
      <div className={c('pa-orb')} /><div className={c('pa-pixel-code')}><PixelCode code={bannerCode(problem.problemRef)} /></div>
      <ParticleWake className={c('pa-particle-flow')} />
      <div className={c('pa-emblem-wrap')}><span className={c('pa-emblem')} /></div>
    </div>
    <div className={c('pa-card-body')}>
      <div className={c('pa-meta-row')}><span className={c('pa-archive-id')}>ARCHIVE / {archiveId(problem.problemRef)}</span><span className={c('pa-state', problem.state)}>{formal ? '正式' : '候选'}</span></div>
      <div className={c('pa-title-block')}>
        <div className={c('pa-title-line')}><h2 className={c('pa-card-title')} id={id} title={problem.label}>{problem.label}</h2></div>
        {problem.summary ? <p className={c('pa-card-desc')} title={problem.summary}>{problem.summary}</p> : null}
      </div>
      <div className={c('pa-metrics')}>
        <div className={c('pa-author-count')}><b>{pad(problem.authors)}</b><span>独立作者</span></div>
        <div className={c('pa-support')} title="每格 1 位独立作者；第 2 格后的刻线是正式门槛">
          <div className={c('pa-support-label')}>正式门槛 2 位</div>
          <div className={c('pa-support-bars')} aria-hidden="true">{Array.from({ length: 8 }, (_, i) => <i key={i} className={c(i < filled && 'filled', i === 1 && 'threshold')} />)}</div>
        </div>
        <div className={c('pa-small-metric')}><b>{pad(problem.comments)}</b><span>评论</span></div>
        <div className={c('pa-small-metric')}><b>{pad(problem.works)}</b><span>作品</span></div>
      </div>
      <div className={c('pa-card-footer')}><span className={c('pa-last-research')}>最近研究 {day(problem.lastResearchedAt)}</span>
        <button type="button" className={c('pa-open-record')} aria-label={`打开${problem.label}档案`} onClick={open}>打开档案 <span className={c('arrow')} aria-hidden="true">↗</span></button></div>
    </div>
  </article>;
}

export function ProblemsView({ domain, nav }: { domain: string; nav: StudyNavigation }) {
  const location = useLocation(), navigate = useNavigate(), params = new URLSearchParams(location.search);
  const status: Status = (['formal', 'candidate'] as const).find(value => value === params.get('state')) ?? 'all';
  const sort: Sort = SORTS.find(option => option.value === params.get('sort'))?.value ?? 'recent';
  const [query, setQuery] = useState(params.get('q') ?? '');
  const problems = useQuery({ queryKey: ['comment-understanding', domain, 'problems'], queryFn: ({ signal }) => readProblems(domain, signal), retry: false });
  function route(changes: Record<string, string | null>) {
    const next = new URLSearchParams(location.search);
    for (const [key, value] of Object.entries(changes)) if (value) next.set(key, value); else next.delete(key);
    void navigate(`/corpus/comments?${next}`, { replace: true, state: location.state as unknown });
  }
  const all = problems.data ?? [], formal = all.filter(p => p.state === 'formal').length;
  const needle = query.trim().toLocaleLowerCase();
  const rows = all.filter(p => (status === 'all' || p.state === status) && (!needle || `${p.label}${p.summary ?? ''}${p.problemRef}`.toLocaleLowerCase().includes(needle)))
    .sort((a, b) => sort === 'recent' ? (b.lastResearchedAt ?? '').localeCompare(a.lastResearchedAt ?? '') || a.problemRef.localeCompare(b.problemRef)
      : sort === 'authors' ? b.authors - a.authors : sort === 'comments' ? b.comments - a.comments : b.works - a.works);
  const search = () => route({ q: query.trim() || null });
  return <div className={c('pa-page')}>
    <form className={c('toolbar pa-toolbar')} role="search" aria-label="搜索用户问题" onSubmit={event => { event.preventDefault(); search(); }}>
      <div className={c('field search-field')}><label className="v7-sr-only" htmlFor="problem-search">搜索问题</label>
        <div className={c('search-shell')}><button type="submit" className={c('find')}>搜索</button>
          <input id="problem-search" type="search" value={query} placeholder="搜索问题名称或描述" autoComplete="off" onChange={event => { setQuery(event.target.value); route({ q: event.target.value.trim() || null }); }} /></div></div>
      <div className={c('field')}><SelectField label="问题状态" value={status} onChange={value => route({ state: value === 'all' ? null : value })}
        options={[{ value: 'all', label: `全部问题 · ${all.length}` }, { value: 'formal', label: `正式问题 · ${formal}` }, { value: 'candidate', label: `候选问题 · ${all.length - formal}` }]} /></div>
      <div className={c('field')}><SelectField label="排序" value={sort} onChange={value => route({ sort: value === 'recent' ? null : value })} options={SORTS} /></div>
    </form>
    {needle ? <p className={c('pa-result')} aria-live="polite">找到 {rows.length} 个问题</p> : null}
    {problems.isPending ? <p className={c('view-status')}>正在读取用户问题…</p>
      : problems.isError ? <p className={c('view-status')} role="alert">用户问题读取失败：{errorText(problems.error)} <button type="button" className={c('text-action')} onClick={() => void problems.refetch()}>重试读取</button></p>
      : <div className={c('pa-grid')}>{rows.length ? rows.map(problem => <Card key={problem.problemRef} problem={problem} open={() => nav.problem(problem.problemRef)} />)
        : <div className={c('pa-empty')}><strong>{all.length ? '没有找到匹配的问题' : '还没有来源支持的用户问题'}</strong>
          <p>{all.length ? '调整搜索词或筛选条件。' : '研究评论后，至少两位独立作者提到同一问题才会成为正式问题。'}</p>
          {all.length ? <ActionButton className={c('small')} onClick={() => { setQuery(''); route({ q: null, state: null }); }}>清除筛选</ActionButton> : null}</div>}</div>}
  </div>;
}
