(() => {
  'use strict';

  const API_ROOT = '/api/local/work-resources';
  const laneOrder = [
    'discovery', 'detail', 'comments', 'replies', 'author',
    'media_slots', 'media_bytes', 'ocr', 'asr',
  ];
  const laneLabels = {
    discovery: '发现', detail: '详情', comments: '评论', replies: '回复', author: '作者',
    media_slots: '媒体槽位', media_bytes: '媒体字节', ocr: '图片文字', asr: '视频转录',
  };
  const stateLabels = {
    UNKNOWN: ['当前未知', 'unknown'],
    SOURCE_TEXT_ONLY: ['仅有来源时间文本', 'warning'],
    NOT_REQUESTED: ['尚未请求', 'unknown'],
    QUEUED: ['已排队', 'info'],
    NOT_OBSERVED: ['尚未形成观察', 'unknown'],
    OBSERVED: ['已观察', 'info'],
    PARTIAL: ['部分取得', 'warning'],
    ACQUIRED: ['已取得', 'success'],
    PROCESSING: ['处理中', 'info'],
    NOT_ENABLED: ['处理器未启用', 'unknown'],
    SEARCHABLE: ['可检索', 'success'],
    KNOWN_EMPTY: ['已处理，未识别到内容', 'info'],
    FAILED: ['执行失败', 'danger'],
    RISK_CONTROL: ['风险控制停止', 'danger'],
    BYTES_CLEANED: ['字节已清理', 'warning'],
    WITHDRAWN_OR_RESTRICTED: ['已撤回或限制读取', 'danger'],
    AVAILABLE: ['本地副本可用', 'success'],
  };
  const viewFilters = {
    all: {},
    partial: { laneState: 'PARTIAL' },
    risk: { laneState: 'RISK_CONTROL' },
    cleaned: { lane: 'media_bytes', laneState: 'BYTES_CLEANED' },
    restricted: { restriction: 'WITHDRAWN_OR_RESTRICTED' },
  };

  /* The five fixed material dimensions. `lane` names the observation this dimension reads, and
   * `applies` decides whether the dimension counts toward the denominator at all. A dimension
   * that provably cannot exist for this work — a transcript for a work with no video — is drawn
   * as an outline and excluded, so an image note reads 4/4 and never 4/5. A dimension we simply
   * have not looked at yet stays in the denominator as unknown, because "not checked" is not the
   * same claim as "does not apply". */
  const materialDimensions = [
    { key: 'body', label: '正文', lane: 'detail', applies: () => true },
    { key: 'comments', label: '评论', lane: 'comments', applies: () => true },
    { key: 'media', label: '媒体', lane: 'media_bytes', applies: () => true },
    { key: 'ocr', label: '图片文字', lane: 'ocr', applies: (facts) => !facts.mediaObserved || facts.hasImagery },
    { key: 'asr', label: '转录', lane: 'asr', applies: (facts) => !facts.mediaObserved || facts.hasVideo },
  ];
  const readyStates = new Set(['ACQUIRED', 'SEARCHABLE', 'COMPLETE', 'KNOWN_EMPTY']);
  const partialStates = new Set(['PARTIAL', 'BYTES_CLEANED']);
  const failedStates = new Set(['FAILED', 'RISK_CONTROL', 'WITHDRAWN_OR_RESTRICTED']);
  const pendingStates = new Set(['QUEUED', 'PROCESSING', 'OBSERVED', 'NOT_OBSERVED', 'NOT_REQUESTED', 'NOT_ENABLED']);

  const fragmentSourceLabels = {
    detail_body: '正文',
    comment_body: '评论',
    ocr_text: '图片文字',
    frame_ocr_text: '视频画面文字',
    asr_text: '视频转录',
  };

  const INSPECTOR_WIDTHS = ['normal', 'wide', 'focus'];
  const WIDTH_STORAGE_KEY = 'linggan.evidence.inspectorWidth';
  const DRAWER_QUERY = '(max-width: 1180px)';

  const refs = {
    form: document.getElementById('ev-query-form'),
    search: document.getElementById('ev-search'),
    sort: document.getElementById('ev-sort'),
    window: document.getElementById('ev-window'),
    lane: document.getElementById('ev-lane'),
    laneState: document.getElementById('ev-lane-state'),
    mediaKind: document.getElementById('ev-media-kind'),
    filterToggle: document.getElementById('ev-filter-toggle'),
    filterPanel: document.getElementById('ev-filter-panel'),
    filterCount: document.getElementById('ev-filter-count'),
    receipt: document.getElementById('ev-read-receipt'),
    bench: document.getElementById('ev-bench'),
    list: document.getElementById('ev-work-list'),
    feedback: document.getElementById('ev-feedback'),
    resultsCount: document.getElementById('ev-results-count'),
    resultsLegend: document.getElementById('ev-results-legend'),
    nextList: document.getElementById('ev-next-list'),
    inspector: document.getElementById('ev-inspector'),
    inspectorTitle: document.getElementById('ev-inspector-title'),
    inspectorSummary: document.getElementById('ev-inspector-summary'),
    inspectorRef: document.getElementById('ev-inspector-ref'),
    inspectorFeedback: document.getElementById('ev-inspector-feedback'),
    openSource: document.getElementById('ev-open-source'),
    requestMedia: document.getElementById('ev-request-media'),
    closeInspector: document.getElementById('ev-close-inspector'),
    reopenInspector: document.getElementById('ev-reopen-inspector'),
    back: document.getElementById('ev-back-to-list'),
    tableHead: document.getElementById('ev-table-head'),
    readout: {
      works: document.getElementById('ev-readout-works'),
      comments: document.getElementById('ev-readout-comments'),
      materials: document.getElementById('ev-readout-materials'),
      gap: document.getElementById('ev-readout-gap'),
    },
    lightbox: document.getElementById('ev-lightbox'),
    lightboxImage: document.getElementById('ev-lightbox-image'),
    lightboxTitle: document.getElementById('ev-lightbox-title'),
    lightboxRef: document.getElementById('ev-lightbox-ref'),
    lightboxCaption: document.getElementById('ev-lightbox-caption'),
    lightboxPrev: document.getElementById('ev-lightbox-prev'),
    lightboxNext: document.getElementById('ev-lightbox-next'),
    lightboxClose: document.getElementById('ev-lightbox-close'),
  };
  const panels = new Map(
    [...document.querySelectorAll('[data-ev-panel]')].map((panel) => [panel.dataset.evPanel, panel]),
  );
  const tabs = [...document.querySelectorAll('[data-ev-tab]')];
  const viewButtons = [...document.querySelectorAll('[data-ev-view]')];
  const layoutButtons = [...document.querySelectorAll('[data-ev-layout]')];
  const widthButtons = [...document.querySelectorAll('[data-ev-width]')];

  const model = {
    items: [],
    cursor: null,
    selectedRef: null,
    activeView: 'all',
    activeLayout: 'research',
    activeTab: 'overview',
    inspectorWidth: 'normal',
    inspectorClosed: false,
    drawerOpen: false,
    listController: null,
    detailController: null,
    commentController: null,
    commentUrl: null,
    commentCursor: null,
    commentItems: [],
    /* The current work's media objects, in the order the rail shows them. The lightbox reads
     * this rather than the DOM so opening it always lands on the object that was clicked. */
    mediaObjects: [],
    mediaIndex: 0,
    lightboxIndex: 0,
    lightboxOpener: null,
  };

  function node(tag, className, text) {
    const element = document.createElement(tag);
    if (className) element.className = className;
    if (text !== undefined && text !== null) element.textContent = String(text);
    return element;
  }

  function tech(text) {
    return node('span', 'v7-tech-key', text);
  }

  function addTextWithTech(parent, chinese, code) {
    parent.append(document.createTextNode(chinese));
    if (code) {
      parent.append(document.createTextNode(' '), tech(code));
    }
  }

  function knownText(value, state, unknownCopy = '当前未知') {
    return state === 'KNOWN' && value !== null && value !== undefined && value !== ''
      ? String(value)
      : unknownCopy;
  }

  function stateMeta(state) {
    return stateLabels[state] || ['来源未完整表达', 'unknown'];
  }

  function stateLine(state, extra) {
    const normalized = typeof state === 'string' && state ? state : 'UNKNOWN';
    const [label, tone] = stateMeta(normalized);
    const line = node('span', 'ev-state-line');
    const dot = node('i', 'ev-state-dot');
    dot.dataset.tone = tone;
    line.append(dot, node('span', null, label), tech(normalized));
    if (extra) line.append(tech(extra));
    return line;
  }

  function sameOriginPath(url, allowedPrefixes) {
    if (typeof url !== 'string' || !url) return null;
    try {
      const parsed = new URL(url, window.location.origin);
      if (parsed.origin !== window.location.origin) return null;
      return allowedPrefixes.some((prefix) => parsed.pathname.startsWith(prefix))
        ? `${parsed.pathname}${parsed.search}`
        : null;
    } catch (_) {
      return null;
    }
  }

  function knownMetric(value, state, label) {
    return state === 'KNOWN' && Number.isFinite(Number(value))
      ? `${label} ${Number(value).toLocaleString('zh-CN')}`
      : null;
  }

  function compactCount(value) {
    if (!Number.isFinite(value)) return '未知';
    if (value < 1000) return String(value);
    if (value < 1000000) return `${(value / 1000).toFixed(value < 10000 ? 1 : 0)}K`;
    return `${(value / 1000000).toFixed(1)}M`;
  }

  async function readJson(url, signal) {
    const response = await fetch(url, {
      method: 'GET',
      headers: { Accept: 'application/json' },
      credentials: 'same-origin',
      signal,
    });
    let payload = null;
    try {
      payload = await response.json();
    } catch (_) {
      payload = null;
    }
    if (!response.ok) {
      const error = new Error(payload?.code || `http_${response.status}`);
      error.code = payload?.code || 'local_read_unavailable';
      error.status = response.status;
      throw error;
    }
    return payload;
  }

  function reducedMotion() {
    return window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  }

  function isDrawerLayout() {
    return window.matchMedia(DRAWER_QUERY).matches;
  }

  /* ---------------------------------------------------------------- query */

  function explicitParams(cursor = null) {
    const params = new URLSearchParams();
    const query = refs.search.value.trim();
    if (query) params.set('q', query);
    if (refs.sort.value !== 'latest_discovery') params.set('sort', refs.sort.value);
    if (refs.window.value !== 'latest_accepted_discovery') params.set('window', refs.window.value);
    if (refs.lane.value) params.set('lane', refs.lane.value);
    if (refs.laneState.value) params.set('laneState', refs.laneState.value);
    if (refs.mediaKind.value) params.set('mediaKind', refs.mediaKind.value);
    if (cursor) params.set('cursor', cursor);
    return params;
  }

  function currentParams(cursor = null) {
    const params = explicitParams(cursor);
    const view = viewFilters[model.activeView] || {};
    Object.entries(view).forEach(([key, value]) => params.set(key, value));
    return params;
  }

  /* Everything a reader could restore travels in the address: the query, the quick view, the
   * layout, which work is open, which Inspector tab it is on and how wide the panel is. Back
   * and forward then return the same screen rather than a reset one. */
  function syncUrl(replace = true) {
    const params = explicitParams();
    if (model.activeView !== 'all') params.set('view', model.activeView);
    if (model.activeLayout !== 'research') params.set('layout', model.activeLayout);
    if (model.selectedRef) params.set('work', model.selectedRef);
    if (model.activeTab !== 'overview') params.set('tab', model.activeTab);
    if (model.inspectorClosed) params.set('panel', 'closed');
    else if (model.inspectorWidth !== 'normal') params.set('panel', model.inspectorWidth);
    const query = params.toString();
    const address = query ? `/corpus/evidence?${query}` : '/corpus/evidence';
    if (replace) history.replaceState(null, '', address);
    else history.pushState(null, '', address);
  }

  function activeFilterCount() {
    return [refs.window.value !== 'latest_accepted_discovery', refs.lane.value, refs.laneState.value, refs.mediaKind.value]
      .filter(Boolean).length;
  }

  function syncFilterCount() {
    refs.filterCount.textContent = String(activeFilterCount());
  }

  /* ---------------------------------------------------------- material rail */

  function laneSummary(item, lane) {
    return Array.isArray(item.laneSummaries)
      ? item.laneSummaries.find((summary) => summary?.lane === lane)
      : null;
  }

  function mediaFacts(item) {
    const media = item.media && typeof item.media === 'object' ? item.media : {};
    const images = Array.isArray(media.images) ? media.images : [];
    const covers = Array.isArray(media.coverCandidates) ? media.coverCandidates : [];
    const videos = Array.isArray(media.video?.items) ? media.video.items : [];
    const observed = typeof media.state === 'string' && media.state !== 'NOT_OBSERVED' && media.state !== 'UNKNOWN';
    return {
      mediaObserved: observed,
      hasImagery: images.length > 0 || covers.length > 0 || media.cover?.state === 'AVAILABLE',
      hasVideo: videos.length > 0,
      localObjectCount: [...images, ...covers, ...videos]
        .filter((slot) => sameOriginPath(slot?.localAssetUrl, ['/api/local/media/'])).length
        + (sameOriginPath(media.cover?.localAssetUrl, ['/api/local/media/']) ? 1 : 0),
    };
  }

  function materialFill(state) {
    if (readyStates.has(state)) return 'ready';
    if (partialStates.has(state)) return 'partial';
    if (failedStates.has(state)) return 'failed';
    if (pendingStates.has(state)) return 'pending';
    return 'unknown';
  }

  /* Applies the summary formula from the material contract:
   *   applicable = 状态 != NOT_APPLICABLE ; ready = COMPLETE | ACQUIRED ; summary = ready/applicable
   * `ready` is null rather than 0 when nothing is known, so the row can say 未知 instead of
   * printing a zero the system cannot stand behind. */
  function materialSummary(item) {
    const facts = mediaFacts(item);
    const segments = materialDimensions.map((dimension) => {
      const applicable = dimension.applies(facts);
      const summary = laneSummary(item, dimension.lane);
      const state = summary?.state || 'UNKNOWN';
      return {
        key: dimension.key,
        label: dimension.label,
        state,
        applicable,
        fill: applicable ? materialFill(state) : 'not-applicable',
      };
    });
    const applicable = segments.filter((segment) => segment.applicable);
    const ready = applicable.filter((segment) => segment.fill === 'ready').length;
    const anyKnown = applicable.some((segment) => segment.fill !== 'unknown');
    return {
      segments,
      applicable: applicable.length,
      ready: anyKnown ? ready : null,
      hasGap: anyKnown && ready < applicable.length,
      failed: segments.some((segment) => segment.fill === 'failed'),
      partial: segments.some((segment) => segment.fill === 'partial'),
      localObjectCount: facts.localObjectCount,
    };
  }

  function materialBlock(item) {
    const summary = materialSummary(item);
    const wrapper = node('div');
    const headline = node('div', 'ev-material-summary');
    headline.append(document.createTextNode('材料 '));
    const ratio = node('b', null, summary.ready === null
      ? `未知 / ${summary.applicable}`
      : `${summary.ready} / ${summary.applicable}`);
    headline.append(ratio);
    wrapper.append(headline);
    const rail = node('div', 'ev-material-rail');
    summary.segments.forEach((segment) => {
      const cell = node('i', 'ev-mat-seg');
      cell.dataset.fill = segment.fill;
      cell.title = segment.applicable
        ? `${segment.label}：${stateMeta(segment.state)[0]}`
        : `${segment.label}：本作品不适用，不计入分母`;
      rail.append(cell);
    });
    wrapper.append(rail);
    return { element: wrapper, summary };
  }

  /* ------------------------------------------------------------- readout */

  /* Describes this read, not the whole library. The read model deliberately has no total — it
   * works to a scan budget — so a library-wide number here would be invented. The labels say
   * 本次读取 for the same reason. */
  function renderReadout() {
    const works = model.items.length;
    let commentsRetained = null;
    let localObjects = 0;
    let gapCount = 0;
    model.items.forEach((item) => {
      const comments = laneSummary(item, 'comments');
      if (comments?.valueState === 'KNOWN' && Number.isFinite(Number(comments.retained))) {
        commentsRetained = (commentsRetained || 0) + Number(comments.retained);
      }
      const summary = materialSummary(item);
      localObjects += summary.localObjectCount;
      if (summary.hasGap || summary.failed) gapCount += 1;
    });
    refs.readout.works.textContent = works ? String(works) : '—';
    refs.readout.comments.textContent = commentsRetained === null ? '未知' : compactCount(commentsRetained);
    refs.readout.materials.textContent = works ? String(localObjects) : '—';
    refs.readout.gap.textContent = works ? String(gapCount) : '—';
  }

  /* ----------------------------------------------------------- result rows */

  function previewBlock(item) {
    const preview = node('div', 'ev-preview');
    const cover = item.media?.cover && typeof item.media.cover === 'object' ? item.media.cover : {};
    const controlledHandle = sameOriginPath(cover.localAssetUrl, ['/api/local/media/', '/api/local/derivative/']);
    const coverState = cover.state || 'NOT_OBSERVED';
    if (controlledHandle) {
      const image = node('img');
      image.src = controlledHandle;
      image.alt = knownText(item.display?.title, item.display?.titleState, '作品封面');
      image.loading = 'lazy';
      image.decoding = 'async';
      preview.append(image);
    } else {
      const [label] = stateMeta(coverState);
      preview.append(node('strong', null, label));
    }
    return preview;
  }

  /* The one serif line in a row. A search hit is highlighted in place using the character
   * offsets the read model returned — the excerpt is sliced with `Array.from`, because a CJK
   * codepoint is more than one JavaScript string index. */
  function evidenceBlock(item) {
    const fragment = item.evidenceFragment;
    const block = node('div', 'ev-evidence');
    if (!fragment || typeof fragment.text !== 'string' || !fragment.text) {
      block.classList.add('ev-evidence--absent');
      block.append(
        node('span', 'ev-evidence-source', '无原声'),
        node('span', null, '尚未取得可引用的原文、评论或图片文字'),
      );
      return block;
    }
    const label = fragmentSourceLabels[fragment.sourceKind] || '来源未表达';
    block.append(node('span', 'ev-evidence-source', label));
    const quote = node('q');
    const characters = Array.from(fragment.text);
    const offset = Number.isInteger(fragment.matchOffset) ? fragment.matchOffset : -1;
    const length = Number.isInteger(fragment.matchLength) ? fragment.matchLength : 0;
    if (offset >= 0 && length > 0 && offset + length <= characters.length) {
      quote.append(
        document.createTextNode(characters.slice(0, offset).join('')),
        node('mark', null, characters.slice(offset, offset + length).join('')),
        document.createTextNode(characters.slice(offset + length).join('')),
      );
    } else {
      quote.textContent = fragment.text;
    }
    block.append(quote);
    return block;
  }

  function engagementBlock(item) {
    const engagement = item.display?.engagement || {};
    const metrics = [
      ['赞', engagement.likeCount, engagement.likeCountState],
      ['评', engagement.commentCount, engagement.commentCountState],
      ['藏', engagement.collectCount, engagement.collectCountState],
      ['转', engagement.shareCount, engagement.shareCountState],
    ];
    const wrapper = node('div', 'ev-engagement');
    metrics.forEach(([label, value, state]) => {
      const cell = node('span', null, label);
      cell.append(node('b', null, state === 'KNOWN' && Number.isFinite(Number(value))
        ? Number(value).toLocaleString('zh-CN')
        : '未知'));
      wrapper.append(cell);
    });
    return wrapper;
  }

  function publishedCopy(item) {
    if (item.display?.publishedAtState === 'KNOWN') return item.display?.publishedAt || '发布时间已知';
    if (item.display?.publishedAtState === 'SOURCE_TEXT_ONLY') {
      return `来源时间：${item.display?.publishedAtSourceText || '已观察'}`;
    }
    return '发布时间当前未知';
  }

  function rowFor(item) {
    const publicRef = item.identity?.publicRef;
    const row = node('article', 'ev-work-row');
    row.setAttribute('role', 'option');
    row.tabIndex = -1;
    row.dataset.publicRef = publicRef || '';
    row.dataset.platform = String(item.identity?.platform || 'unknown').toLowerCase();
    row.setAttribute('aria-selected', String(publicRef === model.selectedRef));

    const creator = knownText(item.display?.creatorDisplayName, item.display?.creatorState, '作者当前未知');
    const target = item.collectionContext?.targetDisplayState === 'KNOWN'
      ? item.collectionContext.targetDisplayName
      : '监控目标当前未知';
    const published = publishedCopy(item);

    const identity = node('div', 'ev-identity');
    const eyebrow = node('div', 'ev-eyebrow');
    eyebrow.append(
      node('span', null, item.identity?.platform?.toUpperCase() || 'PLATFORM UNKNOWN'),
      node('span', null, publicRef ? `WORK ${publicRef.slice(0, 8).toUpperCase()}` : 'PUBLIC REF UNKNOWN'),
    );
    const title = node('h2', null, knownText(item.display?.title, item.display?.titleState, '标题当前未知'));
    const meta = node('div', 'ev-meta');
    meta.append(
      node('b', null, creator),
      document.createTextNode(' · '),
      node('span', 'ev-target-line', target),
      document.createTextNode(' · '),
      node('span', null, published),
    );
    identity.append(eyebrow, title, meta, evidenceBlock(item), engagementBlock(item));

    const material = materialBlock(item);
    const side = node('div', 'ev-side');
    side.append(material.element);
    const detailState = laneSummary(item, 'detail')?.state || 'UNKNOWN';
    side.append(stateLine(detailState));
    const observed = node('span', 'ev-observed', `最近观察 ${item.summary?.lastObservedAt || '未知'}`);
    side.append(observed);

    if (model.activeLayout === 'table') {
      row.append(
        tableCell(title.textContent, `${item.identity?.platform?.toUpperCase() || ''} ${publicRef ? publicRef.slice(0, 8).toUpperCase() : ''}`.trim()),
        tableCell(creator, target),
        material.element,
        stateLine(detailState),
        tableCell(published, ''),
        tableCell(item.summary?.lastObservedAt || '未知', ''),
      );
    } else {
      row.append(previewBlock(item), identity, side);
    }
    row.addEventListener('click', () => selectItem(item, true));
    row.addEventListener('keydown', (event) => onRowKeydown(event, item));
    return row;
  }

  function tableCell(primary, secondary) {
    const cell = node('div', 'ev-table-cell');
    cell.append(node('b', null, primary));
    if (secondary) cell.append(node('span', null, secondary));
    return cell;
  }

  function renderRows(appended) {
    if (!appended) refs.list.replaceChildren();
    refs.list.dataset.layout = model.activeLayout;
    refs.tableHead.hidden = model.activeLayout !== 'table';
    refs.tableHead.setAttribute('aria-hidden', String(model.activeLayout !== 'table'));
    refs.resultsLegend.textContent = model.activeLayout === 'table'
      ? '无封面 · 6 列 · 适合批量核查'
      : (model.activeLayout === 'cover' ? '图片主导 · 视觉供给研究' : '缩略图 · 最强证据 · 材料摘要');
    const existing = new Set([...refs.list.querySelectorAll('[data-public-ref]')].map((row) => row.dataset.publicRef));
    model.items.forEach((item) => {
      const publicRef = item.identity?.publicRef;
      if (publicRef && !existing.has(publicRef)) refs.list.append(rowFor(item));
    });
    const rows = [...refs.list.querySelectorAll('[data-public-ref]')];
    rows.forEach((row) => {
      const selected = row.dataset.publicRef === model.selectedRef;
      row.setAttribute('aria-selected', String(selected));
      row.tabIndex = selected ? 0 : -1;
    });
    if (rows.length && !rows.some((row) => row.tabIndex === 0)) rows[0].tabIndex = 0;
  }

  function setFeedback(kind, title, detail, code) {
    refs.feedback.hidden = false;
    refs.feedback.dataset.kind = kind;
    refs.feedback.replaceChildren();
    refs.feedback.append(node('h2', null, title), node('p', null, detail));
    if (code) refs.feedback.append(tech(code));
  }

  function clearFeedback() {
    refs.feedback.hidden = true;
    refs.feedback.replaceChildren();
  }

  function queryReceipt(payload, appended) {
    const count = model.items.length;
    refs.receipt.replaceChildren();
    const main = node('span');
    main.append(
      node('b', null, appended ? `已继续读取，当前显示 ${count} 个作品集合` : `读取成功，当前显示 ${count} 个作品集合`),
      document.createTextNode(` · 扫描 ${payload.scannedCount ?? '未知'} · 作品级材料投影`),
    );
    const meta = node('span', 'ev-receipt-meta');
    meta.append(tech(`AS OF ${payload.asOf || 'UNKNOWN'}`));
    if (payload.scanLimited) {
      meta.append(document.createTextNode(' · '));
      addTextWithTech(meta, '扫描预算已触发，可继续读取', 'SCAN LIMITED');
    }
    refs.receipt.append(main, meta);
    refs.resultsCount.textContent = `${count} 个作品材料`;
    refs.nextList.hidden = !payload.cursor;
    refs.nextList.disabled = !payload.cursor;
    refs.nextList.dataset.cursor = payload.cursor || '';
  }

  async function loadList({ append = false, keepSelection = false } = {}) {
    model.listController?.abort();
    model.listController = new AbortController();
    const cursor = append ? refs.nextList.dataset.cursor : null;
    const restoreRef = keepSelection ? model.selectedRef : null;
    if (!append) {
      model.items = [];
      model.cursor = null;
      if (!keepSelection) {
        model.selectedRef = null;
        clearInspector();
      }
      setFeedback('loading', '正在读取本机材料投影', '只读取 Linggan 已接纳的作品级材料；不会触发平台搜索或采集。', 'LOCAL READ');
      refs.list.replaceChildren();
      refs.resultsCount.textContent = '正在读取';
    }
    refs.nextList.disabled = true;
    try {
      const params = currentParams(cursor);
      const payload = await readJson(`${API_ROOT}?${params.toString()}`, model.listController.signal);
      if (!payload || !Array.isArray(payload.items)) throw new Error('invalid_material_projection_response');
      model.items = append ? model.items.concat(payload.items) : payload.items;
      model.cursor = payload.cursor || null;
      renderRows(append);
      queryReceipt(payload, append);
      renderReadout();
      syncUrl();
      if (model.items.length === 0) {
        setFeedback('empty', '当前查询没有匹配的作品材料', '读取已经成功；这个结果只描述当前本地查询，不证明平台或现实中没有相关内容。', 'NO MATCHING MATERIAL');
        clearInspector();
      } else {
        clearFeedback();
        const selected = model.items.find((item) => item.identity?.publicRef === (restoreRef || model.selectedRef))
          || model.items[0];
        await selectItem(selected, false);
      }
    } catch (error) {
      if (error.name === 'AbortError') return;
      model.items = [];
      renderRows(false);
      renderReadout();
      refs.resultsCount.textContent = '读取不可用';
      refs.nextList.hidden = true;
      setFeedback('error', '本机材料读取暂时不可用', '当前没有读取任何作品材料；页面不会回退到旧卡片、远程数据库或平台 CDN。', error.code || 'READ PROJECTION UNAVAILABLE');
      refs.receipt.replaceChildren();
      addTextWithTech(refs.receipt, '当前未读取任何材料，不能据此判断库为空或来源不存在。', error.code || 'READ PROJECTION UNAVAILABLE');
    }
  }

  function onRowKeydown(event, item) {
    const rows = [...refs.list.querySelectorAll('[data-public-ref]')];
    const current = event.currentTarget;
    const index = rows.indexOf(current);
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      selectItem(item, true);
      return;
    }
    if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) return;
    event.preventDefault();
    let next = current;
    if (event.key === 'Home') next = rows[0];
    if (event.key === 'End') next = rows[rows.length - 1];
    if (event.key === 'ArrowDown') next = rows[(index + 1) % rows.length];
    if (event.key === 'ArrowUp') next = rows[(index - 1 + rows.length) % rows.length];
    const nextItem = model.items.find((candidate) => candidate.identity?.publicRef === next.dataset.publicRef);
    if (nextItem) selectItem(nextItem, false).then(() => next.focus());
  }

  /* -------------------------------------------------------- inspector shell */

  function applyInspectorState() {
    refs.bench.dataset.inspector = model.inspectorClosed ? 'closed' : model.inspectorWidth;
    refs.bench.dataset.drawer = model.drawerOpen ? 'open' : 'closed';
    refs.reopenInspector.hidden = !model.inspectorClosed || isDrawerLayout();
    widthButtons.forEach((button) => {
      button.setAttribute('aria-pressed', String(!model.inspectorClosed && button.dataset.evWidth === model.inspectorWidth));
    });
  }

  function setInspectorWidth(width) {
    if (!INSPECTOR_WIDTHS.includes(width)) return;
    model.inspectorWidth = width;
    model.inspectorClosed = false;
    try {
      window.localStorage.setItem(WIDTH_STORAGE_KEY, width);
    } catch (_) {
      /* A private window or blocked site data must not break the panel. */
    }
    applyInspectorState();
    syncUrl();
  }

  function closeInspector() {
    if (isDrawerLayout()) {
      model.drawerOpen = false;
    } else {
      model.inspectorClosed = true;
    }
    applyInspectorState();
    syncUrl();
    const row = refs.list.querySelector(`[data-public-ref="${CSS.escape(model.selectedRef || '')}"]`);
    if (row) row.focus({ preventScroll: true });
  }

  function openInspector() {
    if (isDrawerLayout()) model.drawerOpen = true;
    model.inspectorClosed = false;
    applyInspectorState();
    syncUrl();
  }

  function restoreInspectorWidth() {
    let stored = null;
    try {
      stored = window.localStorage.getItem(WIDTH_STORAGE_KEY);
    } catch (_) {
      stored = null;
    }
    if (INSPECTOR_WIDTHS.includes(stored)) model.inspectorWidth = stored;
  }

  function clearInspector() {
    model.detailController?.abort();
    model.commentController?.abort();
    model.commentUrl = null;
    model.commentCursor = null;
    model.commentItems = [];
    model.mediaObjects = [];
    model.mediaIndex = 0;
    refs.inspectorRef.textContent = 'SELECTION REQUIRED';
    refs.inspectorTitle.textContent = '请选择一个作品材料集合';
    refs.inspectorSummary.textContent = '右侧只核验当前选择，不补造未读取的详情。';
    refs.openSource.disabled = true;
    refs.requestMedia.disabled = true;
    refs.inspectorFeedback.hidden = false;
    refs.inspectorFeedback.replaceChildren(node('strong', null, '尚未选择作品'), tech('SELECTION REQUIRED'));
    panels.forEach((panel) => panel.replaceChildren());
  }

  async function selectItem(item, fromUser) {
    const publicRef = item?.identity?.publicRef;
    const detailUrl = sameOriginPath(item?.detailUrl, [`${API_ROOT}/`]);
    if (!publicRef || !detailUrl || detailUrl.endsWith('/comments')) {
      showInspectorSourceIncomplete(item, '列表没有提供可用的同源 detailUrl。');
      return;
    }
    model.selectedRef = publicRef;
    [...refs.list.querySelectorAll('[data-public-ref]')].forEach((row) => {
      const selected = row.dataset.publicRef === publicRef;
      row.setAttribute('aria-selected', String(selected));
      row.tabIndex = selected ? 0 : -1;
    });
    /* Choosing a work always brings the panel back — a reader who closed it and then clicked a
     * row is asking to see that work, not to keep the panel shut. */
    if (fromUser) openInspector();
    else syncUrl();
    refs.inspectorFeedback.hidden = false;
    refs.inspectorFeedback.replaceChildren(node('strong', null, '正在读取当前作品详情'), tech('DETAIL READ'));
    refs.inspectorTitle.textContent = knownText(item.display?.title, item.display?.titleState, '标题当前未知');
    refs.inspectorRef.textContent = `${(item.identity?.platform || 'unknown').toUpperCase()} / WORK ${publicRef.slice(0, 8).toUpperCase()}`;
    refs.inspectorSummary.textContent = '详情、证据、材料与来源分别保留自己的读取回执。';
    model.detailController?.abort();
    model.detailController = new AbortController();
    try {
      const payload = await readJson(detailUrl, model.detailController.signal);
      if (!payload?.item || typeof payload.item !== 'object') throw new Error('invalid_material_detail_response');
      renderDetail(item, payload.item, payload.channels || {});
      refs.inspectorFeedback.hidden = true;
    } catch (error) {
      if (error.name === 'AbortError') return;
      refs.inspectorFeedback.hidden = false;
      refs.inspectorFeedback.replaceChildren(
        node('strong', null, '当前作品详情读取失败'),
        tech(error.code || 'MATERIAL DETAIL UNAVAILABLE'),
      );
      panels.forEach((panel) => panel.replaceChildren(sourceIncompleteBlock('当前详情未读取，列表通道仍可查看，但不能据此补造 Inspector。')));
    }
  }

  function showInspectorSourceIncomplete(item, detail) {
    model.selectedRef = item?.identity?.publicRef || null;
    refs.inspectorRef.textContent = model.selectedRef ? `WORK ${model.selectedRef.slice(0, 8).toUpperCase()}` : 'SELECTION REQUIRED';
    refs.inspectorTitle.textContent = knownText(item?.display?.title, item?.display?.titleState, '标题当前未知');
    refs.inspectorSummary.textContent = detail;
    refs.inspectorFeedback.hidden = false;
    refs.inspectorFeedback.replaceChildren(node('strong', null, '来源信息不完整'), tech('SOURCE INCOMPLETE'));
    panels.forEach((panel) => panel.replaceChildren(sourceIncompleteBlock(detail)));
  }

  function sourceIncompleteBlock(detail) {
    const block = node('section', 'ev-inline-state');
    block.dataset.tone = 'warning';
    block.append(node('strong', null, '来源信息不完整'), node('p', null, detail), tech('SOURCE INCOMPLETE'));
    return block;
  }

  function factGrid(rows) {
    const list = node('dl', 'ev-facts');
    rows.forEach(([label, value, code]) => {
      list.append(node('dt', null, label));
      const value_ = node('dd');
      addTextWithTech(value_, value, code);
      list.append(value_);
    });
    return list;
  }

  function section(title, code) {
    const wrapper = node('section', 'ev-inspect-section');
    const heading = node('h3');
    heading.append(node('span', null, title));
    if (code) heading.append(tech(code));
    wrapper.append(heading);
    return wrapper;
  }

  function laneCell(item, lane) {
    const summary = laneSummary(item, lane);
    const cell = node('div', 'ev-lane');
    cell.append(node('span', 'ev-lane-name', laneLabels[lane]));
    const count = node('span', 'ev-lane-count');
    count.textContent = summary?.valueState === 'KNOWN' && summary.retained !== null && summary.retained !== undefined
      ? `保留 ${summary.retained}`
      : '数量未知';
    cell.append(count, stateLine(summary?.state || 'UNKNOWN'));
    return cell;
  }

  function renderDetail(listItem, item, channels) {
    const inspector = item.inspector && typeof item.inspector === 'object' ? item.inspector : {};
    refs.inspectorTitle.textContent = knownText(item.display?.title, item.display?.titleState, '标题当前未知');
    const creator = knownText(item.display?.creatorDisplayName, item.display?.creatorState, '作者当前未知');
    refs.inspectorSummary.textContent = `${creator} · 最近观察 ${item.summary?.lastObservedAt || '未知'} · 主要限制 ${item.summary?.primaryLimitation || '未表达'}`;
    /* Both actions describe capabilities this build does not have. They stay disabled and say
     * why rather than becoming buttons that quietly do nothing. */
    refs.openSource.disabled = true;
    refs.openSource.title = '本机只读投影不保存平台原始地址，无法从这里跳转。';
    refs.requestMedia.disabled = true;
    refs.requestMedia.title = '补采需要采集授权，本页只读，不触发平台访问。';
    renderOverview(item, inspector);
    renderEvidence(listItem, item, inspector, channels.comments || {});
    renderMaterials(item, inspector, channels);
    renderTrace(inspector, channels.provenance || {});
  }

  function renderOverview(item, inspector) {
    const panel = panels.get('overview');
    panel.replaceChildren();

    const summaryBlock = materialBlock(item);
    const overview = section('材料完整度', 'MATERIAL SUMMARY');
    overview.append(summaryBlock.element);
    const legend = node('div', 'ev-media-hint');
    legend.append(
      node('span', null, materialDimensions.map((dimension) => dimension.label).join(' · ')),
      node('span', null, summaryBlock.summary.segments.some((segment) => !segment.applicable)
        ? '空心格为本作品不适用，不计入分母'
        : '五个维度全部适用于本作品'),
    );
    overview.append(legend);
    panel.append(overview);

    const identity = section('作品字段与来源', 'WORK MATERIAL');
    identity.append(factGrid([
      ['稳定引用', item.identity?.publicRef || '当前未知', 'PUBLIC REF'],
      ['平台', item.identity?.platform || '当前未知', 'PLATFORM'],
      ['标题', knownText(item.display?.title, item.display?.titleState), item.display?.titleState || 'UNKNOWN'],
      ['作品作者', knownText(item.display?.creatorDisplayName, item.display?.creatorState), item.display?.creatorState || 'UNKNOWN'],
      ['监控目标', item.collectionContext?.targetDisplayState === 'KNOWN' ? item.collectionContext.targetDisplayName : '当前未知', item.collectionContext?.relationshipState || 'UNKNOWN'],
      ['作者身份关系', item.collectionContext?.authorIdentityMatchState === 'MATCHED' ? '已由平台作者 ID 证明一致' : '尚未证明监控目标就是作品作者', item.collectionContext?.authorIdentityMatchState || 'NOT_VERIFIED'],
      ['发布时间', publishedCopy(item), item.display?.publishedAtState || 'UNKNOWN'],
      ['时间来源字段', item.display?.publishedAtSourceField || '当前未知', item.display?.publishedAtSourceKind || 'unknown'],
      ['时间精度', item.display?.publishedAtPrecision || 'unknown', item.display?.publishedAtParserVersion || 'PARSER UNKNOWN'],
      ['最近观察', item.summary?.lastObservedAt || '当前未知', 'OBSERVED AT'],
    ]));
    panel.append(identity);

    /* Platform-reported counts and locally retained counts are two different facts and are
     * never merged into one number. */
    const platform = section('平台计数与本地留存', 'PLATFORM VS LOCAL');
    const comments = inspector.commentsCoverage || {};
    platform.append(factGrid([
      ['平台点赞', knownText(item.display?.engagement?.likeCount, item.display?.engagement?.likeCountState), item.display?.engagement?.likeCountState || 'UNKNOWN'],
      ['平台评论数', knownText(item.display?.engagement?.commentCount, item.display?.engagement?.commentCountState), item.display?.engagement?.commentCountState || 'UNKNOWN'],
      ['平台收藏', knownText(item.display?.engagement?.collectCount, item.display?.engagement?.collectCountState), item.display?.engagement?.collectCountState || 'UNKNOWN'],
      ['平台分享', knownText(item.display?.engagement?.shareCount, item.display?.engagement?.shareCountState), item.display?.engagement?.shareCountState || 'UNKNOWN'],
      ['本地留存评论', Number.isFinite(Number(comments.retained)) ? comments.retained : '当前未知', 'RETAINED LOCALLY'],
      ['已去重评论身份', Number.isFinite(Number(comments.uniqueCollectedCount)) ? comments.uniqueCollectedCount : '当前未知', 'UNIQUE COLLECTED'],
      ['本地留存边界', comments.stoppedReason ? `停止原因 ${comments.stoppedReason}` : '当前未表达', comments.collectionState || 'UNKNOWN'],
    ]));
    panel.append(platform);

    const lanes = section('材料通道状态', 'LANE STATUS');
    const board = node('div', 'ev-detail-lanes');
    laneOrder.forEach((lane) => board.append(laneCell(item, lane)));
    lanes.append(board);
    panel.append(lanes);

    const timeline = section('互动数据观察时间线', 'ENGAGEMENT TIMELINE');
    const observations = Array.isArray(inspector.engagementTimeline) ? inspector.engagementTimeline : [];
    if (observations.length === 0) {
      timeline.append(sourceIncompleteBlock('当前详情没有互动数据观察时点；不把未知写成 0。'));
    } else {
      observations.forEach((observation) => {
        const entry = node('article', 'ev-derivative');
        const metrics = [
          knownMetric(observation.likeCount, observation.likeCountState, '赞'),
          knownMetric(observation.commentCount, observation.commentCountState, '评'),
          knownMetric(observation.collectCount, observation.collectCountState, '藏'),
          knownMetric(observation.shareCount, observation.shareCountState, '转'),
        ].filter(Boolean);
        const head = node('div', 'ev-derivative-head');
        head.append(node('strong', null, observation.observedAt || '观察时间当前未知'), tech((observation.sourceLane || 'source').toUpperCase()));
        entry.append(head, node('p', null, metrics.length ? metrics.join(' · ') : '本时点互动字段均为当前未知'));
        timeline.append(entry);
      });
    }
    panel.append(timeline);

    const author = section('作者上下文', 'VERSIONED CONTEXT');
    const context = inspector.authorContext;
    if (!context || typeof context !== 'object') {
      author.append(sourceIncompleteBlock('当前详情没有合格作者上下文；这不表示作者不存在。'));
    } else {
      author.append(factGrid([
        ['显示名', knownText(context.displayName, context.displayNameState), context.displayNameState || 'UNKNOWN'],
        ['简介', context.biographyState === 'KNOWN' ? '已形成状态化上下文' : '当前未知', context.biographyState || 'UNKNOWN'],
        ['关注者数', context.followerCountState === 'KNOWN' && context.followerCount !== null ? context.followerCount : '当前未知', context.followerCountState || 'UNKNOWN'],
        ['观察时间', context.observedAt || '当前未知', 'OBSERVED AT'],
      ]));
    }
    panel.append(author);
  }

  /* ------------------------------------------------------------- evidence */

  function derivedTexts(inspector) {
    return (Array.isArray(inspector.derivatives) ? inspector.derivatives : [])
      .filter((derivative) => typeof derivative?.displayText === 'string' && derivative.displayText.trim())
      .sort((left, right) => String(left.slotKey || '').localeCompare(String(right.slotKey || '')));
  }

  function renderEvidence(listItem, item, inspector, commentChannel) {
    const panel = panels.get('evidence');
    panel.replaceChildren();

    const strongest = section('列表引用的原声', 'ROW EXCERPT');
    const fragment = listItem?.evidenceFragment;
    if (!fragment) {
      strongest.append(sourceIncompleteBlock('这个作品还没有可引用的原文、评论或图片文字。这只描述当前已接纳的材料，不表示平台上没有内容。'));
    } else {
      const quote = node('div', 'ev-evidence');
      quote.append(
        node('span', 'ev-evidence-source', fragmentSourceLabels[fragment.sourceKind] || '来源未表达'),
        node('q', null, fragment.text),
      );
      strongest.append(quote);
      strongest.append(factGrid([
        ['来源类型', fragmentSourceLabels[fragment.sourceKind] || '未表达', fragment.sourceKind || 'UNKNOWN'],
        ['选取依据', fragment.selectionBasis === 'SEARCH_MATCH' ? '包含当前检索词' : '当前最具作者性的可读材料', fragment.selectionBasis || 'UNKNOWN'],
        ['是否截断', fragment.truncated ? '是，已按展示边界截断' : '否', fragment.truncated ? 'TRUNCATED' : 'WHOLE'],
        ['来源引用', fragment.sourceRef || '当前未表达', fragment.slotKey || 'NO SLOT'],
      ]));
    }
    panel.append(strongest);

    /* Raw material is shown as-is. Nothing on this page replaces an original sentence with a
     * generated summary of it. */
    const texts = derivedTexts(inspector);
    const machine = section('图片文字与转录原文', 'MACHINE READ');
    if (texts.length === 0) {
      machine.append(sourceIncompleteBlock('当前详情没有已识别的图片文字或转录文本；不把缺少记录写成处理成功或失败。'));
    } else {
      texts.forEach((derivative) => {
        const entry = node('article', 'ev-derivative');
        const head = node('div', 'ev-derivative-head');
        head.append(
          node('strong', null, fragmentSourceLabels[derivative.kind] || derivative.kind || '派生类型当前未知'),
          tech(derivative.slotKey || 'SLOT UNKNOWN'),
        );
        entry.append(head, node('q', null, derivative.displayText), stateLine(derivative.state || 'UNKNOWN'));
        machine.append(entry);
      });
    }
    panel.append(machine);

    const channelUrl = sameOriginPath(commentChannel?.url, [`${API_ROOT}/`]);
    const discussion = section('读者原声', 'LOCAL AUTHORIZED RESEARCH');
    const access = node('div', 'ev-inline-state');
    access.dataset.tone = 'restricted';
    access.append(
      node('strong', null, '原文只在本机授权详情中按页读取'),
      node('p', null, '作者只显示匿名上下文；页面不会渲染平台用户标识。'),
      tech('IDENTITY WITHHELD'),
    );
    discussion.append(access);
    const receipt = commentChannel?.receipt || inspector.commentsReceipt;
    discussion.append(factGrid([
      ['平台评论数', inspector.commentsCoverage?.expectedCount ?? '当前未知', 'PLATFORM COUNT'],
      ['本地已保存', receipt?.total ?? '当前未知', 'RETAINED LOCALLY'],
    ]));
    const controls = node('div', 'ev-channel-actions');
    const load = node('button', 'ev-button ev-button--secondary', '读取评论原文');
    load.type = 'button';
    load.disabled = !channelUrl;
    load.addEventListener('click', () => loadComments(false));
    controls.append(load);
    if (!channelUrl) controls.append(tech('SOURCE INCOMPLETE'));
    discussion.append(controls, node('div', 'ev-comment-list'));
    panel.append(discussion);
    model.commentUrl = channelUrl;
    model.commentCursor = null;
    model.commentItems = [];
  }

  async function loadComments(append) {
    if (!model.commentUrl) return;
    model.commentController?.abort();
    model.commentController = new AbortController();
    const panel = panels.get('evidence');
    const list = panel.querySelector('.ev-comment-list');
    const actions = panel.querySelector('.ev-channel-actions');
    const url = new URL(model.commentUrl, window.location.origin);
    if (append && model.commentCursor) url.searchParams.set('cursor', model.commentCursor);
    actions.replaceChildren(node('span', 'ev-loading-copy', append ? '正在继续读取评论……' : '正在读取评论……'), tech('LOCAL READ'));
    try {
      const payload = await readJson(`${url.pathname}${url.search}`, model.commentController.signal);
      if (!payload || !Array.isArray(payload.items)) throw new Error('invalid_comment_channel_response');
      model.commentItems = append ? model.commentItems.concat(payload.items) : payload.items;
      model.commentCursor = payload.nextCursor || null;
      renderComments(list, payload);
      actions.replaceChildren();
      const receipt = node('span', 'ev-channel-inline-receipt');
      receipt.textContent = `总数 ${payload.total ?? '未知'} · 当前 ${model.commentItems.length} · 本次 ${payload.returned ?? '未知'}`;
      actions.append(receipt);
      if (payload.truncated && payload.nextCursor) {
        const next = node('button', 'ev-button ev-button--secondary', '继续读取评论');
        next.type = 'button';
        next.addEventListener('click', () => loadComments(true));
        actions.append(next);
      }
    } catch (error) {
      if (error.name === 'AbortError') return;
      actions.replaceChildren();
      const state = node('div', 'ev-inline-state');
      state.dataset.tone = 'danger';
      state.append(node('strong', null, '评论研究通道读取失败'), tech(error.code || 'COMMENT CHANNEL UNAVAILABLE'));
      actions.append(state);
    }
  }

  function renderComments(list, payload) {
    list.replaceChildren();
    if (model.commentItems.length === 0) {
      const empty = node('div', 'ev-inline-state');
      empty.append(node('strong', null, '当前通道没有返回评论材料'), node('p', null, '这只描述本次授权读取，不表示平台评论为 0。'), tech('NO RETURNED MATERIAL'));
      list.append(empty);
      return;
    }
    model.commentItems.forEach((comment) => {
      const article = node('article', 'ev-comment');
      const header = node('div', 'ev-comment-head');
      header.append(
        node('strong', null, comment.relation === 'REPLY' ? '匿名回复' : '匿名评论'),
        tech(comment.sourceRef ? `SOURCE ${String(comment.sourceRef).slice(0, 8).toUpperCase()}` : 'SOURCE INCOMPLETE'),
      );
      const body = node('q', null, comment.bodyState === 'KNOWN' && comment.body !== null ? comment.body : '评论正文当前未知');
      const meta = node('div', 'ev-comment-meta');
      addTextWithTech(meta, comment.bodyTruncated ? '本条正文已在读取边界截断' : '本条正文未在通道内截断', comment.bodyTruncated ? 'TRUNCATED' : 'RETURNED');
      article.append(header, body, meta);
      list.append(article);
    });
    const receipt = node('div', 'ev-comment-receipt');
    addTextWithTech(receipt, `总数 ${payload.total ?? '未知'} · 返回 ${payload.returned ?? '未知'} · ${payload.truncated ? '可继续读取' : '当前页未截断'}`, 'ANONYMOUS RESEARCH VIEW');
    list.append(receipt);
  }

  /* -------------------------------------------------------- LOCAL MEDIA */

  /* Builds the rail's object list. The author's avatar is excluded — it belongs to the author,
   * not to this work's material. Ordering is by the producer's own sequence, which is the only
   * order the system actually holds; `displayOrderState` is UNKNOWN upstream, so the rail says
   * 采集顺序 and never claims the platform's arrangement. */
  function mediaObjectsFrom(inspector, fragment) {
    const slots = Array.isArray(inspector.mediaSlots) ? inspector.mediaSlots : [];
    const texts = new Map();
    (Array.isArray(inspector.derivatives) ? inspector.derivatives : []).forEach((derivative) => {
      if (!derivative?.slotKey) return;
      const existing = texts.get(derivative.slotKey) || { state: null, text: null };
      if (derivative.kind === 'ocr_text' || derivative.kind === 'frame_ocr_text' || derivative.kind === 'asr_text') {
        existing.state = derivative.state || 'UNKNOWN';
        if (typeof derivative.displayText === 'string' && derivative.displayText.trim()) {
          existing.text = derivative.displayText;
        }
      }
      texts.set(derivative.slotKey, existing);
    });
    return slots
      .filter((slot) => slot?.purpose !== 'author_avatar')
      .sort((left, right) => {
        const leftOrdinal = Number(left.producerOrdinal ?? left.relationshipOrdinal ?? 0);
        const rightOrdinal = Number(right.producerOrdinal ?? right.relationshipOrdinal ?? 0);
        if (leftOrdinal !== rightOrdinal) return leftOrdinal - rightOrdinal;
        return String(left.slotKey || '').localeCompare(String(right.slotKey || ''));
      })
      .map((slot, index) => {
        const derived = texts.get(slot.slotKey) || {};
        const blob = slot.blob && typeof slot.blob === 'object' ? slot.blob : null;
        const mime = blob?.deliveryMimeType || blob?.declaredMimeType || '';
        return {
          index,
          slot,
          slotKey: slot.slotKey || null,
          purpose: slot.purpose || 'unknown',
          ordinal: slot.producerOrdinal ?? slot.relationshipOrdinal ?? index + 1,
          objectRef: slot.replica?.materializationRef || null,
          bytesState: slot.bytesState || 'UNKNOWN',
          derivedState: derived.state,
          derivedText: derived.text || null,
          isMatch: Boolean(fragment?.slotKey && fragment.slotKey === slot.slotKey),
          assetUrl: blob?.deliveryState === 'INLINE_SAFE' && mime.startsWith('image/')
            ? sameOriginPath(slot.localAssetUrl, ['/api/local/media/'])
            : null,
          inlineBlocked: Boolean(slot.localAssetUrl) && blob?.deliveryState !== 'INLINE_SAFE',
          mime,
        };
      });
  }

  function purposeLabel(purpose) {
    return { cover: '封面', body_image: '正文图片', video: '视频', live_photo: '实况图片', author_avatar: '作者头像' }[purpose] || purpose;
  }

  function renderMediaRail(objects) {
    const module = node('section', 'ev-media-module');
    const toolbar = node('div', 'ev-media-toolbar');
    const summary = node('div', 'ev-media-summary');
    summary.append(
      node('b', null, `${String(objects.length).padStart(2, '0')} 个本地媒体对象`),
      node('span', null, '采集顺序 · 平台排列顺序未经验证'),
    );
    const nav = node('div', 'ev-media-nav');
    const previous = node('button', null, '←');
    previous.type = 'button';
    previous.setAttribute('aria-label', '上一个媒体对象');
    const position = node('span', 'ev-media-position');
    const next = node('button', null, '→');
    next.type = 'button';
    next.setAttribute('aria-label', '下一个媒体对象');
    nav.append(previous, position, next);
    const matchIndex = objects.findIndex((object) => object.isMatch);
    if (matchIndex >= 0) {
      const jump = node('button', 'ev-media-jump', 'GO TO MATCH');
      jump.type = 'button';
      jump.addEventListener('click', () => scrollToObject(matchIndex, true));
      nav.insertBefore(jump, previous);
    }
    toolbar.append(summary, nav);
    module.append(toolbar);

    const viewport = node('div', 'ev-media-viewport');
    viewport.tabIndex = 0;
    viewport.setAttribute('role', 'group');
    viewport.setAttribute('aria-label', '本地媒体对象，左右方向键切换');
    const strip = node('div', 'ev-media-strip');
    objects.forEach((object) => strip.append(mediaCard(object, objects.length)));
    viewport.append(strip);
    module.append(viewport);

    const progress = node('div', 'ev-media-progress');
    const bar = node('i');
    progress.append(bar);
    module.append(progress);

    const hint = node('div', 'ev-media-hint');
    hint.append(node('span', null, '拖拽 / 滚轮 / ← →'), node('em', null, '点击图片进入原尺寸查看器'));
    module.append(hint);

    bindMediaControls({ viewport, strip, previous, next, position, bar, objects });
    return module;
  }

  function mediaCard(object, total) {
    const card = node('button', 'ev-media-card');
    card.type = 'button';
    card.dataset.index = String(object.index);
    card.dataset.match = String(object.isMatch);
    const frame = node('div', 'ev-media-frame');
    if (object.assetUrl) {
      const image = node('img');
      image.src = object.assetUrl;
      image.alt = `${purposeLabel(object.purpose)} 第 ${object.ordinal} 个本地媒体对象`;
      image.loading = 'lazy';
      image.decoding = 'async';
      /* One broken object must never blank the whole work's media. The card keeps its identity
       * and states the failure in place. */
      image.addEventListener('error', () => {
        image.remove();
        frame.append(node('p', 'ev-media-absent', '这个对象的本地副本读取失败；其余对象不受影响。'));
      });
      frame.append(image);
    } else if (object.inlineBlocked) {
      frame.append(node('p', 'ev-media-absent', `本地副本已验证，但 ${object.mime || '该类型'} 不在可内联集合中。`));
    } else {
      frame.append(node('p', 'ev-media-absent', stateMeta(object.bytesState)[0]));
    }
    frame.append(node('span', 'ev-media-index', `${String(object.index + 1).padStart(2, '0')} / ${String(total).padStart(2, '0')}`));
    card.append(frame);
    const meta = node('div', 'ev-media-meta');
    meta.append(
      node('b', null, object.objectRef ? `MED-${String(object.objectRef).slice(0, 8).toUpperCase()}` : '本地对象 ID 未表达'),
      node('span', null, object.derivedState ? `文字 ${stateMeta(object.derivedState)[0]}` : '文字未处理'),
    );
    card.append(meta);
    card.addEventListener('click', () => openLightbox(object.index, card));
    return card;
  }

  /* Scrolls the rail itself rather than calling `scrollIntoView` on the card. The card's offset
   * parent is not the viewport, so `scrollIntoView` can settle the panel vertically without ever
   * moving the horizontal rail — which is how "GO TO MATCH" ends up focusing the right object
   * while still showing the first one. */
  function scrollToObject(index, focus) {
    const viewport = refs.inspector.querySelector('.ev-media-viewport');
    const card = viewport?.querySelector('.ev-media-strip')?.children[index];
    if (!viewport || !card) return;
    viewport.scrollTo({ left: card.offsetLeft, behavior: reducedMotion() ? 'auto' : 'smooth' });
    if (focus) card.focus({ preventScroll: true });
  }

  function bindMediaControls({ viewport, strip, previous, next, position, bar, objects }) {
    const total = objects.length;

    function currentIndex() {
      const cards = [...strip.children];
      const left = viewport.scrollLeft;
      let closest = 0;
      let distance = Infinity;
      cards.forEach((card, index) => {
        const delta = Math.abs(card.offsetLeft - left);
        if (delta < distance) {
          distance = delta;
          closest = index;
        }
      });
      return closest;
    }

    function sync() {
      const index = currentIndex();
      model.mediaIndex = index;
      position.textContent = `${String(index + 1).padStart(2, '0')} / ${String(total).padStart(2, '0')}`;
      const maximum = viewport.scrollWidth - viewport.clientWidth;
      const ratio = maximum > 0 ? Math.min(1, Math.max(0, viewport.scrollLeft / maximum)) : 1;
      bar.style.transform = `scaleX(${total > 1 ? ratio * (1 - 1 / total) + 1 / total : 1})`;
      previous.disabled = index <= 0;
      next.disabled = index >= total - 1;
      [...strip.children].forEach((card, cardIndex) => {
        card.dataset.current = String(cardIndex === index);
      });
    }

    function step(delta) {
      const target = Math.min(total - 1, Math.max(0, currentIndex() + delta));
      const card = strip.children[target];
      if (card) viewport.scrollTo({ left: card.offsetLeft, behavior: reducedMotion() ? 'auto' : 'smooth' });
    }

    previous.addEventListener('click', () => step(-1));
    next.addEventListener('click', () => step(1));
    viewport.addEventListener('scroll', () => window.requestAnimationFrame(sync), { passive: true });

    /* A vertical wheel over a horizontal rail should move the rail, not the panel behind it. */
    viewport.addEventListener('wheel', (event) => {
      if (Math.abs(event.deltaY) <= Math.abs(event.deltaX)) return;
      const maximum = viewport.scrollWidth - viewport.clientWidth;
      if (maximum <= 0) return;
      const atStart = viewport.scrollLeft <= 0 && event.deltaY < 0;
      const atEnd = viewport.scrollLeft >= maximum - 1 && event.deltaY > 0;
      if (atStart || atEnd) return;
      event.preventDefault();
      viewport.scrollLeft += event.deltaY;
    }, { passive: false });

    let dragging = false;
    let dragOrigin = 0;
    let scrollOrigin = 0;
    let moved = 0;
    viewport.addEventListener('pointerdown', (event) => {
      if (event.pointerType === 'touch') return;
      dragging = true;
      moved = 0;
      dragOrigin = event.clientX;
      scrollOrigin = viewport.scrollLeft;
      viewport.dataset.grabbing = 'true';
      viewport.setPointerCapture(event.pointerId);
    });
    viewport.addEventListener('pointermove', (event) => {
      if (!dragging) return;
      const delta = event.clientX - dragOrigin;
      moved = Math.max(moved, Math.abs(delta));
      viewport.scrollLeft = scrollOrigin - delta;
    });
    const endDrag = (event) => {
      if (!dragging) return;
      dragging = false;
      delete viewport.dataset.grabbing;
      if (viewport.hasPointerCapture?.(event.pointerId)) viewport.releasePointerCapture(event.pointerId);
      /* A drag is not a click. Without this a dragged rail would open the lightbox on release. */
      if (moved > 6) {
        const swallow = (click) => {
          click.preventDefault();
          click.stopPropagation();
        };
        viewport.addEventListener('click', swallow, { capture: true, once: true });
      }
      window.requestAnimationFrame(sync);
    };
    viewport.addEventListener('pointerup', endDrag);
    viewport.addEventListener('pointercancel', endDrag);

    viewport.addEventListener('keydown', (event) => {
      if (event.key === 'ArrowLeft') {
        event.preventDefault();
        step(-1);
      } else if (event.key === 'ArrowRight') {
        event.preventDefault();
        step(1);
      } else if (event.key === 'Home') {
        event.preventDefault();
        step(-total);
      } else if (event.key === 'End') {
        event.preventDefault();
        step(total);
      }
    });

    window.requestAnimationFrame(sync);
  }

  function renderMaterials(item, inspector, channels) {
    const panel = panels.get('materials');
    panel.replaceChildren();
    const fragment = model.items.find((candidate) => candidate.identity?.publicRef === model.selectedRef)?.evidenceFragment;
    const objects = mediaObjectsFrom(inspector, fragment);
    model.mediaObjects = objects;
    model.mediaIndex = 0;

    const local = section('LOCAL MEDIA', `PROJECTED / ${String(objects.length).padStart(2, '0')} OBJECTS`);
    if (objects.length === 0) {
      const empty = node('div', 'ev-inline-state');
      empty.dataset.tone = 'warning';
      empty.append(
        node('strong', null, '当前没有本地媒体对象'),
        node('p', null, '这只说明本机还没有留存这个作品的媒体副本；这不表示作品没有媒体。'),
        tech('NO LOCAL MEDIA'),
      );
      const action = node('div', 'ev-channel-actions');
      const request = node('button', 'ev-button ev-button--secondary', '补采媒体');
      request.type = 'button';
      request.disabled = true;
      request.title = '补采需要采集授权，本页只读，不触发平台访问。';
      action.append(request, tech('ACQUISITION NOT AUTHORIZED HERE'));
      empty.append(action);
      local.append(empty);
    } else {
      local.append(renderMediaRail(objects));
    }
    panel.append(local);

    const ledger = section('材料处理台账', 'PROCESSING LEDGER');
    const receipt = channels.media?.receipt || inspector.mediaSlotsReceipt;
    ledger.append(factGrid([
      ['槽位总数', receipt?.total ?? '当前未知', 'TOTAL SLOTS'],
      ['本次返回', receipt?.returned ?? '当前未知', 'RETURNED'],
      ['是否截断', receipt?.truncated === true ? '是，可继续读取' : receipt?.truncated === false ? '否' : '当前未知', 'TRUNCATED'],
    ]));
    objects.forEach((object) => {
      const entry = node('article', 'ev-derivative');
      const head = node('div', 'ev-derivative-head');
      head.append(
        node('strong', null, `${String(object.index + 1).padStart(2, '0')} · ${purposeLabel(object.purpose)}`),
        tech(object.slotKey || 'SLOT UNKNOWN'),
      );
      entry.append(head, factGrid([
        ['本地对象', object.objectRef || '当前未表达', 'MATERIALIZATION REF'],
        ['字节状态', stateMeta(object.bytesState)[0], object.bytesState],
        ['文字处理', object.derivedState ? stateMeta(object.derivedState)[0] : '未处理', object.derivedState || 'NOT PROCESSED'],
        ['采集序号', object.ordinal, object.slot.displayOrderState === 'KNOWN' ? 'DISPLAY ORDER KNOWN' : 'DISPLAY ORDER UNVERIFIED'],
        ['副本状态', object.slot.replicaState || '当前未知', object.slot.replicaState || 'UNKNOWN'],
      ]));
      ledger.append(entry);
    });
    panel.append(ledger);
  }

  /* -------------------------------------------------------------- lightbox */

  function openLightbox(index, opener) {
    if (!model.mediaObjects.length) return;
    model.lightboxIndex = Math.min(model.mediaObjects.length - 1, Math.max(0, index));
    model.lightboxOpener = opener || null;
    refs.lightbox.hidden = false;
    document.body.style.overflow = 'hidden';
    paintLightbox();
    refs.lightboxClose.focus();
  }

  function paintLightbox() {
    const object = model.mediaObjects[model.lightboxIndex];
    if (!object) return;
    refs.lightboxTitle.textContent = `${purposeLabel(object.purpose)} · 第 ${String(object.index + 1).padStart(2, '0')} / ${String(model.mediaObjects.length).padStart(2, '0')} 个`;
    refs.lightboxRef.textContent = object.objectRef
      ? `MED-${String(object.objectRef).slice(0, 8).toUpperCase()} · 采集序号 ${object.ordinal} · 平台排列顺序未经验证`
      : `本地对象 ID 未表达 · 采集序号 ${object.ordinal}`;
    if (object.assetUrl) {
      refs.lightboxImage.hidden = false;
      refs.lightboxImage.src = object.assetUrl;
      refs.lightboxImage.alt = `${purposeLabel(object.purpose)} 原尺寸本地副本`;
    } else {
      refs.lightboxImage.hidden = true;
      refs.lightboxImage.removeAttribute('src');
    }
    refs.lightboxCaption.textContent = object.assetUrl
      ? `字节 ${stateMeta(object.bytesState)[0]} · 文字 ${object.derivedState ? stateMeta(object.derivedState)[0] : '未处理'}`
      : `这个对象没有可内联的本地副本：${stateMeta(object.bytesState)[0]}`;
    refs.lightboxPrev.disabled = model.lightboxIndex <= 0;
    refs.lightboxNext.disabled = model.lightboxIndex >= model.mediaObjects.length - 1;
  }

  function stepLightbox(delta) {
    const next = model.lightboxIndex + delta;
    if (next < 0 || next >= model.mediaObjects.length) return;
    model.lightboxIndex = next;
    paintLightbox();
  }

  function closeLightbox() {
    refs.lightbox.hidden = true;
    document.body.style.overflow = '';
    refs.lightboxImage.removeAttribute('src');
    /* Focus goes back to the card the reader opened, not to the top of the panel. */
    if (model.lightboxOpener && document.contains(model.lightboxOpener)) {
      model.lightboxOpener.focus({ preventScroll: true });
    }
    model.lightboxOpener = null;
  }

  /* ----------------------------------------------------------------- trace */

  function renderTrace(inspector, provenanceChannel) {
    const panel = panels.get('trace');
    panel.replaceChildren();
    const provenance = inspector.provenance && typeof inspector.provenance === 'object' ? inspector.provenance : null;
    const receipt = provenanceChannel.receipt || provenance?.receipt;
    const channel = section('来源轨迹通道', 'BOUNDED CHANNEL');
    channel.append(factGrid([
      ['总数', receipt?.total ?? '当前未知', 'TOTAL'],
      ['本次返回', receipt?.returned ?? '当前未知', 'RETURNED'],
      ['是否截断', receipt?.truncated === true ? '是，可继续读取' : receipt?.truncated === false ? '否' : '当前未知', 'TRUNCATED'],
    ]));
    panel.append(channel);

    const refsSection = section('来源与执行血缘', 'PROVENANCE');
    if (!provenance) {
      refsSection.append(sourceIncompleteBlock('当前详情没有返回来源血缘。'));
    } else {
      [
        ['Package', provenance.packageRefs],
        ['Task', provenance.taskRefs],
        ['Attempt', provenance.attemptRefs],
        ['Receipt', provenance.receiptRefs],
        ['Target', provenance.targetRefs],
        ['Work Order', provenance.workOrderRefs],
        ['Producer', provenance.producers],
      ].forEach(([label, values]) => {
        const block = node('div', 'ev-provenance-group');
        block.append(node('strong', null, label));
        if (Array.isArray(values) && values.length) {
          values.forEach((value) => block.append(node('code', null, String(value))));
        } else {
          block.append(tech('SOURCE INCOMPLETE'));
        }
        refsSection.append(block);
      });
      const station = node('div', 'ev-inline-state');
      station.append(node('strong', null, '工位与账号镜头'), node('p', null, '当前详情只返回资格状态，不展示平台账号身份。'), tech(provenance.stationAccountLens?.state || 'UNKNOWN'));
      refsSection.append(station);
    }
    panel.append(refsSection);

    const limits = section('限制与访问边界', 'LIMITATIONS');
    const values = Array.isArray(inspector.limitations) ? inspector.limitations : [];
    if (!values.length) {
      limits.append(sourceIncompleteBlock('当前详情没有返回限制列表；页面不据此推断没有限制。'));
    } else {
      const list = node('ul', 'ev-limit-list');
      values.forEach((value) => {
        const entry = node('li');
        addTextWithTech(entry, '当前限制', value);
        list.append(entry);
      });
      limits.append(list);
    }
    panel.append(limits);
  }

  /* ------------------------------------------------------------------ wiring */

  function activateTab(tab, focus = false) {
    tabs.forEach((candidate) => {
      const selected = candidate === tab;
      candidate.setAttribute('aria-selected', String(selected));
      candidate.tabIndex = selected ? 0 : -1;
      panels.get(candidate.dataset.evTab)?.toggleAttribute('hidden', !selected);
    });
    model.activeTab = tab.dataset.evTab;
    if (focus) tab.focus();
    syncUrl();
  }

  function restoreFromUrl() {
    const params = new URLSearchParams(window.location.search);
    refs.search.value = params.get('q') || '';
    const assign = (control, key) => {
      const value = params.get(key);
      if (value && [...control.options].some((option) => option.value === value)) control.value = value;
    };
    assign(refs.sort, 'sort');
    assign(refs.window, 'window');
    assign(refs.lane, 'lane');
    assign(refs.laneState, 'laneState');
    assign(refs.mediaKind, 'mediaKind');
    const view = params.get('view');
    if (view && viewFilters[view]) model.activeView = view;
    const layout = params.get('layout');
    if (['research', 'table', 'cover'].includes(layout)) model.activeLayout = layout;
    const tab = params.get('tab');
    if (tabs.some((candidate) => candidate.dataset.evTab === tab)) model.activeTab = tab;
    const panel = params.get('panel');
    if (panel === 'closed') model.inspectorClosed = true;
    else if (INSPECTOR_WIDTHS.includes(panel)) model.inspectorWidth = panel;
    model.selectedRef = params.get('work') || null;
    viewButtons.forEach((button) => button.setAttribute('aria-pressed', String(button.dataset.evView === model.activeView)));
    layoutButtons.forEach((button) => button.setAttribute('aria-pressed', String(button.dataset.evLayout === model.activeLayout)));
    if (activeFilterCount() > 0) {
      refs.filterPanel.hidden = false;
      refs.filterToggle.setAttribute('aria-expanded', 'true');
    }
    syncFilterCount();
  }

  refs.form.addEventListener('submit', (event) => {
    event.preventDefault();
    model.activeView = 'all';
    viewButtons.forEach((button) => button.setAttribute('aria-pressed', String(button.dataset.evView === 'all')));
    loadList();
  });
  refs.filterToggle.addEventListener('click', () => {
    const open = refs.filterPanel.hidden;
    refs.filterPanel.hidden = !open;
    refs.filterToggle.setAttribute('aria-expanded', String(open));
  });
  document.getElementById('ev-reset').addEventListener('click', () => {
    refs.window.value = 'latest_accepted_discovery';
    refs.lane.value = '';
    refs.laneState.value = '';
    refs.mediaKind.value = '';
    syncFilterCount();
    model.activeView = 'all';
    viewButtons.forEach((button) => button.setAttribute('aria-pressed', String(button.dataset.evView === 'all')));
    loadList();
  });
  [refs.window, refs.lane, refs.laneState, refs.mediaKind].forEach((control) => control.addEventListener('change', () => {
    syncFilterCount();
    loadList();
  }));
  refs.sort.addEventListener('change', () => loadList({ keepSelection: true }));
  refs.nextList.addEventListener('click', () => loadList({ append: true }));
  viewButtons.forEach((button) => button.addEventListener('click', () => {
    model.activeView = button.dataset.evView;
    viewButtons.forEach((candidate) => candidate.setAttribute('aria-pressed', String(candidate === button)));
    loadList();
  }));
  layoutButtons.forEach((button) => button.addEventListener('click', () => {
    model.activeLayout = button.dataset.evLayout;
    layoutButtons.forEach((candidate) => candidate.setAttribute('aria-pressed', String(candidate === button)));
    renderRows(false);
    syncUrl();
  }));
  widthButtons.forEach((button) => button.addEventListener('click', () => setInspectorWidth(button.dataset.evWidth)));
  refs.closeInspector.addEventListener('click', closeInspector);
  refs.reopenInspector.addEventListener('click', () => {
    openInspector();
    refs.inspector.focus?.();
  });
  tabs.forEach((tab) => {
    tab.addEventListener('click', () => activateTab(tab));
    tab.addEventListener('keydown', (event) => {
      if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
      event.preventDefault();
      const index = tabs.indexOf(tab);
      let next = index;
      if (event.key === 'ArrowLeft') next = (index - 1 + tabs.length) % tabs.length;
      if (event.key === 'ArrowRight') next = (index + 1) % tabs.length;
      if (event.key === 'Home') next = 0;
      if (event.key === 'End') next = tabs.length - 1;
      activateTab(tabs[next], true);
    });
  });
  refs.back.addEventListener('click', () => {
    model.drawerOpen = false;
    applyInspectorState();
    const row = refs.list.querySelector(`[data-public-ref="${CSS.escape(model.selectedRef || '')}"]`);
    if (!row) return;
    row.scrollIntoView({ behavior: reducedMotion() ? 'auto' : 'smooth', block: 'center' });
    window.requestAnimationFrame(() => row.focus({ preventScroll: true }));
  });

  refs.lightboxPrev.addEventListener('click', () => stepLightbox(-1));
  refs.lightboxNext.addEventListener('click', () => stepLightbox(1));
  refs.lightboxClose.addEventListener('click', closeLightbox);
  refs.lightbox.addEventListener('click', (event) => {
    if (event.target === refs.lightbox) closeLightbox();
  });

  /* One key handler owns Escape so the layers unwind in the order the reader sees them: the
   * lightbox first, then the mobile drawer. */
  document.addEventListener('keydown', (event) => {
    if (!refs.lightbox.hidden) {
      if (event.key === 'Escape') {
        event.preventDefault();
        closeLightbox();
      } else if (event.key === 'ArrowLeft') {
        event.preventDefault();
        stepLightbox(-1);
      } else if (event.key === 'ArrowRight') {
        event.preventDefault();
        stepLightbox(1);
      } else if (event.key === 'Tab') {
        /* The dialog keeps focus: only its own controls are reachable while it is open. */
        const focusable = [refs.lightboxClose, refs.lightboxPrev, refs.lightboxNext].filter((control) => !control.disabled);
        if (!focusable.length) return;
        const index = focusable.indexOf(document.activeElement);
        event.preventDefault();
        const offset = event.shiftKey ? -1 : 1;
        focusable[(index + offset + focusable.length) % focusable.length].focus();
      }
      return;
    }
    if (event.key === 'Escape' && model.drawerOpen && isDrawerLayout()) {
      event.preventDefault();
      closeInspector();
    }
  });

  window.addEventListener('popstate', () => {
    restoreFromUrl();
    applyInspectorState();
    loadList({ keepSelection: true });
  });
  window.matchMedia(DRAWER_QUERY).addEventListener('change', applyInspectorState);

  restoreInspectorWidth();
  restoreFromUrl();
  applyInspectorState();
  clearInspector();
  activateTab(tabs.find((tab) => tab.dataset.evTab === model.activeTab) || tabs[0]);
  loadList({ keepSelection: true });
})();
