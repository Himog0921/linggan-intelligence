import type { Lifecycle } from './api';
export type Point = Lifecycle['points'][number];
export type Bucket = { label: string; points: Point[] };
export function percentile(values: number[], p: number): number | null {
  if (!values.length) return null;
  const sorted = [...values].sort((a, b) => a - b),
    index = Math.max(0, Math.min(1, p)) * (sorted.length - 1),
    low = sorted[Math.floor(index)],
    high = sorted[Math.ceil(index)];
  return low === undefined || high === undefined ? null : low + (high - low) * (index - Math.floor(index));
}
export function bucketPoints(points: Point[], week: boolean): Bucket[] {
  const buckets = new Map<string, Point[]>();
  for (const p of points) {
    let label = p.date.slice(0, 7);
    if (week) {
      const date = new Date(`${p.date}T00:00:00Z`);
      date.setUTCDate(date.getUTCDate() - ((date.getUTCDay() + 6) % 7));
      label = date.toISOString().slice(0, 10);
    }
    label = label.replaceAll('-', '/');
    const items = buckets.get(label) ?? [];
    items.push(p);
    buckets.set(label, items);
  }
  return [...buckets].sort(([a], [b]) => a.localeCompare(b)).map(([label, points]) => ({ label, points }));
}
export const exact = (v: number) =>
  Number(v.toFixed(2)).toLocaleString('en-US', { maximumFractionDigits: 2 });
export const compact = (v: number) => (Math.abs(v) >= 10000 ? `${(v / 10000).toFixed(1)}万` : exact(v));
export const metricLabel = (metric: string) =>
  ({ likes: '点赞', comments: '评论', collects: '收藏', shares: '转发' })[metric] ?? metric;
export function smoothPath(points: [number, number][]) {
  const first = points[0];
  if (!first) return '';
  let path = `M ${first[0].toFixed(1)},${first[1].toFixed(1)}`;
  for (let i = 0; i < points.length - 1; i++) {
    const previous = points[Math.max(0, i - 1)],
      current = points[i],
      next = points[i + 1],
      following = points[i + 2] ?? next;
    if (!previous || !current || !next || !following) continue;
    const c1 = [current[0] + (next[0] - previous[0]) / 6, current[1] + (next[1] - previous[1]) / 6],
      c2 = [next[0] - (following[0] - current[0]) / 6, next[1] - (following[1] - current[1]) / 6];
    path += ` C ${c1[0]?.toFixed(1)},${c1[1]?.toFixed(1)} ${c2[0]?.toFixed(1)},${c2[1]?.toFixed(1)} ${next[0].toFixed(1)},${next[1].toFixed(1)}`;
  }
  return path;
}
