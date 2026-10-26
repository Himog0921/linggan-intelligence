import assert from 'node:assert/strict';
import { chromium } from 'playwright';

const origin = process.env.WEB_TEAM_ORIGIN;
assert.ok(origin, 'WEB_TEAM_ORIGIN is required');
assert.equal(new URL(origin).hostname, '127.0.0.1');

const members = Array.from({ length: 22 }, (_, index) => ({
  memberRef: `member-${index + 1}`,
  email: `member-${index + 1}@example.com`,
  displayName: `成员 ${index + 1}`,
  role: 'operator',
  state: 'active',
  version: 1,
  currentAccountCount: 0,
}));

const browser = await chromium.launch({ headless: true });
try {
  const page = await browser.newPage({ viewport: { width: 1254, height: 888 } });
  await page.route(`${origin}/settings/team`, async route => route.fulfill({
    response: await route.fetch({ url: `${origin}/ui-assets/` }),
  }));
  await page.route('**/api/local/auth/members', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({ members }),
  }));
  await page.goto(`${origin}/settings/team`);
  const rows = page.locator('main ul > li');
  await rows.last().waitFor();
  assert.equal(await rows.count(), 23);
  const before = await rows.last().boundingBox();
  assert.ok(before && before.y > 888, 'last member should begin below the viewport');

  const main = page.locator('main');
  const box = await main.boundingBox();
  assert.ok(box);
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.wheel(0, 3000);
  await page.waitForFunction(() => {
    const main = document.querySelector('main');
    const last = main?.querySelector('ul > li:last-child');
    return main && last && main.scrollTop > 0 && last.getBoundingClientRect().bottom <= innerHeight;
  }, undefined, { timeout: 2000 });
  const after = await rows.last().boundingBox();
  const scrollTop = await main.evaluate(element => element.scrollTop);
  assert.ok(scrollTop > 0, 'team page must scroll within the fixed app shell');
  assert.ok(after && after.y >= box.y && after.y + after.height <= 888,
    `last member must be reachable, got ${JSON.stringify(after)}`);

  await page.setViewportSize({ width: 390, height: 844 });
  await page.reload();
  await rows.last().waitFor();
  await page.mouse.move(195, 420);
  await page.mouse.wheel(0, 5000);
  await page.waitForFunction(() => {
    const last = document.querySelector('main ul > li:last-child');
    return window.scrollY > 0 && last && last.getBoundingClientRect().bottom <= innerHeight;
  }, undefined, { timeout: 2000 });
  console.log(`PASS: all ${members.length} members are reachable on desktop and mobile`);
} finally {
  await browser.close();
}
