const SELECTOR_HEALTH_KEY = '__lgboomSelectorHealth';
const SELECTOR_HEALTH_ALERT_KEY = '__lgboomSelectorHealthAlerts';
const THIRTY_DAYS_MS = 30 * 24 * 60 * 60 * 1000;
const DEFAULT_ALERT_DEDUPE_MS = 5 * 60 * 1000;
const MAX_ALERT_HISTORY = 20;

export function isSelectorVerificationStale(verifiedAt = '', now = Date.now()) {
  const timestamp = Date.parse(String(verifiedAt || '').trim());
  if (!Number.isFinite(timestamp)) return false;
  return now - timestamp > THIRTY_DAYS_MS;
}

export function buildSelectorCheck({
  name = '',
  ok = false,
  selector = '',
  detail = '',
  verifiedAt = '',
} = {}) {
  return {
    name: String(name || '').trim(),
    ok: Boolean(ok),
    selector: String(selector || '').trim(),
    detail: String(detail || '').trim(),
    verifiedAt: String(verifiedAt || '').trim(),
    stale: isSelectorVerificationStale(verifiedAt),
  };
}

export function finalizeSelectorPreflight(platform, action, {
  ok = true,
  code = 'ok',
  message = '',
  checks = [],
} = {}) {
  return {
    platform: String(platform || '').trim(),
    action: String(action || '').trim(),
    ok: Boolean(ok),
    code: String(code || (ok ? 'ok' : 'selector_missing')).trim(),
    message: String(message || '').trim(),
    checks: Array.isArray(checks) ? checks : [],
    staleChecks: (Array.isArray(checks) ? checks : []).filter((check) => check?.stale),
    missingChecks: (Array.isArray(checks) ? checks : []).filter((check) => check && check.ok === false),
    checkedAt: Date.now(),
  };
}

export function publishSelectorHealthSnapshot(result, win = globalThis.window) {
  if (!win) return result;
  const current = win[SELECTOR_HEALTH_KEY] && typeof win[SELECTOR_HEALTH_KEY] === 'object'
    ? win[SELECTOR_HEALTH_KEY]
    : {};
  const platform = String(result?.platform || '').trim() || 'unknown';
  const action = String(result?.action || '').trim() || 'unknown';
  const platformState = current[platform] && typeof current[platform] === 'object'
    ? current[platform]
    : {};

  win[SELECTOR_HEALTH_KEY] = {
    ...current,
    [platform]: {
      ...platformState,
      [action]: result,
      lastUpdatedAt: Date.now(),
    },
  };

  if (result && !result.ok && typeof console !== 'undefined' && typeof console.warn === 'function') {
    console.warn('[灵感爆爆爆] Selector preflight blocked', result);
  }
  return result;
}

function normalizeCheckList(checks = []) {
  return Array.isArray(checks) ? checks.filter(Boolean) : [];
}

function summarizeCheckDetails(checks = []) {
  return normalizeCheckList(checks)
    .map((check) => String(check?.detail || check?.name || check?.selector || '').trim())
    .filter(Boolean)
    .slice(0, 3)
    .join('、');
}

export function buildSelectorHealthAlertMessage(result) {
  if (!result || typeof result !== 'object') return '';
  const staleChecks = normalizeCheckList(result.staleChecks);
  const missingChecks = normalizeCheckList(result.missingChecks);

  if (result.ok === false) {
    const message = String(result.message || '').trim();
    if (message) return message;
    const detail = summarizeCheckDetails(missingChecks);
    return detail
      ? `当前页面结构信号异常：${detail}，建议刷新页面后重试`
      : '当前页面结构信号异常，建议刷新页面后重试';
  }

  if (staleChecks.length > 0) {
    const detail = summarizeCheckDetails(staleChecks);
    return detail
      ? `页面结构校验已超过 30 天：${detail}，建议回归验证当前页面结构`
      : '页面结构校验已超过 30 天，建议回归验证当前页面结构';
  }

  return '';
}

function buildAlertFingerprint(result, message) {
  const platform = String(result?.platform || '').trim() || 'unknown';
  const action = String(result?.action || '').trim() || 'unknown';
  const code = String(result?.code || '').trim() || (result?.ok === false ? 'selector_missing' : 'stale');
  const checkNames = normalizeCheckList([
    ...(result?.missingChecks || []),
    ...(result?.staleChecks || []),
  ])
    .map((check) => String(check?.name || '').trim())
    .filter(Boolean)
    .join('|');
  return [platform, action, code, checkNames, String(message || '').trim()].join('::');
}

export function consumeSelectorHealthAlertMessage(
  result,
  {
    win = globalThis.window,
    now = Date.now(),
    dedupeMs = DEFAULT_ALERT_DEDUPE_MS,
  } = {},
) {
  const message = buildSelectorHealthAlertMessage(result);
  if (!message || !win) return message;

  const current = win[SELECTOR_HEALTH_ALERT_KEY] && typeof win[SELECTOR_HEALTH_ALERT_KEY] === 'object'
    ? win[SELECTOR_HEALTH_ALERT_KEY]
    : {};
  const fingerprint = buildAlertFingerprint(result, message);
  const last = current[fingerprint];
  if (last && Number(now) - Number(last.at || 0) < dedupeMs) {
    return '';
  }

  const entries = Object.entries(current)
    .filter(([, value]) => Number(now) - Number(value?.at || 0) < dedupeMs * 3)
    .slice(-(MAX_ALERT_HISTORY - 1));

  win[SELECTOR_HEALTH_ALERT_KEY] = Object.fromEntries([
    ...entries,
    [fingerprint, { at: Number(now), message }],
  ]);

  return message;
}

export function queryAny(documentRef, selectors = []) {
  return (Array.isArray(selectors) ? selectors : [selectors]).some((selector) => {
    try {
      return Boolean(documentRef?.querySelector?.(selector));
    } catch {
      return false;
    }
  });
}
