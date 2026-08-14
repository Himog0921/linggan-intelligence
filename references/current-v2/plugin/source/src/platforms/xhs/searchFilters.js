const SEARCH_PAGE_RE = /xiaohongshu\.com\/search_result/i;
const FEED_CONTAINER_SELECTOR = '.feeds-container';
const SEARCH_RESULT_SETTLE_TIMEOUT_MS = 15000;
const SEARCH_RESULT_SETTLE_INTERVAL_MS = 320;
const SEARCH_RESULT_STABLE_ROUNDS = 2;
const SEARCH_RESULT_MIN_WAIT_MS = 650;
const SEARCH_RESULT_UNCHANGED_GRACE_MS = 5200;

const FILTER_GROUPS = {
  sortBasis: {
    stateType: 'sort_type',
    title: '排序依据',
    currentLabel: '沿用当前排序',
    defaultValue: 'current',
    options: [
      { value: 'current', label: '沿用当前' },
      { value: 'general', label: '综合', aliases: ['general'] },
      { value: 'latest', label: '最新', aliases: ['latest', 'time_descending', 'latest_descending', 'create_time_descending'] },
      { value: 'most_liked', label: '最多点赞', aliases: ['likes', 'liked', 'like', 'like_descending', 'likes_descending', 'liked_descending'] },
      { value: 'most_commented', label: '最多评论', aliases: ['comments', 'commented', 'comment', 'comment_descending', 'comments_descending'] },
      { value: 'most_collected', label: '最多收藏', aliases: ['collects', 'collected', 'collect', 'collect_descending', 'collection_descending', 'collected_descending'] },
    ],
  },
  noteType: {
    stateType: 'filter_note_type',
    title: '笔记类型',
    currentLabel: '沿用当前类型',
    defaultValue: 'current',
    options: [
      { value: 'current', label: '沿用当前' },
      { value: 'all', label: '不限', aliases: ['all', '全部'] },
      { value: 'video', label: '视频', aliases: ['video'] },
      { value: 'image', label: '图文', aliases: ['image', 'photo', 'normal'] },
    ],
  },
  publishTime: {
    stateType: 'filter_note_time',
    title: '发布时间',
    currentLabel: '沿用当前时间',
    defaultValue: 'current',
    options: [
      { value: 'current', label: '沿用当前' },
      { value: 'all', label: '不限', aliases: ['all', '全部'] },
      { value: 'one_day', label: '一天内', aliases: ['1天内', '一天', 'day', 'one_day'] },
      { value: 'one_week', label: '一周内', aliases: ['7天内', '一周', 'week', 'one_week'] },
      { value: 'half_year', label: '半年内', aliases: ['6个月内', '半年', 'half_year'] },
    ],
  },
};

const DEFAULTS = Object.fromEntries(
  Object.entries(FILTER_GROUPS).map(([key, group]) => [key, group.defaultValue]),
);

export const XHS_SEARCH_FILTERS = {
  defaults: DEFAULTS,
  groups: FILTER_GROUPS,
};

function normalizeText(value = '') {
  return String(value || '').trim().replace(/\s+/g, '');
}

function toTextList(value) {
  if (Array.isArray(value)) {
    return value.map((item) => String(item || '').trim()).filter(Boolean);
  }
  const text = String(value || '').trim();
  return text ? [text] : [];
}

function unwrapState(value) {
  if (!value || typeof value !== 'object') return value;
  if (value.__v_isRef === true) return unwrapState(value._rawValue ?? value._value ?? value.value);
  if ('value' in value && Object.keys(value).length <= 2) return unwrapState(value.value);
  return value;
}

function findOption(groupKey, value) {
  const group = FILTER_GROUPS[groupKey];
  if (!group) return null;
  const normalizedValue = normalizeText(value).toLowerCase();
  return group.options.find((option) => option.value === normalizedValue)
    || group.options.find((option) => {
      const labels = [option.label, ...(option.aliases || [])].map((item) => normalizeText(item).toLowerCase());
      return labels.includes(normalizedValue);
    })
    || null;
}

function resolveOptionByLabel(groupKey, labels = []) {
  for (const label of labels) {
    const option = findOption(groupKey, label);
    if (option && option.value !== 'current') return option;
  }
  return null;
}

function getGroupLabel(groupKey, value) {
  const option = findOption(groupKey, value) || findOption(groupKey, FILTER_GROUPS[groupKey]?.defaultValue);
  return option?.label || '';
}

export function normalizeXhsSearchFilters(filters = {}) {
  return Object.fromEntries(
    Object.keys(FILTER_GROUPS).map((groupKey) => {
      const option = findOption(groupKey, filters?.[groupKey]);
      return [groupKey, option?.value || FILTER_GROUPS[groupKey].defaultValue];
    }),
  );
}

export function summarizeXhsSearchFilters(filters = {}) {
  const normalized = normalizeXhsSearchFilters(filters);
  const labels = Object.entries(normalized)
    .filter(([, value]) => value !== 'current')
    .map(([groupKey, value]) => getGroupLabel(groupKey, value))
    .filter(Boolean);
  return labels.length > 0 ? labels.join(' · ') : '沿用当前筛选';
}

export function hasExplicitXhsSearchFilters(filters = {}) {
  return Object.values(normalizeXhsSearchFilters(filters)).some((value) => value !== 'current');
}

export function readCurrentXhsSearchFilterSnapshot(win = globalThis.window) {
  const state = unwrapState(win?.__INITIAL_STATE__) || {};
  const searchState = unwrapState(state.search) || {};
  const filterParams = unwrapState(searchState.filterParams) || [];
  const raw = {};
  const result = {};
  const labels = {};

  if (Array.isArray(filterParams)) {
    Object.entries(FILTER_GROUPS).forEach(([groupKey, group]) => {
      const item = filterParams.find((entry) => String(entry?.type || '').trim() === group.stateType);
      const tags = toTextList(unwrapState(item?.tags));
      raw[group.stateType] = tags;
      const option = resolveOptionByLabel(groupKey, tags);
      result[groupKey] = option?.value || group.defaultValue;
      labels[groupKey] = option?.label || group.currentLabel;
    });
  }

  return {
    ...normalizeXhsSearchFilters(result),
    labels,
    raw,
  };
}

function isSearchPage(win = globalThis.window) {
  const href = String(win?.location?.href || '');
  return SEARCH_PAGE_RE.test(href);
}

function canClick(element) {
  return element && typeof element.click === 'function';
}

function isAlreadySelected(element) {
  const attrSelected = ['aria-selected', 'aria-pressed', 'data-active', 'data-selected']
    .some((name) => String(element.getAttribute?.(name) || '').toLowerCase() === 'true');
  const className = String(element.className || '');
  return attrSelected || /\b(active|selected)\b/i.test(className);
}

function collectCandidateElements(root) {
  if (!root || typeof root.querySelectorAll !== 'function') return [];
  return Array.from(root.querySelectorAll('button,[role="button"],div,span'));
}

function getGroupByTitle(groupTitle) {
  const normalizedGroup = normalizeText(groupTitle);
  return Object.values(FILTER_GROUPS).find((group) => normalizeText(group.title) === normalizedGroup) || null;
}

function countExactDescendants(element, normalizedLabel) {
  if (!element || typeof element.querySelectorAll !== 'function') return 0;
  return Array.from(element.querySelectorAll('button,[role="button"],div,span'))
    .filter((child) => child !== element && normalizeText(child.textContent) === normalizedLabel)
    .length;
}

function getElementDepth(element) {
  let depth = 0;
  let node = element?.parentElement || null;
  while (node) {
    depth += 1;
    node = node.parentElement;
  }
  return depth;
}

function getBackgroundScore(element) {
  try {
    const style = globalThis.getComputedStyle?.(element);
    const background = String(style?.backgroundColor || '').toLowerCase();
    if (background && background !== 'transparent' && background !== 'rgba(0, 0, 0, 0)') return 8;
  } catch {
    // Ignore style reads in tests or restricted pages.
  }
  return 0;
}

function scoreClickableCandidate(element, normalizedLabel) {
  const tagName = String(element?.tagName || '').toLowerCase();
  const className = String(element?.className || '');
  let score = getElementDepth(element) + getBackgroundScore(element);
  if (tagName === 'button') score += 40;
  if (String(element?.getAttribute?.('role') || '').toLowerCase() === 'button') score += 35;
  if (/\btags?\b/i.test(className)) score += 30;
  if (tagName === 'div') score += 10;
  if (tagName === 'span') score -= 8;
  score -= countExactDescendants(element, normalizedLabel) * 12;
  return score;
}

function findFilterGroupContainer(doc, groupTitle) {
  const group = getGroupByTitle(groupTitle);
  const normalizedGroup = normalizeText(groupTitle);
  const optionLabels = (group?.options || [])
    .filter((option) => option.value !== 'current')
    .map((option) => normalizeText(option.label))
    .filter(Boolean);
  const titleElements = collectCandidateElements(doc)
    .filter((element) => normalizeText(element.textContent) === normalizedGroup);

  for (const titleElement of titleElements) {
    let node = titleElement.parentElement;
    for (let i = 0; i < 8 && node; i += 1) {
      const text = normalizeText(node.textContent);
      const optionHits = optionLabels.filter((label) => text.includes(label)).length;
      if (text.includes(normalizedGroup) && optionHits >= Math.min(2, optionLabels.length)) {
        return node;
      }
      node = node.parentElement;
    }
  }

  return null;
}

function getContextText(element, groupTitle, optionLabel) {
  let node = element;
  for (let i = 0; i < 8 && node; i += 1) {
    const text = normalizeText(node.textContent);
    if (text.includes(normalizeText(groupTitle)) && text.includes(normalizeText(optionLabel))) {
      return text;
    }
    node = node.parentElement;
  }

  const closest = element.closest?.('section,div,[class*="filter"],[class*="Filter"],[class*="popover"],[class*="panel"]');
  return normalizeText(closest?.textContent || element.textContent || '');
}

function findClickableCandidatesByGroupAndLabel(doc, groupTitle, optionLabel) {
  const normalizedGroup = normalizeText(groupTitle);
  const normalizedLabel = normalizeText(optionLabel);
  const groupContainer = findFilterGroupContainer(doc, groupTitle);
  const pool = groupContainer ? collectCandidateElements(groupContainer) : collectCandidateElements(doc);
  return pool.filter((element) => {
    const ownText = normalizeText(element.textContent);
    if (ownText !== normalizedLabel) return false;
    const contextText = groupContainer
      ? normalizeText(groupContainer.textContent)
      : getContextText(element, groupTitle, optionLabel);
    return contextText.includes(normalizedGroup) && contextText.includes(normalizedLabel) && canClick(element);
  }).sort((a, b) => scoreClickableCandidate(b, normalizedLabel) - scoreClickableCandidate(a, normalizedLabel));
}

function findClickableByGroupAndLabel(doc, groupTitle, optionLabel) {
  return findClickableCandidatesByGroupAndLabel(doc, groupTitle, optionLabel)[0] || null;
}

function isOptionSelected(doc, groupTitle, optionLabel) {
  return findClickableCandidatesByGroupAndLabel(doc, groupTitle, optionLabel)
    .some((element) => isAlreadySelected(element));
}

function hasFilterPanel(doc) {
  const text = collectCandidateElements(doc)
    .map((element) => {
      const ownText = normalizeText(element.textContent);
      const contextText = normalizeText(element.closest?.('section,div,[class*="filter"],[class*="Filter"],[class*="popover"],[class*="panel"]')?.textContent || '');
      return `${ownText} ${contextText}`;
    })
    .join(' ');
  return ['排序依据', '笔记类型', '发布时间'].every((label) => text.includes(normalizeText(label)));
}

function findFilterPanelToggle(doc) {
  return collectCandidateElements(doc).find((element) => {
    const text = normalizeText(element.textContent);
    return canClick(element) && (text === '筛选' || text === '已筛选' || (text.includes('已筛选') && text.length <= 8));
  }) || null;
}

function waitFor(ms = 0) {
  const safeMs = Math.max(0, Number(ms || 0) || 0);
  if (safeMs === 0) return Promise.resolve();
  return new Promise((resolve) => setTimeout(resolve, safeMs));
}

function extractSearchNoteId(url = '') {
  const text = String(url || '').trim();
  if (!text) return '';
  const match = text.match(/\/(?:explore|search_result|discovery\/item)\/([^/?#]+)/i)
    || text.match(/\/user\/profile\/[^/?#]+\/([^/?#]+)/i);
  return String(match?.[1] || '').trim();
}

function getStableText(value = '') {
  return String(value || '').trim().replace(/\s+/g, ' ');
}

function getSearchFeedContainer(doc = globalThis.document) {
  if (!doc || typeof doc.querySelector !== 'function') return null;
  return doc.querySelector(FEED_CONTAINER_SELECTOR);
}

function readSectionText(section, selector) {
  if (!section || typeof section.querySelector !== 'function') return '';
  return getStableText(section.querySelector(selector)?.textContent || '');
}

function readSearchNoteFromSection(section) {
  if (!section || typeof section.querySelector !== 'function') return null;
  const coverLink = section.querySelector('a.cover')
    || section.querySelector('a[href*="/search_result/"],a[href*="/explore/"],a[href*="/discovery/item/"]');
  const href = String(coverLink?.getAttribute?.('href') || coverLink?.href || '').trim();
  const noteId = extractSearchNoteId(href);
  if (!noteId) return null;

  const title = readSectionText(section, '.title')
    || readSectionText(section, '.footer span')
    || getStableText(section.textContent || '').slice(0, 80);
  const likes = readSectionText(section, '.like-wrapper .count');
  return { noteId, title, likes };
}

function isElementLikelyVisible(element) {
  if (!element) return false;
  if (element.hidden) return false;
  if (String(element.getAttribute?.('aria-hidden') || '').toLowerCase() === 'true') return false;

  let node = element;
  for (let i = 0; i < 5 && node; i += 1) {
    try {
      const computedStyle = globalThis.getComputedStyle?.(node);
      const display = String(computedStyle?.display || '').toLowerCase();
      const visibility = String(computedStyle?.visibility || '').toLowerCase();
      const opacity = Number.parseFloat(String(computedStyle?.opacity || ''));
      if (display === 'none' || visibility === 'hidden' || opacity === 0) return false;
    } catch {
      // Some test doubles and extension contexts can reject style reads.
    }
    const inlineStyle = String(node.getAttribute?.('style') || '').toLowerCase();
    if (/display\s*:\s*none|visibility\s*:\s*hidden|opacity\s*:\s*0/.test(inlineStyle)) return false;
    node = node.parentElement;
  }

  if (typeof element.getBoundingClientRect !== 'function' && typeof element.getClientRects !== 'function') {
    return true;
  }

  const rect = element.getBoundingClientRect?.();
  const hasRectSize = Boolean(rect && (rect.width > 0 || rect.height > 0));
  const hasClientRect = Boolean(element.getClientRects?.().length);
  const hasOffsetSize = Boolean((element.offsetWidth || 0) > 0 || (element.offsetHeight || 0) > 0);
  return hasRectSize || hasClientRect || hasOffsetSize;
}

function hasTransientLoadingSignal(doc = globalThis.document) {
  if (!doc || typeof doc.querySelectorAll !== 'function') return false;
  const loadingNodes = Array.from(doc.querySelectorAll(
    '[class*="loading"],[class*="Loading"],[class*="skeleton"],[class*="Skeleton"],[class*="spin"],[class*="Spin"]',
  ));
  return loadingNodes.some((node) => {
    if (!isElementLikelyVisible(node)) return false;
    const text = normalizeText(node.textContent || '');
    const className = String(node.className || '');
    return text.includes('加载') || /loading|skeleton|spin/i.test(className);
  });
}

export function readXhsSearchResultFeedSnapshot(doc = globalThis.document) {
  const container = getSearchFeedContainer(doc);
  if (!container || typeof container.querySelectorAll !== 'function') {
    return {
      hasFeedContainer: false,
      count: 0,
      notes: [],
      signature: '',
      isLoading: hasTransientLoadingSignal(doc),
    };
  }

  const notes = Array.from(container.querySelectorAll('section'))
    .map((section) => readSearchNoteFromSection(section))
    .filter(Boolean);
  const signatureParts = notes
    .slice(0, 10)
    .map((note) => [note.noteId, note.title, note.likes].filter(Boolean).join(':'));

  return {
    hasFeedContainer: true,
    count: notes.length,
    notes,
    signature: signatureParts.join('|'),
    isLoading: hasTransientLoadingSignal(doc),
  };
}

function hasUsableFeedSnapshot(snapshot) {
  return Boolean(snapshot?.hasFeedContainer && snapshot.count > 0 && snapshot.signature && !snapshot.isLoading);
}

function didFeedChange(previousSnapshot, currentSnapshot) {
  if (!previousSnapshot?.signature || previousSnapshot.count === 0) return true;
  return previousSnapshot.signature !== currentSnapshot.signature || previousSnapshot.count !== currentSnapshot.count;
}

export async function waitForXhsSearchResultsSettled({
  document: doc = globalThis.document,
  previousSnapshot = null,
  timeoutMs = SEARCH_RESULT_SETTLE_TIMEOUT_MS,
  intervalMs = SEARCH_RESULT_SETTLE_INTERVAL_MS,
  stableRounds = SEARCH_RESULT_STABLE_ROUNDS,
  minWaitMs = SEARCH_RESULT_MIN_WAIT_MS,
  unchangedGraceMs = SEARCH_RESULT_UNCHANGED_GRACE_MS,
} = {}) {
  const baseline = previousSnapshot || readXhsSearchResultFeedSnapshot(doc);
  if (!baseline.hasFeedContainer) {
    return {
      settled: false,
      skipped: true,
      reason: 'no_baseline_feed',
      previousSnapshot: baseline,
      snapshot: baseline,
    };
  }

  const startedAt = Date.now();
  let lastSignature = '';
  let lastCount = -1;
  let stableCount = 0;
  let lastSnapshot = baseline;

  while (Date.now() - startedAt < timeoutMs) {
    const elapsed = Date.now() - startedAt;
    const snapshot = readXhsSearchResultFeedSnapshot(doc);
    lastSnapshot = snapshot;
    const ready = hasUsableFeedSnapshot(snapshot);
    const changed = ready && didFeedChange(baseline, snapshot);
    const unchangedAllowed = ready && elapsed >= unchangedGraceMs;
    const signatureStable = snapshot.signature === lastSignature && snapshot.count === lastCount;

    if (ready && elapsed >= minWaitMs && signatureStable && (changed || unchangedAllowed)) {
      stableCount += 1;
      if (stableCount >= stableRounds) {
        return {
          settled: true,
          changed,
          previousSnapshot: baseline,
          snapshot,
        };
      }
    } else if (!signatureStable) {
      stableCount = ready && elapsed >= minWaitMs && (changed || unchangedAllowed) ? 1 : 0;
    } else {
      stableCount = 0;
    }

    lastSignature = snapshot.signature;
    lastCount = snapshot.count;
    await waitFor(intervalMs);
  }

  throw new Error('小红书筛选后页面列表没有稳定刷新，请刷新搜索页后重试。');
}

async function ensureFilterPanelOpen(doc, waitMs) {
  if (hasFilterPanel(doc)) return;
  const toggle = findFilterPanelToggle(doc);
  if (!toggle) {
    throw new Error('没有找到小红书搜索页的筛选入口，请先确认当前在搜索结果页。');
  }
  toggle.click();
  await waitFor(waitMs);
  if (!hasFilterPanel(doc)) {
    throw new Error('筛选面板没有打开，请先在页面上确认小红书筛选入口可用。');
  }
}

export async function applyXhsSearchFilters(filters = {}, {
  document: doc = globalThis.document,
  win = globalThis.window,
  waitMs = 500,
  waitForResults = true,
  resultSettleTimeoutMs = SEARCH_RESULT_SETTLE_TIMEOUT_MS,
  onResultsWait,
} = {}) {
  const normalized = normalizeXhsSearchFilters(filters);
  const explicitEntries = Object.entries(normalized)
    .filter(([, value]) => value !== 'current');

  if (explicitEntries.length === 0) {
    return {
      applied: false,
      reason: 'no_explicit_filter',
      filters: normalized,
      snapshot: readCurrentXhsSearchFilterSnapshot(win),
    };
  }

  if (!isSearchPage(win)) {
    throw new Error('小红书筛选只能在搜索结果页使用，请先打开搜索结果页再开始采集。');
  }

  const previousResultsSnapshot = readXhsSearchResultFeedSnapshot(doc);
  await ensureFilterPanelOpen(doc, waitMs);

  let changedFilterCount = 0;
  for (const [groupKey, value] of explicitEntries) {
    const group = FILTER_GROUPS[groupKey];
    const label = getGroupLabel(groupKey, value);
    const targets = findClickableCandidatesByGroupAndLabel(doc, group.title, label);
    if (targets.length === 0) {
      throw new Error(`没有找到小红书筛选项：${group.title} / ${label}`);
    }
    if (isOptionSelected(doc, group.title, label)) {
      continue;
    }
    for (const target of targets) {
      target.click();
      await waitFor(waitMs);
      if (isOptionSelected(doc, group.title, label)) {
        changedFilterCount += 1;
        break;
      }
    }
    if (!isOptionSelected(doc, group.title, label)) {
      throw new Error(`小红书筛选项点击后没有生效：${group.title} / ${label}`);
    }
  }

  let resultsSettle = null;
  if (waitForResults && changedFilterCount > 0) {
    onResultsWait?.();
    resultsSettle = await waitForXhsSearchResultsSettled({
      document: doc,
      previousSnapshot: previousResultsSnapshot,
      timeoutMs: resultSettleTimeoutMs,
    });
  }

  return {
    applied: true,
    filters: normalized,
    summary: summarizeXhsSearchFilters(normalized),
    snapshot: readCurrentXhsSearchFilterSnapshot(win),
    resultsSettle,
  };
}
