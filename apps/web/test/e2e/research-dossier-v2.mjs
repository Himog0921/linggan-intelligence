// Production React → actual Axum → owned synthetic PostgreSQL. No response mocks or model calls.
import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { chromium } from 'playwright';

const directory = process.env.WEB_APP_PROOF_DIR;
assert.ok(directory, 'WEB_APP_PROOF_DIR is required');
const fixture = JSON.parse(await readFile(path.join(directory, 'fixture.json'), 'utf8'));
const { origin, domain, work, imageUnitRef, currentReadingRef, longTail } = fixture;
const host = new URL(origin);
assert.equal(host.hostname, '127.0.0.1'); assert.notEqual(host.port, '3000'); assert.match(host.port, /^\d+$/);
for (const ref of [domain, work, imageUnitRef, currentReadingRef]) assert.match(ref, /^[0-9a-f-]{36}$/i);
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
async function focusBelowHeader(target) {
  const position = await target.evaluate(node => {
    const inspector = node.closest('aside'); const header = inspector.querySelector(':scope > header');
    const inset = parseFloat(getComputedStyle(inspector).getPropertyValue('--lgi-space-4'));
    inspector.scrollTop += node.getBoundingClientRect().top - header.getBoundingClientRect().bottom - inset;
    return { targetTop: node.getBoundingClientRect().top, headerBottom: header.getBoundingClientRect().bottom };
  });
  assert.ok(position.targetTop >= position.headerBottom - 1, `focused evidence clears the sticky header: ${JSON.stringify(position)}`);
}

async function checkWidth(stage) {
  const size = await page.evaluate(() => ({ actual: document.documentElement.scrollWidth, viewport: innerWidth }));
  report.widths.push({ stage, ...size }); assert.ok(size.actual <= size.viewport, `${stage}: ${JSON.stringify(size)}`);
  if (size.viewport >= 1280) {
    const clippedMetrics = await page.locator('[data-layout="research"] [data-engagement-metrics]').evaluateAll(rows => rows.flatMap(row => [...row.children].filter(item => {
      const text = item.querySelector('b'); const box = item.getBoundingClientRect(); const available = row.getBoundingClientRect();
      return text && (text.scrollHeight > Math.ceil(parseFloat(getComputedStyle(text).lineHeight)) || box.right > available.right + 1 || box.height > available.height + 1);
    }).map(item => item.textContent)));
    assert.deepEqual(clippedMetrics, [], `${stage}: interaction labels fit one unclipped line`);
  }
}
async function assertReading(dossier, panel) {
  const refs = [...dossier.readingView.mainBlockRefs, ...dossier.readingView.supplementBlockRefs].filter(ref => !dossier.note.titleBlockRefs.includes(ref));
  assert.deepEqual(await panel.locator('[data-block-ref]').evaluateAll(nodes => nodes.map(node => node.getAttribute('data-block-ref'))), refs, 'the reading follows the canonical ref order exactly once');
  const title = dossier.note.displayTitle ?? '无标题';
  assert.equal(await page.locator('[data-work-title]').count(), 1, 'one visible work title');
  assert.equal(await page.locator('[data-work-title]').textContent(), title, 'the shared header uses the canonical title');
  for (const ref of refs) {
    const block = dossier.materials.flatMap(unit => unit.blocks).find(block => block.ref === ref);
    const rendered = panel.locator(`[data-block-ref="${ref}"]`);
    if (typeof block.text === 'string') assert.ok((await rendered.textContent()).includes(block.text), `complete source text ${ref}`);
  }
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
  report.reviews.push({ action, commandRef: receipt.commandRef, currentRevisionRef: receipt.currentRevisionRef, adoption: receipt.adoption, verification: receipt.verification });
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
  await assertReading(initial, panel);
  assert.equal(await panel.getByLabel('修正人', { exact: true }).count(), 0, 'reading does not mount an editor');
  await assertReading(initial,panel);
  for(const width of [1440,1280]) {await page.setViewportSize({width,height:1000});await checkWidth(`${width} complete reading`);await snapshot(`research-dossier-v2-${width}.png`);}
  const exported=JSON.parse(await download('导出 JSON','dossier-initial.json'));
  assert.deepEqual(exported,initial,'JSON is the same canonical snapshot without a second OCR result copy');
  const markdown=await download('导出 Markdown','dossier-initial.md');assert.ok(markdown.includes(longTail));
  const originals=panel.locator('summary').filter({hasText:/^原件与修正/}).locator('xpath=..');
  assert.equal(await originals.getAttribute('open'),null,'source comparison starts folded while text is readable');
  await panel.locator(`[data-unit-ref="${imageUnitRef}"]`).getByRole('button',{name:'查看来源',exact:true}).first().click();
  assert.notEqual(await originals.getAttribute('open'),null,'source action opens the actual original container');
  const image=panel.locator(`[data-source-unit-ref="${imageUnitRef}"]`);
  const compare=image.locator('details').filter({hasText:/^对照原图/}).first();
  assert.notEqual(await compare.getAttribute('open'),null,'source action opens the actual original image');
  await image.getByRole('img').evaluate(node=>node.decode());
  assert.equal(await image.getByRole('img').evaluate(node=>node.naturalWidth>0),true);
  assert.equal(await page.evaluate(()=>document.activeElement?.dataset.sourceUnitRef),imageUnitRef,'source action exposes a keyboard focus target');
  await focusBelowHeader(image);await snapshot('research-dossier-v2-source-1280.png');
  const metadataReady=page.waitForResponse(response=>new URL(response.url()).pathname===`${base}/metadata`);
  await panel.getByText('作者、时间与互动',{exact:true}).click();
  const metadata=await jsonResponse(await metadataReady);assert.ok(metadata.metadata.publication);
  const metadataPanel=panel.getByText('作者、时间与互动',{exact:true}).locator('xpath=..');
  await metadataPanel.getByText(/分享/).waitFor();await metadataPanel.getByText(/观察时间：/).first().waitFor();
  await snapshot('research-dossier-v2-metadata-1280.png');
  const canonicalImage=material(initial,imageUnitRef);assert.equal(canonicalImage.currentRevisionRef,currentReadingRef);
  const issues=issueList(canonicalImage);assert.ok(issues.some(issue=>issue.resolution==='reading_edit'));assert.ok(issues.some(issue=>issue.resolution==='source_refresh'));
  await image.getByRole('button',{name:'修正文字',exact:true}).click();
  assert.equal(await image.getByRole('combobox',{name:'核对版本'}).count(),0);
  await image.getByLabel('修正人',{exact:true}).fill('合成底稿修正人');
  await image.getByLabel('修正说明',{exact:true}).fill('逐块对照受控原图，保存完整结构与来源缺口。');
  const editedCell=`${'人工核对后的完整表格单元。'.repeat(80)}不得增加，0.5 毫升🙂`;
  const editedList='有顺序的完整清单：不得遗漏第 2 步。';const editedTurn='他人表述：这个结论存在例外🙂';
  await image.getByLabel('第 1 行，第 1 列',{exact:true}).first().fill(editedCell);
  await image.getByLabel('第 1 项',{exact:true}).first().fill(editedList);
  await image.getByLabel('第 1 轮发言',{exact:true}).first().fill(editedTurn);
  for(const field of await image.getByRole('group',{name:'逐块修正完整读本',exact:true}).locator('textarea').all()) assert.equal(await field.getAttribute('maxlength'),null);
  for(const issue of issues) {
    const checkbox=image.getByRole('checkbox',{name:`已对照原件解决：${issue.message}`,exact:true});
    if(issue.resolution==='reading_edit') await checkbox.check();else assert.equal(await checkbox.count(),0);
  }
  await image.getByRole('checkbox',{name:'已对照原件核验文字',exact:true}).check();
  for(const width of [1440,1280]) {await page.setViewportSize({width,height:1000});await checkWidth(`${width} structured editor`);await focusBelowHeader(image.getByRole('group',{name:'逐块修正完整读本',exact:true}));await snapshot(`research-dossier-v2-editor-${width}.png`);}
  const edited=await applyReview('edit',image.getByRole('button',{name:'保存修正',exact:true}));
  assert.equal(edited.command.expectedCurrentRevisionRef,currentReadingRef);assert.equal(edited.command.sourceVersion,canonicalImage.sourceVersion);assert.equal(edited.command.unitRef,imageUnitRef);assert.ok(!('candidateRevisionRef' in edited.command));
  assert.equal(edited.command.editedBlocks.length,canonicalImage.blocks.length);
  assert.equal(edited.command.editedBlocks.find(block=>block.kind==='table').rows[0][0],editedCell);
  assert.equal(edited.command.editedBlocks.find(block=>block.kind==='list').items[0],editedList);
  assert.equal(edited.command.editedBlocks.find(block=>block.kind==='dialogue').turns[0].text,editedTurn);
  assert.deepEqual([...edited.command.resolvedIssueKeys].sort(),issues.filter(issue=>issue.resolution==='reading_edit').map(issue=>issue.key).sort());
  assert.notEqual(edited.receipt.currentRevisionRef,currentReadingRef);assert.equal(edited.receipt.adoption,'human_edited');assert.equal(edited.receipt.verification,'verified');
  const editedUnit=material(edited.next,imageUnitRef);assert.equal(editedUnit.blocks.find(block=>block.kind==='table').rows[0][0],editedCell);
  const unresolved=issueList(editedUnit);for(const issue of issues) assert.equal(unresolved.some(value=>value.key===issue.key),issue.resolution==='source_refresh');
  const finalExport=JSON.parse(await download('导出 JSON','dossier-edited.json'));assert.deepEqual(material(finalExport,imageUnitRef).blocks,editedUnit.blocks);assert.ok(JSON.stringify(finalExport).includes(longTail));
  assert.deepEqual(commands.map(command=>command.action),['edit']);
  await page.setViewportSize({width:1440,height:1000});await focusBelowHeader(panel.locator(`[data-unit-ref="${imageUnitRef}"]`).first());await snapshot('research-dossier-v2-saved-1440.png');
  report.steps.push('Read one canonical snapshot; inspected actual local originals and metadata; saved one full structural edit with source/CAS identity at 1440/1280 desktop.');
  for (const [index, scenario] of (fixture.scenarios ?? []).entries()) {
    assert.match(scenario.work, /^[0-9a-f-]{36}$/i);
    const scenarioDomain = scenario.domain ?? domain;
    const read = page.waitForResponse(response => new URL(response.url()).pathname === `/api/local/work-resources/${scenario.work}/research-dossier` && response.request().method() === 'GET');
    await page.goto(`${origin}/corpus/evidence?domain=${scenarioDomain}&work=${scenario.work}&tab=dossier&panel=wide`);
    const dossier = (await jsonResponse(await read)).dossier;
    await panel.getByRole('heading', { name: '研究底稿', exact: true }).waitFor();
    await assertReading(dossier, panel);
    assert.equal(await panel.getByLabel('修正人', { exact: true }).count(), 0);
    assert.equal(await panel.locator('[data-source-unit-ref]').count(), dossier.materials.length);
    for (const width of [1440, 1280]) {
      await page.setViewportSize({ width, height: 1000 });
      await checkWidth(`${scenario.name} ${width}`);
      await snapshot(`standard-dossier-${index + 1}-${width}.png`);
    }
    if (scenario.name === 'video-transcript') {
      const mediaOriginals = dossier.materials.filter(unit => ['audio', 'video_frame'].includes(unit.kind));
      assert.ok(mediaOriginals.length >= 1);
      const sourceDetails=panel.locator('summary').filter({hasText:/^原件与修正/}).locator('xpath=..');
      if(await sourceDetails.getAttribute('open')===null) await sourceDetails.locator(':scope > summary').click();
      for (const unit of mediaOriginals) {
        const source = panel.locator(`[data-source-unit-ref="${unit.ref}"]`);
        const link = source.getByRole('link', { name: '查看音视频原件', exact: true });
        const url = new URL(await link.getAttribute('href'), origin);
        assert.equal(url.origin, origin, 'video original stays on the owned candidate');
        assert.equal(await source.locator('img').count(), 0, 'an original video is never decoded as an image');
        const original = await context.request.get(url.href);
        assert.equal(original.status(), 200);
        assert.match(original.headers()['content-type'], /^video\/mp4/);
        assert.ok((await original.body()).length > 0, 'saved original bytes are readable');
      }
      report.steps.push('Both video-derived units expose the actual local MP4 original through links, with successful same-origin video/mp4 GETs and no image decoder.');
    }
    if (['video-transcript', 'partial-images', 'originals-only'].includes(scenario.name)) {
      for (const width of [1440, 1280]) {
        await page.setViewportSize({ width, height: 1000 });
        if (scenario.name === 'video-transcript') {
          const supplements = panel.getByRole('region', { name: '补充材料', exact: true });
          await focusBelowHeader(supplements);
          await snapshot(`standard-dossier-video-supplements-${width}.png`);
          const audioUnit = dossier.materials.find(unit => unit.kind === 'audio');
          await focusBelowHeader(panel.locator(`[data-source-unit-ref="${audioUnit.ref}"]`));
          await snapshot(`standard-dossier-video-originals-${width}.png`);
        } else if (scenario.name === 'partial-images') {
          assert.ok(dossier.materials.some(unit => unit.processing?.state === 'failed'), 'partial fixture contains an actual failed processing receipt');
          const stateDetails=panel.locator('summary').filter({hasText:/^材料状态/}).locator('xpath=..');
          if(await stateDetails.getAttribute('open')===null) await stateDetails.locator(':scope > summary').click();
          const failure = panel.getByText(/^处理失败(?:，已有读本仍可读)?。$/).first();
          await focusBelowHeader(failure.locator('xpath=ancestor::section[1]'));
          await snapshot(`standard-dossier-partial-failure-${width}.png`);
        } else {
          const originals = panel.locator('summary').filter({hasText:/^原件与修正/}).locator('xpath=..');
      if(await originals.getAttribute('open')===null) await originals.locator(':scope > summary').click();
          const imageUnit = dossier.materials.find(unit => unit.kind === 'image' && unit.originalUrl);
          if (imageUnit) {
            const source = originals.locator(`[data-source-unit-ref="${imageUnit.ref}"]`);
            const compare = source.locator('details').filter({ hasText: /^对照原图/ }).first();
            if (await compare.getAttribute('open') === null) await compare.locator('summary').first().click();
            const image = source.getByRole('img');
            await image.evaluate(node => {
              if (node.complete) { if (node.naturalWidth === 0) throw new Error('Synthetic attachment image failed to load'); return; }
              return new Promise((resolve, reject) => { node.addEventListener('load', resolve, { once: true }); node.addEventListener('error', () => reject(new Error('Synthetic attachment image failed to load')), { once: true }); });
            });
          }
          await focusBelowHeader(originals);
          await snapshot(`standard-dossier-empty-originals-${width}.png`);
        }
        await checkWidth(`${scenario.name} focused ${width}`);
      }
    }
    report.steps.push(`Canonical scenario ${scenario.name}: ${dossier.readingView.mainBlockRefs.length} main and ${dossier.readingView.supplementBlockRefs.length} supplement refs.`);
  }
  assert.deepEqual(report.blockedRequests, []); assert.deepEqual(report.pageErrors, []); assert.deepEqual(report.consoleErrors, []);
  report.finished = true;
  console.log('research-dossier.v2 browser proof: full canonical reading, exact exports, original source, metadata and structured edit PASS');
} catch (error) {
  report.error = String(error?.stack ?? error);
  if (page) { await writeFile(path.join(directory, 'browser-failure.html'), await page.content()); await snapshot('browser-failure.png'); }
  throw error;
} finally {
  await writeFile(path.join(directory, 'report.json'), JSON.stringify(report, null, 2));
  await browser.close();
}
