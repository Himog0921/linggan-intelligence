/**
 * 抖音采集回归探针（在抖音页面 DevTools Console 运行）
 *
 * 目的：
 * 1. 一次性输出“当前视频定位”相关证据（URL/active/render/cache）
 * 2. 输出 IP 属地、抖音号、下载候选 URL 的真实来源
 * 3. 失败时可直接把结果发给开发排查，减少反复沟通
 */
(function probeDouyinCollect() {
  function dedupeStrings(values = []) {
    const result = [];
    const seen = new Set();
    for (const raw of values) {
      const value = String(raw || '').trim();
      if (!value || seen.has(value)) continue;
      seen.add(value);
      result.push(value);
    }
    return result;
  }

  function normalizeIp(raw = '') {
    return String(raw || '')
      .replace(/^\s*IP属地[:：]?\s*/i, '')
      .replace(/\s+/g, '')
      .trim();
  }

  function parseVideoIdFromUrl(url = location.href) {
    try {
      const u = new URL(url);
      const modalId = u.searchParams.get('modal_id');
      if (modalId) return modalId;
      const pathMatch = u.pathname.match(/\/(video|note)\/([A-Za-z0-9_-]+)/);
      return pathMatch?.[2] || '';
    } catch {
      return '';
    }
  }

  function parseRenderData() {
    const raw = document.getElementById('RENDER_DATA')?.innerHTML;
    if (!raw) return null;
    const attempts = [String(raw)];
    try {
      const decoded = decodeURIComponent(String(raw));
      if (decoded !== raw) attempts.push(decoded);
      const twiceDecoded = decodeURIComponent(decoded);
      if (twiceDecoded !== decoded) attempts.push(twiceDecoded);
    } catch {
      // ignore
    }
    for (const candidate of attempts) {
      try {
        return JSON.parse(candidate);
      } catch {
        // try next
      }
    }
    return null;
  }

  function getActiveVideoId() {
    const active = document.querySelector('[data-e2e="feed-active-video"]');
    return String(
      active?.getAttribute('data-e2e-vid')
      || active?.dataset?.e2eVid
      || active?.getAttribute('data-video-id')
      || '',
    ).trim();
  }

  function getPlayingVideoId() {
    const links = [...document.querySelectorAll('a[href*="/video/"]')];
    for (const link of links) {
      if (!/播放中/.test(String(link.textContent || ''))) continue;
      const match = String(link.getAttribute('href') || '').match(/\/video\/([A-Za-z0-9_-]+)/);
      if (match?.[1]) return match[1];
    }
    return '';
  }

  function collectIpMarkerSamples(limit = 5) {
    const samples = [];
    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
    let node = walker.nextNode();
    while (node && samples.length < limit) {
      const text = String(node.textContent || '').trim();
      if (text && /IP属地/i.test(text)) {
        samples.push(text.slice(0, 120));
      }
      node = walker.nextNode();
    }
    return samples;
  }

  const renderData = parseRenderData();
  const renderVideo = renderData?.app?.videoDetail || null;
  const renderUser = renderData?.app?.user?.info
    || window.__INITIAL_STATE__?.user?.userInfo?._rawValue
    || null;
  const cache = window.__lgboom_dy_video_data || {};

  const urlId = parseVideoIdFromUrl();
  const activeId = getActiveVideoId();
  const renderId = String(renderVideo?.awemeId || renderVideo?.aweme_id || '').trim();
  const playingId = getPlayingVideoId();
  const resolvedId = activeId || renderId || playingId || urlId || '';

  const currentCache = resolvedId ? cache[resolvedId] : null;
  const videoEl = document.querySelector('video');
  const domDesc = String(
    document.querySelector('[data-e2e="video-desc"], [data-e2e="detail-video-info"]')?.textContent || '',
  ).trim();

  const candidates = dedupeStrings([
    currentCache?.videoDownloadUrl,
    currentCache?.videoPlayUrl,
    videoEl?.currentSrc,
    videoEl?.src,
  ]);

  const output = {
    url: location.href,
    time: new Date().toISOString(),
    pageType: /\/user\//.test(location.pathname) ? 'author' : (/\/video\//.test(location.pathname) ? 'video' : 'other'),
    videoIds: {
      urlId,
      activeId,
      renderId,
      playingId,
      resolvedId,
      consistent: Boolean(resolvedId && [urlId, activeId, renderId].filter(Boolean).every((id) => id === resolvedId)),
    },
    render: {
      hasRenderData: Boolean(renderData),
      hasVideoDetail: Boolean(renderVideo),
      hasUserInfo: Boolean(renderUser),
      renderDesc: String(renderVideo?.desc || '').slice(0, 80),
      renderIpLocation: normalizeIp(renderVideo?.authorInfo?.ipLocation || renderVideo?.ipLocation || ''),
      renderUniqueId: String(renderUser?.uniqueId || renderUser?.unique_id || ''),
    },
    cache: {
      cacheSize: Object.keys(cache).length,
      hasResolvedCache: Boolean(currentCache),
      sourceUrl: String(currentCache?.sourceUrl || ''),
      fetchedAt: Number(currentCache?.fetchedAt || 0),
      cacheDesc: String(currentCache?.desc || '').slice(0, 80),
      cacheIpLocation: normalizeIp(currentCache?.ipLocation || ''),
    },
    dom: {
      navigatorOnline: navigator.onLine,
      domDesc: domDesc.slice(0, 80),
      domUserIpText: String(document.querySelector('[data-e2e="user-ip"]')?.textContent || '').trim().slice(0, 80),
      ipMarkerSamples: collectIpMarkerSamples(),
      videoCurrentSrc: String(videoEl?.currentSrc || '').slice(0, 120),
    },
    author: {
      nickname: String(renderUser?.nickname || '').trim(),
      secUid: String(renderUser?.secUid || renderUser?.sec_uid || '').trim(),
      ipLocation: normalizeIp(renderUser?.ipLocation || renderUser?.ip_location || ''),
    },
    download: {
      candidateCount: candidates.length,
      candidates,
      hasBlobCandidate: candidates.some((url) => String(url).startsWith('blob:')),
      hasHttpCandidate: candidates.some((url) => /^https?:\/\//i.test(String(url))),
    },
  };

  console.group('[probe-douyin-collect]');
  console.log('result:', output);
  if (candidates.length > 0) {
    console.table(candidates.map((url, index) => ({ index, url })));
  }
  console.groupEnd();

  return output;
})();
