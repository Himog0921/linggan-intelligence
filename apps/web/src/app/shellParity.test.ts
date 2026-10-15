import { readFileSync } from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import { PERSONAL_MENU_LINKS, PRIMARY_ENTRIES, type PrimarySurface } from './navigation';

// 迁移期旧页面的页头由 shell.rs 输出，React 页头必须与它一致（WEB-REACT-001 R4）。
// 测试在 apps/web 目录下运行（npm test / npm --prefix apps/web test）。
const shellSource = readFileSync(path.resolve(process.cwd(), '../api/src/local_web/shell.rs'), 'utf8');

const SURFACES: Record<string, PrimarySurface> = {
  Corpus: 'corpus',
  Collection: 'collection',
  Topic: 'topic',
  Settings: 'settings',
  Ops: 'ops',
};

function legacyPrimaryEntries() {
  const block = /const PRIMARY_ENTRIES: \[PrimaryEntry; \d+\] = \[([\s\S]*?)\n\];/.exec(shellSource);
  if (!block?.[1]) throw new Error('shell.rs 里找不到 PRIMARY_ENTRIES');
  return block[1]
    .split('PrimaryEntry {')
    .slice(1)
    .map(entry => {
      const text = (name: string) => {
        const match = new RegExp(`${name}: "([^"]*)"`).exec(entry);
        if (!match) throw new Error(`shell.rs 的导航项缺少 ${name}`);
        return match[1] ?? '';
      };
      const surface = /surface: (?:None|Some\(PrimarySurface::(\w+)\))/.exec(entry);
      const href = /href: (?:None|Some\("([^"]*)"\))/.exec(entry);
      if (!surface || !href) throw new Error('shell.rs 的导航项缺少 surface 或 href');
      return {
        zh: text('zh'),
        technicalKey: text('technical_key'),
        state: text('state'),
        surface: surface[1] ? (SURFACES[surface[1]] ?? null) : null,
        href: href[1] ?? null,
      };
    });
}

describe('React 页头与 shell.rs 一致', () => {
  it('一级导航逐项相同（名称、技术旁注、状态词、所属责任、链接）', () => {
    expect(PRIMARY_ENTRIES).toEqual(legacyPrimaryEntries());
  });

  it('个人菜单入口相同', () => {
    for (const link of PERSONAL_MENU_LINKS) {
      expect(shellSource).toContain(`<a href="${link.href}">${link.label}</a>`);
    }
  });

  it('品牌区文字相同', () => {
    expect(shellSource).toContain('领域情报工作台');
    expect(shellSource).toContain('EDITORIAL INTELLIGENCE');
  });
});
