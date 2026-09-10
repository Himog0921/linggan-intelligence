const LOGIN_REQUIRED_PATTERN = /使用已登录.*小红书.*扫码验证身份|小红书\s*APP.*扫码验证身份|扫码验证身份/;
const ACCESS_RESTRICTED_PATTERN = /账号(?:已)?(?:被)?(?:限制|禁用)|访问(?:受限|异常)|当前账号异常/;
const COOLDOWN_PATTERN = /操作(?:过于)?频繁|请(?:稍后|休息后)再试/;

// Only platform status surfaces may supply a negative account fact. Reading body.innerText is
// unsafe: a note title or comment can contain the same words without saying anything about the
// browser session. These accessibility surfaces are outside normal note/comment text containers
// and model a visible platform prompt rather than a user-authored string.
const PLATFORM_ACCOUNT_STATUS_SELECTOR = [
  '[role="dialog"][aria-modal="true"]',
  '[role="alert"]',
  '[aria-live="assertive"]',
  '[data-testid="login"]',
  '[data-testid="risk-control"]',
  '[data-testid="account-restricted"]',
].join(',');

function platformAccountStatusText(doc) {
  if (typeof doc?.querySelectorAll !== 'function') return '';
  return [...doc.querySelectorAll(PLATFORM_ACCOUNT_STATUS_SELECTOR)]
    .map((surface) => String(surface?.innerText || surface?.textContent || '').trim())
    .filter(Boolean)
    .join('\n')
    .slice(0, 3000);
}

/**
 * This existing page-level signal is shared by capability checks and account observation. It
 * reads only already-rendered text; it does not touch cookies, network APIs, or navigation.
 */
export function hasXhsAppScanVerification(win = {}) {
  return LOGIN_REQUIRED_PATTERN.test(platformAccountStatusText(win?.document));
}

/**
 * Return only an explicit, actionable negative fact. Missing or changing page chrome is
 * intentionally inconclusive: that is not evidence that an account has logged out.
 */
export function explicitXhsAccountBlockObservation(doc) {
  const text = platformAccountStatusText(doc);
  if (LOGIN_REQUIRED_PATTERN.test(text)) return { signal: 'login_required' };
  if (ACCESS_RESTRICTED_PATTERN.test(text)) return { signal: 'access_restricted' };
  if (COOLDOWN_PATTERN.test(text)) return { signal: 'cooldown_observed' };
  return null;
}
