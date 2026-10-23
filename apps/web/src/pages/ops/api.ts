import { ApiError, requestJson } from "../../shared/api";
import * as d from "../../shared/decode";
type R = d.Json;
const s = d.str,
  n = d.num,
  b = d.bool;
// Rust serializes these keys on every response. Null has a contract meaning; a missing key
// is a malformed answer, never evidence that the value is unknown or no import exists.
const sn = (r: R, key: string) => (d.field(r, key) === null ? null : s(r, key));
const nn = (r: R, key: string) => (d.field(r, key) === null ? null : n(r, key));
const bn = (r: R, key: string) => (d.field(r, key) === null ? null : b(r, key));
function enumeration<const T extends readonly string[]>(
  r: R,
  key: string,
  choices: T,
): T[number] {
  const value = s(r, key);
  if (!choices.includes(value)) throw new d.ShapeError(key);
  // Every member is checked above; the type narrows only this closed enum.
  return value as T[number];
}
const platform = (r: R) => enumeration(r, "platform", ["xhs", "douyin"]);
const asOf = (r: R) => ({
  exportedAt: s(r, "exportedAt"),
  exportedAtBeijing: s(r, "exportedAtBeijing"),
  batchRef: s(r, "batchRef"),
});
function optional<T>(r: R, key: string, parse: (value: R) => T): T | null {
  return d.field(r, key) === null ? null : parse(d.obj(r, key));
}
const base = (r: R) => ({ asOf: optional(r, "asOf", asOf) });
const names = (r: R, key: string) => {
  if (!Array.isArray(d.field(r, key))) throw new d.ShapeError(key);
  return d.strList(r, key);
};
export function businesses(value: unknown) {
  if (!Array.isArray(value))
    throw new ApiError({ kind: "invalid_response", status: 200 });
  return value.map((item) =>
    d.decode(item, (r) => ({
      domainRef: s(r, "domainRef"),
      name: s(r, "name"),
      accountCount: n(r, "accountCount"),
    })),
  );
}
export function summary(value: unknown) {
  return d.decode(value, (r) => ({
    ...base(r),
    nativeDataPresent: b(r, "nativeDataPresent"),
    month: s(r, "month"),
    activeAccounts: nn(r, "activeAccounts"),
    ledgerAccounts: nn(r, "ledgerAccounts"),
    monthPublished: nn(r, "monthPublished"),
    effectivePublication: enumeration(
      d.obj(r, "effectivePublication"),
      "state",
      ["not_connected"],
    ),
    monthFollowersGained: {
      value: nn(d.obj(r, "monthFollowersGained"), "value"),
      scope: enumeration(d.obj(r, "monthFollowersGained"), "scope", [
        "all_businesses",
      ]),
    },
    monthHits: nn(r, "monthHits"),
  }));
}
function publication(r: R) {
  const views = d.obj(r, "views");
  const state = enumeration(views, "state", ["value", "pending", "optional"]);
  const value = nn(views, "value");
  if ((state === "value") !== (value !== null))
    throw new d.ShapeError("views.value");
  return {
    ref: s(r, "ref"),
    source: enumeration(r, "source", ["imported", "native"]),
    businessDate: s(r, "businessDate"),
    dateState: enumeration(r, "dateState", ["as_recorded", "suspect_year"]),
    platform: platform(r),
    title: sn(r, "title"),
    body: sn(r, "body"),
    url: s(r, "url"),
    account: optional(r, "account", (a) => ({
      name: s(a, "name"),
      ledgerState: enumeration(a, "ledgerState", [
        "in_ledger",
        "absent_from_export",
        "native_active",
      ]),
    })),
    publisher: sn(r, "publisher"),
    views: { state, value, overdue: b(views, "overdue"), hit: b(views, "hit") },
    likes: nn(r, "likes"),
    collects: nn(r, "collects"),
    comments: nn(r, "comments"),
    coverClickRate: nn(r, "coverClickRate"),
    systemCheck: enumeration(r, "systemCheck", ["not_checked"]),
    duplicates: d.list(r, "duplicates", (item) => ({
      businessDate: s(item, "businessDate"),
    })),
    cover: { available: b(d.obj(r, "cover"), "available") },
  };
}
export function publications(value: unknown) {
  return d.decode(value, (r) => ({
    ...base(r),
    page: n(r, "page"),
    pageSize: n(r, "pageSize"),
    total: n(r, "total"),
    totalPages: n(r, "totalPages"),
    operators: names(r, "operators"),
    items: d.list(r, "items", publication),
  }));
}
function account(r: R) {
  return {
    ref: s(r, "ref"),
    source: enumeration(r, "source", ["imported", "native"]),
    platform: platform(r),
    name: s(r, "name"),
    profileUrl: sn(r, "profileUrl"),
    deviceCode: sn(r, "deviceCode"),
    followerCount: nn(r, "followerCount"),
    ipLocation: sn(r, "ipLocation"),
    operator: sn(r, "operator"),
    status: sn(r, "status"),
    health: enumeration(d.obj(r, "health"), "state", ["not_filled"]),
    isRetained: bn(r, "isRetained"),
    notes: sn(r, "notes"),
  };
}
export function accounts(value: unknown) {
  return d.decode(value, (r) => ({
    ...base(r),
    total: n(r, "total"),
    active: n(r, "active"),
    operators: names(r, "operators"),
    items: d.list(r, "items", account),
  }));
}
function dailyItem(r: R) {
  return {
    ref: s(r, "ref"),
    businessDate: s(r, "businessDate"),
    weekday: s(r, "weekday"),
    operator: s(r, "operator"),
    published: n(r, "published"),
    linksOpened: nn(r, "linksOpened"),
    followersGained: nn(r, "followersGained"),
    addRate: nn(r, "addRate"),
    hits: n(r, "hits"),
    monthFollowersGained: nn(r, "monthFollowersGained"),
    monthPublished: n(r, "monthPublished"),
    execution: enumeration(r, "execution", [
      "not_published",
      "not_filled",
      "praise",
      "pending_views",
      "done",
    ]),
  };
}
export function daily(value: unknown) {
  return d.decode(value, (r) => ({
    ...base(r),
    scope: enumeration(r, "scope", ["all_businesses"]),
    limit: n(r, "limit"),
    operators: names(r, "operators"),
    items: d.list(r, "items", dailyItem),
  }));
}
const range = (r: R) => ({
  from: s(r, "from"),
  to: s(r, "to"),
  days: n(r, "days"),
});
const period = (r: R) => enumeration(r, "periodType", ["week", "month"]);
function revision(r: R) {
  return {
    revisionRef: s(r, "revisionRef"),
    periodType: period(r),
    periodKey: s(r, "periodKey"),
    targetValue: n(r, "targetValue"),
    setAtBeijing: s(r, "setAtBeijing"),
  };
}
export function history(value: unknown) {
  return d.decode(value, (r) => ({
    periodType: period(r),
    periodKey: s(r, "periodKey"),
    current: optional(r, "current", revision),
    revisions: d.list(r, "revisions", revision),
  }));
}
export function receipt(value: unknown) {
  return d.decode(value, (r) => {
    const outcome = enumeration(r, "outcome", [
      "accepted",
      "unchanged",
      "stale",
    ]);
    const current = optional(r, "current", revision);
    if (outcome !== "stale" && current === null)
      throw new d.ShapeError("current");
    return {
      requestId: s(r, "requestId"),
      outcome,
      replayed: b(r, "replayed"),
      current,
    };
  });
}

/** A native-write receipt is the durable result of one request identity, not a toast outcome. */
export function nativeReceipt(value: unknown, requestId: string) {
  return d.decode(value, (r) => {
    const receipt = d.obj(r, "receipt");
    const requestRef = s(receipt, "requestRef");
    if (requestRef !== requestId) throw new d.ShapeError("receipt.requestRef");
    return {
      requestRef,
      outcome: enumeration(receipt, "outcome", ["accepted", "unchanged", "rejected", "stale"]),
      code: s(receipt, "code"),
      recordRef: sn(receipt, "recordRef"),
      resultingVersion: nn(receipt, "resultingVersion"),
      replayed: b(receipt, "replayed"),
    };
  });
}

export type NativeAccountCommand = {
  domainRef: string; platform: "xhs" | "douyin"; accountName: string;
  profileUrl: string | null; platformUserId: string | null; handle: string | null;
  status: string | null; isRetained: boolean | null; followerCount: number | null;
  deviceCode: string | null; ipLocation: string | null; notes: string | null;
};

/** The caller owns the UUID so an unanswered request can be retried without duplicating facts. */
export async function createNativeAccount(command: NativeAccountCommand & { requestId: string }) {
  return nativeReceipt(await requestJson("/api/local/ops/native/accounts", {
    method: "POST", body: command,
  }), command.requestId);
}

export type NativePublicationCommand = {
  accountRef: string; publicationUrl: string; canonicalUrl: string | null;
  publishedOn: string | null; title: string | null; body: string | null;
};
export async function createNativePublication(command: NativePublicationCommand & { requestId: string }) {
  return nativeReceipt(await requestJson("/api/local/ops/native/publications", {
    method: "POST", body: { ...command, publishedAt: null },
  }), command.requestId);
}
const figures = (r: R) => ({
  published: nn(r, "published"),
  linksOpened: nn(r, "linksOpened"),
  followersGained: nn(r, "followersGained"),
  conversionRate: nn(r, "conversionRate"),
  hits: nn(r, "hits"),
});
function windowDTO(r: R) {
  return {
    slice: enumeration(r, "slice", ["yesterday", "week", "month", "custom"]),
    today: s(r, "today"),
    thisWeek: s(r, "thisWeek"),
    thisMonth: s(r, "thisMonth"),
    completeThrough: sn(r, "completeThrough"),
    current: range(d.obj(r, "current")),
    covered: optional(r, "covered", range),
    previous: optional(r, "previous", range),
  };
}
function targetDTO(r: R) {
  const state = enumeration(r, "state", ["set", "not_set", "not_applicable"]);
  const rev = optional(r, "revision", revision);
  if (state === "set" && rev === null) throw new d.ShapeError("revision");
  return {
    state,
    periodType: sn(r, "periodType"),
    periodKey: sn(r, "periodKey"),
    revision: rev,
    elapsedDays: nn(r, "elapsedDays"),
    periodFollowers: nn(r, "periodFollowers"),
    achievedPct: nn(r, "achievedPct"),
    timeProgressPct: nn(r, "timeProgressPct"),
    dailyNeeded: nn(r, "dailyNeeded"),
    projectedEnd: nn(r, "projectedEnd"),
    pace: sn(r, "pace"),
  };
}
function insight(r: R) {
  return {
    kind: enumeration(r, "kind", ["pace", "drag", "review"]),
    pace: sn(r, "pace"),
    achievedPct: nn(r, "achievedPct"),
    projectedPct: nn(r, "projectedPct"),
    metric: sn(r, "metric"),
    deltaPct: nn(r, "deltaPct"),
    names: names(r, "names"),
  };
}
function attentionItem(r: R) {
  return {
    kind: enumeration(r, "kind", [
      "drop",
      "zero_report",
      "pending_views",
      "owner_mismatch",
    ]),
    severity: enumeration(r, "severity", ["high", "medium"]),
    operator: sn(r, "operator"),
    account: sn(r, "account"),
    metric: sn(r, "metric"),
    currentValue: nn(r, "currentValue"),
    previousValue: nn(r, "previousValue"),
    dropPct: nn(r, "dropPct"),
    count: nn(r, "count"),
    oldestDate: sn(r, "oldestDate"),
    accountOperator: sn(r, "accountOperator"),
    zeroMetrics: d.list(r, "zeroMetrics", (z) => ({
      metric: s(z, "metric"),
      previousValue: n(z, "previousValue"),
    })),
  };
}
function ranking(r: R) {
  const spark = d.field(r, "sparkline");
  if (
    !Array.isArray(spark) ||
    !spark.every(
      (x) => x === null || (typeof x === "number" && Number.isFinite(x)),
    )
  )
    throw new d.ShapeError("sparkline");
  return {
    operator: s(r, "operator"),
    platform: sn(r, "platform"),
    sparkline: spark.map((x) => (x === null ? null : Number(x))),
    responsibleAccounts: n(r, "responsibleAccounts"),
    published: n(r, "published"),
    linksOpened: nn(r, "linksOpened"),
    followersGained: nn(r, "followersGained"),
    conversionRate: nn(r, "conversionRate"),
    hits: n(r, "hits"),
    pendingViews: n(r, "pendingViews"),
    risk: sn(r, "risk"),
    riskKind: sn(r, "riskKind"),
    riskMetric: sn(r, "riskMetric"),
  };
}
export function overview(value: unknown) {
  return d.decode(value, (r) => {
    const k = d.obj(r, "kpi"),
      delta = d.obj(k, "delta"),
      f = d.obj(r, "funnel"),
      a = d.obj(r, "attention"),
      out = d.obj(r, "dailyOutput"),
      m = d.obj(r, "matrix");
    return {
      ...base(r),
      businessSelected: b(r, "businessSelected"),
      window: windowDTO(d.obj(r, "window")),
      kpi: {
        current: figures(d.obj(k, "current")),
        previous: figures(d.obj(k, "previous")),
        delta: {
          publishedPct: nn(delta, "publishedPct"),
          linksOpenedPct: nn(delta, "linksOpenedPct"),
          followersGainedPct: nn(delta, "followersGainedPct"),
          hitsPct: nn(delta, "hitsPct"),
        },
      },
      funnel: {
        publishToOpenAvailable: b(f, "publishToOpenAvailable"),
        current: {
          publishToOpen: nn(d.obj(f, "current"), "publishToOpen"),
          openToFollow: nn(d.obj(f, "current"), "openToFollow"),
        },
      },
      target: targetDTO(d.obj(r, "target")),
      insights: d.list(r, "insights", insight),
      attention: {
        items: d.list(a, "items", attentionItem),
        rest: d.list(a, "rest", attentionItem),
        hiddenCount: n(a, "hiddenCount"),
        totalCount: n(a, "totalCount"),
      },
      ranking: d.list(r, "ranking", ranking),
      rankingCovered: b(r, "rankingCovered"),
      dailyOutput: {
        days: names(out, "days"),
        coveredTo: sn(out, "coveredTo"),
        operators: names(out, "operators"),
        published: d.list(out, "published", (x) => ({
          date: s(x, "date"),
          operator: s(x, "operator"),
          xhs: n(x, "xhs"),
          douyin: n(x, "douyin"),
        })),
        followers: d.list(out, "followers", (x) => ({
          date: s(x, "date"),
          operator: s(x, "operator"),
          value: n(x, "value"),
        })),
      },
      platformShare: optional(r, "platformShare", (x) => ({
        xhs: n(x, "xhs"),
        douyin: n(x, "douyin"),
      })),
      matrix: {
        covered: b(m, "covered"),
        operators: names(m, "operators"),
        accounts: d.list(m, "accounts", (x) => ({
          ref: s(x, "ref"),
          name: s(x, "name"),
          platform: platform(x),
          retired: b(x, "retired"),
        })),
        cells: d.list(m, "cells", (x) => ({
          account: s(x, "account"),
          operator: s(x, "operator"),
          published: n(x, "published"),
          hits: n(x, "hits"),
        })),
      },
    };
  });
}
export function trend(value: unknown) {
  return d.decode(value, (r) => ({
    ...base(r),
    granularity: enumeration(r, "granularity", ["day", "week", "month"]),
    points: d.list(r, "points", (p) => ({
      start: s(p, "start"),
      label: s(p, "label"),
      followersGained: nn(p, "followersGained"),
      hits: nn(p, "hits"),
      coverage: enumeration(p, "coverage", ["full", "partial", "not_migrated"]),
      coveredTo: sn(p, "coveredTo"),
    })),
  }));
}
export type Summary = ReturnType<typeof summary>;
export type AsOf = Summary["asOf"];
export type Publication = ReturnType<typeof publications>["items"][number];
export type Account = ReturnType<typeof accounts>["items"][number];
export type Daily = ReturnType<typeof daily>["items"][number];
export type Overview = ReturnType<typeof overview>;
export type Window = Overview["window"];
export type Attention = ReturnType<typeof attentionItem>;
export type Insight = ReturnType<typeof insight>;
export type Ranking = ReturnType<typeof ranking>;
export type TrendPoint = ReturnType<typeof trend>["points"][number];
export type PeriodType = "week" | "month";
export type Revision = ReturnType<typeof revision>;
export function url(path: string, params: Record<string, string> = {}) {
  const query = new URLSearchParams(
    Object.entries(params).filter(([, v]) => v !== ""),
  );
  return `/api/local/ops/${path}?${query}`;
}
export async function get<T>(
  path: string,
  parse: (value: unknown) => T,
  signal?: AbortSignal,
) {
  return parse(await requestJson(path, { ...(signal ? { signal } : {}) }));
}
export function reason(error: unknown) {
  if (error instanceof ApiError) {
    const f = error.failure;
    if (f.kind === "network") return "连不上本机服务";
    if (f.kind === "invalid_response") return "返回的内容无法读取";
    const reasons: Record<string, string> = {
      ops_read_unavailable: "数据库暂时读不到",
      ops_business_not_found: "这个业务不存在",
      ops_parameter_invalid: "筛选条件无效",
      read_model_not_connected: "本机服务没有连上数据库",
      ops_target_request_conflict: "这次保存的内容和之前的不一致",
    };
    return reasons[f.code || ""] || `本机服务返回 ${f.status}`;
  }
  return "返回的内容无法读取";
}
