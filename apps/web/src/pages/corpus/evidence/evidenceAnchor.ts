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
export function visibleAnchor(anchor: HTMLElement): { box: Box; workArea: Box; laidOut: boolean } {
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

