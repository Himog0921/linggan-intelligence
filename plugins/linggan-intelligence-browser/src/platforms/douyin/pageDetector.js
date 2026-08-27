/**
 * 抖音页面类型检测
 * 基于已验证的 URL 结构（2026-03-24 验证）
 */

export const DY_PAGE_TYPE = {
  VIDEO_DETAIL: 'videoDetail',   // 单个视频页 /video/{id}
  NOTE_DETAIL: 'noteDetail',     // 图文笔记页 /note/{id}
  SEARCH: 'search',              // 搜索结果页 /search/{keyword}
  PROFILE: 'profile',            // 博主主页 /user/{id} 或 /@{username}
  HOME: 'home',                  // 首页 /
  UNKNOWN: 'unknown',
};

/**
 * 检测当前抖音页面类型
 * @returns {{ type: string, url: string, videoId?: string, noteId?: string, userId?: string }}
 */
export function detectDouyinPageType() {
  const url = window.location.href;
  const pathname = window.location.pathname;
  const params = new URLSearchParams(window.location.search);

  // 视频详情页：/video/{id}
  const videoPathMatch = pathname.match(/^\/video\/([A-Za-z0-9_\-]+)/);
  if (videoPathMatch) {
    return { type: DY_PAGE_TYPE.VIDEO_DETAIL, url, videoId: videoPathMatch[1] };
  }

  // 视频弹窗模式：?modal_id={id}（从搜索页/主页打开）
  const modalId = params.get('modal_id');
  if (modalId) {
    return { type: DY_PAGE_TYPE.VIDEO_DETAIL, url, videoId: modalId };
  }

  // 图文笔记页：/note/{id}
  const notePathMatch = pathname.match(/^\/note\/([A-Za-z0-9_\-]+)/);
  if (notePathMatch) {
    return { type: DY_PAGE_TYPE.NOTE_DETAIL, url, noteId: notePathMatch[1] };
  }

  // 博主主页：/user/{userId} 或 /@{username}
  if (/^\/user\/[A-Za-z0-9_\-]+/.test(pathname) || /^\/@[A-Za-z0-9_\-]+/.test(pathname)) {
    const userMatch = pathname.match(/^\/user\/([A-Za-z0-9_\-]+)/) || pathname.match(/^\/@([A-Za-z0-9_\-]+)/);
    return { type: DY_PAGE_TYPE.PROFILE, url, userId: userMatch?.[1] || '' };
  }

  // 搜索结果页：/search/{keyword}
  if (/^\/search/.test(pathname)) {
    return { type: DY_PAGE_TYPE.SEARCH, url };
  }

  // 首页
  if (pathname === '/' || pathname === '') {
    return { type: DY_PAGE_TYPE.HOME, url };
  }

  return { type: DY_PAGE_TYPE.UNKNOWN, url };
}

/**
 * 获取当前搜索页的 tab 类型
 * 抖音搜索页通过 URL 参数 ?type=xxx 区分 tab
 * @returns {'video'|'general'|'user'|'live'|'unknown'}
 */
export function getDouyinSearchTabType() {
  try {
    const params = new URLSearchParams(window.location.search);
    const type = String(params.get('type') || '').trim().toLowerCase();
    if (type === 'video') return 'video';
    if (type === 'general') return 'general';
    if (type === 'user') return 'user';
    if (type === 'live') return 'live';
    return 'unknown';
  } catch {
    return 'unknown';
  }
}

export function getDouyinSearchKeyword(win = window) {
  try {
    const inputValue = String(win.document?.querySelector('[data-e2e="searchbar-input"]')?.value || '').trim();
    if (inputValue) return inputValue;
    const parsed = new URL(win.location.href, win.location.origin);
    const fromPath = parsed.pathname.match(/\/search\/([^/?#]+)/)?.[1] || '';
    return decodeURIComponent(fromPath || '').trim();
  } catch {
    return '';
  }
}

export function detectDouyinSearchBatchContext(win = window) {
  const pathname = String(win.location?.pathname || '');
  const title = String(win.document?.title || '');
  const keyword = getDouyinSearchKeyword(win);
  const isSearchRoute = /^\/search\/[^/?#]+/.test(pathname);
  const isSearchDetailRoute = /^\/(?:video|note)\/[A-Za-z0-9_\-]+\/search\/[^/?#]+/.test(pathname);
  const hasResultCards = (win.document?.querySelectorAll('li a[href*="/video/"], li a[href*="/note/"]').length || 0) >= 5;
  const hasAnyResultCards = (win.document?.querySelectorAll('a[href*="/video/"], a[href*="/note/"]').length || 0) >= 3;
  const hasSearchTabs = Array.from(win.document?.querySelectorAll('*') || []).some((el) => {
    const text = String(el.textContent || '').trim();
    return text === '综合' || text === '视频' || text === '用户' || text === '直播';
  });
  const hasRelatedSearch = Array.from(win.document?.querySelectorAll('*') || []).some((el) => /相关搜索|大家都在搜/.test(String(el.textContent || '').trim()));
  const stableSearchList = Boolean(
    keyword
    && (isSearchRoute || /抖音搜索/.test(title))
    && (hasResultCards || hasSearchTabs || hasRelatedSearch)
  );

  return {
    keyword,
    isSearchRoute,
    isSearchDetailRoute,
    stableSearchList,
  };
}

export function isStrictDouyinDetailPage(url = window.location.href) {
  try {
    const pathname = new URL(url, window.location.origin).pathname;
    return /^\/video\/[A-Za-z0-9_\-]+/.test(pathname) || /^\/note\/[A-Za-z0-9_\-]+/.test(pathname);
  } catch {
    return /^\/video\/[A-Za-z0-9_\-]+/.test(window.location.pathname) || /^\/note\/[A-Za-z0-9_\-]+/.test(window.location.pathname);
  }
}

/**
 * 从 URL 提取视频/笔记 ID
 * @param {string} url
 * @returns {string | null}
 */
export function extractDouyinContentId(url) {
  // /video/{id}
  const videoMatch = url.match(/\/video\/([A-Za-z0-9_\-]+)/);
  if (videoMatch) return videoMatch[1];

  // modal_id={id}
  try {
    const params = new URLSearchParams(new URL(url).search);
    const modalId = params.get('modal_id');
    if (modalId) return modalId;
  } catch { /* 忽略无效 URL */ }

  // /note/{id}
  const noteMatch = url.match(/\/note\/([A-Za-z0-9_\-]+)/);
  if (noteMatch) return noteMatch[1];

  return null;
}
