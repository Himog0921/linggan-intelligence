import { fetchDouyinJson, dedupeStrings, normalizeRemoteUrl as toAbsUrl } from './commentApi.js';


export function extractHashtags(text = '') {
  const raw = String(text || '');
  const matches = raw.match(/#\s*[^\s#，。！？,.!?:：;；、]+/g) || [];
  return matches.map((tag) => tag.replace(/^#/, '').trim()).filter(Boolean);
}

export function sanitizeVideoTitle(raw = '') {
  return String(raw || '')
    .replace(/#\s*[^\s#，。！？,.!?:：;；、]+/g, '')
    .replace(/^\s*(展开|收起)\s*/g, '')
    .replace(/\s*发布时间[:：][\s\S]*$/g, '')
    .replace(/\s*(展开|收起|全文|更多)\s*$/g, '')
    .replace(/\s+/g, ' ')
    .trim();
}

function ensureVideoCacheState() {
  window.__lgboom_dy_video_data = window.__lgboom_dy_video_data || {};
  window.__lgboom_dy_video_aliases = window.__lgboom_dy_video_aliases || {};
  return {
    entries: window.__lgboom_dy_video_data,
    aliases: window.__lgboom_dy_video_aliases,
  };
}

function getCanonicalVideoId(videoId = '') {
  const id = String(videoId || '').trim();
  if (!id) return '';
  const { aliases } = ensureVideoCacheState();
  return String(aliases[id] || id).trim();
}

export function registerVideoAliases(canonicalId, aliases = []) {
  const baseId = String(canonicalId || '').trim();
  if (!baseId) return;
  const { aliases: aliasMap } = ensureVideoCacheState();
  for (const candidate of dedupeStrings([baseId, ...(Array.isArray(aliases) ? aliases : [])])) {
    if (!candidate || candidate === baseId) continue;
    aliasMap[candidate] = baseId;
  }
}

export function getApiVideoData(videoId) {
  const id = String(videoId || '').trim();
  if (!id) return null;
  const { entries } = ensureVideoCacheState();
  const canonicalId = getCanonicalVideoId(id);
  return entries[canonicalId] || entries[id] || null;
}

export function getApiVideoDataByCandidates(candidates = []) {
  for (const id of dedupeStrings(candidates)) {
    const hit = getApiVideoData(id);
    if (hit) return hit;
  }
  return null;
}

function upsertApiVideoData(videoId, patch = {}, options = {}) {
  const canonicalId = String(videoId || '').trim();
  if (!canonicalId) return null;
  const { entries } = ensureVideoCacheState();
  const aliases = Array.isArray(options.aliases) ? options.aliases : [];
  registerVideoAliases(canonicalId, aliases);
  const prev = entries[canonicalId] || {};
  const merged = {
    ...prev,
    ...patch,
    id: canonicalId,
  };
  if (options.sourceUrl) merged.sourceUrl = options.sourceUrl;
  if (!Number.isFinite(Number(merged.fetchedAt)) || Number(merged.fetchedAt) <= 0) {
    merged.fetchedAt = Date.now();
  }
  entries[canonicalId] = merged;
  return merged;
}

export function getRenderData(doc = document) {
  const raw = doc?.getElementById('RENDER_DATA')?.innerHTML;
  if (!raw) return null;
  const attempts = [String(raw)];
  try {
    const decoded = decodeURIComponent(String(raw));
    if (decoded !== raw) attempts.push(decoded);
  } catch {
    // ignore
  }
  for (const candidate of attempts) {
    try {
      return JSON.parse(candidate);
    } catch {
      // try next candidate
    }
  }
  return null;
}

export function getRenderVideoDetail(doc = document) {
  const renderData = getRenderData(doc);
  return renderData?.app?.videoDetail || null;
}

function getRouterData(doc = document) {
  try {
    const scripts = doc.querySelectorAll('script');
    for (const script of scripts) {
      const text = script.textContent || '';
      if (!text.includes('window._ROUTER_DATA')) continue;
      const match = text.match(/window\._ROUTER_DATA\s*=\s*(\{.+?\});/s);
      if (!match) continue;
      return JSON.parse(match[1]);
    }
  } catch {
    // ignore
  }
  return null;
}

export function getRouterVideoData(doc = document) {
  const routerData = getRouterData(doc);
  const loaderData = routerData?.loaderData?.['video/:id']?.videoInfoRes;
  return loaderData?.item || null;
}

export function getRenderVideoId() {
  const videoDetail = getRenderVideoDetail(document);
  return String(videoDetail?.awemeId || videoDetail?.aweme_id || '');
}

function normalizeRenderUrl(value, depth = 0) {
  if (!value || depth > 4) return '';
  if (typeof value === 'string') return toAbsUrl(value);
  if (Array.isArray(value)) {
    for (const item of value) {
      const url = normalizeRenderUrl(item, depth + 1);
      if (url) return url;
    }
    return '';
  }
  if (typeof value === 'object') {
    const directKeys = ['url', 'uri', 'src', 'playApi', 'playAddr', 'downloadAddr', 'masterUrl', 'playUrl'];
    for (const key of directKeys) {
      const url = normalizeRenderUrl(value[key], depth + 1);
      if (url) return url;
    }

    const listKeys = ['urlList', 'url_list', 'urls', 'originUrlList', 'origin_url_list'];
    for (const key of listKeys) {
      const list = value[key];
      if (!Array.isArray(list)) continue;
      const url = normalizeRenderUrl(list, depth + 1);
      if (url) return url;
    }

    const nestedKeys = ['data', 'video', 'playAddr', 'downloadAddr'];
    for (const key of nestedKeys) {
      const url = normalizeRenderUrl(value[key], depth + 1);
      if (url) return url;
    }
  }
  return '';
}

const REGION_KEYWORDS = [
  '北京', '上海', '天津', '重庆',
  '河北', '山西', '辽宁', '吉林', '黑龙江',
  '江苏', '浙江', '安徽', '福建', '江西', '山东',
  '河南', '湖北', '湖南', '广东', '海南',
  '四川', '贵州', '云南', '陕西', '甘肃', '青海',
  '台湾', '内蒙古', '广西', '西藏', '宁夏', '新疆', '香港', '澳门',
  '美国', '日本', '韩国', '英国', '法国', '德国', '加拿大', '澳大利亚', '新加坡', '马来西亚', '泰国',
];

export function normalizeIpLocation(raw = '') {
  let text = String(raw || '')
    .replace(/^\s*IP属地[:：]?\s*/i, '')
    .replace(/\s+/g, '')
    .trim();
  if (!text) return '';
  text = text.split(/[，。；;,.]/)[0] || '';
  text = text.replace(/[^\u4e00-\u9fa5·]/g, '');
  if (!text) return '';
  text = text.replace(/(男|女)$/g, '');
  if (/^(男|女|未知)$/.test(text)) return '';
  if (/(简介|分享|作品|粉丝|关注|获赞|抖音号|认证|私信|收藏|订阅|更新|合集|识别画面)/.test(text)) return '';
  const duplicatedPrefix = text.match(/^(.{2,4})\1(·.*)?$/);
  if (duplicatedPrefix) {
    text = `${duplicatedPrefix[1]}${duplicatedPrefix[2] || ''}`;
  }
  if (text.length < 2 || text.length > 14) return '';
  return text;
}

function isLikelyRegion(text = '') {
  const value = String(text || '').trim();
  if (!value) return false;
  if (/(省|市|区|县|州|盟|旗|特别行政区)$/.test(value)) return true;
  return REGION_KEYWORDS.some((k) => value === k || value.startsWith(k));
}

function isTimeLikeText(text = '') {
  return /(刚刚|昨天|前天|\d+\s*(秒|分钟|小时|天|周|月|年)前|\d{1,2}月\d{1,2}日|\d{1,2}:\d{2}|\d{4}-\d{1,2}-\d{1,2})/.test(String(text || ''));
}

export function parseLocationFromInfoText(text = '') {
  const raw = String(text || '').trim();
  if (!raw) return '';
  if (!/IP属地/.test(raw) && raw.length > 80) return '';
  const parts = raw.split(/[·|｜]/).map((part) => part.trim()).filter(Boolean);
  for (let i = parts.length - 1; i >= 0; i -= 1) {
    const part = parts[i];
    if (isTimeLikeText(part)) continue;
    if (part.startsWith('@')) continue;
    if (/(展开|收起|发布时间|举报|点赞|评论|收藏|分享|第\d+集|#)/.test(part)) continue;
    const normalized = normalizeIpLocation(part);
    if (normalized && isLikelyRegion(normalized)) return normalized;
  }
  return '';
}

export function mapRenderVideoToCache(videoDetail, sourceUrl = 'render_data') {
  const awemeId = String(videoDetail?.awemeId || videoDetail?.aweme_id || '').trim();
  if (!awemeId) return null;
  const playApi = normalizeRenderUrl(videoDetail?.video?.playApi);
  const playAddr = normalizeRenderUrl(videoDetail?.video?.playAddr);
  const downloadAddr = normalizeRenderUrl(videoDetail?.video?.downloadAddr);
  const cover = normalizeRenderUrl(videoDetail?.video?.cover)
    || normalizeRenderUrl(videoDetail?.video?.originCover)
    || normalizeRenderUrl(videoDetail?.video?.dynamicCover);
  return upsertApiVideoData(awemeId, {
    videoPlayUrl: playApi || playAddr || '',
    videoDownloadUrl: downloadAddr || playApi || playAddr || '',
    playCount: Number(videoDetail?.stats?.playCount || videoDetail?.stats?.play_count || 0),
    ipLocation: normalizeIpLocation(videoDetail?.authorInfo?.ipLocation || videoDetail?.ipLocation || videoDetail?.ipLabel || ''),
    releaseDate: Number(videoDetail?.createTime || 0) > 0 ? Number(videoDetail.createTime) * 1000 : 0,
    duration: Number(videoDetail?.video?.duration || 0),
    desc: videoDetail?.desc || '',
    hashtags: extractHashtags(videoDetail?.desc || ''),
    authorName: videoDetail?.authorInfo?.nickname || '',
    authorId: videoDetail?.authorInfo?.uid || '',
    authorSecUid: videoDetail?.authorInfo?.secUid || '',
    authorAvatar: normalizeRenderUrl(videoDetail?.authorInfo?.avatarUri)
      || normalizeRenderUrl(videoDetail?.authorInfo?.avatarThumb),
    coverImg: cover,
    statsLikes: Number(videoDetail?.stats?.diggCount || 0),
    statsComments: Number(videoDetail?.stats?.commentCount || 0),
    statsCollects: Number(videoDetail?.stats?.collectCount || 0),
    statsShares: Number(videoDetail?.stats?.shareCount || 0),
    authorFans: Number(videoDetail?.authorInfo?.followerCount || 0),
    authorInteractions: Number(videoDetail?.authorInfo?.totalFavorited || 0),
    fetchedAt: Date.now(),
  }, { sourceUrl });
}

export function mapAwemeDetailToApiData(aweme, sourceUrl = '') {
  const id = String(aweme?.aweme_id || '').trim();
  if (!id) return null;
  return upsertApiVideoData(id, {
    videoPlayUrl: aweme.video?.play_addr?.url_list?.[0] || '',
    videoDownloadUrl: aweme.video?.download_addr?.url_list?.[0] || aweme.video?.play_addr?.url_list?.[0] || '',
    playCount: Number(aweme.statistics?.play_count || 0),
    ipLocation: normalizeIpLocation(aweme.ip_label || aweme.region || ''),
    releaseDate: aweme.create_time ? aweme.create_time * 1000 : 0,
    duration: aweme.video?.duration || 0,
    desc: aweme.desc || '',
    hashtags: extractHashtags(aweme.desc || ''),
    authorName: aweme.author?.nickname || '',
    authorId: aweme.author?.uid || '',
    authorSecUid: aweme.author?.sec_uid || '',
    authorAvatar: aweme.author?.avatar_thumb?.url_list?.[0] || '',
    statsLikes: Number(aweme.statistics?.digg_count || 0),
    statsComments: Number(aweme.statistics?.comment_count || 0),
    statsCollects: Number(aweme.statistics?.collect_count || 0),
    statsShares: Number(aweme.statistics?.share_count || 0),
    authorFans: Number(aweme.author?.follower_count || 0),
    authorInteractions: Number(aweme.author?.total_favorited || 0),
    coverImg: aweme.video?.cover?.url_list?.[0] || aweme.video?.dynamic_cover?.url_list?.[0] || '',
    fetchedAt: Date.now(),
  }, { sourceUrl });
}

export function mapRouterVideoToCache(routerVideo, sourceUrl = '_router_data') {
  const id = String(routerVideo?.aweme_id || routerVideo?.awemeId || '').trim();
  if (!id) return null;

  const ipLocation = normalizeIpLocation(
    routerVideo.author?.ip_location
    || routerVideo.author?.ipLocation
    || routerVideo.ip_label
    || routerVideo.ipLabel
    || ''
  );

  const prev = getApiVideoData(id) || {};
  const routerPlayUrl = normalizeRenderUrl(routerVideo.video?.playAddr || routerVideo.video?.play_addr);
  const routerDownloadUrl = normalizeRenderUrl(routerVideo.video?.downloadAddr || routerVideo.video?.download_addr) || routerPlayUrl;
  const videoPlayUrl = toAbsUrl(prev.videoPlayUrl || '') || routerPlayUrl;
  const videoDownloadUrl = toAbsUrl(prev.videoDownloadUrl || '') || routerDownloadUrl || videoPlayUrl;

  return upsertApiVideoData(id, {
    ...prev,
    videoPlayUrl,
    videoDownloadUrl,
    playCount: Number(routerVideo.stats?.playCount || routerVideo.stats?.play_count || routerVideo.statistics?.play_count || 0),
    ipLocation,
    releaseDate: Number(routerVideo.createTime || routerVideo.create_time || 0) > 0
      ? Number(routerVideo.createTime || routerVideo.create_time) * 1000
      : 0,
    duration: Number(routerVideo.video?.duration || 0),
    desc: routerVideo.desc || '',
    hashtags: extractHashtags(routerVideo.desc || ''),
    authorName: routerVideo.author?.nickname || routerVideo.authorInfo?.nickname || '',
    authorId: routerVideo.author?.uid || routerVideo.authorInfo?.uid || '',
    authorSecUid: routerVideo.author?.sec_uid || routerVideo.author?.secUid || routerVideo.authorInfo?.secUid || '',
    authorAvatar: normalizeRenderUrl(routerVideo.author?.avatar_thumb || routerVideo.author?.avatarThumb || routerVideo.authorInfo?.avatarThumb),
    coverImg: normalizeRenderUrl(routerVideo.video?.cover || routerVideo.video?.originCover || routerVideo.video?.dynamicCover),
    statsLikes: Number(routerVideo.stats?.diggCount || routerVideo.stats?.digg_count || routerVideo.statistics?.digg_count || 0),
    statsComments: Number(routerVideo.stats?.commentCount || routerVideo.stats?.comment_count || routerVideo.statistics?.comment_count || 0),
    statsCollects: Number(routerVideo.stats?.collectCount || routerVideo.stats?.collect_count || routerVideo.statistics?.collect_count || 0),
    statsShares: Number(routerVideo.stats?.shareCount || routerVideo.stats?.share_count || routerVideo.statistics?.share_count || 0),
    authorFans: Number(routerVideo.author?.follower_count || routerVideo.authorInfo?.followerCount || 0),
    authorInteractions: Number(routerVideo.author?.total_favorited || routerVideo.authorInfo?.totalFavorited || 0),
    fetchedAt: Date.now(),
  }, { sourceUrl });
}

export async function fetchDetailApiData(videoId, { suppressErrors = true } = {}) {
  if (!videoId) return null;
  const encoded = encodeURIComponent(videoId);
  const apiUrls = [
    `/aweme/v1/web/aweme/detail/?aweme_id=${encoded}&aid=6383`,
    `https://www.douyin.com/aweme/v1/web/aweme/detail/?aweme_id=${encoded}&aid=6383`,
  ];
  try {
    const json = await fetchDouyinJson(apiUrls);
    const mapped = mapAwemeDetailToApiData(json?.aweme_detail, apiUrls[0]);
    if (mapped) return mapped;
  } catch (error) {
    if (!suppressErrors) throw error;
  }
  return null;
}

export function hasUsableApiVideo(data) {
  if (!data) return false;
  return Boolean(String(data.videoDownloadUrl || data.videoPlayUrl || '').trim());
}

export async function resolveApiVideoData(videoId, options = {}) {
  const cached = getApiVideoData(videoId);
  if (hasUsableApiVideo(cached)) return cached;
  const fetched = await fetchDetailApiData(videoId, options);
  return fetched || cached || null;
}
