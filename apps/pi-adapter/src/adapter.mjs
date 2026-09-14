import { embed, EmbeddingError } from './embeddings.mjs';
import { Agent } from '@earendil-works/pi-agent-core';
import { createModels, createProvider } from '@earendil-works/pi-ai';
import { openAICompletionsApi } from '@earendil-works/pi-ai/api/openai-completions.lazy';
import { openAIResponsesApi } from '@earendil-works/pi-ai/api/openai-responses.lazy';
import { anthropicMessagesApi } from '@earendil-works/pi-ai/api/anthropic-messages.lazy';
import { pathToFileURL } from 'node:url';
import {
  bodyCompleted,
  classifyUnterminated,
  completeDiagnostic,
  createObservation,
  failed as failedDiagnostic,
  finishReasonForStopReason,
  observeSseLine,
  rejected,
  receivedChunk,
  requestSent,
  responseStarted,
  succeeded,
  terminalReceived,
} from './diagnostic.mjs';

const API = {'openai-completions':openAICompletionsApi, 'openai-responses':openAIResponsesApi, 'anthropic-messages':anthropicMessagesApi};
export const VERSION = 'linggan.pi.v1/0.85.1';
class Rejected extends Error { constructor(code) { super(code); this.code=code; } }
const integer = (v,min,max) => Number.isSafeInteger(v) && v>=min && v<=max;
const DEEPSEEK_TEXT_MODELS=new Set(['deepseek-v4-flash','deepseek-v4-pro','deepseek-v4-flash-vision-exp']);
// Capability allowlist, not provider-name inference. Unknown models and proxies retain
// prompt-only output and the same strict server validator until separately qualified.
const OPENAI_STRUCTURED_MODELS=new Set(['gpt-4o','gpt-4o-2024-08-06','gpt-4o-2024-11-20','gpt-4o-mini','gpt-4o-mini-2024-07-18','gpt-6-astra']);
function researchOutputFormat(r,base) {
  let packet;try{packet=JSON.parse(r.prompt);}catch{return null;}
  const schemaName=packet?.contract==='comment-research.semantic.v1'?'comment_research_semantic_v1':packet?.contract==='comment-research.semantic.v2'?'comment_research_semantic_v2':packet?.contract==='comment-research.semantic.v3'?'comment_research_semantic_v3':packet?.contract==='comment-research.semantic.v4'?'comment_research_semantic_v4':packet?.contract==='comment-research.semantic.v5'?'comment_research_semantic_v5':packet?.contract==='comment-research.semantic.v1/problem-resolution'?'comment_research_problem_resolution_v1':packet?.contract==='comment-research.semantic.v2/problem-resolution'?'comment_research_problem_resolution_v2':packet?.contract==='comment-research.semantic.v3/problem-resolution'?'comment_research_problem_resolution_v3':packet?.contract==='comment-research.semantic.v4/problem-resolution'?'comment_research_problem_resolution_v4':packet?.contract==='comment-research.semantic.v5/problem-resolution'?'comment_research_problem_resolution_v5':null;
  if(!schemaName)return null;
  const schema=packet.outputSchema;
  if(!schema||schema.type!=='object'||!schema.properties||schema.additionalProperties!==false||JSON.stringify(schema).length>6144)throw new Rejected('invalid_request');
  const official=base.protocol==='https:'&&!base.port&&['','/','/v1','/v1/'].includes(base.pathname);
  const deepseek=official&&base.hostname==='api.deepseek.com'&&DEEPSEEK_TEXT_MODELS.has(r.modelId);
  const openai=official&&base.hostname==='api.openai.com'&&OPENAI_STRUCTURED_MODELS.has(r.modelId);
  // V1 uses optional fields for its two tagged response variants.  Do not claim strict
  // provider validation when the provider would require every property on the root object;
  // Rust remains the contract authority after transport.
  if(r.api==='openai-responses'&&(deepseek||openai))return {text:{format:{type:'json_schema',name:schemaName,schema}}};
  if(r.api==='openai-completions'&&deepseek)return {response_format:{type:'json_object'}};
  if(r.api==='openai-completions'&&openai)return {response_format:{type:'json_schema',json_schema:{name:schemaName,schema}}};
  return null;
}
function validate(r) {
  if(!r || r.version!==VERSION || !['connect','discover','probe','analyze','embed'].includes(r.operation) || !API[r.api]
    || !integer(r.timeoutMs,100,60000) || !integer(r.maxOutputTokens,16,8192)
    || typeof r.apiKey!=='string' || r.apiKey.length>4096 || typeof r.baseUrl!=='string') throw new Rejected('invalid_request');
  let u;try{u=new URL(r.baseUrl);}catch{throw new Rejected('invalid_request');}
  if(u.username||u.password||u.search||u.hash||(r.localEndpoint===true&&!['127.0.0.1','localhost','[::1]'].includes(u.hostname))||!(u.protocol==='https:'||(r.localEndpoint===true&&u.protocol==='http:'&&['127.0.0.1','localhost','[::1]'].includes(u.hostname))))throw new Rejected('endpoint_rejected');
  if(!['connect','discover'].includes(r.operation)&&(!r.modelId||typeof r.modelId!=='string'||r.modelId.length>200||typeof r.prompt!=='string'||typeof r.system!=='string'))throw new Rejected('invalid_request');
  return u;
}
function usageFrom(value,usage) {
  const u=value.usage || value.message?.usage || value.response?.usage;
  if(!u || typeof u!=='object')return;
  const input=u.prompt_tokens ?? u.input_tokens;
  const output=u.completion_tokens ?? u.output_tokens;
  const cacheRead=u.cache_read_input_tokens;
  const cacheWrite=u.cache_creation_input_tokens;
  if(integer(input,0,100000000))usage.inputTokens=input+(integer(cacheRead,0,100000000)?cacheRead:0)+(integer(cacheWrite,0,100000000)?cacheWrite:0);
  if(integer(output,0,100000000))usage.outputTokens=output;
}
function providerFailure(status,url) {
  if(status>=300&&status<400)return 'provider_redirect_rejected';
  if(status===401||status===403)return 'authentication_failed';
  if(status===429)return 'provider_rate_limited';
  if(status===404)return url.pathname.endsWith('/models')?'catalog_unavailable':'provider_endpoint_not_found';
  if(status===400||status===422)return 'provider_request_rejected';
  if(status>=500)return 'provider_unavailable';
  return 'provider_failed';
}
function scopedFetch(base,signal,usage,failure,observation) {
  return async (input,init={}) => {
    let url;try{url=new URL(typeof input==='string'||input instanceof URL?input:input.url);}catch{throw new Rejected('endpoint_rejected');}
    const prefix=base.pathname.replace(/\/$/,'');
    if(url.origin!==base.origin||!(url.pathname===prefix||url.pathname.startsWith(prefix+'/')))throw new Rejected('endpoint_rejected');
    requestSent(observation);
    let response;try{response=await fetch(input,{...init,redirect:'manual',signal:AbortSignal.any([signal,...(init.signal?[init.signal]:[])])});}catch{
      failure.code=signal.aborted?'provider_timeout':'provider_network_error';
      throw new Rejected(failure.code);
    }
    responseStarted(observation,response.status);
    if(!response.ok){failure.code=providerFailure(response.status,url);await response.body?.cancel();throw new Rejected(failure.code);}
    if(!response.body){bodyCompleted(observation);return response;}
    let buffer='';const decoder=new TextDecoder();
    const inspect=line=>observeSseLine(observation,line,usage,usageFrom);
    const body=response.body.pipeThrough(new TransformStream({transform(chunk,controller){
      receivedChunk(observation,chunk.byteLength);
      if(observation.diagnostic.receivedBytes>262144){failure.code='response_too_large';throw new Rejected(failure.code);}
      buffer+=decoder.decode(chunk,{stream:true});
      const lines=buffer.split(/\r?\n/);buffer=lines.pop()??'';
      for(const line of lines)inspect(line);
      controller.enqueue(chunk);
    },flush(){
      buffer+=decoder.decode();
      if(buffer)inspect(buffer);
      bodyCompleted(observation);
    }}));
    return new Response(body,{status:response.status,headers:response.headers});
  };
}
export async function execute(r) {
  let timer,agent,abort;const usage={inputTokens:null,outputTokens:null,costUsd:null};
  const started=Date.now();
  const observation=createObservation(),failure={code:null};
  const response=payload=>({...payload,diagnostic:completeDiagnostic(observation,usage,started)});
  try {
    const base=validate(r);abort=new AbortController();
    timer=setTimeout(()=>{abort.abort();agent?.abort();},r.timeoutMs);
    const transport=scopedFetch(base,abort.signal,usage,failure,observation);
    if(r.operation==='embed') {
      const result=await embed(r,base,transport,usage);usage.inputTokens=result.inputTokens;usage.outputTokens=0;
      succeeded(observation);
      return response({version:VERSION,ok:true,text:result.text,usage,elapsedMs:Date.now()-started});
    }
    if(r.operation==='connect'||r.operation==='discover') {
      const headers=r.api==='anthropic-messages'?{'x-api-key':r.apiKey,'anthropic-version':'2023-06-01'}:{Authorization:`Bearer ${r.apiKey}`};
      const result=await transport(base.href.replace(/\/$/,'')+(r.api==='anthropic-messages'?'/v1/models':'/models'),{headers});
      const payload=await result.json();
      if(!Array.isArray(payload.data))throw new Rejected('catalog_unavailable');
      const ids=payload.data.map(x=>x?.id).filter(id=>typeof id==='string'&&id.length<=200).slice(0,500);
      if(r.apiKey&&ids.some(id=>id.includes(r.apiKey)))throw new Rejected('secret_echo_rejected');
      succeeded(observation);
      return response({version:VERSION,ok:true,modelIds:ids,modelListOrigin:'ACCOUNT_ENDPOINT',usage,elapsedMs:Date.now()-started});
    }
    // Unknown provider context capacity: a conservative adapter envelope, not advertised capability.
    if(Buffer.byteLength(r.prompt)+Buffer.byteLength(r.system)+r.maxOutputTokens>32768)throw new Rejected('model_input_limit');
    const model={id:r.modelId,name:r.modelId,api:r.api,provider:'linggan-explicit',baseUrl:base.href,
      reasoning:false,input:['text'],cost:{input:0,output:0,cacheRead:0,cacheWrite:0},contextWindow:32768,maxTokens:r.maxOutputTokens};
    // Our current contract accepts final text only. DeepSeek V4 otherwise defaults
    // to thinking even when the local model metadata says reasoning:false.
    const deepseekTextOnly=base.hostname==='api.deepseek.com'&&DEEPSEEK_TEXT_MODELS.has(r.modelId);
    const outputFormat=researchOutputFormat(r,base);
    const onPayload=payload=>{
      const formatted={...payload,...outputFormat};
      if(!deepseekTextOnly)return formatted;
      if(r.api==='openai-responses')return {...formatted,reasoning:{effort:'none'}};
      if(r.api==='openai-completions')return {...formatted,thinking:{type:'disabled'}};
      return formatted;
    };
    const models=createModels();
    models.setProvider(createProvider({id:model.provider,baseUrl:base.href,models:[model],
      auth:{apiKey:{name:'Explicit connection',resolve:async()=>({auth:{apiKey:r.apiKey}})}},api:API[r.api]()}));
    agent=new Agent({initialState:{systemPrompt:r.system,model,tools:[]},
      streamFn:(m,c,o)=>models.streamSimple(m,c,{...o,signal:AbortSignal.any([abort.signal,...(o?.signal?[o.signal]:[])]),
        apiKey:r.apiKey,fetch:transport,onPayload,timeoutMs:r.timeoutMs,maxRetries:0,maxTokens:r.maxOutputTokens}),
      shouldStopAfterTurn:()=>true,beforeToolCall:()=>({block:true,reason:'No tools granted',terminate:true})});
    await agent.prompt(r.prompt);
    if(abort.signal.aborted||Date.now()-started>=r.timeoutMs)throw new Rejected('provider_timeout');
    const message=agent.state.messages.filter(m=>m.role==='assistant').at(-1);
    if(!message)throw new Rejected(failure.code||classifyUnterminated(observation));
    const finishReason=finishReasonForStopReason(message.stopReason);
    if(['stop','length','toolUse','tool_calls','content_filter'].includes(message.stopReason))terminalReceived(observation,finishReason);
    if(message.stopReason!=='stop') {
      if(failure.code)throw new Rejected(failure.code);
      if(observation.diagnostic.finishReason==='length')throw new Rejected('output_limit');
      if(observation.diagnostic.finishReason==='content_filter')throw new Rejected('provider_content_filtered');
      if(observation.diagnostic.finishReason==='tool_calls')throw new Rejected('unexpected_content');
      if(message.stopReason==='aborted')throw new Rejected('provider_timeout');
      throw new Rejected(classifyUnterminated(observation));
    }
    if(message.content.some(b=>b.type!=='text'))throw new Rejected('unexpected_content');
    const text=message.content.map(b=>b.text).join('');
    if(Buffer.byteLength(text)>65536)throw new Rejected('response_too_large');
    let normalized=text;try{normalized=JSON.stringify(JSON.parse(text));}catch{ /* business validation follows */ }
    if(r.apiKey&&(text.includes(r.apiKey)||normalized.includes(r.apiKey)))throw new Rejected('secret_echo_rejected');
    succeeded(observation,'stop');
    return response({version:VERSION,ok:true,text,usage,elapsedMs:Date.now()-started});
  } catch(error) {
    let code;
    if(r&&((abort?.signal.aborted===true)||Date.now()-started>=r.timeoutMs))code='provider_timeout';
    else if(error instanceof Rejected||error instanceof EmbeddingError)code=error.code;
    else code=classifyUnterminated(observation);
    if(['invalid_request','endpoint_rejected','model_input_limit'].includes(code))rejected(observation);
    else failedDiagnostic(observation,code);
    return response({version:VERSION,ok:false,failureCode:code,usage,elapsedMs:Date.now()-started});
  } finally {clearTimeout(timer);agent?.abort();}
}

if(process.argv[1] && import.meta.url===pathToFileURL(process.argv[1]).href) {
  let bytes=0,chunks=[];
  try {
    for await(const chunk of process.stdin){bytes+=chunk.length;if(bytes>131072)throw new Error();chunks.push(chunk);}
    const request=JSON.parse(Buffer.concat(chunks).toString('utf8'));
    process.stdout.write(JSON.stringify(await execute(request))+'\n');
  } catch {process.stdout.write(JSON.stringify({version:VERSION,ok:false,failureCode:'invalid_request',usage:{inputTokens:null,outputTokens:null,costUsd:null}})+'\n');}
}
