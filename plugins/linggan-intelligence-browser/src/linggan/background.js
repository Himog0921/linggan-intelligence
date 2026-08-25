import {
  LINGGAN_LOCAL_ORIGIN,
  attemptStartIsAccepted,
  createLocalAttempt,
  createLocalSubmission,
  createManualTaskSpec,
  createLingganPendingResult,
  isTerminalLocalDeliveryResult,
  localPost,
  readLingganLocalReadiness,
  taskCreationIsAccepted,
  unavailableLingganStats,
} from './adapter.js';
import { LINGGAN_RUNTIME_ACTION } from './runtimeActions.js';
import { localProducerOutbox } from './localProducerOutbox.js';

const PRODUCER_INSTANCE_KEY = 'linggan.localTrusted.producerInstanceId';

async function producerInstanceId() {
  const stored = await chrome.storage.local.get(PRODUCER_INSTANCE_KEY);
  const current = String(stored[PRODUCER_INSTANCE_KEY] || '').trim();
  if (current) return current;
  const created = crypto.randomUUID();
  await chrome.storage.local.set({ [PRODUCER_INSTANCE_KEY]: created });
  return created;
}

async function notifyDelivery(tabId, payload) {
  if (!Number.isInteger(tabId)) return;
  try {
    await chrome.tabs.sendMessage(tabId, {
      action: LINGGAN_RUNTIME_ACTION.DISCOVERY_DELIVERY_UPDATE,
      ...payload,
    });
  } catch {
    // The tab may have navigated or been closed. Its durable outbox row remains the recovery
    // source; lack of a page toast never changes submission or admission state.
  }
}

async function flushLocalOutbox() {
  const due = await localProducerOutbox.due({ limit: 5 });
  for (const entry of due) {
    await localProducerOutbox.markInFlight(entry.submissionId);
    await notifyDelivery(entry.originTabId, {
      submissionId: entry.submissionId,
      deliveryState: 'delivering',
      localCoverage: entry.localCoverage,
    });
    try {
      const task = await localPost('/api/local/producer/manual-tasks', entry.taskSpec);
      if (!taskCreationIsAccepted(task)) {
        if (isTerminalLocalDeliveryResult(task)) {
          await localProducerOutbox.terminal(entry.submissionId, task.payload.code || 'task_not_created');
          await notifyDelivery(entry.originTabId, { submissionId: entry.submissionId, deliveryState: 'rejected', code: task.payload.code || 'task_not_created', localCoverage: entry.localCoverage });
          continue;
        }
        throw new Error(task.payload.code || 'task_not_created');
      }
      const attempt = await localPost('/api/local/producer/attempts', entry.attempt);
      if (!attemptStartIsAccepted(attempt)) {
        if (isTerminalLocalDeliveryResult(attempt)) {
          await localProducerOutbox.terminal(entry.submissionId, attempt.payload.code || 'attempt_not_started');
          await notifyDelivery(entry.originTabId, { submissionId: entry.submissionId, deliveryState: 'rejected', code: attempt.payload.code || 'attempt_not_started', localCoverage: entry.localCoverage });
          continue;
        }
        throw new Error(attempt.payload.code || 'attempt_not_started');
      }
      const submitted = await localPost('/api/local/producer/submissions', {
        contractVersion: entry.contractVersion,
        producerInstanceId: entry.producerInstanceId,
        taskId: entry.taskId,
        attemptId: entry.attemptId,
        submissionId: entry.submissionId,
        discoveryPackage: entry.discoveryPackage,
      });
      if (submitted.ok && ['acknowledged', 'replay'].includes(submitted.payload.delivery)) {
        await localProducerOutbox.acknowledge(entry.submissionId, submitted.payload);
        await notifyDelivery(entry.originTabId, {
          submissionId: entry.submissionId,
          deliveryState: submitted.payload.delivery === 'replay' ? 'acknowledged_replay' : 'acknowledged',
          admission: submitted.payload.admission,
          localCoverage: entry.localCoverage,
        });
      } else if (submitted.status >= 400 && submitted.status < 500) {
        await localProducerOutbox.terminal(entry.submissionId, submitted.payload.code);
        await notifyDelivery(entry.originTabId, {
          submissionId: entry.submissionId,
          deliveryState: submitted.status === 409 ? 'conflict' : 'rejected',
          code: submitted.payload.code || 'submission_rejected',
          localCoverage: entry.localCoverage,
        });
      } else {
        await localProducerOutbox.retry(entry.submissionId, submitted.payload.code || 'submission_not_acknowledged');
        await notifyDelivery(entry.originTabId, { submissionId: entry.submissionId, deliveryState: 'queued_locally', code: submitted.payload.code || 'submission_not_acknowledged', localCoverage: entry.localCoverage });
      }
    } catch (error) {
      await localProducerOutbox.retry(entry.submissionId, error?.message || error);
      await notifyDelivery(entry.originTabId, { submissionId: entry.submissionId, deliveryState: 'queued_locally', code: String(error?.message || error), localCoverage: entry.localCoverage });
    }
  }
  return { pending: await localProducerOutbox.pendingCount() };
}

async function queueManualDiscovery(discoveryPackage, { originTabId = null, localCoverage = null } = {}) {
  const producerInstanceId = await producerInstanceId();
  const taskSpec = createManualTaskSpec();
  const attempt = createLocalAttempt({ producerInstanceId, taskId: taskSpec.taskId });
  const submission = createLocalSubmission({
    producerInstanceId,
    taskId: taskSpec.taskId,
    attemptId: attempt.attemptId,
    discoveryPackage,
  });
  await localProducerOutbox.enqueue({ ...submission, taskSpec, attempt, originTabId, localCoverage });
  // The capture boundary ends once the durable browser outbox has accepted this envelope.
  // Network delivery happens behind it so a slow or unavailable Linggan service never holds
  // the page-side capture path hostage.
  void flushLocalOutbox();
  return {
    success: true,
    queued: true,
    deliveryState: 'queued_locally',
    submissionId: submission.submissionId,
  };
}

async function openDashboard() {
  await chrome.tabs.create({ url: chrome.runtime.getURL('dashboard.html') });
  return { success: true };
}

async function getLingganStatus() {
  const readiness = await readLingganLocalReadiness();
  return {
    success: true,
    serverUrl: LINGGAN_LOCAL_ORIGIN,
    enabled: true,
    mode: 'linggan_adapter_pending',
    readiness,
  };
}

chrome.runtime.onMessage.addListener((message = {}, sender, sendResponse) => {
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
        authorizationMessage: `${readiness.message} ${createLingganPendingResult('station_dispatch').message}`,
      };
    }
    if (action === LINGGAN_RUNTIME_ACTION.TEST_FLYWHEEL_CONNECTION) return getLingganStatus();
    if (action === LINGGAN_RUNTIME_ACTION.SUBMIT_DISCOVERY_PACKAGE) {
      return queueManualDiscovery(message.discoveryPackage, {
        originTabId: sender?.tab?.id ?? null,
        localCoverage: message.localCoverage || null,
      });
    }
    if (action === LINGGAN_RUNTIME_ACTION.FLUSH_LOCAL_OUTBOX) return flushLocalOutbox();
    if (action === LINGGAN_RUNTIME_ACTION.CREATE_MANUAL_TASK) {
      const producerInstanceId = await producerInstanceId();
      const taskSpec = createManualTaskSpec();
      const attempt = createLocalAttempt({ producerInstanceId, taskId: taskSpec.taskId });
      return { success: true, producerInstanceId, taskSpec, attempt, scheduler: 'NOT_CONNECTED' };
    }
    if (action === LINGGAN_RUNTIME_ACTION.GET_STATS) return unavailableLingganStats();
    return createLingganPendingResult(action);
  }).then(sendResponse).catch((error) => {
    sendResponse({ success: false, code: 'linggan_adapter_error', message: String(error?.message || error) });
  });
  return true;
});
