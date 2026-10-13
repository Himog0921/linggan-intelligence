/* COLLECTION-JTBD-001: real commands, native modal focus, retained request identity. */
(() => {
  'use strict';
  const root = document.querySelector('[data-collection-runs]');
  if (!root) return;
  const modal = document.getElementById('collection-run-form');
  const drawer = document.getElementById('collection-run-drawer');
  const form = modal.querySelector('form');
  const feedback = form.querySelector('[data-run-form-feedback]');
  const originalUrl = new URL(location.href);
  const draftKey = `collection-run-draft:${originalUrl.searchParams.get('domain') || 'all'}`;
  let exactWorks=null;
  if (originalUrl.searchParams.get('selected')==='1') {
    try {exactWorks=JSON.parse(sessionStorage.getItem('collection-selected:'+(originalUrl.searchParams.get('domain')||''))||'[]');}catch(_){exactWorks=[];}
  }
  let targetPage = 1, selecting = 0, requestKey = null, requestIntent = null, currentRun = null;
  const known=value=>value==null?'未知':escape(value);
  const escape = value => String(value ?? '').replace(/[&<>"']/g, c => ({ '&':'&amp;', '<':'&lt;', '>':'&gt;', '"':'&quot;', "'":'&#39;' }[c]));
  function dialogOpen(dialog) { dialog.showModal(); document.documentElement.classList.add('c-run-dialog-open'); }
  function dialogClose(dialog) { dialog.close(); }
  function returnContext() {
    const candidate=originalUrl.searchParams.get('returnTo');
    if (!candidate) return null;
    try { const url=new URL(candidate,location.origin); return url.origin===location.origin && (url.pathname==='/collection/targets' || url.pathname==='/corpus/evidence') ? url : null; } catch (_) {return null;}
  }
  modal.addEventListener('close',()=>{
    const destination=returnContext();if(destination){location.assign(destination.href);return;}
    const url=new URL(location.href);for(const key of ['once','selected','target']) url.searchParams.delete(key);history.replaceState(null,'',url);
  });
  drawer.addEventListener('close',()=>{const url=new URL(location.href);url.searchParams.delete('run');history.replaceState(null,'',url);});
  for (const dialog of [modal, drawer]) dialog.addEventListener('close', () => {
    if (!modal.open && !drawer.open) document.documentElement.classList.remove('c-run-dialog-open');
  });
  document.querySelectorAll('[data-close-run-dialog]').forEach(b => b.addEventListener('click', () => dialogClose(b.closest('dialog'))));
  async function json(url, options) {
    const response = await fetch(url, options);
    const payload = await response.text();
    let data;try {data=JSON.parse(payload);} catch(_){throw new Error(`服务器未返回可核对的回执（${response.status}）`);}
    if (!response.ok) {
      const messages = {
        domain_not_active:'本次领域尚未运行，请先恢复领域',
        target_outside_domain_or_stopped:'观察目标已停用或不属于本次领域，请重新选择',
        works_outside_domain:'所选作品不属于本次领域，请回到语料重新选择',
        invalid_work_limit:'作品上限须为 1–200 篇',
        invalid_keyword:'请填写 1–80 字的临时关键词',
        invalid_work_set:'所选作品集合无效，请回到语料重新选择',
        unsupported_ranking:'当前排序不可用，请重新选择',
        ranking_not_applicable:'该来源不使用搜索排序',
        collection_run_conflict:'任务状态或请求内容已改变，请重新读取后操作',
        collection_run_not_found:'当前领域下找不到这次采集',
        run_ended:'本轮已结束，成果保留',
        run_stopped:'本次采集已停止，成果保留'
      };
      throw new Error(messages[data.code] || (response.status >= 500 ? '服务暂时不可用，请稍后重试' : `请求未被接受（${response.status}），请重新核对当前范围`));
    }
    return data;
  }
  const field = name => form.elements.namedItem(name);
  function saveDraft() {
    try { sessionStorage.setItem(draftKey, JSON.stringify({ data: Object.fromEntries(new FormData(form)), requestKey, requestIntent })); } catch (_) {}
  }
  try {
    const draft = JSON.parse(sessionStorage.getItem(draftKey) || 'null');
    if (draft) { for (const [name, value] of Object.entries(draft.data)) if (field(name)) field(name).value = value; requestKey = draft.requestKey; requestIntent = draft.requestIntent; }
  } catch (_) {}
  function sourceFields() {
    if (exactWorks!==null) {
      form.querySelector('fieldset').hidden=true;
      form.querySelector('[data-keyword-field]').hidden=true;field('query').required=false;
      form.querySelector('[data-target-field]').hidden=true;field('target').required=false;
      form.querySelector('[data-ranking-field]').hidden=true;
      field('work_limit').value=exactWorks.length;field('work_limit').readOnly=true;
      form.querySelector('[data-run-work-label]').textContent='本次作品数量';
      form.querySelector('[data-run-work-help]').textContent='沿用已勾选的作品，不重新搜索或补换作品。';
      if (!form.querySelector('[data-exact-work-scope]')) {const scope=document.createElement('p');scope.dataset.exactWorkScope='';scope.textContent='已选择 '+exactWorks.length+' 篇作品；沿用精确 ID，不重新搜索。';form.querySelector('.c-run-dialog-body').prepend(scope);}
      return;
    }
    const existing = field('source').value === 'observation_target';
    form.querySelector('[data-keyword-field]').hidden = existing;
    field('query').required = !existing;
    form.querySelector('[data-target-field]').hidden = !existing;
    field('target').required = existing;
    if (existing) loadTargets(false);
    rankingVisibility();
  }
  function rankingVisibility() {
    const creator = field('source').value === 'observation_target' && field('target').selectedOptions[0]?.dataset.kind === 'creator';
    form.querySelector('[data-ranking-field]').hidden = creator;
  }
  async function loadTargets(more) {
    const domain = field('domain').value;
    if (!domain) { field('target').replaceChildren(new Option('请先选择本次领域', '')); return; }
    const generation = ++selecting;
    targetPage = more ? targetPage + 1 : 1;
    const selected = field('target').value || originalUrl.searchParams.get('target') || '';
    try {
      const params = new URLSearchParams({ domainRef: domain, q: field('target_search').value, page: targetPage, selected });
      const result = await json(`/api/collection/runs/targets?${params}`);
      if (generation !== selecting) return;
      if (!more) field('target').replaceChildren(new Option('请选择观察目标', ''));
      for (const t of result.items) {
        if ([...field('target').options].some(o => o.value === t.target_ref)) continue;
        const option = new Option(`${t.name} · ${t.target_kind === 'creator' ? '创作者' : '关键词'}`, t.target_ref);
        option.dataset.kind = t.target_kind;
        field('target').add(option);
      }
      field('target').value = selected;
      form.querySelector('[data-more-targets]').disabled = targetPage * 20 >= result.total;
      rankingVisibility();
    } catch (error) { feedback.textContent = `目标读取失败：${error.message}。草稿已保留。`; }
  }
  document.querySelector('[data-new-run]').addEventListener('click', () => dialogOpen(modal));
  form.addEventListener('change', event => {
    requestKey = null;
    if (event.target.name === 'source') sourceFields();
    if (event.target.name === 'domain' && field('source').value === 'observation_target') loadTargets(false);
    if (event.target.name === 'target') rankingVisibility();
    saveDraft();
  });
  form.addEventListener('input', () => { requestKey = null; saveDraft(); });
  let searchTimer;
  field('target_search').addEventListener('input', () => { clearTimeout(searchTimer); searchTimer = setTimeout(() => loadTargets(false), 250); });
  form.querySelector('[data-more-targets]').addEventListener('click', () => loadTargets(true));
  sourceFields();
  if (exactWorks!==null) field('comment_mode').value='all_public';
  if (originalUrl.searchParams.get('once') === '1' || exactWorks!==null) {
    if (originalUrl.searchParams.get('target')) field('source').value = 'observation_target';
    sourceFields(); dialogOpen(modal);
  }
  form.addEventListener('submit', async event => {
    event.preventDefault();
    const submit = form.querySelector('[type=submit]');
    if (submit.disabled || !form.reportValidity()) return;
    const existing = field('source').value === 'observation_target';
    const creator = existing && field('target').selectedOptions[0]?.dataset.kind === 'creator';
    const intent = { domainRef: field('domain').value, source: exactWorks!==null ? {kind:'selected_works',workRefs:exactWorks} : existing ? { kind: 'observation_target', targetRef: field('target').value } : { kind: 'temporary_keyword', query: field('query').value }, ranking: creator || exactWorks!==null ? null : field('ranking').value, workLimit: Number(field('work_limit').value), commentMode: field('comment_mode').value };
    const fingerprint = JSON.stringify(intent);
    if (requestIntent !== fingerprint) { requestKey = null; requestIntent = fingerprint; }
    requestKey ||= crypto.randomUUID(); saveDraft();
    const body = { idempotencyKey: requestKey, ...intent };
    submit.disabled = true; feedback.textContent = '正在建立这次请求…';
    try {
      let receipt;
      try { receipt = await json(`/api/collection/runs/requests/${requestKey}`); } catch (_) {}
      receipt ||= await json('/api/collection/runs', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) });
      sessionStorage.removeItem(draftKey);
      const destination = new URL('/collection/tasks', location.origin);
      destination.searchParams.set('domain', body.domainRef); destination.searchParams.set('run', receipt.runRef);
      location.assign(destination.href);
    } catch (error) { feedback.textContent = `请求未确认：${error.message}。草稿及申请身份已保留，重试先核对原回执。`; }
    finally { submit.disabled = false; }
  });
  async function openRun(id, domain) {
    const body = drawer.querySelector('[data-run-detail]');
    body.textContent = '正在读取持久进展…';
    if (!drawer.open) dialogOpen(drawer);
    try {
      currentRun = await json(`/api/collection/runs/${id}?domainRef=${encodeURIComponent(domain)}`);
      const r = currentRun;
      document.getElementById('collection-run-title').textContent = r.name;
      document.getElementById('collection-run-title').title = r.name;
      const reason=({surface_ended:'本轮可见作品已发现完毕；保留实际数量',maximum_quota:'已达到本次发现上限',accepted_discovery_scope:'已按本次发现结果固定作品',person_selected_exact_set:'按勾选的作品固定集合',discovery_execution_stopped:'发现未完成，需要处理'})[r.discovery_reason] || (r.discovery_reason ? '发现原因见执行诊断' : '发现尚未结束');
      const status=r.control_intent==='stopped'?'已停止，成果保留':r.control_intent==='paused'?'人工暂停，交付保留':r.ended?(r.needs_attention?'本轮结束，有缺口':'本轮已结束'):r.phase==='blocked'?'需要处理，已有材料保留':r.domain_status==='paused'?'领域暂停，任务保留':r.live_leases>0?'执行中，已有成果可查看':'等待派发';
      body.innerHTML = `<section data-run-progress><p>本次领域：${escape(r.domain_name)} · ${r.legacy ? '按原工单范围' : r.comment_mode === 'all_public' ? '尽量采全，含回复' : '每篇最多 30 条总样本'}</p><h3>这次采集，已取得什么</h3><div class="c-run-readouts"><div><span>固定作品</span><b>${r.scope_frozen_at || r.legacy ? known(r.selected_works) : '待确定'}</b><small>本次上限 ${known(r.work_limit)} 篇</small></div><div><span>可用详情</span><b>${known(r.available_details)}</b><small>其中明确复用 ${known(r.reused_details)} 篇</small></div><div><span>本轮可检索评论</span><b>${known(r.delivered_comments)}</b><small>本次新增 ${known(r.new_comments)} 条</small></div></div><p class="c-run-status-note"><b>${escape(status)}</b><br>${escape(reason)}</p><dl><dt>评论交付</dt><dd>本轮采回 ${known(r.received_comments)} · 可检索 ${known(r.delivered_comments)} · 新增 ${known(r.new_comments)} · 共享累计 ${known(r.cumulative_comments)}</dd><dt>资源进度</dt><dd>${r.legacy ? '按原执行记录核对' : r.domain_status === 'paused' ? '领域暂停，恢复领域后继续' : '待派发 ' + known(r.queued_orders) + ' 单 · 有效租约 ' + known(r.live_leases)}</dd></dl><h3>执行进展</h3><ol class="c-run-phases"><li><span>发现作品</span><span>${r.scope_frozen_at ? known(r.selected_works)+' 篇已固定' : r.legacy ? '按原执行范围' : '等待发现回执'}</span></li><li><span>取得详情与作者</span><span>${r.scope_frozen_at || r.legacy ? known(r.available_details)+' / '+known(r.selected_works)+' 篇' : '已取得 '+known(r.available_details)+' 篇；范围待确定'}</span></li><li><span>${r.comment_mode === 'all_public' ? '评论与回复遍历' : '评论样本交付'}</span><span>${r.scope_frozen_at || r.legacy ? known(r.completed_discussions)+' / '+known(r.selected_works)+' 篇范围已结束' : '已交付 '+known(r.completed_discussions)+' 篇；范围待确定'}</span></li><li><span>结束本轮</span><span>${r.ended?'本轮成果与缺口保留':'材料取得后即可查看'} · 缺口 ${known(r.gap_works)} 篇</span></li></ol><p>${(r.stations||[]).map(st=>`<a href="/collection/runtime?station=${encodeURIComponent(st.stationRef)}&return_to=${encodeURIComponent(`/collection/tasks?domain=${r.domain_ref}&run=${r.run_ref}`)}">工位：${escape(st.name)}</a>`).join(' · ')}</p><p>交付、可用成果与遍历完成分别核对；等待工位不表示采集完成。</p><div class="c-run-controls">${!r.legacy && !r.ended && r.control_intent !== 'stopped' ? `<button type="button" class="c-btn-secondary" data-run-command="${r.control_intent === 'paused' ? 'continue' : 'pause'}">${r.control_intent === 'paused' ? '继续' : '暂停'}</button><button type="button" class="c-btn-quiet" data-run-command="stop">停止本次</button>` : ''}</div><p role="alert" data-run-command-feedback></p><details data-run-execution-diagnostics><summary>执行记录与诊断</summary><p>任务编号 ${escape(r.run_ref)} · 版本 ${known(r.revision)} · 发现原因 ${escape(r.discovery_reason || '尚未取得')}</p></details></section>`;
      if (r.name.length > 40) {
        const completeName = document.createElement('details');completeName.className = 'c-run-complete-name';
        const summary = document.createElement('summary');summary.textContent = '查看完整任务名称';
        const name = document.createElement('p');name.textContent = r.name;completeName.append(summary,name);
        body.querySelector('[data-run-progress] > p').after(completeName);
      }
      drawer.querySelector('[data-run-footer-state]').textContent=status;

      const diagnostic=body.querySelector('[data-run-execution-diagnostics]');diagnostic.addEventListener('toggle',()=>{if(diagnostic.open&&!diagnostic.dataset.loaded){diagnostic.dataset.loaded='1';const panel=document.createElement('section');diagnostic.append(panel);diagnosticPane(panel,id,domain);}});
      const results=document.createElement('section');results.className='c-run-results';results.hidden=true;results.innerHTML=`<h3>本次作品与缺口</h3><p><a href="/corpus/evidence?domain=${encodeURIComponent(domain)}&collectionRunRef=${encodeURIComponent(id)}&returnTo=${encodeURIComponent(`/collection/tasks?domain=${domain}&run=${id}`)}">查看本次作品与评论</a></p><div data-run-works>正在读取…</div>`;body.append(results);setRunView('progress');
      let workPage=1;
      async function loadWorks() {
        try {
          const result=await json(`/api/collection/runs/${id}/works?domainRef=${encodeURIComponent(domain)}&page=${workPage}`);
          results.querySelector('[data-run-works]').innerHTML=result.items.map(w=>`<p><b>${escape(w.ordinal ?? '顺位未知')} · ${escape(w.title || '标题未知')}</b><br><span>${escape(w.creator || '作者未知')} · ${escape(({access_restricted:'访问受限，已保留合格成果',detail_not_accepted:'详情尚未被接纳',discussion_partial_or_not_delivered:'评论或回复交付有缺口',execution_stopped_or_unavailable:'执行已停止或不可用',no_progress_after_bounded_retries:'多段未取得新评论，需要处理'})[w.gapReason] || w.gapReason || (w.checkpoint?.commentsComplete ? '本轮范围已交付' : '等待执行与交付'))}</span><br><a href="/corpus/evidence?domain=${encodeURIComponent(domain)}&collectionRunRef=${encodeURIComponent(id)}&work=${encodeURIComponent(w.publicRef)}&returnTo=${encodeURIComponent(`/collection/tasks?domain=${domain}&run=${id}`)}">查看作品／评论</a></p>`).join('') || '<p>作品集合尚未形成。</p>';
          for (const [label,next,disabled] of [['上一页作品',workPage-1,workPage<=1],['下一页作品',workPage+1,result.total<=workPage*20]]) {const button=document.createElement('button');button.type='button';button.className='c-btn-quiet';button.textContent=label;button.disabled=disabled;button.addEventListener('click',()=>{workPage=next;loadWorks();});results.querySelector('[data-run-works]').append(button);}
        } catch(error) {results.querySelector('[data-run-works]').textContent='作品范围读取失败：'+error.message;}
      }
      await loadWorks();
      const url = new URL(location.href); url.searchParams.set('run', id); history.replaceState(null, '', url);
    } catch (error) { body.textContent = `进展读取失败：${error.message}。当前不能据此确认成果或完成。`; }
  }
  document.querySelectorAll('[data-open-run]').forEach(b => b.addEventListener('click', () => openRun(b.dataset.openRun, b.dataset.runDomain)));
  function setRunView(view) {
    drawer.querySelector('[data-run-progress]')?.toggleAttribute('hidden',view!=='progress');
    drawer.querySelector('.c-run-results')?.toggleAttribute('hidden',view!=='results');
    drawer.querySelectorAll('.c-run-tabs [data-run-view]').forEach(b=>b.setAttribute('aria-current',b.dataset.runView===view?'page':'false'));
  }
  drawer.addEventListener('click', async event => {
    const view=event.target.closest('[data-run-view]');if(view){setRunView(view.dataset.runView);return;}
    if(event.target.closest('[data-expand-run]')){drawer.classList.toggle('c-run-expanded');return;}
    const button = event.target.closest('[data-run-command]');
    if (!button || button.disabled) return;
    const r = currentRun, notice = drawer.querySelector('[data-run-command-feedback]');
    button.disabled = true;
    try {
      await json(`/api/collection/runs/${r.run_ref}/commands`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ commandRef: crypto.randomUUID(), domainRef: r.domain_ref, expectedRevision: r.revision, action: button.dataset.runCommand }) });
      await openRun(r.run_ref, r.domain_ref);
    } catch (error) { notice.textContent = `控制命令未确认：${error.message}`; button.disabled = false; }
  });
  for (const time of document.querySelectorAll('time[datetime]')) {
    const date = new Date(time.dateTime);
    if (!Number.isNaN(date.valueOf())) { time.title = time.dateTime; time.textContent = new Intl.DateTimeFormat('zh-CN', {timeZone:'Asia/Shanghai',month:'2-digit',day:'2-digit',hour:'2-digit',minute:'2-digit',hour12:false}).format(date); }
  }

  function diagnosticPane(container,run=null,domain=null) {
    let page=1;
    async function load() {
      try {
        const params=new URLSearchParams({page});if(domain)params.set('domainRef',domain);if(run)params.set('runRef',run);
        const result=await json('/api/collection/executions?'+params);
        container.innerHTML=`<p>完整执行记录：共 ${result.total} 条 · 第 ${page} 页</p>`+result.items.map(t=>`<details><summary>${escape(t.capabilities?.join(' / '))} · ${escape(t.stageState||'未认领')} · ${escape(t.taskId)}</summary><dl><dt>冻结范围与资源</dt><dd><pre>${escape(JSON.stringify({work:t.workOrderRef,target:t.target,scope:t.frozenScope,lease:t.lease},null,2))}</pre></dd><dt>Attempt / Package / Receipt</dt><dd><pre>${escape(JSON.stringify(t.attempts,null,2))}</pre></dd><dt>交付对账</dt><dd><pre>${escape(JSON.stringify(t.delivery,null,2))}</pre></dd></dl></details>`).join('');
        for(const [label,next,disabled] of [['上一页',page-1,page<=1],['下一页',page+1,page*25>=result.total]]) {const b=document.createElement('button');b.type='button';b.className='c-btn-quiet';b.textContent=label;b.disabled=disabled;b.onclick=()=>{page=next;load();};container.append(b);}
      } catch(error){container.textContent='诊断记录读取失败：'+error.message;}
    }
    load();
  }
  const historyPane=document.querySelector('.c-run-diagnostics');
  historyPane?.addEventListener('toggle',()=>{
    if(historyPane.open&&!historyPane.querySelector('[data-full-executions]')){const panel=document.createElement('section');panel.dataset.fullExecutions='';historyPane.prepend(panel);diagnosticPane(panel,null,originalUrl.searchParams.get('domain')==='all'?null:originalUrl.searchParams.get('domain'));}
  });
  if (originalUrl.searchParams.get('run') && originalUrl.searchParams.get('domain')) openRun(originalUrl.searchParams.get('run'), originalUrl.searchParams.get('domain'));
})();
