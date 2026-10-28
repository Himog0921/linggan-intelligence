// Built UI -> owned Axum/PG -> dedicated service -> fixed Node/SDK -> owned synthetic HTTP.
import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
const directory=process.env.WEB_APP_PROOF_DIR;
assert.ok(directory);
const fixture=JSON.parse(await readFile(path.join(directory,'fixture.json'),'utf8'));
const {origin}=fixture;
const base=new URL(origin);
assert.equal(base.hostname,'127.0.0.1');assert.equal(base.protocol,'http:');assert.notEqual(base.port,'3000');
const phase=process.argv[2];assert.ok(['trial','refreshed-source','restricted-source'].includes(phase));
const report={phase,scope:'owned synthetic probe and two SDK trial calls only',steps:[],errors:[],blocked:[],screenshots:[]};
const browser=await chromium.launch({headless:true,...(process.env.DOSSIER_BROWSER_EXECUTABLE?{executablePath:process.env.DOSSIER_BROWSER_EXECUTABLE}:{})});
const context=await browser.newContext({viewport:{width:1440,height:1000},reducedMotion:'reduce'});
await context.route('**/*',route=>{
 if(new URL(route.request().url()).origin!==origin){report.blocked.push(route.request().url());return route.abort();}
 return route.continue();
});
const page=await context.newPage();page.setDefaultTimeout(20000);
page.on('pageerror',error=>report.errors.push(error.message));
const root=async()=>{
 const response=await page.request.get(origin+'/api/local/ai-settings');assert.equal(response.status(),200);return response.json();
};
async function goto(section,task='writing_legacy'){await page.goto(origin+'/settings/models?section='+section+'&task='+task);await page.getByRole('heading',{name:'AI 设置',exact:true}).waitFor();}
async function post(button,suffix){
 const received=page.waitForResponse(response=>response.request().method()==='POST'&&new URL(response.url()).pathname.endsWith(suffix));
 await button.click();const response=await received;assert.ok(response.ok(),await response.text());return response.json();
}
async function step(name,action){
 try{await action();const screenshot=`${phase}-${report.steps.length+1}.png`;await page.screenshot({path:path.join(directory,screenshot),fullPage:true});report.screenshots.push(screenshot);report.steps.push({name,result:'PASS'});console.log('AI trial UI PASS: '+name);}
 catch(error){report.steps.push({name,result:'FAIL',error:String(error.stack??error)});throw error;}
}
let candidate,comparison,frozenComparison;
try{
 if(phase==='trial'){
  let version;
  await step('editing a skill in settings preserves v1 and creates immutable v2 without a new call',async()=>{
   await goto('skills');
   await page.getByRole('link',{name:/writing-method · v1/}).click();
   await page.getByLabel('SKILL.md',{exact:true}).fill('---\nname: writing-method\ndescription: Synthetic fixed writing method\n---\ncandidate-v2\n');
   version=await post(page.getByRole('button',{name:'保存新版本',exact:true}),'/skills/import');
   assert.equal(version.versionNo,2);assert.notEqual(version.versionRef,fixture.firstVersionRef);
   const data=await root();assert.equal(data.skills.find(skill=>skill.name==='writing-method').versions.length,2);
   assert.equal(data.execution.dailyUsage.reduce((n,row)=>n+Number(row.calls),0),1);
  });
  await step('saving the v2 candidate keeps the existing v1 binding and does not enqueue',async()=>{
   await goto('automation');
   await page.getByLabel('固定做法版本',{exact:true}).selectOption(version.versionRef);
   candidate=await post(page.getByRole('button',{name:'保存配置',exact:true}),'/task-configs');
   assert.equal(candidate.applied,false);
   const task=(await root()).tasks.find(task=>task.taskKey==='writing_legacy');
   assert.equal(task.activeSkillVersionRef,fixture.firstVersionRef);
   assert.equal(task.configuration.taskConfigRef,fixture.baselineConfigRef);
  });
  await step('the two versions execute against the same frozen writing preview',async()=>{
   await goto('skills');
   await page.getByLabel('候选配置',{exact:true}).selectOption(candidate.taskConfigRef);
   await page.getByLabel('固定旧配置',{exact:true}).selectOption(fixture.baselineConfigRef);
   await page.getByLabel('试跑样本',{exact:true}).selectOption(fixture.previewRef);
   comparison=await post(page.getByRole('button',{name:'提交固定样本试跑',exact:true}),'/trials');
   const deadline=Date.now()+20000;
   let receipt;
   while(Date.now()<deadline){
    const response=await page.request.get(`${origin}/api/local/ai-settings/trials/${comparison.comparisonRef}?position=0`);
    assert.equal(response.status(),200);receipt=await response.json();
    if(receipt.qualifiesForActivation)break;
    await page.waitForTimeout(100);
   }
   assert.equal(receipt.qualifiesForActivation,true);
   assert.equal(receipt.inputRef.previewRef,fixture.previewRef);
   await page.getByRole('button',{name:'刷新比较回执',exact:true}).click();
   await page.getByText('合成基线建议',{exact:true}).waitFor();await page.getByText('合成候选建议',{exact:true}).waitFor();
   assert.equal(receipt.baseline.skillVersionRef,fixture.firstVersionRef);assert.equal(receipt.candidate.skillVersionRef,version.versionRef);
   frozenComparison=receipt;
  });
  await step('activation changes the current binding only, leaving three measured synthetic calls',async()=>{
   await page.getByLabel('启用理由',{exact:true}).fill('合成UI两版比较后启用');
   const activated=await post(page.getByRole('button',{name:'启用候选做法',exact:true}),'/skills/activate');
   assert.equal(activated.versionRef,version.versionRef);assert.equal(activated.approvalSource,'trial');
   const data=await root(),task=data.tasks.find(task=>task.taskKey==='writing_legacy');
   assert.equal(task.activeSkillVersionRef,version.versionRef);assert.equal(task.configuration.taskConfigRef,activated.taskConfigRef);
   assert.equal(data.execution.dailyUsage.reduce((n,row)=>n+Number(row.calls),0),3);
   await writeFile(path.join(directory,'trial-receipt.json'),JSON.stringify({candidateConfigRef:candidate.taskConfigRef,comparisonRef:comparison.comparisonRef,inputRef:frozenComparison.inputRef,baselineOutput:frozenComparison.baseline.output,candidateOutput:frozenComparison.candidate.output}));
  });
  await step('rollback and direct switch use saved versions without running another model',async()=>{
   await page.getByLabel('切换做法版本',{exact:true}).selectOption(fixture.firstVersionRef);
   const rollback=await post(page.getByRole('button',{name:'切换做法版本',exact:true}),'/skills/switch');
   assert.equal(rollback.versionRef,fixture.firstVersionRef);assert.equal(rollback.approvalSource,'manual');
   await goto('skills');
   await page.getByLabel('切换做法版本',{exact:true}).selectOption(version.versionRef);
   const switched=await post(page.getByRole('button',{name:'切换做法版本',exact:true}),'/skills/switch');
   assert.equal(switched.versionRef,version.versionRef);assert.equal(switched.approvalSource,'manual');
   const data=await root(),task=data.tasks.find(item=>item.taskKey==='writing_legacy');
   assert.equal(task.activeSkillVersionRef,version.versionRef);
   assert.equal(data.execution.dailyUsage.reduce((n,row)=>n+Number(row.calls),0),3);
  });
  await step('OCR prompt can be edited, enabled and rolled back from settings',async()=>{
   await goto('skills','ocr_recognition');
   const initial=await page.request.get(origin+'/api/local/ai-settings/prompts/ocr_recognition');assert.equal(initial.status(),200);
   const before=await initial.json();
   await page.getByLabel('系统提示词',{exact:true}).fill('SYNTHETIC OCR SYSTEM V2');
   await page.getByLabel('任务指令',{exact:true}).fill('SYNTHETIC OCR INSTRUCTION V2');
   const saved=await post(page.getByRole('button',{name:'保存提示词新版本',exact:true}),'/prompts/save');
   await goto('skills','ocr_recognition');
   await page.getByLabel('提示词版本',{exact:true}).selectOption(saved.versionRef);
   const enabled=await post(page.getByRole('button',{name:'启用此提示词版本',exact:true}),'/prompts/activate');
   assert.equal(enabled.versionRef,saved.versionRef);
   await goto('skills','ocr_recognition');
   await page.getByLabel('提示词版本',{exact:true}).selectOption(before.activeVersionRef);
   const restored=await post(page.getByRole('button',{name:'启用此提示词版本',exact:true}),'/prompts/activate');
   assert.equal(restored.versionRef,before.activeVersionRef);
   assert.equal((await root()).execution.dailyUsage.reduce((n,row)=>n+Number(row.calls),0),3);
  });
  await step('each research method can edit and roll back prompts and skills without dispatch',async()=>{
   for(const taskKey of ['comment_study','comment_problem_resolution','comment_problem_pair_review','topic_research','topic_research_extract_review','topic_research_resolve','topic_research_resolve_review','creator_research','creator_account_research']){
    await goto('skills',taskKey);
    const initial=await page.request.get(`${origin}/api/local/ai-settings/prompts/${taskKey}`);assert.equal(initial.status(),200);
    const before=await initial.json();
    await page.getByLabel('系统提示词',{exact:true}).fill(`SYNTHETIC ${taskKey} SYSTEM V2`);
    await page.getByLabel('任务指令',{exact:true}).fill(`SYNTHETIC ${taskKey} INSTRUCTION V2`);
    const saved=await post(page.getByRole('button',{name:'保存提示词新版本',exact:true}),'/prompts/save');
    await goto('skills',taskKey);await page.getByLabel('提示词版本',{exact:true}).selectOption(saved.versionRef);
    assert.equal((await post(page.getByRole('button',{name:'启用此提示词版本',exact:true}),'/prompts/activate')).versionRef,saved.versionRef);
    await goto('skills',taskKey);await page.getByLabel('提示词版本',{exact:true}).selectOption(before.activeVersionRef);
    assert.equal((await post(page.getByRole('button',{name:'启用此提示词版本',exact:true}),'/prompts/activate')).versionRef,before.activeVersionRef);
    await goto('skills',taskKey);await page.getByLabel('切换做法版本',{exact:true}).selectOption(fixture.firstVersionRef);
    assert.equal((await post(page.getByRole('button',{name:'切换做法版本',exact:true}),'/skills/switch')).versionRef,fixture.firstVersionRef);
    await goto('skills',taskKey);await page.getByLabel('切换做法版本',{exact:true}).selectOption(version.versionRef);
    assert.equal((await post(page.getByRole('button',{name:'切换做法版本',exact:true}),'/skills/switch')).versionRef,version.versionRef);
    await goto('skills',taskKey);await page.getByLabel('切换做法版本',{exact:true}).selectOption(fixture.firstVersionRef);
    assert.equal((await post(page.getByRole('button',{name:'切换做法版本',exact:true}),'/skills/switch')).versionRef,fixture.firstVersionRef);
    const task=(await root()).tasks.find(item=>item.taskKey===taskKey);
    assert.equal(task.lifecycle,'active');
    assert.equal(task.handlerContract,taskKey==='creator_research'?'creator-work-research.v1':taskKey==='creator_account_research'?'creator-account-research.v1':taskKey==='comment_study'?'comment-research-semantic.v1':taskKey.startsWith('comment_problem_')?'comment-problem-stage.v1':'topic-research-stage.v1');
   }
   assert.equal((await root()).execution.dailyUsage.reduce((n,row)=>n+Number(row.calls),0),3);
  });
 }else{
  const saved=JSON.parse(await readFile(path.join(directory,'trial-receipt.json'),'utf8'));
  const restricted=phase==='restricted-source';
  await step(restricted?'a current source restriction hides both historical trial outputs without another call':'a new observation preserves the frozen input and both historical trial outputs',async()=>{
   await goto('skills');
   await page.getByLabel('候选配置',{exact:true}).selectOption(saved.candidateConfigRef);
   await page.getByLabel('查看试跑比较',{exact:true}).selectOption(saved.comparisonRef);
   const response=await page.request.get(`${origin}/api/local/ai-settings/trials/${saved.comparisonRef}?position=0`);assert.equal(response.status(),200);
   const historical=await response.json();assert.equal(historical.qualifiesForActivation,true);
   assert.deepEqual(historical.inputRef,saved.inputRef);
   if(restricted){
    assert.equal(historical.sourceQualification,'unavailable');
    assert.equal(historical.baseline.output,null);assert.equal(historical.candidate.output,null);
    await page.getByText('当前来源不可读取；历史产出已隐藏',{exact:false}).waitFor();
    await page.getByText('尚无可展示产出，或来源资格已经变化。',{exact:true}).first().waitFor();
    assert.equal(await page.getByText('合成基线建议',{exact:true}).count(),0);assert.equal(await page.getByText('合成候选建议',{exact:true}).count(),0);
   }else{
    assert.equal(historical.sourceQualification,'readable');
    assert.deepEqual(historical.baseline.output,saved.baselineOutput);assert.deepEqual(historical.candidate.output,saved.candidateOutput);
    await page.getByText('合成基线建议',{exact:true}).waitFor();await page.getByText('合成候选建议',{exact:true}).waitFor();
   }
   const data=await root();assert.equal(data.execution.dailyUsage.reduce((n,row)=>n+Number(row.calls),0),3);
  });
 }
 assert.deepEqual(report.errors,[]);assert.deepEqual(report.blocked,[]);
}finally{
 if(report.steps.at(-1)?.result==='FAIL')await page.screenshot({path:path.join(directory,`${phase}-failure.png`),fullPage:true}).catch(()=>{});
 await writeFile(path.join(directory,`${phase}-report.json`),JSON.stringify(report,null,2));await browser.close();
}
