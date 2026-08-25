/**
 * 抖音视频采集器
 *
 * 采集方式：DOM 解析 + API 捕获（已验证选择器，2026-03-24）
 * 覆盖场景：视频详情页（/video/{id} 或 ?modal_id={id}）
 * 存储表：notes（platform: 'douyin'）
 */

import { MSG } from '../../shared/constants.js';
import { sendToBackground } from '../../shared/messaging.js';
import { noteStore } from '../../db/noteStore.js';
import { createCollectorEvidence, createCollectorQualityMeta, joinRawDomText } from '../../shared/collectorMetadata.js';
import { extractDouyinContentId } from './pageDetector.js';
import { createVideoContextHelpers } from './videoContext.js';
import { createVideoDomHelpers } from './videoDom.js';
import { sanitizeFilename, downloadViaBlobFallback } from './videoDownload.js';
import { dedupeStrings, normalizeRemoteUrl as toAbsUrl } from './commentApi.js';
import {
  extractHashtags,
  sanitizeVideoTitle,
  getApiVideoData,
  getApiVideoDataByCandidates,
  registerVideoAliases,
  getRenderVideoDetail,
  getRouterVideoData,
  getRenderVideoId,
  normalizeIpLocation,
  parseLocationFromInfoText,
  mapRenderVideoToCache,
  mapAwemeDetailToApiData,
  mapRouterVideoToCache,
  fetchDetailApiData,
  hasUsableApiVideo,
  resolveApiVideoData,
} from './videoApiData.js';
import { withMonitorRecordMeta } from '../../workbench/runtime/monitorTask.js';

// ========== 已验证选择器（2026-03-24）==========
const SEL = {
  desc:         '[data-e2e="video-desc"], [data-e2e="detail-video-info"]',
  authorNick:   '[data-e2e="feed-video-nickname"], [data-e2e="user-info"]',
  videoInfo:    '[data-e2e="video-info"]',       // 老版：@作者·1天前·贵州
  likes:        '[data-e2e="video-player-digg"]',
  commentCount: '[data-e2e="feed-comment-icon"]',
  collects:     '[data-e2e="video-player-collect"]',
  shares:       '[data-e2e="video-player-share"]',
  videoEl:      'video',
};

function getUrlVideoId(url = window.location.href) {
  return extractDouyinContentId(url) || '';
}

function getUrlPathVideoId(url = window.location.href) {
  try {
    const parsed = new URL(url);
    const pathname = parsed.pathname || '';
    const match = pathname.match(/^\/(video|note)\/([A-Za-z0-9_-]+)/);
    return String(match?.[2] || '').trim();
  } catch {
    return '';
  }
}

function hasExplicitVideoUrl(url = window.location.href) {
  try {
    const parsed = new URL(url);
    if (parsed.searchParams.get('modal_id')) return true;
    return /^\/(video|note)\//.test(parsed.pathname || '');
  } catch {
    return false;
  }
}

function getUrlVidParam(url = window.location.href) {
  try {
    const parsed = new URL(url);
    return String(parsed.searchParams.get('vid') || '').trim();
  } catch {
    return '';
  }
}

function getUrlModalId(url = window.location.href) {
  try {
    const parsed = new URL(url);
    return String(parsed.searchParams.get('modal_id') || '').trim();
  } catch {
    return '';
  }
}

function getCurrentCardHintId(url = window.location.href) {
  const explicitPathId = getUrlPathVideoId(url);
  if (explicitPathId) return explicitPathId;
  const activeId = getActiveVideoIdFromDom();
  if (activeId) return activeId;
  const awemeId = getAwemeIdFromDom();
  if (awemeId) return awemeId;
  const modalId = getUrlModalId(url);
  if (modalId) return modalId;
  return getUrlVidParam(url);
}

const {
  readCount,
  extractNicknameFromEl,
  extractIpLocation,
  extractCoverImg,
  extractAuthorIdFromDOM,
  waitForElement,
  waitForContentSettle,
} = createVideoDomHelpers({
  SEL,
  queryInActiveVideo: (...args) => queryInActiveVideo(...args),
  getApiVideoData,
  normalizeIpLocation,
  parseLocationFromInfoText,
});

export function inferDouyinVideoQualityMeta(sourceValue = '', { contextMode = '' } = {}) {
  const source = String(sourceValue || '').trim().toLowerCase();
  const mode = String(contextMode || '').trim().toLowerCase();
  const effective = source || mode;

  if (effective === 'search_dom_result') {
    return createCollectorQualityMeta({
      dataQuality: 'seed',
      qualityReason: 'search_summary_seed',
      sourceTier: 'seed',
    });
  }

  if (effective === 'detail_api_fallback') {
    return createCollectorQualityMeta({
      dataQuality: 'seed',
      qualityReason: 'aweme_seed_without_detail',
      sourceTier: 'seed',
    });
  }

  if (effective === 'dom' || /dom/.test(effective)) {
    return createCollectorQualityMeta({
      dataQuality: 'degraded',
      qualityReason: 'detail_context_dom_fallback',
      sourceTier: 'dom',
    });
  }

  if (/render|router|cache/.test(effective)) {
    return createCollectorQualityMeta({
      dataQuality: 'full',
      sourceTier: 'mixed',
    });
  }

  return createCollectorQualityMeta({
    dataQuality: 'full',
    sourceTier: effective.includes('api') ? 'api' : 'mixed',
  });
}

const {
  queryInActiveVideo,
  getActiveVideoIdFromDom,
  getAwemeIdFromDom,
  buildCurrentVideoContext,
  getContextDomTitle,
  titlesMatch,
  matchContextData,
  resolveCurrentVideoId,
  resolveContextApiData,
  resolveDownloadApiDataForContext,
  resolveStableVideoContext,
} = createVideoContextHelpers({
  SEL,
  dedupeStrings,
  sanitizeVideoTitle,
  getCurrentCardHintId,
  getRenderVideoDetail,
  getRouterVideoData,
  getRenderVideoId,
  getUrlPathVideoId,
  getUrlVidParam,
  getUrlModalId,
  getApiVideoData,
  fetchDetailApiData,
  registerVideoAliases,
  hasUsableApiVideo,
  waitForElement,
  waitForContentSettle,
});

export async function refreshDouyinNoteMediaById(note = {}) {
  const videoId = String(note?.platformContentId || '').trim()
    || String(note?.noteId || '').replace(/^dy_/, '').trim();
  if (!videoId) return null;

  const apiData = await resolveApiVideoData(videoId);
  if (!apiData) return null;

  const candidates = dedupeStrings([
    apiData.videoDownloadUrl,
    apiData.videoPlayUrl,
    note.videoDownloadUrl,
    note.videoPlayUrl,
    note.video,
  ]);
  const primaryVideo = candidates[0] || '';

  return {
    ...note,
    platformContentId: videoId,
    noteId: note.noteId || `dy_${videoId}`,
    contentId: note.contentId || `dy_${videoId}`,
    video: primaryVideo,
    videoPlayUrl: apiData.videoPlayUrl || primaryVideo,
    videoDownloadUrl: apiData.videoDownloadUrl || apiData.videoPlayUrl || primaryVideo,
    videoStreams: candidates.map((url, index) => ({
      url,
      quality: index === 0 ? 'primary' : 'fallback',
      bitrate: 0,
    })),
    cover: apiData.coverImg || note.cover || note.coverImg || '',
    coverImg: apiData.coverImg || note.coverImg || note.cover || '',
    dataSource: apiData.sourceUrl || note.dataSource || 'detail_api_refresh',
    updatedAt: Date.now(),
    lastUpdateTime: new Date().toLocaleString('zh-CN'),
  };
}

export async function collectDouyinVideoById(videoId, options = {}) {
  const resolvedVideoId = String(videoId || '').trim();
  if (!resolvedVideoId) {
    return { ok: false, error: '缺少视频 ID' };
  }

  let apiData = await resolveApiVideoData(resolvedVideoId, {
    suppressErrors: !Boolean(options.propagateSecurityChallenge),
  });
  if (!apiData) {
    const titleHint = sanitizeVideoTitle(options.titleHint || '');
    const authorHint = String(options.authorHint || '').trim();
    if (!titleHint && !authorHint) {
      return { ok: false, error: '未能获取视频详情数据' };
    }
    apiData = {
      id: resolvedVideoId,
      desc: titleHint || `抖音视频_${resolvedVideoId}`,
      authorName: authorHint,
      hashtags: extractHashtags(titleHint || ''),
      statsLikes: Number(options.batchLikesSnapshot || 0),
      sourceUrl: 'search_dom_result',
      fetchedAt: Date.now(),
    };
  }

  const record = await buildDouyinVideoRecord(apiData, {
    ...options,
    videoId: resolvedVideoId,
    fallbackUrl: `https://www.douyin.com/video/${resolvedVideoId}`,
    defaultDataSource: apiData.sourceUrl === 'search_dom_result' ? 'search_dom_result' : 'detail_api',
  });
  return { ok: true, data: record };
}

async function buildDouyinVideoRecord(apiData = {}, options = {}) {
  const resolvedVideoId = String(options.videoId || apiData.id || '').trim();
  if (!resolvedVideoId) {
    throw new Error('缺少视频 ID');
  }

  const title = sanitizeVideoTitle(apiData.desc || options.titleHint || '') || `抖音视频_${resolvedVideoId}`;
  const publishedAtText = String(options.timeHint || '').trim();
  let ipLocation = normalizeIpLocation(apiData.ipLocation || '');
  if (!ipLocation) {
    ipLocation = await fetchIpFromAuthorProfile(apiData.authorSecUid || '');
  }

  const videoCandidates = dedupeStrings([
    apiData.videoDownloadUrl,
    apiData.videoPlayUrl,
  ]);
  const primaryVideo = videoCandidates[0] || '';
  const cover = apiData.coverImg || '';
  const canonicalUrl = String(options.url || options.fallbackUrl || `https://www.douyin.com/video/${resolvedVideoId}`).trim();
  const collectedAt = Date.now();
  const contentId = `dy_${resolvedVideoId}`;
  const shareText = String(options.shareText || '').trim();
  const shareShortUrl = String(options.shareShortUrl || '').trim();
  const collectionRunId = String(options.collectionRunId || '').trim();
  const batchSelectionMode = String(options.batchSelectionMode || '').trim();
  const searchKeyword = String(options.searchKeyword || '').trim();
  const searchPageUrl = String(options.searchPageUrl || '').trim();
  const batchRank = Number(options.batchRank || 0);
  const batchLikesSnapshot = Number.isFinite(Number(options.batchLikesSnapshot))
    ? Number(options.batchLikesSnapshot)
    : Number(apiData.statsLikes || 0);

  const record = withMonitorRecordMeta({
    noteId: contentId,
    contentId,
    platformContentId: resolvedVideoId,
    platform: 'douyin',
    url: canonicalUrl,
    canonicalUrl,
    title,
    content: title,
    bodyText: title,
    type: 'video',
    authorId: String(apiData.authorId || '').trim(),
    authorEntityId: apiData.authorId ? `dy_${String(apiData.authorId).trim()}` : '',
    authorName: String(apiData.authorName || options.authorHint || '').trim(),
    authorAvatar: String(apiData.authorAvatar || '').trim(),
    cover,
    coverImg: cover,
    images: [],
    imageCandidates: [],
    video: primaryVideo,
    videoPlayUrl: apiData.videoPlayUrl || primaryVideo,
    videoDownloadUrl: apiData.videoDownloadUrl || apiData.videoPlayUrl || primaryVideo,
    videoStreams: videoCandidates.map((url, index) => ({
      url,
      quality: index === 0 ? 'primary' : 'fallback',
      bitrate: 0,
    })),
    likes: Number(apiData.statsLikes || 0),
    comments: Number(apiData.statsComments || 0),
    collects: Number(apiData.statsCollects || 0),
    shares: Number(apiData.statsShares || 0),
    playCount: Number(apiData.playCount || 0),
    duration: Number(apiData.duration || 0),
    ipLocation,
    releaseDate: Number(apiData.releaseDate || 0),
    publishedAt: Number(apiData.releaseDate || 0),
    publishedAtText,
    hashtags: Array.isArray(apiData.hashtags) ? apiData.hashtags : extractHashtags(title),
    mediaDownloadStatus: primaryVideo ? '待下载' : '无媒体',
    collectedAt,
    updatedAt: collectedAt,
    createdAt: collectedAt,
    lastUpdateTime: new Date().toLocaleString('zh-CN'),
    dataSource: apiData.sourceUrl || options.defaultDataSource || 'detail_api',
    triggerSource: String(options.triggerSource || 'manual').trim() || 'manual',
    collectionRunId: collectionRunId || undefined,
    batchSelectionMode: batchSelectionMode || undefined,
    batchRank: batchSelectionMode && Number.isFinite(batchRank) && batchRank > 0 ? batchRank : undefined,
    batchLikesSnapshot: batchSelectionMode ? batchLikesSnapshot : undefined,
    searchKeyword: searchKeyword || undefined,
    searchPageUrl: searchPageUrl || undefined,
    shareText: shareText || undefined,
    shareShortUrl: shareShortUrl || undefined,
    shareCapturedAt: shareText ? Date.now() : undefined,
    syncStatus: 'local',
    ...inferDouyinVideoQualityMeta(apiData.sourceUrl || options.defaultDataSource || 'detail_api'),
    ...createCollectorEvidence({
      rawPayload: apiData,
      rawDomText: joinRawDomText([
        title,
        String(apiData.authorName || '').trim(),
        ipLocation,
      ]),
      rawShareText: shareText,
      rawUrl: canonicalUrl,
      rawSource: apiData.sourceUrl || options.defaultDataSource || 'detail_api',
    }),
  }, options.monitorMeta);

  await noteStore.upsert(record);
  return record;
}

export async function collectDouyinVideoByAweme(aweme = {}, options = {}) {
  const awemeId = String(aweme?.aweme_id || aweme?.awemeId || '').trim();
  if (!awemeId) {
    return { ok: false, error: '缺少 aweme_id' };
  }

  let mapped = mapAwemeDetailToApiData(aweme, String(options.sourceUrl || '').trim() || 'profile_aweme_list');
  if (!mapped) {
    return { ok: false, error: '未能解析作品列表中的视频数据' };
  }

  const sourceUrl = String(options.sourceUrl || '').trim();
  const triggerSource = String(options.triggerSource || '').trim();
  const isSearchSeed = Boolean(
    String(options.searchKeyword || '').trim()
    || /search/i.test(sourceUrl)
    || /search/i.test(triggerSource)
  );

  // 搜索结果页和列表 API 的 aweme 经常只带半残视频信息。
  // 这类记录如果不先补 detail，后续面板就会落成“无媒体”。
  if (isSearchSeed || !hasUsableApiVideo(mapped)) {
    try {
      const detailData = await fetchDetailApiData(awemeId, {
        suppressErrors: !Boolean(options.propagateSecurityChallenge),
      });
      if (detailData && hasUsableApiVideo(detailData)) {
        mapped = { ...mapped, ...detailData };
      }
    } catch (error) {
      if (options.propagateSecurityChallenge) throw error;
      // detail API 失败则继续用现有数据兜底
    }
  }

  const record = await buildDouyinVideoRecord(mapped, {
    ...options,
    videoId: awemeId,
    fallbackUrl: `https://www.douyin.com/video/${awemeId}`,
    defaultDataSource: hasUsableApiVideo(mapped)
      ? (isSearchSeed ? 'search_detail_seed' : 'profile_aweme_list')
      : 'detail_api_fallback',
  });
  return { ok: true, data: record };
}

function pickLocationFromApiPayload(payload = null) {
  if (!payload || typeof payload !== 'object') return '';
  const queue = [payload];
  const visited = new Set();
  const keys = ['ip_location', 'ipLocation', 'ip_label', 'ipLabel', 'location', 'region', 'province', 'city', 'district'];

  while (queue.length > 0) {
    const obj = queue.shift();
    if (!obj || typeof obj !== 'object' || visited.has(obj)) continue;
    visited.add(obj);

    for (const [key, value] of Object.entries(obj)) {
      if (value && typeof value === 'object') {
        queue.push(value);
        continue;
      }
      if (value == null) continue;
      const lowerKey = key.toLowerCase();
      if (!keys.some((k) => lowerKey.includes(String(k).toLowerCase()))) continue;
      const parsed = normalizeIpLocation(String(value));
      if (parsed) return parsed;
    }
  }
  return '';
}

async function fetchIpFromAuthorProfile(secUserId = '') {
  if (!secUserId) return '';
  const encoded = encodeURIComponent(secUserId);
  const apiUrls = [
    `/aweme/v1/web/user/profile/other/?sec_user_id=${encoded}&aid=6383`,
    `/aweme/v1/web/user/profile/other/?sec_user_id=${encoded}`,
    `/aweme/v1/web/user/profile/?sec_user_id=${encoded}&aid=6383`,
    `/aweme/v1/web/user/profile/?sec_user_id=${encoded}`,
  ];
  for (const apiUrl of apiUrls) {
    try {
      const resp = await fetch(apiUrl, { credentials: 'include' });
      if (!resp.ok) continue;
      const json = await resp.json();
      const ip = pickLocationFromApiPayload(json);
      if (ip) return ip;
    } catch {
      // 尝试下一个 API
    }
  }
  return '';
}

// ========== 主采集函数 ==========

/**
 * 从视频详情页 DOM + API 缓存采集视频基础信息
 * @returns {{ ok: boolean, data?: object, error?: string }}
 */
export async function collectDouyinVideo(options = {}) {
  const context = await resolveStableVideoContext({ waitForDesc: true, settleMs: 2200 });
  if (!context.primaryId) {
    return { ok: false, error: '无法定位当前正在播放的视频' };
  }

  const domTitle = getContextDomTitle(context);
  const apiData = (await resolveContextApiData(context, { domTitle })) || {};
  const resolvedVideoId = String(apiData?.id || context.primaryId || '').trim();
  if (!resolvedVideoId) {
    return { ok: false, error: '无法解析当前视频 ID' };
  }
  registerVideoAliases(resolvedVideoId, context.aliasIds || context.fetchIds || context.candidateIds);

  const routerMapped = matchContextData(
    context.routerVideo ? mapRouterVideoToCache(context.routerVideo, context.url) : null,
    resolvedVideoId,
    domTitle,
  );
  const renderMapped = matchContextData(
    context.renderDetail ? mapRenderVideoToCache(context.renderDetail, context.url) : null,
    resolvedVideoId,
    domTitle,
  );

  const scopeHint = context.scopeHintId || resolvedVideoId;
  const descEl = queryInActiveVideo('[data-e2e="video-desc"]', scopeHint)
    || queryInActiveVideo('[data-e2e="detail-video-info"]', scopeHint);
  const nickEl = queryInActiveVideo('[data-e2e="feed-video-nickname"]', scopeHint)
    || queryInActiveVideo('[data-e2e="user-info"]', scopeHint);
  const videoEl = queryInActiveVideo(SEL.videoEl, scopeHint);
  const cached = getApiVideoDataByCandidates([resolvedVideoId, ...(context.aliasIds || context.fetchIds || [])]) || {};
  const merged = {
    ...(routerMapped || {}),
    ...(renderMapped || {}),
    ...(cached || {}),
    ...(apiData || {}),
  };

  const title = sanitizeVideoTitle(domTitle || descEl?.textContent || merged.desc || '') || `抖音视频_${resolvedVideoId}`;
  const authorName = merged.authorName || extractNicknameFromEl(nickEl);
  const authorId = merged.authorId || extractAuthorIdFromDOM(scopeHint);
  const likes = Math.max(Number(merged.statsLikes || 0), readCount(SEL.likes, scopeHint));
  const comments = Math.max(Number(merged.statsComments || 0), readCount(SEL.commentCount, scopeHint));
  const collects = Math.max(Number(merged.statsCollects || 0), readCount(SEL.collects, scopeHint));
  const shares = Math.max(Number(merged.statsShares || 0), readCount(SEL.shares, scopeHint));

  let ipLocation = merged.ipLocation || extractIpLocation(merged, scopeHint);
  if (!ipLocation) {
    ipLocation = await fetchIpFromAuthorProfile(merged.authorSecUid || '');
  }

  const cover = extractCoverImg(resolvedVideoId, merged, scopeHint);
  const domVideoUrl = videoEl?.currentSrc || videoEl?.src || '';
  const videoCandidates = dedupeStrings([
    merged.videoDownloadUrl,
    merged.videoPlayUrl,
    domVideoUrl,
  ]);
  const primaryVideo = videoCandidates[0] || '';
  const videoStreams = videoCandidates.map((url, index) => ({
    url,
    quality: index === 0 ? 'primary' : 'fallback',
    bitrate: 0,
  }));
  const publishedAtText = String(
    options.timeHint
      || merged.publishedAtText
      || '',
  ).trim();
  const shareText = String(options.shareText || '').trim();
  const shareShortUrl = shareText.match(/https:\/\/v\.douyin\.com\/[A-Za-z0-9]+\/?/i)?.[0] || '';
  const triggerSource = String(options.triggerSource || 'manual').trim() || 'manual';
  const collectionRunId = String(options.collectionRunId || '').trim();

  const collectedAt = Date.now();
  const contentId = `dy_${resolvedVideoId}`;
  const record = withMonitorRecordMeta({
    noteId: contentId ,
    contentId,
    platformContentId: resolvedVideoId,
    platform: 'douyin',
    url: window.location.href,
    canonicalUrl: window.location.href,
    title,
    content: title,
    bodyText: title,
    type: 'video',
    authorId,
    authorEntityId: authorId ? `dy_${authorId}` : '',
    authorName,
    authorAvatar: merged.authorAvatar || '',
    cover,
    coverImg: cover,
    images: [],
    imageCandidates: [],
    video: primaryVideo,
    videoPlayUrl: merged.videoPlayUrl || primaryVideo,
    videoDownloadUrl: merged.videoDownloadUrl || primaryVideo || '',
    videoStreams,
    likes,
    comments,
    collects,
    shares,
    playCount: Number(merged.playCount || 0),
    duration: Number(merged.duration || 0),
    ipLocation,
    releaseDate: Number(merged.releaseDate || 0),
    publishedAt: Number(merged.releaseDate || 0),
    publishedAtText,
    hashtags: Array.isArray(merged.hashtags) ? merged.hashtags : extractHashtags(title || ''),
    mediaDownloadStatus: primaryVideo ? '待下载' : '无媒体',
    collectedAt,
    updatedAt: collectedAt,
    createdAt: collectedAt,
    lastUpdateTime: new Date().toLocaleString('zh-CN'),
    dataSource: merged.sourceUrl || context.mode || 'dom',
    triggerSource,
    collectionRunId: collectionRunId || undefined,
    shareText: shareText || undefined,
    shareShortUrl: shareShortUrl || undefined,
    shareCapturedAt: shareText ? Date.now() : undefined,
    syncStatus: 'local',
    ...inferDouyinVideoQualityMeta(merged.sourceUrl || context.mode || 'dom', {
      contextMode: context.mode,
    }),
    ...createCollectorEvidence({
      rawPayload: {
        context: {
          mode: context.mode,
          primaryId: context.primaryId,
          aliasIds: context.aliasIds || [],
          fetchIds: context.fetchIds || [],
        },
        merged,
      },
      rawDomText: joinRawDomText([
        title,
        authorName,
        ipLocation,
        context.domTitle || '',
      ]),
      rawShareText: shareText,
      rawUrl: window.location.href,
      rawSource: merged.sourceUrl || context.mode || 'dom',
    }),
  }, options.monitorMeta);

  await noteStore.upsert(record);
  return { ok: true, data: record, context };
}

/**
 * 下载当前页面的视频
 * 策略：
 *   1. 优先用 API 捕获的真实下载 URL（无水印）
 *   2. 回退到 <video> src（可能有水印，blob URL 在某些情况下可下载）
 *
 * @returns {{ ok: boolean, error?: string }}
 */
export async function downloadDouyinVideo() {
  const context = await resolveStableVideoContext({ waitForDesc: true, settleMs: 1800 });
  if (!context.primaryId) {
    return { ok: false, error: '无法定位当前正在播放的视频' };
  }

  const domTitle = getContextDomTitle(context);
  let apiData = (await resolveContextApiData(context, { domTitle })) || {};
  let resolvedVideoId = String(apiData?.id || context.primaryId || '').trim();
  if (!resolvedVideoId) {
    return { ok: false, error: '无法解析当前视频 ID' };
  }
  registerVideoAliases(resolvedVideoId, context.aliasIds || context.fetchIds || context.candidateIds);

  const routerMapped = matchContextData(
    context.routerVideo ? mapRouterVideoToCache(context.routerVideo, context.url) : null,
    resolvedVideoId,
    domTitle,
  );
  const renderMapped = matchContextData(
    context.renderDetail ? mapRenderVideoToCache(context.renderDetail, context.url) : null,
    resolvedVideoId,
    domTitle,
  );

  const scopeHint = context.scopeHintId || resolvedVideoId;
  const videoEl = queryInActiveVideo(SEL.videoEl, scopeHint);
  const domVideoUrl = videoEl?.currentSrc || videoEl?.src || '';
  let cached = getApiVideoDataByCandidates([resolvedVideoId, ...(context.aliasIds || context.fetchIds || [])]) || {};
  let merged = {
    ...(routerMapped || {}),
    ...(renderMapped || {}),
    ...(cached || {}),
    ...(apiData || {}),
  };

  const buildCandidates = () => {
    const rawCandidates = dedupeStrings([
      apiData?.videoDownloadUrl,
      apiData?.videoPlayUrl,
      cached?.videoDownloadUrl,
      cached?.videoPlayUrl,
      merged?.videoDownloadUrl,
      merged?.videoPlayUrl,
      domVideoUrl,
    ]);
    return {
      candidates: dedupeStrings(rawCandidates.map((url) => toAbsUrl(url))).filter((url) => /^https?:\/\//i.test(String(url || ''))),
      blobCandidates: dedupeStrings(rawCandidates).filter((url) => String(url || '').startsWith('blob:')),
    };
  };

  let { candidates, blobCandidates } = buildCandidates();
  if (candidates.length === 0) {
    const forced = await resolveDownloadApiDataForContext({
      ...context,
      primaryId: resolvedVideoId || context.primaryId,
      fetchIds: dedupeStrings([resolvedVideoId, ...(context.fetchIds || [])]),
      secondaryIds: dedupeStrings(context.secondaryIds || context.candidateIds || []),
      aliasIds: dedupeStrings([resolvedVideoId, ...(context.aliasIds || context.fetchIds || context.candidateIds || [])]),
    }, domTitle) || (await resolveContextApiData({
      ...context,
      fetchIds: [resolvedVideoId],
      candidateIds: [resolvedVideoId],
      aliasIds: [resolvedVideoId],
    }, { domTitle }));
    if (forced) {
      apiData = { ...apiData, ...forced };
      registerVideoAliases(String(forced.id || resolvedVideoId).trim(), context.aliasIds || context.fetchIds || context.candidateIds);
      resolvedVideoId = String(forced.id || resolvedVideoId).trim();
      cached = getApiVideoDataByCandidates([resolvedVideoId, ...(context.aliasIds || context.fetchIds || [])]) || cached;
      merged = {
        ...(routerMapped || {}),
        ...(renderMapped || {}),
        ...(cached || {}),
        ...(apiData || {}),
      };
      ({ candidates, blobCandidates } = buildCandidates());
    }
  }

    if (candidates.length === 0 && blobCandidates.length === 0) {
      return { ok: false, error: '视频地址尚未捕获。请先播放一下视频，等1-2秒后重试。' };
    }
    if (candidates.length === 0 && blobCandidates.length > 0) {
      return { ok: false, error: '仅捕获到 blob 临时地址，尚未拿到可下载直链。请切换到当前视频并等待 1-2 秒后重试。' };
    }

  const rawTitle = sanitizeVideoTitle(domTitle || merged?.desc || resolvedVideoId || 'douyin_video');
  const safeName = sanitizeFilename(rawTitle || 'douyin_video');

    try {
      // Douyin CDN 对 chrome.downloads.download 不友好（不支持 Referer，频繁 SERVER_FORBIDDEN），
      // 直接使用页面上下文 blob 下载（MAIN world fetch 带完整 cookie），避免产生多余的失败下载记录
      const fallback = await downloadViaBlobFallback([...candidates, ...blobCandidates], safeName);
      if (fallback.ok) return { ok: true, context };

      // 如果页面上下文下载失败，尝试刷新 API 获取新直链再试一次
      const refreshed = await fetchDetailApiData(resolvedVideoId);
      if (refreshed) {
        apiData = { ...apiData, ...refreshed };
        cached = getApiVideoDataByCandidates([resolvedVideoId, ...context.candidateIds]) || cached;
        merged = {
          ...(routerMapped || {}),
          ...(renderMapped || {}),
          ...(cached || {}),
          ...(apiData || {}),
        };
        ({ candidates, blobCandidates } = buildCandidates());
        if (candidates.length > 0 || blobCandidates.length > 0) {
          const retry = await downloadViaBlobFallback([...candidates, ...blobCandidates], safeName);
          if (retry.ok) return { ok: true, context };
        }
      }

      return { ok: false, error: `下载失败：${fallback.error || '未知错误'}` };
    } catch (err) {
      const fallback = await downloadViaBlobFallback([...candidates, ...blobCandidates], safeName);
      if (fallback.ok) return { ok: true, context };
      return { ok: false, error: `下载失败：${String(err?.message || err)}` };
    }
}
