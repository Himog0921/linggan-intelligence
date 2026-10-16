#!/usr/bin/env node
// Read-only visual regression capture. The refs file and resulting screenshots stay private.
// Usage: node scripts/test-collection-targets-react.cjs http://127.0.0.1:3107 <private-out> <private-refs.json>
const fs = require('node:fs');
const path = require('node:path');
const { createRequire } = require('node:module');
const assert = require('node:assert/strict');
const requireWeb = createRequire(path.resolve('apps/web/package.json'));
const { chromium } = requireWeb('playwright');
async function main() {
  const [originArg, outputArg, refsArg] = process.argv.slice(2);
  assert(originArg && outputArg && refsArg, 'origin/output/refs are required');
  const origin = new URL(originArg);
  assert(['127.0.0.1', 'localhost'].includes(origin.hostname), 'loopback only');
  const output = path.resolve(outputArg), refs = JSON.parse(fs.readFileSync(refsArg, 'utf8'));
  assert(/^[\da-f-]{36}$/i.test(refs.creator) && /^[\da-f-]{36}$/i.test(refs.keyword));
  fs.mkdirSync(output, { recursive: true, mode: 0o700 });
  async function settle(page) {
    await page.evaluate(async () => {
      await document.fonts.ready;
      await Promise.all(document.getAnimations().filter(animation =>
        animation.playState === 'running' && Number.isFinite(animation.effect?.getComputedTiming().endTime)
      ).map(animation => animation.finished.catch(() => undefined)));
      await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    });
  }
  const browser = await chromium.launch({ headless: true });
  const viewport = { width: Number(process.env.COLLECTION_SCREEN_WIDTH ?? 1440), height: Number(process.env.COLLECTION_SCREEN_HEIGHT ?? 1000) };
  assert(Number.isSafeInteger(viewport.width) && Number.isSafeInteger(viewport.height) && viewport.width > 0 && viewport.height > 0);
  const context = await browser.newContext({ viewport, deviceScaleFactor: 1 });
  const writes = [], external = [], report = [];
  await context.route('**/*', async route => {
    const request = route.request();
    if (!['GET', 'HEAD'].includes(request.method())) { writes.push({ method: request.method(), url: new URL(request.url()).pathname }); await route.abort(); return; }
    if (new URL(request.url()).origin !== origin.origin) { external.push(new URL(request.url()).origin); await route.abort(); return; }
    await route.continue();
  });
  const cases = [
    ['targets', { domain: 'all' }],
    ['creator-overview', { domain: 'all', drawer: refs.creator, dtab: 'overview' }],
    ['creator-works', { domain: 'all', drawer: refs.creator, dtab: 'works', wview: 'list' }],
    ['creator-trend', { domain: 'all', drawer: refs.creator, dtab: 'works', wview: 'performance', life_window: 'all', life_metric: 'likes', life_chart: 'trend', life_grain: 'month' }],
    ['creator-distribution', { domain: 'all', drawer: refs.creator, dtab: 'works', wview: 'performance', life_window: 'all', life_metric: 'likes', life_chart: 'distribution', life_grain: 'month' }],
    ['creator-patrol', { domain: 'all', drawer: refs.creator, dtab: 'patrol' }],
    ['keyword-works', { domain: 'all', drawer: refs.keyword, dtab: 'works' }],
    ['keyword-rules', { domain: 'all', rule: refs.keyword }],
  ];
  if (process.env.COLLECTION_TARGET_EXTRA_STATES) cases.push(...JSON.parse(fs.readFileSync(process.env.COLLECTION_TARGET_EXTRA_STATES, 'utf8')));
  const selectedNames = process.env.COLLECTION_TARGET_STATE_FILTER?.split(',');
  if (selectedNames) for (const name of selectedNames) assert(cases.some(row => row[0] === name), `unknown capture state ${name}`);
  try {
    for (const [name, query, readySelector] of cases) {
      if (selectedNames && !selectedNames.includes(name)) continue;
      const page = await context.newPage(), errors = [];
      page.on('pageerror', error => errors.push(error.message));
      const response = await page.goto(`${origin.origin}/collection/targets?${new URLSearchParams(query)}`);
      assert.equal(response.status(), 200);
      await page.locator('.c-tg-item').first().waitFor();
      await page.waitForFunction(() => !document.body.textContent.includes('读取领域…'));
      if (query.drawer) await page.waitForFunction(() => !document.querySelector('.c-dw-empty') && !document.querySelector('#c-drawer-title')?.textContent.includes('正在读取'));
      await page.waitForFunction(() => !document.querySelector('[aria-busy="true"]'));
      if (query.drawer) await page.locator('.c-dw-body').waitFor();
      if (query.rule) await page.locator(readySelector ?? '.c-rule-form').waitFor();
      else if (readySelector) await page.locator(readySelector).waitFor();
      await page.evaluate(async () => { await document.fonts.ready; await Promise.all([...document.images].map(img => img.complete ? Promise.resolve() : new Promise(resolve => { img.addEventListener('load', resolve, { once: true }); img.addEventListener('error', resolve, { once: true }); }))); });
      await settle(page);
      await page.screenshot({ path: path.join(output, `${name}.png`), fullPage: false });
      fs.writeFileSync(path.join(output, `${name}.html`), await page.content(), { mode: 0o600 });
      const geometry = await page.evaluate(() => Object.fromEntries(['.c-toolbar', '.c-tg-kind', '.c-dw', '.c-dw-head', '.c-dw-tabs', '.c-dw-body', '.c-rule-modal', '.life-trend-chart'].map(selector => { const r = document.querySelector(selector)?.getBoundingClientRect(); return [selector, r ? { x: r.x, y: r.y, width: r.width, height: r.height } : null]; })));
      assert.deepEqual(errors, [], `${name} page errors`);
      const scrollFrames = [];
      let selector = query.drawer ? '.c-dw-body' : query.rule ? '.c-rule-body' : '.c-body';
      if (!query.drawer && !query.rule) {
        const documentScrolls = await page.evaluate(() => {
          const body = document.querySelector('.c-body'), documentScroller = document.scrollingElement;
          return body && body.scrollHeight <= body.clientHeight + 1 && documentScroller && documentScroller.scrollHeight > documentScroller.clientHeight + 1;
        });
        if (documentScrolls) selector = 'html';
      }
      const scroll = page.locator(selector);
      if (await scroll.count()) {
        const size = await scroll.evaluate(el => ({ client: el.clientHeight, height: el.scrollHeight }));
        const step = Math.max(1, Math.floor(size.client * 0.8));
        for (let top = step, index = 1; top < size.height && size.height > size.client + 1; top += step, index++) {
          const actual = await scroll.evaluate((el, value) => { el.scrollTop = value; return el.scrollTop; }, top);
          const frame = `${name}-scroll-${String(index).padStart(2, '0')}.png`;
          await settle(page);
          await page.screenshot({ path: path.join(output, frame), fullPage: false });
          scrollFrames.push({ file: frame, scrollTop: actual, selector });
          if (actual + size.client >= size.height - 1) break;
        }
      }
      report.push({ name, status: response.status(), geometry, scrollFrames, errors });
      await page.close();
    }
    assert.deepEqual(writes, [], 'readonly captures must not attempt mutations');
    fs.writeFileSync(path.join(output, 'report.json'), JSON.stringify({ viewport, writes, external, cases: report }, null, 2), { mode: 0o600 });
    console.log(JSON.stringify({ captures: report.length, scrollFrames: report.reduce((sum, row) => sum + row.scrollFrames.length, 0), attemptedWrites: writes.length, externalRequests: external.length, output }));
  } finally { await browser.close(); }
}
main().catch(error => { console.error(error); process.exitCode = 1; });
