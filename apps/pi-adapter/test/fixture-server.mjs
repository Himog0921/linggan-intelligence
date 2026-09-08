// Explicit synthetic protocol fixture. Never serves as a production model fallback.
import http from 'node:http';
const server=http.createServer(async(req,res)=>{
  if(req.headers.authorization!=='Bearer SYNTHETIC-NOT-A-CREDENTIAL'&&req.headers['x-api-key']!=='SYNTHETIC-NOT-A-CREDENTIAL'){res.writeHead(401);res.end('synthetic unauthorized');return;}
  if(req.url==='/v1/models'){res.writeHead(200,{'Content-Type':'application/json'});res.end(JSON.stringify({data:['synthetic-good','synthetic-bad','synthetic-no-usage','synthetic-fail'].map(id=>({id}))}));return;}
  const chunks=[];for await(const chunk of req)chunks.push(chunk);
  const body=JSON.parse(Buffer.concat(chunks));
  if(body.model==='synthetic-fail'){res.writeHead(503);res.end('synthetic failure');return;}
  const prompt=body.messages.findLast(m=>m.role==='user')?.content;
  let material,promptPayload;try{promptPayload=JSON.parse(typeof prompt==='string'?prompt:prompt?.find(b=>b.type==='text')?.text);material=promptPayload.untrustedMaterial;}catch{res.writeHead(400);res.end('synthetic prompt missing');return;}
  const packet=Array.isArray(material.comments);
  const actualWork=!packet&&material.workRef!=='00000000-0000-0000-0000-000000000000';
  if(actualWork&&material.body.includes('[FAIL_PROVIDER]')){res.writeHead(503);res.end('synthetic bounded failure');return;}
  if(actualWork&&material.body.includes('[HANG_PROVIDER]')){res.writeHead(200,{'Content-Type':'text/event-stream'});res.write(':waiting\n\n');return;}
  const output=packet?{comments:material.comments.map(c=>({commentRef:c.commentRef,outcome:c.text.includes('[NO_SIGNAL]')?'no_signal':'interpretable',labels:[],problems:c.text.includes('[NO_SIGNAL]')?[]:[{candidateRef:c.text.includes('[ASSIGN_EXISTING]')?(promptPayload.existingProblems?.[0]?.candidateRef??null):null,equivalenceReason:c.text.includes('[ASSIGN_EXISTING]')?'合成定义比较确认执行场景与边界相同':'',boundaryMatch:c.text.includes('[ASSIGN_EXISTING]')&&Boolean(promptPayload.existingProblems?.length),name:'合成执行精力',meaning:'合成执行过程中的精力困难',evidence:[{quote:c.text.includes('[BAD_QUOTE]')?'INVENTED-NOT-IN-SOURCE':c.text}]}],stances:[],contextMissing:[],uncertaintyReason:null,limitations:['SYNTHETIC / NOT EVIDENCE']}))}:{sourceRef:material.sourceRef,sourceSha256:material.sourceSha256,spans:[{sourceRef:material.sourceRef,startChar:0,endChar:Array.from(material.body).length,quote:material.body,
    facets:[{dimension:'problem',label:'合成执行精力',basis:'explicit'}]}],limitations:['SYNTHETIC / NOT EVIDENCE']};
  if(packet)output.comments=output.comments.filter((c,i)=>!material.comments[i].text.includes('[MISSING]')).map(c=>material.comments.find(m=>m.commentRef===c.commentRef).text.includes('[BAD_SCHEMA]')?{...c,labels:[{label:'invented',evidence:[]}]}:c);
  if(packet&&material.comments.some(c=>c.text.includes('[HANG_PROVIDER]'))){res.writeHead(200,{'Content-Type':'text/event-stream'});res.write(':waiting\n\n');return;}
  if(actualWork&&material.body.includes('[NO_SIGNAL]'))output.spans=[];
  if(packet&&material.comments.some(c=>c.text.includes('[DELAY_PROVIDER]')))await new Promise(resolve=>setTimeout(resolve,1000));
  const text=body.model==='synthetic-bad'||(actualWork&&material.body.includes('[BAD_OUTPUT]'))?'invalid JSON':JSON.stringify(output);
  res.writeHead(200,{'Content-Type':'text/event-stream'});
  const chunk=(delta,finish_reason=null)=>({id:'synthetic-response',object:'chat.completion.chunk',created:1,model:body.model,choices:[{index:0,delta,finish_reason}]});
  const limited=body.model==='synthetic-limit';
  res.write(`data: ${JSON.stringify(chunk({role:'assistant',content:text}))}\n\n`);res.write(`data: ${JSON.stringify(chunk({},limited?'length':'stop'))}\n\n`);
  if(body.model!=='synthetic-no-usage')res.write(`data: ${JSON.stringify({id:'synthetic-response',choices:[],usage:{prompt_tokens:800,completion_tokens:limited?1024:100,total_tokens:limited?1824:900}})}\n\n`);
  res.end('data: [DONE]\n\n');
});
server.listen(0,'127.0.0.1',()=>process.stdout.write(`http://127.0.0.1:${server.address().port}/v1\n`));
