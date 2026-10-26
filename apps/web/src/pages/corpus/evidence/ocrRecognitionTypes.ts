import { ApiError, isRecord } from '../../../shared/api';
import { list } from '../../../shared/decode';
import { parseBlocks, parseIssue } from './dossierBlockDecode';
import { mediaPath } from './evidencePaths';
import type { DossierBlock, DossierIssue } from './dossierTypes';
export type OcrRecognitionState = 'unavailable' | 'ready' | 'queued' | 'running' | 'finalizing' | 'succeeded' | 'stopped' | 'unknown';
export interface OcrRecognitionImage {
  imageRef: string; ordinal: number | null; role: 'cover' | 'page' | 'unknown';
  state: 'pending' | 'running' | 'succeeded' | 'failed' | 'unattempted' | 'unknown';
  slotKey: string | null; aliases: readonly string[]; originalUrl: string | null;
  readingRef: string | null; isCurrent: boolean; blocks: DossierBlock[]; issues: DossierIssue[];
  rawResultRef: string | null; rawResultSha256: string | null; rawJson: string | null;
  invocationRef: string | null; finishReason: string | null;
  usage: { inputTokens: number | null; outputTokens: number | null; measuredTokens: number | null } | null;
  estimatedCostUsd: string | null;
}
export interface OcrRecognitionScope {
  sha256: string; imageCount: number; modelLabel: string; endpointUrl: string;
  images: { slotKey: string; ordinal: number | null; role: 'cover' | 'page' | 'unknown'; originalUrl: string | null }[];
}
export interface OcrRecognitionRun {
  runRef: string | null; workRef: string; state: OcrRecognitionState; completedImages: number; totalImages: number;
  currentStage: string; modelLabel: string | null; unavailableReason: string | null;
  images: readonly OcrRecognitionImage[]; pendingCount: number; failureCode: string | null;
  batchRef: string | null; batchRevision: number | null; cancelRequested: boolean; scope: OcrRecognitionScope | null;
}
function invalid(): never { throw new ApiError({ kind: 'invalid_response', status: 200 }); }
function record(value: unknown): Record<string, unknown> { return isRecord(value) ? value : invalid(); }
function string(value: unknown): string { return typeof value === 'string' ? value : invalid(); }
function optionalString(value: unknown): string | null { return value === null || value === undefined ? null : string(value); }
function integer(value: unknown): number { return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0 ? value : invalid(); }
function optionalInteger(value: unknown): number | null { return value === null || value === undefined ? null : integer(value); }
function array<T>(value: unknown, parse: (entry: unknown) => T): T[] { return Array.isArray(value) ? value.map(parse) : invalid(); }
function choice<const T extends string>(value: unknown, values: readonly T[]): T { return values.find(entry => entry === value) ?? invalid(); }
function image(value: unknown): OcrRecognitionImage {
  const row = record(value); const usage = row.usage === undefined || row.usage === null ? null : record(row.usage);
  if ('regions' in row || 'candidateRevisionRef' in row || typeof row.isCurrent !== 'boolean' || (row.isCurrent && (typeof row.readingRef !== 'string' || !row.readingRef))) return invalid();
  return { imageRef: string(row.imageRef), ordinal: optionalInteger(row.ordinal), role: choice(row.role, ['cover','page','unknown']), state: choice(row.state, ['pending','running','succeeded','failed','unattempted','unknown']), slotKey: optionalString(row.slotKey), aliases: array(row.aliases, string), originalUrl: mediaPath(row.originalUrl),
    readingRef: optionalString(row.readingRef), isCurrent: row.isCurrent, blocks: parseBlocks(row), issues: list(row, 'issues', parseIssue),
    rawResultRef: optionalString(row.rawResultRef), rawResultSha256: optionalString(row.rawResultSha256), rawJson: optionalString(row.rawJson), invocationRef: optionalString(row.invocationRef), finishReason: optionalString(row.finishReason), estimatedCostUsd: optionalString(row.estimatedCostUsd),
    usage: usage ? { inputTokens: optionalInteger(usage.inputTokens), outputTokens: optionalInteger(usage.outputTokens), measuredTokens: optionalInteger(usage.measuredTokens) } : null };
}
export function parseOcrRecognition(value: unknown): OcrRecognitionRun | null {
  if (value === null) return null;
  const row = record(value); const scope = row.scope === null || row.scope === undefined ? null : record(row.scope);
  const completedImages = integer(row.completedImages); const totalImages = integer(row.totalImages);
  if (completedImages > totalImages || typeof row.cancelRequested !== 'boolean' || 'coverTitleStatus' in row) return invalid();
  const images = scope ? array(scope.images, value => { const item = record(value); return { slotKey: string(item.slotKey), ordinal: optionalInteger(item.ordinal), role: choice(item.role, ['cover','page','unknown']), originalUrl: mediaPath(item.originalUrl) }; }) : [];
  if (scope && (integer(scope.imageCount) !== images.length || new Set(images.map(image => image.slotKey)).size !== images.length)) return invalid();
  return { runRef: optionalString(row.runRef), workRef: string(row.workRef), state: choice(row.state, ['unavailable','ready','queued','running','finalizing','succeeded','stopped','unknown']), completedImages, totalImages,
    currentStage: string(row.currentStage), modelLabel: optionalString(row.modelLabel), unavailableReason: optionalString(row.unavailableReason), images: array(row.images, image), pendingCount: integer(row.pendingCount), failureCode: optionalString(row.failureCode), batchRef: optionalString(row.batchRef), batchRevision: optionalInteger(row.batchRevision), cancelRequested: row.cancelRequested,
    scope: scope ? { sha256: string(scope.sha256), imageCount: integer(scope.imageCount), modelLabel: string(scope.modelLabel), endpointUrl: string(scope.endpointUrl), images } : null };
}
