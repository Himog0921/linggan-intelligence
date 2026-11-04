import { exportFileName } from './dossierPresent';
import { fetchDossierMarkdown } from './dossierApi';
import type { DossierBlock, DossierPayload } from './dossierTypes';
export interface DossierFile { name: string; type: string; data: string }
/** Content-only export. Immutable provenance and diagnostics stay in the source dossier. */
function exportBlock(block: DossierBlock) {
  const content = Object.fromEntries(Object.entries(block).filter(([key]) => !['ref', 'issues', 'bbox', 'regionRef', 'parameters'].includes(key)));
  return block.parameters?.length ? { ...content, parameterCandidates: block.parameters.map(parameter => Object.fromEntries(Object.entries(parameter).filter(([key]) => key !== 'sourceRegionRef'))) } : content;
}
export function dossierExport(payload: DossierPayload): DossierFile {
  const { dossier } = payload;
  const included = new Set(dossier.readingView.sections.flatMap(section => section.groups.flatMap(group => group.blockRefs)));
  const data = {
    title: dossier.note.displayTitle,
    sourceUrl: dossier.note.sourceUrl,
    ...(dossier.selection.mode === 'explicit' ? { scope: 'selected' } : {}),
    materials: dossier.materials.map(unit => ({
      kind: unit.kind,
      ...(unit.originalUrl ? { originalUrl: unit.originalUrl } : {}),
      ...(unit.blocks.length === 0 ? { state: unit.sourceState, ...(unit.processing ? { processing: unit.processing.state } : {}) } : {}),
      content: unit.blocks.filter(block => included.has(block.ref)).map(exportBlock),
    })),
  };
  return { name: exportFileName(dossier.workRef, 'json'), type: 'application/json;charset=utf-8', data: JSON.stringify(data, null, 2) };
}
export function downloadFile(file: DossierFile): void {
  const url = URL.createObjectURL(new Blob([file.data], { type: file.type }));
  const link = document.createElement('a'); link.href = url; link.download = file.name; link.click();
  window.setTimeout(() => URL.revokeObjectURL(url), 1000);
}
/** The cutoff, selection and revision pin Markdown to the JSON snapshot in this export session. */
export async function dossierMarkdownExport(payload: DossierPayload, domain: string, signal?: AbortSignal): Promise<DossierFile> {
  const data = await fetchDossierMarkdown(payload.dossier.workRef, domain, payload.dossier, signal);
  return { name: exportFileName(payload.dossier.workRef, 'md'), type: 'text/markdown;charset=utf-8', data };
}
