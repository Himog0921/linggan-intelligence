import { requestJson } from '../../../shared/api';
import { decode, list, obj, optObj, optStr, type Json } from '../../../shared/decode';
import { apiPath } from './evidencePaths';

/**
 * 立即复观测的读写合同：`POST /api/local/work-resources/{ref}/reobserve?domain=…`（无请求体）与
 * `GET …/reobserve/{lease}`（状态，只在有租约后才会被用到）。
 *
 * 目标与用途由服务器从该作品关联的有效深度归档授权推导，标题、作者名、URL 从不当作授权。没有请求编号：
 * 同一范围若已有尚未被工位认领的工单则合并（`MERGE`，同一个 `workOrderRef`），一旦被认领，再点会新建一张工单。
 */
export interface ReobservationTask {
  taskId: string | null;
  capability: string | null;
  state: string;
  /** 数字或 `not_requested`。 */
  commentLimit: string | number | null;
  acquireMedia: string | null;
  claimedAt: string | null;
  attemptId: string | null;
  packageRef: string | null;
  receiptRef: string | null;
}

export interface ReobservationOperation {
  /** `ADMITTED`｜`MERGE`｜`REUSE`｜`DEFERRED`｜`REFUSED`｜`DECISION_REQUIRED`｜`UNKNOWN`；状态读取时不带。 */
  admission: string | null;
  admissionReason: string | null;
  decisionRef: string | null;
  requestRef: string | null;
  workOrderRef: string | null;
  leaseRef: string | null;
  expiresAt: string | null;
  /** `QUEUED`｜`NOT_STARTED`；状态读取时被 `leaseState` 取代。 */
  execution: string | null;
  leaseState: string | null;
  mediaState: string | null;
  mediaReason: string | null;
  tasks: ReobservationTask[];
}

export interface ReobservationReceipt {
  operation: ReobservationOperation;
  /** 同源、在作品资源根下才保留；请求时恒为 null（租约要等工位认领才形成）。 */
  statusPath: string | null;
}

function parseTask(record: Json): ReobservationTask {
  const limit = record.commentLimit;
  return {
    taskId: optStr(record, 'taskId'),
    capability: optStr(record, 'capability'),
    state: optStr(record, 'state') ?? 'UNKNOWN',
    commentLimit: typeof limit === 'number' || typeof limit === 'string' ? limit : null,
    acquireMedia: typeof record.acquireMedia === 'string' ? record.acquireMedia : null,
    claimedAt: optStr(record, 'claimedAt'),
    attemptId: optStr(record, 'attemptId'),
    packageRef: optStr(record, 'packageRef'),
    receiptRef: optStr(record, 'receiptRef'),
  };
}

export function parseOperation(record: Json): ReobservationOperation {
  const media = optObj(record, 'media');
  return {
    admission: optStr(record, 'admission'),
    admissionReason: optStr(record, 'admissionReason'),
    decisionRef: optStr(record, 'decisionRef'),
    requestRef: optStr(record, 'requestRef'),
    workOrderRef: optStr(record, 'workOrderRef'),
    leaseRef: optStr(record, 'leaseRef'),
    expiresAt: optStr(record, 'expiresAt'),
    execution: optStr(record, 'execution'),
    leaseState: optStr(record, 'leaseState'),
    mediaState: media === null ? null : optStr(media, 'state'),
    mediaReason: media === null ? null : optStr(media, 'reason'),
    tasks: Array.isArray(record.tasks) ? list(record, 'tasks', parseTask) : [],
  };
}

export function parseReobservationReceipt(value: unknown): ReobservationReceipt {
  return decode(value, root => ({
    operation: parseOperation(obj(root, 'operation')),
    statusPath: apiPath(root.statusUrl),
  }));
}

export function parseReobservationStatus(value: unknown): ReobservationOperation {
  return decode(value, root => parseOperation(obj(root, 'operation')));
}

/** 只由用户的明确确认触发；不由 Effect 触发，不自动重试。`actionPath` 来自详情的复观测入口并已校验同源。 */
export async function requestReobservation(actionPath: string, signal?: AbortSignal): Promise<ReobservationReceipt> {
  return parseReobservationReceipt(await requestJson(actionPath, { method: 'POST', ...(signal ? { signal } : {}) }));
}

export async function fetchReobservationStatus(statusPath: string, signal?: AbortSignal): Promise<ReobservationOperation> {
  return parseReobservationStatus(await requestJson(statusPath, signal ? { signal } : {}));
}

/**
 * 把状态读取的结果并进请求回执：状态响应里没有的字段（准入、工单等）保持请求时的值，不被解码出来的 null 盖掉。
 * 任务列表以状态响应为准。
 */
export function mergeOperation(previous: ReobservationOperation, status: ReobservationOperation): ReobservationOperation {
  const merged: Record<string, unknown> = { ...previous };
  for (const [key, value] of Object.entries(status)) {
    if (key !== 'tasks' && value !== null) merged[key] = value;
  }
  return { ...(merged as unknown as ReobservationOperation), tasks: status.tasks };
}

/** 终态：已接纳回执、租约结束未见回执、任务结束未见回执、输入不可执行（已停止）。 */
export const TERMINAL_TASK_STATES: ReadonlySet<string> = new Set([
  'ACCEPTED',
  'EXPIRED_WITHOUT_RECEIPT',
  'COMPLETED_WITHOUT_RECEIPT',
  'INPUT_BLOCKED',
]);

export function isTerminalTask(task: Pick<ReobservationTask, 'state'>): boolean {
  return TERMINAL_TASK_STATES.has(task.state);
}
