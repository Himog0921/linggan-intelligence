import type { Dossier, DossierBlock, DossierIssue, DossierMaterial, ReadingGroup, ReadingSection, ReadingSectionKind, Speaker } from './dossierTypes';
import { normalizedTagLabel, pureTagLabels } from './dossierTagText';
import { confirmedEmptySpeech } from './dossierPresent';
import { hasReadingSourceText, missingImageTextPages } from './dossierReadingContent';

export const SECTION_LABELS: Record<ReadingSectionKind, string> = { publication: '发布文案', images: '图片资料', audio: '视频口播', frames: '画面文字' };
export interface ReadingEntry { unit: DossierMaterial; block: DossierBlock }
export interface ReadingIssue extends DossierIssue { displayKey: string }
export interface ResolvedReadingGroup { group: ReadingGroup; entries: ReadingEntry[]; issues: ReadingIssue[] }
export interface ResolvedReadingSection { section: ReadingSection; groups: ResolvedReadingGroup[] }
export interface ReadingGap { key: string; label: string; message: string; state: string }
export type AttributionBoundary = Speaker | 'dialogue' | undefined;
export function endingBlockAttribution(block: DossierBlock | undefined): AttributionBoundary { return block?.kind === 'dialogue' ? 'dialogue' : block ? block.speaker ?? 'unconfirmed' : undefined; }
export function endingGroupAttribution(group: ResolvedReadingGroup | undefined): AttributionBoundary { return endingBlockAttribution(group?.entries.at(-1)?.block); }
export function showReadingAttribution(role: Speaker, previous: AttributionBoundary): boolean { return role !== 'unconfirmed' && previous !== role && (previous !== undefined || role === 'other'); }

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
      .filter(issue => { if (issue.code === 'cloud_image_unchecked') return false; if (seenIssues.has(issue.displayKey)) return false; seenIssues.add(issue.displayKey); return true; });
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
  if (group.mode === 'heading' || group.mode === 'paragraph' || group.mode === 'transcript_paragraph') return entries.map(({ block }, index) => `${index ? group.joiners[index - 1] : ''}${blockText(block)}`).join('');
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
/** Exact source range, including subsecond positions when provided. */
export function formatReadingTimeRange(time: { startMs: number; endMs: number }): string {
  const precise = (milliseconds: number) => `${formatReadingTime(milliseconds)}${milliseconds % 1000 ? `.${String(milliseconds % 1000).padStart(3, '0')}` : ''}`;
  return time.startMs === time.endMs ? precise(time.startMs) : `${precise(time.startMs)}–${precise(time.endMs)}`;
}
export function transcriptStats(groups: readonly ResolvedReadingGroup[]): string {
  const characterCount = groups.reduce((count, group) => count + Array.from(groupText(group).replace(/\s/gu, '')).length, 0);
  const times = groups.map(groupTime).filter((value): value is NonNullable<typeof value> => value !== null);
  const span = times.length ? `已转写 ${formatReadingTime(Math.min(...times.map(time => time.startMs)))}–${formatReadingTime(Math.max(...times.map(time => time.endMs)))} · ` : '';
  return `${span}${characterCount.toLocaleString('zh-CN')} 字`;
}
export function readingAnchor(prefix: string, ref: string): string { return `${prefix}-${encodeURIComponent(ref)}`; }

/** One concise row per relevant missing content type. Processing diagnostics stay in source detail. */
export function readingGaps(dossier: Dossier): ReadingGap[] {
  const rows: ReadingGap[] = [];
  const kinds = { body: '发布文案', images: '图片文字', audio: '视频口播', videoFrames: '画面文字' } as const;
  const materialKinds = { body: 'body', images: 'image', audio: 'audio', videoFrames: 'video_frame' } as const;
  const messages: Record<string, { state: string; message: string }> = {
    not_acquired: { state: '尚未取得', message: '来源尚未取得。' },
    not_processed: { state: '待识读', message: '尚无可用文字。' },
    restricted: { state: '受限', message: '来源不可读取。' },
    partial: { state: '部分可读', message: '部分内容尚未取得。' },
  };
  for (const [key, label] of Object.entries(kinds) as [keyof typeof kinds, string][]) {
    const source = dossier.coverage.sources[key];
    const units = dossier.materials.filter(unit => unit.kind === materialKinds[key]);
    if (source.units === 0 && units.length === 0) continue;
    // Frame sampling is optional enrichment, not a missing video transcript.
    if (key === 'videoFrames' && units.every(unit => !unit.currentRevisionRef && !unit.processing)) continue;
    const incomplete = units.filter(unit => !unit.blocks.some(hasReadingSourceText) && !confirmedEmptySpeech(unit)
      && !['not_applicable', 'known_empty', 'restricted', 'not_acquired'].includes(unit.sourceState));
    const processing = [...new Set(incomplete.flatMap(unit => unit.processing && unit.processing.state !== 'completed' ? [unit.processing.state] : []))];
    const note = messages[source.state];
    if (note) rows.push({ key, label, ...note });
    if (processing.length) {
      const labels: Record<string, string> = { pending: '待处理', running: '处理中', failed: '处理失败', cancelled: '已取消', invalidated: '已失效' };
      const state = processing.length === 1 ? labels[processing[0]!] ?? '待处理' : '部分待处理';
      const existing = rows.find(row => row.key === key);
      const message = key === 'audio' ? '尚无可用转录。' : '尚无可用文字。';
      if (existing) { existing.state = state; existing.message = message; }
      else rows.push({ key, label, state, message });
    } else if (incomplete.length && !note) {
      const partial = units.some(unit => unit.blocks.some(hasReadingSourceText));
      rows.push({ key, label, state: partial ? '部分可读' : '待补全',
        message: `${partial ? '部分内容' : ''}暂无可用${key === 'audio' ? '转录' : '文字'}。` });
    }
  }
  const missingPages = missingImageTextPages(dossier.materials, dossier.coverage.imageInventory);
  if (missingPages.length) {
    const message = `第 ${missingPages.join('、')} 页文字待补全。`;
    const existing = rows.find(row => row.key === 'images');
    if (existing) existing.message = message;
    else rows.push({ key: 'images', label: '图片文字', state: '部分可读', message });
  }
  return rows;
}
export function remainingReadingIssues(dossier: Dossier, sections: readonly ResolvedReadingSection[]): ReadingIssue[] {
  const shown = new Set(sections.flatMap(section => section.groups.flatMap(group => group.issues.map(issue => issue.displayKey))));
  return [...dossier.coverage.issues.map(issue => ({ ...issue, displayKey: `coverage:${issue.key}` })),
    ...dossier.materials.flatMap(unit => unit.issues.map(issue => ({ ...issue, displayKey: `${unit.ref}:${issue.key}` })))].filter(issue => {
    if (issue.code === 'cloud_image_unchecked') return false;
    if (shown.has(issue.displayKey)) return false;
    shown.add(issue.displayKey); return true;
  });
}

/** One disclosure per image; region-specific issues keep their location when collected. */
export function imageReviewIssues(groups: readonly ResolvedReadingGroup[], unitRef: string): ReadingIssue[] {
  return groups.filter(group => group.entries.some(entry => entry.unit.ref === unitRef)).flatMap(group => group.issues.map(issue => {
    const block = group.entries.find(entry => entry.block.ref === issue.blockRef)?.block;
    return block?.regionLabel ? { ...issue, message: `${block.regionLabel}：${issue.message}` } : issue;
  }));
}
