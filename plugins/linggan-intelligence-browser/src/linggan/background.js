import {
  LINGGAN_LOCAL_ORIGIN,
  attemptStartIsAccepted,
  activateLingganInstallationCredential,
  checkInLingganStation,
  claimLingganDispatch,
  grantLingganDetailPageSession,
  reportLingganDetailPageSessionNavigation,
  reportLingganDetailPageRiskSignal,
  claimLingganMediaAcquisition,
  claimedTaskAccountDecision,
  createLocalAttempt,
  createLocalSubmission,
  createTaskSpec,
  decodePageExecutionReceipt,
  isRecoverableDeliveryRegistrationRefusal,
  isTerminalLocalDeliveryResult,
  localPost,
  readLingganLocalReadiness,
  reportLingganAccountEligibility,
  reportLingganDispatchFailure,
  taskCreationIsAccepted,
  unavailableLingganStats,
  validateTaskSpec,
} from './adapter.js';
import { LINGGAN_RUNTIME_ACTION } from './runtimeActions.js';
import { localMediaOutbox, localProducerOutbox } from './localProducerOutbox.js';
import { createManualRuntimeTask, packageDiscovery } from './producerRuntime.js';
import {
  detailPageSessionStore,
  detailPageNavigationGrantStore,
  detailPageLanePreparationStore,
  packageDetailPageSessionLane,
} from './detailPageSessionStore.js';
import { waitForStableTab } from './tabReadiness.js';
import { buildSignedXhsDetailExecutionUrl } from './xhsExecutionTarget.js';
import { executeClaimedMediaAcquisition } from './mediaAcquisitionExecution.js';
import {
  allowedMediaCandidateUri,
  mediaUploadUnitsForRecord,
  normalizeMediaCandidateUri,
  recordMediaDownloadFailure,
} from './mediaTransferRuntime.js';
import { requestMediaWorker } from './mediaWorkerChannel.js';
import { dispatchedCommentMaxTotal, dispatchedMaximumQuota } from './localExecutionSupport.js';
import {
  activatePendingInstallationCredential,
  persistPendingInstallationCredential,
  readInstallationCredential,
  readInstallationCredentialRecord,
} from './installationCredentialStore.js';

const PRODUCER_INSTANCE_KEY = 'linggan.localTrusted.producerInstanceId';
const CAPTURE_DELIVERY_REJECTED = 'capture_delivery_rejected';
let flushingOutbox = null;
let creatingMediaWorker = null;

async function producerInstanceId() {
  const stored = await chrome.storage.local.get(PRODUCER_INSTANCE_KEY);
  const current = String(stored[PRODUCER_INSTANCE_KEY] || '').trim();
  if (current) return current;
  const created = crypto.randomUUID();
  await chrome.storage.local.set({ [PRODUCER_INSTANCE_KEY]: created });
  return created;
}

async function installationCredentialFor(installKey) {
  return readInstallationCredential(chrome.storage.local, installKey);
}

export async function flushLocalOutboxOnce({
  outbox = localProducerOutbox,
  mediaOutbox = localMediaOutbox,
  readReadiness = readLingganLocalReadiness,
  post = localPost,
  flushMedia = flushMediaOutbox,
  readCredential = installationCredentialFor,
  reportDispatchFailure = reportLingganDispatchFailure,
} = {}) {
  const readiness = await readReadiness();
  const producerRoutes = readiness?.deliveryReady === true ? readiness.producerRoutes : null;
  const due = await outbox.due({ limit: 5 });
  for (const entry of due) {
    await outbox.markInFlight(entry.submissionId);
    if (!producerRoutes) {
      await outbox.retry(entry.submissionId, 'local_producer_route_contract_not_ready');
      continue;
    }
    try {
      const task = await post(producerRoutes.taskCreation, entry.taskSpec);
      if (!taskCreationIsAccepted(task)) {
        if (isTerminalLocalDeliveryResult(task)) {
          await outbox.terminal(entry.submissionId, task.payload.code || 'task_not_created');
          continue;
        }
        throw new Error(task.payload.code || 'task_not_created');
      }
      const attempt = await post(producerRoutes.attemptStart, entry.attempt);
      if (!attemptStartIsAccepted(attempt)) {
        // 起步登记被「活权不在了」挡回来时，不能就此认定这份包没被收过：上一次投递可能
        // 正是关掉这条租约的那一次，而服务端早已开出 Receipt。这一种必须继续走投递路由，
        // 由服务端按 durable 事实重放原 Receipt（晚到包按 LOST_AUTHORITY 接纳）；身份冲突、
        // 任务不存在等其他 4xx 仍是终态。
        if (!isRecoverableDeliveryRegistrationRefusal(attempt)) {
          if (isTerminalLocalDeliveryResult(attempt)) {
            await outbox.terminal(entry.submissionId, attempt.payload.code || 'attempt_not_started');
            continue;
          }
          throw new Error(attempt.payload.code || 'attempt_not_started');
        }
      }
      const submitted = await post(producerRoutes.submission, {
        contractVersion: entry.contractVersion,
        producerInstanceId: entry.producerInstanceId,
        taskId: entry.taskId,
        attemptId: entry.attemptId,
        submissionId: entry.submissionId,
        capturePackage: entry.capturePackage,
      });
      if (submitted.ok && ['acknowledged', 'replay'].includes(submitted.payload.delivery)) {
        await outbox.acknowledge(entry.submissionId, submitted.payload);
      } else if (submitted.status >= 400 && submitted.status < 500) {
        // Only the capture-contract rejection proves this immutable Package
        // cannot be accepted. Claim/attempt conflicts have their own server
        // meaning and must not be misreported as a failed page delivery.
        const dispatchFailureCode = submitted.payload.code === 'submission_invalid'
          ? CAPTURE_DELIVERY_REJECTED
          : '';
        await outbox.terminal(entry.submissionId, submitted.payload.code, { dispatchFailureCode });
      } else {
        await outbox.retry(entry.submissionId, submitted.payload.code || 'submission_not_acknowledged');
      }
    } catch (error) {
      await outbox.retry(entry.submissionId, error?.message || error);
    }
  }
  await reportTerminalDeliveryFailures({
    outbox, readiness, readCredential, reportDispatchFailure,
  });
  await flushMedia();
  return {
    pending: await outbox.pendingCount(),
    mediaPending: await mediaOutbox.pendingCount(),
  };
}

async function reportTerminalDeliveryFailures({
  outbox, readiness, readCredential, reportDispatchFailure,
} = {}) {
  if (!readiness?.health || typeof outbox?.terminalDispatchFailures !== 'function') return;
  const terminalEntries = await outbox.terminalDispatchFailures({ limit: 5 });
  for (const entry of terminalEntries) {
    const installationCredential = await readCredential(entry.producerInstanceId);
    if (!installationCredential) continue;
    const reported = await reportDispatchFailure({
      installKey: entry.producerInstanceId,
      installationCredential,
      taskId: entry.taskId,
      failureId: entry.dispatchFailureId,
      failureCode: entry.dispatchFailureCode,
      health: readiness.health,
    });
    if (reported?.reported === true) {
      await outbox.markTerminalDispatchFailureReported(entry.submissionId);
    }
  }
}

function flushLocalOutbox() {
  if (flushingOutbox) return flushingOutbox;
  flushingOutbox = flushLocalOutboxOnce().finally(() => { flushingOutbox = null; });
  return flushingOutbox;
}

async function flushMediaOutbox(preferredUploadId = '') {
  await ensureMediaWorker();
  const installKey = await producerInstanceId();
  const installationCredential = await installationCredentialFor(installKey);
  const response = await requestMediaWorker({
    runtime: chrome.runtime,
    preferredUploadId,
    installationCredential,
  });
  if (response?.success !== true) throw new Error(response?.code || 'media_worker_unavailable');
  return response;
}

async function ensureMediaWorker() {
  if (!chrome.offscreen?.createDocument) throw new Error('media_offscreen_unavailable');
  const offscreenUrl = chrome.runtime.getURL('media-worker.html');
  if (typeof chrome.runtime.getContexts === 'function') {
    const contexts = await chrome.runtime.getContexts({
      contextTypes: ['OFFSCREEN_DOCUMENT'],
      documentUrls: [offscreenUrl],
    });
    if (contexts.length > 0) return;
  } else if (typeof chrome.offscreen.hasDocument === 'function' && await chrome.offscreen.hasDocument()) {
    return;
  }
  if (creatingMediaWorker) return creatingMediaWorker;
  creatingMediaWorker = chrome.offscreen.createDocument({
    url: 'media-worker.html',
    reasons: ['BLOBS'],
    justification: 'Fetch approved public media and complete resumable uploads without MV3 worker interruption.',
  }).catch((error) => {
    if (!/single offscreen document|already exists/i.test(String(error?.message || error))) throw error;
  }).finally(() => { creatingMediaWorker = null; });
  return creatingMediaWorker;
}

async function queueManualDiscovery(discoveryPackage) {
  // Mature page readers emit one surface-card shape and all delivery is routed
  // through this one runtime; there is no second delivery protocol.
  const cards = Array.isArray(discoveryPackage?.cards) ? discoveryPackage.cards : [];
  const platform = String(discoveryPackage?.platform || 'xhs');
  const query = String(discoveryPackage?.query || '').trim();
  const authorExternalId = String(discoveryPackage?.authorExternalId || '').trim();
  if (!query && !authorExternalId) throw new Error('linggan_discovery_target_required');
  const capturePackage = packageDiscovery({
    platform, cards, query, authorExternalId,
    observedAt: String(discoveryPackage?.observedAt || new Date().toISOString()),
    surface: String(discoveryPackage?.surface || 'current_visible_surface'),
  });
  const capability = authorExternalId ? 'profile_discovery' : 'discovery_search';
  return queueCapturePackage({
    taskSpec: createManualRuntimeTask({
      platform, pageType: authorExternalId ? 'profile' : 'search_results',
      target: authorExternalId ? { authorExternalId, surface: 'current_visible_surface' } : { query, surface: 'current_visible_surface' },
      capabilitiesRequested: [capability], maximumQuota: Math.max(1, capturePackage.records.length),
      stopConditions: ['current_surface_read_once', 'maximum_quota'],
    }),
    capturePackage,
  });
}

async function deterministicUuid(seed) {
  const digest = new Uint8Array(await crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(String(seed || '')),
  ));
  const bytes = digest.slice(0, 16);
  bytes[6] = (bytes[6] & 0x0f) | 0x50;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  const hex = Array.from(bytes, (value) => value.toString(16).padStart(2, '0')).join('');
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

/**
 * 这条任务在导航前是否已经拿到服务端铸造的交付身份？
 *
 * 拿到就用它，而不是在本机另算一个：断网期间取回的包在投递时，租约可能已经结束，
 * 服务端只认它自己登记过的那个身份。这里不改写任何已经入队的包——旧包的身份是它自己的
 * 历史事实，不往上重挂新身份。
 */
async function preparedAttemptIdForTask(taskId) {
  const key = String(taskId || '').trim();
  if (!key) return '';
  try {
    const row = await detailPageLanePreparationStore.get(key);
    return String(row?.attemptId || '').trim();
  } catch {
    // 本机读取失败不能变成「没有身份就换一个」：返回空表示按旧路径处理。
    return '';
  }
}

async function queueCapturePackage({ taskSpec, capturePackage, idempotencyKey = '' } = {}) {
  // Keep the stable-instance lookup callable.  Naming the local result
  // `producerInstanceId` shadows the helper for this whole block, which turns
  // the first current-surface delivery into a temporal-dead-zone failure.
  const instanceId = await producerInstanceId();
  if (!taskSpec || !capturePackage) {
    throw new Error('linggan_capture_package_required');
  }
  validateTaskSpec(taskSpec);
  const stableKey = String(idempotencyKey || '').trim();
  const preparedAttemptId = await preparedAttemptIdForTask(taskSpec.taskId);
  const attemptId = preparedAttemptId || (stableKey
    ? await deterministicUuid(`attempt:${instanceId}:${taskSpec.taskId}:${stableKey}`)
    : crypto.randomUUID());
  const submissionId = stableKey
    ? await deterministicUuid(`submission:${instanceId}:${taskSpec.taskId}:${stableKey}`)
    : crypto.randomUUID();
  const attempt = createLocalAttempt({
    producerInstanceId: instanceId,
    taskId: taskSpec.taskId,
    attemptId,
  });
  const submission = createLocalSubmission({
    producerInstanceId: instanceId,
    taskId: taskSpec.taskId,
    attemptId: attempt.attemptId,
    capturePackage,
    submissionId,
  });
  const queuedEnvelope = stableKey
    ? { ...submission, taskSpec, attempt, idempotencyKey: stableKey }
    : { ...submission, taskSpec, attempt };
  const queuedRow = await localProducerOutbox.enqueue(queuedEnvelope);
  // The capture boundary ends once the durable browser outbox has accepted this envelope.
  // Network delivery happens behind it so a slow or unavailable Linggan service never holds
  // the page-side capture path hostage.
  void flushLocalOutbox();
  return {
    success: true,
    queued: true,
    delivery: 'pending',
    submissionId: queuedRow.submissionId,
    taskId: taskSpec.taskId,
    attemptId: queuedRow.attemptId,
  };
}

async function queueMediaSlots({ taskSpec, capturePackage, idempotencyKey = '' } = {}) {
  const queued = await queueCapturePackage({ taskSpec, capturePackage, idempotencyKey });
  const records = Array.isArray(capturePackage?.records) ? capturePackage.records : [];
  let mediaQueued = 0;
  for (const record of records) {
    const observationRef = String(record?.observationRef || '').trim();
    if (!observationRef) continue;
    for (const unit of mediaUploadUnitsForRecord(record)) {
      await localMediaOutbox.enqueue({
        uploadId: await deterministicUuid(`media:${queued.submissionId}:${observationRef}:${unit.componentKind}`),
        slotSubmissionId: queued.submissionId,
        mediaObservationRef: observationRef,
        componentKind: unit.componentKind,
        candidateUris: unit.candidateUris,
      });
      mediaQueued += 1;
    }
  }
  void flushLocalOutbox();
  return { ...queued, mediaQueued, mediaDelivery: 'pending' };
}

async function openDashboard() {
  const [activeTab] = await chrome.tabs.query({ active: true, currentWindow: true });
  if (activeTab?.id) {
    try {
      const response = await chrome.tabs.sendMessage(activeTab.id, { action: LINGGAN_RUNTIME_ACTION.TOGGLE_DASHBOARD });
      if (response?.success) return { success: true, mode: 'page_overlay' };
    } catch {
      // The current tab is not a supported producer page. Use Linggan's own read surface rather
      // than an extension-only dashboard disconnected from the runtime truth system.
    }
  }
  await chrome.tabs.create({ url: `${LINGGAN_LOCAL_ORIGIN}/corpus/evidence` });
  return { success: true, mode: 'evidence_library' };
}

async function getLingganStatus() {
  const readiness = await readLingganLocalReadiness();
  return {
    success: true,
    serverUrl: LINGGAN_LOCAL_ORIGIN,
    enabled: true,
    mode: 'linggan_browser_producer_runtime',
    readiness,
  };
}

/**
 * 报到并汇报真实状态。
 *
 * 报到只写 Linggan 本机记录，不访问任何平台，也不会让任何采集开始——因此它可以在
 * 每次询问状态时执行，而不需要人先批准什么。
 *
 * `registered` 的含义很窄：本次安装已归位到一台工位。仅仅「报到成功」不算，因为
 * 未归位的安装不会被派任何活。
 */
async function reportStationStatus() {
  const readiness = await readLingganLocalReadiness();
  const pluginVersion = chrome.runtime.getManifest().version;
  if (!readiness.reachable) {
    return {
      success: true,
      registered: false,
      authorized: false,
      pluginVersion,
      authorizationMessage: readiness.message,
    };
  }
  const installKey = await producerInstanceId();
  const storedBeforeCheckIn = await readInstallationCredentialRecord(
    chrome.storage.local,
    installKey,
  );
  if (storedBeforeCheckIn?.pending && storedBeforeCheckIn.installationRef) {
    const activation = await activateLingganInstallationCredential({
      installationRef: storedBeforeCheckIn.installationRef,
      credentialRef: storedBeforeCheckIn.pending.credentialRef,
      installationCredential: storedBeforeCheckIn.pending.credential,
      health: readiness.health,
    });
    if (activation.activated) {
      await activatePendingInstallationCredential(
        chrome.storage.local,
        installKey,
        storedBeforeCheckIn.pending.credentialRef,
      );
    }
  }
  const activeCredential = await installationCredentialFor(installKey);
  const checkIn = await checkInLingganStation({
    installKey,
    installationCredential: activeCredential,
    pluginVersion,
    browserLabel: browserLabel(),
    capabilities: await declaredCapabilities(),
    health: readiness.health,
  });
  if (checkIn.installationCredential
      && checkIn.installationCredentialRef
      && checkIn.installationRef) {
    await persistPendingInstallationCredential(chrome.storage.local, {
      installKey,
      installationRef: checkIn.installationRef,
      credentialRef: checkIn.installationCredentialRef,
      credential: checkIn.installationCredential,
    });
    const activation = await activateLingganInstallationCredential({
      installationRef: checkIn.installationRef,
      credentialRef: checkIn.installationCredentialRef,
      installationCredential: checkIn.installationCredential,
      health: readiness.health,
    });
    if (activation.activated) {
      await activatePendingInstallationCredential(
        chrome.storage.local,
        installKey,
        checkIn.installationCredentialRef,
      );
    }
  }
  return {
    success: true,
    registered: (checkIn.state === 'claimed' || checkIn.state === 'heartbeat')
      && Boolean(checkIn.stationRef),
    authorized: Boolean(checkIn.checkedIn),
    pluginVersion,
    stationState: checkIn.state || 'unknown',
    stationName: checkIn.stationDisplayName || '',
    stationAccepting: checkIn.stationAccepting,
    installationRef: checkIn.installationRef || '',
    authorizationMessage: checkIn.message,
  };
}

/**
 * 这次安装真正能做的事。
 *
 * 取值来自 Linggan 执行路径**已经实现**的能力，不是为了通过服务端检查而编的名字：
 * 每一项都对应 `producerRuntime.js` 里一个真实的打包函数。报一个没实现的能力，只会让
 * 工单派下来之后在执行阶段失败。
 *
 * `xhs.pageAccess` 是另一类事实：它说明浏览器已授予小红书站点权限。用
 * `chrome.permissions.contains` 直接查，不需要新增任何权限。
 */
const IMPLEMENTED_CAPABILITIES = [
  'discovery_search',
  'profile_discovery',
  'author_profile',
  'content_detail',
  'comments',
  'replies',
  'media_slots',
];

async function declaredCapabilities() {
  const capabilities = [...IMPLEMENTED_CAPABILITIES];
  try {
    const granted = await chrome.permissions.contains({
      origins: ['https://*.xiaohongshu.com/*'],
    });
    if (granted) capabilities.push('xhs.pageAccess');
  } catch {
    // 查不到就不报。少报一项只是让准入更保守，多报一项会让工单在执行时才失败。
  }
  return capabilities;
}

/// 浏览器标签只用来让人在页面上认出是哪台机器，不作为身份。
function browserLabel() {
  const agent = String(globalThis.navigator?.userAgent || '');
  if (agent.includes('Edg/')) return 'Edge';
  if (agent.includes('Chrome/')) return 'Chrome';
  return '';
}

/**
 * 主动报到。
 *
 * 此前报到只在弹窗来问状态时才跑，于是重载插件后什么也不会发生——这正是「插件绑不上」
 * 的直接原因。报到只写 Linggan 本机记录，不访问任何平台，因此可以主动做。
 *
 * MV3 的 service worker 是随事件唤醒的，所以在模块顶层调用一次即可获得天然心跳，
 * 不需要 alarms 权限（多加权限会让重装时多一次授权确认）。
 */
let checkingInStation = null;
async function checkInStationOnce() {
  if (checkingInStation) return checkingInStation;
  checkingInStation = (async () => {
    try {
      return await reportStationStatus();
    } catch {
      // 报到失败不能影响插件其余功能：Linggan 没开着是常态，不是错误。
      return null;
    } finally {
      checkingInStation = null;
    }
  })();
  return checkingInStation;
}

/**
 * 自动领活的节拍。
 *
 * 用 `chrome.alarms` 而不是长连接或 setInterval：**MV3 的 service worker 空闲会被终止**，
 * setInterval 会随之消失，长连接则是在跟平台的设计对着干。alarms 能在 worker 被杀后把它
 * 叫醒，浏览器重启也还在——它是这个平台的原生唤醒原语。
 *
 * **间隔由服务端在每次回答里给**（`nextPollAfterSeconds`）：没活就退避，有活就立刻再来。
 * 插件不自定节奏，否则想调就得重新发一版，而十个插件会各自按自己的常量敲门。
 */
const PATROL_ALARM = 'linggan-patrol';
// Chrome 对 alarms 的最小周期是 1 分钟，比这更短的退避只能靠下一次事件唤醒。
const MIN_ALARM_MINUTES = 1;
const PATROL_BOOTSTRAP_SECONDS = 60;

// Keep a durable repeating alarm instead of a one-shot wakeup.  A manual station claim happens
// on the local web surface and has no direct channel back into Chrome.  If a MV3 service worker
// is collected after receiving an `installation_not_claimed` answer, a one-shot alarm can leave
// the newly claimed install idle until another unrelated browser event.  The next tick always
// replaces this schedule with the server's newest cadence, so the server remains authoritative.
export function patrolAlarmSchedule(seconds) {
  const requestedSeconds = Number(seconds);
  // `0` is a meaningful server answer: a task was just dispatched and the next task may be
  // ready immediately.  It must become Chrome's one-minute floor, not the five-minute fallback.
  const cadenceSeconds = Number.isFinite(requestedSeconds) && requestedSeconds >= 0
    ? requestedSeconds
    : 300;
  const minutes = Math.max(MIN_ALARM_MINUTES, Math.round(cadenceSeconds / 60));
  return { delayInMinutes: minutes, periodInMinutes: minutes };
}

/**
 * 确保「还有人能叫醒我」这件事成立，且**不必要时绝不碰它**。
 *
 * 原来的写法是每轮结束都无脑重建一次闹钟。在 Chrome 里重建等于「先删旧的、再建新的」
 * 两步，而这一步恰好排在一轮工作的最末尾——清发件箱、最多下载 12 个媒体之后，正是 MV3
 * 最可能回收这个 worker 的时刻。一旦在「删」与「建」之间被掐断，旧闹钟没了、新闹钟还没
 * 建起来，**插件从此再也没有任何东西能叫醒它**，只剩「人正好打开小红书」这一条活路：
 * 内容脚本的被动上报会顺带把 worker 叫醒，于是「手动刷一下小红书就好了」成了这套系统
 * 事实上的唯一节拍器。
 *
 * 2026-09-13 实测到这个状态：两台工位各自睡了 2 小时 26 分，期间服务端健康、凭据有效、
 * 机器没休眠、Chrome 没重启；重载插件（走 onInstalled，那条路径是先建闹钟再干活）后
 * 立刻恢复，并稳定按 5 分 00 秒的节奏报到。
 *
 * 三条一起改：
 * - **节奏没变就不动它。** 绝大多数轮次都是 300 秒 → 300 秒，本来就不需要重建，
 *   不重建就不会暴露在那个删-建的缝隙里。
 * - **闹钟不在就补一个。** 兜底，防住这里没想到的其它丢失路径。
 * - **由调用方在干活之前先调一次**（见 worker 启动处），而不是留到最后。
 *
 * `onlyIfMissing` 区分两种调用意图：worker 刚醒时只想确认「还有人能叫醒下一次」，
 * 不该把服务端刚给的 5 分钟节奏改回 1 分钟；一轮跑完后才是真正要写入新节奏的时刻。
 */
export async function ensurePatrolAlarm(seconds, { onlyIfMissing = false } = {}) {
  const alarms = globalThis.chrome?.alarms;
  if (!alarms?.create) return null;
  const schedule = patrolAlarmSchedule(seconds);
  let existing = null;
  try {
    existing = typeof alarms.get === 'function' ? await alarms.get(PATROL_ALARM) : null;
  } catch {
    // 读不到就当它不存在：多建一个闹钟的代价，远小于一个永远醒不过来的 worker。
    existing = null;
  }
  if (existing && (onlyIfMissing || existing.periodInMinutes === schedule.periodInMinutes)) {
    return { created: false, schedule };
  }
  await alarms.create(PATROL_ALARM, schedule);
  return { created: true, schedule };
}

async function scheduleNextClaim(seconds) {
  return ensurePatrolAlarm(seconds);
}

let patrolInFlight = null;
async function patrolTick() {
  if (patrolInFlight) return patrolInFlight;
  patrolInFlight = (async () => {
    // A claimed station is automatic, not perpetually assumed alive. Every durable alarm wake
    // refreshes only the local installation heartbeat before it asks the server for work.
    const station = await checkInStationOnce();
    let result = station?.authorized && station?.registered
      ? await runDispatchedTask().catch(() => null)
      : {
          success: false,
          state: 'station_check_in_unavailable',
          executed: false,
          nextPollAfterSeconds: 300,
        };

    // Finish delivery of the just-captured discovery package before asking for its derived cover
    // work. The media lane remains separate: a slow CDN does not hold the discovery receipt open.
    await flushLocalOutbox().catch(() => null);
    const mediaResult = await drainMediaAcquisitions().catch(() => null);
    // 服务端说了下次隔多久；说不出来就用 5 分钟退避。
    const nextPollAfterSeconds = Math.min(
      Number(result?.nextPollAfterSeconds ?? 300),
      Number(mediaResult?.nextPollAfterSeconds ?? 300),
    );
    await scheduleNextClaim(nextPollAfterSeconds);
    return { ...result, media: mediaResult, nextPollAfterSeconds };
  })();
  try {
    return await patrolInFlight;
  } finally {
    patrolInFlight = null;
  }
}

async function drainMediaAcquisitions(limit = 12) {
  const results = [];
  for (let index = 0; index < limit; index += 1) {
    const result = await runMediaAcquisitionOnce();
    results.push(result);
    if (!result?.executed) break;
  }
  const last = results.at(-1);
  return {
    success: results.some((result) => result?.success),
    state: last?.state || 'media_unavailable',
    executed: results.filter((result) => result?.executed).length,
    nextPollAfterSeconds: last?.nextPollAfterSeconds ?? 300,
  };
}

async function runMediaAcquisitionOnce() {
  const readiness = await readLingganLocalReadiness();
  if (!readiness.reachable || !readiness.deliveryReady) {
    return { success: false, state: 'unreachable', nextPollAfterSeconds: 900 };
  }
  const installKey = await producerInstanceId();
  const installationCredential = await installationCredentialFor(installKey);
  const claim = await claimLingganMediaAcquisition({
    installKey,
    installationCredential,
    health: readiness.health,
  });
  if (!claim.mayExecute) {
    return {
      success: true,
      state: claim.decision,
      executed: false,
      nextPollAfterSeconds: claim.nextPollAfterSeconds,
    };
  }
  return executeClaimedMediaAcquisition({
    claim,
    installKey,
    allowCandidate: allowedMediaCandidateUri,
    normalizeCandidate: normalizeMediaCandidateUri,
    outbox: localMediaOutbox,
    flush: flushMediaOutbox,
    // 凭据必须一路带到失败上报。`recordMediaDownloadFailure` 的第四个参数原来没人传，
    // 于是它永远发出一个「带 workRef、带 installKey、却没有凭据」的请求——服务端那一支
    // 正是 `installation_credential_required`，稳定回 401。后果不是噪音：下载尝试虽然
    // 记下了，但**这张工单失败这件事从来没有被服务端登记过**，401 又被上层 `.catch`
    // 静静吞掉，于是工单侧看到的是「什么都没发生」。
    recordFailure: (upload, error) => recordMediaDownloadFailure(
      upload,
      error,
      fetch,
      installationCredential,
    ),
    scheduleRecovery: scheduleNextClaim,
  });
}

chrome.alarms?.onAlarm?.addListener((alarm) => {
  if (alarm.name === PATROL_ALARM) void patrolTick();
});

export function recoverPatrolWakeAfterLifecycleRestart() {
  // Schedule before the asynchronous check-in begins.  `onInstalled` / `onStartup` handlers
  // are short-lived MV3 events: if Chrome collects the worker after its check-in, this durable
  // minute-level bootstrap is still present and will wake it to make the first server claim.
  // A completed patrolTick immediately replaces this bootstrap cadence with the server answer.
  void scheduleNextClaim(PATROL_BOOTSTRAP_SECONDS);
  void checkInStationOnce().then(() => patrolTick());
}

/**
 * 把 `chrome.storage.session` 对内容脚本放开。
 *
 * Chrome 的默认值是 `TRUSTED_CONTEXTS`：service worker、popup 读得到，内容脚本读不到，
 * 报错原文是「Access to storage is not allowed from this context.」。本仓库此前从未调用过
 * `setAccessLevel`，于是页面里的仪表盘桥接存不进 nonce，随后每条指令都被自己拒掉。
 *
 * 只放开 session 这一个区，且它本来就是**每次浏览器重启即清空**的短期区；放进去的是一次性
 * 的会话 nonce，不是凭据、不是账号身份。
 */
function openSessionStorageToContentScripts() {
  const session = globalThis.chrome?.storage?.session;
  if (typeof session?.setAccessLevel !== 'function') return;
  // 这一行跑在模块顶层：**同步抛错会让整个 background 模块加载失败**——闹钟监听器、
  // onInstalled/onStartup、启动那一次补位全都注册不上，插件彻底变砖。而
  // `Promise.resolve(f())` 接不住 `f()` 的同步异常：参数先求值，那一刻 promise 还不存在，
  // 后面的 `.catch` 根本没机会介入。所以这里必须是 try/catch，不能只靠 `.catch`。
  try {
    void Promise.resolve(
      session.setAccessLevel({ accessLevel: 'TRUSTED_AND_UNTRUSTED_CONTEXTS' }),
    ).catch(() => {
      // 放开失败不该影响插件其余功能：仪表盘桥接会退回只用 local，其余链路不依赖 session。
    });
  } catch {
    // 同上：放不开就放不开，绝不能把整个 worker 拖下水。
  }
}
openSessionStorageToContentScripts();

chrome.runtime.onInstalled?.addListener(() => {
  // 安装、升级或开发者重载后先持久化一次最短唤醒，再报到并领活；不再要求用户打开弹窗点击「领取」。
  recoverPatrolWakeAfterLifecycleRestart();
});
chrome.runtime.onStartup?.addListener(() => {
  recoverPatrolWakeAfterLifecycleRestart();
});
// service worker 每次被唤醒都会执行到这里。
//
// **顺序是这一段的全部意义**：先确认「还有人能叫醒下一次」，再去签到和领活。下面那两件事
// 可能跑很久（清发件箱、下载媒体），而 MV3 随时会回收这个 worker；把「保住唤醒能力」放在
// 所有慢活之前，是整条链路上唯一不能被打断的一步。用 `onlyIfMissing` 是因为这里只负责
// 兜底补一个，不负责改节奏——服务端刚给的 5 分钟不该被这里改回 1 分钟。
//
// alarm 事件可能紧接着到达，`patrolInFlight` 会把两次入口合并为同一个领取请求。
void ensurePatrolAlarm(PATROL_BOOTSTRAP_SECONDS, { onlyIfMissing: true })
  .catch(() => null)
  .finally(() => {
    void checkInStationOnce().then(() => patrolTick());
  });

/**
 * 领一个服务端派下来的任务并执行它。
 *
 * **只有 `mayExecute` 为真才会碰平台。**拿到任务体不等于拿到许可——闸门、风险暂停、当日
 * 额度都可能在返回任务体的同时否决执行，`claimLingganDispatch` 因此在不许可时根本不把
 * 任务体带出来。
 *
 * 执行严格按派下来的规格：目标、配额、能力都来自任务，不来自页面上的对话框。这一条是
 * 授权链的意义所在——执行端不得自行放宽工单给定的边界。
 */
const SURFACE_CAPABILITIES = new Set([
  'author_profile',
  'profile_discovery',
  'discovery_search',
  'content_detail',
  'media_slots',
  'comments',
  'replies',
]);

// Keep browser-local detail out of the durable failure history.  The server
// accepts only this small vocabulary; an unknown page receipt is honestly a
// generic page-read failure, not a new unreviewed database state.
const DISPATCH_FAILURE_CODES = new Set([
  'capability_not_executable_here',
  'target_incomplete',
  'tab_unavailable',
  'page_timeout',
  'page_unavailable',
  'page_receipt_missing',
  'page_receipt_identity_mismatch',
  'page_read_failed',
  'detail_page_url_invalid',
  'detail_page_session_grant_unavailable',
  'detail_page_session_lane_preparation_unavailable',
  'detail_page_session_recovery_required',
  'account_observation_blocked',
]);

async function requeueClaimedTaskFailure({ claim, installKey, state, message }) {
  const taskId = String(claim?.taskSpec?.taskId || '').trim();
  const normalizedState = String(state || '');
  const capability = String(claim?.taskSpec?.capabilitiesRequested?.[0] || '');
  // After a detail navigation is consumed, an unknown collector failure is
  // execution uncertainty, not permission to repeat the visit.  Keep the
  // legacy generic code only for non-detail surfaces that do not consume the
  // one-page session boundary.
  const failureCode = DISPATCH_FAILURE_CODES.has(normalizedState)
    ? normalizedState
    : (isDetailPageSessionCapability(capability)
      ? 'detail_page_session_recovery_required'
      : 'page_read_failed');
  const reported = await reportLingganDispatchFailure({
    installKey,
    installationCredential: await installationCredentialFor(installKey),
    taskId,
    failureId: crypto.randomUUID(),
    failureCode,
    health: claim?.health,
  });
  return {
    success: false,
    state: String(state || 'page_read_failed'),
    message: String(message || '页面未能执行本次派发任务。'),
    requeued: reported.reported === true,
    nextPollAfterSeconds: Number(reported.nextPollAfterSeconds || 300),
  };
}

async function reportAccountObservationFromPage(observation, { taskId = null } = {}) {
  const station = await reportStationStatus();
  const installKey = await producerInstanceId();
  const installationCredential = await installationCredentialFor(installKey);
  if (!station.authorized || !station.installationRef || !installationCredential) {
    return { reported: false, reasonCode: 'account_observation_station_not_ready' };
  }
  return reportLingganAccountEligibility({
    installationRef: station.installationRef,
    installationCredential,
    taskId,
    observation,
    health: (await readLingganLocalReadiness()).health,
  });
}

/**
 * The claimed page is the only task-scoped account observation target. It has already been
 * opened for this exact Lease, so this reads its rendered DOM and never performs a separate
 * platform navigation, refresh, fetch, or tab search. Inconclusive DOM state stays soft; every
 * conclusive observation must be accepted by the server before collection may start.
 */
async function verifyClaimedTaskAccount(tabId, claim) {
  const page = await chrome.tabs.sendMessage(tabId, {
    action: LINGGAN_RUNTIME_ACTION.OBSERVE_CLAIMED_TASK_ACCOUNT,
  });
  const observation = page?.success === true ? page.observation : null;
  if (!observation) return { mayExecute: true, state: 'account_observation_inconclusive' };
  const report = await reportAccountObservationFromPage(observation, { taskId: claim?.taskId });
  return claimedTaskAccountDecision(report);
}

async function reportClaimedTaskRisk(tabId, claim, installKey, installationCredential) {
  const page = await chrome.tabs.sendMessage(tabId, {
    action: LINGGAN_RUNTIME_ACTION.OBSERVE_CLAIMED_TASK_RISK,
  }).catch(() => null);
  const observation = page?.success === true ? page.observation : null;
  if (!observation || observation.signal !== 'risk_control_interstitial') return { riskObserved: false };
  const report = await reportLingganDetailPageRiskSignal({
    installKey, installationCredential, taskId: claim?.taskId,
    riskSignalId: crypto.randomUUID(), detectorVersion: observation.detectorVersion,
    health: claim?.health,
  });
  return { riskObserved: report.reported === true, cooldownActive: report.cooldownActive === true };
}

async function queueCachedDetailPageSessionLane({ leaseRef, taskSpec } = {}) {
  let entry;
  try {
    entry = await detailPageSessionStore.getForTask({ leaseRef, taskSpec });
  } catch (error) {
    error.detailPageSessionRecoveryRequired = true;
    throw error;
  }
  if (!entry) return null;
  if (entry.alreadyQueued) {
    // A claim response can be replayed while its durable outbox item is still being delivered.
    // Do not create a second Attempt for the same server task.
    void flushLocalOutbox();
    return {
      success: true,
      state: 'cached_page_session_already_queued',
      executed: true,
      capability: entry.capability,
      leaseRef,
      message: `已复用详情页缓存；「${entry.capability}」此前已进入待交付队列。`,
    };
  }
  let capturePackage;
  try {
    capturePackage = packageDetailPageSessionLane(entry, taskSpec);
  } catch (error) {
    error.detailPageSessionRecoveryRequired = true;
    throw error;
  }
  const idempotencyKey = `detail-session:${taskSpec.taskId}:${entry.capability}`;
  const queued = entry.capability === 'media_slots'
    ? await queueMediaSlots({ taskSpec, capturePackage, idempotencyKey })
    : await queueCapturePackage({ taskSpec, capturePackage, idempotencyKey });
  await detailPageSessionStore.markTaskQueued(entry.cacheKey, entry.capability, taskSpec.taskId);
  return {
    success: true,
    state: 'cached_page_session_queued',
    executed: true,
    capability: entry.capability,
    leaseRef,
    submissionId: queued.submissionId,
    message: `已复用同一次详情页读取结果，并将「${entry.capability}」加入待交付队列。`,
  };
}

async function storeDetailPageSessionFromPage(message = {}) {
  const entry = await detailPageSessionStore.put({
    leaseRef: message.leaseRef,
    plan: message.plan,
    note: message.note,
    commentResult: message.commentResult,
    receipt: message.receipt,
  });
  return { success: true, cacheKey: entry.cacheKey };
}

async function markDetailPageSessionTaskQueued(message = {}) {
  const marked = await detailPageSessionStore.markTaskQueued(
    String(message.cacheKey || ''),
    'content_detail',
    String(message.taskId || ''),
  );
  return { success: marked };
}

function isDetailPageSessionCapability(capability) {
  return ['content_detail', 'media_slots', 'comments', 'replies'].includes(capability);
}

/**
 * A dispatched task is replayable; a Chrome navigation is not.  This function
 * ties them together without treating server authorization as proof of a page
 * load.  The IndexedDB transaction in `consume` completes before this returns
 * `shouldNavigate: true`, so two MV3 wakeups cannot both create a window.
 */
async function prepareDetailPageNavigation({ claim, taskSpec, installKey, installationCredential }) {
  const existing = await detailPageNavigationGrantStore.get({ leaseRef: claim.leaseRef, taskSpec });
  if (existing && ['page_opened', 'action_dispatched'].includes(existing.state)) {
    const tabId = Number(existing.tabId || 0);
    const contentExternalId = String(taskSpec?.target?.contentExternalId || '').trim();
    if (tabId) {
      const tab = await chrome.tabs.get(tabId).catch(() => null);
      const tabUrl = String(tab?.url || '');
      const encodedContentId = encodeURIComponent(contentExternalId);
      if (tab && contentExternalId
          && /^https:\/\/([^.]+\.)?xiaohongshu\.com\//i.test(tabUrl)
          && (tabUrl.includes(contentExternalId) || tabUrl.includes(encodedContentId))) {
        // The page belongs to this extension-managed session.  Do not send a
        // second collector action after a worker restart: the original content
        // script can continue and its durable outbox is independently retryable.
        return {
          shouldNavigate: false, recovered: true, tabId, windowId: existing.windowId || null,
          sessionRef: existing.sessionRef || null,
        };
      }
    }
    return {
      shouldNavigate: false, suppressed: true, reason: 'navigation_state_unknown',
      sessionRef: existing.sessionRef || null,
    };
  }
  const prepared = existing || await detailPageNavigationGrantStore.prepare({
    leaseRef: claim.leaseRef,
    taskSpec,
  });
  const grant = await grantLingganDetailPageSession({
    installKey,
    installationCredential,
    taskId: taskSpec.taskId,
    grantRequestId: prepared.grantRequestId,
    executionSourceUrl: claim.executionSourceUrl,
    health: claim.health,
  });
  if (!grant.granted) {
    return { shouldNavigate: false, suppressed: true, reason: grant.reasonCode || grant.outcome };
  }
  await detailPageNavigationGrantStore.attachServerGrant({
    grantKey: prepared.grantKey,
    sessionRef: grant.sessionRef,
    plan: grant.pageSessionPlan,
    lanePreparation: grant.lanePreparation,
  });
  const consumed = await detailPageNavigationGrantStore.consume({
    grantKey: prepared.grantKey,
    sessionRef: grant.sessionRef,
  });
  return {
    shouldNavigate: consumed.shouldNavigate,
    suppressed: !consumed.shouldNavigate,
    reason: consumed.shouldNavigate ? '' : 'navigation_already_consumed',
    grantKey: prepared.grantKey,
    sessionRef: grant.sessionRef,
    pageSessionPlan: grant.pageSessionPlan,
  };
}

async function runDispatchedTask() {
  const readiness = await readLingganLocalReadiness();
  if (!readiness.reachable) {
    return { success: false, state: 'unreachable', message: readiness.message };
  }
  const installKey = await producerInstanceId();
  let installationCredential = await installationCredentialFor(installKey);
  if (!installationCredential) {
    await checkInStationOnce();
    installationCredential = await installationCredentialFor(installKey);
  }
  const claim = await claimLingganDispatch({
    installKey,
    installationCredential,
    health: readiness.health,
  });
  if (!claim.mayExecute) {
    // 不许执行不是故障：闸门默认关着就是正常状态。原样把服务端的判断带回去。
    return {
      success: true,
      state: claim.decision,
      executed: false,
      message: claim.message,
      nextPollAfterSeconds: claim.nextPollAfterSeconds,
    };
  }

  const spec = claim.taskSpec || {};
  const capability = Array.isArray(spec.capabilitiesRequested) ? spec.capabilitiesRequested[0] : '';
  if (!SURFACE_CAPABILITIES.has(capability)) {
    // Only capabilities with an implemented page reader may run unattended. Deepening is allowed
    // here because the server has already frozen exact material identities in a bounded Work Order.
    return requeueClaimedTaskFailure({
      claim: { ...claim, health: readiness.health },
      installKey,
      state: 'capability_not_executable_here',
      message: `派下来的能力「${capability}」当前不在无人值守表层采集范围内。`,
    });
  }

  const authorExternalId = String(spec.target?.authorExternalId || '').trim();
  const query = String(spec.target?.query || '').trim();
  const contentExternalId = String(spec.target?.contentExternalId || '').trim();
  const targetValue = capability === 'discovery_search'
    ? query
    : (['author_profile', 'profile_discovery'].includes(capability) ? authorExternalId : contentExternalId);
  if (!targetValue) {
    return requeueClaimedTaskFailure({
      claim: { ...claim, health: readiness.health },
      installKey,
      state: 'target_incomplete',
      message: '任务没有指明观察目标。',
    });
  }

  // The first content_detail task opens the signed page and captures the server-approved lanes.
  // Later sequential tasks from the same Work Order reuse those persisted facts, but still form
  // their own Task/Attempt/Package/Receipt only after they are actually claimed.
  if (isDetailPageSessionCapability(capability)) {
    let cached;
    try {
      cached = await queueCachedDetailPageSessionLane({
        leaseRef: claim.leaseRef,
        taskSpec: spec,
      });
    } catch (error) {
      return requeueClaimedTaskFailure({
        claim: { ...claim, health: readiness.health },
        installKey,
        state: error?.detailPageSessionRecoveryRequired
          ? 'detail_page_session_recovery_required'
          : 'page_read_failed',
        message: error?.detailPageSessionRecoveryRequired
          ? '同页缓存身份或内容无法验证；已停止自动重开详情页，并将该内容标记为不可自动恢复。'
          : '本机待交付队列暂不可写；没有打开页面，任务将按退避重试交付。',
      });
    }
    if (cached) return { ...cached, nextPollAfterSeconds: claim.nextPollAfterSeconds };

    // Only the first `content_detail` task is permitted to own a page.  A
    // cache miss in a later lane is an execution-uncertainty fact, never an
    // excuse to visit the note again for comments, replies or media.
    if (capability !== 'content_detail' || !claim.pageSessionPlan) {
      return requeueClaimedTaskFailure({
        claim: { ...claim, health: readiness.health },
        installKey,
        state: 'detail_page_session_recovery_required',
        message: '本机没有可交付的同页结果；已停止自动重开详情页，并将该内容标记为不可自动恢复。',
      });
    }
  }
  let navigationGrant = null;
  if (capability === 'content_detail' && claim.pageSessionPlan) {
    try {
      navigationGrant = await prepareDetailPageNavigation({
        claim: { ...claim, health: readiness.health },
        taskSpec: spec,
        installKey,
        installationCredential,
      });
    } catch {
      return requeueClaimedTaskFailure({
        claim: { ...claim, health: readiness.health },
        installKey,
        state: 'detail_page_session_recovery_required',
        message: '详情页导航状态无法确认；为避免重复访问，未自动打开页面，并将该内容标记为不可自动恢复。',
      });
    }
    if (!navigationGrant.shouldNavigate) {
      if (navigationGrant.reason === 'grant_transport_unavailable'
          || navigationGrant.reason === 'grant_route_unavailable') {
        return requeueClaimedTaskFailure({
          claim: { ...claim, health: readiness.health },
          installKey,
          state: 'detail_page_session_grant_unavailable',
          message: '详情页授权服务暂不可用；没有打开页面，任务已进入退避后重试。',
        });
      }
      // 服务端通告了准备握手却没给出准备回执：这一页没有可持久化的交付身份，
      // 打开它等于在没有交付凭据的情况下消耗一次平台访问。停在原地，等人处理。
      if (navigationGrant.reason === 'grant_lane_preparation_missing') {
        return requeueClaimedTaskFailure({
          claim: { ...claim, health: readiness.health },
          installKey,
          state: 'detail_page_session_lane_preparation_unavailable',
          message: '服务端未给出本次导航的通道交付身份；没有打开页面，该内容需人工处理。',
        });
      }
      if (!navigationGrant.recovered && navigationGrant.reason === 'navigation_state_unknown'
          && navigationGrant.sessionRef) {
        await reportLingganDetailPageSessionNavigation({
          installKey, installationCredential, taskId: spec.taskId,
          sessionRef: navigationGrant.sessionRef, kind: 'stopped',
          stopReason: 'navigation_state_unknown', health: readiness.health,
        }).catch(() => null);
      }
      if (!navigationGrant.recovered) {
        return requeueClaimedTaskFailure({
          claim: { ...claim, health: readiness.health },
          installKey,
          state: 'detail_page_session_recovery_required',
          message: '详情页许可已被其他执行状态占用或已不可确认；未自动重开页面，已标记为不可自动恢复。',
        });
      }
      return {
        success: true,
        state: 'detail_page_session_recovered',
        executed: false,
        leaseRef: claim.leaseRef,
        nextPollAfterSeconds: claim.nextPollAfterSeconds,
        message: navigationGrant.recovered
          ? '已确认同一详情页仍由本安装管理；未重复打开或重复下发采集动作。'
          : '详情页导航许可已消费或状态不明；已停止自动重开页面。',
      };
    }
  }
  let opened;
  try {
    opened = await openTaskWindow(capability, targetValue, claim.executionSourceUrl);
  } catch {
    if (navigationGrant?.sessionRef) {
      await reportLingganDetailPageSessionNavigation({
        installKey, installationCredential, taskId: spec.taskId,
        sessionRef: navigationGrant.sessionRef, kind: 'stopped',
        stopReason: 'navigation_state_unknown', health: readiness.health,
      }).catch(() => null);
    }
    return requeueClaimedTaskFailure({
      claim: { ...claim, health: readiness.health },
      installKey,
      state: 'detail_page_session_recovery_required',
      message: '详情页许可已消费但页面未能确认打开；为避免重复访问，已标记为不可自动恢复。',
    });
  }
  const { windowId, tabId } = opened;
  if (!tabId) {
    await closeCollectionWindow(windowId);
    if (navigationGrant?.sessionRef) {
      await reportLingganDetailPageSessionNavigation({
        installKey, installationCredential, taskId: spec.taskId,
        sessionRef: navigationGrant.sessionRef, kind: 'stopped',
        stopReason: 'navigation_state_unknown', health: readiness.health,
      }).catch(() => null);
    }
    return requeueClaimedTaskFailure({
      claim: { ...claim, health: readiness.health },
      installKey,
      state: 'detail_page_session_recovery_required',
      message: '详情页许可已消费但没有可确认的页面；为避免重复访问，已标记为不可自动恢复。',
    });
  }
  if (navigationGrant?.grantKey) {
    try {
      await detailPageNavigationGrantStore.recordWindow({
        grantKey: navigationGrant.grantKey,
        windowId,
        tabId,
      });
    } catch {
      await closeCollectionWindow(windowId);
      if (navigationGrant?.sessionRef) {
        await reportLingganDetailPageSessionNavigation({
          installKey, installationCredential, taskId: spec.taskId,
          sessionRef: navigationGrant.sessionRef, kind: 'stopped',
          stopReason: 'navigation_state_unknown', health: readiness.health,
        }).catch(() => null);
      }
      return requeueClaimedTaskFailure({
        claim: { ...claim, health: readiness.health },
        installKey,
        state: 'detail_page_session_recovery_required',
        message: '页面窗口未能持久登记且已关闭；为避免重复访问，已标记为不可自动恢复。',
      });
    }
  }

  const action = capability === 'author_profile'
    ? LINGGAN_RUNTIME_ACTION.COLLECT_CURRENT_AUTHOR
    : (['profile_discovery', 'discovery_search'].includes(capability)
      ? LINGGAN_RUNTIME_ACTION.DISCOVER_SURFACE
      : (['comments', 'replies'].includes(capability)
        ? LINGGAN_RUNTIME_ACTION.COLLECT_CURRENT_COMMENTS
        : (capability === 'content_detail' && claim.pageSessionPlan
          ? LINGGAN_RUNTIME_ACTION.COLLECT_NOTE_FULL
          : LINGGAN_RUNTIME_ACTION.COLLECT_CURRENT_CONTENT)));
  try {
    const ready = await waitForTabReady(tabId);
    if (!ready) {
      if (navigationGrant?.sessionRef) {
        await reportLingganDetailPageSessionNavigation({
          installKey, installationCredential, taskId: spec.taskId,
          sessionRef: navigationGrant.sessionRef, kind: 'stopped',
          stopReason: 'page_unavailable', health: readiness.health,
        }).catch(() => null);
      }
      return requeueClaimedTaskFailure({
        claim: { ...claim, health: readiness.health },
        installKey,
        state: 'detail_page_session_recovery_required',
        message: '已消费详情页导航许可但页面未就绪；为避免重复开页，冻结 lane 已结束为不可用。',
      });
    }
    if (navigationGrant?.sessionRef) {
      // Only the stable, content-script-confirmed final document counts as a
      // Chrome navigation observation. A tab id returned by window creation
      // or a timeout is not proof that the detail page committed.
      void reportLingganDetailPageSessionNavigation({
        installKey,
        installationCredential,
        taskId: spec.taskId,
        sessionRef: navigationGrant.sessionRef,
        kind: 'navigation_observed',
        health: readiness.health,
      });
    }
    const pageRisk = await reportClaimedTaskRisk(
      tabId, { ...claim, health: readiness.health }, installKey, installationCredential,
    );
    if (pageRisk.riskObserved) {
      if (navigationGrant?.sessionRef) {
        await reportLingganDetailPageSessionNavigation({
          installKey, installationCredential, taskId: spec.taskId,
          sessionRef: navigationGrant.sessionRef, kind: 'stopped',
          stopReason: 'risk_stop', health: readiness.health,
        });
      }
      return requeueClaimedTaskFailure({
        claim: { ...claim, health: readiness.health }, installKey,
        state: 'account_observation_blocked',
        message: pageRisk.cooldownActive
          ? '页面连续出现风控提示；当前插件已自动暂停接单 12 小时。'
          : '页面出现风控提示；已停止当前采集，继续观察同一插件的风险信号。',
      });
    }
    const accountVerification = await verifyClaimedTaskAccount(tabId, claim);
    if (!accountVerification.mayExecute) {
      if (navigationGrant?.sessionRef) {
        void reportLingganDetailPageSessionNavigation({
          installKey, installationCredential, taskId: spec.taskId,
          sessionRef: navigationGrant.sessionRef, kind: 'stopped',
          stopReason: 'risk_stop', health: readiness.health,
        });
      }
      return requeueClaimedTaskFailure({
        claim: { ...claim, health: readiness.health },
        installKey,
        state: accountVerification.state,
        message: accountVerification.message,
      });
    }
    if (navigationGrant?.grantKey) {
      await detailPageNavigationGrantStore.markActionDispatched({ grantKey: navigationGrant.grantKey });
    }
    const response = await chrome.tabs.sendMessage(tabId, {
      action,
      mode: capability === 'discovery_search' ? 'search' : 'profile',
      // 配额来自工单，不来自页面对话框：执行端不得自行放宽。
      maximumQuota: dispatchedMaximumQuota(spec, 1),
      commentLimit: spec.commentLimit,
      maxTotal: dispatchedCommentMaxTotal(spec, capability),
      maxSubComments: Number(spec.target?.replyExpandLimit) || 0,
      commentDepthMode: capability === 'replies' ? 'allReplies' : 'twoLevel',
      // The page collector must submit against this exact server-issued identity. Rebuilding a
      // manual task here would leave the claimed scheduled task without Attempt or Receipt.
      taskSpec: spec,
      leaseRef: claim.leaseRef,
      pageSessionPlan: navigationGrant?.pageSessionPlan || claim.pageSessionPlan,
      triggerSource: 'linggan_dispatched_task',
    });
    const receipt = decodePageExecutionReceipt(response, {
      action,
      capability,
      taskId: spec.taskId,
    });
    if (!receipt.ok) {
      return requeueClaimedTaskFailure({
        claim: { ...claim, health: readiness.health },
        installKey,
        state: receipt.state,
        message: receipt.message || `页面未能执行「${capability}」。`,
      });
    }
    if (navigationGrant?.sessionRef) {
      void reportLingganDetailPageSessionNavigation({
        installKey, installationCredential, taskId: spec.taskId,
        sessionRef: navigationGrant.sessionRef, kind: 'delivery_pending',
        health: readiness.health,
      });
    }
    return {
      success: true,
      state: 'executed',
      executed: true,
      capability,
      leaseRef: claim.leaseRef,
      nextPollAfterSeconds: claim.nextPollAfterSeconds,
      message: receipt.message || `已按派下来的任务执行「${capability}」。`,
    };
  } catch (error) {
    if (navigationGrant?.sessionRef) {
      void reportLingganDetailPageSessionNavigation({
        installKey, installationCredential, taskId: spec.taskId,
        sessionRef: navigationGrant.sessionRef, kind: 'stopped',
        stopReason: 'page_unavailable', health: readiness.health,
      });
    }
    return requeueClaimedTaskFailure({
      claim: { ...claim, health: readiness.health },
      installKey,
      state: 'page_unavailable',
      message: '观察页面未能响应，本次未采集。',
    });
  } finally {
    // 无论成败都关窗。留下的僵尸窗口会一直吃内存——1000 篇/天会开上百次窗。
    await closeCollectionWindow(windowId);
  }
}

/**
 * 在一个独立的、不抢焦点的窗口里打开创作者主页。
 *
 * 与内容工作台同一套做法（`navigationOrchestrator`）：`focused: false` 不打断人手上的
 * 工作，`autoDiscardable: false` 防止浏览器在采集中途把标签页丢弃。**跑完即关**——
 * 1000 篇/天意味着窗口会开上百次，不关就是几十个标签页常驻吃内存。
 *
 * **平台 ID 才是身份**；详情页还必须使用服务端从已接纳发现材料选出的短期签名链接。
 * 签名链接只负责定位页面，不能反过来成为 Task 或作品身份。
 */
async function openTaskWindow(capability, targetValue, executionSourceUrl = '') {
  const url = capability === 'discovery_search'
    ? `https://www.xiaohongshu.com/search_result?keyword=${encodeURIComponent(targetValue)}&source=web_explore_feed`
    : (['author_profile', 'profile_discovery'].includes(capability)
      ? `https://www.xiaohongshu.com/user/profile/${encodeURIComponent(targetValue)}`
      : buildSignedXhsDetailExecutionUrl(targetValue, executionSourceUrl));
  if (!url) return { windowId: null, tabId: null };
  const created = await chrome.windows.create({ url, focused: false, type: 'normal' });
  const tabId = Number(created?.tabs?.[0]?.id || 0) || null;
  if (tabId) {
    await chrome.tabs.update(tabId, { autoDiscardable: false }).catch(() => {});
  }
  return { windowId: created?.id ?? null, tabId };
}

/// 采集窗口用完即关。失败也要关：留下的僵尸窗口会一直吃内存。
async function closeCollectionWindow(windowId) {
  if (!windowId) return;
  try {
    await chrome.windows.remove(windowId);
  } catch {
    // 窗口可能已被人手动关掉，那不是错误。
  }
}

/// 等最终页面完成重定向且 content script 已属于当前 URL。
function waitForTabReady(tabId, timeoutMs = 20000) {
  return waitForStableTab({
    tabs: chrome.tabs,
    tabId,
    readinessAction: LINGGAN_RUNTIME_ACTION.GET_PAGE_CONTEXT,
    timeoutMs,
  });
}

chrome.runtime.onMessage.addListener((message = {}, sender, sendResponse) => {
  const action = String(message.action || '').trim();
  Promise.resolve().then(async () => {
    if (action === LINGGAN_RUNTIME_ACTION.TOGGLE_DASHBOARD) return openDashboard();
    if (action === LINGGAN_RUNTIME_ACTION.GET_FLYWHEEL_CONFIG || action === LINGGAN_RUNTIME_ACTION.SAVE_FLYWHEEL_CONFIG) return getLingganStatus();
    if (action === LINGGAN_RUNTIME_ACTION.RUN_DISPATCHED_TASK) return runDispatchedTask();
    if (action === LINGGAN_RUNTIME_ACTION.GET_EXECUTION_STATION_STATUS) {
      return reportStationStatus();
    }
    if (action === LINGGAN_RUNTIME_ACTION.REPORT_ACCOUNT_ELIGIBILITY) {
      const senderUrl = String(sender?.tab?.url || sender?.url || '');
      if (!/^https:\/\/([^.]+\.)?xiaohongshu\.com\//i.test(senderUrl)) {
        return { reported: false, reasonCode: 'account_observation_source_invalid' };
      }
      return reportAccountObservationFromPage(message.observation);
    }
    if ([
      LINGGAN_RUNTIME_ACTION.STORE_DETAIL_PAGE_SESSION,
      LINGGAN_RUNTIME_ACTION.MARK_DETAIL_PAGE_SESSION_TASK_QUEUED,
    ].includes(action)) {
      const senderUrl = String(sender?.tab?.url || sender?.url || '');
      if (!/^https:\/\/([^.]+\.)?xiaohongshu\.com\//i.test(senderUrl)) {
        return { success: false, code: 'detail_page_session_source_invalid' };
      }
      return action === LINGGAN_RUNTIME_ACTION.STORE_DETAIL_PAGE_SESSION
        ? storeDetailPageSessionFromPage(message)
        : markDetailPageSessionTaskQueued(message);
    }
    if (action === LINGGAN_RUNTIME_ACTION.TEST_FLYWHEEL_CONNECTION) return getLingganStatus();
    if (action === LINGGAN_RUNTIME_ACTION.SUBMIT_DISCOVERY_PACKAGE) {
      return queueManualDiscovery(message.discoveryPackage);
    }
    if (action === LINGGAN_RUNTIME_ACTION.SUBMIT_CAPTURE_PACKAGE) {
      return queueCapturePackage({
        taskSpec: message.taskSpec,
        capturePackage: message.capturePackage,
        idempotencyKey: message.idempotencyKey,
      });
    }
    if (action === LINGGAN_RUNTIME_ACTION.SUBMIT_MEDIA_SLOTS) {
      return queueMediaSlots({
        taskSpec: message.taskSpec,
        capturePackage: message.capturePackage,
        idempotencyKey: message.idempotencyKey,
      });
    }
    if (action === LINGGAN_RUNTIME_ACTION.FLUSH_LOCAL_OUTBOX) return flushLocalOutbox();
    if (action === LINGGAN_RUNTIME_ACTION.CREATE_MANUAL_TASK) {
      const producerInstanceId = await producerInstanceId();
      const taskSpec = createTaskSpec({
        source: 'manual', platform: 'xhs', pageType: 'search_results',
        target: { query: '__manual_placeholder__', surface: 'local_intent_only' }, capabilitiesRequested: ['discovery_search'],
        maximumQuota: 20, commentLimit: 'not_requested', acquireMedia: 'not_requested',
        riskPolicy: 'local_trusted_user_initiated', stopConditions: ['manual_stop', 'maximum_quota'],
      });
      const attempt = createLocalAttempt({ producerInstanceId, taskId: taskSpec.taskId });
      return { success: true, producerInstanceId, taskSpec, attempt, scheduler: 'NOT_CONNECTED' };
    }
    if (action === LINGGAN_RUNTIME_ACTION.GET_PRODUCER_INSTANCE) {
      return { success: true, producerInstanceId: await producerInstanceId() };
    }
    if (action === LINGGAN_RUNTIME_ACTION.CREATE_SCHEDULED_TASK) {
      // A scheduler can send a signed/authorized TaskSpec later. The browser never creates a
      // poller or station lease here; this only validates the shape for adapter tests.
      const taskSpec = createTaskSpec({ ...(message.taskSpec || {}), source: 'scheduled' });
      return { success: true, taskSpec, scheduler: 'NOT_CONNECTED', dispatch: 'NOT_STARTED' };
    }
    if (action === LINGGAN_RUNTIME_ACTION.GET_STATS) return unavailableLingganStats();
    return { success: false, code: 'linggan_runtime_action_unavailable', message: '该界面动作没有 Linggan Runtime 合同，因此没有执行任何采集或写入。' };
  }).then(sendResponse).catch((error) => {
    sendResponse({ success: false, code: 'linggan_adapter_error', message: String(error?.message || error) });
  });
  return true;
});
