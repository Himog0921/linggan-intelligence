(() => {
  'use strict';
  const root = document.getElementById('topic-map-root');
  if (!root) return;
  const content = document.createElement('div'); content.className = 'lgi-tm-content'; root.replaceChildren(content);
  const esc = value => String(value ?? '').replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
  const list = value => Array.isArray(value) ? value : [];
  const known = value => typeof value === 'number' && Number.isFinite(value);
  const number = value => known(value) ? value.toLocaleString('zh-CN') : '—';
  const pct = (n, d) => known(n) && known(d) && d > 0 ? `${(100 * n / d).toFixed(1)}%` : '—';
  const uuid = () => crypto.randomUUID();
  const stageNames = {discover_understand:'发现与理解',seek_assessment:'求助与评估',choose_support:'选择支持',begin_practice:'开始实践',long_term_manage:'长期管理'};
  const fallbackStages = Object.entries(stageNames).map(([stage, label]) => ({stage, label}));
  const otherNames = {cross_stage:'跨阶段综合',general_background:'通用背景',unclear:'已分析但阶段不明',pending:'待分析 / 材料不足'};
  const views = [['structure','结构与表现'],['patterns','内容格局'],['sources','来源分布'],['candidates','新方向'],['action','我方对照与再做']];
  const defaults = {domainRef:'',topicRef:'',platform:'',windowDays:'',referenceWindowDays:'30',view:'overview',tab:'structure',path:'',overlay:'',stage:'',metric:'main',mapMode:'map',materialMode:'latest',summaryChart:'table',plotPlatform:'',changesOnly:false,query:'',collapsed:[],compare:[]};
  let restored = {};
  try { restored = JSON.parse(sessionStorage.getItem('linggan.topic-map.view') || '{}'); } catch (_) {}
  const state = {...defaults, ...restored};
  if (!views.some(([id]) => id === state.tab)) state.tab = 'structure';
  if (!['overview','journey'].includes(state.view)) state.view = 'overview';
  const params = new URLSearchParams(location.search);
  // The shared picker links only the new domain; former scope must not follow it.
  const incomingDomain = params.get('domain') ?? params.get('domainRef');
  if (params.has('domain') || (incomingDomain !== null && incomingDomain !== state.domainRef)) Object.assign(state, defaults, {domainRef:incomingDomain});
  ['domainRef','topicRef','platform','windowDays','referenceWindowDays'].forEach(key => { if (params.has(key)) state[key] = params.get(key); });
  const extraWorks = new Map();
  let snapshot = null, progress = null, loading = false, readError = '', notice = '', pending = false;
  let dialog = null, dialogState = null, dialogStack = [], triggerElement = null, fetchEpoch = 0;
  const title = work => work.title || '标题尚未取得';
  const readableMaterial = w => Boolean(w?.readable || w?.research?.readable);
  const commentOnlyResearch = w => w?.research?.readable === true && w.research.authorSourceState === 'source_unavailable' && w.research.commentSourceState === 'available';
  const authorUnavailable = w => w?.readable === false || w?.research?.authorSourceState === 'source_unavailable';
  const readLabel = (w, label = '核对原文与讨论') => authorUnavailable(w) ? commentOnlyResearch(w) ? '查看已有评论与讨论' : '核对当前可用材料' : label;
  const materialSourceNote = w => authorUnavailable(w) ? `<p class="lgi-tm-note">作者原文当前未取得或不可用。${commentOnlyResearch(w) ? '已有合格评论与讨论继续可读。' : '其他来源的可读范围以当前复核结果为准。'}</p>` : '';
  const topic = ref => list(snapshot?.topics).find(t => t.topicRef === ref);
  const work = ref => list(snapshot?.works).find(w => w.workRef === ref) || extraWorks.get(ref);
  const topicName = ref => topic(ref)?.displayName || '主题';
  const labelState = value => ({provisional:'暂定定义',pending:'待分析',blocked:'当前被阻塞',active:'使用中',persisted:'已保存',candidate:'候选方向',published:'已发布定义',active:'使用中',paused:'已暂停',running:'正在处理',queued:'待处理',completed:'已处理',completed_with_failures:'部分处理',failed:'处理失败',stopped:'已停止',cancelled:'已停止',daily_limit:'日额度已用尽',budget_exhausted:'额度已用尽',deferred:'额度延后',unknown:'未知',not_configured:'尚未设置',ready:'已就绪'}[value] || value || '未知');
  const btn = (text, action, id = '', cls = '', extra = '') => `<button type="button" class="lgi-tm-button ${cls}" data-action="${esc(action)}" data-id="${esc(id)}" ${extra}>${text}</button>`;
  const tab = (text, action, id, selected) => btn(esc(text), action, id, `lgi-tm-tab ${selected ? 'is-active' : ''}`, `aria-pressed="${selected}"`);
  const badge = (text, signal = false) => `<span class="lgi-tm-badge ${signal ? 'is-signal' : ''}">${esc(text)}</span>`;
  const empty = (text, detail = '') => `<div class="lgi-tm-empty"><strong>${esc(text)}</strong>${detail ? `<p>${esc(detail)}</p>` : ''}</div>`;
  const select = (key, label, options) => `<label class="lgi-tm-control"><span>${esc(label)}</span><select data-filter="${key}">${options.map(([value,text]) => `<option value="${esc(value)}" ${String(state[key]) === String(value) ? 'selected' : ''}>${esc(text)}</option>`).join('')}</select></label>`;
  function persist() {
    try { sessionStorage.setItem('linggan.topic-map.view', JSON.stringify(state)); } catch (_) {}
    const url = new URL(location.href);
    url.searchParams.delete('domain');
    ['domainRef','topicRef','platform','windowDays','referenceWindowDays'].forEach(key => state[key] ? url.searchParams.set(key,state[key]) : url.searchParams.delete(key));
    history.replaceState(null, '', url);
  }
  function query(overrides = {}) {
    const values = {...state,...overrides};
    const q = new URLSearchParams();
    ['domainRef','topicRef','platform','windowDays','referenceWindowDays','path','overlay'].forEach(key => { if (values[key]) q.set(key,values[key]); });
    return q.toString();
  }
  function friendlyError(code,status) {
    const names = {topic_map_not_connected:'图谱数据库尚未连接',topic_map_schema_not_ready:'图谱数据结构尚未就绪',research_not_configured:'尚未保存研究模型与预算设置',collection_not_enabled:'尚未明确开启本领域的有界补采',collection_admission_rejected:'现有采集授权或执行范围未满足准入条件',material_not_found:'材料不在当前领域可读范围内',move_children_before_structural_replacement:'旧主题仍有子主题，请先调整子主题归属再替换结构',invalid_structure_cardinality:'合并与拆分的主题数量不符合本次调整方式',invalid_structure_destination:'每个新方向需要名称、定义和显式分配的实际作品',invalid_structure_member:'作品分配需要独立身份与具体理由',structure_source_restricted:'部分来源材料当前不可用，不能重新分配',request_identity_conflict:'本次请求身份与已保存内容冲突，请刷新后重新检查',topic_map_conflict:'定义或规则版本已变化，请刷新当前版本',model_configuration_missing:'所选模型配置尚未取得',research_not_found:'该研究运行尚未读取',domain_required:'先选择明确研究领域',invalid_research_budget:'研究预算须填写有效的明确上限'};
    return names[code] || (code ? `服务未完成此操作：${String(code)}` : `读取未成功（${status}）`);
  }
  async function request(url, options) {
    const response = await fetch(url, {headers:{'Accept':'application/json', ...(options?.body ? {'Content-Type':'application/json'} : {})}, ...options});
    let data; try { data = await response.json(); } catch (_) { throw new Error('服务返回的内容暂时无法读取'); }
    if (!response.ok) {const code=data.message || data.detail || data.reason || data.code || data.error; const failure=new Error(friendlyError(code,response.status));failure.status=response.status;failure.code=code;throw failure;}
    return data;
  }
  async function load() {
    const epoch = ++fetchEpoch; loading = true; readError = ''; render();
    try {
      const result = await request(`/api/local/topic-map?${query()}`);
      if (epoch !== fetchEpoch) return;
      if (!result || !Array.isArray(result.topics) || !Array.isArray(result.works) || !result.scope || !result.statistics) throw new Error('主题图谱读取合同不完整，原有结果继续保留');
      snapshot = result;
      if (!state.domainRef && result.scope.domainRef) state.domainRef = result.scope.domainRef;
      if (!state.domainRef && result.domains.length) { state.domainRef=result.domains[0].domainRef; loading=false; persist(); return await load(); }
      if (state.topicRef && !topic(state.topicRef)) state.topicRef = '';
      persist();
    } catch (error) { if (epoch === fetchEpoch && error.status===404 && state.topicRef) { state.topicRef='';notice='原主题引用当前不可用，已返回该领域概览；旧备选的定义引用仍保留。';persist();return await load(); } if (epoch === fetchEpoch) readError = error.message; }
    finally { if (epoch === fetchEpoch) { loading = false; render(); } }
  }
  async function command(payload, successText) {
    if (pending) return null;
    pending = true; notice = ''; renderNotice();
    try {
      const result = await request('/api/local/topic-map/commands', {method:'POST',body:JSON.stringify({...payload,idempotencyKey:payload.idempotencyKey || uuid(),domainRef:payload.domainRef || state.domainRef})});
      notice = successText || '操作已获得后端回执'; await load(); return result;
    } catch (error) { notice = `操作未完成：${error.message}`; return null; }
    finally { pending = false; renderNotice(); }
  }
  function selectedWorks() { return list(snapshot?.works); }
  function scopeText() {
    return `${list(snapshot?.domains).find(d => (d.domainRef || d.id) === state.domainRef)?.displayName || list(snapshot?.domains).find(d => (d.domainRef || d.id) === state.domainRef)?.name || '当前领域'} · ${state.platform || '全部平台'} · ${state.windowDays ? `近${state.windowDays}天发布` : '全部发布时间'}`;
  }
  function renderNotice() {
    const target = root.querySelector('[data-notice]');
    if (target) { target.textContent = notice; target.hidden = !notice; }
    root.querySelectorAll('[data-mutation]').forEach(node => { node.disabled = pending; });
  }
  function header() {
    return `<div class="lgi-tm-pagehead"><div class="lgi-tm-head-title"><h1>主题图谱</h1><p>从领域结构查看内容，再用具体材料判断下一次切入。</p></div><div class="lgi-tm-row">${btn('我的备选','saved')}${btn('研究进度与设置','research')}${btn('我方矩阵','identity')}</div></div><div class="lgi-tm-main-tabs" aria-label="主题图谱视图">${tab('主题概览','view','overview',state.view === 'overview')}${tab('用户生命旅程','view','journey',state.view === 'journey')}</div>`;
  }
  function controls() {
    return `<div class="lgi-tm-toolbar">${select('platform','平台',[['','全部平台'],['xhs','小红书'],['douyin','抖音']])}${select('windowDays','概览统计窗口',[['','全部发布时间'],['7','近7天'],['30','近30天'],['90','近90天']])}${state.view === 'journey' ? select('path','明确经历',[['','全部经历'],['family','家庭支持'],['adult','成年自我管理']]) : ''}<form class="lgi-tm-search" data-form="search"><label class="v7-sr-only" for="topic-map-search">查找当前范围的作品 / 主题</label><div class="lgi-tm-row"><input id="topic-map-search" name="query" value="${esc(state.query)}" placeholder="输入关键词"><button class="lgi-tm-button" type="submit">查找</button></div></form>${btn('刷新已有结果','refresh')}</div>`;
  }
  function tree() {
    const topics = list(snapshot?.topics).filter(t=>t.lifecycleState!=='superseded'), roots = topics.filter(t => !t.parentTopicRef || !topics.some(x => x.topicRef === t.parentTopicRef));
    function branch(t, trail = new Set()) {
      if (trail.has(t.topicRef)) return '';
      const next = new Set(trail); next.add(t.topicRef);
      const children = topics.filter(c => c.parentTopicRef === t.topicRef), expanded = !list(state.collapsed).includes(t.topicRef);
      return `<li><div class="lgi-tm-tree-row ${state.topicRef === t.topicRef ? 'is-active' : ''}">${children.length ? btn(expanded ? '−' : '+','collapse',t.topicRef,'lgi-tm-tree-toggle',`aria-label="${expanded ? '收起' : '展开'}${esc(t.displayName)}" aria-expanded="${expanded}"`) : '<span class="lgi-tm-tree-spacer"></span>'}${btn(esc(t.displayName),'topic',t.topicRef,'lgi-tm-tree-name')}${t.lifecycleState === 'candidate' ? '<span class="lgi-tm-candidate-dot" title="候选方向">候选</span>' : ''}<span class="lgi-tm-num">${number(t.statistics?.workCount)}</span></div>${children.length && expanded ? `<ul>${children.map(c => branch(c,next)).join('')}</ul>` : ''}</li>`;
    }
    const pickerOptions=[];function optionBranch(t,path=[]){if(path.includes(t.topicRef))return;const next=[...path,t.topicRef];pickerOptions.push(`<option value="${esc(t.topicRef)}" ${state.topicRef===t.topicRef?'selected':''}>${'　'.repeat(path.length)}${esc(t.displayName)}</option>`);topics.filter(c=>c.parentTopicRef===t.topicRef).forEach(c=>optionBranch(c,next));}roots.forEach(t=>optionBranch(t));
    return `<label class="lgi-tm-mobile-tree-picker lgi-tm-control"><span>主题层级 · 全部方向可选择</span><select data-filter="topicRef"><option value="" ${!state.topicRef?'selected':''}>全部方向</option>${pickerOptions.join('')}</select></label><aside class="lgi-tm-tree" aria-label="稳定主题结构"><div class="lgi-tm-section-head"><h2>领域结构</h2>${btn('调整归属','move-topic','','lgi-tm-quiet')}</div><div class="lgi-tm-tree-row ${!state.topicRef ? 'is-active' : ''}">${btn('全部方向','topic','','lgi-tm-tree-name')}<span class="lgi-tm-num">${number(snapshot?.scope?.totalWorkCount)}</span></div><ul>${roots.map(t => branch(t)).join('')}</ul>${topics.length ? '' : '<p class="lgi-tm-note">当前领域尚未建立主题定义，已入库材料仍然可读。</p>'}<p class="lgi-tm-note">每个数字为去重作品，子主题之间可以重叠。</p></aside>`;
  }
  function identity() {
    const t = topic(state.topicRef);
    return `<div class="lgi-tm-object-head"><div><span class="lgi-tm-eyebrow">${t ? '当前主题' : '全领域主题概览'}</span><h2>${esc(t?.displayName || '当前领域概览')}</h2><p>${esc(t?.definitionText || '查看已观察材料的结构、表现和来源；已入库样本不代表全平台供给。')}</p><div class="lgi-tm-row">${t ? badge(labelState(t.lifecycleState)) + badge(`定义 v${t.definitionVersion}`) : ''}${list(t?.aliases).map(alias => badge(alias)).join('')}</div></div><div class="lgi-tm-row">${btn('主题 × 旅程','matrix')}${btn('判断这个方向','judge',state.topicRef,'lgi-tm-primary',state.topicRef ? '' : 'disabled title="先选择具体主题"')}</div></div><div class="lgi-tm-scope"><span>${esc(scopeText())}</span><span>指标截至 ${esc(snapshot?.scope?.asOf || '未知')}</span></div>`;
  }
  function platformStats(stats) { return list(stats?.platforms); }
  function metrics(stats = snapshot?.statistics) {
    const platforms = platformStats(stats);
    return `<section class="lgi-tm-metrics" aria-label="基础概览指标"><div><span>观察到的内容</span>${btn(`<strong class="lgi-tm-value">${number(stats?.workCount)}</strong> 篇`,'materials','','lgi-tm-stat-action')}<small>${number(snapshot?.scope?.readableWorkCount)} 篇可读 · ${number(snapshot?.scope?.unclassifiedWorkCount)} 篇待归主题</small></div><div><div class="lgi-tm-row"><span>样本爆款率</span>${btn('规则','rules','','lgi-tm-quiet')}</div>${platforms.map(p => `<p><span>${esc(platformName(p.platform))}</span> <b class="lgi-tm-num">${p.highPerformanceThreshold == null ? '未设置' : pct(p.highPerformanceCount,p.knownLikeCount)}</b><small>${p.highPerformanceThreshold == null ? '设置明确规则后计算' : `${number(p.highPerformanceCount)} / ${number(p.knownLikeCount)} 篇可判定`} · ${number(p.unknownLikeCount)} 篇点赞未知</small></p>`).join('') || '<strong class="lgi-tm-value">—</strong><small>尚无平台统计</small>'}</div><div><span>常规 / 高位表现</span>${platforms.map(p => `<p>${esc(platformName(p.platform))} <b class="lgi-tm-num">${number(p.medianLikes)} / ${number(p.p90Likes)}</b><small>同平台点赞中位数 / P90</small></p>`).join('') || '<strong class="lgi-tm-value">—</strong><small>点赞数据尚未取得</small>'}</div><div><span>已观察创作者</span><strong class="lgi-tm-value">${number(stats?.authorCount)} <small>位</small></strong><small>${number(stats?.unknownAuthorCount)} 篇作者身份未知<br>仅当前材料的已确认作者</small></div></section>`;
  }
  function platformName(id) { return ({xhs:'小红书',douyin:'抖音'}[id] || id || '平台未知'); }
  function coverage() {
    const s = snapshot?.scope;
    return `<div class="lgi-tm-boundary"><span>${esc(snapshot?.sourceBoundary || '统计引用后端读模型；来源缺口与研究判断分别表达。')}</span>${snapshot?.scope?.readLimit ? `<span>读取上限 ${number(snapshot.scope.readLimit)} · 当前返回 ${list(snapshot.works).length} 篇</span>` : ''}${s && s.totalWorkCount > list(snapshot.works).length ? '<span>作品列表为部分读取；基础总量采用服务端统计。</span>' : ''}</div>`;
  }
  function overview() {
    const stats = topic(state.topicRef)?.statistics || snapshot.statistics;
    return `<div class="lgi-tm-atlas">${tree()}<main class="lgi-tm-main">${identity()}${topicBoundary(topic(state.topicRef))}${metrics(stats)}${coverage()}<div class="lgi-tm-work-tabs" aria-label="概览工作视角">${views.map(([id,name]) => tab(name,'tab',id,state.tab === id)).join('')}</div>${state.tab === 'structure' ? structure() : state.tab === 'patterns' ? patterns() : state.tab === 'sources' ? sources() : state.tab === 'candidates' ? candidates() : actionView()}${state.tab !== 'sources' && state.tab !== 'candidates' ? materialsSection() : ''}</main></div>`;
  }
  function children() { return list(snapshot?.topics).filter(t=>t.lifecycleState!=='superseded').filter(t => state.topicRef ? t.parentTopicRef === state.topicRef : !t.parentTopicRef); }
  function plotPlatform(stats = topic(state.topicRef)?.statistics || snapshot.statistics) {
    const ids = platformStats(stats).map(p => p.platform);
    return ids.includes(state.plotPlatform) ? state.plotPlatform : ids.includes(state.platform) ? state.platform : ids[0] || '';
  }
  function plotSelect(stats) {
    const id = plotPlatform(stats);
    return `<label class="lgi-tm-control"><span>表现图平台</span><select data-filter="plotPlatform">${platformStats(stats || topic(state.topicRef)?.statistics || snapshot.statistics).map(p => `<option value="${esc(p.platform)}" ${p.platform===id?'selected':''}>${esc(platformName(p.platform))}</option>`).join('')}</select></label>`;
  }
  function changeRows() { return list(snapshot?.changes).filter(c => c.structureChanged || c.notViewed || list(c.unviewedWorkRefs).length); }
  function changesBar() {
    const changes = changeRows();
    return `<div class="lgi-tm-changes-bar"><div><strong>${changes.length ? `${changes.length} 个主题的结构或依据变化可查看` : '当前没有尚未查看的结构或依据变化'}</strong><span> · 材料与分类变化，不直接解释为市场变化</span></div><div class="lgi-tm-row">${tab('全部方向','changes-mode','all',!state.changesOnly)}${tab('查看变化','changes-mode','changes',state.changesOnly)}${btn('查看变更记录','changes','','lgi-tm-quiet')}</div></div>`;
  }
  function structure() {
    const allRows = children(), changed = new Set(changeRows().map(c => c.topicRef));
    const rows = allRows.filter(t => (!state.query || t.displayName.includes(state.query)) && (!state.changesOnly || changed.has(t.topicRef)));
    return `${allRows.length ? `<div class="lgi-tm-section-head"><div><h3>主题供给分布</h3><p>点名称下钻，勾选最多三个方向并排比较。</p></div><div class="lgi-tm-overview-tools">${tab('表格','summary-chart','table',state.summaryChart!=='scatter')}${tab('分布图','summary-chart','scatter',state.summaryChart==='scatter')}${btn('并排比较','compare','','',list(state.compare).length ? '' : 'disabled title="先勾选方向"')}</div></div>${rows.length ? state.summaryChart==='scatter' ? topicScatter(rows) : structureTable(rows) : empty('当前子方向没有对应的未读变化','已有统计与材料继续可看，可切回全部方向。')}<p class="lgi-tm-note">子主题可以重叠，父主题按作品去重；比例描述已观察样本，不是市场份额。</p>` : '<div class="lgi-tm-section-head"><div><h3>当前方向的内容与表现</h3><p>叶主题继续展示完整统计与作品，不要求先启动研究。</p></div></div>'}${distribution()}`;
  }
  function structureTable(rows) {
    const parent = topic(state.topicRef)?.statistics || snapshot.statistics, max = Math.max(1,...rows.map(t=>t.statistics?.workCount || 0));
    return `<div class="lgi-tm-table-scroll"><table class="lgi-tm-supply-table"><thead><tr><th>比较</th><th>细分方向</th><th>内容供给</th><th>样本爆款率</th><th>创作者</th><th>中位赞 / P90</th></tr></thead><tbody>${rows.map(t => `<tr><td><input type="checkbox" data-compare="${esc(t.topicRef)}" aria-label="比较${esc(t.displayName)}" ${list(state.compare).includes(t.topicRef) ? 'checked' : ''}></td><td>${btn(esc(t.displayName),'topic',t.topicRef,'lgi-tm-title-link')}<span class="lgi-tm-subline">${list(snapshot.topics).filter(c=>c.parentTopicRef===t.topicRef&&c.lifecycleState!=='superseded').length ? '含子方向' : '细分主题'}${t.lifecycleState==='candidate'?' · 新候选':''}</span></td><td><div class="lgi-tm-countbar"><div class="lgi-tm-count-track"><span style="width:${(t.statistics?.workCount || 0)/max*100}%"></span></div><b class="lgi-tm-num">${number(t.statistics?.workCount)}</b></div><span class="lgi-tm-subline">占当前父级 ${pct(t.statistics?.workCount,parent.workCount)}</span></td><td>${platformStats(t.statistics).map(p=>`<p>${esc(platformName(p.platform))} · ${p.highPerformanceThreshold==null?'规则未设置':pct(p.highPerformanceCount,p.knownLikeCount)}<span class="lgi-tm-subline">${p.highPerformanceThreshold==null?'未判定':`${number(p.highPerformanceCount)} / ${number(p.knownLikeCount)} 篇点赞已知`}</span></p>`).join('') || '表现未知'}</td><td class="lgi-tm-num">${number(t.statistics?.authorCount)}<span class="lgi-tm-subline">${number(t.statistics?.unknownAuthorCount)} 篇作者未知</span></td><td>${platformStats(t.statistics).map(p=>`<p>${esc(platformName(p.platform))}<br><span class="lgi-tm-num">${number(p.medianLikes)} / ${number(p.p90Likes)}</span></p>`).join('') || '指标未知'}</td></tr>`).join('')}</tbody></table></div>`;
  }
  function topicScatter(rows) {
    const platform=plotPlatform(), all=rows.map(t=>({t,p:platformStats(t.statistics).find(p=>p.platform===platform)})).filter(x=>x.p?.workCount>0);
    const data=all.filter(x=>known(x.p.highPerformanceCount)&&x.p.highPerformanceThreshold!=null&&x.p.knownLikeCount>0), unknown=all.length-data.length, max=Math.max(5,...data.map(x=>x.p.workCount))*1.2;
    return `<div class="lgi-tm-section-head"><p>横轴：已观察作品数；纵轴：样本爆款率。仅对照同平台，不划“蓝海区”。${unknown ? `${unknown} 个主题表现不可判定，未画在0%位置，仍可从表格查看。` : ''}</p>${plotSelect()}</div>${data.length ? `<div class="lgi-tm-table-scroll"><svg class="lgi-tm-scatter" viewBox="0 0 960 350" role="group" aria-label="${esc(platformName(platform))}主题供给与表现分布"><path class="lgi-tm-scatter-axis" d="M67 35V293H916"/>${[0,.25,.5,.75,1].map(v=>`<path class="lgi-tm-scatter-grid" d="M67 ${293-v*240}H916"/><text x="54" y="${297-v*240}" text-anchor="end">${v*100}%</text><text x="${67+v*800}" y="317" text-anchor="middle">${Math.round(v*max)}</text>`).join('')}<text x="70" y="20">样本爆款率 · ${esc(platformName(platform))}</text><text x="885" y="339" text-anchor="end">已观察作品数</text>${data.map(({t,p},i)=>{const x=67+p.workCount/max*800,y=293-p.highPerformanceCount/p.knownLikeCount*240;return `<g role="button" tabindex="0" class="lgi-tm-scatter-point" data-action="topic" data-id="${esc(t.topicRef)}" aria-label="${esc(t.displayName)}，${p.workCount}篇，样本爆款率${pct(p.highPerformanceCount,p.knownLikeCount)}"><circle cx="${x}" cy="${y}" r="8"/><text x="${x+13}" y="${y+(i%2?20:-13)}">${esc(t.displayName)}</text><title>${esc(t.displayName)} · ${p.highPerformanceCount}/${p.knownLikeCount} 篇可判定</title></g>`;}).join('')}</svg></div>` : empty('当前平台还没有可判定的主题表现','规则未设置或点赞未知时，不把主题画在0%位置。')}`;
  }
  function distribution() {
    const stats=topic(state.topicRef)?.statistics || snapshot.statistics, platform=plotPlatform(stats), p=platformStats(stats).find(p=>p.platform===platform);
    if(!p)return empty('当前没有可绘制的点赞数据','原始材料仍可查看。');
    const ns=selectedWorks().filter(w=>w.platform===platform), bins=[[0,20,'≤20赞'],[21,99,'21–99赞'],[100,499,'100–499赞'],[500,999,'500–999赞'],[1000,Infinity,'≥1,000赞'],[null,null,'指标缺失']].map(([min,max,label])=>({label,refs:ns.filter(w=>min===null ? !known(w.likes) : known(w.likes)&&w.likes>=min&&w.likes<=max).map(w=>w.workRef)})), max=Math.max(1,...bins.map(b=>b.refs.length));
    return `<section class="lgi-tm-section"><div class="lgi-tm-section-head"><div><h3>表现结构</h3><p>从整体分布检查常规与高位表现，不只看成功案例。</p></div>${plotSelect(stats)}</div><div class="lgi-tm-distribution-layout"><div class="lgi-tm-bin-list"><div class="lgi-tm-section-head"><strong>${esc(platformName(platform))} · 点赞分布</strong><span>${ns.length} 篇当前读取作品</span></div>${bins.map((b,i)=>`<button type="button" class="lgi-tm-bin-row" data-action="subset" data-id="${esc(b.refs.join(','))}" ${b.refs.length?'':'disabled'} aria-label="${b.label}，${b.refs.length}篇，查看作品"><span>${b.label}</span><span class="lgi-tm-bin-track"><i class="lgi-tm-bin-fill ${i===5?'is-missing':''}" style="width:${b.refs.length/max*100}%"></i></span><b class="lgi-tm-num">${b.refs.length}</b></button>`).join('')}</div><aside class="lgi-tm-dist-aside"><h4>常规表现与高位表现，相差多少？</h4><div class="lgi-tm-row">${badge(`中位赞 ${number(p.medianLikes)}`)}${badge(`P90 ${number(p.p90Likes)}`)}${badge(known(p.medianLikes)&&p.medianLikes>0&&known(p.p90Likes)?`${(p.p90Likes/p.medianLikes).toFixed(1)} 倍`:'倍数未判定')}</div><p>中位数描述样本中间位置，P90描述较高位置。比值用于检查分布差异，不预测再次发布的效果。</p><p class="lgi-tm-note">读数引用后端 ${number(p.workCount)} 篇统计范围；横条只描述当前返回的 ${ns.length} 篇作品。${number(p.unknownLikeCount)} 篇点赞未知不计为0，低互动不等于内容失败。</p>${btn('检查高表现参考来自谁','tab','sources','lgi-tm-quiet')}</aside></div></section>`;
  }
  function annotation(work) { return work.annotation && typeof work.annotation === 'object' ? work.annotation : {}; }
  function topicBoundary(t, source = snapshot) {
    if (!t) return '';
    if(t.core?.sourceState === 'source_unavailable')return '<section class="lgi-tm-topic-boundary" aria-label="主题边界来源受限"><h3>主题定义的来源当前受限</h3><p>纳入、排除条件与定义关系暂不可展示。已有当前可读材料仍可核对，不能把来源受限解释为边界尚未定义。</p></section>';
    const core=t.core, inclusion=list(core?.inclusionCriteria).filter(v=>typeof v==='string'&&v.trim()), exclusion=list(core?.exclusionCriteria).filter(v=>typeof v==='string'&&v.trim());
    const origin=({machine_induced:'机器提炼的定义',manual:'人工维护的定义',legacy:'历史定义 · 边界未补齐'})[core?.definitionSource] || (core?'定义来源尚未取得':'历史定义 · 边界未补齐');
    return `<section class="lgi-tm-topic-boundary" aria-label="主题纳入与排除边界"><div class="lgi-tm-section-head"><h3>这个主题包含什么？</h3>${badge(origin)}</div><div class="lgi-tm-criteria"><div><h4>纳入条件</h4>${inclusion.length?`<ul>${inclusion.map(v=>`<li>${esc(v)}</li>`).join('')}</ul>`:'<p class="lgi-tm-note">纳入边界尚未补齐，已有定义与材料继续可读。</p>'}</div><div><h4>排除条件</h4>${exclusion.length?`<ul>${exclusion.map(v=>`<li>${esc(v)}</li>`).join('')}</ul>`:'<p class="lgi-tm-note">排除边界尚未补齐，不能据名称认定范围。</p>'}</div></div>${core?`<p class="lgi-tm-note">当前范围 ${number(core.discussionCount)} 条独立讨论 · 支持 ${number(core.evidenceRoles?.support)} · 反例 ${number(core.evidenceRoles?.challenge)} · 背景 ${number(core.evidenceRoles?.context)}。按讨论计数，不代表作品数或人数；归入同一主题可以保留不同观点。</p>`:''}${relationDetails(core?.relations,source,'本主题')}</section>`;
  }
  function relationDetails(relations, source = snapshot, subject = '此讨论') {
    const rows=list(relations).filter(r=>r&&typeof r==='object');
    if(!rows.length)return '';
    const names={equivalent:'含义相同',broader:`${subject}范围更宽`,narrower:`${subject}范围更细`,related:'相互关联',distinct:'边界不同',uncertain:'关系未确定'};
    return `<details class="lgi-tm-core-details"><summary>与已有主题的关系 · ${rows.length}</summary>${rows.map(r=>{const t=list(source?.topics).find(t=>t.topicRef===r.topicRef),name=t?.displayName || r.displayName || r.label || '已有主题';return `<p>${r.topicRef?btn(esc(name),'topic',r.topicRef,'lgi-tm-title-link'):esc(name)} · ${esc(names[r.relation] || '关系未确定')}${r.reason?`<span class="lgi-tm-subline">${esc(r.reason)}</span>`:''}</p>`;}).join('')}<p class="lgi-tm-note">研究关系供核对，主题树按已保存的归属显示。</p></details>`;
  }
  function coverageState(w) {
    const c=w?.research?.core?.coverage;
    if(!c)return 'unknown';
    const counts=[c.sourceChars,c.coveredChars,c.totalWindows,c.completedWindows], valid=counts.every(v=>Number.isInteger(v)&&v>=0);
    return c.state==='complete'&&valid&&c.coveredChars===c.sourceChars&&c.completedWindows===c.totalWindows?'complete':c.state==='partial'||valid?'partial':'unknown';
  }
  function researchCoverage(w) {
    if(!w?.research)return '';
    const c=w.research.core?.coverage, status=coverageState(w);
    if(!c)return `<p class="lgi-tm-note">${Array.isArray(w.research.core?.units)?'当前研究的处理范围尚未取得。':'旧研究的处理范围未记录，讨论边界尚未补齐。'}</p>`;
    return `<p class="lgi-tm-research-coverage ${status==='partial'?'is-partial':''}"><strong>${status==='complete'?'当前可读来源已处理':status==='partial'?'部分来源已处理':'已处理范围尚未确认'}</strong><span>已处理 ${number(c.coveredChars)} / ${number(c.sourceChars)} 字符 · ${number(c.completedWindows)} / ${number(c.totalWindows)} 段来源</span>${status!=='complete'?'<span>已有结果可读，未处理部分的讨论与归属仍未知。</span>':'<span>覆盖范围限于本次可读来源，不代表平台全文或领域观察完整。</span>'}</p>`;
  }
  function researchCoverageSummary(ns) {
    const rows=[...new Map(ns.map(w=>[w.workRef,w])).values()].filter(w=>w.research), counts={complete:0,partial:0,unknown:0};
    rows.forEach(w=>counts[coverageState(w)]++);
    return rows.length?`<p class="lgi-tm-note">${rows.length} 篇当前材料有研究结果：${counts.complete} 篇当前可读来源已处理 · ${counts.partial} 篇仅处理部分 · ${counts.unknown} 篇处理范围未确认。一个讨论可涉及多个主题，各组数量不相加为总量。</p>`:'';
  }
  function discussionUnits(w) {
    const hasCore=Array.isArray(w?.research?.core?.units), rows=hasCore?w.research.core.units:list(w?.research?.output?.discussions), seen=new Set();
    return rows.flatMap((d,index)=>{
      if(!d||typeof d!=='object')return [];
      const unitKey=hasCore&&typeof d.unitId==='string'&&d.unitId?d.unitId:JSON.stringify([w.workRef,'legacy',index]);
      if(seen.has(unitKey))return [];seen.add(unitKey);
      const candidates=hasCore?['matched','new'].includes(d.status)?list(d.assignments):[]:d.topicRef?[{topicRef:d.topicRef,definitionRef:d.definitionRef,label:d.label}]:[], assigned=new Set();
      const assignments=candidates.filter(a=>{if(!a||typeof a!=='object')return false;const key=assignmentKey(a);if(!key||assigned.has(key))return false;assigned.add(key);return true;});
      return [{...d,unitKey,assignments,legacy:!hasCore,evidence:list(d.evidence)}];
    });
  }
  function assignmentKey(a) { return typeof a?.topicRef==='string'&&a.topicRef?`topic:${a.topicRef}`:typeof a?.definitionRef==='string'&&a.definitionRef?`definition:${a.definitionRef}`:''; }
  function discussionKeys(w) { return new Set(discussionUnits(w).flatMap(d=>d.assignments.map(assignmentKey))); }
  function sharesDiscussion(a,b) { const keys=discussionKeys(a);return [...discussionKeys(b)].some(key=>keys.has(key)); }
  function locatedCitations(w, citations) {
    const fragments=new Map(list(w?.research?.fragments).map(f=>[f.fragmentId,f])), seen=new Set();
    return list(citations).flatMap(c=>{
      if(!c||typeof c!=='object')return [];
      const f=fragments.get(c.fragmentId), chars=Array.from(f?.text || ''), base=Number.isInteger(f?.start)?f.start:0;
      if(!f||!Number.isInteger(c.start)||!Number.isInteger(c.end)||c.start<base||c.end<=c.start||c.end>base+chars.length)return [];
      const key=JSON.stringify([c.fragmentId,c.start,c.end]);if(seen.has(key))return [];seen.add(key);
      return [{...c,fragment:f,localStart:c.start-base,localEnd:c.end-base}];
    });
  }
  function citationText(w, citations) {
    const c=locatedCitations(w,citations)[0];
    return c?Array.from(c.fragment.text).slice(c.localStart,c.localEnd).join(''):'';
  }
  function discussionGroups(ns, source = dialogState?.snapshot || snapshot) {
    const groups=new Map();
    ns.forEach(w=>discussionUnits(w).forEach(d=>(d.assignments.length?d.assignments:[null]).forEach(a=>{
      const key=a?assignmentKey(a):JSON.stringify(['unassigned',w.workRef,d.unitKey]), t=list(source?.topics).find(t=>a?.topicRef&&t.topicRef===a.topicRef);
      if(!groups.has(key))groups.set(key,{key,topicRef:a?.topicRef || '',label:t?.displayName || a?.label || d.label || d.statement || '尚未命名的讨论',canonical:Boolean(a),rows:[],seen:new Set()});
      const group=groups.get(key), rowKey=JSON.stringify([w.workRef,d.unitKey]);if(group.seen.has(rowKey))return;group.seen.add(rowKey);
      group.rows.push({work:w,discussion:d,assignment:a});
    })));
    return [...groups.values()].map(g=>({...g,works:[...new Map(g.rows.map(r=>[r.work.workRef,r.work])).values()],evidenceCount:new Set(g.rows.flatMap(r=>locatedCitations(r.work,r.discussion.evidence).map(c=>JSON.stringify([r.work.workRef,c.fragmentId,c.start,c.end])))).size}));
  }
  function discussionUnitView(row, reader = null) {
    const w=row.work,d=row.discussion, source=dialogState?.snapshot || snapshot;
    const speaker=({author:'作者表达',commenter:'评论者原声',quoted:'引用或转述',unknown:'来源角色未知'})[d.speakerRole] || '来源角色未知';
    const role=({support:'支持材料',challenge:'反例或不同经验',context:'背景材料'})[d.evidenceRole] || '材料作用未判定';
    const status=d.legacy?'旧研究 · 边界未补齐':({matched:'已判断归属',new:'新方向候选',uncertain:'归属尚未确定',out_of_scope:'当前领域外'})[d.status] || '归属尚未确定';
    const citations=locatedCitations(w,d.evidence), quote=citationText(w,d.evidence);
    return `<section class="lgi-tm-discussion-unit"><div class="lgi-tm-row">${badge(speaker)}${badge(role)}${badge(status)}</div><h4>${esc(d.label || '具体讨论')}</h4>${d.statement?`<p>${esc(d.statement)}</p>`:''}${d.rationale?`<p class="lgi-tm-note">材料依据：${esc(d.rationale)}</p>`:''}${d.assignments.length?`<div class="lgi-tm-unit-assignments">${d.assignments.map(a=>{const t=list(source?.topics).find(t=>t.topicRef===a.topicRef);return `<p>归属主题：${a.topicRef?btn(esc(t?.displayName || a.label || '已有主题'),'topic',a.topicRef,'lgi-tm-title-link'):esc(a.label || '已有主题')}<span class="lgi-tm-subline">归属理由：${esc(a.reason || (d.legacy?'旧研究未记录具体归属理由。':'归属理由尚未取得。'))}</span></p>`;}).join('')}${d.assignments.length>1?'<p class="lgi-tm-note">同一条讨论涉及多个主题，讨论总量只计一次。</p>':''}</div>`:d.legacy?'<p class="lgi-tm-note">尚无可核对的主题身份，同名讨论分别保留。</p>':'<p class="lgi-tm-note">尚未归入主题，已有讨论与证据保留。</p>'}${d.reason?`<p class="lgi-tm-note">判断说明：${esc(d.reason)}</p>`:''}${quote?`<p class="lgi-tm-note">原文片段 · ${esc(sourceKind(citations[0].fragment.sourceKind || citations[0].fragment.field))}</p><blockquote>${esc(quote)}</blockquote>`:'<p class="lgi-tm-note">当前没有可展示的精确引用片段，可继续核对原作。</p>'}${reader?`<div class="lgi-tm-row">${citations.map(c=>`<a class="lgi-tm-button lgi-tm-quiet" href="#${encodeURIComponent(readerCitationTarget(w,c,reader.raw))}">${esc(sourceKind(c.fragment.sourceKind || c.fragment.field))} · 第 ${number(c.start+1)}–${number(c.end)} 字</a>`).join('')}</div>`:btn(`${esc(title(w))} · ${readLabel(w,'核对原文')}${citations.length?`与 ${citations.length} 条依据`:''}`,'reader',w.workRef,'lgi-tm-quiet')}${relationDetails(d.relations,source)}</section>`;
  }
  function discussionGroupDetails(g) {
    return `<details class="lgi-tm-core-details"><summary>查看 ${g.rows.length} 条具体讨论、来源与归属依据</summary>${g.rows.map(r=>discussionUnitView(r)).join('')}</details>`;
  }
  function patterns() {
    const ns=selectedWorks(), groups=discussionGroups(ns), tasks=ns.flatMap(w=>list(w.research?.output?.angles).filter(a=>a.answerTask).map(a=>({work:w,angle:a})));
    return `<div class="lgi-tm-section-head"><div><h3>这些内容，实际在怎样讲？</h3><p>当前研究保留具体讨论、场景与回答任务；经验自述、方法说明等讲法分类尚未形成，不用视频或图文形态替代。</p>${researchCoverageSummary(ns)}</div></div><div class="lgi-tm-pattern-grid">${groups.map(g=>`<article class="lgi-tm-panel"><span class="lgi-tm-eyebrow">${g.canonical?'按已保存主题身份归组':'归属待核对的独立讨论'}</span><h4>${esc(g.label)}</h4><p>${g.rows.length} 条独立讨论 · ${g.works.length} 篇当前读取作品 · ${g.evidenceCount} 条去重定位依据</p>${discussionGroupDetails(g)}${btn('查看这些作品','subset',g.works.map(w=>w.workRef).join(','),'lgi-tm-quiet')}</article>`).join('') || empty('讲法分类与具体讨论尚未形成','原文仍然可读；不推断未研究作品的表达方式。')}</div><section class="lgi-tm-section"><h3>哪些具体问题已形成回答任务？</h3><div class="lgi-tm-pattern-grid">${tasks.slice(0,6).map(({work,angle})=>`<article class="lgi-tm-panel"><h4>${esc(angle.label)}</h4><p>${esc(angle.answerTask)}</p>${btn('核对原回答与依据','reader',work.workRef,'lgi-tm-quiet')}</article>`).join('') || empty('当前尚无可引用的回答任务','不能把用户评论问题自动当作原作已有回答。')}</div></section><section class="lgi-tm-section"><h3>具体讨论与场景</h3>${sceneList(ns)}</section>`;
  }
  function sceneList(ns) {
    const groups=new Map();
    ns.forEach(w=>list(w.research?.output?.scenes).forEach(scene=>{
      if(typeof scene.label!=='string'||!scene.label)return;
      if(!groups.has(scene.label))groups.set(scene.label,{works:new Map(),citations:new Set()});
      const group=groups.get(scene.label);group.works.set(w.workRef,w);
      list(scene.evidence).forEach(c=>group.citations.add(JSON.stringify([w.workRef,c.fragmentId,c.start,c.end])));
    }));
    return groups.size ? `<div class="lgi-tm-scene-grid">${[...groups].map(([label,group])=>`<article class="lgi-tm-panel"><h4>${esc(label)}</h4><p>${group.works.size} 篇独立来源作品 · ${group.citations.size} 条去重定位依据</p><div class="lgi-tm-row">${[...group.works.values()].map(w=>btn(esc(title(w)),'reader',w.workRef,'lgi-tm-quiet')).join('')}</div></article>`).join('')}</div>` : empty('当前材料尚未形成可引用的场景分组','阶段尚未确定不影响原作与已有评论的阅读。');
  }
  function sources() {
    const rows = list(snapshot.sources).slice().sort((a,b) => b.workCount-a.workCount), top3=rows.filter(r=>r.authorExternalId).slice(0,3).reduce((n,r)=>n+r.workCount,0), ps=platformStats(snapshot.statistics), configured=ps.filter(p=>p.highPerformanceThreshold!=null), high=configured.reduce((n,p)=>n+(known(p.highPerformanceCount)?p.highPerformanceCount:0),0), highSources=rows.filter(r=>r.authorExternalId&&platformStats(r.statistics).some(p=>known(p.highPerformanceCount)&&p.highPerformanceCount>0)).length;
    return `<div class="lgi-tm-section-head"><div><h3>这些材料主要来自谁？</h3><p>作者贡献引用当前服务端统计范围，作品按身份去重。</p></div>${btn('我方与对标范围','identity')}</div><div class="lgi-tm-source-summary"><div><span>内容来源</span><strong>${number(snapshot.statistics.authorCount)}</strong><span>位已观察创作者 · ${number(snapshot.statistics.unknownAuthorCount)} 篇作者未知</span></div><div><span>前三位已知来源贡献</span><strong>${pct(top3,snapshot.statistics.workCount)}</strong><span>${top3} / ${number(snapshot.statistics.workCount)} 篇当前样本</span></div><div><span>样本高表现参考</span><strong>${configured.length?number(high):'规则未设置'}</strong><span>${configured.length?`涉及 ${highSources} 位已知作者 · ${configured.length<ps.length?'部分平台已设规则':'按各平台已设规则'}`:'不擅自判定高表现'}</span></div></div><div class="lgi-tm-table-scroll"><table><thead><tr><th>创作者</th><th>作品 / 样本占比</th><th>主要方向</th><th>样本表现 / 可判定分母</th><th>作品</th></tr></thead><tbody>${rows.map(r => `<tr><td><div class="lgi-tm-source-who">${avatar(r.creatorDisplayName)}<div><strong>${esc(r.creatorDisplayName || '作者未知')}</strong><p>${esc(platformName(r.platform))}</p></div></div></td><td><b class="lgi-tm-num">${number(r.workCount)}</b> 篇 · ${pct(r.workCount,snapshot.statistics.workCount)}</td><td>${list(r.topicRefs).map(ref => esc(topicName(ref))).join('、') || '尚未归主题'}</td><td>${platformStats(r.statistics).map(p => `<p>${p.highPerformanceThreshold == null ? '样本规则未设置' : `${number(p.highPerformanceCount)} / ${number(p.knownLikeCount)} · ${pct(p.highPerformanceCount,p.knownLikeCount)}`}<br>${number(p.unknownLikeCount)} 篇点赞未知</p>`).join('')}</td><td>${btn('查看作品','subset',list(r.workRefs).join(','),'lgi-tm-quiet')}</td></tr>`).join('')}</tbody></table></div>${rows.length ? '' : empty('当前范围没有作者作品')}<p class="lgi-tm-note">作品可由多条采集路径取得，总量只计一次。不据来源集中程度判断作者能力。</p>${materialsSection()}`;
  }
  function candidates() {
    const rows = list(snapshot?.topics).filter(t => t.lifecycleState === 'candidate');
    return `<div class="lgi-tm-section-head"><div><h3>新方向与尚未归类的材料</h3><p>候选有依据即可查看；新增结构不等于市场增长。</p></div>${btn('结构与依据变更','changes')}${btn('结构调整','structure-change')}</div>${rows.length ? rows.map(t => `<article class="lgi-tm-candidate">${badge('新方向候选',true)}<h4>${esc(t.displayName)}</h4><p>${esc(t.definitionText)}</p><p>${number(t.statistics?.workCount)} 篇当前材料 · 定义 v${esc(t.definitionVersion)}</p><div class="lgi-tm-row">${btn('在概览中查看','topic',t.topicRef)}${btn('判断这个方向','judge',t.topicRef,'lgi-tm-quiet')}</div></article>`).join('') : empty('当前没有新的候选方向','已有主题和材料继续可用；没有新方向不表示观察完整。')}${list(snapshot.candidates).map(c => `<article class="lgi-tm-candidate">${badge('研究提出的候选方向',true)}<h4>${esc(c.label)}</h4><p>由具体作品与 ${list(c.evidence).length} 条引用提出，尚未正式发布定义。</p>${btn('核对候选依据','reader',c.workRef,'lgi-tm-quiet')}</article>`).join('')}<section class="lgi-tm-section"><h3>尚未被主题结构解释的材料</h3><p>${number(snapshot.scope.unclassifiedWorkCount)} 篇待归主题；已有材料仍然可读。</p><div class="lgi-tm-material-grid">${selectedWorks().filter(w => !list(w.topicRefs).length).slice(0,8).map(card).join('')}</div>${btn('查看未归类材料','unassigned','','lgi-tm-quiet')}</section>`;
  }
  function actionView() {
    const stats = topic(state.topicRef)?.statistics || snapshot.statistics;
    const rows = children().length ? children() : topic(state.topicRef) ? [topic(state.topicRef)] : list(snapshot.topics).filter(t=>t.lifecycleState!=='superseded');
    return `<div class="lgi-tm-section-head"><div><h3>从熟悉的方向，找到下一次切入</h3><p>我方发布历史不随概览或外部近期窗口清空。</p></div>${select('referenceWindowDays','外部参考窗口',[['7','近7天'],['30','近30天'],['90','近90天'],['0','全部历史']])}</div><div class="lgi-tm-callout">${snapshot.scope.ownIdentityState === 'unknown' ? '我方矩阵尚未设置，不能据此判断整个账号矩阵没做过。' : `${number(snapshot.scope.ownHistoryPublishedCount)} 篇我方已确认发布 · ${number(stats.ownBreakoutCount)} 篇手动爆款 · ${number(snapshot.scope.ownHistoryPublicationUnknownCount)} 篇发布事实未知`}</div><div class="lgi-tm-table-scroll"><table><thead><tr><th>主题与具体讨论</th><th>我方发布覆盖 / 全部历史</th><th>近期外部低粉高赞</th><th>为什么打开</th></tr></thead><tbody>${rows.map(t => `<tr><td>${btn(esc(t.displayName),'topic',t.topicRef,'lgi-tm-title-link')}<p>${esc(t.definitionText)}</p></td><td>${snapshot.scope.ownIdentityState === 'unknown' ? '我方范围未设置' : `${number(t.statistics?.ownHistoryPublishedCount)} 篇已发布<br>${number(t.statistics?.ownHistoryBreakoutCount)} 篇手动爆款`}</td><td>${number(t.statistics?.recentLowFollowerHighLikeCount)} 篇独立外部作品<p class="lgi-tm-note">粉丝 ≤1,000 且点赞 ≥500，粉丝未知不计入</p></td><td><p>先查看我方原作、外部参考与具体讨论，不把做过当作做完。</p>${btn('判断这个方向','judge',t.topicRef,'lgi-tm-quiet')}</td></tr>`).join('')}</tbody></table></div>`;
  }
  function sortedMaterials(ns) {
    const filtered = ns.filter(w => !state.query || [title(w),w.creatorDisplayName,w.preview].join(' ').includes(state.query));
    if (state.materialMode === 'high') return filtered.filter(w => {const p = platformStats(snapshot?.statistics).find(p => p.platform === w.platform); return known(p?.highPerformanceThreshold) && known(w.likes) && w.likes >= p.highPerformanceThreshold;}).sort((a,b) => a.platform.localeCompare(b.platform) || (b.likes ?? -1) - (a.likes ?? -1));
    if (state.materialMode === 'ordinary') return filtered.filter(w => {const p = platformStats(snapshot?.statistics).find(p => p.platform === w.platform); return known(p?.highPerformanceThreshold) && known(w.likes) && w.likes < p.highPerformanceThreshold;}).sort((a,b) => a.platform.localeCompare(b.platform) || (b.likes ?? -1) - (a.likes ?? -1));
    return filtered.sort((a,b) => String(b.publishedAt || '').localeCompare(String(a.publishedAt || '')));
  }
  function coverUrl(w) {
    const value = w.media?.cover?.localAssetUrl;
    if (typeof value !== 'string') return '';
    try { const url = new URL(value,location.origin); return url.origin === location.origin && ['/api/local/media/','/api/local/derivative/'].some(prefix => url.pathname.startsWith(prefix)) ? url.href : ''; } catch (_) { return ''; }
  }
  function safeUrl(value) { if (typeof value !== 'string') return ''; try { const url = new URL(value,location.origin); return ['http:','https:'].includes(url.protocol) ? url.href : ''; } catch (_) { return ''; } }
  function avatar(name) { const initials=Array.from(typeof name==='string'?name.trim():'').slice(0,2).join('');return `<span class="lgi-tm-avatar" aria-hidden="true">${esc(initials || '·')}</span>`; }
  function card(w) {
    const cover = coverUrl(w);
    return `<article class="lgi-tm-material"><button type="button" class="lgi-tm-cover" data-action="reader" data-id="${esc(w.workRef)}" aria-label="打开${esc(title(w))}">${cover ? `<img src="${esc(cover)}" loading="lazy" alt="${esc(title(w))}的已取得封面">` : '<span>▧<br>封面尚未采集</span>'}</button><div class="lgi-tm-row lgi-tm-card-meta"><span>${esc(platformName(w.platform))}</span>${w.own ? badge(w.ownBreakout ? '我方手动爆款' : '我方作品',w.ownBreakout) : ''}</div><h4>${btn(esc(title(w)),'reader',w.workRef,'lgi-tm-title-link')}</h4>${materialSourceNote(w)}<p class="lgi-tm-author" title="${esc(w.creatorDisplayName || '作者未知')}">${avatar(w.creatorDisplayName)}${esc(w.creatorDisplayName || '作者未知')}</p><p class="lgi-tm-note">${esc(w.publishedAtSourceText || w.publishedAt || '发布时间未知')}</p><div class="lgi-tm-interactions"><span title="点赞精确值 ${number(w.likes)}">赞 <b>${number(w.likes)}</b></span><span title="收藏精确值 ${number(w.collects)}">藏 <b>${number(w.collects)}</b></span><span title="评论精确值 ${number(w.comments)}">评 <b>${number(w.comments)}</b></span><span title="分享精确值 ${number(w.shares)}">分享 <b>${number(w.shares)}</b></span></div></article>`;
  }
  function materialsSection() {
    const ns = sortedMaterials(selectedWorks());
    return `<section class="lgi-tm-section"><div class="lgi-tm-section-head"><div><h3>把数字还原成内容</h3><p>原作与全部材料直接可读，不受精选10篇上限裁切。</p></div><div class="lgi-tm-row">${[['latest','最新作品'],['high','高表现'],['ordinary','常规作品']].map(([id,name]) => tab(name,'material-mode',id,state.materialMode === id)).join('')}</div></div>${ns.length ? `<div class="lgi-tm-material-grid">${ns.slice(0,6).map(card).join('')}</div>` : empty('当前条件没有对应作品',state.materialMode === 'latest' ? '可以调整范围；读取失败与没有材料分别表达。' : '未设置样本表现规则时，不擅自划分高表现与常规作品。')}<div class="lgi-tm-section-head"><p>展示 ${Math.min(6,ns.length)} / ${ns.length} 篇当前读取集合</p>${btn('查看全部材料','materials','','lgi-tm-quiet')}</div></section>`;
  }
  function journeyEntries(journey, mode = state.metric) { return list(mode === 'involved' ? journey?.involved : journey?.main); }
  function stages(journey) { return fallbackStages.map(s => ({...s,...journeyEntries(journey).find(e => e.stage === s.stage)})); }
  function journey() {
    const j = topic(state.topicRef)?.journey || snapshot.journey, stations = stages(j);
    const coordinates = [[150,106],[500,106],[850,106],[750,319],[340,319]];
    const questions = ['发生了什么？怎样理解？','何时求助？如何评估？','哪些支持适合当前处境？','先试什么？怎样开始？','如何调整并持续支持？'];
    const glyphs = ['M4 4h6v14H4zm10 0h6v14h-6M10 5l4-1m-4 14 4-1','M12 3v18M3 12h18M5 5l14 14M19 5 5 19','M4 6h16v13H4zM8 3v6m8-6v6M8 13h8','M4 18l5-8 4 3 7-9M15 4h5v5','M19 7a8 8 0 1 0 1 9M19 3v5h-5'];
    const map = `<div class="lgi-tm-map-shell"><div class="lgi-tm-map-inscription">五个稳定站点 · 内容所回应的经历</div><svg class="lgi-tm-journey-map" viewBox="0 0 1080 462" role="group" aria-label="五主阶段旅程地图，路线为概念关系，不表示人数或转化"><path class="lgi-tm-route-bed" d="M150 108H885C1020 108 1020 319 885 319H340"/><path class="lgi-tm-route" d="M150 108H885C1020 108 1020 319 885 319H340"/><path class="lgi-tm-route-dashes" d="M150 108H885C1020 108 1020 319 885 319H340"/><path class="lgi-tm-route-return" d="M685 289C650 234 592 235 565 267"/><text x="638" y="238" text-anchor="middle" class="lgi-tm-route-note">允许返回、跳过与并行</text>${stations.map((station,i) => {const [x,y]=coordinates[i];return `<g role="button" tabindex="0" class="lgi-tm-station ${state.stage===station.stage?'is-active':''}" data-action="stage" data-id="${station.stage}" aria-label="${esc(station.label)}，${number(station.count)}篇，占${pct(station.count,j?.denominator)}，点击查看相关场景"><rect class="lgi-tm-station-focus" x="${x-120}" y="${y-75}" width="240" height="198" rx="5"/><path class="lgi-tm-station-base-side" d="m${x-65} ${y+1} 65 32 65-32v11l-65 34-65-34Z"/><path class="lgi-tm-station-base-top" d="m${x-65} ${y+1} 65-33 65 33-65 32Z"/><g transform="translate(${x-27},${y-62})" class="lgi-tm-station-art"><path class="lgi-tm-station-art-top" d="m0 17 31-16 24 13-31 16Z"/><path class="lgi-tm-station-art-side" d="M0 17v32l24 14V30Z"/><path class="lgi-tm-station-art-front" d="m24 30 31-16v32L24 63Z"/><path class="lgi-tm-station-art-detail" d="m5 25 12 6m-12 1 12 6m22-30v-11"/><circle class="lgi-tm-station-art-pin" cx="39" cy="-4" r="2.5"/><svg x="29" y="27" width="18" height="22" viewBox="0 0 24 24" class="lgi-tm-station-glyph"><path d="${glyphs[i]}"/></svg></g><text x="${x-89}" y="${y-35}" class="lgi-tm-station-index">0${i+1}</text><text x="${x}" y="${y+62}" text-anchor="middle" class="lgi-tm-station-label">${esc(station.label)}</text><text x="${x}" y="${y+81}" text-anchor="middle" class="lgi-tm-station-question">${questions[i]}</text><text x="${x-8}" y="${y+111}" text-anchor="end" class="lgi-tm-station-count">${number(station.count)}</text><text x="${x+2}" y="${y+109}" class="lgi-tm-station-share">篇 · ${pct(station.count,j?.denominator)}</text>${state.overlay ? `<circle class="lgi-tm-station-overlay" cx="${x+64}" cy="${y-23}" r="5"/>` : ''}</g>`;}).join('')}<text x="22" y="444" class="lgi-tm-route-note">内容覆盖，不是个人分布或真实流转率。受阻与转换不增加主阶段。</text></svg></div>`;
    const ns = selectedWorks().filter(w => !state.stage || (state.metric === 'involved' ? list(w.involvedStages).includes(state.stage) : w.mainStage === state.stage));
    return `<main class="lgi-tm-journey"><div class="lgi-tm-object-head"><div><span class="lgi-tm-eyebrow">内容回应的经历</span><h2>这些作品，覆盖了哪一段经历？</h2><p>${number(j?.denominator)} 篇可读作品 · 主阶段描述内容，不给作者贴人生标签。</p></div>${btn('阶段与统计依据','method')}</div><div class="lgi-tm-toolbar">${[['','全部状态'],['obstruction_recurrence','受阻与反复'],['transition_handoff','环境转换与支持交接']].map(([id,name]) => tab(name,'overlay',id,state.overlay === id)).join('')}<span class="lgi-tm-grow"></span>${tab('主阶段占比','metric','main',state.metric === 'main')}${tab('涉及率','metric','involved',state.metric === 'involved')}${tab('地图','map-mode','map',state.mapMode === 'map')}${tab('列表','map-mode','list',state.mapMode === 'list')}</div>${state.mapMode === 'map' ? map : `<div class="lgi-tm-stage-list">${stations.map((s,i) => `<button class="lgi-tm-panel ${state.stage === s.stage ? 'is-active' : ''}" data-action="stage" data-id="${s.stage}"><span class="lgi-tm-eyebrow">0${i+1}</span><h3>${esc(s.label)}</h3><p>${questions[i]}</p><strong class="lgi-tm-value">${number(s.count)}</strong><p>${pct(s.count,j?.denominator)} · 当前范围作品</p></button>`).join('')}</div>`}<div class="lgi-tm-row lgi-tm-other-stages">${journeyEntries(j).filter(s => !stageNames[s.stage]).map(s => btn(`${esc(otherNames[s.stage] || '其他 / 未归主阶段')} ${number(s.count)} · ${pct(s.count,j.denominator)}`,'stage',s.stage,'lgi-tm-quiet')).join('')}${btn('主题 × 旅程','matrix')}</div><p class="lgi-tm-note">${state.metric === 'main' ? '五主阶段加其他类别以全部可读作品为分母；保留未分析，不舍弃未知。' : '同篇可以实质涉及多个阶段，涉及率合计可以超过100%。'}${state.overlay ? ' 已应用跨阶段覆盖筛选，分母为当前筛选后的作品。' : ''}</p><div class="lgi-tm-section-head"><div><h3>${state.stage ? `${stageNames[state.stage] || otherNames[state.stage] || '当前阶段'}：具体讨论与场景` : '从具体场景，进入关联主题'}</h3><p>阶段不明确时，已有原作和场景仍可查看。</p></div>${state.stage ? btn('回到全部阶段','stage','') : ''}</div>${sceneList(ns)}<div class="lgi-tm-material-grid">${ns.slice(0,6).map(card).join('')}</div>${btn('查看对应全部材料','subset',ns.map(w => w.workRef).join(','),'lgi-tm-quiet')}</main>`;
  }
  function syncContext() {
    const current = document.getElementById('topic-map-crumb-current');
    if (current) current.textContent = (!loading && !readError && state.topicRef ? topicName(state.topicRef) : state.view === 'journey' ? '用户生命旅程' : '主题概览');
    const picker = document.querySelector('.v7-domain-picker');
    if (!picker) return;
    let selected = null;
    picker.querySelectorAll('nav a').forEach(link => {
      if (new URL(link.href, location.origin).searchParams.get('domain') === state.domainRef) {
        link.setAttribute('aria-current','page'); selected = link;
      } else link.removeAttribute('aria-current');
    });
    if (selected) {
      picker.querySelector('summary span').textContent = selected.querySelector('span').textContent;
      picker.querySelector('summary small').textContent = selected.querySelector('small').textContent;
    }
  }
  function render() {
    const focused = document.activeElement;
    const keepFocus = focused?.id && root.contains(focused) ? focused.id : null;
    content.innerHTML = `${header()}${controls()}<div data-notice class="lgi-tm-feedback" role="status" ${notice ? '' : 'hidden'}>${esc(notice)}</div>${readError ? `<div class="lgi-tm-read-error" role="alert"><strong>当前读取未成功</strong><p>${esc(readError)}</p>${btn('重新读取','refresh')}${snapshot ? '<p>上一次结果保留在本次会话中，当前范围读取未成功，暂不混用旧结果展示。</p>' : ''}</div>` : ''}${loading ? '<div class="lgi-tm-loading" role="status">正在读取已有材料与统计…</div>' : ''}${loading || readError ? '' : snapshot ? changesBar() + (state.view === 'overview' ? overview() : journey()) : empty('当前图谱尚未读取','页面读取不启动模型研究或平台采集。')}<footer class="lgi-tm-footer"><span>${esc(snapshot?.methodVersion || '方法尚未读取')}</span><span>样本统计、来源材料与研究判断分别保留边界</span></footer>${list(state.compare).length ? `<div class="lgi-tm-compare-dock"><span>已选 ${state.compare.length} 个方向 · ${state.compare.map(topicName).map(esc).join(' / ')}</span>${btn('并排比较','compare','','lgi-tm-primary')}${btn('清空','compare-clear','','lgi-tm-quiet')}</div>` : ''}`;
    if (keepFocus) document.getElementById(keepFocus)?.focus({preventScroll:true});
    renderNotice();
    syncContext();
  }
  function openDialog(type, data = {}, push = true) {
    if (dialog && push && dialogState) dialogStack.push({...dialogState,scroll:dialog.querySelector('.lgi-tm-dialog-body')?.scrollTop || 0});
    if (!dialog) {
      triggerElement = document.activeElement;
      dialog = document.createElement('dialog'); dialog.className = 'lgi-tm-dialog'; dialog.setAttribute('aria-labelledby','topic-map-dialog-title'); root.appendChild(dialog);
      dialog.addEventListener('click', event => { if (event.target === dialog) closeDialog(); else { event.stopPropagation(); dispatchClick(event); } });
      dialog.addEventListener('cancel', event => { event.preventDefault(); closeDialog(); });
      dialog.addEventListener('submit', submitDialog);
      dialog.addEventListener('change', event => { const target = event.target; if(target.hasAttribute('data-sample-choice')){if(dialog.querySelectorAll('[data-sample-choice]:checked').length>10){target.checked=false;dialogFeedback('本次精选最多10篇独立作品，可先取消现有样本再替换。');}return;} if(target.dataset.detailChoice){const chosen=dialog.querySelectorAll(`[data-detail-choice="${target.dataset.detailChoice}"]:checked`);if(chosen.length>10){target.checked=false;dialogFeedback('一轮最多冻结10篇独立作品详情。');}return;} if (target.dataset.structureSource) { const set = new Set(list(dialogState.sources)); target.checked ? set.add(target.dataset.structureSource) : set.delete(target.dataset.structureSource); dialogState.sources = [...set]; drawDialog(); return; } if (target.dataset.dialogFilter) { dialogState[target.dataset.dialogFilter] = target.value; drawDialog(); } });
    }
    dialogState = {type,...data}; drawDialog(); if (!dialog.open) dialog.showModal();
  }
  function drawDialog() {
    if (!dialog || !dialogState) return;
    const {type} = dialogState;
    const names = {capture:'有界补充材料',alternative:'保存的原版本与依据',move:'调整主题归属',structure:'主题结构调整',matrix:'主题 × 旅程',compare:'并排比较',materials:'全部材料',reader:'原作与讨论',judge:'选题判断',identity:'我方矩阵与观察范围',saved:'我的备选',save:'保留角度与依据',research:'研究进度与设置',rules:'样本表现规则',method:'方法与统计边界',changes:'结构与依据变化',product:'产品机会：需求与支持假设',replace:'替换本次判断样本'};
    dialog.innerHTML = `<div class="lgi-tm-dialog-head"><div><span class="lgi-tm-eyebrow">${esc(scopeText())}</span><h2 id="topic-map-dialog-title">${esc(names[type] || '查看材料')}</h2></div><div class="lgi-tm-row">${dialogStack.length ? btn('返回上一层','dialog-back') : ''}${btn('关闭','dialog-close','','', 'aria-label="关闭对话框，保留研究任务"')}</div></div><div class="lgi-tm-dialog-body">${dialogBody()}</div><div class="lgi-tm-dialog-feedback" role="status" data-dialog-feedback></div>`;
    dialog.querySelector('.lgi-tm-dialog-body').scrollTop = dialogState.scroll || 0;
  }
  function closeDialog() {
    if (!dialog) return; dialog.close(); dialog.remove(); dialog = null; dialogState = null; dialogStack = []; triggerElement?.focus({preventScroll:true});
  }
  function dialogBack() {
    if (!dialogStack.length) return closeDialog(); dialogState = dialogStack.pop(); drawDialog();
  }
  function dialogFeedback(text) { const target = dialog?.querySelector('[data-dialog-feedback]'); if (target) target.textContent = text; }
  function dialogBody() {
    if (dialogState.error) return empty('此区域读取未成功',dialogState.error) + btn('重试','dialog-retry');
    const type = dialogState.type;
    if (type === 'materials') return materialsDialog();
    if (type === 'reader') return readerDialog();
    if (type === 'judge') return judgmentDialog();
    if (type === 'replace') return replaceSamplesDialog();
    if (type === 'matrix') return matrixDialog();
    if (type === 'compare') return compareDialog();
    if (type === 'identity') return dialogState.loading ? '<p role="status">正在读取领域账号与完整我方范围。</p>' : identityDialog();
    if (type === 'rules') return rulesDialog();
    if (type === 'saved') return savedDialog();
    if (type === 'alternative') return alternativeDialog();
    if (type === 'save') return saveDialog();
    if (type === 'research') return researchDialog();
    if (type === 'product') return productDialog();
    if (type === 'changes') return changesDialog();
    if (type === 'structure') return structureDialog();
    if (type === 'move') return moveDialog();
    if (type === 'capture') return captureDialog();
    return `<h3>当前方法与范围</h3><p>${esc(snapshot.methodVersion)}</p><p>${esc(snapshot.sourceBoundary)}</p><div class="lgi-tm-callout">主阶段：发现与理解、求助与评估、选择支持、开始实践、长期管理。受阻与转换为覆盖层，不能加进主阶段分布。</div><p>主阶段分母为当前范围的全部可读独立作品；涉及率可以重叠，合计可以超过100%。五阶段之外保留跨阶段综合、通用背景、阶段不明和待分析。</p><p>样本爆款依据已保存的平台规则，低粉高赞固定粉丝≤1,000且点赞≥500，我方爆款由手动标记决定，三者互不替代。</p><p>已有来源材料不直接证明需求、趋势、因果或产品价值；原文与模型解释分开。</p>`;
  }
  function materialsDialog() {
    const source = dialogState.snapshot || snapshot;
    let ns = list(source.works);
    if (Array.isArray(dialogState.refs)) ns = ns.filter(w => dialogState.refs.includes(w.workRef));
    if (dialogState.unassigned) ns = ns.filter(w => !list(w.topicRefs).length);
    const q = dialogState.query || '', sort = dialogState.sort || 'latest';
    ns = ns.filter(w => !q || [title(w),w.creatorDisplayName,w.preview?.text].join(' ').includes(q));
    if (sort !== 'latest') ns.sort((a,b) => a.platform.localeCompare(b.platform) || (b[sort] ?? -1) - (a[sort] ?? -1));
    else ns.sort((a,b) => String(b.publishedAt || '').localeCompare(String(a.publishedAt || '')));
    const limit = dialogState.limit || 60;
    return `<p>${esc(dialogState.label || '当前范围的全部作品')} · ${ns.length} 篇已读取作品。原始材料不受精选10篇上限限制。</p><div class="lgi-tm-toolbar"><label class="lgi-tm-control"><span>材料排序 / 同平台分组</span><select data-dialog-filter="sort">${[['latest','最新发布时间'],['likes','点赞'],['collects','收藏'],['comments','评论'],['shares','分享']].map(([v,t]) => `<option value="${v}" ${sort === v ? 'selected' : ''}>${t}</option>`).join('')}</select></label><form data-form="material-search" class="lgi-tm-row"><input name="query" value="${esc(q)}" placeholder="查找标题 / 作者" aria-label="查找材料"><button type="submit" class="lgi-tm-button">查找</button></form></div><div class="lgi-tm-material-grid">${ns.slice(0,limit).map(card).join('')}</div>${ns.length > limit ? btn(`展开其余 ${ns.length-limit} 篇`,'materials-more') : ''}${ns.length ? '' : empty('当前条件没有匹配的已读取作品')}<p class="lgi-tm-note">未知数字显示—；跨平台互动不混合作统一排名。</p>`;
  }
  function unreadResearchWork(w) { return {...w,research:null,annotation:null,evidenceFragment:null,mainStage:null,involvedStages:[],overlays:[]}; }
  function researchQualificationUrl(w,domainRef,preferredTopicRef) {
    const q=new URLSearchParams({domainRef,referenceWindowDays:'0'}), refs=list(w.topicRefs), topicRef=refs.includes(preferredTopicRef)?preferredTopicRef:refs[0];
    if(topicRef)q.set('topicRef',topicRef);
    return `/api/local/topic-map?${q}`;
  }
  function applyQualifiedReader(reader,values) {
    reader.resource=values[0].status==='fulfilled'?values[0].value:null;
    reader.resourceError=values[0].status==='rejected'?values[0].reason.message:'';
    reader.commentResource=values[1].status==='fulfilled'?values[1].value:null;
    reader.commentError=values[1].status==='rejected'?values[1].reason.message:'';
    const qualified=values[2].status==='fulfilled'?list(values[2].value?.works).find(w=>w.workRef===reader.work.workRef):null;
    reader.qualificationError=values[2].status==='rejected'?values[2].reason.message:!qualified?'当前范围未返回这篇作品的来源资格，旧研究不继续展示。':'';
    reader.work=qualified?{...qualified,annotation:null}:unreadResearchWork(reader.work);
    if(reader.qualificationError||(reader.resourceError&&!commentOnlyResearch(reader.work)))reader.work=unreadResearchWork(reader.work);
    reader.loading=false;
  }
  async function readWork(ref) {
    const w=work(ref) || dialogState?.snapshot?.works?.find(w=>w.workRef===ref);
    if(!w)return;
    const domainRef=state.domainRef, preferredTopicRef=dialogState?.topicRef || state.topicRef;
    openDialog('reader',{work:unreadResearchWork(w),domainRef,loading:true});const current=dialogState;
    const domain=encodeURIComponent(domainRef),values=await Promise.allSettled([request(`/api/local/work-resources/${encodeURIComponent(ref)}?domain=${domain}`),request(`/api/local/work-resources/${encodeURIComponent(ref)}/comments?domain=${domain}`),request(researchQualificationUrl(w,domainRef,preferredTopicRef))]);
    applyQualifiedReader(current,values);
    if(dialogState===current)drawDialog();
  }
  function fragmentText(fragment) { return typeof fragment === 'string' ? fragment : fragment?.text || ''; }
  function sourceKind(kind) { return ({detail_body:'作者正文',ocr_text:'图片OCR',image_substantive_text:'图片原文',frame_ocr_text:'视频帧OCR',asr_text:'音视频转写',ocr:'图片OCR',transcript:'音视频转写',title:'作品标题',comment_body:'评论原声',comment:'评论原声',clean_comment:'评论原声',unresearched_comment:'评论原声',studied_comment:'评论原声',parent_comment_context:'父评论上下文',body:'作者正文'}[kind] || '来源片段'); }
  function highlighted(text, spans = []) {
    const chars = Array.from(text || ''), valid = list(spans).filter(s => Number.isInteger(s.start) && Number.isInteger(s.end) && s.start >= 0 && s.end > s.start && s.end <= chars.length).sort((a,b) => a.start - b.start);
    const merged=[];valid.forEach(s=>{const previous=merged[merged.length-1];if(previous&&s.start<=previous.end)previous.end=Math.max(previous.end,s.end);else merged.push({start:s.start,end:s.end});});
    let position = 0, out = '';
    merged.forEach(s => { out += esc(chars.slice(position,s.start).join('')) + `<mark>${esc(chars.slice(s.start,s.end).join(''))}</mark>`; position = s.end; });
    return out + esc(chars.slice(position).join(''));
  }
  function readerFragmentInBody(w,f,raw) {
    if(f.field!=='body'||!String(f.fragmentId).startsWith(`${w.workRef}.body.`)||!raw)return false;
    const base=Number.isInteger(f.start)?f.start:0, text=fragmentText(f);
    return Boolean(text)&&Array.from(raw).slice(base,base+Array.from(text).length).join('')===text;
  }
  function readerTitleFragment(w,f) { return f.field==='title'&&String(f.fragmentId).startsWith(`${w.workRef}.title.`)&&fragmentText(f)===title(w); }
  function readerCitationTarget(w,c,raw) {
    return readerFragmentInBody(w,c.fragment,raw)?`lgi-tm-source-body-${w.workRef}`:readerTitleFragment(w,c.fragment)?`lgi-tm-source-title-${w.workRef}`:`lgi-tm-source-fragment-${c.fragmentId}`;
  }
  function readerDiscussions(w,raw) {
    if(!w.research)return '';
    const units=discussionUnits(w);
    return `${researchCoverage(w)}<section class="lgi-tm-reader-discussions"><h4>具体讨论与主题归属</h4>${units.length?units.map(d=>discussionUnitView({work:w,discussion:d},{raw})).join(''):empty(Array.isArray(w.research.core?.units)?'当前已处理范围未形成具体讨论':'旧研究尚无具体讨论','已有原文与其他研究结果继续可读。')}</section>`;
  }
  function readerDialog(reader = dialogState) {
    const w = reader.work, r = reader.resource;
    const item = r?.item, research=!reader.loading&&!reader.qualificationError&&(!reader.resourceError||commentOnlyResearch(w))?w.research:null, body=item?.inspector?.detailCurrent?.body?.value;
    const raw=w.readable===false?'':body || fragmentText(item?.evidenceFragment);
    const fragments=list(research?.fragments).filter(f=>!(readerFragmentInBody(w,f,raw)||readerTitleFragment(w,f)));
    const comments = list(reader.commentResource?.items);
    const source = '';
    const analysis=research?.output || {}, qualifiedWork={...w,research};
    return `<div class="lgi-tm-reader-head"><div><h3 id="lgi-tm-source-title-${esc(w.workRef)}">${esc(title(w))}</h3>${materialSourceNote(w)}<p>${avatar(w.creatorDisplayName)}${esc(w.creatorDisplayName || '作者未知')} · ${esc(platformName(w.platform))} · ${esc(w.publishedAtSourceText || w.publishedAt || '发布时间未知')}</p><div class="lgi-tm-row">${badge(w.titleSource === 'cover_ocr' ? '标题来自封面OCR' : '作品标题')}${w.own ? btn(w.ownBreakout ? '取消我方爆款标记' : '标记我方爆款','breakout',w.workRef,'', 'data-mutation') : ''}${source ? `<a class="lgi-tm-button" href="${esc(source)}" target="_blank" rel="noopener noreferrer">打开来源页面</a>` : ''}</div></div><div class="lgi-tm-interactions">赞 ${number(w.likes)} · 藏 ${number(w.collects)} · 评 ${number(w.comments)} · 分享 ${number(w.shares)}</div></div><div class="lgi-tm-reader-layout"><section><h4>原文与可定位片段</h4>${reader.loading ? '<p role="status">正在复核原文、评论与研究来源资格；旧研究片段和解释暂不展示。</p>' : ''}${reader.resourceError ? `<p class="lgi-tm-read-error">原文读取未成功：${esc(reader.resourceError)}</p>` : ''}${raw ? `<div class="lgi-tm-original" id="lgi-tm-source-body-${esc(w.workRef)}">${highlighted(raw,bodyCitations(qualifiedWork,raw))}</div>${!body ? '<p class="lgi-tm-note">当前为有界来源片段；完整原文读取与来源资格以材料检查器为准。</p>' : ''}` : empty('当前没有可读取的正文片段','缺少正文不等于原作没有回应。')}${fragments.map(f => `<article class="lgi-tm-fragment" id="lgi-tm-source-fragment-${esc(f.fragmentId)}"><span>${esc(sourceKind(f.sourceKind || f.field))}</span><blockquote>${highlighted(fragmentText(f),citationSpans(qualifiedWork,f.fragmentId))}</blockquote></article>`).join('')}<a class="lgi-tm-button" href="/corpus/evidence?work=${encodeURIComponent(w.workRef)}&domain=${encodeURIComponent(state.domainRef)}">在材料检查器定位</a></section><aside class="lgi-tm-analysis">${readerDiscussions(qualifiedWork,raw)}<h4>模型解释 / 已保存版本</h4>${reader.qualificationError ? `<p class="lgi-tm-read-error">研究来源资格复核未完成：${esc(reader.qualificationError)}</p>` : ''}${analysis.journey?.rationale ? `<p>${esc(analysis.journey.rationale)}</p>` : '<p>当前没有通过来源资格检查的可引用研究解释，不用旧摘要代替原声。</p>'}<p>${esc(stageNames[analysis.journey?.mainStage] || otherNames[analysis.journey?.mainStage] || '阶段解释未取得')} · ${esc(list(analysis.journey?.overlays).map(v => v === 'obstruction_recurrence' ? '受阻与反复' : '环境转换与交接').join('、'))}</p>${list(analysis.limitations).map(t => `<p class="lgi-tm-note">${esc(t)}</p>`).join('')}<h4>已有回应关系</h4>${list(analysis.responseMatches).map(m => `<p>${esc(({direct:'有直接回应依据',partial:'部分回应',unmatched:'未找到匹配回应',unknown:'回应未知',not_applicable:'当前不适用'})[m.status] || m.status)}</p>`).join('') || '<p>回应尚未研究，不能把评论问题自动当原作回答。</p>'}</aside></div><section class="lgi-tm-section"><div class="lgi-tm-section-head"><div><h3>已有评论原声</h3><p>评论条数不等于人数，作者观点与评论分别保留来源。</p></div>${btn('深入看这篇的讨论','deep-comments',w.workRef,'', 'data-mutation')}</div>${reader.commentError ? `<p class="lgi-tm-read-error">评论读取未成功：${esc(reader.commentError)}</p>` : ''}<p class="lgi-tm-note">当前展示 ${comments.length} / ${number(reader.commentResource?.total)} 条当前可读评论。</p>${comments.length ? comments.map(c => `<article class="lgi-tm-comment"><blockquote>${esc(c.body || '正文未知')}</blockquote><p>${esc('评论作者身份已隐去')} · ${esc(c.sourceRef || '')}</p>${c.parentText ? `<details><summary>父评论上下文</summary><blockquote>${esc(c.parentText)}</blockquote></details>` : ''}</article>`).join('') : empty(reader.loading ? '评论正在读取' : '当前没有可展示的已有评论','不会为了填满原声区域发起采集。')}${reader.commentResource?.nextCursor ? btn('读取下一页已有评论','comments-more') : ''}</section>`;
  }
  function curated(ref, source = snapshot) {
    const t = list(source.topics).find(t => t.topicRef === ref);
    const candidates = list(source.works).filter(w => readableMaterial(w) && (!t || list(w.topicRefs).includes(ref) || list(t.workRefs).includes(w.workRef)));
    const recent = new Set(list(source.scope.recentReferenceWorkRefs));
    const eligible = candidates.filter(w => w.own ? Boolean(w.publishedAt || w.publishedAtSourceText) : recent.has(w.workRef));
    const high = eligible.filter(w => w.own ? w.ownBreakout : (known(w.followerCount) && w.followerCount <= 1000 && known(w.likes) && w.likes >= 500) || (known(platformStats(source.statistics).find(p => p.platform === w.platform)?.highPerformanceThreshold) && known(w.likes) && w.likes >= platformStats(source.statistics).find(p => p.platform === w.platform).highPerformanceThreshold));
    high.sort((a,b) => Number(b.ownBreakout)-Number(a.ownBreakout) || a.platform.localeCompare(b.platform) || (b.likes ?? -1)-(a.likes ?? -1));
    const picked = high.slice(0,10);
    const contrasts = eligible.filter(w => !picked.some(p => p.workRef === w.workRef) && picked.some(p=>p.platform===w.platform&&sharesDiscussion(p,w)));
    if (!picked.length) return eligible.slice().sort((a,b) => String(b.publishedAt || '').localeCompare(String(a.publishedAt || ''))).slice(0,3);
    const result = [], chosen = new Set();
    const add = w => { if (result.length < 10 && !chosen.has(w.workRef)) { result.push(w); chosen.add(w.workRef); } };
    for (const sample of picked) {
      add(sample);
      if (!sample.own) {
        contrasts.filter(w => w.own && w.platform === sample.platform && sharesDiscussion(sample,w)).forEach(add);
      }
    }
    contrasts.forEach(add);
    return result;
  }
  function ownTopicWorks(ref, source = snapshot) {
    const t = list(source?.topics).find(t => t.topicRef === ref);
    return list(source?.works).filter(w => w.own && Boolean(w.publishedAt || w.publishedAtSourceText) && (!t || list(w.topicRefs).includes(ref) || list(t.workRefs).includes(w.workRef)));
  }
  function analysisOutputs(ns) { return ns.map(w => ({work:w,output:w.research?.output || {}})); }
  function judgmentDialog() {
    const ref=dialogState.topicRef,source=dialogState.snapshot || snapshot,t=list(source.topics).find(t=>t.topicRef===ref) || topic(ref),ns=dialogState.works || curated(ref),allOwn=ownTopicWorks(ref,source),mode=dialogState.tab || 'discussion';
    const external=list(source.works).filter(w=>!w.own&&list(source.scope.recentReferenceWorkRefs).includes(w.workRef));
    const tabs=[['discussion','方向与讨论'],['samples',`精选样本 · ${ns.length}/10`],['own','我方对照与复用'],['angles','角度与标题']];
    return `<div class="lgi-tm-object-head"><div><span class="lgi-tm-eyebrow">方向判断 · 定义 v${number(t?.definitionVersion)}</span><h3>${esc(t?.displayName || '当前方向')}</h3><p>${esc(t?.definitionText)}</p><p>我方全部发布历史 / 外部${state.referenceWindowDays==='0'?'全部历史':'近'+state.referenceWindowDays+'天发布'}参考；独立于概览统计窗口。</p></div>${btn('查看全部材料','materials')}</div>${topicBoundary(t,source)}${dialogState.loading ? '<p role="status">正在补齐全历史我方作品与独立外部参考范围，已有材料先显示。</p>' : ''}${dialogState.readError ? `<p class="lgi-tm-read-error">完整范围读取未成功：${esc(dialogState.readError)}</p>` : ''}<div class="lgi-tm-judge-facts"><div><strong>${list(source.works).length}</strong><span>篇当前相关材料</span></div><div><strong>${allOwn.length}</strong><span>篇已确认我方发布</span></div><div><strong>${external.filter(w=>known(w.followerCount)&&w.followerCount<=1000&&known(w.likes)&&w.likes>=500).length}</strong><span>篇近期低粉高赞参考</span></div></div><nav class="lgi-tm-work-tabs" aria-label="选题判断内容">${tabs.map(([id,name])=>tab(name,'judge-tab',id,mode===id)).join('')}</nav><div class="lgi-tm-judgment-grid"><div class="lgi-tm-judgment-main">${mode==='samples'?sampleView(ns):mode==='own' ? ownView(ref,allOwn,ns) : mode==='angles' ? anglesView(ns) : discussionsView(ns)}</div><aside class="lgi-tm-judgment-rail"><section class="lgi-tm-rail-box"><h3>这一轮，先判断什么？</h3><p>从同题高表现中找借鉴，用同讨论对照检查条件。先看原文，再决定角度。</p><div class="lgi-tm-rail-stats"><div><b>${ns.length}</b><span>精选作品 / 最多10</span></div><div><b>${ns.reduce((n,w)=>n+list(w.research?.fragments).filter(f=>String(f.field).endsWith('comment')).length,0)}</b><span>可定位的评论来源片段</span></div></div>${btn(mode==='angles'?'回到材料对照':'找角度与标题','judge-tab',mode==='angles'?'samples':'angles','lgi-tm-primary')}${btn('查看全部相关材料','materials','','lgi-tm-quiet')}</section><section class="lgi-tm-rail-box"><h3>依据与资料补充</h3><div class="lgi-tm-rail-links">${btn('高表现参考来自谁','judge-sources')}${btn('本次补充与真实进度','capture',ref)}${btn('放回旅程地图','judge-to-journey',ref)}${btn('查看有限产品机会','judge-product',ref)}</div><p>保存只保留方向与依据，不新建监控。反例和材料不足都继续保留。</p></section><section class="lgi-tm-rail-box"><h3>已有结果继续可读</h3><p>研究仅在明确预算与当前最多10篇冻结集合内启动。</p>${btn('研究当前精选','start-topic-research',ref,'', 'data-mutation')}${btn('研究进度与设置','research','','lgi-tm-quiet')}</section></aside></div>`;
  }
  function discussionsView(ns) {
    const groups=discussionGroups(ns);
    return `<div class="lgi-tm-section-head"><div><h3>同一主题，先分清几种具体讨论</h3><p>按已保存的主题身份归组；同名但边界不同的讨论分别保留。支持、反例和背景可以属于同一主题。</p>${researchCoverageSummary(ns)}</div></div>${groups.map((g,i)=>`<article class="lgi-tm-discussion"><div class="lgi-tm-discussion-head"><span class="lgi-tm-discussion-index">${String(i+1).padStart(2,'0')}</span><div><h3>${esc(g.label)}</h3><p>${g.rows.length} 条独立讨论 · ${g.works.length} 篇精选作品 · ${g.evidenceCount} 条去重定位依据</p>${g.canonical?'':'<p class="lgi-tm-note">主题身份尚未确认，不与其他同名材料自动合组。</p>'}</div></div>${discussionGroupDetails(g)}</article>`).join('') || empty('具体讨论关系尚待研究','原作可以先读，不用等分析完成。')}<section class="lgi-tm-section"><h3>高低表现：先看差异，不先下归因</h3>${performancePair(ns)}<p class="lgi-tm-note">同题不代表曝光、作者条件与发布时间可比。这里是材料对照，不是写法导致爆款的结论。</p></section><section class="lgi-tm-section"><h3>实际场景与来源</h3>${sceneList(ns)}</section>`;
  }
  function performancePair(ns) {
    const high=ns.find(w=>!w.own&&known(w.followerCount)&&w.followerCount<=1000&&known(w.likes)&&w.likes>=500);
    const low=high&&ns.find(w=>!w.own&&w.workRef!==high.workRef&&w.platform===high.platform&&known(w.likes)&&w.likes<=20&&sharesDiscussion(high,w));
    if(!high||!low)return empty('当前同讨论、同平台的高低对照尚不充分','先看已有材料，不强凑避坑结论。低互动不等于失败。');
    const common=new Set([...discussionKeys(high)].filter(key=>discussionKeys(low).has(key)));
    return `<div class="lgi-tm-performance-pair">${[high,low].map((w,i)=>{const d=discussionUnits(w).find(d=>d.assignments.some(a=>common.has(assignmentKey(a)))),quote=citationText(w,d?.evidence);return `<article class="lgi-tm-panel">${badge(i?'低互动对照 · 非失败判定':'低粉高赞参考',!i)}<h4>${esc(title(w))}</h4><p>${esc(platformName(w.platform))} · ${number(w.likes)}赞 · 粉丝${number(w.followerCount)} · ${esc(w.publishedAtSourceText || w.publishedAt || '发布时间未知')}</p>${quote?`<blockquote>${esc(quote)}</blockquote>`:'<p>当前没有可展示的精确引用，请核对原作。</p>'}${btn(readLabel(w,'打开原文与已有评论'),'reader',w.workRef,'lgi-tm-quiet')}</article>`;}).join('')}</div>`;
  }
  function anglesFrom(ns) { return analysisOutputs(ns).flatMap(({work,output})=>list(output.angles).map((a,index)=>({...a,workRef:work.workRef,researchResultRef:a.researchResultRef || work.research?.resultRef,researchMethodVersion:work.research?.methodVersion,researchAngleIndex:Number.isInteger(a.researchAngleIndex)?a.researchAngleIndex:index,limitations:list(output.limitations)}))).filter(a=>typeof a.researchResultRef==='string'&&a.researchResultRef.length>0); }
  function anglesView(ns) {
    const angles=anglesFrom(ns);
    return `<div class="lgi-tm-section-head"><div><h3>先选不同角度，再打磨标题</h3><p>角度与回答任务来自当前已保存的研究版本，差异与回答边界需要继续核对原作。</p></div></div>${angles.length ? angles.slice(0,6).map((a,i)=>`<article class="lgi-tm-angle">${badge(`${String(i+1).padStart(2,'0')} · ${a.label}`)}<h3>${esc(a.title)}</h3><p>这篇要回答：${esc(a.answerTask)}</p><p class="lgi-tm-note">与我方已有内容的差别尚待核对，不由标题相似推断覆盖。</p>${a.limitations.map(value=>`<p class="lgi-tm-note">研究限制：${esc(value)}</p>`).join('')}<div class="lgi-tm-row">${btn('核对原作与引用','reader',a.workRef)}${btn('选这个角度，打磨标题','save-angle',String(i),'lgi-tm-primary')}</div></article>`).join('') : empty('当前没有已研究、可引用的不同角度','已有作品可继续读；明确开启研究后才会产生模型角度。')}`;
  }
  function ownView(ref,own,ns) {
    if(snapshot.scope.ownIdentityState==='unknown')return empty('我方矩阵未设置','不能推断全部主题都没有做过。')+btn('设置我方矩阵','identity');
    return `<div class="lgi-tm-section-head"><div><h3>我方原来讲了什么，外部还有什么？</h3><p>整个已确认矩阵的发布历史；标题相近不等于具体问题已经覆盖。</p></div>${btn('展开全部我方原作','judge-own-all')}</div>${own.map(w=>`<article class="lgi-tm-own-work">${badge(w.ownBreakout?'我方手动爆款':'我方已发布',w.ownBreakout)}<h3>${esc(title(w))}</h3><p>${esc(fragmentText(w.evidenceFragment)) || '当前没有安全可展示的原文片段，请打开原作核对。'}</p><p class="lgi-tm-note">${esc(w.creatorDisplayName || '作者未知')} · ${esc(w.publishedAtSourceText || w.publishedAt)} · ${number(w.likes)}赞</p><div class="lgi-tm-row">${btn(readLabel(w,'看原题、封面与正文'),'reader',w.workRef)}${btn('保留复用意图','own-reuse',w.workRef,'lgi-tm-quiet',readableMaterial(w)?'':'disabled title="当前没有可引用来源"')}</div></article>`).join('') || empty('当前已入库发布历史中尚未观察到本主题我方原作','部分覆盖不代表全矩阵从未做过。')}<section class="lgi-tm-section"><h3>相对我方，继续检查哪些差别？</h3><p>按同一主题身份下的讨论并列查看。是否回答了不同问题、遗漏哪些条件，由原作和实际回应共同核对；同名未知归属分别保留。</p>${discussionGroups(ns).map(g=>{const mine=g.works.filter(w=>w.own),others=g.works.filter(w=>!w.own);return `<article class="lgi-tm-panel"><h4>${esc(g.label)}</h4><p class="lgi-tm-note">${g.rows.length} 条独立讨论 · ${g.evidenceCount} 条去重定位依据</p><div class="lgi-tm-own-comparison"><div><h5>我方原作</h5>${mine.map(w=>btn(esc(title(w)),'reader',w.workRef,'lgi-tm-quiet')).join('') || '<p>当前精选无对应原作。</p>'}</div><div><h5>外部参考</h5>${others.map(w=>btn(esc(title(w)),'reader',w.workRef,'lgi-tm-quiet')).join('') || '<p>当前精选无对应外部参考。</p>'}</div></div>${discussionGroupDetails(g)}</article>`;}).join('') || empty('当前尚无可引用的讨论差异分组','可先核对原文与已有评论，不补写差异结论。')}</section>`;
  }
  function sampleReason(w) {
    if(w.own)return w.ownBreakout?'我方手动爆款':'我方已发布原作';
    if(known(w.followerCount)&&w.followerCount<=1000&&known(w.likes)&&w.likes>=500)return '外部低粉高赞参考';
    const p=platformStats(dialogState.snapshot?.statistics || snapshot.statistics).find(p=>p.platform===w.platform);
    return known(w.likes)&&known(p?.highPerformanceThreshold)&&w.likes>=p.highPerformanceThreshold?'同平台高表现参考':'同题材料 / 待核对条件';
  }
  function sampleView(ns) {
    const selected=ns.find(w=>w.workRef===dialogState.sampleRef) || ns[0], reader=dialogState.inlineReader;
    return `<div class="lgi-tm-section-head"><div><h3>最多10篇，先读有用的对照</h3><p>我方原作也计入精选上限。不为凑数加入无关内容，替换只改变本次阅读集合。</p></div>${btn('替换当前精选','replace-samples','', '',dialogState.loading?'disabled':'')}</div>${ns.length ? `<div class="lgi-tm-sample-grid"><aside class="lgi-tm-sample-list" aria-label="精选判断样本">${ns.map((w,i)=>`<button type="button" class="lgi-tm-sample-item ${w.workRef===selected?.workRef?'is-active':''}" data-action="sample" data-id="${esc(w.workRef)}" aria-pressed="${w.workRef===selected?.workRef}"><span class="lgi-tm-sample-num">${String(i+1).padStart(2,'0')}</span><div><h4>${esc(title(w))}</h4><small>${esc(platformName(w.platform))} · ${number(w.likes)}赞 · ${esc(w.publishedAtSourceText || w.publishedAt || '发布时间未知')}</small>${badge(sampleReason(w))}${materialSourceNote(w)}</div></button>`).join('')}</aside><article class="lgi-tm-inline-reader">${reader?.work?.workRef===selected?.workRef ? readerDialog(reader) : empty('选择左侧作品，读取当前可用材料','来源资格通过读取接口复核，不用模型摘要冒充正文。')}${selected&&!reader ? btn(readLabel(selected,'读取这篇原文'),'sample',selected.workRef) : ''}</article></div>` : empty('当前没有精选材料','可以查看已入库材料，或显式选择当前有界材料集合。')}`;
  }
  function replacementCandidates(parent) {
    const source=parent.snapshot || snapshot, recent=new Set(list(source.scope.recentReferenceWorkRefs)), t=list(source.topics).find(t=>t.topicRef===parent.topicRef), members=new Set(list(t?.workRefs));
    return list(source.works).filter(w=>readableMaterial(w)&&(members.has(w.workRef)||list(w.topicRefs).includes(parent.topicRef))&&(w.own?Boolean(w.publishedAt || w.publishedAtSourceText):recent.has(w.workRef)));
  }
  function replaceSamplesDialog() {
    const parent=dialogState.judgeState,rows=replacementCandidates(parent);
    return `<h3>调整当前最多10篇判断材料</h3><p>每个独立作品只计一次；我方原作也计入10篇。替换不启动新研究或补采，已开启研究保留原冻结集合。</p><form data-form="replace-samples" class="lgi-tm-replacement-list">${rows.map(w=>`<label class="lgi-tm-check-label"><input type="checkbox" name="workRefs" value="${esc(w.workRef)}" data-sample-choice ${list(parent.works).some(p=>p.workRef===w.workRef)?'checked':''}><span><strong>${esc(title(w))}</strong><br>${esc(platformName(w.platform))} · ${number(w.likes)}赞 · ${w.own?'我方已发布':'当前外部参考'}</span></label>`).join('') || empty('当前范围没有可加入的可读作品','不从其他主题自动填充。')}<button type="submit" class="lgi-tm-button lgi-tm-primary">更新本次精选</button></form>`;
  }
  async function readSample(ref) {
    const current=dialogState;if(current?.type!=='judge'||!list(current.works).some(w=>w.workRef===ref))return;
    const selected=list(current.works).find(w=>w.workRef===ref),domainRef=state.domainRef,reader={work:unreadResearchWork(selected),domainRef,loading:true};current.sampleRef=ref;current.inlineReader=reader;drawDialog();
    const domain=encodeURIComponent(domainRef),values=await Promise.allSettled([request(`/api/local/work-resources/${encodeURIComponent(ref)}?domain=${domain}`),request(`/api/local/work-resources/${encodeURIComponent(ref)}/comments?domain=${domain}`),request(researchQualificationUrl(selected,domainRef,current.topicRef))]);
    if(current.inlineReader!==reader)return;
    applyQualifiedReader(reader,values);
    if(dialogState?.type==='judge'&&dialogState.inlineReader===reader)drawDialog();
  }
  async function openMatrix() {
    openDialog('matrix',{loading:true,rows:'leaves',metric:state.metric}); const current = dialogState;
    try { current.snapshot = await request(`/api/local/topic-map?${query({topicRef:''})}`); }
    catch (error) { current.error = error.message; }
    if (dialogState === current) { current.loading = false; drawDialog(); }
  }
  function matrixDialog() {
    if (dialogState.loading) return '<p role="status">正在展开全部主题作对照，保留平台、时间、经历路径与覆盖层。</p>';
    const source = dialogState.snapshot, rowMode = dialogState.rows || 'leaves', metric = dialogState.metric || 'main';
    if (!source) return empty('矩阵尚未读取');
    const rows = list(source.topics).filter(t=>t.lifecycleState!=='superseded').filter(t => rowMode === 'roots' ? !t.parentTopicRef : !source.topics.some(c => c.parentTopicRef === t.topicRef));
    return `<p>展开全部主题作对照 · ${esc(scopeText())} · ${metric === 'main' ? '每格独立作品数 ÷ 本行主题全部可读作品' : '每格涉及作品数 ÷ 本行主题全部可读作品，允许重叠'}${state.overlay ? ' · 已应用跨阶段覆盖筛选' : ''}</p><div class="lgi-tm-toolbar"><label class="lgi-tm-control"><span>主题行</span><select data-dialog-filter="rows"><option value="leaves" ${rowMode === 'leaves' ? 'selected' : ''}>全部细分主题</option><option value="roots" ${rowMode === 'roots' ? 'selected' : ''}>一级主题</option></select></label><label class="lgi-tm-control"><span>统计口径</span><select data-dialog-filter="metric"><option value="main" ${metric === 'main' ? 'selected' : ''}>主阶段占比</option><option value="involved" ${metric === 'involved' ? 'selected' : ''}>涉及率</option></select></label></div><div class="lgi-tm-table-scroll"><table class="lgi-tm-matrix"><thead><tr><th>主题 / 行分母</th>${fallbackStages.map(s => `<th>${esc(s.label)}</th>`).join('')}<th>其他 / 未归主阶段</th></tr></thead><tbody>${rows.map(t => {
      const j = t.journey, entries = journeyEntries(j,metric), otherEntries = list(j?.main).filter(e => !stageNames[e.stage]);
      const otherRefs = [...new Set(otherEntries.flatMap(e => list(e.workRefs)))], otherCount = otherEntries.reduce((sum,e) => sum + (known(e.count) ? e.count : 0),0);
      return `<tr class="${t.topicRef === state.topicRef ? 'is-active' : ''}"><th><strong>${esc(t.displayName)}</strong><p>${number(j?.denominator)} 篇可读作品</p></th>${fallbackStages.map(s => {const e = entries.find(e => e.stage === s.stage); return `<td>${btn(`<b>${number(e?.count)}</b><small>${pct(e?.count,j?.denominator)}</small>`,'matrix-cell',`${t.topicRef}|${s.stage}`,'lgi-tm-matrix-cell',`style="--lgi-tm-cell-strength:${known(e?.count)&&j?.denominator>0?Math.min(1,e.count/j.denominator):0}"`)}</td>`;}).join('')}<td>${btn(`<b>${number(otherCount)}</b><small>${pct(otherCount,j?.denominator)}</small>`,'matrix-cell',`${t.topicRef}|other`,'lgi-tm-matrix-cell',`style="--lgi-tm-cell-strength:${j?.denominator>0?Math.min(1,otherCount/j.denominator):0}"`)}<details><summary>类别明细</summary>${otherEntries.map(e => `<p>${esc(otherNames[e.stage] || e.stage)} ${number(e.count)}</p>`).join('')}</details></td></tr>`;
    }).join('')}</tbody></table></div>${rows.length ? '' : empty('当前领域尚无可展开的主题行','已读取材料仍可浏览。')}<p class="lgi-tm-note">主阶段模式包括五阶段与其他。涉及率可能超过100%，其他仍展示尚未归入五主阶段的材料。空格不表示现实中没有需求。</p>`;
  }
  async function openCompare() {
    const refs = list(state.compare).slice(0,3);
    if (!refs.length) return;
    openDialog('compare',{loading:true,refs}); const current = dialogState;
    try { current.snapshot = await request(`/api/local/topic-map?${query({topicRef:''})}`); }
    catch (error) { current.error = error.message; }
    if (dialogState === current) { current.loading = false; drawDialog(); }
  }
  function compareDialog() {
    if (dialogState.loading) return '<p role="status">正在读取所选方向的同一范围统计。</p>';
    const source = dialogState.snapshot, topics = list(source?.topics).filter(t => dialogState.refs.includes(t.topicRef));
    return `<p>${esc(scopeText())} · 各平台分别比较，统计引用同一服务端集合。</p><div class="lgi-tm-compare-grid">${topics.map(t => `<article class="lgi-tm-panel"><h3>${esc(t.displayName)}</h3><p>${esc(t.definitionText)}</p>${topicBoundary(t,source)}<dl class="lgi-tm-facts"><div><dt>去重作品</dt><dd>${number(t.statistics?.workCount)}</dd></div><div><dt>已观察作者</dt><dd>${number(t.statistics?.authorCount)}</dd></div>${platformStats(t.statistics).map(p => `<div><dt>${esc(platformName(p.platform))} 样本爆款</dt><dd>${p.highPerformanceThreshold == null ? '未设置规则' : `${number(p.highPerformanceCount)} / ${number(p.knownLikeCount)} · ${pct(p.highPerformanceCount,p.knownLikeCount)}`}</dd></div><div><dt>${esc(platformName(p.platform))} 点赞中位数 / P90</dt><dd>${number(p.medianLikes)} / ${number(p.p90Likes)}</dd></div>`).join('')}</dl><h4>主阶段分布</h4>${list(t.journey?.main).map(e => `<p>${esc(stageNames[e.stage] || otherNames[e.stage] || e.stage)} <span class="lgi-tm-num">${number(e.count)} · ${pct(e.count,t.journey.denominator)}</span></p>`).join('')}<h4>我方与外部参考</h4><p>已确认我方发布：${number(t.statistics?.ownHistoryPublishedCount)} · 手动爆款：${number(t.statistics?.ownHistoryBreakoutCount)}</p><p>独立外部近期低粉高赞 ${number(t.statistics?.recentLowFollowerHighLikeCount)} 篇</p>${btn('打开判断','judge',t.topicRef,'lgi-tm-quiet')}</article>`).join('')}</div>`;
  }
  async function openIdentity() {
    openDialog('identity',{loading:true}); await refreshIdentity();
  }
  async function refreshIdentity() {
    const current=dialogState;
    try { const full=await request(`/api/local/topic-map?${query({topicRef:'',windowDays:'',platform:'',path:'',overlay:''})}`); if(dialogState===current){current.snapshot=full;current.loading=false;drawDialog();} }
    catch(error){if(dialogState===current){current.error=error.message;current.loading=false;drawDialog();}}
  }
  function identityDialog() {
    const identitySource = dialogState.snapshot || snapshot;
    const rows = list(identitySource.ownCreators), authors = list(identitySource.sources).filter(s => s.authorExternalId);
    return `<div class="lgi-tm-callout">我方身份由你显式指定，仅已确认发布的作品计入覆盖。当前材料仅是已观察范围，不能据此断言整个矩阵没做过。</div><form data-form="own-creator" class="lgi-tm-form"><h3>指定我方账号</h3><label><span>已有观察账号</span><select name="creator" required><option value="">请选择账号</option>${authors.map(a => `<option value="${esc(a.platform)}|${esc(a.authorExternalId)}">${esc(platformName(a.platform))} · ${esc(a.creatorDisplayName || a.authorExternalId)}</option>`).join('')}</select></label><p class="lgi-tm-note">只有实际观察且身份已知的账号进入选择；不会自动推断账号归属。</p><button type="submit" class="lgi-tm-button lgi-tm-primary" data-mutation ${pending ? 'disabled' : ''}>加入我方矩阵</button></form><section class="lgi-tm-section"><h3>当前指定范围</h3>${rows.filter(r => r.active).map(r => `<div class="lgi-tm-list-row"><span>${esc(platformName(r.platform))} · ${esc(authors.find(a => a.platform === r.platform && a.authorExternalId === r.authorExternalId)?.creatorDisplayName || r.authorExternalId)}</span>${btn('移出我方矩阵','own-remove',`${r.platform}|${r.authorExternalId}`,'', 'data-mutation')}</div>`).join('') || empty('尚未指定我方矩阵')}<p class="lgi-tm-note">对标来源沿用采集目标的领域用途，不在这里自动建立监控或深度作者档案。</p></section>`;
  }
  function rulesDialog() {
    return `<p>样本爆款规则按平台独立保存；未知点赞不进入可判定分母。外部低粉高赞与我方手动标签不随本规则改变。</p>${['xhs','douyin'].map(p => { const stat = platformStats(snapshot.statistics).find(s => s.platform === p); return `<form data-form="rule" data-platform="${p}" class="lgi-tm-form"><h3>${platformName(p)}</h3><p>当前规则：${stat?.highPerformanceThreshold == null ? '未设置' : `点赞 ≥ ${number(stat.highPerformanceThreshold)} · 版本 ${esc(stat.highPerformanceRuleVersion)}`}</p><label><span>点赞阈值</span><input name="threshold" type="number" min="1" step="1" required value="${stat?.highPerformanceThreshold ?? ''}" placeholder="输入明确的正整数阈值"></label><button type="submit" class="lgi-tm-button" data-mutation>保存本平台规则</button></form>`; }).join('')}`;
  }
  async function openSaved() {
    openDialog('saved',{loading:true}); const current = dialogState;
    try { current.snapshot = await request(`/api/local/topic-map?${query({topicRef:'',windowDays:'',platform:'',overlay:'',path:''})}`); }
    catch (error) { current.error = error.message; }
    if (dialogState === current) { current.loading = false; drawDialog(); }
  }
  function savedDialog() {
    if (dialogState.loading) return '<p role="status">正在取回后端已保存的备选与来源资格。</p>';
    const rows = list(dialogState.snapshot?.alternatives).filter(a => !dialogState.query || [a.title,a.angle,a.rationale].join(' ').includes(dialogState.query));
    return `<p>保存的角度与产品研究说明保留原定义和引用版本，默认不跟踪、不启动研究或采集。</p><form data-form="saved-search" class="lgi-tm-row"><input name="query" value="${esc(dialogState.query || '')}" placeholder="查找已保存标题 / 角度 / 研究说明" aria-label="查找备选"><button type="submit" class="lgi-tm-button">查找</button></form>${rows.length ? rows.map(a => `<article class="lgi-tm-saved"><div class="lgi-tm-row">${badge(a.kind === 'product_research' ? '产品研究说明' : '内容角度')}${badge(a.sourceState === 'available' ? '来源当前可用' : '读取原版本时复核来源资格')}${badge('不持续跟踪')}</div><h3>${esc(a.title || '保存记录的来源资格已变化')}</h3><p>${esc(a.angle || '原保存文本需经来源资格复核后读取。')}</p><p>${esc(a.rationale)}</p><p class="lgi-tm-note">${esc(a.createdAt)} · ${esc(a.methodVersion)} · ${list(a.evidenceWorkRefs).length} 篇冻结作品引用</p>${btn('取回保存的原版本','saved-materials',a.alternativeRef)}${btn('导出引用与说明','export-alternative',a.alternativeRef,'lgi-tm-quiet')}</article>`).join('') : empty('当前没有匹配的已保存备选','选择角度或编辑产品研究说明并保存后，可在这里取回原版本。')}`;
  }
  function saveDialog() {
    const a=dialogState.angle, product=dialogState.kind==='product_research';
    if(dialogState.savedAlternativeRef)return `<div class="lgi-tm-callout">${product?'产品研究说明':'内容角度'}已获得后端保存回执。保存不启动监控、采集或业务实验。</div><div class="lgi-tm-row">${btn('取回刚保存的原版本','saved-materials',dialogState.savedAlternativeRef)}${btn('复核来源并导出','export-alternative',dialogState.savedAlternativeRef)}</div>`;
    return `<h3>${product?'保留需求、支持假设与最先验证的问题':'这个方向，先收下来'}</h3><p>${product?'修改是人工研究说明，不改变模型原研究结果。':'内容备选不是已发布作品，不计入我方发布覆盖。'}保存保留当前主题定义、研究版本与引用，不自动持续跟踪。</p><form data-form="save" class="lgi-tm-form"><label><span>${product?'研究说明标题':'备选标题 · 可继续打磨'}</span><input name="title" value="${esc(a.title)}" maxlength="120" required></label><label><span>${product?'具体需求与支持假设':'切入角度'}</span><textarea name="angle" maxlength="2000" required>${esc(a.label)}</textarea></label><label><span>${product?'可编辑研究说明 / 最先验证什么':'这篇要回答的问题'}</span><textarea name="rationale" maxlength="2000" required>${esc(a.answerTask)}</textarea></label>${product?'':`<fieldset class="lgi-tm-reuse-options"><legend>复用意图 · 可组合</legend>${['沿用标题','保留核心观点','调整封面','切换场景','重新组织内容'].map(value=>`<label class="lgi-tm-check-label"><input type="checkbox" name="reuse" value="${value}"><span>${value}</span></label>`).join('')}</fieldset><label><span>回答边界 · 没有依据时保留未知</span><textarea name="boundary" maxlength="800" placeholder="明确这篇不回答什么、适用条件以及还缺少哪些材料">${esc(list(a.limitations).join('；'))}</textarea></label><label><span>备注（可不填）</span><textarea name="note" maxlength="400" placeholder="以后打开时，帮助自己想起为什么保留。"></textarea></label>`}<div class="lgi-tm-callout"><strong>原始依据与人工说明一起保存</strong><p>${esc(title(work(a.workRef) || {}))} · ${list(a.evidence).length} 条定位引用</p><p>复用意图、回答边界与备注以明确字段标题保存在现有角度和说明中；不改变原始作品或原模型结论。</p></div><button type="submit" class="lgi-tm-button lgi-tm-primary" data-mutation>${product?'保存研究说明与引用':'保存到我的备选'}</button></form>`;
  }
  function opportunitiesFrom(outputs) {
    return outputs.flatMap(({work,output}) => list(output.productOpportunities).map((o,opportunityIndex) => ({...o,workRef:work.workRef,researchResultRef:o.researchResultRef || work.research?.resultRef,researchMethodVersion:work.research?.methodVersion,researchOpportunityIndex:Number.isInteger(o.researchOpportunityIndex)?o.researchOpportunityIndex:opportunityIndex}))).filter(o=>typeof o.researchResultRef==='string'&&o.researchResultRef.length>0);
  }
  function productContent(outputs) {
    const opportunities = opportunitiesFrom(outputs);
    return `<p>首版只形成需求、支持假设与先验证的问题，可以编辑并保存研究说明及引用；不启动业务实验。</p>${opportunities.length ? opportunities.map((o,index) => `<article class="lgi-tm-panel"><h4>具体需求</h4><p>${esc(o.need)}</p><h4>可能的支持方式</h4><p>${esc(o.hypothesis)}</p><h4>最先验证什么</h4><p>${esc(o.verificationQuestion)}</p><h4>替代解释与限制</h4><p>${esc(o.alternativeExplanation)}</p><div class="lgi-tm-row">${btn('核对依据','reader',o.workRef,'lgi-tm-quiet')}${btn('编辑并保存研究说明','save-opportunity',String(index))}</div></article>`).join('') : empty('当前证据不足以提出可引用的产品机会','先核对具体需求与现有回答，不补造付费意愿、市场规模或产品结论。')}`;
  }
  function productDialog() { return productContent(analysisOutputs(dialogState.works || selectedWorks())); }
  const alternativeAvailable = value => value && ['available','version_changed_available'].includes(value.sourceState) && typeof value.title === 'string' && typeof value.angle === 'string' && typeof value.rationale === 'string';
  async function readAlternative(ref) {
    return request(`/api/local/topic-map/alternative?domainRef=${encodeURIComponent(state.domainRef)}&alternativeRef=${encodeURIComponent(ref)}`);
  }
  async function openAlternative(ref) {
    openDialog('alternative',{alternativeRef:ref,loading:true});const current=dialogState;
    try {const value=await readAlternative(ref);if(dialogState===current){current.alternative=value;current.loading=false;drawDialog();}}
    catch(error){if(dialogState===current){current.error=error.message;current.loading=false;drawDialog();}}
  }
  function alternativeDialog() {
    if(dialogState.loading)return '<p role="status">正在复核当前来源资格，并读取保存时的定义与引用版本…</p>';
    const a=dialogState.alternative;
    if(!alternativeAvailable(a))return empty('保存记录的来源当前不可用','原文本与引用片段不继续展示。历史引用记录保留，不能用当前作品替代原依据。') + `<p class="lgi-tm-note">记录 ${esc(a?.alternativeRef || dialogState.alternativeRef)} · 方法 ${esc(a?.methodVersion || '未知')}</p>`;
    const d=a.originalDefinition, product=a.kind==='product_research';
    return `${a.sourceState==='version_changed_available' ? '<div class="lgi-tm-callout">来源或研究版本后来有变化。下方展示保存时的原定义、说明和已接纳原版本片段；当前来源限制已重新复核。原版本不代表当前判断。</div>' : '<div class="lgi-tm-callout">下方为保存时的版本，当前来源资格已复核。查看与导出不启动研究、监控或采集。</div>'}<div class="lgi-tm-object-head"><div><div class="lgi-tm-row">${badge(product ? '产品研究说明' : '内容角度')}${badge('保存的原版本')}</div><h3>${esc(a.title)}</h3><p>${esc(a.methodVersion)}</p></div>${btn('复核来源并导出Markdown','export-alternative',a.alternativeRef)}</div><section class="lgi-tm-section"><h4>${product ? '具体需求与支持假设' : '保存的角度'}</h4><div class="lgi-tm-original">${esc(a.angle)}</div><h4>${product ? '人工编辑的研究说明' : '回答任务与保留理由'}</h4><div class="lgi-tm-original">${esc(a.rationale)}</div></section><section class="lgi-tm-section"><h3>保存时的主题定义</h3>${d ? `<h4>${esc(d.displayName)} · 定义 v${number(d.version)}</h4><p>${esc(d.definitionText)}</p><small class="lgi-tm-num">${esc(d.definitionRef)}</small>` : empty('原定义尚未返回','不以当前主题定义代替原版本。')}</section><section class="lgi-tm-section"><h3>保存时的正文与原声依据</h3><p>原文和机器读图 / 转写分别保留来源；高亮来自保存时的精确引用范围。</p>${list(a.fragments).map(f => `<article class="lgi-tm-frozen-fragment"><div class="lgi-tm-row">${badge(sourceKind(f.field))}<span class="lgi-tm-num">${esc(f.workRef)}</span></div><blockquote>${highlighted(f.text,list(f.citedRanges))}</blockquote><p class="lgi-tm-note">来源 ${esc(f.sourceRef)} · ${esc(f.sourceVersion || '来源版本未记录')} · 字符 ${number(f.start)}–${number(f.end)}</p></article>`).join('') || empty('本记录没有可展示的冻结来源片段','保留记录不补造来源内容。')}</section><p class="lgi-tm-note">记录 ${esc(a.alternativeRef)} · 原定义 ${esc(a.definitionRef)}</p>`;
  }
  function markdownFence(text) {
    const longest=Math.max(2,...(String(text).match(/`+/g)||[]).map(value=>value.length));
    const fence='`'.repeat(longest+1);return `${fence}\n${text}\n${fence}`;
  }
  function alternativeMarkdown(a) {
    const d=a.originalDefinition, product=a.kind==='product_research';
    const lines=[`# ${a.title}`,'',`类型：${product?'产品研究说明':'内容角度'}`,`保存记录：${a.alternativeRef}`,`领域：${a.domainRef}`,`主题：${a.topicRef}`,`原定义：${a.definitionRef}`,`研究方法：${a.methodVersion}`,`来源复核：${a.sourceState === 'version_changed_available' ? '原版本当前仍可读，后来版本已有变化' : '原版本当前可读'}`,'','保存不开启监控、采集或业务实验。此说明及依据保留当时版本，不代表当前事实、市场趋势或产品价值已得到验证。','',`## ${product?'具体需求与支持假设':'保存的角度'}`,'',a.angle,'',`## ${product?'人工编辑的研究说明':'回答任务与保留理由'}`,'',a.rationale,'','## 保存时的主题定义',''];
    if(d)lines.push(`${d.displayName} · v${d.version}`,d.definitionText,`定义引用：${d.definitionRef}`);else lines.push('原定义未取得，未以当前定义替代。');
    lines.push('','## 保存时的原文引用','');
    list(a.fragments).forEach((f,index)=>{lines.push(`### 引用 ${index+1} · ${sourceKind(f.field)}`,'',`作品：${f.workRef}`,`来源：${f.sourceRef}`,`来源版本：${f.sourceVersion || '未知'}`,`片段身份：${f.fragmentId}`,`字符范围：${f.start}–${f.end}`,`高亮引用范围：${list(f.citedRanges).map(r=>`${r.start}–${r.end}`).join('、') || '无额外高亮范围'}`,'',markdownFence(f.text),'');});
    return lines.join('\n');
  }
  async function exportAlternative(ref) {
    if(pending)return;pending=true;dialogFeedback('正在重新复核保存记录与来源资格，复核通过后才生成本地导出…');
    try {
      const a=await readAlternative(ref);
      if(dialogState?.type==='alternative'&&dialogState.alternativeRef===ref){dialogState.alternative=a;drawDialog();}
      if(!alternativeAvailable(a)){dialogFeedback('来源当前不可用，已停止导出；不输出旧文本或以当前材料补位。');return;}
      const blob=new Blob([alternativeMarkdown(a)],{type:'text/markdown;charset=utf-8'}),url=URL.createObjectURL(blob),anchor=document.createElement('a');
      anchor.href=url;anchor.download=`Linggan_${Array.from(a.title).slice(0,60).join('').replace(/[\/:*?"<>|\u0000-\u001f]/g,'_')}.md`;anchor.hidden=true;root.appendChild(anchor);anchor.click();anchor.remove();setTimeout(()=>URL.revokeObjectURL(url),1000);
      dialogFeedback('来源资格复核通过，已生成本地Markdown导出；没有平台发布或外发模型。');
    }catch(error){if(dialogState?.type==='alternative'){delete dialogState.alternative;dialogState.error=error.message;drawDialog();}dialogFeedback(`导出未完成：${error.message}`);}finally{pending=false;}
  }
  function changesDialog() {
    const changes = list(snapshot.changes).filter(c => c.structureChanged || c.notViewed || list(c.unviewedWorkRefs).length);
    return `<p>当前按主题定义版本与查看回执比较；研究依据变化和历史补录保留各自来源，不混称市场变化。</p>${changes.length ? changes.map(c => `<article class="lgi-tm-list-row"><div><strong>${esc(topicName(c.topicRef))}</strong><p>${c.notViewed ? '首次查看此主题定义' : '定义版本与上次查看不同'} · 定义 v${esc(c.definitionVersion)}</p><p>上次查看：${esc(c.lastViewedAt || '尚无记录')} · ${list(c.unviewedWorkRefs).length} 篇尚未记录为已读</p><p class="lgi-tm-note">材料补录或分类差异不等于市场变化。</p></div>${btn('查看主题','topic',c.topicRef)}${btn('记为已查看本定义','viewed',c.topicRef,'lgi-tm-quiet')}</article>`).join('') : empty('当前没有尚未查看的定义变化','已有主题与历史材料继续可用。')}`;
  }
  async function openResearch() {
    if (!state.domainRef) { notice = '先选择实际领域，再读取研究设置。'; renderNotice(); return; }
    openDialog('research',{loading:true}); const current = dialogState;
    try { progress = await request(`/api/local/topic-map/research?domainRef=${encodeURIComponent(state.domainRef)}`); current.progress = progress; }
    catch (error) { current.error = error.message; }
    if (dialogState === current) { current.loading = false; drawDialog(); }
  }
  function researchPhases(run) {
    if(!run.phases)return '';
    const p=run.phases;
    return `<div class="lgi-tm-research-phases"><p><strong>提炼讨论</strong> · 进行中 ${number(p.extracting)} · 待处理 ${number(p.extractQueued)}</p><p><strong>判断归属</strong> · 进行中 ${number(p.resolving)} · 待处理 ${number(p.resolveQueued)}</p></div>`;
  }
  function researchDialog() {
    if (dialogState.loading) return '<p role="status">正在读取研究设置与真实进度回执。</p>';
    const p = dialogState.progress || progress, policy = p?.policy, models = list(p?.models), usage = p?.usage;
    return `<div class="lgi-tm-callout">开启后，历史材料分批研究、新材料增量处理。只有明确保存开启配置才启动，不因普通开页调用模型。关闭弹窗不取消已发任务，明确停止阻止继续启动新请求。</div><div class="lgi-tm-metrics"><div><span>今日已记账</span><strong>${number(usage?.chargedTokens)}</strong><small>token · ${esc(usage?.timezone || 'Asia/Shanghai')}</small></div><div><span>当前预留</span><strong>${number(usage?.reservedTokens)}</strong><small>token · 不等于已消费</small></div><div><span>每日上限</span><strong>${number(policy?.dailyTokenLimit)}</strong><small>达到上限后延后待处理</small></div><div><span>每次研究上限</span><strong>${number(policy?.runTokenLimit)}</strong><small>token · 与精选10篇不同</small></div></div><form data-form="research-config" class="lgi-tm-form"><h3>研究设置</h3><label><span>已有模型配置</span><select name="modelConfigRef" required><option value="">请选择配置</option>${models.map(m => `<option value="${esc(m.configRef)}" ${policy?.modelConfigRef === m.configRef ? 'selected' : ''}>${esc(m.modelId)} · 输入 ${number(m.inputTokenLimit)} / 输出 ${number(m.outputTokenLimit)} token</option>`).join('')}</select></label><div class="lgi-tm-form-grid"><label><span>每日 token 上限</span><input type="number" name="dailyTokenLimit" min="1024" max="10000000" step="1" required value="${policy?.dailyTokenLimit ?? ''}" placeholder="明确预算后填写"></label><label><span>每次研究 token 上限</span><input type="number" name="runTokenLimit" min="1024" max="10000000" step="1" required value="${policy?.runTokenLimit ?? ''}" placeholder="明确预算后填写"></label></div><label class="lgi-tm-check-label"><input type="checkbox" name="automaticEnabled" ${policy?.automaticEnabled ? 'checked' : ''}><span>明确开启历史回填与新材料自动增量研究</span></label><label class="lgi-tm-check-label"><input type="checkbox" name="collectionEnabled" ${policy?.collectionEnabled ? 'checked' : ''}><span>在已授权范围内允许主题临时补采，每轮最多10篇详情</span></label><p>已有材料仍可用；研究配置不替代平台执行授权、工位范围与采集限制。</p><button type="submit" class="lgi-tm-button lgi-tm-primary" data-mutation>保存明确设置</button></form><section class="lgi-tm-section"><div class="lgi-tm-section-head"><h3>研究进度</h3>${btn('刷新进度','research-refresh')}</div>${list(p?.runs).map(r => `<article class="lgi-tm-run"><div><h4>${esc(labelState(r.state))} ${badge(({historical:'历史回填',incremental:'新材料增量',on_demand:'按需研究'})[r.trigger] || r.trigger)}</h4><p>待处理 ${number(r.queuedCount)} · 已获结果 ${number(r.succeededCount)} · 失败 / 派发未知 ${number(r.failedCount)}</p>${researchPhases(r)}<p>${esc(r.lastReason || '暂无额外原因')} · ${esc(r.createdAt)}</p><small class="lgi-tm-num">${esc(r.runRef)}</small></div><div class="lgi-tm-row">${btn('暂停','research-pause',r.runRef,'', 'data-mutation')}${btn('恢复','research-resume',r.runRef,'', 'data-mutation')}${btn('明确停止','research-stop',r.runRef,'lgi-tm-danger','data-mutation')}</div></article>`).join('') || empty('当前没有研究运行','首次开启或明确按需启动后，真实回执会出现在这里。')}</section>`;
  }
  async function researchCommand(payload) {
    if (pending) return null;
    pending = true; dialogFeedback('正在提交明确动作…');
    try { const result = await request('/api/local/topic-map/research/commands',{method:'POST',body:JSON.stringify({...payload,requestRef:uuid(),domainRef:state.domainRef})}); dialogFeedback('动作已获得后端回执；排队、外发与结果接纳分别以进度为准。'); return result; }
    catch (error) { dialogFeedback(`动作未完成：${error.message}`); return null; }
    finally { pending = false; }
  }
  function allCitations(w) {
    const o=w.research?.output || {};
    return [...discussionUnits(w).flatMap(d=>d.evidence),...list(o.journey?.evidence),...['scenes','responseMatches','angles','productOpportunities'].flatMap(key=>list(o[key]).flatMap(v=>list(v.evidence)))];
  }
  function citationSpans(w,fragmentId) {return locatedCitations(w,allCitations(w)).filter(c=>c.fragmentId===fragmentId).map(c=>({start:c.localStart,end:c.localEnd}));}
  async function recordViewedWork(w) {
    const ref=dialogStack.slice().reverse().find(v=>v.type==='judge')?.topicRef || state.topicRef,t=topic(ref);
    if(!t || (!list(w.topicRefs).includes(ref)&&!list(t.workRefs).includes(w.workRef)))return;
    const current=list(snapshot.changes).find(c=>c.topicRef===ref),refs=[...new Set([...list(current?.lastViewedWorkRefs),w.workRef])].slice(-1000);
    try {await request('/api/local/topic-map/commands',{method:'POST',body:JSON.stringify({action:'viewed',idempotencyKey:uuid(),domainRef:state.domainRef,topicRef:ref,definitionRef:t.definitionRef,workRefs:refs})});if(current){current.lastViewedWorkRefs=refs;current.unviewedWorkRefs=list(current.unviewedWorkRefs).filter(id=>id!==w.workRef);}} catch (_) { /* Read availability remains independent from the optional view receipt. */ }
  }
  function bodyCitations(w,raw) {
    return locatedCitations(w,allCitations(w)).filter(c=>readerFragmentInBody(w,c.fragment,raw)).map(c=>({start:c.start,end:c.end}));
  }
  function moveDialog() {
    const ref=dialogState.moveTopicRef || state.topicRef, t=topic(ref), choices=snapshot.topics.filter(t=>t.lifecycleState!=='superseded');
    const descendants=new Set([ref]);let changed=true;while(changed){changed=false;choices.forEach(c=>{if(descendants.has(c.parentTopicRef)&&!descendants.has(c.topicRef)){descendants.add(c.topicRef);changed=true;}});}
    return `<p>只调整本领域导航归属，不改写主题定义、来源材料与已有判断。</p><form data-form="move-topic" class="lgi-tm-form"><label><span>要调整的主题</span><select name="topicRef" data-dialog-filter="moveTopicRef" required><option value="">选择主题</option>${choices.map(c=>`<option value="${esc(c.topicRef)}" ${c.topicRef===ref?'selected':''}>${esc(c.displayName)}</option>`).join('')}</select></label><label><span>新的父主题</span><select name="parentTopicRef"><option value="">作为一级主题</option>${choices.filter(c=>!descendants.has(c.topicRef)).map(c=>`<option value="${esc(c.topicRef)}" ${t?.parentTopicRef===c.topicRef?'selected':''}>${esc(c.displayName)}</option>`).join('')}</select></label><p>当前父主题：${t?.parentTopicRef?esc(topicName(t.parentTopicRef)):'一级主题'} · 归属版本 ${t?.bindingVersion==null?'尚未绑定':number(t.bindingVersion)}</p><button type="submit" class="lgi-tm-button lgi-tm-primary" data-mutation ${t?'':'disabled'}>保存本次归属调整</button></form>`;
  }
  function structureDialog() {
    const kind = dialogState.kind || 'merge', count = kind === 'merge' ? 1 : dialogState.destinationCount || 2;
    const selected = list(dialogState.sources), eligible = list(snapshot.topics).filter(t=>t.lifecycleState!=='superseded').filter(t => !snapshot.topics.some(c => c.parentTopicRef === t.topicRef));
    const refs = [...new Set(selected.flatMap(ref => list(topic(ref)?.workRefs)))], ns = snapshot.works.filter(w => refs.includes(w.workRef));
    if (dialogState.preview) {
      const {plan,preview} = dialogState;
      return `<div class="lgi-tm-callout">确认后，旧主题保留历史引用，新主题仅采用你显式分配的作品；旧角度、判断与统计不会自动继承。新定义仍是候选。</div><h3>${plan.kind === 'merge' ? '合并' : '拆分'}影响预览</h3>${plan.sources.map(s => `<p>旧主题：${esc(topicName(s.topicRef))} · 定义 ${esc(s.definitionRef)}</p>`).join('')}${plan.destinations.map(d => `<article class="lgi-tm-panel"><h4>${esc(d.displayName)}</h4><p>${esc(d.definitionText)}</p><p>${d.members.length} 篇显式材料分配</p></article>`).join('')}<p>${list(preview.impact?.unassignedWorkRefs).length} 篇未重新分配的材料保留旧历史归属。</p><p>${esc(plan.rationale)}</p><div class="lgi-tm-row">${btn('返回修改','structure-edit')}${btn('确认应用此结构调整','structure-apply','','lgi-tm-primary','data-mutation')}</div>`;
    }
    return `<p>先选择旧叶主题，显式分配每篇作品，再看后端影响预览。拆分与合并不会自动继承旧判断或研究。</p><div class="lgi-tm-toolbar"><label class="lgi-tm-control"><span>调整方式</span><select data-dialog-filter="kind"><option value="merge" ${kind === 'merge' ? 'selected' : ''}>合并多个主题</option><option value="split" ${kind === 'split' ? 'selected' : ''}>拆分一个主题</option></select></label>${kind === 'split' ? btn('再增加一个新方向','structure-add-destination') : ''}</div><form data-form="structure" class="lgi-tm-form"><fieldset><legend>${kind === 'merge' ? '选择至少两个旧叶主题' : '选择一个旧叶主题'}</legend>${eligible.map(t => `<label class="lgi-tm-check-label"><input type="checkbox" data-structure-source="${esc(t.topicRef)}" name="sources" value="${esc(t.topicRef)}" ${selected.includes(t.topicRef) ? 'checked' : ''}><span>${esc(t.displayName)} · ${number(t.statistics?.workCount)} 篇</span></label>`).join('')}</fieldset>${Array.from({length:count},(_,i) => `<fieldset><legend>新方向 ${i+1}</legend><label><span>名称</span><input name="title-${i}" maxlength="120" required></label><label><span>定义与边界</span><textarea name="definition-${i}" maxlength="2000" required></textarea></label><label><span>父主题</span><select name="parent-${i}"><option value="">作为一级主题</option>${snapshot.topics.filter(t => !selected.includes(t.topicRef)&&t.lifecycleState!=='superseded').map(t => `<option value="${esc(t.topicRef)}">${esc(t.displayName)}</option>`).join('')}</select></label></fieldset>`).join('')}<h3>逐篇显式分配</h3><p>不分配的作品保留旧历史，不会自动带入新方向。每个新方向至少选择一篇实际作品。</p>${ns.map(w => `<fieldset><legend>${esc(title(w))}</legend><label><span>进入哪个新方向</span><select name="member-${w.workRef}"><option value="">本次不重新分配</option>${Array.from({length:count},(_,i) => `<option value="${i}">新方向 ${i+1}</option>`).join('')}</select></label><label><span>材料作用</span><select name="role-${w.workRef}"><option value="support">支持</option><option value="challenge">反例</option><option value="boundary">边界</option></select></label><label><span>此分配的具体理由</span><input name="reason-${w.workRef}" maxlength="1000" placeholder="分配后须说明具体依据"></label></fieldset>`).join('')}<label><span>整体结构调整理由</span><textarea name="rationale" maxlength="2000" required></textarea></label><button type="submit" class="lgi-tm-button lgi-tm-primary" data-mutation>预览实际影响</button></form>`;
  }
  async function previewStructure(form,data) {
    const sources = data.getAll('sources').map(ref => { const t=topic(ref); return {topicRef:t.topicRef,definitionRef:t.definitionRef}; });
    const kind = dialogState.kind || 'merge', count = kind === 'merge' ? 1 : dialogState.destinationCount || 2;
    if ((kind === 'merge' && sources.length < 2) || (kind === 'split' && sources.length !== 1)) return dialogFeedback('合并至少选择两个旧主题，拆分只选择一个旧主题。');
    const destinations = Array.from({length:count},(_,i) => ({displayName:String(data.get(`title-${i}`)).trim(),definitionText:String(data.get(`definition-${i}`)).trim(),parentTopicRef:String(data.get(`parent-${i}`)) || null,members:[]}));
    for (const w of snapshot.works) {
      const index = data.get(`member-${w.workRef}`);
      if (index === null || index === '') continue;
      const rationale = String(data.get(`reason-${w.workRef}`) || '').trim();
      if (!rationale) return dialogFeedback(`请说明“${title(w)}”的分配理由。`);
      destinations[Number(index)].members.push({workPublicRef:w.workRef,role:String(data.get(`role-${w.workRef}`)),rationale});
    }
    if (destinations.some(d => !d.members.length)) return dialogFeedback('每个新方向需要至少一篇你显式分配的实际材料。');
    const plan = {requestRef:uuid(),domainRef:state.domainRef,kind,sources,destinations,rationale:String(data.get('rationale')).trim()};
    try { const preview=await request('/api/local/topic-map/structure/preview',{method:'POST',body:JSON.stringify(plan)}); dialogState.plan=plan; dialogState.preview=preview; drawDialog(); }
    catch (error) { dialogFeedback(`预览未完成：${error.message}`); }
  }
  function captureDialog() {
    if (dialogState.loading) return '<p role="status">正在读取已有执行范围与补采轮次…</p>';
    const p = dialogState.collection, search = dialogState.search, deep = Boolean(dialogState.workRef);
    return `<div class="lgi-tm-callout">详情每轮最多读取10篇新作品；临时搜索最多3个显式词、200篇候选，不建立长期监控。深入评论仅由显式点击追加，新增评论与回复合计最多30条。关闭弹窗不取消本轮。</div><form data-form="capture" class="lgi-tm-form">${deep ? `<label><span>现有受控采集目标</span><select name="targetRef" required><option value="">选择已有执行目标</option>${list(p?.targets).map(t => `<option value="${esc(t.targetRef)}">${esc(t.displayName)} · ${t.kind === 'keyword' ? '关键词目标' : '创作者目标'}</option>`).join('')}</select></label><p>深入讨论：${esc(title(work(dialogState.workRef) || {}))} · 最多新增30条评论与回复</p>` : `<label><span>现有关键词执行授权</span><select name="authorizationRef" required><option value="">选择已有授权范围</option>${list(search?.authorizations).map(a => `<option value="${esc(a.authorizationRef)}">${esc(a.purpose)} · 每目标最多 ${number(a.maxWorksPerTarget)} 篇 · 到期 ${esc(a.expiresAt)}</option>`).join('')}</select></label><label><span>本主题临时搜索词</span><input name="keywords" maxlength="300" required placeholder="最多3个明确词，用逗号分隔"></label>`}<label><span>本轮补充目的</span><input name="purpose" maxlength="120" required placeholder="说明当前具体缺项"></label><label class="lgi-tm-check-label"><input type="checkbox" name="newRound"><span>明确再开启新一轮；默认取回同范围已有轮次</span></label><button type="submit" class="lgi-tm-button lgi-tm-primary" data-mutation>${deep ? '明确追加讨论样本' : '明确开启一轮临时搜索'}</button></form><div class="lgi-tm-row">${btn('刷新真实进度','capture-refresh')}</div><section class="lgi-tm-section"><h3>临时搜索与详情补充</h3>${list(search?.rounds).map(r => `<article class="lgi-tm-panel lgi-tm-section"><div class="lgi-tm-section-head"><div><h4>${esc(labelState(r.state))} · ${r.phase === 'search' ? '搜索阶段' : '详情阶段'}</h4><p>${list(r.searches).map(s => esc(s.keyword)).join('、')} · 已发现候选 ${number(r.candidateCount)} / ${number(r.candidateLimit)}</p><p>详情预留 ${number(r.detailSlotsReserved)} / 10 · 已取得合格详情 ${number(r.actualDetailsAcquired)} · 待取得 ${list(r.pendingDetailWorkRefs).length}</p></div>${btn('明确停止本轮','search-stop',r.roundRef,'lgi-tm-danger','data-mutation')}</div>${r.phase === 'search' && list(r.candidateWorkRefs).length && ['active','partial'].includes(r.state) ? `<form data-form="freeze-details" data-round="${esc(r.roundRef)}"><h4>从本轮真实候选中选择最多10篇详情</h4><p>选择后停止继续搜索并冻结本轮详情身份；已有合格详情复用，不额外计新详情。</p>${list(r.candidateWorkRefs).map(ref => {const w = work(ref); return `<label class="lgi-tm-check-label"><input type="checkbox" name="workRefs" value="${esc(ref)}" data-detail-choice="${esc(r.roundRef)}"><span>${esc(w ? title(w) : '作品标题尚未读取')} · ${esc(ref)}</span>${w ? btn('先看已有材料','reader',ref,'lgi-tm-quiet') : ''}</label>`;}).join('')}<button type="submit" class="lgi-tm-button" data-mutation>冻结所选详情集合</button></form>` : ''}<p class="lgi-tm-note">搜索为空不证明现实没有需求，平台执行与分析结果更新分别以真实回执为准。</p></article>`).join('') || empty('当前没有临时搜索轮次','已有材料继续可读，不会为填满页面自动补采。')}</section><section class="lgi-tm-section"><h3>已有详情 / 评论补采轮次</h3>${list(p?.collectionRounds).filter(r=>r.kind!=='search').map(r => `<article class="lgi-tm-run"><div><h4>${esc(labelState(r.state))} · ${r.kind === 'comments' ? '评论追加' : '详情补充'}</h4><p>预留详情身份 ${number(r.detailSlotsReserved)} / 10</p><p>${esc(r.reason || '')} · ${esc(r.roundRef)}</p></div>${btn('明确停止本轮','collection-stop',r.roundRef,'lgi-tm-danger','data-mutation')}</article>`).join('') || empty('当前没有已记录的详情或评论补采','操作回执、真实平台取得与研究结果更新分别表达。')}</section>`;
  }
  async function submitDialog(event) {
    event.preventDefault(); const form = event.target, data = new FormData(form), name = form.dataset.form;
    if(name==='replace-samples'){const refs=[...new Set(data.getAll('workRefs'))],parent=dialogState.judgeState;if(refs.length>10)return dialogFeedback('本次最多10篇独立作品。');const allowed=new Map(replacementCandidates(parent).map(w=>[w.workRef,w]));if(refs.some(ref=>!allowed.has(ref)))return dialogFeedback('所选作品不属于当前主题的合格判断范围，请重新核对。');parent.works=refs.map(ref=>allowed.get(ref));delete parent.inlineReader;delete parent.sampleRef;parent.tab='samples';dialogBack();if(parent.works.length)await readSample(parent.works[0].workRef);return;}
    if (name === 'material-search' || name === 'saved-search') { dialogState.query = String(data.get('query') || '').trim(); drawDialog(); return; }
    if (name === 'own-creator') {
      const [platform,authorExternalId] = String(data.get('creator')).split('|');
      const result = await command({action:'ownCreator',platform,authorExternalId,active:true},'我方账号范围已保存');
      if (result) { await refreshIdentity(); dialogFeedback(`已保存 · 回执 ${result.receiptRef}`); } else dialogFeedback(notice); return;
    }
    if (name === 'rule') {
      const platform = form.dataset.platform, current = platformStats(snapshot.statistics).find(p => p.platform === platform);
      const result = await command({action:'performanceRule',platform,likeThreshold:Number(data.get('threshold')),expectedVersion:current?.highPerformanceRuleVersion ?? null},'平台样本规则已保存');
      if (result) { drawDialog(); dialogFeedback(`规则已保存 · 版本 ${result.revision}`); } else dialogFeedback(notice); return;
    }
    if (name === 'save') {
      const current=dialogState,a = current.angle, t = topic(current.topicRef);
      if (!t) return dialogFeedback('原主题定义尚未读取，当前不能保存。');
      const product=dialogState.kind==='product_research';
      const savedAngle=product?String(data.get('angle')).trim():`切入角度：${String(data.get('angle')).trim()}\n\n复用意图：${data.getAll('reuse').join('、') || '未指定'}`;
      const savedRationale=product?String(data.get('rationale')).trim():`这篇要回答的问题：\n${String(data.get('rationale')).trim()}\n\n回答边界：\n${String(data.get('boundary') || '').trim() || '尚未明确，需要核对原作与适用条件'}\n\n备注：\n${String(data.get('note') || '').trim() || '未填写'}`;
      if(Array.from(savedAngle).length>2000||Array.from(savedRationale).length>2000)return dialogFeedback('完整角度或研究说明超过保存上限，请缩短问题、边界或备注后再保存。');
      const result = await command({action:'saveAlternative',topicRef:t.topicRef,definitionRef:t.definitionRef,title:String(data.get('title')).trim(),angle:savedAngle,rationale:savedRationale,evidenceWorkRefs:[a.workRef],methodVersion:a.researchMethodVersion || snapshot.methodVersion,researchResultRef:a.researchResultRef || null,researchAngleIndex:a.researchResultRef && dialogState.kind!=='product_research' ? a.researchAngleIndex : null,researchOpportunityIndex:a.researchResultRef && dialogState.kind==='product_research' ? a.researchOpportunityIndex : null},'备选已保存到后端，不启用跟踪');
      if (result) { current.savedAlternativeRef=result.subjectRef;if(dialogState===current){drawDialog();dialogFeedback(`已保存 · 回执 ${result.receiptRef}`);} } else dialogFeedback(notice); return;
    }
    if (name === 'research-config') {
      const result = await researchCommand({action:'configure',modelConfigRef:String(data.get('modelConfigRef')),dailyTokenLimit:Number(data.get('dailyTokenLimit')),runTokenLimit:Number(data.get('runTokenLimit')),automaticEnabled:data.has('automaticEnabled'),collectionEnabled:data.has('collectionEnabled')});
      if (result) await refreshResearch(); return;
    }
    if (name === 'freeze-details') { const refs=data.getAll('workRefs');if(!refs.length||refs.length>10)return dialogFeedback('请从本轮候选显式选择1到10篇独立作品。');const result=await collectionCommand({action:'freeze_search_details',roundRef:form.dataset.round,workRefs:refs},true);if(result)await refreshCapture();return;}
    if (name === 'capture') { await submitCapture(form,data); return; }
    if(name==='move-topic'){const t=topic(String(data.get('topicRef')));if(!t)return dialogFeedback('先选中本域主题。');const result=await command({action:'bindTopic',topicRef:t.topicRef,parentTopicRef:String(data.get('parentTopicRef'))||null,expectedVersion:t.bindingVersion??null},'主题导航归属已保存');if(result){drawDialog();dialogFeedback(`归属已保存 · 版本 ${result.revision}`);}else dialogFeedback(notice);return;}
    if (name === 'structure') { await previewStructure(form,data); return; }
  }
  async function refreshResearch() {
    if (!dialogState) return;
    try { progress = await request(`/api/local/topic-map/research?domainRef=${encodeURIComponent(state.domainRef)}`); dialogState.progress = progress; drawDialog(); }
    catch (error) { dialogFeedback(`进度读取未成功：${error.message}`); }
  }
  async function startTopicResearch(ref) {
    const evidence = dialogState?.type === 'judge' && dialogState.topicRef === ref ? list(dialogState.works) : curated(ref, dialogState?.snapshot || snapshot);
    const wrefs = [...new Set(evidence.map(w => w.workRef))].slice(0,10);
    if (!wrefs.length) return dialogFeedback('当前没有可引用的精选材料，先查看已有来源或明确补充材料。');
    const result = await researchCommand({action:'start',trigger:'on_demand',workRefs:wrefs,topicRef:ref || null});
    if (result) dialogFeedback('按需研究已获得后端回执，已有材料继续可读；查看进度确认排队与结果。');
  }
  async function collectionCommand(payload,search = false) {
    if(pending)return null;pending=true;dialogFeedback('正在提交有界动作…');
    try { const result=await request(search ? '/api/local/topic-map/search/commands' : '/api/local/topic-map/collection/commands',{method:'POST',body:JSON.stringify({...payload,requestRef:uuid(),domainRef:state.domainRef})}); dialogFeedback(`已获得后端回执：${labelState(result.state)}。实际取得材料与研究结果不由提交成功代替。`);return result; }
    catch(error){dialogFeedback(`动作未完成：${error.message}`);return null;}finally{pending=false;}
  }
  async function submitCapture(form,data) {
    const purpose=String(data.get('purpose')).trim(),newRound=data.has('newRound');
    const payload=dialogState.workRef ? {action:'deepen_comments',topicRef:dialogState.topicRef || null,targetRef:String(data.get('targetRef')),workRef:dialogState.workRef,purpose,newRound} : {action:'start_search',topicRef:dialogState.topicRef || null,authorizationRef:String(data.get('authorizationRef')),keywords:String(data.get('keywords')).split(/[,，、\n]/).map(s=>s.trim()).filter(Boolean),purpose,newRound};
    if(payload.keywords && (payload.keywords.length<1 || payload.keywords.length>3))return dialogFeedback('本轮须明确1到3个搜索词。');
    const result=await collectionCommand(payload,!dialogState.workRef);if(result)await refreshCapture();
  }
  async function refreshCapture() {
    const current=dialogState;
    try {
      const results=await Promise.allSettled([request(`/api/local/topic-map/research?domainRef=${encodeURIComponent(state.domainRef)}`),request(`/api/local/topic-map/search?domainRef=${encodeURIComponent(state.domainRef)}`),request(`/api/local/topic-map?${query({topicRef:'',windowDays:'',platform:'',path:'',overlay:''})}`)]);
      if(dialogState!==current)return;
      if(results[0].status==='fulfilled')current.collection=results[0].value;else current.collectionError=results[0].reason.message;
      if(results[1].status==='fulfilled')current.search=results[1].value;else current.searchError=results[1].reason.message;
      if(results[2].status==='fulfilled')results[2].value.works.forEach(w=>extraWorks.set(w.workRef,w));
      current.loading=false;drawDialog();if(current.collectionError||current.searchError)dialogFeedback(`部分进度读取未成功：${current.collectionError||current.searchError}`);
    }catch(error){if(dialogState===current){current.loading=false;current.error=error.message;drawDialog();}}
  }
  async function openCapture(ref, workRef = null) { openDialog('capture',{topicRef:ref,workRef,loading:true});await refreshCapture(); }
  async function dispatchClick(event) {
    const target = event.target.closest('[data-action]'); if (!target || !root.contains(target)) return;
    const action = target.dataset.action, id = target.dataset.id || '';
    if (target.disabled) return;
    if (action === 'refresh') { await load(); return; }
    if (action === 'view') { state.view = id; persist(); render(); return; }
    if (action === 'summary-chart') { state.summaryChart=id;persist();render();return; }
    if (action === 'changes-mode') { state.changesOnly=id==='changes';if(state.changesOnly){state.view='overview';state.tab='structure';}persist();render();return; }
    if (action === 'tab') { state.tab = id; persist(); render(); return; }
    if (action === 'topic') { state.topicRef = id; state.query = ''; persist(); if (dialog) closeDialog(); await load(); return; }
    if (action === 'collapse') { const values = new Set(list(state.collapsed)); values.has(id) ? values.delete(id) : values.add(id); state.collapsed = [...values]; persist(); render(); return; }
    if (action === 'material-mode') { state.materialMode = id; persist(); render(); return; }
    if (action === 'stage') { state.stage = id; persist(); render(); return; }
    if (action === 'overlay') { state.overlay = id; state.stage = ''; persist(); await load(); return; }
    if (action === 'metric') { state.metric = id; persist(); render(); return; }
    if (action === 'map-mode') { state.mapMode = id; persist(); render(); return; }
    if (action === 'matrix') { await openMatrix(); return; }
    if (action === 'compare') { await openCompare(); return; }
    if (action === 'compare-clear') { state.compare = []; persist(); render(); return; }
    if (action === 'dialog-close') { closeDialog(); return; }
    if (action === 'dialog-back') { dialogBack(); return; }
    if (action === 'materials') { openDialog('materials',{snapshot:dialogState?.snapshot}); return; }
    if (action === 'subset') { openDialog('materials',{refs:id ? id.split(',') : []}); return; }
    if (action === 'unassigned') { openDialog('materials',{unassigned:true}); return; }
    if (action === 'materials-more') { dialogState.limit = (dialogState.limit || 60) + 60; drawDialog(); return; }
    if (action === 'reader') { await readWork(id); return; }
    if (action === 'comments-more') { const owner=dialogState,current=owner.type==='judge'?owner.inlineReader:owner;if(!current?.commentResource?.nextCursor)return;try{const next=await request(`/api/local/work-resources/${encodeURIComponent(current.work.workRef)}/comments?domain=${encodeURIComponent(state.domainRef)}&cursor=${encodeURIComponent(current.commentResource.nextCursor)}`);if(dialogState===owner&&(owner.type!=='judge'||owner.inlineReader===current)){current.commentResource={...next,items:[...list(current.commentResource.items),...list(next.items)]};drawDialog();}}catch(error){dialogFeedback(`已有评论读取未成功：${error.message}`);}return; }

    if (action === 'judge') { openDialog('judge',{topicRef:id,works:curated(id),tab:'discussion',loading:true}); const current=dialogState; try { const full=await request(`/api/local/topic-map?${query({topicRef:id,windowDays:'',platform:''})}`); full.works.forEach(w=>extraWorks.set(w.workRef,w)); if(dialogState===current){current.works=curated(id,full);current.snapshot=full;current.loading=false;drawDialog();} } catch(error){if(dialogState===current){current.loading=false;current.readError=error.message;drawDialog();}} return; }
    if (action === 'judge-tab') { dialogState.tab = id;drawDialog();if(id==='samples'&&!dialogState.inlineReader&&dialogState.works?.length)await readSample(dialogState.works[0].workRef);return; }
    if (action === 'sample') { await readSample(id);return; }
    if (action === 'replace-samples') { openDialog('replace',{judgeState:dialogState});dialogState.judgeState=dialogStack[dialogStack.length-1];drawDialog();return; }
    if (action === 'judge-product') { openDialog('product',{topicRef:dialogState.topicRef,works:dialogState.works});return; }
    if (action === 'judge-sources') { openDialog('materials',{snapshot:dialogState.snapshot || snapshot,refs:list(dialogState.works).filter(w=>!w.own).map(w=>w.workRef),label:'当前精选外部参考来源'});return; }
    if (action === 'judge-to-journey') { state.topicRef=id;state.view='journey';state.stage='';closeDialog();persist();await load();return; }
    if (action === 'own-reuse') { const w=work(id);if(!w)return;if(!readableMaterial(w))return dialogFeedback('当前没有可引用的来源，暂不能保留这篇作品的复用依据。');openDialog('save',{topicRef:dialogState.topicRef,kind:'angle',angle:{workRef:w.workRef,title:title(w),label:`复用我方已发布作品：${title(w)}`,answerTask:'',researchMethodVersion:snapshot.methodVersion}});return; }
    if (action === 'judge-own-all') { const source=dialogState.snapshot || snapshot, refs=ownTopicWorks(dialogState.topicRef,source).map(w=>w.workRef);openDialog('materials',{snapshot:source,refs,label:'本主题全部已确认我方发布原作'});return; }
    if (action === 'identity') { await openIdentity(); return; }
    if (action === 'structure-add-destination') { dialogState.destinationCount = Math.min(10,(dialogState.destinationCount || 2)+1); drawDialog(); return; }
    if (action === 'structure-edit') { delete dialogState.preview; drawDialog(); return; }
    if (action === 'structure-apply') { try { const result=await request('/api/local/topic-map/structure/apply',{method:'POST',body:JSON.stringify({plan:dialogState.plan,previewHash:dialogState.preview.previewHash})}); await load(); dialogFeedback(`结构调整已保存 · ${result.destinations.length} 个新候选定义。旧历史与备选引用保留。`); target.disabled=true; } catch(error){dialogFeedback(`结构调整未完成：${error.message}`);} return; }
    if(action==='move-topic'){openDialog('move',{moveTopicRef:state.topicRef});return;}
    if (action === 'structure-change') { openDialog('structure',{kind:'merge',destinationCount:1}); return; }
    if (action === 'viewed') { const t=topic(id); const result=await command({action:'viewed',topicRef:id,definitionRef:t.definitionRef,workRefs:list(snapshot.changes).find(c=>c.topicRef===id)?.lastViewedWorkRefs || []},'已记录此主题定义的查看'); if(result)drawDialog(); return; }
    if (action === 'rules' || action === 'method' || action === 'changes') { openDialog(action); return; }
    if (action === 'saved') { await openSaved(); return; }
    if (action === 'saved-materials') { await openAlternative(id); return; }
    if (action === 'export-alternative') { await exportAlternative(id); return; }
    if (action === 'matrix-cell') {
      const [ref,stage] = id.split('|'), source = dialogState.snapshot, t = list(source.topics).find(t => t.topicRef === ref);
      const refs = stage === 'other' ? list(t.journey.main).filter(e => !stageNames[e.stage]).flatMap(e => list(e.workRefs)) : list(dialogState.metric === 'involved' ? t.journey.involved : t.journey.main).find(e => e.stage === stage)?.workRefs || [];
      openDialog('materials',{refs:[...new Set(refs)],snapshot:source,label:`${t.displayName} · ${stageNames[stage] || '其他 / 未归主阶段'}`}); return;
    }
    if (action === 'save-angle') {
      const angles = anglesFrom(dialogState.works);
      const a = angles[Number(id)]; if (a) openDialog('save',{angle:a,topicRef:dialogState.topicRef,kind:'angle'}); return;
    }
    if (action === 'save-opportunity') {
      const opportunities=opportunitiesFrom(analysisOutputs(dialogState.works || selectedWorks())),o=opportunities[Number(id)];if(!o)return;
      const angle={...o,title:Array.from(o.need).slice(0,120).join(''),label:`具体需求：${o.need}\n\n支持假设：${o.hypothesis}`,answerTask:`最先验证：${o.verificationQuestion}\n\n替代解释：${o.alternativeExplanation}`};
      openDialog('save',{angle,topicRef:dialogState.topicRef || state.topicRef,kind:'product_research'});return;
    }
    if (action === 'breakout') {
      const current = dialogState, w = work(id);
      const result = await command({action:'breakout',workRef:id,marked:!w.ownBreakout},'我方爆款标记已保存');
      if (result && dialogState === current) { current.work = work(id); if(current.type==='judge'&&current.inlineReader?.work?.workRef===id)current.inlineReader.work=work(id); drawDialog(); dialogFeedback('手动标记已获得后端回执。未标记不等于表现低。'); } else dialogFeedback(notice); return;
    }
    if (action === 'own-remove') {
      const [platform,authorExternalId] = id.split('|'); const result = await command({action:'ownCreator',platform,authorExternalId,active:false},'我方账号范围已更新'); if (result) await refreshIdentity(); else dialogFeedback(notice); return;
    }
    if (action === 'research') { await openResearch(); return; }
    if (action === 'research-refresh') { await refreshResearch(); return; }
    if (['research-pause','research-resume','research-stop'].includes(action)) { const result = await researchCommand({action:action.replace('research-',''),runRef:id}); if (result) await refreshResearch(); return; }
    if (action === 'start-topic-research') { await startTopicResearch(id); return; }
    if (action === 'capture-refresh') {await refreshCapture();return;}
    if (action === 'search-stop') { const result=await collectionCommand({action:'stop_search',roundRef:id},true);if(result)await refreshCapture();return;}
    if (action === 'collection-stop') { const result=await collectionCommand({action:'stop_collection',roundRef:id});if(result)await refreshCapture();return;}
    if (action === 'capture') { await openCapture(id); return; }
    if (action === 'deep-comments') { await openCapture(state.topicRef,id); return; }
    if (action === 'dialog-retry') { const current = dialogState; if (current.type === 'research') await openResearch(); else if (current.type === 'matrix') await openMatrix(); else if (current.type === 'saved') await openSaved(); else if (current.type === 'reader') await readWork(current.work.workRef); else if(current.type==='capture')await openCapture(current.topicRef,current.workRef);else if(current.type==='alternative')await openAlternative(current.alternativeRef); return; }
  }
  root.addEventListener('click', dispatchClick);
  root.addEventListener('change', async event => {
    const target = event.target;
    if (target.dataset.compare) {
      const values = new Set(list(state.compare)); target.checked ? values.add(target.dataset.compare) : values.delete(target.dataset.compare);
      if (values.size > 3) { target.checked = false; notice = '一次最多比较三个方向，保留可读的并排信息。'; renderNotice(); return; }
      state.compare = [...values]; persist(); render(); return;
    }
    if (target.dataset.filter) {
      state[target.dataset.filter] = target.value;
      if(target.dataset.filter==='plotPlatform'){persist();render();return;}
      if (target.dataset.filter === 'domainRef') { state.topicRef = ''; state.compare = []; }
      persist(); await load();
    }
  });
  root.addEventListener('submit', event => {
    if (event.target.dataset.form !== 'search') return;
    event.preventDefault(); state.query = String(new FormData(event.target).get('query') || '').trim(); persist(); render();
  });
  root.addEventListener('keydown', event => { const target = event.target.closest('g[data-action]'); if (target && ['Enter',' '].includes(event.key)) { event.preventDefault(); target.dispatchEvent(new MouseEvent('click',{bubbles:true})); } });
  load();
})();
