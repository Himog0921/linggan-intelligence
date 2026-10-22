import { requestJson, isRecord, ApiError } from '../../shared/api';
import { object, list, text, integer, nullable, optional, bool, json } from './aiSettingsModel';
const settings = object({ automatic: bool, domainRef: nullable(text), randomCount: integer, riskCount: integer, regressionRefs: list(text), privacyClearedRefs: list(text), privacyClearanceActor: text, privacyClearanceReason: text, regressionCount: integer, maxRequests: integer, maxTokens: integer, maxCostMicroUsd: integer, concurrency: integer, timeoutSeconds: integer });
export type AuditSettings = ReturnType<typeof settings>;
const config = object({ revision: integer, settings, schedule: object({ weekday: text, time: text, timezone: text }), automaticCorrection: bool });
const pair = object({ numerator: integer, denominator: integer, rate: nullable(integer) });
const metrics = object({ titleFalseAdoption: pair, titleMissing: pair, bodyMissing: pair, hallucinatedText: pair, disputedRecovery: pair });
const pool = object({ pool: text, selected: integer, withheld: integer, humanReviewedSamples: integer, pendingDifferences: integer, modelDifferences: integer, humanReviewedAgreements: integer, humanReviewedDisagreements: integer, modelCalls: integer, failures: integer, inputTokens: nullable(integer), outputTokens: nullable(integer), reservedTokens: integer, estimatedCostPicoUsd: nullable(text), metrics });
const inventory = object({ eligible: integer, selected: integer, skippedRestrictedOrRetired: integer, skippedPrivacyUnreviewed: integer, seed: text, filterVersion: text, candidates: optional(list(object({ adoptionRef: text, workRef: text, blobSha256: text, risk: bool }))), randomPopulation: optional(text), requested: optional(object({ random: integer, risk: integer, regression: integer })) });
const run = object({ runRef: text, kind: text, runKey: text, createdAt: text, inventory, domainRef: nullable(text), modelId: text, promptVersion: text, settings: json, reviewVersions: list(json), pools: list(pool), samples: list(object({ sampleRef: text, workRef: nullable(text), pool: text, adoptionRef: nullable(text), observed: bool, visionQualified: nullable(bool), differenceCount: integer, failureCode: nullable(text) })), fourWeekRuns: list(json), truthBasis: text, costBasis: text });
const ROOT = '/api/local/ocr-audit';
export async function fetchAuditSettings(signal?: AbortSignal) { return config(await requestJson(ROOT, { signal })); }
export async function fetchAuditRuns(signal?: AbortSignal) { return object({ runs: list(run) })(await requestJson(`${ROOT}/runs`, { signal })); }
export async function saveAuditSettings(payload: object) { return object({ revision: integer, saved: bool, runStarted: bool })(await requestJson(`${ROOT}/settings`, { method: 'POST', body: payload })); }
export async function startAuditRun(visualProbe: boolean, commandRef: string) {
  const result = await requestJson(`${ROOT}/runs`, { method: 'POST', body: { commandRef, runKey: `${visualProbe ? 'probe' : 'manual'}:${commandRef}`, seed: commandRef, visualProbe } });
  if (!isRecord(result) || typeof result.runRef !== 'string') throw new ApiError({ kind: 'invalid_response', status: 200 });
  return result.runRef;
}
export interface AuditDetail { humanCounts: Record<string, { numerator: number; denominator: number }>; sampleRef: string | null; originalAssetUrl: string | null; modelId: string | null; modelOpinion: unknown; issues: { issueRef: string; suggestion: unknown }[]; revision: number; reviews: { revision: number; decision: string; actor: string; reason: string; beforeText: string; afterText: string | null; sourceKind: string | null }[]; }
export async function fetchAuditDetail(adoptionRef: string, signal?: AbortSignal): Promise<AuditDetail> {
  const value = await requestJson(`${ROOT}/adoptions/${encodeURIComponent(adoptionRef)}`, { signal });
  if (!isRecord(value) || (value.sampleRef !== null && typeof value.sampleRef !== 'string')) throw new ApiError({ kind: 'invalid_response', status: 200 });
  const reviewed = Array.isArray(value.reviews) ? value.reviews.filter(isRecord).filter(r => r.decision !== 'pending').at(-1) : undefined;
  const counts = isRecord(reviewed?.metrics) ? reviewed.metrics : {};
  const humanCounts = Object.fromEntries(['titleFalseAdoption', 'titleMissing', 'bodyMissing', 'hallucinatedText', 'disputedRecovery'].map(key => {
    const pair = counts[key]; return [key, isRecord(pair) && typeof pair.numerator === 'number' && typeof pair.denominator === 'number' ? { numerator: pair.numerator, denominator: pair.denominator } : { numerator: 0, denominator: 0 }];
  }));
  return { humanCounts, sampleRef: value.sampleRef, originalAssetUrl: typeof value.originalAssetUrl === 'string' ? value.originalAssetUrl : null, modelId: typeof value.modelId === 'string' ? value.modelId : null,
    modelOpinion: value.modelOpinion, issues: Array.isArray(value.issues) ? value.issues.filter(isRecord).map(i => ({ issueRef: typeof i.issue_ref === 'string' ? i.issue_ref : '', suggestion: i.suggestion })) : [],
    reviews: Array.isArray(value.reviews) ? value.reviews.filter(isRecord).map(r => ({ revision: typeof r.revision === 'number' ? r.revision : 0, decision: typeof r.decision === 'string' ? r.decision : 'pending', actor: typeof r.actor === 'string' ? r.actor : '审核人未提供', reason: typeof r.reason === 'string' ? r.reason : '依据未提供', beforeText: typeof r.before_text === 'string' ? r.before_text : '', afterText: typeof r.after_text === 'string' ? r.after_text : null, sourceKind: typeof r.source_kind === 'string' ? r.source_kind : null })) : [],
    revision: Array.isArray(value.reviews) ? Math.max(0, ...value.reviews.filter(isRecord).map(r => typeof r.revision === 'number' ? r.revision : 0)) : 0 };
}
export async function reviewAudit(payload: object) {
  const value = await requestJson(`${ROOT}/reviews`, { method: 'POST', body: payload });
  if (!isRecord(value) || typeof value.reviewRef !== 'string' || typeof value.revision !== 'number') throw new ApiError({ kind: 'invalid_response', status: 200 });
  return { reviewRef: value.reviewRef, revision: value.revision, sourceKind: typeof value.sourceKind === 'string' ? value.sourceKind : null };
}
