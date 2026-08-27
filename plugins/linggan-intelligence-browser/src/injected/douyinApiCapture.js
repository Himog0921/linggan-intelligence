/**
 * 抖音 API 响应捕获器（运行在 MAIN world）
 *
 * 拦截页面自身 fetch/XHR，当检测到视频相关 API 时，提取视频下载地址、播放量、
 * IP 属地等字段，存储到 window.__lgboom_dy_video_data 供 content script 使用。
 */
(function () {
  if (window.__lgboom_dy_api_capture_installed) return;
  window.__lgboom_dy_api_capture_installed = true;

  window.__lgboom_dy_video_data = window.__lgboom_dy_video_data || {};
  window.__lgboom_dy_search_pages = window.__lgboom_dy_search_pages || [];
  const BRIDGE_EVENT = '__lgboom_dy_api_data__';
  const SEARCH_BRIDGE_EVENT = '__lgboom_dy_search_data__';
  const BRIDGE_SOURCE = 'lgboom-dy-api-capture';
  const BRIDGE_REQUEST_SOURCE = 'lgboom-dy-content';
  const BRIDGE_REQUEST_TYPE = '__lgboom_dy_api_data_request__';
  const PAGE_FETCH_REQUEST_TYPE = '__lgboom_dy_page_fetch_request__';
  const PAGE_FETCH_RESPONSE_TYPE = '__lgboom_dy_page_fetch_response__';

  const API_PATTERNS = [
    '/aweme/v1/web/aweme/detail/',
    '/aweme/v1/web/note/detail/',
    '/aweme/v1/web/aweme/post/',
    '/aweme/v1/web/feed/',
    '/aweme/v1/web/user/profile/',
    '/aweme/v1/web/user/profile/other/',
    '/aweme/v1/web/general/search/stream/',
  ];

  function isRelevantApi(url) {
    const raw = String(url || '');
    return API_PATTERNS.some((p) => raw.includes(p));
  }

  function safeClone(value) {
    if (value == null) return null;
    try {
      return JSON.parse(JSON.stringify(value));
    } catch {
      return null;
    }
  }

  function emitBridge(items, sourceUrl = '') {
    const normalizedItems = [];
    for (const item of Array.isArray(items) ? items : []) {
      const id = String(item?.id || '').trim();
      const data = safeClone(item?.data);
      if (!id || !data) continue;
      normalizedItems.push({ id, data });
    }
    if (normalizedItems.length === 0) return;

    const payload = {
      items: normalizedItems,
      sourceUrl,
      at: Date.now(),
    };

    try {
      window.dispatchEvent(new CustomEvent(BRIDGE_EVENT, { detail: payload }));
    } catch {}

    try {
      document.dispatchEvent(new CustomEvent(BRIDGE_EVENT, { detail: payload }));
    } catch {}

    try {
      window.postMessage({
        source: BRIDGE_SOURCE,
        type: BRIDGE_EVENT,
        payload,
      }, '*');
    } catch {}
  }

  function emitSearchBridge(pages, sourceUrl = '') {
    const normalizedPages = [];
    for (const page of Array.isArray(pages) ? pages : []) {
      const keyword = String(page?.keyword || '').trim();
      const searchChannel = String(page?.searchChannel || '').trim() || 'aweme_general';
      const items = Array.isArray(page?.items) ? safeClone(page.items) : [];
      if (!keyword || items.length === 0) continue;
      normalizedPages.push({
        ...safeClone(page),
        keyword,
        searchChannel,
        items,
      });
    }
    if (normalizedPages.length === 0) return;

    const payload = {
      pages: normalizedPages,
      sourceUrl,
      at: Date.now(),
    };

    try {
      window.dispatchEvent(new CustomEvent(SEARCH_BRIDGE_EVENT, { detail: payload }));
    } catch {}

    try {
      document.dispatchEvent(new CustomEvent(SEARCH_BRIDGE_EVENT, { detail: payload }));
    } catch {}

    try {
      window.postMessage({
        source: BRIDGE_SOURCE,
        type: SEARCH_BRIDGE_EVENT,
        payload,
      }, '*');
    } catch {}
  }

  function normalizeSearchChannel(value = '') {
    const raw = String(value || '').trim().toLowerCase();
    if (raw === 'video' || raw === 'aweme_video') return 'aweme_video';
    return 'aweme_general';
  }

  function safeDecode(value = '') {
    const text = String(value || '').trim();
    if (!text) return '';
    try {
      return decodeURIComponent(text).trim();
    } catch {
      return text;
    }
  }

  function getSearchAweme(item = {}) {
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

  function parseSearchPagePayload(json, sourceUrl = '') {
    if (!String(sourceUrl || '').includes('/aweme/v1/web/general/search/stream/')) {
      return null;
    }

    let parsedUrl = null;
    try {
      parsedUrl = new URL(sourceUrl, window.location.origin);
    } catch {
      parsedUrl = null;
    }

    const keyword = safeDecode(parsedUrl?.searchParams.get('keyword') || '');
    const searchChannel = normalizeSearchChannel(parsedUrl?.searchParams.get('search_channel') || '');
    const rawItems = Array.isArray(json?.data) ? json.data : [];
    const items = rawItems
      .map((item, index) => {
        const aweme = getSearchAweme(item);
        const awemeId = String(aweme?.aweme_id || '').trim();
        if (!aweme || !awemeId) return null;
        return {
          awemeId,
          aweme: safeClone(aweme),
          orderIndex: index,
        };
      })
      .filter(Boolean);

    if (!keyword || items.length === 0) return null;

    const offset = Number(json?.offset ?? parsedUrl?.searchParams.get('offset') ?? 0) || 0;
    const nextOffset = Number(json?.next_offset ?? json?.offset ?? (offset + rawItems.length)) || (offset + rawItems.length);
    const hasMore = json?.has_more === true || json?.has_more === 1 || json?.hasMore === true || json?.hasMore === 1;

    return {
      keyword,
      searchChannel,
      offset,
      nextOffset,
      hasMore,
      sourceUrl: String(sourceUrl || '').trim(),
      capturedAt: Date.now(),
      items,
    };
  }

  function upsertSearchPage(page) {
    if (!page || !Array.isArray(page.items) || page.items.length === 0) return;
    const pageId = `${String(page.keyword || '').trim()}::${String(page.searchChannel || '').trim() || 'aweme_general'}::${Number(page.offset || 0)}`;
    const existing = Array.isArray(window.__lgboom_dy_search_pages)
      ? window.__lgboom_dy_search_pages
      : [];
    const filtered = existing.filter((entry) => {
      const entryId = `${String(entry?.keyword || '').trim()}::${String(entry?.searchChannel || '').trim() || 'aweme_general'}::${Number(entry?.offset || 0)}`;
      return entryId !== pageId;
    });
    filtered.push(page);
    filtered.sort((a, b) => Number(a?.capturedAt || 0) - Number(b?.capturedAt || 0));
    window.__lgboom_dy_search_pages = filtered.slice(-20);
  }

  function mapAweme(aweme, sourceUrl = '') {
    if (!aweme?.aweme_id) return null;
    const id = String(aweme.aweme_id);
    const now = Date.now();
    const prev = window.__lgboom_dy_video_data[id] || {};
    return {
      ...prev,
      videoPlayUrl: aweme.video?.play_addr?.url_list?.[0] || prev.videoPlayUrl || '',
      videoDownloadUrl: aweme.video?.download_addr?.url_list?.[0]
        || aweme.video?.play_addr?.url_list?.[0]
        || prev.videoDownloadUrl
        || '',
      playCount: Number(aweme.statistics?.play_count || prev.playCount || 0),
      ipLocation: aweme.ip_label || aweme.region || prev.ipLocation || '',
      releaseDate: aweme.create_time ? aweme.create_time * 1000 : Number(prev.releaseDate || 0),
      duration: Number(aweme.video?.duration || prev.duration || 0),
      desc: aweme.desc || prev.desc || '',
      authorName: aweme.author?.nickname || prev.authorName || '',
      authorId: aweme.author?.uid || prev.authorId || '',
      authorSecUid: aweme.author?.sec_uid || prev.authorSecUid || '',
      authorAvatar: aweme.author?.avatar_thumb?.url_list?.[0] || prev.authorAvatar || '',
      coverImg: aweme.video?.cover?.url_list?.[0]
        || aweme.video?.dynamic_cover?.url_list?.[0]
        || prev.coverImg
        || '',
      fetchedAt: now,
      sourceUrl: sourceUrl || prev.sourceUrl || '',
    };
  }

  function collectAwemeObjects(payload, max = 120) {
    if (!payload || typeof payload !== 'object') return [];
    const out = [];
    const queue = [payload];
    const visited = new Set();

    while (queue.length > 0 && out.length < max) {
      const cur = queue.shift();
      if (!cur || typeof cur !== 'object') continue;
      if (visited.has(cur)) continue;
      visited.add(cur);

      if (cur.aweme_id && cur.video) {
        out.push(cur);
      }

      if (Array.isArray(cur)) {
        for (const item of cur) {
          if (item && typeof item === 'object') queue.push(item);
        }
        continue;
      }

      for (const value of Object.values(cur)) {
        if (value && typeof value === 'object') queue.push(value);
      }
    }

    return out;
  }

  function processApiResponse(json, sourceUrl = '') {
    if (!json || typeof json !== 'object') return;
    const changedMap = new Map();
    const searchPage = parseSearchPagePayload(json, sourceUrl);

    if (json.aweme_detail?.aweme_id) {
      const mapped = mapAweme(json.aweme_detail, sourceUrl);
      if (mapped) {
        const id = String(json.aweme_detail.aweme_id);
        window.__lgboom_dy_video_data[id] = mapped;
        changedMap.set(id, mapped);
      }
    }

    const awemeList = collectAwemeObjects(json);
    for (const aweme of awemeList) {
      const mapped = mapAweme(aweme, sourceUrl);
      if (!mapped) continue;
      const id = String(aweme.aweme_id);
      window.__lgboom_dy_video_data[id] = mapped;
      changedMap.set(id, mapped);
    }

    const changed = [...changedMap.entries()].map(([id, data]) => ({ id, data }));
    const count = changed.length;
    if (count > 0) {
      emitBridge(changed, sourceUrl);
      console.log('[灵感爆爆爆] API 捕获视频数据条数:', count, sourceUrl);
    }

    if (searchPage) {
      upsertSearchPage(searchPage);
      emitSearchBridge([searchPage], sourceUrl);
      console.log('[灵感爆爆爆] API 捕获搜索结果页:', searchPage.keyword, searchPage.searchChannel, searchPage.offset, searchPage.items.length);
    }
  }

  const origFetch = window.fetch;
  window.fetch = async function (...args) {
    const resp = await origFetch.apply(this, args);
    const url = typeof args[0] === 'string' ? args[0] : (args[0]?.url || '');
    if (isRelevantApi(url)) {
      try {
        const clone = resp.clone();
        clone.json().then((json) => processApiResponse(json, url)).catch(() => {});
      } catch {
        // ignore
      }
    }
    return resp;
  };

  const origOpen = XMLHttpRequest.prototype.open;
  const origSend = XMLHttpRequest.prototype.send;
  XMLHttpRequest.prototype.open = function (method, url, ...rest) {
    this.__lgboom_url = url;
    return origOpen.apply(this, [method, url, ...rest]);
  };
  XMLHttpRequest.prototype.send = function (...args) {
    if (this.__lgboom_url && isRelevantApi(this.__lgboom_url)) {
      this.addEventListener('load', () => {
        try {
          const json = JSON.parse(this.responseText);
          processApiResponse(json, this.__lgboom_url || '');
        } catch {
          // ignore
        }
      });
    }
    return origSend.apply(this, args);
  };

  function emitSnapshot(sourceUrl = '__bootstrap__') {
    const snapshot = Object.entries(window.__lgboom_dy_video_data || {}).map(([id, data]) => ({ id, data }));
    if (snapshot.length === 0) return;
    emitBridge(snapshot, sourceUrl);
  }

  function emitSearchSnapshot(sourceUrl = '__search_bootstrap__') {
    const pages = Array.isArray(window.__lgboom_dy_search_pages)
      ? window.__lgboom_dy_search_pages
      : [];
    if (pages.length === 0) return;
    emitSearchBridge(pages, sourceUrl);
  }

  async function handlePageFetchRequest(payload = {}) {
    const requestId = String(payload?.requestId || '').trim();
    const urls = Array.isArray(payload?.urls) ? payload.urls : [];
    if (!requestId || urls.length === 0) return;

    let responsePayload = {
      requestId,
      ok: false,
      url: '',
      json: null,
      error: 'fetch_failed',
    };

    for (const rawUrl of urls) {
      const url = String(rawUrl || '').trim();
      if (!url) continue;
      try {
        const response = await origFetch.call(window, url, { credentials: 'include' });
        if (!response.ok) {
          responsePayload = {
            requestId,
            ok: false,
            url,
            json: null,
            error: `HTTP ${response.status}`,
          };
          continue;
        }
        const json = await response.json();
        const statusCode = Number(json?.status_code ?? json?.statusCode ?? 0);
        if (Number.isFinite(statusCode) && statusCode !== 0) {
          responsePayload = {
            requestId,
            ok: false,
            url,
            json: null,
            error: `status_code=${statusCode}`,
          };
          continue;
        }

        responsePayload = {
          requestId,
          ok: true,
          url,
          json: safeClone(json),
          error: '',
        };
        break;
      } catch (err) {
        responsePayload = {
          requestId,
          ok: false,
          url,
          json: null,
          error: String(err?.message || err || 'fetch_failed'),
        };
      }
    }

    try {
      window.postMessage({
        source: BRIDGE_SOURCE,
        type: PAGE_FETCH_RESPONSE_TYPE,
        payload: responsePayload,
      }, '*');
    } catch {
      // ignore
    }
  }

  window.addEventListener('message', (event) => {
    try {
      if (event.source !== window) return;
      const data = event.data || {};
      if (data.source !== BRIDGE_REQUEST_SOURCE) return;
      if (data.type === BRIDGE_REQUEST_TYPE) {
        emitSnapshot('__request__');
        emitSearchSnapshot('__search_request__');
        return;
      }
      if (data.type === PAGE_FETCH_REQUEST_TYPE) {
        handlePageFetchRequest(data.payload || {});
      }
    } catch {
      // ignore
    }
  });

  // ========== 页面上下文下载能力 ==========
  // content script 通过 CustomEvent 请求在 MAIN world 中下载文件
  // MAIN world 的 fetch 携带页面完整 cookie，能通过 CDN 鉴权
  const PAGE_DOWNLOAD_REQ = '__lgboom_page_download_req__';
  const PAGE_DOWNLOAD_RES = '__lgboom_page_download_res__';

  async function handlePageDownloadRequest(payload) {
    const { urls, filename, requestId } = payload || {};
    if (!requestId || !Array.isArray(urls) || urls.length === 0) {
      try {
        window.dispatchEvent(new CustomEvent(PAGE_DOWNLOAD_RES, {
          detail: { requestId, ok: false, error: 'invalid_request' },
        }));
      } catch {}
      return;
    }

    const referer = window.location.href || 'https://www.douyin.com/';
    for (const rawUrl of urls) {
      const url = String(rawUrl || '').trim();
      if (!url) continue;
      // 尝试多种 fetch 配置：omit / include，因为某些 CDN 对 credentials 与 CORS 有冲突
      // 2026-04-15：移除 Range 头，避免触发 CORS preflight 或 CDN 拒绝
      const configs = [
        { credentials: 'include', headers: { Referer: referer } },
        { credentials: 'omit', headers: { Referer: referer } },
        { credentials: 'include', headers: { Referer: referer, Range: 'bytes=0-5242880' } },
      ];
      for (const config of configs) {
        try {
          const resp = await origFetch.call(window, url, config);
          if (!resp.ok) continue;
          const blob = await resp.blob();
          if (!blob || blob.size <= 0) continue;

          const blobUrl = URL.createObjectURL(blob);
          const a = document.createElement('a');
          a.href = blobUrl;
          a.download = filename || 'download';
          a.style.display = 'none';
          document.body.appendChild(a);
          a.click();

          setTimeout(() => {
            URL.revokeObjectURL(blobUrl);
            a.remove();
          }, 10000);

          window.dispatchEvent(new CustomEvent(PAGE_DOWNLOAD_RES, {
            detail: { requestId, ok: true, url },
          }));
          return;
        } catch {
          // 尝试下一个配置或候选 URL
        }
      }
    }

    try {
      window.dispatchEvent(new CustomEvent(PAGE_DOWNLOAD_RES, {
        detail: { requestId, ok: false, error: 'all_candidates_failed' },
      }));
    } catch {}
  }

  window.addEventListener(PAGE_DOWNLOAD_REQ, (e) => {
    handlePageDownloadRequest(e.detail || {});
  });

  // 启动时主动广播一次快照，避免 content script 晚绑定丢数据
  emitSnapshot('__bootstrap__');

  console.log('[灵感爆爆爆] 抖音 API 捕获器已安装');
})();
