import { useInfiniteQuery, type UseInfiniteQueryResult } from '@tanstack/react-query';
import { useEffect } from 'react';
import type { Json } from '../../../shared/decode';
import { errorText } from './format';
import { parseRequestPage, parseSignals, parseTargets, type RunSignal, type RunTarget } from './runDtos';
import { RunRequests } from './RunRequests';
import { RunSignals } from './RunSignals';
import { RunTargetTable } from './RunTargetTable';
import { studyKey, studyRead, type StudyNavigation, type StudyPage } from './studyApi';
import { c } from './styles';

function useRunPage<T extends { runRef: string; page: StudyPage }>(domain: string, runRef: string, panel: string, parse: (value: Json) => T) {
  return useInfiniteQuery({ queryKey: studyKey(domain, `runs/${runRef}/${panel}`), initialPageParam: null as string | null,
    queryFn: ({ signal, pageParam }) => { const params = new URLSearchParams({ limit: '50' }); if (panel !== 'requests') params.set('runRef', runRef); if (pageParam) params.set('cursor', pageParam); return studyRead(domain, panel === 'requests' ? `runs/${encodeURIComponent(runRef)}/requests?${params}` : `${panel}?${params}`, value => { const result = parse(value); if (result.runRef !== runRef) throw new Error('运行详情回执与当前选择不一致。'); return result; }, signal); },
    getNextPageParam: page => page.page.nextCursor || undefined, retry: false, gcTime: Infinity, refetchOnWindowFocus: false,
  });
}
function PageFrame({ children, query, count }: { children: React.ReactNode; query: Pick<UseInfiniteQueryResult, 'isPending' | 'isError' | 'isFetching' | 'isFetchNextPageError' | 'error' | 'hasNextPage' | 'fetchNextPage'>; count: number }) {
  if (query.isPending) return <p className={c('study-empty')}>正在读取评论研究…</p>;
  if (query.isError && !count) return <p className={c('study-empty')}>运行详情读取失败：{errorText(query.error)}</p>;
  return <><p className={c('study-run-page-status')}>当前显示 {count} 条{query.hasNextPage ? '，还有后续内容' : '，已到本次列表末尾'}</p>{children}{query.hasNextPage ? <button type="button" className={c('study-link')} data-run-load-more disabled={query.isFetching} onClick={() => { void query.fetchNextPage(); }}>{query.isFetchNextPageError ? `读取失败，重试：${errorText(query.error)}` : '加载下一页'}</button> : null}</>;
}
function useFocusedPage({ wanted, found, next, busy, failed, fetchMore }: { wanted: boolean; found: boolean; next: boolean; busy: boolean; failed: boolean; fetchMore: () => unknown }) {
  useEffect(() => { if (wanted && !found && next && !busy && !failed) fetchMore(); }, [wanted, found, next, busy, failed, fetchMore]);
  useEffect(() => { if (found) document.querySelector('[data-comment-study-content] [data-focus="true"]')?.scrollIntoView({ block: 'center', behavior: 'auto' }); }, [found]);
}
interface Props { domain: string; runRef: string; nav: StudyNavigation; focusedTarget: string | null; focusedSignal: string | null }
export function RunTargetsPane({ domain, runRef, nav, focusedTarget }: Props) {
  const query = useRunPage(domain, runRef, 'targets', parseTargets), rows: RunTarget[] = query.data?.pages.flatMap(page => page.targets) ?? [];
  useFocusedPage({ wanted: Boolean(focusedTarget), found: Boolean(focusedTarget && rows.some(row => row.targetRef === focusedTarget)), next: query.hasNextPage, busy: query.isFetching, failed: query.isError, fetchMore: query.fetchNextPage });
  return <PageFrame query={query} count={rows.length}><RunTargetTable targets={rows} runRef={runRef} focused={focusedTarget} nav={nav} /></PageFrame>;
}
export function RunSignalsPane({ domain, runRef, nav, focusedTarget, focusedSignal }: Props) {
  const query = useRunPage(domain, runRef, 'signals', parseSignals), rows: RunSignal[] = query.data?.pages.flatMap(page => page.signals) ?? [];
  useFocusedPage({ wanted: Boolean(focusedSignal || focusedTarget), found: Boolean((focusedSignal || focusedTarget) && rows.some(row => focusedSignal ? row.signalRef === focusedSignal : row.targetRef === focusedTarget)), next: query.hasNextPage, busy: query.isFetching, failed: query.isError, fetchMore: query.fetchNextPage });
  return <PageFrame query={query} count={rows.length}><RunSignals signals={rows} runRef={runRef} focusedTarget={focusedTarget} focusedSignal={focusedSignal} nav={nav} /></PageFrame>;
}
export function RunRequestsPane({ domain, runRef }: Props) {
  const query = useRunPage(domain, runRef, 'requests', parseRequestPage), rows = query.data?.pages.flatMap(page => page.requests) ?? [];
  return <PageFrame query={query} count={rows.length}><RunRequests requests={rows} runRef={runRef} domain={domain} /></PageFrame>;
}
