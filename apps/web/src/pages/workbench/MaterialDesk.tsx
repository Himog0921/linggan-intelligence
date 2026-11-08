import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { useQueries, useQuery } from '@tanstack/react-query';
import * as api from './workbenchApi';
import type { CanvasBlock, CanvasGroup, Citation, SourceItem, WorkDocument, WorkSummary } from './workbenchModel';
import styles from './WorkbenchPage.module.css';

const CARD_W = 208;
const CARD_COL = 232;
const CARD_ROW = 226;
const CARD_COLS = 3;
const FRAME_INSET = 24;
const FRAME_HEAD = 96;
const FRAME_W = FRAME_INSET * 2 + (CARD_COLS - 1) * CARD_COL + CARD_W;
const FRAME_GAP = 48;
const MIN_ZOOM = 0.25;
const MAX_ZOOM = 2;
const SHELF_LIMIT = 40;

const kindLabel: Record<string, string> = {
  note: '原笔记', comment: '评论', ocr: '图片文字', transcript: '口播', frame_text: '画面文字', ops_publication: '发布记录',
};
const kindDot: Record<string, string> = {
  note: 'warn', comment: 'info', ocr: 'ok', transcript: 'ok', frame_text: 'ok', ops_publication: 'info',
};
const groupLabel: Record<string, string> = { inbox: '待分', opening: '开头', argument: '论证', example: '例子', ending: '结尾' };
const rolePicks: { id: CanvasGroup; label: string }[] = [
  { id: 'inbox', label: '待分' },
  { id: 'opening', label: '开头' },
  { id: 'argument', label: '论证' },
  { id: 'example', label: '例子' },
  { id: 'ending', label: '结尾' },
];
const shelfFilters = [
  { id: '', label: '全部' },
  { id: 'note', label: '原笔记' },
  { id: 'comment', label: '评论' },
  { id: 'transcript', label: '口播' },
  { id: 'ocr', label: '图片文字' },
] as const;

interface Card {
  id: string;
  note: boolean;
  kind: string;
  dot: string;
  text: string;
  expression: string;
  group: CanvasGroup;
  role: string;
  memo: string;
  excerptId: string | null;
  meta: string;
}
interface Frame {
  work: WorkSummary;
  x: number;
  y: number;
  w: number;
  h: number;
  cards: Card[];
  prepared: boolean;
  accountName: string | null;
  title: string;
}
interface Spot { left: number; right: number; top: number }
interface Props {
  domain: string;
  works: WorkSummary[];
  currentWorkId: string;
  currentDocument: WorkDocument;
  accounts: { ref: string; name: string }[];
  sourceFocus: Citation | null;
  onClearSourceFocus: () => void;
  onOpenWork: (workId: string) => void;
  onSwitchWork: (workId: string) => void;
  onCreateFrame: () => Promise<void>;
  onAddNote: () => void;
  onPlaceMaterial: (source: SourceItem, group: string) => Promise<void>;
  onUpdateCard: (id: string, patch: Partial<CanvasBlock>) => void;
  onRemoveCard: (id: string) => void;
  onRenameTitle: (title: string) => void;
  onPickAccount: (ref: string | null) => void;
  onCreateFramePending?: boolean;
}

function cardsOf(document: WorkDocument | undefined): Card[] {
  if (!document) return [];
  return document.canvas.map((block: CanvasBlock) => {
    const excerpt = block.excerptId ? document.excerpts.find(item => item.id === block.excerptId) : undefined;
    const role = groupLabel[block.group] ?? '待分';
    if (excerpt) return {
      id: block.id,
      note: false,
      kind: kindLabel[excerpt.citation.sourceKind] ?? '材料',
      dot: kindDot[excerpt.citation.sourceKind] ?? 'info',
      text: excerpt.selectedText ?? '来源已受限或不可用',
      expression: '',
      group: block.group,
      role,
      memo: block.note.trim(),
      excerptId: excerpt.id,
      meta: `${kindLabel[excerpt.citation.sourceKind] ?? '来源'} · 采集于 ${new Date(excerpt.citation.asOf).toLocaleDateString('zh-CN')}`,
    };
    return {
      id: block.id,
      note: true,
      kind: '便签',
      dot: 'warn',
      text: block.expression.trim() || block.note.trim() || '（空便签）',
      expression: block.expression,
      group: block.group,
      role,
      memo: block.note.trim(),
      excerptId: null,
      meta: '便签',
    };
  });
}

function layout(works: WorkSummary[], documents: (WorkDocument | undefined)[], accountOf: (ref: string | null | undefined) => string | null): Frame[] {
  const bottoms: [number, number] = [0, 0];
  return works.map((work, index) => {
    const cards = cardsOf(documents[index]);
    const rows = Math.max(1, Math.ceil(cards.length / CARD_COLS));
    const height = FRAME_HEAD + rows * CARD_ROW + FRAME_INSET;
    const column = bottoms[0] <= bottoms[1] ? 0 : 1;
    const frame = {
      work, x: column * (FRAME_W + FRAME_GAP), y: bottoms[column], w: FRAME_W, h: height, cards,
      prepared: (documents[index]?.materials ?? []).some(item => item.addedBy === 'prepared'),
      accountName: accountOf(documents[index]?.accountRef),
      title: documents[index]?.title ?? work.title,
    };
    bottoms[column] += height + FRAME_GAP;
    return frame;
  });
}

const shortDate = (value: string) => new Date(value).toLocaleDateString('zh-CN', { month: '2-digit', day: '2-digit' });

function spotOf(element: Element | null): Spot | null {
  const rect = element?.getBoundingClientRect();
  return rect ? { left: rect.left, right: rect.right, top: rect.top } : null;
}
function inspectorStyle(spot: Spot | null, height: number, width = 320) {
  if (!spot) return undefined;
  let left = spot.right + 12;
  if (left + width > window.innerWidth - 16) left = spot.left - width - 12;
  if (left < 16) left = Math.max(16, Math.min(spot.left, window.innerWidth - width - 16));
  return { left, top: Math.max(16, Math.min(spot.top, window.innerHeight - Math.max(height, 240) - 16)) };
}

export function MaterialDesk({
  domain, works, currentWorkId, currentDocument, accounts, sourceFocus, onClearSourceFocus,
  onOpenWork, onSwitchWork, onCreateFrame, onAddNote, onPlaceMaterial, onUpdateCard, onRemoveCard, onRenameTitle, onPickAccount, onCreateFramePending,
}: Props) {
  const documents = useQueries({
    queries: works.map(work => ({
      queryKey: ['creation-desk-document', work.workId, work.revisionId],
      queryFn: ({ signal }: { signal: AbortSignal }) => api.readWork(work.workId, signal),
      retry: false,
      gcTime: 0,
    })),
  });
  const saved = useQuery({
    queryKey: ['creation-desk-positions', domain],
    queryFn: ({ signal }) => api.deskPositions(domain, signal),
    retry: false,
    gcTime: 0,
  });
  const [shelfOpen, setShelfOpen] = useState(false);
  const [query, setQuery] = useState('');
  const [filter, setFilter] = useState<string>('');
  const [placing, setPlacing] = useState<string | null>(null);
  const [placeError, setPlaceError] = useState('');
  const shelf = useQuery({
    queryKey: ['creation-desk-shelf', domain, query, filter],
    queryFn: ({ signal }) => api.sources({ domainRef: domain, query, ...(filter ? { kind: filter } : {}) }, signal),
    enabled: shelfOpen,
    retry: false,
    gcTime: 0,
  });
  const ready = documents.every(item => item.isSuccess);
  const [moved, setMoved] = useState<Record<string, { x: number; y: number }>>({});
  const [saveFailed, setSaveFailed] = useState(false);
  const [editor, setEditor] = useState<{ kind: 'memo' | 'role' | 'source'; blockId: string; anchor: Spot | null } | null>(null);
  const [settingsFor, setSettingsFor] = useState<string | null>(null);
  const [settingsAnchor, setSettingsAnchor] = useState<Spot | null>(null);
  const inspector = useRef<HTMLElement | null>(null);
  const [inspectorHeight, setInspectorHeight] = useState(0);
  useLayoutEffect(() => {
    setInspectorHeight(inspector.current?.offsetHeight ?? 0);
  }, [editor, settingsFor]);
  const accountOf = useCallback((ref: string | null | undefined) => ref ? accounts.find(item => item.ref === ref)?.name ?? '账号未知' : null, [accounts]);
  const docOf = (work: WorkSummary, index: number) => work.workId === currentWorkId ? currentDocument : documents[index]?.data?.revision.document;
  const frames = layout(works, works.map(docOf), accountOf).map(frame => {
    const spot = moved[frame.work.workId] ?? saved.data?.find(item => item.workId === frame.work.workId);
    return spot ? { ...frame, x: spot.x, y: spot.y } : frame;
  });
  const [view, setView] = useState({ x: 64, y: 64, zoom: 1 });
  const [focus, setFocus] = useState<string | null>(null);
  const autoFitted = useRef(false);
  const viewport = useRef<HTMLDivElement | null>(null);
  const drag = useRef<{ x: number; y: number; panX: number; panY: number } | null>(null);
  const frameDrag = useRef<{ id: string; x: number; y: number; x0: number; y0: number; spot?: { x: number; y: number } } | null>(null);
  const current = frames.find(frame => frame.work.workId === currentWorkId);

  function fit() {
    const box = viewport.current;
    if (!box || !frames.length) return;
    const x0 = Math.min(...frames.map(frame => frame.x));
    const y0 = Math.min(...frames.map(frame => frame.y));
    const x1 = Math.max(...frames.map(frame => frame.x + frame.w));
    const y1 = Math.max(...frames.map(frame => frame.y + frame.h));
    const pad = 40;
    const zoom = Math.min(1, Math.max(MIN_ZOOM, Math.min(
      (box.clientWidth - pad * 2) / (x1 - x0),
      (box.clientHeight - pad * 2 - 40) / (y1 - y0),
    )));
    setView({ zoom, x: pad - x0 * zoom, y: pad + 40 - y0 * zoom });
  }

  useEffect(() => {
    if (autoFitted.current || !ready || !frames.length || !viewport.current) return;
    autoFitted.current = true;
    const box = viewport.current;
    const frame = frames.find(item => item.work.workId === currentWorkId) ?? frames[0];
    if (!frame) return;
    setView({ zoom: 1, x: Math.max(24, (box.clientWidth - frame.w) / 2) - frame.x, y: 72 - frame.y });
  }, [ready, frames, currentWorkId]);

  const focusFrame = (frame: Frame) => {
    const box = viewport.current;
    if (!box) return;
    const top = 72;
    const availW = Math.max(200, box.clientWidth - 64);
    const availH = Math.max(200, box.clientHeight - top - 96);
    const zoom = Math.min(1, Math.max(0.3, Math.min(availW / frame.w, availH / frame.h)));
    setFocus(frame.work.workId);
    setView({
      zoom,
      x: (box.clientWidth - frame.w * zoom) / 2 - frame.x * zoom,
      y: top + Math.max(0, (availH - frame.h * zoom) / 2) - frame.y * zoom,
    });
  };

  const zoomBy = (step: number) => {
    setView(currentView => ({
      ...currentView,
      zoom: Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, Number((currentView.zoom * step).toFixed(3)))),
    }));
  };

  const beginPan = (event: React.PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) return;
    if ((event.target as HTMLElement).closest('.frame, .desk-tools, .focus-bar, .d-search, .desk-editor')) return;
    drag.current = { x: event.clientX, y: event.clientY, panX: view.x, panY: view.y };
    event.currentTarget.setPointerCapture?.(event.pointerId);
  };
  const movePan = (event: React.PointerEvent<HTMLDivElement>) => {
    const start = drag.current;
    if (!start) return;
    setView(currentView => ({ ...currentView, x: start.panX + event.clientX - start.x, y: start.panY + event.clientY - start.y }));
  };
  const endPan = (event: React.PointerEvent<HTMLDivElement>) => {
    drag.current = null;
    if (event.currentTarget.hasPointerCapture?.(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId);
  };

  const beginFrameDrag = (event: React.PointerEvent<HTMLElement>, frame: Frame) => {
    if (event.button !== 0 || (event.target as HTMLElement).closest('button')) return;
    frameDrag.current = { id: frame.work.workId, x: event.clientX, y: event.clientY, x0: frame.x, y0: frame.y };
    event.currentTarget.setPointerCapture?.(event.pointerId);
  };
  const moveFrame = (event: React.PointerEvent<HTMLElement>) => {
    const start = frameDrag.current;
    if (!start) return;
    const spot = {
      x: Math.round(start.x0 + (event.clientX - start.x) / view.zoom),
      y: Math.round(start.y0 + (event.clientY - start.y) / view.zoom),
    };
    start.spot = spot;
    setMoved(previous => ({ ...previous, [start.id]: spot }));
  };
  const endFrame = async (event: React.PointerEvent<HTMLElement>) => {
    const start = frameDrag.current;
    if (!start) return;
    frameDrag.current = null;
    if (event.currentTarget.hasPointerCapture?.(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId);
    if (!start.spot) return;
    try {
      await api.saveDeskPosition({ domainRef: domain, workId: start.id, x: start.spot.x, y: start.spot.y });
      setSaveFailed(false);
    } catch {
      setSaveFailed(true);
    }
  };

  const place = async (item: SourceItem) => {
    setPlacing(item.fragmentId);
    setPlaceError('');
    try {
      await onPlaceMaterial(item, 'inbox');
    } catch (cause) {
      setPlaceError(api.errorText(cause));
    } finally {
      setPlacing(null);
    }
  };

  const openCard = (card: Card, kind: 'memo' | 'role' | 'source', event: React.MouseEvent<HTMLElement> | React.KeyboardEvent<HTMLElement>) => {
    setSettingsFor(null);
    const spot = spotOf((event.currentTarget as HTMLElement).closest('.mcard'));
    setEditor(previous => previous?.kind === kind && previous.blockId === card.id ? null : { kind, blockId: card.id, anchor: spot });
  };
  const openSettings = (frame: Frame, event: React.MouseEvent<HTMLElement>) => {
    setEditor(null);
    setSettingsAnchor(spotOf((event.currentTarget as HTMLElement).closest('.frame')));
    setSettingsFor(frame.work.workId);
  };

  if (!works.length) return <div className="desk immersive-desk no-search">
    <div className="viewport"><div className="viewport-grid" />
      <p className={styles.empty}>这个领域还没有稿件。先在写稿页建立一个选题，材料会铺在这里。</p>
    </div>
  </div>;

  const focused = frames.find(frame => frame.work.workId === focus) ?? null;
  const settingsFrame = frames.find(frame => frame.work.workId === settingsFor) ?? null;
  const editorCard = editor ? frames.flatMap(frame => frame.cards).find(card => card.id === editor.blockId) ?? null : null;
  const sources = shelf.data?.items ?? [];
  const placed = new Set((currentDocument.canvas.map(block => document_excerpt_key(currentDocument, block))));

  return <div className={`desk immersive-desk${focus ? ' is-focused' : ''}${shelfOpen ? '' : ' no-search'}`}>
    {shelfOpen ? <aside className="d-search" id="deskSearch" aria-label="找材料">
      <div className="material-shelf-head"><b>找材料</b>
        <button type="button" className="modal-x" aria-label="收起找材料" onClick={() => setShelfOpen(false)}>×</button>
      </div>
      <div className="find">
        <svg viewBox="0 0 20 20" aria-hidden="true"><circle cx="8.5" cy="8.5" r="5.5" /><path d="m13 13 4 4" /></svg>
        <input aria-label="搜索材料" placeholder="搜爆款、评论、经历、跨行业标题" value={query} onChange={event => setQuery(event.target.value)} autoComplete="off" />
      </div>
      <div className="d-filters" role="group" aria-label="材料类型">
        {shelfFilters.map(item => <button key={item.id || 'all'} type="button" aria-pressed={filter === item.id} onClick={() => setFilter(item.id)}>{item.label}</button>)}
      </div>
      <div className="shelf-results">
        <p className="search-summary">
          <span>共 <span className="data">{shelf.data ? shelf.data.total : '—'}</span> 条{query.trim() ? `，按“${query.trim()}”筛选` : ''}</span>
          <span className="search-target">放入选题框 · {current?.title ?? '未选稿件'}</span>
        </p>
        {shelf.isPending ? <div className="search-empty">正在找材料…</div> : null}
        {shelf.isError ? <div className="search-empty">暂时读不到材料<button type="button" className="btn sm" onClick={() => void shelf.refetch()}>重试</button></div> : null}
        {shelf.data && !sources.length ? <div className="search-empty">没有匹配的材料</div> : null}
        <div className="d-res">
          {sources.slice(0, SHELF_LIMIT).map(item => {
            const readable = item.qualification === 'readable';
            const already = placed.has(`${item.sourceRef}:${item.sourceVersion}:0`);
            return <div key={item.fragmentId} className={`d-res-item${already ? ' on' : ''}`} draggable={false}>
              <div className="aux"><span className={`dot ${kindDot[item.sourceKind] ?? 'info'}`} />{kindLabel[item.sourceKind] ?? '材料'}{item.authorLabel ? ` · ${item.authorLabel}` : ''}</div>
              {item.sourceKind === 'comment'
                ? <div className="paper">{item.text}</div>
                : <div className="t">{item.title || item.text.slice(0, 40)}</div>}
              {item.sourceKind === 'comment' ? null : <div className="aux">{item.text.slice(0, 60)}</div>}
              <div className="row">
                {already ? <span className="aux">已在这个框里</span> : <button type="button" className="btn sm" disabled={placing === item.fragmentId || !readable} title={readable ? undefined : '这段来源现在不可引用'} onClick={() => void place(item)}>{placing === item.fragmentId ? '正在放进框…' : '放进框'}</button>}
              </div>
            </div>;
          })}
        </div>
        {placeError ? <p role="alert" className="search-empty">{placeError}</p> : null}
      </div>
    </aside> : null}
    <div className="viewport" ref={viewport}
      onPointerDown={beginPan} onPointerMove={movePan} onPointerUp={endPan} onPointerCancel={endPan}
      onWheel={event => { event.preventDefault(); zoomBy(event.deltaY > 0 ? 0.9 : 1.1); }}>
      <div className="viewport-grid" style={{
        backgroundSize: `${56 * view.zoom}px ${56 * view.zoom}px, ${14 * view.zoom}px ${14 * view.zoom}px`,
        backgroundPosition: `${view.x}px ${view.y}px, ${view.x}px ${view.y}px`,
      }} />
      <div className="world" style={{ transform: `translate(${view.x}px, ${view.y}px) scale(${view.zoom})` }}>
        {frames.map(frame => <article key={frame.work.workId}
          className={`frame${frame.work.workId === currentWorkId ? ' hot' : ''}${frame.work.workId === focus ? ' focused-frame' : ''}`}
          style={{ left: frame.x, top: frame.y, width: frame.w, height: frame.h }} aria-label={`选题框：${frame.title}`}>
          <header className="frame-head" onPointerDown={event => beginFrameDrag(event, frame)}
            onPointerMove={moveFrame} onPointerUp={event => void endFrame(event)} onPointerCancel={event => void endFrame(event)}>
            <div className="fh-row">
              <span className="fh-k data">{shortDate(frame.work.savedAt)}</span>
              <span className="ttl">{frame.title}</span>
              {frame.prepared ? <span className="fh-tag">系统备料</span> : null}
              <span className="grow" />
              <span className="cnt">{frame.cards.length} 张材料</span>
            </div>
            <div className="fh-row">
              <span className="fh-k">{frame.accountName ?? '还没选账号'}</span>
              <span className="grow" />
              <button type="button" className="frame-command" onClick={() => {
                setEditor(null); setSettingsFor(null);
                if (focus === frame.work.workId) { setFocus(null); fit(); } else focusFrame(frame);
              }}>{focus === frame.work.workId ? '退出专注' : '专注'}</button>
              <button type="button" className="frame-command" onClick={event => openSettings(frame, event)}>
                <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M3 6h14M3 14h14M7 3v6M13 11v6" /></svg>设置
              </button>
              <button type="button" className="btn sm" onClick={() => onOpenWork(frame.work.workId)}>去稿件</button>
            </div>
          </header>
          {frame.cards.map((card, index) => <div key={card.id} role="group" tabIndex={0}
            className={`mcard${card.note ? ' note' : ''}${editor?.blockId === card.id ? ' sel' : ''}`}
            style={{ left: FRAME_INSET + (index % CARD_COLS) * CARD_COL, top: FRAME_HEAD + Math.floor(index / CARD_COLS) * CARD_ROW }}
            aria-label={`${card.kind}：${card.text.slice(0, 40)}，按回车写思路`}
            onClick={event => openCard(card, 'memo', event)}
            onKeyDown={event => { if (event.key === 'Enter') { event.preventDefault(); openCard(card, 'memo', event); } }}>
            <button type="button" className="card-expand" onClick={event => { event.stopPropagation(); openCard(card, 'source', event); }}>展开原文</button>
            <div className="mcard-type">
              <span className={`dot ${card.dot}`} />{card.kind}<span className="grow" />
              <button type="button" className="mcard-x" aria-label={`移出桌面：${card.text.slice(0, 30)}`} onClick={event => { event.stopPropagation(); setEditor(null); onRemoveCard(card.id); }}>×</button>
            </div>
            {card.note ? <div className="txt">{card.text}</div> : <p className="q paper">{card.text}</p>}
            <div className="mcard-foot">
              <button type="button" className={`role-chip${card.role === '待分' ? ' soft' : ''}`} aria-label="修改材料用途" onClick={event => { event.stopPropagation(); openCard(card, 'role', event); }}>{card.role}</button>
              <button type="button" className="mnote" aria-label="编辑材料思路" title={card.memo || '点一下，写思路'} onClick={event => { event.stopPropagation(); openCard(card, 'memo', event); }}>{card.memo || <span className="muted">点一下，写思路</span>}</button>
            </div>
          </div>)}
        </article>)}
      </div>
      {focused ? <div className="focus-bar">
        <button type="button" className="focus-back" onClick={() => { setEditor(null); setFocus(null); fit(); }}>
          <svg viewBox="0 0 20 20" aria-hidden="true"><path d="m10 4-6 6 6 6M4 10h13" /></svg>返回全桌
        </button>
        <span><button type="button" className="btn sm primary" onClick={() => onOpenWork(focused.work.workId)}>去稿件</button></span>
      </div> : null}
      {sourceFocus ? <section className="desk-editor d-insp" role="region" aria-label="来源原文">
        <div className="insp-head"><b>来源原文</b><span className="grow" /><button type="button" className="modal-x" aria-label="收起原文" onClick={onClearSourceFocus}>×</button></div>
        <div className="insp-body">
          <p className="insp-source">{currentDocument.excerpts.find(item => item.citation.sourceRef === sourceFocus.sourceRef && item.citation.sourceVersion === sourceFocus.sourceVersion && item.citation.start === sourceFocus.start && item.citation.end === sourceFocus.end)?.selectedText ?? '这段原文暂时读不到。'}</p>
          <p className="insp-source-meta">{kindLabel[sourceFocus.sourceKind] ?? '来源'} · 采集于 {new Date(sourceFocus.asOf).toLocaleDateString('zh-CN')}</p>
        </div>
      </section> : null}
      {editorCard && editor?.kind === 'memo' && !sourceFocus ? <section className="desk-editor d-insp" role="region" aria-label="材料思路" ref={inspector} style={inspectorStyle(editor.anchor, inspectorHeight)}>
        <div className="insp-head"><b>材料思路</b><span className="grow" /><button type="button" className="modal-x" aria-label="收起编辑" onClick={() => setEditor(null)}>×</button></div>
        <div className="insp-body">
          <p className="editor-caption">{editorCard.text}</p>
          {editorCard.note ? <>
            <label className="sr" htmlFor="desk-note-text">便签内容</label>
            <textarea className="input" id="desk-note-text" rows={4} placeholder="随手记下的一点想法" value={editorCard.expression} onChange={event => onUpdateCard(editorCard.id, { expression: event.target.value })} />
          </> : null}
          <label className="sr" htmlFor="desk-card-memo">材料思路</label>
          <textarea className="input" id="desk-card-memo" rows={4} placeholder="这张材料准备怎么用？" value={editorCard.memo} onChange={event => onUpdateCard(editorCard.id, { note: event.target.value })} />
        </div>
        <div className="insp-foot"><button type="button" className="btn sm" onClick={() => setEditor(null)}>完成</button></div>
      </section> : null}
      {editorCard && editor?.kind === 'role' && !sourceFocus ? <section className="desk-editor d-insp" role="region" aria-label="材料用途" ref={inspector} style={inspectorStyle(editor.anchor, inspectorHeight)}>
        <div className="insp-head"><b>材料用途</b><span className="grow" /><button type="button" className="modal-x" aria-label="收起编辑" onClick={() => setEditor(null)}>×</button></div>
        <div className="insp-body">
          <p className="editor-caption">{editorCard.text}</p>
          <div className="role-picks" role="group" aria-label="材料用途">
            {rolePicks.map(item => <button key={item.id} type="button" aria-pressed={editorCard.group === item.id} onClick={() => onUpdateCard(editorCard.id, { group: item.id })}>{item.label}</button>)}
          </div>
        </div>
      </section> : null}
      {editorCard && editor?.kind === 'source' && !sourceFocus ? <section className="desk-editor d-insp" role="region" aria-label="原文与来源" ref={inspector} style={inspectorStyle(editor.anchor, inspectorHeight, 440)}>
        <div className="insp-head"><b>{editorCard.kind} · 原文与来源</b><span className="grow" /><button type="button" className="modal-x" aria-label="收起编辑" onClick={() => setEditor(null)}>×</button></div>
        <div className="insp-body">
          <div className="insp-source">{editorCard.text}</div>
          <div className="insp-source-meta">{editorCard.meta}</div>
        </div>
      </section> : null}
      {settingsFrame && !sourceFocus && !editorCard ? <section className="desk-editor d-insp" role="region" aria-label="选题框设置" ref={inspector} style={inspectorStyle(settingsAnchor, inspectorHeight, 340)}>
        <div className="insp-head"><b>选题框设置</b><span className="grow" /><button type="button" className="modal-x" aria-label="关闭设置" onClick={() => setSettingsFor(null)}>×</button></div>
        <div className="insp-body">
          {settingsFrame.work.workId === currentWorkId ? <>
            <p className="editor-caption">改完自动保存到这篇稿件。</p>
            <label className="lbl" htmlFor="desk-frame-title">选题</label>
            <input id="desk-frame-title" className="input" value={settingsFrame.title} onChange={event => onRenameTitle(event.target.value)} />
            <label className="lbl" htmlFor="desk-frame-account">给哪个账号写</label>
            <select id="desk-frame-account" className="input" value={currentDocument.accountRef ?? ''} onChange={event => onPickAccount(event.target.value || null)}>
              <option value="">不指定</option>
              {accounts.map(account => <option key={account.ref} value={account.ref}>{account.name}</option>)}
            </select>
            <p className="lbl">材料 · {settingsFrame.cards.length} 张 · 每张写清用途和思路</p>
            {settingsFrame.cards.length ? settingsFrame.cards.map((card, index) => <div className="mrow" key={card.id}>
              <div className="mrow-top"><span className={`dot ${card.dot}`} /><span className="aux">{card.kind}</span><span className="mrow-text">{card.text}</span></div>
              <div className="mrow-edit">
                <select className="input sm" aria-label={`材料 ${index + 1} 用途`} value={card.group} onChange={event => onUpdateCard(card.id, { group: event.target.value as CanvasGroup })}>
                  {rolePicks.map(item => <option key={item.id} value={item.id}>{item.label}</option>)}
                </select>
                <input className="input sm" aria-label={`材料 ${index + 1} 思路`} placeholder="写给 AI 的一句思路" value={card.memo} onChange={event => onUpdateCard(card.id, { note: event.target.value })} />
              </div>
            </div>) : <p className="aux">还没有材料。从左边点“找材料”，选“放进框”。</p>}
          </> : <>
            <p className="insp-source">{settingsFrame.title}</p>
            <p className="insp-source-meta">{settingsFrame.accountName ?? '还没选账号'} · {settingsFrame.cards.length} 张材料</p>
            <p className="editor-caption">这篇不是当前打开的稿件，切过去才能改标题和账号。</p>
            <button type="button" className="btn sm primary" onClick={() => onSwitchWork(settingsFrame.work.workId)}>切到这篇</button>
          </>}
        </div>
      </section> : null}
      <div className="desk-tools">
        <div className="desk-tool-group material-tools" role="group" aria-label="材料与选题">
          <span className="desk-current">{current ? <>取材到 <b>{current.title}</b></> : '选一个选题框，开始组料'}</span>
          <button type="button" className="desk-tool" aria-expanded={shelfOpen} aria-controls="deskSearch" onClick={() => { setEditor(null); setSettingsFor(null); setShelfOpen(open => !open); }}>
            <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="10.5" cy="10.5" r="6.5" /><path d="m15.5 15.5 5 5" /></svg>找材料
          </button>
          <span className="tool-divider" aria-hidden="true" />
          <button type="button" className="desk-tool" aria-label="添加便签" title="添加便签" onClick={onAddNote}>
            <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 4h16v11l-5 5H4ZM15 20v-5h5M8 9h8M8 12h5" /></svg>便签
          </button>
          <button type="button" className="desk-tool" aria-label="新建选题框" title="新建选题框" disabled={onCreateFramePending} onClick={() => void onCreateFrame()}>
            <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M9 4H4v5M15 4h5v5M20 15v5h-5M9 20H4v-5M8 12h8M12 8v8" /></svg>{onCreateFramePending ? '正在建…' : '选题框'}
          </button>
        </div>
        <div className="desk-tool-group view-tools" role="group" aria-label="画布视图">
          <div className="zoom-box">
            <button type="button" aria-label="缩小" disabled={view.zoom <= MIN_ZOOM} onClick={() => zoomBy(0.8)}>−</button>
            <span role="status" aria-label="画布缩放">{Math.round(view.zoom * 100)}%</span>
            <button type="button" aria-label="放大" disabled={view.zoom >= MAX_ZOOM} onClick={() => zoomBy(1.25)}>＋</button>
          </div>
          <button type="button" className="btn sm" onClick={() => { setFocus(null); fit(); }}>看全部</button>
        </div>
      </div>
      {saveFailed ? <span role="status" className="desk-hint">位置没有保存成功，刷新后会回到原位</span> : null}
    </div>
  </div>;
}

function document_excerpt_key(document: WorkDocument, block: CanvasBlock): string {
  const excerpt = block.excerptId ? document.excerpts.find(item => item.id === block.excerptId) : undefined;
  return excerpt ? `${excerpt.citation.sourceRef}:${excerpt.citation.sourceVersion}:${excerpt.citation.start}` : '';
}
