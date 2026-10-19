// WEB-REACT-001 阶段 D1 真实链路证明：本机 Axum 路由 + 隔离 PostgreSQL + 已构建的 React 前端 + 无界面浏览器。
// 由 apps/api 的 `creators_page_real_chain_browser_proof`（scripts/test-web-app-postgres.sh）启动。
// 只访问 fixture 给出的本机地址，其余请求一律拦截；只用合成作者、作品与合成凭据，不调用模型、不访问平台。
import assert from 'node:assert/strict';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { chromium } from 'playwright';

const directory = process.env.WEB_APP_PROOF_DIR;
assert.ok(directory, 'WEB_APP_PROOF_DIR is required');
const fixture = JSON.parse(await readFile(path.join(directory, 'fixture.json'), 'utf8'));
const { origin, domainOne, domainTwo, authorTotal } = fixture;
await mkdir(directory, { recursive: true });

const report = { origin, steps: [], screenshots: [], pageErrors: [], consoleErrors: [], failedRequests: [], httpErrors: [], writes: [], storage: null };
const responseReads = [];
const UUID = /[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/i;
const listUrl = `${origin}/corpus/creators?domain=${domainOne}&pageSize=25`;
const step = async (name, run) => {
  try {
    await run();
    report.steps.push({ name, result: 'PASS' });
  } catch (error) {
    report.steps.push({ name, result: 'FAIL', error: String(error?.stack ?? error) });
    await page.screenshot({ path: path.join(directory, `failure-step-${report.steps.length}.png`), fullPage: true }).catch(() => {});
    report.failureText = await page.locator('body').innerText().catch(() => 'Page unavailable');
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
page.on('requestfailed', request => report.failedRequests.push(`${request.method()} ${new URL(request.url()).pathname} ${request.failure()?.errorText ?? ''}`));
page.on('response', response => {
  if (response.status() < 400) return;
  responseReads.push((async () => {
    const request = response.request();
    report.httpErrors.push({ method: request.method(), url: request.url(), status: response.status(), at: report.steps.length, body: (await response.text().catch(() => 'Body unavailable')).slice(0, 4096) });
  })());
});
// 记录所有非 GET 请求：打开页面、筛选、翻页、打开抽屉都不应该产生写请求。
page.on('request', request => {
  if (request.method() !== 'GET') {
    report.writes.push({ method: request.method(), path: new URL(request.url()).pathname, body: request.postData() ?? null, at: report.steps.length });
  }
});

const shot = async name => {
  await page.screenshot({ path: path.join(directory, `${name}.png`) });
  const layout = await page.evaluate(() => ({ viewport: innerWidth, documentWidth: document.documentElement.scrollWidth }));
  report.screenshots.push({ name, ...layout });
  return layout;
};
const rows = () => page.locator('tbody tr');
const rowOf = name => page.getByRole('row').filter({ has: page.getByRole('button', { name, exact: true }) });
const openList = async (url = listUrl) => {
  await page.goto(url);
  await page.getByRole('table').waitFor();
};
const resultCount = text => page.getByText(text, { exact: true }).first().waitFor();
const drawer = name => page.getByRole('dialog', { name, exact: true });
// 抽屉与对话框是模态的：表格被置为惰性，读表格内容时直接读 DOM，不走无障碍树。
const rowText = name =>
  page.evaluate(label => {
    const button = [...document.querySelectorAll('tbody button')].find(node => node.textContent === label);
    return button?.closest('tr')?.innerText ?? null;
  }, name);

let finished = false;
try {
  await step('打开页面：八列表格、命中数、分页、读取时间；外壳与旧页面同源', async () => {
    const response = await page.goto(listUrl);
    assert.equal(response?.status(), 200);
    await page.getByRole('table').waitFor();
    assert.equal(await page.title(), '创作者 · 语料');
    assert.deepEqual(
      await page.getByRole('columnheader').allTextContents(),
      ['创作者', '内容方向', '作者倾向', '作品表达', '相关作品', '相关最高赞', '作品预览', '观察状态'],
    );
    await resultCount(`${authorTotal} 位创作者`);
    await page.getByText('1 / 2 页', { exact: true }).waitFor();
    assert.equal(await rows().count(), 25);
    assert.match(await page.locator('.v7-context-meta').innerText(), /^读取于 \d{4}-\d{2}-\d{2} \d{2}:\d{2}$/);
    // 一级导航六项；语料是当前项；采集项的状态词来自语料外壳接口（库里已有一个观察目标 → 观察中）。
    const nav = page.getByRole('navigation', { name: '一级导航' });
    assert.equal(await nav.locator('a, button').count(), 6);
    assert.equal(await nav.getByRole('link', { name: /语料/ }).getAttribute('aria-current'), 'page');
    assert.match(await nav.getByRole('link', { name: /采集/ }).innerText(), /观察中/);
    const headerBorder = await page.locator('.v7-global-row').evaluate(node => getComputedStyle(node).borderBottomWidth);
    assert.equal(headerBorder, '2px');
    // 侧栏三项都带着领域；创作者是当前项。
    const side = page.getByRole('complementary', { name: '语料导航' });
    assert.deepEqual(
      await side.getByRole('link').evaluateAll(links => links.map(link => link.getAttribute('href'))),
      [`/corpus/evidence?domain=${domainOne}`, `/corpus/comments?domain=${domainOne}`, `/corpus/creators?domain=${domainOne}`],
    );
    assert.equal(await side.getByRole('link', { name: /创作者/ }).getAttribute('aria-current'), 'page');
    // 面包屑里的领域选择器取自服务器接口。
    assert.match(await page.getByLabel('切换当前观察领域').innerText(), /合成领域一/);
    // 普通层不露 UUID。
    assert.doesNotMatch(await page.locator('main').innerText(), UUID);
    // 没有任何用户动作之前，没有写请求。
    assert.deepEqual(report.writes, []);
    const layout = await shot('creators-1440-list');
    assert.ok(layout.documentWidth <= layout.viewport, '1440 宽度不应横向溢出');
  });

  await step('领域外壳接口：领域列表、解析后的领域、采集状态词都以服务器为准', async () => {
    const shell = await page.evaluate(async domain => (await fetch(`/api/local/corpus/shell?domain=${domain}`)).json(), domainOne);
    assert.equal(shell.selectedDomain, domainOne);
    assert.equal(shell.collectionState, '观察中');
    // 完整迁移会预置几个默认领域；这里只核对本次播种的两个。
    const mine = name => shell.domains.find(item => item.name === name);
    assert.ok(shell.domains.length >= 2);
    assert.equal(mine('合成领域一').domainRef, domainOne);
    assert.equal(mine('合成领域二').domainRef, domainTwo);
    assert.equal(mine('合成领域一').sampleCount, authorTotal);
    assert.equal(mine('合成领域二').sampleCount, 2);
    assert.equal(mine('合成领域一').status, 'active');
    const none = await page.evaluate(async () => (await fetch('/api/local/corpus/shell?domain=not-a-domain')).json());
    assert.equal(none.selectedDomain, null);
    const foreign = await page.evaluate(async () => {
      const response = await fetch('/api/local/corpus/shell', { method: 'POST' });
      await response.text();
      return response.status;
    });
    assert.equal(foreign, 405);
    report.writes.length = 0; // 上面这次是有意发的 POST，不算页面产生的写请求。
  });

  await step('翻页与每页数量：地址里的页码从 0 起，翻到第二页只有 2 位', async () => {
    await page.getByRole('button', { name: '下一页' }).click();
    await page.waitForURL(url => url.searchParams.get('page') === '1');
    await page.getByText('2 / 2 页', { exact: true }).waitFor();
    await page.waitForFunction(() => document.querySelectorAll('tbody tr').length === 2);
    await page.getByRole('button', { name: '上一页' }).click();
    await page.waitForFunction(() => document.querySelectorAll('tbody tr').length === 25);
    // 页码输入越界：就地提示，不翻页。
    await page.getByRole('spinbutton', { name: '跳转页码' }).fill('9');
    await page.getByRole('button', { name: '跳转' }).click();
    await page.getByText('请输入 1 至 2 页。').waitFor();
    // 地址里手写一个越界页码：回到最后一页并替换历史。
    await page.goto(`${origin}/corpus/creators?domain=${domainOne}&pageSize=25&page=9`);
    await page.waitForURL(url => url.searchParams.get('page') === '1');
    await page.getByText('2 / 2 页', { exact: true }).waitFor();
  });

  await step('搜索（防抖）、筛选草稿、条件芯片、重置、快捷视图', async () => {
    await openList();
    const search = page.getByRole('searchbox', { name: '搜索创作者或作品' });
    await search.fill('合成作者 03');
    await resultCount('1 位创作者');
    assert.equal(new URL(page.url()).searchParams.get('query'), '合成作者 03');
    await page.getByRole('button', { name: '清空搜索' }).click();
    await resultCount(`${authorTotal} 位创作者`);
    // 筛选对话框：取消不生效。
    await page.getByRole('button', { name: /^筛选/ }).click();
    let dialog = page.getByRole('dialog', { name: '筛选材料与作者' });
    await dialog.getByRole('checkbox', { name: '亲历自述' }).check();
    await dialog.getByRole('button', { name: '取消' }).click();
    await dialog.waitFor({ state: 'detached' });
    assert.equal(new URL(page.url()).searchParams.get('traits'), null);
    // 应用：真实数据里还没有任何分析结果，所以「亲历自述」命中 0 位——如实显示零命中，不是错误。
    await page.getByRole('button', { name: /^筛选/ }).click();
    dialog = page.getByRole('dialog', { name: '筛选材料与作者' });
    await dialog.getByRole('checkbox', { name: '亲历自述' }).check();
    await dialog.getByRole('button', { name: '应用筛选' }).click();
    await page.getByText('当前条件下没有找到创作者。').waitFor();
    await resultCount('0 位创作者');
    await page.getByText('0 页', { exact: true }).waitFor();
    assert.equal(await page.getByRole('button', { name: '下一页' }).isDisabled(), true);
    assert.equal(await page.getByRole('button', { name: '移除条件：亲历自述' }).count(), 1);
    // 「清除筛选」只清列表条件，领域与每页数量保持。
    await page.getByRole('button', { name: '清除筛选' }).click();
    await resultCount(`${authorTotal} 位创作者`);
    const params = new URL(page.url()).searchParams;
    assert.equal(params.get('domain'), domainOne);
    assert.equal(params.get('pageSize'), '25');
    assert.equal(params.get('traits'), null);
    // 快捷视图：范围统计（未加入观察 26）与点击后的结果一致。
    await page.getByRole('button', { name: '未加入观察 26' }).click();
    await resultCount('26 位创作者');
    assert.equal(await page.getByRole('button', { name: '未加入观察 26' }).getAttribute('aria-pressed'), 'true');
    await page.getByRole('button', { name: /^全部创作者/ }).click();
    await resultCount(`${authorTotal} 位创作者`);
    // 材料范围对话框：统计与判断标准常驻可见。
    await page.getByRole('button', { name: '材料范围' }).click();
    const scope = page.getByRole('dialog', { name: '材料范围与判断标准' });
    assert.match(await scope.innerText(), /至少两篇独立相关作品/);
    assert.match(await scope.innerText(), /已分析，依据不足/);
    await shot('creators-1440-scope');
    await page.keyboard.press('Escape');
    await scope.waitFor({ state: 'detached' });
    assert.deepEqual(report.writes, [], '读取、筛选、翻页、打开对话框都不产生写请求');
  });

  await step('依据抽屉：结论在先、按作品分组、门槛可见；Esc 关闭后焦点回到作者名', async () => {
    await openList();
    await page.getByRole('button', { name: '合成作者 03', exact: true }).click();
    const panel = drawer('合成作者 03');
    await panel.waitFor();
    const overview = await panel.getByRole('tabpanel', { name: '概览' }).innerText();
    assert.ok(overview.indexOf('作者内容倾向') < overview.indexOf('内容方向与作品表达'));
    assert.match(overview, /尚不能判定/);
    assert.match(overview, /自动分析未开启/);
    assert.match(overview, /本次实际取样篇数未记录。/);
    assert.match(overview, /持续创作倾向须至少两篇独立相关作品/);
    assert.doesNotMatch(await panel.innerText(), UUID);
    await panel.getByRole('tab', { name: '作品与依据' }).click();
    const group = panel.locator('details[data-work]').first();
    await group.locator('summary').click();
    assert.equal(await group.getByText('判定门槛：').count(), 4);
    // 还没有任何分析结果：没有被引用的片段，如实说明，而不是显示一个空的依据。
    assert.match(await group.innerText(), /暂无可回查的支持片段。/);
    await panel.getByRole('tab', { name: '作者资料' }).click();
    assert.match(await panel.getByRole('tabpanel', { name: '作者资料' }).innerText(), /合成公开简介|公开简介尚未取得/);
    await panel.getByRole('tab', { name: '概览' }).click();
    await shot('creators-1440-drawer');
    await page.keyboard.press('Escape');
    await panel.waitFor({ state: 'detached' });
    assert.equal(await page.evaluate(() => document.activeElement?.textContent), '合成作者 03');
    assert.deepEqual(report.writes, [], '打开抽屉不产生写请求');
  });

  await step('真实写入：调整判断（补充「与当前领域的关系」）→ 列表重读，抽屉保持', async () => {
    await page.getByRole('button', { name: '合成作者 03', exact: true }).click();
    const panel = drawer('合成作者 03');
    await panel.waitFor();
    await panel.getByRole('tab', { name: '作品与依据' }).click();
    const group = panel.locator('details[data-work]').first();
    await group.locator('summary').click();
    await group.getByRole('button', { name: '补充判断' }).first().click();
    const dialog = page.getByRole('dialog', { name: '补充与当前领域的关系' });
    await dialog.waitFor();
    // 正向判断必须选当前支持材料。
    await dialog.getByRole('combobox', { name: '你的判断' }).selectOption('related');
    await dialog.getByRole('button', { name: '保存判断' }).click();
    await dialog.getByText('请选择至少一项当前材料作为支持依据。').waitFor();
    assert.equal(report.writes.length, 0, '未选支持材料时不发请求');
    await dialog.getByRole('checkbox').first().check();
    await dialog.getByRole('textbox', { name: /调整原因/ }).fill('合成证明：标题与领域一致');
    await shot('creators-1440-correction');
    await dialog.getByRole('button', { name: '保存判断' }).click();
    await dialog.waitFor({ state: 'detached' });
    await page.getByText(/判断已保存；自动分析不会覆盖这一项/).waitFor();
    assert.equal(report.writes.length, 1);
    assert.equal(report.writes[0].method, 'PUT');
    assert.equal(report.writes[0].path, '/api/local/creator-discovery-overrides');
    const body = JSON.parse(report.writes[0].body);
    assert.equal(body.field, 'relevance');
    assert.equal(body.value, 'related');
    assert.equal(body.workRef, fixture.correctedWork);
    assert.ok(body.supportFragmentIds.length >= 1);
    // 抽屉仍然打开；这项判断现在是人工调整，引用了刚才选的那段材料，并能回查原作品。
    await panel.waitFor();
    await group.getByText(/^支持依据 · \d+ 个片段$/).click();
    const quote = group.locator('blockquote').first();
    await quote.waitFor();
    assert.match(await quote.innerText(), /合成/);
    assert.equal(await quote.getByRole('link', { name: '回查原作品' }).count(), 1);
    // 列表重读后，这位作者的「相关作品」变成 1。
    await page.waitForFunction(() => {
      const button = [...document.querySelectorAll('tbody button')].find(node => node.textContent === '合成作者 03');
      return button?.closest('tr')?.querySelectorAll('td')[4]?.innerText.startsWith('1');
    });
    assert.match(await panel.innerText(), /人工调整 · \d{4}-\d{2}-\d{2} \d{2}:\d{2}/);
    await page.keyboard.press('Escape');
    await panel.waitFor({ state: 'detached' });
  });

  await step('真实写入：发现设置（点赞标准，带设置版本号）→ 爆款作者数变化', async () => {
    await page.getByRole('button', { name: '发现设置' }).click();
    const dialog = page.getByRole('dialog', { name: '发现设置' });
    await dialog.waitFor();
    await dialog.getByRole('link', { name: '去 AI 设置' }).waitFor();
    await dialog.getByRole('spinbutton', { name: /最低点赞/ }).fill('100');
    await dialog.getByRole('button', { name: '保存设置' }).click();
    await dialog.waitFor({ state: 'detached' });
    await page.getByText(/发现设置已保存。保存本身不启动采集，也不会立刻调用模型/).waitFor();
    const put = report.writes.at(-1);
    assert.equal(put.method, 'PUT');
    assert.equal(put.path, '/api/local/creator-discovery-policy');
    assert.deepEqual(JSON.parse(put.body), { domain: domainOne, revision: 0, likeThreshold: 100 });
    // 合成作者 03 的相关作品有 300 个赞 ≥ 100，且尚未加入观察。
    await page.getByRole('button', { name: '未观察的爆款作者 1' }).waitFor();
    // 第二次打开带着新的设置版本号。
    await page.getByRole('button', { name: '发现设置' }).click();
    const again = page.getByRole('dialog', { name: '发现设置' });
    await again.waitFor();
    assert.equal(await again.getByRole('spinbutton', { name: /最低点赞/ }).inputValue(), '100');
    await again.getByRole('button', { name: '取消' }).click();
    await again.waitFor({ state: 'detached' });
  });

  await step('加入观察：先确认，确认后才发请求，回执后才显示结果', async () => {
    const before = report.writes.length;
    await rowOf('合成作者 05').getByRole('button', { name: '加入观察' }).click();
    const dialog = page.getByRole('dialog', { name: '加入观察目标' });
    await dialog.waitFor();
    assert.match(await dialog.innerText(), /这一步不会：[\s\S]*立即采集作品、评论或主页[\s\S]*开启或关闭巡查[\s\S]*调用模型/);
    await shot('creators-1440-join-confirm');
    assert.equal(report.writes.length, before, '确认之前不发请求');
    await dialog.getByRole('button', { name: '确认加入' }).click();
    await dialog.getByText('已完成', { exact: true }).waitFor();
    assert.match(await dialog.innerText(), /已把「合成作者 05」加入「合成领域一」的观察名单。已新登记这位作者。巡查尚未开启。/);
    assert.equal(report.writes.length, before + 1);
    const post = report.writes.at(-1);
    assert.equal(post.method, 'POST');
    assert.match(post.path, /\/observation-target$/);
    assert.deepEqual(JSON.parse(post.body), { domain: domainOne, usageRole: 'primary' });
    await dialog.getByRole('button', { name: '关闭', exact: true }).click();
    await dialog.waitFor({ state: 'detached' });
    await rowOf('合成作者 05').getByRole('link', { name: '查看目标' }).waitFor();
    assert.match(await rowText('合成作者 05'), /已加入 · 巡查未开启/);
  });

  await step('加入观察：请求没有得到答复时显示「未确认」，重试发送同一个请求体', async () => {
    let aborted = false;
    await page.route('**/api/local/creators/*/observation-target', async route => {
      if (!aborted) {
        aborted = true;
        await route.abort('connectionreset');
      } else {
        await route.continue();
      }
    });
    const before = report.writes.length;
    await rowOf('合成作者 06').getByRole('button', { name: '加入观察' }).click();
    const dialog = page.getByRole('dialog', { name: '加入观察目标' });
    await dialog.getByRole('button', { name: '确认加入' }).click();
    await dialog.getByText('未确认', { exact: true }).waitFor();
    assert.match(await dialog.innerText(), /结果未确认，可能已经生效/);
    assert.equal(await dialog.getByText('已完成').count(), 0);
    await dialog.getByRole('button', { name: '重试（同一请求）' }).click();
    await dialog.getByText('已完成', { exact: true }).waitFor();
    await page.unroute('**/api/local/creators/*/observation-target');
    const attempts = report.writes.slice(before);
    assert.equal(attempts.length, 2);
    assert.equal(attempts[0].path, attempts[1].path);
    assert.equal(attempts[0].body, attempts[1].body);
    await dialog.getByRole('button', { name: '关闭', exact: true }).click();
    await dialog.waitFor({ state: 'detached' });
    await rowOf('合成作者 06').getByRole('link', { name: '查看目标' }).waitFor();
  });

  await step('切换领域仍停在创作者页并只带 domain；后退回到上一个领域', async () => {
    await page.getByLabel('切换当前观察领域').click();
    await page.getByRole('navigation', { name: '可选观察领域' }).getByRole('link', { name: /合成领域二/ }).click();
    await page.waitForURL(`${origin}/corpus/creators?domain=${domainTwo}`);
    await page.waitForFunction(() => document.querySelectorAll('tbody tr').length === 2);
    assert.equal(new URL(page.url()).pathname, '/corpus/creators');
    // 默认按「最近收录」排序，先后顺序不是这里要证明的；只证明两位作者都在、且只有这两位。
    assert.deepEqual((await page.getByRole('button', { name: /^第二领域作者/ }).allTextContents()).sort(), ['第二领域作者 1', '第二领域作者 2']);
    assert.equal(await page.getByRole('button', { name: /^合成作者 0/ }).count(), 0, '上一个领域的数据不会留在新领域页面上');
    await resultCount('2 位创作者');
    const side = page.getByRole('complementary', { name: '语料导航' });
    assert.equal(await side.getByRole('link', { name: /证据库/ }).getAttribute('href'), `/corpus/evidence?domain=${domainTwo}`);
    await shot('creators-1440-domain-two');
    await page.goBack();
    await page.waitForURL(url => url.searchParams.get('domain') === domainOne);
    await resultCount(`${authorTotal} 位创作者`);
  });

  await step('侧栏进入证据库再回创作者页；从作者数量链接去证据库再返回，位置恢复', async () => {
    const side = page.getByRole('complementary', { name: '语料导航' });
    await side.getByRole('link', { name: /证据库/ }).click();
    await page.waitForURL(url => url.pathname === '/corpus/evidence' && url.searchParams.get('domain') === domainOne);
    await page.getByRole('searchbox', { name: '检索作品与证据' }).waitFor();
    await shot('evidence-from-react-side-nav');
    await page.getByRole('complementary', { name: '语料导航' }).getByRole('link', { name: /创作者/ }).click();
    await page.waitForURL(url => url.pathname === '/corpus/creators' && url.searchParams.get('domain') === domainOne);
    await page.getByRole('table').waitFor();
    // 去证据库看合成作者 03 的相关作品（人工判为相关的那一篇），再用证据库的「返回创作者」回来。
    const url = `${origin}/corpus/creators?domain=${domainOne}&pageSize=25`;
    await openList(url);
    const link = rowOf('合成作者 03').getByRole('link', { name: '1', exact: true });
    await link.click();
    await page.waitForURL(target => target.pathname === '/corpus/evidence');
    await page.getByText('当前为创作者作品筛选集合').waitFor();
    await page.getByRole('link', { name: '返回创作者' }).click();
    await page.waitForURL(target => target.pathname === '/corpus/creators' && target.searchParams.get('domain') === domainOne);
    await page.getByRole('table').waitFor();
    await page.waitForFunction(() => document.activeElement?.textContent === '合成作者 03');
    await shot('creators-1440-after-evidence-return');
  });

  await step('存储只记位置不记内容：localStorage 为空；创作者页自己的 sessionStorage 记录只有位置', async () => {
    report.storage = await page.evaluate(() => ({
      local: Object.keys(localStorage),
      session: Object.keys(sessionStorage),
      returnValue: sessionStorage.getItem('linggan.creatorDiscovery.return'),
    }));
    assert.deepEqual(report.storage.local, []);
    // 上一步去过证据库页，它自己可能留下一条 collection-selected:<领域> 的选择记录，不属于本页；
    // 属于创作者页的只有 linggan.creatorDiscovery.return，而且里面只能是位置，不能有作者名或作品内容。
    for (const key of report.storage.session) assert.match(key, /^(linggan\.creatorDiscovery\.return|collection-selected:[0-9a-f-]{36})$/);
    if (report.storage.returnValue !== null) assert.doesNotMatch(report.storage.returnValue, /合成/);
  });

  for (const [width, height] of [[1440, 900], [1280, 720], [1024, 768], [390, 844]]) {
    await step(`${width}×${height}：列表与抽屉无页面级横向溢出，抽屉在视口内`, async () => {
      await page.setViewportSize({ width, height });
      await openList();
      await resultCount(`${authorTotal} 位创作者`);
      const list = await shot(`creators-${width}-list`);
      assert.ok(list.documentWidth <= list.viewport, `${width} 宽度的列表不应横向溢出`);
      assert.ok(await page.getByRole('button', { name: '下一页' }).isVisible(), '分页在视口内可见');
      await page.getByRole('button', { name: '合成作者 27', exact: true }).click();
      const panel = drawer('合成作者 27');
      await panel.waitFor();
      const box = await panel.boundingBox();
      assert.ok(box && box.x >= -1 && box.x + box.width <= width + 1, '抽屉应在视口内');
      assert.ok(box && box.y + box.height <= height + 1, '抽屉底部（提交栏）应在视口内');
      assert.ok(await panel.getByRole('button', { name: '加入本领域观察' }).isVisible(), '提交栏的动作始终可见');
      const opened = await shot(`creators-${width}-drawer`);
      assert.ok(opened.documentWidth <= opened.viewport, `${width} 宽度打开抽屉后不应横向溢出`);
      await page.keyboard.press('Escape');
      await panel.waitFor({ state: 'detached' });
    });
  }

  await step('页面没有脚本错误；失败请求只有有意拦截的一次', async () => {
    await Promise.allSettled(responseReads);
    const expectedMethodRejection = entry => entry.method === 'POST' && new URL(entry.url).pathname === '/api/local/corpus/shell' && entry.status === 405;
    assert.deepEqual(report.httpErrors.filter(entry => !expectedMethodRejection(entry)), [], 'unexpected HTTP errors must not be hidden by later successful reads');
    assert.deepEqual(report.pageErrors, []);
    assert.deepEqual(report.consoleErrors.filter(text => !text.includes('Failed to load resource')), []);
    // 允许的两类：有意拦截的那一次「加入观察」；以及前端因范围变化主动取消的读取（net::ERR_ABORTED，这是设计行为）。
    const intended = entry => entry.startsWith('POST /api/local/creators/') && entry.includes('/observation-target');
    const cancelledRead = entry => entry.startsWith('GET ') && entry.endsWith('net::ERR_ABORTED');
    assert.deepEqual(report.failedRequests.filter(entry => !intended(entry) && !cancelledRead(entry)), []);
    assert.equal(report.failedRequests.filter(intended).length, 1);
  });
  finished = true;
} finally {
  await Promise.allSettled(responseReads);
  report.finished = finished;
  await writeFile(path.join(directory, 'report.json'), JSON.stringify(report, null, 2));
  await browser.close();
}
console.log(`WEB-REACT-001 D1 creators real-chain proof: ${report.steps.length} steps passed`);
