// The active reader is intentionally derived from the legacy XHS search-surface rule:
// a visible card is a rendered `section` with an `a.cover` that contains a stable note URL.
// It reads the already-rendered surface once; it never scrolls, opens a detail page, injects
// page-context network hooks, or uses account/Cookie/hidden application state.

const MAXIMUM_QUOTA = 20;
const CANARY_QUERY = 'ADHD';

function text(value) {
  return String(value || '').replace(/\s+/g, ' ').trim();
}

function isRendered(element, win = globalThis.window) {
  if (!element) return false;
  if (element.hidden || String(element.getAttribute?.('aria-hidden') || '') === 'true') return false;
  const display = String(win?.getComputedStyle?.(element)?.display || '');
  const visibility = String(win?.getComputedStyle?.(element)?.visibility || '');
  if (display === 'none' || visibility === 'hidden') return false;
  return true;
}

function stableNoteId(href = '') {
  const path = String(href || '').split('?')[0];
  const match = path.match(/\/(?:explore|discovery\/item)\/([a-z0-9_-]+)/i);
  return match?.[1] || '';
}

function absoluteUrl(value, baseUrl) {
  try {
    return new URL(String(value || ''), baseUrl).toString();
  } catch {
    return '';
  }
}

function observedImageUrl(card) {
  const image = card.querySelector?.('a.cover img, a.cover picture img, a.cover source, img');
  const candidate = image?.currentSrc || image?.getAttribute?.('src') || image?.getAttribute?.('data-src') || '';
  const value = String(candidate || '').trim();
  return /^https?:\/\//i.test(value) ? value : '';
}

function cardTitle(card) {
  return text(
    card.querySelector?.('.footer span')?.textContent
    || card.querySelector?.('.title')?.textContent
    || card.querySelector?.('[data-testid="note-title"]')?.textContent,
  );
}

function creatorName(card) {
  return text(
    card.querySelector?.('.author-wrapper .name')?.textContent
    || card.querySelector?.('.author .name')?.textContent
    || card.querySelector?.('[data-testid="creator-name"]')?.textContent,
  );
}

function publishedSourceText(card) {
  const time = card.querySelector?.('time');
  return text(
    time?.getAttribute?.('datetime')
    || time?.textContent
    || card.querySelector?.('.time')?.textContent
    || card.querySelector?.('[data-testid="published-at"]')?.textContent,
  );
}

function cardRect(card) {
  const rect = card.getBoundingClientRect?.() || { top: 0, left: 0 };
  return { top: Number(rect.top || 0), left: Number(rect.left || 0) };
}

function sortSurfaceCards(cards) {
  return cards.sort((left, right) => {
    const rowDifference = Math.abs(left._top - right._top);
    return rowDifference < 50 ? left._left - right._left : left._top - right._top;
  });
}

function selected(element) {
  if (!element) return false;
  const attributes = ['aria-current', 'aria-selected', 'aria-pressed', 'data-active', 'data-selected'];
  if (attributes.some((name) => String(element.getAttribute?.(name) || '').toLowerCase() === 'true')) return true;
  return /(?:^|\s)(?:active|selected|is-active|is-selected)(?:\s|$)/i.test(String(element.className || ''));
}

export function isComprehensiveSortVisible(doc = globalThis.document, win = globalThis.window) {
  if (!doc?.querySelectorAll) return false;
  return [...doc.querySelectorAll('button,[role="button"],div,span')]
    .some((element) => isRendered(element, win) && text(element.textContent) === '综合' && (selected(element) || selected(element.parentElement)));
}

export function validateCurrentXhsSearchSurface({ url = globalThis.location?.href || '', doc = globalThis.document, win = globalThis.window } = {}) {
  let parsed;
  try { parsed = new URL(url); } catch { return { ok: false, code: 'invalid_page_url', message: '当前页面地址无法识别，请打开小红书 ADHD 搜索结果页后重试。' }; }
  if (!/(^|\.)xiaohongshu\.com$/i.test(parsed.hostname) || parsed.pathname !== '/search_result') {
    return { ok: false, code: 'not_xhs_search_results', message: '请先打开小红书的 ADHD 搜索结果页；本次不会在其他页面采集。' };
  }
  if (text(parsed.searchParams.get('keyword')) !== CANARY_QUERY) {
    return { ok: false, code: 'unexpected_query', message: '首轮只接收关键词“ADHD”的当前搜索结果页；本次没有提交。' };
  }
  if (!isComprehensiveSortVisible(doc, win)) {
    return { ok: false, code: 'comprehensive_sort_not_confirmed', message: '未能从当前页面确认“综合”排序已选中；为避免误记排序，本次没有提交。' };
  }
  return { ok: true, parsed };
}

export function readCurrentVisibleXhsSearchCards({ doc = globalThis.document, win = globalThis.window, url = globalThis.location?.href || '' } = {}) {
  const sections = [...(doc?.querySelectorAll?.('.feeds-container section, #exploreFeeds section') || [])]
    .filter((section) => isRendered(section, win));
  const candidates = sections
    .filter((section) => section.querySelector?.('a.cover'))
    .map((section) => {
      const rect = cardRect(section);
      return { section, _top: rect.top + Number(win?.scrollY || 0), _left: rect.left };
    });
  sortSurfaceCards(candidates);
  const seen = new Set();
  const attempted = [];
  let failedCards = 0;
  let inspectedCandidates = 0;

  for (const candidate of candidates.slice(0, MAXIMUM_QUOTA)) {
    const { section } = candidate;
    const cover = section.querySelector?.('a.cover');
    inspectedCandidates += 1;
    const href = absoluteUrl(cover.getAttribute?.('href') || '', url);
    const platformContentId = stableNoteId(href);
    if (!platformContentId || seen.has(platformContentId)) {
      failedCards += 1;
      continue;
    }
    seen.add(platformContentId);
    attempted.push({
      platformContentId,
      title: cardTitle(section) || undefined,
      creatorDisplayName: creatorName(section) || undefined,
      publishedAtSourceText: publishedSourceText(section) || undefined,
      coverCandidate: observedImageUrl(section) ? { observedExternalUri: observedImageUrl(section) } : undefined,
      _top: candidate._top,
      _left: candidate._left,
    });
  }

  const allCandidateCount = candidates.length;
  const ordered = sortSurfaceCards(attempted).map((card, index) => ({
    content: Object.fromEntries(Object.entries({
      platformContentId: card.platformContentId,
      title: card.title,
      creatorDisplayName: card.creatorDisplayName,
      publishedAtSourceText: card.publishedAtSourceText,
      coverCandidate: card.coverCandidate,
    }).filter(([, value]) => value !== undefined)),
    occurrence: { query: CANARY_QUERY, sort: 'comprehensive', observedAt: '', resultPosition: index + 1 },
  }));

  return {
    cards: ordered,
    // These counts are only about the currently rendered search surface. They never describe
    // the total platform result set or an inferred number of missing notes.
    surfaceCardCandidates: allCandidateCount,
    discoveredCards: attempted.length + failedCards,
    emittedCards: ordered.length,
    failedCards,
    notAttemptedCards: Math.max(0, allCandidateCount - inspectedCandidates),
  };
}

export function buildCurrentVisibleXhsDiscoveryPackage({ now = () => new Date(), doc = globalThis.document, win = globalThis.window, url = globalThis.location?.href || '' } = {}) {
  const page = validateCurrentXhsSearchSurface({ url, doc, win });
  if (!page.ok) return page;
  const observedAt = now().toISOString();
  const reading = readCurrentVisibleXhsSearchCards({ doc, win, url });
  if (reading.emittedCards === 0) {
    return { ok: false, code: 'no_eligible_visible_cards', message: '当前搜索结果面没有可稳定识别的卡片；本次没有提交。' };
  }
  const stoppedReason = reading.emittedCards === MAXIMUM_QUOTA ? 'quota_reached' : 'unknown';
  const cards = reading.cards.map((card) => ({
    ...card,
    occurrence: { ...card.occurrence, observedAt },
  }));
  return {
    ok: true,
    package: {
      contractVersion: 'xhs.discovery.visible-card.v1',
      acquisitionSpec: {
        platform: 'xhs', query: CANARY_QUERY, sort: 'comprehensive',
        target: { basis: 'maximum_quota', unit: 'visible_search_card', maximumQuota: MAXIMUM_QUOTA },
      },
      observedAt,
      coverage: {
        unit: 'visible_search_card',
        visibleCards: cards.length,
        discoveredCards: reading.discoveredCards,
        emittedCards: reading.emittedCards,
        failedCards: reading.failedCards,
        notAttemptedCards: reading.notAttemptedCards,
        stoppedReason,
      },
      cards,
    },
    localCoverage: {
      renderedCardCandidates: reading.surfaceCardCandidates,
      discoveredCards: reading.discoveredCards,
      emittedCards: reading.emittedCards,
      failedCards: reading.failedCards,
      notAttemptedCards: reading.notAttemptedCards,
      stoppedReason,
    },
  };
}
