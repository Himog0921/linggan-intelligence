/**
 * 小红书搜索结果列表探针 v2。
 *
 * 在搜索结果页执行。只统计当前已加载的卡片结构，不滚动、不发请求，也不输出内容、作者、链接或稳定标识本身。
 */
(function probeXhsSearchResultsV2() {
  const cards = [...document.querySelectorAll('section')].filter((card) => card.querySelector('a.cover, a[href*="/explore/"], a[href*="/discovery/item/"]'));
  const stableIds = cards.map((card) => {
    const link = card.querySelector('a.cover, a[href*="/explore/"], a[href*="/discovery/item/"]');
    const href = link?.getAttribute('href') || '';
    return href.match(/\/(?:explore|discovery\/item)\/([^/?#]+)/)?.[1] || '';
  }).filter(Boolean);
  const countWith = (selector) => cards.filter((card) => Boolean(card.querySelector(selector))).length;
  const uniqueStableIdCount = new Set(stableIds).size;

  const output = {
    probeVersion: '2.0',
    probeType: 'xhs-search-results',
    sanitized: true,
    loaded: {
      candidateCardCount: cards.length,
      cardsWithStableId: stableIds.length,
      uniqueStableIdCount,
      duplicateStableIdCount: Math.max(stableIds.length - uniqueStableIdCount, 0),
    },
    fieldPresence: {
      cover: countWith('img, video'),
      titleOrSummary: countWith('.footer span, [class*="title"], [class*="desc"]'),
      author: countWith('[class*="author"], [class*="user"]'),
      interaction: countWith('.like-wrapper, [class*="like"], [class*="interact"]'),
      videoMarker: countWith('.play-icon, [class*="video"], video'),
      tagOrCorner: countWith('[class*="tag"], [class*="corner"]'),
    },
    surface: {
      hasFeedContainer: Boolean(document.querySelector('.feeds-container, [class*="feeds"]')),
      visibleCardCount: cards.filter((card) => card.getClientRects().length > 0).length,
      endMarkerCount: document.querySelectorAll('[class*="end"], [class*="empty"], [class*="no-more"]').length,
    },
    limitations: [
      '仅代表执行时已加载的搜索卡片，不代表搜索总量。',
      '不输出卡片标题、作者、互动数、链接或内容标识。',
      '分页、滚动和停止原因须由执行过程的独立回执补充；endMarkerCount 不等于已到平台末尾。',
    ],
  };

  window.__XHS_SEARCH_RESULTS_PROBE_V2__ = output;
  console.info('[probe-xhs-search-results-v2]', output);
  return output;
})();
