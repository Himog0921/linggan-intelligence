// Isolated HTTP proof only. Reject any ordinary workspace before writing or invoking a provider.
import assert from 'node:assert/strict';
import {randomUUID} from 'node:crypto';
const origin=process.argv[2],provider=process.argv[3];
for(const value of [origin,provider]){const u=new URL(value);assert.equal(u.hostname,'127.0.0.1');assert.equal(u.protocol,'http:');}
async function call(path,body,status=200,overrideOrigin){
  const r=await fetch(origin+path,{method:body===undefined?'GET':'POST',headers:{Origin:overrideOrigin||origin,...(body===undefined?{}:{'Content-Type':'application/json'})},body:body===undefined?undefined:JSON.stringify(body)});
  assert.equal(r.status,status);assert.equal(r.headers.get('cache-control'),status===403?null:'no-store');return r.json();
}
const api='/api/local/model-settings';
assert.equal((await call(api)).secretStorage,'SYNTHETIC_PREVIEW_ONLY');
const candidates=(await call('/api/local/comment-research')).page.items;assert.ok(candidates.every(s=>s.body.includes('SYNTHETIC')));
// The preview includes a human annotation whose precedence must be retained. Select
// unannotated synthetic sources so this assertion specifically tests the new model group.
const details=await Promise.all(candidates.map(s=>call('/api/local/comment-research/sources/'+s.sourceRef)));
const sources=candidates.filter((_,i)=>details[i].annotations.human.length===0);assert.ok(sources.length>=2);
await call(api+'/connections',{},403,'https://example.invalid');
const connection={versionRef:randomUUID(),connectionRef:randomUUID(),expectedRevision:0,name:'HTTP 合成连接',api:'openai-completions',baseUrl:provider,localEndpoint:true,apiKey:'SYNTHETIC-NOT-A-CREDENTIAL'};
await call(api+'/connections',connection);assert.equal((await call(api+'/connections',connection)).replayed,true);
assert.ok(!(await call(api)).runs.some(r=>r.operation==='analyze'));
let discovery=await call(api+'/probes',{invocationRef:randomUUID(),connectionVersionRef:connection.versionRef,modelRef:null,operation:'discover'});assert.equal(discovery.ok,true);assert.ok(discovery.modelIds.includes('synthetic-good'));
const modelRef=randomUUID();await call(api+'/models',{modelRef,connectionVersionRef:connection.versionRef,modelId:'synthetic-good'});
const config={configRef:randomUUID(),expectedConfigRef:null,modelRef,inputTokenLimit:16000,outputTokenLimit:2000,timeoutSeconds:5,maxAttempts:2,autoSourceLimit:2,autoTokenLimit:36000};
assert.equal((await call(api+'/config',config,409)).error,'model_not_qualified');
const probe={invocationRef:randomUUID(),connectionVersionRef:connection.versionRef,modelRef,operation:'probe'};
assert.equal((await call(api+'/probes',probe)).commentQualified,true);assert.equal((await call(api+'/probes',probe)).replayed,true);
await call(api+'/config',config);await call(api+'/config',config);
let state=await call(api);assert.equal(state.config.configRef,config.configRef);assert.ok(!state.runs.some(r=>r.operation==='analyze'));
assert.ok(!JSON.stringify(state).includes(connection.apiKey));assert.ok(!JSON.stringify(state).includes('secret_ref'));
const plan={planRef:randomUUID(),configRef:config.configRef,kind:'trial',sourceRefs:[sources[0].sourceRef],sourceLimit:1,tokenLimit:36000,expectedAutoPlanRef:null};
await call(api+'/plans',plan);assert.equal((await call(api+'/plans',plan)).replayed,true);
for(let step=0;step<40;step++){state=await call(api);if(state.runs.some(r=>r.operation==='analyze'&&r.state==='succeeded'))break;await new Promise(r=>setTimeout(r,500));}
const run=state.runs.find(r=>r.operation==='analyze');assert.equal(run.state,'succeeded');assert.equal(run.inputTokens,800);assert.equal(run.outputTokens,100);assert.equal(run.costUsd,null);assert.equal(state.worker.recent,true);
const detail=await call('/api/local/comment-research/sources/'+sources[0].sourceRef);assert.equal(detail.annotations.analysis[0].state,'succeeded');
const groups=await call('/api/local/comment-research/groups');assert.ok(groups.items.some(i=>i.label==='合成执行精力'));
assert.equal((await call('/api/local/comment-research')).modelState,'CONFIGURED');
// Keep the connection disabled throughout the resume/replay checks. Deterministic
// pending-work execution and quota preservation are covered by the PostgreSQL proof.
await call(api+'/connections/state',{connectionRef:connection.connectionRef,expectedRevision:1,enabled:false});
const backfill={...plan,planRef:randomUUID(),kind:'backfill',sourceRefs:[sources[1].sourceRef],sourceLimit:1};
// A new grant requires an enabled connection; disable it again immediately afterward.
await call(api+'/connections/state',{connectionRef:connection.connectionRef,expectedRevision:2,enabled:true});
await call(api+'/plans',backfill);await call(api+'/connections/state',{connectionRef:connection.connectionRef,expectedRevision:3,enabled:false});await call(api+'/plans/'+backfill.planRef+'/stop',{});
assert.equal((await call(api+'/plans',backfill)).enabled,false);
// Existing request replay is read-only even while the connection is disabled.
let paused=(await call(api+'?planRef='+backfill.planRef)).plans[0];assert.equal(paused.planRef,backfill.planRef);assert.equal(paused.enabled,false);
const resume={expectedRevision:paused.revision};await call(api+'/plans/'+backfill.planRef+'/resume',resume);
await call(api+'/plans/'+backfill.planRef+'/stop',{});await call(api+'/plans/'+backfill.planRef+'/resume',resume,409);
paused=(await call(api+'?planRef='+backfill.planRef)).plans[0];await call(api+'/plans/'+backfill.planRef+'/resume',{expectedRevision:paused.revision});
await call(api+'/connections/state',{connectionRef:connection.connectionRef,expectedRevision:4,enabled:true});
const overlap=await call(api+'/plans',{...backfill,planRef:randomUUID()});assert.equal(overlap.queued,0);assert.equal(overlap.existingPlans[0].planRef,backfill.planRef);
for(let step=0;step<40;step++){state=await call(api);if(state.runs.some(r=>r.sourceRef===sources[1].sourceRef&&r.state==='succeeded'))break;await new Promise(r=>setTimeout(r,500));}
assert.equal(state.runs.filter(r=>r.sourceRef===sources[1].sourceRef&&r.operation==='analyze').length,1);
assert.equal(state.runs.find(r=>r.sourceRef===sources[1].sourceRef).state,'succeeded');
const automatic={...plan,planRef:randomUUID(),kind:'automatic',sourceRefs:[],sourceLimit:2};await call(api+'/plans',automatic);await call(api+'/plans/'+automatic.planRef+'/stop',{});assert.equal((await call(api+'/plans',automatic)).enabled,false);
await call(api+'/connections/state',{connectionRef:connection.connectionRef,expectedRevision:5,enabled:false});
assert.equal((await call('/api/local/comment-research')).modelState,'PAUSED');
await call(api+'/connections/state',{connectionRef:connection.connectionRef,expectedRevision:1,enabled:true},409);
console.log('Model Pi isolated HTTP proof passed: settings, probes, defaults, real SDK worker, source output, usage, scope, pause and replay.');
