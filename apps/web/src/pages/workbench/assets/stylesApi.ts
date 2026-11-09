import { ApiError, requestJson } from '../../../shared/api';
import { ShapeError, bool, decode, list, num, obj, str, strList, type Json } from '../../../shared/decode';

export type Style = {
  code: string; accountRef: string; revision: number; name: string; sourceNote: string;
  structure: string[]; rhythm: string[]; wording: string[]; skip: string[]; savedAt: string;
};
export type StyleCommand = {
  requestId: string; accountRef: string; expectedRevision: number; name: string; sourceNote: string;
  structure: string[]; rhythm: string[]; wording: string[]; skip: string[];
};
export function parseStyle(item: Json): Style {
  const revision = num(item, 'revision');
  if (!Number.isSafeInteger(revision) || revision < 1) throw new ShapeError('revision');
  return {
    code: str(item, 'code'), accountRef: str(item, 'accountRef'), revision,
    name: str(item, 'name'), sourceNote: str(item, 'sourceNote'),
    structure: strList(item, 'structure'), rhythm: strList(item, 'rhythm'),
    wording: strList(item, 'wording'), skip: strList(item, 'skip'),
    savedAt: str(item, 'savedAt')
  };
}
const receipt = (value: unknown) => decode(value, root => ({ style: parseStyle(obj(root, 'style')), replayed: bool(root, 'replayed') }));
/** True when the server refused the write because a newer revision exists. */
export function staleStyleCommand(error: unknown): boolean {
  return error instanceof ApiError && error.failure.kind === 'http' && error.failure.code === 'creation_assets_stale';
}
const base = '/ops/api/creation/styles';
export async function listStyles(domain: string, signal?: AbortSignal): Promise<Style[]> {
  const value = await requestJson(`${base}?domainRef=${encodeURIComponent(domain)}`, { signal });
  return decode(value, root => list(root, 'items', parseStyle));
}
export async function createStyle(command: StyleCommand) {
  return receipt(await requestJson(base, { method: 'POST', body: command }));
}
export async function reviseStyle(code: string, command: StyleCommand) {
  return receipt(await requestJson(`${base}/${encodeURIComponent(code)}`, { method: 'PUT', body: command }));
}
