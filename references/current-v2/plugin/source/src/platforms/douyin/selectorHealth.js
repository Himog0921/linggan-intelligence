import {
  buildSelectorCheck,
  finalizeSelectorPreflight,
  publishSelectorHealthSnapshot,
  queryAny,
} from '../../shared/selectorHealth.js';
import { detectDouyinSearchBatchContext } from './pageDetector.js';
import { detectDouyinSecurityChallenge } from './securityChallenge.js';

const PLATFORM = 'douyin';
const SELECTOR_VERIFIED_AT = '2026-04-28T00:00:00+08:00';

const DETAIL_ACTIONS = new Set([
  'dy_collectVideo',
  'dy_downloadVideo',
  'dy_collectComments',
  'dy_collectCommentImages',
]);

const PROFILE_ACTIONS = new Set([
  'dy_collectAuthor',
]);

const BATCH_ACTIONS = new Set([
  'dy_batchVideos',
  'dy_batchComments',
]);

const DETAIL_SIGNAL_SELECTORS = [
  'video',
  '[data-e2e="video-desc"]',
  '[data-e2e="detail-video-info"]',
  '[data-e2e="video-info"]',
];

const PROFILE_SIGNAL_SELECTORS = [
  '[data-e2e="user-detail"]',
  'img[data-e2e="user-avatar"]',
  '[data-e2e="user-info-fans"]',
];

const PROFILE_LIST_SIGNAL_SELECTORS = [
  '[data-e2e="user-post-list"]',
  '[data-e2e="user-post-item"]',
  'a[href*="/video/"]',
  'a[href*="/note/"]',
];

function detectLocalDouyinPageType(win = globalThis.window) {
  const pathname = String(win?.location?.pathname || '').trim();
  const search = String(win?.location?.search || '').trim();
  const params = new URLSearchParams(search);

  if (
    /^\/video\/[A-Za-z0-9_-]+/.test(pathname)
    || /^\/note\/[A-Za-z0-9_-]+/.test(pathname)
    || Boolean(params.get('modal_id'))
  ) {
    return 'detail';
  }

  if (/^\/user\/[A-Za-z0-9_-]+/.test(pathname) || /^\/@[A-Za-z0-9_-]+/.test(pathname)) {
    return 'profile';
  }

  if (/^\/search(?:\/|$)/.test(pathname)) {
    return 'search';
  }

  return 'unknown';
}

function createPageTypeCheck(expected, actual) {
  const expectedLabel = Array.isArray(expected) ? expected.join('/') : String(expected || '').trim();
  return buildSelectorCheck({
    name: 'pageType',
    ok: Array.isArray(expected) ? expected.includes(actual) : actual === expected,
    selector: expectedLabel,
    detail: actual,
    verifiedAt: SELECTOR_VERIFIED_AT,
  });
}

function finalizeAndPublish(action, win, {
  ok = true,
  code = 'ok',
  message = '',
  checks = [],
} = {}) {
  return publishSelectorHealthSnapshot(
    finalizeSelectorPreflight(PLATFORM, action, {
      ok,
      code,
      message,
      checks,
    }),
    win,
  );
}

function createSelectorMissingResult(action, win, message, checks = []) {
  return finalizeAndPublish(action, win, {
    ok: false,
    code: 'selector_missing',
    message,
    checks,
  });
}

function createPageMismatchResult(action, win, message, checks = []) {
  return finalizeAndPublish(action, win, {
    ok: false,
    code: 'page_mismatch',
    message,
    checks,
  });
}

function buildDomSignalCheck(name, selectors, documentRef, detail = '') {
  return buildSelectorCheck({
    name,
    ok: queryAny(documentRef, selectors),
    selector: Array.isArray(selectors) ? selectors.join(' | ') : String(selectors || '').trim(),
    detail,
    verifiedAt: SELECTOR_VERIFIED_AT,
  });
}

function buildSearchSignalCheck(win) {
  const searchContext = detectDouyinSearchBatchContext(win);
  return buildSelectorCheck({
    name: 'stableSearchList',
    ok: Boolean(searchContext?.stableSearchList),
    selector: 'search tabs | result cards | related search',
    detail: String(searchContext?.keyword || '').trim(),
    verifiedAt: SELECTOR_VERIFIED_AT,
  });
}

function runDetailPreflight(action, { document, win, pageType }) {
  const pageCheck = createPageTypeCheck('detail', pageType);
  if (!pageCheck.ok) {
    return createPageMismatchResult(action, win, '当前不在抖音作品详情页，请先打开单条作品后再操作', [pageCheck]);
  }

  const detailCheck = buildDomSignalCheck(
    'detailDom',
    DETAIL_SIGNAL_SELECTORS,
    document,
    'video/detail metadata',
  );
  if (!detailCheck.ok) {
    return createSelectorMissingResult(action, win, '当前页面缺少抖音详情页信号，建议刷新页面后重试', [pageCheck, detailCheck]);
  }

  return finalizeAndPublish(action, win, {
    ok: true,
    checks: [pageCheck, detailCheck],
  });
}

function runProfilePreflight(action, { document, win, pageType }) {
  const pageCheck = createPageTypeCheck('profile', pageType);
  if (!pageCheck.ok) {
    return createPageMismatchResult(action, win, '当前不在抖音博主页，请先进入目标博主页后再操作', [pageCheck]);
  }

  const profileCheck = buildDomSignalCheck(
    'profileDom',
    PROFILE_SIGNAL_SELECTORS,
    document,
    'author header',
  );
  if (!profileCheck.ok) {
    return createSelectorMissingResult(action, win, '当前页面缺少抖音博主信息信号，建议刷新页面后重试', [pageCheck, profileCheck]);
  }

  return finalizeAndPublish(action, win, {
    ok: true,
    checks: [pageCheck, profileCheck],
  });
}

function runBatchPreflight(action, { document, win, pageType }) {
  const pageCheck = createPageTypeCheck(['profile', 'search'], pageType);
  if (!pageCheck.ok) {
    return createPageMismatchResult(action, win, '当前不在抖音搜索页或博主页，请先进入正确页面后再操作', [pageCheck]);
  }

  if (pageType === 'profile') {
    const profileListCheck = buildDomSignalCheck(
      'profileListDom',
      PROFILE_LIST_SIGNAL_SELECTORS,
      document,
      'profile list',
    );
    if (!profileListCheck.ok) {
      return createSelectorMissingResult(action, win, '当前博主页未识别到作品列表信号，建议滚动加载后重试', [pageCheck, profileListCheck]);
    }

    return finalizeAndPublish(action, win, {
      ok: true,
      checks: [pageCheck, profileListCheck],
    });
  }

  const searchCheck = buildSearchSignalCheck(win);

  if (!searchCheck.ok) {
    return createSelectorMissingResult(action, win, '当前搜索页未识别到稳定结果列表信号，建议等待页面加载完成后重试', [pageCheck, searchCheck]);
  }

  return finalizeAndPublish(action, win, {
    ok: true,
    checks: [pageCheck, searchCheck],
  });
}

function buildSecurityChallengeCheck(documentRef, win) {
  return buildSelectorCheck({
    name: 'securityChallenge',
    ok: !detectDouyinSecurityChallenge({
      root: documentRef,
      href: win?.location?.href || '',
    }),
    selector: 'captcha | verify | safety challenge',
    detail: String(win?.location?.href || '').trim(),
    verifiedAt: SELECTOR_VERIFIED_AT,
  });
}

function createSecurityChallengeResult(action, win, documentRef) {
  return finalizeAndPublish(action, win, {
    ok: false,
    code: 'security_challenge',
    message: '检测到抖音安全验证，请先完成验证后继续操作',
    checks: [buildSecurityChallengeCheck(documentRef, win)],
  });
}

export function runDouyinSelectorPreflight(
  action,
  { params = {}, document = window.document, win = window } = {},
) {
  const normalizedAction = String(action || '').trim();
  if (detectDouyinSecurityChallenge({ root: document, href: win?.location?.href || '' })) {
    return createSecurityChallengeResult(normalizedAction, win, document);
  }

  const pageType = detectLocalDouyinPageType(win);

  if (DETAIL_ACTIONS.has(normalizedAction)) {
    return runDetailPreflight(normalizedAction, { params, document, win, pageType });
  }

  if (PROFILE_ACTIONS.has(normalizedAction)) {
    return runProfilePreflight(normalizedAction, { params, document, win, pageType });
  }

  if (BATCH_ACTIONS.has(normalizedAction)) {
    return runBatchPreflight(normalizedAction, { params, document, win, pageType });
  }

  return finalizeAndPublish(normalizedAction, win, {
    ok: true,
    checks: [createPageTypeCheck(pageType || 'unknown', pageType)],
  });
}

export function runDouyinSelectorBootstrapProbe(
  { document = window.document, win = window } = {},
) {
  const pageType = detectLocalDouyinPageType(win);
  const action = 'bootstrap';

  if (detectDouyinSecurityChallenge({ root: document, href: win?.location?.href || '' })) {
    return createSecurityChallengeResult(action, win, document);
  }

  if (pageType === 'detail') {
    const pageCheck = createPageTypeCheck('detail', pageType);
    const detailCheck = buildDomSignalCheck(
      'detailDom',
      DETAIL_SIGNAL_SELECTORS,
      document,
      'video/detail metadata',
    );
    return finalizeAndPublish(action, win, detailCheck.ok
      ? {
          ok: true,
          code: 'ok',
          checks: [pageCheck, detailCheck],
        }
      : {
          ok: false,
          code: 'selector_missing',
          message: '当前页面缺少抖音详情页信号，建议刷新页面后重试',
          checks: [pageCheck, detailCheck],
        });
  }

  if (pageType === 'profile') {
    const pageCheck = createPageTypeCheck('profile', pageType);
    const profileCheck = buildDomSignalCheck(
      'profileDom',
      PROFILE_SIGNAL_SELECTORS,
      document,
      'author header',
    );
    return finalizeAndPublish(action, win, profileCheck.ok
      ? {
          ok: true,
          code: 'ok',
          checks: [pageCheck, profileCheck],
        }
      : {
          ok: false,
          code: 'selector_missing',
          message: '当前页面缺少抖音博主信息信号，建议刷新页面后重试',
          checks: [pageCheck, profileCheck],
        });
  }

  if (pageType === 'search') {
    const pageCheck = createPageTypeCheck('search', pageType);
    const searchCheck = buildSearchSignalCheck(win);
    return finalizeAndPublish(action, win, searchCheck.ok
      ? {
          ok: true,
          code: 'ok',
          checks: [pageCheck, searchCheck],
        }
      : {
          ok: false,
          code: 'selector_missing',
          message: '当前搜索页未识别到稳定结果列表信号，建议等待页面加载完成后重试',
          checks: [pageCheck, searchCheck],
        });
  }

  return finalizeAndPublish(action, win, {
    ok: true,
    code: 'skipped',
    checks: [createPageTypeCheck(pageType || 'unknown', pageType)],
  });
}
