import { useEffect, useLayoutEffect, useState, type ReactNode } from 'react';
import { useLocation, useNavigate } from 'react-router';
import { useQueryClient } from '@tanstack/react-query';
import { AppShell } from '../../../app/AppShell';
import { CorpusSideNav } from '../../../corpus/CorpusSideNav';
import { DomainPicker } from '../../../corpus/DomainPicker';
import { useCorpusShell } from '../../../corpus/useCorpusShell';
import { CommentsView } from './CommentsView';
import { Overview } from './Overview';
import { initialOverviewPreferences } from './overviewState';
import { ProblemsView } from './ProblemsView';
import { RunPicker, RunsView } from './RunsView';
import { AiBusinessStatus } from '../../settings/AiBusinessStatus';
import { PolicyHistory } from './PolicyHistory';
import type { SelectedComment } from './studyApi';
import type { StudyNavigation } from './studyApi';
import { c } from './styles';
import { errorText, num } from './format';
import type { CommentCounts } from './CommentsView';

type View = 'overview' | 'comments' | 'runs' | 'problems';
const views: readonly [View, string][] = [['overview', '概览'], ['comments', '用户评论'], ['runs', '研究批次'], ['problems', '用户问题']];
const activeView = (value: string | null): View => value === 'targets' ? 'runs' : value === 'pending' ? 'problems' : views.some(([key]) => key === value) ? value as View : 'overview';

export function CommentsPage() {
  const location = useLocation(), domainParam = new URLSearchParams(location.search).get('domain');
  const shell = useCorpusShell(domainParam), domain = shell.data?.selectedDomain ?? null;
  const current = shell.data?.domains.find(row => row.domainRef === domain);
  const picker = shell.data && (shell.data.domains.length >= 2 || !current) ? <DomainPicker domains={shell.data.domains} selected={domain} action="/corpus/comments" /> : null;
  const crumb = picker ? <>语料 <span className="v7-slash">/</span> {picker} <span className="v7-slash">/</span> <b>评论研究</b></> : <>语料 <span className="v7-slash">/</span> <b>评论研究</b></>;
  const status = current?.status === 'active' ? '运行中' : current?.status === 'paused' ? '已暂停' : current?.status || '未知';
  return <Workspace key={domain ?? 'unselected'} domain={domain} domainName={current?.name || '当前领域'} domainStatus={status} collectionState={shell.data?.collectionState} crumb={crumb} loading={shell.isPending} shellError={shell.isError ? errorText(shell.error) : ''} fatalShellError={shell.isError && !shell.data} retryShell={() => { void shell.refetch(); }} />;
}

function Workspace({ domain, domainName, domainStatus, collectionState, crumb, loading, shellError, fatalShellError, retryShell }: { domain: string | null; domainName: string; domainStatus: string; collectionState?: string; crumb: ReactNode; loading: boolean; shellError: string; fatalShellError: boolean; retryShell: () => void }) {
  const location = useLocation(), navigate = useNavigate(), params = new URLSearchParams(location.search), view = activeView(params.get('view'));
  const [overviewPreferences, setOverviewPreferences] = useState(initialOverviewPreferences);
  const [savedViews, setSavedViews] = useState<Partial<Record<View, { search: string; state: unknown }>>>({});
  const [selectedComments, setSelectedComments] = useState<SelectedComment[]>([]);
  const [commentCounts, setCommentCounts] = useState<CommentCounts | null>(null);
  const queryClient = useQueryClient();
  const [usageRefs] = useState(() => new Map<string, string>());
  useLayoutEffect(() => { if (window.scrollY) window.scrollTo(0, 0); }, []);
  useEffect(() => () => { if (domain) queryClient.removeQueries({ queryKey: ['comment-study', domain] }); }, [domain, queryClient]);
  function destination(next: View, extra: Record<string, string | null> = {}, stateExtra: Record<string, unknown> = {}) {
    const saved = view === next ? { search: location.search, state: location.state as unknown } : savedViews[next];
    const target = new URLSearchParams(saved?.search || '');
    target.set('view', next); if (domain) target.set('domain', domain);
    const fields: Record<View, string[]> = { overview: [], comments: ['q', 'workRef', 'state', 'signalKind', 'cursor', 'voiceRole', 'sort', 'minLikes', 'postUsage'], runs: ['runRef', 'panel', 'targetRef', 'signalRef'], problems: ['problemRef', 'state', 'q', 'sort', 'cursor'] };
    for (const key of [...target.keys()]) if (!['view', 'domain', ...fields[next]].includes(key)) target.delete(key);
    for (const [key, value] of Object.entries(extra)) if (value) target.set(key, value); else target.delete(key);
    setSavedViews(old => ({ ...old, [view]: { search: location.search, state: location.state as unknown } }));
    return { url: `/corpus/comments?${target}`, state: { ...(saved?.state && typeof saved.state === 'object' ? saved.state : {}), ...stateExtra } };
  }
  const nav: StudyNavigation = {
    view: next => { const to = destination(next); void navigate(to.url, { state: to.state }); },
    run: (runRef, panel = 'targets', targetRef = null, signalRef = null) => { const to = destination('runs', { runRef, panel, targetRef, signalRef }); void navigate(to.url, { state: to.state }); },
    problem: problemRef => { const to = destination('problems', { problemRef }); void navigate(to.url, { state: to.state }); },
    candidates: state => { const to = destination('problems', { state: 'clues', cursor: null, q: null, problemRef: null }, { problemResults: { domain, view: 'clues', q: '', order: 'recent', cursor: null, candidate: state, density: 'standard' } }); void navigate(to.url, { state: to.state }); },
    comment: key => {
      const to = destination('comments', { commentWorkRef: key.workRef, commentExternalId: key.commentExternalId, detail: 'comment' }, { commentDetail: true, commentDomain: domain });
      void (async () => { await navigate(location.pathname + location.search, { replace: true, state: { ...(location.state as object | null), commentScrollY: window.scrollY } }); await navigate(to.url, { state: to.state }); })();
    },
  };
  function closeComment() {
    if ((location.state as { commentDetail?: boolean } | null)?.commentDetail) { void navigate(-1); return; }
    const next = new URLSearchParams(location.search); for (const key of ['commentWorkRef', 'commentExternalId', 'detail']) next.delete(key);
    void navigate(`/corpus/comments?${next}`, { replace: true, state: location.state as unknown });
  }
  const contextNav = <nav className="v7-tertiary-nav" data-comment-study-context aria-label="评论研究视图">{views.map(([key, label]) => <button key={key} type="button" aria-current={view === key ? 'page' : undefined} onClick={() => nav.view(key)}>{label}</button>)}<select aria-label="切换评论研究视图" className="v7-tertiary-select" value={view} onChange={event => nav.view(event.target.value as View)}>{views.map(([key, label]) => <option key={key} value={key}>{label}</option>)}</select></nav>;
  const contextMeta = view === 'comments' && domain ? <details className="v7-context-count">
    <summary aria-label="查看当前筛选评论统计">当前筛选可研究 <b>{num(commentCounts?.eligibleCommentCount)}</b> 条 <span aria-hidden="true">⌄</span></summary>
    <div className="v7-context-count-detail" role="group" aria-label="当前筛选评论统计">
      <p>可显示 <b>{num(commentCounts?.displayableCommentCount)}</b> 条</p>
      <p>可研究 <b>{num(commentCounts?.eligibleCommentCount)}</b> 条</p>
      <p>已索引 <b>{num(commentCounts?.indexedCount)}</b> 条</p>
      <p>待索引 <b>{num(commentCounts?.pendingCount)}</b> 条</p>
      <small>— 表示当前读数未获确认；不是零。</small>
    </div>
  </details> : `${domainName} · 状态 ${domainStatus}`;
  return <AppShell active="corpus" collectionState={collectionState} boundaryLabel="本机服务状态" crumb={crumb} contextNav={contextNav} meta={contextMeta}>
    <title>评论研究</title><div className="v7-shell"><CorpusSideNav active="comments" domain={domain} foot={<>只读呈现新评论研究链路<br />自动排程保持关闭</>} />
    {fatalShellError ? <main className={c('study-main')} aria-labelledby="comment-study-title"><h1 id="comment-study-title" className="v7-sr-only">评论研究</h1><p className={c('study-empty')} role="status">评论研究准备信息读取失败：{shellError} <button type="button" onClick={retryShell}>重试读取</button></p></main> :
    <main className={c('study-main')} aria-labelledby="comment-study-title"><h1 id="comment-study-title" className="v7-sr-only">评论研究</h1>
    <AiBusinessStatus taskKey="comment_study" />
    {domain ? <PolicyHistory domain={domain} /> : null}
    {shellError ? <p className={c('study-table-status')} role="status">领域信息刷新失败；当前显示上次读取结果。{shellError} <button type="button" onClick={retryShell}>重新读取领域信息</button></p> : null}
    {domain && view === 'runs' ? <RunPicker domain={domain} selected={params.get('runRef')} nav={nav} /> : <div id="study-run-picker" className={c('study-run-picker')} hidden />}
    <section id="study-tab-result" className={view === 'comments' ? c('study-tab-comments') : undefined} aria-live="polite" data-comment-study-content aria-busy={loading}>
      {loading ? <p className={c('study-empty')}>正在读取评论研究…</p> : !domain ? <p className={c('study-empty')}>请先在页头选择观察领域，再查看评论研究。</p> : view === 'comments' ? <CommentsView domain={domain} nav={nav} selected={selectedComments} onSelected={setSelectedComments} onCounts={setCommentCounts} requestRefs={usageRefs} closeDetail={closeComment} /> : view === 'runs' ? <RunsView domain={domain} nav={nav} initialFeedback="" /> : view === 'problems' ? <ProblemsView domain={domain} nav={nav} /> : <Overview domain={domain} nav={nav} preferences={overviewPreferences} onPreferences={setOverviewPreferences} />}
    </section>
    </main>}</div></AppShell>;
}
