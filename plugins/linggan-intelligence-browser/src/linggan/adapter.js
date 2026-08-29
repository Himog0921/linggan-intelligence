export const LINGGAN_LOCAL_ORIGIN = 'http://localhost:3000';
const TASK_SPEC_VERSION = 'linggan.task-spec.v1';
const ATTEMPT_VERSION = 'linggan.producer.attempt.v1';
const SUBMISSION_VERSION = 'linggan.producer.capture-package.v1';
const FULL_LOCAL_PRODUCER_READINESS = Object.freeze({
  dataState: 'LINGGAN_BROWSER_PRODUCER_RUNTIME',
  schema: 'PLUGIN_RUNTIME_001_SCHEMA_READY',
});

export function formatLingganRuntimeNotice() {
  return 'Linggan 本机执行端已启用：系统侧有等待任务时会自动领取，采集结果先写入本机可靠队列，再由 Linggan 接纳。';
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

function isFullLocalProducerReadiness(health) {
  return health?.dataState === FULL_LOCAL_PRODUCER_READINESS.dataState
    && health?.database?.schema === FULL_LOCAL_PRODUCER_READINESS.schema;
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
    const localServiceReachable = health?.service === 'linggan-local-web'
      && health?.listener === 'loopback-only'
      && health?.database?.state === 'READY';
    if (!localServiceReachable) {
      return {
        connected: false,
        reachable: false,
        health,
        message: 'Linggan 本机服务可访问，但当前不是可接收本机 Producer 采集包的运行状态。',
      };
    }
    const producerRoutes = producerRoutesFromHealth(health);
    if (!isFullLocalProducerReadiness(health) || !producerRoutes) {
      return {
        connected: false,
        reachable: true,
        deliveryReady: false,
        health,
        message: 'Linggan 本机服务可访问，但尚未升级到可接收完整 Producer 采集包的运行状态；未发送任何采集包。',
      };
    }
    return {
      connected: true,
      reachable: true,
      deliveryReady: true,
      producerRoutes,
      health,
      message: `Linggan 本机服务可访问；${health.dataState} 已就绪。`,
    };
  } catch {
    return { connected: false, message: 'Linggan 本机服务当前不可访问。' };
  }
}


/// 工位报到路由由 /health 通告，插件不写死路径——与 producer 路由同一套约定。
function stationCheckInRoute(health) {
  const path = String(health?.routes?.station?.checkIn || '').trim();
  return path.startsWith('/api/local/stations/') && !/[?#]/.test(path) ? path : null;
}

export function stationCheckInRouteFromHealth(health) {
  return stationCheckInRoute(health);
}

/**
 * 向 Linggan 报到。
 *
 * 报的是「这次安装」，不是「这台工位」：installKey 存在 chrome.storage.local，重装即换。
 * 工位由人在 Linggan 里登记，插件不创建也不选择工位——它只说明自己是谁、能做什么。
 * 服务端据此决定自动归位、还是挂进待认领。
 */
export async function checkInLingganStation({
  installKey,
  pluginVersion,
  browserLabel = '',
  capabilities = [],
  origin = LINGGAN_LOCAL_ORIGIN,
  fetchImpl = globalThis.fetch,
  health = null,
} = {}) {
  if (typeof fetchImpl !== 'function') {
    return { checkedIn: false, message: '浏览器当前无法连接 Linggan 本机服务。' };
  }
  if (!String(installKey || '').trim()) {
    return { checkedIn: false, message: '本次安装还没有本机标识，无法报到。' };
  }
  const route = stationCheckInRoute(health);
  if (!route) {
    // 服务端没通告这个路由，说明工位表还没建好。此时报到必然失败，不如不发。
    return {
      checkedIn: false,
      message: 'Linggan 本机服务尚未开放工位报到；未发送任何报到请求。',
    };
  }
  try {
    const response = await fetchImpl(`${origin}${route}`, {
      method: 'POST',
      credentials: 'omit',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        installKey,
        pluginVersion,
        browserLabel,
        capabilities: Array.isArray(capabilities) ? capabilities : [],
      }),
    });
    if (!response.ok) {
      return { checkedIn: false, message: `Linggan 本机服务返回 ${response.status}。` };
    }
    const body = await response.json().catch(() => null);
    const state = String(body?.state || '').trim();
    return {
      checkedIn: true,
      state,
      installationRef: String(body?.installationRef || '').trim(),
      stationRef: String(body?.stationRef || '').trim(),
      message: stationStateMessage(state),
    };
  } catch {
    return { checkedIn: false, message: 'Linggan 本机服务当前不可访问。' };
  }
}

/// 三种状态对使用者意味着完全不同的事，不能都说成「已连接」。
function stationStateMessage(state) {
  if (state === 'claimed') return '已报到并归位到一台工位。';
  if (state === 'heartbeat') return '已报到；本次安装此前已归位。';
  if (state === 'awaiting_claim') {
    return '已报到，但还没有归位到任何工位。请在 Linggan 的采集 → 执行工位页认领它，或先开一个认领窗口。';
  }
  return '已报到。';
}

/// 派发路由同样由 /health 通告，插件不写死。
export function dispatchClaimRouteFromHealth(health) {
  const path = String(health?.routes?.dispatch?.claim || '').trim();
  return path.startsWith('/api/local/dispatch/') && !/[?#]/.test(path) ? path : null;
}

/**
 * 问 Linggan：现在有我能做的活吗？
 *
 * **只认 `mayExecute`**。服务端可能返回任务体的同时并不允许执行（例如闸门关着），
 * 拿到任务体不等于拿到许可；凭「有没有 taskSpec」判断会绕过整条授权链。
 */
export async function claimLingganDispatch({
  installKey,
  origin = LINGGAN_LOCAL_ORIGIN,
  fetchImpl = globalThis.fetch,
  health = null,
} = {}) {
  if (typeof fetchImpl !== 'function') {
    return { mayExecute: false, decision: 'unavailable', message: '浏览器当前无法连接 Linggan。', nextPollAfterSeconds: 900 };
  }
  const route = dispatchClaimRouteFromHealth(health);
  if (!route) {
    return { mayExecute: false, decision: 'unavailable', message: 'Linggan 本机服务尚未开放任务派发。', nextPollAfterSeconds: 900 };
  }
  try {
    const response = await fetchImpl(`${origin}${route}`, {
      method: 'POST',
      credentials: 'omit',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ installKey }),
    });
    if (!response.ok) {
      return { mayExecute: false, decision: 'unavailable', message: `Linggan 返回 ${response.status}。`, nextPollAfterSeconds: 900 };
    }
    const body = await response.json().catch(() => null);
    const mayExecute = body?.mayExecute === true;
    return {
      mayExecute,
      decision: String(body?.decision || 'unknown'),
      // 不许执行时**不把任务体带出去**：留着它只会让下游有机会「反正拿到了就跑」。
      taskSpec: mayExecute ? body?.taskSpec ?? null : null,
      leaseRef: mayExecute ? String(body?.leaseRef || '') : '',
      message: String(body?.reason || ''),
      // 节奏由服务端给。插件不自定间隔——否则想调就得重新发一版插件。
      nextPollAfterSeconds: Number(body?.nextPollAfterSeconds ?? 300),
    };
  } catch {
    // 连不上时退避得久一些：Linggan 没开着是常态，不该每分钟敲一次。
    return { mayExecute: false, decision: 'unavailable', message: 'Linggan 本机服务当前不可访问。', nextPollAfterSeconds: 900 };
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
  // 风险策略与来源必须配对，与服务端同一条规则：
  // manual = 有人在键盘前点的、看着它跑；scheduled = 服务端在一份有到期时间的租约内授权。
  // 双向校验缺一不可——只放开 scheduled 而不禁止 manual 用服务端策略，本插件就能自签
  // 一份「服务端已授权」的任务。
  const expectedRiskPolicy = spec.source === 'scheduled'
    ? 'server_authorized_leased'
    : 'local_trusted_user_initiated';
  if (spec.riskPolicy !== expectedRiskPolicy) throw new Error('task_spec_risk_policy_invalid');
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
