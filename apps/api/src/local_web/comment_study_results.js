/* COMMENT-RESULTS-V3-001. Only the three results renderers are registered here.
 * The overview, research commands and their server-receipt handlers stay untouched.
 * No source bodies, summaries or synthetic findings are persisted in the browser. */
(() => {
  'use strict';
  const escape = value => String(value ?? '').replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
  const text = value => typeof value === 'string' && value.trim() ? value : '';
  const number = value => typeof value === 'number' && Number.isFinite(value) && value >= 0 ? value : null;
  const count = value => number(value) === null ? '—' : new Intl.NumberFormat('zh-CN').format(value);
  const date = value => { const d = new Date(value || ''); return Number.isNaN(d.valueOf()) ? '时间未记录' : new Intl.DateTimeFormat('zh-CN',{timeZone:'Asia/Shanghai',month:'2-digit',day:'2-digit',hour:'2-digit',minute:'2-digit',hour12:false}).format(d); };
  const readable = row => row?.sourceState === 'known' && !!text(row.commentText);
  const keyOf = row => JSON.stringify([row?.commentKey?.workRef,row?.commentKey?.commentExternalId]);
  const unique = rows => {
    if(rows.some(row=>!text(row?.commentKey?.workRef)||!text(row?.commentKey?.commentExternalId)))throw new Error('原声定位信息不完整，请重新读取。');
    return [...new Map(rows.map(row => [keyOf(row),row])).values()];
  };
  const kinds = {first_record:'首次建档',definition_revision:'定义版本更新',evidence_added:'仅补入依据',unchanged:'已有问题'};
  const candidateStates = {all:'全部处理状态',pending:'正在归并',deferred_novel:'新表达，尚未建档',deferred_ambiguous:'问题边界待判断',deferred_context:'等待语境',retrieval_incomplete:'候选检索未完成',budget_stopped:'额度用尽，处理未完成',protocol_rejected:'模型输出未接纳',failed:'归并失败'};
  const metric = (value,label) => `<span class="sr3-metric"><b title="${escape(count(value))}">${number(value)!==null&&value>=10000?`${(value/10000).toFixed(1)}万`:count(value)}</b><span>${label}</span></span>`;
  const warning = p => p.definitionReadable !== true ? '定义依据当前不可读取' : p.definitionCurrent === false ? '当前定义依据已变化，需核对适用范围' : p.supportWorkCount === 1 ? '依据集中在 1 篇作品' : p.supportCommentCount === 0 ? '当前暂无可读支持原声' : '';
  const options = (map,value) => Object.entries(map).map(([key,label]) => `<option value="${key}"${key===value?' selected':''}>${label}</option>`).join('');
  const button = (action,label,extra='') => `<button type="button" class="sr3-button" data-sr3="${action}" ${extra}>${label}</button>`;
  const failure = (label,action='retry') => `<div class="sr3-message" role="status"><strong>${escape(label)}</strong>${button(action,'重新读取')}</div>`;
  const contractError = () => new Error('读取回执与当前范围不一致，请重新读取。');

  let pendingEntry=null;
  let incomingRoute=typeof initialStudyRoute!=='undefined'&&['problems','pending'].includes(initialStudyRoute.get('view'))?new URLSearchParams(initialStudyRoute):null;
  class StudyUserProblems extends HTMLElement {
    connectedCallback() {
      this.domain = typeof domainRef === 'string' ? domainRef : '';
      this.requests = new Map(); this.items = []; this.detail = null; this.evidence = [];
      this.readRoute(); this.route(false); this.renderShell();
      this.onResize=()=>this.dialogBounds();window.addEventListener('resize',this.onResize,{passive:true});
      this.onClick = event => this.click(event);
      this.onSubmit = event => { if(event.target.matches('[data-sr3-search]')) {event.preventDefault();this.q=new FormData(event.target).get('q').trim();this.cursor=null;this.route();void this.load();} };
      this.onChange = event => {
        const name=event.target.dataset.sr3Select;
        if(name==='order'){this.order=event.target.value;this.cursor=null;this.route();void this.load();}
        if(name==='candidate'){this.candidate=event.target.value;this.cursor=null;this.route();void this.load();}
      };
      this.addEventListener('click',this.onClick);this.addEventListener('submit',this.onSubmit);this.addEventListener('change',this.onChange);
      void this.load();
      if(this.problemRef)void this.openProblem(this.problemRef,false);
    }
    disconnectedCallback() {
      for(const controller of this.requests?.values() || [])controller.abort();
      this.requests?.clear();this.detail=null;this.evidence=[];this.items=[];
      this.removeEventListener('click',this.onClick);this.removeEventListener('submit',this.onSubmit);this.removeEventListener('change',this.onChange);
      window.removeEventListener('resize',this.onResize);
      this.dialog?.close();this.dialog?.remove();
    }
    readRoute() {
      const params=incomingRoute&&(!incomingRoute.get('domain')||incomingRoute.get('domain')===this.domain)?incomingRoute:new URLSearchParams(location.search),saved=history.state?.problemResults;
      incomingRoute=null;
      const same=saved?.domain===this.domain;
      this.view=['all','changes','clues'].includes(params.get('state'))?params.get('state'):(same&&['all','changes','clues'].includes(saved.view)?saved.view:'all');
      this.q=params.has('q')?params.get('q'):(same?saved.q||'':'');
      this.order=['recent','created','support'].includes(params.get('sort'))?params.get('sort'):(same&&['recent','created','support'].includes(saved.order)?saved.order:'recent');
      this.cursor=params.has('cursor')?params.get('cursor'):(same?saved.cursor||null:null);
      this.candidate=same&&candidateStates[saved.candidate]?saved.candidate:'all';
      this.density=same&&saved.density==='compact'?'compact':'standard';
      this.problemRef=params.get('problemRef')||null;
      if(pendingEntry?.domain===this.domain){this.view=pendingEntry.view;this.candidate=pendingEntry.candidate;this.cursor=null;this.q='';pendingEntry=null;}
    }
    route(push=true) {
      const url=new URL(location.href);url.searchParams.set('view','problems');
      for(const field of ['runRef','panel','signalRef','targetRef','workRef','commentWorkRef','commentExternalId','detail','signalKind','voiceRole','minLikes','postUsage'])url.searchParams.delete(field);
      for(const [key,value] of [['state',this.view],['q',this.q],['sort',this.order],['cursor',this.cursor],['problemRef',this.problemRef]]){
        if(value)url.searchParams.set(key,value);else url.searchParams.delete(key);
      }
      if(typeof selectedProblemRef!=='undefined')selectedProblemRef=this.problemRef;
      history[push?'pushState':'replaceState']({...history.state,problemResults:{domain:this.domain,view:this.view,q:this.q,order:this.order,cursor:this.cursor,candidate:this.candidate,density:this.density}},'',url);
    }
    async request(path,slot) {
      this.requests.get(slot)?.abort();
      const controller=new AbortController();this.requests.set(slot,controller);
      const url=new URL('/api/local/comment-study/'+path,location.origin);url.searchParams.set('domain',this.domain);
      const response=await fetch(url,{headers:{Accept:'application/json'},cache:'no-store',signal:controller.signal});
      const data=await response.json().catch(()=>null);
      if(!response.ok)throw new Error(data?.error?.message||`读取失败（${response.status}）`);
      if(!this.isConnected||controller.signal.aborted)throw new DOMException('Aborted','AbortError');
      if(data?.domainRef&&data.domainRef!==this.domain)throw contractError();
      return data;
    }
    find(selector){return this.querySelector(selector);}
    renderShell() {
      this.className='study-problems-workbench';
      this.innerHTML=`<div class="sr3-heading"><div><span class="sr3-eyebrow">评论研究 / 成果</span><h2>用户问题</h2></div><span class="sr3-heading-note">问题长期积累，原声随时可查</span></div>
        <div class="sr3-toolbar"><nav class="sr3-tabs" aria-label="问题阅读范围">${['all','changes','clues'].map((v,i)=>`<button type="button" data-sr3="view" data-value="${v}" aria-pressed="${this.view===v}">${['全部问题','最近变化','未归并线索'][i]}</button>`).join('')}</nav>
        <form data-sr3-search class="sr3-search" role="search"><label class="v7-sr-only" for="sr3-query">搜索问题标题与定义</label><input id="sr3-query" name="q" type="search" maxlength="300" autocomplete="off" value="${escape(this.q)}" placeholder="搜索问题标题与定义"><button type="submit" aria-label="搜索问题">搜索</button></form>
        <label class="sr3-select">排序<select data-sr3-select="order">${options({recent:'最近记录更新',created:'最近建档',support:'关联原声最多'},this.order)}</select></label>
        <div class="sr3-density" aria-label="卡片密度">${button('density','阅读卡',`data-value="standard" aria-pressed="${this.density==='standard'}"`)}${button('density','紧凑卡',`data-value="compact" aria-pressed="${this.density==='compact'}"`)}</div></div>
        <div class="sr3-range"><span data-sr3-status role="status">正在读取当前领域…</span><div>${button('scope','研究范围')}${button('refresh','刷新')}</div></div>
        <section class="sr3-scope" data-sr3-scope hidden><strong>这些数字指向库内研究材料，不代表市场规模。</strong><p>原声按作品与评论标识去重，账号按平台与账号标识去重，不等于核验后的独立自然人。最近变化按建档、定义版本或原声首次关联时间统计近 28 天；补入历史评论不表示需求增长。</p><p>关联原声预览按首次加入时间选取，不保证代表全部情境；它不是整段定义的语义验证。读取基线约束翻页新增材料，来源可读性在每次读取时重新核对。</p></section>
        <div data-sr3-candidate-controls hidden><label class="sr3-select">处理状态<select data-sr3-select="candidate">${options(candidateStates,this.candidate)}</select></label></div>
        <div data-sr3-list data-density="${this.density}"></div><nav class="sr3-pagination" aria-label="问题翻页" data-sr3-pages></nav>`;
    }
    async load() {
      if(!this.domain){this.find('[data-sr3-list]').innerHTML='<p class="sr3-message">请先在页头选择领域。</p>';this.find('[data-sr3-status]').textContent='尚未选择领域';return;}
      const serial=(this.listSerial||0)+1;this.listSerial=serial;this.items=[];
      this.find('[data-sr3-list]').innerHTML='<div class="sr3-loading" role="status">正在读取研究成果…</div>';
      this.find('[data-sr3-pages]').innerHTML='';
      const clues=this.view==='clues';
      this.find('[data-sr3-search]').hidden=clues;this.find('[data-sr3-select="order"]').closest('label').hidden=clues;
      this.find('[data-sr3-candidate-controls]').hidden=!clues;
      for(const el of this.querySelectorAll('[data-sr3="view"]'))el.setAttribute('aria-pressed',String(el.dataset.value===this.view));
      try {
        const query=new URLSearchParams({limit:'12'});if(this.cursor)query.set('cursor',this.cursor);
        if(clues)query.set('state',this.candidate);else{query.set('q',this.q);query.set('view',this.view);query.set('order',this.order);}
        const data=await this.request(`${clues?'problem-candidates':'problem-cards'}?${query}`,'list');
        if(serial!==this.listSerial)return;
        if(!clues&&(data.contract!=='comment-study.problem-cards.v1'||data.domainRef!==this.domain))throw contractError();
        const rows=clues?data.expressions:data.problems;if(!Array.isArray(rows))throw contractError();
        this.items=rows;this.next=data.page?.nextCursor||null;
        this.find('[data-sr3-status]').textContent=clues?`本页 ${rows.length} 条线索${this.next?' · 还有后续内容':' · 已到末页'} · 未计入问题总数`:`匹配 ${count(data.totalCount)} 个问题 · 本页 ${rows.length} 个${this.view==='changes'?' · 近 28 天的记录变化':''}`;
        this.renderList();
        this.find('[data-sr3-pages]').innerHTML=`${this.cursor?button('first','回到第一页'):''}<span>${clues?'未归并表达按最近处理记录排列':'检索、排序与分页由服务端执行'}</span>${this.next?button('next','下一页 →'):''}`;
      } catch(error) {
        if(error.name==='AbortError'||serial!==this.listSerial)return;
        this.find('[data-sr3-status]').textContent='未取得本次查询结果';
        this.find('[data-sr3-list]').innerHTML=failure(error.message)+(this.cursor?button('first','从第一页重新读取'):'');
      }
    }
    renderList() {
      const target=this.find('[data-sr3-list]');target.dataset.density=this.density;
      if(!this.items.length){
        const label=this.view==='clues'?'当前状态下没有可读取的未归并线索':this.view==='changes'?'近 28 天没有新的研究记录变化':this.q?'没有匹配的问题':'尚无已建档问题，未归并表达仍可查看';
        target.innerHTML=`<div class="sr3-empty"><span class="sr3-empty-mark">[ — ]</span><h3>${label}</h3><p>${this.q?'试试更短的关键词。':'已有材料不会因为缺少新结论而被隐藏，也不会用演示内容补齐。'}</p>${button(this.q?'clear':'view',this.q?'清除搜索':this.view==='clues'?'查看全部问题':'查看未归并线索',`data-value="${this.view==='clues'?'all':'clues'}"`)}</div>`;return;
      }
      if(this.view==='changes'){
        const groups=[['定义记录','首次建档与已有定义的新版本；不等于模型已经验证结论。',this.items.filter(p=>p.changeKind!=='evidence_added')],['仅材料补入','同一问题补入依据，不据此生成新的研究结论。',this.items.filter(p=>p.changeKind==='evidence_added')]];
        target.innerHTML=groups.filter(g=>g[2].length).map(([title,note,rows])=>`<section class="sr3-group"><div class="sr3-group-head"><h3>${title}</h3><span>${note}</span></div><div class="sr3-grid">${rows.map(p=>this.card(p)).join('')}</div></section>`).join('');
      }else target.innerHTML=`<div class="sr3-grid">${this.items.map(p=>this.view==='clues'?this.clueCard(p):this.card(p)).join('')}</div>`;
    }
    card(p) {
      const canDefine=p.definitionReadable===true;const preview=readable(p.preview)?p.preview:null;
      const title=canDefine?text(p.title)||'问题标题未记录':'问题定义当前不可读取';
      const change=kinds[p.changeKind]||'更新状态未记录';
      return `<article class="sr3-card ${p.changeKind==='definition_revision'?'sr3-revised':''}"><div class="sr3-card-top"><span class="sr3-id">${escape(String(p.problemRef).slice(0,8))}</span><span class="sr3-kind">${change}</span><span class="sr3-register" aria-hidden="true">+</span></div>
        <h3 class="sr3-card-title">${escape(title)}</h3><div class="sr3-note"><span>已保存的研究定义</span><p>${escape(canDefine?text(p.definition)||'定义正文未记录':'不从受限材料补写结论。')}</p></div>
        <div class="sr3-quote"><span class="sr3-quote-label">关联原声 <i aria-hidden="true">↘</i></span><blockquote>${preview?escape(preview.commentText):'当前没有可展示的原声预览。'}</blockquote><span class="sr3-quote-caption">${preview?'预览 1 条 · 不作为整段定义的验证':'原声可读性以最新读取为准'}</span></div>
        <div class="sr3-card-bottom"><div class="sr3-metrics">${metric(p.supportCommentCount,'原声')}${metric(p.supportAuthorCount,'账号')}${metric(p.supportWorkCount,'作品')}</div><span class="sr3-open-glyph" aria-hidden="true">↗</span></div>
        <div class="sr3-card-foot"><span>${escape(warning(p)||`近 28 天首次关联 ${count(p.recentAddedSupportCommentCount)} 条原声`)}</span><time>${date(p.updatedAt)}</time></div>
        <button type="button" class="sr3-card-hit" data-sr3="open" data-ref="${escape(p.problemRef)}" aria-label="查看问题：${escape(title)}"><span class="v7-sr-only">查看定义、原声与形成过程</span></button></article>`;
    }
    clueCard(p) {
      const canRead=readable(p),other=['solution','experience'].includes(p.kind);
      return `<article class="sr3-card sr3-clue"><div class="sr3-card-top"><span class="sr3-id">线索</span><span class="sr3-kind">${escape(candidateStates[p.state]||'处理状态未记录')}</span><span aria-hidden="true">+</span></div><h3 class="sr3-card-title">${escape(canRead?text(p.proposition)||'尚未记录归一表达':'来源当前不可读取')}</h3><div class="sr3-note"><span>${escape(typeof signalKindLabel!=='undefined'?signalKindLabel[p.kind]||p.kind:p.kind)}</span><p>${other?'建议或个人经验，不作为已归纳问题。':'有效表达尚未建立长期问题，处理状态不代表用户观点。'}</p></div><div class="sr3-quote"><span class="sr3-quote-label">原始表达</span><blockquote>${escape(canRead?p.commentText:'来源受限，原声及衍生内容不显示。')}</blockquote></div><div class="sr3-card-bottom"><span class="sr3-caption">查看原声与处理依据</span><span class="sr3-open-glyph" aria-hidden="true">↗</span></div><div class="sr3-card-foot"><span>1 条表达 · 不计入问题数量</span><time>${date(p.createdAt)}</time></div><button type="button" class="sr3-card-hit" data-sr3="clue" data-ref="${escape(p.signalRef)}" aria-label="查看这条未归并表达"><span class="v7-sr-only">查看线索和处理依据</span></button></article>`;
    }
    makeDialog() {
      if(this.dialog)return;
      const d=document.createElement('dialog');d.className='sr3-drawer';d.setAttribute('aria-labelledby','sr3-drawer-title');
      d.innerHTML=`<header class="sr3-drawer-bar"><span>用户问题 / 研究详情</span>${button('close','关闭 ×')}</header><div class="sr3-drawer-scroll" data-sr3-detail></div><footer class="sr3-drawer-footer"><span data-sr3-feedback role="status">只读研究结果</span>${button('copy','复制摘要与来源')}</footer>`;
      d.addEventListener('cancel',event=>{event.preventDefault();this.close();});
      d.addEventListener('click',event=>{if(event.target===d){const r=d.getBoundingClientRect();if(event.clientX<r.left||event.clientX>r.right||event.clientY<r.top||event.clientY>r.bottom)this.close();}});
      this.appendChild(d);this.dialog=d;this.dialogBounds();
    }
    dialogBounds() {
      if(!this.dialog)return;
      const header=parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--lgi-header-height'))||0;
      const top=innerWidth>680?Math.max(header,document.querySelector('.study-toolbar')?.getBoundingClientRect().bottom||0):0;
      this.dialog.style.setProperty('--sr3-dialog-top',`${top}px`);
    }
    async openProblem(ref,push=true) {
      this.opener=document.activeElement;this.problemRef=ref;this.detail=null;this.evidence=[];
      this.makeDialog();if(push)this.route();
      this.dialog.querySelector('[data-sr3-detail]').innerHTML='<h2 id="sr3-drawer-title" tabindex="-1">正在读取问题…</h2>';
      this.dialog.querySelector('[data-sr3="copy"]').hidden=false;
      this.dialog.querySelector('[data-sr3="copy"]').disabled=true;
      this.dialog.querySelector('[data-sr3-feedback]').textContent='正在核对当前定义与原声…';
      if(!this.dialog.open)this.dialog.showModal();
      this.dialog.querySelector('[data-sr3="close"]').focus();
      await this.loadProblem(ref);
    }
    async loadProblem(ref) {
      const serial=(this.detailSerial||0)+1;this.detailSerial=serial;
      this.detail=null;this.evidence=[];this.evidenceNext=null;
      const detailPath=`problems/${encodeURIComponent(ref)}`;
      const [detail,evidence]=await Promise.allSettled([this.request(detailPath,'detail'),this.request(detailPath+'/evidence?limit=20','evidence')]);
      if(serial!==this.detailSerial||!this.dialog?.open||this.problemRef!==ref)return;
      if(detail.status!=='fulfilled'||detail.value?.problem?.problemRef!==ref){
        this.dialog.querySelector('[data-sr3-detail]').innerHTML='<h2 id="sr3-drawer-title">问题暂时不可读取</h2>'+failure(detail.status==='rejected'?detail.reason.message:'问题回执不一致','retry-detail');return;
      }
      this.detail=detail.value;
      if(this.detail.problem.definitionReadable!==true){
        const card=this.items.find(p=>p.problemRef===ref);
        if(card){Object.assign(card,{...this.detail.problem,preview:null});this.renderList();}
      }
      const ok=evidence.status==='fulfilled'&&evidence.value?.problemRef===ref&&Array.isArray(evidence.value.evidence)&&evidence.value.evidence.every(r=>text(r.commentKey?.workRef)&&text(r.commentKey?.commentExternalId));
      this.evidence=ok?unique(evidence.value.evidence):[];this.evidenceNext=ok?evidence.value.page?.nextCursor:null;
      this.evidenceError=ok?'':'原声列表暂时不可读取；不代表没有依据。';
      this.renderDetail();
    }
    renderDetail() {
      const p=this.detail.problem,can=p.definitionReadable===true,identity=p.stableIdentity||{};
      const title=can?text(p.title)||'标题未记录':'问题定义当前不可读取';
      this.dialog.querySelector('[data-sr3-detail]').innerHTML=`<section class="sr3-detail-intro"><span class="sr3-eyebrow">${escape(p.problemRef)} / 问题研究</span><h2 id="sr3-drawer-title" tabindex="-1">${escape(title)}</h2><p class="sr3-definition">${escape(can?text(p.definition)||'定义未记录':'当前定义依赖的原声不可读取，不显示衍生结论。')}</p><div class="sr3-metrics">${metric(p.supportCommentCount,'关联原声')}${metric(p.supportAuthorCount,'评论账号')}${metric(p.supportWorkCount,'来源作品')}</div>${warning(p)?`<p class="sr3-caution">${escape(warning(p))}</p>`:''}</section>
        <nav class="sr3-jumps" aria-label="问题详情章节">${button('jump','问题理解','data-section="identity"')}${button('jump','原声依据','data-section="evidence"')}${button('jump','形成过程','data-section="history"')}</nav>
        <section class="sr3-detail-section" id="sr3-identity"><div class="sr3-section-head"><h3>问题具体是什么</h3><span>研究概括，不是用户原话</span></div>${can?`<dl class="sr3-identity">${[['actor','谁遇到'],['context','什么情境'],['goalOrExpectedState','想达成'],['barrierOrUnmetNeed','卡在哪里']].map(([key,label])=>`<div><dt>${label}</dt><dd>${escape(text(identity[key])||'当前未记录')}</dd></div>`).join('')}</dl>`:'<p>来源恢复可读后，再查看具体定义。</p>'}</section>
        <section class="sr3-detail-section" id="sr3-evidence"><div class="sr3-section-head"><h3>原声与关联表达</h3><span>已读取 <b data-sr3-evidence-count>${this.evidence.length}</b> 条</span></div><p class="sr3-caption">一条评论只展示一次。关联关系不等于对整段定义的独立验证；需要结合父评论与作品核对。</p><div data-sr3-evidence-list>${this.evidenceError?failure(this.evidenceError,'retry-detail'):this.evidenceHTML()}</div><div data-sr3-evidence-pager>${this.evidenceNext?button('more-evidence','继续读取原声'):''}</div></section>
        <section class="sr3-detail-section" id="sr3-history"><div class="sr3-section-head"><h3>定义与形成过程</h3><span>只呈现已保存的记录</span></div><p class="sr3-caption">版本新增不自动意味着获得新洞察。以下原文按保存版本排列，不由前端生成差异结论。</p>${(this.detail.revisionHistory||[]).map(r=>`<details class="sr3-revision"><summary>定义版本 ${count(r.identityVersion)} · ${date(r.createdAt)}${r.definitionReadable===true?'':' · 依据不可读取'}</summary>${r.definitionReadable===true?`<h4>${escape(r.title)}</h4><p>${escape(r.definition)}</p>`:'<p>这一版本的来源当前不可读取，历史文字不显示。</p>'}</details>`).join('')||'<p>历史版本尚未记录。</p>'}
        ${can?`<details class="sr3-revision"><summary>当前纳入与排除边界</summary><h4>纳入</h4>${this.criteria(p.includeCriteria)}<h4>排除</h4>${this.criteria(p.excludeCriteria)}</details>`:''}
        <details class="sr3-revision"><summary>来源分布</summary>${this.distribution()}</details>
        <p class="sr3-caption">内容对照尚未接入本页。不据此判断我方没讲过、市场没有回答，也不自动生成运营建议。</p></section>`;
      this.dialog.querySelector('[data-sr3="copy"]').disabled=!can;
      this.dialog.querySelector('[data-sr3-feedback]').textContent='复制只包含可读定义与来源定位，不批量导出原声。';
    }
    criteria(rows){return Array.isArray(rows)&&rows.length?`<ul>${rows.map(r=>`<li>${escape(typeof r==='string'?r:JSON.stringify(r))}</li>`).join('')}</ul>`:'<p>尚未记录具体条件。</p>';}
    distribution(){const d=this.detail.sourceDistribution;if(!d)return '<p>来源分布暂不可读取。</p>';return (d.works||[]).map(w=>`<div class="sr3-source-row"><span>${escape(w.workTitle||'作品标题未记录')}</span><b>${count(w.commentCount)} 条</b></div>`).join('')||'<p>暂无可读来源。</p>';}
    evidenceHTML(){return this.evidence.length?this.evidence.map((row,i)=>`<article class="sr3-evidence" data-sr3-evidence-row="${i}"><header><span class="sr3-id">${String(i+1).padStart(2,'0')}</span><strong>${escape(readable(row)?row.authorDisplayName||'账号名称未记录':'来源状态未确认')}</strong><span>${date(row.addedAt)} 加入依据</span></header>${readable(row)?`<blockquote>${escape(row.commentText)}</blockquote>${(row.signals||[]).length?`<details class="sr3-related"><summary>查看与这条原声关联的研究表达</summary>${row.signals.map(s=>`<p><strong>${escape(typeof signalKindLabel!=='undefined'?signalKindLabel[s.kind]||s.kind:s.kind)}</strong> · ${escape(s.proposition||'表达未记录')}</p>`).join('')}</details>`:''}`:'<p>原声当前不可读取，关联研究文字不显示。</p>'}<div class="sr3-evidence-bottom"><span>作品 ${escape(String(row.commentKey?.workRef||row.workRef||'未记录').slice(0,12))}</span>${row.commentKey?.workRef&&row.commentKey?.commentExternalId?button('context','展开作品与父语境',`data-index="${i}" aria-expanded="false"`):''}</div><div class="sr3-context" data-sr3-context="${i}" hidden></div></article>`).join(''):'<p class="sr3-message">当前没有可展示的原声，不代表问题已经解决。</p>';}
    async moreEvidence() {
      if(!this.evidenceNext||!this.detail)return;
      const ref=this.problemRef,serial=this.detailSerial,trigger=this.dialog.querySelector('[data-sr3="more-evidence"]');trigger.disabled=true;
      try {
        const data=await this.request(`problems/${encodeURIComponent(ref)}/evidence?${new URLSearchParams({limit:'20',cursor:this.evidenceNext})}`,'more-evidence');
        if(serial!==this.detailSerial||!this.dialog.open)return;
        if(data.problemRef!==ref||!Array.isArray(data.evidence))throw contractError();
        this.evidence=unique([...this.evidence,...data.evidence]);this.evidenceNext=data.page?.nextCursor||null;
        this.dialog.querySelector('[data-sr3-evidence-list]').innerHTML=this.evidenceHTML();
        this.dialog.querySelector('[data-sr3-evidence-count]').textContent=this.evidence.length;
        this.dialog.querySelector('[data-sr3-evidence-pager]').innerHTML=this.evidenceNext?button('more-evidence','继续读取原声'):'';
        this.dialog.querySelector('[data-sr3-feedback]').textContent=`已读取 ${this.evidence.length} 条原声。`;
      }catch(e){if(e.name!=='AbortError'&&serial===this.detailSerial){trigger.disabled=false;trigger.textContent='读取失败，点击重试';}}
    }
    async context(index,trigger) {
      const row=this.evidence[index];if(!row?.commentKey)return;
      const target=this.dialog.querySelector(`[data-sr3-context="${index}"]`);
      if(!target.hidden){target.hidden=true;target.replaceChildren();trigger.setAttribute('aria-expanded','false');return;}
      target.hidden=false;target.textContent='正在核对原声与语境…';trigger.setAttribute('aria-expanded','true');
      const serial=this.detailSerial;
      try {
        const data=await this.request(`comments/detail?${new URLSearchParams(row.commentKey)}`,`context-${index}`);
        if(serial!==this.detailSerial||!target.isConnected||target.hidden)return;
        if(keyOf(data)!==keyOf(row))throw contractError();
        if(data.source?.sourceState!=='known'){
          target.textContent='原声当前不可读取，正在重新核对问题及引用。';
          if(this.problemRef){await this.loadProblem(this.problemRef);void this.load();}else {this.detail=null;this.evidence=[];this.dialog.querySelector('[data-sr3-detail]').innerHTML='<h2 id="sr3-drawer-title">来源当前不可读取</h2><p class="sr3-message">原声、归一表达及比较说明已隐藏。</p>'; }return;
        }
        const parent=data.parentContext;
        target.innerHTML=`<span class="sr3-caption">所属作品</span><h4>${escape(data.work?.displayTitle||data.comment?.workTitle||'作品标题未记录')}</h4><p class="sr3-caption">此处只取得作品标题，不把标题推断成作品完整观点。</p><span class="sr3-caption">父评论语境</span>${text(parent?.commentText)?`<blockquote>${escape(parent.commentText)}</blockquote>`:'<p>没有可显示的父评论语境；不推断为主评论。</p>'}<p class="sr3-caption">父评论只用于理解这条表达，不计为它的独立支持依据。</p>`;
      }catch(e){if(e.name!=='AbortError'&&serial===this.detailSerial&&target.isConnected){target.textContent=`${e.message} 收起后可重试。`;}}
    }
    async openClue(ref) {
      const row=this.items.find(p=>p.signalRef===ref);if(!row)return;
      this.opener=document.activeElement;this.problemRef=null;this.detail=null;this.makeDialog();
      const serial=(this.detailSerial||0)+1;this.detailSerial=serial;
      this.evidence=[];this.dialog.querySelector('[data-sr3="copy"]').hidden=true;
      this.dialog.querySelector('[data-sr3-detail]').innerHTML='<h2 id="sr3-drawer-title">正在核对线索来源…</h2>';
      if(!this.dialog.open)this.dialog.showModal();this.dialog.querySelector('[data-sr3="close"]').focus();
      try {
        const fresh=await this.request(`comments/detail?${new URLSearchParams(row.commentKey)}`,'clue');
        if(serial!==this.detailSerial||!this.dialog.open)return;
        if(keyOf(fresh)!==keyOf(row))throw contractError();
        const can=fresh.source?.sourceState==='known'&&!!text(fresh.comment?.commentText);
        const unchanged=can&&fresh.comment.commentText===row.commentText;
        this.evidence=[{...row,commentText:can?fresh.comment.commentText:null,sourceState:can?'known':'restricted',signals:[]}];
        this.dialog.querySelector('[data-sr3-detail]').innerHTML=`<section class="sr3-detail-intro"><span class="sr3-eyebrow">未归并线索 / 不作为已建档问题</span><h2 id="sr3-drawer-title">${escape(unchanged?row.proposition||'归一表达未记录':can?'原声已变化，旧研究表达不再显示':'来源当前不可读取')}</h2><p>${escape(candidateStates[row.state]||'处理状态未记录')}</p></section><section class="sr3-detail-section">${this.evidenceHTML()}<h3>当前候选比较记录</h3>${unchanged&&(row.pairOutcomes||[]).length?row.pairOutcomes.map(p=>`<p>${escape(typeof pairStateLabel!=='undefined'?pairStateLabel[p.state]||p.state:p.state)} · ${escape(typeof pairDecisionLabel!=='undefined'?pairDecisionLabel[p.decisionReason]||p.decisionReason||'原因未记录':p.decisionReason||'原因未记录')}</p>`).join(''):'<p>没有当前可读取的比较结论，不能据此判断是否已有独立依据。</p>'}${row.runRef?button('run','查看来源研究批次',`data-ref="${escape(row.runRef)}"`):''}</section>`;
        this.dialog.querySelector('[data-sr3-feedback]').textContent='线索可阅读，不需要人工确认才能展示。';
      }catch(e){if(e.name!=='AbortError'&&serial===this.detailSerial)this.dialog.querySelector('[data-sr3-detail]').innerHTML='<h2 id="sr3-drawer-title">线索暂时不可读取</h2>'+failure(e.message,'close');}
    }
    close() {
      ++this.detailSerial;this.requests.forEach((c,k)=>{if(k!=='list')c.abort();});
      this.dialog?.close();this.dialog?.querySelector('[data-sr3-detail]').replaceChildren();this.detail=null;this.evidence=[];
      if(this.problemRef){this.problemRef=null;this.route(false);}
      if(this.opener?.isConnected)this.opener.focus({preventScroll:true});else this.querySelector('[data-sr3="open"]')?.focus({preventScroll:true});
    }
    async copy() {
      const ref=this.problemRef,serial=this.detailSerial;if(!this.detail||!ref)return;
      const action=this.dialog.querySelector('[data-sr3="copy"]'),feedback=this.dialog.querySelector('[data-sr3-feedback]');
      action.disabled=true;feedback.textContent='正在重新核对定义可读性…';
      try {
        const fresh=await this.request(`problems/${encodeURIComponent(ref)}`,'copy');
        if(serial!==this.detailSerial||!this.dialog.open)return;
        if(fresh.problem?.problemRef!==ref)throw contractError();
        if(fresh.problem.definitionReadable!==true){await this.loadProblem(ref);return;}
        const p=fresh.problem,url=new URL(location.href);url.search='';url.hash='';url.searchParams.set('domain',this.domain);url.searchParams.set('view','problems');url.searchParams.set('problemRef',ref);
        const content=`用户问题：${text(p.title)||'标题未记录'}\n研究定义（非用户原话）：${text(p.definition)||'未记录'}\n\n关联原声 ${count(p.supportCommentCount)} 条，账号 ${count(p.supportAuthorCount)} 个，作品 ${count(p.supportWorkCount)} 篇。\n范围限制：${warning(p)||'库内材料，不代表市场规模；关联不等于整段定义已验证。'}\n\n来源定位：${url}\n定义版本：${p.revisionRef||'未记录'}\n复制时已重新核对可读性；后续仍以来源权限为准。`;
        try {if(!navigator.clipboard?.writeText)throw new Error('Clipboard unavailable');await navigator.clipboard.writeText(content);if(serial===this.detailSerial)feedback.textContent='已复制可读定义与来源定位。';}
        catch {if(serial!==this.detailSerial)return;const area=document.createElement('textarea');area.className='sr3-copy-text';area.readOnly=true;area.setAttribute('aria-label','可手动复制的问题摘要与来源');area.value=content;this.dialog.querySelector('[data-sr3-detail]').appendChild(area);area.focus();area.select();feedback.textContent='未自动复制，请复制已选中的文本。';}
      } catch(e){if(e.name!=='AbortError'&&serial===this.detailSerial)feedback.textContent=`复制未完成：${e.message}`;}
      finally{if(serial===this.detailSerial&&this.dialog.open)action.disabled=this.detail?.problem?.definitionReadable!==true;}
    }
    click(event) {
      const el=event.target.closest('[data-sr3]');if(!el||!this.contains(el))return;
      // A dialog is a child of this component; its click bubbles here exactly once.
      const action=el.dataset.sr3;
      if(action==='view'){this.close();this.view=el.dataset.value;this.cursor=null;this.q='';this.route();void this.load();}
      else if(action==='density'){this.density=el.dataset.value;this.route(false);this.renderList();for(const b of this.querySelectorAll('[data-sr3="density"]'))b.setAttribute('aria-pressed',String(b.dataset.value===this.density));}
      else if(action==='scope'){const s=this.find('[data-sr3-scope]');s.hidden=!s.hidden;el.setAttribute('aria-expanded',String(!s.hidden));}
      else if(['retry','refresh','first','clear'].includes(action)){if(action!=='retry')this.cursor=null;if(action==='clear'){this.q='';this.find('#sr3-query').value='';}this.route(false);void this.load();}
      else if(action==='next'){this.cursor=this.next;this.route();void this.load();this.scrollIntoView({block:'start'});}
      else if(action==='open')void this.openProblem(el.dataset.ref);
      else if(action==='clue')void this.openClue(el.dataset.ref);
      else if(action==='close')this.close();
      else if(action==='retry-detail')void this.loadProblem(this.problemRef);
      else if(action==='more-evidence')void this.moreEvidence();
      else if(action==='context')void this.context(Number(el.dataset.index),el);
      else if(action==='jump')this.dialog.querySelector(`#sr3-${el.dataset.section}`)?.scrollIntoView({block:'start',behavior:matchMedia('(prefers-reduced-motion: reduce)').matches?'auto':'smooth'});
      else if(action==='copy')void this.copy();
      else if(action==='run'){this.close();if(typeof navigateToRun==='function')void navigateToRun(el.dataset.ref,'signals');}
    }
  }

  if(typeof module!=='undefined'&&module.exports)module.exports={escape,count,date,readable,keyOf,unique,warning,StudyUserProblems};
  if(typeof customElements==='undefined'||typeof TAB_RENDERERS==='undefined')return;
  if(customElements.get('study-user-problems'))return;
  customElements.define('study-user-problems',StudyUserProblems);
  // Preserve the destination of the unchanged overview's existing pending-expression links.
  document.querySelector('#study-tab-result')?.addEventListener('click',event=>{
    if(activeView!=='overview')return;
    const pending=event.target.closest('[data-overview-pending-state]');
    const entry=event.target.closest('[data-study-view="problems"]');
    if(pending||entry)pendingEntry={domain:domainRef,view:pending||entry?.closest('[aria-labelledby="study-unknown-title"]')?'clues':'all',candidate:pending?.dataset.overviewPendingState||'all'};
  },true);
  window.addEventListener('popstate',()=>{
    const params=new URLSearchParams(location.search);
    incomingRoute=['problems','pending'].includes(params.get('view'))?params:null;
  },true);
  const comments=TAB_RENDERERS.comments,runs=TAB_RENDERERS.runs;
  TAB_RENDERERS.comments=async()=>`<div class="study-results-comments">${await comments()}</div>`;
  TAB_RENDERERS.runs=async()=>`<div class="study-results-runs">${await runs()}</div>`;
  TAB_RENDERERS.problems=async()=>'<study-user-problems></study-user-problems>';
  // Replace only an initial legacy problems render. For comments/runs, move the existing
  // nodes into a scoped wrapper after their original render: no duplicate reads or bindings.
  if(typeof activeView!=='undefined'&&activeView==='problems')void renderActiveTab();
  else if(typeof activeView!=='undefined'&&['comments','runs'].includes(activeView)){
    const root=document.querySelector('#study-tab-result'),view=activeView;
    if(root){
      const wrap=()=>{
        if(activeView!==view){observer.disconnect();return;}
        if(root.getAttribute('aria-busy')==='true')return;
        observer.disconnect();
        if(root.querySelector(`.study-results-${view}`))return;
        const wrapper=document.createElement('div');wrapper.className=`study-results-${view}`;
        wrapper.append(...root.childNodes);root.append(wrapper);
      };
      const observer=new MutationObserver(wrap);observer.observe(root,{childList:true,attributes:true,attributeFilter:['aria-busy']});wrap();
    }
  }
})();
