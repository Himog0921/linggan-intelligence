/**
 * 小红书搜索上下文探针 v2。
 *
 * 在已经进入搜索结果的页面执行。只输出可见结构、数量与状态；不输出搜索词、联想词、卡片文本、链接、Cookie 或网络响应。
 */
(function probeXhsSearchContextV2() {
  const selectorCount = (selector) => document.querySelectorAll(selector).length;
  const anyVisible = (selectors) => selectors.some((selector) => {
    const element = document.querySelector(selector);
    return Boolean(element && element.getClientRects().length);
  });
  const pageKind = location.pathname.includes('/search_result') || location.pathname.includes('/search')
    ? 'search'
    : 'other';
  const inputs = [...document.querySelectorAll('input, textarea')];
  const searchInputs = inputs.filter((input) => /search|搜索/i.test(
    `${input.getAttribute('placeholder') || ''} ${input.getAttribute('aria-label') || ''}`,
  ));
  const activeControls = [...document.querySelectorAll(
    '[aria-selected="true"], [aria-pressed="true"], .active, .selected, .is-active, .is-selected',
  )];
  const suggestionSelectors = [
    '[class*="suggest"]',
    '[class*="sug-"]',
    '[class*="autocomplete"]',
    '[class*="recommend"]',
    '[class*="associate"]',
  ];
  const filterSelectors = ['[class*="filter"]', '[class*="sort"]', '[role="tablist"]'];

  const output = {
    probeVersion: '2.0',
    probeType: 'xhs-search-context',
    sanitized: true,
    page: { kind: pageKind, hasSearchInput: searchInputs.length > 0, searchInputCount: searchInputs.length },
    suggestions: {
      visible: anyVisible(suggestionSelectors),
      containerCount: suggestionSelectors.reduce((sum, selector) => sum + selectorCount(selector), 0),
      itemCount: selectorCount('[class*="suggest"] li, [class*="suggest"] [role="option"], [class*="sug-item"], [class*="autocomplete"] li, [class*="recommend"] li'),
    },
    filters: {
      groupCount: filterSelectors.reduce((sum, selector) => sum + selectorCount(selector), 0),
      activeControlCount: activeControls.length,
      tabCount: selectorCount('[role="tab"]'),
    },
    resultSurface: {
      hasFeedContainer: anyVisible(['.feeds-container', '#userPostedFeeds', '[class*="feeds"]']),
      sectionCount: selectorCount('section'),
    },
    limitations: [
      '不读取或输出搜索词、联想词和筛选标签文本。',
      '筛选组名称与业务含义需由人工页面观察和登记册共同确认。',
    ],
  };

  window.__XHS_SEARCH_CONTEXT_PROBE_V2__ = output;
  console.info('[probe-xhs-search-context-v2]', output);
  return output;
})();
