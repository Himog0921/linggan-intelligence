import { LINGGAN_RUNTIME_ACTION } from './runtimeActions.js';

const XHS_ORIGINS = new Set(['xiaohongshu.com', 'www.xiaohongshu.com']);
const XHS_PROFILE_PATH = /^\/user\/profile\/([a-f0-9]{16,32})\/?$/i;
const CURRENT_ACCOUNT_NAVIGATION_SELECTOR = 'nav, [role="navigation"], ul, ol, [role="list"]';
const REQUIRED_GLOBAL_NAVIGATION_PATHS = new Set(['/explore', '/notification', '/chat']);
const ACCOUNT_PROBE_ATTEMPTS = 8;
const ACCOUNT_PROBE_RETRY_DELAY_MS = 500;

function normalizeMarker(value) {
  return String(value || '').replace(/\s+/g, '').trim();
}

function hasCurrentAccountMarker(link) {
  if (!link || typeof link.getAttribute !== 'function') return false;
  return [link.textContent, link.getAttribute('aria-label'), link.getAttribute('title')]
    .map(normalizeMarker)
    .some((value) => value === '我' || value === '我的');
}

function xhsNavigationPath(href) {
  try {
    const url = new URL(String(href || '').trim(), 'https://www.xiaohongshu.com');
    if (url.protocol !== 'https:' || !XHS_ORIGINS.has(url.hostname.toLowerCase())) return '';
    return url.pathname;
  } catch {
    return '';
  }
}

function isGlobalNavigationScope(link) {
  if (!link || typeof link.closest !== 'function') return false;
  const navigation = link.closest(CURRENT_ACCOUNT_NAVIGATION_SELECTOR);
  if (!navigation || typeof navigation.querySelectorAll !== 'function') return false;
  const paths = new Set(
    [...navigation.querySelectorAll('a[href]')]
      .map((candidate) => xhsNavigationPath(candidate.getAttribute?.('href')))
      .filter(Boolean),
  );
  return [...REQUIRED_GLOBAL_NAVIGATION_PATHS].every((path) => paths.has(path));
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * Read only the explicit current-account link already rendered in XHS navigation. A creator
 * profile's `__INITIAL_STATE__.user.userInfo` describes the viewed creator, so it must never
 * be used as the execution account identity.
 */
export function currentAccountHrefFromDocument(doc) {
  if (!doc || typeof doc.querySelectorAll !== 'function') return '';
  for (const link of doc.querySelectorAll('a[href]')) {
    if (!hasCurrentAccountMarker(link) || !isGlobalNavigationScope(link)) continue;
    const href = String(link.getAttribute('href') || '').trim();
    if (accountObservationFromCurrentAccountHref(href).signal === 'authenticated_observed') return href;
  }
  return '';
}

export function accountObservationFromCurrentAccountHref(href) {
  let url;
  try {
    url = new URL(String(href || '').trim(), 'https://www.xiaohongshu.com');
  } catch {
    return { signal: 'signal_incomplete', rawPlatformAccountId: '' };
  }
  if (url.protocol !== 'https:' || !XHS_ORIGINS.has(url.hostname.toLowerCase())) {
    return { signal: 'signal_incomplete', rawPlatformAccountId: '' };
  }
  const match = url.pathname.match(XHS_PROFILE_PATH);
  const rawPlatformAccountId = String(match?.[1] || '').trim();
  if (!rawPlatformAccountId) return { signal: 'signal_incomplete', rawPlatformAccountId: '' };
  return { signal: 'authenticated_observed', rawPlatformAccountId };
}

/**
 * Read only the current account fact already rendered as part of an XHS page load. This never
 * navigates, opens a tab, reads cookies, starts collection, or retains the raw id after the
 * message resolves. An absent marker is inconclusive, so it never emits a synthetic negative
 * account report that could overwrite a prior real observation.
 */
export async function reportPassiveAccountEligibility({
  readCurrentAccountHref,
  sendMessage,
  attempts = ACCOUNT_PROBE_ATTEMPTS,
  sleep = delay,
}) {
  let observation = { signal: 'signal_incomplete', rawPlatformAccountId: '' };
  const maxAttempts = Math.min(ACCOUNT_PROBE_ATTEMPTS, Math.max(1, Number(attempts) || 1));
  for (let attempt = 0; attempt < maxAttempts; attempt += 1) {
    try {
      observation = accountObservationFromCurrentAccountHref(await readCurrentAccountHref());
    } catch {
      observation = { signal: 'signal_incomplete', rawPlatformAccountId: '' };
    }
    if (observation.signal === 'authenticated_observed' || attempt + 1 === maxAttempts) break;
    await sleep(ACCOUNT_PROBE_RETRY_DELAY_MS);
  }
  if (observation.signal !== 'authenticated_observed') {
    return { reported: false, reason: 'current_account_not_observed' };
  }
  return sendMessage({
    action: LINGGAN_RUNTIME_ACTION.REPORT_ACCOUNT_ELIGIBILITY,
    ...observation,
  });
}
