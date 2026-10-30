import test from 'node:test';
import assert from 'node:assert/strict';
import { execute, VERSION } from '../src/adapter.mjs';

test('dispatch marker follows transport start and precedes the provider response', async () => {
  const originalFetch=globalThis.fetch, originalWrite=process.stderr.write, previous=process.env.LINGGAN_DISPATCH_SIGNAL;
  const events=[];let respond;
  process.env.LINGGAN_DISPATCH_SIGNAL='1';
  globalThis.fetch=()=>{events.push('fetch');return new Promise(resolve=>{respond=resolve;});};
  process.stderr.write=function(chunk){events.push(String(chunk));return true;};
  try {
    const request={version:VERSION,operation:'connect',api:'openai-completions',baseUrl:'https://example.test/v1',apiKey:'synthetic',timeoutMs:3000,maxOutputTokens:32};
    const call=execute(request);
    for(let i=0;i<100&&events.length<2;i++)await new Promise(resolve=>setTimeout(resolve,5));
    assert.deepEqual(events,['fetch','LGI_DISPATCH_STARTED\n']);
    respond(new Response(JSON.stringify({data:[]}),{status:200,headers:{'content-type':'application/json'}}));
    assert.equal((await call).ok,true);
    events.length=0;assert.equal((await execute({...request,timeoutMs:1})).ok,false);assert.deepEqual(events,[]);
  } finally {
    globalThis.fetch=originalFetch;process.stderr.write=originalWrite;
    if(previous===undefined)delete process.env.LINGGAN_DISPATCH_SIGNAL;else process.env.LINGGAN_DISPATCH_SIGNAL=previous;
  }
});
