// Built React → actual Axum → owned PG. Procedural synthetic image only; no external network.
import assert from 'node:assert/strict';import {chromium} from 'playwright';import {readFile,writeFile} from 'node:fs/promises';import path from 'node:path';
const directory=process.env.OCR_AUDIT_PROOF_OUTPUT;assert.ok(directory);
const fixture=JSON.parse(await readFile(path.join(directory,'fixture.json'),'utf8'));const {origin}=fixture;
const url=new URL(origin);assert.equal(url.hostname,'127.0.0.1');assert.notEqual(url.port,'3000');
const browser=await chromium.launch({headless:true});const context=await browser.newContext({viewport:{width:1440,height:1000}});const errors=[];const requests=[];
await context.route('**/*',route=>{if(new URL(route.request().url()).origin!==origin)return route.abort();return route.continue();});
const page=await context.newPage();page.setDefaultTimeout(20000);page.on('pageerror',e=>errors.push(e.message));page.on('request',r=>{if(r.method()==='POST')requests.push(new URL(r.url()).pathname);});
try{
 await page.goto(origin+'/settings/models?view=runs');await page.getByRole('heading',{name:'图片文字质量抽检',exact:true}).waitFor();const originalReport=page.locator(`details[data-audit-run="${fixture.runRef}"]`);await originalReport.locator(':scope > summary').click();await originalReport.getByRole('columnheader',{name:'人工确认指标',exact:true}).first().waitFor();await page.screenshot({path:path.join(directory,'audit-report.png'),fullPage:true});
 await page.goto(`${origin}/corpus/evidence?domain=${fixture.domain}&work=${fixture.workRef}&tab=dossier&panel=focus`);
 const trigger=page.getByRole('button',{name:'查看质量抽检',exact:true,includeHidden:true});await trigger.waitFor({state:'attached'});
 // Reveal the existing dossier/image disclosures, without setting application state or mocking APIs.
 const summary=page.locator('details').filter({has:trigger}).locator(':scope > summary').first();
 const outer=page.locator('details').filter({has:trigger});
 for(let n=0;n<await outer.count();n++){const d=outer.nth(n);if(await d.getAttribute('open')===null&&await d.locator(':scope > summary').count())await d.locator(':scope > summary').click();}
 if(!await trigger.isVisible())await summary.click();await trigger.click();await page.getByRole('heading',{name:/模型第二意见/}).waitFor();
 const form=page.getByRole('group',{name:'对照原图后的审阅',exact:true});
 await form.getByLabel('审阅决定',{exact:true}).selectOption('adopt');await form.getByLabel('修正后本图完整采用文字',{exact:true}).fill('42');await form.getByLabel('审核人',{exact:true}).fill('SYNTHETIC-BROWSER-NOT-MOG');await form.getByLabel('对照依据与原因',{exact:true}).fill('Synthetic procedural image independently inspected in the owned proof.');
 const receipt=page.waitForResponse(r=>new URL(r.url()).pathname==='/api/local/ocr-audit/reviews'&&r.request().method()==='POST');await page.getByRole('button',{name:'保存审阅决定',exact:true}).click();const response=await receipt;assert.equal(response.status(),200,await response.text());
 await page.getByText(/审阅已保存，第 3 版/).waitFor();await page.screenshot({path:path.join(directory,'audit-review.png'),fullPage:true});
 assert.deepEqual(errors,[]);assert.deepEqual(requests,['/api/local/ocr-audit/reviews']);
 await writeFile(path.join(directory,'browser-result.json'),JSON.stringify({result:'PASS',scope:'synthetic owned Axum/PG, one human command, zero provider calls from browser',steps:['reports keep human denominators distinct','image opinion is available next to original','human correction persists as revision 3 and dossier refreshes'],requests,errors},null,2));
 console.log('OCR audit browser: settings/report → original/local/opinion → one persisted human revision PASS');
}catch(error){await writeFile(path.join(directory,'browser-failure.html'),await page.content());await page.screenshot({path:path.join(directory,'browser-failure.png'),fullPage:true});throw error;}finally{await browser.close();}
