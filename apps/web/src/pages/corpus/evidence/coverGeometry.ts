import { laneSummary } from './evidencePresent';
import type { WorkItem } from './evidenceTypes';

/**
 * 封面排版里卡片正面的线稿标记。它只是「已知作品形态」的目录标记，不推断内容含义、证据状态或质量：
 * 有视频、多图、单图、讨论各有一种；没有任何区别时，按稳定的作品引用哈希在三种通用图文线稿里取其一，
 * 这样整页都是单封面作品时也不会在排序或翻页后塌成一片重复的图标。
 */
export type GeometryKind = 'wave' | 'stack' | 'cluster' | 'grid' | 'cone' | 'frame';

const SILENT_COMMENT_STATES = new Set(['UNKNOWN', 'NOT_REQUESTED', 'NOT_OBSERVED']);

/** FNV-1a 变体：与旧页逐位一致，保证同一作品在新旧页取到同一种标记。 */
function referenceHash(reference: string): number {
  let hash = 2_166_136_261;
  for (const character of reference) hash = Math.imul(hash ^ (character.codePointAt(0) ?? 0), 16_777_619) >>> 0;
  return hash;
}

export function coverGeometryKind(item: Pick<WorkItem, 'ref' | 'media' | 'lanes' | 'display'>): GeometryKind {
  if (item.media.videoCount > 0) return 'wave';
  if (item.media.imageCount > 1) return 'stack';
  if (item.media.imageCount > 0) return 'grid';
  const comments = laneSummary(item.lanes, 'comments');
  if (comments !== null && !SILENT_COMMENT_STATES.has(comments.state)) return 'cluster';
  const hash = referenceHash(item.ref || item.display.title || 'cover');
  const index = ((hash ^ (hash >>> 16)) >>> 0) % 3;
  return (['grid', 'cone', 'frame'] as const)[index] ?? 'grid';
}

/** 卡片正面小字「图文／多图／视频／讨论 · 已观察」里的形态词。 */
export function coverKindLabel(kind: GeometryKind): string {
  if (kind === 'wave') return '视频';
  if (kind === 'stack') return '多图';
  if (kind === 'cluster') return '讨论';
  return '图文';
}

export type Shape =
  | { tag: 'circle'; cx: number; cy: number; r: number }
  | { tag: 'rect'; x: number; y: number; width: number; height: number }
  | { tag: 'path'; d: string };

/** 六种线稿，都画在 300×400 视图的同一块安全区（x 48–252，y 80–320）里，保证没有哪张卡的视觉分量更重。 */
export const GEOMETRY_SHAPES: Readonly<Record<GeometryKind, readonly Shape[]>> = {
  wave: [
    ...[24, 48, 72, 96].map((r): Shape => ({ tag: 'circle', cx: 150, cy: 200, r })),
    { tag: 'path', d: 'M54 200h192M150 104v192' },
  ],
  stack: [
    ...([[50, 90], [60, 120], [70, 150]] as const).map(([x, y]): Shape => ({ tag: 'rect', x, y, width: 160, height: 170 })),
    { tag: 'path', d: 'M80 140h100M80 180h100M80 220h100' },
  ],
  cluster: [
    ...([[110, 180, 60], [190, 180, 60], [150, 240, 60]] as const).map(([cx, cy, r]): Shape => ({ tag: 'circle', cx, cy, r })),
    { tag: 'path', d: 'M125 260 175 300M140 222l60 56' },
  ],
  grid: [
    { tag: 'rect', x: 48, y: 80, width: 204, height: 240 },
    ...[99, 150, 201].map((position): Shape => ({ tag: 'path', d: `M${position} 80v240` })),
    ...[140, 200, 260].map((position): Shape => ({ tag: 'path', d: `M48 ${position}h204` })),
    { tag: 'path', d: 'M48 80 150 200 252 80M48 320 150 200 252 320' },
  ],
  frame: [
    { tag: 'rect', x: 48, y: 80, width: 204, height: 240 },
    { tag: 'rect', x: 85, y: 123, width: 130, height: 154 },
    { tag: 'path', d: 'M85 174h130M85 226h130M150 123v154' },
    { tag: 'path', d: 'M110 148 150 180 190 148M110 252 150 220 190 252' },
  ],
  cone: [
    { tag: 'path', d: 'M150 80 48 320h204Z' },
    ...[96, 144, 192].map((position): Shape => ({ tag: 'path', d: `M150 80 ${position} 320` })),
    { tag: 'path', d: 'M78 246h144' },
  ],
};
