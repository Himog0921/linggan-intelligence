// Independent review regression: execute the actual topic-map.js async reader functions.
// Synthetic fetch + inert DOM; no provider/platform/database/network access.
// Run: node scripts/test-topic-map-ui.cjs [optional source path]
const fs=require('node:fs'),vm=require('node:vm'),assert=require('node:assert/strict');
const sourcePath=process.argv[2] || require('node:path').join(__dirname,'../apps/api/src/local_web/topic_map.js');
const blank={replaceChildren(){},addEventListener(){},querySelector(){return null},querySelectorAll(){return []},contains(){return false}};
const ctx={document:{getElementById(){return blank},createElement(){return {...blank}},activeElement:null},sessionStorage:{getItem(){return null}},location:{search:'',href:'http://127.0.0.1:3109/topics',origin:'http://127.0.0.1:3109'},URL,URLSearchParams,crypto:require('node:crypto').webcrypto,FormData:class{constructor(f){this.f=f}get(k){return this.f.values?.[k]}getAll(k){return this.f.arrays?.[k]||[]}},fetch:null,console};
const tail=/  load\(\);\n\}\)\(\);\s*$/;let src=fs.readFileSync(sourcePath,'utf8');assert(tail.test(src),'known topic-map initialization boundary');
const initializationSource=src.replace(tail,'globalThis.initialState=state;})();');
src=src.replace(tail,`globalThis.audit={readSample,readWork,readerDialog,replaceSamplesDialog,submitDialog,set(s,d,stack=[]){snapshot=s;dialogState=d;dialogStack=stack;state.domainRef='domain';state.topicRef='topic-A';},get(){return dialogState;}};drawDialog=()=>{};render=()=>{};load=async()=>{};openDialog=(type,data)=>{if(dialogState)dialogStack.push(dialogState);dialogState={type,...data};};})();`);vm.runInNewContext(src,ctx);
(async()=>{
 const restoredScope={domainRef:'domain-A',topicRef:'OLD_TOPIC',platform:'xhs',windowDays:'7',path:'family',overlay:'obstruction_recurrence',query:'OLD_QUERY',compare:['OLD_TOPIC']};
 const initialize=search=>{
   const isolated={...ctx,sessionStorage:{getItem(){return JSON.stringify(restoredScope)}},location:{...ctx.location,search,href:`${ctx.location.origin}/topics${search}`}};
   vm.runInNewContext(initializationSource,isolated);
   return JSON.parse(JSON.stringify(isolated.initialState));
 };
 for(const search of ['?domain=domain-B','?domainRef=domain-B','?domain=domain-A']){
   const initial=initialize(search);
   assert.equal(initial.domainRef,search.includes('domain-B')?'domain-B':'domain-A');
   for(const key of ['topicRef','path','overlay','query','platform','windowDays'])assert.equal(initial[key],'',`${search}: former domain scope must be cleared`);
   assert.deepEqual(initial.compare,[],`${search}: former topic comparisons must be cleared`);
 }
 const deep=initialize('?domainRef=domain-A&topicRef=DEEP_TOPIC&platform=douyin');
 assert.equal(deep.topicRef,'DEEP_TOPIC');assert.equal(deep.platform,'douyin');assert.equal(deep.path,'family');assert.equal(deep.query,'OLD_QUERY');assert.deepEqual(deep.compare,['OLD_TOPIC']);
 const explicit=initialize('?domainRef=domain-B&topicRef=NEW_TOPIC&platform=douyin&windowDays=30');
 assert.equal(explicit.topicRef,'NEW_TOPIC');assert.equal(explicit.platform,'douyin');assert.equal(explicit.windowDays,'30');assert.equal(explicit.path,'');assert.deepEqual(explicit.compare,[]);
 console.log('PASS: both domain aliases clear former scope; same-domain deep links preserve session and explicit URL selection');
 const stale={workRef:'A',title:'Synthetic A',readable:true,platform:'xhs',topicRefs:['topic-A'],research:{fragments:[{fragmentId:'old-comment',field:'studied_comment',text:'RESTRICTED_OLD_COMMENT'}],output:{journey:{rationale:'RESTRICTED_OLD_INTERPRETATION'}}},annotation:{rationale:'RESTRICTED_OLD_ANNOTATION'},evidenceFragment:{text:'RESTRICTED_OLD_FALLBACK'}};
 const b={...stale,workRef:'B',title:'UNRELATED_B',topicRefs:['topic-B']};
 const snap={scope:{recentReferenceWorkRefs:['A','B'],ownIdentityState:'known'},topics:[{topicRef:'topic-A',workRefs:['A']},{topicRef:'topic-B',workRefs:['B']}],works:[stale,b],statistics:{platforms:[]}};
 const fresh={...stale,research:{fragments:[{fragmentId:'new-comment',field:'studied_comment',text:'CURRENT_ALLOWED_COMMENT'}],output:{journey:{rationale:'CURRENT_ALLOWED_INTERPRETATION'}}}};
 for(const mode of ['no-research','missing-work','failed','qualified'])for(const entry of ['sample','standalone']){
   let qualificationRequests=0;
   ctx.fetch=async url=>{
     if(url.includes('/topic-map?')){qualificationRequests++;const q=new URL(url,'http://127.0.0.1').searchParams;assert.equal(q.get('domainRef'),'domain');assert.equal(q.get('referenceWindowDays'),'0');assert.equal(q.get('topicRef'),'topic-A');
       if(mode==='failed')return {ok:false,status:503,json:async()=>({error:'qualification unavailable'})};
       return {ok:true,json:async()=>({works:mode==='missing-work'?[]:[mode==='qualified'?fresh:{...stale,research:null,annotation:null}]})};
     }
     return {ok:true,json:async()=>url.includes('/comments')?{items:[],total:0}:{item:{inspector:{detailCurrent:{body:{value:'CURRENT_SAFE_BODY'}}}}}};
   };
   const judge={type:'judge',topicRef:'topic-A',works:[stale],snapshot:snap};ctx.audit.set(snap,judge);
   await(entry==='sample'?ctx.audit.readSample('A'):ctx.audit.readWork('A'));
   const reader=entry==='sample'?judge.inlineReader:ctx.audit.get(),html=ctx.audit.readerDialog(reader);
   assert.equal(qualificationRequests,1,`${entry}/${mode}: current research must be independently qualified`);
   assert(!html.includes('RESTRICTED_OLD_'),`${entry}/${mode}: stale comment/annotation/interpretation must not escape`);
   assert(html.includes('CURRENT_SAFE_BODY'),`${entry}/${mode}: current safe body remains independently readable`);
   assert.equal(html.includes('CURRENT_ALLOWED_COMMENT'),mode==='qualified');assert.equal(html.includes('CURRENT_ALLOWED_INTERPRETATION'),mode==='qualified');
 }
 console.log('PASS: inline + standalone × fresh null/missing/failure/qualified; current body independent');
 const parent={type:'judge',topicRef:'topic-A',works:[stale],loading:false,readError:'failed expanded scope'};ctx.audit.set(snap,{type:'replace',judgeState:parent},[parent]);
 assert(!ctx.audit.replaceSamplesDialog().includes('UNRELATED_B'));
 await ctx.audit.submitDialog({preventDefault(){},target:{dataset:{form:'replace-samples'},arrays:{workRefs:['B']}}});
 assert.deepEqual(parent.works.map(w=>w.workRef),['A']);
 console.log('PASS: failed full-topic read cannot offer or accept unrelated topic identity');
 const slots=[];ctx.fetch=url=>new Promise(resolve=>slots.push({url,resolve}));const sameTopicB={...b,topicRefs:['topic-A']},judge={type:'judge',topicRef:'topic-A',works:[stale,sameTopicB]};ctx.audit.set(snap,judge);
 const pa=ctx.audit.readSample('A'),pb=ctx.audit.readSample('B');
 function release(batch,ref){batch.forEach(({url,resolve})=>resolve({ok:true,json:async()=>url.includes('/topic-map?')?{works:[{...fresh,workRef:ref}]}:url.includes('/comments')?{items:[],total:0}:{item:{}}}));}
 release(slots.slice(3),'B');await pb;release(slots.slice(0,3),'A');await pa;assert.equal(judge.inlineReader.work.workRef,'B');
 console.log('PASS: reverse completion order cannot overwrite the latest selected identity');
 slots.length=0;const pc=ctx.audit.readSample('A');ctx.audit.set(snap,null);release(slots,'A');await pc;assert.equal(ctx.audit.get(),null);
 console.log('PASS: close during pending source requests cannot reopen or crash');
})().catch(e=>{console.error(e);process.exitCode=1});
