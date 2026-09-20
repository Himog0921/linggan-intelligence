export const LINGGAN_LOCAL_ORIGIN = 'http://localhost:3000';
const TASK_SPEC_VERSION = 'linggan.producer.task-spec.v1';
const ATTEMPT_VERSION = 'linggan.producer.attempt.v1';
const SUBMISSION_VERSION = 'linggan.producer.capture-package.v1';
const FULL_LOCAL_PRODUCER_READINESS = Object.freeze({
  dataState: 'LINGGAN_BROWSER_PRODUCER_RUNTIME',
  schema: 'PLUGIN_RUNTIME_002_SCHEMA_READY',
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
  const mediaAcquisitionClaim = producerRoute(routes.mediaAcquisitionClaim);
  return taskCreation && attemptStart && submission && mediaAcquisitionClaim
    ? { taskCreation, attemptStart, submission, mediaAcquisitionClaim }
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
  installationCredential = '',
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
        installationCredential: String(installationCredential || '').trim() || null,
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
      // The display name is server-confirmed presentation data, never a local
      // identity or a claim key. It lets the matched plugin panel show the
      // exact same durable station name as Collection Runtime.
      stationDisplayName: String(body?.stationDisplayName || '').trim(),
      stationAccepting: typeof body?.stationAccepting === 'boolean'
        ? body.stationAccepting
        : null,
      installationCredentialRef: String(body?.installationCredentialRef || '').trim(),
      // This value is consumed immediately by the MV3 background worker and is never included
      // in a notice, UI response, log or durable outbox entry.
      installationCredential: String(body?.installationCredential || '').trim(),
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

/// The failure route belongs to the same local dispatch contract as claim.
/// Do not infer it from the claim path: the server advertises exactly which
/// route its currently applied schema can accept.
export function dispatchFailureRouteFromHealth(health) {
  const path = String(health?.routes?.dispatch?.failure || '').trim();
  return path.startsWith('/api/local/dispatch/') && !/[?#]/.test(path) ? path : null;
}

export function detailPageSessionGrantRouteFromHealth(health) {
  const path = String(health?.routes?.dispatch?.detailPageSessionGrant || '').trim();
  return path.startsWith('/api/local/dispatch/') && !/[?#]/.test(path) ? path : null;
}

export function detailPageSessionNavigationRouteFromHealth(health) {
  const path = String(health?.routes?.dispatch?.detailPageSessionNavigation || '').trim();
  return path.startsWith('/api/local/dispatch/') && !/[?#]/.test(path) ? path : null;
}

export function detailPageRiskSignalRouteFromHealth(health) {
  const path = String(health?.routes?.dispatch?.detailPageRiskSignal || '').trim();
  return path.startsWith('/api/local/dispatch/') && !/[?#]/.test(path) ? path : null;
}

export async function reportLingganDetailPageRiskSignal({
  installKey, installationCredential, taskId, riskSignalId, detectorVersion,
  origin = LINGGAN_LOCAL_ORIGIN, fetchImpl = globalThis.fetch, health = null,
} = {}) {
  const route = detailPageRiskSignalRouteFromHealth(health);
  if (typeof fetchImpl !== 'function' || !route || !String(installKey || '').trim()
      || !String(installationCredential || '').trim() || !String(taskId || '').trim()
      || !String(riskSignalId || '').trim() || !String(detectorVersion || '').trim()) {
    return { reported: false, reasonCode: 'risk_signal_route_unavailable' };
  }
  try {
    const response = await fetchImpl(`${origin}${route}`, {
      method: 'POST', credentials: 'omit', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ installKey, installationCredential, taskId, riskSignalId, detectorVersion }),
    });
    const body = await response.json().catch(() => null);
    return response.ok && body?.outcome === 'recorded'
      ? { reported: true, cooldownActive: body.cooldownActive === true, cooldownUntil: body.cooldownUntil || null }
      : { reported: false, reasonCode: String(body?.code || 'risk_signal_rejected') };
  } catch {
    return { reported: false, reasonCode: 'risk_signal_transport_unavailable' };
  }
}

/**
 * Obtain a detail-page authorization using the browser-persisted request id.
 * Repeating the same id may recover a lost response; a new id is never a
 * license to reset an existing Work Order/material navigation boundary.
 */
export async function grantLingganDetailPageSession({
  installKey,
  installationCredential,
  taskId,
  grantRequestId,
  origin = LINGGAN_LOCAL_ORIGIN,
  fetchImpl = globalThis.fetch,
  health = null,
} = {}) {
  const route = detailPageSessionGrantRouteFromHealth(health);
  if (typeof fetchImpl !== 'function' || !route
      || !String(installKey || '').trim() || !String(installationCredential || '').trim()
      || !String(taskId || '').trim() || !String(grantRequestId || '').trim()) {
    return { granted: false, outcome: 'unavailable', reasonCode: 'grant_route_unavailable' };
  }
  try {
    const response = await fetchImpl(`${origin}${route}`, {
      method: 'POST', credentials: 'omit', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ installKey, installationCredential, taskId, grantRequestId }),
    });
    const body = await response.json().catch(() => null);
    if (!response.ok) {
      return { granted: false, outcome: 'rejected', reasonCode: String(body?.code || 'grant_rejected') };
    }
    const outcome = String(body?.outcome || '');
    const sessionRef = String(body?.sessionRef || '').trim();
    if (['authorized', 'replay'].includes(outcome) && sessionRef
        && body?.pageSessionPlan && typeof body.pageSessionPlan === 'object'
        && !Array.isArray(body.pageSessionPlan)) {
      return { granted: true, outcome, sessionRef, pageSessionPlan: body.pageSessionPlan };
    }
    return { granted: false, outcome: outcome || 'invalid', sessionRef, reasonCode: String(body?.reasonCode || 'grant_contract_invalid') };
  } catch {
    return { granted: false, outcome: 'unavailable', reasonCode: 'grant_transport_unavailable' };
  }
}

/**
 * Report a Chrome-observed, extension-managed tab after the durable local
 * consumption record has committed. This call is observational only: a
 * transport failure must not trigger another page open.
 */
export async function reportLingganDetailPageSessionNavigation({
  installKey,
  installationCredential,
  taskId,
  sessionRef,
  kind = 'navigation_observed',
  stopReason = null,
  origin = LINGGAN_LOCAL_ORIGIN,
  fetchImpl = globalThis.fetch,
  health = null,
} = {}) {
  const route = detailPageSessionNavigationRouteFromHealth(health);
  if (typeof fetchImpl !== 'function' || !route
      || !String(installKey || '').trim() || !String(installationCredential || '').trim()
      || !String(taskId || '').trim() || !String(sessionRef || '').trim()) {
    return false;
  }
  try {
    const response = await fetchImpl(`${origin}${route}`, {
      method: 'POST', credentials: 'omit', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ installKey, installationCredential, taskId, sessionRef, kind, stopReason }),
    });
    const body = await response.json().catch(() => null);
    return response.ok && body?.outcome === 'recorded';
  } catch {
    return false;
  }
}

export function accountEligibilityRouteFromHealth(health) {
  const path = String(health?.routes?.station?.eligibilityReport || '').trim();
  return path.startsWith('/api/local/stations/') && !/[?#]/.test(path) ? path : null;
}

/**
 * The server tells a newly installed producer whether account observation is usable without
 * disclosing the secret that keys account identities. A missing identity key prevents positive
 * identity reports, but explicit login, cooling, or restriction facts remain reportable and
 * authoritative; transport or DOM uncertainty never turns a claimed task into an account block.
 */
export function accountObservationAvailabilityFromHealth(health) {
  const availability = String(health?.routes?.station?.accountObservation || '').trim();
  return ['ready', 'schema_unavailable', 'identity_key_missing'].includes(availability)
    ? availability
    : 'unavailable';
}

export function credentialActivationRouteFromHealth(health) {
  const path = String(health?.routes?.station?.credentialActivation || '').trim();
  return path.startsWith('/api/local/stations/') && !/[?#]/.test(path) ? path : null;
}

/**
 * Acknowledge a pending, hash-only credential only after chrome.storage.local has durably stored
 * the one-time value. The service keeps a previous active credential usable until this succeeds.
 */
export async function activateLingganInstallationCredential({
  installationRef,
  credentialRef,
  installationCredential,
  origin = LINGGAN_LOCAL_ORIGIN,
  fetchImpl = globalThis.fetch,
  health = null,
} = {}) {
  const route = credentialActivationRouteFromHealth(health);
  if (typeof fetchImpl !== 'function'
      || !route
      || !String(installationRef || '').trim()
      || !String(credentialRef || '').trim()
      || !String(installationCredential || '').trim()) {
    return { activated: false, reasonCode: 'installation_credential_activation_not_ready' };
  }
  try {
    const response = await fetchImpl(`${origin}${route}`, {
      method: 'POST',
      credentials: 'omit',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        installationRef,
        credentialRef,
        installationCredential,
      }),
    });
    const body = await response.json().catch(() => null);
    if (!response.ok) {
      return {
        activated: false,
        reasonCode: String(body?.code || 'installation_credential_activation_rejected'),
      };
    }
    return { activated: body?.outcome === 'activated' };
  } catch {
    return { activated: false, reasonCode: 'installation_credential_activation_unavailable' };
  }
}

const ACCOUNT_ELIGIBILITY_SIGNALS = new Set([
  'authenticated_observed',
  'cooldown_observed',
  'login_required',
  'access_restricted',
]);

const CONFIRMED_ACCOUNT_BLOCK_STATES = new Set(['cooling', 'needs_login', 'restricted']);

/**
 * A task page may stop only on a server-confirmed account decision. Transport failure, a
 * temporarily unavailable local host, and a malformed/unrecognized receipt are inconclusive;
 * they must not be converted into an account block after the task has already been leased.
 */
export function confirmedAccountObservationBlocksClaim(report) {
  if (report?.claimedTaskNotHeld === true) return true;
  if (report?.reported !== true) return false;
  if (report.bindingMismatch === true) return true;
  if (report.frozenAccountMismatch === true) return true;
  if (report.accountBusy === true) return true;
  return CONFIRMED_ACCOUNT_BLOCK_STATES.has(String(report.eligibilityState || ''));
}

/** Turn the account-report receipt into the only task-page stop decision. */
export function claimedTaskAccountDecision(report) {
  if (report?.claimedTaskNotHeld === true) {
    return {
      mayExecute: false,
      state: 'account_observation_blocked',
      message: '任务页已观察到账号状态变化或限制，已停止本次采集并等待服务端恢复条件。',
    };
  }
  if (report?.reported !== true) {
    return { mayExecute: true, state: 'account_observation_inconclusive', message: '' };
  }
  const blocked = confirmedAccountObservationBlocksClaim(report);
  return {
    mayExecute: !blocked,
    state: blocked ? 'account_observation_blocked' : 'account_observation_confirmed',
    message: blocked
      ? '任务页已观察到账号状态变化或限制，已停止本次采集并等待服务端恢复条件。'
      : '',
  };
}

function normalizedAccountObservation(observation) {
  if (!observation || typeof observation !== 'object' || Array.isArray(observation)) return null;
  const signal = String(observation.signal || '').trim();
  if (!ACCOUNT_ELIGIBILITY_SIGNALS.has(signal)) return null;
  if (signal === 'authenticated_observed') {
    const rawPlatformAccountId = String(observation.rawPlatformAccountId || '').trim();
    return rawPlatformAccountId ? { signal, rawPlatformAccountId } : null;
  }
  return { signal };
}

/**
 * Report one passive account observation. The producer never declares a final eligibility
 * state; the service owns that projection. A raw platform id, when an already-open page exposes
 * it, exists only in this request body and is not returned or cached.
 */
export async function reportLingganAccountEligibility({
  installationRef,
  installationCredential,
  taskId = null,
  observation,
  origin = LINGGAN_LOCAL_ORIGIN,
  fetchImpl = globalThis.fetch,
  health = null,
} = {}) {
  const route = accountEligibilityRouteFromHealth(health);
  const normalizedObservation = normalizedAccountObservation(observation);
  if (typeof fetchImpl !== 'function'
      || !String(installationRef || '').trim()
      || !String(installationCredential || '').trim()
      || !normalizedObservation) {
    return { reported: false, reasonCode: 'account_observation_not_ready' };
  }
  if (!route) {
    const availability = accountObservationAvailabilityFromHealth(health);
    return {
      reported: false,
      reasonCode: availability === 'identity_key_missing'
        ? 'account_observation_identity_key_missing'
        : 'account_observation_not_ready',
    };
  }
  try {
    const response = await fetchImpl(`${origin}${route}`, {
      method: 'POST',
      credentials: 'omit',
      headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      installationRef,
      installationCredential,
      observation: normalizedObservation,
      ...(String(taskId || '').trim() ? { taskId: String(taskId).trim() } : {}),
      }),
    });
    const body = await response.json().catch(() => null);
    if (!response.ok) {
      const reasonCode = String(body?.code || 'account_observation_rejected');
      return {
        reported: false,
        reasonCode,
        claimedTaskNotHeld: reasonCode === 'account_observation_claim_not_held',
      };
    }
    return {
      reported: body?.outcome === 'observed',
      accountRef: String(body?.accountRef || '').trim(),
      eligibilityState: String(body?.eligibilityState || 'unknown'),
      bindingRequired: body?.bindingRequired === true,
      bindingMismatch: body?.bindingMismatch === true,
      frozenAccountMismatch: body?.frozenAccountMismatch === true,
      accountBusy: body?.accountBusy === true,
    };
  } catch {
    return { reported: false, reasonCode: 'account_observation_unavailable' };
  }
}

/**
 * 问 Linggan：现在有我能做的活吗？
 *
 * **只认 `mayExecute`**。服务端可能返回任务体的同时并不允许执行（例如闸门关着），
 * 拿到任务体不等于拿到许可；凭「有没有 taskSpec」判断会绕过整条授权链。
 */
export async function claimLingganDispatch({
  installKey,
  installationCredential,
  origin = LINGGAN_LOCAL_ORIGIN,
  fetchImpl = globalThis.fetch,
  health = null,
} = {}) {
  if (typeof fetchImpl !== 'function') {
    return { mayExecute: false, decision: 'unavailable', message: '浏览器当前无法连接 Linggan。', nextPollAfterSeconds: 900 };
  }
  if (!String(installationCredential || '').trim()) {
    return { mayExecute: false, decision: 'installation_credential_missing', message: '本次安装尚未取得服务端凭据。', nextPollAfterSeconds: 300 };
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
      body: JSON.stringify({ installKey, installationCredential }),
    });
    const body = await response.json().catch(() => null);
    if (!response.ok) {
      return {
        mayExecute: false,
        decision: String(body?.code || 'unavailable'),
        message: `Linggan 返回 ${response.status}。`,
        nextPollAfterSeconds: response.status === 401 ? 300 : 900,
      };
    }
    if (body?.mayExecute !== true) {
      return {
        mayExecute: false,
        decision: String(body?.decision || 'unknown'),
        // 不许执行时**不把任务体带出去**：留着它只会让下游有机会「反正拿到了就跑」。
        taskSpec: null,
        executionSourceUrl: '',
        leaseRef: '',
        pageSessionPlan: null,
        message: String(body?.reason || ''),
        nextPollAfterSeconds: normalizePollSeconds(body?.nextPollAfterSeconds),
      };
    }
    try {
      return decodeAcquiredDispatch(body);
    } catch (error) {
      return {
        mayExecute: false,
        decision: 'invalid_dispatch',
        taskSpec: null,
        executionSourceUrl: '',
        leaseRef: '',
        pageSessionPlan: null,
        message: String(error?.message || 'dispatch_contract_invalid'),
        nextPollAfterSeconds: 900,
      };
    }
  } catch {
    // 连不上时退避得久一些：Linggan 没开着是常态，不该每分钟敲一次。
    return { mayExecute: false, decision: 'unavailable', message: 'Linggan 本机服务当前不可访问。', nextPollAfterSeconds: 900 };
  }
}

/**
 * Record a failed browser start for the installation that currently owns a
 * scheduled task, then let the server make that frozen task eligible again.
 *
 * This is intentionally narrower than a producer submission: there is no
 * Attempt, Capture Package, Evidence, or raw browser error text in this call.
 */
export async function reportLingganDispatchFailure({
  installKey,
  installationCredential,
  taskId,
  failureId,
  failureCode,
  origin = LINGGAN_LOCAL_ORIGIN,
  fetchImpl = globalThis.fetch,
  health = null,
} = {}) {
  if (typeof fetchImpl !== 'function') {
    return { reported: false, nextPollAfterSeconds: 300 };
  }
  const route = dispatchFailureRouteFromHealth(health);
  if (!route || !String(installKey || '').trim() || !String(installationCredential || '').trim() || !String(taskId || '').trim()
      || !String(failureId || '').trim() || !String(failureCode || '').trim()) {
    return { reported: false, nextPollAfterSeconds: 300 };
  }
  try {
    const response = await fetchImpl(`${origin}${route}`, {
      method: 'POST',
      credentials: 'omit',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ installKey, installationCredential, taskId, failureId, failureCode }),
    });
    if (!response.ok) return { reported: false, nextPollAfterSeconds: 300 };
    const body = await response.json().catch(() => null);
    const outcome = String(body?.outcome || '');
    const terminalUnavailable = outcome === 'unavailable' && body?.taskState === 'unavailable';
    if ((!['requeued', 'replay'].includes(outcome) || body?.taskState !== 'pending')
        && !terminalUnavailable) {
      return { reported: false, nextPollAfterSeconds: 300 };
    }
    return {
      reported: true,
      outcome,
      nextPollAfterSeconds: normalizePollSeconds(body?.nextPollAfterSeconds),
    };
  } catch {
    return { reported: false, nextPollAfterSeconds: 300 };
  }
}

function normalizePollSeconds(value, fallback = 300) {
  const seconds = Number(value ?? fallback);
  // A dispatch response legitimately returns 0: the server has just handed us work and the
  // following sequential task may be eligible as soon as its Package is accepted.  Chrome's
  // alarm floor is applied later by background.js; rejecting 0 here would discard a permitted
  // task before the Browser Producer can execute it.
  if (!Number.isFinite(seconds) || seconds < 0 || seconds > 86400) {
    throw new Error('dispatch_poll_interval_invalid');
  }
  return Math.floor(seconds);
}

/**
 * Decode the only response shape that is allowed to control a platform page.
 * A service response is input, not authority by itself: every identity and bounded plan is
 * checked here before background.js can open a window.
 */
export function decodeAcquiredDispatch(body = {}) {
  if (body?.mayExecute !== true || String(body?.decision || '') !== 'dispatch') {
    throw new Error('dispatch_permission_invalid');
  }
  const taskSpec = validateTaskSpec(body.taskSpec);
  const leaseRef = String(body?.leaseRef || '').trim();
  if (!leaseRef) throw new Error('dispatch_lease_required');
  const executionSourceUrl = String(body?.executionSourceUrl || '').trim();
  const capability = taskSpec.capabilitiesRequested[0];
  if (['content_detail', 'comments', 'replies', 'media_slots'].includes(capability)
      && !executionSourceUrl) {
    throw new Error('dispatch_execution_source_required');
  }
  const pageSessionPlan = body?.pageSessionPlan == null
    ? null
    : body.pageSessionPlan;
  if (pageSessionPlan !== null
      && (typeof pageSessionPlan !== 'object' || Array.isArray(pageSessionPlan))) {
    throw new Error('dispatch_page_session_plan_invalid');
  }
  return {
    mayExecute: true,
    decision: 'dispatch',
    taskSpec,
    executionSourceUrl,
    leaseRef,
    pageSessionPlan,
    message: String(body?.reason || ''),
    nextPollAfterSeconds: normalizePollSeconds(body?.nextPollAfterSeconds),
  };
}

/**
 * A page action is complete only when the content runtime returns the same task identity that
 * background.js dispatched. Missing replies and legacy truthy objects are not receipts.
 */
export function decodePageExecutionReceipt(response, expected = {}) {
  if (!response || typeof response !== 'object' || Array.isArray(response)) {
    return { ok: false, state: 'page_receipt_missing', message: '页面没有返回执行回执。' };
  }
  if (response.success !== true) {
    return {
      ok: false,
      state: String(response.state || response.code || 'page_read_failed'),
      message: String(response.message || '页面执行失败。'),
    };
  }
  const action = String(response.action || '');
  const capability = String(response.capability || '');
  const taskId = String(response.taskId || '');
  if (action !== String(expected.action || '')
      || capability !== String(expected.capability || '')
      || taskId !== String(expected.taskId || '')) {
    return { ok: false, state: 'page_receipt_identity_mismatch', message: '页面回执与派发任务身份不一致。' };
  }
  return { ok: true, state: String(response.state || 'page_read_completed'), message: String(response.message || '') };
}

/// Claim one bounded server-owned media acquisition. The source observation already passed
/// admission; this permission only authorizes fetching its candidate bytes and uploading them.
export async function claimLingganMediaAcquisition({
  installKey,
  installationCredential,
  origin = LINGGAN_LOCAL_ORIGIN,
  fetchImpl = globalThis.fetch,
  health = null,
} = {}) {
  if (typeof fetchImpl !== 'function') {
    return { mayExecute: false, decision: 'unavailable', nextPollAfterSeconds: 900 };
  }
  const route = producerRoutesFromHealth(health)?.mediaAcquisitionClaim;
  if (!route || !String(installKey || '').trim() || !String(installationCredential || '').trim()) {
    return { mayExecute: false, decision: 'unavailable', nextPollAfterSeconds: 900 };
  }
  try {
    const response = await fetchImpl(`${origin}${route}`, {
      method: 'POST',
      credentials: 'omit',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ installKey, installationCredential }),
    });
    if (!response.ok) {
      return { mayExecute: false, decision: 'unavailable', nextPollAfterSeconds: 300 };
    }
    const body = await response.json().catch(() => null);
    const mayExecute = body?.decision === 'acquired';
    return {
      mayExecute,
      decision: String(body?.decision || 'unknown'),
      workRef: mayExecute ? String(body?.workRef || '') : '',
      mediaObservationRef: mayExecute ? String(body?.observationRef || '') : '',
      componentKind: mayExecute ? String(body?.componentKind || 'single') : '',
      claimGeneration: mayExecute ? Number(body?.claimGeneration) : 0,
      candidateUris: mayExecute && Array.isArray(body?.candidateUris) ? body.candidateUris : [],
      nextPollAfterSeconds: Number(body?.nextPollAfterSeconds ?? 300),
    };
  } catch {
    return { mayExecute: false, decision: 'unavailable', nextPollAfterSeconds: 900 };
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
  if (spec?.contractVersion !== TASK_SPEC_VERSION) throw new Error('task_spec_contract_version_invalid');
  if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(String(spec?.taskId || ''))) throw new Error('task_spec_task_id_invalid');
  if (!String(spec?.pageType || '').trim()) throw new Error('task_spec_page_type_invalid');
  if (!['manual', 'scheduled'].includes(spec.source) || !['xhs', 'douyin'].includes(spec.platform)) throw new Error('task_spec_source_or_platform_invalid');
  if (!spec.target || typeof spec.target !== 'object' || Array.isArray(spec.target)) throw new Error('task_spec_target_invalid');
  const capabilities = Array.isArray(spec.capabilitiesRequested) ? spec.capabilitiesRequested : [];
  if (capabilities.length !== 1 || !CAPABILITIES.has(capabilities[0])) throw new Error('task_spec_capability_lane_invalid');
  const capability = capabilities[0];
  const target = spec.target;
  const requireText = (key) => { if (!String(target[key] || '').trim()) throw new Error(`task_spec_target_${key}_required`); };
  if (capability === 'discovery_search') requireText('query');
  if (capability === 'profile_discovery' || capability === 'author_profile') requireText('authorExternalId');
  if (['content_detail', 'comments', 'replies', 'media_bytes'].includes(capability)) requireText('contentExternalId');
  if (capability === 'media_slots') {
    const hasContent = Boolean(String(target.contentExternalId || '').trim());
    const hasAuthor = Boolean(String(target.authorExternalId || '').trim());
    if (hasContent === hasAuthor) throw new Error('task_spec_media_slots_target_invalid');
  }
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
