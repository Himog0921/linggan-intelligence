// Desktop UI through real Axum/PG/service/SDK; owned synthetic model outputs only.
import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
const directory=process.env.WEB_APP_PROOF_DIR;
const {origin,domain,work,coreRef}=JSON.parse(await readFile(path.join(directory,'fixture.json'),'utf8'));
assert.equal(new URL(origin).hostname,'127.0.0.1');assert.notEqual(new URL(origin).port,'3000');
const report={scope:'desktop synthetic materials and owned loopback provider; not model-quality evidence',steps:[],errors:[],blocked:[]};
const browser=await chromium.launch({headless:true,...(process.env.DOSSIER_BROWSER_EXECUTABLE?{executablePath:process.env.DOSSIER_BROWSER_EXECUTABLE}:{})});
const context=await browser.newContext({viewport:{width:1440,height:1000},reducedMotion:'reduce'});
await context.route('**/*',route=>{if(new URL(route.request().url()).origin!==origin){report.blocked.push(route.request().url());return route.abort();}return route.continue();});
const page=await context.newPage();page.setDefaultTimeout(20000);page.on('pageerror',e=>report.errors.push(e.message));
async function post(button,url){await button.waitFor({state:'visible'});const [r]=await Promise.all([page.waitForResponse(r=>r.request().method()==='POST'&&new URL(r.url()).pathname===url),button.click()]);assert.equal(r.status(),200,await r.text());return r.json();}
async function get(url){const r=await page.request.get(origin+url);assert.equal(r.status(),200);return r.json();}
async function until(url,ready){const deadline=Date.now()+25000;let r;do {r=await get(url);if(ready(r))return r;await page.waitForTimeout(100);}while(Date.now()<deadline);assert.fail('workflow did not settle: '+JSON.stringify(r));}
async function step(name,action){await action();await page.screenshot({path:path.join(directory,`research-${report.steps.length+1}.png`),fullPage:true});report.steps.push({name,result:'PASS'});console.log('Research UI PASS: '+name);}
try {
 await step('restored borderless overview retains source counts, curve and period controls after an empty research reset',async()=>{
  await page.goto(`${origin}/corpus/comments?domain=${domain}&view=overview`);
  const hero=page.getByRole('region',{name:'评论研究概览',exact:true});
  await hero.getByText('可研究评论',{exact:true}).waitFor();
  const reading=await get(`/api/local/comment-research/overview?domainRef=${domain}`);
  assert.equal(reading.corpusSummary.eligibleCommentCount,2);assert.equal(reading.researchSummary.currentSignalCount,0);
  const metrics=page.locator('[aria-label="研究累计读数与昨日活动"]');
  const chart=page.getByRole('region',{name:'评论观察趋势',exact:true});
  const a=await metrics.boundingBox(),b=await chart.boundingBox();assert.ok(a&&b&&a.x+a.width<=b.x+1,'approved left metrics/right observation layout');
  await page.getByRole('button',{name:'近 7 天',exact:true}).click();await page.getByRole('button',{name:'表格',exact:true}).click();
  assert.equal(await hero.getByRole('row').count(),8);
  await page.getByRole('link',{name:'用户评论',exact:true}).click();await page.getByRole('searchbox',{name:'字面搜索',exact:true}).waitFor();
  await page.getByRole('link',{name:'概览',exact:true}).click();
  assert.equal(await page.getByRole('button',{name:'表格',exact:true}).getAttribute('aria-pressed'),'true');
  await page.getByRole('button',{name:'近 56 天',exact:true}).click();await page.getByRole('button',{name:'图表',exact:true}).click();
  await hero.getByRole('img',{name:'评论柱状与研究信号渐变曲线，共同横轴及采集覆盖',exact:true}).waitFor();
 });
 await step('select actual catalog comments, inspect dossier and start semantic/problem/review chain',async()=>{
  await page.goto(`${origin}/corpus/comments?domain=${domain}&view=comments`);
  await page.getByRole('button',{name:'选择评论',exact:true}).click();
  const checks=page.getByRole('checkbox',{name:'选择这条评论',exact:true});await checks.first().waitFor();assert.equal(await checks.count(),2);
  await checks.nth(0).check();await checks.nth(1).check();
  await page.getByRole('button',{name:'检查所选评论',exact:true}).click();
  await page.getByRole('button',{name:'开始所选评论研究',exact:true}).click();
  await page.getByText('已提交 2 条研究，结果尚未完成。',{exact:true}).waitFor();
  const data=await until(`/api/local/comment-research/problems?domainRef=${domain}`,r=>r.problems.length===1);assert.equal(data.problems[0].evidence.length,2);const receipts=await until(`/api/local/comment-research/runs?domainRef=${domain}`,r=>r.runs.every(run=>run.state!=='running'));assert.ok(receipts.runs.every(run=>run.state==='succeeded'),JSON.stringify(receipts));
  await page.getByRole('link',{name:'用户问题',exact:true}).click();
  await page.getByRole('heading',{name:'每日提醒需求',exact:true}).waitFor();assert.equal(await page.getByRole('blockquote').count(),2);
 });
 await step('reopen an immutable research receipt and restore the exact raw quote',async()=>{
  await page.getByRole('link',{name:'研究记录',exact:true}).click();
  await page.getByRole('button',{name:'查看评论与研究依据',exact:true}).first().click();
  const detail=page.getByRole('region',{name:'研究这条评论',exact:true});await detail.getByText('合成测试需要提醒',{exact:true}).waitFor();
  await detail.getByText('我的孩子需要每天有人提醒',{exact:true}).waitFor();
 });
 await step('current overview counts qualified semantic outputs and independently reviewed problems',async()=>{
  await page.goto(`${origin}/corpus/comments?domain=${domain}&view=overview`);
  await page.getByRole('region',{name:'评论研究概览',exact:true}).waitFor();
  const reading=await get(`/api/local/comment-research/overview?domainRef=${domain}`);
  assert.equal(reading.researchSummary.succeededCommentCount,2);assert.equal(reading.researchSummary.currentSignalCount,2);assert.equal(reading.knowledgeSummary.problemCount,1);
  await page.getByRole('heading',{name:'每日提醒需求',exact:true}).waitFor();
 });
 await step('creator work and explicit account sample use their own configured tasks',async()=>{
  await page.goto(`${origin}/corpus/creators?domain=${domain}`);
  await page.locator('[data-author-key]').first().click();
  const account=page.getByRole('complementary').getByRole('tabpanel',{name:'概览',exact:true});
  await account.getByRole('button',{name:'检查账号样本',exact:true}).click();
  const receipt=await post(account.getByRole('button',{name:'开始账号研究',exact:true}),'/api/local/creator-account-research');
  await until(`/api/local/creator-account-research/${receipt.requestRef}`,r=>r.state==='succeeded');
  await account.getByRole('button',{name:'刷新研究状态',exact:true}).click();
  await page.getByRole('tab',{name:'研究线索',exact:true}).click();
  const single=page.getByRole('tabpanel',{name:'研究线索',exact:true}).locator('details').filter({hasText:'合成底稿验收'}).first();
  await single.locator('summary').click();
  await single.getByRole('button',{name:'检查研究底稿',exact:true}).click();
  const result=await post(single.getByRole('button',{name:'开始单篇研究',exact:true}),'/api/local/creator-research');
  await until(`/api/local/creator-research/${result.requestRef}`,r=>r.state==='succeeded');
  await single.getByRole('button',{name:'刷新研究状态',exact:true}).click();
  await single.getByText('单篇研究已完成，判断和依据已更新。',{exact:true}).waitFor();
 });
 await step('topic single-pass research publishes the accepted assignment',async()=>{
  await page.goto(`${origin}/topics?tm=1&domainRef=${domain}`);
  await page.locator('[data-action="research"]').first().click();
  const flow=page.getByRole('region',{name:'主题研究流程',exact:true});
  await flow.getByRole('checkbox',{name:'合成底稿验收',exact:true}).check();
  const receipt=await post(flow.getByRole('button',{name:'开始研究',exact:true}),'/api/local/topic-research');
  const data=await until(`/api/local/topic-research/runs?domainRef=${domain}`,r=>r.runs.find(run=>run.runRef===receipt.runRef)?.pendingStageCount===0);
  const run=data.runs.find(run=>run.runRef===receipt.runRef);assert.equal(run.failedStageCount,0);assert.equal(run.succeededStageCount,run.windowCount);
  await flow.getByRole('button',{name:'刷新进度',exact:true}).click();
  await flow.getByText(/研究完成/).waitFor();
  const topicRuns=await get(`/api/local/topic-research/runs?domainRef=${domain}`);
  assert.ok(topicRuns.runs.every(run=>run.executionMode==='single_pass'&&run.stages.length===run.windowCount&&run.stages.every(stage=>stage.stageNumber===0)));
  const graph=await get(`/api/local/topic-map?domainRef=${domain}`);assert.ok(graph.topics.length>0);
  assert.ok(graph.works.find(item=>item.workRef===work)?.topicRefs.length>0);
  assert.equal(graph.scope.unclassifiedWorkCount,0);
  assert.deepEqual(graph.scope.progress.graph,{assigned:1,unassigned:0});
  assert.equal(graph.scope.progress.material.ready,1);
  assert.equal(graph.scope.progress.understanding.available,1);
  assert.equal(graph.scope.operations.publishedWorkCount,1);
  assert.equal(graph.scope.operations.hitWorkCount,1);
  assert.equal(graph.scope.operations.works[0].publications[0].views,6000);
  assert.equal(graph.works.find(item=>item.workRef===work).ownBreakout,false);
  assert.equal(graph.topics[0].statistics.operations.hitWorkCount,1);
  const patterns=graph.works.find(item=>item.workRef===work)?.research?.contentPatterns;
  assert.equal(patterns?.coreRef,coreRef);
  assert.equal(patterns.items.find(item=>item.kind==='content_type')?.label,'practical_guide');
  assert.deepEqual(patterns.items[0].unitIds,['u1']);assert.ok(patterns.items[0].evidence.length>0);
  await page.keyboard.press('Escape');
  await page.getByRole('region',{name:'当前范围处理进度',exact:true}).getByText(/待归主题\s*0/, {exact:true}).waitFor();
  const progress=page.getByRole('region',{name:'当前范围处理进度',exact:true});
  await progress.getByText(/可用\s*1/,{exact:true}).waitFor();
  const operations=page.getByRole('region',{name:'运营发布覆盖',exact:true});
  await operations.getByText(/运营发布\s*1\s*篇/,{exact:true}).waitFor();
  await operations.getByText(/运营爆款\s*1\s*篇/,{exact:true}).waitFor();
  await page.getByText('运营爆款',{exact:true}).waitFor();
  await page.locator('[data-action="tab"][data-id="patterns"]').waitFor();
  await progress.scrollIntoViewIfNeeded();
  await page.screenshot({path:path.join(directory,'topic-scope-progress.png'),fullPage:true});
  await page.locator('[data-action="tab"][data-id="patterns"]').click();
  const types=page.getByRole('region',{name:'内容类型',exact:true});
  await types.getByRole('heading',{name:'方法指南',exact:true}).waitFor();
  await types.locator('summary').filter({hasText:'查看依据'}).click();
  await types.getByRole('blockquote').filter({hasText:'结尾不能丢失🙂'}).waitFor();
  assert.equal(await types.locator('[data-core-ref]').getAttribute('data-core-ref'),coreRef);
  await page.screenshot({path:path.join(directory,'topic-core-patterns.png'),fullPage:true});
  await types.getByRole('button',{name:'核对原文',exact:true}).click();
  await page.getByRole('dialog').getByText(/结尾不能丢失/).first().waitFor();
 });
 await step('comment semantics attach to existing topics and reopen the exact original comment',async()=>{
  const endpoint=`/api/local/topic-map/comment-assignments?${new URLSearchParams({domainRef:domain,workRef:work})}`;
  const before=await get(endpoint);assert.equal(before.items.length,2);
  assert.ok(before.items.every(item=>item.assignmentState==='not_analyzed'));
  const unassigned=page.getByRole('region',{name:'评论主题挂靠',exact:true});
  await unassigned.getByText('尚未分析',{exact:true}).first().waitFor();
  await page.keyboard.press('Escape');
  await page.locator('[data-action="research"]').first().click();
  const toggle=page.getByRole('switch');await toggle.waitFor();
  assert.equal(await toggle.getAttribute('aria-checked'),'false');
  await post(toggle,'/api/local/topic-research/automation');
  const assigned=await until(endpoint,r=>r.items.length===2&&r.items.every(item=>item.assignmentState==='complete'));
  assert.ok(assigned.items.every(item=>item.sourceAvailable&&item.signals.length===1&&item.signals[0].assignment.status==='matched'));
  await post(toggle,'/api/local/topic-research/automation');
  await page.keyboard.press('Escape');
  await page.goto(`${origin}/topics?tm=1&domainRef=${domain}`);
  await page.locator(`[data-action="reader"][data-id="${work}"]`).first().click();
  const matches=page.getByRole('region',{name:'评论主题挂靠',exact:true});
  await matches.getByText('已匹配',{exact:true}).first().waitFor();
  const target=assigned.items[0];
  const original=matches.locator(`a[href*="researchRequestRef=${target.requestRef}"]`);
  const destination=new URL(await original.getAttribute('href'),origin);
  assert.equal(destination.searchParams.get('commentExternalId'),target.commentExternalId);
  assert.equal(destination.searchParams.get('commentWorkRef'),work);
  await matches.scrollIntoViewIfNeeded();
  await page.screenshot({path:path.join(directory,'topic-comment-assignments.png'),fullPage:true});
  await original.click();
  await page.waitForURL(url=>url.pathname==='/corpus/comments'&&url.searchParams.get('researchRequestRef')===target.requestRef);
  await page.getByRole('dialog').getByText('我的孩子需要每天有人提醒。',{exact:true}).first().waitFor();
  await page.getByRole('region',{name:'评论主题挂靠',exact:true}).getByText('已匹配',{exact:true}).waitFor();
 });
 assert.deepEqual(report.errors,[]);
} catch(error){report.failure=String(error.stack??error);report.runs=await get(`/api/local/comment-research/runs?domainRef=${domain}`);report.topic=await get(`/api/local/topic-research/runs?domainRef=${domain}`);await page.screenshot({path:path.join(directory,'research-failed.png'),fullPage:true});process.exitCode=1;console.error(error);}
finally{await writeFile(path.join(directory,'report.json'),JSON.stringify(report,null,2));await browser.close();}
