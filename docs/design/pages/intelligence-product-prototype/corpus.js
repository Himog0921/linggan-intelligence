// Synthetic research surfaces share the same demo works and problem references.
const corpusDemo = {
  mode:'expressions',
  creators:[
    {id:'ca',name:'样本作者 A',works:8,weeks:[2,1,3,2],topics:'晚间流程、任务启动',last:'09.04',coverage:'窗口仅部分观察',watched:true,work:'w1'},
    {id:'cb',name:'样本作者 B',works:3,weeks:[1,0,1,1],topics:'奖励工具、过程记录',last:'09.02',coverage:'来自关键词搜索',watched:false,work:'w2'},
    {id:'cc',name:'样本作者 C',works:1,weeks:[0,0,0,1],topics:'家长提醒',last:'09.05',coverage:'来自单篇详情',watched:false,work:'w3'}
  ],
  saved:[]
};
try {const saved=JSON.parse(localStorage.getItem('linggan-design-queries-20260906')||'[]');corpusDemo.saved=Array.isArray(saved)?saved.filter(q=>q&&typeof q.name==='string'&&['evidence','comments','creators'].includes(q.surface)&&typeof q.query==='string'):[];}catch{}
function corpusScopeLinks(){return `<div class="toolbar">${link('/corpus/evidence','本领域','plain-link')}${link('/corpus/cross-industry','跨行业样本','plain-link')}${link('/corpus/notes','外部样本随手记','plain-link')}<span class="meta">来源范围与笔记工具</span></div>`;}
function corpusPage(tab){
  if(tab==='comments')return commentResearch();
  if(tab==='creators')return creatorResearch();
  if(tab==='queries')return savedQueries();
  if(tab==='cross-industry'||tab==='notes')return corpusScopeLinks()+corpusLegacy(tab);
  return corpusLegacy('evidence').replace('本领域材料','证据库')+`<section class="section"><h2>从材料继续研究</h2><div class="row-actions">${link('/corpus/comments','研究已取得的评论 →','plain-link')}${link('/corpus/creators','发现领域创作者 →','plain-link')}</div></section>`;
}
function commentResearch(){
  const scopeId=new URLSearchParams(location.hash.split('?')[1]||'').get('work');
  const problemWorks={time:'w1',repeat:'w5',keep:'w2'};
  const items=DEMO.problems.filter(p=>!scopeId||problemWorks[p.id]===scopeId);
  const choices=[['expressions','表达浏览'],['groups','问题分组']];
  return heading('评论研究','已取得的评论在这里持续汇集，先读原声，再组织问题。',button('c-save-query','保存当前查询'))+`<div class="toolbar">${searchBox('comment-search','搜索表达、场景或来源作品')}<span class="grow"></span>${choices.map(([key,name])=>button('c-mode',name,corpusDemo.mode===key?'primary':'',`data-id="${key}"`)).join('')}${button('matrix','问题与回答')}</div><p class="meta">合成演示：${scopeId?'限定作品 '+esc(scopeId)+'；':''}可读评论即时进入；新增归组进行中时，原声仍可阅读。${scopeId?link('/corpus/comments','查看全部已取得评论','plain-link'):''}</p>`+(corpusDemo.mode==='groups'?questionsTable(items):`<div class="row-list">${items.map(p=>{const w=DEMO.works.find(w=>w.id===problemWorks[p.id]);return `<article class="filter-row" data-search="${esc(p.quote+' '+p.name+' '+w.title)}"><div class="row-head"><div><h3>${esc(p.quote)}</h3><p class="meta">匿名合成表达 · 评论时间未知 · 取得于 09.06 · 来源：${w.title}</p></div>${badge('已取得','ok')}</div><p>初步问题：${p.name}</p><div class="row-actions">${button('problem','看上下文与问题','',`data-id="${p.id}"`)}${button('work','阅读来源作品','quiet',`data-id="${w.id}"`)}${button('correct','纠正归类','quiet')}</div></article>`;}).join('')||empty('当前范围没有可读表达','保留作品范围，新增评论取得后在这里出现。')}</div>`)+`<div class="note">分组引用同一批评论材料；表达数、涉及作品数和作品作者数分别计算。${link('/insights/questions','查看领域中的用户问题 →','plain-link')}</div>`;
}
function creatorResearch(){return heading('创作者','从全部已取得作品发现谁在发布 ADHD 内容，再决定关注谁。',button('c-save-query','保存当前查询'))+`<div class="toolbar">${searchBox('creator-search','搜索创作者或细分主题')}<span class="grow"></span>${badge('近 4 周 · 已观察相关作品')}</div><div class="table-wrap"><table><thead><tr><th>创作者</th><th class="num">相关作品</th><th>各周观察到的发表数</th><th>主要细分主题</th><th>最近已观察发表</th><th>覆盖情况</th><th>持续观察</th></tr></thead><tbody>${corpusDemo.creators.map(c=>`<tr class="filter-row" data-search="${esc(c.name+' '+c.topics)}"><td>${button('c-creator',c.name,'quiet',`data-id="${c.id}"`)}<span class="meta">小红书 · 合成身份</span></td><td class="num">${c.works}</td><td class="num">${c.weeks.join(' / ')}</td><td>${c.topics}</td><td>${c.last}</td><td>${c.coverage}</td><td>${badge(c.watched?'持续观察中':'未设持续观察',c.watched?'ok':'')}</td></tr>`).join('')}</tbody></table></div><div class="note">这些是样本发布分布。未完整观察作者主页时，不用采回数量推断真实更新频率，也不显示 ADHD 内容占比。</div>`;}
function creatorDetail(id){const c=corpusDemo.creators.find(c=>c.id===id);if(!c)return;showDialog(c.name,`<p>${c.topics} · ${c.coverage}</p><div class="tabs">${button('c-creator-tab','概览','quiet',`data-id="${id}" data-tab="summary"`)}${button('c-creator-tab','作品与主题','quiet',`data-id="${id}" data-tab="works"`)}${button('c-creator-tab','发布节奏','quiet',`data-id="${id}" data-tab="rhythm"`)}${button('c-creator-tab','讨论与表现','quiet',`data-id="${id}" data-tab="discussion"`)}</div><div id="creator-content" class="section">${creatorTab(c,'summary')}</div><div class="drawer-footer">${button('c-observe',c.watched?'查看持续观察':'加入持续观察','primary',`data-id="${c.id}"`)}</div>`);}
function creatorTab(c,tab){if(tab==='works')return workCards(DEMO.works.filter(w=>w.id===c.work));if(tab==='rhythm')return `<h3>各周已观察到的发表分布</h3>${metrics(c.weeks.map((v,i)=>['第 '+(i+1)+' 周',v]))}<p>固定四周窗口共观察到 ${c.works} 篇。覆盖为“${c.coverage}”，因此真实发布频率未知。</p><p class="meta">日期缺失的作品另列；置顶作品按原发布时间归位；不按首次采回日期画发布曲线。</p>`;if(tab==='discussion')return `<h3>从这位作者的作品进入讨论</h3><p>查看具体问题发生在哪篇作品，以及作者是否作出回应。评论者的问题不代表作者立场。</p>${link('/corpus/comments?work='+c.work,'研究这些作品的评论 →','plain-link')}`;return metrics([['已观察相关作品',c.works],['出现相关作品的周数',c.weeks.filter(v=>v>0).length],['样本平均每周',(c.works/4).toFixed(1)+' 篇']])+`<p>已确认发布过相关作品，可以进入领域作者目录；无需事先加入采集目标。</p><p class="meta">总数与代表作品均为合成演示，代表作品并非全量明细。</p>`;}
function savedQueries(){return heading('已存查询','保存问题的查找条件，下次在最新可用材料上继续看。')+`<div class="row-list">${corpusDemo.saved.map((q,i)=>`<article><div class="row-head"><div><h3>${esc(q.name)}</h3><p>${{evidence:'证据库',comments:'评论研究',creators:'创作者'}[q.surface]} · ${esc(q.query||'当前全部材料')}${q.work?' · 作品 '+esc(q.work):''}</p><p class="meta">保存条件，不冻结结果；未开启自动采集或通知。</p></div><div>${button('c-open-query','重新执行','primary',`data-id="${i}"`)}${button('c-delete-query','删除查询','quiet',`data-id="${i}"`)}</div></div></article>`).join('')||empty('还没有保存查询','在证据库、评论研究或创作者页筛选后，保存当前查询。',link('/corpus/comments','从评论研究开始 →','plain-link'))}</div>`;}
document.addEventListener('click',event=>{
  const b=event.target.closest('[data-action]');if(!b)return;const action=b.dataset.action,id=b.dataset.id;
  if(action!=='saved-query'&&!action.startsWith('c-'))return;
  event.stopImmediatePropagation();
  if(action==='c-mode'){corpusDemo.mode=id;render();}
  if(action==='c-creator')creatorDetail(id);
  if(action==='c-creator-tab'){const c=corpusDemo.creators.find(c=>c.id===id);document.getElementById('creator-content').innerHTML=creatorTab(c,b.dataset.tab);}
  if(action==='c-observe'){closeDialog();go('/collection/targets');notice('已转到观察目标；原型不创建真实目标或执行采集。');}
  if(action==='c-save-query'||action==='saved-query'){
    const surface=['comments','creators'].includes(route().split('/')[2])?route().split('/')[2]:'evidence',field={evidence:'material-search',comments:'comment-search',creators:'creator-search'}[surface],query=state.queries[field]||'',work=new URLSearchParams(location.hash.split('?')[1]||'').get('work');
    showDialog('保存当前查询',`<form data-form="corpus-query" data-surface="${surface}" data-query="${esc(query)}" data-work="${esc(work||'')}"><label class="form-field"><span>查询名称</span><input name="name" required value="${esc(query||{evidence:'证据库材料',comments:'已取得评论',creators:'领域创作者'}[surface])}"></label><p>搜索：${esc(query||'全部')}；${work?'限定作品 '+esc(work):'当前范围'}。</p><button type="submit" class="primary">保存查询条件</button></form>`);
  }
  if(action==='c-open-query'){const q=corpusDemo.saved[Number(id)],field={evidence:'material-search',comments:'comment-search',creators:'creator-search'}[q.surface];state.queries[field]=q.query;go('/corpus/'+q.surface+(q.work?'?work='+encodeURIComponent(q.work):''));}
  if(action==='c-delete-query'){corpusDemo.saved.splice(Number(id),1);localStorage.setItem('linggan-design-queries-20260906',JSON.stringify(corpusDemo.saved));render();}
});
document.addEventListener('submit',event=>{const form=event.target;if(form.dataset.form!=='corpus-query')return;event.preventDefault();event.stopImmediatePropagation();corpusDemo.saved.push({name:String(new FormData(form).get('name')),surface:form.dataset.surface,query:form.dataset.query,work:form.dataset.work});try{localStorage.setItem('linggan-design-queries-20260906',JSON.stringify(corpusDemo.saved));}catch{}closeDialog();go('/corpus/queries');});

function corpusWorkComment(id){const key={w1:'time',w2:'keep',w5:'repeat'}[id],p=DEMO.problems.find(p=>p.id===key);return p?`<h3>合成评论上下文</h3><blockquote>${esc(p.quote)}</blockquote><p>与评论研究引用同一合成材料；上下文及分析状态分别显示。</p>${button('problem','查看相关问题','',`data-id="${p.id}"`)}`:'<p>评论尚未取得。当前材料不足以显示评论上下文。</p>';}
