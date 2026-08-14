/**
 * 抖音博主采集器
 *
 * 采集方式：DOM 解析（已验证选择器，2026-03-24）
 * 覆盖场景：用户主页（/user/{userId}）
 * 存储表：authors（platform: 'douyin'）
 */

import { authorStore } from '../../db/authorStore.js';
import { createCollectorEvidence, createCollectorQualityMeta, joinRawDomText } from '../../shared/collectorMetadata.js';
import { getRenderData } from './videoApiData.js';
import { MONITOR_RECORD_MODE } from '../../workbench/protocol/schema.js';
import { withMonitorRecordMeta } from '../../workbench/runtime/monitorTask.js';
import { fetchDouyinWithTimeout } from './fetchWithTimeout.js';

// ========== 已验证选择器（2026-03-24）==========
const SEL = {
  // 用户详情容器（包含昵称+徽章文字）
  userDetail:  '[data-e2e="user-detail"]',
  signature:   '[data-e2e="user-signature"]',
  avatar:      'img[data-e2e="user-avatar"]',
  badge:       '[data-e2e="badge-role-name"]',
  followCount: '[data-e2e="user-info-follow"]',
  fansCount:   '[data-e2e="user-info-fans"]',
  likeCount:   '[data-e2e="user-info-like"]',
  postCount:   '[data-e2e="user-tab-count"]',
  userIp:      '[data-e2e="user-ip"]',
};

/**
 * 将抖音显示数字解析为整数
 */
function parseDyNumber(text = '') {
  const s = String(text).trim().replace(/,/g, '');
  // 有时文字包含前缀，如 "粉丝167.8万"——先提取数字部分
  const numPart = s.replace(/^[^0-9.]*/, '');
  if (s.includes('万')) return Math.round(parseFloat(numPart) * 10000);
  if (s.includes('亿')) return Math.round(parseFloat(numPart) * 100000000);
  return parseInt(numPart.replace(/[^0-9]/g, ''), 10) || 0;
}

function getText(selector) {
  return document.querySelector(selector)?.textContent?.trim() || '';
}

function getSrc(selector) {
  return document.querySelector(selector)?.getAttribute('src') || '';
}

function sanitizeNickname(raw = '') {
  let text = String(raw || '').replace(/\s+/g, ' ').trim();
  if (!text) return '';

  const markerPatterns = [
    /认证徽章/,
    /20\d{2}年\d{1,2}月精选作者/,
    /精选作者/,
    /优质作者/,
    /关注\d/,
    /粉丝/,
    /获赞/,
    /抖音号[:：]/,
    /IP属地[:：]/,
  ];
  let cutIndex = -1;
  for (const pattern of markerPatterns) {
    const idx = text.search(pattern);
    if (idx >= 0 && (cutIndex < 0 || idx < cutIndex)) {
      cutIndex = idx;
    }
  }
  if (cutIndex > 0) {
    text = text.slice(0, cutIndex).trim();
  }
  return text.replace(/^@/, '').trim();
}

function getRenderUserInfo() {
  const renderData = getRenderData(document);
  return renderData?.app?.user?.info
    || window.__INITIAL_STATE__?.user?.userInfo?._rawValue
    || null;
}

function pickNumber(...values) {
  for (const raw of values) {
    const value = Number(raw);
    if (Number.isFinite(value) && value > 0) return value;
  }
  for (const raw of values) {
    const value = Number(raw);
    if (Number.isFinite(value) && value >= 0) return value;
  }
  return 0;
}

function normalizeIpLocation(raw = '') {
  let text = String(raw || '')
    .replace(/\s+/g, '')
    .replace(/^\s*IP属地[:：]?\s*/i, '')
    .trim();
  if (!text) return '';

  // 过滤时间、性别、简介等非属地内容
  text = text.split(/[，。；;,.]/)[0] || '';
  text = text.replace(/[^\u4e00-\u9fa5·]/g, '');
  text = text.replace(/[·丨｜|]+$/g, '');
  text = text.replace(/(男|女)$/g, '');
  if (/^(男|女|未知)$/.test(text)) return '';
  if (/(简介|分享|作品|粉丝|关注|获赞|抖音号|认证|私信|收藏|订阅|今年|昨天|今天|刚刚)/.test(text)) return '';

  // 处理“上海上海·黄浦”这类重复前缀
  const duplicatedPrefix = text.match(/^(.{2,4})\1(·.*)?$/);
  if (duplicatedPrefix) {
    text = `${duplicatedPrefix[1]}${duplicatedPrefix[2] || ''}`;
  }

  if (text.length < 2 || text.length > 14) return '';
  return text;
}

function parseIpFromMarkedText(raw = '') {
  const text = String(raw || '');
  if (!/IP属地/i.test(text)) return '';
  const markerMatch = text.match(/IP属地[:：]?\s*([\u4e00-\u9fa5·]{2,20})(?=男|女|分享|主页|关注|粉丝|获赞|抖音号|\s|$|，|。|；)/i);
  if (markerMatch?.[1]) {
    const value = normalizeIpLocation(markerMatch[1]);
    if (value) return value;
  }
  const normalized = normalizeIpLocation(text.replace(/.*?IP属地[:：]?/i, ''));
  if (normalized) return normalized;
  return '';
}

function extractIpFromDomByMarker() {
  const userDetailText = getText(SEL.userDetail);
  const fromUserDetail = parseIpFromMarkedText(userDetailText);
  if (fromUserDetail) return fromUserDetail;

  const directText = getText(SEL.userIp);
  const direct = parseIpFromMarkedText(directText);
  if (direct) return direct;

  // 只扫描“文本节点”中的 IP 标记，不再读取大容器 textContent，避免把简介/性别拼进来
  const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
  let node = walker.nextNode();
  while (node) {
    const parentTag = String(node.parentElement?.tagName || '').toLowerCase();
    if (['script', 'style', 'noscript', 'template'].includes(parentTag)) {
      node = walker.nextNode();
      continue;
    }
    const text = String(node.textContent || '').trim();
    if (!text || text.length > 120) {
      node = walker.nextNode();
      continue;
    }
    if (/[{}<>]/.test(text) || /self\.__|function\s*\(|webpack|sourceMappingURL/i.test(text)) {
      node = walker.nextNode();
      continue;
    }
    if (text && /IP属地/i.test(text)) {
      const parsed = parseIpFromMarkedText(text);
      if (parsed) return parsed;
    }
    node = walker.nextNode();
  }

  return '';
}

function decodeUserId(raw = '') {
  try {
    return decodeURIComponent(raw);
  } catch {
    return raw;
  }
}

function normalizeAccountId(raw = '') {
  return String(raw || '').trim().replace(/\s+/g, '');
}

function isValidDouyinId(raw = '') {
  const value = normalizeAccountId(raw);
  if (!value) return false;
  if (/^0+$/.test(value)) return false;
  return /^[A-Za-z0-9._-]{2,40}$/.test(value);
}

function parseDouyinIdFromText(raw = '') {
  const text = String(raw || '');
  const match = text.match(/抖音号[:：]\s*([A-Za-z0-9._-]{2,40})(?=\s|$|IP属地|关注|粉丝|获赞|私信|主页|分享|作品|合集)/);
  if (!match?.[1]) return '';
  const value = normalizeAccountId(match[1]);
  return isValidDouyinId(value) ? value : '';
}

function extractDouyinIdFromDom() {
  const candidates = [
    getText(SEL.userDetail),
    getText(SEL.signature),
  ];
  for (const item of candidates) {
    const parsed = parseDouyinIdFromText(item);
    if (parsed) return parsed;
  }
  return '';
}

function pickProfileMetaFromApiPayload(payload = null) {
  if (!payload || typeof payload !== 'object') {
    return { ipLocation: '', douyinId: '' };
  }
  const queue = [payload];
  const visited = new Set();
  const locationLikeKeys = [
    'ip_location',
    'ipLocation',
    'ip_label',
    'ipLabel',
    'location',
    'region',
    'province',
    'city',
    'district',
  ];
  const douyinIdLikeKeys = [
    'unique_id',
    'uniqueId',
    'short_id',
    'shortId',
    'douyin_id',
    'douyinId',
  ];

  let ipLocation = '';
  const douyinIdCandidates = [];

  const accountIdPriority = (lowerKey) => {
    if (lowerKey.includes('unique_id') || lowerKey.includes('uniqueid')) return 100;
    if (lowerKey.includes('douyin_id') || lowerKey.includes('douyinid')) return 90;
    if (lowerKey.includes('short_id') || lowerKey.includes('shortid')) return 60;
    return 0;
  };

  while (queue.length > 0) {
    const obj = queue.shift();
    if (!obj || typeof obj !== 'object') continue;
    if (visited.has(obj)) continue;
    visited.add(obj);

    for (const [key, value] of Object.entries(obj)) {
      if (value && typeof value === 'object') {
        queue.push(value);
        continue;
      }
      if (value == null) continue;

      const lowerKey = key.toLowerCase();
      if (!ipLocation && locationLikeKeys.some((k) => lowerKey.includes(String(k).toLowerCase()))) {
        const parsedLocation = normalizeIpLocation(String(value));
        if (parsedLocation) ipLocation = parsedLocation;
      }
      if (douyinIdLikeKeys.some((k) => lowerKey.includes(String(k).toLowerCase()))) {
        const parsedDouyinId = normalizeAccountId(String(value));
        if (isValidDouyinId(parsedDouyinId)) {
          douyinIdCandidates.push({
            id: parsedDouyinId,
            priority: accountIdPriority(lowerKey),
          });
        }
      }
    }
  }
  douyinIdCandidates.sort((a, b) => b.priority - a.priority);
  const douyinId = douyinIdCandidates[0]?.id || '';
  return { ipLocation, douyinId };
}

async function fetchProfileMetaFromApi(userId) {
  if (!userId) return { ipLocation: '', douyinId: '' };
  const secUserId = encodeURIComponent(userId);
  const apiUrls = [
    `/aweme/v1/web/user/profile/other/?sec_user_id=${secUserId}&aid=6383`,
    `/aweme/v1/web/user/profile/other/?sec_user_id=${secUserId}`,
    `/aweme/v1/web/user/profile/?sec_user_id=${secUserId}&aid=6383`,
    `/aweme/v1/web/user/profile/?sec_user_id=${secUserId}`,
  ];

  let best = { ipLocation: '', douyinId: '' };
  for (const apiUrl of apiUrls) {
    try {
      const resp = await fetchDouyinWithTimeout(apiUrl, { credentials: 'include' });
      if (!resp.ok) continue;
      const json = await resp.json();
      const parsed = pickProfileMetaFromApiPayload(json);
      if (parsed.ipLocation && !best.ipLocation) best.ipLocation = parsed.ipLocation;
      if (parsed.douyinId && !best.douyinId) best.douyinId = parsed.douyinId;
      if (best.ipLocation && best.douyinId) return best;
    } catch {
      // 尝试下一个 API 候选
    }
  }
  return best;
}

/**
 * 从 [data-e2e="user-detail"] 提取昵称
 * 问题：该元素的 textContent 包含"昵称 + 认证徽章 + 认证描述 + 统计数字"
 * 策略：取"认证徽章"之前的文字；若没有徽章，取"关注"之前的文字
 */
function extractNickname() {
  const el = document.querySelector(SEL.userDetail);
  if (!el) return '';

  // 优先从第一个直接文本节点获取（最干净）
  for (const node of el.childNodes) {
    if (node.nodeType === Node.TEXT_NODE) {
      const t = node.textContent.trim();
      if (t) return sanitizeNickname(t);
    }
    if (node.nodeType === Node.ELEMENT_NODE) {
      // 跳过徽章和统计元素
      const tag = (node.getAttribute('data-e2e') || '');
      if (tag.includes('badge') || tag.includes('user-info')) continue;
      // 取第一个有意义的元素文本（通常就是昵称容器）
      const t = node.textContent.replace(/认证徽章[\s\S]*/g, '').trim();
      if (t && !t.includes('关注') && !t.includes('粉丝')) return sanitizeNickname(t);
    }
  }

  // 回退：正则截断法
  const full = el.textContent || '';
  return sanitizeNickname(full
    .replace(/认证徽章[\s\S]*/g, '')
    .replace(/关注\d+[\s\S]*/g, ''));
}

/**
 * 从 URL 提取博主 userId
 */
function extractUserId() {
  const m = window.location.pathname.match(/\/user\/([^/?#]+)/)
         || window.location.pathname.match(/^\/@([^/?#]+)/);
  return m ? decodeUserId(m[1]) : '';
}

/**
 * 从博主主页 DOM 采集作者信息
 * @returns {{ ok: boolean, data?: object, error?: string }}
 */
export async function collectDouyinAuthor(options = {}) {
  // 等待关键元素渲染（最多 3 秒）
  await waitForElement(SEL.fansCount, 3000).catch(() => {});
  const renderUser = getRenderUserInfo();
  const urlUserId = extractUserId();
  const renderSecUid = decodeUserId(renderUser?.secUid || renderUser?.sec_uid || '');
  const userId = urlUserId || renderSecUid;
  if (!userId) {
    return { ok: false, error: '无法从当前 URL 提取博主 ID，请确认已打开抖音博主主页' };
  }

  const isRenderMatchedUser = Boolean(renderUser && renderSecUid && renderSecUid === userId);
  const safeRenderUser = isRenderMatchedUser ? renderUser : null;

  const nickname  = sanitizeNickname(String(safeRenderUser?.nickname || '').trim()) || extractNickname();
  const signature = String(safeRenderUser?.desc || '').trim() || getText(SEL.signature);
  const avatar    = String(safeRenderUser?.avatar300Url || safeRenderUser?.avatarUrl || '').trim() || getSrc(SEL.avatar);
  const badge     = getText(SEL.badge);

  // 各数字字段包含前缀文字（"粉丝167.8万"），parseDyNumber 会处理
  const follows = pickNumber(safeRenderUser?.followingCount, parseDyNumber(getText(SEL.followCount)));
  const fans = pickNumber(safeRenderUser?.mplatformFollowersCount, safeRenderUser?.followerCount, parseDyNumber(getText(SEL.fansCount)));
  const likes = pickNumber(safeRenderUser?.totalFavorited, parseDyNumber(getText(SEL.likeCount)));
  const postCount = pickNumber(safeRenderUser?.awemeCount, safeRenderUser?.aweme_count, parseDyNumber(getText(SEL.postCount)));
  const douyinIdFromRender = isValidDouyinId(safeRenderUser?.uniqueId || safeRenderUser?.unique_id || '')
    ? normalizeAccountId(safeRenderUser?.uniqueId || safeRenderUser?.unique_id || '')
    : '';
  const ipLocationFromRender = normalizeIpLocation(safeRenderUser?.ipLocation || safeRenderUser?.ip_location || '');

  // 新策略：结构化状态优先（render）+ API + DOM 标记兜底
  const profileMetaFromApi = await fetchProfileMetaFromApi(userId);
  const douyinIdFromApi = profileMetaFromApi.douyinId || '';
  const douyinIdFromDom = extractDouyinIdFromDom();
  const isAtHandleRoute = /^\/@/.test(window.location.pathname || '');
  let douyinId = '';
  let douyinIdSource = 'none';
  if (isValidDouyinId(douyinIdFromApi)) {
    douyinId = douyinIdFromApi;
    douyinIdSource = 'api';
  } else if (isValidDouyinId(douyinIdFromDom)) {
    douyinId = douyinIdFromDom;
    douyinIdSource = 'dom';
  } else if (isValidDouyinId(douyinIdFromRender)) {
    douyinId = douyinIdFromRender;
    douyinIdSource = 'render';
  } else if (isAtHandleRoute && isValidDouyinId(userId)) {
    douyinId = userId;
    douyinIdSource = 'fallback-handle';
  }

  const ipLocationFromApi = profileMetaFromApi.ipLocation || '';
  const ipLocationFromDom = extractIpFromDomByMarker();
  const ipLocation = ipLocationFromApi || ipLocationFromRender || ipLocationFromDom || '';
  const ipLocationSource = ipLocationFromApi
    ? 'api'
    : (ipLocationFromRender ? 'render' : (ipLocationFromDom ? 'dom-marker' : 'none'));
  const qualityReason = !isRenderMatchedUser && renderUser
    ? 'render_user_mismatch'
    : (douyinIdSource === 'fallback-handle'
      ? 'handle_route_fallback'
      : (ipLocationSource === 'dom-marker' ? 'dom_marker_ip_fallback' : ''));
  const dataQuality = qualityReason ? 'degraded' : 'full';

  const collectedAt = Date.now();
  const record = withMonitorRecordMeta({
    userId:     `dy_${userId}`,
    authorEntityId: `douyin_${userId}`,
    platformAuthorId: userId,
    platform:   'douyin',
    name:       nickname,
    profileUrl: window.location.href,
    redId:      douyinId,
    handle: douyinId,
    douyinId,
    secUserId: userId,
    douyinIdSource,
    fans,
    follows,
    interactions: likes,            // 借用 interactions 存总获赞数
    badge,
    signature,
    avatar,
    postCount,
    ipLocation,
    location: ipLocation,
    ipLocationSource,
    collectedAt,
    createdAt:  collectedAt,
    collectionRunId: String(options.collectionRunId || '').trim(),
    syncStatus: 'local',
    ...createCollectorQualityMeta({
      dataQuality,
      qualityReason,
      sourceTier: 'mixed',
    }),
    ...createCollectorEvidence({
      rawPayload: {
        renderUser: safeRenderUser,
        profileMetaFromApi,
        badge,
        signature,
      },
      rawDomText: joinRawDomText([
        nickname,
        douyinId,
        signature,
        ipLocation,
        badge,
      ]),
      rawUrl: window.location.href,
      rawSource: 'douyin.user-detail+profile-api',
    }),
  }, options.monitorMeta, MONITOR_RECORD_MODE.AUTHOR_PROFILE);

  await authorStore.upsert(record);
  return { ok: true, data: record };
}

// ========== 工具函数 ==========

function waitForElement(selector, timeoutMs = 3000) {
  return new Promise((resolve, reject) => {
    const el = document.querySelector(selector);
    if (el) { resolve(el); return; }
    const t = setTimeout(() => { obs.disconnect(); reject(new Error('timeout')); }, timeoutMs);
    const obs = new MutationObserver(() => {
      const found = document.querySelector(selector);
      if (found) { clearTimeout(t); obs.disconnect(); resolve(found); }
    });
    obs.observe(document.body, { childList: true, subtree: true });
  });
}
