import { humanMoment } from '../../shared/formatting';
import { DraftRail } from './DraftRail';
import { ActionButton } from '../../shared/ActionButton';
import { useEffect, useEffectEvent, useLayoutEffect, useRef, useState, type ReactNode } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useBlocker, useSearchParams } from 'react-router';
import { OpsFrame } from '../ops/OpsFrame';
import { OpsScopePicker } from '../ops/OpsScopePicker';
import { ApiError } from '../../shared/api';
import { RequestIdentities } from '../../shared/requestIdentity';
import * as api from './workbenchApi';
import { newDocument, newCanvasBlock, parseAsset, scalarRange, type AiPreview, type AiReadiness, type AiStatus, type TitleStatus, type Citation, type CanvasBlock, type CanvasGroup, type Revision, type SaveReceipt, type UseTarget, type ValidatedAnchor, type WorkDocument, type WorkEnvelope, type WorkSummary, type IdeaBoxSummary, type WritingAction, type WritingSelection } from './workbenchModel';
import { ManuscriptEditor } from './ManuscriptEditor';
import { CanvasBoard } from './CanvasBoard';
import { RevisionCompare } from './RevisionCompare';
import { TopicLibrary } from './TopicLibrary';
import { TopicDetail } from './TopicDetail';
import { CheckPanel } from './assets/CheckPanel';
import { PublicationLinker, type PublicationPending } from './assets/PublicationLinker';
import { SaveStructuralTemplate } from './assets/SaveStructuralTemplate';
import type { CreateCommand as TemplateCreateCommand } from './assets/structuralTemplatesApi';
import { AssetsLibrary } from './assets/AssetsLibrary';
import { MaterialBrowser } from './MaterialBrowser';
import { newImagePage } from './imagePageFactory';
import { ImagePages } from './ImagePages';
import styles from './WorkbenchPage.module.css';
type View = 'topic' | 'materials' | 'canvas' | 'body' | 'cover' | 'inner' | 'export';
const VIEWS: readonly { id: View; label: string }[] = [{id:'topic',label:'选题方案'},{id:'materials',label:'材料浏览'},{id:'canvas',label:'创作画布'},{id:'body',label:'正文'},{id:'cover',label:'封面'},{id:'inner',label:'内页'},{id:'export',label:'预览与导出'}];
function targetLocation(target: UseTarget, document: WorkDocument): {
  view: View;
  label: string;
} {
  switch (target.kind) {
    case 'title':
      return {
        view: 'body',
        label: '正文标题'
      };
    case 'angle':
      return {
        view: 'topic',
        label: '切入角度'
      };
    case 'outline':
      return {
        view: 'topic',
        label: target.index < document.outline.length ? `提纲第 ${target.index + 1} 项` : '提纲目标已删除'
      };
    case 'tag':
      return {
        view: 'body',
        label: target.index < document.tags.length ? `发布标签 ${target.index + 1}` : '标签目标已删除'
      };
    case 'canvasNote':
    case 'canvasExpression':
      return {
        view: 'canvas',
        label: document.canvas.some(c => c.id === target.canvasId) ? target.kind === 'canvasNote' ? '画布备注' : '画布表达' : '画布目标已删除'
      };
    case 'paragraph':
      {
        const index = document.paragraphs.findIndex(p => p.id === target.paragraphId);
        return {
          view: 'body',
          label: index < 0 ? '正文目标已删除' : `正文第 ${index + 1} 段`
        };
      }
    case 'pageBlock':
    case 'pageTitle':
    case 'pageSubtitle':
      {
        const index = document.pages.findIndex(p => p.id === target.pageId);
        const page = document.pages[index];
        const exists = !!page && (target.kind !== 'pageBlock' || page.blocks.some(b => b.id === target.blockId));
        return {
          view: page?.kind === 'cover' ? 'cover' : 'inner',
          label: exists ? `${page?.kind === 'cover' ? '封面' : `内页 ${document.pages.slice(0, index + 1).filter(item => item.kind !== 'cover').length}`}${target.kind === 'pageTitle' ? '标题' : target.kind === 'pageSubtitle' ? '副标题' : ''}` : '图页目标已删除'
        };
      }
  }
}
const SECTION_TITLE = { today: '今日选题', works: '我的创作', assets: '资产库' } as const;
function viewValue(value: string | null): View {
  return VIEWS.find(v => v.id === value)?.id ?? 'topic';
}
function Modal({
  title,
  onClose,
  children
}: {
  title: string;
  onClose: () => void;
  children: ReactNode;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const previous = window.document.activeElement;
    const dialog = ref.current;
    dialog?.showModal();
    return () => {
      dialog?.close();
      if (previous instanceof HTMLElement) previous.focus();
    };
  }, []);
  return <dialog ref={ref} aria-label={title} className={styles.dialog} onCancel={e => {
    e.preventDefault();
    onClose();
  }}>
<div className={styles.dialogHead}>
<h2>{title}</h2>
<ActionButton variant="outline" aria-label="关闭" onClick={onClose}>×</ActionButton>
</div>{children}</dialog>;
}
export function WorkbenchPage() {
  const [params, setParams] = useSearchParams();
  const domain = params.get('domain') ?? '';
  const work = params.get('work');
  const sectionParam = params.get('section');
  const section = work ? 'works' : sectionParam === 'works' || sectionParam === 'assets' ? sectionParam : 'today';
  const [title, setTitle] = useState('');
  const [immersive, setImmersive] = useState(false);
  const [error, setError] = useState('');
  const [creating, setCreating] = useState(false);
  const [createUnknown, setCreateUnknown] = useState(false);
  const [assetPending, setAssetPending] = useState(false);
  const [assetDirty, setAssetDirty] = useState(false);
  const [assetReset, setAssetReset] = useState(0);
  const [templatePendingWorks, setTemplatePendingWorks] = useState<Record<string, boolean>>({});
  const anyTemplatePending = Object.values(templatePendingWorks).some(Boolean);
  const [publicationPendingWorks, setPublicationPendingWorks] = useState<Record<string, boolean>>({});
  const anyPublicationPending = Object.values(publicationPendingWorks).some(Boolean);
  const [titlePendingWorks, setTitlePendingWorks] = useState<Record<string, boolean>>({});
  const anyTitlePending = Object.values(titlePendingWorks).some(Boolean);
  const [writingPendingWorks, setWritingPendingWorks] = useState<Record<string, boolean>>({});
  const anyWritingPending = Object.values(writingPendingWorks).some(Boolean);
  const blocker = useBlocker(assetPending || assetDirty || anyTemplatePending || anyPublicationPending || anyTitlePending || anyWritingPending);
  const markWritingPending = (workId: string, value: boolean) => setWritingPendingWorks(previous => ({ ...previous, [workId]: value }));
  const markTitlePending = (workId: string, value: boolean) => setTitlePendingWorks(previous => ({ ...previous, [workId]: value }));
  const markPublicationPending = (workId: string, value: boolean) => setPublicationPendingWorks(previous => ({ ...previous, [workId]: value }));
  const markTemplatePending = (workId: string, value: boolean) => setTemplatePendingWorks(previous => ({ ...previous, [workId]: value }));
  const [identities] = useState(() => new RequestIdentities());
  const createPending = useRef<{
    requestId: string;
    domainRef: string;
    document: WorkDocument;
  } | null>(null);
  const queryClient = useQueryClient();
  const [openWorks, setOpenWorks] = useState<WorkEnvelope[]>([]);
  const lastWork = work ?? [...openWorks].reverse().find(item => item.domainRef === domain)?.workId;
  const domainQuery = useQuery({
    queryKey: ['creation-domains'],
    queryFn: ({
      signal
    }) => api.domains(signal),
    staleTime: 0
  });
  const validDomain = domainQuery.data?.some(option => option.domainRef === domain) === true;
  useEffect(() => { window.document.title = `${work ? '作品编辑' : SECTION_TITLE[section]} · 运营`; }, [work, section]);
  const works = useQuery({
    queryKey: ['creation-works', domain],
    queryFn: ({
      signal
    }) => api.listWorks(domain, signal),
    enabled: !!domain && validDomain,
    gcTime: 0
  });
  const ideas = useQuery({ queryKey: ['creation-idea-boxes', domain], queryFn: ({ signal }) => api.listIdeaBoxes(domain, signal), enabled: !!domain && validDomain, gcTime: 0 });
  const current = useQuery({
    queryKey: ['creation-work', domain, work],
    queryFn: ({
      signal
    }) => {
      if (!work) throw new api.WorkbenchActionError('未选作品');
      return api.readWork(work, signal);
    },
    enabled: !!work && validDomain,
    gcTime: 0
  });
  if (current.data && current.data.domainRef === domain && !openWorks.some(w => w.workId === current.data.workId)) setOpenWorks([...openWorks, current.data]);
  function open(id: string) {
    setParams({
      domain,
      work: id,
      view: section === 'works' ? 'body' : 'topic'
    });
  }
  useEffect(() => {
    const protectCreate = (event: BeforeUnloadEvent) => {
      if (creating || createUnknown || assetPending || anyTemplatePending || anyPublicationPending || anyTitlePending || anyWritingPending || assetDirty || title.trim()) { event.preventDefault(); event.returnValue = ''; }
    };
    window.addEventListener('beforeunload', protectCreate);
    return () => window.removeEventListener('beforeunload', protectCreate);
  }, [creating, createUnknown, assetPending, anyTemplatePending, anyPublicationPending, anyTitlePending, anyWritingPending, assetDirty, title]);
  async function createWork() {
    if (!domain || creating) return;
    setCreating(true);
    setError('');
    const document = newDocument(title.trim());
    const frozen = createPending.current ?? {
      domainRef: domain,
      document,
      requestId: identities.keyFor('create', JSON.stringify({
        domain,
        document
      }))
    };
    createPending.current = frozen;
    try {
      const r = (createUnknown ? await api.lookupWorkRequest(frozen.requestId) : null) ?? (await api.create(frozen));
      identities.settle('create');
      createPending.current = null;
      setCreateUnknown(false);
      setTitle('');
      await queryClient.invalidateQueries({
        queryKey: ['creation-works', domain]
      });
      setParams({
        domain: frozen.domainRef,
        work: r.workId,
        view: 'topic'
      });
    } catch (e) {
      if (e instanceof ApiError && e.answered) {
        identities.settle('create');
        createPending.current = null;
        setCreateUnknown(false);
      } else setCreateUnknown(true);
      setError(api.errorText(e));
    } finally {
      setCreating(false);
    }
  }
  return <OpsFrame section="creation" domain={domain} immersive={immersive && !!work}
    scope={<OpsScopePicker creation options={domainQuery.data ?? []} value={domain} loading={domainQuery.isPending} failed={domainQuery.isError}
      disabled={creating || createUnknown || assetPending} onChange={next => { if (createPending.current || assetPending) return; setError(''); setParams({ domain: next, section }); }} />}
    tabs={<>
      <button type="button" disabled={assetPending} aria-current={section !== 'assets' && (!work || !['materials', 'canvas'].includes(viewValue(params.get('view')))) ? 'page' : undefined} onClick={() => {
        setParams(lastWork ? { domain, work: lastWork, view: 'body' } : { domain, section: 'works' });
      }}>写稿</button>
      <button type="button" disabled={!lastWork || assetPending} aria-current={work && ['materials', 'canvas'].includes(viewValue(params.get('view'))) ? 'page' : undefined} onClick={() => {
        if (lastWork) setParams({ domain, work: lastWork, view: 'materials' });
      }}>材料桌</button>
      <button type="button" disabled={assetPending} aria-current={!work && section === 'assets' ? 'page' : undefined} onClick={() => {
        if (work || section !== 'assets') setParams({ domain, section: 'assets' });
      }}>资产库</button>
    </>}
    meta={<span>个人创作空间</span>}>
<main className={`${styles.workspace} ${immersive && work ? styles.immersiveWorkspace : ''}`} aria-label="运营创作">
    {blocker.state === 'blocked' ? assetDirty && !assetPending && !anyTemplatePending && !anyPublicationPending && !anyTitlePending && !anyWritingPending ? <p role="alert" className={styles.error}>当前内容有未保存编辑。 <button type="button" onClick={() => blocker.reset()}>继续编辑</button> <button type="button" onClick={() => { setAssetDirty(false); setAssetReset(value => value + 1); blocker.proceed(); }}>丢弃编辑并离开</button></p> : <p role="alert" className={styles.error}>操作结果尚未确认，请先留在当前页面并沿用原请求重试。 <button type="button" onClick={() => blocker.reset()}>返回当前页面</button></p> : null}
    {domainQuery.error ? <p className={styles.error} role="alert">{api.errorText(domainQuery.error)}</p> : null}
    {domainQuery.isPending ? <div className={styles.empty} role="status">正在读取创作领域…</div> : !domain || !validDomain ? <div className={styles.entryEmpty}><span className={styles.entryIndex} aria-hidden="true">01 / CREATE</span><h1>{domain ? '当前领域不可用于创作' : '从一个领域开始创作'}</h1><p>{domain ? '请在上方面包屑中切换可用领域，再查看选题和稿件。' : '在上方面包屑选择创作领域，随后可以建立选题，或继续已保存的稿件。'}</p>{domainQuery.isError ? <ActionButton onClick={() => void domainQuery.refetch()}>重新读取领域</ActionButton> : <p>今日选题 · 方案与材料 / 我的创作 · 正文与图页</p>}</div> : work ? <>{current.isPending ? <p>恢复服务器草稿中…</p> : current.error ? <p role="alert" className={styles.error}>{api.errorText(current.error)} <ActionButton variant="outline" onClick={() => void current.refetch()}>重新读取</ActionButton>
</p> : current.data && current.data.domainRef === domain ? null : <p className={styles.error}>作品不属于当前领域。</p>}</> : section === 'assets' ? <AssetsLibrary key={`${domain}:${assetReset}`} domainRef={domain} onPendingChange={setAssetPending} onDirtyChange={setAssetDirty} /> : <section className={styles.library}>
<div className={styles.libraryTools}>
<h1 className={styles.srOnly}>{section === 'today' ? '今日选题' : '我的创作'}</h1>
<div role="group" aria-label="写稿内容" className={styles.steps}>{(['today', 'works'] as const).map(value => <button type="button" key={value} aria-pressed={section === value} onClick={() => setParams({ domain, section: value })}>{value === 'today' ? '今日选题' : '我的稿件'}</button>)}</div>
<small>{works.isError ? '作品读取失败' : works.data ? `${works.data.length} 篇已保存作品` : '读取中'}</small>
</div>
{error ? <p role="alert" className={styles.error}>{error}</p> : null}{works.error ? <p role="alert" className={styles.error}>{api.errorText(works.error)}</p> : null}<TopicLibrary key={domain} section={section} works={works.isError ? [] : works.data ?? []} ideas={ideas.data ?? []} ideasStatus={ideas.status} onRetryIdeas={() => void ideas.refetch()} loading={works.isPending} failed={works.isError} onOpen={open} action={<form className={styles.createForm} onSubmit={e => {
          e.preventDefault();
          void createWork();
        }}>
<input aria-label="新选题标题" placeholder="新选题标题" value={title} disabled={creating || createUnknown} onChange={e => setTitle(e.target.value)} />
<ActionButton type="submit" variant="primary" busy={creating} disabled={!title.trim() || creating}>{creating ? '建立中…' : createUnknown ? '核实原请求并重试' : '建立选题'}</ActionButton>
</form>} /></section>}{openWorks.map(w => <div hidden={w.workId !== work || w.domainRef !== domain} key={w.workId}>
<WorkEditor onOpenLibrary={() => setParams({ domain: w.domainRef, section: 'works' })} onWritingPending={markWritingPending} onTitlePending={markTitlePending} onPublicationPending={markPublicationPending} templateBlocked={blocker.state === 'blocked'} onResetTemplateBlock={() => { if (blocker.state === 'blocked') blocker.reset(); }} onTemplatePending={markTemplatePending} immersive={immersive} onImmersive={setImmersive} initial={w} ideas={ideas.data ?? []} ideaState={ideas.status} onOpenTopic={id => setParams({ domain: w.domainRef, work: id, view: 'topic' })} drafts={works.data ?? []} draftsStatus={works.isError ? 'error' : works.isPending ? 'loading' : 'ready'} onOpenDraft={open} active={w.workId === work && w.domainRef === domain} view={viewValue(params.get('view'))} onView={view => setParams({
          domain: w.domainRef,
          work: w.workId,
          view
        })} />
</div>)}
    </main>
</OpsFrame>;
}
interface Pending {
  requestId: string;
  action: string;
  fingerprint: string;
  run: () => Promise<SaveReceipt>;
  snapshot: WorkDocument | null;
  afterReceipt?: (receipt:SaveReceipt)=>void;
}
function WorkEditor({
  initial,
  onOpenLibrary,
  templateBlocked, onResetTemplateBlock, onTemplatePending, onPublicationPending, onTitlePending, onWritingPending,
  ideas, ideaState, onOpenTopic,
  drafts,
  draftsStatus,
  onOpenDraft,
  active,
  view,
  onView,
  immersive,
  onImmersive
}: {
  initial: WorkEnvelope;
  onOpenLibrary: () => void;
  onTitlePending: (workId: string, pending: boolean) => void;
  onWritingPending: (workId: string, pending: boolean) => void;
  templateBlocked: boolean; onResetTemplateBlock: () => void; onTemplatePending: (workId: string, pending: boolean) => void; onPublicationPending: (workId: string, pending: boolean) => void;
  ideas: IdeaBoxSummary[]; ideaState: 'pending' | 'error' | 'success'; onOpenTopic: (id: string) => void;
  drafts: WorkSummary[];
  draftsStatus: 'loading' | 'error' | 'ready';
  onOpenDraft: (id: string) => void;
  active: boolean;
  view: View;
  onView: (v: View) => void;
  immersive: boolean;
  onImmersive: (value: boolean) => void;
}) {
  const queryClient = useQueryClient();
  const [document, setDocument] = useState(initial.revision.document);
  const [tagsDraft, setTagsDraft] = useState(initial.revision.document.tags.join(' '));
  const tagsLive = useRef(tagsDraft);
  const [revision, setRevision] = useState(initial.revision);
  const [exports, setExports] = useState(initial.exports);
  const [busy, setBusy] = useState(false);
  const [editingLocked, setEditingLocked] = useState(false);
  const [hasPending, setHasPending] = useState(false);
  const [undoState, setUndoState] = useState({
    canUndo: false,
    canRedo: false
  });
  const [aiUnknown, setAiUnknown] = useState(false);
  const writingGeneration = useRef(0);
  const [conflictVisible, setConflictVisible] = useState(true);
  const [feedback, setFeedback] = useState('');
  const [resetRequired, setResetRequired] = useState(initial.experienceResetRequired === true);
  const [draftInputs, setDraftInputs] = useState({ cover: '', note: '', idea: '' });
  useEffect(() => { if (resetRequired) {
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setDraftInputs({ cover: '', note: '', idea: '' });
  } }, [resetRequired]);
  const [recheckStatus, setRecheckStatus] = useState<'checking' | 'ready' | 'failed'>('checking');
  const [recheckAttempt, setRecheckAttempt] = useState(0);
  const [sourceInvalidated, setSourceInvalidated] = useState<WorkEnvelope | null>(null);
  const [sourceResetSequence, setSourceResetSequence] = useState(0);
  const [sourceRefreshSequence, setSourceRefreshSequence] = useState(0);
  const [titleResult, setTitleResult] = useState<TitleStatus | null>(null);
  const [titleAcceptedRevision, setTitleAcceptedRevision] = useState<string | null>(null);
  const [titleCheckedSequence, setTitleCheckedSequence] = useState(-1);
  const [titleStatusRetry, setTitleStatusRetry] = useState(0);
  const [titleBusy, setTitleBusy] = useState(false);
  const [titleMessage, setTitleMessage] = useState('');
  const [titlePrepared, setTitlePrepared] = useState<{ preview: AiPreview; command: Parameters<typeof api.titleExecute>[0] } | null>(null);
  const [titleInputChecked, setTitleInputChecked] = useState(false);
  const [titleRequest, setTitleRequest] = useState<Parameters<typeof api.titleExecute>[0] | null>(null);
  const titleRequestRef = useRef<Parameters<typeof api.titleExecute>[0] | null>(null);
  const titleOperation = useRef({ preparing: false, adopting: false });
  const titleGeneration = useRef(0);
  const [titleAdoptIndex, setTitleAdoptIndex] = useState<{kind:'cover'|'note'; index:number} | null>(null);

  const recheckGeneration = useRef(0);
  const [resetPrompt, setResetPrompt] = useState(false);
  const [saveState, setSaveState] = useState<'saved' | 'dirty' | 'saving' | 'failed' | 'unknown' | 'conflict'>('saved');
  const [conflict, setConflict] = useState<SaveReceipt | null>(null);
  const [history, setHistory] = useState<Revision[] | null>(null);
  const [recoveries, setRecoveries] = useState<WorkDocument[]>([]);
  const [focusSequence, setFocusSequence] = useState(0);
  const [sourceFocus, setSourceFocus] = useState<Citation | null>(null);
  const [placement, setPlacement] = useState<CanvasBlock | null>(null);
  const [bodyTarget, setBodyTarget] = useState('new');
  const [pageTarget, setPageTarget] = useState('');
  const [wantBody, setWantBody] = useState(true);
  const [wantPage, setWantPage] = useState(false);
  const [composing, setComposing] = useState(false);
  const [selection, setSelection] = useState<(WritingSelection & {
    text: string;
  }) | null>(null);
  const [comparisonDrafts,setComparisonDrafts]=useState<string[]|null>(null);
  const [aiDrafts,setAiDrafts]=useState<string[]|null>(null);
  const comparisonDirty=useRef(false);
  const [aiAdoptedIndices,setAiAdoptedIndices]=useState<number[]>([]);
  const [aiCopyFallback,setAiCopyFallback]=useState(false);
  const [aiComparison, setAiComparison] = useState(false);
  const [compareVisible,setCompareVisible]=useState(false);
  const [compareBase, setCompareBase] = useState<WorkDocument | null>(null);
  const [copyContainer, setCopyContainer] = useState<HTMLDivElement | null>(null);
  const [aiOpen, setAiOpen] = useState(false);
  const [aiAction, setAiAction] = useState<WritingAction>('draft');
  const [instruction, setInstruction] = useState('');
  const [aiMaterials, setAiMaterials] = useState<string[]>([]);
  const [readiness, setReadiness] = useState<AiReadiness | null>(null);
  const [readinessFailed, setReadinessFailed] = useState(false);
  const readinessGeneration = useRef(0);
  const [preview, setPreview] = useState<AiPreview | null>(null);
  const [ai, setAi] = useState<AiStatus | null>(null);
  const aiLive = useRef(ai);
  useLayoutEffect(() => { aiLive.current = ai; }, [ai]);
  const [aiBusy, setAiBusy] = useState(false);
  const [confirmed, setConfirmed] = useState(false);
  const [budget, setBudget] = useState('');
  const [identities] = useState(() => new RequestIdentities());
  const [confirmationIdentities] = useState(() => new RequestIdentities());
  const [pendingConfirmation, setPendingConfirmation] = useState<string | null>(null);
  const [publicationPending, setPublicationPending] = useState<PublicationPending | null>(null);
  const [templatePending, setTemplatePending] = useState<TemplateCreateCommand | null>(null);
  const live = useRef(document);
  const server = useRef(revision);
  const pending = useRef<Pending | null>(null);
  const lock = useRef(false);
  const deferredRecheck = useRef(false);
  const composition = useRef(false);
  const undo = useRef<WorkDocument[]>([]);
  const redo = useRef<WorkDocument[]>([]);
  const aiPending = useRef<{
    requestId: string;
    previewRef: string;
    expectedRevision: string;
    confirmedInputHash: string;
    budgetTokens: number;
    confirmed: boolean;
  } | null>(null);
  const accounts = useQuery({
    queryKey: ['creation-accounts', initial.domainRef],
    queryFn: ({
      signal
    }) => api.accounts(initial.domainRef, signal),
    gcTime: 0
  });
  const dirty = JSON.stringify(document) !== JSON.stringify(revision.document) || tagsDraft !== document.tags.join(' ');
  const hasWithdrawnParagraph = document.experienceSlots.some(slot => document.paragraphs.some(p => p.id === slot.paragraphId && p.text === '[本人已撤回这段经历]'));
  function edit(next: WorkDocument, record = true) {
    if (pending.current && pending.current.action !== 'save') return;
    if (record) {
      undo.current.push(live.current);
      redo.current = [];
    }
    setUndoState({
      canUndo: undo.current.length > 0,
      canRedo: redo.current.length > 0
    });
    live.current = next;
    setDocument(next);
    if (pending.current) {
      setSaveState('unknown');
    } else if (conflict) {
      setSaveState('conflict');
    } else {
      const equalsServer = JSON.stringify(next) === JSON.stringify(server.current.document) && tagsLive.current === next.tags.join(' ');
      setSaveState(previous => previous === 'failed' || previous === 'unknown' || previous === 'conflict' ? previous : equalsServer ? 'saved' : 'dirty');
      if (saveState !== 'failed') setFeedback('');
    }
  }
  function changeTags(value: string) {
    tagsLive.current = value;
    setTagsDraft(value);
    setSaveState(previous => previous === 'unknown' || previous === 'conflict' || previous === 'failed' ? previous : 'dirty');
  }
  function applyReceipt(r: SaveReceipt, snapshot: WorkDocument | null) {
    void queryClient.invalidateQueries({ queryKey: ['creation-works', initial.domainRef] });
    void queryClient.invalidateQueries({ queryKey: ['creation-idea-boxes', initial.domainRef] });
    const oldExperience = server.current.document.experienceSlots;
    const newExperience = r.revision.document.experienceSlots;
    if (JSON.stringify(oldExperience) !== JSON.stringify(newExperience) ||
        ((oldExperience.length > 0 || newExperience.length > 0) && server.current.document.accountRef !== r.revision.document.accountRef)) {
      void (async () => {
        await queryClient.cancelQueries({ queryKey: ['creation-assets', 'draft-checks'] });
        await queryClient.invalidateQueries({ queryKey: ['creation-assets', 'draft-checks'] });
      })();
      if (typeof BroadcastChannel !== 'undefined') {
        const channel = new BroadcastChannel('linggan-creation-experience-events');
        channel.postMessage('draft-experience-changed');
        channel.close();
      }
    }
    server.current = r.revision;
    setRevision(r.revision);
    if (snapshot === null || JSON.stringify(live.current) === JSON.stringify(snapshot)) {
      live.current = r.revision.document;
      setDocument(r.revision.document);
      const sameTags = snapshot === null || tagsLive.current === snapshot.tags.join(' ');
      if (sameTags) {
        tagsLive.current = r.revision.document.tags.join(' ');
        setTagsDraft(tagsLive.current);
      }
      setSaveState(sameTags ? 'saved' : 'dirty');
    } else setSaveState('dirty');
    setConflict(null);
  }
  async function run(p: Pending) {
    const retrying = pending.current === p;
    if (pending.current && pending.current !== p) throw new api.WorkbenchActionError('上一项操作结果尚未确认，请先核实原请求。');
    if (composition.current && p.action !== 'save') throw new api.WorkbenchActionError('请先完成中文输入');
    if (lock.current) throw new api.WorkbenchActionError('上一项操作尚未完成');
    lock.current = true;
    setBusy(true);
    setEditingLocked(p.action !== 'save');
    pending.current = p;
    setHasPending(true);
    setSaveState('saving');
    try {
      const r = (retrying ? await api.lookupWorkRequest(p.requestId, initial.workId) : null) ?? (await p.run());
      identities.settle(p.action);
      pending.current = null;
      setHasPending(false);
      const previous = live.current;
      applyReceipt(r, p.snapshot);
      if (p.action === 'adopt' || p.action === 'restore') {
        undo.current.push(previous);
        redo.current = [];
        setUndoState({
          canUndo: true,
          canRedo: false
        });
      }
      setFeedback('服务器已保存');
      p.afterReceipt?.(r);
      return r;
    } catch (e) {
      let c: SaveReceipt | null = null;
      try {
        c = api.conflictReceipt(e);
        if (c && (c.requestId !== p.requestId || c.workId !== initial.workId || c.outcome !== 'stale')) c = null;
      } catch {/* Invalid conflict bodies never replace local input. */}
      if (c) {
        setConflict(c);
        setConflictVisible(true);
        setSaveState('conflict');
        pending.current = null;
        setHasPending(false);
        identities.settle(p.action);
      } else if (e instanceof ApiError && e.answered) {
        if (e.failure.kind === 'http' && e.failure.code === 'creation_experience_reset_required') setResetRequired(true);
        pending.current = null;
        setHasPending(false);
        identities.settle(p.action);
        setSaveState('failed');
      } else setSaveState('unknown');
      setFeedback(api.errorText(e));
      throw e;
    } finally {
      lock.current = false;
      setBusy(false);
      setEditingLocked(!!pending.current && pending.current.action !== 'save');
      if (p.action === 'adopt-title') protectTitle();
      if (deferredRecheck.current) { deferredRecheck.current = false; setRecheckAttempt(value => value + 1); }
    }
  }
  async function saveCurrent(): Promise<Revision> {
    if (sourceInvalidated || resetRequired || recheckStatus !== 'ready') throw new api.WorkbenchActionError('请先核验并处理已撤回的经历');
    if (composition.current) throw new api.WorkbenchActionError('请先完成中文输入');
    if (lock.current) throw new api.WorkbenchActionError('保存仍在进行，请稍后重试');
    if (pending.current) {
      await run(pending.current);
    }
    const tags = [...new Set(tagsLive.current.split(/\s+/u).filter(Boolean))];
    if (JSON.stringify(tags) !== JSON.stringify(live.current.tags)) {
      live.current = {
        ...live.current,
        tags
      };
      setDocument(live.current);
    }
    const snapshot = live.current;
    if (JSON.stringify(snapshot) === JSON.stringify(server.current.document)) {
      tagsLive.current = server.current.document.tags.join(' ');
      setTagsDraft(tagsLive.current);
      setSaveState('saved');
      setFeedback('服务器版本已是当前内容');
      return server.current;
    }
    const body = {
      expectedRevision: server.current.id,
      document: snapshot
    };
    const fingerprint = JSON.stringify(body);
    const requestId = identities.keyFor('save', fingerprint);
    const r = await run({
      requestId,
      action: 'save',
      fingerprint,
      snapshot,
      run: () => api.save(initial.workId, {
        ...body,
        requestId
      })
    });
    return r.revision;
  }
  // The copy saves first. An autosave that is already in flight is waited for instead of failing the click.
  async function saveBeforeCopy(): Promise<Revision> {
    for (let waited = 0; lock.current && waited < 5000; waited += 50) await new Promise<void>(resolve => window.setTimeout(resolve, 50));
    return saveCurrent();
  }
  function clearStaleEditor() {
    titleGeneration.current++;
    writingGeneration.current++;
    identities.settle('preview');
    setTitleResult(null); setTitlePrepared(null); setTitleInputChecked(false); setTitleAcceptedRevision(null); setTitleCheckedSequence(-1); setTitleAdoptIndex(null);
    undo.current = []; redo.current = []; setUndoState({ canUndo: false, canRedo: false });
    comparisonDirty.current = false;
    setRecoveries([]); setHistory(null); setSelection(null); setPlacement(null);
    setCompareBase(null); setComparisonDrafts(null); setAiDrafts(null); setAi(null); setPreview(null); setInstruction(''); setAiMaterials([]);
    setConflict(null); setConflictVisible(false);
  }
  async function resumeAfterWithdrawal() {
    const expectedRevision = server.current.id;
    const fingerprint = JSON.stringify({ expectedRevision });
    const requestId = identities.keyFor('reset-withdrawn', fingerprint);
    try {
      await run({ requestId, action: 'reset-withdrawn', fingerprint, snapshot: null,
        run: () => api.resetWithdrawn(initial.workId, { requestId, expectedRevision }),
        afterReceipt: () => { clearStaleEditor(); setSourceInvalidated(null); setSourceResetSequence(value => value + 1); setResetRequired(false); setResetPrompt(false); },
      });
    } catch { setResetPrompt(false); /* The error and retry state are shown by run. */ }
  }
  const autoSave = useEffectEvent(() => {
    void saveCurrent().catch(() => undefined);
  });
  useEffect(() => {
    if (sourceInvalidated || resetRequired || recheckStatus !== 'ready' || !dirty || composing || saveState === 'unknown' || saveState === 'conflict' || saveState === 'failed') return;
    const timer = window.setTimeout(() => {
      if (!lock.current && !pending.current) autoSave();
    }, 900);
    return () => window.clearTimeout(timer);
  }, [document, tagsDraft, dirty, composing, saveState, resetRequired, recheckStatus, sourceInvalidated]);
  // A withdrawal in another tab invalidates the visible editor without silently discarding edits.
  useLayoutEffect(() => {
    if (!active) return;
    let disposed = false;
    let generation = 0;
    const sourceGeneration = recheckGeneration;
    async function recheck() {
      setRecheckStatus('checking');
      setSourceRefreshSequence(value => value + 1);
      const current = ++generation;
      sourceGeneration.current++;
      if (lock.current) { deferredRecheck.current = true; return; }
      try {
        const latest = await api.readWork(initial.workId);
        if (disposed || current !== generation) return;
        const prior = server.current;
        const hasSources = prior.document.derivedSources.length > 0 || prior.document.excerpts.length > 0;
        const qualifiedPrior = latest.revision.id === prior.id ? latest.revision : hasSources ? await api.readRevision(initial.workId, prior.id) : prior;
        const savedCitations = [...prior.document.derivedSources.map(item => item.citation), ...prior.document.excerpts.map(item => item.citation)];
        const localCitations = [...live.current.derivedSources.map(item => item.citation), ...live.current.excerpts.filter(item => item.selectedText !== null).map(item => item.citation)]
          .filter(citation => !savedCitations.some(saved => JSON.stringify(saved) === JSON.stringify(citation)));
        const localQualified = await Promise.all(localCitations.map(async citation => {
          try { return await api.qualifyCitation(initial.domainRef, citation); }
          catch (cause) { if (cause instanceof ApiError && cause.failure.kind === 'http' && [400, 403, 404, 409].includes(cause.failure.status)) return false; throw cause; }
        }));
        const suggestion = aiLive.current;
        const freshSuggestion = suggestion?.suggestion ? await api.aiStatus(suggestion.requestId) : null;
        if (disposed || current !== generation) return;
        if ((hasSources && JSON.stringify(qualifiedPrior.document) !== JSON.stringify(prior.document)) || localQualified.some(value => !value)) {
          identities.settle('preview');
          setSourceInvalidated(latest);
          setHistory(null); setPlacement(null); setCompareVisible(false); setAiComparison(false);
          setAiOpen(false); setSelection(null); setAi(null); setAiDrafts(null); setPreview(null); setInstruction(''); setAiMaterials([]); setAiCopyFallback(false);
        } else if (freshSuggestion && (freshSuggestion.sourceQualification !== 'readable' || !freshSuggestion.suggestion)) {
          setAi(null); setAiDrafts(null); setPreview(null); setAiComparison(false); setAiOpen(false); setAiCopyFallback(false);
          setFeedback('参考来源当前不可用，生成建议已隐藏。');
        }
        if (latest.experienceResetRequired || latest.revision.document.experienceEpoch !== server.current.document.experienceEpoch) {
          identities.settle('preview');
          setResetRequired(true);
          setHistory(null); setPlacement(null); setCompareVisible(false); setAiComparison(false);
          setAiOpen(false); setSelection(null); setAi(null); setAiDrafts(null); setPreview(null); setInstruction(''); setAiMaterials([]); setAiCopyFallback(false);
        }
        setRecheckStatus('ready');
      } catch { if (!disposed && current === generation) setRecheckStatus('failed'); }
    }
    const channel = typeof BroadcastChannel === 'undefined' ? null : new BroadcastChannel('linggan-creation-experience-events');
    if (channel) channel.onmessage = event => { if (event.data === 'withdrawn') void recheck(); };
    window.addEventListener('focus', recheck);
    void recheck();
    return () => { disposed = true; sourceGeneration.current++; channel?.close(); window.removeEventListener('focus', recheck); };
  }, [active, initial.workId, initial.domainRef, recheckAttempt, identities]);
  useEffect(() => {
    const handler = (e: BeforeUnloadEvent) => {
      if (JSON.stringify(live.current) !== JSON.stringify(server.current.document) || tagsLive.current !== live.current.tags.join(' ') || pending.current || pendingConfirmation || publicationPending || templatePending || comparisonDirty.current || Object.values(draftInputs).some(value => value.trim())) {
        e.preventDefault();
        e.returnValue = '';
      }
    };
    window.addEventListener('beforeunload', handler);
    return () => window.removeEventListener('beforeunload', handler);
  }, [pendingConfirmation, publicationPending, templatePending, draftInputs]);
  function protectTitle() {
    onTitlePending(initial.workId, titleOperation.current.preparing || titleOperation.current.adopting || !!titleRequestRef.current || pending.current?.action === 'adopt-title');
  }
  async function sendTitle(command: Parameters<typeof api.titleExecute>[0]) {
    setTitleBusy(true); setTitleMessage(''); protectTitle();
    try {
      const response = await api.titleExecute(command);
      setTitleResult(response); setTitleCheckedSequence(sourceRefreshSequence);
      if (['succeeded','failed','cancelled'].includes(response.state)) { titleRequestRef.current = null; setTitleRequest(null); protectTitle(); }
    } catch (cause) {
      const rejected = cause instanceof ApiError && cause.failure.kind === 'http' &&
        ['creation_ai_invalid','creation_ai_stale','creation_ai_conflict','creation_ai_admission_denied'].includes(cause.failure.code ?? '');
      if (rejected) { titleRequestRef.current = null; setTitleRequest(null); protectTitle(); }
      setTitleMessage(rejected ? api.errorText(cause) : '请求结果尚未确认，请沿用原请求重试。');
    } finally { setTitleBusy(false); }
  }
  async function generateTitles() {
    if (titleBusy || titleRequestRef.current || sourceInvalidated || resetRequired || recheckStatus !== 'ready') return;
    const batch = ++titleGeneration.current, sourceGeneration = recheckGeneration.current;
    const currentPreparation = () => batch === titleGeneration.current && sourceGeneration === recheckGeneration.current;
    titleOperation.current.preparing=true; protectTitle();
    setTitleBusy(true); setTitleMessage(''); setTitleResult(null); setTitleAcceptedRevision(null); setTitleCheckedSequence(-1);
    try {
      const saved = await saveBeforeCopy();
      if (!currentPreparation()) return;
      const ready = await api.titleReadiness();
      if (!currentPreparation()) return;
      if (!ready.configured || !ready.configRef || ready.deniedReasons?.length) throw new api.WorkbenchActionError('请先在设置中配置标题任务的模型和外发权限。');
      const prepared = await api.titlePreview({previewRef:crypto.randomUUID(),workRef:initial.workId,expectedRevision:saved.id,configRef:ready.configRef});
      if (!currentPreparation() || server.current.id !== saved.id || JSON.stringify(live.current) !== JSON.stringify(server.current.document) || tagsLive.current !== live.current.tags.join(' ')) return;
      const command = {requestId:crypto.randomUUID(),previewRef:prepared.previewRef,expectedRevision:saved.id,confirmedInputHash:prepared.inputHash,budgetTokens:prepared.reservedTokens,confirmed:true};
      setTitlePrepared({ preview: prepared, command }); setTitleInputChecked(false);
    } catch (cause) { setTitleMessage(api.errorText(cause)); }
    finally { titleOperation.current.preparing=false; protectTitle(); setTitleBusy(false); }
  }
  function confirmTitleInput() {
    if (!titlePrepared || !titleInputChecked || titleBusy || titleRequestRef.current || dirty || sourceInvalidated || resetRequired || recheckStatus !== 'ready' || titlePrepared.command.expectedRevision !== revision.id) return;
    const command = titlePrepared.command;
    titleRequestRef.current = command; setTitleRequest(command); setTitlePrepared(null); setTitleInputChecked(false); protectTitle();
    void sendTitle(command);
  }
  const refreshTitles = useEffectEvent(async (id: string, generation: number) => {
    const batch = titleGeneration.current;
    try {
      const next = await api.titleStatus(id);
      if (generation !== recheckGeneration.current || batch !== titleGeneration.current) return;
      setTitleResult(next); setTitleCheckedSequence(sourceRefreshSequence);
      if (['succeeded','failed','cancelled'].includes(next.state) && titleRequestRef.current?.requestId === id) { titleRequestRef.current=null; setTitleRequest(null); protectTitle(); }
      if (next.sourceQualification !== 'readable') setTitleMessage('参考来源当前不可用，标题建议已隐藏。');
    } catch { if (batch === titleGeneration.current && generation === recheckGeneration.current) { setTitleCheckedSequence(-1); setTitleMessage('无法核验标题来源，请重试。'); } }
  });
  useEffect(() => {
    if (!active || !titleResult || sourceInvalidated || resetRequired || recheckStatus !== 'ready') return;
    const generation = recheckGeneration.current;
    if (titleCheckedSequence !== sourceRefreshSequence) { void refreshTitles(titleResult.requestId, generation); return; }
    if (['queued','running','cancel_requested'].includes(titleResult.state)) {
      const timer=window.setTimeout(() => void refreshTitles(titleResult.requestId,generation),1200);
      return () => window.clearTimeout(timer);
    }
  }, [active, titleResult, titleCheckedSequence, titleStatusRetry, sourceRefreshSequence, sourceInvalidated, resetRequired, recheckStatus]);
  async function cancelTitleGeneration() {
    if (!titleResult || titleBusy) return;
    const id=titleResult.requestId, batch=titleGeneration.current;
    setTitleBusy(true);
    try {
      const next=await api.cancelTitle(id);
      if (batch !== titleGeneration.current) return;
      setTitleResult(next); setTitleCheckedSequence(-1);
      if (['succeeded','failed','cancelled'].includes(next.state) && titleRequestRef.current?.requestId === id) {
        titleRequestRef.current=null; setTitleRequest(null); protectTitle();
      }
    } catch { if (batch === titleGeneration.current) setTitleMessage('取消结果尚未确认，正在继续核对。'); }
    finally { setTitleBusy(false); }
  }
  async function chooseTitle(kind:'cover'|'note', index:number) {
    if (!titleResult || titleBusy || titleCheckedSequence !== sourceRefreshSequence || dirty || (titleAcceptedRevision ?? titleResult.inputRevision) !== revision.id) return;
    const generation = recheckGeneration.current;
    titleOperation.current.adopting=true;protectTitle();setTitleBusy(true);setTitleMessage('');
    try {
      const fresh=await api.titleStatus(titleResult.requestId);
      if (generation !== recheckGeneration.current || JSON.stringify(live.current) !== JSON.stringify(server.current.document) || tagsLive.current !== live.current.tags.join(' ')) { setTitleMessage('稿件已变化，请先保存并重新生成标题。'); return; }
      if (fresh.sourceQualification !== 'readable' || !fresh.suggestion || (fresh.staleBase && titleAcceptedRevision !== revision.id)) { setTitleResult(fresh);setTitleMessage('标题的参考来源或稿件版本已变化，请重新生成。');return; }
      const body={expectedRevision:revision.id,kind,index};const fingerprint=JSON.stringify([titleResult.requestId,body]);
      const requestId=identities.keyFor('adopt-title',fingerprint);setTitleAdoptIndex({kind,index});
      await run({requestId,action:'adopt-title',fingerprint,snapshot:null,run:()=>api.adoptTitle(titleResult.requestId,{requestId,...body}),afterReceipt:receipt=>{setTitleAcceptedRevision(receipt.revision.id);setTitleAdoptIndex(null);protectTitle();}});
    } catch (cause) { setTitleMessage(api.errorText(cause)); }
    finally { titleOperation.current.adopting=false; protectTitle(); setTitleBusy(false); }
  }
  function collect(a: ValidatedAnchor, group: CanvasGroup) {
    const existing = live.current.excerpts.find(e => e.citation.sourceRef === a.citation.sourceRef && e.citation.sourceVersion === a.citation.sourceVersion && e.citation.start === a.citation.start && e.citation.end === a.citation.end);
    if (existing && live.current.canvas.some(c => c.excerptId === existing.id)) {
      setFeedback('这段原文已在画布中');
      return;
    }
    const excerpt = {
      id: crypto.randomUUID(),
      ...a
    };
    edit({
      ...live.current,
      excerpts: existing ? live.current.excerpts : [...live.current.excerpts, excerpt],
      canvas: [...live.current.canvas, newCanvasBlock(group, existing?.id ?? excerpt.id)]
    });
  }
  function updateCanvas(id: string, patch: Partial<CanvasBlock>) {
    edit({
      ...live.current,
      canvas: live.current.canvas.map(c => c.id === id ? {
        ...c,
        ...patch,
        expressionRevision: patch.expression !== undefined && patch.expression !== c.expression ? c.expressionRevision + 1 : c.expressionRevision
      } : c)
    });
  }
  function createPlacementPage(kind:'opinion'|'steps') {
    if(busy||live.current.pages.length>=30)return;
    const page=newImagePage(kind);
    edit({...live.current,pages:[...live.current.pages,page]});
    setPageTarget(page.id);setWantPage(true);
  }
  async function place() {
    if (!placement) return;
    try {
      let d = live.current;
      const targets: UseTarget[] = [];
      if (wantBody) {
        const paragraphId = bodyTarget === 'new' ? crypto.randomUUID() : bodyTarget;
        if (bodyTarget === 'new') {
          d = {
            ...d,
            paragraphs: [...d.paragraphs, {
              id: paragraphId,
              text: ''
            }]
          };
        }
        targets.push({
          kind: 'paragraph',
          paragraphId
        });
      }
      if (wantPage && pageTarget) {
        const page = d.pages.find(p => p.id === pageTarget);
        if (page) {
          const existing = d.uses.find(u => u.canvasId === placement.id && u.target.kind === 'pageBlock' && u.target.pageId === page.id);
          const blockId = existing?.target.kind === 'pageBlock' ? existing.target.blockId : page.blocks.find(b => !b.text)?.id ?? crypto.randomUUID();
          if (!page.blocks.some(b => b.id === blockId)) d = {
            ...d,
            pages: d.pages.map(p => p.id === page.id ? {
              ...p,
              blocks: [...p.blocks, {
                id: blockId,
                text: ''
              }]
            } : p)
          };
          targets.push({
            kind: 'pageBlock',
            pageId: page.id,
            blockId
          });
        }
      }
      if (!targets.length) throw new api.WorkbenchActionError('请选择正文或具体图页');
      if (d !== live.current) edit(d);
      const saved = await saveCurrent();
      const body = {
        expectedRevision: saved.id,
        canvasId: placement.id,
        targets
      };
      const fingerprint = JSON.stringify(body);
      const requestId = identities.keyFor('place', fingerprint);
      await run({
        requestId,
        action: 'place',
        fingerprint,
        snapshot: null,
        run: () => api.place(initial.workId, {
          ...body,
          requestId
        })
      });
      setPlacement(null);
    } catch (e) {
      setFeedback(api.errorText(e));
    }
  }
  function retainCurrentCopy() {
    const copy = { ...live.current, tags: [...new Set(tagsLive.current.split(/\s+/u).filter(Boolean))] };
    setRecoveries(old => [...old, copy]);
  }
  async function restoreVersion(id: string) {
    try {
      retainCurrentCopy();
      const body = {
        expectedRevision: server.current.id,
        restoreRevision: id
      };
      const fingerprint = JSON.stringify(body);
      const requestId = identities.keyFor('restore', fingerprint);
      await run({
        requestId,
        action: 'restore',
        fingerprint,
        snapshot: null,
        run: () => api.restore(initial.workId, {
          ...body,
          requestId
        })
      });
      setHistory(null);
    } catch (e) {
      setFeedback(api.errorText(e));
    }
  }
  async function showHistory() {
    try {
      setHistory(await api.history(initial.workId));
    } catch (e) {
      setFeedback(api.errorText(e));
    }
  }
  async function openAi(action: WritingAction) {
    if (aiPending.current) {
      setAiOpen(true);
      setFeedback('上一项生成请求结果尚未确认。请核对保留的原始输入并核实原请求，之后再准备新的动作。');
      return;
    }
    writingGeneration.current++;
    const generation = ++readinessGeneration.current;
    setReadiness(null);
    setReadinessFailed(false);
    setAiMaterials(live.current.excerpts.filter(e => e.selectedText !== null).map(e => e.id));
    setAiAction(action);
    setAiOpen(true);
    setPreview(null);
    setConfirmed(false);
    setFeedback('');
    try {
      const nextReadiness = await api.readiness();
      if (generation === readinessGeneration.current) setReadiness(nextReadiness);
    } catch (e) {
      if (generation === readinessGeneration.current) {
        setReadinessFailed(true);
        setFeedback(api.errorText(e));
      }
    }
  }
  async function previewAi() {
    if (aiPending.current) {
      setFeedback('请先核实上一个生成请求，不能替换其冻结输入。');
      return;
    }
    const batch = writingGeneration.current, sourceGeneration = recheckGeneration.current;
    const currentPreview = () => batch === writingGeneration.current && sourceGeneration === recheckGeneration.current;
    setAiBusy(true);
    try {
      const saved = await saveCurrent();
      if (!currentPreview()) return;
      if (!readiness?.configRef) throw new api.WorkbenchActionError('请先配置可用的创作模型');
      if (aiAction === 'rewrite_selection' && !selection) throw new api.WorkbenchActionError('请先选择要改写的原文');
      if (saved.document.experienceSlots.length && (aiAction !== 'rewrite_selection' || saved.document.experienceSlots.length !== 1 || selection?.paragraphId !== saved.document.experienceSlots[0]?.paragraphId)) throw new api.WorkbenchActionError('当前可改写一段已确认经历；开稿和其他经历组合仍在接入。');
      const citations = saved.document.excerpts.filter(e => e.selectedText !== null && aiMaterials.includes(e.id)).map(e => e.citation);
      let exact: WritingSelection | null = null;
      if (aiAction === 'rewrite_selection' && selection) {
        const p = saved.document.paragraphs.find(p => p.id === selection.paragraphId);
        if (!p || Array.from(p.text).slice(selection.start, selection.end).join('') !== selection.text) throw new api.WorkbenchActionError('所选文字已经改变，请重新选择');
        const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(selection.text));
        exact = {
          paragraphId: selection.paragraphId,
          start: selection.start,
          end: selection.end,
          textHash: Array.from(new Uint8Array(digest), b => b.toString(16).padStart(2, '0')).join('')
        };
      }
      const body = {
        previewRef: identities.keyFor('preview', JSON.stringify({
          saved: saved.id,
          instruction,
          aiAction,
          exact,
          citations
        })),
        workRef: initial.workId,
        expectedRevision: saved.id,
        action: aiAction,
        configRef: readiness.configRef,
        citations,
        paragraphIds: exact ? [exact.paragraphId] : [],
        selection: exact,
        instruction
      };
      const p = await api.preview(body);
      identities.settle('preview');
      if (!currentPreview() || saved.id !== server.current.id || JSON.stringify(live.current) !== JSON.stringify(server.current.document)) return;
      setPreview(p);
      setBudget(String(p.reservedTokens));
      setConfirmed(false);
    } catch (e) {
      if (currentPreview()) setFeedback(api.errorText(e));
    } finally {
      setAiBusy(false);
    }
  }
  async function executeAi() {
    if ((!preview || !confirmed) && !aiPending.current || aiBusy) return;
    const generation = recheckGeneration.current, batch = writingGeneration.current;
    setAiBusy(true);
    try {
      let frozen = aiPending.current;
      if (!frozen) {
        if (!preview || !confirmed || dirty || preview.inputRevision !== server.current.id) throw new api.WorkbenchActionError('底稿已改变，请重新预览');
        const body = { previewRef:preview.previewRef, expectedRevision:preview.inputRevision, confirmedInputHash:preview.inputHash, budgetTokens:Number(budget), confirmed:true };
        frozen = { ...body, requestId:identities.keyFor('execute',JSON.stringify(body)) };
      }
      aiPending.current = frozen;
      onWritingPending(initial.workId, true);
      let status: AiStatus;
      if (aiUnknown) {
        try {
          status = await api.aiStatus(frozen.requestId);
        } catch (e) {
          if (e instanceof ApiError && e.failure.kind === 'http' && e.failure.status === 404) {
            if (!preview || !confirmed || dirty || sourceInvalidated || resetRequired || recheckStatus !== 'ready' || generation !== recheckGeneration.current || batch !== writingGeneration.current || frozen.expectedRevision !== server.current.id) {
              identities.settle('execute'); aiPending.current=null; setAiUnknown(false); onWritingPending(initial.workId,false); setFeedback('原请求未入队，来源或稿件已变化，请重新准备。'); return;
            }
            status = await api.execute(frozen);
          } else throw e;
        }
      } else status = await api.execute(frozen);
      if (generation !== recheckGeneration.current || batch !== writingGeneration.current) { setAiUnknown(true); return; }
      setAi(status);
      if(status.suggestion){setAiOpen(false);setAiComparison(true);setAiAdoptedIndices([]);setAiDrafts(null);setAiCopyFallback(false);if(view !== 'body') onView('body');}
      identities.settle('execute');
      aiPending.current = null;
      setAiUnknown(false);
      onWritingPending(initial.workId, false);
      setPreview(null);
    } catch (e) {
      if (!aiPending.current || e instanceof ApiError && e.failure.kind === 'http' && ['creation_ai_invalid','creation_ai_stale','creation_ai_conflict','creation_ai_admission_denied','creation_ai_source_unavailable'].includes(e.failure.code ?? '')) {
        identities.settle('execute');
        aiPending.current = null;
        setAiUnknown(false);
        onWritingPending(initial.workId, false);
      } else setAiUnknown(true);
      setFeedback(api.errorText(e));
    } finally {
      setAiBusy(false);
    }
  }
  async function refreshAi(cancel = false) {
    const current = aiLive.current;
    if (!current) return;
    const generation = recheckGeneration.current, batch = writingGeneration.current;
    try {
      const next = cancel ? await api.cancel(current.requestId) : await api.aiStatus(current.requestId);
      if (generation === recheckGeneration.current && batch === writingGeneration.current && aiLive.current?.requestId === current.requestId) setAi(next);
    } catch (error) { if (generation === recheckGeneration.current && batch === writingGeneration.current) setFeedback(api.errorText(error)); }
  }
  async function adopt(indices: number[] | null, pages: number[] | null, adoptTitle: boolean, editedTexts?: string[]) {
    if (!ai || dirty || ai.staleBase || ai.inputRevision !== server.current.id) {
      setFeedback('底稿已改变，旧建议不能覆盖当前文字');
      return;
    }
    try {
      const body = {
        expectedRevision: server.current.id,
        paragraphIndices: indices,
        pageIndices: pages,
        adoptTitle
      };
      const fingerprint = JSON.stringify({
        id: ai.requestId,
        ...body
      });
      const requestId = identities.keyFor('adopt', fingerprint);
      retainCurrentCopy();
      await run({
        requestId, action:'adopt', fingerprint, snapshot:null,
        run:()=>api.adopt(ai.requestId,{...body,requestId}),
        afterReceipt:receipt=>{
      let unresolved=false;
      if (editedTexts && ai.suggestion) {
        let next=receipt.revision.document;
        const chosen=indices ?? ai.suggestion.paragraphs.map((_,index)=>index);
        for(const index of chosen){const original=ai.suggestion.paragraphs[index];const changed=editedTexts[index];if(!original || changed===undefined || changed===original.text)continue;
          const candidates=next.paragraphs.filter(paragraph=>original.paragraphId ? paragraph.id===original.paragraphId : !document.paragraphs.some(old=>old.id===paragraph.id) && paragraph.text===original.text);
          if(candidates.length!==1){unresolved=true;setFeedback('建议已保存，手改文字保留在对照区；目标无法唯一定位，请逐段核对。');continue;}
          const target=candidates[0]!;
          const text=ai.action==='rewrite_selection' && selection && original.paragraphId===selection.paragraphId ? Array.from(target.text).slice(0,selection.start).join('')+changed+Array.from(target.text).slice(selection.start+Array.from(original.text).length).join('') : changed;
          next={...next,paragraphs:next.paragraphs.map(paragraph=>paragraph.id===target.id?{...paragraph,text}:paragraph)};
        }
        if(JSON.stringify(next)!==JSON.stringify(receipt.revision.document))edit(next);
      }
      const hasRemaining=indices!==null&&!!ai.suggestion?.paragraphs.some((_,index)=>!indices.includes(index));
      if(hasRemaining){setAiDrafts(editedTexts??aiDrafts??ai.suggestion!.paragraphs.map(paragraph=>paragraph.text));setAiAdoptedIndices(indices??[]);setAiComparison(true);setAiOpen(false);comparisonDirty.current=true;onView('body');}
      else if(!unresolved){setAiComparison(false);comparisonDirty.current=false;setAiDrafts(null);}
      setAi({
        ...ai,
        staleBase: true
      });
        }
      });
    } catch (e) {
      setFeedback(api.errorText(e));
    }
  }
  function unadoptedAiText() {return ai?.suggestion?.paragraphs.flatMap((paragraph,index)=>aiAdoptedIndices.includes(index)?[]:[aiDrafts?.[index]??paragraph.text]).join('\n\n')??'';}
  async function copyUnadoptedAi() {
    const current = aiLive.current;
    if (!current?.suggestion || sourceInvalidated || resetRequired || recheckStatus !== 'ready') return;
    const generation = recheckGeneration.current;
    try {
      const fresh = await api.aiStatus(current.requestId);
      if (generation !== recheckGeneration.current || aiLive.current?.requestId !== current.requestId || fresh.sourceQualification !== 'readable' || !fresh.suggestion) {
        setAi(null); setAiDrafts(null); setAiComparison(false); setAiCopyFallback(false);
        setFeedback('参考来源当前不可用，生成建议已隐藏。'); return;
      }
      try { await navigator.clipboard.writeText(unadoptedAiText()); setFeedback('未采用改稿已复制'); }
      catch { if (generation === recheckGeneration.current) setAiCopyFallback(true); }
    } catch { if (generation === recheckGeneration.current) setFeedback('无法核验参考来源，请重试。'); }
  }
  function refreshComparisonBase() {
    const next=live.current;
    setComparisonDrafts(next.paragraphs.map(paragraph=>{const index=compareBase?.paragraphs.findIndex(old=>old.id===paragraph.id)??-1;return index>=0?comparisonDrafts?.[index]??paragraph.text:paragraph.text;}));
    setCompareBase(next);
  }
  function undoHistory() {
    const previous = undo.current.pop();
    if (previous) {
      redo.current.push(live.current);
      edit(previous, false);
      tagsLive.current = previous.tags.join(' ');
      setTagsDraft(tagsLive.current);
    }
  }
  function redoHistory() {
    const next = redo.current.pop();
    if (next) {
      undo.current.push(live.current);
      edit(next, false);
      tagsLive.current = next.tags.join(' ');
      setTagsDraft(tagsLive.current);
    }
  }
  function textSelection(id: string, target: HTMLTextAreaElement) {
    if (target.selectionStart === target.selectionEnd) {
      setSelection(null);
      return;
    }
    const r = scalarRange(target.value, target.selectionStart, target.selectionEnd);
    setSelection({
      paragraphId: id,
      ...r,
      textHash: ''
    });
  }
  function compose(value: boolean) {
    composition.current = value;
    setComposing(value);
  }
  const linkedIdea = document.ideaRef && document.ideaName === null ? ideas.find(idea => idea.ideaRef === document.ideaRef) : undefined;
  const topicUnavailable = !!document.ideaRef && document.ideaName === null && !linkedIdea;
  function goView(next: View) {
    if (next === 'topic' && linkedIdea) onOpenTopic(linkedIdea.anchorWorkId);
    else if (next !== 'topic' || !topicUnavailable) onView(next);
  }
  const commandContents = <>
<details className={styles.workflowMenu}><summary>创作步骤 · {VIEWS.find(item => item.id === view)?.label}</summary><nav aria-label="创作步骤" className={styles.workflowOptions}>{VIEWS.map(item => <button type="button" key={item.id} disabled={item.id === 'topic' && topicUnavailable} aria-current={view === item.id ? 'step' : undefined} onClick={event => { goView(item.id); event.currentTarget.closest('details')?.removeAttribute('open'); }}>{item.label}</button>)}</nav></details>
{view === 'body' ? <fieldset disabled={editingLocked || !!sourceInvalidated || resetRequired || recheckStatus !== 'ready'} className={styles.toolbarFieldset}><div className={styles.writeControls}><div className={styles.steps}><button aria-pressed={!compareVisible} onClick={() => setCompareVisible(false)}>沉浸编辑</button><button disabled={document.experienceGaps.length > 0} title={document.experienceGaps.length ? '请先填好或删除经历位' : undefined} aria-pressed={compareVisible} onClick={() => {setCompareVisible(true);if(!compareBase){setCompareBase(live.current);setComparisonDrafts(null);}}}>改稿对照</button></div>
<div className={styles.row}>
<ActionButton variant="quiet" disabled={!undoState.canUndo || busy} onClick={undoHistory}>撤销</ActionButton>
<ActionButton variant="quiet" disabled={!undoState.canRedo || busy} onClick={redoHistory}>重做</ActionButton>
<span>{Array.from(document.paragraphs.map(p => p.text).join('\n\n')).length} 字</span>
</div>
<div className={styles.row}>
<ActionButton variant="outline" onClick={() => void openAi('draft')}>按材料起草</ActionButton>
<ActionButton variant="outline" disabled={!selection} onClick={() => void openAi('rewrite_selection')}>改写选段</ActionButton>
<ActionButton variant="outline" onClick={() => void openAi('page_copy')}>图组文案</ActionButton>
</div>
</div></fieldset> : null}
<div className={styles.row}>
<details className={styles.saveDetails}>
<summary aria-label={`保存状态：${saveState === 'saved' ? '已保存' : saveState === 'saving' ? '保存中' : saveState === 'dirty' ? '未保存' : saveState === 'unknown' ? '结果未确认' : saveState === 'conflict' ? '版本冲突' : '保存失败'}`}>{saveState === 'saved' ? '✓' : saveState === 'saving' ? '保存中…' : saveState === 'dirty' ? '未保存' : '!'}<span className={styles.srOnly}>保存状态</span>
</summary>
<p>服务器版本 {revision.number} · {humanMoment(revision.savedAt)}</p>
<p>未确认编辑仅保留在本页内存；关闭页面前请完成保存。</p>
</details>
<ActionButton variant="outline" busy={busy&&saveState==='saving'} disabled={busy || composing || !!conflict} onClick={() => void saveCurrent().catch(() => undefined)}>{hasPending && saveState === 'unknown' ? '核实并重试' : saveState === 'saving' ? '保存中…' : '保存'}</ActionButton>
<ActionButton variant="quiet" onClick={onOpenLibrary}>全部稿件</ActionButton><ActionButton variant="quiet" disabled={resetRequired || recheckStatus !== 'ready'} onClick={() => void showHistory()}>历史</ActionButton><ActionButton variant="quiet" onClick={() => onImmersive(!immersive)}>{immersive ? '退出沉浸' : '收起导航'}</ActionButton>{view === 'materials' || view === 'canvas' ? <ActionButton variant="primary" onClick={() => onView('body')}>进入正文 →</ActionButton> : null}
</div>
  </>;
  return <div className={styles.editorWorkspace} data-view={view}>
    {aiUnknown && !preview ? <div className={styles.warning} role="alert">上次生成请求尚未确认。 <ActionButton variant="outline" disabled={aiBusy} onClick={() => void executeAi()}>核实上次生成</ActionButton></div> : null}
    {view === 'body' ? <details className={styles.compactCommand}>
      <summary>写稿工具 · {saveState === 'saved' ? '已保存' : saveState === 'saving' ? '保存中' : saveState === 'dirty' ? '未保存' : saveState === 'unknown' ? '结果未确认' : saveState === 'conflict' ? '版本冲突' : '保存失败'}</summary>
      <div className={styles.compactCommandPanel}>{commandContents}</div>
    </details> : <div className={styles.commandRow}>{commandContents}</div>}
    {(feedback && ['failed', 'unknown', 'conflict'].includes(saveState)) || conflict ? <div className={styles.error} role="alert">
<p>{feedback}</p>{conflict ? <ActionButton variant="outline" onClick={() => setConflictVisible(true)}>比较版本并解决</ActionButton> : null}{resetRequired ? <ActionButton variant="outline" disabled={busy || recheckStatus !== 'ready'} onClick={() => setResetPrompt(true)}>撤回后重新开始编辑</ActionButton> : null}{hasPending ? <ActionButton variant="outline" disabled={busy} onClick={() => void run(pending.current!).catch(() => undefined)}>沿用原请求重试</ActionButton> : null}</div> : null}<span role="status" className={styles.srOnly}>{feedback}</span>
    {(hasWithdrawnParagraph || resetRequired) && saveState !== 'failed' ? <div className={styles.warning}><p>稿件历史中有已撤回的经历，请先重新开始编辑。</p><ActionButton variant="outline" disabled={busy || recheckStatus !== 'ready'} onClick={() => setResetPrompt(true)}>撤回后重新开始编辑</ActionButton></div> : null}
    {initial.sourceStatuses.some(s => s.state !== 'readable') || document.excerpts.some(e => e.selectedText === null) || document.uses.some(u => u.state === 'needs_review') ? <details className={styles.warning}>
<summary>部分来源需要核验</summary>{initial.sourceStatuses.filter(s => s.state !== 'readable').map(s => <p key={s.excerptId}>{s.state === 'restricted' ? '来源已受限' : s.state === 'changed' ? '来源已变化' : '来源暂不可用'}</p>)}<details><summary>来源诊断详情</summary><pre>{JSON.stringify(initial.sourceStatuses, null, 2)}</pre></details></details> : null}
    {sourceInvalidated ? <div className={styles.warning}><p>参考来源的状态已变化。请丢弃本页未保存的编辑并重新读取，才能继续。</p><ActionButton variant="outline" onClick={() => {
      clearStaleEditor(); const latest = sourceInvalidated; server.current = latest.revision; live.current = latest.revision.document;
      setRevision(latest.revision); setDocument(latest.revision.document); tagsLive.current = latest.revision.document.tags.join(' '); setTagsDraft(tagsLive.current);
      setSaveState('saved'); setSourceResetSequence(value => value + 1); setSourceInvalidated(null); setResetRequired(latest.experienceResetRequired === true);
    }}>丢弃本页编辑并重新读取</ActionButton></div> : null}
    {recheckStatus === 'checking' ? <p role="status">正在核验稿件来源…</p> : recheckStatus === 'failed' ? <p role="alert">暂时无法核验稿件来源。未保存的编辑仍在本页。<ActionButton variant="quiet" onClick={() => setRecheckAttempt(value => value + 1)}>重新核验</ActionButton></p> : null}
    <fieldset hidden={!!sourceInvalidated || resetRequired || recheckStatus !== 'ready'} disabled={editingLocked || !!sourceInvalidated || resetRequired || recheckStatus !== 'ready'} className={styles.editingSurface}>{view === 'topic' ? <TopicDetail drafts={drafts} draftsStatus={draftsStatus} onOpenDraft={onOpenDraft} accountsState={accounts.status} onRetryAccounts={()=>void accounts.refetch()} document={document} accounts={accounts.data ?? []} accountError={accounts.error ? api.errorText(accounts.error) : undefined} onChange={edit} onCompose={compose} onRead={() => onView('materials')} onWrite={() => onView('body')} /> : null}
    <div hidden={view !== 'materials'}>
<MaterialBrowser key={`${initial.domainRef}:${focusSequence}:${sourceResetSequence}`} contextKey={`${initial.workId}:${sourceResetSequence}:${sourceRefreshSequence}`} active={active && view === 'materials'} domain={initial.domainRef} document={document} onChange={next=>edit({...live.current,materials:next.materials,materialPreparation:next.materialPreparation})} onCollect={collect} focusCitation={sourceFocus} onCanvas={() => onView('canvas')} />
</div>
    {view === 'canvas' ? <CanvasBoard onCompose={compose} document={document} busy={busy} onChange={edit} onUpdate={updateCanvas} onLocate={citation=>{setSourceFocus(citation);setFocusSequence(n=>n+1);onView('materials');}} onMaterials={()=>onView('materials')} onUse={target=>{const location=targetLocation(target,document);return {label:location.label,open:()=>onView(location.view)};}} onPlace={card=>{setPlacement(card);setBodyTarget(document.uses.flatMap(use=>use.canvasId===card.id&&use.target.kind==='paragraph'?[use.target.paragraphId]:[])[0]??'new');setWantBody(true);setWantPage(false);setPageTarget(document.pages.find(page=>page.kind!=='cover')?.id??'');}} /> : null}
    {view === 'body' ? <div className={styles.bodyWorkspace}>
{compareVisible&&compareBase&&JSON.stringify(compareBase.paragraphs)!==JSON.stringify(document.paragraphs)?<div className={styles.warning}><ActionButton variant="outline" onClick={refreshComparisonBase}>更新对照底稿，保留改稿</ActionButton></div>:null}
{aiComparison&&ai?.suggestion&&ai.staleBase?<div className={styles.warning}><p>本次采用已改变原底稿，其余改稿仍保留在右侧，不能继续直接采用。</p><ActionButton variant="outline" onClick={()=>void copyUnadoptedAi()}>复制未采用改稿</ActionButton>{aiCopyFallback?<label>未采用改稿，可手动复制<textarea readOnly value={unadoptedAiText()}/></label>:null}</div>:null}
{aiComparison && ai?.suggestion ? <RevisionCompare draftTexts={aiDrafts} onDraftTexts={texts=>{setAiDrafts(texts);comparisonDirty.current=true;}} key={ai.requestId} pairs={ai.suggestion.paragraphs.map((paragraph,index)=>({id:paragraph.paragraphId??String(index),before:ai.action==='rewrite_selection'&&selection?.paragraphId===paragraph.paragraphId?selection.text:document.paragraphs.find(item=>item.id===paragraph.paragraphId)?.text??'',after:paragraph.text,label:`第 ${index+1} 段`}))} stale={dirty || ai.staleBase || ai.inputRevision!==revision.id} disabled={busy || composing} onAdopt={(indices,texts)=>void adopt(indices,[],false,texts)} onClose={()=>setAiComparison(false)} onUndo={undoHistory} canUndo={undoState.canUndo} /> : compareVisible && compareBase && document.experienceGaps.length === 0 ? <RevisionCompare draftTexts={comparisonDrafts} onDraftTexts={texts=>{setComparisonDrafts(texts);comparisonDirty.current=true;}} key={initial.workId+'-manual-comparison'} pairs={compareBase.paragraphs.map((p, index) => ({id:p.id,before:p.text,after:p.text,label:`第 ${index+1} 段`}))} stale={JSON.stringify(compareBase.paragraphs) !== JSON.stringify(document.paragraphs)} disabled={busy || composing} onAdopt={(indices, texts) => { if (JSON.stringify(live.current.paragraphs) !== JSON.stringify(compareBase.paragraphs)) return; const next={...document,paragraphs:document.paragraphs.map((p,index) => indices === null || indices.includes(index) ? {...p,text:texts[index] ?? p.text} : p)};edit(next);if(indices===null){setCompareBase(null);setCompareVisible(false);setComparisonDrafts(null);comparisonDirty.current=false;}else{setCompareBase(next);setComparisonDrafts(texts);comparisonDirty.current=next.paragraphs.some((paragraph,index)=>paragraph.text!==texts[index]);} }} onClose={() => setCompareVisible(false)} onUndo={undoHistory} canUndo={undoState.canUndo} /> : <div className={`${styles.writingLayout} ${styles.withChecks}`}><DraftRail drafts={drafts} ideas={ideas} status={draftsStatus} currentId={initial.workId} onOpen={onOpenDraft} /><ManuscriptEditor titleGenerator={<section aria-label="自动标题候选">
  <div className={styles.row}><ActionButton variant="quiet" disabled={titleBusy || editingLocked || !!sourceInvalidated || resetRequired || recheckStatus !== 'ready' || !!titleRequest} onClick={() => void generateTitles()}>换一批</ActionButton><a href="/settings/models?section=skills&amp;task=creation_title">标题 skill ↗</a></div>
  {templateBlocked && (titleRequest || titleBusy || (titleAdoptIndex && hasPending)) ? <p role="alert">标题请求尚未完成，请先留在本页。 <button type="button" onClick={onResetTemplateBlock}>返回标题</button></p> : null}
  {titleRequest && !titleResult ? <ActionButton variant="outline" disabled={titleBusy} onClick={() => void sendTitle(titleRequest)}>沿用原请求重试</ActionButton> : null}
  {titleResult && ['queued','running','cancel_requested'].includes(titleResult.state) ? <div><p role="status">正在生成标题…</p><ActionButton variant="quiet" disabled={titleBusy || titleResult.state === 'cancel_requested'} onClick={() => void cancelTitleGeneration()}>取消生成</ActionButton></div> : null}
  {titleResult?.state === 'failed' ? <p role="alert">{api.aiFailureText(titleResult.failureCode)}</p> : null}
  {titleMessage ? <p role="alert">{titleMessage}{titleResult ? <button type="button" onClick={() => { setTitleCheckedSequence(-1); setTitleStatusRetry(value => value + 1); }}>重新核验</button> : null}</p> : null}
  {active && recheckStatus === 'ready' && !sourceInvalidated && !resetRequired && titleCheckedSequence === sourceRefreshSequence && titleResult?.sourceQualification === 'readable' && titleResult.suggestion ? <>
    <div className={styles.candidateList} aria-label="自动封面候选">{titleResult.suggestion.coverTitles.map((value,index)=><button key={`${index}:${value}`} className={titleResult.titleWarnings?.cover[index] ? styles.titleRisk : undefined} disabled={titleBusy || busy || dirty || (titleAcceptedRevision ?? titleResult.inputRevision) !== revision.id} onClick={() => void chooseTitle('cover',index)}>{value}{titleResult.titleWarnings?.cover[index] ? ' · 疗效承诺提醒' : ''}</button>)}</div>
    <div className={styles.candidateList} aria-label="自动笔记标题候选">{titleResult.suggestion.noteTitles.map((value,index)=><button key={`${index}:${value}`} className={titleResult.titleWarnings?.note[index] ? styles.titleRisk : undefined} disabled={titleBusy || busy || dirty || (titleAcceptedRevision ?? titleResult.inputRevision) !== revision.id} onClick={() => void chooseTitle('note',index)}>{value}{titleResult.titleWarnings?.note[index] ? ' · 疗效承诺提醒' : ''}</button>)}</div>
    {dirty || (titleAcceptedRevision ?? titleResult.inputRevision) !== revision.id ? <p>稿件已变化，请重新生成。</p> : null}
  </> : null}
  {titleAdoptIndex && hasPending ? <p>标题采用结果尚未确认，请沿用原请求核对。</p> : null}
</section>} draftInputs={draftInputs} onDraftInputs={setDraftInputs} ideas={ideas} ideaState={ideaState} onOpenTopic={onOpenTopic} workId={initial.workId} onCover={() => onView('cover')} onTopic={() => goView('topic')} accountName={accounts.isError ? '账号读取失败' : accounts.data?.find(item => item.ref === document.accountRef)?.name} document={document} edit={edit} compose={compose} isComposing={()=>composition.current} textSelection={textSelection} tagsDraft={tagsDraft} onTagsChange={changeTags} /><CheckPanel workId={initial.workId} domainRef={initial.domainRef} document={document} revision={revision} saveState={saveState} composing={composing} locked={editingLocked} active={active} pendingConfirmation={pendingConfirmation} setPendingConfirmation={setPendingConfirmation} confirmationIdentities={confirmationIdentities} tagsDraft={tagsDraft} onTagsChange={changeTags} edit={edit} saveCurrent={saveBeforeCopy} onGoCover={() => onView('cover')} onOpenSource={source => { setSourceFocus(source); setFocusSequence(n => n + 1); onView('materials'); }} copyContainer={copyContainer} sensitiveVisible={active && recheckStatus === 'ready' && !resetRequired} withdrawalInvalidated={resetRequired} /></div>}
</div> : null}
    <div hidden={!['cover','inner','export'].includes(view)}>
<ImagePages step={view === 'inner' ? 'inner' : view === 'export' ? 'export' : 'cover'} onStepChange={onView} onOpenSource={excerptId => { const excerpt=document.excerpts.find(item=>item.id===excerptId);if(excerpt){setSourceFocus(excerpt.citation);setFocusSequence(n=>n+1);onView('materials');} }} document={document} workId={initial.workId} currentRevisionId={dirty ? undefined : revision.id} sourceRevision={revision} onReadRevision={id => api.readRevision(initial.workId, id)} sourceCheckGeneration={() => recheckGeneration.current} sourceResetSequence={sourceResetSequence} onQualifySources={async citations => { const checked = await Promise.all(citations.map(async citation => { return api.qualifyCitation(initial.domainRef, citation); })); return checked.every(Boolean); }} onChange={edit} onUploadAsset={async file => {
          await saveCurrent();
          const bytes = await file.arrayBuffer();
          const digest = await crypto.subtle.digest('SHA-256', bytes);
          const fingerprint = Array.from(new Uint8Array(digest), b => b.toString(16).padStart(2, '0')).join('');
          const requestId = identities.keyFor('asset', fingerprint);
          try {
            const prior = await api.requestReceipt(requestId);
            const r = prior === null ? await api.uploadAsset(initial.workId, file, requestId) : parseAsset(prior);
            identities.settle('asset');
            return r;
          } catch (e) {
            if (e instanceof ApiError && e.answered) identities.settle('asset');
            throw e;
          }
        }} disabled={busy} onUndo={undoHistory} onRedo={redoHistory} canUndo={undoState.canUndo} canRedo={undoState.canRedo} onSave={saveCurrent} exports={exports} onExportComplete={record => setExports(old => [record, ...old.filter(x => x.id !== record.id)])} exportTransport={{
          freeze: api.freeze,
          upload: api.upload,
          finalize: api.finalize,
          retry: api.retry,
          fail: api.fail,
          lookup: api.lookupExportRequest,
          read: api.readExport
        }} />
</div>
</fieldset>
    {(view === 'body' && !sourceInvalidated && !resetRequired && recheckStatus === 'ready') || templatePending || publicationPending ? <div className={styles.studioFooter}><div ref={setCopyContainer} className={styles.footerCopyMount} hidden={!!sourceInvalidated || resetRequired || recheckStatus !== 'ready' || editingLocked} /><SaveStructuralTemplate workId={initial.workId} domainRef={initial.domainRef} pending={templatePending} onPending={value => { setTemplatePending(value); onTemplatePending(initial.workId, value !== null); }} blocked={templateBlocked && templatePending !== null} onResetBlock={onResetTemplateBlock} saveCurrent={saveBeforeCopy} disabled={editingLocked || !!sourceInvalidated || resetRequired || recheckStatus !== 'ready'} /><PublicationLinker workId={initial.workId} domainRef={initial.domainRef} pending={publicationPending} onPending={value => { setPublicationPending(value); onPublicationPending(initial.workId, value !== null); }} blocked={templateBlocked && publicationPending !== null} onResetBlock={onResetTemplateBlock} saveCurrent={saveBeforeCopy} disabled={editingLocked || !!sourceInvalidated || resetRequired || recheckStatus !== 'ready'} /><ActionButton variant="quiet" disabled={editingLocked || !!sourceInvalidated || resetRequired || recheckStatus !== 'ready'} onClick={() => onView('cover')}>制作封面 →</ActionButton></div> : null}
    {recheckStatus === 'ready' && !sourceInvalidated && conflict && conflictVisible ? <Modal title="服务器已有新版本" onClose={() => { if (!resetRequired && conflict.revision.document.experienceEpoch === document.experienceEpoch) setConflictVisible(false); }}>
{resetRequired ? <>
<p>服务器稿件已经变化。先丢弃本页未保存的编辑并读取服务器版本，再继续处理撤回。</p>
<ActionButton variant="primary" onClick={() => void (async () => {
          clearStaleEditor();
          applyReceipt(conflict, null);
          try {
            const latest = await api.readWork(initial.workId);
            const ready = latest.revision.id === conflict.revision.id && !latest.experienceResetRequired;
            setResetRequired(!ready);
            setFeedback(ready ? '已使用服务器版本，可以继续编辑' : '已读取服务器版本，请继续处理撤回');
          } catch { setFeedback('无法核验服务器版本，请继续处理撤回。'); }
        })()}>丢弃本页编辑，使用服务器版本</ActionButton>
</> : conflict.revision.document.experienceEpoch !== document.experienceEpoch ? <>
<p>经历已撤回，服务器已重新整理稿件。为避免保留撤回前的文字，请丢弃本页未保存的编辑并使用服务器版本。</p>
<ActionButton variant="primary" onClick={() => {
          clearStaleEditor();
          applyReceipt(conflict, null);
          setFeedback('已使用服务器版本，可以继续编辑');
        }}>丢弃本页编辑，使用服务器版本</ActionButton>
</> : <>
<div className={styles.compare}>
<section>
<h3>本页未确认编辑</h3>
<p>{document.title}</p>
<pre>{document.paragraphs.map(p => p.text).join('\n\n')}</pre>
</section>
<section>
<h3>服务器版本 {conflict.revision.number}</h3>
<p>{conflict.revision.document.title}</p>
<pre>{conflict.revision.document.paragraphs.map(p => p.text).join('\n\n')}</pre>
</section>
</div>
<div className={styles.row}>
<ActionButton variant="outline" onClick={() => {
          retainCurrentCopy();
          applyReceipt(conflict, null);
          setFeedback('已读取服务器版本，本页文字保留在恢复列表');
        }}>采用服务器版本，保留本页副本</ActionButton>
<ActionButton variant="primary" onClick={() => {
          server.current = conflict.revision;
          setRevision(conflict.revision);
          setConflict(null);
          setSaveState('dirty');
          setFeedback('已保留本页编辑，请保存为新版本');
        }}>保留本页编辑，另存新版本</ActionButton>
</div>
</>}
</Modal> : null}
    {resetPrompt ? <Modal title="撤回后重新开始编辑" onClose={() => setResetPrompt(false)}>
<p>将以服务器已保存的稿件重新开始，移除已撤回的经历段落。本页尚未保存的编辑和本页恢复副本会被丢弃。</p>
<div className={styles.row}><ActionButton variant="outline" disabled={busy} onClick={() => setResetPrompt(false)}>取消</ActionButton>
<ActionButton variant="primary" busy={busy} disabled={busy || recheckStatus !== 'ready'} onClick={() => void resumeAfterWithdrawal()}>丢弃本页编辑并继续</ActionButton></div>
</Modal> : null}
    {recheckStatus === 'ready' && !resetRequired && history ? <Modal title="历史与恢复" onClose={() => setHistory(null)}>
<div className={styles.history}>{history.map(r => <section key={r.id}>
<div className={styles.row}>
<strong>版本 {r.number} · {r.document.title || '未命名'}</strong>
<span>{humanMoment(r.savedAt)}</span>
<ActionButton variant="quiet" disabled={busy || r.id === revision.id} onClick={() => void restoreVersion(r.id)}>恢复为新版本</ActionButton>
</div>
<details>
<summary>查看正文</summary>
<pre>{r.document.paragraphs.map(p => p.text).join('\n\n')}</pre>
</details>
</section>)}{recoveries.map((d, i) => <section key={i}>
<strong>本页保留副本 {i + 1}</strong>
<pre>{d.paragraphs.map(p => p.text).join('\n\n')}</pre>
<ActionButton variant="quiet" disabled={busy} onClick={() => {
            if (pending.current && pending.current.action !== 'save') return;
            tagsLive.current = d.tags.join(' ');
            setTagsDraft(tagsLive.current);
            edit(d);
            setHistory(null);
          }}>恢复到编辑区</ActionButton>
</section>)}</div>
</Modal> : null}
    {titlePrepared && recheckStatus === 'ready' && !sourceInvalidated && !resetRequired ? <Modal title="AI 会看到的内容" onClose={() => { setTitlePrepared(null); setTitleInputChecked(false); }}>
      <p>本次生成标题会发送以下内容。请核对并在稿件中移除真实姓名、学校、密码和点赞数后再发送。</p>
      <h3>标题任务指令</h3><pre>{titlePrepared.preview.outboundSystem}</pre>
      <h3>本次材料</h3><pre>{JSON.stringify(titlePrepared.preview.outboundInput, null, 2)}</pre>
      <label><input type="checkbox" checked={titleInputChecked} onChange={event => setTitleInputChecked(event.target.checked)} />我已核对，发送内容不含真实姓名、学校、密码和点赞数</label>
      {dirty || titlePrepared.command.expectedRevision !== revision.id ? <p role="alert">稿件已修改，请关闭并重新准备。</p> : null}
      <div className={styles.row}><ActionButton variant="quiet" onClick={() => { setTitlePrepared(null); setTitleInputChecked(false); }}>取消</ActionButton><ActionButton variant="primary" disabled={!titleInputChecked || titleBusy || dirty || titlePrepared.command.expectedRevision !== revision.id} onClick={confirmTitleInput}>确认并生成</ActionButton></div>
    </Modal> : null}
    {recheckStatus === 'ready' && !resetRequired && placement ? <Modal title="放入作品" onClose={() => setPlacement(null)}>
<p className={styles.ownQuote}>{placement.expression || document.excerpts.find(e => e.id === placement.excerptId)?.selectedText}</p>
<div className={styles.placement}>
<label>
<input type="checkbox" checked={wantBody} onChange={e => setWantBody(e.target.checked)} /> 正文</label>
<select aria-label="放入正文位置" value={bodyTarget} onChange={e => setBodyTarget(e.target.value)}>
<option value="new">新增一段</option>{document.paragraphs.map((p, i) => <option key={p.id} value={p.id}>第 {i + 1} 段 · {p.text || '空段落'}</option>)}</select>
<label>
<input type="checkbox" checked={wantPage} onChange={e => setWantPage(e.target.checked)} /> 指定内页</label>
<select aria-label="放入图页位置" value={pageTarget} onChange={e => setPageTarget(e.target.value)}>
<option value="">选择内页</option>{document.pages.filter(p => p.kind !== 'cover').map((p, i) => <option key={p.id} value={p.id}>{i + 1} · {p.title || '未命名内页'}</option>)}</select>
</div><div className={styles.row}><ActionButton disabled={busy||document.pages.length>=30} onClick={()=>createPlacementPage('opinion')}>新建内页</ActionButton><ActionButton variant="quiet" disabled={busy||document.pages.length>=30} onClick={()=>createPlacementPage('steps')}>新建步骤页</ActionButton></div><p>原文摘录与我的表达保持独立；放入后手改文字会保留。</p>
<ActionButton variant="primary" busy={busy} disabled={busy || !wantBody && (!wantPage || !pageTarget)} onClick={() => void place()}>确认放入</ActionButton>
</Modal> : null}
    {aiOpen && !sourceInvalidated && !resetRequired && recheckStatus === 'ready' ? <Modal title={aiAction === 'draft' ? '按所选材料起草' : aiAction === 'rewrite_selection' ? '改写选段' : '图组文案建议'} onClose={() => { writingGeneration.current++; setAiOpen(false); }}>
<div className={styles.aiPanel}>
<p>{!readiness ? readinessFailed ? '模型配置读取失败，请关闭后重新打开。' : '正在读取模型配置' : <>{!readiness.configured ? '尚未配置创作模型' : readiness.transportCallable === true ? '模型可连接' : readiness.transportCallable === false ? '模型连接测试尚未通过' : '连接情况尚未确认'} · {readiness.writingQualified === true ? '创作输出格式已验证' : readiness.writingQualified === false ? '创作输出格式未通过验证' : '创作输出格式尚未验证'}</>}</p>{selection && aiAction === 'rewrite_selection' ? <p className={styles.ownQuote}>{selection.text}</p> : null}<details>
<summary>本次使用的材料（{aiMaterials.length}）</summary>{document.excerpts.map(e => <label key={e.id}>
<input type="checkbox" disabled={e.selectedText === null || !!preview || aiBusy} checked={aiMaterials.includes(e.id)} onChange={event => setAiMaterials(old => event.target.checked ? [...old, e.id] : old.filter(id => id !== e.id))} />
<span>{e.selectedText ?? '来源已受限或不可用'}</span>
</label>)}</details>
<label>这次怎么写<textarea aria-label="改写指令" disabled={aiUnknown || aiBusy} value={instruction} onChange={e => setInstruction(e.target.value)} />
</label>
<ActionButton variant="outline" busy={aiBusy} disabled={aiUnknown || aiBusy || !readiness?.configured || !readiness.configRef} onClick={() => void previewAi()}>准备并核对输入</ActionButton>{preview ? <section>
<p>{preview.sourceCount} 段材料 · {preview.inputCharacterCount} 字符 · 最多 {preview.maxCalls} 次调用</p>
<details open>
<summary>实际发出内容 / 模型 / 方法</summary>
{preview.outboundSystem ? <pre>{preview.outboundSystem}</pre> : null}
<pre>{JSON.stringify(preview.outboundInput, null, 2)}</pre>
<p>{preview.methodVersion} · {preview.modelRef} · {preview.connectionVersionRef}</p>
</details>
<label>本次额度（token）<input type="number" min={preview.reservedTokens} aria-label="本次调用额度" disabled={aiUnknown || aiBusy} value={budget} onChange={e => setBudget(e.target.value)} />
</label><p>发送前请核对材料，去掉真实姓名、学校、密码和点赞数。按配置的输入与输出上限预留，实测用量明确后释放差额；未知用量继续占用预留。</p>
<label>
<input type="checkbox" disabled={aiUnknown || aiBusy} checked={confirmed} onChange={e => setConfirmed(e.target.checked)} />已核对不含真实姓名、学校、密码和点赞数，确认输入及额度</label>
<ActionButton variant="primary" busy={aiBusy} disabled={!confirmed || aiBusy || dirty || Number(budget) < preview.reservedTokens} onClick={() => void executeAi()}>{aiUnknown ? '核实原请求并重试' : '执行一次'}</ActionButton>
</section> : aiUnknown ? <ActionButton variant="outline" busy={aiBusy} onClick={() => void executeAi()}>核实上次生成</ActionButton> : null}{ai ? <section>
<div className={styles.row}>
<strong>{api.aiStateText(ai.state)}</strong>
<ActionButton variant="outline" onClick={() => void refreshAi()}>刷新结果</ActionButton>{['running', 'queued'].includes(ai.state) ? <ActionButton variant="quiet" onClick={() => void refreshAi(true)}>取消这次生成</ActionButton> : null}</div>{ai.failureCode ? <p role="alert" className={styles.error}>{api.aiFailureText(ai.failureCode)}</p> : null}{ai.suggestion ? <>
<p>{dirty || ai.staleBase || ai.inputRevision !== revision.id ? '底稿已有新编辑，建议已过期，不能直接采用' : '对照后选择采用'}</p>{ai.suggestion.title !== null ? <section className={styles.diff}>
<del>{document.title}</del>
<ins>{ai.suggestion.title}</ins>
<ActionButton variant="outline" disabled={busy || dirty || ai.staleBase || ai.inputRevision !== revision.id} onClick={() => void adopt([], [], true)}>采用这个标题</ActionButton>
</section> : null}<ActionButton variant="outline" onClick={()=>{setAiComparison(true);setAiOpen(false);onView('body');}}>打开改稿对照</ActionButton>{ai.suggestion.pages.map((p, i) => <section className={styles.diff} key={`p${i}`}>
<ins>{p.heading}\n{p.body}</ins>
<ActionButton variant="outline" disabled={busy || dirty || ai.staleBase || ai.inputRevision !== revision.id} onClick={() => void adopt([], [i], false)}>采用这页文案</ActionButton>
</section>)}<ActionButton variant="primary" disabled={busy || dirty || ai.staleBase || ai.inputRevision !== revision.id} onClick={() => void adopt(null, null, true)}>采用全部建议</ActionButton>
</> : null}<details>
<summary>调用记录</summary>
<p>{ai.methodVersion} · {ai.invocationRef ?? '尚未派发'}</p><pre>{JSON.stringify({
                state: ai.state,
                failureCode: ai.failureCode
              }, null, 2)}</pre>
{ai.usage.state === 'not_reserved' ? <p>尚未派发，未建立调用账。</p> : <p>输入 {ai.usage.inputTokens ?? '未知'} / 输出 {ai.usage.outputTokens ?? '未知'} / 费用 {ai.usage.costUsd ?? '未知'}</p>}
</details>
</section> : null}{feedback ? <p role="alert" className={styles.error}>{feedback}</p> : null}</div>
</Modal> : null}
    </div>;
}
