import test from 'node:test';
import assert from 'node:assert/strict';
import http from 'node:http';
import { execute, VERSION } from '../src/adapter.mjs';

// Small synthetic schema tests the wire boundary. The Rust packet owns the real schema.
const schema={type:'object',properties:{comments:{type:'array',items:{type:'object',properties:{commentRef:{type:'string'}},required:['commentRef'],additionalProperties:false}}},required:['comments'],additionalProperties:false};
const prompt=JSON.stringify({contract:'comment-research.v3',task:'Return JSON.',outputSchema:schema,untrustedMaterial:{comments:[]}});
const request={version:VERSION,operation:'analyze',api:'openai-responses',baseUrl:'https://api.deepseek.com/v1',apiKey:'SYNTHETIC-NOT-A-CREDENTIAL',modelId:'deepseek-v4-flash',timeoutMs:3000,maxOutputTokens:1024,system:'SYNTHETIC / NOT EVIDENCE. Return JSON only.',prompt};

function complete(res,api,text='{"comments":[]}') {
  res.writeHead(200,{'Content-Type':'text/event-stream'});
  if(api==='openai-completions') {
    const chunk={id:'synthetic',object:'chat.completion.chunk',created:1,model:'synthetic',choices:[{index:0,delta:{role:'assistant',content:text},finish_reason:null}]};
    res.write(`data: ${JSON.stringify(chunk)}\n\n`);
    chunk.choices[0]={index:0,delta:{},finish_reason:'stop'};
    res.write(`data: ${JSON.stringify(chunk)}\n\n`);res.end('data: [DONE]\n\n');return;
  }
  const item={id:'msg_synthetic',type:'message',role:'assistant',status:'completed',content:[{type:'output_text',text,annotations:[]}]};
  const events=[{type:'response.created',response:{id:'resp_synthetic'}},{type:'response.output_item.added',output_index:0,item:{...item,content:[]}},{type:'response.output_text.delta',output_index:0,delta:text},{type:'response.output_item.done',output_index:0,item},{type:'response.completed',response:{id:'resp_synthetic',status:'completed',output:[item],usage:{input_tokens:20,output_tokens:5,total_tokens:25,input_tokens_details:{cached_tokens:0},output_tokens_details:{reasoning_tokens:0}}}}];
  for(const event of events)res.write(`event: ${event.type}\ndata: ${JSON.stringify(event)}\n\n`);
  res.end();
}
async function fixture(run,{status=200,text}={}) {
  const bodies=[];
  const server=http.createServer(async(req,res)=>{
    const chunks=[];for await(const c of req)chunks.push(c);
    const body=JSON.parse(Buffer.concat(chunks));bodies.push(body);
    if(status!==200){res.writeHead(status);res.end('SYNTHETIC-NOT-A-CREDENTIAL');return;}
    complete(res,req.url.endsWith('/chat/completions')?'openai-completions':'openai-responses',text);
  });
  await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
  const originalFetch=globalThis.fetch;
  globalThis.fetch=(input,init)=>{
    const url=new URL(typeof input==='string'||input instanceof URL?input:input.url);
    assert.ok(['api.deepseek.com','api.openai.com','api.deepseek.com.example.invalid'].includes(url.hostname));
    return originalFetch(`http://127.0.0.1:${server.address().port}${url.pathname}`,init);
  };
  try{await run(bodies);}finally{globalThis.fetch=originalFetch;await new Promise(resolve=>server.close(resolve));}
}
test('actual SDK sends DeepSeek Responses schema and chat JSON mode for probe and analysis',async()=>{
  await fixture(async bodies=>{
    for(const api of ['openai-responses','openai-completions'])for(const operation of ['probe','analyze']){
      assert.equal((await execute({...request,api,operation})).ok,true);
      const body=bodies.at(-1);
      if(api==='openai-responses'){
        assert.deepEqual(body.text.format,{type:'json_schema',name:'comment_research_v3',schema});
        assert.deepEqual(body.reasoning,{effort:'none'});
      }else{
        assert.deepEqual(body.response_format,{type:'json_object'});
        assert.deepEqual(body.thinking,{type:'disabled'});
      }
      assert.ok(!body.tools?.length);
    }
    assert.equal(bodies.length,4);
  });
});
test('official supported OpenAI models use strict schema in each documented protocol',async()=>{
  await fixture(async bodies=>{
    for(const api of ['openai-responses','openai-completions']){
      assert.equal((await execute({...request,baseUrl:'https://api.openai.com/v1',modelId:'gpt-4o-mini',api})).ok,true);
      const body=bodies.at(-1);
      if(api==='openai-responses')assert.deepEqual(body.text.format,{type:'json_schema',name:'comment_research_v3',strict:true,schema});
      else assert.deepEqual(body.response_format,{type:'json_schema',json_schema:{name:'comment_research_v3',strict:true,schema}});
    }
  });
});
test('unknown host, path, model and unrelated prompts do not inherit structured capability',async()=>{
  await fixture(async bodies=>{
    for(const change of [
      {baseUrl:'https://api.deepseek.com.example.invalid/v1'},
      {baseUrl:'https://api.deepseek.com/custom'},
      {modelId:'future-unknown-model'},
      {baseUrl:'https://api.openai.com/v1',modelId:'unknown-model'},
      {prompt:'Return JSON.'},
      {prompt:JSON.stringify({contract:'another-contract',untrustedMaterial:{contract:'comment-research.v3',outputSchema:schema}})},
    ]){
      assert.equal((await execute({...request,...change})).ok,true);
      assert.equal(bodies.at(-1).text?.format,undefined);
      assert.equal(bodies.at(-1).response_format,undefined);
    }
  });
});
test('invalid server schema is rejected before transport; provider schema rejection is not retried',async()=>{
  await fixture(async bodies=>{
    assert.equal((await execute({...request,prompt:JSON.stringify({contract:'comment-research.v3',outputSchema:{type:'array'}})})).failureCode,'invalid_request');
    assert.equal(bodies.length,0);
  });
  for(const [status,code] of [[400,'provider_request_rejected'],[422,'provider_request_rejected'],[503,'provider_unavailable']]){
    await fixture(async bodies=>{
      const result=await execute(request);
      assert.equal(result.ok,false);
      assert.equal(result.failureCode,code);
      assert.equal(bodies.length,1);
      assert.ok(!JSON.stringify(result).includes('SYNTHETIC-NOT-A-CREDENTIAL'));
    },{status});
  }
});
test('transport never repairs malformed JSON or strips wrappers before business validation',async()=>{
  const malformed='```json\n{"comments":\n```';
  await fixture(async()=>{
    const result=await execute(request);
    assert.equal(result.ok,true);
    assert.equal(result.text,malformed);
  },{text:malformed});
});
