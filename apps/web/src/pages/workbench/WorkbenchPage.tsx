import { useEffect, useEffectEvent, useRef, useState, type ReactNode } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useSearchParams } from 'react-router';
import { AppShell } from '../../app/AppShell';
import { ApiError } from '../../shared/api';
import { RequestIdentities } from '../../shared/requestIdentity';
import * as api from './workbenchApi';
import { GROUPS, newDocument, newCanvasBlock, parseAsset, scalarRange, type AiPreview, type AiReadiness, type AiStatus, type Citation, type CanvasBlock, type CanvasGroup, type Revision, type SaveReceipt, type UseTarget, type ValidatedAnchor, type WorkDocument, type WorkEnvelope, type WritingAction, type WritingSelection } from './workbenchModel';
import { MaterialBrowser } from './MaterialBrowser';
import { ImagePages } from './ImagePages';
import styles from './WorkbenchPage.module.css';
type View = 'topic' | 'materials' | 'canvas' | 'body' | 'images';
const VIEWS: readonly {
  id: View;
  label: string;
}[] = [{
  id: 'topic',
  label: '选题'
}, {
  id: 'materials',
  label: '材料'
}, {
  id: 'canvas',
  label: '画布'
}, {
  id: 'body',
  label: '正文'
}, {
  id: 'images',
  label: '图组 / 导出'
}];
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
          view: 'images',
          label: exists ? `图页 ${index + 1}${target.kind === 'pageTitle' ? '标题' : target.kind === 'pageSubtitle' ? '副标题' : ''}` : '图页目标已删除'
        };
      }
  }
}
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
<button aria-label="关闭" onClick={onClose}>×</button>
</div>{children}</dialog>;
}
export function WorkbenchPage() {
  const [params, setParams] = useSearchParams();
  const domain = params.get('domain') ?? '';
  const work = params.get('work');
  const [section, setSection] = useState<'today' | 'works'>(work ? 'works' : 'today');
  const [title, setTitle] = useState('');
  const [error, setError] = useState('');
  const [creating, setCreating] = useState(false);
  const [createUnknown, setCreateUnknown] = useState(false);
  const [identities] = useState(() => new RequestIdentities());
  const createPending = useRef<{
    requestId: string;
    domainRef: string;
    document: WorkDocument;
  } | null>(null);
  const queryClient = useQueryClient();
  const [openWorks, setOpenWorks] = useState<WorkEnvelope[]>([]);
  const domainQuery = useQuery({
    queryKey: ['creation-domains'],
    queryFn: ({
      signal
    }) => api.domains(signal),
    staleTime: 0
  });
  const works = useQuery({
    queryKey: ['creation-works', domain],
    queryFn: ({
      signal
    }) => api.listWorks(domain, signal),
    enabled: !!domain,
    gcTime: 0
  });
  const current = useQuery({
    queryKey: ['creation-work', domain, work],
    queryFn: ({
      signal
    }) => {
      if (!work) throw new api.WorkbenchActionError('未选作品');
      return api.readWork(work, signal);
    },
    enabled: !!work && !!domain,
    gcTime: 0
  });
  if (current.data && current.data.domainRef === domain && !openWorks.some(w => w.workId === current.data.workId)) setOpenWorks([...openWorks, current.data]);
  function open(id: string) {
    setParams({
      domain,
      work: id,
      view: 'topic'
    });
  }
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
  return <AppShell active="ops" activeState="创作" boundaryLabel="本机创作" crumb={<>运营 / 我的创作</>} meta={<span>个人空间</span>}>
<main className={styles.workspace}>
    <nav className={styles.opsNav} aria-label="运营导航">
<a href="/ops/overview">总览</a>
<a href="/ops/publications">发布</a>
<a href="/ops/accounts">账号</a>
<a href="/ops/daily">每日数据</a>
<a href="/ops/io">导入导出</a>
<button aria-pressed={section === 'today'} onClick={() => {
          setSection('today');
          setParams(domain ? {
            domain
          } : {});
        }}>今日选题</button>
<button aria-pressed={section === 'works'} onClick={() => {
          setSection('works');
          setParams(domain ? {
            domain
          } : {});
        }}>我的创作</button>
<select aria-label="创作领域" value={domain} disabled={creating || createUnknown} onChange={e => {
          if (createPending.current) return;
          setError('');
          setParams(e.target.value ? {
            domain: e.target.value
          } : {});
        }}>
<option value="">选择领域</option>{domainQuery.data?.map(d => <option key={d.domainRef} value={d.domainRef}>{d.name}</option>)}</select>
</nav>
    {domainQuery.error ? <p className={styles.error} role="alert">{api.errorText(domainQuery.error)}</p> : null}
    {!domain ? <div className={styles.empty}>选择领域，开始或继续一篇创作。</div> : work ? <>{current.isPending ? <p>恢复服务器草稿中…</p> : current.error ? <p role="alert" className={styles.error}>{api.errorText(current.error)} <button onClick={() => void current.refetch()}>重新读取</button>
</p> : current.data && current.data.domainRef === domain ? null : <p className={styles.error}>作品不属于当前领域。</p>}</> : <section className={styles.library}>
<div className={styles.libraryTools}>
<h1>{section === 'today' ? '今日选题' : '我的创作'}</h1>
<small>{works.data ? `${works.data.length} 篇已保存作品` : '读取中'}</small>
</div>
<form className={styles.createForm} onSubmit={e => {
          e.preventDefault();
          void createWork();
        }}>
<input aria-label="新选题标题" placeholder="先确定一个选题，例如：孩子迟迟不开始写作业" value={title} disabled={createUnknown} onChange={e => setTitle(e.target.value)} />
<button className={styles.primary} disabled={!title.trim() || creating}>{creating ? '建立中…' : createUnknown ? '核实原请求并重试' : '建立选题'}</button>
</form>{error ? <p role="alert" className={styles.error}>{error}</p> : null}{works.error ? <p role="alert" className={styles.error}>{api.errorText(works.error)}</p> : null}<div className={styles.workList}>{works.data?.map(w => <button key={w.workId} onClick={() => open(w.workId)}>
<span>{w.title || '未命名选题'}</span>
<small>继续创作 · {new Date(w.savedAt).toLocaleString('zh-CN')}</small>
<span aria-hidden="true">↗</span>
</button>)}</div>{works.data?.length === 0 ? <p className={styles.empty}>还没有已保存方案。先手动建立一篇，再从真实材料取材。</p> : null}</section>}{openWorks.map(w => <div hidden={w.workId !== work || w.domainRef !== domain} key={w.workId}>
<WorkEditor initial={w} active={w.workId === work && w.domainRef === domain} view={viewValue(params.get('view'))} onView={view => setParams({
          domain: w.domainRef,
          work: w.workId,
          view
        })} />
</div>)}
    </main>
</AppShell>;
}
interface Pending {
  requestId: string;
  action: string;
  fingerprint: string;
  run: () => Promise<SaveReceipt>;
  snapshot: WorkDocument | null;
}
function WorkEditor({
  initial,
  active,
  view,
  onView
}: {
  initial: WorkEnvelope;
  active: boolean;
  view: View;
  onView: (v: View) => void;
}) {
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
  const [conflictVisible, setConflictVisible] = useState(true);
  const [feedback, setFeedback] = useState('');
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
  const [aiOpen, setAiOpen] = useState(false);
  const [aiAction, setAiAction] = useState<WritingAction>('draft');
  const [instruction, setInstruction] = useState('');
  const [aiMaterials, setAiMaterials] = useState<string[]>([]);
  const [readiness, setReadiness] = useState<AiReadiness | null>(null);
  const [preview, setPreview] = useState<AiPreview | null>(null);
  const [ai, setAi] = useState<AiStatus | null>(null);
  const [aiBusy, setAiBusy] = useState(false);
  const [confirmed, setConfirmed] = useState(false);
  const [budget, setBudget] = useState('');
  const [identities] = useState(() => new RequestIdentities());
  const live = useRef(document);
  const server = useRef(revision);
  const pending = useRef<Pending | null>(null);
  const lock = useRef(false);
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
  function applyReceipt(r: SaveReceipt, snapshot: WorkDocument | null) {
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
    }
  }
  async function saveCurrent(): Promise<Revision> {
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
  const autoSave = useEffectEvent(() => {
    void saveCurrent().catch(() => undefined);
  });
  useEffect(() => {
    if (!dirty || composing || saveState === 'unknown' || saveState === 'conflict' || saveState === 'failed') return;
    const timer = window.setTimeout(() => {
      if (!lock.current && !pending.current) autoSave();
    }, 900);
    return () => window.clearTimeout(timer);
  }, [document, tagsDraft, dirty, composing, saveState]);
  useEffect(() => {
    const handler = (e: BeforeUnloadEvent) => {
      if (JSON.stringify(live.current) !== JSON.stringify(server.current.document) || tagsLive.current !== live.current.tags.join(' ') || pending.current) {
        e.preventDefault();
        e.returnValue = '';
      }
    };
    window.addEventListener('beforeunload', handler);
    return () => window.removeEventListener('beforeunload', handler);
  }, []);
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
  async function restoreVersion(id: string) {
    try {
      setRecoveries(r => [...r, live.current]);
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
    setAiMaterials(live.current.excerpts.filter(e => e.selectedText !== null).map(e => e.id));
    setAiAction(action);
    setAiOpen(true);
    setPreview(null);
    setConfirmed(false);
    setFeedback('');
    try {
      setReadiness(await api.readiness());
    } catch (e) {
      setFeedback(api.errorText(e));
    }
  }
  async function previewAi() {
    if (aiPending.current) {
      setFeedback('请先核实上一个生成请求，不能替换其冻结输入。');
      return;
    }
    setAiBusy(true);
    try {
      const saved = await saveCurrent();
      if (!readiness?.configRef) throw new api.WorkbenchActionError('请先配置可用的创作模型');
      if (aiAction === 'rewrite_selection' && !selection) throw new api.WorkbenchActionError('请先选择要改写的原文');
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
      setPreview(p);
      setBudget(String(p.reservedTokens));
      setConfirmed(false);
    } catch (e) {
      setFeedback(api.errorText(e));
    } finally {
      setAiBusy(false);
    }
  }
  async function executeAi() {
    if (!preview || !confirmed || aiBusy) return;
    setAiBusy(true);
    try {
      if (dirty || preview.inputRevision !== server.current.id) throw new api.WorkbenchActionError('底稿已改变，请重新预览');
      const body = {
        previewRef: preview.previewRef,
        expectedRevision: preview.inputRevision,
        confirmedInputHash: preview.inputHash,
        budgetTokens: Number(budget),
        confirmed: true
      };
      const frozen = aiPending.current ?? {
        ...body,
        requestId: identities.keyFor('execute', JSON.stringify(body))
      };
      aiPending.current = frozen;
      let status: AiStatus;
      if (aiUnknown) {
        try {
          status = await api.aiStatus(frozen.requestId);
        } catch (e) {
          if (e instanceof ApiError && e.failure.kind === 'http' && e.failure.status === 404) status = await api.execute(frozen);else throw e;
        }
      } else status = await api.execute(frozen);
      setAi(status);
      identities.settle('execute');
      aiPending.current = null;
      setAiUnknown(false);
      setPreview(null);
    } catch (e) {
      if (e instanceof ApiError && e.answered) {
        identities.settle('execute');
        aiPending.current = null;
        setAiUnknown(false);
      } else setAiUnknown(true);
      setFeedback(api.errorText(e));
    } finally {
      setAiBusy(false);
    }
  }
  async function adopt(indices: number[] | null, pages: number[] | null, adoptTitle: boolean) {
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
      setRecoveries(r => [...r, live.current]);
      await run({
        requestId,
        action: 'adopt',
        fingerprint,
        snapshot: null,
        run: () => api.adopt(ai.requestId, {
          ...body,
          requestId
        })
      });
      setAi({
        ...ai,
        staleBase: true
      });
    } catch (e) {
      setFeedback(api.errorText(e));
    }
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
  return <div className={styles.editorWorkspace}>
    <div className={styles.commandRow}>
<nav aria-label="创作步骤" className={styles.steps}>{VIEWS.map(v => <button key={v.id} aria-current={view === v.id ? 'step' : undefined} onClick={() => onView(v.id)}>{v.label}</button>)}</nav>
<div className={styles.row}>
<details className={styles.saveDetails}>
<summary aria-label={`保存状态：${saveState === 'saved' ? '已保存' : saveState === 'saving' ? '保存中' : saveState === 'dirty' ? '未保存' : saveState === 'unknown' ? '结果未确认' : saveState === 'conflict' ? '版本冲突' : '保存失败'}`}>{saveState === 'saved' ? '✓' : saveState === 'saving' ? '保存中…' : saveState === 'dirty' ? '未保存' : '!'}<span className={styles.srOnly}>保存状态</span>
</summary>
<p>服务器版本 {revision.number} · {new Date(revision.savedAt).toLocaleString('zh-CN')}</p>
<p>未确认编辑仅保留在本页内存；关闭页面前请完成保存。</p>
</details>
<button disabled={busy || composing || !!conflict} onClick={() => void saveCurrent().catch(() => undefined)}>{hasPending ? '核实原请求并重试' : '保存'}</button>
<button onClick={() => void showHistory()}>历史</button>
</div>
</div>
    {feedback && saveState !== 'saved' || conflict ? <div className={styles.error} role="alert">
<p>{feedback}</p>{conflict ? <button onClick={() => setConflictVisible(true)}>比较版本并解决</button> : null}{hasPending ? <button disabled={busy} onClick={() => void run(pending.current!).catch(() => undefined)}>沿用原请求重试</button> : null}</div> : null}<span role="status" className={styles.srOnly}>{feedback}</span>
    {initial.sourceStatuses.some(s => s.state !== 'readable') || document.excerpts.some(e => e.selectedText === null) || document.uses.some(u => u.state === 'needs_review') ? <details className={styles.warning}>
<summary>部分来源需要核验</summary>{initial.sourceStatuses.filter(s => s.state !== 'readable').map(s => <p key={s.excerptId}>{s.state === 'restricted' ? '来源已受限' : s.state === 'changed' ? '来源已变化' : '来源暂不可用'}</p>)}<details><summary>来源诊断详情</summary><pre>{JSON.stringify(initial.sourceStatuses, null, 2)}</pre></details></details> : null}
    <fieldset disabled={editingLocked} className={styles.editingSurface}>{view === 'topic' ? <section className={styles.topicEditor}>
<label>选题<input aria-label="选题标题" value={document.title} onCompositionStart={() => compose(true)} onCompositionEnd={() => compose(false)} onChange={e => edit({
            ...document,
            title: e.target.value
          })} />
</label>
<label>切入角度<textarea aria-label="切入角度" placeholder="这篇具体回应什么困境？" value={document.angle} onCompositionStart={() => compose(true)} onCompositionEnd={() => compose(false)} onChange={e => edit({
            ...document,
            angle: e.target.value
          })} />
</label>
<label>提纲<textarea aria-label="创作提纲" placeholder="每行一个要点" value={document.outline.join('\n')} onCompositionStart={() => compose(true)} onCompositionEnd={() => compose(false)} onChange={e => edit({
            ...document,
            outline: e.target.value.split('\n')
          })} />
</label>
<label>所属账号<select aria-label="所属账号" value={document.accountRef ?? ''} onChange={e => edit({
            ...document,
            accountRef: e.target.value || null
          })}>
<option value="">暂不绑定，导出前选定</option>{accounts.data?.map(a => <option key={a.ref} value={a.ref}>{a.name} · {a.platform}</option>)}</select>
</label>{accounts.error ? <p role="alert" className={styles.error}>{api.errorText(accounts.error)}</p> : null}{accounts.data?.length === 0 ? <p className={styles.warning}>这个领域还没有运营账号。可继续创作，导出前需在账号页登记。</p> : null}<div className={styles.topicBasis}>
<span>依据</span>
<strong>{document.excerpts.length} 段已摘录材料</strong>
<button onClick={() => onView('materials')}>打开材料 →</button>
</div>
<div className={styles.topicBasis}>
<span>待补</span>
<p>{!document.excerpts.length ? '还未选定来源材料' : !document.angle.trim() ? '尚未明确切入角度' : !document.outline.some(s => s.trim()) ? '尚未形成提纲' : '核对材料范围与自己的表达'}</p>
</div>
</section> : null}
    <div hidden={view !== 'materials'}>
<MaterialBrowser key={`${initial.domainRef}:${focusSequence}`} contextKey={initial.workId} active={active && view === 'materials'} domain={initial.domainRef} document={document} onCollect={collect} focusCitation={sourceFocus} />
</div>
    {view === 'canvas' ? <>
<div className={styles.canvasTools}>
<span>{document.canvas.length} 张材料卡</span>
<button onClick={() => edit({
            ...document,
            canvas: [...document.canvas, newCanvasBlock()]
          })}>＋ 写一张自己的想法</button>
<button onClick={() => onView('materials')}>继续取材 →</button>
</div>
<div className={styles.canvasBoard}>{GROUPS.map(g => <section key={g.id} className={styles.canvasLane}>
<h2>{g.label} <small>{document.canvas.filter(c => c.group === g.id).length}</small>
</h2>{document.canvas.filter(c => c.group === g.id).map(c => {
              const ex = document.excerpts.find(e => e.id === c.excerptId);
              return <article className={styles.canvasCard} key={c.id}>{ex ? <>
<small>来源原文</small>
<blockquote>{ex.selectedText ?? '来源已受限或不可用'}</blockquote>
<button onClick={() => {
                    setSourceFocus(ex.citation);
                    setFocusSequence(n => n + 1);
                    onView('materials');
                  }}>回到出处 ↗</button>
</> : <small>自己的想法</small>}<label>我的表达<textarea aria-label={`我的表达 ${c.id}`} value={c.expression} onCompositionStart={() => compose(true)} onCompositionEnd={() => compose(false)} onChange={e => updateCanvas(c.id, {
                    expression: e.target.value
                  })} />
</label>
<label>备注<input value={c.note} onChange={e => updateCanvas(c.id, {
                    note: e.target.value
                  })} />
</label>
<select aria-label="材料归组" value={c.group} onChange={e => updateCanvas(c.id, {
                  group: e.target.value as CanvasGroup
                })}>{GROUPS.map(group => <option key={group.id} value={group.id}>{group.label}</option>)}</select>
<div className={styles.useLinks}>{document.uses.filter(u => u.canvasId === c.id).map(u => <button key={u.id} onClick={() => onView(targetLocation(u.target, document).view)}>{targetLocation(u.target, document).label} · {u.state === 'current' ? '已放入' : u.state === 'modified' ? '已手改' : '需核验'}</button>)}</div>
<button className={styles.placeButton} disabled={busy || !c.expression.trim() && !ex?.selectedText} onClick={() => {
                  setPlacement(c);
                  setBodyTarget(document.uses.find(u => u.canvasId === c.id && u.target.kind === 'paragraph')?.target.kind === 'paragraph' ? document.uses.flatMap(u => u.canvasId === c.id && u.target.kind === 'paragraph' ? [u.target.paragraphId] : [])[0] ?? 'new' : 'new');
                  setWantBody(true);
                  setWantPage(false);
                  setPageTarget(document.pages.find(p => p.kind !== 'cover')?.id ?? '');
                }}>放入正文 / 指定图页 →</button>
<button onClick={() => edit({
                  ...document,
                  canvas: document.canvas.filter(x => x.id !== c.id),
                  uses: document.uses.filter(u => u.canvasId !== c.id)
                })}>移除卡片</button>
</article>;
            })}</section>)}</div>
</> : null}
    {view === 'body' ? <div className={styles.bodyWorkspace}>
<div className={styles.writeControls}>
<div className={styles.row}>
<button disabled={!undoState.canUndo || busy} onClick={undoHistory}>撤销</button>
<button disabled={!undoState.canRedo || busy} onClick={redoHistory}>重做</button>
<span>{Array.from(document.paragraphs.map(p => p.text).join('\n\n')).length} 字</span>
</div>
<div className={styles.row}>
<button onClick={() => void openAi('draft')}>按材料起草</button>
<button disabled={!selection} onClick={() => void openAi('rewrite_selection')}>改写选段</button>
<button onClick={() => void openAi('page_copy')}>图组文案</button>
</div>
</div>
<div className={styles.manuscript}>
<input className={styles.manuscriptTitle} aria-label="正文标题" placeholder="标题" value={document.title} onChange={e => edit({
            ...document,
            title: e.target.value
          })} onCompositionStart={() => compose(true)} onCompositionEnd={() => compose(false)} />{document.paragraphs.map((p, i) => <div key={p.id} className={styles.paragraph}>
<textarea aria-label={`正文第 ${i + 1} 段`} placeholder="写下这一段…" value={p.text} onCompositionStart={() => compose(true)} onCompositionEnd={() => compose(false)} onChange={e => edit({
              ...document,
              paragraphs: document.paragraphs.map(x => x.id === p.id ? {
                ...x,
                text: e.target.value
              } : x)
            })} onSelect={e => textSelection(p.id, e.currentTarget)} onKeyDown={e => {
              if (e.key === 'Enter' && !e.shiftKey && !e.nativeEvent.isComposing && !composition.current) {
                e.preventDefault();
                const t = e.currentTarget;
                const before = p.text.slice(0, t.selectionStart),
                  after = p.text.slice(t.selectionEnd);
                const next = {
                  id: crypto.randomUUID(),
                  text: after
                };
                edit({
                  ...document,
                  paragraphs: document.paragraphs.flatMap(x => x.id === p.id ? [{
                    ...x,
                    text: before
                  }, next] : [x])
                });
                window.setTimeout(() => window.document.querySelector<HTMLTextAreaElement>(`[data-paragraph="${next.id}"]`)?.focus(), 0);
              }
            }} data-paragraph={p.id} />
<details>
<summary>段落与来源</summary>
<button onClick={() => edit({
                ...document,
                paragraphs: document.paragraphs.filter(x => x.id !== p.id),
                uses: document.uses.filter(u => u.target.kind !== 'paragraph' || u.target.paragraphId !== p.id)
              })}>删除这一段</button>{document.uses.filter(u => u.target.kind === 'paragraph' && u.target.paragraphId === p.id).map(u => <p key={u.id}>{u.contentOrigin === 'expression' ? '我的表达' : '原文摘录'} · {u.state === 'current' ? '当前使用' : u.state === 'modified' ? '已手动修改' : '需要核验'}</p>)}</details>
</div>)}<button className={styles.addParagraph} onClick={() => edit({
            ...document,
            paragraphs: [...document.paragraphs, {
              id: crypto.randomUUID(),
              text: ''
            }]
          })}>＋ 继续写一段</button>
<label className={styles.tags}>确认发布标签<input aria-label="确认发布标签" placeholder="以空格分隔" value={tagsDraft} onCompositionStart={() => compose(true)} onCompositionEnd={() => compose(false)} onChange={e => {
              tagsLive.current = e.target.value;
              setTagsDraft(e.target.value);
              setSaveState(previous => previous === 'unknown' || previous === 'conflict' || previous === 'failed' ? previous : 'dirty');
            }} />
</label>
</div>
</div> : null}
    <div hidden={view !== 'images'}>
<ImagePages document={document} workId={initial.workId} onChange={edit} onUploadAsset={async file => {
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
    {conflict && conflictVisible ? <Modal title="服务器已有新版本" onClose={() => setConflictVisible(false)}>
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
<button onClick={() => {
          setRecoveries(old => [...old, live.current]);
          applyReceipt(conflict, null);
          setFeedback('已读取服务器版本，本页文字保留在恢复列表');
        }}>采用服务器版本，保留本页副本</button>
<button className={styles.primary} onClick={() => {
          server.current = conflict.revision;
          setRevision(conflict.revision);
          setConflict(null);
          setSaveState('dirty');
          setFeedback('已保留本页编辑，请保存为新版本');
        }}>保留本页编辑，另存新版本</button>
</div>
</Modal> : null}
    {history ? <Modal title="历史与恢复" onClose={() => setHistory(null)}>
<div className={styles.history}>{history.map(r => <section key={r.id}>
<div className={styles.row}>
<strong>版本 {r.number} · {r.document.title || '未命名'}</strong>
<span>{new Date(r.savedAt).toLocaleString('zh-CN')}</span>
<button disabled={busy || r.id === revision.id} onClick={() => void restoreVersion(r.id)}>恢复为新版本</button>
</div>
<details>
<summary>查看正文</summary>
<pre>{r.document.paragraphs.map(p => p.text).join('\n\n')}</pre>
</details>
</section>)}{recoveries.map((d, i) => <section key={i}>
<strong>本页保留副本 {i + 1}</strong>
<pre>{d.paragraphs.map(p => p.text).join('\n\n')}</pre>
<button onClick={() => {
            edit(d);
            setHistory(null);
          }}>恢复到编辑区</button>
</section>)}</div>
</Modal> : null}
    {placement ? <Modal title="放入作品" onClose={() => setPlacement(null)}>
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
</div>{!document.pages.some(p => p.kind !== 'cover') ? <p>还没有内页，可先到图组添加。</p> : null}<p>原文摘录与我的表达保持独立；放入后手改文字会保留。</p>
<button className={styles.primary} disabled={busy || !wantBody && (!wantPage || !pageTarget)} onClick={() => void place()}>确认放入</button>
</Modal> : null}
    {aiOpen ? <Modal title={aiAction === 'draft' ? '按所选材料起草' : aiAction === 'rewrite_selection' ? '改写选段' : '图组文案建议'} onClose={() => setAiOpen(false)}>
<div className={styles.aiPanel}>
<p>{!readiness ? '正在读取模型配置' : !readiness.configured ? '尚未配置创作模型' : readiness.transportCallable === true ? '模型可连接' : '连接情况尚未确认'} · {readiness?.writingQualified === true ? '创作能力已验证' : '创作能力尚未验证'}</p>{selection && aiAction === 'rewrite_selection' ? <p className={styles.ownQuote}>{selection.text}</p> : null}<details>
<summary>本次使用的材料（{aiMaterials.length}）</summary>{document.excerpts.map(e => <label key={e.id}>
<input type="checkbox" disabled={e.selectedText === null || !!preview || aiBusy} checked={aiMaterials.includes(e.id)} onChange={event => setAiMaterials(old => event.target.checked ? [...old, e.id] : old.filter(id => id !== e.id))} />
<span>{e.selectedText ?? '来源已受限或不可用'}</span>
</label>)}</details>
<label>这次怎么写<textarea aria-label="改写指令" disabled={aiUnknown || aiBusy} value={instruction} onChange={e => setInstruction(e.target.value)} />
</label>
<button disabled={aiUnknown || aiBusy || !readiness?.configured || !readiness.configRef} onClick={() => void previewAi()}>准备并核对输入</button>{preview ? <section>
<p>{preview.sourceCount} 段材料 · {preview.inputCharacterCount} 字符 · 最多 {preview.maxCalls} 次调用</p>
<details>
<summary>实际发出内容 / 模型 / 方法</summary>
<pre>{JSON.stringify(preview.outboundInput, null, 2)}</pre>
<p>{preview.methodVersion} · {preview.modelRef} · {preview.connectionVersionRef}</p>
</details>
<label>本次额度（token）<input type="number" min={preview.reservedTokens} aria-label="本次调用额度" disabled={aiUnknown || aiBusy} value={budget} onChange={e => setBudget(e.target.value)} />
</label>
<label>
<input type="checkbox" disabled={aiUnknown || aiBusy} checked={confirmed} onChange={e => setConfirmed(e.target.checked)} />确认这次输入和额度，执行一次</label>
<button className={styles.primary} disabled={!confirmed || aiBusy || dirty || Number(budget) < preview.reservedTokens} onClick={() => void executeAi()}>{aiUnknown ? '核实原请求并重试' : '执行一次'}</button>
</section> : null}{ai ? <section>
<div className={styles.row}>
<strong>{api.aiStateText(ai.state)}</strong>
<button onClick={() => void api.aiStatus(ai.requestId).then(setAi).catch(e => setFeedback(api.errorText(e)))}>刷新结果</button>{['running', 'queued'].includes(ai.state) ? <button onClick={() => void api.cancel(ai.requestId).then(setAi).catch(e => setFeedback(api.errorText(e)))}>取消这次生成</button> : null}</div>{ai.failureCode ? <p role="alert" className={styles.error}>{api.aiFailureText(ai.failureCode)}</p> : null}{ai.suggestion ? <>
<p>{dirty || ai.staleBase || ai.inputRevision !== revision.id ? '底稿已有新编辑，建议已过期，不能直接采用' : '对照后选择采用'}</p>{ai.suggestion.title !== null ? <section className={styles.diff}>
<del>{document.title}</del>
<ins>{ai.suggestion.title}</ins>
<button disabled={busy || dirty || ai.staleBase || ai.inputRevision !== revision.id} onClick={() => void adopt([], [], true)}>采用这个标题</button>
</section> : null}{ai.suggestion.paragraphs.map((s, i) => <section className={styles.diff} key={i}>
<del>{ai.action === 'rewrite_selection' && selection?.paragraphId === s.paragraphId ? selection.text : document.paragraphs.find(p => p.id === s.paragraphId)?.text ?? ''}</del>
<ins>{s.text}</ins>
<button disabled={busy || dirty || ai.staleBase || ai.inputRevision !== revision.id} onClick={() => void adopt([i], [], false)}>采用这一段</button>
</section>)}{ai.suggestion.pages.map((p, i) => <section className={styles.diff} key={`p${i}`}>
<ins>{p.heading}\n{p.body}</ins>
<button disabled={busy || dirty || ai.staleBase || ai.inputRevision !== revision.id} onClick={() => void adopt([], [i], false)}>采用这页文案</button>
</section>)}<button className={styles.primary} disabled={busy || dirty || ai.staleBase || ai.inputRevision !== revision.id} onClick={() => void adopt(null, null, true)}>采用全部建议</button>
</> : null}<details>
<summary>调用记录</summary>
<p>{ai.methodVersion} · {ai.invocationRef}</p><pre>{JSON.stringify({
                state: ai.state,
                failureCode: ai.failureCode
              }, null, 2)}</pre>
<p>输入 {ai.usage.inputTokens ?? '未知'} / 输出 {ai.usage.outputTokens ?? '未知'} / 费用 {ai.usage.costUsd ?? '未知'}</p>
</details>
</section> : null}{feedback ? <p role="alert" className={styles.error}>{feedback}</p> : null}</div>
</Modal> : null}
    </div>;
}
