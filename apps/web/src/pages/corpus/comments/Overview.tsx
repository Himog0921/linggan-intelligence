import { useQuery } from '@tanstack/react-query';
import type { Dispatch, SetStateAction } from 'react';
import { ActionButton } from '../../../shared/ActionButton';
import { readResearchOverview } from './commentResearchApi';
import { ObservationSeries } from './ObservationSeries';
import type { SeriesRange } from './researchModel';
import { OverviewKnowledge, OverviewMetrics } from './OverviewMetrics';
import { DomainWordCloud, type Category } from './DomainWordCloud';
import { CrIcon } from './CrIcon';
import { signalKindLabel } from './labels';
import type { StudyNavigation } from './studyApi';
import { errorText, num } from './format';
import { c } from './styles';

export interface OverviewPreferences { seriesDays: SeriesRange; seriesView: 'chart' | 'table'; wordCategory: Category }
const kindLabel = (kind: string) => signalKindLabel[kind] ?? ({ scenario: '困难场景' } as Record<string, string>)[kind] ?? '研究信号';

export function Overview({ domain, nav, preferences, onPreferences }: { domain: string; nav: StudyNavigation; preferences: OverviewPreferences; onPreferences: Dispatch<SetStateAction<OverviewPreferences>> }) {
  const query = useQuery({ queryKey: ['comment-research', domain, 'overview'], queryFn: ({ signal }) => readResearchOverview(domain, signal), retry: false });
  const overview = query.data, knowledge = overview?.knowledgeSummary;
  // Source-derived text (problem labels, signal propositions) is withheld while a refresh is pending or failed: qualification may have changed.
  const lower = !query.isError && !query.isFetching ? overview : undefined;
  const problems = { formal: knowledge?.problemCount ?? 0, candidate: knowledge?.pendingCandidateCount ?? 0 };
  const wordDays = preferences.seriesDays === 'fit' ? 56 : preferences.seriesDays;
  return <section aria-label="评论研究概览">
    {query.isError ? <p className={c('view-status')} role="alert">概览读数刷新失败：{errorText(query.error)} <button type="button" className={c('text-action')} onClick={() => void query.refetch()}>重试读取</button></p> : null}
    <div className={c('overview-top')}>
      {overview ? <OverviewMetrics overview={overview} /> : <div className={c('ov-cell metric-col')}><p className={c('view-status')}>{query.isPending ? '正在读取评论研究读数…' : '概览读数暂不可用；用户评论仍可单独查看。'}</p></div>}
      <ObservationSeries series={overview?.observationSeries ?? null} range={preferences.seriesDays} view={preferences.seriesView}
        onRange={seriesDays => onPreferences(old => ({ ...old, seriesDays }))} onView={seriesView => onPreferences(old => ({ ...old, seriesView }))} />
      {overview ? <OverviewKnowledge overview={overview} problems={problems} candidates={() => nav.view('problems', { state: 'candidate', q: null })} /> : <div className={c('ov-cell ov-row2 metric-col')} />}
      <DomainWordCloud domain={domain} days={wordDays} nav={nav} category={preferences.wordCategory} onCategory={wordCategory => onPreferences(old => ({ ...old, wordCategory }))} />
    </div>
    {lower ? <div className={c('overview-lower')}>
      <section aria-labelledby="overview-problems-title"><h2 id="overview-problems-title" className={c('section-title')}>已有依据的用户问题 <small>按独立作者支持</small></h2>
        {lower.knowledgeSummary.problemPreview.length ? lower.knowledgeSummary.problemPreview.slice(0, 2).map(problem => <div key={problem.problemRef} className={c('problem-row')}>
          <div><p>{problem.label}</p><small>{problem.state === 'candidate' ? '候选问题' : '正式问题'} · {num(problem.authors)} 位作者 · {num(problem.works)} 篇作品</small></div>
          <ActionButton variant="quiet" className={c('quiet-small icon-gap')} aria-label={`查看${problem.label}的证据`} onClick={() => nav.problem(problem.problemRef)}>证据 <CrIcon name="ArrowUpRight" /></ActionButton>
        </div>) : <p className={c('view-status')}>当前尚无正式问题。至少两位独立作者的有效原声支持才会形成正式问题。</p>}
      </section>
      <section aria-labelledby="overview-signals-title"><h2 id="overview-signals-title" className={c('section-title')}>近期研究信号 <small>依据评论原声</small></h2>
        {lower.recentSignals?.length ? lower.recentSignals.slice(0, 3).map(signal => <div key={signal.requestRef + signal.proposition} className={c('signal-row')}>
          <div><p>{signal.proposition}</p><small>{[kindLabel(signal.kind), signal.authorDisplayName].filter(Boolean).join(' · ')}</small></div>
          <ActionButton variant="quiet" className={c('quiet-small icon-gap')} aria-label={`查看${signal.authorDisplayName ?? '这条评论'}的原声`} onClick={() => nav.comment(signal.commentKey, { requestRef: signal.requestRef })}>原声 <CrIcon name="ArrowUpRight" /></ActionButton>
        </div>) : <p className={c('view-status')}>{lower.recentSignals ? '暂无近期研究信号。' : '近期研究信号暂不可读取。'}</p>}
      </section>
    </div> : null}
  </section>;
}
