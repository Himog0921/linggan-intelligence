(() => {
'use strict';
const $=id=>document.getElementById(id),form=$('filters'),drawer=$('drawer');
const labels={personal_experience:'亲历自述',professional_output:'知识与方法讲解',explicit_promotion:'推广表达',institution_or_brand:'机构／品牌自述',vertical_tendency:'本领域持续创作',multi_topic:'多主题分享',unknown:'尚不能判定',related:'已判相关',unrelated:'已判无关',yes:'有依据',no:'本次材料未见',monitoring:'巡查已开启',paused:'观察已暂停',dismissed:'已忽略',not_enabled:'已加入 · 巡查未开启'};
const esc=value=>String(value??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const count=value=>value==null?'—':Number(value).toLocaleString('zh-CN');
const number=value=>value==null?'—':`<span title="${esc(count(value))}">${Number(value)>=10000?(Number(value)/10000).toFixed(1)+'万':esc(count(value))}</span>`;
const traitInputs=()=>[...form.querySelectorAll('[name=traits]')];
const criteria={personal_experience:'具体本人或家庭经历、行动、过程或感受；泛建议、第一人称广告与转载不足以支持。',professional_output:'有概念、知识或方法讲解；不认证职业资质。',explicit_promotion:'材料明确介绍产品、服务、课程或购买入口；本次未见不代表没有商业行为。',institution_or_brand:'公开简介明确自述机构或品牌身份；不凭昵称、头像或粉丝数推断。',relevance:'结合当前领域说明、研究目标与这篇作品的可读片段判断。',focus:'持续创作倾向须至少两篇独立相关作品，及引用简介或两篇主页发现作品。多主题分享须有较宽来源材料；有限领域样本不足以判断整个账号。'};
const humanError=code=>({policy_revision_conflict:'设置已被更新，请关闭弹窗刷新后重试',invalid_publication_range:'开始日期须早于结束日期',invalid_publication_date:'请填写有效日期',domain_paused:'当前领域已暂停',invalid_override_support:'支持材料已变化，请刷新后重新选择',observation_lookup_unavailable:'观察状态暂时无法读取',model_budget_exhausted:'本日分析额度已用尽',active_primary_domain_conflict:'其他领域已有主观察，请调整原关系或使用参照用途'}[code]||'操作未完成，请刷新后重试');
let result=null,page=0,controller=null,sequence=0,composing=false,timer,selected=null,drawerSequence=0,policySequence=0,policyContext=null,initialLoad=true,returnScroll=null,returnFocus=null,navigating=false;
const shellDomain=new URLSearchParams(location.search).get('domain')||'';
try{const saved=JSON.parse(sessionStorage.getItem('linggan.creatorDiscovery.return')||'null');if(saved?.url===location.pathname+location.search&&Number.isFinite(saved.scrollY)){returnScroll=saved.scrollY;returnFocus=saved.creatorKey||null;}sessionStorage.removeItem('linggan.creatorDiscovery.return');}catch{sessionStorage.removeItem('linggan.creatorDiscovery.return');}
const main=document.querySelector('.creators-main'),pageNavs=[...document.querySelectorAll('[data-page-nav]')],headerReadout=$('creator-header-readout');
const scrollRoot=()=>$('creator-results');
const scrollPosition=()=>scrollRoot()===window?window.scrollY:scrollRoot().scrollTop;
const restorePosition=value=>{if(scrollRoot()===window)window.scrollTo(0,value);else scrollRoot().scrollTop=value;};
const scrollToResults=()=>{const target=$('creator-results'),root=scrollRoot();if(root===window)window.scrollTo(0,window.scrollY+target.getBoundingClientRect().top);else scrollRoot().scrollTop=0;};
function clearPagination(){for(const nav of pageNavs){nav.querySelector('[data-page-summary]').textContent='';nav.querySelector('[data-page-prev]').disabled=true;nav.querySelector('[data-page-next]').disabled=true;nav.querySelector('[data-page-input]').disabled=true;nav.querySelector('[data-page-jump] button').disabled=true;nav.querySelector('[data-page-input]').value='';nav.querySelector('[data-page-error]').textContent='';nav.querySelector('[data-page-input]').removeAttribute('aria-invalid');}}
function renderPagination(){const totalPages=Math.ceil(result.total/Number($('page-size').value));for(const nav of pageNavs){nav.querySelector('[data-page-summary]').textContent=totalPages?`第 ${page+1} / ${totalPages} 页`:'0 页';nav.querySelector('[data-page-prev]').disabled=!totalPages||page===0;nav.querySelector('[data-page-next]').disabled=!totalPages||page+1>=totalPages;nav.querySelector('[data-page-input]').disabled=!totalPages;nav.querySelector('[data-page-jump] button').disabled=!totalPages;nav.querySelector('[data-page-input]').max=String(totalPages);nav.querySelector('[data-page-input]').value=totalPages?String(page+1):'';nav.querySelector('[data-page-error]').textContent='';nav.querySelector('[data-page-input]').removeAttribute('aria-invalid');}}
const fields=['domain','usageRole','platform','publishedFrom','publishedTo','query','searchMode','traits','topicHint','observation','minLikes','relevance','focus','sort','viral','highLikes'];
function setTraits(values){const selected=new Set((values||'').split(',').filter(Boolean));for(const input of traitInputs())input.checked=selected.has(input.value);}
function restoreLocation(){const saved=new URLSearchParams(location.search);page=/^\d+$/.test(saved.get('page')||'')?Math.min(Number(saved.get('page')),1000000):0;$('page-size').value=['25','50','100'].includes(saved.get('pageSize'))?saved.get('pageSize'):'50';$('window').value=saved.has('publishedFrom')||saved.has('publishedTo')?'custom':'all';for(const name of fields){if(name==='domain')continue;const el=form.elements[name];if(name==='traits')setTraits(saved.get(name));else if(el.type==='checkbox')el.checked=saved.get(name)==='true';else if(saved.has(name))el.value=saved.get(name);else el.value=name==='usageRole'?'primary':name==='platform'?'xhs':name==='searchMode'?'author':name==='sort'?'recent':'';}}
function scope(){const q={};for(const name of fields){const el=form.elements[name];if(name==='traits'){const values=traitInputs().filter(input=>input.checked).map(input=>input.value);if(values.length)q[name]=values.join(',');}else if(el.type==='checkbox'){if(el.checked)q[name]=true;}else if(name==='topicHint'){if(el.value.trim())q[name]=el.value.trim();}else if(el.value)q[name]=el.type==='number'?Number(el.value):el.value;}q.page=page;q.pageSize=Number($('page-size').value);return q;}
function updateScopeSummary(){
 const q=scope(),chips=[];
 if(q.query)chips.push(['query',`${q.searchMode==='work'?'作品内容':'创作者资料'}：${q.query}`]);
 for(const trait of (q.traits||'').split(',').filter(Boolean))chips.push(['traits',labels[trait],trait]);
 const names={topicHint:'内容方向',observation:'观察状态',minLikes:'最低点赞',relevance:'领域关系',focus:'作者倾向',sort:'排序',viral:'有相关爆款',highLikes:'有点赞数可查作品'};
 const observations={outside:'未加入本领域',inside:'已加入本领域',monitoring:'巡查已开启',paused:'观察已暂停',other_domains_only:'仅在其他领域观察',dismissed:'已忽略'};
 for(const key of Object.keys(names))if(q[key]!=null&&q[key]!==''&&q[key]!==false&&!(key==='sort'&&q[key]==='recent'))chips.push([key,key==='viral'||key==='highLikes'?names[key]:`${names[key]}：${key==='observation'?observations[q[key]]:key==='sort'?form.elements.sort.selectedOptions[0]?.textContent:labels[q[key]]||q[key]}`]);
 $('scope-summary').textContent='筛选条件'+(chips.length?` · ${chips.length}`:'');
 $('active-filters').innerHTML=chips.length?`<span class="creator-help">当前条件</span>`+chips.map(([key,text,value])=>`<button type="button" data-remove="${key}" ${value?`data-trait="${value}"`:''} aria-label="移除${esc(text)}">${esc(text)} ×</button>`).join(''):'';
 $('custom-dates').hidden=$('window').value!=='custom';
}
$('active-filters').onclick=e=>{const b=e.target.closest('[data-remove]');if(!b)return;const key=b.dataset.remove;if(key==='traits')setTraits(traitInputs().filter(input=>input.checked&&input.value!==b.dataset.trait).map(input=>input.value).join(','));else if(form.elements[key].type==='checkbox')form.elements[key].checked=false;else form.elements[key].value=key==='sort'?'recent':'';page=0;load(true,true);};
for(const details of document.querySelectorAll('.creator-scope,.creator-definitions'))details.addEventListener('toggle',()=>{if(details.open)for(const other of document.querySelectorAll('.creator-scope,.creator-definitions'))if(other!==details)other.open=false;});
document.addEventListener('click',e=>{for(const details of document.querySelectorAll('.creator-scope,.creator-definitions'))if(!details.contains(e.target))details.open=false;});
document.addEventListener('keydown',e=>{if(e.key==='Escape'&&!drawer.open&&!correctionDialog.open)for(const details of document.querySelectorAll('.creator-scope,.creator-definitions'))if(details.open){details.open=false;details.querySelector('summary').focus();e.preventDefault();}});
function updateHeaderReadout(data){headerReadout.textContent=data?`命中作者 ${count(data.total)} · ${data.displayAsOf?'读取于 '+data.displayAsOf:'读取时间未知'}`:'读数暂不可用';}
function appendStatus(message){const status=$('status');status.textContent+=`${status.textContent?' · ':''}${message}`;}
function params(q){const p=new URLSearchParams();Object.entries(q).forEach(([k,v])=>{if(v!==undefined)p.set(k,String(v));});return p;}
function workLink(item,extra={},work){const q={...scope(),creatorKey:item?.creatorKey,...extra};delete q.page;delete q.pageSize;delete q.observation;delete q.focus;if(q.unknownAuthor){for(const key of ['creatorKey','query','traits','topicHint','relevance','minLikes','viral','highLikes','focus','observation','sort'])delete q[key];q.searchMode='author';}else if(q.searchMode==='author')delete q.query;const p=new URLSearchParams({domain:q.domain,creatorFilter:JSON.stringify(q),returnTo:location.pathname+location.search});if(item)p.set('creatorKey',item.creatorKey);if(work)p.set('work',work);return '/corpus/evidence?'+p;}
async function request(url,options){const r=await fetch(url,options);let body;try{body=await r.json();}catch{throw Error('服务未返回有效数据');}if(!r.ok)throw Error(body.error?.code||body.error||body.code||`请求失败 ${r.status}`);return body;}
async function mutate(url,method,body){return request(url,{method,headers:{'Content-Type':'application/json'},body:JSON.stringify(body)});}
function reset(){for(const n of ['query','topicHint','observation','minLikes','relevance','focus'])form.elements[n].value='';form.elements.sort.value='recent';setTraits('');for(const n of ['viral','highLikes'])form.elements[n].checked=false;page=0;}
async function load(syncUrl=true,historyEntry=false,moveToResults=false,restoreAfter=null,clampRetried=false){if(navigating)return;controller?.abort();const restoreScroll=initialLoad?returnScroll:restoreAfter;initialLoad=false;const current=++sequence;const q=scope();updateScopeSummary();policySequence++;policyContext=null;if($('policy-dialog').open)$('policy-dialog').close();if(correctionDialog.open)correctionDialog.close();correction=null;drawerSequence++;result=null;selected=null;resources.clear();$('rows').replaceChildren();$('stats').replaceChildren();$('coverage').replaceChildren();$('domain-context').replaceChildren();$('result-count').textContent='读取中…';clearPagination();headerReadout.textContent=q.domain?'读取中…':'请选择领域';if(drawer.open)drawer.close();if(!q.domain){$('overview-summary').textContent='材料范围尚未选择';result=null;$('result-count').textContent='尚未选择领域';$('status').textContent='请在领域入口选择研究范围。';return;}controller=new AbortController();const signal=controller.signal;$('status').textContent='读取作者与作品…';$('status').dataset.error='false';if(q.publishedFrom&&q.publishedTo&&q.publishedFrom>=q.publishedTo){$('status').textContent='开始日期须早于结束日期。';$('status').dataset.error='true';$('result-count').textContent='日期范围无效';return;}if(syncUrl){const url='/corpus/creators?'+params(q);if(url!==location.pathname+location.search)history[historyEntry?'pushState':'replaceState'](null,'',url);}try{const data=await request('/api/local/creators?'+params(q),{signal});if(current!==sequence)return;const totalPages=Math.ceil(data.total/q.pageSize),lastPage=Math.max(0,totalPages-1);if(page>lastPage){page=lastPage;history.replaceState(null,'','/corpus/creators?'+params(scope()));if(totalPages){if(clampRetried)throw Error('page_out_of_range');return load(false,false,moveToResults,restoreScroll,true);}}result=data;updateHeaderReadout(data);render();if(moveToResults)requestAnimationFrame(()=>{if(current===sequence)scrollToResults();});const mediaAvailable=await hydrate(data.items.map(item=>item.representatives[0]?.workRef).filter(Boolean),current,q.domain,signal);if(current===sequence){render();if(mediaAvailable===false)appendStatus('媒体暂不可用，可刷新重试');if(restoreScroll!==null)requestAnimationFrame(()=>{restorePosition(restoreScroll);if(returnFocus){const index=result.items.findIndex(item=>item.creatorKey===returnFocus);$('rows').querySelector(`[data-open="${index}"]`)?.focus({preventScroll:true});returnFocus=null;}});}}catch(e){if(e.name==='AbortError'||current!==sequence)return;result=null;updateHeaderReadout(null);$('rows').replaceChildren();$('stats').replaceChildren();$('coverage').replaceChildren();$('domain-context').replaceChildren();$('result-count').textContent='读取中…';clearPagination();if(e.message==='observation_lookup_unavailable'){$('status').textContent='观察关系暂不可用，先清除观察条件查看目录。';const clear=document.createElement('button');clear.type='button';clear.textContent='清除观察条件';clear.onclick=()=>{form.elements.observation.value='';form.elements.viral.checked=false;form.elements.highLikes.checked=false;page=0;load(true,true);};$('status').append(clear);}else if(e.message==='viral_threshold_unset'){$('status').textContent='当前领域未设爆款点赞标准，无法按相关爆款数排序。';const change=document.createElement('button');change.type='button';change.textContent='改为最近收录';change.onclick=()=>{form.elements.sort.value='recent';page=0;load(true,true);};$('status').append(' ',change);}else $('status').textContent='读取失败，请刷新结果重试。';$('result-count').textContent='结果暂不可用';$('status').dataset.error='true';}}
const resources=new Map();
function asset(url){if(typeof url!=='string'||!url.startsWith('/api/local/')||url.startsWith('//'))return null;const u=new URL(url,location.origin);return u.origin===location.origin&&/^\/api\/local\/(media|derivative)\//.test(u.pathname)?u.pathname+u.search:null;}
async function hydrate(refs,current,domain,signal){const unique=[...new Set(refs)].slice(0,100);if(!unique.length)return;try{const data=await request('/api/local/work-resources?'+new URLSearchParams({domain,publicRefs:unique.join(',')}),{signal});if(current!==sequence)return;for(const item of data.items||[])resources.set(item.identity.publicRef,item);}catch(e){if(e.name!=='AbortError'&&current===sequence)return false;}}
function title(w){return resources.get(w.workRef)?.display?.title||w.title||'标题未取得';}
function cover(w){const media=resources.get(w.workRef)?.media;const url=asset(media?.cover?.localAssetUrl);return url?`<img class="creator-cover" src="${esc(url)}" alt="作品封面" loading="lazy">`:'<span class="creator-cover"><svg viewBox="0 0 24 24" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.5"><rect x="3" y="3" width="18" height="18"/><path d="m3 17 6-6 5 5 3-3 4 4"/></svg><span>未取得<br>封面</span></span>';}
function avatar(item){const media=resources.get(item.representatives[0]?.workRef)?.media;const url=media?.avatar?.blob?.deliveryState==='INLINE_SAFE'?asset(media.avatar.localAssetUrl):null;return url?`<img class="creator-avatar" src="${esc(url)}" alt="作者头像" loading="lazy">`:`<span class="creator-avatar" aria-label="头像未取得">${esc((item.displayName||'作者').slice(0,1))}</span>`;}
function analysisLabel(state,code){const known={provider_unavailable:'模型服务暂不可用',model_provider_unavailable:'模型服务暂不可用',budget_exhausted:'本日额度已用尽',model_budget_exhausted:'本日额度已用尽',invalid_result:'结果未通过校验',model_invalid_output:'结果未通过校验',source_unavailable:'材料暂不可用',model_source_unavailable:'材料暂不可用',model_secret_unavailable:'模型密钥不可用',model_not_qualified:'模型尚未通过用途验证',model_disabled:'模型配置未启用',lease_expired:'上次执行超时'};return state==='failed'||state==='paused'?`${state==='failed'?'分析失败':'分析已暂停'} · ${known[code]||'查看处理信息，确认配置后重试'}`:'';}
function operation(analysis,value){const state=analysis?.state;if(state==='running')return '分析中';if(state==='queued')return '排队待分析';if(state==='failed'||state==='paused')return analysisLabel(state,analysis.lastErrorCode);if(!analysis?.resultAt)return result?.policy.analysisEnabled?'尚未取得当前分析结果':'自动分析未开启';return value==='unknown'||!value?'已分析，依据不足':'已有分析结果';}
function retryAllowed(state){return result?.policy.analysisEnabled&&(state==='failed'||state==='paused');}
function preservedState(item){const state=item.observation?.lifecycleState;return state==='paused'||state==='dismissed'?labels[state]:null;}
function targetLink(item){return '/collection/targets?'+new URLSearchParams({domain:scope().domain,drawer:item.observation.targetRef});}
function observed(item){const o=item.observation;if(o?.lookupState==='unavailable'||o?.inCurrentDomain==null)return '观察关系暂不可用';if(o.inCurrentDomain)return `${labels[o.monitoringState]||'观察状态待核对'}${o.usageRole==='reference'?' · 参照':''}`;const state=preservedState(item);if(state)return `${state} · 尚未加入本领域`;return o.existsInOtherDomains?'仅在其他领域观察':'未加入本领域观察';}
function matchReasons(item){const q=scope(),workReasons=[],authorReasons=[];
 if(q.query)(q.searchMode==='work'?workReasons:authorReasons).push(q.searchMode==='work'?`作品内容含“${q.query}”`:`作者资料含“${q.query}”`);
 if(q.publishedFrom&&q.publishedTo)workReasons.push(`发表日期自 ${q.publishedFrom} 至 ${q.publishedTo}（结束日不含）`);
 else if(q.publishedFrom)workReasons.push(`发表日期不早于 ${q.publishedFrom}`);
 else if(q.publishedTo)workReasons.push(`发表日期早于 ${q.publishedTo}`);
 if(q.relevance)workReasons.push(`领域相关判断为${labels[q.relevance]}`);
 if(q.traits)workReasons.push(`具备${q.traits.split(',').map(t=>labels[t]).filter(Boolean).join('或')}来源特征`);
 if(q.topicHint)workReasons.push(`内容方向短语为“${q.topicHint}”`);
 if(q.minLikes!=null)workReasons.push(`点赞不少于 ${count(q.minLikes)}`);
 if(q.viral&&result.policy.likeThreshold!=null)workReasons.push(`相关作品点赞达到本领域标准 ${count(result.policy.likeThreshold)}`);
 if(q.highLikes)workReasons.push('有已知点赞数的相关或待判断作品');
 if(q.focus)authorReasons.push(`作者内容倾向为${labels[q.focus]}`);
 if(q.observation)authorReasons.push(`观察关系为${({outside:'未纳入',inside:'已纳入',monitoring:'巡查中',paused:'已暂停',other_domains_only:'仅其他领域有目标',dismissed:'已忽略'})[q.observation]||q.observation}`);
 return `当前${q.usageRole==='reference'?'参照':'主研究'}用途有 ${item.matchedWorkRefs.length} 篇作品命中${workReasons.length?`：${workReasons.join('；')}`:'当前领域范围'}。${authorReasons.length?`作者同时符合：${authorReasons.join('；')}。`:''}${workReasons.length?'多项作品条件在同一篇作品上核对。':''}`;
}
function sourceKind(work,fragment){if(fragment?.sourceType==='author_profile'||fragment?.field==='biography')return '公开账号简介';return work?.acquisitionKind==='profile_discovery'?'主页发现作品':work?.acquisitionKind==='limited_domain_sample'?'本领域已有作品':'已有作品材料';}
function sourceField(field){return ({biography:'公开简介',title:'标题',body:'正文',ocr:'图文识别',transcript:'转写'})[field]||'材料片段';}
function evidenceWorkRef(fragment){const ref=fragment?.workRef;return fragment?.sourceType==='work_material'&&typeof ref==='string'&&/^[0-9a-f]{8}-[0-9a-f-]{27,}$/i.test(ref)?ref:null;}
function evidenceBlock(fragment,work){const ref=evidenceWorkRef(fragment),link=ref?` · <a href="${esc(workLink(selected,{},ref))}">回查作品</a>`:'';return `<blockquote>${esc(fragment.text||'片段暂不可读')}<small>${esc(sourceKind(work,fragment))} · ${esc(sourceField(fragment.field))}${link}</small></blockquote>`;}
function analysisTime(analysis){return analysis?.resultAtDisplay?`自动依据更新于 ${analysis.resultAtDisplay}`:'自动依据更新时间未知';}
function citedEvidence(analysis,fields){const evidence=[...(analysis?.automatic?.evidence||analysis?.evidence||[])],ids=new Set(fields.flatMap(field=>analysis?.manual?.[field]?.supportFragmentIds||[]));for(const fragment of analysis?.supportFragments||[])if(ids.has(fragment.fragmentId)&&!evidence.some(item=>item.fragmentId===fragment.fragmentId))evidence.push(fragment);return evidence;}
function manualTime(analysis,field){const at=analysis?.manual?.[field]?.at,date=at?new Date(at):null;return date&&!Number.isNaN(date.getTime())?`人工纠正于 ${new Intl.DateTimeFormat('zh-CN',{timeZone:'Asia/Shanghai',year:'numeric',month:'2-digit',day:'2-digit',hour:'2-digit',minute:'2-digit',hour12:false}).format(date)}`:null;}
function observationAction(item,index){
 const o=item.observation,kept=preservedState(item),paused=result.domain.status==='paused',target=o.targetRef?`<a href="${esc(targetLink(item))}">${kept?'查看目标及恢复':'查看观察目标'}</a>`:'';
 if(o.inCurrentDomain)return target;
 if(o.inCurrentDomain==null)return '<button disabled>观察关系暂不可用</button>';
 const role=scope().usageRole||'primary';
 const blocked=role==='primary'&&o.hasOtherPrimaryDomain;
 const button=(selectedRole,label,disabled=false)=>`<button data-add="${index}" data-role="${selectedRole}" ${disabled||paused?'disabled':''}>${label}</button>`;
 const choice=blocked?`${button('primary','主研究需先调整原领域',true)}${button('reference','按参照用途加入')}<small>其他领域已有主观察（可能已暂停）；参照用途不接收主研究巡查，也可先在观察目标管理中调整原关系。</small>`:button(role,kept?`加入本领域（保持${kept} · ${role==='reference'?'参照':'主研究'}）`:role==='reference'?'按参照用途加入':'加入本领域观察');
 return choice+target;
}
function contentClue(item,index){const topics=(item.topicHints||[]).filter(Boolean),tags=(item.traits||[]).map(value=>labels[value]).filter(Boolean);return `<button class="creator-link creator-clue" data-open="${index}" aria-label="查看${esc(item.displayName||'此作者')}的判断依据"><span class="creator-topics" title="${esc(topics.join(' · '))}">${esc(topics.join(' · ')||'内容方向尚未形成')}</span>${tags.length?`<span class="creator-tags">${tags.map(tag=>`<span class="creator-tag">${esc(tag)}</span>`).join('')}</span>`:''}<span class="creator-focus">作者倾向：${esc(labels[item.focus?.value]||'尚不能判定')}</span>${!item.focus?.value||item.focus.value==='unknown'?`<small>${esc(operation(item.authorAnalysis,item.focus?.value))}</small>`:''}</button>`;}
function updateQuickState(){const q=scope(),other=['query','topicHint','minLikes','relevance','focus','traits'].some(key=>q[key]);const active={all:!other&&!q.observation&&!q.viral&&!q.highLikes&&q.sort==='recent',outside:!other&&q.observation==='outside'&&!q.viral&&!q.highLikes&&q.sort==='recent',high:!other&&q.observation==='outside'&&(q.viral||q.highLikes)};for(const button of $('quick').querySelectorAll('[data-quick]'))button.setAttribute('aria-pressed',String(Boolean(active[button.dataset.quick])));}
function render(){
 const s=result.stats,hasRule=result.policy.likeThreshold!=null,outsideAvailable=s.outside!=null;
 const stats=[['all','范围内创作者',s.authors],['vertical','本领域持续创作',s.vertical],['personal','有亲历自述作品',s.personal],['outside','未加入本领域观察',s.outside],['high',hasRule?'未观察的相关爆款作者':'未观察的高赞线索',hasRule?s.outsideViral:outsideAvailable?'查看':null]];
 $('stats').innerHTML=stats.map(([key,title,n])=>`<button type="button" data-stat="${key}" title="只查看此项，清除其他搜索与筛选" ${((key==='outside'||key==='high')&&!outsideAvailable)?'disabled':''}>${title}<strong>${n==null?'—':typeof n==='number'?number(n):esc(n)}</strong></button>`).join('')+`<p class="creator-stats-note">统计按当前领域、用途和发表日期计算，不随下方搜索与筛选改变。${hasRule?`相关爆款标准：点赞 ≥ ${count(result.policy.likeThreshold)}。`:'尚未设置爆款标准，高赞线索仅按已有点赞排序。'}</p>`;
 $('overview-summary').textContent=`材料范围：${count(s.authors)} 位创作者 · ${count(s.works)} 篇作品 · 自动分析${result.policy.analysisEnabled?'已开启':'未开启'}`;
 $('domain-context').innerHTML=`<p><strong>${esc(result.domain.name||'当前领域')}</strong> · ${esc(result.domain.description||'尚未填写领域说明')}<br>研究目标：${esc(result.domain.researchGoal||'尚未填写，当前仅有领域名称作为上下文')}。</p><p>作品判断逐篇进行；作者判断最多抽取20篇已有作品摘要，并受材料与输入额度限制，不要求全账号作品分析完。${result.policy.analysisEnabled?'排队进度以各作者的处理状态为准。':'自动分析未开启，页面只展示已有结果；浏览与切换领域不会启动分析。'}</p>`;
 for(const key of ['outside','high'])document.querySelector(`[data-quick=${key}]`).disabled=!outsideAvailable;
 document.querySelector('[data-quick=high]').textContent=hasRule?'未观察的爆款作者':'未观察的高赞作者';
 form.elements.viral.disabled=!hasRule;form.elements.observation.disabled=!outsideAvailable;form.elements.sort.querySelector('[value=viral_works]').disabled=!hasRule;
 $('result-count').textContent=`当前结果 ${count(result.total)} 位创作者`;
 $('status').textContent=result.total===0?'没有符合当前条件的创作者，移除部分条件后重试。':!result.policy.analysisEnabled?'自动分析未开启，仅展示已有判断。':'选择作者查看判断依据。';
 $('coverage').innerHTML=`已判相关 ${count(s.relatedAuthors)} 位作者 · <a href="${esc(workLink(null,{unknownAuthor:true}))}">作品作者资料待补 ${count(s.unknownAuthorWorks)} 篇</a>${(scope().publishedFrom||scope().publishedTo)&&s.unknownDateWorks?` · <button id="unknown-dates">另有 ${count(s.unknownDateWorks)} 篇发表日期未知，查看全部日期</button>`:''}`;
 $('topic-options').innerHTML=[...new Set(result.items.flatMap(item=>item.topicHints||[]))].map(hint=>`<option value="${esc(hint)}"></option>`).join('');
 updateScopeSummary();updateQuickState();
 $('rows').innerHTML=result.items.map((item,index)=>{const w=item.representatives[0];return `<tr><td><div class="creator-author-cell">${avatar(item)}<div class="creator-cell-copy"><button class="creator-link creator-name" data-open="${index}" title="${esc(item.displayName||'作者名称未取得')}">${esc(item.displayName||'作者名称未取得')}</button><small>粉丝 ${number(item.profile?.followerCount)}</small><small>范围内 ${number(item.collectedWorkCount)} 篇作品</small></div></div></td><td>${contentClue(item,index)}</td><td><a class="creator-metric" href="${esc(workLink(item,{relevance:'related',traits:undefined,topicHint:undefined,viral:false,highLikes:false,minLikes:undefined,query:undefined}))}">${number(item.relatedWorkCount)} 篇已判相关</a><small><a href="${esc(workLink(item,{relevance:'unknown',traits:undefined,topicHint:undefined,viral:false,highLikes:false,minLikes:undefined,query:undefined}))}">待判断 ${number(item.unknownRelevanceWorkCount)} 篇</a></small><small>相关作品最高赞 ${number(item.maxRelatedLikeCount)}</small>${hasRule?`<small>相关爆款 ${number(item.viralWorkCount)} 篇</small>`:''}</td><td>${w?`<div class="creator-work-cell">${cover(w)}<div class="creator-cell-copy"><a class="creator-work-title" href="${esc(workLink(item,{},w.workRef))}" title="${esc(title(w))}">${esc(title(w))}</a><small>赞 ${number(w.likes)} · 藏 ${number(w.collects)} · 评 ${number(w.comments)}</small><small>本页仅预览命中作品</small></div></div>`:'暂无可预览作品'}</td><td><span class="creator-observation-state">${esc(observed(item))}</span><div class="creator-row-action">${observationAction(item,index)}</div></td></tr>`;}).join('')||'<tr><td colspan="5" class="creator-empty">没有符合当前条件的创作者。当前条件仍显示在上方，可逐项移除。</td></tr>';
 renderPagination();
}
function quick(key){if((key==='outside'||key==='high')&&result?.stats.outside==null)return;reset();if(key==='outside'||key==='high')form.elements.observation.value='outside';if(key==='high'){form.elements.viral.checked=result.policy.likeThreshold!=null;form.elements.highLikes.checked=result.policy.likeThreshold==null;form.elements.sort.value=result.policy.likeThreshold!=null?'viral_works':'high_likes';}if(key==='personal')setTraits('personal_experience');if(key==='vertical')form.elements.focus.value='vertical_tendency';page=0;load(true,true);}
$('stats').addEventListener('click',e=>{const b=e.target.closest('[data-stat]');if(b)quick(b.dataset.stat);});$('quick').addEventListener('click',e=>{const b=e.target.closest('[data-quick]');if(b)quick(b.dataset.quick);});
function effective(analysis,field){return analysis?.manual?.[field]?.value||(field==='relevance'?analysis?.automatic?.relevance:field==='focus'?analysis?.automatic?.focus:field==='institution_or_brand'?analysis?.institution_or_brand:analysis?.automatic?.traits?.[field]);}
function judgment(analysis,field,work){
 const value=effective(analysis,field)||{value:'unknown'},ids=new Set(analysis?.manual?.[field]?.supportFragmentIds||value.evidenceFragmentIds||[]),fragments=citedEvidence(analysis,[field]).filter(fragment=>ids.has(fragment.fragmentId));
 const name=field==='focus'?'作者内容倾向':field==='relevance'?'与当前领域的关系':labels[field];
 return `<div class="creator-judgment"><div class="creator-judgment-head"><span>${name}</span><strong>${esc(labels[value.value]||'尚不能判定')}</strong><button class="creator-adjust" data-correct="${field}">${value.value==='unknown'?'补充判断':'调整判断'}</button></div>${value.reason?`<p>${esc(value.reason)}</p>`:''}${manualTime(analysis,field)?`<small>${esc(manualTime(analysis,field))} · 当前使用人工判断</small>`:''}${fragments.length?`<details><summary>支持依据 · ${fragments.length} 个片段</summary>${fragments.map(fragment=>evidenceBlock(fragment,work)).join('')}</details>`:''}</div>`;
}
async function open(index){
 const item=result?.items[index];if(!item)return;
 const current=sequence,draw=++drawerSequence,domain=scope().domain,signal=controller.signal;
 const mediaAvailable=await hydrate(item.representatives.map(w=>w.workRef),current,domain,signal);
 if(current!==sequence||draw!==drawerSequence)return;
 if(mediaAvailable===false)appendStatus('媒体暂不可用，可刷新重试');
 selected=item;
 $('drawer-title').textContent=selected.displayName||'作者名称未取得';
 const author=selected.authorAnalysis||{},auto=author.automatic||{},sampleBasis=auto.sampleBasis==='profile_discovery_sample'?'包含主页发现作品的样本':auto.sampleBasis==='limited_domain_sample'?'本领域已有作品的有限样本':'材料范围尚未记录';
 const works=selected.representatives.map((w,position)=>`<details class="creator-work" data-work="${esc(w.workRef)}" ${position===0?'open':''}><summary>${esc(title(w))}</summary><p class="creator-work-meta">${esc(sourceKind(w))} · ${esc(w.relevance==='unknown'?'领域关系待判断':labels[w.relevance])} · 赞 ${number(w.likes)} · 藏 ${number(w.collects)} · 评 ${number(w.comments)}</p><a href="${esc(workLink(selected,{},w.workRef))}">打开这篇作品</a><div>${['relevance','personal_experience','professional_output','explicit_promotion'].map(field=>judgment(w.analysis,field,w)).join('')}</div><small>${esc(operation(w.analysis,w.relevance))} · ${esc(analysisTime(w.analysis))}</small>${retryAllowed(w.analysis?.state)?'<button data-retry>重试这篇作品的失败分析</button>':''}<details><summary>作品处理信息</summary><small>收录于 ${esc(w.firstAddedDisplay||'时间未知')} · 点赞观察于 ${esc(w.likesObservedDisplay||'时间未知')}</small><small>处理原因：${esc(analysisLabel(w.analysis?.state,w.analysis?.lastErrorCode)||'无失败记录')}</small></details></details>`).join('');
 $('drawer-body').innerHTML=`<section><p><strong>${esc(result.domain.name||'当前领域')} · ${scope().usageRole==='reference'?'参照材料':'主研究材料'}</strong> · 小红书 · 粉丝 ${number(selected.profile?.followerCount)}</p><p>范围内 ${number(selected.collectedWorkCount)} 篇作品：已判相关 ${number(selected.relatedWorkCount)} 篇，待判断 ${number(selected.unknownRelevanceWorkCount)} 篇，已判无关 ${number(selected.unrelatedWorkCount)} 篇。</p><details><summary>为什么出现在当前结果 · 命中 ${selected.matchedWorkRefs.length} 篇</summary><p>${esc(matchReasons(selected))}</p></details><a href="${esc(workLink(selected))}">查看全部命中作品</a></section><section><h3>作者在本领域的判断</h3>${judgment(author,'focus')}${judgment(author,'institution_or_brand')}<small>${esc(operation(author,selected.focus?.value))} · ${esc(analysisTime(author))}</small>${retryAllowed(author.state)?'<button data-retry-author>重试作者的失败分析</button>':''}<details><summary>判断标准与材料范围</summary><p>${esc(criteria.focus)}</p><p>${esc(sampleBasis)} · ${auto.sampleWorkCount==null?'本次取样篇数未记录':`本次使用 ${count(auto.sampleWorkCount)} 篇作品摘要`}${auto.availableWorkCount==null?'':` / 当时已有 ${count(auto.availableWorkCount)} 篇`}。</p>${(auto.sampleLimitations||['仅使用当前领域已有材料，不代表账号全部作品']).map(text=>`<p>${esc(text)}</p>`).join('')}<p>亲历自述属于作品表达，可以与知识讲解、推广或持续创作倾向同时成立。</p></details></section><section><h3>作品表达与支持依据</h3><p class="creator-help">预览当前条件下的 ${selected.representatives.length} 篇作品，展开判断可回查原文片段。</p>${works}</section><section><details><summary>公开资料与处理信息</summary><p>公开简介：${esc(selected.profile?.biography||'尚未取得')}</p><p>来源账号标识：${esc(selected.authorExternalId||'尚未取得')}</p><p>自动分析：${result.policy.analysisEnabled?'本领域已开启':'本领域未开启'}。${result.domain.status==='paused'?'领域已暂停，不能修改判断或观察关系。':''}</p><p>领域说明：${esc(result.domain.description||'尚未填写')}<br>研究目标：${esc(result.domain.researchGoal||'尚未填写')}</p></details></section>`;
 $('drawer-footer').innerHTML=`<div class="creator-controls"><span>${esc(observed(selected))}</span><div>${observationAction(selected,index)}</div></div><p id="drawer-action-status" role="status"></p>`;
 if(result.domain.status!=='active')for(const button of drawer.querySelectorAll('#drawer-body button,#drawer-footer button')){button.disabled=true;button.title='当前领域已暂停，不能修改';}
 if(!drawer.open)drawer.showModal();$('drawer-body').scrollTop=0;
}
document.addEventListener('click',e=>{const link=e.target.closest('a[href^="/corpus/evidence?"]');if(link)sessionStorage.setItem('linggan.creatorDiscovery.return',JSON.stringify({url:location.pathname+location.search,scrollY:scrollPosition(),creatorKey:new URL(link.href).searchParams.get('creatorKey')}));});
async function addObservation(button){
 if(!button||button.disabled||!result||result.domain.domainRef!==scope().domain)return;
 const item=result.items[Number(button.dataset.add)],role=button.dataset.role;
 if(!item)return;
 const current=sequence,draw=drawerSequence,wasOpen=drawer.open;
 const isCurrent=()=>!navigating&&current===sequence&&draw===drawerSequence&&drawer.open===wasOpen&&button.isConnected;
 button.disabled=true;
 try{
  const receipt=await mutate(`/api/local/creators/${encodeURIComponent(item.creatorKey)}/observation-target`,'POST',{domain:scope().domain,usageRole:role});
  if(!isCurrent())return;
  button.textContent='已加入';item.observation.inCurrentDomain=true;item.observation.targetRef=receipt.targetRef;
  await load();
  if(navigating||sequence!==current+1)return;
  const kept=receipt.lifecycleState==='paused'||receipt.lifecycleState==='dismissed'?labels[receipt.lifecycleState]:null;
  appendStatus(`已加入本领域${receipt.usageRole==='reference'?'参照用途':'观察目标'}${kept?`，目标保持${kept}`:''}`);
  if(kept){const link=document.createElement('a');link.href=targetLink(item);link.textContent='查看目标及恢复';$('status').append(' · ',link);}$('status').setAttribute('tabindex','-1');$('status').focus();
 }catch(err){if(isCurrent()){button.disabled=false;const status=drawer.contains(button)?$('drawer-action-status'):$('status');status.textContent='加入失败：'+humanError(err.message);}}
}
$('rows').addEventListener('click',e=>{const openButton=e.target.closest('[data-open]');if(openButton){open(Number(openButton.dataset.open));return;}const button=e.target.closest('[data-add]');if(button)addObservation(button);});
$('drawer-close').onclick=()=>drawer.close();
drawer.addEventListener('close',()=>{if(selected&&result){const index=result.items.findIndex(item=>item.creatorKey===selected.creatorKey);$('rows').querySelector(`[data-open="${index}"]`)?.focus();}});
const correctionDialog=$('correction-dialog');
let correction=null;
const correctionRequests=new WeakSet(),policyRequests=new WeakSet();
function openCorrection(field,workRef){
 const choices=field==='relevance'?[['related','与当前领域相关'],['unrelated','与当前领域无关'],['unknown','尚不能判定']]:field==='focus'?[['vertical_tendency','本领域持续创作'],['multi_topic','多主题分享'],['unknown','尚不能判定']]:[['yes','材料支持此表达／身份'],['no','本次材料未见'],['unknown','尚不能判定']];
 const authorField=field==='focus'||field==='institution_or_brand';
 const work=authorField?null:selected.representatives.find(w=>w.workRef===workRef);
 if(!authorField&&!work)return;
 const manual=authorField?selected.authorAnalysis?.manual?.[field]:work.analysis.manual?.[field];
 const automatic=authorField?(field==='focus'?selected.focus:selected.authorAnalysis?.institution_or_brand):(field==='relevance'?work.analysis.automatic?.relevance:work.analysis.automatic?.traits?.[field]);
 const current=manual?.value?.value||automatic?.value||'unknown';
 correction={domain:scope().domain,creatorKey:selected.creatorKey,workRef:work?.workRef||null,field};
 const supports=authorField?(field==='institution_or_brand'?selected.authorAnalysis?.identitySupportFragments:selected.authorAnalysis?.supportFragments):work.analysis.supportFragments;
 const existing=new Set(manual?.supportFragmentIds||[]);
 $('support-options').replaceChildren();
 const groups=new Map();for(const fragment of supports||[]){const key=fragment.field==='biography'?'profile':fragment.workRef||'work';if(!groups.has(key))groups.set(key,[]);groups.get(key).push(fragment);}
 for(const [key,fragments] of groups){const details=document.createElement('details'),summary=document.createElement('summary');const sourceWork=selected.representatives.find(item=>item.workRef===key);summary.textContent=(key==='profile'?'公开简介':fragments[0].workTitle||sourceWork?.title||'已有作品')+` · ${fragments.length} 个片段`;details.append(summary);details.open=fragments.some(fragment=>existing.has(fragment.fragmentId))||key==='profile';
 for(const fragment of fragments){const label=document.createElement('label'),checkbox=document.createElement('input'),text=document.createElement('span'),kind=document.createElement('small');checkbox.type='checkbox';checkbox.value=fragment.fragmentId;checkbox.checked=existing.has(fragment.fragmentId);kind.textContent=sourceField(fragment.field);text.append(kind,document.createTextNode(fragment.text||'片段不可读'));label.append(checkbox,text);details.append(label);} $('support-options').append(details);}
 $('correction-support').hidden=!(supports||[]).length;
 $('correction-title').textContent=(current==='unknown'?'补充':'调整')+(field==='focus'?'作者内容倾向':field==='relevance'?'作品领域关系':labels[field]);
 $('correction-context').textContent=`${authorField?'创作者':'作品'}：${authorField?selected.displayName||'作者名称未取得':title(work)} · ${result.domain.name} · 仅修改本项判断`;
 $('correction-criteria').textContent='判定依据：'+criteria[field];
 const select=$('correction-value');select.replaceChildren();
 for(const [value,label] of choices){const option=document.createElement('option');option.value=value;option.textContent=label;select.append(option);}
 select.value=current;$('correction-reason').value=manual?.value?.reason||'';
 $('correction-restore').disabled=!manual;
 $('correction-status').textContent='';$('correction-status').dataset.error='false';
 correctionDialog.showModal();correctionDialog.querySelector('.creator-dialog-scroll').scrollTop=0;
}
async function saveCorrection(value){
 const context=correction,current=sequence,draw=drawerSequence;
 if(!context||correctionRequests.has(context))return;
 correctionRequests.add(context);
 const isCurrent=()=>!navigating&&current===sequence&&draw===drawerSequence&&correction===context&&correctionDialog.open;
 const status=$('correction-status');status.textContent='保存中…';status.dataset.error='false';
 try{
  const supportFragmentIds=[...$('support-options').querySelectorAll('input:checked')].map(el=>el.value);
  const positive=value!==null&&((correction.field==='relevance'&&value==='related')||(correction.field==='focus'&&value!=='unknown')||(correction.field!=='relevance'&&correction.field!=='focus'&&value==='yes'));
  if(positive&&!supportFragmentIds.length){status.textContent='请选择至少一项当前材料作为支持依据。';status.dataset.error='true';return;}
  await mutate('/api/local/creator-discovery-overrides','PUT',{...context,value,reason:value===null?null:$('correction-reason').value.trim()||null,supportFragmentIds});
  if(!isCurrent())return;
  correctionDialog.close();drawer.close();selected=null;await load();
 }catch(err){if(isCurrent()){status.textContent='保存失败：'+humanError(err.message);status.dataset.error='true';}}
 finally{correctionRequests.delete(context);}
}
$('correction-close').onclick=()=>correctionDialog.close();
$('correction-restore').onclick=()=>saveCorrection(null);
$('correction-form').onsubmit=e=>{e.preventDefault();saveCorrection($('correction-value').value);};
drawer.addEventListener('click',async e=>{
 const target=e.target.closest('button');if(!target||!selected)return;
 if(target.hasAttribute('data-add')){addObservation(target);return;}
 const work=target.closest('[data-work]')?.dataset.work;
 const field=target.dataset.correct||(target.hasAttribute('data-focus')?'focus':target.hasAttribute('data-identity')?'institution_or_brand':null);
 if(field){openCorrection(field,work);return;}
 if(!target.hasAttribute('data-retry')&&!target.hasAttribute('data-retry-author'))return;
 target.disabled=true;
 const status=$('drawer-action-status'),current=sequence,draw=drawerSequence,item=selected;
 const isCurrent=()=>!navigating&&current===sequence&&draw===drawerSequence&&selected===item&&drawer.open;
 try{
  const body={domain:scope().domain};
  if(target.hasAttribute('data-retry-author'))body.creatorKey=selected.creatorKey;else body.workRef=work;
  const receipt=await mutate('/api/local/creator-discovery-analysis/retry','POST',body);
  if(!isCurrent())return;
  status.textContent=receipt.queued?'已排队，符合配置及额度后执行':'当前没有可重试的失败分析';
 }catch(err){if(isCurrent()){status.textContent='重试失败：'+humanError(err.message);target.disabled=false;}}
});
form.addEventListener('submit',e=>{e.preventDefault();page=0;load(true,true);});form.addEventListener('change',e=>{if(e.target.id==='window'||['query','topicHint'].includes(e.target.name))return;if(e.target.name==='searchMode')form.elements.query.placeholder=e.target.value==='work'?'标题、正文或内容方向':'昵称、平台账号、公开简介';page=0;load(true,true);});for(const name of ['query','topicHint']){const input=form.elements[name];input.addEventListener('compositionstart',()=>{composing=true;clearTimeout(timer);});input.addEventListener('compositionend',()=>{composing=false;clearTimeout(timer);timer=setTimeout(()=>{page=0;load(true,true);},250);});input.addEventListener('input',()=>{if(composing)return;clearTimeout(timer);timer=setTimeout(()=>{page=0;load(true,true);},250);});}

$('reset').onclick=()=>{reset();load(true,true);};$('refresh').onclick=()=>load();for(const nav of pageNavs){nav.querySelector('[data-page-prev]').onclick=()=>changePage(page-1);nav.querySelector('[data-page-next]').onclick=()=>changePage(page+1);nav.querySelector('[data-page-jump]').onsubmit=e=>{e.preventDefault();const input=nav.querySelector('[data-page-input]'),target=Number(input.value),totalPages=Math.ceil((result?.total||0)/Number($('page-size').value));if(!Number.isSafeInteger(target)||target<1||target>totalPages){nav.querySelector('[data-page-error]').textContent=`请输入 1 至 ${totalPages} 页。`;input.setAttribute('aria-invalid','true');return;}input.removeAttribute('aria-invalid');nav.querySelector('[data-page-error]').textContent='';changePage(target-1);};}function changePage(target){const totalPages=Math.ceil((result?.total||0)/Number($('page-size').value));if(!result||target<0||target>=totalPages||target===page)return;page=target;load(true,true,true);}$('page-size').onchange=()=>{page=0;load(true,true,true);};$('coverage').onclick=e=>{if(e.target.id==='unknown-dates'){form.elements.publishedFrom.value='';form.elements.publishedTo.value='';$('window').value='all';page=0;load(true,true);}};
$('window').addEventListener('change',()=>{const days=Number($('window').value);if($('window').value==='custom'){updateScopeSummary();form.elements.publishedFrom.focus();return;}const now=new Date();const shanghai=new Intl.DateTimeFormat('en-CA',{timeZone:'Asia/Shanghai',year:'numeric',month:'2-digit',day:'2-digit'}).format(now);const start=new Date(`${shanghai}T00:00:00+08:00`);const end=new Date(start);end.setUTCDate(end.getUTCDate()+1);start.setUTCDate(start.getUTCDate()-(days-1));const date=d=>new Intl.DateTimeFormat('en-CA',{timeZone:'Asia/Shanghai',year:'numeric',month:'2-digit',day:'2-digit'}).format(d);form.elements.publishedFrom.value=days?date(start):'';form.elements.publishedTo.value=days?date(end):'';page=0;load(true,true);});
$('policy-open').onclick=async()=>{
 if(!result||result.domain.domainRef!==scope().domain)return;const currentPolicy=++policySequence,domain=scope().domain;policyContext={domain,revision:result.policy.revision};
 $('threshold').value=result.policy.likeThreshold??'';
 $('analysis-enabled').checked=result.policy.analysisEnabled;
 $('daily-token-limit').value=result.policy.dailyTokenLimit??'';
 $('policy-status').textContent=result.domain.status==='paused'?'当前领域已暂停，不能修改设置。':'';
 $('policy-form').querySelector('[type=submit]').disabled=result.domain.status==='paused';
 $('model-config').replaceChildren();$('model-config').disabled=true;$('model-config').value='';
 $('policy-dialog').showModal();
 try{
  const settings=await request('/api/local/model-settings');
  if(currentPolicy!==policySequence||domain!==scope().domain||policyContext?.domain!==domain)return;
  const configs=settings.config?[settings.config]:[];
  $('model-config').replaceChildren();
  const empty=document.createElement('option');empty.value='';empty.textContent='选择模型配置';$('model-config').append(empty);
  for(const config of configs){const option=document.createElement('option');option.value=config.configRef;option.textContent=config.modelId||config.configRef;$('model-config').append(option);}
  if(result.policy.configRef&&!configs.some(config=>config.configRef===result.policy.configRef)){const current=document.createElement('option');current.value=result.policy.configRef;current.textContent='当前来源分析配置';$('model-config').append(current);}
  $('model-config').disabled=false;$('model-config').value=result.policy.configRef||'';
 }catch(err){if(currentPolicy!==policySequence||domain!==scope().domain||policyContext?.domain!==domain)return;$('model-config').replaceChildren();$('model-config').disabled=true;$('model-config').value='';$('policy-status').textContent='模型配置暂时无法读取。可保存点赞标准，分析设置保持原值。';}
};
$('policy-close').onclick=()=>{policySequence++;policyContext=null;$('policy-dialog').close();};
$('policy-form').onsubmit=async e=>{
 e.preventDefault();
 const status=$('policy-status');
 const context=policyContext;if(!context||!result||context.domain!==scope().domain||context.domain!==result.domain.domainRef||context.revision!==result.policy.revision){status.textContent='领域或设置已变化，请重新打开设置。';status.dataset.error='true';return;}
 if(policyRequests.has(context))return;
 status.dataset.error='false';status.textContent='';
 const current=sequence,currentPolicy=policySequence;
 const isCurrent=()=>!navigating&&current===sequence&&currentPolicy===policySequence&&policyContext===context&&$('policy-dialog').open;
 const settingsAvailable=!$('model-config').disabled;
 const enabled=$('analysis-enabled').checked,limit=$('daily-token-limit').value?Number($('daily-token-limit').value):null,config=settingsAvailable?$('model-config').value||null:null;
 if(settingsAvailable&&enabled&&!config){status.textContent='启用分析前请选择模型配置。';status.dataset.error='true';return;}
 if(settingsAvailable&&enabled&&!limit){status.textContent='启用分析前请填写每日词元上限。';status.dataset.error='true';return;}
 policyRequests.add(context);status.textContent='保存中…';
 try{
  const payload={domain:context.domain,revision:context.revision,likeThreshold:$('threshold').value?Number($('threshold').value):null};
  if(settingsAvailable)Object.assign(payload,{analysisEnabled:enabled,configRef:config,dailyTokenLimit:limit});
  await mutate('/api/local/creator-discovery-policy','PUT',payload);
  if(!isCurrent())return;
  $('policy-dialog').close();await load();
 }catch(err){if(isCurrent()){status.textContent=err.message==='policy_revision_conflict'?'设置已被更新，请关闭弹窗刷新后重试。':'保存失败：'+humanError(err.message);status.dataset.error='true';}}
 finally{policyRequests.delete(context);}
};
let historyRestoreTimer,historyReload=false,historyPopstate=false,historyReloading=false;
function scheduleHistoryRestore(reload=false,popstate=false){if(historyReloading)return;historyReload ||=reload;historyPopstate ||=popstate;clearTimeout(historyRestoreTimer);historyRestoreTimer=setTimeout(()=>{const shouldReload=historyReload,keepUrl=historyPopstate;historyReload=false;historyPopstate=false;historyRestoreTimer=null;if(shouldReload||(new URLSearchParams(location.search).get('domain')||'')!==shellDomain){historyReloading=true;location.reload();return;}navigating=false;restoreLocation();load(!keepUrl);},0);}
window.addEventListener('pagehide',()=>{navigating=true;controller?.abort();sequence++;});window.addEventListener('pageshow',e=>scheduleHistoryRestore(e.persisted));window.addEventListener('popstate',()=>scheduleHistoryRestore(false,true));
})();
