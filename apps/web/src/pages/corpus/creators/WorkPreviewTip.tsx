import { useCallback, useEffect, useLayoutEffect, useRef, useState, type FocusEvent, type PointerEvent } from 'react';
import { createPortal } from 'react-dom';
import { Cover } from './Media';
import { Metric } from './Metric';
import { relevanceLabel } from './creatorsPresent';
import { workLink, type LinkContext } from './creatorsQuery';
import type { Work } from './creatorsTypes';
import type { WorkResources } from './useWorkResources';
import styles from './CreatorsPage.module.css';

const OPEN_DELAY = 250;
const CLOSE_DELAY = 120;
const GAP = 8;
const MARGIN = 8;

/**
 * 表格里的作品预览：标题与缩图是同一个对象。悬停或聚焦先出现完整封面、完整标题、
 * 已知关键数据和原作品入口；Enter 打开后焦点进入浮层，Esc 关闭并回到触发点。
 * 浮层是固定定位的 portal，表格自身的横向滚动不会裁切它。
 */
export function WorkPreviewCell({
  work,
  title,
  creatorKey,
  link,
  resources,
}: {
  work: Work;
  title: string;
  creatorKey: string;
  link: LinkContext;
  resources: WorkResources;
}) {
  const root = useRef<HTMLDivElement>(null);
  const layer = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const titleLink = useRef<HTMLAnchorElement>(null);
  const openTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const closeTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const openNow = useRef(false);
  const hovered = useRef(false);
  const layerHovered = useRef(false);
  const focusWithin = useRef(false);
  const startedFromPointer = useRef(false);
  const [open, setOpen] = useState(false);
  const [coords, setCoords] = useState<{ top: number; left: number } | null>(null);
  const [activate, setActivate] = useState(0);

  const clearTimers = useCallback(() => {
    if (openTimer.current) clearTimeout(openTimer.current);
    if (closeTimer.current) clearTimeout(closeTimer.current);
    openTimer.current = null;
    closeTimer.current = null;
  }, []);

  const close = useCallback((returnFocus = false) => {
    clearTimers();
    openNow.current = false;
    layerHovered.current = false;
    setOpen(false);
    setCoords(null);
    if (returnFocus) trigger.current?.focus({ preventScroll: true });
  }, [clearTimers]);

  const position = useCallback(() => {
    const anchor = root.current;
    const tip = layer.current;
    if (!anchor || !tip) return;
    const rect = anchor.getBoundingClientRect();
    const width = tip.offsetWidth;
    const height = tip.offsetHeight;
    const left = Math.max(MARGIN, Math.min(rect.left, window.innerWidth - width - MARGIN));
    let top = rect.bottom + GAP;
    if (top + height > window.innerHeight - MARGIN) top = rect.top - height - GAP;
    top = Math.max(MARGIN, Math.min(top, window.innerHeight - height - MARGIN));
    setCoords({ top, left });
  }, []);

  const show = useCallback((delay: number) => {
    clearTimers();
    if (openNow.current) return;
    const reveal = () => {
      openNow.current = true;
      setOpen(true);
    };
    if (delay > 0) openTimer.current = setTimeout(reveal, delay);
    else reveal();
  }, [clearTimers]);

  const maybeClose = useCallback(() => {
    if (!openNow.current) return;
    if (hovered.current || layerHovered.current || focusWithin.current) return;
    clearTimers();
    closeTimer.current = setTimeout(() => {
      if (hovered.current || layerHovered.current || focusWithin.current) return;
      close();
    }, CLOSE_DELAY);
  }, [close, clearTimers]);

  useEffect(() => () => clearTimers(), [clearTimers]);

  useEffect(() => {
    if (!open) return;
    const onScroll = () => position();
    window.addEventListener('scroll', onScroll, true);
    window.addEventListener('resize', onScroll);
    return () => {
      window.removeEventListener('scroll', onScroll, true);
      window.removeEventListener('resize', onScroll);
    };
  }, [open, position]);

  useLayoutEffect(() => {
    if (!open) return;
    position();
  }, [open, position]);

  useEffect(() => {
    if (!activate || !open) return;
    const node = layer.current?.querySelector<HTMLElement>('a,button');
    node?.focus({ preventScroll: true });
  }, [activate, open]);

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      event.stopPropagation();
      close(true);
    };
    document.addEventListener('keydown', onKeyDown, true);
    return () => document.removeEventListener('keydown', onKeyDown, true);
  }, [close, open]);

  const onCellEnter = () => {
    hovered.current = true;
    if (startedFromPointer.current) startedFromPointer.current = false;
    show(OPEN_DELAY);
  };
  const onCellLeave = () => {
    hovered.current = false;
    maybeClose();
  };
  const onCellFocus = () => {
    focusWithin.current = true;
    show(0);
  };
  const onCellBlur = (event: FocusEvent<HTMLDivElement>) => {
    const next = event.relatedTarget as Node | null;
    if (next && (root.current?.contains(next) || layer.current?.contains(next))) return;
    focusWithin.current = false;
    maybeClose();
  };
  const onPointerDown = () => {
    startedFromPointer.current = true;
    close();
  };
  const activatePreview = () => {
    focusWithin.current = true;
    if (!openNow.current) {
      show(0);
      // 先让浮层挂载，再移动焦点。
      requestAnimationFrame(() => setActivate(value => value + 1));
    } else {
      setActivate(value => value + 1);
    }
  };

  return (
    <div
      ref={root}
      className={styles.workCell}
      data-work-preview=""
      onMouseEnter={onCellEnter}
      onMouseLeave={onCellLeave}
      onFocus={onCellFocus}
      onBlur={onCellBlur}
    >
      <button
        type="button"
        className={styles.coverTrigger}
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-label={`预览作品 ${title}`}
        onPointerDown={onPointerDown}
        onClick={activatePreview}
      >
        <Cover url={resources.byRef.get(work.workRef)?.coverUrl ?? null} pending={resources.pending} className={styles.cover} />
      </button>
      <div className={styles.cellCopy}>
        <div className={styles.workTitleRow}>
          <a
            ref={titleLink}
            className={styles.workTitle}
            href={workLink(link, creatorKey, {}, work.workRef)}
            data-return-key={creatorKey}
            onPointerDown={onPointerDown}
          >
            {title}
          </a>
          <button
            ref={trigger}
            type="button"
            className={styles.previewTrigger}
            aria-haspopup="dialog"
            aria-expanded={open}
            aria-label={`查看完整预览：${title}`}
            onPointerDown={onPointerDown}
            onClick={activatePreview}
          >
            预览
          </button>
        </div>
        <span className={styles.secondary}>
          {relevanceLabel(work.relevance)} · 赞 <Metric value={work.likes} />
        </span>
      </div>
      {open && typeof document !== 'undefined'
        ? createPortal(
          <div
            ref={layer}
            className={styles.workPopover}
            role="dialog"
            aria-label={`作品预览：${title}`}
            style={{ top: coords?.top ?? 0, left: coords?.left ?? 0, visibility: coords ? 'visible' : 'hidden' }}
            onMouseEnter={() => {
              layerHovered.current = true;
              clearTimers();
            }}
            onMouseLeave={() => {
              layerHovered.current = false;
              maybeClose();
            }}
            onFocus={() => {
              focusWithin.current = true;
            }}
            onBlur={(event: FocusEvent<HTMLDivElement>) => {
              const next = event.relatedTarget as Node | null;
              if (next && layer.current?.contains(next)) return;
              focusWithin.current = false;
              if (next && root.current?.contains(next)) return;
              close();
            }}
            onPointerDown={(event: PointerEvent<HTMLDivElement>) => event.stopPropagation()}
          >
            <Cover url={resources.byRef.get(work.workRef)?.coverUrl ?? null} pending={resources.pending} className={styles.popoverCover} fallback="封面未取得" />
            <div className={styles.popoverCopy}>
              <p className={styles.popoverTitle}>{title}</p>
              <p className={styles.popoverMeta}>
                {relevanceLabel(work.relevance)} · 赞 <Metric value={work.likes} />
              </p>
              <a href={workLink(link, creatorKey, {}, work.workRef)} data-return-key={creatorKey}>
                打开原作品 →
              </a>
            </div>
          </div>,
          document.body,
        )
        : null}
    </div>
  );
}
