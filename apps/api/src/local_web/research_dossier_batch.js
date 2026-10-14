/* A fixed, resumable client batch over the same single-work command. No background scheduler. */
(() => {
  'use strict';
  const key = 'linggan-research-dossier-batch-v1';
  let batch = null, dialog = null, running = false, paused = false;
  const el = (tag, text) => { const n = document.createElement(tag); if (text != null) n.textContent = text; return n; };
  function button(text, fn) { const n = el('button',text); n.type='button'; n.className='ev-button'; n.addEventListener('click',fn); return n; }
  function save() { try { sessionStorage.setItem(key,JSON.stringify(batch)); } catch { /* The in-memory scope remains usable. */ } window.dispatchEvent(new Event('dossier-batch-changed')); }
  function restore() {
    try { const value=JSON.parse(sessionStorage.getItem(key)); if (Array.isArray(value?.items) && value.items.length<=200) batch=value; } catch { batch=null; }
  }
  function paint() {
    if (!dialog || !batch) return;
    const done=batch.items.filter(i=>i.state==='done').length;
    const failed=batch.items.filter(i=>i.state==='failed').length;
    const summary=el('p',`${batch.items.length} 篇固定作品 · 已处理 ${done} 篇 · 失败 ${failed} 篇 · 尚未处理 ${batch.items.length-done-failed} 篇`);
    summary.setAttribute('role','status');
    const list=el('ul');
    for (const item of batch.items) {
      let text=`${item.work.slice(0,8)} · `;
      if (item.state==='done') text+=`更新 ${item.result.updated} 个布局，已是当前版本 ${item.result.alreadyCurrent} 个，跳过 ${item.result.skipped} 个`;
      else if (item.state==='failed') text+='本篇处理失败，可重新预览后重试';
      else if (item.plan) text+=`${item.plan.pendingLayouts} 个待分层，${item.plan.alreadyCurrent} 个已是当前版本，${item.plan.needsOcr} 个需先识字`;
      else text+=running?'正在读取处理范围…':'范围尚未读取';
      list.append(el('li',text));
    }
    const actions=el('div'); actions.className='rd-toolbar';
    const preview=button('预览 / 刷新范围',()=>previewBatch()); preview.disabled=running;
    const start=button('开始 / 继续整理',()=>execute()); start.disabled=running || !batch.items.some(i=>i.state!=='done' && i.plan);
    const stop=button('处理完当前篇后暂停',()=>{paused=true;}); stop.disabled=!running;
    const close=button('关闭',()=>{paused=true;dialog.close();});
    actions.append(preview,start,stop,close);
    dialog.replaceChildren(el('h2','重新整理选定作品的图片文字'),summary,
      el('p','仅使用已经取得的 OCR 行。缺少识别结果的图片不会自动识字或补采。关闭或暂停后，已完成结果保留；刷新页面后可继续未完成范围。'),list,actions);
  }
  async function request(item,method='GET') {
    const base=`/api/local/work-resources/${encodeURIComponent(item.work)}/research-dossier`;
    const response=await fetch(method==='GET'?`${base}/relayer-plan?domain=${encodeURIComponent(batch.domain)}`:`${base}/relayer`,
      method==='GET'?{cache:'no-store'}:{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({domain:batch.domain,expectedRevision:item.plan.expectedRevision})});
    if (!response.ok) throw new Error('request_failed');
    return response.json();
  }
  async function previewBatch() {
    if (running) return; running=true;paused=false;paint();
    try { for (const item of batch.items) {
      if (paused) break; if (item.state==='done') continue;
      try { const payload=await request(item); item.plan=payload.relayerPlan;item.state='ready'; }
      catch { item.state='failed';item.plan=null; }
      save();paint();
    }} finally {running=false;save();paint();}
  }
  async function execute() {
    if (running) return; running=true;paused=false;paint();
    try { for (const item of batch.items) {
      if (paused) break; if (item.state==='done'||!item.plan) continue;
      try { item.result=await request(item,'POST');item.state='done'; }
      catch { item.state='failed';item.plan=null; }
      save();paint();
    }} finally {running=false;save();paint();}
  }
  window.LingganDossierBatch={
    open(works,domain,resume=false) {
      if (running) {dialog?.showModal();return;}
      if (resume) restore();
      if (!resume || !batch) batch={domain,items:[...new Set(works)].map(work=>({work,state:'pending'}))};
      if (!batch?.items?.length || batch.items.length>200) return;
      if (!dialog) {dialog=el('dialog');dialog.className='rd-batch';dialog.addEventListener('cancel',()=>{paused=true;});document.body.append(dialog);}
      save();paint();dialog.showModal();
    },
    hasPending() { if (!batch) restore();return batch?.items?.some(i=>i.state!=='done') || false; },
  };
})();
