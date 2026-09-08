// MODEL-CALL-004 synthetic DOM contract proof; no provider or shared-runtime calls.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import vm from 'node:vm';
const source=readFileSync(new URL('../../apps/api/src/local_web/model_settings.js',import.meta.url),'utf8');
function setup(overrides={}){
  const nodes=new Map(),requests=[];
  class Element{
    constructor(){this.innerHTML='';this.textContent='';this.value='';this.disabled=false;this.hidden=false;this.listeners=new Map();this.fields=new Map();this.elements={namedItem:name=>this.fields.get(name)};this.formValues=[];}
    addEventListener(kind,cb){this.listeners.set(kind,cb)}
    replaceChildren(){this.innerHTML=''} close(){this.open=false} showModal(){this.open=true} focus(){} scrollIntoView(){} querySelectorAll(){return[]} reportValidity(){return true}
  }
  const get=id=>{if(!nodes.has(id))nodes.set(id,new Element());return nodes.get(id)};
  const model={modelRef:'partial-model',modelId:'partial-test-model',connectionName:'合成服务',connectionVersionRef:'version',currentVersion:true,enabled:true,modelCallable:true,commentQualified:false,test:{ok:true,modelCallable:true,validationCode:'partial_fields_rejected'}};
  const data={models:[model,{...model,modelRef:'failed-model',modelCallable:false,test:{failureCode:'output_limit'}},{...model,modelRef:'never-tested',modelCallable:false,test:null}],connections:[{connectionRef:'connection',versionRef:'version',enabled:true,name:'合成服务',api:'openai-completions',baseUrl:'https://example.invalid/v1'}],model:{modelCallable:false,modelConnected:false,modelState:'NEEDS_SELECTION'},plans:[],runs:[],legacyPlansAvailable:false,...overrides};
  const context={document:{getElementById:get,querySelectorAll:()=>[],addEventListener(){}},window:{},location:{search:''},URL,URLSearchParams,crypto:{randomUUID:()=> 'synthetic-command'},FormData:class{constructor(el){this.values=el.formValues}get(k){return this.values.find(([n])=>n===k)?.[1]??null}has(k){return this.values.some(([n])=>n===k)}[Symbol.iterator](){return this.values[Symbol.iterator]()}},fetch:async(url,options)=>{requests.push({url,body:options.body&&JSON.parse(options.body)});return{ok:true,json:async()=>url.endsWith('/embedding')?{}:url.endsWith('/probes')?data.probeResult||model.test:data}}};
  const startup="  load().then(()=>{const ref=new URLSearchParams(location.search).get('sourceRef');if(ref&&data.config&&data.legacyPlansAvailable!==false)return chooseSources('trial',ref);}).catch(showError);";
  assert.ok(source.includes(startup));
  vm.runInNewContext(source.replace(startup,'  window.test={load,modelStatus,selectDefault,probeMessage,diagnosticsHtml,diagnosticActual,callable,runConnection,setCommand:c=>{command=c;},setData:d=>{data=d;}};'),context);
  context.window.test.setData(data);return {app:context.window.test,data,get,requests,context};
}
test('callable partial model is selectable while failed and untested models remain disabled',async()=>{
  const {app,get}=setup();await app.load();const options=get('default-model').innerHTML;
  assert.match(options,/<option value="partial-model" >/);
  assert.match(options,/<option value="failed-model" disabled>/);
  assert.match(options,/<option value="never-tested" disabled>/);
  app.selectDefault('partial-model');assert.equal(get('default-model').value,'partial-model');
  assert.throws(()=>app.selectDefault('failed-model'),/测试模型调用/);
});
test('default config saves callable model even when commentQualified is false',async()=>{
  const {app,get,requests}=setup();await app.load();const form=get('config-form');form.formValues=[['modelRef','partial-model'],['inputTokenLimit','16000']];
  await form.listeners.get('submit')({preventDefault(){},currentTarget:form,submitter:{}});
  assert.equal(requests.find(r=>r.url.endsWith('/config')).body.modelRef,'partial-model');
  assert.match(get('settings-feedback').textContent,/默认模型已保存/);
});
test('forged selection of failed model cannot save default config',async()=>{
  const {app,get,requests}=setup();await app.load();const form=get('config-form');form.formValues=[['modelRef','failed-model']];
  await form.listeners.get('submit')({preventDefault(){},currentTarget:form,submitter:{}});
  assert.ok(!requests.some(r=>r.url.endsWith('/config')));
});
test('partial probe success does not imply every field passed and truncated output is not ready',()=>{
  const {app}=setup();assert.match(app.probeMessage({ok:true,modelCallable:true,commentQualified:false,commentValidation:{status:'partial'}}),/模型调用成功，可以设为默认。评论测试部分字段未通过，研究时逐项校验/);
  assert.doesNotMatch(app.probeMessage({ok:false,modelCallable:true,failureCode:'output_limit'}),/可以设为默认/);
});
test('historical partial result discloses absent field diagnostics',()=>{
  const {app}=setup();const html=app.diagnosticsHtml({validationCode:'partial_fields_rejected'});
  assert.match(html,/历史测试未保存具体字段诊断/);assert.doesNotMatch(html,/接纳 0|字段位置/);
});
test('diagnostics display server paths and type counts while omitting raw completion fields',()=>{
  const {app}=setup();const html=app.diagnosticsHtml({prompt:'SECRET PROMPT',commentValidation:{status:'partial',diagnosticsRecorded:true,acceptedFields:2,rejectedFields:1,uncertainFields:null,diagnostics:[{code:'field_schema_invalid',path:'$.comments[0].labels[0].basis',expected:'类型 <string>',actual:{type:'string',characters:17,value:'SECRET COMPLETION',raw:'SECRET RAW'}}]}});
  assert.match(html,/接纳 2 项 · 未接纳 1 项 · 含义不确定 未记录/);assert.match(html,/labels\[0\].basis/);assert.match(html,/文本 · 字数 17/);assert.match(html,/&lt;string&gt;/);assert.doesNotMatch(html,/SECRET|<string>/);
});
test('all v4 parser error codes have Chinese explanations',()=>{
  const {app}=setup();const rust=readFileSync(new URL('../../crates/intelligence/src/comment_packet_result.rs',import.meta.url),'utf8');
  const codes=[...new Set([...rust.matchAll(/(?:Err\(|ok_or\(|map_err\(\|_\| )"([a-z_]+)"/g)].map(m=>m[1]))];
  for(const code of codes){const html=app.diagnosticsHtml({commentValidation:{status:'failed',diagnosticsRecorded:true,diagnostics:[{code}]}});assert.doesNotMatch(html,/<strong>该字段未通过校验<\/strong>/,code);assert.doesNotMatch(html,new RegExp(code),code);}
});

test('single provider dialog exposes choose-default after successful partial probe',async()=>{
  const {app,data,get,requests}=setup({probeResult:{ok:true,modelCallable:true,commentQualified:false,commentValidation:{status:'partial',diagnosticsRecorded:true,acceptedFields:2,rejectedFields:1,uncertainFields:0,diagnostics:[]}}});
  const connection={...data.connections[0],baseUrl:'https://example.invalid/v1',name:'合成服务',api:'openai-completions',localEndpoint:false};
  const model=data.models[0],form=get('model-command');form.formValues=[['name',connection.name],['api',connection.api],['baseUrl',connection.baseUrl],['apiKey',''],['modelId',model.modelId]];
  for(const [key,value]of form.formValues){const field={value};form.fields.set(key,field);form.elements[key]=field;}
  app.setCommand({kind:'connection',connection,models:[model],connectionRef:connection.connectionRef});
  await app.runConnection('test');assert.equal(get('dialog-select').hidden,false);
  assert.match(get('dialog-feedback').textContent,/调用成功，可以设为默认/);
  assert.equal(requests.filter(r=>r.url.endsWith('/probes')).length,1);
  get('dialog-select').onclick();assert.equal(get('default-model').value,model.modelRef);
});

test('comment entry surfaces follow modelConnected instead of comment qualification',async()=>{
  const code=readFileSync(new URL('../../apps/api/src/local_web/comment_daily.js',import.meta.url),'utf8');
  const dialog={innerHTML:'',showModal(){this.open=true},querySelector(){return null},addEventListener(){}};
  const model={model:{modelConnected:true,modelCallable:true},config:{modelRef:'partial-model'},models:[{modelRef:'partial-model',modelId:'合成可调用',commentQualified:false}]};
  const context={window:{},document:{createElement:()=>dialog,body:{append(){}},addEventListener(){}},fetch:async()=>({ok:true,json:async()=>model}),crypto:{randomUUID:()=> 'test-command'}};
  vm.runInNewContext(code,context);await context.window.CommentDaily.openSelected(['one']);assert.match(dialog.innerHTML,/data-daily-selected/);
  model.model={modelConnected:false,modelCallable:true,modelState:'NEEDS_CALL_TEST'};await context.window.CommentDaily.openSelected(['one']);assert.doesNotMatch(dialog.innerHTML,/data-daily-selected/);assert.match(dialog.innerHTML,/测试模型调用/);
});
