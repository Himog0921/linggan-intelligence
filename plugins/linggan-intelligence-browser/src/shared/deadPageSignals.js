/**
 * 失效页面 title 关键词表。任务目标是某条具体作品，但当前页面 title
 * 显示为死页时命中，用于判定 CONTENT_NOT_FOUND。
 */
export const DEAD_PAGE_TITLE_PATTERN =
  /页面不见了|暂时无法浏览|无法浏览|已删除|已私密|页面不存在|访问的页面|作品不存在|视频不可见|已失效/;

export function looksLikeDeadPageTitle(title = '') {
  return DEAD_PAGE_TITLE_PATTERN.test(String(title || '').trim());
}

/**
 * A scheduled detail-page task names one concrete XHS work.  This detector
 * deliberately reads only the browser's final URL and document title: note
 * body/comment text is user content and must never manufacture a platform
 * failure.  A positive result means the *signed execution URL* is no longer
 * usable, not that the work itself has been deleted permanently.
 */
export function explicitXhsDetailPageUrlInvalid({
  currentUrl = '',
  title = '',
  expectedContentExternalId = '',
} = {}) {
  if (!String(expectedContentExternalId || '').trim()) return false;
  let url;
  try {
    url = new URL(String(currentUrl || '').trim());
  } catch {
    return false;
  }
  if (url.protocol !== 'https:' || !/(^|\.)xiaohongshu\.com$/i.test(url.hostname)) return false;
  // XHS redirects an expired/invalid signed detail URL to this platform-owned
  // surface.  It is a stronger fact than any collector exception.
  if (url.pathname === '/explore' && url.searchParams.get('source') === '404') return true;
  return looksLikeDeadPageTitle(title);
}
