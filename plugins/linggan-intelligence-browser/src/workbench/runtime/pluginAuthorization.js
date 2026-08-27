import { normalizeServerUrl } from '../../shared/utils.js';

export const WORKBENCH_PLUGIN_AUTH_STORAGE_KEY = 'workbenchPluginAuthorization';

const DEFAULT_SERVER_URL = 'https://lingganboom.fun';

function normalizeString(value = '') {
  return String(value || '').trim();
}

function normalizeScope(value) {
  return Array.isArray(value)
    ? value
      .map((item) => normalizeString(item))
      .filter(Boolean)
    : [];
}

function normalizeStation(value) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  const stationId = normalizeString(value.stationId || value.id);
  const stationToken = normalizeString(value.stationToken || value.token);
  if (!stationId || !stationToken) return null;
  return {
    stationId,
    stationToken,
    displayName: normalizeString(value.displayName),
    role: normalizeString(value.role || 'execution') || 'execution',
    status: normalizeString(value.status),
  };
}

function resolveDefaultStorageArea() {
  return globalThis.chrome?.storage?.local || null;
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

async function removeStorage(storageArea, key) {
  if (storageArea?.remove) {
    await storageArea.remove(key);
    return;
  }
  if (storageArea?.set) {
    await storageArea.set({ [key]: null });
  }
}

async function readErrorText(response) {
  if (!response?.text) return '';
  return response.text().catch(() => '');
}

function createHttpError(message, { status = 0, retryable = false } = {}) {
  const error = new Error(message);
  error.status = status;
  error.retryable = Boolean(retryable);
  return error;
}

function isRetryableStatus(status) {
  return [408, 429, 500, 502, 503, 504].includes(Number(status));
}

export function hasActivePluginAuthorization(authorization = {}) {
  const token = normalizeString(
    authorization.authorizationToken
    || authorization.apiToken
    || authorization.token,
  );
  const status = normalizeString(authorization.status || authorization.authorizationStatus || '');
  if (!token) return false;
  if (!status) return true;
  return !['revoked', 'expired', 'suspended', 'disabled'].includes(status);
}

export function getPluginAuthorizationBlockedMessage(authorization = {}) {
  const status = normalizeString(authorization.status || authorization.authorizationStatus || '');
  if (status === 'expired') {
    return '插件授权已过期，请去内容工作台设置生成新的授权码。';
  }
  if (status === 'revoked') {
    return '插件授权已被撤销，请联系管理员重新授权。';
  }
  if (status === 'pending') {
    return '授权申请已发送，等待内容工作台审批。';
  }
  if (status === 'approved') {
    return '授权已通过，请在插件里点击检查审批结果完成激活。';
  }
  if (status === 'suspended' || status === 'disabled') {
    return '插件授权当前不可用，请联系管理员恢复。';
  }
  return '当前浏览器还没有插件授权。可以从内容工作台重新下载安装，或在插件里发起授权申请。';
}

export async function readStoredPluginAuthorization({
  storageArea = resolveDefaultStorageArea(),
  storageKey = WORKBENCH_PLUGIN_AUTH_STORAGE_KEY,
} = {}) {
  const stored = await readStorage(storageArea, storageKey);
  return stored && typeof stored === 'object' ? { ...stored } : {};
}

export async function assertActivePluginAuthorization({
  storageArea = resolveDefaultStorageArea(),
  storageKey = WORKBENCH_PLUGIN_AUTH_STORAGE_KEY,
} = {}) {
  const authorization = await readStoredPluginAuthorization({ storageArea, storageKey });
  if (hasActivePluginAuthorization(authorization)) {
    return authorization;
  }
  const error = new Error(getPluginAuthorizationBlockedMessage(authorization));
  error.code = 'plugin_authorization_required';
  error.authorizationStatus = normalizeString(authorization.status || authorization.authorizationStatus || 'missing');
  error.userMessage = error.message;
  throw error;
}

export function createPluginAuthorizationClient({
  storageArea = resolveDefaultStorageArea(),
  storageKey = WORKBENCH_PLUGIN_AUTH_STORAGE_KEY,
  fetchFn = globalThis.fetch?.bind(globalThis),
  resolveServerUrl = async () => DEFAULT_SERVER_URL,
  randomUUID = () => globalThis.crypto?.randomUUID?.() || `${Date.now()}-${Math.random()}`,
  now = () => Date.now(),
} = {}) {
  async function getStoredAuthorization() {
    return readStoredPluginAuthorization({ storageArea, storageKey });
  }

  async function saveAuthorization(authorization = {}) {
    const existing = await getStoredAuthorization();
    const next = {
      ...existing,
      ...authorization,
      updatedAt: now(),
    };
    return writeStorage(storageArea, storageKey, next);
  }

  async function clearAuthorization({ preserveDeviceId = true } = {}) {
    const existing = await getStoredAuthorization();
    if (!preserveDeviceId || !normalizeString(existing.deviceId)) {
      await removeStorage(storageArea, storageKey);
      return {};
    }
    const preserved = {
      deviceId: normalizeString(existing.deviceId),
      updatedAt: now(),
      clearedAt: now(),
    };
    await writeStorage(storageArea, storageKey, preserved);
    return preserved;
  }

  async function ensureDeviceId() {
    const existing = await getStoredAuthorization();
    const deviceId = normalizeString(existing.deviceId) || normalizeString(randomUUID());
    if (deviceId !== normalizeString(existing.deviceId)) {
      await saveAuthorization({ deviceId });
    }
    return deviceId;
  }

  async function postJson(path, body = {}) {
    if (typeof fetchFn !== 'function') {
      throw createHttpError('fetch unavailable', { retryable: true });
    }
    const baseUrl = normalizeServerUrl(await resolveServerUrl(), DEFAULT_SERVER_URL);
    const response = await fetchFn(`${baseUrl}${path}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    });
    if (!response.ok) {
      const text = await readErrorText(response);
      throw createHttpError(text || `HTTP ${response.status}`, {
        status: response.status,
        retryable: isRetryableStatus(response.status),
      });
    }
    return response.json().catch(() => ({}));
  }

  async function authorizeWithCode({
    authorizationCode = '',
    stationKey = '',
    pluginVersion = '',
    browserLabel = '',
    capabilities = [],
  } = {}) {
    const deviceId = await ensureDeviceId();
    const data = await postJson('/api/plugin-authorizations/activate', {
      authorizationCode: normalizeString(authorizationCode),
      deviceId,
      stationKey: normalizeString(stationKey),
      pluginVersion: normalizeString(pluginVersion),
      browserLabel: normalizeString(browserLabel),
      capabilities: normalizeScope(capabilities),
    });
    return saveAuthorization({
      deviceId,
      authorizationId: normalizeString(data.authorizationId || data.id),
      authorizationToken: normalizeString(data.authorizationToken || data.apiToken || data.token),
      status: normalizeString(data.status || 'active') || 'active',
      teamId: normalizeString(data.teamId),
      teamName: normalizeString(data.teamName),
      memberId: normalizeString(data.memberId),
      memberName: normalizeString(data.memberName),
      seatId: normalizeString(data.seatId),
      seatName: normalizeString(data.seatName),
      issuedBy: normalizeString(data.issuedBy),
      scope: normalizeScope(data.scope),
      expiresAt: normalizeString(data.expiresAt),
      station: normalizeStation(data.station),
      authorizedAt: now(),
    });
  }

  async function requestWorkbenchApproval({
    pluginVersion = '',
    browserLabel = '',
  } = {}) {
    const deviceId = await ensureDeviceId();
    const data = await postJson('/api/plugin-authorizations/requests', {
      deviceId,
      pluginVersion: normalizeString(pluginVersion),
      browserLabel: normalizeString(browserLabel),
    });
    return saveAuthorization({
      deviceId,
      authorizationId: normalizeString(data.requestId || data.authorizationId || data.id),
      authorizationToken: '',
      status: normalizeString(data.status || 'pending') || 'pending',
      authorizationMessage: normalizeString(data.message),
      requestedAt: now(),
    });
  }

  async function claimApprovedRequest({
    stationKey = '',
    pluginVersion = '',
    browserLabel = '',
    capabilities = [],
  } = {}) {
    const existing = await getStoredAuthorization();
    const authorizationId = normalizeString(existing.authorizationId || existing.id);
    const deviceId = normalizeString(existing.deviceId) || await ensureDeviceId();
    if (!authorizationId) {
      const error = new Error('authorization_request_missing');
      error.code = 'authorization_request_missing';
      throw error;
    }
    const data = await postJson(`/api/plugin-authorizations/requests/${authorizationId}/claim`, {
      deviceId,
      stationKey: normalizeString(stationKey),
      pluginVersion: normalizeString(pluginVersion),
      browserLabel: normalizeString(browserLabel),
      capabilities: normalizeScope(capabilities),
    });
    const authorizationToken = normalizeString(data.authorizationToken || data.apiToken || data.token);
    if (normalizeString(data.status) !== 'active' || !authorizationToken) {
      const authorization = await saveAuthorization({
        deviceId,
        authorizationId,
        status: normalizeString(data.status || existing.status || 'pending') || 'pending',
        authorizationMessage: normalizeString(data.message),
      });
      return { authorization, station: null, claimed: false };
    }
    const authorization = await saveAuthorization({
      deviceId,
      authorizationId: normalizeString(data.authorizationId || authorizationId),
      authorizationToken,
      status: 'active',
      teamName: normalizeString(data.teamName),
      memberName: normalizeString(data.memberName),
      seatName: normalizeString(data.seatName),
      expiresAt: normalizeString(data.expiresAt),
      authorizedAt: now(),
    });
    return {
      authorization,
      station: data.station && typeof data.station === 'object' ? data.station : null,
      claimed: true,
    };
  }

  return {
    getStoredAuthorization,
    saveAuthorization,
    clearAuthorization,
    authorizeWithCode,
    requestWorkbenchApproval,
    claimApprovedRequest,
  };
}
