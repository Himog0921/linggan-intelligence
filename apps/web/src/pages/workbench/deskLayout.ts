/** Where the desk puts its topic frames. Pure geometry in world units, so the rules can be tested without a canvas. */
export interface Box { x: number; y: number; w: number; h: number }
export interface Spot { x: number; y: number }

/** The clear space kept between two frames, wide enough for a frame's edge and its shadow. */
export const FRAME_CLEARANCE = 24;

const apart = (a: Box, b: Box, gap: number) => a.x + a.w + gap <= b.x || b.x + b.w + gap <= a.x || a.y + a.h + gap <= b.y || b.y + b.h + gap <= a.y;
export const isFree = (box: Box, others: readonly Box[], gap = FRAME_CLEARANCE) => others.every(other => apart(box, other, gap));

/**
 * The position closest to where `box` is that leaves it clear of every other frame. A box that is already clear stays.
 * The candidates line the box up beside or above/below each other frame (and at its own x/y), so the answer is always
 * a position on some neighbour's edge — and the spot right of everything is always among them, so there is always one.
 */
export function nearestFree(box: Box, others: readonly Box[], gap = FRAME_CLEARANCE): Spot {
  if (isFree(box, others, gap)) return { x: box.x, y: box.y };
  const xs = [box.x, ...others.flatMap(other => [other.x + other.w + gap, other.x - box.w - gap])];
  const ys = [box.y, ...others.flatMap(other => [other.y + other.h + gap, other.y - box.h - gap])];
  let best: Spot | null = null;
  let bestDistance = Infinity;
  for (const y of ys) for (const x of xs) {
    if (!isFree({ ...box, x, y }, others, gap)) continue;
    const distance = (x - box.x) ** 2 + (y - box.y) ** 2;
    // Equal distances go to the higher, then the more left spot, so the same layout always settles the same way.
    if (distance < bestDistance || distance === bestDistance && best && (y < best.y || y === best.y && x < best.x)) { best = { x, y }; bestDistance = distance; }
  }
  return best ?? { x: Math.max(...others.map(other => other.x + other.w)) + gap, y: box.y };
}

/**
 * Where a frame the desk places by itself goes when its default place is taken: the first free place in reading order
 * (higher first, then further left) that is not above or left of the default, so a new frame never lands out of sight
 * above the others. A frame the user places is different: that one goes to the nearest free place (`nearestFree`).
 */
export function firstFree(box: Box, others: readonly Box[], gap = FRAME_CLEARANCE): Spot {
  if (isFree(box, others, gap)) return { x: box.x, y: box.y };
  const xs = [box.x, ...others.map(other => other.x + other.w + gap).filter(x => x > box.x)];
  const ys = [box.y, ...others.map(other => other.y + other.h + gap).filter(y => y > box.y)];
  let best: Spot | null = null;
  for (const y of ys) for (const x of xs) {
    if (!isFree({ ...box, x, y }, others, gap)) continue;
    if (!best || y < best.y || y === best.y && x < best.x) best = { x, y };
  }
  // Right of everything is always free, so the search above cannot come back empty; this only satisfies the type.
  return best ?? { x: Math.max(...others.map(other => other.x + other.w)) + gap, y: box.y };
}

/**
 * The largest size, at most the one asked for, at which a frame resized from its corner stops short of the frames next
 * to it. null when no size at least `min` is clear, so the caller can put the old size back.
 */
export function fitSize(box: Box, others: readonly Box[], min: { w: number; h: number }, gap = FRAME_CLEARANCE): { w: number; h: number } | null {
  let { w, h } = box;
  for (let attempt = 0; attempt <= others.length; attempt++) {
    const current = { ...box, w, h };
    const hit = others.find(other => !apart(current, other, gap));
    if (!hit) return { w, h };
    // Either stop short of the blocking frame to its left or above it, whichever leaves more of the frame.
    const options = [
      { w: hit.x - gap - box.x, h },
      { w, h: hit.y - gap - box.y },
    ].filter(option => option.w >= min.w && option.h >= min.h && (option.w < w || option.h < h));
    if (!options.length) return null;
    const area = (option: { w: number; h: number }) => option.w * option.h;
    const next = options.reduce((best, option) => area(option) > area(best) ? option : best);
    ({ w, h } = next);
  }
  return null;
}

export interface FrameWish { id: string; w: number; h: number; pinned: Spot | null }

/**
 * Settles every frame at a position where no two frames overlap.
 *  - A frame the user has placed keeps its position unless an earlier-placed frame is in the way (a frame grown by new
 *    cards, or an overlap saved before this rule): it moves to the nearest free spot. Placed frames are taken top to
 *    bottom, left to right, so the upper-left frame stays put.
 *  - A frame without a position goes to its place in the two-column default (the shorter column first) and, if a placed
 *    frame is there, to the first free place after it in reading order.
 *  - `active` is the frame being dragged or resized: it is left exactly where the user holds it, and the others settle
 *    as if it were not there. It finds its clear spot when it is released.
 */
export function placeFrames(wishes: readonly FrameWish[], columnWidth: number, gap: number, active: string | null = null): Record<string, Spot> {
  const spots: Record<string, Spot> = {};
  const placed: Box[] = [];
  const take = (wish: FrameWish, spot: Spot) => { spots[wish.id] = spot; if (wish.id !== active) placed.push({ ...spot, w: wish.w, h: wish.h }); };
  const pinned = wishes.filter(wish => wish.pinned).sort((a, b) => a.pinned!.y - b.pinned!.y || a.pinned!.x - b.pinned!.x);
  for (const wish of pinned) {
    const wanted = { ...wish.pinned!, w: wish.w, h: wish.h };
    take(wish, wish.id === active ? wish.pinned! : nearestFree(wanted, placed));
  }
  const bottoms: [number, number] = [0, 0];
  for (const wish of wishes) {
    if (wish.pinned) continue;
    const column = bottoms[0] <= bottoms[1] ? 0 : 1;
    const wanted = { x: column * (columnWidth + gap), y: bottoms[column], w: wish.w, h: wish.h };
    const spot = firstFree(wanted, placed, gap);
    take(wish, spot);
    bottoms[column] = Math.max(bottoms[column], spot.y + wish.h) + gap;
  }
  return spots;
}
