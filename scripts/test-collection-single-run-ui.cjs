/* Formal populated UI proof. It never navigates to the reference Demo or an external host. */
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
const { chromium } = require('playwright');
(async () => {
  const directory = process.env.COLLECTION_PROOF_DIR;
  const fixture = JSON.parse(await fs.readFile(path.join(directory, 'fixture.json'), 'utf8'));
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext();
  const page = await context.newPage();
  const visualMeasurements = [];
  const takeScreenshot = page.screenshot.bind(page);
  page.screenshot = async options => {
    const measurement = await page.evaluate(() => {
      const selectors = ['.c-run-toolbar', '.c-run-scroll', '.c-run-table', '.c-run-table th', '.c-run-table td', 'dialog[open]', 'dialog[open] h2', 'dialog[open] footer', 'dialog[open] input:not([type=hidden])', 'dialog[open] select', 'dialog[open] button', '.c-run-readouts', '.c-domain-list-row', '.c-stn-row', '.c-tg-item'];
      return {viewport: {width: innerWidth, height: innerHeight}, documentWidth: document.documentElement.scrollWidth, elements: selectors.flatMap(selector => [...document.querySelectorAll(selector)].filter(element => element.getClientRects().length).slice(0, 8).map(element => {
        const box = element.getBoundingClientRect(); const style = getComputedStyle(element);
        return {selector, text: element.textContent.trim().slice(0, 80), x: box.x, y: box.y, width: box.width, height: box.height, font: style.fontFamily, size: style.fontSize, weight: style.fontWeight, lineHeight: style.lineHeight, color: style.color, background: style.backgroundColor, padding: style.padding, border: style.borderTopWidth, scrollWidth: element.scrollWidth, clientWidth: element.clientWidth};
      }))};
    });
    const result = await takeScreenshot(options);
    visualMeasurements.push({file: path.basename(options.path), ...measurement});
    await fs.writeFile(path.join(directory, 'visual-measurements.json'), JSON.stringify(visualMeasurements, null, 2));
    if (options.path.includes('targets-1440') || options.path.includes('stations-1440')) await fs.writeFile(options.path.replace('.png', '.html'), await page.content());
    return result;
  };
  const errors = []; page.on('pageerror', e => errors.push(e.message)); const writes=[];page.on('request',r=>{if(r.method()==='POST')writes.push(new URL(r.url()).pathname);});
  await context.route('**/*', route => new URL(route.request().url()).origin === fixture.origin ? route.continue() : route.abort());
  async function tasksSettled() {
    await page.locator('.c-run-table tbody tr').first().waitFor();
    await page.waitForFunction(()=>!document.querySelector('.c-run-table')?.textContent.includes('正在读取采集记录'));
    assert(!(await page.locator('.c-run-table').innerText()).includes('任务读取失败'),'the populated fixture must be readable');
  }
  const nativeGoto=page.goto.bind(page);
  page.goto=async (url,...options)=>{const response=await nativeGoto(url,...options);if(new URL(url).pathname==='/collection/tasks')await tasksSettled();return response;};
  const reports = [];
  try {
    for (const width of [1440, 1920, 390]) {
      await page.setViewportSize({ width, height: 1000 });
      await page.goto(`${fixture.origin}/collection/tasks?domain=${fixture.domainRef}`);
      await page.locator('[data-new-run]').waitFor(); await page.locator('.c-run-table tbody tr').nth(19).waitFor();
      assert.equal(await page.locator('.c-run-table tbody tr').count(), 20);
      assert((await page.locator('.c-run-pagination').innerText()).includes('27'));
      await page.screenshot({ path: path.join(directory, `tasks-${width}.png`), fullPage: true });
      await page.locator('[data-new-run]').click();
      const form = page.locator('#collection-run-form'); await form.waitFor({ state: 'visible' });
      await page.waitForFunction(id=>document.querySelector('#collection-run-form [name=domain]')?.value===id,fixture.domainRef);
      assert.equal(await form.locator('[name=domain]').inputValue(), fixture.domainRef);
      assert.equal(await form.locator('[name=work_limit]').inputValue(), '50');
      await page.screenshot({ path: path.join(directory, `oneoff-${width}.png`) });
      if (width === 1440) {
        await form.locator('[name=source][value=observation_target]').check();
        await form.locator('[name=target] option').nth(1).waitFor({state:'attached'});
        assert.equal(await form.locator('[data-keyword-field]').isVisible(), false);
        await page.screenshot({path:path.join(directory,'oneoff-existing-1440.png')});
        await form.locator('[name=source][value=temporary_keyword]').check();
      }
      const footer = await form.locator('footer').boundingBox(); assert(footer && footer.y + footer.height <= 1000);
      await page.keyboard.press('Escape'); assert(await page.locator('[data-new-run]').evaluate(b => b === document.activeElement));
      reports.push(`populated tasks, defaults, footer and focus at ${width}`);
    }
    assert.equal(writes.length,0,'browsing and opening/closing the form must not issue a collection write');
    await page.setViewportSize({width:1440,height:1000});
    await page.goto(`${fixture.origin}/collection/tasks?domain=${fixture.domainRef}`);
    let releaseRead;const readGate=new Promise(resolve=>{releaseRead=resolve;});
    let readStarted;const started=new Promise(resolve=>{readStarted=resolve;});
    let readSettled;const settled=new Promise(resolve=>{readSettled=resolve;});
    await page.route('**/api/collection/runs/*?domainRef=*',async route=>{const response=await route.fetch();readStarted();await readGate;try{await route.fulfill({response});}finally{readSettled();}});
    await page.locator('[data-open-run]').first().waitFor();await page.locator('[data-open-run]').first().click();await started;
    await page.locator('#collection-run-drawer[open]').waitFor({state:'visible'});await page.keyboard.press('Escape');await page.locator('#collection-run-drawer').waitFor({state:'hidden'});
    releaseRead();await settled;
    await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
    assert.equal(new URL(page.url()).searchParams.has('run'),false,'a late read must not restore a closed drawer into the page context');
    await page.unroute('**/api/collection/runs/*?domainRef=*');
    await page.setViewportSize({width:1440,height:1000});
    await page.goto(`${fixture.origin}/collection/tasks?domain=${fixture.domainRef}&page=2`); await page.locator('.c-run-table tbody tr').nth(6).waitFor();
    assert.equal(await page.locator('.c-run-table tbody tr').count(), 7);
    for(const [filter,total] of [['all',27],['active',24],['attention',1],['results',1],['ended',3]]) {
      await page.goto(`${fixture.origin}/collection/tasks?domain=${fixture.domainRef}&filter=${filter}`);
      const result=await (await page.request.get(`${fixture.origin}/api/collection/runs?domainRef=${fixture.domainRef}&filter=${filter}`)).json();assert.equal(result.total,total);await page.waitForFunction(n=>document.querySelectorAll('.c-run-table tbody tr').length===n,Math.min(total,20));assert.equal(await page.locator('.c-run-table tbody tr').count(),Math.min(total,20));
      if(filter==='results')await page.screenshot({path:path.join(directory,'tasks-results-1440.png')});
    }
    await page.goto(`${fixture.origin}/collection/tasks?domain=${fixture.domainRef}&filter=attention`);await page.locator('[data-open-run]').first().click();await page.locator('[data-run-detail]').filter({hasText:'需要处理'}).waitFor();await page.screenshot({path:path.join(directory,'blocked-drawer-1440.png')});
    await page.keyboard.press('Escape');await page.locator('.c-run-tools [name=q]').fill('合成采集 01');await page.locator('.c-run-tools [type=submit]').click();await page.waitForURL(/q=/);await tasksSettled();assert.equal(new URL(page.url()).searchParams.get('filter'),'attention');assert.equal(await page.locator('.c-run-table tbody tr').count(),1);
    await page.goto(`${fixture.origin}/collection/tasks?domain=${fixture.domainRef}&q=${encodeURIComponent('超长合成')}`);assert.equal(await page.locator('.c-run-table tbody tr').count(),1);await page.locator('[data-open-run]').first().click();await page.locator('[data-run-detail]').filter({hasText:'人工暂停'}).waitFor();await page.setViewportSize({width:390,height:1000});await page.screenshot({path:path.join(directory,'paused-long-drawer-390.png')});await page.setViewportSize({width:1440,height:1000});
    await page.locator('.c-run-complete-name summary').click();assert((await page.locator('.c-run-complete-name p').innerText()).length>40);await page.screenshot({path:path.join(directory,'long-name-complete-1440.png')});await page.locator('[data-run-execution-diagnostics] > summary').click();await page.locator('[data-run-execution-diagnostics]').filter({hasText:'完整执行记录：共'}).waitFor();assert.equal(await page.locator('.c-run-complete-name section').count(),0);
    await page.goto(`${fixture.origin}/collection/tasks?domain=${fixture.otherDomainRef}`); await page.locator('.c-run-empty').filter({hasText:'没有采集记录'}).waitFor();
    assert((await page.locator('.c-run-empty').innerText()).includes('没有采集记录'));
    await page.locator('[data-new-run]').click();
    await page.locator('[name=query]').fill('浏览器真实申请');
    let lostIdentity;
    await page.route('**/api/collection/runs', async route => {
      if(route.request().method()!=='POST')return route.continue();
      lostIdentity=route.request().postDataJSON().idempotencyKey;
      await route.fetch(); await route.abort();
    });
    await page.locator('#collection-run-form [type=submit]').click();
    await page.getByRole('button',{name:'核实原请求回执',exact:true}).waitFor();
    assert.equal(await page.locator('[name=query]').inputValue(), '浏览器真实申请');
    const failedDraft=await page.evaluate(domain => JSON.parse(sessionStorage.getItem('collection-run-draft:'+domain)),fixture.otherDomainRef);
    assert.equal(failedDraft.request.identity,lostIdentity);
    assert.equal(failedDraft.request.sent,true);
    const postsBeforeRecovery=writes.filter(p=>p==='/api/collection/runs').length;
    await page.unroute('**/api/collection/runs');
    await page.getByRole('button',{name:'核实原请求回执',exact:true}).click();
    await page.waitForURL(/run=/); assert.equal(writes.filter(p=>p==='/api/collection/runs').length,postsBeforeRecovery,'recover by receipt without a second POST'); await page.locator('#collection-run-drawer').waitFor({state:'visible'});
    await page.locator('#collection-run-title').filter({hasText:'浏览器真实申请'}).waitFor();
    await page.screenshot({path:path.join(directory,'run-drawer.png')});
    await page.locator('#collection-run-drawer [data-close-run-dialog]').click();
    await page.goto(`${fixture.origin}/collection/tasks?domain=${fixture.domainRef}&run=${fixture.runRef}`);
    await page.locator('[data-run-detail]').filter({hasText:'可检索 150'}).waitFor();
    await page.screenshot({path:path.join(directory,'progress-drawer-1440.png')});
    await page.locator('.c-run-tabs [data-run-view=results]').click();
    await page.locator('.c-run-results [data-run-works] > p').first().waitFor();assert.equal(await page.locator('.c-run-results [data-run-works] > p').count(),5);
    await page.screenshot({path:path.join(directory,'results-drawer-1440.png')});
    await page.setViewportSize({width:390,height:1000});await page.screenshot({path:path.join(directory,'results-drawer-390.png')});await page.setViewportSize({width:1440,height:1000});
    await page.locator('.c-run-tabs [data-run-view=progress]').click();
    await page.locator('[data-run-execution-diagnostics] > summary').click();
    await page.locator('[data-run-detail]').filter({hasText:'完整执行记录：共 21 条'}).waitFor();
    await page.locator('.c-run-tabs [data-run-view=results]').click();
    await page.locator('[data-run-detail] a[href^="/corpus/evidence"]').first().click();
    await page.locator('input[data-collection-work-select]').first().waitFor();
    assert.equal(await page.locator('input[data-collection-work-select]').count(),5);
    const scoped=await page.request.get(`${fixture.origin}/api/local/work-resources?domain=${fixture.domainRef}&collectionRunRef=${fixture.runRef}`);
    assert.equal(scoped.status(),200);const materials=await scoped.json();assert.equal(materials.items.length,5);
    const firstRef=materials.items[0].identity.publicRef;
    const comments=await page.request.get(`${fixture.origin}/api/local/work-resources/${firstRef}/comments?domain=${fixture.domainRef}&collectionRunRef=${fixture.runRef}`);
    const commentPage=await comments.json();assert.equal(commentPage.total,30);assert.equal(commentPage.returned,20);assert(commentPage.nextCursor);
    const second=await page.request.get(`${fixture.origin}/api/local/work-resources/${firstRef}/comments?domain=${fixture.domainRef}&collectionRunRef=${fixture.runRef}&cursor=${commentPage.nextCursor}`);assert.equal((await second.json()).returned,10);
    const foreign=await page.request.get(`${fixture.origin}/api/local/work-resources?domain=${fixture.otherDomainRef}&collectionRunRef=${fixture.runRef}`);assert.equal((await foreign.json()).items.length,0);
    await page.screenshot({path:path.join(directory,'corpus-run-1440.png'),fullPage:true});
    await page.goto(`${fixture.origin}/corpus/evidence?domain=${fixture.domainRef}&collectionRunRef=${fixture.selectionRunRef}`);
    await page.locator('input[data-collection-work-select]').first().waitFor();
    assert.equal(await page.locator('input[data-collection-work-select]').count(),50);
    await page.getByRole('button',{name:'勾选本页',exact:true}).click();
    await page.locator('#ev-next-list').click();
    await page.locator('input[data-collection-work-select]').nth(50).waitFor();
    await page.getByRole('button',{name:'勾选本页',exact:true}).click();
    await page.screenshot({path:path.join(directory,'corpus-selection-1440.png')});
    await page.reload();await page.locator('input[data-collection-work-select]:checked').first().waitFor();assert.equal(await page.locator('input[data-collection-work-select]:checked').count(),50);
    // A previous accepted request can remain unconfirmed in the browser draft.
    // Entering a different exact scope must never resolve that old receipt as the new request.
    await page.evaluate(({domainRef,existingRequestKey}) => {
      const oldIntent={domainRef,source:{kind:'temporary_keyword',query:'合成有成果任务'},ranking:'comprehensive',workLimit:5,commentMode:'sample_30'};
      sessionStorage.setItem('collection-run-draft:'+domainRef,JSON.stringify({data:{source:'temporary_keyword',query:'合成有成果任务',domain:domainRef,ranking:'comprehensive',work_limit:'5',comment_mode:'sample_30'},requestKey:existingRequestKey,requestIntent:JSON.stringify(oldIntent)}));
    },fixture);
    await page.getByRole('button',{name:'评论深采',exact:true}).click();
    await page.locator('#collection-run-form').waitFor({state:'visible'});assert.equal(await page.locator('[name=work_limit]').inputValue(),'51');assert.equal(await page.locator('[name=comment_mode]').inputValue(),'all_public');
    assert.equal(await page.locator('[data-keyword-field]').isVisible(),false);assert.equal(await page.locator('[data-ranking-field]').isVisible(),false);
    const selectionBody=page.waitForRequest(r=>r.method()==='POST'&&r.url().endsWith('/api/collection/runs'));
    await page.screenshot({path:path.join(directory,'oneoff-selected-1440.png')});
    await page.locator('#collection-run-form [type=submit]').click();const body=(await selectionBody).postDataJSON();assert.equal(body.source.kind,'selected_works');assert.equal(new Set(body.source.workRefs).size,51);assert.equal(body.ranking,null);assert.notEqual(body.idempotencyKey,fixture.existingRequestKey);
    await page.waitForURL(/run=/);
    await page.goto(`${fixture.origin}/collection/tasks?domain=${fixture.legacyDomainRef}&filter=results&run=${fixture.legacyRunRef}`);
    await page.locator('[data-run-detail]').filter({hasText:'可用详情'}).waitFor();
    await page.locator('.c-run-tabs [data-run-view=results]').click();
    const legacyWorks=page.locator('.c-run-results [data-run-works] > p');
    await legacyWorks.filter({hasText:'合成历史已取得详情'}).waitFor();
    assert.equal(await legacyWorks.count(),1);
    assert((await legacyWorks.innerText()).includes('历史遍历完成证明未知；已取得材料可查看'));
    assert(!(await legacyWorks.innerText()).includes('等待执行与交付'));
    await page.screenshot({path:path.join(directory,'legacy-results-1440.png')});
    const legacyList=await page.request.get(`${fixture.origin}/api/collection/runs?domainRef=${fixture.legacyDomainRef}&filter=results`);
    const legacyRows=await legacyList.json();assert.equal(legacyRows.total,1);assert.equal(legacyRows.items[0].available_details,1);
    const legacyMaterials=await page.request.get(`${fixture.origin}/api/local/work-resources?domain=${fixture.legacyDomainRef}&collectionRunRef=${fixture.legacyRunRef}`);
    assert.equal(legacyMaterials.status(),200);assert.equal((await legacyMaterials.json()).items.length,1);
    reports.push('legacy accepted detail remains readable with unknown traversal proof, never labelled awaiting delivery');
    assert.equal(errors.length,0,errors.join('\n'));
    await fs.writeFile(path.join(directory,'browser-results.json'),JSON.stringify({reports,errors,verified:['populated React tasks, pagination and domain isolation','lost real create response recovered by durable receipt without another POST','selected works exact scope across Corpus pages','legacy material and current run detail/results/diagnostics'],notVerified:['real platform','all-public browser collection','all states visual acceptance']},null,2));
    console.log('formal populated collection browser proof passed');
  } catch(error) {
    await page.screenshot({path:path.join(directory,'failure.png'),fullPage:true}).catch(()=>{});
    console.error('Formal UI failure',JSON.stringify({url:page.url(),errors,writes,feedback:await page.locator('[data-run-form-feedback]').textContent().catch(()=>null)}));
    throw error;
  } finally {await browser.close();}
})().catch(error => {console.error(error);process.exitCode=1;});
