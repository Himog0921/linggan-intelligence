import { requestJson, requestText } from '../../../shared/api';
import { parseDossierComments, parseDossierPayload, parseReadingReceipt } from './dossierDecode';
import type { Dossier, DossierCommentPage, DossierPayload, ReadingReviewRequest } from './dossierTypes';
import { API_ROOT } from './evidencePaths';

function base(work: string): string { return `${API_ROOT}/${encodeURIComponent(work)}/research-dossier`; }
function scope(domain: string): string { return `domain=${encodeURIComponent(domain)}`; }
export async function fetchDossier(work: string, domain: string, signal?: AbortSignal): Promise<DossierPayload> {
  return parseDossierPayload(await requestJson(`${base(work)}?${scope(domain)}`, { signal }));
}
export async function fetchDossierMarkdown(work: string, domain: string, snapshot: Pick<Dossier, 'asOf' | 'revision' | 'selection'>, signal?: AbortSignal): Promise<string> {
  const params = new URLSearchParams({ domain, format: 'markdown', asOf: snapshot.asOf, expectedRevision: snapshot.revision });
  if (snapshot.selection.mode === 'explicit') {
    for (const field of ['kinds', 'unitRefs', 'blockRefs', 'imagePages'] as const) {
      const values = snapshot.selection[field];
      if (values.length) params.set(field, values.join(','));
    }
  }
  return requestText(`${base(work)}?${params}`, { signal });
}
export async function reviewReading(work: string, domain: string, unit: string, command: ReadingReviewRequest) {
  return parseReadingReceipt(await requestJson(`${base(work)}/materials/${encodeURIComponent(unit)}/review?${scope(domain)}`, { method: 'POST', body: command }));
}
export async function fetchDossierComments(work: string, domain: string, cursor: string | null, signal?: AbortSignal): Promise<DossierCommentPage> {
  return parseDossierComments(await requestJson(`${base(work)}/comments?${scope(domain)}${cursor === null ? '' : `&cursor=${encodeURIComponent(cursor)}`}`, { signal }));
}
