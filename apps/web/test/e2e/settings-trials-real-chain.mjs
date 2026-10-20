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
const phase=process.argv[2];assert.ok(['trial','changed-source'].includes(phase));
const report={phase,scope:'owned synthetic probe and two SDK trial calls only',steps:[],errors:[],blocked:[],screenshots:[]};
const browser=await chromium.launch({headless:true});
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
async function goto(section){await page.goto(origin+'/settings/models?section='+section+'&task=writing_legacy');await page.getByRole('heading',{name:'AI 设置',exact:true}).waitFor();}
async function post(button,suffix){
 const received=page.waitForResponse(response=>response.request().method()==='POST'&&new URL(response.url()).pathname.endsWith(suffix));
 await button.click();const response=await received;assert.ok(response.ok(),await response.text());return response.json();
}
async function step(name,action){
 try{await action();const screenshot=`${phase}-${report.steps.length+1}.png`;await page.screenshot({path:path.join(directory,screenshot),fullPage:true});report.screenshots.push(screenshot);report.steps.push({name,result:'PASS'});console.log('AI trial UI PASS: '+name);}
 catch(error){report.steps.push({name,result:'FAIL',error:String(error.stack??error)});throw error;}
}
let candidate,comparison;
try{
 if(phase==='trial'){
  let version;
  await step('local directory reimport preserves v1 and creates immutable v2 without a new call',async()=>{
   await goto('skills');
   await page.getByLabel('选择做法目录',{exact:true}).setInputFiles(fixture.package);
   version=await post(page.getByRole('button',{name:'导入做法包',exact:true}),'/skills/import');
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
  });
  await step('activation changes the current binding only, leaving three measured synthetic calls',async()=>{
   await page.getByLabel('启用理由',{exact:true}).fill('合成UI两版比较后启用');
   const activated=await post(page.getByRole('button',{name:'启用候选做法',exact:true}),'/skills/activate');
   assert.equal(activated.versionRef,version.versionRef);assert.equal(activated.approvalSource,'trial');
   const data=await root(),task=data.tasks.find(task=>task.taskKey==='writing_legacy');
   assert.equal(task.activeSkillVersionRef,version.versionRef);assert.equal(task.configuration.taskConfigRef,activated.taskConfigRef);
   assert.equal(data.execution.dailyUsage.reduce((n,row)=>n+Number(row.calls),0),3);
   await writeFile(path.join(directory,'trial-receipt.json'),JSON.stringify({candidateConfigRef:candidate.taskConfigRef,comparisonRef:comparison.comparisonRef}));
  });
 }else{
  const saved=JSON.parse(await readFile(path.join(directory,'trial-receipt.json'),'utf8'));
  await step('a source replacement hides both historical trial outputs on a fresh read',async()=>{
   await goto('skills');
   await page.getByLabel('候选配置',{exact:true}).selectOption(saved.candidateConfigRef);
   await page.getByLabel('查看试跑比较',{exact:true}).selectOption(saved.comparisonRef);
   const response=await page.request.get(`${origin}/api/local/ai-settings/trials/${saved.comparisonRef}?position=0`);assert.equal(response.status(),200);
   const historical=await response.json();assert.equal(historical.qualifiesForActivation,true);assert.equal(historical.sourceQualification,'unavailable');
   assert.equal(historical.baseline.output,null);assert.equal(historical.candidate.output,null);
   await page.getByText('当前来源不可读取或已换版；历史产出已隐藏',{exact:false}).waitFor();
   await page.getByText('尚无可展示产出，或来源资格已经变化。',{exact:true}).first().waitFor();
   assert.equal(await page.getByText('合成基线建议',{exact:true}).count(),0);assert.equal(await page.getByText('合成候选建议',{exact:true}).count(),0);
   const data=await root();assert.equal(data.execution.dailyUsage.reduce((n,row)=>n+Number(row.calls),0),3);
  });
 }
 assert.deepEqual(report.errors,[]);assert.deepEqual(report.blocked,[]);
}finally{
 if(report.steps.at(-1)?.result==='FAIL')await page.screenshot({path:path.join(directory,`${phase}-failure.png`),fullPage:true}).catch(()=>{});
 await writeFile(path.join(directory,`${phase}-report.json`),JSON.stringify(report,null,2));await browser.close();
}
