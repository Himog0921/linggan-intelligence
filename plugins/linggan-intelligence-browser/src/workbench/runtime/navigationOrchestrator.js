const NAVIGATION_TIMEOUT_MS = 30000;

function decodePossiblyEncodedKeyword(target = '') {
  let value = String(target || '').trim();
  for (let i = 0; i < 2; i += 1) {
    if (!/%[0-9a-f]{2}/i.test(value)) break;
    try {
      const decoded = decodeURIComponent(value);
      if (!decoded || decoded === value) break;
      value = decoded;
    } catch {
      break;
    }
  }
  return value;
}

function buildXhsSearchUrl(target, options = {}) {
  const keyword = decodePossiblyEncodedKeyword(target);
  const sortMode = String(options.sortMode || '').trim();
  const sortParam = sortMode === 'hot' ? '&sort=hot' : sortMode === 'time' ? '&sort=time' : '';
  return `https://www.xiaohongshu.com/search_result?keyword=${encodeURIComponent(keyword)}${sortParam}`;
}

function buildXhsProfileUrl(target) {
  const value = String(target || '').trim();
  if (!value) return null;
  return isHttpUrl(value) ? value : `https://www.xiaohongshu.com/user/profile/${encodeURIComponent(value)}`;
}

function isHttpUrl(value = '') {
  return /^https?:\/\//i.test(String(value || '').trim());
}

function isXhsDetailUrl(url = '') {
  const value = String(url || '').trim();
  return (
    /^https?:\/\/(?:www\.)?xiaohongshu\.com\/(?:explore|discovery\/item|search_result)\/[^/?#]+/i.test(value) ||
    /^https?:\/\/(?:www\.)?xiaohongshu\.com\/user\/profile\/[^/?#]+\/[^/?#]+/i.test(value)
  );
}

function extractXhsNoteId(value = '') {
  const raw = String(value || '').trim();
  if (!raw) return '';
  if (/^[a-z0-9]{20,32}$/i.test(raw)) return raw;
  try {
    const url = new URL(raw);
    const match =
      url.pathname.match(/\/(?:explore|discovery\/item|search_result|item)\/([a-z0-9]+)/i) ||
      url.pathname.match(/\/user\/profile\/[^/?#]+\/([a-z0-9]+)/i);
    return match?.[1] || '';
  } catch {
    return '';
  }
}

function buildXhsDetailUrl(target) {
  const value = String(target || '').trim();
  const noteId = extractXhsNoteId(value);
  if (!noteId) return isHttpUrl(value) ? value : null;
  const url = new URL(`https://www.xiaohongshu.com/discovery/item/${encodeURIComponent(noteId)}`);
  url.searchParams.set('source', 'webshare');
  url.searchParams.set('xhsshare', 'pc_web');
  try {
    const original = new URL(value);
    const token = original.searchParams.get('xsec_token');
    if (token) url.searchParams.set('xsec_token', token);
  } catch {
    // note id inputs do not carry xsec_token; the page can still open as a canonical detail URL.
  }
  url.searchParams.set('xsec_source', 'pc_share');
  return url.toString();
}

function isDouyinDetailUrl(url = '') {
  return /^https?:\/\/(?:www\.)?douyin\.com\/(?:video|note)\/[^/?#]+/i.test(String(url || '').trim());
}

function shouldPreserveDetailTarget(taskType, target, options = {}) {
  const normalizedTarget = String(target || '').trim();
  if (!isHttpUrl(normalizedTarget)) return false;
  if (String(options.targetPageType || '').trim() === 'detail') return true;
  if (taskType === 'xhs.batchNotes' || taskType === 'xhs.list_scan') {
    return isXhsDetailUrl(normalizedTarget);
  }
  if (taskType === 'douyin.batchNotes') return isDouyinDetailUrl(normalizedTarget);
  return false;
}

const TASK_URL_BUILDERS = {
  'xhs.batchNotes': (target, options = {}) => {
    if (isHttpUrl(target)) return String(target || '').trim();
    if (String(options.targetPageType || '').trim() === 'profile') {
      return buildXhsProfileUrl(target);
    }
    return shouldPreserveDetailTarget('xhs.batchNotes', target, options)
      ? String(target || '').trim()
      : buildXhsSearchUrl(target, options);
  },
  'xhs.list_scan': (target, options = {}) => {
    if (String(options.targetPageType || '').trim() === 'profile') {
      return buildXhsProfileUrl(target);
    }
    if (shouldPreserveDetailTarget('xhs.list_scan', target, options)) {
      return buildXhsDetailUrl(target) || String(target || '').trim();
    }
    if (isHttpUrl(target) && /\/user\/profile\//i.test(String(target || ''))) {
      return String(target || '').trim();
    }
    return buildXhsSearchUrl(target, options);
  },
  'xhs.note_full': (target) => buildXhsDetailUrl(target),
  'xhs.batchComments': (target, options = {}) =>
    /^https?:\/\//i.test(target)
      ? target
      : String(options.targetPageType || '').trim() === 'profile'
        ? buildXhsProfileUrl(target)
        : buildXhsSearchUrl(target, options),
  'xhs.comment_scan': (target) => buildXhsDetailUrl(target),
  'xhs.collectAuthor': (target) =>
    /^https?:\/\//i.test(target) ? target : buildXhsProfileUrl(target),
  'xhs.authorNoteLinks': (target) =>
    /^https?:\/\//i.test(target) ? target : buildXhsProfileUrl(target),
  'xhs.author_profile': (target) =>
    /^https?:\/\//i.test(target) ? target : buildXhsProfileUrl(target),
  'xhs.author_links': (target) =>
    /^https?:\/\//i.test(target) ? target : buildXhsProfileUrl(target),
  'douyin.batchNotes': (target, options = {}) =>
    shouldPreserveDetailTarget('douyin.batchNotes', target, options)
      ? String(target || '').trim()
      : `https://www.douyin.com/search/${encodeURIComponent(target)}`,
  'douyin.batchComments': (target) =>
    /^https?:\/\//i.test(target) ? target : `https://www.douyin.com/search/${encodeURIComponent(target)}`,
  'douyin.collectAuthor': (target) =>
    /^https?:\/\//i.test(target) ? target : `https://www.douyin.com/user/${target}`,
  'douyin.singleComments': (target) =>
    /^https?:\/\//i.test(target) ? target : null,
  'douyin.commentImageDownload': (target) =>
    /^https?:\/\//i.test(target) ? target : null,
};

export function buildTaskNavigationUrl(taskType = '', target = '', options = {}) {
  const builder = TASK_URL_BUILDERS[String(taskType || '').trim()];
  if (typeof builder !== 'function') return null;
  const url = builder(String(target || '').trim(), options);
  if (!url || !/^https?:\/\//i.test(url)) return null;
  return url;
}

export function navigateToTask(taskType, target, options = {}) {
  return new Promise((resolve) => {
    const url = buildTaskNavigationUrl(taskType, target, options);
    if (!url) {
      resolve({ tabId: null, error: 'cannot_build_url' });
      return;
    }

    let completed = false;

    chrome.windows.create({ url, focused: false, type: 'normal' }, async (createdWindow) => {
      if (chrome.runtime.lastError || !createdWindow?.id) {
        resolve({ tabId: null, error: 'tab_create_failed' });
        return;
      }

      let tabId = Number(createdWindow?.tabs?.[0]?.id || 0) || null;
      if (!tabId) {
        try {
          const tabs = await chrome.tabs.query({
            windowId: createdWindow.id,
            active: true,
          });
          tabId = Number(tabs?.[0]?.id || 0) || null;
        } catch {
          tabId = null;
        }
      }

      if (!tabId) {
        resolve({ tabId: null, error: 'tab_create_failed' });
        return;
      }

      chrome.tabs.update(tabId, { autoDiscardable: false }).catch(() => {});

      const timeout = setTimeout(() => {
        if (completed) return;
        completed = true;
        chrome.tabs.onUpdated.removeListener(listener);
        // V1.1（2026-06-29）：导航超时后关闭标签页，防止僵尸窗口累积。
        closeTab(tabId).catch(() => {});
        resolve({ tabId, error: 'navigation_timeout', timedOut: true });
      }, NAVIGATION_TIMEOUT_MS);

      function listener(updatedTabId, changeInfo) {
        if (updatedTabId !== tabId || changeInfo.status !== 'complete') return;
        if (completed) return;
        completed = true;
        clearTimeout(timeout);
        chrome.tabs.onUpdated.removeListener(listener);
        resolve({ tabId, error: null, windowId: createdWindow.id });
      }

      chrome.tabs.onUpdated.addListener(listener);
    });
  });
}

export async function closeTab(tabId) {
  if (!tabId) return;
  await chrome.tabs.remove(tabId).catch(() => {});
}
