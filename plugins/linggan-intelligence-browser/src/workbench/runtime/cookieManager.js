import { accountStore } from '../../db/accountStore.js';

const XHS_DOMAIN = '.xiaohongshu.com';
const XHS_URL = 'https://www.xiaohongshu.com';
export const PLATFORM_COOKIE_DOMAINS = {
  xhs: XHS_DOMAIN,
  douyin: '.douyin.com',
};

function parseCookieJson(cookieJson) {
  if (Array.isArray(cookieJson)) return cookieJson;
  if (typeof cookieJson === 'string') {
    try {
      const parsed = JSON.parse(cookieJson);
      return Array.isArray(parsed) ? parsed : [parsed];
    } catch {
      return [];
    }
  }
  return [];
}

export async function selectAvailableAccount(platform = 'xhs') {
  return accountStore.getAvailable(platform);
}

export function resolveBoundTaskAccountId(task = {}) {
  const payload = task && typeof task.payload === 'object' && !Array.isArray(task.payload)
    ? task.payload
    : {};
  return String(
    task.platformAccountId
      || task.reservedPlatformAccountId
      || task.accountId
      || payload.platformAccountId
      || payload.reservedPlatformAccountId
      || payload.accountId
      || ''
  ).trim();
}

export async function selectAccountForWorkbenchTask(task = {}, platform = 'xhs') {
  const boundAccountId = resolveBoundTaskAccountId(task);
  if (boundAccountId) {
    if (/^runtime:/i.test(boundAccountId)) {
      return {
        accountId: boundAccountId,
        platform,
        runtimeSession: true,
      };
    }
    const account = await accountStore.getById(boundAccountId);
    return account || null;
  }
  return selectAvailableAccount(platform);
}

export function resolveCookieDomain(platformOrDomain = 'xhs') {
  const value = String(platformOrDomain || '').trim();
  if (!value) return XHS_DOMAIN;
  if (PLATFORM_COOKIE_DOMAINS[value]) return PLATFORM_COOKIE_DOMAINS[value];
  return value;
}

export async function clearCookiesForDomain(domain = XHS_DOMAIN) {
  try {
    const cookies = await chrome.cookies.getAll({ domain });
    for (const cookie of cookies) {
      const url = `http${cookie.secure ? 's' : ''}://${cookie.domain.replace(/^\./, '')}${cookie.path || '/'}`;
      await chrome.cookies.remove({ url, name: cookie.name });
    }
    return { success: true, removed: cookies.length };
  } catch (error) {
    console.warn('[灵感爆爆爆] clearCookies failed:', error);
    return { success: false, error: String(error?.message || error) };
  }
}

export async function injectCookiesForAccount(cookieJson, platformOrDomain = 'xhs') {
  const domain = resolveCookieDomain(platformOrDomain);
  const cookies = parseCookieJson(cookieJson);
  if (!cookies.length) {
    return { success: false, error: 'no_valid_cookies' };
  }

  await clearCookiesForDomain(domain);

  let injected = 0;
  for (const cookie of cookies) {
    if (!cookie.name) continue;
    try {
      const url = cookie.url || `https://${(cookie.domain || domain).replace(/^\./, '')}${cookie.path || '/'}`;
      await chrome.cookies.set({
        url,
        name: cookie.name,
        value: String(cookie.value || ''),
        domain: cookie.domain || domain,
        path: cookie.path || '/',
        secure: cookie.secure !== false,
        httpOnly: Boolean(cookie.httpOnly),
        sameSite: cookie.sameSite || 'lax',
        ...(cookie.expirationDate ? { expirationDate: Number(cookie.expirationDate) } : {}),
      });
      injected++;
    } catch (error) {
      console.warn(`[灵感爆爆爆] cookie set failed for ${cookie.name}:`, error);
    }
  }
  return { success: true, injected };
}
