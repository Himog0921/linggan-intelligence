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
  try{const result=await execute(request(s.url));assert.equal(result.ok,true);assert.equal(result.text,'{"ok":true}');assert.deepEqual(result.usage,{inputTokens:17,outputTokens:5,costUsd:null});assert.equal(calls,1);}finally{await s.close();}
});
test('omitted provider usage remains unknown and catalog is an account endpoint list',async()=>{
  const s=await server((req,res)=>{if(req.url.endsWith('/models')){res.writeHead(200,{'Content-Type':'application/json'});res.end(JSON.stringify({data:[{id:'synthetic-model'}]}));}else success(res,'{"ok":true}',false);});
  try{const d=await execute(request(s.url,{operation:'discover'}));assert.deepEqual(d.modelIds,['synthetic-model']);assert.equal(d.modelListOrigin,'ACCOUNT_ENDPOINT');const r=await execute(request(s.url));assert.equal(r.usage.inputTokens,null);assert.equal(r.usage.costUsd,null);}finally{await s.close();}
});
test('provider errors and redirects do not leak credentials or retry internally',async()=>{
  let calls=0;const s=await server((req,res)=>{calls++;res.writeHead(401);res.end('synthetic-credential-only raw provider error');});
  try{const result=await execute(request(s.url));assert.equal(result.ok,false);assert.ok(!JSON.stringify(result).includes('synthetic-credential-only'));assert.equal(calls,1);}finally{await s.close();}
  const redirect=await server((req,res)=>{res.writeHead(302,{location:'https://example.invalid/leak'});res.end();});
  try{assert.equal((await execute(request(redirect.url))).ok,false);}finally{await redirect.close();}
});
test('an unending provider stream is locally aborted',async()=>{
  const s=await server((req,res)=>{res.writeHead(200,{'Content-Type':'text/event-stream'});res.write(':waiting\n\n');});
  try{const started=Date.now();const r=await execute(request(s.url,{timeoutMs:100}));assert.equal(r.ok,false);assert.equal(r.failureCode,'provider_timeout');assert.ok(Date.now()-started<1500);}finally{await s.close();}
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
