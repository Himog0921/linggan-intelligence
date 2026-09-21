import { PAGE_TYPE } from './constants.js';

const SELECTOR_HEALTH_KEY = '__lgboomSelectorHealth';
const SELECTOR_HEALTH_ALERT_KEY = '__lgboomSelectorHealthAlerts';
/// 页面把快照交给宿主（扩展的 background）的出口。由内容脚本入口装一次；
/// 没装（例如单元测试里自己造的 `win`）时，一切照旧，只是不上报。
const SELECTOR_HEALTH_REPORTER_KEY = '__lgboomSelectorHealthReporter';
const THIRTY_DAYS_MS = 30 * 24 * 60 * 60 * 1000;
const DEFAULT_ALERT_DEDUPE_MS = 5 * 60 * 1000;
const MAX_ALERT_HISTORY = 20;

/// 上报快照的字段（闭集）。
///
/// 这份快照会离开浏览器，所以它只带**受限词**：平台、页面类型、能力、两个时刻，以及
/// 本次检查过的 / 缺的 / 验证日期陈旧的**检查项名**。它不带选择器串、DOM 文本、页面 URL，
/// 也不带检查项的中文描述——那些是这个页面上的原文。
///
/// 测试按这份表逐字断言，因此「快照里还有别的字段」是不可能悄悄发生的。
export const SELECTOR_HEALTH_SNAPSHOT_FIELDS = Object.freeze([
  'platform',
  'pageType',
  'capability',
  'checkedAt',
  'verifiedAt',
  'checkedCategories',
  'missingCategories',
  'staleCategories',
]);

/// 受限词：小写字母、数字、下划线，1–32 字符。服务端另有一份同规则的校验，
/// 两边都在做同一件事不是重复：这里防的是「本机把不该出门的东西发出去」，
/// 那边防的是「不管谁发来的都按受限形状收」。
const RESTRICTED_TOKEN = /^[a-z0-9_]{1,32}$/;
/// 时刻：只接受可能是日期时间的形状（ISO 8601 / RFC 3339 及其 Z 或 ±HH:MM 偏移）。
const BOUNDED_MOMENT = /^[0-9A-Za-z:+.-]{1,40}$/;
const MAX_CATEGORIES = 16;

/// 动作 → 它守住的能力。今日取值都来自 `background.js` 的 `IMPLEMENTED_CAPABILITIES`
/// （`page_probe` 除外：`bootstrap` 是一次页面探针，不对应任何一项采集能力，
/// 如实写 unknown 而不是编一个能力名）。
const SELECTOR_ACTION_CAPABILITIES = Object.freeze({
  batchNotes: 'discovery_search',
  batchComments: 'comments',
  collectCommentImages: 'media_slots',
  collectAuthor: 'author_profile',
  bootstrap: 'unknown',
});

/// 页面自己的 `PAGE_TYPE` → 上报词表。两套词表本来就在同一个插件里各写各的
/// （`noteDetail` 是页面探测的写法，`note_detail` 是 TaskSpec 的写法），
/// 上报只认后者，转换只此一处。
const PAGE_TYPE_LITERALS = Object.freeze({
  [PAGE_TYPE.NOTE_DETAIL]: 'note_detail',
  [PAGE_TYPE.SEARCH]: 'search_results',
  [PAGE_TYPE.PROFILE]: 'profile',
  [PAGE_TYPE.EXPLORE]: 'explore',
  // 抖音的详情页在 TaskSpec 里本来就叫 `detail`，不并进 `note_detail`。
  detail: 'detail',
});

/// 上报词表本身（= `PAGE_TYPE_LITERALS` 的取值，加上 `unknown`）。
///
/// 它同时是**第二次**收口：页面报上来的字面值再收一次仍然落在同一个集合里
/// （`noteDetail` 会被换成 `note_detail`，而 `note_detail` 原样留下），
/// 因此 background 与页面之间传的是什么形状，出浏览器的是什么形状，只有一个答案。
const REPORTED_PAGE_TYPES = new Set([...Object.values(PAGE_TYPE_LITERALS), 'unknown']);

export function restrictedToken(value) {
  const token = String(value || '').trim().toLowerCase();
  return RESTRICTED_TOKEN.test(token) ? token : '';
}

/// 页面自己探测出来的页面类型 → 上报词表里的字面值。认不出来就写 unknown。
export function pageTypeLiteral(pageType) {
  return PAGE_TYPE_LITERALS[String(pageType || '').trim()] || 'unknown';
}

/// 页面说的页面类型 → 上报词表。两种写法都认：本机的页面探测写法（`noteDetail`）与
/// 已经在词表里的字面值（`note_detail`）——收两次仍然是同一个答案，认不出来的写 unknown。
function reportedPageType(value) {
  const literal = pageTypeLiteral(value);
  if (literal !== 'unknown') return literal;
  const token = restrictedToken(value);
  return REPORTED_PAGE_TYPES.has(token) ? token : 'unknown';
}

function boundedMoment(value) {
  const text = String(value || '').trim();
  return BOUNDED_MOMENT.test(text) ? text : 'unknown';
}

function categoryList(values) {
  const seen = new Set();
  for (const value of Array.isArray(values) ? values : []) {
    // 认不出来的检查项名不丢：写成 `unclassified`（「有一个检查项，但名字不是受限形状」），
    // 丢掉它等于把「这次检查失败了」这件事从快照里删掉。
    seen.add(restrictedToken(value) || 'unclassified');
    if (seen.size >= MAX_CATEGORIES) break;
  }
  return [...seen];
}

/// 一份快照，或 `null`（连平台名都没有的东西不是快照）。
///
/// 这是**唯一**把任意输入收成受限快照的地方：页面构造它、background 收到上报后也用它
/// 再收一次，因此「出了浏览器的形状」只有一种。
export function normalizeSelectorHealthSnapshot(candidate) {
  if (!candidate || typeof candidate !== 'object' || Array.isArray(candidate)) return null;
  const platform = restrictedToken(candidate.platform);
  if (!platform) return null;
  return {
    platform,
    pageType: reportedPageType(candidate.pageType),
    capability: restrictedToken(candidate.capability) || 'unknown',
    checkedAt: boundedMoment(candidate.checkedAt),
    verifiedAt: boundedMoment(candidate.verifiedAt),
    checkedCategories: categoryList(candidate.checkedCategories),
    missingCategories: categoryList(candidate.missingCategories),
    staleCategories: categoryList(candidate.staleCategories),
  };
}

/// 一次 preflight 结果 → 上报快照。
export function buildSelectorHealthSnapshot(result) {
  if (!result || typeof result !== 'object') return null;
  const checks = Array.isArray(result.checks) ? result.checks.filter(Boolean) : [];
  // 上次人工验证日期取**最早**的那一份：一次 preflight 可能同时看几个检查项，其中有
  // 一项从四月起没人重验过，就不能拿另一项的六月说「这些选择器是六月验过的」。
  const verified = checks
    .map((check) => String(check?.verifiedAt || '').trim())
    .filter(Boolean)
    .sort();
  return normalizeSelectorHealthSnapshot({
    platform: result.platform,
    pageType: result.pageType,
    capability: SELECTOR_ACTION_CAPABILITIES[String(result.action || '').trim()] || 'unknown',
    checkedAt: Number.isFinite(Number(result.checkedAt))
      ? new Date(Number(result.checkedAt)).toISOString()
      : '',
    verifiedAt: verified[0] || '',
    checkedCategories: checks.map((check) => check?.name),
    missingCategories: checks.filter((check) => check.ok === false).map((check) => check?.name),
    staleCategories: checks.filter((check) => check.stale === true).map((check) => check?.name),
  });
}

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
  pageType = '',
} = {}) {
  return {
    platform: String(platform || '').trim(),
    action: String(action || '').trim(),
    // 这次检查发生在哪类页面。由知道这件事的调用方给（探测结果或动作本身的语义）；
    // 没人给就是 unknown——不从检查项名字里猜。
    pageType: pageTypeLiteral(pageType),
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

  reportSelectorHealthSnapshot(result, win);

  if (result && !result.ok && typeof console !== 'undefined' && typeof console.warn === 'function') {
    console.warn('[灵感爆爆爆] Selector preflight blocked', result);
  }
  return result;
}

/// 把这次检查的受限快照交给宿主（装了才交）。
///
/// 上报失败不能影响任何采集动作：诊断是旁注，页面该做什么还做什么。它也从不在
/// 这里等待结果——页面不因为「服务端收没收下诊断」改变自己的行为。
function reportSelectorHealthSnapshot(result, win) {
  const reporter = win?.[SELECTOR_HEALTH_REPORTER_KEY];
  if (typeof reporter !== 'function') return;
  try {
    const snapshot = buildSelectorHealthSnapshot(result);
    if (!snapshot) return;
    const outcome = reporter(snapshot);
    if (outcome && typeof outcome.catch === 'function') outcome.catch(() => {});
  } catch {
    // 见上：诊断不改变页面行为。
  }
}

/// 内容脚本入口调用一次：之后每次 preflight 都会把快照交给它。
export function installSelectorHealthReporter(reporter, win = globalThis.window) {
  if (!win || typeof reporter !== 'function') return false;
  win[SELECTOR_HEALTH_REPORTER_KEY] = reporter;
  return true;
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
