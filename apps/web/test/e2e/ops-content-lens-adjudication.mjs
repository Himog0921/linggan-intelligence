// Run against a disposable seeded PostgreSQL copy. This test records two real adjudications.
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import { chromium } from 'playwright';

const [urlFile, tokenFile] = process.argv.slice(2);
const url = new URL((await fs.readFile(urlFile, 'utf8')).trim());
assert.equal(url.hostname, '127.0.0.1');
assert.equal(url.protocol, 'http:');
assert.ok(url.port && !['3000', '3001'].includes(url.port));
const base = '/api/local/ops/content-lens/';
const browser = await chromium.launch();
try {
  const context = await browser.newContext({viewport:{width:1440,height:1000}});
  await context.route('**/*', route => {
    const request = route.request();
    const target = new URL(request.url());
    const allowed = target.origin === url.origin && (['GET','HEAD'].includes(request.method())
      || (target.pathname === base + 'adjudications' && request.method() === 'POST'));
    return allowed ? route.continue() : route.abort();
  });
  await context.addCookies([{name:'linggan_ops_session', value:(await fs.readFile(tokenFile,'utf8')).trim(), url:url.origin}]);
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', error => errors.push(error.name));
  async function get(endpoint) {
    const response = await page.request.get(url.origin + base + endpoint);
    assert.ok(response.ok(), endpoint.split('?')[0] + ' returned ' + response.status());
    return response.json();
  }
  const before = (await get('snapshot')).snapshot;
  assert.ok(before, 'Seed the all-domain snapshot before this test.');
  const snapshotRef = before.snapshotRef;
  const sample = await get('spot-check?' + new URLSearchParams({snapshotRef}));
  const index = sample.items.findIndex(item => item.title && Object.keys(item.effective).every(dim => item.effective[dim] === item[dim]));
  assert.ok(index >= 0, 'The seed needs a visible title without a correction.');
  const item = sample.items[index];
  const from = item.effective.offer;
  const publications = (snapshot,key) => snapshot.structure.offer.find(row => row.key === key)?.publications ?? 0;
  const to = sample.vocabulary.offer.filter(choice => choice.key !== from)
    .sort((a,b) => publications(before,a.key) - publications(before,b.key))[0];
  assert.ok(to);
  await page.goto(url.origin + '/ops/content?domain=all');
  await page.getByRole('button',{name:'查看人工抽看样本'}).click();
  const dialog = page.getByRole('dialog');
  for (let i = 0; i < index; i++) await dialog.getByRole('button',{name:'下一条'}).click();
  const title = dialog.getByRole('heading',{name:item.title, exact:true});
  await title.waitFor();
  await dialog.getByLabel('裁定维度').selectOption('offer');
  async function submit(action) {
    const receipt = page.waitForResponse(response => new URL(response.url()).pathname === base + 'adjudications');
    await dialog.getByRole('button',{name:action}).click();
    const response = await receipt;
    assert.equal(response.status(),200, 'Adjudication HTTP status');
    const json = await response.json();
    await dialog.getByText('裁定已记录。').waitFor();
    await title.waitFor();
    return json.snapshotRef;
  }
  const confirmed = await submit('确认当前分类');
  const afterConfirmation = (await get('snapshot')).snapshot;
  assert.equal(afterConfirmation.snapshotRef,confirmed);
  if (before.quality.correctedTitles != null) {
    assert.equal(afterConfirmation.quality.correctedTitles,before.quality.correctedTitles);
  }
  await dialog.getByLabel('裁定分类').selectOption(to.key);
  const corrected = await submit('保存改正');
  assert.notEqual(confirmed,corrected);
  const after = (await get('snapshot')).snapshot;
  assert.equal(after.snapshotRef,corrected);
  assert.equal(after.quality.correctedTitles,afterConfirmation.quality.correctedTitles + 1);
  const moved = publications(before,from) - publications(after,from);
  assert.ok(moved > 0);
  assert.equal(publications(after,to.key) - publications(before,to.key), moved);
  const row = dialog.getByRole('row').filter({has:page.getByRole('rowheader',{name:'内容内核',exact:true})});
  await row.getByText('已记录',{exact:true}).waitFor();
  const cells = await row.getByRole('cell').allTextContents();
  assert.deepEqual(cells,[sample.vocabulary.offer.find(choice => choice.key === from).name,to.name,'已记录']);
  await dialog.getByRole('button',{name:'关闭'}).click();
  await page.getByText('已改正 ' + after.quality.correctedTitles + ' 个标题').waitFor();
  const count = publications(after,to.key);
  const read = page.waitForResponse(response => new URL(response.url()).pathname === base + 'publications');
  await page.getByRole('button',{name:to.name + '，' + count + ' 篇',exact:true}).click();
  const detail = await read;
  assert.equal(detail.status(),200);
  const drill = await detail.json();
  assert.equal(drill.snapshotRef,corrected);
  assert.equal(drill.total,count);
  await dialog.getByText('共 ' + count + ' 条发布记录').waitFor();
  assert.deepEqual(errors,[]);
  console.log(JSON.stringify({samples:sample.items.length, movedPublications:moved,
    correctedTitles:after.quality.correctedTitles, drillTotal:count, writes:2, pageErrors:errors.length}));
} finally {
  await browser.close();
}
