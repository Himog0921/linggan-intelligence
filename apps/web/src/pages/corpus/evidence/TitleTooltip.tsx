import { useCallback, useEffect, useLayoutEffect, useRef, useState, type ReactNode, type RefObject } from 'react';
import { createPortal } from 'react-dom';
import styles from './TitleTooltip.module.css';

export type TitleTruncation = 'width' | 'lines';

export interface TitleTooltipController<T extends HTMLElement> {
  anchorRef: RefObject<T | null>;
  onEnter: () => void;
  onLeave: () => void;
  onFocus: () => void;
  onBlur: () => void;
  onPointerDown: () => void;
  tooltip: ReactNode;
}

const OPEN_DELAY = 280;
const CLOSE_DELAY = 120;
const GAP = 6;
const VIEWPORT_MARGIN = 8;

interface Box {
  left: number;
  right: number;
  top: number;
  bottom: number;
}

const CLIPS = new Set(['auto', 'scroll', 'hidden', 'clip']);
const SCROLLS = new Set(['auto', 'scroll']);

/**
 * The table's local horizontal scroll clips the title, so the hint must be placed against the part of
 * the title that is actually visible inside the list work area, not against the raw anchor rectangle.
 * Walk the clipping ancestors: their intersection is the visible title, and the first horizontal
 * scroller is the work area whose left/right edges bound the hint.
 */
function visibleAnchor(anchor: HTMLElement): { box: Box; workArea: Box; laidOut: boolean } {
  const rect = anchor.getBoundingClientRect();
  const box: Box = { left: rect.left, right: rect.right, top: rect.top, bottom: rect.bottom };
  const viewport: Box = { left: 0, right: window.innerWidth, top: 0, bottom: window.innerHeight };
  if (rect.width <= 0 || rect.height <= 0) return { box, workArea: viewport, laidOut: false };

  let workArea = viewport;
  let foundWorkArea = false;
  for (let element = anchor.parentElement; element && element !== document.body; element = element.parentElement) {
    const overflow = window.getComputedStyle(element);
    const scrollX = SCROLLS.has(overflow.overflowX);
    if (CLIPS.has(overflow.overflowX) || CLIPS.has(overflow.overflowY)) {
      const clip = element.getBoundingClientRect();
      if (CLIPS.has(overflow.overflowX)) {
        box.left = Math.max(box.left, clip.left);
        box.right = Math.min(box.right, clip.right);
      }
      if (CLIPS.has(overflow.overflowY)) {
        box.top = Math.max(box.top, clip.top);
        box.bottom = Math.min(box.bottom, clip.bottom);
      }
      if (!foundWorkArea && scrollX) {
        workArea = { left: clip.left, right: clip.right, top: clip.top, bottom: clip.bottom };
        foundWorkArea = true;
      }
    }
  }
  return { box, workArea, laidOut: true };
}

/**
 * Shows the full work title only when the visible title is actually clipped.
 *
 * The hint is a small local portal rather than a shared overlay framework: it is used by the evidence
 * work title and the cover title, which clip in different ways (single-line ellipsis vs two-line clamp).
 * It is anchored to the title, flips above the title when the viewport has no room below, and follows
 * scroll/resize so the table's local horizontal scroll cannot clip or strand it. Native `title` attributes
 * are removed from those triggers so the browser tooltip and this hint never appear together.
 */
export function useTitleTooltip<T extends HTMLElement>(text: string, truncation: TitleTruncation): TitleTooltipController<T> {
  const anchorRef = useRef<T>(null);
  const tipRef = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const [coords, setCoords] = useState<{ top: number; left: number } | null>(null);
  const openTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const closeTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const openNow = useRef(false);
  const hovered = useRef(false);
  const focused = useRef(false);
  const tipHovered = useRef(false);
  const suppressFocus = useRef(false);
  const truncationNow = useRef(truncation);

  const clearOpenTimer = useCallback(() => {
    if (openTimer.current !== null) clearTimeout(openTimer.current);
    openTimer.current = null;
  }, []);

  const clearCloseTimer = useCallback(() => {
    if (closeTimer.current !== null) clearTimeout(closeTimer.current);
    closeTimer.current = null;
  }, []);

  const close = useCallback(() => {
    clearOpenTimer();
    clearCloseTimer();
    openNow.current = false;
    tipHovered.current = false;
    setOpen(false);
    setCoords(null);
  }, [clearCloseTimer, clearOpenTimer]);

  const isTruncated = useCallback(() => {
    const element = anchorRef.current;
    if (!element) return false;
    return truncationNow.current === 'width'
      ? element.scrollWidth > element.clientWidth + 1
      : element.scrollHeight > element.clientHeight + 1;
  }, []);

  const updatePosition = useCallback(() => {
    const anchor = anchorRef.current;
    const tip = tipRef.current;
    if (!anchor || !tip) return;
    const { box, workArea, laidOut } = visibleAnchor(anchor);
    if (laidOut && (box.right - box.left <= 1 || box.bottom - box.top <= 1)) {
      close();
      return;
    }
    if (box.bottom < 0 || box.top > window.innerHeight) {
      close();
      return;
    }
    const width = tip.offsetWidth;
    const height = tip.offsetHeight;
    let top = box.bottom + GAP;
    if (top + height > window.innerHeight - VIEWPORT_MARGIN) top = box.top - height - GAP;
    top = Math.max(VIEWPORT_MARGIN, Math.min(top, window.innerHeight - height - VIEWPORT_MARGIN));
    const minLeft = Math.max(workArea.left, 0) + VIEWPORT_MARGIN;
    const maxLeft = Math.min(workArea.right, window.innerWidth) - width - VIEWPORT_MARGIN;
    let left = Math.max(minLeft, Math.min(box.left, maxLeft));
    left = Math.max(VIEWPORT_MARGIN, Math.min(left, window.innerWidth - width - VIEWPORT_MARGIN));
    setCoords({ top, left });
  }, [close]);

  const show = useCallback((immediate: boolean) => {
    clearCloseTimer();
    if (openNow.current) return;
    if (!isTruncated()) return;
    clearOpenTimer();
    const reveal = () => {
      if (!isTruncated()) return;
      openNow.current = true;
      setOpen(true);
    };
    if (immediate) reveal();
    else openTimer.current = setTimeout(() => {
      openTimer.current = null;
      reveal();
    }, OPEN_DELAY);
  }, [clearCloseTimer, clearOpenTimer, isTruncated]);

  const maybeClose = useCallback(() => {
    clearOpenTimer();
    if (!openNow.current) return;
    if (hovered.current || focused.current || tipHovered.current) return;
    clearCloseTimer();
    closeTimer.current = setTimeout(() => {
      closeTimer.current = null;
      if (hovered.current || focused.current || tipHovered.current) return;
      close();
    }, CLOSE_DELAY);
  }, [clearCloseTimer, clearOpenTimer, close]);

  const onEnter = useCallback(() => {
    hovered.current = true;
    show(false);
  }, [show]);

  const onLeave = useCallback(() => {
    hovered.current = false;
    maybeClose();
  }, [maybeClose]);

  const onFocus = useCallback(() => {
    // A pointer press also focuses the title. That focus must not open the hint, or opening a detail
    // would leave it floating over the list; keyboard focus (tab/refocus) still opens it immediately.
    if (suppressFocus.current) {
      suppressFocus.current = false;
      return;
    }
    focused.current = true;
    show(true);
  }, [show]);

  const onBlur = useCallback(() => {
    focused.current = false;
    maybeClose();
  }, [maybeClose]);

  const onPointerDown = useCallback(() => {
    suppressFocus.current = true;
    close();
    // Focus normally lands right after pointerdown; clear the flag for later keyboard focus even if
    // this browser does not focus buttons on press.
    window.setTimeout(() => {
      suppressFocus.current = false;
    }, 0);
  }, [close]);

  useEffect(() => () => {
    clearOpenTimer();
    clearCloseTimer();
  }, [clearCloseTimer, clearOpenTimer]);

  useEffect(() => {
    truncationNow.current = truncation;
  }, [truncation]);

  useEffect(() => {
    if (!open) return;
    const onScroll = () => updatePosition();
    window.addEventListener('scroll', onScroll, true);
    window.addEventListener('resize', onScroll);
    return () => {
      window.removeEventListener('scroll', onScroll, true);
      window.removeEventListener('resize', onScroll);
    };
  }, [open, updatePosition]);

  useLayoutEffect(() => {
    if (!open) return;
    updatePosition();
  }, [open, updatePosition, text]);

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      event.stopPropagation();
      close();
    };
    document.addEventListener('keydown', onKeyDown, true);
    return () => document.removeEventListener('keydown', onKeyDown, true);
  }, [close, open]);

  // Width changes (inspector width, table local scroll, font swap) re-check truncation and move the hint.
  useEffect(() => {
    const anchor = anchorRef.current;
    if (!anchor || typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(() => {
      if (!isTruncated()) {
        close();
        return;
      }
      if (openNow.current) updatePosition();
    });
    observer.observe(anchor);
    return () => observer.disconnect();
  }, [close, isTruncated, text, updatePosition]);

  const tooltip = open && typeof document !== 'undefined'
    ? createPortal(
      <div
        ref={tipRef}
        className={styles.tooltip}
        data-title-tooltip=""
        aria-hidden="true"
        style={{ top: coords?.top ?? 0, left: coords?.left ?? 0, visibility: coords ? 'visible' : 'hidden' }}
        onMouseEnter={() => {
          tipHovered.current = true;
          clearCloseTimer();
        }}
        onMouseLeave={() => {
          tipHovered.current = false;
          maybeClose();
        }}
        onClick={event => event.stopPropagation()}
        onPointerDown={event => event.stopPropagation()}
        onMouseDown={event => event.stopPropagation()}
      >
        {text}
      </div>,
      document.body,
    )
    : null;

  return { anchorRef, onEnter, onLeave, onFocus, onBlur, onPointerDown, tooltip };
}
