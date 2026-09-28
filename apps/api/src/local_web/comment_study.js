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
const MAX_SELECTED_WORKS = 100;
const workTitleSourceLabel = { platform_title: '平台标题', cover_ocr: '封面 OCR', unknown: '标题未记录' };
const selectedWorks = () => [...selectedWorkRoles].map(([contentPublicRef, observationRole]) => ({ contentPublicRef, observationRole }));
const visibleWorks = () => loadedWorks;
let savedPolicies = [];
let activePolicyRef = null;
let pendingStartSignature = null;
let pendingStartRef = null;

function updateSelection() {
  const count = selectedWorkRoles.size;
  const selectedEligible = [...selectedWorkRoles.keys()].reduce((total, ref) =>
    total + Number(selectedWorkMeta.get(ref)?.eligibleCommentCount || 0), 0);
  const budget = Number(document.querySelector('#comment-budget').value || 0);
  const frozenCount = budget > 0 ? Math.min(selectedEligible, budget) : 0;
  document.querySelector('#selected-count').textContent = count
    ? `已选择 ${count}/${MAX_SELECTED_WORKS} 篇 · 当前已知合格 ${selectedEligible} 条 · 本次最多冻结 ${frozenCount} 条`
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
  const labels = { workAuthorUnknown:'作品作者身份未知',creatorVoice:'作品作者本人',bodyUnavailable:'正文不可研究',sourceRestricted:'来源受限',textNotResearchable:'文本不具研究条件' };
  const reasons = Object.entries(labels).map(([key,label])=>[label,Number(excluded[key]||0)]).filter(([,count])=>count>0).map(([label,count])=>`${label} ${count} 条`);
  const excludedCount=Number(preview.totalCommentCount||0)-Number(preview.eligibleCommentCount||0);
  const unknownAuthors=Number(preview.unknownAuthorCount||0);
  target.textContent=`${roleLabel} · 截至 ${String(preview.asOf||'').replace('T',' ')}：共 ${Number(preview.totalCommentCount||0)} 条评论；可研究 ${Number(preview.eligibleCommentCount||0)} 条；未纳入 ${excludedCount} 条${unknownAuthors?`；作者身份未知 ${unknownAuthors} 条（不作为独立用户计数）`:''}${reasons.length?`（${reasons.join('；')}）`:''}。`;
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
  const data=await get(workCatalogPath(cursor));
  loadedWorks=(data.items||[]).map(item=>({...item,observationRole:item.observationRole||workCatalogState.observationRole}));
  workCatalogState.cursor=cursor;
  workCatalogState.nextCursor=data.page?.nextCursor||null;
  workCatalogState.total=Number(data.totalWorkCount||0);
  workCatalogState.coverage=data.indexCoverage||null;
  renderWorks();
}
async function searchWorksNow(){
  workCatalogState.q=document.querySelector('#work-filter').value.trim();
  workCatalogState.cursor=null;workCatalogState.history=[];
  await loadWorksPage(null);
}
async function loadSetup(){
  const status=document.querySelector('#setup-status');
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
    await loadWorksPage(null);
    await loadPolicies();
    status.dataset.kind=available?'info':'error';
    status.textContent=setup.domainStatus==='paused'?`${domainName}已暂停；历史结果可读，不能创建新策略或运行。`:available?`已加载 ${workCatalogState.total} 篇 primary 作品；可显式切换查看 reference 作品。`:'没有启用的模型配置，无法保存策略。';
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
  updateSelection();
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
  deferred_ambiguous: '存在多个可能匹配，继续等待', deferred_novel: '独立新证据，等待第二条佐证',
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
const sourceStateLabel = { known: null, restricted: '来源已被限制，原文不再显示', unknown: '原文未知（来源未采集到正文）' };
const runStateLabel = {
  queued: '排队中', running: '处理中', completed: '已完成',
  completed_with_failures: '已结束 · 有目标未完成', cancelled: '已停止'
};
const dispatchStateLabel = { enabled: '允许继续派发', paused: '已暂停派发', stopped: '已停止派发' };
const dispatchReasonLabel = {
  user_paused: '用户暂停', user_stopped: '用户停止', budget_exhausted: '预算已用尽',
  legacy_unrecorded: '历史方法未记录', upgrade_guard: '升级保护', method_unavailable: '方法不可用'
};
let stopDialogRunRef = null;
let stopDialogControlVersion = null;
let runControlFeedback = '';
let runControlOutcomeNeedsRefresh = false;
const label = (map, value) => (value == null ? null : (map[value] ?? '未知状态'));
const PENDING_RESOLUTION_STATES = new Set(['pending', 'deferred_context', 'deferred_ambiguous', 'deferred_novel', 'retrieval_incomplete', 'budget_stopped']);

let allRuns = [];
let activeView = 'overview';
let selectedRunRef = null;
const RUN_SCOPED_VIEWS = new Set(['pending']);

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
      <p>选择作品 ${Number(latest.selectedWorkCount)} 篇（primary ${Number(latest.primaryWorkCount || 0)} / reference ${Number(latest.referenceWorkCount || 0)}），冻结评论目标 ${targetTotal} 条。</p>
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
    <td><span class="study-badge">${esc(observationRoleLabel[target.observationRole] || '来源角色未知')}</span>${commentBlock}</td>
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

const commentCatalogState={q:'',studyState:'all',voiceRole:'reader_and_unknown',workRef:null,workLabel:'',cursor:null,history:[],response:null,summary:null,workChoices:[]};
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
  const map={sourceRestricted:'来源受限',bodyUnavailable:'正文不可用',indexPending:'等待本地清洗',textNotResearchable:'纯无效文本',workAuthorUnknown:'作品作者身份未知',creatorVoice:'作品作者声音'};
  return(item.studyEligibility?.reasons||[]).map(reason=>map[reason]||reason).join('；')||'暂不可研究';
}
async function renderCommentsTab(){
  if(!domainRef)return'<p class="study-empty">评论目录尚未就绪。</p>';
  const params={q:commentCatalogState.q,workRef:commentCatalogState.workRef,studyState:commentCatalogState.studyState,voiceRole:commentCatalogState.voiceRole,cursor:commentCatalogState.cursor,limit:50};
  const summaryParams={...params};delete summaryParams.cursor;delete summaryParams.limit;
  const[data,summary]=await Promise.all([get(catalogQuery('comments',params)),get(catalogQuery('catalog-summary',summaryParams))]);
  commentCatalogState.response=data;commentCatalogState.summary=summary;
  const rows=data.items||[],stats=summary.summary||{},coverage=data.indexCoverage||{},pending=coverage.pendingCount==null?'未知':Number(coverage.pendingCount);
  const workOptions=commentCatalogState.workChoices.map(work=>`<option value="${esc(work.workRef)}"${work.workRef===commentCatalogState.workRef?' selected':''}>${esc(work.displayTitle||'未命名作品')}</option>`).join('');
  const body=rows.length?rows.map(item=>`<tr><td><button type="button" class="study-comment-open" data-comment-work="${esc(item.commentKey.workRef)}" data-comment-id="${esc(item.commentKey.commentExternalId)}"><span class="study-comment-voice">${esc(item.commentText)}</span></button><small>${esc(commentEligibility(item))}</small></td><td><span class="study-work-title">${esc(item.workTitle||'未命名作品')}</span><small>${esc(workTitleSourceLabel[item.workTitleSource]||item.workTitleSource||'标题未记录')}</small></td><td>${esc(commentStatus(item))}</td><td>${esc(item.authorDisplayName||'作者未记录')}<small>${esc(String(item.observedAt||'').replace('T',' '))}</small></td></tr>`).join(''):'<tr><td colspan="4" class="study-table-empty">当前筛选没有可显示的用户评论。</td></tr>';
  return`<section class="study-comments" aria-labelledby="comments-title"><header class="study-comments-head"><div><p class="study-label">评论证据库</p><h2 id="comments-title">用户评论</h2><p>当前筛选可显示 ${Number(stats.displayableCommentCount||0)} 条；可研究 ${Number(stats.eligibleCommentCount||0)} 条；已索引 ${Number(coverage.indexedCount||0)}，待索引 ${pending}。</p></div></header>
  <form id="comment-filter-form" class="study-comment-filters"><label>字面搜索<input id="comment-query" type="search" value="${esc(commentCatalogState.q)}" placeholder="搜索清洗后的评论文本"></label><label>研究状态<select id="comment-study-state">${[['all','全部'],['never_studied','从未研究'],['in_progress','处理中'],['studied','已有成功结果'],['needs_context','等待语境'],['failed','最近失败']].map(([v,t])=>`<option value="${v}"${commentCatalogState.studyState===v?' selected':''}>${t}</option>`).join('')}</select></label><label>声音角色<select id="comment-voice-role">${[['reader_and_unknown','用户评论（默认）'],['reader','已知用户'],['unknown','身份未知'],['creator','作品作者'],['all','全部声音']].map(([v,t])=>`<option value="${v}"${commentCatalogState.voiceRole===v?' selected':''}>${t}</option>`).join('')}</select></label><button type="submit">应用筛选</button></form>
  <div class="study-comment-work-filter"><form id="comment-work-search-form"><label>所属作品<input id="comment-work-search" type="search" value="${esc(commentCatalogState.workLabel)}" placeholder="搜索作品标题"></label><button type="submit">查找作品</button></form><label>已匹配作品<select id="comment-work-select"><option value="">全部作品</option>${workOptions}</select></label></div>
  <div class="study-review-table-wrap"><table class="study-review-table study-comments-table"><thead><tr><th scope="col">原声</th><th scope="col">所属作品</th><th scope="col">研究状态</th><th scope="col">作者 / 观测</th></tr></thead><tbody>${body}</tbody></table></div>
  <nav class="study-pagination" aria-label="用户评论翻页"><button id="comment-prev" type="button"${commentCatalogState.history.length?'':' disabled'}>上一页</button><span>第 ${commentCatalogState.history.length+1} 页</span><button id="comment-next" type="button"${data.page?.nextCursor?'':' disabled'}>下一页</button></nav></section>`;
}
async function searchCommentWorks(query){
  const data=await get(catalogQuery('works',{q:query,limit:20}));commentCatalogState.workChoices=data.items||[];
  const select=document.querySelector('#comment-work-select');if(!select)return;
  select.innerHTML='<option value="">全部作品</option>'+commentCatalogState.workChoices.map(work=>`<option value="${esc(work.workRef)}">${esc(work.displayTitle||'未命名作品')}</option>`).join('');
}
function bindCommentsView(){
  const form=document.querySelector('#comment-filter-form');if(!form)return;
  form.addEventListener('submit',async event=>{event.preventDefault();commentCatalogState.q=document.querySelector('#comment-query').value.trim();commentCatalogState.studyState=document.querySelector('#comment-study-state').value;commentCatalogState.voiceRole=document.querySelector('#comment-voice-role').value;resetCommentPage();await renderActiveTab();});
  document.querySelector('#comment-work-search-form').addEventListener('submit',async event=>{event.preventDefault();const query=document.querySelector('#comment-work-search').value.trim();commentCatalogState.workLabel=query;try{await searchCommentWorks(query);}catch(error){document.querySelector('#comment-work-select').innerHTML='<option value="">作品读取失败</option>';}});
  document.querySelector('#comment-work-select').addEventListener('change',async event=>{commentCatalogState.workRef=event.currentTarget.value||null;const chosen=commentCatalogState.workChoices.find(work=>work.workRef===commentCatalogState.workRef);commentCatalogState.workLabel=chosen?.displayTitle||'';resetCommentPage();await renderActiveTab();});
  document.querySelector('#comment-prev').addEventListener('click',async()=>{commentCatalogState.cursor=commentCatalogState.history.pop()??null;await renderActiveTab();});
  document.querySelector('#comment-next').addEventListener('click',async()=>{const next=commentCatalogState.response?.page?.nextCursor;if(!next)return;commentCatalogState.history.push(commentCatalogState.cursor);commentCatalogState.cursor=next;await renderActiveTab();});
  document.querySelectorAll('[data-comment-work][data-comment-id]').forEach(button=>button.addEventListener('click',()=>openCommentDetail(button.dataset.commentWork,button.dataset.commentId)));
}
function detailSection(title,body){return`<section class="study-detail-section"><h3>${esc(title)}</h3>${body}</section>`;}
function renderCommentDetail(data){
  const comment=data.comment,source=data.source||{},work=data.work||{},parent=data.parentContext,history=data.studyHistory?.items||[];
  const raw=comment?.commentText?`<blockquote>${esc(comment.commentText)}</blockquote>`:`<p class="study-restricted">当前原声不可显示：${esc(source.displayState||source.sourceState||'unknown')}</p>`;
  const workBody=`<p><strong>${esc(work.displayTitle||comment?.workTitle||'未命名作品')}</strong> <small>${esc(workTitleSourceLabel[work.displayTitleSource]||work.displayTitleSource||'标题未记录')}</small></p>`;
  const parentBody=parent?.commentText?`<blockquote>${esc(parent.commentText)}</blockquote><p class="study-signal-meta">仅作为语境，不作为当前评论的独立证据。</p>`:'<p class="muted">没有可显示的父评论语境。</p>';
  const cleanBody=comment?`<p>${esc(comment.researchText||'')}</p><p class="study-signal-meta">状态：${esc(comment.cleanState||'unknown')} · 原因：${esc((comment.cleanReasons||[]).join('、')||'无')}</p>`:'<p class="muted">当前版本不可清洗或尚未索引。</p>';
  const historyRow=item=>`<li><strong>${esc(label(targetStateLabel,item.state)??item.state)}</strong><span>${esc(String(item.createdAt||'').replace('T',' '))}</span><span>Signal ${Number(item.signalCount||0)} 条</span><span>方法：${esc(item.method?.recordingState==='recorded'?(item.method.methodName||item.method.policyRef):'历史未记录')}</span></li>`;
  const historyBody=history.length?`<ol class="study-history-list">${history.map(historyRow).join('')}</ol>`:'<p class="muted">尚无研究历史。</p>';
  const next=data.studyHistory?.page?.nextCursor;
  const more=next?`<button type="button" id="comment-history-more" data-work="${esc(data.commentKey.workRef)}" data-comment-id="${esc(data.commentKey.commentExternalId)}" data-cursor="${esc(next)}">继续读取研究历史</button>`:'';
  return raw+detailSection('所属作品',workBody)+detailSection('父评论语境',parentBody)+detailSection('清洗文本',cleanBody)+detailSection(`研究历史（${Number(data.studyHistory?.totalCount||0)}）`,historyBody+more);
}
async function loadMoreCommentHistory(button){
  const data=await get(catalogQuery('comments/history',{workRef:button.dataset.work,commentExternalId:button.dataset.commentId,cursor:button.dataset.cursor,limit:50}));
  const list=document.querySelector('#comment-detail-body .study-history-list');
  if(list)list.insertAdjacentHTML('beforeend',(data.items||[]).map(item=>`<li><strong>${esc(label(targetStateLabel,item.state)??item.state)}</strong><span>${esc(String(item.createdAt||'').replace('T',' '))}</span><span>Signal ${Number(item.signalCount||0)} 条</span><span>方法：${esc(item.method?.recordingState==='recorded'?(item.method.methodName||item.method.policyRef):'历史未记录')}</span></li>`).join(''));
  if(data.page?.nextCursor){button.dataset.cursor=data.page.nextCursor;button.disabled=false;button.textContent='继续读取研究历史';}else button.remove();
}
function bindCommentHistoryMore(){
  const button=document.querySelector('#comment-history-more');if(!button)return;
  button.addEventListener('click',async()=>{button.disabled=true;button.textContent='正在读取…';try{await loadMoreCommentHistory(button);}catch(error){button.disabled=false;button.textContent=`读取失败，重试：${error.message}`;}});
}
async function openCommentDetail(workRef,commentExternalId){
  const dialog=document.querySelector('#comment-detail-dialog'),body=document.querySelector('#comment-detail-body');body.innerHTML='<p class="study-empty">正在读取评论详情…</p>';if(!dialog.open)dialog.showModal();
  try{const data=await get(catalogQuery('comments/detail',{workRef,commentExternalId}));body.innerHTML=renderCommentDetail(data);bindCommentHistoryMore();}catch(error){body.innerHTML=`<p class="study-empty">评论详情读取失败：${esc(error.message)}</p>`;}
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
  const body = signal.sourceState === 'restricted'
    ? `<p class="study-restricted">来源已被限制，原声与摘要不再显示。</p>`
    : `<p class="study-signal-proposition">${esc(signal.proposition)}</p><blockquote>${esc(signal.evidence)}</blockquote>`;
  return `
    <article class="study-signal-card">
      <header><span class="study-badge">${esc(label(signalKindLabel, signal.kind) ?? signal.kind)}</span><span>${esc(observationRoleLabel[signal.observationRole] || '来源角色未知')} · ${esc(label(resolutionLabel, signal.resolutionState) ?? '尚未进入归并判断')}</span></header>
      ${body}
      <p class="study-signal-meta">归并资格：${esc(label(eligibilityLabel, signal.eligibilityState) ?? signal.eligibilityState)}${signal.eligibilityReason ? ` · ${esc(signal.eligibilityReason)}` : ''}</p>
      ${(signal.pairOutcomes || []).map(pairOutcomeSummary).join('')}
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
  const dispatchLabel = dispatchStateLabel[run.dispatchState] || '派发状态未知';
  const dispatchReason = dispatchReasonLabel[run.dispatchReason];
  return `<tr>
      <td>${esc(run.runRef.slice(0, 8))}…<p>${esc(run.createdAt)}</p></td>
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
  const data = await get('runs?limit=50');
  const feedback = '<p id="study-run-control-feedback" class="study-run-control-feedback" role="status" aria-live="polite" tabindex="-1">' + esc(runControlFeedback) + '</p>';
  if (!data.runs?.length) return feedback + '<p class="study-empty">尚未创建过研究运行。</p>';
  return feedback + '<div class="study-review-table-wrap"><table class="study-review-table"><thead><tr><th scope="col">运行</th><th scope="col">状态</th><th scope="col">作品</th><th scope="col">目标</th><th scope="col">已产出</th><th scope="col">无信号</th><th scope="col">等待语境</th><th scope="col">失败</th><th scope="col">来源受限</th><th scope="col">操作</th></tr></thead><tbody>' + data.runs.map(runRow).join('') + '</tbody></table></div>';
}

const TAB_RENDERERS = { overview: renderOverviewTab, comments: renderCommentsTab, pending: renderPendingTab, problems: renderProblemsTab, runs: renderRunsTab };

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
  if (token !== renderToken) return;
  container.innerHTML = html;
  container.setAttribute('aria-busy', 'false');
  if (view === 'comments') bindCommentsView();
  if (view === 'runs') bindRunControls(container);
  if (view === 'runs' && !renderFailed) {
    runControlOutcomeNeedsRefresh = false;
    runControlFeedback = '';
  }
}

function bindRunControls(container) {
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
  let policySaved = false;
  button.disabled = true;
  try {
    const response = await post('policies', {
      methodName: document.querySelector('#method-name').value,
      parentPolicyRef: null,
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
    status.textContent = `已保存不可变方法版本：${reference}。选择该版本后可用于本次预览和启动。`;
  } catch (error) {
    status.textContent = policySaved
      ? `方法已保存，但方法目录刷新失败：${error.message}`
      : `未保存方法版本：${error.message}`;
  }
  finally { button.disabled = !document.querySelector('#model-config').value; }
});
document.querySelector('#study-policy').addEventListener('change', () => {
  document.querySelector('#activate-policy').disabled = !document.querySelector('#study-policy').value || document.querySelector('#study-policy').value === activePolicyRef;
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
  const works = selectedWorks();
  return {
    domainRef,
    policyRef: document.querySelector('#study-policy').value,
    scope: { kind: 'works', workRefs: works.map(work => work.contentPublicRef) },
    workRoles: works,
    mode: document.querySelector('#study-mode').value,
    limits: { commentBudget: Number(document.querySelector('#comment-budget').value), contextCharacterBudget: Number(document.querySelector('#context-character-budget').value), tokenLimit: Number(document.querySelector('#token-budget').value) }
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
    if(response.runRef){studyDialog.close(); activeView = 'runs'; highlightTab('runs'); await loadProjection();}
  } catch (error) { result.textContent = `未创建研究运行：${error.message}`; }
  finally { updateSelection(); }
});
document.querySelector('#work-filter').addEventListener('input',()=>{clearTimeout(workSearchTimer);workSearchTimer=setTimeout(()=>{void searchWorksNow().catch(error=>{document.querySelector('#work-filter-status').textContent=`作品搜索失败：${error.message}`;});},250);});
document.querySelector('#work-prev').addEventListener('click',async()=>{const previous=workCatalogState.history.pop()??null;await loadWorksPage(previous);});
document.querySelector('#work-next').addEventListener('click',async()=>{if(!workCatalogState.nextCursor)return;workCatalogState.history.push(workCatalogState.cursor);await loadWorksPage(workCatalogState.nextCursor);});
document.querySelector('#comment-budget').addEventListener('input', updateSelection);
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
document.querySelector('#comment-detail-close').addEventListener('click',()=>document.querySelector('#comment-detail-dialog').close());
void(async()=>{await loadSetup();await loadProjection();})();
