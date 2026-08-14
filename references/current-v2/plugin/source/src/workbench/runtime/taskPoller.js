import {
  REMOTE_ERROR_CODE,
  REMOTE_TASK_CONTROL_ACTION,
  WORKBENCH_EVENT_SOURCE,
  WORKBENCH_RECORD_TYPE,
  WORKBENCH_TASK_EVENT_TYPE,
} from '../protocol/schema.js';
import { createTaskLeaseIdleSnapshot } from './taskLeaseClient.js';
import { attachTaskRuntimeObservability } from './taskRuntimeObservability.js';
import { resolveCollectionProfile } from './collectionProfile.js';
import { parseTargetIdentity } from '../../shared/targetIdentity.js';
import xhsTerminalMapper from '../protocol/v2/xhs-terminal-mapper.cjs';

const { mapRuntimeTerminalToCaptureSubmissionV2 } = xhsTerminalMapper;

function deepClone(obj) {
  if (obj === null || typeof obj !== 'object') return obj;
  try {
    return structuredClone(obj);
  } catch {
    return JSON.parse(JSON.stringify(obj));
  }
}

function buildStartupPatch({ pluginRunId = '', activeExecutor = '' } = {}) {
  const normalizedRunId = String(pluginRunId || '').trim();
  return {
    status: normalizedRunId ? 'running' : 'dispatched',
    progress: normalizedRunId ? 10 : 5,
    pluginRunId: normalizedRunId || null,
    activeExecutor: String(activeExecutor || '').trim() || null,
    latestHeartbeatAt: new Date().toISOString(),
    errorMessage: null,
  };
}

const DISPATCH_STARTUP_TIMEOUT_MS = 45 * 1000;
const DISPATCH_STARTUP_RETRY_DELAY_MS = 2 * 60 * 1000;
const MAX_TASK_EXECUTION_TIME_MS = 30 * 60 * 1000; // 30 分钟硬超时
const PAUSED_TASK_AUTO_RELEASE_MS = 10 * 60 * 1000; // 暂停 10 分钟自动释放
const RUNNING_RESULT_LOOKUP_TIMEOUT_MS = 12 * 60 * 1000;
const LOCAL_ACTIVE_TASK_WITHOUT_LEASE_TIMEOUT_MS = 5 * 60 * 1000;
const LOCAL_ACTIVE_TASK_WITHOUT_LEASE_RETRY_DELAY_MS = 2 * 60 * 1000;
const LEASE_CONFLICT_RETRY_DELAY_MS = 2 * 60 * 1000;
const RECONCILE_IDLE_INTERVAL_MS = 60 * 1000;
const AUTHORIZATION_FAILURE_IDLE_MS = 15 * 60 * 1000;
const TICK_STALE_TIMEOUT_MS = 2 * 60 * 1000;

function isRecoverableConnectionError(error) {
  const msg = String(error?.message || error || '');
  return /Could not establish connection|Receiving end does not exist|context invalidated|The message port closed|sendToTab timeout|Cannot access contents|Extension manifest must request permission/i.test(msg);
}

function isLeaseConflictError(error) {
  const status = Number(error?.status || 0);
  const msg = String(error?.message || error || '');
  return status === 409 || /LEASE_CONFLICT|held by another station|lease is held/i.test(msg);
}

function isAuthorizationFailureError(error) {
  return [401, 403].includes(Number(error?.status || 0));
}

function isMissingServerTaskError(error) {
  return Number(error?.status || 0) === 404;
}

function isRecordSchemaValidationError(error) {
  return (
    error?.retryable === false
    && Array.isArray(error?.validationErrors)
    && error?.observability?.recordSchemaFailed === true
  );
}

function buildRecordSchemaFailureMessage(error) {
  const firstError = Array.isArray(error?.validationErrors) ? error.validationErrors[0] : null;
  return String(
    firstError?.message
    || error?.message
    || '采集结果结构不合格，已停止本轮同步。',
  ).trim();
}

function isMonitorTask(task = {}) {
  const source = String(task?.source || '').trim();
  const strategy = String(task?.taskStrategy || task?.payload?.taskStrategy || '').trim();
  return (
    source === 'monitor' ||
    Boolean(task?.payload?.monitorId) ||
    strategy === 'author_baseline' ||
    strategy === 'author_patrol' ||
    strategy === 'keyword_patrol' ||
    strategy === 'detail_probe' ||
    strategy === 'deep_collect'
  );
}

function recoverableConnectionStatusForTask(task = {}) {
  return isMonitorTask(task)
    ? {
        status: 'failed',
        progress: 100,
        eventType: WORKBENCH_TASK_EVENT_TYPE.TASK_FAILED,
        message: '页面连接中断，本轮监控已结束，后续会自动重试',
      }
    : {
        status: 'paused',
        progress: 5,
        eventType: WORKBENCH_TASK_EVENT_TYPE.TASK_PAUSED,
        message: '页面连接中断，已自动暂停，请点击恢复继续',
      };
}

function parseTimestamp(value) {
  if (!value) return 0;
  if (value instanceof Date) return value.getTime();
  if (typeof value === 'number') return Number.isFinite(value) ? value : 0;
  const parsed = new Date(value).getTime();
  return Number.isFinite(parsed) ? parsed : 0;
}

function buildRunningProgress(run = {}) {
  const planned = Number(run?.resultSummary?.itemsPlanned || 0);
  const succeeded = Number(run?.resultSummary?.itemsSucceeded || 0);
  const failed = Number(run?.resultSummary?.failedItems || 0);
  if (planned > 0) {
    const ratio = Math.min(0.95, Math.max(0.1, (succeeded + failed) / planned));
    return Math.round(ratio * 100);
  }
  return 50;
}

function mapRunStatusToWorkbenchStatus(status = '') {
  const normalized = String(status || '').trim().toLowerCase();
  if (normalized === 'done') return { status: 'completed', final: true, progress: 100 };
  if (normalized === 'stopped') return { status: 'stopped', final: true, progress: null };
  if (normalized === 'failed' || normalized === 'canceled' || normalized === 'rejected') {
    return { status: 'failed', final: true, progress: 100 };
  }
  if (normalized === 'paused') {
    return { status: 'paused', final: false, progress: null };
  }
  if (normalized === 'stopping') return { status: 'stopping', final: false, progress: null };
  if (normalized === 'running' || normalized === 'accepted') {
    return { status: 'running', final: false, progress: null };
  }
  return { status: 'dispatched', final: false, progress: 5 };
}

function mapWorkbenchStatusToEventType(status = '') {
  switch (String(status || '').trim()) {
    case 'completed':
      return WORKBENCH_TASK_EVENT_TYPE.TASK_SUCCEEDED;
    case 'stopped':
      return WORKBENCH_TASK_EVENT_TYPE.TASK_RELEASED;
    case 'failed':
      return WORKBENCH_TASK_EVENT_TYPE.TASK_FAILED;
    case 'paused':
      return WORKBENCH_TASK_EVENT_TYPE.TASK_PAUSED;
    case 'stopping':
      return WORKBENCH_TASK_EVENT_TYPE.TASK_STOPPING;
    case 'running':
      return WORKBENCH_TASK_EVENT_TYPE.TASK_PROGRESS;
    default:
      return WORKBENCH_TASK_EVENT_TYPE.TASK_HEARTBEAT;
  }
}

function isTerminalControlAction(action = '') {
  const normalized = String(action || '').trim();
  return normalized === REMOTE_TASK_CONTROL_ACTION.STOP
    || normalized === REMOTE_TASK_CONTROL_ACTION.DELETE;
}

function mapControlActionToStateEvent(action = '') {
  switch (String(action || '').trim()) {
    case REMOTE_TASK_CONTROL_ACTION.PAUSE:
      return WORKBENCH_TASK_EVENT_TYPE.TASK_PAUSED;
    case REMOTE_TASK_CONTROL_ACTION.RESUME:
      return WORKBENCH_TASK_EVENT_TYPE.TASK_RESUMED;
    case REMOTE_TASK_CONTROL_ACTION.STOP:
    case REMOTE_TASK_CONTROL_ACTION.DELETE:
      return WORKBENCH_TASK_EVENT_TYPE.TASK_STOPPING;
    default:
      return WORKBENCH_TASK_EVENT_TYPE.TASK_HEARTBEAT;
  }
}

function toFiniteNumber(value, fallback = 0) {
  const num = Number(value);
  return Number.isFinite(num) ? num : fallback;
}

function toOptionalInteger(value) {
  const num = Number(value);
  return Number.isFinite(num) ? Math.floor(num) : undefined;
}

function normalizeLeaseSnapshot(lease = {}) {
  const snapshot = {
    leaseToken: String(lease?.leaseToken || '').trim(),
    expiresAt: String(lease?.expiresAt || '').trim(),
  };
  const attemptId = String(lease?.attemptId || '').trim();
  const captureId = String(lease?.captureId || '').trim();
  const executionPlanVersion = String(lease?.executionPlanVersion || '').trim();
  const leaseEpoch = toOptionalInteger(lease?.leaseEpoch);
  const attemptNumber = toOptionalInteger(lease?.attemptNumber);
  if (attemptId) snapshot.attemptId = attemptId;
  if (captureId) snapshot.captureId = captureId;
  if (executionPlanVersion) snapshot.executionPlanVersion = executionPlanVersion;
  if (leaseEpoch !== undefined) snapshot.leaseEpoch = leaseEpoch;
  if (attemptNumber !== undefined) snapshot.attemptNumber = attemptNumber;
  return snapshot;
}

function isTerminalReadinessReason(reasonCode = '') {
  return new Set([
    REMOTE_ERROR_CODE.CONTENT_NOT_FOUND,
    REMOTE_ERROR_CODE.ERROR_PAGE,
    REMOTE_ERROR_CODE.PAGE_PERMISSION_DENIED,
  ]).has(String(reasonCode || '').trim());
}

function isCommentDetailTask(task = {}) {
  const taskType = String(task?.taskType || '').trim();
  if (!['xhs.batchComments', 'douyin.batchComments', 'douyin.singleComments'].includes(taskType)) {
    return false;
  }
  const payload = task?.payload && typeof task.payload === 'object' && !Array.isArray(task.payload)
    ? task.payload
    : {};
  const strategy = String(task?.taskStrategy || payload.taskStrategy || '').trim();
  return strategy === 'detail_probe'
    || Boolean(payload.noteId)
    || Boolean(payload.platformContentId)
    || Boolean(payload.awemeId);
}

function shouldFailCapabilityRejection(reasonCode = '', capability = {}) {
  const normalizedReasonCode = String(reasonCode || '').trim();
  if (isTerminalReadinessReason(normalizedReasonCode)) return true;
  const readinessReasonCode = String(capability?.report?.readiness?.reasonCode || '').trim();
  if (isTerminalReadinessReason(readinessReasonCode)) return true;
  return (
    normalizedReasonCode === REMOTE_ERROR_CODE.UNSUPPORTED_TASK_TYPE &&
    isCommentDetailTask(capability?.task)
  );
}

function shouldPreserveTaskExecutionContextForCapabilityRejection(reasonCode = '') {
  return new Set([
    REMOTE_ERROR_CODE.LOGIN_REQUIRED,
    REMOTE_ERROR_CODE.LOGIN_EXPIRED,
    REMOTE_ERROR_CODE.PLATFORM_SECURITY_CHALLENGE,
  ]).has(String(reasonCode || '').trim());
}

function buildCapabilityReportDiagnostic(report = {}) {
  if (!report || typeof report !== 'object' || Array.isArray(report)) return {};
  const readiness = report.readiness && typeof report.readiness === 'object' ? report.readiness : {};
  const canRunTaskTypes = Array.isArray(report?.capabilities?.canRunTaskTypes)
    ? report.capabilities.canRunTaskTypes.map((item) => String(item || '').trim()).filter(Boolean)
    : [];
  const diagnostic = {
    reportUrl: String(report.url || '').trim(),
    reportMode: String(report.mode || '').trim(),
    reportPageType: String(report.pageType || '').trim(),
    readinessReady: readiness.ready === undefined ? undefined : Boolean(readiness.ready),
    readinessReasonCode: String(readiness.reasonCode || '').trim(),
    readinessReasonMessage: String(readiness.reasonMessage || '').trim(),
    capabilityTaskTypes: canRunTaskTypes.slice(0, 20),
  };
  return Object.fromEntries(
    Object.entries(diagnostic).filter(([, value]) => value !== undefined && value !== '' && (!Array.isArray(value) || value.length > 0)),
  );
}

function normalizeCapabilityRejection(capability = {}) {
  const reasonCode = String(capability?.reasonCode || capability?.error || 'capability_rejected').trim()
    || 'capability_rejected';
  const reasonMessage = firstNonEmptyText(
    capability?.reasonMessage,
    capability?.error,
    '当前页面暂时不能执行这个任务',
  );
  const shouldFail = shouldFailCapabilityRejection(reasonCode, capability);
  const payload = {
    status: shouldFail ? 'failed' : 'pending',
    reason: 'capability_rejected',
    reasonCode,
    errorMessage: reasonMessage,
    userMessage: reasonMessage,
    taskType: String(capability?.taskType || '').trim(),
    ...buildCapabilityReportDiagnostic(capability?.report),
  };
  if (capability?.recommendedAction) {
    payload.recommendedAction = String(capability.recommendedAction || '').trim();
  }
  if (reasonCode === REMOTE_ERROR_CODE.PAGE_TARGET_MISMATCH || /target_mismatch/i.test(reasonCode)) {
    payload.errorCode = 'TARGET_MISMATCH';
  }
  return {
    reasonCode,
    reasonMessage,
    payload,
    taskStatus: shouldFail ? 'failed' : 'pending',
    progress: shouldFail ? 100 : 0,
  };
}

function buildLeaseEventFields({ task = {}, lease = null, executorInstanceId = '', accountId = '' } = {}) {
  const fields = {
    attemptId: String(lease?.attemptId || task?.currentAttemptId || task?.attemptId || '').trim(),
    leaseId: String(lease?.leaseToken || '').trim(),
    leaseEpoch: toOptionalInteger(lease?.leaseEpoch ?? task?.leaseEpoch),
    stationId: String(executorInstanceId || task?.executorInstanceId || '').trim(),
    accountId: String(accountId || task?.accountId || '').trim(),
    platform: String(task?.platform || '').trim(),
  };
  return Object.fromEntries(Object.entries(fields).filter(([, value]) => value !== undefined && value !== ''));
}

function buildDispatchFailurePayload({
  reason = 'dispatch_failed',
  errorMessage = '',
  reasonCode = '',
  message = '',
} = {}) {
  const resolvedMessage = firstNonEmptyText(errorMessage, message, '派发任务失败');
  const payload = {
    status: 'failed',
    reason,
    errorMessage: resolvedMessage,
    userMessage: firstNonEmptyText(message, resolvedMessage),
  };
  const normalizedReasonCode = String(reasonCode || '').trim();
  if (normalizedReasonCode) payload.reasonCode = normalizedReasonCode;
  return payload;
}

function normalizeExecutionLockResult(result = {}) {
  const acquired = result?.acquired !== false && result?.success !== false;
  const reasonCode = String(result?.reasonCode || result?.code || 'account_busy').trim() || 'account_busy';
  const reasonMessage = firstNonEmptyText(
    result?.reasonMessage,
    result?.message,
    '同一账号正在执行另一个采集任务',
  );
  const retryAfterMs = toFiniteNumber(result?.retryAfterMs, 0);
  return {
    acquired,
    reasonCode,
    reasonMessage,
    retryAfterMs,
    existingTaskId: String(result?.existingTaskId || '').trim(),
  };
}

function buildExecutionLockSnapshot({ task = {}, lease = null, accountId = '' } = {}) {
  const lock = {
    platform: String(task?.platform || '').trim(),
    accountId: String(accountId || task?.accountId || '').trim(),
    taskId: String(task?.id || task?.taskId || '').trim(),
    leaseToken: String(lease?.leaseToken || '').trim(),
    attemptId: String(lease?.attemptId || task?.currentAttemptId || task?.attemptId || '').trim(),
  };
  return Object.fromEntries(Object.entries(lock).filter(([, value]) => value));
}

function buildAccountBusyPayload({ accountId = '', platform = '', lockResult = {} } = {}) {
  const normalized = normalizeExecutionLockResult(lockResult);
  const payload = {
    status: 'pending',
    reason: normalized.reasonCode,
    reasonCode: normalized.reasonCode,
    errorMessage: normalized.reasonMessage,
    userMessage: normalized.reasonMessage,
    accountId: String(accountId || '').trim(),
    platform: String(platform || '').trim(),
  };
  if (normalized.retryAfterMs > 0) payload.retryAfterMs = normalized.retryAfterMs;
  return payload;
}

function buildPreDispatchReleasePayload(preCheck = {}) {
  const reasonCode = firstNonEmptyText(
    preCheck.reasonCode,
    preCheck.code,
    preCheck.reason,
    'pre_dispatch_check_failed',
  );
  const reasonMessage = firstNonEmptyText(
    preCheck.reasonMessage,
    preCheck.message,
    preCheck.reason,
    reasonCode,
  );
  const payload = {
    status: 'pending',
    reason: reasonCode,
    reasonCode,
    errorMessage: reasonMessage,
    userMessage: reasonMessage,
  };
  const retryAfterMs = toFiniteNumber(preCheck.retryAfterMs, 0);
  if (retryAfterMs > 0) payload.retryAfterMs = retryAfterMs;
  return payload;
}

function normalizePageMode(report = {}) {
  const mode = String(report?.mode || '').trim();
  if (mode) return mode;
  const pageType = String(report?.pageType || '').trim();
  if (pageType === 'noteDetail' || pageType === 'videoDetail' || pageType === 'detail') return 'detail';
  if (pageType === 'profile') return 'profile';
  if (pageType === 'search') return 'search';
  return pageType || '';
}

function buildPageFingerprint(report = {}) {
  if (!report || typeof report !== 'object' || Array.isArray(report)) return null;
  const url = String(report.url || '').trim();
  const pageType = normalizePageMode(report);
  if (!url && !pageType && !report.platform) return null;
  const identity = parseTargetIdentity(url);
  const profileId = identity.profileId;
  const contentId = identity.contentId;
  const fingerprint = {
    platform: String(report.platform || '').trim(),
    pageType,
    rawPageType: String(report.pageType || '').trim(),
    url,
  };
  if (profileId) fingerprint.profileId = profileId;
  if (contentId) fingerprint.contentId = contentId;
  if (profileId || contentId || pageType) {
    fingerprint.routeKey = [pageType, profileId || contentId].filter(Boolean).join(':');
  }
  if (report.readiness && typeof report.readiness === 'object') {
    fingerprint.ready = Boolean(report.readiness.ready);
    fingerprint.readinessReasonCode = String(report.readiness.reasonCode || '').trim();
  }
  return fingerprint;
}

function attachExecutionContextToPatch(patch = {}, { lease = null, pageFingerprint = null } = {}) {
  if (!patch || typeof patch !== 'object' || Array.isArray(patch)) return patch;
  const next = { ...patch };
  if (lease?.leaseToken && !next.leaseToken) next.leaseToken = lease.leaseToken;
  if (lease?.attemptId && !next.attemptId) next.attemptId = lease.attemptId;
  if (lease?.leaseEpoch !== undefined && next.leaseEpoch === undefined) next.leaseEpoch = lease.leaseEpoch;
  if (pageFingerprint && !next.pageFingerprint) next.pageFingerprint = pageFingerprint;
  return next;
}

function firstText(value) {
  return typeof value === 'string' && value.trim() ? value.trim() : '';
}

function firstNonEmptyText(...values) {
  for (const value of values) {
    const text = typeof value === 'string' ? value.trim() : String(value || '').trim();
    if (text) return text;
  }
  return '';
}

function normalizeObject(value) {
  return value && typeof value === 'object' && !Array.isArray(value) ? value : {};
}

function normalizeStreamedRecordCounts(value = {}) {
  const source = normalizeObject(value);
  return {
    note: Math.max(0, Number(source.note || 0) || 0),
    comment: Math.max(0, Number(source.comment || 0) || 0),
    author: Math.max(0, Number(source.author || 0) || 0),
    media: Math.max(0, Number(source.media || 0) || 0),
  };
}

function normalizePersistedActiveTaskContext(value = {}) {
  const source = normalizeObject(value);
  const taskId = String(source.taskId || source.id || '').trim();
  if (!taskId) return null;
  return {
    ...source,
    taskId,
    externalTaskId: String(source.externalTaskId || source.taskId || '').trim(),
    pluginRunId: String(source.pluginRunId || source.collectionRunId || '').trim(),
    platform: String(source.platform || '').trim(),
    source: String(source.source || '').trim(),
    taskStrategy: String(source.taskStrategy || '').trim(),
    accountId: String(source.accountId || '').trim(),
    tabId: toOptionalInteger(source.tabId) || null,
    pluginOpenedTabId: toOptionalInteger(source.pluginOpenedTabId) || null,
    workbenchStatus: String(source.workbenchStatus || source.status || '').trim(),
    resultFingerprint: String(source.resultFingerprint || '').trim(),
    controlCursor: String(source.controlCursor || '').trim(),
    errorMessage: String(source.errorMessage || '').trim(),
    attemptId: String(source.attemptId || source.lease?.attemptId || '').trim(),
    leaseEpoch: toOptionalInteger(source.leaseEpoch ?? source.lease?.leaseEpoch),
    executionPhase: String(source.executionPhase || '').trim(),
    pageFingerprint: normalizeObject(source.pageFingerprint),
    dispatchedAtMs: toFiniteNumber(source.dispatchedAtMs, 0),
    attemptStartedAtMs: toFiniteNumber(source.attemptStartedAtMs, 0),
    pendingAccountUsageId: String(source.pendingAccountUsageId || '').trim(),
    firstRecordSeen: Boolean(source.firstRecordSeen),
    streamedRecordCounts: normalizeStreamedRecordCounts(source.streamedRecordCounts),
    lease: normalizeObject(source.lease),
  };
}

function normalizeRunDiagnostic(run = {}, errorMessage = '') {
  const runRecord = normalizeObject(run.runRecord);
  const stored = normalizeObject(run.diagnostic || runRecord.diagnostic);
  const resolvedError = firstNonEmptyText(
    errorMessage,
    run.errorMessage,
    run.error,
    runRecord.errorMessage,
    runRecord.error,
    stored.technicalMessage,
    stored.userMessage,
  );
  const userMessage = firstNonEmptyText(
    run.userMessage,
    runRecord.userMessage,
    stored.userMessage,
    resolvedError,
  );

  if (!resolvedError && !userMessage && Object.keys(stored).length === 0) {
    return null;
  }

  return {
    ...stored,
    stage: firstNonEmptyText(stored.stage, run.stage, runRecord.stage, 'collecting'),
    failureCategory: firstNonEmptyText(
      stored.failureCategory,
      run.failureCategory,
      runRecord.failureCategory,
      'terminal_failed',
    ),
    reasonCode: firstNonEmptyText(
      stored.reasonCode,
      run.reasonCode,
      run.errorCode,
      runRecord.reasonCode,
      runRecord.errorCode,
      'collection_failed',
    ),
    userMessage,
    technicalMessage: firstNonEmptyText(stored.technicalMessage, resolvedError),
    recommendedAction: firstNonEmptyText(stored.recommendedAction, run.recommendedAction, runRecord.recommendedAction),
    evidence: normalizeObject(stored.evidence),
  };
}

function resolveRunErrorMessage(run = {}) {
  const runRecord = normalizeObject(run.runRecord);
  return firstNonEmptyText(
    run.errorMessage,
    run.error,
    runRecord.errorMessage,
    runRecord.error,
    run.diagnostic?.technicalMessage,
    run.diagnostic?.userMessage,
    runRecord.diagnostic?.technicalMessage,
    runRecord.diagnostic?.userMessage,
  );
}

function buildFailureEventPayload({ run = {}, status = 'failed', progress = 100, errorMessage = '', latestSummary = {} } = {}) {
  const diagnostic = normalizeRunDiagnostic(run, errorMessage);
  const userMessage = firstNonEmptyText(run.userMessage, run.runRecord?.userMessage, diagnostic?.userMessage, errorMessage, '任务执行失败');
  return {
    status,
    progress,
    errorMessage,
    userMessage,
    latestSummary,
    ...(diagnostic
      ? {
          diagnostic,
          stage: diagnostic.stage,
          failureCategory: diagnostic.failureCategory,
          reasonCode: diagnostic.reasonCode,
          technicalMessage: diagnostic.technicalMessage,
          recommendedAction: diagnostic.recommendedAction,
          evidence: diagnostic.evidence,
        }
      : {}),
  };
}

function pickMediaUrlFromArray(value) {
  if (!Array.isArray(value)) return '';

  for (const item of value) {
    const direct = firstText(item);
    if (direct) return direct;

    const nestedArray = pickMediaUrlFromArray(item);
    if (nestedArray) return nestedArray;

    if (item && typeof item === 'object' && !Array.isArray(item)) {
      const nested = firstText(item.urlDefault) || firstText(item.url) || firstText(item.src);
      if (nested) return nested;
    }
  }

  return '';
}

function sanitizeNoteRecord(note = {}, expectedPlatform = '') {
  const platform = String(note.platform || '').trim();
  const strictXhsSource = String(expectedPlatform || '').trim() === 'xhs' || platform === 'xhs';
  const images = Array.isArray(note.images) ? note.images.filter(Boolean) : [];
  const imageCandidates = Array.isArray(note.imageCandidates) ? note.imageCandidates.filter(Boolean) : [];
  const cover = firstText(note.cover)
    || firstText(note.coverImg)
    || firstText(note.coverUrl)
    || firstText(note.thumbnail)
    || pickMediaUrlFromArray(images)
    || pickMediaUrlFromArray(imageCandidates);
  const url = String(note.url || note.noteUrl || '').trim();
  const canonicalUrl = String(note.canonicalUrl || url).trim();
  const rawUrl = String(note.rawUrl || canonicalUrl || url).trim();
  const targetKey = String(note.targetKey || '').trim();

  return {
    targetKey,
    platform,
    noteId: String(strictXhsSource ? (note.noteId || '') : (note.noteId || note.platformContentId || note.contentId || '')).trim(),
    platformContentId: String(strictXhsSource ? (note.platformContentId || '') : (note.platformContentId || note.noteId || note.contentId || '')).trim(),
    title: String(note.title || '').trim(),
    content: String(note.content || note.desc || note.bodyText || '').trim(),
    url,
    canonicalUrl,
    rawUrl,
    sourceUrl: String(note.sourceUrl || '').trim(),
    runnableDetailUrl: String(note.runnableDetailUrl || '').trim(),
    rawShareText: firstText(note.rawShareText),
    cover,
    coverImg: firstText(note.coverImg) || cover,
    coverUrl: firstText(note.coverUrl) || cover,
    images,
    imageCandidates,
    imageCandidateSlots: Array.isArray(note.imageCandidateSlots)
      ? note.imageCandidateSlots.map((slot) => ({
          ordinal: Number.isSafeInteger(slot?.ordinal) && slot.ordinal >= 0 ? slot.ordinal : null,
          candidates: Array.isArray(slot?.candidates) ? slot.candidates.filter(Boolean) : [],
        }))
      : [],
    videoUrl: firstText(note.videoUrl) || firstText(note.video) || firstText(note.videoDownloadUrl) || firstText(note.videoPlayUrl),
    likes: toFiniteNumber(note.likes, 0),
    likeCount: toFiniteNumber(note.likeCount ?? note.likes, 0),
    collects: toFiniteNumber(note.collects, 0),
    collectCount: toFiniteNumber(note.collectCount ?? note.collects, 0),
    comments: toFiniteNumber(note.comments, 0),
    commentCount: toFiniteNumber(note.commentCount ?? note.comments, 0),
    shares: toFiniteNumber(note.shares, 0),
    shareCount: toFiniteNumber(note.shareCount ?? note.shares, 0),
    authorId: String(note.authorId || note.authorPlatformId || note.userId || '').trim(),
    authorPlatformId: String(note.authorPlatformId || note.authorId || note.userId || '').trim(),
    authorEntityId: String(note.authorEntityId || '').trim(),
    authorName: String(note.authorName || note.author || '').trim(),
    authorAvatar: firstText(note.authorAvatar) || firstText(note.avatar),
    publishedAt: note.publishedAt ?? note.publishTime ?? note.releaseDate ?? null,
    publishedAtText:
      firstText(note.publishedAtText)
      || firstText(note.publishTimeText)
      || firstText(note.releaseDateText)
      || firstText(note.releaseDate)
      || firstText(note.timeText)
      || firstText(note.time),
    type: String(strictXhsSource ? (note.type || '') : (note.type || note.contentType || note.noteType || note.itemType || '')).trim(),
    lastUpdateTime: note.lastUpdateTime ?? null,
    rank: toFiniteNumber(note.rank ?? note.batchRank, 0),
    batchRank: toFiniteNumber(note.batchRank ?? note.rank, 0),
    isPinned: Boolean(note.isPinned),
    observedAt: String(note.observedAt || '').trim(),
    collectionRunId: String(note.collectionRunId || '').trim(),
    monitorMode: String(note.monitorMode || '').trim(),
    monitorId: String(note.monitorId || note.monitorMeta?.monitorId || '').trim(),
    taskStrategy: String(note.taskStrategy || note.monitorMeta?.taskStrategy || '').trim(),
    monitorMeta: note.monitorMeta && typeof note.monitorMeta === 'object' && !Array.isArray(note.monitorMeta)
      ? { ...note.monitorMeta }
      : {},
  };
}

function sanitizeCommentRecord(comment = {}) {
  return {
    commentId: String(comment.commentId || comment.id || '').trim(),
    noteId: String(comment.noteId || comment.contentId || comment.rootNoteId || '').trim(),
    text: String(comment.text || comment.content || comment.comment || '').trim(),
    author: String(comment.author || comment.authorName || comment.userName || '').trim(),
    authorId: String(comment.authorId || comment.userId || '').trim(),
    likes: toFiniteNumber(comment.likes, 0),
    level: toFiniteNumber(comment.level, 1) || 1,
    url: String(comment.url || '').trim(),
  };
}

function isValidCommentRecordForWriteback(comment = {}) {
  return Boolean(comment.commentId && comment.noteId && comment.text);
}

function sanitizeAuthorRecord(author = {}, expectedPlatform = '') {
  const platform = String(author.platform || '').trim();
  const strictXhsSource = String(expectedPlatform || '').trim() === 'xhs' || platform === 'xhs';
  const authorId = String(strictXhsSource
    ? (author.authorId || '')
    : (author.authorId || author.platformAuthorId || author.authorPlatformId || author.userId || author.id || '')).trim();
  const platformAuthorId = String(strictXhsSource
    ? (author.platformAuthorId || '')
    : (author.platformAuthorId || author.authorPlatformId || authorId)).trim();
  const description = String(author.description || author.bio || author.signature || '').trim();
  const fans = toFiniteNumber(author.fans ?? author.followers ?? author.followerCount, 0);
  const follows = toFiniteNumber(author.follows ?? author.following ?? author.followingCount, 0);
  const interactions = toFiniteNumber(
    author.interactions ?? author.likesAndCollects ?? author.likedCount ?? author.totalLiked,
    0,
  );
  const works = toFiniteNumber(author.works ?? author.workCount ?? author.notes ?? author.noteCount, 0);
  const notes = toFiniteNumber(author.notes ?? author.noteCount ?? author.works ?? author.workCount, 0);
  return {
    authorId,
    platformAuthorId,
    authorEntityId: String(author.authorEntityId || '').trim(),
    userId: String(author.userId || '').trim(),
    platform,
    name: String(author.name || author.authorName || author.nickname || '').trim(),
    profileUrl: String(author.profileUrl || author.url || '').trim(),
    avatar: firstText(author.avatar) || firstText(author.avatarUrl) || firstText(author.image),
    description,
    bio: description,
    ipLocation: String(author.ipLocation || '').trim(),
    location: String(author.location || '').trim(),
    handle: String(author.handle || author.redId || author.douyinId || '').trim(),
    redId: String(author.redId || '').trim(),
    douyinId: String(author.douyinId || '').trim(),
    fans,
    followers: fans,
    follows,
    following: follows,
    interactions,
    likesAndCollects: interactions,
    works,
    notes,
    monitorMode: String(author.monitorMode || '').trim(),
    monitorId: String(author.monitorId || author.monitorMeta?.monitorId || '').trim(),
    taskStrategy: String(author.taskStrategy || author.monitorMeta?.taskStrategy || '').trim(),
    monitorMeta: author.monitorMeta && typeof author.monitorMeta === 'object' && !Array.isArray(author.monitorMeta)
      ? { ...author.monitorMeta }
      : {},
  };
}

function sanitizeMediaAssetRecord(asset = {}) {
  return {
    assetId: String(asset.assetId || asset.id || '').trim(),
    sourceUrl: String(asset.sourceUrl || asset.url || '').trim(),
    localPath: String(asset.localPath || '').trim(),
    status: String(asset.status || '').trim(),
    assetType: String(asset.assetType || '').trim(),
    role: String(asset.role || '').trim(),
    ordinal: Number.isSafeInteger(asset.ordinal) && asset.ordinal >= 0 ? asset.ordinal : null,
    candidateUrls: Array.isArray(asset.candidateUrls) ? asset.candidateUrls.filter(Boolean) : [],
    coverProvenance: String(asset.coverProvenance || '').trim(),
    noteId: String(asset.noteId || asset.contentId || '').trim(),
    commentId: String(asset.commentId || '').trim(),
  };
}

export function buildWorkbenchResultSummary(run = {}) {
  const records = run?.records || {};
  const notes = Array.isArray(records.notes)
    ? records.notes.map((note) => sanitizeNoteRecord(note, run.platform)).filter((note) => note.noteId || note.title || note.content)
    : [];
  const comments = Array.isArray(records.comments)
    ? records.comments.map(sanitizeCommentRecord).filter(isValidCommentRecordForWriteback)
    : [];
  const authors = Array.isArray(records.authors)
    ? records.authors.map((author) => sanitizeAuthorRecord(author, run.platform)).filter((author) => author.authorId || author.name)
    : [];
  const mediaAssets = Array.isArray(records.mediaAssets)
    ? records.mediaAssets.map(sanitizeMediaAssetRecord).filter((asset) => asset.assetId || asset.sourceUrl || asset.localPath)
    : [];

  return {
    ...(run?.resultSummary || {}),
    records: {
      notes,
      comments,
      authors,
      mediaAssets,
    },
  };
}

function buildWorkbenchRecordDeltas(
  activeTask = {},
  pluginRunId = '',
  resultSummary = {},
  sequenceBase = Date.now(),
  executionContext = null,
) {
  const taskId = String(activeTask.taskId || '').trim();
  const runId = String(pluginRunId || activeTask.pluginRunId || activeTask.externalTaskId || activeTask.taskId || '').trim();
  if (!taskId || !runId) return [];

  const records = resultSummary.records || {};
  let sequence = Number.isFinite(Number(sequenceBase)) ? Math.floor(Number(sequenceBase)) : Date.now();
  const nextSequence = () => {
    sequence += 1;
    return sequence;
  };

  const noteDeltas = (Array.isArray(records.notes) ? records.notes : []).map((note) => ({
    taskId,
    pluginRunId: runId,
    recordType: WORKBENCH_RECORD_TYPE.NOTE,
    externalRecordId: String(note.noteId || note.platformContentId || note.url || '').trim(),
    sequence: nextSequence(),
    payload: note,
    executionContext,
  }));

  const commentDeltas = (Array.isArray(records.comments) ? records.comments : []).map((comment) => ({
    taskId,
    pluginRunId: runId,
    recordType: WORKBENCH_RECORD_TYPE.COMMENT,
    externalRecordId: String(comment.commentId || comment.id || '').trim(),
    sequence: nextSequence(),
    payload: comment,
    executionContext,
  }));

  const authorDeltas = (Array.isArray(records.authors) ? records.authors : []).map((author) => ({
    taskId,
    pluginRunId: runId,
    recordType: WORKBENCH_RECORD_TYPE.AUTHOR,
    externalRecordId: String(author.authorId || author.userId || author.profileUrl || '').trim(),
    sequence: nextSequence(),
    payload: author,
    executionContext,
  }));

  const mediaDeltas = (Array.isArray(records.mediaAssets) ? records.mediaAssets : []).map((asset) => ({
    taskId,
    pluginRunId: runId,
    recordType: WORKBENCH_RECORD_TYPE.MEDIA,
    externalRecordId: String(asset.assetId || asset.sourceUrl || asset.localPath || '').trim(),
    sequence: nextSequence(),
    payload: asset,
    executionContext,
  }));

  return [
    ...noteDeltas,
    ...commentDeltas,
    ...authorDeltas,
    ...mediaDeltas,
  ].filter((record) => record.externalRecordId || Object.keys(record.payload || {}).length > 0);
}

function hasResultLookupError(result = {}) {
  return /collectionRunId or externalTaskId required|collectionRun not found/i.test(String(result?.error || '').trim());
}

function resolveDispatchCollectionRunId(dispatch = {}) {
  return String(
    dispatch?.collectionRunId
    || dispatch?.resultLookup?.collectionRunId
    || '',
  ).trim();
}

function hydrateTrackedTask(task = {}, now = Date.now(), persistedContext = null) {
  const taskId = String(task?.id || '').trim();
  if (!taskId) return null;
  const persisted = normalizePersistedActiveTaskContext(persistedContext);
  const matchingPersisted = persisted?.taskId === taskId ? persisted : null;
  return {
    taskId,
    externalTaskId: String(task?.externalTaskId || matchingPersisted?.externalTaskId || taskId).trim(),
    taskType: String(task?.taskType || '').trim(),
    platform: String(task?.platform || matchingPersisted?.platform || '').trim(),
    source: String(task?.source || matchingPersisted?.source || '').trim(),
    taskStrategy: String(task?.taskStrategy || matchingPersisted?.taskStrategy || '').trim(),
    payload: task?.payload && typeof task.payload === 'object' ? task.payload : {},
    collectionProfile: resolveCollectionProfile(task, matchingPersisted),
    pluginRunId: String(task?.pluginRunId || matchingPersisted?.pluginRunId || '').trim(),
    executorInstanceId: String(task?.executorInstanceId || matchingPersisted?.executorInstanceId || matchingPersisted?.stationId || '').trim(),
    accountId: String(task?.accountId || matchingPersisted?.accountId || '').trim(),
    tabId: toOptionalInteger(task?.tabId) || matchingPersisted?.tabId || null,
    pluginOpenedTabId: toOptionalInteger(task?.pluginOpenedTabId) || matchingPersisted?.pluginOpenedTabId || null,
    workbenchStatus: String(task?.status || matchingPersisted?.workbenchStatus || 'dispatched').trim() || 'dispatched',
    resultFingerprint: String(matchingPersisted?.resultFingerprint || '').trim(),
    controlCursor: String(task?.controlCursor || matchingPersisted?.controlCursor || '').trim(),
    errorMessage: String(task?.errorMessage || matchingPersisted?.errorMessage || '').trim(),
    attemptId: String(task?.currentAttemptId || task?.attemptId || matchingPersisted?.attemptId || '').trim(),
    leaseEpoch: toOptionalInteger(task?.leaseEpoch ?? matchingPersisted?.leaseEpoch),
    executionPhase: String(task?.executionPhase || matchingPersisted?.executionPhase || '').trim(),
    pageFingerprint: task?.pageFingerprint && typeof task.pageFingerprint === 'object' && !Array.isArray(task.pageFingerprint)
      ? { ...task.pageFingerprint }
      : (matchingPersisted?.pageFingerprint && Object.keys(matchingPersisted.pageFingerprint).length > 0 ? { ...matchingPersisted.pageFingerprint } : null),
    dispatchedAtMs:
      parseTimestamp(task?.dispatchedAt)
      || parseTimestamp(task?.updatedAt)
      || parseTimestamp(task?.createdAt)
      || toFiniteNumber(matchingPersisted?.dispatchedAtMs, 0)
      || now,
    pendingAccountUsageId: String(matchingPersisted?.pendingAccountUsageId || '').trim(),
    firstRecordSeen: Boolean(matchingPersisted?.firstRecordSeen),
    streamedRecordCounts: normalizeStreamedRecordCounts(matchingPersisted?.streamedRecordCounts),
    attemptStartedAtMs:
      toFiniteNumber(matchingPersisted?.attemptStartedAtMs, 0)
      || parseTimestamp(task?.dispatchedAt)
      || now,
    stopRequestedAtMs: toFiniteNumber(matchingPersisted?.stopRequestedAtMs, 0),
    stopControlAction: String(task?.controlAction || matchingPersisted?.stopControlAction || '').trim(),
    deleteRequested: Boolean(task?.deletedAt || matchingPersisted?.deleteRequested),
  };
}

function cleanupTaskSnapshot(task = {}) {
  const taskId = String(task?.taskId || task?.id || '').trim();
  const externalTaskId = String(task?.externalTaskId || task?.id || taskId).trim();
  const pluginRunId = String(task?.pluginRunId || task?.collectionRunId || '').trim();
  const pluginOpenedTabId = toOptionalInteger(task?.pluginOpenedTabId) || null;
  return {
    taskId,
    externalTaskId,
    pluginRunId,
    ...(pluginOpenedTabId ? { pluginOpenedTabId } : {}),
  };
}

function buildIdleTickResult(claimed = {}, idleSnapshot = null) {
  const normalizedSnapshot = idleSnapshot || createTaskLeaseIdleSnapshot(claimed) || null;
  const reason = normalizedSnapshot?.reason || (
    claimed?.reason && typeof claimed.reason === 'object' && !Array.isArray(claimed.reason)
      ? { ...claimed.reason }
      : null
  );
  const nextPollAfterMs = Number.isFinite(Number(claimed?.nextPollAfterMs))
    ? Number(claimed.nextPollAfterMs)
    : Number(normalizedSnapshot?.nextPollAfterMs || 0);

  const result = {
    success: true,
    idle: true,
    nextPollAfterMs: Number.isFinite(nextPollAfterMs) ? nextPollAfterMs : 0,
  };
  if (normalizedSnapshot?.idleReasonCode) {
    result.idleReasonCode = normalizedSnapshot.idleReasonCode;
  }
  if (normalizedSnapshot?.idleReasonMessage) {
    result.idleReasonMessage = normalizedSnapshot.idleReasonMessage;
  }
  if (reason) {
    result.reason = reason;
  }
  return result;
}

function buildRetryableClaimIdleResult(error = {}) {
  const code = String(error?.reasonCode || 'server_backpressure').trim() || 'server_backpressure';
  const message = String(error?.message || '服务端暂时繁忙，请稍后重试。').trim();
  const nextPollAfterMs = Number.isFinite(Number(error?.nextPollAfterMs))
    ? Number(error.nextPollAfterMs)
    : 60_000;
  return buildIdleTickResult({
    task: null,
    nextPollAfterMs,
    reason: { code, message },
  });
}

function buildAuthorizationFailureIdleResult(error = {}) {
  const message = String(error?.message || '插件授权已失效，请重新授权后再接单。').trim();
  return buildIdleTickResult({
    task: null,
    nextPollAfterMs: AUTHORIZATION_FAILURE_IDLE_MS,
    reason: {
      code: 'authorization_invalid',
      message,
    },
  });
}

export function createTaskPoller(deps = {}) {
  const state = {
    activeTask: null,
    activeLease: null,
    ticking: false,
    seenControlIds: new Set(),
    lastIdleReason: null,
  };

  let tickPromise = null;
  let tickStartedAtMs = 0;
  let lastReconcileAtMs = 0;

  function getNow() {
    return typeof deps.now === 'function' ? deps.now() : Date.now();
  }

  function buildRuntimeCaptureSubmission(activeTask, terminal) {
    if (String(activeTask?.platform || '').trim() !== 'xhs') return null;
    const lease = state.activeLease ? normalizeLeaseSnapshot(state.activeLease) : {};
    const reservation = {
      jobId: String(activeTask?.taskId || activeTask?.id || '').trim(),
      attemptId: String(lease.attemptId || activeTask?.attemptId || '').trim(),
      leaseEpoch: toOptionalInteger(lease.leaseEpoch ?? activeTask?.leaseEpoch),
      platform: 'xhs',
      collectionProfile: String(activeTask?.collectionProfile || '').trim(),
      targetKey: String(activeTask?.payload?.targetKey || activeTask?.targetKey || '').trim(),
      captureId: String(activeTask?.captureId || lease.captureId || '').trim(),
      executionPlanVersion: String(
        activeTask?.executionPlanVersion || lease.executionPlanVersion || '',
      ).trim(),
    };
    const collectorVersion = typeof deps.collectorVersion === 'function'
      ? String(deps.collectorVersion() || '').trim()
      : String(deps.collectorVersion || '').trim();
    const mapper = typeof deps.mapRuntimeTerminalToCaptureSubmissionV2 === 'function'
      ? deps.mapRuntimeTerminalToCaptureSubmissionV2
      : mapRuntimeTerminalToCaptureSubmissionV2;
    const mapped = mapper({
      reservation,
      terminal,
      collectorVersion,
      observedTargetKey: null,
    });
    if (mapped?.ok !== true) {
      const error = new Error(String(mapped?.reason || 'v2_capture_mapping_failed'));
      error.reasonCode = String(mapped?.reason || 'v2_capture_mapping_failed');
      error.retryable = false;
      error.cause = mapped?.error;
      throw error;
    }
    return mapped.body;
  }

  // 出站积压熔断（报告 §9.2）：可补发写回超过阈值（条数或最老记录年龄）时，
  // 说明结果送不回服务端；此时接新任务只会批量制造租约过期。熔断期间
  // 每 tick 触发一次补发自愈，积压回落后自动恢复接单。终态死信只做告警，
  // 不计入熔断条数；它们不会被补发清除，计入会造成永久拒接且无法自愈。
  async function evaluateOutboxBacklogGate() {
    if (typeof deps.getOutboxBacklog !== 'function') return null;
    let backlog = null;
    try {
      backlog = await deps.getOutboxBacklog();
    } catch {
      return null;
    }
    if (!backlog || typeof backlog !== 'object') return null;

    const backlogLimit = Number.isFinite(Number(deps.outboxBacklogLimit))
      ? Number(deps.outboxBacklogLimit)
      : 200;
    const maxAgeMs = Number.isFinite(Number(deps.outboxOldestUnsentMaxAgeMs))
      ? Number(deps.outboxOldestUnsentMaxAgeMs)
      : 30 * 60 * 1000;

    const retryable = Number(backlog.retryable || 0);
    const deadLetter = Number(backlog.deadLetter || 0);
    const oldestUnsentCreatedAt = Number(backlog.oldestUnsentCreatedAt || 0);
    const oldestUnsentAgeMs = oldestUnsentCreatedAt > 0 ? Math.max(0, getNow() - oldestUnsentCreatedAt) : 0;

    const overCount = retryable > backlogLimit;
    const overAge = oldestUnsentAgeMs > maxAgeMs;
    if (!overCount && !overAge) return null;

    if (typeof deps.flushOutbox === 'function') {
      try {
        await deps.flushOutbox();
      } catch {
        // 自愈补发失败不影响熔断结论
      }
    }

    return {
      reason: {
        code: 'OUTBOX_BACKLOG_BLOCKED',
        message: `本地写回积压（待重试 ${retryable} 条 / 死信 ${deadLetter} 条`
          + `${oldestUnsentAgeMs > 0 ? `，最老 ${Math.round(oldestUnsentAgeMs / 60000)} 分钟` : ''}），暂停接新任务等待写回恢复`,
      },
      nextPollAfterMs: 60_000,
    };
  }

  async function readAuthorizationFailureBackoff() {
    if (typeof deps.readAuthorizationFailureBackoff !== 'function') return null;
    let snapshot = null;
    try {
      snapshot = await deps.readAuthorizationFailureBackoff();
    } catch {
      return null;
    }
    const retryAtMs = Number(snapshot?.retryAtMs || 0);
    const now = getNow();
    if (!Number.isFinite(retryAtMs) || retryAtMs <= now) {
      if (snapshot && typeof deps.clearAuthorizationFailureBackoff === 'function') {
        try {
          await deps.clearAuthorizationFailureBackoff();
        } catch {
          // ignore storage cleanup failures
        }
      }
      return null;
    }
    const reason = snapshot?.reason && typeof snapshot.reason === 'object' && !Array.isArray(snapshot.reason)
      ? snapshot.reason
      : {
          code: String(snapshot?.code || 'authorization_invalid').trim() || 'authorization_invalid',
          message: String(snapshot?.message || '授权或插件版本异常，退避后重试。').trim(),
        };
    const idleResult = buildIdleTickResult({
      task: null,
      nextPollAfterMs: retryAtMs - now,
      reason,
    });
    state.lastIdleReason = createTaskLeaseIdleSnapshot(idleResult);
    return idleResult;
  }

  async function persistAuthorizationFailureBackoff(idleResult = {}) {
    if (typeof deps.writeAuthorizationFailureBackoff !== 'function') return;
    const nextPollAfterMs = Number.isFinite(Number(idleResult?.nextPollAfterMs))
      ? Number(idleResult.nextPollAfterMs)
      : AUTHORIZATION_FAILURE_IDLE_MS;
    const retryAtMs = getNow() + Math.max(0, nextPollAfterMs);
    try {
      await deps.writeAuthorizationFailureBackoff({
        retryAtMs,
        reason: idleResult?.reason || {
          code: String(idleResult?.idleReasonCode || 'authorization_invalid').trim() || 'authorization_invalid',
          message: String(idleResult?.idleReasonMessage || '授权或插件版本异常，退避后重试。').trim(),
        },
      });
    } catch {
      // ignore storage write failures
    }
  }

  async function clearAuthorizationFailureBackoff() {
    if (typeof deps.clearAuthorizationFailureBackoff !== 'function') return;
    try {
      await deps.clearAuthorizationFailureBackoff();
    } catch {
      // ignore storage cleanup failures
    }
  }

  async function handleAuthorizationFailure(error = {}) {
    const idleResult = buildAuthorizationFailureIdleResult(error);
    state.lastIdleReason = createTaskLeaseIdleSnapshot(idleResult);
    await persistAuthorizationFailureBackoff(idleResult);
    return idleResult;
  }

  async function notifyContentScriptToStop(activeTask = {}) {
    const tabId = activeTask?.tabId;
    if (!tabId) return;
    const taskControl = {
      taskId: String(activeTask.externalTaskId || activeTask.taskId || '').trim(),
      taskType: String(activeTask.taskType || '').trim(),
      collectionRunId: String(activeTask.pluginRunId || '').trim(),
      action: REMOTE_TASK_CONTROL_ACTION.STOP,
      protocolVersion: 'v1',
    };
    try {
      const result = await chrome.tabs.sendMessage(tabId, {
        action: 'workbenchTaskControl',
        command: 'stop',
        taskControl,
      });
      if (result?.success !== false && result?.accepted !== false && !result?.error) return;
      await chrome.tabs.sendMessage(tabId, {
        action: 'workbenchTaskControl',
        command: 'stop',
      });
    } catch {
      // 标签页已关闭或导航到其他页面，忽略错误
    }
  }

  async function patchTask(taskId, patch) {
    if (!taskId || !patch || typeof deps.patchTask !== 'function') return null;
    const activeTask = state.activeTask?.taskId === taskId ? state.activeTask : null;
    return deps.patchTask(taskId, attachExecutionContextToPatch(patch, {
      lease: activeTask ? state.activeLease : null,
      pageFingerprint: activeTask?.pageFingerprint || null,
    }));
  }

  async function resolveExecutorInstanceId() {
    if (typeof deps.getExecutorInstanceId !== 'function') return '';
    return String(await deps.getExecutorInstanceId() || '').trim();
  }

  function buildActiveTaskContextSnapshot() {
    if (!state.activeTask) return null;
    return {
      ...state.activeTask,
      lease: state.activeLease ? normalizeLeaseSnapshot(state.activeLease) : {},
      updatedAtMs: getNow(),
    };
  }

  async function persistActiveTaskContext() {
    if (typeof deps.writeActiveTaskContext !== 'function') return;
    const snapshot = buildActiveTaskContextSnapshot();
    if (!snapshot) return;
    try {
      await deps.writeActiveTaskContext(snapshot);
    } catch {
      // Persistence must not block task execution.
    }
  }

  async function readActiveTaskContext(taskId = '') {
    if (typeof deps.readActiveTaskContext !== 'function') return null;
    try {
      const snapshot = normalizePersistedActiveTaskContext(await deps.readActiveTaskContext(taskId));
      if (!snapshot) return null;
      const normalizedTaskId = String(taskId || '').trim();
      return !normalizedTaskId || snapshot.taskId === normalizedTaskId ? snapshot : null;
    } catch {
      return null;
    }
  }

  async function clearActiveTaskContext() {
    if (typeof deps.clearActiveTaskContext !== 'function') return;
    try {
      await deps.clearActiveTaskContext();
    } catch {
      // Storage cleanup failure should not block lease cleanup.
    }
  }

  async function releaseExecutionLock(activeTask = state.activeTask) {
    if (typeof deps.releaseExecutionLock !== 'function') return;
    const platform = String(activeTask?.platform || '').trim();
    const accountId = String(activeTask?.accountId || '').trim();
    const taskId = String(activeTask?.taskId || activeTask?.id || '').trim();
    if (!platform || !accountId || !taskId) return;
    try {
      await deps.releaseExecutionLock({ platform, accountId, taskId });
    } catch {
      // A stale local lock should not block remote task cleanup.
    }
  }

  async function clearActiveLease(activeTaskForCleanup = state.activeTask) {
    await releaseExecutionLock(activeTaskForCleanup);
    state.activeLease = null;
    await clearActiveTaskContext();
    if (typeof deps.clearTaskLease === 'function') {
      await deps.clearTaskLease();
    }
  }

  async function flushDeltasBeforeCleanup() {
    if (typeof deps.flushDeltas !== 'function') return { success: true, skipped: true };
    const result = await deps.flushDeltas();
    if (result?.success === false) {
      const error = new Error(String(result.reason || result.error || 'delta_flush_failed_before_cleanup'));
      error.retryable = true;
      error.flushResult = result;
      throw error;
    }
    return result;
  }

  function buildTerminalSnapshot({ status = 'completed', progress = 100, latestSummary = {} } = {}) {
    return {
      status,
      progress,
      latestSummary: latestSummary && typeof latestSummary === 'object' && !Array.isArray(latestSummary)
        ? latestSummary
        : {},
      latestHeartbeatAt: new Date().toISOString(),
    };
  }

  async function enqueueTerminalTaskEvent(activeTask, eventType, payload = {}, options = {}) {
    const status = String(options.status || payload.status || '').trim() || 'failed';
    const progress = Number.isFinite(Number(options.progress ?? payload.progress))
      ? Number(options.progress ?? payload.progress)
      : 100;
    return enqueueTaskEvent(activeTask, eventType, {
      status,
      progress,
      ...payload,
    }, {
      ...options,
      snapshot: options.snapshot || buildTerminalSnapshot({
        status,
        progress,
        latestSummary: options.latestSummary || payload.latestSummary || {},
      }),
    });
  }

  async function flushTerminalDeltasBeforeCleanup(reason = 'terminal_snapshot_flush') {
    try {
      await flushDeltasBeforeCleanup();
      return { success: true };
    } catch (err) {
      console.warn(`[taskPoller] ${reason} failed, keeping active lease for retry:`, err?.message || err);
      return {
        success: false,
        error: err,
        reason,
      };
    }
  }

  async function finalizeControlledStop(activeTask = state.activeTask, terminalControl = {}) {
    if (!activeTask) return { success: true, idle: true };
    const action = String(
      terminalControl.action
      || terminalControl.originalAction
      || activeTask.stopControlAction
      || REMOTE_TASK_CONTROL_ACTION.STOP
    ).trim();
    const deleteRequested = Boolean(
      terminalControl.deleteRequested
      || action === REMOTE_TASK_CONTROL_ACTION.DELETE
      || activeTask.deleteRequested
    );
    const controlRequestId = String(terminalControl.controlRequestId || '').trim();
    const cleanupTask = cleanupTaskSnapshot(activeTask);
    const userMessage = deleteRequested
      ? '任务已删除，插件已停止本地执行并释放账号。'
      : '任务已停止，插件已释放账号。';

    await notifyContentScriptToStop(activeTask);
    try {
      await patchTask(activeTask.taskId, {
        status: 'stopped',
        pluginRunId: activeTask.pluginRunId || null,
        errorMessage: null,
        deferRelease: true,
      });
    } catch {
      // The task may already be deleted; local cleanup still has to continue.
    }
    await enqueueTerminalTaskEvent(activeTask, WORKBENCH_TASK_EVENT_TYPE.TASK_STOPPED, {
      status: 'stopped',
      progress: 100,
      action,
      controlRequestId,
      deleteRequested,
      reasonCode: 'control_stop_applied',
      userMessage,
    }, { controlRequestId });
    const flushResult = await flushTerminalDeltasBeforeCleanup('terminal_stop_snapshot_flush');
    if (!flushResult.success) {
      return {
        success: true,
        final: false,
        flushPending: true,
        status: 'stopped',
        reason: 'terminal_stop_snapshot_pending',
        cleanupTask,
      };
    }
    try {
      await patchTask(activeTask.taskId, {
        status: 'stopped',
        pluginRunId: activeTask.pluginRunId || null,
        errorMessage: null,
      });
    } catch {
      // Remote release can be retried by reconciliation; local cleanup still has to continue.
    }
    state.activeTask = null;
    state.seenControlIds.clear();
    await clearActiveLease(activeTask);
    return {
      success: true,
      final: true,
      status: 'stopped',
      reason: deleteRequested ? 'control_delete_stopped' : 'control_stop_stopped',
      cleanupTask,
    };
  }

  async function acquireExecutionLock(task = {}, preCheck = null, lease = null) {
    const accountId = String(preCheck?.accountId || task?.accountId || '').trim();
    const platform = String(task?.platform || '').trim();
    if (!accountId || !platform || typeof deps.acquireExecutionLock !== 'function') {
      return { acquired: true, lock: null };
    }
    const lock = buildExecutionLockSnapshot({ task, lease, accountId });
    try {
      const result = normalizeExecutionLockResult(await deps.acquireExecutionLock(lock));
      if (!result.acquired && await shouldReleaseStaleExecutionLock(result.existingTaskId, task)) {
        await deps.releaseExecutionLock?.({
          platform,
          accountId,
          taskId: result.existingTaskId,
        });
        const retry = normalizeExecutionLockResult(await deps.acquireExecutionLock(lock));
        return {
          ...retry,
          lock,
          staleLockReleased: retry.acquired,
          staleLockTaskId: result.existingTaskId,
        };
      }
      return { ...result, lock };
    } catch (error) {
      return {
        acquired: false,
        reasonCode: 'account_lock_error',
        reasonMessage: String(error?.message || error || '账号执行锁检查失败'),
        retryAfterMs: 60000,
        lock,
      };
    }
  }

  async function shouldReleaseStaleExecutionLock(existingTaskId = '', task = {}) {
    const normalizedExistingTaskId = String(existingTaskId || '').trim();
    const normalizedCurrentTaskId = String(task?.id || task?.taskId || '').trim();
    if (!normalizedExistingTaskId || normalizedExistingTaskId === normalizedCurrentTaskId) return false;
    if (/^manual:/i.test(normalizedExistingTaskId)) return false;
    if (state.activeTask?.taskId === normalizedExistingTaskId) return false;
    if (typeof deps.readTaskLease !== 'function') return true;
    try {
      const localLease = await deps.readTaskLease();
      if (String(localLease?.taskId || '').trim() !== normalizedExistingTaskId) return true;
      const expiresAtMs = Date.parse(String(localLease?.expiresAt || '').trim());
      return Number.isFinite(expiresAtMs) && expiresAtMs <= getNow();
    } catch {
      return false;
    }
  }

  async function refreshExecutionLock(activeTask = state.activeTask) {
    if (!activeTask || typeof deps.acquireExecutionLock !== 'function') return;
    await acquireExecutionLock({
      id: activeTask.taskId,
      platform: activeTask.platform,
      accountId: activeTask.accountId,
      currentAttemptId: activeTask.attemptId,
    }, { accountId: activeTask.accountId }, state.activeLease);
  }

  function updateActiveTask(patch = {}) {
    if (!state.activeTask) return null;
    const nextPatch = typeof patch === 'function'
      ? patch({ ...state.activeTask })
      : patch;
    if (!nextPatch || typeof nextPatch !== 'object') {
      return state.activeTask ? { ...state.activeTask } : null;
    }
    state.activeTask = deepClone({
      ...state.activeTask,
      ...nextPatch,
    });
    void persistActiveTaskContext();
    return { ...state.activeTask };
  }

  async function consumePendingAccountUsage(activeTask, mappedStatus = '', pluginRunId = '') {
    const accountId = String(activeTask?.pendingAccountUsageId || '').trim();
    if (!accountId) return false;
    if (!pluginRunId || mappedStatus === 'paused' || mappedStatus === 'dispatched') {
      return false;
    }
    if (typeof deps.consumePendingAccountUsage !== 'function') {
      activeTask.pendingAccountUsageId = '';
      return true;
    }
    await deps.consumePendingAccountUsage(accountId, activeTask);
    activeTask.pendingAccountUsageId = '';
    return true;
  }

  async function claimTask(task, lease = null) {
    if (!task?.id) return { success: false, skipped: true, reason: 'missing_task_id' };
    let preCheck = null;

    if (typeof deps.beforeDispatch === 'function') {
      preCheck = await deps.beforeDispatch(task);
      if (preCheck?.shouldPause) {
        if (lease) {
          const payload = buildPreDispatchReleasePayload(preCheck);
          await patchTask(task.id, {
            status: 'pending',
            progress: 0,
            errorMessage: payload.errorMessage,
          });
          if (typeof deps.enqueueEvent === 'function') {
            const executorInstanceId = await resolveExecutorInstanceId();
            await deps.enqueueEvent({
              taskId: task.id,
              pluginRunId: '',
              eventType: WORKBENCH_TASK_EVENT_TYPE.TASK_RELEASED,
              source: WORKBENCH_EVENT_SOURCE.PLUGIN,
              sequence: Date.now(),
              ...buildLeaseEventFields({
                task,
                lease,
                executorInstanceId,
                accountId: preCheck?.accountId,
              }),
              payload,
            });
            if (typeof deps.flushDeltas === 'function') await deps.flushDeltas();
          }
          await clearActiveLease();
          return { success: false, skipped: true, reason: payload.reasonCode };
        }
        await patchTask(task.id, {
          status: 'paused',
          progress: 5,
          errorMessage: preCheck.reason || 'pre_dispatch_check_failed',
        });
        return { success: false, skipped: true, reason: preCheck.reason };
      }
    }

    let capability;
    try {
      capability = typeof deps.capabilityCheck === 'function'
        ? await deps.capabilityCheck(task)
        : { accepted: false };
    } catch (error) {
      const isRecoverable = isRecoverableConnectionError(error);
      const recoveryStatus = isRecoverable
        ? recoverableConnectionStatusForTask(task)
        : { status: 'failed', progress: 100, eventType: WORKBENCH_TASK_EVENT_TYPE.TASK_FAILED, message: '能力检查失败' };
      await patchTask(task.id, {
        status: recoveryStatus.status,
        progress: recoveryStatus.progress,
        errorMessage: String(error?.message || error || 'capability_check_failed'),
      });
      if (isRecoverable && typeof deps.enqueueEvent === 'function') {
        const executorInstanceId = await resolveExecutorInstanceId();
        await deps.enqueueEvent({
          taskId: task.id,
          pluginRunId: '',
          eventType: recoveryStatus.eventType,
          source: WORKBENCH_EVENT_SOURCE.PLUGIN,
          sequence: Date.now(),
          ...buildLeaseEventFields({
            task,
            lease,
            executorInstanceId,
            accountId: preCheck?.accountId,
          }),
          payload: {
            reason: 'connection_interrupted',
            message: recoveryStatus.message,
            status: recoveryStatus.status,
            errorMessage: String(error?.message || error || 'capability_check_failed'),
          },
        });
        if (typeof deps.flushDeltas === 'function') await deps.flushDeltas();
      }
      if (lease) await clearActiveLease();
      return {
        success: false,
        skipped: false,
        reason: String(error?.message || error || 'capability_check_failed'),
        cleanupTask: cleanupTaskSnapshot(task),
      };
    }

    if (!capability?.accepted) {
      const rejection = normalizeCapabilityRejection({
        ...capability,
        taskType: task.taskType,
        task,
      });
      const preserveTaskExecutionContext = shouldPreserveTaskExecutionContextForCapabilityRejection(
        rejection.reasonCode,
      );
      const executorInstanceId = await resolveExecutorInstanceId();
      const eventFields = buildLeaseEventFields({
        task,
        lease,
        executorInstanceId,
        accountId: preCheck?.accountId,
      });
      if (lease) {
        await patchTask(task.id, {
          status: rejection.taskStatus,
          progress: rejection.progress,
          errorMessage: rejection.reasonMessage,
        });
      }
      if (typeof deps.enqueueEvent === 'function' && task?.id) {
        await deps.enqueueEvent({
          taskId: task.id,
          pluginRunId: '',
          eventType: WORKBENCH_TASK_EVENT_TYPE.TASK_CAPABILITY_MISMATCH,
          source: WORKBENCH_EVENT_SOURCE.PLUGIN,
          sequence: Date.now(),
          ...eventFields,
          payload: {
            taskType: String(task.taskType || '').trim(),
            reasonCode: rejection.reasonCode,
            reasonMessage: rejection.reasonMessage,
            recommendedAction: capability?.recommendedAction || '',
            status: rejection.taskStatus,
            ...buildCapabilityReportDiagnostic(capability?.report),
          },
        });
        if (lease && rejection.taskStatus === 'pending') {
          await deps.enqueueEvent({
            taskId: task.id,
            pluginRunId: '',
            eventType: WORKBENCH_TASK_EVENT_TYPE.TASK_RELEASED,
            source: WORKBENCH_EVENT_SOURCE.PLUGIN,
            sequence: Date.now() + 1,
            ...eventFields,
            payload: rejection.payload,
          });
        }
        if (typeof deps.flushDeltas === 'function') {
          await deps.flushDeltas();
        }
      }
      if (lease) await clearActiveLease();
      return {
        success: true,
        skipped: true,
        reason: rejection.reasonMessage,
        preserveTaskExecutionContext,
        cleanupTask: cleanupTaskSnapshot(task),
      };
    }

    const executionLock = await acquireExecutionLock(task, preCheck, lease);
    if (!executionLock.acquired) {
      const lockAccountId = String(preCheck?.accountId || task?.accountId || '').trim();
      const lockPlatform = String(task?.platform || '').trim();
      const payload = buildAccountBusyPayload({
        accountId: lockAccountId,
        platform: lockPlatform,
        lockResult: executionLock,
      });
      if (lease) {
        await patchTask(task.id, {
          status: 'pending',
          progress: 0,
          errorMessage: payload.errorMessage,
        });
      }
      if (typeof deps.enqueueEvent === 'function' && task?.id) {
        const executorInstanceId = await resolveExecutorInstanceId();
        await deps.enqueueEvent({
          taskId: task.id,
          pluginRunId: '',
          eventType: WORKBENCH_TASK_EVENT_TYPE.TASK_RELEASED,
          source: WORKBENCH_EVENT_SOURCE.PLUGIN,
          sequence: Date.now(),
          ...buildLeaseEventFields({
            task,
            lease,
            executorInstanceId,
            accountId: lockAccountId,
          }),
          payload,
        });
        if (typeof deps.flushDeltas === 'function') await deps.flushDeltas();
      }
      if (lease) await clearActiveLease();
      return {
        success: true,
        skipped: true,
        reason: payload.reasonCode,
        cleanupTask: cleanupTaskSnapshot(task),
      };
    }

    let dispatch;
    try {
      dispatch = typeof deps.dispatchTask === 'function'
        ? await deps.dispatchTask(task)
        : { accepted: false };
    } catch (error) {
      const isRecoverable = isRecoverableConnectionError(error);
      const recoveryStatus = isRecoverable
        ? recoverableConnectionStatusForTask(task)
        : { status: 'failed', progress: 100, eventType: WORKBENCH_TASK_EVENT_TYPE.TASK_FAILED, message: '派发任务失败' };
      const errorMessage = String(error?.message || error || 'dispatch_failed');
      await patchTask(task.id, {
        status: recoveryStatus.status,
        progress: recoveryStatus.progress,
        errorMessage,
      });
      if (typeof deps.enqueueEvent === 'function') {
        const executorInstanceId = await resolveExecutorInstanceId();
        const eventFields = buildLeaseEventFields({
          task,
          lease,
          executorInstanceId,
          accountId: preCheck?.accountId,
        });
        await deps.enqueueEvent({
          taskId: task.id,
          pluginRunId: '',
          eventType: recoveryStatus.eventType,
          source: WORKBENCH_EVENT_SOURCE.PLUGIN,
          sequence: Date.now(),
          ...eventFields,
          payload: isRecoverable
            ? {
                reason: 'connection_interrupted',
                message: recoveryStatus.message,
                status: recoveryStatus.status,
                errorMessage,
              }
            : buildDispatchFailurePayload({
                reason: 'dispatch_failed',
                errorMessage,
                message: recoveryStatus.message,
              }),
        });
        if (typeof deps.flushDeltas === 'function') await deps.flushDeltas();
      }
      await releaseExecutionLock(executionLock.lock);
      if (lease) await clearActiveLease();
      return {
        success: false,
        skipped: false,
        reason: errorMessage,
        cleanupTask: cleanupTaskSnapshot(task),
      };
    }

    if (!dispatch?.accepted) {
      const errorMessage = String(dispatch?.error || 'dispatch_failed');
      await patchTask(task.id, {
        status: 'failed',
        progress: 100,
        errorMessage,
      });
      if (typeof deps.enqueueEvent === 'function') {
        const executorInstanceId = await resolveExecutorInstanceId();
        await deps.enqueueEvent({
          taskId: task.id,
          pluginRunId: '',
          eventType: WORKBENCH_TASK_EVENT_TYPE.TASK_FAILED,
          source: WORKBENCH_EVENT_SOURCE.PLUGIN,
          sequence: Date.now(),
          ...buildLeaseEventFields({
            task,
            lease,
            executorInstanceId,
            accountId: preCheck?.accountId,
          }),
          payload: buildDispatchFailurePayload({
            reason: 'dispatch_rejected',
            errorMessage,
            reasonCode: dispatch?.reasonCode || dispatch?.errorCode || '',
            message: dispatch?.reasonMessage || '页面拒绝执行任务',
          }),
        });
        if (typeof deps.flushDeltas === 'function') await deps.flushDeltas();
      }
      await releaseExecutionLock(executionLock.lock);
      if (lease) await clearActiveLease();
      return {
        success: false,
        skipped: false,
        reason: errorMessage,
        cleanupTask: cleanupTaskSnapshot(task),
      };
    }

    if (typeof deps.afterDispatchSuccess === 'function') {
      try {
        await deps.afterDispatchSuccess(task, preCheck, dispatch);
      } catch (error) {
        console.warn('[灵感爆爆爆] post-dispatch bookkeeping failed:', error);
      }
    }

    const dispatchCollectionRunId = resolveDispatchCollectionRunId(dispatch);
    const executorInstanceId = await resolveExecutorInstanceId();
    const accountId = String(preCheck?.accountId || '').trim();
    const pageFingerprint = buildPageFingerprint(dispatch?.capabilityReport);
    if (!lease || dispatchCollectionRunId) {
      await patchTask(task.id, buildStartupPatch({
        pluginRunId: dispatchCollectionRunId,
        activeExecutor: executorInstanceId,
      }));
    }
    state.activeTask = {
      taskId: task.id,
      externalTaskId: String(dispatch?.resultLookup?.externalTaskId || dispatch?.taskId || task.id).trim(),
      taskType: String(task?.taskType || '').trim(),
      platform: String(task?.platform || '').trim(),
      source: String(task?.source || '').trim(),
      taskStrategy: String(task?.taskStrategy || task?.payload?.taskStrategy || '').trim(),
      payload: task?.payload && typeof task.payload === 'object' ? task.payload : {},
      collectionProfile: resolveCollectionProfile(task),
      pluginRunId: dispatchCollectionRunId,
      executorInstanceId,
      accountId,
      tabId: toOptionalInteger(dispatch?.tabId) || toOptionalInteger(task?.tabId) || null,
      pluginOpenedTabId:
        toOptionalInteger(dispatch?.pluginOpenedTabId) ||
        toOptionalInteger(capability?.pluginOpenedTabId) ||
        toOptionalInteger(task?.pluginOpenedTabId) ||
        null,
      workbenchStatus: dispatchCollectionRunId ? 'running' : 'dispatched',
      resultFingerprint: '',
      controlCursor: '',
      errorMessage: '',
      attemptId: String(lease?.attemptId || task?.currentAttemptId || '').trim(),
      captureId: String(task?.captureId || lease?.captureId || '').trim(),
      executionPlanVersion: String(
        task?.executionPlanVersion || lease?.executionPlanVersion || '',
      ).trim(),
      leaseEpoch: toOptionalInteger(lease?.leaseEpoch ?? task?.leaseEpoch),
      executionPhase: String(task?.executionPhase || 'assigned').trim() || 'assigned',
      pageFingerprint,
      dispatchedAtMs: getNow(),
      attemptStartedAtMs: getNow(),
      pendingAccountUsageId: '',
      firstRecordSeen: false,
      streamedRecordCounts: {},
    };
    state.activeLease = lease
      ? normalizeLeaseSnapshot(lease)
      : null;
    if (!lease) {
      await enqueueTaskEvent(state.activeTask, WORKBENCH_TASK_EVENT_TYPE.TASK_CLAIMED, {
        status: dispatchCollectionRunId ? 'running' : 'dispatched',
        message: dispatchCollectionRunId ? '页面已创建本地执行单，任务开始执行' : '工作台任务已被插件认领',
        collectionRunId: dispatchCollectionRunId || undefined,
      });
    }
    if (dispatchCollectionRunId) {
      await enqueueTaskEvent(state.activeTask, WORKBENCH_TASK_EVENT_TYPE.TASK_PAGE_OPENED, {
        status: 'dispatched',
        message: '采集页已打开，等待执行确认',
        collectionRunId: dispatchCollectionRunId,
      });
      await enqueueTaskEvent(state.activeTask, WORKBENCH_TASK_EVENT_TYPE.TASK_RUNNING, {
        status: 'running',
        message: '采集页已开始执行',
        collectionRunId: dispatchCollectionRunId,
      });
    }
    await persistActiveTaskContext();
    state.lastIdleReason = null;
    return { success: true, accepted: true };
  }

  function getPluginRunId(activeTask = {}) {
    return String(activeTask.pluginRunId || activeTask.externalTaskId || activeTask.taskId || '').trim();
  }

  function executionContextForTask(activeTask = {}) {
    const lease = state.activeLease ? normalizeLeaseSnapshot(state.activeLease) : {};
    return {
      attemptId: String(lease.attemptId || activeTask.attemptId || '').trim(),
      leaseToken: String(lease.leaseToken || '').trim(),
      leaseEpoch: toOptionalInteger(lease.leaseEpoch ?? activeTask.leaseEpoch),
      pageFingerprint: activeTask.pageFingerprint || null,
      stationId: String(activeTask.executorInstanceId || '').trim(),
      accountId: String(activeTask.accountId || '').trim(),
      platform: String(activeTask.platform || '').trim(),
    };
  }

  async function enqueueTaskEvent(activeTask, eventType, payload = {}, options = {}) {
    if (!activeTask || typeof deps.enqueueEvent !== 'function') return null;
    const sequence = options.sequence || getNow();
    return deps.enqueueEvent({
      taskId: activeTask.taskId,
      pluginRunId: getPluginRunId(activeTask),
      eventType,
      source: options.source || WORKBENCH_EVENT_SOURCE.PLUGIN,
      sequence,
      controlRequestId: options.controlRequestId || '',
      payload: attachTaskRuntimeObservability({
        task: activeTask,
        payload,
        eventType,
        now: sequence,
        report: options.reportRuntime,
      }),
      snapshot: options.snapshot || null,
      executionContext: executionContextForTask(activeTask),
    });
  }

  async function consumeControlRequests(activeTask) {
    if (!activeTask || typeof deps.fetchControlRequests !== 'function' || typeof deps.applyTaskControl !== 'function') {
      return { success: true, controls: 0 };
    }

    let response;
    try {
      response = await deps.fetchControlRequests(activeTask.taskId, {
        executorInstanceId: activeTask.executorInstanceId || await resolveExecutorInstanceId(),
        after: activeTask.controlCursor || '',
      });
    } catch (error) {
      if (isMissingServerTaskError(error)) {
        return { success: false, missingActiveTask: true, reason: 'control_task_missing' };
      }
      throw error;
    }

    const controls = Array.isArray(response?.controls) ? response.controls : [];
    let handled = 0;
    let terminalControl = null;
    for (const control of controls) {
      const controlRequestId = String(control?.controlRequestId || control?.id || control?.idempotencyKey || '').trim();
      if (controlRequestId && state.seenControlIds.has(controlRequestId)) continue;

      const originalAction = String(control?.action || '').trim();
      const deleteRequested = originalAction === REMOTE_TASK_CONTROL_ACTION.DELETE;
      const actionToApply = deleteRequested ? REMOTE_TASK_CONTROL_ACTION.STOP : originalAction;
      const controlToApply = {
        ...control,
        taskId: String(control?.taskId || activeTask.taskId || '').trim(),
        taskType: String(control?.taskType || activeTask.taskType || '').trim(),
        action: actionToApply,
        originalAction,
        deleteRequested,
      };

      await enqueueTaskEvent(activeTask, WORKBENCH_TASK_EVENT_TYPE.TASK_CONTROL_REQUESTED, {
        controlRequestId,
        action: originalAction,
        source: control?.source || WORKBENCH_EVENT_SOURCE.WORKBENCH,
        requestedAt: control?.requestedAt || '',
      }, { controlRequestId, source: WORKBENCH_EVENT_SOURCE.WORKBENCH });

      try {
        const applied = await deps.applyTaskControl(controlToApply);
        if (applied?.success === false || applied?.accepted === false || applied?.error) {
          throw new Error(applied?.error || 'control_not_accepted');
        }
        await enqueueTaskEvent(activeTask, WORKBENCH_TASK_EVENT_TYPE.TASK_CONTROL_APPLIED, {
          controlRequestId,
          action: originalAction,
          appliedAction: actionToApply,
          deleteRequested,
        }, { controlRequestId });
        await enqueueTaskEvent(activeTask, mapControlActionToStateEvent(originalAction), {
          controlRequestId,
          action: originalAction,
          status: deleteRequested || actionToApply === REMOTE_TASK_CONTROL_ACTION.STOP ? 'stopping' : actionToApply,
          deleteRequested,
        }, { controlRequestId });
        if (isTerminalControlAction(originalAction)) {
          activeTask.workbenchStatus = 'stopping';
          activeTask.stopRequestedAtMs = getNow();
          activeTask.stopControlAction = originalAction;
          activeTask.deleteRequested = deleteRequested;
          terminalControl = {
            controlRequestId,
            action: originalAction,
            appliedAction: actionToApply,
            deleteRequested,
          };
        }
      } catch (error) {
        await enqueueTaskEvent(activeTask, WORKBENCH_TASK_EVENT_TYPE.TASK_CONTROL_FAILED, {
          controlRequestId,
          action: originalAction,
          errorMessage: String(error?.message || error || 'control_failed'),
          deleteRequested,
        }, { controlRequestId });
      }

      handled += 1;
      if (controlRequestId) state.seenControlIds.add(controlRequestId);
      activeTask.controlCursor = String(control?.cursor || response?.nextCursor || activeTask.controlCursor || '').trim();
    }

    if (response?.nextCursor && !controls.length) {
      activeTask.controlCursor = String(response.nextCursor || '').trim();
    }
    return { success: true, controls: handled, terminalControl };
  }

  async function pollActiveTask() {
    if (!state.activeTask || typeof deps.getResultPackage !== 'function') {
      return { success: true, idle: true };
    }
    const activeTask = state.activeTask;
    if (activeTask.workbenchStatus === 'stopping' || activeTask.deleteRequested) {
      return finalizeControlledStop(activeTask);
    }
    // V1.1（2026-06-29）：硬超时。任务执行超过 30 分钟强制终止，
    // 防止内容脚本死循环或页面卡死导致任务永久占用工位。
    if (
      Number(activeTask.attemptStartedAtMs || 0) > 0 &&
      getNow() - Number(activeTask.attemptStartedAtMs || 0) >= MAX_TASK_EXECUTION_TIME_MS
    ) {
      const timeoutMsg = `任务执行超过 ${Math.round(MAX_TASK_EXECUTION_TIME_MS / 60000)} 分钟，已自动终止。`;
      await notifyContentScriptToStop(activeTask);
      try {
        await patchTask(activeTask.taskId, {
          status: 'failed',
          pluginRunId: activeTask.pluginRunId || null,
          errorMessage: timeoutMsg,
          deferRelease: true,
        });
      } catch { /* 网络错误不阻塞本地清理 */ }
      await enqueueTerminalTaskEvent(activeTask, WORKBENCH_TASK_EVENT_TYPE.TASK_FAILED, {
        status: 'failed',
        progress: 100,
        errorMessage: timeoutMsg,
        reason: 'max_execution_time_exceeded',
      });
      const flushResult = await flushTerminalDeltasBeforeCleanup('terminal_timeout_snapshot_flush');
      if (!flushResult.success) {
        return {
          success: true,
          final: false,
          flushPending: true,
          reason: 'max_execution_time_snapshot_pending',
          cleanupTask: cleanupTaskSnapshot(activeTask),
        };
      }
      try {
        await patchTask(activeTask.taskId, {
          status: 'failed',
          pluginRunId: activeTask.pluginRunId || null,
          errorMessage: timeoutMsg,
        });
      } catch { /* release retry is handled by later reconciliation */ }
      state.activeTask = null;
      state.seenControlIds.clear();
      await clearActiveLease(activeTask);
      return {
        success: true,
        final: true,
        released: true,
        reason: 'max_execution_time_exceeded',
        cleanupTask: cleanupTaskSnapshot(activeTask),
      };
    }
    // V1.1（2026-06-29）：非监控任务暂停超过阈值自动释放。
    // BUG B2：此前 paused 任务永不自动释放，state.activeTask 永久残留。
    if (
      activeTask.workbenchStatus === 'paused' &&
      !isMonitorTask(activeTask) &&
      Number(activeTask.pausedAtMs || activeTask.attemptStartedAtMs || 0) > 0 &&
      getNow() - Number(activeTask.pausedAtMs || activeTask.attemptStartedAtMs || 0) >= PAUSED_TASK_AUTO_RELEASE_MS
    ) {
      const msg = '任务已暂停超过 10 分钟，自动释放。';
      await patchTask(activeTask.taskId, {
        status: 'pending',
        progress: 0,
        pluginRunId: activeTask.pluginRunId || null,
        errorMessage: msg,
        notBeforeAt: new Date(getNow() + LOCAL_ACTIVE_TASK_WITHOUT_LEASE_RETRY_DELAY_MS).toISOString(),
      });
      state.activeTask = null;
      state.seenControlIds.clear();
      await clearActiveLease(activeTask);
      return {
        success: true,
        released: true,
        reason: 'paused_task_auto_release',
        cleanupTask: cleanupTaskSnapshot(activeTask),
      };
    }
    if (
      !state.activeLease &&
      !isMonitorTask(activeTask) &&
      Number(activeTask.attemptStartedAtMs || 0) > 0 &&
      getNow() - Number(activeTask.attemptStartedAtMs || 0) >= LOCAL_ACTIVE_TASK_WITHOUT_LEASE_TIMEOUT_MS
    ) {
      const message = '插件本地任务没有有效租约，已自动释放重试。';
      const notBeforeAt = new Date(getNow() + LOCAL_ACTIVE_TASK_WITHOUT_LEASE_RETRY_DELAY_MS).toISOString();
      await patchTask(activeTask.taskId, {
        status: 'pending',
        progress: 0,
        pluginRunId: activeTask.pluginRunId || null,
        errorMessage: message,
        notBeforeAt,
      });
      await enqueueTaskEvent(activeTask, WORKBENCH_TASK_EVENT_TYPE.TASK_HEARTBEAT, {
        status: 'pending',
        errorMessage: message,
        userMessage: message,
        reason: 'local_lease_missing_timeout',
        retryAfterMs: LOCAL_ACTIVE_TASK_WITHOUT_LEASE_RETRY_DELAY_MS,
        notBeforeAt,
      });
      state.activeTask = null;
      state.seenControlIds.clear();
      await clearActiveLease(activeTask);
      return {
        success: true,
        released: true,
        reason: 'local_lease_missing_timeout',
        cleanupTask: cleanupTaskSnapshot(activeTask),
      };
    }
    if (state.activeLease && typeof deps.renewTaskLease === 'function') {
      try {
        const renewal = await deps.renewTaskLease(activeTask.taskId, state.activeLease, {
          status: activeTask.workbenchStatus || 'running',
        });
        if (renewal?.expiresAt) {
          state.activeLease = {
            ...normalizeLeaseSnapshot(state.activeLease),
            expiresAt: String(renewal.expiresAt || '').trim(),
          };
        }
      } catch (error) {
        if (isAuthorizationFailureError(error)) {
          await enqueueTaskEvent(activeTask, WORKBENCH_TASK_EVENT_TYPE.TASK_HEARTBEAT, {
            leaseRenewalFailed: true,
            reason: 'authorization_invalid',
            errorMessage: String(error?.message || error || 'authorization_invalid'),
            retryAfterMs: AUTHORIZATION_FAILURE_IDLE_MS,
          });
          return handleAuthorizationFailure(error);
        }
        if (isLeaseConflictError(error)) {
          await enqueueTaskEvent(activeTask, WORKBENCH_TASK_EVENT_TYPE.TASK_HEARTBEAT, {
            leaseRenewalFailed: true,
            reason: 'lease_conflict',
            errorMessage: String(error?.message || error || 'lease_conflict'),
            retryAfterMs: LEASE_CONFLICT_RETRY_DELAY_MS,
          });
          state.activeTask = null;
          state.seenControlIds.clear();
          await clearActiveLease(activeTask);
          return {
            success: true,
            released: true,
            reason: 'lease_conflict',
            nextPollAfterMs: LEASE_CONFLICT_RETRY_DELAY_MS,
            cleanupTask: cleanupTaskSnapshot(activeTask),
          };
        }
        // V1.1（2026-06-29）：网络错误导致续期失败时不静默。
        // 标记租约可能已过期，下次 poll 时调 reconcile。
        if (state.activeLease) {
          state.activeLease.renewalFailedAtMs = getNow();
          state.activeLease.renewalFailedReason = String(error?.message || error || 'lease_renewal_failed');
        }
        await enqueueTaskEvent(activeTask, WORKBENCH_TASK_EVENT_TYPE.TASK_HEARTBEAT, {
          leaseRenewalFailed: true,
          errorMessage: String(error?.message || error || 'lease_renewal_failed'),
        });
      }
    }
    // V1.1（2026-06-29）：租约续期连续失败时强制 reconcile。
    if (
      state.activeLease?.renewalFailedAtMs &&
      getNow() - state.activeLease.renewalFailedAtMs >= 60_000
    ) {
      console.warn('[taskPoller] lease renewal failed for 60s, forcing release');
      state.activeTask = null;
      state.seenControlIds.clear();
      await clearActiveLease(activeTask);
      return {
        success: true,
        released: true,
        reason: 'lease_renewal_persistent_failure',
        cleanupTask: cleanupTaskSnapshot(activeTask),
      };
    }
    await refreshExecutionLock(activeTask);
    const controlResult = await consumeControlRequests(activeTask);
    if (controlResult?.terminalControl) {
      return finalizeControlledStop(activeTask, controlResult.terminalControl);
    }
    if (controlResult?.missingActiveTask) {
      const cleanupTask = cleanupTaskSnapshot(activeTask);
      state.activeTask = null;
      state.seenControlIds.clear();
      await clearActiveLease(activeTask);
      return {
        success: true,
        released: true,
        reason: controlResult.reason || 'control_task_missing',
        cleanupTask,
      };
    }

    let result;
    try {
        const lookup = {
          collectionRunId: String(activeTask.pluginRunId || '').trim(),
          externalTaskId: String(activeTask.externalTaskId || '').trim(),
        };
        const tabId = toOptionalInteger(activeTask.tabId);
        if (tabId) lookup.tabId = tabId;
        result = await deps.getResultPackage(lookup);
    } catch (error) {
      result = { success: false, error: String(error?.message || error || 'result_lookup_failed') };
    }

    if (!result?.success) {
      if (hasResultLookupError(result)) {
        const now = getNow();
        if (
          activeTask.workbenchStatus === 'running' &&
          now - Number(activeTask.attemptStartedAtMs || 0) >= RUNNING_RESULT_LOOKUP_TIMEOUT_MS
        ) {
          const handoffErrorMessage = '采集页结果包没有交回工作台：插件没有找到本轮执行页，已停止这条卡住的任务。';
          await patchTask(activeTask.taskId, {
            status: 'failed',
            progress: 100,
            pluginRunId: activeTask.pluginRunId || null,
            errorMessage: handoffErrorMessage,
            deferRelease: true,
          });
          await enqueueTerminalTaskEvent(activeTask, WORKBENCH_TASK_EVENT_TYPE.TASK_FAILED, {
            status: 'failed',
            progress: 100,
            reason: 'result_package_handoff_lost',
            reasonCode: 'result_package_handoff_lost',
            errorMessage: handoffErrorMessage,
            userMessage: handoffErrorMessage,
          });
          const flushResult = await flushTerminalDeltasBeforeCleanup('terminal_handoff_lost_snapshot_flush');
          if (!flushResult.success) {
            return {
              success: true,
              final: false,
              flushPending: true,
              reason: 'result_package_handoff_lost_snapshot_pending',
              cleanupTask: cleanupTaskSnapshot(activeTask),
            };
          }
          try {
            await patchTask(activeTask.taskId, {
              status: 'failed',
              progress: 100,
              pluginRunId: activeTask.pluginRunId || null,
              errorMessage: handoffErrorMessage,
            });
          } catch { /* release retry is handled by later reconciliation */ }
          await notifyContentScriptToStop(activeTask);
          state.activeTask = null;
          state.seenControlIds.clear();
          await clearActiveLease(activeTask);
          return {
            success: false,
            failed: true,
            reason: 'result_package_handoff_lost',
            cleanupTask: cleanupTaskSnapshot(activeTask),
          };
        }
        if (
          activeTask.workbenchStatus === 'dispatched' &&
          now - Number(activeTask.attemptStartedAtMs || 0) >= DISPATCH_STARTUP_TIMEOUT_MS
        ) {
          const startupErrorMessage = '任务已派出，但页面没有真正启动，已自动释放重试。';
          const notBeforeAt = new Date(now + DISPATCH_STARTUP_RETRY_DELAY_MS).toISOString();
          await patchTask(activeTask.taskId, {
            status: 'pending',
            progress: 0,
            pluginRunId: activeTask.pluginRunId || null,
            errorMessage: startupErrorMessage,
            notBeforeAt,
          });
          await enqueueTaskEvent(activeTask, WORKBENCH_TASK_EVENT_TYPE.TASK_PAGE_OPEN_FAILED, {
            status: 'pending',
            errorMessage: startupErrorMessage,
            userMessage: startupErrorMessage,
            reason: 'dispatch_startup_timeout',
            retryAfterMs: DISPATCH_STARTUP_RETRY_DELAY_MS,
            notBeforeAt,
          });
          state.activeTask = null;
          state.seenControlIds.clear();
          await clearActiveLease(activeTask);
          return {
            success: true,
            released: true,
            reason: 'dispatch_startup_timeout',
            cleanupTask: cleanupTaskSnapshot(activeTask),
          };
        }
        return { success: true, waiting: true };
      }
      const errorMessage = String(result?.error || 'result_lookup_failed');
      if (isRecoverableConnectionError(errorMessage)) {
        const recoveryStatus = recoverableConnectionStatusForTask(activeTask);
        const pausedProgress = activeTask.workbenchStatus === 'running' ? buildRunningProgress({}) : 5;
        await patchTask(activeTask.taskId, {
          status: recoveryStatus.status,
          progress: recoveryStatus.status === 'failed' ? 100 : pausedProgress,
          pluginRunId: activeTask.pluginRunId || null,
          errorMessage,
          ...(recoveryStatus.status === 'failed' ? { deferRelease: true } : {}),
        });
        const enqueueRecoveryEvent = recoveryStatus.status === 'failed'
          ? enqueueTerminalTaskEvent
          : enqueueTaskEvent;
        await enqueueRecoveryEvent(activeTask, recoveryStatus.eventType, {
          status: recoveryStatus.status,
          errorMessage,
          message: recoveryStatus.message,
        });
        if (recoveryStatus.status === 'failed') {
          const flushResult = await flushTerminalDeltasBeforeCleanup('terminal_connection_failed_snapshot_flush');
          if (!flushResult.success) {
            return {
              success: true,
              final: false,
              flushPending: true,
              reason: 'connection_failed_snapshot_pending',
              cleanupTask: cleanupTaskSnapshot(activeTask),
            };
          }
          try {
            await patchTask(activeTask.taskId, {
              status: recoveryStatus.status,
              progress: 100,
              pluginRunId: activeTask.pluginRunId || null,
              errorMessage,
            });
          } catch { /* release retry is handled by later reconciliation */ }
          await notifyContentScriptToStop(activeTask);
          state.activeTask = null;
          await clearActiveLease(activeTask);
          return {
            success: false,
            failed: true,
            cleanupTask: cleanupTaskSnapshot(activeTask),
          };
        }
        await notifyContentScriptToStop(activeTask);
        return { success: true, paused: true };
      }
      await patchTask(activeTask.taskId, {
        status: 'failed',
        progress: 100,
        pluginRunId: activeTask.pluginRunId || null,
        errorMessage: errorMessage,
        deferRelease: true,
      });
      await enqueueTerminalTaskEvent(activeTask, WORKBENCH_TASK_EVENT_TYPE.TASK_FAILED, {
        status: 'failed',
        progress: 100,
        errorMessage,
        userMessage: result?.userMessage || '任务执行失败',
        errorCode: result?.errorCode || result?.reasonCode || '',
      });
      const flushResult = await flushTerminalDeltasBeforeCleanup('terminal_result_lookup_failed_snapshot_flush');
      if (!flushResult.success) {
        return {
          success: true,
          final: false,
          flushPending: true,
          reason: 'result_lookup_failed_snapshot_pending',
          cleanupTask: cleanupTaskSnapshot(activeTask),
        };
      }
      try {
        await patchTask(activeTask.taskId, {
          status: 'failed',
          progress: 100,
          pluginRunId: activeTask.pluginRunId || null,
          errorMessage,
        });
      } catch { /* release retry is handled by later reconciliation */ }
      await notifyContentScriptToStop(activeTask);
      state.activeTask = null;
      await clearActiveLease(activeTask);
      return {
        success: false,
        failed: true,
        cleanupTask: cleanupTaskSnapshot(activeTask),
      };
    }

    const run = result?.result || {};
    let mapped = mapRunStatusToWorkbenchStatus(run.status);
    let captureSubmissionV2 = null;
    let captureMappingFailure = null;
    if (mapped.final) {
      try {
        captureSubmissionV2 = buildRuntimeCaptureSubmission(activeTask, run);
      } catch (error) {
        captureMappingFailure = error;
        mapped = { status: 'failed', progress: 100, final: true };
      }
    }
    const pluginRunId = String(run.collectionRunId || activeTask.pluginRunId || '').trim();
    await consumePendingAccountUsage(activeTask, mapped.status, pluginRunId);
    const resultSummary = buildWorkbenchResultSummary(run);
    const errorMessage = captureMappingFailure
      ? `v2_capture_mapping_failed:${String(captureMappingFailure.reasonCode || 'unknown')}`
      : resolveRunErrorMessage(run);
    if (captureMappingFailure) {
      const cleanupTask = cleanupTaskSnapshot(activeTask);
      await patchTask(activeTask.taskId, {
        status: 'failed',
        progress: 100,
        pluginRunId: pluginRunId || null,
        resultSummary,
        errorMessage,
        deferRelease: false,
        failurePath: 'v2_non_evidence_terminal_control',
      });
      await notifyContentScriptToStop(activeTask);
      state.activeTask = null;
      state.seenControlIds.clear();
      await clearActiveLease(activeTask);
      return {
        success: false,
        failed: true,
        final: true,
        status: 'failed',
        reason: 'v2_capture_mapping_failed_non_evidence_control',
        reasonCode: String(captureMappingFailure.reasonCode || 'unknown'),
        evidenceSubmitted: false,
        cleanupTask,
      };
    }
    const recordDeltas = buildWorkbenchRecordDeltas(
      activeTask,
      pluginRunId || getPluginRunId(activeTask),
      resultSummary,
      getNow(),
      executionContextForTask(activeTask),
    );
    const failurePayload = mapped.status === 'failed'
      ? buildFailureEventPayload({
          run,
          status: mapped.status,
          progress: mapped.progress ?? buildRunningProgress(run),
          errorMessage,
          latestSummary: run?.resultSummary || {},
          reasonCode: captureMappingFailure?.reasonCode,
        })
      : null;
    const fingerprint = JSON.stringify({
      status: mapped.status,
      pluginRunId,
      resultSummary,
      errorMessage,
      failurePayload,
    });

    if (
      fingerprint !== activeTask.resultFingerprint ||
      mapped.status !== activeTask.workbenchStatus ||
      errorMessage !== activeTask.errorMessage
    ) {
      await patchTask(activeTask.taskId, {
        status: mapped.status,
        progress: mapped.progress ?? buildRunningProgress(run),
        pluginRunId: pluginRunId || null,
        resultSummary,
        errorMessage: errorMessage || null,
        ...(mapped.final === true && mapped.status !== 'completed' ? { deferRelease: true } : {}),
      });
      activeTask.pluginRunId = pluginRunId;
      activeTask.workbenchStatus = mapped.status;
      activeTask.resultFingerprint = fingerprint;
      activeTask.errorMessage = errorMessage;
      if (mapped.final && recordDeltas.length && typeof deps.enqueueRecords === 'function') {
        try {
          await deps.enqueueRecords(recordDeltas);
	        } catch (error) {
	          if (!isRecordSchemaValidationError(error)) throw error;
	          const schemaErrorMessage = buildRecordSchemaFailureMessage(error);
	          await patchTask(activeTask.taskId, {
	            status: 'failed',
	            progress: 100,
	            pluginRunId: pluginRunId || null,
	            resultSummary,
	            errorMessage: schemaErrorMessage,
	            deferRelease: true,
	          });
	          await enqueueTerminalTaskEvent(activeTask, WORKBENCH_TASK_EVENT_TYPE.TASK_FAILED, {
	            status: 'failed',
	            progress: 100,
	            errorMessage: schemaErrorMessage,
	            userMessage: '采集结果不完整，已停止本轮同步并记录健康告警。',
	            reasonCode: error.reasonCode || error.code || 'record_payload_schema_invalid',
	            errorCode: error.code || error.reasonCode || 'record_payload_schema_invalid',
	            recordType: error.observability?.recordType || '',
	            observability: error.observability || {},
	            latestSummary: run?.resultSummary || {},
	          }, { reportRuntime: true });
	          const flushResult = await flushTerminalDeltasBeforeCleanup('terminal_schema_invalid_snapshot_flush');
	          if (!flushResult.success) {
	            return {
	              success: true,
	              final: false,
	              flushPending: true,
	              reason: 'record_payload_schema_invalid_snapshot_pending',
	              cleanupTask: cleanupTaskSnapshot(activeTask),
	            };
	          }
	          try {
	            await patchTask(activeTask.taskId, {
	              status: 'failed',
	              progress: 100,
	              pluginRunId: pluginRunId || null,
	              resultSummary,
	              errorMessage: schemaErrorMessage,
	            });
	          } catch { /* release retry is handled by later reconciliation */ }
	          await notifyContentScriptToStop(activeTask);
          const cleanupTask = cleanupTaskSnapshot(activeTask);
          state.activeTask = null;
          state.seenControlIds.clear();
          await clearActiveLease(activeTask);
          return {
            success: false,
            failed: true,
            reason: 'record_payload_schema_invalid',
            cleanupTask,
          };
        }
        if (!activeTask.firstRecordSeen) {
          activeTask.firstRecordSeen = true;
          await enqueueTaskEvent(activeTask, WORKBENCH_TASK_EVENT_TYPE.TASK_FIRST_RECORD_SEEN, {
            status: mapped.status,
            recordCount: recordDeltas.length,
            collectionRunId: pluginRunId || getPluginRunId(activeTask),
          });
        }
      }
      await enqueueTaskEvent(activeTask, mapWorkbenchStatusToEventType(mapped.status), {
        ...(failurePayload || {
          status: mapped.status,
          progress: mapped.progress ?? buildRunningProgress(run),
          errorMessage: errorMessage || '',
          latestSummary: run?.resultSummary || {},
        }),
      }, {
        snapshot: {
          status: mapped.status,
          progress: mapped.progress ?? buildRunningProgress(run),
          latestSummary: run?.resultSummary || {},
          latestHeartbeatAt: new Date().toISOString(),
          ...(captureSubmissionV2 ? { captureSubmissionV2 } : {}),
        },
      });
      await persistActiveTaskContext();
    }

    if (mapped.final) {
      const cleanupTask = cleanupTaskSnapshot(activeTask);
	      // 终态结果包必须先提交成功，才能清理本地执行凭证。
	      let flushError = null;
	      try {
	        await flushDeltasBeforeCleanup();
	      } catch (err) {
	        flushError = err;
	        console.warn('[taskPoller] flushDeltasBeforeCleanup failed, keeping active lease for retry:', err?.message || err);
	      }
	      if (flushError) {
	        return {
	          success: true,
	          final: false,
	          status: mapped.status,
	          flushPending: true,
	          cleanupTask,
	        };
	      }
	      if (mapped.status !== 'completed') {
	        try {
	          await patchTask(activeTask.taskId, {
	            status: mapped.status,
	            progress: mapped.progress ?? buildRunningProgress(run),
	            pluginRunId: pluginRunId || null,
	            resultSummary,
	            errorMessage: errorMessage || null,
	          });
	        } catch { /* release retry is handled by later reconciliation */ }
	      }
	      state.activeTask = null;
	      state.seenControlIds.clear();
	      await clearActiveLease(activeTask);
	      return {
        success: true,
        final: true,
        status: mapped.status,
        cleanupTask,
      };
    }

    return { success: true, final: mapped.final, status: mapped.status };
  }

  async function reconcileBeforeClaim() {
    if (typeof deps.reconcileTaskLease !== 'function') return null;

    const localLease = typeof deps.readTaskLease === 'function'
      ? await deps.readTaskLease()
      : null;
    const now = getNow();
    if (!localLease?.leaseToken && now - lastReconcileAtMs < RECONCILE_IDLE_INTERVAL_MS) {
      return null;
    }
    lastReconcileAtMs = now;

    let result;
    try {
      result = await deps.reconcileTaskLease({ localLease });
    } catch (error) {
      if (isAuthorizationFailureError(error)) {
        return { idleResult: await handleAuthorizationFailure(error) };
      }
      if (error?.retryable) {
        const idleResult = buildRetryableClaimIdleResult(error);
        state.lastIdleReason = createTaskLeaseIdleSnapshot(idleResult);
        return { idleResult };
      }
      return null;
    }

    if (!result?.success || result?.skipped) return null;
    const action = String(result.action || '').trim();
    if (['clear_local', 'idle', 'release', 'released', 'expired'].includes(action)) {
      const activeTask = state.activeTask;
      state.activeTask = null;
      state.seenControlIds.clear();
      await clearActiveLease(activeTask);
      return null;
    }

    const lease = result.lease || result.serverLease || null;
    if (lease?.taskId && lease?.leaseToken) {
      state.activeLease = normalizeLeaseSnapshot(lease);
    }

    if (result.task?.id) {
      const persistedContext = await readActiveTaskContext(result.task.id);
      const persistedAttemptId = String(persistedContext?.attemptId || '').trim();
      const persistedPluginRunId = String(persistedContext?.pluginRunId || '').trim();
      const serverPluginRunId = String(result.task?.pluginRunId || result.task?.collectionRunId || '').trim();
      const currentAttemptId = String(
        state.activeLease?.attemptId
        || result.task?.currentAttemptId
        || result.task?.attemptId
        || '',
      ).trim();
      const sameLocalRun = Boolean(serverPluginRunId && persistedPluginRunId === serverPluginRunId);
      const hydrated = hydrateTrackedTask(
        result.task,
        now,
        persistedAttemptId && currentAttemptId && persistedAttemptId !== currentAttemptId && !sameLocalRun
          ? null
          : persistedContext,
      );
      if (hydrated) {
        state.activeTask = hydrated;
        await persistActiveTaskContext();
        return { recoveredTask: hydrated };
      }
    }

    return null;
  }

  async function tick() {
    if (tickPromise) {
      const timeoutMs = Number.isFinite(Number(deps.tickStaleTimeoutMs))
        ? Math.max(1, Number(deps.tickStaleTimeoutMs))
        : TICK_STALE_TIMEOUT_MS;
      const ageMs = getNow() - tickStartedAtMs;
      if (Number.isFinite(ageMs) && ageMs < timeoutMs) {
        return { success: true, skipped: true, reason: 'tick_in_progress' };
      }
      console.warn('[灵感爆爆爆] workbench task poll tick timed out, starting a fresh tick', {
        ageMs,
        timeoutMs,
      });
      tickPromise = null;
      tickStartedAtMs = 0;
    }
    tickStartedAtMs = getNow();
    let currentTickPromise;
    currentTickPromise = (async () => {
      state.ticking = true;
      try {
        const authorizationBackoff = await readAuthorizationFailureBackoff();
        if (authorizationBackoff) {
          return authorizationBackoff;
        }

        if (state.activeTask) {
          return await pollActiveTask();
        }

        const reconciled = await reconcileBeforeClaim();
        if (reconciled?.idleResult) {
          return reconciled.idleResult;
        }
        if (reconciled?.recoveredTask) {
          return await pollActiveTask();
        }

        const backlogBlock = await evaluateOutboxBacklogGate();
        if (backlogBlock) {
          const idleSnapshot = createTaskLeaseIdleSnapshot(backlogBlock);
          state.lastIdleReason = idleSnapshot;
          return buildIdleTickResult(backlogBlock, idleSnapshot);
        }

        if (typeof deps.claimTaskLease !== 'function') {
          state.lastIdleReason = null;
          await clearAuthorizationFailureBackoff();
          return buildIdleTickResult({}, null);
        }

        let claimed;
        try {
          claimed = await deps.claimTaskLease();
        } catch (error) {
          if (isAuthorizationFailureError(error)) {
            return handleAuthorizationFailure(error);
          }
          if (error?.retryable) {
            const idleResult = buildRetryableClaimIdleResult(error);
            state.lastIdleReason = createTaskLeaseIdleSnapshot(idleResult);
            return idleResult;
          }
          throw error;
        }
        if (claimed?.task) {
          await clearAuthorizationFailureBackoff();
          return await claimTask(claimed.task, claimed.lease || null);
        }

        const idleSnapshot = createTaskLeaseIdleSnapshot(claimed);
        state.lastIdleReason = idleSnapshot;
        if (idleSnapshot?.idleReasonCode === 'PLUGIN_VERSION_OUTDATED') {
          await persistAuthorizationFailureBackoff(buildIdleTickResult(claimed, idleSnapshot));
        } else {
          await clearAuthorizationFailureBackoff();
        }
        return buildIdleTickResult(claimed, idleSnapshot);
      } finally {
        if (tickPromise === currentTickPromise) {
          state.ticking = false;
          tickPromise = null;
          tickStartedAtMs = 0;
        }
      }
    })();
    tickPromise = currentTickPromise;
    return tickPromise;
  }

  function getState() {
    return {
      activeTask: state.activeTask ? { ...state.activeTask } : null,
      activeLease: state.activeLease ? { ...state.activeLease } : null,
      ticking: state.ticking,
      lastIdleReason: state.lastIdleReason ? { ...state.lastIdleReason } : null,
    };
  }

  function getExecutionContext(taskId = '') {
    const normalizedTaskId = String(taskId || '').trim();
    if (!normalizedTaskId || state.activeTask?.taskId !== normalizedTaskId) return null;
    return {
      ...(state.activeLease ? normalizeLeaseSnapshot(state.activeLease) : {}),
      attemptId: String(state.activeLease?.attemptId || state.activeTask?.attemptId || '').trim(),
      leaseEpoch: toOptionalInteger(state.activeLease?.leaseEpoch ?? state.activeTask?.leaseEpoch),
      pageFingerprint: state.activeTask?.pageFingerprint || null,
      stationId: String(state.activeTask?.executorInstanceId || '').trim(),
      accountId: String(state.activeTask?.accountId || '').trim(),
      platform: String(state.activeTask?.platform || '').trim(),
    };
  }

  return {
    tick,
    getState,
    getExecutionContext,
    persistActiveTaskContext,
    updateActiveTask,
  };
}
