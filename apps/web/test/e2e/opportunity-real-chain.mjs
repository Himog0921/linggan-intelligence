// Built React, actual Axum endpoints and the playbook's synthetic PostgreSQL dataset.
import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { chromium } from 'playwright';

const dir = process.env.WEB_APP_PROOF_DIR;
if (!dir) throw new Error('WEB_APP_PROOF_DIR required');
const { origin, domain, problems } = JSON.parse(await readFile(path.join(dir, 'fixture.json'), 'utf8'));
const report = { steps: [], errors: [], blocked: [], reads: [] };
const browser = await chromium.launch({ headless: true });
const context = await browser.newContext({ viewport: { width: 1440, height: 900 }, reducedMotion: 'reduce' });
await context.route('**/*', route => {
  const request = route.request();
  if (new URL(request.url()).origin !== origin || !['GET', 'HEAD'].includes(request.method())) {
    report.blocked.push({ method: request.method(), url: request.url() });
    return route.abort();
  }
  return route.continue();
});
const page = await context.newPage();
page.on('pageerror', error => report.errors.push(error.message));
page.on('response', response => {
  if (new URL(response.url()).pathname.startsWith('/api/local/')) {
    report.reads.push({ path: new URL(response.url()).pathname, status: response.status() });
  }
});
async function step(name, operation) {
  await operation();
  report.steps.push(name);
  console.log(`Opportunity proof: ${name}`);
}
try {
  await step('home reads cards and retains the selected domain in three entry links', async () => {
    await page.goto(`${origin}/opportunity?domain=${domain}`, { waitUntil: 'networkidle' });
    await page.getByRole('heading', { name: '机会首页', exact: true }).waitFor();
    await page.getByText('本页没有观察到上升的问题。').waitFor();
    const table = page.getByRole('table', { name: '本页其他观察与不可比较项' });
    await table.waitFor({ state: 'visible' });
    await table.getByRole('link', { name: 'synthetic P1' }).waitFor();
    for (const [name, expected] of [
      ['查看用户问题', `/corpus/comments?domain=${domain}&view=problems`],
      ['查看作品依据', `/corpus/evidence?domain=${domain}`],
      ['查看创作者', `/corpus/creators?domain=${domain}`],
    ]) assert.equal(await page.getByRole('link', { name: new RegExp(name) }).getAttribute('href'), expected);
    await page.screenshot({ path: path.join(dir, 'home.png'), fullPage: true });
    await table.getByRole('link', { name: 'synthetic P1' }).click();
    await page.getByRole('heading', { name: 'synthetic P1', exact: true }).waitFor();
  });
  for (const [index, problem] of problems.entries()) {
    await step(`P${index + 1} renders the built card`, async () => {
      if (index > 0) await page.goto(`${origin}/opportunity?domain=${domain}&problem=${problem}`, { waitUntil: 'networkidle' });
      await page.getByRole('heading', { name: `synthetic P${index + 1}`, exact: true }).waitFor();
      await page.getByText('重要性未校准', { exact: true }).first().waitFor();
      await page.getByText('暂不判断', { exact: true }).first().waitFor();
      await page.getByRole('heading', { name: '判断与依据' }).waitFor();
      await page.screenshot({ path: path.join(dir, `p${index + 1}.png`), fullPage: true });
      await page.getByRole('heading', { name: '判断与依据' }).scrollIntoViewIfNeeded();
      await page.screenshot({ path: path.join(dir, `p${index + 1}-evidence.png`), fullPage: true });
    });
  }
  await step('P1 reads comment, work and creator evidence from the real API', async () => {
    await page.goto(`${origin}/opportunity?domain=${domain}&problem=${problems[0]}`, { waitUntil: 'networkidle' });
    for (const [name, kind] of [['评论依据', 'comments'], ['作品依据', 'works'], ['作者依据', 'creators']]) {
      const response = page.waitForResponse(r => r.url().includes(`/evidence?kind=${kind}`));
      await page.getByRole('button', { name }).click();
      const result = await response;
      assert.equal(result.status(), 200, `${kind} evidence`);
      await page.getByText(/依据读取于/).waitFor();
    }
  });
  assert.deepEqual(report.errors, []);
  assert.deepEqual(report.blocked, []);
  assert.ok(report.reads.some(read => read.path.endsWith('/opportunities') && read.status === 200));
  console.log('Opportunity proof: all synthetic reads passed; no writes or external requests');
} finally {
  await writeFile(path.join(dir, 'report.json'), JSON.stringify(report, null, 2));
  await browser.close();
}
