// Actual old/new Login UI, no shared writes or external requests. Only explicit development navigation.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { chromium } from 'playwright';
import { verifyLoginLifecycle } from './login-lifecycle.mjs';
import { verifyShellLayout } from './shell-layout.mjs';
const candidate = process.env.WEB_LOGIN_CANDIDATE;
const baseline = process.env.WEB_LOGIN_BASELINE;
const directory = process.env.WEB_LOGIN_PROOF_DIR;
assert.ok(candidate && directory); for (const origin of [candidate, baseline].filter(Boolean)) assert.equal(new URL(origin).hostname,'127.0.0.1');
await mkdir(directory,{recursive:true});
const browser = await chromium.launch({headless:true}), report = {views:[], errors:[], blocked:[]};
try {
 for (const [name, origin] of [['before',baseline],['candidate',candidate]].filter(([,origin])=>origin)) {
  for (const [width,height] of [[1440,900],[1280,720],[1024,768],[800,900],[390,844],[375,667]]) {
   const context = await browser.newContext({viewport:{width,height},reducedMotion:'reduce'});
   const page = await context.newPage(); page.on('pageerror',e=>report.errors.push(String(e)));
   await page.route('**/*',route=>{
    const request=route.request();if(new URL(request.url()).origin!==origin || !['GET','HEAD'].includes(request.method())) {report.blocked.push({url:request.url(),method:request.method()});return route.abort();}return route.continue();
   });
   await page.goto(origin+'/login'); await page.getByRole('heading',{name:'登录工作台',exact:true}).waitFor();
   await page.waitForFunction(()=>['still','mobile','fallback'].includes(document.getElementById('station-scene')?.dataset.status));
   assert.equal(await page.locator('#station-scene').getAttribute('data-status'),width<=800?'mobile':'still');
   assert.equal(await page.locator('.station-live').count(),width<=800?0:1);
   assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
   async function capture(state) {
    const file=`${name}-${width}-${height}-${state}.png`;await page.screenshot({path:path.join(directory,file),fullPage:true});
    const boxes=await page.locator('.topbar,.auth,#email,#password,#submit-button,.hero-copy').evaluateAll(nodes=>nodes.map(node=>({selector:node.id||node.className,box:node.getBoundingClientRect().toJSON()})));
    report.views.push({name,width,height,state,file,boxes});
   }
   await capture('initial'); await page.getByRole('button',{name:'登录',exact:true}).click();
   assert.equal(await page.locator('#email').getAttribute('aria-invalid'),'true');await capture('empty');
   await page.getByLabel('邮箱',{exact:true}).fill('invalid');await page.getByLabel('密码',{exact:true}).fill('demo-only');
   await page.getByRole('button',{name:'登录',exact:true}).click();assert.ok(await page.getByText('请输入有效的邮箱地址。').isVisible());await capture('invalid');
   await page.getByRole('button',{name:'显示密码'}).click();assert.equal(await page.locator('#password').getAttribute('type'),'text');await capture('show-password');
   await page.getByText('登录遇到问题？',{exact:true}).click();await capture('help');await page.keyboard.press('Escape');assert.equal(await page.locator('#login-help').getAttribute('open'),null);
   await page.getByLabel('邮箱',{exact:true}).fill('demo@example.com');await page.getByLabel('记住邮箱').check();await capture('ready');
   await page.getByRole('button',{name:'登录',exact:true}).click();await page.waitForURL(/\/ops\/overview(?:\?|$)/);
   assert.deepEqual(await page.evaluate(()=>Object.entries(localStorage)),[['linggan.login.remembered-email','demo@example.com']]);
   await context.close();
  }
 }
 report.shell = await verifyShellLayout(browser,candidate,path.join(directory,'shell'));
 report.lifecycle = await verifyLoginLifecycle(browser, candidate, path.join(directory, 'lifecycle'));
 assert.deepEqual(report.errors,[]);assert.deepEqual(report.blocked,[]);
} finally {await browser.close();await writeFile(path.join(directory,'report.json'),JSON.stringify(report,null,2));}
console.log(JSON.stringify({screenshots:report.views.length,errors:report.errors,blocked:report.blocked}));
