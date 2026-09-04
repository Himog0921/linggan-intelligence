import { LINGGAN_RUNTIME_ACTION } from './runtimeActions.js';

export function accountObservationFromUserInfo(userInfo) {
  if (!userInfo || typeof userInfo !== 'object' || Array.isArray(userInfo)) {
    return { signal: 'signal_incomplete', rawPlatformAccountId: '' };
  }
  const rawPlatformAccountId = String(
    userInfo.userId || userInfo.user_id || userInfo.id || '',
  ).trim();
  if (!rawPlatformAccountId || rawPlatformAccountId.length > 512) {
    return { signal: 'signal_incomplete', rawPlatformAccountId: '' };
  }
  return { signal: 'authenticated_observed', rawPlatformAccountId };
}

/**
 * Read only the current account fact already present in an open XHS page. This never navigates,
 * opens a tab, reads cookies, starts collection, or retains the raw id after the message resolves.
 */
export async function reportPassiveAccountEligibility({ readPageUser, sendMessage }) {
  let observation = { signal: 'signal_incomplete', rawPlatformAccountId: '' };
  try {
    const pageState = await readPageUser();
    observation = accountObservationFromUserInfo(pageState?.userInfo);
  } catch {
    // Absence of a trustworthy page signal is itself a closed, fail-closed observation.
  }
  return sendMessage({
    action: LINGGAN_RUNTIME_ACTION.REPORT_ACCOUNT_ELIGIBILITY,
    ...observation,
  });
}
