import { isRecord, requestJson } from '../../../shared/api';
import type { AnalysisLatest, AnalysisPreview, AnalysisRead, AnalysisReceipt, AnalysisRequest, AnalysisScope } from './noteAnalysisTypes';
const ROOT = '/api/local/note-analysis';
function params(domainRef: string, workRef: string, scope: AnalysisScope) { return new URLSearchParams({ domainRef, workRef, scope }); }
function contract<T>(value: unknown, name: string): T {
  if (!isRecord(value) || value.contract !== name) throw new Error('单篇分析响应格式不正确');
  return value as T;
}
function validRead(value: unknown): value is AnalysisRead {
  if (!isRecord(value) || value.contract !== 'note-analysis.read.v1' || !isRecord(value.run) || !isRecord(value.versions) || !isRecord(value.coverage)) return false;
  if (!['pending', 'running', 'succeeded', 'failed', 'skipped', 'cancelled'].includes(String(value.run.state)) || typeof value.requestRef !== 'string') return false;
  if (value.result === null) return value.run.state !== 'succeeded';
  if (value.run.state !== 'succeeded' || !isRecord(value.result)) return false;
  const result = value.result;
  const strings = (items: unknown): items is string[] => Array.isArray(items) && items.every(item => typeof item === 'string');
  const statement = (item: unknown) => isRecord(item) && typeof item.id === 'string' && typeof item.text === 'string' && strings(item.evidenceRefs);
  if (result.contract !== 'note-analysis.result.v1' || typeof result.summary !== 'string' || !isRecord(result.understanding)) return false;
  if (!statement(result.understanding.discussion) || !statement(result.understanding.answer) || !Object.values(result.understanding).every(item => item === null || statement(item))) return false;
  if (!Array.isArray(result.features) || !result.features.every(statement) || !Array.isArray(result.feedback) || !result.feedback.every(statement)) return false;
  if (!Array.isArray(result.explanations) || !result.explanations.every(item => statement(item) && strings(item.featureIds) && strings(item.feedbackIds) && typeof item.support === 'string')) return false;
  if (!Array.isArray(result.tags) || !result.tags.every(item => isRecord(item) && typeof item.label === 'string' && typeof item.retrievalUse === 'string' && strings(item.judgmentIds))) return false;
  if (!Array.isArray(result.methodCandidates) || !result.methodCandidates.every(item => isRecord(item) && typeof item.text === 'string' && typeof item.conditions === 'string' && strings(item.basisIds))) return false;
  return strings(result.limitations) && strings(result.summaryBasisIds) && Array.isArray(result.evidence) && result.evidence.every(item => isRecord(item) && ['id', 'quote', 'sourceRef', 'sourceVersion', 'citationId'].every(key => typeof item[key] === 'string')) && strings(value.coverage.issues);
}
export async function previewAnalysis(domainRef: string, workRef: string, scope: AnalysisScope): Promise<AnalysisPreview> {
  return contract(await requestJson(`${ROOT}/preview?${params(domainRef, workRef, scope)}`), 'note-analysis.preview.v1');
}
export async function startAnalysis(body: AnalysisRequest): Promise<AnalysisReceipt> {
  const value = await requestJson(ROOT, { method: 'POST', body });
  if (!isRecord(value) || !['requestRef', 'batchRef', 'itemRef'].every(key => typeof value[key] === 'string')) throw new Error('提交回执无法确认');
  return value as unknown as AnalysisReceipt;
}
export async function readAnalysis(domainRef: string, requestRef: string, signal?: AbortSignal): Promise<AnalysisRead> {
  const value = await requestJson(`${ROOT}/${encodeURIComponent(requestRef)}?${new URLSearchParams({ domainRef })}`, signal ? { signal } : {});
  if (!validRead(value)) throw new Error('单篇分析结果格式不正确');
  return value;
}
export async function latestAnalysis(domainRef: string, workRef: string, scope: AnalysisScope, signal?: AbortSignal): Promise<AnalysisLatest> {
  const value = contract<AnalysisLatest>(await requestJson(`${ROOT}/latest?${params(domainRef, workRef, scope)}`, signal ? { signal } : {}), 'note-analysis.latest.v1');
  if (!(value.latestRun === null || validRead(value.latestRun)) || !(value.latestSuccessful === null || (validRead(value.latestSuccessful) && value.latestSuccessful.run.state === 'succeeded'))) throw new Error('单篇分析记录格式不正确');
  return value;
}

const PREFIX = 'linggan.note-analysis.v1:';
/** Only frozen request identity/parameters are retained; no source text or model output. */
export const analysisRequests = {
  get(key: string): AnalysisRequest | null {
    const raw = sessionStorage.getItem(PREFIX + key);
    if (raw === null) return null;
    const value: unknown = JSON.parse(raw);
    if (!isRecord(value) || !['commandRef', 'domainRef', 'workRef', 'knowledgeCutoff', 'inputHash', 'taskConfigRef'].every(field => typeof value[field] === 'string') || typeof value.taskRevision !== 'number' || !['note', 'note_with_comments'].includes(String(value.scope)) || `${value.domainRef}:${value.workRef}:${value.scope}` !== key) throw new Error('原请求身份无法读取');
    return value as unknown as AnalysisRequest;
  },
  set(key: string, value: AnalysisRequest) { sessionStorage.setItem(PREFIX + key, JSON.stringify(value)); },
  delete(key: string) { sessionStorage.removeItem(PREFIX + key); },
};
