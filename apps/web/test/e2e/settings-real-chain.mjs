// WEB-REACT-001 阶段 B 真实链路证明：本机 Axum 路由 + 隔离 PostgreSQL + 已构建的 React 前端。
// 由 apps/api 的 `settings_page_real_chain_browser_proof`（scripts/test-web-app-postgres.sh）启动。
// 只访问 fixture 给出的本机地址，其余请求一律拦截；只用合成凭据，模型连接指向本机回环端口。
import assert from 'node:assert/strict';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { chromium } from 'playwright';

const directory = process.env.WEB_APP_PROOF_DIR;
assert.ok(directory, 'WEB_APP_PROOF_DIR is required');
const { origin } = JSON.parse(await readFile(path.join(directory, 'fixture.json'), 'utf8'));
await mkdir(directory, { recursive: true });

const report = { origin, steps: [], screenshots: [], pageErrors: [], consoleErrors: [], failedRequests: [], probeBodies: [], settingsReads: [] };
let currentStep = 'setup';
const step = async (name, run) => {
  currentStep = name;
  try {
    await run();
    report.steps.push({ name, result: 'PASS' });
  } catch (error) {
    report.steps.push({ name, result: 'FAIL', error: String(error?.stack ?? error) });
    throw error;
  }
};

const browser = await chromium.launch({ headless: true });
const context = await browser.newContext({ viewport: { width: 1440, height: 900 }, locale: 'zh-CN', timezoneId: 'Asia/Shanghai' });
await context.route('**/*', route => (new URL(route.request().url()).origin === origin ? route.continue() : route.abort()));
const page = await context.newPage();
page.on('pageerror', error => report.pageErrors.push(error.message));
page.on('console', message => {
  if (message.type() === 'error') report.consoleErrors.push(message.text());
});
const settingsReads = new Map();
page.on('requestfailed', request => {
  report.failedRequests.push(`${request.method()} ${new URL(request.url()).pathname} ${request.failure()?.errorText ?? ''}`);
  const read = settingsReads.get(request);
  if (read) Object.assign(read, { failedStep: currentStep, error: request.failure()?.errorText, endedAt: Date.now() });
});
page.on('requestfinished', request => {
  const read = settingsReads.get(request);
  if (read) Object.assign(read, { finishedStep: currentStep, endedAt: Date.now() });
});
page.on('request', request => {
  if (request.method() === 'GET' && new URL(request.url()).pathname === '/api/local/model-settings') {
    const read = { number: report.settingsReads.length + 1, startedStep: currentStep, startedAt: Date.now() };
    settingsReads.set(request, read);
    report.settingsReads.push(read);
  }
  if (request.method() === 'POST' && new URL(request.url()).pathname === '/api/local/model-settings/probes') {
    report.probeBodies.push(JSON.parse(request.postData() ?? '{}'));
  }
});

const shot = async name => {
  const file = path.join(directory, `${name}.png`);
  await page.screenshot({ path: file });
  const layout = await page.evaluate(() => ({ viewport: innerWidth, documentWidth: document.documentElement.scrollWidth }));
  report.screenshots.push({ name, ...layout });
  return layout;
};
const status = () => page.locator('#settings-feedback');
const dialog = () => page.getByRole('dialog');

// A write receipt can precede its invalidation GET. Await that actual new read,
// its body, and the rendered ready control before another write or navigation.
const afterSettingsRead = async action => {
  const [response] = await Promise.all([
    page.waitForResponse(response => response.request().method() === 'GET' && new URL(response.url()).pathname === '/api/local/model-settings'),
    action(),
  ]);
  assert.equal(response.status(), 200);
  assert.equal(await response.finished(), null);
  await page.getByRole('button', { name: '刷新', exact: true }).waitFor();
};

let finished = false;
try {
  await step('入口跳转与页面加载', async () => {
    let response;
    await afterSettingsRead(async () => { response = await page.goto(`${origin}/settings`); });
    assert.equal(response?.status(), 200);
    assert.equal(new URL(page.url()).pathname, '/settings/models');
    await page.getByRole('heading', { name: '评论研究的模型设置' }).waitFor();
    assert.match(await page.title(), /模型与 AI/);
    // 共享外壳来自 shell.css：页头存在且一级导航六项。
    const nav = page.getByRole('navigation', { name: '一级导航' });
    assert.equal(await nav.locator('a, button').count(), 6);
    assert.equal(await nav.getByRole('link', { name: /语料/ }).getAttribute('href'), '/corpus');
    const headerBorder = await page.locator('.v7-global-row').evaluate(node => getComputedStyle(node).borderBottomWidth);
    assert.equal(headerBorder, '2px');
  });

  await step('空状态如实说明', async () => {
    await page.getByText('还没有供应商连接。').waitFor();
    await page.getByText('尚未保存默认研究模型。先添加并测试连接。').waitFor();
    await page.getByText('还没有调用记录。').waitFor();
    const layout = await shot('settings-1440-empty');
    assert.ok(layout.documentWidth <= layout.viewport, '1440 宽度不应横向溢出');
  });

  await step('键盘打开对话框，Esc 关闭并把焦点还给触发按钮', async () => {
    const add = page.getByRole('button', { name: '添加连接' });
    await add.focus();
    await page.keyboard.press('Enter');
    await dialog().waitFor();
    await page.keyboard.press('Escape');
    await dialog().waitFor({ state: 'detached' });
    assert.equal(await page.evaluate(() => document.activeElement?.textContent), '添加连接');
  });

  await step('添加连接（只保存，不测试）', async () => {
    await page.getByRole('button', { name: '添加连接' }).click();
    const form = dialog();
    await form.getByLabel('连接名称').fill('合成连接');
    await form.getByLabel('API 地址').fill('http://127.0.0.1:9/v1');
    await form.getByLabel('API Key').fill('SYNTHETIC-NOT-A-CREDENTIAL');
    await form.getByLabel('模型 ID').fill('synthetic-model');
    await form.getByLabel('本机回环服务（允许 HTTP）').check();
    await shot('settings-1440-dialog');
    await afterSettingsRead(() => form.getByRole('button', { name: '保存', exact: true }).click());
    await status().getByText('供应商连接已保存；尚未发送测试请求。').waitFor();
    await dialog().waitFor({ state: 'detached' });
    const table = page.getByRole('table').first();
    await table.getByText('合成连接').waitFor();
    await table.getByText('需要完成模型测试', { exact: false }).waitFor();
  });

  await step('测试模型：结果按真实回执显示，失败原因保留错误码', async () => {
    await afterSettingsRead(() => page.getByRole('button', { name: '测试', exact: true }).click());
    await status().getByText('模型测试未通过', { exact: false }).waitFor();
    const code = await status().locator('code').textContent();
    assert.ok(code && code.length > 0, '失败回执应带错误码');
    report.probeFailureCode = code;
    const ledger = page.getByRole('table').nth(1);
    await ledger.getByText('能力测试 · synthetic-model').waitFor();
    await ledger.getByText('失败').first().waitFor();
    await shot('settings-1440-after-probe');
  });

  await step('请求没有答复时显示「未确认」，重试沿用同一个请求编号', async () => {
    let aborted = false;
    await page.route('**/api/local/model-settings/probes', async route => {
      if (!aborted) {
        aborted = true;
        await route.abort('connectionreset');
      } else {
        await route.continue();
      }
    });
    const before = report.probeBodies.length;
    await page.getByRole('button', { name: '测试', exact: true }).click();
    await status().getByText('结果未确认', { exact: false }).waitFor();
    await afterSettingsRead(() => page.getByRole('button', { name: '测试', exact: true }).click());
    await status().getByText('模型测试未通过', { exact: false }).waitFor();
    await page.unroute('**/api/local/model-settings/probes');
    const attempts = report.probeBodies.slice(before);
    assert.equal(attempts.length, 2);
    assert.equal(attempts[0].invocationRef, attempts[1].invocationRef);
  });

  await step('刷新页面后数据仍在（服务器为准）', async () => {
    await afterSettingsRead(() => page.reload());
    await page.getByRole('table').first().getByText('合成连接').waitFor();
    await page.getByRole('table').nth(1).getByText('能力测试 · synthetic-model').first().waitFor();
  });

  await step('停用与启用：带修订号提交，回执后更新', async () => {
    await afterSettingsRead(() => page.getByRole('button', { name: '停用' }).click());
    await status().getByText('连接已停用。').waitFor();
    await page.getByRole('table').first().getByText('已停用', { exact: true }).waitFor();
    await afterSettingsRead(() => page.getByRole('button', { name: '启用' }).click());
    await status().getByText('连接已启用。').waitFor();
  });

  await step('未通过测试的模型不能保存为默认研究模型', async () => {
    const select = page.getByRole('combobox', { name: '研究模型' });
    report.modelOptions = await select.evaluate(node => [...node.options].map(option => ({ text: option.textContent, value: option.value, disabled: option.disabled })));
    const candidates = report.modelOptions.filter(option => option.value !== '');
    assert.ok(candidates.length > 0, '下拉框里应有已登记的模型');
    assert.ok(candidates.every(option => option.disabled), '未通过 V1 JSON 测试的模型必须不可选');
    let configPosts = 0;
    page.on('request', request => {
      if (request.method() === 'POST' && new URL(request.url()).pathname === '/api/local/model-settings/config') configPosts += 1;
    });
    await page.getByRole('button', { name: '保存默认配置' }).click();
    assert.equal(configPosts, 0);
  });

  await step('编辑连接且模型 ID 不变：新版本下重新登记模型，不再报版本冲突', async () => {
    await page.getByRole('button', { name: '编辑' }).click();
    const form = dialog();
    await form.getByLabel('连接名称').fill('合成连接（改）');
    await afterSettingsRead(() => form.getByRole('button', { name: '保存', exact: true }).click());
    await status().getByText('供应商连接已保存；尚未发送测试请求。').waitFor();
    const table = page.getByRole('table').first();
    await table.getByText('合成连接（改）').waitFor();
    await table.getByText('synthetic-model').waitFor();
  });

  await step('短屏可以滚到页面底部', async () => {
    await page.setViewportSize({ width: 1280, height: 720 });
    const note = page.getByText('调用记录保留使用量和安全传输诊断，不保存评论正文或模型原始输出。');
    await note.scrollIntoViewIfNeeded();
    assert.equal(await note.isVisible(), true);
    await shot('settings-1280x720-bottom');
  });

  for (const [width, height] of [
    [1024, 768],
    [390, 844],
  ]) {
    await step(`${width} 宽度无横向溢出`, async () => {
      await page.setViewportSize({ width, height });
      await afterSettingsRead(() => page.goto(`${origin}/settings/models`));
      await page.getByRole('heading', { name: '评论研究的模型设置' }).waitFor();
      // 等数据读完再量：连接表、默认配置与调用记录都在页面上时才算真实布局。
      await page.getByRole('table').first().getByText('合成连接（改）').waitFor();
      await page.getByRole('heading', { name: '后台执行器与调用记录' }).waitFor();
      const layout = await shot(`settings-${width}`);
      assert.ok(layout.documentWidth <= layout.viewport, `${width} 宽度不应横向溢出`);
    });
  }

  await step('从新页头进入语料：/corpus 跳转到证据库（React 页）', async () => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await afterSettingsRead(() => page.goto(`${origin}/settings/models`));
    // Wait for the real settings read before leaving; the header alone can render
    // while GET /api/local/model-settings is still in flight and would be aborted.
    await page.getByRole('table').first().getByText('合成连接（改）').waitFor();
    await page.getByRole('navigation', { name: '一级导航' }).getByRole('link', { name: /语料/ }).click();
    await page.waitForURL(url => url.pathname === '/corpus/evidence');
    await page.getByRole('navigation', { name: '一级导航' }).waitFor();
    await shot('corpus-from-react-header');
  });

  await step('页面没有脚本错误；失败请求只有有意拦截的一次', async () => {
    assert.deepEqual(report.pageErrors, []);
    assert.deepEqual(report.consoleErrors.filter(text => !text.includes('Failed to load resource')), []);
    const unexpected = report.failedRequests.filter(entry => !entry.startsWith('POST /api/local/model-settings/probes'));
    assert.deepEqual(unexpected, []);
  });
  finished = true;
} finally {
  report.finished = finished;
  await writeFile(path.join(directory, 'report.json'), JSON.stringify(report, null, 2));
  await browser.close();
}
console.log(`WEB-REACT-001 settings real-chain proof: ${report.steps.length} steps passed`);
