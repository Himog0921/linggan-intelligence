import { describe, expect, it } from 'vitest';
import { GEOMETRY_SHAPES, coverGeometryKind, coverKindLabel } from './coverGeometry';
import { parseWorkList } from './evidenceDecode';
import { listItemJson, listJson, workRef } from './evidenceFixtures';

const item = (n: number, options: Parameters<typeof listItemJson>[1] = {}) => parseWorkList(listJson([listItemJson(n, options)])).items[0]!;

describe('封面线稿标记（E10）', () => {
  it('有视频、多图、单图、讨论各有一种形态', () => {
    expect(coverGeometryKind(item(1, { media: { videos: 1 } }))).toBe('wave');
    expect(coverGeometryKind(item(1, { media: { images: 3 } }))).toBe('stack');
    expect(coverGeometryKind(item(1, { media: { images: 1 } }))).toBe('grid');
    expect(coverGeometryKind(item(1, { lanes: { comments: 'SEARCHABLE' } }))).toBe('cluster');
    expect(coverGeometryKind(item(1, { lanes: { comments: 'UNKNOWN' } }))).not.toBe('cluster');
    expect(coverGeometryKind(item(1, { lanes: { comments: 'NOT_OBSERVED' } }))).not.toBe('cluster');
  });

  it('没有区别时按稳定的作品引用哈希在三种通用线稿里取其一——与旧页算法逐位一致', () => {
    // 期望值由旧页 evidence_library.js 的哈希算法实际计算得到。
    const expected: Record<number, string> = { 1: 'frame', 2: 'cone', 3: 'cone', 4: 'frame', 5: 'grid', 6: 'grid', 7: 'frame' };
    for (const [n, kind] of Object.entries(expected)) expect(coverGeometryKind(item(Number(n))), workRef(Number(n))).toBe(kind);
    expect(coverGeometryKind(item(2))).toBe(coverGeometryKind(item(2)));
  });

  it('形态词与线稿数据：每种形态都有图形，只画在安全区内的图元', () => {
    expect(coverKindLabel('wave')).toBe('视频');
    expect(coverKindLabel('stack')).toBe('多图');
    expect(coverKindLabel('cluster')).toBe('讨论');
    expect(coverKindLabel('grid')).toBe('图文');
    for (const kind of ['wave', 'stack', 'cluster', 'grid', 'cone', 'frame'] as const) expect(GEOMETRY_SHAPES[kind].length).toBeGreaterThan(1);
  });
});
