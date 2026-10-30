import assert from 'node:assert/strict';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';

// document.fonts.ready also resolves after a failed download, so it cannot prove the fonts arrived. Ask every face
// the stylesheet declares to load through the served route and require each one to report "loaded".
async function verifyDeclaredFontsLoad(page) {
  const css = await readFile(new URL('../../src/styles/typography.css', import.meta.url), 'utf8');
  const declared = [...css.matchAll(/@font-face\s*\{[^}]*?font-family:\s*"([^"]+)"/g)].flatMap(match => match[1] ?? []);
  assert.ok(declared.length >= 6, `typography.css must declare the LIDS font faces, found ${declared.length}`);
  const faces = await page.evaluate(async () => {
    const all = [...document.fonts];
    await Promise.allSettled(all.map(face => face.load()));
    return all.map(face => ({ family: face.family.replace(/^["']|["']$/g, ''), status: face.status }));
  });
  for (const family of declared) {
    const face = faces.find(candidate => candidate.family === family);
    assert.ok(face, `${family}: declared in typography.css but absent from document.fonts`);
    assert.equal(face.status, 'loaded', `${family}: font face must load from the served /ui-assets route`);
  }
  return faces;
}

// Guards the visible navigation regressions previously covered by retired Rust CSS assertions.
export async function verifyShellLayout(browser, origin, out, storageState) {
  await mkdir(out, {recursive:true});
  const report = { views: [], errors: [], blocked: [] };
  const near = (actual, expected, message) => assert.ok(Math.abs(actual - expected) <= 1, `${message}: expected ${expected}, got ${actual}`);
  for (const width of [1440,1280,1024,390]) {
    const context=await browser.newContext({storageState,viewport:{width,height:900},reducedMotion:'reduce'});
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
      if(width===1440)report.fonts=await verifyDeclaredFontsLoad(page);
      await page.waitForFunction(() => {
        const app = document.querySelector('.v7-app');
        const header = document.querySelector('[data-v7-shell-header]');
        return Boolean(app && header && parseFloat(getComputedStyle(app).getPropertyValue('--v7-header-height')) > 0);
      });
      const sampleShell = (withDrawer) => page.evaluate((includeDrawer) => {
        const app = document.querySelector('.v7-app');
        const header = document.querySelector('[data-v7-shell-header]');
        const content = document.querySelector('.v7-shell');
        if (!(app instanceof HTMLElement && header instanceof HTMLElement && content instanceof HTMLElement)) throw new Error('shared shell geometry absent');
        const drawerNode = includeDrawer ? document.querySelector('#c-drawer') : null;
        return { header: header.getBoundingClientRect().toJSON(), content: content.getBoundingClientRect().toJSON(), measured: parseFloat(getComputedStyle(app).getPropertyValue('--v7-header-height')), drawer: drawerNode ? drawerNode.getBoundingClientRect().toJSON() : null };
      }, withDrawer);
      const nextFrames = () => page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
      // The root variable follows a header resize one frame later (ResizeObserver, then the next frame). A web font that
      // arrives late can re-wrap the header between "ready" and one sample, so sample until the header, the content start,
      // the variable and (when asked) the fixed drawer agree, then assert that very sample. A variable that never catches
      // up still fails: the last sample is asserted and reports the real numbers.
      const shellGeometry = async (kind, withDrawer = false) => {
        const settled = (g) => Math.abs(g.measured - g.header.bottom) <= 1 && Math.abs(g.content.top - g.header.bottom) <= 1
          && (!withDrawer || (g.drawer !== null && Math.abs(g.drawer.y - g.header.bottom) <= 1));
        let geometry = await sampleShell(withDrawer);
        for (let attempt = 0; attempt < 100 && !settled(geometry); attempt++) {
          await nextFrames();
          geometry = await sampleShell(withDrawer);
        }
        near(geometry.content.top, geometry.header.bottom, `${width}/${kind}: content must begin at actual header bottom`);
        near(geometry.measured, geometry.header.bottom, `${width}/${kind}: drawer variable must be actual header bottom`);
        return geometry;
      };
      const noContext = await shellGeometry('without-context-nav');
      const nav=page.locator('.v7-primary-nav');
      const entries=await nav.locator(':scope > :is(a,button)').evaluateAll(nodes=>nodes.map(node=>({name:node.querySelector('.v7-nav-zh')?.textContent,box:node.getBoundingClientRect().toJSON(),display:getComputedStyle(node).display})));
      assert.deepEqual(entries.map(entry=>entry.name),['雷达','主题图谱','语料','洞察','采集','运营'],'global navigation follows the six approved responsibilities; creation stays under operations');
      for(const entry of entries){assert.ok(entry.box.width>0&&entry.box.height>0,entry.name);assert.notEqual(entry.display,'none');}
      assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false,`${width}: global horizontal overflow`);
      let rail=null;
      if(width<820){
        rail=await nav.evaluate(node=>({clientWidth:node.clientWidth,scrollWidth:node.scrollWidth}));
        assert.ok(rail.scrollWidth>rail.clientWidth,`${width}: narrow primary rail must retain its local overflow`);
        const last=nav.locator(':scope > :is(a,button)').last();
        await last.scrollIntoViewIfNeeded();
        const box=await last.boundingBox();
        assert.ok(box&&box.x>=-.5&&box.x+box.width<=width+.5,`${width}: last primary entry is not reachable in its local rail`);
      }else for(const entry of entries){assert.ok(entry.box.left>=-.5&&entry.box.right<=width+.5,`${width}: ${entry.name} outside viewport`);}
      const menu=await page.getByText('个人菜单',{exact:true}).boundingBox();
      assert.ok(menu&&menu.x>=0&&menu.x+menu.width<=width+.5,`${width}: personal menu outside viewport`);
      if(width===1440){
        const current=page.locator('.v7-side-nav[aria-current="page"]').first();await current.waitFor();
        const colors=()=>current.evaluate(node=>({background:getComputedStyle(node).backgroundColor,color:getComputedStyle(node).color}));
        const before=await colors();assert.notEqual(before.background,'rgba(0, 0, 0, 0)');assert.notEqual(before.background,'rgb(255, 255, 255)');
        await current.hover();assert.deepEqual(await colors(),before);
      }
      await page.goto(origin+'/corpus/comments?domain=all');
      await page.locator('[data-comment-study-context]').waitFor();
      await page.waitForFunction(() => {
        const app = document.querySelector('.v7-app');
        const header = document.querySelector('[data-v7-shell-header]');
        return Boolean(app && header && parseFloat(getComputedStyle(app).getPropertyValue('--v7-header-height')) >= header.getBoundingClientRect().bottom - 1);
      });
      const withContext = await shellGeometry('with-context-nav');
      assert.equal(await page.locator('[data-comment-study-context]').count(), 1, `${width}: tertiary navigation missing`);

      await page.goto(origin+'/collection/targets?drawer=shell-geometry');
      const drawer = page.locator('#c-drawer');
      await drawer.waitFor();
      await page.waitForFunction(() => {
        const app = document.querySelector('.v7-app');
        const header = document.querySelector('[data-v7-shell-header]');
        return Boolean(app && header && parseFloat(getComputedStyle(app).getPropertyValue('--v7-header-height')) >= header.getBoundingClientRect().bottom - 1);
      });
      const collection = await shellGeometry('collection-drawer', true);
      const drawerBox = collection.drawer;
      assert.ok(drawerBox, `${width}: collection drawer absent from layout`);
      near(drawerBox.y, collection.header.bottom, `${width}: collection drawer must begin at actual header bottom`);
      if(width===390){
        // The isolated proof database has no collection rows, so this page cannot scroll by itself and there is no
        // list link to reopen the drawer from. Add inert height outside React's root instead: the real shared header,
        // document scroll and fixed portal drawer stay the things under test. The drawer's top comes from the same
        // variable however it was opened, so a stale variable shows up here exactly as it would after a reopen.
        await page.evaluate(()=>{
          const spacer=document.createElement('div');
          spacer.setAttribute('data-shell-geometry-spacer','');
          spacer.style.height='3000px';
          document.body.append(spacer);
        });
        const scrollY=await page.evaluate(()=>{
          window.scrollTo(0,document.documentElement.scrollHeight);
          return window.scrollY;
        });
        assert.ok(scrollY>0,'390: document must scroll while the collection drawer is open');
        await page.waitForFunction(() => {
          const header=document.querySelector('[data-v7-shell-header]');
          return header instanceof HTMLElement && Math.abs(parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--v7-header-height'))-Math.max(0,header.getBoundingClientRect().bottom))<=1;
        });
        const scrolled=await page.evaluate(()=>{
          const header=document.querySelector('[data-v7-shell-header]');
          const drawer=document.querySelector('#c-drawer');
          if(!(header instanceof HTMLElement&&drawer instanceof HTMLElement))throw new Error('scrolled drawer geometry absent');
          return {header:header.getBoundingClientRect().toJSON(),drawer:drawer.getBoundingClientRect().toJSON(),measured:parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--v7-header-height'))};
        });
        assert.ok(scrolled.header.bottom<=0,`390/scrolled: shared header must have scrolled out of view, bottom=${scrolled.header.bottom}`);
        near(scrolled.measured,Math.max(0,scrolled.header.bottom),'390/scrolled: drawer variable must follow visible header bottom');
        near(scrolled.drawer.y,Math.max(0,scrolled.header.bottom),'390/scrolled: collection drawer must not retain an initial-header gap');
        collection.scrolled=scrolled;
        await page.evaluate(()=>{document.querySelector('[data-shell-geometry-spacer]')?.remove();window.scrollTo(0,0);});
      }
      await page.screenshot({path:path.join(out,`shell-${width}.png`),fullPage:true});
      report.views.push({width,entries,rail,menu,noContext,withContext,collection,drawer:drawerBox});
    } finally {await context.close();}
  }
  await writeFile(path.join(out,'report.json'),JSON.stringify(report,null,2));
  assert.deepEqual(report.errors,[]);assert.deepEqual(report.blocked,[]);
  return report;
}
