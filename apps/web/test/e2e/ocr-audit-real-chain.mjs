// Actual React → Axum → owned PostgreSQL: quality audit records an opinion and never edits source readings.
import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
const directory = process.env.OCR_AUDIT_PROOF_OUTPUT; assert.ok(directory);
const fixture = JSON.parse(await readFile(path.join(directory, 'fixture.json'), 'utf8'));
const { origin, runRef, sampleRef, readingRef, workRef, domain } = fixture;
const host = new URL(origin); assert.equal(host.hostname, '127.0.0.1'); assert.notEqual(host.port, '3000');
for (const value of [runRef, sampleRef, readingRef, workRef, domain]) assert.match(value, /^[0-9a-f-]{36}$/i);
const browser = await chromium.launch({ headless: true, ...(process.env.DOSSIER_BROWSER_EXECUTABLE ? { executablePath: process.env.DOSSIER_BROWSER_EXECUTABLE } : {}) });
const report = { result: 'FAIL', scope: 'owned synthetic Axum/PostgreSQL, one opinion-only command, no provider calls from browser', steps: [], requests: [], blocked: [], errors: [] };
let page;
async function readDossier() {
  return page.evaluate(async url => { const response = await fetch(url, { cache: 'no-store', credentials: 'same-origin' }); if (!response.ok) throw new Error(`Dossier read failed: ${response.status}`); return (await response.json()).dossier; }, `/api/local/work-resources/${workRef}/research-dossier?domain=${domain}`);
}
try {
  const context = await browser.newContext({ viewport: { width: 1440, height: 1000 }, locale: 'zh-CN' });
  await context.route('**/*', route => {
    const request = route.request(); const url = new URL(request.url());
    const review = request.method() === 'POST' && url.pathname === '/api/local/ocr-audit/reviews';
    if (url.origin !== origin || (request.method() !== 'GET' && !review)) { report.blocked.push(`${request.method()} ${request.url()}`); return route.abort(); }
    if (review) report.requests.push(request.postDataJSON());
    return route.continue();
  });
  page = await context.newPage(); page.setDefaultTimeout(25000);
  page.on('pageerror', error => report.errors.push(error.message));
  page.on('console', message => { if (message.type() === 'error') report.errors.push(message.text()); });
  await page.goto(`${origin}/settings/models?view=runs`);
  await page.getByRole('heading', { name: '图片文字质量抽检', exact: true }).waitFor();
  const before = await readDossier(); assert.equal(before.contract, 'research-dossier.v2');
  const run = page.locator(`details[data-audit-run="${runRef}"]`);
  await run.locator(':scope > summary').click();
  await run.getByRole('columnheader', { name: '人工确认指标', exact: true }).first().waitFor();
  await page.screenshot({ path: path.join(directory, 'audit-report.png'), fullPage: true });
  const sample = run.locator(`[data-audit-sample="${sampleRef}"]`);
  const detailRead = page.waitForResponse(response => new URL(response.url()).pathname === `/api/local/ocr-audit/readings/${readingRef}`);
  await sample.getByRole('button', { name: '审阅抽检意见', exact: true }).click();
  const detailResponse = await detailRead; assert.equal(detailResponse.status(), 200, await detailResponse.text());
  const detail = await detailResponse.json(); assert.equal(detail.sampleRef, sampleRef);
  const baselineRevision = Math.max(0, ...detail.reviews.map(review => review.revision));
  assert.equal(baselineRevision, 2, 'two independent backend opinions precede the browser review');
  await sample.getByRole('heading', { name: '模型第二意见', exact: true }).waitFor();
  const image = sample.getByRole('img', { name: '抽检来源图片', exact: true });
  if (detail.originalAssetUrl) { await image.waitFor(); await image.evaluate(node => { if (node.complete) { if (node.naturalWidth === 0) throw new Error('Audit source image failed to load'); return; } return new Promise((resolve, reject) => { node.addEventListener('load', resolve, { once: true }); node.addEventListener('error', () => reject(new Error('Audit source image failed to load')), { once: true }); }); }); }
  assert.equal(await sample.locator('textarea').count(), 0, 'audit has no reading replacement editor');
  await sample.getByLabel('审阅决定', { exact: true }).selectOption('confirmed');
  await sample.getByLabel('审核人', { exact: true }).fill('SYNTHETIC-BROWSER-NOT-MOG');
  await sample.getByLabel('核对依据', { exact: true }).fill('Procedural owned image inspected; record this opinion independently of the current material reading.');
  const saved = page.waitForResponse(response => new URL(response.url()).pathname === '/api/local/ocr-audit/reviews' && response.request().method() === 'POST');
  await sample.getByRole('button', { name: '保存审阅意见', exact: true }).click();
  const response = await saved; assert.equal(response.status(), 200, await response.text());
  const receipt = await response.json(); assert.equal(receipt.readingRef, readingRef); assert.equal(receipt.revision, baselineRevision + 1); assert.equal(receipt.decision, 'confirmed');
  await sample.getByText(`抽检审阅已保存，第 ${baselineRevision + 1} 版。`, { exact: true }).waitFor();
  const after = await readDossier(); assert.deepEqual(after.materials, before.materials); assert.equal(after.materialRevision, before.materialRevision);
  assert.equal(report.requests.length, 1);
  const command = report.requests[0]; assert.equal(command.sampleRef, sampleRef); assert.equal(command.expectedRevision, baselineRevision); assert.equal(command.decision, 'confirmed'); assert.equal(command.materialReviewCommandRef, null);
  for (const field of ['field', 'text', 'editedBlocks', 'candidateRevisionRef']) assert.ok(!(field in command));
  assert.deepEqual(report.blocked, []); assert.deepEqual(report.errors, []);
  await page.screenshot({ path: path.join(directory, 'audit-review.png'), fullPage: true });
  report.steps = ['Report retains distinct human denominators.', 'Original image and model second opinion are reviewed in settings.', `One opinion-only review persists revision ${baselineRevision + 1}.`, 'Canonical materials and material revision are unchanged.']; report.result = 'PASS';
  console.log('OCR audit browser: report → original + model opinion → opinion-only revision; canonical reading unchanged PASS');
} catch (error) {
  report.error = String(error?.stack ?? error);
  if (page) { await writeFile(path.join(directory, 'browser-failure.html'), await page.content()); await page.screenshot({ path: path.join(directory, 'browser-failure.png'), fullPage: true }); }
  throw error;
} finally {
  await writeFile(path.join(directory, 'browser-result.json'), JSON.stringify(report, null, 2));
  await browser.close();
}
