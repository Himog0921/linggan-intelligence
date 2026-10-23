import { expect, it } from "vitest";
import * as api from "./api";
const page = { asOf: null, total: 0, active: 0, operators: [], items: [] };
it("validates payloads before rendering and preserves null coverage", () => {
  expect(api.accounts(page).asOf).toBeNull();
  expect(() => api.accounts({ ...page, active: "0" })).toThrow();
  expect(() =>
    api.accounts({ ...page, items: [{ name: "missing fields" }] }),
  ).toThrow();
  expect(() => api.businesses({ items: [] })).toThrow();
  expect(
    api.businesses([
      { domainRef: "domain", name: "合成业务", accountCount: 0 },
    ])[0]?.accountCount,
  ).toBe(0);
});
it("does not allow missing data to manufacture a success receipt", () => {
  expect(() =>
    api.receipt({
      requestId: "r",
      outcome: "accepted",
      replayed: false,
      current: null,
    }),
  ).toThrow();
  expect(() =>
    api.receipt({
      requestId: "r",
      outcome: "success",
      replayed: false,
      current: null,
    }),
  ).toThrow();
  expect(
    api.receipt({
      requestId: "r",
      outcome: "stale",
      replayed: false,
      current: null,
    }).outcome,
  ).toBe("stale");
});
it("requires explicit native-data provenance on a summary", () => {
  const summary = {
    asOf: null,
    nativeDataPresent: true,
    month: "2026-10",
    activeAccounts: 1,
    ledgerAccounts: 1,
    monthPublished: 0,
    effectivePublication: { state: "not_connected" },
    monthFollowersGained: { value: null, scope: "all_businesses" },
    monthHits: 0,
  };
  expect(api.summary(summary).nativeDataPresent).toBe(true);
  expect(() => api.summary(without(summary, "nativeDataPresent"))).toThrow();
});
it("rejects invalid state enums rather than inferring a platform or execution status", () => {
  expect(() =>
    api.daily({
      asOf: null,
      scope: "all_businesses",
      limit: 200,
      operators: [],
      items: [
        {
          ref: "r",
          businessDate: "2026-10-16",
          weekday: "五",
          operator: "甲",
          published: 0,
          linksOpened: 0,
          followersGained: 0,
          addRate: null,
          hits: 0,
          monthFollowersGained: 0,
          monthPublished: 0,
          execution: "failed",
        },
      ],
    }),
  ).toThrow();
});

function without(value: Record<string, unknown>, key: string) {
  return Object.fromEntries(
    Object.entries(value).filter(([name]) => name !== key),
  );
}
it("requires asOf to be present; explicit null alone means never imported", () => {
  expect(api.accounts(page).asOf).toBeNull();
  expect(() => api.accounts(without(page, "asOf"))).toThrow();
  expect(() =>
    api.publications({
      page: 1,
      pageSize: 50,
      total: 0,
      totalPages: 0,
      operators: [],
      items: [],
    }),
  ).toThrow();
  expect(() =>
    api.daily({
      scope: "all_businesses",
      limit: 200,
      operators: [],
      items: [],
    }),
  ).toThrow();
});
it("rejects null where the contract requires a string array", () => {
  expect(() => api.accounts({ ...page, operators: null })).toThrow();
  expect(() => api.accounts({ ...page, operators: [null] })).toThrow();
});
it("requires explicit null for an absent current target", () => {
  const history = {
    periodType: "week",
    periodKey: "2026-W42",
    current: null,
    revisions: [],
  };
  expect(api.history(history).current).toBeNull();
  expect(() => api.history(without(history, "current"))).toThrow();
  expect(() =>
    api.receipt({ requestId: "r", outcome: "stale", replayed: false }),
  ).toThrow();
});
it("requires nullable account fields without replacing missing values with unknown", () => {
  const account = {
    ref: "r",
    source: "imported",
    platform: "xhs",
    name: "合成账号",
    profileUrl: null,
    deviceCode: null,
    followerCount: null,
    ipLocation: null,
    operator: null,
    status: null,
    health: { state: "not_filled" },
    isRetained: null,
    notes: null,
  };
  expect(
    api.accounts({ ...page, items: [account] }).items[0]?.followerCount,
  ).toBeNull();
  for (const key of ["profileUrl", "followerCount", "isRetained"]) {
    expect(() =>
      api.accounts({ ...page, items: [without(account, key)] }),
    ).toThrow();
  }
});
