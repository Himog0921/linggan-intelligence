import { describe, expect, it } from 'vitest';
import { FRAME_CLEARANCE, firstFree, fitSize, isFree, nearestFree, placeFrames, type Box, type FrameWish } from './deskLayout';

const box = (x: number, y: number, w = 200, h = 100): Box => ({ x, y, w, h });
const wish = (id: string, w: number, h: number, pinned: { x: number; y: number } | null = null): FrameWish => ({ id, w, h, pinned });
/** True when no two boxes are closer than the clearance. */
const noneTouch = (boxes: Box[]) => boxes.every((a, i) => boxes.every((b, j) => i === j || isFree(a, [b])));

describe('nearestFree', () => {
  it('leaves a frame that is already clear of the others', () => {
    expect(nearestFree(box(500, 0), [box(0, 0)])).toEqual({ x: 500, y: 0 });
  });

  it('keeps a frame that sits exactly one clearance away', () => {
    expect(nearestFree(box(200 + FRAME_CLEARANCE, 0), [box(0, 0)])).toEqual({ x: 224, y: 0 });
  });

  it('moves a frame dropped on another to the closest side, not the far end of the desk', () => {
    // Deep in the middle of a tall frame, the right side is the shortest way out.
    expect(nearestFree(box(30, 100), [box(0, 0, 200, 400)])).toEqual({ x: 224, y: 100 });
    // Dropped low on a short frame: below is closer than the right side.
    expect(nearestFree(box(10, 90), [box(0, 0)])).toEqual({ x: 10, y: 124 });
    // Dropped high on it: above is closer.
    expect(nearestFree(box(30, 0), [box(0, 0)])).toEqual({ x: 30, y: -124 });
  });

  it('finds a gap between frames and never lands on one of them', () => {
    const others = [box(0, 0), box(224, 0), box(0, 124)];
    const spot = nearestFree(box(50, 50), others);
    expect(isFree({ ...box(0, 0), ...spot }, others)).toBe(true);
  });

  it('settles the same way every time when two ways out are equally near', () => {
    const first = nearestFree(box(100, 0, 100, 100), [box(0, 0, 200, 100)]);
    const again = nearestFree(box(100, 0, 100, 100), [box(0, 0, 200, 100)]);
    expect(first).toEqual(again);
  });
});

describe('fitSize', () => {
  const min = { w: 320, h: 220 };

  it('keeps a size that does not reach another frame', () => {
    expect(fitSize({ x: 0, y: 0, w: 600, h: 400 }, [box(900, 0, 300, 300)], min)).toEqual({ w: 600, h: 400 });
  });

  it('stops a frame short of the neighbour it was dragged into', () => {
    // The neighbour starts at x=700, so the frame may reach 700 - 24.
    expect(fitSize({ x: 0, y: 0, w: 900, h: 400 }, [box(700, 0, 300, 300)], min)).toEqual({ w: 676, h: 400 });
  });

  it('shortens the frame instead when that keeps more of it', () => {
    // The neighbour sits below: stopping at y = 450 - 24 keeps the whole width.
    expect(fitSize({ x: 0, y: 0, w: 900, h: 600 }, [box(0, 450, 900, 300)], min)).toEqual({ w: 900, h: 426 });
  });

  it('gives up when no allowed size is clear, so the caller can put the old size back', () => {
    expect(fitSize({ x: 0, y: 0, w: 900, h: 600 }, [box(100, 100, 300, 300)], min)).toBeNull();
  });
});

describe('placeFrames', () => {
  const COLUMN = 720;
  const GAP = 48;

  it('lays frames that have no position out in two columns, the shorter column first', () => {
    const spots = placeFrames([wish('a', 720, 300), wish('b', 720, 500), wish('c', 720, 300)], COLUMN, GAP);
    expect(spots.a).toEqual({ x: 0, y: 0 });
    expect(spots.b).toEqual({ x: 768, y: 0 });
    // Column one is shorter (300 < 500), so the third frame goes under the first.
    expect(spots.c).toEqual({ x: 0, y: 348 });
  });

  it('keeps a frame where the user put it', () => {
    const spots = placeFrames([wish('a', 720, 300, { x: 40, y: 80 })], COLUMN, GAP);
    expect(spots.a).toEqual({ x: 40, y: 80 });
  });

  it('puts a new frame to the right of a wide frame stretched across its place, level with it, not above it', () => {
    // The first frame was stretched to 1450 wide; the default second column would start at 768, inside it.
    const spots = placeFrames([wish('a', 1450, 760, { x: 0, y: 0 }), wish('b', 720, 300)], COLUMN, GAP);
    expect(spots.b).toEqual({ x: 1450 + GAP, y: 0 });
  });

  it('puts a new frame under a tall frame when there is no room beside it, never out of sight above the others', () => {
    const spots = placeFrames([wish('a', 2400, 760, { x: 0, y: 0 }), wish('b', 720, 300)], COLUMN, GAP);
    expect(spots.b!.y).toBeGreaterThanOrEqual(0);
    expect(noneTouch([{ ...spots.a!, w: 2400, h: 760 }, { ...spots.b!, w: 720, h: 300 }])).toBe(true);
  });

  it('moves a saved frame off another one that grew over it, and keeps the upper-left frame still', () => {
    const spots = placeFrames([wish('top', 720, 700, { x: 0, y: 0 }), wish('under', 720, 300, { x: 0, y: 400 })], COLUMN, GAP);
    expect(spots.top).toEqual({ x: 0, y: 0 });
    expect(noneTouch([{ ...spots.top!, w: 720, h: 700 }, { ...spots.under!, w: 720, h: 300 }])).toBe(true);
  });

  it('settles frames saved on top of each other before this rule existed', () => {
    const wishes = [wish('a', 720, 300, { x: 0, y: 0 }), wish('b', 720, 300, { x: 100, y: 50 }), wish('c', 720, 300, { x: 200, y: 80 })];
    const spots = placeFrames(wishes, COLUMN, GAP);
    expect(noneTouch(wishes.map(item => ({ ...spots[item.id]!, w: item.w, h: item.h })))).toBe(true);
  });

  it('leaves the frame being dragged exactly where it is held, and settles the others without it', () => {
    const wishes = [wish('a', 720, 300, { x: 0, y: 0 }), wish('held', 720, 300, { x: 100, y: 50 })];
    const spots = placeFrames(wishes, COLUMN, GAP, 'held');
    expect(spots.held).toEqual({ x: 100, y: 50 });
    expect(spots.a).toEqual({ x: 0, y: 0 });
  });

  it('does not let frames saved far away leave gaps in the default columns', () => {
    const spots = placeFrames([wish('far', 720, 300, { x: 5000, y: 5000 }), wish('a', 720, 300)], COLUMN, GAP);
    expect(spots.a).toEqual({ x: 0, y: 0 });
  });
});

describe('firstFree', () => {
  it('leaves a frame whose default place is clear', () => {
    expect(firstFree(box(768, 0), [box(0, 0, 720, 300)], 48)).toEqual({ x: 768, y: 0 });
  });

  it('goes right or down in reading order, never up or left of the default place', () => {
    const others = [box(0, 0, 1450, 760)];
    const spot = firstFree({ x: 0, y: 0, w: 720, h: 300 }, others, 48);
    expect(spot).toEqual({ x: 1498, y: 0 });
    expect(spot.x).toBeGreaterThanOrEqual(0);
    expect(spot.y).toBeGreaterThanOrEqual(0);
  });

  it('takes the free place that comes first in reading order', () => {
    // Higher comes first: a place further right on the top row beats the free place under the first frame.
    const others = [box(0, 0, 720, 300), box(768, 0, 720, 300)];
    expect(firstFree({ x: 0, y: 0, w: 720, h: 300 }, others, 48)).toEqual({ x: 1536, y: 0 });
    // Even next to a very tall frame, which has no room under it on the right, the top row is still the first choice.
    expect(firstFree({ x: 0, y: 0, w: 720, h: 300 }, [box(0, 0, 720, 300), box(768, 0, 720, 3000)], 48)).toEqual({ x: 1536, y: 0 });
  });
});
