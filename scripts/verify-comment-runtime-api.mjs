// CI-RUN-002: exercise real HTTP routes against a disposable, marked synthetic preview.
import assert from 'node:assert/strict';
const [origin] = process.argv.slice(2);
const url = new URL(origin);
assert.equal(url.protocol, 'http:'); assert.equal(url.hostname, '127.0.0.1');
async function call(path, body, status=200, requestOrigin=origin) {
  const response = await fetch(origin+path, {method:body?'POST':'GET',headers:{Origin:requestOrigin,...(body?{'Content-Type':'application/json'}:{})},body:body?JSON.stringify(body):undefined});
  assert.equal(response.status,status,`${path}: ${response.status}`);
  return response.json();
}
const models=await call('/api/local/model-settings');
assert.equal(models.secretStorage,'SYNTHETIC_PREVIEW_ONLY');
const base='/api/local/comment-research/daily', own='00000000-0000-4000-8000-000000000001';
const daily=await call(base), batch=daily.items[0];assert.ok(batch);
const detail=await call(`${base}/${batch.batchRef}?domain=${own}`);
assert.equal(detail.callTotal,1);assert.equal(detail.calls[0].state,'partial');assert.ok(detail.calls[0].workTitle);
const trace=await call(`${base}/${batch.batchRef}/requests/${detail.calls[0].invocationRef}?domain=${own}`);
assert.equal(trace.availability,'AVAILABLE');assert.equal(trace.input.comments.length,3);assert.equal(trace.validation.length,1);assert.ok(trace.validation[0].path.includes('.quote'));
await call(`${base}/${batch.batchRef}?domain=00000000-0000-4000-8000-000000000002`,undefined,403);
const settings=await call(`${base}/context-settings`);
await call(`${base}/context-settings`,{expectedRevision:settings.revision,policy:settings.policy},403,'https://untrusted.invalid');
assert.equal((await call(`${base}/context-settings`)).revision,settings.revision);
const updated=await call(`${base}/context-settings`,{expectedRevision:settings.revision,policy:{...settings.policy,parent:false}});
assert.equal(updated.policy.parent,false);assert.equal((await call(base)).schedule.enabled,false);
await call(`${base}/context-settings`,{expectedRevision:settings.revision,policy:settings.policy},409);
await call(`${base}/context-settings`,{expectedRevision:updated.revision,policy:settings.policy});
assert.equal((await call(`${base}/${batch.batchRef}/requests/${detail.calls[0].invocationRef}?domain=${own}`)).policy.parent,true);
console.log('CI-RUN-002 isolated HTTP proof passed: real batch/request/context routes, source scope, Origin guard, optimistic save, frozen policy and unchanged disabled daily schedule.');
