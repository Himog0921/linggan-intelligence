import { IntelligenceIcon } from '../../shared/IntelligenceIcon';
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { useQueries, useQuery } from '@tanstack/react-query';
import * as api from './workbenchApi';
import type { CanvasBlock, CanvasGroup, Citation, SourceItem, WorkDocument, WorkSummary } from './workbenchModel';
import styles from './WorkbenchPage.module.css';

const CARD_W = 208;
const CARD_COL = 232;
const CARD_GAP = 16;
const CARD_ESTIMATE = 170;
const FRAME_INSET = 24;
const FRAME_HEAD = 96;
const FRAME_COLS = 3;
const FRAME_W = FRAME_INSET * 2 + (FRAME_COLS - 1) * CARD_COL + CARD_W;
const FRAME_MIN_H = 300;
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
  x: number;
  y: number;
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
interface Spot {
  x: number;
  y: number;
}
interface SpotBox { left: number; right: number; top: number }
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
      x: 0,
      y: 0,
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
      x: 0,
      y: 0,
    };
  });
}

function layout(
  works: WorkSummary[],
  documents: (WorkDocument | undefined)[],
  accountOf: (ref: string | null | undefined) => string | null,
  heights: Record<string, number>,
  spots: Record<string, Spot>,
  sizes: Record<string, { w: number; h: number }>,
  positions: Record<string, Spot>,
): Frame[] {
  const bottoms: [number, number] = [0, 0];
  return works.map((work, index) => {
    const manual = sizes[work.workId];
    const cards = cardsOf(documents[index]).map(card => ({ ...card, ...(spots[card.id] ?? {}) }));
    const cols = Math.max(FRAME_COLS, Math.floor((Math.max(manual?.w ?? 0, FRAME_W) - FRAME_INSET) / CARD_COL));
    let col = 0;
    let rowY = FRAME_HEAD;
    let rowHeight = 0;
    let right = FRAME_W;
    let bottom = FRAME_HEAD;
    for (const card of cards) {
      if (!spots[card.id]) {
        if (col >= cols) {
          col = 0;
          rowY += rowHeight + CARD_GAP;
          rowHeight = 0;
        }
        card.x = FRAME_INSET + col * CARD_COL;
        card.y = rowY;
        rowHeight = Math.max(rowHeight, heights[card.id] ?? CARD_ESTIMATE);
        col += 1;
      }
      right = Math.max(right, card.x + CARD_W + FRAME_INSET);
      bottom = Math.max(bottom, card.y + (heights[card.id] ?? CARD_ESTIMATE));
    }
    const w = Math.max(right, manual?.w ?? 0);
    const h = Math.max(FRAME_MIN_H, bottom + FRAME_INSET, manual?.h ?? 0);
    const column = bottoms[0] <= bottoms[1] ? 0 : 1;
    const frame: Frame = {
      work,
      x: positions[work.workId]?.x ?? column * (FRAME_W + FRAME_GAP),
      y: positions[work.workId]?.y ?? bottoms[column],
      w,
      h,
      cards,
      prepared: (documents[index]?.materials ?? []).some(item => item.addedBy === 'prepared'),
      accountName: accountOf(documents[index]?.accountRef),
      title: documents[index]?.title ?? work.title,
    };
    bottoms[column] += h + FRAME_GAP;
    return frame;
  });
}

function Editable({ value, editable, className, label, onCommit }: {
  value: string;
  editable: boolean;
  className?: string;
  label?: string;
  onCommit: (next: string) => void;
}) {
  const ref = useRef<HTMLDivElement | null>(null);
  useLayoutEffect(() => {
    const node = ref.current;
    if (node && document.activeElement !== node && node.textContent !== value) node.textContent = value;
  }, [value]);
  return <div
    ref={ref}
    className={className}
    contentEditable={editable}
    suppressContentEditableWarning
    role={editable ? 'textbox' : undefined}
    aria-label={editable ? label : undefined}
    spellCheck={false}
    onPointerDown={event => { if (editable) event.stopPropagation(); }}
    onClick={event => { if (editable) event.stopPropagation(); }}
    onKeyDown={event => { if (editable && event.key === 'Enter') { event.preventDefault(); event.currentTarget.blur(); } }}
    onBlur={event => {
      const text = (event.currentTarget.textContent ?? '').replace(/\s+/g, ' ').trim();
      if (editable && text && text !== value) onCommit(text);
      else event.currentTarget.textContent = value;
    }}
  />;
}

const shortDate = (value: string) => new Date(value).toLocaleDateString('zh-CN', { month: '2-digit', day: '2-digit' });

function spotOf(element: Element | null): SpotBox | null {
  const rect = element?.getBoundingClientRect();
  return rect ? { left: rect.left, right: rect.right, top: rect.top } : null;
}
function inspectorStyle(spot: SpotBox | null, height: number, width = 320) {
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
  const savedCards = useQuery({
    queryKey: ['creation-desk-card-positions', domain],
    queryFn: ({ signal }) => api.deskCardPositions(domain, signal),
    retry: false,
    gcTime: 0,
  });
  const [shelfOpen, setShelfOpen] = useState(false);
  const [shelfScope, setShelfScope] = useState<'selected' | 'all'>(() => currentDocument.materials.length ? 'selected' : 'all');
  const [query, setQuery] = useState('');
  const [filter, setFilter] = useState<string>('');
  const [placing, setPlacing] = useState<string | null>(null);
  const [placeError, setPlaceError] = useState('');
  const shelf = useQuery({
    queryKey: ['creation-desk-shelf', domain, currentWorkId, shelfScope, currentDocument.materials, query, filter],
    queryFn: async ({ signal }) => {
      if (shelfScope === 'all') return api.sources({ domainRef: domain, query, ...(filter ? { kind: filter } : {}) }, signal);
      const collection = await api.resolveMaterials({ domainRef: domain, selections: currentDocument.materials }, signal);
      const items = collection.documents.flatMap(document => document.fragments.map(item => ({
        ...item, qualification: document.qualification === 'readable' ? item.qualification : document.qualification,
      }))).filter(item => (!filter || item.sourceKind === filter) && `${item.title} ${item.text}`.includes(query.trim()));
      return { items, total: items.length };
    },
    enabled: shelfOpen,
    retry: false,
    gcTime: 0,
  });
  const ready = documents.every(item => item.isSuccess);
  const [moved, setMoved] = useState<Record<string, Spot>>({});
  const [sized, setSized] = useState<Record<string, { w: number; h: number }>>({});
  const [cardMoved, setCardMoved] = useState<Record<string, Spot>>({});
  const [heights, setHeights] = useState<Record<string, number>>({});
  const [draggingId, setDraggingId] = useState<string | null>(null);
  const [saveFailed, setSaveFailed] = useState(false);
  const [editor, setEditor] = useState<{ kind: 'memo' | 'role' | 'source'; blockId: string; anchor: SpotBox | null } | null>(null);
  const [settingsFor, setSettingsFor] = useState<string | null>(null);
  const [settingsAnchor, setSettingsAnchor] = useState<SpotBox | null>(null);
  const inspector = useRef<HTMLElement | null>(null);
  const [inspectorHeight, setInspectorHeight] = useState(0);
  useLayoutEffect(() => {
    setInspectorHeight(inspector.current?.offsetHeight ?? 0);
  }, [editor, settingsFor]);
  const accountOf = useCallback((ref: string | null | undefined) => ref ? accounts.find(item => item.ref === ref)?.name ?? '账号未知' : null, [accounts]);
  const docOf = (work: WorkSummary, index: number) => work.workId === currentWorkId ? currentDocument : documents[index]?.data?.revision.document;
  const savedSpots: Record<string, Spot> = {};
  for (const item of savedCards.data ?? []) savedSpots[`${item.workId}:${item.blockId}`] = { x: item.x, y: item.y };
  const savedSizes: Record<string, { w: number; h: number }> = {};
  const framePositions: Record<string, Spot> = {};
  for (const item of saved.data ?? []) {
    framePositions[item.workId] = { x: item.x, y: item.y };
    if (item.width && item.height) savedSizes[item.workId] = { w: item.width, h: item.height };
  }
  const spots: Record<string, Spot> = { ...savedSpots, ...cardMoved };
  const sizes: Record<string, { w: number; h: number }> = { ...savedSizes, ...sized };
  const positions: Record<string, Spot> = { ...framePositions, ...moved };
  const frames = layout(works, works.map(docOf), accountOf, heights, spots, sizes, positions);
  const [view, setView] = useState({ x: 64, y: 64, zoom: 1 });
  const [focus, setFocus] = useState<string | null>(null);
  const autoFitted = useRef(false);
  const viewport = useRef<HTMLDivElement | null>(null);
  const drag = useRef<{ x: number; y: number; panX: number; panY: number } | null>(null);
  const frameDrag = useRef<{ id: string; x: number; y: number; x0: number; y0: number; moved: boolean; spot?: Spot } | null>(null);
  const frameDragged = useRef(false);
  const cardDrag = useRef<{ id: string; workId: string; ox: number; oy: number; px: number; py: number; moved: boolean; spot?: Spot } | null>(null);
  const cardDragged = useRef(false);
  const resizeDrag = useRef<{ id: string; px: number; py: number; w: number; h: number; spot?: { w: number; h: number } } | null>(null);
  const current = frames.find(frame => frame.work.workId === currentWorkId);

  const cardKey = frames.map(frame => frame.cards.map(card => card.id).join(',')).join('|');
  useLayoutEffect(() => {
    const box = viewport.current;
    if (!box || typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(entries => {
      setHeights(previous => {
        let changed = false;
        const next = { ...previous };
        for (const entry of entries) {
          const node = entry.target as HTMLElement;
          const id = node.dataset.card;
          const height = Math.round(node.offsetHeight);
          if (id && next[id] !== height) { next[id] = height; changed = true; }
        }
        return changed ? next : previous;
      });
    });
    box.querySelectorAll<HTMLElement>('.mcard[data-card]').forEach(node => observer.observe(node));
    return () => observer.disconnect();
  }, [cardKey]);

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

  const worldOf = (clientX: number, clientY: number) => {
    const rect = viewport.current?.getBoundingClientRect();
    return rect ? { x: (clientX - rect.left - view.x) / view.zoom, y: (clientY - rect.top - view.y) / view.zoom } : null;
  };

  const saveFrame = async (frame: Frame) => {
    const size = sizes[frame.work.workId] ?? null;
    try {
      await api.saveDeskPosition({
        domainRef: domain, workId: frame.work.workId, x: frame.x, y: frame.y,
        width: size?.w ?? null, height: size?.h ?? null,
      });
      setSaveFailed(false);
    } catch {
      setSaveFailed(true);
    }
  };

  const beginFrameDrag = (event: React.PointerEvent<HTMLElement>, frame: Frame) => {
    if (event.button !== 0 || (event.target as HTMLElement).closest('button, input, select, textarea, [contenteditable="true"]')) return;
    frameDrag.current = { id: frame.work.workId, x: event.clientX, y: event.clientY, x0: frame.x, y0: frame.y, moved: false };
    event.currentTarget.setPointerCapture?.(event.pointerId);
  };
  const moveFrame = (event: React.PointerEvent<HTMLElement>) => {
    const start = frameDrag.current;
    if (!start) return;
    if (!start.moved && Math.abs(event.clientX - start.x) + Math.abs(event.clientY - start.y) < 3) return;
    start.moved = true;
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
    frameDragged.current = start.moved;
    if (event.currentTarget.hasPointerCapture?.(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId);
    if (!start.spot) return;
    const frame = frames.find(item => item.work.workId === start.id);
    if (frame) await saveFrame(frame);
  };

  const beginCardDrag = (event: React.PointerEvent<HTMLElement>, frame: Frame, card: Card) => {
    if (event.button !== 0) return;
    if ((event.target as HTMLElement).closest('button, input, select, textarea, [contenteditable="true"]')) return;
    const world = worldOf(event.clientX, event.clientY);
    if (!world) return;
    cardDragged.current = false;
    cardDrag.current = {
      id: card.id, workId: frame.work.workId,
      ox: world.x - frame.x - card.x, oy: world.y - frame.y - card.y,
      px: event.clientX, py: event.clientY, moved: false,
    };
    event.currentTarget.setPointerCapture?.(event.pointerId);
    event.stopPropagation();
    setEditor(null);
    setSettingsFor(null);
  };
  const moveCard = (event: React.PointerEvent<HTMLElement>) => {
    const start = cardDrag.current;
    if (!start) return;
    if (!start.moved && Math.abs(event.clientX - start.px) + Math.abs(event.clientY - start.py) < 4) return;
    const world = worldOf(event.clientX, event.clientY);
    const frame = frames.find(item => item.work.workId === start.workId);
    if (!world || !frame) return;
    start.moved = true;
    cardDragged.current = true;
    setDraggingId(start.id);
    const spot = { x: Math.round(world.x - frame.x - start.ox), y: Math.round(world.y - frame.y - start.oy) };
    start.spot = spot;
    setCardMoved(previous => ({ ...previous, [start.id]: spot }));
  };
  const endCard = async (event: React.PointerEvent<HTMLElement>) => {
    const start = cardDrag.current;
    if (!start) return;
    cardDrag.current = null;
    setDraggingId(null);
    if (event.currentTarget.hasPointerCapture?.(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId);
    if (!start.moved || !start.spot) return;
    try {
      await api.saveDeskCardPosition({ domainRef: domain, workId: start.workId, blockId: start.id, x: start.spot.x, y: start.spot.y });
      setSaveFailed(false);
    } catch {
      setSaveFailed(true);
    }
  };

  const beginResize = (event: React.PointerEvent<HTMLElement>, frame: Frame) => {
    if (event.button !== 0) return;
    event.stopPropagation();
    setEditor(null);
    setSettingsFor(null);
    resizeDrag.current = { id: frame.work.workId, px: event.clientX, py: event.clientY, w: frame.w, h: frame.h };
    event.currentTarget.setPointerCapture?.(event.pointerId);
  };
  const moveResize = (event: React.PointerEvent<HTMLElement>) => {
    const start = resizeDrag.current;
    if (!start) return;
    const next = {
      w: Math.max(320, Math.round(start.w + (event.clientX - start.px) / view.zoom)),
      h: Math.max(220, Math.round(start.h + (event.clientY - start.py) / view.zoom)),
    };
    start.spot = next;
    setSized(previous => ({ ...previous, [start.id]: next }));
  };
  const endResize = async (event: React.PointerEvent<HTMLElement>) => {
    const start = resizeDrag.current;
    if (!start) return;
    resizeDrag.current = null;
    if (event.currentTarget.hasPointerCapture?.(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId);
    if (!start.spot) return;
    const frame = frames.find(item => item.work.workId === start.id);
    if (frame) await saveFrame(frame);
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
        <button type="button" className="modal-x" aria-label="收起找材料" onClick={() => setShelfOpen(false)}><IntelligenceIcon name="close" size={16}/></button>
      </div>
      <div className="d-filters" role="group" aria-label="材料范围">
        <button type="button" aria-pressed={shelfScope === 'selected'} onClick={() => setShelfScope('selected')}>本篇材料</button>
        <button type="button" aria-pressed={shelfScope === 'all'} onClick={() => setShelfScope('all')}>全部材料</button>
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
          {(shelfScope === 'selected' ? sources : sources.slice(0, SHELF_LIMIT)).map(item => {
            const readable = item.qualification === 'readable';
            const already = placed.has(JSON.stringify([item.sourceKind, item.sourceRef, item.sourceVersion, item.blockRef, item.field, 0]));
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
        {frames.map(frame => {
          const editable = frame.work.workId === currentWorkId;
          return <article key={frame.work.workId}
            className={`frame${frame.work.workId === currentWorkId ? ' hot' : ''}${frame.work.workId === focus ? ' focused-frame' : ''}`}
            style={{ left: frame.x, top: frame.y, width: frame.w, height: frame.h }} aria-label={`选题框：${frame.title}`}>
            <header className="frame-head" onPointerDown={event => beginFrameDrag(event, frame)}
              onPointerMove={moveFrame} onPointerUp={event => void endFrame(event)} onPointerCancel={event => void endFrame(event)}
              onClick={event => {
                if (frameDragged.current) { frameDragged.current = false; return; }
                if ((event.target as HTMLElement).closest('button, input, select, textarea, [contenteditable="true"]')) return;
                openSettings(frame, event);
              }}>
              <div className="fh-row">
                <span className="fh-k data">{shortDate(frame.work.savedAt)}</span>
                <Editable className="ttl" value={frame.title} editable={editable} label="选题框标题" onCommit={onRenameTitle} />
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
                  <IntelligenceIcon name="adjust" size={18}/>设置
                </button>
                <button type="button" className="btn sm" onClick={() => onOpenWork(frame.work.workId)}>去稿件</button>
              </div>
            </header>
            {frame.cards.map(card => <div key={card.id} role="group" tabIndex={0} data-card={card.id}
              className={`mcard${card.note ? ' note' : ''}${editor?.blockId === card.id ? ' sel' : ''}${draggingId === card.id ? ' dragging' : ''}`}
              style={{ left: card.x, top: card.y }}
              aria-label={`${card.kind}：${card.text.slice(0, 40)}，按回车写思路`}
              onPointerDown={event => beginCardDrag(event, frame, card)}
              onPointerMove={moveCard} onPointerUp={event => void endCard(event)} onPointerCancel={event => void endCard(event)}
              onClick={event => {
                if (cardDragged.current) { cardDragged.current = false; return; }
                openCard(card, 'memo', event);
              }}
              onKeyDown={event => { if (event.key === 'Enter') { event.preventDefault(); openCard(card, 'memo', event); } }}>
              <button type="button" className="card-expand" onClick={event => { event.stopPropagation(); openCard(card, 'source', event); }}>展开原文</button>
              <div className="mcard-type">
                <span className={`dot ${card.dot}`} />{card.kind}<span className="grow" />
                <button type="button" className="mcard-x" aria-label={`移出桌面：${card.text.slice(0, 30)}`} onClick={event => { event.stopPropagation(); setEditor(null); onRemoveCard(card.id); }}>×</button>
              </div>
              {card.note
                ? <Editable className="txt" value={card.text} editable={editable} label="便签内容" onCommit={next => onUpdateCard(card.id, { expression: next })} />
                : <p className="q paper">{card.text}</p>}
              <div className="mcard-foot">
                <button type="button" className={`role-chip${card.role === '待分' ? ' soft' : ''}`} aria-label="修改材料用途" onClick={event => { event.stopPropagation(); openCard(card, 'role', event); }}>{card.role}</button>
                <button type="button" className="mnote" aria-label="编辑材料思路" title={card.memo || '点一下，写思路'} onClick={event => { event.stopPropagation(); openCard(card, 'memo', event); }}>{card.memo || <span className="muted">点一下，写思路</span>}</button>
              </div>
            </div>)}
            <div className="frame-resize" aria-hidden="true"
              onPointerDown={event => beginResize(event, frame)}
              onPointerMove={moveResize} onPointerUp={event => void endResize(event)} onPointerCancel={event => void endResize(event)} />
          </article>;
        })}
      </div>
      {focused ? <div className="focus-bar">
        <button type="button" className="focus-back" onClick={() => { setEditor(null); setFocus(null); fit(); }}>
          <IntelligenceIcon name="back" size={18}/>返回全桌
        </button>
        <span><button type="button" className="btn sm primary" onClick={() => onOpenWork(focused.work.workId)}>去稿件</button></span>
      </div> : null}
      {sourceFocus ? <section className="desk-editor d-insp" role="region" aria-label="来源原文">
        <div className="insp-head"><b>来源原文</b><span className="grow" /><button type="button" className="modal-x" aria-label="收起原文" onClick={onClearSourceFocus}><IntelligenceIcon name="close" size={16}/></button></div>
        <div className="insp-body">
          <p className="insp-source">{currentDocument.excerpts.find(item => item.citation.sourceRef === sourceFocus.sourceRef && item.citation.sourceVersion === sourceFocus.sourceVersion && item.citation.start === sourceFocus.start && item.citation.end === sourceFocus.end)?.selectedText ?? '这段原文暂时读不到。'}</p>
          <p className="insp-source-meta">{kindLabel[sourceFocus.sourceKind] ?? '来源'} · 采集于 {new Date(sourceFocus.asOf).toLocaleDateString('zh-CN')}</p>
        </div>
      </section> : null}
      {editorCard && editor?.kind === 'memo' && !sourceFocus ? <section className="desk-editor d-insp" role="region" aria-label="材料思路" ref={inspector} style={inspectorStyle(editor.anchor, inspectorHeight)}>
        <div className="insp-head"><b>材料思路</b><span className="grow" /><button type="button" className="modal-x" aria-label="收起编辑" onClick={() => setEditor(null)}><IntelligenceIcon name="close" size={16}/></button></div>
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
        <div className="insp-head"><b>材料用途</b><span className="grow" /><button type="button" className="modal-x" aria-label="收起编辑" onClick={() => setEditor(null)}><IntelligenceIcon name="close" size={16}/></button></div>
        <div className="insp-body">
          <p className="editor-caption">{editorCard.text}</p>
          <div className="role-picks" role="group" aria-label="材料用途">
            {rolePicks.map(item => <button key={item.id} type="button" aria-pressed={editorCard.group === item.id} onClick={() => onUpdateCard(editorCard.id, { group: item.id })}>{item.label}</button>)}
          </div>
        </div>
      </section> : null}
      {editorCard && editor?.kind === 'source' && !sourceFocus ? <section className="desk-editor d-insp" role="region" aria-label="原文与来源" ref={inspector} style={inspectorStyle(editor.anchor, inspectorHeight, 440)}>
        <div className="insp-head"><b>{editorCard.kind} · 原文与来源</b><span className="grow" /><button type="button" className="modal-x" aria-label="收起编辑" onClick={() => setEditor(null)}><IntelligenceIcon name="close" size={16}/></button></div>
        <div className="insp-body">
          <div className="insp-source">{editorCard.text}</div>
          <div className="insp-source-meta">{editorCard.meta}</div>
        </div>
      </section> : null}
      {settingsFrame && !sourceFocus && !editorCard ? <section className="desk-editor d-insp" role="region" aria-label="选题框设置" ref={inspector} style={inspectorStyle(settingsAnchor, inspectorHeight, 340)}>
        <div className="insp-head"><b>选题框设置</b><span className="grow" /><button type="button" className="modal-x" aria-label="关闭设置" onClick={() => setSettingsFor(null)}><IntelligenceIcon name="close" size={16}/></button></div>
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
            <IntelligenceIcon name="search" size={18}/>找材料
          </button>
          <span className="tool-divider" aria-hidden="true" />
          <button type="button" className="desk-tool" aria-label="添加便签" title="添加便签" onClick={onAddNote}>
            <IntelligenceIcon name="sticky" size={18}/>便签
          </button>
          <button type="button" className="desk-tool" aria-label="新建选题框" title="新建选题框" disabled={onCreateFramePending} onClick={() => void onCreateFrame()}>
            <IntelligenceIcon name="frame-add" size={18}/>{onCreateFramePending ? '正在建…' : '选题框'}
          </button>
        </div>
        <div className="desk-tool-group view-tools" role="group" aria-label="画布视图">
          <div className="zoom-box">
            <button type="button" aria-label="缩小" disabled={view.zoom <= MIN_ZOOM} onClick={() => zoomBy(0.8)}><IntelligenceIcon name="zoom-out" size={16}/></button>
            <span role="status" aria-label="画布缩放">{Math.round(view.zoom * 100)}%</span>
            <button type="button" aria-label="放大" disabled={view.zoom >= MAX_ZOOM} onClick={() => zoomBy(1.25)}><IntelligenceIcon name="zoom-in" size={16}/></button>
          </div>
          <button type="button" className="btn sm" onClick={() => { setFocus(null); fit(); }}><IntelligenceIcon name="fit" size={16}/>看全部</button>
        </div>
      </div>
      {saveFailed ? <span role="status" className="desk-hint">位置没有保存成功，刷新后会回到原位</span> : null}
    </div>
  </div>;
}

function document_excerpt_key(document: WorkDocument, block: CanvasBlock): string {
  const excerpt = block.excerptId ? document.excerpts.find(item => item.id === block.excerptId) : undefined;
  return excerpt ? JSON.stringify([excerpt.citation.sourceKind, excerpt.citation.sourceRef, excerpt.citation.sourceVersion, excerpt.citation.blockRef, excerpt.citation.field, excerpt.citation.start]) : '';
}
