import { ApiError } from '../../../shared/api';
import type { DossierBlock, DossierMaterial, SourceState } from './dossierTypes';
const SOURCE_STATES: Record<SourceState, string> = { present: '已取得', known_empty: '来源明确为空', not_acquired: '尚未取得', not_applicable: '不适用', restricted: '材料受限' };
export function sourceStateLabel(state: SourceState): string { return SOURCE_STATES[state]; }
export function materialLabel(unit: DossierMaterial): string {
  if (unit.kind === 'body') return '正文';
  if (unit.kind === 'tags') return '独立标签';
  if (unit.kind === 'audio') return '音轨转写';
  if (unit.kind === 'video_frame') return '视频画面文字';
  return unit.page === null ? '图片 · 页码未知' : `第 ${unit.page} 页`;
}
export function readingLabel(unit: DossierMaterial): string {
  if (unit.sourceState !== 'present') return sourceStateLabel(unit.sourceState);
  if (unit.reading === null) return unit.kind === 'tags' ? '来源标签' : '尚未整理';
  const adoption = { automatic: '自动采用', human_adopted: '人工采用', human_edited: '人工修正' }[unit.reading.adoption];
  return `${adoption} · ${unit.reading.verification === 'verified' ? '已人工核验' : '尚未人工核验'}`;
}
export function emptyMaterialNote(unit: DossierMaterial): string {
  if (unit.sourceState === 'restricted') return '当前来源受限，内容已隐藏。';
  if (unit.sourceState === 'known_empty') return '本次来源明确为空。';
  if (unit.sourceState === 'not_applicable') return '此材料类型不适用于当前笔记。';
  if (unit.sourceState === 'not_acquired') return '来源尚未取得，不能判断原作是否没有内容。';
  return unit.currentRevisionRef === null ? '已取得来源，尚未形成当前读本。' : '本次处理未取得可读文字，仍需对照来源核验。';
}
export function exportFileName(work: string, kind: 'md' | 'json'): string { return `研究底稿-${work}.${kind}`; }
export interface CommentNode<T extends { sourceRef: string; parentSourceRef: string | null; relation: 'ROOT' | 'REPLY' }> { item: T; replies: T[]; orphan: boolean }
export function nestComments<T extends { sourceRef: string; parentSourceRef: string | null; relation: 'ROOT' | 'REPLY' }>(items: readonly T[]): CommentNode<T>[] {
  const bySource = new Map(items.map(item => [item.sourceRef, item] as const)); const nodes: CommentNode<T>[] = []; const roots = new Map<string, CommentNode<T>>();
  for (const item of items) if (item.relation === 'ROOT') { const node: CommentNode<T> = { item, replies: [], orphan: false }; roots.set(item.sourceRef, node); nodes.push(node); }
  for (const item of items) if (item.relation === 'REPLY') { const parent = item.parentSourceRef === null ? undefined : bySource.get(item.parentSourceRef); const root = parent?.relation === 'ROOT' ? roots.get(parent.sourceRef) : undefined; if (root) root.replies.push(item); else nodes.push({ item, replies: [], orphan: parent === undefined }); }
  return nodes;
}

/** An explicit access/source denial invalidates the visible cached reading. */
export function hideCachedReading(error: unknown): boolean {
  return error instanceof ApiError && error.failure.kind === 'http' && [401, 403, 404, 410].includes(error.failure.status);
}
export function blockText(block: DossierBlock): string {
  const speakers = { author: '作品作者', other: '他人表述', unconfirmed: '表达者未确认' };
  switch (block.kind) {
    case 'list': return block.items.map((item, index) => `${block.ordered ? `${(block.start ?? 1) + index}.` : '•'} ${item}`).join('\n');
    case 'table': return [block.columns, ...block.rows].map(row => row.join(' | ')).join('\n');
    case 'dialogue': return block.turns.map(turn => `${speakers[turn.speaker]}：${turn.text}`).join('\n');
    default: return block.text;
  }
}
