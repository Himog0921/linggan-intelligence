import { ApiError, requestJson } from '../../../shared/api';
import { ShapeError, bool, decode, field, num, obj, optObj, optStr, str, type Json } from '../../../shared/decode';
import { stepNames, type Step } from '../workbenchModel';

export function readTemplateLabels(name: string, tags: string): TemplateLabels {
  return { name: name.trim(), tags: tags.split(/[,，]/).map(tag => tag.trim()).filter(Boolean) };
}

export function validTemplateLabels({ name, tags }: TemplateLabels): boolean {
  return Array.from(name).length <= 60 && tags.length <= 8
    && tags.every(tag => Array.from(tag).length <= 24);
}

export type TemplateLabels = { name: string; tags: string[] };
export type Template = { code: string; revision: number; sourceWorkId: string | null; sourceRevisionId: string | null; evidenceSource?: { workRef: string; materialRevision: string }; sourceAvailable: boolean; status: 'review' | 'ok' | 'off'; steps: Step[]; labels?: TemplateLabels; createdAt: string };
export type CreateCommand = { requestId: string; workId: string; expectedRevision: string; steps: Step[]; labels?: TemplateLabels };
export type EvidenceTemplateCommand = { requestId: string; workRef: string; domainRef: string; materialRevision: string; steps: Step[] };
export function rejectedTemplateCommand(error: unknown): error is ApiError {
  return error instanceof ApiError && error.failure.kind === 'http' && ['creation_assets_parameter_invalid', 'creation_assets_not_found', 'creation_assets_request_conflict', 'creation_assets_stale', 'creation_assets_source_unavailable', 'creation_assets_template_already_imported'].includes(error.failure.code ?? '');
}
function parseTemplate(item: Json): Template {
  const status = str(item, 'status');
  if (status !== 'review' && status !== 'ok' && status !== 'off') throw new ShapeError('status');
  const steps = field(item, 'steps');
  if (!Array.isArray(steps) || steps.length < 1 || steps.length > 12 || steps.some(step => typeof step !== 'string' || !Object.hasOwn(stepNames, step))) throw new ShapeError('steps');
  const revision = num(item, 'revision');
  if (!Number.isSafeInteger(revision) || revision < 1) throw new ShapeError('revision');
  const labels = optObj(item, 'labels');
  let parsedLabels: TemplateLabels | undefined;
  if (labels) {
    const tags = field(labels, 'tags');
    if (!Array.isArray(tags) || tags.some(tag => typeof tag !== 'string')) throw new ShapeError('tags');
    parsedLabels = { name: str(labels, 'name'), tags: tags as string[] };
  }
  const evidence = optObj(item, 'evidenceSource');
  const sourceWorkId = optStr(item, 'sourceWorkId');
  const sourceRevisionId = optStr(item, 'sourceRevisionId');
  if (evidence ? sourceWorkId !== null || sourceRevisionId !== null : !sourceWorkId || !sourceRevisionId) throw new ShapeError('source');
  return { labels: parsedLabels, code: str(item, 'code'), revision, sourceWorkId, sourceRevisionId,
    evidenceSource: evidence ? { workRef: str(evidence, 'workRef'), materialRevision: str(evidence, 'materialRevision') } : undefined, sourceAvailable: bool(item, 'sourceAvailable'), status, steps: steps as Step[], createdAt: str(item, 'createdAt') };
}
const receipt = (value: unknown) => decode(value, root => ({ template: parseTemplate(obj(root, 'template')), replayed: bool(root, 'replayed') }));
const base = '/ops/api/creation/structural-templates';
export async function listTemplates(domain: string, signal?: AbortSignal, search = ''): Promise<Template[]> {
  const query = new URLSearchParams({ domainRef: domain });
  if (search) query.set('search', search);
  const value = await requestJson(`${base}?${query}`, { signal });
  if (!Array.isArray(value)) throw new ShapeError('templates');
  return value.map(item => decode(item, parseTemplate));
}
export async function createTemplate(command: CreateCommand) {
  return receipt(await requestJson(base, { method: 'POST', body: command }));
}
export async function reviewTemplate(code: string, command: { requestId: string; expectedRevision: number; action: 'approve' | 'disable' | 'enable' }) {
  return receipt(await requestJson(`${base}/${encodeURIComponent(code)}/review`, { method: 'POST', body: command }));
}
export async function templateHistory(code: string, signal?: AbortSignal): Promise<Template[]> {
  const value = await requestJson(`${base}/${encodeURIComponent(code)}/revisions`, { signal });
  if (!Array.isArray(value)) throw new ShapeError('template history');
  return value.map(item => decode(item, parseTemplate));
}
export async function reviseTemplate(code: string, command: { requestId: string; expectedRevision: number; steps: Step[]; labels?: TemplateLabels }) {
  return receipt(await requestJson(`${base}/${encodeURIComponent(code)}/revisions`, { method: 'POST', body: command }));
}

export async function createEvidenceTemplate(command: EvidenceTemplateCommand) {
  return receipt(await requestJson(`${base}/from-evidence`, { method: 'POST', body: command }));
}

export async function previewTemplateReplication(source: Pick<EvidenceTemplateCommand, 'domainRef' | 'workRef' | 'materialRevision'>, signal?: AbortSignal) {
  const query = new URLSearchParams(source);
  return decode(await requestJson(`${base}/replication-preview?${query}`, { signal }), root => ({
    system: str(root, 'system'), prompt: str(root, 'prompt'),
  }));
}
