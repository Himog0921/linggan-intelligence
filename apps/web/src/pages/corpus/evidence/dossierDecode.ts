import { bool, decode, list, num, obj, optNum, optObj, optStr, ShapeError, str, type Json } from '../../../shared/decode';
import { apiPath, mediaPath } from './evidencePaths';
import { choice, count, parseBlocks, parseIssue, strings } from './dossierBlockDecode';
import type { Dossier, DossierCommentPage, DossierCoverage, DossierMaterial, DossierPayload, DossierSelection, ReadingCandidate, ReadingReviewReceipt, ReadingReviewSummary, RelayerPlan, RelayerReceipt } from './dossierTypes';

const KINDS = ['body', 'tags', 'image', 'audio', 'video_frame'] as const;
const PRODUCERS = ['platform', 'ordinary_ocr', 'local_vision', 'cloud_vision', 'asr', 'frame_ocr', 'human_edit'] as const;
const ADOPTIONS = ['automatic', 'human_adopted', 'human_edited'] as const;
const VERIFICATIONS = ['unchecked', 'verified'] as const;
const REVIEW_ACTIONS = ['adopt', 'edit', 'keep', 'pending', 'reject'] as const;
const COVERAGE = ['available', 'known_empty', 'not_acquired', 'not_applicable', 'restricted', 'partial', 'not_processed'] as const;
function optionalCount(row: Json, name: string, minimum = 0): number | null {
  const value = optNum(row, name); return value === null ? null : count(value, name, minimum);
}
function parseReview(row: Json): ReadingReviewSummary | null {
  if (row.review === undefined) throw new ShapeError('review');
  const review = optObj(row, 'review');
  if (review === null) return null;
  const actor = str(review, 'actor'); const reason = str(review, 'reason');
  if (!actor.trim() || !reason.trim() || Array.from(actor).length > 120 || Array.from(reason).length > 300) throw new ShapeError('review');
  return { commandRef: str(review, 'commandRef'), actor, reason, action: choice(review.action, REVIEW_ACTIONS, 'review.action'), reviewedAt: str(review, 'reviewedAt') };
}
function parseMaterial(row: Json): DossierMaterial {
  const reading = optObj(row, 'reading');
  const processing = optObj(row, 'processing');
  const pageSource = optObj(row, 'pageSource');
  if (pageSource && ['sourceRef', 'sourceVersion', 'slotKey'].some(key => str(pageSource, key) === '')) throw new ShapeError('pageSource');
  return {
    ref: str(row, 'ref'), kind: choice(row.kind, KINDS, 'kind'), slotKey: optStr(row, 'slotKey'), page: optionalCount(row, 'page', 1),
    ...(pageSource === null ? {} : { pageSource: { basis: choice(pageSource.basis, ['same_verified_bytes'], 'pageSource.basis'), sourceRef: str(pageSource, 'sourceRef'), sourceVersion: str(pageSource, 'sourceVersion'), slotKey: str(pageSource, 'slotKey') } }),
    sourceState: choice(row.sourceState, ['present', 'known_empty', 'not_acquired', 'not_applicable', 'restricted'], 'sourceState'),
    sourceRef: optStr(row, 'sourceRef'), sourceVersion: optStr(row, 'sourceVersion'), currentRevisionRef: optStr(row, 'currentRevisionRef'),
    originalUrl: mediaPath(row.originalUrl), blocks: parseBlocks(row), issues: list(row, 'issues', parseIssue),
    processing: processing === null ? null : { state: choice(processing.state, ['pending', 'running', 'completed', 'failed', 'cancelled', 'invalidated'], 'processing.state'), reason: optStr(processing, 'reason') },
    reading: reading === null ? null : { generatedBy: choice(reading.generatedBy, PRODUCERS, 'generatedBy'), adoption: choice(reading.adoption, ADOPTIONS, 'adoption'), verification: choice(reading.verification, VERIFICATIONS, 'verification'), ruleVersion: str(reading, 'ruleVersion'), review: parseReview(reading) },
  };
}
function parseCoverage(row: Json): DossierCoverage {
  const images = obj(row, 'imageInventory'); const source = obj(row, 'sources');
  const expectedPages = optionalCount(images, 'expectedPages');
  const acquiredPages = count(images.acquiredPages, 'acquiredPages'); const processedPages = count(images.processedPages, 'processedPages');
  if (processedPages > acquiredPages || (expectedPages !== null && acquiredPages > expectedPages)) throw new ShapeError('imageInventory');
  if (!Array.isArray(images.missingPages)) throw new ShapeError('missingPages');
  const missingPages = images.missingPages.map(value => count(value, 'missingPages', 1));
  if (new Set(missingPages).size !== missingPages.length || missingPages.some(page => expectedPages === null || page > expectedPages)) throw new ShapeError('missingPages');
  const readSource = (key: string) => {
    const value = obj(source, key); const units = count(value.units, 'units'); const readings = count(value.readings, 'readings');
    if (readings > units) throw new ShapeError('readings');
    return { state: choice(value.state, COVERAGE, 'state'), units, readings };
  };
  return { imageInventory: { basis: choice(images.basis, ['platform_image_list', 'observed_set'], 'basis'), expectedPages, acquiredPages, processedPages, missingPages, unknownOrder: bool(images, 'unknownOrder') },
    sources: { body: readSource('body'), images: readSource('images'), audio: readSource('audio'), videoFrames: readSource('videoFrames') }, issues: list(row, 'issues', parseIssue) };
}
function parseSelection(row: Json): DossierSelection {
  const kinds = strings(row.kinds, 'kinds').map(value => choice(value, KINDS, 'kinds'));
  if (!Array.isArray(row.imagePages)) throw new ShapeError('imagePages');
  return { mode: choice(row.mode, ['full', 'explicit'], 'mode'), kinds, unitRefs: strings(row.unitRefs, 'unitRefs'), blockRefs: strings(row.blockRefs, 'blockRefs'), imagePages: row.imagePages.map(value => count(value, 'imagePages', 1)), omittedUnits: count(row.omittedUnits, 'omittedUnits'), completeRevision: str(row, 'completeRevision') };
}
function parseDossierRow(row: Json): Dossier {
  if (row.contract !== 'research-dossier.v2' || 'fragments' in row || 'media' in row || 'markdown' in row) throw new ShapeError('contract');
  const note = obj(row, 'note'); const titleSource = optObj(note, 'titleSource'); const attachments = obj(row, 'attachments');
  const attachment = (name: string) => { const href = apiPath(str(obj(attachments, name), 'href')); if (href === null) throw new ShapeError(`attachments.${name}`); return { href }; };
  const materials = list(row, 'materials', parseMaterial); const blocks = materials.flatMap(unit => unit.blocks);
  if (new Set(materials.map(unit => unit.ref)).size !== materials.length || new Set(blocks.map(block => block.ref)).size !== blocks.length) throw new ShapeError('materials.ref');
  const reading = obj(row, 'readingView');
  const mainBlockRefs = strings(reading.mainBlockRefs, 'readingView.mainBlockRefs');
  const supplementBlockRefs = strings(reading.supplementBlockRefs, 'readingView.supplementBlockRefs');
  const titleBlockRefs = strings(note.titleBlockRefs, 'note.titleBlockRefs');
  const blockRefs = new Set(blocks.map(block => block.ref));
  const orderedRefs = [...mainBlockRefs, ...supplementBlockRefs];
  if (new Set(orderedRefs).size !== orderedRefs.length || orderedRefs.some(ref => !blockRefs.has(ref)) || orderedRefs.length !== blocks.length
    || new Set(titleBlockRefs).size !== titleBlockRefs.length || titleBlockRefs.some(ref => !blockRefs.has(ref))) throw new ShapeError('readingView.blockRefs');
  return {
    contract: 'research-dossier.v2', workRef: str(row, 'workRef'), revision: str(row, 'revision'), materialRevision: str(row, 'materialRevision'), asOf: str(row, 'asOf'),
    note: { platform: str(note, 'platform'), sourceUrl: optStr(note, 'sourceUrl'), platformTitle: optStr(note, 'platformTitle'),
      titleSource: titleSource === null ? null : { sourceRef: optStr(titleSource, 'sourceRef'), sourceVersion: optStr(titleSource, 'sourceVersion') },
      displayTitle: optStr(note, 'displayTitle'), displayTitleBasis: choice(note.displayTitleBasis, ['platform', 'cover', 'missing'], 'displayTitleBasis'), titleBlockRefs },
    materials, readingView: { mainBlockRefs, supplementBlockRefs }, coverage: parseCoverage(obj(row, 'coverage')), attachments: { comments: attachment('comments'), author: attachment('author'), interaction: attachment('interaction'), versions: attachment('versions') }, selection: parseSelection(obj(row, 'selection')), raw: row,
  };
}
export function parseDossier(value: unknown): Dossier { return decode(value, parseDossierRow); }
export function parseReadingVersion(value: unknown): { workRef: string; currentRevisionRef: string | null; material: DossierMaterial } {
  return decode(value, row => {
    if (row.currentRevisionRef === undefined) throw new ShapeError('currentRevisionRef');
    return { workRef: str(row, 'workRef'), currentRevisionRef: optStr(row, 'currentRevisionRef'), material: parseMaterial(obj(row, 'material')) };
  });
}
export function parseDossierPayload(value: unknown): DossierPayload {
  return decode(value, root => ({ dossier: parseDossierRow(obj(root, 'dossier')), relayerPlan: parseRelayerPlan(obj(root, 'relayerPlan')) }));
}
export function parseRelayerPlan(row: Json): RelayerPlan {
  return { workCount: optionalCount(row, 'workCount'), imageSlots: optionalCount(row, 'imageSlots'), imageJobs: optionalCount(row, 'imageJobs'), layoutCount: count(row.layoutCount, 'layoutCount'), pendingLayouts: count(row.pendingLayouts, 'pendingLayouts'), alreadyCurrent: count(row.alreadyCurrent, 'alreadyCurrent'), needsOcr: count(row.needsOcr, 'needsOcr'), needsAcquisition: count(row.needsAcquisition, 'needsAcquisition'), pendingReviewCount: count(row.pendingReviewCount, 'pendingReviewCount'), expectedRevision: str(row, 'expectedRevision') };
}
export function parseRelayerPlanResponse(value: unknown): RelayerPlan { return decode(value, root => parseRelayerPlan(obj(root, 'relayerPlan'))); }
export function parseCandidates(value: unknown): ReadingCandidate[] {
  return decode(value, root => list(root, 'candidates', row => ({ unitRef: str(row, 'unitRef'), slotKey: optStr(row, 'slotKey'), candidateRevisionRef: str(row, 'candidateRevisionRef'), currentRevisionRef: optStr(row, 'currentRevisionRef'), blocks: parseBlocks(row), issues: list(row, 'issues', parseIssue), generatedBy: choice(row.generatedBy, PRODUCERS, 'generatedBy'), verification: choice(row.verification, VERIFICATIONS, 'verification'), createdAt: str(row, 'createdAt'), review: parseReview(row) })));
}
export function parseReadingReceipt(value: unknown): ReadingReviewReceipt {
  return decode(value, row => {
    const action = choice(row.action, REVIEW_ACTIONS, 'action'); const currentRevisionRef = optStr(row, 'currentRevisionRef');
    const adoption = row.adoption === null ? null : choice(row.adoption, ADOPTIONS, 'adoption');
    const verification = row.verification === null ? null : choice(row.verification, VERIFICATIONS, 'verification');
    if (currentRevisionRef === null ? adoption !== null || verification !== null || ['adopt', 'edit', 'keep'].includes(action) : adoption === null || verification === null) throw new ShapeError('currentRevisionRef');
    return { commandRef: str(row, 'commandRef'), unitRef: str(row, 'unitRef'), candidateRevisionRef: str(row, 'candidateRevisionRef'), action, currentRevisionRef, adoption, verification, reviewRef: optStr(row, 'reviewRef') };
  });
}
export function parseDossierComments(value: unknown): DossierCommentPage {
  return decode(value, root => ({ items: list(root, 'items', row => ({ sourceRef: str(row, 'sourceRef'), parentSourceRef: optStr(row, 'parentSourceRef'), relation: choice(row.relation, ['ROOT', 'REPLY'], 'relation'), body: optStr(row, 'body'), likeCount: optionalCount(row, 'likeCount') })), total: optionalCount(root, 'total'), nextCursor: optStr(root, 'nextCursor') }));
}
export function parseRelayerReceipt(value: unknown): RelayerReceipt {
  return decode(value, root => ({ scopeCount: num(root, 'scopeCount'), updated: num(root, 'updated'), alreadyCurrent: num(root, 'alreadyCurrent'), skipped: num(root, 'skipped'), needsOcr: num(root, 'needsOcr'), needsAcquisition: count(root.needsAcquisition, 'needsAcquisition') }));
}
