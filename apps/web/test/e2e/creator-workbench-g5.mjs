// G5 verification of the REAL React 19 + Vite 8 preview route (not the offline React 16 pilot).
// Operates only on loopback, synthetic preview data, and GET assets; rejects all API traffic.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import { chromium } from 'playwright';

const webRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const port = process.env.CW_G5_PORT || '4179';
const origin = 'http://127.0.0.1:' + port;
const output = process.env.CW_G5_DIR || path.join(webRoot, 'artifacts', 'private', 'creator-workbench-g5');
const referenceSha256 = 'c1e564b38811bee179154ac3b2af1847abceab1fa5f44fb67a81d5b958f6350e';
const viewports = [[1440, 1000], [1280, 720], [1024, 768], [820, 900], [390, 844], [375, 812]];
const report = {
  referenceSha256, candidate: 'actual React19/Vite8 DEV route', productionIntegrated: false,
  visualReferenceScreenshotsCompared: false, visualStatus: 'NOT_VERIFIED',
  behaviorStatus: 'PARTIAL_NOT_VERIFIED', route: null, viewports: [], interactions: [],
  pageErrors: [], networkViolations: [], failures: [], browser: null, fonts: [],
};
let server;
let browser;
await mkdir(output, { recursive: true });
const near = (value, target, name, tolerance = 2) => {
  assert.ok(value !== null && Math.abs(value - target) <= tolerance,
    name + ': expected ' + target + ' ±' + tolerance + ', measured ' + value);
};
async function verify(name, action) {
  try {
    await action();
    report.interactions.push({ name, result: 'PASS' });
    console.log('PASS ' + name);
  } catch (error) {
    const message = String(error?.stack || error);
    report.interactions.push({ name, result: 'FAIL', message });
    report.failures.push({ name, message });
    console.error('FAIL ' + name + ': ' + message);
  }
}
async function screenshot(page, name) {
  await page.screenshot({ path: path.join(output, name + '.png'), fullPage: false, animations: 'disabled' });
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
function sample() {
  const rounded = n => Math.round(n * 100) / 100;
  const box = selector => {
    const node = document.querySelector(selector);
    if (!(node instanceof HTMLElement)) return null;
    const r = node.getBoundingClientRect(), cs = getComputedStyle(node);
    return { x: rounded(r.x), y: rounded(r.y), w: rounded(r.width), h: rounded(r.height),
      display: cs.display, overflowX: cs.overflowX, fontFamily: cs.fontFamily };
  };
  return {
    viewport: { width: innerWidth, height: innerHeight },
    documentScrollWidth: document.documentElement.scrollWidth,
    header: box('[data-v7-shell-header]'),
    sidebar: box('.v7-side'),
    queue: box('[aria-label="稿件队列"]'),
    editor: box('[aria-label="稿件编辑区"]'),
    editorHead: box('[data-testid="editor-head"]'),
    editorScroll: box('[data-testid="editor-scroll"]'),
    editorInner: box('[data-testid="editor-inner"]'),
    intent: box('[data-testid="intent-row"]'),
    related: box('[data-testid="related-card"]'),
    cover: box('[data-testid="cover-card"]'),
    coverFields: box('[data-testid="cover-fields"]'),
    bodyHeading: box('[data-testid="body-heading"]'),
    firstParagraph: box('[data-testid="first-paragraph"]'),
    inspector: box('[aria-label="稿件检查器"]'),
    main: box('main[aria-label="创作工作台"]'),
    fonts: [...document.fonts].map(f => ({ family: f.family, status: f.status })),
  };
}
try {
  server = spawn(process.execPath,
    ['node_modules/vite/bin/vite.js', '--host', '127.0.0.1', '--port', port, '--strictPort'],
    { cwd: webRoot, env: { ...process.env, LINGGAN_WEB_DEV_API_ORIGIN: '' }, stdio: ['ignore','pipe','pipe'] });
  server.stdout.on('data', chunk => console.log('[Vite] ' + String(chunk).trim()));
  server.stderr.on('data', chunk => console.error('[Vite] ' + String(chunk).trim()));
  await waitForVite();
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
    page.on('pageerror', error => report.pageErrors.push({ width, message: error.message }));
    page.setDefaultTimeout(10_000);
    let located = false;
    for (const pathname of ['/ui-assets/__preview/creator-workbench', '/__preview/creator-workbench']) {
      try {
        await page.goto(origin + pathname, { waitUntil: 'networkidle', timeout: 20_000 });
        await page.getByRole('main', { name: '创作工作台' }).waitFor({ state: 'visible', timeout: 2500 });
        report.route = pathname;
        located = true;
        break;
      } catch (error) {
        const diagnostic = await page.evaluate(() => ({
          path: location.pathname, documentTitle: document.title,
          bodyText: (document.body?.innerText || '').slice(0, 400),
          scripts: [...document.querySelectorAll('script')].map(script => script.getAttribute('src')),
          mainCount: document.querySelectorAll('main').length,
        })).catch(() => null);
        console.log('Preview path not ready: ' + pathname + ' (' + String(error?.message || error).slice(0, 140) + ') ' + JSON.stringify(diagnostic));
        await screenshot(page, 'route-probe-' + width + '-' + pathname.replace(/[^a-z0-9]/gi, '_')).catch(() => {});
      }
    }
    if (!located) {
      report.failures.push({ name: 'preview-route-' + width, message: 'Real Vite preview route did not render' });
      await context.close();
      continue;
    }
    await page.evaluate(async () => {
      await document.fonts.ready;
      await Promise.all([...document.images].map(image => image.decode().catch(() => {})));
      await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    });
    const geometry = await page.evaluate(sample);
    report.viewports.push({ width, height, geometry, screenshot: 'default-' + width + 'x' + height + '.png' });
    console.log('GEOMETRY ' + width + ' ' + JSON.stringify(geometry));
    await screenshot(page, 'default-' + width + 'x' + height);
    await verify('viewport-' + width + '-no-horizontal-document-overflow', async () => {
      assert.ok(geometry.documentScrollWidth <= width + 2,
        width + ': document width ' + geometry.documentScrollWidth + ' exceeds viewport ' + width);
    });
    await verify('viewport-' + width + '-main-visible', async () => {
      assert.ok(geometry.main && geometry.main.w > 0 && geometry.main.h > 0,
        width + ': main workspace collapsed');
    });
    if (width === 1440) {
      report.fonts = geometry.fonts;
      await verify('approved-light-rail-and-preview-navigation', async () => {
        assert.equal(await page.locator('.v7-context-row').getAttribute('data-context-texture'), 'light');
        assert.equal(await page.getByText('A娃家长矩阵', { exact: true }).isVisible(), true);
        assert.equal(await page.getByRole('button', { name: '发布记录' }).isVisible(), true);
      });
      await verify('gold-master-1440-primary-geometry', async () => {
        near(geometry.header?.h, 121.69, 'header height');
        near(geometry.sidebar?.w, 160, 'shared sidebar width');
        near(geometry.queue?.x, 160, 'queue x');
        near(geometry.queue?.w, 240, 'queue width');
        near(geometry.editor?.x, 400, 'editor x');
        near(geometry.editor?.w, 704, 'editor width');
        near(geometry.inspector?.x, 1104, 'inspector x');
        near(geometry.inspector?.w, 336, 'inspector width');
      });
      await verify('gold-master-1440-editor-composition', async () => {
        near(geometry.editorHead?.h, 48, 'editor head height');
        near(geometry.editorScroll?.y, 169.69, 'editor scroll top');
        near(geometry.editorInner?.x, 420, 'editor inner x');
        near(geometry.editorInner?.w, 654, 'editor inner width');
        near(geometry.intent?.y, 181.69, 'intent row y');
        near(geometry.related?.y, 207.69, 'related card y');
        near(geometry.related?.h, 68, 'related card height');
        near(geometry.cover?.x, 420, 'cover x');
        near(geometry.cover?.y, 287.69, 'cover y');
        near(geometry.cover?.w, 132, 'cover width');
        near(geometry.cover?.h, 176, 'cover height');
        near(geometry.coverFields?.x, 568, 'cover fields x');
        near(geometry.coverFields?.w, 506, 'cover fields width');
        near(geometry.bodyHeading?.y, 516.19, 'body heading y');
        near(geometry.firstParagraph?.y, 563.19, 'first paragraph y');
      });
      await verify('gold-master-1440-editor-composition', async () => {
        near(geometry.editorHead?.h, 48, 'editor head height');
        near(geometry.editorScroll?.y, 169.69, 'editor scroll top');
        near(geometry.editorInner?.x, 420, 'editor inner x');
        near(geometry.editorInner?.w, 654, 'editor inner width');
        near(geometry.intent?.y, 181.69, 'intent row y');
        near(geometry.related?.y, 207.69, 'related card y');
        near(geometry.related?.h, 68, 'related card height');
        near(geometry.cover?.x, 420, 'cover x');
        near(geometry.cover?.y, 287.69, 'cover y');
        near(geometry.cover?.w, 132, 'cover width');
        near(geometry.cover?.h, 176, 'cover height');
        near(geometry.coverFields?.x, 568, 'cover fields x');
        near(geometry.coverFields?.w, 506, 'cover fields width');
        near(geometry.bodyHeading?.y, 516.19, 'body heading y');
        near(geometry.firstParagraph?.y, 563.19, 'first paragraph y');
      });
      await verify('asset-dialog-six-types-and-focus-return', async () => {
        const trigger = page.getByRole('button', { name: '资产库' });
        await trigger.click();
        const dialog = page.getByRole('dialog', { name: '创作资产库' });
        await dialog.waitFor({ state: 'visible' });
        assert.equal(await page.getByRole('navigation', { name: '资产类型' }).getByRole('button').count(), 6);
        await screenshot(page, 'asset-persona-1440.png');
        await page.keyboard.press('Escape');
        await dialog.waitFor({ state: 'hidden' });
        assert.equal(await trigger.evaluate(el => document.activeElement === el), true);
      });
      await verify('editor-desk-return-preserves-draft-and-canvas-boundary', async () => {
        const text = 'G5 验证本地编辑状态保留';
        const paragraph = page.getByRole('textbox', { name: '钩子段落' });
        await paragraph.fill(text);
        await page.getByRole('button', { name: /选题与材料/ }).click();
        await page.getByRole('region', { name: '沉浸材料桌' }).waitFor({ state: 'visible' });
        const canvas = page.getByTestId('canvas-viewport');
        const original = await canvas.locator('[data-canvas-item]').count();
        const card = canvas.locator('[data-canvas-item]').first();
        await card.dblclick();
        assert.equal(await canvas.locator('[data-canvas-item]').count(), original, 'card double-click created note');
        await canvas.dblclick({ position: { x: 14, y: 14 } });
        assert.equal(await canvas.locator('[data-canvas-item]').count(), original + 1, 'empty canvas double-click did not create note');
        await screenshot(page, 'desk-1440.png');
        await page.getByRole('button', { name: '返回写稿' }).click();
        assert.equal(await page.getByRole('textbox', { name: '钩子段落' }).textContent(), text);
      });

      await verify('g5-path-dialog-escape-and-focus-restore', async () => {
        const trigger = page.getByRole('button', { name: '使用路径' });
        await trigger.click();
        const dialog = page.getByRole('dialog', { name: '使用路径' });
        await dialog.waitFor({ state: 'visible' });
        assert.ok(await dialog.getByText('四条常走的路').isVisible());
        await page.keyboard.press('Escape');
        await dialog.waitFor({ state: 'hidden' });
        assert.equal(await trigger.evaluate(node => document.activeElement === node), true);
      });
      await verify('g5-sample-template-preview-no-persist', async () => {
        const trigger = page.getByRole('button', { name: '存为样板' });
        await trigger.click();
        const dialog = page.getByRole('dialog', { name: '存为样板' });
        await dialog.waitFor({ state: 'visible' });
        await dialog.getByRole('textbox', { name: '样板名称' }).fill('仅供验证的样板');
        await dialog.getByRole('button', { name: /存进样板库/ }).click();
        await dialog.waitFor({ state: 'hidden' });
        assert.equal(await page.getByText(/未调用正式 API/).count() > 0, true);
      });
      await verify('g5-experience-picker-blocks-unconfirmed', async () => {
        const before = await page.locator('[contenteditable="true"][role="textbox"]').count();
        await page.getByRole('button', { name: '后面插入经历' }).first().click({ force: true });
        const dialog = page.getByRole('dialog', { name: '选择经历' });
        await dialog.waitFor({ state: 'visible' });
        assert.equal(await dialog.getByRole('button', { name: '尚不可用' }).isDisabled(), true);
        await dialog.getByRole('button', { name: '用这段' }).click();
        await dialog.waitFor({ state: 'hidden' });
        const after = await page.locator('[contenteditable="true"][role="textbox"]').count();
        assert.equal(after, before + 1);
      });
      await verify('g5-product-edit-local-cancel', async () => {
        await page.getByRole('button', { name: '资产库' }).click();
        const dialog = page.getByRole('dialog', { name: '创作资产库' });
        await dialog.getByRole('navigation', { name: '资产类型' }).getByRole('button', { name: /产品事实卡/ }).click();
        await dialog.getByRole('button', { name: '编辑', exact: true }).click();
        assert.equal(await dialog.getByRole('textbox', { name: '规范名称' }).isVisible(), true);
        await dialog.getByRole('button', { name: '取消', exact: true }).click();
        assert.equal(await dialog.getByRole('textbox', { name: '规范名称' }).count(), 0);
        await page.keyboard.press('Escape');
      });
      await verify('g5-tour-popover-dismissable', async () => {
        const trigger = page.getByRole('button', { name: /体验清单/ });
        await trigger.click();
        const tour = page.getByRole('region', { name: '体验清单' });
        await tour.waitFor({ state: 'visible' });
        assert.equal(await tour.locator('li').count(), 8);
        await tour.getByRole('button', { name: '收起' }).click();
        await tour.waitFor({ state: 'hidden' });
      });
      await verify('inspector-keyboard-tabs', async () => {
        const checks = page.getByRole('tab', { name: /检查/ });
        await checks.focus();
        await page.keyboard.press('ArrowRight');
        assert.equal(await page.getByRole('tab', { name: '素材' }).getAttribute('aria-selected'), 'true');
        await page.keyboard.press('End');
        assert.equal(await page.getByRole('tab', { name: '预览' }).getAttribute('aria-selected'), 'true');
      });
    }
    if (width <= 820) {
      await verify('mobile-' + width + '-queue-and-inspector-reachable', async () => {
        const queue = page.getByRole('button', { name: '打开今日稿件队列' });
        const inspector = page.getByRole('button', { name: '打开稿件检查器' });
        await queue.click();
        assert.equal(await queue.getAttribute('aria-expanded'), 'true');
        assert.equal(await page.getByRole('complementary', { name: '稿件队列' }).isVisible(), true);
        await page.getByRole('button', { name: '关闭稿件队列' }).click();
        assert.equal(await queue.getAttribute('aria-expanded'), 'false');
        await inspector.click();
        assert.equal(await inspector.getAttribute('aria-expanded'), 'true');
        assert.equal(await page.getByRole('complementary', { name: '稿件检查器' }).isVisible(), true);
        await screenshot(page, 'mobile-inspector-' + width + '.png');
        await page.getByRole('button', { name: '关闭稿件检查器' }).click();
        assert.equal(await inspector.getAttribute('aria-expanded'), 'false');
      });
    }
    await context.close();
  }

  // G3 visual matrix: 28 canonical states. Six default viewports were captured
  // above. Capture every reproducible additional state with synthetic fixtures;
  // report unimplemented UI explicitly instead of silently counting it passed.
  const openAsset = async (page, label) => {
    await page.getByRole('button', { name: '资产库' }).click();
    await page.getByRole('dialog', { name: '创作资产库' }).waitFor({ state: 'visible' });
    if (label) await page.getByRole('navigation', { name: '资产类型' })
      .getByRole('button', { name: new RegExp(label) }).click();
  };
  const openDeskState = async page => {
    if (page.viewportSize()?.width <= 820) {
      await page.getByRole('button', { name: '打开今日稿件队列' }).click();
    }
    await page.getByRole('button', { name: /选题与材料/ }).click();
    await page.getByRole('region', { name: '沉浸材料桌' }).waitFor({ state: 'visible' });
  };
  const visualStates = [
    ['asset-persona', 1440, 1000, page => openAsset(page, null)],
    ['asset-experience', 1440, 1000, page => openAsset(page, '经历库')],
    ['asset-product', 1440, 1000, page => openAsset(page, '产品事实卡')],
    ['asset-product-edit', 1440, 1000, async page => {
      await openAsset(page, '产品事实卡');
      await page.getByRole('dialog', { name: '创作资产库' }).getByRole('button', { name: '编辑', exact: true }).click();
    }],
    ['asset-bridge', 1440, 1000, page => openAsset(page, '衔接句')],
    ['asset-sample', 1440, 1000, page => openAsset(page, '样板库')],
    ['asset-style', 1440, 1000, page => openAsset(page, '风格')],
    ['inspector-material', 1440, 1000, page => page.getByRole('tab', { name: '素材' }).click()],
    ['inspector-title', 1440, 1000, page => page.getByRole('tab', { name: '标题' }).click()],
    ['inspector-preview', 1440, 1000, page => page.getByRole('tab', { name: '预览' }).click()],
    ['side-collapsed', 1440, 1000, page => page.getByRole('button', { name: '折叠侧栏' }).click()],
    ['draft-d2', 1440, 1000, page => page.getByRole('button', { name: /我不再替他收书包了/ }).click()],
    ['experience-picker', 1440, 1000, page => page.getByRole('button', { name: '后面插入经历' }).first().click({ force: true })],
    ['save-as-template', 1440, 1000, page => page.getByRole('button', { name: '存为样板' }).click()],
    ['path-guide', 1440, 1000, page => page.getByRole('button', { name: '使用路径' }).click()],
    ['tour-popover', 1440, 1000, page => page.getByRole('button', { name: /体验清单/ }).click()],
    ['desk-default', 1440, 1000, openDeskState],
    ['desk-search', 1440, 1000, async page => {
      await openDeskState(page);
      await page.getByRole('searchbox', { name: '搜索材料' }).fill('调整');
      await page.waitForTimeout(350);
    }],
    ['desk-card-editor', 1440, 1000, async page => {
      await openDeskState(page);
      await page.locator('[data-canvas-item="m1"]').click();
    }],
    ['desk-focus', 1440, 1000, async page => {
      await openDeskState(page);
      await page.locator('[data-canvas-item="f1"]').click();
    }],
    ['mobile-asset-persona', 390, 844, page => openAsset(page, null)],
    ['mobile-desk', 390, 844, openDeskState],
  ];
  report.visualStates = [
    ...viewports.map(([width, height]) => ({
      id: 'V-BASE-' + width + 'x' + height,
      result: 'CAPTURED_NOT_VISUALLY_ACCEPTED',
      screenshot: 'default-' + width + 'x' + height + '.png',
    })),
  ];
  for (const [id, width, height, action] of visualStates) {
    const context = await browser.newContext({
      viewport: { width, height }, deviceScaleFactor: 1,
      locale: 'zh-CN', timezoneId: 'Asia/Shanghai',
      colorScheme: 'light', reducedMotion: 'reduce', serviceWorkers: 'block',
    });
    let captureStatus = 'CAPTURED_NOT_VISUALLY_ACCEPTED';
    let error = null;
    const fileName = 'state-' + id + '-' + width + 'x' + height + '.png';
    try {
      await context.route('**/*', route => {
        const req = route.request(), uri = new URL(req.url());
        if (uri.origin !== origin || !['GET', 'HEAD'].includes(req.method()) || uri.pathname.includes('/api/')) {
          report.networkViolations.push({ state: id, method: req.method(), url: req.url() });
          return route.abort();
        }
        return route.continue();
      });
      const page = await context.newPage();
      page.on('pageerror', err => report.pageErrors.push({ state: id, message: err.message }));
      page.setDefaultTimeout(8000);
      await page.goto(origin + '/ui-assets/__preview/creator-workbench', { waitUntil: 'networkidle', timeout: 20000 });
      await page.getByRole('main', { name: '创作工作台' }).waitFor({ state: 'visible' });
      await page.evaluate(async () => { await document.fonts.ready; });
      await action(page);
      await page.screenshot({ path: path.join(output, fileName), animations: 'disabled' });
    } catch (caught) {
      captureStatus = 'CANDIDATE_CAPTURE_FAILED';
      error = String(caught?.message || caught).slice(0, 420);
    } finally {
      report.visualStates.push({ id: 'V-STATE-' + id, result: captureStatus,
        screenshot: captureStatus.startsWith('CAPTURED') ? fileName : null, error });
      await context.close();
    }
  }
  report.visualStateCoverage = {
    required: 28, accounted: report.visualStates.length,
    captured: report.visualStates.filter(item => item.result.startsWith('CAPTURED')).length,
    notImplemented: report.visualStates.filter(item => item.result === 'NOT_IMPLEMENTED_IN_G4_PREVIEW').length,
    failedCapture: report.visualStates.filter(item => item.result === 'CANDIDATE_CAPTURE_FAILED').length,
    visuallyAccepted: 0,
  };
  assert.equal(report.visualStateCoverage.accounted, 28, 'Every approved visual state must be accounted for');
  console.log('G5_VISUAL_STATE_MATRIX ' + JSON.stringify(report.visualStateCoverage));
} catch (error) {
  report.failures.push({ name: 'harness', message: String(error?.stack || error) });
} finally {
  report.browser = report.browser || 'launch_failed';
  // A G5 proof never becomes VERIFIED merely because smoke assertions passed:
  // screenshot-to-GoldMaster diffs, 40 journeys, 20 negatives and 8 a11y classes are separate hard gates.
  report.status = report.failures.length || report.pageErrors.length || report.networkViolations.length
    ? 'BROWSER_PROOF_FAILED' : 'BROWSER_SMOKE_PASSED_G5_NOT_ACCEPTED';
  await writeFile(path.join(output, 'g5-browser-report.json'), JSON.stringify(report, null, 2));
  if (browser) await browser.close();
  if (server && server.exitCode === null) server.kill('SIGTERM');
  console.log('G5_BROWSER_SUMMARY ' + JSON.stringify({
    status: report.status, browser: report.browser, viewports: report.viewports.length,
    checks: report.interactions.length, failedChecks: report.interactions.filter(x => x.result === 'FAIL').length,
    pageErrors: report.pageErrors.length, networkViolations: report.networkViolations.length, failures: report.failures,
    visualStatus: report.visualStatus, behaviorStatus: report.behaviorStatus, output,
  }));
  if (report.status === 'BROWSER_PROOF_FAILED') process.exitCode = 1;
}
