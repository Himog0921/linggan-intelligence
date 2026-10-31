import { bool, list, obj, ShapeError, str, type Json } from '../../../shared/decode';
import { choice, strings } from './dossierBlockDecode';
import { normalizedTagLabel, pureTagLabels } from './dossierTagText';
import { missingImageTextPages } from './dossierReadingContent';
import type { DossierCoverage, DossierMaterial, ReadingGroup, ReadingSection, ReadingSectionKind } from './dossierTypes';

const SECTION_KINDS = ['publication', 'images', 'audio', 'frames'] as const;
const MODES = ['blocks', 'paragraph', 'transcript_paragraph', 'list', 'table', 'tags'] as const;
const KIND_SECTION: Record<DossierMaterial['kind'], ReadingSectionKind> = {
  body: 'publication', tags: 'publication', image: 'images', audio: 'audio', video_frame: 'frames',
};
/** No client-side fallback or regrouping: the server's complete ordered reference plan is required. */
export function parseReadingSections(reading: Json, materials: readonly DossierMaterial[], mainRefs: readonly string[], supplementRefs: readonly string[], inventory: DossierCoverage['imageInventory']): ReadingSection[] {
  const byRef = new Map(materials.flatMap(unit => unit.blocks.map(block => [block.ref, { unit, block }] as const)));
  const seenSections = new Set<string>();
  const seenGroups = new Set<string>();
  const missingTextPages = new Set(missingImageTextPages(materials, inventory));
  let previousKindIndex = -1;
  const sections = list(reading, 'sections', row => {
    const ref = str(row, 'ref');
    if (!ref || seenSections.has(ref)) throw new ShapeError('readingView.sections.ref');
    seenSections.add(ref);
    const kind = choice(row.kind, SECTION_KINDS, 'readingView.sections.kind');
    const kindIndex = SECTION_KINDS.indexOf(kind);
    if (kindIndex <= previousKindIndex) throw new ShapeError('readingView.sections.order');
    previousKindIndex = kindIndex;
    const groups = list(row, 'groups', value => {
      const groupRef = str(value, 'ref');
      const mode = choice(value.mode, MODES, 'readingView.group.mode');
      const blockRefs = strings(value.blockRefs, 'readingView.group.blockRefs');
      const continuation = obj(value, 'continuation');
      if (!groupRef || seenGroups.has(groupRef) || blockRefs.length === 0 || new Set(blockRefs).size !== blockRefs.length) throw new ShapeError('readingView.group.ref');
      seenGroups.add(groupRef);
      const resolved = blockRefs.map(blockRef => {
        const entry = byRef.get(blockRef);
        if (!entry || KIND_SECTION[entry.unit.kind] !== kind) throw new ShapeError('readingView.group.blockRefs');
        return entry;
      });
      const base = { ref: groupRef, mode, blockRefs, continuation: {
        fromPrevious: bool(continuation, 'fromPrevious'), toNext: bool(continuation, 'toNext'),
      } };
      if (mode !== 'blocks' && new Set(resolved.map(({ block }) => block.speaker ?? 'unconfirmed')).size > 1) throw new ShapeError('readingView.group.speaker');
      if (mode === 'paragraph' || mode === 'transcript_paragraph') {
        const joiners = strings(value.joiners, 'readingView.group.joiners').map(joiner => choice(joiner, ['', ' ', '\n'], 'readingView.group.joiners'));
        const imageProse = mode === 'paragraph' && kind === 'images' && new Set(resolved.map(entry => entry.unit.ref)).size === 1;
        if (joiners.length !== blockRefs.length - 1 || resolved.some(entry => entry.block.kind !== 'paragraph' && !(imageProse && entry.block.kind === 'heading'))) throw new ShapeError('readingView.group.paragraph');
        if (mode === 'transcript_paragraph' && (kind !== 'audio' || new Set(resolved.map(entry => entry.unit.ref)).size !== 1)) throw new ShapeError('readingView.group.transcript');
        return { ...base, mode, joiners } satisfies ReadingGroup;
      }
      if ('joiners' in value) throw new ShapeError('readingView.group.joiners');
      if (mode === 'list') {
        let nextStart: number | null = null;
        for (const { block } of resolved) {
          if (block.kind !== 'list' || (resolved.length > 1 && !block.ordered) || (nextStart !== null && block.start !== nextStart)) throw new ShapeError('readingView.group.list');
          nextStart = (block.start ?? 1) + block.items.length;
        }
      }
      if (mode === 'table') {
        const first = resolved[0]?.block;
        if (first?.kind !== 'table' || resolved.some(entry => entry.block.kind !== 'table' || JSON.stringify(entry.block.columns) !== JSON.stringify(first.columns))) throw new ShapeError('readingView.group.table');
      }
      if (mode === 'tags') {
        const tagValues = resolved.map(({ unit, block }) => {
          if (unit.blocks.length !== 1) throw new ShapeError('readingView.group.tags');
          if (unit.kind === 'body' && block.kind === 'paragraph') {
            const tags = pureTagLabels(block.text);
            if (!tags) throw new ShapeError('readingView.group.tags');
            return tags;
          }
          if (unit.kind === 'tags' && block.kind === 'list' && !block.ordered) {
            const tags = block.items.map(normalizedTagLabel);
            if (tags.some(tag => !tag)) throw new ShapeError('readingView.group.tags');
            return tags;
          }
          throw new ShapeError('readingView.group.tags');
        });
        if (resolved.length > 2 || (resolved.length === 2 && (resolved[0]?.unit.kind !== 'body' || resolved[1]?.unit.kind !== 'tags'
          || !resolved[0].unit.sourceRef || resolved[0].unit.sourceRef !== resolved[1].unit.sourceRef
          || JSON.stringify(tagValues[0]) !== JSON.stringify(tagValues[1])))) throw new ShapeError('readingView.group.tags');
      }
      return { ...base, mode } satisfies ReadingGroup;
    });
    if (groups.length === 0) throw new ShapeError('readingView.sections.groups');
    if (!Array.isArray(row.gaps)) throw new ShapeError('readingView.sections.gaps');
    const gapAnchors = new Set<string | null>(), gapPages = new Set<number>();
    const gaps = list(row, 'gaps', gap => {
      if (gap.beforeGroupRef !== null && (typeof gap.beforeGroupRef !== 'string' || !groups.some(group => group.ref === gap.beforeGroupRef))) throw new ShapeError('readingView.sections.gaps.beforeGroupRef');
      if (!Array.isArray(gap.missingPages) || gap.missingPages.length === 0 || gap.missingPages.some(page => typeof page !== 'number' || !Number.isSafeInteger(page) || page < 1)) throw new ShapeError('readingView.sections.gaps.missingPages');
      const beforeGroupRef = gap.beforeGroupRef as string | null, missingPages = gap.missingPages as number[];
      if (gapAnchors.has(beforeGroupRef) || missingPages.some((page, index) => gapPages.has(page) || !missingTextPages.has(page) || (index > 0 && page <= missingPages[index - 1]!))) throw new ShapeError('readingView.sections.gaps.order');
      gapAnchors.add(beforeGroupRef); missingPages.forEach(page => gapPages.add(page));
      return { beforeGroupRef, missingPages };
    });
    if (gaps.length && (kind !== 'images' || inventory.unknownOrder)) throw new ShapeError('readingView.sections.gaps.kind');
    return { ref, kind, groups, gaps };
  });
  const groupedRefs = sections.flatMap(section => section.groups.flatMap(group => group.blockRefs));
  const orderedRefs = [...mainRefs, ...supplementRefs];
  const membership = new Set(orderedRefs);
  if (groupedRefs.length !== orderedRefs.length || new Set(groupedRefs).size !== groupedRefs.length || groupedRefs.some(ref => !membership.has(ref))) throw new ShapeError('readingView.sections.order');
  // Legacy arrays retain the Works tab's original order. Only sections define dossier order.
  return sections;
}
