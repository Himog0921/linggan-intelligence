import { useEffect, useRef, useState, type KeyboardEvent, type MouseEvent, type PointerEvent } from 'react';
import { stateMeta } from './evidencePresent';
import { positionText, type MediaObject } from './mediaObjects';
import { matches } from './useMediaQuery';
import styles from './MediaRail.module.css';

interface MediaRailProps {
  objects: readonly MediaObject[];
  /** 打开原尺寸查看器。`opener` 是被点的卡片：关闭后焦点回到它。 */
  onOpen: (index: number, opener: HTMLElement) => void;
}

/** 超过这个位移才算拖拽；纯点击仍是点击。 */
const DRAG_THRESHOLD = 4;
const REDUCED_MOTION = '(prefers-reduced-motion: reduce)';

function scrollRail(viewport: HTMLElement, left: number) {
  const behavior = matches(REDUCED_MOTION) ? 'auto' : 'smooth';
  if (typeof viewport.scrollTo === 'function') viewport.scrollTo({ left, behavior });
  else viewport.scrollLeft = left;
}

function Card({ object, total, onOpen }: { object: MediaObject; total: number; onOpen: MediaRailProps['onOpen'] }) {
  const [phase, setPhase] = useState<'loading' | 'loaded' | 'failed'>('loading');
  const failed = phase === 'failed';
  const label = `${object.label} 第 ${object.ordinal} 个本地媒体对象`;
  return (
    <button
      type="button"
      className={styles.card}
      data-index={object.index}
      data-match={object.isMatch}
      title={object.objectRef ? `本地对象 MED-${object.objectRef.slice(0, 8).toUpperCase()}` : '本地对象编号未表达'}
      onClick={event => onOpen(object.index, event.currentTarget)}
    >
      <span className={styles.frame}>
        {object.assetUrl && !failed ? (
          // 单张图读取失败只在原位提示：一个坏对象不能让整个作品看起来没有媒体。
          <img src={object.assetUrl} alt={label} loading="lazy" decoding="async" onLoad={() => setPhase('loaded')} onError={() => setPhase('failed')} />
        ) : object.assetUrl ? (
          <span className={styles.absent}>这个对象的本地副本读取失败；其余对象不受影响。</span>
        ) : object.inlineBlocked ? (
          <span className={styles.absent}>{`本地副本已验证，但 ${object.mime || '该类型'} 不在可内联集合中。`}</span>
        ) : (
          <span className={styles.absent}>{stateMeta(object.bytesState)[0]}</span>
        )}
        {object.assetUrl && phase === 'loading' ? <span className={styles.loading} role="status">读取中…</span> : null}
        <span className={styles.index}>{positionText(object.index, total)}</span>
      </span>
      <span className={styles.meta}>
        <b>{object.label}</b>
        <span>{object.derivedState ? `文字 ${stateMeta(object.derivedState)[0]}` : '文字未处理'}</span>
      </span>
    </button>
  );
}

/**
 * 「本机媒体材料」轨道。每个对象各自独立、各自有状态；图片 `contain` 不裁切（裁切等于悄悄改动证据）。
 * 交互：上一个／下一个按钮、进度条、鼠标拖拽（位移超过 4px 才进入拖拽）、纵向滚轮转横向（到边让位）、
 * 键盘 ← → Home End、「跳到命中」；平滑滚动在减少动态效果时直接跳。
 */
export function MediaRail({ objects, onOpen }: MediaRailProps) {
  const viewport = useRef<HTMLDivElement>(null);
  const strip = useRef<HTMLDivElement>(null);
  const gesture = useRef({ pressed: false, dragging: false, origin: 0, scroll: 0, moved: 0, swallow: false });
  const [view, setView] = useState({ index: 0, ratio: 0 });
  const [grabbing, setGrabbing] = useState(false);
  const total = objects.length;
  const matchIndex = objects.findIndex(object => object.isMatch);

  function closestIndex(): number {
    const element = viewport.current;
    const cards = [...(strip.current?.children ?? [])] as HTMLElement[];
    if (!element) return 0;
    let closest = 0;
    let distance = Infinity;
    cards.forEach((card, index) => {
      const delta = Math.abs(card.offsetLeft - element.scrollLeft);
      if (delta < distance) {
        distance = delta;
        closest = index;
      }
    });
    return closest;
  }

  function sync() {
    const element = viewport.current;
    if (!element) return;
    const maximum = element.scrollWidth - element.clientWidth;
    setView({ index: closestIndex(), ratio: maximum > 0 ? Math.min(1, Math.max(0, element.scrollLeft / maximum)) : 1 });
  }

  function step(delta: number) {
    const element = viewport.current;
    const target = (strip.current?.children[Math.min(total - 1, Math.max(0, closestIndex() + delta))] ?? null) as HTMLElement | null;
    if (element && target) scrollRail(element, target.offsetLeft);
  }

  function jumpToMatch() {
    const element = viewport.current;
    const card = (strip.current?.children[matchIndex] ?? null) as HTMLElement | null;
    if (!element || !card) return;
    scrollRail(element, card.offsetLeft);
    card.focus({ preventScroll: true });
  }

  // 纵向滚轮转横向：到边让位给页面。需要非被动监听才能 preventDefault，React 的 onWheel 是被动的。
  useEffect(() => {
    const element = viewport.current;
    if (!element) return;
    const onWheel = (event: WheelEvent) => {
      if (Math.abs(event.deltaY) <= Math.abs(event.deltaX)) return;
      const maximum = element.scrollWidth - element.clientWidth;
      if (maximum <= 0) return;
      const atStart = element.scrollLeft <= 0 && event.deltaY < 0;
      const atEnd = element.scrollLeft >= maximum - 1 && event.deltaY > 0;
      if (atStart || atEnd) return;
      event.preventDefault();
      element.scrollLeft += event.deltaY;
    };
    element.addEventListener('wheel', onWheel, { passive: false });
    return () => element.removeEventListener('wheel', onWheel);
  }, []);

  function onPointerDown(event: PointerEvent<HTMLDivElement>) {
    if (event.pointerType === 'touch' || event.button !== 0) return;
    gesture.current = { ...gesture.current, pressed: true, dragging: false, origin: event.clientX, scroll: event.currentTarget.scrollLeft, moved: 0 };
  }

  // 只在指针真的走过一段距离之后才捕获：在 pointerdown 就捕获，浏览器会把随后的 click 改投给轨道本身，
  // 点图片打开查看器就悄悄失效了。
  function onPointerMove(event: PointerEvent<HTMLDivElement>) {
    const state = gesture.current;
    if (!state.pressed) return;
    const delta = event.clientX - state.origin;
    state.moved = Math.max(state.moved, Math.abs(delta));
    if (!state.dragging) {
      if (state.moved <= DRAG_THRESHOLD) return;
      state.dragging = true;
      setGrabbing(true);
      event.currentTarget.setPointerCapture?.(event.pointerId);
    }
    event.currentTarget.scrollLeft = state.scroll - delta;
  }

  function endDrag(event: PointerEvent<HTMLDivElement>) {
    const state = gesture.current;
    if (!state.pressed) return;
    state.pressed = false;
    if (state.dragging) {
      state.dragging = false;
      setGrabbing(false);
      if (event.currentTarget.hasPointerCapture?.(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId);
      // 拖拽不是点击：否则松手时会在拖到的那张上打开查看器。
      state.swallow = true;
    }
    window.requestAnimationFrame(sync);
  }

  function swallowClick(event: MouseEvent) {
    if (!gesture.current.swallow) return;
    gesture.current.swallow = false;
    event.preventDefault();
    event.stopPropagation();
  }

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    const moves: Record<string, number> = { ArrowLeft: -1, ArrowRight: 1, Home: -total, End: total };
    const delta = moves[event.key];
    if (delta === undefined) return;
    event.preventDefault();
    step(delta);
  }

  const scale = total > 1 ? view.ratio * (1 - 1 / total) + 1 / total : 1;
  return (
    <section className={styles.module} aria-label="本机媒体材料轨道">
      <div className={styles.toolbar}>
        <div className={styles.summary}>
          <b>{`${String(total).padStart(2, '0')} 个本地媒体对象`}</b>
          <span>采集顺序 · 平台排列顺序未经验证</span>
        </div>
        <div className={styles.nav}>
          {matchIndex >= 0 ? (
            <button type="button" className={styles.jump} onClick={jumpToMatch}>
              跳到命中
            </button>
          ) : null}
          <button type="button" aria-label="上一个媒体对象" disabled={view.index <= 0} onClick={() => step(-1)}>
            ←
          </button>
          <span className={styles.position}>{positionText(view.index, total)}</span>
          <button type="button" aria-label="下一个媒体对象" disabled={view.index >= total - 1} onClick={() => step(1)}>
            →
          </button>
        </div>
      </div>
      <div
        ref={viewport}
        className={styles.viewport}
        role="group"
        aria-label="本地媒体对象，左右方向键切换"
        tabIndex={0}
        data-grabbing={grabbing}
        onScroll={() => window.requestAnimationFrame(sync)}
        onKeyDown={onKeyDown}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={endDrag}
        onPointerCancel={endDrag}
        onClickCapture={swallowClick}
      >
        <div ref={strip} className={styles.strip}>
          {objects.map(object => (
            <Card key={`${object.slotKey ?? object.index}:${object.assetUrl ?? ''}`} object={object} total={total} onOpen={onOpen} />
          ))}
        </div>
      </div>
      <div className={styles.progress} aria-hidden="true">
        <i style={{ transform: `scaleX(${scale})` }} />
      </div>
      <div className={styles.hint}>
        <span>拖拽 / 滚轮 / ← →</span>
        <em>点击图片进入原尺寸查看器</em>
      </div>
    </section>
  );
}
