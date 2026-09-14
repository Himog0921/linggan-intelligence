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
  const query = new URLSearchParams(location.search);
  const initialPage = Math.max(1, Number.parseInt(query.get('page') || '1', 10) || 1);
  const state = { view: query.get('view') || 'overview', page: initialPage, setup: null, preview: null };
  if (!views.has(state.view)) state.view = 'overview';
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

  function resultWindow(data) {
    const input = data.result?.inputCounts || {};
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
    const input = data.result?.inputCounts || {};
    const selected = Number(input.selectedCommentCount ?? input.analyzedCommentCount ?? 0);
    const included = Number(input.includedCommentCount ?? input.analyzedCommentCount ?? 0);
    const excluded = Number(input.excludedTerminalCommentCount ?? 0);
    const organization = input.problemOrganizationCoverage || {};
    const partial = input.publicationCoverage === 'partial';
    const window = resultWindow(data);
    const coverage = partial
      ? '部分研究版本 · 本版纳入 ' + count(included) + ' / ' + count(selected) + ' 条冻结原声，' + count(excluded) + ' 条未纳入；问题归并 ' + count(organization.numerator) + ' / ' + count(organization.denominator) + ' 条 Atom。'
      : '完整研究版本 · 本版纳入 ' + count(included) + ' / ' + count(selected) + ' 条冻结原声。';
    const comparison = data.result?.comparison || {};
    const windowText = window.kind === 'baseline'
      ? '本版样本位于基线窗口 ' + escape(date(comparison.baseline?.start)) + ' 至 ' + escape(date(comparison.baseline?.end)) + '；当前窗口没有可比样本。'
      : '当前窗口 ' + escape(date(comparison.current?.start)) + ' 至 ' + escape(date(comparison.current?.end)) + '。';
    return '<p class="cr-v1-result-meta">已发布 ' + escape(date(data.result?.publishedAt)) + ' · ' + windowText + ' · ' + escape(coverage) + (partial ? ' <a href="?view=runs">查看运行记录</a>' : '') + '</p>';
  }

  function renderOverview(data) {
    const items = data.currentProblems || [];
    const window = resultWindow(data);
    const baselineOnly = window.kind === 'baseline';
    const heading = baselineOnly ? '本版已被研究的问题' : '当前已被研究的问题';
    const explanation = baselineOnly
      ? '本版冻结原声全部位于基线窗口，当前窗口没有可比样本。这里呈现已完成的研究证据；变化是否成立仍只由“变化观察”判断。'
      : '这里回答“当前用户在表达什么”。变化信号只在“变化观察”中呈现。';
    const columns = baselineOnly
      ? ['用户问题', '本版基线评论占比', '本版基线作品覆盖', '本版基线评论数']
      : ['用户问题', '当前评论占比', '当前作品覆盖', '当前评论数'];
    const rows = items.map(item => {
      const commentCount = baselineOnly ? item.baselineCommentCount : item.currentCommentCount;
      const workCount = baselineOnly ? item.baselineWorkCount : item.currentWorkCount;
      const commentShare = baselineOnly ? share(commentCount, window.comments) : item.currentCommentShare;
      const workShare = baselineOnly ? share(workCount, window.works) : item.currentWorkShare;
      return '<tr><td><strong>' + escape(item.name) + '</strong><p>' + escape(item.meaning) + '</p></td><td>' + pct(commentShare) + '</td><td>' + pct(workShare) + '</td><td>' + count(commentCount) + '</td></tr>';
    });
    const overviewDisclosure = items.length
      ? '<p class="cr-v1-overview-disclosure">概览仅展示 ' + count(items.length) + ' 个代表问题。<a href="?view=problems">查看全部用户问题（可分页浏览）</a></p>'
      : '';
    result.innerHTML = '<section class="cr-v1-intro"><h2>' + heading + '</h2><p>' + explanation + '</p>' + resultMeta(data) + '</section>' +
      (items.length ? table(columns, rows) + overviewDisclosure : empty('本版研究没有形成可显示的问题。'));
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
    const window = resultWindow(data);
    const baselineOnly = window.kind === 'baseline';
    const columns = baselineOnly
      ? ['问题定义', '本版基线样本', '证据 Atom']
      : ['问题定义', '当前窗口', '前一窗口', '证据 Atom'];
    const rows = page.items.map(item => {
      const definition = '<td><strong>' + escape(item.name) + '</strong><p>' + escape(item.meaning) + '</p></td>';
      if (baselineOnly) {
        const commentShare = share(item.baseline?.commentCount, window.comments);
        const workShare = share(item.baseline?.workCount, window.works);
        return '<tr>' + definition + '<td>' + count(item.baseline?.commentCount) + ' 条评论 · ' + pct(commentShare) + '<p>' + count(item.baseline?.workCount) + ' 篇作品 · ' + pct(workShare) + '</p></td><td>' + count(item.evidenceAtomCount) + '</td></tr>';
      }
      return '<tr>' + definition + '<td>' + count(item.current?.commentCount) + ' 条评论 · ' + pct(item.current?.commentShare) + '<p>' + count(item.current?.workCount) + ' 篇作品 · ' + pct(item.current?.workShare) + '</p></td><td>' + count(item.baseline?.commentCount) + ' 条评论 · ' + pct(item.baseline?.commentShare) + '<p>' + count(item.baseline?.workCount) + ' 篇作品 · ' + pct(item.baseline?.workShare) + '</p></td><td>' + count(item.evidenceAtomCount) + '</td></tr>';
    });
    const explanation = baselineOnly
      ? '本版冻结样本没有落入当前窗口，因此先展示实际被研究的基线样本；“变化观察”仍会明确说明不可比较。'
      : '不同表达只有在记录了归并依据后才属于同一问题；向量相似度本身不会合并身份。';
    result.innerHTML = '<section class="cr-v1-intro"><h2>稳定用户问题</h2><p>' + explanation + '</p>' + resultMeta(data) + '</section>' +
      (page.items.length ? pagination(page, '用户问题', 'top') + table(columns, rows) + pagination(page, '用户问题') : empty('本版没有可显示的稳定问题。'));
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
        const published = item.publishedResult?.resultRevisionRef
          ? `已发布${publishedCoverage === 'partial' ? '（部分覆盖）' : ''} ${escape(date(item.publishedResult.publishedAt))}`
          : item.state === 'completed_with_failures' ? '未发布：覆盖不足或有未组织的研究信号'
            : item.state === 'failed' ? '未发布：运行失败' : '尚未发布';
        return `<tr><td><strong>${escape(date(item.createdAt))}</strong><p>${escape(runStateLabel(item.state))} · 冻结 ${count(item.selectedSources)} 条评论</p>${item.finishedAt ? `<p>结束于 ${escape(date(item.finishedAt))}</p>` : ''}</td><td><strong>${escape(modelExecutionSummary(execution))}</strong><details class="cr-v1-run-detail"><summary>查看调用账本摘要</summary><p>${escape(modelExecutionDetails(execution))}</p></details></td><td><p>${escape(itemStates || '尚未开始处理')}</p>${runFailures ? `<p>${escape(runFailures)}</p>` : ''}${itemFailures ? `<p>${escape(itemFailures)}</p>` : ''}</td><td>${published}</td></tr>`;
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
      const data = await request(api + '/' + state.view + '?limit=' + limit + '&offset=' + offset);
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
    if (!Number(preview.selectedSources || 0)) return '当前没有可进入本轮研究的用户原声。已有有效结论、正在执行或当前合同已拒绝的项会保留在各自的运行记录中。';
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
      target.innerHTML = `<p>服务端会按当前策略和既有运行记录自动选择；确认时会再次计算并冻结范围。</p><dl><div><dt>本次自动处理</dt><dd>${count(preview.selectedSources)} 条</dd></div><div><dt>尚未进入研究</dt><dd>${count(preview.unprocessedSources)} 条</dd></div><div><dt>可恢复</dt><dd>${count(preview.recoverableSources)} 条</dd></div><div><dt>已提取研究信号</dt><dd>${count(preview.succeededSources)} 条</dd></div><div><dt>未提取到信号</dt><dd>${count(preview.noSignalSources)} 条</dd></div><div><dt>已有执行或恢复中</dt><dd>${count(Number(preview.activeSources) + Number(preview.retryableSources))} 条</dd></div><div><dt>需关联语境</dt><dd>${count(preview.selectedContextSources)} 条</dd></div></dl><p>已提取信号和未提取信号的原声保留为有效研究结论，不会重复调用模型。执行中的原声由原 Run 推进；可恢复项会按当前研究输入自动判断是否进入本轮。</p>${preview.selectedMissingParentContextSources ? `<p>所选范围中有 ${count(preview.selectedMissingParentContextSources)} 条需要关联语境，但当前没有可读父评论记录；该事实会随输入冻结保留，不会由页面补造。</p>` : ''}${terminal ? `<p>当前研究输入下不再自动外发：${escape(terminal)}。</p>` : ''}`;
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
      setStatus(`已冻结 ${count(receipt.selectedSources)} 条普通用户评论，后台将继续完成语义提取、向量归并与结果发布。`, 'ready');
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
