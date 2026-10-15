import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

// LIDS 约束：页面样式只能消费 --lgi-* Token，不写死颜色、字号、行高和间距。
// 边框与描边的线宽（1px/2px/3px）没有对应 Token，允许直接写。
// 测试在 apps/web 目录下运行（npm test / npm --prefix apps/web test）。
const sourceRoot = `${path.resolve(process.cwd(), 'src')}${path.sep}`;
const SIZED_PROPERTY =
  /^(margin|padding|gap|row-gap|column-gap|font|font-size|line-height|letter-spacing|inset|top|right|bottom|left|width|height|min-width|min-height|max-width|max-height|border-radius)(-[a-z-]+)?$/;
const LITERAL_LENGTH = /(?<![\w-])\d*\.?\d+(px|rem|em)\b/;
const COLOR_LITERAL = /#[0-9a-fA-F]{3,8}\b|\brgba?\(|\bhsla?\(/;

function moduleStylesheets(): string[] {
  return readdirSync(sourceRoot, { recursive: true, encoding: 'utf8' })
    .filter(file => file.endsWith('.module.css'))
    .map(file => path.join(sourceRoot, file));
}

function violations(file: string): string[] {
  const css = readFileSync(file, 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
  const found: string[] = [];
  for (const declaration of css.matchAll(/([a-z-]+)\s*:\s*([^;{}]+);/g)) {
    const property = declaration[1] ?? '';
    const value = declaration[2] ?? '';
    if (COLOR_LITERAL.test(value)) found.push(`${property}: ${value}`);
    if (SIZED_PROPERTY.test(property) && LITERAL_LENGTH.test(value)) found.push(`${property}: ${value}`);
  }
  return found;
}

describe('页面样式只用 LIDS Token', () => {
  const files = moduleStylesheets();

  it('至少检查到一个页面样式文件', () => {
    expect(files.length).toBeGreaterThan(0);
  });

  it.each(files)('%s', file => {
    expect(violations(file)).toEqual([]);
  });

  it('检查本身能识别违规写法', () => {
    const sample = path.join(sourceRoot, 'app', 'NotFoundPage.module.css');
    expect(violations(sample)).toEqual([]);
    expect(COLOR_LITERAL.test('#ef4f25')).toBe(true);
    expect(SIZED_PROPERTY.test('padding-left') && LITERAL_LENGTH.test('12px')).toBe(true);
    expect(LITERAL_LENGTH.test('var(--lgi-space-2)')).toBe(false);
  });
});
