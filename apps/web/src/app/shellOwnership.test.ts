import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { describe, expect, it } from 'vitest';

/** 共享外壳（AppShell / ContextBar / Sidebar）的样式只能住在 shell.css 与它的材质表里。 */
const OWNED_SELECTORS = [
  '.v7-side-head',
  '.v7-side-navigation',
  '.v7-nav-group',
  '.v7-nav-subnav',
  '.v7-side-subnav-item',
  '.v7-context-row',
  '.v7-context-main',
  '.v7-context-crumb',
  '.v7-context-meta',
  '.v7-context-section-nav',
  '.v7-tertiary-nav',
  '.v7-tertiary-select',
  '.v7-context-action',
  '.v7-domain-picker',
  '.v7-annotated-current',
];
const OWNERS = new Set(['styles/shell.css', 'app/ContextTexture.module.css']);

// 从当前目录向上找到 web 应用根（含 src/styles/shell.css），本地和 CI 从哪一层启动都成立。
function webRoot(): string {
  for (let directory = process.cwd(); ; directory = dirname(directory)) {
    if (existsSync(join(directory, 'src/styles/shell.css'))) return directory;
    if (dirname(directory) === directory) throw new Error('找不到 apps/web 根目录');
  }
}

function cssFiles(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
    const path = join(directory, entry.name);
    return entry.isDirectory() ? cssFiles(path) : entry.name.endsWith('.css') ? [path] : [];
  });
}

describe('共享外壳样式越权检查', () => {
  it('页面样式不覆盖共享页头选择器；要改就改 shell.css', () => {
    const root = join(webRoot(), 'src');
    const sources = cssFiles(root).map(file => ({ file: relative(root, file), text: readFileSync(file, 'utf8') }));
    // 文件读空会让下面的检查永远通过；先确认真的读到了共享表自己的选择器。
    expect(sources.find(entry => entry.file === 'styles/shell.css')?.text).toContain('.v7-context-row');

    const violations = sources
      .filter(entry => !OWNERS.has(entry.file))
      .flatMap(entry =>
        OWNED_SELECTORS.filter(selector => entry.text.includes(selector)).map(selector => `${entry.file} → ${selector}`),
      );
    expect(violations).toEqual([]);
  });
});
