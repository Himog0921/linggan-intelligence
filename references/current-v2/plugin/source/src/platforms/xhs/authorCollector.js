import { getByInject, parseCount } from '../../shared/utils.js';
import { authorStore } from '../../db/authorStore.js';
import { createCollectorEvidence, createCollectorQualityMeta, joinRawDomText } from '../../shared/collectorMetadata.js';
import { MONITOR_RECORD_MODE } from '../../workbench/protocol/schema.js';
import { withMonitorRecordMeta } from '../../workbench/runtime/monitorTask.js';

/**
 * 采集博主信息（小红书）
 * 技术路径：
 *   - 计数（关注/粉丝/赞藏）和标签 → 注入 user.js 从 __INITIAL_STATE__ 读取（DOM 无可靠选择器）
 *   - 名称/小红书号/IP/简介/头像 → 直接 DOM（已验证: .user-name/.user-redId/.user-IP/.user-desc/.user-image）
 */
export async function collectAuthor(options = {}) {
  // 从 URL 提取用户 ID
  const userIdMatch = window.location.href.match(/profile\/([a-z0-9]+)/i);
  if (!userIdMatch) {
    throw new Error('无法从 URL 提取用户 ID，请确认当前页面是博主主页');
  }

  // 1. 从 __INITIAL_STATE__ 读取结构化数据（关注数/粉丝数/赞藏数/标签）
  let pageData = {};
  let injectFailed = false;
  try {
    const injected = await getByInject(window, 'user');
    pageData = injected?.userPageData || {};
  } catch (e) {
    injectFailed = true;
    console.warn('[authorCollector] 注入脚本失败，将只采集 DOM 数据', e.message);
  }

  // 解析 interactions: 兼容多层级结构
  const interactions = pageData.interactions || pageData.userPageData?.interactions || [];
  const countMap = {};
  interactions.forEach(item => {
    const type = item.type || item.name || item.label;
    const count = item.count ?? item.countText ?? item.displayText ?? item.value ?? item.num ?? 0;
    if (type) countMap[type] = parseCount(count);
  });

  // 解析 tags
  const tags = pageData.tags || pageData.userPageData?.tags || [];
  const keywords = tags.map(t => t.name).filter(Boolean);

  // basicInfo / extraInfo 兼容多路径
  const basicInfo = pageData.basicInfo || pageData.userPageData?.basicInfo || {};
  const extraInfo = pageData.extraInfo || pageData.userPageData?.extraInfo || {};
  const userInfo = pageData.userInfo || pageData.userPageData?.userInfo || {};

  // 2. DOM 选择器 + 结构化数据多重兜底
  const name = getText('.user-name') || basicInfo.nickname || userInfo.nickname || '';
  const redId = getText('.user-redId', '小红书号：') || basicInfo.redId || userInfo.redId || '';
  let location = getText('.user-IP', 'IP属地：') || basicInfo.ipLocation || userInfo.ipLocation || '';
  let description = getText('.user-desc') || basicInfo.desc || userInfo.desc || '';
  let avatar = document.querySelector('.user-image')?.getAttribute('src')
    || basicInfo.imageb
    || basicInfo.images
    || userInfo.imageb
    || userInfo.images
    || userInfo.avatar
    || '';
  // 若 avatar 仍是对象/数组，尝试提取 url
  if (avatar && typeof avatar === 'object') {
    avatar = avatar.url || avatar.src || avatar[0]?.url || avatar[0]?.src || '';
  }

  let ipLocation = basicInfo.ipLocation || userInfo.ipLocation || location;

  // 数据校验：IP 属地应该是短文本（如"广东"），若长度超过 20 或包含明显简介特征，则视为被污染
  if (ipLocation && (ipLocation.length > 20 || /加入|咨询|合作|微信|v:|私信/.test(ipLocation))) {
    ipLocation = '';
  }
  // 若简介和 IP 相同且简介较长，清空 IP
  if (ipLocation && description && ipLocation === description && description.length > 10) {
    ipLocation = '';
  }
  // 若 location 字段被污染，同步修正
  if (location && description && location === description && description.length > 10) {
    location = '';
  }

  const gender = normalizeGender(basicInfo.gender ?? userInfo.gender);
  const accountStatus = String(extraInfo.blockType || '').trim();
  const followedByMe = normalizeFollowStatus(extraInfo.fstatus);
  const hasStructuredPageData = Object.keys(pageData || {}).length > 0;

  const collectedAt = Date.now();
  const author = withMonitorRecordMeta({
    userId: userIdMatch[1],
    authorId: userIdMatch[1],
    authorEntityId: `xhs_${userIdMatch[1]}`,
    platformAuthorId: userIdMatch[1],
    platform: 'xhs',
    profileUrl: window.location.href,
    redId,
    handle: redId,
    name,
    avatar,
    location,
    ipLocation,
    gender,
    accountStatus,
    followedByMe,
    description,
    keywords,
    follows: pickInteractionCount(countMap, ['follows', 'following', '关注']),
    fans: pickInteractionCount(countMap, ['fans', 'followers', 'follower', '粉丝', '粉絲', '关注者']),
    interactions: pickInteractionCount(countMap, [
      'interaction',
      'interactions',
      'likesAndCollects',
      'likedAndCollected',
      '获赞与收藏',
      '获赞和收藏',
      '获赞收藏',
      '赞与收藏',
      '赞藏',
      '获赞',
    ]),
    collectedAt,
    createdAt: collectedAt,
    collectionRunId: String(options.collectionRunId || '').trim(),
    syncStatus: 'pending',
    lastSyncAt: null,
    ...createCollectorQualityMeta({
      dataQuality: injectFailed || !hasStructuredPageData ? 'degraded' : 'full',
      qualityReason: injectFailed || !hasStructuredPageData ? 'inject_failed_dom_only' : '',
      sourceTier: injectFailed || !hasStructuredPageData ? 'dom' : 'mixed',
    }),
    ...createCollectorEvidence({
      rawPayload: {
        pageData,
        basicInfo,
        extraInfo,
      },
      rawDomText: joinRawDomText([
        name,
        redId,
        location,
        description,
        keywords.join(' '),
      ]),
      rawUrl: window.location.href,
      rawSource: 'xhs.userPageData+dom',
    }),
  }, options.monitorMeta, MONITOR_RECORD_MODE.AUTHOR_PROFILE);

  await authorStore.upsert(author);
  return author;
}

function normalizeGender(rawGender) {
  const value = Number(rawGender);
  if (value === 1) return 1;
  if (value === 0 || value === 2) return value;
  return 0;
}

function pickInteractionCount(map = {}, keys = []) {
  for (const key of keys) {
    if (typeof map[key] === 'number' && map[key] > 0) return map[key];
  }
  const normalizedKeys = keys.map((key) => String(key).toLowerCase());
  for (const [rawKey, value] of Object.entries(map)) {
    const normalized = String(rawKey || '').toLowerCase();
    if (normalizedKeys.some((key) => normalized.includes(key)) && Number(value) > 0) {
      return Number(value);
    }
  }
  return 0;
}

function normalizeFollowStatus(rawStatus) {
  const status = String(rawStatus || '').toLowerCase().trim();
  if (!status || status === 'none') return false;
  return true;
}

function getText(selector, prefix = '') {
  const el = document.querySelector(selector);
  if (!el) return '';
  let text = el.textContent?.trim() || '';
  if (prefix && text.startsWith(prefix)) {
    text = text.slice(prefix.length).trim();
  }
  return text;
}
