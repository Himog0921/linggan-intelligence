#!/usr/bin/env node
// Synthetic UI proof only; not a PostgreSQL, provider, production, or independent-review proof.
import assert from 'node:assert/strict';
import {readFileSync, writeFileSync, mkdtempSync, rmSync} from 'node:fs';
import {resolve, dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {tmpdir} from 'node:os';
import {createServer} from 'node:http';
import {spawn} from 'node:child_process';
import vm from 'node:vm';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'..');
const assets=resolve(root,'apps/api/src/local_web');
const source=readFileSync(resolve(assets,'comment_study_results.js'),'utf8');
const css=readFileSync(resolve(assets,'comment_study_results.css'),'utf8');
const context={module:{exports:{}},HTMLElement:class {},Intl,Date,Map,JSON,Number,String,Error};
vm.runInNewContext(source,context);
const api=context.module.exports;
let unit=0;
for(const value of [null,undefined,'4',NaN,-1]){assert.equal(api.count(value),'—');unit++;}
assert.equal(api.count(0),'0');unit++;
assert.equal(api.escape('<img onerror="x">'), '&lt;img onerror=&quot;x&quot;&gt;');unit++;
assert.equal(api.readable({sourceState:'unknown',commentText:'secret'}),false);unit++;
assert.equal(api.readable({sourceState:'known',commentText:' '}),false);unit++;
assert.throws(()=>api.unique([{}]));unit++;
const row={commentKey:{workRef:'work',commentExternalId:'a'}};
assert.equal(api.unique([row,row,{commentKey:{workRef:'other',commentExternalId:'a'}}]).length,2);unit++;
const instance=new api.StudyUserProblems();
const hidden=instance.card({problemRef:'p',title:'SECRET-TITLE',definition:'SECRET-DEFINITION',definitionReadable:false,preview:{sourceState:'restricted',commentText:'SECRET-VOICE'}});
assert(!hidden.includes('SECRET-'));unit++;
assert(!css.match(/(?:^|\n)\s*(?:body|h[1-6]|button|input)\s*\{/));unit++;
assert(!source.includes('localStorage')&&!source.includes('sessionStorage'));unit++;
// The legacy host normalizes the URL before the new asset executes. Preserve the
// original bookmarked problem scope without changing that shared/overview host.
const incoming=new URLSearchParams('view=problems&domain=d&q=needle&state=changes&sort=support&cursor=page-two');
const routing={...context,module:{exports:{}},URLSearchParams,initialStudyRoute:incoming,location:{search:'?view=problems&domain=d'},history:{state:null}};
vm.runInNewContext(source,routing);
const restored=new routing.module.exports.StudyUserProblems();restored.domain='d';restored.readRoute();
assert.equal(restored.q,'needle');unit++;
assert.equal(restored.view,'changes');unit++;
assert.equal(restored.order,'support');unit++;
assert.equal(restored.cursor,'page-two');unit++;
console.log(`PASS ${unit} deterministic checks`);
if(process.argv.includes('--unit-only'))process.exit(0);
const tokens=readFileSync(process.env.COMMENT_RESULTS_TEST_TOKENS||resolve(assets,'lids_tokens.css'),'utf8');
const fixture=String.raw`
const DOMAIN='11111111-1111-4111-8111-111111111111';
let domainRef=DOMAIN,activeView='overview',selectedProblemRef=null,originalCalls={overview:0,comments:0,runs:0};
const signalKindLabel={problem:'问题',solution:'建议'},pairStateLabel={},pairDecisionLabel={};
const originalOverview=async()=>{originalCalls.overview++;return '<section class="study-overview"><h2 id="protected-title">概览保持原样</h2><div class="study-observation">7 / 28 / 56</div></section>';};
const TAB_RENDERERS={overview:originalOverview,comments:async()=>{originalCalls.comments++;return '<div class="study-comments"><table class="study-review-table study-comments-table"><tbody><tr><td>保留原评论控件</td></tr></tbody></table></div>';},runs:async()=>{originalCalls.runs++;return '<table class="study-review-table"><tr><td><button data-run-control="pause">暂停派发</button></td></tr></table>';},problems:async()=>''};
let renderToken=0;
async function renderActiveTab(){const token=++renderToken;const html=await TAB_RENDERERS[activeView]();if(token===renderToken)document.querySelector('#study-tab-result').innerHTML=html;}
async function navigateToRun(){activeView='runs';await renderActiveTab();}
window.addEventListener('popstate',()=>{activeView=new URLSearchParams(location.search).get('view')||'overview';void renderActiveTab();});
const requests=[],config={fail:false,restricted:false,wrongDomain:false,slow:false};
const ids=['aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa','bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb','cccccccc-cccc-4ccc-8ccc-cccccccccccc'];
const key={workRef:'dddddddd-dddd-4ddd-8ddd-dddddddddddd',commentExternalId:'comment-a'};
const voices=[{commentKey:key,sourceState:'known',authorDisplayName:'合成账号',commentText:'合成原声：尝试只维持了两天。',addedAt:'2026-10-14T00:00:00Z',signals:[{kind:'problem',proposition:'合成归纳'}]}, {commentKey:{...key,commentExternalId:'comment-b'},sourceState:'known',commentText:'合成原声：我不知道下一步怎样继续。',addedAt:'2026-10-13T00:00:00Z'}];
function problems(){return ids.map((id,i)=>({problemRef:id,revisionRef:'revision-'+i,identityVersion:i===0?2:1,definitionReadable:!(config.restricted&&i===0),definitionCurrent:true,title:config.restricted&&i===0?null:['试过多种办法，困难依然反复出现','知道需要鼓励，却不知道当下怎么表达','家里做法不一致，约定很难持续下去'][i],definition:config.restricted&&i===0?null:'合成研究定义：具体情境与期待仍需结合来源理解。',stableIdentity:{actor:'合成用户',context:'合成情境',goalOrExpectedState:'希望有所改善',barrierOrUnmetNeed:'安排难以持续'},supportCommentCount:i===0?8:i===1?0:null,supportAuthorCount:i===0?7:i===1?0:null,supportWorkCount:i===0?1:i===1?0:null,recentAddedSupportCommentCount:2,changeKind:['definition_revision','first_record','evidence_added'][i],updatedAt:'2026-10-14T00:00:00Z',preview:i!==0||config.restricted?null:voices[0]}));}
window.fetch=async(input,init={})=>{
 const u=new URL(input);requests.push({path:u.pathname,q:u.searchParams.get('q'),cursor:u.searchParams.get('cursor'),method:init.method||'GET'});
 if(config.slow&&u.pathname.includes(ids[0]))await new Promise(r=>setTimeout(r,120));
 if(init.signal?.aborted)throw new DOMException('Aborted','AbortError');
 let data,status=200;const path=u.pathname;
 if(path.endsWith('/problem-cards')){
  if(config.fail){status=503;data={error:{message:'测试读取失败'}};}else{let rows=problems();if(u.searchParams.get('q')==='无匹配')rows=[];if(u.searchParams.get('cursor'))rows=rows.slice(2);data={contract:'comment-study.problem-cards.v1',domainRef:config.wrongDomain?'wrong':DOMAIN,totalCount:rows.length?7:0,problems:rows,page:{nextCursor:!u.searchParams.get('cursor')&&rows.length?'test-cursor':null}};}
 }else if(path.endsWith('/problem-candidates'))data={domainRef:DOMAIN,expressions:[{...voices[0],signalRef:'clue-a',kind:'solution',state:'deferred_novel',proposition:'合成建议，不作为问题结论',runRef:'run-a'}],page:{}};
 else if(path.endsWith('/comments/detail'))data={domainRef:DOMAIN,commentKey:{workRef:u.searchParams.get('workRef'),commentExternalId:u.searchParams.get('commentExternalId')},source:{sourceState:config.restricted?'restricted':'known'},comment:{commentText:voices[0].commentText},work:{displayTitle:'合成作品标题'},parentContext:{commentText:'合成父评论：前面的安排是什么？'}};
 else if(path.endsWith('/evidence')){const id=path.split('/').at(-2);data={domainRef:DOMAIN,problemRef:id,evidence:config.restricted?[]:[...voices,voices[0]],page:{nextCursor:u.searchParams.get('cursor')?null:'evidence-next'}};}
 else if(path.includes('/problems/')){const p=problems().find(p=>path.endsWith(p.problemRef));data={domainRef:DOMAIN,problem:p,revisionHistory:[{identityVersion:2,definitionReadable:!config.restricted,title:config.restricted?null:'已保存版本',definition:config.restricted?null:'合成历史定义',createdAt:'2026-10-14T00:00:00Z'}],sourceDistribution:{works:[{workTitle:'合成作品',commentCount:2}]}};}
 else {status=404;data={error:{message:'未预期的测试路径'}};}
 return {ok:status===200,status,json:async()=>data};
};
const $=s=>document.querySelector(s);
const wait=async(fn)=>{for(let i=0;i<100;i++){if(fn())return;await new Promise(r=>setTimeout(r,10));}throw new Error('wait timed out');};
const checks=[];function check(ok,label){if(!ok)throw new Error(label);checks.push(label);}
async function go(view){activeView=view;const u=new URL(location.href);u.search='';u.searchParams.set('domain',DOMAIN);u.searchParams.set('view',view);history.replaceState({},'',u);await renderActiveTab();}
`;
const tests=String.raw`
(async()=>{
try{
 check(TAB_RENDERERS.overview===originalOverview,'overview renderer untouched');
 check(originalCalls.overview===0,'asset does not invoke overview');
 await go('overview');const protectedHTML=$('#study-tab-result').innerHTML;const protectedColor=getComputedStyle($('#protected-title')).color;
 await go('problems');await wait(()=>$('.sr3-card'));
 check(document.querySelectorAll('.sr3-card').length===3,'three real-DOM cards');
 check($('[data-sr3-status]').textContent.includes('7'),'server total is not current page length');
 check(getComputedStyle($('.sr3-card-title')).webkitLineClamp==='2','two-line heading');
 check(!$('.sr3-card').innerHTML.includes('需求增长'),'no invented trend');
 const before=$('.sr3-card').getBoundingClientRect().height;
 $('[data-sr3=density][data-value=compact]').click();
 check($('.sr3-card').getBoundingClientRect().height<before,'compact density fits more content');
 $('[data-sr3=density][data-value=standard]').click();
 const opener=$('[data-sr3=open]');opener.focus();opener.click();await wait(()=>$('.sr3-evidence'));
 check($('.sr3-drawer').open,'wide dialog opened');
 check(document.querySelectorAll('.sr3-evidence').length===2,'deduplicate work/comment identity');
 check(requests.filter(r=>r.path.endsWith('/'+ids[0])).length===1,'single click gives one detail read');
 $('[data-sr3=context]').click();await wait(()=>$('.sr3-context').textContent.includes('合成父评论'));
 check($('[data-sr3=context]').getAttribute('aria-expanded')==='true','context stays expanded (no double handler)');
 check($('.sr3-context').textContent.includes('只取得作品标题'),'no full-work claim from title');
 $('[data-sr3=more-evidence]').click();await wait(()=>!$('[data-sr3=more-evidence]'));
 check(document.querySelectorAll('.sr3-evidence').length===2,'paging does not duplicate quotes');
 Object.defineProperty(navigator,'clipboard',{configurable:true,value:{writeText:async t=>{window.copied=t;}}});
 $('[data-sr3=copy]').click();await wait(()=>window.copied);
 check(!window.copied.includes(voices[0].commentText),'copy is summary/locator not raw bulk export');
 check(window.copied.includes('problemRef='+ids[0]),'copy has actual problem locator');
 Object.defineProperty(navigator,'clipboard',{configurable:true,value:{writeText:async()=>{throw new Error('denied');}}});
 $('[data-sr3=copy]').click();await wait(()=>$('.sr3-copy-text'));
 check($('[data-sr3-feedback]').textContent.includes('未自动复制'),'copy rejection not reported as success');
 $('.sr3-drawer').dispatchEvent(new Event('cancel',{cancelable:true}));
 check(!$('.sr3-drawer').open,'Escape-equivalent cancel closes');
 check(document.activeElement===opener,'focus returns to opening card');
 $('#sr3-query').value='无匹配';$('[data-sr3-search]').dispatchEvent(new Event('submit',{bubbles:true,cancelable:true}));await wait(()=>$('.sr3-empty'));
 check($('.sr3-empty').textContent.includes('没有匹配'),'filtered empty distinct from no research');
 $('[data-sr3=clear]').click();await wait(()=>$('.sr3-card'));
 $('[data-sr3=view][data-value=changes]').click();await wait(()=>$('.sr3-group'));
 check(document.querySelectorAll('.sr3-group').length===2,'revision and materials grouped separately');
 $('[data-sr3=next]').click();await wait(()=>document.querySelectorAll('.sr3-card').length===1);
 check(requests.some(r=>r.cursor==='test-cursor'),'server cursor is sent back');
 $('[data-sr3=first]').click();await wait(()=>document.querySelectorAll('.sr3-card').length===3);
 config.fail=true;$('[data-sr3=refresh]').click();await wait(()=>$('[data-sr3=retry]'));
 check($('[data-sr3-status]').textContent.includes('未取得'),'read failure not shown as zero');
 config.fail=false;$('[data-sr3=retry]').click();await wait(()=>$('.sr3-card'));
 config.wrongDomain=true;$('[data-sr3=refresh]').click();await wait(()=>$('[data-sr3=retry]'));
 check($('[data-sr3-list]').textContent.includes('范围不一致'),'reject cross-domain result');
 config.wrongDomain=false;$('[data-sr3=retry]').click();await wait(()=>$('.sr3-card'));
 const component=$('study-user-problems');config.slow=true;
 void component.openProblem(ids[0]);void component.openProblem(ids[1]);await wait(()=>$('#sr3-drawer-title').textContent.includes('鼓励'));
 await new Promise(r=>setTimeout(r,150));check($('#sr3-drawer-title').textContent.includes('鼓励'),'late detail cannot overwrite selection');config.slow=false;
 component.close();config.restricted=true;void component.openProblem(ids[0]);await wait(()=>$('#sr3-drawer-title').textContent.includes('不可读取'));
 check(!$('[data-sr3-detail]').textContent.includes('合成历史定义'),'unreadable revision hidden');
 check($('[data-sr3=copy]').disabled,'restricted definition cannot be copied');
 component.close();config.restricted=false;
 $('[data-sr3=view][data-value=clues]').click();await wait(()=>$('[data-sr3=clue]'));
 check($('[data-sr3-search]').hidden,'do not fake unsupported candidate search');
 check($('.sr3-card').textContent.includes('建议或个人经验'),'suggestion not promoted to problem');
 $('[data-sr3=clue]').click();await wait(()=>$('#sr3-drawer-title').textContent.includes('合成建议'));
 check($('[data-sr3=copy]').hidden,'no candidate bulk copy');component.close();
 await go('comments');check(!!$('.study-results-comments .study-comments-table'),'original comments wrapped');
 await go('runs');check(!!$('.study-results-runs [data-run-control=pause]'),'run control retained');
 await go('overview');check($('#study-tab-result').innerHTML===protectedHTML,'overview DOM preserved');
 check(getComputedStyle($('#protected-title')).color===protectedColor,'overview style preserved');
 check(requests.every(r=>r.method==='GET'),'new UI never sends writes');
 await go('problems');await wait(()=>$('.sr3-card'));
 check(document.documentElement.scrollWidth<=innerWidth,'no viewport horizontal overflow');
 window.__PROOF__={ok:true,checks};
}catch(error){window.__PROOF__={ok:false,checks,error:String(error),stack:error.stack};}
 const meta=document.createElement('meta');meta.name='comment-results-proof';meta.content=btoa(String.fromCharCode(...new TextEncoder().encode(JSON.stringify(window.__PROOF__))));document.head.append(meta);
})();`;
const html=`<!doctype html><html lang="zh-CN" data-theme="linggan-intelligence"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><style>${tokens}\nbody{margin:0;padding:24px;font-family:var(--lgi-font-sans)}.v7-sr-only{position:absolute;width:1px;height:1px;clip-path:inset(50%);overflow:hidden}#protected-title{color:rgb(10,20,30)}@media(max-width:680px){body{padding:12px}}</style><link rel="stylesheet" href="/style.css"></head><body><p>合成测试材料 · 不是项目研究数据</p><main id="study-tab-result"></main><script>${fixture}</script><script src="/results.js"></script><script>${process.argv.includes('--serve')?"void go('problems')":tests}</script></body></html>`;
if(process.argv.includes('--export-offline-fixture')) {
 const destination=process.argv[process.argv.indexOf('--export-offline-fixture')+1];
 assert(destination,'An explicit temporary output path is required');
 // Offline origin/history adapters only for environments that prohibit URL navigation.
 // This mode does not validate the host router, browser Back/Forward, or HTTP transport.
 const bridge=`<script>const FixtureURL=window.URL;window.URL=class extends FixtureURL{constructor(input,base){super(input==='about:blank'?'https://fixture.invalid/fixture':input,base==='null'?'https://fixture.invalid':base);}};let fixtureHistory=null;Object.defineProperty(history,'state',{get:()=>fixtureHistory});history.pushState=history.replaceState=(state)=>{fixtureHistory=state;};</script>`;
 const offline=html.replace('<link rel="stylesheet" href="/style.css">',`<style>${css}</style>`).replace('<script src="/results.js"></script>',`<script>${source}</script>`).replace('<script>'+fixture,bridge+'<script>'+fixture);
 writeFileSync(destination,offline);console.log('Wrote offline fixture: '+destination);process.exit(0);
}
const server=createServer((req,res)=>{if(req.url.startsWith('/results.js')){res.setHeader('Content-Type','text/javascript');res.end(source);}else if(req.url.startsWith('/style.css')){res.setHeader('Content-Type','text/css');res.end(css);}else{res.setHeader('Content-Type','text/html; charset=utf-8');res.end(html);}});
await new Promise(r=>server.listen(0,'127.0.0.1',r));
const url=`http://127.0.0.1:${server.address().port}/fixture`;
if(process.argv.includes('--serve')){console.log(url);process.on('SIGTERM',()=>server.close());}
else {
 const profile=mkdtempSync(resolve(tmpdir(),'comment-results-browser-'));
 try{
  const out=await new Promise((resolve,reject)=>{
   const child=spawn(process.env.CHROMIUM_PATH||'chromium',['--headless','--no-sandbox','--disable-gpu','--disable-dev-shm-usage','--no-proxy-server',`--user-data-dir=${profile}`,'--window-size=1440,1000','--virtual-time-budget=10000','--dump-dom',url]);let stdout='',stderr='';
   const timer=setTimeout(()=>{child.kill('SIGKILL');reject(new Error('Browser proof timed out'));},45000);
   child.stdout.on('data',b=>stdout+=b);child.stderr.on('data',b=>stderr+=b);child.on('error',e=>{clearTimeout(timer);reject(e);});child.on('close',code=>{clearTimeout(timer);code===0?resolve(stdout):reject(new Error(stderr));});
  });
  const encoded=out.match(/name="comment-results-proof" content="([^"]+)"/);
  assert(encoded,'Browser did not finish its proof');const proof=JSON.parse(Buffer.from(encoded[1],'base64').toString());
  console.log(JSON.stringify(proof,null,2));assert(proof.ok,proof.error);
  console.log(`PASS ${proof.checks.length} browser assertions; API and registry are synthetic fixtures, new JS/CSS are actual assets.`);
 }finally{server.closeAllConnections();server.close();rmSync(profile,{recursive:true,force:true});}
}
