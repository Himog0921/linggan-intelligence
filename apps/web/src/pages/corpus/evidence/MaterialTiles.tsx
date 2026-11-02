import { useCallback, useEffect, useId, useLayoutEffect, useRef, useState, type CSSProperties } from 'react';
import { createPortal } from 'react-dom';
import { segmentReading, type MaterialSegment, type MaterialSummary } from './evidencePresent';
import { visibleAnchor } from './evidenceAnchor';
import hint from './TitleTooltip.module.css';
import styles from './MaterialTiles.module.css';

const NAMES: Readonly<Record<string, string>> = { body: '正', comments: '评', media: '媒', ocr: '图', asr: '转' };
const MARKS: Record<MaterialSegment['fill'], string> = {
  ready: 'M2 6.5 4.5 9 10 3',
  partial: 'M2 9H6 M8 9H10',
  pending: 'M6 1.5a4.5 4.5 0 1 0 0 9 4.5 4.5 0 0 0 0-9ZM6 3.5V6l1.6 1',
  failed: 'M6 2V7 M6 9.5v.1',
  unknown: 'M3.3 4a2.7 2.7 0 0 1 5.4 0c0 1.8-2.7 1.8-2.7 3.4M6 10v.1',
  'not-applicable': 'M2 6H10',
};
const reading = (segment: MaterialSegment) => segment.applicable ? segmentReading(segment) : '本作品不适用';

/** The reading-row material group opens one local, nonmodal explanation; no material fetch is needed. */
export function MaterialTiles({ summary }: { summary: MaterialSummary }) {
  const [open, setOpen] = useState(false);
  const [position, setPosition] = useState<CSSProperties | null>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const panel = useRef<HTMLDivElement>(null);
  const id = useId();
  const close = useCallback((returnFocus = false) => {
    setOpen(false);
    if (returnFocus) trigger.current?.focus({ preventScroll: true });
  }, []);
  const updatePosition = useCallback(() => {
    const anchor = trigger.current, popup = panel.current;
    if (!anchor || !popup) return;
    const { box, laidOut } = visibleAnchor(anchor);
    const host = anchor.closest('[data-evidence-results]')?.getBoundingClientRect();
    const area = { left: Math.max(0, host?.left ?? 0), right: Math.min(window.innerWidth, host?.right ?? window.innerWidth),
      top: Math.max(0, host?.top ?? 0), bottom: Math.min(window.innerHeight, host?.bottom ?? window.innerHeight) };
    if (laidOut && (Math.min(box.right, area.right) - Math.max(box.left, area.left) <= 1
      || Math.min(box.bottom, area.bottom) - Math.max(box.top, area.top) <= 1)) {
      close();
      return;
    }
    const width = popup.offsetWidth, height = popup.offsetHeight;
    const left = Math.max(area.left + 8, Math.min(box.right - width, area.right - width - 8));
    let top = box.bottom + 6;
    if (top + height > area.bottom - 8) top = box.top - height - 6;
    top = Math.max(area.top + 8, Math.min(top, area.bottom - height - 8));
    setPosition({ left, top, maxHeight: Math.max(0, area.bottom - area.top - 16) });
  }, [close]);
  useLayoutEffect(() => {
    if (open) updatePosition();
  }, [open, summary, updatePosition]);
  useEffect(() => {
    if (!open) return;
    const outside = (event: PointerEvent | FocusEvent) => {
      if (event.target instanceof Node && !trigger.current?.contains(event.target) && !panel.current?.contains(event.target)) close();
    };
    const key = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      event.preventDefault(); event.stopPropagation(); close(true);
    };
    document.addEventListener('pointerdown', outside, true);
    document.addEventListener('focusin', outside);
    document.addEventListener('keydown', key, true);
    window.addEventListener('scroll', updatePosition, true);
    window.addEventListener('resize', updatePosition);
    const observer = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(updatePosition);
    if (panel.current) observer?.observe(panel.current);
    const frame = requestAnimationFrame(() => panel.current?.querySelector('button')?.focus({ preventScroll: true }));
    return () => {
      document.removeEventListener('pointerdown', outside, true);
      document.removeEventListener('focusin', outside);
      document.removeEventListener('keydown', key, true);
      window.removeEventListener('scroll', updatePosition, true);
      window.removeEventListener('resize', updatePosition);
      observer?.disconnect(); cancelAnimationFrame(frame);
    };
  }, [close, open, updatePosition]);
  return <>
    <button ref={trigger} type="button" className={styles.groupButton} data-material-tiles=""
      aria-label={`材料说明：${summary.segments.map(segment => `${segment.label} ${reading(segment)}`).join('；')}`}
      aria-expanded={open} aria-controls={id} aria-haspopup="dialog"
      onClick={event => { event.stopPropagation(); setOpen(value => !value); }}>
      {summary.segments.map(segment => <span key={segment.key} className={styles.tile} data-fill={segment.fill} aria-hidden="true">
        <span className={styles.name}>{NAMES[segment.key]}</span>
        <svg className={styles.mark} viewBox="0 0 12 12" aria-hidden="true" focusable="false"><path d={MARKS[segment.fill]}/></svg>
      </span>)}
    </button>
    {open ? createPortal(<div ref={panel} id={id} role="dialog" aria-modal="false" aria-labelledby={`${id}-title`}
      className={`${hint.tooltip} ${styles.panel}`} data-material-explanation=""
      style={{ ...position, visibility: position ? 'visible' : 'hidden' }}
      onClick={event => event.stopPropagation()} onPointerDown={event => event.stopPropagation()}>
      <header><h2 id={`${id}-title`}>材料说明</h2><button type="button" className={styles.close} onClick={() => close(true)}>关闭</button></header>
      <dl>{summary.segments.map(segment => <div key={segment.key}><dt>{segment.label}</dt><dd>{reading(segment)}</dd></div>)}</dl>
    </div>, document.body) : null}
  </>;
}
