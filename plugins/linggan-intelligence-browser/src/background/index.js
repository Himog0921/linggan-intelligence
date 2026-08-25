import '../extensionPublicPath.js';
import { MSG } from '../shared/constants.js';
import { parseTargetIdentity } from '../shared/targetIdentity.js';
import {
  testConnection,
  getFlywheelConfig,
  saveFlywheelConfig,
  mergeFlywheelAuthorization,
  syncManualRecordsToWorkbench,
} from '../sync/flywheelSync.js';
import { validateCapabilityCheck, validateTaskControl, validateTaskEnvelope } from '../workbench/protocol/validator.js';
import {
  REMOTE_ERROR_CODE,
  REMOTE_TASK_TYPE,
  WORKBENCH_EVENT_SOURCE,
  WORKBENCH_MESSAGE_TYPE,
  WORKBENCH_PROTOCOL_VERSION,
  WORKBENCH_RECORD_TYPE,
  WORKBENCH_TASK_EVENT_TYPE,
} from '../workbench/protocol/schema.js';
import { mapTaskEnvelopeToCapabilityCheck, mapTaskEnvelopeToInternalCommand } from '../workbench/runtime/taskEnvelopeMapper.js';
import { mapTaskControlToInternalCommand } from '../workbench/runtime/taskControlMapper.js';
import { createResultPackager } from '../workbench/runtime/resultPackager.js';
import { resolveCollectionProfile } from '../workbench/runtime/collectionProfile.js';
import { canDispatchTaskFromCapabilityReport } from '../workbench/runtime/capabilityCheck.js';
import { createExecutionStationClient } from '../workbench/runtime/executionStationClient.js';
import {
  claimCollectionTaskLease,
  commitCollectionTaskDeltaThroughSync,
  createTaskLeaseStorageStore,
  reconcileExecutionStationLease,
  renewCollectionTaskLease,
  syncCollectionTaskStatusThroughSync,
} from '../workbench/runtime/taskLeaseClient.js';
import {
  collectStationRuntimeStates,
  collectStationPlatformAccounts,
  MONITOR_STATION_CAPABILITIES,
  stationCapabilitiesForRuntimeStates,
} from '../workbench/runtime/executionStationRuntime.js';
import { createTaskPoller } from '../workbench/runtime/taskPoller.js';
import {
  createExecutionAccountLockManager,
  createExecutionAccountLockStorageStore,
} from '../workbench/runtime/executionAccountLock.js';
import { createManualExecutionLockCoordinator } from '../workbench/runtime/manualExecutionLock.js';
import {
  MIN_CHROME_ALARM_INTERVAL_MS,
  scheduleWorkbenchTaskPollAlarm,
  shouldRunWorkbenchTaskPollAfterHeartbeatResult,
} from '../workbench/runtime/taskPollSchedule.js';
import {
  parseWorkbenchPushPayload,
  registerWorkbenchPushSubscription,
  shouldWakeForWorkbenchPush,
} from '../workbench/runtime/workbenchPushSubscription.js';
import { createTaskDeltaReporter } from '../workbench/runtime/taskDeltaReporter.js';
import {
  normalizeNavigatedTaskTabsSnapshot,
  rememberNavigatedTaskTab,
  removeNavigatedTaskTabs,
  taskExecutionCleanupKeys,
} from '../workbench/runtime/taskExecutionCleanup.js';
import { normalizeProgressEvent } from '../workbench/runtime/progressEvent.js';
import { attachTaskRuntimeObservability } from '../workbench/runtime/taskRuntimeObservability.js';
import { navigateToTask, closeTab } from '../workbench/runtime/navigationOrchestrator.js';
import { getPersistentExecutorInstanceId } from '../workbench/runtime/executorIdentity.js';
import {
  createPluginAuthorizationClient,
  getPluginAuthorizationBlockedMessage,
  hasActivePluginAuthorization,
} from '../workbench/runtime/pluginAuthorization.js';
import { applyPackagedInstallBootstrap } from '../workbench/runtime/pluginInstallBootstrap.js';
import { enrichNoteWithDataFoundationPayload } from '../workbench/runtime/dataFoundationPayload.js';
import { collectionRunStore } from '../db/collectionRunStore.js';
import { workbenchOutboxStore } from '../db/workbenchOutboxStore.js';
import { captureJournalStore } from '../db/captureJournalStore.js';
import { buildOutboxRecoveryExport } from '../workbench/runtime/outboxRecoveryExport.js';
import { accountStore } from '../db/accountStore.js';
import { injectCookiesForAccount, selectAccountForWorkbenchTask, selectAvailableAccount } from '../workbench/runtime/cookieManager.js';
import { noteStore } from '../db/noteStore.js';
import { commentStore } from '../db/commentStore.js';
import { authorStore } from '../db/authorStore.js';
import { mediaAssetStore } from '../db/mediaAssetStore.js';
import { sendToTab as sendSharedToTab } from '../shared/messaging.js';
import { normalizeWorkbenchMessageResponse } from '../workbench/protocol/responseEnvelope.js';
import {
  buildDownloadHeaders,
  fetchBinaryAsDataUrl,
  sanitizeDownloadFilename,
  tryDownloadCandidate,
} from './downloadService.js';
import { authorizeBackgroundMessage, createSensitiveActionSet } from './messageSecurity.js';

// ========== 启动时清理残留规则 ==========
// 防止上次异常退出后 BLOCK_MEDIA 规则残留导致页面加载不出来
chrome.declarativeNetRequest.updateDynamicRules({
  removeRuleIds: [1],
}).catch(() => {});

const resultPackager = createResultPackager({
  collectionRunStore,
  noteStore,
  commentStore,
  authorStore,
  mediaAssetStore,
});
const workbenchTaskRegistry = new Map();
const navigatedTabs = new Map();
const NAVIGATED_TASK_TABS_STORAGE_KEY = 'workbenchNavigatedTaskTabs';
const ACTIVE_TASK_CONTEXT_STORAGE_KEY = 'workbenchActiveTaskContext';
const EXECUTION_ACCOUNT_LOCK_STORAGE_KEY = 'workbenchExecutionAccountLocks';
const WORKBENCH_TASK_AUTH_BACKOFF_STORAGE_KEY = 'workbenchTaskAuthorizationBackoff';
const WORKBENCH_STATION_HEARTBEAT_BACKOFF_STORAGE_KEY = 'workbenchStationHeartbeatBackoff';
const WORKBENCH_TASK_POLL_NEXT_STORAGE_KEY = 'workbenchTaskPollNextAtMs';
const WORKBENCH_STATION_HEARTBEAT_NEXT_STORAGE_KEY = 'workbenchStationHeartbeatNextAtMs';
const WORKBENCH_PUSH_SUBSCRIPTION_NEXT_STORAGE_KEY = 'workbenchPushSubscriptionNextAtMs';
const WORKBENCH_TASK_POLL_ALARM = 'workbench-task-poll';
const AUTHORIZATION_FAILURE_IDLE_MS = 15 * 60 * 1000;
const WORKBENCH_STATION_HEARTBEAT_ALARM = 'workbench-station-heartbeat';
const WORKBENCH_STATION_HEARTBEAT_INTERVAL_MS = 60 * 1000;
const INITIAL_WORKBENCH_TASK_POLL_MINUTES = 0.5;
const WORKBENCH_PUSH_SUBSCRIPTION_REFRESH_MS = 6 * 60 * 60 * 1000;
const WORKBENCH_PUSH_SUBSCRIPTION_RETRY_MS = 15 * 60 * 1000;
const WORKBENCH_PUSH_SUBSCRIPTION_UNREGISTERED_RETRY_MS = 30 * 1000;

let consecutiveEmptyPolls = 0;
let nextExecutionStationHeartbeatAtMs = 0;
let nextWorkbenchTaskPollAtMs = 0;
let nextWorkbenchPushSubscriptionAtMs = 0;

function navigatedTaskTabStorageArea() {
  return globalThis.chrome?.storage?.session || globalThis.chrome?.storage?.local || null;
}

function navigatedTabsSnapshotFromMemory() {
  return normalizeNavigatedTaskTabsSnapshot(Object.fromEntries(navigatedTabs.entries()));
}

async function readNavigatedTaskTabsSnapshot() {
  const area = navigatedTaskTabStorageArea();
  if (!area?.get) return navigatedTabsSnapshotFromMemory();
  try {
    const stored = await area.get(NAVIGATED_TASK_TABS_STORAGE_KEY);
    return normalizeNavigatedTaskTabsSnapshot(stored?.[NAVIGATED_TASK_TABS_STORAGE_KEY]);
  } catch {
    return navigatedTabsSnapshotFromMemory();
  }
}

async function persistNavigatedTaskTabsSnapshot() {
  const area = navigatedTaskTabStorageArea();
  if (!area?.set) return;
  try {
    await area.set({ [NAVIGATED_TASK_TABS_STORAGE_KEY]: navigatedTabsSnapshotFromMemory() });
  } catch (error) {
    console.warn('[灵感爆爆爆] persistNavigatedTaskTabsSnapshot failed', error);
  }
}

function localStorageArea() {
  return globalThis.chrome?.storage?.local || null;
}

async function readLocalStorageValue(key) {
  const area = localStorageArea();
  if (!area?.get) return null;
  try {
    const stored = await area.get(key);
    return stored?.[key] || null;
  } catch {
    return null;
  }
}

async function writeLocalStorageValue(key, value) {
  const area = localStorageArea();
  if (!area?.set) return;
  await area.set({ [key]: value || null });
}

async function clearLocalStorageValue(key) {
  const area = localStorageArea();
  if (!area) return;
  if (area.remove) {
    await area.remove(key);
    return;
  }
  if (area.set) {
    await area.set({ [key]: null });
  }
}

async function readLocalStorageTimestamp(key) {
  const value = await readLocalStorageValue(key);
  const timestamp = Number(value);
  return Number.isFinite(timestamp) && timestamp > 0 ? timestamp : 0;
}

async function writeLocalStorageTimestamp(key, timestamp = 0) {
  const normalized = Number(timestamp);
  if (!Number.isFinite(normalized) || normalized <= 0) {
    await clearLocalStorageValue(key);
    return;
  }
  await writeLocalStorageValue(key, normalized);
}

async function hydrateNextTimestamp(key, currentValue = 0) {
  const stored = await readLocalStorageTimestamp(key);
  return Math.max(Number(currentValue) || 0, stored);
}

async function readTaskAuthorizationBackoff() {
  return readLocalStorageValue(WORKBENCH_TASK_AUTH_BACKOFF_STORAGE_KEY);
}

async function writeTaskAuthorizationBackoff(snapshot = {}) {
  return writeLocalStorageValue(WORKBENCH_TASK_AUTH_BACKOFF_STORAGE_KEY, snapshot);
}

async function clearTaskAuthorizationBackoff() {
  return clearLocalStorageValue(WORKBENCH_TASK_AUTH_BACKOFF_STORAGE_KEY);
}

async function readActiveTaskExecutionContext(taskId = '') {
  const snapshot = await readLocalStorageValue(ACTIVE_TASK_CONTEXT_STORAGE_KEY);
  if (!snapshot || typeof snapshot !== 'object' || Array.isArray(snapshot)) return null;
  const normalizedTaskId = String(taskId || '').trim();
  const snapshotTaskId = String(snapshot.taskId || '').trim();
  return !normalizedTaskId || snapshotTaskId === normalizedTaskId ? snapshot : null;
}

async function writeActiveTaskExecutionContext(snapshot = {}) {
  const normalized = snapshot && typeof snapshot === 'object' && !Array.isArray(snapshot)
    ? { ...snapshot, updatedAtMs: Date.now() }
    : null;
  return writeLocalStorageValue(ACTIVE_TASK_CONTEXT_STORAGE_KEY, normalized);
}

async function clearActiveTaskExecutionContext() {
  return clearLocalStorageValue(ACTIVE_TASK_CONTEXT_STORAGE_KEY);
}

async function readHeartbeatAuthorizationBackoff() {
  return readLocalStorageValue(WORKBENCH_STATION_HEARTBEAT_BACKOFF_STORAGE_KEY);
}

async function writeHeartbeatAuthorizationBackoff(retryAtMs = 0) {
  return writeLocalStorageValue(WORKBENCH_STATION_HEARTBEAT_BACKOFF_STORAGE_KEY, {
    retryAtMs,
    reason: {
      code: 'authorization_invalid',
      message: '执行工位授权失效，退避后重试。',
    },
  });
}

async function clearHeartbeatAuthorizationBackoff() {
  return clearLocalStorageValue(WORKBENCH_STATION_HEARTBEAT_BACKOFF_STORAGE_KEY);
}

async function writeNavigatedTaskTabsSnapshot(snapshot = {}) {
  const normalized = normalizeNavigatedTaskTabsSnapshot(snapshot);
  navigatedTabs.clear();
  for (const [taskId, tabId] of Object.entries(normalized)) {
    navigatedTabs.set(taskId, tabId);
  }

  const area = navigatedTaskTabStorageArea();
  if (!area?.set) return;
  await area.set({
    [NAVIGATED_TASK_TABS_STORAGE_KEY]: normalized,
  }).catch(() => {});
}

async function rememberNavigatedTaskExecutionTab(taskId = '', tabId = 0) {
  const snapshot = {
    ...(await readNavigatedTaskTabsSnapshot()),
    ...navigatedTabsSnapshotFromMemory(),
  };
  await writeNavigatedTaskTabsSnapshot(
    rememberNavigatedTaskTab(snapshot, taskId, tabId),
  );
}

async function closeRememberedTaskExecutionTabs(taskIds = [], explicitTabIds = []) {
  const snapshot = {
    ...(await readNavigatedTaskTabsSnapshot()),
    ...navigatedTabsSnapshotFromMemory(),
  };
  const tabIdsToClose = new Set(
    explicitTabIds
      .map((tabId) => Number(tabId || 0))
      .filter((tabId) => Number.isFinite(tabId) && tabId > 0),
  );
  for (const taskId of taskIds) {
    const tabId = Number(snapshot[String(taskId || '').trim()] || 0);
    if (!tabId) continue;
    tabIdsToClose.add(tabId);
  }
  for (const tabId of tabIdsToClose) {
    await closeTab(tabId);
  }
  await writeNavigatedTaskTabsSnapshot(
    removeNavigatedTaskTabs(snapshot, taskIds),
  );
}

function isUrlLike(value = '') {
  return /^https?:\/\//i.test(String(value || '').trim());
}

function isXhsDetailUrl(url = '') {
  const identity = parseTargetIdentity(url);
  return identity.platform === 'xhs' && Boolean(identity.contentId);
}

function isDouyinDetailUrl(url = '') {
  const identity = parseTargetIdentity(url);
  return identity.platform === 'douyin' && Boolean(identity.contentId);
}

function normalizeString(value = '') {
  return String(value || '').trim();
}

function extractXhsDetailIdFromUrl(url = '') {
  const identity = parseTargetIdentity(url);
  return identity.platform === 'xhs' ? identity.contentId : '';
}

function extractDouyinDetailIdFromUrl(url = '') {
  const identity = parseTargetIdentity(url);
  return identity.platform === 'douyin' ? identity.contentId : '';
}

function extractDetailContentIdFromUrl(url = '') {
  return extractXhsDetailIdFromUrl(url) || extractDouyinDetailIdFromUrl(url);
}

function buildCanonicalXhsDetailUrl(noteId = '', sourceUrl = '') {
  const normalizedNoteId = normalizeString(noteId).replace(/^xhs_/, '');
  if (!normalizedNoteId) return '';
  let token = '';
  try {
    token = new URL(normalizeString(sourceUrl)).searchParams.get('xsec_token') || '';
  } catch {
    token = '';
  }
  const url = new URL(`https://www.xiaohongshu.com/discovery/item/${encodeURIComponent(normalizedNoteId)}`);
  url.searchParams.set('source', 'webshare');
  url.searchParams.set('xhsshare', 'pc_web');
  if (token) url.searchParams.set('xsec_token', token);
  url.searchParams.set('xsec_source', 'pc_share');
  return url.toString();
}

function isRecoverableConnectionError(error) {
  const message = String(error?.message || error || '');
  return /Could not establish connection|Receiving end does not exist|context invalidated|The message port closed|sendToTab timeout|Cannot access contents|Extension manifest must request permission/i.test(message);
}

function buildContentScriptUnavailableCapabilityResponse({ task = {}, error = null } = {}) {
  const target = task.target || {};
  const platform = normalizeString(task.platform);
  const pageType = normalizeString(target.pageType);
  const url = normalizeString(target.url || task.target);
  const technicalMessage = normalizeString(error?.message || error || 'content_script_unavailable');

  return {
    success: true,
    accepted: false,
    error: REMOTE_ERROR_CODE.PAGE_CONTEXT_UNAVAILABLE,
    reasonCode: REMOTE_ERROR_CODE.PAGE_CONTEXT_UNAVAILABLE,
    reasonMessage: '当前页面没有加载插件内容脚本，请刷新页面，或确认正在使用已加载灵感爆爆爆插件的抖音/小红书页面。',
    recommendedAction: 'reload_supported_page_with_plugin',
    report: {
      type: WORKBENCH_MESSAGE_TYPE.CAPABILITY_REPORT,
      protocolVersion: WORKBENCH_PROTOCOL_VERSION,
      contextVersion: WORKBENCH_PROTOCOL_VERSION,
      supportedProtocolVersions: [WORKBENCH_PROTOCOL_VERSION],
      platform,
      mode: pageType,
      pageType,
      url,
      isStableSearchList: false,
      isDyVideoPage: false,
      isDyStrictDetailPage: false,
      capabilities: {
        canRunTaskTypes: [],
      },
      readiness: {
        ready: false,
        reasonCode: REMOTE_ERROR_CODE.PAGE_CONTEXT_UNAVAILABLE,
        reasonMessage: '当前页面没有加载插件内容脚本',
      },
      recommendedNextAction: 'reload_supported_page_with_plugin',
      contextSnapshot: {
        contentScriptLoaded: false,
        technicalMessage,
      },
    },
  };
}

function isSignedXhsShareUrl(url = '') {
  const value = normalizeString(url);
  if (!value) return false;
  if (/^https?:\/\/(?:[^/]+\.)?xhslink\.com\//i.test(value)) return true;
  try {
    const parsed = new URL(value);
    const path = parsed.pathname.toLowerCase();
    if (path.includes('/discovery/item/')) return true;
    const source = String(parsed.searchParams.get('xsec_source') || '').toLowerCase();
    const hasToken = Boolean(parsed.searchParams.get('xsec_token'));
    return /^\/user\/profile\/[^/]+\/[^/]+/i.test(parsed.pathname) && hasToken && source === 'pc_user';
  } catch {
    return false;
  }
}

function getTaskPlatformContentId(task = {}) {
  const payload = task?.payload && typeof task.payload === 'object' ? task.payload : {};
  return normalizeString(
    payload.platformContentId
      || payload.noteId
      || payload.contentId
      || extractDetailContentIdFromUrl(task.target),
  ).replace(/^(xhs_|dy_)/, '');
}

async function resolvePreferredTaskTarget(task = {}) {
  const platform = normalizeString(task.platform).toLowerCase();
  const target = normalizeString(task.target);
  const pageType = inferPageTypeFromTask(task);
  if (platform !== 'xhs' || pageType !== 'detail') return target;
  if (isSignedXhsShareUrl(target)) return target;
  const noteId = getTaskPlatformContentId(task);
  if (!noteId) return target;

  const existing = await noteStore.getById(noteId).catch(() => null);
  const candidates = [
    existing?.rawUrl,
    existing?.url,
    existing?.canonicalUrl,
    existing?.noteUrl,
    target,
  ];
  return candidates.find((candidate) => isSignedXhsShareUrl(candidate)) || buildCanonicalXhsDetailUrl(noteId, target) || target;
}

function inferPageTypeFromTask(task = {}) {
  const taskType = String(task.taskType || '').trim();
  const targetUrl = String(task.target || '').trim();
  const payload = task.payload && typeof task.payload === 'object' ? task.payload : {};
  const collectionProfile = String(task.collectionProfile || payload.collectionProfile || '').trim();
  const declaredTargetPageType = String(payload.targetPageType || '').trim();
  if (declaredTargetPageType === 'detail' || declaredTargetPageType === 'profile' || declaredTargetPageType === 'search') {
    return declaredTargetPageType;
  }
  if (collectionProfile === 'author_links' || collectionProfile === 'author_profile') {
    return 'profile';
  }
  if (collectionProfile === 'note_full' || collectionProfile === 'note_detail' || collectionProfile === 'comment_probe') {
    return 'detail';
  }
  if (collectionProfile === 'list_scan') {
    if (/\/user\//i.test(targetUrl) || /\/user\/profile\//i.test(targetUrl)) return 'profile';
    return 'search';
  }
  if (taskType === 'douyin.singleComments' || taskType === 'douyin.commentImageDownload') {
    return 'detail';
  }
  if (
    taskType === REMOTE_TASK_TYPE.XHS_COLLECT_AUTHOR ||
    taskType === REMOTE_TASK_TYPE.XHS_AUTHOR_PROFILE ||
    taskType === REMOTE_TASK_TYPE.XHS_AUTHOR_NOTE_LINKS ||
    taskType === REMOTE_TASK_TYPE.XHS_AUTHOR_LINKS ||
    taskType === 'douyin.collectAuthor'
  ) {
    return 'profile';
  }
  if (
    taskType === REMOTE_TASK_TYPE.XHS_BATCH_COMMENTS ||
    taskType === REMOTE_TASK_TYPE.XHS_COMMENT_SCAN
  ) {
    return 'detail';
  }
  if (
    taskType === REMOTE_TASK_TYPE.XHS_BATCH_NOTES ||
    taskType === REMOTE_TASK_TYPE.XHS_LIST_SCAN ||
    taskType === REMOTE_TASK_TYPE.XHS_NOTE_FULL ||
    taskType === 'douyin.batchNotes'
  ) {
    if (isXhsDetailUrl(targetUrl) || isDouyinDetailUrl(targetUrl)) {
      return 'detail';
    }
    if (/\/user\//i.test(targetUrl) || /\/user\/profile\//i.test(targetUrl)) {
      return 'profile';
    }
  }
  if (/\/user\//i.test(targetUrl) || /\/user\/profile\//i.test(targetUrl)) {
    return 'profile';
  }
  return 'search';
}

function resolveRiskControlAccountId(activeTask = {}) {
  return String(activeTask?.accountId || '').trim();
}

function buildRiskControlActiveTaskPatch({ accountId = '', accountName = '', errorMessage = '' } = {}) {
  const normalizedAccountId = String(accountId || '').trim();
  const normalizedAccountName = String(accountName || '').trim();
  const message = String(
    errorMessage
    || (normalizedAccountId && normalizedAccountName
      ? `风控(300017)，已切换到账号"${normalizedAccountName}"，请恢复任务继续`
      : ''),
  ).trim();
  return {
    workbenchStatus: 'paused',
    accountId: normalizedAccountId,
    pendingAccountUsageId: normalizedAccountId,
    errorMessage: message,
  };
}

function normalizeWorkbenchTaskTarget(task = {}) {
  const targetUrl = String(task.target || '').trim();
  return {
    pageType: inferPageTypeFromTask(task),
    url: targetUrl,
  };
}

function buildBatchNotesDispatchMessage(msg = {}) {
  const { tabId: _tabId, ...forwarded } = msg || {};
  const externalTaskType = normalizeString(msg?.externalTaskMeta?.externalTaskType);
  const monitorMeta = msg?.monitorMeta || msg?.externalTaskMeta?.monitorMeta || null;
  const targetNoteId = normalizeString(msg?.targetNoteId || monitorMeta?.targetNoteId).replace(/^xhs_/, '');
  const isXhsRemoteDetail = (
    externalTaskType === REMOTE_TASK_TYPE.XHS_BATCH_NOTES ||
    externalTaskType === REMOTE_TASK_TYPE.XHS_NOTE_FULL
  ) && String(msg?.mode || '').trim() === 'detail';

  if (isXhsRemoteDetail) {
    return {
      ...forwarded,
      action: MSG.START_BATCH_NOTES,
      monitorMeta,
      targetNoteId: targetNoteId || undefined,
    };
  }

  return {
    ...forwarded,
    action: MSG.START_BATCH_NOTES,
    monitorMeta,
    targetNoteId: targetNoteId || undefined,
  };
}

function buildBatchCommentsDispatchMessage(msg = {}) {
  const { tabId: _tabId, ...forwarded } = msg || {};
  return {
    ...forwarded,
    action: MSG.START_BATCH_COMMENTS,
  };
}

function buildTaskEnvelopeFromCollectionTask(task = {}) {
  const payload = task.payload || {};
  const taskStrategy = String(task.taskStrategy || payload.taskStrategy || '').trim();
  return {
    type: 'task.envelope',
    protocolVersion: 'v1',
    taskId: String(task.id || '').trim(),
    taskType: String(task.taskType || '').trim(),
    platform: String(task.platform || '').trim(),
    taskStrategy,
    triggerSource: 'collection_task_poller',
    target: normalizeWorkbenchTaskTarget(task),
    payload: {
      ...payload,
      taskStrategy: payload.taskStrategy || taskStrategy || undefined,
    },
  };
}

export {
  buildBatchCommentsDispatchMessage,
  buildBatchNotesDispatchMessage,
  inferPageTypeFromTask,
  normalizeWorkbenchTaskTarget,
  resolveRiskControlAccountId,
  buildRiskControlActiveTaskPatch,
  buildContentScriptUnavailableCapabilityResponse,
  resolvePreferredTaskTarget,
  scoreTaskTabCandidate,
  selectReachableTaskTab,
};

function normalizeWorkbenchTaskRegistryId(value = '') {
  return String(value || '').trim();
}

function getWorkbenchTaskContext(taskId = '') {
  const normalizedTaskId = normalizeWorkbenchTaskRegistryId(taskId);
  if (!normalizedTaskId) return null;

  const directMatch = workbenchTaskRegistry.get(normalizedTaskId);
  if (directMatch) return directMatch;

  for (const entry of workbenchTaskRegistry.values()) {
    if (!entry) continue;
    if (normalizeWorkbenchTaskRegistryId(entry.taskId) === normalizedTaskId) return entry;
    if (normalizeWorkbenchTaskRegistryId(entry.externalTaskId) === normalizedTaskId) return entry;
  }

  return null;
}

function setWorkbenchTaskContext(taskId = '', patch = {}) {
  const normalizedTaskId = normalizeWorkbenchTaskRegistryId(taskId);
  const normalizedExternalTaskId = normalizeWorkbenchTaskRegistryId(patch.externalTaskId || normalizedTaskId);
  const nextKey = normalizedExternalTaskId || normalizedTaskId;
  if (!nextKey) return null;

  const current = getWorkbenchTaskContext(nextKey) || getWorkbenchTaskContext(normalizedTaskId) || {};
  const currentKey = normalizeWorkbenchTaskRegistryId(current.externalTaskId || current.taskId);
  const next = {
    ...current,
    ...patch,
    taskId: normalizeWorkbenchTaskRegistryId(patch.taskId || current.taskId || normalizedTaskId || nextKey),
    externalTaskId: normalizeWorkbenchTaskRegistryId(
      patch.externalTaskId
      || current.externalTaskId
      || current.taskId
      || normalizedTaskId
      || nextKey,
    ),
    updatedAt: Number.isFinite(Number(patch.updatedAt)) ? Number(patch.updatedAt) : Date.now(),
  };

  if (currentKey && currentKey !== nextKey) {
    workbenchTaskRegistry.delete(currentKey);
  }
  workbenchTaskRegistry.set(nextKey, next);
  return next;
}

function clearWorkbenchTaskContext(taskId = '') {
  const normalizedTaskId = normalizeWorkbenchTaskRegistryId(taskId);
  if (!normalizedTaskId) return;
  if (workbenchTaskRegistry.delete(normalizedTaskId)) return;

  for (const [key, entry] of workbenchTaskRegistry.entries()) {
    if (normalizeWorkbenchTaskRegistryId(entry?.taskId) === normalizedTaskId) {
      workbenchTaskRegistry.delete(key);
      return;
    }
    if (normalizeWorkbenchTaskRegistryId(entry?.externalTaskId) === normalizedTaskId) {
      workbenchTaskRegistry.delete(key);
      return;
    }
  }
}

function shouldKeepNavigatedTaskTabForCapabilityFailure(result = {}) {
  const reasonCode = String(result?.reasonCode || result?.error || '').trim();
  return new Set([
    REMOTE_ERROR_CODE.LOGIN_REQUIRED,
    REMOTE_ERROR_CODE.LOGIN_EXPIRED,
    REMOTE_ERROR_CODE.PLATFORM_SECURITY_CHALLENGE,
  ]).has(reasonCode);
}

function getWorkbenchTaskTabId(taskId = '') {
  return getWorkbenchTaskContext(taskId)?.tabId || null;
}

function getPlatformTabQuery(task = {}) {
  return String(task.platform || '').trim() === 'douyin'
    ? ['https://www.douyin.com/*']
    : ['https://xiaohongshu.com/*', 'https://*.xiaohongshu.com/*', 'https://www.xiaohongshu.com/*'];
}

function scoreTaskTabCandidate(tab = {}, targetUrl = '') {
  const tabUrl = normalizeString(tab?.url);
  if (!tabUrl) return -1;
  if (targetUrl && tabUrl === targetUrl) return 100;
  if (targetUrl) {
    const targetContentId = extractDetailContentIdFromUrl(targetUrl);
    const tabContentId = extractDetailContentIdFromUrl(tabUrl);
    if (targetContentId && tabContentId && targetContentId === tabContentId) return 95;
  }
  if (targetUrl && tabUrl.startsWith(targetUrl)) return 90;
  if (targetUrl && targetUrl.startsWith(tabUrl)) return 80;
  if (tab.active) return 20;
  return 0;
}

function isForegroundActiveTaskTab(tab = {}, focusedWindowId = null) {
  const normalizedFocusedWindowId = Number.isFinite(Number(focusedWindowId))
    ? Number(focusedWindowId)
    : null;
  if (!normalizedFocusedWindowId) return false;
  return Boolean(tab?.active) && Number(tab?.windowId || 0) === normalizedFocusedWindowId;
}

async function selectReachableTaskTab(
  candidates = [],
  targetUrl = '',
  capabilityCheck = async () => ({ accepted: false }),
  options = {},
) {
  const avoidActiveInWindowId = Number.isFinite(Number(options?.avoidActiveInWindowId))
    ? Number(options.avoidActiveInWindowId)
    : null;
  const strictlyAvoidActiveInWindow = Boolean(options?.strictlyAvoidActiveInWindow && avoidActiveInWindowId);
  const rankedCandidates = [...(Array.isArray(candidates) ? candidates : [])]
    .filter((tab) => tab?.id)
    .sort((a, b) => scoreTaskTabCandidate(b, targetUrl) - scoreTaskTabCandidate(a, targetUrl));

  const preferredCandidates = avoidActiveInWindowId
    ? rankedCandidates.filter((tab) => !isForegroundActiveTaskTab(tab, avoidActiveInWindowId))
    : rankedCandidates;
  const fallbackCandidates = avoidActiveInWindowId
    ? rankedCandidates.filter((tab) => isForegroundActiveTaskTab(tab, avoidActiveInWindowId))
    : [];

  const candidateGroups = strictlyAvoidActiveInWindow
    ? [preferredCandidates]
    : [preferredCandidates, fallbackCandidates];

  for (const group of candidateGroups) {
    for (const tab of group) {
      try {
        const capability = await capabilityCheck(tab);
        if (capability?.accepted) {
          return tab;
        }
      } catch (error) {
        if (isRecoverableConnectionError(error)) {
          continue;
        }
        throw error;
      }
    }
  }

  return null;
}

async function keepTaskExecutionTabAlive(tabId = 0) {
  const normalizedTabId = Number(tabId || 0);
  if (!normalizedTabId) return;
  await chrome.tabs.update(normalizedTabId, { autoDiscardable: false }).catch(() => {});
}

async function resolveTaskExecutionTabId(task = {}) {
  const taskId = String(task.id || '').trim();
  const cachedTabId = getWorkbenchTaskTabId(taskId);
  if (cachedTabId) {
    await keepTaskExecutionTabAlive(cachedTabId);
    return cachedTabId;
  }

  const targetUrl = String(task.target || '').trim();
  if (!isUrlLike(targetUrl)) {
    return null;
  }

  const tabs = await chrome.tabs.query({ url: getPlatformTabQuery(task) });
  const mappedCapabilityTask = mapTaskEnvelopeToCapabilityCheck(buildTaskEnvelopeFromCollectionTask(task));
  let focusedWindowId = null;
  try {
    const focusedWindow = await chrome.windows?.getLastFocused?.();
    focusedWindowId = Number(focusedWindow?.id || 0) || null;
  } catch {
    focusedWindowId = null;
  }
  const tab = await selectReachableTaskTab(tabs, targetUrl, async (candidate) => (
    bgHandlers[MSG.WORKBENCH_CAPABILITY_CHECK]({
      tabId: candidate.id,
      task: mappedCapabilityTask,
    }, {})
  ), {
    avoidActiveInWindowId: focusedWindowId,
    strictlyAvoidActiveInWindow: true,
  });
  if (tab?.id) {
    await keepTaskExecutionTabAlive(tab.id);
    setWorkbenchTaskContext(taskId, {
      taskId,
      externalTaskId: taskId,
      tabId: tab.id,
      taskType: String(task.taskType || '').trim(),
      platform: String(task.platform || '').trim(),
    });
    return tab.id;
  }
  return null;
}

// ========== 消息路由 ==========

const SENSITIVE_ACTIONS = createSensitiveActionSet(MSG);

chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
  const handler = bgHandlers[message.action];
  if (!handler) return false;

  const authorization = authorizeBackgroundMessage({
    action: message.action,
    sender,
    runtimeId: chrome.runtime.id,
    sensitiveActions: SENSITIVE_ACTIONS,
  });
  if (!authorization.allowed) {
    sendResponse(normalizeWorkbenchMessageResponse(message.action, {
      success: false,
      error: authorization.error,
    }));
    return false;
  }

  Promise.resolve(handler(message, sender)).then((result) => {
    sendResponse(normalizeWorkbenchMessageResponse(message.action, result));
  }).catch(err => {
    sendResponse(normalizeWorkbenchMessageResponse(message.action, {
      success: false,
      error: err.message,
    }));
  });
  return true;
});

function sendToTab(tabId, payload, options = {}) {
  return sendSharedToTab(tabId, payload, {
    autoReconnect: true,
    timeoutMs: 10000,
    ...options,
  });
}

function getLatestWorkbenchRegistryEntry() {
  let latestEntry = null;
  for (const entry of workbenchTaskRegistry.values()) {
    if (!entry?.tabId) continue;
    if (!latestEntry || Number(entry.updatedAt || 0) > Number(latestEntry.updatedAt || 0)) {
      latestEntry = entry;
    }
  }
  return latestEntry;
}

function getTrackedWorkbenchControlTabId() {
  const activeTask = taskPoller?.getState?.().activeTask;
  const externalTaskId = String(activeTask?.externalTaskId || '').trim();
  if (externalTaskId) {
    const registryEntry = getWorkbenchTaskContext(externalTaskId);
    if (registryEntry?.tabId) return registryEntry.tabId;
  }
  return getLatestWorkbenchRegistryEntry()?.tabId || null;
}

function getActiveWorkbenchTaskForMessage(msg = {}, sender = {}) {
  const activeTask = taskPoller?.getState?.().activeTask || null;
  const externalTaskId = String(msg.externalTaskId || msg.taskId || '').trim();
  if (externalTaskId && activeTask?.externalTaskId === externalTaskId) return activeTask;
  if (externalTaskId && activeTask?.taskId === externalTaskId) return activeTask;
  const senderTabId = sender?.tab?.id;
  if (senderTabId) {
    for (const [registeredExternalTaskId, entry] of workbenchTaskRegistry.entries()) {
      if (entry?.tabId === senderTabId && activeTask?.externalTaskId === registeredExternalTaskId) {
        return activeTask;
      }
    }
  }
  return activeTask;
}

function getActivePluginRunId(activeTask = {}, msg = {}) {
  return String(msg.collectionRunId || activeTask?.pluginRunId || activeTask?.externalTaskId || activeTask?.taskId || '').trim();
}

function normalizeWorkbenchRecordType(value = '') {
  const normalized = String(value || '').trim();
  return Object.values(WORKBENCH_RECORD_TYPE).includes(normalized)
    ? normalized
    : WORKBENCH_RECORD_TYPE.NOTE;
}

function normalizeObjectRecord(value = {}) {
  return value && typeof value === 'object' && !Array.isArray(value) ? value : {};
}

const FINAL_RESULT_PACKAGE_ONLY_PROFILES = new Set([
  'author_links',
  'author_profile',
  'comment_probe',
  'list_scan',
  'note_detail',
  'note_full',
]);

function collectionProfileForActiveTask(activeTask = {}, message = {}) {
  return resolveCollectionProfile({
    collectionProfile:
      activeTask.collectionProfile
      || activeTask.payload?.collectionProfile
      || message.collectionProfile
      || message.externalTaskMeta?.collectionProfile
      || '',
    taskType:
      activeTask.taskType
      || activeTask.payload?.taskType
      || message.taskType
      || message.externalTaskMeta?.externalTaskType
      || '',
    payload: activeTask.payload,
  });
}

function shouldWaitForFinalResultPackage(activeTask = {}, message = {}) {
  return FINAL_RESULT_PACKAGE_ONLY_PROFILES.has(collectionProfileForActiveTask(activeTask, message));
}

function deriveExternalRecordId(recordType = WORKBENCH_RECORD_TYPE.NOTE, record = {}) {
  if (recordType === WORKBENCH_RECORD_TYPE.COMMENT) {
    return String(record.commentId || record.id || '').trim();
  }
  if (recordType === WORKBENCH_RECORD_TYPE.AUTHOR) {
    return String(record.authorId || record.userId || record.profileUrl || record.id || '').trim();
  }
  if (recordType === WORKBENCH_RECORD_TYPE.MEDIA) {
    return String(record.assetId || record.sourceUrl || record.localPath || record.id || '').trim();
  }
  return String(record.noteId || record.platformContentId || record.contentId || record.url || record.id || '').trim();
}

function incrementStreamedRecordCount(counts = {}, recordType = '') {
  const next = counts && typeof counts === 'object' && !Array.isArray(counts)
    ? { ...counts }
    : {};
  const key = String(recordType || '').trim();
  if (!key) return next;
  next[key] = Math.max(0, Number(next[key] || 0) || 0) + 1;
  return next;
}

async function enqueueWorkbenchProgressEvent(message = {}, sender = {}) {
  const activeTask = getActiveWorkbenchTaskForMessage(message, sender);
  if (!activeTask) return;
  const progressEvent = normalizeProgressEvent(message.progressEvent || message);
  const pluginRunId = getActivePluginRunId(activeTask, message);
  if (!pluginRunId) return;
  const executionContext = getCurrentTaskExecutionContext(activeTask.taskId);
  await taskDeltaReporter.enqueueEvent({
    taskId: activeTask.taskId,
    pluginRunId,
    eventType: WORKBENCH_TASK_EVENT_TYPE.TASK_PROGRESS,
    source: WORKBENCH_EVENT_SOURCE.CONTENT,
    sequence: progressEvent.heartbeatAt || Date.now(),
    payload: attachTaskRuntimeObservability({
      task: activeTask,
      payload: progressEvent,
      eventType: WORKBENCH_TASK_EVENT_TYPE.TASK_PROGRESS,
      now: progressEvent.heartbeatAt || Date.now(),
    }),
    snapshot: {
      status: progressEvent.status,
      progress: progressEvent.total > 0
        ? Math.min(95, Math.round((progressEvent.current / progressEvent.total) * 100))
        : undefined,
      latestHeartbeatAt: new Date(progressEvent.heartbeatAt || Date.now()).toISOString(),
    },
    executionContext,
  });
}

async function sendTaskControlToTab(preferredTabId, action) {
  const candidates = [];
  const normalizedPreferredTabId = Number.isFinite(Number(preferredTabId)) ? Number(preferredTabId) : null;
  if (normalizedPreferredTabId) {
    candidates.push(normalizedPreferredTabId);
  }
  const trackedTabId = getTrackedWorkbenchControlTabId();
  if (trackedTabId && !candidates.includes(trackedTabId)) {
    candidates.push(trackedTabId);
  }

  if (!candidates.length) {
    return { success: false, error: 'No tabId' };
  }

  let lastRecoverableError = '';
  for (const tabId of candidates) {
    const response = await sendToTab(tabId, { action }, { allowContextError: true });
    if (!response?.skipped) {
      return { success: true, tabId, response };
    }
    lastRecoverableError = String(response?.error || '').trim();
  }

  return {
    success: false,
    error: lastRecoverableError || 'no_control_target',
  };
}

async function clearBadge(tabId) {
  if (!tabId) return;
  await chrome.action.setBadgeText({ text: '', tabId }).catch(() => {});
}

async function syncStatusForActiveTask(activeTask = {}, patch = {}) {
  const config = await getAuthorizedFlywheelConfig();
  const identity = await executionStationClient.getStoredStationIdentity();
  const authorization = await pluginAuthorizationClient.getStoredAuthorization();
  if (!shouldPollWorkbenchTasks(config) || !identity?.stationId || !identity?.stationToken) {
    return { success: true, skipped: true, reason: 'station_not_registered' };
  }
  const runtimeSnapshot = await collectExecutionStationRuntimeSnapshot(identity);
  return syncCollectionTaskStatusThroughSync({
    serverUrl: config.serverUrl,
    taskId: activeTask.taskId,
    patch,
    stationId: identity.stationId,
    stationToken: identity.stationToken,
    authorizationToken: String(config?.apiToken || authorization.authorizationToken || '').trim(),
    capabilities: runtimeSnapshot.capabilities,
    platformAccounts: runtimeSnapshot.platformAccounts,
    pluginVersion: getPluginVersion(),
    store: taskLeaseStore,
  });
}

async function handleRiskControl300017(activeTask) {
  const accountId = resolveRiskControlAccountId(activeTask);

  if (accountId) {
    await accountStore.markCooldown(accountId, 2 * 60 * 60 * 1000);
  }

  const nextAccount = await selectAvailableAccount('xhs');
  if (!nextAccount) {
    const errorMessage = '所有账号均已触发风控限制，请等待冷却或添加新账号';
    await syncStatusForActiveTask(activeTask, {
      status: 'paused',
      errorMessage,
    });
    taskPoller?.updateActiveTask?.((current) => (
      current?.taskId === activeTask?.taskId
        ? {
            workbenchStatus: 'paused',
            pendingAccountUsageId: '',
            errorMessage,
          }
        : null
    ));
    return;
  }

  const result = await injectCookiesForAccount(nextAccount.cookieJson);
  if (!result.success) {
    const errorMessage = '切换账号 Cookie 注入失败';
    await syncStatusForActiveTask(activeTask, {
      status: 'paused',
      errorMessage,
    });
    taskPoller?.updateActiveTask?.((current) => (
      current?.taskId === activeTask?.taskId
        ? {
            workbenchStatus: 'paused',
            pendingAccountUsageId: '',
            errorMessage,
          }
        : null
    ));
    return;
  }

  const activeTaskPatch = buildRiskControlActiveTaskPatch({
    accountId: nextAccount.accountId,
    accountName: nextAccount.name,
  });
  await syncStatusForActiveTask(activeTask, {
    status: 'paused',
    errorMessage: activeTaskPatch.errorMessage,
  });
  taskPoller?.updateActiveTask?.((current) => (
    current?.taskId === activeTask?.taskId
      ? activeTaskPatch
      : null
  ));
}

const bgHandlers = {
  // 屏蔽媒体资源（批量采集加速）
  [MSG.BLOCK_MEDIA]: async (msg, sender) => {
    const tabId = sender.tab?.id;
    if (!tabId) return { error: 'No tabId' };
    await chrome.declarativeNetRequest.updateDynamicRules({
      addRules: [{
        id: 1,
        priority: 1,
        action: { type: 'block' },
        condition: {
          resourceTypes: ['image', 'media'],
          tabIds: [tabId],
        },
      }],
      removeRuleIds: [1],
    });
    return { success: true };
  },

  // 恢复媒体资源
  [MSG.UNBLOCK_MEDIA]: async () => {
    await chrome.declarativeNetRequest.updateDynamicRules({
      removeRuleIds: [1],
    });
    return { success: true };
  },

  // 通过 scripting 注入脚本模拟 Esc 键（关闭笔记弹窗）
  [MSG.DISPATCH_ESC]: async (msg, sender) => {
    const tabId = sender.tab?.id || msg.tabId;
    if (!tabId) return { error: 'No tabId' };

    try {
      await chrome.scripting.executeScript({
        target: { tabId },
        func: () => {
          window.dispatchEvent(new KeyboardEvent('keydown', {
            key: 'Escape', code: 'Escape', keyCode: 27, bubbles: true,
          }));
          window.dispatchEvent(new KeyboardEvent('keyup', {
            key: 'Escape', code: 'Escape', keyCode: 27, bubbles: true,
          }));
        },
      });
    } catch (error) {
      return { error: String(error?.message || error) };
    }
    return { success: true };
  },

  // 统一媒体下载能力：支持候选 URL 顺序重试 + 文件夹路径

  [MSG.FETCH_BINARY_AS_DATA_URL]: async (msg = {}) => {
    const candidates = dedupeCandidates(msg.candidates || msg.url || []);
    if (candidates.length === 0) {
      return { success: false, error: 'No candidate url' };
    }
    return fetchBinaryAsDataUrl(candidates);
  },

  [MSG.DOWNLOAD_MEDIA_FILE]: async (msg = {}, sender = {}) => {
    let candidates = dedupeCandidates(msg.candidates || msg.url || []);
    if (candidates.length === 0) {
      return { success: false, error: 'No candidate url' };
    }
    // 抖音 CDN 对多个候选会同时产生多条失败记录，且视频类通常第一个候选就是最佳质量
    const isDouyin = candidates.some((url) => /douyin/i.test(String(url || '')));
    if (isDouyin && candidates.length > 1) {
      candidates = candidates.slice(0, 1);
    }
    const filename = sanitizeDownloadFilename(msg.filename);
    const errors = [];

    for (let i = 0; i < candidates.length; i++) {
      const candidate = candidates[i];
      try {
        const headers = buildDownloadHeaders(msg.headers || []);
        const result = await tryDownloadCandidate(candidate, filename, {
          saveAs: Boolean(msg.saveAs),
          conflictAction: msg.conflictAction || 'uniquify',
          timeoutMs: Number(msg.timeoutMs || 180000),
          headers,
          waitForCompletion: msg.waitForCompletion !== false,
        });
        if (result.success) {
          return {
            success: true,
            sourceUrl: candidate,
            candidateIndex: i,
            quality: i === 0 ? 'HD' : 'SD',
            downloadId: result.downloadId,
            filename,
            queued: result.reason === 'queued',
          };
        }
        errors.push(`${candidate} -> ${result.reason}`);
      } catch (err) {
        errors.push(`${candidate} -> ${String(err?.message || err)}`);
      }
    }

    return {
      success: false,
      error: 'all_candidates_failed',
      errors,
      filename,
    };
  },

  // 批量笔记采集（转发到 content script）
  [MSG.START_BATCH_NOTES]: async (msg, sender) => {
    const authorizationStatus = await getPluginAuthorizationSnapshot();
    if (!authorizationStatus.authorized) {
      throw new Error(authorizationStatus.authorizationMessage);
    }
    const tabId = msg.tabId || sender.tab?.id;
    if (!tabId) return { error: 'No tabId' };
    chrome.action.setBadgeText({ text: '⏳', tabId });
    chrome.action.setBadgeBackgroundColor({ color: '#3498db', tabId });
    let manualLock = null;
    try {
      manualLock = await prepareManualExecutionLockForDispatch({
        action: MSG.START_BATCH_NOTES,
        msg,
        tabId,
        sender,
      });
      const response = await sendToTab(tabId, buildBatchNotesDispatchMessage(manualLock.message), {
        timeoutMs: msg?.asyncDispatch ? 12000 : 10000,
      });
      if (response?.success === false || response?.accepted === false || response?.error) {
        await manualExecutionLockCoordinator.release(manualLock.lock);
      } else if (manualLock.lock?.accountId) {
        await accountStore.updateUsage(manualLock.lock.accountId);
      }
      return response;
    } catch (err) {
      await manualExecutionLockCoordinator.release(manualLock?.lock);
      await clearBadge(tabId);
      throw err;
    }
  },

  [MSG.STOP_BATCH_NOTES]: async (msg) => {
    const control = await sendTaskControlToTab(msg.tabId, MSG.STOP_BATCH_NOTES);
    if (!control?.success) {
      return { error: control?.error || 'No tabId' };
    }
    await clearBadge(control.tabId);
    return { success: true };
  },

  [MSG.PAUSE_BATCH_NOTES]: async (msg) => {
    const control = await sendTaskControlToTab(msg.tabId, MSG.PAUSE_BATCH_NOTES);
    if (!control?.success) {
      return { error: control?.error || 'No tabId' };
    }
    return { success: true };
  },

  [MSG.RESUME_BATCH_NOTES]: async (msg) => {
    const control = await sendTaskControlToTab(msg.tabId, MSG.RESUME_BATCH_NOTES);
    if (!control?.success) {
      return { error: control?.error || 'No tabId' };
    }
    return { success: true };
  },

  // 批量评论采集（转发到 content script）
  [MSG.START_BATCH_COMMENTS]: async (msg, sender) => {
    const authorizationStatus = await getPluginAuthorizationSnapshot();
    if (!authorizationStatus.authorized) {
      throw new Error(authorizationStatus.authorizationMessage);
    }
    const tabId = msg.tabId || sender.tab?.id;
    if (!tabId) return { error: 'No tabId' };
    chrome.action.setBadgeText({ text: '评', tabId });
    chrome.action.setBadgeBackgroundColor({ color: '#e74c3c', tabId });
    let manualLock = null;
    try {
      manualLock = await prepareManualExecutionLockForDispatch({
        action: MSG.START_BATCH_COMMENTS,
        msg,
        tabId,
        sender,
      });
      const response = await sendToTab(tabId, buildBatchCommentsDispatchMessage(manualLock.message), {
        timeoutMs: msg?.asyncDispatch ? 12000 : 10000,
      });
      if (response?.success === false || response?.accepted === false || response?.error) {
        await manualExecutionLockCoordinator.release(manualLock.lock);
      } else if (manualLock.lock?.accountId) {
        await accountStore.updateUsage(manualLock.lock.accountId);
      }
      return response;
    } catch (err) {
      await manualExecutionLockCoordinator.release(manualLock?.lock);
      await clearBadge(tabId);
      throw err;
    }
  },

  [MSG.STOP_BATCH_COMMENTS]: async (msg) => {
    const control = await sendTaskControlToTab(msg.tabId, MSG.STOP_BATCH_COMMENTS);
    if (!control?.success) {
      return { error: control?.error || 'No tabId' };
    }
    await clearBadge(control.tabId);
    return { success: true };
  },

  [MSG.PAUSE_BATCH_COMMENTS]: async (msg) => {
    const control = await sendTaskControlToTab(msg.tabId, MSG.PAUSE_BATCH_COMMENTS);
    if (!control?.success) {
      return { error: control?.error || 'No tabId' };
    }
    return { success: true };
  },

  [MSG.RESUME_BATCH_COMMENTS]: async (msg) => {
    const control = await sendTaskControlToTab(msg.tabId, MSG.RESUME_BATCH_COMMENTS);
    if (!control?.success) {
      return { error: control?.error || 'No tabId' };
    }
    return { success: true };
  },

  [MSG.RELEASE_EXECUTION_ACCOUNT_LOCK]: async (msg = {}) => {
    const lock = msg.executionLock && typeof msg.executionLock === 'object'
      ? msg.executionLock
      : msg;
    return manualExecutionLockCoordinator.release(lock);
  },

  [MSG.TOGGLE_DASHBOARD]: async (msg) => {
    const control = await sendTaskControlToTab(msg.tabId, MSG.TOGGLE_DASHBOARD);
    if (!control?.success) {
      return { error: control?.error || 'No tabId' };
    }
    return { success: true };
  },

  // ========== 飞轮工作台同步 ==========

  [MSG.SYNC_TO_WORKBENCH]: async (msg) => {
    const { notes = [], comments = [], authors = [] } = msg;
    const totalItems = notes.length + comments.length + authors.length;
    if (totalItems === 0) {
      return { success: false, error: 'No items to sync' };
    }

    const authorizationStatus = await getPluginAuthorizationSnapshot();
    if (!authorizationStatus.authorized) {
      return {
        success: false,
        error: authorizationStatus.authorizationMessage,
        errorCode: 'plugin_authorization_required',
      };
    }

    const authorization = authorizationStatus.authorization || {};
    const config = mergeFlywheelAuthorization(await getFlywheelConfig(), authorization);
    const authorizationToken = String(config?.apiToken || '').trim();
    try {
      return await syncManualRecordsToWorkbench({
        ...config,
        apiToken: authorizationToken,
      }, { notes, comments, authors });
    } catch (error) {
      return {
        success: false,
        error: String(error?.message || error || '请先登录使用者账号，再同步插件采集数据。'),
        errorCode: 'plugin_data_workspace_required',
      };
    }
  },

  [MSG.TEST_FLYWHEEL_CONNECTION]: async (msg) => {
    return testConnection(msg.serverUrl);
  },

  [MSG.GET_ACCOUNTS]: async () => {
    const accounts = await accountStore.getAll();
    return { success: true, accounts };
  },

  [MSG.ADD_ACCOUNT]: async (msg) => {
    const { name, cookieJson, platform, dailyQuotaLimit } = msg;
    try {
      const account = await accountStore.create({ name, cookieJson, platform, dailyQuotaLimit });
      return { success: true, account };
    } catch (error) {
      return { success: false, error: String(error?.message || error) };
    }
  },

  [MSG.REMOVE_ACCOUNT]: async (msg) => {
    await accountStore.remove(msg.accountId);
    return { success: true };
  },

  [MSG.UPDATE_ACCOUNT]: async (msg) => {
    const { accountId, ...patch } = msg;
    await accountStore.update(accountId, patch);
    return { success: true };
  },

  [MSG.GET_FLYWHEEL_CONFIG]: async () => {
    return getFlywheelConfig();
  },

  [MSG.SAVE_FLYWHEEL_CONFIG]: async (msg) => {
    await saveFlywheelConfig(msg.config);
    return { success: true };
  },

  [MSG.GET_EXECUTION_STATION_STATUS]: async () => {
    const authorizationStatus = await getPluginAuthorizationSnapshot();
    const identity = await executionStationClient.getStoredStationIdentity();
    const [runtimeSnapshot, lockSnapshot, unsentOutboxCount, outboxStatusCounts] = await Promise.all([
      collectExecutionStationRuntimeSnapshot(identity),
      executionAccountLockManager.snapshot().catch(() => ({ locks: {} })),
      workbenchOutboxStore.countUnsent().catch(() => 0),
      workbenchOutboxStore.countsByStatus().catch(() => null),
    ]);
    const activeLocks = Object.values(lockSnapshot?.locks || {})
      .map((lock) => summarizeExecutionLockForDiagnostics(lock))
      .filter((lock) => lock.platform || lock.accountId || lock.taskId);
    const currentTask = summarizeActiveTaskForDiagnostics(taskPoller?.getState?.()?.activeTask || null);
    return {
      success: true,
      authorized: authorizationStatus.authorized,
      authorization: authorizationStatus.authorization,
      authorizationMessage: authorizationStatus.authorizationMessage,
      registered: Boolean(identity?.stationId && identity?.stationToken),
      pluginVersion: getPluginVersion(),
      identity: summarizeStationIdentityForDiagnostics(identity),
      capabilities: runtimeSnapshot.capabilities,
      platformAccounts: runtimeSnapshot.platformAccounts,
      diagnostics: {
        currentTask,
        activeLocks,
        activeLockCount: activeLocks.length,
        unsentOutboxCount: Number(unsentOutboxCount || 0),
        outboxStatusCounts: outboxStatusCounts || null,
      },
    };
  },

  [MSG.EXPORT_OUTBOX_RECOVERY]: async () => {
    // plugin_local_recovery 导出（报告 §9.3）：只读打包账本 + 死信，不改任何行状态。
    const identity = await executionStationClient.getStoredStationIdentity().catch(() => null);
    const [journalEntries, deadLetters] = await Promise.all([
      captureJournalStore.listAll({ limit: 5000 }),
      workbenchOutboxStore.listDeadLetters({ limit: 5000 }),
    ]);
    return {
      success: true,
      export: buildOutboxRecoveryExport({
        journalEntries,
        deadLetters,
        pluginVersion: getPluginVersion(),
        stationId: identity?.stationId || '',
      }),
    };
  },

  [MSG.AUTHORIZE_PLUGIN_ACCESS]: async (msg = {}) => {
    const authorizationCode = String(msg.authorizationCode || '').trim();
    const serverUrl = String(msg.serverUrl || '').trim();
    if (serverUrl) {
      await saveFlywheelConfig({ serverUrl, enabled: true });
    }
    if (!authorizationCode) {
      return { success: false, error: 'authorization_code_required' };
    }
    try {
      const previousAuthorization = await pluginAuthorizationClient.getStoredAuthorization();
      const stationKey = await executionStationClient.ensureStationKey();
      const currentIdentity = await executionStationClient.getStoredStationIdentity();
      const runtimeSnapshot = await collectExecutionStationRuntimeSnapshot(currentIdentity);
      const authorization = await pluginAuthorizationClient.authorizeWithCode({
        authorizationCode,
        stationKey,
        pluginVersion: getPluginVersion(),
        browserLabel: String(msg.browserLabel || '').trim(),
        capabilities: runtimeSnapshot.capabilities,
      });
      await saveFlywheelConfig({
        enabled: true,
        apiToken: String(authorization.authorizationToken || '').trim(),
        dataToken: '',
        dataTokenExpiresAt: '',
        dataWorkspaceId: '',
        dataUserEmail: '',
        dataUserName: '',
      });
      if (
        previousAuthorization?.authorizationId
        && authorization?.authorizationId
        && previousAuthorization.authorizationId !== authorization.authorizationId
      ) {
        await taskLeaseStore.clear();
      }
      await saveConnectedWorkbenchStation({
        station: authorization.station,
        stationKey,
        capabilities: runtimeSnapshot.capabilities,
      });
      const identity = await executionStationClient.getStoredStationIdentity();
      const heartbeat = await sendExecutionStationHeartbeat('online');
      await maybeRunWorkbenchTaskPollAfterHeartbeat(heartbeat);
      void registerWorkbenchPushSubscriptionTick();
      const { platformAccounts } = await collectExecutionStationRuntimeSnapshot(identity);
      return {
        success: true,
        authorized: true,
        authorization,
        identity,
        heartbeat,
        platformAccounts,
      };
    } catch (error) {
      return {
        success: false,
        error: String(error?.message || error || 'authorize_plugin_access_failed'),
      };
    }
  },

  [MSG.REQUEST_PLUGIN_AUTHORIZATION]: async (msg = {}) => {
    const serverUrl = String(msg.serverUrl || '').trim();
    if (serverUrl) {
      await saveFlywheelConfig({ serverUrl, enabled: true });
    }
    if (!serverUrl) {
      return { success: false, error: 'workbench_url_required' };
    }
    try {
      const authorization = await pluginAuthorizationClient.requestWorkbenchApproval({
        pluginVersion: getPluginVersion(),
        browserLabel: String(msg.browserLabel || '').trim(),
      });
      return {
        success: true,
        authorized: false,
        authorization,
        message: authorization.authorizationMessage || '授权申请已发送，请等待内容工作台审批。',
      };
    } catch (error) {
      return {
        success: false,
        error: String(error?.message || error || 'request_plugin_authorization_failed'),
      };
    }
  },

  [MSG.CLAIM_PLUGIN_AUTHORIZATION_REQUEST]: async (msg = {}) => {
    const serverUrl = String(msg.serverUrl || '').trim();
    if (serverUrl) {
      await saveFlywheelConfig({ serverUrl, enabled: true });
    }
    if (!serverUrl) {
      return { success: false, error: 'workbench_url_required' };
    }
    try {
      const previousAuthorization = await pluginAuthorizationClient.getStoredAuthorization();
      const stationKey = await executionStationClient.ensureStationKey();
      const currentIdentity = await executionStationClient.getStoredStationIdentity();
      const runtimeSnapshot = await collectExecutionStationRuntimeSnapshot(currentIdentity);
      const result = await pluginAuthorizationClient.claimApprovedRequest({
        stationKey,
        pluginVersion: getPluginVersion(),
        browserLabel: String(msg.browserLabel || '').trim(),
        capabilities: runtimeSnapshot.capabilities,
      });
      if (!result?.claimed) {
        return {
          success: true,
          authorized: false,
          authorization: result?.authorization || null,
          message: result?.authorization?.authorizationMessage || '申请还在等待工作台审批。',
        };
      }
      await saveFlywheelConfig({
        enabled: true,
        apiToken: String(result.authorization?.authorizationToken || '').trim(),
        dataToken: '',
        dataTokenExpiresAt: '',
        dataWorkspaceId: '',
        dataUserEmail: '',
        dataUserName: '',
      });
      if (
        previousAuthorization?.authorizationId
        && result.authorization?.authorizationId
        && previousAuthorization.authorizationId !== result.authorization.authorizationId
      ) {
        await taskLeaseStore.clear();
      }
      await saveConnectedWorkbenchStation({
        station: result.station,
        stationKey,
        capabilities: runtimeSnapshot.capabilities,
      });
      const identity = await executionStationClient.getStoredStationIdentity();
      const heartbeat = await sendExecutionStationHeartbeat('online');
      await maybeRunWorkbenchTaskPollAfterHeartbeat(heartbeat);
      void registerWorkbenchPushSubscriptionTick();
      const { platformAccounts } = await collectExecutionStationRuntimeSnapshot(identity);
      return {
        success: true,
        authorized: true,
        authorization: result.authorization,
        identity,
        heartbeat,
        platformAccounts,
      };
    } catch (error) {
      return {
        success: false,
        error: String(error?.message || error || 'claim_plugin_authorization_request_failed'),
      };
    }
  },

  [MSG.CLEAR_PLUGIN_AUTHORIZATION]: async () => {
    await pluginAuthorizationClient.clearAuthorization();
    await saveFlywheelConfig({
      apiToken: '',
      dataToken: '',
      dataTokenExpiresAt: '',
      dataWorkspaceId: '',
      dataUserEmail: '',
      dataUserName: '',
    });
    await executionStationClient.clearStationIdentity();
    await taskLeaseStore.clear();
    return { success: true };
  },

  [MSG.REGISTER_EXECUTION_STATION]: async (msg = {}) => {
    const authorizationStatus = await getPluginAuthorizationSnapshot();
    const pairingCode = String(msg.pairingCode || '').trim();
    const serverUrl = String(msg.serverUrl || '').trim();
    if (serverUrl) {
      await saveFlywheelConfig({ serverUrl, enabled: true });
    }
    if (!authorizationStatus.authorized) {
      return {
        success: false,
        error: authorizationStatus.authorizationMessage,
        errorCode: 'plugin_authorization_required',
      };
    }
    if (!pairingCode) {
      return { success: false, error: 'pairing_code_required' };
    }
    try {
      const identity = await executionStationClient.registerWithPairingCode({
        pairingCode,
        capabilities: stationCapabilitiesForRole(),
        pluginVersion: getPluginVersion(),
        browserLabel: String(msg.browserLabel || '').trim(),
      });
      const heartbeat = await sendExecutionStationHeartbeat('online');
      await maybeRunWorkbenchTaskPollAfterHeartbeat(heartbeat);
      void registerWorkbenchPushSubscriptionTick();
      const { platformAccounts } = await collectExecutionStationRuntimeSnapshot(identity);
      return {
        success: true,
        identity,
        heartbeat,
        platformAccounts,
      };
    } catch (error) {
      return {
        success: false,
        error: String(error?.message || error || 'register_execution_station_failed'),
      };
    }
  },

  [MSG.SEND_EXECUTION_STATION_HEARTBEAT]: async () => {
    const heartbeat = await sendExecutionStationHeartbeat('online');
    await maybeRunWorkbenchTaskPollAfterHeartbeat(heartbeat);
    return heartbeat;
  },

  // ========== Cookie 管理 ==========

  [MSG.GET_PLATFORM_COOKIES]: async (msg = {}) => {
    const platformConfig = {
      xhs: {
        domains: ['.xiaohongshu.com', 'xiaohongshu.com', 'www.xiaohongshu.com'],
        tabQueries: ['*://xiaohongshu.com/*', '*://*.xiaohongshu.com/*'],
        origin: 'https://www.xiaohongshu.com',
      },
      douyin: {
        domains: ['.douyin.com', 'www.douyin.com'],
        tabQueries: ['*://*.douyin.com/*'],
        origin: 'https://www.douyin.com',
      },
    };
    const requestedPlatform = String(msg.platform || '').trim();
    const activeConfigs = requestedPlatform && platformConfig[requestedPlatform]
      ? { [requestedPlatform]: platformConfig[requestedPlatform] }
      : platformConfig;

    const collectUnique = (batch, seen, target) => {
      for (const c of batch) {
        const key = `${c.domain}|${c.name}`;
        if (!seen.has(key)) { seen.add(key); target.push(c); }
      }
    };

    const results = {};
    for (const [platform, config] of Object.entries(activeConfigs)) {
      const seen = new Set();
      const allCookies = [];

      for (const domain of config.domains) {
        try {
          const batch = await chrome.cookies.getAll({ domain });
          collectUnique(batch, seen, allCookies);
        } catch {}
      }

      if (allCookies.length === 0) {
        try {
          const tabs = await chrome.tabs.query({ url: config.tabQueries });
          const activeTab = tabs.find(t => t.active) || tabs[0];
          if (activeTab?.id) {
            const response = await sendToTab(activeTab.id, {
              action: 'getDocumentCookie',
            }, { allowContextError: true, timeoutMs: 3000 });
            if (response?.cookieString) {
              const pairs = response.cookieString.split(';').map(s => s.trim()).filter(Boolean);
              for (const pair of pairs) {
                const eqIdx = pair.indexOf('=');
                if (eqIdx < 0) continue;
                const name = pair.substring(0, eqIdx);
                const value = pair.substring(eqIdx + 1);
                const key = `${config.origin}|${name}`;
                if (!seen.has(key)) {
                  seen.add(key);
                  allCookies.push({ name, value, domain: config.domains[0], path: '/', secure: true, httpOnly: false, sameSite: 'lax' });
                }
              }
            }
          }
        } catch {}
      }

      const cookieString = allCookies
        .filter(c => c.name && c.value)
        .map(c => `${c.name}=${c.value}`)
        .join('; ');
      results[platform] = {
        cookies: allCookies.map(c => ({
          name: c.name, value: c.value, domain: c.domain, path: c.path,
          secure: c.secure, httpOnly: c.httpOnly || false, sameSite: c.sameSite || 'lax',
          ...(c.expirationDate ? { expirationDate: c.expirationDate } : {}),
        })),
        cookieString,
        count: allCookies.length,
        capturedAt: new Date().toISOString(),
      };
    }
    const stored = await chrome.storage.local.get('platformCookies');
    const mergedResults = {
      ...(stored.platformCookies || {}),
      ...results,
    };
    await chrome.storage.local.set({ platformCookies: mergedResults });
    const success = Object.values(results).some((result) => Number(result?.count || 0) > 0);
    return { success, results: mergedResults };
  },

  [MSG.GET_STORED_PLATFORM_COOKIES]: async () => {
    const data = await chrome.storage.local.get('platformCookies');
    return { success: true, results: data.platformCookies || null };
  },

  [MSG.PROGRESS]: async (msg = {}, sender = {}) => {
    try {
      const progressEvent = normalizeProgressEvent(msg.progressEvent || msg);
      if (progressEvent.riskControlCode === '300017') {
        const activeTask = getActiveWorkbenchTaskForMessage(msg, sender);
        if (activeTask) {
          await handleRiskControl300017(activeTask);
        }
      }
      await enqueueWorkbenchProgressEvent(msg, sender);
    } catch (error) {
      console.warn('[灵感爆爆爆] 工作台进度增量写入失败:', error);
    }
    return { success: true };
  },

  [MSG.WORKBENCH_DELTA_FLUSH]: async () => {
    return taskDeltaReporter.flush();
  },

  [MSG.WORKBENCH_RECORD_DELTA]: async (msg = {}, sender = {}) => {
    const activeTask = getActiveWorkbenchTaskForMessage(msg, sender);
    if (!activeTask) {
      return { success: false, skipped: true, error: 'no_active_workbench_task' };
    }
    if (shouldWaitForFinalResultPackage(activeTask, msg)) {
      return {
        success: true,
        skipped: true,
        reason: 'final_result_package_required',
      };
    }

    const executionContext = getCurrentTaskExecutionContext(activeTask.taskId);
    const incomingRecords = Array.isArray(msg.records) && msg.records.length > 0
      ? msg.records
      : [msg];
    const records = incomingRecords
      .map((item = {}) => {
        const recordType = normalizeWorkbenchRecordType(item.recordType);
        const record = normalizeObjectRecord(item.record || item.payload);
        if (Object.keys(record).length === 0) return null;
        return {
          taskId: activeTask.taskId,
          pluginRunId: getActivePluginRunId(activeTask, item),
          recordType,
          externalRecordId: String(item.externalRecordId || deriveExternalRecordId(recordType, record)).trim(),
          sequence: Number(item.sequence || Date.now()),
          payload: record,
          collectedAt: String(item.collectedAt || ''),
          executionContext,
        };
      })
      .filter(Boolean);
    if (records.length === 0) {
      return { success: false, skipped: true, error: 'empty_record_payload' };
    }

    await taskDeltaReporter.enqueueRecords(records);
    taskPoller?.updateActiveTask?.((current) => {
      if (!current || current.taskId !== activeTask.taskId) return null;
      const streamedRecordCounts = records.reduce(
        (counts, record) => incrementStreamedRecordCount(counts, record.recordType),
        current.streamedRecordCounts,
      );
      return {
        firstRecordSeen: true,
        streamedRecordCounts,
      };
    });
    return { success: true, count: records.length };
  },

  [MSG.WORKBENCH_LOCAL_CONTROL_EVENT]: async (msg = {}, sender = {}) => {
    const activeTask = getActiveWorkbenchTaskForMessage(msg, sender);
    if (!activeTask) {
      return { success: false, skipped: true, error: 'no_active_workbench_task' };
    }
    const pluginRunId = getActivePluginRunId(activeTask, msg);
    const executionContext = getCurrentTaskExecutionContext(activeTask.taskId);
    const controlAction = String(msg.controlAction || '').trim();
    const status = String(msg.status || '').trim();
    const sequence = Date.now();
    const stateEventType = status === 'paused'
      ? WORKBENCH_TASK_EVENT_TYPE.TASK_PAUSED
      : status === 'running'
        ? WORKBENCH_TASK_EVENT_TYPE.TASK_RESUMED
        : status === 'stopped'
          ? WORKBENCH_TASK_EVENT_TYPE.TASK_STOPPED
          : WORKBENCH_TASK_EVENT_TYPE.TASK_STOPPING;

    await taskDeltaReporter.enqueueEvent({
      taskId: activeTask.taskId,
      pluginRunId,
      eventType: WORKBENCH_TASK_EVENT_TYPE.TASK_CONTROL_APPLIED,
      source: WORKBENCH_EVENT_SOURCE.PLUGIN,
      sequence,
      payload: {
        origin: 'plugin_local',
        controlAction,
        status,
        message: String(msg.message || ''),
        occurredAt: msg.occurredAt || new Date(sequence).toISOString(),
      },
      executionContext,
    });
    await taskDeltaReporter.enqueueEvent({
      taskId: activeTask.taskId,
      pluginRunId,
      eventType: stateEventType,
      source: WORKBENCH_EVENT_SOURCE.PLUGIN,
      sequence: sequence + 1,
      payload: {
        origin: 'plugin_local',
        controlAction,
        status,
        message: String(msg.message || ''),
        occurredAt: msg.occurredAt || new Date(sequence).toISOString(),
      },
      snapshot: {
        status,
        latestHeartbeatAt: new Date(sequence).toISOString(),
      },
      executionContext,
    });
    return { success: true };
  },

  [MSG.WORKBENCH_CAPABILITY_CHECK]: async (msg = {}, sender = {}) => {
    const task = msg.task || {};
    const validation = validateCapabilityCheck(task);
    if (!validation.valid) {
      return {
        success: false,
        accepted: false,
        error: 'invalid_capability_check',
        validationErrors: validation.errors,
      };
    }

    const tabId = msg.tabId || sender.tab?.id;
    if (!tabId) {
      return { success: false, accepted: false, error: 'No tabId' };
    }

    let response;
    try {
      response = await sendToTab(tabId, { action: MSG.GET_PAGE_CONTEXT }, {
        allowContextError: false,
        timeoutMs: 4000,
      });
    } catch (error) {
      if (isRecoverableConnectionError(error)) {
        return buildContentScriptUnavailableCapabilityResponse({ task, error });
      }
      throw error;
    }
    const report = response?.context || null;
    const decision = canDispatchTaskFromCapabilityReport(report, task.taskType, task.target);
    return {
      success: true,
      accepted: decision.accepted,
      report,
      reasonCode: decision.reasonCode,
      reasonMessage: decision.reasonMessage,
    };
  },

  [MSG.WORKBENCH_DISPATCH_TASK]: async (msg = {}, sender = {}) => {
    const task = msg.task || {};
    const validation = validateTaskEnvelope(task);
    if (!validation.valid) {
      return {
        success: false,
        accepted: false,
        error: 'invalid_task_envelope',
        validationErrors: validation.errors,
      };
    }

    const tabId = msg.tabId || sender.tab?.id;
    if (!tabId) {
      return { success: false, accepted: false, error: 'No tabId' };
    }

    const capabilityResponse = await bgHandlers[MSG.WORKBENCH_CAPABILITY_CHECK]({
      task: mapTaskEnvelopeToCapabilityCheck(task),
      tabId,
    }, sender);
    if (!capabilityResponse?.accepted) {
      return capabilityResponse;
    }

    const internalCommand = mapTaskEnvelopeToInternalCommand(task, { tabId });
    let dispatchResponse = null;
    if (internalCommand.dispatchTarget === 'background') {
      dispatchResponse = await bgHandlers[internalCommand.action]({
        ...internalCommand.payload,
        tabId,
      }, sender);
    } else {
      dispatchResponse = await sendToTab(tabId, {
        action: internalCommand.action,
        ...internalCommand.payload,
      }, {
        timeoutMs: internalCommand.payload?.asyncDispatch ? 12000 : 10000,
      });
    }

    if (dispatchResponse?.accepted === false || dispatchResponse?.success === false) {
      return {
        success: false,
        accepted: false,
        error: String(dispatchResponse?.error || 'dispatch_not_accepted'),
        dispatchAction: internalCommand.action,
        dispatchTarget: internalCommand.dispatchTarget,
        capabilityReport: capabilityResponse.report,
      };
    }

    const collectionRunId = normalizeString(
      dispatchResponse?.collectionRunId || dispatchResponse?.resultLookup?.collectionRunId,
    );
    setWorkbenchTaskContext(internalCommand.taskMeta.externalTaskId, {
      taskId: internalCommand.taskMeta.externalTaskId,
      externalTaskId: internalCommand.taskMeta.externalTaskId,
      collectionRunId,
      tabId,
      taskType: internalCommand.taskMeta.externalTaskType,
      platform: String(task.platform || '').trim(),
      updatedAt: Date.now(),
    });

    return {
      success: true,
      accepted: true,
      taskId: internalCommand.taskMeta.externalTaskId,
      tabId,
      taskType: internalCommand.taskMeta.externalTaskType,
      dispatchAction: internalCommand.action,
      dispatchTarget: internalCommand.dispatchTarget,
      capabilityReport: capabilityResponse.report,
      pending: Boolean(dispatchResponse?.pending),
      collectionRunId: collectionRunId || undefined,
      resultLookup: {
        externalTaskId: internalCommand.taskMeta.externalTaskId,
        collectionRunId: collectionRunId || undefined,
      },
    };
  },

  [MSG.WORKBENCH_TASK_CONTROL]: async (msg = {}, sender = {}) => {
    const taskControl = msg.taskControl || {};
    const validation = validateTaskControl(taskControl);
    if (!validation.valid) {
      return {
        success: false,
        accepted: false,
        error: 'invalid_task_control',
        validationErrors: validation.errors,
      };
    }

    const tabId = msg.tabId || sender.tab?.id;
    if (!tabId) {
      return { success: false, accepted: false, error: 'No tabId' };
    }

    const internalCommand = mapTaskControlToInternalCommand(taskControl, { tabId });
    if (internalCommand.dispatchTarget === 'background') {
      await bgHandlers[internalCommand.action]({
        ...internalCommand.payload,
        tabId,
      }, sender);
    } else {
      await sendToTab(tabId, {
        action: internalCommand.action,
        ...internalCommand.payload,
      }, { allowContextError: true });
    }

    return {
      success: true,
      accepted: true,
      taskId: internalCommand.taskMeta.externalTaskId,
      taskType: internalCommand.taskMeta.externalTaskType,
      controlAction: internalCommand.taskMeta.controlAction,
      dispatchAction: internalCommand.action,
    };
  },

  [MSG.WORKBENCH_GET_RESULT_PACKAGE]: async (msg = {}) => {
    const externalTaskId = String(msg.externalTaskId || '').trim();
    const registryCollectionRunId = String(
      msg.collectionRunId
      || getWorkbenchTaskContext(externalTaskId)?.collectionRunId
      || '',
    ).trim();
    const collectionRunId = registryCollectionRunId;
    if (!collectionRunId && !externalTaskId) {
      return { success: false, error: 'collectionRunId or externalTaskId required' };
    }
    const tabId = msg.tabId || getWorkbenchTaskTabId(externalTaskId);
    if (tabId) {
      try {
        const response = await sendToTab(tabId, {
          action: MSG.WORKBENCH_GET_RESULT_PACKAGE,
          collectionRunId,
          externalTaskId,
        }, {
          allowContextError: false,
          timeoutMs: 4000,
        });
        return response;
      } catch (error) {
        const errMsg = String(error?.message || error || 'tab_result_lookup_failed');
        console.warn('[灵感爆爆爆] getResultPackage sendToTab 失败，回退到本地读取:', errMsg);
        if (/Could not establish connection|Receiving end does not exist/i.test(errMsg)) {
          keepTaskExecutionTabAlive(tabId).catch(() => {});
        }
        // fallthrough to local packager
      }
    }
    try {
      const result = collectionRunId
        ? await resultPackager.packageByCollectionRunId(collectionRunId)
        : await resultPackager.packageByExternalTaskId(externalTaskId);
      return {
        success: true,
        result,
      };
    } catch (error) {
      return {
        success: false,
        error: String(error?.message || error || 'result_lookup_failed'),
      };
    }
  },
};

function shouldPollWorkbenchTasks(config = {}) {
  return Boolean(String(config?.serverUrl || '').trim())
    && config?.enabled !== false
    && Boolean(String(config?.apiToken || '').trim());
}

function getPluginVersion() {
  try {
    return chrome.runtime?.getManifest?.()?.version || '';
  } catch {
    return '';
  }
}

const pluginAuthorizationClient = createPluginAuthorizationClient({
  storageArea: chrome.storage?.local,
  resolveServerUrl: async () => {
    const config = await getFlywheelConfig();
    return config?.serverUrl || '';
  },
});

const executionStationClient = createExecutionStationClient({
  storageArea: chrome.storage?.local,
  resolveServerUrl: async () => {
    const config = await getFlywheelConfig();
    return config?.serverUrl || '';
  },
  resolveAuthorization: async () => pluginAuthorizationClient.getStoredAuthorization(),
});
const taskLeaseStore = createTaskLeaseStorageStore({
  storageArea: chrome.storage?.local,
});

function normalizeStationRole(value = '') {
  void value;
  return 'execution';
}

function stationCapabilitiesForRole(_role = 'execution', runtimeStates = []) {
  return stationCapabilitiesForRuntimeStates(runtimeStates, MONITOR_STATION_CAPABILITIES);
}

async function getPluginAuthorizationSnapshot() {
  const authorization = await pluginAuthorizationClient.getStoredAuthorization();
  return {
    authorized: hasActivePluginAuthorization(authorization),
    authorization,
    authorizationMessage: hasActivePluginAuthorization(authorization)
      ? ''
      : getPluginAuthorizationBlockedMessage(authorization),
  };
}

async function getAuthorizedFlywheelConfig() {
  const [config, authorization] = await Promise.all([
    getFlywheelConfig(),
    pluginAuthorizationClient.getStoredAuthorization(),
  ]);
  return mergeFlywheelAuthorization(config, authorization);
}

async function collectStationPlatformAccountsForIdentity(identity = null) {
  const role = normalizeStationRole(identity?.role);
  return collectStationPlatformAccounts(accountStore, { purpose: role });
}

async function collectExecutionStationRuntimeSnapshot(identity = null) {
  const role = normalizeStationRole(identity?.role);
  const runtimeStates = await collectStationRuntimeStates();
  const platformAccounts = await collectStationPlatformAccounts(accountStore, {
    purpose: role,
    runtimeStates,
  });
  return {
    role,
    runtimeStates,
    platformAccounts,
    capabilities: stationCapabilitiesForRole(role, runtimeStates),
  };
}

function summarizeActiveTaskForDiagnostics(activeTask = null) {
  if (!activeTask || typeof activeTask !== 'object') return null;
  return {
    taskId: String(activeTask.taskId || activeTask.id || '').trim(),
    externalTaskId: String(activeTask.externalTaskId || '').trim(),
    taskType: String(activeTask.taskType || activeTask.sourceType || '').trim(),
    platform: String(activeTask.platform || '').trim(),
    accountId: String(activeTask.accountId || activeTask.pendingAccountUsageId || '').trim(),
    workbenchStatus: String(activeTask.workbenchStatus || activeTask.status || '').trim(),
    pluginRunId: String(activeTask.pluginRunId || '').trim(),
    attemptId: String(activeTask.attemptId || '').trim(),
    leaseEpoch: Number.isFinite(Number(activeTask.leaseEpoch)) ? Number(activeTask.leaseEpoch) : null,
    dispatchedAtMs: Number.isFinite(Number(activeTask.dispatchedAtMs)) ? Number(activeTask.dispatchedAtMs) : null,
  };
}

function summarizeExecutionLockForDiagnostics(lock = {}) {
  return {
    key: String(lock.key || '').trim(),
    platform: String(lock.platform || '').trim(),
    accountId: String(lock.accountId || '').trim(),
    taskId: String(lock.taskId || '').trim(),
    attemptId: String(lock.attemptId || '').trim(),
    acquiredAtMs: Number.isFinite(Number(lock.acquiredAtMs)) ? Number(lock.acquiredAtMs) : null,
    expiresAtMs: Number.isFinite(Number(lock.expiresAtMs)) ? Number(lock.expiresAtMs) : null,
  };
}

function summarizeStationIdentityForDiagnostics(identity = null) {
  if (!identity || typeof identity !== 'object') return null;
  return {
    stationId: String(identity.stationId || '').trim(),
    displayName: String(identity.displayName || '').trim(),
    role: String(identity.role || '').trim(),
  };
}

async function saveConnectedWorkbenchStation({ station = null, stationKey = '', capabilities = [] } = {}) {
  const stationId = String(station?.stationId || station?.id || '').trim();
  const stationToken = String(station?.stationToken || station?.token || '').trim();
  if (!stationId || !stationToken) return null;
  return executionStationClient.saveStationIdentity({
    stationKey: String(stationKey || '').trim(),
    stationId,
    stationToken,
    displayName: String(station?.displayName || '').trim(),
    role: String(station?.role || 'execution').trim() || 'execution',
    capabilities: Array.isArray(capabilities) ? capabilities : [],
    pairedAt: Date.now(),
  });
}

async function sendExecutionStationHeartbeat(status = 'online') {
  const config = await getAuthorizedFlywheelConfig();
  if (!shouldPollWorkbenchTasks(config)) {
    return { success: false, retryable: false, reason: 'workbench_not_configured' };
  }

  // V1.1 自动 claim：授权 active 但 identity 缺 stationId/stationToken（通过 request 流程授权，
  // 服务端自动审批+建工位，但插件侧没收到 station 数据）→ 自动 claim 领取 station
  const preIdentity = await executionStationClient.getStoredStationIdentity();
  if (!preIdentity?.stationId || !preIdentity?.stationToken) {
    try {
      const stationKey = await executionStationClient.ensureStationKey();
      const claimed = await pluginAuthorizationClient.claimApprovedRequest({
        stationKey,
        pluginVersion: getPluginVersion(),
        browserLabel: navigator?.userAgent || '',
        capabilities: (await collectExecutionStationRuntimeSnapshot(preIdentity)).capabilities,
      });
      if (claimed?.station) {
        await saveConnectedWorkbenchStation({
          station: claimed.station,
          stationKey,
          capabilities: (await collectExecutionStationRuntimeSnapshot(preIdentity)).capabilities,
        });
        console.log('[heartbeat] auto-claim 成功，已保存 station identity');
      }
    } catch (claimError) {
      // claim 还没审批/已激活但没有 pending request → 静默跳过（不是每次心跳都要 log）
    }
  }

  const identity = await executionStationClient.getStoredStationIdentity();
  if (!identity?.stationId || !identity?.stationToken) {
    return { success: false, retryable: false, reason: 'station_not_registered' };
  }
  const runtimeSnapshot = await collectExecutionStationRuntimeSnapshot(identity);

  // V1.1：把当前活动任务状态喂给 sync 协议，用于构造 activeLeases[] + capacity lane
  const taskPollerState = typeof taskPoller?.getState === 'function' ? taskPoller.getState() : null;
  const activeTask = taskPollerState?.activeTask || null;
  const localLease = taskPollerState?.activeLease || null;
  const activeLane = String(activeTask?.platform || '').trim().toLowerCase();

  return executionStationClient.sendHeartbeat({
    status,
    capabilities: runtimeSnapshot.capabilities,
    pluginVersion: getPluginVersion(),
    platformAccounts: runtimeSnapshot.platformAccounts,
    activeLane,
    localLease,
    activeTask: activeTask
      ? {
          platform: activeTask.platform,
          stage: activeTask.executionPhase || activeTask.stage,
          progress: activeTask.progress,
          lastProgressAtMs: activeTask.lastProgressAtMs,
        }
      : null,
  });
}

let getCurrentTaskExecutionContext = () => null;

const taskDeltaReporter = createTaskDeltaReporter({
  store: workbenchOutboxStore,
  captureJournal: captureJournalStore,
  pluginVersion: getPluginVersion(),
  commitTaskDelta: async (config, taskId, envelope) => {
    const identity = await executionStationClient.getStoredStationIdentity();
    const authorization = await pluginAuthorizationClient.getStoredAuthorization();
    if (!identity?.stationId || !identity?.stationToken) {
      const error = new Error('station_not_registered');
      error.retryable = true;
      throw error;
    }
    const runtimeSnapshot = await collectExecutionStationRuntimeSnapshot(identity);
    return commitCollectionTaskDeltaThroughSync({
      serverUrl: config.serverUrl,
      taskId,
      envelope,
      stationId: identity.stationId,
      stationToken: identity.stationToken,
      authorizationToken: String(config?.apiToken || authorization.authorizationToken || '').trim(),
      capabilities: runtimeSnapshot.capabilities,
      platformAccounts: runtimeSnapshot.platformAccounts,
      pluginVersion: getPluginVersion(),
      store: taskLeaseStore,
    });
  },
  getFlywheelConfig: getAuthorizedFlywheelConfig,
  prepareRecordPayload: async (config, record) => {
    if (String(record?.recordType || '').trim() !== WORKBENCH_RECORD_TYPE.NOTE) {
      return record?.payload || {};
    }
    return enrichNoteWithDataFoundationPayload(record?.payload || {}, {
      taskId: record?.taskId,
      pluginRunId: record?.pluginRunId,
      externalRecordId: record?.externalRecordId,
    });
  },
  shouldPollWorkbenchTasks,
  getExecutorInstanceId: getPersistentExecutorInstanceId,
});

const executionAccountLockManager = createExecutionAccountLockManager({
  store: createExecutionAccountLockStorageStore({
    storageArea: chrome.storage?.session || chrome.storage?.local,
    storageKey: EXECUTION_ACCOUNT_LOCK_STORAGE_KEY,
  }),
});

async function shouldReleaseStaleWorkbenchExecutionLock({ existingTaskId = '' } = {}) {
  const normalizedTaskId = String(existingTaskId || '').trim();
  if (!normalizedTaskId || /^manual:/i.test(normalizedTaskId)) return false;
  const activeTask = taskPoller?.getState?.().activeTask;
  if (
    String(activeTask?.taskId || '').trim() === normalizedTaskId
    || String(activeTask?.externalTaskId || '').trim() === normalizedTaskId
  ) {
    return false;
  }

  try {
    const localLease = await taskLeaseStore.read();
    if (String(localLease?.taskId || '').trim() === normalizedTaskId && String(localLease?.leaseToken || '').trim()) {
      const expiresAtMs = Date.parse(String(localLease?.expiresAt || '').trim());
      if (!Number.isFinite(expiresAtMs) || expiresAtMs > Date.now()) {
        return false;
      }
    }
  } catch {
    return false;
  }

  try {
    const context = await readActiveTaskExecutionContext(normalizedTaskId);
    if (context?.taskId) return false;
  } catch {
    return false;
  }

  return true;
}

const manualExecutionLockCoordinator = createManualExecutionLockCoordinator({
  accountStore,
  lockManager: executionAccountLockManager,
  injectCookiesForAccount,
  shouldReleaseStaleWorkbenchLock: shouldReleaseStaleWorkbenchExecutionLock,
});

async function resolveTabUrlForManualLock(tabId = '', sender = {}) {
  const senderUrl = String(sender?.tab?.url || '').trim();
  if (senderUrl) return senderUrl;
  const normalizedTabId = Number(tabId);
  if (!Number.isFinite(normalizedTabId) || !chrome.tabs?.get) return '';
  try {
    const tab = await chrome.tabs.get(normalizedTabId);
    return String(tab?.url || '').trim();
  } catch {
    return '';
  }
}

async function prepareManualExecutionLockForDispatch({ action = '', msg = {}, tabId = '', sender = {} } = {}) {
  const tabUrl = await resolveTabUrlForManualLock(tabId, sender);
  return manualExecutionLockCoordinator.prepare({
    action,
    msg,
    tabId,
    tabUrl,
  });
}

const taskPoller = createTaskPoller({
  // 出站积压熔断：写回积压超阈值时拒接新任务（OUTBOX_BACKLOG_BLOCKED），
  // 并在熔断 tick 内触发补发自愈。
  getOutboxBacklog: () => workbenchOutboxStore.countsByStatus(),
  flushOutbox: () => taskDeltaReporter.flush(),
  beforeDispatch: async (task) => {
    const platform = String(task.platform || '').trim();
    if (platform !== 'xhs' && platform !== 'douyin') return { shouldPause: false };
    const account = await selectAccountForWorkbenchTask(task, platform);
    if (!account) {
      return { shouldPause: true, reason: 'no_available_account' };
    }
    if (account.runtimeSession) {
      return { shouldPause: false, accountId: account.accountId };
    }
    const result = await injectCookiesForAccount(account.cookieJson, platform);
    if (!result.success) {
      return { shouldPause: true, reason: 'cookie_injection_failed' };
    }
    return { shouldPause: false, accountId: account.accountId };
  },
  afterDispatchSuccess: async (task, preCheck = {}) => {
    const platform = String(task?.platform || '').trim();
    const accountId = String(preCheck?.accountId || '').trim();
    if ((platform !== 'xhs' && platform !== 'douyin') || !accountId) return;
    await accountStore.updateUsage(accountId);
  },
  consumePendingAccountUsage: async (accountId) => {
    const normalizedAccountId = String(accountId || '').trim();
    if (!normalizedAccountId) return;
    await accountStore.updateUsage(normalizedAccountId);
  },
  acquireExecutionLock: async (lock) => executionAccountLockManager.acquire(lock),
  releaseExecutionLock: async (lock) => executionAccountLockManager.release(lock),
  claimTaskLease: async () => {
    const config = await getAuthorizedFlywheelConfig();
    if (!shouldPollWorkbenchTasks(config)) return { task: null, nextPollAfterMs: 0 };
    const identity = await executionStationClient.getStoredStationIdentity();
    const authorization = await pluginAuthorizationClient.getStoredAuthorization();
    if (!identity?.stationId || !identity?.stationToken) {
      return {
        task: null,
        nextPollAfterMs: 30000,
        reason: {
          code: 'station_not_registered',
          message: '请先把插件绑定为执行设备，绑定后才会按任务优先级接单。',
        },
      };
    }
    const runtimeSnapshot = await collectExecutionStationRuntimeSnapshot(identity);
    return claimCollectionTaskLease({
      serverUrl: config.serverUrl,
      stationId: identity.stationId,
      stationToken: identity.stationToken,
      authorizationToken: String(config?.apiToken || authorization.authorizationToken || '').trim(),
      capabilities: runtimeSnapshot.capabilities,
      platformAccounts: runtimeSnapshot.platformAccounts,
      pluginVersion: getPluginVersion(),
      store: taskLeaseStore,
    });
  },
  renewTaskLease: async (taskId, lease = {}, options = {}) => {
    const config = await getAuthorizedFlywheelConfig();
    const identity = await executionStationClient.getStoredStationIdentity();
    const authorization = await pluginAuthorizationClient.getStoredAuthorization();
    if (!shouldPollWorkbenchTasks(config) || !identity?.stationId || !identity?.stationToken) {
      return { success: false, skipped: true, reason: 'station_not_registered' };
    }
    return renewCollectionTaskLease({
      serverUrl: config.serverUrl,
      taskId,
      stationId: identity.stationId,
      stationToken: identity.stationToken,
      leaseToken: lease.leaseToken,
      attemptId: lease.attemptId,
      leaseEpoch: lease.leaseEpoch,
      attemptNumber: lease.attemptNumber,
      authorizationToken: String(config?.apiToken || authorization.authorizationToken || '').trim(),
      status: options?.status || 'running',
      pluginVersion: getPluginVersion(),
      store: taskLeaseStore,
    });
  },
  reconcileTaskLease: async ({ localLease = null } = {}) => {
    const config = await getAuthorizedFlywheelConfig();
    const identity = await executionStationClient.getStoredStationIdentity();
    const authorization = await pluginAuthorizationClient.getStoredAuthorization();
    if (!shouldPollWorkbenchTasks(config) || !identity?.stationId || !identity?.stationToken) {
      return { success: false, skipped: true, reason: 'station_not_registered' };
    }
    const runtimeSnapshot = await collectExecutionStationRuntimeSnapshot(identity);
    return reconcileExecutionStationLease({
      serverUrl: config.serverUrl,
      stationId: identity.stationId,
      stationToken: identity.stationToken,
      authorizationToken: String(config?.apiToken || authorization.authorizationToken || '').trim(),
      localLease,
      capabilities: runtimeSnapshot.capabilities,
      platformAccounts: runtimeSnapshot.platformAccounts,
      pluginVersion: getPluginVersion(),
      store: taskLeaseStore,
    });
  },
  readAuthorizationFailureBackoff: readTaskAuthorizationBackoff,
  writeAuthorizationFailureBackoff: writeTaskAuthorizationBackoff,
  clearAuthorizationFailureBackoff: clearTaskAuthorizationBackoff,
  readActiveTaskContext: readActiveTaskExecutionContext,
  writeActiveTaskContext: writeActiveTaskExecutionContext,
  clearActiveTaskContext: clearActiveTaskExecutionContext,
  readTaskLease: () => taskLeaseStore.read(),
  clearTaskLease: () => taskLeaseStore.clear(),
  patchTask: async (taskId, patch) => {
    const config = await getAuthorizedFlywheelConfig();
    if (!shouldPollWorkbenchTasks(config)) return null;
    const identity = await executionStationClient.getStoredStationIdentity();
    const authorization = await pluginAuthorizationClient.getStoredAuthorization();
    if (!identity?.stationId || !identity?.stationToken) {
      return { success: true, skipped: true, reason: 'station_not_registered' };
    }
    const runtimeSnapshot = await collectExecutionStationRuntimeSnapshot(identity);
    return syncCollectionTaskStatusThroughSync({
      serverUrl: config.serverUrl,
      taskId,
      patch,
      stationId: identity.stationId,
      stationToken: identity.stationToken,
      authorizationToken: String(config?.apiToken || authorization.authorizationToken || '').trim(),
      capabilities: runtimeSnapshot.capabilities,
      platformAccounts: runtimeSnapshot.platformAccounts,
      pluginVersion: getPluginVersion(),
      store: taskLeaseStore,
    });
  },
  fetchControlRequests: async (taskId, options) => {
    void taskId;
    void options;
    return { success: true, controls: [], nextCursor: '' };
  },
  applyTaskControl: async (control = {}) => {
    const taskId = String(control.taskId || '').trim();
    const tabId = getWorkbenchTaskTabId(taskId)
      || getTrackedWorkbenchControlTabId();
    return bgHandlers[MSG.WORKBENCH_TASK_CONTROL]({
      tabId,
      taskControl: {
        type: WORKBENCH_MESSAGE_TYPE.TASK_CONTROL,
        protocolVersion: WORKBENCH_PROTOCOL_VERSION,
        taskId,
        taskType: String(control.taskType || '').trim(),
        action: String(control.action || '').trim(),
      },
    }, {});
  },
  enqueueEvent: async (event) => taskDeltaReporter.enqueueEvent(event),
  enqueueRecords: async (records) => taskDeltaReporter.enqueueRecords(records),
  flushDeltas: async () => taskDeltaReporter.flush(),
  getExecutorInstanceId: getPersistentExecutorInstanceId,
  capabilityCheck: async (task) => {
    const preferredTarget = await resolvePreferredTaskTarget(task);
    const preparedTask = preferredTarget && preferredTarget !== String(task?.target || '').trim()
      ? { ...task, target: preferredTarget }
      : task;

    const tabId = await resolveTaskExecutionTabId(preparedTask);
    if (tabId) {
      return bgHandlers[MSG.WORKBENCH_CAPABILITY_CHECK]({
        tabId,
        task: mapTaskEnvelopeToCapabilityCheck(buildTaskEnvelopeFromCollectionTask(preparedTask)),
      }, {});
    }

    const taskType = String(preparedTask.taskType || '').trim();
    const target = String(preparedTask.target || '').trim();
    const taskId = String(preparedTask.id || '').trim();
    const navResult = await navigateToTask(taskType, target, preparedTask.payload || {});
    if (!navResult.tabId) {
      return { success: false, accepted: false, error: navResult.error || 'no_matching_tab' };
    }

    setWorkbenchTaskContext(taskId, {
      taskId,
      externalTaskId: taskId,
      tabId: navResult.tabId,
      taskType,
      platform: String(task.platform || '').trim(),
    });
    await rememberNavigatedTaskExecutionTab(taskId, navResult.tabId);

    const result = await bgHandlers[MSG.WORKBENCH_CAPABILITY_CHECK]({
      tabId: navResult.tabId,
      task: mapTaskEnvelopeToCapabilityCheck(buildTaskEnvelopeFromCollectionTask(preparedTask)),
    }, {});

    if (!result?.accepted && !shouldKeepNavigatedTaskTabForCapabilityFailure(result)) {
      await closeRememberedTaskExecutionTabs([taskId]);
      clearWorkbenchTaskContext(taskId);
    }

    return {
      ...result,
      pluginOpenedTabId: navResult.tabId,
    };
  },
  dispatchTask: async (task) => {
    const preferredTarget = await resolvePreferredTaskTarget(task);
    const preparedTask = preferredTarget && preferredTarget !== String(task?.target || '').trim()
      ? { ...task, target: preferredTarget }
      : task;
    const tabId = await resolveTaskExecutionTabId(preparedTask);
    if (!tabId) {
      return { success: false, accepted: false, error: 'no_matching_tab' };
    }
    const response = await bgHandlers[MSG.WORKBENCH_DISPATCH_TASK]({
      tabId,
      task: buildTaskEnvelopeFromCollectionTask(preparedTask),
    }, {});
    return {
      ...response,
      tabId: response?.tabId || tabId,
    };
  },
  getResultPackage: async (lookup = {}) => {
    const normalizedLookup = lookup && typeof lookup === 'object'
      ? lookup
      : { externalTaskId: lookup };
    return bgHandlers[MSG.WORKBENCH_GET_RESULT_PACKAGE](normalizedLookup);
  },
});

getCurrentTaskExecutionContext = (taskId = '') => taskPoller?.getExecutionContext?.(taskId) || null;

async function runWorkbenchTaskPollTick({ force = false } = {}) {
  const now = Date.now();
  nextWorkbenchTaskPollAtMs = await hydrateNextTimestamp(
    WORKBENCH_TASK_POLL_NEXT_STORAGE_KEY,
    nextWorkbenchTaskPollAtMs,
  );
  if (!force && nextWorkbenchTaskPollAtMs > now) {
    return { success: true, skipped: true, reason: 'task_poll_waiting' };
  }
  const prevActiveTask = taskPoller?.getState?.()?.activeTask;
  let result = null;
  try {
    result = await taskPoller.tick();
    await taskDeltaReporter.flush();

    if (result?.idle) {
      consecutiveEmptyPolls++;
    } else {
      consecutiveEmptyPolls = 0;
    }

    const alarmConfig = scheduleWorkbenchTaskPollAlarm({
      alarmsApi: chrome.alarms,
      alarmName: WORKBENCH_TASK_POLL_ALARM,
      result,
      consecutiveEmptyPolls,
    });
    nextWorkbenchTaskPollAtMs = Date.now() + alarmConfig.intervalMs;
    await writeLocalStorageTimestamp(WORKBENCH_TASK_POLL_NEXT_STORAGE_KEY, nextWorkbenchTaskPollAtMs);
  } catch (error) {
    console.error('[灵感爆爆爆] workbench task poll tick failed', error);
    nextWorkbenchTaskPollAtMs = Date.now() + MIN_CHROME_ALARM_INTERVAL_MS;
    await writeLocalStorageTimestamp(WORKBENCH_TASK_POLL_NEXT_STORAGE_KEY, nextWorkbenchTaskPollAtMs);
  }

  const currentActiveTask = taskPoller?.getState?.()?.activeTask;
  const cleanupTask = result?.cleanupTask || (prevActiveTask && !currentActiveTask ? prevActiveTask : null);
  if (cleanupTask && !result?.preserveTaskExecutionContext) {
    const { registryIds, navigationIds, tabIds } = taskExecutionCleanupKeys(cleanupTask);
    for (const registryId of registryIds) {
      clearWorkbenchTaskContext(registryId);
    }
    await closeRememberedTaskExecutionTabs(navigationIds, tabIds);
  }
}

async function maybeRunWorkbenchTaskPollAfterHeartbeat(heartbeat = null) {
  if (shouldRunWorkbenchTaskPollAfterHeartbeatResult({
    heartbeat,
    activeTask: taskPoller?.getState?.()?.activeTask,
    nextPollAtMs: nextWorkbenchTaskPollAtMs,
    nowMs: Date.now(),
  })) {
    if (heartbeat?.shouldPollNow) {
      await clearTaskAuthorizationBackoff();
      nextWorkbenchTaskPollAtMs = 0;
      await writeLocalStorageTimestamp(WORKBENCH_TASK_POLL_NEXT_STORAGE_KEY, 0);
    }
    await runWorkbenchTaskPollTick({ force: Boolean(heartbeat?.shouldPollNow) });
    return true;
  }
  return false;
}

async function runExecutionStationHeartbeatTick() {
  const now = Date.now();
  nextExecutionStationHeartbeatAtMs = await hydrateNextTimestamp(
    WORKBENCH_STATION_HEARTBEAT_NEXT_STORAGE_KEY,
    nextExecutionStationHeartbeatAtMs,
  );
  if (nextExecutionStationHeartbeatAtMs > now) {
    return;
  }
  const storedBackoff = await readHeartbeatAuthorizationBackoff();
  const storedRetryAtMs = Number(storedBackoff?.retryAtMs || 0);
  if (Number.isFinite(storedRetryAtMs) && storedRetryAtMs > now) {
    nextExecutionStationHeartbeatAtMs = storedRetryAtMs;
    return;
  }
  if (storedBackoff) {
    await clearHeartbeatAuthorizationBackoff();
  }
  let heartbeat;
  try {
    heartbeat = await sendExecutionStationHeartbeat('online');
  } catch (error) {
    console.warn('[灵感爆爆爆] execution station heartbeat failed', error);
    nextExecutionStationHeartbeatAtMs = Date.now() + WORKBENCH_STATION_HEARTBEAT_INTERVAL_MS;
    await writeLocalStorageTimestamp(WORKBENCH_STATION_HEARTBEAT_NEXT_STORAGE_KEY, nextExecutionStationHeartbeatAtMs);
    return;
  }
  if (!heartbeat?.success) {
    if ([401, 403].includes(Number(heartbeat?.status || 0))) {
      nextExecutionStationHeartbeatAtMs = Date.now() + AUTHORIZATION_FAILURE_IDLE_MS;
      await writeHeartbeatAuthorizationBackoff(nextExecutionStationHeartbeatAtMs);
      await writeLocalStorageTimestamp(WORKBENCH_STATION_HEARTBEAT_NEXT_STORAGE_KEY, nextExecutionStationHeartbeatAtMs);
      return;
    }
    if (heartbeat?.retryable && Number(heartbeat?.nextRetryAt || 0) > Date.now()) {
      nextExecutionStationHeartbeatAtMs = Number(heartbeat.nextRetryAt);
      await writeLocalStorageTimestamp(WORKBENCH_STATION_HEARTBEAT_NEXT_STORAGE_KEY, nextExecutionStationHeartbeatAtMs);
    }
    return;
  }
  nextExecutionStationHeartbeatAtMs = Date.now() + WORKBENCH_STATION_HEARTBEAT_INTERVAL_MS;
  await writeLocalStorageTimestamp(WORKBENCH_STATION_HEARTBEAT_NEXT_STORAGE_KEY, nextExecutionStationHeartbeatAtMs);
  await clearHeartbeatAuthorizationBackoff();
  void registerWorkbenchPushSubscriptionTick();
  await maybeRunWorkbenchTaskPollAfterHeartbeat(heartbeat);
}

async function runPackagedInstallBootstrapTick() {
  try {
    return await applyPackagedInstallBootstrap({
      authorizationClient: pluginAuthorizationClient,
      stationClient: executionStationClient,
      saveFlywheelConfig,
      sendHeartbeat: sendExecutionStationHeartbeat,
      stationCapabilities: stationCapabilitiesForRole(),
      pluginVersion: getPluginVersion(),
      browserLabel: globalThis.navigator?.userAgent || '',
    });
  } catch (error) {
    console.warn('[灵感爆爆爆] packaged install bootstrap skipped', error);
    return { applied: false, reason: 'bootstrap_failed' };
  }
}

async function registerWorkbenchPushSubscriptionTick({ force = false } = {}) {
  const now = Date.now();
  nextWorkbenchPushSubscriptionAtMs = await hydrateNextTimestamp(
    WORKBENCH_PUSH_SUBSCRIPTION_NEXT_STORAGE_KEY,
    nextWorkbenchPushSubscriptionAtMs,
  );
  if (!force && nextWorkbenchPushSubscriptionAtMs > now) {
    return { registered: false, reason: 'push_subscription_waiting' };
  }
  try {
    const config = await getAuthorizedFlywheelConfig();
    if (!shouldPollWorkbenchTasks(config)) {
      nextWorkbenchPushSubscriptionAtMs = Date.now() + WORKBENCH_PUSH_SUBSCRIPTION_RETRY_MS;
      await writeLocalStorageTimestamp(WORKBENCH_PUSH_SUBSCRIPTION_NEXT_STORAGE_KEY, nextWorkbenchPushSubscriptionAtMs);
      return { registered: false, reason: 'workbench_disabled' };
    }
    const result = await registerWorkbenchPushSubscription({
      registration: globalThis.registration || globalThis.self?.registration,
      executionStationClient: {
        fetchVapidPublicKey: () => executionStationClient.fetchVapidPublicKey(),
        registerPushSubscription: ({ subscription }) => executionStationClient.registerPushSubscription({
          subscription,
          pluginVersion: getPluginVersion(),
          browserLabel: globalThis.navigator?.userAgent || '',
        }),
      },
    });
    if (result?.registered) {
      nextWorkbenchPushSubscriptionAtMs = Date.now() + WORKBENCH_PUSH_SUBSCRIPTION_REFRESH_MS;
    } else if (result?.reason === 'station_not_registered') {
      nextWorkbenchPushSubscriptionAtMs = Date.now() + WORKBENCH_PUSH_SUBSCRIPTION_UNREGISTERED_RETRY_MS;
    } else {
      nextWorkbenchPushSubscriptionAtMs = Date.now() + WORKBENCH_PUSH_SUBSCRIPTION_RETRY_MS;
    }
    await writeLocalStorageTimestamp(WORKBENCH_PUSH_SUBSCRIPTION_NEXT_STORAGE_KEY, nextWorkbenchPushSubscriptionAtMs);
    return result;
  } catch (error) {
    console.warn('[灵感爆爆爆] workbench push subscription skipped', error);
    nextWorkbenchPushSubscriptionAtMs = Date.now() + WORKBENCH_PUSH_SUBSCRIPTION_RETRY_MS;
    await writeLocalStorageTimestamp(WORKBENCH_PUSH_SUBSCRIPTION_NEXT_STORAGE_KEY, nextWorkbenchPushSubscriptionAtMs);
    return { registered: false, reason: 'push_subscription_failed' };
  }
}

async function handleWorkbenchPushEvent(event) {
  const payload = parseWorkbenchPushPayload(event?.data || null);
  if (!shouldWakeForWorkbenchPush(payload)) return;
  nextWorkbenchTaskPollAtMs = 0;
  await writeLocalStorageTimestamp(WORKBENCH_TASK_POLL_NEXT_STORAGE_KEY, 0);
  await runWorkbenchTaskPollTick({ force: true });
}

// ========== 采集完成时清除 badge ==========

chrome.runtime.onMessage.addListener((message, sender) => {
  if (message.action === MSG.COLLECT_DONE && sender.tab?.id) {
    chrome.action.setBadgeText({ text: '', tabId: sender.tab.id });
    if (getActiveWorkbenchTaskForMessage(message, sender)) {
      void runWorkbenchTaskPollTick({ force: true }).catch((error) => {
        console.warn('[灵感爆爆爆] 采集完成后同步工作台任务状态失败:', error);
      });
    }
  }
});

if (chrome.alarms?.onAlarm) {
  chrome.alarms.onAlarm.addListener((alarm) => {
    if (alarm?.name === WORKBENCH_TASK_POLL_ALARM) {
      void runWorkbenchTaskPollTick();
      return;
    }
    if (alarm?.name === WORKBENCH_STATION_HEARTBEAT_ALARM) {
      void runExecutionStationHeartbeatTick();
      return;
    }
    if (alarm?.name === 'daily-quota-reset') {
      void accountStore.resetDailyQuota();
      return;
    }
    if (alarm?.name === 'capture-journal-prune') {
      // 只修剪「已被服务端 ACK + 超过 14 天」的采集事实；未确认送达的永不清理。
      void captureJournalStore.pruneAcked({
        isAcked: async (entryId) => (await workbenchOutboxStore.getStatusByKey(entryId)) === 'acked',
      }).catch(() => {});
      return;
    }
  });
}

chrome.runtime.onStartup?.addListener(() => {
  chrome.alarms?.create(WORKBENCH_TASK_POLL_ALARM, { periodInMinutes: INITIAL_WORKBENCH_TASK_POLL_MINUTES });
  chrome.alarms?.create(WORKBENCH_STATION_HEARTBEAT_ALARM, { periodInMinutes: 1 });
  void runPackagedInstallBootstrapTick().finally(() => {
    void registerWorkbenchPushSubscriptionTick();
    void runWorkbenchTaskPollTick();
    void runExecutionStationHeartbeatTick();
  });
});

chrome.runtime.onInstalled?.addListener(() => {
  chrome.alarms?.create(WORKBENCH_TASK_POLL_ALARM, { periodInMinutes: INITIAL_WORKBENCH_TASK_POLL_MINUTES });
  chrome.alarms?.create(WORKBENCH_STATION_HEARTBEAT_ALARM, { periodInMinutes: 1 });
  void runPackagedInstallBootstrapTick().finally(() => {
    void registerWorkbenchPushSubscriptionTick();
    void taskDeltaReporter.flush();
    void runExecutionStationHeartbeatTick();
  });
});

globalThis.self?.addEventListener?.('push', (event) => {
  event.waitUntil(handleWorkbenchPushEvent(event));
});

// V1.1（2026-06-29）：tabs.onRemoved — 用户手动关闭标签页时自动释放关联任务。
// 此前无此监听器，关闭的标签页导致任务永久卡在"执行中"。
chrome.tabs?.onRemoved?.addListener((tabId) => {
  try {
    for (const [taskId, trackedTabId] of navigatedTabs.entries()) {
      if (trackedTabId === tabId) {
        navigatedTabs.delete(taskId);
        void persistNavigatedTaskTabsSnapshot();
        console.log(`[background] tab ${tabId} removed — cleaned up navigated task ${taskId}`);
        break;
      }
    }
    // 也通知 workbenchTaskRegistry 清理（让 taskPoller 感知 tab 已消失）
    for (const [taskId, binding] of workbenchTaskRegistry.entries()) {
      if (binding?.tabId === tabId) {
        workbenchTaskRegistry.delete(taskId);
        break;
      }
    }
  } catch (error) {
    console.warn('[灵感爆爆爆] tabs.onRemoved cleanup failed', error);
  }
});

// V1.1（2026-06-29）：runtime.onSuspend — SW 即将被 Chrome 终止时持久化关键状态。
// 此前 SW 重启丢失内存状态导致任务成为孤儿。
if (typeof chrome?.runtime?.onSuspend?.addListener === 'function') {
  chrome.runtime.onSuspend.addListener(() => {
    try {
      void persistNavigatedTaskTabsSnapshot().catch(() => {});
      void taskPoller.persistActiveTaskContext().catch(() => {});
    } catch (error) {
      console.warn('[灵感爆爆爆] runtime.onSuspend persistence failed', error);
    }
  });
}

chrome.alarms?.create(WORKBENCH_TASK_POLL_ALARM, { periodInMinutes: INITIAL_WORKBENCH_TASK_POLL_MINUTES });
chrome.alarms?.create(WORKBENCH_STATION_HEARTBEAT_ALARM, { periodInMinutes: 1 });

// 每日配额清零（每小时检查一次日期变化）
chrome.alarms?.create('daily-quota-reset', { periodInMinutes: 60 });

// 采集事实账本修剪（每 6 小时；只清已 ACK 且超龄条目）
chrome.alarms?.create('capture-journal-prune', { periodInMinutes: 360 });

void runPackagedInstallBootstrapTick().finally(() => {
  void registerWorkbenchPushSubscriptionTick();
  void runExecutionStationHeartbeatTick();
});

console.log('[灵感爆爆爆] Background Service Worker 已启动');
