(() => {
  'use strict';
  const api = '/api/local/model-settings';
  const $ = selector => document.querySelector(selector);
  const escape = value => String(value ?? '').replace(/[&<>"']/g, char => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[char]));
  const date = value => value ? new Intl.DateTimeFormat('zh-CN',{dateStyle:'medium',timeStyle:'short',timeZone:'Asia/Shanghai'}).format(new Date(value)) : '未知';
  const note = value => `<p class="lgi-model-note">${escape(value)}</p>`;
  const table = (headers, rows) => `<div class="lgi-model-table"><table><thead><tr>${headers.map(value=>`<th scope="col">${escape(value)}</th>`).join('')}</tr></thead><tbody>${rows.join('')}</tbody></table></div>`;
  const state = { data:null, command:null, busy:false };
  const errorText = { invalid_model_command:'输入不符合模型设置要求。', model_revision_conflict:'设置已变化，请刷新后重试。', model_disabled:'连接未启用。', model_not_qualified:'请先完成模型能力测试。', embedding_not_qualified:'请先完成向量测试后再启用。', model_database_unavailable:'模型设置数据库暂不可用。' };

  async function request(path, options = {}) {
    const response = await fetch(path,{cache:'no-store',headers:options.body?{'Content-Type':'application/json'}:undefined,...options});
    const body = await response.json().catch(()=>({}));
    if(!response.ok) throw new Error(errorText[body.error] || '请求没有完成，请保留当前输入后重试。');
    return body;
  }
  function callable(model) { return model?.enabled && model?.modelCallable && model?.semanticQualified; }
  function modelState(model) { return !model?.enabled ? '连接已停用' : callable(model) ? '可用于评论研究 V1' : model?.modelCallable ? '模型已响应，但 V1 JSON 测试未通过' : '需要完成模型测试'; }
  function render() {
    const data = state.data;
    const models = data.models || [];
    $('#default-model').innerHTML = `<option value="">选择已测试的研究模型</option>${models.map(model=>`<option value="${escape(model.modelRef)}" ${data.config?.modelRef===model.modelRef?'selected':''} ${callable(model)?'':'disabled'}>${escape(`${model.connectionName} / ${model.modelId} · ${modelState(model)}`)}</option>`).join('')}`;
    const config = data.config;
    for (const [name, value] of Object.entries({inputTokenLimit:config?.inputTokenLimit||16000,outputTokenLimit:config?.outputTokenLimit||2000,timeoutSeconds:config?.timeoutSeconds||30,maxAttempts:config?.maxAttempts||2})) $('#config-form').elements[name].value = value;
    $('#default-guidance').textContent = config ? `默认研究模型已保存：${models.find(model=>model.modelRef===config.modelRef)?.modelId || '模型记录不可用'}。保存不会启动研究。` : '尚未保存默认研究模型。先添加并测试连接。';
    $('#connection-list').innerHTML = data.connections.length ? table(['连接','模型','状态'],data.connections.map(connection=>{
      const connectionModels=models.filter(model=>model.connectionVersionRef===connection.versionRef);
      return `<tr><td><strong>${escape(connection.name)}</strong>${note(`${connection.api} · ${connection.baseUrl}`)}<button data-action="edit-connection" data-ref="${connection.connectionRef}">编辑</button><button data-action="toggle-connection" data-ref="${connection.connectionRef}">${connection.enabled?'停用':'启用'}</button></td><td>${connectionModels.length?connectionModels.map(model=>`<p><strong>${escape(model.modelId)}</strong> · ${escape(modelState(model))}<button data-action="test-model" data-ref="${model.modelRef}">测试</button></p>`).join(''):note('尚未添加模型 ID。')}</td><td>${connection.enabled?'已启用':'已停用'}${connection.test?.failureCode?note(connection.test.failureCode):''}</td></tr>`;
    })) : note('还没有供应商连接。');
    const embedding = data.embedding;
    $('#embedding-state').textContent = !embedding?.configured ? '尚未配置向量召回模型。' : embedding.enabled && embedding.qualified ? `已启用：${embedding.modelId}，${embedding.dimensions} 维。` : embedding.qualified ? '已测试但尚未启用。' : '尚未通过向量测试。';
    const worker = data.worker || {};
    $('#worker-state').textContent = worker.recent ? `执行器最近活动：${date(worker.lastSeenAt)}。` : `执行器未在最近 90 秒报告活动。${worker.lastError ? `上次错误：${worker.lastError}` : ''}`;
    $('#invocation-list').innerHTML = data.invocations.length ? table(['时间','调用','状态与使用量'],data.invocations.map(row=>`<tr><td>${escape(date(row.createdAt))}</td><td>${escape(row.operation)} · ${escape(row.modelId || '连接测试')}</td><td>${escape(row.state)}${row.failureCode?note(row.failureCode):''}${note(`输入 ${row.inputTokens ?? '未知'} · 输出 ${row.outputTokens ?? '未知'} · 计入 ${row.chargedTokens} Token`)}</td></tr>`)) : note('还没有调用记录。');
  }
  async function load(){state.data=await request(api);render();}
  function open(title, fields, command, testLabel='保存并测试') { state.command=command; $('#dialog-title').textContent=title; $('#dialog-fields').innerHTML=fields; $('#dialog-feedback').textContent=''; $('#dialog-test').textContent=testLabel; $('#dialog-test').hidden=!command.test; $('#model-dialog').showModal(); }
  function close(){if(!state.busy){state.command=null;$('#model-dialog').close();}}
  const input=(name,label,value='',attrs='')=>`<label>${label}<input name="${name}" value="${escape(value)}" ${attrs}></label>`;
  function editConnection(connection) {
    const model=state.data.models.find(item=>item.connectionVersionRef===connection?.versionRef);
    open(connection?'编辑供应商连接':'添加供应商连接',input('name','连接名称',connection?.name||'','required maxlength="100"')+`<label>接口协议<select name="api"><option value="openai-completions" ${connection?.api==='openai-completions'?'selected':''}>OpenAI 兼容 Chat Completions</option><option value="openai-responses" ${connection?.api==='openai-responses'?'selected':''}>OpenAI Responses</option><option value="anthropic-messages" ${connection?.api==='anthropic-messages'?'selected':''}>Anthropic Messages</option></select></label>`+input('baseUrl','API 地址',connection?.baseUrl||'','required type="url" maxlength="1000"')+input('apiKey',connection?'API Key（留空沿用）':'API Key','','type="password" autocomplete="new-password" maxlength="4096"')+input('modelId','模型 ID',model?.modelId||'','required maxlength="200"')+`<label class="lgi-model-check"><input name="localEndpoint" type="checkbox" ${connection?.localEndpoint?'checked':''}>本机回环服务（允许 HTTP）</label>`+note('保存仅写入本机设置。点击测试才发送合成 V1 JSON 请求，可能产生少量费用。'),{kind:'connection',connection,model,test:true});
  }
  async function saveConnection(test) {
    const form=new FormData($('#model-command')), command=state.command, previous=command.connection;
    const fields={name:String(form.get('name')).trim(),api:form.get('api'),baseUrl:String(form.get('baseUrl')).trim(),apiKey:form.get('apiKey'),localEndpoint:form.has('localEndpoint')};
    const connectionRef=previous?.connectionRef||crypto.randomUUID();
    const saved=await request(`${api}/connections`,{method:'POST',body:JSON.stringify({versionRef:crypto.randomUUID(),connectionRef,expectedRevision:previous?.revision||0,...fields})});
    const model=await request(`${api}/models`,{method:'POST',body:JSON.stringify({modelRef:previous&&command.model?.modelId===String(form.get('modelId')).trim()?command.model.modelRef:crypto.randomUUID(),connectionVersionRef:saved.versionRef,modelId:String(form.get('modelId')).trim()})});
    if(test){const receipt=await request(`${api}/probes`,{method:'POST',body:JSON.stringify({invocationRef:crypto.randomUUID(),connectionVersionRef:saved.versionRef,modelRef:model.modelRef,operation:'probe'})});$('#dialog-feedback').textContent=receipt.semanticQualified?'V1 JSON 测试通过。':'模型已返回，但 V1 JSON 测试未通过。';await load();return;}
    await load();close();$('#settings-feedback').textContent='供应商连接已保存；尚未发送测试请求。';
  }
  function editEmbedding(){const options=state.data.models.filter(model=>model.enabled).map(model=>`<option value="${escape(model.modelRef)}" ${state.data.embedding?.modelRef===model.modelRef?'selected':''}>${escape(`${model.connectionName} / ${model.modelId}`)}</option>`).join('');open('向量召回模型',`<label>模型<select name="modelRef" required><option value="">选择模型</option>${options}</select></label><label class="lgi-model-check"><input name="enabled" type="checkbox" ${state.data.embedding?.enabled?'checked':''}>测试通过后启用向量召回</label>`+note('测试只发送合成文本。通过只证明连接和向量结构，不代表召回质量。'),{kind:'embedding',test:true},'保存并测试向量');}
  async function saveEmbedding(test){const form=new FormData($('#model-command'));const saved=await request(`${api}/embedding`,{method:'POST',body:JSON.stringify({expectedRevision:state.data.embedding.revision,modelRef:form.get('modelRef'),enabled:test?false:form.has('enabled')})});if(test){const receipt=await request(`${api}/embedding/probe`,{method:'POST',body:JSON.stringify({invocationRef:crypto.randomUUID(),configRef:saved.configRef})});$('#dialog-feedback').textContent=receipt.embeddingQualified?'向量测试通过。再次保存并勾选“启用”后才会用于新研究。':'向量测试未通过。';await load();return;}await load();close();$('#settings-feedback').textContent='向量召回配置已保存。';}
  async function submitDialog(event){event.preventDefault();if(state.busy)return;state.busy=true;try{if(!$('#model-command').reportValidity())return;await (state.command.kind==='connection'?saveConnection(event.submitter=== $('#dialog-test')):saveEmbedding(event.submitter=== $('#dialog-test')));}catch(error){$('#dialog-feedback').textContent=error.message;}finally{state.busy=false;}}
  $('#config-form').addEventListener('submit',async event=>{event.preventDefault();try{const form=new FormData(event.currentTarget);const model=state.data.models.find(item=>item.modelRef===form.get('modelRef'));if(!callable(model))throw new Error('请选择已通过 V1 JSON 测试的模型。');await request(`${api}/config`,{method:'POST',body:JSON.stringify({configRef:crypto.randomUUID(),expectedConfigRef:state.data.config?.configRef||null,modelRef:form.get('modelRef'),inputTokenLimit:Number(form.get('inputTokenLimit')),outputTokenLimit:Number(form.get('outputTokenLimit')),timeoutSeconds:Number(form.get('timeoutSeconds')),maxAttempts:Number(form.get('maxAttempts'))})});await load();$('#settings-feedback').textContent='默认研究模型已保存；没有启动评论研究。';}catch(error){$('#settings-feedback').textContent=error.message;}});
  document.addEventListener('click',async event=>{const button=event.target.closest('[data-action]');if(!button)return;try{const ref=button.dataset.ref;if(button.dataset.action==='edit-connection')editConnection(state.data.connections.find(item=>item.connectionRef===ref));else if(button.dataset.action==='toggle-connection'){const connection=state.data.connections.find(item=>item.connectionRef===ref);await request(`${api}/connections/state`,{method:'POST',body:JSON.stringify({connectionRef:ref,expectedRevision:connection.revision,enabled:!connection.enabled})});await load();}else if(button.dataset.action==='test-model'){const model=state.data.models.find(item=>item.modelRef===ref);const receipt=await request(`${api}/probes`,{method:'POST',body:JSON.stringify({invocationRef:crypto.randomUUID(),connectionVersionRef:model.connectionVersionRef,modelRef:model.modelRef,operation:'probe'})});await load();$('#settings-feedback').textContent=receipt.semanticQualified?'模型 V1 JSON 测试通过。':'模型测试未通过。';}}catch(error){$('#settings-feedback').textContent=error.message;}});
  $('#new-connection').addEventListener('click',()=>editConnection(null));$('#embedding-edit').addEventListener('click',editEmbedding);$('#reload').addEventListener('click',()=>load().catch(error=>{$('#settings-feedback').textContent=error.message;}));$('#dialog-close').addEventListener('click',close);$('#model-command').addEventListener('submit',submitDialog);$('#dialog-test').addEventListener('click',event=>{event.preventDefault();$('#model-command').requestSubmit($('#dialog-test'));});$('#model-dialog').addEventListener('cancel',event=>{event.preventDefault();close();});
  load().catch(error=>{$('#settings-feedback').textContent=error.message;});
})();
