// Desktop UI through real Axum/PG/service/SDK; owned synthetic model outputs only.
import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
const directory=process.env.WEB_APP_PROOF_DIR;
const {origin,domain}=JSON.parse(await readFile(path.join(directory,'fixture.json'),'utf8'));
assert.equal(new URL(origin).hostname,'127.0.0.1');assert.notEqual(new URL(origin).port,'3000');
const report={scope:'desktop synthetic materials and owned loopback provider; not model-quality evidence',steps:[],errors:[],blocked:[]};
const browser=await chromium.launch({headless:true,...(process.env.DOSSIER_BROWSER_EXECUTABLE?{executablePath:process.env.DOSSIER_BROWSER_EXECUTABLE}:{})});
const context=await browser.newContext({viewport:{width:1440,height:1000},reducedMotion:'reduce'});
await context.route('**/*',route=>{if(new URL(route.request().url()).origin!==origin){report.blocked.push(route.request().url());return route.abort();}return route.continue();});
const page=await context.newPage();page.setDefaultTimeout(20000);page.on('pageerror',e=>report.errors.push(e.message));
async function post(button,url){const received=page.waitForResponse(r=>r.request().method()==='POST'&&new URL(r.url()).pathname===url);await button.click();const r=await received;assert.equal(r.status(),200,await r.text());return r.json();}
async function get(url){const r=await page.request.get(origin+url);assert.equal(r.status(),200);return r.json();}
async function until(url,ready){const deadline=Date.now()+25000;let r;do {r=await get(url);if(ready(r))return r;await page.waitForTimeout(100);}while(Date.now()<deadline);assert.fail('workflow did not settle: '+JSON.stringify(r));}
async function step(name,action){await action();await page.screenshot({path:path.join(directory,`research-${report.steps.length+1}.png`),fullPage:true});report.steps.push({name,result:'PASS'});console.log('Research UI PASS: '+name);}
try {
 await step('select actual catalog comments, inspect dossier and start semantic/problem/review chain',async()=>{
  await page.goto(`${origin}/corpus/comments?domain=${domain}&view=comments`);
  await page.getByRole('button',{name:'选择评论',exact:true}).click();
  const checks=page.getByRole('checkbox',{name:'选择这条评论',exact:true});await checks.first().waitFor();assert.equal(await checks.count(),2);
  await checks.nth(0).check();await checks.nth(1).check();
  await page.getByRole('button',{name:'检查所选评论',exact:true}).click();
  await page.getByRole('button',{name:'开始所选评论研究',exact:true}).click();
  await page.getByText('已提交 2 条研究，结果尚未完成。',{exact:true}).waitFor();
  const data=await until(`/api/local/comment-research/problems?domainRef=${domain}`,r=>r.problems.length===1);assert.equal(data.problems[0].evidence.length,2);const receipts=await until(`/api/local/comment-research/runs?domainRef=${domain}`,r=>r.runs.every(run=>run.state!=='running'));assert.ok(receipts.runs.every(run=>run.state==='succeeded'),JSON.stringify(receipts));
  await page.getByRole('button',{name:'用户问题',exact:true}).click();
  await page.getByRole('heading',{name:'每日提醒需求',exact:true}).waitFor();assert.equal(await page.getByRole('blockquote').count(),2);
 });
 await step('reopen an immutable research receipt and restore the exact raw quote',async()=>{
  await page.getByRole('button',{name:'研究记录',exact:true}).click();
  await page.getByRole('button',{name:'查看评论与研究依据',exact:true}).first().click();
  const detail=page.getByRole('region',{name:'研究这条评论',exact:true});await detail.getByText('合成测试需要提醒',{exact:true}).waitFor();
  await detail.getByText('我的孩子需要每天有人提醒',{exact:true}).waitFor();
 });
 await step('creator work and explicit account sample use their own configured tasks',async()=>{
  await page.goto(`${origin}/corpus/creators?domain=${domain}`);
  await page.locator('[data-author-key]').first().click();
  let account=page.getByRole('region',{name:'账号样本研究',exact:true});
  await account.getByRole('button',{name:'检查账号样本',exact:true}).click();
  const receipt=await post(account.getByRole('button',{name:'开始账号研究',exact:true}),'/api/local/creator-account-research');
  await until(`/api/local/creator-account-research/${receipt.requestRef}`,r=>r.state==='succeeded');
  await account.getByRole('button',{name:'刷新研究状态',exact:true}).click();
  await page.getByRole('tab',{name:'作品与依据',exact:true}).click();
  await page.locator('details').filter({hasText:'合成底稿验收'}).first().locator('summary').click();
  const single=page.getByRole('region',{name:'单篇作品研究',exact:true});
  await single.getByRole('button',{name:'检查研究底稿',exact:true}).click();
  const result=await post(single.getByRole('button',{name:'开始单篇研究',exact:true}),'/api/local/creator-research');
  await until(`/api/local/creator-research/${result.requestRef}`,r=>r.state==='succeeded');
  await single.getByRole('button',{name:'刷新研究状态',exact:true}).click();
  await single.getByText('单篇研究已完成，判断和依据已更新。',{exact:true}).waitFor();
 });
 await step('topic four-stage research publishes only after both independent reviews',async()=>{
  await page.goto(`${origin}/topics?tm=1&domainRef=${domain}`);
  await page.locator('[data-action="research"]').first().click();
  const flow=page.getByRole('region',{name:'主题研究流程',exact:true});
  await flow.getByRole('checkbox',{name:'合成底稿验收',exact:true}).check();
  const receipt=await post(flow.getByRole('button',{name:'开始四阶段研究',exact:true}),'/api/local/topic-research');
  const data=await until(`/api/local/topic-research/runs?domainRef=${domain}`,r=>r.runs.find(run=>run.runRef===receipt.runRef)?.pendingStageCount===0);
  const run=data.runs.find(run=>run.runRef===receipt.runRef);assert.equal(run.failedStageCount,0);assert.ok(run.succeededStageCount>=4);
  await flow.getByRole('button',{name:'刷新阶段回执',exact:true}).click();
  await flow.getByText(/阶段处理完成/).waitFor();
  const graph=await get(`/api/local/topic-map?domainRef=${domain}`);assert.ok(graph.topics.length>0);
 });
 assert.deepEqual(report.errors,[]);
} catch(error){report.failure=String(error.stack??error);report.runs=await get(`/api/local/comment-research/runs?domainRef=${domain}`);report.topic=await get(`/api/local/topic-research/runs?domainRef=${domain}`);await page.screenshot({path:path.join(directory,'research-failed.png'),fullPage:true});process.exitCode=1;console.error(error);}
finally{await writeFile(path.join(directory,'report.json'),JSON.stringify(report,null,2));await browser.close();}
