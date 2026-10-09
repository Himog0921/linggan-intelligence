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
  const defaults = {domainRef:'',topicRef:'',platform:'',windowDays:'',referenceWindowDays:'30',view:'overview',tab:'structure',path:'',overlay:'',stage:'',metric:'main',mapMode:'map',materialMode:'latest',query:'',collapsed:[],compare:[]};
  let restored = {};
  try { restored = JSON.parse(sessionStorage.getItem('linggan.topic-map.view') || '{}'); } catch (_) {}
  const state = {...defaults, ...restored};
  if (!views.some(([id]) => id === state.tab)) state.tab = 'structure';
  if (!['overview','journey'].includes(state.view)) state.view = 'overview';
  const params = new URLSearchParams(location.search);
  ['domainRef','topicRef','platform','windowDays','referenceWindowDays'].forEach(key => { if (params.has(key)) state[key] = params.get(key); });
  const extraWorks = new Map();
  let snapshot = null, progress = null, loading = false, readError = '', notice = '', pending = false;
  let dialog = null, dialogState = null, dialogStack = [], triggerElement = null, fetchEpoch = 0;
  const title = work => work.title || '标题尚未取得';
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
    return `<div class="lgi-tm-pagehead"><div><span class="lgi-tm-eyebrow">领域知识与来源材料</span><h1>主题图谱</h1><p>从领域结构查看内容，再用具体材料判断下一次切入。</p></div><div class="lgi-tm-row">${btn('我的备选','saved')}${btn('研究进度与设置','research')}${btn('我方矩阵','identity')}</div></div><div class="lgi-tm-main-tabs" aria-label="主题图谱视图">${tab('主题概览','view','overview',state.view === 'overview')}${tab('用户生命旅程','view','journey',state.view === 'journey')}</div>`;
  }
  function controls() {
    const domains = list(snapshot?.domains).map(d => [d.domainRef || d.id,d.displayName || d.name]);
    return `<div class="lgi-tm-toolbar">${select('domainRef','领域', domains.length ? domains : [['','领域尚未读取']])}${select('platform','平台',[['','全部平台'],['xhs','小红书'],['douyin','抖音']])}${select('windowDays','概览统计窗口',[['','全部发布时间'],['7','近7天'],['30','近30天'],['90','近90天']])}${state.view === 'journey' ? select('path','明确经历',[['','全部经历'],['family','家庭支持'],['adult','成年自我管理']]) : ''}<form class="lgi-tm-search" data-form="search"><label for="topic-map-search">查找当前范围的作品 / 主题</label><div class="lgi-tm-row"><input id="topic-map-search" name="query" value="${esc(state.query)}" placeholder="输入关键词"><button class="lgi-tm-button" type="submit">查找</button></div></form>${btn('刷新已有结果','refresh')}</div>`;
  }
  function tree() {
    const topics = list(snapshot?.topics).filter(t=>t.lifecycleState!=='superseded'), roots = topics.filter(t => !t.parentTopicRef || !topics.some(x => x.topicRef === t.parentTopicRef));
    function branch(t, trail = new Set()) {
      if (trail.has(t.topicRef)) return '';
      const next = new Set(trail); next.add(t.topicRef);
      const children = topics.filter(c => c.parentTopicRef === t.topicRef), expanded = !list(state.collapsed).includes(t.topicRef);
      return `<li><div class="lgi-tm-tree-row ${state.topicRef === t.topicRef ? 'is-active' : ''}">${children.length ? btn(expanded ? '−' : '+','collapse',t.topicRef,'lgi-tm-tree-toggle',`aria-label="${expanded ? '收起' : '展开'}${esc(t.displayName)}" aria-expanded="${expanded}"`) : '<span class="lgi-tm-tree-spacer"></span>'}${btn(esc(t.displayName),'topic',t.topicRef,'lgi-tm-tree-name')}${t.lifecycleState === 'candidate' ? '<span class="lgi-tm-candidate-dot" title="候选方向">候选</span>' : ''}<span class="lgi-tm-num">${number(t.statistics?.workCount)}</span></div>${children.length && expanded ? `<ul>${children.map(c => branch(c,next)).join('')}</ul>` : ''}</li>`;
    }
    return `<aside class="lgi-tm-tree" aria-label="稳定主题结构"><div class="lgi-tm-section-head"><h2>领域结构</h2>${btn('调整归属','move-topic','','lgi-tm-quiet')}</div><div class="lgi-tm-tree-row ${!state.topicRef ? 'is-active' : ''}">${btn('全部方向','topic','','lgi-tm-tree-name')}<span class="lgi-tm-num">${number(snapshot?.scope?.totalWorkCount)}</span></div><ul>${roots.map(t => branch(t)).join('')}</ul>${topics.length ? '' : '<p class="lgi-tm-note">当前领域尚未建立主题定义，已入库材料仍然可读。</p>'}<p class="lgi-tm-note">每个数字为去重作品，子主题之间可以重叠。</p></aside>`;
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
    return `<div class="lgi-tm-atlas">${tree()}<main class="lgi-tm-main">${identity()}${metrics(stats)}${coverage()}<div class="lgi-tm-work-tabs" aria-label="概览工作视角">${views.map(([id,name]) => tab(name,'tab',id,state.tab === id)).join('')}</div>${state.tab === 'structure' ? structure() : state.tab === 'patterns' ? patterns() : state.tab === 'sources' ? sources() : state.tab === 'candidates' ? candidates() : actionView()}${state.tab !== 'sources' && state.tab !== 'candidates' ? materialsSection() : ''}</main></div>`;
  }
  function children() { return list(snapshot?.topics).filter(t=>t.lifecycleState!=='superseded').filter(t => state.topicRef ? t.parentTopicRef === state.topicRef : !t.parentTopicRef); }
  function structure() {
    const rows = children().filter(t => !state.query || t.displayName.includes(state.query));
    return `${rows.length ? `<div class="lgi-tm-section-head"><div><h3>主题供给分布</h3><p>点名称下钻，勾选最多三个方向并排比较。</p></div>${btn('并排比较','compare','','',list(state.compare).length ? '' : 'disabled title="先勾选方向"')}</div><div class="lgi-tm-table-scroll"><table><thead><tr><th>比较</th><th>主题与范围</th><th>作品数</th><th>已观察作者</th><th>平台表现 / 点赞中位数 / P90</th><th>判断</th></tr></thead><tbody>${rows.map(t => `<tr><td><input type="checkbox" data-compare="${esc(t.topicRef)}" aria-label="比较${esc(t.displayName)}" ${list(state.compare).includes(t.topicRef) ? 'checked' : ''}></td><td>${btn(esc(t.displayName),'topic',t.topicRef,'lgi-tm-title-link')}<p>${esc(t.definitionText)}</p>${t.lifecycleState === 'candidate' ? badge('候选方向',true) : ''}</td><td class="lgi-tm-num">${number(t.statistics?.workCount)}</td><td class="lgi-tm-num">${number(t.statistics?.authorCount)}</td><td>${platformStats(t.statistics).map(p => `<p>${esc(platformName(p.platform))} · ${p.highPerformanceThreshold == null ? '样本规则未设置' : pct(p.highPerformanceCount,p.knownLikeCount)}<br><span class="lgi-tm-num">${number(p.medianLikes)} / ${number(p.p90Likes)}</span></p>`).join('') || '指标未知'}</td><td>${btn('判断这个方向','judge',t.topicRef,'lgi-tm-quiet')}</td></tr>`).join('')}</tbody></table></div><p class="lgi-tm-note">子主题可以重叠，父主题按作品去重；不把样本占比当市场份额。</p>` : '<div class="lgi-tm-section-head"><div><h3>当前方向的内容与表现</h3><p>叶主题继续展示完整统计与作品，不要求先启动研究。</p></div></div>'}${distribution()}`;
  }
  function distribution() {
    const ps = platformStats(topic(state.topicRef)?.statistics || snapshot.statistics);
    return `<section class="lgi-tm-section"><div class="lgi-tm-section-head"><div><h3>同平台表现分布</h3><p>横轴为作品点赞数，纵轴为当前读取集合作品数；未知点赞不作为0绘图。</p></div></div><div class="lgi-tm-distributions">${ps.map(p => {
      const ns = selectedWorks().filter(w => w.platform === p.platform && known(w.likes));
      const bins = [[0,20,'0–20'],[21,99,'21–99'],[100,499,'100–499'],[500,999,'500–999'],[1000,4999,'1,000–4,999'],[5000,Infinity,'≥5,000']].map(([min,max,label]) => ({label,count:ns.filter(w => w.likes >= min && w.likes <= max).length}));
      const max = Math.max(1,...bins.map(b => b.count));
      return `<div><h4>${esc(platformName(p.platform))}</h4><div class="lgi-tm-histogram" role="img" aria-label="${esc(platformName(p.platform))} 当前读取集合点赞分布">${bins.map(b => `<div><span class="lgi-tm-num">${b.count}</span><div class="lgi-tm-bar-slot"><span class="lgi-tm-bar" style="height:${b.count / max * 100}%"></span></div><small>${b.label}</small></div>`).join('')}</div><p class="lgi-tm-note">${ns.length} 篇当前已读取的已知点赞作品 · 服务端范围共 ${number(p.workCount)} 篇</p></div>`;
    }).join('') || empty('当前没有可绘制的点赞数据','原始材料仍可查看。')}</div></section>`;
  }
  function annotation(work) { return work.annotation && typeof work.annotation === 'object' ? work.annotation : {}; }
  function patterns() {
    const ns = selectedWorks(), groups = new Map();
    ns.forEach(w => { const key = list(w.media?.video?.items).length ? '视频作品' : list(w.media?.images).length ? '图文作品' : '原始类型尚未取得'; if (!groups.has(key)) groups.set(key,[]); groups.get(key).push(w); });
    return `<div class="lgi-tm-section-head"><div><h3>这些内容，实际在怎样讲？</h3><p>作品形态引用实际媒体，讲法与场景引用已保存研究；未知仍保留原文入口。</p></div></div><div class="lgi-tm-pattern-grid">${[...groups].map(([kind,ws]) => `<article class="lgi-tm-panel"><span class="lgi-tm-eyebrow">当前读取集合</span><h4>${esc(kind)}</h4><p>${ws.length} 篇作品</p>${ws[0]?.evidenceFragment ? `<blockquote>${esc(fragmentText(ws[0].evidenceFragment))}</blockquote>` : '<p class="lgi-tm-note">没有可定位片段时不补写代表原声。</p>'}${btn('查看这些作品','subset',ws.map(w => w.workRef).join(','),'lgi-tm-quiet')}</article>`).join('')}</div><section class="lgi-tm-section"><h3>具体讨论与场景</h3>${sceneList(ns)}</section>`;
  }
  function sceneList(ns) {
    const scenes = ns.flatMap(w => list(w.research?.output?.scenes).map(scene => ({...scene,workRef:w.workRef})));
    return scenes.length ? `<div class="lgi-tm-scene-grid">${scenes.map(s => `<article class="lgi-tm-panel"><h4>${esc(s.label)}</h4><p>${list(s.evidence).length} 条具体定位依据</p>${btn('看来源与回应','reader',s.workRef,'lgi-tm-quiet')}</article>`).join('')}</div>` : empty('当前材料尚未形成可引用的场景分组','阶段尚未确定不影响原作与已有评论的阅读。');
  }
  function sources() {
    const rows = list(snapshot.sources).slice().sort((a,b) => b.workCount-a.workCount);
    return `<div class="lgi-tm-section-head"><div><h3>这些材料主要来自谁？</h3><p>作者贡献引用当前服务端统计范围，作品按身份去重。</p></div>${btn('我方与对标范围','identity')}</div><div class="lgi-tm-table-scroll"><table><thead><tr><th>创作者</th><th>作品 / 样本占比</th><th>主要方向</th><th>样本表现 / 可判定分母</th><th>作品</th></tr></thead><tbody>${rows.map(r => `<tr><td><strong>${esc(r.creatorDisplayName || '作者未知')}</strong><p>${esc(platformName(r.platform))}</p></td><td><b class="lgi-tm-num">${number(r.workCount)}</b> 篇 · ${pct(r.workCount,snapshot.statistics.workCount)}</td><td>${list(r.topicRefs).map(ref => esc(topicName(ref))).join('、') || '尚未归主题'}</td><td>${platformStats(r.statistics).map(p => `<p>${p.highPerformanceThreshold == null ? '样本规则未设置' : `${number(p.highPerformanceCount)} / ${number(p.knownLikeCount)} · ${pct(p.highPerformanceCount,p.knownLikeCount)}`}<br>${number(p.unknownLikeCount)} 篇点赞未知</p>`).join('')}</td><td>${btn('查看作品','subset',list(r.workRefs).join(','),'lgi-tm-quiet')}</td></tr>`).join('')}</tbody></table></div>${rows.length ? '' : empty('当前范围没有作者作品')}<p class="lgi-tm-note">作品可由多条采集路径取得，总量只计一次。不据来源集中程度判断作者能力。</p>${materialsSection()}`;
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
  function card(w) {
    const cover = coverUrl(w);
    return `<article class="lgi-tm-material"><button type="button" class="lgi-tm-cover" data-action="reader" data-id="${esc(w.workRef)}" aria-label="打开${esc(title(w))}">${cover ? `<img src="${esc(cover)}" loading="lazy" alt="${esc(title(w))}的已取得封面">` : '<span>▧<br>封面尚未采集</span>'}</button><div class="lgi-tm-row lgi-tm-card-meta"><span>${esc(platformName(w.platform))}</span>${w.own ? badge(w.ownBreakout ? '我方手动爆款' : '我方作品',w.ownBreakout) : ''}</div><h4>${btn(esc(title(w)),'reader',w.workRef,'lgi-tm-title-link')}</h4><p class="lgi-tm-author" title="${esc(w.creatorDisplayName || '作者未知')}">${esc(w.creatorDisplayName || '作者未知')}</p><p class="lgi-tm-note">${esc(w.publishedAtSourceText || w.publishedAt || '发布时间未知')}</p><div class="lgi-tm-interactions"><span title="点赞精确值 ${number(w.likes)}">赞 <b>${number(w.likes)}</b></span><span title="收藏精确值 ${number(w.collects)}">藏 <b>${number(w.collects)}</b></span><span title="评论精确值 ${number(w.comments)}">评 <b>${number(w.comments)}</b></span><span title="分享精确值 ${number(w.shares)}">分享 <b>${number(w.shares)}</b></span></div></article>`;
  }
  function materialsSection() {
    const ns = sortedMaterials(selectedWorks());
    return `<section class="lgi-tm-section"><div class="lgi-tm-section-head"><div><h3>把数字还原成内容</h3><p>原作与全部材料直接可读，不受精选10篇上限裁切。</p></div><div class="lgi-tm-row">${[['latest','最新作品'],['high','高表现'],['ordinary','常规作品']].map(([id,name]) => tab(name,'material-mode',id,state.materialMode === id)).join('')}</div></div>${ns.length ? `<div class="lgi-tm-material-grid">${ns.slice(0,6).map(card).join('')}</div>` : empty('当前条件没有对应作品',state.materialMode === 'latest' ? '可以调整范围；读取失败与没有材料分别表达。' : '未设置样本表现规则时，不擅自划分高表现与常规作品。')}<div class="lgi-tm-section-head"><p>展示 ${Math.min(6,ns.length)} / ${ns.length} 篇当前读取集合</p>${btn('查看全部材料','materials','','lgi-tm-quiet')}</div></section>`;
  }
  function journeyEntries(journey, mode = state.metric) { return list(mode === 'involved' ? journey?.involved : journey?.main); }
  function stages(journey) { return fallbackStages.map(s => ({...s,...journeyEntries(journey).find(e => e.stage === s.stage)})); }
  function journey() {
    const j = topic(state.topicRef)?.journey || snapshot.journey, stations = stages(j);
    const coordinates = [[140,90],[400,90],[660,90],[530,280],[230,280]];
    const map = `<div class="lgi-tm-map-shell"><svg class="lgi-tm-journey-map" viewBox="0 0 820 440" role="group" aria-label="五个主阶段的内容旅程地图"><path class="lgi-tm-route" d="M140 90H670C790 90 790 280 670 280H230"/>${stations.map((s,i) => {const [x,y] = coordinates[i];return `<g role="button" tabindex="0" class="lgi-tm-station ${state.stage === s.stage ? 'is-active' : ''}" data-action="stage" data-id="${s.stage}" aria-label="${esc(s.label)} ${number(s.count)}篇，点击查看材料"><rect class="lgi-tm-station-plate" x="${x-108}" y="${y-48}" width="216" height="144"/><text x="${x-82}" y="${y-20}" class="lgi-tm-station-index">0${i+1}</text><text x="${x}" y="${y+5}" text-anchor="middle" class="lgi-tm-station-label">${esc(s.label)}</text><text x="${x}" y="${y+45}" text-anchor="middle" class="lgi-tm-station-count">${number(s.count)} 篇 · ${pct(s.count,j?.denominator)}</text></g>`;}).join('')}<text x="28" y="422" class="lgi-tm-route-note">允许返回、跳过与并行。路径不表示人数、真实流转或转化率。</text></svg></div>`;
    const ns = selectedWorks().filter(w => !state.stage || (state.metric === 'involved' ? list(w.involvedStages).includes(state.stage) : w.mainStage === state.stage));
    return `<main class="lgi-tm-journey"><div class="lgi-tm-object-head"><div><span class="lgi-tm-eyebrow">内容回应的经历</span><h2>这些作品，覆盖了哪一段经历？</h2><p>${number(j?.denominator)} 篇可读作品 · 主阶段描述内容，不给作者贴人生标签。</p></div>${btn('阶段与统计依据','method')}</div><div class="lgi-tm-toolbar">${[['','全部状态'],['obstruction_recurrence','受阻与反复'],['transition_handoff','环境转换与支持交接']].map(([id,name]) => tab(name,'overlay',id,state.overlay === id)).join('')}<span class="lgi-tm-grow"></span>${tab('主阶段占比','metric','main',state.metric === 'main')}${tab('涉及率','metric','involved',state.metric === 'involved')}${tab('地图','map-mode','map',state.mapMode === 'map')}${tab('列表','map-mode','list',state.mapMode === 'list')}</div>${state.mapMode === 'map' ? map : `<div class="lgi-tm-stage-list">${stations.map(s => `<button class="lgi-tm-panel ${state.stage === s.stage ? 'is-active' : ''}" data-action="stage" data-id="${s.stage}"><h3>${esc(s.label)}</h3><strong class="lgi-tm-value">${number(s.count)}</strong><p>${pct(s.count,j?.denominator)} · 当前范围作品</p></button>`).join('')}</div>`}<div class="lgi-tm-row lgi-tm-other-stages">${journeyEntries(j).filter(s => !stageNames[s.stage]).map(s => btn(`${esc(otherNames[s.stage] || '其他 / 未归主阶段')} ${number(s.count)} · ${pct(s.count,j.denominator)}`,'stage',s.stage,'lgi-tm-quiet')).join('')}${btn('主题 × 旅程','matrix')}</div><p class="lgi-tm-note">${state.metric === 'main' ? '五主阶段加其他类别以全部可读作品为分母；保留未分析，不舍弃未知。' : '同篇可以实质涉及多个阶段，涉及率合计可以超过100%。'}${state.overlay ? ' 已应用跨阶段覆盖筛选，分母为当前筛选后的作品。' : ''}</p><div class="lgi-tm-section-head"><div><h3>${state.stage ? `${stageNames[state.stage] || otherNames[state.stage] || '当前阶段'}：具体讨论与场景` : '从具体场景，进入关联主题'}</h3><p>阶段不明确时，已有原作和场景仍可查看。</p></div>${state.stage ? btn('回到全部阶段','stage','') : ''}</div>${sceneList(ns)}<div class="lgi-tm-material-grid">${ns.slice(0,6).map(card).join('')}</div>${btn('查看对应全部材料','subset',ns.map(w => w.workRef).join(','),'lgi-tm-quiet')}</main>`;
  }
  function render() {
    const focused = document.activeElement;
    const keepFocus = focused?.id && root.contains(focused) ? focused.id : null;
    content.innerHTML = `${header()}${controls()}<div data-notice class="lgi-tm-feedback" role="status" ${notice ? '' : 'hidden'}>${esc(notice)}</div>${readError ? `<div class="lgi-tm-read-error" role="alert"><strong>当前读取未成功</strong><p>${esc(readError)}</p>${btn('重新读取','refresh')}${snapshot ? '<p>下面保留上一次已读取结果；并非当前更新已经完成。</p>' : ''}</div>` : ''}${loading ? '<div class="lgi-tm-loading" role="status">正在读取已有材料与统计…</div>' : ''}${snapshot ? state.view === 'overview' ? overview() : journey() : loading ? '' : empty('当前图谱尚未读取','页面读取不启动模型研究或平台采集。')}<footer class="lgi-tm-footer"><span>${esc(snapshot?.methodVersion || '方法尚未读取')}</span><span>样本统计、来源材料与研究判断分别保留边界</span></footer>${list(state.compare).length ? `<div class="lgi-tm-compare-dock"><span>已选 ${state.compare.length} 个方向 · ${state.compare.map(topicName).map(esc).join(' / ')}</span>${btn('并排比较','compare','','lgi-tm-primary')}${btn('清空','compare-clear','','lgi-tm-quiet')}</div>` : ''}`;
    if (keepFocus) document.getElementById(keepFocus)?.focus({preventScroll:true});
    renderNotice();
  }
  function openDialog(type, data = {}, push = true) {
    if (dialog && push && dialogState) dialogStack.push({...dialogState,scroll:dialog.querySelector('.lgi-tm-dialog-body')?.scrollTop || 0});
    if (!dialog) {
      triggerElement = document.activeElement;
      dialog = document.createElement('dialog'); dialog.className = 'lgi-tm-dialog'; dialog.setAttribute('aria-labelledby','topic-map-dialog-title'); root.appendChild(dialog);
      dialog.addEventListener('click', event => { if (event.target === dialog) closeDialog(); else { event.stopPropagation(); dispatchClick(event); } });
      dialog.addEventListener('cancel', event => { event.preventDefault(); closeDialog(); });
      dialog.addEventListener('submit', submitDialog);
      dialog.addEventListener('change', event => { const target = event.target; if(target.dataset.detailChoice){const chosen=dialog.querySelectorAll(`[data-detail-choice="${target.dataset.detailChoice}"]:checked`);if(chosen.length>10){target.checked=false;dialogFeedback('一轮最多冻结10篇独立作品详情。');}return;} if (target.dataset.structureSource) { const set = new Set(list(dialogState.sources)); target.checked ? set.add(target.dataset.structureSource) : set.delete(target.dataset.structureSource); dialogState.sources = [...set]; drawDialog(); return; } if (target.dataset.dialogFilter) { dialogState[target.dataset.dialogFilter] = target.value; drawDialog(); } });
    }
    dialogState = {type,...data}; drawDialog(); if (!dialog.open) dialog.showModal();
  }
  function drawDialog() {
    if (!dialog || !dialogState) return;
    const {type} = dialogState;
    const names = {capture:'有界补充材料',alternative:'保存的原版本与依据',move:'调整主题归属',structure:'主题结构调整',matrix:'主题 × 旅程',compare:'并排比较',materials:'全部材料',reader:'原作与讨论',judge:'选题判断',identity:'我方矩阵与观察范围',saved:'我的备选',save:'保留角度与依据',research:'研究进度与设置',rules:'样本表现规则',method:'方法与统计边界',changes:'结构与依据变化',product:'产品机会：需求与支持假设'};
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
  async function readWork(ref) {
    const w = work(ref) || dialogState?.snapshot?.works?.find(w => w.workRef === ref);
    if (!w) return;
    openDialog('reader',{work:w,loading:true}); const current = dialogState;
    try {
      const domain = encodeURIComponent(state.domainRef);
      const values = await Promise.allSettled([request(`/api/local/work-resources/${encodeURIComponent(ref)}?domain=${domain}`),request(`/api/local/work-resources/${encodeURIComponent(ref)}/comments?domain=${domain}`)]);
      if (dialogState !== current) return;
      current.resource = values[0].status === 'fulfilled' ? values[0].value : null;
      current.resourceError = values[0].status === 'rejected' ? values[0].reason.message : '';
      current.commentResource = values[1].status === 'fulfilled' ? values[1].value : null;
      current.commentError = values[1].status === 'rejected' ? values[1].reason.message : '';
    } catch (error) { if (dialogState === current) current.resourceError = error.message; }
    finally { if (dialogState === current) { current.loading = false; drawDialog(); } }
  }
  function fragmentText(fragment) { return typeof fragment === 'string' ? fragment : fragment?.text || ''; }
  function sourceKind(kind) { return ({detail_body:'作者正文',ocr_text:'图片OCR',image_substantive_text:'图片原文',frame_ocr_text:'视频帧OCR',asr_text:'音视频转写',ocr:'图片OCR',transcript:'音视频转写',title:'作品标题',comment_body:'评论原声',comment:'评论原声',clean_comment:'评论原声',unresearched_comment:'评论原声',studied_comment:'评论原声',body:'作者正文'}[kind] || '来源片段'); }
  function highlighted(text, spans = []) {
    const chars = Array.from(text || ''), valid = list(spans).filter(s => Number.isInteger(s.start) && Number.isInteger(s.end) && s.start >= 0 && s.end > s.start && s.end <= chars.length).sort((a,b) => a.start - b.start);
    let position = 0, out = '';
    valid.forEach(s => { if (s.start < position) return; out += esc(chars.slice(position,s.start).join('')) + `<mark>${esc(chars.slice(s.start,s.end).join(''))}</mark>`; position = s.end; });
    return out + esc(chars.slice(position).join(''));
  }
  function readerDialog() {
    const w = dialogState.work, r = dialogState.resource;
    const item = r?.item, fragments = dialogState.resourceError ? [] : list(w.research?.fragments), body = item?.inspector?.detailCurrent?.body?.value;
    const raw = body || fragmentText(item?.evidenceFragment) || (dialogState.loading && !dialogState.resourceError ? fragmentText(w.evidenceFragment) : '');
    const comments = list(dialogState.commentResource?.items);
    const source = '';
    const a = annotation(w), analysis = w.research?.output || {};
    return `<div class="lgi-tm-reader-head"><div><h3>${esc(title(w))}</h3><p>${esc(w.creatorDisplayName || '作者未知')} · ${esc(platformName(w.platform))} · ${esc(w.publishedAtSourceText || w.publishedAt || '发布时间未知')}</p><div class="lgi-tm-row">${badge(w.titleSource === 'cover_ocr' ? '标题来自封面OCR' : '作品标题')}${w.own ? btn(w.ownBreakout ? '取消我方爆款标记' : '标记我方爆款','breakout',w.workRef,'', 'data-mutation') : ''}${source ? `<a class="lgi-tm-button" href="${esc(source)}" target="_blank" rel="noopener noreferrer">打开来源页面</a>` : ''}</div></div><div class="lgi-tm-interactions">赞 ${number(w.likes)} · 藏 ${number(w.collects)} · 评 ${number(w.comments)} · 分享 ${number(w.shares)}</div></div><div class="lgi-tm-reader-layout"><section><h4>原文与可定位片段</h4>${dialogState.loading ? '<p role="status">正在读取原文与评论，已有片段先展示。</p>' : ''}${dialogState.resourceError ? `<p class="lgi-tm-read-error">原文读取未成功：${esc(dialogState.resourceError)}</p>` : ''}${raw ? `<div class="lgi-tm-original">${highlighted(raw,bodyCitations(w))}</div>${!body ? '<p class="lgi-tm-note">当前为有界来源片段；完整原文读取与来源资格以材料检查器为准。</p>' : ''}` : empty('当前没有可读取的正文片段','缺少正文不等于原作没有回应。')}${fragments.map(f => `<article class="lgi-tm-fragment"><span>${esc(sourceKind(f.sourceKind || f.field))}</span><blockquote>${highlighted(fragmentText(f),citationSpans(w,f.fragmentId))}</blockquote></article>`).join('')}<a class="lgi-tm-button" href="/corpus/evidence?work=${encodeURIComponent(w.workRef)}&domain=${encodeURIComponent(state.domainRef)}">在材料检查器定位</a></section><aside class="lgi-tm-analysis"><h4>模型解释 / 已保存版本</h4>${a.rationale || analysis.journey?.rationale ? `<p>${esc(a.rationale || analysis.journey?.rationale)}</p>` : '<p>此作品尚无可引用的主题分析，不用摘要代替原声。</p>'}<p>${esc(stageNames[w.mainStage] || otherNames[w.mainStage] || '阶段未知')} · ${esc(list(w.overlays).map(v => v === 'obstruction_recurrence' ? '受阻与反复' : '环境转换与交接').join('、'))}</p>${list(analysis.limitations).map(t => `<p class="lgi-tm-note">${esc(t)}</p>`).join('')}<h4>已有回应关系</h4>${list(analysis.responseMatches).map(m => `<p>${esc(({direct:'有直接回应依据',partial:'部分回应',unmatched:'未找到匹配回应',unknown:'回应未知',not_applicable:'当前不适用'})[m.status] || m.status)}</p>`).join('') || '<p>回应尚未研究，不能把评论问题自动当原作回答。</p>'}</aside></div><section class="lgi-tm-section"><div class="lgi-tm-section-head"><div><h3>已有评论原声</h3><p>评论条数不等于人数，作者观点与评论分别保留来源。</p></div>${btn('深入看这篇的讨论','deep-comments',w.workRef,'', 'data-mutation')}</div>${dialogState.commentError ? `<p class="lgi-tm-read-error">评论读取未成功：${esc(dialogState.commentError)}</p>` : ''}<p class="lgi-tm-note">当前展示 ${comments.length} / ${number(dialogState.commentResource?.total)} 条当前可读评论。</p>${comments.length ? comments.map(c => `<article class="lgi-tm-comment"><blockquote>${esc(c.body || '正文未知')}</blockquote><p>${esc('评论作者身份已隐去')} · ${esc(c.sourceRef || '')}</p>${c.parentText ? `<details><summary>父评论上下文</summary><blockquote>${esc(c.parentText)}</blockquote></details>` : ''}</article>`).join('') : empty(dialogState.loading ? '评论正在读取' : '当前没有可展示的已有评论','不会为了填满原声区域发起采集。')}${dialogState.commentResource?.nextCursor ? btn('读取下一页已有评论','comments-more') : ''}</section>`;
  }
  function curated(ref, source = snapshot) {
    const t = list(source.topics).find(t => t.topicRef === ref);
    const candidates = list(source.works).filter(w => w.readable && (!t || list(w.topicRefs).includes(ref) || list(t.workRefs).includes(w.workRef)));
    const recent = new Set(list(source.scope.recentReferenceWorkRefs));
    const eligible = candidates.filter(w => w.own ? Boolean(w.publishedAt || w.publishedAtSourceText) : recent.has(w.workRef));
    const high = eligible.filter(w => w.own ? w.ownBreakout : (known(w.followerCount) && w.followerCount <= 1000 && known(w.likes) && w.likes >= 500) || (known(platformStats(source.statistics).find(p => p.platform === w.platform)?.highPerformanceThreshold) && known(w.likes) && w.likes >= platformStats(source.statistics).find(p => p.platform === w.platform).highPerformanceThreshold));
    high.sort((a,b) => Number(b.ownBreakout)-Number(a.ownBreakout) || a.platform.localeCompare(b.platform) || (b.likes ?? -1)-(a.likes ?? -1));
    const picked = high.slice(0,10);
    const labels = new Set(picked.flatMap(w => list(w.research?.output?.discussions).map(d => `${w.platform}|${d.label}`)));
    const contrasts = eligible.filter(w => !picked.some(p => p.workRef === w.workRef) && list(w.research?.output?.discussions).some(d => labels.has(`${w.platform}|${d.label}`)));
    if (!picked.length) return eligible.slice().sort((a,b) => String(b.publishedAt || '').localeCompare(String(a.publishedAt || ''))).slice(0,3);
    const result = [], chosen = new Set();
    const add = w => { if (result.length < 10 && !chosen.has(w.workRef)) { result.push(w); chosen.add(w.workRef); } };
    for (const sample of picked) {
      add(sample);
      if (!sample.own) {
        const discussions = new Set(list(sample.research?.output?.discussions).map(d => d.label));
        contrasts.filter(w => w.own && w.platform === sample.platform && list(w.research?.output?.discussions).some(d => discussions.has(d.label))).forEach(add);
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
    const ref = dialogState.topicRef, t = topic(ref), ns = dialogState.works || curated(ref), outputs = analysisOutputs(ns), angles = outputs.flatMap(({work,output}) => list(output.angles).map((a,angleIndex) => ({...a,workRef:work.workRef,researchResultRef:work.research?.resultRef,researchMethodVersion:work.research?.methodVersion,researchAngleIndex:angleIndex})));
    const mode = dialogState.tab || 'discussion', allOwn = ownTopicWorks(ref,dialogState.snapshot || snapshot);
    return `<div class="lgi-tm-object-head"><div><h3>${esc(t?.displayName || '当前方向')}</h3><p>${esc(t?.definitionText)}</p><p>本次精选 ${ns.length} 篇独立作品 · 最多10篇，不凑满。全部相关材料继续可看。<br>我方全部发布历史 / 外部${state.referenceWindowDays === '0' ? '全部历史' : '近'+state.referenceWindowDays+'天发布'}参考；此阅读范围不被概览统计窗口清空。</p></div>${btn('查看全部材料','materials')}</div>${dialogState.loading ? '<p role="status">正在补齐全历史我方作品与独立外部参考范围，已有材料先显示。</p>' : ''}${dialogState.readError ? `<p class="lgi-tm-read-error">完整范围读取未成功：${esc(dialogState.readError)}</p>` : ''}<div class="lgi-tm-work-tabs">${[['discussion','具体讨论'],['samples','精选与对照'],['own','我方原作与复用'],['angles','角度与标题'],['product','产品机会']].map(([id,name]) => tab(name,'judge-tab',id,mode === id)).join('')}</div>${mode === 'samples' ? `<p>按已读取材料选择参考，不把低互动当失败；同平台比较保留发布时间。</p><div class="lgi-tm-material-grid">${ns.map(card).join('')}</div>` : mode === 'own' ? `<div class="lgi-tm-callout">${snapshot.scope.ownIdentityState === 'unknown' ? '我方矩阵未设置，不能推断全部没做过。' : '这里只展示已确认我方作品。手动爆款可复用核心、换场景、沿用标题或重新组织，具体效果仍待验证。'}</div><p>本次精选中的我方作品 ${ns.filter(w => w.own).length} 篇；本主题已读取的我方发布原作 ${allOwn.length} 篇，全部原作不受精选10篇上限限制。</p>${btn('展开本主题全部已发布我方原作','judge-own-all','', '',dialogState.loading ? 'disabled' : '')}<div class="lgi-tm-material-grid">${ns.filter(w => w.own).map(card).join('') || empty('当前精选没有可比较的我方原作','可展开全部已确认我方发布原作继续核对，不推断从未做过。')}</div>` : mode === 'angles' ? `${angles.length ? angles.slice(0,6).map((a,i) => `<article class="lgi-tm-angle"><h4>${esc(a.label || a.angle)}</h4><h3>${esc(a.title)}</h3><p>回答任务：${esc(a.answerTask || a.rationale)}</p><p class="lgi-tm-note">来源：${esc(title(work(a.workRef) || {}))} · ${list(a.evidence).length} 个定位引用</p><div class="lgi-tm-row">${btn('核对原作','reader',a.workRef)}${btn('保留这个角度','save-angle',String(i),'lgi-tm-primary')}</div></article>`).join('') : empty('当前没有已研究、可引用的不同角度','已有作品可继续读；明确开启研究后才会产生模型角度。')}` : mode === 'product' ? productContent(outputs) : `${sceneList(ns)}<h3>讨论对照与可查来源</h3>${outputs.flatMap(({work,output}) => list(output.discussions).map(d => `<article class="lgi-tm-discussion"><h4>${esc(d.label || d.title)}</h4><p>${list(d.evidence).length} 条定位依据 · ${esc(title(work))}</p>${btn('回到原文与原声','reader',work.workRef,'lgi-tm-quiet')}</article>`)).join('') || empty('具体讲法尚无已保存的研究结论','先看原作与已有评论，不补造共同痛点或反例。')}<div class="lgi-tm-material-grid">${ns.slice(0,4).map(card).join('')}</div>`}<section class="lgi-tm-section"><div class="lgi-tm-section-head"><div><h3>已有结果继续可读，有界补充按需开启</h3><p>关闭弹窗不会取消本轮任务，重开复用同一轮；明确停止保留已取得材料。</p></div>${btn('研究这个方向','start-topic-research',ref,'', 'data-mutation')}</div><div class="lgi-tm-row">${btn('补充本主题材料','capture',ref)}${btn('查看研究进度','research')}${btn('查看完整材料','materials')}</div></section>`;
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
      return `<tr class="${t.topicRef === state.topicRef ? 'is-active' : ''}"><th><strong>${esc(t.displayName)}</strong><p>${number(j?.denominator)} 篇可读作品</p></th>${fallbackStages.map(s => {const e = entries.find(e => e.stage === s.stage); return `<td>${btn(`<b>${number(e?.count)}</b><small>${pct(e?.count,j?.denominator)}</small>`,'matrix-cell',`${t.topicRef}|${s.stage}`,'lgi-tm-matrix-cell')}</td>`;}).join('')}<td>${btn(`<b>${number(otherCount)}</b><small>${pct(otherCount,j?.denominator)}</small>`,'matrix-cell',`${t.topicRef}|other`,'lgi-tm-matrix-cell')}<details><summary>类别明细</summary>${otherEntries.map(e => `<p>${esc(otherNames[e.stage] || e.stage)} ${number(e.count)}</p>`).join('')}</details></td></tr>`;
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
    return `<p>${esc(scopeText())} · 各平台分别比较，统计引用同一服务端集合。</p><div class="lgi-tm-compare-grid">${topics.map(t => `<article class="lgi-tm-panel"><h3>${esc(t.displayName)}</h3><p>${esc(t.definitionText)}</p><dl class="lgi-tm-facts"><div><dt>去重作品</dt><dd>${number(t.statistics?.workCount)}</dd></div><div><dt>已观察作者</dt><dd>${number(t.statistics?.authorCount)}</dd></div>${platformStats(t.statistics).map(p => `<div><dt>${esc(platformName(p.platform))} 样本爆款</dt><dd>${p.highPerformanceThreshold == null ? '未设置规则' : `${number(p.highPerformanceCount)} / ${number(p.knownLikeCount)} · ${pct(p.highPerformanceCount,p.knownLikeCount)}`}</dd></div><div><dt>${esc(platformName(p.platform))} 点赞中位数 / P90</dt><dd>${number(p.medianLikes)} / ${number(p.p90Likes)}</dd></div>`).join('')}</dl><h4>主阶段分布</h4>${list(t.journey?.main).map(e => `<p>${esc(stageNames[e.stage] || otherNames[e.stage] || e.stage)} <span class="lgi-tm-num">${number(e.count)} · ${pct(e.count,t.journey.denominator)}</span></p>`).join('')}<h4>我方与外部参考</h4><p>已确认我方发布：${number(t.statistics?.ownHistoryPublishedCount)} · 手动爆款：${number(t.statistics?.ownHistoryBreakoutCount)}</p><p>独立外部近期低粉高赞 ${number(t.statistics?.recentLowFollowerHighLikeCount)} 篇</p>${btn('打开判断','judge',t.topicRef,'lgi-tm-quiet')}</article>`).join('')}</div>`;
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
    const a = dialogState.angle, product = dialogState.kind === 'product_research';
    if (dialogState.savedAlternativeRef) return `<div class="lgi-tm-callout">${product ? '产品研究说明' : '内容角度'}已获得后端保存回执。保存不启动监控、采集或业务实验。</div><div class="lgi-tm-row">${btn('取回刚保存的原版本','saved-materials',dialogState.savedAlternativeRef)}${btn('复核来源并导出','export-alternative',dialogState.savedAlternativeRef)}</div>`;
    return `<p>${product ? '可修改研究说明，再保留需求、支持假设、待验证问题与原依据。修改是人工说明，不改变模型的原研究结果。' : '保留本次角度与作品依据，不改写原判断版本。'}保存不自动开启监控、补采或业务实验。</p><form data-form="save" class="lgi-tm-form"><label><span>${product ? '研究说明标题' : '标题'}</span><input name="title" value="${esc(a.title)}" maxlength="120" required></label><label><span>${product ? '具体需求与支持假设' : '角度'}</span><textarea name="angle" maxlength="2000" required>${esc(a.label)}</textarea></label><label><span>${product ? '可编辑研究说明 / 最先验证什么' : '回答任务 / 保留理由'}</span><textarea name="rationale" maxlength="2000" required>${esc(a.answerTask)}</textarea></label><p>原依据：${esc(title(work(a.workRef) || {}))} · ${list(a.evidence).length} 个定位引用</p><button type="submit" class="lgi-tm-button lgi-tm-primary" data-mutation>${product ? '保存研究说明与引用' : '保存备选'}</button></form>`;
  }
  function opportunitiesFrom(outputs) {
    return outputs.flatMap(({work,output}) => list(output.productOpportunities).map((o,opportunityIndex) => ({...o,workRef:work.workRef,researchResultRef:work.research?.resultRef,researchMethodVersion:work.research?.methodVersion,researchOpportunityIndex:opportunityIndex})));
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
  function researchDialog() {
    if (dialogState.loading) return '<p role="status">正在读取研究设置与真实进度回执。</p>';
    const p = dialogState.progress || progress, policy = p?.policy, models = list(p?.models), usage = p?.usage;
    return `<div class="lgi-tm-callout">开启后，历史材料分批研究、新材料增量处理。只有明确保存开启配置才启动，不因普通开页调用模型。关闭弹窗不取消已发任务，明确停止阻止继续启动新请求。</div><div class="lgi-tm-metrics"><div><span>今日已记账</span><strong>${number(usage?.chargedTokens)}</strong><small>token · ${esc(usage?.timezone || 'Asia/Shanghai')}</small></div><div><span>当前预留</span><strong>${number(usage?.reservedTokens)}</strong><small>token · 不等于已消费</small></div><div><span>每日上限</span><strong>${number(policy?.dailyTokenLimit)}</strong><small>达到上限后延后待处理</small></div><div><span>每次研究上限</span><strong>${number(policy?.runTokenLimit)}</strong><small>token · 与精选10篇不同</small></div></div><form data-form="research-config" class="lgi-tm-form"><h3>研究设置</h3><label><span>已有模型配置</span><select name="modelConfigRef" required><option value="">请选择配置</option>${models.map(m => `<option value="${esc(m.configRef)}" ${policy?.modelConfigRef === m.configRef ? 'selected' : ''}>${esc(m.modelId)} · 输入 ${number(m.inputTokenLimit)} / 输出 ${number(m.outputTokenLimit)} token</option>`).join('')}</select></label><div class="lgi-tm-form-grid"><label><span>每日 token 上限</span><input type="number" name="dailyTokenLimit" min="1024" max="10000000" step="1" required value="${policy?.dailyTokenLimit ?? ''}" placeholder="明确预算后填写"></label><label><span>每次研究 token 上限</span><input type="number" name="runTokenLimit" min="1024" max="10000000" step="1" required value="${policy?.runTokenLimit ?? ''}" placeholder="明确预算后填写"></label></div><label class="lgi-tm-check-label"><input type="checkbox" name="automaticEnabled" ${policy?.automaticEnabled ? 'checked' : ''}><span>明确开启历史回填与新材料自动增量研究</span></label><label class="lgi-tm-check-label"><input type="checkbox" name="collectionEnabled" ${policy?.collectionEnabled ? 'checked' : ''}><span>在已授权范围内允许主题临时补采，每轮最多10篇详情</span></label><p>已有材料仍可用；研究配置不替代平台执行授权、工位范围与采集限制。</p><button type="submit" class="lgi-tm-button lgi-tm-primary" data-mutation>保存明确设置</button></form><section class="lgi-tm-section"><div class="lgi-tm-section-head"><h3>研究进度</h3>${btn('刷新进度','research-refresh')}</div>${list(p?.runs).map(r => `<article class="lgi-tm-run"><div><h4>${esc(labelState(r.state))} ${badge(({historical:'历史回填',incremental:'新材料增量',on_demand:'按需研究'})[r.trigger] || r.trigger)}</h4><p>待处理 ${number(r.queuedCount)} · 已获结果 ${number(r.succeededCount)} · 失败 / 派发未知 ${number(r.failedCount)}</p><p>${esc(r.lastReason || '暂无额外原因')} · ${esc(r.createdAt)}</p><small class="lgi-tm-num">${esc(r.runRef)}</small></div><div class="lgi-tm-row">${btn('暂停','research-pause',r.runRef,'', 'data-mutation')}${btn('恢复','research-resume',r.runRef,'', 'data-mutation')}${btn('明确停止','research-stop',r.runRef,'lgi-tm-danger','data-mutation')}</div></article>`).join('') || empty('当前没有研究运行','首次开启或明确按需启动后，真实回执会出现在这里。')}</section>`;
  }
  async function researchCommand(payload) {
    if (pending) return null;
    pending = true; dialogFeedback('正在提交明确动作…');
    try { const result = await request('/api/local/topic-map/research/commands',{method:'POST',body:JSON.stringify({...payload,requestRef:uuid(),domainRef:state.domainRef})}); dialogFeedback('动作已获得后端回执；排队、外发与结果接纳分别以进度为准。'); return result; }
    catch (error) { dialogFeedback(`动作未完成：${error.message}`); return null; }
    finally { pending = false; }
  }
  function allCitations(w) {
    const o=w.research?.output;if(!o)return [];
    return [...list(o.journey?.evidence),...['scenes','discussions','responseMatches','angles','productOpportunities'].flatMap(key=>list(o[key]).flatMap(v=>list(v.evidence)))];
  }
  function citationSpans(w,fragmentId) {return allCitations(w).filter(c=>c.fragmentId===fragmentId);}
  async function recordViewedWork(w) {
    const ref=dialogStack.slice().reverse().find(v=>v.type==='judge')?.topicRef || state.topicRef,t=topic(ref);
    if(!t || (!list(w.topicRefs).includes(ref)&&!list(t.workRefs).includes(w.workRef)))return;
    const current=list(snapshot.changes).find(c=>c.topicRef===ref),refs=[...new Set([...list(current?.lastViewedWorkRefs),w.workRef])].slice(-1000);
    try {await request('/api/local/topic-map/commands',{method:'POST',body:JSON.stringify({action:'viewed',idempotencyKey:uuid(),domainRef:state.domainRef,topicRef:ref,definitionRef:t.definitionRef,workRefs:refs})});if(current){current.lastViewedWorkRefs=refs;current.unviewedWorkRefs=list(current.unviewedWorkRefs).filter(id=>id!==w.workRef);}} catch (_) { /* Read availability remains independent from the optional view receipt. */ }
  }
  function bodyCitations(w) {
    const output = w.research?.output;
    if (!output) return [];
    const citations = [...list(output.journey?.evidence),...list(output.scenes).flatMap(s => list(s.evidence)),...list(output.discussions).flatMap(s => list(s.evidence))];
    return citations.filter(c => c.fragmentId.startsWith(`${w.workRef}.body.`));
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
      const a = dialogState.angle, t = topic(dialogState.topicRef);
      if (!t) return dialogFeedback('原主题定义尚未读取，当前不能保存。');
      const result = await command({action:'saveAlternative',topicRef:t.topicRef,definitionRef:t.definitionRef,title:String(data.get('title')).trim(),angle:String(data.get('angle')).trim(),rationale:String(data.get('rationale')).trim(),evidenceWorkRefs:[a.workRef],methodVersion:a.researchMethodVersion || snapshot.methodVersion,researchResultRef:a.researchResultRef || null,researchAngleIndex:a.researchResultRef && dialogState.kind!=='product_research' ? a.researchAngleIndex : null,researchOpportunityIndex:a.researchResultRef && dialogState.kind==='product_research' ? a.researchOpportunityIndex : null},'备选已保存到后端，不启用跟踪');
      if (result) { dialogState.savedAlternativeRef=result.subjectRef;drawDialog();dialogFeedback(`已保存 · 回执 ${result.receiptRef}`); } else dialogFeedback(notice); return;
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
    if (action === 'comments-more') { const current=dialogState; try { const next=await request(`/api/local/work-resources/${encodeURIComponent(current.work.workRef)}/comments?domain=${encodeURIComponent(state.domainRef)}&cursor=${encodeURIComponent(current.commentResource.nextCursor)}`); if(dialogState===current){current.commentResource={...next,items:[...current.commentResource.items,...next.items]};drawDialog();} }catch(error){dialogFeedback(`已有评论读取未成功：${error.message}`);}return; }
    if (action === 'judge') { openDialog('judge',{topicRef:id,works:curated(id),tab:'discussion',loading:true}); const current=dialogState; try { const full=await request(`/api/local/topic-map?${query({topicRef:id,windowDays:'',platform:''})}`); full.works.forEach(w=>extraWorks.set(w.workRef,w)); if(dialogState===current){current.works=curated(id,full);current.snapshot=full;current.loading=false;drawDialog();} } catch(error){if(dialogState===current){current.loading=false;current.readError=error.message;drawDialog();}} return; }
    if (action === 'judge-tab') { dialogState.tab = id; drawDialog(); return; }
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
      const outputs = analysisOutputs(dialogState.works), angles = outputs.flatMap(({work,output}) => list(output.angles).map((a,angleIndex) => ({...a,workRef:work.workRef,researchResultRef:work.research?.resultRef,researchMethodVersion:work.research?.methodVersion,researchAngleIndex:angleIndex})));
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
      if (result && dialogState === current) { current.work = work(id); drawDialog(); dialogFeedback('手动标记已获得后端回执。未标记不等于表现低。'); } else dialogFeedback(notice); return;
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
