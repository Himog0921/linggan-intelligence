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
  };
  const viewFilters = {
    all: {},
    partial: { laneState: 'PARTIAL' },
    risk: { laneState: 'RISK_CONTROL' },
    cleaned: { lane: 'media_bytes', laneState: 'BYTES_CLEANED' },
    restricted: { restriction: 'WITHDRAWN_OR_RESTRICTED' },
  };

  const refs = {
    form: document.getElementById('ev-query-form'),
    search: document.getElementById('ev-search'),
    window: document.getElementById('ev-window'),
    lane: document.getElementById('ev-lane'),
    laneState: document.getElementById('ev-lane-state'),
    mediaKind: document.getElementById('ev-media-kind'),
    receipt: document.getElementById('ev-read-receipt'),
    list: document.getElementById('ev-work-list'),
    feedback: document.getElementById('ev-feedback'),
    resultsCount: document.getElementById('ev-results-count'),
    nextList: document.getElementById('ev-next-list'),
    inspector: document.getElementById('ev-inspector'),
    inspectorTitle: document.getElementById('ev-inspector-title'),
    inspectorSummary: document.getElementById('ev-inspector-summary'),
    inspectorRef: document.getElementById('ev-inspector-ref'),
    inspectorFeedback: document.getElementById('ev-inspector-feedback'),
    back: document.getElementById('ev-back-to-list'),
    tableHead: document.getElementById('ev-table-head'),
  };
  const panels = new Map(
    [...document.querySelectorAll('[data-ev-panel]')].map((panel) => [panel.dataset.evPanel, panel]),
  );
  const tabs = [...document.querySelectorAll('[data-ev-tab]')];
  const viewButtons = [...document.querySelectorAll('[data-ev-view]')];
  const layoutButtons = [...document.querySelectorAll('[data-ev-layout]')];

  const model = {
    items: [],
    cursor: null,
    selectedRef: null,
    activeView: 'all',
    activeLayout: 'research',
    listController: null,
    detailController: null,
    commentController: null,
    commentUrl: null,
    commentCursor: null,
    commentItems: [],
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

  function stateTag(state) {
    const normalized = typeof state === 'string' && state ? state : 'UNKNOWN';
    const [label, tone] = stateMeta(normalized);
    const tag = node('span', `ev-state ev-state--${tone}`);
    tag.dataset.state = normalized;
    tag.append(node('i', 'ev-state-dot'), node('span', 'ev-state-label', label), tech(normalized));
    return tag;
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

  function explicitParams(cursor = null) {
    const params = new URLSearchParams();
    const query = refs.search.value.trim();
    if (query) params.set('q', query);
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

  function syncUrl() {
    const params = explicitParams();
    if (model.activeView !== 'all') params.set('view', model.activeView);
    if (model.activeLayout !== 'research') params.set('layout', model.activeLayout);
    const query = params.toString();
    history.replaceState(null, '', query ? `/corpus/evidence?${query}` : '/corpus/evidence');
  }

  function setFeedback(kind, title, detail, code) {
    refs.feedback.hidden = false;
    refs.feedback.dataset.kind = kind;
    refs.feedback.replaceChildren();
    const heading = node('h2', null, title);
    const copy = node('p', null, detail);
    refs.feedback.append(heading, copy);
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
    const strong = node('b', null, appended ? `已继续读取，当前显示 ${count} 个作品集合` : `读取成功，当前显示 ${count} 个作品集合`);
    main.append(strong, document.createTextNode(` · 扫描 ${payload.scannedCount ?? '未知'} · 作品级材料投影`));
    const meta = node('span', 'ev-receipt-meta');
    meta.append(tech(`AS OF ${payload.asOf || 'UNKNOWN'}`));
    if (payload.scanLimited) {
      meta.append(document.createTextNode(' · '));
      addTextWithTech(meta, '扫描预算已触发，可继续读取', 'SCAN LIMITED');
    }
    refs.receipt.append(main, meta);
    refs.resultsCount.textContent = `${count} 个作品集合`;
    refs.nextList.hidden = !payload.cursor;
    refs.nextList.disabled = !payload.cursor;
    refs.nextList.dataset.cursor = payload.cursor || '';
  }

  function laneSummary(item, lane) {
    return Array.isArray(item.laneSummaries)
      ? item.laneSummaries.find((summary) => summary?.lane === lane)
      : null;
  }

  function laneCell(item, lane) {
    const summary = laneSummary(item, lane);
    const cell = node('div', 'ev-lane');
    const name = node('span', 'ev-lane-name', laneLabels[lane]);
    const status = summary ? stateTag(summary.state) : stateTag('UNKNOWN');
    const count = node('span', 'ev-lane-count');
    if (summary?.valueState === 'KNOWN' && summary.retained !== null && summary.retained !== undefined) {
      count.textContent = `保留 ${summary.retained}`;
    } else {
      count.textContent = '数量未知';
    }
    cell.append(name, status, count);
    return cell;
  }

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
      preview.append(image);
      const selection = cover.selectedBy === 'explicit_cover' ? '平台封面' : '统一封面回退';
      preview.append(node('span', 'ev-preview-label', `${selection} · 本地副本`));
      preview.dataset.tone = 'available';
    } else {
      const [label] = stateMeta(coverState);
      preview.append(node('strong', null, label), tech(coverState));
    }
    return preview;
  }

  function authorAvatar(media, alt = '作品作者头像') {
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

  function creatorAndTargetFacts(item, compact = false) {
    const creator = knownText(item.display?.creatorDisplayName, item.display?.creatorState, '当前未知');
    const target = item.collectionContext?.targetDisplayState === 'KNOWN'
      ? item.collectionContext.targetDisplayName
      : '当前未知';
    const facts = node('div', compact ? 'ev-context-facts ev-context-facts--compact' : 'ev-context-facts');
    const creatorFact = node('div', 'ev-identity-fact ev-creator-fact');
    const creatorCopy = node('span');
    creatorCopy.append(node('small', null, '作品作者'), node('strong', null, creator));
    creatorFact.append(authorAvatar(item.media, `${creator}的头像`), creatorCopy);
    const targetFact = node('div', 'ev-identity-fact ev-target-fact');
    const targetCopy = node('span');
    targetCopy.append(node('small', null, '监控目标'), node('strong', null, target));
    targetFact.append(targetCopy, tech(item.collectionContext?.authorIdentityMatchState || 'NOT_VERIFIED'));
    facts.append(creatorFact, targetFact);
    return facts;
  }

  function rowFor(item) {
    const publicRef = item.identity?.publicRef;
    const row = node('article', `ev-work-row ev-work-row--${model.activeLayout}`);
    row.setAttribute('role', 'option');
    row.tabIndex = -1;
    row.dataset.publicRef = publicRef || '';
    row.dataset.platform = String(item.identity?.platform || 'unknown').toLowerCase();
    row.setAttribute('aria-selected', String(publicRef === model.selectedRef));

    const identity = node('div', 'ev-identity');
    const eyebrow = node('div', 'ev-eyebrow');
    eyebrow.append(
      node('span', null, item.identity?.platform?.toUpperCase() || 'PLATFORM UNKNOWN'),
      tech(publicRef ? `WORK ${publicRef}` : 'PUBLIC REF UNKNOWN'),
    );
    const title = node('h2', null, knownText(item.display?.title, item.display?.titleState, '标题当前未知'));
    const meta = node('div', 'ev-meta');
    const published = item.display?.publishedAtState === 'KNOWN'
      ? item.display?.publishedAt || '发布时间已知'
      : (item.display?.publishedAtState === 'SOURCE_TEXT_ONLY'
        ? `来源时间：${item.display?.publishedAtSourceText || '已观察'}`
        : '发布时间当前未知');
    meta.append(creatorAndTargetFacts(item), node('span', 'ev-time-line', published));
    const engagement = item.display?.engagement || {};
    const metrics = [
      knownMetric(engagement.likeCount, engagement.likeCountState, '赞'),
      knownMetric(engagement.commentCount, engagement.commentCountState, '评'),
      knownMetric(engagement.collectCount, engagement.collectCountState, '藏'),
      knownMetric(engagement.shareCount, engagement.shareCountState, '转'),
    ].filter(Boolean);
    if (metrics.length) meta.append(node('span', 'ev-engagement', metrics.join(' · ')));
    identity.append(eyebrow, title, meta);

    const lanes = node('div', 'ev-lane-board');
    laneOrder.forEach((lane) => lanes.append(laneCell(item, lane)));

    const limitation = node('div', 'ev-limitation');
    const primary = item.summary?.primaryLimitation || 'SOURCE_INCOMPLETE';
    addTextWithTech(limitation, primary === 'SOURCE_INCOMPLETE' ? '来源信息不完整' : '主要限制', primary);
    const observed = node('span', 'ev-observed');
    addTextWithTech(observed, `最近观察 ${item.summary?.lastObservedAt || '未知'}`, 'OBSERVED AT');

    if (model.activeLayout === 'table') {
      const work = node('div', 'ev-table-work');
      work.append(eyebrow, title);
      const context = node('div', 'ev-table-context');
      context.append(creatorAndTargetFacts(item, true));
      const time = node('div', 'ev-table-time', published);
      const status = node('div', 'ev-table-status');
      status.append(stateTag(laneSummary(item, 'detail')?.state || 'UNKNOWN'), observed);
      row.append(work, context, time, status);
    } else {
      row.append(previewBlock(item), identity, lanes, limitation, observed);
    }
    row.addEventListener('click', () => selectItem(item, true));
    row.addEventListener('keydown', (event) => onRowKeydown(event, item));
    return row;
  }

  function renderRows(appended) {
    if (!appended) refs.list.replaceChildren();
    refs.list.dataset.layout = model.activeLayout;
    refs.tableHead.hidden = model.activeLayout !== 'table';
    refs.tableHead.setAttribute('aria-hidden', String(model.activeLayout !== 'table'));
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

  async function loadList({ append = false } = {}) {
    model.listController?.abort();
    model.listController = new AbortController();
    const cursor = append ? refs.nextList.dataset.cursor : null;
    if (!append) {
      model.items = [];
      model.cursor = null;
      model.selectedRef = null;
      clearInspector();
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
      syncUrl();
      if (model.items.length === 0) {
        setFeedback('empty', '当前查询没有匹配的作品材料', '读取已经成功；这个结果只描述当前本地查询，不证明平台或现实中没有相关内容。', 'NO MATCHING MATERIAL');
      } else {
        clearFeedback();
        const selected = model.items.find((item) => item.identity?.publicRef === model.selectedRef) || model.items[0];
        await selectItem(selected, false);
      }
    } catch (error) {
      if (error.name === 'AbortError') return;
      model.items = [];
      renderRows(false);
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

  function clearInspector() {
    model.detailController?.abort();
    model.commentController?.abort();
    model.commentUrl = null;
    model.commentCursor = null;
    model.commentItems = [];
    refs.inspectorRef.textContent = 'SELECTION REQUIRED';
    refs.inspectorTitle.textContent = '请选择一个作品材料集合';
    refs.inspectorSummary.textContent = '右侧只核验当前选择，不补造未读取的详情。';
    refs.inspectorFeedback.hidden = false;
    refs.inspectorFeedback.replaceChildren(node('strong', null, '尚未选择作品'), tech('SELECTION REQUIRED'));
    panels.forEach((panel) => panel.replaceChildren());
  }

  async function selectItem(item, shouldScroll) {
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
    refs.inspectorFeedback.hidden = false;
    refs.inspectorFeedback.replaceChildren(node('strong', null, '正在读取当前作品详情'), tech('DETAIL READ'));
    refs.inspectorTitle.textContent = knownText(item.display?.title, item.display?.titleState, '标题当前未知');
    refs.inspectorRef.textContent = `WORK ${publicRef}`;
    refs.inspectorSummary.textContent = '详情、评论、媒体、派生与来源分别保留自己的读取回执。';
    model.detailController?.abort();
    model.detailController = new AbortController();
    try {
      const payload = await readJson(detailUrl, model.detailController.signal);
      if (!payload?.item || typeof payload.item !== 'object') throw new Error('invalid_material_detail_response');
      renderDetail(payload.item, payload.channels || {});
      refs.inspectorFeedback.hidden = true;
      if (shouldScroll && window.matchMedia('(max-width: 900px)').matches) {
        refs.inspector.scrollIntoView({ behavior: reducedMotion() ? 'auto' : 'smooth', block: 'start' });
      }
    } catch (error) {
      if (error.name === 'AbortError') return;
      refs.inspectorFeedback.hidden = false;
      refs.inspectorFeedback.replaceChildren();
      const strong = node('strong', null, '当前作品详情读取失败');
      refs.inspectorFeedback.append(strong, tech(error.code || 'MATERIAL DETAIL UNAVAILABLE'));
      panels.forEach((panel) => panel.replaceChildren(sourceIncompleteBlock('当前详情未读取，列表 lane 仍可查看，但不能据此补造 Inspector。')));
    }
  }

  function showInspectorSourceIncomplete(item, detail) {
    model.selectedRef = item?.identity?.publicRef || null;
    refs.inspectorRef.textContent = model.selectedRef ? `WORK ${model.selectedRef}` : 'SELECTION REQUIRED';
    refs.inspectorTitle.textContent = knownText(item?.display?.title, item?.display?.titleState, '标题当前未知');
    refs.inspectorSummary.textContent = detail;
    refs.inspectorFeedback.hidden = false;
    refs.inspectorFeedback.replaceChildren(node('strong', null, '来源信息不完整'), tech('SOURCE INCOMPLETE'));
    panels.forEach((panel) => panel.replaceChildren(sourceIncompleteBlock(detail)));
  }

  function sourceIncompleteBlock(detail) {
    const block = node('section', 'ev-inline-state');
    block.dataset.tone = 'warning';
    const heading = node('h3', null, '来源信息不完整');
    const copy = node('p', null, detail);
    block.append(heading, copy, tech('SOURCE INCOMPLETE'));
    return block;
  }

  function factGrid(rows) {
    const dl = node('dl', 'ev-facts');
    rows.forEach(([label, value, code]) => {
      dl.append(node('dt', null, label));
      const dd = node('dd');
      addTextWithTech(dd, value, code);
      dl.append(dd);
    });
    return dl;
  }

  function section(title, code) {
    const wrapper = node('section', 'ev-inspect-section');
    const heading = node('h3');
    addTextWithTech(heading, title, code);
    wrapper.append(heading);
    return wrapper;
  }

  function renderDetail(item, channels) {
    const inspector = item.inspector && typeof item.inspector === 'object' ? item.inspector : {};
    refs.inspectorTitle.textContent = knownText(item.display?.title, item.display?.titleState, '标题当前未知');
    refs.inspectorRef.textContent = `WORK ${item.identity?.publicRef || 'UNKNOWN'}`;
    refs.inspectorSummary.textContent = item.summary?.primaryLimitation
      ? `当前主要限制：${item.summary.primaryLimitation}`
      : '当前主要限制未由来源完整表达。';
    renderOverview(item, inspector);
    renderDiscussion(item, inspector, channels.comments || {});
    renderMedia(item.media, inspector, channels);
    renderProvenance(inspector, channels.provenance || {});
  }

  function renderOverview(item, inspector) {
    const panel = panels.get('overview');
    panel.replaceChildren();
    const identity = section('作品字段与来源', 'WORK MATERIAL');
    identity.append(factGrid([
      ['稳定引用', item.identity?.publicRef || '当前未知', 'PUBLIC REF'],
      ['平台', item.identity?.platform || '当前未知', 'PLATFORM'],
      ['标题', knownText(item.display?.title, item.display?.titleState), item.display?.titleState || 'UNKNOWN'],
      ['发布时间', item.display?.publishedAtState === 'KNOWN' ? item.display?.publishedAt || '已知' : (item.display?.publishedAtState === 'SOURCE_TEXT_ONLY' ? item.display?.publishedAtSourceText || '仅有来源文本' : '当前未知'), item.display?.publishedAtState || 'UNKNOWN'],
      ['时间来源字段', item.display?.publishedAtSourceField || '当前未知', item.display?.publishedAtSourceKind || 'unknown'],
      ['时间精度', item.display?.publishedAtPrecision || 'unknown', item.display?.publishedAtParserVersion || 'PARSER UNKNOWN'],
      ['点赞', knownText(item.display?.engagement?.likeCount, item.display?.engagement?.likeCountState), item.display?.engagement?.likeCountState || 'UNKNOWN'],
      ['评论', knownText(item.display?.engagement?.commentCount, item.display?.engagement?.commentCountState), item.display?.engagement?.commentCountState || 'UNKNOWN'],
      ['收藏', knownText(item.display?.engagement?.collectCount, item.display?.engagement?.collectCountState), item.display?.engagement?.collectCountState || 'UNKNOWN'],
      ['分享', knownText(item.display?.engagement?.shareCount, item.display?.engagement?.shareCountState), item.display?.engagement?.shareCountState || 'UNKNOWN'],
      ['最近观察', item.summary?.lastObservedAt || '当前未知', 'OBSERVED AT'],
    ]));
    panel.append(identity);

    const identityContext = section('作者与监控目标', 'SEPARATE SOURCE FACTS');
    identityContext.append(
      creatorAndTargetFacts(item),
      factGrid([
        ['作者身份关系', item.collectionContext?.authorIdentityMatchState === 'MATCHED' ? '已由平台作者 ID 证明一致' : '尚未证明监控目标就是作品作者', item.collectionContext?.authorIdentityMatchState || 'NOT_VERIFIED'],
      ]),
    );
    panel.append(identityContext);

    const timeline = section('互动数据观察时间线', 'ENGAGEMENT TIMELINE');
    const observations = Array.isArray(inspector.engagementTimeline) ? inspector.engagementTimeline : [];
    if (observations.length === 0) {
      timeline.append(sourceIncompleteBlock('当前详情没有互动数据观察时点；不把未知写成 0。'));
    } else {
      const list = node('div', 'ev-slot-list');
      observations.forEach((observation) => {
        const entry = node('article', 'ev-derivative');
        const metrics = [
          knownMetric(observation.likeCount, observation.likeCountState, '赞'),
          knownMetric(observation.commentCount, observation.commentCountState, '评'),
          knownMetric(observation.collectCount, observation.collectCountState, '藏'),
          knownMetric(observation.shareCount, observation.shareCountState, '转'),
        ].filter(Boolean);
        entry.append(
          node('strong', null, observation.observedAt || '观察时间当前未知'),
          node('p', null, metrics.length ? metrics.join(' · ') : '本时点互动字段均为当前未知'),
          tech((observation.sourceLane || 'source').toUpperCase()),
        );
        list.append(entry);
      });
      timeline.append(list);
    }
    panel.append(timeline);

    const lanes = section('材料通道状态', 'LANE STATUS');
    const board = node('div', 'ev-detail-lanes');
    laneOrder.forEach((lane) => board.append(laneCell(item, lane)));
    lanes.append(board);
    panel.append(lanes);

    const raw = section('正文与受限材料', 'MINIMUM NECESSARY');
    const bodyField = Array.isArray(inspector.overview?.fields)
      ? inspector.overview.fields.find((field) => field?.field === 'body')
      : null;
    const restricted = node('div', 'ev-inline-state');
    restricted.dataset.tone = 'restricted';
    restricted.append(
      node('strong', null, '普通详情不返回原始正文'),
      node('p', null, bodyField?.accessLevel === 'RESTRICTED_SOURCE'
        ? '正文属于受限来源材料；当前页面没有扩大其访问范围。'
        : '正文的访问级别未由来源完整表达。'),
      tech(bodyField?.accessLevel || 'SOURCE INCOMPLETE'),
    );
    raw.append(restricted);
    panel.append(raw);

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

  function receiptBlock(title, receipt, channelUrl) {
    const wrapper = node('div', 'ev-channel-receipt');
    const heading = node('div', 'ev-channel-title');
    heading.append(node('strong', null, title), tech('BOUNDED CHANNEL'));
    wrapper.append(heading);
    if (!receipt || typeof receipt !== 'object') {
      wrapper.append(sourceIncompleteBlock('当前详情没有提供该通道的读取回执。'));
      return wrapper;
    }
    wrapper.append(factGrid([
      ['总数', receipt.total ?? '当前未知', receipt.total === null || receipt.total === undefined ? 'UNKNOWN' : 'TOTAL'],
      ['本次返回', receipt.returned ?? '当前未知', receipt.returned === null || receipt.returned === undefined ? 'UNKNOWN' : 'RETURNED'],
      ['是否截断', receipt.truncated === true ? '是，可继续读取' : receipt.truncated === false ? '否' : '当前未知', 'TRUNCATED'],
      ['下一游标', receipt.nextCursor || '无', 'NEXT CURSOR'],
    ]));
    if (receipt.truncated && !channelUrl) {
      const note = node('div', 'ev-inline-state');
      note.dataset.tone = 'warning';
      note.append(node('strong', null, '继续读取入口尚未提供'), node('p', null, '回执表明通道已截断，但详情没有给出可调用的下一页 URL；页面不会猜测后端路由。'), tech('SOURCE INCOMPLETE'));
      wrapper.append(note);
    }
    return wrapper;
  }

  function renderDiscussion(item, inspector, commentChannel) {
    const panel = panels.get('discussion');
    panel.replaceChildren();
    const commentsCoverage = section('评论与回复覆盖', 'COVERAGE');
    commentsCoverage.append(factGrid([
      ['评论状态', stateMeta(laneSummary(item, 'comments')?.state || 'UNKNOWN')[0], laneSummary(item, 'comments')?.state || 'UNKNOWN'],
      ['评论数量', inspector.commentsCoverage?.countState === 'KNOWN' ? inspector.commentsCoverage?.count : '当前未知', inspector.commentsCoverage?.countState || 'UNKNOWN'],
      ['回复状态', stateMeta(laneSummary(item, 'replies')?.state || 'UNKNOWN')[0], laneSummary(item, 'replies')?.state || 'UNKNOWN'],
      ['回复数量', inspector.repliesCoverage?.countState === 'KNOWN' ? inspector.repliesCoverage?.count : '当前未知', inspector.repliesCoverage?.countState || 'UNKNOWN'],
    ]));
    panel.append(commentsCoverage);

    const channelUrl = sameOriginPath(commentChannel?.url, [`${API_ROOT}/`]);
    panel.append(receiptBlock('评论研究通道', commentChannel?.receipt || inspector.commentsReceipt, channelUrl));

    const access = section('本机授权评论研究', 'LOCAL AUTHORIZED RESEARCH');
    const accessState = node('div', 'ev-inline-state');
    accessState.dataset.tone = 'restricted';
    accessState.append(
      node('strong', null, '原文只在本机授权详情中按页读取'),
      node('p', null, '作者只显示匿名上下文；页面不会渲染平台用户标识，也不会把评论原文放回普通列表。'),
      tech('IDENTITY WITHHELD'),
    );
    access.append(accessState);
    const controls = node('div', 'ev-channel-actions');
    const load = node('button', 'ev-button ev-button--secondary', '读取评论原文');
    load.type = 'button';
    load.disabled = !channelUrl;
    load.addEventListener('click', () => loadComments(false));
    controls.append(load);
    if (!channelUrl) controls.append(tech('SOURCE INCOMPLETE'));
    access.append(controls, node('div', 'ev-comment-list'));
    panel.append(access);
    model.commentUrl = channelUrl;
    model.commentCursor = null;
    model.commentItems = [];
  }

  async function loadComments(append) {
    if (!model.commentUrl) return;
    model.commentController?.abort();
    model.commentController = new AbortController();
    const panel = panels.get('discussion');
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
        tech(comment.sourceRef ? `SOURCE ${comment.sourceRef}` : 'SOURCE INCOMPLETE'),
      );
      const body = node('p', null, comment.bodyState === 'KNOWN' && comment.body !== null ? comment.body : '评论正文当前未知');
      const meta = node('div', 'ev-comment-meta');
      addTextWithTech(meta, comment.bodyTruncated ? '本条正文已在读取边界截断' : '本条正文未在通道内截断', comment.bodyTruncated ? 'TRUNCATED' : 'RETURNED');
      article.append(header, body, meta);
      list.append(article);
    });
    const receipt = node('div', 'ev-comment-receipt');
    addTextWithTech(receipt, `总数 ${payload.total ?? '未知'} · 返回 ${payload.returned ?? '未知'} · ${payload.truncated ? '可继续读取' : '当前页未截断'}`, 'ANONYMOUS RESEARCH VIEW');
    list.append(receipt);
  }

  function renderMedia(mediaResource, inspector, channels) {
    const panel = panels.get('media');
    panel.replaceChildren();
    const media = mediaResource && typeof mediaResource === 'object' ? mediaResource : {};
    const mediaReceipt = channels.media?.receipt || inspector.mediaSlotsReceipt;
    const derivativeReceipt = channels.derivatives?.receipt || inspector.derivativesReceipt;
    panel.append(receiptBlock('媒体槽位通道', mediaReceipt, channels.media?.url));
    const mediaSection = section('统一媒体资源', media.contractVersion || 'MEDIA RESOURCE NOT OBSERVED');
    const slots = [
      ...(media.avatar?.slotKey ? [media.avatar] : []),
      ...(Array.isArray(media.coverCandidates) ? media.coverCandidates : []),
      ...(Array.isArray(media.images) ? media.images : []),
      ...(Array.isArray(media.video?.items) ? media.video.items : []),
    ];
    if (slots.length === 0) {
      mediaSection.append(sourceIncompleteBlock('统一媒体资源当前没有可读取的内容媒体；这不表示作品没有媒体。'));
    } else {
      const list = node('div', 'ev-slot-list');
      slots.forEach((slot) => list.append(renderSlot(slot)));
      mediaSection.append(list);
    }
    panel.append(mediaSection, receiptBlock('派生材料通道', derivativeReceipt, channels.derivatives?.url));

    const derivatives = section('OCR / ASR 标准资源', 'UNIFIED DERIVATIVES');
    const items = [
      ...(Array.isArray(media.ocr?.resources) ? media.ocr.resources : []),
      ...(Array.isArray(media.transcript?.resources) ? media.transcript.resources : []),
    ];
    if (items.length === 0) {
      derivatives.append(sourceIncompleteBlock('统一媒体资源当前没有 OCR / ASR 文件；不把缺少记录写成处理成功或失败。'));
    } else {
      items.forEach((item) => derivatives.append(renderDerivative(item)));
    }
    panel.append(derivatives);
  }

  function renderSlot(slot) {
    const article = node('article', 'ev-slot');
    const title = node('div', 'ev-slot-head');
    const purposeLabels = {
      author_avatar: '作者头像',
      cover: '作品封面',
      body_image: '正文图片',
      video: '作品视频',
      live_photo: '动态照片',
    };
    title.append(node('strong', null, purposeLabels[slot.purpose] || slot.purpose || '用途当前未知'), tech(slot.slotKey || 'SLOT UNKNOWN'));
    article.append(title, factGrid([
      ['显示顺序', slot.displayOrderState === 'KNOWN' ? slot.displayOrdinal : '当前未知', slot.displayOrderState || 'UNKNOWN'],
      ['来源代次', slot.origin?.sourceGeneration ?? '当前未知', 'GENERATION'],
      ['候选断言数', slot.origin?.candidateUriCount ?? '当前未知', slot.origin?.candidateSetState || 'UNKNOWN'],
      ['字节状态', stateMeta(slot.bytesState || 'UNKNOWN')[0], slot.bytesState || 'UNKNOWN'],
      ['副本状态', slot.replicaState || '当前未知', slot.replicaState || 'UNKNOWN'],
      ['处置状态', stateMeta(slot.dispositionState || 'UNKNOWN')[0], slot.dispositionState || 'UNKNOWN'],
    ]));
    const asset = sameOriginPath(slot.localAssetUrl, ['/api/local/media/', '/api/local/derivative/']);
    const blob = slot.blob && typeof slot.blob === 'object' ? slot.blob : null;
    if (asset && blob?.deliveryState === 'INLINE_SAFE') {
      const preview = node('div', 'ev-controlled-preview');
      const mime = blob.deliveryMimeType || blob.declaredMimeType || '';
      if (mime.startsWith('image/')) {
        const image = node('img');
        image.src = asset;
        image.alt = 'Linggan 受控本地媒体副本';
        image.width = 320;
        image.height = 220;
        image.loading = 'lazy';
        preview.append(image);
      } else if (mime.startsWith('video/')) {
        const video = node('video');
        video.src = asset;
        video.controls = true;
        video.preload = 'metadata';
        video.setAttribute('aria-label', 'Linggan 受控本地视频副本');
        preview.append(video);
      } else {
        preview.append(node('p', null, '本地副本已验证，但当前安全类型不在页面内联集合中。'), tech('SOURCE INCOMPLETE'));
      }
      article.append(preview);
    } else if (asset) {
      const blocked = node('div', 'ev-inline-state');
      blocked.dataset.tone = 'warning';
      blocked.append(node('strong', null, '当前媒体不内联展示'), node('p', null, '受控句柄存在，但媒体类型未被标记为可安全内联。'), tech(blob?.deliveryState || 'UNSUPPORTED MEDIA TYPE'));
      article.append(blocked);
    }
    return article;
  }

  function renderDerivative(item) {
    const article = node('article', 'ev-derivative');
    const heading = node('div', 'ev-slot-head');
    heading.append(node('strong', null, item.kind || '派生类型当前未知'), stateTag(item.state || 'UNKNOWN'));
    article.append(heading, factGrid([
      ['处理版本', item.processorVersion || '当前未知', 'PROCESSOR VERSION'],
      ['来源范围', item.sourceScope || '当前未知', 'SOURCE SCOPE'],
      ['失败/限制原因', item.reason || '无已知原因', item.reason ? 'REASON' : 'UNKNOWN'],
    ]));
    if (typeof item.displayText === 'string' && item.displayText.trim()) {
      const text = node('div', 'ev-derived-text');
      text.append(
        node('strong', null, item.kind === 'asr_text' ? '视频转录摘要' : '识别文字摘要'),
        node('p', null, item.displayText),
        tech(item.languageTag || item.languageState || 'LANGUAGE UNKNOWN'),
      );
      article.append(text);
    }
    const asset = sameOriginPath(item.sourceLocation?.localAssetUrl, ['/api/local/derivative/']);
    if (asset && item.state === 'ACQUIRED') {
      const link = node('a', 'ev-asset-link', '打开受控本地派生材料');
      link.href = asset;
      link.target = '_blank';
      link.rel = 'noopener';
      article.append(link);
    }
    return article;
  }

  function renderProvenance(inspector, provenanceChannel) {
    const panel = panels.get('provenance');
    panel.replaceChildren();
    const provenance = inspector.provenance && typeof inspector.provenance === 'object' ? inspector.provenance : null;
    panel.append(receiptBlock('来源血缘通道', provenanceChannel.receipt || provenance?.receipt, provenanceChannel.url));
    const refsSection = section('来源与执行血缘', 'PROVENANCE');
    if (!provenance) {
      refsSection.append(sourceIncompleteBlock('当前详情没有返回来源血缘。'));
    } else {
      const groups = [
        ['Package', provenance.packageRefs],
        ['Task', provenance.taskRefs],
        ['Attempt', provenance.attemptRefs],
        ['Receipt', provenance.receiptRefs],
        ['Target', provenance.targetRefs],
        ['Work Order', provenance.workOrderRefs],
        ['Producer', provenance.producers],
      ];
      groups.forEach(([label, values]) => {
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
        const item = node('li');
        addTextWithTech(item, '当前限制', value);
        list.append(item);
      });
      limits.append(list);
    }
    panel.append(limits);
  }

  function activateTab(tab, focus = false) {
    tabs.forEach((candidate) => {
      const selected = candidate === tab;
      candidate.setAttribute('aria-selected', String(selected));
      candidate.tabIndex = selected ? 0 : -1;
      panels.get(candidate.dataset.evTab)?.toggleAttribute('hidden', !selected);
    });
    if (focus) tab.focus();
  }

  function reducedMotion() {
    return window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  }

  function restoreFromUrl() {
    const params = new URLSearchParams(window.location.search);
    refs.search.value = params.get('q') || '';
    if ([...refs.window.options].some((option) => option.value === params.get('window'))) refs.window.value = params.get('window');
    if ([...refs.lane.options].some((option) => option.value === params.get('lane'))) refs.lane.value = params.get('lane');
    if ([...refs.laneState.options].some((option) => option.value === params.get('laneState'))) refs.laneState.value = params.get('laneState');
    if ([...refs.mediaKind.options].some((option) => option.value === params.get('mediaKind'))) refs.mediaKind.value = params.get('mediaKind');
    const view = params.get('view');
    if (view && viewFilters[view]) model.activeView = view;
    const layout = params.get('layout');
    if (['research', 'table', 'cover'].includes(layout)) model.activeLayout = layout;
    viewButtons.forEach((button) => button.setAttribute('aria-pressed', String(button.dataset.evView === model.activeView)));
    layoutButtons.forEach((button) => button.setAttribute('aria-pressed', String(button.dataset.evLayout === model.activeLayout)));
  }

  refs.form.addEventListener('submit', (event) => {
    event.preventDefault();
    model.activeView = 'all';
    viewButtons.forEach((button) => button.setAttribute('aria-pressed', String(button.dataset.evView === 'all')));
    loadList();
  });
  document.getElementById('ev-reset').addEventListener('click', () => {
    refs.form.reset();
    model.activeView = 'all';
    viewButtons.forEach((button) => button.setAttribute('aria-pressed', String(button.dataset.evView === 'all')));
    loadList();
  });
  [refs.window, refs.lane, refs.laneState, refs.mediaKind].forEach((control) => control.addEventListener('change', () => loadList()));
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
    const row = refs.list.querySelector(`[data-public-ref="${CSS.escape(model.selectedRef || '')}"]`);
    if (!row) return;
    row.scrollIntoView({ behavior: reducedMotion() ? 'auto' : 'smooth', block: 'center' });
    requestAnimationFrame(() => row.focus({ preventScroll: true }));
  });

  restoreFromUrl();
  clearInspector();
  activateTab(tabs[0]);
  loadList();
})();
