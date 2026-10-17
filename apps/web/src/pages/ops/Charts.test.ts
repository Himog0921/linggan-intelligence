import { describe, expect, it } from "vitest";
import { smoothSeriesPath } from "./curve";

describe("Ops trend curves", () => {
  it("uses smooth cubic segments instead of straight polylines", () => {
    const d = smoothSeriesPath([{ x: 0, y: 12 }, { x: 10, y: 4 }, { x: 20, y: 16 }]);
    expect(d).toContain(" C");
    expect(d.match(/ C/g)).toHaveLength(2);
    expect(d.startsWith("M0.00,12.00")).toBe(true);
  });

  it("never draws a line through an unknown or not-migrated measurement", () => {
    const d = smoothSeriesPath([
      { x: 0, y: 10 }, { x: 10, y: 7 }, null,
      { x: 30, y: 3 }, { x: 40, y: 1 },
    ]);
    expect(d.match(/M/g)).toHaveLength(2);
    expect(d.match(/ C/g)).toHaveLength(2);
    expect(d).toContain("M30.00,3.00");
    expect(d).not.toContain("20.00");
  });

  it("keeps its control points within the observed adjacent endpoint range", () => {
    const d = smoothSeriesPath([{ x: 0, y: 0 }, { x: 10, y: 100 }, { x: 20, y: 0 }]);
    const coordinates = [...d.matchAll(/ C(\d+\.?\d*),(-?\d+\.?\d*) (\d+\.?\d*),(-?\d+\.?\d*) /g)];
    expect(coordinates).toHaveLength(2);
    for (const match of coordinates) {
      expect(Number(match[2])).toBeGreaterThanOrEqual(0);
      expect(Number(match[2])).toBeLessThanOrEqual(100);
      expect(Number(match[4])).toBeGreaterThanOrEqual(0);
      expect(Number(match[4])).toBeLessThanOrEqual(100);
    }
  });

  it("renders no invented line if only isolated facts are known", () => {
    expect(smoothSeriesPath([null, null])).toBe("");
    expect(smoothSeriesPath([{ x: 10, y: 8 }, null])).toBe("M10.00,8.00");
  });
});
