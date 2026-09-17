const endpoint = '/api/local/comment-study/';
const esc = value => String(value ?? '').replace(/[&<>\"]/g, character => ({
  '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;'
}[character]));

const get = async path => {
  const response = await fetch(endpoint + path, { headers: { Accept: 'application/json' } });
  if (!response.ok) throw new Error((await response.json().catch(() => ({}))).error || `HTTP ${response.status}`);
  return response.json();
};
const post = async (path, body) => {
  const response = await fetch(endpoint + path, { method: 'POST', headers: { Accept: 'application/json', 'Content-Type': 'application/json' }, body: JSON.stringify(body) });
  if (!response.ok) throw new Error((await response.json().catch(() => ({}))).error || `HTTP ${response.status}`);
  return response.json();
};
const list = (items, render, empty) => items?.length ? items.map(render).join('') : `<p class="muted">${esc(empty)}</p>`;
const selectedWorks = () => [...document.querySelectorAll('input[name="work-ref"]:checked')].map(input => input.value);

function updateSelection() {
  const count = selectedWorks().length;
  document.querySelector('#selected-count').textContent = `已选择 ${count} 篇`;
  document.querySelector('#start-run').disabled = count === 0;
}
function renderWorks(works) {
  const container = document.querySelector('#works');
  container.innerHTML = list(works, work => `<label class="work-option"><input type="checkbox" name="work-ref" value="${esc(work.workRef)}"><span><strong>${esc(work.title)}</strong><small>${esc(work.workRef)} · ${Number(work.eligibleCommentCount)} 条可研究评论</small></span></label>`, '当前没有符合条件的 ADHD 作品。');
  container.querySelectorAll('input').forEach(input => input.addEventListener('change', updateSelection));
  updateSelection();
}
async function loadSetup() {
  const status = document.querySelector('#setup-status');
  try {
    const setup = await get('setup');
    const select = document.querySelector('#model-config');
    select.innerHTML = list(setup.modelConfigs, config => `<option value="${esc(config.configRef)}">${esc(config.modelId)} · 输入 ${Number(config.inputTokenLimit)} / 输出 ${Number(config.outputTokenLimit)} tokens</option>`, '没有可用模型配置。');
    const available = setup.modelConfigs?.length > 0;
    select.disabled = !available;
    document.querySelector('#save-policy').disabled = !available;
    renderWorks(setup.eligibleWorks || []);
    status.textContent = available ? `已加载 ${setup.eligibleWorks?.length || 0} 篇可选作品` : '没有启用的模型配置，无法保存策略。';
  } catch (error) {
    status.textContent = `无法读取准备信息：${error.message}`;
    document.querySelector('#works').innerHTML = '<p class="muted">作品列表不可用。</p>';
  }
}
async function loadProjection() {
  try {
    const [overview, runs, problems] = await Promise.all([get('overview'), get('runs?limit=8'), get('problems?limit=8')]);
    const latest = overview.latestRun;
    const target = latest?.targetStates || {};
    document.querySelector('#states').innerHTML = overview.cleanLayerState === 'not_configured' ? '<span>尚未配置新研究策略</span>' : [`<span>运行：${esc(latest?.state || '尚无运行')}</span>`, `<span>目标：${Object.values(target).reduce((sum, value) => sum + Number(value), 0)}</span>`, `<span>无信号：${Number(target.no_signal || 0)}</span>`, `<span>等待语境：${Number(target.needs_context || 0)}</span>`].join('');
    document.querySelector('#runs').innerHTML = list(runs.runs, run => `<div class="item"><strong>${esc(run.state)}</strong><small>${esc(run.runRef)} · 目标 ${run.targetCount} · 已完成 ${run.succeededCount}</small></div>`, '尚无新 StudyRun。');
    document.querySelector('#problems').innerHTML = list(problems.problems, problem => `<div class="item"><strong>${esc(problem.definition)}</strong><small>${esc(problem.state)} · 证据 ${problem.membershipCount}</small></div>`, '尚无长期 Problem。');
    if (!latest?.runRef) { document.querySelector('#signals').innerHTML = '<p class="muted">尚无可读取的 Signal。</p>'; return; }
    const signals = await get(`signals?runRef=${encodeURIComponent(latest.runRef)}&limit=8`);
    document.querySelector('#signals').innerHTML = list(signals.signals, signal => `<div class="item"><strong>${esc(signal.proposition)}</strong><small>${esc(signal.eligibilityState)}${signal.resolutionState ? ` · ${esc(signal.resolutionState)}` : ''}</small></div>`, '本次运行没有 Signal。');
  } catch (error) { document.querySelector('#states').innerHTML = `<span>读取失败：${esc(error.message)}</span>`; }
}
document.querySelector('#policy-form').addEventListener('submit', async event => {
  event.preventDefault();
  const button = document.querySelector('#save-policy');
  const status = document.querySelector('#setup-status');
  button.disabled = true;
  try {
    const response = await post('policy', { modelConfigRef: document.querySelector('#model-config').value, commentBudget: Number(document.querySelector('#comment-budget').value), contextCharacterBudget: Number(document.querySelector('#context-character-budget').value) });
    status.textContent = `Study policy 已保存：${response.policyRef}`;
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
    await loadProjection();
  } catch (error) { result.textContent = `未创建 StudyRun：${error.message}`; }
  finally { updateSelection(); }
});
void Promise.all([loadSetup(), loadProjection()]);
