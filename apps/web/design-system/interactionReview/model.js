import {INITIAL_RUNS} from '../taskLedger/model.js';
import presets from './presets.json';
import {spec} from '../dsReview/data.js';
const accepted=spec.visual.overlayApplication;
export const originalText='先把任务放小，再观察真正卡住的位置。';
export const unresolved=r=>r&&['pending','unknown','refresh'].includes(r.phase);
export const phaseLabel={pending:'请求在途',unknown:'结果未确认',refresh:'命令已确认，运行待核对',confirmed:'结果已核对',rejected:'请求被明确拒绝',conflict:'版本冲突',resolved:'冲突已重新读取'};
export const actionLabel={create:'发起采集',save:'保存正文',settings:'保存配置',pause:'暂停本次',continue:'继续本次',stop:'停止本次'};
export function initial(q=new URLSearchParams()){
 const scenario=presets.scenarios[q.get('scenario')]?q.get('scenario'):'read';
 return {scenario,scene:presets.scenarios[scenario].scene,domain:'adhd',policy:accepted.decision.selected,shape:['drawer','dialog','nonmodal'].includes(q.get('shape'))?q.get('shape'):'drawer',width:['auto','1440','1024','768','390','320'].includes(q.get('width'))?q.get('width'):'auto',overlay:null,guard:null,request:null,seq:0,scopes:{},doc:originalText,savedDoc:originalText,forms:{create:'家庭任务启动',settings:'家庭协作'},savedForms:{create:'家庭任务启动',settings:'家庭协作'},tasks:Object.fromEntries(INITIAL_RUNS.map(r=>[r.id,{...r,control:r.status==='paused'?'paused':r.status==='stopped'?'stopped':['ended','legacy'].includes(r.status)?r.status:'running'}])),notice:'所有数据与回执均为合成；操作不连接业务。',log:[]};
}
const controlFields=control=>({control,status:control==='running'?'waiting':control,stage:control==='running'?'等待调度':control==='paused'?'人工暂停':control==='ended'?'本轮结束':'已停止，成果保留',next:control==='stopped'?'已得材料保留，本次不可继续':control==='paused'?'核对资格后可继续固定范围':control==='running'?'等待有效租约':'本轮结束，查看已有材料'});
const log=(s,text)=>({...s,notice:text,log:[text,...s.log].slice(0,12)});
const scopeDefaults=()=>({doc:originalText,savedDoc:originalText,forms:{create:'家庭任务启动',settings:'家庭协作'},savedForms:{create:'家庭任务启动',settings:'家庭协作'}});
function switchDomain(s,domain){if(domain===s.domain)return s;const scopes={...s.scopes,[s.domain]:{doc:s.doc,savedDoc:s.savedDoc,forms:s.forms,savedForms:s.savedForms}};return {...s,domain,scopes,...(scopes[domain]||scopeDefaults())}}
export const formKey=s=>s.overlay?.formKey||s.overlay?.kind;
export const formValue=(s,saved=false)=>(saved?s.savedForms:s.forms)[formKey(s)]??(saved?s.savedForms:s.forms)[s.overlay?.kind]??'';
export function dirty(s){return ['create','settings'].includes(s.overlay?.kind)?formValue(s)!==formValue(s,true):s.scene.startsWith('P12:')&&s.doc!==s.savedDoc}
function openOverlay(s,overlay){const key=overlay.formKey;if(key&&!(key in s.forms)&&typeof overlay.initialValue==='string')return {...s,overlay,forms:{...s.forms,[key]:overlay.initialValue},savedForms:{...s.savedForms,[key]:overlay.initialValue}};return {...s,overlay};}
function applyIntent(s,i){
 const next={...s,overlay:null,guard:null};
 if(i.type==='scenario')return {...next,scenario:i.value,scene:presets.scenarios[i.value].scene};
 if(i.type==='scene')return {...next,scene:i.value};
 if(i.type==='domain')return switchDomain(next,i.value);
 if(i.type==='open')return openOverlay(next,i.overlay);
 if(i.type==='reset'){const n=initial();return {...n,scenario:s.scenario,scene:s.scene,policy:s.policy,shape:s.shape,width:s.width,seq:s.seq,request:unresolved(s.request)?{...s.request,detached:true}:null,tasks:s.tasks,notice:'画面已重置；未确认请求仍保留跟踪，不会取消。'}}
 return next;
}
function requestLeave(s,intent,via){
 if(via==='backdrop'&&!accepted.policy.readonlyBackdropCloses)return s;
 if(s.overlay?.kind==='stop'&&!unresolved(s.request)&&intent.type==='close')return log({...s,overlay:{...s.overlay,kind:'task'}},'已取消停止确认，返回任务详情，运行保持原状态。');
 if(unresolved(s.request)){
  if(s.request.detached&&!s.overlay)return log(applyIntent(s,intent),'当前已在跟踪原请求，可以继续浏览；不取消、不重复提交。');
  return {...s,guard:{kind:'tracking',intent},notice:'收起只离开当前视图，原请求身份、对象与范围继续保留。'};
 }
 const localDirty=s.overlay&&['close','open'].includes(intent.type)?['create','settings'].includes(s.overlay.kind)&&formValue(s)!==formValue(s,true):dirty(s);
 if(localDirty)return {...s,guard:{kind:'dirty',intent},notice:'未保存内容仍在当前页，请决定怎样离开。'};
 return log(applyIntent(s,intent),'已返回原上下文；没有取消业务执行。');
}
function begin(s,action,intent=null){
 if(unresolved(s.request)||s.request?.phase==='conflict')return log(s,'上一项结果或版本尚未核对，先读取原回执/当前版本，不重复发出新请求。');
 const run=s.overlay?.runId?s.tasks[s.overlay.runId]:null;
 if(['pause','continue','stop'].includes(action)&&(!run||['stopped','ended','legacy'].includes(run.control)||!Number.isInteger(run.revision)||(action==='continue'&&run.control!=='paused')))return log(s,'当前运行不具备这项控制资格；已停止或结束不能继续。');
 const key=action==='save'?null:formKey(s)||action;
 const text=action==='save'?s.doc:['create','settings'].includes(s.overlay?.kind)?formValue(s):s.forms[action]||'';
 if(['create','settings'].includes(action)&&!text.trim())return {...s,guard:null,notice:'请填写名称或关键词后提交。',invalid:true};
 const snapshot={scene:s.scene,domain:run?(run.domain==='ADHD'?'adhd':'learning'):s.overlay?.domain||s.domain,runId:run?.id,revision:run?.revision,objectTitle:s.overlay?.title||'当前作品',formKey:key,text,action};
 return log({...s,guard:null,invalid:false,seq:s.seq+1,request:{id:'DEMO-REQ-'+String(s.seq+1).padStart(3,'0'),phase:'pending',snapshot,action,intent,detached:false,reads:0}},actionLabel[action]+'请求已发出，尚无结果；输入与作用范围已冻结。');
}
export function reduce(s,a){
 switch(a.type){
 case 'configure':return a.key==='policy'?{...s,policy:accepted.decision.selected}:{...s,[a.key]:a.value};
 case 'notice':return log(s,a.text);
 case 'edit':return unresolved(s.request)?log(s,'原请求未确认，暂不覆盖冻结输入。'):{...s,doc:a.value};
 case 'form':return unresolved(s.request)?s:{...s,invalid:false,forms:{...s.forms,[a.kind]:a.value}};
 case 'open':return s.overlay?requestLeave(s,{type:'open',overlay:a.overlay},'switch'):{...openOverlay(s,a.overlay),invalid:false,notice:'正在查看当前对象；本地演练不会执行真实命令。'};
 case 'example':{if(unresolved(s.request)||dirty(s))return log(s,'先核对原请求或处理未保存内容，再载入另一份合成样例。');const run=Object.values(s.tasks).find(r=>r.domain===(s.domain==='learning'?'考研自习':'ADHD'));return {...s,tasks:{...s.tasks,[run.id]:{...run,...controlFields(a.control)}},overlay:{kind:'task',title:run.name+' · '+a.label,runId:run.id,domain:s.domain},notice:'已明确载入'+a.label+'，仅改变合成演练前提。'};}
 case 'leave':return requestLeave(s,a.intent||{type:'close'},a.via||'button');
 case 'stay':return {...s,guard:null};
 case 'discard':{const next=['create','settings'].includes(s.overlay?.kind)?{...s,forms:{...s.forms,[formKey(s)]:formValue(s,true)}}:{...s,doc:s.savedDoc};return log(applyIntent(next,s.guard.intent),'已明确放弃这次未保存修改。');}
 case 'saveLeave':return begin(s,['create','settings'].includes(s.overlay?.kind)?s.overlay.kind:'save',s.guard.intent);
 case 'detach':return log(applyIntent({...s,request:{...s.request,detached:true}},s.guard.intent),'已收起并保留原请求跟踪。迟到回执不会抢回页面或重新打开窗口。');
 case 'submit':return begin(s,a.action);
 case 'stopConfirm':return s.overlay?{...s,overlay:{...s.overlay,kind:'stop',returnKind:'task'}}:s;
 case 'cancelStop':return {...s,overlay:{...s.overlay,kind:'task'},guard:null,notice:'取消停止确认，原任务状态保持。'};
 case 'lookup':return s.request?.phase==='unknown'?log({...s,request:{...s.request,phase:'pending',reads:s.request.reads+1}},'正在核对原请求回执：'+s.request.id+'，没有创建第二个请求。'):s;
 case 'refreshForm':return s.request?.phase==='conflict'&&!s.request.snapshot.runId?log({...s,request:{...s.request,phase:'resolved'}},'已模拟读取最新版本，当前稿件保留；没有强制覆盖，后续保存仍需明确提交。'):s;
 case 'reopen':{const r=s.request;if(!r)return s;return {...switchDomain(s,r.snapshot.domain),scene:r.snapshot.scene,overlay:r.snapshot.runId?{kind:'task',title:s.tasks[r.snapshot.runId].name,runId:r.snapshot.runId,domain:r.snapshot.domain}:r.action==='save'?null:{kind:r.action,title:r.snapshot.objectTitle||actionLabel[r.action],formKey:r.snapshot.formKey,domain:r.snapshot.domain},request:{...r,detached:false}};}
 case 'receipt':{
  const r=s.request;if(!r||!['pending','unknown'].includes(r.phase))return s;
  if(a.outcome!=='accepted')return log({...s,guard:null,request:{...r,phase:a.outcome==='timeout'?'unknown':a.outcome}},a.outcome==='timeout'?'结果未确认；保留原身份，先核对回执。':a.outcome==='conflict'?'版本已变化，不能沿旧版本执行；先重新读取当前对象。':'请求被明确拒绝；内容与原运行保持，允许修改后重新发起。');
  if(['pause','continue','stop'].includes(r.action))return log({...s,guard:null,request:{...r,phase:'refresh'}},'原命令回执身份与结果已核对；还需重新读取运行，不能只凭按钮更新状态。');
  let n={...s,guard:null,request:{...r,phase:'confirmed'}};
  if(r.snapshot.domain===s.domain){if(r.action==='save'&&s.doc===r.snapshot.text)n.savedDoc=r.snapshot.text;else if(r.action!=='save')n.savedForms={...n.savedForms,[r.snapshot.formKey||r.action]:r.snapshot.text};}else{const cache=s.scopes[r.snapshot.domain]||scopeDefaults();n.scopes={...s.scopes,[r.snapshot.domain]:r.action==='save'?{...cache,savedDoc:r.snapshot.text}:{...cache,savedForms:{...cache.savedForms,[r.snapshot.formKey||r.action]:r.snapshot.text}}};}
  if(r.intent&&!r.detached&&r.snapshot.domain===s.domain)n=applyIntent(n,r.intent);
  return log(n,r.detached?'原请求回执已到；当前页面保持，不自动导航或弹出。':r.action==='create'?'合成回执：本次请求已受理并排队，不代表执行完成。':'合成回执：这一份内容已保存。');
 }
 case 'readRun':{
  const r=s.request;if(!r||!['refresh','conflict'].includes(r.phase))return s;
  const run=s.tasks[r.snapshot.runId];if(!run)return log(s,'不是运行控制回执，请回到原表单核对版本。');
  if(a.fail)return log(s,'运行读取失败，保留上次材料；命令回执仍可核对，不重复控制。');
  const control=r.phase==='conflict'?(a.control||run.control):r.action==='stop'?'stopped':r.action==='pause'?'paused':'running';
  return log({...s,overlay:s.overlay?.kind==='stop'&&s.overlay.runId===run.id?{...s.overlay,kind:'task'}:s.overlay,tasks:{...s.tasks,[run.id]:{...run,...controlFields(control),queued:control==='stopped'?0:run.queued,revision:run.revision+1}},request:{...r,phase:r.phase==='conflict'?'resolved':'confirmed'}},'已重新读取原运行：'+({stopped:'已停止，成果保留',paused:'已暂停，可在合格时继续',running:'继续等待调度',ended:'本轮结束'}[control]||control));
 }
 case 'deliver':{const id=s.overlay?.runId||s.request?.snapshot.runId,run=s.tasks[id];return run?log({...s,tasks:{...s.tasks,[id]:{...run,details:run.details+1,comments:run.comments+4}}},'合成在途材料已交付；运行控制状态没有因此恢复。'):s;}
 default:return s;
 }
}
