#!/usr/bin/env node
// Same-data visual capture only. Screenshots and the geometric manifest stay private.
// Usage: node scripts/test-collection-management-react.cjs <loopback-origin> <private-out> [private-refs.json]
// Optional refs: { "stationRef": "uuid", "domainRef": "uuid" }. Default: first rendered object.
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
const { createRequire } = require('node:module');
const { chromium } = createRequire(path.resolve(__dirname, '../apps/web/package.json'))('playwright');

function positions(total, visible) {
  const end = Math.max(0, total - visible), step = Math.max(1, Math.floor(visible * 0.8));
  const values = [0];
  for (let at = step; at < end; at += step) values.push(at);
  if (end > 0) values.push(end);
  return values;
}
async function main() {
  const [originArg, outputArg, refsArg] = process.argv.slice(2);
  assert(originArg && outputArg, 'loopback origin and private output directory are required');
  const origin = new URL(originArg);
  assert.equal(origin.protocol, 'http:');
  assert(['127.0.0.1', 'localhost'].includes(origin.hostname) && origin.port, 'loopback port only');
  assert(!origin.username && !origin.password && origin.pathname === '/' && !origin.search && !origin.hash);
  const output = path.resolve(outputArg);
  const privateRoot = path.resolve(__dirname, '../artifacts/private/web-react-001-g');
  assert(output.startsWith(`${privateRoot}${path.sep}`), 'real captures must stay in the ignored private evidence directory');
  const refs = refsArg ? JSON.parse(await fs.readFile(refsArg, 'utf8')) : {};
  for (const field of ['stationRef', 'domainRef']) {
    if (refs[field] !== undefined) assert(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(refs[field]), `invalid ${field}`);
  }
  const viewport = { width: Number(process.env.COLLECTION_SCREEN_WIDTH ?? 1440), height: Number(process.env.COLLECTION_SCREEN_HEIGHT ?? 1000) };
  assert(Object.values(viewport).every(value => Number.isSafeInteger(value) && value > 0));
  await fs.mkdir(output, { recursive: true, mode: 0o700 });
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({ viewport, deviceScaleFactor: 1 });
  const writes = [], external = [], errors = [], cases = [];
  const startedAt = new Date().toISOString();
  let capturedRefs;
  await context.route('**/*', async route => {
    const request = route.request(), url = new URL(request.url());
    if (!['GET', 'HEAD'].includes(request.method())) {
      writes.push({ method: request.method(), path: url.pathname }); return route.abort();
    }
    if (url.origin !== origin.origin) { external.push(url.origin); return route.abort(); }
    return route.continue();
  });
  const page = await context.newPage();
  page.on('pageerror', error => errors.push(error.message));
  async function settled() {
    await page.evaluate(async () => {
      await document.fonts.ready;
      await Promise.all([...document.images].map(img => img.complete ? Promise.resolve() : new Promise(resolve => {
        img.addEventListener('load', resolve, { once: true });
        img.addEventListener('error', resolve, { once: true });
      })));
      await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
      const finite = document.getAnimations().filter(animation => {
        const end = animation.effect?.getComputedTiming().endTime;
        return animation.playState !== 'finished' && animation.playState !== 'idle'
          && typeof end === 'number' && Number.isFinite(end);
      });
      if (finite.length) {
        let timer;
        try {
          await Promise.race([
            Promise.all(finite.map(animation => animation.finished.catch(() => undefined))),
            new Promise((_, reject) => { timer = setTimeout(() => reject(new Error('finite CSS animation did not settle within 5 seconds')), 5000); }),
          ]);
        } finally { clearTimeout(timer); }
        await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
      }
    });
  }
  async function visit(route, ready) {
    const response = await page.goto(`${origin.origin}${route}`);
    assert.equal(response.status(), 200);
    await page.locator(ready).first().waitFor({ state: 'visible' });
    await page.waitForFunction(() => !document.body.textContent.includes('读取领域…'));
    await settled();
  }
  async function capture(name, scope = '.c-body') {
    await settled();
    // These are actual scrollable elements, including the document and nested tables. Do not
    // enlarge a panel or rewrite overflow to manufacture a full-content screenshot.
    const scopeHandle = await page.locator(scope).elementHandle();
    assert(scopeHandle, `missing capture scope ${scope}`);
    const candidates = [scopeHandle, ...await page.locator(`${scope} *`).elementHandles()];
    if (scope === '.c-body') candidates.unshift(await page.locator('html').elementHandle());
    const containers = [], frames = [];
    for (const element of candidates) {
      const geometry = await element.evaluate(el => {
        const actual = el === document.documentElement ? document.scrollingElement : el;
        const rect = el.getBoundingClientRect(), style = getComputedStyle(el);
        const scrollY = /auto|scroll|overlay/.test(style.overflowY) || el === document.documentElement;
        const scrollX = /auto|scroll|overlay/.test(style.overflowX) || el === document.documentElement;
        return { visible: rect.width > 0 && rect.height > 0 && style.visibility !== 'hidden' && !el.closest('[hidden],dialog:not([open]),[inert]'),
          x: rect.x, y: rect.y, width: rect.width, height: rect.height,
          className: typeof el.className === 'string' ? el.className : el.tagName,
          clientWidth: actual.clientWidth, clientHeight: actual.clientHeight, scrollWidth: actual.scrollWidth, scrollHeight: actual.scrollHeight,
          scrollX: scrollX && actual.scrollWidth > actual.clientWidth + 1,
          scrollY: scrollY && actual.scrollHeight > actual.clientHeight + 1 };
      });
      if (geometry.visible && (geometry.scrollX || geometry.scrollY)) containers.push({ element, geometry });
    }
    await page.screenshot({ path: path.join(output, `${name}.png`), fullPage: false });
    await page.screenshot({ path: path.join(output, `${name}-document.png`), fullPage: true });
    for (const [index, { element, geometry }] of containers.entries()) {
      if (await element.evaluate(el => el !== document.documentElement)) await element.scrollIntoViewIfNeeded();
      const xs = geometry.scrollX ? positions(geometry.scrollWidth, geometry.clientWidth) : [0];
      const ys = geometry.scrollY ? positions(geometry.scrollHeight, geometry.clientHeight) : [0];
      for (const y of ys) for (const x of xs) {
        const actual = await element.evaluate((el, point) => {
          const target = el === document.documentElement ? document.scrollingElement : el;
          target.scrollTo({ top: point.y, left: point.x, behavior: 'instant' });
          return { x: target.scrollLeft, y: target.scrollTop };
        }, { x, y });
        await settled();
        const file = `${name}-container-${String(index + 1).padStart(2, '0')}-x${actual.x}-y${actual.y}.png`;
        await page.screenshot({ path: path.join(output, file), fullPage: false });
        frames.push({ file, container: index, ...actual });
      }
      await element.evaluate(el => (el === document.documentElement ? document.scrollingElement : el).scrollTo({ top: 0, left: 0, behavior: 'instant' }));
    }
    const current = new URL(page.url());
    cases.push({ name, scope, url: `${current.pathname}${current.search}`, containers: containers.map(({ geometry }) => geometry), frames });
  }
  try {
    await visit('/collection/domains', '.domain-create');
    const domainRefs = await page.locator('.domain-card').evaluateAll(rows => rows.map(row => row.getAttribute('data-domain-ref')));
    const domainIndex = refs.domainRef ? domainRefs.indexOf(refs.domainRef) : 0;
    assert(domainRefs.length > 0 && domainIndex >= 0, 'same-data domain is required for the edit dialog');
    await capture('domains');

    await visit('/collection/runtime', '.c-stn-row');
    await capture('runtime');
    const row = refs.stationRef ? page.locator(`[data-station-entry="${refs.stationRef}"]`) : page.locator('[data-station-entry]').first();
    const stationRef = await row.getAttribute('data-station-entry');
    assert(stationRef);
    capturedRefs = { stationRef, domainRef: domainRefs[domainIndex] };
    await row.locator('button.c-stn-manage').click();
    const manage = page.locator(`#station-manage-${stationRef}`);
    await manage.waitFor({ state: 'visible' });
    await capture('station-manage', `#station-manage-${stationRef}`);
    await manage.getByRole('button', { name: '关闭', exact: true }).click();
    await page.getByRole('button', { name: '新增工位', exact: true }).click();
    await page.locator('#station-register').waitFor({ state: 'visible' });
    // Equalize focus with the real legacy page, whose hidden return_to input was focused first.
    await page.locator('#station-register input[name=display_name]').focus();
    await capture('station-register', '#station-register');
    await page.locator('#station-register').getByRole('button', { name: '关闭', exact: true }).click();
    await page.getByRole('button', { name: '运行概览', exact: true }).click();
    await page.locator('#c-runtime-drawer[data-open]').waitFor({ state: 'visible' });
    await capture('runtime-overview', '#c-runtime-drawer');
    await page.getByRole('button', { name: '关闭运行概览', exact: true }).click();
    const unclaimed = page.locator('.c-stn-unclaimed');
    if (await unclaimed.count()) {
      if (await unclaimed.getAttribute('open') === null) await unclaimed.locator('summary').click();
      await capture('runtime-installations');
    } else cases.push({ name: 'runtime-installations', skipped: 'no unclaimed installations in the current read' });

    await visit('/collection/targets?domain=all', '.c-toolbar');
    const picker = page.locator('.v7-domain-picker');
    if (await picker.getAttribute('open') === null) await picker.locator('summary').click();
    await picker.locator('a[data-collection-management][href^="/collection/domains"]').click();
    const domainDialog = page.locator('.c-domain-manager[open]');
    await domainDialog.locator('.c-domain-list-row').first().waitFor();
    await capture('domain-manager', '.c-domain-manager[open]');
    await domainDialog.locator('.c-domain-list-row').nth(domainIndex).getByRole('button', { name: '编辑', exact: true }).click();
    await domainDialog.locator('.domain-fields').waitFor();
    await capture('domain-editor', '.c-domain-manager[open]');
    await domainDialog.getByText('材料统计与通道', { exact: true }).click();
    await capture('domain-editor-statistics', '.c-domain-manager[open]');
    await domainDialog.getByRole('button', { name: '← 返回领域列表', exact: true }).click();
    await domainDialog.getByRole('button', { name: '＋ 新建领域', exact: true }).click();
    await domainDialog.locator('.domain-create').waitFor();
    await capture('domain-new', '.c-domain-manager[open]');
    assert.deepEqual(writes, [], 'no capture may attempt a write');
    assert.deepEqual(external, [], 'no capture may access an external origin');
    assert.deepEqual(errors, [], 'page errors invalidate visual parity evidence');
    await fs.writeFile(path.join(output, 'management-capture-report.json'), JSON.stringify({ startedAt, finishedAt: new Date().toISOString(), origin: origin.origin, viewport, refs: capturedRefs, settlement: 'finite CSS animations awaited; infinite animations excluded; 5 second timeout', writes, external, errors, cases }, null, 2), { mode: 0o600 });
    console.log(JSON.stringify({ captures: cases.filter(row => !row.skipped).length, scrollFrames: cases.reduce((sum, row) => sum + (row.frames?.length ?? 0), 0), attemptedWrites: writes.length, externalRequests: external.length, output }));
  } finally { await browser.close(); }
}
main().catch(error => { console.error(error); process.exitCode = 1; });
