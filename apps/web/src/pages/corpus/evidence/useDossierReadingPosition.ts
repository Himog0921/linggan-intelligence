import { useCallback, useEffect, useLayoutEffect, useRef, useState, type RefObject } from 'react';
import styles from './DossierDocument.module.css';

export interface ReadingPosition { anchor: string | null; within: number; offset: number; top: number }
const memory = new Map<string, ReadingPosition>();
function load(key: string): ReadingPosition | null {
  if (memory.has(key)) return memory.get(key) ?? null;
  try {
    const value: unknown = JSON.parse(localStorage.getItem(key) ?? 'null');
    if (value && typeof value === 'object' && 'top' in value && typeof value.top === 'number' && Number.isFinite(value.top)
      && 'within' in value && typeof value.within === 'number' && Number.isFinite(value.within)
      && 'anchor' in value && (value.anchor === null || typeof value.anchor === 'string')) {
      const offset = 'offset' in value ? value.offset : 0; // Compatible with earlier anchor-only preferences.
      if (typeof offset !== 'number' || !Number.isFinite(offset)) return null;
      return { anchor: value.anchor, within: Math.max(0, Math.min(1, value.within)), offset, top: Math.max(0, value.top) };
    }
  } catch { /* Position is optional; unavailable storage never blocks reading. */ }
  return null;
}
export function captureReadingPosition(root: HTMLElement, inset: number): ReadingPosition {
  if (root.scrollTop < 5) return { anchor: null, within: 0, offset: 0, top: 0 };
  const top = root.getBoundingClientRect().top + inset;
  const anchors = Array.from(root.querySelectorAll<HTMLElement>('[data-reading-anchor]'));
  let target = anchors[0];
  for (const anchor of anchors) { if (anchor.getBoundingClientRect().top <= top + 1) target = anchor; else break; }
  if (!target) return { anchor: null, within: 0, offset: 0, top: root.scrollTop };
  const rect = target.getBoundingClientRect();
  const height = Math.max(1, rect.height), delta = top - rect.top;
  const within = Math.max(0, Math.min(1, delta / height));
  // Paragraph gaps are pixel distances, not part of a paragraph's reflowing height.
  return { anchor: target.dataset.readingAnchor ?? null, within, offset: delta - within * height, top: root.scrollTop };
}
export function restoreReadingPosition(root: HTMLElement, inset: number, position: ReadingPosition | null) {
  if (!position || position.top < 5) { root.scrollTop = 0; return; }
  const target = Array.from(root.querySelectorAll<HTMLElement>('[data-reading-anchor]')).find(el => el.dataset.readingAnchor === position.anchor);
  if (!target) { root.scrollTop = position.top; return; }
  const rect = target.getBoundingClientRect();
  root.scrollTop = root.scrollTop + rect.top - root.getBoundingClientRect().top - inset + Math.max(1, rect.height) * position.within + position.offset;
}
export function jumpReading(root: HTMLElement, target: HTMLElement, inset: number, fromTop = false) {
  const top = fromTop ? 0 : Math.max(0, root.scrollTop + target.getBoundingClientRect().top - root.getBoundingClientRect().top - inset);
  const behavior = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ? 'auto' : 'smooth';
  root.scrollTo({ top, behavior });
  target.focus({ preventScroll: true });
  const located = styles.located;
  if (located) {
    target.classList.remove(located);
    requestAnimationFrame(() => target.classList.add(located));
    window.setTimeout(() => target.classList.remove(located), 1500);
  }
}

/** Stores opaque anchors and geometry only. Source text is never persisted here. */
export function useDossierReadingPosition(rootRef: RefObject<HTMLDivElement | null>, toolbarRef: RefObject<HTMLDivElement | null>, key: string, active: boolean, revision: string) {
  const [current, setCurrent] = useState<{ section: string | null; group: string | null }>({ section: null, group: null });
  const last = useRef<ReadingPosition | null>(null);
  const suppress = useRef(false);
  const inset = useCallback(() => (toolbarRef.current?.getBoundingClientRect().height ?? 55) + 14, [toolbarRef]);
  const capture = useCallback(() => rootRef.current ? captureReadingPosition(rootRef.current, inset()) : null, [rootRef, inset]);
  const restore = useCallback((position: ReadingPosition | null) => {
    const root = rootRef.current;
    if (!root) return;
    suppress.current = true;
    restoreReadingPosition(root, inset(), position);
    requestAnimationFrame(() => { suppress.current = false; });
  }, [rootRef, inset]);
  const save = useCallback(() => {
    if (suppress.current || !rootRef.current || rootRef.current.clientHeight === 0) return;
    const position = capture();
    if (position) { last.current = position; memory.set(key, position); try { localStorage.setItem(key, JSON.stringify(position)); } catch { /* In-memory reading remains usable. */ } }
  }, [capture, key, rootRef]);
  useLayoutEffect(() => {
    if (!active) return;
    const position = load(key); last.current = position;
    const frame = requestAnimationFrame(() => restore(position));
    return () => cancelAnimationFrame(frame);
  }, [active, key, restore, revision]);
  useEffect(() => {
    const root = rootRef.current;
    if (!root || !active) return;
    let frame = 0, writeTimer = 0, resizeFrame = 0;
    let width = root.clientWidth;
    let lastForKey = last.current;
    const updateCurrent = () => {
      frame = 0;
      const y = root.getBoundingClientRect().top + inset() + 6;
      let section: string | null = null, group: string | null = null;
      for (const element of Array.from(root.querySelectorAll<HTMLElement>('[data-section-ref]'))) { if (element.getBoundingClientRect().top <= y || section === null) section = element.dataset.sectionRef ?? null; }
      for (const element of Array.from(root.querySelectorAll<HTMLElement>('[data-transcript-paragraph]'))) { if (element.getBoundingClientRect().top <= y) group = element.dataset.readingAnchor ?? null; else break; }
      setCurrent(previous => previous.section === section && previous.group === group ? previous : { section, group });
    };
    const scroll = () => {
      if (!frame) frame = requestAnimationFrame(updateCurrent);
      if (!suppress.current) {
        const position = capture();
        if (position) { last.current = position; lastForKey = position; memory.set(key, position); }
        clearTimeout(writeTimer); writeTimer = window.setTimeout(save, 220);
      }
    };
    const resize = () => {
      if (root.clientWidth === width || root.clientHeight === 0) return;
      width = root.clientWidth;
      cancelAnimationFrame(resizeFrame);
      resizeFrame = requestAnimationFrame(() => { restore(last.current); updateCurrent(); });
    };
    const observer = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(resize);
    observer?.observe(root);
    root.addEventListener('scroll', scroll, { passive: true });
    window.addEventListener('pagehide', save);
    frame = requestAnimationFrame(updateCurrent);
    return () => {
      if (lastForKey) { memory.set(key, lastForKey); try { localStorage.setItem(key, JSON.stringify(lastForKey)); } catch { /* optional storage */ } }
      root.removeEventListener('scroll', scroll); window.removeEventListener('pagehide', save); observer?.disconnect();
      cancelAnimationFrame(frame); cancelAnimationFrame(resizeFrame); clearTimeout(writeTimer);
    };
  }, [active, capture, inset, key, restore, revision, rootRef, save]);
  return { current, capture, restore, save, inset };
}
