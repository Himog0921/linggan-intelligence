// Independent review regression: execute the actual topic-map.js async reader functions.
// Synthetic fetch + inert DOM; no provider/platform/database/network access.
// Run: node scripts/test-topic-map-ui.cjs [optional source path]
const fs=require('node:fs'),vm=require('node:vm'),assert=require('node:assert/strict');
const sourcePath=process.argv[2] || require('node:path').join(__dirname,'../apps/api/src/local_web/topic_map.js');
const blank={replaceChildren(){},addEventListener(){},querySelector(){return null},querySelectorAll(){return []},contains(){return false}};
const ctx={document:{getElementById(){return blank},createElement(){return {...blank}},activeElement:null},sessionStorage:{getItem(){return null}},location:{search:'',href:'http://127.0.0.1:3109/topics',origin:'http://127.0.0.1:3109'},URL,URLSearchParams,crypto:require('node:crypto').webcrypto,FormData:class{constructor(f){this.f=f}get(k){return this.f.values?.[k]}getAll(k){return this.f.arrays?.[k]||[]}},fetch:null,console};
const tail=/  load\(\);\n\}\)\(\);\s*$/;let src=fs.readFileSync(sourcePath,'utf8');assert(tail.test(src),'known topic-map initialization boundary');
const initializationSource=src.replace(tail,'globalThis.initialState=state;})();');
src=src.replace(tail,`globalThis.audit={readSample,readWork,readerDialog,replaceSamplesDialog,submitDialog,topicBoundary,discussionUnits,discussionGroups,discussionsView,performancePair,curated,researchCoverage,researchPhases,citationText,citationSpans,bodyCitations,readerDiscussions,allCitations,sharesDiscussion,anglesFrom,opportunitiesFrom,anglesView,productContent,sceneList,commentFragmentCount,comparisonScopeNote,set(s,d,stack=[]){snapshot=s;dialogState=d;dialogStack=stack;state.domainRef='domain';state.topicRef='topic-A';},get(){return dialogState;}};drawDialog=()=>{};render=()=>{};load=async()=>{};openDialog=(type,data)=>{if(dialogState)dialogStack.push(dialogState);dialogState={type,...data};};})();`);vm.runInNewContext(src,ctx);
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
 const stale={workRef:'A',title:'Synthetic A',readable:true,platform:'xhs',topicRefs:['topic-A'],research:{fragments:[{fragmentId:'old-comment',field:'studied_comment',text:'RESTRICTED_OLD_COMMENT'}],output:{journey:{rationale:'RESTRICTED_OLD_INTERPRETATION'}},core:{coverage:{state:'partial',sourceChars:80,coveredChars:20,totalWindows:4,completedWindows:1},units:[{unitId:'old-unit',statement:'RESTRICTED_OLD_CORE_STATEMENT',label:'RESTRICTED_OLD_CORE_LABEL',rationale:'RESTRICTED_OLD_CORE_RATIONALE',speakerRole:'commenter',evidenceRole:'challenge',status:'matched',assignments:[{topicRef:'topic-A',label:'RESTRICTED_OLD_ASSIGNMENT',reason:'RESTRICTED_OLD_ASSIGNMENT_REASON'}],evidence:[{fragmentId:'old-comment',start:0,end:10}]}]}},annotation:{rationale:'RESTRICTED_OLD_ANNOTATION'},evidenceFragment:{text:'RESTRICTED_OLD_FALLBACK'}};
 const b={...stale,workRef:'B',title:'UNRELATED_B',topicRefs:['topic-B']};
 const snap={scope:{recentReferenceWorkRefs:['A','B'],ownIdentityState:'known'},topics:[{topicRef:'topic-A',workRefs:['A']},{topicRef:'topic-B',workRefs:['B']}],works:[stale,b],statistics:{platforms:[]}};
 const fresh={...stale,research:{fragments:[{fragmentId:'new-comment',field:'studied_comment',text:'CURRENT_ALLOWED_COMMENT'},{fragmentId:'new-core-comment',field:'studied_comment',text:'CURRENT_EVIDENCE'}],output:{journey:{rationale:'CURRENT_ALLOWED_INTERPRETATION'}},core:{coverage:{state:'complete',sourceChars:23,coveredChars:23,totalWindows:1,completedWindows:1},units:[{unitId:'fresh-unit',statement:'CURRENT_ALLOWED_CORE',label:'当前合格讨论',speakerRole:'commenter',evidenceRole:'challenge',status:'matched',assignments:[{topicRef:'topic-A',label:'当前主题',reason:'CURRENT_ALLOWED_REASON'}],evidence:[{fragmentId:'new-core-comment',start:0,end:7}]}]}}};
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
   assert.equal(html.includes('CURRENT_ALLOWED_CORE'),mode==='qualified');assert.equal(html.includes('CURRENT_ALLOWED_REASON'),mode==='qualified');
 }
 console.log('PASS: inline + standalone × fresh null/missing/failure/qualified; current body independent');
 const parent={type:'judge',topicRef:'topic-A',works:[stale],loading:false,readError:'failed expanded scope'};ctx.audit.set(snap,{type:'replace',judgeState:parent},[parent]);
 assert(!ctx.audit.replaceSamplesDialog().includes('UNRELATED_B'));
 await ctx.audit.submitDialog({preventDefault(){},target:{dataset:{form:'replace-samples'},arrays:{workRefs:['B']}}});
 assert.deepEqual(parent.works.map(w=>w.workRef),['A']);
 console.log('PASS: failed full-topic read cannot offer or accept unrelated topic identity');
 const slots=[];ctx.fetch=url=>new Promise(resolve=>slots.push({url,resolve}));const sameTopicB={...b,topicRefs:['topic-A']},judge={type:'judge',topicRef:'topic-A',works:[stale,sameTopicB]};ctx.audit.set(snap,judge);
 const pa=ctx.audit.readSample('A'),pb=ctx.audit.readSample('B');
 assert(!ctx.audit.readerDialog(judge.inlineReader).includes('RESTRICTED_OLD_'),'pending qualification hides old discussion statements, reasons and quotations');
 function release(batch,ref){batch.forEach(({url,resolve})=>resolve({ok:true,json:async()=>url.includes('/topic-map?')?{works:[{...fresh,workRef:ref}]}:url.includes('/comments')?{items:[],total:0}:{item:{}}}));}
 release(slots.slice(3),'B');await pb;release(slots.slice(0,3),'A');await pa;assert.equal(judge.inlineReader.work.workRef,'B');
 console.log('PASS: reverse completion order cannot overwrite the latest selected identity');
 slots.length=0;const pc=ctx.audit.readSample('A');ctx.audit.set(snap,null);release(slots,'A');await pc;assert.equal(ctx.audit.get(),null);
 console.log('PASS: close during pending source requests cannot reopen or crash');

 const definition={topicRef:'topic-A',displayName:'任务启动',definitionText:'围绕开始第一步的困难与支持。',definitionVersion:1,workRefs:['core-A','core-B'],core:{definitionSource:'manual',inclusionCriteria:['开始任务时的困难','<img src=x onerror=alert(1)>'],exclusionCriteria:['已经开始后的持续管理'],discussionCount:3,evidenceRoles:{support:1,challenge:2,context:0},relations:[{topicRef:'topic-C',displayName:'同名但范围不同',relation:'distinct',reason:'已经开始后的维持，边界不同。'}]}};
 const sharedName={...definition,topicRef:'topic-C',core:{...definition.core,inclusionCriteria:['持续投入任务'],exclusionCriteria:['尚未开始的困难']},workRefs:['core-C']};
 const other={...definition,topicRef:'topic-B',displayName:'支持条件'};
 const raw='前'.repeat(100)+'作者😀原声与尾段';
 const fragment={fragmentId:'core-A.body.tail',field:'body',start:100,end:108,text:'作者😀原声与尾段'};
 const comment={fragmentId:'core-A.comment.tail',field:'studied_comment',start:30,end:40,text:'否定作者，未必有效。'};
 const support={unitId:'unit-author',label:'第一步可以缩小',statement:'MODEL_STATEMENT <script>alert(1)</script>',speakerRole:'author',evidenceRole:'support',rationale:'本条引用作者提出的方法。',status:'matched',assignments:[{topicRef:'topic-A',definitionRef:'def-A',label:'起步困难',reason:'讨论的是如何开始第一步。'}],evidence:[{fragmentId:fragment.fragmentId,start:102,end:105},{fragmentId:fragment.fragmentId,start:102,end:105}]};
 const challenge={unitId:'unit-comment',label:'方法并非对我有效',statement:'评论者描述自己的不同经历。',speakerRole:'commenter',evidenceRole:'challenge',rationale:'作者提法与评论经历分别保留。',status:'matched',assignments:[{topicRef:'topic-A',definitionRef:'def-A',label:'开始行动',reason:'仍在讨论启动困难。'},{topicRef:'topic-B',definitionRef:'def-B',label:'环境支持',reason:'同时说明了支持条件。'}],evidence:[{fragmentId:comment.fragmentId,start:30,end:34}]};
 const coreA={workRef:'core-A',title:'作者与评论的不同经历',readable:true,platform:'xhs',topicRefs:['topic-A','topic-B'],followerCount:50,likes:800,publishedAt:'2026-10-01',research:{fragments:[fragment,comment],output:{discussions:[{label:'LEGACY_SHOULD_NOT_REAPPEAR'}],journey:{rationale:'已有旅程解释仍然可读。'}},core:{coverage:{sourceChars:800,coveredChars:200,totalWindows:4,completedWindows:1,state:'partial'},units:[support,{...support},challenge]}}};
 const coreB={...coreA,workRef:'core-B',title:'不同名称但同主题',likes:10,topicRefs:['topic-A'],research:{fragments:[{...comment,fragmentId:'core-B.comment.tail'}],core:{coverage:{sourceChars:10,coveredChars:10,totalWindows:1,completedWindows:1,state:'complete'},units:[{...challenge,unitId:'unit-B',label:'迟迟动不了手',assignments:[{topicRef:'topic-A',definitionRef:'def-A-v2',label:'任务开始',reason:'仍属同一主题，保留定义版本。'}],evidence:[{fragmentId:'core-B.comment.tail',start:30,end:34}]}]}}};
 const coreC={...coreB,workRef:'core-C',title:'同名但不同主题',research:{...coreB.research,core:{...coreB.research.core,units:[{...support,unitId:'unit-C',label:'第一步可以缩小',assignments:[{topicRef:'topic-C',definitionRef:'def-C',label:'任务启动',reason:'描述持续投入，不属于开始第一步。'}],evidence:[]}]}}};
 const coreSnapshot={scope:{recentReferenceWorkRefs:['core-A','core-B','core-C'],ownIdentityState:'known'},topics:[definition,other,sharedName],works:[coreA,coreB,coreC],statistics:{platforms:[]}};
 ctx.audit.set(coreSnapshot,null);
 const groups=ctx.audit.discussionGroups([coreA,coreA,coreB,coreC]);
 const primary=groups.find(g=>g.key==='topic:topic-A'),secondary=groups.find(g=>g.key==='topic:topic-B');
 assert.equal(ctx.audit.discussionUnits(coreA).length,2,'repeated unitId is one discussion');
 assert.equal(primary.rows.length,3);assert.equal(primary.works.length,2);assert.equal(primary.evidenceCount,3,'repeated citations and repeated work rows do not inflate evidence');
 assert.equal(secondary.rows.length,1,'one discussion can have multiple real assignments');
 assert.equal(groups.filter(g=>g.label==='任务启动').length,2,'same display name does not merge different topic identities');
 assert(ctx.audit.sharesDiscussion(coreA,coreB),'same canonical topic survives different labels and definition versions');
 assert(!ctx.audit.sharesDiscussion(coreA,coreC),'same label is not semantic equivalence');
 const paired=ctx.audit.performancePair([coreA,coreC,coreB]);
 assert(paired.includes('不同名称但同主题'));assert(!paired.includes('同名但不同主题'));
 const curated=ctx.audit.curated('topic-A',coreSnapshot);
 assert.deepEqual(Array.from(curated,w=>w.workRef),['core-A','core-B'],'curation uses canonical discussion identity for contrast');
 console.log('PASS: stable topic identity, changed labels/definitions, multilabel discussion and citation/works deduplication');

 const legacy=(ref,topicRef)=>({workRef:ref,research:{output:{discussions:[{label:'任务启动',topicRef,evidence:[]}]}}});
 assert.equal(ctx.audit.discussionGroups([legacy('legacy-A'),legacy('legacy-B')]).length,2,'unnamed legacy identity cannot merge by display label');
 assert(ctx.audit.sharesDiscussion(legacy('legacy-C','topic-A'),coreB),'saved legacy canonical identity remains usable');
 const emptyCore={...coreA,research:{...coreA.research,core:{...coreA.research.core,units:[]}}};
 assert.equal(ctx.audit.discussionGroups([emptyCore]).length,0,'explicitly empty core must not resurrect old discussions');
 assert(!ctx.audit.readerDiscussions(emptyCore,raw).includes('LEGACY_SHOULD_NOT_REAPPEAR'));
 assert(ctx.audit.readerDiscussions(emptyCore,raw).includes('当前已处理范围未形成具体讨论'));
 assert.equal(ctx.audit.allCitations(emptyCore).length,0,'legacy discussion citations are not revived with empty core');
 const pending={...coreA,workRef:'pending',research:{...coreA.research,core:{...coreA.research.core,units:[{...support,unitId:'uncertain',status:'uncertain',speakerRole:'unknown',reason:'缺少启动前后的时间条件。'},{...support,unitId:'outside',status:'out_of_scope',speakerRole:'quoted',evidenceRole:'context',reason:'所述场景明确超出本领域。'}]}}};
 assert(!ctx.audit.sharesDiscussion(pending,coreA),'uncertain and out-of-scope discussions are not confirmed assignments');
 const rendered=ctx.audit.discussionsView([coreA,coreB,pending,legacy('old')]);
 for(const text of ['作者表达','评论者原声','引用或转述','来源角色未知','反例或不同经验','背景材料','归属理由：','同一条讨论涉及多个主题','归属尚未确定','当前领域外','旧研究 · 边界未补齐','MODEL_STATEMENT &lt;script&gt;alert(1)&lt;/script&gt;'])assert(rendered.includes(text),text);
 assert(!rendered.includes('<script>'));assert(!rendered.includes('<blockquote>MODEL_STATEMENT'));
 assert(rendered.includes('data-action="reader"'),'evidence entry continues through the existing reader qualification');
 console.log('PASS: old results, explicit empty core, unresolved/out-of-scope, author/comment/quoted roles and escaped statements');

 const boundary=ctx.audit.topicBoundary(definition,coreSnapshot);
 assert(boundary.includes('纳入条件'));assert(boundary.includes('已经开始后的持续管理'));assert(boundary.includes('人工维护的定义'));assert(boundary.includes('当前范围 3 条独立讨论'));assert(boundary.includes('背景 0'));assert(boundary.includes('边界不同'));
 assert(boundary.includes('&lt;img src=x onerror=alert(1)&gt;'));assert(!boundary.includes('<img'));
 assert(ctx.audit.topicBoundary({topicRef:'old',definitionText:'旧定义'}).includes('边界未补齐'));
 assert(ctx.audit.topicBoundary({...definition,core:{inclusionCriteria:[],exclusionCriteria:[]}}).includes('当前范围 — 条独立讨论'),'unknown count is not zero');
 const partial=ctx.audit.researchCoverage(coreA);
 assert(partial.includes('部分来源已处理'));assert(partial.includes('200 / 800 字符'));assert(partial.includes('1 / 4 段来源'));assert(!partial.includes('当前可读来源已处理'));
 assert(ctx.audit.researchCoverage(coreB).includes('当前可读来源已处理'));
 assert(ctx.audit.researchCoverage({...coreA,research:{core:{coverage:{...coreA.research.core.coverage,state:'complete'}}}}).includes('部分来源已处理'),'inconsistent completion must not appear complete');
 assert(ctx.audit.researchCoverage(legacy('legacy')).includes('处理范围未记录'));
 assert(ctx.audit.researchCoverage({research:{core:{coverage:{}}}}).includes('— / — 字符'));
 const phases=ctx.audit.researchPhases({phases:{extractQueued:0,extracting:1,resolveQueued:2}});
 assert(phases.includes('提炼讨论'));assert(phases.includes('判断归属'));assert(phases.includes('待处理 0'));assert(phases.includes('进行中 —'));assert(!phases.includes('extractQueued'));
 console.log('PASS: inclusion/exclusion, provenance, known zero vs unknown, partial coverage and separate research progress');

 assert.equal(ctx.audit.citationText(coreA,support.evidence),'😀原声','nonzero source window uses Unicode scalar offsets');
 assert.equal(JSON.stringify(ctx.audit.citationSpans(coreA,fragment.fragmentId)),JSON.stringify([{start:2,end:5}]));
 assert.equal(JSON.stringify(ctx.audit.bodyCitations(coreA,raw)),JSON.stringify([{start:102,end:105}]));
 assert.equal(ctx.audit.bodyCitations(coreA,'不同的当前正文').length,0,'body highlight requires matching current source text');
 assert.equal(ctx.audit.citationText(coreA,[{fragmentId:fragment.fragmentId,start:2,end:5}]),'','absolute span cannot be guessed as a local span');
 const reader=ctx.audit.readerDialog({work:coreA,resource:{item:{inspector:{detailCurrent:{body:{value:raw}}}}},commentResource:{items:[],total:0}});
 assert(reader.includes('<mark>😀原声</mark>'));assert(reader.includes('<mark>否定作者</mark>'));
 assert(reader.includes('href="#lgi-tm-source-body-core-A"'));assert(reader.includes('id="lgi-tm-source-body-core-A"'));
 assert(reader.includes('href="#lgi-tm-source-fragment-core-A.comment.tail"'));assert(reader.includes('id="lgi-tm-source-fragment-core-A.comment.tail"'));
 assert(reader.includes('已有旅程解释仍然可读。'));assert(reader.includes('部分来源已处理'));assert(!reader.includes('<script>'));
 for(const block of [{loading:true},{resourceError:'原文已撤回'},{qualificationError:'来源不再合格'}]){
   const hidden=ctx.audit.readerDialog({work:coreA,...block});
   assert(!hidden.includes('MODEL_STATEMENT'));assert(!hidden.includes('评论者描述自己的不同经历'));assert(!hidden.includes('讨论的是如何开始第一步。'));assert(!hidden.includes('😀原声'));
 }
 console.log('PASS: qualified reader, Unicode tail-window source anchors/highlights and withdrawn derived content');

 const commentOnly={...coreA,readable:false,research:{...coreA.research,readable:true,authorSourceState:'source_unavailable',commentSourceState:'available',resultRef:null,resultRefs:[],fragments:[comment],output:{},core:{...coreA.research.core,units:[challenge]}}};
 const commentSnapshot={...coreSnapshot,works:[commentOnly],scope:{...coreSnapshot.scope,recentReferenceWorkRefs:[commentOnly.workRef]}};
 assert.equal(ctx.audit.curated('topic-A',commentSnapshot).length,1,'qualified comment research enters curation without author text');
 for(const entry of ['sample','standalone']) {
   const judge={type:'judge',topicRef:'topic-A',works:[commentOnly],snapshot:commentSnapshot};ctx.audit.set(commentSnapshot,judge);
   ctx.fetch=async url=>url.includes('/topic-map?')?{ok:true,json:async()=>commentSnapshot}:url.includes('/comments')?{ok:true,json:async()=>({items:[{body:'当前合格评论'}],total:1})}:{ok:false,status:404,json:async()=>({code:'material_not_found'})};
   await(entry==='sample'?ctx.audit.readSample(commentOnly.workRef):ctx.audit.readWork(commentOnly.workRef));
   const currentReader=entry==='sample'?judge.inlineReader:ctx.audit.get(),html=ctx.audit.readerDialog(currentReader);
   assert(html.includes('评论者描述自己的不同经历'),'independently qualified comments survive unavailable author resource');
   assert(html.includes('作者原文当前未取得或不可用'));assert(html.includes('已有合格评论与讨论继续可读'));
   assert(!html.includes('MODEL_STATEMENT'));assert(!html.includes('已有旅程解释仍然可读。'));
 }
 const maskedBody=ctx.audit.readerDialog({work:commentOnly,resource:{item:{inspector:{detailCurrent:{body:{value:'WITHDRAWN_AUTHOR_BODY'}}}}},commentResource:{items:[],total:0}});
 assert(!maskedBody.includes('WITHDRAWN_AUTHOR_BODY'),'explicit author unreadability cannot expose an old body response');
 ctx.audit.set(commentSnapshot,{type:'replace',judgeState:{topicRef:'topic-A',works:[commentOnly],snapshot:commentSnapshot}});
 assert(ctx.audit.replaceSamplesDialog().includes(commentOnly.title),'qualified comment-only material can replace the selected sample');
 const restrictedBoundary=ctx.audit.topicBoundary({...definition,core:{...definition.core,sourceState:'source_unavailable',inclusionCriteria:['RESTRICTED_CRITERION']}});
 assert(restrictedBoundary.includes('主题定义的来源当前受限'));assert(!restrictedBoundary.includes('RESTRICTED_CRITERION'));assert(!restrictedBoundary.includes('纳入边界尚未补齐'));
 const draft={...commentOnly,research:{...commentOnly.research,output:{angles:[{label:'UNFINISHED_ANGLE'}],productOpportunities:[{need:'UNFINISHED_OPPORTUNITY'}]}}};
 assert.equal(ctx.audit.anglesFrom([draft]).length,0);assert.equal(ctx.audit.opportunitiesFrom([{work:draft,output:draft.research.output}]).length,0,'accepted partial units do not authorize saving unfinished model drafts');
 console.log('PASS: current comment-only qualification, missing author source, restricted definition and unfinished result origins');

 const combined={workRef:'combined',research:{resultRef:'latest-window',methodVersion:'topic-map.research.v2',output:{angles:[{label:'第一窗角度',researchResultRef:'first-window',researchMethodVersion:'topic-map.research.v1',researchAngleIndex:4},{label:'第二窗角度',researchResultRef:'second-window',researchAngleIndex:0},{label:'旧结果角度'}],productOpportunities:[{need:'第一窗需求',researchResultRef:'first-window',researchOpportunityIndex:2},{need:'第二窗需求',researchResultRef:'second-window',researchMethodVersion:'topic-map.research.v2',researchOpportunityIndex:0},{need:'旧结果需求'}]}}};
 const angles=ctx.audit.anglesFrom([combined]),opportunities=ctx.audit.opportunitiesFrom([{work:combined,output:combined.research.output}]);
 assert.deepEqual(Array.from(angles,a=>[a.researchResultRef,a.researchAngleIndex]),[['first-window',4],['second-window',0],['latest-window',2]]);
 assert.deepEqual(Array.from(opportunities,a=>[a.researchResultRef,a.researchOpportunityIndex]),[['first-window',2],['second-window',0],['latest-window',2]]);
 let savedPayload=null;
 ctx.fetch=async(url,options)=>{assert.equal(url,'/api/local/topic-map/commands');savedPayload=JSON.parse(options.body);return {ok:true,json:async()=>({subjectRef:'saved',receiptRef:'receipt'})};};
 for(const [kind,angle,indexField] of [['angle',angles[0],'researchAngleIndex'],['product_research',opportunities[1],'researchOpportunityIndex']]){
   ctx.audit.set(coreSnapshot,{type:'save',topicRef:'topic-A',kind,angle});
   await ctx.audit.submitDialog({preventDefault(){},target:{dataset:{form:'save'},values:{title:'合成保存验证',angle:'保留原窗口依据',rationale:'待核对来源'}}});
   assert.equal(savedPayload.researchResultRef,angle.researchResultRef);assert.equal(savedPayload[indexField],angle[indexField]);
   assert.equal(savedPayload.methodVersion,kind==='angle'?'topic-map.research.v1':'topic-map.research.v2','each result preserves its own method version across mixed legacy/current windows');
   assert.equal(savedPayload[kind==='angle'?'researchOpportunityIndex':'researchAngleIndex'],null);
 }
 console.log('PASS: combined-window angle/opportunity origins survive the actual save command, including original index zero');

 const crossBody={...fragment,workRef:'core-A',sourceRef:'author-source'},crossComment={...comment,fragmentId:'core-B.comment.tail',workRef:'core-B',sourceRef:'comment-source'};
 const crossEvidence=[{fragmentId:crossBody.fragmentId,start:102,end:105},{fragmentId:crossComment.fragmentId,start:30,end:34}];
 const comparisonScope={resultRef:'shared-comparison',scopeWorkRefs:['core-A','core-B','not-selected'],selectedWorkRefs:['core-A','core-B'],state:'partial',boundary:'只覆盖选入的讨论；<script>不得作为市场结论</script>'};
 const sharedAngle={label:'同名角度',title:'共同的回答任务',answerTask:'需要核对两篇原作',researchResultRef:'shared-comparison',researchAngleIndex:0,evidenceWorkRefs:['core-A','core-B'],comparisonScope,evidence:crossEvidence};
 const sharedOpportunity={need:'合成需求',hypothesis:'合成支持',verificationQuestion:'先验证什么',alternativeExplanation:'保留其它解释',researchResultRef:'shared-comparison',researchOpportunityIndex:0,evidenceWorkRefs:['core-A','core-B'],comparisonScope,evidence:crossEvidence};
 const compared=work=>({...work,research:{...work.research,resultRef:'shared-comparison',methodVersion:'topic-map.research.v2',fragments:[crossBody,crossComment],output:{angles:[sharedAngle,{...sharedAngle,researchResultRef:work.workRef+'-independent',evidenceWorkRefs:[work.workRef],comparisonScope:null}],productOpportunities:[sharedOpportunity],scenes:[{label:'共同比较场景',evidence:crossEvidence,comparisonScope}],responseMatches:[{status:'partial',evidence:crossEvidence,comparisonScope}]}}});
 const comparedA=compared(coreA),comparedB=compared(coreB),comparedWorks=[comparedB,comparedA];
 ctx.audit.set({...coreSnapshot,works:comparedWorks},null);
 const crossAngles=ctx.audit.anglesFrom(comparedWorks),crossOpportunities=ctx.audit.opportunitiesFrom(comparedWorks.map(work=>({work,output:work.research.output})));
 assert.equal(crossAngles.length,3,'one shared comparison, plus each distinct independent result; labels do not determine identity');
 assert.equal(crossOpportunities.length,1,'one comparison is not duplicated across participating works');
 assert.equal(crossAngles[0].workRef,'core-B','saving starts from the non-primary participating work');
 const scopedAngles=ctx.audit.anglesView(comparedWorks),scopedProducts=ctx.audit.productContent(comparedWorks.map(work=>({work,output:work.research.output})));
 for(const html of [scopedAngles,scopedProducts]){assert(html.includes('选取 2 / 3 篇作品'));assert(html.includes('&lt;script&gt;'));assert(!html.includes('<script>'));}
 const crossReader=ctx.audit.readerDialog({work:comparedB,resource:{item:{inspector:{detailCurrent:{body:{value:raw}}}}},commentResource:{items:[],total:0}});
 assert(crossReader.includes('作者正文 · 来自《作者与评论的不同经历》'),'another work’s original text retains its owner in the reader');
 assert(crossReader.includes('选取 2 / 3 篇作品'),'response relationships retain their actual comparison scope');
 assert.equal(ctx.audit.bodyCitations(comparedB,raw).length,0,'matching foreign text cannot become the current author’s body citation');
 assert(ctx.audit.sceneList(comparedWorks).includes('2 篇独立来源作品 · 2 条去重定位依据'));
 assert.equal(ctx.audit.commentFragmentCount(comparedWorks),1,'reprojected comment fragments count once');
 const unknownOwner={...comparedA,research:{...comparedA.research,fragments:[{...crossBody,workRef:null},crossComment]}};
 assert.equal(ctx.audit.bodyCitations(unknownOwner,raw).length,0,'explicitly unknown owner is not inferred from a matching fragment ID');
 assert(ctx.audit.readerDialog({work:unknownOwner,resource:{item:{inspector:{detailCurrent:{body:{value:raw}}}}},commentResource:{items:[],total:0}}).includes('原文所属作品未知'));
 assert(ctx.audit.sceneList([unknownOwner]).includes('1 篇已知来源作品 · 2 条去重定位依据'));
 for(const [kind,angle,indexField] of [['angle',crossAngles[0],'researchAngleIndex'],['product_research',crossOpportunities[0],'researchOpportunityIndex']]){
   ctx.audit.set({...coreSnapshot,works:comparedWorks},{type:'save',topicRef:'topic-A',kind,angle});
   await ctx.audit.submitDialog({preventDefault(){},target:{dataset:{form:'save'},values:{title:'非主作品保存比较',angle:'保留两侧依据',rationale:'只保存本次比较'}}});
   assert.equal(savedPayload.researchResultRef,'shared-comparison');assert.equal(savedPayload[indexField],0);
   assert.deepEqual(savedPayload.evidenceWorkRefs,['core-A','core-B'],'the original selected participant refs include the physical primary');
 }
 console.log('PASS: shared comparisons deduplicate, retain source ownership/scope, and save from either work with original zero indexes');
})().catch(e=>{console.error(e);process.exitCode=1});
