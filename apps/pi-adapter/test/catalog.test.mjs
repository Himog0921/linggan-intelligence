import test from 'node:test';
import assert from 'node:assert/strict';
import { readCatalog, catalogRate } from '../src/catalog.mjs';
import { LIMITS } from '../src/execution.mjs';
import { pinnedSdkAvailable } from '../src/sdk-version.mjs';
test('offline presets retain exact pinned source and unknown timeout, format, zero price and callability',()=>{
  assert.equal(pinnedSdkAvailable(),true);
  const page=readCatalog({api:'openai-completions',provider:'deepseek',modelId:'deepseek-v4-pro'});
  assert.equal(page.total,1);assert.equal(page.sdkVersion,'0.85.1');
  const p=page.presets[0];
  assert.equal(p.supported,true);
  assert.equal(p.limits.contextTokens,1_000_000);assert.equal(p.limits.maxOutputTokens,384_000);
  assert.equal(p.limits.maxTimeoutSeconds,null);
  assert.equal(p.outputMode,'prompt_only');assert.equal(p.strictSchema,false);
  assert.deepEqual(p.prices,{input:'435000',output:'870000',cachedRead:'3625',cachedWrite:null});
  assert.ok(p.unknownFields.includes('currentEndpointCallability'));
  assert.match(p.sourceVersion,/^pi-0\.85\.1:[0-9a-f]{64}$/);
  assert.deepEqual(readCatalog({sourceVersion:p.sourceVersion}).presets,[p]);
  assert.equal(Object.hasOwn(p,'headers'),false);
});
test('the declared resource ceilings cover the entire pinned catalog in supported protocols',()=>{
  const first=readCatalog({limit:100});let count=0,maxOutput=0;
  for(let offset=0;offset<first.total;offset+=100){
    const page=readCatalog({offset,limit:100});
    for(const p of page.presets){
      assert.ok(p.limits.contextTokens<=LIMITS.maxContextTokens,p.modelId);
      assert.ok(p.limits.maxOutputTokens<=LIMITS.maxOutputTokens,p.modelId);
      maxOutput=Math.max(maxOutput,p.limits.maxOutputTokens);count++;
    }
  }
  assert.equal(count,first.total);assert.ok(count>1000);assert.ok(maxOutput>1_048_576);
});
test('only static inventory is queried; upload paths and unbounded requests have no filesystem meaning',()=>{
  assert.equal(readCatalog({provider:'../../Downloads'}).total,0);
  assert.throws(()=>readCatalog({path:'/Users/not-a-catalog'}),/invalid_request/);
  assert.throws(()=>readCatalog({query:'x'.repeat(201)}),/invalid_request/);
  assert.throws(()=>readCatalog({api:'unsupported'}),/invalid_request/);
  assert.throws(()=>readCatalog({limit:101}),/invalid_request/);
});
test('catalog rates use exact micro-USD and preserve unknowns instead of float rounding or SDK zeroes',()=>{
  assert.equal(catalogRate(0),null);assert.equal(catalogRate(undefined),null);assert.equal(catalogRate(-1),null);
  assert.equal(catalogRate(0.003625),'3625');assert.equal(catalogRate(1.005),'1005000');
  assert.equal(catalogRate(0.0000001),null);
});
