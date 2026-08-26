import {
  LINGGAN_LOCAL_ORIGIN,
  attemptStartIsAccepted,
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
    const dependency = await localProducerOutbox.get(upload.slotSubmissionId);
    if (!dependency || dependency.status === 'terminal') {
      await localMediaOutbox.terminal(upload.uploadId, 'media_slot_delivery_not_accepted');
      continue;
    }
    if (dependency.status !== 'acknowledged') continue;
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
      void recordMediaDownloadFailure(upload, error).catch(() => {});
      await localMediaOutbox.retry(upload.uploadId, error?.message || error);
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
  await fetch(`${LINGGAN_LOCAL_ORIGIN}/api/local/producer/media-observations/${encodeURIComponent(observationRef)}/download-failures`, {
    method: 'POST', credentials: 'omit', headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ attemptedUri, terminalReason }),
  });
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
      const response = await fetch(candidate, { credentials: 'omit' });
      if (!response.ok) throw new Error(`media_download_http_${response.status}`);
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
  const producerInstanceId = await producerInstanceId();
  if (!taskSpec || !capturePackage) {
    throw new Error('linggan_capture_package_required');
  }
  const attempt = createLocalAttempt({ producerInstanceId, taskId: taskSpec.taskId });
  const submission = createLocalSubmission({
    producerInstanceId,
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
    if (!candidateUris.some((value) => String(value || '').trim()) || !observationRef) continue;
    await localMediaOutbox.enqueue({
      uploadId: crypto.randomUUID(), slotSubmissionId: queued.submissionId,
      mediaObservationRef: observationRef,
      candidateUris: candidateUris.map((value) => String(value || '').trim()).filter(Boolean).slice(0, 6),
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

chrome.runtime.onMessage.addListener((message = {}, _sender, sendResponse) => {
  const action = String(message.action || '').trim();
  Promise.resolve().then(async () => {
    if (action === LINGGAN_RUNTIME_ACTION.TOGGLE_DASHBOARD) return openDashboard();
    if (action === LINGGAN_RUNTIME_ACTION.GET_FLYWHEEL_CONFIG || action === LINGGAN_RUNTIME_ACTION.SAVE_FLYWHEEL_CONFIG) return getLingganStatus();
    if (action === LINGGAN_RUNTIME_ACTION.GET_EXECUTION_STATION_STATUS) {
      const readiness = await readLingganLocalReadiness();
      return {
        success: true,
        registered: false,
        authorized: false,
        pluginVersion: chrome.runtime.getManifest().version,
        authorizationMessage: `${readiness.message} 当前测试模式为本机直连；不使用旧工位或授权机制。`,
      };
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
