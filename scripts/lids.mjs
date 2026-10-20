#!/usr/bin/env node
/** Project-local LIDS source -> runtime -> real component examples -> offline export. */
import fs from 'node:fs/promises';
import path from 'node:path';
import {fileURLToPath, pathToFileURL} from 'node:url';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
import {createRequire} from 'node:module';

export const ROOT=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const WEB=path.join(ROOT,'apps/web'), DS=path.join(WEB,'design-system'), GEN=path.join(DS,'generated');
const LIDS='docs/design/lids', OUT=path.join(WEB,'dist-design-system');
const names=['typography','focus','control-material','shell-layout','ledger-layout','composition-layout','overlay-policy','component-reuse','media-material','color'];
const mirrors=['typographyApplication','focusApplication','materialApplication','shellApplication','ledgerApplication','compositionApplication','overlayApplication','componentApplication','mediaApplication'];
const hash=b=>createHash('sha256').update(b).digest('hex');
const json=v=>JSON.stringify(v,null,2)+'\n';
const read=rel=>fs.readFile(path.join(ROOT,rel),'utf8');
const readJson=async rel=>JSON.parse(await read(rel));
async function put(rel,content){const p=path.join(ROOT,rel);await fs.mkdir(path.dirname(p),{recursive:true});await fs.writeFile(p,content);}
async function walk(dir){const rows=[];for(const e of await fs.readdir(dir,{withFileTypes:true})){if(e.name==='generated')continue;const p=path.join(dir,e.name);rows.push(...(e.isDirectory()?await walk(p):[p]));}return rows;}
export function validateContracts(contract){
 const rows=contract?.components;if(!rows?.C09||!rows?.C16)throw new Error('C09/C16 contracts are required');
 if(rows.C16.debounceMs!==250||json(rows.C16.triggers)!==json(['submit','debounce_250']))throw new Error('C16 approved query semantics changed; requires an explicit standard decision');
 for(const [id,c] of Object.entries(rows)){
  for(const field of ['implementation','styles','demo','tests','states','owns','excludes','ruleSources','consumers'])if(!c[field]?.length)throw new Error(`${id}: missing ${field}`);
  if(c.certified!==false)throw new Error(`${id}: this sync does not grant CMP certification`);
  for(const file of [c.implementation,c.styles,c.demo,...c.tests,...c.consumers])if(path.isAbsolute(file)||file.split('/').includes('..'))throw new Error(`${id}: source path escapes project`);
 }
}
async function contracts(){const c=await readJson(`${LIDS}/component-contracts.json`);validateContracts(c);return c;}
export async function componentFingerprints(c,readFile=read){
 const result={};for(const [id,item] of Object.entries(c.components)){
  const entries=[];for(const p of [item.implementation,item.styles,...item.tests,...item.ruleSources.map(n=>`${LIDS}/${n}.json`)])entries.push([p,hash(await readFile(p))]);
  entries.push(['contract',hash(json(item))]);result[id]=hash(json(entries));
 }return result;
}
export function validateChange(record,fingerprints){
 if(!['implementation_fix','shared_standard','page_exception'].includes(record?.kind)||!record.reason?.trim()||!record.authorization?.trim())throw new Error('A classified component change with reason and authorization is required');
 for(const [id,value] of Object.entries(fingerprints))if(record.componentFingerprints?.[id]!==value)throw new Error(`${id}: component change declaration is stale; record the change kind/reason before sync`);
 if(record.kind==='page_exception'&&!record.exceptionScope?.trim())throw new Error('page_exception requires a bounded exceptionScope');
}
async function inputs(c){
 const files=new Set([...names.map(n=>`${LIDS}/${n}.json`),`${LIDS}/catalog-source.json`,`${LIDS}/component-contracts.json`,`${LIDS}/component-sync.md`,'docs/design/changes/lids-component-change.json','AGENTS.md','CLAUDE.md','codx.md','docs/governance/ui-design-execution.md','scripts/lids.mjs','scripts/lids.test.mjs','scripts/sync-lids-runtime.py','apps/web/package.json','apps/web/src/shared/SelectField.tsx','apps/web/src/shared/SelectField.module.css','apps/web/src/shared/inputMethod.ts','apps/web/src/styles/typography.css']);
 const catalog=await readJson(`${LIDS}/catalog-source.json`);
 for(const page of catalog.pages)for(const source of page.implementationSources||[]){if(path.isAbsolute(source)||source.split('/').includes('..'))throw new Error('Page implementation source escapes project: '+source);files.add(source);}
 for(const row of Object.values(c.components))for(const p of [row.implementation,row.styles,...row.tests,...row.consumers])files.add(p);
 for(const p of await walk(DS))files.add(path.relative(ROOT,p).split(path.sep).join('/'));
 const fontRoot='apps/web/public/fonts/lids',manifest=await readJson(fontRoot+'/manifest.json');files.add(fontRoot+'/manifest.json');
 for(const file of manifest.files){const p=fontRoot+'/'+file.file;const bytes=await fs.readFile(path.join(ROOT,p));if(hash(bytes)!==file.sha256)throw new Error('Font/license drift: '+file.file);files.add(p);}
 const rows=[];for(const p of [...files].sort())rows.push({path:p,sha256:hash(await fs.readFile(path.join(ROOT,p)))});return rows;
}
function componentDoc(c){return '# LIDS 标准件实现目录（生成）\n\n> 状态: 权威当前\n> 最后核对: 2026-10-20\n> 适用范围: 已接通同源展示的标准件；不授予CMP\n> 事实来源: component-contracts.json；lids:sync生成\n> 冲突时以谁为准: 正式规则源和业务合同\n\n'+Object.entries(c.components).map(([id,v])=>`## ${id} · ${v.name}\n\n- 实现：[${v.implementation}](../../../${v.implementation})\n- 样式：[${v.styles}](../../../${v.styles})\n- 变体/触发：${(v.variants||v.triggers).join(' / ')}\n- 状态：${v.states.join(' / ')}\n- 负责：${v.owns.join('；')}\n- 页面负责：${v.excludes.join('；')}\n- 测试：${v.tests.join('、')}\n- 展示：${v.demo}（引用同一真实组件）\n`).join('\n');}
async function generated(c){
 const rules={},sources=[];for(const name of names){const p=`${LIDS}/${name}.json`,bytes=await read(p);rules[name]=JSON.parse(bytes);sources.push({path:p,sha256:hash(bytes)});}
 const registry=await readJson(`${LIDS}/catalog-source.json`);
 names.slice(0,9).forEach((n,i)=>{registry.visual[mirrors[i]]={...rules[n],generatedFrom:{path:sources[i].path,sha256:sources[i].sha256}};});
 for(const [id,contract] of Object.entries(c.components)){
  const row=registry.components.find(row=>row.id===id);row.variants=contract.variants||contract.triggers;row.states=contract.states;
  row.owns=contract.owns.join('；');row.excludes=contract.excludes.join('；');
 }
 const sourceInputs=await inputs(c),fingerprint=hash(json(sourceInputs));
 const manifest=await readJson('apps/web/public/fonts/lids/manifest.json'),licenses=[];
 for(const f of manifest.files.filter(f=>f.file.endsWith('.txt')))licenses.push({file:f.file,text:await read('apps/web/public/fonts/lids/'+f.file),sha256:f.sha256});
 const activePages=registry.pages.filter(page=>page.status!=='retired');
 const inventory={activePages:activePages.length,activeModes:activePages.reduce((n,p)=>n+p.modes.length,0),retiredPages:registry.pages.length-activePages.length,pages:registry.pages.length,modes:registry.pages.reduce((n,p)=>n+p.modes.length,0),patterns:registry.patterns.length,regions:registry.regions.length,components:registry.components.length,interactions:registry.interactions.length,businessBlocks:registry.businessBlocks.length};
 const snapshot={id:'LIDS-PROJECT-CURRENT',status:'accepted_rules_live_components_not_CMP_certification',date:'2026-10-20',rules,licenses,sources,inventory,fingerprint,sharedComponents:c.components};
 const typeRows=Object.entries(rules.typography.roles).map(([key,v])=>`| ${rules.typography.roleLabels[key]} \`${key}\` | ${v.size} | ${v.weight} | ${v.lineHeight} |`).join('\n');
 if(!(await read(`${LIDS}/typography.md`)).includes('<!-- TYPE-B:BEGIN -->'))throw new Error('Typography generated table marker missing');
 const typeDoc=(await read(`${LIDS}/typography.md`)).replace(/<!-- TYPE-B:BEGIN -->[\s\S]*?<!-- TYPE-B:END -->/,`<!-- TYPE-B:BEGIN -->\n\n| 角色 | 字号px | 字重 | 行高 |\n| --- | --- | --- | --- |\n${typeRows}\n\n<!-- TYPE-B:END -->`);
 const config=`// Generated from docs/design/lids/component-contracts.json by lids:sync.\nexport const ACTION_VARIANTS = ${JSON.stringify(c.components.C09.variants)} as const;\nexport type ActionVariant = typeof ACTION_VARIANTS[number];\nexport const FIND_TRIGGERS = ${JSON.stringify(c.components.C16.triggers)} as const;\nexport type FindTrigger = typeof FIND_TRIGGERS[number];\nexport const FIND_DEBOUNCE_MS = ${c.components.C16.debounceMs};\n`;
 return new Map([['apps/web/design-system/generated/registry.json',json(registry)],['apps/web/design-system/generated/source-data.json',json(snapshot)],['apps/web/design-system/generated/source-lock.json',json({fingerprint,inputs:sourceInputs})],['apps/web/src/shared/lidsComponents.generated.ts',config],[`${LIDS}/component-library.generated.md`,componentDoc(c)],[`${LIDS}/typography.md`,typeDoc]]);
}
async function checkBindings(c){
 const demo=await read('apps/web/design-system/dsReview/ui.jsx');
 for(const [id,row] of Object.entries(c.components)){
  const base=path.basename(row.implementation,'.tsx');if(!demo.includes(`import {${base}} from '../../src/shared/${base}'`))throw new Error(`${id}: demonstration must import the real shared component`);
  for(const p of row.consumers){const text=await read(p);if(!text.includes(base))throw new Error(`${id}: registered consumer no longer references ${base}: ${p}`);if(id==='C16'&&(/data-find\b|>\s*FIND\s*</.test(text)))throw new Error(`${id}: registered page duplicates the shared FIND skin: ${p}`);}
 }
}
export async function check({exportFile=true}={}){
 const c=await contracts();validateChange(await readJson('docs/design/changes/lids-component-change.json'),await componentFingerprints(c));await checkBindings(c);
 execFileSync('python3',['scripts/sync-lids-runtime.py','--check'],{cwd:ROOT,stdio:'pipe'});
 const expected=await generated(c);for(const [p,value] of expected){let actual;try{actual=await read(p)}catch{throw new Error('Missing generated file: '+p)}if(actual!==value)throw new Error('LIDS drift: '+p+'; run lids:sync');}
 if(exportFile){let m;try{m=JSON.parse(await fs.readFile(path.join(OUT,'source-manifest.json'),'utf8'))}catch(e){if(e.code!=='ENOENT')throw e;}
  if(m){const lock=JSON.parse(expected.get('apps/web/design-system/generated/source-lock.json'));if(m.fingerprint!==lock.fingerprint||hash(await fs.readFile(path.join(OUT,m.html.file)))!==m.html.sha256)throw new Error('Offline HTML is stale; run lids:sync or lids:build');}}
 return expected;
}
async function buildExport(){
 await check({exportFile:false});
 const require=createRequire(path.join(WEB,'package.json'));const {build}=await import(pathToFileURL(require.resolve('vite')).href),react=(await import(pathToFileURL(require.resolve('@vitejs/plugin-react')).href)).default;
 const buildDir=path.join(OUT,'build');
 await build({configFile:false,root:WEB,publicDir:false,base:'./',plugins:[react()],build:{outDir:buildDir,emptyOutDir:true,rollupOptions:{input:path.join(DS,'finalEdition/main.jsx'),output:{format:'iife',inlineDynamicImports:true,entryFileNames:'design-system.js',assetFileNames:'[name][extname]'}}}});
 const files=await fs.readdir(buildDir);let css=(await Promise.all(files.filter(f=>f.endsWith('.css')).map(f=>fs.readFile(path.join(buildDir,f),'utf8')))).join('\n').replace(/@font-face\s*\{[^}]*\}/g,'');
 let faces=((await read('apps/web/src/styles/typography.css')).match(/@font-face\s*\{[^}]*\}/g)||[]).join('\n');
 for(const m of [...faces.matchAll(/url\('([^']+)'\)/g)]){const rel=m[1].split('/fonts/lids/')[1],bytes=await fs.readFile(path.join(WEB,'public/fonts/lids',rel));faces=faces.replaceAll(m[1],`data:${rel.endsWith('.ttf')?'font/ttf':'font/woff2'};base64,${bytes.toString('base64')}`);}
 const js=(await fs.readFile(path.join(buildDir,'design-system.js'),'utf8')).replace(/@font-face\s*\{[^}]*\}/g,'');
 const source=JSON.parse(await fs.readFile(path.join(GEN,'source-data.json'),'utf8'));
 const html=`<!doctype html><html lang="zh-CN"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta name="lids-source" content="${source.fingerprint}"><title>Linggan Intelligence · 当前设计系统</title><style>${faces}\n${css}</style></head><body><div id="root"></div><script>${js.replaceAll('</script','<\\/script')}</script></body></html>`;
 if(/<(?:script|link)[^>]+(?:src|href)=/i.test(html)||(html.match(/@font-face/g)||[]).length!==6)throw new Error('Offline export has external dependencies or missing faces');
 const name='Linggan-Intelligence-Design-System.html';await fs.writeFile(path.join(OUT,name),html);
 const manifest={fingerprint:source.fingerprint,html:{file:name,sha256:hash(html),bytes:Buffer.byteLength(html)},sharedComponents:['C09','C16'],inventory:source.inventory,systemMaturity:'PROPOSED; confirmed rules binding',businessRequests:0};
 await fs.writeFile(path.join(OUT,'source-manifest.json'),json(manifest));
 const pkg=path.join(OUT,'package');await fs.rm(pkg,{recursive:true,force:true});await fs.mkdir(pkg,{recursive:true});await fs.copyFile(path.join(OUT,name),path.join(pkg,name));await fs.copyFile(path.join(OUT,'source-manifest.json'),path.join(pkg,'source-manifest.json'));
 await fs.cp(path.join(ROOT,LIDS),path.join(pkg,'docs/design/lids'),{recursive:true});await fs.cp(path.join(WEB,'public/fonts/lids'),path.join(pkg,'fonts'),{recursive:true});
 await fs.copyFile(path.join(ROOT,'docs/governance/ui-design-execution.md'),path.join(pkg,'AI-EXECUTION.md'));
 execFileSync('python3',['-c','import pathlib,zipfile,sys;root=pathlib.Path(sys.argv[1]);z=zipfile.ZipFile(sys.argv[2],"w",zipfile.ZIP_DEFLATED);[z.write(p,p.relative_to(root)) for p in sorted(root.rglob("*")) if p.is_file()];z.close()',pkg,path.join(OUT,'LIDS-Design-System-Package.zip')],{cwd:ROOT});
 console.log(`LIDS HTML: ${path.join(OUT,name)}\nSource: ${source.fingerprint}`);
}
async function main(){
 const [command,...args]=process.argv.slice(2);
 if(command==='record'){
  const arg=k=>{const i=args.indexOf('--'+k);if(i<0||!args[i+1]||args[i+1].startsWith('--'))throw new Error('Missing --'+k);return args[i+1]};const c=await contracts();
  const record={kind:arg('kind'),reason:arg('reason'),authorization:arg('authorization'),exceptionScope:args.includes('--scope')?arg('scope'):undefined,componentFingerprints:await componentFingerprints(c)};
  validateChange(record,record.componentFingerprints);await put('docs/design/changes/lids-component-change.json',json(record));console.log('Recorded explicit component change classification');
 }else if(command==='sync'){
  const c=await contracts();validateChange(await readJson('docs/design/changes/lids-component-change.json'),await componentFingerprints(c));await checkBindings(c);
  execFileSync('python3',['scripts/sync-lids-runtime.py'],{cwd:ROOT,stdio:'pipe'});for(const [p,value] of await generated(c))await put(p,value);
  if(!args.includes('--no-build'))await buildExport();console.log('LIDS sources, runtime, catalog and real-component examples synchronized');
 }else if(command==='check'){await check();console.log('LIDS sync: PASS');
 }else if(command==='build')await buildExport();
 else if(command==='item') {const c=await contracts(),id=args[0];if(!c.components[id])throw new Error('Unknown closed-loop component '+id);console.log(json(c.components[id]));}
 else if(command==='serve'){
  await check();const {default:http}=await import('node:http');http.createServer(async(req,res)=>{
   if(!['GET','HEAD'].includes(req.method)){res.writeHead(405);res.end();return;}
   const url=new URL(req.url,'http://127.0.0.1');if(url.pathname==='/favicon.ico'){res.writeHead(204);res.end();return;}if(url.pathname==='/__preview'){
    const width=['375','390','820','1024','1440'].includes(url.searchParams.get('width'))?url.searchParams.get('width'):'390';
    res.writeHead(200,{'Content-Type':'text/html; charset=utf-8'});res.end(`<html><meta charset="utf-8"><title>LIDS narrow</title><style>body{margin:0;background:#eee}iframe{display:block;margin:auto;width:${width}px;height:950px;border:0}</style><iframe title="LIDS ${width}" src="/"></iframe></html>`);return;
   }
   if(url.pathname!=='/'&&url.pathname!=='/Linggan-Intelligence-Design-System.html'){res.writeHead(404);res.end();return;}
   const data=await fs.readFile(path.join(OUT,'Linggan-Intelligence-Design-System.html'));res.writeHead(200,{'Content-Type':'text/html; charset=utf-8','Cache-Control':'no-store'});res.end(req.method==='HEAD'?undefined:data);
  }).listen(4188,'127.0.0.1',()=>console.log('Current LIDS: http://127.0.0.1:4188/ (no API proxy)'));
 }else throw new Error('Use record --kind <implementation_fix|shared_standard|page_exception> --reason <text> --authorization <source> [--scope <bounded page>], sync, check, build, item C09|C16, or serve');
}
if(process.argv[1]&&path.resolve(process.argv[1])===fileURLToPath(import.meta.url))main().catch(e=>{console.error(e.message);process.exitCode=1});
