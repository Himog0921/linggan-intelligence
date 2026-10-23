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
assert.equal(endpoint.hostname, '127.0.0.1'); assert.notEqual(endpoint.port, '3000'); assert.match(endpoint.port, /^\d+$/);
for (const ref of [workRef, domain]) assert.match(ref, /^[0-9a-f-]{36}$/i);
const statePath = `/api/local/work-resources/${workRef}/ocr-recognition`;
const candidatesPath = `/api/local/work-resources/${workRef}/research-dossier/candidates?domain=${domain}`;
const configPath = '/api/local/ai-settings/task-configs';
const report = { result: 'FAIL', scope: 'actual React/API/owned PG; loopback supplier mock; no real provider calls', writes: [], startCommands: [], blockedRequests: [], errors: [], steps: [] };
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
    const allowedWrite = request.method() === 'POST' && [configPath, statePath].includes(url.pathname);
    if (url.origin !== origin || (request.method() !== 'GET' && !allowedWrite)) {
      report.blockedRequests.push(`${request.method()} ${request.url()}`); return route.abort();
    }
    if (allowedWrite) report.writes.push(url.pathname);
    if (allowedWrite && url.pathname === statePath) report.startCommands.push(request.postDataJSON());
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
  const action = page.locator('[data-ocr-action]');
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
  assert.match(middle.images[0].candidateRevisionRef, /^[0-9a-f-]{36}$/i); assert.ok(middle.images[0].blocks.length > 0);
  assert.ok(middle.images.slice(1).every(image => image.candidateRevisionRef === null), 'uncompleted images do not invent candidate readings');
  assert.equal(await action.getAttribute('style'), '--ocr-progress: 33%;');
  assert.equal(await page.getByRole('progressbar', { name: '图片文字识别进度', exact: true }).getAttribute('aria-valuetext'), '33%');
  assert.doesNotMatch(await action.innerText(), /\d+\s*\/\s*\d+/);
  await page.screenshot({ path: path.join(directory, 'recognition-progress-33.png'), fullPage: true });
  report.steps.push('33% is backed by one persisted candidate out of the explicitly selected three images.');
  // This owned fixture gate releases the second loopback response; it is not a product endpoint.
  const release = await page.request.post(`${origin}/__ocr-proof/release`); assert.equal(release.status(), 200);
  await page.getByRole('button', { name: '查看识别结果', exact: true }).waitFor();
  const final = await readJson(statePath);
  assert.equal(final.state, 'succeeded'); assert.equal(final.completedImages, 3); assert.equal(final.totalImages, 3);
  assert.equal(final.runRef, middle.runRef); assert.equal(final.images.length, 3);
  assert.deepEqual(final.images.map(image => image.slotKey), slotKeys);
  for (const image of final.images) {
    assert.match(image.candidateRevisionRef, /^[0-9a-f-]{36}$/i); assert.ok(image.blocks.length > 0); assert.ok(Array.isArray(image.issues));
    assert.equal(typeof image.rawJson, 'string'); assert.ok(image.rawJson.length > 0); assert.ok(image.originalUrl);
    assert.ok(Number.isInteger(image.usage?.measuredTokens) && image.usage.measuredTokens > 0);
    assert.ok(!('regions' in image), 'legacy OCR regions remain inside raw diagnostics only');
  }
  assert.ok(!('coverTitle' in final) && !('coverTitleStatus' in final), 'headings are canonical blocks, without a parallel cover-title field');
  const result = page.getByRole('region', { name: '图片文字识别结果', exact: true }); await result.waitFor();
  const firstPage = result.locator(`[data-ocr-image="${final.images[0].imageRef}"]`);
  for (const text of ['合成封面标题', '封面正文', 'SYNTHETIC BRAND']) await firstPage.getByText(text, { exact: true }).waitFor();
  assert.equal(await result.getByRole('heading', { name: '页码未知的图片', exact: true }).count(), 2);
  await page.getByRole('button', { name: '查看识别结果', exact: true }).click();
  const resultHeading = await result.getByRole('heading', { name: '候选读本', exact: true }).boundingBox();
  const inspectorTab = await page.getByRole('tab', { name: '作品', exact: true }).boundingBox();
  assert.ok(resultHeading && inspectorTab && resultHeading.y >= inspectorTab.y + inspectorTab.height, 'view results lands below the sticky inspector header');
  await page.screenshot({ path: path.join(directory, 'recognition-result.png'), fullPage: true });
  await result.getByRole('button', { name: '逐图对照', exact: true }).click();
  await loadedImage(firstPage.getByRole('img', { name: '第 1 页原图', exact: true }));
  const receipts = result.locator(':scope > details'); await receipts.locator(':scope > summary').click();
  const firstRaw = receipts.locator(':scope > details').first();
  assert.equal(await firstRaw.locator(':scope > summary').innerText(), '第 1 页 · 原始 JSON 与用量');
  await firstRaw.locator(':scope > summary').click();
  const raw = firstRaw.getByLabel('第 1 页原始 JSON', { exact: true });
  assert.equal(await raw.textContent(), final.images[0].rawJson);
  await raw.scrollIntoViewIfNeeded();
  await page.screenshot({ path: path.join(directory, 'recognition-raw-json.png'), fullPage: true });
  await firstPage.getByRole('button', { name: '核对原图', exact: true }).click();
  await page.getByRole('dialog').waitFor();
  await page.getByRole('button', { name: '关闭查看器', exact: true }).click();
  const candidates = (await readJson(candidatesPath)).candidates;
  assert.equal(candidates.length, 3);
  const candidateRefs = final.images.map(image => image.candidateRevisionRef);
  assert.deepEqual(candidates.map(candidate => candidate.candidateRevisionRef).sort(), [...candidateRefs].sort());
  assert.ok(candidates.every(candidate => candidate.currentRevisionRef === null), 'manual OCR does not automatically adopt any candidate');
  report.steps.push('All three canonical candidate readings, literal heading/body/brand, original image comparison and exact raw diagnostics are readable; adoption remains separate.');

  await page.reload(); await page.getByRole('button', { name: '查看识别结果', exact: true }).waitFor();
  const reloaded = await readJson(statePath);
  assert.equal(reloaded.runRef, final.runRef, 'refresh only reads the existing run');
  assert.deepEqual(reloaded.images.map(image => image.candidateRevisionRef), candidateRefs);
  await page.setViewportSize({ width: 980, height: 1100 });
  await page.screenshot({ path: path.join(directory, 'recognition-narrow.png'), fullPage: true });
  assert.deepEqual(report.errors, []); assert.deepEqual(report.blockedRequests, []);
  assert.equal(report.writes.filter(value => value === statePath).length, 1, 'one explicit browser start command');
  assert.equal(report.writes.filter(value => value === configPath).length, 1, 'one explicit model configuration command');
  report.steps.push('Reload preserves the persisted run and candidate identities without another dispatch.');
  Object.assign(report, { result: 'PASS', runRef: final.runRef, candidateRefs, observedPercentage: 33, finalPercentage: 100 });
  console.log('Manual OCR browser: model → explicit selected slots → persisted 33% → canonical candidates/raw/source → refresh PASS');
} catch (error) {
  report.error = String(error?.stack ?? error);
  if (page) { await writeFile(path.join(directory, 'recognition-browser-failure.html'), await page.content()); await page.screenshot({ path: path.join(directory, 'recognition-browser-failure.png'), fullPage: true }); }
  throw error;
} finally {
  await writeFile(path.join(directory, 'recognition-browser-result.json'), JSON.stringify(report, null, 2));
  await browser.close();
}
