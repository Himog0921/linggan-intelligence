#!/usr/bin/env node
// Read-only real-page modal presentation capture. Every non-GET/HEAD and foreign request
// is blocked before dispatch. Clicks only open local UI; no save/confirm/toggle is invoked.
const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const { createRequire } = require('node:module');
const { chromium } = createRequire(path.resolve('apps/web/package.json'))('playwright');
(async () => {
  const [origin, directory, refsPath] = process.argv.slice(2);
  assert(origin && directory && refsPath);
  assert(['127.0.0.1', 'localhost'].includes(new URL(origin).hostname));
  const refs = JSON.parse(fs.readFileSync(refsPath, 'utf8'));
  const output = path.resolve(directory); fs.mkdirSync(output, { recursive: true, mode: 0o700 });
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
  const context = await browser.newContext({ viewport: { width: 1440, height: 1000 }, deviceScaleFactor: 1 });
  const writes = [], external = [], errors = [], cases = [];
  await context.route('**/*', async route => {
    const request = route.request();
    if (!['GET', 'HEAD'].includes(request.method())) { writes.push(new URL(request.url()).pathname); return route.abort(); }
    if (new URL(request.url()).origin !== new URL(origin).origin) { external.push(new URL(request.url()).origin); return route.abort(); }
    return route.continue();
  });
  const page = await context.newPage(); page.on('pageerror', error => errors.push(error.message));
  async function goto(query = {}) {
    await page.goto(`${origin}/collection/targets?${new URLSearchParams({ domain: 'all', ...query })}`);
    await page.locator('.c-tg-item').first().waitFor();
    await page.waitForFunction(() => !document.body.textContent.includes('读取领域…') && !document.body.textContent.includes('正在读取观察目标'));
    if (query.drawer) await page.waitForFunction(() => !document.querySelector('.c-dw-empty') && !document.querySelector('#c-drawer-title')?.textContent.includes('正在读取'));
  }
  async function capture(name, selector, scrollSelector) {
    const element = page.locator(`${selector}:visible`).first(); await element.waitFor({ state: 'visible' });
    await page.evaluate(async () => { await document.fonts.ready; await Promise.all([...document.images].map(i => i.complete ? null : new Promise(r => { i.onload = r; i.onerror = r; }))); });
    await settle(page);
    await page.screenshot({ path: path.join(output, `${name}.png`) });
    fs.writeFileSync(path.join(output, `${name}.html`), await page.content(), { mode: 0o600 });
    const geometry = await element.boundingBox();
    const focus = await page.evaluate(() => ({ tag: document.activeElement?.tagName, name: document.activeElement?.getAttribute('name'), text: document.activeElement?.textContent?.slice(0, 100) }));
    const frames = [];
    if (scrollSelector && await page.locator(scrollSelector).count()) {
      const scroll = page.locator(scrollSelector).first();
      const size = await scroll.evaluate(el => ({ height: el.scrollHeight, client: el.clientHeight }));
      for (let y = Math.floor(size.client * 0.8), n = 1; size.height > size.client + 1 && y < size.height; y += Math.max(1, Math.floor(size.client * 0.8)), n++) {
        const top = await scroll.evaluate((el, y) => { el.scrollTop = y; return el.scrollTop; }, y);
        const file = `${name}-scroll-${String(n).padStart(2, '0')}.png`; await settle(page); await page.screenshot({ path: path.join(output, file) }); frames.push({ file, top });
        if (top + size.client >= size.height - 1) break;
      }
    }
    cases.push({ name, status: 'CAPTURED', geometry, focus, frames });
  }
  async function domainManager() {
    await goto();
    await page.locator('.v7-domain-picker > summary').click();
    await page.locator('.c-domain-management-link').click();
    await page.locator('.c-domain-list-row').first().waitFor();
  }
  try {
    await goto();
    await page.locator('[name=identity]').fill('ad0000000000000000000991');
    await page.getByRole('button', { name: '＋ 新建目标', exact: true }).click();
    await capture('target-create-domain', '.c-tg-batch-dialog');
    await page.locator('.c-tg-batch-dialog .c-tg-drawn-toggle').last().click();
    await capture('target-create-domain-select', '.c-tg-batch-dialog');

    await goto();
    await page.locator('.c-tg-select input').first().check();
    await page.getByRole('button', { name: /批量编辑/ }).click();
    await capture('target-batch', '.c-tg-batch-dialog');

    await goto({ delete: refs.creator });
    await capture('target-delete', '.c-tg-delete');

    await domainManager(); await capture('domain-uses-list', '.c-domain-manager', '.c-run-dialog-body');
    await page.locator('.c-domain-list-row').first().getByRole('button', { name: '编辑', exact: true }).click();
    await page.locator('.c-domain-manager .domain-card').waitFor();
    await capture('domain-uses-edit', '.c-domain-manager', '.c-run-dialog-body');

    await goto();
    const assignment = page.locator('.c-tg-act[href*="assign_domain="]').first();
    if (await assignment.count()) { await assignment.click(); await capture('target-assign-domain', '.c-tg-batch-dialog'); }
    else cases.push({ name: 'target-assign-domain', status: 'NOT VERIFIED', reason: 'Current real list has no unassigned target eligible for first-domain assignment; no synthetic identity or DB state inserted.' });

    await goto();
    const creators = await page.locator('#target-kind-creator').locator('xpath=..').locator('a[id^="target-"]').evaluateAll(nodes => nodes.map(node => node.id.slice(7)));
    let retirement = false;
    for (const target of creators) {
      await goto({ drawer: target, dtab: 'overview' });
      if (await page.locator('.c-dw-retire').count()) { await page.locator('.c-dw-retire').scrollIntoViewIfNeeded(); await capture('target-material-retirement', '.c-dw-retire', '.c-dw-body'); retirement = true; break; }
    }
    if (!retirement) cases.push({ name: 'target-material-retirement', status: 'NOT VERIFIED', reason: 'No current creator overview exposes a material-retirement judgment form. Existing implementation renders this inline, not as a separate modal; no shared data was changed to manufacture eligibility.' });
    assert.deepEqual(writes, []); assert.deepEqual(external, []); assert.deepEqual(errors, []);
    fs.writeFileSync(path.join(output, 'report.json'), JSON.stringify({ viewport: { width: 1440, height: 1000 }, writes, external, errors, cases }, null, 2), { mode: 0o600 });
    console.log(JSON.stringify({ captured: cases.filter(c => c.status === 'CAPTURED').length, notVerified: cases.filter(c => c.status === 'NOT VERIFIED').map(c => c.name), writes: writes.length, external: external.length }));
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exitCode = 1; });
