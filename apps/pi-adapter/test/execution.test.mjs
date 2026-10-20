import test from 'node:test';
import assert from 'node:assert/strict';
import http from 'node:http';
import { execute, VERSION } from '../src/adapter.mjs';
import { LIMITS, PROTOCOL, validCompatibility } from '../src/execution.mjs';
import { capabilityFacts } from './request-facts.mjs';
const PRIVATE_THOUGHT = 'SYNTHETIC_PRIVATE_THINKING_NOT_A_BUSINESS_RESULT';
const responseSchema = {name:'synthetic_result_v1',schema:{type:'object',properties:{ok:{type:'boolean'}},required:['ok'],additionalProperties:false}};
function completion(res, text='{"ok":true}', thought=false) {
  res.writeHead(200,{'Content-Type':'text/event-stream'});
  const chunk=(delta,finish_reason=null)=>({id:'synthetic',object:'chat.completion.chunk',choices:[{index:0,delta,finish_reason}]});
  if(thought)res.write(`data: ${JSON.stringify(chunk({role:'assistant',reasoning_content:PRIVATE_THOUGHT}))}\n\n`);
  res.write(`data: ${JSON.stringify(chunk({role:'assistant',content:text}))}\n\n`);
  res.write(`data: ${JSON.stringify(chunk({},'stop'))}\n\n`);
  res.write(`data: ${JSON.stringify({choices:[],usage:{prompt_tokens:100,completion_tokens:40,prompt_tokens_details:{cached_tokens:30,cache_write_tokens:0},completion_tokens_details:{reasoning_tokens:thought?25:0}}})}\n\n`);
  res.end('data: [DONE]\n\n');
}
function anthropic(res) {
  res.writeHead(200,{'Content-Type':'text/event-stream'});
  const events=[{type:'message_start',message:{id:'msg_synthetic',model:'unknown-custom-model',role:'assistant',type:'message',content:[],usage:{input_tokens:70,output_tokens:0,cache_read_input_tokens:30,cache_creation_input_tokens:0}}},
    {type:'content_block_start',index:0,content_block:{type:'thinking',thinking:'',signature:''}},
    {type:'content_block_delta',index:0,delta:{type:'thinking_delta',thinking:PRIVATE_THOUGHT}},
    {type:'content_block_stop',index:0},{type:'content_block_start',index:1,content_block:{type:'text',text:''}},
    {type:'content_block_delta',index:1,delta:{type:'text_delta',text:'{"ok":true}'}},
    {type:'content_block_stop',index:1},{type:'message_delta',delta:{stop_reason:'end_turn'},usage:{output_tokens:40}},{type:'message_stop'}];
  for(const event of events)res.write(`event: ${event.type}\ndata: ${JSON.stringify(event)}\n\n`);
  res.end();
}
async function fixture(run, send=completion) {
  const bodies=[];
  const server=http.createServer(async(req,res)=>{
    const chunks=[];for await(const chunk of req)chunks.push(chunk);
    if(req.headers.authorization)assert.equal(req.headers.authorization,'Bearer SYNTHETIC-NOT-A-CREDENTIAL');
    else assert.equal(req.headers['x-api-key'],'SYNTHETIC-NOT-A-CREDENTIAL');
    bodies.push(JSON.parse(Buffer.concat(chunks)));send(res);
  });
  await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
  const baseUrl=`http://127.0.0.1:${server.address().port}/v1`;
  const r={version:VERSION,operation:'analyze',api:'openai-completions',baseUrl,localEndpoint:true,
    apiKey:'SYNTHETIC-NOT-A-CREDENTIAL',modelId:'unknown-custom-model',timeoutMs:3000,maxOutputTokens:256,
    system:'Synthetic / not evidence.',prompt:'Return final JSON.',execution:capabilityFacts('openai-completions',{outputMode:'prompt_only'})};
  try{await run(r,bodies);}finally{await new Promise(resolve=>server.close(resolve));}
}
test('a frozen capability supports large input, output and timeout without the historical caps',async()=>{
  const output='材'.repeat(25_000);
  await fixture(async(r,bodies)=>{
    const result=await execute({...r,prompt:'材'.repeat(100_000),maxOutputTokens:384_000,timeoutMs:3_600_000,
      execution:capabilityFacts(r.api,{inputTokenLimit:600_000,outputMode:'prompt_only'})});
    assert.equal(result.ok,true,JSON.stringify(result));
    assert.ok(result.text===output);
    assert.ok(Buffer.byteLength(result.text)>65536);
    assert.equal(bodies.length,1);
    assert.equal(bodies[0].max_tokens,384_000);
    assert.ok(Buffer.byteLength(JSON.stringify(bodies[0]))>131072);
    assert.equal(result.usage.costUsd,null);
  },res=>completion(res,output));
});
test('compatibility, thinking maps and structured output apply to an unknown model and local endpoint',async()=>{
  await fixture(async(r,bodies)=>{
    const result=await execute({...r,execution:capabilityFacts(r.api,{thinkingLevel:'max',thinkingLevelMap:{off:'none',max:'vendor_max',high:'high'},
      compat:{thinkingFormat:'deepseek',supportsReasoningEffort:true,maxTokensField:'max_completion_tokens'},outputMode:'json_schema',strictSchema:true,responseSchema})});
    assert.equal(result.ok,true,JSON.stringify(result));
    assert.equal(bodies[0].max_completion_tokens,r.maxOutputTokens);
    assert.equal(bodies[0].max_tokens,undefined);
    assert.deepEqual(bodies[0].thinking,{type:'enabled'});
    assert.equal(bodies[0].reasoning_effort,'vendor_max');
    assert.deepEqual(bodies[0].response_format,{type:'json_schema',json_schema:{...responseSchema,strict:true}});
    assert.equal(result.text,'{"ok":true}');
    assert.ok(!JSON.stringify(result).includes(PRIVATE_THOUGHT));
    assert.equal(result.usage.inputTokens+result.usage.outputTokens,140);
    assert.equal(result.usage.cachedInputTokens,30);
    assert.equal(result.usage.reasoningTokens,25);
  },res=>completion(res,'{"ok":true}',true));
});
test('invalid capabilities, unsupported bounds and unknown metadata fail before any HTTP request',async()=>{
  await fixture(async(r,bodies)=>{
    const base=r.execution;
    for(const change of [
      {execution:null,maxOutputTokens:8193},
      {execution:{...base,inputTokenLimit:900_000},maxOutputTokens:384_000},
      {execution:{...base,thinkingLevel:'low'}},
      {execution:{...base,thinkingLevelMap:{off:null}}},
      {execution:{...base,compat:{headers:{Authorization:'not-allowed'}}}},
      {execution:{...base,compat:{openRouterRouting:{only:['other']}}}},
      {execution:{...base,limits:{...base.limits,contextTokens:LIMITS.maxContextTokens+1}}},
      {api:'openai-responses',execution:capabilityFacts('openai-responses',{compat:{supportsMaxOutputTokens:false},outputMode:'prompt_only'})},
    ])assert.equal((await execute({...r,...change})).failureCode,'invalid_request');
    assert.equal(bodies.length,0);
    assert.equal((await execute({...r,prompt:'材'.repeat(2000),execution:{...base,inputTokenLimit:1024}})).failureCode,'model_input_limit');
    assert.equal(bodies.length,0);
  });
});
test('off does not silently accept a provider that returns thinking content',async()=>{
  await fixture(async(r,bodies)=>{
    const result=await execute(r);
    assert.equal(result.failureCode,'unexpected_content');
    assert.equal(result.text,undefined);
    assert.equal(result.usage.outputTokens,40);
    assert.equal(bodies.length,1);
    assert.ok(!JSON.stringify(result).includes(PRIVATE_THOUGHT));
  },res=>completion(res,'{"ok":true}',true));
});
test('Anthropic thinking stays inside the output ceiling and native schema preserves adaptive effort',async()=>{
  await fixture(async(r,bodies)=>{
    const api='anthropic-messages',baseUrl=r.baseUrl.replace(/\/v1$/,'');
    for(const adaptive of [false,true]){
      const result=await execute({...r,api,baseUrl,maxOutputTokens:4096,execution:capabilityFacts(api,{thinkingLevel:'high',
        compat:{forceAdaptiveThinking:adaptive},outputMode:adaptive?'json_schema':'prompt_only',strictSchema:adaptive,
        ...(adaptive?{responseSchema}:{})})});
      assert.equal(result.ok,true,JSON.stringify(result));
      const body=bodies.at(-1);
      assert.equal(body.max_tokens,4096,'SDK thinking allowance cannot enlarge the frozen output ceiling');
      if(adaptive){
        assert.equal(body.thinking.type,'adaptive');
        assert.equal(body.output_config.effort,'high');
        assert.deepEqual(body.output_config.format,{type:'json_schema',schema:responseSchema.schema});
      }else{
        assert.equal(body.thinking.type,'enabled');
        assert.ok(body.thinking.budget_tokens>=1024&&body.thinking.budget_tokens<=3072);
      }
      assert.ok(!JSON.stringify(result).includes(PRIVATE_THOUGHT));
      assert.equal(result.usage.inputTokens,100);
    }
    const invalid=await execute({...r,api,baseUrl,maxOutputTokens:1024,execution:capabilityFacts(api,{thinkingLevel:'high',outputMode:'prompt_only'})});
    assert.equal(invalid.failureCode,'invalid_request');
    assert.equal(bodies.length,2);
  },anthropic);
});
test('pinned SDK metadata vocabulary excludes route and header injection',()=>{
  assert.equal(PROTOCOL.sdkVersion,'0.85.1');
  assert.ok(validCompatibility('openai-completions',{thinkingFormat:'chat-template',chatTemplateKwargs:{enable_thinking:{$var:'thinking.enabled'}}}));
  for(const invalid of [{headers:{}},{openRouterRouting:{}},{chatTemplateKwargs:{__proto__:true,constructor:true}},{supportsStore:'true'},{thinkingFormat:'future-unchecked'}])assert.equal(validCompatibility('openai-completions',invalid),false);
});
