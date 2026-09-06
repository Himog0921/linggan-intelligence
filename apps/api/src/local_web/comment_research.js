(() => {
  'use strict';
  const $ = (id) => document.getElementById(id);
  const api = '/api/local/comment-research';
  const esc = (value) => String(value ?? '').replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
  const params = new URLSearchParams(location.search);
  const initial = document.querySelector('[data-initial-view]').dataset.initialView;
  const state = {view:initial === 'queries' ? 'queries' : params.get('view') || 'voices',text:params.get('text') || '',workRef:params.get('workRef') || '',cursor:null,history:[],next:null,collectionRef:params.get('collectionRef') || '',generation:0,detailGeneration:0,collections:[],assets:new Map(),queries:new Map()};
  if (!['voices','groups','assets','queries'].includes(state.view)) state.view = 'voices';
  let detail = null;
  let command = null;
  let commandId = null;
  const dimensionLabels = {scene:'场景',problem:'问题',tried_method:'尝试过的方法',stated_failure_reason:'评论者自述的失败原因',emotion:'情绪',expectation:'期望',expression:'表达方式'};
  const analysisLabels = {pending:'待分析',running:'分析中',succeeded:'已形成候选',no_signal:'分析完成，未提取到信号',failed:'分析失败'};
  const failureLabels = {provider_timeout:'模型调用超过本地等待上限，可在三次尝试限额内重试',provider_unavailable:'模型服务不可用',invalid_output:'模型返回内容未通过引用校验',source_unavailable:'来源当前不可用',lease_expired:'执行租约已过期'};
  const errorLabels = {source_unavailable:'来源当前不可用于研究，正文和派生内容已停止展示。',revision_conflict:'记录已发生变化，请刷新后重试。',idempotency_conflict:'本次保存标识已有不同内容，请重新打开保存表单。',invalid_command:'内容或来源范围不符合要求，请核对后重试。',invalid_query:'查询条件或分页已失效，请重新查询。',comment_research_unavailable:'评论研究暂时无法读取，请检查本机数据服务和本包 schema。'};
  async function request(path, body) {
    const response = await fetch(path, {method:body ? 'POST':'GET',headers:body ? {'Content-Type':'application/json'}:{},body:body ? JSON.stringify(body):undefined,cache:'no-store'});
    const payload = await response.json().catch(() => ({}));
    if (!response.ok) throw new Error(errorLabels[payload.error] || '请求未完成，请保留当前输入并重试。');
    return payload;
  }
  function feedback(message, error=false) { $('feedback').textContent=message; $('feedback').classList.toggle('lgi-research-error',error); }
  function urlState() {
    const query = new URLSearchParams();
    if(state.view!=='queries') query.set('view',state.view);
    if(state.text) query.set('text',state.text);
    if(state.workRef) query.set('workRef',state.workRef);
    if(state.collectionRef) query.set('collectionRef',state.collectionRef);
    history.replaceState(null,'',`${location.pathname}${query.size ? '?' + query : ''}`);
  }
  function table(headers, rows) {
    return `<table><thead><tr>${headers.map(h=>`<th scope="col" style="width:${h[1]}">${esc(h[0])}</th>`).join('')}</tr></thead><tbody>${rows.join('')}</tbody></table>`;
  }
  function empty(text) { return `<p class="lgi-research-empty">${esc(text)}</p>`; }
  function resetPage() {state.cursor=null;state.history=[];state.next=null;}
  async function load() {
    const generation=++state.generation;
    $('search-text').value=state.text;
    $('search-form').hidden=state.view!=='voices';
    $('asset-tools').hidden=state.view!=='assets';
    document.querySelector('.lgi-research-tabs').hidden=state.view==='queries';
    document.querySelectorAll('[data-view]').forEach(el=>{if(el.dataset.view===state.view) el.setAttribute('aria-current','page');else el.removeAttribute('aria-current');});
    $('result-title').textContent=({voices:'原声浏览',groups:'问题分组',assets:'语料资产',queries:'已存查询'})[state.view];
    $('result-count').textContent='';$('results').setAttribute('aria-busy','true');$('results').innerHTML=empty('正在读取…');
    $('previous').hidden=true;$('next').hidden=true;feedback('');urlState();
    try {
      let data;
      if(state.view==='voices') {
        const q=new URLSearchParams({text:state.text});
        if(state.workRef) q.set('workRef',state.workRef);
        if(state.cursor) q.set('cursor',state.cursor);
        data=await request(`${api}?${q}`);
      } else if(state.view==='assets') {
        const q=new URLSearchParams();if(state.collectionRef) q.set('collectionRef',state.collectionRef);if(state.cursor) q.set('cursor',state.cursor);
        [data]=await Promise.all([request(`${api}/assets?${q}`),loadCollections()]);
      } else data=await request(`${api}/${state.view}`);
      if(generation!==state.generation) return;
      if(state.view==='voices') renderVoices(data);
      if(state.view==='assets') renderAssets(data);
      if(state.view==='groups') renderGroups(data);
      if(state.view==='queries') renderQueries(data);
      $('previous').hidden=state.history.length===0;$('next').hidden=!state.next;
    } catch(e) {if(generation===state.generation){$('results').innerHTML=empty('本次读取没有完成。可使用刷新重试。');feedback(e.message,true);}}
    finally {if(generation===state.generation)$('results').setAttribute('aria-busy','false');}
  }
  function renderVoices(data) {
    const {page,works}=data;state.next=page.nextCursor;
    $('result-count').textContent=`${page.total.toLocaleString()} 条匹配原声 · 本页 ${page.items.length} 条`;
    $('model-state').textContent='模型尚未接入 · 原声可直接研究';
    const byRef=new Map(works.map(w=>[w.workRef,w]));
    $('results').innerHTML=page.items.length ? table([['评论原声','50%'],['来源作品','28%'],['观察与操作','22%']],page.items.map(s=>{
      const w=byRef.get(s.workRef);return `<tr><td><blockquote>${esc(s.body ? Array.from(s.body).slice(0,350).join('') : '正文尚未取得')}</blockquote><div class="lgi-research-meta">${s.isReply?'回复':'主评论'} · 评论用户身份已隐藏${s.bodyTruncated?' · 显示开头片段':''}</div></td><td><span class="lgi-research-source-title">${esc(w?.title || '作品标题未知')}</span><div class="lgi-research-meta">${esc(w?.creatorDisplayName || '作品作者未知')}</div></td><td><div class="lgi-research-meta">观察于 ${esc(s.observedAt)}</div><button type="button" data-source="${s.sourceRef}">研读与收存</button></td></tr>`;
    })) : empty(state.text?'没有符合当前条件的可读原声。可以缩短关键词后重试。':'尚未取得可读评论。采集接纳后的评论会直接出现在这里，无需手动搬运。');
    if(state.workRef) feedback('当前范围限定为一个作品；清除地址中的 workRef 可返回全部作品。');
  }
  function renderAssets(data) {
    state.assets=new Map(data.items.map(a=>[a.assetRef,a]));state.next=data.nextCursor;$('result-count').textContent=`${data.total.toLocaleString()} 次人工收存`;
    $('results').innerHTML=data.items.length ? table([['收存表达','46%'],['为什么收存','32%'],['来源资格','22%']],data.items.map(a=>`<tr><td><blockquote>${esc(a.quote || '来源当前不可读')}</blockquote></td><td>${esc(a.reason || '来源受限，停止展示衍生内容')}<div class="lgi-research-meta">${esc(a.createdAt)} · ${esc(state.collections.find(c=>c.collectionRef===a.collectionRef)?.name || '未加入集合')}</div></td><td>${a.eligibility==='READABLE'?`${a.isCurrentSource?'当前来源版本':'固定历史来源版本'}<br><button type="button" data-source="${a.sourceRef}">查看来源</button>`:'来源不可用'}<button type="button" data-manage-asset="${a.assetRef}">整理与修订</button><div class="lgi-research-meta">修订 ${a.revision} · 人工收存不代表已证实</div></td></tr>`)) : empty('这个范围还没有收存表达。从原声浏览中选中一句有价值的话，写下理由即可保存。');
  }
  function renderGroups(data) {
    state.next=null;$('result-count').textContent=`${data.items.length} 个候选问题组${data.truncated?' · 显示前 100 组':''}`;
    $('results').innerHTML=data.items.length ? table([['候选问题','48%'],['样本范围','20%'],['查看表达','32%']],data.items.map((g,i)=>`<tr><td>${esc(g.label)}<div class="lgi-research-meta">${g.origin==='human'?'人工问题标注':'模型候选'} · 按相同问题标签归组</div></td><td>${g.sampleCount.toLocaleString()} 条当前原声</td><td>${g.sourceRefs.map((ref,n)=>`<button type="button" data-source="${ref}">样本 ${n+1}</button>`).join(' ')}</td></tr>`)) : empty('还没有可用的问题标注。可以研读原声并补充人工问题标注；模型接入后会产生有来源的候选。');
    feedback('这里是样本内候选标签分组，尚不是语义聚类、正式主题或需求规模。人工修订优先显示，分析历史保留在原声详情。');
  }
  function renderQueries(data) {
    state.queries=new Map(data.items.map(q=>[q.queryRef,q]));state.next=null;$('result-count').textContent=`${data.items.length} 个查询`;
    $('results').innerHTML=data.items.length ? table([['查询名称','35%'],['保存条件','45%'],['使用','20%']],data.items.map(q=>{
      const p=new URLSearchParams({text:q.text});if(q.workRef)p.set('workRef',q.workRef);
      return `<tr><td>${esc(q.name)}</td><td>${esc(q.text || '全部可读原声')}<div class="lgi-research-meta">${q.workRef?'限定作品':'跨全部作品'} · 每次使用重新查询最新可读材料</div></td><td><a href="/corpus/comments?${esc(p)}">打开查询</a><br><button type="button" data-manage-query="${q.queryRef}">编辑或删除</button></td></tr>`;
    })) : empty('还没有已存查询。在评论研究中设定条件后使用「保存查询」。保存不会触发采集或订阅。');
  }
  async function loadCollections() {
    const data=await request(`${api}/collections`);state.collections=data.items;
    $('collection-filter').innerHTML='<option value="">全部集合</option>'+data.items.map(c=>`<option value="${c.collectionRef}">${esc(c.name)} · ${c.savedCount}</option>`).join('');
    $('collection-filter').value=state.collectionRef;
  }
  async function openSource(ref) {
    const generation=++state.detailGeneration;detail=null;
    $('detail-content').innerHTML=empty('正在读取当前来源资格与研究历史…');
    if(!$('research-detail').open)$('research-detail').showModal();
    try {
      const value=await request(`${api}/sources/${ref}`);
      if(generation!==state.detailGeneration || !$('research-detail').open)return;
      detail=value;const s=value.source;
      const annotations=value.annotations;const human=annotations.human[0];
      $('detail-content').innerHTML=`<p class="lgi-research-meta">${s.isCurrent?'当前来源版本':'固定历史来源版本'} · 观察于 ${esc(s.observedAt)}</p><section><h3>原声</h3><blockquote id="source-body">${esc(s.body || '正文尚未取得')}</blockquote><p class="lgi-research-meta">${s.bodyTruncated?"当前仅显示原文前 4,000 字符。":""}选中一段原声后收存；没有选中时默认收存显示的全文。字符范围按 Unicode 字符记录。</p><div class="lgi-research-actions"><button type="button" id="asset-open" ${s.body?'':'disabled'}>收存表达</button><button type="button" id="annotation-open">编辑研究标注</button><a href="/corpus/evidence?work=${s.workRef}">查看来源作品</a></div></section><section><h3>作品上下文</h3><p>${esc(value.work?.title?.value || "作品标题尚未取得")}</p><blockquote>${esc(value.work?.body?.value || "作品正文尚未取得，不补全缺失上下文。")}</blockquote></section><section><h3>对话上下文</h3>${value.parent?`<blockquote>${esc(value.parent.body || '父评论正文未知')}</blockquote>`:`<p>${value.parentState==='NOT_APPLICABLE'?'这是一条主评论。':'父评论尚未取得或当前不可读，不据此补全含义。'}</p>`}</section><section><h3>人工研究标注</h3>${human?facetHtml(human.facets)+`<p>${esc(human.reason)}</p><p class="lgi-research-meta">修订 ${human.revision} · 共保留 ${annotations.human.length} 个版本</p>`:'<p>尚未标注。人工选择和模型输出分别保留。</p>'}</section><section><h3>分析记录</h3>${annotations.analysis.length?annotations.analysis.map(a=>`<p>${esc(analysisLabels[a.state] || '状态未知')} · <span class="lgi-research-meta">${esc(a.ruleVersion)} / ${esc(a.modelVersion)}</span></p>${a.contextReadable===false?'<p>关联上下文已受限，停止展示候选内容。</p>':''}${a.result?(a.result.spans || []).map(span=>facetHtml(span.facets)).join(''):''}${a.failureCode?`<p class="lgi-research-meta">${esc(failureLabels[a.failureCode] || a.failureCode)}</p>`:''}`).join(''):'<p>尚未分析。模型没有接入，原声浏览与人工研究可继续。</p>'}</section>`;
    } catch(e){if(generation===state.detailGeneration){$('detail-content').innerHTML=empty(e.message);detail=null;}}
  }
  function facetHtml(facets) {return facets.map(f=>`<p><strong>${esc(dimensionLabels[f.dimension] || f.dimension)}</strong>：${esc(f.label)} <span class="lgi-research-meta">${f.basis==='explicit'?'来源直接表达':'研究推断'}</span></p>`).join('');}
  function selectedSpan() {
    const node=$('source-body');const selection=window.getSelection();
    let start=0,end=Array.from(detail.source.body).length;
    if(selection?.rangeCount && !selection.isCollapsed) {
      const range=selection.getRangeAt(0);
      if(node.contains(range.startContainer)&&node.contains(range.endContainer)) {
        const prefix=range.cloneRange();prefix.selectNodeContents(node);prefix.setEnd(range.startContainer,range.startOffset);
        start=Array.from(prefix.toString()).length;end=start+Array.from(range.toString()).length;
      }
    }
    return {startChar:start,endChar:end,quote:Array.from(detail.source.body).slice(start,end).join('')};
  }
  async function openCommand(kind) {
    command=kind;commandId=crypto.randomUUID();$('command-feedback').textContent='';
    if(kind==='asset') {
      if(!detail?.source.body)return;const span=selectedSpan();await loadCollections();
      command={kind,...span,sourceRef:detail.source.sourceRef,sourceSha256:detail.sourceSha256};
      $('command-title').textContent='收存表达';
      $('command-fields').innerHTML=`<blockquote>${esc(span.quote)}</blockquote><p class="lgi-research-meta">范围 ${span.startChar}–${span.endChar} · 固定来源版本</p><label>收存理由<textarea name="reason" required maxlength="1000" placeholder="为什么这句话值得再次研究？"></textarea></label><label>集合<select name="collectionRef"><option value="">暂不加入集合</option>${state.collections.map(c=>`<option value="${c.collectionRef}">${esc(c.name)}</option>`).join('')}</select></label>`;
    } else if(kind==='annotation') {
      if(!detail)return;const human=detail.annotations.human[0];command={kind,sourceRef:detail.source.sourceRef,expectedRevision:human?.revision || 0};
      $('command-title').textContent='编辑研究标注';
      $('command-fields').innerHTML=Object.entries(dimensionLabels).map(([key,label])=>{
        const existing=human?.facets.find(f=>f.dimension===key);
        return `<label>${esc(label)}<input name="facet-${key}" maxlength="100" value="${esc(existing?.label || '')}" placeholder="没有依据时留空"><select name="basis-${key}" aria-label="${esc(label)}的依据"><option value="explicit">来源直接表达</option><option value="inferred" ${existing?.basis==='inferred'?'selected':''}>研究推断</option></select></label>`;
      }).join('')+'<label>修订理由<textarea name="reason" required maxlength="1000"></textarea></label>';
    } else {
      $('command-title').textContent=kind==='query'?'保存查询':'新建集合';
      $('command-fields').innerHTML=`<label>名称<input name="name" required maxlength="100"></label><p>${kind==='query'?'保存当前检索条件；下次打开时查询最新可读材料，不触发采集。':'集合保存人工选择的引用，不改变来源事实或总体统计。'}</p>`;
    }
    $('research-form-dialog').showModal();$('command-fields').querySelector('input,textarea,select')?.focus();
  }
  async function openManagement(kind, ref) {
    commandId=crypto.randomUUID();$('command-feedback').textContent='';
    if(kind==='asset-revision') {
      const [history]=await Promise.all([request(`${api}/assets/${ref}/history`),loadCollections()]);
      const current=history.items[0];if(current.withdrawn)throw new Error('这次收存已撤销，请刷新列表。');
      const readable=history.eligibility==='READABLE';
      command={kind,assetRef:ref,expectedRevision:current.revision,readable};
      $('command-title').textContent='整理语料资产';
      $('command-fields').innerHTML=`<p>固定来源和原始片段保持不变。每次整理保留修订记录。</p>${readable?`<label>收存理由<textarea name="reason" required maxlength="1000">${esc(current.reason)}</textarea></label><label>所属集合<select name="collectionRef"><option value="">不加入集合</option>${state.collections.map(c=>`<option value="${c.collectionRef}" ${c.collectionRef===current.collectionRef?'selected':''}>${esc(c.name)}</option>`).join('')}</select></label>`:'<p>来源当前不可读，理由和历史派生内容已隐藏。仍可撤销误收存。</p>'}<label>本次整理说明<textarea name="changeReason" required maxlength="1000" placeholder="记录修改或撤销的原因"></textarea></label><label>操作<select name="action">${readable?'<option value="edit">保存整理</option>':''}<option value="withdraw">撤销这次收存</option></select></label><p class="lgi-research-meta">撤销后不再进入语料资产和集合统计；保留审计记录，旧请求不会恢复收存。</p><details><summary>查看 ${history.items.length} 个修订版本</summary>${history.items.map(h=>`<p>修订 ${h.revision} · ${h.withdrawn?'已撤销':'收存'}<br>${esc(h.reason || '理由已隐藏或已撤销')}<br><span class="lgi-research-meta">${esc(h.changeReason || '整理说明已隐藏')}</span></p>`).join('')}</details>`;
    } else {
      const current=state.queries.get(ref);if(!current)return;
      command={kind:'query-revision',queryRef:ref,expectedRevision:current.revision};
      $('command-title').textContent='编辑已存查询';
      $('command-fields').innerHTML=`<label>查询名称<input name="name" required maxlength="100" value="${esc(current.name)}"></label><label>检索原声<input name="text" maxlength="200" value="${esc(current.text)}" placeholder="留空查询全部可读原声"></label><label>作品范围<select name="workRef"><option value="">跨全部作品</option>${current.workRef?`<option value="${current.workRef}" selected>保留当前限定作品</option>`:''}</select></label><label>操作<select name="action"><option value="edit">保存修改</option><option value="delete">删除这个查询</option></select></label><p class="lgi-research-meta">修改后使用新条件查询当前可读材料。删除不会删除语料，旧请求不会恢复查询。</p>`;
    }
    $('research-form-dialog').showModal();$('command-fields').querySelector('input,textarea,select')?.focus();
  }
  $('research-command-form').addEventListener('submit',async event=>{
    event.preventDefault();const form=new FormData(event.currentTarget);const button=$('command-submit');button.disabled=true;$('command-feedback').textContent='正在保存…';
    try {
      if(command==='query')await request(`${api}/queries`,{queryRef:commandId,name:form.get('name'),text:state.text,workRef:state.workRef || null});
      else if(command==='collection')await request(`${api}/collections`,{collectionRef:commandId,name:form.get('name')});
      else if(command?.kind==='asset')await request(`${api}/assets`,{assetRef:commandId,sourceRef:command.sourceRef,startChar:command.startChar,endChar:command.endChar,sourceSha256:command.sourceSha256,reason:form.get('reason'),collectionRef:form.get('collectionRef') || null});
      else if(command?.kind==='asset-revision') {
        const withdrawn=form.get('action')==='withdraw';
        await request(`${api}/assets/revisions`,{revisionRef:commandId,assetRef:command.assetRef,expectedRevision:command.expectedRevision,reason:withdrawn?null:form.get('reason'),collectionRef:withdrawn?null:form.get('collectionRef') || null,withdrawn,changeReason:form.get('changeReason')});
      } else if(command?.kind==='query-revision') {
        await request(`${api}/queries/revisions`,{revisionRef:commandId,queryRef:command.queryRef,expectedRevision:command.expectedRevision,name:form.get('name'),text:form.get('text'),workRef:form.get('workRef') || null,deleted:form.get('action')==='delete'});
      } else if(command?.kind==='annotation') {
        const facets=Object.keys(dimensionLabels).filter(key=>String(form.get(`facet-${key}`) || '').trim()).map(key=>({dimension:key,label:String(form.get(`facet-${key}`)).trim(),basis:form.get(`basis-${key}`)}));
        await request(`${api}/annotations`,{annotationRef:commandId,sourceRef:command.sourceRef,expectedRevision:command.expectedRevision,facets,reason:form.get('reason')});
      }
      $('research-form-dialog').close();feedback('已保存到本机研究记录。');
      if(command?.kind==='annotation')await openSource(command.sourceRef);
      if(state.view==='assets'||state.view==='queries'){resetPage();await load();}
    } catch(e){$('command-feedback').textContent=e.message;}
    finally {button.disabled=false;}
  });
  document.addEventListener('click',event=>{
    const close=event.target.closest('[data-close]');if(close){$(close.dataset.close).close();return;}
    const manageAsset=event.target.closest('[data-manage-asset]');if(manageAsset){openManagement('asset-revision',manageAsset.dataset.manageAsset).catch(e=>feedback(e.message,true));return;}
    const manageQuery=event.target.closest('[data-manage-query]');if(manageQuery){openManagement('query-revision',manageQuery.dataset.manageQuery).catch(e=>feedback(e.message,true));return;}
    const source=event.target.closest('[data-source]');if(source){openSource(source.dataset.source);return;}
    const tab=event.target.closest('[data-view]');if(tab){event.preventDefault();state.view=tab.dataset.view;resetPage();load();return;}
    const commands={'asset-open':'asset','annotation-open':'annotation','save-query-open':'query','collection-open':'collection'};
    const kind=commands[event.target.id];if(kind)openCommand(kind).catch(e=>feedback(e.message,true));
  });
  $('research-detail').addEventListener('close',()=>{state.detailGeneration++;detail=null;$('detail-content').replaceChildren();});
  $('search-form').addEventListener('submit',event=>{event.preventDefault();state.text=$('search-text').value.trim();resetPage();load();});
  $('collection-filter').addEventListener('change',()=>{state.collectionRef=$('collection-filter').value;resetPage();load();});
  $('next').addEventListener('click',()=>{state.history.push(state.cursor);state.cursor=state.next;load();});
  $('previous').addEventListener('click',()=>{state.cursor=state.history.pop() || null;load();});
  $('reload').addEventListener('click',()=>{resetPage();load();});
  load();
})();
