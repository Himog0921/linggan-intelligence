// Production React → actual Axum → isolated synthetic PostgreSQL. No response mocks or model calls.
import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { chromium } from 'playwright';

const directory = process.env.WEB_APP_PROOF_DIR;
assert.ok(directory, 'WEB_APP_PROOF_DIR is required');
const fixture = JSON.parse(await readFile(path.join(directory, 'fixture.json'), 'utf8'));
const { origin, domain, work, longTail } = fixture;
const host = new URL(origin);
assert.equal(host.hostname, '127.0.0.1');
assert.notEqual(host.port, '3000');
assert.match(host.port, /^\d+$/);
for (const ref of [domain, work]) assert.match(ref, /^[0-9a-f-]{36}$/i);
const report = { finished: false, contract: 'research-dossier.v2', steps: [], widths: [], pageErrors: [], consoleErrors: [], blockedRequests: [], markdownSnapshots: [] };
const browser = await chromium.launch({ headless: true, ...(process.env.DOSSIER_BROWSER_EXECUTABLE ? { executablePath: process.env.DOSSIER_BROWSER_EXECUTABLE } : {}) });
let page;
let active;
const apiPath = ref => `/api/local/work-resources/${ref}/research-dossier`;
const responseFor = (ref, markdown = false) => response => {
  const url = new URL(response.url());
  return url.pathname === apiPath(ref) && (url.searchParams.get('format') === 'markdown') === markdown && response.request().method() === 'GET';
};
async function snapshot(name) { await page.screenshot({ path: path.join(directory, name) }); }
async function readResponse(response) { assert.equal(response.status(), 200, await response.text()); return response.json(); }
async function checkWidth(stage) {
  const size = await page.evaluate(() => ({ actual: document.documentElement.scrollWidth, viewport: innerWidth }));
  report.widths.push({ stage, ...size });
  assert.ok(size.actual <= size.viewport, `${stage}: ${JSON.stringify(size)}`);
  assert.ok(await page.locator('[data-dossier-document]').evaluate(node => node.scrollWidth <= node.clientWidth + 1), `${stage}: document has no horizontal overflow`);
}
async function openReading(item) {
  const response = page.waitForResponse(responseFor(item.work));
  await page.goto(`${origin}/corpus/evidence?domain=${item.domain}&work=${item.work}&tab=dossier&panel=wide`);
  const dossier = (await readResponse(await response)).dossier;
  await page.getByRole('article', { name: '连续阅读文档', exact: true }).waitFor();
  active = { ...item, dossier };
  return dossier;
}
async function assertReading(dossier) {
  const panel = page.locator('#evidence-panel-dossier');
  const sections = dossier.readingView.sections;
  assert.deepEqual(await panel.locator('[data-section-ref]').evaluateAll(nodes => nodes.map(node => node.dataset.sectionRef).filter(ref => ref !== 'reading:missing')), sections.map(section => section.ref));
  const refs = sections.flatMap(section => section.groups.flatMap(group => group.blockRefs));
  assert.equal(new Set(refs).size, refs.length, 'the sole reading plan references each canonical block once');
  assert.deepEqual(new Set(refs), new Set([...dossier.readingView.mainBlockRefs, ...dossier.readingView.supplementBlockRefs]), 'legacy Works and new reading cover the same blocks without imposing a new Works order');
  const blocks = new Map(dossier.materials.flatMap(unit => unit.blocks).map(block => [block.ref, block]));
  for (const section of sections) for (const group of section.groups) {
    const node = panel.locator(`[data-reading-anchor="${group.ref}"]`);
    assert.equal(await node.count(), 1, 'each stable reading group appears once');
    const text = await node.textContent();
    for (const ref of group.blockRefs) {
      const block = blocks.get(ref); assert.ok(block);
      const pieces = typeof block.text === 'string' ? [block.text]
        : block.kind === 'list' ? block.items
          : block.kind === 'table' ? block.rows.flat()
            : block.kind === 'dialogue' ? block.turns.map(turn => turn.text) : [];
      if (group.mode !== 'tags') for (const piece of pieces) assert.ok(text.includes(piece), `full canonical text survives: ${ref}`);
    }
    const hidden = await node.evaluate(element => ({ folded: Boolean(element.closest('details:not([open]), [hidden]')), clamp: getComputedStyle(element).webkitLineClamp }));
    assert.equal(hidden.folded, false);
    assert.ok(!hidden.clamp || hidden.clamp === 'none');
  }
  assert.equal(await page.locator('[data-work-title]').count(), 1, 'the shared title is not duplicated in the document');
  for (const text of ['查看来源', '原件与修正', '映射来源', '查看原始发布文案']) assert.equal(await panel.getByText(text, { exact: true }).count(), 0);
  assert.equal(await panel.locator('img').count(), 0, 'the dossier is the reading, including multi-image works');
}
async function download(label, filename) {
  const ready = page.waitForEvent('download');
  await page.getByRole('button', { name: label, exact: true }).click();
  const file = await ready; assert.equal(await file.failure(), null);
  const target = path.join(directory, filename); await file.saveAs(target); return readFile(target, 'utf8');
}
async function exportSnapshot(label) {
  const root = page.locator('[data-dossier-document]');
  const before = await root.evaluate(node => node.scrollTop);
  const received = page.waitForResponse(responseFor(active.work, true));
  await page.locator('#evidence-panel-dossier').getByRole('button', { name: '导出', exact: true }).click();
  const response = await received;
  assert.equal(response.status(), 200);
  const url = new URL(response.url());
  assert.ok(url.searchParams.get('asOf'));
  assert.ok(url.searchParams.get('expectedRevision'));
  const pinnedUrl = new URL(url); pinnedUrl.searchParams.delete('format');
  const pinnedResponse = await page.context().request.get(pinnedUrl.href);
  assert.equal(pinnedResponse.status(), 200);
  const pinned = (await pinnedResponse.json()).dossier;
  assert.equal(url.searchParams.get('asOf'), pinned.asOf);
  assert.equal(url.searchParams.get('expectedRevision'), pinned.revision);
  const markdown = await response.text();
  const dialog = page.getByRole('dialog', { name: '导出研究底稿' });
  await dialog.getByRole('article', { name: 'Markdown 排版预览' }).waitFor();
  await dialog.getByRole('button', { name: '源码', exact: true }).click();
  assert.equal(await dialog.getByLabel('Markdown 源码', { exact: true }).textContent(), markdown);
  assert.equal(await download('下载 .md', `${label}.md`), markdown, 'download uses exactly the preview bytes');
  await dialog.getByRole('button', { name: '复制内容', exact: true }).click();
  await dialog.getByText('已复制完整底稿', { exact: true }).waitFor();
  assert.equal(await page.evaluate(() => navigator.clipboard.readText()), markdown, 'clipboard contains the same complete file');
  await dialog.getByRole('button', { name: 'JSON', exact: true }).click();
  const json = await download('下载 .json', `${label}.json`);
  assert.equal(await dialog.getByLabel('JSON 源码', { exact: true }).textContent(), json);
  assert.deepEqual(JSON.parse(json), pinned, 'JSON is the canonical snapshot frozen by this export session');
  assert.ok(!markdown.includes('## 来源材料') && !markdown.includes('## 映射来源'));
  report.markdownSnapshots.push({ work: active.work, asOf: pinned.asOf, revision: pinned.revision, bytes: Buffer.byteLength(markdown) });
  await page.keyboard.press('Escape');
  await dialog.waitFor({ state: 'detached' });
  assert.ok(Math.abs(await root.evaluate(node => node.scrollTop) - before) <= 2, 'closing export preserves reading position');
  assert.equal(await page.getByRole('tab', { name: '研究底稿', exact: true }).getAttribute('aria-selected'), 'true');
  return markdown;
}
async function jumpToAudio(dossier) {
  const sectionRef = dossier.readingView.sections.find(section => section.kind === 'audio').ref;
  await page.getByRole('button', { name: /^章节导航/ }).click();
  await page.getByRole('navigation', { name: '底稿章节', exact: true }).getByRole('button', { name: /视频口播/ }).click();
  await page.waitForFunction(ref => {
    const section = document.querySelector(`[data-section-ref="${ref}"]`);
    const root = document.querySelector('[data-dossier-document]');
    if (!section || !root) return false;
    const offset = section.getBoundingClientRect().top - root.firstElementChild.getBoundingClientRect().bottom;
    return offset >= 0 && offset < 32;
  }, sectionRef);
  return page.locator(`[data-section-ref="${sectionRef}"]`);
}
try {
  const context = await browser.newContext({ viewport: { width: 1363, height: 936 }, locale: 'zh-CN', acceptDownloads: true, reducedMotion: 'reduce', permissions: ['clipboard-read', 'clipboard-write'] });
  await context.route('**/*', route => {
    const request = route.request(), url = new URL(request.url());
    if (url.protocol === 'blob:') return route.continue();
    if (url.origin !== origin || !['GET', 'HEAD'].includes(request.method())) {
      report.blockedRequests.push(`${request.method()} ${request.url()}`); return route.abort();
    }
    return route.continue();
  });
  page = await context.newPage(); page.setDefaultTimeout(25000);
  page.on('pageerror', error => report.pageErrors.push(error.message));
  page.on('console', message => { if (message.type() === 'error') report.consoleErrors.push(message.text()); });
  const initial = await openReading({ domain, work });
  assert.equal(initial.contract, 'research-dossier.v2');
  const body = initial.materials.find(unit => unit.kind === 'body');
  assert.ok(body.blocks.map(block => block.text ?? '').join('').length > 100000);
  assert.ok(body.blocks.some(block => block.text?.includes(longTail)));
  await assertReading(initial);
  assert.ok((await exportSnapshot('full-reading')).includes(longTail));
  report.steps.push('Full 100k+ character canonical body and all structures survive reading, Markdown, JSON, copy and download.');
  for (const scenario of fixture.scenarios) {
    const dossier = await openReading(scenario);
    await assertReading(dossier);
    await checkWidth(`${scenario.name} standard`);
    if (scenario.name === 'approved-demo') {
      await snapshot('approved-demo-reading.png');
      const root = page.locator('[data-dossier-document]');
      const audio = await jumpToAudio(dossier);
      assert.ok((await audio.textContent()).includes('出现缺口时下一步怎么办。'), 'all 4m52s speech is visible in the main document');
      assert.equal(await audio.locator('[data-transcript-paragraph]').count(), 12);
      await snapshot('approved-demo-transcript.png');
      const before = await root.evaluate(node => node.scrollTop);
      await audio.getByRole('button', { name: '专注阅读', exact: true }).click();
      const focus = page.getByRole('dialog', { name: '视频口播 · 专注阅读' });
      await focus.waitFor({ state: 'visible' });
      await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
      assert.equal(await focus.locator('[data-transcript-paragraph]').count(), 12);
      assert.ok((await focus.textContent()).includes('出现缺口时下一步怎么办。'));
      await focus.getByLabel('定位口播时间', { exact: true }).selectOption({ label: '02:53' });
      await page.waitForFunction(() => document.querySelector('[aria-label="完整口播正文"]').scrollTop > 0);
      const focusTop = await focus.getByLabel('完整口播正文', { exact: true }).evaluate(node => node.scrollTop);
      await snapshot('approved-demo-focus.png');
      await page.keyboard.press('Escape');
      await focus.waitFor({ state: 'detached' });
      assert.ok(Math.abs(await root.evaluate(node => node.scrollTop) - before) <= 2);
      await audio.getByRole('button', { name: '专注阅读', exact: true }).click();
      await focus.waitFor({ state: 'visible' });
      await page.waitForFunction(top => Math.abs(document.querySelector('[aria-label="完整口播正文"]').scrollTop - top) <= 2, focusTop);
      await page.keyboard.press('Escape');
      await focus.waitFor({ state: 'detached' });
      assert.ok(Math.abs(await root.evaluate(node => node.scrollTop) - before) <= 2, 'focus reopening preserves both the focus and main document positions');
      await page.getByRole('tab', { name: '作品', exact: true }).click();
      await page.getByRole('tab', { name: '研究底稿', exact: true }).click();
      await page.waitForFunction(top => Math.abs(document.querySelector('[data-dossier-document]').scrollTop - top) <= 2, before);
      assert.ok(Math.abs(await root.evaluate(node => node.scrollTop) - before) <= 2, 'switching Works returns to the same reading position');
      await exportSnapshot('approved-demo');
      for (const width of [1280, 900, 680, 390]) {
        await page.setViewportSize({ width, height: 936 });
        await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
        await checkWidth(`approved-demo ${width}`);
        await snapshot(`approved-demo-${width}.png`);
      }
      await page.setViewportSize({ width: 1363, height: 936 });
      report.steps.push('Approved demo: continuous 26-segment / 4m52s speech, focus/Esc/tab return, and narrow viewports.');
    }
    if (['twelve-images', 'thirteen-images'].includes(scenario.name)) {
      const total = scenario.name === 'twelve-images' ? 12 : 13;
      assert.equal(dossier.materials.filter(unit => unit.kind === 'image').length, total);
      assert.ok((await page.locator('[data-dossier-document]').textContent()).includes(`第${total}页完整文字`));
      await exportSnapshot(scenario.name);
    }
    if (scenario.name === 'partial-images') {
      assert.ok(dossier.materials.some(unit => unit.processing?.state === 'failed'));
      assert.ok((await page.locator('[data-dossier-document]').textContent()).includes('待补全'));
      await snapshot('partial-images-reading.png');
    }
    if (scenario.name === 'originals-only') {
      assert.equal(dossier.readingView.sections.length, 0);
      assert.ok((await page.locator('[data-dossier-document]').textContent()).includes('缺失内容'));
    }
    report.steps.push(`Read-only scenario ${scenario.name}: ${dossier.readingView.sections.length} sections, ${dossier.materials.length} actual materials.`);
  }
  assert.deepEqual(report.blockedRequests, []);
  assert.deepEqual(report.pageErrors, []);
  assert.deepEqual(report.consoleErrors, []);
  report.finished = true;
  console.log('research-dossier.v2 browser proof PASS: continuous reading, frozen exports, 12/13 images, long speech and no writes');
} catch (error) {
  report.error = String(error?.stack ?? error);
  if (page) { await writeFile(path.join(directory, 'browser-failure.html'), await page.content()); await snapshot('browser-failure.png'); }
  throw error;
} finally {
  await writeFile(path.join(directory, 'report.json'), JSON.stringify(report, null, 2));
  await browser.close();
}
