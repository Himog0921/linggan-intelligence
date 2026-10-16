import type {
  Attention,
  Insight,
  Ranking,
  TrendPoint,
  Window,
  AsOf,
} from "./api";
export const missing = "—";
export function count(value: number | null | undefined) {
  if (value == null)
    return { text: missing, title: "没有值或未知", missing: true };
  const exact = value.toLocaleString("zh-CN");
  return {
    text:
      Math.abs(value) < 10000
        ? exact
        : `${(value / 10000).toFixed(1).replace(/\.0$/, "")}万`,
    title: Math.abs(value) < 10000 ? "" : exact,
    missing: false,
  };
}
export const number = (value: number | null | undefined) =>
  value == null ? missing : value.toLocaleString("zh-CN");
export function percent(value: number | null | undefined, digits = 1) {
  return value == null
    ? missing
    : `${value.toFixed(digits).replace(/\.0+$/, "")}%`;
}
export function safeHref(value: string) {
  try {
    const u = new URL(value);
    return ["http:", "https:"].includes(u.protocol) ? u.href : null;
  } catch {
    return null;
  }
}
export const platformLabel: Record<string, string> = {
  xhs: "小红书",
  douyin: "抖音",
  both: "双平台",
};
export const accountStatus: Record<string, string> = {
  active: "使用中",
  violation_disabled: "违规停用",
  warming: "养号中",
  paused: "暂停",
  archived: "已归档",
};
export const sliceTitle = {
  yesterday: "昨日加微",
  week: "本周加微",
  month: "本月加微",
  custom: "当期加微",
};
export const metricLabel: Record<string, string> = {
  published: "发布",
  links_opened: "链接打开",
  followers_gained: "加微",
};
export const riskLabel: Record<string, string> = {
  high: "待复盘",
  medium: "待确认",
};
export const nextStep = {
  drop: "查看排行并复盘下降原因",
  zero_report: "确认是否漏填（切换前在工作台补）",
  pending_views: "补阅读量（切换前在工作台填）",
  owner_mismatch: "确认账号负责人和发布负责人",
};
export function delta(value: number | null | undefined) {
  return value == null
    ? { text: "环比 —", tone: "" }
    : value === 0
      ? { text: "持平上期", tone: "" }
      : {
          text: `${value > 0 ? "↑" : "↓"}${percent(Math.abs(value))}`,
          tone: value > 0 ? "up" : "down",
        };
}
export function insight(item: Insight) {
  if (item.kind === "pace") {
    if (item.pace === "achieved")
      return {
        tone: "ok",
        text: `目标已达成（完成 ${percent(item.achievedPct)}）`,
      };
    const projected =
      item.projectedPct === null ? missing : `${item.projectedPct}%`;
    if (item.pace === "behind")
      return {
        tone: "risk",
        text: `目标有缺口：按当前节奏预计完成 ${projected}`,
      };
    if (item.projectedPct !== null && item.projectedPct >= 100)
      return { tone: "ok", text: `节奏正常：按当前节奏预计完成 ${projected}` };
    return { tone: "", text: `按当前节奏预计完成 ${projected}` };
  }
  if (item.kind === "drag")
    return {
      tone: "watch",
      text: `${metricLabel[item.metric || ""] || item.metric}下滑是主因（环比 −${percent(item.deltaPct === null ? null : Math.abs(item.deltaPct))}）`,
    };
  return { tone: "risk", text: `${item.names.join("、")}需今天复盘` };
}
export function attention(item: Attention) {
  if (item.kind === "drop")
    return {
      text: `${item.operator} · ${metricLabel[item.metric || ""] || item.metric}比上期下降 ${percent(item.dropPct === null ? null : Math.abs(item.dropPct))}（${number(item.previousValue)} → ${number(item.currentValue)}）`,
      action: nextStep.drop,
    };
  if (item.kind === "zero_report")
    return {
      text: `${item.operator} · ${item.zeroMetrics.map((z) => metricLabel[z.metric] || z.metric).join("、")}本期 ${number(item.count)} 天都记的是 0（上期 ${item.zeroMetrics.map((z) => number(z.previousValue)).join("、")}）`,
      action: nextStep.zero_report,
    };
  if (item.kind === "pending_views")
    return {
      text: `${item.operator} · 待补阅读量 ${number(item.count)} 篇${item.oldestDate ? `，最早 ${item.oldestDate}` : ""}`,
      action: nextStep.pending_views,
    };
  return {
    text: `账号「${item.account}」由 ${item.accountOperator} 负责，发布人是 ${item.operator}（${number(item.count)} 条）`,
    action: nextStep.owner_mismatch,
  };
}
export function rankingStep(row: Ranking) {
  return row.riskKind === "drop"
    ? `复盘${metricLabel[row.riskMetric || ""] || ""}下降`
    : {
        zero_report: "确认是否漏填",
        pending_views: "补阅读量",
        owner_mismatch: "确认负责人",
      }[row.riskKind || ""] || "";
}
export function scale(values: (number | null)[], height: number) {
  const max = Math.max(0, ...values.filter((v): v is number => v !== null));
  return (v: number | null) =>
    v === null || max === 0 ? 0 : (v / max) * height;
}
export const span = (r: { from: string; to: string }) =>
  r.from === r.to ? r.from : `${r.from} ~ ${r.to}`;
export function windowLine(w: Window, asOf: AsOf) {
  const through = asOf
    ? `数据截至 ${asOf.exportedAtBeijing.slice(5)}，按 ${w.completeThrough?.slice(5)} 及以前计算`
    : "还没有迁入数据";
  if (!w.covered)
    return `${span(w.current)} · 这段时间还没有迁入的数据（${through}）`;
  const counted =
    w.covered.to === w.current.to
      ? `${w.current.days} 天`
      : `按已迁入的 ${span(w.covered)}（${w.covered.days} 天）计算`;
  return `${span(w.current)}（${counted}）· 比上期 ${w.previous ? span(w.previous) : missing}`;
}
export const blockEmptyText = (w: Window, text: string) =>
  w.covered === null ? "这段时间还没有迁入的数据" : text;
export function trendPointNote(p: TrendPoint) {
  return p.coverage === "not_migrated"
    ? "还没迁入"
    : p.coverage === "partial"
      ? `按 ${p.coveredTo} 及以前计算`
      : "";
}
export function trendAccessibleLabel(
  points: TrendPoint[],
  granularity: string,
) {
  return [
    `加微与爆款长趋势，${points.length} 个${{ day: "日", week: "周", month: "月" }[granularity]}`,
    ...points
      .filter((p) => p.coverage === "partial")
      .map((p) => `${p.label} · ${trendPointNote(p)}`),
  ].join("；");
}
