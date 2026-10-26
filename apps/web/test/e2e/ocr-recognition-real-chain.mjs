// Actual React → Axum → owned PostgreSQL. Only the model supplier is a loopback mock.
import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';

const directory = process.env.OCR_AUDIT_PROOF_OUTPUT;
assert.ok(directory, 'OCR_AUDIT_PROOF_OUTPUT is required');
const fixture = JSON.parse(await readFile(path.join(directory, 'recognition-fixture.json'), 'utf8'));
const { origin, workRef, domain } = fixture;
const endpoint = new URL(origin);
assert.equal(endpoint.hostname, '127.0.0.1'); assert.notEqual(endpoint.port, '3000'); assert.notEqual(endpoint.port, '3001'); assert.match(endpoint.port, /^\d+$/);
for (const ref of [workRef, domain]) assert.match(ref, /^[0-9a-f-]{36}$/i);
const statePath = `/api/local/work-resources/${workRef}/ocr-recognition`;
const dossierPath = `/api/local/work-resources/${workRef}/research-dossier`;
const configPath = '/api/local/ai-settings/task-configs';
const reviewPaths = new Set();
const report = { result: 'FAIL', scope: 'actual React/API/owned PG; loopback supplier mock; no real provider calls', writes: [], startCommands: [], reviewCommands: [], blockedRequests: [], errors: [], steps: [] };
const browser = await chromium.launch({ headless: true, ...(process.env.DOSSIER_BROWSER_EXECUTABLE ? { executablePath: process.env.DOSSIER_BROWSER_EXECUTABLE } : {}) });
let page;
async function readJson(requestPath) {
  const response = await page.request.get(origin + requestPath);
  assert.equal(response.status(), 200, await response.text());
  return response.json();
}
async function loadedImage(image) {
  await image.waitFor();
  await image.evaluate(node => {
    if (node.complete) { if (node.naturalWidth === 0) throw new Error('Owned synthetic source image failed to load'); return; }
    return new Promise((resolve, reject) => {
      node.addEventListener('load', resolve, { once: true });
      node.addEventListener('error', () => reject(new Error('Owned synthetic source image failed to load')), { once: true });
    });
  });
}
try {
  const context = await browser.newContext({ viewport: { width: 1440, height: 1100 }, locale: 'zh-CN' });
  await context.route('**/*', route => {
    const request = route.request(); const url = new URL(request.url());
    const allowedReview = request.method() === 'POST' && reviewPaths.has(url.pathname) && url.searchParams.get('domain') === domain;
    const allowedWrite = request.method() === 'POST' && ([configPath, statePath].includes(url.pathname) || allowedReview);
    if (url.origin !== origin || (request.method() !== 'GET' && !allowedWrite)) {
      report.blockedRequests.push(`${request.method()} ${request.url()}`); return route.abort();
    }
    if (allowedWrite) report.writes.push(url.pathname);
    if (allowedWrite && url.pathname === statePath) report.startCommands.push(request.postDataJSON());
    if (allowedReview) report.reviewCommands.push({ path: url.pathname, command: request.postDataJSON() });
    return route.continue();
  });
  page = await context.newPage(); page.setDefaultTimeout(30000);
  page.on('pageerror', error => report.errors.push(error.message));
  page.on('console', message => { if (message.type() === 'error') report.errors.push(message.text()); });

  await page.goto(`${origin}/settings/models?view=advanced&area=work&task=ocr_recognition`);
  await page.getByRole('heading', { name: 'OCR 模型', exact: true }).waitFor();
  await page.getByRole('combobox', { name: '选择 OCR 模型', exact: true }).waitFor();
  const before = await readJson('/api/local/ai-settings');
  const controls = before.controls.map(row => ({ scope: row.scope, enabled: row.enabled }));
  await page.screenshot({ path: path.join(directory, 'recognition-settings.png'), fullPage: true });
  const saved = page.waitForResponse(response => new URL(response.url()).pathname === configPath && response.request().method() === 'POST');
  await page.getByRole('button', { name: '保存配置', exact: true }).click();
  const savedResponse = await saved; assert.equal(savedResponse.status(), 200, await savedResponse.text());
  const after = await readJson('/api/local/ai-settings');
  assert.deepEqual(after.controls.map(row => ({ scope: row.scope, enabled: row.enabled })), controls);
  const ready = await readJson(statePath);
  assert.equal(ready.runRef, null, 'saving settings sends no image and creates no run');
  assert.equal(ready.state, 'ready'); assert.equal(ready.scope.imageCount, 3);
  const slotKeys = ready.scope.images.map(image => image.slotKey);
  assert.deepEqual(slotKeys, [1, 2, 3].map(number => `xhs:ocr-recognition-browser-synthetic:image:${number}`));
  assert.deepEqual(ready.scope.images.map(image => ({ ordinal: image.ordinal, role: image.role })), [{ ordinal: 1, role: 'cover' }, { ordinal: null, role: 'unknown' }, { ordinal: null, role: 'unknown' }]);
  report.steps.push('OCR model configuration saved without creating a run or changing runtime switches.');

  await page.goto(`${origin}/corpus/evidence?domain=${domain}&work=${workRef}&tab=overview&panel=focus`);
  const action = page.locator('[data-ocr-action]:visible');
  await page.getByRole('button', { name: 'OCR 识别', exact: true }).waitFor();
  await page.screenshot({ path: path.join(directory, 'recognition-ready.png'), fullPage: true });
  await action.click();
  const selection = page.getByRole('group', { name: '确认本篇图片识别范围', exact: true });
  await selection.waitFor();
  const checkboxes = selection.getByRole('checkbox'); assert.equal(await checkboxes.count(), 3);
  assert.equal(await selection.getByRole('checkbox', { name: '第 1 页', exact: true }).count(), 1);
  assert.equal(await selection.getByRole('checkbox', { name: '页码未知的图片', exact: true }).count(), 2, 'unknown page numbers remain unknown');
  for (const checkbox of await checkboxes.all()) assert.equal(await checkbox.isChecked(), true);
  await checkboxes.nth(2).uncheck();
  await selection.getByText('重新识读 2 张原图', { exact: true }).waitFor();
  await checkboxes.nth(2).check();
  await selection.getByText('重新识读 3 张原图', { exact: true }).waitFor();
  assert.equal((await readJson(statePath)).runRef, null, 'scope selection is read-only');
  const started = page.waitForResponse(response => new URL(response.url()).pathname === statePath && response.request().method() === 'POST');
  await selection.getByRole('button', { name: '确认并开始识别', exact: true }).click();
  const startedResponse = await started; assert.equal(startedResponse.status(), 200, await startedResponse.text());
  assert.equal(report.startCommands.length, 1);
  const command = report.startCommands[0];
  assert.match(command.commandRef, /^[0-9a-f-]{36}$/i);
  assert.equal(command.expectedScopeSha256, ready.scope.sha256); assert.deepEqual(command.slotKeys, slotKeys);
  assert.deepEqual(Object.keys(command).sort(), ['commandRef', 'expectedScopeSha256', 'slotKeys']);
  report.steps.push('One explicit command freezes the displayed inventory hash and the three selected source slots; unknown page numbers are preserved.');

  await page.getByRole('button', { name: '识别中 33%', exact: true }).waitFor();
  const middle = await readJson(statePath);
  assert.equal(middle.state, 'running'); assert.equal(middle.completedImages, 1); assert.equal(middle.totalImages, 3);
  assert.match(middle.images[0].readingRef, /^[0-9a-f-]{36}$/i); assert.ok(middle.images[0].blocks.length > 0);
  assert.ok(middle.images.slice(1).every(image => image.readingRef === null), 'uncompleted images do not invent readings');
  assert.equal(await action.getAttribute('style'), '--ocr-progress: 33%;');
  assert.equal(await page.getByRole('progressbar', { name: '图片文字识别进度', exact: true }).getAttribute('aria-valuetext'), '33%');
  assert.doesNotMatch(await action.innerText(), /\d+\s*\/\s*\d+/);
  await page.screenshot({ path: path.join(directory, 'recognition-progress-33.png'), fullPage: true });
  report.steps.push('33% is backed by one persisted current reading out of the explicitly selected three images.');
  // This owned fixture gate releases the second loopback response; it is not a product endpoint.
  const release = await page.request.post(`${origin}/__ocr-proof/release`); assert.equal(release.status(), 200);
  await page.getByRole('button', { name: '识别完成 100%', exact: true }).waitFor();
  const final = await readJson(statePath);
  assert.equal(final.state, 'succeeded'); assert.equal(final.completedImages, 3); assert.equal(final.totalImages, 3);
  assert.equal(final.runRef, middle.runRef); assert.equal(final.images.length, 3);
  assert.deepEqual(final.images.map(image => image.slotKey), slotKeys);
  for (const image of final.images) {
    assert.match(image.readingRef, /^[0-9a-f-]{36}$/i); assert.equal(image.isCurrent,true);
    assert.ok(image.blocks.length>0 && image.originalUrl);
    assert.equal(typeof image.rawJson,'string');assert.ok(image.usage.measuredTokens>0);
    assert.ok(!('candidateRevisionRef' in image));
  }
  const canonical=await readJson(`${dossierPath}?domain=${domain}`);
  const imageUnits=canonical.dossier.materials.filter(unit=>unit.kind==='image');
  assert.equal(imageUnits.length,3);
  assert.deepEqual(imageUnits.map(unit=>unit.currentRevisionRef).sort(),final.images.map(image=>image.readingRef).sort());
  const workReading=page.getByRole('region',{name:'作品文字',exact:true});
  for(const text of ['合成封面标题','封面正文','SYNTHETIC BRAND']) await workReading.getByText(text,{exact:true}).first().waitFor();
  assert.equal(await workReading.getByText('SYNTHETIC BRAND', { exact: true }).count(), 3, 'a real repeated source line remains in all three images');
  assert.equal(await page.getByRole('region',{name:'图片文字识别结果',exact:true}).count(),0,'no second OCR full-text panel');
  await page.screenshot({path:path.join(directory,'recognition-current-1440.png'),fullPage:true});
  report.steps.push('Completed image outputs are actual canonical current readings; the work view refreshes the same reading without a duplicate candidate panel.');

  await page.getByRole('tab',{name:'研究底稿',exact:true}).click();
  const dossier=page.getByRole('tabpanel',{name:'研究底稿',exact:true});
  await dossier.getByRole('heading',{name:'研究底稿',exact:true}).waitFor();
  const unit=imageUnits.find(unit=>unit.currentRevisionRef===final.images[0].readingRef);assert.ok(unit);
  await dossier.locator(`[data-unit-ref="${unit.ref}"]`).getByRole('button',{name:'查看来源',exact:true}).first().click();
  const original=dossier.locator(`[data-source-unit-ref="${unit.ref}"]`);
  await loadedImage(original.getByRole('img'));
  await original.getByRole('button',{name:'修正文字',exact:true}).click();
  const reviewPath=`${dossierPath}/materials/${unit.ref}/review`;reviewPaths.add(reviewPath);
  const manualText='合成浏览器人工修正文：完整保留数字 42、否定与来源。';
  await original.getByLabel('段落文字',{exact:true}).first().fill(manualText);
  await original.getByLabel('修正人',{exact:true}).fill('合成浏览器修正人');
  await original.getByLabel('修正说明',{exact:true}).fill('对照受控原图保存完整修正。');
  await original.getByRole('checkbox',{name:'已对照原件核验文字',exact:true}).check();
  const savedHuman=page.waitForResponse(response=>new URL(response.url()).pathname===reviewPath && response.request().method()==='POST');
  await original.getByRole('button',{name:'保存修正',exact:true}).click();
  const response=await savedHuman;assert.equal(response.status(),200,await response.text());const receipt=await response.json();
  assert.equal(receipt.adoption,'human_edited');assert.equal(receipt.verification,'verified');
  const human=report.reviewCommands[0].command;
  assert.equal(human.action,'edit');assert.equal(human.unitRef,unit.ref);assert.equal(human.sourceVersion,unit.sourceVersion);
  assert.equal(human.expectedCurrentRevisionRef,unit.currentRevisionRef);assert.ok(!('candidateRevisionRef' in human));
  const afterHuman=await readJson(`${dossierPath}?domain=${domain}`);
  const current=afterHuman.dossier.materials.find(item=>item.ref===unit.ref);
  assert.equal(current.currentRevisionRef,receipt.currentRevisionRef);assert.ok(current.blocks.some(block=>block.text===manualText));
  assert.equal(current.reading.review.actor,human.actor);assert.equal(current.reading.review.reason,human.reason);
  await dossier.locator(`[data-unit-ref="${unit.ref}"]`).getByText(manualText,{exact:true}).waitFor();
  await dossier.getByText(/2 张图片已进入当前底稿/).waitFor();
  const sameRun=await readJson(statePath);assert.equal(sameRun.runRef,final.runRef);
  assert.equal(sameRun.images[0].isCurrent,false);assert.equal(sameRun.images[0].rawJson,final.images[0].rawJson);
  assert.deepEqual(sameRun.images.map(image=>image.readingRef),final.images.map(image=>image.readingRef));
  await page.screenshot({path:path.join(directory,'recognition-human-edit-1440.png'),fullPage:true});
  report.steps.push('Source navigation opens an actual decoded image; one edit command updates canonical content and the recognition status refreshes its actual current pointer.');
  await page.reload();await page.getByRole('tabpanel',{name:'研究底稿',exact:true}).locator(`[data-unit-ref="${unit.ref}"]`).getByText(manualText,{exact:true}).waitFor();
  const reloaded=await readJson(statePath);assert.equal(reloaded.runRef,final.runRef);
  await page.setViewportSize({width:1280,height:1000});
  await page.screenshot({path:path.join(directory,'recognition-human-edit-1280.png'),fullPage:true});
  assert.deepEqual(report.errors,[]);assert.deepEqual(report.blockedRequests,[]);
  assert.equal(report.writes.filter(value=>value===statePath).length,1);assert.equal(report.writes.filter(value=>value===configPath).length,1);
  assert.deepEqual(report.reviewCommands.map(value=>value.command.action),['edit']);
  Object.assign(report,{result:'PASS',runRef:final.runRef,readingRefs:final.images.map(image=>image.readingRef),observedPercentage:33,finalPercentage:100});
  console.log('Current OCR browser: explicit source scope → persisted progress → canonical reading → source image → edit-only → refresh PASS');
} catch (error) {
  report.error = String(error?.stack ?? error);
  if (page) { await writeFile(path.join(directory, 'recognition-browser-failure.html'), await page.content()); await page.screenshot({ path: path.join(directory, 'recognition-browser-failure.png'), fullPage: true }); }
  throw error;
} finally {
  await writeFile(path.join(directory, 'recognition-browser-result.json'), JSON.stringify(report, null, 2));
  await browser.close();
}
