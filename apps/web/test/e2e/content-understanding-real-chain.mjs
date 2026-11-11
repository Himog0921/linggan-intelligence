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
  if (!fixture.live) {
    for (const layout of ['research', 'table', 'cover']) {
      await page.goto(`${origin}/corpus/evidence?domain=${domain}&panel=closed&layout=${layout}`);
      const readyRow = page.locator(`[data-layout="${layout}"] [data-work-ref="${work}"]`);
      const blockedRow = page.locator(`[data-layout="${layout}"] [data-work-ref="${unprocessed}"]`);
      await readyRow.getByText('材料已准备', { exact: true }).first().waitFor();
      await blockedRow.getByText('材料待补齐', { exact: true }).first().waitFor();
      await page.evaluate(() => document.fonts.ready);
      await page.screenshot({ path: path.join(directory, `materials-list-${layout}.png`) });
      if (layout === 'cover') {
        for (const width of [1363, 640, 390]) {
          await page.setViewportSize({ width, height: 844 });
          for (const [row, label] of [[readyRow, '材料已准备'], [blockedRow, '材料待补齐']]) {
            const status = row.getByText(label, { exact: true }).first();
            await status.scrollIntoViewIfNeeded();
            const readable = await status.evaluate(node => {
              const range = document.createRange();
              range.selectNodeContents(node);
              const rects = [...range.getClientRects()];
              for (let parent = node.parentElement; parent; parent = parent.parentElement) {
                const style = getComputedStyle(parent);
                if (style.clipPath !== 'none' || /hidden|clip|auto|scroll/.test(`${style.overflowX} ${style.overflowY}`)) {
                  const bounds = parent.getBoundingClientRect();
                  if (rects.some(rect => rect.left < bounds.left - 1 || rect.right > bounds.right + 1
                    || rect.top < bounds.top - 1 || rect.bottom > bounds.bottom + 1)) return false;
                }
              }
              const observed = node.parentElement.parentElement.querySelector('[title^="最近观察"]')?.getBoundingClientRect();
              return rects.length > 0 && (!observed || rects.every(rect => rect.right <= observed.left + 1
                || rect.left >= observed.right - 1 || rect.bottom <= observed.top + 1 || rect.top >= observed.bottom - 1));
            });
            assert.ok(readable, `${width}宽度下${label}不可裁切或与观察时间重叠`);
            if (width === 390) await row.screenshot({ path: path.join(directory, `materials-list-cover-narrow-${label === '材料已准备' ? 'ready' : 'blocked'}.png`) });
          }
          assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
        }
        await page.setViewportSize({ width: 1363, height: 936 });
      }
    }
    report.steps.push('真实列表接口：研读、表格和封面均区分材料已准备与待补齐，封面窄屏可读。');
  }
  await open(work);
  await understanding.getByText('这篇尚无当前材料的理解结果，以下保留原始材料。', { exact: true }).waitFor();
  await checkOriginal();
  assert.equal(report.writes.length, 0);
  if (!fixture.live) await understanding.getByText('材料已准备', { exact: true }).waitFor();
  report.steps.push('材料已准备但未分析时不写理解完成，原文完整，打开页面不自动生成。');
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
  if (!fixture.live) {
    await understanding.getByText('材料待补齐', { exact: true }).waitFor();
    await understanding.getByText('原始媒体清单尚未确认，暂不能判断图文或视频材料是否齐全。', { exact: true }).waitFor();
  }
  report.steps.push('切换未分析笔记不会串入上篇理解，未知媒体清单显示具体缺项。');
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
  if (!fixture.live) {
    const scopeNote = '本次依据口播与已提取的画面文字，未覆盖整段视频的全部画面。';
    for (const [state, label, message] of [
      ['preparing', '材料准备中', '正在处理画面文字。'],
      ['blocked', '材料待补齐', '原视频本地副本暂不可读取。'],
    ]) {
      await page.route('**/api/local/content-understanding/latest?**', route => route.fulfill({
        status: 200, contentType: 'application/json', body: JSON.stringify({
          ...saved.body, analysis: null, materialReadiness: { state, kind: 'video',
            gaps: [{ code: 'synthetic_presentation', state, message, slotKey: 'video:1' }], scopeNote },
        }),
      }));
      await page.reload();
      await understanding.getByText(label, { exact: true }).waitFor();
      await understanding.getByText(message, { exact: true }).waitFor();
      await checkOriginal();
      await page.setViewportSize({ width: 390, height: 844 });
      assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
      await page.screenshot({ path: path.join(directory, `materials-${state}-narrow.png`) });
      await page.setViewportSize({ width: 1363, height: 936 });
      await page.unroute('**/api/local/content-understanding/latest?**');
    }
    await page.route('**/api/local/content-understanding/latest?**', route => route.fulfill({
      status: 200, contentType: 'application/json', body: JSON.stringify({ ...saved.body,
        materialReadiness: { state: 'ready', kind: 'video', gaps: [], scopeNote } }),
    }));
    await page.reload();
    await checkResult();
    assert.equal(await understanding.getByText(scopeNote, { exact: true }).count(), 0);
    assert.equal(await understanding.getByText('本次按已取得的材料理解，材料缺口见下方原始材料。', { exact: true }).count(), 0);
    await page.screenshot({ path: path.join(directory, 'materials-video-scope.png') });
    await page.unroute('**/api/local/content-understanding/latest?**');
    report.steps.push('仅展示层模拟：准备和缺原件提示正确，已保存结果不插入处理范围说明，390宽度可读且不新增生成请求。');

    const card = report.result.card;
    const question = '开头怎样说明练习的用途？';
    const statement = '先交代材料，再说明练习步骤。';
    const evidence = card.evidence.find(item => typeof item.quote === 'string');
    assert.ok(evidence);
    const deepPath = '/api/local/content-understanding/deep';
    const deep = { commandRef: crypto.randomUUID(), domainRef: domain, workRef: work,
      coreRef: card.analysisRef, question, findings: [{ id: 'f1', findingType: 'structure',
        statement, basisUnitIds: [card.core.mainTopicUnitId], evidenceRefs: [evidence.id] }] };
    assert.equal((await call(deepPath, 'POST', { ...deep, workRef: unprocessed })).status, 409);
    assert.equal((await call(deepPath, 'POST', { ...deep,
      findings: [{ ...deep.findings[0], evidenceRefs: ['missing'] }] })).status, 400);
    const historicalAsOf = (await call(`/api/local/work-resources/${work}/research-dossier?domain=${domain}`)).body.dossier.asOf;
    const published = await call(deepPath, 'POST', deep);
    assert.equal(published.status, 200, JSON.stringify(published.body));
    assert.deepEqual((await call(deepPath, 'POST', deep)).body, published.body);
    assert.equal((await call(latest + '&asOf=' + encodeURIComponent(historicalAsOf))).body.analysis.result.card.deep, null);

    const checkDeep = async () => {
      await checkResult();
      await understanding.getByText(question, { exact: true }).waitFor();
      await understanding.getByText(statement, { exact: false }).waitFor();
      await understanding.getByText('引用依据', { exact: true }).click();
      await understanding.getByText(evidence.quote, { exact: true }).waitFor();
    };
    await page.getByRole('button', { name: '刷新底稿', exact: true }).click();
    await checkDeep();
    await page.reload();
    await checkDeep();
    const refreshed = (await call(latest)).body.analysis;
    assert.deepEqual(refreshed.result.card.core, card.core);
    assert.deepEqual(refreshed.result.card.evidence, card.evidence);
    assert.equal(refreshed.deepQuestion, question);
    assert.equal(refreshed.result.card.deep.analysisRef, published.body.analysisRef);
    assert.deepEqual(refreshed.result.card.deep.findings, deep.findings);
    report.steps.push('人工深化真实写入、幂等、错误作品和无效引用拒绝、历史时间读取及底稿刷新通过。');
    const generatePath = deepPath + '/generate';
    const generatedQuestion = '作者怎样安排材料和核对顺序？';
    const generatedStatement = '作者用材料示范核对步骤。';
    const generate = { commandRef: crypto.randomUUID(), domainRef: domain, workRef: work,
      coreRef: card.analysisRef, question: generatedQuestion,
      taskConfigRef: fixture.deepConfigRef, taskRevision: fixture.deepRevision };
    const denied = await call(generatePath, 'POST', generate);
    assert.equal(denied.status, 409);
    assert.deepEqual(denied.body.reasons, ['egress_not_approved']);
    const egress = await call('/api/local/ai-settings/egress', 'POST', {
      commandRef: crypto.randomUUID(), endpointRef: fixture.endpointRef, taskKey: 'content_understanding_deep',
      materialClass: 'note_material', expectedEventRef: null, allowed: true, reason: '隔离合成端点测试',
    });
    assert.equal(egress.status, 200, JSON.stringify(egress.body));
    assert.equal((await call(generatePath, 'POST', { ...generate, workRef: unprocessed })).status, 409);
    assert.equal((await call(generatePath, 'POST', { ...generate, taskConfigRef: payload.taskConfigRef })).status, 409);
    const waitForDeep = async batchRef => {
      const batchPath = '/api/local/ai-settings/batches/' + batchRef;
      for (let i = 0; i < 300; i += 1) {
        const batch = await call(batchPath);
        assert.equal(batch.status, 200);
        assert.equal(batch.body.batch.taskKey, 'content_understanding_deep');
        const [item] = batch.body.items;
        if (!['pending', 'running'].includes(item.state)) return item;
        await new Promise(resolve => setTimeout(resolve, 200));
      }
      throw new Error('单篇深化未在等待时间内结束');
    };
    const bad = await call(generatePath, 'POST', { ...generate, commandRef: crypto.randomUUID(), question: '合成无效引用' });
    assert.equal(bad.status, 202, JSON.stringify(bad.body));
    const badItem = await waitForDeep(bad.body.batchRef);
    assert.equal(badItem.state, 'failed');
    assert.equal(badItem.failureCode, 'ai_output_invalid');
    assert.deepEqual((await call(latest)).body.analysis.result.card.deep, refreshed.result.card.deep);

    const queued = await call(generatePath, 'POST', generate);
    assert.equal(queued.status, 202, JSON.stringify(queued.body));
    const replayed = await call(generatePath, 'POST', generate);
    assert.deepEqual(replayed.body, queued.body);
    const completed = await waitForDeep(queued.body.batchRef);
    assert.equal(completed.state, 'succeeded', JSON.stringify(completed));
    assert.equal(completed.attemptCount, 1);
    const readGenerated = async () => {
      await checkResult();
      await understanding.getByText(generatedQuestion, { exact: true }).waitFor();
      await understanding.getByText(generatedStatement, { exact: false }).waitFor();
      await understanding.getByText('引用依据', { exact: true }).click();
      await understanding.getByText(evidence.quote, { exact: true }).waitFor();
    };
    await page.getByRole('button', { name: '刷新底稿', exact: true }).click();
    await readGenerated();
    await page.reload();
    await readGenerated();
    const generated = (await call(latest)).body.analysis;
    assert.equal(generated.deepQuestion, generatedQuestion);
    assert.equal(generated.result.card.deep.analysisRef, queued.body.requestRef);
    assert.equal(generated.result.card.deep.methodVersion, 'content-understanding.l2.model.v1');
    assert.deepEqual(generated.result.card.deep.findings, [{ id: 'f1', findingType: 'structure',
      statement: generatedStatement, basisUnitIds: ['u3'], evidenceRefs: [evidence.id] }]);
    assert.deepEqual(generated.result.card.core, card.core);
    assert.deepEqual(generated.result.card.evidence, card.evidence);
    report.generated = generated.result.card.deep;
    await understanding.screenshot({ path: path.join(directory, 'understanding-deep-generated.png') });
    report.steps.push('人工问题经 L2 独立许可、真实队列和合成端点完成深化；无效引用保留原 Deep，底稿与 Card 刷新一致。');
  }
  assert.deepEqual(report.writes, ['/api/local/content-understanding', '/api/local/content-understanding',
    ...(!fixture.live ? [
      ...Array(4).fill('/api/local/content-understanding/deep'),
      '/api/local/content-understanding/deep/generate', '/api/local/ai-settings/egress',
      ...Array(5).fill('/api/local/content-understanding/deep/generate'),
    ] : [])]);
  assert.deepEqual(report.pageErrors, []);
  assert.ok(report.consoleErrors.every(message => message.includes('503')
    || (!fixture.live && /\b(400|409)\b/u.test(message))));
  report.finished = true;
} catch (error) {
  report.error = error.stack;
  throw error;
} finally {
  await writeFile(path.join(directory, 'report.json'), JSON.stringify(report, null, 2));
  await browser.close();
}
