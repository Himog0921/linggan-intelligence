// Production React → actual Axum → owned synthetic PostgreSQL. No response mocks or model calls.
import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { chromium } from 'playwright';

const directory = process.env.WEB_APP_PROOF_DIR;
assert.ok(directory, 'WEB_APP_PROOF_DIR is required');
const fixture = JSON.parse(await readFile(path.join(directory, 'fixture.json'), 'utf8'));
const { origin, domain, work, imageUnitRef, candidateRefs, longTail } = fixture;
const host = new URL(origin);
assert.equal(host.hostname, '127.0.0.1'); assert.notEqual(host.port, '3000'); assert.match(host.port, /^\d+$/);
assert.ok(Array.isArray(candidateRefs) && candidateRefs.length === 2);
for (const ref of [domain, work, imageUnitRef, ...candidateRefs]) assert.match(ref, /^[0-9a-f-]{36}$/i);
assert.equal(typeof longTail, 'string');
const base = `/api/local/work-resources/${work}/research-dossier`;
const reviewPath = `${base}/materials/${imageUnitRef}/review`;
const commands = [];
const report = { finished: false, contract: 'research-dossier.v2', steps: [], widths: [], pageErrors: [], consoleErrors: [], blockedRequests: [], dossierReads: 0, reviews: [] };
const browser = await chromium.launch({ headless: true, ...(process.env.DOSSIER_BROWSER_EXECUTABLE ? { executablePath: process.env.DOSSIER_BROWSER_EXECUTABLE } : {}) });
let page;
function dossierResponse(response) {
  const url = new URL(response.url());
  return url.pathname === base && url.searchParams.get('format') !== 'markdown' && response.request().method() === 'GET';
}
function material(dossier, ref) { const unit = dossier.materials.find(unit => unit.ref === ref); assert.ok(unit, `material ${ref} exists`); return unit; }
function issueList(unit) { return [...new Map([...unit.issues, ...unit.blocks.flatMap(block => block.issues ?? [])].map(issue => [issue.key, issue])).values()]; }
async function snapshot(name) { await page.screenshot({ path: path.join(directory, name) }); }
async function checkWidth(stage) {
  const size = await page.evaluate(() => ({ actual: document.documentElement.scrollWidth, viewport: innerWidth }));
  report.widths.push({ stage, ...size }); assert.ok(size.actual <= size.viewport, `${stage}: ${JSON.stringify(size)}`);
}
async function jsonResponse(response) { assert.equal(response.status(), 200, await response.text()); return response.json(); }
async function download(label, filename) {
  const ready = page.waitForEvent('download');
  await page.getByRole('button', { name: label, exact: true }).click();
  const file = await ready; assert.equal(await file.failure(), null);
  const target = path.join(directory, filename); await file.saveAs(target); return readFile(target, 'utf8');
}
async function applyReview(action, button) {
  const posted = page.waitForResponse(response => new URL(response.url()).pathname === reviewPath && response.request().method() === 'POST' && response.request().postDataJSON().action === action);
  const refreshed = page.waitForResponse(dossierResponse);
  await button.click();
  const response = await posted; const receipt = await jsonResponse(response);
  const next = (await jsonResponse(await refreshed)).dossier;
  assert.equal(receipt.action, action); assert.equal(receipt.unitRef, imageUnitRef);
  assert.equal(material(next, imageUnitRef).currentRevisionRef, receipt.currentRevisionRef);
  report.reviews.push({ action, commandRef: receipt.commandRef, candidateRevisionRef: receipt.candidateRevisionRef, currentRevisionRef: receipt.currentRevisionRef, adoption: receipt.adoption, verification: receipt.verification });
  return { receipt, next, command: response.request().postDataJSON() };
}
try {
  const context = await browser.newContext({ viewport: { width: 1440, height: 1000 }, locale: 'zh-CN', acceptDownloads: true });
  await context.route('**/*', route => {
    const request = route.request(); const url = new URL(request.url());
    const allowedReview = request.method() === 'POST' && url.pathname === reviewPath && url.searchParams.get('domain') === domain;
    if (url.origin !== origin || (request.method() !== 'GET' && !allowedReview)) {
      report.blockedRequests.push(`${request.method()} ${request.url()}`); return route.abort();
    }
    if (allowedReview) commands.push(request.postDataJSON());
    return route.continue();
  });
  page = await context.newPage(); page.setDefaultTimeout(25000);
  page.on('pageerror', error => report.pageErrors.push(error.message));
  page.on('console', message => { if (message.type() === 'error') report.consoleErrors.push(message.text()); });
  page.on('request', request => { if (request.method() === 'GET' && new URL(request.url()).pathname === base) report.dossierReads++; });
  const first = page.waitForResponse(dossierResponse);
  await page.goto(`${origin}/corpus/evidence?domain=${domain}&work=${work}&tab=dossier&panel=wide`);
  const initial = (await jsonResponse(await first)).dossier;
  assert.equal(initial.contract, 'research-dossier.v2'); assert.equal(initial.workRef, work);
  for (const forbidden of ['fragments', 'media', 'markdown', 'inputs']) assert.ok(!(forbidden in initial), `${forbidden} is not a parallel reading`);
  const panel = page.locator('#evidence-panel-dossier');
  await panel.getByRole('heading', { name: '研究底稿', exact: true }).waitFor();
  const body = initial.materials.find(unit => unit.kind === 'body'); assert.ok(body);
  const bodyText = body.blocks.filter(block => typeof block.text === 'string').map(block => block.text).join('\n');
  assert.ok(bodyText.length > 100000, `fixture body must exercise full reading: ${bodyText.length}`); assert.ok(bodyText.includes(longTail));
  for (const kind of ['list', 'table', 'quote']) assert.ok(body.blocks.some(block => block.kind === kind), `body preserves ${kind}`);
  const bodyView = panel.locator(`[data-unit-ref="${body.ref}"]`);
  assert.equal(await bodyView.locator(':scope > div > [data-block-ref]').count(), body.blocks.length);
  for (const block of body.blocks.filter(block => typeof block.text === 'string')) {
    const rendered = bodyView.locator(`[data-block-ref="${block.ref}"]`);
    assert.ok((await rendered.textContent()).includes(block.text), `whole ${block.kind} remains in the visible reading`);
    const folding = await rendered.evaluate(node => ({ hiddenDetails: Boolean(node.closest('details:not([open])')), hidden: Boolean(node.closest('[hidden]')), clamp: getComputedStyle(node).webkitLineClamp }));
    assert.equal(folding.hiddenDetails, false); assert.equal(folding.hidden, false); assert.ok(!folding.clamp || folding.clamp === 'none');
  }
  assert.equal(await bodyView.locator('table').count(), body.blocks.filter(block => block.kind === 'table').length);
  report.steps.push('Actual v2 GET renders more than 100k characters, lists, table and quotation continuously without a content fold.');
  await checkWidth('1440 initial reading'); await snapshot('research-dossier-v2-1440.png');
  const tail = body.blocks.find(block => typeof block.text === 'string' && block.text.includes(longTail));
  await bodyView.locator(`[data-block-ref="${tail.ref}"]`).evaluate(node => node.scrollIntoView({ block: 'end' }));
  await snapshot('research-dossier-v2-tail-1440.png');
  await page.setViewportSize({ width: 390, height: 844 });
  await checkWidth('390 full reading');
  assert.ok((await bodyView.textContent()).includes(longTail)); await snapshot('research-dossier-v2-390.png');
  await page.setViewportSize({ width: 1440, height: 1000 });

  const exported = JSON.parse(await download('导出 JSON', 'dossier-original.json'));
  assert.deepEqual(exported, initial, 'JSON export is exactly the received dossier, without UI copies');
  const markdownRead = page.waitForResponse(response => { const url = new URL(response.url()); return url.pathname === base && url.searchParams.get('format') === 'markdown'; });
  const markdown = await download('导出 Markdown', 'dossier-original.md');
  const markdownResponse = await markdownRead;
  assert.equal(markdownResponse.status(), 200); assert.equal(new URL(markdownResponse.url()).searchParams.get('asOf'), initial.asOf);
  assert.ok(markdown.length > 100000); assert.ok(markdown.includes(longTail));
  report.steps.push('JSON download equals the original payload; separate Markdown download uses the same cutoff and contains the complete long body.');

  const image = panel.locator(`[data-unit-ref="${imageUnitRef}"]`);
  const before = material(initial, imageUnitRef); assert.ok(before.currentRevisionRef);
  await image.locator('summary').filter({ hasText: /^对照原图/ }).click();
  const sourceImage = image.getByRole('img'); await sourceImage.waitFor();
  await sourceImage.evaluate(node => { if (node.complete) { if (node.naturalWidth === 0) throw new Error('Synthetic original image failed to load'); return; } return new Promise((resolve, reject) => { node.addEventListener('load', resolve, { once: true }); node.addEventListener('error', () => reject(new Error('Synthetic original image failed to load')), { once: true }); }); });
  assert.ok(await sourceImage.evaluate(node => node.naturalWidth > 0));
  await snapshot('research-dossier-v2-original-1440.png');
  await image.locator('summary').filter({ hasText: /^对照原图/ }).click();
  const candidateRead = page.waitForResponse(response => new URL(response.url()).pathname === `${base}/candidates`);
  await image.getByRole('button', { name: '核对与修正', exact: true }).click();
  const candidates = (await jsonResponse(await candidateRead)).candidates;
  for (const ref of candidateRefs) assert.ok(candidates.some(value => value.candidateRevisionRef === ref && value.unitRef === imageUnitRef));
  const candidate = candidates.find(value => value.candidateRevisionRef === candidateRefs[1]);
  for (const kind of ['table', 'list', 'dialogue']) assert.ok(candidate.blocks.some(block => block.kind === kind), `candidate has ${kind}`);
  const issues = issueList(candidate); assert.ok(issues.some(issue => issue.resolution === 'reading_edit')); assert.ok(issues.some(issue => issue.resolution === 'source_refresh'));
  await image.getByLabel('核对版本', { exact: true }).selectOption(candidateRefs[1]);
  assert.equal(commands.length, 0, 'reading candidates or source images does not adopt or invoke a model');
  await image.getByLabel('审核人', { exact: true }).fill('合成底稿审核人');
  await image.getByLabel('核对依据', { exact: true }).fill('逐块对照受控原图，保存完整结构与来源缺口。');
  await snapshot('research-dossier-v2-candidate-1440.png');
  const adopted = await applyReview('adopt', image.getByRole('button', { name: '采用此候选', exact: true }));
  assert.equal(adopted.command.expectedCurrentRevisionRef, before.currentRevisionRef); assert.equal(adopted.command.candidateRevisionRef, candidateRefs[1]);
  assert.equal(adopted.receipt.currentRevisionRef, candidateRefs[1]); assert.equal(adopted.receipt.adoption, 'human_adopted');
  report.steps.push('Selected candidate 2 by exact identity and adopted through one CAS review command, independently of quality audit.');

  await image.getByRole('button', { name: '逐块修正', exact: true }).click();
  assert.equal(await image.getByLabel('核对版本', { exact: true }).inputValue(), '', 'adopted candidate becomes current, without a stale selection');
  const editedCell = `${'人工核对后的完整表格单元。'.repeat(80)}不得增加，0.5 毫升🙂`;
  const editedList = '有顺序的完整清单：不得遗漏第 2 步。';
  const editedTurn = '他人表述：这个结论存在例外🙂';
  await image.getByLabel('第 1 行，第 1 列', { exact: true }).first().fill(editedCell);
  await image.getByLabel('第 1 项', { exact: true }).first().fill(editedList);
  await image.getByLabel('第 1 轮发言', { exact: true }).first().fill(editedTurn);
  for (const field of await image.getByRole('group', { name: '逐块修正完整读本', exact: true }).locator('textarea').all()) assert.equal(await field.getAttribute('maxlength'), null);
  for (const issue of issues) {
    const checkbox = image.getByRole('checkbox', { name: `已对照来源解决：${issue.message}`, exact: true });
    if (issue.resolution === 'reading_edit') await checkbox.check();
    else assert.equal(await checkbox.count(), 0, 'source refresh gaps cannot be dismissed as text edits');
  }
  await image.getByRole('checkbox', { name: '已对照来源核验当前读本', exact: true }).check();
  await page.setViewportSize({ width: 390, height: 844 }); await checkWidth('390 structured editor');
  assert.equal(await image.getByLabel('第 1 行，第 1 列', { exact: true }).first().inputValue(), editedCell);
  await snapshot('research-dossier-v2-editor-390.png');
  const edited = await applyReview('edit', image.getByRole('button', { name: '保存完整修正版', exact: true }));
  assert.equal(edited.command.expectedCurrentRevisionRef, adopted.receipt.currentRevisionRef); assert.equal(edited.command.candidateRevisionRef, adopted.receipt.currentRevisionRef);
  assert.equal(edited.command.editedBlocks.length, candidate.blocks.length);
  assert.equal(edited.command.editedBlocks.find(block => block.kind === 'table').rows[0][0], editedCell);
  assert.equal(edited.command.editedBlocks.find(block => block.kind === 'list').items[0], editedList);
  assert.equal(edited.command.editedBlocks.find(block => block.kind === 'dialogue').turns[0].text, editedTurn);
  assert.deepEqual([...edited.command.resolvedIssueKeys].sort(), issues.filter(issue => issue.resolution === 'reading_edit').map(issue => issue.key).sort());
  assert.notEqual(edited.receipt.currentRevisionRef, adopted.receipt.currentRevisionRef); assert.equal(edited.receipt.adoption, 'human_edited'); assert.equal(edited.receipt.verification, 'verified');
  const editedUnit = material(edited.next, imageUnitRef);
  assert.equal(editedUnit.blocks.find(block => block.kind === 'table').rows[0][0], editedCell);
  const unresolved = issueList(editedUnit);
  for (const issue of issues) assert.equal(unresolved.some(value => value.key === issue.key), issue.resolution === 'source_refresh');
  await image.getByText(editedCell, { exact: true }).first().waitFor();
  report.steps.push('Edited table/list/dialogue structures on the adopted current reading; full fields survived 390px layout; only selected reading issues were resolved.');

  const kept = await applyReview('keep', image.getByRole('button', { name: '保留当前读本', exact: true }));
  assert.equal(kept.command.expectedCurrentRevisionRef, edited.receipt.currentRevisionRef); assert.equal(kept.command.candidateRevisionRef, edited.receipt.currentRevisionRef);
  assert.equal(kept.receipt.currentRevisionRef, edited.receipt.currentRevisionRef); assert.ok(!('editedBlocks' in kept.command)); assert.deepEqual(kept.command.resolvedIssueKeys, []);
  await image.getByText('已记录保留当前读本。', { exact: true }).waitFor(); await checkWidth('390 persisted reading');
  await snapshot('research-dossier-v2-saved-390.png');
  await page.setViewportSize({ width: 1440, height: 1000 }); await checkWidth('1440 persisted reading'); await snapshot('research-dossier-v2-saved-1440.png');
  const finalExport = JSON.parse(await download('导出 JSON', 'dossier-edited.json'));
  assert.deepEqual(material(finalExport, imageUnitRef).blocks, material(kept.next, imageUnitRef).blocks);
  assert.ok(JSON.stringify(finalExport).includes(longTail));
  report.steps.push('Keep-current preserved the edited revision; final canonical JSON contains the complete revised blocks and untouched long body.');
  assert.deepEqual(commands.map(command => command.action), ['adopt', 'edit', 'keep']); assert.equal(new Set(commands.map(command => command.commandRef)).size, 3);
  assert.deepEqual(report.blockedRequests, []); assert.deepEqual(report.pageErrors, []); assert.deepEqual(report.consoleErrors, []);
  report.finished = true;
  console.log('research-dossier.v2 browser proof: full reading, exact exports, candidate adoption, structured edit and keep-current PASS');
} catch (error) {
  report.error = String(error?.stack ?? error);
  if (page) { await writeFile(path.join(directory, 'browser-failure.html'), await page.content()); await snapshot('browser-failure.png'); }
  throw error;
} finally {
  await writeFile(path.join(directory, 'report.json'), JSON.stringify(report, null, 2));
  await browser.close();
}
