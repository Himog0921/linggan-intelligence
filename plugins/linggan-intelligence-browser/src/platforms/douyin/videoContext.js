export function createVideoContextHelpers({
  SEL,
  dedupeStrings,
  sanitizeVideoTitle,
  getCurrentCardHintId,
  getRenderVideoDetail,
  getRouterVideoData,
  getRenderVideoId,
  getUrlPathVideoId,
  getUrlVidParam,
  getUrlModalId,
  getApiVideoData,
  fetchDetailApiData,
  registerVideoAliases,
  hasUsableApiVideo,
  waitForElement,
  waitForContentSettle,
} = {}) {
  function isElementVisible(el) {
    if (!el) return false;
    const rect = el.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) return false;
    const style = window.getComputedStyle(el);
    if (style.display === 'none' || style.visibility === 'hidden' || style.opacity === '0') return false;
    return true;
  }

  function getActiveVideoRoot(hintId = '') {
    const normalizedHint = String(hintId || '').trim();
    const active = document.querySelector('[data-e2e="feed-active-video"]');
    if (active) return active;
    if (normalizedHint) {
      const byAttr = document.querySelector(`[data-e2e="feed-active-video"][data-e2e-vid="${normalizedHint}"]`);
      if (byAttr) return byAttr;
      const byClass = document.querySelector(`.video_${normalizedHint}`);
      if (byClass) return byClass;
    }
    return null;
  }

  function queryInActiveVideo(selector, hintId = '') {
    const roots = dedupeStrings([hintId, getCurrentCardHintId(window.location.href)]).flatMap((id) => {
      const hinted = getActiveVideoRoot(id);
      return hinted ? [hinted] : [];
    });
    const active = document.querySelector('[data-e2e="feed-active-video"]');
    if (active) roots.unshift(active);

    for (const root of roots) {
      const list = [...root.querySelectorAll(selector)];
      if (list.length > 0) {
        const visible = list.find(isElementVisible);
        if (visible) return visible;
        return list[0];
      }
      const wrapper = root.closest('[class*="video_"]');
      if (wrapper && wrapper !== root) {
        const wrappedList = [...wrapper.querySelectorAll(selector)];
        if (wrappedList.length > 0) {
          const visibleWrapped = wrappedList.find(isElementVisible);
          if (visibleWrapped) return visibleWrapped;
          return wrappedList[0];
        }
      }
    }
    const globalList = [...document.querySelectorAll(selector)];
    if (globalList.length === 0) return null;
    const visibleGlobal = globalList.find(isElementVisible);
    return visibleGlobal || globalList[0];
  }

  function getActiveVideoIdFromDom() {
    const active = document.querySelector('[data-e2e="feed-active-video"]');
    const candidates = [
      active?.getAttribute('data-e2e-vid'),
      active?.dataset?.e2eVid,
      active?.getAttribute('data-video-id'),
    ];
    for (const item of candidates) {
      const id = String(item || '').trim();
      if (!id) continue;
      if (/^[A-Za-z0-9_-]{8,24}$/.test(id)) return id;
    }
    return '';
  }

  function getAwemeIdFromDom() {
    const scopedInfo = queryInActiveVideo('[data-e2e="video-info"]');
    const scopedAwemeId = scopedInfo?.getAttribute('data-e2e-aweme-id');
    const candidates = [
      scopedAwemeId,
      queryInActiveVideo('[data-e2e-aweme-id]')?.getAttribute('data-e2e-aweme-id'),
      document.querySelector('[data-e2e="video-info"]')?.getAttribute('data-e2e-aweme-id'),
      document.querySelector('[data-e2e-aweme-id]')?.getAttribute('data-e2e-aweme-id'),
    ];
    for (const item of candidates) {
      const id = String(item || '').trim();
      if (!id) continue;
      if (/^\d{10,24}$/.test(id)) return id;
      if (/^[A-Za-z0-9_-]{10,24}$/.test(id)) return id;
    }
    return '';
  }

  function getCurrentDomTitle() {
    return sanitizeVideoTitle(queryInActiveVideo(SEL.desc)?.textContent || '');
  }

  function findRecentVideoIdByTitle(domTitle = '') {
    const title = sanitizeVideoTitle(domTitle || '');
    if (!title) return '';
    const entries = Object.entries(window.__lgboom_dy_video_data || {});
    let matchedId = '';
    let matchedAt = 0;
    for (const [id, data] of entries) {
      const apiTitle = sanitizeVideoTitle(data?.desc || '');
      if (!apiTitle) continue;
      if (apiTitle === title || apiTitle.includes(title) || title.includes(apiTitle)) {
        const fetchedAt = Number(data?.fetchedAt || 0);
        if (fetchedAt >= matchedAt) {
          matchedAt = fetchedAt;
          matchedId = id;
        }
      }
    }
    return matchedId;
  }

  function pickFirstId(...values) {
    return dedupeStrings(values)[0] || '';
  }

  function queryInScopeRoot(root, selector) {
    if (!root) return null;
    const targets = [root];
    const wrapper = root.closest('[class*="video_"]');
    if (wrapper && wrapper !== root) targets.push(wrapper);

    for (const target of targets) {
      const list = [...target.querySelectorAll(selector)];
      if (list.length === 0) continue;
      const visible = list.find(isElementVisible);
      if (visible) return visible;
      return list[0];
    }
    return null;
  }

  function buildDomVideoSnapshot(initialId = '') {
    const activeEl = document.querySelector('[data-e2e="feed-active-video"]');
    const hintId = pickFirstId(
      initialId,
      getActiveVideoIdFromDom(),
      getAwemeIdFromDom(),
      getUrlModalId(window.location.href),
      getUrlPathVideoId(window.location.href),
      getUrlVidParam(window.location.href),
    );
    const scopeRoot = activeEl || getActiveVideoRoot(hintId) || null;
    const infoEl = queryInScopeRoot(scopeRoot, SEL.videoInfo) || queryInActiveVideo(SEL.videoInfo, hintId);
    const descEl = queryInScopeRoot(scopeRoot, '[data-e2e="video-desc"]')
      || queryInScopeRoot(scopeRoot, '[data-e2e="detail-video-info"]')
      || queryInActiveVideo(SEL.desc, hintId);
    const videoEl = queryInScopeRoot(scopeRoot, SEL.videoEl) || queryInActiveVideo(SEL.videoEl, hintId);

    const activeVid = String(
      scopeRoot?.getAttribute?.('data-e2e-vid')
      || scopeRoot?.dataset?.e2eVid
      || activeEl?.getAttribute?.('data-e2e-vid')
      || activeEl?.dataset?.e2eVid
      || ''
    ).trim();
    const awemeId = String(
      infoEl?.getAttribute?.('data-e2e-aweme-id')
      || queryInScopeRoot(scopeRoot, '[data-e2e-aweme-id]')?.getAttribute?.('data-e2e-aweme-id')
      || ''
    ).trim();

    return {
      scopeRoot,
      activeEl,
      infoEl,
      descEl,
      videoEl,
      activeVid,
      awemeId,
      title: sanitizeVideoTitle(descEl?.textContent || ''),
      infoText: String(infoEl?.textContent || '').trim(),
      hasActiveContext: Boolean(scopeRoot || activeEl || infoEl || descEl || videoEl),
      rootClassName: String(scopeRoot?.className || activeEl?.className || '').trim(),
    };
  }

  function hasModalActiveVideoContext() {
    const snapshot = buildDomVideoSnapshot();
    return Boolean(getUrlModalId(window.location.href) || snapshot.hasActiveContext);
  }

  function pickPreferredVideoId({
    initialId = '',
    snapshot = buildDomVideoSnapshot(initialId),
    preferModalContext = hasModalActiveVideoContext(),
    pathVideoId = getUrlPathVideoId(window.location.href),
    vidFromQuery = getUrlVidParam(window.location.href),
    routerAwemeId = String(getRouterVideoData(document)?.aweme_id || getRouterVideoData(document)?.awemeId || ''),
    renderId = getRenderVideoId(),
    awemeId = snapshot.awemeId || getAwemeIdFromDom(),
    activeId = snapshot.activeVid || getActiveVideoIdFromDom(),
    modalId = getUrlModalId(window.location.href),
    titleMatchedId = findRecentVideoIdByTitle(snapshot.title || getCurrentDomTitle()),
    mode = '',
  } = {}) {
    const scene = mode || (pathVideoId ? 'direct' : (preferModalContext ? 'modal' : (vidFromQuery ? 'profilePreview' : 'unknown')));
    const modalOrder = [awemeId, activeId, modalId, renderId, routerAwemeId, titleMatchedId, initialId];
    const directOrder = [pathVideoId, awemeId, activeId, renderId, routerAwemeId, modalId, titleMatchedId, initialId];
    const previewOrder = [vidFromQuery, renderId, routerAwemeId, titleMatchedId, initialId];
    const fallbackOrder = [awemeId, activeId, modalId, pathVideoId, vidFromQuery, renderId, routerAwemeId, titleMatchedId, initialId];
    const order = scene === 'modal'
      ? modalOrder
      : scene === 'direct'
        ? directOrder
        : scene === 'profilePreview'
          ? previewOrder
          : fallbackOrder;
    return pickFirstId(...order);
  }

  function resolveCurrentVideoId(initialId = '') {
    return pickPreferredVideoId({ initialId });
  }

  function buildCurrentVideoContext(initialId = '') {
    const dom = buildDomVideoSnapshot(initialId);
    const pathVideoId = getUrlPathVideoId(window.location.href);
    const vidFromQuery = getUrlVidParam(window.location.href);
    const modalId = getUrlModalId(window.location.href);
    const activeId = dom.activeVid || getActiveVideoIdFromDom();
    const awemeId = dom.awemeId || getAwemeIdFromDom();
    const routerVideo = getRouterVideoData(document);
    const routerId = String(routerVideo?.aweme_id || routerVideo?.awemeId || '').trim();
    const renderDetail = getRenderVideoDetail(document);
    const renderId = String(renderDetail?.awemeId || renderDetail?.aweme_id || '').trim();
    const titleMatchedId = findRecentVideoIdByTitle(dom.title || getCurrentDomTitle());
    const hasModalContext = Boolean(modalId || dom.hasActiveContext);
    const mode = pathVideoId ? 'direct' : (hasModalContext ? 'modal' : (vidFromQuery ? 'profilePreview' : 'unknown'));

    const primaryId = pickPreferredVideoId({
      initialId,
      snapshot: dom,
      preferModalContext: mode === 'modal',
      pathVideoId,
      vidFromQuery,
      routerAwemeId: routerId,
      renderId,
      awemeId,
      activeId,
      modalId,
      titleMatchedId,
      mode,
    });

    const scopeHintId = pickFirstId(awemeId, activeId, modalId, pathVideoId, primaryId, vidFromQuery);
    const fetchIds = dedupeStrings(
      mode === 'modal'
        ? [primaryId, awemeId, activeId, modalId]
        : mode === 'direct'
          ? [primaryId, pathVideoId, awemeId, activeId, modalId]
          : [primaryId, vidFromQuery, pathVideoId]
    );
    const secondaryIds = dedupeStrings(
      mode === 'modal'
        ? [renderId, routerId, titleMatchedId, initialId]
        : [renderId, routerId, titleMatchedId, initialId, modalId, awemeId, activeId]
    );
    const aliasIds = dedupeStrings(
      mode === 'modal'
        ? [primaryId, awemeId, activeId, modalId]
        : [primaryId, pathVideoId, vidFromQuery, awemeId, activeId, modalId]
    );
    const candidateIds = dedupeStrings([...fetchIds, ...secondaryIds]);

    return {
      url: window.location.href,
      mode,
      primaryId,
      scopeHintId,
      dom,
      candidateIds,
      fetchIds,
      secondaryIds,
      aliasIds,
      ids: {
        pathVideoId,
        vidFromQuery,
        modalId,
        activeId,
        awemeId,
        renderId,
        routerId,
        titleMatchedId,
      },
      routerVideo,
      renderDetail,
    };
  }

  function getContextDomTitle(context = null) {
    return sanitizeVideoTitle(
      context?.dom?.title
      || queryInActiveVideo('[data-e2e="video-desc"]', context?.scopeHintId || context?.primaryId || '')?.textContent
      || queryInActiveVideo('[data-e2e="detail-video-info"]', context?.scopeHintId || context?.primaryId || '')?.textContent
      || ''
    );
  }

  function titlesMatch(left = '', right = '') {
    const a = sanitizeVideoTitle(left);
    const b = sanitizeVideoTitle(right);
    if (!a || !b) return false;
    return a === b || a.includes(b) || b.includes(a);
  }

  function matchContextData(data, resolvedVideoId = '', domTitle = '') {
    if (!data) return null;
    const dataId = String(data.id || '').trim();
    if (resolvedVideoId && dataId && dataId === String(resolvedVideoId).trim()) return data;
    if (domTitle && titlesMatch(domTitle, data?.desc || '')) return data;
    return null;
  }

  async function resolveContextApiData(context, options = {}) {
    const strictIds = dedupeStrings(options.fetchIds || context?.fetchIds || []);
    const secondaryIds = dedupeStrings(context?.secondaryIds || []);
    const domTitle = sanitizeVideoTitle(options.domTitle || getContextDomTitle(context));
    const aliasIds = dedupeStrings(options.aliasIds || context?.aliasIds || strictIds);
    const requireTitleMatch = Boolean(domTitle) && context?.mode === 'modal';

    const tryAccept = (data, id, allowPrimaryFallback = false) => {
      if (!data) return null;
      const normalizedId = String(data.id || id || '').trim();
      const matchedByTitle = !domTitle || titlesMatch(domTitle, data?.desc || '');
      if (matchedByTitle || (allowPrimaryFallback && !requireTitleMatch)) {
        registerVideoAliases(String(normalizedId || context?.primaryId || '').trim(), aliasIds);
        return data;
      }
      return null;
    };

    for (let index = 0; index < strictIds.length; index += 1) {
      const id = strictIds[index];
      const allowPrimaryFallback = index === 0;
      const cached = tryAccept(getApiVideoData(id), id, allowPrimaryFallback);
      if (cached) return cached;

      const fetched = tryAccept(await fetchDetailApiData(id), id, allowPrimaryFallback);
      if (fetched) return fetched;
    }

    for (const id of secondaryIds) {
      const cached = tryAccept(getApiVideoData(id), id, false);
      if (cached) return cached;

      const fetched = tryAccept(await fetchDetailApiData(id), id, false);
      if (fetched) return fetched;
    }

    return null;
  }

  async function resolveDownloadApiDataForContext(context, domTitle = '') {
    const strictIds = dedupeStrings([
      context?.primaryId,
      ...(context?.fetchIds || []),
      getUrlVidParam(context?.url || window.location.href),
    ]);
    const secondaryIds = dedupeStrings([
      ...(context?.secondaryIds || []),
      ...(context?.candidateIds || []),
    ]);
    const title = sanitizeVideoTitle(domTitle || getContextDomTitle(context));
    const aliasIds = dedupeStrings(context?.aliasIds || context?.fetchIds || context?.candidateIds || []);

    const tryAccept = (data, id, allowLoose = false) => {
      if (!hasUsableApiVideo(data)) return null;
      const matchedByTitle = !title || titlesMatch(title, data?.desc || '');
      if (!matchedByTitle && !allowLoose) return null;
      registerVideoAliases(String(data?.id || id || context?.primaryId || '').trim(), aliasIds);
      return data;
    };

    for (let index = 0; index < strictIds.length; index += 1) {
      const id = strictIds[index];
      const cached = tryAccept(getApiVideoData(id), id, index === 0 && !title);
      if (cached) return cached;

      const fetched = tryAccept(await fetchDetailApiData(id), id, index === 0 && !title);
      if (fetched) return fetched;
    }

    for (const id of secondaryIds) {
      const cached = tryAccept(getApiVideoData(id), id, false);
      if (cached) return cached;

      const fetched = tryAccept(await fetchDetailApiData(id), id, false);
      if (fetched) return fetched;
    }

    return null;
  }

  async function resolveStableVideoContext({
    initialId = '',
    waitForDesc = true,
    settleMs = 2000,
    timeoutMs = 4200,
  } = {}) {
    let context = buildCurrentVideoContext(initialId);
    if (!waitForDesc) return context;

    if (context.mode === 'modal' || context.mode === 'direct') {
      await waitForElement(SEL.desc, 3000).catch(() => {});
    }
    await waitForContentSettle(settleMs, context.scopeHintId || context.primaryId || '');

    const start = Date.now();
    let lastStableKey = '';
    let stableSince = 0;
    let bestContext = context;

    while (Date.now() - start < timeoutMs) {
      context = buildCurrentVideoContext(context.primaryId || initialId);
      bestContext = context;
      const domTitle = getContextDomTitle(context);
      const isReady = context.mode === 'modal'
        ? Boolean(context.primaryId && context.dom?.hasActiveContext && domTitle)
        : context.mode === 'direct'
          ? Boolean(context.primaryId)
          : Boolean(context.primaryId || context.ids?.vidFromQuery);

      const stableKey = isReady ? JSON.stringify({
        mode: context.mode,
        primaryId: context.primaryId,
        scopeHintId: context.scopeHintId,
        awemeId: context.dom?.awemeId || '',
        activeVid: context.dom?.activeVid || '',
        rootClassName: context.dom?.rootClassName || '',
        domTitle,
      }) : '';

      if (stableKey && stableKey === lastStableKey) {
        if (stableSince && Date.now() - stableSince >= 260) {
          return context;
        }
      } else {
        lastStableKey = stableKey;
        stableSince = Date.now();
      }

      await new Promise((resolve) => setTimeout(resolve, 120));
    }

    return bestContext;
  }

  return {
    queryInActiveVideo,
    getActiveVideoIdFromDom,
    getAwemeIdFromDom,
    buildCurrentVideoContext,
    getContextDomTitle,
    titlesMatch,
    matchContextData,
    resolveCurrentVideoId,
    resolveContextApiData,
    resolveDownloadApiDataForContext,
    resolveStableVideoContext,
  };
}
