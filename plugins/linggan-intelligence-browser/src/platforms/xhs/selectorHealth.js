import {
  buildSelectorCheck,
  finalizeSelectorPreflight,
  publishSelectorHealthSnapshot,
} from '../../shared/selectorHealth.js';
import { POPUP_SELECTORS } from './batchShared.js';
import { PAGE_TYPE } from '../../shared/constants.js';

const PROFILE_FEED_SELECTOR = '#userPostedFeeds';
const FEEDS_CONTAINER_SELECTOR = '.feeds-container';
const SELECTOR_VERIFIED_AT = '2026-04-28T00:00:00+08:00';
const SEARCH_FEED_VERIFIED_AT = '2026-06-01T00:00:00+08:00';
const NOTE_DETAIL_SIGNAL_SELECTORS = POPUP_SELECTORS;
const COMMENTS_CONTAINER_SELECTORS = [
  '.comments-container',
  '[class*="comment-list"]',
  '[class*="comment_container"]',
  '.parent-comment',
  '.comment-item',
];
const AUTHOR_PAGE_SELECTORS = [
  '.user-name',
  '.user-redId',
  '.user-desc',
  '.user-image',
];

export function runXhsSelectorPreflight(
  action,
  { params = {}, document = window.document, win = window } = {},
) {
  const actionName = String(action || '').trim();

  switch (actionName) {
    case 'batchNotes':
      return runBatchPreflight('batchNotes', '批量笔记', params, document, win);
    case 'batchComments':
      return runBatchPreflight('batchComments', '批量评论', params, document, win);
    case 'collectCommentImages':
      return runCommentContainerPreflight(actionName, '评论图片区', document, win);
    case 'collectAuthor':
      return runAuthorPreflight(document, win);
    default:
      return publishSelectorHealthSnapshot(
        finalizeSelectorPreflight('xhs', actionName, {
          ok: true,
          code: 'ok',
          checks: [],
        }),
        win,
      );
  }
}

export function runXhsSelectorBootstrapProbe({ document = window.document, win = window } = {}) {
  const pageType = detectLocalXhsPageType(win);

  switch (pageType) {
    case PAGE_TYPE.SEARCH: {
      const check = buildPresenceCheck(
        document,
        'feed_container',
        [FEEDS_CONTAINER_SELECTOR],
        '笔记流容器',
        SEARCH_FEED_VERIFIED_AT,
      );
      return publishSelectorHealthSnapshot(
        finalizeSelectorPreflight('xhs', 'bootstrap', check.ok
          ? {
              ok: true,
              code: 'ok',
              checks: [check],
            }
          : {
              ok: false,
              code: 'selector_missing',
              message: '当前搜索页未识别到笔记流容器，建议等待页面稳定后重试',
              checks: [check],
            }),
        win,
      );
    }
    case PAGE_TYPE.PROFILE: {
      const check = buildPresenceCheck(
        document,
        'author_profile_shell',
        AUTHOR_PAGE_SELECTORS,
        '博主页结构信号',
      );
      return publishSelectorHealthSnapshot(
        finalizeSelectorPreflight('xhs', 'bootstrap', check.ok
          ? {
              ok: true,
              code: 'ok',
              checks: [check],
            }
          : {
              ok: false,
              code: 'selector_missing',
              message: '当前博主页未识别到基础资料区，建议刷新页面后重试',
              checks: [check],
            }),
        win,
      );
    }
    case PAGE_TYPE.NOTE_DETAIL: {
      const check = buildPresenceCheck(
        document,
        'note_detail_shell',
        NOTE_DETAIL_SIGNAL_SELECTORS,
        '笔记详情容器',
      );
      return publishSelectorHealthSnapshot(
        finalizeSelectorPreflight('xhs', 'bootstrap', check.ok
          ? {
              ok: true,
              code: 'ok',
              checks: [check],
            }
          : {
              ok: false,
              code: 'selector_missing',
              message: '当前笔记页未识别到详情容器信号，建议刷新页面后重试',
              checks: [check],
            }),
        win,
      );
    }
    default:
      return publishSelectorHealthSnapshot(
        finalizeSelectorPreflight('xhs', 'bootstrap', {
          ok: true,
          code: 'skipped',
          checks: [],
        }),
        win,
      );
  }
}

function runBatchPreflight(action, label, params, document, win) {
  const mode = String(params?.mode || '').trim().toLowerCase();
  const isProfileMode = mode === 'profile';
  const selector = isProfileMode ? PROFILE_FEED_SELECTOR : FEEDS_CONTAINER_SELECTOR;
  const check = buildPresenceCheck(
    document,
    isProfileMode ? 'profile_feed_container' : 'feed_container',
    [selector],
    isProfileMode ? '博主页笔记流容器' : '笔记流容器',
    isProfileMode ? SELECTOR_VERIFIED_AT : SEARCH_FEED_VERIFIED_AT,
  );

  return publishSelectorHealthSnapshot(
    finalizeSelectorPreflight('xhs', action, check.ok
      ? {
          ok: true,
          code: 'ok',
          checks: [check],
        }
      : {
          ok: false,
          code: 'selector_missing',
          message: `${label}页面结构已变化，未找到${check.detail}`,
          checks: [check],
        }),
    win,
  );
}

function runCommentContainerPreflight(action, label, document, win) {
  const check = buildPresenceCheck(
    document,
    'comments_container',
    COMMENTS_CONTAINER_SELECTORS,
    '评论容器',
  );

  return publishSelectorHealthSnapshot(
    finalizeSelectorPreflight('xhs', action, check.ok
      ? {
          ok: true,
          code: 'ok',
          checks: [check],
        }
      : {
          ok: false,
          code: 'selector_missing',
          message: `${label}页面结构已变化，未找到${check.detail}`,
          checks: [check],
        }),
    win,
  );
}

function runAuthorPreflight(document, win) {
  const check = buildPresenceCheck(
    document,
    'author_profile_shell',
    AUTHOR_PAGE_SELECTORS,
    '博主页结构信号',
  );

  return publishSelectorHealthSnapshot(
    finalizeSelectorPreflight('xhs', 'collectAuthor', check.ok
      ? {
          ok: true,
          code: 'ok',
          checks: [check],
        }
      : {
          ok: false,
          code: 'selector_missing',
          message: `博主页页面结构已变化，未找到${check.detail}`,
          checks: [check],
        }),
    win,
  );
}

function buildPresenceCheck(document, name, selectors, detail, verifiedAt = SELECTOR_VERIFIED_AT) {
  const selectorList = Array.isArray(selectors) ? selectors : [selectors];
  const matchedSelector = selectorList.find((selector) => hasSelector(document, selector)) || '';
  return buildSelectorCheck({
    name,
    ok: Boolean(matchedSelector),
    selector: matchedSelector || selectorList.join(' | '),
    detail,
    verifiedAt,
  });
}

function hasSelector(document, selector) {
  try {
    return Boolean(document?.querySelector?.(selector));
  } catch {
    return false;
  }
}

function detectLocalXhsPageType(win = globalThis.window) {
  const href = String(win?.location?.href || '').trim();
  const pathname = String(win?.location?.pathname || '').trim();

  if (/\/explore\/[a-z0-9]+/i.test(pathname) || /\/discovery\/item\/[a-z0-9]+/i.test(pathname)) {
    return PAGE_TYPE.NOTE_DETAIL;
  }
  if (/\/search_result/.test(pathname) || href.includes('keyword=')) {
    return PAGE_TYPE.SEARCH;
  }
  if (/\/user\/profile\/[a-z0-9]+/i.test(pathname)) {
    return PAGE_TYPE.PROFILE;
  }
  if (pathname === '/' || pathname === '/explore') {
    return PAGE_TYPE.EXPLORE;
  }
  return PAGE_TYPE.UNKNOWN;
}
