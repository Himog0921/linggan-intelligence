import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';

// Real browser lifecycle proof; all reads stay on the candidate's loopback origin.
// visibility/page-transition events are controlled stimuli; viewport and media changes use browser APIs.
export async function verifyLoginLifecycle(browser, origin, out) {
await mkdir(out, { recursive: true });
const report = { steps: [], errors: [], blocked: [], timeline: [] };
const context=await browser.newContext({viewport:{width:1440,height:900},reducedMotion:'no-preference'});const page=await context.newPage();
page.on('pageerror',e=>report.errors.push(String(e)));await page.route('**/*',r=>{if(new URL(r.request().url()).origin!==origin||!['GET','HEAD'].includes(r.request().method())){report.blocked.push(r.request().url());return r.abort();}return r.continue();});
await page.addInitScript(()=>{
 const pending=new Set(),observers=new Set(),listeners=[];let sequence=0;const raf=requestAnimationFrame,caf=cancelAnimationFrame;
 window.requestAnimationFrame=fn=>{let id=raf.call(window,t=>{pending.delete(id);fn(t);});pending.add(id);return id;};window.cancelAnimationFrame=id=>{pending.delete(id);return caf.call(window,id);};
 const IO=IntersectionObserver;window.IntersectionObserver=class extends IO{constructor(...args){super(...args);observers.add(this);}disconnect(){observers.delete(this);return super.disconnect();}};
 for(const target of [window,document]){const add=target.addEventListener.bind(target),remove=target.removeEventListener.bind(target);target.addEventListener=(type,fn,...rest)=>{if(['visibilitychange','pagehide','pageshow'].includes(type)&&!listeners.some(x=>x.target===target&&x.type===type&&x.fn===fn))listeners.push({target,type,fn,id:++sequence,owner:new Error().stack.includes('/ui-assets/LoginPage-')?'login':'shared'});return add(type,fn,...rest);};target.removeEventListener=(type,fn,...rest)=>{const i=listeners.findIndex(x=>x.target===target&&x.type===type&&x.fn===fn);if(i>=0)listeners.splice(i,1);return remove(type,fn,...rest);};}
 window.__sceneProof={inspect:()=>({pending:pending.size,observers:observers.size,listeners:listeners.map(x=>({id:x.id,type:x.type,owner:x.owner}))})};
});
const value=()=>page.locator('#station-scene').evaluate(el=>({time:Number(el.dataset.time),status:el.dataset.status,svg:el.querySelectorAll('svg.station-live').length}));
async function step(name,fn){await fn();report.steps.push(name);await writeFile(path.join(out,'lifecycle-report.json'),JSON.stringify(report,null,2));}
try{
 await page.goto(origin+'/login');await page.waitForFunction(()=>document.querySelector('#station-scene')?.dataset.status==='running');
 await step('complete16secondLoop',async()=>{let wrapped=false,last=-1;for(let i=0;i<90;i++){const v=await value();report.timeline.push(v);if(last>15&&v.time<1)wrapped=true;last=v.time;await page.waitForTimeout(200);if(wrapped)break;}assert.ok(wrapped,'16-second loop must wrap');await page.screenshot({path:path.join(out,'running.png')});});
 await step('offscreenPauseAndResume',async()=>{await page.locator('#station-scene').evaluate(el=>el.style.display='none');await page.waitForFunction(()=>document.querySelector('#station-scene')?.dataset.status==='paused');const before=await value();await page.waitForTimeout(400);assert.equal((await value()).time,before.time);await page.locator('#station-scene').evaluate(el=>el.style.display='');await page.waitForFunction(()=>document.querySelector('#station-scene')?.dataset.status==='running');const first=await value();assert.ok((first.time-before.time+16)%16<.25);await page.waitForTimeout(250);assert.notEqual((await value()).time,first.time);});
 await step('visibilityEventPauseAndResume',async()=>{await page.evaluate(()=>{Object.defineProperty(document,'hidden',{configurable:true,value:true});document.dispatchEvent(new Event('visibilitychange'));});const before=await value();assert.equal(before.status,'paused');await page.waitForTimeout(300);assert.equal((await value()).time,before.time);await page.evaluate(()=>{delete document.hidden;document.dispatchEvent(new Event('visibilitychange'));});await page.waitForFunction(()=>document.querySelector('#station-scene')?.dataset.status==='running');});
 await step('mobileDoesNotMountLiveSvgAndDesktopRestoresOne',async()=>{await page.setViewportSize({width:390,height:844});await page.waitForFunction(()=>document.querySelector('#station-scene')?.dataset.status==='mobile');assert.equal((await value()).svg,0);await page.setViewportSize({width:1440,height:900});await page.waitForFunction(()=>document.querySelector('#station-scene')?.dataset.status==='running');assert.equal((await value()).svg,1);});
 await step('reducedMotionStable8400ms',async()=>{await page.emulateMedia({reducedMotion:'reduce'});await page.waitForFunction(()=>document.querySelector('#station-scene')?.dataset.status==='still');assert.equal((await value()).time,8.4);await page.waitForTimeout(300);assert.equal((await value()).time,8.4);});
 await step('persistedPageHideAndShow',async()=>{await page.evaluate(()=>window.dispatchEvent(new PageTransitionEvent('pagehide',{persisted:true})));assert.equal((await value()).status,'paused');await page.evaluate(()=>window.dispatchEvent(new PageTransitionEvent('pageshow',{persisted:true})));assert.equal((await value()).status,'still');});
 await step('normalMotionResumesAfterPreferenceChange',async()=>{await page.emulateMedia({reducedMotion:'no-preference'});await page.waitForFunction(()=>document.querySelector('#station-scene')?.dataset.status==='running');const before=await value();await page.waitForTimeout(200);assert.notEqual((await value()).time,before.time);});
 await step('reactRouteUnmountReleasesRafObserversAndListeners',async()=>{const mounted=await page.evaluate(()=>window.__sceneProof.inspect());assert.ok(mounted.listeners.filter(x=>x.owner==='login').length>=3);await page.evaluate(()=>{history.pushState(null,'','/settings/models');window.dispatchEvent(new PopStateEvent('popstate'));});await page.locator('#station-scene').waitFor({state:'detached'});await page.waitForTimeout(200);const after=await page.evaluate(()=>window.__sceneProof.inspect());report.afterUnmount=after;assert.equal(after.pending,0);assert.equal(after.observers,0);assert.deepEqual(after.listeners.filter(x=>mounted.listeners.some(old=>old.id===x.id&&old.owner==='login')),[]);});
 assert.deepEqual(report.errors,[]);assert.deepEqual(report.blocked,[]);
}finally{await context.close();await writeFile(path.join(out,'lifecycle-report.json'),JSON.stringify(report,null,2));}
return report;
}
