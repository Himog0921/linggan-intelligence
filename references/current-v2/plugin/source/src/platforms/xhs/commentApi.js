import { parseCount } from '../../shared/utils.js';
import { createCollectorEvidence, createCollectorQualityMeta, joinRawDomText } from '../../shared/collectorMetadata.js';

const PAGE_BRIDGE_SOURCE = 'lgboom-xhs-api-capture';
const PAGE_FETCH_REQUEST_SOURCE = 'lgboom-xhs-content';
const SNAPSHOT_REQUEST_TYPE = '__lgboom_xhs_comment_api_request__';
const SNAPSHOT_RESPONSE_TYPE = '__lgboom_xhs_comment_api_response__';
const PROFILE_NOTES_REQUEST_TYPE = '__lgboom_xhs_profile_notes_request__';
const PROFILE_NOTES_RESPONSE_TYPE = '__lgboom_xhs_profile_notes_response__';
const SEARCH_NOTES_REQUEST_TYPE = '__lgboom_xhs_search_notes_request__';
const SEARCH_NOTES_RESPONSE_TYPE = '__lgboom_xhs_search_notes_response__';
const PAGE_FETCH_REQUEST_TYPE = '__lgboom_xhs_page_fetch_request__';
const PAGE_FETCH_RESPONSE_TYPE = '__lgboom_xhs_page_fetch_response__';

function normalizeText(value) {
  return String(value || '').trim();
}

function normalizeAvatarUrl(raw = '') {
  const value = normalizeText(raw);
  if (!value) return '';
  if (value.startsWith('//')) return `https:${value}`;
  if (value.startsWith('/')) return `https://www.xiaohongshu.com${value}`;
  return value;
}

function safeClone(value) {
  if (value == null) return null;
  try {
    return JSON.parse(JSON.stringify(value));
  } catch {
    return null;
  }
}

function readPayload(json = {}) {
  if (json?.data && typeof json.data === 'object') return json.data;
  return json || {};
}

function readCommentArray(source = {}) {
  const payload = readPayload(source);
  const candidates = [
    payload?.comments,
    payload?.comment_list,
    payload?.list,
    source?.comments,
    source?.comment_list,
    Array.isArray(payload) ? payload : null,
  ];
  for (const candidate of candidates) {
    if (Array.isArray(candidate)) return candidate;
  }
  return [];
}

function readCursor(source = {}) {
  const payload = readPayload(source);
  const candidates = [
    payload?.cursor,
    payload?.next_cursor,
    payload?.nextCursor,
    source?.cursor,
    source?.next_cursor,
    source?.nextCursor,
  ];
  for (const value of candidates) {
    const text = normalizeText(value);
    if (text) return text;
  }
  return '';
}

function readHasMore(source = {}) {
  const payload = readPayload(source);
  const candidates = [
    payload?.has_more,
    payload?.hasMore,
    payload?.more,
    source?.has_more,
    source?.hasMore,
    source?.more,
  ];
  for (const value of candidates) {
    if (value === true || value === 1 || value === '1') return true;
    if (value === false || value === 0 || value === '0') return false;
  }
  return false;
}

function parsePublishedAt(raw) {
  if (raw == null || raw === '') return 0;
  if (typeof raw === 'string' && /\D/.test(raw) && !/^\d+$/.test(raw)) {
    const parsed = Date.parse(raw);
    return Number.isFinite(parsed) ? parsed : 0;
  }
  const value = Number(raw);
  if (!Number.isFinite(value) || value <= 0) return 0;
  return value < 1000000000000 ? value * 1000 : value;
}

function formatPublishedAtText(raw, publishedAt) {
  const explicit = normalizeText(raw);
  if (explicit && /\D/.test(explicit) && !/^\d+$/.test(explicit)) return explicit;
  if (publishedAt <= 0) return explicit;
  try {
    return new Date(publishedAt).toISOString();
  } catch {
    return explicit;
  }
}

function resolveUser(comment = {}) {
  const candidates = [
    comment?.user_info,
    comment?.userInfo,
    comment?.user,
    comment?.author,
  ];
  for (const candidate of candidates) {
    if (candidate && typeof candidate === 'object') return candidate;
  }
  return {};
}

function resolveUserName(user = {}) {
  return normalizeText(user?.nickname || user?.nick_name || user?.name || user?.user_name);
}

function resolveUserId(user = {}) {
  return normalizeText(user?.user_id || user?.userId || user?.id || user?.uid);
}

function resolveUserAvatar(user = {}) {
  const candidates = [
    user?.image,
    user?.avatar,
    user?.avatar_url,
    user?.images?.[0],
    user?.imageb,
  ];
  for (const candidate of candidates) {
    const value = normalizeAvatarUrl(candidate);
    if (value) return value;
  }
  return '';
}

function resolveProfileUrl(user = {}) {
  const direct = normalizeText(user?.profile_url || user?.profileUrl || user?.url);
  if (direct) return direct;
  const userId = resolveUserId(user);
  return userId ? `https://www.xiaohongshu.com/user/profile/${encodeURIComponent(userId)}` : '';
}

function resolveReplyTarget(comment = {}) {
  const candidates = [
    comment?.target_comment,
    comment?.targetComment,
    comment?.reply_comment,
    comment?.replyComment,
    comment?.reply_to_comment,
    comment?.replyToComment,
    comment?.parent_comment,
    comment?.parentComment,
  ];
  for (const candidate of candidates) {
    if (!candidate || typeof candidate !== 'object') continue;
    const targetUser = resolveUser(candidate);
    const replyToCommentId = normalizeText(candidate?.id || candidate?.comment_id || candidate?.commentId);
    const replyToUserName = resolveUserName(targetUser);
    if (replyToCommentId || replyToUserName) {
      return { replyToCommentId, replyToUserName };
    }
  }

  const replyToUser = comment?.reply_to_user_info || comment?.replyToUserInfo || comment?.reply_user;
  if (replyToUser && typeof replyToUser === 'object') {
    return {
      replyToCommentId: '',
      replyToUserName: resolveUserName(replyToUser),
    };
  }

  return { replyToCommentId: '', replyToUserName: '' };
}

function readInlineReplies(comment = {}) {
  const candidates = [
    comment?.sub_comments,
    comment?.subComments,
    comment?.reply_comments,
    comment?.replyComments,
    comment?.replies,
  ];
  for (const candidate of candidates) {
    if (Array.isArray(candidate)) return candidate;
  }
  return [];
}

function toQueryString(params = {}) {
  return Object.entries(params)
    .filter(([, value]) => value != null)
    .map(([key, value]) => `${encodeURIComponent(key)}=${encodeURIComponent(String(value))}`)
    .join('&');
}

function normalizeBoolean(value) {
  if (value === true || value === 1 || value === '1') return true;
  if (value === false || value === 0 || value === '0') return false;
  return null;
}

function readReplyTotal(comment = {}) {
  const candidates = [
    comment?.sub_comment_count,
    comment?.subCommentCount,
    comment?.sub_comment_num,
    comment?.subCommentNum,
    comment?.reply_count,
    comment?.replyCount,
  ];
  for (const candidate of candidates) {
    const value = Number(candidate);
    if (Number.isFinite(value) && value >= 0) return value;
  }
  return 0;
}

function hasMoreReplies(comment = {}) {
  const candidates = [
    comment?.sub_comment_has_more,
    comment?.subCommentHasMore,
    comment?.has_more_sub_comment,
    comment?.hasMoreSubComment,
    comment?.has_more_reply,
    comment?.hasMoreReply,
  ];
  for (const candidate of candidates) {
    const value = normalizeBoolean(candidate);
    if (value !== null) return value;
  }
  return false;
}

function getSnapshotPageKey(page = {}) {
  return [
    normalizeText(page?.endpoint || 'page'),
    normalizeText(page?.noteId),
    normalizeText(page?.rootCommentId),
    normalizeText(page?.cursor) || '__first__',
  ].join('|');
}

function mergeSnapshotPages(existingPages = [], nextPage) {
  const pages = Array.isArray(existingPages) ? existingPages.map((item) => safeClone(item)).filter(Boolean) : [];
  if (!nextPage) return pages;
  const nextKey = getSnapshotPageKey(nextPage);
  const filtered = pages.filter((page) => getSnapshotPageKey(page) !== nextKey);
  filtered.push(safeClone(nextPage));
  filtered.sort((a, b) => Number(a?.capturedAt || 0) - Number(b?.capturedAt || 0));
  return filtered;
}

function readXsecTokenFromUrl(url = '') {
  try {
    return normalizeText(new URL(url, 'https://www.xiaohongshu.com').searchParams.get('xsec_token'));
  } catch {
    return '';
  }
}

function readCurrentXsecToken() {
  try {
    return readXsecTokenFromUrl(window.location?.href || '');
  } catch {
    return '';
  }
}

function resolveSnapshotXsecToken(snapshot = {}) {
  const pageLists = [
    ...(Array.isArray(snapshot?.pages) ? snapshot.pages : []),
    ...(Array.isArray(snapshot?.subPages) ? snapshot.subPages : []),
  ];
  for (let i = pageLists.length - 1; i >= 0; i--) {
    const token = readXsecTokenFromUrl(pageLists[i]?.sourceUrl);
    if (token) return token;
  }
  return readCurrentXsecToken();
}

function withOptionalXsecToken(query = {}, xsecToken = '') {
  const token = normalizeText(xsecToken);
  return token ? { ...query, xsec_token: token } : query;
}

function buildXhsCommentPageRequestUrls(noteId = '', cursor = '', { xsecToken = '' } = {}) {
  const query = {
    note_id: normalizeText(noteId),
    cursor: normalizeText(cursor),
    top_comment_id: '',
    image_formats: 'jpg,webp,avif',
  };
  return [
    `/api/sns/web/v2/comment/page?${toQueryString(withOptionalXsecToken(query, xsecToken))}`,
    `/api/sns/web/v2/comment/page?${toQueryString({
      note_id: normalizeText(noteId),
      cursor: normalizeText(cursor),
    })}`,
  ];
}

function buildXhsSubCommentPageRequestUrls(noteId = '', rootCommentId = '', cursor = '', { xsecToken = '' } = {}) {
  const query = {
    note_id: normalizeText(noteId),
    root_comment_id: normalizeText(rootCommentId),
    num: 10,
    cursor: normalizeText(cursor),
    image_formats: 'jpg,webp,avif',
    top_comment_id: '',
  };
  return [
    `/api/sns/web/v2/comment/sub/page?${toQueryString(withOptionalXsecToken(query, xsecToken))}`,
    `/api/sns/web/v2/comment/sub/page?${toQueryString({
      note_id: normalizeText(noteId),
      root_comment_id: normalizeText(rootCommentId),
      cursor: normalizeText(cursor),
    })}`,
  ];
}

function shouldHydrateSubReplies(comment = {}, snapshot = {}) {
  const rootCommentId = normalizeText(comment?.id || comment?.comment_id || comment?.commentId);
  if (!rootCommentId) return false;
  const total = readReplyTotal(comment);
  const inlineCount = readInlineReplies(comment).length;
  const pagedCount = (Array.isArray(snapshot?.subPages) ? snapshot.subPages : [])
    .filter((page) => normalizeText(page?.rootCommentId) === rootCommentId)
    .reduce((sum, page) => sum + (Array.isArray(page?.comments) ? page.comments.length : 0), 0);
  if (hasMoreReplies(comment)) return true;
  return total > inlineCount + pagedCount;
}

export function parseXhsCommentPagePayload(json = {}, { sourceUrl = '' } = {}) {
  let parsedUrl = null;
  try {
    parsedUrl = new URL(sourceUrl, 'https://www.xiaohongshu.com');
  } catch {
    parsedUrl = null;
  }

  const payload = readPayload(json);
  const endpoint = String(sourceUrl || '').includes('/api/sns/web/v2/comment/sub/page') ? 'sub' : 'page';
  const noteId = normalizeText(
    parsedUrl?.searchParams.get('note_id')
    || payload?.note_id
    || payload?.noteId
    || json?.note_id
    || json?.noteId
  );
  const rootCommentId = normalizeText(
    parsedUrl?.searchParams.get('root_comment_id')
    || payload?.root_comment_id
    || payload?.rootCommentId
    || json?.root_comment_id
    || json?.rootCommentId
  );

  return {
    endpoint,
    noteId,
    rootCommentId,
    cursor: readCursor(json),
    hasMore: readHasMore(json),
    comments: readCommentArray(json).map((item) => safeClone(item)).filter(Boolean),
    sourceUrl: normalizeText(sourceUrl),
    capturedAt: Date.now(),
  };
}

export function mapXhsCommentRecord(comment = {}, note = {}, {
  parentCommentId = '',
  rootCommentId = '',
  level = 1,
  sortMode = 'unknown',
  collectionRunId = '',
  qualityMeta = {},
} = {}) {
  const commentId = normalizeText(comment?.id || comment?.comment_id || comment?.commentId);
  const user = resolveUser(comment);
  const authorId = resolveUserId(user);
  const publishedAt = parsePublishedAt(comment?.create_time || comment?.createTime || comment?.time);
  const replyTarget = resolveReplyTarget(comment);
  const effectiveRootCommentId = normalizeText(rootCommentId || parentCommentId || commentId);
  const noteId = normalizeText(note?.noteId);
  const contentId = normalizeText(note?.contentId || (noteId ? `xhs_${noteId}` : ''));
  const noteUrl = normalizeText(note?.url || note?.noteUrl);

  return {
    platform: 'xhs',
    noteId,
    contentId,
    noteUrl,
    commentId,
    platformCommentId: commentId,
    commentEntityId: noteId && commentId ? `xhs_${noteId}_${commentId}` : '',
    text: normalizeText(comment?.content || comment?.text || comment?.desc),
    author: resolveUserName(user),
    authorId,
    authorEntityId: authorId ? `xhs_${authorId}` : '',
    profileUrl: resolveProfileUrl(user),
    avatarUrl: resolveUserAvatar(user),
    location: normalizeText(comment?.ip_location || comment?.ipLocation || comment?.location),
    ipLocation: normalizeText(comment?.ip_location || comment?.ipLocation || comment?.location),
    likes: parseCount(comment?.like_count ?? comment?.likes ?? comment?.likeCount ?? 0),
    parentCommentId: normalizeText(parentCommentId),
    rootCommentId: effectiveRootCommentId,
    level,
    replyToCommentId: level > 1 ? normalizeText(replyTarget.replyToCommentId || parentCommentId) : '',
    replyToUserName: level > 1 ? normalizeText(replyTarget.replyToUserName) : '',
    time: formatPublishedAtText(comment?.time || comment?.create_time || comment?.createTime, publishedAt),
    publishedAt,
    publishedAtText: formatPublishedAtText(comment?.time || comment?.create_time || comment?.createTime, publishedAt),
    sortMode,
    collectionRunId: normalizeText(collectionRunId) || undefined,
    collectedAt: Date.now(),
    createdAt: Date.now(),
    syncStatus: 'pending',
    ...createCollectorQualityMeta({
      dataQuality: 'full',
      sourceTier: 'api',
      ...qualityMeta,
    }),
    ...createCollectorEvidence({
      rawPayload: comment,
      rawDomText: joinRawDomText([
        resolveUserName(user),
        normalizeText(comment?.content || comment?.text || comment?.desc),
        formatPublishedAtText(comment?.time || comment?.create_time || comment?.createTime, publishedAt),
        normalizeText(comment?.ip_location || comment?.ipLocation || comment?.location),
      ]),
      rawUrl: noteUrl,
      rawSource: 'xhs.comments.api',
    }),
  };
}

export function buildXhsCommentsFromSnapshot(snapshot = {}, note = {}, {
  maxSubComments = 0,
  sortMode = 'unknown',
  collectionRunId = '',
  qualityMeta = {},
} = {}) {
  const orderedKeys = [];
  const records = new Map();
  const replyCountByRoot = new Map();

  const pushRecord = (record) => {
    const key = `${normalizeText(record?.commentId)}|${normalizeText(record?.parentCommentId)}|${Number(record?.level || 1)}`;
    if (!record?.commentId) return;
    if (!records.has(key)) orderedKeys.push(key);
    records.set(key, record);
  };

  const canIncludeReply = (rootId) => {
    if (!(maxSubComments > 0)) return true;
    const current = Number(replyCountByRoot.get(rootId) || 0);
    if (current >= maxSubComments) return false;
    replyCountByRoot.set(rootId, current + 1);
    return true;
  };

  const pageList = Array.isArray(snapshot?.pages) ? snapshot.pages : [];
  pageList
    .slice()
    .sort((a, b) => Number(a?.capturedAt || 0) - Number(b?.capturedAt || 0))
    .forEach((page) => {
      const comments = Array.isArray(page?.comments) ? page.comments : [];
      comments.forEach((comment) => {
        const main = mapXhsCommentRecord(comment, note, {
          level: 1,
          parentCommentId: '',
          rootCommentId: '',
          sortMode,
          collectionRunId,
          qualityMeta,
        });
        if (!main.commentId) return;
        const rootId = main.commentId;
        pushRecord({ ...main, rootCommentId: rootId });

        const inlineReplies = readInlineReplies(comment);
        inlineReplies.forEach((reply) => {
          if (!canIncludeReply(rootId)) return;
          pushRecord(mapXhsCommentRecord(reply, note, {
            level: 2,
            parentCommentId: rootId,
            rootCommentId: rootId,
            sortMode,
            collectionRunId,
            qualityMeta,
          }));
        });
      });
    });

  const subPageList = Array.isArray(snapshot?.subPages) ? snapshot.subPages : [];
  subPageList
    .slice()
    .sort((a, b) => Number(a?.capturedAt || 0) - Number(b?.capturedAt || 0))
    .forEach((page) => {
      const rootId = normalizeText(page?.rootCommentId);
      if (!rootId) return;
      const comments = Array.isArray(page?.comments) ? page.comments : [];
      comments.forEach((comment) => {
        if (!canIncludeReply(rootId)) return;
        pushRecord(mapXhsCommentRecord(comment, note, {
          level: 2,
          parentCommentId: rootId,
          rootCommentId: rootId,
          sortMode,
          collectionRunId,
          qualityMeta,
        }));
      });
    });

  return orderedKeys.map((key) => records.get(key)).filter(Boolean);
}

export async function hydrateXhsCommentSnapshot(snapshot = {}, {
  noteId = '',
  fetchJson = fetchXhsJsonViaBridge,
  shouldStop = () => false,
  waitIfPaused = async () => {},
  maxMainPages = 6,
  maxSubPagesPerRoot = 6,
} = {}) {
  const normalizedNoteId = normalizeText(noteId || snapshot?.noteId);
  if (!normalizedNoteId) return {
    noteId: '',
    pages: [],
    subPages: [],
  };

  let hydrated = {
    noteId: normalizedNoteId,
    pages: Array.isArray(snapshot?.pages) ? snapshot.pages.map((page) => safeClone(page)).filter(Boolean) : [],
    subPages: Array.isArray(snapshot?.subPages) ? snapshot.subPages.map((page) => safeClone(page)).filter(Boolean) : [],
  };
  const xsecToken = resolveSnapshotXsecToken(hydrated);

  let mainFetchCount = 0;
  while (mainFetchCount < maxMainPages && !shouldStop()) {
    const lastPage = hydrated.pages[hydrated.pages.length - 1];
    if (!lastPage?.hasMore) break;
    await waitIfPaused();
    if (shouldStop()) break;
    const requestUrls = buildXhsCommentPageRequestUrls(normalizedNoteId, lastPage.cursor, { xsecToken });
    const json = await fetchJson(requestUrls);
    const nextPage = parseXhsCommentPagePayload(json, { sourceUrl: requestUrls[0] });
    if (!nextPage.noteId) nextPage.noteId = normalizedNoteId;
    hydrated.pages = mergeSnapshotPages(hydrated.pages, nextPage);
    mainFetchCount += 1;
    if (!nextPage.hasMore) break;
    if ((Array.isArray(nextPage.comments) ? nextPage.comments.length : 0) === 0) break;
  }

  const rootComments = hydrated.pages.flatMap((page) => Array.isArray(page?.comments) ? page.comments : []);
  for (const rootComment of rootComments) {
    const rootCommentId = normalizeText(rootComment?.id || rootComment?.comment_id || rootComment?.commentId);
    if (!rootCommentId) continue;
    if (!shouldHydrateSubReplies(rootComment, hydrated)) continue;

    let subFetchCount = 0;
    let continueFetch = true;
    while (continueFetch && subFetchCount < maxSubPagesPerRoot && !shouldStop()) {
      await waitIfPaused();
      if (shouldStop()) break;

      const existingRootPages = hydrated.subPages.filter((page) => normalizeText(page?.rootCommentId) === rootCommentId);
      const lastSubPage = existingRootPages[existingRootPages.length - 1] || null;
      const requestCursor = lastSubPage?.hasMore ? normalizeText(lastSubPage.cursor) : (existingRootPages.length > 0 ? '' : '');
      if (existingRootPages.length > 0 && lastSubPage && !lastSubPage.hasMore && !hasMoreReplies(rootComment)) {
        break;
      }

      const requestUrls = buildXhsSubCommentPageRequestUrls(normalizedNoteId, rootCommentId, requestCursor, { xsecToken });
      const json = await fetchJson(requestUrls);
      const nextPage = parseXhsCommentPagePayload(json, { sourceUrl: requestUrls[0] });
      nextPage.noteId = nextPage.noteId || normalizedNoteId;
      nextPage.rootCommentId = nextPage.rootCommentId || rootCommentId;
      hydrated.subPages = mergeSnapshotPages(hydrated.subPages, nextPage);
      subFetchCount += 1;

      const declaredTotal = readReplyTotal(rootComment);
      const inlineCount = readInlineReplies(rootComment).length;
      const pagedCount = hydrated.subPages
        .filter((page) => normalizeText(page?.rootCommentId) === rootCommentId)
        .reduce((sum, page) => sum + (Array.isArray(page?.comments) ? page.comments.length : 0), 0);

      continueFetch = Boolean(
        nextPage.hasMore
        || (declaredTotal > 0 && inlineCount + pagedCount < declaredTotal),
      );
      if ((Array.isArray(nextPage.comments) ? nextPage.comments.length : 0) === 0) break;
    }
  }

  return hydrated;
}

function postBridgeRequest(type, payload, responseType, timeoutMs = 1200) {
  const requestId = `xhs_bridge_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;
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
        if (data.type !== responseType) return;
        const bridgePayload = data.payload || {};
        if (bridgePayload.requestId !== requestId) return;
        if (bridgePayload.ok) {
          finishResolve(bridgePayload);
          return;
        }
        finishReject(new Error(bridgePayload.error || 'bridge_request_failed'));
      } catch (error) {
        finishReject(error);
      }
    };
    const timer = window.setTimeout(() => {
      finishReject(new Error('bridge_request_timeout'));
    }, timeoutMs);

    window.addEventListener('message', onMessage);

    try {
      window.postMessage({
        source: PAGE_FETCH_REQUEST_SOURCE,
        type,
        payload: {
          requestId,
          ...payload,
        },
      }, '*');
    } catch (error) {
      finishReject(error);
    }
  });
}

export async function requestXhsCommentSnapshot(noteId = '') {
  const normalizedNoteId = normalizeText(noteId);
  if (!normalizedNoteId) return null;
  const payload = await postBridgeRequest(
    SNAPSHOT_REQUEST_TYPE,
    { noteId: normalizedNoteId },
    SNAPSHOT_RESPONSE_TYPE,
  );
  return {
    noteId: normalizedNoteId,
    pages: Array.isArray(payload?.pages) ? payload.pages : [],
    subPages: Array.isArray(payload?.subPages) ? payload.subPages : [],
  };
}

export async function requestXhsProfileNotesSnapshot(userId = '') {
  const normalizedUserId = normalizeText(userId);
  const payload = await postBridgeRequest(
    PROFILE_NOTES_REQUEST_TYPE,
    { userId: normalizedUserId },
    PROFILE_NOTES_RESPONSE_TYPE,
  );
  return {
    userId: normalizedUserId,
    pages: Array.isArray(payload?.pages) ? payload.pages : [],
  };
}

export async function requestXhsSearchNotesSnapshot(keyword = '') {
  const normalizedKeyword = normalizeText(keyword);
  const payload = await postBridgeRequest(
    SEARCH_NOTES_REQUEST_TYPE,
    { keyword: normalizedKeyword },
    SEARCH_NOTES_RESPONSE_TYPE,
  );
  return {
    keyword: normalizedKeyword,
    pages: Array.isArray(payload?.pages) ? payload.pages : [],
  };
}

export async function fetchXhsJsonViaBridge(urls = []) {
  const payload = await postBridgeRequest(
    PAGE_FETCH_REQUEST_TYPE,
    { urls: Array.isArray(urls) ? urls.map((item) => normalizeText(item)).filter(Boolean) : [] },
    PAGE_FETCH_RESPONSE_TYPE,
    4000,
  );
  return payload?.json || null;
}

export function ensureXhsCommentApiBridge() {
  if (typeof document === 'undefined' || typeof chrome === 'undefined' || typeof chrome.runtime?.getURL !== 'function') return;
  if (document.getElementById('__lgboom_xhs_api_capture')) return;
  const script = document.createElement('script');
  script.id = '__lgboom_xhs_api_capture';
  script.src = chrome.runtime.getURL('injected/xhsApiCapture.js');
  script.onload = () => script.remove();
  (document.head || document.documentElement).appendChild(script);
}
