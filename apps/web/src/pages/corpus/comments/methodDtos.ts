import { bool, decode, list, num, obj, optBool, optNum, optObj, optStr, str, type Json } from '../../../shared/decode';
import type { StudyPage } from './studyApi';

export interface SourcePreview {
  asOf: string | null; totalCommentCount: number | null; eligibleCommentCount: number | null; unknownAuthorCount: number | null;
  excludedCounts: Record<string, number | null>;
}
export interface StudySetup {
  domainRef: string; domainStatus: string;
  modelConfigs: { configRef: string; modelId: string; inputTokenLimit: number; outputTokenLimit: number; enabled: boolean | null }[];
  sourcePreview: SourcePreview | null; referenceSourcePreview: SourcePreview | null;
}
export interface MethodStage { systemInstruction: string | null; outputSchema: unknown }
export interface MethodManifest extends Json { stages: Partial<Record<'semantic' | 'resolution' | 'pair', MethodStage>>; modelConfigRef?: string }
export interface StudyPolicy {
  policyRef: string; methodName: string | null; parentPolicyRef: string | null; methodHash: string | null; createdAt: string | null;
  recordingState: string | null; isActive: boolean | null;
  defaults: { commentBudget: number | null } | null;
  methodManifest: MethodManifest | null; model: unknown;
}
export interface StudyWork {
  workRef: string; displayTitle: string | null; displayTitleSource: string | null; observationRole: string | null; eligibleCommentCount: number | null;
}
export interface WorkCatalog { items: StudyWork[]; totalWorkCount: number | null; page: StudyPage; indexCoverage: { indexedCount: number | null; pendingCount: number | null } | null }
export const countMap = (row: Json | null): Record<string, number | null> => Object.fromEntries(Object.keys(row ?? {}).map(key => [key, optNum(row!, key)]));
export function parseSource(row: Json): SourcePreview {
  return { asOf: optStr(row, 'asOf'), totalCommentCount: optNum(row, 'totalCommentCount'), eligibleCommentCount: optNum(row, 'eligibleCommentCount'), unknownAuthorCount: optNum(row, 'unknownAuthorCount'), excludedCounts: countMap(optObj(row, 'excludedCounts')) };
}
export function parseSetup(value: Json): StudySetup {
  return decode(value, row => ({ domainRef: str(row, 'domainRef'), domainStatus: str(row, 'domainStatus'), modelConfigs: list(row, 'modelConfigs', m => ({ configRef: str(m, 'configRef'), modelId: str(m, 'modelId'), inputTokenLimit: num(m, 'inputTokenLimit'), outputTokenLimit: num(m, 'outputTokenLimit'), enabled: optBool(m, 'enabled') })), sourcePreview: optObj(row, 'sourcePreview') ? parseSource(obj(row, 'sourcePreview')) : null, referenceSourcePreview: optObj(row, 'referenceSourcePreview') ? parseSource(obj(row, 'referenceSourcePreview')) : null }));
}
export function parsePolicy(row: Json): StudyPolicy {
  const defaults = optObj(row, 'defaults'), manifest = optObj(row, 'methodManifest');
  let methodManifest: MethodManifest | null = null;
  if (manifest) {
    const stages = optObj(manifest, 'stages');
    methodManifest = { ...manifest, stages: {} };
    for (const key of ['semantic', 'resolution', 'pair'] as const) {
      const stage = stages ? optObj(stages, key) : null;
      if (stage) methodManifest.stages[key] = { systemInstruction: optStr(stage, 'systemInstruction'), outputSchema: stage.outputSchema ?? null };
    }
    const ref = optStr(manifest, 'modelConfigRef'); if (ref) methodManifest.modelConfigRef = ref;
  }
  return { policyRef: str(row, 'policyRef'), methodName: optStr(row, 'methodName'), parentPolicyRef: optStr(row, 'parentPolicyRef'), methodHash: optStr(row, 'methodHash'), createdAt: optStr(row, 'createdAt'), recordingState: optStr(row, 'recordingState'), isActive: optBool(row, 'isActive'), defaults: defaults ? { commentBudget: optNum(defaults, 'commentBudget') } : null, methodManifest, model: row.model ?? null };
}
export const parsePolicyDetail = (row: Json) => decode(row, r => parsePolicy(obj(r, 'policy')));
export function parsePolicyPage(row: Json) {
  return decode(row, r => { const page = obj(r, 'page'); return { items: list(r, 'items', parsePolicy), page: { nextCursor: optStr(page, 'nextCursor'), hasMore: bool(page, 'hasMore') } }; });
}
export function parseWorkCatalog(row: Json): WorkCatalog {
  return decode(row, r => { const page = obj(r, 'page'), coverage = optObj(r, 'indexCoverage'); return { items: list(r, 'items', w => ({ workRef: str(w, 'workRef'), displayTitle: optStr(w, 'displayTitle'), displayTitleSource: optStr(w, 'displayTitleSource'), observationRole: optStr(w, 'observationRole'), eligibleCommentCount: optNum(w, 'eligibleCommentCount') })), totalWorkCount: optNum(r, 'totalWorkCount'), page: { nextCursor: optStr(page, 'nextCursor') }, indexCoverage: coverage ? { indexedCount: optNum(coverage, 'indexedCount'), pendingCount: optNum(coverage, 'pendingCount') } : null }; });
}
