import { expect, it } from 'vitest';
import { parsePreview } from './api';

function fixture() {
  const identity = { domainRef: 'domain', workRef: 'work', snapshotIdentity: 'snapshot', directionVersionRef: 'direction-version', skillVersionRef: 'skill-version' };
  const catalogue = { origin: 'offline_mock', versionRef: 'direction-version', groups: [], directions: [] };
  const output = { contract: 'note-interpretation-offline.v1', origin: 'offline_mock', identity, relevance: 'unknown', primaryDirection: null, alsoRelated: null, reason: null, confidence: 'possible', perspective: 'unknown', keywords: [] };
  const text = '不能忽略例外🙂'.repeat(20000);
  const input = { contract: 'note-interpretation-offline-input.v2', origin: 'offline_mock', identity, asOf: '2026-10-23T08:00:00Z', columns: ['title', 'body', 'tags', 'ocr', 'transcript', 'frame_text'].map(field => ({ field, state: 'available' })), fragments: [{ citationId: 'block', field: 'transcript', text, start: 0, end: Array.from(text).length, sourceRef: 'source', sourceVersion: 'source-version', readingRef: 'reading', unitRef: 'unit', speaker: 'other', structure: { ref: 'block', kind: 'paragraph', text, speaker: 'other', time: { startMs: 0, endMs: 10000 } }, issues: [{ key: 'voice', code: 'uncertain_voice', message: '说话者待核验', resolution: 'reading_edit' }] }] };
  return { contract: 'note-interpretation-offline-preview.v1', origin: 'offline_mock', scenario: 'baseline', sample: { origin: 'offline_mock', manifestSha256: 'sha', fixtureRevision: 'fixture', asOf: input.asOf }, catalogue, baselineCatalogue: catalogue, candidateCatalogue: catalogue, comparison: [], notes: [{ title: '合成输入', likes: null, publishedWeek: null, input, baseline: output, result: { machine: output, effective: output, judgment: null, manualApplied: false, needsReview: false } }], summary: { total: 1, unknown: 1, notRelated: 0, noSuitableDirection: 0, unknownLikes: 1, unknownWeek: 1, directions: [] } };
}
it('offline input uses the sole v2 contract and preserves complete canonical structure, attribution, issues and cutoff', () => {
  const raw = fixture(); const input = parsePreview(raw).notes[0]!.input;
  expect(input.columns.map(column => column.field)).toEqual(['title', 'body', 'tags', 'ocr', 'transcript', 'frame_text']);
  expect(input.asOf).toBe(raw.notes[0]!.input.asOf); expect(input.fragments[0]).toMatchObject(raw.notes[0]!.input.fragments[0]!);
});
it('offline input rejects the old five-column contract and mismatched block identity', () => {
  const old = fixture(); old.notes[0]!.input.contract = 'note-interpretation-offline-input.v1'; expect(() => parsePreview(old)).toThrow();
  const wrong = fixture(); wrong.notes[0]!.input.fragments[0]!.structure.ref = 'another'; expect(() => parsePreview(wrong)).toThrow();
});
