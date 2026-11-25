import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { useLocation, useNavigate } from 'react-router';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { AppShell } from '../../../app/AppShell';
import { ContextBreadcrumb } from '../../../shared/ContextBar';
import { CorpusSideNav } from '../../../corpus/CorpusSideNav';
import { DomainPicker } from '../../../corpus/DomainPicker';
import { useCorpusShell } from '../../../corpus/useCorpusShell';
import { CommentsView } from './CommentsView';
import { Overview, type OverviewPreferences } from './Overview';
import { ProblemsView } from './ProblemsView';
import { CommentDrawer } from './CommentDrawer';
import { ProblemDrawer } from './ProblemDrawer';
import { ResearchRecords } from './ResearchRecords';
import { recordsActivity } from './researchModel';
import { ResearchPreview } from './ResearchPreview';
import { CrIcon } from './CrIcon';
import type { SelectedComment, StudyNavigation, StudyView } from './studyApi';
import { readRuns } from './understandingApi';
import { c } from './styles';
import { errorText } from './format';

const LABELS: Record<StudyView, string> = { overview: '概览', comments: '用户评论', problems: '用户问题' };
/** Old links: `view=runs|targets` open the records dialog over the overview; `view=pending` was the problem list. */
const parseView = (value: string | null): StudyView => value === 'comments' ? 'comments' : value === 'problems' || value === 'pending' ? 'problems' : 'overview';
const legacyRecords = (value: string | null) => value === 'runs' || value === 'targets';
const DRAWER_KEYS = ['detail', 'commentWorkRef', 'commentExternalId', 'researchRequestRef', 'problemRef'];
const VIEW_FIELDS: Record<StudyView, string[]> = {
  overview: [],
  comments: ['q', 'searchScope', 'workRef', 'state', 'signalKind', 'cursor', 'voiceRole', 'sort', 'minLikes', 'postUsage', 'page', 'size', 'supportOf'],
  problems: ['state', 'q', 'sort'],
};

export function CommentsPage() {
  const location = useLocation(), params = new URLSearchParams(location.search), domainParam = params.get('domain');
  const shell = useCorpusShell(domainParam), domain = shell.data?.selectedDomain ?? null;
  const current = shell.data?.domains.find(row => row.domainRef === domain);
  const view = parseView(params.get('view'));
  const picker = shell.data && (shell.data.domains.length >= 2 || !current) ? <DomainPicker domains={shell.data.domains} selected={domain} action="/corpus/comments" /> : null;
  const crumb = <ContextBreadcrumb items={picker ? ['语料', picker, '评论研究', LABELS[view]] : ['语料', '评论研究', LABELS[view]]} />;
  return <Workspace key={domain ?? 'unselected'} domain={domain} crumb={crumb} loading={shell.isPending} shellError={shell.isError ? errorText(shell.error) : ''} fatalShellError={shell.isError && !shell.data} retryShell={() => { void shell.refetch(); }} />;
}

/** The toast joins the top-most open modal (after this render commits), otherwise the modal's inert backdrop would hide it. */
function Toast({ message, host }: { message: string; host: Element }) {
  return createPortal(<div className={c('toast')} role="status" aria-live="polite">{message}</div>, host);
}

interface PreviewRequest { selected: SelectedComment[]; rerun: boolean; submitted?: () => void }

function Workspace({ domain, crumb, loading, shellError, fatalShellError, retryShell }: { domain: string | null; crumb: ReactNode; loading: boolean; shellError: string; fatalShellError: boolean; retryShell: () => void }) {
  const location = useLocation(), navigate = useNavigate(), params = new URLSearchParams(location.search);
  const rawView = params.get('view'), view = parseView(rawView);
  const state = (location.state ?? {}) as { drawer?: boolean; records?: boolean };
  const [savedViews, setSavedViews] = useState<Partial<Record<StudyView, { search: string; state: unknown }>>>({});
  const [selectedComments, setSelectedComments] = useState<SelectedComment[]>([]);
  const [overviewPreferences, setOverviewPreferences] = useState<OverviewPreferences>({ seriesDays: 'fit', seriesView: 'chart', wordCategory: 'all' });
  const [preview, setPreview] = useState<PreviewRequest | null>(null);
  const [drawerHistory, setDrawerHistory] = useState<string[]>([]);
  const [toast, setToast] = useState<{ message: string; id: number; host: Element } | null>(null);
  const toastTimer = useRef<number | null>(null);
  const queryClient = useQueryClient();
  const [usageRefs] = useState(() => new Map<string, string>());
  const main = useRef<HTMLElement>(null);
  useLayoutEffect(() => { if (window.scrollY) window.scrollTo(0, 0); }, []);
  useEffect(() => () => { if (domain) queryClient.removeQueries({ queryKey: ['comment-study', domain] }); }, [domain, queryClient]);
  useEffect(() => () => { if (toastTimer.current !== null) window.clearTimeout(toastTimer.current); }, []);
  const runs = useQuery({
    queryKey: ['comment-understanding', domain, 'runs'], enabled: Boolean(domain),
    queryFn: ({ signal }) => readRuns(domain!, signal), retry: false,
    refetchInterval: query => query.state.data?.runs.some(run => run.unitCounts ? run.unitCounts.pending > 0 : run.state === 'open') ||
      query.state.data?.consolidations.some(item => ['pending', 'running'].includes(item.state)) ? 2500 : false,
  });

  function toastMessage(message: string) {
    // Pick the host on the next task, after a dialog that is closing in this same event has left the DOM.
    window.setTimeout(() => setToast(old => ({ message, id: (old?.id ?? 0) + 1, host: [...document.querySelectorAll('dialog[open]')].at(-1) ?? document.body })), 0);
    if (toastTimer.current !== null) window.clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToast(null), 3500);
  }
  function go(next: URLSearchParams, replace: boolean, extraState: Record<string, unknown> = {}) {
    void navigate(`/corpus/comments?${next}`, { replace, state: { ...(location.state as object | null), ...extraState } });
  }
  function destination(next: StudyView, extra: Record<string, string | null> = {}) {
    const saved = view === next && !legacyRecords(rawView) ? { search: location.search, state: location.state as unknown } : savedViews[next];
    const target = new URLSearchParams(saved?.search || '');
    target.set('view', next); if (domain) target.set('domain', domain);
    for (const key of [...target.keys()]) if (!['view', 'domain', ...VIEW_FIELDS[next]].includes(key)) target.delete(key);
    for (const [key, value] of Object.entries(extra)) if (value) target.set(key, value); else target.delete(key);
    setSavedViews(old => ({ ...old, [view]: { search: location.search, state: location.state as unknown } }));
    return { url: `/corpus/comments?${target}`, state: saved?.state && typeof saved.state === 'object' ? saved.state : {} };
  }
  /** Drawer moves: opening from the page pushes one history entry; related moves replace it and remember where they came from. */
  function openDrawer(fields: Record<string, string | null>, related: boolean) {
    const next = new URLSearchParams(location.search);
    for (const key of DRAWER_KEYS) next.delete(key);
    for (const [key, value] of Object.entries(fields)) if (value) next.set(key, value);
    const open = drawerOpen;
    if (open && related) setDrawerHistory(old => [...old, location.search]);
    else if (!open) setDrawerHistory([]);
    go(next, open, open ? {} : { drawer: true });
  }
  function closeDrawer() {
    setDrawerHistory([]);
    if (state.drawer) { void navigate(-1); return; }
    const next = new URLSearchParams(location.search);
    for (const key of DRAWER_KEYS) next.delete(key);
    go(next, true);
  }
  function drawerBack() {
    const previous = drawerHistory.at(-1);
    if (previous === undefined) return;
    setDrawerHistory(old => old.slice(0, -1));
    void navigate(`/corpus/comments${previous}`, { replace: true, state: location.state as unknown });
  }
  const recordsOpen = params.get('records') === '1' || legacyRecords(rawView);
  function openRecords(runRef?: string | null) {
    const next = new URLSearchParams(location.search);
    for (const key of DRAWER_KEYS) next.delete(key);
    next.set('records', '1');
    if (runRef) next.set('runRef', runRef); else if (!recordsOpen) next.delete('runRef');
    setDrawerHistory([]);
    go(next, recordsOpen, recordsOpen ? {} : { records: true });
  }
  function closeRecords() {
    if (state.records && !legacyRecords(rawView)) { void navigate(-1); return; }
    const next = new URLSearchParams(location.search);
    for (const key of ['records', 'runRef', 'panel', 'targetRef', 'signalRef']) next.delete(key);
    if (legacyRecords(rawView)) next.set('view', 'overview');
    go(next, true, { records: false });
  }
  const nav: StudyNavigation = {
    view: (next, extra) => { const to = destination(next, extra); void navigate(to.url, { state: to.state }); },
    comment: (key, options) => openDrawer({ detail: 'comment', commentWorkRef: key.workRef, commentExternalId: key.commentExternalId, researchRequestRef: options?.requestRef ?? null }, Boolean(options?.related)),
    problem: (problemRef, options) => { if (problemRef) openDrawer({ detail: 'problem', problemRef }, Boolean(options?.related)); else closeDrawer(); },
    records: openRecords,
    preview: (selected, options) => setPreview({ selected, rerun: Boolean(options?.rerun), ...(options?.submitted ? { submitted: options.submitted } : {}) }),
    toast: toastMessage,
  };

  const detail = params.get('detail');
  const commentKey = (detail === 'comment' || params.get('commentWorkRef')) && params.get('commentExternalId')
    ? { workRef: params.get('commentWorkRef') || params.get('workRef') || '', commentExternalId: params.get('commentExternalId')! } : null;
  const problemRef = !commentKey ? params.get('problemRef') : null;
  const drawerOpen = Boolean((commentKey && commentKey.workRef) || problemRef);
  const activity = recordsActivity(runs.data?.runs ?? []);
  const entry = domain ? <button type="button" className={c('record-entry')} aria-haspopup="dialog" aria-expanded={recordsOpen}
    aria-label={`研究记录${activity.active ? `，${activity.active}项进行中` : ''}${activity.failed ? `，${activity.failed}项有失败` : ''}`} onClick={() => openRecords()}>
    <CrIcon name="History" /><span>研究记录</span>
    <span className={c('entry-status')}>{activity.active ? `${activity.active} 项进行中` : activity.failed ? `${activity.failed} 项有失败` : ''}</span>
  </button> : null;

  return <AppShell active="corpus" boundaryLabel="本机服务状态" contextBar={{ breadcrumb: crumb, meta: entry }}>
    <title>{`${LABELS[view]} · 评论研究`}</title><div className="v7-shell"><CorpusSideNav active="comments" domain={domain} view={view} onViewSelect={next => nav.view(next)} foot={<>评论理解 · 用户问题</>} />
    <main ref={main} className={c('cr-main')} aria-labelledby="comment-study-title" tabIndex={-1}>
      <h1 id="comment-study-title" className="v7-sr-only">评论研究 · {LABELS[view]}</h1>
      {fatalShellError ? <p className={c('view-status')} role="status">评论研究准备信息读取失败：{shellError} <button type="button" className={c('text-action')} onClick={retryShell}>重试读取</button></p> : <>
        {shellError ? <p className={c('view-status')} role="status">领域信息刷新失败；当前显示上次读取结果。{shellError} <button type="button" className={c('text-action')} onClick={retryShell}>重新读取领域信息</button></p> : null}
        <section id="study-tab-result" aria-live="polite" data-comment-study-content aria-busy={loading}>
          {loading ? <p className={c('view-status')}>正在读取评论研究…</p> : !domain ? <p className={c('view-status')}>请先在页头选择观察领域，再查看评论研究。</p>
            : view === 'comments' ? <CommentsView domain={domain} nav={nav} selected={selectedComments} onSelected={setSelectedComments} requestRefs={usageRefs} />
            : view === 'problems' ? <ProblemsView domain={domain} nav={nav} />
            : <Overview domain={domain} nav={nav} preferences={overviewPreferences} onPreferences={setOverviewPreferences} />}
        </section>
      </>}
    </main></div>
    {domain && commentKey && commentKey.workRef ? <CommentDrawer key={`${commentKey.workRef}:${commentKey.commentExternalId}`} domain={domain} commentKey={commentKey} request={params.get('researchRequestRef')} nav={nav}
      close={closeDrawer} back={drawerHistory.length ? drawerBack : null}
      filterWork={(workRef, label) => nav.view('comments', { workRef, q: label, searchScope: 'work', supportOf: null, page: null, cursor: null })} /> : null}
    {domain && problemRef ? <ProblemDrawer key={problemRef} domain={domain} problemRef={problemRef} nav={nav} close={closeDrawer} back={drawerHistory.length ? drawerBack : null}
      showComments={ref => nav.view('comments', { supportOf: ref, q: null, workRef: null, state: null, signalKind: null, minLikes: null, postUsage: null, page: null, cursor: null })} /> : null}
    {domain && recordsOpen ? <ResearchRecords domain={domain} runs={runs} runRef={params.get('runRef')} nav={nav} close={closeRecords}
      expand={runRef => { const next = new URLSearchParams(location.search); if (runRef) next.set('runRef', runRef); else next.delete('runRef'); go(next, true); }} /> : null}
    {domain && preview ? <ResearchPreview key={preview.selected.map(item => item.sourceRef).join(',') + preview.rerun} domain={domain} selected={preview.selected} initialRerun={preview.rerun} nav={nav}
      close={() => setPreview(null)} submitted={(batchRef, message) => { preview.submitted?.(); setPreview(null); void runs.refetch(); openRecords(batchRef); toastMessage(message); }} /> : null}
    {toast && toast.host.isConnected ? <Toast key={toast.id} message={toast.message} host={toast.host} /> : null}
  </AppShell>;
}
