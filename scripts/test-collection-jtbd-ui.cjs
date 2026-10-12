'use strict';

// Actual SSR and browser navigation, with no database and no collection POST.
// Rust render tests separately protect the populated tables and station actions.
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
const { chromium } = require('playwright');

const origin = 'http://127.0.0.1:18731';
const output = process.env.COLLECTION_PROOF_DIR;
assert(output, 'proof output must be an explicitly supplied temporary directory');

async function ready() {
  for (let attempt = 0; attempt < 40; attempt += 1) {
    try {
      const response = await fetch(`${origin}/health`);
      if (response.ok) return;
    } catch {}
    await new Promise(resolve => setTimeout(resolve, 500));
  }
  throw new Error('isolated API did not start');
}

(async () => {
  await ready();
  await fs.mkdir(output, { recursive: true });
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({ viewport: { width: 1440, height: 1000 } });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await context.route('**/*', route => {
    const request = route.request();
    const url = new URL(request.url());
    if (url.origin !== origin || request.method() !== 'GET') {
      return route.abort();
    }
    return route.continue();
  });
  const checks = [];
  try {
    await page.goto(`${origin}/collection`);
    assert.equal(new URL(page.url()).pathname, '/collection/targets');
    const rail = page.locator('aside[aria-label="采集导航"] a.v7-side-nav');
    assert.deepEqual(await rail.allTextContents(), ['01观察目标', '02采集任务']);
    checks.push('default page and exactly two daily navigation entries');

    const source = '/collection/targets?domain=all&filter=creator&sort=last';
    await page.goto(origin + source);
    await page.locator('.v7-domain-picker summary').click();
    const domains = page.locator('.v7-domain-picker a[data-collection-management]');
    assert((await domains.innerText()).includes('管理领域'));
    await domains.click();
    assert.equal(new URL(page.url()).pathname, '/collection/domains');
    assert.equal(new URL(page.url()).searchParams.get('return_to'), source);
    const back = page.locator('[data-collection-management-return]');
    assert.equal(await back.getAttribute('href'), source);
    await page.screenshot({ path: path.join(output, 'domains-unavailable.png'), fullPage: true });
    await back.click();
    assert.equal(page.url(), origin + source);
    checks.push('domain management remains available without data and returns exact list context');

    const stationEntry = page.locator('.v7-context-meta a[data-collection-management]');
    const entryBox = await stationEntry.boundingBox();
    const crumbBox = await page.locator('.v7-context-crumb').boundingBox();
    assert(entryBox.x > crumbBox.x + crumbBox.width, 'station entry is right of the breadcrumb');
    await stationEntry.click();
    assert.equal(new URL(page.url()).pathname, '/collection/runtime');
    assert.equal(new URL(page.url()).searchParams.get('return_to'), source);
    assert.equal(await page.locator('[data-collection-management-return]').getAttribute('href'), source);
    await page.screenshot({ path: path.join(output, 'stations-unavailable.png'), fullPage: true });
    await page.locator('[data-collection-management-return]').click();
    checks.push('global station management entry and exact return context');

    await page.goto(`${origin}/collection/tasks?domain=all`);
    assert.equal(await page.locator('#monitor-rule-single-collection').count(), 1);
    await page.screenshot({ path: path.join(output, 'tasks-unavailable.png'), fullPage: true });
    await page.locator('#monitor-rule-single-collection').click();
    await page.locator('[data-monitor-rule-initial-focus]').waitFor();
    assert.equal(await page.locator('button[name="command_kind"][value="manual_observe"]:enabled').count(), 0);
    await page.screenshot({ path: path.join(output, 'single-collection-unavailable.png'), fullPage: true });
    await page.keyboard.press('Escape');
    await page.waitForURL(url => url.pathname === '/collection/tasks' && url.search === '?domain=all' && url.hash === '#monitor-rule-single-collection');
    await page.waitForFunction(() => document.activeElement?.id === 'monitor-rule-single-collection');
    checks.push('one generic entry, unavailable state cannot submit, Escape returns focus');

    for (const oldPath of ['/collection/attention?domain=all', '/collection/operations?mode=trace&domain=all']) {
      await page.goto(origin + oldPath);
      assert.equal(new URL(page.url()).pathname, '/collection/tasks');
      assert.equal(new URL(page.url()).searchParams.get('domain'), 'all');
    }
    await page.goto(origin + source);
    await page.screenshot({ path: path.join(output, 'targets-unavailable.png'), fullPage: true });
    checks.push('retired pages redirect to task ledger and preserve domain');
    assert.deepEqual(errors, [], 'no uncaught browser errors');
    const report = { viewport: '1440x1000', database: 'not configured', checks, browserErrors: errors,
      notVerified: ['populated production screenshot', 'database mutation', 'plugin claim', 'real platform collection', 'deployment'] };
    await fs.writeFile(path.join(output, 'report.json'), JSON.stringify(report, null, 2));
    console.log(JSON.stringify(report, null, 2));
  } finally {
    await browser.close();
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
