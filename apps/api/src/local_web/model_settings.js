(() => {
  'use strict';
  const api='/api/local/model-settings', $=id=>document.getElementById(id);
  const esc=v=>String(v??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
  const errors={model_schema_missing:'本机尚未完成评论研究与模型配置数据库初始化（0039、0040）。当前无法保存或测试，请完成部署后刷新。',provider_endpoint_not_found:'调用地址返回 404。请核对 API 地址、接口协议和模型 ID。',catalog_unavailable:'供应商不提供兼容的模型目录，可手动添加准确模型 ID。',provider_rate_limited:'供应商限流，请稍后重试。',output_limit:'模型已响应，但输出达到本次上限，评论分析结果不完整。',unexpected_content:'模型输出包含未授权内容类型，未接纳。',response_too_large:'供应商响应超出接收上限，已停止读取。',secret_echo_rejected:'供应商响应包含凭据回显，已拒绝保存或展示。',endpoint_rejected:'请求地址超出已配置连接范围，已阻止。',invalid_request:'模型请求格式不符合当前适配协议。',model_revision_conflict:'设置已被修改。请刷新后重新操作；刚才的请求未覆盖新版本。',model_disabled:'连接或计划已暂停，未发起新调用。',model_not_qualified:'先测试模型，确认评论输出通过校验。',model_secret_unavailable:'无法访问本机 Keychain 凭据。请检查系统授权，或更换这条连接的凭据。',model_adapter_unavailable:'Pi 运行依赖未就绪。请检查本机 Node 与适配器安装状态。',model_budget_exhausted:'剩余额度不足以预留一次调用。',model_source_unavailable:'所选来源已不可读或版本已变化，请重新选择。',model_input_limit:'来源与上下文超过本次输入预算。',model_invalid_output:'调用已结束，输出未通过评论来源与结构校验。',invalid_model_command:'请检查地址、模型 ID、来源数量及额度范围。',model_database_unavailable:'模型设置数据库暂不可用。',provider_timeout:'等待供应商超时；停止等待不代表远端请求已撤销。',authentication_failed:'凭据未通过供应商验证。',provider_failed:'供应商调用失败，请查看连接或稍后重试。',provider_unavailable:'供应商暂不可用，系统按本次配置的次数与额度重试。',worker_interrupted:'执行中断，用量暂未知，保留预留额度。',model_budget_overrun:'供应商报告的用量超过本次上限，结果未被接纳。',provider_redirect_rejected:'供应商返回重定向，凭据未被转发。'};
  const explain=c=>errors[c]||({model_not_found:'该模型或连接已不存在。',claim_conflict:'任务已被其他执行者接续。',result_validation_pending:'已取得调用回执，结果仍待确认。'}[c])||c||'未测试';
  const kinds={trial:'单条试运行',automatic:'自动新增',backfill:'历史补跑'}, ops={connect:'连接测试',discover:'发现模型',probe:'模型能力测试',analyze:'评论分析'};
  let data=null, command=null, busy=false, loadFailed=false;
  let focusedPlan=new URLSearchParams(location.search).get('planRef');
  async function request(path,body){
    const r=await fetch(path,{method:body===undefined?'GET':'POST',headers:body===undefined?{}:{'Content-Type':'application/json'},body:body===undefined?undefined:JSON.stringify(body),cache:'no-store'});
    const v=await r.json();if(!r.ok)throw new Error(explain(v.error));return v;
  }
  const table=(heads,rows)=>`<div class="lgi-model-table"><table><thead><tr>${heads.map(h=>`<th scope="col">${h}</th>`).join('')}</tr></thead><tbody>${rows.join('')}</tbody></table></div>`;
  const button=(action,ref,label,disabled=false)=>`<button type="button" data-action="${action}" data-ref="${esc(ref)}" ${disabled?'disabled':''}>${label}</button>`;
  const note=t=>`<p class="lgi-model-note">${esc(t)}</p>`;
  function modelStatus(m){return !m.enabled?'连接已停用':!m.test?'尚未测试':m.test.commentQualified?'可调用 · 评论输出通过校验':m.test.modelCallable?(m.test.failureCode?explain(m.test.failureCode):'可调用 · 评论输出未通过校验'):explain(m.test.failureCode);}
  async function load(){
    try { data=await request(api+(focusedPlan?'?planRef='+encodeURIComponent(focusedPlan):'')); } catch(e) { loadFailed=true; data=null; $('storage-state').textContent='配置未就绪'; document.querySelectorAll('.lgi-model-settings button:not(#reload),.lgi-model-settings input,.lgi-model-settings select').forEach(el=>el.disabled=true); $('connection-list').replaceChildren(); throw e; }
    if(loadFailed){$('settings-feedback').textContent='连接已恢复，配置已重新读取。';loadFailed=false;}
    document.querySelectorAll('.lgi-model-settings button,.lgi-model-settings input,.lgi-model-settings select').forEach(el=>el.disabled=false);
    $('storage-state').textContent=data.secretStorage==='SYNTHETIC_PREVIEW_ONLY'?'合成隔离预览 · 凭据仅为测试标记 · 非真实模型质量验收':'本机工作空间 · 凭据保存于 macOS Keychain';
    $('connection-list').innerHTML=data.connections.length?table(['供应商 / API 地址','模型与状态','操作'],data.connections.map(c=>{
      const models=data.models.filter(m=>m.connectionVersionRef===c.versionRef);
      return `<tr><td><strong>${esc(c.name)}</strong>${note(c.baseUrl)}${note(c.api)}</td><td>${models.length?models.map(m=>`<div>${esc(m.modelId)}${note(modelStatus(m))}</div>`).join(''):note('尚未配置模型')}${!c.enabled?note('连接已停用'):''}</td><td>${button('edit-connection',c.connectionRef,'编辑与测试')} ${button('toggle',c.connectionRef,c.enabled?'停用':'启用')}</td></tr>`;
    })):note('还没有供应商。点击“添加供应商”，填写地址、API Key 和模型。');
    const selected=data.config?.modelRef;
    $('default-model').innerHTML='<option value="">请选择已通过评论校验的模型</option>'+data.models.filter(m=>(m.enabled&&m.test?.commentQualified)||m.modelRef===selected).map(m=>`<option value="${m.modelRef}">${esc(m.connectionName+' / '+m.modelId+(m.enabled?'':'（连接已停用）'))}</option>`).join('');
    if(data.config)for(const [key,value]of Object.entries(data.config)){const field=$('config-form').elements.namedItem(key);if(field)field.value=value;}
    ['trial','automatic','backfill'].forEach(id=>$(id).disabled=!data.config);
    renderActivity();
    if(focusedPlan)$('research-controls').open=true;
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
    command=action;command.id=crypto.randomUUID();$('dialog-test').hidden=action.kind!=='connection';$('dialog-title').textContent=title;$('dialog-fields').innerHTML=fields;$('dialog-feedback').textContent='';$('dialog-submit').textContent=label;$('dialog-submit').disabled=false;$('model-dialog').showModal();
  }
  function close(){if(busy)return;const key=$('model-command').elements.namedItem('apiKey');if(key)key.value='';$('dialog-fields').replaceChildren();command=null;$('model-dialog').close();}
  const textField=(name,label,value='',extra='')=>`<label>${label}<input name="${name}" value="${esc(value)}" ${extra}></label>`;
  function endpointPreview() {
    const f=$('model-command').elements, raw=f.baseUrl.value.trim(), protocol=f.api.value;
    try {
      const u=new URL(raw);
      if(!['http:','https:'].includes(u.protocol)||u.username||u.password||u.search||u.hash)throw new Error();
      let p=u.pathname.replace(/\/+$/,'');
      if(protocol==='anthropic-messages')p=p.replace(/\/v1(?:\/messages)?$/,'');
      else {p=p.replace(protocol==='openai-responses'?/\/responses$/:/\/chat\/completions$/,'');if(!p)p='/v1';}
      $('endpoint-preview').textContent='模型请求地址：'+u.origin+p+(protocol==='anthropic-messages'?'/v1/messages':protocol==='openai-responses'?'/responses':'/chat/completions');
    }catch{$('endpoint-preview').textContent='填写有效的 API 地址后显示最终请求地址。';}
  }
  function editConnection(c){
    if(!data)return;
    const models=data.models.filter(m=>m.connectionVersionRef===c?.versionRef);
    open(c?'编辑供应商':'添加供应商',
      textField('name','连接名称',c?.name||'','required maxlength="100" placeholder="例如：我的模型服务"')+
      `<label>接口协议<select name="api"><option value="openai-completions" ${c?.api==='openai-completions'?'selected':''}>OpenAI 兼容 · Chat Completions</option><option value="openai-responses" ${c?.api==='openai-responses'?'selected':''}>OpenAI · Responses</option><option value="anthropic-messages" ${c?.api==='anthropic-messages'?'selected':''}>Anthropic · Messages</option></select></label>`+
      textField('baseUrl','API 地址',c?.baseUrl||'','type="url" required maxlength="1000" placeholder="https://api.example.com/v1"')+
      '<p id="endpoint-preview" class="lgi-model-note"></p>'+
      textField('apiKey',c?'API Key（留空沿用已存凭据）':'API Key','','type="password" autocomplete="new-password" maxlength="4096"')+
      textField('modelId','模型 ID',models[0]?.modelId||'','maxlength="200" list="discovered-models" placeholder="填写供应商提供的准确模型 ID"')+
      '<datalist id="discovered-models"></datalist><button type="button" id="dialog-discover">保存连接并获取模型</button>'+
      note('也可直接填写模型 ID，无需获取目录。保存并测试会保存当前配置，再发送一条合成请求，可能产生少量费用；上限 10 秒、1024 输出 token，无自动重试。')+
      `<details><summary>高级设置</summary><label class="lgi-model-check"><input type="checkbox" name="localEndpoint" ${c?.localEndpoint?'checked':''}>本机回环服务（允许 HTTP）</label>${note('更换地址或协议时须重新填写凭据。新配置保留旧任务的原版本；停用连接可阻止其后续调用。')}</details>`,
      {kind:'connection',connection:c,connectionRef:c?.connectionRef||crypto.randomUUID(),models},'保存');
    endpointPreview();
    $('model-command').oninput=e=>{if(command?.kind!=='connection')return;endpointPreview();$('dialog-feedback').textContent='配置已修改；需要重新测试。';if(['api','baseUrl','apiKey','localEndpoint'].includes(e.target.name))$('discovered-models').replaceChildren();};
    $('dialog-discover').onclick=()=>runConnection('discover');
  }
  function lockConnection(locked){
    busy=locked; $('model-command').querySelectorAll('input,select,button').forEach(el=>el.disabled=locked);
  }
  async function persistConnection(){
    const f=new FormData($('model-command')),c=command;
    const fields={name:String(f.get('name')).trim(),api:f.get('api'),baseUrl:String(f.get('baseUrl')).trim(),apiKey:f.get('apiKey'),localEndpoint:f.has('localEndpoint')};
    const old=c.connection;
    if(!old||fields.name!==old.name||fields.api!==old.api||fields.baseUrl!==old.baseUrl||fields.localEndpoint!==old.localEndpoint||fields.apiKey){
      if(old&&!fields.apiKey&&(fields.baseUrl!==old.baseUrl||fields.api!==old.api||fields.localEndpoint!==old.localEndpoint))throw new Error('更换地址或协议时，请重新填写 API Key。');
      // Same payload retries keep its id; edited drafts get a new version identity.
      const fingerprint=JSON.stringify(fields);
      if(c.draftFingerprint!==fingerprint){c.id=crypto.randomUUID();c.draftFingerprint=fingerprint;}
      const saved=await request(api+'/connections',{versionRef:c.id,connectionRef:c.connectionRef,expectedRevision:old?.revision||0,...fields});
      c.connection={...fields,apiKey:undefined,connectionRef:c.connectionRef,versionRef:saved.versionRef,revision:saved.revision,enabled:old?.enabled??true};
      c.models=[]; c.draftFingerprint=null; $('model-command').elements.apiKey.value='';
    }
    const modelId=String(f.get('modelId')).trim();
    let model=c.models.find(m=>m.modelId===modelId);
    if(modelId&&!model){
      if(c.pendingModelId!==modelId){c.pendingModelId=modelId;c.pendingModelRef=crypto.randomUUID();}
      await request(api+'/models',{modelRef:c.pendingModelRef,connectionVersionRef:c.connection.versionRef,modelId});
      model={modelRef:c.pendingModelRef,connectionVersionRef:c.connection.versionRef,modelId};c.models.push(model);c.pendingModelRef=null;c.pendingModelId=null;
    }
    return {connection:c.connection,model};
  }
  async function runConnection(mode){
    if(busy||!command||!$('model-command').reportValidity())return;
    if(mode==='test'&&!$('model-command').elements.modelId.value.trim()){$('dialog-feedback').textContent='请填写模型 ID，再测试实际调用。';$('model-command').elements.modelId.focus();return;}
    // Read fields before disabling them: disabled inputs do not enter FormData.
    busy=true; $('dialog-feedback').textContent='正在保存配置…';
    try {
      const pending=persistConnection();lockConnection(true);
      const {connection,model}=await pending;
      if(mode==='save'){$('dialog-feedback').textContent='配置已保存，本次没有发起调用。';}
      else{
        $('dialog-feedback').textContent=mode==='test'?'配置已保存，正在测试模型调用…':'连接已保存，正在获取模型列表…';
        const receipt=await request(api+'/probes',{invocationRef:crypto.randomUUID(),connectionVersionRef:connection.versionRef,modelRef:mode==='test'?model.modelRef:null,operation:mode==='test'?'probe':'discover'});
        const r=receipt.replayed?receipt.result:receipt;
        if(mode==='discover'){
          $('discovered-models').innerHTML=(r?.modelIds||[]).map(id=>`<option value="${esc(id)}"></option>`).join('');
          $('dialog-feedback').textContent=r?.ok?`取得 ${r.modelIds?.length||0} 个候选模型。请在模型 ID 中选择，再测试调用。`:'连接已保存。'+explain(r?.failureCode);
        }else{
          $('dialog-feedback').textContent=r?.modelCallable?`模型${r.ok?'调用成功':'已响应'}${r.elapsedMs!=null?'，耗时 '+r.elapsedMs+' ms':''}。`+(r.commentQualified?'合成评论输出通过校验，可设为默认。':(r.failureCode?explain(r.failureCode):'评论输出未通过校验，暂不能设为评论分析默认。')):'配置已保存，测试失败：'+explain(r?.failureCode);
        }
      }
      await load();
      const latest=data.connections.find(c=>c.connectionRef===connection.connectionRef);
      if(latest){command.connection=latest;$('model-command').elements.baseUrl.value=latest.baseUrl;endpointPreview();}
      if(mode==='save'){lockConnection(false);close();$('settings-feedback').textContent='供应商配置已保存。保存没有发起模型调用。';}
    }catch(e){$('dialog-feedback').textContent=(command?.connection?'已保存的连接仍保留。':'')+e.message;}
    finally{lockConnection(false);}
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
    event.preventDefault();if(command?.kind==='connection'){await runConnection(event.submitter?.value==='test'?'test':'save');return;}if(busy)return;busy=true;$('dialog-submit').disabled=true;
    try{
      const f=new FormData($('model-command')),c=command;let result;
      if(c.kind==='resume')result=await request(api+'/plans/'+c.planRef+'/resume',{expectedRevision:c.revision});
      else{
        const refs=f.getAll('sourceRefs');if(c.kind!=='automatic'&&(refs.length===0||(c.kind==='trial'&&refs.length!==1)))throw new Error('请先选择要分析的原声。');
        result=await request(api+'/plans',{planRef:c.id,configRef:c.configRef,kind:c.kind,sourceRefs:refs,sourceLimit:c.sourceLimit||refs.length,tokenLimit:c.tokenLimit||Number(f.get('tokenLimit')),expectedAutoPlanRef:c.kind==='automatic'?c.expectedAutoPlanRef:null});
      }
      busy=false;close();await load();$('existing-plans').innerHTML=(result?.existingPlans||[]).map(p=>`<p>所选来源已有${esc(kinds[p.kind])}：<a href="/settings/models?planRef=${p.planRef}">查看所属计划${p.enabled?'':'并恢复'}</a>。本次未为它另建任务或补充额度。</p>`).join('');$('settings-feedback').textContent=result?.queued===0?'请求已保存；相同来源与配置已有任务，未重复调用。':'已保存。评论计划由执行循环接续，调用结果显示在额度与运行中。';
    }catch(e){$('dialog-feedback').textContent=e.message;}finally{busy=false;$('dialog-submit').disabled=false;}
  }
  $('config-form').addEventListener('submit',async e=>{e.preventDefault();const button=e.submitter;button.disabled=true;try{const f=new FormData(e.currentTarget),body={configRef:crypto.randomUUID(),expectedConfigRef:data.config?.configRef||null};for(const[k,v]of f)body[k]=k==='modelRef'?v:Number(v);await request(api+'/config',body);await load();$('settings-feedback').textContent='用途配置已保存，没有发起分析。';}catch(e){$('settings-feedback').textContent=e.message;}finally{button.disabled=false;}});
  $('new-connection').onclick=()=>editConnection(null);$('automatic').onclick=automatic;
  $('trial').onclick=()=>chooseSources('trial').catch(showError);$('backfill').onclick=()=>chooseSources('backfill').catch(showError);
  $('reload').onclick=()=>load().catch(showError);$('dialog-close').onclick=close;$('model-dialog').addEventListener('cancel',e=>{e.preventDefault();close();});$('model-command').addEventListener('submit',submit);
  function showError(e){$('settings-feedback').textContent=e.message;}
  document.addEventListener('click',async e=>{
    const b=e.target.closest('[data-action]');if(e.target.id==='find-sources'){try{await searchSources();}catch(e){$('dialog-feedback').textContent=e.message;}return;}if(!b)return;b.disabled=true;
    try{const ref=b.dataset.ref,action=b.dataset.action;if(action==='edit-connection')editConnection(data.connections.find(c=>c.connectionRef===ref));
      else if(action==='toggle'){const c=data.connections.find(c=>c.connectionRef===ref);await request(api+'/connections/state',{connectionRef:ref,expectedRevision:c.revision,enabled:!c.enabled});await load();}
      else if(action==='resume')resumePlan(data.plans.find(p=>p.planRef===ref));
      else if(action==='stop'){await request(api+'/plans/'+ref+'/stop',{});await load();}
    }catch(e){showError(e);}finally{b.disabled=false;}
  });
  load().then(()=>{const ref=new URLSearchParams(location.search).get('sourceRef');if(ref&&data.config)return chooseSources('trial',ref);}).catch(showError);
})();
