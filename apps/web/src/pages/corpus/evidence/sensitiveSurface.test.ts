import { readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

/**
 * 证据库页面的敏感面：评论原文、平台用户标识与媒体只经受控的详情读取与同源句柄；页面源码里不出现
 * 把任意字符串变成 HTML 的写法，不直接发请求，也不带回已经移除的旧入口。旧页用字符串断言守这些
 * （`evidence_runtime_keeps_sensitive_text_and_media_inside_controlled_detail_reads` 等），这里对渲染源码做同样的扫描。
 * 测试在 apps/web 目录下运行（npm test / npm --prefix apps/web test）。
 */
const directory = path.resolve(process.cwd(), 'src/pages/corpus/evidence');
const ALLOWED_FAKES = /^(evidenceFixtures|testKit|pageKit)\./;

function stripComments(source: string): string {
  return source.replace(/\/\*[\s\S]*?\*\//g, '').replace(/(^|[^:'"`])\/\/.*$/gm, '$1');
}

const sources = readdirSync(directory)
  .filter(file => /\.(ts|tsx)$/.test(file) && !/\.test\.(ts|tsx)$/.test(file) && !ALLOWED_FAKES.test(file))
  .map(file => ({ file, code: stripComments(readFileSync(path.join(directory, file), 'utf8')) }));

function offenders(pattern: RegExp): string[] {
  return sources.filter(({ code }) => pattern.test(code)).map(({ file }) => file);
}

describe('页面源码的敏感面', () => {
  it('至少扫描到渲染源码', () => {
    expect(sources.length).toBeGreaterThan(40);
  });

  it('没有把字符串变成 HTML 的写法：dangerouslySetInnerHTML、innerHTML、outerHTML、insertAdjacentHTML、document.write、eval', () => {
    expect(offenders(/dangerouslySetInnerHTML|\binnerHTML\b|\bouterHTML\b|insertAdjacentHTML|document\.write|\beval\s*\(|new Function\s*\(/)).toEqual([]);
  });

  it('不引用平台用户标识：authorExternalId 与作品的外部编号不在任何渲染代码里（底稿导出用服务器原样给的对象，不按名字取）', () => {
    expect(offenders(/authorExternalId|contentExternalId|externalId/)).toEqual([]);
  });

  it('页面代码不直接发请求：全部经 shared/api 的 requestJson，没有 fetch、XMLHttpRequest、sendBeacon、WebSocket、EventSource', () => {
    expect(offenders(/\bfetch\s*\(|XMLHttpRequest|sendBeacon|new WebSocket|new EventSource/)).toEqual([]);
  });

  it('没有跨源地址：除了解析用的占位基址，代码里不写 http(s) 地址', () => {
    const found = sources.flatMap(({ file, code }) => [...code.matchAll(/https?:\/\/[^\s'"`)]+/g)].map(match => `${file}: ${match[0]}`));
    expect(found.sort()).toEqual(['evidenceApi.ts: http://local.invalid', 'evidencePaths.ts: http://localhost'].sort());
  });

  it('不带回已移除的旧入口与旧写法：旧跨行业接口、旧卡片、「ACK 完整」、浏览器里的评论原文持久化', () => {
    expect(offenders(/api\/local\/evidence-library\/legacy|api\/local\/cross-industry|crossIndustry|ACK 完整/)).toEqual([]);
  });

  it('只写三类本机存储键：检查器宽度偏好、评论深采交接、批量整理断点；评论与作品内容不进存储', () => {
    const keys = sources.flatMap(({ file, code }) => [...code.matchAll(/(?:localStorage|sessionStorage)\.(?:setItem|getItem|removeItem)\(\s*([^,)]+)/g)].map(match => `${file}:${match[1]?.trim()}`));
    expect(keys.sort()).toEqual(
      [
        'batchStore.ts:BATCH_KEY',
        'batchStore.ts:BATCH_KEY',
        'usePanelWidthPreference.ts:STORAGE_KEY',
        'usePanelWidthPreference.ts:STORAGE_KEY',
        'useCollectionSelection.ts:selectionKey(domain',
        'useCollectionSelection.ts:selectionKey(domain',
      ].sort(),
    );
    expect(offenders(/document\.cookie|indexedDB|caches\.open/)).toEqual([]);
  });
});
