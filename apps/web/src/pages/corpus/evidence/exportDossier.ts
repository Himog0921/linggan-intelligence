import { exportFileName } from './dossierPresent';
import { fetchDossierMarkdown } from './dossierApi';
import type { DossierPayload } from './dossierTypes';
export interface DossierFile { name: string; type: string; data: string }
/** JSON is the received snapshot, without UI-only fields or a second text copy. */
export function dossierExport(payload: DossierPayload): DossierFile {
  return { name: exportFileName(payload.dossier.workRef, 'json'), type: 'application/json;charset=utf-8', data: JSON.stringify(payload.dossier.raw, null, 2) };
}
export function downloadFile(file: DossierFile): void {
  const url = URL.createObjectURL(new Blob([file.data], { type: file.type }));
  const link = document.createElement('a'); link.href = url; link.download = file.name; link.click();
  window.setTimeout(() => URL.revokeObjectURL(url), 1000);
}
export function downloadDossier(payload: DossierPayload): void { downloadFile(dossierExport(payload)); }
/** The snapshot cutoff keeps Markdown consistent with the JSON the user is viewing. */
export async function downloadDossierMarkdown(payload: DossierPayload, domain: string): Promise<void> {
  const data = await fetchDossierMarkdown(payload.dossier.workRef, domain, payload.dossier.asOf);
  downloadFile({ name: exportFileName(payload.dossier.workRef, 'md'), type: 'text/markdown;charset=utf-8', data });
}
