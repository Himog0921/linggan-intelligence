import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

describe('正式LIDS源与交付资产',()=>{
 it('唯一运行Token必须由已确认源再生，字体、B、C、M2、A和L1无手写漂移',()=>{
  expect(execFileSync('python3',['../../scripts/sync-lids-runtime.py','--check'],{encoding:'utf8'})).toContain('PASS');
 });
 it('发布的字体和许可逐字节匹配登记，CSS使用swap和现有/ui-assets路径',()=>{
  const root=path.resolve('public/fonts/lids');
  const manifest: {files:{file:string;sha256:string;bytes:number}[]} = JSON.parse(readFileSync(path.join(root,'manifest.json'),'utf8'));
  expect(manifest.files).toHaveLength(10);
  for(const file of manifest.files){const bytes=readFileSync(path.join(root,file.file));expect(bytes.length).toBe(file.bytes);expect(createHash('sha256').update(bytes).digest('hex')).toBe(file.sha256);}
  const css=readFileSync('src/styles/typography.css','utf8');expect(css.match(/font-display:swap/g)).toHaveLength(6);
  expect(css).toContain('/ui-assets/fonts/lids/opposans/OPPOSans40.ttf');
 });
 it('CSS里每个字体URL都是已登记且已交付的文件，登记的字体也都被CSS使用（任一字体路径漂移都会让浏览器静默回退系统字体）',()=>{
  const root=path.resolve('public/fonts/lids');
  const manifest: {files:{file:string}[]} = JSON.parse(readFileSync(path.join(root,'manifest.json'),'utf8'));
  const css=readFileSync('src/styles/typography.css','utf8');
  const prefix='/ui-assets/fonts/lids/';
  const urls=new Set([...css.matchAll(/url\('([^']+)'\)/g)].flatMap(match=>match[1]??[]));
  for(const url of urls)expect(url.startsWith(prefix),url).toBe(true);
  const referenced=[...urls].map(url=>url.slice(prefix.length)).sort();
  const registered=manifest.files.map(entry=>entry.file).filter(file=>/\.(woff2|ttf)$/.test(file)).sort();
  expect(referenced).toEqual(registered);
  for(const file of referenced)expect(existsSync(path.join(root,file)),file).toBe(true);
 });
});
