import { hasActivePluginAuthorization } from './pluginAuthorization.js';

const INSTALL_CONFIG_PATH = 'lgboom-install.json';

function normalizeString(value = '') {
  return String(value || '').trim();
}

function isExpired(expiresAt = '') {
  const value = normalizeString(expiresAt);
  if (!value) return false;
  const time = Date.parse(value);
  return Number.isFinite(time) && time <= Date.now();
}

function normalizeInstallConfig(raw = {}) {
  if (!raw || typeof raw !== 'object' || Array.isArray(raw)) return null;
  const authorizationCode = normalizeString(raw.authorizationCode);
  const pairingCode = normalizeString(raw.pairingCode);
  if (!authorizationCode) return null;
  return {
    schemaVersion: Number(raw.schemaVersion || 1),
    serverUrl: normalizeString(raw.serverUrl),
    authorizationCode,
    pairingCode,
    expiresAt: normalizeString(raw.expiresAt),
  };
}

function normalizeConnectedStation(value = {}) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  const stationId = normalizeString(value.stationId || value.id);
  const stationToken = normalizeString(value.stationToken || value.token);
  if (!stationId || !stationToken) return null;
  return {
    stationId,
    stationToken,
    displayName: normalizeString(value.displayName),
    role: normalizeString(value.role || 'execution') || 'execution',
  };
}

export async function readPackagedInstallConfig({
  runtime = globalThis.chrome?.runtime,
  fetchFn = globalThis.fetch?.bind(globalThis),
} = {}) {
  if (!runtime?.getURL || typeof fetchFn !== 'function') return null;
  try {
    const response = await fetchFn(runtime.getURL(INSTALL_CONFIG_PATH), { cache: 'no-store' });
    if (!response?.ok) return null;
    return normalizeInstallConfig(await response.json());
  } catch {
    return null;
  }
}

export async function applyPackagedInstallBootstrap({
  readConfig = readPackagedInstallConfig,
  authorizationClient,
  stationClient,
  saveFlywheelConfig = async () => {},
  sendHeartbeat = async () => {},
  stationCapabilities = [],
  pluginVersion = '',
  browserLabel = '',
  now = () => Date.now(),
} = {}) {
  const config = await readConfig();
  if (!config) return { applied: false, reason: 'missing_config' };
  if (isExpired(config.expiresAt)) return { applied: false, reason: 'expired_config' };

  const [authorization, station] = await Promise.all([
    authorizationClient?.getStoredAuthorization?.() ?? {},
    stationClient?.getStoredStationIdentity?.() ?? {},
  ]);
  const hasAuthorization = hasActivePluginAuthorization(authorization);
  const hasStation = Boolean(normalizeString(station?.stationId) && normalizeString(station?.stationToken));
  if (hasAuthorization && hasStation) {
    return { applied: false, reason: 'already_configured' };
  }

  if (config.serverUrl) {
    await saveFlywheelConfig({ serverUrl: config.serverUrl, enabled: true });
  }

  const capabilities = Array.isArray(stationCapabilities) ? stationCapabilities : [];
  const shouldAuthorizeWithPackagedCode = !hasAuthorization || !hasStation;
  const stationKey = !hasStation
    ? normalizeString(station?.stationKey) || normalizeString(await stationClient?.ensureStationKey?.())
    : normalizeString(station?.stationKey);
  const nextAuthorization = shouldAuthorizeWithPackagedCode
    ? await authorizationClient.authorizeWithCode({
      authorizationCode: config.authorizationCode,
      stationKey,
      pluginVersion,
      browserLabel,
      capabilities,
    })
    : authorization;

  if (shouldAuthorizeWithPackagedCode) {
    await saveFlywheelConfig({
      enabled: true,
      apiToken: normalizeString(nextAuthorization.authorizationToken),
      dataToken: '',
      dataTokenExpiresAt: '',
      dataWorkspaceId: '',
      dataUserEmail: '',
      dataUserName: '',
    });
  }

  let nextStation = station;
  const connectedStation = normalizeConnectedStation(nextAuthorization?.station);
  if (!hasStation && connectedStation && stationKey) {
    nextStation = await stationClient.saveStationIdentity({
      stationKey,
      ...connectedStation,
      capabilities,
      pairedAt: now(),
    });
    await sendHeartbeat('online');
  } else if (!hasStation && config.pairingCode && typeof stationClient?.registerWithPairingCode === 'function') {
    nextStation = await stationClient.registerWithPairingCode({
      pairingCode: config.pairingCode,
      capabilities,
      pluginVersion,
      browserLabel,
    });
    await sendHeartbeat('online');
  } else if (!hasStation) {
    nextStation = null;
  }

  return {
    applied: true,
    authorization: nextAuthorization,
    station: nextStation,
  };
}
