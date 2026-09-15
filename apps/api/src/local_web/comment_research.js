(() => {
  'use strict';

  const api = '/api/local/comment-research';
  const $ = (selector) => document.querySelector(selector);
  const result = $('#research-result');
  const status = $('#research-status');
  const dialog = $('#research-settings-dialog');
  const runDialog = $('#research-run-dialog');
  const form = $('#research-settings-form');
  const views = new Set(['overview', 'voices', 'problems', 'changes', 'runs']);
  const pagedViews = new Set(['voices', 'problems', 'runs']);
  const problemViews = new Set(['all', 'confirmed', 'deferred']);
  const query = new URLSearchParams(location.search);
  const initialPage = Math.max(1, Number.parseInt(query.get('page') || '1', 10) || 1);
  const state = { view: query.get('view') || 'overview', page: initialPage, problemView: query.get('problemView') || 'all', problemItems: [], setup: null, preview: null };
  if (!views.has(state.view)) state.view = 'overview';
  if (!problemViews.has(state.problemView)) state.problemView = 'all';
  if (!pagedViews.has(state.view)) state.page = 1;

  const errorText = {
    comment_research_result_unavailable: '还没有可读取的已发布研究结果。先保存策略并开始一轮研究；后台完成提取、向量归并和结果冻结后，页面会显示新的版本。',
    comment_research_v1_schema_missing: '评论研究 V1 的数据库结构尚未就绪。',
    comment_research_unavailable: '评论研究暂时不可用，请检查本机数据服务。',
    comment_research_v1_unavailable: '本次研究结果暂时无法读取；上一次已显示的结果不会被伪造成新结果。',
    research_policy_missing: '请先保存研究策略。',
    research_policy_input_contract_stale: '评论研究的输入合同已更新。请重新保存研究策略，再查看并确认新的冻结范围；此操作不会发送评论。',
    embedding_not_ready: '请先在“模型与向量设置”中完成向量模型测试并启用；系统不会创建一个必然无法归并和发布的研究运行。',
    research_model_not_ready: '请先在“模型与 AI 设置”中完成当前研究模型的 V1 语义测试；系统不会向未通过结构化输出检查的模型发送评论。',
    no_eligible_research_comments: '当前没有可进入研究的普通用户评论。作品作者回复、身份未知或不可读评论不会被混入。',
    invalid_research_request: '研究设置或运行范围不符合要求。',
  };

  const escape = (value) => String(value ?? '').replace(/[&<>"']/g, character => ({ '&':'&amp;', '<':'&lt;', '>':'&gt;', '"':'&quot;', "'":'&#39;' })[character]);
  const date = value => value ? new Intl.DateTimeFormat('zh-CN', { dateStyle:'medium', timeStyle:'short', timeZone:'Asia/Shanghai' }).format(new Date(value)) : '未知';
  const count = value => Number(value ?? 0).toLocaleString('zh-CN');
  const pct = value => `${(Number(value ?? 0) * 100).toFixed(1)}%`;
  const empty = (message, heading = '暂时没有可显示的研究结果') => `<section class="cr-v1-empty"><h2>${escape(heading)}</h2><p>${escape(message)}</p></section>`;
  const table = (head, rows) => `<div class="cr-v1-table-wrap"><table><thead><tr>${head.map(item => `<th scope="col">${escape(item)}</th>`).join('')}</tr></thead><tbody>${rows.join('')}</tbody></table></div>`;

  function pageLimit() {
    return state.view === 'problems' ? 10 : 20;
  }

  function pagination(page, noun, placement = 'bottom') {
    const total = Number(page.total ?? 0);
    const limit = Number(page.limit ?? pageLimit());
    const offset = Number(page.offset ?? 0);
    if (!total) return '';
    const current = Math.floor(offset / limit) + 1;
    const pages = Math.max(1, Math.ceil(total / limit));
    const from = offset + 1;
    const to = Math.min(offset + limit, total);
    const previous = Math.max(0, offset - limit);
    const next = offset + limit;
    const placementLabel = placement === 'top' ? '表格上方' : '表格下方';
    return '<nav class="cr-v1-pagination cr-v1-pagination--' + placement + '" aria-label="' + escape(noun) + '分页（' + placementLabel + '）"><span>显示 ' + count(from) + '–' + count(to) + '，共 ' + count(total) + ' 条 · 第 ' + count(current) + ' / ' + count(pages) + ' 页</span><div><button type="button" data-page-offset="' + String(previous) + '"' + (offset === 0 ? ' disabled' : '') + '>上一页</button><button type="button" data-page-offset="' + String(next) + '"' + (next >= total ? ' disabled' : '') + '>下一页</button></div></nav>';
  }

  async function request(path, options = {}) {
    const response = await fetch(path, { cache:'no-store', headers: options.body ? {'Content-Type':'application/json'} : undefined, ...options });
    const payload = await response.json().catch(() => ({}));
    if (!response.ok) {
      const error = new Error(errorText[payload.error] || '请求没有完成，请保留当前输入后重试。');
      error.code = payload.error;
      throw error;
    }
    return payload;
  }

  function setStatus(message, kind = '') {
    status.textContent = message;
    status.dataset.kind = kind;
  }

  function updateUrl() {
    const query = new URLSearchParams(location.search);
    query.set('view', state.view);
    if (state.view === 'problems') query.set('problemView', state.problemView);
    else query.delete('problemView');
    if (pagedViews.has(state.view) && state.page > 1) query.set('page', String(state.page));
    else query.delete('page');
    history.replaceState(null, '', location.pathname + '?' + query);
  }

  function renderTabs() {
    document.querySelectorAll('[data-view]').forEach(button => {
      button.setAttribute('aria-current', button.dataset.view === state.view ? 'page' : 'false');
    });
  }

  function setupSummary() {
    const { policy, defaultConfig, embedding, worker } = state.setup || {};
    if (!defaultConfig) return '尚未在模型工作区选择研究模型。保存策略和开始研究都不会发送评论。';
    if (!defaultConfig.connectionEnabled) return '当前研究模型连接已停用。保存策略和开始研究都不会发送评论。';
    if (!researchModelReady()) return '当前研究模型尚未通过评论研究 V1 的结构化输出测试。保存策略和开始研究都不会发送评论。';
    const embeddingText = embeddingReady()
      ? `向量归并已就绪（${embedding.dimensions} 维）。`
      : '向量归并尚未就绪；开始研究已锁定，避免产生无法归并和发布的无效运行。请先在模型与向量设置中完成测试并启用。';
    const policyText = policy ? `当前策略已保存：每轮最多 ${count(policy.sourceLimit)} 条评论。` : '尚未保存研究策略。';
    const workerText = worker?.state === 'error'
      ? '后台执行器上次报告异常；已冻结项目会保留真实状态与安全失败原因。'
      : !worker?.lastSeenAt
        ? '尚未记录后台执行器心跳；确认后只会冻结范围，模型调用会等待 Worker 启动。'
        : `后台执行器上次心跳：${date(worker.lastSeenAt)}（${worker.state === 'running' ? '正在推进' : '空闲'}）。连续自动排程关闭。`;
    return `${policyText}${embeddingText}${workerText}`;
  }

  function embeddingReady() {
    return Boolean(state.setup?.embedding?.enabled && state.setup?.embedding?.qualified && state.setup?.embedding?.connectionEnabled);
  }

  function researchModelReady() {
    return Boolean(state.setup?.defaultConfig?.connectionEnabled && state.setup?.defaultConfig?.semanticReady);
  }

  function renderRunAvailability() {
    const start = $('#start-run');
    start.disabled = false;
    start.title = '先查看服务端自动计算的本轮范围；确认时仍会复核模型与向量资格。';
  }

  async function loadSetup() {
    state.setup = await request(`${api}/setup`);
    renderRunAvailability();
    setStatus(setupSummary(), researchModelReady() && embeddingReady() ? 'ready' : 'warning');
  }

  function statisticsResult(data) {
    return data.statisticsResult || data.result || null;
  }

  function resultWindow(data) {
    const input = statisticsResult(data)?.inputCounts || {};
    const currentComments = Number(input.currentCommentDenominator ?? 0);
    const currentWorks = Number(input.currentWorkDenominator ?? 0);
    const baselineComments = Number(input.baselineCommentDenominator ?? 0);
    const baselineWorks = Number(input.baselineWorkDenominator ?? 0);
    if (currentComments > 0) return { kind:'current', comments:currentComments, works:currentWorks };
    if (baselineComments > 0) return { kind:'baseline', comments:baselineComments, works:baselineWorks };
    return { kind:'none', comments:0, works:0 };
  }

  function share(countValue, denominator) {
    return denominator > 0 ? Number(countValue ?? 0) / denominator : 0;
  }

  function resultMeta(data) {
    const statistics = statisticsResult(data);
    if (!statistics) return '<p class="cr-v1-result-meta">尚无可用于占比、排行或变化判断的统计研究版本；下方只显示累计已确认事实。</p>';
    const input = statistics.inputCounts || {};
    const selected = Number(input.selectedCommentCount ?? input.analyzedCommentCount ?? 0);
    const included = Number(input.includedCommentCount ?? input.analyzedCommentCount ?? 0);
    const excluded = Number(input.excludedTerminalCommentCount ?? 0);
    const organization = input.problemOrganizationCoverage || {};
    const partial = input.publicationCoverage === 'partial';
    const window = resultWindow(data);
    const coverage = partial
      ? '部分研究版本 · 本版纳入 ' + count(included) + ' / ' + count(selected) + ' 条冻结原声，' + count(excluded) + ' 条未纳入；问题归并 ' + count(organization.numerator) + ' / ' + count(organization.denominator) + ' 条 Atom。'
      : '完整研究版本 · 本版纳入 ' + count(included) + ' / ' + count(selected) + ' 条冻结原声。';
    const comparison = statistics.comparison || {};
    const windowText = window.kind === 'baseline'
      ? '本版样本位于基线窗口 ' + escape(date(comparison.baseline?.start)) + ' 至 ' + escape(date(comparison.baseline?.end)) + '；当前窗口没有可比样本。'
      : '当前窗口 ' + escape(date(comparison.current?.start)) + ' 至 ' + escape(date(comparison.current?.end)) + '。';
    return '<p class="cr-v1-result-meta">统计版本发布于 ' + escape(date(statistics.publishedAt)) + ' · ' + windowText + ' · ' + escape(coverage) + (partial ? ' <a href="?view=runs">查看运行记录</a>' : '') + '</p>';
  }

  function cumulativeMeta(data) {
    const cumulative = data.cumulative || {};
    const problems = Number(cumulative.problemCount ?? 0);
    const atoms = Number(cumulative.confirmedAtomCount ?? 0);
    const comments = Number(cumulative.confirmedCommentCount ?? 0);
    const works = Number(cumulative.confirmedWorkCount ?? 0);
    const last = cumulative.lastConfirmedAt ? `最近确认于 ${date(cumulative.lastConfirmedAt)}。` : '尚未确认任何稳定用户问题。';
    return `<p class="cr-v1-result-meta">累计已确认：${count(problems)} 个用户问题 · ${count(atoms)} 条归并证据 · ${count(comments)} 条评论 · ${count(works)} 篇作品。${escape(last)}</p>`;
  }

  function renderOverview(data) {
    const items = data.currentProblems || [];
    const heading = '累计已确认的用户问题';
    const explanation = '每条证据都已经通过问题归并接纳合同，因此会立即累计到这里。本轮尚未完成的信号不会被删除，但也不会被写成占比、排行或趋势。';
    const columns = ['用户问题', '累计评论证据', '累计作品', '归并 Atom', '最近确认'];
    const rows = items.map(item => {
      return '<tr><td><strong>' + escape(item.name) + '</strong><p>' + escape(item.meaning) + '</p></td><td>' + count(item.confirmedCommentCount) + '</td><td>' + count(item.confirmedWorkCount) + '</td><td>' + count(item.confirmedAtomCount) + '</td><td>' + escape(date(item.lastConfirmedAt)) + '</td></tr>';
    });
    const overviewDisclosure = items.length
      ? '<p class="cr-v1-overview-disclosure">概览仅展示 ' + count(items.length) + ' 个代表问题。<a href="?view=problems">查看全部用户问题（可分页浏览）</a></p>'
      : '';
    result.innerHTML = '<section class="cr-v1-intro"><h2>' + heading + '</h2><p>' + explanation + '</p>' + cumulativeMeta(data) + resultMeta(data) + '</section>' +
      (items.length ? table(columns, rows) + overviewDisclosure : empty('当前还没有通过归并接纳合同的用户问题；这不表示没有评论或没有正在处理的研究信号。'));
  }

  function renderVoices(data) {
    const page = data.page || {items:[], total:0};
    result.innerHTML = `<section class="cr-v1-intro"><h2>用户原声</h2><p>这里呈现当前可读、已通过身份与清洗过滤的普通用户评论。原文是证据；研究正文是独立派生，不会改写原文。</p><p class="cr-v1-result-meta">当前共 ${count(page.total)} 条可读用户原声。没有对应研究运行的评论会如实显示为“尚未进入研究”。</p></section>` +
      (page.items.length ? pagination(page, '用户原声', 'top') + table(['评论原文', '研究正文', '作品与观察时间', '最新研究状态'], page.items.map(item => `<tr><td><blockquote>${escape(item.commentText || '正文尚未取得')}</blockquote></td><td><p>${escape(item.researchText || '未形成研究正文')}</p><span class="cr-v1-badge">普通用户</span></td><td><strong>${escape(item.workTitle || '作品标题未知')}</strong><p>${escape(date(item.observedAt))}</p></td><td>${escape(voiceResearchStatusLabel(item.researchStatus))}<p>${escape(voiceResearchDetail(item.researchStatus, item.researchFailureCode))}</p></td></tr>`)) + pagination(page, '用户原声') : empty('当前没有可读的普通用户原声。作者回复、身份未知及已被清洗剔除的内容不会混入这里。', '暂时没有可显示的用户原声'));
  }

  function voiceResearchStatusLabel(state) {
    return ({
      unresearched:'尚未进入研究', pending:'等待研究', running:'正在研究', retryable:'等待重试',
      succeeded:'已提取研究信号', no_signal:'未提取到研究信号', incompatible:'无法按当前合同研究',
      unrecoverable:'无法继续研究', model_failed:'模型研究失败', restricted:'已限制', cancelled:'本轮已取消'
    })[state] || '研究状态未知';
  }

  function voiceResearchDetail(state, failureCode) {
    if (failureCode) return itemFailureLabel(failureCode);
    return ({
      unresearched:'尚未创建该评论的研究任务', pending:'已进入研究队列', running:'正在执行本轮研究',
      retryable:'上次未完成，正在等待重试', succeeded:'已完成本轮研究',
      no_signal:'本轮未提取到可归并的研究信号', incompatible:'当前输入无法按研究合同处理',
      unrecoverable:'当前研究无法继续处理', model_failed:'模型研究未完成',
      restricted:'该评论已被限制用于研究', cancelled:'本轮研究已取消'
    })[state] || '尚未取得可显示的研究状态说明';
  }

  function renderProblems(data) {
    const page = data.page || {items:[], total:0};
    state.problemItems = page.items || [];
    const facets = data.facets || {};
    const filters = [
      ['all', '全部', facets.all],
      ['confirmed', '已形成问题', facets.confirmed],
      ['deferred', '待归并信号', facets.deferred],
    ].map(([view, label, total]) => '<button type="button" class="cr-v1-filter" data-problem-view="' + view + '" aria-pressed="' + String(state.problemView === view) + '">' + escape(label) + ' <span>' + count(total) + '</span></button>').join('');
    const columns = ['状态与结论', '问题定义或用户原声', '证据/候选', '最近更新', ''];
    const rows = page.items.map(item => {
      const index = state.problemItems.indexOf(item);
      if (item.itemKind === 'deferred') {
        const candidateCount = Array.isArray(item.candidateSnapshot) ? item.candidateSnapshot.length : 0;
        return '<tr><td><span class="cr-v1-signal">' + escape(deferredDecisionLabel(item.decisionKind)) + '</span><p>尚未建立 membership</p></td><td><p class="cr-v1-note">原声已留存在受限证据区；本页不展示逐字评论。</p><p>' + escape(item.proposition || '未形成归一描述') + '</p></td><td>' + count(candidateCount) + ' 个冻结候选<p>' + escape(recheckLabel(item.recheckConditions)) + '</p></td><td>' + escape(date(item.decision?.updatedAt || item.updatedAt)) + '</td><td><button type="button" data-problem-detail="' + index + '">查看依据</button></td></tr>';
      }
      return '<tr><td><span class="cr-v1-badge">已形成问题</span><p>已建立 membership</p></td><td><strong>' + escape(item.name) + '</strong><p>' + escape(item.meaning) + '</p></td><td>' + count(item.confirmedCommentCount) + ' 条评论 · ' + count(item.confirmedWorkCount) + ' 篇作品<p>' + count(item.confirmedAtomCount) + ' 个归并 Atom</p></td><td>' + escape(date(item.lastConfirmedAt)) + '</td><td><button type="button" data-problem-detail="' + index + '">查看定义</button></td></tr>';
    });
    const explanation = '稳定 Problem 只由已建立 membership 的证据构成。待归并信号已完成当前判断、保留受限原声的出处与重评条件，但不会被写成 Problem、占比或趋势。';
    const emptyText = state.problemView === 'deferred'
      ? '当前没有待归并信号；这只表示当前读取范围中没有这种已完成处置，不代表评论或研究任务为零。'
      : state.problemView === 'confirmed'
        ? '当前没有已形成的稳定问题。尚未归并信号不会被删除或伪装成零。'
        : '当前没有可显示的稳定 Problem 或已完成待归并信号。未评估、执行中和失败记录仍保留在运行记录中。';
    result.innerHTML = '<section class="cr-v1-intro"><h2>用户问题与待归并信号</h2><p>' + explanation + '</p>' + cumulativeMeta(data) + resultMeta(data) + '</section><nav class="cr-v1-filter-bar" aria-label="用户问题状态筛选">' + filters + '</nav>' +
      (page.items.length ? pagination(page, '用户问题与待归并信号', 'top') + table(columns, rows) + pagination(page, '用户问题与待归并信号') : empty(emptyText));
  }

  function deferredDecisionLabel(decision) {
    return ({ deferred_novel:'等待独立同类证据', deferred_ambiguous:'等待消歧', deferred_context:'等待必要语境' })[decision] || '待归并信号';
  }

  function recheckLabel(conditions) {
    const labels = {
      independent_same_frame_signal:'出现独立同类信号', candidate_catalog_changed:'候选库变化', policy_scope_changed:'领域边界变化',
      candidate_definition_changed:'候选定义变化', material_context_added:'补充关键语境', atom_frame_changed:'Atom frame 修订', model_contract_repaired:'模型合同修复'
    };
    const values = Array.isArray(conditions) ? conditions.map(value => labels[value] || value) : [];
    return values.length ? '重评：' + values.join('；') : '暂无自动重评条件';
  }

  function frameFieldLabel(label, field) {
    if (!field) return '<li><strong>' + escape(label) + '</strong>：未知</li>';
    const value = field.value || '未知';
    const basis = field.basis === 'explicit' ? '原声明确' : field.basis === 'context_resolved' ? '上下文支持' : '未知';
    return '<li><strong>' + escape(label) + '</strong>：' + escape(value) + ' <span>（' + escape(basis) + '；' + escape((field.evidenceRefs || []).join('、') || '无引用') + '）</span></li>';
  }

  function candidateDetail(candidate, comparisons) {
    const definition = candidate.definition || {};
    const comparison = (comparisons || []).find(item => Number(item.candidateIndex) === Number(candidate.candidateIndex));
    const labels = { subject:'主体', goal:'目标', barrier:'障碍', context:'场景', materialContradiction:'实质矛盾' };
    const truth = { yes:'一致', no:'不一致', unknown:'未知' };
    const dimensions = comparison ? ['subject', 'goal', 'barrier', 'context', 'materialContradiction'].map(key => labels[key] + '：' + (truth[comparison[key]] || '未知')).join(' · ') : '本次没有可显示的逐维比较';
    return '<li><strong>候选 ' + escape(candidate.candidateIndex) + '：' + escape(definition.name || '未命名定义') + '</strong><p>' + escape(definition.definition || '定义快照不可读') + '</p><p>纳入：' + escape((definition.include || []).join('；') || '未记录') + '</p><p>排除：' + escape((definition.exclude || []).join('；') || '未记录') + '</p><p>' + escape(dimensions) + '</p></li>';
  }

  function renderProblemDetail(item) {
    const dialog = $('#problem-detail-dialog');
    if (item.itemKind !== 'deferred') {
      $('#problem-detail-title').textContent = item.name || '稳定用户问题';
      $('#problem-detail-body').innerHTML = '<p>这是已建立 membership 的稳定 Problem。累计证据：' + count(item.confirmedCommentCount) + ' 条评论、' + count(item.confirmedWorkCount) + ' 篇作品、' + count(item.confirmedAtomCount) + ' 个 Atom。</p><p>' + escape(item.meaning || '尚未取得定义说明') + '</p><p class="cr-v1-note">这里不会重新调用模型解释或修改归并关系。</p>';
    } else {
      const frame = item.problemFrame || {};
      const decision = item.decision || {};
      const candidates = item.candidateSnapshot || [];
      const comparisons = decision.comparisons || [];
      const history = item.executionHistory || [];
      $('#problem-detail-title').textContent = deferredDecisionLabel(item.decisionKind);
      $('#problem-detail-body').innerHTML = '<section><h3>受限原声与归一描述</h3><p class="cr-v1-note">逐字评论仅保留在受限证据区；本工作台不会读取或展示原文。</p><p>归一描述：' + escape(item.proposition || '未形成') + '</p></section>' +
        '<section><h3>问题结构与出处</h3><ul class="cr-v1-detail-list">' + frameFieldLabel('主体', frame.subject) + frameFieldLabel('目标/期待状态', frame.goal) + frameFieldLabel('障碍/未满足需要', frame.barrier) + frameFieldLabel('场景', frame.context) + '</ul><p>领域关系：' + escape(({ in_scope:'属于当前 ADHD 研究范围', out_of_scope:'不属于当前 ADHD 研究范围', uncertain:'当前无法确认研究范围' })[frame.scopeRelation] || '未知') + '</p></section>' +
        '<section><h3>候选比较</h3>' + (candidates.length ? '<ol class="cr-v1-detail-list">' + candidates.map(candidate => candidateDetail(candidate, comparisons)).join('') + '</ol>' : '<p>当时没有可比较的稳定 Problem 候选，因此没有发生单条新建。</p>') + '</section>' +
        '<section><h3>结论与再次判断条件</h3><p>' + escape(deferredDecisionLabel(item.decisionKind)) + '。' + escape(recheckLabel(item.recheckConditions)) + '</p></section>' +
        '<section><h3>执行历史</h3>' + (history.length ? '<ul class="cr-v1-detail-list">' + history.map(entry => '<li>' + escape(voiceResearchStatusLabel(entry.state)) + ' · 尝试 ' + count(entry.attempts) + ' 次' + (entry.failureCode ? ' · ' + escape(itemFailureLabel(entry.failureCode)) : '') + '</li>').join('') + '</ul>' : '<p>当前没有可显示的执行历史。</p>') + '</section>';
    }
    dialog.showModal();
  }

  function observationLabel(kind) {
    return ({ rising:'升温', falling:'降温', spreading:'扩散', newly_observed:'新出现' })[kind] || '未命名观察';
  }

  function reasonLabel(code) {
    return ({ insufficient_window_coverage:'两个完整窗口的样本覆盖不足', first_observed_in_comparable_history:'首次出现在本系统可比研究历史中', comment_share_increased:'评论占比上升', comment_share_decreased:'评论占比下降', work_coverage_increased:'作品覆盖率上升' })[code] || '已记录原因';
  }

  function runStateLabel(state) {
    return ({ queued:'等待处理', running:'正在研究', completed:'已完成', completed_with_failures:'部分完成', failed:'失败', cancelled:'已取消' })[state] || '状态未知';
  }

  function runFailureLabel(code) {
    return ({ runItems:'语义提取未完成', embedding:'向量候选未完成', problemResolution:'问题归并未完成' })[code] || '研究步骤未完成';
  }

  function itemFailureLabel(code) {
    return ({
      semantic_json_unparseable:'语义 JSON 未通过结构解析',
      semantic_contract_rejected:'语义字段或证据范围未通过合同校验',
      semantic_evidence_offset_unmappable:'证据 Unicode 位置无法映射回原评论',
      semantic_claim_lost:'该项在接纳语义结果前已失去处理租约',
      semantic_acceptance_storage_failed:'语义结果接纳记录失败',
      model_configuration_missing:'运行冻结时缺少研究模型配置',
      model_not_qualified:'研究模型未通过 V1 语义测试',
      model_disabled:'研究模型连接已停用',
      model_secret_unavailable:'研究模型凭据当前不可用',
      model_adapter_unavailable:'研究模型执行适配器当前不可用',
      model_database_unavailable:'模型调用账本暂时不可用',
      model_source_unavailable:'研究来源在处理时不可用',
      model_budget_exhausted:'研究预算已用尽',
      model_input_limit:'单条输入超出模型限制',
      embedding_not_qualified:'向量模型不可用',
      embedding_configuration_unavailable:'向量模型配置在执行中不可用',
      worker_interrupted:'后台执行中断，正在按次数限制恢复',
      context_insufficient_parent_unavailable:'该回复需要父评论语境，但当前没有可读父评论；未发送给模型，也不计为“没有研究信号”',
      context_input_contract_invalid:'该回复的冻结上下文不符合当前输入合同；未发送给模型',
      source_unavailable:'研究来源在处理时已不可读取',
      provider_timeout:'模型服务超时',
      provider_failed:'模型服务未返回可用结果'
    })[code] || '该项未完成研究处理；已保留安全失败类别';
  }

  function runItemStateLabel(state) {
    return ({
      pending:'等待执行', running:'正在执行', succeeded:'已提取研究信号', no_signal:'未提取到研究信号',
      retryable:'等待恢复', incompatible:'无法按当前合同处理', unrecoverable:'无法继续处理',
      model_failed:'模型处理失败', restricted:'已限制用于研究', cancelled:'本轮已取消'
    })[state] || '状态未知';
  }

  function previewTerminalStateLabel(state) {
    return ({
      incompatible:'当前合同不兼容', unrecoverable:'无法继续处理', model_failed:'模型重试已耗尽',
      restricted:'已限制用于研究', cancelled:'已取消'
    })[state] || '已被限制或终止';
  }

  function stageLabel(stage) {
    return ({ semantic_extraction:'语义提取', embedding:'向量候选', problem_resolution:'问题归并' })[stage] || '研究调用';
  }

  function countSummary(values, label, formatter = value => value) {
    return Object.entries(values || {})
      .filter(([, value]) => Number(value) > 0)
      .map(([key, value]) => `${formatter(key)} ${count(value)}${label}`)
      .join(' · ');
  }

  function duration(milliseconds) {
    const value = Number(milliseconds);
    if (!Number.isFinite(value) || value < 0) return '不可用';
    return value >= 1000 ? `${(value / 1000).toFixed(value >= 10000 ? 0 : 1)} 秒` : `${Math.round(value)} 毫秒`;
  }

  function modelExecutionSummary(execution) {
    const calls = Number(execution?.callCount || 0);
    if (!calls) return '尚未建立模型调用';
    const started = Number(execution?.startedCallCount || 0);
    const succeeded = Number(execution?.succeededCallCount || 0);
    const failed = Number(execution?.failedCallCount || 0);
    const running = Number(execution?.runningCallCount || 0);
    return `已记录 ${count(calls)} 次调用：已开始 ${count(started)} · 成功 ${count(succeeded)} · 失败 ${count(failed)} · 进行中 ${count(running)}`;
  }

  function modelExecutionDetails(execution) {
    const callCount = Number(execution?.callCount || 0);
    if (!callCount) return '本轮尚未将任何评论交给模型。若范围已冻结，请查看后台执行器状态。';
    const parts = [];
    const stages = countSummary(execution.stageCounts, ' 次', stageLabel);
    if (stages) parts.push(`阶段：${stages}`);
    if (Number(execution.elapsedMeasuredCallCount || 0) > 0) parts.push(`累计模型耗时：${duration(execution.elapsedMs)}`);
    if (Number(execution.usageMeasuredCallCount || 0) > 0) {
      parts.push(`模型报回 Token：输入 ${count(execution.inputTokens)} · 输出 ${count(execution.outputTokens)}`);
    }
    if (execution.chargedTokens !== null && execution.chargedTokens !== undefined) {
      parts.push(`账本 Token：${count(execution.chargedTokens)}${Number(execution.usageMeasuredCallCount || 0) < callCount ? '（含用量未知调用的预留）' : ''}`);
    }
    const failures = countSummary(execution.failureCounts, ' 次', itemFailureLabel);
    if (failures) parts.push(`调用失败原因：${failures}`);
    return parts.join('。') || '模型调用已建立，尚未取得可显示的用量或耗时。';
  }

  function renderChanges(data) {
    const observations = data.observations || [];
    const incomparable = data.notComparable || [];
    result.innerHTML = `<section class="cr-v1-intro"><h2>变化观察</h2><p>这里只回答“相对前一个完整 7 天，什么发生了可证实的变化”。它不重复当前问题列表。</p>${resultMeta(data)}</section>` +
      (observations.length ? table(['观察', '问题', '依据'], observations.map(item => `<tr><td><span class="cr-v1-signal">${escape(observationLabel(item.kind))}</span></td><td><strong>${escape(item.problemName)}</strong><p>${escape(item.problemMeaning)}</p></td><td>${escape(reasonLabel(item.reasonCode))}</td></tr>`)) : `<section class="cr-v1-empty"><h2>暂未发布可比变化</h2><p>这不表示没有讨论；当前已发布结果没有达到本页的可比条件或变化阈值。</p></section>`) +
      (incomparable.length ? `<section class="cr-v1-subsection"><h2>尚不可比较</h2>${table(['问题', '原因'], incomparable.map(item => `<tr><td>${escape(item.problemName)}</td><td>${escape(reasonLabel(item.reasonCode))}</td></tr>`))}</section>` : '');
  }

  function renderRuns(data) {
    const page = data.page || {items:[], total:0};
    result.innerHTML = `<section class="cr-v1-intro"><h2>运行记录</h2><p>每一轮都区分范围已冻结、模型是否实际执行、逐项结果与发布状态。运行失败不会被显示成“没有研究发现”。</p></section>` +
      (page.items.length ? table(['运行与冻结输入', '模型执行', '项目结果与安全原因', '发布'], page.items.map(item => {
        const runFailures = countSummary(item.failureCounts, ' 条', runFailureLabel);
        const itemFailures = countSummary(item.itemFailureCounts, ' 条', itemFailureLabel);
        const itemStates = countSummary(item.itemStates, ' 条', runItemStateLabel);
        const execution = item.modelExecution || {};
        const publishedCoverage = item.publishedResult?.inputCounts?.publicationCoverage;
        const health = item.researchHealth || {};
        const organization = health.organizationCoverage || {};
        const deferredText = [
          Number(health.deferredNovelAtomCount || 0) ? `等待独立证据 ${count(health.deferredNovelAtomCount)} 条` : '',
          Number(health.deferredAmbiguousAtomCount || 0) ? `等待消歧 ${count(health.deferredAmbiguousAtomCount)} 条` : '',
          Number(health.deferredContextAtomCount || 0) ? `等待语境 ${count(health.deferredContextAtomCount)} 条` : '',
          Number(health.outOfScopeProblemAtomCount || 0) ? `范围外 ${count(health.outOfScopeProblemAtomCount)} 条` : '',
          Number(health.notUserProblemAtomCount || 0) ? `非用户问题 ${count(health.notUserProblemAtomCount)} 条` : ''
        ].filter(Boolean).join(' · ');
        const healthText = `研究信号 ${count(health.researchSignalCount)} 条 · 问题/需求 ${count(health.problemBearingAtomCount)} 条 · 已归并 ${count(health.organizedProblemAtomCount)} 条 · 待开始归并 ${count(health.pendingProblemResolutionAtomCount)} 条 · 处理中 ${count(health.activeProblemResolutionAtomCount)} 条 · 归并终态失败 ${count(health.failedProblemResolutionAtomCount)} 条 · 本轮归并覆盖 ${pct(Number(organization.denominator) ? Number(organization.numerator) / Number(organization.denominator) : 1)}`;
        const activatedBacklog = Number(health.activatedBacklogAtomCount || 0);
        const skippedBacklog = Number(health.activatedBacklogSkippedAtomCount || 0);
        const backlogText = activatedBacklog ? `历史待归并续办 ${count(activatedBacklog)} 条 · 已补入 ${count(health.activatedBacklogResolvedAtomCount)} 条 · 处理中 ${count(health.activatedBacklogPendingAtomCount)} 条 · 未完成 ${count(health.activatedBacklogFailedAtomCount)} 条${skippedBacklog ? ` · 旧合同已跳过 ${count(skippedBacklog)} 条` : ''}` : '';
        const published = item.publishedResult?.resultRevisionRef
          ? `已发布${publishedCoverage === 'partial' ? '（部分覆盖）' : ''} ${escape(date(item.publishedResult.publishedAt))}`
          : item.state === 'completed_with_failures' ? '统计版本未发布：请查看本轮完整度与安全失败原因；已确认归并仍已累计到用户问题'
            : item.state === 'failed' ? '未发布：运行失败' : '尚未发布';
        return `<tr><td><strong>${escape(date(item.createdAt))}</strong><p>${escape(runStateLabel(item.state))} · 冻结 ${count(item.selectedSources)} 条评论</p>${item.finishedAt ? `<p>结束于 ${escape(date(item.finishedAt))}</p>` : ''}</td><td><strong>${escape(modelExecutionSummary(execution))}</strong><details class="cr-v1-run-detail"><summary>查看调用账本摘要</summary><p>${escape(modelExecutionDetails(execution))}</p></details></td><td><p>${escape(healthText)}</p>${deferredText ? `<p>${escape(deferredText)}</p>` : ''}${backlogText ? `<p>${escape(backlogText)}</p>` : ''}<p>${escape(itemStates || '尚未开始处理')}</p>${runFailures ? `<p>${escape(runFailures)}</p>` : ''}${itemFailures ? `<p>${escape(itemFailures)}</p>` : ''}</td><td>${published}</td></tr>`;
      })) + pagination(page, '运行记录') : empty('还没有运行记录。保存策略后可先查看系统自动选择的范围。'));
  }

  function render(view, data) {
    result.dataset.view = view;
    ({ overview:renderOverview, voices:renderVoices, problems:renderProblems, changes:renderChanges, runs:renderRuns })[view](data);
  }

  async function loadView() {
    renderTabs(); updateUrl(); result.setAttribute('aria-busy', 'true');
    try {
      const limit = pageLimit();
      const offset = pagedViews.has(state.view) ? (state.page - 1) * limit : 0;
      const problemQuery = state.view === 'problems' ? '&problemView=' + encodeURIComponent(state.problemView) : '';
      const data = await request(api + '/' + state.view + '?limit=' + limit + '&offset=' + offset + problemQuery);
      const total = Number(data.page?.total ?? 0);
      if (pagedViews.has(state.view) && total > 0 && offset >= total) {
        state.page = Math.max(1, Math.ceil(total / limit));
        return loadView();
      }
      render(state.view, data);
    } catch (error) {
      result.innerHTML = empty(error.message);
    } finally {
      result.setAttribute('aria-busy', 'false');
    }
  }

  async function openSettings() {
    try {
      await loadSetup();
      const { policy, defaultConfig, embedding } = state.setup;
      $('#policy-model').value = researchModelReady() ? defaultConfig.modelId : '尚未通过 V1 语义测试';
      form.elements.sourceLimit.value = policy?.sourceLimit || 1000;
      form.elements.tokenLimit.value = policy?.tokenLimit || 100000;
      $('#embedding-state').textContent = embeddingReady() ? `向量模型已通过测试并启用：${embedding.modelId}。` : '向量模型尚未通过测试、尚未启用或连接不可用。请在模型与向量设置中完成后再开始需要归并的研究。';
      $('#settings-feedback').textContent = researchModelReady() ? '' : '请先在模型与 AI 设置中完成当前研究模型的 V1 语义测试。';
      dialog.showModal();
    } catch (error) {
      setStatus(error.message, 'error');
    }
  }

  async function savePolicy(event) {
    event.preventDefault();
    const defaultConfig = state.setup?.defaultConfig;
    if (!researchModelReady()) {
      $('#settings-feedback').textContent = '当前研究模型尚未通过 V1 语义测试，无法保存策略。';
      return;
    }
    const button = form.querySelector('[type="submit"]'); button.disabled = true;
    try {
      await request(`${api}/policy`, { method:'POST', body:JSON.stringify({ configRef:defaultConfig.configRef, sourceLimit:Number(form.elements.sourceLimit.value), tokenLimit:Number(form.elements.tokenLimit.value) }) });
      await loadSetup();
      $('#settings-feedback').textContent = '研究策略已保存。之后先查看系统自动范围，再确认冻结并开始研究。';
    } catch (error) {
      $('#settings-feedback').textContent = error.message;
    } finally {
      button.disabled = false;
    }
  }

  function previewBlocker(preview) {
    if (!preview?.policyConfigured) return '请先保存研究策略，系统才有本轮自动选择上限。';
    if (!researchModelReady()) return errorText.research_model_not_ready;
    if (!embeddingReady()) return errorText.embedding_not_ready;
    if (!Number(preview.selectedSources || 0) && !Number(preview.selectedBacklogAtoms || 0)) return '当前没有可进入本轮研究的用户原声，也没有符合续办条件的历史待归并 Atom。已有有效结论、正在执行或终态 unresolved 的项会保留在各自的运行记录中。';
    return '';
  }

  function renderRunPreview(preview) {
    const target = $('#run-preview');
    const feedback = $('#run-preview-feedback');
    const confirm = $('#confirm-run');
    const terminal = countSummary(preview.terminalItemStates, ' 条', previewTerminalStateLabel);
    if (!preview.policyConfigured) {
      target.innerHTML = `<p>当前有 ${count(preview.eligibleSources)} 条可读普通用户原声，但尚未保存研究策略，系统不能计算本轮上限。</p>`;
    } else {
      target.innerHTML = `<p>服务端会按当前策略和既有运行记录自动选择；确认时会再次计算并冻结范围。</p><dl><div><dt>本次新增评论</dt><dd>${count(preview.selectedSources)} 条</dd></div><div><dt>历史待归并续办</dt><dd>${count(preview.selectedBacklogAtoms)} 条</dd></div><div><dt>尚未进入研究</dt><dd>${count(preview.unprocessedSources)} 条</dd></div><div><dt>可恢复</dt><dd>${count(preview.recoverableSources)} 条</dd></div><div><dt>已提取研究信号</dt><dd>${count(preview.succeededSources)} 条</dd></div><div><dt>未提取到信号</dt><dd>${count(preview.noSignalSources)} 条</dd></div><div><dt>已有执行或恢复中</dt><dd>${count(Number(preview.activeSources) + Number(preview.retryableSources))} 条</dd></div><div><dt>需关联语境</dt><dd>${count(preview.selectedContextSources)} 条</dd></div></dl><p>已提取信号和未提取信号的原声保留为有效研究结论，不会重复调用模型。符合止损条件的历史待归并 Atom 只会在本次确认创建的新 Run 中续办；成功后立即补入累计问题库和其原冻结 Run 的组织覆盖，不会形成新的统计窗口。</p>${preview.selectedMissingParentContextSources ? `<p>所选范围中有 ${count(preview.selectedMissingParentContextSources)} 条需要关联语境，但当前没有可读父评论记录；该事实会随输入冻结保留，不会由页面补造。</p>` : ''}${terminal ? `<p>当前研究输入下不再自动外发：${escape(terminal)}。</p>` : ''}`;
    }
    const blocker = previewBlocker(preview);
    feedback.textContent = blocker || (state.setup?.worker?.lastSeenAt ? '确认后会创建一个新的冻结 Run；模型是否已执行及其结果会在运行记录中如实更新。' : '确认后会创建一个新的冻结 Run；尚未记录 Worker 心跳，模型调用会等待 Worker 启动。');
    feedback.dataset.kind = blocker ? 'warning' : 'ready';
    confirm.disabled = Boolean(blocker);
  }

  async function openRunPreview() {
    try {
      await loadSetup();
      state.preview = await request(`${api}/runs/preview`);
      renderRunPreview(state.preview);
      runDialog.showModal();
    } catch (error) {
      setStatus(error.message, 'error');
    }
  }

  async function confirmRun() {
    const button = $('#confirm-run');
    button.disabled = true;
    try {
      await loadSetup();
      const blocker = previewBlocker(state.preview);
      if (blocker) {
        $('#run-preview-feedback').textContent = blocker;
        $('#run-preview-feedback').dataset.kind = 'warning';
        return;
      }
      const receipt = await request(`${api}/runs`, { method:'POST', body:JSON.stringify({}) });
      setStatus(`已冻结 ${count(receipt.selectedSources)} 条新增评论${Number(receipt.selectedBacklogAtoms || 0) ? `，并续办 ${count(receipt.selectedBacklogAtoms)} 条历史待归并 Atom` : ''}；后台将继续完成语义提取、向量归并与结果发布。`, 'ready');
      runDialog.close();
      state.view = 'runs'; state.page = 1; await loadView();
    } catch (error) {
      $('#run-preview-feedback').textContent = error.message;
      $('#run-preview-feedback').dataset.kind = 'error';
    }
    finally {
      if (runDialog.open) button.disabled = Boolean(previewBlocker(state.preview));
    }
  }

  document.querySelectorAll('[data-view]').forEach(button => button.addEventListener('click', async () => { state.view = button.dataset.view; state.page = 1; await loadView(); }));
  result.addEventListener('click', async event => {
    const filter = event.target.closest('[data-problem-view]');
    if (filter) {
      state.problemView = filter.dataset.problemView;
      state.page = 1;
      await loadView();
      return;
    }
    const detail = event.target.closest('[data-problem-detail]');
    if (detail) {
      const item = state.problemItems[Number(detail.dataset.problemDetail)];
      if (item) renderProblemDetail(item);
      return;
    }
    const button = event.target.closest('[data-page-offset]');
    if (!button || button.disabled) return;
    state.page = Math.max(1, Math.floor(Number(button.dataset.pageOffset) / pageLimit()) + 1);
    await loadView();
    result.scrollIntoView({ block:'start', behavior:'smooth' });
  });
  $('#refresh').addEventListener('click', async () => { await loadSetup().catch(error => setStatus(error.message, 'error')); await loadView(); });
  $('#research-settings').addEventListener('click', openSettings);
  $('#start-run').addEventListener('click', openRunPreview);
  $('#confirm-run').addEventListener('click', confirmRun);
  form.addEventListener('submit', savePolicy);
  document.querySelectorAll('[data-close]').forEach(button => button.addEventListener('click', () => document.getElementById(button.dataset.close).close()));

  Promise.all([loadSetup(), loadView()]).catch(error => setStatus(error.message, 'error'));
})();
