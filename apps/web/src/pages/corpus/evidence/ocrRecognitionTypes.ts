import { ApiError, isRecord } from '../../../shared/api';

export type OcrRecognitionState = 'unavailable' | 'ready' | 'queued' | 'running' | 'finalizing' | 'succeeded' | 'stopped' | 'unknown';
export type OcrTextStatus = 'adopted' | 'pending' | 'excluded';
export interface OcrRecognitionSpan {
  /** Half-open Unicode code-point offsets within immutable region text. */
  start: number; end: number; status: OcrTextStatus; reason: string | null;
  comparisonText?: string | null; comparisonRef?: string | null;
}
export interface OcrRecognitionRegion {
  regionRef: string; text: string; role: string; status: OcrTextStatus; reason: string | null;
  sourceStart: number; sourceEnd: number; bbox: readonly number[] | null; spans: readonly OcrRecognitionSpan[];
}
export interface OcrRecognitionImage {
  imageRef: string; ordinal: number | null; role: 'cover' | 'page' | 'unknown';
  state: 'pending' | 'running' | 'succeeded' | 'failed' | 'unattempted' | 'unknown';
  slotKey: string | null; aliases: readonly string[]; originalUrl: string | null;
  rawResultRef: string | null; rawResultSha256: string | null; rawJson: string | null;
  invocationRef: string | null; finishReason: string | null;
  usage: { inputTokens: number | null; outputTokens: number | null; measuredTokens: number | null } | null;
  estimatedCostUsd: string | null; regions: readonly OcrRecognitionRegion[];
  adoptionRef?: string | null; reviewRevision?: number; machineText?: string | null;
  humanBody?: { reviewRef: string; revision: number; text: string; actor: string; reason: string; reviewedAt: string } | null;
}
export interface OcrRecognitionScope {
  sha256: string; imageCount: number; modelLabel: string; endpointUrl: string;
}
/** Counts are persisted transport facts. User-facing task progress exposes percentages only. */
export interface OcrRecognitionRun {
  runRef: string | null; workRef: string; state: OcrRecognitionState; completedImages: number; totalImages: number;
  currentStage: string; modelLabel: string | null; unavailableReason: string | null;
  coverTitle: string | null; coverTitleStatus: 'adopted' | 'pending' | 'absent' | null;
  images: readonly OcrRecognitionImage[]; pendingCount: number; failureCode: string | null;
  batchRef: string | null; batchRevision: number | null; cancelRequested: boolean; scope: OcrRecognitionScope | null;
}
function invalid(): never { throw new ApiError({ kind: 'invalid_response', status: 200 }); }
function record(value: unknown): Record<string, unknown> { return isRecord(value) ? value : invalid(); }
function string(value: unknown): string { return typeof value === 'string' ? value : invalid(); }
function optionalString(value: unknown): string | null { return value === null || value === undefined ? null : string(value); }
function integer(value: unknown): number { return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0 ? value : invalid(); }
function optionalInteger(value: unknown): number | null { return value === null || value === undefined ? null : integer(value); }
function array<T>(value: unknown, decode: (entry: unknown) => T): T[] { return Array.isArray(value) ? value.map(decode) : invalid(); }
function choice<const T extends string>(value: unknown, values: readonly T[]): T { return values.find(entry => entry === value) ?? invalid(); }
function localAsset(value: unknown): string | null {
  const path = optionalString(value);
  return path === null || (path.startsWith('/api/local/') && !/[\\\s]/.test(path)) ? path : invalid();
}
function region(value: unknown): OcrRecognitionRegion {
  const row = record(value); const text = string(row.text); const length = Array.from(text).length;
  const spans = row.spans === undefined ? [] : array(row.spans, value => {
    const span = record(value); const start = integer(span.start); const end = integer(span.end);
    if (end <= start || end > length) return invalid();
    return { start, end, status: choice(span.status, ['adopted', 'pending', 'excluded']), reason: optionalString(span.reason), comparisonText: optionalString(span.comparisonText), comparisonRef: optionalString(span.comparisonRef) };
  });
  if (spans.some((span, index) => index > 0 && span.start < (spans[index - 1]?.end ?? 0))) return invalid();
  const bbox = row.bbox === null ? null : array(row.bbox, entry => typeof entry === 'number' && Number.isFinite(entry) && entry >= 0 && entry <= 1 ? entry : invalid());
  if (bbox && (bbox.length !== 4 || bbox[0]! >= bbox[2]! || bbox[1]! >= bbox[3]!)) return invalid();
  return { regionRef: string(row.regionRef), text, role: string(row.role), status: choice(row.status, ['adopted', 'pending', 'excluded']), reason: optionalString(row.reason),
    sourceStart: integer(row.sourceStart), sourceEnd: integer(row.sourceEnd), bbox, spans };
}
function image(value: unknown): OcrRecognitionImage {
  const row = record(value); const usage = row.usage === undefined || row.usage === null ? null : record(row.usage);
  return { imageRef: string(row.imageRef), ordinal: optionalInteger(row.ordinal), role: choice(row.role, ['cover', 'page', 'unknown']),
    state: choice(row.state, ['pending', 'running', 'succeeded', 'failed', 'unattempted', 'unknown']), slotKey: optionalString(row.slotKey), aliases: row.aliases === undefined ? [] : array(row.aliases, string),
    originalUrl: localAsset(row.originalUrl), rawResultRef: optionalString(row.rawResultRef), rawResultSha256: optionalString(row.rawResultSha256), rawJson: optionalString(row.rawJson),
    invocationRef: optionalString(row.invocationRef), usage: usage ? { inputTokens: optionalInteger(usage.inputTokens), outputTokens: optionalInteger(usage.outputTokens), measuredTokens: optionalInteger(usage.measuredTokens) } : null,
    estimatedCostUsd: optionalString(row.estimatedCostUsd), finishReason: optionalString(row.finishReason), regions: array(row.regions, region),
    adoptionRef: optionalString(row.adoptionRef), reviewRevision: row.reviewRevision === undefined ? 0 : integer(row.reviewRevision), machineText: optionalString(row.machineText),
    humanBody: row.humanBody === undefined || row.humanBody === null ? null : (() => { const review = record(row.humanBody); return { reviewRef: string(review.reviewRef), revision: integer(review.revision), text: string(review.text), actor: string(review.actor), reason: string(review.reason), reviewedAt: string(review.reviewedAt) }; })() };
}
export function parseOcrRecognition(value: unknown): OcrRecognitionRun | null {
  if (value === null) return null;
  const row = record(value); const scope = row.scope === undefined || row.scope === null ? null : record(row.scope);
  const completedImages = integer(row.completedImages); const totalImages = integer(row.totalImages);
  if (completedImages > totalImages || (row.cancelRequested !== undefined && typeof row.cancelRequested !== 'boolean')) return invalid();
  return { runRef: optionalString(row.runRef), workRef: string(row.workRef), state: choice(row.state, ['unavailable', 'ready', 'queued', 'running', 'finalizing', 'succeeded', 'stopped', 'unknown']),
    completedImages, totalImages, currentStage: string(row.currentStage), modelLabel: optionalString(row.modelLabel), unavailableReason: optionalString(row.unavailableReason),
    coverTitle: optionalString(row.coverTitle), coverTitleStatus: row.coverTitleStatus === null ? null : choice(row.coverTitleStatus, ['adopted', 'pending', 'absent']),
    images: array(row.images, image), pendingCount: integer(row.pendingCount), failureCode: optionalString(row.failureCode), batchRef: optionalString(row.batchRef), batchRevision: optionalInteger(row.batchRevision),
    cancelRequested: row.cancelRequested === true,
    scope: scope ? { sha256: string(scope.sha256), imageCount: integer(scope.imageCount), modelLabel: string(scope.modelLabel), endpointUrl: string(scope.endpointUrl) } : null };
}
