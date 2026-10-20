// AI-GEN1 / G1 approved four sections: real React + Axum + owned PostgreSQL.
// No executor is started; all saved connections use an inert loopback endpoint.
import assert from 'node:assert/strict';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { chromium } from 'playwright';
const directory = process.env.WEB_APP_PROOF_DIR;
assert.ok(directory, 'owned proof directory is required');
const { origin } = JSON.parse(await readFile(path.join(directory, 'fixture.json'), 'utf8'));
assert.equal(new URL(origin).hostname, '127.0.0.1');
const longName = 'SYNTHETIC长名称' + '家庭养育'.repeat(20);
const longUrl = 'http://127.0.0.1:9/v1/SYNTHETIC_' + 'a'.repeat(600);
const modelId = 'synthetic-' + 'm'.repeat(180);
const skillEnd = 'SYNTHETIC长文结束锚点';
const report = { scope: 'synthetic isolated database; no model execution', steps: [], screenshots: [], errors: [], httpFailures: [], posts: [], failedRequests: [], consoleErrors: [], assetErrors: [] };
const browser = await chromium.launch({ headless: true });
const context = await browser.newContext({ viewport: { width: 1440, height: 900 }, locale: 'zh-CN', timezoneId: 'Asia/Shanghai' });
await context.route('**/*', route => new URL(route.request().url()).origin === origin ? route.continue() : route.abort());
const page = await context.newPage();
page.on('pageerror', error => report.errors.push(error.message));
page.on('console', message => { if (message.type() === 'error') report.consoleErrors.push(message.text()); });
page.on('requestfailed', request => report.failedRequests.push(`${request.method()} ${new URL(request.url()).pathname} ${request.failure()?.errorText ?? ''}`));
page.on('response', response => { const pathname = new URL(response.url()).pathname; if (pathname.startsWith('/ui-assets/') && response.status() >= 400) report.assetErrors.push(`${response.request().method()} ${pathname} ${response.status()}`); });
page.on('response', response => { if (response.status() >= 400) report.httpFailures.push({ path: new URL(response.url()).pathname, status: response.status() }); });
page.on('request', request => { if (request.method() === 'POST') report.posts.push(new URL(request.url()).pathname); });
async function step(name, action) {
  try { await action(); report.steps.push({ name, result: 'PASS' }); console.log(`settings PASS: ${name}`); }
  catch (error) { report.steps.push({ name, result: 'FAIL', error: String(error.stack ?? error) }); throw error; }
}
async function goto(query = '') { await page.goto(`${origin}/settings/models${query}`); await page.getByRole('heading', { name: 'AI 设置', exact: true }).waitFor(); }
async function receipt(text) { await page.getByRole('status').filter({ hasText: text }).waitFor(); }
try {
  await step('new connection save does not test or enqueue', async () => {
    await goto();
    await page.getByLabel('连接名称', { exact: true }).fill(longName);
    await page.getByLabel('服务地址', { exact: true }).fill(longUrl);
    await page.getByLabel('API 密钥', { exact: true }).fill('SYNTHETIC-NOT-A-CREDENTIAL');
    await page.getByText('本机端点', { exact: true }).check();
    await page.getByRole('button', { name: '保存连接', exact: true }).click();
    await receipt('连接已保存');
    await page.getByLabel('登记模型标识', { exact: true }).fill(modelId);
    await page.getByRole('button', { name: '登记模型', exact: true }).click();
    await receipt('模型标识已保存');
  });
  await step('manual capability and zero versus unknown prices', async () => {
    await page.getByLabel('上下文容量', { exact: true }).fill('4096');
    await page.getByLabel('最大输出容量', { exact: true }).fill('512');
    await page.getByLabel('最长等待秒数', { exact: true }).fill('30');
    await page.getByLabel('声明依据及版本', { exact: true }).fill('synthetic-browser-manual-v1');
    await page.getByRole('checkbox', { name: '关闭思考', exact: true }).check();
    await page.getByRole('button', { name: '保存能力声明', exact: true }).click();
    await receipt('能力声明已保存');
    await page.getByLabel('输入价格', { exact: true }).fill('0');
    await page.getByLabel('价格依据及版本', { exact: true }).fill('synthetic-price-v1');
    await page.getByRole('button', { name: '保存价格', exact: true }).click();
    await receipt('价格已保存');
  });
  await step('quota save is separate from enablement', async () => {
    await goto('?section=automation&task=model_probe');
    await page.getByLabel('每日额度', { exact: true }).selectOption('unlimited');
    await page.getByRole('button', { name: '保存配置', exact: true }).click();
    await receipt('配置已保存');
    assert.equal(await page.getByRole('button', { name: '启用全局 AI', exact: true }).isDisabled(), true);
  });
  const skillDirectory = path.join(directory, 'browser-example');
  await mkdir(skillDirectory, { recursive: true });
  await writeFile(path.join(skillDirectory, 'SKILL.md'), `---\nname: browser-example\ndescription: ${'合成做法说明'.repeat(35)}\n---\nUse only the explicitly selected writing material.\n${Array.from({ length: 150 }, (_, i) => `第${i + 1}段：合成长文🙂。${'x'.repeat(180)}`).join('\n')}\n${skillEnd}\n`);
  await step('folder import creates a fixed local version', async () => {
    await goto('?section=skills&task=writing_legacy');
    await page.getByLabel('选择做法目录', { exact: true }).setInputFiles(skillDirectory);
    await page.getByRole('button', { name: '导入做法包', exact: true }).click();
    await receipt('已导入做法');
  });
  const settings = await (await page.request.get(`${origin}/api/local/ai-settings`)).json();
  const model = settings.models.find(m => m.modelId === modelId);
  const version = settings.skills.find(s => s.name === 'browser-example').versions[0];
  assert.ok(model.capability); assert.equal(model.price.input, '0'); assert.equal(model.price.output, null);
  await step('writing candidate save and reasoned first-version activation', async () => {
    await goto('?section=automation&task=writing_legacy');
    await page.getByLabel('选择模型', { exact: true }).selectOption(model.identityRef);
    await page.getByLabel('输入 token 上限', { exact: true }).fill('1024');
    await page.getByLabel('输出 token 上限', { exact: true }).fill('128');
    await page.getByLabel('单次等待秒数', { exact: true }).fill('5');
    await page.getByLabel('思考等级', { exact: true }).selectOption('off');
    await page.getByLabel('固定做法版本', { exact: true }).selectOption(version.versionRef);
    await page.getByLabel('每日额度', { exact: true }).selectOption('unlimited');
    await page.getByRole('button', { name: '保存配置', exact: true }).click();
    await receipt('候选配置已保存');
    await goto('?section=skills&task=writing_legacy');
    await page.getByRole('checkbox', { name: '此做法在本工作首版，申请跳过旧新比较', exact: true }).check();
    await page.getByLabel('启用理由', { exact: true }).fill('Synthetic first-version browser activation; switches remain off.');
    await page.getByRole('button', { name: '启用候选做法', exact: true }).click();
    await receipt('固定做法版本已启用');
  });
  await step('all four sections remain readable at four viewports', async () => {
    for (const [width, height] of [[1280, 800], [1440, 900], [1920, 1080], [390, 844]]) {
      await page.setViewportSize({ width, height });
      for (const section of ['models', 'automation', 'skills', 'health']) {
        const selection = section === 'models' ? `&connection=${model.connectionRef}&model=${model.identityRef}` : section === 'skills' ? `&task=writing_legacy&version=${version.versionRef}` : '';
        await goto(`?section=${section}${selection}`);
        await page.getByRole('heading', { name: { models: '模型', automation: '自动化与额度', skills: '做法', health: '运行状况' }[section], exact: true, level: 2 }).waitFor();
        if (section === 'skills') {
          await page.getByText('只读做法内容', { exact: true }).click();
          const body = page.locator('details[open] pre');
          await body.waitFor();
          assert.ok((await body.innerText()).endsWith(skillEnd));
          await body.evaluate(element => { element.scrollTop = element.scrollHeight; });
          assert.ok(await body.evaluate(element => element.scrollTop + element.clientHeight >= element.scrollHeight - 1));
        }
        const dimensions = await page.evaluate(() => {
          const main = document.querySelector('main');
          return { client: document.documentElement.clientWidth, scroll: document.documentElement.scrollWidth, mainClient: main.clientWidth, mainScroll: main.scrollWidth };
        });
        assert.ok(dimensions.scroll <= dimensions.client, `${width} ${section} must not overflow document`);
        assert.ok(dimensions.mainScroll <= dimensions.mainClient, `${width} ${section} long fields must not overflow main: ${JSON.stringify(dimensions)}`);
        const name = `settings-${section}-${width}.png`;
        await page.screenshot({ path: path.join(directory, name), fullPage: true }); report.screenshots.push(name);
      }
    }
    await page.keyboard.press('Tab'); assert.ok(await page.evaluate(() => document.activeElement !== document.body));
  });
  await step('actual read receipts preserve off, not started, and no invocation', async () => {
    const current = await (await page.request.get(`${origin}/api/local/ai-settings`)).json();
    assert.equal(current.execution.service.observedState, 'not_started');
    assert.deepEqual(current.batches, []); assert.deepEqual(current.execution.dailyUsage, []);
    assert.ok(current.controls.every(control => control.enabled === false));
    assert.equal(current.tasks.find(task => task.taskKey === 'writing_legacy').activeSkillVersionRef, version.versionRef);
    assert.ok(report.posts.every(route => !route.endsWith('/probes') && !route.endsWith('/batches') && !route.endsWith('/control')));
    assert.deepEqual(report.errors, []);
    assert.deepEqual(report.httpFailures, []);
    // A navigation may cancel the large font GET; an answered asset error is never a cancellation.
    const cancelledRead = entry => entry.startsWith('GET ') && entry.endsWith('net::ERR_ABORTED');
    assert.deepEqual(report.failedRequests.filter(entry => !cancelledRead(entry)), []);
    assert.deepEqual(report.consoleErrors.filter(text => !text.includes('Failed to load resource')), []);
    assert.deepEqual(report.assetErrors, [], 'static assets must not answer with HTTP errors');
  });
} finally {
  if (report.steps.at(-1)?.result === 'FAIL') await page.screenshot({ path: path.join(directory, 'settings-failure.png'), fullPage: true }).catch(() => {});
  await writeFile(path.join(directory, 'settings-report.json'), `${JSON.stringify(report, null, 2)}\n`);
  await browser.close();
}
console.log(`AI-GEN1 settings browser: ${report.steps.length} steps passed, ${report.screenshots.length} screenshots, no queue or model calls.`);
