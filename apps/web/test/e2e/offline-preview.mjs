// One real React -> Axum -> pure-contract journey. The Rust host has no configured database.
import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';

const directory = process.env.C0_PREVIEW_PROOF_DIR;
assert.ok(directory);
const fixture = JSON.parse(await readFile(path.join(directory, 'fixture.json'), 'utf8'));
const { origin } = fixture;
const base = new URL(origin);
assert.equal(base.protocol, 'http:'); assert.equal(base.hostname, '127.0.0.1'); assert.notEqual(base.port, '3000');
assert.equal(fixture.database, 'NotConfigured');
const report = { scope: 'fixed synthetic sample; no database, provider or worker', steps: [], errors: [], blocked: [], screenshots: [] };
const browser = await chromium.launch({ headless: true });
const context = await browser.newContext({ viewport: { width: 1440, height: 1000 }, reducedMotion: 'reduce' });
await context.route('**/*', route => {
  const request = route.request(), url = new URL(request.url());
  if (url.origin !== origin || (request.method() !== 'GET' && url.pathname !== '/api/local/ai-offline-preview')) {
    report.blocked.push(`${request.method()} ${url.origin}${url.pathname}`); return route.abort();
  }
  return route.continue();
});
const page = await context.newPage(); page.setDefaultTimeout(20000);
page.on('pageerror', error => report.errors.push(error.message));
async function step(name, action) {
  await action(); const file = `offline-${report.steps.length + 1}.png`;
  await page.screenshot({ path: path.join(directory, file), fullPage: true });
  report.screenshots.push(file); report.steps.push({ name, result: 'PASS' }); console.log(`C0 UI PASS: ${name}`);
}
async function calculate(action) {
  const response = page.waitForResponse(r => r.request().method() === 'POST' && new URL(r.url()).pathname === '/api/local/ai-offline-preview');
  const [result] = await Promise.all([response, action()]);
  assert.ok(result.ok(), await result.text()); return result.json();
}
try {
  await step('existing topics entry opens explicit synthetic input and fixed output', async () => {
    await page.goto(`${origin}/topics`);
    await page.getByRole('link', { name: '离线预演', exact: true }).click();
    await page.getByRole('heading', { name: '方向与解读 · 离线预演' }).waitFor();
    await page.getByRole('heading', { name: '合成输入', exact: true }).waitFor();
    assert.ok(await page.getByText('合成样本 · 未调用模型 · 未写正式业务结果', { exact: true }).isVisible());
    await page.getByText('固定样本身份与边界', { exact: true }).click();
    assert.match(await page.locator('main').innerText(), /没有采样真实库/);
    assert.ok(await page.getByLabel('合成笔记').getByText(/合成点赞：未知/).count() > 0);
  });
  await step('session classification clears reason without a formal save', async () => {
    for (const [button, message] of [
      ['恢复此篇示例', '已恢复此篇固定示例，未写业务记录。'],
      ['恢复全部示例', '已恢复固定示例；本次会话裁定已清除。'],
    ]) {
      await page.getByLabel('相关性', { exact: true }).selectOption('unknown');
      await page.getByLabel('裁定理由', { exact: true }).fill('尚未应用的草稿');
      const restored = await calculate(() => page.getByRole('button', { name: button, exact: true }).click());
      await page.getByText(message, { exact: true }).waitFor();
      const expected = restored.notes.find(n => n.title.startsWith('合成01')).result.effective;
      assert.equal(await page.getByLabel('相关性', { exact: true }).inputValue(), expected.relevance);
      assert.equal(await page.getByLabel('裁定理由', { exact: true }).inputValue(), expected.reason ?? '');
    }
    await page.getByLabel('裁定理由', { exact: true }).fill('');
    const data = await calculate(() => page.getByRole('button', { name: '应用到本次预演', exact: true }).click());
    const first = data.notes.find(n => n.title.startsWith('合成01'));
    assert.equal(first.result.manualApplied, true); assert.equal(first.result.effective.reason, null);
    await page.getByText('本次预演已更新，未保存到业务记录。', { exact: true }).waitFor();
  });
  await step('candidate merge retains the old judgment and asks for review', async () => {
    const data = await calculate(() => page.getByLabel('示例版本', { exact: true }).selectOption('candidate'));
    const first = data.notes.find(n => n.title.startsWith('合成01'));
    assert.equal(first.result.needsReview, true); assert.equal(first.result.manualApplied, false);
    assert.notEqual(first.result.effective.primaryDirection, first.result.judgment.primaryDirection);
    await page.getByRole('status').filter({ hasText: '原裁定方向已合并或移除' }).waitFor();
    await page.getByText('旧版与当前机器示例对照', { exact: true }).click();
    assert.match(await page.locator('aside').innerText(), /保留的会话裁定：拖延与启动/);
    await page.getByLabel('主方向', { exact: true }).selectOption({ label: '走神' });
    const confirmed = await calculate(() => page.getByRole('button', { name: '应用到本次预演', exact: true }).click());
    assert.equal(confirmed.notes.find(n => n.title.startsWith('合成01')).result.needsReview, false);
    const returned = await calculate(() => page.getByLabel('示例版本', { exact: true }).selectOption('baseline'));
    assert.equal(returned.notes.find(n => n.title.startsWith('合成01')).result.manualApplied, true);
  });
  await step('catalogue comparison, sample links and main-only aggregates use the server contract', async () => {
    await page.getByRole('button', { name: '方向清单', exact: true }).click();
    await page.getByRole('heading', { name: '合成方向清单 · 20 个方向', exact: true }).waitFor();
    await page.getByText('旧结构对照：保留、合并、删除、新增', { exact: true }).click();
    assert.match(await page.locator('main').innerText(), /启动与作业/);
    await page.getByRole('button', { name: '合成01', exact: true }).first().click();
    await page.getByRole('heading', { name: '合成输入', exact: true }).waitFor();
    await page.getByText('查看本次合成样本汇总 · 6 篇', { exact: true }).click();
    assert.match(await page.locator('main').innerText(), /点赞未知 2 篇/);
    const result = await page.request.get(`${origin}/api/local/ai-offline-preview`);
    const snapshot = await result.json();
    assert.equal(snapshot.summary.directions.reduce((n, row) => n + row.count, 0), 3);
    assert.equal(snapshot.summary.directions.length, 20);
  });
  await step('reload clears session judgments and narrow main stays readable', async () => {
    await page.reload(); await page.getByRole('heading', { name: '合成输入', exact: true }).waitFor();
    assert.equal(await page.getByText('当前显示本次会话裁定。', { exact: true }).count(), 0);
    await page.setViewportSize({ width: 390, height: 844 });
    const widths = await page.locator('main').evaluate(el => ({ client: el.clientWidth, scroll: el.scrollWidth }));
    assert.ok(widths.scroll <= widths.client + 1, JSON.stringify(widths));
    assert.equal(await page.getByRole('button', { name: '应用到本次预演', exact: true }).count(), 1);
  });
  assert.deepEqual(report.blocked, []); assert.deepEqual(report.errors, []);
} catch (error) {
  report.errors.push(String(error.stack ?? error)); process.exitCode = 1;
} finally {
  await writeFile(path.join(directory, 'report.json'), JSON.stringify(report, null, 2));
  await browser.close();
}
