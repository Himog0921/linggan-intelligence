import { readFileSync } from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import { REACT_PAGE_PATHS } from './pages';

// 服务器只为登记过的路径返回本应用：两边登记表必须逐项相同。
// 测试在 apps/web 目录下运行（npm test / npm --prefix apps/web test）。
const serverSource = readFileSync(path.resolve(process.cwd(), '../api/src/local_web/web_app.rs'), 'utf8');

function serverPages(): string[] {
  const block = /const REACT_PAGES: &\[&str\] = &\[([\s\S]*?)\];/.exec(serverSource);
  if (!block?.[1]) throw new Error('web_app.rs 里找不到 REACT_PAGES');
  return [...block[1].matchAll(/"([^"]+)"/g)].map(match => match[1] ?? '');
}

describe('页面登记表', () => {
  it('前端路由与服务器登记逐项相同', () => {
    expect([...REACT_PAGE_PATHS].sort()).toEqual(serverPages().sort());
  });
});
