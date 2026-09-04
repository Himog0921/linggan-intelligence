import { LINGGAN_RUNTIME_ACTION } from './runtimeActions.js';

const XHS_ORIGINS = new Set(['xiaohongshu.com', 'www.xiaohongshu.com']);
const XHS_PROFILE_PATH = /^\/user\/profile\/([a-f0-9]{16,32})\/?$/i;

function normalizeMarker(value) {
  return String(value || '').replace(/\s+/g, '').trim();
}

function isCurrentAccountLink(link) {
  if (!link || typeof link.getAttribute !== 'function') return false;
  return [link.textContent, link.getAttribute('aria-label'), link.getAttribute('title')]
    .map(normalizeMarker)
    .some((value) => value === '我' || value === '我的');
}

/**
 * Read only the explicit current-account link already rendered in XHS navigation. A creator
 * profile's `__INITIAL_STATE__.user.userInfo` describes the viewed creator, so it must never
 * be used as the execution account identity.
 */
export function currentAccountHrefFromDocument(doc) {
  if (!doc || typeof doc.querySelectorAll !== 'function') return '';
  for (const link of doc.querySelectorAll('a[href]')) {
    if (!isCurrentAccountLink(link)) continue;
    const href = String(link.getAttribute('href') || '').trim();
    if (href) return href;
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
  if (!XHS_ORIGINS.has(url.hostname.toLowerCase())) {
    return { signal: 'signal_incomplete', rawPlatformAccountId: '' };
  }
  const match = url.pathname.match(XHS_PROFILE_PATH);
  const rawPlatformAccountId = String(match?.[1] || '').trim();
  if (!rawPlatformAccountId) return { signal: 'signal_incomplete', rawPlatformAccountId: '' };
  return { signal: 'authenticated_observed', rawPlatformAccountId };
}

/**
 * Read only the current account fact already present in an open XHS page. This never navigates,
 * opens a tab, reads cookies, starts collection, or retains the raw id after the message resolves.
 */
export async function reportPassiveAccountEligibility({ readCurrentAccountHref, sendMessage }) {
  let observation = { signal: 'signal_incomplete', rawPlatformAccountId: '' };
  try {
    observation = accountObservationFromCurrentAccountHref(await readCurrentAccountHref());
  } catch {
    // Absence of a trustworthy page signal is itself a closed, fail-closed observation.
  }
  return sendMessage({
    action: LINGGAN_RUNTIME_ACTION.REPORT_ACCOUNT_ELIGIBILITY,
    ...observation,
  });
}
