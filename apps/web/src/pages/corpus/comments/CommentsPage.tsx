import { useEffect, useLayoutEffect, useState } from 'react';
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
import { StartResearch, type SelectedComment } from './StartResearch';
import type { StudyNavigation } from './studyApi';
import type { WorkRole } from './WorkPicker';
import { c } from './styles';
import { errorText } from './format';

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
  return <AppShell active="corpus" boundaryLabel="本机服务状态" crumb={crumb} meta={`${current?.name || '选择领域'} · 状态 ${status}`}>
    <title>评论研究</title><div className="v7-shell"><CorpusSideNav active="comments" domain={domainParam} foot={<>只读呈现新评论研究链路<br />自动排程保持关闭</>} />
      {shell.isError && !shell.data ? <main className={c('study-main')} aria-labelledby="comment-study-title"><h1 id="comment-study-title" className="v7-sr-only">评论研究</h1><p className={c('study-empty')} role="status">评论研究准备信息读取失败：{errorText(shell.error)} <button type="button" onClick={() => { void shell.refetch(); }}>重试读取</button></p></main> : <Workspace key={domain ?? 'unselected'} domain={domain} domainName={current?.name || '当前领域'} loading={shell.isPending} shellError={shell.isError ? errorText(shell.error) : ''} retryShell={() => { void shell.refetch(); }} />}
    </div>
  </AppShell>;
}

function Workspace({ domain, domainName, loading, shellError, retryShell }: { domain: string | null; domainName: string; loading: boolean; shellError: string; retryShell: () => void }) {
  const location = useLocation(), navigate = useNavigate(), params = new URLSearchParams(location.search), view = activeView(params.get('view'));
  const [overviewPreferences, setOverviewPreferences] = useState(initialOverviewPreferences);
  const [savedViews, setSavedViews] = useState<Partial<Record<View, { search: string; state: unknown }>>>({});
  const [selectedComments, setSelectedComments] = useState<SelectedComment[]>([]), [selectedWorks, setSelectedWorks] = useState<WorkRole[]>([]);
  const [study, setStudy] = useState<{ open: boolean; kind: 'works' | 'comments'; opening: number }>({ open: false, kind: 'works', opening: 0 }), [feedback, setFeedback] = useState<{ runRef: string; text: string } | null>(null);
  const noticeRoute = `${view}:${params.get('runRef') || ''}`;
  const [lastNoticeRoute, setLastNoticeRoute] = useState(noticeRoute);
  if (lastNoticeRoute !== noticeRoute) { setLastNoticeRoute(noticeRoute); if (feedback && (view !== 'runs' || params.get('runRef') !== feedback.runRef)) setFeedback(null); }
  const [setupStatus, setSetupStatus] = useState<{ text: string; kind: string }>({ text: '', kind: 'info' });
  const [usageRefs] = useState(() => new Map<string, string>()), [recoveryRefs] = useState(() => new Map<string, string>());
  const queryClient = useQueryClient();
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
    view: next => { setFeedback(null); const to = destination(next); void navigate(to.url, { state: to.state }); },
    run: (runRef, panel = 'targets', targetRef = null, signalRef = null) => { setFeedback(null); const to = destination('runs', { runRef, panel, targetRef, signalRef }); void navigate(to.url, { state: to.state }); },
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
  return <main className={c('study-main')} aria-labelledby="comment-study-title"><h1 id="comment-study-title" className="v7-sr-only">评论研究</h1><header className={c('study-toolbar')} data-comment-study-toolbar><nav className={c('study-tabs')} aria-label="评论研究视图">{views.map(([key, title]) => <button key={key} type="button" data-view={key} aria-current={view === key ? 'page' : undefined} onClick={() => nav.view(key)}>{title}</button>)}</nav><div className={c('study-toolbar-actions')}><span id="setup-status" className={c('study-setup-status')} role="status" data-kind={setupStatus.kind} hidden={!setupStatus.text}>{setupStatus.text}</span><button type="button" id="open-study-dialog" className={c('study-primary-action')} disabled={!domain} onClick={() => setStudy(old => ({ open: true, kind: 'works', opening: old.opening + 1 }))}>发起研究</button></div></header>
    {shellError ? <p className={c('study-table-status')} role="status">领域信息刷新失败；当前显示上次读取结果。{shellError} <button type="button" onClick={retryShell}>重新读取领域信息</button></p> : null}
    {domain && view === 'runs' ? <RunPicker domain={domain} selected={params.get('runRef')} nav={nav} /> : <div id="study-run-picker" className={c('study-run-picker')} hidden />}
    <section id="study-tab-result" aria-live="polite" data-comment-study-content aria-busy={loading}>
      {loading ? <p className={c('study-empty')}>正在读取评论研究…</p> : !domain ? <p className={c('study-empty')}>请先在页头选择观察领域，再查看评论研究。</p> : view === 'comments' ? <CommentsView domain={domain} nav={nav} selected={selectedComments} onSelected={setSelectedComments} onStudy={() => setStudy(old => ({ open: true, kind: 'comments', opening: old.opening + 1 }))} requestRefs={usageRefs} closeDetail={closeComment} /> : view === 'runs' ? <RunsView domain={domain} nav={nav} recoveryRefs={recoveryRefs} initialFeedback={feedback?.runRef === params.get('runRef') ? feedback.text : ''} /> : view === 'problems' ? <ProblemsView domain={domain} nav={nav} /> : <Overview domain={domain} nav={nav} preferences={overviewPreferences} onPreferences={setOverviewPreferences} />}
    </section>
    {domain ? <StartResearch domain={domain} domainName={domainName} onStatus={setSetupStatus} open={study.open} kind={study.kind} opening={study.opening} selectedComments={selectedComments} selectedWorks={selectedWorks} onSelectedWorks={setSelectedWorks} onClose={() => setStudy(old => ({ ...old, open: false }))} onCreated={(runRef, text) => { setFeedback({ runRef, text }); void queryClient.invalidateQueries({ queryKey: ['comment-study', domain, 'runs-directory'] }); setStudy(old => ({ ...old, open: false })); const to = destination('runs', { runRef, panel: 'targets', targetRef: null, signalRef: null }); void navigate(to.url, { state: to.state }); }} /> : null}
  </main>;
}
