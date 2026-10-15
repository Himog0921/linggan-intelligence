import { ApiError, requestJson, requestBinaryJson } from '../../shared/api';
import { object, parseAiStatus, parseAnchor, parseAsset, parseContext, parseEnvelope, parseExportRecord, parseFrozen, parseHistory, parsePreview, parseReadiness, parseSaveReceipt, parseSourceList, parseWorkList, uuid, WorkbenchContractError, type Citation, type UseTarget, type WorkDocument, type WritingAction, type WritingSelection } from './workbenchModel';
const base = '/ops/api/creation';
const post = (path: string, body: unknown) => requestJson(path, {
  method: 'POST',
  body
});
export async function listWorks(domain: string, signal?: AbortSignal) {
  return parseWorkList(await requestJson(`${base}/works?domainRef=${encodeURIComponent(domain)}`, {
    signal
  })).items;
}
export async function readWork(id: string, signal?: AbortSignal) {
  const value = parseEnvelope(await requestJson(`${base}/works/${uuid(id)}`, {
    signal
  }));
  if (value.workId !== id) throw new WorkbenchContractError();
  return value;
}
export async function create(body: {
  requestId: string;
  domainRef: string;
  document: WorkDocument;
}) {
  return receipt(await post(`${base}/works`, body), body.requestId);
}
export async function save(id: string, body: {
  requestId: string;
  expectedRevision: string;
  document: WorkDocument;
}) {
  return receipt(await post(`${base}/works/${uuid(id)}/save`, body), body.requestId, id);
}
export async function place(id: string, body: {
  requestId: string;
  expectedRevision: string;
  canvasId: string;
  targets: UseTarget[];
}) {
  return receipt(await post(`${base}/works/${uuid(id)}/place`, body), body.requestId, id);
}
export async function restore(id: string, body: {
  requestId: string;
  expectedRevision: string;
  restoreRevision: string;
}) {
  return receipt(await post(`${base}/works/${uuid(id)}/restore`, body), body.requestId, id);
}
function receipt(value: unknown, requestId: string, workId?: string) {
  const r = parseSaveReceipt(value);
  if (r.requestId !== requestId || workId && r.workId !== workId) throw new WorkbenchContractError();
  if (r.outcome === 'stale') throw new ApiError({
    kind: 'http',
    status: 409,
    code: 'creation_stale'
  }, {
    error: {
      code: 'creation_stale'
    },
    receipt: r
  });
  return r;
}
export async function history(id: string) {
  return parseHistory(await requestJson(`${base}/works/${uuid(id)}/revisions`)).items;
}
export async function sources(query: {
  domainRef: string;
  kind: string;
  query: string;
  cursor?: number;
}, signal?: AbortSignal) {
  const params = new URLSearchParams({
    domainRef: query.domainRef,
    kind: query.kind,
    query: query.query
  });
  if (query.cursor !== undefined) params.set('cursor', String(query.cursor));
  return parseSourceList(await requestJson(`${base}/sources?${params}`, {
    signal
  }));
}
export async function source(domain: string, item: {
  sourceKind: string;
  sourceRef: string;
  workRef: string | null;
  field: string;
}, signal?: AbortSignal) {
  const q = new URLSearchParams({
    domainRef: domain,
    field: item.field
  });
  if (item.workRef) q.set('workRef', item.workRef);
  return parseContext(await requestJson(`${base}/sources/${item.sourceKind}/${uuid(item.sourceRef)}?${q}`, {
    signal
  }));
}
export async function anchor(body: {
  domainRef: string;
  sourceKind: string;
  sourceRef: string;
  workRef: string | null;
  field: string;
  sourceVersion: string;
  start: number;
  end: number;
}) {
  return parseAnchor(await post(`${base}/sources/validate`, body));
}
export async function uploadAsset(id: string, file: File, requestId: string) {
  const record = parseAsset(await requestBinaryJson(`${base}/works/${uuid(id)}/assets`, file, {
    method: 'POST',
    contentType: file.type,
    requestId
  }));
  if (record.workId !== id) throw new WorkbenchContractError();
  return record;
}
export async function freeze(id: string, body: {
  requestId: string;
  expectedRevision: string;
  rendererVersion: string;
  fontVersion: string;
}) {
  const result = parseFrozen(await post(`${base}/works/${uuid(id)}/exports`, body));
  if (result.export.workId !== id || result.revision.id !== body.expectedRevision || result.export.revisionId !== body.expectedRevision) throw new WorkbenchContractError();
  return result;
}
export async function upload(id: string, pageId: string, blob: Blob) {
  const record = parseExportRecord(await requestBinaryJson(`${base}/exports/${uuid(id)}/pages/${uuid(pageId)}`, blob, {
    method: 'PUT',
    contentType: 'image/png'
  }));
  if (record.id !== id || !record.pages.some(p => p.pageId === pageId)) throw new WorkbenchContractError();
  return record;
}
export async function finalize(id: string, body: {
  requestId: string;
}) {
  const record = parseExportRecord(await post(`${base}/exports/${uuid(id)}/finalize`, body));
  if (record.id !== id) throw new WorkbenchContractError();
  return record;
}
export async function retry(id: string, body: {
  requestId: string;
}) {
  const result = parseFrozen(await post(`${base}/exports/${uuid(id)}/retry`, body));
  if (result.export.id !== id || result.export.revisionId !== result.revision.id) throw new WorkbenchContractError();
  return result;
}
export async function fail(id: string, body: {
  requestId: string;
  code: string;
}) {
  const record = parseExportRecord(await post(`${base}/exports/${uuid(id)}/fail`, body));
  if (record.id !== id) throw new WorkbenchContractError();
  return record;
}
export async function readiness(signal?: AbortSignal) {
  return parseReadiness(await requestJson(`${base}/ai/readiness`, {
    signal
  }));
}
export async function preview(body: {
  previewRef: string;
  workRef: string;
  expectedRevision: string;
  action: WritingAction;
  configRef: string;
  citations: Citation[];
  paragraphIds: string[];
  selection: WritingSelection | null;
  instruction: string;
}) {
  return parsePreview(await post(`${base}/ai/preview`, body));
}
export async function execute(body: {
  requestId: string;
  previewRef: string;
  expectedRevision: string;
  confirmedInputHash: string;
  budgetTokens: number;
  confirmed: boolean;
}) {
  const r = parseAiStatus(await post(`${base}/ai/requests`, body));
  if (r.requestId !== body.requestId) throw new WorkbenchContractError();
  return r;
}
export async function aiStatus(id: string, signal?: AbortSignal) {
  const r = parseAiStatus(await requestJson(`${base}/ai/requests/${uuid(id)}`, {
    signal
  }));
  if (r.requestId !== id) throw new WorkbenchContractError();
  return r;
}
export async function cancel(id: string) {
  return parseAiStatus(await post(`${base}/ai/requests/${uuid(id)}/cancel`, {}));
}
export async function adopt(id: string, body: {
  requestId: string;
  expectedRevision: string;
  paragraphIndices: number[] | null;
  pageIndices: number[] | null;
  adoptTitle: boolean;
}) {
  return receipt(await post(`${base}/ai/requests/${uuid(id)}/adopt`, body), body.requestId);
}
export interface DomainOption {
  domainRef: string;
  name: string;
}
export interface AccountOption {
  ref: string;
  name: string;
  platform: string;
}
export async function domains(signal?: AbortSignal): Promise<DomainOption[]> {
  const r = object(await requestJson('/api/collection/management/domains', {
    signal
  }));
  if (!Array.isArray(r.items)) throw new WorkbenchContractError();
  return r.items.map(v => {
    const i = object(v);
    if (typeof i.name !== 'string') throw new WorkbenchContractError();
    return {
      domainRef: uuid(i.domainRef),
      name: i.name
    };
  });
}
export async function accounts(domain: string, signal?: AbortSignal): Promise<AccountOption[]> {
  try {
    const r = object(await requestJson(`/api/local/ops/accounts?domain=${encodeURIComponent(domain)}`, {
      signal
    }));
    if (!Array.isArray(r.items)) throw new WorkbenchContractError();
    return r.items.map(v => {
      const i = object(v);
      if (typeof i.name !== 'string' || typeof i.platform !== 'string') throw new WorkbenchContractError();
      return {
        ref: uuid(i.ref),
        name: i.name,
        platform: i.platform
      };
    });
  } catch (e) {
    if (e instanceof ApiError && e.failure.kind === 'http' && e.failure.status === 404 && e.failure.code === 'ops_business_not_found') return [];
    throw e;
  }
}
export function conflictReceipt(error: unknown) {
  if (error instanceof ApiError && error.failure.kind === 'http' && error.failure.status === 409) {
    const body = object(error.responseBody);
    if (body.receipt !== undefined) return parseSaveReceipt(body.receipt);
  }
  return null;
}
export class WorkbenchActionError extends Error {}

/** Human feedback is separate from the preserved invocation diagnostics. */
export function aiFailureText(code: string | null): string {
  const messages: Record<string, string> = {
    writing_output_invalid: '这次返回的建议暂时无法使用，正文没有改变。可调整要求后重新准备，也可以继续手写。',
    source_or_base_changed_before_dispatch: '选定材料或底稿已经变化，这次生成没有继续。请核对后重新准备。',
    creation_ai_source_unavailable: '所选材料已经变化或暂不可用。请回到材料重新选取，再准备建议。',
    creation_ai_model_unavailable: '创作模型暂不可用，请到设置核对连接。当前输入仍保留，也可以继续手写。',
    model_secret_unavailable: '模型连接信息暂不可用，请到设置核对连接。当前输入仍保留。',
    provider_timeout: '生成等待超时，当前输入仍保留。请先刷新结果，核实后再决定是否重新准备。',
    provider_rate_limited: '模型服务暂时繁忙，当前输入仍保留。可稍后刷新结果或继续手写。',
    cancelled: '这次生成已取消，正文没有改变。可继续手写或重新准备建议。',
    cancelled_before_dispatch: '这次生成已取消，正文没有改变。可继续手写或重新准备建议。'
  };
  return code && messages[code] ? messages[code] : '这次没有生成可用建议，当前输入仍保留。请先刷新结果，或继续手写。';
}
export function aiStateText(state: string): string {
  const labels: Record<string, string> = {
    succeeded: '建议已返回',
    running: '处理中',
    queued: '等待处理',
    cancel_requested: '正在取消，请稍后刷新结果',
    cancelled: '已取消',
    failed: '生成失败'
  };
  return labels[state] ?? '结果暂未确认，请刷新查看';
}
export function errorText(error: unknown): string {
  if (error instanceof WorkbenchContractError) return '暂时无法读取创作结果，当前输入仍保留。请刷新核对。';
  if (error instanceof WorkbenchActionError) return error.message;
  if (error instanceof ApiError) {
    if (error.failure.kind === 'network' || error.failure.kind === 'invalid_response') return '结果尚未确认，当前输入仍保留。请核实后用原请求重试。';
    const code = error.failure.code;
    const messages: Record<string, string> = {
      creation_stale: '服务器已有新版本，请比较并选择保留方式。',
      creation_source_unavailable: '来源可用情况已变化，请回到材料核对。',
      source_changed: '来源版本已变化，请重新读取并选取。',
      source_restricted: '来源已受限，不能继续引用。',
      source_unavailable: '来源暂不可读。',
      creation_not_connected: '创作暂时无法读取，当前输入仍保留。请稍后重试。',
      creation_storage_not_configured: '图片存储尚未配置，仍可继续写正文。',
      creation_ai_model_unavailable: '创作模型尚不可用，仍可手写。',
      creation_ai_stale: '底稿已改变，请重新预览。',
      creation_request_conflict: '这次操作与先前的内容不一致，请先刷新核对。当前输入仍保留。'
    };
    return code && messages[code] ? messages[code] : '操作未完成，当前输入仍保留。请稍后重试。';
  }
  return '操作未完成，当前输入仍保留。请稍后重试。';
}
/** Read the original request result before an explicitly requested resend. */
export async function requestReceipt(requestId: string): Promise<unknown | null> {
  try {
    const r = object(await requestJson(`${base}/requests/${uuid(requestId)}`));
    if (r.requestId !== requestId || !['work', 'asset', 'export'].includes(String(r.kind)) || !('receipt' in r) || Object.keys(r).some(k => !['requestId', 'kind', 'receipt'].includes(k))) throw new WorkbenchContractError();
    return r.receipt;
  } catch (e) {
    if (e instanceof ApiError && e.failure.kind === 'http' && e.failure.status === 404) return null;
    throw e;
  }
}
export async function lookupWorkRequest(requestId: string, workId?: string) {
  const value = await requestReceipt(requestId);
  return value === null ? null : receipt(value, requestId, workId);
}
export async function lookupExportRequest(requestId: string) {
  const value = await requestReceipt(requestId);
  return value === null ? null : parseFrozen(value);
}
export async function readExport(id: string) {
  const record = parseExportRecord(await requestJson(`${base}/exports/${uuid(id)}`));
  if (record.id !== id) throw new WorkbenchContractError();
  return record;
}
