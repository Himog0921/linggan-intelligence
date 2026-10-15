import { readFileSync } from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import { CORPUS_ENTRIES, type CorpusPageKey } from './corpusPages';

// 迁移期证据库、评论研究仍由旧页面提供，它们的侧栏由 shell.rs 输出，React 侧栏必须与它一致（R4）。
// 测试在 apps/web 目录下运行（npm test / npm --prefix apps/web test）。
const shellSource = readFileSync(path.resolve(process.cwd(), '../api/src/local_web/shell.rs'), 'utf8');

const PAGES: Record<string, CorpusPageKey> = { Evidence: 'evidence', Comments: 'comments', Creators: 'creators' };

function legacyCorpusEntries() {
  const block = /const CORPUS_ENTRIES: \[CorpusEntry; \d+\] = \[([\s\S]*?)\n\];/.exec(shellSource);
  if (!block?.[1]) throw new Error('shell.rs 里找不到 CORPUS_ENTRIES');
  return block[1]
    .split('CorpusEntry {')
    .slice(1)
    .map(entry => {
      const text = (name: string) => {
        const match = new RegExp(`${name}: "([^"]*)"`).exec(entry);
        if (!match) throw new Error(`shell.rs 的语料导航项缺少 ${name}`);
        return match[1] ?? '';
      };
      const href = /href: (?:None|Some\("([^"]*)"\))/.exec(entry);
      const page = /page: (?:None|Some\(CorpusPage::(\w+)\))/.exec(entry);
      if (!href || !page) throw new Error('shell.rs 的语料导航项缺少 href 或 page');
      return {
        ordinal: text('ordinal'),
        label: text('label'),
        href: href[1] ?? null,
        page: page[1] ? (PAGES[page[1]] ?? null) : null,
      };
    });
}

describe('React 语料侧栏与 shell.rs 一致', () => {
  it('三项逐项相同（序号、名称、链接、所属页面）', () => {
    expect(CORPUS_ENTRIES).toEqual(legacyCorpusEntries());
  });

  it('侧栏标记的固定部分相同：无障碍名称、类名、证据库计数徽标位、底部区', () => {
    for (const snippet of [
      'aria-label="语料导航"',
      'class="v7-side-nav"',
      'aria-current="page"',
      '<i>{ordinal}</i>',
      '<b class="ev-rail-count" id="ev-rail-count" hidden></b>',
      '<div class="v7-side-foot">{foot}</div>',
      '{href}?domain={domain_ref}',
    ]) {
      expect(shellSource, snippet).toContain(snippet);
    }
  });

  it('没有已提供路由的项保持禁用而不是死链（shell.rs 的规则）', () => {
    expect(shellSource).toContain('aria-disabled="true"');
    expect(CORPUS_ENTRIES.every(entry => entry.href !== null)).toBe(true);
  });
});
