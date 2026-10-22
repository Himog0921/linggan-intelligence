import { type Dispatch, type SetStateAction } from 'react';
import type { OverviewPreferences } from './overviewState';
import { ObservationSeries } from './ObservationSeries';
import { OverviewMetrics } from './OverviewMetrics';
import { DomainWordCloud } from './DomainWordCloud';
import { parseOverview } from './overviewDtos';
import { useStudyRead, type StudyNavigation } from './studyApi';
import { c } from './styles';

/** V2.3 compact overview. Research methods and provenance remain accessible in Runs. */
export function Overview({ domain, nav, preferences, onPreferences }: {
  domain: string | null;
  nav: StudyNavigation;
  preferences: OverviewPreferences;
  onPreferences: Dispatch<SetStateAction<OverviewPreferences>>;
}) {
  const overview = useStudyRead(domain, 'overview', parseOverview);
  if (!domain) return <p className={c('study-empty')}>请先在页头选择观察领域。</p>;
  if (overview.isPending) return <p className={c('study-empty')}>正在读取评论研究…</p>;
  if (!overview.data) return <p className={c('study-restricted')}>总览读取失败；其他页签可单独查看已有研究结果。</p>;
  return <div className={c('study-overview')} data-compact-wordcloud="v23">
    <section className={c('study-overview-hero')} aria-label="评论研究概览">
      <OverviewMetrics overview={overview.data} candidates={() => nav.candidates('deferred_novel')}/>
      <div className={c('study-overview-right')}>
        <ObservationSeries series={overview.data.observationSeries}
          days={preferences.seriesDays} view={preferences.seriesView}
          onDays={seriesDays => onPreferences(old => ({ ...old, seriesDays }))}
          onView={seriesView => onPreferences(old => ({ ...old, seriesView }))}/>
        <DomainWordCloud domain={domain} days={preferences.seriesDays} nav={nav}
          category={preferences.wordCategory}
          onCategory={wordCategory => onPreferences(old => ({ ...old, wordCategory }))}/>
      </div>
    </section>
  </div>;
}
