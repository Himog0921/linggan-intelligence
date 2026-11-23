// Read-only desktop acceptance on the isolated L1d snapshot fixtures (small and paged).
// node test/e2e/ops-content-lens.mjs ORIGINS_JSON EVIDENCE_DIR
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import path from 'node:path';
import { chromium } from 'playwright';

const [originsFile, output] = process.argv.slice(2);
const origins = JSON.parse(await fs.readFile(originsFile, 'utf8'));
await fs.mkdir(output, { recursive:true });
const browser = await chromium.launch();
const results = { viewport:{width:1440,height:1000}, states:[], checks:[], errors:[] };
const context = await browser.newContext({ viewport:results.viewport, deviceScaleFactor:1, reducedMotion:'reduce' });
const page = await context.newPage();
page.on('pageerror',error => results.errors.push(error.message));
let allowedOrigin;
await context.route('**/*',r => new URL(r.request().url()).origin === allowedOrigin && ['GET','HEAD'].includes(r.request().method())
  ? r.continue() : r.abort());
async function use(name) {
  const {origin,tokenFile} = origins[name], url=new URL(origin);
  assert.equal(url.hostname,'127.0.0.1'); assert.ok(!['3000','3001'].includes(url.port));
  allowedOrigin=origin;
  await context.clearCookies();
  await context.addCookies([{name:'linggan_ops_session',value:(await fs.readFile(tokenFile,'utf8')).trim(),url:origin}]);
  await page.goto(origin+'/ops/content?domain=all');
  await page.getByRole('main').waitFor();
}
async function shot(name, target) {
  await target.scrollIntoViewIfNeeded();
  await page.screenshot({path:path.join(output,name+'.png')});
  results.states.push(name);
}
async function data(endpoint) {
  const answer=await page.request.get(allowedOrigin+'/api/local/ops/content-lens/'+endpoint);
  assert.ok(answer.ok(),endpoint+' '+answer.status()); return answer.json();
}
async function compareList(trigger,snapshot,conditions,total) {
  const request = page.waitForResponse(r => new URL(r.url()).pathname === '/api/local/ops/content-lens/publications');
  await trigger.click(); const response=await request; assert.equal(response.status(),200);
  const dialog=page.getByRole('dialog'); await dialog.getByText('共 '+total+' 条发布记录').waitFor();
  const expected = await data('publications?'+new URLSearchParams({snapshotRef:snapshot.snapshotRef, metric:'publications',subset:'all',offset:'0',limit:'25',...conditions}));
  assert.equal(expected.total,total);
  for(const [key,value] of Object.entries(conditions)) assert.equal(new URL(response.url()).searchParams.get(key),value);
  assert.equal(new URL(response.url()).searchParams.get('snapshotRef'),snapshot.snapshotRef);
  const ids = await dialog.locator('[data-publication-ref]').evaluateAll(nodes=>nodes.map(n=>n.dataset.publicationRef));
  assert.deepEqual(ids, expected.items.map(r=>r.publicationRef));
  const rows = await dialog.locator('tbody tr').allTextContents();
  for (const [i,r] of expected.items.entries()) {
    assert.ok(rows[i].includes(r.title ?? '标题未取得'));
    assert.ok(rows[i].includes(String(r.views ?? '未填')));
  }
  if (total > expected.limit) {
    await dialog.getByRole('button',{name:'下一页'}).click();
    const tail=await data('publications?'+new URLSearchParams({snapshotRef:snapshot.snapshotRef,metric:'publications',subset:'all',limit:'25',offset:'25',...conditions}));
    await dialog.locator('[data-publication-ref="'+tail.items[0].publicationRef+'"]').waitFor();
    assert.deepEqual(await dialog.locator('[data-publication-ref]').evaluateAll(ns=>ns.map(n=>n.dataset.publicationRef)),tail.items.map(r=>r.publicationRef));
  }
  results.checks.push({snapshot:snapshot.snapshotRef,conditions,total,firstPageMembers:ids});
  await dialog.getByRole('button',{name:'关闭'}).click();
}
try {
  await use('disabled');
  await page.getByText('内容回溯未启用。').waitFor();
  assert.equal(await page.getByLabel('运营导航').getByRole('link',{name:'内容回溯'}).count(),0);
  await shot('01-disabled',page.getByText('内容回溯未启用。'));
  await use('empty'); await shot('02-empty',page.getByText(/暂无标注数据/));
  await use('active'); await shot('03-running',page.getByRole('progressbar'));
  assert.equal(await page.getByRole('progressbar').getAttribute('value'),'0.5'); assert.equal(await page.getByText('发布效果',{exact:true}).count(),0);
  await use('enabled');
  const small=(await data('snapshot')).snapshot;
  await page.getByLabel('运营导航').getByRole('link',{name:'内容回溯'}).waitFor();
  await shot('04-low-agreement',page.getByText(/复标一致率不足/));
  await page.locator('summary').filter({hasText:'未达标'}).waitFor();
  await shot('05-stale',page.getByText(/数据陈旧/));
  const momentName=small.structure.moment.find(r=>r.key==='homework_study').name;
  const offerName=small.structure.offer.find(r=>r.key==='actionable_method').name;
  await compareList(page.getByRole('button',{name:offerName+'，2 篇',exact:true}),small,
    {offer:'actionable_method',metric:'publications',subset:'all'},2);
  await compareList(page.getByRole('button',{name:momentName+' × '+offerName+'，2 篇',exact:true}),small,
    {moment:'homework_study',offer:'actionable_method',metric:'publications',subset:'all'},2);
  await page.getByRole('navigation',{name:'效果维度'}).getByRole('button',{name:'选题内容'}).click();
  const smallRow=page.getByRole('row').filter({has:page.getByRole('button',{name:momentName+'过千率分子 1',exact:true})});
  await shot('06-small-denominator',smallRow);
  assert.ok(await smallRow.getByText('样本不足',{exact:true}).count()>0);
  for(const [word,subset] of [['分子','numerator'],['分母','denominator']]) {
    await compareList(smallRow.getByRole('button',{name:momentName+'过千率'+word+' 1'}),small,
      {moment:'homework_study',metric:'overThousand',subset},1);
  }
  await page.route('**/api/local/ops/content-lens/snapshot*',r=>r.abort());
  await page.reload();
  await shot('07-read-error',page.getByText(/快照读取失败/));
  assert.equal(await page.getByText('内容结构',{exact:true}).count(),0);
  await page.unroute('**/api/local/ops/content-lens/snapshot*');
  await page.getByRole('button',{name:'重试'}).click(); await page.getByText('内容结构',{exact:true}).waitFor();
  await use('enough');
  const good=(await data('snapshot')).snapshot;
  await page.getByRole('navigation',{name:'效果维度'}).getByRole('button',{name:'选题内容'}).click();
  const result=good.effect.byDimension.moment.rows.find(r=>r.midRate.denominator===25);
  const name=good.structure.moment.find(r=>r.key===result.key).name;
  const enoughRow=page.getByRole('row').filter({has:page.getByRole('button',{name:name+'过千率分子 13',exact:true})});
  const pct=v=>new Intl.NumberFormat('zh-CN',{style:'percent',maximumFractionDigits:1}).format(v);
  assert.ok((await enoughRow.textContent()).includes(pct(result.midRate.value)));
  assert.ok((await enoughRow.textContent()).includes('区间 '+pct(result.midRate.low)+'–'+pct(result.midRate.high)));
  await shot('08-rate-interval',enoughRow);
  const header=await page.getByRole('columnheader',{name:'过千率',exact:true}).boundingBox();
  assert.ok(header && header.y >= 0 && header.y + header.height <= results.viewport.height);
  for (const [word,subset,n] of [['分子','numerator',13],['分母','denominator',25]]) {
    await compareList(enoughRow.getByRole('button',{name:name+'过千率'+word+' '+n}),good,{moment:result.key,metric:'overThousand',subset},n);
  }
  await compareList(page.getByRole('button',{name:'查看有标题的发布记录'}),good,{metric:'titledPublications',subset:'all'},26);
  await page.getByRole('button',{name:'查看人工抽看样本'}).click();
  await page.getByRole('dialog').getByRole('table').waitFor();
  await page.getByRole('dialog').getByRole('button',{name:'关闭'}).click();
  assert.deepEqual(results.errors,[]);
  console.log('Seven desktop states, independent members, 13/25 interval, pagination and retry verified.');
} finally {
  await fs.writeFile(path.join(output,'desktop-results.json'),JSON.stringify(results,null,2));
  await browser.close();
}
