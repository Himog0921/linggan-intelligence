// WEB-REACT-001 C: formal React UI + real Axum + isolated PostgreSQL, synthetic materials only.
// The Rust fixture owns the server/database lifecycle. All authoring writes below use the visible UI.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { inflateRawSync, inflateSync } from 'node:zlib';
import { chromium } from 'playwright';

const directory = process.env.CW_BROWSER_PROOF_DIR;
assert.ok(directory, 'CW_BROWSER_PROOF_DIR is required');
const fixture = JSON.parse(await readFile(path.join(directory, 'fixture.json'), 'utf8'));
const { origin, domainRef, accountRef } = fixture;
assert.equal(new URL(origin).hostname, '127.0.0.1');
await mkdir(directory, { recursive: true });
const report = { origin, synthetic: true, steps: [], screenshots: [], pageErrors: [], externalRequests: [], requests: [], failedRequests: [], artifacts: [] };
const browser = await chromium.launch({ headless: true });
const context = await browser.newContext({ viewport: { width: 1440, height: 900 }, locale: 'zh-CN', timezoneId: 'Asia/Shanghai', reducedMotion: 'reduce' });
await context.route('**/*', route => {
  if (new URL(route.request().url()).origin === origin) return route.continue();
  report.externalRequests.push(route.request().url());
  return route.abort();
});
const page = await context.newPage();
context.on('page', surface => surface.on('pageerror', error => report.pageErrors.push(error.message)));
page.on('pageerror', error => report.pageErrors.push(error.message));
context.on('request', request => report.requests.push({ method: request.method(), path: new URL(request.url()).pathname,
  requestId: request.method() === 'POST' && request.headers()['content-type']?.includes('json') ? request.postDataJSON()?.requestId ?? null : null }));
context.on('requestfailed', request => report.failedRequests.push({method:request.method(),path:new URL(request.url()).pathname,error:request.failure()?.errorText}));
page.setDefaultTimeout(20_000);
let finished = false;
let workId;
let workUrl;
let exportedRecord;
let packageBytes;
const imageSection = () => page.getByRole('region', { name: '封面与内页' });
const stage = async (name, surface = page) => {
  const button=surface.getByRole('navigation', {name:'创作步骤'}).getByRole('button',{name,exact:true});
  await button.click();
  await until(async()=>await button.getAttribute('aria-current')==='step',`${name} view has committed`);
};
const save = async (surface = page) => {
  await surface.getByRole('button', {name:'保存',exact:true}).click();
  await surface.locator('[aria-label="保存状态：已保存"]').waitFor();
};
const envelope = async () => {
  const response = await context.request.get(`${origin}/ops/api/creation/works/${workId}`);
  assert.equal(response.status(),200); return response.json();
};
async function step(name, run) {
  try { await run(); report.steps.push({name,result:'PASS'}); }
  catch(error) { report.steps.push({name,result:'FAIL',error:String(error?.stack ?? error)}); throw error; }
}
async function screenshot(name, surface = page) {
  await surface.screenshot({path:path.join(directory,`${name}.png`)});
  const geometry = await surface.evaluate(() => {
    const metrics=element=>element?{width:element.getBoundingClientRect().width,left:element.getBoundingClientRect().left,clientWidth:element.clientWidth,scrollWidth:element.scrollWidth,scrollLeft:element.scrollLeft,overflowX:getComputedStyle(element).overflowX}:null;
    return {viewport:innerWidth,documentWidth:document.documentElement.scrollWidth,documentScrollLeft:document.documentElement.scrollLeft,workspace:metrics(document.querySelector('main')),app:metrics(document.querySelector('.v7-app'))};
  });
  report.screenshots.push({name,...geometry}); return geometry;
}
async function until(check, description, timeout = 20_000) {
  const deadline=Date.now()+timeout;
  while(Date.now()<deadline) { if(await check())return; await new Promise(resolve=>setTimeout(resolve,50)); }
  throw new Error(`Timed out: ${description}`);
}
// A negative debounce assertion needs a bounded observation window, not a single instant check.
async function remains(check, description, duration=1200) {
  const deadline=Date.now()+duration;
  while(Date.now()<deadline) { assert.ok(await check(),description); await new Promise(resolve=>setTimeout(resolve,50)); }
}
async function selectExact(locator, selectedText) {
  await locator.evaluate((element,text) => {
    const full=element.textContent ?? '', start=full.indexOf(text);
    if(start<0)throw new Error(`Selection text missing: ${text}`);
    const walker=document.createTreeWalker(element,NodeFilter.SHOW_TEXT), nodes=[];
    while(walker.nextNode())nodes.push(walker.currentNode);
    const point=offset=>{let consumed=0;for(const node of nodes){if(consumed+node.textContent.length>=offset)return [node,offset-consumed];consumed+=node.textContent.length;}throw new Error('range offset');};
    const range=document.createRange();range.setStart(...point(start));range.setEnd(...point(start+text.length));
    const selection=window.getSelection();selection.removeAllRanges();selection.addRange(range);
    element.dispatchEvent(new MouseEvent('mouseup',{bubbles:true}));
  },selectedText);
  await until(async()=>await locator.locator('mark').textContent()===selectedText,'exact Unicode selection highlight');
}
async function collectFromPaper(text, selectedText, readingLabel) {
  const paper=page.getByRole('region',{name:'分组摘录纸条'}).getByRole('button').filter({hasText:text}).first();
  const sourceFragment=(await paper.getAttribute('data-fragment-id')).split(':').slice(0,-2).join(':');
  await paper.click();
  const leaf=page.getByLabel(readingLabel);
  await leaf.waitFor();
  await until(async()=>((await leaf.getAttribute('data-fragment-id'))??'').split(':').slice(0,-2).join(':')===sourceFragment,'reading leaf belongs to the selected source fragment');
  if(selectedText)await selectExact(leaf,selectedText);
  const fragment=await leaf.getAttribute('data-fragment-id');
  assert.equal(await page.locator('button[aria-pressed="true"][data-fragment-id]').getAttribute('data-fragment-id'),fragment);
  assert.equal(await page.getByRole('navigation',{name:'材料分布索引'}).locator('[aria-current="location"]').count(),1);
  await page.getByRole('button',{name:'摘入画布',exact:true}).click();
  await until(async()=>!(await page.getByRole('button',{name:'摘入画布',exact:true}).isDisabled()),'excerpt validation receipt');
}
function crc32(bytes) {
  let crc=0xffffffff;
  for(const byte of bytes){crc^=byte;for(let bit=0;bit<8;bit++)crc=(crc>>>1)^((crc&1)?0xedb88320:0);}
  return (crc^0xffffffff)>>>0;
}
function inspectPng(bytes) {
  assert.ok(bytes.subarray(0,8).equals(Buffer.from([137,80,78,71,13,10,26,10])),'PNG signature');
  assert.equal(bytes.readUInt32BE(16),1080);assert.equal(bytes.readUInt32BE(20),1440);
  let offset=8,ended=false;const compressed=[];
  while(offset<bytes.length){const size=bytes.readUInt32BE(offset),type=bytes.toString('ascii',offset+4,offset+8),data=bytes.subarray(offset+8,offset+8+size);
    assert.equal(crc32(bytes.subarray(offset+4,offset+8+size)),bytes.readUInt32BE(offset+8+size),`${type} CRC`);
    if(type==='IDAT')compressed.push(data);if(type==='IEND')ended=true;offset+=12+size;
  }
  assert.ok(ended);const pixels=inflateSync(Buffer.concat(compressed));assert.ok(pixels.length>1080*1440);
  assert.ok(new Set(pixels).size>16,'render includes text/image detail, not an empty solid canvas');
  return {width:1080,height:1440,byteSize:bytes.length,sha256:createHash('sha256').update(bytes).digest('hex')};
}
function inspectZip(bytes) {
  let eocd=bytes.length-22;while(eocd>=0&&bytes.readUInt32LE(eocd)!==0x06054b50)eocd--;
  assert.ok(eocd>=0,'ZIP end record');const count=bytes.readUInt16LE(eocd+10),entries=[];let cursor=bytes.readUInt32LE(eocd+16);
  for(let i=0;i<count;i++){
    assert.equal(bytes.readUInt32LE(cursor),0x02014b50);const method=bytes.readUInt16LE(cursor+10),checksum=bytes.readUInt32LE(cursor+16),size=bytes.readUInt32LE(cursor+20);
    const nameLength=bytes.readUInt16LE(cursor+28),extra=bytes.readUInt16LE(cursor+30),comment=bytes.readUInt16LE(cursor+32),local=bytes.readUInt32LE(cursor+42);
    const name=bytes.toString('utf8',cursor+46,cursor+46+nameLength);assert.equal(bytes.readUInt32LE(local),0x04034b50);
    const start=local+30+bytes.readUInt16LE(local+26)+bytes.readUInt16LE(local+28),compressed=bytes.subarray(start,start+size);
    const content=method===0?compressed:method===8?inflateRawSync(compressed):assert.fail(`unsupported ZIP compression ${method}`);
    assert.equal(crc32(content),checksum,`${name} CRC`);entries.push({name,content});cursor+=46+nameLength+extra+comment;
  }
  return entries;
}
async function download(locator, filename) {
  const pending=page.waitForEvent('download');await locator.click();const result=await pending;const target=path.join(directory,filename);
  await result.saveAs(target);assert.equal(await result.failure(),null);return readFile(target);
}
async function assertWholeEditPreview() {
  const canvas=imageSection().locator('canvas').first();await canvas.scrollIntoViewIfNeeded();
  const bounds=await canvas.boundingBox(),viewport=page.viewportSize();assert.ok(bounds&&viewport);
  assert.ok(bounds.height<=viewport.height*0.55+1,'single editor preview is bounded by available viewport');
  assert.ok(Math.abs(bounds.width/bounds.height-0.75)<0.01,'preview preserves full 3:4 artwork ratio');
  assert.ok(bounds.y>=0&&bounds.y+bounds.height<=viewport.height+1,'entire edit preview fits within visible viewport');
}
async function assertHorizontalReachability(locator, description) {
  await locator.scrollIntoViewIfNeeded();
  const bounds=await locator.boundingBox(),viewport=page.viewportSize();assert.ok(bounds&&viewport,`${description} has a layout box`);
  assert.ok(bounds.x>=-0.5,`${description} left edge ${bounds.x} must be inside the viewport`);
  assert.ok(bounds.x+bounds.width<=viewport.width+0.5,`${description} right edge ${bounds.x+bounds.width} must be inside the viewport`);
  const containers=await page.evaluate(()=>[document.documentElement,document.querySelector('.v7-app'),document.querySelector('main')].filter(Boolean).map(node=>({name:node.tagName,client:node.clientWidth,scroll:node.scrollWidth,left:node.scrollLeft})));
  for(const container of containers){assert.equal(container.left,0,`${description}: ${container.name} must not be horizontally shifted`);assert.ok(container.scroll<=container.client+1,`${description}: ${container.name} must not contain clipped horizontal page overflow (${container.scroll}/${container.client})`);}
}
async function assertPrimaryContrast(locator, width, phase='default') {
  const colors=await locator.evaluate(node=>({background:getComputedStyle(node).backgroundColor,foreground:getComputedStyle(node).color}));
  const channels=value=>value.match(/[\d.]+/g).map(Number);
  const background=channels(colors.background),foreground=channels(colors.foreground);
  assert.ok(background.length===3||background[3]===1,'enabled primary button must have an opaque fill');
  const luminance=rgb=>rgb.slice(0,3).map(value=>{const normalized=value/255;return normalized<=0.04045?normalized/12.92:((normalized+0.055)/1.055)**2.4;}).reduce((sum,value,index)=>sum+value*[0.2126,0.7152,0.0722][index],0);
  const values=[luminance(background),luminance(foreground)].sort((a,b)=>b-a),contrast=(values[0]+0.05)/(values[1]+0.05);
  assert.ok(contrast>=4.5,`primary action contrast ${contrast} must be at least 4.5:1`);
  report.primaryContrast??=[];report.primaryContrast.push({width,phase,...colors,contrast});
}

try {
  await step('设置页不加载创作模块或本地图页字体',async()=>{
    await page.goto(`${origin}/settings/models`);await page.getByRole('heading',{name:'评论研究的模型设置'}).waitFor();
    const resources=await page.evaluate(()=>performance.getEntriesByType('resource').map(entry=>entry.name));
    assert.ok(resources.every(url=>!url.includes('WorkbenchPage')&&!url.includes('noto-sans-sc')));
    report.settingsResources=resources.map(url=>new URL(url).pathname);
  });
  await step('运营入口建立真实服务器作品并保存角度提纲账号',async()=>{
    await page.goto(`${origin}/ops`);assert.equal(new URL(page.url()).pathname,'/ops/creation');
    await page.getByLabel('创作领域').selectOption(domainRef);await page.getByLabel('新选题标题').fill('作业启动');
    await page.getByRole('button',{name:'建立选题',exact:true}).click();await page.getByLabel('切入角度').waitFor();
    workId=new URL(page.url()).searchParams.get('work');assert.ok(workId);workUrl=page.url();report.workId=workId;
    await page.getByLabel('切入角度').fill('把作业启动缩小成可执行的第一步；仅作隔离合成验收。');
    await page.getByLabel('创作提纲').fill('先理解启动困难\n只做两分钟\n留出家庭缓冲');
    await page.getByLabel('所属账号').selectOption(accountRef);await save();
    const saved=await envelope();assert.equal(saved.revision.document.accountRef,accountRef);assert.equal(saved.revision.document.outline.length,3);
    await screenshot('workbench-1440-topic');
  });
  await step('封面和四张内页：模板配色对齐、自有图片裁切、稳定排序与撤销',async()=>{
    await stage('图组 / 导出');const area=imageSection();await area.getByRole('button',{name:'添加封面',exact:true}).click();
    await area.getByRole('button',{name:'预览带入正文标题'}).click();assert.equal(await area.getByLabel('图页标题').inputValue(),'');
    await area.getByRole('button',{name:'确认带入标题'}).click();await until(async()=>await area.getByLabel('图页标题').inputValue()==='作业启动','confirmed title imported');assert.equal(await area.getByLabel('图页标题').inputValue(),'作业启动');
    await area.getByRole('button',{name:'撤销标题带入'}).click();await until(async()=>await area.getByLabel('图页标题').inputValue()==='','title import undone');assert.equal(await area.getByLabel('图页标题').inputValue(),'');
    await area.getByRole('button',{name:'预览带入正文标题'}).click();await area.getByRole('button',{name:'确认带入标题'}).click();
    await area.getByLabel('副标题').fill('先开始两分钟');await area.getByRole('combobox',{name:'图页模板',exact:true}).selectOption('creation-v1-editorial');
    await area.getByRole('combobox',{name:'图页配色',exact:true}).selectOption('sage');await area.getByRole('combobox',{name:'图页文字对齐',exact:true}).selectOption('center');
    const definitions=[['观点','把开始缩小','先把练习本放到桌上。'],['原声','听见真实困难','困难常常在第一步。'],['步骤','今天可以这样做','放好练习本。'],['观点','给家庭留出缓冲','允许短暂停顿。']];
    for(const [kind,title,body] of definitions){await area.getByRole('button',{name:`添加${kind}页`,exact:true}).click();await area.getByLabel('图页标题').fill(title);await area.getByLabel(kind==='步骤'?'步骤 1':'文案 1',{exact:true}).fill(body);}
    const dataUrl=await page.evaluate(()=>{const canvas=document.createElement('canvas');canvas.width=640;canvas.height=400;const ctx=canvas.getContext('2d');ctx.fillStyle='#d8e6ca';ctx.fillRect(0,0,640,400);ctx.fillStyle='#42683f';ctx.fillRect(80,50,220,300);ctx.fillStyle='#e4b24b';ctx.beginPath();ctx.arc(450,200,100,0,Math.PI*2);ctx.fill();return canvas.toDataURL('image/png');});
    await area.getByLabel('自有图片').setInputFiles({name:'synthetic-own-image.png',mimeType:'image/png',buffer:Buffer.from(dataUrl.split(',')[1],'base64')});
    await area.getByText('自有图片已上传；保存作品后保留图片关联').waitFor();
    await area.getByLabel('裁切宽度').fill('80');await area.getByLabel('横向位置').fill('10');await save();
    const before=(await envelope()).revision.document.pages;const ids=before.map(item=>item.id);
    await area.getByRole('button',{name:'上移',exact:true}).click();await save();
    assert.deepEqual((await envelope()).revision.document.pages.map(item=>item.id),[ids[0],ids[1],ids[2],ids[4],ids[3]]);
    await area.getByRole('button',{name:'撤销图页编辑'}).click();await save();assert.deepEqual((await envelope()).revision.document.pages.map(item=>item.id),ids);
    await area.getByRole('button',{name:'删除此页'}).click();await area.getByRole('button',{name:'恢复删除页'}).click();await save();
    assert.deepEqual((await envelope()).revision.document.pages.map(item=>item.id),ids);
    await assertWholeEditPreview();
    await screenshot('workbench-1440-image-edit');
  });
  await step('两篇笔记、两条匿名原声和回复语境；纸条索引书页精确联动',async()=>{
    await stage('材料');await collectFromPaper('浏览器原文锚点一','合成长文😀','可选择原文第 2 段');
    assert.equal(await page.getByLabel('可选择原文第 2 段').getAttribute('data-work-ref'),fixture.noteRefs[0]);
    const second=page.getByRole('region',{name:'分组摘录纸条'}).getByRole('button').filter({hasText:'浏览器原文锚点二'}).first();await second.click();
    await until(async()=>await page.getByRole('region',{name:'原文阅读'}).locator('[data-work-ref]').first().getAttribute('data-work-ref')===fixture.noteRefs[1],'second note source work identity');
    await page.getByRole('group',{name:'材料类型'}).getByRole('button',{name:'匿名原声',exact:true}).click();
    await collectFromPaper('浏览器匿名原声一😀','原声一😀','可选择原文第 1 段');
    assert.equal(await page.getByLabel('可选择原文第 1 段').getAttribute('data-source-ref'),fixture.voiceRefs[0]);
    const wall=page.getByRole('region',{name:'分组摘录纸条'});await wall.getByRole('button').filter({hasText:'浏览器匿名原声二🙂'}).first().click();
    await until(async()=>await page.locator('[data-source-ref]').first().getAttribute('data-source-ref')===fixture.voiceRefs[1],'second voice read');
    await wall.getByRole('button').filter({hasText:'浏览器回复原声😀'}).first().click();await page.getByText('原文上下文 / 来源详情',{exact:true}).click();
    await page.getByRole('heading',{name:'回复语境',exact:true}).first().waitFor();await page.getByRole('region',{name:'原文阅读'}).getByText('浏览器匿名原声一😀',{exact:false}).last().waitFor();
    await save();const doc=(await envelope()).revision.document;assert.equal(doc.excerpts.length,2);assert.ok(doc.excerpts.some(item=>item.selectedText==='合成长文😀'));assert.ok(doc.excerpts.some(item=>item.selectedText==='原声一😀'));
    await screenshot('workbench-1440-materials');
  });
  await step('画布自己的表达一次放入正文和指定图页，稳定引用可回查',async()=>{
    await stage('画布');const cards=page.locator('article').filter({has:page.getByRole('button',{name:'回到出处 ↗',exact:true})});
    await until(async()=>await cards.count()===2,'both persisted excerpt cards have rendered');
    assert.equal(await cards.count(),2);
    for(let index=0;index<2;index++){
      const card=cards.nth(index);await card.getByRole('textbox',{name:/我的表达/}).fill(index===0?'先把练习本放到桌上。':'先做两分钟，再决定下一步。');
      await card.getByRole('button',{name:'放入正文 / 指定图页 →',exact:true}).click();const dialog=page.getByRole('dialog',{name:'放入作品'});
      await dialog.getByLabel('指定内页',{exact:true}).check();const options=await dialog.getByLabel('放入图页位置').locator('option').evaluateAll(nodes=>nodes.filter(node=>node.value).map(node=>node.value));
      await dialog.getByLabel('放入图页位置').selectOption(options[index]);await dialog.getByRole('button',{name:'确认放入',exact:true}).click();await dialog.waitFor({state:'detached'});
    }
    await save();const doc=(await envelope()).revision.document;assert.equal(doc.paragraphs.length,2);assert.equal(doc.uses.filter(item=>item.target.kind==='paragraph').length,2);assert.equal(doc.uses.filter(item=>item.target.kind==='pageBlock').length,2);
    await cards.first().getByRole('button',{name:'回到出处 ↗',exact:true}).click();await page.getByRole('region',{name:'原文阅读'}).locator('mark').waitFor();
    await screenshot('workbench-source-backlink');
  });
  await step('正文标签保存后重开，图组和引用完整恢复',async()=>{
    await stage('正文');await page.getByLabel('确认发布标签').fill('家庭养育 作业启动');await page.getByLabel('确认发布标签').blur();await save();
    await page.reload();await page.getByLabel('正文第 1 段').waitFor();assert.equal(await page.getByLabel('确认发布标签').inputValue(),'家庭养育 作业启动');
    const doc=(await envelope()).revision.document;assert.equal(doc.pages.length,5);assert.equal(doc.excerpts.length,2);assert.equal(doc.uses.length,4);assert.ok(doc.pages.some(item=>item.assetId));
    report.persistedPageIds=doc.pages.map(item=>item.id);
  });
  await step('正文修改后刷新仍定位关联图页，采用一页保留其他手改',async()=>{
    const doc=(await envelope()).revision.document;const linked=doc.uses.find(item=>item.target.kind==='pageBlock'&&item.canvasId===doc.uses.find(use=>use.target.kind==='paragraph'&&use.target.paragraphId===doc.paragraphs[0].id).canvasId);
    const targetPage=doc.pages.find(item=>item.id===linked.target.pageId),other=doc.pages.find(item=>item.kind==='steps');
    await page.getByLabel('正文第 1 段').fill('先把练习本放好，只做眼前这一小步。');await save();await page.reload();await stage('图组 / 导出');
    await imageSection().getByRole('navigation',{name:'图页顺序'}).getByRole('button').filter({hasText:targetPage.title}).click();
    await imageSection().getByRole('button',{name:'带入这一段',exact:true}).waitFor();await imageSection().getByRole('button',{name:'带入这一段',exact:true}).click();await save();
    const after=(await envelope()).revision.document;assert.equal(after.pages.find(item=>item.id===targetPage.id).blocks.find(item=>item.id===linked.target.blockId).text,'先把练习本放好，只做眼前这一小步。');assert.deepEqual(after.pages.find(item=>item.id===other.id),other);
    await screenshot('workbench-a09-persisted-update');
  });
  await step('保存实际提交后断开回执：未确认、无自动重试、GET 核实原编号',async()=>{
    await stage('正文');let intercepted=false;let originalId;let committed;
    const pattern='**/ops/api/creation/works/*/save';
    await page.route(pattern,async route=>{if(intercepted)return route.continue();intercepted=true;originalId=route.request().postDataJSON().requestId;committed=await route.fetch();assert.equal(committed.status(),200);await route.abort('connectionreset');});
    await page.getByLabel('正文第 2 段').fill('先做两分钟，再给自己一次选择。');await page.getByRole('button',{name:'保存',exact:true}).click();
    await page.getByRole('button',{name:'沿用原请求重试',exact:true}).waitFor();const count=report.requests.filter(item=>item.method==='POST'&&item.path.endsWith('/save')).length;
    await remains(()=>report.requests.filter(item=>item.method==='POST'&&item.path.endsWith('/save')).length===count,'unknown save must not auto resend');
    assert.ok(await page.locator('[aria-label="保存状态：结果未确认"]').count());
    await page.getByRole('button',{name:'沿用原请求重试',exact:true}).click();await page.locator('[aria-label="保存状态：已保存"]').waitFor();
    assert.ok(report.requests.some(item=>item.method==='GET'&&item.path===`/ops/api/creation/requests/${originalId}`));
    assert.equal(report.requests.filter(item=>item.method==='POST'&&item.path.endsWith('/save')).length,count,'GET receipt avoids duplicate POST after committed save');
    await page.unroute(pattern);report.saveReconciledRequestId=originalId;await screenshot('workbench-after-unknown-save-recovery');
  });
  await step('实际冻结版本、完整五页 PNG 和发布 ZIP 校验',async()=>{
    await stage('图组 / 导出');const area=imageSection();await area.getByRole('button',{name:'整组预览',exact:true}).click();
    await until(async()=>await area.getByText('画幅检查通过',{exact:true}).count()===5,'all five target-pixel previews ready',45_000);
    await screenshot('workbench-1440-complete-preview');await area.getByRole('button',{name:'保存并生成图文包',exact:true}).click();
    await area.getByText('图文包已生成，可下载',{exact:true}).waitFor({timeout:60_000});const history=area.locator('details').filter({has:page.getByText(/已生成与中断的图文包/)});await history.locator('summary').click();
    packageBytes=await download(area.getByRole('link',{name:'重新下载图文包',exact:true}).first(),'creation-package.zip');
    const entries=inspectZip(packageBytes);assert.deepEqual(entries.map(entry=>entry.name),['正文.txt','01-封面.png','02-内页.png','03-内页.png','04-内页.png','05-内页.png']);
    const current=await envelope();exportedRecord=current.exports.find(record=>record.state==='complete');assert.ok(exportedRecord);
    assert.ok(exportedRecord.fontVersion.includes('ed05125987c35ee57be16be7242568f1d516fbf497bc0ef4161499ca9e8d8fce'));
    const body=entries[0].content.toString('utf8');assert.ok(body.startsWith('作业启动'));assert.ok(body.includes('家庭养育'));assert.ok(body.includes('作业启动'));
    for(const paragraph of current.revision.document.paragraphs)assert.ok(body.includes(paragraph.text));
    for(const excerpt of current.revision.document.excerpts)assert.ok(!body.includes(excerpt.id)&&!body.includes(excerpt.citation.sourceRef),'no internal citation metadata');
    assert.ok(!body.includes('content-workbench.v1')&&!body.includes('rendererVersion'));
    for(const entry of entries.slice(1)){const info=inspectPng(entry.content);await writeFile(path.join(directory,entry.name),entry.content);report.artifacts.push({name:entry.name,...info});}
    assert.equal(createHash('sha256').update(packageBytes).digest('hex'),exportedRecord.packageSha256);report.exportId=exportedRecord.id;report.exportRevision=exportedRecord.revisionId;
    const single=await download(area.getByRole('link',{name:'下载第 1 张 PNG',exact:true}).first(),'single-cover.png');assert.ok(single.equals(entries[1].content));inspectPng(single);
  });
  await step('单页独立冻结生成，不借用可变编辑器内容',async()=>{
    const area=imageSection();await area.getByRole('button',{name:'返回编辑',exact:true}).click();await area.getByRole('button',{name:'保存并生成此页 PNG',exact:true}).click();
    await area.getByText('单页 PNG 已生成，可在下方下载；其余图页可继续生成图文包',{exact:true}).waitFor({timeout:60_000});
    const current=await envelope(),partial=current.exports.find(record=>record.state==='rendering'&&record.pages.filter(item=>item.sha256).length===1);assert.ok(partial);
    const ready=partial.pages.find(item=>item.sha256);const link=area.locator(`a[href="/ops/api/creation/exports/${partial.id}/pages/${ready.pageId}"]`);
    const bytes=await download(link,'independent-single-page.png');inspectPng(bytes);report.singlePageExportId=partial.id;
  });
  await step('双标签版本冲突保留双方文字，历史图文包保持不可变',async()=>{
    await stage('正文');await save();const second=await context.newPage();second.setDefaultTimeout(20_000);await second.goto(page.url());await second.getByLabel('正文第 2 段').waitFor();
    await page.getByLabel('正文第 2 段').fill('服务器版本：保留两分钟的起点。');await save();
    await second.getByLabel('正文第 2 段').fill('本页版本：保留自己的新表达。');await second.getByRole('button',{name:'保存',exact:true}).click();
    const conflict=second.getByRole('dialog',{name:'服务器已有新版本'});await conflict.waitFor();await conflict.getByText('服务器版本：保留两分钟的起点。',{exact:false}).waitFor();await conflict.getByText('本页版本：保留自己的新表达。',{exact:false}).waitFor();
    await screenshot('workbench-second-tab-conflict',second);await conflict.getByRole('button',{name:'保留本页编辑，另存新版本',exact:true}).click();await save(second);await second.close();
    await page.reload();await page.getByLabel('正文第 2 段').waitFor();assert.equal(await page.getByLabel('正文第 2 段').inputValue(),'本页版本：保留自己的新表达。');
    const response=await context.request.get(`${origin}/ops/api/creation/exports/${exportedRecord.id}/download`);assert.equal(response.status(),200);assert.ok((await response.body()).equals(packageBytes));
  });
  await step('键盘 Esc 关闭历史并还原焦点，减少动态效果生效',async()=>{
    const history=page.getByRole('button',{name:'历史',exact:true});await history.focus();await page.keyboard.press('Enter');await page.getByRole('dialog',{name:'历史与恢复'}).waitFor();await page.keyboard.press('Escape');await page.getByRole('dialog').waitFor({state:'detached'});
    assert.equal(await page.evaluate(()=>document.activeElement?.textContent),'历史');assert.ok(await page.evaluate(()=>matchMedia('(prefers-reduced-motion: reduce)').matches));
  });
  for(const [width,height] of [[1440,900],[1280,720],[1024,768],[390,844]]){
    await step(`${width}×${height} 正文与图组可操作，无页面横向溢出`,async()=>{
      await page.setViewportSize({width,height});await stage('正文');
      for(const name of ['正文标题','正文第 1 段','确认发布标签'])await assertHorizontalReachability(page.getByLabel(name,{exact:true}),name);
      let layout=await screenshot(`workbench-${width}x${height}-body`);assert.ok(layout.documentWidth<=layout.viewport);
      await stage('图组 / 导出');
      for(const name of ['图页模板','图页标题','副标题','图页配色','图页文字对齐','自有图片'])await assertHorizontalReachability(imageSection().getByLabel(name,{exact:true}),name);
      for(const name of ['保存并生成图文包','保存并生成此页 PNG'])await assertHorizontalReachability(imageSection().getByRole('button',{name,exact:true}),name);
      await assertPrimaryContrast(imageSection().getByRole('button',{name:'保存并生成图文包',exact:true}),width);
      await imageSection().getByRole('button',{name:'保存并生成图文包',exact:true}).hover();await assertPrimaryContrast(imageSection().getByRole('button',{name:'保存并生成图文包',exact:true}),width,'hover');await page.mouse.move(0,0);
      await assertWholeEditPreview();await assertHorizontalReachability(imageSection().locator('canvas').first(),'完整图页画布');layout=await screenshot(`workbench-${width}x${height}-images`);assert.ok(layout.documentWidth<=layout.viewport);
      await stage('材料');await page.getByRole('region',{name:'原文阅读'}).waitFor();
      await assertHorizontalReachability(page.getByLabel('检索已取得材料',{exact:true}),'材料检索');await assertHorizontalReachability(page.getByRole('region',{name:'原文阅读'}),'原文阅读区');await assertHorizontalReachability(page.getByRole('button',{name:'摘入画布',exact:true}),'摘入画布');
      const indexBounds=await page.getByRole('navigation',{name:'材料分布索引'}).locator('button, i').evaluateAll(nodes=>nodes.map(node=>{const box=node.getBoundingClientRect();return {left:box.left,right:box.right};}));
      assert.ok(indexBounds.length>100,'long synthetic source retains all fragment index segments');assert.ok(indexBounds.every(box=>box.left>=0&&box.right<=width+0.5),'every index segment and marker stays within the viewport');
      layout=await screenshot(`workbench-${width}x${height}-materials`);assert.ok(layout.documentWidth<=layout.viewport);
    });
  }
  await step('没有外部请求、模型执行或页面脚本错误',async()=>{
    assert.deepEqual(report.externalRequests,[]);assert.deepEqual(report.pageErrors,[]);
    assert.ok(report.requests.every(request=>!request.path.includes('/ai/requests')&&!request.path.includes('/probes')),'no model/provider command');
    const unexpected=report.failedRequests.filter(request=>!(request.method==='POST'&&request.path.endsWith('/save'))&&!request.error?.includes('ERR_ABORTED'));
    assert.deepEqual(unexpected,[]);report.finalRevision=(await envelope()).revision.id;
  });
  finished=true;
} catch(error) {
  report.overflowDiagnostics=await page.evaluate(()=>{
    const main=document.querySelector('main');if(!main)return [];
    const edge=main.getBoundingClientRect();
    const describe=node=>{const box=node.getBoundingClientRect(),style=getComputedStyle(node);return {tag:node.tagName,className:node.className,label:node.getAttribute('aria-label'),text:node.textContent?.slice(0,100),x:box.x,right:box.right,width:box.width,clientWidth:node.clientWidth,scrollWidth:node.scrollWidth,scrollLeft:node.scrollLeft,display:style.display,minWidth:style.minWidth,widthStyle:style.width,overflowX:style.overflowX,gridColumns:style.gridTemplateColumns,columns:style.columns};};
    return [...main.querySelectorAll('*')].filter(node=>{const box=node.getBoundingClientRect();return box.width>0&&(box.left<edge.left-1||box.right>edge.right+1||node.scrollWidth>node.clientWidth+1);}).slice(0,300).map(node=>({node:describe(node),parent:node.parentElement?describe(node.parentElement):null}));
  }).catch(()=>[]);
  await page.screenshot({path:path.join(directory,'failure.png')}).catch(()=>undefined);
  await writeFile(path.join(directory,'failure-body.txt'),await page.locator('body').innerText().catch(()=>''));
  report.error=String(error?.stack??error);throw error;
} finally {
  report.finished=finished;report.workUrl=workUrl;await writeFile(path.join(directory,'report.json'),JSON.stringify(report,null,2));await browser.close();
}
console.log(`WEB-REACT-001 content workbench real-chain proof: ${report.steps.length} steps passed`);
