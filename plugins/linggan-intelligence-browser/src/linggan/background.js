import {
  LINGGAN_LOCAL_ORIGIN,
  attemptStartIsAccepted,
  checkInLingganStation,
  claimLingganDispatch,
  claimLingganMediaAcquisition,
  createLocalAttempt,
  createLocalSubmission,
  createTaskSpec,
  isTerminalLocalDeliveryResult,
  localPost,
  readLingganLocalReadiness,
  taskCreationIsAccepted,
  unavailableLingganStats,
} from './adapter.js';
import { LINGGAN_RUNTIME_ACTION } from './runtimeActions.js';
import { localMediaOutbox, localProducerOutbox } from './localProducerOutbox.js';
import { createManualRuntimeTask, packageDiscovery } from './producerRuntime.js';

const PRODUCER_INSTANCE_KEY = 'linggan.localTrusted.producerInstanceId';
const MAX_MEDIA_BYTES = 256 * 1024 * 1024;
const MEDIA_CHUNK_BYTES = 1024 * 1024;
let flushingOutbox = null;

const PLATFORM_MEDIA_HOST_SUFFIXES = Object.freeze([
  '.xhscdn.com', '.xiaohongshu.com', '.byteimg.com', '.bytevcloudcdn.com',
  '.douyinpic.com', '.douyinstatic.com', '.douyinvod.com', '.amemv.com',
]);

function allowedMediaCandidateUri(value) {
  try {
    const uri = new URL(String(value || '').trim());
    if (uri.protocol !== 'https:' || uri.username || uri.password || uri.port) return false;
    const hostname = uri.hostname.toLowerCase();
    return PLATFORM_MEDIA_HOST_SUFFIXES.some((suffix) => hostname.endsWith(suffix));
  } catch {
    return false;
  }
}

async function producerInstanceId() {
  const stored = await chrome.storage.local.get(PRODUCER_INSTANCE_KEY);
  const current = String(stored[PRODUCER_INSTANCE_KEY] || '').trim();
  if (current) return current;
  const created = crypto.randomUUID();
  await chrome.storage.local.set({ [PRODUCER_INSTANCE_KEY]: created });
  return created;
}

export async function flushLocalOutboxOnce({
  outbox = localProducerOutbox,
  mediaOutbox = localMediaOutbox,
  readReadiness = readLingganLocalReadiness,
  post = localPost,
  flushMedia = flushMediaOutbox,
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
        if (isTerminalLocalDeliveryResult(attempt)) {
          await outbox.terminal(entry.submissionId, attempt.payload.code || 'attempt_not_started');
          continue;
        }
        throw new Error(attempt.payload.code || 'attempt_not_started');
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
        await outbox.terminal(entry.submissionId, submitted.payload.code);
      } else {
        await outbox.retry(entry.submissionId, submitted.payload.code || 'submission_not_acknowledged');
      }
    } catch (error) {
      await outbox.retry(entry.submissionId, error?.message || error);
    }
  }
  await flushMedia();
  return {
    pending: await outbox.pendingCount(),
    mediaPending: await mediaOutbox.pendingCount(),
  };
}

function flushLocalOutbox() {
  if (flushingOutbox) return flushingOutbox;
  flushingOutbox = flushLocalOutboxOnce().finally(() => { flushingOutbox = null; });
  return flushingOutbox;
}

async function flushMediaOutbox() {
  const due = await localMediaOutbox.due({ limit: 2 });
  for (const upload of due) {
    if (upload.slotSubmissionId) {
      const dependency = await localProducerOutbox.get(upload.slotSubmissionId);
      if (!dependency || dependency.status === 'terminal') {
        await localMediaOutbox.terminal(upload.uploadId, 'media_slot_delivery_not_accepted');
        continue;
      }
      if (dependency.status !== 'acknowledged') continue;
    }
    await localMediaOutbox.markInFlight(upload.uploadId);
    try {
      const response = await fetchMediaCandidate(upload.candidateUris);
      const sha256 = await sha256Hex(await response.bytes.arrayBuffer());
      const uploaded = await uploadMediaInChunks({
        mediaObservationRef: upload.mediaObservationRef,
        blob: response.bytes,
        mimeType: response.mimeType,
        sha256,
      });
      await localMediaOutbox.acknowledge(upload.uploadId, uploaded);
    } catch (error) {
      // Failure is a media-lane fact, not a reason to invalidate already accepted text/discovery.
      // It is best effort because the failed candidate may be retried after an offline interval.
      const failure = await recordMediaDownloadFailure(upload, error).catch(() => null);
      if (upload.serverWorkRef) {
        // A server-owned work generation is attempted once. The server decides whether another
        // generation may be leased; retaining this generation locally would create two retry
        // authorities and could exceed the three-attempt limit.
        await localMediaOutbox.terminal(
          upload.uploadId,
          failure?.work?.state || 'media_acquisition_generation_finished',
        );
      } else {
        await localMediaOutbox.retry(upload.uploadId, error?.message || error);
      }
    }
  }
}

async function recordMediaDownloadFailure(upload, error) {
  const attemptedUri = String(upload?.candidateUris?.[0] || '').trim();
  const observationRef = String(upload?.mediaObservationRef || '').trim();
  if (!attemptedUri || !observationRef) return;
  const message = String(error?.message || error || 'unknown');
  const terminalReason = /mime/i.test(message) ? 'mime_mismatch'
    : /size/i.test(message) ? 'size_limit'
      : /http_404|expired/i.test(message) ? 'expired_url'
        : /cancel/i.test(message) ? 'cancelled' : 'network_error';
  const response = await fetch(`${LINGGAN_LOCAL_ORIGIN}/api/local/producer/media-observations/${encodeURIComponent(observationRef)}/download-failures`, {
    method: 'POST', credentials: 'omit', headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      attemptedUri,
      terminalReason,
      ...(upload?.serverWorkRef ? {
        workRef: upload.serverWorkRef,
        claimGeneration: upload.claimGeneration,
        installKey: upload.installKey,
      } : {}),
    }),
  });
  const payload = await response.json().catch(() => null);
  if (!response.ok) throw new Error(payload?.code || 'media_download_failure_not_recorded');
  return payload;
}

async function uploadMediaInChunks({ mediaObservationRef, blob, mimeType, sha256 }) {
  const start = await fetch(`${LINGGAN_LOCAL_ORIGIN}/api/local/producer/media-observations/${encodeURIComponent(mediaObservationRef)}/uploads`, {
    method: 'POST', credentials: 'omit',
    headers: {
      'x-linggan-media-sha256': sha256,
      'x-linggan-media-mime': mimeType,
      'x-linggan-media-size': String(blob.size),
    },
  });
  const startPayload = await start.json().catch(() => ({}));
  if (!start.ok) throw new Error(startPayload.code || 'media_upload_not_started');
  const sessionRef = String(startPayload.sessionRef || '').trim();
  let offset = Number(startPayload.nextOffset);
  if (!sessionRef || !Number.isInteger(offset) || offset < 0 || offset > blob.size) throw new Error('media_upload_resume_invalid');
  while (offset < blob.size) {
    const chunk = blob.slice(offset, Math.min(blob.size, offset + MEDIA_CHUNK_BYTES));
    const written = await fetch(`${LINGGAN_LOCAL_ORIGIN}/api/local/producer/media-uploads/${encodeURIComponent(sessionRef)}/chunks`, {
      method: 'PATCH', credentials: 'omit',
      headers: { 'content-type': 'application/octet-stream', 'x-linggan-media-offset': String(offset) },
      body: chunk,
    });
    const writtenPayload = await written.json().catch(() => ({}));
    if (!written.ok) throw new Error(writtenPayload.code || 'media_chunk_not_acknowledged');
    const nextOffset = Number(writtenPayload.nextOffset);
    if (!Number.isInteger(nextOffset) || nextOffset <= offset || nextOffset > blob.size) throw new Error('media_chunk_progress_invalid');
    offset = nextOffset;
  }
  const finalized = await fetch(`${LINGGAN_LOCAL_ORIGIN}/api/local/producer/media-uploads/${encodeURIComponent(sessionRef)}/finalize`, {
    method: 'POST', credentials: 'omit',
  });
  const payload = await finalized.json().catch(() => ({}));
  if (!finalized.ok) throw new Error(payload.code || 'media_upload_not_finalized');
  return payload;
}

async function fetchMediaCandidate(candidateUris = []) {
  let lastError = new Error('media_candidate_unavailable');
  for (const candidate of candidateUris.slice(0, 6)) {
    try {
      if (!allowedMediaCandidateUri(candidate)) throw new Error('media_candidate_origin_not_allowed');
      const response = await fetch(candidate, { credentials: 'omit' });
      if (!response.ok) throw new Error(`media_download_http_${response.status}`);
      if (!allowedMediaCandidateUri(response.url || candidate)) throw new Error('media_redirect_origin_not_allowed');
      const mimeType = String(response.headers.get('content-type') || '').split(';')[0].trim();
      if (!mimeType.startsWith('image/') && !mimeType.startsWith('video/') && !mimeType.startsWith('audio/')) throw new Error('media_mime_not_allowed');
      const declaredSize = Number(response.headers.get('content-length'));
      if (Number.isFinite(declaredSize) && declaredSize > MAX_MEDIA_BYTES) throw new Error('media_size_limit_exceeded');
      const bytes = await response.blob();
      if (bytes.size > MAX_MEDIA_BYTES) throw new Error('media_size_limit_exceeded');
      return { bytes, mimeType };
    } catch (error) { lastError = error; }
  }
  throw lastError;
}

async function sha256Hex(arrayBuffer) {
  const bytes = new Uint8Array(await crypto.subtle.digest('SHA-256', arrayBuffer));
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('');
}

async function queueManualDiscovery(discoveryPackage) {
  // Old page readers can still emit the mature surface-card shape while they are all routed
  // through this one runtime. Convert it at the boundary rather than preserving a second
  // Workbench delivery protocol.
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

async function queueCapturePackage({ taskSpec, capturePackage } = {}) {
  // Keep the stable-instance lookup callable.  Naming the local result
  // `producerInstanceId` shadows the helper for this whole block, which turns
  // the first current-surface delivery into a temporal-dead-zone failure.
  const instanceId = await producerInstanceId();
  if (!taskSpec || !capturePackage) {
    throw new Error('linggan_capture_package_required');
  }
  const attempt = createLocalAttempt({ producerInstanceId: instanceId, taskId: taskSpec.taskId });
  const submission = createLocalSubmission({
    producerInstanceId: instanceId,
    taskId: taskSpec.taskId,
    attemptId: attempt.attemptId,
    capturePackage,
  });
  await localProducerOutbox.enqueue({ ...submission, taskSpec, attempt });
  // The capture boundary ends once the durable browser outbox has accepted this envelope.
  // Network delivery happens behind it so a slow or unavailable Linggan service never holds
  // the page-side capture path hostage.
  void flushLocalOutbox();
  return {
    success: true,
    queued: true,
    delivery: 'pending',
    submissionId: submission.submissionId,
    taskId: taskSpec.taskId,
    attemptId: attempt.attemptId,
  };
}

async function queueMediaSlots({ taskSpec, capturePackage } = {}) {
  const queued = await queueCapturePackage({ taskSpec, capturePackage });
  const records = Array.isArray(capturePackage?.records) ? capturePackage.records : [];
  for (const record of records) {
    const candidateUris = Array.isArray(record?.observation?.candidateUris)
      ? record.observation.candidateUris
      : [record?.observation?.externalUri];
    const observationRef = String(record?.observationRef || '').trim();
    const allowedCandidates = candidateUris
      .map((value) => String(value || '').trim())
      .filter(allowedMediaCandidateUri)
      .slice(0, 6);
    if (allowedCandidates.length === 0 || !observationRef) continue;
    await localMediaOutbox.enqueue({
      uploadId: crypto.randomUUID(), slotSubmissionId: queued.submissionId,
      mediaObservationRef: observationRef,
      candidateUris: allowedCandidates,
    });
  }
  void flushLocalOutbox();
  return { ...queued, mediaQueued: records.length, mediaDelivery: 'pending' };
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
  const checkIn = await checkInLingganStation({
    installKey,
    pluginVersion,
    browserLabel: browserLabel(),
    capabilities: await declaredCapabilities(),
    health: readiness.health,
  });
  return {
    success: true,
    registered: checkIn.state === 'claimed' || checkIn.state === 'heartbeat',
    authorized: Boolean(checkIn.checkedIn),
    pluginVersion,
    stationState: checkIn.state || 'unknown',
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

async function scheduleNextClaim(seconds) {
  if (!globalThis.chrome?.alarms?.create) return;
  const minutes = Math.max(MIN_ALARM_MINUTES, Math.round((Number(seconds) || 300) / 60));
  await globalThis.chrome.alarms.create(PATROL_ALARM, { delayInMinutes: minutes });
}

let patrolInFlight = null;
async function patrolTick() {
  if (patrolInFlight) return patrolInFlight;
  patrolInFlight = (async () => {
    const result = await runDispatchedTask().catch(() => null);
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
  const claim = await claimLingganMediaAcquisition({
    installKey,
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
  const candidateUris = claim.candidateUris
    .map((value) => String(value || '').trim())
    .filter(allowedMediaCandidateUri)
    .slice(0, 6);
  if (!claim.workRef || !claim.mediaObservationRef || !Number.isInteger(claim.claimGeneration)
      || claim.claimGeneration < 1 || candidateUris.length === 0) {
    return { success: false, state: 'media_claim_invalid', nextPollAfterSeconds: 300 };
  }
  await localMediaOutbox.enqueue({
    uploadId: `${claim.workRef}:${claim.claimGeneration}`,
    serverWorkRef: claim.workRef,
    claimGeneration: claim.claimGeneration,
    installKey,
    mediaObservationRef: claim.mediaObservationRef,
    candidateUris,
  });
  await flushMediaOutbox();
  return {
    success: true,
    state: 'media_generation_executed',
    executed: true,
    workRef: claim.workRef,
    nextPollAfterSeconds: 0,
  };
}

chrome.alarms?.onAlarm?.addListener((alarm) => {
  if (alarm.name === PATROL_ALARM) void patrolTick();
});

chrome.runtime.onInstalled?.addListener(() => {
  // 安装、升级或开发者重载后立即报到并领活；不再要求用户打开弹窗点击「领取」。
  void checkInStationOnce().then(() => patrolTick());
});
chrome.runtime.onStartup?.addListener(() => {
  void checkInStationOnce().then(() => patrolTick());
});
// service worker 每次被唤醒都会执行到这里：先签到，再领一次。alarm 事件可能紧接着到达，
// `patrolInFlight` 会把两次入口合并为同一个领取请求。
void checkInStationOnce().then(() => patrolTick());

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
const SURFACE_CAPABILITIES = new Set(['author_profile', 'profile_discovery', 'discovery_search']);

async function runDispatchedTask() {
  const readiness = await readLingganLocalReadiness();
  if (!readiness.reachable) {
    return { success: false, state: 'unreachable', message: readiness.message };
  }
  const installKey = await producerInstanceId();
  const claim = await claimLingganDispatch({ installKey, health: readiness.health });
  if (!claim.mayExecute) {
    // 不许执行不是故障：闸门默认关着就是正常状态。原样把服务端的判断带回去。
    return { success: true, state: claim.decision, executed: false, message: claim.message };
  }

  const spec = claim.taskSpec || {};
  const capability = Array.isArray(spec.capabilitiesRequested) ? spec.capabilitiesRequested[0] : '';
  if (!SURFACE_CAPABILITIES.has(capability)) {
    // 无人值守链只执行有界的表层观察。详情、评论与媒体字节需要显式工单，不能由发现结果
    // 自动无限展开。
    return {
      success: true,
      state: 'capability_not_executable_here',
      executed: false,
      message: `派下来的能力「${capability}」当前不在无人值守表层采集范围内。`,
    };
  }

  const authorExternalId = String(spec.target?.authorExternalId || '').trim();
  const query = String(spec.target?.query || '').trim();
  const targetValue = capability === 'discovery_search' ? query : authorExternalId;
  if (!targetValue) {
    return { success: true, state: 'target_incomplete', executed: false, message: '任务没有指明观察目标。' };
  }
  const { windowId, tabId } = await openTaskWindow(capability, targetValue);
  if (!tabId) {
    await closeCollectionWindow(windowId);
    return { success: false, state: 'tab_unavailable', message: '无法打开观察页面。' };
  }

  const action = capability === 'author_profile'
    ? LINGGAN_RUNTIME_ACTION.COLLECT_CURRENT_AUTHOR
    : LINGGAN_RUNTIME_ACTION.DISCOVER_SURFACE;
  try {
    const ready = await waitForTabReady(tabId);
    if (!ready) {
      return { success: false, state: 'page_timeout', message: '观察页面加载超时，本次未采集。' };
    }
    const response = await chrome.tabs.sendMessage(tabId, {
      action,
      mode: capability === 'discovery_search' ? 'search' : 'profile',
      // 配额来自工单，不来自页面对话框：执行端不得自行放宽。
      maximumQuota: Number(spec.maximumQuota) || 1,
      // The page collector must submit against this exact server-issued identity. Rebuilding a
      // manual task here would leave the claimed scheduled task without Attempt or Receipt.
      taskSpec: spec,
      triggerSource: 'linggan_dispatched_task',
    });
    return {
      success: true,
      state: 'executed',
      executed: true,
      capability,
      leaseRef: claim.leaseRef,
      nextPollAfterSeconds: claim.nextPollAfterSeconds,
      message: response?.message || `已按派下来的任务执行「${capability}」。`,
    };
  } catch (error) {
    return { success: false, state: 'page_unavailable', message: '观察页面未能响应，本次未采集。' };
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
 * **平台 ID 才是身份**，URL 由它拼出来，不反过来。
 */
async function openTaskWindow(capability, targetValue) {
  const url = capability === 'discovery_search'
    ? `https://www.xiaohongshu.com/search_result?keyword=${encodeURIComponent(targetValue)}&source=web_explore_feed`
    : `https://www.xiaohongshu.com/user/profile/${encodeURIComponent(targetValue)}`;
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

/// 等页面加载完成。不等就发消息，content script 往往还没注入。
function waitForTabReady(tabId, timeoutMs = 20000) {
  return new Promise((resolve) => {
    let settled = false;
    const finish = (ready) => {
      if (settled) return;
      settled = true;
      chrome.tabs.onUpdated.removeListener(listener);
      resolve(ready);
    };
    function listener(updatedTabId, changeInfo) {
      if (updatedTabId === tabId && changeInfo.status === 'complete') finish(true);
    }
    chrome.tabs.onUpdated.addListener(listener);
    // 超时也要有结论：一个永远不 resolve 的等待会把整个执行挂住。
    setTimeout(() => finish(false), timeoutMs);
  });
}

chrome.runtime.onMessage.addListener((message = {}, _sender, sendResponse) => {
  const action = String(message.action || '').trim();
  Promise.resolve().then(async () => {
    if (action === LINGGAN_RUNTIME_ACTION.TOGGLE_DASHBOARD) return openDashboard();
    if (action === LINGGAN_RUNTIME_ACTION.GET_FLYWHEEL_CONFIG || action === LINGGAN_RUNTIME_ACTION.SAVE_FLYWHEEL_CONFIG) return getLingganStatus();
    if (action === LINGGAN_RUNTIME_ACTION.RUN_DISPATCHED_TASK) return runDispatchedTask();
    if (action === LINGGAN_RUNTIME_ACTION.GET_EXECUTION_STATION_STATUS) {
      return reportStationStatus();
    }
    if (action === LINGGAN_RUNTIME_ACTION.TEST_FLYWHEEL_CONNECTION) return getLingganStatus();
    if (action === LINGGAN_RUNTIME_ACTION.SUBMIT_DISCOVERY_PACKAGE) {
      return queueManualDiscovery(message.discoveryPackage);
    }
    if (action === LINGGAN_RUNTIME_ACTION.SUBMIT_CAPTURE_PACKAGE) {
      return queueCapturePackage({ taskSpec: message.taskSpec, capturePackage: message.capturePackage });
    }
    if (action === LINGGAN_RUNTIME_ACTION.SUBMIT_MEDIA_SLOTS) {
      return queueMediaSlots({ taskSpec: message.taskSpec, capturePackage: message.capturePackage });
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
