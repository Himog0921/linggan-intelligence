import { describe, expect, it } from "vitest";
import {
  count,
  percent,
  delta,
  safeHref,
  blockEmptyText,
  trendPointNote,
  windowLine,
  scale,
} from "./format";
import type { Window } from "./api";
describe("OPS-HTTP-001 display contracts", () => {
  it("keeps missing, zero, exact and abbreviated numbers distinct", () => {
    expect(count(null)).toEqual({
      text: "—",
      title: "没有值或未知",
      missing: true,
    });
    expect(count(undefined).text).toBe("—");
    expect(count(0).text).toBe("0");
    expect(count(9439).text).toBe("9,439");
    expect(count(10000)).toEqual({
      text: "1万",
      title: "10,000",
      missing: false,
    });
    expect(count(103583).text).toBe("10.4万");
    expect(percent(null)).toBe("—");
    expect(percent(0)).toBe("0%");
    expect(percent(33.333)).toBe("33.3%");
  });
  it("only makes http(s) sources clickable", () => {
    expect(safeHref("javascript:alert(1)")).toBeNull();
    expect(safeHref("//external.test")).toBeNull();
    expect(safeHref("https://example.test/")).toBe("https://example.test/");
  });
  it("does not infer a comparison for unknown or a bar for unknown/zero", () => {
    expect(delta(null).text).toBe("环比 —");
    expect(delta(0).text).toBe("持平上期");
    expect(delta(-12.5)).toEqual({ text: "↓12.5%", tone: "down" });
    const height = scale([null, 0, 10], 120);
    expect(height(null)).toBe(0);
    expect(height(0)).toBe(0);
    expect(height(10)).toBe(120);
  });
  const window: Window = {
    slice: "week",
    today: "2026-10-16",
    thisWeek: "2026-W42",
    thisMonth: "2026-10",
    completeThrough: "2026-10-11",
    current: { from: "2026-10-12", to: "2026-10-16", days: 5 },
    covered: null,
    previous: null,
  };
  it("does not turn unimported windows into known empty results", () => {
    expect(blockEmptyText(window, "当期无发布")).toBe(
      "这段时间还没有迁入的数据",
    );
    expect(windowLine(window, null)).toContain("还没有迁入数据");
    expect(
      blockEmptyText(
        {
          ...window,
          covered: { from: "2026-10-12", to: "2026-10-12", days: 1 },
        },
        "当期无发布",
      ),
    ).toBe("当期无发布");
  });
  it("retains partial trend cutoffs", () => {
    expect(
      trendPointNote({
        start: "2026-10-01",
        label: "10月",
        followersGained: 48,
        hits: 2,
        coverage: "partial",
        coveredTo: "2026-10-11",
      }),
    ).toBe("按 2026-10-11 及以前计算");
  });
});
