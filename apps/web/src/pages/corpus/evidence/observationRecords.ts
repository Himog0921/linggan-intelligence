/**
 * 「采集记录」标签的给人看的读法（Mog 2026-10-23）：这篇被观察过几次、每次的数字和上一次比变了多少、
 * 最近一次是多久以前，以及评论采过几次、采到多少。全部是纯函数，只重排详情接口已给的事实，不补造。
 */
import type { CoverageEntry, Metric, TimelinePoint } from './evidenceTypes';

export const OBSERVED_METRICS = [
  ['like', '点赞'],
  ['comment', '评论'],
  ['collect', '收藏'],
  ['share', '分享'],
] as const;

export type ObservedMetric = (typeof OBSERVED_METRICS)[number][0];

/**
 * 详情接口只给最近 100 次互动观察（`material_detail_read.rs` 的展示窗口）。正好满 100 次时，
 * 更早的观察不在这里，页面要说出来，不把窗口当全部。
 */
export const TIMELINE_WINDOW = 100;

export interface MetricCell {
  /** 这次读到的数字；没读到是 null（显示「—」，不是 0）。 */
  value: number | null;
  /** 与这一项上一次读到的数字之差；任一边没读到就是 null。 */
  delta: number | null;
}

export interface ObservationRow {
  /** 这一行最早、最晚的观察时间（连续几次读数完全相同时合成一行）。 */
  first: string | null;
  last: string | null;
  count: number;
  /** `discovery`：在搜索结果或作者主页的列表里看到；`detail`：打开了详情页。 */
  lane: string | null;
  cells: Record<ObservedMetric, MetricCell>;
}

function knownValue(metric: Metric): number | null {
  return metric.state === 'KNOWN' ? metric.value : null;
}

function sameReading(row: ObservationRow, lane: string | null, cells: Record<ObservedMetric, MetricCell>): boolean {
  return row.lane === lane && OBSERVED_METRICS.every(([key]) => row.cells[key].value === cells[key].value);
}

/**
 * 从早到晚走一遍：每一项与它上一次读到的值比较；同一来源连续读到完全相同的数字合成一行（保留首末时间与次数）。
 * 返回从新到旧，最上面是最近一次。
 */
export function observationRows(points: readonly TimelinePoint[]): ObservationRow[] {
  const previous: Record<ObservedMetric, number | null> = { like: null, comment: null, collect: null, share: null };
  const rows: ObservationRow[] = [];
  for (const point of points) {
    const cells = {} as Record<ObservedMetric, MetricCell>;
    for (const [key] of OBSERVED_METRICS) {
      const value = knownValue(point[key]);
      const before = previous[key];
      cells[key] = { value, delta: value !== null && before !== null ? value - before : null };
      if (value !== null) previous[key] = value;
    }
    const current = rows[rows.length - 1];
    if (current && sameReading(current, point.sourceLane, cells)) {
      current.last = point.observedAt;
      current.count += 1;
    } else {
      rows.push({ first: point.observedAt, last: point.observedAt, count: 1, lane: point.sourceLane, cells });
    }
  }
  return rows.reverse();
}

const LANE_COPY: Readonly<Record<string, string>> = {
  discovery: '列表中看到',
  detail: '打开详情页',
};

export function observationLaneCopy(lane: string | null): string {
  return (lane ? LANE_COPY[lane] : undefined) ?? '其他来源';
}

/** 变化写法：+12 / −3；没变写「持平」；算不出来是 null（不显示）。 */
export function deltaCopy(delta: number | null): string | null {
  if (delta === null) return null;
  if (delta === 0) return '持平';
  return `${delta > 0 ? '+' : '−'}${Math.abs(delta).toLocaleString('zh-CN')}`;
}

const DAY_MS = 86_400_000;

/**
 * 两种时间写法都认：列表层是北京时间、不带时区（`2026-10-15 05:59`），详情时间线是带时区的 ISO；
 * 数据库文本 `…+08` 补成 `+08:00`。小数秒只留三位（浏览器只保证认三位）。认不出来是 NaN。
 */
function momentMillis(value: string): number {
  const match = /^(\d{4}-\d{2}-\d{2})[ T](\d{2}:\d{2})(?::(\d{2})(?:\.(\d+))?)?(Z|[+-]\d{2}(?::?\d{2})?)?$/.exec(value.trim());
  if (!match) return Number.NaN;
  const [, day, minute, second = '00', fraction = '', zone = '+08:00'] = match;
  const millis = fraction ? `.${fraction.slice(0, 3).padEnd(3, '0')}` : '';
  const offset = zone === 'Z' ? 'Z' : `${zone.slice(0, 3)}:${zone.replace(':', '').slice(3, 5).padEnd(2, '0')}`;
  return Date.parse(`${day}T${minute}:${second}${millis}${offset}`);
}

/** 距今几天：今天／N 天前；时间读不出来或在将来是 null。 */
export function elapsedCopy(value: string | null | undefined, now: number): string | null {
  if (!value) return null;
  const at = momentMillis(value);
  if (Number.isNaN(at) || at > now) return null;
  const days = Math.floor((now - at) / DAY_MS);
  return days === 0 ? '今天' : `${days} 天前`;
}

// ───────────────────────── 评论采集 ─────────────────────────

const SCOPE_COPY: Readonly<Record<string, string>> = {
  detail_window: '随详情顺带采（最多 30 条）',
  all_public_comments: '尽量采全',
  incremental_new: '分段续采',
};

export interface CommentCollectionRow {
  at: string | null;
  scope: string;
  /** 这次采到的不重复评论（含回复）；没报是 null。 */
  collected: number | null;
  /** 采的时候页面上显示的评论数。 */
  shown: number | null;
  /** 只对「尽量采全」有意义：已采全／没采全；其余范围本来就有上限，不做这个判断。 */
  complete: boolean | null;
}

export function commentCollectionRows(history: readonly CoverageEntry[]): CommentCollectionRow[] {
  return [...history]
    .sort((left, right) => (right.observedAt ?? '').localeCompare(left.observedAt ?? ''))
    .map(entry => ({
      at: entry.observedAt,
      scope: (entry.collectionScope ? SCOPE_COPY[entry.collectionScope] : undefined) ?? '评论采集',
      collected: entry.uniqueCollectedCount ?? entry.retained ?? entry.producerAcquired,
      shown: entry.pageCommentCount,
      complete: entry.collectionScope === 'all_public_comments' && entry.collectionState !== null ? entry.collectionState === 'complete' : null,
    }));
}
