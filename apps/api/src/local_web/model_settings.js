(() => {
  'use strict';
  const api='/api/local/model-settings', $=id=>document.getElementById(id);
  const esc=v=>String(v??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
  const errors={catalog_unavailable:'供应商不提供兼容的模型目录，可手动添加准确模型 ID。',provider_rate_limited:'供应商限流，请稍后重试。',output_limit:'输出达到上限，未作为完整分析接纳。',unexpected_content:'模型输出包含未授权内容类型，未接纳。',response_too_large:'供应商响应超出接收上限，已停止读取。',secret_echo_rejected:'供应商响应包含凭据回显，已拒绝保存或展示。',endpoint_rejected:'请求地址超出已配置连接范围，已阻止。',invalid_request:'模型请求格式不符合当前适配协议。',model_revision_conflict:'设置已被修改。请刷新后重新操作；刚才的请求未覆盖新版本。',model_disabled:'连接或计划已暂停，未发起新调用。',model_not_qualified:'先测试模型，确认评论输出通过校验。',model_secret_unavailable:'无法访问本机 Keychain 凭据。请检查系统授权，或更换这条连接的凭据。',model_adapter_unavailable:'Pi 运行依赖未就绪。请检查本机 Node 与适配器安装状态。',model_budget_exhausted:'剩余额度不足以预留一次调用。',model_source_unavailable:'所选来源已不可读或版本已变化，请重新选择。',model_input_limit:'来源与上下文超过本次输入预算。',model_invalid_output:'调用已结束，输出未通过评论来源与结构校验。',invalid_model_command:'请检查地址、模型 ID、来源数量及额度范围。',model_database_unavailable:'模型设置数据库暂不可用。',provider_timeout:'等待供应商超时；停止等待不代表远端请求已撤销。',authentication_failed:'凭据未通过供应商验证。',provider_failed:'供应商调用失败，请查看连接或稍后重试。',provider_unavailable:'供应商暂不可用，系统按本次配置的次数与额度重试。',worker_interrupted:'执行中断，用量暂未知，保留预留额度。',model_budget_overrun:'供应商报告的用量超过本次上限，结果未被接纳。',provider_redirect_rejected:'供应商返回重定向，凭据未被转发。'};
  const explain=c=>errors[c]||({model_not_found:'该模型或连接已不存在。',claim_conflict:'任务已被其他执行者接续。',result_validation_pending:'已取得调用回执，结果仍待确认。'}[c])||c||'未测试';
  const kinds={trial:'单条试运行',automatic:'自动新增',backfill:'历史补跑'}, ops={connect:'连接测试',discover:'发现模型',probe:'模型能力测试',analyze:'评论分析'};
  let data=null, command=null, busy=false;
  let focusedPlan=new URLSearchParams(location.search).get('planRef');
  async function request(path,body){
    const r=await fetch(path,{method:body===undefined?'GET':'POST',headers:body===undefined?{}:{'Content-Type':'application/json'},body:body===undefined?undefined:JSON.stringify(body),cache:'no-store'});
    const v=await r.json();if(!r.ok)throw new Error(explain(v.error));return v;
  }
  const table=(heads,rows)=>`<div class="lgi-model-table"><table><thead><tr>${heads.map(h=>`<th scope="col">${h}</th>`).join('')}</tr></thead><tbody>${rows.join('')}</tbody></table></div>`;
  const button=(action,ref,label,disabled=false)=>`<button type="button" data-action="${action}" data-ref="${esc(ref)}" ${disabled?'disabled':''}>${label}</button>`;
  const note=t=>`<p class="lgi-model-note">${esc(t)}</p>`;
  function connectionStatus(c){return !c.enabled?'已停用':!c.test?'已保存，尚未测试':c.test.ok?'目录连接成功':explain(c.test.failureCode);}
  function modelStatus(m){return !m.enabled?'连接已停用':!m.test?'尚未测试':m.test.commentQualified?'可调用 · 评论输出通过校验':m.test.modelCallable?'可调用 · 评论输出未通过校验':explain(m.test.failureCode);}
  async function load(){
    data=await request(api+(focusedPlan?'?planRef='+encodeURIComponent(focusedPlan):''));
    $('storage-state').textContent=data.secretStorage==='SYNTHETIC_PREVIEW_ONLY'?'合成隔离预览 · 凭据仅为测试标记 · 非真实模型质量验收':'本机工作空间 · 凭据保存于 macOS Keychain';
    $('connection-list').innerHTML=data.connections.length?table(['连接','状态','操作'],data.connections.map(c=>`<tr><td>${esc(c.name)}${note(c.baseUrl)}${note(c.api)}</td><td>${esc(connectionStatus(c))}${note(`配置修订 ${c.revision} · 凭据不回显`)}</td><td>${button('connect',c.connectionRef,'测试连接',!c.enabled)} ${button('discover',c.connectionRef,'发现模型',!c.enabled)} ${button('edit-connection',c.connectionRef,'更换连接配置')} ${button('toggle',c.connectionRef,c.enabled?'停用':'启用')}</td></tr>`)):note('先添加供应商连接。保存不会发送评论。');
    $('model-list').innerHTML=data.models.length?table(['模型 ID','调用与输出','操作'],data.models.map(m=>`<tr><td>${esc(m.modelId)}${note(m.connectionName)}</td><td>${esc(modelStatus(m))}</td><td>${button('probe',m.modelRef,'测试模型能力',!m.enabled)}</td></tr>`)):note('从连接发现模型，或添加准确的模型 ID。');
    const selected=data.config?.modelRef;
    $('default-model').innerHTML='<option value="">请选择已通过评论校验的模型</option>'+data.models.filter(m=>(m.enabled&&m.test?.commentQualified)||m.modelRef===selected).map(m=>`<option value="${m.modelRef}">${esc(m.connectionName+' / '+m.modelId+(m.enabled?'':'（连接已停用）'))}</option>`).join('');
    if(data.config)for(const [key,value]of Object.entries(data.config)){const field=$('config-form').elements.namedItem(key);if(field)field.value=value;}
    ['trial','automatic','backfill'].forEach(id=>$(id).disabled=!data.config);
    renderActivity();
    if(focusedPlan)$('plan-'+focusedPlan)?.scrollIntoView({block:'center'});
  }
  function renderActivity(){
    const auto=data.plans.find(p=>p.planRef===data.activeAutoPlanRef),worker=data.worker;
    $('automatic-state').textContent=auto?.enabled?`自动新增已启用：只纳入 ${auto.createdAt} 后新接纳的可读来源，已纳入 ${auto.sourceCount}/${auto.sourceLimit} 条。换默认模型不会重置这份累计额度。`:'自动新增已暂停或尚未启用。保存设置不改变此状态。';
    $('worker-state').textContent=!worker?.recent?'执行循环暂无近期心跳。提交的计划会保留，待本机 worker 启动后接续。':worker.lastError?`执行循环最近失败：${explain(worker.lastError)}`:`执行循环在线 · ${worker.state==='running'?'正在处理一项调用':'等待可执行任务'} · 最近心跳 ${worker.lastSeenAt}`;
    $('plan-list').innerHTML=data.plans.length?table(['处理计划','来源与额度','状态'],data.plans.map(p=>`<tr id="plan-${p.planRef}"><td>${kinds[p.kind]}${note(p.createdAt)}</td><td>已纳入 ${p.sourceCount}/${p.sourceLimit} 条${note(`额度占用 ${p.budgetUsed.toLocaleString()} / ${p.tokenLimit.toLocaleString()} token（含 ${p.unknownUsageCount} 次用量未知）`)}</td><td>${!p.enabled?'已暂停':p.nextReservation!=null&&p.tokenLimit-p.budgetUsed<p.nextReservation?'余额不足以预留下次调用':'允许派发'}${p.disabledCount?note(`${p.disabledCount} 条等待的连接已停用`):''}${note(`待处理 ${p.pendingCount??0} · 执行中 ${p.runningCount??0} · 已结束 ${p.finishedCount??0}`)}${p.enabled?button('stop',p.planRef,'暂停此计划'):button('resume',p.planRef,'恢复此计划')}</td></tr>`)):note('没有评论分析计划。先选择一条可读原声试运行。');
    $('run-list').innerHTML=data.runs.length?table(['最近调用','状态与原因','用量'],data.runs.map(r=>`<tr><td>${ops[r.operation]}${note(r.modelId||'模型目录请求')}${note(r.createdAt)}${r.sourceRef?`<a href="/corpus/comments?source=${r.sourceRef}">查看来源与标注</a>`:''}</td><td>${r.state==='running'?'执行中':r.state==='succeeded'?'已完成':'失败'}${r.failureCode?note(explain(r.failureCode)):''}${r.attempts?note(`此任务已尝试 ${r.attempts} 次`):''}${r.configRef?`<details><summary>配置版本</summary>${note(r.configRef)}</details>`:''}</td><td>输入 ${r.inputTokens??'未知'} · 输出 ${r.outputTokens??'未知'}${note(`本机额度计入 ${r.budgetAccounted} token · 金额待供应商核对`)}${r.elapsedMs!=null?note(`耗时 ${r.elapsedMs} ms`):''}</td></tr>`)):note('没有调用回执。');
  }
  function open(title,fields,action,label='保存'){
    command=action;command.id=crypto.randomUUID();$('dialog-title').textContent=title;$('dialog-fields').innerHTML=fields;$('dialog-feedback').textContent='';$('dialog-submit').textContent=label;$('dialog-submit').disabled=false;$('model-dialog').showModal();
  }
  function close(){if(busy)return;const key=$('model-command').elements.namedItem('apiKey');if(key)key.value='';$('dialog-fields').replaceChildren();command=null;$('model-dialog').close();}
  const textField=(name,label,value='',extra='')=>`<label>${label}<input name="${name}" value="${esc(value)}" ${extra}></label>`;
  function editConnection(c){
    open(c?'更换连接配置':'添加供应商连接',textField('name','连接名称',c?.name||'','required maxlength="100"')+`<label>接口协议<select name="api">${['openai-completions','openai-responses','anthropic-messages'].map(a=>`<option ${a===c?.api?'selected':''}>${a}</option>`).join('')}</select></label>`+textField('baseUrl','供应商基础地址',c?.baseUrl||'','type="url" placeholder="https://供应商地址/v1" required maxlength="1000"')+`<label class="lgi-model-check"><input type="checkbox" name="localEndpoint" ${c?.localEndpoint?'checked':''}>这是本机回环测试服务（允许 HTTP）</label>`+textField('apiKey','API 凭据（保存后不回显）','','type="password" autocomplete="new-password" maxlength="4096"')+note('更换会创建连接版本。已有任务保留原地址与原凭据；如需停止旧任务，请先停用整条连接。停用后更换配置不会自动启用。')+note('OpenAI 兼容协议通常填写以 /v1 结尾的地址；Anthropic 协议填写服务根地址，SDK 会追加 /v1/messages。部分服务不支持目录，可手动添加 ID 后测试模型。'),{kind:'connection',connection:c,connectionRef:c?.connectionRef||crypto.randomUUID()});
  }
  function addModel(version,ids=[]){
    const conns=data.connections.filter(c=>c.enabled);
    open('添加模型 ID',`<label>供应商连接<select name="connectionVersionRef" required><option value="">请选择</option>${conns.map(c=>`<option value="${c.versionRef}" ${version===c.versionRef?'selected':''}>${esc(c.name)}</option>`).join('')}</select></label>`+textField('modelId','准确模型 ID',ids[0]||'','required maxlength="200" list="discovered-models"')+`<datalist id="discovered-models">${ids.map(id=>`<option value="${esc(id)}"></option>`).join('')}</datalist>`+note(ids.length?`账户目录返回 ${ids.length} 个候选；目录存在不代表调用或评论输出通过测试。`:'请复制供应商提供的准确 ID，不根据名称猜测。'),{kind:'model'});
  }
  async function probe(operation,ref){
    const isModel=operation==='probe',m=isModel?data.models.find(m=>m.modelRef===ref):null,c=!isModel?data.connections.find(c=>c.connectionRef===ref):null;
    $('settings-feedback').textContent=isModel?'正在用合成评论测试模型。上限：10 秒、1024 输出 token，无自动重试。':'正在读取供应商模型目录…';
    const result=await request(api+'/probes',{invocationRef:crypto.randomUUID(),connectionVersionRef:m?.connectionVersionRef||c.versionRef,modelRef:m?.modelRef||null,operation});
    await load();const r=result.replayed?result.result:result;
    $('settings-feedback').textContent=isModel?(r?.commentQualified?'模型可调用，合成评论输出通过校验。可选择为评论分析默认模型。':r?.modelCallable?'模型可调用，但评论输出未通过校验。':explain(r?.failureCode)):(r?.ok?'模型目录连接成功；仍需单独测试模型。':explain(r?.failureCode));
    if(operation==='discover'&&r?.ok)addModel(c.versionRef,r.modelIds||[]);
  }
  function automatic(){
    const c=data.config;
    open('启用自动新增',note(`启用后，仅处理本次启用时间之后新接纳且可读的来源。数量上限 ${c.autoSourceLimit} 条，累计 ${c.autoTokenLimit} token。每次调用先预留 ${c.inputTokenLimit+c.outputTokenLimit} token；最多尝试 ${c.maxAttempts} 次。`)+note('此操作会替换原自动计划；原计划未开始的调用暂停。历史评论不会自动补跑。保存新默认模型后，新任务使用新配置，剩余累计额度不变。'),{kind:'automatic',configRef:c.configRef,expectedAutoPlanRef:data.activeAutoPlanRef,sourceLimit:c.autoSourceLimit,tokenLimit:c.autoTokenLimit},'确认启用');
  }
  function resumePlan(p){
    const automaticNote=p.kind==='automatic'?(p.planRef===data.activeAutoPlanRef?'当前自动计划会沿原启用时间继续纳入可读来源，包括暂停期间新增，保留原数量上限与剩余额度。':'这份自动计划已被替代；仅恢复已经归入它的工作，不抢占当前自动计划，也不再纳入新来源。'):'仅接续此计划已有的未完成工作。';
    open('恢复原计划',note(`${kinds[p.kind]} · 已纳入 ${p.sourceCount}/${p.sourceLimit} 条 · 已占用 ${p.budgetUsed}/${p.tokenLimit} token。`)+note(automaticNote)+note('已有工作保留原模型配置、来源、尝试次数及用量；已完成来源不会重跑。余额不足或连接停用仍会阻止新调用。'),{kind:'resume',planRef:p.planRef,revision:p.revision},'确认恢复');
  }
  async function chooseSources(kind,sourceRef){
    const c=data.config;
    open(kind==='trial'?'选择一条原声试运行':'选择历史评论补跑',note('仅发送所选来源及其有限可读上下文。提交前可查看原文与作品；读取范围之外的历史评论不会被追加。')+`<div class="lgi-model-search"><input id="source-search" aria-label="筛选可读评论" placeholder="输入原声关键词"><button type="button" id="find-sources">查找</button></div><p id="source-count"></p><div id="source-options"></div>`+textField('tokenLimit','本次累计 token 上限',c.inputTokenLimit+c.outputTokenLimit,`type="number" min="${c.inputTokenLimit+c.outputTokenLimit}" max="10000000" required`)+note(`当前模型：${data.models.find(m=>m.modelRef===c.modelRef)?.modelId}。每次预留 ${c.inputTokenLimit+c.outputTokenLimit} token，最多尝试 ${c.maxAttempts} 次；用量未知时保留预留。`),{kind,configRef:c.configRef},kind==='trial'?'提交这条试运行':'提交所选历史补跑');
    await searchSources(sourceRef);
  }
  async function searchSources(sourceRef){
    const list=sourceRef?{page:{items:[(await request('/api/local/comment-research/sources/'+encodeURIComponent(sourceRef))).source],total:1},works:[]} : await request('/api/local/comment-research?text='+encodeURIComponent($('source-search').value));
    const works=new Map(list.works.map(w=>[w.workRef,w]));
    $('source-count').textContent=`当前展示 ${list.page.items.length} 条可读原声；可搜索缩小范围。`;
    $('source-options').innerHTML=list.page.items.filter(s=>s.body).map(s=>`<label class="lgi-model-source"><input type="${command.kind==='trial'?'radio':'checkbox'}" name="sourceRefs" value="${s.sourceRef}" ${sourceRef?'checked':''}><span><blockquote>${esc(s.body)}</blockquote>${note(works.get(s.workRef)?.title||'来源作品可从评论研究查看')}<a href="/corpus/comments?source=${s.sourceRef}" target="_blank" rel="noopener">查看完整来源</a></span></label>`).join('')||note('此范围没有可发送的正文。');
  }
  async function submit(event){
    event.preventDefault();if(busy)return;busy=true;$('dialog-submit').disabled=true;
    try{
      const f=new FormData($('model-command')),c=command;let result;
      if(c.kind==='connection')result=await request(api+'/connections',{versionRef:c.id,connectionRef:c.connectionRef,expectedRevision:c.connection?.revision||0,name:f.get('name'),api:f.get('api'),baseUrl:f.get('baseUrl'),apiKey:f.get('apiKey'),localEndpoint:f.has('localEndpoint')});
      else if(c.kind==='resume')result=await request(api+'/plans/'+c.planRef+'/resume',{expectedRevision:c.revision});
      else if(c.kind==='model')result=await request(api+'/models',{modelRef:c.id,connectionVersionRef:f.get('connectionVersionRef'),modelId:f.get('modelId')});
      else{
        const refs=f.getAll('sourceRefs');if(c.kind!=='automatic'&&(refs.length===0||(c.kind==='trial'&&refs.length!==1)))throw new Error('请先选择要分析的原声。');
        result=await request(api+'/plans',{planRef:c.id,configRef:c.configRef,kind:c.kind,sourceRefs:refs,sourceLimit:c.sourceLimit||refs.length,tokenLimit:c.tokenLimit||Number(f.get('tokenLimit')),expectedAutoPlanRef:c.kind==='automatic'?c.expectedAutoPlanRef:null});
      }
      busy=false;close();await load();$('existing-plans').innerHTML=(result?.existingPlans||[]).map(p=>`<p>所选来源已有${esc(kinds[p.kind])}：<a href="/settings/models?planRef=${p.planRef}">查看所属计划${p.enabled?'':'并恢复'}</a>。本次未为它另建任务或补充额度。</p>`).join('');$('settings-feedback').textContent=result?.queued===0?'请求已保存；相同来源与配置已有任务，未重复调用。':'已保存。评论计划由执行循环接续，调用结果显示在额度与运行中。';
    }catch(e){$('dialog-feedback').textContent=e.message;}finally{busy=false;$('dialog-submit').disabled=false;}
  }
  $('config-form').addEventListener('submit',async e=>{e.preventDefault();const button=e.submitter;button.disabled=true;try{const f=new FormData(e.currentTarget),body={configRef:crypto.randomUUID(),expectedConfigRef:data.config?.configRef||null};for(const[k,v]of f)body[k]=k==='modelRef'?v:Number(v);await request(api+'/config',body);await load();$('settings-feedback').textContent='用途配置已保存，没有发起分析。';}catch(e){$('settings-feedback').textContent=e.message;}finally{button.disabled=false;}});
  $('new-connection').onclick=()=>editConnection(null);$('new-model').onclick=()=>addModel();$('automatic').onclick=automatic;
  $('trial').onclick=()=>chooseSources('trial').catch(showError);$('backfill').onclick=()=>chooseSources('backfill').catch(showError);
  $('reload').onclick=()=>load().catch(showError);$('dialog-close').onclick=close;$('model-dialog').addEventListener('cancel',e=>{e.preventDefault();close();});$('model-command').addEventListener('submit',submit);
  function showError(e){$('settings-feedback').textContent=e.message;}
  document.addEventListener('click',async e=>{
    const b=e.target.closest('[data-action]');if(e.target.id==='find-sources'){try{await searchSources();}catch(e){$('dialog-feedback').textContent=e.message;}return;}if(!b)return;b.disabled=true;
    try{const ref=b.dataset.ref,action=b.dataset.action;if(['connect','discover','probe'].includes(action))await probe(action,ref);
      else if(action==='edit-connection')editConnection(data.connections.find(c=>c.connectionRef===ref));
      else if(action==='toggle'){const c=data.connections.find(c=>c.connectionRef===ref);await request(api+'/connections/state',{connectionRef:ref,expectedRevision:c.revision,enabled:!c.enabled});await load();}
      else if(action==='resume')resumePlan(data.plans.find(p=>p.planRef===ref));
      else if(action==='stop'){await request(api+'/plans/'+ref+'/stop',{});await load();}
    }catch(e){showError(e);}finally{b.disabled=false;}
  });
  load().then(()=>{const ref=new URLSearchParams(location.search).get('sourceRef');if(ref&&data.config)return chooseSources('trial',ref);}).catch(showError);
})();
