import { createCollectorEvidence, createCollectorQualityMeta, joinRawDomText } from '../../shared/collectorMetadata.js';
import {
  createDouyinSecurityChallengeError,
  detectDouyinSecurityChallenge,
  isDouyinSecurityChallengeError,
  maybeCreateDouyinSecurityChallengeError,
} from './securityChallenge.js';

const API_COMMON = 'device_platform=webapp&aid=6383&channel=channel_pc_web';
const PAGE_BRIDGE_SOURCE = 'lgboom-dy-api-capture';
const PAGE_FETCH_REQUEST_SOURCE = 'lgboom-dy-content';
const PAGE_FETCH_REQUEST_TYPE = '__lgboom_dy_page_fetch_request__';
const PAGE_FETCH_RESPONSE_TYPE = '__lgboom_dy_page_fetch_response__';

export function dedupeStrings(values = []) {
  const result = [];
  const seen = new Set();
  for (const raw of values) {
    const value = String(raw || '').trim();
    if (!value || seen.has(value)) continue;
    seen.add(value);
    result.push(value);
  }
  return result;
}

export function normalizeRemoteUrl(raw = '') {
  const value = String(raw || '').trim();
  if (!value) return '';
  if (value.startsWith('//')) return `${window.location.protocol}${value}`;
  if (value.startsWith('/')) return `${window.location.origin}${value}`;
  return value;
}

export function normalizeRemoteCandidates(candidates = []) {
  const list = Array.isArray(candidates) ? candidates : [candidates];
  const result = [];
  for (const raw of list) {
    const value = normalizeRemoteUrl(raw);
    if (!value) continue;
    if (!result.includes(value)) result.push(value);
    const noQuery = value.split('?')[0];
    if (noQuery && !result.includes(noQuery)) result.push(noQuery);
  }
  return result.filter((value) => /^https?:\/\//i.test(value));
}

export function looksLikeImageUrl(raw = '') {
  const value = String(raw || '').trim();
  if (!/^https?:\/\//i.test(value) && !/^\/\//.test(value)) return false;
  if (/\.(png|jpe?g|webp|gif|avif)(\?|$)/i.test(value)) return true;
  return /(douyinpic|byteimg|ibyteimg|tos-cn|p3-sign|p6-sign|p26-sign|image)/i.test(value);
}

function toProfileUrl(user = {}) {
  const secUid = String(user?.sec_uid || user?.secUid || '').trim();
  if (secUid) return `https://www.douyin.com/user/${encodeURIComponent(secUid)}`;
  return '';
}

function toAvatarUrl(user = {}) {
  return normalizeRemoteUrl(
    user?.avatar_thumb?.url_list?.[0]
    || user?.avatar_medium?.url_list?.[0]
    || user?.avatar_larger?.url_list?.[0]
    || ''
  );
}

function parsePublishedAt(raw) {
  const value = Number(raw || 0);
  if (!Number.isFinite(value) || value <= 0) return 0;
  return value < 1000000000000 ? value * 1000 : value;
}

function formatPublishedAtText(ts) {
  const value = Number(ts || 0);
  if (!Number.isFinite(value) || value <= 0) return '';
  try {
    return new Date(value).toISOString();
  } catch {
    return '';
  }
}

function getCommentCursor(payload = {}) {
  // 优先使用 next_cursor / next_offset，避免重复请求同一页
  const candidates = [
    payload.next_cursor,
    payload.next_offset,
    payload.cursor,
    payload.offset,
  ];
  for (const raw of candidates) {
    const value = Number(raw);
    if (Number.isFinite(value) && value >= 0) return value;
  }
  return 0;
}

function hasMoreComments(payload = {}) {
  const candidates = [
    payload.has_more,
    payload.hasMore,
    payload.has_more_comments,
  ];
  for (const value of candidates) {
    if (value === true || value === 1 || value === '1') return true;
    if (value === false || value === 0 || value === '0') return false;
  }
  return false;
}

async function fetchDouyinJsonViaBridge(urls = [], timeoutMs = 10000) {
  const candidates = (Array.isArray(urls) ? urls : [])
    .map((item) => String(item || '').trim())
    .filter(Boolean);
  if (candidates.length === 0) return null;

  const requestId = `dy_page_fetch_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;

  return new Promise((resolve, reject) => {
    let settled = false;
    const cleanup = () => {
      window.removeEventListener('message', onMessage);
      clearTimeout(timer);
    };
    const finishResolve = (value) => {
      if (settled) return;
      settled = true;
      cleanup();
      resolve(value);
    };
    const finishReject = (error) => {
      if (settled) return;
      settled = true;
      cleanup();
      reject(error);
    };
    const onMessage = (event) => {
      try {
        if (event.source !== window) return;
        const data = event.data || {};
        if (data.source !== PAGE_BRIDGE_SOURCE) return;
        if (data.type !== PAGE_FETCH_RESPONSE_TYPE) return;
        const payload = data.payload || {};
        if (payload.requestId !== requestId) return;
        if (payload.ok && payload.json) {
          finishResolve(payload.json);
          return;
        }
        finishReject(new Error(payload.error || 'page_fetch_failed'));
      } catch (err) {
        finishReject(err);
      }
    };
    const timer = window.setTimeout(() => {
      finishReject(new Error('page_fetch_timeout'));
    }, timeoutMs);

    window.addEventListener('message', onMessage);

    try {
      window.postMessage({
        source: PAGE_FETCH_REQUEST_SOURCE,
        type: PAGE_FETCH_REQUEST_TYPE,
        payload: {
          requestId,
          urls: candidates,
        },
      }, '*');
    } catch (err) {
      finishReject(err);
    }
  });
}

export async function fetchDouyinJson(urls = []) {
  try {
    const bridged = await fetchDouyinJsonViaBridge(urls);
    if (bridged) return bridged;
  } catch {
    // 页面桥接失败时，继续回退到 content fetch
  }

  let lastError = null;
  for (const url of urls) {
    try {
      const response = await fetch(url, { credentials: 'include' });
      if (!response.ok) {
        lastError = new Error(`HTTP ${response.status}`);
        continue;
      }
      const json = await response.json();
      const statusCode = Number(json?.status_code ?? json?.statusCode ?? 0);
      if (Number.isFinite(statusCode) && statusCode !== 0) {
        lastError = maybeCreateDouyinSecurityChallengeError({
          statusCode,
          payload: json,
          root: globalThis.document || null,
          href: globalThis.window?.location?.href || '',
        }) || new Error(`status_code=${statusCode}`);
        continue;
      }
      return json;
    } catch (err) {
      if (isDouyinSecurityChallengeError(err)) {
        lastError = err;
        continue;
      }
      lastError = detectDouyinSecurityChallenge({
        root: globalThis.document || null,
        href: globalThis.window?.location?.href || '',
      })
        ? createDouyinSecurityChallengeError({ reason: 'dom_signal' })
        : err;
    }
  }
  throw lastError || new Error('fetch_failed');
}

export async function fetchCommentListPage(awemeId, { cursor = 0, count = 20, sortMode = 'hot' } = {}) {
  const sortType = sortMode === 'latest' ? 1 : 0;
  const queryBase = `${API_COMMON}&aweme_id=${encodeURIComponent(awemeId)}&cursor=${cursor}&count=${count}&item_type=0&insert_ids=&sort_type=${sortType}`;
  const urls = [
    `/aweme/v1/web/comment/list/?${queryBase}&whale_cut_token=&cut_version=1`,
    `/aweme/v1/web/comment/list/?${queryBase}`,
    `/aweme/v1/web/comment/list/?${API_COMMON}&item_id=${encodeURIComponent(awemeId)}&cursor=${cursor}&count=${count}&sort_type=${sortType}`,
  ];
  const json = await fetchDouyinJson(urls);
  return {
    comments: json?.comments || json?.comment_list || [],
    cursor: getCommentCursor(json),
    hasMore: hasMoreComments(json),
    raw: json,
  };
}

export async function fetchReplyListPage(awemeId, commentId, { cursor = 0, count = 20 } = {}) {
  const urls = [
    `/aweme/v1/web/comment/list/reply/?${API_COMMON}&item_id=${encodeURIComponent(awemeId)}&comment_id=${encodeURIComponent(commentId)}&cursor=${cursor}&count=${count}`,
    `/aweme/v1/web/comment/list/reply/?${API_COMMON}&aweme_id=${encodeURIComponent(awemeId)}&comment_id=${encodeURIComponent(commentId)}&cursor=${cursor}&count=${count}`,
  ];
  const json = await fetchDouyinJson(urls);
  return {
    comments: json?.comments || json?.reply_comments || json?.comment_list || [],
    cursor: getCommentCursor(json),
    hasMore: hasMoreComments(json),
    raw: json,
  };
}

function parseReplyTarget(comment = {}) {
  const replyToCommentId = String(
    comment?.reply_id
    || comment?.reply_comment_id
    || comment?.reply_to_comment_id
    || comment?.reply_comment?.cid
    || ''
  ).trim();
  const replyToUserName = String(
    comment?.reply_to_user?.nickname
    || comment?.reply_user?.nickname
    || comment?.reply_comment?.user?.nickname
    || ''
  ).trim();
  return { replyToCommentId, replyToUserName };
}

export function mapDouyinCommentRecord(comment = {}, note = {}, {
  parseCount,
  parentCommentId = '',
  rootCommentId = '',
  level = 1,
  sortMode = 'hot',
  positionIndex = 0,
  collectionRunId = '',
} = {}) {
  const commentId = String(comment?.cid || comment?.comment_id || comment?.id || '').trim();
  const user = comment?.user || {};
  const publishedAt = parsePublishedAt(comment?.create_time || comment?.createTime);
  const replyTarget = parseReplyTarget(comment);
  const platformAuthorId = String(user?.uid || user?.user_id || '').trim();
  const contentId = String(note?.contentId || note?.noteId || '').trim();

  return {
    commentId,
    commentEntityId: contentId && commentId ? `douyin_${contentId}_${commentId}` : '',
    platform: 'douyin',
    contentId,
    noteId: note?.noteId || '',
    noteUrl: note?.url || window.location.href,
    searchKeyword: String(note?.searchKeyword || '').trim(),
    searchPageUrl: String(note?.searchPageUrl || '').trim(),
    text: String(comment?.text || comment?.content || '').trim(),
    author: String(user?.nickname || user?.name || '').trim(),
    authorId: platformAuthorId,
    authorEntityId: platformAuthorId ? `douyin_${platformAuthorId}` : '',
    profileUrl: toProfileUrl(user),
    avatarUrl: toAvatarUrl(user),
    location: String(comment?.ip_label || comment?.ipLabel || comment?.ip_location || '').trim(),
    ipLocation: String(comment?.ip_label || comment?.ipLabel || comment?.ip_location || '').trim(),
    likes: parseCount(comment?.digg_count ?? comment?.like_count ?? comment?.diggCount ?? 0),
    parentCommentId,
    rootCommentId: rootCommentId || commentId,
    level,
    replyToCommentId: level > 1 ? (replyTarget.replyToCommentId || parentCommentId) : '',
    replyToUserName: level > 1 ? replyTarget.replyToUserName : '',
    time: formatPublishedAtText(publishedAt),
    publishedAt,
    publishedAtText: formatPublishedAtText(publishedAt),
    sortMode,
    positionIndex,
    collectionRunId: String(collectionRunId || '').trim() || undefined,
    collectedAt: Date.now(),
    createdAt: Date.now(),
    syncStatus: 'local',
    ...createCollectorQualityMeta({
      dataQuality: 'full',
      sourceTier: 'api',
    }),
    ...createCollectorEvidence({
      rawPayload: comment,
      rawDomText: joinRawDomText([
        String(user?.nickname || user?.name || '').trim(),
        String(comment?.text || comment?.content || '').trim(),
        formatPublishedAtText(publishedAt),
        String(comment?.ip_label || comment?.ipLabel || comment?.ip_location || '').trim(),
      ]),
      rawUrl: note?.url || window.location.href,
      rawSource: 'douyin.comment_api',
    }),
  };
}
