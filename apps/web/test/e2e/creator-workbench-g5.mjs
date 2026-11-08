// 预览地址直接提供冻结的 Gold Master Demo（2026-11-06 V2），不做任何改写。
// 本脚本验证三件事：提供出去的字节与冻结基准一致；六视口真实 Chromium 能渲染且无页面错误；
// 资产库弹层可打开并可关闭。只允许本机 loopback 的 GET 请求。
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import { chromium } from 'playwright';

const webRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const port = process.env.CW_G5_PORT || '4179';
const origin = 'http://127.0.0.1:' + port;
const output = process.env.CW_G5_DIR || path.join(webRoot, 'artifacts', 'private', 'creator-workbench-g5');
const referenceSha256 = 'c1e564b38811bee179154ac3b2af1847abceab1fa5f44fb67a81d5b958f6350e';
const referenceFile = path.join(webRoot, 'design-reference/creator-workbench-goldmaster-2026-11-06-v2.html');
const routePath = '/ui-assets/__preview/creator-workbench';
const viewports = [[1440, 1000], [1280, 720], [1024, 768], [820, 900], [390, 844], [375, 812]];
const report = {
  referenceSha256, candidate: 'frozen Gold Master Demo served as-is', productionIntegrated: false,
  route: null, servedSha256: null, viewports: [], checks: [], pageErrors: [], networkViolations: [], failures: [], browser: null,
};
let server;
let browser;

const sha256 = buffer => createHash('sha256').update(buffer).digest('hex');
await mkdir(output, { recursive: true });

async function check(name, action) {
  try {
    await action();
    report.checks.push({ name, result: 'PASS' });
    console.log('PASS ' + name);
  } catch (error) {
    const message = String(error?.stack || error);
    report.checks.push({ name, result: 'FAIL', message });
    report.failures.push({ name, message });
    console.error('FAIL ' + name + ': ' + message);
  }
}

async function screenshot(page, name) {
  await page.screenshot({ path: path.join(output, name + '.png'), animations: 'disabled' });
}

async function waitForVite() {
  for (let attempt = 0; attempt < 90; attempt++) {
    if (server.exitCode !== null) throw new Error('Vite exited early: ' + server.exitCode);
    try {
      const response = await fetch(origin + '/ui-assets/', { signal: AbortSignal.timeout(750) });
      if (response.ok) return;
    } catch {
      // The loopback Vite server can reject connections while starting; retry.
    }
    await new Promise(resolve => setTimeout(resolve, 350));
  }
  throw new Error('Vite dev server did not serve /ui-assets/ on loopback');
}

try {
  server = spawn(process.execPath,
    ['node_modules/vite/bin/vite.js', '--host', '127.0.0.1', '--port', port, '--strictPort'],
    { cwd: webRoot, env: { ...process.env, LINGGAN_WEB_DEV_API_ORIGIN: '' }, stdio: ['ignore', 'pipe', 'pipe'] });
  server.stdout.on('data', chunk => console.log('[Vite] ' + String(chunk).trim()));
  server.stderr.on('data', chunk => console.error('[Vite] ' + String(chunk).trim()));
  await waitForVite();

  await check('preview-route-serves-frozen-reference', async () => {
    const response = await fetch(origin + routePath);
    assert.equal(response.status, 200);
    assert.match(response.headers.get('content-type') || '', /text\/html/);
    report.servedSha256 = sha256(Buffer.from(await response.arrayBuffer()));
    assert.equal(report.servedSha256, referenceSha256);
    report.route = routePath;
  });

  await check('repository-reference-file-matches-frozen-sha256', async () => {
    assert.equal(sha256(await readFile(referenceFile)), referenceSha256);
  });

  browser = await chromium.launch({ headless: true });
  report.browser = browser.version();

  for (const [width, height] of viewports) {
    const context = await browser.newContext({
      viewport: { width, height }, deviceScaleFactor: 1, locale: 'zh-CN',
      timezoneId: 'Asia/Shanghai', colorScheme: 'light', reducedMotion: 'reduce',
      serviceWorkers: 'block',
    });
    await context.route('**/*', route => {
      const request = route.request(), url = new URL(request.url());
      if (url.origin !== origin || !['GET', 'HEAD'].includes(request.method()) || url.pathname.includes('/api/')) {
        report.networkViolations.push({ viewport: width, method: request.method(), url: request.url() });
        return route.abort();
      }
      return route.continue();
    });
    const page = await context.newPage();
    page.on('pageerror', error => report.pageErrors.push({ viewport: width, message: error.message }));
    page.setDefaultTimeout(10_000);
    try {
      await page.goto(origin + routePath, { waitUntil: 'networkidle', timeout: 20_000 });
      await page.locator('#app').waitFor({ state: 'visible' });
      await page.evaluate(async () => {
        await document.fonts.ready;
        await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
      });
      const measured = await page.evaluate(() => {
        const box = selector => {
          const node = document.querySelector(selector);
          if (!node) return null;
          const r = node.getBoundingClientRect();
          return { x: Math.round(r.x), y: Math.round(r.y), w: Math.round(r.width), h: Math.round(r.height) };
        };
        return {
          appChildren: document.querySelector('#app')?.children.length ?? 0,
          queue: box('#queue'), editor: box('#editor'),
          documentScrollWidth: document.documentElement.scrollWidth,
          queueText: (document.querySelector('#queue')?.innerText || '').slice(0, 120),
        };
      });
      await screenshot(page, 'reference-' + width + 'x' + height);
      report.viewports.push({ width, height, ...measured });
      await check('viewport-' + width + '-renders-reference', async () => {
        assert.ok(measured.appChildren > 0, 'app root is empty');
        assert.ok(measured.queue && measured.queue.w > 0, 'queue missing');
        assert.ok(measured.editor && measured.editor.w > 0, 'editor missing');
      });
      await check('viewport-' + width + '-no-horizontal-overflow', async () => {
        assert.ok(measured.documentScrollWidth <= width + 2,
          'document width ' + measured.documentScrollWidth + ' exceeds viewport ' + width);
      });
      if (width === 1440) {
        await check('asset-dialog-opens-and-closes', async () => {
          await page.getByRole('button', { name: '资产库' }).click();
          const dialog = page.locator('#overlay [role="dialog"]');
          await dialog.waitFor({ state: 'visible' });
          await screenshot(page, 'asset-dialog-1440');
          await page.keyboard.press('Escape');
          await dialog.waitFor({ state: 'hidden' });
        });
      }
    } catch (error) {
      report.failures.push({ name: 'viewport-' + width, message: String(error?.stack || error) });
      console.error('FAIL viewport-' + width + ': ' + String(error?.message || error));
      await screenshot(page, 'failed-' + width + 'x' + height).catch(() => {});
    } finally {
      await context.close();
    }
  }
} catch (error) {
  report.failures.push({ name: 'harness', message: String(error?.stack || error) });
} finally {
  report.browser = report.browser || 'launch_failed';
  // 冒烟通过不等于设计验收：视觉签收仍需人工对照冻结基准。
  report.status = report.failures.length || report.pageErrors.length || report.networkViolations.length
    ? 'BROWSER_PROOF_FAILED' : 'BROWSER_SMOKE_PASSED_G5_NOT_ACCEPTED';
  await writeFile(path.join(output, 'g5-browser-report.json'), JSON.stringify(report, null, 2));
  if (browser) await browser.close();
  if (server && server.exitCode === null) server.kill('SIGTERM');
  console.log('G5_BROWSER_SUMMARY ' + JSON.stringify({
    status: report.status, browser: report.browser, servedSha256: report.servedSha256,
    viewports: report.viewports.length, checks: report.checks.length,
    failedChecks: report.checks.filter(x => x.result === 'FAIL').length,
    pageErrors: report.pageErrors.length, networkViolations: report.networkViolations.length,
    failures: report.failures, output,
  }));
  if (report.status === 'BROWSER_PROOF_FAILED') process.exitCode = 1;
}
