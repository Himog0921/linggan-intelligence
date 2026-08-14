/**
 * 抖音视频页字段探针（在抖音视频页面 DevTools Console 执行）
 *
 * 目标：
 * 1. 从 URL / DOM / RENDER_DATA / __INITIAL_STATE__ 四层同时取值
 * 2. 输出“字段候选值 + 来源路径 + 一致性诊断”
 * 3. 用于从 0 开始重建稳定采集器
 */
(function probeDouyinVideoFields() {
  const MAX_SCAN_RESULTS = 120;

  function short(value, max = 160) {
    const text = String(value ?? '');
    return text.length > max ? `${text.slice(0, max)}...` : text;
  }

  function parseJsonCandidates(raw) {
    if (!raw) return null;
    const candidates = [String(raw)];
    try {
      const decoded = decodeURIComponent(String(raw));
      if (decoded !== raw) candidates.push(decoded);
      const twiceDecoded = decodeURIComponent(decoded);
      if (twiceDecoded !== decoded) candidates.push(twiceDecoded);
    } catch {
      // ignore
    }
    for (const item of candidates) {
      try {
        return JSON.parse(item);
      } catch {
        // try next
      }
    }
    return null;
  }

  function getRenderData() {
    const raw = document.getElementById('RENDER_DATA')?.innerHTML || '';
    return parseJsonCandidates(raw);
  }

  function getByPath(root, path) {
    if (!root || !path) return undefined;
    const segs = path.split('.');
    let cur = root;
    for (const seg of segs) {
      if (cur == null) return undefined;
      cur = cur[seg];
    }
    return cur;
  }

  function normalizeUrl(url) {
    const value = String(url || '').trim();
    if (!value) return '';
    if (value.startsWith('//')) return `${location.protocol}${value}`;
    if (value.startsWith('/')) return `${location.origin}${value}`;
    return value;
  }

  function normalizeAnyUrl(value) {
    if (!value) return '';
    if (typeof value === 'string') return normalizeUrl(value);
    if (Array.isArray(value)) {
      for (const item of value) {
        const found = normalizeAnyUrl(item);
        if (found) return found;
      }
      return '';
    }
    if (typeof value === 'object') {
      const keys = ['url', 'uri', 'src', 'playApi', 'playAddr', 'downloadAddr', 'masterUrl', 'playUrl', 'urlList', 'url_list'];
      for (const key of keys) {
        const found = normalizeAnyUrl(value[key]);
        if (found) return found;
      }
    }
    return '';
  }

  function collectSelectorText(selector) {
    const el = document.querySelector(selector);
    if (!el) return { found: false, text: '', html: '' };
    return {
      found: true,
      text: short((el.textContent || '').trim()),
      html: short((el.outerHTML || '').replace(/\s+/g, ' ')),
    };
  }

  function scanKeys(root, patterns, max = MAX_SCAN_RESULTS) {
    const out = [];
    const queue = [{ node: root, path: 'root', depth: 0 }];
    const visited = new Set();
    while (queue.length > 0 && out.length < max) {
      const { node, path, depth } = queue.shift();
      if (!node || typeof node !== 'object' || depth > 8) continue;
      if (visited.has(node)) continue;
      visited.add(node);
      const entries = Array.isArray(node) ? node.entries() : Object.entries(node);
      for (const [key, value] of entries) {
        const k = String(key);
        const p = `${path}.${k}`;
        const lower = k.toLowerCase();
        if (patterns.some((item) => lower.includes(item.toLowerCase()))) {
          const type = value == null ? 'nullish' : (Array.isArray(value) ? 'array' : typeof value);
          let preview = '';
          if (type === 'string' || type === 'number' || type === 'boolean') preview = short(value);
          if (type === 'array') preview = `len=${value.length}`;
          if (type === 'object') preview = `keys=${Object.keys(value || {}).slice(0, 8).join(',')}`;
          out.push({ path: p, key: k, type, preview });
          if (out.length >= max) break;
        }
        if (value && typeof value === 'object') queue.push({ node: value, path: p, depth: depth + 1 });
      }
    }
    return out;
  }

  function dedupe(values = []) {
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

  function parseIdsFromUrl() {
    let modalId = '';
    let pathVideoId = '';
    try {
      const u = new URL(location.href);
      modalId = u.searchParams.get('modal_id') || '';
      pathVideoId = u.pathname.match(/\/(video|note)\/([A-Za-z0-9_-]+)/)?.[2] || '';
    } catch {
      // ignore
    }
    return { modalId, pathVideoId, urlVideoId: modalId || pathVideoId };
  }

  const renderData = getRenderData();
  const initialState = window.__INITIAL_STATE__ || null;
  const videoDetail = getByPath(renderData, 'app.videoDetail') || null;
  const videoEl = document.querySelector('video');
  const activeVideoEl = document.querySelector('[data-e2e="feed-active-video"]');
  const ids = parseIdsFromUrl();
  const activeId = String(
    activeVideoEl?.getAttribute('data-e2e-vid')
    || activeVideoEl?.dataset?.e2eVid
    || activeVideoEl?.getAttribute('data-video-id')
    || '',
  ).trim();
  const renderVideoId = String(videoDetail?.awemeId || videoDetail?.aweme_id || '').trim();
  const resolvedVideoId = ids.urlVideoId || activeId || renderVideoId || '';

  const renderPlayUrl = normalizeAnyUrl(videoDetail?.video?.playApi) || normalizeAnyUrl(videoDetail?.video?.playAddr);
  const renderDownloadUrl = normalizeAnyUrl(videoDetail?.video?.downloadAddr);
  const domVideoSrc = normalizeUrl(videoEl?.currentSrc || videoEl?.src || '');
  const candidates = dedupe([renderDownloadUrl, renderPlayUrl, domVideoSrc]);

  const output = {
    time: new Date().toISOString(),
    url: location.href,
    title: document.title,
    page: {
      pathname: location.pathname,
      hasRenderData: Boolean(renderData),
      hasInitialState: Boolean(initialState),
    },
    ids: {
      ...ids,
      activeId,
      renderVideoId,
      resolvedVideoId,
      consistent: Boolean(resolvedVideoId)
        && [ids.urlVideoId, activeId, renderVideoId].filter(Boolean).every((id) => id === resolvedVideoId),
    },
    directFields: {
      desc_render: short(videoDetail?.desc || ''),
      desc_dom: collectSelectorText('[data-e2e="video-desc"], [data-e2e="detail-video-info"]').text,
      authorName_render: short(videoDetail?.authorInfo?.nickname || ''),
      authorUid_render: short(videoDetail?.authorInfo?.uid || ''),
      authorSecUid_render: short(videoDetail?.authorInfo?.secUid || ''),
      ip_render: short(videoDetail?.authorInfo?.ipLocation || videoDetail?.ipLocation || videoDetail?.ipLabel || ''),
      stats_render: {
        diggCount: Number(videoDetail?.stats?.diggCount || 0),
        commentCount: Number(videoDetail?.stats?.commentCount || 0),
        collectCount: Number(videoDetail?.stats?.collectCount || 0),
        shareCount: Number(videoDetail?.stats?.shareCount || 0),
        playCount: Number(videoDetail?.stats?.playCount || 0),
      },
      createTime_render: Number(videoDetail?.createTime || 0),
      duration_render: Number(videoDetail?.video?.duration || 0),
    },
    download: {
      candidates,
      hasHttpCandidate: candidates.some((url) => /^https?:\/\//i.test(url)),
      hasBlobCandidate: candidates.some((url) => /^blob:/i.test(url)),
      domCurrentSrc: short(domVideoSrc, 220),
      renderPlayUrl: short(renderPlayUrl, 220),
      renderDownloadUrl: short(renderDownloadUrl, 220),
    },
    selectorSnapshot: {
      desc: collectSelectorText('[data-e2e="video-desc"], [data-e2e="detail-video-info"]'),
      nickname: collectSelectorText('[data-e2e="feed-video-nickname"], [data-e2e="user-info"]'),
      videoInfo: collectSelectorText('[data-e2e="video-info"]'),
      activeVideo: {
        found: Boolean(activeVideoEl),
        attrs: activeVideoEl
          ? {
              'data-e2e-vid': activeVideoEl.getAttribute('data-e2e-vid') || '',
              'data-video-id': activeVideoEl.getAttribute('data-video-id') || '',
              class: short(activeVideoEl.className || ''),
            }
          : null,
      },
    },
    renderKeyScan: scanKeys(videoDetail, [
      'aweme', 'desc', 'author', 'nick', 'uid', 'sec', 'ip', 'location', 'create', 'duration',
      'digg', 'comment', 'collect', 'share', 'play', 'download', 'cover', 'url', 'addr',
    ]),
    initialStateKeyScan: scanKeys(initialState, [
      'aweme', 'video', 'play', 'download', 'author', 'uid', 'sec', 'ip', 'location',
    ], 80),
    diagnosis: [],
  };

  if (!output.ids.resolvedVideoId) {
    output.diagnosis.push('未能解析当前视频ID（URL/active/render均为空）');
  }
  if (!output.ids.consistent) {
    output.diagnosis.push('视频ID信号不一致（URL/active/render存在冲突）');
  }
  if (!output.download.hasHttpCandidate) {
    output.diagnosis.push('未发现可下载http(s)候选，当前下载很可能失败');
  }
  if (output.download.hasBlobCandidate && !output.download.hasHttpCandidate) {
    output.diagnosis.push('仅发现blob候选，刷新或切换视频后该链接可能失效');
  }

  window.__DY_VIDEO_FIELD_PROBE__ = output;
  console.group('[probe-douyin-video-fields]');
  console.log('result:', output);
  console.log('json:', JSON.stringify(output, null, 2));
  console.groupEnd();
  return output;
})();
