(function () {
  if (window.__lgboom_xhs_api_capture_installed) return;
  window.__lgboom_xhs_api_capture_installed = true;

  window.__lgboom_xhs_comment_pages = window.__lgboom_xhs_comment_pages || {};
  window.__lgboom_xhs_sub_comment_pages = window.__lgboom_xhs_sub_comment_pages || {};
  window.__lgboom_xhs_user_posted_pages = window.__lgboom_xhs_user_posted_pages || [];
  window.__lgboom_xhs_search_note_pages = window.__lgboom_xhs_search_note_pages || [];

  // Bounded, content-free diagnostics. It answers one question: did this hook see the page's
  // own comment API traffic, and did that payload carry numeric times? No comment text,
  // author data or signed URLs are kept here.
  const diag = {
    installedAt: Date.now(),
    requests: [],
  };
  window.__lgboom_xhs_capture_diag = diag;

  const BRIDGE_SOURCE = 'lgboom-xhs-api-capture';
  const REQUEST_SOURCE = 'lgboom-xhs-content';
  const SNAPSHOT_REQUEST_TYPE = '__lgboom_xhs_comment_api_request__';
  const SNAPSHOT_RESPONSE_TYPE = '__lgboom_xhs_comment_api_response__';
  const SNAPSHOT_RESET_REQUEST_TYPE = '__lgboom_xhs_comment_api_reset_request__';
  const SNAPSHOT_RESET_RESPONSE_TYPE = '__lgboom_xhs_comment_api_reset_response__';
  const PROFILE_NOTES_REQUEST_TYPE = '__lgboom_xhs_profile_notes_request__';
  const PROFILE_NOTES_RESPONSE_TYPE = '__lgboom_xhs_profile_notes_response__';
  const SEARCH_NOTES_REQUEST_TYPE = '__lgboom_xhs_search_notes_request__';
  const SEARCH_NOTES_RESPONSE_TYPE = '__lgboom_xhs_search_notes_response__';
  const PAGE_FETCH_REQUEST_TYPE = '__lgboom_xhs_page_fetch_request__';
  const PAGE_FETCH_RESPONSE_TYPE = '__lgboom_xhs_page_fetch_response__';
  const CAPTURE_PROBE_REQUEST_TYPE = '__lgboom_xhs_capture_probe_request__';
  const CAPTURE_PROBE_RESPONSE_TYPE = '__lgboom_xhs_capture_probe_response__';
  const PROBE_REQUEST_LOG_LIMIT = 24;
  const PROBE_TIME_TEXT_SAMPLES = 8;
  const API_PATTERNS = [
    '/api/sns/web/v1/comment',
    '/api/sns/web/v2/comment',
    '/api/sns/web/v1/user_posted',
    '/api/sns/web/v2/search/notes',
  ];

  function safeClone(value) {
    if (value == null) return null;
    try {
      return JSON.parse(JSON.stringify(value));
    } catch {
      return null;
    }
  }

  function isRelevantApi(url) {
    const raw = String(url || '');
    return API_PATTERNS.some((pattern) => raw.includes(pattern));
  }

  function isProfileNotesApi(url) {
    return String(url || '').includes('/api/sns/web/v1/user_posted');
  }

  function isSearchNotesApi(url) {
    return String(url || '').includes('/api/sns/web/v2/search/notes');
  }

  function safeDecode(value = '') {
    try {
      return decodeURIComponent(String(value || ''));
    } catch {
      return String(value || '');
    }
  }

  function normalizeText(value) {
    return String(value || '').trim();
  }

  // Only the endpoint path is recorded: query strings carry note ids, cursors and tokens.
  function readApiPath(url = '') {
    try {
      return new URL(String(url || ''), window.location.origin).pathname;
    } catch {
      return '';
    }
  }

  function recordRequest(url, { status = null, ok = false, transport = '' } = {}) {
    try {
      const path = readApiPath(url);
      if (!path) return;
      diag.requests.push({ path, status, ok: Boolean(ok), transport, at: Date.now() });
      if (diag.requests.length > PROBE_REQUEST_LOG_LIMIT) {
        diag.requests = diag.requests.slice(-PROBE_REQUEST_LOG_LIMIT);
      }
    } catch {
      // diagnostics must never disturb the page
    }
  }

  function readEpoch(value) {
    if (value == null || value === '' || typeof value === 'object') return null;
    const numeric = Number(value);
    if (!Number.isFinite(numeric) || numeric <= 0) return null;
    const epoch = numeric < 1000000000000 ? numeric * 1000 : numeric;
    return epoch >= 1000000000000 && epoch < 253402300800000 ? epoch : null;
  }

  function readCommentTimeField(comment = {}) {
    return comment?.create_time ?? comment?.createTime ?? comment?.time ?? null;
  }

  function pageCommentTimeStats(pages = []) {
    const stats = { comments: 0, epochTimes: 0, textTimes: new Map() };
    const visit = (comment) => {
      if (!comment || typeof comment !== 'object') return;
      stats.comments += 1;
      const raw = readCommentTimeField(comment);
      if (readEpoch(raw) !== null) {
        stats.epochTimes += 1;
      } else {
        const text = normalizeText(raw);
        if (text) stats.textTimes.set(text, (stats.textTimes.get(text) || 0) + 1);
      }
      const inline = comment?.sub_comments || comment?.subComments || comment?.reply_comments || comment?.replies;
      if (Array.isArray(inline)) inline.forEach(visit);
    };
    (Array.isArray(pages) ? pages : []).forEach((page) => {
      (Array.isArray(page?.comments) ? page.comments : []).forEach(visit);
    });
    return stats;
  }

  function mergeTimeStats(target, source) {
    target.comments += source.comments;
    target.epochTimes += source.epochTimes;
    source.textTimes.forEach((count, text) => {
      target.textTimes.set(text, (target.textTimes.get(text) || 0) + count);
    });
    return target;
  }

  function timeTextSamples(map) {
    return [...map.entries()]
      .sort((a, b) => b[1] - a[1])
      .slice(0, PROBE_TIME_TEXT_SAMPLES)
      .map(([text, count]) => ({ text, count }));
  }

  function buildCaptureProbe(noteId) {
    const mainPages = Array.isArray(window.__lgboom_xhs_comment_pages?.[noteId])
      ? window.__lgboom_xhs_comment_pages[noteId]
      : [];
    const subPages = Object.entries(window.__lgboom_xhs_sub_comment_pages || {})
      .filter(([key]) => key.startsWith(`${noteId}::`))
      .flatMap(([, value]) => Array.isArray(value) ? value : []);
    const stats = mergeTimeStats(pageCommentTimeStats(mainPages), pageCommentTimeStats(subPages));
    return {
      installedAt: diag.installedAt,
      now: Date.now(),
      requests: diag.requests.slice(),
      mainPages: mainPages.length,
      subPages: subPages.length,
      comments: stats.comments,
      epochTimes: stats.epochTimes,
      timeTextSamples: timeTextSamples(stats.textTimes),
    };
  }

  function readPayload(json) {
    if (json?.data && typeof json.data === 'object') return json.data;
    return json || {};
  }

  function extractComments(json) {
    const payload = readPayload(json);
    const candidates = [
      payload?.comments,
      payload?.comment_list,
      payload?.list,
      json?.comments,
      json?.comment_list,
      Array.isArray(payload) ? payload : null,
    ];
    for (const candidate of candidates) {
      if (Array.isArray(candidate)) {
        return candidate.map((item) => safeClone(item)).filter(Boolean);
      }
    }
    return [];
  }

  function readCursor(json) {
    const payload = readPayload(json);
    const candidates = [
      payload?.cursor,
      payload?.next_cursor,
      payload?.nextCursor,
      json?.cursor,
      json?.next_cursor,
      json?.nextCursor,
    ];
    for (const value of candidates) {
      if (value == null) continue;
      const text = normalizeText(value);
      if (text) return text;
    }
    return '';
  }

  function readHasMore(json) {
    const payload = readPayload(json);
    const candidates = [
      payload?.has_more,
      payload?.hasMore,
      payload?.more,
      json?.has_more,
      json?.hasMore,
      json?.more,
    ];
    for (const value of candidates) {
      if (value === true || value === 1 || value === '1') return true;
      if (value === false || value === 0 || value === '0') return false;
    }
    return false;
  }

  function extractProfileNotes(json) {
    const payload = readPayload(json);
    const candidates = [
      payload?.notes,
      payload?.items,
      payload?.list,
      json?.notes,
      json?.items,
      json?.list,
      Array.isArray(payload) ? payload : null,
    ];
    for (const candidate of candidates) {
      if (Array.isArray(candidate)) {
        return candidate.map((item) => safeClone(item)).filter(Boolean);
      }
    }
    return [];
  }

  function extractSearchNotes(json) {
    const payload = readPayload(json);
    const candidates = [
      payload?.items,
      payload?.notes,
      payload?.list,
      json?.items,
      json?.notes,
      json?.list,
      Array.isArray(payload) ? payload : null,
    ];
    for (const candidate of candidates) {
      if (Array.isArray(candidate)) {
        return candidate.map((item) => safeClone(item)).filter(Boolean);
      }
    }
    return [];
  }

  function buildSearchNotesSnapshot(json, sourceUrl = '') {
    let parsedPageUrl = null;
    try {
      parsedPageUrl = new URL(window.location.href, window.location.origin);
    } catch {
      parsedPageUrl = null;
    }

    const notes = extractSearchNotes(json);
    if (notes.length === 0) return null;
    return {
      keyword: safeDecode(parsedPageUrl?.searchParams.get('keyword') || ''),
      pageUrl: normalizeText(window.location.href),
      notes,
      sourceUrl: normalizeText(sourceUrl),
      capturedAt: Date.now(),
    };
  }

  function buildProfileNotesSnapshot(json, sourceUrl = '') {
    let parsedUrl = null;
    try {
      parsedUrl = new URL(sourceUrl, window.location.origin);
    } catch {
      parsedUrl = null;
    }

    const payload = readPayload(json);
    const userId = normalizeText(
      parsedUrl?.searchParams.get('user_id')
      || payload?.user_id
      || payload?.userId
      || json?.user_id
      || json?.userId
    );
    const notes = extractProfileNotes(json);
    if (!userId && notes.length === 0) return null;

    return {
      userId,
      cursor: readCursor(json) || normalizeText(parsedUrl?.searchParams.get('cursor')),
      hasMore: readHasMore(json),
      notes,
      sourceUrl: normalizeText(sourceUrl),
      capturedAt: Date.now(),
    };
  }

  function buildSnapshot(json, sourceUrl = '') {
    let parsedUrl = null;
    try {
      parsedUrl = new URL(sourceUrl, window.location.origin);
    } catch {
      parsedUrl = null;
    }

    const payload = readPayload(json);
    const endpoint = String(sourceUrl || '').includes('/api/sns/web/v2/comment/sub/page') ? 'sub' : 'page';
    const noteId = normalizeText(
      parsedUrl?.searchParams.get('note_id')
      || payload?.note_id
      || payload?.noteId
      || json?.note_id
      || json?.noteId
    );
    const rootCommentId = normalizeText(
      parsedUrl?.searchParams.get('root_comment_id')
      || payload?.root_comment_id
      || payload?.rootCommentId
      || json?.root_comment_id
      || json?.rootCommentId
    );

    if (!noteId) return null;

    return {
      endpoint,
      noteId,
      rootCommentId,
      cursor: readCursor(json),
      hasMore: readHasMore(json),
      comments: extractComments(json),
      sourceUrl: normalizeText(sourceUrl),
      capturedAt: Date.now(),
    };
  }

  function upsertPage(store, key, snapshot) {
    if (!key || !snapshot) return;
    const current = Array.isArray(store[key]) ? store[key] : [];
    const pageKey = `${snapshot.endpoint}|${snapshot.cursor || '__first__'}|${snapshot.sourceUrl || '__source__'}`;
    const filtered = current.filter((item) => {
      const itemKey = `${normalizeText(item?.endpoint)}|${normalizeText(item?.cursor) || '__first__'}|${normalizeText(item?.sourceUrl) || '__source__'}`;
      return itemKey !== pageKey;
    });
    filtered.push(snapshot);
    filtered.sort((a, b) => Number(a?.capturedAt || 0) - Number(b?.capturedAt || 0));
    store[key] = filtered.slice(-30);
  }

  function handleJson(url, json) {
    if (isSearchNotesApi(url)) {
      const snapshot = buildSearchNotesSnapshot(json, url);
      if (!snapshot) return;
      const current = Array.isArray(window.__lgboom_xhs_search_note_pages)
        ? window.__lgboom_xhs_search_note_pages
        : [];
      const snapshotKey = `${snapshot.keyword || '__keyword__'}|${snapshot.sourceUrl || '__source__'}|${snapshot.capturedAt}`;
      const filtered = current.filter((item) => {
        const itemKey = `${normalizeText(item?.keyword) || '__keyword__'}|${normalizeText(item?.sourceUrl) || '__source__'}|${Number(item?.capturedAt || 0)}`;
        return itemKey !== snapshotKey;
      });
      filtered.push(snapshot);
      filtered.sort((a, b) => Number(a?.capturedAt || 0) - Number(b?.capturedAt || 0));
      window.__lgboom_xhs_search_note_pages = filtered.slice(-12);
      return;
    }

    if (isProfileNotesApi(url)) {
      const snapshot = buildProfileNotesSnapshot(json, url);
      if (!snapshot) return;
      const current = Array.isArray(window.__lgboom_xhs_user_posted_pages)
        ? window.__lgboom_xhs_user_posted_pages
        : [];
      const snapshotKey = `${snapshot.userId || '__user__'}|${snapshot.cursor || '__first__'}|${snapshot.sourceUrl || '__source__'}`;
      const filtered = current.filter((item) => {
        const itemKey = `${normalizeText(item?.userId) || '__user__'}|${normalizeText(item?.cursor) || '__first__'}|${normalizeText(item?.sourceUrl) || '__source__'}`;
        return itemKey !== snapshotKey;
      });
      filtered.push(snapshot);
      filtered.sort((a, b) => Number(a?.capturedAt || 0) - Number(b?.capturedAt || 0));
      window.__lgboom_xhs_user_posted_pages = filtered.slice(-16);
      return;
    }

    const snapshot = buildSnapshot(json, url);
    if (!snapshot) return;

    if (snapshot.endpoint === 'sub') {
      const key = `${snapshot.noteId}::${snapshot.rootCommentId || '__root__'}`;
      upsertPage(window.__lgboom_xhs_sub_comment_pages, key, snapshot);
      return;
    }
    upsertPage(window.__lgboom_xhs_comment_pages, snapshot.noteId, snapshot);
  }

  function attachFetchHook() {
    if (typeof window.fetch !== 'function') return;
    const originalFetch = window.fetch.bind(window);
    window.fetch = async function (...args) {
      const response = await originalFetch(...args);
      try {
        const requestUrl = String(args?.[0]?.url || args?.[0] || response?.url || '');
        if (isRelevantApi(requestUrl)) {
          recordRequest(requestUrl, {
            status: Number(response?.status) || null,
            ok: Boolean(response?.ok),
            transport: 'fetch',
          });
          if (response?.ok) {
            response.clone().json().then((json) => {
              handleJson(requestUrl, json);
            }).catch(() => {});
          }
        }
      } catch {
        // ignore capture failures
      }
      return response;
    };
  }

  function attachXhrHook() {
    if (typeof XMLHttpRequest === 'undefined') return;
    const originalOpen = XMLHttpRequest.prototype.open;
    const originalSend = XMLHttpRequest.prototype.send;

    XMLHttpRequest.prototype.open = function (method, url, ...rest) {
      this.__lgboom_xhs_request_url = url;
      return originalOpen.call(this, method, url, ...rest);
    };

    XMLHttpRequest.prototype.send = function (...args) {
      this.addEventListener('load', function () {
        try {
          const requestUrl = String(this.__lgboom_xhs_request_url || this.responseURL || '');
          if (isRelevantApi(requestUrl)) {
            const status = Number(this.status) || null;
            const ok = this.status >= 200 && this.status < 300;
            recordRequest(requestUrl, { status, ok, transport: 'xhr' });
            if (ok) {
              const json = JSON.parse(String(this.responseText || '{}'));
              handleJson(requestUrl, json);
            }
          }
        } catch {
          // ignore capture failures
        }
      });
      this.addEventListener('error', function () {
        try {
          const requestUrl = String(this.__lgboom_xhs_request_url || this.responseURL || '');
          if (isRelevantApi(requestUrl)) recordRequest(requestUrl, { status: 0, ok: false, transport: 'xhr' });
        } catch {
          // ignore capture failures
        }
      });
      return originalSend.call(this, ...args);
    };
  }

  function respond(type, payload) {
    try {
      window.postMessage({
        source: BRIDGE_SOURCE,
        type,
        payload,
      }, '*');
    } catch {
      // ignore bridge failures
    }
  }

  async function fetchViaPage(urls = []) {
    let lastError = null;
    for (const url of Array.isArray(urls) ? urls : []) {
      const requestUrl = normalizeText(url);
      if (!requestUrl) continue;
      try {
        const response = await fetch(requestUrl, { credentials: 'include' });
        if (!response.ok) {
          lastError = new Error(`HTTP ${response.status}`);
          continue;
        }
        const json = await response.clone().json();
        handleJson(requestUrl, json);
        return json;
      } catch (err) {
        lastError = err;
      }
    }
    throw lastError || new Error('page_fetch_failed');
  }

  function handleBridgeMessage(event) {
    try {
      if (event.source !== window) return;
      const data = event.data || {};
      if (data.source !== REQUEST_SOURCE) return;

      if (data.type === SNAPSHOT_REQUEST_TYPE) {
        const requestId = normalizeText(data.payload?.requestId);
        const noteId = normalizeText(data.payload?.noteId);
        const pages = safeClone(window.__lgboom_xhs_comment_pages[noteId] || []) || [];
        const subPages = Object.entries(window.__lgboom_xhs_sub_comment_pages || {})
          .filter(([key]) => key.startsWith(`${noteId}::`))
          .flatMap(([, value]) => Array.isArray(value) ? value : [])
          .map((item) => safeClone(item))
          .filter(Boolean);
        respond(SNAPSHOT_RESPONSE_TYPE, {
          requestId,
          ok: true,
          noteId,
          pages,
          subPages,
        });
        return;
      }

      if (data.type === SNAPSHOT_RESET_REQUEST_TYPE) {
        const requestId = normalizeText(data.payload?.requestId);
        const noteId = normalizeText(data.payload?.noteId);
        delete window.__lgboom_xhs_comment_pages[noteId];
        Object.keys(window.__lgboom_xhs_sub_comment_pages || {})
          .filter((key) => key.startsWith(`${noteId}::`))
          .forEach((key) => { delete window.__lgboom_xhs_sub_comment_pages[key]; });
        respond(SNAPSHOT_RESET_RESPONSE_TYPE, {
          requestId,
          ok: true,
          noteId,
          reset: true,
        });
        return;
      }

      if (data.type === PROFILE_NOTES_REQUEST_TYPE) {
        const requestId = normalizeText(data.payload?.requestId);
        const userId = normalizeText(data.payload?.userId);
        const pages = (Array.isArray(window.__lgboom_xhs_user_posted_pages)
          ? window.__lgboom_xhs_user_posted_pages
          : [])
          .filter((item) => !userId || normalizeText(item?.userId) === userId)
          .map((item) => safeClone(item))
          .filter(Boolean);
        respond(PROFILE_NOTES_RESPONSE_TYPE, {
          requestId,
          ok: true,
          userId,
          pages,
        });
        return;
      }

      if (data.type === SEARCH_NOTES_REQUEST_TYPE) {
        const requestId = normalizeText(data.payload?.requestId);
        const keyword = normalizeText(data.payload?.keyword);
        const pages = (Array.isArray(window.__lgboom_xhs_search_note_pages)
          ? window.__lgboom_xhs_search_note_pages
          : [])
          .filter((item) => !keyword || normalizeText(item?.keyword) === keyword)
          .map((item) => safeClone(item))
          .filter(Boolean);
        respond(SEARCH_NOTES_RESPONSE_TYPE, {
          requestId,
          ok: true,
          keyword,
          pages,
        });
        return;
      }

      if (data.type === CAPTURE_PROBE_REQUEST_TYPE) {
        const requestId = normalizeText(data.payload?.requestId);
        const noteId = normalizeText(data.payload?.noteId);
        respond(CAPTURE_PROBE_RESPONSE_TYPE, {
          requestId,
          ok: true,
          probe: buildCaptureProbe(noteId),
        });
        return;
      }

      if (data.type === PAGE_FETCH_REQUEST_TYPE) {
        const requestId = normalizeText(data.payload?.requestId);
        fetchViaPage(data.payload?.urls)
          .then((json) => {
            respond(PAGE_FETCH_RESPONSE_TYPE, {
              requestId,
              ok: true,
              json: safeClone(json),
            });
          })
          .catch((error) => {
            respond(PAGE_FETCH_RESPONSE_TYPE, {
              requestId,
              ok: false,
              error: String(error?.message || error || 'page_fetch_failed'),
            });
          });
      }
    } catch {
      // ignore bridge failures
    }
  }

  attachFetchHook();
  attachXhrHook();
  window.addEventListener('message', handleBridgeMessage);
})();
