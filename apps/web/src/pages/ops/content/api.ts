import { useQuery } from '@tanstack/react-query';
import { requestJson } from '../../../shared/api';
import * as d from '../../../shared/decode';

const base = '/api/local/ops/content-lens/';
export const dims = ['offer', 'moment', 'situation'] as const;
export type Dimension = typeof dims[number];
export const labels = { offer: '给什么', moment: '时刻', situation: '处境', audience: '面向谁' };
export type EffectDimension = 'offer' | 'moment';
export type SampleMetric = 'publications' | 'titledPublications' | 'titledViews' | 'views' | 'overThousand' | 'hit' | 'activeComments' | 'commentsPerThousandViews' | 'withinAccountViews';
export type Drill = { label: string; metric: SampleMetric; subset?: 'all' | 'numerator' | 'denominator' } & Partial<Record<Dimension, string>>;
const maybe = <T,>(r: d.Json, key: string, parse: (r: d.Json) => T): T | null =>
  d.field(r, key) === null ? null : parse(d.obj(r, key));
const numberOrNull = (r: d.Json, k: string) => d.field(r, k) === null ? null : d.num(r, k);
const textOrNull = (r: d.Json, k: string) => d.field(r, k) === null ? null : d.str(r, k);

function phase(r: d.Json) {
  return { state: d.str(r, 'state'), total: d.num(r, 'totalShards'), succeeded: d.num(r, 'succeededShards') };
}
function status(value: unknown) {
  return d.decode(value, r => ({ enabled: d.bool(r, 'enabled'), run: maybe(r, 'run', v => ({
    inProgress: d.bool(v, 'inProgress'), progress: d.num(v, 'progress'), primary: phase(d.obj(v, 'primary')), audit: maybe(v, 'audit', phase),
  })) }));
}
export function useLensStatus(enabled = true, watch = false) {
  return useQuery({ queryKey: ['ops', 'content-lens', 'status'], enabled, retry: false,
    queryFn: async ({ signal }) => status(await requestJson(base + 'status', { signal })),
    refetchInterval: query => watch && query.state.data?.enabled ? 5000 : false,
  });
}
function group(r: d.Json) {
  return { key: d.str(r, 'key'), name: d.str(r, 'name'), publications: d.num(r, 'publications'), share: numberOrNull(r, 'share') };
}
function rate(r: d.Json) {
  return { numerator: d.num(r, 'numerator'), denominator: d.num(r, 'denominator'), low: numberOrNull(r, 'low'),
    high: numberOrNull(r, 'high'), value: numberOrNull(r, 'value'), reason: textOrNull(r, 'reason') };
}
export type Rate = ReturnType<typeof rate>;
function subsetValue(r: d.Json) { return { value: numberOrNull(r, 'value'), publications: d.num(r, 'publications') }; }
function effect(r: d.Json) {
  const available = d.bool(r, 'available');
  return { available, rows: available ? d.list(r, 'rows', row => ({
    key: d.str(row, 'key'), medianViews: numberOrNull(row, 'medianViews'), highCount: d.num(row, 'highCount'),
    midRate: rate(d.obj(row, 'midRate')), activeCommentRate: rate(d.obj(row, 'activeCommentRate')),
    commentsPerThousandViews: subsetValue(d.obj(row, 'commentsPerThousandViews')),
    withinAccountResidual: subsetValue(d.obj(row, 'withinAccountResidual')),
  })) : [] };
}
function snapshot(r: d.Json) {
  if (d.str(r, 'schemaVersion') !== 'ops-content-lens.snapshot.v1') throw new d.ShapeError('schemaVersion');
  const q = d.obj(r, 'quality'), s = d.obj(r, 'structure'), c = d.obj(r, 'coverage'), e = d.obj(r, 'effect'), by = d.obj(e, 'byDimension');
  if (d.str(e, 'grade') !== 'hypothesis') throw new d.ShapeError('effect.grade');
  return { ref: d.str(r, 'snapshotRef'), quality: {
    latest: textOrNull(q, 'latestPublishedDate'), stale: d.optBool(q, 'isStale'), days: numberOrNull(q, 'staleDays'),
    publications: d.num(q, 'publications'), pending: d.num(q, 'pendingTitles'), withViews: d.num(q, 'withViews'), titled: d.num(q, 'titled'), share: numberOrNull(q, 'withViewsShare'),
    taxonomy: d.num(q, 'taxonomyVersion'), metric: d.num(q, 'metricVersion'), adjudicated: d.num(q, 'adjudicatedTitles'), corrected: d.optNum(q, 'correctedTitles'),
    agreement: d.list(q, 'agreement', a => ({ run: d.str(a, 'runRef'), dimension: d.str(a, 'dimension'), meetsThreshold: d.bool(a, 'meetsThreshold'), value: numberOrNull(a, 'agreement'), kappa: numberOrNull(a, 'kappa'), count: d.num(a, 'sampleCount') })),
  }, structure: { offer: d.list(s, 'offer', group), moment: d.list(s, 'moment', group), situation: d.list(s, 'situation', group) },
  coverage: { cells: d.list(c, 'cells', cell => ({ row: d.str(cell, 'row'), column: d.str(cell, 'column'), count: d.num(cell, 'publications'), state: d.str(cell, 'state') })),
    top10Share: numberOrNull(c, 'top10Share'), empty: d.num(c, 'emptyCells'), sparse: d.num(c, 'sparseCells') },
  effect: { offer: effect(d.obj(by, 'offer')), moment: effect(d.obj(by, 'moment')),
    variance: d.list(e, 'varianceShare', v => ({ factor: d.str(v, 'factor'), scope: d.str(v, 'scope'), observed: numberOrNull(v, 'observed'),
      excess: numberOrNull(v, 'excess'), nullMean: numberOrNull(v, 'nullMean'), p: numberOrNull(v, 'pValue'), observations: d.num(v, 'observations') })),
  }};
}
export type Snapshot = ReturnType<typeof snapshot>;
export async function getSnapshot(domain: string, signal: AbortSignal) {
  const query = domain === 'all' ? '' : '?' + new URLSearchParams({ domainRef: domain });
  return d.decode(await requestJson(base + 'snapshot' + query, { signal }), r => {
    const body = maybe(r, 'snapshot', snapshot);
    if (!body) return null;
    const freshness = d.obj(r, 'freshness');
    return { ...body, quality: { ...body.quality, days: numberOrNull(freshness, 'staleDays'), stale: d.optBool(freshness, 'isStale') } };
  });
}
export async function getPublications(ref: string, drill: Drill, offset: number, signal: AbortSignal) {
  const query = new URLSearchParams({ snapshotRef: ref, offset: String(offset), limit: '25', metric: drill.metric, subset: drill.subset ?? 'all' });
  for (const dim of dims) if (drill[dim]) query.set(dim, drill[dim]);
  return d.decode(await requestJson(base + 'publications?' + query, { signal }), r => {
    if (d.str(r, 'snapshotRef') !== ref || d.num(r, 'offset') !== offset) throw new d.ShapeError('page');
    return { total: d.num(r, 'total'), limit: d.num(r, 'limit'), items: d.list(r, 'items', p => ({
      ref: d.str(p, 'publicationRef'), title: textOrNull(p, 'title'), account: textOrNull(p, 'accountName'),
      date: textOrNull(p, 'businessDate'), views: numberOrNull(p, 'views'), comments: numberOrNull(p, 'comments'),
    })) };
  });
}
export const reviewDimensions = ['moment', 'situation', 'offer', 'audience'] as const;
export type ReviewDimension = typeof reviewDimensions[number];

function byDimension<T>(read: (dimension: ReviewDimension) => T): Record<ReviewDimension, T> {
  return { moment: read('moment'), situation: read('situation'), offer: read('offer'), audience: read('audience') };
}
export async function getSpotCheck(ref: string, signal: AbortSignal) {
  return d.decode(await requestJson(base + 'spot-check?' + new URLSearchParams({ snapshotRef: ref }), { signal }), r => {
    if (d.str(r, 'snapshotRef') !== ref) throw new d.ShapeError('snapshotRef');
    const vocabulary = d.obj(r, 'vocabulary');
    return {
      vocabulary: byDimension(dim => d.list(vocabulary, dim, item => ({ key: d.str(item, 'key'), name: d.str(item, 'name') }))),
      items: d.list(r, 'items', item => {
        const effective = d.obj(item, 'effective');
        return {
          key: d.str(item, 'titleKey'), title: textOrNull(item, 'title'), confidence: d.str(item, 'confidence'),
          machine: byDimension(dim => d.str(item, dim)), effective: byDimension(dim => d.str(effective, dim)),
        };
      }),
    };
  });
}
export type ReviewCommand = { snapshotRef: string; titleKey: string; dimension: ReviewDimension; value: string; reason?: string };
export async function adjudicate(command: ReviewCommand, commandRef: string) {
  return d.decode(await requestJson(base + 'adjudications', { method: 'POST', body: { ...command, commandRef } }), r =>
    ({ snapshotRef: d.str(r, 'snapshotRef'), reused: d.bool(r, 'reused') }));
}
export async function submitRun(commandRef: string) {
  return d.decode(await requestJson(base + 'runs', { method: 'POST', body: { commandRef } }), r => textOrNull(r, 'runRef'));
}
export async function generate(domain: string) {
  return d.decode(await requestJson(base + 'snapshots', { method: 'POST', body: { domainRef: domain === 'all' ? null : domain } }), r => d.str(r, 'snapshotRef'));
}
export const percent = (value: number | null) => value === null ? '未知' : new Intl.NumberFormat('zh-CN', { style: 'percent', maximumFractionDigits: 1 }).format(value);
