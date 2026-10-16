#!/usr/bin/env node
// Canonical Axum + disposable PostgreSQL proof. This script refuses the two current-data hosts.
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
const { randomUUID } = require('node:crypto');
const { createRequire } = require('node:module');
const { chromium } = createRequire(path.resolve(__dirname, '../apps/web/package.json'))('playwright');

async function main() {
  const directory = process.env.COLLECTION_PROOF_DIR;
  assert(directory, 'isolated fixture directory is required');
  const fixture = JSON.parse(await fs.readFile(path.join(directory, 'fixture.json'), 'utf8'));
  const origin = new URL(fixture.origin);
  assert(['127.0.0.1', 'localhost'].includes(origin.hostname), 'loopback isolated fixture only');
  assert(origin.port && !['3000', '3107'].includes(origin.port), 'never mutate production or read-only parity preview');
  const endpoint = suffix => `${origin.origin}${suffix}`;
  const contextReply = await fetch(endpoint('/api/collection/commands/context'));
  assert.equal(contextReply.status, 200);
  const { epoch } = await contextReply.json();
  const reports = [], errors = [], external = [], posts = [];
  async function get(surface) {
    const response = await fetch(endpoint(`/api/collection/workspace/${surface}`));
    assert.equal(response.status, 200); const value = await response.json();
    assert.equal(value.readable, true); return value;
  }
  async function command(action, fields, requestRef = randomUUID(), expected = 'applied') {
    const response = await fetch(endpoint(`/api/collection/commands/${action}`), {
      method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ requestRef, epoch, fields }),
    });
    const receipt = await response.json();
    assert.equal(receipt.requestRef, requestRef); assert.equal(receipt.action, action); assert.equal(receipt.outcome, expected, `${action}: ${JSON.stringify(receipt)}`);
    return receipt;
  }
  async function eventually(read, predicate) {
    for (let attempt = 0; attempt < 80; attempt++) {
      const value = await read(); if (predicate(value)) return value;
      await new Promise(resolve => setTimeout(resolve, 75));
    }
    assert.fail('persisted management read did not reach the expected state');
  }
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({ viewport: { width: 1440, height: 1000 } });
  await context.route('**/*', route => {
    const request = route.request(); const url = new URL(request.url());
    if (url.origin !== origin.origin) { external.push(url.origin); return route.abort(); }
    if (request.method() === 'POST') posts.push(url.pathname);
    return route.continue();
  });
  const page = await context.newPage(); page.on('pageerror', error => errors.push(error.message));
  async function submit(action, click, expected = 'applied') {
    const answer = page.waitForResponse(response => response.request().method() === 'POST' && new URL(response.url()).pathname === `/api/collection/commands/${action}`);
    await click(); const response = await answer; const receipt = await response.json();
    assert.equal(receipt.action, action); assert.equal(receipt.outcome, expected, JSON.stringify(receipt));
    return receipt;
  }
  const domainCard = name => page.locator('.domain-card').filter({ has: page.getByRole('heading', { name, exact: true }) });
  const stationRow = reference => page.locator(`[data-station-entry="${reference}"]`);
  async function manage(reference) {
    await stationRow(reference).locator('button.c-stn-manage').click();
    const modal = page.locator(`#station-manage-${reference}`); await modal.waitFor({ state: 'visible' }); return modal;
  }
  try {
    // Domain create/update/pause/resume are actual UI submissions and verified from a new read.
    await page.goto(endpoint(`/collection/domains?return_to=${encodeURIComponent(`/collection/tasks?domain=${fixture.domainRef}`)}`));
    await page.locator('.domain-create').waitFor();
    const create = page.locator('.domain-create');
    const initialName = `合成管理领域-${randomUUID().slice(0, 8)}`;
    await create.locator('[name=name]').fill(initialName);
    await create.locator('[name=description]').fill('隔离数据库中的管理页面证明');
    await create.locator('[name=research_goal]').fill('只验证本地领域与工位命令');
    await submit('domain-save', () => create.getByRole('button', { name: '创建领域', exact: true }).click());
    const created = await eventually(() => get('domains'), value => value.domains.some(domain => domain.name === initialName));
    const domain = created.domains.find(domain => domain.name === initialName); assert(domain);
    await domainCard(initialName).waitFor(); assert.equal(await create.locator('[name=name]').inputValue(), '');
    await create.locator('[name=name]').fill(initialName); await create.locator('[name=research_goal]').fill('重复名称草稿必须保留');
    const duplicate = await submit('domain-save', () => create.getByRole('button', { name: '创建领域', exact: true }).click(), 'rejected');
    assert.equal(duplicate.code, 'name_taken'); assert.equal(await create.locator('[name=research_goal]').inputValue(), '重复名称草稿必须保留');
    const finalName = `${initialName}-已编辑`; let card = domainCard(initialName);
    await card.locator('.domain-fields [name=name]').fill(finalName);
    await card.locator('.domain-fields [name=research_goal]').fill('更新后的隔离研究目标');
    await submit('domain-save', () => card.getByRole('button', { name: '保存领域设置', exact: true }).click());
    await domainCard(finalName).waitFor(); card = domainCard(finalName);
    for (const status of ['paused', 'active']) {
      await card.locator('.domain-fields [name=status]').selectOption(status);
      await submit('domain-save', () => card.getByRole('button', { name: '保存领域设置', exact: true }).click());
      await eventually(() => get('domains'), value => value.domains.find(item => item.domainRef === domain.domainRef)?.status === status);
    }
    const current = await get('domains'); const target = current.targets[0]; assert(target, 'long-term synthetic target is available');
    const add = card.locator('.domain-target-add');
    await add.locator('[name=target_ref]').selectOption(target.targetRef); await add.locator('[name=role]').selectOption('reference');
    await submit('domain-target', () => add.getByRole('button', { name: '关联目标', exact: true }).click());
    await eventually(() => get('domains'), value => value.relations.some(row => row.domainRef === domain.domainRef && row.targetRef === target.targetRef && row.role === 'reference'));
    const assigned = card.locator('tbody tr').filter({ hasText: target.name }); await assigned.waitFor();
    await assigned.locator('[name=role]').selectOption('primary');
    await submit('domain-target', () => assigned.getByRole('button', { name: '保存', exact: true }).click());
    await eventually(() => get('domains'), value => value.relations.some(row => row.domainRef === domain.domainRef && row.targetRef === target.targetRef && row.role === 'primary'));
    await submit('domain-target', () => assigned.getByRole('button', { name: '解除关联', exact: true }).click());
    await eventually(() => get('domains'), value => !value.relations.some(row => row.domainRef === domain.domainRef && row.targetRef === target.targetRef));
    reports.push('domain UI create/update/pause/resume/reference/primary/remove and explicit rejection draft retention');
    await page.screenshot({ path: path.join(directory, 'management-domains-1440.png'), fullPage: false });

    // Prove process identity replay cannot duplicate a local domain command or change its intent.
    const identity = randomUUID(); const replayName = `合成回执领域-${randomUUID().slice(0, 8)}`;
    const fields = { name: replayName, description: '', research_goal: '', status: 'active' };
    const first = await command('domain-save', fields, identity); const replay = await command('domain-save', fields, identity);
    assert.deepEqual(replay, first); assert.equal((await get('domains')).domains.filter(domain => domain.name === replayName).length, 1);
    const conflict = await command('domain-save', { ...fields, name: `${replayName}-冲突` }, identity, 'rejected');
    assert.equal(conflict.code, 'request_identity_conflict');
    assert.equal((await get('domains')).domains.filter(domain => domain.name === `${replayName}-冲突`).length, 0);
    reports.push('same identity replays one persisted domain; changed intent is rejected without a second domain');

    await page.goto(endpoint(`/collection/runtime?return_to=${encodeURIComponent(`/collection/tasks?domain=${fixture.domainRef}&run=${fixture.runRef}`)}`));
    await page.locator('.c-runtime').waitFor(); await page.getByRole('button', { name: '新增工位', exact: true }).click();
    const initialStation = `合成管理工位-${randomUUID().slice(0, 8)}`;
    await page.locator('#station-register [name=display_name]').fill(initialStation);
    await submit('runtime-register', () => page.locator('#station-register').getByRole('button', { name: '登记', exact: true }).click());
    const registered = await eventually(() => get('runtime'), value => value.projection.stations.some(station => station.name === initialStation));
    const station = registered.projection.stations.find(station => station.name === initialStation); assert(station);
    const duplicateStation = await command('runtime-register', { display_name: initialStation }, randomUUID(), 'rejected');
    assert.equal(duplicateStation.code, 'station_rejected');
    assert.equal((await get('runtime')).projection.stations.filter(item => item.name === initialStation).length, 1);
    let row = stationRow(station.stationRef); await row.waitFor();
    await row.getByRole('button', { name: `编辑 ${initialStation} 工位名称`, exact: true }).click();
    const finalStation = `${initialStation}-已更名`; await row.locator('[name=display_name]').fill(finalStation);
    await submit('runtime-rename', () => row.getByRole('button', { name: '保存', exact: true }).click());
    await eventually(() => get('runtime'), value => value.projection.stations.find(item => item.stationRef === station.stationRef)?.name === finalStation);
    await row.getByText(finalStation, { exact: true }).waitFor();
    let modal = await manage(station.stationRef);
    await modal.locator('[name=valid_for_hours]').selectOption('8');
    await submit('runtime-claim-window', () => modal.getByRole('button', { name: '开窗口', exact: true }).click());
    await modal.getByRole('button', { name: '现在就关掉认领窗口', exact: true }).waitFor();
    await submit('runtime-close-window', () => modal.getByRole('button', { name: '现在就关掉认领窗口', exact: true }).click());
    await modal.getByRole('button', { name: '开窗口', exact: true }).waitFor();
    await modal.getByRole('button', { name: '关闭', exact: true }).click();
    for (const [label, accepting] of [['恢复', true], ['暂停', false], ['恢复', true]]) {
      await submit('runtime-accepting', () => row.getByRole('button', { name: label, exact: true }).click());
      await eventually(() => get('runtime'), value => value.projection.stations.find(item => item.stationRef === station.stationRef)?.account?.accepting === accepting);
    }
    modal = await manage(fixture.activeStationRef);
    await submit('runtime-account-bindings', () => modal.getByRole('button', { name: '确认这个观察账号', exact: true }).click());
    await eventually(() => get('runtime'), value => value.projection.stations.find(item => item.stationRef === fixture.activeStationRef)?.account?.bindingState === 'current');
    await modal.getByRole('button', { name: '关闭', exact: true }).click();
    const unclaimed = page.locator('.c-stn-unclaimed'); if (await unclaimed.getAttribute('open') === null) await unclaimed.locator('summary').click();
    const installation = page.locator(`[data-installation-ref="${fixture.unclaimedRef}"]`); await installation.waitFor();
    await installation.locator('[name=station_ref]').selectOption(station.stationRef);
    await submit('runtime-claims', () => installation.getByRole('button', { name: '认领', exact: true }).click());
    await eventually(() => get('runtime'), value => !value.projection.unclaimed.some(item => item.installationRef === fixture.unclaimedRef));
    reports.push('station UI register/rename/window open/close/accepting pause/resume/account confirmation/installation claim');

    // The real server applies once; losing that HTTP reply must offer a GET receipt, never POST again.
    row = stationRow(station.stationRef); await row.getByRole('button', { name: `编辑 ${finalStation} 工位名称`, exact: true }).click();
    const recoveredName = `${initialStation}-回执核实`; await row.locator('[name=display_name]').fill(recoveredName);
    let dropped = 0;
    await page.route('**/api/collection/commands/runtime-rename', async route => { dropped++; await route.fetch(); await route.abort('failed'); });
    await row.getByRole('button', { name: '保存', exact: true }).click(); await row.getByText(/结果未确认/).waitFor();
    assert.equal(await row.getByRole('button', { name: '保存', exact: true }).isDisabled(), true);
    await row.getByRole('button', { name: '核实同一请求', exact: true }).click();
    await row.getByText(recoveredName, { exact: true }).waitFor(); assert.equal(dropped, 1);
    await page.unroute('**/api/collection/commands/runtime-rename');
    await eventually(() => get('runtime'), value => value.projection.stations.find(item => item.stationRef === station.stationRef)?.name === recoveredName);
    reports.push('lost real rename reply reconciles same request by GET and persists exactly once');
    await page.screenshot({ path: path.join(directory, 'management-runtime-1440.png'), fullPage: false });
    modal = await manage(station.stationRef);
    await submit('runtime-retire', () => modal.getByRole('button', { name: '停用这台工位', exact: true }).click());
    await eventually(() => get('runtime'), value => !value.projection.stations.some(item => item.stationRef === station.stationRef));
    reports.push('station retire removes the registered resource from a fresh read');

    const returnUrl = `/collection/tasks?domain=${fixture.domainRef}&run=${fixture.runRef}`;
    await page.goto(endpoint(`/collection/runtime?station=${fixture.activeStationRef}&return_to=${encodeURIComponent(returnUrl)}`));
    await page.waitForFunction(reference => document.activeElement?.getAttribute('data-station-entry') === reference, fixture.activeStationRef);
    assert.equal(await page.locator('[data-collection-management-return]').getAttribute('href'), returnUrl);
    for (const [width, height] of [[1280, 720], [1024, 768], [390, 844]]) {
      await page.setViewportSize({ width, height });
      await page.goto(endpoint('/collection/domains')); await page.locator('.domain-card').first().waitFor();
      await page.screenshot({ path: path.join(directory, `management-domains-${width}.png`), fullPage: false });
      await page.goto(endpoint('/collection/runtime')); await page.locator('.c-stn-row').first().waitFor();
      await page.screenshot({ path: path.join(directory, `management-runtime-${width}.png`), fullPage: false });
    }
    reports.push('source station focus/return query and management surfaces at 1440/1280/1024/390');
    assert.deepEqual(errors, []); assert.deepEqual(external, []);
    await fs.writeFile(path.join(directory, 'management-json-report.json'), JSON.stringify({ reports, errors, external, posts }, null, 2));
    console.log(`Collection management JSON/UI proof: ${reports.length} contracts; ${posts.length} isolated UI commands; no external requests.`);
  } finally { await browser.close(); }
}
main().catch(error => { console.error(error); process.exitCode = 1; });
