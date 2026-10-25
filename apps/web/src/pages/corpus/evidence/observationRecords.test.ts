import { describe, expect, it } from 'vitest';
import type { CoverageEntry, TimelinePoint } from './evidenceTypes';
import { commentCollectionRows, deltaCopy, elapsedCopy, observationLaneCopy, observationRows } from './observationRecords';

const unknown = { value: null, state: 'UNKNOWN' };
const known = (value: number) => ({ value, state: 'KNOWN' });

function point(observedAt: string, lane: string, like: number | null, comment: number | null = null): TimelinePoint {
  return {
    observedAt,
    sourceLane: lane,
    like: like === null ? unknown : known(like),
    comment: comment === null ? unknown : known(comment),
    collect: unknown,
    share: unknown,
  };
}

describe('观察记录：每次的数字与上一次比（Mog 2026-10-23）', () => {
  it('从新到旧；每项只和它自己上一次读到的值比，没读到的那次不参与，也不当成 0', () => {
    const rows = observationRows([
      point('2026-10-12T08:36:46Z', 'discovery', 293),
      point('2026-10-14T21:59:48Z', 'detail', 293, 37),
      point('2026-10-20T01:00:00Z', 'discovery', 305),
    ]);
    expect(rows.map(row => row.last)).toEqual(['2026-10-20T01:00:00Z', '2026-10-14T21:59:48Z', '2026-10-12T08:36:46Z']);
    expect(rows[0]!.cells.like).toEqual({ value: 305, delta: 12 });
    expect(rows[0]!.cells.comment).toEqual({ value: null, delta: null });
    expect(rows[1]!.cells.like).toEqual({ value: 293, delta: 0 });
    expect(rows[1]!.cells.comment).toEqual({ value: 37, delta: null });
    expect(rows[2]!.cells.like).toEqual({ value: 293, delta: null });
  });

  it('同一来源连续读到完全相同的数字合成一行，保留首末时间与次数；来源不同不合并', () => {
    const rows = observationRows([
      point('2026-10-01T00:00:00Z', 'discovery', 10),
      point('2026-10-02T00:00:00Z', 'discovery', 10),
      point('2026-10-03T00:00:00Z', 'discovery', 10),
      point('2026-10-04T00:00:00Z', 'detail', 10),
    ]);
    expect(rows).toHaveLength(2);
    expect(rows[1]).toMatchObject({ first: '2026-10-01T00:00:00Z', last: '2026-10-03T00:00:00Z', count: 3, lane: 'discovery' });
    expect(rows[0]).toMatchObject({ count: 1, lane: 'detail' });
  });

  it('没有观察就是空列表', () => {
    expect(observationRows([])).toEqual([]);
  });

  it('来源、变化与距今的写法', () => {
    expect(observationLaneCopy('discovery')).toBe('列表中看到');
    expect(observationLaneCopy('detail')).toBe('打开详情页');
    expect(observationLaneCopy(null)).toBe('其他来源');
    expect([deltaCopy(12), deltaCopy(-3), deltaCopy(0), deltaCopy(null), deltaCopy(12345)]).toEqual(['+12', '−3', '持平', null, '+12,345']);
    const now = Date.parse('2026-10-23T10:00:00Z');
    expect(elapsedCopy('2026-10-15 05:59', now)).toBe('8 天前');
    expect(elapsedCopy('2026-10-14T21:59:48.643Z', now)).toBe('8 天前');
    expect(elapsedCopy('2026-10-15 05:59:48.711965+08', now)).toBe('8 天前');
    expect(elapsedCopy('2026-10-23 17:00', now)).toBe('今天');
    expect(elapsedCopy('2026-10-24 17:00', now)).toBeNull();
    expect(elapsedCopy('不是时间', now)).toBeNull();
    expect(elapsedCopy(null, now)).toBeNull();
  });
});

function coverage(overrides: Partial<CoverageEntry>): CoverageEntry {
  return {
    observedAt: '2026-10-14T21:59:48Z', state: 'PARTIAL', collectionScope: 'detail_window', requestedLimit: 30, pageCommentCount: 37,
    expectedCount: 30, uniqueCollectedCount: 21, collectionState: 'partial', producerAcquired: 18, retained: 18, failed: 0,
    taskRef: null, attemptRef: null, packageRef: null, receiptRef: null, ...overrides,
  };
}

describe('评论采集记录', () => {
  it('从新到旧；采到的数优先用不重复数；只有「尽量采全」才判断采全没有', () => {
    const rows = commentCollectionRows([
      coverage({}),
      coverage({ observedAt: '2026-10-20T00:00:00Z', collectionScope: 'all_public_comments', collectionState: 'complete', uniqueCollectedCount: 37 }),
      coverage({ observedAt: '2026-10-18T00:00:00Z', collectionScope: 'all_public_comments', collectionState: 'partial', uniqueCollectedCount: 30 }),
    ]);
    expect(rows.map(row => [row.scope, row.collected, row.shown, row.complete])).toEqual([
      ['尽量采全', 37, 37, true],
      ['尽量采全', 30, 37, false],
      ['随详情顺带采（最多 30 条）', 21, 37, null],
    ]);
  });

  it('不认识的范围写「评论采集」；数量都没报是 null', () => {
    const [row] = commentCollectionRows([coverage({ collectionScope: null, uniqueCollectedCount: null, retained: null, producerAcquired: null, pageCommentCount: null })]);
    expect(row).toMatchObject({ scope: '评论采集', collected: null, shown: null, complete: null });
  });
});
