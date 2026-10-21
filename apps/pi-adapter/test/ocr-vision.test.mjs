import test from 'node:test';import assert from 'node:assert/strict';import http from 'node:http';
import {execute,VERSION} from '../src/adapter.mjs';import {capabilityFacts} from './request-facts.mjs';
// Generated/synthetic PNG bytes only; no real image or external endpoint.
const png=Buffer.from([137,80,78,71,13,10,26,10,0,0,0,0]).toString('base64');
test('qualified image reaches pinned SDK as data URI, without a local answer or tools',async()=>{
 const bodies=[];const server=http.createServer(async(req,res)=>{const chunks=[];for await(const c of req)chunks.push(c);const b=JSON.parse(Buffer.concat(chunks));bodies.push(b);
 res.writeHead(200,{'Content-Type':'text/event-stream'});const chunk=(delta,finish_reason=null)=>({id:'synthetic-vision',object:'chat.completion.chunk',created:1,model:b.model,choices:[{index:0,delta,finish_reason}]});
 res.write(`data: ${JSON.stringify(chunk({role:'assistant',content:'{"titlePresence":"absent","titleText":null,"regions":[],"unreadable":false}'}))}\n\n`);res.write(`data: ${JSON.stringify(chunk({},b.model==='synthetic-length'?'length':'stop'))}\n\n`);res.end('data: [DONE]\n\n');});
 await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
 const request={version:VERSION,api:'openai-completions',operation:'analyze_image',baseUrl:`http://127.0.0.1:${server.address().port}/v1`,localEndpoint:true,apiKey:'SYNTHETIC-NOT-A-CREDENTIAL',modelId:'synthetic-vision',execution:capabilityFacts('openai-completions',{outputMode:'json_object'}),timeoutMs:3000,maxOutputTokens:1024,system:'Read visible image only and return json.',prompt:'{"contract":"ocr-visual-audit.v1"}',images:[{mimeType:'image/png',data:png}]};
 try{
  const result=await execute(request);assert.equal(result.ok,true,JSON.stringify(result));assert.equal(result.usage.inputTokens,null);
  const user=bodies[0].messages.find(m=>m.role==='user');assert.equal(user.content.filter(b=>b.type==='image_url').length,1);assert.equal(user.content.find(b=>b.type==='image_url').image_url.url,`data:image/png;base64,${png}`);assert.equal(bodies[0].response_format.type,'json_object');assert.ok(!bodies[0].tools?.length);assert.equal(bodies[0].messages.some(m=>JSON.stringify(m).includes('LOCAL-OCR-ANSWER')),false);
  assert.equal((await execute({...request,modelId:'synthetic-length'})).failureCode,'output_limit');
  const before=bodies.length;assert.equal((await execute({...request,operation:'analyze'})).failureCode,'invalid_request');assert.equal((await execute({...request,images:[{mimeType:'image/jpeg',data:png}]})).failureCode,'invalid_request');assert.equal(bodies.length,before);
 }finally{await new Promise(resolve=>server.close(resolve));}
});
