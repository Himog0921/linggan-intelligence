// COMMENT-DAILY-001: only the explicitly marked, disposable local synthetic preview.
import assert from 'node:assert/strict';
import {randomUUID} from 'node:crypto';
const [origin,provider]=process.argv.slice(2);
for(const value of [origin,provider]){const u=new URL(value);assert.equal(u.hostname,'127.0.0.1');assert.equal(u.protocol,'http:');}
async function call(path,body,status=200){const r=await fetch(origin+path,{method:body?'POST':'GET',headers:{Origin:origin,...(body?{'Content-Type':'application/json'}:{})},body:body?JSON.stringify(body):undefined});assert.equal(r.status,status,`${path}: ${r.status}`);return r.json();}
const models='/api/local/model-settings',api='/api/local/comment-research';
const settings=await call(models);assert.equal(settings.secretStorage,'SYNTHETIC_PREVIEW_ONLY');assert.equal(settings.config,null);
const versionRef=randomUUID(),modelRef=randomUUID(),configRef=randomUUID();
await call(models+'/connections',{versionRef,connectionRef:randomUUID(),expectedRevision:0,name:'每日研究合成验收',api:'openai-completions',baseUrl:provider,localEndpoint:true,apiKey:'SYNTHETIC-NOT-A-CREDENTIAL'});
await call(models+'/models',{modelRef,connectionVersionRef:versionRef,modelId:'synthetic-good'});
const probe=await call(models+'/probes',{invocationRef:randomUUID(),connectionVersionRef:versionRef,modelRef,operation:'probe'});assert.equal(probe.commentQualified,true);assert.equal(probe.commentContract,'comment-research.v2');
await call(models+'/config',{configRef,expectedConfigRef:null,modelRef,inputTokenLimit:16000,outputTokenLimit:2000,timeoutSeconds:5,maxAttempts:2,autoSourceLimit:20,autoTokenLimit:100000});
const page=(await call(api)).page;
const source=page.items.find(s=>s.body?.startsWith('SYNTHETIC'));
const refs=page.items.filter(s=>s.workRef===source.workRef&&s.body?.startsWith('SYNTHETIC')).slice(0,3).map(s=>s.sourceRef);assert.equal(refs.length,3);
const batchRef=randomUUID(),request={batchRef,configRef,sourceRefs:refs,tokenLimit:100000};
await call(api+'/daily/selected',request);assert.equal((await call(api+'/daily/selected',request)).replayed,true);
let detail;
for(let i=0;i<40;i++){detail=await call(api+'/daily/'+batchRef);if(detail.items.every(i=>i.state==='succeeded'))break;await new Promise(r=>setTimeout(r,500));}
assert.ok(detail.items.every(i=>i.state==='succeeded'));
let daily=await call(api+'/daily');const batch=daily.items.find(b=>b.batchRef===batchRef);assert.equal(batch.total,3);assert.equal(batch.chargedTokens,900);
const annotations=await call(api+'/sources/'+refs[0]);assert.ok(annotations.annotations.analysis.some(a=>a.ruleVersion==='comment-research.v2'&&a.state==='succeeded'));
assert.ok((await call(api+'/groups')).items.some(g=>g.label==='合成执行精力'));
const cleaned=await call(api+'?cleanState=direct');assert.ok(cleaned.page.total>0);assert.ok(cleaned.researchStates.every(s=>s.cleanState==='direct'));
await call(api+'/daily/schedule',{expectedRevision:0,enabled:true,configRef,sourceLimit:20,tokenLimit:100000});
const s=(await call(api+'/daily')).schedule;assert.equal(s.enabled,true);assert.ok(s.next_end);
await call(api+'/daily/schedule',{expectedRevision:s.revision,enabled:false,configRef,sourceLimit:20,tokenLimit:100000});
assert.equal((await call(api+'/daily')).schedule.enabled,false);
console.log('Daily research isolated HTTP proof passed: packet probe, one work/three comments/one invocation, persisted results, exact groups, cleaning filter, schedule enable/pause, replay.');
