import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { chromium } from 'playwright';

const directory = process.env.WEB_APP_PROOF_DIR;
assert.ok(directory);
const fixture = JSON.parse(await readFile(path.join(directory, 'fixture.json'), 'utf8'));
const { origin, domain, work, unprocessed, body } = fixture;
assert.equal(new URL(origin).hostname, '127.0.0.1');
assert.ok(!['3000', '3001'].includes(new URL(origin).port));
const report = { finished: false, steps: [], pageErrors: [], consoleErrors: [], blockedRequests: [], writes: [], result: null };
const browser = await chromium.launch({ headless: true, ...(process.env.DOSSIER_BROWSER_EXECUTABLE ? { executablePath: process.env.DOSSIER_BROWSER_EXECUTABLE } : {}) });
try {
  const context = await browser.newContext({ viewport: { width: 1363, height: 936 }, locale: 'zh-CN', reducedMotion: 'reduce' });
  await context.route('**/*', route => {
    const request = route.request();
    if (new URL(request.url()).origin !== origin) {
      report.blockedRequests.push(`${request.method()} ${request.url()}`);
      return route.abort();
    }
    if (!['GET', 'HEAD'].includes(request.method())) report.writes.push(new URL(request.url()).pathname);
    return route.continue();
  });
  const page = await context.newPage();
  page.setDefaultTimeout(25000);
  page.on('pageerror', error => report.pageErrors.push(error.message));
  page.on('console', message => { if (message.type() === 'error') report.consoleErrors.push(message.text()); });
  const selection = ref => new URLSearchParams({ domainRef: domain, workRef: ref }).toString();
  const latest = `/api/local/content-understanding/latest?${selection(work)}`;
  const open = async ref => {
    await page.goto(`${origin}/corpus/evidence?domain=${domain}&work=${ref}&tab=dossier&panel=wide`);
    await page.getByRole('article', { name: '连续阅读文档', exact: true }).waitFor();
    await page.evaluate(() => document.fonts.ready);
  };
  const understanding = page.getByRole('region', { name: '整篇内容理解', exact: true });
  const original = page.getByRole('article', { name: '连续阅读文档', exact: true });
  const call = async (url, method = 'GET', payload) => page.evaluate(async ({ url, method, payload }) => {
    const response = await fetch(url, { method, ...(payload ? { headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(payload) } : {}) });
    return { status: response.status, body: await response.json() };
  }, { url, method, payload });

  const checkOriginal = async () => {
    const rendered = (await original.textContent()).replace(/\s/gu, '');
    for (const text of fixture.originalTexts ?? [body]) {
      assert.ok(rendered.includes(text.replace(/\s/gu, '')), '原始文字不可遗漏');
    }
  };
  await open(work);
  await understanding.getByText('这篇尚无当前材料的理解结果，以下保留原始材料。', { exact: true }).waitFor();
  await checkOriginal();
  assert.equal(report.writes.length, 0);
  report.steps.push('未分析时原文完整，打开页面不自动生成。');
  const preview = await call(`/api/local/content-understanding/preview?${selection(work)}`);
  assert.equal(preview.status, 200);
  assert.equal(preview.body.canStart, true, JSON.stringify(preview.body.reasons));
  assert.equal(preview.body.imageCount, fixture.imageCount ?? 1);
  const payload = { commandRef: crypto.randomUUID(), domainRef: domain, workRef: work,
    knowledgeCutoff: preview.body.knowledgeCutoff, inputHash: preview.body.inputHash,
    taskConfigRef: preview.body.taskConfigRef, taskRevision: preview.body.taskRevision };
  if (fixture.live) await writeFile(path.join(directory, 'dispatch.json'), JSON.stringify(payload), { flag: 'wx', mode: 0o600 });
  const started = await call('/api/local/content-understanding', 'POST', payload);
  assert.equal(started.status, 202, JSON.stringify(started.body));
  const replay = await call('/api/local/content-understanding', 'POST', payload);
  assert.equal(replay.status, 202);
  assert.equal(replay.body.requestRef, started.body.requestRef);
  const saved = await page.evaluate(async ({ url, waitMs }) => {
    const deadline = Date.now() + waitMs;
    while (Date.now() < deadline) {
      const response = await fetch(url);
      const body = await response.json();
      if (['succeeded', 'failed', 'skipped'].includes(body.analysis?.state)) return { status: response.status, body };
      await new Promise(resolve => setTimeout(resolve, 200));
    }
    throw new Error('理解任务未在等待时间内结束');
  }, { url: latest, waitMs: fixture.waitMs ?? 60000 });
  await writeFile(path.join(directory, 'saved.json'), JSON.stringify(saved, null, 2));
  assert.equal(saved.body.analysis.state, 'succeeded', saved.body.analysis.failureCode ?? 'no terminal success');
  assert.equal(saved.body.analysis.result.card.analysisRef, started.body.requestRef);
  report.result = saved.body.analysis.result;
  assert.ok(!JSON.stringify(saved.body).includes('rawOutput'));
  const summary = report.result.card.core.summary.text;
  const checkResult = async () => {
    await understanding.getByText(summary, { exact: true }).waitFor();
    for (const unit of report.result.card.core.knowledgeUnits) assert.ok((await understanding.textContent()).includes(unit.statement));
    await checkOriginal();
  };
  const dossierBefore = await call(`/api/local/work-resources/${work}/research-dossier?domain=${domain}`);
  assert.equal(dossierBefore.status, 200);
  await page.getByRole('button', { name: '刷新底稿', exact: true }).click();
  await checkResult();
  const dossierAfter = await call(`/api/local/work-resources/${work}/research-dossier?domain=${domain}`);
  assert.equal(dossierAfter.status, 200);
  assert.equal(dossierAfter.body.dossier.revision, dossierBefore.body.dossier.revision);
  report.steps.push('材料版本未变时，点击刷新底稿也会读到刚保存的理解。');
  await page.reload();
  await checkResult();
  await page.evaluate(() => document.fonts.ready);
  await page.screenshot({ path: path.join(directory, 'understanding-desktop.png') });
  report.steps.push('正式HTTP生成→队列→Node SDK→保存→页面与Content Card读取同一Core；刷新后保留。');
  await page.setViewportSize({ width: 390, height: 844 });
  await checkResult();
  assert.ok(await original.evaluate(node => node.scrollWidth <= node.clientWidth + 1));
  assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
  await page.screenshot({ path: path.join(directory, 'understanding-narrow.png') });
  report.steps.push('390宽度可读，无横向溢出。');
  await page.setViewportSize({ width: 1363, height: 936 });
  await open(unprocessed);
  await understanding.getByText('这篇尚无当前材料的理解结果，以下保留原始材料。', { exact: true }).waitFor();
  assert.ok((await original.textContent()).includes('这篇合成原文尚未经过理解，仍可完整阅读。'));
  assert.ok(!(await understanding.textContent()).includes(summary));
  report.steps.push('切换未分析笔记不会串入上篇理解。');
  await open(work);
  await checkResult();
  assert.deepEqual((await call(latest)).body.analysis.result, report.result);
  assert.equal(report.blockedRequests.length, 0);
  assert.deepEqual(report.pageErrors, []);
  assert.deepEqual(report.consoleErrors, []);
  let failedAsOf;
  await page.route('**/api/local/content-understanding/latest?**', route => {
    failedAsOf = new URL(route.request().url()).searchParams.get('asOf');
    return route.fulfill({ status: 503, contentType: 'application/json', body: '{"error":"synthetic_read_failure"}' });
  });
  await page.reload();
  await understanding.getByText('理解结果暂时无法读取，原始材料仍可阅读。', { exact: false }).waitFor();
  await checkOriginal();
  assert.ok(!(await understanding.textContent()).includes(summary));
  await page.screenshot({ path: path.join(directory, 'understanding-read-failure.png') });
  await page.unroute('**/api/local/content-understanding/latest?**');
  assert.ok(failedAsOf);
  const retried = page.waitForRequest(request => new URL(request.url()).pathname === '/api/local/content-understanding/latest');
  await understanding.getByRole('button', { name: '重试读取', exact: true }).click();
  assert.equal(new URL((await retried).url()).searchParams.get('asOf'), failedAsOf);
  await checkResult();
  report.steps.push('人为模拟读取失败时原文可读、不展示旧理解；重试只读取、不重新生成。');
  assert.deepEqual(report.writes, ['/api/local/content-understanding', '/api/local/content-understanding']);
  assert.deepEqual(report.pageErrors, []);
  assert.ok(report.consoleErrors.every(message => message.includes('503')));
  report.finished = true;
} catch (error) {
  report.error = error.stack;
  throw error;
} finally {
  await writeFile(path.join(directory, 'report.json'), JSON.stringify(report, null, 2));
  await browser.close();
}
