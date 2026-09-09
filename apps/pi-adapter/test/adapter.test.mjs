import test from 'node:test';
import assert from 'node:assert/strict';
import http from 'node:http';
import { execute, VERSION } from '../src/adapter.mjs';

async function server(handler) {
  const s=http.createServer(handler);await new Promise(resolve=>s.listen(0,'127.0.0.1',resolve));
  return {url:`http://127.0.0.1:${s.address().port}/v1`,close:()=>new Promise(resolve=>s.close(resolve))};
}
const request=(baseUrl,extra={})=>({version:VERSION,operation:'probe',api:'openai-completions',baseUrl,
  localEndpoint:true,apiKey:'synthetic-credential-only',modelId:'synthetic-model',timeoutMs:3000,maxOutputTokens:128,
  system:'Synthetic protocol proof, no real material.',prompt:'Return a JSON object.',...extra});
const DIAGNOSTIC_KEYS=['elapsedMs','finishReason','httpStatus','receivedBytes','responseStarted','retryClass','schemaVersion','sdkErrorType','stage','terminalReceived','usageKnown'];
function assertDiagnostic(result,expected={}) {
  const d=result.diagnostic;
  assert.deepEqual(Object.keys(d).sort(),DIAGNOSTIC_KEYS);
  assert.equal(d.schemaVersion,1);
  assert.ok(Number.isSafeInteger(d.elapsedMs)&&d.elapsedMs>=0);
  for(const [key,value] of Object.entries(expected))assert.equal(d[key],value,`diagnostic.${key}`);
  return d;
}
function success(res,text='{"ok":true}',usage=true) {
  res.writeHead(200,{'Content-Type':'text/event-stream'});
  const chunk=(delta,finish_reason=null)=>({id:'synthetic-response',object:'chat.completion.chunk',created:1,model:'synthetic-model',choices:[{index:0,delta,finish_reason}]});
  res.write(`data: ${JSON.stringify(chunk({role:'assistant',content:text}))}\n\n`);
  res.write(`data: ${JSON.stringify(chunk({},'stop'))}\n\n`);
  if(usage)res.write(`data: ${JSON.stringify({id:'synthetic-response',choices:[],usage:{prompt_tokens:17,completion_tokens:5,total_tokens:22}})}\n\n`);
  res.end('data: [DONE]\n\n');
}
test('actual Pi AI and Agent SDK use explicit key, no tools, one request, recorded wire usage',async()=>{
  let calls=0;
  const s=await server(async(req,res)=>{
    calls++;assert.equal(req.headers.authorization,'Bearer synthetic-credential-only');
    const chunks=[];for await(const c of req)chunks.push(c);
    const body=JSON.parse(Buffer.concat(chunks));assert.equal(body.model,'synthetic-model');
    assert.ok(!body.tools?.length);assert.equal(body.stream,true);success(res);
  });
  try{const result=await execute(request(s.url));assert.equal(result.ok,true);assert.equal(result.text,'{"ok":true}');assert.deepEqual(result.usage,{inputTokens:17,outputTokens:5,costUsd:null});assert.equal(calls,1);const d=assertDiagnostic(result,{stage:'terminal',httpStatus:200,responseStarted:true,terminalReceived:true,finishReason:'stop',usageKnown:true,sdkErrorType:null,retryClass:'never'});assert.ok(d.receivedBytes>0);}finally{await s.close();}
});
test('omitted provider usage remains unknown and catalog is an account endpoint list',async()=>{
  const s=await server((req,res)=>{if(req.url.endsWith('/models')){res.writeHead(200,{'Content-Type':'application/json'});res.end(JSON.stringify({data:[{id:'synthetic-model'}]}));}else success(res,'{"ok":true}',false);});
  try{const d=await execute(request(s.url,{operation:'discover'}));assert.deepEqual(d.modelIds,['synthetic-model']);assert.equal(d.modelListOrigin,'ACCOUNT_ENDPOINT');assertDiagnostic(d,{stage:'terminal',httpStatus:200,responseStarted:true,terminalReceived:true,finishReason:null,usageKnown:false,sdkErrorType:null,retryClass:'never'});const r=await execute(request(s.url));assert.equal(r.usage.inputTokens,null);assert.equal(r.usage.costUsd,null);assertDiagnostic(r,{usageKnown:false});}finally{await s.close();}
});
test('HTTP diagnostics distinguish rate limits and outages without leaking credential-like bodies',async()=>{
  let calls=0;const s=await server((req,res)=>{calls++;res.writeHead(401);res.end('synthetic-credential-only raw provider error');});
  try{const result=await execute(request(s.url));assert.equal(result.ok,false);assert.ok(!JSON.stringify(result).includes('synthetic-credential-only'));assert.equal(calls,1);assertDiagnostic(result,{stage:'failed',httpStatus:401,responseStarted:true,terminalReceived:null,receivedBytes:null,finishReason:null,usageKnown:false,sdkErrorType:'http',retryClass:'never'});}finally{await s.close();}
  const rate=await server((req,res)=>{res.writeHead(429);res.end('Authorization: Bearer synthetic-credential-only https://127.0.0.1/secret');});
  try{const result=await execute(request(rate.url));assert.equal(result.failureCode,'provider_rate_limited');assert.ok(!JSON.stringify(result).includes('synthetic-credential-only'));assert.ok(!JSON.stringify(result).includes('Authorization'));assert.ok(!JSON.stringify(result).includes(rate.url));assertDiagnostic(result,{stage:'failed',httpStatus:429,responseStarted:true,terminalReceived:null,receivedBytes:null,finishReason:null,usageKnown:false,sdkErrorType:'http',retryClass:'after_cooldown'});}finally{await rate.close();}
  const unavailable=await server((req,res)=>{res.writeHead(500);res.end('synthetic provider failure');});
  try{const result=await execute(request(unavailable.url));assert.equal(result.failureCode,'provider_unavailable');assertDiagnostic(result,{stage:'failed',httpStatus:500,responseStarted:true,terminalReceived:null,receivedBytes:null,finishReason:null,usageKnown:false,sdkErrorType:'http',retryClass:'after_cooldown'});}finally{await unavailable.close();}
  const redirect=await server((req,res)=>{res.writeHead(302,{location:'https://example.invalid/leak'});res.end();});
  try{assert.equal((await execute(request(redirect.url))).ok,false);}finally{await redirect.close();}
});
test('an unending provider stream is locally aborted',async()=>{
  const s=await server((req,res)=>{res.writeHead(200,{'Content-Type':'text/event-stream'});res.write(':waiting\n\n');});
  try{const started=Date.now();const r=await execute(request(s.url,{timeoutMs:100}));assert.equal(r.ok,false);assert.equal(r.failureCode,'provider_timeout');assert.ok(Date.now()-started<1500);const d=assertDiagnostic(r,{stage:'timed_out',httpStatus:200,responseStarted:true,terminalReceived:null,finishReason:null,usageKnown:false,sdkErrorType:'timeout',retryClass:'manual_review'});assert.ok(d.receivedBytes>0);}finally{await s.close();}
});
test('actual Pi SDK distinguishes an interrupted stream, missing terminal, and output limit',async()=>{
  const first={id:'synthetic-response',object:'chat.completion.chunk',choices:[{index:0,delta:{role:'assistant',content:'{"ok":true}'},finish_reason:null}]};
  const missing=await server((req,res)=>{res.writeHead(200,{'Content-Type':'text/event-stream'});res.end(`data: ${JSON.stringify(first)}\n\n`);});
  try{const r=await execute(request(missing.url));assert.equal(r.failureCode,'provider_terminal_missing');const d=assertDiagnostic(r,{stage:'failed',httpStatus:200,responseStarted:true,terminalReceived:null,finishReason:null,usageKnown:false,sdkErrorType:'invalid_response',retryClass:'manual_review'});assert.ok(d.receivedBytes>0);}finally{await missing.close();}
  const interrupted=await server((req,res)=>{res.writeHead(200,{'Content-Type':'text/event-stream'});res.write(`data: ${JSON.stringify(first)}\n\n`);setTimeout(()=>res.socket.destroy(),5);});
  try{const r=await execute(request(interrupted.url));assert.equal(r.failureCode,'provider_stream_interrupted');const d=assertDiagnostic(r,{stage:'failed',httpStatus:200,responseStarted:true,terminalReceived:null,finishReason:null,usageKnown:false,sdkErrorType:'stream_interrupted',retryClass:'manual_review'});assert.ok(d.receivedBytes>0);}finally{await interrupted.close();}
  const limited=await server((req,res)=>{const chunk=(delta,finishReason=null)=>({id:'synthetic-response',object:'chat.completion.chunk',choices:[{index:0,delta,finish_reason:finishReason}]});res.writeHead(200,{'Content-Type':'text/event-stream'});res.write(`data: ${JSON.stringify(chunk({role:'assistant',content:'{"ok":true}'}))}\n\n`);res.write(`data: ${JSON.stringify(chunk({},'length'))}\n\n`);res.end('data: [DONE]\n\n');});
  try{const r=await execute(request(limited.url));assert.equal(r.failureCode,'output_limit');const d=assertDiagnostic(r,{stage:'terminal',httpStatus:200,responseStarted:true,terminalReceived:true,finishReason:'length',usageKnown:false,sdkErrorType:null,retryClass:'never'});assert.ok(d.receivedBytes>0);}finally{await limited.close();}
});
test('real Anthropic SDK protocol preserves start input and final output usage',async()=>{
  const s=await server(async(req,res)=>{
    assert.ok(req.url.startsWith('/v1/messages'));assert.equal(req.headers['x-api-key'],'synthetic-credential-only');
    const chunks=[];for await(const c of req)chunks.push(c);const body=JSON.parse(Buffer.concat(chunks));assert.equal(body.max_tokens,128);assert.ok(!body.tools?.length);
    res.writeHead(200,{'Content-Type':'text/event-stream'});
    const events=[{type:'message_start',message:{id:'msg_synthetic',model:'synthetic-model',role:'assistant',type:'message',content:[],usage:{input_tokens:10,output_tokens:0,cache_read_input_tokens:3}}},
      {type:'content_block_start',index:0,content_block:{type:'text',text:''}},
      {type:'content_block_delta',index:0,delta:{type:'text_delta',text:'{"ok":true}'}},
      {type:'content_block_stop',index:0},{type:'message_delta',delta:{stop_reason:'end_turn'},usage:{output_tokens:5}},{type:'message_stop'}];
    for(const e of events)res.write(`event: ${e.type}\ndata: ${JSON.stringify(e)}\n\n`);res.end();
  });
  try{const r=await execute(request(s.url.replace(/\/v1$/,''),{api:'anthropic-messages'}));assert.equal(r.ok,true);assert.equal(r.text,'{"ok":true}');assert.deepEqual(r.usage,{inputTokens:13,outputTokens:5,costUsd:null});}finally{await s.close();}
});
test('real OpenAI Responses SDK protocol records response completion usage',async()=>{
  const s=await server(async(req,res)=>{
    assert.equal(req.url,'/v1/responses');const chunks=[];for await(const c of req)chunks.push(c);const body=JSON.parse(Buffer.concat(chunks));assert.ok(!body.tools?.length);
    res.writeHead(200,{'Content-Type':'text/event-stream'});
    const item={id:'msg_synthetic',type:'message',role:'assistant',status:'completed',content:[{type:'output_text',text:'{"ok":true}',annotations:[]}]};
    const events=[{type:'response.created',response:{id:'resp_synthetic'}},{type:'response.output_item.added',output_index:0,item:{...item,content:[]}},{type:'response.output_text.delta',output_index:0,delta:'{"ok":true}'},{type:'response.output_item.done',output_index:0,item},{type:'response.completed',response:{id:'resp_synthetic',status:'completed',output:[item],usage:{input_tokens:12,output_tokens:4,total_tokens:16,input_tokens_details:{cached_tokens:0},output_tokens_details:{reasoning_tokens:0}}}}];
    for(const e of events)res.write(`event: ${e.type}\ndata: ${JSON.stringify(e)}\n\n`);res.end();
  });
  try{const r=await execute(request(s.url,{api:'openai-responses'}));assert.equal(r.ok,true);assert.equal(r.text,'{"ok":true}');assert.deepEqual(r.usage,{inputTokens:12,outputTokens:4,costUsd:null});}finally{await s.close();}
});
test('invalid request and unsafe endpoint are rejected before any provider call',async()=>{
  assert.equal((await execute(null)).failureCode,'invalid_request');
  for(const baseUrl of ['http://example.invalid/v1','https://example.invalid/v1','https://user:secret@example.invalid/v1','https://example.invalid/v1?token=secret'])assert.equal((await execute(request(baseUrl))).ok,false);
});

test('provider credential echoes are rejected before model or catalog output is returned',async()=>{
  const s=await server((req,res)=>{if(req.url.endsWith('/models')){res.writeHead(200,{'Content-Type':'application/json'});res.end(JSON.stringify({data:[{id:'synthetic-credential-only'}]}));}else success(res,'{"label":"synthetic-credential-only"}');});
  try{for(const operation of ['probe','discover']){const r=await execute(request(s.url,{operation}));assert.equal(r.failureCode,'secret_echo_rejected');assert.ok(!JSON.stringify(r).includes('synthetic-credential-only'));}}finally{await s.close();}
});

test('model calls work without a model catalog and endpoint failures remain actionable',async()=>{
  const seen=[];
  const s=await server((req,res)=>{seen.push(req.url);if(req.url==='/v1/chat/completions')success(res);else{res.writeHead(404);res.end('not found');}});
  try {
    assert.equal((await execute(request(s.url,{operation:'discover'}))).failureCode,'catalog_unavailable');
    assert.equal((await execute(request(s.url))).ok,true);
    assert.equal((await execute(request(s.url+'/wrong'))).failureCode,'provider_endpoint_not_found');
    assert.deepEqual(seen,['/v1/models','/v1/chat/completions','/v1/wrong/chat/completions']);
  }finally{await s.close();}
});

test('embedding batches preserve sparse positional identity and bounded twenty inputs',async()=>{
  const s=await server(async(req,res)=>{
    const chunks=[];for await(const c of req)chunks.push(c);
    const body=JSON.parse(Buffer.concat(chunks));assert.equal(body.input.length,20);
    res.writeHead(200,{'Content-Type':'application/json'});
    res.end(JSON.stringify({data:[{index:19,embedding:[1,2]},{index:0,embedding:[3,4]},{index:5,embedding:[0,0]}],usage:{prompt_tokens:40}}));
  });
  try {
    const result=await execute(request(s.url,{operation:'embed',prompt:JSON.stringify(Array.from({length:20},(_,i)=>`synthetic ${i}`))}));
    assert.equal(result.ok,true);const out=JSON.parse(result.text);
    assert.equal(out.vectors.length,20);assert.deepEqual(out.vectors[19],[1,2]);assert.deepEqual(out.vectors[0],[3,4]);assert.equal(out.vectors[1],null);assert.equal(out.vectors[5],null);assert.equal(out.rejected.length,18);
    assert.equal(result.usage.inputTokens,40);
  } finally {await s.close();}
});
test('embedding duplicate and out of range identities never misassociate vectors',async()=>{
  for(const data of [[{index:0,embedding:[1,2]},{index:0,embedding:[2,1]}],[{index:2,embedding:[1,2]}]]) {
    const s=await server((req,res)=>{res.writeHead(200,{'Content-Type':'application/json'});res.end(JSON.stringify({data,usage:{prompt_tokens:4}}));});
    try {const result=await execute(request(s.url,{operation:'embed',prompt:'["one","two"]'}));assert.equal(result.ok,false);assert.equal(result.failureCode,'invalid_embedding_identity');assert.equal(result.usage.inputTokens,4);}finally{await s.close();}
  }
});
