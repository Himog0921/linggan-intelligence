import { PAGE_TYPE } from '../../shared/constants.js';

/**
 * 检测当前页面类型（小红书）
 * 根据 URL 和 DOM 特征判断
 */
export function detectPageType() {
  const url = window.location.href;
  const pathname = window.location.pathname;

  // 笔记详情页：/explore/xxx 或 /discovery/item/xxx
  if (/\/explore\/[a-z0-9]+/i.test(pathname) ||
      /\/discovery\/item\/[a-z0-9]+/i.test(pathname)) {
    return { type: PAGE_TYPE.NOTE_DETAIL, url };
  }

  // 小红书签名分享有时会走 /user/profile/{authorId}/{noteId}?xsec_token=...
  // 这类页面虽然路径像博主主页，实际打开的是单篇作品详情。
  if (/\/user\/profile\/[a-z0-9]+\/[a-z0-9]+/i.test(pathname) && /xsec_token=/i.test(url)) {
    return { type: PAGE_TYPE.NOTE_DETAIL, url };
  }

  // 搜索结果页
  if (/\/search_result/.test(pathname) || url.includes('keyword=')) {
    return { type: PAGE_TYPE.SEARCH, url };
  }

  // 博主主页：/user/profile/xxx
  if (/\/user\/profile\/[a-z0-9]+/i.test(pathname)) {
    return { type: PAGE_TYPE.PROFILE, url };
  }

  // 发现页
  if (pathname === '/' || pathname === '/explore') {
    return { type: PAGE_TYPE.EXPLORE, url };
  }

  return { type: PAGE_TYPE.UNKNOWN, url };
}
