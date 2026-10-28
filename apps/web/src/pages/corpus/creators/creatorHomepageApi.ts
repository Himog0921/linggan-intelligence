import { requestJson } from '../../../shared/api';

export type PreviewState = 'not_requested' | 'running' | 'complete' | 'partial' | 'stopped';
export interface HomepageNote {
  workRef: string;
  ordinal: number;
  /** The server says whether this card is already formal research material of the domain. */
  inDomain: boolean;
  title: string | null;
  likes: number | null;
  comments: number | null;
  collects: number | null;
}
export interface HomepagePreview {
  state: PreviewState;
  runRef: string | null;
  revision: number | null;
  requestedLimit: number;
  sampleCount: number;
  /** Cards frozen into the sample; may exceed the listed ones when some cannot be read right now. */
  frozenCount: number | null;
  scopeFrozenAt: string | null;
  stopReason: string | null;
  works: HomepageNote[];
}
export interface RunReceipt {
  runRef: string;
  domainRef: string | null;
  replayed: boolean;
}

function uuid(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') return crypto.randomUUID();
  throw new Error('当前浏览器不支持安全请求标识');
}

function asObject(value: unknown): Record<string, unknown> {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) throw new Error('作者数据格式不正确');
  return value as Record<string, unknown>;
}
function optionalNumber(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0 ? value : null;
}
function optionalString(value: unknown): string | null {
  return typeof value === 'string' && value.length > 0 ? value : null;
}
export function parseHomepagePreview(value: unknown): HomepagePreview {
  const raw = asObject(value);
  const validStates: PreviewState[] = ['not_requested', 'running', 'complete', 'partial', 'stopped'];
  if (!validStates.includes(raw.state as PreviewState) || !Array.isArray(raw.works)) {
    throw new Error('主页浅看状态无法识别');
  }
  const works: HomepageNote[] = raw.works.map(source => {
    const item = asObject(source);
    if (typeof item.workRef !== 'string' || typeof item.ordinal !== 'number') {
      throw new Error('主页样本作品无法识别');
    }
    return {
      workRef: item.workRef,
      ordinal: item.ordinal,
      inDomain: item.inDomain === true,
      title: optionalString(item.title),
      likes: optionalNumber(item.likes),
      comments: optionalNumber(item.comments),
      collects: optionalNumber(item.collects),
    };
  });
  return {
    state: raw.state as PreviewState,
    runRef: optionalString(raw.runRef),
    revision: optionalNumber(raw.revision),
    requestedLimit: optionalNumber(raw.requestedLimit) ?? 30,
    sampleCount: works.length,
    frozenCount: optionalNumber(raw.frozenCount),
    scopeFrozenAt: optionalString(raw.scopeFrozenAt),
    stopReason: optionalString(raw.stopReason),
    works,
  };
}
export async function getHomepagePreview(domain: string, creatorKey: string, signal?: AbortSignal): Promise<HomepagePreview> {
  const query = new URLSearchParams({ domain });
  const value = await requestJson(`/api/local/creators/${encodeURIComponent(creatorKey)}/preview?${query}`, signal ? { signal } : {});
  return parseHomepagePreview(value);
}
function parseRunReceipt(value: unknown): RunReceipt {
  const raw = asObject(value);
  if (typeof raw.runRef !== 'string' || (raw.domainRef !== undefined && typeof raw.domainRef !== 'string') || typeof raw.replayed !== 'boolean') {
    throw new Error('采集回执缺少必须字段');
  }
  return { runRef: raw.runRef, domainRef: typeof raw.domainRef === 'string' ? raw.domainRef : null, replayed: raw.replayed };
}
export function newCommandRef(): string { return uuid(); }
export async function startHomepagePreview(domain: string, creatorKey: string, commandRef: string): Promise<RunReceipt> {
  return parseRunReceipt(await requestJson(
    `/api/local/creators/${encodeURIComponent(creatorKey)}/preview`,
    { method: 'POST', body: { domain, commandRef } },
  ));
}
export async function studyCreatorWorks(
  domain: string,
  previewRef: string,
  workRefs: string[],
  preset: 'standard' | 'text_only',
  idempotencyKey: string,
): Promise<RunReceipt> {
  const refs = [...new Set(workRefs)];
  if (refs.length === 0 || refs.length > 30 || refs.length !== workRefs.length) throw new Error('必须精确选择1至30篇');
  return parseRunReceipt(await requestJson('/api/collection/runs', {
    method: 'POST',
    body: {
      idempotencyKey,
      domainRef: domain,
      source: { kind: 'creator_selected_works', previewRef, workRefs: refs },
      ranking: null,
      workLimit: refs.length,
      commentMode: preset === 'standard' ? 'sample_30' : 'none',
      commentTarget: null,
    },
  }));
}
export async function stopHomepagePreview(domain: string, runRef: string, revision: number, commandRef: string) {
  return requestJson(`/api/collection/runs/${encodeURIComponent(runRef)}/commands`, {
    method: 'POST',
    body: { domainRef: domain, commandRef, expectedRevision: revision, action: 'stop' },
  });
}
