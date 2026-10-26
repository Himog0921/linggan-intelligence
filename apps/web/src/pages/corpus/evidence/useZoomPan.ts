import { useEffect, useRef, useState, type MouseEvent, type PointerEvent, type RefObject } from 'react';
import { DOUBLE_CLICK_ZOOM, WHEEL_STEP, ZOOM_MAX, ZOOM_MIN, ZOOM_STEP, clampPan, zoomAbout, type Pan } from './zoomMath';

interface View {
  zoom: number;
  pan: Pan;
}

const FIT: View = { zoom: ZOOM_MIN, pan: { x: 0, y: 0 } };

/**
 * 查看器的缩放与平移：按钮、键盘、滚轮（以指针为锚）、双击、拖动。几何算法在 zoomMath（纯函数，有测试）。
 * 框和图片的 ref 由调用方持有并传入：这样返回值里只有状态与处理函数，渲染期间读它不算读 ref。
 * `resetKey` 一变（换了一张图）就回到「适合」：在渲染期间重置状态，而不是靠重建整个查看器——重建会把
 * 键盘用户刚按下的「下一张」按钮换成新节点，焦点就丢了。
 */
export function useZoomPan(frame: RefObject<HTMLDivElement | null>, image: RefObject<HTMLImageElement | null>, resetKey: string | number, enabled = true) {
  const [view, setView] = useState<View>(FIT);
  const [grabbing, setGrabbing] = useState(false);
  const [seenKey, setSeenKey] = useState(resetKey);
  if (seenKey !== resetKey) {
    setSeenKey(resetKey);
    setView(FIT);
    setGrabbing(false);
  }
  const drag = useRef<{ origin: Pan } | null>(null);

  function apply(zoom: number, pan: Pan) {
    const box = frame.current?.getBoundingClientRect();
    const size = { width: box?.width ?? 0, height: box?.height ?? 0 };
    const natural = { width: image.current?.clientWidth ?? 0, height: image.current?.clientHeight ?? 0 };
    setView({ zoom, pan: clampPan(pan, zoom, size, natural) });
  }

  function zoomTo(requested: number, anchor: Pan | null = null) {
    const next = zoomAbout(view.zoom, view.pan, requested, anchor);
    apply(next.zoom, next.pan);
  }

  const reset = () => apply(ZOOM_MIN, { x: 0, y: 0 });
  const anchorOf = (event: { clientX: number; clientY: number }): Pan => {
    const box = frame.current?.getBoundingClientRect();
    return { x: event.clientX - (box?.left ?? 0), y: event.clientY - (box?.top ?? 0) };
  };

  // 滚轮缩放需要非被动监听才能 preventDefault（React 的 onWheel 是被动的）。监听只注册一次，
  // 通过 `latest` 调用最新一次渲染里的处理函数。
  const latest = useRef<{ wheel: ((event: WheelEvent) => void) | null }>({ wheel: null });
  useEffect(() => {
    latest.current = {
      wheel: event => {
        const box = frame.current?.getBoundingClientRect();
        zoomTo(view.zoom * (event.deltaY < 0 ? WHEEL_STEP : 1 / WHEEL_STEP), { x: event.clientX - (box?.left ?? 0), y: event.clientY - (box?.top ?? 0) });
      },
    };
  });
  useEffect(() => {
    const element = frame.current;
    if (!element || !enabled) return;
    const onWheel = (event: WheelEvent) => {
      event.preventDefault();
      latest.current.wheel?.(event);
    };
    element.addEventListener('wheel', onWheel, { passive: false });
    return () => element.removeEventListener('wheel', onWheel);
  }, [frame, enabled]);

  return {
    zoom: view.zoom,
    pan: view.pan,
    grabbing,
    canZoomIn: view.zoom < ZOOM_MAX - 0.001,
    canZoomOut: view.zoom > ZOOM_MIN + 0.001,
    zoomIn: () => zoomTo(view.zoom * ZOOM_STEP),
    zoomOut: () => zoomTo(view.zoom / ZOOM_STEP),
    reset,
    /** 双击在「适合」与工作倍率之间切换，锚在点击处——看文字卡片一角再回来最快。 */
    onDoubleClick: (event: MouseEvent) => {
      if (!enabled) return;
      if (view.zoom > ZOOM_MIN) reset();
      else zoomTo(DOUBLE_CLICK_ZOOM, anchorOf(event));
    },
    onPointerDown: (event: PointerEvent<HTMLDivElement>) => {
      if (view.zoom <= ZOOM_MIN) return;
      drag.current = { origin: { x: event.clientX - view.pan.x, y: event.clientY - view.pan.y } };
      setGrabbing(true);
      event.currentTarget.setPointerCapture?.(event.pointerId);
    },
    onPointerMove: (event: PointerEvent<HTMLDivElement>) => {
      const state = drag.current;
      if (state) apply(view.zoom, { x: event.clientX - state.origin.x, y: event.clientY - state.origin.y });
    },
    onPointerEnd: (event: PointerEvent<HTMLDivElement>) => {
      if (!drag.current) return;
      drag.current = null;
      setGrabbing(false);
      if (event.currentTarget.hasPointerCapture?.(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId);
    },
  };
}
