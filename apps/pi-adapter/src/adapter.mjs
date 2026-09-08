import { Agent } from '@earendil-works/pi-agent-core';
import { createModels, createProvider } from '@earendil-works/pi-ai';
import { openAICompletionsApi } from '@earendil-works/pi-ai/api/openai-completions.lazy';
import { openAIResponsesApi } from '@earendil-works/pi-ai/api/openai-responses.lazy';
import { anthropicMessagesApi } from '@earendil-works/pi-ai/api/anthropic-messages.lazy';
import { pathToFileURL } from 'node:url';

const API = {'openai-completions':openAICompletionsApi, 'openai-responses':openAIResponsesApi, 'anthropic-messages':anthropicMessagesApi};
export const VERSION = 'linggan.pi.v1/0.85.1';
class Rejected extends Error { constructor(code) { super(code); this.code=code; } }
const integer = (v,min,max) => Number.isSafeInteger(v) && v>=min && v<=max;
function validate(r) {
  if(!r || r.version!==VERSION || !['connect','discover','probe','analyze'].includes(r.operation) || !API[r.api]
    || !integer(r.timeoutMs,100,60000) || !integer(r.maxOutputTokens,16,8192)
    || typeof r.apiKey!=='string' || r.apiKey.length>4096 || typeof r.baseUrl!=='string') throw new Rejected('invalid_request');
  const u=new URL(r.baseUrl);
  if(u.username||u.password||u.search||u.hash||(r.localEndpoint===true&&!['127.0.0.1','localhost','[::1]'].includes(u.hostname))||!(u.protocol==='https:'||(r.localEndpoint===true&&u.protocol==='http:'&&['127.0.0.1','localhost','[::1]'].includes(u.hostname))))throw new Rejected('endpoint_rejected');
  if(!['connect','discover'].includes(r.operation)&&(!r.modelId||typeof r.modelId!=='string'||r.modelId.length>200||typeof r.prompt!=='string'||typeof r.system!=='string'))throw new Rejected('invalid_request');
  return u;
}
function usageFrom(value,usage) {
  const u=value.usage || value.message?.usage || value.response?.usage;
  if(!u || typeof u!=='object')return;
  const input=u.prompt_tokens ?? u.input_tokens;
  const output=u.completion_tokens ?? u.output_tokens;
  if(integer(input,0,100000000))usage.inputTokens=input+(u.cache_read_input_tokens||0)+(u.cache_creation_input_tokens||0);
  if(integer(output,0,100000000))usage.outputTokens=output;
}
function scopedFetch(base,signal,usage,failure) {
  return async (input,init={}) => {
    const url=new URL(typeof input==='string'||input instanceof URL?input:input.url);
    const prefix=base.pathname.replace(/\/$/,'');
    if(url.origin!==base.origin||!(url.pathname===prefix||url.pathname.startsWith(prefix+'/')))throw new Rejected('endpoint_rejected');
    const response=await fetch(input,{...init,redirect:'manual',signal:AbortSignal.any([signal,...(init.signal?[init.signal]:[])])});
    if(!response.ok){failure.code=response.status>=300&&response.status<400?'provider_redirect_rejected':response.status===401||response.status===403?'authentication_failed':response.status===429?'provider_rate_limited':response.status===404?(url.pathname.endsWith('/models')?'catalog_unavailable':'provider_endpoint_not_found'):'provider_failed';await response.body?.cancel();throw new Rejected(failure.code);}
    if(!response.body)return response;
    let bytes=0,buffer='';const decoder=new TextDecoder();
    const body=response.body.pipeThrough(new TransformStream({transform(chunk,controller){
      bytes+=chunk.byteLength;if(bytes>262144)throw new Rejected('response_too_large');
      buffer+=decoder.decode(chunk,{stream:true});
      const lines=buffer.split('\n');buffer=lines.pop();
      for(const line of lines){if(line.startsWith('data:')){try{usageFrom(JSON.parse(line.slice(5)),usage);}catch{ /* non-JSON SSE terminator */ }}}
      controller.enqueue(chunk);
    }}));
    return new Response(body,{status:response.status,headers:response.headers});
  };
}
export async function execute(r) {
  let timer,agent;const usage={inputTokens:null,outputTokens:null,costUsd:null};
  const started=Date.now();
  try {
    const base=validate(r), abort=new AbortController(), failure={code:null};
    timer=setTimeout(()=>{abort.abort();agent?.abort();},r.timeoutMs);
    const transport=scopedFetch(base,abort.signal,usage,failure);
    if(r.operation==='connect'||r.operation==='discover') {
      const headers=r.api==='anthropic-messages'?{'x-api-key':r.apiKey,'anthropic-version':'2023-06-01'}:{Authorization:`Bearer ${r.apiKey}`};
      const result=await transport(base.href.replace(/\/$/,'')+(r.api==='anthropic-messages'?'/v1/models':'/models'),{headers});
      const payload=await result.json();
      if(!Array.isArray(payload.data))throw new Rejected('catalog_unavailable');
      const ids=payload.data.map(x=>x?.id).filter(id=>typeof id==='string'&&id.length<=200).slice(0,500);
      if(r.apiKey&&ids.some(id=>id.includes(r.apiKey)))throw new Rejected('secret_echo_rejected');
      return {version:VERSION,ok:true,modelIds:ids,modelListOrigin:'ACCOUNT_ENDPOINT',usage,elapsedMs:Date.now()-started};
    }
    // Unknown provider context capacity: a conservative adapter envelope, not advertised capability.
    if(Buffer.byteLength(r.prompt)+Buffer.byteLength(r.system)+r.maxOutputTokens>32768)throw new Rejected('model_input_limit');
    const model={id:r.modelId,name:r.modelId,api:r.api,provider:'linggan-explicit',baseUrl:base.href,
      reasoning:false,input:['text'],cost:{input:0,output:0,cacheRead:0,cacheWrite:0},contextWindow:32768,maxTokens:r.maxOutputTokens};
    // Our current contract accepts final text only. DeepSeek V4 otherwise defaults
    // to thinking even when the local model metadata says reasoning:false.
    const deepseekTextOnly=base.hostname==='api.deepseek.com'&&['deepseek-v4-flash','deepseek-v4-pro','deepseek-v4-flash-vision-exp'].includes(r.modelId);
    const onPayload=payload=>{
      if(!deepseekTextOnly)return payload;
      if(r.api==='openai-responses')return {...payload,reasoning:{effort:'none'}};
      if(r.api==='openai-completions')return {...payload,thinking:{type:'disabled'}};
      return payload;
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
    if(!message||!['stop'].includes(message.stopReason))throw new Rejected(message?.stopReason==='length'?'output_limit':failure.code||'provider_failed');
    if(message.content.some(b=>b.type!=='text'))throw new Rejected('unexpected_content');
    const text=message.content.map(b=>b.text).join('');
    if(Buffer.byteLength(text)>65536)throw new Rejected('response_too_large');
    let normalized=text;try{normalized=JSON.stringify(JSON.parse(text));}catch{ /* business validation follows */ }
    if(r.apiKey&&(text.includes(r.apiKey)||normalized.includes(r.apiKey)))throw new Rejected('secret_echo_rejected');
    return {version:VERSION,ok:true,text,usage,elapsedMs:Date.now()-started};
  } catch(error) {
    const code=r&&Date.now()-started>=r.timeoutMs?'provider_timeout':error instanceof Rejected?error.code:'provider_failed';
    return {version:VERSION,ok:false,failureCode:code,usage,elapsedMs:Date.now()-started};
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
