import { useCallback, useEffect, useId, useLayoutEffect, useRef, useState, type FocusEvent, type KeyboardEvent } from 'react';
import { createPortal } from 'react-dom';
import { useTitleTooltip } from '../evidence/TitleTooltip';
import { Cover } from './Media';
import { Metric } from './Metric';
import { relevanceLabel } from './creatorsPresent';
import { workLink, type LinkContext } from './creatorsQuery';
import type { Work } from './creatorsTypes';
import type { WorkResources } from './useWorkResources';
import styles from './CreatorsPage.module.css';

/** 持续悬停这么久才展开；离开单元格时尚未打开的计时立即取消。 */
const OPEN_DELAY = 500;
const CLOSE_DELAY = 120;
const GAP = 8;
const MARGIN = 8;

/**
 * 表格里的作品预览。单元格只有缩图与单行标题：指针持续停留 500ms 才出现完整封面、
 * 完整标题、已知指标与原作品入口；快速划过不会延后弹出。
 * 键盘聚焦标题先给完整标题提示；按 ↓ 展开预览并把焦点移入，Esc 关闭并回到标题。
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
  const hintId = useId();
  const {
    anchorRef: titleAnchorRef,
    onFocus: onTitleFocus,
    onBlur: onTitleBlur,
    onPointerDown: onTitlePointerDown,
    close: closeTitleTip,
    tooltip: titleTooltip,
  } = useTitleTooltip<HTMLAnchorElement>(title, 'width', { delay: 250, maxWidth: 320 });
  const trigger = titleAnchorRef;
  const openTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const closeTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const openNow = useRef(false);
  const hovered = useRef(false);
  const layerHovered = useRef(false);
  const focusWithin = useRef(false);
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
  }, [clearTimers, trigger]);

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

  // 未打开就离开：取消待打开计时，快速划过不会在之后弹出。
  const maybeClose = useCallback(() => {
    if (!openNow.current) {
      clearTimers();
      return;
    }
    if (hovered.current || layerHovered.current || focusWithin.current) return;
    clearTimers();
    closeTimer.current = setTimeout(() => {
      if (hovered.current || layerHovered.current || focusWithin.current) return;
      close();
    }, CLOSE_DELAY);
  }, [clearTimers, close]);

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
    if (!coords) position();
  }, [open, coords, position]);

  useEffect(() => {
    if (!activate || !open) return;
    const node = layer.current?.querySelector<HTMLElement>('a,button');
    node?.focus({ preventScroll: true });
  }, [activate, open]);

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (event: globalThis.KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      event.stopPropagation();
      const active = document.activeElement;
      // 只有焦点本来就在作品单元或浮层里时才把焦点交回标题；左表里的 Esc 不抢焦点。
      const inside = !!active && (root.current?.contains(active) || layer.current?.contains(active));
      close(inside);
    };
    document.addEventListener('keydown', onKeyDown, true);
    return () => document.removeEventListener('keydown', onKeyDown, true);
  }, [close, open]);

  const onCellEnter = () => {
    hovered.current = true;
    show(OPEN_DELAY);
  };
  const onCellLeave = () => {
    hovered.current = false;
    maybeClose();
  };
  const onCellFocus = () => {
    focusWithin.current = true;
  };
  const onCellBlur = (event: FocusEvent<HTMLDivElement>) => {
    const next = event.relatedTarget as Node | null;
    if (next && (root.current?.contains(next) || layer.current?.contains(next))) return;
    focusWithin.current = false;
    maybeClose();
  };
  const onTitleKeyDown = (event: KeyboardEvent<HTMLAnchorElement>) => {
    if (event.key !== 'ArrowDown') return;
    event.preventDefault();
    closeTitleTip();
    show(0);
    // 先让浮层挂载，再把焦点移进「打开原作品」。
    requestAnimationFrame(() => setActivate(value => value + 1));
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
      <Cover url={resources.byRef.get(work.workRef)?.coverUrl ?? null} pending={resources.pending} className={styles.cover} />
      <a
        ref={titleAnchorRef}
        className={styles.workTitle}
        href={workLink(link, creatorKey, {}, work.workRef)}
        data-return-key={creatorKey}
        onFocus={onTitleFocus}
        onBlur={onTitleBlur}
        onPointerDown={onTitlePointerDown}
        onKeyDown={onTitleKeyDown}
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-describedby={hintId}
      >
        {title}
        {titleTooltip}
      </a>
      <span id={hintId} className="v7-sr-only">按向下方向键查看作品预览</span>
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
              maybeClose();
            }}
            onPointerDown={event => event.stopPropagation()}
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
