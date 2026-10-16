// WEB-REACT-001 阶段 D2 的插件入口证明：浏览器插件用 `chrome.tabs.create` 打开 `<本机地址>/corpus/evidence`
// （`plugins/linggan-intelligence-browser/src/linggan/background.js`，旧入口没有来源闸门，新入口经
// `local_comment_study_guard`：Host 须本机，Sec-Fetch-Site 须为 none 或 same-origin）。
// 本脚本在 Playwright 的持久化上下文里装一个最小的、未打包的 MV3 测试扩展（只有 tabs 权限），
// 由扩展自己的 service worker 调用 `chrome.tabs.create`：先打开本脚本自起的记录服务器，如实记下浏览器
// 为这种导航发出的请求头（Playwright 不为扩展新开的标签页报网络事件，所以请求头只能在服务器侧记）；
// 再打开隔离服务器上的证据库入口，用页面自己的 Navigation Timing 读 HTTP 状态并确认 React 页渲染出来
// ——结论是「插件入口不会被来源闸门挡住」还是「被挡住」，不靠推测。
// 只访问本机地址；扩展文件写在临时目录，用完即删。
import assert from 'node:assert/strict';
import { mkdtemp, rm, writeFile, readFile } from 'node:fs/promises';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';
import { chromium } from 'playwright';

const directory = process.env.WEB_APP_PROOF_DIR;
assert.ok(directory, 'WEB_APP_PROOF_DIR is required');
const { origin } = JSON.parse(await readFile(path.join(directory, 'fixture.json'), 'utf8'));
// 真实插件用的是 `http://localhost:3000`：主机名是 localhost，端口随隔离服务器。
const entry = `http://localhost:${new URL(origin).port}/corpus/evidence`;

const extension = await mkdtemp(path.join(os.tmpdir(), 'linggan-entry-extension-'));
const profile = await mkdtemp(path.join(os.tmpdir(), 'linggan-entry-profile-'));
await writeFile(
  path.join(extension, 'manifest.json'),
  JSON.stringify({ manifest_version: 3, name: 'Linggan entry probe (test only)', version: '0.0.1', permissions: ['tabs'], background: { service_worker: 'background.js' } }),
);
await writeFile(path.join(extension, 'background.js'), '// 测试扩展：本身什么也不做；Playwright 在它的 service worker 里调用 chrome.tabs.create。\n');

const result = { entry, installed: false, probeHeaders: null, status: null, title: null, rendered: null, dialog: null };
// 记录服务器：只回一个空白页，记下浏览器为扩展发起的导航带了哪些来源相关的请求头。
const seen = [];
const recorder = http.createServer((request, response) => {
  if (request.url === '/corpus/evidence') {
    seen.push(Object.fromEntries(['host', 'sec-fetch-site', 'sec-fetch-mode', 'sec-fetch-dest', 'origin'].map(name => [name, request.headers[name] ?? null])));
  }
  response.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
  response.end('<!doctype html><title>recorder</title>');
});
await new Promise(resolve => recorder.listen(0, '127.0.0.1', resolve));
const recorderEntry = `http://localhost:${recorder.address().port}/corpus/evidence`;
let context;
try {
  context = await chromium.launchPersistentContext(profile, {
    channel: 'chromium',
    headless: true,
    args: [`--disable-extensions-except=${extension}`, `--load-extension=${extension}`],
  });
  const worker = context.serviceWorkers()[0] ?? (await context.waitForEvent('serviceworker'));
  result.installed = worker.url().startsWith('chrome-extension://');
  assert.ok(result.installed, '测试扩展没有装上');
  const openFromExtension = async url => {
    const opened = context.waitForEvent('page');
    await worker.evaluate(target => globalThis.chrome.tabs.create({ url: target }), url);
    const page = await opened;
    await page.waitForLoadState('domcontentloaded');
    return page;
  };
  // ① 记录服务器：浏览器为扩展发起的导航带的请求头（来源闸门判的就是这几项）。
  const probe = await openFromExtension(recorderEntry);
  for (let attempt = 0; attempt < 50 && seen.length === 0; attempt += 1) await probe.waitForTimeout(100);
  assert.ok(seen[0], '记录服务器没有收到扩展发起的导航');
  result.probeHeaders = seen[0];
  await probe.close();
  // ② 真实入口：页面自己读到的文档 HTTP 状态（403 时会是闸门的错误回执，不会渲染出页面）。
  const page = await openFromExtension(entry);
  result.status = await page.evaluate(() => performance.getEntriesByType('navigation')[0]?.responseStatus ?? null);
  // 页面真的渲染出来：外壳、标题与「选择研究领域」对话框（插件入口没有领域参数）。
  await page.getByRole('heading', { level: 1, name: '证据库' }).waitFor();
  result.title = await page.title();
  result.rendered = true;
  result.dialog = await page.getByRole('dialog', { name: '选择研究领域' }).isVisible();
  await page.screenshot({ path: path.join(directory, 'evidence-plugin-entry.png') });
  await writeFile(path.join(directory, 'plugin-entry.json'), JSON.stringify(result, null, 2));
  assert.equal(result.status, 200, `插件入口被拒绝：HTTP ${result.status}`);
  assert.ok(['none', 'same-origin'].includes(result.probeHeaders['sec-fetch-site']), `Sec-Fetch-Site: ${result.probeHeaders['sec-fetch-site']}`);
  assert.equal(result.probeHeaders.origin, null);
  assert.equal(result.title, '证据库 · 语料');
  assert.equal(result.dialog, true);
  console.log(`WEB-REACT-001 plugin-entry proof: HTTP ${result.status}, Sec-Fetch-Site: ${result.probeHeaders['sec-fetch-site']} (tabs.create)`);
} catch (error) {
  await writeFile(path.join(directory, 'plugin-entry.json'), JSON.stringify({ ...result, error: String(error?.stack ?? error) }, null, 2));
  throw error;
} finally {
  await context?.close();
  recorder.close();
  await rm(extension, { recursive: true, force: true });
  await rm(profile, { recursive: true, force: true });
}
