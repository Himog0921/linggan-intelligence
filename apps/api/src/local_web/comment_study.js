const endpoint = '/api/local/comment-study/';
let domainRef = new URLSearchParams(window.location.search).get('domain');
const domainName = document.body.dataset.domainName || '当前领域';
const esc = value => String(value ?? '').replace(/[&<>\"]/g, character => ({
  '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;'
}[character]));

const get = async path => {
  const url = new URL(endpoint + path, window.location.origin);
  if (domainRef) url.searchParams.set('domain', domainRef);
  const response = await fetch(url, { headers: { Accept: 'application/json' } });
  if (!response.ok) throw new Error(`请求失败（状态码 ${response.status}）`);
  return response.json();
};
const post = async (path, body, includeDomain = true) => {
  const response = await fetch(endpoint + path, { method: 'POST', headers: { Accept: 'application/json', 'Content-Type': 'application/json' }, body: JSON.stringify({ ...body, ...(includeDomain && domainRef ? { domainRef } : {}) }) });
  if (!response.ok) throw new Error(`请求失败（状态码 ${response.status}）`);
  return response.json();
};
const list = (items, render, empty) => items?.length ? items.map(render).join('') : `<p class="muted">${esc(empty)}</p>`;
let loadedWorks = [];
let sourcePreview = null;
const selectedWorkRoles = new Map();
const selectedWorkMeta = new Map();
const workCatalogState = { cursor: null, nextCursor: null, history: [], q: '', total: 0, coverage: null, observationRole: 'primary' };
let workSearchTimer = null;
let retryWorksCursor = null;
const MAX_SELECTED_WORKS = 100;
const MAX_SELECTED_COMMENTS = 3000;
const selectedCommentKeys = new Map();
let studySelectionKind = 'works';
const workTitleSourceLabel = { platform_title: '平台标题', cover_ocr: '封面 OCR', unknown: '标题未记录' };
const selectedWorks = () => [...selectedWorkRoles].map(([contentPublicRef, observationRole]) => ({ contentPublicRef, observationRole }));
const visibleWorks = () => loadedWorks;
let savedPolicies = [];
let activePolicyRef = null;
let pendingStartSignature = null;
let pendingStartRef = null;
let parentPolicyRef = null;
let runLimitsPolicyRef = null;

function updateSelection() {
  const count = studySelectionKind === 'comments' ? selectedCommentKeys.size : selectedWorkRoles.size;
  const selectedEligible = [...selectedWorkRoles.keys()].reduce((total, ref) =>
    total + Number(selectedWorkMeta.get(ref)?.eligibleCommentCount || 0), 0);
  const budget = Number(document.querySelector('#run-comment-budget').value || 0);
  const frozenCount = budget > 0 ? Math.min(selectedEligible, budget) : 0;
  document.querySelector('#selected-count').textContent = studySelectionKind === 'comments'
    ? `已选择 ${count}/${MAX_SELECTED_COMMENTS} 条评论 · 本次最多冻结 ${budget>0?Math.min(count,budget):0} 条`
    : count ? `已选择 ${count}/${MAX_SELECTED_WORKS} 篇 · 当前已知合格 ${selectedEligible} 条 · 本次最多冻结 ${frozenCount} 条`
    : `已选择 0 篇 · 最多 ${MAX_SELECTED_WORKS} 篇`;
  const hasPolicy = Boolean(document.querySelector('#study-policy').value);
  document.querySelector('#start-run').disabled = count === 0 || !hasPolicy;
  document.querySelector('#preview-run').disabled = count === 0 || !hasPolicy;
  const visible = visibleWorks();
  const selectVisible = document.querySelector('#select-visible-works');
  const selectedVisibleCount = visible.filter(work => selectedWorkRoles.get(work.workRef) === work.observationRole).length;
  selectVisible.checked = visible.length > 0 && selectedVisibleCount === visible.length;
  selectVisible.indeterminate = selectedVisibleCount > 0 && selectedVisibleCount < visible.length;
  selectVisible.disabled = selectedWorkRoles.size >= MAX_SELECTED_WORKS && selectedVisibleCount < visible.length;
}

function renderSourcePreview(preview, targetId, roleLabel) {
  const target = document.querySelector(targetId);
  if (!target) return;
  if (!preview) { target.textContent = `${roleLabel}来源资格统计暂不可用。`; return; }
  const excluded = preview.excludedCounts || {};
  const labels = { workAuthorUnknown:'作品作者身份未知',commentAuthorUnknown:'评论作者身份未知，不纳入研究',creatorVoice:'作品作者本人',bodyUnavailable:'正文不可研究',sourceRestricted:'来源受限',textNotResearchable:'文本不具研究条件' };
  const reasons = Object.entries(labels).map(([key,label])=>[label,Number(excluded[key]||0)]).filter(([,count])=>count>0).map(([label,count])=>`${label} ${count} 条`);
  const excludedCount=Number(preview.totalCommentCount||0)-Number(preview.eligibleCommentCount||0);
  const unknownAuthors=Number(preview.unknownAuthorCount||0);
  target.textContent=`${roleLabel} · 截至 ${String(preview.asOf||'').replace('T',' ')}：共 ${Number(preview.totalCommentCount||0)} 条评论；可研究 ${Number(preview.eligibleCommentCount||0)} 条；未纳入 ${excludedCount} 条${unknownAuthors&&!Number(excluded.commentAuthorUnknown||0)?`；评论作者身份未知 ${unknownAuthors} 条，不纳入研究`:''}${reasons.length?`（${reasons.join('；')}）`:''}。`;
}
function workCatalogPath(cursor=null){
  const query=new URLSearchParams({domain:domainRef,observationRole:workCatalogState.observationRole,limit:'50'});
  if(workCatalogState.q)query.set('q',workCatalogState.q);
  if(cursor)query.set('cursor',cursor);
  return `works?${query}`;
}
function renderWorks(){
  const container=document.querySelector('#works');
  const coverage=workCatalogState.coverage||{};
  const pending=coverage.pendingCount==null?'未知':Number(coverage.pendingCount);
  const roleLabel=workCatalogState.observationRole==='primary'?'primary':'reference';
  document.querySelector('#work-filter-status').textContent=`服务端作品目录（${roleLabel}）：当前页 ${loadedWorks.length} 篇 / 匹配 ${workCatalogState.total} 篇；已索引评论 ${Number(coverage.indexedCount||0)}，待索引 ${pending}。`;
  document.querySelector('#work-page-status').textContent=`第 ${workCatalogState.history.length+1} 页`;
  document.querySelector('#work-prev').disabled=workCatalogState.history.length===0;
  document.querySelector('#work-next').disabled=!workCatalogState.nextCursor;
  container.innerHTML=loadedWorks.length?loadedWorks.map(work=>{
    const workRef=String(work.workRef);
    const role=work.observationRole||workCatalogState.observationRole;
    const checked=selectedWorkRoles.get(workRef)===role;
    const alreadySelected=selectedWorkRoles.has(workRef);
    const capped=!alreadySelected&&selectedWorkRoles.size>=MAX_SELECTED_WORKS;
    const title=work.displayTitle||'未命名作品';
    return `<tr><td><input id="work-${esc(workRef)}" type="checkbox" name="work-ref" value="${esc(workRef)}" aria-label="选择作品：${esc(title)}"${checked?' checked':''}${capped?' disabled':''}></td><td><label for="work-${esc(workRef)}"><span class="study-work-title">${esc(title)}</span><small>${esc(workTitleSourceLabel[work.displayTitleSource]||work.displayTitleSource||'标题未记录')}</small></label></td><td>${esc(role)}</td><td>${Number(work.eligibleCommentCount||0)}</td></tr>`;
  }).join(''):'<tr><td class="study-table-empty" colspan="4">当前搜索没有匹配作品。</td></tr>';
  container.querySelectorAll('input[name="work-ref"]').forEach(input=>input.addEventListener('change',event=>{
    const work=loadedWorks.find(item=>String(item.workRef)===event.currentTarget.value);
    if(!work)return;
    const role=work.observationRole||workCatalogState.observationRole;
    if(event.currentTarget.checked){
      if(!selectedWorkRoles.has(work.workRef)&&selectedWorkRoles.size>=MAX_SELECTED_WORKS){event.currentTarget.checked=false;document.querySelector('#work-filter-status').textContent=`最多选择 ${MAX_SELECTED_WORKS} 篇作品；已保留原选择。`;return;}
      selectedWorkRoles.set(work.workRef,role);
      selectedWorkMeta.set(work.workRef,{eligibleCommentCount:Number(work.eligibleCommentCount||0)});
    }else if(selectedWorkRoles.get(work.workRef)===role){selectedWorkRoles.delete(work.workRef);selectedWorkMeta.delete(work.workRef);}
    renderWorks();
  }));
  updateSelection();
}
async function loadWorksPage(cursor=null){
  if(!domainRef)return;
  retryWorksCursor=cursor;
  try{
    const data=await get(workCatalogPath(cursor));
    loadedWorks=(data.items||[]).map(item=>({...item,observationRole:item.observationRole||workCatalogState.observationRole}));
    workCatalogState.cursor=cursor;
    workCatalogState.nextCursor=data.page?.nextCursor||null;
    workCatalogState.total=Number(data.totalWorkCount||0);
    workCatalogState.coverage=data.indexCoverage||null;
    renderWorks();
    return true;
  }catch(error){
    loadedWorks=[];workCatalogState.nextCursor=null;
    document.querySelector('#work-filter-status').textContent=`作品目录加载失败：${error.message}。可重试或调整搜索条件。`;
    document.querySelector('#work-page-status').textContent=`第 ${workCatalogState.history.length+1} 页`;
    document.querySelector('#work-prev').disabled=workCatalogState.history.length===0;
    document.querySelector('#work-next').disabled=true;
    document.querySelector('#works').innerHTML=`<tr><td class="study-table-empty" colspan="4">作品列表暂不可用：${esc(error.message)} <button id="retry-work-catalog" type="button">重试</button></td></tr>`;
    document.querySelector('#retry-work-catalog').addEventListener('click',event=>{event.currentTarget.disabled=true;void loadWorksPage(retryWorksCursor);});
    updateSelection();
    return false;
  }
}
async function searchWorksNow(){
  workCatalogState.q=document.querySelector('#work-filter').value.trim();
  workCatalogState.cursor=null;workCatalogState.history=[];
  await loadWorksPage(null);
}
async function loadSetup(){
  const status=document.querySelector('#setup-status');
  if(!domainRef){
    status.dataset.kind='info';
    status.textContent='请先在页头选择观察领域。';
    document.querySelector('#open-study-dialog').disabled=true;
    return;
  }
  try{
    const setup=await get('setup');domainRef=setup.domainRef;
    const select=document.querySelector('#model-config');
    select.innerHTML=list(setup.modelConfigs,config=>`<option value="${esc(config.configRef)}">${esc(config.modelId)} · 输入上限 ${Number(config.inputTokenLimit)} 词元／输出上限 ${Number(config.outputTokenLimit)} 词元</option>`,'没有可用模型配置。');
    const available=setup.modelConfigs?.length>0&&setup.domainStatus==='active';
    select.disabled=!available;document.querySelector('#save-policy').disabled=!available;
    sourcePreview=setup.sourcePreview||null;
    renderSourcePreview(sourcePreview,'#source-preview','primary');
    renderSourcePreview(setup.referenceSourcePreview,'#reference-source-preview','reference');
    document.querySelector('#reference-source-preview').hidden=Number(setup.referenceSourcePreview?.totalCommentCount||0)===0;
    selectedWorkRoles.clear();selectedWorkMeta.clear();workCatalogState.q='';workCatalogState.history=[];workCatalogState.cursor=null;workCatalogState.observationRole='primary';
    document.querySelector('#work-filter').value='';
    const referenceToggle=document.querySelector('#include-reference-works');referenceToggle.checked=false;
    referenceToggle.disabled=setup.domainStatus!=='active'||Number(setup.referenceSourcePreview?.totalCommentCount||0)===0;
    const [worksResult,policiesResult]=await Promise.allSettled([loadWorksPage(null),loadPolicies()]);
    const worksLoaded=worksResult.status==='fulfilled'&&worksResult.value;
    if(policiesResult.status==='rejected')document.querySelector('#policy-status').textContent=`研究方法列表加载失败：${policiesResult.reason.message}`;
    status.dataset.kind=available?'info':'error';
    status.textContent=setup.domainStatus==='paused'?`${domainName}已暂停；历史结果可读，不能创建新策略或运行。`:available?(worksLoaded?`已加载 ${workCatalogState.total} 篇 primary 作品；可显式切换查看 reference 作品。`:'准备信息已加载；作品目录暂时不可用，请在下方重试。'):'没有启用的模型配置，无法保存策略。';
  }catch(error){
    status.dataset.kind='error';status.textContent=`无法读取准备信息：${error.message}`;
    document.querySelector('#work-filter-status').textContent='作品目录不可用。';
    sourcePreview=null;renderSourcePreview(null,'#source-preview','primary');renderSourcePreview(null,'#reference-source-preview','reference');
    document.querySelector('#works').innerHTML='<tr><td class="study-table-empty" colspan="4">作品目录不可用。</td></tr>';
  }
}

async function loadPolicies(preferredRef=null){
  const select=document.querySelector('#study-policy');
  const firstPage=await get('policies?limit=100');
  let result=firstPage;
  let policyItems=firstPage.items||[];
  activePolicyRef=policyItems.find(item=>item.isActive)?.policyRef||null;
  while(!activePolicyRef&&result.page?.hasMore){
    const cursor=result.page.nextCursor;
    if(!cursor)throw new Error('方法目录分页回执不完整');
    result=await get(`policies?limit=100&cursor=${encodeURIComponent(cursor)}`);
    policyItems=result.items||[];
    activePolicyRef=policyItems.find(item=>item.isActive)?.policyRef||null;
  }
  savedPolicies=(firstPage.items||[]).filter(item=>item.recordingState==='recorded');
  select.innerHTML=savedPolicies.length?savedPolicies.map(item=>`<option value="${esc(item.policyRef)}">${esc(item.methodName||'未命名方法')}${item.isActive?' · 默认':''} · ${esc(item.policyRef.slice(0,8))}</option>`).join(''):'<option value="">没有已记录的方法版本</option>';
  select.disabled=savedPolicies.length===0;
  const isRecorded=reference=>savedPolicies.some(item=>item.policyRef===reference);
  const desired=(isRecorded(preferredRef)?preferredRef:null)
    ||(isRecorded(activePolicyRef)?activePolicyRef:null)
    ||savedPolicies[0]?.policyRef||'';
  if(desired)select.value=desired;
  document.querySelector('#activate-policy').disabled=!select.value||select.value===activePolicyRef;
  updatePolicySummary();
  document.querySelector('#edit-policy').disabled=!document.querySelector('#model-config').value;
  if(!savedPolicies.length&&document.querySelector('#model-config').value)await openPolicyEditor();
  updateSelection();
}

function updatePolicySummary(){
  const reference=document.querySelector('#study-policy').value;
  const policy=savedPolicies.find(item=>item.policyRef===reference);
  const summary=document.querySelector('#selected-policy-summary');
  const button=document.querySelector('#edit-policy');
  button.textContent=policy?'编辑方法':'新建方法';
  summary.textContent=policy
    ?`${policy.methodName||'未命名方法'} · 版本 ${String(policy.policyRef).slice(0,8)} · 评论上限 ${Number(policy.defaults?.commentBudget||0)} 条 · 语境上限 ${Number(policy.defaults?.contextCharacterBudget||0)} 字符${policy.isActive?' · 当前默认':''}`
    :'还没有已保存的方法版本。首次使用前需要创建一版研究方法。';
  if(reference!==runLimitsPolicyRef){
    document.querySelector('#run-comment-budget').value=String(policy?.defaults?.commentBudget||100);
    document.querySelector('#run-context-character-budget').value=String(policy?.defaults?.contextCharacterBudget||6000);
    runLimitsPolicyRef=reference;
  }
}

function instructionExtra(instruction){
  const match=String(instruction||'').match(/<stage-instructions>\n([\s\S]*?)\n<\/stage-instructions>$/);
  return match?match[1]:null;
}

function closePolicyEditor(){
  document.querySelector('#policy-form').hidden=true;
  document.querySelector('#study-policy').disabled=savedPolicies.length===0;
  parentPolicyRef=null;
  document.querySelector('#edit-policy').disabled=!document.querySelector('#model-config').value;
}

async function openPolicyEditor(){
  const form=document.querySelector('#policy-form');
  const button=document.querySelector('#edit-policy');
  const status=document.querySelector('#policy-status');
  const editableControls=[...form.querySelectorAll('input,select,textarea,button[type="submit"]')];
  const reference=document.querySelector('#study-policy').value;
  if(!document.querySelector('#model-config').value)return;
  form.hidden=false;button.disabled=true;
  document.querySelector('#study-policy').disabled=true;
  editableControls.forEach(control=>control.disabled=true);
  status.textContent=reference?'正在读取所选方法版本…':'创建第一版研究方法。';
  try{
    parentPolicyRef=reference||null;
    document.querySelector('#method-name').value='评论研究方法';
    document.querySelector('#comment-budget').value='100';
    document.querySelector('#context-character-budget').value='6000';
    document.querySelector('#stage-semantic').value='';
    document.querySelector('#stage-resolution').value='';
    document.querySelector('#stage-pair').value='';
    if(reference){
      const response=await get(`policies/${encodeURIComponent(reference)}`);
      const policy=response.policy||{};
      const manifest=policy.methodManifest;
      if(!manifest?.stages)throw new Error('所选方法没有可编辑的完整版本记录');
      const semantic=instructionExtra(manifest.stages.semantic?.systemInstruction);
      const resolution=instructionExtra(manifest.stages.resolution?.systemInstruction);
      const pair=instructionExtra(manifest.stages.pair?.systemInstruction);
      if([semantic,resolution,pair].some(value=>value===null))throw new Error('无法安全还原此版本的补充说明；原版本未更改');
      document.querySelector('#method-name').value=policy.methodName||'评论研究方法';
      document.querySelector('#comment-budget').value=String(policy.defaults?.commentBudget||100);
      document.querySelector('#context-character-budget').value=String(policy.defaults?.contextCharacterBudget||6000);
      document.querySelector('#stage-semantic').value=semantic;
      document.querySelector('#stage-resolution').value=resolution;
      document.querySelector('#stage-pair').value=pair;
      const configuredModel=manifest.modelConfigRef;
      if([...document.querySelector('#model-config').options].some(option=>option.value===configuredModel))document.querySelector('#model-config').value=configuredModel;
      status.textContent='编辑副本已载入；保存会生成新的不可变方法版本，原版本与已运行研究保持不变。';
    }
    document.querySelector('#save-policy').disabled=false;
  }catch(error){
    status.textContent=`无法打开方法编辑：${error.message}`;
    closePolicyEditor();
  }finally{editableControls.forEach(control=>control.disabled=false);button.disabled=false;}
}

const targetStateLabel = {
  ready: '准备就绪', needs_context: '等待语境', excluded: '来源受限，未处理', queued: '排队中',
  running: '处理中', succeeded: '已产出结果', no_signal: '已处理 · 无信号', failed: '处理失败'
};
const observationRoleLabel = { primary: 'primary · 本领域', reference: 'reference · 参考领域' };
const eligibilityLabel = {
  eligible: '具备归并资格', deferred_context: '语境不足，暂缓', not_user_problem: '不构成用户问题',
  not_applicable: '不适用（非问题/需求类信号）'
};
const resolutionLabel = {
  pending: '待归并判断', assigned: '已归入用户问题', deferred_context: '语境不足，继续等待',
  deferred_ambiguous: '归并边界尚未确定', deferred_novel: '新表达，尚未建档',
  retrieval_incomplete: '候选目录未查全，当前不能判定是否为新问题',
  budget_stopped: '归并预算已到上限，当前未完成判断',
  not_user_problem: '判定不构成用户问题', protocol_rejected: '模型输出不合规，已拒绝', failed: '归并判断失败'
};
const pairStateLabel = {
  pending: '正在比较首个合格候选', approved: '已共同建立用户问题', rejected: '未共同建立用户问题',
  failed: '比较未完成，请查看原因'
};
const pairDecisionLabel = {
  approved: '两条独立证据支持同一用户问题，已建立问题',
  not_same_problem: '关键维度不同，当前不是同一用户问题',
  ambiguous: '当前证据不足以可靠判断是否为同一问题',
  insufficient_independent_evidence: '独立证据条件不足，未建立问题',
  contract_rejected_json_schema: '模型输出结构不符合约定，未接纳',
  contract_rejected_contract: '模型输出合同版本不符合约定，未接纳',
  contract_rejected_candidate_set_mismatch: '模型返回的比较对象不符合约定，未接纳',
  contract_rejected_invalid_verdict: '模型返回的维度判断无效，未接纳',
  contract_rejected_invalid_problem_definition: '模型给出的问题定义不完整，未接纳',
  contract_rejected_pair_signal_mismatch: '模型返回的研究信号不对应当前配对，未接纳',
  attempts_exhausted: '请求尝试次数已用尽，未收到可接纳结果',
  budget_exhausted: '运行预算已用尽，比较未完成',
  input_limit_exceeded: '请求超过模型输入上限，未发送'
};
const signalKindLabel = {
  problem: '问题', need: '需求', belief: '观念', emotion: '情绪', experience: '经历',
  solution: '解决方案', quote: '引述', context: '语境', question: '疑问'
};
const problemStateLabel = { active: '生效中', retired: '已停用' };
const contextStateLabel = { ready: '语境完整', partial: '语境部分（有截断）', missing: '缺少语境' };
const dependencyStateLabel = { self_contained: '不依赖父评论', parent_available: '父评论已冻结', parent_required_missing: '判定缺少父评论' };
const sourceStateLabel = { known: null, restricted: '来源已被限制，原文不再显示', unknown: '原文未知（来源未采集到正文）' };
const runStateLabel = {
  queued: '排队中', running: '处理中', completed: '已完成',
  completed_with_failures: '已结束 · 有目标未完成', cancelled: '已停止'
};
const requestStageLabel = { semantic: '评论语义提取', resolution: '问题归并', pair: '独立证据配对' };
const modelRequestStateLabel = { running: '处理中', succeeded: '已完成', failed: '处理失败' };
const dispatchStateLabel = { enabled: '允许继续派发', paused: '已暂停派发', stopped: '已停止派发' };
const dispatchReasonLabel = {
  user_paused: '用户暂停', user_stopped: '用户停止', budget_exhausted: '预算已用尽',
  legacy_unrecorded: '历史方法未记录', upgrade_guard: '升级保护', method_unavailable: '方法不可用'
};
let stopDialogRunRef = null;
let stopDialogControlVersion = null;
let recoveryRunRef = null;
const recoveryRequestRefs = new Map();
let runControlFeedback = '';
let runControlOutcomeNeedsRefresh = false;
const label = (map, value) => (value == null ? null : (map[value] ?? '未知状态'));
let allRuns = [];
let runListNextCursor = null;
let runListLoaded = false;
let runListLoading = false;
let runListRequest = null;
let runListGeneration = 0;
const validStudyViews = new Set(['overview', 'comments', 'runs', 'problems']);
const initialStudyRoute = new URLSearchParams(window.location.search);
let activeView = initialStudyRoute.get('view') === 'targets' ? 'runs' : initialStudyRoute.get('view')==='pending'?'problems':validStudyViews.has(initialStudyRoute.get('view')) ? initialStudyRoute.get('view') : 'overview';
let selectedRunRef = initialStudyRoute.get('runRef');
const RUN_PANELS = new Set(['targets', 'signals', 'requests', 'method']);
let selectedRunPanel = initialStudyRoute.get('view') === 'targets' ? 'targets' : RUN_PANELS.has(initialStudyRoute.get('panel')) ? initialStudyRoute.get('panel') : 'targets';
let focusedTargetRef = initialStudyRoute.get('targetRef');
let focusedSignalRef = initialStudyRoute.get('signalRef');
let selectedProblemRef = initialStudyRoute.get('problemRef');
const problemListCache={items:[],nextCursor:null,loaded:false,hasMore:false};
const problemCandidateState={state:'all',items:[],nextCursor:null,loaded:false,asOf:null,error:null};
const RUN_SCOPED_VIEWS = new Set(['runs']);
const runPanelCache = {
  targets: { runRef: null, items: [], nextCursor: null, loaded: false },
  signals: { runRef: null, items: [], nextCursor: null, loaded: false },
  requests: { runRef: null, items: [], nextCursor: null, loaded: false },
};
let runPanelGeneration = 0;

function syncStudyRoute(push = false) {
  const url = new URL(window.location.href);
  url.searchParams.set('view', activeView);
  if (activeView === 'runs') {
    if (selectedRunRef) url.searchParams.set('runRef', selectedRunRef);
    else url.searchParams.delete('runRef');
    url.searchParams.set('panel', selectedRunPanel);
    if (focusedTargetRef) url.searchParams.set('targetRef', focusedTargetRef);
    else url.searchParams.delete('targetRef');
    if (focusedSignalRef) url.searchParams.set('signalRef', focusedSignalRef);
    else url.searchParams.delete('signalRef');
  } else {
    url.searchParams.delete('runRef');
    url.searchParams.delete('panel');
    url.searchParams.delete('targetRef');
    url.searchParams.delete('signalRef');
  }
  if (activeView === 'problems' && selectedProblemRef) url.searchParams.set('problemRef', selectedProblemRef);
  else url.searchParams.delete('problemRef');
  if (activeView === 'comments') {
    for (const [key,value] of [['q',commentCatalogState.q],['workRef',commentCatalogState.workRef],['state',commentCatalogState.studyState==='all'?null:commentCatalogState.studyState],['cursor',commentCatalogState.cursor],['voiceRole',commentCatalogState.voiceRole==='reader_and_unknown'?null:commentCatalogState.voiceRole]]) {
      if (value) url.searchParams.set(key,value); else url.searchParams.delete(key);
    }
  } else {
    for (const key of ['q','workRef','state','cursor','voiceRole']) url.searchParams.delete(key);
  }
  if (activeView !== 'comments') {
    url.searchParams.delete('commentWorkRef');
    url.searchParams.delete('commentExternalId');
    url.searchParams.delete('detail');
  }
  const state={...(window.history.state||{}),commentDetail:activeView==='comments'&&url.searchParams.has('commentExternalId'),commentFilter:captureCommentFilter()};
  window.history[push ? 'pushState' : 'replaceState'](state, '', url);
}

function resetRunPanelCache() {
  runPanelGeneration += 1;
  for (const state of Object.values(runPanelCache)) {
    state.runRef = null;
    state.items = [];
    state.nextCursor = null;
    state.loaded = false;
  }
}

function runOptionLabel(run) {
  return `${run.runRef.slice(0, 8)}… · ${esc(runStateLabel[run.state] || run.state)} · ${run.createdAt.slice(0, 16).replace('T', ' ')}`;
}
function renderRunPicker() {
  const picker = document.querySelector('#study-run-picker');
  const select = document.querySelector('#study-run-select');
  picker.hidden = !RUN_SCOPED_VIEWS.has(activeView);
  if (!RUN_SCOPED_VIEWS.has(activeView)) return;
  if (!allRuns.length) {
    select.innerHTML = selectedRunRef
      ? `<option value="${esc(selectedRunRef)}" selected>已打开运行 ${esc(selectedRunRef.slice(0, 8))}…</option>`
      : '<option value="">尚无研究运行</option>';
    select.disabled = !selectedRunRef;
    return;
  }
  select.disabled = false;
  const selectedNotLoaded = selectedRunRef && !allRuns.some(run => run.runRef === selectedRunRef)
    ? `<option value="${esc(selectedRunRef)}" selected>已打开运行 ${esc(selectedRunRef.slice(0, 8))}…（可在下方列表加载更早运行）</option>`
    : '';
  select.innerHTML = selectedNotLoaded + allRuns.map(run => `<option value="${esc(run.runRef)}"${run.runRef === selectedRunRef ? ' selected' : ''}>${runOptionLabel(run)}</option>`).join('');
}

function targetRow(target) {
  const restrictionNote = target.parentContext?.state === 'restricted'
    ? '父评论语境受限，本条及衍生内容已隐藏。'
    : sourceStateLabel[target.sourceState];
  const sourceReadable = target.sourceState === 'known' && target.parentContext?.state !== 'restricted';
  const commentBlock = sourceReadable && target.commentText
    ? `<blockquote>${esc(target.commentText)}</blockquote>`
    : `<p class="study-restricted">${esc(restrictionNote || '原文当前不可读取。')}</p>`;
  const workSources = target.workContext?.sources || [];
  const workContext = !sourceReadable
    ? '<p class="study-restricted">来源当前受限，冻结语境不再显示。</p>'
    : workSources.length
    ? `<ul class="study-context-sources">${workSources.map(source => `<li><span>${esc(source.kind || '作品语境')}</span><blockquote>${esc(source.text || '')}</blockquote></li>`).join('')}</ul>`
    : `<p class="study-detail-muted">${esc(label(contextStateLabel, target.contextState) ?? target.contextState)}；本次运行没有保存可展示的语境片段。</p>`;
  const parent = target.parentContext || { state: 'none' };
  const parentLabel = { none: '根评论，没有父评论关系', not_included: '本条是回复，但本次运行没有冻结父评论', missing: '父评论关系存在，但冻结时未读取到可用原文', restricted: '父评论来源当前受限，原文已隐藏' };
  const parentContext = sourceReadable && parent.state === 'available'
    ? `<blockquote>${esc(parent.researchText || '')}</blockquote><p class="study-detail-muted">父评论只作为解释语境，不作为本条评论的 Signal 证据。</p>`
    : `<p class="study-detail-muted">${esc(parentLabel[parent.state] || '父评论语境状态未知')}</p>`;
  const attempt = target.latestAttempt || null;
  const modelReason = sourceReadable && target.modelReason
    ? `<section><h4>模型给出的原因</h4><p>${esc(target.modelReason)}</p></section>`
    : '';
  const failureCode = attempt?.providerFailureCode || attempt?.rejectionCode || target.terminalReason;
  const recovery = target.state === 'needs_context'
    ? (parent.state === 'none' && target.dependencyState === 'parent_required_missing'
      ? '这条是根评论，没有父评论关系；旧规则把短文本错误标成缺父评论。可从源 Run 显式补跑。'
      : parent.state === 'not_included'
      ? '这次 Run 漏带了父评论；修正输入组装后可从源 Run 显式补跑。'
      : parent.state === 'missing'
        ? '当前没有可用父评论原文；可先补齐来源输入再显式补跑。相同输入不会反复重试。'
        : !target.modelReason
          ? '目标在进入模型前就被判定缺语境；先核对冻结的作品语境和依赖判定。'
          : '模型拿到了当前冻结文本但仍无法确认指代；只有补充了有效语境后才值得重跑。')
    : ['response_too_large', 'output_limit'].includes(failureCode)
      ? (attempt?.retryStrategy === 'split_single_target'
        ? '大批次已改为单目标处理；单条仍超过响应上限时会停止，不会原样重复请求。'
        : '响应超过 adapter 上限；系统会避免原样重试，按单目标恢复或给出终态原因。')
      : ['authentication_failed', 'provider_authentication_failed', 'model_secret_unavailable'].includes(failureCode)
        ? '先修复模型连接凭证；固定配置错误不会在同一 Run 内重复尝试。'
        : target.state === 'failed'
          ? '根据下方故障码先处理输入、连接或响应问题，再创建续做运行。'
          : '查看本次冻结输入和研究结果。';
  const usage = attempt?.usageKnown
    ? `已知用量：输入 ${Number(attempt.inputTokens)} · 输出 ${Number(attempt.outputTokens)} · 计费 ${Number(attempt.chargedTokens)} Token`
    : attempt?.reservedTokens != null
      ? `用量未知；按预留额度 ${Number(attempt.reservedTokens)} Token 保守记账（计入 ${Number(attempt.chargedTokens || 0)} Token）`
      : '模型用量记录不可用';
  const responseLimit = { sse_stream_262144: 'SSE 响应流超过 262,144 bytes', final_text_65536: '最终文本超过 65,536 bytes', output_tokens: '模型输出词元上限' }[attempt?.responseLimit];
  const diagnostics = attempt
    ? `<details class="study-target-diagnostics"><summary>技术诊断</summary><p>尝试 ${Number(target.attemptCount || 0)} 次 · ${esc(attempt.state || '状态未知')}${attempt.rejectionCode ? ` · 拒绝码 ${esc(attempt.rejectionCode)}` : ''}${attempt.providerFailureCode ? ` · 调用错误 ${esc(attempt.providerFailureCode)}` : ''}${attempt.retryStrategy ? ` · 后续策略 ${esc(attempt.retryStrategy)}` : ''}</p><p>阶段 ${esc(attempt.stage || '未记录')} · HTTP ${esc(attempt.httpStatus ?? '未记录')} · 接收 ${esc(attempt.receivedBytes ?? '未记录')} bytes · 收到结束事件 ${attempt.terminalReceived == null ? '未知' : (attempt.terminalReceived ? '是' : '否')}${responseLimit ? ` · ${esc(responseLimit)}` : ''}</p><p>${esc(usage)}${target.terminalReason ? ` · 终态 ${esc(target.terminalReason)}` : ''}</p></details>`
    : `<p class="study-detail-muted">没有模型调用记录（尝试 ${Number(target.attemptCount || 0)} 次）。</p>`;
  const detail = `<details class="study-target-detail"><summary>查看本次输入、原因与处理建议</summary><div class="study-target-detail-body"><section><h4>本条原声与清洗文本</h4>${!sourceReadable ? '<p class="study-restricted">来源已受限，原声与清洗文本不再显示。</p>' : `<p>${esc(target.researchText || target.commentText || '没有可展示的清洗文本')}</p>`}</section><section><h4>Run 冻结的作品语境</h4>${workContext}</section><section><h4>Run 冻结的父评论语境</h4>${parentContext}</section>${modelReason}<section><h4>下一步</h4><p>${esc(recovery)}</p></section>${diagnostics}</div></details>`;
  return `<tr data-target-ref="${esc(target.targetRef)}" data-focus="${focusedTargetRef === target.targetRef}">
    <td><span class="study-badge">${esc(observationRoleLabel[target.observationRole] || '来源角色未知')}</span>${commentBlock}</td>
    <td>${esc(label(targetStateLabel, target.state) ?? target.state)}${target.exclusionReason ? `<p>${esc(target.exclusionReason)}</p>` : ''}${target.terminalReason ? `<p>${esc(target.terminalReason)}</p>` : ''}${target.commentKey?.commentExternalId?`<p><button type="button" class="study-link" data-target-comment-work="${esc(target.commentKey.workRef)}" data-target-comment-id="${esc(target.commentKey.commentExternalId)}">查看评论详情</button></p>`:''}${detail}</td>
    <td>${esc(label(contextStateLabel, target.contextState) ?? target.contextState)}<p>${esc(dependencyStateLabel[target.dependencyState] || target.dependencyState || '评论依赖未记录')}</p></td>
    <td>${Number(target.signalCount)} 条${target.resolutionState ? `<p>${esc(label(resolutionLabel, target.resolutionState) ?? target.resolutionState)}</p>` : ''}${Number(target.signalCount)>0?`<p><button type="button" class="study-link" data-target-signals="${esc(target.targetRef)}">查看相关信号</button></p>`:''}</td>
  </tr>`;
}
function renderTargetsPanel(targets) {
  if (!targets?.length) return '<p class="study-empty">这次运行没有冻结任何评论目标。</p>';
  return `<div class="study-review-table-wrap"><table class="study-review-table"><thead><tr><th scope="col">评论原声</th><th scope="col">处理状态与输入说明</th><th scope="col">语境与依赖</th><th scope="col">研究信号</th></tr></thead><tbody>${targets.map(targetRow).join('')}</tbody></table></div>`;
}

const commentCatalogState={q:'',studyState:'all',voiceRole:'reader_and_unknown',workRef:null,workLabel:'',cursor:null,history:[],response:null,summary:null,workChoices:[]};
const COMMENT_STATES=new Set(['all','never_studied','in_progress','studied','needs_context','failed']);
const COMMENT_VOICES=new Set(['reader_and_unknown','reader','unknown','creator','all']);
function restoreCommentRoute(params,snapshot){
  restoreCommentFilter(snapshot);
  commentCatalogState.q=params.get('q')||'';
  commentCatalogState.workRef=params.get('workRef')||null;
  const state=params.get('state');commentCatalogState.studyState=COMMENT_STATES.has(state)?state:'all';
  const voice=params.get('voiceRole');commentCatalogState.voiceRole=COMMENT_VOICES.has(voice)?voice:'reader_and_unknown';
  commentCatalogState.cursor=params.get('cursor')||null;
  if(!snapshot||snapshot.cursor!==commentCatalogState.cursor)commentCatalogState.history=[];
}
function captureCommentFilter(){const {q,studyState,voiceRole,workRef,workLabel,cursor,history,workChoices}=commentCatalogState;return{q,studyState,voiceRole,workRef,workLabel,cursor,history:[...history],workChoices:[...workChoices]};}
function restoreCommentFilter(snapshot){if(!snapshot)return;Object.assign(commentCatalogState,snapshot,{history:[...(snapshot.history||[])],workChoices:[...(snapshot.workChoices||[])]});}
if(activeView==='comments')restoreCommentRoute(initialStudyRoute,window.history.state?.commentFilter);
function catalogQuery(path,extra={}){
  const query=new URLSearchParams({domain:domainRef});
  Object.entries(extra).forEach(([key,value])=>{if(value!=null&&value!=='')query.set(key,String(value));});
  return `${path}?${query}`;
}
function resetCommentPage(){commentCatalogState.cursor=null;commentCatalogState.history=[];}
function commentStatus(item){
  if(!item.latestStudy)return'尚未研究';
  const latest=label(targetStateLabel,item.latestStudy.state)??item.latestStudy.state;
  if(item.effectiveStudy&&item.latestStudy.targetRef!==item.effectiveStudy.targetRef){const effective=label(targetStateLabel,item.effectiveStudy.state)??item.effectiveStudy.state;return`${latest}；历史有效结果：${effective}`;}
  return latest;
}
function commentEligibility(item){
  if(item.studyEligibility?.eligible)return'可研究';
  const map={sourceRestricted:'来源受限',bodyUnavailable:'正文不可用',indexPending:'等待本地清洗',textNotResearchable:'纯无效文本',workAuthorUnknown:'作品作者身份未知',commentAuthorUnknown:'评论作者身份未知',creatorVoice:'作品作者声音'};
  return(item.studyEligibility?.reasons||[]).map(reason=>map[reason]||reason).join('；')||'暂不可研究';
}
function commentSelectionKey(commentKey){return `${commentKey.workRef}\u0000${commentKey.commentExternalId}`;}
function selectedCommentWorkCount(){return new Set([...selectedCommentKeys.values()].map(item=>item.commentKey.workRef)).size;}
function updateCommentSelectionView(){
  const count=selectedCommentKeys.size,works=selectedCommentWorkCount();
  const status=document.querySelector('#comment-selection-count');
  if(status)status.textContent=`已选 ${count}/${MAX_SELECTED_COMMENTS} 条评论，来自 ${works}/${MAX_SELECTED_WORKS} 篇作品；本页可继续选择。`;
  const action=document.querySelector('#study-selected-comments');if(action)action.disabled=count===0;
  document.querySelectorAll('[data-select-comment]').forEach(input=>{
    if(input.checked||input.dataset.eligible!=='true')return;
    input.disabled=count>=MAX_SELECTED_COMMENTS||(works>=MAX_SELECTED_WORKS&&![...selectedCommentKeys.values()].some(item=>item.commentKey.workRef===input.dataset.workRef));
  });
}
async function renderCommentsTab(){
  if(!domainRef)return'<p class="study-empty">评论目录尚未就绪。</p>';
  const params={q:commentCatalogState.q,workRef:commentCatalogState.workRef,studyState:commentCatalogState.studyState,voiceRole:commentCatalogState.voiceRole,cursor:commentCatalogState.cursor,limit:50};
  const summaryParams={...params};delete summaryParams.cursor;delete summaryParams.limit;
  const[data,summary]=await Promise.all([get(catalogQuery('comments',params)),get(catalogQuery('catalog-summary',summaryParams))]);
  commentCatalogState.response=data;commentCatalogState.summary=summary;
  const rows=data.items||[],stats=summary.summary||{},coverage=data.indexCoverage||{},pending=coverage.pendingCount==null?'未知':Number(coverage.pendingCount);
  const workOptions=commentCatalogState.workChoices.map(work=>`<option value="${esc(work.workRef)}"${work.workRef===commentCatalogState.workRef?' selected':''}>${esc(work.displayTitle||'未命名作品')}</option>`).join('');
  const body=rows.length?rows.map(item=>{const key=commentSelectionKey(item.commentKey),role=item.observationRole,eligible=item.studyEligibility?.eligible===true&&(role==='primary'||role==='reference'),checked=selectedCommentKeys.has(key),newWork=![...selectedCommentKeys.values()].some(selected=>selected.commentKey.workRef===item.commentKey.workRef),capped=!checked&&(selectedCommentKeys.size>=MAX_SELECTED_COMMENTS||(newWork&&selectedCommentWorkCount()>=MAX_SELECTED_WORKS));return`<tr><td><input type="checkbox" data-select-comment data-work-ref="${esc(item.commentKey.workRef)}" data-comment-id="${esc(item.commentKey.commentExternalId)}" data-eligible="${eligible}" aria-label="选择这条评论进入研究"${checked?' checked':''}${!eligible||capped?' disabled':''}></td><td><button type="button" class="study-comment-open" data-comment-work="${esc(item.commentKey.workRef)}" data-comment-id="${esc(item.commentKey.commentExternalId)}"><span class="study-comment-voice">${esc(item.commentText)}</span></button><small>${esc(commentEligibility(item))}</small></td><td><span class="study-work-title">${esc(item.workTitle||'未命名作品')}</span><small>${esc(workTitleSourceLabel[item.workTitleSource]||item.workTitleSource||'标题未记录')}</small></td><td>${esc(commentStatus(item))}</td><td>${esc(item.authorDisplayName||'作者未记录')}<small>${esc(String(item.observedAt||'').replace('T',' '))}</small></td></tr>`}).join(''):'<tr><td colspan="5" class="study-table-empty">当前筛选没有可显示的用户评论。</td></tr>';
  return`<section class="study-comments" aria-labelledby="comments-title"><header class="study-comments-head"><div><p class="study-label">评论证据库</p><h2 id="comments-title">用户评论</h2><p>当前筛选可显示 ${Number(stats.displayableCommentCount||0)} 条；可研究 ${Number(stats.eligibleCommentCount||0)} 条；已索引 ${Number(coverage.indexedCount||0)}，待索引 ${pending}。</p></div><div class="study-comment-selection"><span id="comment-selection-count" role="status"></span><button id="study-selected-comments" type="button" disabled>研究所选评论</button></div></header>
  <form id="comment-filter-form" class="study-comment-filters"><label>字面搜索<input id="comment-query" type="search" value="${esc(commentCatalogState.q)}" placeholder="搜索清洗后的评论文本"></label><label>研究状态<select id="comment-study-state">${[['all','全部'],['never_studied','从未研究'],['in_progress','处理中'],['studied','已有成功结果'],['needs_context','等待语境'],['failed','最近失败']].map(([v,t])=>`<option value="${v}"${commentCatalogState.studyState===v?' selected':''}>${t}</option>`).join('')}</select></label><label>声音角色<select id="comment-voice-role">${[['reader_and_unknown','用户评论（默认）'],['reader','已知用户'],['unknown','身份未知'],['creator','作品作者'],['all','全部声音']].map(([v,t])=>`<option value="${v}"${commentCatalogState.voiceRole===v?' selected':''}>${t}</option>`).join('')}</select></label><button type="submit">应用筛选</button></form>
  <div class="study-comment-work-filter"><form id="comment-work-search-form"><label>所属作品<input id="comment-work-search" type="search" value="${esc(commentCatalogState.workLabel)}" placeholder="搜索作品标题"></label><button type="submit">查找作品</button></form><label>已匹配作品<select id="comment-work-select"><option value="">全部作品</option>${workOptions}</select></label></div>
  <div class="study-review-table-wrap"><table class="study-review-table study-comments-table"><thead><tr><th scope="col">选择</th><th scope="col">原声</th><th scope="col">所属作品</th><th scope="col">研究状态</th><th scope="col">作者 / 观测</th></tr></thead><tbody>${body}</tbody></table></div>
  <nav class="study-pagination" aria-label="用户评论翻页"><button id="comment-prev" type="button"${commentCatalogState.history.length?'':' disabled'}>上一页</button><span>第 ${commentCatalogState.history.length+1} 页</span><button id="comment-next" type="button"${data.page?.nextCursor?'':' disabled'}>下一页</button></nav></section>`;
}
async function searchCommentWorks(query){
  const data=await get(catalogQuery('works',{q:query,limit:20}));commentCatalogState.workChoices=data.items||[];
  const select=document.querySelector('#comment-work-select');if(!select)return;
  select.innerHTML='<option value="">全部作品</option>'+commentCatalogState.workChoices.map(work=>`<option value="${esc(work.workRef)}">${esc(work.displayTitle||'未命名作品')}</option>`).join('');
}
function bindCommentsView(){
  const form=document.querySelector('#comment-filter-form');if(!form)return;
  document.querySelectorAll('[data-select-comment]').forEach(input=>input.addEventListener('change',event=>{
    const box=event.currentTarget,key={workRef:box.dataset.workRef,commentExternalId:box.dataset.commentId},identity=commentSelectionKey(key);
    if(box.checked){
      const item=(commentCatalogState.response?.items||[]).find(row=>commentSelectionKey(row.commentKey)===identity);
      if(!item||item.studyEligibility?.eligible!==true||!['primary','reference'].includes(item.observationRole)||selectedCommentKeys.size>=MAX_SELECTED_COMMENTS||(selectedCommentWorkCount()>=MAX_SELECTED_WORKS&&![...selectedCommentKeys.values()].some(selected=>selected.commentKey.workRef===key.workRef))){box.checked=false;return;}
      selectedCommentKeys.set(identity,{commentKey:key,observationRole:item.observationRole});
    }else selectedCommentKeys.delete(identity);
    updateCommentSelectionView();
  }));
  document.querySelector('#study-selected-comments').addEventListener('click',()=>{setStudySelectionKind('comments');studyDialog.showModal();});
  updateCommentSelectionView();
  form.addEventListener('submit',async event=>{event.preventDefault();commentCatalogState.q=document.querySelector('#comment-query').value.trim();commentCatalogState.studyState=document.querySelector('#comment-study-state').value;commentCatalogState.voiceRole=document.querySelector('#comment-voice-role').value;resetCommentPage();syncStudyRoute(true);await renderActiveTab();});
  document.querySelector('#comment-work-search-form').addEventListener('submit',async event=>{event.preventDefault();const query=document.querySelector('#comment-work-search').value.trim();commentCatalogState.workLabel=query;try{await searchCommentWorks(query);}catch(error){document.querySelector('#comment-work-select').innerHTML='<option value="">作品读取失败</option>';}});
  document.querySelector('#comment-work-select').addEventListener('change',async event=>{commentCatalogState.workRef=event.currentTarget.value||null;const chosen=commentCatalogState.workChoices.find(work=>work.workRef===commentCatalogState.workRef);commentCatalogState.workLabel=chosen?.displayTitle||'';resetCommentPage();syncStudyRoute(true);await renderActiveTab();});
  document.querySelector('#comment-prev').addEventListener('click',async()=>{commentCatalogState.cursor=commentCatalogState.history.pop()??null;syncStudyRoute(true);await renderActiveTab();});
  document.querySelector('#comment-next').addEventListener('click',async()=>{const next=commentCatalogState.response?.page?.nextCursor;if(!next)return;commentCatalogState.history.push(commentCatalogState.cursor);commentCatalogState.cursor=next;syncStudyRoute(true);await renderActiveTab();});
  document.querySelectorAll('[data-comment-work][data-comment-id]').forEach(button=>button.addEventListener('click',()=>openCommentDetail(button.dataset.commentWork,button.dataset.commentId)));
}
function detailSection(title,body){return`<section class="study-detail-section"><h3>${esc(title)}</h3>${body}</section>`;}
function commentHistoryRow(item){
  const run = item.runRef;
  const actions = run ? `<div class="study-link-actions"><button type="button" class="study-link" data-history-run="${esc(run)}" data-history-panel="targets" data-history-target="${esc(item.targetRef||'')}">查看本次目标</button><button type="button" class="study-link" data-history-run="${esc(run)}" data-history-panel="signals" data-history-target="${esc(item.targetRef||'')}">查看本次信号</button><button type="button" class="study-link" data-history-run="${esc(run)}" data-history-panel="method">查看本次方法</button></div>` : '<span>历史 Run 编号未记录，无法跳转。</span>';
  return `<li><strong>${esc(label(targetStateLabel,item.state)??item.state)}</strong><span>${esc(String(item.createdAt||'').replace('T',' '))}</span><span>Signal ${Number(item.signalCount||0)} 条</span><span>方法：${esc(item.method?.recordingState==='recorded'?(item.method.methodName||item.method.policyRef):'历史未记录')}</span>${actions}</li>`;
}
function renderCommentDetail(data){
  const comment=data.comment,source=data.source||{},work=data.work||{},parent=data.parentContext,history=data.studyHistory?.items||[];
  const sourceReadable=source.sourceState==='known';
  const raw=sourceReadable&&comment?.commentText?`<blockquote>${esc(comment.commentText)}</blockquote>`:`<p class="study-restricted">当前原声不可显示：${esc(source.displayState||source.sourceState||'unknown')}</p>`;
  const workBody=`<p><strong>${esc(work.displayTitle||comment?.workTitle||'未命名作品')}</strong> <small>${esc(workTitleSourceLabel[work.displayTitleSource]||work.displayTitleSource||'标题未记录')}</small></p><button type="button" class="study-link" data-detail-work="${esc(data.commentKey.workRef)}" data-detail-work-title="${esc(work.displayTitle||comment?.workTitle||'')}">查看该作品的评论</button>`;
  const parentKey=parent?.commentKey;
  const parentBody=sourceReadable&&parent?.commentText?`<blockquote>${esc(parent.commentText)}</blockquote><p class="study-signal-meta">仅作为语境，不作为当前评论的独立证据。</p>${parentKey?.workRef&&parentKey?.commentExternalId?`<button type="button" class="study-link" data-detail-parent-work="${esc(parentKey.workRef)}" data-detail-parent-id="${esc(parentKey.commentExternalId)}">查看父评论详情</button>`:''}`:'<p class="muted">没有可显示的父评论语境。</p>';
  const cleanBody=sourceReadable&&comment?`<p>${esc(comment.researchText||'')}</p><p class="study-signal-meta">状态：${esc(comment.cleanState||'unknown')} · 原因：${esc((comment.cleanReasons||[]).join('、')||'无')}</p>`:'<p class="study-restricted">清洗文本当前不可显示。</p>';
  const historyBody=history.length?`<ol class="study-history-list">${history.map(commentHistoryRow).join('')}</ol>`:'<p class="muted">尚无研究历史。</p>';
  const next=data.studyHistory?.page?.nextCursor;
  const more=next?`<button type="button" id="comment-history-more" data-work="${esc(data.commentKey.workRef)}" data-comment-id="${esc(data.commentKey.commentExternalId)}" data-cursor="${esc(next)}">继续读取研究历史</button>`:'';
  return raw+detailSection('所属作品',workBody)+detailSection('父评论语境',parentBody)+detailSection('清洗文本',cleanBody)+detailSection(`研究历史（${Number(data.studyHistory?.totalCount||0)}）`,historyBody+more);
}
async function loadMoreCommentHistory(button){
  const data=await get(catalogQuery('comments/history',{workRef:button.dataset.work,commentExternalId:button.dataset.commentId,cursor:button.dataset.cursor,limit:50}));
  const list=document.querySelector('#comment-detail-body .study-history-list');
  if(list)list.insertAdjacentHTML('beforeend',(data.items||[]).map(commentHistoryRow).join(''));
  if(data.page?.nextCursor){button.dataset.cursor=data.page.nextCursor;button.disabled=false;button.textContent='继续读取研究历史';}else button.remove();
}
function bindCommentHistoryMore(){
  const button=document.querySelector('#comment-history-more');if(!button)return;
  button.addEventListener('click',async()=>{button.disabled=true;button.textContent='正在读取…';try{await loadMoreCommentHistory(button);}catch(error){button.disabled=false;button.textContent=`读取失败，重试：${error.message}`;}});
}
let commentDetailRequest = 0;
async function openCommentDetail(workRef,commentExternalId,{push=true}={}){
  if(push){
    window.history.replaceState({...window.history.state,commentScrollY:window.scrollY,commentFilter:captureCommentFilter()},'',window.location.href);
    const url=new URL(window.location.href);
    url.searchParams.set('view','comments');
    for(const key of ['problemRef','runRef','panel','targetRef','signalRef'])url.searchParams.delete(key);
    url.searchParams.set('commentWorkRef',workRef);
    url.searchParams.set('commentExternalId',commentExternalId);
    url.searchParams.set('detail','comment');
    window.history.pushState({...window.history.state,commentDetail:true,commentFilter:captureCommentFilter()},'',url);
  }
  const request=++commentDetailRequest;
  const dialog=document.querySelector('#comment-detail-dialog'),body=document.querySelector('#comment-detail-body');body.innerHTML='<p class="study-empty">正在读取评论详情…</p>';if(!dialog.open)dialog.showModal();
  try{const data=await get(catalogQuery('comments/detail',{workRef,commentExternalId}));if(request!==commentDetailRequest)return;body.innerHTML=renderCommentDetail(data);bindCommentHistoryMore();}catch(error){if(request===commentDetailRequest)body.innerHTML=`<p class="study-empty">评论详情读取失败：${esc(error.message)}</p>`;}
}
function closeCommentDetail({back=true}={}){
  const dialog=document.querySelector('#comment-detail-dialog');
  ++commentDetailRequest;
  if(dialog.open)dialog.close();
  if(back&&window.history.state?.commentDetail){window.history.back();return;}
  const url=new URL(window.location.href);
  if(url.searchParams.has('commentWorkRef')||url.searchParams.get('detail')==='comment'){
    url.searchParams.delete('commentWorkRef');url.searchParams.delete('commentExternalId');url.searchParams.delete('detail');
    window.history.replaceState({...window.history.state,commentDetail:false},'',url);
  }
}
function pairOutcomeSummary(outcome) {
  const state = label(pairStateLabel, outcome.state) ?? '配对状态未知';
  const decision = outcome.decisionReason
    ? (label(pairDecisionLabel, outcome.decisionReason) ?? '配对结论未能识别')
    : '历史配对未记录可分类结论';
  const selection = outcome.selection;
  const recallRank = Number(selection?.recallRank);
  const admissibleRank = Number(selection?.admissibleRank);
  const selectionText = Number.isInteger(recallRank) && recallRank > 0
    && Number.isInteger(admissibleRank) && admissibleRank > 0
    ? `本次自动选择：召回候选第 ${recallRank} 位；通过独立性筛选后第 ${admissibleRank} 位。`
    : '历史配对未记录候选选择信息。';
  return `<p class="study-signal-meta">首个候选比较：${esc(state)} · ${esc(decision)}<br>${esc(selectionText)}</p>`;
}
function signalCard(signal) {
  const sourceReadable=signal.sourceState==='known';
  const body = !sourceReadable
    ? `<p class="study-restricted">本条或父语境已受限，研究衍生文本不再显示。</p>`
    : `<p class="study-signal-proposition">${esc(signal.proposition)}</p><blockquote>${esc(signal.evidence)}</blockquote>`;
  return `
    <article class="study-signal-card" data-signal-ref="${esc(signal.signalRef)}" data-target-ref="${esc(signal.targetRef)}" data-focus="${focusedSignalRef === signal.signalRef || focusedTargetRef === signal.targetRef}">
      <header><span class="study-badge">${esc(label(signalKindLabel, signal.kind) ?? signal.kind)}</span><span>${esc(observationRoleLabel[signal.observationRole] || '来源角色未知')} · ${esc(label(resolutionLabel, signal.resolutionState) ?? '尚未进入归并判断')}</span></header>
      ${body}
      <p class="study-signal-meta">归并资格：${esc(label(eligibilityLabel, signal.eligibilityState) ?? signal.eligibilityState)}${sourceReadable&&signal.eligibilityReason ? ` · ${esc(signal.eligibilityReason)}` : ''}</p>
      ${sourceReadable?(signal.pairOutcomes || []).map(pairOutcomeSummary).join(''):''}
      <div class="study-link-actions"><button type="button" class="study-link" data-signal-target="${esc(signal.targetRef)}">查看目标评论</button>${!sourceReadable?'<span>来源受限，关联信息已隐藏</span>':signal.resolvedProblemRef?`<button type="button" class="study-link" data-signal-problem="${esc(signal.resolvedProblemRef)}">查看用户问题</button>`:'<span>尚未关联长期用户问题</span>'}</div>
    </article>`;
}
async function renderSelectedRunPanel() {
  if (!selectedRunRef) return '<p class="study-empty">尚无研究运行，先创建一次研究运行。</p>';
  const requestedRunRef=selectedRunRef;
  let runDetail=null;
  try{const response=await get(`runs/${encodeURIComponent(requestedRunRef)}`);if(response.run?.runRef!==requestedRunRef)throw new Error('运行详情回执与当前选择不一致。');runDetail=response.run;}catch{runDetail=null;}
  const labels = { targets: '目标评论与上下文', signals: '研究结果', requests: '调用记录', method: '本次方法' };
  let content;
  let pageStatus = '';
  let more = '';
  if (selectedRunPanel === 'method') {
    content = runDetail?renderRunMethodPanel(runDetail):'<p class="study-restricted">本次方法暂时读取失败。</p>';
  } else {
    const state = runPanelCache[selectedRunPanel];
    if (!state.loaded || state.runRef !== selectedRunRef) await loadRunPanelPage(selectedRunPanel, true);
    const wantedRef=selectedRunPanel==='targets'?focusedTargetRef:selectedRunPanel==='signals'?(focusedSignalRef||focusedTargetRef):null;
    const matches=item=>selectedRunPanel==='targets'?item.targetRef===wantedRef:(focusedSignalRef?item.signalRef===wantedRef:item.targetRef===wantedRef);
    while(wantedRef&&state.nextCursor&&!state.items.some(matches))await loadRunPanelPage(selectedRunPanel);
    content = selectedRunPanel === 'targets'
      ? renderTargetsPanel(state.items)
      : selectedRunPanel==='signals' ? list(state.items, signalCard, '本次运行没有可读取的研究信号。')
        : renderRunRequestsPanel(state.items);
    pageStatus = `<p class="study-run-page-status">当前显示 ${state.items.length} 条${state.nextCursor ? '，还有后续内容' : '，已到本次列表末尾'}</p>`;
    more = state.nextCursor ? '<button type="button" class="study-link" data-run-load-more>加载下一页</button>' : '';
  }
  const tabs = Object.entries({targets:'目标评论',signals:'研究信号',requests:'调用记录',method:'本次方法'})
    .map(([key,text])=>`<button type="button" role="tab" data-run-panel="${key}" aria-selected="${selectedRunPanel===key}">${text}</button>`).join('');
  return `<section class="study-run-detail"><header><h2 tabindex="-1">${esc(labels[selectedRunPanel])}</h2><p>当前运行 ${esc(selectedRunRef)}</p></header>${runDetail?renderRunSummary(runDetail):'<p class="study-restricted">本次运行摘要暂时读取失败，以下分页内容仍可查看。</p>'}<nav class="study-run-panels" role="tablist" aria-label="本次运行内容">${tabs}</nav>${pageStatus}${content}${more}</section>`;
}

function renderRunSummary(run){
  const semantic=run.semanticSummary,knowledge=run.knowledgeSummary,cost=run.costSummary;
  const n=value=>value==null||Number.isNaN(Number(value))?'—':Number(value).toLocaleString('zh-CN');
  const semanticBody=semantic?`<p>冻结 ${n(semantic.targetCount)} 条目标评论；提取成功 ${n(semantic.succeededTargetCount)} 条，无信号 ${n(semantic.noSignalTargetCount)} 条，等待语境 ${n(semantic.needsContextTargetCount)} 条，失败 ${n(semantic.failedTargetCount)} 条，排除 ${n(semantic.excludedTargetCount)} 条。</p><p>提取尝试 ${n(semantic.attemptCount)} 次，产生 ${n(semantic.signalCount)} 条信号；其中当前有效 ${n(semantic.currentSignalCount)} 条。</p>`:'<p class="study-detail-muted">提取汇总未记录。</p>';
  const knowledgeBody=knowledge?`<p>当前有归并资格 ${n(knowledge.eligibleSignalCount)} 条信号；归入问题 ${n(knowledge.assignedSignalCount)} 条，本 Run 已关联问题 ${n(knowledge.linkedProblemCount)} 个。</p><p>归并待处理 ${n(knowledge.pendingResolutionCount)} 条、候选比较待处理 ${n(knowledge.pendingPairCount)} 组；新表达暂缓 ${n(knowledge.deferredNovelCount)} 条、边界未定 ${n(knowledge.deferredAmbiguousCount)} 条、等待语境 ${n(knowledge.deferredContextCount)} 条。</p><p>检索未完成 ${n(knowledge.retrievalIncompleteCount)} 条、预算中止 ${n(knowledge.budgetStoppedCount)} 条、协议拒绝 ${n(knowledge.protocolRejectedCount)} 条、归并失败 ${n(knowledge.failedResolutionCount)} 条。评论提取结束不代表归并全部闭合。</p>`:'<p class="study-detail-muted">归并汇总未记录。</p>';
  let costBody='<p class="study-detail-muted">调用账本未记录。</p>';
  if(cost?.recordingState==='unrecorded')costBody='<p class="study-detail-muted">历史调用记录未记录，无法判断请求次数或用量。</p>';
  else if(cost){
    const incomplete=cost.recordingState!=='recorded';
    costBody=`<p>${incomplete?'历史调用记录可能不完整；':''}请求 ${n(cost.requestCount)} 次，已派发 ${n(cost.dispatchedRequestCount)} 次；用量已知 ${n(cost.usageKnownRequestCount)} 次、未知 ${n(cost.usageUnknownRequestCount)} 次。</p><p>${cost.totalInputTokens!=null&&cost.totalOutputTokens!=null?`输入 ${n(cost.totalInputTokens)}、输出 ${n(cost.totalOutputTokens)} Token`:`已知输入 ${n(cost.knownInputTokens)}、已知输出 ${n(cost.knownOutputTokens)} Token；完整用量未知`}${cost.chargedTokens==null?'':`；保守计入 ${n(cost.chargedTokens)} Token`}。账单金额未记录。</p>`;
  }
  return `<section class="study-run-summary" aria-label="本次运行摘要"><article><h3>评论提取</h3>${semanticBody}</article><article><h3>问题归并</h3>${knowledgeBody}</article><article><h3>调用与用量</h3>${costBody}</article></section>`;
}

function renderRunMethodPanel(run) {
  const manifest = run.method?.manifest;
  const limits = run.limits || {};
  const limitText = value => value == null ? '未记录' : Number(value).toLocaleString('zh-CN');
  const frozen = `<p>本次冻结上限：评论 ${limitText(limits.commentBudget)} 条；单作品语境 ${limitText(limits.contextCharacterBudget)} 字符；模型 ${limitText(limits.tokenLimit)} Token。</p>`;
  if (!manifest?.stages) {
    return `<div class="study-method-detail"><p>方法版本 ${esc(run.policyRef)} · 历史方法内容未记录。</p>${frozen}<p>不会用当前默认方法补写这次运行。</p></div>`;
  }
  const stages = {semantic:'评论语义提取',resolution:'问题归并',pair:'独立证据配对'};
  const stageRows = Object.entries(stages).map(([key,title])=>{
    const stage = manifest.stages?.[key];
    if (!stage) return `<section><h3>${title}</h3><p>本阶段说明未记录。</p></section>`;
    const schema = stage.outputSchema == null ? '未记录' : JSON.stringify(stage.outputSchema,null,2);
    return `<section><h3>${title}</h3><pre>${esc(stage.systemInstruction || '说明未记录')}</pre><details><summary>查看严格输出 Schema</summary><pre>${esc(schema)}</pre></details></section>`;
  }).join('');
  const fixed = Object.fromEntries(Object.entries(manifest).filter(([key])=>key!=='stages'));
  return `<div class="study-method-detail"><p><strong>${esc(run.method?.name||'未命名方法')}</strong> · 版本 ${esc(run.policyRef)} · hash ${esc(run.method?.hash||'未记录')}</p>${frozen}${stageRows}<details><summary>固定规则、清洗器与模型配置</summary><pre>${esc(JSON.stringify(fixed,null,2))}</pre></details></div>`;
}
function renderRunRequestsPanel(requests){
  return list(requests,request=>{
    const usage=request.usageKnown?`输入 ${Number(request.inputTokens)} · 输出 ${Number(request.outputTokens)} · 计费 ${Number(request.chargedTokens)} Token`:`用量未知 · 预留 ${Number(request.reservedTokens)} Token · 保守计入 ${Number(request.chargedTokens)} Token`;
    return `<article class="study-request-card"><header><strong>${esc(requestStageLabel[request.stage]||'未知阶段')}</strong><span>${esc(label(modelRequestStateLabel,request.state)||'状态未记录')} · ${request.dispatched?'已派发':'未派发'}</span></header><p>模型：${esc(request.modelIdentity?.modelId||'名称未记录')}</p><p>${esc(String(request.createdAt||'').replace('T',' '))} · 尝试 ${Number(request.attemptOrdinal||0)}</p><p>${esc(usage)}${request.elapsedMs!=null?` · ${Number(request.elapsedMs)} ms`:''}</p>${request.failureCode?`<p>错误码：${esc(request.failureCode)}</p>`:''}${request.invocationRef?`<button type="button" class="study-link" data-request-detail="${esc(request.invocationRef)}" aria-expanded="false">查看本次请求详情</button><div class="study-request-detail" hidden></div>`:'<p class="study-detail-muted">请求编号未记录，无法读取完整快照。</p>'}</article>`;
  },'本次 Run 没有模型请求记录。');
}
function renderRequestDetail(request){
  const identity=request.modelIdentity||{};
  const meta=`<p>请求 ${esc(request.invocationRef)} · ${esc(requestStageLabel[request.stage]||'未知阶段')} · ${esc(label(modelRequestStateLabel,request.state)||'状态未记录')} · ${request.dispatched?'已派发':'未派发'}</p><p>模型：${esc(identity.modelId||'名称未记录')}</p><details><summary>查看冻结模型身份</summary><p>modelRef ${esc(identity.modelRef||'未记录')} · connectionVersionRef ${esc(identity.connectionVersionRef||'未记录')} · modelConfigRef ${esc(request.modelConfigRef||'未记录')}</p></details><p>创建于 ${esc(String(request.createdAt||'').replace('T',' '))}${request.dispatchStartedAt?` · 派发于 ${esc(String(request.dispatchStartedAt).replace('T',' '))}`:''}</p>`;
  if(request.sourceState==='restricted')return `${meta}<p class="study-restricted">来源当前受限，本次请求正文快照不再显示。</p>`;
  if(request.sourceState==='unavailable')return `${meta}<p class="study-restricted">来源或候选当前不可用，本次请求正文快照不再显示。</p>`;
  if(request.sourceState!=='known')return `${meta}<p class="study-restricted">来源状态未确认，本次请求正文快照不显示。</p>`;
  if(request.recordingState!=='recorded'||request.requestManifest==null)return `${meta}<p class="study-detail-muted">历史请求的完整快照未记录。</p>`;
  return `${meta}<h4>本次冻结请求快照</h4><pre>${esc(JSON.stringify(request.requestManifest,null,2))}</pre>`;
}

async function loadRunPanelPage(panel, reset = false) {
  const state = runPanelCache[panel];
  const requestedRunRef = selectedRunRef;
  const generation = runPanelGeneration;
  if (reset || state.runRef !== selectedRunRef) {
    state.runRef = selectedRunRef;
    state.items = [];
    state.nextCursor = null;
    state.loaded = false;
  }
  if (!selectedRunRef || (!reset && !state.nextCursor)) return;
  const cursor = reset ? null : state.nextCursor;
  const query = new URLSearchParams({ limit: '50' });
  if(panel!=='requests')query.set('runRef',selectedRunRef);
  if (cursor) query.set('cursor', cursor);
  const path=panel==='requests'?`runs/${encodeURIComponent(selectedRunRef)}/requests?${query}`:`${panel}?${query}`;
  const response = await get(path);
  if (generation !== runPanelGeneration || state.runRef !== requestedRunRef || selectedRunRef !== requestedRunRef) return;
  if (response.runRef !== requestedRunRef) throw new Error('运行详情回执与当前选择不一致。');
  const items = response[panel] || [];
  state.items = cursor ? [...state.items, ...items] : items;
  state.nextCursor = response.page?.nextCursor || null;
  state.loaded = true;
}

async function renderProblemsTab() {
  const [problemResult,candidateResult]=await Promise.allSettled([
    problemListCache.loaded?Promise.resolve():loadProblemPage(true),
    problemCandidateState.loaded?Promise.resolve():loadProblemCandidatePage(true)
  ]);
  const problemError=problemResult.status==='rejected';
  problemCandidateState.error=candidateResult.status==='rejected';
  const rows = problemListCache.items;
  const count = problemListCache.hasMore ? `当前读取 ${rows.length} 个，还有后续问题` : `当前读取 ${rows.length} 个问题`;
  const cards = list(rows, problem => `<article class="study-problem-card" data-problem-ref="${esc(problem.problemRef)}" data-focus="${selectedProblemRef===problem.problemRef}"><header><span>${esc(label(problemStateLabel, problem.state) ?? problem.state)}</span><span>关联研究信号 ${Number(problem.membershipCount||0)} 条</span></header><h3 class="study-signal-proposition">${esc(problem.title||problem.definition||'定义当前不可读取')}</h3><p class="study-signal-meta">支持评论 ${Number(problem.supportCommentCount||0)} 条 · 独立作者 ${Number(problem.supportAuthorCount||0)} 位 · 作品 ${Number(problem.supportWorkCount||0)} 篇${problem.definitionCurrent===false?' · 当前定义依据已失效':''}</p><button type="button" class="study-link" data-problem-open="${esc(problem.problemRef)}">查看定义与原声依据</button></article>`, '尚无已建立的长期用户问题。');
  const more=problemListCache.nextCursor?'<button type="button" class="study-link" data-problem-list-more>加载更多用户问题</button>':'';
  let detailHtml='';
  if(selectedProblemRef){
    try{const [detail,evidence]=await Promise.all([get(`problems/${encodeURIComponent(selectedProblemRef)}`),get(`problems/${encodeURIComponent(selectedProblemRef)}/evidence?limit=50`)]);if(detail.problem?.problemRef!==selectedProblemRef||evidence.problemRef!==selectedProblemRef)throw new Error('用户问题详情与当前选择不一致。');detailHtml=renderProblemDetail(detail.problem,evidence,detail.revisionHistory||[]);}catch(error){detailHtml=`<p class="study-restricted">问题详情读取失败：${esc(error.message)}</p>`;}
  }
  return `<section aria-labelledby="filed-problems-title"><h2 id="filed-problems-title">已建档用户问题</h2>${problemError?'<p class="study-restricted">已建档问题列表暂时读取失败。</p>':`<p class="study-run-page-status">${esc(count)}</p>${cards}${more}`}${detailHtml}</section>${renderProblemCandidates()}`;
}
async function loadProblemCandidatePage(reset=false){
  if(!reset&&!problemCandidateState.nextCursor)return;
  const query=new URLSearchParams({state:problemCandidateState.state,limit:'50'});
  if(!reset)query.set('cursor',problemCandidateState.nextCursor);
  const response=await get(`problem-candidates?${query}`);
  problemCandidateState.items=reset?(response.expressions||[]):[...problemCandidateState.items,...(response.expressions||[])];
  problemCandidateState.nextCursor=response.page?.nextCursor||null;
  problemCandidateState.asOf=response.page?.asOf||null;
  problemCandidateState.loaded=true;problemCandidateState.error=null;
}
function renderProblemCandidates(){
  const stateLabels={all:'全部未闭合状态',pending:'正在归并',deferred_novel:'新表达，尚未建档',deferred_ambiguous:'归并边界尚未确定',deferred_context:'等待语境',retrieval_incomplete:'候选检索未完成',budget_stopped:'机器处理未完成：预算上限',protocol_rejected:'机器输出未接纳',failed:'机器归并失败'};
  const options=Object.entries(stateLabels).map(([state,title])=>`<option value="${state}"${problemCandidateState.state===state?' selected':''}>${title}</option>`).join('');
  const cards=problemCandidateState.items.map(item=>{
    const readable=item.sourceState==='known',key=item.commentKey||{};
    const machineState=['retrieval_incomplete','budget_stopped','protocol_rejected','failed'].includes(item.state);
    const pairOutcomes=Array.isArray(item.pairOutcomes)?item.pairOutcomes:[];
    const pairStatus=!readable?'':pairOutcomes.length?`<div class="study-candidate-pairs"><h4>当前候选比较</h4>${pairOutcomes.map(outcome=>`<p class="study-signal-meta">${esc(pairStateLabel[outcome.state]||'比较状态未记录')} · ${esc(outcome.decisionReason?(pairDecisionLabel[outcome.decisionReason]||'比较原因未记录'):'比较原因未记录')}</p>`).join('')}</div>`:'<p class="study-signal-meta">当前没有可读取的候选比较结论；不能据此判断是否已有独立证据。</p>';
    const evidence=readable?`<h3>${esc(item.proposition||'归一表达未记录')}</h3>${item.evidence?`<blockquote>${esc(item.evidence)}</blockquote>`:''}${item.commentText?`<p class="study-candidate-voice">原声：${esc(item.commentText)}</p>`:''}`:'<p class="study-restricted">来源当前不可读，原声及归一表达不显示。</p>';
    return `<article class="study-problem-card study-candidate-card"><header><span>${esc(label(resolutionLabel,item.state)??'归并状态未记录')}</span><span>${esc(label(signalKindLabel,item.kind)??item.kind)}</span></header>${machineState?'<p class="study-restricted">机器处理未完成或未接纳，不能据此判断用户观点或问题边界。</p>':''}${evidence}${pairStatus}<p class="study-signal-meta">${readable?esc(item.authorDisplayName||'作者未记录'):'来源状态未确认'} · ${esc(String(item.createdAt||'').replace('T',' '))}</p><div class="study-link-actions">${key.workRef&&key.commentExternalId?`<button type="button" class="study-link" data-evidence-work="${esc(key.workRef)}" data-evidence-id="${esc(key.commentExternalId)}">查看评论及父语境</button>`:''}${item.runRef&&item.signalRef?`<button type="button" class="study-link" data-candidate-run="${esc(item.runRef)}" data-candidate-target="${esc(item.targetRef||'')}" data-candidate-signal="${esc(item.signalRef)}">${machineState?'查看运行与信号诊断':'查看归并依据'}</button>`:''}</div></article>`;
  }).join('');
  return `<section class="study-problem-candidates" aria-labelledby="problem-candidate-title"><header class="study-candidate-head"><div><h2 id="problem-candidate-title">尚未建档的表达与处理状态</h2><p>当前有效原声可能仍在判断、需要更多依据，或机器处理未完成；具体原因以每条状态和候选比较为准。筛选与分页由服务端执行。</p></div><label>归并状态<select id="problem-candidate-state">${options}</select></label></header>${problemCandidateState.error?'<p class="study-restricted">未建档表达暂时读取失败；已建档问题仍可查看。</p>':`<p class="study-run-page-status">当前读取 ${problemCandidateState.items.length} 条${problemCandidateState.nextCursor?'，还有后续内容':''}${problemCandidateState.asOf?` · 截至 ${esc(String(problemCandidateState.asOf).replace('T',' '))}`:''}</p>${cards||'<p class="study-empty">当前筛选没有可读取的未建档表达。</p>'}${problemCandidateState.nextCursor?'<button type="button" class="study-link" data-candidate-more>加载更多未建档表达</button>':''}`}</section>`;
}
async function loadProblemPage(reset=false){
  if(!reset&&!problemListCache.nextCursor)return;
  const query=new URLSearchParams({limit:'100'});
  if(!reset)query.set('cursor',problemListCache.nextCursor);
  const response=await get(`problems?${query}`);
  problemListCache.items=reset?(response.problems||[]):[...problemListCache.items,...(response.problems||[])];
  problemListCache.nextCursor=response.page?.nextCursor||null;
  problemListCache.hasMore=Boolean(response.page?.hasMore);
  problemListCache.loaded=true;
}
function renderProblemEvidence(row){
  const sourceReadable=row.sourceState==='known'&&typeof row.commentText==='string'&&row.commentText.length>0;
  const key=row.commentKey||{};
  const signals=sourceReadable?(row.signals||[]).map(signal=>`<li><span>${esc(label(signalKindLabel,signal.kind)??signal.kind)}</span> · ${esc(signal.proposition||'未记录归一表达')}${signal.evidence?`<blockquote>${esc(signal.evidence)}</blockquote>`:''}</li>`).join(''):'';
  return `<article class="study-problem-evidence">${sourceReadable?`<blockquote>${esc(row.commentText)}</blockquote>`:'<p class="study-restricted">来源当前受限，原声及衍生内容不再显示。</p>'}${sourceReadable?`<p class="study-signal-meta">${esc(row.authorDisplayName||'作者未记录')} · ${esc(row.workRef||'作品未记录')}</p>`:''}${signals?`<ul>${signals}</ul>`:''}${key.workRef&&key.commentExternalId?`<button type="button" class="study-link" data-evidence-work="${esc(key.workRef)}" data-evidence-id="${esc(key.commentExternalId)}">查看评论及父语境</button>`:''}</article>`;
}
function renderProblemIdentity(identity){
  const fields=[['actor','谁遇到问题'],['goalOrExpectedState','期望达到'],['barrierOrUnmetNeed','阻碍或未满足需求'],['context','发生情境']];
  return `<dl class="study-problem-identity">${fields.map(([key,title])=>`<div><dt>${title}</dt><dd>${esc(identity?.[key]||'当前未知')}</dd></div>`).join('')}</dl>`;
}
function renderProblemCriteria(items){return Array.isArray(items)&&items.length?`<ul>${items.map(item=>`<li>${esc(typeof item==='string'?item:JSON.stringify(item))}</li>`).join('')}</ul>`:'<p>尚未记录具体条件。</p>';}
function renderProblemDetail(problem,evidence,revisionHistory){
  const next=evidence.page?.nextCursor;
  const revision=problem.revisionRef?` · 修订 ${esc(problem.revisionRef)}`:'';
  const readable=problem.definitionReadable===true;
  const definition=readable?`<section><h3>当前定义</h3><p>${esc(problem.definition||'未记录')}</p>${renderProblemIdentity(problem.stableIdentity)}</section><section><h3>纳入依据</h3>${renderProblemCriteria(problem.includeCriteria)}</section><section><h3>排除依据</h3>${renderProblemCriteria(problem.excludeCriteria)}</section>`:'<p class="study-restricted">定义所依赖的种子原声当前受限，定义与条件已隐藏。</p>';
  const revisions=revisionHistory.length?`<details><summary>定义修订记录（${revisionHistory.length}）</summary><ol>${revisionHistory.map(item=>`<li>版本 ${Number(item.identityVersion)} · ${esc(String(item.createdAt||'').replace('T',' '))} · ${esc(item.revisionRef)}</li>`).join('')}</ol></details>`:'';
  return `<section class="study-problem-detail" aria-label="用户问题详情"><header><h2 tabindex="-1">${esc(problem.title||problem.definition||'定义当前不可读取')}</h2><button type="button" class="study-link" data-problem-close>返回问题列表</button></header><p class="study-signal-meta">${esc(label(problemStateLabel,problem.state)??problem.state)}${revision} · 支持评论 ${Number(problem.supportCommentCount||0)} 条 · 独立作者 ${Number(problem.supportAuthorCount||0)} 位 · 作品 ${Number(problem.supportWorkCount||0)} 篇${problem.definitionCurrent===false?' · 当前定义依据已失效':''}</p>${definition}${revisions}<section><h3>原声与关联信号</h3><p class="study-run-page-status">当前显示 ${(evidence.evidence||[]).length} 条${next?'，还有后续内容':'，已到列表末尾'}</p><div id="study-problem-evidence">${list(evidence.evidence,renderProblemEvidence,'当前没有可显示的原声依据。')}</div>${next?`<button type="button" class="study-link" data-problem-evidence-more="${esc(next)}">加载更多原声</button>`:''}</section></section>`;
}

function runRow(run) {
  const canControl = run.selectionContract === 'comment-study.run-selection.v2'
    && !run.finishedAt && run.controlVersion != null
    && Number.isInteger(Number(run.controlVersion))
    && Number(run.controlVersion) >= 0;
  const actions = [];
  if (canControl && run.dispatchState === 'enabled') {
    actions.push(`<button class="study-run-control" type="button" data-run-control="pause" data-run="${esc(run.runRef)}" data-control-version="${Number(run.controlVersion)}">暂停派发</button>`);
  }
  if (canControl && run.dispatchState === 'paused' && run.dispatchReason === 'user_paused') {
    actions.push(`<button class="study-run-control" type="button" data-run-control="resume" data-run="${esc(run.runRef)}" data-control-version="${Number(run.controlVersion)}">恢复派发</button>`);
  }
  if (canControl && ['enabled', 'paused'].includes(run.dispatchState)) {
    const pendingCount = run.pendingCount == null ? '未知' : Number(run.pendingCount);
    actions.push(`<button class="study-run-control study-run-stop" type="button" data-run-control="stop" data-run="${esc(run.runRef)}" data-control-version="${Number(run.controlVersion)}" data-target-count="${Number(run.targetCount)}" data-pending-count="${pendingCount}">停止本次运行</button>`);
  }
  const unfinished = Number(run.failedCount || 0) + Number(run.cancelledCount || 0)
    + Number(run.needsContextCount || 0) + Number(run.excludedCount || 0);
  if (run.selectionContract === 'comment-study.run-selection.v2' && run.finishedAt && unfinished > 0) {
    actions.push(`<button type="button" class="study-link" data-run-recover="${esc(run.runRef)}">补跑未完成</button>`);
  }
  actions.push(`<button type="button" class="study-link" data-run-open="${esc(run.runRef)}">${selectedRunRef === run.runRef ? '当前查看' : '查看结果'}</button>`);
  const dispatchLabel = dispatchStateLabel[run.dispatchState] || '派发状态未知';
  const dispatchReason = dispatchReasonLabel[run.dispatchReason];
  return `<tr>
      <td>${esc(run.runRef.slice(0, 8))}…<p>${esc(run.createdAt)}</p>${run.recoverySourceRunRef ? `<p>补跑自 ${esc(run.recoverySourceRunRef.slice(0, 8))}…</p>` : ''}</td>
      <td>${esc(runStateLabel[run.state] || run.state)}<p>${esc(dispatchLabel)}${dispatchReason ? ` · ${esc(dispatchReason)}` : ''}</p></td>
      <td>${Number(run.workCount)}<p>primary ${Number(run.primaryWorkCount || 0)} · reference ${Number(run.referenceWorkCount || 0)}</p></td>
      <td>${Number(run.targetCount)}</td>
      <td>${Number(run.succeededCount)}</td>
      <td>${Number(run.noSignalCount)}</td>
      <td>${Number(run.needsContextCount)}</td>
      <td>${Number(run.failedCount)}</td>
      <td>${Number(run.excludedCount)}</td>
      <td><div class="study-run-actions">${actions.join('') || '—'}</div></td>
    </tr>`;
}
async function renderRunsTab() {
  if (!runListLoaded || runControlOutcomeNeedsRefresh) await loadRunListPage(true);
  const feedback = '<p id="study-run-control-feedback" class="study-run-control-feedback" role="status" aria-live="polite" tabindex="-1">' + esc(runControlFeedback) + '</p>';
  if (!allRuns.length) return feedback + '<p class="study-empty">尚未创建过研究运行。</p>';
  const table = '<div class="study-review-table-wrap"><table class="study-review-table"><thead><tr><th scope="col">运行</th><th scope="col">状态</th><th scope="col">作品</th><th scope="col">目标</th><th scope="col">有信号</th><th scope="col">无信号</th><th scope="col">等待语境</th><th scope="col">处理失败</th><th scope="col">已排除</th><th scope="col">操作</th></tr></thead><tbody>' + allRuns.map(runRow).join('') + '</tbody></table></div>';
  const more = runListNextCursor
    ? `<p class="study-run-page-status">已显示 ${allRuns.length} 次运行 <button type="button" class="study-link" data-run-list-load-more${runListLoading ? ' disabled' : ''}>${runListLoading ? '读取中…' : '加载更早运行'}</button></p>`
    : `<p class="study-run-page-status">已显示 ${allRuns.length} 次运行，已到列表末尾</p>`;
  let panel;
  try {
    panel = await renderSelectedRunPanel();
  } catch (error) {
    panel = `<p class="study-empty">运行详情读取失败：${esc(error.message)}</p>`;
  }
  return feedback + table + more + panel;
}

async function loadRunListPage(reset = false) {
  if (runListRequest && !reset) return runListRequest;
  if (reset) {
    runListGeneration += 1;
    allRuns = [];
    runListNextCursor = null;
    runListLoaded = false;
  } else if (!runListNextCursor) {
    return;
  }
  const generation = runListGeneration;
  runListLoading = true;
  const cursor = reset ? null : runListNextCursor;
  const request = (async () => {
    const query = new URLSearchParams({ limit: '50' });
    if (cursor) query.set('cursor', cursor);
    const response = await get(`runs?${query}`);
    if (generation !== runListGeneration) return;
    const rows = response.runs || [];
    allRuns = cursor ? [...allRuns, ...rows] : rows;
    runListNextCursor = response.page?.nextCursor || null;
    runListLoaded = true;
    if (!selectedRunRef) selectedRunRef = allRuns[0]?.runRef ?? null;
    renderRunPicker();
  })();
  runListRequest = request;
  try {
    await request;
  } finally {
    if (runListRequest === request) {
      runListLoading = false;
      runListRequest = null;
    }
  }
}

const TAB_RENDERERS = { overview: renderIntelligenceOverviewTab, comments: renderCommentsTab, runs: renderRunsTab, problems: renderProblemsTab };

// A newer render can start (Tab click, run picker change) before an older one's fetch resolves.
// Without this token, a slow response from an abandoned render could overwrite whatever the
// user is looking at now with stale content. Only the render that is still current when its
// fetch resolves is allowed to touch the DOM.
let renderToken = 0;
async function renderActiveTab() {
  const container = document.querySelector('#study-tab-result');
  const token = ++renderToken;
  const view = activeView;
  if (!domainRef) {
    container.innerHTML = '<p class="study-empty">请先在页头选择观察领域，再查看评论研究。</p>';
    container.setAttribute('aria-busy', 'false');
    return true;
  }
  container.setAttribute('aria-busy', 'true');
  let html;
  let renderFailed = false;
  try {
    html = await TAB_RENDERERS[view]();
  } catch (error) {
    renderFailed = true;
    if (view === 'runs') {
      const confirmed = runControlFeedback.startsWith('已按服务端回执');
      const outcome = runControlOutcomeNeedsRefresh
        ? runControlFeedback + (confirmed
          ? ' 运行列表刷新失败；以上为本次命令回执，最新运行状态未重新读取。'
          : ' 运行列表刷新也失败；命令结果未获确认，最新运行状态未重新读取。')
        : '';
      const detail = (outcome ? outcome + ' ' : '运行列表读取失败：') + error.message;
      html = '<p id="study-run-control-feedback" class="study-run-control-feedback" role="status" aria-live="polite" tabindex="-1">' + esc(detail) + '</p>';
    } else {
      html = `<p class="study-empty">读取失败：${esc(error.message)}</p>`;
    }
  }
  if (token !== renderToken) return false;
  container.innerHTML = html;
  container.setAttribute('aria-busy', 'false');
  if (view === 'comments') bindCommentsView();
  if (view === 'runs') bindRunControls(container);
  if (view === 'problems') bindProblemControls(container);
  const focused=container.querySelector('[data-focus="true"]');
  if(focused)requestAnimationFrame(()=>focused.scrollIntoView({block:'center',behavior:'auto'}));
  if (view === 'runs' && !renderFailed) {
    runControlOutcomeNeedsRefresh = false;
    runControlFeedback = '';
  }
  return true;
}

function bindRunControls(container) {
  container.querySelectorAll('[data-request-detail]').forEach(button=>button.addEventListener('click',async()=>{
    const body=button.nextElementSibling;
    if(button.getAttribute('aria-expanded')==='true'){body.hidden=true;button.setAttribute('aria-expanded','false');button.textContent='查看本次请求详情';return;}
    body.hidden=false;body.innerHTML='<p class="study-detail-muted">正在读取本次请求快照…</p>';button.disabled=true;
    const invocationRef=button.dataset.requestDetail,runRef=selectedRunRef;
    try{
      const response=await get(`requests/${encodeURIComponent(invocationRef)}`);
      if(selectedRunRef!==runRef||selectedRunPanel!=='requests'||!button.isConnected)return;
      if(response.request?.invocationRef!==invocationRef||response.request?.runRef!==runRef)throw new Error('请求详情回执与当前运行不一致。');
      body.innerHTML=renderRequestDetail(response.request);button.setAttribute('aria-expanded','true');button.textContent='收起本次请求详情';
    }catch(error){if(button.isConnected)body.innerHTML=`<p class="study-restricted">请求详情读取失败：${esc(error.message)}</p>`;}
    finally{if(button.isConnected)button.disabled=false;}
  }));
  container.querySelectorAll('[data-target-signals]').forEach(button=>button.addEventListener('click',()=>navigateToRun(selectedRunRef,'signals',button.dataset.targetSignals)));
  container.querySelectorAll('[data-target-comment-work][data-target-comment-id]').forEach(button=>button.addEventListener('click',async()=>{activeView='comments';highlightTab(activeView);await renderActiveTab();void openCommentDetail(button.dataset.targetCommentWork,button.dataset.targetCommentId);}));
  container.querySelectorAll('[data-signal-target]').forEach(button=>button.addEventListener('click',()=>navigateToRun(selectedRunRef,'targets',button.dataset.signalTarget)));
  container.querySelectorAll('[data-signal-problem]').forEach(button=>button.addEventListener('click',()=>navigateToProblem(button.dataset.signalProblem)));
  container.querySelectorAll('[data-run-recover]').forEach(button => {
    button.addEventListener('click', () => {
      const run = allRuns.find(item => item.runRef === button.dataset.runRecover);
      if (!run) return;
      recoveryRunRef = run.runRef;
      const limits = run.limits || {};
      document.querySelector('#study-recover-description').textContent =
        `源 Run ${run.runRef}。仅从本次未完成评论中重新核对当前资格，跳过后续已成功、在途、受限或仍不可恢复的评论；沿用原方法与本次最多 ${Number(limits.commentBudget)} 条、${Number(limits.tokenLimit)} 词元预算。新 Run 会保留源 Run 关联，原记录不改写。`;
      document.querySelector('#study-recover-error').textContent = '';
      document.querySelector('#study-recover-confirm').disabled = false;
      document.querySelector('#study-recover-dialog').showModal();
      document.querySelector('#study-recover-dismiss').focus();
    });
  });
  container.querySelectorAll('[data-run-control="stop"]').forEach(button => {
    button.addEventListener('click', () => {
      stopDialogRunRef = button.dataset.run;
      stopDialogControlVersion = Number(button.dataset.controlVersion);
      const pendingLabel = button.dataset.pendingCount === '未知'
        ? '未知' : `${Number(button.dataset.pendingCount)} 条`;
      document.querySelector('#study-stop-run-identity').textContent =
        `Run ${stopDialogRunRef} · 当前未终态 ${pendingLabel} · 目标总数 ${Number(button.dataset.targetCount)} 条`;
      document.querySelector('#study-stop-error').textContent = '';
      const dialog = document.querySelector('#study-stop-dialog');
      const confirm = document.querySelector('#study-stop-confirm');
      confirm.disabled = false;
      confirm.textContent = '确认停止';
      dialog.showModal();
      document.querySelector('#study-stop-dismiss').focus();
    });
  });
  container.querySelectorAll('[data-run-control="pause"], [data-run-control="resume"]').forEach(button => {
    button.addEventListener('click', () => submitRunControl(button));
  });
  container.querySelectorAll('[data-run-open]').forEach(button => {
    button.addEventListener('click', async () => {
      const requestedRun = button.dataset.runOpen;
      selectedRunRef = requestedRun;
      selectedRunPanel = 'targets';
      focusedTargetRef = null;
      focusedSignalRef = null;
      resetRunPanelCache();
      renderRunPicker();
      syncStudyRoute(true);
      const rendered = await renderActiveTab();
      if (!rendered || activeView !== 'runs' || selectedRunRef !== requestedRun) return;
      const detail = container.querySelector('.study-run-detail');
      if (!detail) return;
      const reduceMotion = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false;
      detail.scrollIntoView({ behavior: reduceMotion ? 'auto' : 'smooth', block: 'start' });
      detail.querySelector('h2')?.focus({ preventScroll: true });
    });
  });
  container.querySelectorAll('[data-run-panel]').forEach(button => {
    button.addEventListener('click', async () => {
      selectedRunPanel = button.dataset.runPanel;
      focusedTargetRef = null;
      focusedSignalRef = null;
      syncStudyRoute(true);
      await renderActiveTab();
    });
  });
  container.querySelector('[data-run-load-more]')?.addEventListener('click', async event => {
    const button = event.currentTarget;
    const requestedRun = selectedRunRef;
    const requestedPanel = selectedRunPanel;
    button.disabled = true;
    try {
      await loadRunPanelPage(requestedPanel);
      if (requestedRun === selectedRunRef && requestedPanel === selectedRunPanel) await renderActiveTab();
    } catch (error) {
      button.disabled = false;
      button.textContent = `读取下一页失败：${error.message}`;
    }
  });
  container.querySelector('[data-run-list-load-more]')?.addEventListener('click', async event => {
    const button = event.currentTarget;
    button.disabled = true;
    try {
      await loadRunListPage();
      await renderActiveTab();
    } catch (error) {
      button.disabled = false;
      button.textContent = `读取更早运行失败：${error.message}`;
    }
  });
}

async function onProblemEvidenceClick(event){
  const button=event.target.closest('[data-evidence-work][data-evidence-id]');
  if(!button)return;
  activeView='comments';highlightTab(activeView);
  await renderActiveTab();
  void openCommentDetail(button.dataset.evidenceWork,button.dataset.evidenceId);
}
function bindProblemControls(container){
  container.querySelector('#problem-candidate-state')?.addEventListener('change',async event=>{problemCandidateState.state=event.currentTarget.value;problemCandidateState.loaded=false;problemCandidateState.items=[];problemCandidateState.nextCursor=null;await renderActiveTab();});
  container.querySelector('[data-candidate-more]')?.addEventListener('click',async event=>{const button=event.currentTarget;button.disabled=true;try{await loadProblemCandidatePage();await renderActiveTab();}catch(error){button.disabled=false;button.textContent=`读取失败，重试：${error.message}`;}});
  container.querySelectorAll('[data-candidate-signal]').forEach(button=>button.addEventListener('click',()=>navigateToRun(button.dataset.candidateRun,'signals',button.dataset.candidateTarget||null,button.dataset.candidateSignal)));
  container.querySelectorAll('[data-problem-open]').forEach(button=>button.addEventListener('click',()=>navigateToProblem(button.dataset.problemOpen)));
  container.querySelector('[data-problem-list-more]')?.addEventListener('click',async event=>{const button=event.currentTarget;button.disabled=true;try{await loadProblemPage();await renderActiveTab();}catch(error){button.textContent=`读取失败，重试：${error.message}`;button.disabled=false;}});
  container.querySelector('[data-problem-close]')?.addEventListener('click',()=>{selectedProblemRef=null;syncStudyRoute(true);void renderActiveTab();});
  container.removeEventListener('click',onProblemEvidenceClick);
  container.addEventListener('click',onProblemEvidenceClick);
  container.querySelector('[data-problem-evidence-more]')?.addEventListener('click',async event=>{
    const button=event.currentTarget,problemRef=selectedProblemRef;
    button.disabled=true;
    try{
      const data=await get(`problems/${encodeURIComponent(problemRef)}/evidence?limit=50&cursor=${encodeURIComponent(button.dataset.problemEvidenceMore)}`);
      if(problemRef!==selectedProblemRef||data.problemRef!==problemRef)return;
      document.querySelector('#study-problem-evidence')?.insertAdjacentHTML('beforeend',(data.evidence||[]).map(renderProblemEvidence).join(''));
      if(data.page?.nextCursor){button.dataset.problemEvidenceMore=data.page.nextCursor;button.disabled=false;}else button.remove();
    }catch(error){button.textContent=`读取失败，重试：${error.message}`;button.disabled=false;}
  });
}
async function navigateToRun(runRef,panel='targets',targetRef=null,signalRef=null){
  if(!runRef)return;
  const dialog=document.querySelector('#comment-detail-dialog');if(dialog.open)dialog.close();++commentDetailRequest;
  activeView='runs';selectedRunRef=runRef;selectedRunPanel=RUN_PANELS.has(panel)?panel:'targets';
  focusedTargetRef=targetRef||null;focusedSignalRef=signalRef||null;
  resetRunPanelCache();highlightTab(activeView);renderRunPicker();syncStudyRoute(true);
  await renderActiveTab();
}
async function navigateToProblem(problemRef){
  if(!problemRef)return;
  const dialog=document.querySelector('#comment-detail-dialog');if(dialog.open)dialog.close();++commentDetailRequest;
  if(activeView!=='problems')problemListCache.loaded=false;
  activeView='problems';selectedProblemRef=problemRef;highlightTab(activeView);syncStudyRoute(true);
  await renderActiveTab();
}
async function navigateToComment(workRef,commentExternalId){
  if(!workRef||!commentExternalId)return;
  activeView='comments';highlightTab(activeView);
  await renderActiveTab();
  await openCommentDetail(workRef,commentExternalId);
}

async function postRunControl(runRef, action, expectedControlVersion) {
  const response = await fetch(`${endpoint}runs/${encodeURIComponent(runRef)}/${action}`, {
    method: 'POST',
    headers: { Accept: 'application/json', 'Content-Type': 'application/json' },
    body: JSON.stringify({ expectedControlVersion })
  });
  const receipt = await response.json();
  if (!response.ok) {
    const error = new Error(receipt.error?.message || `请求失败（状态码 ${response.status}）`);
    error.status = response.status;
    throw error;
  }
  if (receipt.data?.runRef !== runRef
      || Number(receipt.data?.controlVersion) !== expectedControlVersion + 1
      || receipt.data?.dispatchState !== ({ pause: 'paused', resume: 'enabled', stop: 'stopped' })[action]
      || receipt.data?.dispatchReason !== ({ pause: 'user_paused', resume: null, stop: 'user_stopped' })[action]) {
    throw new Error('服务回执与本次控制命令不匹配。');
  }
  return receipt.data;
}

async function submitRunControl(button) {
  const runRef = button.dataset.run;
  const action = button.dataset.runControl;
  const expectedControlVersion = Number(button.dataset.controlVersion);
  const feedback = document.querySelector('#study-run-control-feedback');
  button.disabled = true;
  runControlFeedback = action === 'pause' ? '正在暂停派发…' : '正在恢复派发…';
  if (feedback) feedback.textContent = runControlFeedback;
  try {
    await postRunControl(runRef, action, expectedControlVersion);
    runControlFeedback = action === 'pause' ? '已按服务端回执暂停派发。' : '已按服务端回执恢复派发。';
    runControlOutcomeNeedsRefresh = true;
    await renderActiveTab();
    const nextAction = action === 'pause' ? 'resume' : 'pause';
    const nextControl = document.querySelector('#study-tab-result [data-run="' + CSS.escape(runRef) + '"][data-run-control="' + nextAction + '"]');
    if (nextControl) nextControl.focus();
    else document.querySelector('#study-run-control-feedback')?.focus();
  } catch (error) {
    runControlFeedback = `控制未获确认：${error.message}`;
    runControlOutcomeNeedsRefresh = true;
    await renderActiveTab();
    document.querySelector('#study-run-control-feedback')?.focus();
  }
}

async function submitRunStop() {
  if (!stopDialogRunRef || stopDialogControlVersion == null) return;
  const runRef = stopDialogRunRef;
  const expectedControlVersion = stopDialogControlVersion;
  const dialog = document.querySelector('#study-stop-dialog');
  const confirm = document.querySelector('#study-stop-confirm');
  const status = document.querySelector('#study-stop-error');
  confirm.disabled = true;
  confirm.textContent = '正在提交…';
  status.textContent = '';
  try {
    await postRunControl(runRef, 'stop', expectedControlVersion);
    dialog.close();
    stopDialogRunRef = null;
    stopDialogControlVersion = null;
    runControlFeedback = '已按服务端回执停止本次运行。';
    runControlOutcomeNeedsRefresh = true;
    await renderActiveTab();
    document.querySelector('#study-run-control-feedback')?.focus();
  } catch (error) {
    dialog.close();
    stopDialogRunRef = null;
    stopDialogControlVersion = null;
    runControlFeedback = `停止未获确认：${error.message}；已重新读取当前运行状态，请核对后再操作。`;
    runControlOutcomeNeedsRefresh = true;
    await renderActiveTab();
    document.querySelector('#study-run-control-feedback')?.focus();
  }
}

async function submitRunRecovery() {
  if (!recoveryRunRef) return;
  const sourceRunRef = recoveryRunRef;
  const confirm = document.querySelector('#study-recover-confirm');
  const status = document.querySelector('#study-recover-error');
  confirm.disabled = true;
  status.textContent = '正在重新核对并创建运行…';
  const requestRef = recoveryRequestRefs.get(sourceRunRef) || crypto.randomUUID();
  recoveryRequestRefs.set(sourceRunRef, requestRef);
  try {
    const receipt = await post(`runs/${encodeURIComponent(sourceRunRef)}/recover`, { requestRef });
    recoveryRequestRefs.delete(sourceRunRef);
    document.querySelector('#study-recover-dialog').close();
    recoveryRunRef = null;
    runControlFeedback = receipt.outcome === 'created'
      ? `已按服务端回执创建补跑 Run ${receipt.runRef}，本次冻结 ${Number(receipt.targetCount)} 条。`
      : '本次没有仍可恢复的评论；未创建空运行。';
    runControlOutcomeNeedsRefresh = true;
    if (receipt.runRef) { selectedRunRef = receipt.runRef; selectedRunPanel = 'targets'; resetRunPanelCache(); syncStudyRoute(true); }
    await renderActiveTab();
    document.querySelector('#study-run-control-feedback')?.focus();
  } catch (error) {
    status.textContent = `补跑未获确认：${error.message}。重试将沿用本次请求编号。`;
    confirm.disabled = false;
  }
}

document.querySelector('#study-recover-confirm').addEventListener('click', submitRunRecovery);
document.querySelector('#study-recover-dismiss').addEventListener('click', () => {
  document.querySelector('#study-recover-dialog').close();
  recoveryRunRef = null;
});
document.querySelector('#study-recover-dialog').addEventListener('cancel', event => {
  if (document.querySelector('#study-recover-confirm').disabled) event.preventDefault();
  else recoveryRunRef = null;
});

document.querySelector('#study-stop-confirm').addEventListener('click', submitRunStop);
document.querySelector('#study-stop-dismiss').addEventListener('click', () => {
  if (!document.querySelector('#study-stop-confirm').disabled) {
    document.querySelector('#study-stop-dialog').close();
    stopDialogRunRef = null;
    stopDialogControlVersion = null;
  }
});
document.querySelector('#study-stop-dialog').addEventListener('cancel', event => {
  if (document.querySelector('#study-stop-confirm').disabled) event.preventDefault();
  else { stopDialogRunRef = null; stopDialogControlVersion = null; }
});

async function loadProjection() {
  if (domainRef) {
    try {
      await loadRunListPage(true);
    } catch (error) { allRuns = []; }
  }
  highlightTab(activeView);
  syncStudyRoute(false);
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
  if(activeView==='comments')window.history.replaceState({...window.history.state,commentFilter:captureCommentFilter(),commentScrollY:window.scrollY},'',window.location.href);
  if(view==='problems')problemListCache.loaded=false;
  activeView = view;
  highlightTab(view);
  renderRunPicker();
  syncStudyRoute(true);
  await renderActiveTab();
}
document.querySelectorAll('.study-tabs button').forEach(button => button.addEventListener('click', () => switchToView(button.dataset.view)));
document.querySelector('#study-run-select').addEventListener('change', async event => {
  selectedRunRef = event.currentTarget.value || null;
  resetRunPanelCache();
  syncStudyRoute(true);
  await renderActiveTab();
});
window.addEventListener('popstate', () => {
  const params = new URLSearchParams(window.location.search);
  const view = params.get('view');
  activeView = view==='targets'?'runs':view==='pending'?'problems':validStudyViews.has(view) ? view : 'overview';
  selectedRunRef = params.get('runRef');
  selectedRunPanel = view==='targets'?'targets':RUN_PANELS.has(params.get('panel')) ? params.get('panel') : 'targets';
  focusedTargetRef=params.get('targetRef');
  focusedSignalRef=params.get('signalRef');
  selectedProblemRef=params.get('problemRef');
  if(activeView==='comments')restoreCommentRoute(params,window.history.state?.commentFilter);
  const dialog=document.querySelector('#comment-detail-dialog');
  if(dialog.open){dialog.close();++commentDetailRequest;}
  resetRunPanelCache();
  highlightTab(activeView);
  renderRunPicker();
  void loadProjection().then(()=>{
    const workRef=params.get('commentWorkRef')||(params.get('detail')==='comment'?params.get('workRef'):null),commentId=params.get('commentExternalId');
    if(activeView==='comments'&&workRef&&commentId)void openCommentDetail(workRef,commentId,{push:false});
    else if(activeView==='comments'&&Number.isFinite(window.history.state?.commentScrollY))window.scrollTo({top:window.history.state.commentScrollY,behavior:'auto'});
  });
});
const studyDialog = document.querySelector('#study-dialog');
function setStudySelectionKind(kind){
  studySelectionKind=kind;
  document.querySelector('.study-workspace').dataset.selectionKind=kind;
  document.querySelector('#study-dialog-title').textContent=kind==='comments'?'研究所选评论':'发起一次新研究';
  document.querySelector('#work-picker-title').textContent=kind==='comments'?'已选评论':'选择要研究的作品';
  document.querySelector('#selection-scope-note').textContent=kind==='comments'?'只冻结当前已选、服务端确认可研究的评论；预览会再次核对资格和预算。创建研究运行不会直接调用模型。':'只冻结当前所选作品中、受评论数上限约束的可研究评论；创建研究运行不会调用模型。';
  document.querySelector('#selection-preview').textContent='';
  document.querySelector('#run-result').textContent='';
  pendingStartSignature=null;pendingStartRef=null;
  updateSelection();
}
document.querySelector('#open-study-dialog').addEventListener('click', () => {setStudySelectionKind('works');studyDialog.showModal();});
document.querySelector('#study-dialog-close').addEventListener('click', () => studyDialog.close());
document.querySelector('#edit-policy').addEventListener('click', openPolicyEditor);
document.querySelector('#cancel-policy-edit').addEventListener('click', () => {
  closePolicyEditor();
  document.querySelector('#policy-status').textContent='';
});
document.querySelector('#policy-form').addEventListener('submit', async event => {
  event.preventDefault();
  const button = document.querySelector('#save-policy');
  const status = document.querySelector('#policy-status');
  let policySaved = false;
  button.disabled = true;
  try {
    const response = await post('policies', {
      methodName: document.querySelector('#method-name').value,
      parentPolicyRef,
      modelConfigRef: document.querySelector('#model-config').value,
      defaults: { commentBudget: Number(document.querySelector('#comment-budget').value), contextCharacterBudget: Number(document.querySelector('#context-character-budget').value) },
      stageInstructions: { semantic: document.querySelector('#stage-semantic').value, resolution: document.querySelector('#stage-resolution').value, pair: document.querySelector('#stage-pair').value }
    });
    policySaved = true;
    const reference = response.policy?.policyRef;
    if (!reference) {
      status.textContent = '方法已保存，但保存回执缺少方法版本编号，未能自动选中此版本。';
      return;
    }
    await loadPolicies(reference);
    closePolicyEditor();
    status.textContent = `已保存并选中新方法版本：${reference}。可直接用于本次预览和启动。`;
  } catch (error) {
    status.textContent = policySaved
      ? `方法已保存，但方法目录刷新失败：${error.message}`
      : `未保存方法版本：${error.message}`;
  }
  finally { button.disabled = !document.querySelector('#model-config').value; }
});
document.querySelector('#study-policy').addEventListener('change', () => {
  document.querySelector('#activate-policy').disabled = !document.querySelector('#study-policy').value || document.querySelector('#study-policy').value === activePolicyRef;
  updatePolicySummary();
  pendingStartSignature = null; pendingStartRef = null; updateSelection();
});
document.querySelector('#activate-policy').addEventListener('click', async () => {
  const reference = document.querySelector('#study-policy').value;
  if (!reference) return;
  const status = document.querySelector('#policy-status');
  const button = document.querySelector('#activate-policy');
  button.disabled = true;
  try {
    await post(`policies/${encodeURIComponent(reference)}/activate?domain=${encodeURIComponent(domainRef)}`, { expectedActivePolicyRef: activePolicyRef }, false);
    status.textContent = '已将所选方法设为当前领域默认版本。';
    await loadPolicies(reference);
  } catch (error) { status.textContent = `未能切换默认方法：${error.message}`; button.disabled = false; }
});
document.querySelector('#study-mode').addEventListener('change', event => {
  const needsReason = event.currentTarget.value !== 'new_only';
  document.querySelector('#study-reason-label').hidden = !needsReason;
  pendingStartSignature = null; pendingStartRef = null;
});
function selectionCommand() {
  if(studySelectionKind==='comments'){
    const selected=[...selectedCommentKeys.values()];
    const roles=new Map(selected.map(item=>[item.commentKey.workRef,item.observationRole]));
    return{
      domainRef,policyRef:document.querySelector('#study-policy').value,
      scope:{kind:'comments',commentKeys:selected.map(item=>item.commentKey)},
      workRoles:[...roles].map(([contentPublicRef,observationRole])=>({contentPublicRef,observationRole})),
      mode:document.querySelector('#study-mode').value,
      limits:{commentBudget:Number(document.querySelector('#run-comment-budget').value),contextCharacterBudget:Number(document.querySelector('#run-context-character-budget').value),tokenLimit:Number(document.querySelector('#token-budget').value)}
    };
  }
  const works = selectedWorks();
  return {
    domainRef,
    policyRef: document.querySelector('#study-policy').value,
    scope: { kind: 'works', workRefs: works.map(work => work.contentPublicRef) },
    workRoles: works,
    mode: document.querySelector('#study-mode').value,
    limits: { commentBudget: Number(document.querySelector('#run-comment-budget').value), contextCharacterBudget: Number(document.querySelector('#run-context-character-budget').value), tokenLimit: Number(document.querySelector('#token-budget').value) }
  };
}
document.querySelector('#preview-run').addEventListener('click', async () => {
  const status = document.querySelector('#selection-preview');
  const button = document.querySelector('#preview-run');
  button.disabled = true;
  try {
    const preview = await post('selection-preview', selectionCommand());
    status.textContent = `预览：所选 ${Number(preview.requestedWorkCount||0)} 篇作品，${Number(preview.scopeCommentCount||0)} 条评论符合范围，预计创建 ${Number(preview.targetCount||0)} 条目标；待索引 ${Number(preview.indexCoverage?.pendingCount||0)} 条。预览不保留评论，也不调用模型。`;
  } catch (error) { status.textContent = `预览失败：${error.message}`; }
  finally { updateSelection(); }
});
document.querySelector('#start-run').addEventListener('click', async () => {
  const button = document.querySelector('#start-run');
  const result = document.querySelector('#run-result');
  button.disabled = true;
  try {
    const command = selectionCommand();
    const mode = command.mode;
    command.reason = mode === 'new_only' ? null : document.querySelector('#study-reason').value;
    const signature = JSON.stringify(command);
    if (pendingStartSignature !== signature || !pendingStartRef) {
      pendingStartSignature = signature;
      pendingStartRef = crypto.randomUUID();
    }
    command.requestRef = pendingStartRef;
    const response = await post('runs', command);
    pendingStartSignature = null; pendingStartRef = null;
    result.textContent = response.outcome === 'created'
      ? `已创建 ${response.runRef}：覆盖 ${Number(response.coveredWorkCount||0)} 篇作品，冻结 ${Number(response.targetCount||0)} 条目标评论。当前运行已排队；实际模型调用由后续执行层控制。`
      : `本次未创建空运行：${response.outcome==='no_work'?'没有符合所选模式的新评论':'仍有评论等待索引'}。 requestRef=${response.requestRef}`;
    selectedRunRef = response.runRef;
    if(response.runRef){studyDialog.close(); activeView = 'runs'; selectedRunPanel = 'targets'; resetRunPanelCache(); highlightTab('runs'); syncStudyRoute(true); await loadProjection();}
  } catch (error) { result.textContent = `未创建研究运行：${error.message}`; }
  finally { updateSelection(); }
});
document.querySelector('#work-filter').addEventListener('input',()=>{clearTimeout(workSearchTimer);workSearchTimer=setTimeout(()=>{void searchWorksNow().catch(error=>{document.querySelector('#work-filter-status').textContent=`作品搜索失败：${error.message}`;});},250);});
document.querySelector('#work-prev').addEventListener('click',async()=>{const previous=workCatalogState.history.pop()??null;await loadWorksPage(previous);});
document.querySelector('#work-next').addEventListener('click',async()=>{if(!workCatalogState.nextCursor)return;workCatalogState.history.push(workCatalogState.cursor);await loadWorksPage(workCatalogState.nextCursor);});
['#run-comment-budget','#run-context-character-budget'].forEach(selector => document.querySelector(selector).addEventListener('input', () => {
  pendingStartSignature = null; pendingStartRef = null; updateSelection();
}));
document.querySelector('#token-budget').addEventListener('input', () => { pendingStartSignature = null; pendingStartRef = null; updateSelection(); });
document.querySelector('#study-reason').addEventListener('input', () => { pendingStartSignature = null; pendingStartRef = null; });
document.querySelector('#include-reference-works').addEventListener('change', async event => {
  workCatalogState.observationRole = event.currentTarget.checked ? 'reference' : 'primary';
  workCatalogState.cursor = null;
  workCatalogState.history = [];
  try { await loadWorksPage(null); }
  catch (error) { document.querySelector('#work-filter-status').textContent = `作品目录读取失败：${error.message}`; }
});
document.querySelector('#select-visible-works').addEventListener('change', event => {
  let remaining = MAX_SELECTED_WORKS - selectedWorkRoles.size;
  visibleWorks().forEach(work => {
    const role = work.observationRole || workCatalogState.observationRole;
    if (event.currentTarget.checked && selectedWorkRoles.get(work.workRef) !== role && remaining > 0) {
      selectedWorkRoles.set(work.workRef, role);
      selectedWorkMeta.set(work.workRef, { eligibleCommentCount: Number(work.eligibleCommentCount || 0) });
      remaining -= 1;
    } else if (!event.currentTarget.checked && selectedWorkRoles.get(work.workRef) === role) {
      selectedWorkRoles.delete(work.workRef);
      selectedWorkMeta.delete(work.workRef);
    }
  });
  renderWorks();
});
document.querySelector('#comment-detail-close').addEventListener('click',()=>closeCommentDetail());
document.querySelector('#comment-detail-dialog').addEventListener('cancel',event=>{event.preventDefault();closeCommentDetail();});
document.querySelector('#comment-detail-body').addEventListener('click',async event=>{
  const history=event.target.closest('[data-history-run]');
  if(history){await navigateToRun(history.dataset.historyRun,history.dataset.historyPanel,history.dataset.historyTarget||null);return;}
  const parent=event.target.closest('[data-detail-parent-work][data-detail-parent-id]');
  if(parent){void openCommentDetail(parent.dataset.detailParentWork,parent.dataset.detailParentId);return;}
  const work=event.target.closest('[data-detail-work]');
  if(work){const dialog=document.querySelector('#comment-detail-dialog');if(dialog.open)dialog.close();++commentDetailRequest;commentCatalogState.workRef=work.dataset.detailWork;commentCatalogState.workLabel=work.dataset.detailWorkTitle||'';commentCatalogState.workChoices=[{workRef:commentCatalogState.workRef,displayTitle:commentCatalogState.workLabel}];resetCommentPage();const url=new URL(window.location.href);url.searchParams.delete('commentWorkRef');url.searchParams.delete('commentExternalId');url.searchParams.delete('detail');url.searchParams.set('workRef',commentCatalogState.workRef);url.searchParams.delete('cursor');window.history.pushState({...window.history.state,commentDetail:false,commentFilter:captureCommentFilter()},'',url);await renderActiveTab();}
});
/* COMMENT-STUDY-INTELLIGENCE-OVERVIEW-002 */
  const overviewDomainRef = new URLSearchParams(window.location.search).get('domain');
  const overviewState = {method:'solution',seriesDays:28,seriesView:window.matchMedia('(max-width:620px)').matches?'table':'chart',cache:null};
  const overviewNum = value => value == null || Number.isNaN(Number(value)) ? '—' : new Intl.NumberFormat('zh-CN').format(Number(value));
  const overviewPath = (path,extra={}) => {const query=new URLSearchParams();if(overviewDomainRef)query.set('domain',overviewDomainRef);Object.entries(extra).forEach(([key,value])=>{if(value!=null&&value!=='')query.set(key,value)});return query.size?`${path}?${query}`:path};
  const overviewReadout = (text,value,unit,note,active=false)=>`<div class="study-readout" data-active="${active}"><span class="study-readout-label">${esc(text)}</span><div class="study-readout-value"><b>${esc(value)}</b>${unit?`<span>${esc(unit)}</span>`:''}</div><span class="study-readout-note">${esc(note)}</span></div>`;
  const coverageLabel=state=>({recorded:'有观察记录',partial:'部分观察',none:'无观察记录'}[state]||'观察状态未知');
  const observationPoints=series=>(Array.isArray(series?.points)?series.points:[]).slice(-overviewState.seriesDays);
  function renderObservationChart(points,series){if(!points.length)return'<p class="study-observation-empty">当前没有可读取的评论观察序列。</p>';const width=1000,height=286,left=48,right=18,top=22,bottom=42,plotWidth=width-left-right,plotHeight=height-top-bottom,maximum=Math.max(1,...points.flatMap(point=>[Number(point.newObservedCommentCount||0),Number(point.acceptedSignalCount||0)])),step=plotWidth/Math.max(points.length,1),barWidth=Math.max(3,Math.min(18,step*.56)),x=index=>left+step*index+step/2,y=value=>top+plotHeight-Number(value||0)/maximum*plotHeight;const grid=[0,.25,.5,.75,1].map(ratio=>{const yy=top+plotHeight-plotHeight*ratio;return`<g><line x1="${left}" y1="${yy}" x2="${width-right}" y2="${yy}" class="study-series-grid-line"/><text x="${left-9}" y="${yy+4}" text-anchor="end">${overviewNum(Math.round(maximum*ratio))}</text></g>`}).join('');const bars=points.map((point,index)=>{const value=Number(point.newObservedCommentCount||0),yy=y(value);return`<rect x="${(x(index)-barWidth/2).toFixed(2)}" y="${yy.toFixed(2)}" width="${barWidth.toFixed(2)}" height="${Math.max(0,top+plotHeight-yy).toFixed(2)}"><title>${esc(point.date)} · 首次观察评论 ${overviewNum(value)} 条</title></rect>`}).join('');const linePoints=points.map((point,index)=>`${x(index).toFixed(2)},${y(point.acceptedSignalCount).toFixed(2)}`).join(' ');const dots=points.map((point,index)=>`<circle cx="${x(index).toFixed(2)}" cy="${y(point.acceptedSignalCount).toFixed(2)}" r="3"><title>${esc(point.date)} · 接纳信号 ${overviewNum(point.acceptedSignalCount)} 条</title></circle>`).join('');const ticks=points.map((point,index)=>{const every=points.length<=7?1:points.length<=28?4:8;if(index!==0&&index!==points.length-1&&index%every!==0)return'';return`<text x="${x(index).toFixed(2)}" y="${height-14}" text-anchor="middle">${esc(String(point.date).slice(5))}</text>`}).join('');const coverage=points.map(point=>`<i data-state="${esc(point.observationCoverage)}" title="${esc(point.date)} · ${esc(coverageLabel(point.observationCoverage))}"></i>`).join('');return`<div class="study-series-chart"><div class="study-series-legend"><span><i data-kind="comments"></i>首次观察评论</span><span><i data-kind="signals"></i>接纳信号</span><span>按 ${esc(series?.timezone||'Asia/Shanghai')} 自然日</span></div><div class="study-series-svg-wrap" role="img" aria-label="最近 ${overviewState.seriesDays} 天评论观察量。柱状为首次观察评论，折线为接纳信号。"><svg viewBox="0 0 ${width} ${height}" preserveAspectRatio="none" aria-hidden="true">${grid}<g class="study-series-bars">${bars}</g><polyline class="study-series-line" points="${linePoints}"/><g class="study-series-dots">${dots}</g><g class="study-series-axis">${ticks}</g></svg></div><div class="study-series-coverage" style="--study-series-columns:${points.length}" aria-label="每日观察覆盖">${coverage}</div></div>`}
  function renderObservationTable(points){if(!points.length)return'<p class="study-observation-empty">当前没有可读取的评论观察序列。</p>';return`<div class="study-series-table-wrap"><table class="study-series-table"><thead><tr><th scope="col">日期</th><th scope="col">首次观察评论</th><th scope="col">进入研究评论</th><th scope="col">接纳信号</th><th scope="col">覆盖作品</th><th scope="col">观察覆盖</th></tr></thead><tbody>${[...points].reverse().map(point=>`<tr><td><time datetime="${esc(point.date)}">${esc(point.date)}</time></td><td>${overviewNum(point.newObservedCommentCount)}</td><td>${overviewNum(point.studiedCommentCount)}</td><td>${overviewNum(point.acceptedSignalCount)}</td><td>${overviewNum(point.coveredWorkCount)}</td><td><span class="study-coverage-state" data-state="${esc(point.observationCoverage)}">${esc(coverageLabel(point.observationCoverage))}</span></td></tr>`).join('')}</tbody></table></div>`}
  function renderObservationSeries(series){const points=observationPoints(series);const body=series?(overviewState.seriesView==='table'?renderObservationTable(points):renderObservationChart(points,series)):'<p class="study-observation-empty">当前领域没有可读取的评论观察序列。</p>';return`<section class="study-observation" aria-labelledby="study-observation-title"><header class="study-observation-head"><div><h2 id="study-observation-title">评论观察量</h2><p>柱状记录评论首次被系统接受观察的日期；折线记录 Signal 实际写入日期。没有观察记录不等于用户没有表达。</p></div><div class="study-observation-controls"><div class="study-text-switcher" role="group" aria-label="观察时间范围">${[7,28,56].map(days=>`<button type="button" data-series-days="${days}" aria-pressed="${overviewState.seriesDays===days}">近 ${days} 天</button>`).join('')}</div><div class="study-text-switcher" role="group" aria-label="评论观察量视图"><button type="button" data-series-view="chart" aria-pressed="${overviewState.seriesView==='chart'}">图表</button><button type="button" data-series-view="table" aria-pressed="${overviewState.seriesView==='table'}">表格</button></div></div></header>${body}<p class="study-series-note">${series?.points?.length?`序列截至 ${esc(series.points.at(-1).date)}（${esc(series.timezone||'Asia/Shanghai')}）。`:''}“首次观察评论”按稳定评论身份去重；“进入研究评论”按 Target source 去重；观察覆盖来自 comments / replies 采集台账。</p></section>`}
  function overviewVoiceCard(item){
    const readable=item?.sourceState==='known',key=item?.commentKey||{};
    return `<article class="study-voice">${readable?`<blockquote>“${esc(item.commentText||'原声暂不可读')}”</blockquote>${item.proposition?`<p>${esc(item.proposition)}</p>`:''}`:'<p class="study-restricted">来源当前受限，原声与研究衍生文本不显示。</p>'}<div class="study-voice-footer"><span>${esc(item.workTitle||'来源作品')}</span>${key.workRef&&key.commentExternalId?`<button class="study-link" type="button" data-overview-comment-work="${esc(key.workRef)}" data-overview-comment-id="${esc(key.commentExternalId)}">查看评论及父语境</button>`:''}</div></article>`;
  }
  function renderIntelligenceOverview(data){
    const overview=data.overview;
    if(!overview)return '<p class="study-restricted">总览读取失败；其他页签可单独查看已有研究结果。</p>';
    const corpus=overview.corpusSummaryState==='known'?overview.corpusSummary:null;
    const research=overview.researchSummary||null;
    const knowledge=overview.knowledgeSummary||null;
    const problems=Array.isArray(knowledge?.problemSupportPreview)?knowledge.problemSupportPreview:[];
    const voices=Array.isArray(knowledge?.voicePreview)?knowledge.voicePreview:[];
    const methodKind=overviewState.method;
    const methods=Array.isArray(knowledge?.[methodKind==='solution'?'solutionPreview':'experiencePreview'])?knowledge[methodKind==='solution'?'solutionPreview':'experiencePreview']:[];
    const methodCount=knowledge?.[methodKind==='solution'?'solutionSignalCount':'experienceSignalCount'];
    const methodWorkCount=knowledge?.[methodKind==='solution'?'solutionWorkCount':'experienceWorkCount'];
    const pending=[
      ['deferred_novel','新表达，尚未建档','deferred_novel'],
      ['deferred_ambiguous','归并边界尚未确定','deferred_ambiguous'],
      ['deferred_context','等待语境','deferred_context'],
      ['retrieval_incomplete','候选检索未完成','retrieval_incomplete'],
      ['pending','归并仍在处理','pending'],
      ['budget_stopped','预算中止',null],
      ['protocol_rejected','协议拒绝',null],
      ['failed','归并处理失败',null]
    ].map(([state,title,candidate])=>({state,title,candidate,count:knowledge?.[{deferred_novel:'deferredNovelCount',deferred_ambiguous:'deferredAmbiguousCount',deferred_context:'deferredContextCount',retrieval_incomplete:'retrievalIncompleteCount',pending:'pendingResolutionCount',budget_stopped:'budgetStoppedCount',protocol_rejected:'protocolRejectedCount',failed:'failedResolutionCount'}[state]]})).filter(item=>Number(item.count)>0);
    const scope=overview.latestRun?`最近一次研究 · ${String(overview.latestRun.createdAt||'').slice(0,16).replace('T',' ')}`:'当前领域尚无研究运行';
    const asOf=overview.asOf?`截至 ${String(overview.asOf).replace('T',' ')}`:'汇总时间未记录';
    const coverage=overview.indexCoverage;
    const coverageNote=coverage?`已索引 ${overviewNum(coverage.indexedCount)} 条；待索引 ${overviewNum(coverage.pendingCount)} 条`:'索引覆盖暂不可读';
    return `<div class="study-overview">
      <div class="study-overview-scope"><span><strong>${esc(domainName)} 评论研究</strong> · ${esc(scope)}</span><code>${esc(overview.domainRef||overviewDomainRef||'领域未记录')} · ${esc(asOf)}</code></div>
      <section class="study-readout-strip" aria-label="评论研究观察基础">
        ${overviewReadout('可显示评论',overviewNum(corpus?.displayableCommentCount),'条',corpus?coverageNote:'评论库存汇总暂不可读')}
        ${overviewReadout('可研究评论',overviewNum(corpus?.eligibleCommentCount),'条',corpus?'按当前来源资格':'来源资格暂不可读',true)}
        ${overviewReadout('已进入研究',overviewNum(research?.studiedCommentCount),'条','当前有效评论身份去重')}
        ${overviewReadout('当前研究信号',overviewNum(research?.currentSignalCount),'条','当前有效研究结果')}
        ${overviewReadout('长期用户问题',overviewNum(knowledge?.problemCount),'个','当前问题档案')}
      </section>
      ${renderObservationSeries(overview.observationSeries)}
      <div class="study-overview-grid">
        <section class="study-overview-section" aria-labelledby="study-problem-title"><div class="study-section-head"><h2 id="study-problem-title">近期获得新依据的问题</h2><button class="study-link" type="button" data-study-view="problems">查看用户问题</button></div><p class="study-section-note">近 ${overviewNum(knowledge?.supportWindowDays)} 天新增不同评论依据；展示当前可读的最多 3 个问题，不按需求强度排序。</p>${problems.length?`<ol class="study-problem-list">${problems.map((item,index)=>`<li class="study-problem-item"><div class="study-problem-top"><span class="study-problem-index">0${index+1}</span><h3 class="study-problem-title">${esc(item.title||'问题标题未记录')}</h3></div><p class="study-problem-meta">近 28 天新增 ${overviewNum(item.addedSupportCommentCount)} 条不同评论依据 · 当前支持 ${overviewNum(item.supportCommentCount)} 条</p>${item.voice?.sourceState==='known'?`<blockquote class="study-problem-quote">“${esc(item.voice.commentText||'')}”</blockquote>`:'<p class="study-restricted">原声当前不可读。</p>'}<div class="study-link-actions"><button class="study-link" type="button" data-overview-problem="${esc(item.problemRef)}">查看问题定义与依据</button>${item.voice?.commentKey?.workRef&&item.voice?.commentKey?.commentExternalId?`<button class="study-link" type="button" data-overview-comment-work="${esc(item.voice.commentKey.workRef)}" data-overview-comment-id="${esc(item.voice.commentKey.commentExternalId)}">查看原声</button>`:''}</div></li>`).join('')}</ol>`:'<p class="study-empty">近 28 天没有新增可读的不同评论依据；全部问题仍可在用户问题页查看。</p>'}</section>
        <section class="study-overview-section" aria-labelledby="study-run-title"><div class="study-section-head"><h2 id="study-run-title">研究与归并进度</h2><button class="study-link" type="button" data-study-view="runs">查看运行记录</button></div><p>当前有 ${overviewNum(research?.succeededCommentCount)} 条评论形成研究信号，${overviewNum(research?.noSignalCommentCount)} 条评论在当前有效研究中未提取到信号。</p><p>已归入问题 ${overviewNum(knowledge?.assignedSignalCount)} 条信号；活跃问题 ${overviewNum(knowledge?.activeProblemCount)} 个，支持不足 ${overviewNum(knowledge?.supportInsufficientProblemCount)} 个。</p>${overview.latestRun?`<p>最近 Run：${esc(label(runStateLabel,overview.latestRun.state)||'状态未记录')} · ${esc(String(overview.latestRun.createdAt||'').replace('T',' '))}</p><button class="study-link" type="button" data-overview-run="${esc(overview.latestRun.runRef)}">查看本次方法、目标和调用记录</button>`:'<p class="study-empty">尚无 Run；可从评论列表选择评论发起研究。</p>'}</section>
      </div>
      <div class="study-overview-grid" data-balance="equal">
        <section class="study-overview-section" aria-labelledby="study-method-title"><div class="study-section-head"><h2 id="study-method-title">用户提到的办法与经历</h2><span>不替用户判断有效性</span></div><div class="study-method-switcher" role="tablist" aria-label="办法与经历"><button type="button" role="tab" data-method-kind="solution" aria-selected="${methodKind==='solution'}">解决办法</button><button type="button" role="tab" data-method-kind="experience" aria-selected="${methodKind==='experience'}">使用经历</button></div><p class="study-section-note">当前 ${overviewNum(methodCount)} 条信号，覆盖 ${overviewNum(methodWorkCount)} 篇作品；下方为服务端当前可读预览，最多 3 条。</p>${methods.length?`<div class="study-voice-list">${methods.map(overviewVoiceCard).join('')}</div>`:'<p class="study-empty">当前没有可读预览，可到研究运行查看分页结果。</p>'}</section>
        <section class="study-overview-section" aria-labelledby="study-voice-title"><div class="study-section-head"><h2 id="study-voice-title">原声预览</h2><button class="study-link" type="button" data-study-view="comments">查看评论目录</button></div><p class="study-section-note">从当前有效来源读取最多 3 条不同评论；回到评论详情可看父语境与研究历史。</p>${voices.length?`<div class="study-voice-list">${voices.map(overviewVoiceCard).join('')}</div>`:'<p class="study-empty">当前没有可读的研究原声预览。</p>'}</section>
      </div>
      <section class="study-overview-section" aria-labelledby="study-unknown-title"><div class="study-section-head"><h2 id="study-unknown-title">尚未看清的部分</h2><button class="study-link" type="button" data-study-view="problems">查看未建档表达</button></div><p class="study-section-note">以下为当前有效信号的服务端归并状态，不是人工审核待办。</p>${pending.length?`<div class="study-unknown-list">${pending.map(item=>`<div class="study-unknown-row"><span>${esc(item.title)}</span><b>${overviewNum(item.count)}</b><button type="button" ${item.candidate?`data-overview-pending-state="${esc(item.candidate)}"`:'data-study-view="runs"'}>查看</button></div>`).join('')}</div>`:'<p class="study-empty">当前没有待说明的归并状态。</p>'}</section>
      <p class="study-overview-footnote">总览数据由服务端按当前有效来源汇总。浏览、筛选与查看详情不会创建研究运行或调用模型。</p>
    </div>`;
  }
  async function loadIntelligenceOverview(){
    try{return {overview:await get(overviewPath('overview')),errors:{overview:false}};}
    catch{return {overview:null,errors:{overview:true}};}
  }
  async function renderIntelligenceOverviewTab(){overviewState.cache=await loadIntelligenceOverview();return renderIntelligenceOverview(overviewState.cache)}
  function rerenderIntelligenceOverview(){if(activeView!=='overview'||!overviewState.cache)return;document.querySelector('#study-tab-result').innerHTML=renderIntelligenceOverview(overviewState.cache)}
  document.querySelector('#study-tab-result').addEventListener('click',event=>{const view=event.target.closest('[data-study-view]');if(view){switchToView(view.dataset.studyView);return}const seriesDays=event.target.closest('[data-series-days]');if(seriesDays){overviewState.seriesDays=Number(seriesDays.dataset.seriesDays);rerenderIntelligenceOverview();return}const seriesView=event.target.closest('[data-series-view]');if(seriesView){overviewState.seriesView=seriesView.dataset.seriesView;rerenderIntelligenceOverview();return}const selectedProblem=event.target.closest('[data-overview-problem]');if(selectedProblem){void navigateToProblem(selectedProblem.dataset.overviewProblem);return}const comment=event.target.closest('[data-overview-comment-work][data-overview-comment-id]');if(comment){void navigateToComment(comment.dataset.overviewCommentWork,comment.dataset.overviewCommentId);return}const run=event.target.closest('[data-overview-run]');if(run){void navigateToRun(run.dataset.overviewRun);return}const pending=event.target.closest('[data-overview-pending-state]');if(pending){problemCandidateState.state=pending.dataset.overviewPendingState;problemCandidateState.loaded=false;void switchToView('problems');return}const method=event.target.closest('[data-method-kind]');if(method){overviewState.method=method.dataset.methodKind;rerenderIntelligenceOverview()}});
  const setupStatus=document.querySelector('#setup-status');
  const statusObserver=new MutationObserver(()=>{setupStatus.hidden=!setupStatus.textContent.trim()});
  statusObserver.observe(setupStatus,{childList:true,characterData:true,subtree:true});
  setupStatus.hidden=!setupStatus.textContent.trim();
void(async()=>{await loadSetup();await loadProjection();const params=new URLSearchParams(window.location.search);const detailWork=params.get('commentWorkRef')||(params.get('detail')==='comment'?params.get('workRef'):null);if(activeView==='comments'&&detailWork&&params.get('commentExternalId'))void openCommentDetail(detailWork,params.get('commentExternalId'),{push:false});})();
