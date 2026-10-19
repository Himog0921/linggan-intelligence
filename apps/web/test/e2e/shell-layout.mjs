import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';

// Guards the visible navigation regressions previously covered by retired Rust CSS assertions.
export async function verifyShellLayout(browser, origin, out) {
  await mkdir(out, {recursive:true});
  const report = { views: [], errors: [], blocked: [] };
  for (const width of [1440,1280,1024,390]) {
    const context=await browser.newContext({viewport:{width,height:900},reducedMotion:'reduce'});
    const page=await context.newPage();
    page.on('pageerror',error=>report.errors.push(String(error)));
    await page.route('**/*',route=>{
      const request=route.request();
      if(new URL(request.url()).origin!==origin||!['GET','HEAD'].includes(request.method())){
        report.blocked.push(request.url());return route.abort();
      }
      return route.continue();
    });
    try {
      await page.goto(origin+'/ops/overview?domain=all');
      await page.locator('.v7-primary-nav').waitFor();
      await page.evaluate(()=>document.fonts.ready);
      const entries=await page.locator('.v7-primary-nav > :is(a,button)').evaluateAll(nodes=>nodes.map(node=>({name:node.querySelector('.v7-nav-zh')?.textContent,box:node.getBoundingClientRect().toJSON(),display:getComputedStyle(node).display})));
      assert.equal(entries.length,6);
      for(const entry of entries){assert.ok(entry.box.width>0&&entry.box.height>0,entry.name);assert.notEqual(entry.display,'none');assert.ok(entry.box.left>=-.5&&entry.box.right<=width+.5,`${width}: ${entry.name} outside viewport`);}
      const menu=await page.getByText('个人菜单',{exact:true}).boundingBox();
      assert.ok(menu&&menu.x>=0&&menu.x+menu.width<=width+.5,`${width}: personal menu outside viewport`);
      if(width===1440){
        const current=page.locator('.v7-side-nav[aria-current="page"]').first();await current.waitFor();
        const colors=()=>current.evaluate(node=>({background:getComputedStyle(node).backgroundColor,color:getComputedStyle(node).color}));
        const before=await colors();assert.notEqual(before.background,'rgba(0, 0, 0, 0)');assert.notEqual(before.background,'rgb(255, 255, 255)');
        await current.hover();assert.deepEqual(await colors(),before);
      }
      await page.screenshot({path:path.join(out,`shell-${width}.png`),fullPage:true});
      report.views.push({width,entries,menu});
    } finally {await context.close();}
  }
  await writeFile(path.join(out,'report.json'),JSON.stringify(report,null,2));
  assert.deepEqual(report.errors,[]);assert.deepEqual(report.blocked,[]);
  return report;
}
