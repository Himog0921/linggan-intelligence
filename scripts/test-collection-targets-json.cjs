/* WEB-REACT-001 G: real Router + isolated PostgreSQL target workspace proof.
 * Invoked only by collection_single_run_browser_with_persisted_data. Fixture is synthetic;
 * never point this script at the normal runtime or the read-only visual preview. */
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
const { createRequire } = require('node:module');
const { chromium } = createRequire(path.resolve('apps/web/package.json'))('playwright');
(async () => {
  const directory = process.env.COLLECTION_PROOF_DIR;
  assert(directory, 'COLLECTION_PROOF_DIR is required');
  const fixture = JSON.parse(await fs.readFile(path.join(directory, 'fixture.json'), 'utf8'));
  const origin = new URL(fixture.origin);
  assert(['127.0.0.1', 'localhost'].includes(origin.hostname));
  assert(!['3000', '3107'].includes(origin.port), 'this proof must never write to normal runtime/preview');
  assert(fixture.domainRef && fixture.otherDomainRef, 'two isolated synthetic domains required');
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({ viewport: { width: 1440, height: 1000 } });
  const page = await context.newPage(), errors = [], reports = [], writes = [];
  page.on('pageerror', error => errors.push(error.message));
  page.on('request', request => { if (request.method() === 'POST') writes.push(new URL(request.url()).pathname); });
  await context.route('**/*', route => new URL(route.request().url()).origin === origin.origin ? route.continue() : route.abort());
  async function get(url) {
    return page.evaluate(async url => { const r = await fetch(url); return { status: r.status, body: await r.json() }; }, url);
  }
  async function command(action, fields, ref) {
    return page.evaluate(async ({ action, fields, ref }) => {
      const context = await (await fetch('/api/collection/commands/context')).json();
      const r = await fetch(`/api/collection/commands/${action}`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ requestRef: ref ?? crypto.randomUUID(), epoch: context.epoch, fields }) });
      return { status: r.status, body: await r.json() };
    }, { action, fields, ref });
  }
  async function mutate(action, trigger, outcome = 'applied') {
    const wait = page.waitForResponse(r => new URL(r.url()).pathname === `/api/collection/commands/${action}` && r.request().method() === 'POST');
    await trigger(); const response = await wait, receipt = await response.json();
    assert.equal(receipt.action, action); assert.equal(receipt.outcome, outcome, JSON.stringify(receipt));
    assert.equal(typeof receipt.requestRef, 'string');
    return receipt;
  }
  async function targets(domain = fixture.domainRef) {
    const r = await get(`/api/collection/workspace/targets?domain=${domain}`); assert.equal(r.status, 200); return r.body;
  }
  async function goto(query) {
    await page.goto(`${origin.origin}/collection/targets?${new URLSearchParams(query)}`);
    await page.waitForFunction(() => !document.body.textContent.includes('正在读取观察目标') && !!document.querySelector('.c-tg-directory,.c-empty-known-view'));
  }
  async function create(kind, identity, domain) {
    await goto({ domain });
    await page.getByRole('button', { name: '＋ 新建目标', exact: true }).click();
    const modal = page.getByRole('dialog', { name: '新建观察目标' }); await modal.waitFor();
    await modal.locator('[name=target_kind]').selectOption(kind);
    await modal.locator('[name=identity]').fill(identity);
    await modal.locator('select[name=domain]').selectOption(domain);
    await mutate('new', () => modal.getByRole('button', { name: '建立目标', exact: true }).click());
    const list = await targets(domain), found = list.rows.find(r => r.identity === identity || r.name === identity);
    assert(found, `created ${kind} must be present in its explicit domain`); return found;
  }
  async function openRule(targetRef, slot, domain = fixture.domainRef) {
    await goto({ domain, rule: targetRef, ...(slot ? { rule_slot: slot } : {}) });
    await page.locator('.c-rule-form').waitFor();
  }
  try {
    await goto({ domain: fixture.domainRef });
    assert.equal(writes.length, 0, 'read-only list must not mutate');
    const initial = await targets();
    assert.equal(initial.limit, 200); assert(initial.rows.length > 0, 'proof requires persisted target rows');
    assert(initial.rows.every(row => row.recentNew === null || typeof row.recentNew === 'number'));
    const missing = await get('/api/collection/workspace/target/00000000-0000-4000-8000-999999999999');
    assert.equal(missing.status, 404);
    reports.push('persisted list, null/zero contract, independent missing-target 404');

    // V4 uses actual accepted packages, not a frontend response mock.
    const inspectionDomain=fixture.inspectionDomainRef;
    const inspectionList=await targets(inspectionDomain);
    const inspectionTarget=inspectionList.rows.find(row=>row.targetRef===fixture.inspectionTargetRef);
    assert(inspectionTarget,'the persistent V4 fixture must appear in the correct domain');
    const base=`/api/collection/workspace/target/${inspectionTarget.targetRef}/inspection`;
    const rounds=await get(`${base}/rounds?domain=${inspectionDomain}`);
    assert.equal(rounds.status,200);assert(rounds.body.rounds[0]);
    const round=rounds.body.rounds[0];
    const result=await get(`${base}/rounds/${round.roundRef}?domain=${inspectionDomain}`);
    assert.equal(result.status,200);assert.equal(result.body.works.length,2);
    const populated={target:inspectionTarget,base,round,works:result.body.works};
    assert(populated, 'V4 browser proof requires a real accepted round with at least two works');
    await goto({ domain: inspectionDomain, drawer: populated.target.targetRef, dtab: 'works', inspect_right_round: populated.round.roundRef });
    for(const work of populated.works.slice(0,2)){
      const row=page.getByRole('row').filter({has:page.getByRole('button',{name:work.title,exact:true})});
      await row.getByRole('button',{name:'加入对照',exact:true}).click();
      await row.getByRole('button',{name:'移出对照',exact:true}).waitFor();
    }
    assert.equal(await page.getByRole('button',{name:'移出对照',exact:true}).count(),2,'two distinct persisted works must be selected');
    await page.getByRole('button', { name: '并排对照', exact: true }).click();
    await page.getByRole('heading', { name: '作品并排对照', exact: true }).waitFor();
    await page.getByRole('button', { name: '当前累计', exact: true }).waitFor();
    const canonicalResponse = page.waitForResponse(response => response.url().includes('/api/local/work-resources/') && response.url().includes('/research-dossier?'));
    await page.getByRole('button', { name: '展开已取得的底稿', exact: true }).first().click();
    const canonical = await canonicalResponse; assert.equal(canonical.status(), 200);
    const canonicalPayload = await canonical.json(); assert.equal(canonicalPayload.dossier.contract, 'research-dossier.v2');
    const canonicalUrl = new URL(canonical.url()); assert.equal(canonicalUrl.searchParams.get('asOf'), new URL(page.url()).searchParams.get('inspect_as_of'));
    await page.getByText('以下按当前研究截点读取正式底稿。', { exact: true }).waitFor();
    await page.screenshot({ path: path.join(directory, 'targets-v4-canonical-dossier.png') });
    for(const width of [1440,1024,390]){
      await page.setViewportSize({width,height:900});
      const geometry=await page.locator('#c-drawer').evaluate(el=>({width:el.getBoundingClientRect().width,
        overflow:el.scrollWidth-el.clientWidth,headOverflow:el.querySelector('.c-dw-head').scrollWidth-el.querySelector('.c-dw-head').clientWidth,
        bodyOverflow:el.querySelector('.c-dw-body').scrollWidth-el.querySelector('.c-dw-body').clientWidth}));
      assert(Math.abs(geometry.width-(width<=760?width:Math.min(960,width-96)))<=1,JSON.stringify(geometry));
      assert(geometry.overflow<=1&&geometry.headOverflow<=1&&geometry.bodyOverflow<=1,'drawer stays bounded; comparison scrolls locally: '+JSON.stringify(geometry));
      await page.screenshot({path:path.join(directory,`targets-v4-compact-${width}.png`)});
    }
    await page.setViewportSize({width:1440,height:1000});
    await page.getByRole('button', { name: '发布后 24 小时', exact: true }).click();
    const v4Selected = populated.works.slice(0, 2).map(work => work.workRef).join(',');
    const age = await get(`${populated.base}/work-comparison?domain=${inspectionDomain}&works=${v4Selected}&mode=age_24h`);
    assert.equal(age.status, 200); assert.equal(age.body.mode, 'age_24h'); assert.equal(age.body.items.length, 2);
    assert(age.body.items.every(item => item.metrics.likes.value === null), 'fixture has no qualified 24-hour observation; latest values must not fill history');
    await page.getByRole('button', { name: '返回原列表或图表', exact: true }).click();
    await page.getByRole('button', { name: populated.works[0].title, exact: true }).click();
    await page.getByRole('heading', { name: '观察轨迹', exact: true }).waitFor();
    const trace = await get(`${populated.base}/works/${populated.works[0].workRef}/observations?domain=${inspectionDomain}`);
    assert.equal(trace.status, 200); assert(trace.body.observations.length > 0);
    assert.equal(writes.length, 0, 'rounds, comparison and trace do not collect or invoke');
    await page.screenshot({ path: path.join(directory, 'targets-v4-observation-trace.png') });
    reports.push('V4 React to Axum/PostgreSQL: real round, two works, canonical V2 at the same cutoff, age-24h unknown, trace and return without writes');

    const creatorId = 'ad0000000000000000000101';
    const creator = await create('creator', creatorId, fixture.domainRef);
    const keyword = await create('keyword', 'React迁移合成关键词G001', fixture.domainRef);
    const otherKeyword = await create('keyword', 'React迁移异领域合成关键词G001', fixture.otherDomainRef);
    assert(!(await targets()).rows.some(row => row.targetRef === otherKeyword.targetRef));
    assert(!(await targets(fixture.otherDomainRef)).rows.some(row => row.targetRef === creator.targetRef));
    reports.push('UI creates creator/keyword with explicit domain; scoped lists exclude other-domain targets');

    await goto({ domain: fixture.domainRef });
    const row = page.locator('.c-tg-item').filter({ has: page.locator(`#target-${creator.targetRef}`) });
    await row.locator('input[type=checkbox]').check();
    await page.getByRole('button', { name: /批量编辑/ }).click();
    const batch = page.getByRole('dialog', { name: '批量编辑观察目标' });
    await batch.locator('[name=group_name]').fill('合成分组G001');
    await mutate('batch', () => batch.getByRole('button', { name: '保存分组' }).click());
    assert.equal((await targets()).rows.find(r => r.targetRef === creator.targetRef).group, '合成分组G001');
    reports.push('UI batch edit obtains server receipt and changes only selected target group');

    await goto({ domain: fixture.domainRef, drawer: creator.targetRef, dtab: 'works', wview: 'performance', life_window: 'invalid-window' });
    await page.getByText('作品分布的查询条件无效', { exact: true }).waitFor();
    await page.screenshot({ path: path.join(directory, 'targets-invalid-lifecycle.png') });
    await goto({ domain: fixture.domainRef, drawer: creator.targetRef, dtab: 'works', wview: 'list' });
    await page.locator('.c-dw-catalog-summary').waitFor();
    assert((await page.locator('.c-dw-body').innerText()).includes('当前筛选没有匹配的作品'));
    reports.push('URL refresh preserves creator works subview; invalid lifecycle query remains invalid');

    // A newly stored keyword is not qualified for patrol, even when saving disabled rules.
    await openRule(keyword.targetRef, 'new');
    await mutate('rules', () => page.getByRole('button', { name: '保存新版本', exact: true }).click(), 'rejected');
    assert.equal((await get(`/api/collection/workspace/target/${keyword.targetRef}/rules`)).body.form.expected_revision, 0);
    reports.push('new keyword baseline gate rejects rule save without inventing archive completion');

    await openRule(creator.targetRef, 'new');
    await page.locator('[name=automatic_enabled]').uncheck();
    await page.locator('[name=creator_follow_details]').uncheck();
    await page.locator('#fixed_interval_seconds').selectOption('21600');
    await mutate('rules', () => page.getByRole('button', { name: '保存新版本', exact: true }).click());
    const disabledCreatorRule = (await get(`/api/collection/workspace/target/${creator.targetRef}/rules`)).body.form;
    assert.equal(disabledCreatorRule.automatic_enabled, false);
    assert.equal(disabledCreatorRule.creator_follow_details, false);
    await openRule(creator.targetRef);
    await mutate('rules', () => page.getByRole('button', { name: '恢复自动调度', exact: true }).click());
    assert.equal((await get(`/api/collection/workspace/target/${creator.targetRef}/rules`)).body.form.automatic_enabled, true);
    await openRule(creator.targetRef);
    await mutate('rules', () => page.getByRole('button', { name: '暂停未来自动调度', exact: true }).click());
    assert.equal((await get(`/api/collection/workspace/target/${creator.targetRef}/rules`)).body.form.automatic_enabled, false);
    reports.push('creator rule UI saves a fixed cadence, resumes and pauses with real version receipts');

    assert(fixture.legacyTargetRef && fixture.legacyDomainRef, 'qualified keyword fixture is required for multi-slot proof');
    const qualifiedRef = fixture.legacyTargetRef, qualifiedDomain = fixture.legacyDomainRef;
    const qualified = await get(`/api/collection/workspace/target/${qualifiedRef}?domain=${qualifiedDomain}`);
    assert.equal(qualified.body.target.kind, 'keyword'); assert.equal(qualified.body.target.archiveLabel, '档案已建立', 'accepted search baseline and every discovered detail must qualify');
    await openRule(qualifiedRef, 'new', qualifiedDomain);
    await page.locator('.c-rule-form select[name=ranking_key]').selectOption('most_liked');
    await page.locator('#fixed_interval_seconds').selectOption('21600');
    await mutate('rules', () => page.getByRole('button', { name: '保存新版本', exact: true }).click());
    let panel = await get(`/api/collection/workspace/target/${qualifiedRef}/rules`);
    assert.equal(panel.body.form.expected_revision, 1); assert.equal(panel.body.form.ranking_key, 'most_liked');
    assert.equal(panel.body.form.fixed_interval_seconds, '21600');
    reports.push('qualified keyword rule save preserves its ranking and fixed cadence');

    // Add a second independent ranking, then retire it from the real per-rule table.
    await openRule(qualifiedRef, 'new', qualifiedDomain);
    await page.locator('.c-rule-form select[name=ranking_key]').selectOption('latest');
    await mutate('rules', () => page.getByRole('button', { name: '保存新版本', exact: true }).click());
    let detail = await get(`/api/collection/workspace/target/${qualifiedRef}?domain=${qualifiedDomain}`);
    assert.equal(detail.body.rules.length, 2);
    await goto({ domain: qualifiedDomain, drawer: qualifiedRef, dtab: 'patrol' });
    const rules = page.locator('.c-dw-rule-table tbody tr'); await rules.nth(1).waitFor();
    await mutate('monitor-rules-retire', () => rules.nth(1).getByRole('button', { name: '停用', exact: true }).click());
    detail = await get(`/api/collection/workspace/target/${qualifiedRef}?domain=${qualifiedDomain}`);
    assert.equal(detail.body.rules.length, 1);
    reports.push('multiple ranking slots stay separate; per-rule retirement preserves remaining rule');

    const rejected = await command('rules', { target_ref: qualifiedRef, rule_slot: 'most_liked', expected_revision: 999, idempotency_key: '00000000-0000-4000-8000-999999999991', command_kind: 'pause' });
    assert.equal(rejected.body.outcome, 'rejected');
    reports.push('stale revision is rejected by server without being reported as applied');

    // Lose only the answer after the server has persisted a harmless group change. Recovery
    // must query the original request identity; it must never repeat the POST.
    await goto({ domain: fixture.domainRef });
    const selected = page.locator('.c-tg-item').filter({ has: page.locator(`#target-${creator.targetRef}`) });
    await selected.locator('input[type=checkbox]').check(); await page.getByRole('button', { name: /批量编辑/ }).click();
    await page.getByRole('dialog', { name: '批量编辑观察目标' }).locator('[name=group_name]').fill('回执核实合成分组');
    let lostRef = null, intercepted = 0;
    await page.route('**/api/collection/commands/batch', async route => { intercepted++; const r = await route.fetch(); lostRef = (await r.json()).requestRef; await route.abort('failed'); });
    await page.getByRole('button', { name: '保存分组', exact: true }).click();
    await page.getByRole('alertdialog', { name: '提交结果未确认', exact: true }).waitFor();
    const verifyButton = page.getByRole('button', { name: '核实本次请求', exact: true });
    await verifyButton.waitFor();
    assert(await page.getByRole('dialog', { name: '批量编辑观察目标' }).isVisible(), 'original modal stays mounted with draft');
    await page.waitForFunction(() => document.activeElement?.textContent === '核实本次请求');
    await page.keyboard.press('Tab');
    assert(await verifyButton.evaluate(el => document.activeElement === el), 'pending confirmation stays keyboard reachable above original modal trap');
    assert(lostRef); assert.equal(intercepted, 1);
    assert((await page.getByRole('alertdialog', { name: '提交结果未确认', exact: true }).getByRole('status').innerText()).includes(lostRef));
    await page.getByRole('button', { name: '核实本次请求', exact: true }).click();
    await page.waitForFunction(() => !document.body.textContent.includes('提交结果未确认'));
    assert.equal(intercepted, 1); await page.unroute('**/api/collection/commands/batch');
    assert.equal(await page.getByRole('dialog', { name: '批量编辑观察目标' }).locator('[name=group_name]').inputValue(), '回执核实合成分组', 'definitive GET receipt keeps original draft');
    assert.equal((await targets()).rows.find(r => r.targetRef === creator.targetRef).group, '回执核实合成分组');
    reports.push('lost POST answer keeps original requestRef; GET receipt confirms without second POST');

    // Deleting a newly stored control target exercises preview and explicit name confirmation.
    const preview = await get(`/api/collection/workspace/target/${creator.targetRef}/deletion`);
    assert.equal(preview.status, 200); assert.equal(preview.body.blocked, false); assert.equal(preview.body.retainedWorks, 0);
    await goto({ domain: fixture.domainRef, delete: creator.targetRef });
    const deletion = page.getByRole('dialog', { name: `彻底删除「${creator.name}」` }); await deletion.waitFor();
    await deletion.locator('[name=confirm_name]').fill('名字不匹配');
    await mutate('delete', () => deletion.getByRole('button', { name: '彻底删除', exact: true }).click(), 'rejected');
    assert((await targets()).rows.some(r => r.targetRef === creator.targetRef));
    await deletion.locator('[name=confirm_name]').fill(creator.name);
    await page.screenshot({ path: path.join(directory, 'targets-delete-preview.png') });
    await mutate('delete', () => deletion.getByRole('button', { name: '彻底删除', exact: true }).click());
    assert(!(await targets()).rows.some(r => r.targetRef === creator.targetRef));
    reports.push('deletion preview preserves counts; wrong confirmation rejected; exact name applies');

    await goto({ domain: fixture.domainRef });
    await page.setViewportSize({ width: 390, height: 844 });
    await page.screenshot({ path: path.join(directory, 'targets-react-390.png'), fullPage: false });
    assert.deepEqual(errors, []);
    await fs.writeFile(path.join(directory, 'targets-react-proof.json'), JSON.stringify({ reports, writes, errors }, null, 2));
    console.log(JSON.stringify({ targetProofs: reports.length, scriptErrors: errors.length }));
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exit(1); });
