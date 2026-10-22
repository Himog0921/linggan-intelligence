import { useInfiniteQuery } from '@tanstack/react-query';
import { type Dispatch, type SetStateAction, type ReactNode } from 'react';
import type { OverviewPreferences } from './overviewState';
import { errorText, num } from './format';
import { ObservationSeries } from './ObservationSeries';
import { DomainWordCloud } from './DomainWordCloud';
import { OverviewMetrics } from './OverviewMetrics';
import { parseCurrentSignals, parseOverview, type VoicePreview } from './overviewDtos';
import { studyKey, studyRead, useStudyRead, type StudyNavigation } from './studyApi';
import { c } from './styles';

function LinkButton({ children, onClick }: { children: ReactNode; onClick: () => void }) {
  return <button className={c('study-link')} type="button" onClick={onClick}>{children}</button>;
}
function Voice({ item, nav }: { item: VoicePreview; nav: StudyNavigation }) {
  return <article className={c('study-voice')}>
    {item.sourceState === 'known' ? <><blockquote>“{item.commentText || '原声暂不可读'}”</blockquote>{item.proposition ? <p title="研究提取的用户表达，未经独立验证">{item.proposition}</p> : null}</> : <p className={c('study-restricted')}>来源当前受限，原声与研究衍生文本不显示。</p>}
    <div className={c('study-voice-footer')}><span>{item.workTitle || '来源作品'}</span>{item.commentKey ? <LinkButton onClick={() => nav.comment(item.commentKey!)}>查看评论及父语境</LinkButton> : null}</div>
  </article>;
}
function CurrentSignals({ domain, kind, nav }: { domain: string; kind: string; nav: StudyNavigation }) {
  const query = useInfiniteQuery({ queryKey: studyKey(domain, `current-signals/${kind}`), initialPageParam: null as string | null,
    queryFn: ({ signal, pageParam }) => { const params = new URLSearchParams({ kind, limit: '50' }); if (pageParam) params.set('cursor', pageParam); return studyRead(domain, `current-signals?${params}`, parseCurrentSignals, signal); },
    getNextPageParam: last => last.page.nextCursor || undefined, retry: false, gcTime: Infinity, refetchOnWindowFocus: false,
  });
  const title = kind === 'solution' ? '解决办法' : '使用经历';
  const items = query.data?.pages.flatMap(page => page.signals) ?? [], total = query.data?.pages.at(-1)?.page.totalCount;
  return <div className={c('study-current-signals')} aria-label={`全部${title}`}><h3>全部{title}</h3><p className={c('study-section-note')}>按当前有效信号逐条分页；同一评论的多条信号会分别出现。{total == null ? '总数暂不可读' : `共 ${num(total)} 条`}，已加载 {items.length} 条{query.hasNextPage ? '，还有后续内容' : ''}。</p>
    {query.isError ? <><p className={c('study-restricted')}>完整列表读取失败：{errorText(query.error)}</p><LinkButton onClick={() => { void (items.length ? query.fetchNextPage() : query.refetch()); }}>重试读取</LinkButton></> : items.length ? items.map((item, i) => <div className={c('study-current-signal')} key={item.signalRef || i}><Voice item={item} nav={nav} />{item.runRef ? <LinkButton onClick={() => nav.run(item.runRef!)}>查看研究批次</LinkButton> : null}</div>) : !query.isFetching ? <p className={c('study-empty')}>当前没有可读的此类信号。</p> : null}
    {query.isFetching ? <p role="status">正在读取…</p> : null}{query.hasNextPage && !query.isError ? <LinkButton onClick={() => { void query.fetchNextPage(); }}>加载更多</LinkButton> : null}
  </div>;
}

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
  const { method, directory } = preferences;
  const knowledge = overview.data.knowledgeSummary;
  const problems = knowledge?.problemSupportPreview ?? [];
  const methods = knowledge?.[method === 'solution' ? 'solutionPreview' : 'experiencePreview'] ?? [];
  const hasMethods = Boolean(knowledge?.solutionSignalCount || knowledge?.experienceSignalCount);
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
    {problems.length ? <section className={c('study-overview-section')} aria-labelledby="study-problem-title"><div className={c('study-section-head')}><h2 id="study-problem-title">近期获得新依据的问题</h2><LinkButton onClick={() => nav.view('problems')}>查看用户问题</LinkButton></div><p className={c('study-section-note')}>近 {num(knowledge?.supportWindowDays)} 天新增不同评论依据；展示当前可读的最多 3 个问题，不按需求强度排序。</p>{problems.length ? <ol className={c('study-problem-list')}>{problems.map((item, index) => <li className={c('study-problem-item')} key={item.problemRef}><div className={c('study-problem-top')}><span className={c('study-problem-index')}>0{index + 1}</span><h3 className={c('study-problem-title')}>{item.title || '问题标题未记录'}</h3></div><p className={c('study-problem-meta')}>近 28 天新增 {num(item.addedSupportCommentCount)} 条不同评论依据 · 当前支持 {num(item.supportCommentCount)} 条</p>{item.voice?.sourceState === 'known' ? <blockquote className={c('study-problem-quote')}>“{item.voice.commentText || ''}”</blockquote> : <p className={c('study-restricted')}>原声当前不可读。</p>}<div className={c('study-link-actions')}><LinkButton onClick={() => nav.problem(item.problemRef)}>查看问题定义与依据</LinkButton>{item.voice?.commentKey ? <LinkButton onClick={() => nav.comment(item.voice!.commentKey!)}>查看原声</LinkButton> : null}</div></li>)}</ol> : null}</section> : null}
    {hasMethods ? <section className={c('study-overview-section')} aria-labelledby="study-method-title"><div className={c('study-section-head')}><h2 id="study-method-title">用户提到的办法与经历</h2><LinkButton onClick={() => onPreferences(old => ({ ...old, directory: !old.directory }))}>{directory ? '收起完整列表' : `查看全部${method === 'solution' ? '解决办法' : '使用经历'}`}</LinkButton></div><div className={c('study-method-switcher')} role="tablist" aria-label="办法与经历">{([['solution', '解决办法'], ['experience', '使用经历']] as const).map(([kind, title]) => <button type="button" role="tab" data-method-kind={kind} aria-selected={method === kind} key={kind} onClick={() => { onPreferences(old => ({ ...old, method: kind, directory: false })); }}>{title}</button>)}</div><p className={c('study-section-note')}>当前 {num(knowledge?.[method === 'solution' ? 'solutionSignalCount' : 'experienceSignalCount'])} 条信号，覆盖 {num(knowledge?.[method === 'solution' ? 'solutionWorkCount' : 'experienceWorkCount'])} 篇作品。来自用户自述，不代表相关做法已经验证有效或安全。</p>{methods.length ? <div className={c('study-voice-list')}>{methods.map((item, i) => <Voice key={item.signalRef || i} item={item} nav={nav} />)}</div> : <p className={c('study-empty')}>当前没有可读预览，可查看完整列表确认。</p>}{directory ? <CurrentSignals key={method} domain={domain} kind={method} nav={nav} /> : null}</section> : null}
  </div>;
}
