import type { Dossier, DossierBlock, DossierIssue, DossierMaterial, ReadingGroup, ReadingSection, ReadingSectionKind, Speaker } from './dossierTypes';
import { normalizedTagLabel, pureTagLabels } from './dossierTagText';
import { processingLabel } from './dossierPresent';
import { missingImageTextPages } from './dossierReadingContent';

export const SECTION_LABELS: Record<ReadingSectionKind, string> = { publication: '发布文案', images: '图片文字', audio: '视频口播', frames: '画面文字' };
export interface ReadingEntry { unit: DossierMaterial; block: DossierBlock }
export interface ReadingIssue extends DossierIssue { displayKey: string }
export interface ResolvedReadingGroup { group: ReadingGroup; entries: ReadingEntry[]; issues: ReadingIssue[] }
export interface ResolvedReadingSection { section: ReadingSection; groups: ResolvedReadingGroup[] }
export interface ReadingGap { key: string; label: string; message: string; state: string }
export type AttributionBoundary = Speaker | 'dialogue' | undefined;
export function endingBlockAttribution(block: DossierBlock | undefined): AttributionBoundary { return block?.kind === 'dialogue' ? 'dialogue' : block ? block.speaker ?? 'unconfirmed' : undefined; }
export function endingGroupAttribution(group: ResolvedReadingGroup | undefined): AttributionBoundary { return endingBlockAttribution(group?.entries.at(-1)?.block); }
export function showReadingAttribution(role: Speaker, previous: AttributionBoundary): boolean { return previous !== role && (previous !== undefined || role === 'other'); }

/** Resolves references only. Never sorts media, infers page order, or merges blocks on the client. */
export function resolveReadingSections(dossier: Dossier): ResolvedReadingSection[] {
  const byRef = new Map(dossier.materials.flatMap(unit => unit.blocks.map(block => [block.ref, { unit, block }] as const)));
  const seenIssues = new Set<string>();
  return dossier.readingView.sections.map(section => ({ section, groups: section.groups.map(group => {
    const entries = group.blockRefs.map(ref => {
      const entry = byRef.get(ref);
      if (!entry) throw new Error('规范阅读引用不存在'); // The runtime decoder prevents this path.
      return entry;
    });
    const issues = entries.flatMap(({ block, unit }) => [...(block.issues ?? []), ...unit.issues.filter(issue => !issue.blockRef || group.blockRefs.includes(issue.blockRef))]
      .map(issue => ({ ...issue, displayKey: `${unit.ref}:${issue.key}` })))
      .filter(issue => { if (seenIssues.has(issue.displayKey)) return false; seenIssues.add(issue.displayKey); return true; });
    return { group, entries, issues };
  }) }));
}

export function blockText(block: DossierBlock): string {
  switch (block.kind) {
    case 'heading': case 'paragraph': case 'quote': case 'gap': return block.text;
    case 'list': return block.items.join('\n');
    case 'table': return [block.columns, ...block.rows].map(row => row.join('\t')).join('\n');
    case 'dialogue': return block.turns.map(turn => turn.text).join('\n');
  }
}
export function groupText({ group, entries }: ResolvedReadingGroup): string {
  if (group.mode === 'paragraph' || group.mode === 'transcript_paragraph') return entries.map(({ block }, index) => `${index ? group.joiners[index - 1] : ''}${blockText(block)}`).join('');
  if (group.mode === 'tags') {
    return groupTagLabels(entries).join('\n');
  }
  return entries.map(({ block }) => blockText(block)).join('\n');
}
export function groupTagLabels(entries: readonly ReadingEntry[]): string[] {
  const native = entries.find(({ unit }) => unit.kind === 'tags')?.block;
  if (native?.kind === 'list') return native.items.map(normalizedTagLabel);
  const body = entries.find(({ unit }) => unit.kind === 'body')?.block;
  return body?.kind === 'paragraph' ? pureTagLabels(body.text) ?? [] : [];
}
export function groupTime(group: ResolvedReadingGroup): { startMs: number; endMs: number } | null {
  const first = group.entries[0]?.block.time;
  const last = group.entries.at(-1)?.block.time;
  return first && last ? { startMs: first.startMs, endMs: last.endMs } : null;
}
export function formatReadingTime(milliseconds: number): string {
  const seconds = Math.floor(milliseconds / 1000);
  return `${String(Math.floor(seconds / 60)).padStart(2, '0')}:${String(seconds % 60).padStart(2, '0')}`;
}
export function transcriptStats(groups: readonly ResolvedReadingGroup[]): string {
  const characterCount = groups.reduce((count, group) => count + Array.from(groupText(group).replace(/\s/gu, '')).length, 0);
  const times = groups.map(groupTime).filter((value): value is NonNullable<typeof value> => value !== null);
  const span = times.length ? `已转写 ${formatReadingTime(Math.min(...times.map(time => time.startMs)))}–${formatReadingTime(Math.max(...times.map(time => time.endMs)))} · ` : '';
  return `${span}${characterCount.toLocaleString('zh-CN')} 字`;
}
export function readingAnchor(prefix: string, ref: string): string { return `${prefix}-${encodeURIComponent(ref)}`; }

export function readingGaps(dossier: Dossier): ReadingGap[] {
  const rows: ReadingGap[] = [];
  const kinds = { body: '发布文案', images: '图片文字', audio: '视频口播', videoFrames: '画面文字' } as const;
  for (const [key, label] of Object.entries(kinds) as [keyof typeof kinds, string][]) {
    const source = dossier.coverage.sources[key];
    if (['available', 'known_empty', 'not_applicable'].includes(source.state)) continue;
    const messages: Record<string, { state: string; message: string }> = {
      not_acquired: { state: '尚未取得', message: '来源尚未取得，不能判断原作是否没有内容。' },
      not_processed: { state: '待识读', message: '已有来源，尚无当前可用的识读文字。' },
      restricted: { state: '受限', message: '当前来源不可读取。' },
      partial: { state: '部分可读', message: '当前范围尚不完整，已取得的文字照常保留。' },
    };
    const note = messages[source.state];
    if (note) rows.push({ key, label, ...note });
  }
  const inventory = dossier.coverage.imageInventory;
  const imageNote: string[] = [];
  const missingTextPages = missingImageTextPages(dossier.materials, inventory);
  if (missingTextPages.length) imageNote.push(`第 ${missingTextPages.join('、')} 页文字待补全`);
  if (inventory.unknownOrder) imageNote.push('图片原始顺序未确认');
  if (inventory.expectedPages === null && inventory.acquiredPages > 0) imageNote.push('原作总页数未知');
  if (imageNote.length) {
    const existing = rows.find(row => row.key === 'images');
    if (existing) existing.message = `${imageNote.join('；')}。已取得文字照常保留。`;
    else rows.push({ key: 'image-range', label: '图片文字', state: '范围待确认', message: `${imageNote.join('；')}。` });
  }
  for (const [kind, coverageKey] of [['body', 'body'], ['image', 'images'], ['audio', 'audio'], ['video_frame', 'videoFrames']] as const) {
    const units = dossier.materials.filter(unit => unit.kind === kind && !['not_applicable', 'restricted', 'not_acquired'].includes(unit.sourceState));
    const descriptions = [...new Set(units.flatMap(unit => {
      const state = unit.processing?.state;
      if (!state || state === 'completed') return [];
      const labels = { pending: '待处理', running: '处理中', failed: '处理失败', cancelled: '处理已取消', invalidated: '处理结果已失效' };
      if (unit.sourceState === 'known_empty' && unit.currentRevisionRef) return [`${labels[state]}；既有已确认无文字的结果仍有效。`];
      const message = processingLabel(unit);
      return message ? [message] : [];
    }))];
    if (!descriptions.length) continue;
    const states = [...new Set(units.flatMap(unit => unit.processing && unit.processing.state !== 'completed' ? [unit.processing.state] : []))];
    const labels: Record<string, string> = { pending: '待处理', running: '处理中', failed: '处理失败', cancelled: '已取消', invalidated: '已失效' };
    const state = states.length === 1 ? labels[states[0]!] ?? '待核对' : '部分待处理';
    const existing = rows.find(row => row.key === coverageKey);
    if (existing) { existing.state = state; existing.message = `${descriptions.join(' ')}${existing.message}`; }
    else rows.push({ key: `processing-${kind}`, label: kinds[coverageKey], state, message: descriptions.join(' ') });
  }
  return rows;
}
export function remainingReadingIssues(dossier: Dossier, sections: readonly ResolvedReadingSection[]): ReadingIssue[] {
  const shown = new Set(sections.flatMap(section => section.groups.flatMap(group => group.issues.map(issue => issue.displayKey))));
  return [...dossier.coverage.issues.map(issue => ({ ...issue, displayKey: `coverage:${issue.key}` })),
    ...dossier.materials.flatMap(unit => unit.issues.map(issue => ({ ...issue, displayKey: `${unit.ref}:${issue.key}` })))].filter(issue => {
    if (shown.has(issue.displayKey)) return false;
    shown.add(issue.displayKey); return true;
  });
}
