/**
 * 查看器的缩放与平移算法（纯函数）。缩放挂在框内的图片上，框负责裁切；只有图片比框大时平移才有意义，
 * 所以偏移被夹在真实溢出范围内——否则图片能被拖到屏幕外，读者得猜怎么找回来。
 */
export const ZOOM_MIN = 1;
export const ZOOM_MAX = 6;
/** 按钮与键盘 +／- 的步进倍率；滚轮每格用更小的倍率。 */
export const ZOOM_STEP = 1.5;
export const WHEEL_STEP = 1.12;
/** 双击在「适合」与这个工作倍率之间切换。 */
export const DOUBLE_CLICK_ZOOM = 2.5;

export interface Pan {
  x: number;
  y: number;
}

export interface Size {
  width: number;
  height: number;
}

export function clampZoom(zoom: number): number {
  return Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, zoom));
}

export function clampPan(pan: Pan, zoom: number, frame: Size, image: Size): Pan {
  const overflowX = Math.max(0, image.width * zoom - frame.width);
  const overflowY = Math.max(0, image.height * zoom - frame.height);
  // `+ 0` 把 -0 规整成 0，避免界面上出现 translate(-0px)。
  return { x: Math.min(0, Math.max(-overflowX, pan.x)) + 0, y: Math.min(0, Math.max(-overflowY, pan.y)) + 0 };
}

/** 以框内一点为锚缩放：锚点下的像素留在锚点下，图片不会从读者正在看的地方漂走。回到 1× 时偏移归零。 */
export function zoomAbout(zoom: number, pan: Pan, requested: number, anchor: Pan | null): { zoom: number; pan: Pan } {
  const target = clampZoom(requested);
  if (target === zoom) return { zoom, pan };
  let next = pan;
  if (anchor !== null) {
    const ratio = target / zoom;
    next = { x: anchor.x - ratio * (anchor.x - pan.x), y: anchor.y - ratio * (anchor.y - pan.y) };
  }
  return { zoom: target, pan: target === ZOOM_MIN ? { x: 0, y: 0 } : next };
}
