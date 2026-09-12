(() => {
  'use strict';

  const api = '/api/local/comment-research';
  const $ = (selector) => document.querySelector(selector);
  const result = $('#research-result');
  const status = $('#research-status');
  const dialog = $('#research-settings-dialog');
  const form = $('#research-settings-form');
  const views = new Set(['overview', 'voices', 'problems', 'changes', 'runs']);
  const state = { view: new URLSearchParams(location.search).get('view') || 'overview', setup: null };
  if (!views.has(state.view)) state.view = 'overview';

  const errorText = {
    comment_research_result_unavailable: '还没有可读取的已发布研究结果。先保存策略并开始一轮研究；后台完成提取、向量归并和结果冻结后，页面会显示新的版本。',
    comment_research_v1_schema_missing: '评论研究 V1 的数据库结构尚未就绪。',
    comment_research_unavailable: '评论研究暂时不可用，请检查本机数据服务。',
    comment_research_v1_unavailable: '本次研究结果暂时无法读取；上一次已显示的结果不会被伪造成新结果。',
    research_policy_missing: '请先保存研究策略。',
    embedding_not_ready: '请先在“模型与向量设置”中完成向量模型测试并启用；系统不会创建一个必然无法归并和发布的研究运行。',
    research_model_not_ready: '请先在“模型与 AI 设置”中完成当前研究模型的 V1 语义测试；系统不会向未通过结构化输出检查的模型发送评论。',
    no_eligible_research_comments: '当前没有可进入研究的普通用户评论。作品作者回复、身份未知或不可读评论不会被混入。',
    invalid_research_request: '研究设置或运行范围不符合要求。',
  };

  const escape = (value) => String(value ?? '').replace(/[&<>"']/g, character => ({ '&':'&amp;', '<':'&lt;', '>':'&gt;', '"':'&quot;', "'":'&#39;' })[character]);
  const date = value => value ? new Intl.DateTimeFormat('zh-CN', { dateStyle:'medium', timeStyle:'short', timeZone:'Asia/Shanghai' }).format(new Date(value)) : '未知';
  const count = value => Number(value ?? 0).toLocaleString('zh-CN');
  const pct = value => `${(Number(value ?? 0) * 100).toFixed(1)}%`;
  const empty = message => `<section class="cr-v1-empty"><h2>暂时没有可显示的研究结果</h2><p>${escape(message)}</p></section>`;
  const table = (head, rows) => `<div class="cr-v1-table-wrap"><table><thead><tr>${head.map(item => `<th scope="col">${escape(item)}</th>`).join('')}</tr></thead><tbody>${rows.join('')}</tbody></table></div>`;

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
    history.replaceState(null, '', `${location.pathname}?${query}`);
  }

  function renderTabs() {
    document.querySelectorAll('[data-view]').forEach(button => {
      button.setAttribute('aria-current', button.dataset.view === state.view ? 'page' : 'false');
    });
  }

  function setupSummary() {
    const { policy, defaultConfig, embedding, worker } = state.setup || {};
    if (!researchModelReady()) return '研究模型尚未通过 V1 语义测试。保存策略和开始研究都不会发送评论。';
    const embeddingText = embeddingReady()
      ? `向量归并已就绪（${embedding.dimensions} 维）。`
      : '向量归并尚未就绪；开始研究已锁定，避免产生无法归并和发布的无效运行。请先在模型与向量设置中完成测试并启用。';
    const policyText = policy ? `当前策略已保存：每轮最多 ${count(policy.sourceLimit)} 条评论。` : '尚未保存研究策略。';
    const workerText = worker?.state === 'error' ? '后台执行器上次报告异常。' : '连续自动排程关闭。';
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
    const ready = embeddingReady() && researchModelReady();
    start.disabled = !ready;
    start.title = ready ? '' : '请先在模型与 AI 设置完成研究模型的 V1 语义测试，并在模型与向量设置启用向量模型。';
  }

  async function loadSetup() {
    state.setup = await request(`${api}/setup`);
    renderRunAvailability();
    setStatus(setupSummary(), researchModelReady() && embeddingReady() ? 'ready' : 'warning');
  }

  function resultMeta(data) {
    const input = data.result?.inputCounts || {};
    return `<p class="cr-v1-result-meta">已发布 ${escape(date(data.result?.publishedAt))} · 当前窗口 ${escape(date(data.result?.comparison?.current?.start))} 至 ${escape(date(data.result?.comparison?.current?.end))} · 冻结样本 ${count(input.analyzedCommentCount)} 条</p>`;
  }

  function renderOverview(data) {
    const items = data.currentProblems || [];
    result.innerHTML = `<section class="cr-v1-intro"><h2>当前已被研究的问题</h2><p>这里回答“当前用户在表达什么”。变化信号只在“变化观察”中呈现。</p>${resultMeta(data)}</section>` +
      (items.length ? table(['用户问题', '当前评论占比', '当前作品覆盖', '当前评论数'], items.map(item => `<tr><td><strong>${escape(item.name)}</strong><p>${escape(item.meaning)}</p></td><td>${pct(item.currentCommentShare)}</td><td>${pct(item.currentWorkShare)}</td><td>${count(item.currentCommentCount)}</td></tr>`)) : empty('本版研究没有形成可显示的问题。'));
  }

  function renderVoices(data) {
    const page = data.page || {items:[], total:0};
    result.innerHTML = `<section class="cr-v1-intro"><h2>用户原声</h2><p>这里仅呈现已通过身份过滤的普通用户评论。原文是证据；研究正文是独立派生，不会改写原文，也不显示向量准备等实现状态。</p>${resultMeta(data)}<p class="cr-v1-result-meta">本版共 ${count(page.total)} 条冻结输入。</p></section>` +
      (page.items.length ? table(['评论原文', '研究正文', '作品与观察时间', '研究结果'], page.items.map(item => `<tr><td><blockquote>${escape(item.commentText || '正文尚未取得')}</blockquote></td><td><p>${escape(item.researchText || '未形成研究正文')}</p><span class="cr-v1-badge">普通用户</span></td><td><strong>${escape(item.workTitle || '作品标题未知')}</strong><p>${escape(date(item.observedAt))}</p></td><td>${escape(item.researchOutcome || '状态未知')}<p>${(item.atomKinds || []).map(escape).join(' · ') || '未形成 Atom'}</p></td></tr>`)) : empty('本版没有可显示的原声。'));
  }

  function renderProblems(data) {
    const page = data.page || {items:[], total:0};
    result.innerHTML = `<section class="cr-v1-intro"><h2>稳定用户问题</h2><p>不同表达只有在记录了归并依据后才属于同一问题；向量相似度本身不会合并身份。</p>${resultMeta(data)}</section>` +
      (page.items.length ? table(['问题定义', '当前窗口', '前一窗口', '证据 Atom'], page.items.map(item => `<tr><td><strong>${escape(item.name)}</strong><p>${escape(item.meaning)}</p></td><td>${count(item.current?.commentCount)} 条评论 · ${pct(item.current?.commentShare)}<p>${count(item.current?.workCount)} 篇作品 · ${pct(item.current?.workShare)}</p></td><td>${count(item.baseline?.commentCount)} 条评论 · ${pct(item.baseline?.commentShare)}<p>${count(item.baseline?.workCount)} 篇作品 · ${pct(item.baseline?.workShare)}</p></td><td>${count(item.evidenceAtomCount)}</td></tr>`)) : empty('本版没有可显示的稳定问题。'));
  }

  function observationLabel(kind) {
    return ({ rising:'升温', falling:'降温', spreading:'扩散', newly_observed:'新出现' })[kind] || '未命名观察';
  }

  function reasonLabel(code) {
    return ({ insufficient_window_coverage:'两个完整窗口的样本覆盖不足', first_observed_in_comparable_history:'首次出现在本系统可比研究历史中', comment_share_increased:'评论占比上升', comment_share_decreased:'评论占比下降', work_coverage_increased:'作品覆盖率上升' })[code] || '已记录原因';
  }

  function runStateLabel(state) {
    return ({ queued:'等待处理', running:'正在研究', completed:'已完成', completed_with_failures:'未发布', failed:'失败', cancelled:'已取消' })[state] || '状态未知';
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
      model_not_qualified:'研究模型未通过 V1 语义测试',
      model_budget_exhausted:'研究预算已用尽',
      model_input_limit:'单条输入超出模型限制',
      embedding_not_qualified:'向量模型不可用',
      provider_timeout:'模型服务超时',
      provider_failed:'模型服务未返回可用结果'
    })[code] || '该项未通过研究处理';
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
    result.innerHTML = `<section class="cr-v1-intro"><h2>运行记录</h2><p>记录冻结样本、逐项结果与已发布版本。运行失败不会被显示成“没有研究发现”。</p></section>` +
      (page.items.length ? table(['开始时间', '运行状态', '冻结输入', '已发布结果'], page.items.map(item => { const failures=Object.entries(item.failureCounts || {}).filter(([,value]) => Number(value) > 0); const itemFailures=Object.entries(item.itemFailureCounts || {}).filter(([,value]) => Number(value) > 0); return `<tr><td>${escape(date(item.createdAt))}</td><td><strong>${escape(runStateLabel(item.state))}</strong><p>${Object.entries(item.itemStates || {}).map(([key, value]) => `${escape(key)} ${count(value)}`).join(' · ') || '尚未开始'}</p>${failures.length ? `<p>${failures.map(([key, value]) => `${escape(runFailureLabel(key))} ${count(value)} 条`).join(' · ')}</p>` : ''}${itemFailures.length ? `<p>${itemFailures.map(([key, value]) => `${escape(itemFailureLabel(key))} ${count(value)} 条`).join(' · ')}</p>` : ''}</td><td>${count(item.selectedSources)} 条评论</td><td>${item.publishedResult?.resultRevisionRef ? `已发布 ${escape(date(item.publishedResult.publishedAt))}` : item.state === 'completed_with_failures' ? '未发布：请修复运行记录所示问题后重新开始研究' : '尚未发布'}</td></tr>`; })) : empty('还没有运行记录。保存策略后可以直接开始第一轮研究。'));
  }

  function render(view, data) {
    ({ overview:renderOverview, voices:renderVoices, problems:renderProblems, changes:renderChanges, runs:renderRuns })[view](data);
  }

  async function loadView() {
    renderTabs(); updateUrl(); result.setAttribute('aria-busy', 'true');
    try {
      const data = await request(`${api}/${state.view}?limit=20&offset=0`);
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
      $('#settings-feedback').textContent = '研究策略已保存。之后每次点击“开始研究”将直接冻结范围，不再要求重复授权。';
    } catch (error) {
      $('#settings-feedback').textContent = error.message;
    } finally {
      button.disabled = false;
    }
  }

  async function startRun() {
    try {
      await loadSetup();
      if (!researchModelReady() || !embeddingReady()) {
        setStatus(!researchModelReady() ? errorText.research_model_not_ready : errorText.embedding_not_ready, 'warning');
        return;
      }
      if (!state.setup?.policy) { await openSettings(); return; }
      const receipt = await request(`${api}/runs`, { method:'POST', body:JSON.stringify({}) });
      setStatus(`已冻结 ${count(receipt.selectedSources)} 条普通用户评论，后台将继续完成语义提取、向量归并与结果发布。`, 'ready');
      state.view = 'runs'; await loadView();
    } catch (error) {
      setStatus(error.message, 'error');
    }
  }

  document.querySelectorAll('[data-view]').forEach(button => button.addEventListener('click', async () => { state.view = button.dataset.view; await loadView(); }));
  $('#refresh').addEventListener('click', async () => { await loadSetup().catch(error => setStatus(error.message, 'error')); await loadView(); });
  $('#research-settings').addEventListener('click', openSettings);
  $('#start-run').addEventListener('click', startRun);
  form.addEventListener('submit', savePolicy);
  document.querySelectorAll('[data-close]').forEach(button => button.addEventListener('click', () => document.getElementById(button.dataset.close).close()));

  Promise.all([loadSetup(), loadView()]).catch(error => setStatus(error.message, 'error'));
})();
