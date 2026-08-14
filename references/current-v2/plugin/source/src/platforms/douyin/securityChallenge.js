const SECURITY_TEXT_PATTERNS = [
  /(安全验证|请完成验证|验证码|滑块|拖动滑块|向右滑动|旋转验证|异常访问|访问受限|操作频繁)/i,
  /\b(captcha|verify|verification|security|risk)\b/i,
];

const SECURITY_SELECTORS = [
  '.captcha-verify-container',
  '.captcha_verify_container',
  '.secsdk-captcha',
  '[class*="captcha"]',
  '[class*="verify"]',
  '[data-e2e*="captcha"]',
  '[data-e2e*="verify"]',
  'iframe[src*="captcha"]',
  'iframe[src*="verify"]',
];

const DEFAULT_USER_MESSAGE = '检测到抖音安全验证，请先完成验证后点击“继续”。';

function stringifyPayload(payload = null) {
  if (!payload || typeof payload !== 'object') return String(payload || '').trim();
  const candidates = [
    payload.status_msg,
    payload.statusMessage,
    payload.message,
    payload.msg,
    payload.description,
    payload.detail,
    payload.error,
    payload.err_msg,
    payload.error_msg,
    payload.verify_msg,
  ];
  const picked = candidates
    .map((value) => String(value || '').trim())
    .filter(Boolean)
    .join(' ');
  if (picked) return picked;
  try {
    return JSON.stringify(payload);
  } catch {
    return '';
  }
}

function isVisibleElement(node) {
  if (!node) return false;
  if (node.offsetHeight > 0 || node.offsetWidth > 0) return true;
  if (typeof node.getBoundingClientRect === 'function') {
    const rect = node.getBoundingClientRect();
    return Number(rect?.height || 0) > 0 || Number(rect?.width || 0) > 0;
  }
  return false;
}

export function matchesDouyinSecurityChallengeText(text = '') {
  const value = String(text || '').trim();
  if (!value) return false;
  return SECURITY_TEXT_PATTERNS.some((pattern) => pattern.test(value));
}

export function matchesDouyinSecurityChallengeUrl(url = '') {
  const value = String(url || '').trim();
  if (!value) return false;
  return /(captcha|verify|verification|security|risk)/i.test(value);
}

export function detectDouyinSecurityChallenge({
  root = globalThis.document || null,
  href = globalThis.window?.location?.href || '',
  selectors = SECURITY_SELECTORS,
} = {}) {
  if (matchesDouyinSecurityChallengeUrl(href)) return true;
  if (!root || typeof root.querySelector !== 'function') return false;

  for (const selector of selectors) {
    try {
      const node = root.querySelector(selector);
      if (isVisibleElement(node)) return true;
    } catch {
      // ignore invalid selector in unsupported environments
    }
  }

  const text = String(
    root?.body?.innerText
    || root?.documentElement?.innerText
    || root?.body?.textContent
    || ''
  ).trim();
  return matchesDouyinSecurityChallengeText(text);
}

export function createDouyinSecurityChallengeError({
  statusCode = 0,
  message = DEFAULT_USER_MESSAGE,
  reason = 'status_code',
  payloadText = '',
} = {}) {
  const error = new Error(String(message || DEFAULT_USER_MESSAGE));
  error.name = 'DouyinSecurityChallengeError';
  error.code = 'douyin_security_verification';
  error.statusCode = Number(statusCode || 0) || 0;
  error.reason = String(reason || 'status_code');
  error.payloadText = String(payloadText || '').trim();
  error.userMessage = DEFAULT_USER_MESSAGE;
  return error;
}

export function isDouyinSecurityChallengeError(error) {
  if (!error) return false;
  return error.name === 'DouyinSecurityChallengeError'
    || error.code === 'douyin_security_verification';
}

export function maybeCreateDouyinSecurityChallengeError({
  statusCode = 0,
  payload = null,
  root = globalThis.document || null,
  href = globalThis.window?.location?.href || '',
} = {}) {
  const normalizedStatusCode = Number(statusCode || 0);
  if (!Number.isFinite(normalizedStatusCode) || normalizedStatusCode === 0) return null;

  const payloadText = stringifyPayload(payload);
  const hasPayloadSignal = matchesDouyinSecurityChallengeText(payloadText);
  const hasPageSignal = detectDouyinSecurityChallenge({ root, href });
  if (!hasPayloadSignal && !hasPageSignal) return null;

  return createDouyinSecurityChallengeError({
    statusCode: normalizedStatusCode,
    reason: 'status_code',
    payloadText,
  });
}

export async function pauseForDouyinSecurityChallenge(error, {
  current = 0,
  total = 0,
  scannedImages = 0,
  formatProgress = null,
  onPause = null,
  waitIfPaused = async () => {},
  shouldStop = () => false,
} = {}) {
  if (!isDouyinSecurityChallengeError(error) || typeof onPause !== 'function') {
    return {
      handled: false,
      stopped: false,
      message: '',
    };
  }

  let progressText = '';
  if (typeof formatProgress === 'function') {
    progressText = String(formatProgress({
      current: Number(current || 0),
      total: Number(total || 0),
      scannedImages: Number(scannedImages || 0),
    }) || '').trim();
  } else {
    const progressHints = [];
    if (Number(current || 0) > 0) progressHints.push(`当前已采集 ${Number(current || 0)} 条评论`);
    if (Number(scannedImages || 0) > 0) progressHints.push(`已发现 ${Number(scannedImages || 0)} 张图片`);
    if (Number(total || 0) > 0 && Number(current || 0) > 0) progressHints.push(`上限 ${Number(total || 0)}`);
    progressText = progressHints.join('，');
  }

  const message = progressText
    ? `${error.userMessage}${progressText}。`
    : error.userMessage;

  await onPause({
    message,
    current: Number(current || 0),
    total: Number(total || 0),
    scannedImages: Number(scannedImages || 0),
    reason: String(error.reason || 'status_code'),
  });
  await waitIfPaused();

  return {
    handled: true,
    stopped: Boolean(shouldStop()),
    message,
  };
}
