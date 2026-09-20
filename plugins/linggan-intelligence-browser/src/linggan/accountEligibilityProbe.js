import { explicitXhsAccountBlockObservation } from '../platforms/xhs/accountObservation.js';
import { LINGGAN_RUNTIME_ACTION } from './runtimeActions.js';

const XHS_ORIGINS = new Set(['xiaohongshu.com', 'www.xiaohongshu.com']);
const XHS_PROFILE_PATH = /^\/user\/profile\/([a-f0-9]{16,32})\/?$/i;
const CURRENT_ACCOUNT_NAVIGATION_SELECTOR = 'nav, [role="navigation"], ul, ol, [role="list"]';
const REQUIRED_GLOBAL_NAVIGATION_PATHS = new Set(['/explore', '/notification', '/chat']);

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

/**
 * Read only the explicit current-account link already rendered in XHS navigation. A creator
 * profile's `__INITIAL_STATE__.user.userInfo` describes the viewed creator, never the account
 * that holds the browser session.
 */
export function currentAccountHrefFromDocument(doc) {
  if (!doc || typeof doc.querySelectorAll !== 'function') return '';
  for (const link of doc.querySelectorAll('a[href]')) {
    if (!hasCurrentAccountMarker(link) || !isGlobalNavigationScope(link)) continue;
    const href = String(link.getAttribute('href') || '').trim();
    if (accountObservationFromCurrentAccountHref(href)) return href;
  }
  return '';
}

export function accountObservationFromCurrentAccountHref(href) {
  let url;
  try {
    url = new URL(String(href || '').trim(), 'https://www.xiaohongshu.com');
  } catch {
    return null;
  }
  if (url.protocol !== 'https:' || !XHS_ORIGINS.has(url.hostname.toLowerCase())) return null;
  const rawPlatformAccountId = String(url.pathname.match(XHS_PROFILE_PATH)?.[1] || '').trim();
  return rawPlatformAccountId ? { signal: 'authenticated_observed', rawPlatformAccountId } : null;
}

/**
 * A page observation is either a verified current-account identity, an explicit block page, or
 * inconclusive. It never turns absent navigation into a negative server fact.
 */
export function observeXhsAccountFromDocument(doc) {
  const authenticated = accountObservationFromCurrentAccountHref(currentAccountHrefFromDocument(doc));
  return authenticated || explicitXhsAccountBlockObservation(doc);
}

// This is deliberately narrower than generic account eligibility: only the
// explicit platform cooldown prompt on an already-open task page is allowed to
// contribute to the plugin circuit breaker.
export function observeXhsDetailRiskFromDocument(doc) {
  return explicitXhsAccountBlockObservation(doc)?.signal === 'cooldown_observed'
    ? { signal: 'risk_control_interstitial', detectorVersion: 'xhs-risk-interstitial.v1' }
    : null;
}

/**
 * Natural page load reporting is a single DOM read. No retry loop, alarm, tab enumeration or
 * platform request is introduced; inconclusive DOM state simply sends nothing.
 */
export async function reportPassiveAccountEligibility({ document: doc = globalThis.document, sendMessage } = {}) {
  if (typeof sendMessage !== 'function') return { reported: false, reason: 'account_observation_not_ready' };
  const observation = observeXhsAccountFromDocument(doc);
  if (!observation) return { reported: false, reason: 'account_observation_inconclusive' };
  return sendMessage({
    action: LINGGAN_RUNTIME_ACTION.REPORT_ACCOUNT_ELIGIBILITY,
    observation,
  });
}
