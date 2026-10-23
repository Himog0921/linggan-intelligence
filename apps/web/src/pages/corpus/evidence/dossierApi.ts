import { ApiError, requestJson, requestText } from '../../../shared/api';
import { parseCandidates, parseDossierComments, parseDossierPayload, parseReadingReceipt, parseReadingVersion, parseRelayerPlanResponse, parseRelayerReceipt } from './dossierDecode';
import type { DossierCommentPage, DossierPayload, ReadingReviewRequest, RelayerPlan, RelayerReceipt } from './dossierTypes';
import { API_ROOT } from './evidencePaths';

function base(work: string): string { return `${API_ROOT}/${encodeURIComponent(work)}/research-dossier`; }
function scope(domain: string): string { return `domain=${encodeURIComponent(domain)}`; }
export async function fetchDossier(work: string, domain: string, signal?: AbortSignal): Promise<DossierPayload> {
  return parseDossierPayload(await requestJson(`${base(work)}?${scope(domain)}`, { signal }));
}
export async function fetchDossierMarkdown(work: string, domain: string, asOf: string, signal?: AbortSignal): Promise<string> {
  return requestText(`${base(work)}?${scope(domain)}&format=markdown&asOf=${encodeURIComponent(asOf)}`, { signal });
}
export async function fetchRelayerPlan(work: string, domain: string, signal?: AbortSignal): Promise<RelayerPlan> {
  return parseRelayerPlanResponse(await requestJson(`${base(work)}/relayer-plan?${scope(domain)}`, { signal }));
}
export async function fetchCandidates(work: string, domain: string, signal?: AbortSignal) {
  return parseCandidates(await requestJson(`${base(work)}/candidates?${scope(domain)}`, { signal }));
}
export async function fetchReadingVersion(work: string, domain: string, reading: string, signal?: AbortSignal) {
  const value = parseReadingVersion(await requestJson(`${base(work)}/readings/${encodeURIComponent(reading)}?${scope(domain)}`, { signal }));
  if (value.workRef !== work || value.material.currentRevisionRef !== reading) throw new ApiError({ kind: 'invalid_response', status: 200 });
  return value;
}
export async function reviewReading(work: string, domain: string, unit: string, command: ReadingReviewRequest) {
  return parseReadingReceipt(await requestJson(`${base(work)}/materials/${encodeURIComponent(unit)}/review?${scope(domain)}`, { method: 'POST', body: command }));
}
export async function fetchDossierComments(work: string, domain: string, cursor: string | null, signal?: AbortSignal): Promise<DossierCommentPage> {
  return parseDossierComments(await requestJson(`${base(work)}/comments?${scope(domain)}${cursor === null ? '' : `&cursor=${encodeURIComponent(cursor)}`}`, { signal }));
}
export async function relayerDossier(work: string, domain: string, expectedRevision: string): Promise<RelayerReceipt> {
  return parseRelayerReceipt(await requestJson(`${base(work)}/relayer`, { method: 'POST', body: { expectedRevision, domain } }));
}
