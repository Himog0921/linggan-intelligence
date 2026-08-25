export const LINGGAN_LOCAL_ORIGIN = 'http://localhost:3000';
const TASK_SPEC_VERSION = 'linggan.task-spec.v1';
const ATTEMPT_VERSION = 'linggan.producer.attempt.v1';
const SUBMISSION_VERSION = 'linggan.producer.capture-package.v1';

export function formatLingganRuntimeNotice() {
  return 'Linggan 本机执行端已启用：页面采集结果会先写入本机可靠队列，再由 Linggan 接纳。自动调度尚未启动。';
}

export async function readLingganLocalReadiness(fetchImpl = globalThis.fetch) {
  if (typeof fetchImpl !== 'function') {
    return { connected: false, message: '浏览器当前无法检查 Linggan 本机服务。' };
  }
  try {
    const response = await fetchImpl(`${LINGGAN_LOCAL_ORIGIN}/health`, {
      method: 'GET',
      credentials: 'omit',
    });
    if (!response.ok) {
      return { connected: false, message: `Linggan 本机服务返回 ${response.status}。` };
    }
    return { connected: true, message: 'Linggan 本机服务可访问；Browser Producer Runtime 已可交付采集包。' };
  } catch {
    return { connected: false, message: 'Linggan 本机服务当前不可访问。' };
  }
}

export function createManualTaskSpec({ taskId = crypto.randomUUID() } = {}) {
  return createTaskSpec({
    taskId, source: 'manual', platform: 'xhs', pageType: 'search_results',
    target: { surface: 'current_visible_search_surface' }, capabilitiesRequested: ['discovery_search'],
    maximumQuota: 20, commentLimit: 'not_requested', acquireMedia: 'not_requested',
    riskPolicy: 'local_trusted_user_initiated', stopConditions: ['current_surface_read_once', 'maximum_quota'],
  });
}

export function createTaskSpec({
  taskId = crypto.randomUUID(), source = 'manual', platform, pageType, target,
  capabilitiesRequested, maximumQuota = null, commentLimit = 'not_requested',
  acquireMedia = 'not_requested', riskPolicy = 'local_trusted_user_initiated', stopConditions = [],
} = {}) {
  return {
    contractVersion: 'linggan.producer.task-spec.v1', taskId, source, platform, pageType,
    target: target && typeof target === 'object' && !Array.isArray(target) ? target : {},
    capabilitiesRequested: Array.isArray(capabilitiesRequested) ? capabilitiesRequested : [],
    maximumQuota: Number.isInteger(maximumQuota) && maximumQuota > 0 ? maximumQuota : null,
    commentLimit, acquireMedia, riskPolicy,
    stopConditions: Array.isArray(stopConditions) ? stopConditions : [],
  };
}

export function createLocalAttempt({ producerInstanceId, taskId, attemptId = crypto.randomUUID() } = {}) {
  return { contractVersion: ATTEMPT_VERSION, producerInstanceId, taskId, attemptId };
}

export function createLocalSubmission({ producerInstanceId, taskId, attemptId, capturePackage, discoveryPackage, submissionId = crypto.randomUUID() } = {}) {
  // `discoveryPackage` is deliberately accepted only as a compatibility input while callers move
  // to the typed shared runtime. It is wrapped as an explicit discovery package, never silently
  // treated as a completed detail/comment/media capture.
  const packageValue = capturePackage || discoveryPackage;
  return { contractVersion: SUBMISSION_VERSION, producerInstanceId, taskId, attemptId, submissionId, capturePackage: packageValue };
}

export async function localPost(path, body, fetchImpl = globalThis.fetch) {
  const response = await fetchImpl(`${LINGGAN_LOCAL_ORIGIN}${path}`, {
    method: 'POST', credentials: 'omit', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body),
  });
  const payload = await response.json().catch(() => ({}));
  return { ok: response.ok, status: response.status, payload };
}

export function taskCreationIsAccepted(result) {
  return result?.ok && ['created', 'replay'].includes(result.payload?.outcome);
}

export function attemptStartIsAccepted(result) {
  return result?.ok && ['started', 'replay'].includes(result.payload?.outcome);
}

export function isTerminalLocalDeliveryResult(result) {
  return (result?.status >= 400 && result.status < 500) || result?.payload?.outcome === 'conflict';
}

export function unavailableLingganStats(statsState = 'not_connected') {
  return {
    success: true,
    statsState,
    notes: null,
    comments: null,
    authors: null,
    source: 'linggan_data_not_read',
  };
}
