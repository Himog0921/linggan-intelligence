// One production React → Axum → owned synthetic PG path. Never calls a provider or writes through UI.
import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { chromium } from 'playwright';

const directory = process.env.WEB_APP_PROOF_DIR;
assert.ok(directory);
const { origin, domain, work } = JSON.parse(await readFile(path.join(directory, 'fixture.json'), 'utf8'));
const host = new URL(origin);
assert.equal(host.hostname, '127.0.0.1');
assert.notEqual(host.port, '3000');
assert.match(host.port, /^\d+$/);
const report = { finished: false, steps: [], pageErrors: [], blockedRequests: [], reads: 0 };
const browser = await chromium.launch({ headless: true });
try {
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 }, locale: 'zh-CN' });
  await context.route('**/*', route => {
    const request = route.request();
    if (new URL(request.url()).origin !== origin || request.method() !== 'GET') {
      report.blockedRequests.push(`${request.method()} ${request.url()}`);
      return route.abort();
    }
    return route.continue();
  });
  const page = await context.newPage();
  page.on('pageerror', error => report.pageErrors.push(error.message));
  page.on('request', request => { if (new URL(request.url()).pathname.endsWith('/note-material')) report.reads++; });
  await page.goto(`${origin}/corpus/evidence?domain=${domain}&work=${work}&tab=dossier`);
  await page.getByRole('heading', { name: '每篇解读材料', exact: true }).waitFor();
  assert.equal(report.reads, 0, 'folded section does not read material');
  const section = page.locator('section').filter({ has: page.getByRole('heading', { name: '每篇解读材料', exact: true }) });
  const [response] = await Promise.all([
    page.waitForResponse(response => new URL(response.url()).pathname.endsWith('/note-material')),
    section.getByText('展开每篇解读材料', { exact: true }).click(),
  ]);
  assert.equal(response.status(), 200);
  const data = await response.json();
  assert.equal(data.workRef, work); assert.equal(data.domainRef, domain);
  await section.getByText(`底稿准备状态：${data.ready ? '材料已达到后端准备条件' : '材料仍有待处理项'}`, { exact: true }).waitFor();
  assert.equal(report.reads, 1);
  for (const name of ['标题', '封面大字', '正文', '话题标签', '图片文字']) await section.getByRole('heading', { name, exact: true }).waitFor();
  assert.match(await section.innerText(), /来源未提供|尚未取得/);
  await section.getByText('本栏来源受限，文字不在此显示。', { exact: true }).waitFor();
  assert.ok(data.columns.some(column => column.field === 'ocr' && column.state === 'restricted'));
  assert.ok(data.fragments.every(fragment => fragment.field !== 'ocr'), 'actual restricted handler returns no OCR plaintext');
  const title = data.fragments.find(fragment => fragment.field === 'title');
  assert.ok(title);
  await section.getByText(`${title.citationId} · ${Array.from(title.text).length} 字符 · 查看片段与来源`, { exact: true }).click();
  await section.getByText(title.text, { exact: true }).waitFor();
  assert.equal(title.end - title.start, Array.from(title.text).length);
  await section.getByText(`${title.start}–${title.end}`, { exact: true }).waitFor();
  report.steps.push('lazy actual GET, backend readiness, five columns, missing/restricted, literal citation');
  await section.getByText('读取身份与底稿处理范围', { exact: true }).click();
  await section.getByText(data.snapshotIdentity, { exact: true }).waitFor();
  await section.getByText('转写上下文（未纳入默认每篇输入）', { exact: true }).waitFor();
  await section.getByText('读取身份与底稿处理范围', { exact: true }).click();
  await section.getByRole('heading', { name: '标题', exact: true }).evaluate(node => node.scrollIntoView({ block: 'center' }));
  await page.screenshot({ path: path.join(directory, 'note-material-1440.png') });
  await Promise.all([
    page.waitForResponse(response => new URL(response.url()).pathname.endsWith('/note-material')),
    section.getByRole('button', { name: '刷新每篇材料', exact: true }).click(),
  ]);
  await section.getByRole('button', { name: '刷新每篇材料', exact: true }).waitFor();
  await page.waitForFunction(() => ![...document.querySelectorAll('button')].some(button => button.textContent === '刷新每篇材料' && button.disabled));
  assert.equal(report.reads, 2);
  await page.setViewportSize({ width: 390, height: 844 });
  const width = await page.evaluate(() => ({ actual: document.documentElement.scrollWidth, viewport: innerWidth }));
  assert.ok(width.actual <= width.viewport, JSON.stringify(width));
  await page.screenshot({ path: path.join(directory, 'note-material-390.png') });
  report.steps.push('read identity, context boundary, refresh, desktop/narrow readability');
  assert.deepEqual(report.pageErrors, []); assert.deepEqual(report.blockedRequests, []);
  report.finished = true;
} catch (error) {
  report.error = String(error?.stack ?? error);
  throw error;
} finally {
  await writeFile(path.join(directory, 'report.json'), JSON.stringify(report, null, 2));
  await browser.close();
}
