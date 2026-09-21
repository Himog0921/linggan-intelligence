// Synthetic browser proof. A disposable profile and dead proxy prevent access to real services.
// PLAYWRIGHT_MODULE_PATH may point at an already installed Playwright module.
import assert from 'node:assert/strict';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { createRequire } from 'node:module';
import { indexedDB, IDBKeyRange } from 'fake-indexeddb';
globalThis.indexedDB=indexedDB; globalThis.IDBKeyRange=IDBKeyRange;
const require=createRequire(import.meta.url);
const { chromium }=require(process.env.PLAYWRIGHT_MODULE_PATH || 'playwright');
const { default:Dexie }=await import('dexie');
const { createTaskSpec }=await import('../src/linggan/adapter.js');
const { createDetailPageSessionStore,createDetailPageLanePreparationStore }=await import('../src/linggan/detailPageSessionStore.js');
const fixture=new Dexie('mv3-fixture');
fixture.version(1).stores({sessions:'&cacheKey, prunableAt',preparations:'&taskId'});
const lanes=['content_detail','media_slots','comments','replies'];
const instance='10000000-0000-4000-8000-000000000001';
const specs=lanes.map((capability,index)=>createTaskSpec({taskId:`20000000-0000-4000-8000-${String(index+1).padStart(12,'0')}`,
  source:'scheduled',platform:'xhs',pageType:'note_detail',target:{contentExternalId:'synthetic-mv3'},
  capabilitiesRequested:[capability],maximumQuota:1,riskPolicy:'server_authorized_leased',stopConditions:['maximum_quota']}));
await createDetailPageLanePreparationStore({table:fixture.preparations,transaction:work=>fixture.transaction('rw',fixture.preparations,work)}).recordLanes({
  sessionRef:'synthetic-session',leaseRef:'original-lease',contentExternalId:'synthetic-mv3',
  lanes:specs.map((taskSpec,index)=>({taskSpec,taskId:taskSpec.taskId,capability:lanes[index],
    attemptId:`30000000-0000-4000-8000-${String(index+1).padStart(12,'0')}`,leaseExpiresAt:'2026-01-01T00:00:00Z'}))});
await createDetailPageSessionStore(fixture.sessions).put({leaseRef:'original-lease',
  plan:{contractVersion:'linggan.detail-page-session.v1',contentExternalId:'synthetic-mv3',lanes,commentLimit:30,replyExpandLimit:2,cacheTtlSeconds:1},
  note:{noteId:'synthetic-mv3',title:'SYNTHETIC / NOT EVIDENCE',observedAt:'2026-01-01T00:00:00Z'},
  commentResult:{total:0,comments:[],stopReason:'surface_ended'}});
const seed={sessions:await fixture.sessions.toArray(),lanePreparations:await fixture.preparations.toArray()};
const profile=await mkdtemp(path.join(tmpdir(),'linggan-mv3-proof-'));
const extension=path.resolve('dist');
let context;
try {
  context=await chromium.launchPersistentContext(profile,{headless:true,channel:'chromium',
    ...(process.env.CHROMIUM_EXECUTABLE_PATH ? {executablePath:process.env.CHROMIUM_EXECUTABLE_PATH}:{}),
    args:[`--disable-extensions-except=${extension}`,`--load-extension=${extension}`,
      '--proxy-server=http://127.0.0.1:9','--proxy-bypass-list=<-loopback>']});
  await context.route('**/*',route=>route.abort());
  const worker=context.serviceWorkers()[0] || await context.waitForEvent('serviceworker');
  const id=new URL(worker.url()).hostname;
  const page=await context.newPage();
  await page.goto(`chrome-extension://${id}/popup.html`);
  await page.evaluate(()=>chrome.runtime.sendMessage({action:"lingganFlushLocalOutbox"}));
  await page.evaluate(async ({seed,instance})=>{
    await chrome.storage.local.set({'linggan.localTrusted.producerInstanceId':instance});
    const db=await new Promise((resolve,reject)=>{const r=indexedDB.open('LingganDetailPageSessionCache');r.onsuccess=()=>resolve(r.result);r.onerror=()=>reject(r.error);});
    await new Promise((resolve,reject)=>{const tx=db.transaction(['sessions','lanePreparations'],'readwrite');
      for(const [table,rows] of Object.entries(seed)) for(const row of rows) tx.objectStore(table).put(row);
      tx.oncomplete=resolve;tx.onerror=()=>reject(tx.error);});db.close();
  },{seed,instance});
  const flush=()=>page.evaluate(()=>chrome.runtime.sendMessage({action:'lingganFlushLocalOutbox'}));
  const rows=()=>page.evaluate(async()=>{
    const db=await new Promise((resolve,reject)=>{const r=indexedDB.open('LingganLocalProducerOutbox');r.onsuccess=()=>resolve(r.result);r.onerror=()=>reject(r.error);});
    const rows=await new Promise((resolve,reject)=>{const r=db.transaction('submissions').objectStore('submissions').getAll();r.onsuccess=()=>resolve(r.result);r.onerror=()=>reject(r.error);});db.close();return rows;
  });
  await flush();
  await flush();
  const before=await rows();
  assert.equal(before.length,4,'offline page facts enter four durable envelopes');
  const cdp=await context.newCDPSession(page);
  let version;
  cdp.on('ServiceWorker.workerVersionUpdated',event=>{version=event.versions.find(v=>v.scriptURL===worker.url()&&v.runningStatus==='running') || version;});
  await cdp.send('ServiceWorker.enable');
  for(let n=0;!version&&n<40;n++) await new Promise(r=>setTimeout(r,50));
  assert.ok(version,'the actual MV3 worker is running');
  await worker.evaluate(()=>{globalThis.beforeStopMarker='old-worker';});
  await cdp.send('ServiceWorker.stopWorker',{versionId:version.versionId});
  await page.evaluate(()=>chrome.runtime.sendMessage({action:'lingganGetProducerInstance'}));
  const restarted=context.serviceWorkers()[0];
  assert.equal(await restarted.evaluate(()=>globalThis.beforeStopMarker),undefined,'MV3 execution context was replaced');
  await restarted.evaluate(()=>{
    globalThis.syntheticSubmissions=[];
    globalThis.fetch=async(url,options={})=>{
      const parsed=new URL(url);
      if(parsed.origin!=='http://localhost:3000') throw new Error('real_network_forbidden');
      let body;
      if(parsed.pathname==='/health') body={service:'linggan-local-web',listener:'loopback-only',dataState:'LINGGAN_BROWSER_PRODUCER_RUNTIME',
        database:{state:'READY',schema:'PLUGIN_RUNTIME_002_SCHEMA_READY'},routes:{localProducer:{
          taskCreation:'/api/local/producer/tasks',attemptStart:'/api/local/producer/attempts',submission:'/api/local/producer/submissions',mediaAcquisitionClaim:'/api/local/producer/media-claim'}}};
      else if(parsed.pathname==='/api/local/producer/tasks') body={outcome:'replay'};
      else if(parsed.pathname==='/api/local/producer/attempts') body={outcome:'started'};
      else if(parsed.pathname==='/api/local/producer/submissions') {
        globalThis.syntheticSubmissions.push(JSON.parse(options.body));
        body={delivery:'acknowledged',material_admission:'ACCEPTED',execution_effect:'LOST_AUTHORITY'};
      } else throw new Error('unexpected_route');
      return new Response(JSON.stringify(body),{status:200,headers:{'content-type':'application/json'}});
    };
  });
  await flush(); // Complete any offline startup flush before advancing retry timestamps.
  // Expired retry timestamps merely fast-forward this isolated fixture's delivery clock.
  await page.evaluate(async()=>{
    const db=await new Promise(resolve=>{const r=indexedDB.open('LingganLocalProducerOutbox');r.onsuccess=()=>resolve(r.result);});
    await new Promise((resolve,reject)=>{const tx=db.transaction('submissions','readwrite');const r=tx.objectStore('submissions').openCursor();
      r.onsuccess=()=>{const cursor=r.result;if(cursor){cursor.update({...cursor.value,nextAttemptAt:0});cursor.continue();}};
      tx.oncomplete=resolve;tx.onerror=()=>reject(tx.error);});db.close();
  });
  await flush();
  const after=await rows();
  const submissions=await restarted.evaluate(()=>globalThis.syntheticSubmissions);

  assert.equal(after.filter(row=>row.status==='acknowledged').length,4);
  assert.deepEqual(after.map(row=>[row.submissionId,row.attemptId,row.capturePackage]),before.map(row=>[row.submissionId,row.attemptId,row.capturePackage]));
  assert.equal(submissions.length,4);
  assert.equal(context.pages().filter(p=>/^https?:/.test(p.url())).length,0,'no platform navigation');
  console.log(JSON.stringify({proof:'SYNTHETIC_MV3_WORKER_RESTART',lanes:4,acknowledged:4,identityAndPayloadUnchanged:true,platformNavigations:0,realServiceRequests:0}));
} finally {
  await context?.close(); await fixture.delete(); await rm(profile,{recursive:true,force:true});
}
