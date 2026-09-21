const endpoint = '/api/local/comment-study/';
const esc = value => String(value ?? '').replace(/[&<>\"]/g, character => ({
  '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;'
}[character]));

const get = async path => {
  const response = await fetch(endpoint + path, { headers: { Accept: 'application/json' } });
  if (!response.ok) throw new Error(`请求失败（状态码 ${response.status}）`);
  return response.json();
};
const post = async (path, body) => {
  const response = await fetch(endpoint + path, { method: 'POST', headers: { Accept: 'application/json', 'Content-Type': 'application/json' }, body: JSON.stringify(body) });
  if (!response.ok) throw new Error(`请求失败（状态码 ${response.status}）`);
  return response.json();
};
const list = (items, render, empty) => items?.length ? items.map(render).join('') : `<p class="muted">${esc(empty)}</p>`;
let loadedWorks = [];
let sourcePreview = null;
const selectedWorkRefs = new Set();

const normalizedFilter = () => document.querySelector('#work-filter').value.trim().toLocaleLowerCase('zh-CN');
const visibleWorks = () => {
  const filter = normalizedFilter();
  return filter ? loadedWorks.filter(work => String(work.title ?? '').toLocaleLowerCase('zh-CN').includes(filter)) : loadedWorks;
};
const selectedWorks = () => [...selectedWorkRefs];

function updateSelection() {
  const count = selectedWorks().length;
  const selectedEligible = loadedWorks
    .filter(work => selectedWorkRefs.has(work.workRef))
    .reduce((total, work) => total + Number(work.eligibleCommentCount || 0), 0);
  const budget = Number(document.querySelector('#comment-budget').value || 0);
  const frozenCount = budget > 0 ? Math.min(selectedEligible, budget) : 0;
  document.querySelector('#selected-count').textContent = count
    ? `已选择 ${count} 篇 · 合格 ${selectedEligible} 条 · 本次最多冻结 ${frozenCount} 条`
    : '已选择 0 篇';
  document.querySelector('#start-run').disabled = count === 0;
  const visible = visibleWorks();
  const selectVisible = document.querySelector('#select-visible-works');
  const selectedVisibleCount = visible.filter(work => selectedWorkRefs.has(work.workRef)).length;
  selectVisible.checked = visible.length > 0 && selectedVisibleCount === visible.length;
  selectVisible.indeterminate = selectedVisibleCount > 0 && selectedVisibleCount < visible.length;
}

function renderSourcePreview(preview) {
  const target = document.querySelector('#source-preview');
  if (!preview) {
    target.textContent = '评论资格统计暂不可用。';
    return;
  }
  const excluded = preview.excludedCounts || {};
  const labels = {
    commentAuthorUnknown: '评论作者身份未知',
    workAuthorUnknown: '作品作者身份未知',
    creatorVoice: '作品作者本人',
    bodyUnavailable: '正文不可研究',
    sourceRestricted: '来源受限',
    textNotResearchable: '文本不具研究条件'
  };
  const reasons = Object.entries(labels)
    .map(([key, label]) => [label, Number(excluded[key] || 0)])
    .filter(([, count]) => count > 0)
    .map(([label, count]) => `${label} ${count} 条`);
  const excludedCount = Number(preview.totalCommentCount || 0) - Number(preview.eligibleCommentCount || 0);
  target.textContent = `截至 ${String(preview.asOf || '').replace('T', ' ')}：共 ${Number(preview.totalCommentCount || 0)} 条评论；可研究 ${Number(preview.eligibleCommentCount || 0)} 条；未纳入 ${excludedCount} 条${reasons.length ? `（${reasons.join('；')}）` : ''}。`;
}
function renderWorks() {
  const container = document.querySelector('#works');
  const visible = visibleWorks();
  const filter = normalizedFilter();
  document.querySelector('#work-filter-status').textContent = filter
    ? `当前筛选命中 ${visible.length} 篇，已加载 ${loadedWorks.length} 篇可研究作品。`
    : `已加载 ${loadedWorks.length} 篇可研究作品。`;
  container.innerHTML = visible.length
    ? visible.map(work => `<tr><td><input id="work-${esc(work.workRef)}" type="checkbox" name="work-ref" value="${esc(work.workRef)}" aria-label="选择作品：${esc(work.title)}"${selectedWorkRefs.has(work.workRef) ? ' checked' : ''}></td><td><label for="work-${esc(work.workRef)}"><span class="study-work-title">${esc(work.title)}</span></label></td><td>${Number(work.eligibleCommentCount)}</td></tr>`).join('')
    : `<tr><td class="study-table-empty" colspan="3">${filter ? '当前筛选没有命中已加载作品。' : '当前没有符合条件的 ADHD 作品。'}</td></tr>`;
  container.querySelectorAll('input[name="work-ref"]').forEach(input => input.addEventListener('change', event => {
    if (event.currentTarget.checked) selectedWorkRefs.add(event.currentTarget.value);
    else selectedWorkRefs.delete(event.currentTarget.value);
    updateSelection();
  }));
  updateSelection();
}
async function loadSetup() {
  const status = document.querySelector('#setup-status');
  try {
    const setup = await get('setup');
    const select = document.querySelector('#model-config');
    select.innerHTML = list(setup.modelConfigs, config => `<option value="${esc(config.configRef)}">${esc(config.modelId)} · 输入上限 ${Number(config.inputTokenLimit)} 词元／输出上限 ${Number(config.outputTokenLimit)} 词元</option>`, '没有可用模型配置。');
    const available = setup.modelConfigs?.length > 0;
    select.disabled = !available;
    document.querySelector('#save-policy').disabled = !available;
    sourcePreview = setup.sourcePreview || null;
    renderSourcePreview(sourcePreview);
    loadedWorks = setup.eligibleWorks || [];
    selectedWorkRefs.clear();
    renderWorks();
    status.textContent = available ? `已加载 ${setup.eligibleWorks?.length || 0} 篇可选作品；此列表最多展示 100 篇。` : '没有启用的模型配置，无法保存策略。';
  } catch (error) {
    status.textContent = `无法读取准备信息：${error.message}`;
    document.querySelector('#work-filter-status').textContent = '作品列表不可用。';
    sourcePreview = null;
    renderSourcePreview(null);
    document.querySelector('#works').innerHTML = '<tr><td class="study-table-empty" colspan="3">作品列表不可用。</td></tr>';
  }
}
const targetStateLabel = {
  ready: '准备就绪', needs_context: '等待语境', excluded: '来源受限，未处理', queued: '排队中',
  running: '处理中', succeeded: '已产出结果', no_signal: '已处理 · 无信号', failed: '处理失败'
};
const eligibilityLabel = {
  eligible: '具备归并资格', deferred_context: '语境不足，暂缓', not_user_problem: '不构成用户问题',
  not_applicable: '不适用（非问题/需求类信号）'
};
const resolutionLabel = {
  pending: '待归并判断', assigned: '已归入用户问题', deferred_context: '语境不足，继续等待',
  deferred_ambiguous: '存在多个可能匹配，继续等待', deferred_novel: '独立新证据，等待第二条佐证',
  retrieval_incomplete: '候选目录未查全，当前不能判定是否为新问题',
  budget_stopped: '归并预算已到上限，当前未完成判断',
  not_user_problem: '判定不构成用户问题', protocol_rejected: '模型输出不合规，已拒绝', failed: '归并判断失败'
};
const signalKindLabel = {
  problem: '问题', need: '需求', belief: '观念', emotion: '情绪', experience: '经历',
  solution: '解决方案', quote: '引述', context: '语境', question: '疑问'
};
const problemStateLabel = { active: '生效中', retired: '已停用' };
const contextStateLabel = { ready: '语境完整', partial: '语境部分（有截断）', missing: '缺少语境' };
const sourceStateLabel = { known: null, restricted: '来源已被限制，原文不再显示', unknown: '原文未知（来源未采集到正文）' };
const label = (map, value) => (value == null ? null : (map[value] ?? '未知状态'));
const PENDING_RESOLUTION_STATES = new Set(['pending', 'deferred_context', 'deferred_ambiguous', 'deferred_novel', 'retrieval_incomplete', 'budget_stopped']);

let allRuns = [];
let activeView = 'overview';
let selectedRunRef = null;
const RUN_SCOPED_VIEWS = new Set(['targets', 'pending']);

function runOptionLabel(run) {
  return `${run.runRef.slice(0, 8)}… · ${esc(label(targetStateLabel, run.state) ?? run.state)} · ${run.createdAt.slice(0, 16).replace('T', ' ')}`;
}
function renderRunPicker() {
  const picker = document.querySelector('#study-run-picker');
  const select = document.querySelector('#study-run-select');
  picker.hidden = !RUN_SCOPED_VIEWS.has(activeView);
  if (!RUN_SCOPED_VIEWS.has(activeView)) return;
  if (!allRuns.length) {
    select.innerHTML = '<option value="">尚无研究运行</option>';
    select.disabled = true;
    return;
  }
  select.disabled = false;
  select.innerHTML = allRuns.map(run => `<option value="${esc(run.runRef)}"${run.runRef === selectedRunRef ? ' selected' : ''}>${runOptionLabel(run)}</option>`).join('');
}

async function renderOverviewTab() {
  const overview = await get('overview');
  if (overview.cleanLayerState === 'not_configured') {
    return '<p class="study-empty">尚未配置研究策略：先在上方保存一次策略。</p>';
  }
  const latest = overview.latestRun;
  if (!latest) {
    return '<p class="study-empty">尚未创建过研究运行：选择作品并点击「创建研究运行」。</p>';
  }
  const targetTotal = Object.values(latest.targetStates || {}).reduce((sum, value) => sum + Number(value), 0);
  const semantic = latest.semanticSummary || {};
  const stateRows = (map, states, emptyLabel) => Object.keys(states || {}).length
    ? Object.entries(states).map(([key, value]) => `<div class="study-stat"><strong>${Number(value)}</strong><span>${esc(label(map, key) ?? key)}</span></div>`).join('')
    : `<p class="study-empty">${esc(emptyLabel)}</p>`;
  return `
    <article class="study-overview-run">
      <header><p class="study-label">最新一次运行</p><h3>${esc(latest.runRef)}</h3><p>创建于 ${esc(latest.createdAt)}${latest.finishedAt ? ` · 结束于 ${esc(latest.finishedAt)}` : ' · 尚未结束'}</p></header>
      <p>选择作品 ${Number(latest.selectedWorkCount)} 篇，冻结评论目标 ${targetTotal} 条。</p>
      <div class="study-stat-group"><p class="study-label">目标处理状态</p><div class="study-stat-row">${stateRows(targetStateLabel, latest.targetStates, '尚无目标。')}</div></div>
      <div class="study-stat-group"><p class="study-label">研究信号的归并资格</p><div class="study-stat-row">${stateRows(eligibilityLabel, latest.signalStates, '本次运行没有研究信号。')}</div></div>
      <div class="study-stat-group"><p class="study-label">归并判断结果</p><div class="study-stat-row">${stateRows(resolutionLabel, latest.resolutionStates, '尚无已产生的归并判断。')}</div></div>
      <div class="study-stat-group"><p class="study-label">语义与证据校验</p><div class="study-stat-row">
        <div class="study-stat"><strong>${Number(semantic.modelInvocationCount || 0)}</strong><span>模型调用</span></div>
        <div class="study-stat"><strong>${Number(semantic.firstAttemptAcceptedTargetCount || 0)}</strong><span>首次成功目标</span></div>
        <div class="study-stat"><strong>${Number(semantic.retryRecoveredTargetCount || 0)}</strong><span>重试恢复目标</span></div>
        <div class="study-stat"><strong>${Number(semantic.finalSemanticContractFailureCount || 0)}</strong><span>最终语义合同失败</span></div>
        <div class="study-stat"><strong>${Number(semantic.finalEvidenceFailureCount || 0)}</strong><span>最终原声证据失败</span></div>
        <div class="study-stat"><strong>${Number(semantic.acceptedEvidenceSpanMismatchCount || 0)}</strong><span>已接纳证据定位不符</span></div>
      </div></div>
      <p>已关联到长期用户问题：<strong>${Number(latest.problemMembershipCount)}</strong> 条研究信号。</p>
    </article>`;
}

function targetRow(target) {
  const restrictionNote = sourceStateLabel[target.sourceState];
  const commentBlock = target.commentText
    ? `<blockquote>${esc(target.commentText)}</blockquote>`
    : `<p class="study-restricted">${esc(restrictionNote || '原文当前不可读取。')}</p>`;
  return `<tr>
    <td>${commentBlock}</td>
    <td>${esc(label(targetStateLabel, target.state) ?? target.state)}${target.exclusionReason ? `<p>${esc(target.exclusionReason)}</p>` : ''}</td>
    <td>${esc(label(contextStateLabel, target.contextState) ?? target.contextState)}</td>
    <td>${Number(target.signalCount)} 条${target.resolutionState ? `<p>${esc(label(resolutionLabel, target.resolutionState) ?? target.resolutionState)}</p>` : ''}</td>
  </tr>`;
}
async function renderTargetsTab() {
  if (!selectedRunRef) return '<p class="study-empty">尚无研究运行，先创建一次研究运行。</p>';
  const data = await get(`targets?runRef=${encodeURIComponent(selectedRunRef)}&limit=100`);
  if (!data.targets?.length) return '<p class="study-empty">这次运行没有冻结任何评论目标。</p>';
  return `<div class="study-review-table-wrap"><table class="study-review-table"><thead><tr><th scope="col">评论原声</th><th scope="col">处理状态</th><th scope="col">语境</th><th scope="col">研究信号</th></tr></thead><tbody>${data.targets.map(targetRow).join('')}</tbody></table></div>`;
}

function signalCard(signal) {
  const body = signal.sourceState === 'restricted'
    ? `<p class="study-restricted">来源已被限制，原声与摘要不再显示。</p>`
    : `<p class="study-signal-proposition">${esc(signal.proposition)}</p><blockquote>${esc(signal.evidence)}</blockquote>`;
  return `
    <article class="study-signal-card">
      <header><span class="study-badge">${esc(label(signalKindLabel, signal.kind) ?? signal.kind)}</span><span>${esc(label(resolutionLabel, signal.resolutionState) ?? '尚未进入归并判断')}</span></header>
      ${body}
      <p class="study-signal-meta">归并资格：${esc(label(eligibilityLabel, signal.eligibilityState) ?? signal.eligibilityState)}${signal.eligibilityReason ? ` · ${esc(signal.eligibilityReason)}` : ''}</p>
    </article>`;
}
async function renderPendingTab() {
  if (!selectedRunRef) return '<p class="study-empty">尚无研究运行，先创建一次研究运行。</p>';
  const data = await get(`signals?runRef=${encodeURIComponent(selectedRunRef)}&limit=100`);
  const pending = (data.signals || []).filter(signal => signal.resolutionState == null || PENDING_RESOLUTION_STATES.has(signal.resolutionState));
  return list(pending, signalCard, '当前没有待归并的研究信号。');
}

async function renderProblemsTab() {
  const data = await get('problems?limit=100');
  return list(data.problems, problem => `
    <article class="study-problem-card">
      <header><span>${esc(label(problemStateLabel, problem.state) ?? problem.state)}</span><span>关联研究信号 ${Number(problem.membershipCount)} 条</span></header>
      <p class="study-signal-proposition">${esc(problem.definition)}</p>
      <p class="study-signal-meta">纳入条件：${esc(JSON.stringify(problem.includeCriteria))}</p>
      <p class="study-signal-meta">排除条件：${esc(JSON.stringify(problem.excludeCriteria))}</p>
    </article>`, '尚无已建立的长期用户问题。');
}

function runRow(run) {
  return `<tr>
      <td>${esc(run.runRef.slice(0, 8))}…<p>${esc(run.createdAt)}</p></td>
      <td>${esc(label(targetStateLabel, run.state) ?? run.state)}</td>
      <td>${Number(run.workCount)}</td>
      <td>${Number(run.targetCount)}</td>
      <td>${Number(run.succeededCount)}</td>
      <td>${Number(run.noSignalCount)}</td>
      <td>${Number(run.needsContextCount)}</td>
      <td>${Number(run.failedCount)}</td>
      <td>${Number(run.excludedCount)}</td>
    </tr>`;
}
async function renderRunsTab() {
  const data = await get('runs?limit=50');
  if (!data.runs?.length) return '<p class="study-empty">尚未创建过研究运行。</p>';
  return `<div class="study-review-table-wrap"><table class="study-review-table"><thead><tr><th scope="col">运行</th><th scope="col">状态</th><th scope="col">作品</th><th scope="col">目标</th><th scope="col">已产出</th><th scope="col">无信号</th><th scope="col">等待语境</th><th scope="col">失败</th><th scope="col">来源受限</th></tr></thead><tbody>${data.runs.map(runRow).join('')}</tbody></table></div>`;
}

const TAB_RENDERERS = { overview: renderOverviewTab, targets: renderTargetsTab, pending: renderPendingTab, problems: renderProblemsTab, runs: renderRunsTab };

// A newer render can start (Tab click, run picker change) before an older one's fetch resolves.
// Without this token, a slow response from an abandoned render could overwrite whatever the
// user is looking at now with stale content. Only the render that is still current when its
// fetch resolves is allowed to touch the DOM.
let renderToken = 0;
async function renderActiveTab() {
  const container = document.querySelector('#study-tab-result');
  const token = ++renderToken;
  const view = activeView;
  container.setAttribute('aria-busy', 'true');
  let html;
  try {
    html = await TAB_RENDERERS[view]();
  } catch (error) {
    html = `<p class="study-empty">读取失败：${esc(error.message)}</p>`;
  }
  if (token !== renderToken) return;
  container.innerHTML = html;
  container.setAttribute('aria-busy', 'false');
}

async function loadProjection() {
  try {
    const runs = await get('runs?limit=50');
    allRuns = runs.runs || [];
    if (!selectedRunRef || !allRuns.some(run => run.runRef === selectedRunRef)) {
      selectedRunRef = allRuns[0]?.runRef ?? null;
    }
    renderRunPicker();
  } catch (error) { allRuns = []; }
  await renderActiveTab();
}
function highlightTab(view) {
  document.querySelectorAll('.study-tabs button').forEach(button => {
    if (button.dataset.view === view) button.setAttribute('aria-current', 'page');
    else button.removeAttribute('aria-current');
  });
}
async function switchToView(view) {
  if (view === activeView) return;
  activeView = view;
  highlightTab(view);
  renderRunPicker();
  await renderActiveTab();
}
document.querySelectorAll('.study-tabs button').forEach(button => button.addEventListener('click', () => switchToView(button.dataset.view)));
document.querySelector('#study-run-select').addEventListener('change', async event => {
  selectedRunRef = event.currentTarget.value || null;
  await renderActiveTab();
});
const studyDialog = document.querySelector('#study-dialog');
document.querySelector('#open-study-dialog').addEventListener('click', () => studyDialog.showModal());
document.querySelector('#study-dialog-close').addEventListener('click', () => studyDialog.close());
document.querySelector('#policy-form').addEventListener('submit', async event => {
  event.preventDefault();
  const button = document.querySelector('#save-policy');
  const status = document.querySelector('#policy-status');
  button.disabled = true;
  try {
    const response = await post('policy', { modelConfigRef: document.querySelector('#model-config').value, commentBudget: Number(document.querySelector('#comment-budget').value), contextCharacterBudget: Number(document.querySelector('#context-character-budget').value) });
    status.textContent = `研究策略已保存：${response.policyRef}`;
    await loadProjection();
  } catch (error) { status.textContent = `未保存策略：${error.message}`; }
  finally { button.disabled = !document.querySelector('#model-config').value; }
});
document.querySelector('#start-run').addEventListener('click', async () => {
  const button = document.querySelector('#start-run');
  const result = document.querySelector('#run-result');
  button.disabled = true;
  try {
    const response = await post('runs', { contentPublicRefs: selectedWorks() });
    result.textContent = `已创建 ${response.runRef}：覆盖 ${response.coveredWorkCount} 篇作品，冻结 ${response.targetCount} 条目标评论。尚未调用模型。`;
    selectedRunRef = response.runRef;
    studyDialog.close();
    activeView = 'runs';
    highlightTab('runs');
    await loadProjection();
  } catch (error) { result.textContent = `未创建研究运行：${error.message}`; }
  finally { updateSelection(); }
});
document.querySelector('#work-filter').addEventListener('input', renderWorks);
document.querySelector('#comment-budget').addEventListener('input', updateSelection);
document.querySelector('#select-visible-works').addEventListener('change', event => {
  visibleWorks().forEach(work => {
    if (event.currentTarget.checked) selectedWorkRefs.add(work.workRef);
    else selectedWorkRefs.delete(work.workRef);
  });
  renderWorks();
});
void Promise.all([loadSetup(), loadProjection()]);
