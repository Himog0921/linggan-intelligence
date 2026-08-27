/**
 * 抖音搜索页全量探查 v1.0（2026-04-18）
 *
 * 在抖音搜索结果页（综合/视频/用户等 Tab）DevTools Console 执行
 *
 * 覆盖：
 * 1. 页面类型识别（综合搜索 vs 视频搜索 vs 用户搜索）
 * 2. DOM 卡片结构验证（视频链接、标题、点赞数）
 * 3. RENDER_DATA / __INITIAL_STATE__ 可用性
 * 4. API 拦截缓存状态
 * 5. 搜索结果 data-e2e 属性分布
 */
(function probeDouyinSearchFull() {
  const PROBE_VERSION = '1.0';

  function short(v, max = 160) {
    const t = String(v ?? '');
    return t.length > max ? t.slice(0, max) + '...' : t;
  }

  function selectorCheck(sel) {
    const el = document.querySelector(sel);
    if (!el) return { found: false, text: '', tag: '' };
    return {
      found: true,
      text: short((el.textContent || '').trim()),
      tag: el.tagName,
      classes: short(el.className || ''),
    };
  }

  function selectorCountCheck(sel) {
    const els = document.querySelectorAll(sel);
    return {
      count: els.length,
      samples: Array.from(els).slice(0, 5).map(el => ({
        tag: el.tagName,
        text: short((el.textContent || '').trim(), 60),
        href: el.getAttribute('href') ? short(el.getAttribute('href'), 100) : undefined,
        dataE2e: el.getAttribute('data-e2e') || undefined,
      })),
    };
  }

  // ============ 1. 页面类型识别 ============
  const url = new URL(location.href);
  const searchType = url.searchParams.get('type') || 'general';
  const keyword = url.searchParams.get('keyword') || '';
  const isSearchPage = location.pathname.includes('/search');

  // ============ 2. 数据源可用性 ============
  function parseJsonCandidates(raw) {
    if (!raw) return null;
    const candidates = [String(raw)];
    try {
      const decoded = decodeURIComponent(String(raw));
      if (decoded !== raw) candidates.push(decoded);
    } catch { /* ignore */ }
    for (const item of candidates) {
      try { return JSON.parse(item); } catch { /* next */ }
    }
    return null;
  }

  const renderDataEl = document.getElementById('RENDER_DATA');
  const renderData = renderDataEl ? parseJsonCandidates(renderDataEl.innerHTML) : null;
  const initialState = window.__INITIAL_STATE__ || null;
  const apiCache = window.__lgboom_dy_video_data || null;

  const dataSources = {
    hasRenderData: Boolean(renderData),
    renderDataKeys: renderData ? Object.keys(renderData).slice(0, 10) : [],
    hasInitialState: Boolean(initialState),
    initialStateKeys: initialState ? Object.keys(initialState).slice(0, 10) : [],
    hasApiCache: Boolean(apiCache),
    apiCacheSize: apiCache ? Object.keys(apiCache).length : 0,
    apiCacheSampleIds: apiCache ? Object.keys(apiCache).slice(0, 5) : [],
  };

  // ============ 3. DOM 结构探查 ============

  // 视频链接
  const videoLinks = selectorCountCheck('a[href*="/video/"]');
  const liVideoLinks = selectorCountCheck('li a[href*="/video/"]');

  // 所有 data-e2e 属性分布
  const dataE2eMap = {};
  document.querySelectorAll('[data-e2e]').forEach(el => {
    const val = el.getAttribute('data-e2e');
    dataE2eMap[val] = (dataE2eMap[val] || 0) + 1;
  });

  // 搜索结果区域探查
  const searchResultArea = {
    searchResultList: selectorCountCheck('[data-e2e="search-result-list"]'),
    searchResultCard: selectorCountCheck('[data-e2e="search-result-card"]'),
    scrollList: selectorCountCheck('[data-e2e="scroll-list"]'),
  };

  // 通用列表容器
  const listContainers = {
    ulElements: selectorCountCheck('ul'),
    liElements: selectorCountCheck('li'),
    mainContent: selectorCheck('main'),
    searchSideNav: selectorCheck('[data-e2e="search-side-nav"]'),
  };

  // 卡片结构探查：尝试多种选择器找视频卡片
  const cardCandidates = {
    // 视频搜索页已知有效
    liWithVideoLink: selectorCountCheck('li:has(a[href*="/video/"])'),
    // 其他可能的卡片选择器
    divWithVideoLink: selectorCountCheck('div:has(> a[href*="/video/"])'),
    // 搜索结果卡片
    searchCards: selectorCountCheck('[class*="search"][class*="card"], [class*="search"][class*="item"], [class*="result"][class*="item"]'),
  };

  // ============ 4. 综合搜索页特殊探查 ============
  // 找到所有可能包含视频信息的结构
  const comprehensiveSearch = {};
  if (searchType === 'general' || !isSearchPage) {
    // 尝试找所有包含 aweme/video 字样的属性
    const allWithAweme = [];
    document.querySelectorAll('*').forEach(el => {
      for (const attr of el.attributes) {
        if (/aweme|video/i.test(attr.value) && allWithAweme.length < 10) {
          allWithAweme.push({
            tag: el.tagName,
            attr: attr.name,
            value: short(attr.value, 80),
            classes: short(el.className, 60),
          });
        }
      }
    });
    comprehensiveSearch.elementsWithAwemeAttr = allWithAweme;

    // 查找可点击的视频入口
    const clickableEntries = [];
    document.querySelectorAll('a, [role="link"], [onclick]').forEach((el, i) => {
      if (i > 200) return;
      const href = el.getAttribute('href') || '';
      const text = (el.textContent || '').trim();
      if (/video|aweme/i.test(href) && clickableEntries.length < 10) {
        clickableEntries.push({
          tag: el.tagName,
          href: short(href, 120),
          text: short(text, 60),
        });
      }
    });
    comprehensiveSearch.clickableVideoEntries = clickableEntries;
  }

  // ============ 5. 互动数据元素 ============
  const statsElements = {
    digg: selectorCountCheck('[data-e2e*="digg"], [data-e2e*="like"]'),
    comment: selectorCountCheck('[data-e2e*="comment"]'),
    share: selectorCountCheck('[data-e2e*="share"]'),
  };

  // ============ 诊断 ============
  const diagnosis = [];
  if (!isSearchPage) diagnosis.push('WARNING: 当前不在搜索页面');
  if (searchType === 'general' && videoLinks.count === 0) {
    diagnosis.push('CONFIRMED: 综合搜索页无 a[href*="/video/"] 链接——DOM 发现策略不可用，需走 API');
  }
  if (searchType === 'video' && videoLinks.count > 0) {
    diagnosis.push('OK: 视频搜索页有视频链接——DOM 发现策略可用');
  }
  if (searchType === 'video' && videoLinks.count === 0) {
    diagnosis.push('WARNING: 视频搜索页但未发现视频链接——可能页面未加载完');
  }
  if (Object.keys(dataE2eMap).length === 0) {
    diagnosis.push('WARNING: 页面无任何 data-e2e 属性——可能结构已大幅变化');
  }
  if (apiCache && Object.keys(apiCache).length > 0) {
    diagnosis.push('OK: API 拦截缓存有数据——插件注入脚本工作正常');
  }

  const output = {
    probeVersion: PROBE_VERSION,
    probeType: 'douyin-search-full',
    time: new Date().toISOString(),
    url: location.href,
    pageInfo: {
      isSearchPage,
      searchType,
      keyword,
      pathname: location.pathname,
    },
    dataSources,
    domStructure: {
      videoLinks,
      liVideoLinks,
      dataE2eDistribution: dataE2eMap,
      searchResultArea,
      listContainers,
      cardCandidates,
      statsElements,
    },
    comprehensiveSearch,
    diagnosis,
  };

  window.__DY_SEARCH_PROBE__ = output;
  console.group('[probe-douyin-search-full]');
  console.log('result:', output);
  console.log('json:', JSON.stringify(output, null, 2));
  console.groupEnd();
  return output;
})();
