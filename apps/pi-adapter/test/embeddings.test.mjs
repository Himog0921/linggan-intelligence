import test from 'node:test';
import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {execute,VERSION} from '../src/adapter.mjs';
const request=(baseUrl)=>({version:VERSION,operation:'embed',api:'openai-completions',baseUrl,localEndpoint:true,apiKey:'SYNTHETIC-NOT-A-CREDENTIAL',modelId:'synthetic-embedding',timeoutMs:2000,maxOutputTokens:16,system:'',prompt:JSON.stringify(['合成样本 A','合成样本 B'])});
async function serve(payload,fn){const server=createServer(async(req,res)=>{let body='';for await(const chunk of req)body+=chunk;assert.equal(req.url,'/v1/embeddings');assert.equal(JSON.parse(body).encoding_format,'float');res.setHeader('Content-Type','application/json');res.end(JSON.stringify(payload));});await new Promise(r=>server.listen(0,'127.0.0.1',r));try{await fn(`http://127.0.0.1:${server.address().port}/v1`)}finally{await new Promise(r=>server.close(r));}}
test('embedding protocol preserves index order and provider input usage',async()=>{await serve({data:[{index:1,embedding:[0,1,0]},{index:0,embedding:[1,0,0]}],usage:{prompt_tokens:9}},async url=>{const r=await execute(request(url));assert.equal(r.ok,true);assert.deepEqual(JSON.parse(r.text),{vectors:[[1,0,0],[0,1,0]],dimensions:3,rejected:[]});assert.equal(r.usage.inputTokens,9);assert.equal(r.usage.outputTokens,0);});});
test('ambiguous duplicate indices reject the entire response without losing usage',async()=>{
  await serve({data:[{index:0,embedding:[1]},{index:0,embedding:[2]}],usage:{prompt_tokens:7}},async url=>{
    const r=await execute(request(url));assert.equal(r.ok,false);assert.equal(r.failureCode,'invalid_embedding_identity');assert.equal(r.usage.inputTokens,7);
  });
});
test('missing and zero-vector members retain positional rejection while valid members survive',async()=>{
  for(const [data,vectors,rejected] of [
    [[{index:0,embedding:[1]}],[[1],null],[{index:1,code:'missing_vector'}]],
    [[{index:0,embedding:[0]},{index:1,embedding:[1]}],[null,[1]],[{index:0,code:'invalid_vector'}]]
  ])await serve({data,usage:{prompt_tokens:7}},async url=>{
    const r=await execute(request(url));assert.equal(r.ok,true);const output=JSON.parse(r.text);
    assert.deepEqual(output.vectors,vectors);assert.deepEqual(output.rejected,rejected);assert.equal(r.usage.inputTokens,7);
  });
});
test('missing usage remains unknown',async()=>{await serve({data:[{index:0,embedding:[1]},{index:1,embedding:[2]}]},async url=>{const r=await execute(request(url));assert.equal(r.ok,true);assert.equal(r.usage.inputTokens,null);});});
