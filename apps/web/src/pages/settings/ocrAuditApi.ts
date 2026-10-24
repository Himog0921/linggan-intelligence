import { requestJson, isRecord, ApiError } from '../../shared/api';
import { object, list, text, integer, nullable, optional, bool, json } from './aiSettingsModel';
const settings = object({ automatic: bool, domainRef: nullable(text), randomCount: integer, riskCount: integer, regressionRefs: list(text), privacyClearedRefs: list(text), privacyClearanceActor: text, privacyClearanceReason: text, regressionCount: integer, concurrency: integer, timeoutSeconds: integer });
export type AuditSettings = ReturnType<typeof settings>;
const config = object({ revision: integer, settings, schedule: object({ weekday: text, time: text, timezone: text }), automaticCorrection: bool });
const pair = object({ numerator: integer, denominator: integer, rate: nullable(integer) });
const metrics = object({ titleFalseAdoption: pair, titleMissing: pair, bodyMissing: pair, hallucinatedText: pair, disputedRecovery: pair });
const pool = object({ pool: text, selected: integer, withheld: integer, humanReviewedSamples: integer, pendingDifferences: integer, modelDifferences: integer, humanReviewedAgreements: integer, humanReviewedDisagreements: integer, modelCalls: integer, failures: integer, inputTokens: nullable(integer), outputTokens: nullable(integer), reservedTokens: integer, estimatedCostPicoUsd: nullable(text), metrics });
const inventory = object({ eligible: integer, selected: integer, skippedRestrictedOrRetired: integer, skippedPrivacyUnreviewed: integer, seed: text, filterVersion: text, candidates: optional(list(object({ readingRef: text, workRef: text, blobSha256: text, risk: bool }))), randomPopulation: optional(text), requested: optional(object({ random: integer, risk: integer, regression: integer })) });
const run = object({ runRef: text, kind: text, runKey: text, createdAt: text, inventory, domainRef: nullable(text), modelId: text, promptVersion: text, settings: json, reviewVersions: list(json), pools: list(pool), samples: list(object({ sampleRef: text, workRef: nullable(text), pool: text, readingRef: nullable(text), observed: bool, visionQualified: nullable(bool), differenceCount: integer, failureCode: nullable(text) })), fourWeekRuns: list(json), truthBasis: text, costBasis: text });
const ROOT = '/api/local/ocr-audit';
export async function fetchAuditSettings(signal?: AbortSignal) { return config(await requestJson(ROOT, { signal })); }
export async function fetchAuditRuns(signal?: AbortSignal) { return object({ runs: list(run) })(await requestJson(`${ROOT}/runs`, { signal })); }
export async function saveAuditSettings(payload: object) { return object({ revision: integer, saved: bool, runStarted: bool })(await requestJson(`${ROOT}/settings`, { method: 'POST', body: payload })); }
export async function startAuditRun(visualProbe: boolean, commandRef: string) {
  const result = await requestJson(`${ROOT}/runs`, { method: 'POST', body: { commandRef, runKey: `${visualProbe ? 'probe' : 'manual'}:${commandRef}`, seed: commandRef, visualProbe } });
  if (!isRecord(result) || typeof result.runRef !== 'string') throw new ApiError({ kind: 'invalid_response', status: 200 });
  return result.runRef;
}
export interface AuditDetail {
  humanCounts: Record<string, { numerator: number; denominator: number }>;
  sampleRef: string | null; originalAssetUrl: string | null; modelId: string | null; modelOpinion: unknown;
  issues: { issueRef: string; suggestion: unknown }[]; revision: number;
  reviews: { revision: number; decision: 'confirmed' | 'dismissed' | 'pending'; actor: string; reason: string; materialReviewCommandRef: string | null }[];
}
const metricCounts = ['titleFalseAdoption', 'titleMissing', 'bodyMissing', 'hallucinatedText', 'disputedRecovery'];
export async function fetchAuditDetail(readingRef: string, signal?: AbortSignal): Promise<AuditDetail> {
  const value = await requestJson(`${ROOT}/readings/${encodeURIComponent(readingRef)}`, { signal });
  if (!isRecord(value) || (value.sampleRef !== null && typeof value.sampleRef !== 'string') || !Array.isArray(value.reviews) || !Array.isArray(value.issues)) throw new ApiError({ kind: 'invalid_response', status: 200 });
  const reviews = value.reviews.map(raw => {
    if (!isRecord(raw) || !['confirmed','dismissed','pending'].includes(String(raw.decision)) || typeof raw.revision !== 'number' || typeof raw.actor !== 'string' || typeof raw.reason !== 'string') throw new ApiError({ kind: 'invalid_response', status: 200 });
    return { revision: raw.revision, decision: raw.decision as 'confirmed' | 'dismissed' | 'pending', actor: raw.actor, reason: raw.reason, materialReviewCommandRef: typeof raw.material_review_command_ref === 'string' ? raw.material_review_command_ref : null };
  });
  const reviewed = value.reviews.filter(isRecord).at(-1);
  const counts = reviewed?.decision !== 'pending' && isRecord(reviewed?.metrics) ? reviewed.metrics : {};
  const humanCounts = Object.fromEntries(metricCounts.map(key => {
    const pair = counts[key]; return [key, isRecord(pair) && typeof pair.numerator === 'number' && typeof pair.denominator === 'number' ? { numerator: pair.numerator, denominator: pair.denominator } : { numerator: 0, denominator: 0 }];
  }));
  return { humanCounts, sampleRef: value.sampleRef, originalAssetUrl: typeof value.originalAssetUrl === 'string' ? value.originalAssetUrl : null, modelId: typeof value.modelId === 'string' ? value.modelId : null, modelOpinion: value.modelOpinion,
    issues: value.issues.map(raw => { if (!isRecord(raw) || typeof raw.issue_ref !== 'string') throw new ApiError({ kind: 'invalid_response', status: 200 }); return { issueRef: raw.issue_ref, suggestion: raw.suggestion }; }),
    reviews, revision: Math.max(0, ...reviews.map(r => r.revision)) };
}
export interface AuditReviewRequest {
  commandRef: string; sampleRef: string; issueRef: string | null; expectedRevision: number;
  decision: 'confirmed' | 'dismissed' | 'pending'; materialReviewCommandRef: string | null;
  actor: string; reason: string; metrics: Record<string, { numerator: number; denominator: number }>;
}
export async function reviewAudit(payload: AuditReviewRequest) {
  const value = await requestJson(`${ROOT}/reviews`, { method: 'POST', body: payload });
  if (!isRecord(value) || typeof value.reviewRef !== 'string' || typeof value.revision !== 'number' || typeof value.readingRef !== 'string' || !['confirmed','dismissed','pending'].includes(String(value.decision))) throw new ApiError({ kind: 'invalid_response', status: 200 });
  return { reviewRef: value.reviewRef, readingRef: value.readingRef, revision: value.revision, decision: String(value.decision), materialReviewCommandRef: typeof value.materialReviewCommandRef === 'string' ? value.materialReviewCommandRef : null };
}
