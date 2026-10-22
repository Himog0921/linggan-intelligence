/** D3: built React + actual Axum + disposable PostgreSQL. No provider or shared database. */
import { chromium } from 'playwright';
import assert from 'node:assert/strict';
import { parseArgs } from 'node:util';
import fs from 'node:fs/promises';
import path from 'node:path';
const {values:v}=parseArgs({options:Object.fromEntries(['api-base-url','domain-ref','work-ref','existing-policy-ref','initial-active-policy-ref','proof-token','e2e-run-ref','e2e-problem-ref'].map(k=>[k,{type:'string'}]))});
const origin=v['api-base-url'],domain=v['domain-ref'];
const base=new URL(origin);
assert.equal(base.protocol,'http:');assert.ok(['127.0.0.1','localhost','[::1]'].includes(base.hostname));assert.ok(base.port && base.port!=='3000');
assert.ok(domain && v['proof-token']);
const mode=v['e2e-run-ref']?'read':'commands',root=path.join(process.env.WEB_APP_PROOF_DIR||process.env.TMPDIR||'/tmp','comments-'+mode);
await fs.mkdir(root,{recursive:true});
const browser=await chromium.launch({headless:true});
const context=await browser.newContext({viewport:{width:1440,height:1000},reducedMotion:'reduce'});
const page=await context.newPage();page.setDefaultTimeout(30000);
const report={mode,steps:[],errors:[],writes:[],responses:[],blocked:[],cancelled:[]};
const pending=[];
page.on('pageerror',e=>report.errors.push(e.message));
page.on('request',r=>{if(r.method()!=='GET'&&r.url().includes('/api/'))report.writes.push({path:new URL(r.url()).pathname,body:r.postDataJSON()});});
page.on('response',r=>{if(r.url().includes('/api/local/comment-study/')){const job=r.json().then(body=>report.responses.push({path:new URL(r.url()).pathname,method:r.request().method(),status:r.status(),body})).catch(()=>{});pending.push(job);}});
page.on('requestfailed',r=>{if(r.failure()?.errorText.includes('ERR_ABORTED'))report.cancelled.push(new URL(r.url()).pathname);});
const networkGuard=route=>{const r=route.request();if(new URL(r.url()).origin!==base.origin||(mode==='read'&&!['GET','HEAD'].includes(r.method()))){report.blocked.push({method:r.method(),path:new URL(r.url()).pathname});return route.abort('blockedbyclient');}return route.continue();};
await context.route('**/*',networkGuard);
async function step(name,fn){try{await fn();await page.screenshot({path:path.join(root,String(report.steps.length+1).padStart(2,'0')+'.png'),fullPage:true});report.steps.push({name,status:'PASS'});}catch(e){await page.screenshot({path:path.join(root,'failure.png'),fullPage:true}).catch(()=>{});report.steps.push({name,status:'FAIL',message:e.message});throw e;}}
const button=name=>page.getByRole('button',{name,exact:true});
try {
 const proof=await page.request.get(origin+'/__comment-study-browser-proof');assert.equal(proof.status(),200);assert.equal(await proof.text(),v['proof-token']);
 if(mode==='commands'){
  await step('Comment directory preserves filtering without retired AI controls',async()=>{
   await page.goto(`${origin}/corpus/comments?domain=${domain}&view=comments`);
   await page.locator('#comment-filter-form').waitFor();
   assert.equal(await button('发起研究').count(),0);
   assert.equal(await page.getByRole('link',{name:'去 AI 设置'}).count(),0);
   assert.equal(await page.getByText('查看历史方法版本',{exact:true}).count(),0);
  });
  await step('Old start, resume, recovery and method writes are actually rejected without a model',async()=>{
   for(const pathname of ['/api/local/comment-study/runs','/api/local/comment-study/policies',
    `/api/local/comment-study/policies/${v['existing-policy-ref']}/activate`,
    '/api/local/comment-study/runs/00000000-0000-4000-8000-000000000001/resume',
    '/api/local/comment-study/runs/00000000-0000-4000-8000-000000000001/recover']){
    const response=await page.request.post(origin+pathname,{data:{}});
    assert.equal(response.status(),410);assert.equal((await response.json()).error,'ai_legacy_execution_retired');
   }
  });
  await step('Human post usage retains request identity after a lost receipt and can be undone',async()=>{
   let dropMark=true;
   await page.route('**/api/local/comment-study/comments/post-usage',async route=>{
    if(dropMark){dropMark=false;const reply=await route.fetch();assert.equal(reply.status(),200);await route.abort('failed');}else await route.continue();
   });
   await page.locator('[data-post-usage="mark"]').first().click();await page.getByText(/保存未获确认/).waitFor();
   assert.equal(await page.locator('[data-post-usage="unmark"]').count(),0);
   await page.locator('[data-post-usage="mark"]').first().click();await page.locator('[data-post-usage="unmark"]').first().waitFor();
   const writes=report.writes.filter(item=>item.path.endsWith('/comments/post-usage'));
   assert.equal(writes.length,2);assert.equal(writes[0].body.requestRef,writes[1].body.requestRef);
   await page.locator('[data-post-usage="unmark"]').first().click();await page.locator('[data-post-usage="unmark"]').waitFor({state:'hidden'});
  });
  await step('Cross-view comment selection and all four historical views never create old research',async()=>{
   await button('选择评论').click();
   await page.locator('[data-select-comment][data-eligible="true"]').first().waitFor();
   await page.locator('[data-select-comment][data-eligible="true"]').first().check();
   for(const name of ['概览','研究批次','用户问题','用户评论']){await button(name).click();assert.equal(await button('发起研究').count(),0);}
   await page.locator('[data-select-comment][data-eligible="true"]').first().waitFor();
   assert.equal(await page.locator('[data-select-comment][data-eligible="true"]').first().isChecked(),true);
   assert.ok(report.writes.every(write=>write.path.endsWith('/comments/post-usage')));
   await button('取消选择').click();
   assert.equal(await page.locator('[data-select-comment]').count(),0);
  });
 }else{
  const run=v['e2e-run-ref'],problem=v['e2e-problem-ref'];
  await step('Overview, runs and problems do not expose proactive research',async()=>{for(const view of ['overview','runs','problems']){await page.goto(`${origin}/corpus/comments?${new URLSearchParams({domain,view})}`);await page.getByRole('button',{name:'发起研究',exact:true}).waitFor({state:'hidden'});}await page.goto(`${origin}/corpus/comments?${new URLSearchParams({domain,view:'comments'})}`);assert.equal(await button('发起研究').count(),0);});
  const response=await page.request.get(`${origin}/api/local/comment-study/targets?${new URLSearchParams({domain,runRef:run,limit:'100'})}`);assert.equal(response.status(),200);const targets=await response.json(),target=targets.targets.find(row=>row.commentKey?.commentExternalId&&row.signalCount>0);assert.ok(target);const key=target.commentKey;
  await step('Comment deep link shows actual parent context and history without empty sections',async()=>{
   const detailResponse=await page.request.get(`${origin}/api/local/comment-study/comments/detail?${new URLSearchParams({domain,...key})}`);
   assert.equal(detailResponse.status(),200);const detail=await detailResponse.json();
   await page.goto(`${origin}/corpus/comments?${new URLSearchParams({domain,view:'comments',commentWorkRef:key.workRef,commentExternalId:key.commentExternalId})}`);
   await page.getByRole('heading',{name:'所属作品',exact:true}).waitFor();
   const parentHeading=page.getByRole('heading',{name:'父评论语境',exact:true});
   if(detail.parentContext?.commentText){await parentHeading.waitFor();await page.locator('#comment-detail-body').getByText(detail.parentContext.commentText,{exact:true}).waitFor();}
   else assert.equal(await parentHeading.count(),0);
   assert.ok(await page.locator('#comment-detail-body [data-history-run="'+run+'"]').count()>=3);
  });
  await step('History to immutable run method and real request snapshot',async()=>{await page.locator('#comment-detail-body [data-history-panel="method"]').first().click();await page.getByRole('heading',{name:'本次方法',exact:true}).waitFor();await page.getByRole('tab',{name:'调用记录',exact:true}).click();await page.locator('[data-request-detail]').first().click();await page.getByRole('heading',{name:'本次冻结请求快照',exact:true}).waitFor();assert.ok((await page.locator('[data-comment-study-content]').innerText()).includes('SYNTHETIC / NOT EVIDENCE'));});
  await step('Signal links actual long-lived problem definition and evidence',async()=>{await page.getByRole('tab',{name:'研究信号',exact:true}).click();await page.locator('[data-signal-problem="'+problem+'"]').first().click();await page.locator('#sr3-drawer-title').waitFor();assert.equal(new URL(page.url()).searchParams.get('problemRef'),problem);await page.locator('[data-sr3="context"]').first().click();await page.getByText('父评论只用于理解这条表达，不计为它的独立支持依据。',{exact:true}).first().waitFor();await page.locator('[data-sr3="close"]').click();assert.equal(new URL(page.url()).searchParams.has('problemRef'),false);});
  await step('Problem cards reopen and browser history restores scope',async()=>{await page.locator('[data-sr3="open"]').first().click();await page.locator('#sr3-drawer-title').waitFor();await page.goBack();await page.locator('#sr3-drawer-title').waitFor({state:'hidden'});await page.goForward();await page.locator('#sr3-drawer-title').waitFor();await page.locator('[data-sr3="close"]').click();});
  await step('Deferred clues verify fresh source before showing comparisons',async()=>{await page.getByRole('button',{name:'未归并线索',exact:true}).click();await page.locator('[data-sr3-select="candidate"]').selectOption('deferred_novel');await page.locator('[data-sr3="clue"]').first().click();await page.getByRole('heading',{name:'当前候选比较记录',exact:true}).waitFor();await page.locator('[data-sr3="run"]').click();await page.getByRole('heading',{name:'研究结果',exact:true}).waitFor();await page.goBack();await page.locator('[data-sr3="clue"]').first().waitFor();});
  await step('Four view navigation keeps domain and overview stays read-only',async()=>{await button('概览').click();await page.getByRole('heading',{name:'近期获得新依据的问题',exact:true}).waitFor();await page.locator('[data-series-days="7"]').click();await page.locator('[data-series-view="table"]').click();assert.equal(new URL(page.url()).searchParams.get('domain'),domain);});
  await step('Responsive dialogs retain reachable close controls without page overflow',async()=>{
   for(const [width,dpr] of [[1440,1],[1024,1],[390,1],[720,2]]){
    const scoped=await browser.newContext({viewport:{width,height:1000},deviceScaleFactor:dpr,reducedMotion:'reduce'});await scoped.route('**/*',networkGuard);const check=await scoped.newPage();check.on('pageerror',e=>report.errors.push(e.message));
    try{await check.goto(`${origin}/corpus/comments?${new URLSearchParams({domain,view:'comments'})}`);await check.locator('#comment-filter-form').waitFor();const filters=await check.locator('#comment-filter-form').evaluate(form=>{const labels=[...form.querySelectorAll('label')].slice(0,4).map(label=>label.getBoundingClientRect());return {clientWidth:form.clientWidth,scrollWidth:form.scrollWidth,labelWidths:labels.map(label=>label.width),labelHeights:labels.map(label=>label.height)};});assert.ok(filters.scrollWidth<=filters.clientWidth+1,JSON.stringify(filters));assert.ok(filters.labelWidths.every(width=>width>=72),JSON.stringify(filters));assert.ok(filters.labelHeights.every(height=>height<80),JSON.stringify(filters));await check.screenshot({path:path.join(root,`filters-${width}-${dpr}x.png`),fullPage:true});await check.locator('button[data-comment-work][data-comment-id]').first().click();await check.locator('#comment-detail-close').waitFor();
     const geometry=await check.evaluate(()=>{const r=document.querySelector('#comment-detail-close').getBoundingClientRect();return {x:r.x,y:r.y,right:r.right,bottom:r.bottom,width:innerWidth,height:innerHeight,documentWidth:document.documentElement.scrollWidth};});
     assert.ok(geometry.x>=0&&geometry.y>=0&&geometry.right<=geometry.width&&geometry.bottom<=geometry.height,JSON.stringify(geometry));assert.ok(geometry.documentWidth<=geometry.width,JSON.stringify(geometry));await check.screenshot({path:path.join(root,`viewport-${width}-${dpr}x.png`),fullPage:true});await check.locator('#comment-detail-close').click();assert.equal(await check.locator('#comment-detail-dialog').count(),0);
    }finally{await scoped.close();}
   }
  });
  assert.deepEqual(report.writes,[]);
 }
 assert.deepEqual(report.errors,[]);assert.deepEqual(report.blocked,[]);
 report.status='PASS';
}catch(error){report.status='FAIL';report.failure=error.message;throw error;}finally{await Promise.allSettled(pending);await fs.writeFile(path.join(root,'report.json'),JSON.stringify(report,null,2));await browser.close();}
console.log(`D3 ${mode}: ${report.steps.length} real browser scenarios passed`);
