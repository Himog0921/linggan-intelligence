(() => {
'use strict';
// This page only projects the existing domain-scoped API. No demo data or client-side classification.
const $ = id => document.getElementById(id);
const form = $('filters'), filterForm = $('filter-form'), drawer = $('drawer'), main = document.querySelector('.creators-main');
const labels = {personal_experience:'亲历自述',professional_output:'知识与方法讲解',explicit_promotion:'推广表达',institution_or_brand:'机构／品牌自述',vertical_tendency:'本领域持续创作',multi_topic:'多主题分享',unknown:'尚不能判定',related:'已判相关',unrelated:'已判无关',yes:'有依据',no:'未见',monitoring:'巡查中',paused:'已暂停',dismissed:'已忽略',not_enabled:'已纳入未开启'};
const observationLabels = {outside:'未加入本领域',inside:'已加入本领域',monitoring:'巡查中',paused:'已暂停',other_domains_only:'仅其他领域有目标',dismissed:'已忽略'};
const esc = value => String(value ?? '').replace(/[&<>"']/g,c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const count = value => value == null || !Number.isFinite(Number(value)) ? '—' : Number(value).toLocaleString('zh-CN');
const defaults = {usageRole:'primary',platform:'xhs',searchMode:'author',sort:'recent'};
const filterKeys = ['usageRole','platform','publishedFrom','publishedTo','traits','topicHint','observation','minLikes','relevance','focus','viral','highLikes'];
const extraKeys = ['query','traits','topicHint','observation','minLikes','relevance','focus','viral','highLikes'];
const queryKeys = ['searchMode','sort','query',...filterKeys];
const domain = form.elements.domain.value, shellDomain = new URLSearchParams(location.search).get('domain') || '';
let query = {...defaults}, result = null, page = 0, sequence = 0, drawerSequence = 0, controller = null;
let selected = null, selectedIndex = -1, tab = 'overview', drawerTrigger = null, navigating = false, composing = false, timer;
let policyContext = null, correction = null, filterDraft = null, historyTimer, pendingReturn = null, suppressFocus = false;
const pendingWrites = new Set(), resources = new Map(), tabPositions = {};
const scope = () => ({domain,...query,page,pageSize:Number($('page-size').value)});
const params = value => {const p=new URLSearchParams();Object.entries(value).forEach(([k,v])=>{if(v!==undefined&&v!==null&&v!==''&&v!==false)p.set(k,String(v));});return p;};
const setStatus = (message='',error=false) => {$('status').textContent=message;$('status').dataset.error=String(error);};
function humanError(code) {
 const known={policy_revision_conflict:'设置已被更新，请重新打开后再保存。',active_primary_domain_conflict:'其他领域已有主研究关系，请查看原观察目标或选择参照用途。',creator_resource_not_found:'材料或创作者已不可读取，请刷新。',creator_read_unavailable:'读取暂不可用，请重试。',creator_analysis_invalid_output:'模型结果未通过校验。',model_budget_exhausted:'本日额度已用尽。',model_disabled:'分析已关闭。',model_secret_unavailable:'模型凭据不可用。',model_not_qualified:'模型当前不可执行。',creator_source_changed:'材料已变化，请刷新后重试。',override_support_required:'请选择当前支持材料。'};
 return known[code] || '请求未完成，请重试；详细原因可在处理信息中查看。';
}
async function request(url,options={}) {
 const response=await fetch(url,options);let body;
 try{body=await response.json();}catch{throw new Error('invalid_response');}
 if(!response.ok)throw new Error(body?.error?.code || body?.error || body?.code || 'request_failed');
 return body;
}
const mutate = (url,method,body) => request(url,{method,headers:{'Content-Type':'application/json'},body:JSON.stringify(body)});
function syncToolbar(){form.elements.searchMode.value=query.searchMode;form.elements.sort.value=query.sort;form.elements.query.value=query.query||'';$('creator-query').placeholder=query.searchMode==='work'?'标题、正文、图文识别或内容方向':'昵称、平台账号、公开简介';$('search-clear').hidden=!query.query;$('search-shortcut').hidden=!!query.query;}
function restoreLocation(){
 const p=new URLSearchParams(location.search);query={...defaults};
 for(const key of queryKeys){const v=p.get(key);if(v!==null&&v!=='')query[key]=['viral','highLikes'].includes(key)?v==='true':key==='minLikes'?Number(v):v;}
 page=/^\d+$/.test(p.get('page')||'')?Math.min(Number(p.get('page')),1000000):0;
 $('page-size').value=['25','50','100'].includes(p.get('pageSize'))?p.get('pageSize'):'50';syncToolbar();
}
function readReturn(){try{const value=JSON.parse(sessionStorage.getItem('linggan.creatorDiscovery.return')||'null');if(value?.url===location.pathname+location.search)pendingReturn=value;sessionStorage.removeItem('linggan.creatorDiscovery.return');}catch{pendingReturn=null;}}
function saveReturn(key){
 const saved={url:location.pathname+location.search,scrollY:$('creator-results').scrollTop,scrollX:$('creator-results').scrollLeft,creatorKey:key||selected?.creatorKey};
 if(drawer.open&&selected){Object.assign(saved,{drawer:true,tab,drawerScroll:$('drawer-body').scrollTop,expanded:[...$('drawer-evidence').querySelectorAll('[data-work][open]')].map(el=>el.dataset.work)});}
 try{sessionStorage.setItem('linggan.creatorDiscovery.return',JSON.stringify(saved));}catch{/* Navigation remains available when storage is disabled. */}
}
function focusAuthor(key){const button=[...$('rows').querySelectorAll('.creator-name')].find(el=>el.dataset.key===key);button?.focus({preventScroll:true});}
function closeTransient(){suppressFocus=true;for(const id of ['filter-dialog','scope-dialog','policy-dialog','correction-dialog','drawer']){if($(id).open)$(id).close();}policyContext=null;correction=null;selected=null;selectedIndex=-1;drawerSequence++;suppressFocus=false;}
function pageControls(){
 const pages=result?Math.ceil(result.total/Number($('page-size').value)):0;
 document.querySelector('[data-page-summary]').textContent=result?(pages?`${page+1} / ${pages} 页`:'0 页'):'';
 document.querySelector('[data-page-prev]').disabled=!pages||page===0;
 document.querySelector('[data-page-next]').disabled=!pages||page+1>=pages;
 const input=document.querySelector('[data-page-input]');input.disabled=!pages;input.max=String(pages);input.value=pages?String(page+1):'';
 document.querySelector('[data-page-jump] button').disabled=!pages;document.querySelector('[data-page-error]').textContent='';input.removeAttribute('aria-invalid');
}
function workLink(item,extra={},workRef){
 const q={...scope(),creatorKey:item?.creatorKey,...extra};delete q.page;delete q.pageSize;delete q.observation;delete q.focus;
 if(q.unknownAuthor){for(const key of ['creatorKey','query','traits','topicHint','relevance','minLikes','viral','highLikes','sort'])delete q[key];q.searchMode='author';}
 else if(q.searchMode==='author')delete q.query;
 const p=new URLSearchParams({domain,creatorFilter:JSON.stringify(q),returnTo:location.pathname+location.search});
 if(item)p.set('creatorKey',item.creatorKey);if(workRef)p.set('work',workRef);
 return '/corpus/evidence?'+p;
}
function scopeWorkLink(item,relevance){return workLink(item,{relevance,query:undefined,traits:undefined,topicHint:undefined,minLikes:undefined,viral:false,highLikes:false});}
// A cited work may be outside the list's current date/expression filter. Clear only those
// temporary filters for the exact work; retain domain, author identity and material usage.
function evidenceLink(item,workRef){return workLink(item,{publishedFrom:undefined,publishedTo:undefined,query:undefined,traits:undefined,topicHint:undefined,relevance:undefined,minLikes:undefined,viral:false,highLikes:false,sort:'recent'},workRef);}
function asset(url){if(typeof url!=='string'||!url.startsWith('/api/local/'))return null;const u=new URL(url,location.origin);return u.origin===location.origin&&/^\/api\/local\/(media|derivative)\//.test(u.pathname)?u.pathname+u.search:null;}
function title(work){return resources.get(work.workRef)?.display?.title||work.title||'标题未取得';}
function cover(work){return `<span class="creator-cover" data-cover="${esc(work.workRef)}" aria-label="封面未取得">▧</span>`;}
function avatar(item){return `<span class="creator-avatar" data-avatar="${esc(item.representatives?.[0]?.workRef||'')}" aria-label="头像未取得">${esc(Array.from(item.displayName||'作者')[0])}</span>`;}
function paintMedia(root=document){
 for(const el of root.querySelectorAll('[data-cover],[data-avatar]')){
  const isAvatar=el.hasAttribute('data-avatar'),ref=isAvatar?el.dataset.avatar:el.dataset.cover,media=resources.get(ref)?.media;
  const candidate=isAvatar?(media?.avatar?.blob?.deliveryState==='INLINE_SAFE'?media.avatar.localAssetUrl:null):media?.cover?.localAssetUrl;
  const url=asset(candidate);if(!url||el.dataset.loaded===url)continue;
  const img=document.createElement('img');img.src=url;img.alt=isAvatar?'作者头像':'作品封面';img.loading='lazy';img.addEventListener('error',()=>{el.replaceChildren(document.createTextNode(isAvatar?'·':'▧'));el.removeAttribute('data-loaded');},{once:true});el.replaceChildren(img);el.dataset.loaded=url;
 }
 for(const el of root.querySelectorAll('[data-work-title]')){const display=resources.get(el.dataset.workTitle)?.display?.title;if(display){el.textContent=display;el.title=display;}}
}
async function hydrate(refs,current,signal){
 const unique=[...new Set(refs.filter(Boolean))].slice(0,100);if(!unique.length)return true;
 try{const data=await request('/api/local/work-resources?'+new URLSearchParams({domain,publicRefs:unique.join(',')}),{signal});if(current!==sequence)return false;for(const item of data.items||[]){if(item.identity?.publicRef)resources.set(item.identity.publicRef,item);}paintMedia();return true;}catch(e){return e.name==='AbortError'?null:false;}
}
function analysisState(analysis,value){
 if(analysis?.state==='running')return '分析中';if(analysis?.state==='queued')return '排队待分析';if(analysis?.state==='failed')return '分析失败';if(analysis?.state==='paused')return '分析已暂停';
 if((analysis?.resultAt||analysis?.resultAtDisplay)&&(value==='unknown'||!value))return '已分析，依据不足';
 if(analysis?.resultAt||analysis?.resultAtDisplay)return '已有分析结果';return result?.policy?.analysisEnabled?'尚未分析':'自动分析未开启';
}
function observed(item){const o=item.observation;if(o?.lookupState==='unavailable'||o?.inCurrentDomain==null)return '观察关系未知';if(o.inCurrentDomain)return (labels[o.monitoringState]||'观察状态待核对')+(o.usageRole==='reference'?' · 参照':'');if(['paused','dismissed'].includes(o.lifecycleState))return `${labels[o.lifecycleState]} · 未加入本领域`;return o.existsInOtherDomains?'仅其他领域已观察':'未加入观察';}
function targetLink(item){return '/collection/targets?'+new URLSearchParams({domain,drawer:item.observation.targetRef});}
function rowAction(item,index){
 const o=item.observation;if(o?.inCurrentDomain==null||o?.lookupState==='unavailable')return '<span class="creator-secondary">关系待读取</span>';
 if(o.inCurrentDomain)return `<a href="${esc(targetLink(item))}">查看目标</a>`;
 if((query.usageRole==='primary'&&o.hasOtherPrimaryDomain)||['paused','dismissed'].includes(o.lifecycleState))return `<button type="button" data-open="${index}" data-tab="profile">查看加入方式</button>`;
 return `<button type="button" data-add="${index}" data-role="${esc(query.usageRole)}" ${result.domain.status!=='active'?'disabled':''}>加入观察</button>`;
}
function observationAction(item,index){
 const o=item.observation;if(o?.inCurrentDomain==null||o?.lookupState==='unavailable')return '<button type="button" disabled>观察关系暂不可用</button>';
 if(o.inCurrentDomain)return `<a href="${esc(targetLink(item))}">查看观察目标 →</a>`;
 const kept=['paused','dismissed'].includes(o.lifecycleState)?labels[o.lifecycleState]:null,blocked=query.usageRole==='primary'&&o.hasOtherPrimaryDomain;
 const role=blocked?'reference':query.usageRole,disabled=result.domain.status!=='active';
 const label=blocked?'按参照用途加入':kept?`加入并保持${kept}`:role==='reference'?'按参照用途加入':'加入本领域观察';
 return `<button type="button" class="creator-primary" data-add="${index}" data-role="${esc(role)}" ${disabled?'disabled':''}>${label}</button>${blocked?'<small>其他领域已有主研究关系；此操作仅加入参照用途，不改变原巡查规则。</small>':''}${o.targetRef?`<a href="${esc(targetLink(item))}">查看原目标</a>`:''}`;
}
function hasWorkFilter(){return !!(query.traits||query.topicHint||query.relevance||query.minLikes!=null||query.viral||query.highLikes||(query.query&&query.searchMode==='work'));}
const highMode = () => query.sort==='high_likes'||query.highLikes||query.viral;
function renderRows(){
 const filtered=hasWorkFilter(),high=highMode();
 $('related-heading').textContent=filtered?'命中相关':'相关作品';$('likes-heading').textContent=high?'命中最高赞':'相关最高赞';
 $('rows').innerHTML=result.items.map((item,index)=>{
  const work=item.representatives?.[0],directions=(item.topicHints||[]).filter(Boolean).join(' · '),traits=(item.traits||[]).filter(k=>k!=='institution_or_brand'&&labels[k]),focus=item.focus?.value||'unknown';
  const n=filtered?item.matchedRelatedWorkCount:item.relatedWorkCount,likes=high?item.candidateHighLikeCount:item.maxRelatedLikeCount;
  const nLink=filtered?workLink(item,{relevance:'related'}):scopeWorkLink(item,'related');
  const state=analysisState(item.authorAnalysis,focus);
  return `<tr data-key="${esc(item.creatorKey)}"><td><div class="creator-cell creator-author-cell">${avatar(item)}<div class="creator-cell-copy"><button type="button" class="creator-link creator-name creator-ellipsis" data-open="${index}" data-key="${esc(item.creatorKey)}" title="${esc(item.displayName||'作者名称未取得')}">${esc(item.displayName||'作者名称未取得')}</button><span class="creator-secondary">粉丝 ${count(item.profile?.followerCount)}</span></div></div></td><td><div class="creator-cell"><button type="button" class="creator-link" data-open="${index}" title="${esc(directions||'内容方向尚未形成')}"><span class="creator-clamp">${esc(directions||'内容方向尚未形成')}</span></button></div></td><td><div class="creator-cell"><button type="button" class="creator-link creator-ellipsis" data-open="${index}" title="${esc(labels[focus])}">${esc(labels[focus])}</button><span class="creator-secondary" title="${esc(state)}">${esc(state)}</span></div></td><td><div class="creator-cell"><button type="button" class="creator-link" data-open="${index}" data-tab="evidence" title="${esc(traits.map(k=>labels[k]).join('、')||'暂无已确认表达')}"><span class="creator-expression"><span>${esc(traits.length?labels[traits[0]]:'—')}</span>${traits.length>1?`<small>+${traits.length-1}</small>`:''}</span></button></div></td><td class="creator-number"><div class="creator-cell">${n>0?`<a class="creator-metric" href="${esc(nLink)}" data-return-key="${esc(item.creatorKey)}">${count(n)}</a>`:`<span class="creator-metric">${count(n)}</span>`}${filtered?`<span class="creator-secondary">范围内 ${count(item.relatedWorkCount)}</span>`:item.unknownRelevanceWorkCount>0?`<a class="creator-secondary" href="${esc(scopeWorkLink(item,'unknown'))}" data-return-key="${esc(item.creatorKey)}">待判断 ${count(item.unknownRelevanceWorkCount)}</a>`:'<span class="creator-secondary">篇相关作品</span>'}</div></td><td class="creator-number"><div class="creator-cell"><span class="creator-metric" title="${esc(count(likes))}">${count(likes)}</span><span class="creator-secondary">${high?'当前命中作品':'已判相关作品'}</span></div></td><td><div class="creator-cell">${work?`<div class="creator-work-cell">${cover(work)}<div class="creator-cell-copy"><a class="creator-clamp" data-work-title="${esc(work.workRef)}" data-return-key="${esc(item.creatorKey)}" href="${esc(workLink(item,{},work.workRef))}" title="${esc(title(work))}">${esc(title(work))}</a><span class="creator-secondary">${esc(labels[work.relevance]||'待判断')} · 赞 ${count(work.likes)}</span></div></div>`:'<span class="creator-secondary">暂无可用作品</span>'}</div></td><td><div class="creator-cell"><span class="creator-ellipsis" title="${esc(observed(item))}">${esc(observed(item))}</span><div class="creator-row-action">${rowAction(item,index)}</div></div></td></tr>`;
 }).join('');
 for(const th of document.querySelectorAll('#creator-results thead th'))th.removeAttribute('aria-sort');
 document.querySelector(`[data-sort="${query.sort}"]`)?.closest('th').setAttribute('aria-sort','descending');
 paintMedia($('rows'));syncPendingButtons();
}
function conditionEntries(){return extraKeys.filter(k=>query[k]!==undefined&&query[k]!==''&&query[k]!==false).flatMap(k=>k==='traits'?query.traits.split(',').filter(Boolean).map(v=>({key:k,value:v,text:labels[v]||v})):[{key:k,text:({query:`${query.searchMode==='work'?'作品':'作者'}：${query.query}`,topicHint:`方向：${query.topicHint}`,observation:observationLabels[query.observation],minLikes:`最低点赞：${count(query.minLikes)}`,relevance:labels[query.relevance],focus:labels[query.focus],viral:'有相关爆款',highLikes:'有高赞线索'})[k]||k}]);}
function renderChrome(){
 const s=result.stats,p=result.policy,hasRule=p.likeThreshold!=null,entries=conditionEntries();
 $('result-count').textContent=`${count(result.total)} 位创作者`;
 const range=query.publishedFrom||query.publishedTo?`${query.publishedFrom||'不限'} 至 ${query.publishedTo||'不限'}（不含）`:'全部日期';
 $('scope-summary').textContent=`${query.usageRole==='reference'?'参照':'主研究'} · ${range}`;$('scope-summary').title=$('scope-summary').textContent;
 $('analysis-state').textContent=p.analysisEnabled?'自动分析已开启':'自动分析未开启';$('analysis-state').dataset.enabled=String(!!p.analysisEnabled);
 $('filter-count').textContent=String(entries.length);$('active-conditions').hidden=!entries.length;
 $('active-conditions').innerHTML=entries.map(e=>`<button type="button" data-remove="${esc(e.key)}" ${e.value?`data-value="${esc(e.value)}"`:''} title="${esc(e.text)}" aria-label="移除条件：${esc(e.text)}">${esc(e.text)} ×</button>`).join('')+(entries.length?'<button type="button" data-clear>清除筛选</button>':'');
 const clean=!extraKeys.some(k=>query[k]!==undefined&&query[k]!==''&&query[k]!==false),high=query.observation==='outside'&&(query.highLikes||query.viral);
 for(const button of $('quick').querySelectorAll('[data-quick]')){const key=button.dataset.quick;button.setAttribute('aria-pressed',String(key==='all'?clean:key==='high'?!!high:query.observation==='outside'&&!high));if(key!=='all')button.disabled=s.outside==null;}
 $('quick').querySelector('[data-quick=all]').textContent=`全部创作者 ${count(s.authors)}`;
 $('quick').querySelector('[data-quick=outside]').textContent=`未加入观察 ${count(s.outside)}`;
 $('quick').querySelector('[data-quick=high]').textContent=hasRule?`未观察的爆款作者 ${count(s.outsideViral)}`:'未观察的高赞作者';
 form.elements.sort.querySelector('[value=viral_works]').disabled=!hasRule;
 $('stats').innerHTML=[['all','范围内作者',s.authors],['vertical','持续创作倾向',s.vertical],['personal','有亲历作品',s.personal],['outside','未加入观察',s.outside],['high',hasRule?'未观察的爆款作者':'未观察的高赞作者',hasRule?s.outsideViral:null]].map(([key,text,value])=>`<button type="button" data-stat="${key}" ${(key==='outside'||key==='high')&&s.outside==null?'disabled':''}>${text}<strong>${key==='high'&&!hasRule?'查看 →':count(value)}</strong></button>`).join('');
 $('coverage').innerHTML=`当前范围 ${count(s.works)} 篇作品 · 已确认相关 ${count(s.relatedAuthors)} 位作者。<br><a href="${esc(workLink(null,{unknownAuthor:true}))}">作者身份待补 ${count(s.unknownAuthorWorks)} 篇</a>${(query.publishedFrom||query.publishedTo)&&s.unknownDateWorks>0?` · <button type="button" id="unknown-dates" class="creator-text-button">另有 ${count(s.unknownDateWorks)} 篇发布时间未知</button>`:''}`;
 $('domain-context').innerHTML=`<p><strong>${esc(result.domain.name||'当前领域')}</strong> · ${query.usageRole==='reference'?'参照用途':'主研究用途'}</p><p>领域说明：${esc(result.domain.description||'尚未填写')}</p><p>研究目标：${esc(result.domain.researchGoal||'尚未填写')}</p>${!p.analysisEnabled?'<p>自动分析未开启，当前标签仅统计已有判断。</p>':''}`;
 $('table-empty').hidden=result.total!==0;$('creator-results').setAttribute('aria-busy','false');pageControls();syncToolbar();
 const limitations=[];if(!p.analysisEnabled&&(query.traits||query.focus||query.topicHint))limitations.push('仅检索已有判断；自动分析尚未开启。');if(result.domain.status!=='active')limitations.push('当前领域已暂停，历史材料仍可读，写入操作不可用。');setStatus(limitations.join(' '));
}
async function load({push=false,restore=null}={}){
 if(navigating)return;clearTimeout(timer);controller?.abort();const current=++sequence;closeTransient();controller=new AbortController();const signal=controller.signal;
 result=null;resources.clear();$('rows').replaceChildren();$('table-empty').hidden=true;$('creator-results').setAttribute('aria-busy','true');$('result-count').textContent=domain?'读取中…':'请选择领域';pageControls();setStatus(domain?'读取创作者与作品…':'请选择领域。');
 const header=$('creator-header-readout');if(header)header.textContent='';
 if(!domain){$('creator-results').setAttribute('aria-busy','false');return;}
 const url='/corpus/creators?'+params(scope());if(url!==location.pathname+location.search)history[push?'pushState':'replaceState'](null,'',url);
 try{
  let data=await request('/api/local/creators?'+params(scope()),{signal});if(current!==sequence)return;
  if(data?.domain?.domainRef!==domain||!Array.isArray(data.items)||!Number.isFinite(data.total))throw new Error('invalid_response');
  const last=Math.max(0,Math.ceil(data.total/Number($('page-size').value))-1);
  if(page>last){page=last;history.replaceState(null,'','/corpus/creators?'+params(scope()));data=await request('/api/local/creators?'+params(scope()),{signal});if(current!==sequence)return;if(data?.domain?.domainRef!==domain||!Array.isArray(data.items)||!Number.isFinite(data.total))throw new Error('invalid_response');}
  result=data;renderChrome();renderRows();if(header)header.textContent=data.displayAsOf?`读取于 ${data.displayAsOf}`:'读取时间未知';
  const back=restore||pendingReturn;pendingReturn=null;$('creator-results').scrollTop=Number.isFinite(back?.scrollY)?back.scrollY:0;$('creator-results').scrollLeft=Number.isFinite(back?.scrollX)?back.scrollX:0;
  const available=await hydrate(data.items.map(i=>i.representatives?.[0]?.workRef),current,signal);if(current!==sequence)return;
  if(available===false)setStatus('媒体暂不可用；文字、作品链接与查询仍可使用。');
  if(back?.creatorKey){focusAuthor(back.creatorKey);const index=data.items.findIndex(i=>i.creatorKey===back.creatorKey);if(back.drawer&&index>=0)openAuthor(index,back.tab||'overview',back);}
 }catch(error){if(error.name==='AbortError'||current!==sequence)return;result=null;$('rows').replaceChildren();$('result-count').textContent='读取失败';$('creator-results').setAttribute('aria-busy','false');$('stats').replaceChildren();$('coverage').replaceChildren();pageControls();setStatus(humanError(error.message),true);
  if(error.message==='observation_lookup_unavailable'){$('status').textContent='观察关系暂不可用。';const b=document.createElement('button');b.textContent='清除观察条件后重试';b.onclick=()=>{delete query.observation;delete query.viral;delete query.highLikes;page=0;load({push:true});};$('status').append(b);}
  else if(error.message==='viral_threshold_unset'){$('status').textContent='当前未设爆款标准。';const b=document.createElement('button');b.textContent='改看高赞线索';b.onclick=()=>{delete query.viral;query.highLikes=true;query.sort='high_likes';page=0;load({push:true});};$('status').append(b);}
  else{const b=document.createElement('button');b.textContent='重试读取';b.onclick=()=>load();$('status').append(' ',b);}
 }
}
function clearExtra(target){for(const key of extraKeys)delete target[key];target.sort='recent';return target;}
function quick(key){if(!result)return;if(['outside','high'].includes(key)&&result.stats.outside==null)return;clearExtra(query);if(key==='outside'||key==='high')query.observation='outside';if(key==='high'){if(result.policy.likeThreshold!=null){query.viral=true;query.sort='viral_works';}else{query.highLikes=true;query.sort='high_likes';}}if(key==='personal')query.traits='personal_experience';if(key==='vertical')query.focus='vertical_tendency';page=0;load({push:true});}
function writeDraft(value){filterDraft={...value};for(const key of filterKeys){if(key==='traits'){const checked=new Set((value.traits||'').split(','));for(const el of filterForm.querySelectorAll('[name=traits]'))el.checked=checked.has(el.value);}else{const el=filterForm.elements[key];if(el.type==='checkbox')el.checked=!!value[key];else el.value=value[key]??defaults[key]??'';}}$('window').value=value.publishedFrom||value.publishedTo?'custom':'all';$('date-fields').hidden=$('window').value!=='custom';}
function readDraft(){const draft={...(filterDraft||query)};for(const key of filterKeys){delete draft[key];if(key==='traits'){const values=[...filterForm.querySelectorAll('[name=traits]:checked')].map(el=>el.value);if(values.length)draft.traits=values.join(',');}else{const el=filterForm.elements[key];if(el.type==='checkbox'){if(el.checked)draft[key]=true;}else if(el.value!=='')draft[key]=key==='minLikes'?Number(el.value):el.value.trim();}}return draft;}
function openFilters(){const scopeData=result;flushSearch();writeDraft(query);$('filter-status').textContent='';const hasRule=scopeData?.policy?.likeThreshold!=null;filterForm.elements.viral.disabled=!hasRule;filterForm.elements.observation.disabled=scopeData?.stats?.outside==null;$('filter-limit').textContent=hasRule?'最低点赞只改变当前筛选，不修改爆款标准。':'尚未设置爆款标准；仍可筛选高赞线索。';$('filter-dialog').showModal();}
function presetDate(){const v=$('window').value;$('date-fields').hidden=v!=='custom';if(v==='custom')return;const from=filterForm.elements.publishedFrom,to=filterForm.elements.publishedTo;if(v==='all'){from.value='';to.value='';return;}const formatter=new Intl.DateTimeFormat('en-CA',{timeZone:'Asia/Shanghai',year:'numeric',month:'2-digit',day:'2-digit'}),today=formatter.format(new Date()),start=new Date(`${today}T00:00:00+08:00`),end=new Date(start);end.setUTCDate(end.getUTCDate()+1);start.setUTCDate(start.getUTCDate()-(Number(v)-1));from.value=formatter.format(start);to.value=formatter.format(end);}
function sourceField(field){return ({biography:'公开简介',title:'标题',body:'正文',ocr:'图文识别',transcript:'转写'})[field]||'材料片段';}
function sourceKind(work){return work.acquisitionKind==='profile_discovery'?'主页发现作品':work.acquisitionKind==='limited_domain_sample'?'本领域已有作品':'已有作品材料';}
function effective(analysis,field){return analysis?.manual?.[field]?.value||(field==='relevance'?analysis?.automatic?.relevance:field==='focus'?analysis?.automatic?.focus:field==='institution_or_brand'?analysis?.institution_or_brand:analysis?.automatic?.traits?.[field])||{value:'unknown'};}
function evidenceFor(analysis,field){
 const manual=analysis?.manual?.[field],value=effective(analysis,field),ids=new Set(manual?manual.supportFragmentIds||[]:value.evidenceFragmentIds||[]);
 const candidates=[...(analysis?.automatic?.evidence||[]),...(analysis?.evidence||[]),...(analysis?.supportFragments||[]),...(analysis?.identitySupportFragments||[])],seen=new Set();
 return candidates.filter(f=>ids.has(f.fragmentId)&&!seen.has(f.fragmentId)&&seen.add(f.fragmentId));
}
function evidenceBlock(fragment,item){
 const workRef=fragment.sourceType==='work_material'&&typeof fragment.workRef==='string'&&/^[0-9a-f]{8}-[0-9a-f-]{27,}$/i.test(fragment.workRef)?fragment.workRef:null;
 const link=workRef?` · <a href="${esc(evidenceLink(item,workRef))}" data-return-key="${esc(item.creatorKey)}">回查原作品</a>`:'';
 return `<blockquote>${esc(fragment.text||'片段暂不可读')}<small>${sourceField(fragment.field)}${link}</small></blockquote>`;
}
function timeLabel(analysis){return analysis?.resultAtDisplay?`自动依据更新于 ${analysis.resultAtDisplay}`:'自动依据更新时间未记录';}
function fieldBlock(item,analysis,field){
 const value=effective(analysis,field),evidence=evidenceFor(analysis,field),name=field==='focus'?'作者倾向':field==='relevance'?'领域相关性':labels[field],manual=analysis?.manual?.[field];
 return `<div class="creator-field"><div class="creator-field-heading"><strong>${esc(name)} · ${esc(labels[value.value]||'尚不能判定')}</strong><button type="button" class="creator-text-button" data-correct="${esc(field)}">调整判断</button></div>${value.reason?`<p class="creator-reason">${esc(value.reason)}</p><button type="button" class="creator-text-button" data-expand-reason>展开说明</button>`:''}<p class="creator-help">${manual?'人工调整'+(manual.at?' · '+esc(manual.at):''):esc(timeLabel(analysis))}</p>${evidence.length?`<details><summary>支持依据 · ${evidence.length} 个片段</summary>${evidence.map(f=>evidenceBlock(f,item)).join('')}</details>`:'<p class="creator-help">暂无可回查的支持片段。</p>'}</div>`;
}
function retryControl(analysis,author=false){return result.policy.analysisEnabled&&['failed','paused'].includes(analysis?.state)?`<button type="button" class="creator-text-button" ${author?'data-retry-author':'data-retry'}>重试失败分析</button>`:'';}
function workPreview(item,work){return `<div class="creator-preview-card">${cover(work)}<div><a class="creator-clamp" href="${esc(workLink(item,{},work.workRef))}" data-work-title="${esc(work.workRef)}">${esc(title(work))}</a><span class="creator-secondary">${esc(labels[work.relevance]||'待判断')} · 赞 ${count(work.likes)} · 藏 ${count(work.collects)} · 评 ${count(work.comments)}</span></div></div>`;}
function drawerBounds(){const shell=document.querySelector('.v7-shell');if(!shell)return;const top=Math.max(0,Math.min(shell.getBoundingClientRect().top,innerHeight));document.documentElement.style.setProperty('--creator-shell-top',`${top}px`);}
function setTab(next){if(!['overview','evidence','profile'].includes(next))next='overview';tabPositions[tab]=$('drawer-body').scrollTop;tab=next;for(const button of $('drawer-tabs').querySelectorAll('[data-tab]')){const active=button.dataset.tab===tab;button.setAttribute('aria-selected',String(active));button.tabIndex=active?0:-1;$('drawer-'+button.dataset.tab).hidden=!active;}$('drawer-body').scrollTop=tabPositions[tab]||0;}
async function openAuthor(index,next='overview',restore=null){
 const item=result?.items[index];if(!item)return;const current=sequence,draw=++drawerSequence;selected=item;selectedIndex=index;drawerTrigger=item.creatorKey;Object.keys(tabPositions).forEach(k=>delete tabPositions[k]);tab='overview';$('drawer-body').scrollTop=0;
 $('drawer-title').textContent=item.displayName||'作者名称未取得';$('drawer-title').title=item.displayName||'';$('drawer-avatar').innerHTML=avatar(item);
 $('drawer-subtitle').textContent=`${result.domain.name||'当前领域'} · ${query.usageRole==='reference'?'参照材料':'主研究材料'} · 小红书 · 粉丝 ${count(item.profile?.followerCount)}`;
 $('drawer-metrics').innerHTML=`<span>相关作品<strong>${count(item.relatedWorkCount)}</strong></span><span>待判断<strong>${count(item.unknownRelevanceWorkCount)}</strong></span><span>相关最高赞<strong>${count(item.maxRelatedLikeCount)}</strong></span>`;
 const analysis=item.authorAnalysis||{},focus=item.focus||{value:'unknown'},state=analysisState(analysis,focus.value),sample=analysis.automatic?.sampleWorkCount;
 const sampleText=Number.isInteger(sample)?`本次实际使用 ${sample} 篇作品摘要${Number.isInteger(analysis.automatic.availableWorkCount)?`；分析当时已有 ${analysis.automatic.availableWorkCount} 篇`:''}。`:'本次实际取样篇数未记录。';
 const authorEvidence=evidenceFor(analysis,'focus');
 $('drawer-overview').innerHTML=`<section><h3>作者内容倾向</h3><div class="creator-judgment-heading"><strong>${esc(labels[focus.value]||'尚不能判定')}</strong><button type="button" class="creator-text-button" data-correct="focus">调整判断</button></div><p class="creator-help">${esc(state)}</p><p class="creator-reason">${esc(focus.reason||'当前材料尚不足以形成作者整体判断。')}</p><button type="button" class="creator-text-button" data-expand-reason>展开说明</button><p class="creator-help">${sampleText}仅使用本领域已取得材料，不代表账号全部作品。</p>${retryControl(analysis,true)}${authorEvidence.length?`<details><summary>判断依据 · ${authorEvidence.length} 个片段</summary>${authorEvidence.map(f=>evidenceBlock(f,item)).join('')}</details>`:''}</section><section><h3>内容方向与作品表达</h3><p>${esc((item.topicHints||[]).join(' · ')||'内容方向尚未形成')}</p><div class="creator-tag-list">${(item.traits||[]).filter(k=>labels[k]).map(k=>`<span>${esc(labels[k])}</span>`).join('')}</div></section><section><div class="creator-field-heading"><h3>当前条件的作品预览</h3><button type="button" class="creator-text-button" data-switch-tab="evidence">查看依据 →</button></div>${(item.representatives||[]).map(w=>workPreview(item,w)).join('')||'<p class="creator-help">暂无可用作品。</p>'}<p class="creator-help">预览最多三篇；不是本次作者分析的全部样本。</p><a href="${esc(workLink(item))}">查看全部 ${count(item.matchedWorkRefs?.length)} 篇命中作品 →</a></section><details><summary>判断标准与材料范围</summary><p class="creator-help">作者倾向基于本领域累计材料；当前列表作品按所选日期和条件。亲历仅表示自述表达，不认证真实性。已判相关 ${count(item.relatedWorkCount)} 篇，待判断 ${count(item.unknownRelevanceWorkCount)} 篇，已判无关 ${count(item.unrelatedWorkCount)} 篇。</p><p class="creator-help">${result.policy.likeThreshold==null?'未设置爆款标准。':`爆款最低点赞 ${count(result.policy.likeThreshold)}；当前范围相关爆款 ${count(item.viralWorkCount)} 篇。`}</p></details>`;
 $('drawer-evidence').innerHTML=`<p class="creator-help">范围内 ${count(item.collectedWorkCount)} 篇 · 当前条件命中 ${count(item.matchedWorkRefs?.length)} 篇。下方预览最多三篇。</p><a href="${esc(workLink(item))}">查看全部命中作品 →</a>${(item.representatives||[]).map(w=>`<details class="creator-work-evidence" data-work="${esc(w.workRef)}"><summary>${cover(w)}<div><span class="creator-clamp" data-work-title="${esc(w.workRef)}" title="${esc(title(w))}">${esc(title(w))}</span><span class="creator-secondary">${esc(labels[w.relevance]||'待判断')} · 赞 ${count(w.likes)}</span></div></summary><div class="creator-work-body"><p class="creator-help">${esc(sourceKind(w))} · 收录于 ${esc(w.firstAddedDisplay||'未记录')} · 指标观察 ${esc(w.likesObservedDisplay||'未记录')}</p><a href="${esc(evidenceLink(item,w.workRef))}">打开原作品 →</a>${['relevance','personal_experience','professional_output','explicit_promotion'].map(field=>fieldBlock(item,w.analysis,field)).join('')}<p class="creator-help">${esc(analysisState(w.analysis,w.relevance))}</p>${retryControl(w.analysis)}${w.analysis?.lastErrorCode?`<details><summary>处理信息</summary><p>${esc(w.analysis.lastErrorCode)}</p></details>`:''}</div></details>`).join('')}`;
 $('drawer-profile').innerHTML=`<section><h3>公开简介</h3><p class="creator-profile-copy">${esc(item.profile?.biography||'公开简介尚未取得。')}</p></section><section><h3>公开身份线索</h3>${fieldBlock(item,analysis,'institution_or_brand')}<p class="creator-help">仅依据公开资料，不根据昵称、头像或粉丝量推断身份。</p></section><section><h3>材料与来源</h3><dl class="creator-profile-list"><dt>来源平台</dt><dd>小红书</dd><dt>平台账号</dt><dd>${esc(item.authorExternalId||'未取得')}</dd><dt>当前领域</dt><dd>${esc(result.domain.name||'当前领域')}</dd><dt>范围内作品</dt><dd>${count(item.collectedWorkCount)} 篇</dd><dt>当前命中</dt><dd>${count(item.matchedWorkRefs?.length)} 篇</dd><dt>观察关系</dt><dd>${esc(observed(item))}</dd><dt>实际取样</dt><dd>${sampleText}</dd></dl></section><details><summary>处理信息</summary><p class="creator-help">${esc(timeLabel(analysis))}</p><p class="creator-help">${esc(analysis.lastErrorCode||'未记录错误代码')}</p><p class="creator-help">来源采样：${esc(analysis.automatic?.sampleBasis||'未记录')}</p></details>`;
 $('drawer-observation').textContent=observed(item);$('drawer-action-status').textContent='';$('drawer-action').innerHTML=observationAction(item,index);
 if(result.domain.status!=='active')for(const b of $('drawer-body').querySelectorAll('button[data-correct],button[data-retry],button[data-retry-author]'))b.disabled=true;
 for(const row of $('rows').children)row.dataset.selected=String(row.dataset.key===item.creatorKey);
 drawerBounds();setTab(next);if(!drawer.open)drawer.showModal();paintMedia(drawer);syncPendingButtons();
 if(restore){for(const el of $('drawer-evidence').querySelectorAll('[data-work]'))el.open=(restore.expanded||[]).includes(el.dataset.work);$('drawer-body').scrollTop=restore.drawerScroll||0;}
 const media=await hydrate((item.representatives||[]).map(w=>w.workRef),current,controller.signal);
 if(draw!==drawerSequence||current!==sequence||!drawer.open)return;if(media===false)$('drawer-action-status').textContent='部分媒体未能读取；原有文字和证据入口仍可用。';
}
function contextCurrent(context){return !navigating&&context.sequence===sequence&&context.draw===drawerSequence&&selected?.creatorKey===context.creatorKey&&drawer.open;}
function addKey(item,role){return `add:${domain}:${item.creatorKey}:${role}`;}
function syncPendingButtons(){
 for(const button of document.querySelectorAll('[data-add],[data-retry],[data-retry-author]')){
  const isAdd=button.hasAttribute('data-add'),item=isAdd?result?.items[Number(button.dataset.add)]:selected;
  if(!item)continue;
  const key=isAdd?addKey(item,button.dataset.role):`retry:${domain}:${button.hasAttribute('data-retry-author')?item.creatorKey:button.closest('[data-work]')?.dataset.work}`;
  if(pendingWrites.has(key)){
   if(!button.hasAttribute('data-pending-label')){button.dataset.pendingLabel=button.textContent;button.dataset.pendingDisabled=String(button.disabled);}
   button.disabled=true;button.textContent=isAdd?'加入中…':'正在排队…';
  }else if(button.hasAttribute('data-pending-label')){
   button.disabled=button.dataset.pendingDisabled==='true';button.textContent=button.dataset.pendingLabel;delete button.dataset.pendingLabel;delete button.dataset.pendingDisabled;
  }
 }
}
async function addObservation(button){
 if(button.disabled||!result||result.domain.status!=='active')return;const item=result.items[Number(button.dataset.add)],role=button.dataset.role;if(!item||!['primary','reference'].includes(role))return;
 const key=addKey(item,role);if(pendingWrites.has(key))return;const context={sequence,draw:drawerSequence,creatorKey:item.creatorKey},fromDrawer=drawer.contains(button);
 const current=()=>!navigating&&context.sequence===sequence&&context.draw===drawerSequence&&(fromDrawer?contextCurrent(context)&&![...document.querySelectorAll('dialog[open]')].some(d=>d!==drawer):!document.querySelector('dialog[open]'));
 pendingWrites.add(key);syncPendingButtons();
 try{const receipt=await mutate(`/api/local/creators/${encodeURIComponent(item.creatorKey)}/observation-target`,'POST',{domain,usageRole:role});if(!current())return;
  await load();if(navigating||sequence!==context.sequence+1)return;const kept=['paused','dismissed'].includes(receipt.lifecycleState)?`，目标保持${labels[receipt.lifecycleState]}`:'';setStatus(`已加入本领域${receipt.usageRole==='reference'?'参照用途':'观察目标'}${kept}。`);
 }catch(error){if(current()){if(fromDrawer)$('drawer-action-status').textContent=humanError(error.message);else setStatus(humanError(error.message),true);button.disabled=false;button.textContent='重试加入';}}
 finally{pendingWrites.delete(key);syncPendingButtons();}
}
function openCorrection(field,workRef){
 if(!selected||!result||result.domain.status!=='active')return;const authorField=['focus','institution_or_brand'].includes(field),work=authorField?null:selected.representatives.find(w=>w.workRef===workRef);if(!authorField&&!work)return;
 const analysis=authorField?selected.authorAnalysis:work.analysis,manual=analysis?.manual?.[field],current=field==='focus'?(manual?.value||selected.focus):effective(analysis,field);
 correction={domain,creatorKey:selected.creatorKey,workRef:work?.workRef||null,field,sequence,draw:drawerSequence};
 const supports=authorField?(field==='institution_or_brand'?analysis?.identitySupportFragments:analysis?.supportFragments):analysis?.supportFragments,existing=new Set(manual?.supportFragmentIds||[]),groups=new Map();
 for(const fragment of supports||[]){const groupKey=fragment.field==='biography'?'profile':fragment.workRef||'unattributed';if(!groups.has(groupKey))groups.set(groupKey,{title:groupKey==='profile'?'公开简介':fragment.workTitle||'作品材料',fragments:[]});groups.get(groupKey).fragments.push(fragment);}
 $('support-options').innerHTML=[...groups.values()].map(group=>`<details open><summary>${esc(group.title)} · ${group.fragments.length} 个片段</summary>${group.fragments.map(f=>`<label><input type="checkbox" value="${esc(f.fragmentId)}" ${existing.has(f.fragmentId)?'checked':''}><span>${sourceField(f.field)} · ${esc(f.text||'片段暂不可读')}</span></label>`).join('')}</details>`).join('');
 $('correction-support').hidden=false;if(!groups.size)$('support-options').innerHTML='<p class="creator-help">当前没有可选支持材料；仍可选择尚不能判定或恢复自动。</p>';
 $('correction-title').textContent='调整'+(field==='focus'?'作者倾向':field==='relevance'?'领域相关性':labels[field]);$('correction-context').textContent=authorField?selected.displayName||'作者名称未取得':title(work);
 const values=field==='relevance'?['related','unrelated','unknown']:field==='focus'?['vertical_tendency','multi_topic','unknown']:['yes','no','unknown'];$('correction-value').innerHTML=values.map(v=>`<option value="${v}">${labels[v]}</option>`).join('');$('correction-value').value=current?.value||'unknown';$('correction-reason').value=manual?.value?.reason||'';$('correction-restore').disabled=!manual;$('correction-status').textContent='';$('correction-status').dataset.error='false';
 $('correction-form').querySelector('[type=submit]').disabled=false;$('correction-dialog').showModal();
}
async function saveCorrection(value){
 const context=correction;if(!context||!contextCurrent(context)||pendingWrites.has(context))return;
 const ids=[...$('support-options').querySelectorAll('input:checked')].map(el=>el.value),positive=value!==null&&((context.field==='relevance'&&value==='related')||(context.field==='focus'&&value!=='unknown')||(!['focus','relevance'].includes(context.field)&&value==='yes'));
 const status=$('correction-status');if(positive&&!ids.length){status.textContent='请选择当前材料作为支持依据。';status.dataset.error='true';return;}
 const current=()=>correction===context&&contextCurrent(context)&&$('correction-dialog').open;
 pendingWrites.add(context);status.textContent='保存中…';status.dataset.error='false';const submit=$('correction-form').querySelector('[type=submit]');submit.disabled=true;
 try{const {sequence:ignored,draw:ignoredDraw,...identity}=context;await mutate('/api/local/creator-discovery-overrides','PUT',{...identity,value,reason:value===null?null:$('correction-reason').value.trim()||null,supportFragmentIds:ids});if(!current())return;const back={creatorKey:selected.creatorKey,drawer:true,tab,drawerScroll:$('drawer-body').scrollTop,scrollY:$('creator-results').scrollTop,scrollX:$('creator-results').scrollLeft};await load({restore:back});}
 catch(error){if(current()){status.textContent=humanError(error.message);status.dataset.error='true';}}
 finally{pendingWrites.delete(context);if(current())submit.disabled=false;}
}
async function retry(button){
 if(button.disabled||!selected||!result||result.domain.status!=='active')return;const work=button.closest('[data-work]')?.dataset.work,author=button.hasAttribute('data-retry-author'),context={sequence,draw:drawerSequence,creatorKey:selected.creatorKey};
 const key=`retry:${domain}:${author?selected.creatorKey:work}`;if(pendingWrites.has(key))return;pendingWrites.add(key);syncPendingButtons();
 try{const receipt=await mutate('/api/local/creator-discovery-analysis/retry','POST',{domain,...(author?{creatorKey:selected.creatorKey}:{workRef:work})});if(contextCurrent(context))$('drawer-action-status').textContent=receipt.queued?'已排队，符合现有配置及额度后执行。':'当前没有可重试的失败分析。';}
 catch(error){if(contextCurrent(context)){$('drawer-action-status').textContent=humanError(error.message);button.disabled=false;}}
 finally{pendingWrites.delete(key);syncPendingButtons();}
}
async function openPolicy(){
 if(!result)return;const context={domain,revision:result.policy.revision,sequence,policy:{...result.policy}};policyContext=context;
 $('threshold').value=context.policy.likeThreshold??'';$('analysis-enabled').checked=!!context.policy.analysisEnabled;$('daily-token-limit').value=context.policy.dailyTokenLimit??'';
 $('policy-status').textContent=result.domain.status!=='active'?'当前领域已暂停，不能修改设置。':'';$('policy-status').dataset.error='false';$('policy-form').querySelector('[type=submit]').disabled=result.domain.status!=='active';
 $('model-config').replaceChildren();$('model-config').disabled=true;$('analysis-enabled').disabled=true;$('daily-token-limit').disabled=true;$('policy-dialog').showModal();
 const current=()=>policyContext===context&&context.sequence===sequence&&$('policy-dialog').open&&!navigating;
 try{const settings=await request('/api/local/model-settings');if(!current())return;const configs=settings.config?[settings.config]:[];$('model-config').innerHTML='<option value="">选择模型配置</option>';for(const config of configs){const option=document.createElement('option');option.value=config.configRef;option.textContent=config.modelId||config.configRef;$('model-config').append(option);}if(context.policy.configRef&&!configs.some(c=>c.configRef===context.policy.configRef)){const option=document.createElement('option');option.value=context.policy.configRef;option.textContent='当前来源分析配置';$('model-config').append(option);}$('model-config').value=context.policy.configRef||'';$('model-config').disabled=false;$('analysis-enabled').disabled=false;$('daily-token-limit').disabled=false;}
 catch(error){if(current())$('policy-status').textContent='模型配置读取失败。可保存点赞标准，分析设置保持原值。';}
}
async function savePolicy(event){
 event.preventDefault();const context=policyContext,status=$('policy-status');if(!context||context.sequence!==sequence||!result||result.domain.status!=='active'||pendingWrites.has(context))return;
 const available=!$('model-config').disabled,enabled=$('analysis-enabled').checked,limit=$('daily-token-limit').value?Number($('daily-token-limit').value):null,config=$('model-config').value||null;
 if(available&&enabled&&(!config||!Number.isSafeInteger(limit)||limit<1024||limit>10000000)){status.textContent='启用分析前请选择模型，并填写有效的每日词元上限。';status.dataset.error='true';return;}
 const payload={domain:context.domain,revision:context.revision,likeThreshold:$('threshold').value?Number($('threshold').value):null};if(available)Object.assign(payload,{analysisEnabled:enabled,configRef:config,dailyTokenLimit:limit});
 const current=()=>policyContext===context&&context.sequence===sequence&&$('policy-dialog').open&&!navigating;
 pendingWrites.add(context);status.textContent='保存中…';status.dataset.error='false';const submit=$('policy-form').querySelector('[type=submit]');submit.disabled=true;
 try{await mutate('/api/local/creator-discovery-policy','PUT',payload);if(current())await load();}
 catch(error){if(current()){status.textContent=humanError(error.message);status.dataset.error='true';}}
 finally{pendingWrites.delete(context);if(current())submit.disabled=false;}
}
function closeDialog(id){if(id==='policy-dialog')policyContext=null;if(id==='correction-dialog')correction=null;$(id).close();}
function changePage(value){const pages=result?Math.ceil(result.total/Number($('page-size').value)):0;if(!result||value<0||value>=pages||value===page)return;page=value;load({push:true});}
function flushSearch(){clearTimeout(timer);const value=form.elements.query.value.trim();if(value===(query.query||''))return;if(value)query.query=value;else delete query.query;page=0;load({push:true});}
function scheduleSearch(){if(composing)return;clearTimeout(timer);timer=setTimeout(flushSearch,250);}
form.addEventListener('submit',event=>{event.preventDefault();if(composing)return;clearTimeout(timer);query.query=form.elements.query.value.trim();if(!query.query)delete query.query;page=0;load({push:true});});
form.elements.query.addEventListener('input',scheduleSearch);form.elements.query.addEventListener('compositionstart',()=>{composing=true;clearTimeout(timer);});form.elements.query.addEventListener('compositionend',()=>{composing=false;scheduleSearch();});
form.elements.searchMode.addEventListener('change',()=>{query.searchMode=form.elements.searchMode.value;query.query=form.elements.query.value.trim();page=0;load({push:true});});
form.elements.sort.addEventListener('change',()=>{query.sort=form.elements.sort.value;query.query=form.elements.query.value.trim();if(!query.query)delete query.query;page=0;load({push:true});});
$('search-clear').onclick=()=>{delete query.query;page=0;load({push:true});};$('refresh').onclick=()=>{page=0;load();};$('filter-open').onclick=openFilters;$('window').onchange=presetDate;
filterForm.onsubmit=event=>{event.preventDefault();if(!filterForm.reportValidity())return;const draft=readDraft();if(draft.publishedFrom&&draft.publishedTo&&draft.publishedFrom>=draft.publishedTo){$('filter-status').textContent='结束日期必须晚于开始日期（结束日不含）。';$('filter-status').dataset.error='true';return;}query=draft;page=0;load({push:true});};
$('reset').onclick=()=>writeDraft(clearExtra(readDraft()));$('empty-reset').onclick=()=>{clearExtra(query);page=0;load({push:true});};
$('active-conditions').onclick=event=>{const button=event.target.closest('button');if(!button)return;if(button.hasAttribute('data-clear'))clearExtra(query);else if(button.dataset.remove==='traits'){const traits=(query.traits||'').split(',').filter(v=>v!==button.dataset.value);if(traits.length)query.traits=traits.join(',');else delete query.traits;}else delete query[button.dataset.remove];page=0;load({push:true});};
$('quick').onclick=event=>{const b=event.target.closest('[data-quick]');if(b&&!b.disabled)quick(b.dataset.quick);};$('stats').onclick=event=>{const b=event.target.closest('[data-stat]');if(b&&!b.disabled)quick(b.dataset.stat);};
$('scope-open').onclick=()=>{if(result)$('scope-dialog').showModal();};$('coverage').onclick=event=>{if(event.target.id==='unknown-dates'){delete query.publishedFrom;delete query.publishedTo;page=0;load({push:true});}};
$('rows').onclick=event=>{const open=event.target.closest('[data-open]');if(open){openAuthor(Number(open.dataset.open),open.dataset.tab||'overview');return;}const add=event.target.closest('[data-add]');if(add)addObservation(add);};
$('creator-results').querySelector('thead').onclick=event=>{const b=event.target.closest('[data-sort]');if(b){query.sort=b.dataset.sort;page=0;load({push:true});}};
$('drawer-close').onclick=()=>drawer.close();drawer.addEventListener('close',()=>{if(drawer.open)return;drawerSequence++;for(const row of $('rows').children)delete row.dataset.selected;if(!suppressFocus&&!navigating)focusAuthor(drawerTrigger);selected=null;});
$('drawer-tabs').onclick=event=>{const b=event.target.closest('[data-tab]');if(b)setTab(b.dataset.tab);};$('drawer-tabs').onkeydown=event=>{if(!['ArrowLeft','ArrowRight','Home','End'].includes(event.key))return;event.preventDefault();const tabs=[...$('drawer-tabs').querySelectorAll('[data-tab]')],i=tabs.findIndex(b=>b.dataset.tab===tab),next=event.key==='Home'?0:event.key==='End'?tabs.length-1:(i+(event.key==='ArrowRight'?1:-1)+tabs.length)%tabs.length;setTab(tabs[next].dataset.tab);tabs[next].focus();};
$('drawer-body').onclick=event=>{const b=event.target.closest('button');if(!b||b.disabled)return;if(b.hasAttribute('data-correct'))openCorrection(b.dataset.correct,b.closest('[data-work]')?.dataset.work);else if(b.hasAttribute('data-retry')||b.hasAttribute('data-retry-author'))retry(b);else if(b.dataset.switchTab)setTab(b.dataset.switchTab);else if(b.hasAttribute('data-expand-reason')){const reason=b.previousElementSibling;const expanded=reason.dataset.expanded!=='true';reason.dataset.expanded=String(expanded);b.textContent=expanded?'收起说明':'展开说明';}};
$('drawer-action').onclick=event=>{const b=event.target.closest('[data-add]');if(b)addObservation(b);};
$('policy-open').onclick=openPolicy;$('policy-close').onclick=()=>closeDialog('policy-dialog');$('policy-form').onsubmit=savePolicy;
$('correction-close').onclick=()=>closeDialog('correction-dialog');$('correction-restore').onclick=()=>saveCorrection(null);$('correction-form').onsubmit=event=>{event.preventDefault();saveCorrection($('correction-value').value);};
for(const id of ['policy-dialog','correction-dialog'])$(id).addEventListener('cancel',()=>{if(id==='policy-dialog')policyContext=null;else correction=null;});
document.addEventListener('click',event=>{const close=event.target.closest('[data-close]');if(close)closeDialog(close.dataset.close);const link=event.target.closest('a[href^="/corpus/evidence?"]');if(link)saveReturn(link.dataset.returnKey);});
document.querySelector('[data-page-prev]').onclick=()=>changePage(page-1);document.querySelector('[data-page-next]').onclick=()=>changePage(page+1);$('page-size').onchange=()=>{page=0;load({push:true});};
document.querySelector('[data-page-jump]').onsubmit=event=>{event.preventDefault();const input=document.querySelector('[data-page-input]'),value=Number(input.value),pages=result?Math.ceil(result.total/Number($('page-size').value)):0;if(!Number.isSafeInteger(value)||value<1||value>pages){document.querySelector('[data-page-error]').textContent=`请输入 1 至 ${pages} 页。`;input.setAttribute('aria-invalid','true');return;}changePage(value-1);};
document.addEventListener('keydown',event=>{if(event.key==='/'&&!event.ctrlKey&&!event.metaKey&&!event.altKey&&!document.querySelector('dialog[open]')&&!event.target.closest('input,textarea,select,[contenteditable=true]')){event.preventDefault();form.elements.query.focus();}});
window.addEventListener('resize',drawerBounds);if(window.ResizeObserver){const observer=new ResizeObserver(drawerBounds);const header=document.querySelector('.v7-global-header');if(header)observer.observe(header);}
function historyRestore(){clearTimeout(historyTimer);historyTimer=setTimeout(()=>{if((new URLSearchParams(location.search).get('domain')||'')!==shellDomain){location.reload();return;}navigating=false;restoreLocation();readReturn();load();},0);}
window.addEventListener('pagehide',()=>{navigating=true;clearTimeout(timer);clearTimeout(historyTimer);controller?.abort();sequence++;drawerSequence++;policyContext=null;correction=null;});
window.addEventListener('pageshow',event=>{if(event.persisted)historyRestore();});window.addEventListener('popstate',historyRestore);
restoreLocation();readReturn();drawerBounds();load();
})();
