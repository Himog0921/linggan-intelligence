export const MONITOR_STATION_CAPABILITIES = [
  'xhs.list_scan',
  'xhs.author_links',
  'xhs.note_full',
  'xhs.comment_scan',
  'xhs.author_profile',
  'douyin.list_scan',
  'douyin.note_full',
  'douyin.comment_scan',
  'douyin.author_profile',
];

const PLATFORM_RUNTIME_CONFIGS = [
  {
    platform: 'xhs',
    label: '小红书',
    origins: ['https://xiaohongshu.com/*', 'https://*.xiaohongshu.com/*'],
    cookieDomains: ['xiaohongshu.com', '.xiaohongshu.com'],
    loginCookieNames: ['web_session', 'access-token'],
  },
  {
    platform: 'douyin',
    label: '抖音',
    origins: ['https://www.douyin.com/*'],
    cookieDomains: ['douyin.com', '.douyin.com', 'www.douyin.com'],
    loginCookieNames: ['sessionid', 'sid_guard'],
  },
];

function normalizeText(value = '') {
  return String(value || '').trim();
}

function invokeChromeAsync(fn, args = [], timeoutMs = 250) {
  if (typeof fn !== 'function') return Promise.resolve(null);
  return new Promise((resolve) => {
    let settled = false;
    let timer = null;
    const done = (result) => {
      if (settled) return;
      settled = true;
      if (timer) clearTimeout(timer);
      resolve(result ?? null);
    };
    timer = setTimeout(() => done(null), timeoutMs);
    try {
      const maybePromise = fn(...args, done);
      if (maybePromise && typeof maybePromise.then === 'function') {
        maybePromise.then(done).catch(() => done(null));
        return;
      }
      if (maybePromise !== undefined) {
        done(maybePromise);
        return;
      }
    } catch {
      try {
        const maybePromise = fn(...args);
        if (maybePromise && typeof maybePromise.then === 'function') {
          maybePromise.then(done).catch(() => done(null));
        } else {
          done(maybePromise);
        }
      } catch {
        done(null);
      }
    }
  });
}

async function hasOriginPermission(chromeApi, origins = []) {
  if (!chromeApi?.permissions?.contains || origins.length === 0) return 'unknown';
  try {
    const result = await invokeChromeAsync(chromeApi.permissions.contains.bind(chromeApi.permissions), [{ origins }]);
    if (result === true) return 'granted';
    if (result === false) return 'denied';
    return 'unknown';
  } catch {
    return 'unknown';
  }
}

async function readPlatformCookies(chromeApi, domains = []) {
  if (!chromeApi?.cookies?.getAll) {
    return { readable: false, cookies: [] };
  }
  const batches = await Promise.all(domains.map(async (domain) => {
    try {
      const cookies = await invokeChromeAsync(chromeApi.cookies.getAll.bind(chromeApi.cookies), [{ domain }]);
      return Array.isArray(cookies) ? cookies : [];
    } catch {
      return [];
    }
  }));
  return {
    readable: true,
    cookies: batches.flat(),
  };
}

function inferLoginState(cookies = [], loginCookieNames = []) {
  if (!Array.isArray(cookies) || cookies.length === 0) return 'logged_out';
  const names = new Set(cookies.map((cookie) => normalizeText(cookie?.name)));
  return loginCookieNames.some((name) => names.has(name)) ? 'logged_in' : 'logged_out';
}

export async function collectStationRuntimeStates({
  chromeApi = globalThis.chrome,
  now = Date.now(),
} = {}) {
  const states = [];
  for (const config of PLATFORM_RUNTIME_CONFIGS) {
    const pagePermission = await hasOriginPermission(chromeApi, config.origins);
    const cookieSnapshot = await readPlatformCookies(chromeApi, config.cookieDomains);
    const loginState = cookieSnapshot.readable
      ? inferLoginState(cookieSnapshot.cookies, config.loginCookieNames)
      : 'unknown';
    states.push({
      platform: config.platform,
      label: config.label,
      pagePermission,
      cookiesReadable: cookieSnapshot.readable,
      loginState,
      platformBlocked: false,
      checkedAt: now,
    });
  }
  return states;
}

function hasInjectableStoredAccount(account = {}) {
  return Boolean(normalizeText(account.accountId) && normalizeText(account.cookieJson));
}

function runtimeHealthStatus(runtimeState = null, fallback = 'unknown', options = {}) {
  if (!runtimeState) return fallback;
  if (runtimeState.pagePermission === 'denied' || runtimeState.pagePermission === 'missing') {
    return 'restricted';
  }
  if (runtimeState.platformBlocked) return 'restricted';
  if (runtimeState.loginState === 'logged_out' || runtimeState.loginState === 'login_expired') {
    if (options.canInjectStoredCookies) return fallback === 'unknown' ? 'healthy' : fallback;
    return 'needs_login';
  }
  if (runtimeState.loginState === 'logged_in' && runtimeState.pagePermission === 'granted') {
    return fallback === 'unknown' ? 'healthy' : fallback;
  }
  return fallback;
}

function isAccountHealthy(account = {}, now = Date.now()) {
  if (normalizeText(account.status || 'available') !== 'available') return false;
  if (Number(account.dailyQuotaLimit || 0) > 0 && Number(account.dailyQuotaUsed || 0) >= Number(account.dailyQuotaLimit || 0)) {
    return false;
  }
  return !(Number(account.cooldownUntil || 0) > now);
}

function mapAccountHealth(account = {}, now = Date.now()) {
  if (Number(account.cooldownUntil || 0) > now) return 'cooling';
  return isAccountHealthy(account, now) ? 'healthy' : 'unhealthy';
}

export function buildPlatformAccountReports(accounts = [], {
  purpose = 'execution',
  now = Date.now(),
  runtimeStates = [],
} = {}) {
  const latestByPlatform = new Map();
  const runtimeByPlatform = new Map(
    (Array.isArray(runtimeStates) ? runtimeStates : [])
      .filter((state) => normalizeText(state?.platform))
      .map((state) => [normalizeText(state.platform), state])
  );
  for (const account of Array.isArray(accounts) ? accounts : []) {
    const platform = normalizeText(account.platform || 'xhs');
    if (!platform) continue;
    const current = latestByPlatform.get(platform);
    const runtimeState = runtimeByPlatform.get(platform) || null;
    const baseHealthStatus = mapAccountHealth(account, now);
    const canInjectStoredCookies = baseHealthStatus === 'healthy' && hasInjectableStoredAccount(account);
    const next = {
      platform,
      platformAccountId: normalizeText(account.accountId) || null,
      displayName: normalizeText(account.name) || null,
      purpose,
      healthStatus: runtimeHealthStatus(runtimeState, baseHealthStatus, { canInjectStoredCookies }),
      cooldownUntil: Number(account.cooldownUntil || 0) || 0,
      dailyTaskCount: Number(account.dailyQuotaUsed || 0),
      dailyOpenedCount: Number(account.dailyQuotaUsed || 0),
      rawProfile: {
        status: normalizeText(account.status || 'available'),
        dailyQuotaUsed: Number(account.dailyQuotaUsed || 0),
        dailyQuotaLimit: Number(account.dailyQuotaLimit || 0),
        cooldownUntil: Number(account.cooldownUntil || 0),
        lastUsedAt: Number(account.lastUsedAt || 0),
        pagePermission: runtimeState?.pagePermission || 'unknown',
        loginState: runtimeState?.loginState || 'unknown',
        cookiesReadable: runtimeState?.cookiesReadable ?? false,
        platformBlocked: runtimeState?.platformBlocked ?? false,
        runtimeCheckedAt: runtimeState?.checkedAt || now,
        storedAccountAvailable: canInjectStoredCookies,
        executionLoginMode: canInjectStoredCookies ? 'stored_cookie_injection' : 'browser_session',
      },
    };

    if (!current || (current.healthStatus !== 'healthy' && next.healthStatus === 'healthy')) {
      latestByPlatform.set(platform, next);
    }
  }
  for (const runtimeState of runtimeByPlatform.values()) {
    if (latestByPlatform.has(runtimeState.platform)) continue;
    const healthStatus = runtimeHealthStatus(runtimeState, 'unknown');
    latestByPlatform.set(runtimeState.platform, {
      platform: runtimeState.platform,
      platformAccountId: `runtime:${runtimeState.platform}`,
      displayName: runtimeState.label || runtimeState.platform,
      purpose,
      healthStatus,
      cooldownUntil: 0,
      dailyTaskCount: 0,
      dailyOpenedCount: 0,
      rawProfile: {
        status: healthStatus === 'healthy' ? 'available' : healthStatus,
        pagePermission: runtimeState.pagePermission || 'unknown',
        loginState: runtimeState.loginState || 'unknown',
        cookiesReadable: runtimeState.cookiesReadable ?? false,
        platformBlocked: runtimeState.platformBlocked ?? false,
        runtimeCheckedAt: runtimeState.checkedAt || now,
      },
    });
  }
  return [...latestByPlatform.values()];
}

export async function collectStationPlatformAccounts(accountStore, options = {}) {
  const runtimeStates = Array.isArray(options.runtimeStates)
    ? options.runtimeStates
    : await collectStationRuntimeStates(options);
  const accounts = accountStore?.getAll ? await accountStore.getAll() : [];
  return buildPlatformAccountReports(accounts, { ...options, runtimeStates });
}

export function stationCapabilitiesForRuntimeStates(runtimeStates = [], baseCapabilities = MONITOR_STATION_CAPABILITIES) {
  const capabilities = new Set(Array.isArray(baseCapabilities) ? baseCapabilities : []);
  for (const state of Array.isArray(runtimeStates) ? runtimeStates : []) {
    const platform = normalizeText(state?.platform);
    if (!platform) continue;
    if (state.pagePermission === 'granted') capabilities.add(`${platform}.pageAccess`);
    if (state.loginState === 'logged_in') capabilities.add(`${platform}.loggedIn`);
  }
  return [...capabilities];
}
