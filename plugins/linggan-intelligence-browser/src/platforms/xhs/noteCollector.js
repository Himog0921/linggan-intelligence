import {
  getByInject,
  safeUrl,
  extractNoteId,
  parseCount,
  toHighQualityImageUrl,
  pickBestVideoStream,
  getHighQualityImageCandidates,
} from '../../shared/utils.js';
import { isContextValid } from '../../shared/messaging.js';
import { noteStore } from '../../db/noteStore.js';
import { createCollectorEvidence, joinRawDomText } from '../../shared/collectorMetadata.js';
import { withMonitorRecordMeta } from '../../workbench/runtime/monitorTask.js';
import {
  ensureXhsCommentApiBridge,
  fetchXhsJsonViaBridge,
  requestXhsProfileNotesSnapshot,
  requestXhsSearchNotesSnapshot,
} from './commentApi.js';

const XHS_CONTEXT_REFRESH_MESSAGE = '插件刚更新，请刷新当前页面后再点一次，刷新后即可继续。';

function normalizeDiscoverySnapshot(snapshot) {
  if (typeof snapshot === 'number') {
    return {
      visibleCount: snapshot,
      knownNoteIds: null,
    };
  }

  const knownNoteIds = snapshot?.knownNoteIds instanceof Set
    ? snapshot.knownNoteIds
    : null;
  return {
    visibleCount: normalizePositiveInteger(snapshot?.visibleCount, 0),
    knownNoteIds,
  };
}

function hasDiscoveryAdvanced(currentNotes, snapshot) {
  if (snapshot.knownNoteIds && currentNotes.some((note) => note?.noteId && !snapshot.knownNoteIds.has(note.noteId))) {
    return true;
  }
  return currentNotes.length > snapshot.visibleCount;
}

async function waitForDiscoverySettle(containerSelector, previousSnapshot, timeout, isProfileMode) {
  const startedAt = Date.now();
  let stableRounds = 0;
  const snapshot = normalizeDiscoverySnapshot(previousSnapshot);

  while (Date.now() - startedAt < timeout) {
    const currentNotes = discoverNotesFromDOM(containerSelector);
    if (hasDiscoveryAdvanced(currentNotes, snapshot)) {
      stableRounds += 1;
      if (stableRounds >= 2) return;
    } else {
      stableRounds = 0;
    }
    await new Promise((resolve) => setTimeout(resolve, isProfileMode ? 200 : 160));
  }
}

function getXhsImageUrl(image = {}) {
  return image?.urlDefault || image?.url_default || image?.url || '';
}

export function extractXhsLivePhotoStreams(imageList = []) {
  if (!Array.isArray(imageList)) return [];
  return imageList.map((image, index) => {
    const stream = image?.stream || image?.livePhoto?.stream || image?.live_photo?.stream || {};
    const hasLiveFlag = Boolean(
      image?.livePhoto
      || image?.live_photo
      || image?.isLivePhoto
      || image?.is_live_photo
      || (stream && Object.keys(stream).length > 0)
    );
    if (!hasLiveFlag || !stream || Object.keys(stream).length === 0) return null;

    const selection = pickBestVideoStream(stream);
    const candidates = (selection.streams || []).map((item) => item?.url).filter(Boolean);
    if (!selection.url && candidates.length === 0) return null;

    const coverUrl = toHighQualityImageUrl(getXhsImageUrl(image));
    return {
      imageIndex: index + 1,
      url: selection.url || candidates[0] || '',
      candidates,
      streams: selection.streams || [],
      coverUrl,
    };
  }).filter(Boolean);
}

export function selectNoteKey(noteMap = {}, preferredNoteId = '', currentUrl = '') {
  const expectedId = String(preferredNoteId || '').trim();
  if (expectedId && noteMap && noteMap[expectedId]) {
    return expectedId;
  }

  const currentNoteId = extractNoteId(currentUrl);
  const validKeys = Object.keys(noteMap || {}).filter(k => k && k !== 'undefined' && k.length > 10);
  if (currentNoteId && noteMap && noteMap[currentNoteId]) {
    return currentNoteId;
  }
  return validKeys[0] || '';
}

function normalizeNoteData(noteData) {
  let note = Array.isArray(noteData) ? noteData[0] : noteData;
  if (note && note.note && !note.noteId && !note.id) {
    note = note.note;
  }
  return note || null;
}

function firstPresentValue(source = {}, keys = []) {
  if (!source || typeof source !== 'object') return null;
  for (const key of keys) {
    if (source[key] != null && source[key] !== '') return source[key];
  }
  return null;
}

export function parseXhsInteractCount(interactInfo = {}, keys = []) {
  return parseCount(firstPresentValue(interactInfo, keys));
}

export function isCollectedNoteUsable(note = {}, expectedNoteId = '', { requireStats = false } = {}) {
  const normalizedExpectedId = String(expectedNoteId || '').trim();
  const normalizedActualId = String(note?.noteId || note?.id || '').trim();
  if (normalizedExpectedId && normalizedActualId && normalizedExpectedId !== normalizedActualId) {
    return false;
  }

  const hasText = Boolean(String(note?.title || '').trim() || String(note?.desc || '').trim());
  const hasMedia = Boolean(
    (Array.isArray(note?.imageList) && note.imageList.length > 0)
      || note?.video?.media?.stream
      || note?.video?.consumer
  );
  const hasAuthor = Boolean(String(note?.user?.nickname || '').trim() || String(note?.user?.userId || '').trim());

  if (!requireStats) {
    const interactInfo = note?.interactInfo || {};
    const hasStats = Boolean(
      firstPresentValue(interactInfo, ['likedCount', 'likeCount', 'likes'])
      || firstPresentValue(interactInfo, ['commentCount', 'comments'])
      || firstPresentValue(interactInfo, ['shareCount', 'shares'])
      || firstPresentValue(interactInfo, ['collectedCount', 'collectCount', 'collects', 'favoriteCount'])
    );
    return hasText || hasMedia || hasAuthor || hasStats;
  }

  // detail_probe 场景：要求关键互动字段全部到位，避免 AJAX 填充未完成时过早采集
  const interactInfo = note?.interactInfo;
  const hasFullStats = Boolean(
    interactInfo
    && (interactInfo.likedCount != null || interactInfo.likeCount != null)
    && (interactInfo.collectedCount != null || interactInfo.collectCount != null)
    && (interactInfo.commentCount != null || interactInfo.comments != null)
  );
  const hasValidMedia = Boolean(
    (Array.isArray(note?.imageList) && note.imageList.length > 0 && (note.imageList[0]?.url || note.imageList[0]?.urlDefault))
    || note?.video?.media?.stream
    || note?.video?.consumer
  );

  return (hasText || hasValidMedia || hasAuthor) && hasFullStats;
}

export function resolveExpectedNoteFromMap(noteMap = {}, expectedNoteId = '', currentUrl = '') {
  const expectedId = String(expectedNoteId || '').trim();
  const preferredKey = selectNoteKey(noteMap, expectedId, currentUrl);
  const candidateKeys = [];
  if (expectedId) candidateKeys.push(expectedId);
  if (preferredKey && !candidateKeys.includes(preferredKey)) candidateKeys.push(preferredKey);

  const entries = Object.entries(noteMap || {});
  for (const [key, value] of entries) {
    const note = normalizeNoteData(value);
    const actualId = String(note?.noteId || note?.id || '').trim();
    if (expectedId && actualId === expectedId && !candidateKeys.includes(key)) {
      candidateKeys.unshift(key);
    }
  }

  for (const key of candidateKeys) {
    const note = normalizeNoteData(noteMap?.[key]);
    if (!note) continue;
    const actualId = String(note?.noteId || note?.id || '').trim();
    return {
      noteKey: key,
      note,
      actualNoteId: actualId,
      exactMatch: !expectedId || actualId === expectedId,
      usable: isCollectedNoteUsable(note, expectedId),
    };
  }

  return {
    noteKey: '',
    note: null,
    actualNoteId: '',
    exactMatch: false,
    usable: false,
  };
}

/**
 * 提取话题标签
 */
function extractHashtags(text = '') {
  const raw = String(text || '');
  const matches = raw.match(/#\s*[^\s#，。！？,.!?:：;；、]+/g) || [];
  return matches.map(tag => tag.replace(/^#/, '').trim()).filter(Boolean);
}

/**
 * 移除话题标签
 */
function removeHashtags(text = '') {
  return String(text || '').replace(/#\s*[^\s#，。！？,.!?:：;；、]+/g, '').replace(/\s+/g, ' ').trim();
}

function stripLooseHashes(text = '') {
  return String(text || '')
    .replace(/^\s*#{1,6}\s*/gm, '')
    .replace(/(^|\s)(?:#\s*){2,}(?=\s|$)/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
}

function cleanNoteBodyText(text = '') {
  return stripLooseHashes(removeHashtags(text));
}

function toNormalizedTimestamp(raw) {
  if (raw == null || raw === '') return 0;
  if (raw instanceof Date) {
    const ts = raw.getTime();
    return Number.isFinite(ts) && ts > 0 ? ts : 0;
  }
  if (typeof raw === 'number') {
    if (!Number.isFinite(raw) || raw <= 0) return 0;
    return raw < 1000000000000 ? raw * 1000 : raw;
  }
  const text = String(raw).trim();
  if (!text) return 0;
  if (/^\d+$/.test(text)) {
    const value = Number(text);
    if (!Number.isFinite(value) || value <= 0) return 0;
    return value < 1000000000000 ? value * 1000 : value;
  }
  const parsed = Date.parse(text);
  return Number.isFinite(parsed) ? parsed : 0;
}

function buildDateFromParts(baseDate, {
  year,
  month,
  day,
  hours = 0,
  minutes = 0,
  seconds = 0,
} = {}) {
  const candidate = new Date(baseDate);
  candidate.setFullYear(year, month - 1, day);
  candidate.setHours(hours, minutes, seconds, 0);
  return candidate;
}

export function parseXhsPublishedAt(raw, { now = Date.now() } = {}) {
  const directTimestamp = toNormalizedTimestamp(raw);
  if (directTimestamp > 0) return directTimestamp;

  const text = String(raw || '').trim()
    .replace(/^[发發]布于?\s*/i, '')
    .replace(/^[发發]布時間[:：]?\s*/i, '')
    .replace(/^[编編][辑輯]于?\s*/i, '')
    .replace(/\s*IP.*$/i, '')
    .trim();
  if (!text) return 0;

  const nowDate = new Date(now);
  const relativeMatch = text.match(/^(\d+)\s*(秒钟?|分钟|分鐘|小时|小時|天|周|週|个月|個月|月|年)前$/);
  if (relativeMatch) {
    const amount = Number(relativeMatch[1]);
    if (Number.isFinite(amount) && amount >= 0) {
      const unit = relativeMatch[2];
      const multipliers = {
        秒: 1000,
        秒钟: 1000,
        分钟: 60 * 1000,
        分鐘: 60 * 1000,
        小时: 60 * 60 * 1000,
        小時: 60 * 60 * 1000,
        天: 24 * 60 * 60 * 1000,
        周: 7 * 24 * 60 * 60 * 1000,
        週: 7 * 24 * 60 * 60 * 1000,
        个月: 30 * 24 * 60 * 60 * 1000,
        個月: 30 * 24 * 60 * 60 * 1000,
        月: 30 * 24 * 60 * 60 * 1000,
        年: 365 * 24 * 60 * 60 * 1000,
      };
      const multiplier = multipliers[unit];
      if (multiplier) return Math.max(0, nowDate.getTime() - amount * multiplier);
    }
  }

  if (/^刚刚$/i.test(text)) return nowDate.getTime();

  const dayWordMatch = text.match(/^(今天|昨日|昨天|前天)\s*(\d{1,2}:\d{2})?$/);
  if (dayWordMatch) {
    const [, dayWord, timePart] = dayWordMatch;
    const candidate = new Date(nowDate);
    candidate.setSeconds(0, 0);
    if (dayWord === '昨天' || dayWord === '昨日') candidate.setDate(candidate.getDate() - 1);
    if (dayWord === '前天') candidate.setDate(candidate.getDate() - 2);
    const [hours, minutes] = String(timePart || '00:00').split(':').map((value) => Number(value || 0));
    candidate.setHours(hours || 0, minutes || 0, 0, 0);
    return candidate.getTime();
  }

  const fullDateMatch = text.match(/^(\d{4})[年./-](\d{1,2})[月./-](\d{1,2})(?:日)?(?:\s+(\d{1,2}):(\d{2})(?::(\d{2}))?)?$/);
  if (fullDateMatch) {
    const [, year, month, day, hours, minutes, seconds] = fullDateMatch;
    return buildDateFromParts(nowDate, {
      year: Number(year),
      month: Number(month),
      day: Number(day),
      hours: Number(hours || 0),
      minutes: Number(minutes || 0),
      seconds: Number(seconds || 0),
    }).getTime();
  }

  const monthDayMatch = text.match(/^(\d{1,2})[月/-](\d{1,2})(?:日)?(?:\s+(\d{1,2}):(\d{2})(?::(\d{2}))?)?$/);
  if (monthDayMatch) {
    const [, month, day, hours, minutes, seconds] = monthDayMatch;
    let candidate = buildDateFromParts(nowDate, {
      year: nowDate.getFullYear(),
      month: Number(month),
      day: Number(day),
      hours: Number(hours || 0),
      minutes: Number(minutes || 0),
      seconds: Number(seconds || 0),
    });
    if (candidate.getTime() - nowDate.getTime() > 24 * 60 * 60 * 1000) {
      candidate = buildDateFromParts(nowDate, {
        year: nowDate.getFullYear() - 1,
        month: Number(month),
        day: Number(day),
        hours: Number(hours || 0),
        minutes: Number(minutes || 0),
        seconds: Number(seconds || 0),
      });
    }
    return candidate.getTime();
  }

  return 0;
}

/**
 * 采集单篇笔记数据
 * 技术路径：注入 noteMap.js → 从 __INITIAL_STATE__ 提取结构化数据
 *
 * @param {Window} wd - 目标 window（主页面或 iframe.contentWindow）
 * @returns {Object} 采集到的笔记数据
 */
export async function collectNote(wd = window, options = {}) {
  if (!isContextValid()) {
    throw new Error(XHS_CONTEXT_REFRESH_MESSAGE);
  }

  // 1. 注入脚本获取 noteDetailMap（最多重试 3 次，等待 __INITIAL_STATE__ 填充）
  let noteMap = null;
  let lastErr = null;
  for (let attempt = 0; attempt < 3; attempt++) {
    try {
      if (attempt > 0) {
        await new Promise(r => setTimeout(r, 1500 * attempt));
      }
      noteMap = await getByInject(wd, 'noteMap');
      if (noteMap && Object.keys(noteMap).length > 0) break;
    } catch (e) {
      lastErr = e;
      console.warn(`[灵感爆爆爆] getByInject 第 ${attempt + 1} 次失败:`, e.message);
      const msg = String(e?.message || '');
      if (/Extension context invalidated|context invalidated/i.test(msg) || !isContextValid()) {
        lastErr = new Error(XHS_CONTEXT_REFRESH_MESSAGE);
        break;
      }
    }
    noteMap = null;
  }

  if (!noteMap || Object.keys(noteMap).length === 0) {
    throw lastErr || new Error('未找到笔记数据，请确认当前页面是笔记详情页');
  }

  // 2. 找到正确的笔记数据
  // 优先用当前 URL 的 noteId 精确匹配，避免拿到 'undefined' / '' 等无效 key
  const expectedNoteId = String(options.expectedNoteId || '').trim();
  const currentUrl = wd.location?.href || window.location.href;
  const currentNoteId = extractNoteId(currentUrl);
  const validKeys = Object.keys(noteMap).filter(k => k && k !== 'undefined' && k.length > 10);

  console.log('[灵感爆爆爆] noteMap keys:', validKeys, '| currentNoteId:', currentNoteId, '| expectedNoteId:', expectedNoteId);

  const noteKey = selectNoteKey(noteMap, expectedNoteId, currentUrl);

  if (!noteKey) {
    throw new Error('未找到笔记数据，请确认当前页面是笔记详情页');
  }

  const { note } = resolveExpectedNoteFromMap(noteMap, expectedNoteId, currentUrl);

  console.log('[灵感爆爆爆] note 原始数据:', JSON.stringify({
    noteId: note?.noteId, id: note?.id, title: note?.title,
    likes: note?.interactInfo?.likedCount, type: note?.type,
  }));

  if (!note || (!note.noteId && !note.id && !note.title)) {
    throw new Error('笔记数据解析失败，数据结构异常');
  }

  if (!isCollectedNoteUsable(note, expectedNoteId, { requireStats: true })) {
    throw new Error(`笔记数据未稳定就绪: expected=${expectedNoteId || 'unknown'} actual=${note.noteId || note.id || ''}`);
  }

  // 3. 映射字段
  const imageUrls = (note.imageList || [])
    .map((img) => toHighQualityImageUrl(getXhsImageUrl(img)))
    .filter(Boolean);
  const imageCandidates = (note.imageList || [])
    .map((img) => getHighQualityImageCandidates(getXhsImageUrl(img)))
    .filter((arr) => arr.length > 0);
  const livePhotoStreams = extractXhsLivePhotoStreams(note.imageList || []);
  const videoSelection = pickBestVideoStream(note.video?.media?.stream || {});
  const existing = await noteStore.getById(note.noteId || note.id || noteKey);

  const platformContentId = note.noteId || note.id || noteKey;
  const collectedAt = Date.now();
  const rawPublicCommentCount = firstPresentValue(note.interactInfo, ['commentCount', 'comments']);
  const publicCommentCount = rawPublicCommentCount == null ? null : parseCount(rawPublicCommentCount);
  const publishedAt = parseXhsPublishedAt(
    note.publishTime
      || note.publishDate
      || note.publishedAt
      || note.createTime
      || note.create_time
      || note.time,
    { now: collectedAt },
  );

  const noteInfo = withMonitorRecordMeta({
    noteId: platformContentId,
    contentId: `xhs_${platformContentId}`,
    platformContentId,
    platform: 'xhs',
    url: safeUrl(wd.location?.href || window.location.href),
    canonicalUrl: safeUrl(wd.location?.href || window.location.href),
    title: cleanNoteBodyText(note.title || ''),
    content: cleanNoteBodyText(note.desc || ''),
    bodyText: cleanNoteBodyText(note.desc || ''),
    hashtags: [...new Set([...extractHashtags(note.title || ''), ...extractHashtags(note.desc || '')])],
    type: note.type === 'video' ? 'video' : 'normal',
    cover: toHighQualityImageUrl(getXhsImageUrl(note.imageList?.[0] || {})),
    images: imageUrls,
    imageCandidates,
    livePhotoStreams,
    video: videoSelection.url,
    videoStreams: videoSelection.streams || [],
    likes: parseXhsInteractCount(note.interactInfo, ['likedCount', 'likeCount', 'likes']),
    collects: parseXhsInteractCount(note.interactInfo, ['collectedCount', 'collectCount', 'collects', 'favoriteCount']),
    comments: publicCommentCount ?? 0,
    publicCommentCount,
    publicCommentCountKnown: publicCommentCount !== null,
    shares: parseXhsInteractCount(note.interactInfo, ['shareCount', 'shares']),
    keywords: (note.tagList || []).map(t => t.name).filter(Boolean),
    topicIds: (note.tagList || []).map(t => t.id).filter(Boolean),
    atUserList: (note.atUserList || []).map((u) => ({
      userId: u?.userId || '',
      nickname: u?.nickname || '',
    })).filter((u) => u.userId || u.nickname),
    releaseDate: note.time || '',
    lastUpdateTime: note.lastUpdateTime || '',
    ipLocation: note.ipLocation || '',
    shareRestricted: Boolean(note.shareInfo?.unShare),
    authorFollowed: Boolean(note.interactInfo?.followed),
    authorId: note.user?.userId || '',
    authorEntityId: note.user?.userId ? `xhs_${note.user.userId}` : '',
    authorName: note.user?.nickname || '',
    authorAvatar: note.user?.avatar || '',
    publishedAt,
    publishedAtText: note.time || '',
    collectedAt,
    updatedAt: collectedAt,
    collectionRunId: String(options.collectionRunId || '').trim(),
    dataSource: '__INITIAL_STATE__',
    createdAt: existing?.createdAt || collectedAt,
    mediaQuality: 'HD',
    syncStatus: 'pending',
    lastSyncAt: null,
    ...createCollectorEvidence({
      rawPayload: note,
      rawDomText: joinRawDomText([
        note.title || '',
        note.desc || '',
        (note.tagList || []).map((item) => item?.name || '').filter(Boolean).join(' '),
      ]),
      rawUrl: safeUrl(wd.location?.href || window.location.href),
      rawSource: '__INITIAL_STATE__.noteMap',
    }),
  }, options.monitorMeta);

  // 4. 写入 IndexedDB（主键 noteId 自动去重）
  await noteStore.upsert(noteInfo);

  return noteInfo;
}

/**
 * 从搜索/博主页的笔记卡片 DOM 中提取基础信息
 * 用于批量采集时的笔记列表发现
 *
 * 按视觉位置排序（先上后下，同行先左后右），解决瀑布流双列布局下
 * DOM 顺序 ≠ 视觉顺序导致的采集乱序问题
 */
export function discoverNotesFromDOM(containerSelector) {
  const notes = [];
  const seenIds = new Set();
  const sections = document.querySelectorAll(`${containerSelector} section`);

  sections.forEach((section) => {
    const coverLink = section.querySelector('a.cover');
    if (!coverLink) return;

    const url = coverLink.getAttribute('href') || '';
    const noteId = extractNoteId(url);
    if (!noteId || seenIds.has(noteId)) return;
    seenIds.add(noteId);

    const titleEl = section.querySelector('.footer span') || section.querySelector('.title');
    const likesEl = section.querySelector('.like-wrapper .count');
    const hasVideo = section.querySelector('.play-icon') !== null;
    const cover = pickCardCoverImage(section, coverLink);

    // 获取元素的视觉位置用于排序
    const rect = section.getBoundingClientRect();

    notes.push({
      noteId,
      url: safeUrl(url),
      title: titleEl?.textContent?.trim() || '',
      likes: likesEl?.textContent?.trim() || '0',
      type: hasVideo ? 'video' : 'normal',
      cover,
      coverImg: cover,
      coverUrl: cover,
      thumbnail: cover,
      images: cover ? [cover] : [],
      element: section,
      _top: rect.top + window.scrollY,
      _left: rect.left,
    });
  });

  // 按视觉位置排序：先按纵坐标（容差 50px 视为同行），再按横坐标
  notes.sort((a, b) => {
    const rowDiff = Math.abs(a._top - b._top);
    if (rowDiff < 50) return a._left - b._left; // 同行按左右
    return a._top - b._top; // 不同行按上下
  });

  return notes;
}

function normalizePositiveInteger(value, fallback = 0) {
  const parsed = Number(value);
  if (!Number.isFinite(parsed) || parsed <= 0) return fallback;
  return Math.floor(parsed);
}

function firstText(value) {
  if (typeof value !== 'string') return '';
  const trimmed = value.trim();
  return trimmed && trimmed !== '[object Object]' ? trimmed : '';
}

function readObject(source = {}, keys = []) {
  if (!source || typeof source !== 'object') return {};
  for (const key of keys) {
    const value = source[key];
    if (value && typeof value === 'object' && !Array.isArray(value)) return value;
  }
  return {};
}

function readArray(source = {}, keys = []) {
  if (!source || typeof source !== 'object') return [];
  for (const key of keys) {
    const value = source[key];
    if (Array.isArray(value)) return value;
  }
  return [];
}

function pickFirstText(source = {}, keys = []) {
  if (!source || typeof source !== 'object') return '';
  for (const key of keys) {
    const raw = source[key];
    const value = typeof raw === 'number' && Number.isFinite(raw)
      ? String(raw)
      : firstText(raw);
    if (value) return value;
  }
  return '';
}

function readBoolean(value) {
  if (value === true) return true;
  if (value === false || value == null) return false;
  const text = String(value).trim().toLowerCase();
  return text === 'true' || text === '1' || text === 'yes';
}

function readAttribute(element, name) {
  if (!element || typeof element.getAttribute !== 'function') return '';
  return firstText(element.getAttribute(name));
}

function firstSrcsetUrl(value = '') {
  const text = firstText(value);
  if (!text) return '';
  const candidates = text
    .split(',')
    .map((item) => firstText(item).split(/\s+/)[0])
    .filter(Boolean);
  return candidates[candidates.length - 1] || '';
}

function backgroundImageUrl(value = '') {
  const text = firstText(value);
  const match = text.match(/url\((["']?)(.*?)\1\)/i);
  return firstText(match?.[2] || '');
}

function pickImageUrlFromElement(element) {
  if (!element) return '';
  const candidates = [
    firstText(element.currentSrc),
    firstText(element.src),
    readAttribute(element, 'src'),
    readAttribute(element, 'data-src'),
    readAttribute(element, 'data-original'),
    readAttribute(element, 'data-lazy-src'),
    readAttribute(element, 'data-url'),
    firstSrcsetUrl(readAttribute(element, 'srcset')),
    backgroundImageUrl(element.style?.backgroundImage),
    backgroundImageUrl(readAttribute(element, 'style')),
  ];
  const raw = candidates.find(Boolean) || '';
  return raw ? toHighQualityImageUrl(raw) : '';
}

function pickProfilePostedCover(note = {}) {
  const cover = readObject(note, ['cover', 'coverInfo', 'cover_info']);
  const infoList = Array.isArray(cover.info_list)
    ? cover.info_list
    : (Array.isArray(cover.infoList) ? cover.infoList : []);
  for (const item of infoList) {
    const url = pickFirstText(item, ['url', 'urlDefault', 'url_default', 'src']);
    if (url) return toHighQualityImageUrl(url);
  }
  return toHighQualityImageUrl(
    pickFirstText(cover, ['url', 'urlDefault', 'url_default', 'src'])
    || pickFirstText(note, ['coverUrl', 'cover_url', 'cover'])
  );
}

function pickSearchNoteCover(noteCard = {}) {
  const imageList = readArray(noteCard, ['image_list', 'imageList', 'images']);
  for (const image of imageList) {
    const infoList = readArray(image, ['info_list', 'infoList']);
    for (const item of infoList) {
      const url = pickFirstText(item, ['url', 'urlDefault', 'url_default', 'src']);
      if (url) return toHighQualityImageUrl(url);
    }
    const direct = pickFirstText(image, ['urlDefault', 'url_default', 'url', 'src']);
    if (direct) return toHighQualityImageUrl(direct);
  }
  return pickProfilePostedCover(noteCard);
}

function collectSearchImageCandidates(noteCard = {}) {
  const imageList = readArray(noteCard, ['image_list', 'imageList', 'images']);
  return imageList
    .map((image) => {
      const infoList = readArray(image, ['info_list', 'infoList']);
      const urls = infoList
        .map((item) => pickFirstText(item, ['url', 'urlDefault', 'url_default', 'src']))
        .filter(Boolean)
        .map((url) => toHighQualityImageUrl(url));
      const direct = pickFirstText(image, ['urlDefault', 'url_default', 'url', 'src']);
      if (direct) urls.push(toHighQualityImageUrl(direct));
      return [...new Set(urls)];
    })
    .filter((urls) => urls.length > 0);
}

function extractXhsProfileUserIdFromUrl(url = '') {
  const text = String(url || '').trim();
  const match = text.match(/\/user\/profile\/([^/?#]+)/i);
  return match ? decodeURIComponent(match[1]) : '';
}

function safeDecodeText(value = '') {
  try {
    return decodeURIComponent(String(value || ''));
  } catch {
    return String(value || '');
  }
}

function extractXhsSearchKeywordFromUrl(url = '') {
  try {
    return safeDecodeText(new URL(String(url || ''), 'https://www.xiaohongshu.com').searchParams.get('keyword') || '').trim();
  } catch {
    return '';
  }
}

function readSearchPublishTimeText(noteCard = {}) {
  const tags = readArray(noteCard, ['corner_tag_info', 'cornerTagInfo', 'cornerTags']);
  for (const tag of tags) {
    const type = pickFirstText(tag, ['type', 'tagType']);
    const text = pickFirstText(tag, ['text', 'name', 'title']);
    if (text && (!type || type === 'publish_time')) return text;
  }
  return '';
}

function readCurrentProfileToken(currentUrl = '') {
  try {
    return new URL(String(currentUrl || ''), 'https://www.xiaohongshu.com').searchParams.get('xsec_token') || '';
  } catch {
    return '';
  }
}

function readCurrentXsecSource(currentUrl = '') {
  try {
    return new URL(String(currentUrl || ''), 'https://www.xiaohongshu.com').searchParams.get('xsec_source') || 'pc_user';
  } catch {
    return 'pc_user';
  }
}

function buildProfilePostedUrl({ userId = '', currentUrl = '', cursor = '' } = {}) {
  const token = readCurrentProfileToken(currentUrl);
  if (!userId || !token) return '';
  const params = new URLSearchParams({
    num: '30',
    cursor,
    user_id: userId,
    image_formats: 'jpg,webp,avif',
    xsec_token: token,
    xsec_source: readCurrentXsecSource(currentUrl),
  });
  return `https://edith.xiaohongshu.com/api/sns/web/v1/user_posted?${params.toString()}`;
}

function readProfilePostedNotes(json = {}) {
  const payload = json?.data && typeof json.data === 'object' ? json.data : json;
  const candidates = [
    payload?.notes,
    payload?.items,
    payload?.list,
    json?.notes,
    json?.items,
    json?.list,
    Array.isArray(payload) ? payload : null,
  ];
  for (const candidate of candidates) {
    if (Array.isArray(candidate)) return candidate;
  }
  return [];
}

export function normalizeProfilePostedNote(note = {}, {
  index = 0,
  sourceUrl = '',
  userId = '',
} = {}) {
  const noteId = firstText(note.note_id) || firstText(note.noteId) || firstText(note.id);
  if (!noteId) return null;

  const author = readObject(note, ['user', 'user_info', 'userInfo', 'author']);
  const authorId = pickFirstText(author, ['user_id', 'userId', 'id']) || userId;
  const xsecToken = pickFirstText(note, ['xsec_token', 'xsecToken']);
  const url = xsecToken
    ? `https://www.xiaohongshu.com/explore/${encodeURIComponent(noteId)}?xsec_token=${encodeURIComponent(xsecToken)}&xsec_source=pc_user`
    : `https://www.xiaohongshu.com/explore/${encodeURIComponent(noteId)}`;
  const interact = readObject(note, ['interact_info', 'interactInfo', 'interact']);
  const cover = pickProfilePostedCover(note);

  return {
    noteId,
    url,
    title: pickFirstText(note, ['display_title', 'displayTitle', 'title']),
    likes: pickFirstText(interact, ['liked_count', 'likedCount', 'like_count', 'likeCount']) || '0',
    collects: pickFirstText(interact, ['collected_count', 'collectedCount', 'collect_count', 'collectCount']) || '',
    comments: pickFirstText(interact, ['comment_count', 'commentCount', 'comments']) || '',
    shares: pickFirstText(interact, ['shared_count', 'sharedCount', 'share_count', 'shareCount']) || '',
    isPinned: readBoolean(interact.sticky ?? interact.isSticky ?? note.sticky ?? note.isSticky),
    sticky: readBoolean(interact.sticky ?? interact.isSticky ?? note.sticky ?? note.isSticky),
    type: pickFirstText(note, ['type', 'note_type', 'noteType']) || 'normal',
    cover,
    coverImg: cover,
    coverUrl: cover,
    thumbnail: cover,
    images: cover ? [cover] : [],
    authorId,
    authorPlatformId: authorId,
    authorName: pickFirstText(author, ['nickname', 'nick_name', 'name']),
    authorAvatar: pickFirstText(author, ['avatar', 'image', 'imageb']),
    sourceUrl,
    dataSource: 'xhs.user_posted',
    _discoveryOrder: index,
    _top: index * 160,
    _left: 0,
  };
}

function collectProfilePostedPages(snapshot = {}, userId = '') {
  const pages = Array.isArray(snapshot?.pages) ? snapshot.pages : [];
  const notes = [];
  for (const page of pages) {
    const pageUserId = firstText(page?.userId);
    if (userId && pageUserId && pageUserId !== userId) continue;
    const sourceUrl = firstText(page?.sourceUrl);
    const pageNotes = Array.isArray(page?.notes) ? page.notes : [];
    for (const item of pageNotes) {
      notes.push({ note: item, sourceUrl });
    }
  }
  return notes;
}

function dedupeProfilePostedNotes(items = [], userId = '', limit = 30) {
  const seen = new Set();
  const normalized = [];
  for (const item of items) {
    const mapped = normalizeProfilePostedNote(item.note || item, {
      index: normalized.length,
      sourceUrl: item.sourceUrl || '',
      userId,
    });
    if (!mapped || seen.has(mapped.noteId)) continue;
    seen.add(mapped.noteId);
    normalized.push(mapped);
    if (limit > 0 && normalized.length >= limit) break;
  }
  return normalized;
}

export async function discoverProfileSurfaceNotesFromApi({
  expectedCount = 30,
  currentUrl = '',
  requestSnapshot = requestXhsProfileNotesSnapshot,
  fetchJson = fetchXhsJsonViaBridge,
  ensureBridge = ensureXhsCommentApiBridge,
} = {}) {
  const sourceUrl = currentUrl || (typeof window !== 'undefined' ? window.location.href : '');
  const userId = extractXhsProfileUserIdFromUrl(sourceUrl);
  if (!userId) return [];

  try {
    ensureBridge?.();
  } catch {
    // Bridge injection is best-effort; captured page requests may already be available.
  }

  const limit = normalizePositiveInteger(expectedCount, 30);
  const fromSnapshot = await requestSnapshot(userId)
    .then((snapshot) => dedupeProfilePostedNotes(collectProfilePostedPages(snapshot, userId), userId, limit))
    .catch(() => []);
  if (fromSnapshot.length > 0) {
    return attachSurfaceDiscoveryMeta(fromSnapshot, {
      method: 'captured_user_posted',
      expectedCount: limit,
      totalNotes: fromSnapshot.length,
      stopReason: fromSnapshot.length >= limit ? 'target_reached' : 'captured_partial',
      isFinished: fromSnapshot.length >= limit,
    });
  }

  const requestUrl = buildProfilePostedUrl({ userId, currentUrl: sourceUrl });
  if (!requestUrl) return [];

  const json = await fetchJson([requestUrl]).catch(() => null);
  if (!json) return [];
  const notes = dedupeProfilePostedNotes(readProfilePostedNotes(json).map((note) => ({ note, sourceUrl: requestUrl })), userId, limit);
  return attachSurfaceDiscoveryMeta(notes, {
    method: 'user_posted_direct',
    expectedCount: limit,
    totalNotes: notes.length,
    stopReason: notes.length >= limit ? 'target_reached' : 'api_partial',
    isFinished: notes.length >= limit,
  });
}

function readSearchNoteCard(item = {}) {
  const candidates = [
    item?.note_card,
    item?.noteCard,
    item?.note,
    item?.card,
    item,
  ];
  for (const candidate of candidates) {
    if (candidate && typeof candidate === 'object' && !Array.isArray(candidate)) return candidate;
  }
  return {};
}

export function normalizeSearchSurfaceNote(item = {}, {
  index = 0,
  sourceUrl = '',
  keyword = '',
} = {}) {
  const noteCard = readSearchNoteCard(item);
  const noteId = firstText(item.id)
    || pickFirstText(noteCard, ['note_id', 'noteId', 'id']);
  if (!noteId) return null;

  const xsecToken = pickFirstText(item, ['xsec_token', 'xsecToken'])
    || pickFirstText(noteCard, ['xsec_token', 'xsecToken']);
  const url = xsecToken
    ? `https://www.xiaohongshu.com/explore/${encodeURIComponent(noteId)}?xsec_token=${encodeURIComponent(xsecToken)}&xsec_source=pc_search`
    : `https://www.xiaohongshu.com/explore/${encodeURIComponent(noteId)}`;
  const user = readObject(noteCard, ['user', 'user_info', 'userInfo', 'author']);
  const interact = readObject(noteCard, ['interact_info', 'interactInfo', 'interact']);
  const cover = pickSearchNoteCover(noteCard);
  const imageCandidates = collectSearchImageCandidates(noteCard);
  const images = imageCandidates.flat().filter(Boolean);

  return {
    noteId,
    url,
    title: pickFirstText(noteCard, ['display_title', 'displayTitle', 'title']),
    likes: pickFirstText(interact, ['liked_count', 'likedCount', 'like_count', 'likeCount']) || '0',
    collects: pickFirstText(interact, ['collected_count', 'collectedCount', 'collect_count', 'collectCount']) || '',
    comments: pickFirstText(interact, ['comment_count', 'commentCount', 'comments']) || '',
    shares: pickFirstText(interact, ['shared_count', 'sharedCount', 'share_count', 'shareCount']) || '',
    isPinned: false,
    sticky: false,
    type: pickFirstText(noteCard, ['type', 'note_type', 'noteType']) || 'normal',
    cover,
    coverImg: cover,
    coverUrl: cover,
    thumbnail: cover,
    images: images.length > 0 ? [...new Set(images)] : (cover ? [cover] : []),
    imageCandidates,
    authorId: pickFirstText(user, ['user_id', 'userId', 'id']),
    authorPlatformId: pickFirstText(user, ['user_id', 'userId', 'id']),
    authorName: pickFirstText(user, ['nickname', 'nick_name', 'name']),
    authorAvatar: pickFirstText(user, ['avatar', 'image', 'imageb']),
    publishedAtText: readSearchPublishTimeText(noteCard),
    searchKeyword: keyword,
    sourceUrl,
    dataSource: 'xhs.search_notes',
    _discoveryOrder: index,
    _top: index * 160,
    _left: 0,
  };
}

function collectSearchNotePages(snapshot = {}, keyword = '') {
  const pages = Array.isArray(snapshot?.pages) ? snapshot.pages : [];
  const notes = [];
  for (const page of pages) {
    const pageKeyword = firstText(page?.keyword);
    if (keyword && pageKeyword && pageKeyword !== keyword) continue;
    const sourceUrl = firstText(page?.sourceUrl);
    const pageNotes = Array.isArray(page?.notes) ? page.notes : [];
    for (const item of pageNotes) {
      notes.push({ note: item, sourceUrl, keyword: pageKeyword || keyword });
    }
  }
  return notes;
}

function dedupeSearchSurfaceNotes(items = [], keyword = '', limit = 30) {
  const seen = new Set();
  const normalized = [];
  for (const item of items) {
    const mapped = normalizeSearchSurfaceNote(item.note || item, {
      index: normalized.length,
      sourceUrl: item.sourceUrl || '',
      keyword: item.keyword || keyword,
    });
    if (!mapped || seen.has(mapped.noteId)) continue;
    seen.add(mapped.noteId);
    normalized.push(mapped);
    if (limit > 0 && normalized.length >= limit) break;
  }
  return normalized;
}

export async function discoverSearchSurfaceNotesFromApi({
  expectedCount = 30,
  currentUrl = '',
  requestSnapshot = requestXhsSearchNotesSnapshot,
  ensureBridge = ensureXhsCommentApiBridge,
} = {}) {
  const sourceUrl = currentUrl || (typeof window !== 'undefined' ? window.location.href : '');
  const keyword = extractXhsSearchKeywordFromUrl(sourceUrl);
  if (!keyword) return [];

  try {
    ensureBridge?.();
  } catch {
    // Bridge injection is best-effort; search results may already be captured.
  }

  const limit = normalizePositiveInteger(expectedCount, 30);
  return requestSnapshot(keyword)
    .then((snapshot) => dedupeSearchSurfaceNotes(collectSearchNotePages(snapshot, keyword), keyword, limit))
    .then((notes) => attachSurfaceDiscoveryMeta(notes, {
      method: 'captured_search_notes',
      expectedCount: limit,
      totalNotes: notes.length,
      stopReason: notes.length >= limit ? 'target_reached' : 'captured_partial',
      isFinished: notes.length >= limit,
    }))
    .catch(() => []);
}

function getSurfaceNoteMergeKey(note = {}) {
  const id = firstText(
    note.noteId
    || note.platformContentId
    || note.id
    || note.contentId
    || note.note_id
    || '',
  );
  if (id) return `id:${id}`;
  const url = firstText(note.url || note.link || note.href || '');
  if (url) return `url:${url.split('?')[0]}`;
  return '';
}

function mergeSurfaceNotes(...groups) {
  const notes = [];
  const seen = new Set();
  for (const group of groups) {
    for (const note of Array.isArray(group) ? group : []) {
      if (!note) continue;
      const key = getSurfaceNoteMergeKey(note);
      if (key && seen.has(key)) continue;
      if (key) seen.add(key);
      notes.push(note);
    }
  }
  return notes;
}

function buildSurfaceDiscoveryFieldQuality(notes = []) {
  const totalNotes = Array.isArray(notes) ? notes.length : 0;
  const countWith = (predicate) => (Array.isArray(notes) ? notes.filter(predicate).length : 0);
  const withTitle = countWith((note) => firstText(note?.title || note?.content || note?.bodyText));
  const withLikeText = countWith((note) => firstText(note?.likes || note?.likeText));
  const withLikeCount = countWith((note) => firstText(note?.likes || note?.likeText) !== '');
  const withCover = countWith((note) => firstText(note?.cover || note?.coverUrl || note?.coverImg || note?.thumbnail));
  const withXsecToken = countWith((note) => /[?&]xsec_token=/i.test(firstText(note?.url || note?.sourceUrl || note?.rawUrl)));
  const pinnedCount = countWith((note) => Boolean(note?.isPinned || note?.sticky));
  const rate = (value) => (totalNotes > 0 ? Math.round((value / totalNotes) * 1000) / 1000 : 0);
  return {
    totalNotes,
    withTitle,
    withLikeText,
    withLikeCount,
    withCover,
    withXsecToken,
    pinnedCount,
    titleRate: rate(withTitle),
    likeTextRate: rate(withLikeText),
    likeCountRate: rate(withLikeCount),
    coverRate: rate(withCover),
    xsecTokenRate: rate(withXsecToken),
  };
}

function attachSurfaceDiscoveryMeta(notes = [], meta = {}) {
  if (!Array.isArray(notes)) return notes;
  const safeMeta = meta && typeof meta === 'object' && !Array.isArray(meta) ? meta : {};
  Object.defineProperty(notes, 'discoveryMeta', {
    value: {
      ...safeMeta,
      totalNotes: Number.isFinite(Number(safeMeta.totalNotes)) ? Number(safeMeta.totalNotes) : notes.length,
      fieldQuality: safeMeta.fieldQuality || buildSurfaceDiscoveryFieldQuality(notes),
    },
    configurable: true,
    enumerable: false,
  });
  return notes;
}

function readSurfaceDiscoveryMeta(notes = []) {
  return notes && typeof notes === 'object' && !Array.isArray(notes.discoveryMeta)
    && notes.discoveryMeta && typeof notes.discoveryMeta === 'object'
    ? notes.discoveryMeta
    : null;
}

function limitSurfaceNotes(notes = [], expectedCount = 0) {
  if (expectedCount <= 0) return notes;
  return attachSurfaceDiscoveryMeta(notes.slice(0, expectedCount), readSurfaceDiscoveryMeta(notes) || {});
}

function buildMergedDiscoveryMeta({
  preferredNotes = [],
  scrollNotes = [],
  mergedNotes = [],
  expectedCount = 0,
} = {}) {
  const preferredMeta = readSurfaceDiscoveryMeta(preferredNotes);
  const scrollMeta = readSurfaceDiscoveryMeta(scrollNotes);
  const sourceMethod = preferredNotes.length > 0 && scrollNotes.length > 0
    ? 'captured_plus_dom_scroll'
    : (scrollMeta?.method || preferredMeta?.method || 'dom_scroll_persistent_map');
  return {
    method: sourceMethod,
    expectedCount,
    preferredCount: preferredNotes.length,
    scrollCount: scrollNotes.length,
    totalNotes: mergedNotes.length,
    stopReason: scrollMeta?.stopReason || preferredMeta?.stopReason || '',
    rounds: scrollMeta?.rounds || 0,
    maxRounds: scrollMeta?.maxRounds || 0,
    canLoadMore: scrollMeta?.canLoadMore ?? undefined,
    isFinished: scrollMeta?.isFinished ?? (expectedCount > 0 ? mergedNotes.length >= expectedCount : undefined),
    fieldQuality: buildSurfaceDiscoveryFieldQuality(mergedNotes),
  };
}

export async function discoverSurfaceNotesFromBestSource(containerSelector, maxScrolls = 10, options = {}) {
  const currentUrl = String(
    options.currentUrl
    || (typeof window !== 'undefined' ? window.location.href : ''),
  ).trim();
  const expectedCount = normalizePositiveInteger(options.expectedCount, 0);
  const profileDiscover = typeof options.profileDiscover === 'function'
    ? options.profileDiscover
    : discoverProfileSurfaceNotesFromApi;
  const searchDiscover = typeof options.searchDiscover === 'function'
    ? options.searchDiscover
    : discoverSearchSurfaceNotesFromApi;
  const scrollDiscover = typeof options.scrollDiscover === 'function'
    ? options.scrollDiscover
    : discoverWithScroll;

  let preferredNotes = [];
  if (containerSelector === '#userPostedFeeds') {
    preferredNotes = await profileDiscover({ expectedCount, currentUrl }).catch(() => []);
  } else if (extractXhsSearchKeywordFromUrl(currentUrl)) {
    preferredNotes = await searchDiscover({ expectedCount, currentUrl }).catch(() => []);
  }

  if (preferredNotes.length > 0 && (expectedCount <= 0 || preferredNotes.length >= expectedCount)) {
    return limitSurfaceNotes(preferredNotes, expectedCount);
  }

  const scrollNotes = await scrollDiscover(containerSelector, maxScrolls, options);
  const mergedNotes = mergeSurfaceNotes(preferredNotes, scrollNotes);
  attachSurfaceDiscoveryMeta(mergedNotes, buildMergedDiscoveryMeta({
    preferredNotes,
    scrollNotes,
    mergedNotes,
    expectedCount,
  }));
  return limitSurfaceNotes(mergedNotes, expectedCount);
}

function pickCardCoverImage(section, coverLink) {
  const imageSelectors = [
    'img, picture img, source',
    'img',
    'picture img',
    'source',
  ];
  const candidates = [];

  for (const selector of imageSelectors) {
    const fromCover = typeof coverLink?.querySelector === 'function'
      ? coverLink.querySelector(selector)
      : null;
    if (fromCover) candidates.push(fromCover);

    const fromSection = typeof section?.querySelector === 'function'
      ? section.querySelector(selector)
      : null;
    if (fromSection) candidates.push(fromSection);
  }

  candidates.push(coverLink, section);

  for (const candidate of candidates) {
    const imageUrl = pickImageUrlFromElement(candidate);
    if (imageUrl) return imageUrl;
  }

  return '';
}

function getWindowScrollTarget() {
  return { type: 'window', element: null };
}

function isScrollableElement(element) {
  if (!element) return false;
  const scrollHeight = Number(element.scrollHeight || 0);
  const clientHeight = Number(element.clientHeight || 0);
  return clientHeight > 0 && scrollHeight > clientHeight + 24;
}

function getDiscoveryScrollTarget(containerSelector) {
  if (typeof document === 'undefined' || typeof document.querySelector !== 'function') {
    return getWindowScrollTarget();
  }

  let node = document.querySelector(containerSelector);
  while (node && node !== document.body && node !== document.documentElement) {
    if (isScrollableElement(node)) {
      return { type: 'element', element: node };
    }
    node = node.parentElement;
  }

  return getWindowScrollTarget();
}

function isElementScrollTarget(scrollTarget) {
  return scrollTarget?.type === 'element' && scrollTarget.element;
}

function getScrollMetrics(scrollTarget = getWindowScrollTarget()) {
  if (isElementScrollTarget(scrollTarget)) {
    const element = scrollTarget.element;
    const elementHeight = Number(element.clientHeight || 0);
    const windowHeight = typeof window === 'undefined' ? Number(document?.documentElement?.clientHeight || 0) : Number(window.innerHeight || 0);
    // 对列表自身可滚动的虚拟流，必须按列表可视高度算“到底”，不能拿浏览器整窗高度代替。
    const viewportHeight = Math.max(elementHeight || windowHeight, 1);
    const scrollHeight = Math.max(Number(element.scrollHeight || 0), viewportHeight);
    const scrollTop = Math.max(Number(element.scrollTop || 0), 0);
    const maxTop = Math.max(0, scrollHeight - viewportHeight);
    return {
      scrollTop,
      viewportHeight,
      scrollHeight,
      maxTop,
      atBottom: scrollTop >= Math.max(0, maxTop - 24),
    };
  }

  const doc = document.documentElement || document.body;
  const body = document.body || doc;
  const scrollTop = Math.max(window.scrollY || 0, doc?.scrollTop || 0, body?.scrollTop || 0);
  const viewportHeight = Math.max(window.innerHeight || 0, doc?.clientHeight || 0, body?.clientHeight || 0);
  const scrollHeight = Math.max(doc?.scrollHeight || 0, body?.scrollHeight || 0, viewportHeight);
  const maxTop = Math.max(0, scrollHeight - viewportHeight);
  return {
    scrollTop,
    viewportHeight,
    scrollHeight,
    maxTop,
    atBottom: scrollTop >= Math.max(0, maxTop - 24),
  };
}

function scrollDiscoveryTargetTo(scrollTarget, top) {
  const nextTop = Math.max(0, Number(top || 0));
  if (isElementScrollTarget(scrollTarget)) {
    scrollTarget.element.scrollTop = nextTop;
    return;
  }
  window.scrollTo({ top: nextTop, behavior: 'auto' });
}

function scrollDiscoveryTargetBy(scrollTarget, top) {
  const offset = Number(top || 0);
  if (isElementScrollTarget(scrollTarget)) {
    const currentTop = Math.max(0, Number(scrollTarget.element.scrollTop || 0));
    scrollTarget.element.scrollTop = Math.max(0, currentTop + offset);
    return;
  }
  window.scrollBy({ top: offset, behavior: 'auto' });
}

function sleep(ms = 0) {
  return new Promise((resolve) => setTimeout(resolve, Math.max(0, Number(ms || 0))));
}

function clampNumber(value, min, max) {
  const num = Number(value);
  if (!Number.isFinite(num)) return min;
  return Math.min(max, Math.max(min, num));
}

function estimateProfileMaxRounds(expectedCount = 0) {
  if (expectedCount >= 150) return 180;
  if (expectedCount >= 80) return 140;
  if (expectedCount >= 40) return 90;
  if (expectedCount > 0) return 45;
  return 30;
}

function estimateProfileStableLimit(expectedCount = 0) {
  if (expectedCount >= 150) return 15;
  if (expectedCount >= 80) return 12;
  if (expectedCount >= 40) return 10;
  return 6;
}

function estimateProfileBottomConfirmationRounds(expectedCount = 0) {
  if (expectedCount >= 150) return 12;
  if (expectedCount >= 80) return 10;
  if (expectedCount >= 40) return 9;
  return 6;
}

const PROFILE_SCROLL_RATIOS = [0.58, 0.74, 0.88, 0.66, 0.81, 0.62, 0.9, 0.7];
const PROFILE_SETTLE_EXTRA_MS = [120, 360, 220, 520, 180, 420, 260, 600];
const PROFILE_MICRO_PAUSE_MS = [90, 150, 120, 210, 110, 180];

function getDiscoveryScrollRatio(plan = {}, roundIndex = 0) {
  if (!plan.isProfileMode) return plan.stepRatio;
  const value = PROFILE_SCROLL_RATIOS[roundIndex % PROFILE_SCROLL_RATIOS.length] || plan.stepRatio;
  return clampNumber(value, 0.5, 0.92);
}

function getDiscoverySettleDelay(plan = {}, roundIndex = 0) {
  if (!plan.isProfileMode) return plan.settleDelay;
  const extra = PROFILE_SETTLE_EXTRA_MS[roundIndex % PROFILE_SETTLE_EXTRA_MS.length] || 0;
  return Math.max(plan.settleDelay, plan.settleDelay + extra);
}

async function performDiscoveryScroll(scrollTarget, {
  currentTop = 0,
  nextTop = 0,
  step = 0,
  plan = {},
  roundIndex = 0,
} = {}) {
  if (!plan.isProfileMode) {
    if (nextTop > currentTop + 1) {
      scrollDiscoveryTargetTo(scrollTarget, nextTop);
    } else {
      scrollDiscoveryTargetBy(scrollTarget, step);
    }
    return;
  }

  const targetTop = nextTop > currentTop + 1 ? nextTop : currentTop + Math.max(120, step);
  const delta = Math.max(0, targetTop - currentTop);
  if (delta <= 1) return;

  const firstRatio = roundIndex % 3 === 0 ? 0.48 : (roundIndex % 3 === 1 ? 0.64 : 0.56);
  const firstTop = Math.max(currentTop + 80, Math.min(targetTop, currentTop + Math.round(delta * firstRatio)));
  scrollDiscoveryTargetTo(scrollTarget, firstTop);
  dispatchDiscoveryWheel(scrollTarget, Math.max(80, firstTop - currentTop));
  await sleep(PROFILE_MICRO_PAUSE_MS[roundIndex % PROFILE_MICRO_PAUSE_MS.length] || 120);

  scrollDiscoveryTargetTo(scrollTarget, targetTop);
  dispatchDiscoveryWheel(scrollTarget, Math.max(80, targetTop - firstTop));
}

export function buildDiscoveryPlan(containerSelector, {
  maxScrolls = 10,
  expectedCount = 0,
} = {}) {
  const isProfileMode = containerSelector === '#userPostedFeeds';
  const normalizedMaxScrolls = normalizePositiveInteger(maxScrolls, 10);
  const normalizedExpectedCount = normalizePositiveInteger(expectedCount, 0);
  const profileTargetRounds = estimateProfileMaxRounds(normalizedExpectedCount);
  const maxRounds = isProfileMode
    ? Math.max(
      normalizedMaxScrolls,
      profileTargetRounds,
    )
    : normalizedMaxScrolls;

  return {
    isProfileMode,
    expectedCount: normalizedExpectedCount,
    maxRounds,
    settleDelay: isProfileMode && normalizedExpectedCount >= 80 ? 1500 : (isProfileMode ? 1300 : 900),
    stableNoNewLimit: isProfileMode ? estimateProfileStableLimit(normalizedExpectedCount) : 2,
    bottomConfirmationRounds: isProfileMode ? estimateProfileBottomConfirmationRounds(normalizedExpectedCount) : 0,
    stepRatio: isProfileMode ? 0.74 : 0.68,
    requireBottomOrExpected: isProfileMode,
    humanScroll: isProfileMode,
  };
}

export function shouldStopDiscovery({
  noNewCount = 0,
  stableNoNewLimit = 2,
  discoveredCount = 0,
  expectedCount = 0,
  atBottom = false,
  bottomNoNewCount = 0,
  bottomConfirmationRounds = 0,
  requireBottomOrExpected = false,
} = {}) {
  if (noNewCount < stableNoNewLimit) return false;
  if (!requireBottomOrExpected) return true;
  const discoveredEnough = expectedCount > 0 && discoveredCount >= expectedCount;
  if (discoveredEnough) return true;
  return atBottom && bottomNoNewCount >= bottomConfirmationRounds;
}

function createWheelEvent(deltaY) {
  if (typeof WheelEvent === 'function') {
    return () => new WheelEvent('wheel', {
      deltaY,
      bubbles: true,
      cancelable: true,
    });
  }

  if (typeof Event === 'function') {
    return () => {
      const event = new Event('wheel', {
        bubbles: true,
        cancelable: true,
      });
      try {
        Object.defineProperty(event, 'deltaY', { value: deltaY });
      } catch {
        // Some browser Event objects do not allow redefining properties.
      }
      return event;
    };
  }

  return () => ({ type: 'wheel', deltaY });
}

function dispatchDiscoveryWheel(scrollTarget, deltaY) {
  const makeEvent = createWheelEvent(deltaY);
  const targets = isElementScrollTarget(scrollTarget)
    ? [scrollTarget.element]
    : [window, document, document.documentElement, document.body];

  for (const target of targets) {
    if (!target || typeof target.dispatchEvent !== 'function') continue;
    try {
      target.dispatchEvent(makeEvent());
    } catch {
      // The wheel event is only a loading nudge; normal scroll still moves the page.
    }
  }
}

async function probeProfileBottom(containerSelector, previousSnapshot, settleDelay, scrollTarget) {
  const metrics = getScrollMetrics(scrollTarget);
  if (!metrics.atBottom) return false;

  const bounceStep = Math.max(180, Math.round(metrics.viewportHeight * 0.35));
  const retreatTop = Math.max(0, metrics.scrollTop - bounceStep);
  if (retreatTop < metrics.scrollTop - 1) {
    scrollDiscoveryTargetTo(scrollTarget, retreatTop);
    await sleep(220);
  }

  const refreshed = getScrollMetrics(scrollTarget);
  const returnTop = Math.max(metrics.maxTop, refreshed.maxTop);
  if (returnTop > refreshed.scrollTop + 1) {
    scrollDiscoveryTargetTo(scrollTarget, returnTop);
  }

  const nudgeStep = Math.max(220, Math.round(metrics.viewportHeight * 0.45));
  scrollDiscoveryTargetBy(scrollTarget, nudgeStep);
  dispatchDiscoveryWheel(scrollTarget, nudgeStep);

  await waitForDiscoverySettle(
    containerSelector,
    previousSnapshot,
    settleDelay + 900,
    true,
  );
  await sleep(320);
  return true;
}

/**
 * 滚动发现更多笔记（处理懒加载）
 * 在批量采集前调用，确保尽可能多的笔记被加载到 DOM 中
 */
export async function discoverWithScroll(containerSelector, maxScrolls = 10, options = {}) {
  const allNotes = new Map(); // key=noteId，滚动期间持续累积，不依赖回顶后的 DOM
  let noNewCount = 0;
  let bottomNoNewCount = 0;
  let roundsUsed = 0;
  let stopReason = 'max_rounds_reached';
  let lastMetrics = null;
  let scrollTarget = getDiscoveryScrollTarget(containerSelector);
  const plan = buildDiscoveryPlan(containerSelector, {
    maxScrolls,
    expectedCount: options.expectedCount,
  });

  for (let i = 0; i < plan.maxRounds; i++) {
    roundsUsed = i + 1;
    scrollTarget = getDiscoveryScrollTarget(containerSelector);
    const found = discoverNotesFromDOM(containerSelector);
    let hasNew = false;

    for (const note of found) {
      if (!allNotes.has(note.noteId)) {
        allNotes.set(note.noteId, {
          ...note,
          _discoveryOrder: allNotes.size,
        });
        hasNew = true;
      }
    }

    const discoveredEnough = plan.expectedCount > 0 && allNotes.size >= plan.expectedCount;
    if (discoveredEnough) {
      stopReason = 'target_reached';
      break;
    }

    const metrics = getScrollMetrics(scrollTarget);
    lastMetrics = metrics;
    const discoverySnapshot = {
      visibleCount: found.length,
      knownNoteIds: new Set(allNotes.keys()),
    };
    if (!hasNew) {
      noNewCount++;
      if (plan.isProfileMode && metrics.atBottom && !discoveredEnough) {
        bottomNoNewCount++;
        if (bottomNoNewCount < plan.bottomConfirmationRounds) {
          await probeProfileBottom(containerSelector, discoverySnapshot, plan.settleDelay, scrollTarget);
          continue;
        }
      } else {
        bottomNoNewCount = 0;
      }
      if (shouldStopDiscovery({
        noNewCount,
        stableNoNewLimit: plan.stableNoNewLimit,
        discoveredCount: allNotes.size,
        expectedCount: plan.expectedCount,
        atBottom: metrics.atBottom,
        bottomNoNewCount,
        bottomConfirmationRounds: plan.bottomConfirmationRounds,
        requireBottomOrExpected: plan.requireBottomOrExpected,
      })) {
        stopReason = metrics.atBottom ? 'bottom_confirmed' : 'stable_no_new';
        break;
      }
    } else {
      noNewCount = 0;
      bottomNoNewCount = 0;
    }

    // 博主页使用长短步交替和分段停顿，模拟正常浏览，降低空白卡片和误判到底的概率。
    const step = Math.round(metrics.viewportHeight * getDiscoveryScrollRatio(plan, i));
    const nextTop = Math.min(metrics.maxTop, metrics.scrollTop + step);
    if (nextTop > metrics.scrollTop + 1 || !metrics.atBottom) {
      await performDiscoveryScroll(scrollTarget, {
        currentTop: metrics.scrollTop,
        nextTop,
        step,
        plan,
        roundIndex: i,
      });
    }
    await waitForDiscoverySettle(
      containerSelector,
      discoverySnapshot,
      getDiscoverySettleDelay(plan, i),
      plan.isProfileMode,
    );

    // 某些博主页会出现短暂空白，额外等待一次再做下一轮
    if (plan.isProfileMode && found.length === 0) {
      await sleep(450);
    }
  }

  // 滚回顶部
  scrollDiscoveryTargetTo(scrollTarget, 0);
  await sleep(200);

  // 关键修复：返回滚动期间累积的所有笔记（按发现时的视觉位置排序）
  // 不能再次调用 discoverNotesFromDOM，因为虚拟列表在回顶后已卸载底部卡片
  const result = Array.from(allNotes.values());
  result.sort((a, b) => {
    const orderA = Number.isFinite(a._discoveryOrder) ? a._discoveryOrder : Number.MAX_SAFE_INTEGER;
    const orderB = Number.isFinite(b._discoveryOrder) ? b._discoveryOrder : Number.MAX_SAFE_INTEGER;
    if (orderA !== orderB) return orderA - orderB;
    const rowDiff = Math.abs(a._top - b._top);
    if (rowDiff < 50) return a._left - b._left;
    return a._top - b._top;
  });
  if (result.length === 0 && stopReason === 'max_rounds_reached') stopReason = 'no_cards_found';
  return attachSurfaceDiscoveryMeta(result, {
    method: 'dom_scroll_persistent_map',
    expectedCount: plan.expectedCount,
    totalNotes: result.length,
    rounds: roundsUsed,
    maxRounds: plan.maxRounds,
    stopReason,
    noNewCount,
    bottomNoNewCount,
    lastRound: {
      scrollTop: Math.round(lastMetrics?.scrollTop || 0),
      viewportHeight: Math.round(lastMetrics?.viewportHeight || 0),
      documentHeight: Math.round(lastMetrics?.scrollHeight || 0),
      atBottom: Boolean(lastMetrics?.atBottom),
    },
    canLoadMore: stopReason === 'max_rounds_reached',
    isFinished: stopReason === 'target_reached' || stopReason === 'bottom_confirmed',
  });
}
