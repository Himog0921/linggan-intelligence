import { collectionRunStore } from '../../db/collectionRunStore.js';
import { parseCount } from '../../shared/utils.js';
import { detectDouyinPageType, detectDouyinSearchBatchContext, getDouyinSearchKeyword, getDouyinSearchTabType, DY_PAGE_TYPE } from './pageDetector.js';
import { mergeCapturedDouyinSearchPages, normalizeDouyinSearchChannel } from './searchCapture.js';
import { fetchDouyinWithTimeout } from './fetchWithTimeout.js';
import {
  createDouyinSecurityChallengeError,
  detectDouyinSecurityChallenge,
  isDouyinSecurityChallengeError,
  maybeCreateDouyinSecurityChallengeError,
} from './securityChallenge.js';

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function normalizeDouyinBatchDelayRange(step) {
  if (typeof step === 'number') {
    const normalized = Math.max(0, Math.round(Number(step) || 0));
    return { min: normalized, max: normalized };
  }

  const min = Math.max(0, Math.round(Number(step?.min ?? step?.max ?? 0) || 0));
  const maxCandidate = Math.max(0, Math.round(Number(step?.max ?? step?.min ?? min) || min));
  return {
    min: Math.min(min, maxCandidate),
    max: Math.max(min, maxCandidate),
  };
}

function scaleDouyinBatchDelayRange(range, multiplier = 1) {
  const normalized = normalizeDouyinBatchDelayRange(range);
  return {
    min: Math.max(0, Math.round(normalized.min * multiplier)),
    max: Math.max(0, Math.round(normalized.max * multiplier)),
  };
}

function pickDouyinBatchDelay(range, { random = Math.random } = {}) {
  const normalized = normalizeDouyinBatchDelayRange(range);
  const span = Math.max(0, normalized.max - normalized.min);
  const ratio = Math.min(1, Math.max(0, Number(random?.() ?? Math.random()) || 0));
  return normalized.min + Math.round(span * ratio);
}

export function createDouyinBatchPacer({
  baseRange = { min: 180, max: 260 },
  windowSize = 6,
  random = Math.random,
} = {}) {
  const outcomes = [];

  function pushOutcome(ok) {
    outcomes.push(ok ? 0 : 1);
    while (outcomes.length > Math.max(1, Number(windowSize || 0) || 6)) {
      outcomes.shift();
    }
  }

  function getErrorRate() {
    if (!outcomes.length) return 0;
    const failures = outcomes.reduce((sum, value) => sum + Number(value || 0), 0);
    return failures / outcomes.length;
  }

  function getBackoffLevel() {
    const errorRate = getErrorRate();
    if (outcomes.length >= 4 && errorRate >= 0.75) return 2;
    if (outcomes.length >= 3 && errorRate >= 0.25) return 1;
    return 0;
  }

  function getDelayRange(customRange = baseRange) {
    const multiplier = [1, 1.5, 2.2][getBackoffLevel()] || 1;
    const scaled = scaleDouyinBatchDelayRange(customRange, multiplier);
    return {
      min: scaled.min,
      max: scaled.max,
      errorRate: getErrorRate(),
      backoffLevel: getBackoffLevel(),
    };
  }

  async function wait({
    waitIfPaused = async () => {},
    shouldStop = () => false,
    baseRange: customRange = baseRange,
  } = {}) {
    await waitIfPaused();
    if (shouldStop()) return 0;
    const range = getDelayRange(customRange);
    return waitDouyinBatchStep({
      min: range.min,
      max: range.max,
    }, {
      random,
    });
  }

  return {
    recordSuccess() {
      pushOutcome(true);
    },
    recordFailure() {
      pushOutcome(false);
    },
    getDelayRange,
    wait,
  };
}

function parseChunkedJsonPayload(rawText = '') {
  const text = String(rawText || '').trim();
  if (!text) {
    throw new Error('接口返回为空');
  }

  try {
    return JSON.parse(text);
  } catch {
    // try chunked transport format next
  }

  let cursor = 0;
  let combined = '';
  while (cursor < text.length) {
    while (text.slice(cursor, cursor + 2) === '\r\n') cursor += 2;
    const lineEnd = text.indexOf('\r\n', cursor);
    if (lineEnd === -1) break;
    const sizeText = text.slice(cursor, lineEnd).trim();
    if (!/^[0-9a-fA-F]+$/.test(sizeText)) break;
    const chunkSize = Number.parseInt(sizeText, 16);
    if (!Number.isFinite(chunkSize)) break;
    cursor = lineEnd + 2;
    if (chunkSize === 0) break;
    combined += text.slice(cursor, cursor + chunkSize);
    cursor += chunkSize;
    if (text.slice(cursor, cursor + 2) === '\r\n') cursor += 2;
  }

  const candidate = combined.trim() || text;
  try {
    return JSON.parse(candidate);
  } catch {
    const firstBrace = candidate.search(/[\[{]/);
    const lastObjectBrace = candidate.lastIndexOf('}');
    const lastArrayBrace = candidate.lastIndexOf(']');
    const lastBrace = Math.max(lastObjectBrace, lastArrayBrace);
    if (firstBrace >= 0 && lastBrace > firstBrace) {
      return JSON.parse(candidate.slice(firstBrace, lastBrace + 1));
    }
    throw new Error('搜索接口返回不是有效 JSON');
  }
}

function getProfileSecUserId() {
  const match = window.location.pathname.match(/^\/user\/([^/?#]+)/);
  return String(match?.[1] || '').trim();
}

function getSearchKeywordFromUrl() {
  try {
    const parsed = new URL(window.location.href);
    const fromPath = parsed.pathname.match(/^\/search\/([^/?#]+)/)?.[1] || '';
    return decodeURIComponent(fromPath || '').trim();
  } catch {
    return '';
  }
}

function isElementVisible(el) {
  if (!el) return false;
  const rect = el.getBoundingClientRect();
  if (rect.width <= 0 || rect.height <= 0) return false;
  const style = window.getComputedStyle(el);
  return style.display !== 'none' && style.visibility !== 'hidden';
}

function extractSearchResultIdFromHref(href = '') {
  const match = String(href || '').match(/\/(?:video|note)\/([A-Za-z0-9_-]+)/);
  return String(match?.[1] || '').trim();
}

function normalizeResultLines(text = '') {
  return String(text || '')
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean);
}

function isDurationLine(line = '') {
  return /^(合集|\d{1,2}:\d{2}(?::\d{2})?)$/.test(String(line || '').trim());
}

function isAuthorLine(line = '') {
  return /^@/.test(String(line || '').trim());
}

function parseSearchResultSummary(text = '') {
  const lines = normalizeResultLines(text);
  let cursor = 0;
  while (cursor < lines.length && isDurationLine(lines[cursor])) cursor += 1;
  const likesText = lines[cursor] || '0';
  const likes = parseCount(likesText);
  cursor += 1;

  const titleParts = [];
  while (cursor < lines.length && !isAuthorLine(lines[cursor])) {
    titleParts.push(lines[cursor]);
    cursor += 1;
  }
  const authorLine = isAuthorLine(lines[cursor]) ? lines[cursor] : '';
  const timeLine = lines[cursor + 1] || '';

  return {
    likesText,
    likes,
    titleHint: titleParts.join(' ').trim(),
    authorHint: authorLine.replace(/^@/, '').trim(),
    timeHint: timeLine.trim(),
  };
}

export function sortDouyinVisualSearchEntries(entries = [], rowTolerance = 24) {
  const normalized = (Array.isArray(entries) ? entries : [])
    .filter(Boolean)
    .map((entry, index) => ({
      ...entry,
      top: Number.isFinite(Number(entry?.top)) ? Number(entry.top) : 0,
      left: Number.isFinite(Number(entry?.left)) ? Number(entry.left) : 0,
      domIndex: Number.isFinite(Number(entry?.domIndex)) ? Number(entry.domIndex) : index,
    }));

  return normalized.sort((a, b) => {
    const topDiff = a.top - b.top;
    if (Math.abs(topDiff) > rowTolerance) return topDiff;
    const leftDiff = a.left - b.left;
    if (Math.abs(leftDiff) > 1) return leftDiff;
    return a.domIndex - b.domIndex;
  });
}

export function mergeDouyinSearchTargetsByVisibleOrder(domTargets = [], apiTargets = []) {
  const result = [];
  const seen = new Set();
  const apiById = new Map();

  for (const target of Array.isArray(apiTargets) ? apiTargets : []) {
    const awemeId = String(target?.awemeId || target?.key || '').trim();
    if (!awemeId || apiById.has(awemeId)) continue;
    apiById.set(awemeId, target);
  }

  for (const target of Array.isArray(domTargets) ? domTargets : []) {
    const awemeId = String(target?.awemeId || target?.key || '').trim();
    if (!awemeId || seen.has(awemeId)) continue;
    seen.add(awemeId);
    const apiTarget = apiById.get(awemeId);
    result.push({
      ...(apiTarget || {}),
      ...(target || {}),
      aweme: target?.aweme || apiTarget?.aweme || null,
      likes: Number(apiTarget?.likes ?? target?.likes ?? 0),
      comments: Number(apiTarget?.comments ?? target?.comments ?? 0),
      createTime: Number(apiTarget?.createTime ?? target?.createTime ?? 0),
      sourceUrl: String(target?.sourceUrl || apiTarget?.sourceUrl || '').trim(),
      searchKeyword: String(target?.searchKeyword || apiTarget?.searchKeyword || '').trim(),
    });
  }

  for (const target of Array.isArray(apiTargets) ? apiTargets : []) {
    const awemeId = String(target?.awemeId || target?.key || '').trim();
    if (!awemeId || seen.has(awemeId)) continue;
    seen.add(awemeId);
    result.push(target);
  }

  return result;
}

async function discoverDouyinSearchTargetsFromDom({
  maxCount = 20,
  topByLikes = false,
  shouldStop = () => false,
  waitIfPaused = async () => {},
} = {}) {
  const keyword = getCurrentDouyinSearchKeyword();
  if (!keyword) {
    throw new Error('未能识别当前搜索词');
  }

  const pageLimit = topByLikes ? 10 : Math.max(2, Math.ceil(maxCount / 8));
  const seen = new Map();
  let stagnantRounds = 0;
  let previousCount = 0;

  for (let pageNo = 0; pageNo < pageLimit; pageNo += 1) {
    await waitIfPaused();
    if (shouldStop()) break;

    const cards = sortDouyinVisualSearchEntries(
      Array.from(document.querySelectorAll('a[href*="/video/"], a[href*="/note/"]'))
        .map((link, domIndex) => {
          const card = link.closest('li') || link.closest('[data-e2e]') || link;
          const rect = (card?.getBoundingClientRect?.() || link?.getBoundingClientRect?.() || {});
          return {
            link,
            card,
            domIndex,
            top: Number(rect?.top || 0),
            left: Number(rect?.left || 0),
          };
        }),
    );
    for (const entry of cards) {
      const link = entry.link;
      const card = entry.card || link;
      if (!isElementVisible(card) || !isElementVisible(link)) continue;
      const href = String(link.href || '').trim();
      const awemeId = extractSearchResultIdFromHref(href);
      if (!awemeId || seen.has(awemeId)) continue;
      const summary = parseSearchResultSummary(card.innerText || link.innerText || link.textContent || '');
      seen.set(awemeId, {
        key: awemeId,
        awemeId,
        href,
        titleHint: summary.titleHint || awemeId,
        authorHint: summary.authorHint || '',
        timeHint: summary.timeHint || '',
        likes: Number(summary.likes || 0),
        comments: 0,
        createTime: 0,
        orderIndex: seen.size,
        searchKeyword: keyword,
        sourceUrl: 'dom.search_result',
      });
    }

    if (!topByLikes && seen.size >= maxCount) break;
    if (seen.size == previousCount) stagnantRounds += 1;
    else stagnantRounds = 0;
    previousCount = seen.size;
    if (stagnantRounds >= 2) break;

    window.scrollBy({ top: Math.round(window.innerHeight * 0.9), behavior: 'auto' });
    await waitDouyinBatchStep(topByLikes ? { min: 320, max: 480 } : { min: 260, max: 380 });
  }

  const targets = Array.from(seen.values());
  if (topByLikes) {
    return targets
      .sort((a, b) => {
        const likesDiff = Number(b.likes || 0) - Number(a.likes || 0);
        if (likesDiff !== 0) return likesDiff;
        return Number(a.orderIndex || 0) - Number(b.orderIndex || 0);
      })
      .slice(0, maxCount)
      .map((target, index) => ({ ...target, rank: index + 1 }));
  }

  return targets.slice(0, maxCount);
}

export function getCurrentDouyinSearchKeyword() {
  const keyword = getDouyinSearchKeyword(window);
  if (keyword) return keyword;
  return getSearchKeywordFromUrl();
}

function getNextCursor(payload = {}) {
  const candidates = [
    payload.max_cursor,
    payload.cursor,
    payload.next_cursor,
    payload.nextCursor,
  ];
  for (const raw of candidates) {
    const value = Number(raw);
    if (Number.isFinite(value) && value >= 0) return value;
  }
  return 0;
}

function hasMore(payload = {}) {
  const candidates = [
    payload.has_more,
    payload.hasMore,
    payload.has_more_aweme,
  ];
  for (const value of candidates) {
    if (value === true || value === 1 || value === '1') return true;
    if (value === false || value === 0 || value === '0') return false;
  }
  return false;
}

export async function createDouyinBatchRun({
  taskType,
  pageType,
  triggerSource,
  externalTaskMeta = {},
  config = {},
  meta = {},
} = {}) {
  return collectionRunStore.createRun({
    externalTaskId: String(externalTaskMeta.externalTaskId || '').trim(),
    externalTaskType: String(externalTaskMeta.externalTaskType || '').trim(),
    executorInstanceId: String(externalTaskMeta.executorInstanceId || '').trim(),
    protocolVersion: String(externalTaskMeta.protocolVersion || '').trim(),
    platform: 'douyin',
    taskType,
    pageType,
    triggerSource,
    resultUploadStatus: String(externalTaskMeta.externalTaskId || '').trim() ? 'pending_upload' : 'local_only',
    lastHeartbeatAt: Date.now(),
    config,
    meta: {
      pageUrl: window.location.href,
      secUserId: getProfileSecUserId(),
      searchKeyword: getCurrentDouyinSearchKeyword(),
      ...meta,
    },
  });
}

export async function fetchDouyinProfileVideoPage(secUserId, { cursor = 0, count = 18 } = {}) {
  const encoded = encodeURIComponent(secUserId);
  const urls = [
    `/aweme/v1/web/aweme/post/?device_platform=webapp&aid=6383&channel=channel_pc_web&sec_user_id=${encoded}&max_cursor=${cursor}&count=${count}`,
    `/aweme/v1/web/aweme/post/?device_platform=webapp&aid=6383&sec_user_id=${encoded}&max_cursor=${cursor}&count=${count}`,
    `/aweme/v1/web/aweme/post/?sec_user_id=${encoded}&max_cursor=${cursor}&count=${count}`,
  ];

  let lastError = null;
  for (const url of urls) {
    try {
      const response = await fetchDouyinWithTimeout(url, { credentials: 'include' });
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
      return {
        list: Array.isArray(json?.aweme_list) ? json.aweme_list : [],
        cursor: getNextCursor(json),
        hasMore: hasMore(json),
        sourceUrl: url,
      };
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

  throw lastError || new Error('获取博主页作品列表失败');
}

export async function fetchDouyinSearchVideoPage(keyword, { offset = 0, count = 10, searchChannel = '' } = {}) {
  const encoded = encodeURIComponent(String(keyword || '').trim());
  if (!encoded) {
    throw new Error('缺少搜索词');
  }

  // 根据当前搜索 tab 类型选择频道参数
  // 视频 tab → aweme_video（只返回视频结果，与页面一致）
  // 综合/其他 → aweme_general
  const tabType = getDouyinSearchTabType();
  const channel = searchChannel || (tabType === 'video' ? 'aweme_video' : 'aweme_general');

  const queryBase = [
    'device_platform=webapp',
    'aid=6383',
    'channel=channel_pc_web',
    `keyword=${encoded}`,
    `offset=${offset}`,
    `count=${count}`,
    `search_channel=${channel}`,
    'search_source=normal_search',
    'is_filter_search=0',
    'need_filter_settings=1',
    'query_correct_type=1',
    'disable_rs=0',
    'from_group_id=',
  ].join('&');

  const urls = [
    `/aweme/v1/web/general/search/stream/?${queryBase}`,
    `/aweme/v1/web/general/search/stream/?device_platform=webapp&aid=6383&keyword=${encoded}&offset=${offset}&count=${count}&search_channel=${channel}&search_source=normal_search`,
    `/aweme/v1/web/general/search/stream/?aid=6383&keyword=${encoded}&offset=${offset}&count=${count}`,
  ];

  let lastError = null;
  for (const url of urls) {
    try {
      const response = await fetchDouyinWithTimeout(url, { credentials: 'include' });
      if (!response.ok) {
        lastError = new Error(`HTTP ${response.status}`);
        continue;
      }
      const rawText = await response.text();
      const json = parseChunkedJsonPayload(rawText);
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
      const items = Array.isArray(json?.data) ? json.data : [];
      const awemeList = items
        .map((item) => item?.aweme_info || item?.awemeInfo || item?.aweme_detail || null)
        .filter(Boolean);
      const nextOffset = Number(json?.offset ?? json?.next_offset ?? offset + items.length);
      const hasMoreFlag = json?.has_more ?? json?.hasMore;
      const pageHasMore = typeof hasMoreFlag === 'boolean'
        ? hasMoreFlag
        : (hasMoreFlag === 1 || hasMoreFlag === '1' || items.length >= count);
      return {
        list: awemeList,
        offset: Number.isFinite(nextOffset) ? nextOffset : offset + items.length,
        hasMore: Boolean(pageHasMore),
        sourceUrl: url,
      };
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

  throw lastError || new Error('获取搜索结果作品列表失败');
}

function collectDouyinMediaUrls(value, depth = 0) {
  if (!value || depth > 4) return [];
  if (typeof value === 'string') {
    const trimmed = value.trim();
    return trimmed && trimmed !== '[object Object]' ? [trimmed] : [];
  }
  if (Array.isArray(value)) {
    return value.flatMap((item) => collectDouyinMediaUrls(item, depth + 1));
  }
  if (typeof value === 'object') {
    const urls = [];
    const listKeys = ['url_list', 'urlList', 'urls', 'origin_url_list', 'originUrlList'];
    for (const key of listKeys) {
      urls.push(...collectDouyinMediaUrls(value[key], depth + 1));
    }
    const directKeys = ['url', 'src'];
    for (const key of directKeys) {
      urls.push(...collectDouyinMediaUrls(value[key], depth + 1));
    }
    return urls;
  }
  return [];
}

function getDouyinAwemeCoverCandidates(aweme = {}) {
  const video = aweme?.video && typeof aweme.video === 'object' ? aweme.video : {};
  const candidates = [
    video.cover,
    video.origin_cover,
    video.dynamic_cover,
    video.animated_cover,
  ].flatMap((value) => collectDouyinMediaUrls(value));
  return [...new Set(candidates.filter(Boolean))];
}

export function buildDouyinBatchTargetFromAweme(aweme = {}, index = 0) {
  const awemeId = String(aweme?.aweme_id || '').trim();
  if (!awemeId) return null;
  const coverCandidates = getDouyinAwemeCoverCandidates(aweme);
  const cover = coverCandidates[0] || '';

  return {
    key: awemeId,
    awemeId,
    href: `https://www.douyin.com/video/${awemeId}`,
    titleHint: String(aweme?.desc || '').trim(),
    cover,
    coverImg: cover,
    coverUrl: cover,
    thumbnail: cover,
    images: cover ? [cover] : [],
    imageCandidates: coverCandidates.map((url) => ({ url })),
    likes: Number(aweme?.statistics?.digg_count || 0),
    comments: Number(aweme?.statistics?.comment_count || 0),
    createTime: Number(aweme?.create_time || 0),
    orderIndex: index,
    aweme,
  };
}

export function buildDouyinSearchTargetFromAweme(aweme = {}, index = 0, searchKeyword = '') {
  const target = buildDouyinBatchTargetFromAweme(aweme, index);
  if (!target) return null;
  return {
    ...target,
    searchKeyword: String(searchKeyword || '').trim(),
  };
}

function getCapturedDouyinSearchTargets(keyword, searchChannel) {
  const merged = mergeCapturedDouyinSearchPages(window.__lgboom_dy_search_pages || [], {
    keyword,
    searchChannel,
  });

  const targets = merged.items
    .map((item, index) => {
      const target = buildDouyinSearchTargetFromAweme(
        item.aweme,
        index,
        item.keyword || keyword,
      );
      if (!target) return null;
      return {
        ...target,
        sourceUrl: item.sourceUrl || 'captured.search_stream',
        searchKeyword: String(item.keyword || keyword || '').trim(),
      };
    })
    .filter(Boolean);

  return {
    targets,
    hasMore: Boolean(merged.hasMore),
    nextOffset: Number(merged.nextOffset || 0),
    pageCount: Number(merged.pageCount || 0),
  };
}

export async function discoverDouyinSearchTargets({
  maxCount = 20,
  topByLikes = false,
  shouldStop = () => false,
  waitIfPaused = async () => {},
} = {}) {
  const keyword = getCurrentDouyinSearchKeyword();
  if (!keyword) {
    throw new Error('未能识别当前搜索词');
  }

  const pageLimit = topByLikes ? 10 : Math.max(2, Math.ceil(maxCount / 8));
  const seen = new Set();
  const targets = [];
  const searchChannel = normalizeDouyinSearchChannel(getDouyinSearchTabType());
  const captured = getCapturedDouyinSearchTargets(keyword, searchChannel);

  for (const target of captured.targets) {
    if (seen.has(target.key)) continue;
    seen.add(target.key);
    targets.push(target);
  }

  let offset = Number(captured.nextOffset || 0);
  let pageNo = Number(captured.pageCount || 0);
  let more = captured.targets.length > 0 ? Boolean(captured.hasMore) : true;
  const pacer = createDouyinBatchPacer({
    baseRange: topByLikes ? { min: 100, max: 160 } : { min: 180, max: 280 },
  });

  if (!topByLikes && targets.length >= maxCount) {
    return targets.slice(0, maxCount);
  }

  while (more && pageNo < pageLimit) {
    await waitIfPaused();
    if (shouldStop()) break;

    let page;
    try {
      page = await fetchDouyinSearchVideoPage(keyword, {
        offset,
        count: topByLikes ? 10 : Math.min(10, Math.max(maxCount, 10)),
      });
      pacer.recordSuccess();
    } catch (error) {
      pacer.recordFailure();
      throw error;
    }

    const pageTargets = (page.list || [])
      .map((aweme, index) => {
        const target = buildDouyinSearchTargetFromAweme(aweme, targets.length + index, keyword);
        if (!target) return null;
        return {
          ...target,
          sourceUrl: page.sourceUrl,
          searchKeyword: keyword,
        };
      })
      .filter(Boolean);

    for (const target of pageTargets) {
      if (seen.has(target.key)) continue;
      seen.add(target.key);
      targets.push(target);
    }

    pageNo += 1;
    offset = page.offset;
    more = Boolean(page.hasMore);

    if (!topByLikes && targets.length >= maxCount) break;
    await pacer.wait({ waitIfPaused, shouldStop });
  }

  if (topByLikes) {
    return targets
      .sort((a, b) => {
        const likesDiff = Number(b.likes || 0) - Number(a.likes || 0);
        if (likesDiff !== 0) return likesDiff;
        return Number(a.orderIndex || 0) - Number(b.orderIndex || 0);
      })
      .slice(0, maxCount)
      .map((target, index) => ({ ...target, rank: index + 1 }));
  }

  return targets.slice(0, maxCount);
}

export async function discoverDouyinBatchTargets({
  maxCount = 20,
  topByLikes = false,
  shouldStop = () => false,
  waitIfPaused = async () => {},
} = {}) {
  const page = detectDouyinPageType();
  const searchContext = detectDouyinSearchBatchContext();
  const searchTabType = getDouyinSearchTabType();
  if (searchContext.stableSearchList) {
    let domTargets = [];
    try {
      domTargets = await discoverDouyinSearchTargetsFromDom({
        maxCount,
        topByLikes: false,
        shouldStop,
        waitIfPaused,
      });
    } catch {
      domTargets = [];
    }

    // 视频搜索页（有 <a href="/video/..."> 链接）：优先 DOM 发现，API 兜底
    // 综合搜索页（无视频链接）：直接走 API 发现
    const tabType = getDouyinSearchTabType();
    if (!topByLikes && domTargets.length > 0 && tabType === 'video') {
      try {
        const apiTargets = await discoverDouyinSearchTargets({
          maxCount,
          topByLikes,
          shouldStop,
          waitIfPaused,
        });
        const mergedTargets = mergeDouyinSearchTargetsByVisibleOrder(domTargets, apiTargets);
        if (mergedTargets.length > 0) return mergedTargets.slice(0, maxCount);
      } catch {
        return domTargets.slice(0, maxCount);
      }
    }

    // API 发现：综合页主路径，视频页兜底
    try {
      const apiTargets = await discoverDouyinSearchTargets({
        maxCount,
        topByLikes,
        shouldStop,
        waitIfPaused,
      });
      if (!topByLikes) {
        const mergedTargets = mergeDouyinSearchTargetsByVisibleOrder(domTargets, apiTargets);
        if (mergedTargets.length > 0) return mergedTargets.slice(0, maxCount);
      }
      if (apiTargets.length > 0) return apiTargets;
    } catch {
      // API 也失败了
    }

    // 最后尝试 DOM 发现（兜底，用于非标准视频搜索页）
    if (domTargets.length > 0) {
      if (topByLikes) {
        return domTargets
          .sort((a, b) => {
            const likesDiff = Number(b.likes || 0) - Number(a.likes || 0);
            if (likesDiff !== 0) return likesDiff;
            return Number(a.orderIndex || 0) - Number(b.orderIndex || 0);
          })
          .slice(0, maxCount)
          .map((target, index) => ({ ...target, rank: index + 1 }));
      }
      return domTargets.slice(0, maxCount);
    }

    throw new Error('当前搜索页未识别到视频结果，请停留在搜索结果页再批量采集');
  }
  if (page.type !== DY_PAGE_TYPE.PROFILE) {
    throw new Error('请在抖音博主主页，或带稳定搜索结果列表的抖音搜索页使用批量采集');
  }

  const secUserId = getProfileSecUserId();
  if (!secUserId) {
    throw new Error('未能识别当前博主主页 secUid');
  }

  const pageLimit = topByLikes ? 10 : Math.max(2, Math.ceil(maxCount / 12));
  const seen = new Set();
  const targets = [];
  let cursor = 0;
  let pageNo = 0;
  let more = true;
  const pacer = createDouyinBatchPacer({
    baseRange: topByLikes ? { min: 100, max: 160 } : { min: 180, max: 280 },
  });

  while (more && pageNo < pageLimit) {
    await waitIfPaused();
    if (shouldStop()) break;

    let page;
    try {
      page = await fetchDouyinProfileVideoPage(secUserId, {
        cursor,
        count: topByLikes ? 18 : Math.min(18, Math.max(maxCount, 10)),
      });
      pacer.recordSuccess();
    } catch (error) {
      pacer.recordFailure();
      throw error;
    }

    const pageTargets = (page.list || [])
      .map((aweme, index) => {
        const target = buildDouyinBatchTargetFromAweme(aweme, targets.length + index);
        if (!target) return null;
        return {
          ...target,
          sourceUrl: page.sourceUrl,
        };
      })
      .filter(Boolean);

    for (const target of pageTargets) {
      if (seen.has(target.key)) continue;
      seen.add(target.key);
      targets.push(target);
    }

    pageNo += 1;
    cursor = page.cursor;
    more = Boolean(page.hasMore);

    if (!topByLikes && targets.length >= maxCount) break;
    await pacer.wait({ waitIfPaused, shouldStop });
  }

  if (topByLikes) {
    return targets
      .sort((a, b) => {
        const likesDiff = Number(b.likes || 0) - Number(a.likes || 0);
        if (likesDiff !== 0) return likesDiff;
        return Number(a.orderIndex || 0) - Number(b.orderIndex || 0);
      })
      .slice(0, maxCount)
      .map((target, index) => ({ ...target, rank: index + 1 }));
  }

  return targets.slice(0, maxCount);
}

export async function waitDouyinBatchStep(step, { random = Math.random } = {}) {
  const delay = pickDouyinBatchDelay(step, { random });
  await sleep(delay);
  return delay;
}
