function safeDecode(value = '') {
  const text = String(value || '').trim();
  if (!text) return '';
  try {
    return decodeURIComponent(text).trim();
  } catch {
    return text;
  }
}

function toNonNegativeNumber(value, fallback = 0) {
  const next = Number(value);
  if (!Number.isFinite(next) || next < 0) return fallback;
  return next;
}

function parseHasMoreFlag(value, fallback = false) {
  if (value === true || value === 1 || value === '1') return true;
  if (value === false || value === 0 || value === '0') return false;
  return fallback;
}

function parseSearchUrl(sourceUrl = '') {
  const raw = String(sourceUrl || '').trim();
  if (!raw) return null;
  try {
    const base = typeof window !== 'undefined' && window.location?.origin
      ? window.location.origin
      : 'https://www.douyin.com';
    return new URL(raw, base);
  } catch {
    return null;
  }
}

export function normalizeDouyinSearchChannel(raw = '') {
  const value = String(raw || '').trim().toLowerCase();
  if (value === 'video' || value === 'aweme_video') return 'aweme_video';
  return 'aweme_general';
}

export function normalizeDouyinSearchKeyword(raw = '') {
  return safeDecode(raw).toLowerCase();
}

export function extractDouyinSearchAweme(item = {}) {
  const candidates = [
    item?.aweme_info,
    item?.awemeInfo,
    item?.aweme_detail,
    item?.awemeDetail,
    item?.item_data?.aweme_info,
    item?.item_data?.awemeInfo,
    item?.data?.aweme_info,
    item?.data?.awemeInfo,
    item?.aweme_infos?.[0],
    item?.aweme_list?.[0],
  ];

  for (const candidate of candidates) {
    if (candidate?.aweme_id) return candidate;
  }
  return null;
}

export function parseDouyinSearchPagePayload(json = {}, sourceUrl = '', capturedAt = Date.now()) {
  if (!String(sourceUrl || '').includes('/aweme/v1/web/general/search/stream/')) {
    return null;
  }

  const parsedUrl = parseSearchUrl(sourceUrl);
  const keyword = safeDecode(parsedUrl?.searchParams.get('keyword') || '');
  const searchChannel = normalizeDouyinSearchChannel(parsedUrl?.searchParams.get('search_channel') || '');
  const items = Array.isArray(json?.data) ? json.data : [];
  const awemeItems = items
    .map((item, index) => {
      const aweme = extractDouyinSearchAweme(item);
      const awemeId = String(aweme?.aweme_id || '').trim();
      if (!aweme || !awemeId) return null;
      return {
        awemeId,
        aweme,
        orderIndex: index,
      };
    })
    .filter(Boolean);

  if (!keyword || awemeItems.length === 0) {
    return null;
  }

  const offset = toNonNegativeNumber(
    json?.offset ?? parsedUrl?.searchParams.get('offset') ?? 0,
    0,
  );
  const nextOffset = toNonNegativeNumber(
    json?.next_offset ?? json?.offset ?? (offset + items.length),
    offset + items.length,
  );
  const hasMore = parseHasMoreFlag(
    json?.has_more ?? json?.hasMore,
    items.length > 0,
  );

  return {
    keyword,
    normalizedKeyword: normalizeDouyinSearchKeyword(keyword),
    searchChannel,
    offset,
    nextOffset,
    hasMore,
    sourceUrl: String(sourceUrl || '').trim(),
    capturedAt: toNonNegativeNumber(capturedAt, Date.now()),
    items: awemeItems,
  };
}

function getPageIdentity(page = {}) {
  return [
    normalizeDouyinSearchKeyword(page.keyword),
    normalizeDouyinSearchChannel(page.searchChannel),
    toNonNegativeNumber(page.offset, 0),
  ].join('::');
}

export function upsertDouyinSearchPage(pages = [], page = null, maxPages = 20) {
  const list = Array.isArray(pages) ? pages : [];
  if (!page || !Array.isArray(page.items) || page.items.length === 0) {
    return list.slice(-maxPages);
  }

  const pageId = getPageIdentity(page);
  const next = [];
  let replaced = false;

  for (const existing of list) {
    if (getPageIdentity(existing) !== pageId) {
      next.push(existing);
      continue;
    }
    if (!replaced) {
      next.push(page);
      replaced = true;
    }
  }

  if (!replaced) next.push(page);
  next.sort((a, b) => {
    const capturedDiff = toNonNegativeNumber(a?.capturedAt, 0) - toNonNegativeNumber(b?.capturedAt, 0);
    if (capturedDiff !== 0) return capturedDiff;
    return toNonNegativeNumber(a?.offset, 0) - toNonNegativeNumber(b?.offset, 0);
  });
  return next.slice(-maxPages);
}

export function mergeCapturedDouyinSearchPages(pages = [], {
  keyword = '',
  searchChannel = 'aweme_general',
} = {}) {
  const normalizedKeyword = normalizeDouyinSearchKeyword(keyword);
  const normalizedChannel = normalizeDouyinSearchChannel(searchChannel);
  const latestByOffset = new Map();

  for (const page of Array.isArray(pages) ? pages : []) {
    if (!page) continue;
    if (normalizeDouyinSearchKeyword(page.keyword) !== normalizedKeyword) continue;
    if (normalizeDouyinSearchChannel(page.searchChannel) !== normalizedChannel) continue;

    const offset = toNonNegativeNumber(page.offset, 0);
    const prev = latestByOffset.get(offset);
    if (!prev || toNonNegativeNumber(page.capturedAt, 0) >= toNonNegativeNumber(prev.capturedAt, 0)) {
      latestByOffset.set(offset, page);
    }
  }

  const orderedPages = [...latestByOffset.values()].sort((a, b) => {
    const offsetDiff = toNonNegativeNumber(a?.offset, 0) - toNonNegativeNumber(b?.offset, 0);
    if (offsetDiff !== 0) return offsetDiff;
    return toNonNegativeNumber(a?.capturedAt, 0) - toNonNegativeNumber(b?.capturedAt, 0);
  });

  const items = [];
  const seen = new Set();
  let hasMore = false;
  let nextOffset = 0;

  for (const page of orderedPages) {
    hasMore = Boolean(page?.hasMore);
    nextOffset = Math.max(nextOffset, toNonNegativeNumber(page?.nextOffset, 0));

    for (const rawItem of Array.isArray(page?.items) ? page.items : []) {
      const aweme = rawItem?.aweme || extractDouyinSearchAweme(rawItem);
      const awemeId = String(rawItem?.awemeId || aweme?.aweme_id || '').trim();
      if (!aweme || !awemeId || seen.has(awemeId)) continue;
      seen.add(awemeId);
      items.push({
        awemeId,
        aweme,
        keyword: page.keyword,
        searchChannel: normalizeDouyinSearchChannel(page.searchChannel),
        sourceUrl: String(page.sourceUrl || '').trim() || 'captured.search_stream',
        orderIndex: items.length,
      });
    }
  }

  return {
    items,
    hasMore,
    nextOffset,
    pageCount: orderedPages.length,
  };
}
