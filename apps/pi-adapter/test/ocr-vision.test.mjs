import test from 'node:test';import assert from 'node:assert/strict';import http from 'node:http';
import {execute,VERSION,estimateTopicInputTokens} from '../src/adapter.mjs';import {LIMITS} from '../src/execution.mjs';import {capabilityFacts} from './request-facts.mjs';
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
  assert.deepEqual(user.content.filter(block=>block.type==='text').map(block=>block.text),[request.prompt]);
  assert.equal((await execute({...request,modelId:'synthetic-length'})).failureCode,'output_limit');
  const before=bodies.length;for(const change of [{images:undefined},{images:[]},{images:[...request.images,...request.images]},{execution:undefined},{images:[{mimeType:'image/jpeg',data:png}]}])assert.equal((await execute({...request,...change})).failureCode,'invalid_request');assert.equal(bodies.length,before);
 }finally{await new Promise(resolve=>server.close(resolve));}
});
function syntheticImages(count) {
 const formats=[['image/png',[137,80,78,71,13,10,26,10]],['image/jpeg',[255,216,255]],['image/webp',[82,73,70,70,0,0,0,0,87,69,66,80]]];
 return Array.from({length:count},(_,index)=>{const [mimeType,magic]=formats[index%formats.length];return {mimeType,data:Buffer.from([...magic,index]).toString('base64')};});
}
async function analyzeFixture(run,api='openai-completions') {
 const bodies=[];const server=http.createServer(async(req,res)=>{
  const chunks=[];for await(const chunk of req)chunks.push(chunk);bodies.push(JSON.parse(Buffer.concat(chunks)));
  res.writeHead(200,{'Content-Type':'text/event-stream'});
  if(api==='openai-responses'){
   const item={id:'msg_synthetic',type:'message',role:'assistant',status:'completed',content:[{type:'output_text',text:'{"ok":true}',annotations:[]}]};
   const events=[{type:'response.created',response:{id:'resp_synthetic'}},{type:'response.output_item.added',output_index:0,item:{...item,content:[]}},{type:'response.output_text.delta',output_index:0,delta:'{"ok":true}'},{type:'response.output_item.done',output_index:0,item},{type:'response.completed',response:{id:'resp_synthetic',status:'completed',output:[item]}}];
   for(const event of events)res.write(`event: ${event.type}\ndata: ${JSON.stringify(event)}\n\n`);
   res.end();
  }else{
   const chunk=(delta,finish_reason=null)=>({id:'synthetic-analysis',object:'chat.completion.chunk',choices:[{index:0,delta,finish_reason}]});
   res.write(`data: ${JSON.stringify(chunk({role:'assistant',content:'{"ok":true}'}))}\n\n`);
   res.write(`data: ${JSON.stringify(chunk({},'stop'))}\n\n`);res.end('data: [DONE]\n\n');
  }
 });
 await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
 const request={version:VERSION,api,operation:'analyze',baseUrl:`http://127.0.0.1:${server.address().port}/v1`,localEndpoint:true,
  apiKey:'SYNTHETIC-NOT-A-CREDENTIAL',modelId:'synthetic-vision',execution:capabilityFacts(api,{outputMode:'json_object'}),
  timeoutMs:3000,maxOutputTokens:1024,system:'Synthetic image analysis; return JSON.',prompt:'Preserve all supplied images in order.',images:syntheticImages(11)};
 try{await run(request,bodies);}finally{await new Promise(resolve=>server.close(resolve));}
}
for(const api of ['openai-completions','openai-responses'])for(const count of [1,3,11])test(`${api} analyze preserves ${count} image(s) with adjacent citation IDs and exact bytes on the loopback wire`,async()=>{
 await analyzeFixture(async(request,bodies)=>{
  const images=syntheticImages(count),result=await execute({...request,images});
  assert.equal(result.ok,true,JSON.stringify(result));assert.equal(bodies.length,1);
  const content=(api==='openai-responses'?bodies[0].input:bodies[0].messages).find(message=>message.role==='user').content;
  const wireUrls=content.filter(block=>['image_url','input_image'].includes(block.type)).map(block=>api==='openai-responses'?block.image_url:block.image_url.url);
  assert.deepEqual(wireUrls,images.map(image=>`data:${image.mimeType};base64,${image.data}`));
  assert.deepEqual(wireUrls.map(url=>Buffer.from(url.split(',')[1],'base64')),images.map(image=>Buffer.from(image.data,'base64')));
  assert.equal(content.length,1+count*2);
  assert.equal(content[0].text,request.prompt);
  for(let index=0;index<count;index++){
   assert.equal(content[1+index*2].type,api==='openai-responses'?'input_text':'text');
   assert.ok(content[1+index*2].text.startsWith(`inputCitationId=image_${index+1};`));
   const image=content[2+index*2];
   assert.equal(api==='openai-responses'?image.image_url:image.image_url.url,`data:${images[index].mimeType};base64,${images[index].data}`);
  }
  assert.ok(!bodies[0].tools?.length);assert.equal(result.usage.inputTokens,null,'image reserve is not reported provider usage');
 },api);
});
test('analyze validates every image, including an invalid eleventh image',async()=>{
 await analyzeFixture(async(request,bodies)=>{
  const oversized=Buffer.alloc(8_000_001);Buffer.from(png,'base64').copy(oversized);
  for(const image of [null,{mimeType:'image/png',data:1},{mimeType:'image/gif',data:png},
   {mimeType:'image/jpeg',data:png},{mimeType:'image/png',data:'!'+png},
   {mimeType:'image/png',data:Buffer.from([137,80,78,71,13,10,26,10]).toString('base64').replace(/=$/,'')},
   {mimeType:'image/png',data:oversized.toString('base64')},{mimeType:'image/png',data:'A'.repeat(10666669)}]){
   const result=await execute({...request,images:[...request.images.slice(0,10),image]});
   assert.equal(result.failureCode,'invalid_request');
  }
  assert.equal(bodies.length,0);
 });
});
test('image inputs require a nonempty list, qualified execution and an allowed operation',async()=>{
 await analyzeFixture(async(request,bodies)=>{
  for(const change of [{images:[]},{images:null},{images:{}},{execution:undefined},{execution:null},{execution:{}},
   ...['connect','discover','probe','embed'].map(operation=>({operation})),
   {images:syntheticImages(1),execution:undefined}]){
   assert.equal((await execute({...request,...change})).failureCode,'invalid_request');
  }
  assert.equal(bodies.length,0);
  assert.equal((await execute({...request,images:undefined})).ok,true,'text-only analyze stays supported');
  assert.equal(bodies.length,1);
  assert.deepEqual(bodies[0].messages.find(message=>message.role==='user').content,[{type:'text',text:request.prompt}]);
 });
});
test('input reserve counts every image and honors the exact input and combined context ceilings',async()=>{
 await analyzeFixture(async(request,bodies)=>{
  for(const count of [1,11]){
   const images=syntheticImages(count),inputTokenLimit=estimateTopicInputTokens(request.system,request.prompt)+count*1024;
   const execution={...request.execution,inputTokenLimit,limits:{...request.execution.limits,maxOutputTokens:request.maxOutputTokens,contextTokens:inputTokenLimit+request.maxOutputTokens}};
   const before=bodies.length;
   assert.equal((await execute({...request,images,execution:{...execution,inputTokenLimit:inputTokenLimit-1}})).failureCode,'model_input_limit');
   assert.equal((await execute({...request,images,execution:{...execution,limits:{...execution.limits,contextTokens:execution.limits.contextTokens-1}}})).failureCode,'invalid_request');
   assert.equal(bodies.length,before);
   const result=await execute({...request,images,execution});
   assert.equal(result.ok,true,JSON.stringify(result));assert.equal(result.usage.inputTokens,null);assert.equal(bodies.length,before+1);
  }
 });
});
test('multiple images obey the shared total request byte ceiling without truncation',async()=>{
 await analyzeFixture(async(request,bodies)=>{
  const overhead=Buffer.byteLength(JSON.stringify(request))-request.images.reduce((sum,image)=>sum+image.data.length,0);
  const bytesPerImage=3*Math.floor((LIMITS.maxRequestBytes-overhead)/(request.images.length*4));
  request.images=request.images.map(()=>{const bytes=Buffer.alloc(bytesPerImage);Buffer.from(png,'base64').copy(bytes);return {mimeType:'image/png',data:bytes.toString('base64')};});
  request.prompt+='x'.repeat(LIMITS.maxRequestBytes-Buffer.byteLength(JSON.stringify(request)));
  assert.equal(Buffer.byteLength(JSON.stringify(request)),LIMITS.maxRequestBytes);
  const result=await execute(request);assert.equal(result.ok,true,JSON.stringify(result));assert.equal(bodies.length,1);
  const urls=bodies[0].messages.find(message=>message.role==='user').content.filter(block=>block.type==='image_url').map(block=>block.image_url.url);
  assert.deepEqual(urls,request.images.map(image=>`data:${image.mimeType};base64,${image.data}`));
  assert.equal((await execute({...request,prompt:request.prompt+'x'})).failureCode,'invalid_request');assert.equal(bodies.length,1);
 });
});
