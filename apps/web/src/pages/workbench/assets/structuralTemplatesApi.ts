import { ApiError, requestJson } from '../../../shared/api';
import { ShapeError, bool, decode, field, num, obj, str, type Json } from '../../../shared/decode';

export const stepNames = { opening: '开头', scene: '场景', experience: '经历', feeling: '感受', reflection: '观点', action: '行动', ending: '收尾' } as const;
export type Step = keyof typeof stepNames;
export type Template = { code: string; revision: number; sourceWorkId: string; sourceRevisionId: string; sourceAvailable: boolean; status: 'review' | 'ok' | 'off'; steps: Step[]; createdAt: string };
export type CreateCommand = { requestId: string; workId: string; expectedRevision: string; steps: Step[] };
export function rejectedTemplateCommand(error: unknown): error is ApiError {
  return error instanceof ApiError && error.failure.kind === 'http' && ['creation_assets_parameter_invalid', 'creation_assets_not_found', 'creation_assets_request_conflict', 'creation_assets_stale', 'creation_assets_source_unavailable'].includes(error.failure.code ?? '');
}
function parseTemplate(item: Json): Template {
  const status = str(item, 'status');
  if (status !== 'review' && status !== 'ok' && status !== 'off') throw new ShapeError('status');
  const steps = field(item, 'steps');
  if (!Array.isArray(steps) || steps.length < 1 || steps.length > 12 || steps.some(step => typeof step !== 'string' || !Object.hasOwn(stepNames, step))) throw new ShapeError('steps');
  const revision = num(item, 'revision');
  if (!Number.isSafeInteger(revision) || revision < 1) throw new ShapeError('revision');
  return { code: str(item, 'code'), revision, sourceWorkId: str(item, 'sourceWorkId'), sourceRevisionId: str(item, 'sourceRevisionId'), sourceAvailable: bool(item, 'sourceAvailable'), status, steps: steps as Step[], createdAt: str(item, 'createdAt') };
}
const receipt = (value: unknown) => decode(value, root => ({ template: parseTemplate(obj(root, 'template')), replayed: bool(root, 'replayed') }));
const base = '/ops/api/creation/structural-templates';
export async function listTemplates(domain: string, signal?: AbortSignal): Promise<Template[]> {
  const value = await requestJson(`${base}?domainRef=${encodeURIComponent(domain)}`, { signal });
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
export async function reviseTemplate(code: string, command: { requestId: string; expectedRevision: number; steps: Step[] }) {
  return receipt(await requestJson(`${base}/${encodeURIComponent(code)}/revisions`, { method: 'POST', body: command }));
}
