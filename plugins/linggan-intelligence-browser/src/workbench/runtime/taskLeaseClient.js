import { normalizeServerUrl } from '../../shared/utils.js';
import {
  buildSyncRequestV11,
  extractMailboxVersionsFromResponse,
  resolveStationSessionId,
} from '../protocol/syncEnvelopeV11.js';

const DEFAULT_SERVER_URL = 'https://lingganboom.fun';
const DEFAULT_LEASE_STORAGE_KEY = 'workbenchActiveTaskLease';

function normalizeString(value = '') {
  return String(value || '').trim();
}

function toStringArray(value) {
  return Array.isArray(value) ? value.filter((item) => typeof item === 'string') : [];
}

function toFiniteNumber(value, fallback = 0) {
  const num = Number(value);
  return Number.isFinite(num) ? num : fallback;
}

function toOptionalInteger(value) {
  const num = Number(value);
  return Number.isFinite(num) ? Math.floor(num) : undefined;
}

// 报告 §9.1：采集阶段进度上限 95%。100% 是"服务端已确认终态包入库"的语义，
// 只能由服务端在 commit_raw_snapshot/release_job 被接受后判定，插件不得自报。
const CAPTURE_PROGRESS_CAP = 95;

function clampCaptureProgress(value) {
  const num = toOptionalInteger(value);
  if (num === undefined) return undefined;
  return Math.min(CAPTURE_PROGRESS_CAP, Math.max(0, num));
}

function normalizeLeaseSnapshot({
  task = null,
  lease = null,
  attempt = null,
  fallback = null,
} = {}) {
  const taskId = normalizeString(lease?.taskId || task?.id || fallback?.taskId);
  const leaseToken = normalizeString(lease?.leaseToken || fallback?.leaseToken);
  const expiresAt = normalizeString(lease?.expiresAt || fallback?.expiresAt);
  const attemptId = normalizeString(
    lease?.attemptId
    || attempt?.attemptId
    || task?.currentAttemptId
    || fallback?.attemptId,
  );
  const attemptNumber = toOptionalInteger(
    lease?.attemptNumber
    ?? attempt?.attemptNumber
    ?? task?.attemptCount
    ?? fallback?.attemptNumber,
  );
  const leaseEpoch = toOptionalInteger(
    lease?.leaseEpoch
    ?? task?.leaseEpoch
    ?? fallback?.leaseEpoch,
  );
  const snapshot = {
    taskId,
    leaseToken,
    expiresAt,
  };
  if (attemptId) snapshot.attemptId = attemptId;
  if (attemptNumber !== undefined) snapshot.attemptNumber = attemptNumber;
  if (leaseEpoch !== undefined) snapshot.leaseEpoch = leaseEpoch;
  return snapshot;
}

function isPlainObject(value) {
  return value && typeof value === 'object' && !Array.isArray(value);
}

function normalizePayload(value = {}) {
  return isPlainObject(value) ? { ...value } : {};
}

function addLeaseAttemptFields(body = {}, lease = {}) {
  const attemptId = normalizeString(lease?.attemptId);
  const leaseEpoch = toOptionalInteger(lease?.leaseEpoch);
  const attemptNumber = toOptionalInteger(lease?.attemptNumber);
  if (attemptId) body.attemptId = attemptId;
  if (leaseEpoch !== undefined) body.leaseEpoch = leaseEpoch;
  if (attemptNumber !== undefined) body.attemptNumber = attemptNumber;
  return body;
}

function extractIdleClaimReason(claim = {}) {
  const rawReason = claim?.reason && typeof claim.reason === 'object' && !Array.isArray(claim.reason)
    ? { ...claim.reason }
    : null;
  const code = normalizeString(claim?.idleReasonCode || claim?.reasonCode || rawReason?.code || '');
  const message = normalizeString(claim?.idleReasonMessage || claim?.reasonMessage || rawReason?.message || '');
  const nextPollAfterMs = toFiniteNumber(claim?.nextPollAfterMs, 0);
  const reason = rawReason || (code || message ? { code, message } : null);

  return {
    code,
    message,
    nextPollAfterMs,
    reason,
  };
}

export function createTaskLeaseIdleSnapshot(claim = {}) {
  const { code, message, nextPollAfterMs, reason } = extractIdleClaimReason(claim);
  const mailboxVersion = extractMailboxVersion(claim);
  if (!code && !message && !reason && mailboxVersion === undefined) return null;
  return attachMailboxVersion({
    taskId: '',
    leaseToken: '',
    expiresAt: '',
    idleReasonCode: code,
    idleReasonMessage: message,
    nextPollAfterMs,
    reason,
  }, mailboxVersion);
}

export function formatTaskLeaseIdleNotice(snapshot = {}) {
  const code = normalizeString(snapshot?.idleReasonCode || snapshot?.reason?.code || '');
  const message = normalizeString(snapshot?.idleReasonMessage || snapshot?.reason?.message || '');
  const nextPollAfterMs = toFiniteNumber(snapshot?.nextPollAfterMs, 0);
  const detail = message || code;
  if (!detail) return null;

  let text = `最近一次不接单原因：${detail}`;
  if (message && code && message !== code) {
    text += `（${code}）`;
  }
  if (nextPollAfterMs > 0) {
    text += `，约 ${Math.ceil(nextPollAfterMs / 1000)} 秒后重试`;
  }

  return {
    message: text,
    type: 'warning',
    visible: true,
  };
}

async function readErrorText(response) {
  if (!response?.text) return '';
  return response.text().catch(() => '');
}

function parseRetryAfterHeader(value = '') {
  const normalized = normalizeString(value);
  if (!normalized) return 0;
  const seconds = Number(normalized);
  if (Number.isFinite(seconds) && seconds > 0) return Math.round(seconds * 1000);
  const parsedDate = new Date(normalized).getTime();
  if (Number.isFinite(parsedDate)) return Math.max(0, parsedDate - Date.now());
  return 0;
}

function parseErrorBody(text = '') {
  try {
    const parsed = JSON.parse(normalizeString(text));
    return parsed && typeof parsed === 'object' ? parsed : {};
  } catch {
    return {};
  }
}

function isRetryableStatus(status) {
  return [408, 429, 500, 502, 503, 504].includes(Number(status));
}

function errorMessageFromResponseText(text = '', fallback = '') {
  const body = normalizeString(text);
  if (!body) return fallback;
  try {
    const parsed = JSON.parse(body);
    const message = normalizeString(parsed?.error || parsed?.message);
    return message || body;
  } catch {
    return body;
  }
}

function createHttpError(message, { status = 0, reasonCode = '', nextPollAfterMs = 0 } = {}) {
  const error = new Error(message);
  error.status = Number(status || 0);
  error.retryable = isRetryableStatus(status);
  error.reasonCode = normalizeString(reasonCode);
  error.nextPollAfterMs = toFiniteNumber(nextPollAfterMs, 0);
  return error;
}

function isStationSyncEnvelope(data = {}) {
  return Boolean(
    data
    && typeof data === 'object'
    && !Array.isArray(data)
    && (
      Object.prototype.hasOwnProperty.call(data, 'heartbeat')
      || Object.prototype.hasOwnProperty.call(data, 'reconcile')
      || Object.prototype.hasOwnProperty.call(data, 'claim')
      || Object.prototype.hasOwnProperty.call(data, 'mailbox')
      || Object.prototype.hasOwnProperty.call(data, 'mailboxVersions')
      || Object.prototype.hasOwnProperty.call(data, 'reservations')
      || Object.prototype.hasOwnProperty.call(data, 'operationResults')
      || Object.prototype.hasOwnProperty.call(data, 'mode')
    )
  );
}

function extractMailboxVersion(data = {}) {
  return toOptionalInteger(
    data?.mailboxVersions?.station
    ?? data?.mailboxVersion
    ?? data?.mailbox?.version
    ?? data?.sync?.mailbox?.version
    ?? data?.dispatch?.mailbox?.version,
  );
}

function attachMailboxVersion(snapshot = {}, mailboxVersion) {
  const normalizedMailboxVersion = toOptionalInteger(mailboxVersion);
  if (normalizedMailboxVersion === undefined) return snapshot;
  return {
    ...snapshot,
    mailboxVersion: normalizedMailboxVersion,
  };
}

/**
 * 同 attachMailboxVersion，但额外持久化 V1.1 lane 版本号。
 * laneVersions 为空时退化为只 attach station 版本（向后兼容）。
 */
function attachMailboxLaneVersions(snapshot = {}, mailboxVersion, laneVersions = {}) {
  const withStation = attachMailboxVersion(snapshot, mailboxVersion);
  if (!laneVersions || typeof laneVersions !== 'object' || Object.keys(laneVersions).length === 0) {
    return withStation;
  }
  return {
    ...withStation,
    mailboxLaneVersions: { ...laneVersions },
  };
}

function acceptedDeltaKeys(envelope = {}) {
  return {
    acceptedEventKeys: Array.isArray(envelope?.events)
      ? envelope.events.map((event) => normalizeString(event?.idempotencyKey)).filter(Boolean)
      : [],
    acceptedRecordKeys: Array.isArray(envelope?.records)
      ? envelope.records.map((record) => normalizeString(record?.idempotencyKey)).filter(Boolean)
      : [],
  };
}

function isTerminalSnapshotStatus(status = '') {
  const normalized = normalizeString(status).toLowerCase();
  return normalized === 'completed'
    || normalized === 'failed'
    || normalized === 'stopped'
    || normalized === 'cancelled'
    || normalized === 'canceled';
}

function normalizeRawRecordType(value = '') {
  const normalized = normalizeString(value);
  if (normalized === 'note' || normalized === 'comment' || normalized === 'author') return normalized;
  return 'metric';
}

function operationObservedAt(envelope = {}, fallback = new Date().toISOString()) {
  const snapshotAt = normalizeString(envelope?.snapshot?.latestHeartbeatAt);
  if (snapshotAt) return snapshotAt;
  const recordAt = Array.isArray(envelope?.records)
    ? normalizeString(envelope.records.find((record) => normalizeString(record?.collectedAt))?.collectedAt)
    : '';
  return recordAt || fallback;
}

function captureIdForEnvelope(taskId = '', envelope = {}) {
  const cursor = normalizeString(envelope?.cursor);
  const pluginRunId = normalizeString(envelope?.pluginRunId);
  const attemptId = normalizeString(envelope?.attemptId);
  return [
    'plugin-v11',
    normalizeString(taskId),
    pluginRunId || 'run',
    attemptId || 'attempt',
    cursor || Date.now(),
  ].join(':');
}

function rawRecordsFromEnvelope(envelope = {}, fallbackObservedAt = new Date().toISOString()) {
  return (Array.isArray(envelope?.records) ? envelope.records : [])
    .filter((record) => record && typeof record === 'object' && !Array.isArray(record))
    .map((record, index) => ({
      recordType: normalizeRawRecordType(record.recordType),
      platform: normalizeString(record.platform || envelope?.executionContext?.platform),
      targetKey: normalizeString(record.targetKey || ''),
      externalRecordId: normalizeString(record.externalRecordId || ''),
      sequence: toOptionalInteger(record.sequence) ?? index,
      payload: isPlainObject(record.payload) ? record.payload : {},
      payloadHash: normalizeString(record.payloadHash || ''),
      observedAt: normalizeString(record.observedAt || record.collectedAt) || fallbackObservedAt,
      collectedAt: normalizeString(record.collectedAt) || null,
      idempotencyKey: normalizeString(record.idempotencyKey),
      dedupeKey: normalizeString(record.dedupeKey || ''),
    }))
    .filter((record) => record.idempotencyKey);
}

function nextPollAfterMsFromSync(data = {}) {
  return toFiniteNumber(data?.nextSync?.afterMs, 0)
    || toFiniteNumber(data?.nextSyncAfterMs, 0);
}

function isNoteDetailProfile(collectionProfile = '') {
  return collectionProfile === 'note_full' || collectionProfile === 'note_detail';
}

function noteIdFromTargetKey(targetKey = '') {
  const normalized = normalizeString(targetKey);
  const match = normalized.match(/(?:^|:)note:([^:]+)$/);
  return normalizeString(match?.[1]);
}

function inferTaskTypeFromReservation(reservation = {}) {
  const spec = normalizePayload(reservation.taskSpec);
  const payload = normalizePayload(spec.payload);
  const platform = normalizeString(spec.platform || reservation.platform || payload.platform);
  const collectionProfile = normalizeString(spec.collectionProfile || payload.collectionProfile);
  const jobType = normalizeString(spec.jobType || payload.jobType);

  const explicitTaskType = normalizeString(
    spec.taskType
    || payload.taskType
    || payload.originalTaskType
    || payload.externalTaskType,
  );

  if (platform === 'xhs') {
    if (collectionProfile === 'author_links') return 'xhs.author_links';
    if (collectionProfile === 'comment_probe' || jobType.includes('comment')) return 'xhs.comment_scan';
    if (collectionProfile === 'author_profile' || jobType.includes('author_profile')) return 'xhs.author_profile';
    if (isNoteDetailProfile(collectionProfile)) return 'xhs.note_full';
    if (collectionProfile === 'list_scan') return 'xhs.list_scan';
    if (explicitTaskType) return explicitTaskType;
    return 'xhs.batchNotes';
  }
  if (platform === 'douyin') {
    if (collectionProfile === 'comment_probe' || jobType.includes('comment')) return 'douyin.batchComments';
    if (collectionProfile === 'author_profile' || jobType.includes('author')) return 'douyin.collectAuthor';
    if (isNoteDetailProfile(collectionProfile)) return 'douyin.batchNotes';
    if (collectionProfile === 'list_scan') return 'douyin.batchNotes';
    if (explicitTaskType) return explicitTaskType;
    return 'douyin.batchNotes';
  }
  if (explicitTaskType) return explicitTaskType;
  return '';
}

function buildTaskFromReservation(reservation = {}) {
  const spec = normalizePayload(reservation.taskSpec);
  const payload = normalizePayload(spec.payload);
  const jobId = normalizeString(reservation.jobId || spec.id || payload.id || payload.taskId);
  const platform = normalizeString(spec.platform || reservation.platform || payload.platform);
  const collectionProfile = normalizeString(spec.collectionProfile || payload.collectionProfile);
  const jobType = normalizeString(spec.jobType || payload.jobType);
  const targetKey = normalizeString(spec.targetKey || payload.targetKey);
  const platformAccountId = normalizeString(
    reservation.platformAccountId
    || reservation.reservedPlatformAccountId
    || spec.platformAccountId
    || spec.reservedPlatformAccountId
    || payload.platformAccountId
    || payload.reservedPlatformAccountId
    || payload.accountId,
  );
  const taskType = inferTaskTypeFromReservation(reservation);
  const taskStrategy = normalizeString(spec.taskStrategy || payload.taskStrategy || payload.strategy);
  const source = normalizeString(spec.source || payload.source || payload.sourceSystem || 'workbench');
  const target = normalizeString(
    spec.target
    || payload.target
    || payload.url
    || payload.canonicalUrl
    || payload.profileUrl
    || payload.noteUrl
    || spec.targetKey,
  );
  const normalizedPayload = { ...payload };
  if (isNoteDetailProfile(collectionProfile)) {
    normalizedPayload.targetPageType = normalizeString(normalizedPayload.targetPageType) || 'detail';

    const targetNoteId = noteIdFromTargetKey(targetKey);
    if (targetNoteId) {
      normalizedPayload.noteId = normalizeString(normalizedPayload.noteId) || targetNoteId;
      normalizedPayload.platformContentId = normalizeString(normalizedPayload.platformContentId) || targetNoteId;
    }
  }
  if (platform === 'xhs' && collectionProfile === 'note_full') {
    const includeCommentsAlreadySet =
      Object.prototype.hasOwnProperty.call(normalizedPayload, 'includeComments')
      || Object.prototype.hasOwnProperty.call(normalizedPayload, 'collectComments');
    if (!includeCommentsAlreadySet) {
      normalizedPayload.includeComments = true;
    }
    const commentLimitAlreadySet =
      Object.prototype.hasOwnProperty.call(normalizedPayload, 'commentLimit')
      || Object.prototype.hasOwnProperty.call(normalizedPayload, 'commentsLimit');
    if (!commentLimitAlreadySet) {
      normalizedPayload.commentLimit = 30;
      normalizedPayload.commentsLimit = 30;
    }
    normalizedPayload.collectMode = normalizeString(normalizedPayload.collectMode)
      || (normalizedPayload.includeComments === false ? 'detailsOnly' : 'detailsWithComments');
  }

  return {
    id: jobId,
    taskId: jobId,
    platform,
    taskType,
    source,
    taskStrategy,
    target,
    accountId: platformAccountId,
    platformAccountId,
    reservedPlatformAccountId: platformAccountId,
    lane: normalizeString(reservation.lane || spec.lane),
    collectionProfile,
    jobType,
    payload: {
      ...normalizedPayload,
      platform,
      target,
      targetKey,
      taskStrategy,
      accountId: platformAccountId,
      platformAccountId,
      reservedPlatformAccountId: platformAccountId,
      collectionProfile,
      jobType,
    },
  };
}

function buildLeaseFromStartResult(reservation = {}, result = {}) {
  const taskId = normalizeString(reservation.jobId);
  const leaseToken = normalizeString(result.leaseToken);
  if (!taskId || !leaseToken) return null;
  const lease = {
    taskId,
    leaseToken,
    expiresAt: normalizeString(result.leaseExpiresAt || result.expiresAt),
  };
  const attemptId = normalizeString(result.attemptId);
  const leaseEpoch = toOptionalInteger(result.leaseEpoch);
  if (attemptId) lease.attemptId = attemptId;
  if (leaseEpoch !== undefined) lease.leaseEpoch = leaseEpoch;
  return lease;
}

function firstReservation(data = {}) {
  return Array.isArray(data?.reservations) && data.reservations.length > 0
    ? data.reservations[0]
    : null;
}

async function startReservationThroughSync({
  rawData = {},
  serverUrl = '',
  stationId = '',
  stationToken = '',
  authorizationToken = '',
  capabilities = [],
  platformAccounts = [],
  pluginVersion = '',
  stationSessionId = '',
  fetchFn,
} = {}) {
  const reservation = firstReservation(rawData);
  if (!reservation?.jobId || !reservation?.reserveToken) return rawData;

  const mailboxVersions = extractMailboxVersionsFromResponse(rawData);
  const operationId = `start_${normalizeString(reservation.jobId)}_${Date.now()}`;
  const platformAccountId = normalizeString(
    reservation.platformAccountId
    || reservation.reservedPlatformAccountId
    || reservation.taskSpec?.platformAccountId
    || reservation.taskSpec?.reservedPlatformAccountId,
  );
  const body = buildSyncRequestV11({
    stationId,
    stationToken,
    pluginVersion,
    stationSessionId,
    capabilities,
    platformAccounts,
    mailboxStationVersion: mailboxVersions.station,
    mailboxLaneVersions: mailboxVersions.lanes,
    includeCapacity: false,
    operations: [{
      operationId,
      type: 'start_job',
      jobId: normalizeString(reservation.jobId),
      reserveToken: normalizeString(reservation.reserveToken),
      reservationEpoch: toOptionalInteger(reservation.reservationEpoch),
      platformAccountId,
      startedAt: new Date().toISOString(),
    }],
  });

  const startData = await postJson({
    serverUrl,
    path: '/api/execution-stations/sync',
    fetchFn,
    authorizationToken,
    body,
  });
  const result = startData?.operationResults?.[operationId] || null;
  if (result?.status !== 'accepted') {
    return {
      ...startData,
      claim: {
        task: null,
        reason: {
          code: normalizeString(result?.reason || 'start_job_rejected'),
          message: normalizeString(result?.reason || '预留任务未能开始执行。'),
        },
        nextPollAfterMs: nextPollAfterMsFromSync(startData) || 30000,
      },
    };
  }

  const lease = buildLeaseFromStartResult(reservation, result);
  return {
    ...startData,
    reservation,
    claim: {
      task: buildTaskFromReservation(reservation),
      lease,
      attempt: {
        attemptId: normalizeString(result.attemptId),
      },
    },
  };
}

function normalizeStationSyncClaimResponse(data = {}) {
  if (!isStationSyncEnvelope(data)) return data;
  const mailboxVersion = extractMailboxVersion(data);
  if (data?.claim) {
    return {
      ...data.claim,
      sync: data,
      dispatch: data,
      heartbeat: data.heartbeat || null,
      reconcile: data.reconcile || null,
      mailboxVersion,
    };
  }

  const reconcile = data?.reconcile || null;
  const action = normalizeString(reconcile?.action || '');
  if (action === 'resume') {
    return {
      task: null,
      lease: null,
      resumeLease: normalizeLeaseSnapshot({
        task: reconcile?.task,
        lease: reconcile?.lease || reconcile?.serverLease,
      }),
      reason: {
        code: 'server_task_resume_required',
        message: '服务端已有执行中任务，先恢复本地任务后再继续。',
      },
      nextPollAfterMs: 1000,
      mailboxVersion,
      sync: data,
      dispatch: data,
      heartbeat: data.heartbeat || null,
      reconcile,
    };
  }

  const isClearLocal = action === 'clear_local';
  return {
    task: null,
    clearLocalLease: isClearLocal,
    reason: {
      code: isClearLocal ? 'server_cleared_local_task' : 'no_pending_task',
      message: isClearLocal
        ? '服务端没有当前任务，本地任务状态已清理。'
        : '当前没有可领取任务。',
    },
    nextPollAfterMs: toFiniteNumber(data?.nextSyncAfterMs, 0),
    mailboxVersion,
    sync: data,
    dispatch: data,
    heartbeat: data.heartbeat || null,
    reconcile,
  };
}

async function postJson({
  serverUrl = '',
  path = '',
  body = {},
  fetchFn = globalThis.fetch?.bind(globalThis),
  authorizationToken = '',
  /** V1.1（2026-06-29）：请求超时（毫秒），防止 fetch 永久挂起阻塞 tick。 */
  timeoutMs = 30000,
} = {}) {
  if (typeof fetchFn !== 'function') throw new Error('fetch unavailable');
  const headers = { 'Content-Type': 'application/json' };
  const normalizedAuthorizationToken = normalizeString(authorizationToken);
  if (normalizedAuthorizationToken) {
    headers.Authorization = `Bearer ${normalizedAuthorizationToken}`;
  }
  const controller = typeof AbortController !== 'undefined' ? new AbortController() : null;
  const timer = controller ? setTimeout(() => controller.abort(), timeoutMs) : null;
  const response = await fetchFn(`${normalizeServerUrl(serverUrl, DEFAULT_SERVER_URL)}${path}`, {
    method: 'POST',
    headers,
    body: JSON.stringify(body),
    signal: controller?.signal ?? undefined,
  });
  if (timer) clearTimeout(timer);
  if (!response.ok) {
    const text = await readErrorText(response);
    const parsed = parseErrorBody(text);
    const headerRetryAfterMs = parseRetryAfterHeader(response.headers?.get?.('Retry-After'));
    const bodyRetryAfterMs = toFiniteNumber(parsed?.retryAfterMs, 0)
      || toFiniteNumber(parsed?.retryAfterSeconds, 0) * 1000;
    throw createHttpError(errorMessageFromResponseText(text, `HTTP ${response.status}`), {
      status: response.status,
      reasonCode: parsed?.code || parsed?.reasonCode || '',
      nextPollAfterMs: headerRetryAfterMs || bodyRetryAfterMs,
    });
  }
  return response.json().catch(() => ({}));
}

export function createTaskLeaseMemoryStore(initial = null) {
  let lease = initial ? { ...initial } : null;
  return {
    async read() {
      return lease ? { ...lease } : null;
    },
    async write(next) {
      lease = next ? { ...next } : null;
      return lease;
    },
    async clear() {
      lease = null;
    },
  };
}

export function createTaskLeaseStorageStore({
  storageArea = globalThis.chrome?.storage?.local,
  storageKey = DEFAULT_LEASE_STORAGE_KEY,
} = {}) {
  return {
    async read() {
      if (!storageArea?.get) return null;
      const data = await storageArea.get(storageKey);
      const lease = data?.[storageKey] || null;
      return lease ? { ...lease } : null;
    },
    async write(next) {
      if (!storageArea?.set) return next ? { ...next } : null;
      const lease = next ? { ...next } : null;
      await storageArea.set({ [storageKey]: lease });
      return lease;
    },
    async clear() {
      if (storageArea?.remove) {
        await storageArea.remove(storageKey);
        return;
      }
      if (storageArea?.set) {
        await storageArea.set({ [storageKey]: null });
      }
    },
  };
}

export async function claimCollectionTaskLease({
  serverUrl = '',
  stationId = '',
  stationToken = '',
  authorizationToken = '',
  capabilities = [],
  platformAccounts = [],
  pluginVersion = '',
  fetchFn,
  store = null,
  storageArea = globalThis.chrome?.storage?.local,
} = {}) {
  const localLease = typeof store?.read === 'function' ? await store.read() : null;
  const mailboxVersion = extractMailboxVersion(localLease || {});
  // V1.1：持久化的 lane 版本（如果 store 里存了，使用它构造 mailboxCursors）
  const mailboxLaneVersions = localLease && typeof localLease === 'object'
    && localLease.mailboxLaneVersions && typeof localLease.mailboxLaneVersions === 'object'
    ? localLease.mailboxLaneVersions
    : {};
  const stationSessionId = await resolveStationSessionId({ storageArea });

  // V1.1 envelope body
  const requestBody = buildSyncRequestV11({
    stationId,
    stationToken,
    pluginVersion,
    stationSessionId,
    capabilities,
    platformAccounts,
    localLease: localLease && typeof localLease === 'object'
      ? normalizeLeaseSnapshot({ fallback: localLease })
      : null,
    mailboxStationVersion: mailboxVersion,
    mailboxLaneVersions,
  });

  const initialRawData = await postJson({
    serverUrl,
    path: '/api/execution-stations/sync',
    fetchFn,
    authorizationToken,
    body: requestBody,
  });
  const rawData = await startReservationThroughSync({
    rawData: initialRawData,
    serverUrl,
    stationId,
    stationToken,
    authorizationToken,
    capabilities,
    platformAccounts,
    pluginVersion,
    stationSessionId,
    fetchFn,
  });
  const data = normalizeStationSyncClaimResponse(rawData);
  const responseMailboxVersion = extractMailboxVersion(data);
  // V1.1 响应里的 lane 版本（如果有），持久化以便下次构造 cursor
  const responseMailboxVersions = extractMailboxVersionsFromResponse(rawData);
  const responseLaneVersions = responseMailboxVersions.lanes || {};

  if (data?.task && data?.lease && store?.write) {
    await store.write(attachMailboxLaneVersions(normalizeLeaseSnapshot({
      task: data.task,
      lease: data.lease,
      attempt: data.attempt,
    }), responseMailboxVersion, responseLaneVersions));
  } else if (data?.resumeLease?.taskId && data?.resumeLease?.leaseToken && store?.write) {
    await store.write(attachMailboxLaneVersions(data.resumeLease, responseMailboxVersion, responseLaneVersions));
  } else if (store?.write) {
    const idleSnapshot = createTaskLeaseIdleSnapshot(data);
    if (idleSnapshot) {
      await store.write(attachMailboxLaneVersions(idleSnapshot, responseMailboxVersion, responseLaneVersions));
    } else if (store?.clear) {
      await store.clear();
    }
  }

  return data;
}

export async function commitCollectionTaskDeltaThroughSync({
  serverUrl = '',
  taskId = '',
  envelope = {},
  stationId = '',
  stationToken = '',
  authorizationToken = '',
  capabilities = [],
  platformAccounts = [],
  pluginVersion = '',
  fetchFn,
  store = null,
  storageArea = globalThis.chrome?.storage?.local,
} = {}) {
  const normalizedTaskId = normalizeString(taskId || envelope?.taskId);
  if (!normalizedTaskId) {
    throw createHttpError('taskId required', { status: 400, reasonCode: 'task_id_required' });
  }
  const leaseToken = normalizeString(envelope?.leaseToken);
  const leaseEpoch = toOptionalInteger(envelope?.leaseEpoch);
  const attemptId = normalizeString(envelope?.attemptId);
  const nowIso = new Date().toISOString();
  const observedAt = operationObservedAt(envelope, nowIso);
  const inputRecordCount = Array.isArray(envelope?.records) ? envelope.records.length : 0;
  const records = rawRecordsFromEnvelope(envelope, observedAt);
  const droppedRecordCount = Math.max(0, inputRecordCount - records.length);
  const shouldCommitRawSnapshot = records.length > 0 || isTerminalSnapshotStatus(envelope?.snapshot?.status);
  const operationId = `${shouldCommitRawSnapshot ? 'commit' : 'progress'}_${normalizedTaskId}_${Date.now()}`;
  const resultSummary = isPlainObject(envelope?.resultSummary)
    ? envelope.resultSummary
    : isPlainObject(envelope?.snapshot?.latestSummary)
      ? envelope.snapshot.latestSummary
      : undefined;

  const existingLease = typeof store?.read === 'function' ? await store.read() : null;
  const mailboxVersion = extractMailboxVersion(existingLease || {});
  const mailboxLaneVersions = existingLease && typeof existingLease === 'object'
    && existingLease.mailboxLaneVersions && typeof existingLease.mailboxLaneVersions === 'object'
    ? existingLease.mailboxLaneVersions
    : {};
  const stationSessionId = await resolveStationSessionId({ storageArea });
  const localLease = normalizeLeaseSnapshot({
    fallback: {
      taskId: normalizedTaskId,
      leaseToken,
      attemptId,
      leaseEpoch,
    },
  });

  const operations = shouldCommitRawSnapshot
    ? [{
        operationId,
        type: 'commit_raw_snapshot',
        jobId: normalizedTaskId,
        attemptId,
        leaseToken,
        leaseEpoch,
        captureId: captureIdForEnvelope(normalizedTaskId, envelope),
        expectedTargetKey: normalizeString(envelope?.executionContext?.expectedTargetKey || ''),
        observedTargetKey: normalizeString(envelope?.executionContext?.observedTargetKey || ''),
        observedAt,
        ...(resultSummary ? { resultSummary } : {}),
        records,
      }]
    : [{
        operationId,
        type: 'progress_update',
        jobId: normalizedTaskId,
        leaseToken,
        leaseEpoch,
        progress: clampCaptureProgress(envelope?.snapshot?.progress),
        stage: normalizeString(envelope?.snapshot?.status || 'running'),
        observedAt,
      }];

  const body = buildSyncRequestV11({
    stationId,
    stationToken,
    pluginVersion,
    stationSessionId,
    capabilities,
    platformAccounts,
    mailboxStationVersion: mailboxVersion,
    mailboxLaneVersions,
    includeCapacity: false,
    localLease,
    operations,
  });

  const data = await postJson({
    serverUrl,
    path: '/api/execution-stations/sync',
    fetchFn,
    authorizationToken,
    body,
  });
  const result = data?.operationResults?.[operationId] || {};
  if (result?.status !== 'accepted') {
    const rejectionReason = normalizeString(result.reason || 'sync_operation_rejected');
    // V1.1（2026-06-29）：区分永久性拒绝（LEASE_EXPIRED/IDENTITY_MISMATCH 等）
    // 和临时性拒绝。永久拒绝不应无限重试。
    const permanentRejections = new Set([
      'lease_epoch_mismatch',
      'lease_token_mismatch',
      'status_not_in_progress',
      'identity_mismatch',
      'job_not_found',
      'queue_entry_not_found',
      'station_workspace_mismatch',
      'station_not_found',
      'job_workspace_mismatch',
      'workspace_required',
      'invalid_records',
    ]);
    const isPermanent = permanentRejections.has(rejectionReason);
    throw createHttpError(rejectionReason, {
      status: isPermanent ? 410 : 409,
      reasonCode: rejectionReason,
      retryable: !isPermanent,
      nextPollAfterMs: isPermanent ? null : (nextPollAfterMsFromSync(data) || 30000),
    });
  }

  return {
    success: true,
    ...acceptedDeltaKeys(envelope),
    clientRecordStats: {
      inputRecordCount,
      committedRecordCount: records.length,
      droppedRecordCount,
      dropReason: droppedRecordCount > 0 ? 'missing_idempotency_key_or_invalid_record' : '',
    },
    operationResult: result,
    sync: data,
  };
}

export async function syncCollectionTaskStatusThroughSync({
  serverUrl = '',
  taskId = '',
  patch = {},
  stationId = '',
  stationToken = '',
  authorizationToken = '',
  capabilities = [],
  platformAccounts = [],
  pluginVersion = '',
  fetchFn,
  store = null,
  storageArea = globalThis.chrome?.storage?.local,
} = {}) {
  const normalizedTaskId = normalizeString(taskId);
  if (!normalizedTaskId) {
    throw createHttpError('taskId required', { status: 400, reasonCode: 'task_id_required' });
  }
  const existingLease = typeof store?.read === 'function' ? await store.read() : null;
  const mailboxVersion = extractMailboxVersion(existingLease || {});
  const mailboxLaneVersions = existingLease && typeof existingLease === 'object'
    && existingLease.mailboxLaneVersions && typeof existingLease.mailboxLaneVersions === 'object'
    ? existingLease.mailboxLaneVersions
    : {};
  const leaseAuth = normalizeLeaseSnapshot({ fallback: existingLease || {} });
  const stationSessionId = await resolveStationSessionId({ storageArea });
  const status = normalizeString(patch?.status);
  const operationId = `status_${normalizedTaskId}_${Date.now()}`;
  const leaseEpoch = toOptionalInteger(patch?.leaseEpoch ?? leaseAuth.leaseEpoch);
  const leaseToken = normalizeString(patch?.leaseToken || leaseAuth.leaseToken);
  if (!leaseToken) {
    return {
      success: true,
      skipped: true,
      reason: 'missing_lease_for_v11_status_sync',
      task: { id: normalizedTaskId, status },
      event: null,
    };
  }

  const isTerminalFailure = status === 'failed' || status === 'stopped' || status === 'cancelled';
  const isRequeue = status === 'pending' || status === 'queued';
  const shouldReleaseLease = (isTerminalFailure || isRequeue) && leaseToken && patch?.deferRelease !== true;
  const operations = shouldReleaseLease
    ? [{
        operationId,
        type: 'release_job',
        jobId: normalizedTaskId,
        leaseToken,
        leaseEpoch,
        reasonCode: normalizeString(patch?.reasonCode || patch?.errorCode || patch?.errorMessage || status || 'station_status_update'),
        retryable: isRequeue,
      }]
    : [{
        operationId,
        type: 'progress_update',
        jobId: normalizedTaskId,
        leaseToken,
        leaseEpoch,
        progress: clampCaptureProgress(patch?.progress),
        stage: status || 'running',
        observedAt: new Date().toISOString(),
      }];

  const body = buildSyncRequestV11({
    stationId,
    stationToken,
    pluginVersion,
    stationSessionId,
    capabilities,
    platformAccounts,
    mailboxStationVersion: mailboxVersion,
    mailboxLaneVersions,
    includeCapacity: false,
    localLease: leaseAuth.taskId ? leaseAuth : null,
    operations,
  });

  const data = await postJson({
    serverUrl,
    path: '/api/execution-stations/sync',
    fetchFn,
    authorizationToken,
    body,
  });
  const result = data?.operationResults?.[operationId] || {};
  if (result?.status !== 'accepted') {
    throw createHttpError(normalizeString(result.reason || result.message || 'sync_status_rejected'), {
      status: 409,
      reasonCode: normalizeString(result.reason || 'sync_status_rejected'),
      nextPollAfterMs: nextPollAfterMsFromSync(data) || 30000,
    });
  }

  return {
    success: true,
    task: {
      id: normalizedTaskId,
      status,
      progress: patch?.progress,
      pluginRunId: patch?.pluginRunId,
    },
    event: null,
    operationResult: result,
    sync: data,
  };
}

export async function renewCollectionTaskLease({
  serverUrl = '',
  taskId = '',
  stationId = '',
  stationToken = '',
  leaseToken = '',
  authorizationToken = '',
  status = 'running',
  attemptId = '',
  leaseEpoch,
  attemptNumber,
  pluginVersion = '',
  fetchFn,
  store = null,
  storageArea = globalThis.chrome?.storage?.local,
} = {}) {
  const normalizedTaskId = normalizeString(taskId);
  const leaseAuth = normalizeLeaseSnapshot({
    fallback: {
      taskId: normalizedTaskId,
      leaseToken,
      attemptId,
      leaseEpoch,
      attemptNumber,
    },
  });
  const existingLease = typeof store?.read === 'function' ? await store.read() : null;
  const mailboxVersion = extractMailboxVersion(existingLease || {});
  const mailboxLaneVersions = existingLease && typeof existingLease === 'object'
    && existingLease.mailboxLaneVersions && typeof existingLease.mailboxLaneVersions === 'object'
    ? existingLease.mailboxLaneVersions
    : {};
  const stationSessionId = await resolveStationSessionId({ storageArea });
  const operationId = `progress_${normalizedTaskId}_${Date.now()}`;
  const body = buildSyncRequestV11({
    stationId,
    stationToken,
    pluginVersion,
    stationSessionId,
    mailboxStationVersion: mailboxVersion,
    mailboxLaneVersions,
    includeCapacity: false,
    localLease: leaseAuth,
    operations: [{
      operationId,
      type: 'progress_update',
      jobId: normalizedTaskId,
      leaseToken: normalizeString(leaseToken),
      leaseEpoch: toOptionalInteger(leaseEpoch),
      progress: normalizeString(status) === 'running' ? 50 : undefined,
      stage: normalizeString(status) || 'running',
    }],
  });

  const data = await postJson({
    serverUrl,
    path: '/api/execution-stations/sync',
    fetchFn,
    authorizationToken,
    body,
  });
  const result = data?.operationResults?.[operationId] || {};
  if (result?.status === 'rejected') {
    throw createHttpError(normalizeString(result.reason || 'lease_renew_rejected'), {
      status: 409,
      reasonCode: normalizeString(result.reason || 'lease_renew_rejected'),
    });
  }
  const responseMailboxVersion = extractMailboxVersion(data);
  const responseMailboxVersions = extractMailboxVersionsFromResponse(data);
  const responseLaneVersions = responseMailboxVersions.lanes || {};
  const nextLease = normalizeLeaseSnapshot({
    fallback: {
      ...leaseAuth,
      expiresAt: result.leaseExpiresAt || result.expiresAt,
      leaseEpoch: result.leaseEpoch ?? leaseAuth.leaseEpoch,
    },
  });

  if (store?.write && nextLease.expiresAt) {
    await store.write(attachMailboxLaneVersions(nextLease, responseMailboxVersion, responseLaneVersions));
  }

  return {
    success: result?.status === 'accepted',
    ...result,
    expiresAt: result.leaseExpiresAt || result.expiresAt,
    sync: data,
    mailboxVersions: responseMailboxVersions,
  };
}

export async function reconcileExecutionStationLease({
  serverUrl = '',
  stationId = '',
  stationToken = '',
  authorizationToken = '',
  localLease = null,
  capabilities = [],
  platformAccounts = [],
  pluginVersion = '',
  fetchFn,
  store = null,
  storageArea = globalThis.chrome?.storage?.local,
} = {}) {
  const normalizedLocalLease = localLease && typeof localLease === 'object'
    ? {
        ...normalizeLeaseSnapshot({ fallback: localLease }),
      }
    : null;

  const mailboxVersion = extractMailboxVersion(localLease || {});
  const mailboxLaneVersions = localLease && typeof localLease === 'object'
    && localLease.mailboxLaneVersions && typeof localLease.mailboxLaneVersions === 'object'
    ? localLease.mailboxLaneVersions
    : {};
  const stationSessionId = await resolveStationSessionId({ storageArea });

  // V1.1 envelope body
  const body = buildSyncRequestV11({
    stationId,
    stationToken,
    pluginVersion,
    stationSessionId,
    capabilities,
    platformAccounts,
    localLease: normalizedLocalLease,
    mailboxStationVersion: mailboxVersion,
    mailboxLaneVersions,
    includeCapacity: false,
  });

  const rawData = await postJson({
    serverUrl,
    path: '/api/execution-stations/sync',
    fetchFn,
    authorizationToken,
    body,
  });
  const data = isStationSyncEnvelope(rawData) ? rawData.reconcile || {} : rawData;
  const responseMailboxVersion = extractMailboxVersion(rawData);
  const responseMailboxVersions = extractMailboxVersionsFromResponse(rawData);
  const responseLaneVersions = responseMailboxVersions.lanes || {};

  const action = normalizeString(data?.action || data?.status || '');
  const serverLease = data?.lease || data?.serverLease || null;
  const normalizedServerLease = normalizeLeaseSnapshot({
    task: data?.task,
    lease: serverLease || data,
    attempt: data?.attempt,
    fallback: {
      taskId: data?.taskId,
      leaseToken: data?.leaseToken,
      expiresAt: data?.expiresAt,
      attemptId: data?.attemptId,
      leaseEpoch: data?.leaseEpoch,
    },
  });
  const taskId = normalizeString(normalizedServerLease.taskId);
  const leaseToken = normalizeString(normalizedServerLease.leaseToken);

  if (store?.write && taskId && leaseToken) {
    await store.write(attachMailboxLaneVersions(normalizedServerLease, responseMailboxVersion, responseLaneVersions));
  } else if (
    store?.write &&
    ['clear_local', 'idle', 'release', 'released', 'expired'].includes(action)
  ) {
    const idleSnapshot = createTaskLeaseIdleSnapshot({
      ...normalizeStationSyncClaimResponse(rawData),
      mailboxVersion: responseMailboxVersion,
    });
    if (idleSnapshot) {
      await store.write(attachMailboxLaneVersions(idleSnapshot, responseMailboxVersion, responseLaneVersions));
    } else if (store?.clear) {
      await store.clear();
    }
  }

  return {
    success: true,
    ...data,
    sync: isStationSyncEnvelope(rawData) ? rawData : null,
    mailbox: isStationSyncEnvelope(rawData) ? rawData.mailbox || null : null,
    mailboxVersions: responseMailboxVersions,
    action,
    lease: taskId && leaseToken
      ? attachMailboxLaneVersions(normalizedServerLease, responseMailboxVersion, responseLaneVersions)
      : null,
  };
}
