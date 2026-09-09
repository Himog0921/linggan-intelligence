(() => {
  'use strict';
  const esc=v=>String(v??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
  const labels={pending:'待处理',running:'分析中',succeeded:'有研究结果',no_signal:'无研究信号',failed:'失败',low_information:'旧版低信息',dropped:'已过滤噪声',anomaly:'异常',context_missing:'缺少上下文',restricted:'当前受限',source_limit:'超出本批数量上限'};
  const reasons={whitespace_or_encoding:'规范了空白或字符转义',source_too_long:'内容超过当前处理长度上限',empty_or_damaged:'空内容或编码损坏',reaction_only:'旧规则保留的表情或标点',context_dependent:'需要结合父评论或作品理解',contact_masked:'联系方式在外发时遮盖'};
  const errors={comment_daily_schema_missing:'每日研究尚未完成数据库升级，原声浏览仍可使用。',model_not_qualified:'请先测试模型调用，再保存默认研究模型与额度。',model_revision_conflict:'设置已变化，请关闭后重新打开。',model_source_unavailable:'所选评论已有不可读或版本变化，请刷新后重选。',invalid_model_command:'请核对选择范围与额度。',model_database_unavailable:'研究服务暂不可用。'};
  const failures={model_input_limit:'作品上下文与单条评论超过输入额度，尚未调用模型',model_budget_overrun:'供应商报告用量超出单次额度',context_missing:'父评论与作品正文均未取得',source_limit:'保留在批次中，未进入本次数量额度',worker_interrupted:'执行中断，远端用量可能未知',json_or_schema_invalid:'输出结构不符合合同',missing_comment:'模型未返回这一条评论',quote_missing_or_ambiguous:'引用缺失或在原文中不唯一',quote_redacted_or_empty:'引用为空或包含遮盖内容',facets_or_evidence_invalid:'研究字段或引用不符合合同',provider_failed:'供应商未交付完整输出',provider_timeout:'供应商响应超时',context_changed:'调用期间上下文变化，结果未接纳',source_or_plan_unavailable:'来源受限或计划已暂停',model_disabled:'连接或计划已暂停',json_invalid:'返回内容不是有效 JSON',schema_invalid:'评论包结构不符合要求',item_schema_invalid:'评论字段缺失或类型错误',field_schema_invalid:'该研究字段缺失、类型或取值不符合要求',output_bounds_invalid:'评论字段数量或长度超出限制',output_bounds:'评论字段数量或长度超出限制',unexpected_comment:'返回了输入范围外的评论',unknown_comment:'返回了输入范围外的评论编号',duplicate_comment:'重复返回评论',quote_out_of_bounds:'引用位置超出原声范围',provider_output_incomplete:'输出未完整结束',partial_fields_rejected:'部分研究字段未通过校验',all_fields_rejected:'所有研究字段均未通过校验',outcome_conflict:'结果状态与研究字段互相矛盾',interpretable_without_evidence:'声明可理解，但没有提供研究字段',uncertainty_reason_missing:'没有说明含义不确定的原因',evidence_bounds:'每项研究字段须提供一至四条原声引用',context_evidence_required_or_conflicting:'上下文引用与表达依据不一致',context_fragment_unknown:'引用了未提供的上下文片段',context_quote_missing_or_ambiguous:'上下文引用缺失或不唯一',problem_bounds:'问题名称或定义长度不符合要求',stance_bounds:'立场对象长度不符合要求',field_rejected:'该字段未通过校验'};
  const cleanLabels={direct:'可直接分析',context:'需要上下文',low_information:'旧版低信息',dropped:'已过滤噪声',anomaly:'异常'};
  const date=v=>v?new Date(v).toLocaleString('zh-CN',{timeZone:'Asia/Shanghai',month:'2-digit',day:'2-digit',hour:'2-digit',minute:'2-digit'}):'未知';
  let overview=null,detailsGeneration=0;
  const dialog=document.createElement('dialog');dialog.className='lgi-research-dialog lgi-daily-dialog';document.body.append(dialog);
  const ask=async(path,body)=>{const r=await fetch('/api/local/comment-research/daily'+path,{method:body?'POST':'GET',headers:body?{'Content-Type':'application/json'}:{},body:body?JSON.stringify(body):undefined,cache:'no-store'});const v=await r.json();if(!r.ok)throw new Error(errors[v.error]||'操作没有完成，请保留输入后重试。');return v;};
  function show(title,body){dialog.innerHTML=`<div class="lgi-research-dialog-heading"><h2>${esc(title)}</h2><button type="button" data-daily-close>关闭</button></div>${body}`;if(!dialog.open)dialog.showModal();}
  const loading=title=>show(title,'<p role="status">正在读取…</p>');
  const fail=e=>{const el=dialog.querySelector('[data-form-error]');if(el)el.textContent=e.message;else show('未完成',`<p role="alert">${esc(e.message)}</p>`);};
  const api={onRefresh:()=>{}};
  api.cleaningHtml=p=>`<section><h3>清洗处理</h3>${p?.cleanState?`<p>${esc(cleanLabels[p.cleanState])} · ${esc((p.cleaning?.reasons||[]).map(r=>reasons[r]||'规则处理').join('；')||'保留原始表达')}</p><details><summary>查看规范后的文本</summary><blockquote>${esc(p.cleaning?.text||'没有可分析文本')}</blockquote></details><p class="lgi-research-meta">原文完整保留，外发模型时另行遮盖可识别联系方式。</p>`:'<p>尚未清洗，不影响阅读与人工收存。</p>'}</section>`;
  api.mediaHtml=items=>`<details><summary>已有媒体文字</summary>${(items||[]).length?(items||[]).map(v=>`<p>${esc(({ocr_text:'图片文字',frame_ocr_text:'视频画面文字',asr_text:'视频转录'})[v.kind]||'媒体文字')} · ${v.state==='ACQUIRED'?'已取得':'尚不可用'}</p>${v.displayText?`<blockquote>${esc(v.displayText)}</blockquote>`:''}`).join(''):'<p>尚未取得 OCR 或转录文字。可从来源作品查看已有媒体。</p>'}</details>`;
  api.render=data=>{
    overview=data;
    document.getElementById('result-count').textContent=`最近 ${data.items.length} 批 · 北京时间`;
    document.getElementById('results').innerHTML=`<p class="lgi-research-meta">每日研究${data.schedule.enabled?'已启用':'未启用或已暂停'}${data.schedule.next_end?' · 下一个待封存截点 '+esc(date(data.schedule.next_end)):''}。数字只描述本机观察样本，确定性噪声不进入研究语料，历史记录与异常分别保留。重试失败会占用本批剩余额度，未知用量预留不释放。</p>`+(data.items.length?`<table><thead><tr><th>研究批次</th><th>材料范围</th><th>处理进展</th><th>额度使用</th><th>操作</th></tr></thead><tbody>${data.items.map(b=>`<tr><td>${b.kind==='daily'?'每日新增':'指定样本'}<div class="lgi-research-meta">${esc(date(b.start))}—${esc(date(b.end))}</div></td><td>${b.total.toLocaleString()} 条<div class="lgi-research-meta">${b.works.toLocaleString()} 篇当前可读作品</div></td><td>${Object.entries(b.counts).map(([k,n])=>`${esc(labels[k]||k)} ${n}`).join(' · ')||'没有新增评论'}${!b.enabled?'<div class="lgi-research-meta">批次已暂停</div>':''}<div class="lgi-research-meta">${Object.entries(b.cleaning||{}).map(([k,n])=>esc(cleanLabels[k]||'待清洗')+' '+n).join(' · ')}</div>${b.counts.pending&&b.tokenLimit-b.chargedTokens<b.nextReservation?'<div class="lgi-research-meta">剩余额度不足以预留下一包</div>':''}</td><td>${b.chargedTokens.toLocaleString()} / ${b.tokenLimit.toLocaleString()}<div class="lgi-research-meta">Token · 含用量未知时的预留</div></td><td><button type="button" data-batch="${b.batchRef}">查看</button><button type="button" data-batch-toggle="${b.batchRef}" data-enabled="${!b.enabled}">${b.enabled?'暂停':'恢复'}</button>${b.counts.failed?`<button type="button" data-batch-retry="${b.batchRef}" data-command="${crypto.randomUUID()}" ${b.enabled?'':'disabled'}>重试失败</button>`:''}</td></tr>`).join('')}</tbody></table>`:'<p class="lgi-research-empty">还没有研究批次。可在「研究设置」中授权自动新增、历史补齐和日额度；启用后按 23:00 截点自动建立符合资格的研究范围。</p>');
  };
  async function modelSettings(){const r=await fetch('/api/local/model-settings',{cache:'no-store'});if(!r.ok)throw new Error('模型设置暂不可读。');return r.json();}
  function modelSummary(m){if(!m.model?.modelConnected)return '<p>'+esc(({NEEDS_SELECTION:'已有模型调用成功，请选择并保存默认模型。',NEEDS_CALL_TEST:'请在供应商弹窗中测试模型调用，无需先设置默认模型。',NEEDS_QUALIFICATION:'请在供应商弹窗中测试模型调用，无需先设置默认模型。',PAUSED:'模型连接已暂停，请先启用。'})[m.model?.modelState]||'请添加供应商并完成测试，再选择默认模型。')+' <a href="/settings/models">前往模型设置</a></p>';const model=(m.models||[]).find(v=>v.modelRef===m.config.modelRef);return `<p>研究模型：${esc(model?.modelId||'当前冻结配置')} · ${esc(model?.connectionName||'已配置供应商')}。所选评论、父评论与作品已有文字会经清洗及联系方式遮盖后发送。</p>`;}
  api.openSelected=async refs=>{loading('分析所选评论');try{const m=await modelSettings();const id=crypto.randomUUID();show('分析所选评论',`${modelSummary(m)}<p>选中 ${refs.length} 条评论。按所属作品分包；不增加采集。</p>${m.model?.modelConnected?'<form data-daily-selected><label>本批 Token 总上限<input name="tokens" type="number" min="1024" max="10000000" value="100000" required></label><p data-form-error role="status"></p><button type="submit">确认范围并开始分析</button></form>':''}`);dialog.querySelector('form')?.addEventListener('submit',async e=>{e.preventDefault();const b=e.submitter;b.disabled=true;try{await ask('/selected',{batchRef:id,configRef:m.config.configRef,sourceRefs:refs,tokenLimit:Number(new FormData(e.target).get('tokens'))});dialog.close();document.getElementById('research-detail').close();api.onRefresh(true);}catch(err){fail(err);}finally{b.disabled=false;}});}catch(e){fail(e);}};
  api.openSettings=async()=>{loading('每日研究设置');try{const [data,m]=await Promise.all([ask(''),modelSettings()]);const s=data.schedule;show('每日研究设置',`${modelSummary(m)}<p>北京时间 23:00 固定批次。首次启用从启用时刻开始；暂停期间不调用，恢复后按原截点补建批次。每批有独立额度。</p>${m.config?`<form data-daily-settings><label>每日自动研究<select name="enabled"><option value="false">暂停</option><option value="true" ${m.model?.modelConnected?'':'disabled'} ${s.enabled?'selected':''}>启用</option></select></label><label>每批最多分析评论数<input name="sources" type="number" min="1" max="3000" value="${s.source_limit}" required></label><label>每批 Token 总上限<input name="tokens" type="number" min="1024" max="10000000" value="${s.token_limit}" required></label><p class="lgi-research-meta">来源清单与超额条目保留。评论按模型预算自动分包；问题自动归并还需要在研究设置中开启，并配置问题召回模型。保存每日运行设置不会修改这些上下文开关。</p><p data-form-error role="status"></p><button type="submit">保存研究设置</button></form>`:''}`);dialog.querySelector('form')?.addEventListener('submit',async e=>{e.preventDefault();const b=e.submitter;b.disabled=true;const f=new FormData(e.target);try{if(f.get('enabled')==='true'&&!m.model?.modelConnected)throw new Error('请先完成模型调用测试并保存默认模型，再启用每日研究。');await ask('/schedule',{expectedRevision:s.revision,enabled:f.get('enabled')==='true',configRef:m.config.configRef,sourceLimit:Number(f.get('sources')),tokenLimit:Number(f.get('tokens'))});dialog.close();api.onRefresh();}catch(err){fail(err);}finally{b.disabled=false;}});}catch(e){fail(e);}};
  api.readSettings=()=>ask('');
  api.saveSchedule=payload=>ask('/schedule',payload);
  const policyFor=s=>{const p=s.autoPolicy||s.auto_policy||{};return {continuousNew:p.continuousNew??p.continuous_new??false,historicalEnabled:p.historicalEnabled??p.historical_enabled??false,historyStart:p.historyStart??p.history_start??'',outdatedPolicy:p.outdatedPolicy??p.outdated_policy??'disabled',unknownRetryMaxAttempts:p.unknownRetryMaxAttempts??p.unknown_retry_max_attempts??0,unknownRetryTokenLimit:p.unknownRetryTokenLimit??p.unknown_retry_token_limit??0,dayTokenLimit:p.dayTokenLimit??p.day_token_limit??s.tokenLimit??s.token_limit??100000,replayTokenLimit:p.replayTokenLimit??p.replay_token_limit??0,semanticTokenLimit:p.semanticTokenLimit??p.semantic_token_limit??0};};
  api.openSettings=async()=>{
    loading('自动研究设置');
    try {
      const [data, modelState] = await Promise.all([api.readSettings(), modelSettings()]);
      const schedule = data.schedule || {};
      const policy = policyFor(schedule);
      const configRef = modelState.config?.configRef || schedule.configRef || schedule.config_ref;
      if (!configRef) {
        show('自动研究设置', `${modelSummary(modelState)}<p>当前没有可保存的研究模型配置。请先完成模型设置。</p>`);
        return;
      }
      show('自动研究设置', `${modelSummary(modelState)}<p>北京时间 23:00 固定截点。首次启用从实际启用时刻开始；暂停恢复保留边界。保存不会立即调用模型。</p><form data-daily-settings><fieldset><legend>授权范围</legend><label>自动研究<select name="enabled"><option value="false">暂停</option><option value="true" ${modelState.model?.modelConnected ? '' : 'disabled'} ${schedule.enabled ? 'selected' : ''}>启用</option></select></label><label><input name="continuousNew" type="checkbox" ${policy.continuousNew ? 'checked' : ''}>持续研究新增评论</label><label><input name="historicalEnabled" type="checkbox" ${policy.historicalEnabled ? 'checked' : ''}>补齐历史未研究评论</label><label>历史起点<input name="historyStart" value="${esc(policy.historyStart)}" placeholder="2026-09-01T00:00:00+08:00"></label><label>已有结果更新<select name="outdatedPolicy"><option value="disabled" ${policy.outdatedPolicy === 'disabled' ? 'selected' : ''}>不自动更新</option><option value="current_only" ${policy.outdatedPolicy === 'current_only' ? 'selected' : ''}>仅当前范围</option><option value="historical" ${policy.outdatedPolicy === 'historical' ? 'selected' : ''}>按历史政策</option></select></label></fieldset><fieldset><legend>共享日额度</legend><label>每日 Token 总额度<input name="dayTokenLimit" type="number" min="1024" max="10000000" value="${esc(policy.dayTokenLimit)}" required></label><label>每日最多评论数<input name="sourceLimit" type="number" min="1" max="3000" value="${esc(schedule.sourceLimit ?? schedule.source_limit)}" required></label><label>未知用量额外恢复次数<input name="unknownRetryMaxAttempts" type="number" min="0" max="1" value="${esc(policy.unknownRetryMaxAttempts)}" required></label><label>未知用量恢复额度<input name="unknownRetryTokenLimit" type="number" min="0" value="${esc(policy.unknownRetryTokenLimit)}" required></label><label>规则比较额度<input name="replayTokenLimit" type="number" min="0" value="${esc(policy.replayTokenLimit)}" required></label><label>问题整理额度<input name="semanticTokenLimit" type="number" min="0" value="${esc(policy.semanticTokenLimit)}" required></label></fieldset><p class="lgi-research-meta">恢复、规则比较与问题整理共用日总额度；未知用量预留不因重试自动释放。</p><p data-form-error role="status"></p><button type="submit">保存自动研究设置</button></form>`);
      dialog.querySelector('form')?.addEventListener('submit', async (event) => {
        event.preventDefault();
        const submit = event.submitter;
        submit.disabled = true;
        const form = new FormData(event.target);
        try {
          const dayTokenLimit = Number(form.get('dayTokenLimit'));
          const autoPolicy = {
            continuousNew: form.has('continuousNew'),
            historicalEnabled: form.has('historicalEnabled'),
            historyStart: String(form.get('historyStart') || '').trim() || null,
            outdatedPolicy: form.get('outdatedPolicy'),
            unknownRetryMaxAttempts: Number(form.get('unknownRetryMaxAttempts')),
            unknownRetryTokenLimit: Number(form.get('unknownRetryTokenLimit')),
            dayTokenLimit,
            replayTokenLimit: Number(form.get('replayTokenLimit')),
            semanticTokenLimit: Number(form.get('semanticTokenLimit')),
          };
          if (autoPolicy.historicalEnabled && !autoPolicy.historyStart)
            throw new Error('启用历史补齐时必须给出明确的历史起点。');
          if (autoPolicy.unknownRetryTokenLimit + autoPolicy.replayTokenLimit + autoPolicy.semanticTokenLimit > dayTokenLimit)
            throw new Error('恢复、比较与整理额度之和不能超过每日总额度。');
          if (form.get('enabled') === 'true' && !modelState.model?.modelConnected)
            throw new Error('请先完成模型调用测试并保存默认模型，再启用自动研究。');
          await api.saveSchedule({ expectedRevision: schedule.revision, enabled: form.get('enabled') === 'true', configRef, sourceLimit: Number(form.get('sourceLimit')), tokenLimit: dayTokenLimit, autoPolicy });
          dialog.close();
          api.onRefresh();
        } catch (error) {
          fail(error);
        } finally {
          submit.disabled = false;
        }
      });
    } catch (error) {
      fail(error);
    }
  };
  async function showBatch(ref,after=null){const generation=++detailsGeneration;loading('批次处理明细');try{const data=await ask('/'+ref+(after?'?after='+after:''));if(generation!==detailsGeneration||!dialog.open)return;show('批次处理明细',`<p>逐条核对原声与分析。当前页最多 50 条；研究结果为候选解释。</p><table><thead><tr><th>评论</th><th>状态与原因</th><th>研究结果</th></tr></thead><tbody>${data.items.map(i=>`<tr><td><button type="button" class="lgi-voice-open" data-source="${i.sourceRef}"><span class="lgi-voice-preview">${esc(i.body||'来源当前受限')}</span></button></td><td>${esc(labels[i.state]||i.state)}<div class="lgi-research-meta">${esc(failures[i.failureCode]||i.failureCode||'')}</div></td><td>${i.analysis?(i.analysis.spans||[]).flatMap(s=>s.facets).map(f=>`<div>${esc(f.label)} <span class="lgi-research-meta">${f.basis==='explicit'?'直接表达':'推断'}</span></div>`).join('')||'未提取到研究信号':'尚无可展示结果'}</td></tr>`).join('')}</tbody></table>${data.nextCursor?`<button type="button" data-batch-next="${ref}" data-after="${data.nextCursor}">下一页</button>`:''}`);}catch(e){if(generation===detailsGeneration)fail(e);}}
  document.addEventListener('click',async e=>{const one=e.target.closest('[data-daily-source]');if(one){api.openSelected([one.dataset.dailySource]);return;}const close=e.target.closest('[data-daily-close]');if(close){dialog.close();return;}const retry=e.target.closest('[data-batch-retry]');if(retry){retry.disabled=true;try{const r=await ask('/'+retry.dataset.batchRetry+'/retry',{commandRef:retry.dataset.command});await api.onRefresh();document.getElementById('feedback').textContent='已重新排队 '+r.queued+' 条；仍受原批次次数与剩余额度限制，未知用量预留保留。';}catch(err){document.getElementById('feedback').textContent=err.message;}finally{retry.disabled=false;}return;}const b=e.target.closest('[data-batch]');if(b){showBatch(b.dataset.batch);return;}const next=e.target.closest('[data-batch-next]');if(next){showBatch(next.dataset.batchNext,next.dataset.after);return;}const toggle=e.target.closest('[data-batch-toggle]');if(toggle){toggle.disabled=true;try{await ask('/'+toggle.dataset.batchToggle,{enabled:toggle.dataset.enabled==='true'});api.onRefresh();}catch(err){document.getElementById('feedback').textContent=err.message;}finally{toggle.disabled=false;}}});
  dialog.addEventListener('close',()=>{detailsGeneration++;dialog.replaceChildren();});
  window.CommentDaily=api;
})();

(() => {
  const api = window.CommentDaily;
  if (!api?.render) return;
  const render = api.render;
  const batchTitle = (batch) => ({
    daily: "每日新增",
    backlog: "历史补齐／结果更新",
    supplement: "补充研究",
    selected: "指定范围研究",
  })[batch?.kind] || "指定范围研究";
  api.render = (data) => {
    render(data);
    (data.items || []).forEach((batch, index) => {
      const cell = document.querySelectorAll("#results tbody tr")[index]?.cells[0];
      const title = [...(cell?.childNodes || [])].find((node) => node.nodeType === Node.TEXT_NODE);
      if (title) title.textContent = batchTitle(batch);
      if (batch.kind === "backlog" && cell) {
        const note = document.createElement("div");
        note.className = "lgi-research-meta";
        note.textContent = "按已授权历史或结果更新排队，不计为今日新增需求。";
        cell.append(note);
      }
    });
  };
})();
