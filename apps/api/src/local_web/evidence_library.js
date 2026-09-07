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
    LEASED: ['租约已签发', 'info'],
    ACTIVE: ['租约有效', 'info'],
    MERGED: ['已合并到在途工作', 'info'],
    RELEASED: ['租约已释放', 'warning'],
    CLAIMED: ['工位已认领', 'info'],
    RUNNING: ['执行中', 'info'],
    ACCEPTED: ['已接纳回执', 'success'],
    EXPIRED: ['租约已过期', 'warning'],
    EXPIRED_WITHOUT_RECEIPT: ['租约结束，未见回执', 'warning'],
    COMPLETED_WITHOUT_RECEIPT: ['任务结束，未见回执', 'warning'],
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

  const SORT_OPTIONS = [
    { value: 'latest_discovery', label: '最近观察' },
    { value: 'relevance', label: '相关度' },
  ];
  const INSPECTOR_WIDTHS = ['normal', 'wide', 'focus'];
  const WIDTH_STORAGE_KEY = 'linggan.evidence.inspectorWidth';
  const DRAWER_QUERY = '(max-width: 1180px)';

  const refs = {
    form: document.getElementById('ev-query-form'),
    search: document.getElementById('ev-search'),
    sortToggle: document.getElementById('ev-sort-toggle'),
    sortValue: document.getElementById('ev-sort-value'),
    sortList: document.getElementById('ev-sort-list'),
    filterToggle: document.getElementById('ev-filter-toggle'),
    filterPanel: document.getElementById('ev-filter-panel'),
    filterCount: document.getElementById('ev-filter-count'),
    receipt: document.getElementById('ev-read-receipt'),
    bench: document.getElementById('ev-bench'),
    list: document.getElementById('ev-work-list'),
    feedback: document.getElementById('ev-feedback'),
    resultsLegend: document.getElementById('ev-results-legend'),
    railCount: document.getElementById('ev-rail-count'),
    nextList: document.getElementById('ev-next-list'),
    inspector: document.getElementById('ev-inspector'),
    inspectorTitle: document.getElementById('ev-inspector-title'),
    inspectorRef: document.getElementById('ev-inspector-ref'),
    headMetrics: document.getElementById('ev-head-metrics'),
    inspectorFeedback: document.getElementById('ev-inspector-feedback'),
    openSource: document.getElementById('ev-open-source'),
    requestMedia: document.getElementById('ev-request-media'),
    closeInspector: document.getElementById('ev-close-inspector'),
    reopenInspector: document.getElementById('ev-reopen-inspector'),
    back: document.getElementById('ev-back-to-list'),
    tableHead: document.getElementById('ev-table-head'),
    lightbox: document.getElementById('ev-lightbox'),
    lightboxFrame: document.getElementById('ev-lightbox-frame'),
    lightboxImage: document.getElementById('ev-lightbox-image'),
    zoomIn: document.getElementById('ev-zoom-in'),
    zoomOut: document.getElementById('ev-zoom-out'),
    zoomReset: document.getElementById('ev-zoom-reset'),
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
  const filterSelects = [...document.querySelectorAll('[data-ev-select]')].map(createSelect);
  const selectByName = new Map(filterSelects.map((control) => [control.name, control]));
  const filterNames = ['window', 'lane', 'laneState', 'mediaKind'];

  const model = {
    items: [],
    cursor: null,
    selectedRef: null,
    activeView: 'all',
    activeLayout: 'research',
    activeTab: 'overview',
    activeSort: 'latest_discovery',
    inspectorWidth: 'normal',
    inspectorClosed: false,
    drawerOpen: false,
    listController: null,
    detailController: null,
    detailItem: null,
    detailChannels: null,
    detailUrl: null,
    listItem: null,
    /* The current work's media objects, in the order the rail shows them. The lightbox reads
     * this rather than the DOM so opening it always lands on the object that was clicked. */
    mediaObjects: [],
    mediaIndex: 0,
    lightboxIndex: 0,
    lightboxOpener: null,
    zoom: 1,
    panX: 0,
    panY: 0,
  };
  const observation = window.LingganEvidenceObservation;
  const reobservation = observation.createController({
    apiRoot: API_ROOT,
    sameOriginPath,
    readJson,
    onChange: refreshReobservationSection,
    onTerminal: refreshSelectedDetailAfterReobservation,
  });
  const commentResearch = observation.createCommentResearchController({
    readJson,
    onChange: refreshCommentResearch,
  });

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

  /* Rows show minute precision; the exact instant stays on the element's title and in the
   * Inspector's fact grid. A scanning row does not need seconds, a `+00` offset or a `Z`, and
   * an ISO string long enough to wrap costs more than the precision it buys. */
  function compactMoment(value) {
    if (typeof value !== 'string' || !value) return null;
    const match = value.match(/^(\d{4})-(\d{2})-(\d{2})[T ](\d{2}):(\d{2})/);
    if (!match) return value;
    const [, year, month, day, hour, minute] = match;
    return `${year}-${month}-${day} ${hour}:${minute}`;
  }

  function compactDay(value) {
    if (typeof value !== 'string' || !value) return null;
    const match = value.match(/^(\d{4}-\d{2}-\d{2})/);
    return match ? match[1] : value;
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
    if (model.activeSort !== 'latest_discovery') params.set('sort', model.activeSort);
    const windowValue = selectByName.get('window').value;
    if (windowValue !== 'latest_accepted_discovery') params.set('window', windowValue);
    ['lane', 'laneState', 'mediaKind'].forEach((name) => {
      const value = selectByName.get(name).value;
      if (value) params.set(name, value);
    });
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
   * layout, which work is open, which Inspector tab it is on and how wide the panel is.
   *
   * Only the moves a person would call "where I was" push a history entry — running a query,
   * switching quick view or layout, opening a different work. Panel width, the open/closed
   * state and the Inspector tab replace instead: they are preferences about how the current
   * screen is displayed, and pushing them would bury the actual navigation under a pile of
   * back-steps that each undo a 30px width change. */
  function syncUrl(mode = 'replace') {
    const params = explicitParams();
    // 当前观察领域由地址承载，且不归这个页面管——它是整个语料模块的观察对象，切到哪个
    // 领域，语料下每个子页都跟着走。这里若不原样带回，每一次列表刷新都会把它抹掉，
    // 领域就变成了「选一次、下一次交互即失效」。
    const domain = new URLSearchParams(window.location.search).get('domain');
    if (domain) params.set('domain', domain);
    if (model.activeView !== 'all') params.set('view', model.activeView);
    if (model.activeLayout !== 'research') params.set('layout', model.activeLayout);
    if (model.selectedRef) params.set('work', model.selectedRef);
    if (model.activeTab !== 'overview') params.set('tab', model.activeTab);
    if (model.inspectorClosed) params.set('panel', 'closed');
    else if (model.inspectorWidth !== 'normal') params.set('panel', model.inspectorWidth);
    const query = params.toString();
    const address = query ? `/corpus/evidence?${query}` : '/corpus/evidence';
    if (mode === 'push' && address !== `${window.location.pathname}${window.location.search}`) {
      history.pushState(null, '', address);
    } else {
      history.replaceState(null, '', address);
    }
  }

  /* Both decks' popovers behave the same way: one open at a time, closed by clicking outside,
   * by Escape, and by choosing something. They are anchored to their button rather than
   * expanding the deck, so opening one never reflows the results underneath. */
  function closePopover(toggle, popover) {
    popover.hidden = true;
    toggle.setAttribute('aria-expanded', 'false');
  }

  /* Opening one popover closes the others — except the ones it lives inside. The four filter
   * selects sit within the filter panel, so a blanket close would dismiss the panel the moment
   * a reader opened a select in it. */
  function openPopover(toggle, popover) {
    closeAllPopovers(popover);
    popover.hidden = false;
    toggle.setAttribute('aria-expanded', 'true');
  }

  function allPopovers() {
    return [
      [refs.filterToggle, refs.filterPanel],
      [refs.sortToggle, refs.sortList],
      ...filterSelects.map((control) => [control.toggle, control.list]),
    ];
  }

  function closeAllPopovers(keepAncestorsOf = null) {
    allPopovers().forEach(([toggle, popover]) => {
      if (keepAncestorsOf && popover.contains(keepAncestorsOf)) return;
      closePopover(toggle, popover);
    });
  }

  function togglePopover(toggle, popover) {
    if (popover.hidden) openPopover(toggle, popover);
    else closePopover(toggle, popover);
  }

  /* A drawn select. The native control cannot be styled past its box — its popup is drawn by
   * the operating system with its own radius, blue highlight and shadow — so a page with a
   * hard-edged language ends up with four system-rendered menus inside it. These carry the
   * same value semantics (a name, a current value, change events) with the page's own surface. */
  function createSelect(root) {
    const toggle = root.querySelector('.ev-select-toggle');
    const list = root.querySelector('.ev-select-list');
    const value = root.querySelector('.ev-select-value');
    const options = [...list.querySelectorAll('[data-ev-option]')];
    /* The first option is the default, not merely the initial value: a filter counts as active
     * only when it differs from it. Treating any truthy value as active marked 读取窗口 as
     * filtered permanently, because its default carries a value rather than an empty string. */
    const fallback = options[0]?.dataset.evOption ?? '';
    const control = {
      name: root.dataset.evName,
      root,
      toggle,
      list,
      fallback,
      value: fallback,
      onChange: null,
    };
    function paint() {
      const current = options.find((option) => option.dataset.evOption === control.value) || options[0];
      value.textContent = current ? current.textContent : '';
      options.forEach((option) => {
        option.setAttribute('aria-selected', String(option === current));
      });
      root.dataset.active = String(control.value !== control.fallback);
    }
    control.set = (next, notify = false) => {
      const exists = options.some((option) => option.dataset.evOption === next);
      control.value = exists ? next : fallback;
      paint();
      if (notify) control.onChange?.();
    };
    toggle.addEventListener('click', (event) => {
      event.stopPropagation();
      togglePopover(toggle, list);
    });
    options.forEach((option) => {
      option.addEventListener('click', () => {
        closePopover(toggle, list);
        toggle.focus();
        control.set(option.dataset.evOption, true);
      });
    });
    list.addEventListener('keydown', (event) => {
      if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) return;
      event.preventDefault();
      const index = options.indexOf(document.activeElement);
      let next = 0;
      if (event.key === 'ArrowDown') next = (index + 1) % options.length;
      if (event.key === 'ArrowUp') next = (index - 1 + options.length) % options.length;
      if (event.key === 'End') next = options.length - 1;
      options[next].focus();
    });
    paint();
    return control;
  }

  function setSort(value) {
    const option = SORT_OPTIONS.find((candidate) => candidate.value === value) || SORT_OPTIONS[0];
    model.activeSort = option.value;
    refs.sortValue.textContent = option.label;
    [...refs.sortList.querySelectorAll('[data-ev-sort]')].forEach((button) => {
      button.setAttribute('aria-selected', String(button.dataset.evSort === option.value));
    });
  }

  function activeFilterCount() {
    return filterNames.filter((name) => {
      const control = selectByName.get(name);
      return control.value !== control.fallback;
    }).length;
  }

  function syncFilterCount() {
    const count = activeFilterCount();
    refs.filterCount.textContent = String(count);
    refs.filterToggle.dataset.active = String(count > 0);
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
    const rail = node('div', 'ev-material-rail');
    summary.segments.forEach((segment) => {
      const cell = node('i', 'ev-mat-seg');
      cell.dataset.fill = segment.fill;
      cell.title = segment.applicable
        ? `${segment.label}：${stateMeta(segment.state)[0]}`
        : `${segment.label}：本作品不适用，不计入分母`;
      rail.append(cell);
    });
    return { rail, summary };
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
    refs.railCount.textContent = String(works);
    refs.railCount.hidden = works === 0;
    /* The other three figures the readout used to carry stay reachable as the entry's tooltip
     * rather than as a header block: they describe this read, and this read is the list. */
    refs.railCount.title = `本次读取 ${works} 个作品 · 已留存评论 ${commentsRetained === null ? '未知' : compactCount(commentsRetained)} · 本地材料 ${localObjects} · 部分或缺口 ${gapCount}`;
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

  /* The author's avatar is a media object like any other: it is shown only from a controlled
   * local handle, and when there is no verified copy the slot states why rather than falling
   * back to a platform URL. Ported from main's XHS-MEDIA-AUTHOR-EVIDENCE-001. */
  function authorAvatar(media, alt = '作者头像') {
    const avatar = media?.avatar && typeof media.avatar === 'object' ? media.avatar : {};
    const asset = sameOriginPath(avatar.localAssetUrl, ['/api/local/media/']);
    const wrapper = node('span', 'ev-author-avatar');
    if (asset && avatar.blob?.deliveryState === 'INLINE_SAFE') {
      const image = node('img');
      image.src = asset;
      image.alt = alt;
      image.loading = 'lazy';
      wrapper.append(image);
      wrapper.dataset.state = 'acquired';
    } else {
      wrapper.append(node('span', null, '作者'));
      wrapper.dataset.state = String(avatar.state || 'NOT_OBSERVED').toLowerCase();
      wrapper.setAttribute('aria-label', `作者头像${stateMeta(avatar.state || 'NOT_OBSERVED')[0]}`);
    }
    return wrapper;
  }

  /* DEC-作者合并 (2026-09-04): a creator target is watched through its own profile URL, so the
   * works collected there are that creator's — target and author are one field, everywhere. The
   * platform author name from the work detail still wins when present, and a reported MISMATCH
   * still blocks the fallback rather than asserting an authorship the platform denies. A keyword
   * target is a search term, not a person, and never fills the author. */
  function effectiveAuthor(item) {
    if (item.display?.creatorState === 'KNOWN' && item.display?.creatorDisplayName) {
      return item.display.creatorDisplayName;
    }
    const context = item.collectionContext || {};
    const matched = context.authorIdentityMatchState !== 'MISMATCH';
    if (matched && context.targetKind === 'creator' && context.targetDisplayState === 'KNOWN' && context.targetDisplayName) {
      return context.targetDisplayName;
    }
    return '当前未知';
  }

  function authorFact(item) {
    const name = effectiveAuthor(item);
    const fact = node('div', 'ev-author-fact');
    fact.append(authorAvatar(item.media, `${name}的头像`), node('strong', null, name));
    return fact;
  }

  const targetKindLabels = { creator: '博主监控', keyword: '关键词监控' };

  /* The row shows one author; the Inspector keeps the collection lineage so "how did this work
   * get here" stays answerable without putting a second identity back on the list. */
  function monitoringOriginCopy(item) {
    const context = item.collectionContext || {};
    if (context.targetDisplayState !== 'KNOWN' || !context.targetDisplayName) return '当前未知';
    const kind = targetKindLabels[context.targetKind];
    return kind ? `${context.targetDisplayName}（${kind}）` : context.targetDisplayName;
  }

  /* The one serif quote, used by both the row and the Inspector so a search hit is highlighted
   * in the same place in both. The excerpt is sliced with `Array.from` because the offsets the
   * read model returns are character offsets, and a CJK codepoint is more than one JavaScript
   * string index. Row and panel differ only in CSS: the row clamps to one line, the panel wraps. */
  function evidenceBlock(fragment) {
    const block = node('div', 'ev-evidence');
    if (!fragment || typeof fragment.text !== 'string' || !fragment.text) {
      block.classList.add('ev-evidence--absent');
      block.append(
        node('span', 'ev-evidence-source', '无原声'),
        node('span', null, '尚未取得可引用的原文、评论或图片文字'),
      );
      return block;
    }
    block.append(node('span', 'ev-evidence-source', fragmentSourceLabels[fragment.sourceKind] || '来源未表达'));
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

  const ENGAGEMENT_METRICS = [
    ['like', '点赞'], ['comment', '评论'], ['collect', '收藏'], ['share', '分享'],
  ];

  function engagementBlock(item, { numbersOnly = false } = {}) {
    const engagement = item.display?.engagement || {};
    const metrics = [
      ['like', '点赞', engagement.likeCount, engagement.likeCountState],
      ['comment', '评论', engagement.commentCount, engagement.commentCountState],
      ['collect', '收藏', engagement.collectCount, engagement.collectCountState],
      ['share', '分享', engagement.shareCount, engagement.shareCountState],
    ];
    const wrapper = node('div', 'ev-engagement');
    metrics.forEach(([kind, label, value, state]) => {
      const cell = node('span');
      const reading = state === 'KNOWN' && Number.isFinite(Number(value))
        ? Number(value).toLocaleString('zh-CN')
        : '未知';
      // In the table the icon is in the column header, so a row carries the number alone. The
      // accessible name has to carry both: an aria-label on this span replaces its text in the
      // row's computed name, and the header icons cannot fill the gap because ev-table-head is
      // aria-hidden.
      if (numbersOnly) cell.setAttribute('aria-label', `${label} ${reading}`);
      else cell.append(metricIcon(kind, label));
      cell.append(node('b', null, reading));
      wrapper.append(cell);
    });
    return wrapper;
  }

  function publishedCopy(item, compact = false, withFieldName = true) {
    if (item.display?.publishedAtState === 'KNOWN') {
      const raw = item.display?.publishedAt;
      if (!raw) return '发布时间已知';
      return compact ? compactDay(raw) : raw;
    }
    if (item.display?.publishedAtState === 'SOURCE_TEXT_ONLY') {
      return `来源时间：${item.display?.publishedAtSourceText || '已观察'}`;
    }
    return withFieldName ? '发布时间当前未知' : '当前未知';
  }

  function rowFor(item) {
    const publicRef = item.identity?.publicRef;
    const row = node('article', 'ev-work-row');
    row.setAttribute('role', 'option');
    row.tabIndex = -1;
    row.dataset.publicRef = publicRef || '';
    row.dataset.platform = String(item.identity?.platform || 'unknown').toLowerCase();
    row.setAttribute('aria-selected', String(publicRef === model.selectedRef));

    const published = publishedCopy(item, true);

    const identity = node('div', 'ev-identity');
    const eyebrow = node('div', 'ev-eyebrow');
    eyebrow.append(node('span', null, item.identity?.platform?.toUpperCase() || '平台未知'));
    const title = node('h2', null, knownText(item.display?.title, item.display?.titleState, '标题当前未知'));
    const meta = node('div', 'ev-meta');
    meta.append(authorFact(item), node('span', 'ev-time-line', published));
    identity.append(eyebrow, title, meta, evidenceBlock(item.evidenceFragment), engagementBlock(item));

    const material = materialBlock(item);
    const side = node('div', 'ev-side');
    side.append(material.rail);
    const detailState = laneSummary(item, 'detail')?.state || 'UNKNOWN';
    side.append(stateLine(detailState));
    const observedAt = item.summary?.lastObservedAt;
    const observed = node('span', 'ev-observed', `最近观察 ${compactMoment(observedAt) || '未知'}`);
    if (observedAt) observed.title = `最近观察 ${observedAt}`;
    side.append(observed);

    if (model.activeLayout === 'table') {
      row.append(
        tableCell(title.textContent, ''),
        (() => {
          const cell = node('div', 'ev-table-cell');
          cell.append(authorFact(item));
          return cell;
        })(),
        (() => {
          const cell = node('div', 'ev-table-material');
          cell.append(material.rail, stateLine(detailState));
          return cell;
        })(),
        engagementBlock(item, { numbersOnly: true }),
        tableCell(publishedCopy(item, true, false), ''),
        tableCell(compactMoment(item.summary?.lastObservedAt) || '未知', ''),
      );
    } else {
      row.append(previewBlock(item), identity, side);
    }
    row.addEventListener('click', () => selectItem(item, 'user'));
    row.addEventListener('keydown', (event) => onRowKeydown(event, item));
    return row;
  }

  /* LIDS icon system: 24 grid, 1.5 stroke, currentColor, sized 16/20/24. Character glyphs are
   * forbidden -- they render differently per platform, cannot control stroke weight and will not
   * align to the text beside them. Each icon keeps an accessible name: the number next to it says
   * nothing on its own. */
  const METRIC_ICON_PATHS = {
    like: 'M12 20.5C12 20.5 3.5 15.4 3.5 9.9A4.9 4.9 0 0 1 12 6.6a4.9 4.9 0 0 1 8.5 3.3c0 5.5-8.5 10.6-8.5 10.6z',
    comment: 'M4 3.5h16a1.5 1.5 0 0 1 1.5 1.5v9a1.5 1.5 0 0 1-1.5 1.5H9.2L4 20.2V3.5z',
    collect: 'M6.5 2.5h11a1 1 0 0 1 1 1v18l-6.5-4.8-6.5 4.8v-18a1 1 0 0 1 1-1z',
    share: 'M21.6 2.6 2.9 9.9a.6.6 0 0 0 .05 1.13l4.6 1.5 1.6 4.9a.6.6 0 0 0 1.06.18l2.2-2.9 4.4 3.2a.6.6 0 0 0 .94-.33l4-14.2a.6.6 0 0 0-.15-.78z',
  };

  function metricIcon(kind, label) {
    const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    svg.setAttribute('viewBox', '0 0 24 24');
    svg.setAttribute('width', '16');
    svg.setAttribute('height', '16');
    svg.setAttribute('fill', 'currentColor');
    svg.setAttribute('role', 'img');
    svg.setAttribute('aria-label', label);
    const path = document.createElementNS('http://www.w3.org/2000/svg', 'path');
    path.setAttribute('d', METRIC_ICON_PATHS[kind]);
    svg.append(path);
    return svg;
  }

  function tableCell(primary, secondary) {
    const cell = node('div', 'ev-table-cell');
    cell.append(node('b', null, primary));
    if (secondary) cell.append(node('span', null, secondary));
    return cell;
  }

  function renderRows(appended) {
    if (refs.headMetrics && !refs.headMetrics.childElementCount) {
      ENGAGEMENT_METRICS.forEach(([kind, label]) => refs.headMetrics.append(metricIcon(kind, label)));
    }
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

  /* The receipt bar only appears when the read did something the counts above cannot show. A
   * successful ordinary read is already fully described by the readout and the rows, so
   * restating "读取成功，当前显示 14 个作品集合" a third time is noise, not honesty. The read
   * timestamp moves onto the results header as a title, where it stays checkable without
   * occupying a line of its own. */
  function queryReceipt(payload, appended) {
    const notes = [];
    if (payload.scanLimited) {
      notes.push(['扫描预算已触发，当前结果不是全部匹配；可继续读取']);
    }
    if (payload.truncated && !payload.cursor) {
      notes.push(['本次读取已截断，但来源没有给出继续读取的游标']);
    }
    refs.receipt.replaceChildren();
    refs.receipt.hidden = notes.length === 0;
    if (notes.length) {
      const main = node('span');
      notes.forEach(([copy, code], index) => {
        if (index) main.append(document.createTextNode(' · '));
        addTextWithTech(main, copy, code);
      });
      const meta = node('span', 'ev-receipt-meta');
      meta.append(tech(`扫描 ${payload.scannedCount ?? '未知'}`));
      refs.receipt.append(main, meta);
    }
    refs.resultsLegend.title = `读取时间 ${payload.asOf || '未知'} · 扫描 ${payload.scannedCount ?? '未知'} 条${appended ? ' · 已继续读取' : ''}`;
    refs.nextList.hidden = !payload.cursor;
    refs.nextList.disabled = !payload.cursor;
    refs.nextList.dataset.cursor = payload.cursor || '';
  }

  /* A `?work=` link addresses the stable Work directly. The first list page is only a browsing
   * window and must not be treated as the set of Works that exist: a lifecycle point can name an
   * older Work that is outside that page. This shell contains identity plus the same-origin
   * detail handle only; the detail response remains the sole source for Inspector facts. */
  function directWorkItem(publicRef) {
    if (typeof publicRef !== 'string'
      || !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(publicRef)) {
      return null;
    }
    return {
      identity: { publicRef },
      detailUrl: `${API_ROOT}/${encodeURIComponent(publicRef)}`,
      display: { title: null, titleState: 'UNKNOWN' },
    };
  }

  async function loadList({
    append = false,
    keepSelection = false,
    revealUrlSelection = false,
    history: historyMode = 'replace',
  } = {}) {
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
      setFeedback('loading', '正在读取本机材料投影', '只读取 Linggan 已接纳的作品级材料；不会触发平台搜索或采集。');
      refs.list.replaceChildren();
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
      syncUrl(append ? 'replace' : historyMode);
      const requestedRef = restoreRef || model.selectedRef;
      const requestedItem = requestedRef
        ? model.items.find((item) => item.identity?.publicRef === requestedRef)
        : null;
      const selectionSource = revealUrlSelection && requestedRef ? 'url' : 'auto';
      if (model.items.length === 0) {
        setFeedback('empty', '当前查询没有匹配的作品材料', '读取已经成功；这个结果只描述当前本地查询，不证明平台或现实中没有相关内容。');
        if (directWorkItem(requestedRef)) await selectItem(directWorkItem(requestedRef), selectionSource);
        else clearInspector();
      } else {
        clearFeedback();
        if (requestedItem) await selectItem(requestedItem, selectionSource);
        else if (directWorkItem(requestedRef)) await selectItem(directWorkItem(requestedRef), selectionSource);
        else await selectItem(model.items[0], 'auto');
      }
    } catch (error) {
      if (error.name === 'AbortError') return;
      model.items = [];
      renderRows(false);
      renderReadout();
      refs.nextList.hidden = true;
      setFeedback('error', '本机材料读取暂时不可用', '当前没有读取任何作品材料；页面不会回退到旧卡片、远程数据库或平台 CDN。', error.code || null);
      refs.receipt.replaceChildren();
      refs.receipt.hidden = false;
      addTextWithTech(refs.receipt, '当前未读取任何材料，不能据此判断库为空或来源不存在。', error.code || null);
    }
  }

  function onRowKeydown(event, item) {
    const rows = [...refs.list.querySelectorAll('[data-public-ref]')];
    const current = event.currentTarget;
    const index = rows.indexOf(current);
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      selectItem(item, 'user');
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
    if (nextItem) selectItem(nextItem, 'auto').then(() => next.focus());
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

  /* Does not write history itself. Opening the panel is always part of a larger action —
   * choosing a work, or reopening after a close — and that action decides the history mode. */
  function openInspector() {
    if (isDrawerLayout()) model.drawerOpen = true;
    model.inspectorClosed = false;
    applyInspectorState();
  }

  function storedInspectorWidth() {
    let stored = null;
    try {
      stored = window.localStorage.getItem(WIDTH_STORAGE_KEY);
    } catch (_) {
      /* A private window or blocked site data must not break the panel. */
      stored = null;
    }
    return INSPECTOR_WIDTHS.includes(stored) ? stored : 'normal';
  }

  function clearInspector() {
    model.detailController?.abort();
    reobservation.reset();
    commentResearch.reset();
    model.detailItem = null;
    model.detailChannels = null;
    model.detailUrl = null;
    model.listItem = null;
    model.mediaObjects = [];
    model.mediaIndex = 0;
    refs.inspectorRef.textContent = '';
    refs.inspectorTitle.textContent = '请选择一个作品材料集合';
    refs.openSource.disabled = true;
    refs.requestMedia.disabled = true;
    refs.inspectorFeedback.hidden = false;
    refs.inspectorFeedback.replaceChildren(node('strong', null, '尚未选择作品'));
    panels.forEach((panel) => panel.replaceChildren());
  }

  async function selectItem(item, selectionSource) {
    const publicRef = item?.identity?.publicRef;
    const detailUrl = sameOriginPath(item?.detailUrl, [`${API_ROOT}/`]);
    if (!publicRef || !detailUrl || detailUrl.endsWith('/comments')) {
      showInspectorSourceIncomplete(item, '列表没有提供可用的同源 detailUrl。');
      return;
    }
    if (model.selectedRef !== publicRef) {
      reobservation.reset();
      commentResearch.reset();
    }
    model.selectedRef = publicRef;
    model.detailUrl = detailUrl;
    [...refs.list.querySelectorAll('[data-public-ref]')].forEach((row) => {
      const selected = row.dataset.publicRef === publicRef;
      row.setAttribute('aria-selected', String(selected));
      row.tabIndex = selected ? 0 : -1;
    });
    /* A reader click and a successful `?work=` restoration both promise a visible Inspector.
     * Only the click is a new navigation step and pushes history; URL restoration and automatic
     * list selection replace the current entry. This keeps a direct mobile link visible without
     * manufacturing a second Back step or changing the addressed Work ref. */
    if (selectionSource === 'user' || selectionSource === 'url') openInspector();
    syncUrl(selectionSource === 'user' ? 'push' : 'replace');
    refs.inspectorFeedback.hidden = false;
    refs.inspectorFeedback.replaceChildren(node('strong', null, '正在读取当前作品详情'));
    refs.inspectorTitle.textContent = knownText(item.display?.title, item.display?.titleState, '标题当前未知');
    refs.inspectorRef.textContent = publicRef.slice(0, 8).toUpperCase();
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
      // error.code is a contract value and keeps its technical rendering; the invented English
      // fallback does not, so an unlabelled failure just says so in Chinese.
      refs.inspectorFeedback.replaceChildren(node('strong', null, '当前作品详情读取失败'));
      if (error.code) refs.inspectorFeedback.append(tech(error.code));
      panels.forEach((panel) => panel.replaceChildren(sourceIncompleteBlock('当前详情未读取，列表通道仍可查看，但不能据此补造 Inspector。')));
    }
  }

  function showInspectorSourceIncomplete(item, detail) {
    model.selectedRef = item?.identity?.publicRef || null;
    refs.inspectorRef.textContent = model.selectedRef ? model.selectedRef.slice(0, 8).toUpperCase() : '';
    refs.inspectorTitle.textContent = knownText(item?.display?.title, item?.display?.titleState, '标题当前未知');
    refs.inspectorFeedback.hidden = false;
    refs.inspectorFeedback.replaceChildren(node('strong', null, '来源信息不完整'), tech('SOURCE INCOMPLETE'));
    panels.forEach((panel) => panel.replaceChildren(sourceIncompleteBlock(detail)));
  }

  // PAT-003 places the boundary band in front of a region once. This used to render a warning
  // panel per empty block, so a single inspector carried the same sentence five or more times
  // and the warning colour stopped meaning anything. The specific sentence stays -- it says
  // something the band cannot -- but it no longer competes with real failures for attention.
  function sourceIncompleteBlock(detail) {
    const block = node('p', 'ev-source-note');
    block.append(node('span', null, detail), tech('SOURCE INCOMPLETE'));
    return block;
  }

  /* Collapsing is layout, not rewriting. A row that moves inside the disclosure keeps its exact
   * label, its exact copy and its exact state code; expanding restores the previous rendering
   * character for character. It exists because a grid where eight of eleven rows read UNKNOWN
   * buries the three rows that carry an answer. Below three unknowns there is nothing to gain,
   * so the grid renders flat. */
  const UNKNOWN_COLLAPSE_THRESHOLD = 3;

  function appendFactRows(list, rows) {
    rows.forEach(([label, value, code]) => {
      list.append(node('dt', null, label));
      const value_ = node('dd');
      addTextWithTech(value_, value, code);
      list.append(value_);
    });
  }

  function isUnknownRow([, value, code]) {
    return code === 'UNKNOWN' && typeof value === 'string' && value.includes('未知');
  }

  function factGrid(rows) {
    const unknown = rows.filter(isUnknownRow);
    if (unknown.length < UNKNOWN_COLLAPSE_THRESHOLD) {
      const list = node('dl', 'ev-facts');
      appendFactRows(list, rows);
      return list;
    }
    const fragment = document.createDocumentFragment();
    const known = rows.filter((row) => !isUnknownRow(row));
    if (known.length > 0) {
      const list = node('dl', 'ev-facts');
      appendFactRows(list, known);
      fragment.append(list);
    }
    const details = node('details', 'ev-facts-collapsed');
    details.append(node('summary', null, `尚未取得 ${unknown.length} 项`));
    const hidden = node('dl', 'ev-facts');
    appendFactRows(hidden, unknown);
    details.append(hidden);
    fragment.append(details);
    return fragment;
  }

  function section(title, code) {
    const wrapper = node('section', 'ev-inspect-section');
    const heading = node('h3');
    heading.append(node('span', null, title));
    if (code) heading.append(tech(code));
    wrapper.append(heading);
    return wrapper;
  }

  function stateTag(state) {
    return stateLine(state);
  }

  function observationUi() {
    return { node, tech, section, factGrid, stateTag, sourceIncompleteBlock, laneLabels, knownMetric };
  }

  function receiptBlock(title, receipt, channelUrl) {
    const wrapper = node('div', 'ev-channel-receipt');
    const heading = node('div', 'ev-channel-title');
    heading.append(node('strong', null, title));
    wrapper.append(heading);
    if (!receipt || typeof receipt !== 'object') {
      wrapper.append(sourceIncompleteBlock('当前详情没有提供该通道的读取回执。'));
      return wrapper;
    }
    wrapper.append(factGrid([
      ['总数', receipt.total ?? '当前未知', receipt.total === null || receipt.total === undefined ? 'UNKNOWN' : null],
      ['本次返回', receipt.returned ?? '当前未知', receipt.returned === null || receipt.returned === undefined ? 'UNKNOWN' : null],
      ['是否截断', receipt.truncated === true ? '是，可继续读取' : receipt.truncated === false ? '否' : '当前未知'],
      ['下一游标', receipt.nextCursor || '无'],
    ]));
    if (receipt.truncated && !channelUrl) {
      const note = node('div', 'ev-inline-state');
      note.dataset.tone = 'warning';
      note.append(node('strong', null, '继续读取入口尚未提供'), node('p', null, '回执表明通道已截断，但详情没有给出可调用的下一页 URL；页面不会猜测后端路由。'), tech('SOURCE INCOMPLETE'));
      wrapper.append(note);
    }
    return wrapper;
  }

  function discussionUi() {
    return {
      ...observationUi(),
      apiRoot: API_ROOT,
      sameOriginPath,
      stateMeta,
      laneSummary,
      receiptBlock,
      addTextWithTech,
    };
  }

  function refreshReobservationSection() {
    const panel = panels.get('overview');
    const previous = panel?.querySelector('[data-ev-reobservation]');
    if (!previous || !model.detailItem) return;
    previous.replaceWith(observation.renderReobservationSection(
      model.detailItem,
      model.detailChannels?.reobservation || {},
      reobservation,
      observationUi(),
    ));
  }

  async function refreshSelectedDetailAfterReobservation() {
    const detailUrl = model.detailUrl;
    const selectedRef = model.selectedRef;
    if (!detailUrl || !selectedRef) return;
    model.detailController?.abort();
    model.detailController = new AbortController();
    try {
      const payload = await readJson(detailUrl, model.detailController.signal);
      if (!payload?.item || typeof payload.item !== 'object') throw new Error('invalid_material_refresh_response');
      if (model.selectedRef !== selectedRef) return;
      reobservation.setReadError(null);
      renderDetail(model.listItem, payload.item, payload.channels || {});
    } catch (error) {
      if (error.name === 'AbortError') return;
      reobservation.setReadError(error.code || 'MATERIAL_DETAIL_REFRESH_UNAVAILABLE');
    }
  }

  function refreshCommentResearch() {
    if (!model.detailItem) return;
    const inspector = model.detailItem.inspector && typeof model.detailItem.inspector === 'object'
      ? model.detailItem.inspector
      : {};
    renderEvidence(model.listItem, model.detailItem, inspector, model.detailChannels?.comments || {});
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
    model.listItem = listItem;
    model.detailItem = item;
    model.detailChannels = channels;
    /* The header carries no prose at all: the selected row, one column to the left, already
     * names the work, its author and when it was last observed. The limitation is a fact about
     * this read, so it goes into the overview's fact grid with everything else. */
    refs.inspectorTitle.textContent = knownText(item.display?.title, item.display?.titleState, '标题当前未知');
    /* Both actions describe capabilities this build does not have. They stay disabled and say
     * why rather than becoming buttons that quietly do nothing. */
    refs.openSource.disabled = true;
    refs.openSource.title = '本机只读投影不保存平台原始地址，无法从这里跳转。';
    refs.requestMedia.disabled = true;
    refs.requestMedia.title = '补采需要采集授权，本页只读，不触发平台访问。';
    renderOverview(item, inspector, channels.reobservation || {});
    renderEvidence(listItem, item, inspector, channels.comments || {});
    renderMaterials(item, inspector, channels);
    renderTrace(inspector, channels.provenance || {});
  }

  /* Translates the limitation enums this page actually sees. An unmapped code still shows its
   * raw value as the technical key beside this line, so a new one degrades to honest rather
   * than to silence. */
  function limitationCopy(code) {
    return {
      OTHER_LANES_NOT_EVALUATED: '其他材料通道尚未评估',
      SOURCE_INCOMPLETE: '来源信息不完整',
      ACTUAL_CANDIDATE_NOT_REPORTED_BY_PRODUCER: '采集端没有回报实际使用的候选地址',
      COMMENTS_NOT_EVALUATED: '评论尚未评估',
      MEDIA_NOT_EVALUATED: '媒体尚未评估',
      RAW_BODY_NOT_RETURNED: '正文原文不在本次读取范围内',
      LANE_NOT_EVALUATED: '该通道尚未评估',
    }[code] || '来源未完整表达';
  }

  /* Names each dimension by the state its segment is drawn in. The rail already carries this,
   * but only to a mouse: without hovering every cell there was no way to tell which of the five
   * is the filled one. Stating it turns the legend into the answer, which is why there is no
   * separate summary line above it saying the same thing again. */
  function completenessLegend(summary) {
    const legend = node('div', 'ev-completeness-legend');
    summary.segments.forEach((segment) => {
      const cell = node('span', null, segment.label);
      cell.dataset.fill = segment.fill;
      cell.title = segment.applicable
        ? `${segment.label}：${stateMeta(segment.state)[0]}`
        : `${segment.label}：本作品不适用，不计入分母`;
      legend.append(cell);
    });
    return legend;
  }

  function renderOverview(item, inspector, reobservationChannel) {
    const panel = panels.get('overview');
    panel.replaceChildren();

    // The ratio goes the same way it went in the table: the rail draws each dimension's state
    // and the labels under it name them, so 2 / 5 is a third encoding of what is already there.
    const summaryBlock = materialBlock(item);
    const overview = section('材料完整度');
    overview.append(summaryBlock.rail);
    overview.append(completenessLegend(summaryBlock.summary));
    panel.append(overview);

    const identity = section('作品字段与来源');
    identity.append(factGrid([
      ['稳定引用', item.identity?.publicRef || '当前未知'],
      ['平台', item.identity?.platform || '当前未知'],
      ['标题', knownText(item.display?.title, item.display?.titleState), item.display?.titleState || 'UNKNOWN'],
      ['发布时间', publishedCopy(item, false, false), item.display?.publishedAtState || 'UNKNOWN'],
      ['时间来源字段', item.display?.publishedAtSourceField || '当前未知', item.display?.publishedAtSourceKind || 'unknown'],
      ['时间精度', item.display?.publishedAtPrecision || 'unknown', item.display?.publishedAtParserVersion || null],
      ['最近观察', item.summary?.lastObservedAt || '当前未知'],
      ['当前主要限制',
        item.summary?.primaryLimitation && item.summary.primaryLimitation !== 'NONE'
          ? limitationCopy(item.summary.primaryLimitation)
          : '本次读取没有记录额外限制',
        item.summary?.primaryLimitation || null],
    ]));
    panel.append(identity);

    panel.append(observation.renderReobservationSection(
      item,
      reobservationChannel,
      reobservation,
      observationUi(),
    ));

    const originContext = section('来源与溯源');
    originContext.append(
      authorFact(item),
      factGrid([['来源监控', monitoringOriginCopy(item)]]),
    );
    panel.append(originContext);

    const current = section('当前互动状态与变化');
    const currentMetrics = inspector.engagementCurrent?.metrics && typeof inspector.engagementCurrent.metrics === 'object'
      ? inspector.engagementCurrent.metrics
      : {};
    current.append(factGrid([
      observation.currentMetricFact('点赞', currentMetrics.likeCount),
      observation.currentMetricFact('评论', currentMetrics.commentCount),
      observation.currentMetricFact('收藏', currentMetrics.collectCount),
      observation.currentMetricFact('分享', currentMetrics.shareCount),
    ]));
    panel.append(current);

    const detailCurrent = section('详情字段当前事实');
    const fields = inspector.detailCurrent && typeof inspector.detailCurrent === 'object' ? inspector.detailCurrent : {};
    detailCurrent.append(factGrid([
      observation.detailCurrentFact('标题来源', fields.title),
      observation.detailCurrentFact('作者来源', fields.creator),
      observation.detailCurrentFact('发布时间来源', fields.publishedAt),
      observation.detailCurrentFact('正文字段', fields.body),
    ]));
    panel.append(detailCurrent);

    /* Platform-reported counts and locally retained counts are two different facts and are
     * never merged into one number. */
    const platform = section('平台计数与本地留存');
    const comments = inspector.commentsCoverage || {};
    platform.append(factGrid([
      ['平台点赞', knownText(item.display?.engagement?.likeCount, item.display?.engagement?.likeCountState), item.display?.engagement?.likeCountState || 'UNKNOWN'],
      ['平台评论数', knownText(item.display?.engagement?.commentCount, item.display?.engagement?.commentCountState), item.display?.engagement?.commentCountState || 'UNKNOWN'],
      ['平台收藏', knownText(item.display?.engagement?.collectCount, item.display?.engagement?.collectCountState), item.display?.engagement?.collectCountState || 'UNKNOWN'],
      ['平台分享', knownText(item.display?.engagement?.shareCount, item.display?.engagement?.shareCountState), item.display?.engagement?.shareCountState || 'UNKNOWN'],
      ['本地留存评论', Number.isFinite(Number(comments.retained)) ? comments.retained : '当前未知'],
      ['已去重评论身份', Number.isFinite(Number(comments.uniqueCollectedCount)) ? comments.uniqueCollectedCount : '当前未知', ],
      ['本地留存边界', comments.stoppedReason ? `停止原因 ${comments.stoppedReason}` : '当前未表达', comments.collectionState || 'UNKNOWN'],
    ]));
    panel.append(platform);

    const lanes = section('材料通道状态');
    const board = node('div', 'ev-detail-lanes');
    laneOrder.forEach((lane) => board.append(laneCell(item, lane)));
    lanes.append(board);
    panel.append(lanes);

    panel.append(observation.engagementTimeline(inspector, observationUi()));

    const author = section('作者上下文');
    const context = inspector.authorContext;
    if (!context || typeof context !== 'object') {
      author.append(sourceIncompleteBlock('当前详情没有合格作者上下文；这不表示作者不存在。'));
    } else {
      author.append(factGrid([
        ['显示名', knownText(context.displayName, context.displayNameState), context.displayNameState || 'UNKNOWN'],
        ['简介', context.biographyState === 'KNOWN' ? '已形成状态化上下文' : '当前未知', context.biographyState || 'UNKNOWN'],
        ['关注者数', context.followerCountState === 'KNOWN' && context.followerCount !== null ? context.followerCount : '当前未知', context.followerCountState || 'UNKNOWN'],
        ['观察时间', context.observedAt || '当前未知'],
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

    const strongest = section('列表引用的原声');
    const fragment = listItem?.evidenceFragment;
    if (!fragment) {
      strongest.append(sourceIncompleteBlock('这个作品还没有可引用的原文、评论或图片文字。这只描述当前已接纳的材料，不表示平台上没有内容。'));
    } else {
      strongest.append(evidenceBlock(fragment));
      strongest.append(factGrid([
        ['来源类型', fragmentSourceLabels[fragment.sourceKind] || '未表达', fragment.sourceKind || 'UNKNOWN'],
        ['选取依据', fragment.selectionBasis === 'SEARCH_MATCH' ? '包含当前检索词' : '当前最具作者性的可读材料', fragment.selectionBasis || 'UNKNOWN'],
        ['是否截断', fragment.truncated ? '是，已按展示边界截断' : '否'],
        ['来源引用', fragment.sourceRef || '当前未表达', fragment.sourceRef ? null : 'UNKNOWN'],
      ]));
      if (fragment.slotKey) {
        strongest.append(factGrid([['所在媒体对象', fragment.slotKey]]));
      }
    }
    panel.append(strongest);

    /* Raw material is shown as-is. Nothing on this page replaces an original sentence with a
     * generated summary of it. */
    const texts = derivedTexts(inspector);
    const machine = section('图片文字与转录原文');
    if (texts.length === 0) {
      machine.append(sourceIncompleteBlock('当前详情没有已识别的图片文字或转录文本；不把缺少记录写成处理成功或失败。'));
    } else {
      // material_evidence_fragment.rs already excludes cover OCR from row-level quotation: a
      // cover is a designed graphic, and OCR of one comes back as noise. That judgement existed
      // only in the read model, so this surface presented the same noise as acquired evidence.
      // The text is still shown -- it is real material -- but it is no longer presented as
      // something a reader can quote.
      machine.append(node('p', 'ev-section-note', '以下为机器识别结果，与原图可能有出入。'));
      const coverSlotKeys = new Set(
        (Array.isArray(inspector.mediaSlots) ? inspector.mediaSlots : [])
          .filter((slot) => slot?.purpose === 'cover' && slot.slotKey)
          .map((slot) => slot.slotKey),
      );
      texts.forEach((derivative) => {
        const entry = node('article', 'ev-derivative');
        const isCover = coverSlotKeys.has(derivative.slotKey);
        const head = node('div', 'ev-derivative-head');
        head.append(
          node('strong', null, fragmentSourceLabels[derivative.kind] || derivative.kind || '派生类型当前未知'),
          tech(derivative.slotKey || '槽位未知'),
        );
        if (isCover) head.append(node('span', 'ev-inline-note', '封面图 · 不宜直接引用'));
        entry.append(head, node('q', null, derivative.displayText), stateLine(derivative.state || 'UNKNOWN'));
        if (isCover) {
          entry.append(node('p', 'ev-section-note', '封面是设计排版，机器识别结果常为噪声；列表引用的原声不采用它。'));
        }
        machine.append(entry);
      });
    }
    panel.append(machine);

    const discussion = node('div', 'ev-observation-discussion');
    observation.renderDiscussion(
      item,
      inspector,
      commentChannel,
      commentResearch,
      discussion,
      discussionUi(),
    );
    panel.append(discussion);
  }

  /* -------------------------------------------------------- LOCAL MEDIA */

  /* Builds the rail's object list. The author's avatar is excluded — it belongs to the author,
   * not to this work's material. Ordering is by the producer's own sequence, which is the only
   * order the system actually holds; `displayOrderState` is UNKNOWN upstream, so the rail says
   * 采集顺序 and never claims the platform's arrangement. */
  function mediaObjectsFrom(inspector, fragment, { includeAvatar = false } = {}) {
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
      .filter((slot) => includeAvatar || slot?.purpose !== 'author_avatar')
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
      const jump = node('button', 'ev-media-jump', '跳到命中');
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

    /* Pointer capture is taken only once the pointer has actually travelled — never on
     * pointerdown. While an element holds the capture the browser retargets the following
     * `click` to that element, so capturing up front sent every click to the rail instead of to
     * the card under the cursor, and opening an image by clicking it silently stopped working.
     * A press that never moves now stays an ordinary click on the card. */
    const DRAG_THRESHOLD = 4;
    let pressed = false;
    let dragging = false;
    let dragOrigin = 0;
    let scrollOrigin = 0;
    let moved = 0;
    viewport.addEventListener('pointerdown', (event) => {
      if (event.pointerType === 'touch' || event.button !== 0) return;
      pressed = true;
      dragging = false;
      moved = 0;
      dragOrigin = event.clientX;
      scrollOrigin = viewport.scrollLeft;
    });
    viewport.addEventListener('pointermove', (event) => {
      if (!pressed) return;
      const delta = event.clientX - dragOrigin;
      moved = Math.max(moved, Math.abs(delta));
      if (!dragging) {
        if (moved <= DRAG_THRESHOLD) return;
        dragging = true;
        viewport.dataset.grabbing = 'true';
        viewport.setPointerCapture(event.pointerId);
      }
      viewport.scrollLeft = scrollOrigin - delta;
    });
    const endDrag = (event) => {
      if (!pressed) return;
      pressed = false;
      if (dragging) {
        dragging = false;
        delete viewport.dataset.grabbing;
        if (viewport.hasPointerCapture?.(event.pointerId)) viewport.releasePointerCapture(event.pointerId);
        /* A drag is not a click. Without this a dragged rail would open the viewer on release. */
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

    const local = section('本机媒体材料', String(objects.length).padStart(2, '0'));
    if (objects.length === 0) {
      const empty = node('div', 'ev-inline-state');
      empty.dataset.tone = 'warning';
      empty.append(
        node('strong', null, '当前没有本地媒体对象'),
        node('p', null, '这只说明本机还没有留存这个作品的媒体副本；这不表示作品没有媒体。'),
      );
      const action = node('div', 'ev-channel-actions');
      const request = node('button', 'ev-button ev-button--secondary', '补采媒体');
      request.type = 'button';
      request.disabled = true;
      request.title = '补采需要采集授权，本页只读，不触发平台访问。';
      action.append(request, node('span', 'ev-inline-note', '本页不授权采集'));
      empty.append(action);
      local.append(empty);
    } else {
      local.append(renderMediaRail(objects));
    }
    panel.append(local);

    const ledger = section('材料处理台账');
    const receipt = channels.media?.receipt || inspector.mediaSlotsReceipt;
    ledger.append(factGrid([
      ['槽位总数', receipt?.total ?? '当前未知'],
      ['本次返回', receipt?.returned ?? '当前未知'],
      ['是否截断', receipt?.truncated === true ? '是，可继续读取' : receipt?.truncated === false ? '否' : '当前未知'],
    ]));
    /* The ledger accounts for every local object including the author's avatar; the browsing
     * rail above deliberately leaves the avatar out, because it belongs to the author rather
     * than to this work's own sequence of images. */
    const ledgerObjects = mediaObjectsFrom(inspector, fragment, { includeAvatar: true });
    ledgerObjects.forEach((object) => {
      const entry = node('article', 'ev-derivative');
      const head = node('div', 'ev-derivative-head');
      head.append(
        node('strong', null, `${String(object.index + 1).padStart(2, '0')} · ${purposeLabel(object.purpose)}`),
        tech(object.slotKey || '槽位未知'),
      );
      entry.append(head, factGrid([
        ['本地对象', object.objectRef || '当前未表达'],
        ['字节状态', stateMeta(object.bytesState)[0], object.bytesState],
        ['文字处理', object.derivedState ? stateMeta(object.derivedState)[0] : '未处理', object.derivedState || null],
        ['采集序号', object.slot.displayOrderState === 'KNOWN'
          ? String(object.ordinal)
          : `${object.ordinal}（平台排列顺序未经验证）`],
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
    resetZoom();
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

  /* Zoom lives on the image inside the frame; the frame clips. Panning is only meaningful once
   * the image is larger than its frame, so the offsets are clamped to the actual overflow —
   * without that the picture can be dragged off screen and the reader has to guess how to get
   * it back. */
  const ZOOM_MIN = 1;
  const ZOOM_MAX = 6;

  function clampPan() {
    const frame = refs.lightboxFrame.getBoundingClientRect();
    const naturalWidth = refs.lightboxImage.clientWidth;
    const naturalHeight = refs.lightboxImage.clientHeight;
    const overflowX = Math.max(0, naturalWidth * model.zoom - frame.width);
    const overflowY = Math.max(0, naturalHeight * model.zoom - frame.height);
    model.panX = Math.min(0, Math.max(-overflowX, model.panX));
    model.panY = Math.min(0, Math.max(-overflowY, model.panY));
  }

  function paintZoom() {
    clampPan();
    refs.lightboxImage.style.transform = `translate(${model.panX}px, ${model.panY}px) scale(${model.zoom})`;
    refs.lightboxFrame.dataset.zoomed = String(model.zoom > 1);
    refs.zoomReset.textContent = `${Math.round(model.zoom * 100)}%`;
    refs.zoomOut.disabled = model.zoom <= ZOOM_MIN + 0.001;
    refs.zoomIn.disabled = model.zoom >= ZOOM_MAX - 0.001;
  }

  /* Zooms about a point so the pixel under the cursor stays under the cursor. Without an anchor
   * the picture appears to drift away from whatever the reader was looking at. */
  function setZoom(next, anchorX = null, anchorY = null) {
    const target = Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, next));
    if (target === model.zoom) return;
    if (anchorX !== null) {
      const ratio = target / model.zoom;
      model.panX = anchorX - ratio * (anchorX - model.panX);
      model.panY = anchorY - ratio * (anchorY - model.panY);
    }
    model.zoom = target;
    if (target === ZOOM_MIN) {
      model.panX = 0;
      model.panY = 0;
    }
    paintZoom();
  }

  function resetZoom() {
    model.zoom = 1;
    model.panX = 0;
    model.panY = 0;
    paintZoom();
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
    resetZoom();
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
    const channel = section('来源轨迹通道');
    channel.append(factGrid([
      ['总数', receipt?.total ?? '当前未知'],
      ['本次返回', receipt?.returned ?? '当前未知'],
      ['是否截断', receipt?.truncated === true ? '是，可继续读取' : receipt?.truncated === false ? '否' : '当前未知'],
    ]));
    panel.append(channel);

    const refsSection = section('来源与执行血缘');
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

    const limits = section('限制与访问边界');
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

  /* The address is authoritative, including by omission: a parameter that is absent means the
   * control is at its default, not that it keeps whatever it happened to be. Leaving the old
   * value in place is what makes Back fail — stepping back from `?view=partial` to a bare URL
   * would otherwise keep the partial filter while the tab strip claimed 全部材料. */
  function restoreFromUrl() {
    const params = new URLSearchParams(window.location.search);
    refs.search.value = params.get('q') || '';
    selectByName.get('window').set(params.get('window') || 'latest_accepted_discovery');
    ['lane', 'laneState', 'mediaKind'].forEach((name) => selectByName.get(name).set(params.get(name) || ''));
    const sort = params.get('sort');
    setSort(SORT_OPTIONS.some((option) => option.value === sort) ? sort : 'latest_discovery');
    const view = params.get('view');
    model.activeView = view && viewFilters[view] ? view : 'all';
    const layout = params.get('layout');
    model.activeLayout = ['research', 'table', 'cover'].includes(layout) ? layout : 'research';
    const tab = params.get('tab');
    model.activeTab = tabs.some((candidate) => candidate.dataset.evTab === tab) ? tab : 'overview';
    /* Width is the one parameter whose default is a stored preference rather than a constant:
     * the panel size a person chose is meant to survive navigation. */
    const panel = params.get('panel');
    model.inspectorClosed = panel === 'closed';
    if (INSPECTOR_WIDTHS.includes(panel)) model.inspectorWidth = panel;
    else if (panel !== 'closed') model.inspectorWidth = storedInspectorWidth();
    model.selectedRef = params.get('work') || null;
    viewButtons.forEach((button) => button.setAttribute('aria-pressed', String(button.dataset.evView === model.activeView)));
    layoutButtons.forEach((button) => button.setAttribute('aria-pressed', String(button.dataset.evLayout === model.activeLayout)));
    syncFilterCount();
  }

  refs.form.addEventListener('submit', (event) => {
    event.preventDefault();
    model.activeView = 'all';
    viewButtons.forEach((button) => button.setAttribute('aria-pressed', String(button.dataset.evView === 'all')));
    loadList({ history: 'push' });
  });
  refs.filterToggle.addEventListener('click', () => togglePopover(refs.filterToggle, refs.filterPanel));
  document.addEventListener('pointerdown', (event) => {
    if (event.target.closest('.ev-popover-anchor')) return;
    /* Clicking inside the filter panel — on a label, on its background — should not dismiss it. */
    if (event.target.closest('.ev-filter-panel')) {
      filterSelects.forEach((control) => closePopover(control.toggle, control.list));
      return;
    }
    closeAllPopovers();
  });
  document.getElementById('ev-reset').addEventListener('click', () => {
    selectByName.get('window').set('latest_accepted_discovery');
    ['lane', 'laneState', 'mediaKind'].forEach((name) => selectByName.get(name).set(''));
    syncFilterCount();
    model.activeView = 'all';
    viewButtons.forEach((button) => button.setAttribute('aria-pressed', String(button.dataset.evView === 'all')));
    loadList({ history: 'push' });
  });
  filterNames.forEach((name) => {
    selectByName.get(name).onChange = () => {
      syncFilterCount();
      loadList({ history: 'push' });
    };
  });
  refs.sortToggle.addEventListener('click', () => togglePopover(refs.sortToggle, refs.sortList));
  [...refs.sortList.querySelectorAll('[data-ev-sort]')].forEach((option) => {
    option.addEventListener('click', () => {
      setSort(option.dataset.evSort);
      closePopover(refs.sortToggle, refs.sortList);
      refs.sortToggle.focus();
      loadList({ keepSelection: true, history: 'push' });
    });
  });
  refs.nextList.addEventListener('click', () => loadList({ append: true }));
  viewButtons.forEach((button) => button.addEventListener('click', () => {
    model.activeView = button.dataset.evView;
    viewButtons.forEach((candidate) => candidate.setAttribute('aria-pressed', String(candidate === button)));
    loadList({ history: 'push' });
  }));
  layoutButtons.forEach((button) => button.addEventListener('click', () => {
    model.activeLayout = button.dataset.evLayout;
    layoutButtons.forEach((candidate) => candidate.setAttribute('aria-pressed', String(candidate === button)));
    renderRows(false);
    syncUrl('push');
  }));
  widthButtons.forEach((button) => button.addEventListener('click', () => setInspectorWidth(button.dataset.evWidth)));
  refs.closeInspector.addEventListener('click', closeInspector);
  refs.reopenInspector.addEventListener('click', () => {
    openInspector();
    syncUrl();
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
  refs.zoomIn.addEventListener('click', () => setZoom(model.zoom * 1.5));
  refs.zoomOut.addEventListener('click', () => setZoom(model.zoom / 1.5));
  refs.zoomReset.addEventListener('click', resetZoom);

  function framePoint(event) {
    const frame = refs.lightboxFrame.getBoundingClientRect();
    return [event.clientX - frame.left, event.clientY - frame.top];
  }

  refs.lightboxFrame.addEventListener('wheel', (event) => {
    event.preventDefault();
    const [x, y] = framePoint(event);
    setZoom(model.zoom * (event.deltaY < 0 ? 1.12 : 1 / 1.12), x, y);
  }, { passive: false });

  /* Double click toggles between fit and a working magnification, anchored where the reader
   * clicked — the fastest way to inspect one corner of a text card and come back. */
  refs.lightboxFrame.addEventListener('dblclick', (event) => {
    const [x, y] = framePoint(event);
    if (model.zoom > 1) resetZoom();
    else setZoom(2.5, x, y);
  });

  let panning = false;
  let panOrigin = [0, 0];
  refs.lightboxFrame.addEventListener('pointerdown', (event) => {
    if (model.zoom <= 1) return;
    panning = true;
    panOrigin = [event.clientX - model.panX, event.clientY - model.panY];
    refs.lightboxFrame.dataset.grabbing = 'true';
    refs.lightboxFrame.setPointerCapture(event.pointerId);
  });
  refs.lightboxFrame.addEventListener('pointermove', (event) => {
    if (!panning) return;
    model.panX = event.clientX - panOrigin[0];
    model.panY = event.clientY - panOrigin[1];
    paintZoom();
  });
  const endPan = (event) => {
    if (!panning) return;
    panning = false;
    delete refs.lightboxFrame.dataset.grabbing;
    if (refs.lightboxFrame.hasPointerCapture?.(event.pointerId)) {
      refs.lightboxFrame.releasePointerCapture(event.pointerId);
    }
  };
  refs.lightboxFrame.addEventListener('pointerup', endPan);
  refs.lightboxFrame.addEventListener('pointercancel', endPan);

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
      } else if (event.key === '+' || event.key === '=') {
        event.preventDefault();
        setZoom(model.zoom * 1.5);
      } else if (event.key === '-' || event.key === '_') {
        event.preventDefault();
        setZoom(model.zoom / 1.5);
      } else if (event.key === '0') {
        event.preventDefault();
        resetZoom();
      } else if (event.key === 'Tab') {
        /* The dialog keeps focus: only its own controls are reachable while it is open. */
        const focusable = [refs.lightboxClose, refs.zoomOut, refs.zoomReset, refs.zoomIn, refs.lightboxPrev, refs.lightboxNext]
          .filter((control) => !control.disabled);
        if (!focusable.length) return;
        const index = focusable.indexOf(document.activeElement);
        event.preventDefault();
        const offset = event.shiftKey ? -1 : 1;
        focusable[(index + offset + focusable.length) % focusable.length].focus();
      }
      return;
    }
    const openSelect = filterSelects.find((control) => !control.list.hidden);
    if (event.key === 'Escape' && openSelect) {
      event.preventDefault();
      closePopover(openSelect.toggle, openSelect.list);
      openSelect.toggle.focus();
      return;
    }
    if (event.key === 'Escape' && (!refs.filterPanel.hidden || !refs.sortList.hidden)) {
      event.preventDefault();
      const toggle = refs.filterPanel.hidden ? refs.sortToggle : refs.filterToggle;
      closeAllPopovers();
      toggle.focus();
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
    activateTab(tabs.find((tab) => tab.dataset.evTab === model.activeTab) || tabs[0]);
    loadList({ keepSelection: true, revealUrlSelection: true });
  });
  window.matchMedia(DRAWER_QUERY).addEventListener('change', applyInspectorState);

  restoreFromUrl();
  applyInspectorState();
  clearInspector();
  activateTab(tabs.find((tab) => tab.dataset.evTab === model.activeTab) || tabs[0]);
  loadList({ keepSelection: true, revealUrlSelection: true });
})();
