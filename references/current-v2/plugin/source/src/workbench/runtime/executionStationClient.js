import { normalizeServerUrl } from '../../shared/utils.js';
import {
  buildSyncRequestV11,
  extractMailboxVersionsFromResponse,
  extractNextSyncFromResponse,
  resolveStationSessionId,
  clearStationSessionId,
} from '../protocol/syncEnvelopeV11.js';

const STORAGE_KEY = 'workbenchExecutionStation';
const DEFAULT_SERVER_URL = 'https://lingganboom.fun';

function normalizeString(value = '') {
  return String(value || '').trim();
}

function toStringArray(value) {
  return Array.isArray(value) ? value.filter((item) => typeof item === 'string') : [];
}

function isPlainObject(value) {
  return value && typeof value === 'object' && !Array.isArray(value);
}

async function readStorage(storageArea, key) {
  if (!storageArea?.get) return {};
  const result = await storageArea.get(key);
  return result?.[key] || {};
}

async function writeStorage(storageArea, key, value) {
  if (!storageArea?.set) return value;
  await storageArea.set({ [key]: value });
  return value;
}

async function readErrorText(response) {
  if (!response?.text) return '';
  return response.text().catch(() => '');
}

function toFiniteNumber(value, fallback = 0) {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}

function toOptionalInteger(value) {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? Math.floor(parsed) : undefined;
}

export function stationIdentityNeedsRepair(identity = {}) {
  return ![
    identity?.stationId,
    identity?.stationToken,
    identity?.stationSigningSecret,
  ].every((value) => normalizeString(value));
}

export function buildClaimedStationIdentity({
  station = {},
  stationKey = '',
  capabilities = [],
  pairedAt = Date.now(),
} = {}) {
  return {
    stationKey: normalizeString(stationKey),
    stationId: normalizeString(station?.stationId || station?.id),
    stationToken: normalizeString(station?.stationToken || station?.token),
    stationSigningSecret: normalizeString(station?.stationSigningSecret),
    stationSigningSecretVersion: toOptionalInteger(station?.stationSigningSecretVersion),
    displayName: normalizeString(station?.displayName),
    role: normalizeString(station?.role || 'execution') || 'execution',
    capabilities: toStringArray(capabilities),
    pairedAt,
  };
}

function parseRetryAfterHeader(value = '') {
  const normalized = normalizeString(value);
  if (!normalized) return 0;
  const seconds = Number(normalized);
  if (Number.isFinite(seconds) && seconds > 0) return Math.round(seconds * 1000);
  const parsedDate = new Date(normalized).getTime();
  if (Number.isFinite(parsedDate)) return Math.max(0, parsedDate - Date.now());
  return 0;
}

function parseErrorBody(text = '') {
  try {
    const parsed = JSON.parse(normalizeString(text));
    return parsed && typeof parsed === 'object' ? parsed : {};
  } catch {
    return {};
  }
}

function errorMessageFromResponseText(text = '', fallback = '') {
  const body = normalizeString(text);
  if (!body) return fallback;
  const parsed = parseErrorBody(body);
  const message = normalizeString(parsed?.error || parsed?.message);
  return message || body;
}

function createHttpError(message, { status = 0, retryable = false, reasonCode = '', retryAfterMs = 0 } = {}) {
  const error = new Error(message);
  error.status = status;
  error.retryable = Boolean(retryable);
  error.reasonCode = normalizeString(reasonCode);
  error.retryAfterMs = toFiniteNumber(retryAfterMs, 0);
  return error;
}

function isRetryableStatus(status) {
  return [408, 429, 500, 502, 503, 504].includes(Number(status));
}

export function createExecutionStationClient({
  storageArea = globalThis.chrome?.storage?.local,
  fetchFn = globalThis.fetch?.bind(globalThis),
  resolveServerUrl = async () => DEFAULT_SERVER_URL,
  resolveAuthorization = async () => ({}),
  randomUUID = () => globalThis.crypto?.randomUUID?.() || `${Date.now()}-${Math.random()}`,
  now = () => Date.now(),
} = {}) {
  async function getStoredStationIdentity() {
    return readStorage(storageArea, STORAGE_KEY);
  }

  async function saveStationIdentity(identity = {}) {
    const existing = await getStoredStationIdentity();
    const next = {
      ...existing,
      ...identity,
      updatedAt: now(),
    };
    return writeStorage(storageArea, STORAGE_KEY, next);
  }

  async function ensureStationKey() {
    const existing = await getStoredStationIdentity();
    const stationKey = normalizeString(existing.stationKey) || normalizeString(randomUUID());
    if (stationKey !== existing.stationKey) {
      await saveStationIdentity({ stationKey });
    }
    return stationKey;
  }

  async function postJson(path, body = {}) {
    if (typeof fetchFn !== 'function') {
      throw createHttpError('fetch unavailable', { retryable: true });
    }
    const baseUrl = normalizeServerUrl(await resolveServerUrl(), DEFAULT_SERVER_URL);
    const authorization = await resolveAuthorization();
    const authorizationToken = normalizeString(
      authorization?.authorizationToken
      || authorization?.apiToken
      || authorization?.token,
    );
    const headers = { 'Content-Type': 'application/json' };
    if (authorizationToken) {
      headers.Authorization = `Bearer ${authorizationToken}`;
    }
    const response = await fetchFn(`${baseUrl}${path}`, {
      method: 'POST',
      headers,
      body: JSON.stringify(body),
    });
    if (!response.ok) {
      const text = await readErrorText(response);
      const parsed = parseErrorBody(text);
      const headerRetryAfterMs = parseRetryAfterHeader(response.headers?.get?.('Retry-After'));
      const bodyRetryAfterMs = toFiniteNumber(parsed?.retryAfterMs, 0)
        || toFiniteNumber(parsed?.retryAfterSeconds, 0) * 1000;
      // 错误码：优先 code/reasonCode（机读码，如 plugin_protocol_backpressure），
      // 然后是 V1.1 sync 的 error 字段（如 VERSION_REJECTED / PROTOCOL_VERSION_REJECTED）
      throw createHttpError(errorMessageFromResponseText(text, `HTTP ${response.status}`), {
        status: response.status,
        retryable: isRetryableStatus(response.status),
        reasonCode: parsed?.code || parsed?.reasonCode || parsed?.error || '',
        retryAfterMs: headerRetryAfterMs || bodyRetryAfterMs,
      });
    }
    return response.json().catch(() => ({}));
  }

  async function getJson(path) {
    if (typeof fetchFn !== 'function') {
      throw createHttpError('fetch unavailable', { retryable: true });
    }
    const baseUrl = normalizeServerUrl(await resolveServerUrl(), DEFAULT_SERVER_URL);
    const response = await fetchFn(`${baseUrl}${path}`, {
      method: 'GET',
      headers: { Accept: 'application/json' },
    });
    if (!response.ok) {
      const text = await readErrorText(response);
      throw createHttpError(errorMessageFromResponseText(text, `HTTP ${response.status}`), {
        status: response.status,
        retryable: isRetryableStatus(response.status),
      });
    }
    return response.json().catch(() => ({}));
  }

  async function registerWithPairingCode({
    pairingCode = '',
    capabilities = [],
    pluginVersion = '',
    browserLabel = '',
  } = {}) {
    const stationKey = await ensureStationKey();
    const authorization = await resolveAuthorization();
    const data = await postJson('/api/execution-stations/register', {
      pairingCode: normalizeString(pairingCode),
      authorizationId: normalizeString(authorization?.authorizationId),
      stationKey,
      pluginVersion: normalizeString(pluginVersion),
      browserLabel: normalizeString(browserLabel),
      capabilities: toStringArray(capabilities),
    });
    const identity = await saveStationIdentity({
      stationId: normalizeString(data.stationId),
      stationToken: normalizeString(data.stationToken),
      stationSigningSecret: normalizeString(data.stationSigningSecret),
      stationSigningSecretVersion: toOptionalInteger(data.stationSigningSecretVersion),
      displayName: normalizeString(data.displayName),
      role: normalizeString(data.role),
      stationKey,
      capabilities: toStringArray(capabilities),
      pairedAt: now(),
    });
    return identity;
  }

  async function sendHeartbeat({
    status = 'online',
    capabilities = [],
    pluginVersion = '',
    platformAccounts = [],
    // V1.1 新增：可选参数，用于构造 activeLeases[] / capacity
    activeLane = '',
    localLease = null,
    activeTask = null,
  } = {}) {
    const identity = await getStoredStationIdentity();
    const authorization = await resolveAuthorization();
    const stationId = normalizeString(identity.stationId);
    const stationToken = normalizeString(identity.stationToken);
    if (!stationId || !stationToken) {
      return { success: false, retryable: false, reason: 'station_not_registered' };
    }

    try {
      const storedMailboxStationVersion = toOptionalInteger(identity.mailboxVersion);
      const mailboxLaneVersions = isPlainObject(identity.mailboxLaneVersions)
        ? identity.mailboxLaneVersions
        : {};
      const stationSessionId = await resolveStationSessionId({ storageArea });

      const requestBody = buildSyncRequestV11({
        stationId,
        stationToken,
        pluginVersion,
        stationSessionId,
        capabilities: capabilities.length ? capabilities : identity.capabilities,
        platformAccounts,
        activeLane,
        localLease,
        activeTask,
        mailboxStationVersion: storedMailboxStationVersion,
        mailboxLaneVersions,
        includeCapacity: false,
      });

      const data = await postJson('/api/execution-stations/sync', requestBody);

      const heartbeat = data?.heartbeat && typeof data.heartbeat === 'object' && !Array.isArray(data.heartbeat)
        ? data.heartbeat
        : data;
      const nextMailboxVersions = extractMailboxVersionsFromResponse(data);
      const nextSync = extractNextSyncFromResponse(data);

      // 持久化 V1.1 mailbox 版本号（station + lanes）
      const identityPatch = {
        stationId,
        stationToken,
        role: normalizeString(heartbeat?.station?.role) || normalizeString(identity.role),
        lastHeartbeatAt: now(),
        capabilities: toStringArray(capabilities.length ? capabilities : identity.capabilities),
      };
      if (nextMailboxVersions.station !== undefined) {
        identityPatch.mailboxVersion = nextMailboxVersions.station;
      }
      if (Object.keys(nextMailboxVersions.lanes).length > 0) {
        identityPatch.mailboxLaneVersions = nextMailboxVersions.lanes;
      }
      await saveStationIdentity(identityPatch);

      const mailbox = nextMailboxVersions.station !== undefined
        ? { version: nextMailboxVersions.station }
        : null;
      const stationMailboxChanged = nextMailboxVersions.station !== undefined
        && nextMailboxVersions.station !== storedMailboxStationVersion;
      const laneMailboxChanged = Object.entries(nextMailboxVersions.lanes).some(
        ([lane, version]) => toOptionalInteger(mailboxLaneVersions[lane]) !== version,
      );
      const mailboxChanged = stationMailboxChanged || laneMailboxChanged;

      return {
        success: true,
        ...heartbeat,
        sync: data,
        mailbox,
        mailboxVersions: nextMailboxVersions,
        nextSync,
        shouldPollNow: mailboxChanged || /^(mailbox|claim)/.test(nextSync.reason),
      };
    } catch (error) {
      return {
        success: false,
        retryable: error?.retryable !== false,
        status: Number(error?.status || 0),
        reasonCode: normalizeString(error?.reasonCode || ''),
        error: String(error?.message || error || 'heartbeat_failed'),
        nextRetryAfterMs: toFiniteNumber(error?.retryAfterMs, 0) || 30_000,
        nextRetryAt: now() + (toFiniteNumber(error?.retryAfterMs, 0) || 30_000),
      };
    }
  }

  async function fetchVapidPublicKey() {
    try {
      return await getJson('/api/push/vapid-public-key');
    } catch (error) {
      return {
        enabled: false,
        error: String(error?.message || error || 'vapid_public_key_failed'),
      };
    }
  }

  async function registerPushSubscription({
    subscription = null,
    pluginVersion = '',
    browserLabel = '',
  } = {}) {
    const identity = await getStoredStationIdentity();
    const authorization = await resolveAuthorization();
    const stationId = normalizeString(identity.stationId);
    const stationToken = normalizeString(identity.stationToken);
    if (!stationId || !stationToken) {
      return { ok: false, skipped: true, reason: 'station_not_registered' };
    }

    const data = await postJson('/api/execution-stations/push-subscription', {
      stationId,
      stationToken,
      authorizationId: normalizeString(authorization?.authorizationId),
      pluginVersion: normalizeString(pluginVersion),
      browserLabel: normalizeString(browserLabel),
      subscription,
    });
    await saveStationIdentity({
      pushSubscriptionEndpoint: normalizeString(subscription?.endpoint),
      pushSubscriptionRegisteredAt: now(),
    });
    return { ok: true, ...data };
  }

  async function clearStationIdentity({ preserveStationKey = true } = {}) {
    // 同步清理 V1.1 stationSessionId（换授权/换工位时服务端应看到新 session）
    await clearStationSessionId({ storageArea });
    const existing = await getStoredStationIdentity();
    if (!preserveStationKey || !normalizeString(existing.stationKey)) {
      if (storageArea?.remove) {
        await storageArea.remove(STORAGE_KEY);
        return {};
      }
      return writeStorage(storageArea, STORAGE_KEY, {});
    }
    return writeStorage(storageArea, STORAGE_KEY, {
      stationKey: normalizeString(existing.stationKey),
      updatedAt: now(),
      clearedAt: now(),
    });
  }

  return {
    getStoredStationIdentity,
    saveStationIdentity,
    ensureStationKey,
    clearStationIdentity,
    registerWithPairingCode,
    sendHeartbeat,
    fetchVapidPublicKey,
    registerPushSubscription,
    // V1.1：暴露 session id 解析给上层（用于诊断）
    resolveStationSessionId: () => resolveStationSessionId({ storageArea }),
  };
}
