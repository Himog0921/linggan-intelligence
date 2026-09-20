import { MSG, COMMENT_DEPTH_MODE } from '../shared/constants.js';
import { sendToTab as sendSharedToTab } from '../shared/messaging.js';
import { unwrapCompatResponseData } from '../shared/responseEnvelope.js';

export const PLATFORM = {
  XHS: 'xhs',
  DOUYIN: 'douyin',
  UNKNOWN: 'unknown',
};

export const PAGE_MODE = {
  DETAIL: 'detail',
  PROFILE: 'profile',
  SEARCH: 'search',
  UNKNOWN: 'unknown',
};

export function detectPlatformByUrl(url) {
  try {
    const parsed = new URL(url);
    const host = parsed.hostname || '';
    if (host.includes('xiaohongshu.com')) return PLATFORM.XHS;
    if (host.includes('douyin.com')) return PLATFORM.DOUYIN;
    return PLATFORM.UNKNOWN;
  } catch {
    return PLATFORM.UNKNOWN;
  }
}

export function isDouyinStrictDetailUrl(url) {
  return /\/video\/[A-Za-z0-9_\-]+/.test(url)
    || /\/note\/[A-Za-z0-9_\-]+/.test(url);
}

export function isDouyinVideoUrl(url) {
  return /\/video\/[A-Za-z0-9_\-]+/.test(url)
    || /[?&]modal_id=/.test(url)
    || /\/note\/[A-Za-z0-9_\-]+/.test(url);
}

export function getModeFromUrl(url, platform = PLATFORM.XHS) {
  if (platform === PLATFORM.DOUYIN) {
    if (/\/user\/|\/@/.test(url)) return PAGE_MODE.PROFILE;
    if (/\/search/.test(url)) return PAGE_MODE.SEARCH;
    if (isDouyinVideoUrl(url)) return PAGE_MODE.DETAIL;
    return PAGE_MODE.UNKNOWN;
  }
  if (/\/user\/profile\//.test(url)) return PAGE_MODE.PROFILE;
  if (/\/search_result/.test(url)) return PAGE_MODE.SEARCH;
  if (/\/explore\/|\/discovery\/item\//.test(url)) return PAGE_MODE.DETAIL;
  return PAGE_MODE.UNKNOWN;
}

export function getPageCapabilities(platform, mode, options = {}) {
  const { isDyVideoPage = false, isDyStrictDetailPage = false, isStableSearchList = false } = options;
  if (platform === PLATFORM.XHS) {
    const isProfile = mode === PAGE_MODE.PROFILE;
    const isDetail = mode === PAGE_MODE.DETAIL;
    return {
      canCollectPrimary: isDetail,
      canCollectSecondary: isDetail || isProfile,
      canDownloadCommentImages: false,
      canBatchNotes: mode === PAGE_MODE.SEARCH || mode === PAGE_MODE.PROFILE,
      canBatchComments: mode === PAGE_MODE.SEARCH || mode === PAGE_MODE.PROFILE,
      secondaryAction: isProfile ? 'author' : (isDetail ? 'comment' : 'none'),
      isDyVideoPage,
      isDyStrictDetailPage,
      isStableSearchList,
    };
  }
  if (platform === PLATFORM.DOUYIN) {
    return {
      canCollectPrimary: isDyVideoPage,
      canCollectSecondary: isDyVideoPage || mode === PAGE_MODE.PROFILE,
      canDownloadCommentImages: isDyStrictDetailPage,
      canBatchNotes: mode === PAGE_MODE.PROFILE || (mode === PAGE_MODE.SEARCH && isStableSearchList),
      canBatchComments: mode === PAGE_MODE.PROFILE || (mode === PAGE_MODE.SEARCH && isStableSearchList),
      secondaryAction: isDyVideoPage ? 'comment' : (mode === PAGE_MODE.PROFILE ? 'author' : 'none'),
      isDyVideoPage,
      isDyStrictDetailPage,
      isStableSearchList,
    };
  }
  return {
    canCollectPrimary: false,
    canCollectSecondary: false,
    canDownloadCommentImages: false,
    canBatchNotes: false,
    canBatchComments: false,
    secondaryAction: 'comment',
    isDyVideoPage,
    isDyStrictDetailPage,
    isStableSearchList,
  };
}

export function getPrimaryActionWarning(platform, mode, capabilities) {
  if (platform === PLATFORM.DOUYIN && !capabilities.canCollectPrimary) {
    return '请先进入抖音视频详情页或弹层页，再点击"采集当前视频"。';
  }
  if (platform === PLATFORM.XHS && mode !== PAGE_MODE.DETAIL) {
    return '请先进入小红书笔记详情页，再采集当前笔记。';
  }
  return '当前页面暂不支持该操作。';
}

export function getSecondaryActionWarning(platform, mode, capabilities) {
  if (platform === PLATFORM.DOUYIN && capabilities.secondaryAction === 'author') {
    return '请先进入抖音博主页，再采集当前博主。';
  }
  if (platform === PLATFORM.XHS && capabilities.secondaryAction === 'author') {
    return '请先进入小红书博主页，再采集当前博主。';
  }
  if (platform === PLATFORM.DOUYIN) {
    return '请先进入抖音视频详情页或弹层页，再采集当前评论。';
  }
  if (platform === PLATFORM.XHS && mode !== PAGE_MODE.DETAIL) {
    return '请先进入小红书笔记详情页，再采集当前评论。';
  }
  return '当前页面暂不支持该操作。';
}

export function getBatchActionWarning(platform, mode, capabilities = {}) {
  if (platform === PLATFORM.DOUYIN) {
    if (mode === PAGE_MODE.DETAIL) {
      return '抖音批量任务请在搜索结果页或博主页发起，详情页只适合单条动作。';
    }
    if (mode === PAGE_MODE.SEARCH && !capabilities.isStableSearchList) {
      return '当前抖音页面还没形成稳定搜索列表，请等待列表加载完成后再发起批量任务。';
    }
    return '当前抖音页面还不是稳定批量场景，请切到搜索结果页或博主页后重试。';
  }
  if (platform === PLATFORM.XHS) {
    if (mode === PAGE_MODE.DETAIL) {
      return '小红书批量任务请在搜索结果页或博主页发起，详情页只适合单条动作。';
    }
    return '当前小红书页面还不是稳定批量场景，请切到搜索结果页或博主页后重试。';
  }
  return '当前页面暂不支持批量任务。';
}

export function getErrorMessage(err) {
  if (!err) return '';
  if (typeof err === 'string') return err;
  if (typeof err.message === 'string') return err.message;
  return String(err);
}

export function isContextError(msg) {
  return /Extension context invalidated|context invalidated|Could not establish connection|Receiving end does not exist|message port closed/i.test(msg);
}

export function toFriendlyError(err) {
  const msg = getErrorMessage(err);
  if (isContextError(msg)) {
    return '插件刚更新或页面连接已断开，请刷新当前页面后再点一次，刷新后即可继续。';
  }
  if (/account_busy|同一账号正在执行另一个采集任务/.test(msg)) {
    return '这个平台账号正在执行另一条采集，请等当前任务结束后再试。';
  }
  if (/login_required|needs_login|login_expired|账号登录状态不可用|bad_cookie/.test(msg)) {
    return '当前平台账号需要重新登录，请先在对应平台页面完成登录，再回到插件继续采集。';
  }
  if (/permission_denied|page_permission_denied|missing_permission|缺少.*权限|权限缺失/.test(msg)) {
    return '浏览器助手缺少当前平台页面权限，请重新授权插件后再试。';
  }
  if (/未找到笔记数据|笔记详情页/.test(msg)) {
    return '当前不是完整笔记详情页，请先打开笔记详情后再采集。';
  }
  return msg || '操作失败，请稍后重试。';
}

export function formatMaintenanceStats(stats = {}) {
  const notes = Number(stats.notes || 0);
  const comments = Number(stats.comments || 0);
  const authors = Number(stats.authors || 0);
  const mediaAssets = Number(stats.mediaAssets || 0);
  const total = Number(stats.total || 0);
  if (total <= 0) {
    return '数据维护完成：当前历史数据已是最新结构，无需回填。';
  }
  const parts = [];
  if (notes > 0) parts.push(`笔记 ${notes}`);
  if (comments > 0) parts.push(`评论 ${comments}`);
  if (authors > 0) parts.push(`博主 ${authors}`);
  if (mediaAssets > 0) parts.push(`媒体 ${mediaAssets}`);
  return `数据维护完成：共回填 ${total} 条，${parts.join('，')}。`;
}

export function escapeHtml(text = '') {
  return String(text)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

function getFlywheelTrackingMeta(record = {}) {
  const qualityReason = record?.qualityReason;
  return {
    dataQuality: String(record?.dataQuality || '').trim(),
    qualityReason: qualityReason == null ? '' : String(qualityReason),
    sourceTier: String(record?.sourceTier || '').trim(),
    collectionRunId: String(record?.collectionRunId || '').trim(),
  };
}

export function inferProgressStage({ statusText = '', taskState = '', stage = '', current = 0, total = 0, error = null } = {}) {
  const text = String(statusText || '').trim();
  const normalizedTaskState = String(taskState || '').trim();
  const normalizedStage = String(stage || '').trim();
  if (normalizedTaskState === 'error') {
    return { label: '失败', className: 'is-error', description: error?.message || '任务执行失败，请检查提示后重试。' };
  }
  if (normalizedTaskState === 'paused') {
    return { label: '已暂停', className: 'is-paused', description: '任务已暂停，可以继续或停止。' };
  }
  if (normalizedTaskState === 'stopping') {
    return { label: '停止中', className: 'is-warning', description: '任务正在完成当前步骤并安全停止。' };
  }
  if (/暂停/.test(text)) {
    return { label: '已暂停', className: 'is-paused', description: '任务已暂停，可以继续或停止。' };
  }
  if (normalizedStage === 'downloading' || /下载|打包/.test(text)) {
    return { label: '下载中', className: 'is-running', description: '当前任务已进入下载或打包阶段。' };
  }
  if (normalizedStage === 'discovering' || /扫描|发现|滚动/.test(text)) {
    return { label: '扫描中', className: 'is-running', description: '正在发现目标内容或扫描素材，请稍候。' };
  }
  if (normalizedStage === 'context_check' || /启动|准备/.test(text) || (current === 0 && total > 0)) {
    return { label: '准备中', className: '', description: '任务已经发起，正在建立本轮执行上下文。' };
  }
  if (total > 0 && current >= total) {
    return { label: '处理中', className: 'is-running', description: '任务已接近完成，请继续等待最终结果。' };
  }
  return { label: '进行中', className: 'is-running', description: '任务正在执行中，可在这里查看进度和控制。' };
}

export function sendToTab(tabId, payload, options = {}) {
  return sendSharedToTab(tabId, payload, {
    autoReconnect: true,
    ...options,
  });
}

export function unwrapTabResponseData(result, fallback) {
  return unwrapCompatResponseData(result, fallback);
}

export function sendToBackground(action, payload, { timeoutMs = 15000 } = {}) {
  return new Promise((resolve, reject) => {
    let settled = false;
    const timer = Number.isFinite(Number(timeoutMs)) && Number(timeoutMs) > 0
      ? setTimeout(() => {
        settled = true;
        reject(new Error(`插件后台暂时没有回应，请刷新扩展后再试：${String(action || 'unknown_action')}`));
      }, Number(timeoutMs))
      : null;

    chrome.runtime.sendMessage({ action, ...payload }, (response) => {
      if (settled) return;
      settled = true;
      if (timer) clearTimeout(timer);
      const runtimeErr = chrome.runtime.lastError;
      if (runtimeErr) {
        reject(new Error(getErrorMessage(runtimeErr)));
        return;
      }
      if (response?.error) {
        reject(new Error(response.error));
        return;
      }
      resolve(response || { success: true });
    });
  });
}

export function mapNoteToFlywheel(note) {
  return {
    contentId: note.contentId || '',
    platform: note.platform || 'xhs',
    noteId: note.platformContentId || note.noteId || '',
    url: note.canonicalUrl || note.url || '',
    title: note.title || '',
    bodyText: note.bodyText || note.content || '',
    type: note.type || 'normal',
    cover: note.cover || note.coverImg || '',
    likes: note.likes || 0,
    collects: note.collects || note.bookmarks || 0,
    comments_count: note.comments_count || note.commentCount || 0,
    shares: note.shares || note.shareCount || 0,
    authorId: note.authorEntityId || note.authorId || '',
    authorName: note.authorName || note.author || '',
    authorAvatar: note.authorAvatar || note.avatar || '',
    keywords: Array.isArray(note.keywords) ? note.keywords.join(',') : (note.keywords || ''),
    hashtags: Array.isArray(note.hashtags) ? note.hashtags.join(',') : (note.hashtags || ''),
    publishedAt: note.publishedAt || 0,
    collectedAt: note.collectedAt || Date.now(),
    ipLocation: note.ipLocation || note.location || '',
    ...getFlywheelTrackingMeta(note),
    syncStatus: 'synced',
    lastSyncAt: Date.now(),
  };
}

export function mapCommentToFlywheel(comment) {
  return {
    commentId: comment.commentId || comment.platformCommentId || '',
    contentId: comment.contentId || '',
    noteId: comment.noteId || '',
    platform: comment.platform || 'xhs',
    text: comment.text || comment.content || '',
    author: comment.author || comment.authorName || comment.userName || '',
    authorId: comment.authorEntityId || comment.authorId || '',
    likes: comment.likes || comment.likeCount || 0,
    parentCommentId: comment.parentCommentId || '',
    rootCommentId: comment.rootCommentId || '',
    level: comment.level || 1,
    replyToCommentId: comment.replyToCommentId || '',
    replyToUserName: comment.replyToUserName || '',
    publishedAt: comment.publishedAt || 0,
    collectedAt: comment.collectedAt || Date.now(),
    ...getFlywheelTrackingMeta(comment),
    syncStatus: 'synced',
  };
}

export function mapAuthorToFlywheel(author) {
  return {
    authorEntityId: author.authorEntityId || '',
    platform: author.platform || 'xhs',
    userId: author.userId || author.platformAuthorId || '',
    handle: author.handle || author.redId || author.douyinId || '',
    name: author.name || author.authorName || '',
    avatar: author.avatar || author.avatarUrl || '',
    description: author.description || author.bio || '',
    fans: author.fans || author.followers || 0,
    follows: author.follows || author.following || 0,
    interactions: author.interactions || author.noteCount || 0,
    ipLocation: author.ipLocation || author.location || '',
    profileUrl: author.profileUrl || author.url || '',
    collectedAt: author.collectedAt || Date.now(),
    ...getFlywheelTrackingMeta(author),
    syncStatus: 'synced',
  };
}

export function getPageContextText(platform, mode, options = {}) {
  const { isDyVideoPage = false, isDyStrictDetailPage = false, isStableSearchList = false } = options;
  const tags = [];
  let scene = '未识别页面';
  let hint = '请切换到小红书或抖音页面，再执行采集任务。';

  if (platform === PLATFORM.XHS) {
    if (mode === PAGE_MODE.DETAIL) {
      scene = '小红书笔记详情页';
      hint = '适合执行单篇笔记和单篇评论。评论图片区当前仅支持页内入口，不在 Popup 中提供。';
      tags.push('单条可用', '评论可用');
    } else if (mode === 'profile') {
      scene = '小红书博主页';
      hint = '适合手动采集当前博主，也适合发起批量笔记和批量评论。';
      tags.push('博主可用', '批量可用');
    } else if (mode === 'search') {
      scene = '小红书搜索/列表页';
      hint = '适合从当前可见列表发起批量笔记或批量评论。';
      tags.push('批量可用', '顺位采集');
    } else {
      scene = '小红书页面';
      hint = '当前页面可用能力取决于是否处于详情页或稳定列表页。';
    }
  } else if (platform === PLATFORM.DOUYIN) {
    if (isDyStrictDetailPage) {
      scene = '抖音详情页';
      hint = '单条视频、评论和评论图片区都适合在这里执行，稳定性最高。';
      tags.push('单条可用', '评论可用', '图片素材可用');
    } else if (isDyVideoPage) {
      scene = '抖音视频弹层页';
      hint = '适合采集当前视频与评论；评论图片区建议先进入真正详情页再执行。';
      tags.push('单条可用', '分享采集可用');
    } else if (mode === 'profile') {
      scene = '抖音博主页';
      hint = '适合博主采集、批量视频和批量评论。';
      tags.push('批量可用', '博主可用');
    } else if (mode === 'search') {
      scene = '抖音搜索结果页';
      hint = isStableSearchList
        ? '当前已识别为稳定搜索列表，可执行批量视频或批量评论。'
        : '当前仍未识别为稳定搜索列表，暂不建议从这里发起批量任务。';
      tags.push(isStableSearchList ? '搜索批量可用' : '搜索列表待确认', '顺位/Top N');
    } else {
      scene = '抖音页面';
      hint = '当前页面可用能力取决于是否处于详情页、弹层页或稳定搜索列表。';
    }
  }

  return { scene, hint, tags };
}
