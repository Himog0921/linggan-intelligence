import test from 'node:test';
import assert from 'node:assert/strict';
import http from 'node:http';
import { execute, VERSION } from '../src/adapter.mjs';

// Route the SDK's final HTTP request to a local fixture. Never contact DeepSeek.
test('DeepSeek V4 Responses uses non-thinking mode consistently for probe and analysis', async () => {
  const bodies=[];
  const server=http.createServer(async(req,res)=>{
    const chunks=[];for await(const chunk of req)chunks.push(chunk);
    const body=JSON.parse(Buffer.concat(chunks));bodies.push(body);
    const complete=body.reasoning?.effort==='none';
    res.writeHead(200,{'Content-Type':'text/event-stream'});
    const item={id:'msg_synthetic',type:'message',role:'assistant',status:'completed',content:[{type:'output_text',text:'{"ok":true}',annotations:[]}]};
    const response={id:'resp_synthetic',status:complete?'completed':'incomplete',output:complete?[item]:[],usage:{input_tokens:459,output_tokens:complete?8:1024,total_tokens:complete?467:1483,input_tokens_details:{cached_tokens:0},output_tokens_details:{reasoning_tokens:complete?0:1024}}};
    if(!complete)response.incomplete_details={reason:'max_output_tokens'};
    const events=[{type:'response.created',response:{id:'resp_synthetic'}}];
    if(complete)events.push({type:'response.output_item.added',output_index:0,item:{...item,content:[]}},{type:'response.output_text.delta',output_index:0,delta:'{"ok":true}'},{type:'response.output_item.done',output_index:0,item});
    events.push({type:complete?'response.completed':'response.incomplete',response});
    for(const e of events)res.write(`event: ${e.type}\ndata: ${JSON.stringify(e)}\n\n`);res.end();
  });
  await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
  const originalFetch=globalThis.fetch;
  globalThis.fetch=(input,init)=>{
    const url=new URL(typeof input==='string'||input instanceof URL?input:input.url);
    assert.ok(['api.deepseek.com','api.deepseek.com.example.invalid'].includes(url.hostname));
    return originalFetch(`http://127.0.0.1:${server.address().port}${url.pathname}`,init);
  };
  const request={version:VERSION,api:'openai-responses',baseUrl:'https://api.deepseek.com/v1',apiKey:'SYNTHETIC-NOT-A-CREDENTIAL',modelId:'deepseek-v4-flash',timeoutMs:3000,maxOutputTokens:1024,system:'Synthetic protocol proof.',prompt:'Return one JSON object.'};
  try {
    for(const operation of ['probe','analyze']){
      const result=await execute({...request,operation});
      assert.equal(result.ok,true,JSON.stringify(result));
      assert.equal(result.usage.outputTokens,8);
    }
    const other=await execute({...request,operation:'probe',baseUrl:'https://api.deepseek.com.example.invalid/v1'});
    assert.equal(other.failureCode,'output_limit');
    assert.equal(other.usage.outputTokens,1024);
    const unknown=await execute({...request,operation:'probe',modelId:'unknown-model'});
    assert.equal(unknown.failureCode,'output_limit');
    assert.equal(bodies.length,4);
    assert.equal(bodies[0].max_output_tokens,1024);
    assert.equal(bodies[2].reasoning,undefined);
    assert.equal(bodies[3].reasoning,undefined);
  } finally {
    globalThis.fetch=originalFetch;
    await new Promise(resolve=>server.close(resolve));
  }
});

test('DeepSeek V4 Chat Completions sends the documented thinking toggle',async()=>{
  const bodies=[];
  const server=http.createServer(async(req,res)=>{
    const chunks=[];for await(const c of req)chunks.push(c);bodies.push(JSON.parse(Buffer.concat(chunks)));
    res.writeHead(200,{'Content-Type':'text/event-stream'});
    const chunk={id:'synthetic',object:'chat.completion.chunk',created:1,model:'deepseek-v4-pro',choices:[{index:0,delta:{role:'assistant',content:'{"ok":true}'},finish_reason:null}]};
    res.write(`data: ${JSON.stringify(chunk)}\n\n`);chunk.choices[0]={index:0,delta:{},finish_reason:'stop'};res.write(`data: ${JSON.stringify(chunk)}\n\n`);res.end('data: [DONE]\n\n');
  });
  await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
  const originalFetch=globalThis.fetch;
  globalThis.fetch=(input,init)=>{const url=new URL(typeof input==='string'||input instanceof URL?input:input.url);assert.equal(url.hostname,'api.deepseek.com');return originalFetch(`http://127.0.0.1:${server.address().port}${url.pathname}`,init);};
  try {
    for(const operation of ['probe','analyze'])assert.equal((await execute({version:VERSION,api:'openai-completions',baseUrl:'https://api.deepseek.com/v1',apiKey:'SYNTHETIC-NOT-A-CREDENTIAL',modelId:'deepseek-v4-pro',operation,timeoutMs:3000,maxOutputTokens:1024,system:'Synthetic proof.',prompt:'Return JSON.'})).ok,true);
    assert.equal(bodies.length,2);
    for(const b of bodies){assert.deepEqual(b.thinking,{type:'disabled'});assert.ok(!b.tools?.length);}
  }finally{globalThis.fetch=originalFetch;await new Promise(resolve=>server.close(resolve));}
});
