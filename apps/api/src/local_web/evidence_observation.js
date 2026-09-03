(() => {
  'use strict';

  const TERMINAL_TASK_STATES = new Set(['ACCEPTED', 'EXPIRED_WITHOUT_RECEIPT', 'COMPLETED_WITHOUT_RECEIPT']);

  function createController({ apiRoot, sameOriginPath, readJson, onChange, onTerminal, postJson = postJsonSameOrigin, timer = window }) {
    const state = {
      controller: null,
      timerId: null,
      actionUrl: null,
      statusUrl: null,
      operation: null,
      error: null,
      readError: null,
      pollError: null,
    };

    function notify() {
      onChange?.(snapshot());
    }

    function snapshot() {
      return {
        actionUrl: state.actionUrl,
        statusUrl: state.statusUrl,
        operation: state.operation,
        error: state.error,
        readError: state.readError,
        pollError: state.pollError,
      };
    }

    function availability(item, channel) {
      const actionUrl = sameOriginPath(channel?.url, [`${apiRoot}/`]);
      return {
        actionable: item?.identity?.platform === 'xhs' && channel?.eligible === true && Boolean(actionUrl),
        actionUrl,
        supported: channel?.supported === true,
        eligible: channel?.eligible === true,
        reason: channel?.reason || 'REOBSERVATION_ELIGIBILITY_UNKNOWN',
      };
    }

    function reset() {
      state.controller?.abort();
      if (state.timerId) timer.clearTimeout(state.timerId);
      Object.assign(state, {
        controller: null,
        timerId: null,
        actionUrl: null,
        statusUrl: null,
        operation: null,
        error: null,
        readError: null,
        pollError: null,
      });
    }

    async function request(actionUrl) {
      if (!actionUrl || state.operation?.leaseRef) return;
      state.actionUrl = actionUrl;
      state.error = null;
      state.readError = null;
      state.pollError = null;
      state.controller?.abort();
      state.controller = new AbortController();
      notify();
      try {
        const payload = await postJson(actionUrl, state.controller.signal);
        if (!payload?.operation || typeof payload.operation !== 'object') throw new Error('invalid_reobservation_response');
        state.operation = payload.operation;
        state.statusUrl = sameOriginPath(payload.statusUrl, [`${apiRoot}/`]);
        await afterOperationChange();
      } catch (error) {
        if (error.name === 'AbortError') return;
        state.error = error.code || 'REOBSERVATION_UNAVAILABLE';
        notify();
      }
    }

    async function poll() {
      if (!state.statusUrl || !state.operation) return;
      state.controller?.abort();
      state.controller = new AbortController();
      try {
        const payload = await readJson(state.statusUrl, state.controller.signal);
        if (!payload?.operation || typeof payload.operation !== 'object') throw new Error('invalid_reobservation_status');
        state.operation = { ...state.operation, ...payload.operation };
        state.pollError = null;
        await afterOperationChange();
      } catch (error) {
        if (error.name === 'AbortError') return;
        state.pollError = error.code || 'REOBSERVATION_STATUS_UNAVAILABLE';
        notify();
      }
    }

    async function afterOperationChange() {
      notify();
      const tasks = state.operation?.tasks;
      if (Array.isArray(tasks) && tasks.length > 0 && tasks.every(isTerminal)) {
        await onTerminal?.();
        return;
      }
      schedulePoll();
    }

    function schedulePoll() {
      if (state.timerId) timer.clearTimeout(state.timerId);
      const tasks = state.operation?.tasks;
      if (!state.statusUrl || !Array.isArray(tasks) || tasks.length === 0 || tasks.every(isTerminal)) return;
      state.timerId = timer.setTimeout(() => { void poll(); }, 2500);
    }

    function setReadError(error) {
      state.readError = error || null;
      notify();
    }

    return { availability, request, reset, setReadError, snapshot, isTerminal };
  }

  async function postJsonSameOrigin(url, signal) {
    const response = await fetch(url, {
      method: 'POST',
      headers: { Accept: 'application/json' },
      credentials: 'same-origin',
      signal,
    });
    let payload = null;
    try { payload = await response.json(); } catch (_) { payload = null; }
    if (!response.ok) {
      const error = new Error(payload?.code || `http_${response.status}`);
      error.code = payload?.code || 'reobservation_unavailable';
      error.status = response.status;
      throw error;
    }
    return payload;
  }

  function isTerminal(task) {
    return TERMINAL_TASK_STATES.has(task?.state);
  }

  function renderReobservationSection(item, channel, controller, ui) {
    const { node, tech, section, factGrid, stateTag, sourceIncompleteBlock, laneLabels } = ui;
    const view = controller.snapshot();
    const access = controller.availability(item, channel);
    const wrapper = section('立即复观测');
    wrapper.dataset.evReobservation = 'true';
    const notice = node('div', 'ev-inline-state');
    notice.dataset.tone = 'warning';
    notice.append(
      node('strong', null, '按既有授权链路发起一次标准详情复观测'),
      node('p', null, '范围固定为详情、评论和回复：评论窗口最多 30 条；不会新建媒体槽位、下载媒体字节或启动 OCR / ASR。执行仍需已有监控目标关联、有效深度归档授权，以及可认领租约的本机 Browser Producer。'),
      tech(channel?.requires || '授权条件未知'),
    );
    wrapper.append(notice);
    if (!access.actionable) {
      const detail = !access.supported
        ? '该作品的平台当前不支持复观测；页面没有创建可调用入口。'
        : access.eligible
          ? '复观测入口未通过同源路径校验；页面不会猜测后端路由。'
          : '该作品当前尚未具备监控目标关联的有效深度归档授权；页面不会提供不可执行的操作。';
      wrapper.append(sourceIncompleteBlock(detail), tech(access.reason));
      return wrapper;
    }
    const actions = node('div', 'ev-channel-actions');
    const leaseExists = Boolean(view.operation?.leaseRef);
    const button = node('button', 'ev-button ev-button--primary', leaseExists ? '复观测已请求' : '立即复观测');
    button.type = 'button';
    button.disabled = leaseExists;
    button.addEventListener('click', () => { void controller.request(access.actionUrl); });
    actions.append(button, tech(channel?.mediaPolicy || '媒体策略未知'));
    wrapper.append(actions);
    if (view.error) wrapper.append(failureBlock('未能创建复观测请求', '没有把此结果写成已排队、已执行或已接纳；请依据返回的授权或运行状态处理。', view.error, ui));
    if (view.readError) wrapper.append(failureBlock('复观测状态已更新，但作品当前事实未重新读取', '保留已知 Task/Attempt/Package/Receipt；没有把 Inspector 旧快照写成新事实。', view.readError, ui));
    if (view.pollError) wrapper.append(failureBlock('复观测状态读取失败', '已保留上一次已知状态；页面没有把未知状态写成完成。', view.pollError, ui));
    if (view.operation) wrapper.append(reobservationOperationBlock(view.operation, ui));
    return wrapper;
  }

  function failureBlock(title, detail, code, { node, tech }) {
    const failure = node('div', 'ev-inline-state');
    failure.dataset.tone = 'danger';
    failure.append(node('strong', null, title), node('p', null, detail), tech(code));
    return failure;
  }

  function reobservationOperationBlock(operation, { node, tech, factGrid, stateTag, sourceIncompleteBlock, laneLabels }) {
    const block = node('div', 'ev-reobservation-operation');
    const leaseState = operation.leaseState || operation.execution || 'NOT_STARTED';
    const heading = node('div', 'ev-channel-title');
    heading.append(node('strong', null, '本次复观测执行状态'), stateTag(leaseState));
    block.append(heading, factGrid([
      ['准入结果', admissionCopy(operation.admission), operation.admission || null],
      ['准入决定', operation.decisionRef || '当前未知'],
      ['准入说明', operation.admissionReason || '状态读取未返回新的准入说明'],
      ['请求', operation.requestRef || '当前未知'],
      ['工单', operation.workOrderRef || '尚未形成'],
      ['租约', operation.leaseRef || '尚未形成'],
      ['到期时间', operation.expiresAt || '当前未知'],
      ['媒体策略', operation.media?.state === 'NOT_REQUESTED' ? '未请求新媒体，复用既有资产' : '当前未知', operation.media?.reason || 'NOT REQUESTED'],
    ]));
    const tasks = Array.isArray(operation.tasks) ? operation.tasks : [];
    if (tasks.length === 0) {
      block.append(sourceIncompleteBlock('准入已形成记录，但当前没有可读取的执行 lane；这不是平台已读取或执行成功。'));
      return block;
    }
    const list = node('div', 'ev-reobservation-tasks');
    tasks.forEach((task) => {
      const row = node('article', 'ev-reobservation-task');
      const header = node('div', 'ev-slot-head');
      header.append(node('strong', null, laneLabels[task.capability] || task.capability || '通道当前未知'), stateTag(task.state || 'UNKNOWN'));
      row.append(header, factGrid([
        ['任务', task.taskId || '当前未知'],
        ['评论上限', task.commentLimit === 'not_requested' ? '本详情 lane 不单独请求评论' : (task.commentLimit ?? '当前未知')],
        ['媒体', task.acquireMedia === 'not_requested' ? '未请求' : (task.acquireMedia ?? '当前未知')],
        ['认领时间', task.claimedAt || '尚未认领'],
        ['尝试', task.attemptId || '尚未开始'],
        ['包 / 回执', task.packageRef || task.receiptRef ? `${task.packageRef || '包未知'} / ${task.receiptRef || '回执未见'}` : '尚未形成', 'PACKAGE / RECEIPT'],
      ]));
      list.append(row);
    });
    block.append(list);
    return block;
  }

  function admissionCopy(admission) {
    const labels = {
      ADMITTED: '已按关联授权准入', MERGE: '已合并到覆盖当前作品的在途工作', REUSE: '既有材料已满足请求',
      DEFERRED: '请求已记录，等待可用资源', REFUSED: '请求已记录，但不在当前可执行范围',
      DECISION_REQUIRED: '请求已记录，尚需决定',
    };
    return labels[admission] || '准入结论当前未知';
  }

  function currentMetricFact(label, metric, code) {
    const current = metric?.state === 'KNOWN' && metric.current && typeof metric.current === 'object' ? metric.current : null;
    if (!current) return [label, '当前未知', 'UNKNOWN'];
    const previous = metric.previous && typeof metric.previous === 'object' && Number.isFinite(Number(metric.previous.value)) ? `；前值 ${Number(metric.previous.value).toLocaleString('zh-CN')}` : '；无可比较前值';
    const delta = metric.deltaState === 'KNOWN' && Number.isFinite(Number(metric.delta)) ? `；变化 ${Number(metric.delta) > 0 ? '+' : ''}${Number(metric.delta).toLocaleString('zh-CN')}` : '；变化未知';
    return [label, `${Number(current.value).toLocaleString('zh-CN')}${current.observedAt ? `（${current.observedAt}）` : ''}${previous}${delta}`, code];
  }

  function detailCurrentFact(label, field, code) {
    if (!field || typeof field !== 'object' || field.state === 'UNKNOWN') return [label, '当前未知', 'UNKNOWN'];
    const source = field.source && typeof field.source === 'object' ? field.source : {};
    const sourceRef = source.packageRef || source.materialRef || '来源引用未返回';
    const value = field.state === 'SOURCE_TEXT_ONLY' ? `仅有来源文本：${field.sourceText || '当前未知'}；${sourceRef}` : `${field.value || '已知'}；${sourceRef}`;
    return [label, value, field.state || code];
  }

  function engagementTimeline(inspector, { node, tech, section, sourceIncompleteBlock, knownMetric }) {
    const timeline = section('互动数据观察时间线');
    const observations = Array.isArray(inspector.engagementTimeline) ? inspector.engagementTimeline : [];
    if (observations.length === 0) {
      timeline.append(sourceIncompleteBlock('当前详情没有互动数据观察时点；不把未知写成 0。'));
      return timeline;
    }
    /* This is a timeline of change, so consecutive observations that read identically are one
     * fact observed more than once, not several facts. Listing each one separately made two
     * reads that both found nothing look like a rendering repeat and hid the thing actually
     * worth knowing: it was attempted twice and returned nothing both times. Each run keeps its
     * first and last observation time, so no observation is dropped from the record. */
    const runs = [];
    observations.forEach((observation) => {
      const metrics = [
        knownMetric(observation.likeCount, observation.likeCountState, '赞'), knownMetric(observation.commentCount, observation.commentCountState, '评'),
        knownMetric(observation.collectCount, observation.collectCountState, '藏'), knownMetric(observation.shareCount, observation.shareCountState, '转'),
      ].filter(Boolean);
      const copy = metrics.length ? metrics.join(' · ') : '本时点互动字段均为当前未知';
      const lane = (observation.sourceLane || 'source').toUpperCase();
      const observedAt = observation.observedAt || '观察时间当前未知';
      const previous = runs[runs.length - 1];
      if (previous && previous.copy === copy && previous.lane === lane) {
        previous.lastObservedAt = observedAt;
        previous.count += 1;
        return;
      }
      runs.push({ copy, lane, firstObservedAt: observedAt, lastObservedAt: observedAt, count: 1 });
    });
    const list = node('div', 'ev-slot-list');
    runs.forEach((run) => {
      const entry = node('article', 'ev-derivative');
      const when = run.count === 1
        ? run.firstObservedAt
        : `${run.firstObservedAt} — ${run.lastObservedAt}`;
      entry.append(node('strong', null, when));
      if (run.count > 1) entry.append(node('span', 'ev-inline-note', `连续 ${run.count} 次观察结果相同`));
      entry.append(node('p', null, run.copy), tech(run.lane));
      list.append(entry);
    });
    timeline.append(list);
    return timeline;
  }

  function coverageHistorySection(inspector, { node, section, factGrid, stateTag, sourceIncompleteBlock }) {
    const history = section('评论 / 回复复观测历史');
    history.append(coverageHistoryBlock('评论', inspector.commentsCoverageHistory, { node, factGrid, stateTag, sourceIncompleteBlock }), coverageHistoryBlock('回复', inspector.repliesCoverageHistory, { node, factGrid, stateTag, sourceIncompleteBlock }));
    return history;
  }

  function coverageHistoryBlock(label, history, { node, factGrid, stateTag, sourceIncompleteBlock }) {
    const wrapper = node('section', 'ev-coverage-history');
    wrapper.append(node('h4', null, `${label}：每条为一个独立 Package / Receipt，不做跨尝试合计`));
    const entries = Array.isArray(history) ? history : [];
    if (entries.length === 0) {
      wrapper.append(sourceIncompleteBlock(`当前没有 ${label} 覆盖历史；这不表示该通道未曾执行。`));
      return wrapper;
    }
    const groups = [
      ['详情窗口（最多 30 条）', entries.filter((entry) => entry?.collectionScope === 'detail_window')],
      ['独立全量或其他范围', entries.filter((entry) => entry?.collectionScope && entry.collectionScope !== 'detail_window')],
      ['范围未完整表达', entries.filter((entry) => !entry?.collectionScope)],
    ];
    groups.forEach(([heading, group]) => {
      if (!group.length) return;
      const groupElement = node('div', 'ev-coverage-history-group');
      groupElement.append(node('strong', null, heading));
      group.forEach((entry) => groupElement.append(coverageHistoryEntry(entry, { node, factGrid, stateTag })));
      wrapper.append(groupElement);
    });
    return wrapper;
  }

  function coverageHistoryEntry(entry, { node, factGrid, stateTag }) {
    const article = node('article', 'ev-derivative ev-history-item');
    const header = node('div', 'ev-slot-head');
    header.append(node('strong', null, entry.observedAt || '观察时间当前未知'), stateTag(entry.state || 'UNKNOWN'));
    article.append(header, factGrid([
      ['范围', entry.collectionScope || '当前未知'], ['请求上限', entry.requestedLimit ?? '当前未知'],
      ['页面评论数', entry.pageCommentCount ?? '当前未知'], ['预期 / 去重保留', `${entry.expectedCount ?? '当前未知'} / ${entry.uniqueCollectedCount ?? '当前未知'}`, entry.collectionState || 'UNKNOWN'],
      ['采集 / 保留 / 失败', `${entry.producerAcquired ?? '当前未知'} / ${entry.retained ?? '当前未知'} / ${entry.failed ?? '当前未知'}`],
      ['Task / Attempt', `${entry.taskRef || '当前未知'} / ${entry.attemptRef || '当前未知'}`, 'TASK / ATTEMPT'], ['Package / Receipt', `${entry.packageRef || '当前未知'} / ${entry.receiptRef || '未见回执'}`, 'PACKAGE / RECEIPT'],
    ]));
    return article;
  }

  function createCommentResearchController({ readJson, onChange }) {
    const state = {
      url: null, cursor: null, items: [], controller: null, loading: false, error: null,
      loaded: false, total: null, returned: null, truncated: null,
    };

    function snapshot() {
      return {
        url: state.url, cursor: state.cursor, items: state.items, loading: state.loading, error: state.error,
        loaded: state.loaded, total: state.total, returned: state.returned, truncated: state.truncated,
      };
    }

    function setSource(url) {
      if (state.url === url) return;
      reset();
      state.url = url || null;
    }

    function reset() {
      state.controller?.abort();
      state.controller = null;
      state.cursor = null;
      state.items = [];
      state.loading = false;
      state.error = null;
      state.loaded = false;
      state.total = null;
      state.returned = null;
      state.truncated = null;
    }

    async function load(append = false) {
      if (!state.url) return;
      state.controller?.abort();
      state.controller = new AbortController();
      state.loading = true;
      state.error = null;
      onChange?.(snapshot());
      const url = new URL(state.url, window.location.origin);
      if (append && state.cursor) url.searchParams.set('cursor', state.cursor);
      try {
        const payload = await readJson(`${url.pathname}${url.search}`, state.controller.signal);
        if (!payload || !Array.isArray(payload.items)) throw new Error('invalid_comment_channel_response');
        state.items = append ? state.items.concat(payload.items) : payload.items;
        state.cursor = payload.nextCursor || null;
        state.loaded = true;
        state.total = payload.total ?? null;
        state.returned = payload.returned ?? null;
        state.truncated = payload.truncated === true;
        state.loading = false;
        onChange?.(snapshot(), payload);
      } catch (error) {
        if (error.name === 'AbortError') return;
        state.loading = false;
        state.error = error.code || 'COMMENT_CHANNEL_UNAVAILABLE';
        onChange?.(snapshot());
      }
    }

    return { load, reset, setSource, snapshot };
  }

  function renderDiscussion(item, inspector, commentChannel, controller, panel, ui) {
    const { apiRoot, sameOriginPath, node, tech, section, factGrid, stateMeta, laneSummary, receiptBlock } = ui;
    panel.replaceChildren();
    const commentsCoverage = section('评论与回复覆盖');
    commentsCoverage.append(factGrid([
      ['评论状态', stateMeta(laneSummary(item, 'comments')?.state || 'UNKNOWN')[0], laneSummary(item, 'comments')?.state || 'UNKNOWN'],
      ['评论数量', inspector.commentsCoverage?.countState === 'KNOWN' ? inspector.commentsCoverage?.count : '当前未知', inspector.commentsCoverage?.countState || 'UNKNOWN'],
      ['回复状态', stateMeta(laneSummary(item, 'replies')?.state || 'UNKNOWN')[0], laneSummary(item, 'replies')?.state || 'UNKNOWN'],
      ['回复数量', inspector.repliesCoverage?.countState === 'KNOWN' ? inspector.repliesCoverage?.count : '当前未知', inspector.repliesCoverage?.countState || 'UNKNOWN'],
    ]));
    panel.append(commentsCoverage, coverageHistorySection(inspector, ui));

    const channelUrl = sameOriginPath(commentChannel?.url, [`${apiRoot}/`]);
    controller.setSource(channelUrl);
    panel.append(receiptBlock('评论研究通道', commentChannel?.receipt || inspector.commentsReceipt, channelUrl));
    const access = section('本机授权评论研究');
    const accessState = node('div', 'ev-inline-state');
    accessState.dataset.tone = 'restricted';
    accessState.append(node('strong', null, '原文只在本机授权详情中按页读取'), node('p', null, '作者只显示匿名上下文；页面不会渲染平台用户标识，也不会把评论原文放回普通列表。'));
    const controls = node('div', 'ev-channel-actions');
    const snapshot = controller.snapshot();
    const load = node('button', 'ev-button ev-button--secondary', snapshot.loading ? '正在读取评论' : '读取评论原文');
    load.type = 'button';
    load.disabled = !channelUrl || snapshot.loading;
    load.addEventListener('click', () => { void controller.load(false); });
    controls.append(load);
    if (!channelUrl) controls.append(tech('SOURCE INCOMPLETE'));
    access.append(accessState, controls, renderComments(snapshot, controller, ui));
    panel.append(access);
  }

  function renderComments(snapshot, controller, { node, tech, addTextWithTech }) {
    const list = node('div', 'ev-comment-list');
    if (snapshot.error) {
      const failure = node('div', 'ev-inline-state');
      failure.dataset.tone = 'danger';
      failure.append(node('strong', null, '评论研究通道读取失败'), tech(snapshot.error));
      list.append(failure);
      return list;
    }
    if (snapshot.items.length === 0) {
      if (snapshot.loaded) {
        const empty = node('div', 'ev-inline-state');
        empty.append(node('strong', null, '当前通道没有返回评论材料'), node('p', null, '这只描述本次授权读取，不表示平台评论为 0。'), node('span', 'ev-inline-note', '本次未返回材料'));
        list.append(empty);
      }
      return list;
    }
    snapshot.items.forEach((comment) => {
      const article = node('article', 'ev-comment');
      const header = node('div', 'ev-comment-head');
      header.append(node('strong', null, comment.relation === 'REPLY' ? '匿名回复' : '匿名评论'), tech(comment.sourceRef || 'SOURCE INCOMPLETE'));
      const body = node('p', null, comment.bodyState === 'KNOWN' && comment.body !== null ? comment.body : '评论正文当前未知');
      const meta = node('div', 'ev-comment-meta');
      addTextWithTech(meta, comment.bodyTruncated ? '本条正文已在读取边界截断' : '本条正文未在通道内截断', null);
      article.append(header, body, meta);
      list.append(article);
    });
    const receipt = node('div', 'ev-comment-receipt');
    addTextWithTech(receipt, `总数 ${snapshot.total ?? '未知'} · 当前 ${snapshot.items.length} · 本次 ${snapshot.returned ?? '未知'}${snapshot.truncated ? '；本页已截断，可继续读取' : snapshot.cursor ? '；可继续读取' : ''}`);
    list.append(receipt);
    if (snapshot.cursor) {
      const next = node('button', 'ev-button ev-button--secondary', '继续读取评论');
      next.type = 'button';
      next.addEventListener('click', () => { void controller.load(true); });
      list.append(next);
    }
    return list;
  }

  window.LingganEvidenceObservation = {
    createController,
    renderReobservationSection,
    currentMetricFact,
    detailCurrentFact,
    engagementTimeline,
    coverageHistorySection,
    createCommentResearchController,
    renderDiscussion,
  };
})();
