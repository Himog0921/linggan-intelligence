// WEB-REACT-001 阶段 D2 真实链路证明：本机 Axum 路由 + 隔离 PostgreSQL + 已构建的 React 前端 + 无界面浏览器。
// 由 apps/api 的 `evidence_page_real_chain_browser_proof`（scripts/test-web-app-postgres.sh）启动。
// 只访问 fixture 给出的本机地址，其余请求一律拦截；只用合成作品、合成评论与合成媒体，不调用模型、不访问平台。
import assert from 'node:assert/strict';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { chromium } from 'playwright';

const directory = process.env.WEB_APP_PROOF_DIR;
assert.ok(directory, 'WEB_APP_PROOF_DIR is required');
const fixture = JSON.parse(await readFile(path.join(directory, 'fixture.json'), 'utf8'));
const { origin, domainOne, domainTwo, rich, video, risk, partial, cleaned, restricted, reobserve, offPage, plain, collectionRun, totalWorks, pageSize } = fixture;
await mkdir(directory, { recursive: true });

const report = {
  origin,
  steps: [],
  screenshots: [],
  pageErrors: [],
  consoleErrors: [],
  failedRequests: [],
  blockedRequests: [],
  requests: [],
  writes: [],
  storage: null,
  sources: {},
};
const UUID = /[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/i;
const base = `${origin}/corpus/evidence?domain=${domainOne}`;
const LIST = '/api/local/work-resources';
const step = async (name, run) => {
  try {
    await run();
    report.steps.push({ name, result: 'PASS' });
  } catch (error) {
    // 失败现场：截图＋当时打开着的对话框文字（合成数据），供本机与 CI 产物直接查看，不必重跑猜原因。
    const failure = { name, result: 'FAIL', error: String(error?.stack ?? error) };
    try {
      await page.screenshot({ path: path.join(directory, `failure-step-${report.steps.length + 1}.png`) });
      failure.openDialogs = await page.evaluate(() =>
        [...document.querySelectorAll('dialog[open], [role="dialog"]')].map(node => (node.textContent ?? '').replace(/\s+/g, ' ').slice(0, 800)),
      );
    } catch {
      // 页面已经关闭时只记录错误本身。
    }
    report.steps.push(failure);
    throw error;
  }
};

const browser = await chromium.launch({ headless: true });
const context = await browser.newContext({ viewport: { width: 1440, height: 900 }, locale: 'zh-CN', timezoneId: 'Asia/Shanghai', acceptDownloads: true });
// 只放行本机源；其余一律拦截并记账（本页不得有任何外域请求，包括平台图片地址）。
await context.route('**/*', async route => {
  const url = new URL(route.request().url());
  if (url.origin === origin) {
    // 在可控的慢链路下运行，防止只等旧列表或检查器标题就读取后续详情分区。
    if (url.pathname.startsWith('/api/local/')) await new Promise(resolve => setTimeout(resolve, 500));
    return route.continue();
  }
  report.blockedRequests.push(url.href);
  return route.abort();
});
const page = await context.newPage();
await (await context.newCDPSession(page)).send('Emulation.setCPUThrottlingRate', { rate: 6 });
page.on('pageerror', error => report.pageErrors.push(error.message));
page.on('console', message => {
  if (message.type() === 'error') report.consoleErrors.push(message.text());
});
page.on('requestfailed', request => report.failedRequests.push(`${request.method()} ${new URL(request.url()).pathname} ${request.failure()?.errorText ?? ''}`));
page.on('request', request => {
  const url = new URL(request.url());
  report.requests.push({ method: request.method(), path: url.pathname, search: url.search, at: report.steps.length });
});
// 所有非 GET：记下方法、路径、状态与（重新分层的）回执。打开、筛选、切换、读取都不应该产生写请求。
page.on('response', async response => {
  const request = response.request();
  if (request.method() === 'GET') return;
  const entry = { method: request.method(), path: new URL(request.url()).pathname, status: response.status(), at: report.steps.length, updated: null };
  if (entry.path.endsWith('/relayer')) {
    try {
      entry.updated = (await response.json()).updated ?? null;
    } catch {
      /* 非 JSON 回答就没有更新数。 */
    }
  }
  report.writes.push(entry);
});

const shot = async name => {
  await page.screenshot({ path: path.join(directory, `${name}.png`) });
  const layout = await page.evaluate(() => ({ viewport: innerWidth, documentWidth: document.documentElement.scrollWidth }));
  report.screenshots.push({ name, ...layout });
  return layout;
};
// 写请求的记账在响应事件里（重新分层还要读回执），比页面上的结果晚一拍：先等它记完再核对数量。
const writesTotal = async expected => {
  for (let attempt = 0; attempt < 50 && report.writes.length < expected; attempt += 1) await page.waitForTimeout(100);
  assert.equal(report.writes.length, expected);
};
const requestsTo = (method, pattern) => report.requests.filter(request => request.method === method && pattern.test(request.path));
const listReads = () => requestsTo('GET', /^\/api\/local\/work-resources$/).length;
const rows = () => page.locator('li[data-work-ref], tbody tr[data-work-ref]');
const rowOf = title => page.locator('[data-work-ref]').filter({ has: page.getByRole('button', { name: title, exact: true }) });
const inspector = () => page.locator('aside[data-width]');
const panel = name => page.getByRole('tabpanel', { name });
const search = () => page.getByRole('searchbox', { name: '检索作品与证据' });
const open = async (extra = '', url = base) => {
  await page.goto(`${url}${extra}`);
  await rows().first().waitFor();
};
const waitInspector = async title => {
  await page.waitForFunction(text => document.querySelector('aside[data-width] h2')?.textContent === text, title);
};
const TITLE = { rich: '合成长文：从两分钟开始建立习惯', video: '合成视频作品：三十秒示范', risk: '合成作品 风险控制停止', partial: '合成作品 部分取得', cleaned: '合成作品 字节已清理', restricted: '合成作品 撤回或受限', reobserve: '合成作品 可复观测', offPage: '合成作品 很久之前', plain: '合成作品 00' };

let finished = false;
try {
  // ① 打开与外壳
  await step('打开页面：外壳、标题、读取时间、侧栏徽标；自动选中第一项并替换地址；没有写请求；普通层无 UUID', async () => {
    const response = await page.goto(base);
    assert.equal(response?.status(), 200);
    await rows().first().waitFor();
    assert.equal(await page.title(), '证据库 · 语料');
    assert.equal((await page.getByRole('heading', { level: 1 }).textContent()).trim(), '证据库');
    // 地址：第一项自动选中，写回 work（替换历史，不新增）。
    await page.waitForURL(url => url.searchParams.get('work') === rich);
    assert.equal(await rows().count(), pageSize);
    await waitInspector(TITLE.rich);
    const nav = page.getByRole('navigation', { name: '一级导航' });
    assert.equal(await nav.locator('a, button').count(), 6);
    assert.equal(await nav.getByRole('link', { name: /语料/ }).getAttribute('aria-current'), 'page');
    const side = page.getByRole('complementary', { name: '语料导航' });
    assert.deepEqual(
      await side.getByRole('link').evaluateAll(links => links.map(link => link.getAttribute('href'))),
      [`/corpus/evidence?domain=${domainOne}`, `/corpus/comments?domain=${domainOne}`, `/corpus/creators?domain=${domainOne}`],
    );
    assert.equal(await side.getByRole('link', { name: /证据库/ }).getAttribute('aria-current'), 'page');
    assert.match(await page.getByLabel('切换当前观察领域').innerText(), /合成领域一/);
    const readout = page.getByText(/^读取于 \d{4}-\d{2}-\d{2} \d{2}:\d{2}$/);
    await readout.waitFor();
    assert.match(await readout.innerText(), /^读取于 \d{4}-\d{2}-\d{2} \d{2}:\d{2}$/);
    // 侧栏徽标：本次读取的作品数，悬停说明里是三个读数；读模型不给总数，页面不编造「共 N 个」。
    assert.equal((await page.locator('#ev-rail-count').innerText()).trim(), String(pageSize));
    assert.match(await page.locator('#ev-rail-count').getAttribute('title'), /^本次读取 50 个作品 · 已留存评论 \d+ · 本地材料 \d+ · 部分或缺口 \d+$/);
    assert.match(await page.locator('.v7-side-foot').innerText(), /只读本机材料投影/);
    // 第一行的封面是真的受控本机图片（不是平台地址）。
    await page.waitForFunction(() => {
      const image = document.querySelector('li[data-work-ref] img');
      return image instanceof HTMLImageElement && image.complete && image.naturalWidth > 0;
    });
    assert.match(await page.locator('li[data-work-ref] img').first().getAttribute('src'), /^\/api\/local\/(media|derivative)\//);
    assert.doesNotMatch(await page.locator('main').innerText(), UUID);
    assert.deepEqual(report.writes, []);
    const layout = await shot('evidence-1440-research');
    assert.ok(layout.documentWidth <= layout.viewport, '1440 宽度不应横向溢出');
  });

  // ② 未选领域
  await step('未选领域：弹出领域选择，不读列表、不改地址；work 随选择保留；关闭只回到空态', async () => {
    const reads = listReads();
    await page.goto(`${origin}/corpus/evidence`);
    const dialog = page.getByRole('dialog', { name: '选择研究领域' });
    await dialog.waitFor();
    assert.ok((await dialog.getByRole('link').count()) >= 2);
    assert.match(await dialog.innerText(), /合成领域一/);
    await page.waitForTimeout(400);
    assert.equal(new URL(page.url()).search, '', '没有领域时地址不被改写');
    assert.equal(listReads(), reads, '没有领域不读列表');
    assert.equal(await search().isDisabled(), true);
    assert.equal(await page.getByRole('button', { name: '读取材料' }).isDisabled(), true);
    assert.equal(await search().getAttribute('title'), '请先选择研究领域，再读取统一作品材料。');
    await shot('evidence-1440-no-domain');
    await page.keyboard.press('Escape');
    await dialog.waitFor({ state: 'hidden' });
    await page.getByRole('heading', { name: '请选择研究领域' }).waitFor();
    assert.equal(listReads(), reads);
    // 带着作品进来：每个领域链接都带上它，选定后继续定位这件作品。
    await page.goto(`${origin}/corpus/evidence?work=${rich}`);
    const again = page.getByRole('dialog', { name: '选择研究领域' });
    await again.waitFor();
    for (const href of await again.getByRole('link').evaluateAll(links => links.map(link => link.getAttribute('href')))) {
      assert.match(href, new RegExp(`&work=${rich}$`));
    }
    await again.getByRole('link', { name: /合成领域一/ }).click();
    await page.waitForURL(url => url.searchParams.get('domain') === domainOne && url.searchParams.get('work') === rich);
    await waitInspector(TITLE.rich);
  });

  // ③ 三种排版、50 条＋继续读取、排版切换不重新请求、3:4 探针
  await step('50 条＋「继续读取作品」；三种排版互相切换不重新读取；封面是 3:4', async () => {
    await open();
    assert.equal(await rows().count(), pageSize);
    await page.getByRole('button', { name: '继续读取作品', exact: true }).click();
    await page.waitForFunction(total => document.querySelectorAll('li[data-work-ref]').length === total, totalWorks);
    assert.equal(await page.getByRole('button', { name: '继续读取作品', exact: true }).count(), 0, '游标用完，按钮消失');
    assert.equal((await page.locator('#ev-rail-count').innerText()).trim(), String(totalWorks));
    assert.equal(new URL(page.url()).searchParams.get('work'), rich, '续读保持当前选择');
    // 研读行的预览是 3:4。
    const research = await page.locator('li[data-work-ref] [data-media-state]').first().boundingBox();
    assert.ok(Math.abs(research.width / research.height - 0.75) < 0.03, `研读预览比例 ${research.width / research.height}`);
    const reads = listReads();
    await page.getByRole('button', { name: '表格', exact: true }).click();
    await page.getByRole('table').waitFor();
    assert.equal(await page.locator('tbody tr[data-work-ref]').count(), totalWorks);
    assert.deepEqual(await page.getByRole('columnheader').allTextContents(), ['作品', '作者', '材料', '互动：', '发布时间', '最近观察']);
    await shot('evidence-1440-table');
    await page.getByRole('button', { name: '封面', exact: true }).click();
    await page.locator('li[data-work-ref]').first().waitFor();
    assert.equal(await page.locator('li[data-work-ref]').count(), totalWorks);
    assert.equal(listReads(), reads, '切换排版不重新读取');
    assert.equal(new URL(page.url()).searchParams.get('layout'), 'cover');
    // 翻转卡的舞台（正面线稿与背面原图共用的 3:4 框）。
    const stage = await page.locator('li[data-work-ref] [data-unavailable]').first().boundingBox();
    assert.ok(Math.abs(stage.width / stage.height - 0.75) < 0.03, `封面舞台比例 ${stage.width / stage.height}`);
    assert.ok((await page.locator('li[data-work-ref]').first().boundingBox()).width < 400, '封面是多列栅格，不是一列通栏');
    await shot('evidence-1440-cover');
    // 后退：封面 → 表格 → 研读。
    await page.goBack();
    await page.getByRole('table').waitFor();
    await page.goBack();
    await page.locator('li[data-work-ref]').first().waitFor();
    assert.equal(await page.getByRole('table').count(), 0);
  });

  // ④ 检索、下拉、系统视图、后退恢复
  await step('检索只在提交时读取；四类筛选与系统视图得到对应作品；后退还原；清空筛选复位', async () => {
    await open();
    const reads = listReads();
    await search().fill('风险控制');
    await page.waitForTimeout(600);
    assert.equal(await rows().count(), pageSize, '输入过程中不自动搜索：结果没变');
    assert.equal(listReads(), reads, '输入过程中不读取');
    await search().press('Enter');
    await page.waitForURL(url => url.searchParams.get('q') === '风险控制');
    await page.waitForFunction(() => document.querySelectorAll('li[data-work-ref]').length === 1);
    assert.equal(await rowOf(TITLE.risk).count(), 1);
    assert.ok(listReads() > reads);
    await page.goBack();
    await page.waitForFunction(size => document.querySelectorAll('li[data-work-ref]').length === size, pageSize);
    assert.equal(await search().inputValue(), '', '后退时检索框跟着变');
    assert.equal(new URL(page.url()).searchParams.has('q'), false);
    // 筛选浮层：三个下拉＋徽标；通道状态=风险控制停止。
    await page.getByRole('button', { name: /^筛选/ }).click();
    const laneState = page.getByRole('combobox', { name: '通道状态' });
    await laneState.click();
    const laneStateOptions = page.getByRole('listbox', { name: '通道状态' });
    const riskControl = laneStateOptions.getByRole('option', { name: '风险控制停止', exact: true });
    assert.equal(await riskControl.getAttribute('aria-selected'), 'false');
    assert.equal(await riskControl.getAttribute('aria-disabled'), null);
    // 长菜单的目标项可能在当前可视段外；经键盘到达并选择，核对同一无障碍菜单路径。
    await laneState.press('End');
    await page.waitForFunction(() => {
      const trigger = document.querySelector('[role="combobox"][aria-label="通道状态"]');
      const active = trigger?.getAttribute('aria-activedescendant');
      return active !== null && document.getElementById(active)?.textContent?.includes('撤回或受限') === true;
    });
    await laneState.press('ArrowUp');
    await page.waitForFunction(() => {
      const trigger = document.querySelector('[role="combobox"][aria-label="通道状态"]');
      const active = trigger?.getAttribute('aria-activedescendant');
      return active !== null && document.getElementById(active)?.textContent?.includes('字节已清理') === true;
    });
    await laneState.press('ArrowUp');
    await page.waitForFunction(() => {
      const trigger = document.querySelector('[role="combobox"][aria-label="通道状态"]');
      const active = trigger?.getAttribute('aria-activedescendant');
      return active !== null && document.getElementById(active)?.textContent?.includes('风险控制停止') === true;
    });
    await laneState.press('Enter');
    await page.waitForURL(url => url.searchParams.get('laneState') === 'RISK_CONTROL');
    await page.waitForFunction(() => document.querySelector('[role="combobox"][aria-label="通道状态"]')?.textContent?.includes('风险控制停止') === true);
    assert.match(await laneState.innerText(), /风险控制停止/);
    await page.waitForFunction(() => document.querySelectorAll('li[data-work-ref]').length === 1);
    assert.equal(await rowOf(TITLE.risk).count(), 1);
    assert.equal(await page.getByLabel('已选 1 项筛选').count(), 1);
    await laneState.click();
    const allStates = page.getByRole('listbox', { name: '通道状态' }).getByRole('option', { name: '全部状态', exact: true });
    assert.equal(await allStates.getAttribute('aria-selected'), 'false');
    await allStates.click();
    await page.waitForURL(url => !url.searchParams.has('laneState'));
    await page.waitForFunction(() => document.querySelector('[role="combobox"][aria-label="通道状态"]')?.textContent?.includes('全部状态') === true);
    assert.match(await laneState.innerText(), /全部状态/);
    await page.waitForFunction(size => document.querySelectorAll('li[data-work-ref]').length === size, pageSize);
    const mediaKind = page.getByRole('combobox', { name: '媒体类型' });
    await mediaKind.click();
    const video = page.getByRole('listbox', { name: '媒体类型' }).getByRole('option', { name: '视频', exact: true });
    assert.equal(await video.getAttribute('aria-selected'), 'false');
    assert.equal(await video.getAttribute('aria-disabled'), null);
    await video.click();
    // 等筛选后的列表真的到了：地址带上媒体类型，且列表不再是未筛选时的整页（只等「至少一行」会被旧列表满足）。
    await page.waitForURL(url => url.searchParams.get('mediaKind') === 'video');
    await page.waitForFunction(() => document.querySelector('[role="combobox"][aria-label="媒体类型"]')?.textContent?.includes('视频') === true);
    assert.match(await mediaKind.innerText(), /视频/);
    await page.waitForFunction(size => {
      const count = document.querySelectorAll('li[data-work-ref]').length;
      return count >= 1 && count < size;
    }, pageSize);
    assert.equal(await rowOf(TITLE.video).count(), 1);
    await page.keyboard.press('Escape');
    // 系统视图：部分取得／风险停止／媒体已清理／撤回或受限；地址只写 view，接口收到展开后的条件。
    await open();
    for (const [name, key, title] of [['部分取得', 'partial', TITLE.partial], ['风险停止', 'risk', TITLE.risk], ['媒体已清理', 'cleaned', TITLE.cleaned], ['撤回或受限', 'restricted', TITLE.restricted]]) {
      await page.getByRole('button', { name, exact: true }).click();
      await page.waitForURL(url => url.searchParams.get('view') === key);
      // 新条件的结果替换旧结果（不拿旧列表占位）：等到列表变成子集，再核对内容。
      await page.waitForFunction(size => document.querySelectorAll('li[data-work-ref]').length < size, pageSize);
      await rowOf(title).waitFor();
      assert.equal(await page.getByRole('button', { name, exact: true }).getAttribute('aria-pressed'), 'true');
    }
    const lastList = requestsTo('GET', /^\/api\/local\/work-resources$/).at(-1);
    assert.match(lastList.search, /restriction=WITHDRAWN_OR_RESTRICTED/);
    // 回到全部材料：清空筛选（浮层里）也是一样的复位。
    await page.getByRole('button', { name: /^筛选/ }).click();
    await page.getByRole('button', { name: '清空筛选' }).click();
    await page.waitForFunction(size => document.querySelectorAll('li[data-work-ref]').length === size, pageSize);
    assert.equal(new URL(page.url()).searchParams.has('view'), false);
    await shot('evidence-1440-after-reset');
  });

  // ⑤ 选中→阅读主序、技术事实折叠、tab 深链
  await step('阅读主序：标题、作者、发布时间、互动、正文；技术事实默认折叠；tab 深链', async () => {
    await open(`&work=${rich}`);
    await waitInspector(TITLE.rich);
    const reading = panel('概览');
    // 标题来自列表，作者、互动与正文来自随后的详情读取：等正文到了再读（只等标题会读到空白的分区）。
    await reading.locator('p[data-collapsed]').waitFor();
    const text = await reading.innerText();
    assert.match(text, /合成作者 林/);
    for (const figure of ['12,345', '321', '4,567', '89']) assert.ok(text.includes(figure), figure);
    assert.ok(text.includes('第1段。'));
    // 长文折叠与展开：折叠只是不占满整屏，正文仍完整在页面里（裁切不删字）。
    const toggle = reading.getByRole('button', { name: '展开全文' });
    assert.equal(await toggle.getAttribute('aria-expanded'), 'false');
    const body = reading.locator('p[data-collapsed]');
    assert.equal(await body.getAttribute('data-collapsed'), 'true');
    const folded = await body.evaluate(node => ({ shown: node.clientHeight, full: node.scrollHeight }));
    assert.ok(folded.full > folded.shown, '折叠时正文被裁切');
    await toggle.click();
    assert.equal(await body.getAttribute('data-collapsed'), 'false');
    const unfolded = await body.evaluate(node => ({ shown: node.clientHeight, full: node.scrollHeight }));
    assert.ok(unfolded.shown > folded.shown && unfolded.shown >= unfolded.full - 1, '展开后完整显示');
    await page.getByText('合成长文结尾标记。').waitFor();
    // 技术事实：折叠；展开后才有稳定引用与技术码。
    const facts = reading.locator('details').filter({ has: page.locator('summary', { hasText: '技术事实' }) });
    assert.equal(await facts.evaluate(node => node.open), false);
    assert.doesNotMatch(await reading.innerText(), UUID);
    await facts.locator('summary').click();
    assert.ok((await facts.innerText()).includes(rich));
    await shot('evidence-1440-rich-overview');
    for (const [tab, marker] of [['evidence', '图片文字与转录原文'], ['materials', '材料处理台账'], ['trace', '来源与执行血缘']]) {
      await open(`&work=${rich}&tab=${tab}`);
      await waitInspector(TITLE.rich);
      await page.getByRole('heading', { name: marker }).waitFor();
    }
    // 每个标签点击都写入地址，并替换历史（不让标签切换堆满后退栈）。
    await page.getByRole('tab', { name: '证据' }).click();
    await page.waitForURL(url => url.searchParams.get('tab') === 'evidence');
  });

  // ⑥ 媒体轨道与查看器
  await step('媒体轨道：受控本机图片；查看器缩放、Esc、焦点回位；视频不内联', async () => {
    await open(`&work=${rich}&tab=materials`);
    await waitInspector(TITLE.rich);
    const cards = panel('材料').locator('button[data-index]');
    // 图片都是受控本机句柄，且真的加载出来。
    const images = panel('材料').locator('img[alt$="本地媒体对象"]');
    // 材料分区随详情读取到达：等第四张出现再数（标题先到时分区还是空的）。
    await images.nth(3).waitFor();
    assert.equal(await images.count(), 4);
    await page.waitForFunction(() => [...document.querySelectorAll('[role="tabpanel"]:not([hidden]) img')].every(image => image.complete && image.naturalWidth > 0));
    for (const source of await images.evaluateAll(list => list.map(image => image.getAttribute('src')))) assert.match(source, /^\/api\/local\/media\//);
    await shot('evidence-1440-materials');
    const first = panel('材料').locator('button[data-index="0"]');
    await first.click();
    const dialog = page.getByRole('dialog').filter({ has: page.getByRole('button', { name: '关闭查看器' }) });
    await dialog.waitFor();
    assert.match(await dialog.locator('strong').first().textContent(), /第 01 \/ 04 个/);
    assert.equal(await dialog.getByRole('button', { name: '100%' }).count(), 1);
    await dialog.getByRole('button', { name: '放大' }).click();
    await dialog.getByRole('button', { name: '150%' }).waitFor();
    await page.keyboard.press('0');
    await dialog.getByRole('button', { name: '100%' }).waitFor();
    await dialog.getByRole('button', { name: '下一张' }).click();
    assert.match(await dialog.locator('strong').first().textContent(), /第 02 \/ 04 个/);
    await shot('evidence-1440-lightbox');
    await page.keyboard.press('Escape');
    await dialog.waitFor({ state: 'hidden' });
    assert.equal(await page.evaluate(() => document.activeElement?.getAttribute('data-index')), '0', '关闭后焦点回到打开它的那一张');
    assert.equal(await cards.count(), 4);
    // 视频：不内联播放，也不当图片渲染——卡片只写字节状态词；封面等其余对象不受影响。
    await open(`&work=${video}&tab=materials`);
    await waitInspector(TITLE.video);
    const videoCard = panel('材料').locator('button[data-index]').filter({ hasText: '视频' });
    await videoCard.first().waitFor();
    assert.equal(await page.locator('video').count(), 0);
    assert.equal(await videoCard.count(), 1);
    assert.equal(await videoCard.locator('img').count(), 0, '视频不当图片渲染');
    assert.match(await videoCard.innerText(), /已取得/);
    assert.equal(await panel('材料').locator('img[alt$="本地媒体对象"]').count(), 1, '同一作品的封面仍是受控图片');
  });

  // ⑦ 评论
  await step('评论：点击前没有评论请求；点击后 20 条匿名评论，续读；不出现平台用户标识', async () => {
    const before = requestsTo('GET', /\/comments$/).length;
    await open(`&work=${rich}&tab=evidence`);
    await waitInspector(TITLE.rich);
    assert.equal(requestsTo('GET', /\/comments$/).length, before, '没有点击就不读取评论原文');
    const evidence = panel('证据');
    await evidence.getByRole('button', { name: '读取评论原文' }).click();
    await page.waitForFunction(() => document.querySelectorAll('[role="tabpanel"]:not([hidden]) article[data-relation]').length === 20);
    assert.match(await evidence.innerText(), /总数 \d+ · 当前 20 · 本次 20/);
    await evidence.getByRole('button', { name: '继续读取评论' }).click();
    await page.waitForFunction(() => document.querySelectorAll('[role="tabpanel"]:not([hidden]) article[data-relation]').length === 40);
    const text = await evidence.innerText();
    assert.doesNotMatch(text, /reader-\d|replier-\d/);
    assert.ok(text.includes('匿名评论'));
    // 评论与回复按稳定来源编号排序（orderBasis: stable_source_ref_not_platform_order），编号每次播种都不同，
    // 所以不断言哪一条在前两页；断言两页恰好是 40 条不同的评论或回复正文：续读不重复、不遗漏。
    const shown = [...text.matchAll(/合成(评论|回复) (\d+)：/g)].map(match => `${match[1]}${match[2]}`);
    assert.equal(shown.length, 40, '两页共 40 条评论或回复正文');
    assert.equal(new Set(shown).size, 40, '续读没有重复的条目');
    await shot('evidence-1440-comments');
  });

  // ⑧ 研究底稿
  await step('研究底稿：读取、预览旧任务输入（不调用模型）、导出内容、重新分层（已完成／已是当前版本）', async () => {
    const before = requestsTo('GET', /research-dossier$/).length;
    await open(`&work=${rich}`);
    await waitInspector(TITLE.rich);
    assert.equal(requestsTo('GET', /research-dossier$/).length, before, '底稿标签没有激活前不读取底稿');
    await page.getByRole('tab', { name: '研究底稿' }).click();
    const dossier = panel('研究底稿');
    await dossier.getByRole('heading', { name: '研究底稿' }).waitFor();
    assert.equal(requestsTo('GET', /research-dossier$/).length, before + 1);
    assert.match(await dossier.innerText(), /本篇可用布局 1 个 · 待分层 1 个 · 已是当前版本 0 个/);
    await shot('evidence-1440-dossier');
    // 对照原图：展开第一张图的识别结果，点一行在原图上标出位置（框坐标须都在 0–1）。
    await dossier.locator('details > summary').filter({ hasText: /图片|识别/ }).first().click();
    // 预览旧任务输入：只读，不调用模型。
    await dossier.getByRole('button', { name: '预览旧任务输入' }).click();
    await dossier.getByRole('button', { name: '按旧配置预览完整请求' }).waitFor();
    await dossier.getByRole('button', { name: '按旧配置预览完整请求' }).click();
    await page.waitForFunction(() => /预览|估算|上限/.test(document.querySelector('[role="tabpanel"]:not([hidden])')?.textContent ?? ''));
    // 导出：Markdown 与 JSON 都是用户显式触发的下载，内容来自服务器给的底稿。
    const [markdown] = await Promise.all([page.waitForEvent('download'), dossier.getByRole('button', { name: '导出 Markdown' }).click()]);
    assert.match(markdown.suggestedFilename(), /\.md$/);
    assert.ok((await readFile(await markdown.path(), 'utf8')).includes('合成长文'));
    const [json] = await Promise.all([page.waitForEvent('download'), dossier.getByRole('button', { name: '导出 JSON' }).click()]);
    assert.match(json.suggestedFilename(), /\.json$/);
    assert.ok(JSON.parse(await readFile(await json.path(), 'utf8')).revision);
    // 重新分层：第一次更新 1 个版面；之后已是当前版本。
    const writes = report.writes.length;
    await dossier.getByRole('button', { name: /^重新分层/ }).click();
    await page.waitForFunction(() => /本次 1 个版面：更新 1 个/.test(document.querySelector('[role="tabpanel"]:not([hidden])')?.textContent ?? ''));
    await writesTotal(writes + 1);
    await page.waitForFunction(() => /待分层 0 个 · 已是当前版本 1 个/.test(document.querySelector('[role="tabpanel"]:not([hidden])')?.textContent ?? ''));
    await dossier.getByRole('button', { name: /^重新分层/ }).click();
    await page.waitForFunction(() => /已是当前版本，无需更新/.test(document.querySelector('[role="tabpanel"]:not([hidden])')?.textContent ?? ''));
    await writesTotal(writes + 2);
  });

  await step('批量整理弹窗：固定范围逐篇预览与整理；断点只存引用；刷新后可继续', async () => {
    await open();
    for (const title of [TITLE.video, TITLE.risk, TITLE.partial]) await rowOf(title).getByRole('checkbox', { name: '勾选作品用于批量操作' }).check();
    await page.getByRole('button', { name: '整理图片文字', exact: true }).click();
    const dialog = page.getByRole('dialog', { name: '重新整理选定作品的图片文字' });
    await dialog.waitFor();
    await dialog.getByText('3 篇固定作品 · 已处理 0 篇 · 失败 0 篇 · 尚未处理 3 篇').waitFor();
    await dialog.getByRole('button', { name: '预览 / 刷新范围' }).click();
    await page.waitForFunction(() => document.querySelectorAll('dialog[open] li').length === 3 && [...document.querySelectorAll('dialog[open] li')].every(item => /待分层/.test(item.textContent ?? '')));
    const stored = JSON.parse(await page.evaluate(() => sessionStorage.getItem('linggan-research-dossier-batch-v1')));
    assert.deepEqual(stored.items.map(item => item.work).sort(), [video, risk, partial].sort());
    assert.doesNotMatch(JSON.stringify(stored), /合成/);
    await shot('evidence-1440-batch');
    await dialog.getByRole('button', { name: '关闭' }).click();
    // 刷新后可继续：从断点读回固定范围。
    await page.reload();
    await rows().first().waitFor();
    await page.getByRole('button', { name: '继续上次整理' }).click();
    const again = page.getByRole('dialog', { name: '重新整理选定作品的图片文字' });
    await again.getByText('3 篇固定作品 · 已处理 0 篇 · 失败 0 篇 · 尚未处理 3 篇').waitFor();
    await again.getByRole('button', { name: '开始 / 继续整理' }).click();
    await again.getByText('3 篇固定作品 · 已处理 3 篇 · 失败 0 篇 · 尚未处理 0 篇').waitFor();
    await again.getByRole('button', { name: '关闭' }).click();
    await page.reload();
    await rows().first().waitFor();
    assert.equal(await page.getByRole('button', { name: '继续上次整理' }).count(), 0, '全部完成后没有未完成的断点');
  });

  // ⑨ 立即复观测
  await step('立即复观测：确认步骤、取消不发请求；确认后库内一条请求一张工单、没有租约；再提交合并到同一工单；不可用时给原因', async () => {
    await open(`&work=${reobserve}`);
    await waitInspector(TITLE.reobserve);
    const section = panel('概览');
    const writes = report.writes.length;
    await section.getByRole('button', { name: '立即复观测' }).click();
    const dialog = page.getByRole('dialog', { name: '确认立即复观测' });
    await dialog.waitFor();
    assert.match(await dialog.innerText(), /提交后不能撤销/);
    assert.equal(await page.evaluate(() => document.activeElement?.textContent), '取消', '焦点默认在「取消」');
    await shot('evidence-1440-reobserve-confirm');
    await dialog.getByRole('button', { name: '取消' }).click();
    assert.equal(report.writes.length, writes, '取消不发请求');
    await section.getByRole('button', { name: '立即复观测' }).click();
    await dialog.getByRole('button', { name: '确认提交' }).click();
    await section.getByText(/已入队：复观测工单已形成，尚未由工位认领/).waitFor();
    assert.equal(await section.getByRole('button', { name: '复观测已入队' }).isDisabled(), true);
    await writesTotal(writes + 1);
    assert.equal(report.writes.at(-1).status, 200);
    await shot('evidence-1440-reobserve-queued');
    // 再提交一次（新开页面）：合并到覆盖这篇作品的在途工单。
    await open(`&work=${reobserve}`);
    await waitInspector(TITLE.reobserve);
    await section.getByRole('button', { name: '立即复观测' }).click();
    await dialog.getByRole('button', { name: '确认提交' }).click();
    await section.getByText(/已合并到覆盖当前作品的在途工单/).waitFor();
    await writesTotal(writes + 2);
    assert.equal(requestsTo('GET', /reobserve\/[^/]+$/).length, 0, '请求时没有租约，所以没有状态读取');
    // 不可用：没有关联授权的作品只说原因，不提供按钮。
    await open(`&work=${plain}`);
    await waitInspector(TITLE.plain);
    await panel('概览').getByText(/尚未具备监控目标关联的有效深度归档授权/).first().waitFor();
    assert.equal(await panel('概览').getByRole('button', { name: '立即复观测' }).count(), 0);
  });

  // ⑩ 入站
  await step('入站：地址直达不在首页的作品（不回退首项）；固定采集范围横幅；创作者页 → 证据库 → 返回创作者', async () => {
    await open(`&work=${offPage}`);
    await waitInspector(TITLE.offPage);
    assert.equal(new URL(page.url()).searchParams.get('work'), offPage, '不回退到首项、不改写地址');
    assert.equal(await page.locator('[data-work-ref] [aria-current="true"]').count(), 0, '直读的作品不在列表里');
    // 本次采集的固定作品集合：横幅、返回目标与读取范围。
    const back = `/collection/tasks?domain=${domainOne}&run=${collectionRun}`;
    // 这个运行编号在库里没有作品：页面如实给出结果（空或读取失败），但横幅、返回目标与读取范围都在。
    const runRead = page.waitForRequest(request => request.url().includes(`collectionRunRef=${collectionRun}`) && new URL(request.url()).pathname === LIST);
    await page.goto(`${base}&collectionRunRef=${collectionRun}&returnTo=${encodeURIComponent(back)}`);
    await page.getByText(/当前为本次采集的固定作品/).waitFor();
    await runRead;
    assert.equal(await page.getByRole('link', { name: '返回采集任务' }).getAttribute('href'), back);
    // 无效的运行编号：识别为地址问题，不发请求。
    const reads = listReads();
    await page.goto(`${base}&collectionRunRef=not-a-uuid`);
    await page.getByText('地址里的筛选条件无效').waitFor();
    assert.equal(listReads(), reads);
    await page.getByRole('button', { name: '清除这个筛选条件' }).click();
    await rows().first().waitFor();
    // 创作者页 → 证据库 → 返回创作者。
    await page.goto(`${origin}/corpus/creators?domain=${domainOne}&pageSize=25`);
    await page.getByRole('table').waitFor();
    const link = page.locator('tbody a[href^="/corpus/evidence"]').first();
    await link.click();
    await page.waitForURL(url => url.pathname === '/corpus/evidence');
    await page.getByText('当前为创作者作品筛选集合').waitFor();
    await rows().first().waitFor();
    await page.getByRole('link', { name: '返回创作者' }).click();
    await page.waitForURL(url => url.pathname === '/corpus/creators' && url.searchParams.get('domain') === domainOne);
    await page.getByRole('table').waitFor();
  });

  // ⑪ 评论深采交接
  await step('评论深采交接：写入采集页读取的存储键（数组格式），再跳转 /collection/tasks?selected=1&returnTo=…', async () => {
    // 前面的批量整理留下了它勾选的三篇：这一步从空集合开始，证明键里恰好是这里勾选的两篇。
    await open();
    await page.evaluate(key => sessionStorage.removeItem(key), `collection-selected:${domainOne}`);
    await open();
    assert.equal(await page.getByText('已勾选 0 篇作品').count(), 1);
    await rowOf(TITLE.rich).getByRole('checkbox', { name: '勾选作品用于批量操作' }).check();
    await rowOf(TITLE.video).getByRole('checkbox', { name: '勾选作品用于批量操作' }).check();
    assert.deepEqual(
      JSON.parse(await page.evaluate(key => sessionStorage.getItem(key), `collection-selected:${domainOne}`)).sort(),
      [rich, video].sort(),
    );
    const here = await page.evaluate(() => location.pathname + location.search);
    await Promise.all([page.waitForURL(url => url.pathname === '/collection/tasks'), page.getByRole('button', { name: '评论深采', exact: true }).click()]);
    const target = new URL(page.url());
    assert.equal(target.searchParams.get('domain'), domainOne);
    assert.equal(target.searchParams.get('selected'), '1');
    assert.equal(target.searchParams.get('returnTo'), here);
    assert.deepEqual(
      JSON.parse(await page.evaluate(key => sessionStorage.getItem(key), `collection-selected:${domainOne}`)).sort(),
      [rich, video].sort(),
      '采集页读同一个键',
    );
    await page.evaluate(key => sessionStorage.removeItem(key), `collection-selected:${domainOne}`);
  });

  // ⑫ 存储
  await step('存储只有三类键且无作品内容', async () => {
    await open();
    await page.getByRole('button', { name: '加宽', exact: true }).click();
    await rowOf(TITLE.rich).getByRole('checkbox', { name: '勾选作品用于批量操作' }).check();
    report.storage = await page.evaluate(() => ({
      local: Object.fromEntries(Object.keys(localStorage).map(key => [key, localStorage.getItem(key)])),
      session: Object.fromEntries(Object.keys(sessionStorage).map(key => [key, sessionStorage.getItem(key)])),
    }));
    assert.deepEqual(Object.keys(report.storage.local), ['linggan.evidence.inspectorWidth']);
    for (const key of Object.keys(report.storage.session)) assert.match(key, /^(collection-selected:[0-9a-f-]{36}|linggan-research-dossier-batch-v1)$/);
    for (const value of [...Object.values(report.storage.local), ...Object.values(report.storage.session)]) assert.doesNotMatch(value, /合成|先做两分钟/);
    await page.evaluate(() => {
      localStorage.clear();
      sessionStorage.clear();
    });
  });

  // ⑬ 四个视口
  for (const [width, height] of [[1440, 900], [1280, 720], [1024, 768], [390, 844]]) {
    await step(`${width}×${height}：无页面级横向溢出；${width >= 1181 ? '首屏见第一行与检查器标题' : '检查器是抽屉，点选展开、Esc 收起'}；静止态主按钮是实心墨色`, async () => {
      await page.setViewportSize({ width, height });
      await open();
      await waitInspector(TITLE.rich);
      await page.mouse.move(0, 0);
      let layout = await shot(`evidence-${width}-list`);
      assert.ok(layout.documentWidth <= layout.viewport, `${width} 宽度不应横向溢出`);
      // 静止态的主按钮：没有被页面级按钮样式压成白底（D1 教训：jsdom 不计算层叠，只有真实浏览器看得出）。
      const primary = await page.getByRole('button', { name: '读取材料' }).evaluate(node => {
        const style = getComputedStyle(node);
        return { background: style.backgroundColor, color: style.color, height: node.getBoundingClientRect().height };
      });
      assert.notEqual(primary.background, 'rgba(0, 0, 0, 0)');
      assert.notEqual(primary.background, 'rgb(255, 255, 255)');
      assert.ok(primary.height >= 40, `主按钮高度 ${primary.height}`);
      const firstRow = await rows().first().boundingBox();
      if (width >= 1181) {
        // 1280×720 首屏：第一行与检查器标题都在视口里。
        assert.ok(firstRow.y + firstRow.height <= height + 1 || firstRow.y < height, '第一行在首屏');
        const title = await inspector().locator('h2').first().boundingBox();
        assert.ok(title === null || title.y < height);
        const aside = await inspector().boundingBox();
        assert.ok(aside.x + aside.width <= width + 1, '检查器在视口内');
      } else {
        // 抽屉：自动选中不展开（惰性）；点选展开；Esc 收起并把焦点还给所选行。
        assert.equal(await inspector().evaluate(node => node.hasAttribute('inert')), true);
        await rowOf(TITLE.video).getByRole('button', { name: TITLE.video, exact: true }).click();
        await page.waitForFunction(() => !document.querySelector('aside[data-width]')?.hasAttribute('inert'));
        await waitInspector(TITLE.video);
        // 抽屉是滑入的：等动画停稳再量位置。
        await page.waitForFunction(() => getComputedStyle(document.querySelector('aside[data-width]')).transform === 'none');
        const drawer = await inspector().boundingBox();
        assert.ok(drawer.x >= 0 && drawer.x + drawer.width <= width + 1, '抽屉在视口内');
        layout = await shot(`evidence-${width}-drawer`);
        assert.ok(layout.documentWidth <= layout.viewport, `${width} 抽屉打开时不应横向溢出`);
        await page.keyboard.press('Escape');
        await page.waitForFunction(() => document.querySelector('aside[data-width]')?.hasAttribute('inert'));
        assert.equal(await page.evaluate(() => document.activeElement?.textContent), TITLE.video);
      }
    });
  }

  // ⑭ 全程只有同源请求、无脚本与控制台错误
  await step('全程只有同源请求；没有脚本错误；失败请求只有有意拦截的', async () => {
    assert.deepEqual(report.blockedRequests, [], '没有外域请求（包括平台图片）');
    assert.deepEqual(report.pageErrors, []);
    assert.deepEqual(report.consoleErrors.filter(text => !text.includes('Failed to load resource')), []);
    assert.deepEqual(report.failedRequests.filter(entry => !entry.includes('net::ERR_ABORTED')), []);
    // 写请求只有两类：重新分层（来自底稿与批量弹窗）与立即复观测。
    for (const write of report.writes) assert.match(write.path, /\/(relayer|reobserve)$/, `${write.method} ${write.path}`);
    assert.ok(report.writes.every(write => write.status === 200));
  });

  // ⑯ 负例（来源闸门与明确错误）
  await step('负例：跨站来源的页面与写请求返回 403；缺少领域、不存在的领域、非法范围与游标得到明确错误，不是空列表', async () => {
    const api = context.request;
    for (const [site, expected] of [['cross-site', 403], ['same-site', 403], ['none', 200], ['same-origin', 200]]) {
      const response = await api.get(`${origin}/corpus/evidence`, { headers: { 'Sec-Fetch-Site': site } });
      assert.equal(response.status(), expected, `Sec-Fetch-Site: ${site}`);
    }
    const hostile = await api.post(`${origin}/api/local/work-resources/${reobserve}/reobserve?domain=${domainOne}`, { headers: { Origin: 'https://evil.example' } });
    assert.equal(hostile.status(), 403);
    assert.equal((await hostile.json()).error, 'local_comment_study_origin_rejected');
    const crossSite = await api.post(`${origin}/api/local/work-resources/${reobserve}/reobserve?domain=${domainOne}`, { headers: { 'Sec-Fetch-Site': 'cross-site' } });
    assert.equal(crossSite.status(), 403);
    const read = async query => {
      const response = await api.get(`${origin}${LIST}${query}`);
      return { status: response.status(), body: await response.json().catch(() => null) };
    };
    assert.deepEqual([(await read('')).status, (await read('')).body.code], [400, 'domain_required']);
    assert.equal((await read(`?domain=${crypto.randomUUID()}`)).status >= 400, true, '不存在的领域不是空列表');
    assert.equal((await read(`?domain=${domainOne}&collectionRunRef=not-a-uuid`)).status, 400);
    assert.equal((await read(`?domain=${domainOne}&cursor=not-a-cursor`)).status, 400);
    assert.equal((await read(`?domain=${domainOne}&sort=relevance`)).status, 400, 'DR-1：相关度排序服务器不支持');
    assert.equal((await read(`?domain=${domainOne}&laneState=RETIRED`)).status, 400, 'DR-1：旧 OCR 已停用不是后端状态');
  });
  finished = true;
} finally {
  report.finished = finished;
  report.sources = { origin, totalWorks, pageSize, domainTwo, cleaned, restricted };
  await writeFile(path.join(directory, 'report.json'), JSON.stringify(report, null, 2));
  await browser.close();
}
console.log(`WEB-REACT-001 evidence page real-chain proof: ${report.steps.length} steps passed`);
