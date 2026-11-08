import { useCallback, useEffect, useRef, useState } from 'react';
import { useQueries, useQuery } from '@tanstack/react-query';
import * as api from './workbenchApi';
import type { CanvasBlock, WorkDocument, WorkSummary } from './workbenchModel';
import styles from './WorkbenchPage.module.css';

const CARD_W = 208;
const CARD_COL = 232;
const CARD_ROW = 190;
const CARD_COLS = 3;
const FRAME_INSET = 24;
const FRAME_HEAD = 84;
const FRAME_W = FRAME_INSET * 2 + (CARD_COLS - 1) * CARD_COL + CARD_W;
const FRAME_GAP = 48;
const MIN_ZOOM = 0.25;
const MAX_ZOOM = 2;

const kindLabel: Record<string, string> = {
  note: '原笔记', comment: '评论', ocr: '图片文字', transcript: '口播', frame_text: '画面文字', ops_publication: '发布记录',
};

interface Card {
  id: string;
  note: boolean;
  kind: string;
  text: string;
}
interface Frame {
  work: WorkSummary;
  x: number;
  y: number;
  w: number;
  h: number;
  cards: Card[];
}

function cardsOf(document: WorkDocument | undefined): Card[] {
  if (!document) return [];
  return document.canvas.map((block: CanvasBlock) => {
    const excerpt = block.excerptId ? document.excerpts.find(item => item.id === block.excerptId) : undefined;
    if (excerpt) return {
      id: block.id,
      note: false,
      kind: kindLabel[excerpt.citation.sourceKind] ?? '材料',
      text: excerpt.selectedText ?? '来源已受限或不可用',
    };
    const text = block.expression.trim() || block.note.trim();
    return { id: block.id, note: true, kind: '想法', text: text || '还没有写内容' };
  });
}

function layout(works: WorkSummary[], documents: (WorkDocument | undefined)[]): Frame[] {
  const bottoms: [number, number] = [0, 0];
  return works.map((work, index) => {
    const cards = cardsOf(documents[index]);
    const rows = Math.max(1, Math.ceil(cards.length / CARD_COLS));
    const height = FRAME_HEAD + rows * CARD_ROW + FRAME_INSET;
    const column = bottoms[0] <= bottoms[1] ? 0 : 1;
    const frame = { work, x: column * (FRAME_W + FRAME_GAP), y: bottoms[column], w: FRAME_W, h: height, cards };
    bottoms[column] += height + FRAME_GAP;
    return frame;
  });
}

export function MaterialDesk({ domain, works, currentWorkId, onOpenWork, onBrowse }: {
  domain: string;
  works: WorkSummary[];
  currentWorkId: string;
  onOpenWork: (workId: string) => void;
  onBrowse: () => void;
}) {
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
  const ready = documents.every(item => item.isSuccess);
  const [moved, setMoved] = useState<Record<string, { x: number; y: number }>>({});
  const [saveFailed, setSaveFailed] = useState(false);
  const frames = layout(works, documents.map(item => item.data?.revision.document)).map(frame => {
    const spot = moved[frame.work.workId] ?? saved.data?.find(item => item.workId === frame.work.workId);
    return spot ? { ...frame, x: spot.x, y: spot.y } : frame;
  });
  const [view, setView] = useState({ x: 64, y: 64, zoom: 1 });
  const [focus, setFocus] = useState<string | null>(null);
  const autoFitted = useRef(false);
  const viewport = useRef<HTMLDivElement | null>(null);
  const drag = useRef<{ x: number; y: number; panX: number; panY: number } | null>(null);
  const frameDrag = useRef<{ id: string; x: number; y: number; x0: number; y0: number; spot?: { x: number; y: number } } | null>(null);

  const fit = useCallback(() => {
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
  }, [frames]);

  useEffect(() => {
    if (autoFitted.current || !ready || !frames.length) return;
    autoFitted.current = true;
    fit();
  }, [ready, frames, fit]);

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
    setView(current => ({
      ...current,
      zoom: Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, Number((current.zoom * step).toFixed(3)))),
    }));
  };

  const beginPan = (event: React.PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) return;
    if ((event.target as HTMLElement).closest('.frame, .desk-tools, .focus-bar')) return;
    drag.current = { x: event.clientX, y: event.clientY, panX: view.x, panY: view.y };
    event.currentTarget.setPointerCapture?.(event.pointerId);
  };
  const movePan = (event: React.PointerEvent<HTMLDivElement>) => {
    const start = drag.current;
    if (!start) return;
    setView(current => ({ ...current, x: start.panX + event.clientX - start.x, y: start.panY + event.clientY - start.y }));
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

  if (!works.length) return <div className="desk immersive-desk">
    <p className={styles.empty}>这个领域还没有稿件。先在写稿页建立一个选题，材料会铺在这里。</p>
  </div>;

  const focused = frames.find(frame => frame.work.workId === focus) ?? null;

  return <div className={`desk immersive-desk${focus ? ' is-focused' : ''}`}>
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
          style={{ left: frame.x, top: frame.y, width: frame.w, height: frame.h }} aria-label={`选题框：${frame.work.title}`}>
          <header className="frame-head" onPointerDown={event => beginFrameDrag(event, frame)}
            onPointerMove={moveFrame} onPointerUp={event => void endFrame(event)} onPointerCancel={event => void endFrame(event)}>
            <div className="fh-row">
              <span className="fh-k data">{new Date(frame.work.savedAt).toLocaleDateString('zh-CN', { month: '2-digit', day: '2-digit' })}</span>
              <span className="ttl">{frame.work.title}</span>
              <span className="grow" />
              <span className="cnt">{frame.cards.length} 张材料</span>
            </div>
            <div className="fh-row">
              <span className="fh-k">最近保存 {new Date(frame.work.savedAt).toLocaleDateString('zh-CN')}</span>
              <span className="grow" />
              <button type="button" className="frame-command" onClick={() => focus === frame.work.workId ? (setFocus(null), fit()) : focusFrame(frame)}>{focus === frame.work.workId ? '退出专注' : '专注'}</button>
              <button type="button" className="frame-command" onClick={() => onOpenWork(frame.work.workId)}>去写稿</button>
            </div>
          </header>
          {frame.cards.map((card, index) => <div key={card.id} className={`mcard${card.note ? ' note' : ''}`}
            style={{ left: FRAME_INSET + (index % CARD_COLS) * CARD_COL, top: FRAME_HEAD + Math.floor(index / CARD_COLS) * CARD_ROW }}>
            <div className="mcard-type"><span className={`dot ${card.note ? 'warn' : 'info'}`} />{card.kind}</div>
            <p className={card.note ? 'txt' : 'q paper'}>{card.text}</p>
          </div>)}
        </article>)}
      </div>
      {focused ? <div className="focus-bar">
        <button type="button" className="focus-back" onClick={() => { setFocus(null); fit(); }}>
          <svg viewBox="0 0 20 20" aria-hidden="true"><path d="m10 4-6 6 6 6M4 10h13" /></svg>返回全桌
        </button>
      </div> : null}
      <div className="desk-tools">
        <div className="desk-tool-group material-tools" role="group" aria-label="材料与选题">
          <button type="button" className="desk-tool" onClick={onBrowse}>找材料</button>
          <span className="tool-divider" aria-hidden="true" />
          <button type="button" className="desk-tool" onClick={onBrowse}>摘一段</button>
        </div>
        <div className="desk-tool-group view-tools" role="group" aria-label="画布视图">
          <div className="zoom-box">
            <button type="button" aria-label="缩小" disabled={view.zoom <= MIN_ZOOM} onClick={() => zoomBy(0.8)}>−</button>
            <span role="status" aria-label="画布缩放">{Math.round(view.zoom * 100)}%</span>
            <button type="button" aria-label="放大" disabled={view.zoom >= MAX_ZOOM} onClick={() => zoomBy(1.25)}>＋</button>
          </div>
          <button type="button" className="btn sm" onClick={() => { setFocus(null); fit(); }}>看全部</button>
          {saveFailed ? <span role="status" className="desk-hint">位置没有保存成功，刷新后会回到原位</span> : null}
        </div>
      </div>
    </div>
  </div>;
}
