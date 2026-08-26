export const LINGGAN_LOCAL_ORIGIN = 'http://localhost:3000';
const TASK_SPEC_VERSION = 'linggan.task-spec.v1';
const ATTEMPT_VERSION = 'linggan.producer.attempt.v1';
const SUBMISSION_VERSION = 'linggan.producer.capture-package.v1';
const APPROVED_LOCAL_PRODUCER_READINESS_VARIANTS = Object.freeze([
  Object.freeze({
    dataState: 'LINGGAN_BROWSER_PRODUCER_RUNTIME',
    schema: 'PLUGIN_RUNTIME_001_SCHEMA_READY',
  }),
  Object.freeze({
    dataState: 'LOCAL_TRUSTED_PRODUCER',
    schema: 'LOCAL_003_SCHEMA_READY',
  }),
]);

export function formatLingganRuntimeNotice() {
  return 'Linggan 本机执行端已启用：页面采集结果会先写入本机可靠队列，再由 Linggan 接纳。自动调度尚未启动。';
}

function producerRoute(value) {
  const path = String(value || '').trim();
  return path.startsWith('/api/local/producer/') && !/[?#]/.test(path) ? path : null;
}

function producerRoutesFromHealth(health) {
  const routes = health?.routes?.localProducer;
  if (!routes || typeof routes !== 'object' || Array.isArray(routes)) return null;
  const taskCreation = producerRoute(routes.taskCreation);
  const attemptStart = producerRoute(routes.attemptStart);
  const submission = producerRoute(routes.submission);
  return taskCreation && attemptStart && submission
    ? { taskCreation, attemptStart, submission }
    : null;
}

function isApprovedLocalProducerReadiness(health) {
  return APPROVED_LOCAL_PRODUCER_READINESS_VARIANTS.some(({ dataState, schema }) => (
    health?.dataState === dataState && health?.database?.schema === schema
  ));
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
    const health = await response.json().catch(() => null);
    const producerRoutes = producerRoutesFromHealth(health);
    const ready = health?.service === 'linggan-local-web'
      && health?.listener === 'loopback-only'
      && health?.database?.state === 'READY'
      && isApprovedLocalProducerReadiness(health)
      && producerRoutes;
    if (!ready) {
      return {
        connected: false,
        reachable: false,
        message: 'Linggan 本机服务可访问，但当前不是可接收本机 Producer 采集包的运行状态。',
      };
    }
    return {
      connected: true,
      reachable: true,
      producerRoutes,
      message: `Linggan 本机服务可访问；${health.dataState} 已就绪。`,
    };
  } catch {
    return { connected: false, message: 'Linggan 本机服务当前不可访问。' };
  }
}

export function createManualTaskSpec({ taskId = crypto.randomUUID() } = {}) {
  return createTaskSpec({
    taskId, source: 'manual', platform: 'xhs', pageType: 'search_results',
    target: { query: '__manual_placeholder__', surface: 'current_visible_search_surface' }, capabilitiesRequested: ['discovery_search'],
    maximumQuota: 20, commentLimit: 'not_requested', acquireMedia: 'not_requested',
    riskPolicy: 'local_trusted_user_initiated', stopConditions: ['current_surface_read_once', 'maximum_quota'],
  });
}

export function createTaskSpec({
  taskId = crypto.randomUUID(), source = 'manual', platform, pageType, target,
  capabilitiesRequested, maximumQuota = null, commentLimit = 'not_requested',
  acquireMedia = 'not_requested', riskPolicy = 'local_trusted_user_initiated', stopConditions = [],
} = {}) {
  const value = {
    contractVersion: 'linggan.producer.task-spec.v1', taskId, source, platform, pageType,
    target: target && typeof target === 'object' && !Array.isArray(target) ? target : {},
    capabilitiesRequested: Array.isArray(capabilitiesRequested) ? capabilitiesRequested : [],
    maximumQuota: Number.isInteger(maximumQuota) && maximumQuota > 0 ? maximumQuota : null,
    commentLimit, acquireMedia, riskPolicy,
    stopConditions: Array.isArray(stopConditions) ? stopConditions : [],
  };
  validateTaskSpec(value);
  return value;
}

const CAPABILITIES = new Set([
  'discovery_search', 'profile_discovery', 'content_detail', 'comments', 'replies',
  'author_profile', 'media_slots', 'media_bytes', 'batch_checkpoint',
]);
const STOP_CONDITIONS = new Set([
  'manual_stop', 'maximum_quota', 'current_surface_read_once', 'surface_ended',
  'time_budget', 'risk_budget', 'detail_read_complete', 'collector_complete',
]);

// The browser must reject an ambiguous instruction before it creates an outbox envelope.  This
// is intentionally closed rather than a "reasonable defaults" parser: a producer may execute
// mechanics, but may not infer what Linggan meant to collect.
export function validateTaskSpec(spec = {}) {
  if (!['manual', 'scheduled'].includes(spec.source) || !['xhs', 'douyin'].includes(spec.platform)) throw new Error('task_spec_source_or_platform_invalid');
  if (!spec.target || typeof spec.target !== 'object' || Array.isArray(spec.target)) throw new Error('task_spec_target_invalid');
  const capabilities = Array.isArray(spec.capabilitiesRequested) ? spec.capabilitiesRequested : [];
  if (capabilities.length !== 1 || !CAPABILITIES.has(capabilities[0])) throw new Error('task_spec_capability_lane_invalid');
  const capability = capabilities[0];
  const target = spec.target;
  const requireText = (key) => { if (!String(target[key] || '').trim()) throw new Error(`task_spec_target_${key}_required`); };
  if (capability === 'discovery_search') requireText('query');
  if (capability === 'profile_discovery' || capability === 'author_profile') requireText('authorExternalId');
  if (['content_detail', 'comments', 'replies', 'media_slots', 'media_bytes'].includes(capability)) requireText('contentExternalId');
  if (capability === 'batch_checkpoint') requireText('taskType');
  if (!(spec.commentLimit === 'not_requested' || (Number.isInteger(spec.commentLimit) && spec.commentLimit > 0))) throw new Error('task_spec_comment_limit_invalid');
  if (!['not_requested', 'slots', 'bytes'].includes(spec.acquireMedia)) throw new Error('task_spec_acquire_media_invalid');
  if (spec.riskPolicy !== 'local_trusted_user_initiated') throw new Error('task_spec_risk_policy_invalid');
  if (!Array.isArray(spec.stopConditions) || spec.stopConditions.length === 0 || spec.stopConditions.some((value) => !STOP_CONDITIONS.has(value))) throw new Error('task_spec_stop_conditions_invalid');
  if (spec.maximumQuota !== null && (!Number.isInteger(spec.maximumQuota) || spec.maximumQuota <= 0)) throw new Error('task_spec_maximum_quota_invalid');
  return spec;
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
