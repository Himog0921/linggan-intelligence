import { createHash } from 'node:crypto';
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
// main.tsx imports this canonical runtime source directly. A var() with an invented
// name is not compliant: its declaration can disappear and leave invisible UI.
const runtimeTokens = readFileSync(path.resolve(process.cwd(), 'src/styles/lids_tokens.css'), 'utf8');
const definedTokens = new Set([...runtimeTokens.matchAll(/(--lgi-[\w-]+)\s*:/g)].map(match => match[1]));

function unknownTokenReferences(css: string): string[] {
  const source = css.replace(/\/\*[\s\S]*?\*\//g, '');
  return [...new Set([...source.matchAll(/var\(\s*(--lgi-[\w-]+)/g)]
    .map(match => match[1] ?? '').filter(token => !definedTokens.has(token)))].sort();
}

function moduleStylesheets(): string[] {
  return readdirSync(sourceRoot, { recursive: true, encoding: 'utf8' })
    .filter(file => file.endsWith('.module.css'))
    .map(file => path.join(sourceRoot, file));
}

function violationsIn(source: string): string[] {
  const css = source.replace(/\/\*[\s\S]*?\*\//g, '');
  const found: string[] = [];
  for (const declaration of css.matchAll(/([a-z-]+)\s*:\s*([^;{}]+)(?=[;}])/g)) {
    const property = declaration[1] ?? '';
    const value = declaration[2] ?? '';
    if (COLOR_LITERAL.test(value)) found.push(`${property}: ${value}`);
    if (SIZED_PROPERTY.test(property) && LITERAL_LENGTH.test(value)) found.push(`${property}: ${value}`);
  }
  return found;
}

const violations = (file: string) => violationsIn(readFileSync(file, 'utf8'));

describe('页面样式只用 LIDS Token', () => {
  const files = moduleStylesheets();

  it('至少检查到一个页面样式文件', () => {
    expect(files.length).toBeGreaterThan(0);
  });

  it.each(files)('%s', file => {
    if (path.basename(file) === 'CollectionSurface.module.css') {
      // Mog 2026-10-16: preserve the approved Collection UI exactly during migration.
      // Only the moved, frozen legacy sheet carries its existing geometry; new styles use tokens.
      expect(createHash('sha256').update(readFileSync(file)).digest('hex')).toBe('a8c6407f8741ad615e1f8ed235d544393d243a4164ee35c27661e6a3ff9ce1d6');
      return;
    }
    if (path.basename(file) === 'Login.module.css') {
      // LOGIN-ANATOMY-001 approved local palette/geometry, preserved by H actual screenshot proof.
      expect(createHash('sha256').update(readFileSync(file)).digest('hex')).toBe('b6503ee8595bb62a766cd3033ad1ef8350d9e32924ddf1161d508d67722a7ebe');
      return;
    }
    if (file === path.join(sourceRoot, 'pages', 'corpus', 'evidence', 'DossierDocument.module.css')) {
      // RESEARCH-DOSSIER-READING-20261027: Mog's approved demo is a local reading surface.
      // See evidence-library-page.md. Freeze this sheet; all other pages keep LIDS token checks.
      expect(createHash('sha256').update(readFileSync(file)).digest('hex')).toBe('8d51b68f332b1202ae4ccaa7b6aa711b98dc64cdbdf215ec1b8464137363369d');
      return;
    }
    expect(violations(file)).toEqual([]);
  });

  it.each(files)('所有 LIDS 引用均有运行时定义：%s', file => {
    expect(unknownTokenReferences(readFileSync(file, 'utf8'))).toEqual([]);
  });

  it('未知 var() 反例必须失败，即使写有合法回退；注释与已定义引用不误报', () => {
    expect(unknownTokenReferences('.action { background: var(--lgi-unknown-reference-proof, var(--lgi-canvas)); }'))
      .toEqual(['--lgi-unknown-reference-proof']);
    expect(unknownTokenReferences('/* var(--lgi-unknown-reference-proof) */ .action { color: var(--lgi-ink); }'))
      .toEqual([]);
  });

  it('末项无分号与有分号的硬编码都必须失败', () => {
    expect(violationsIn('.a { height:248px }')).toEqual(['height: 248px ']);
    expect(violationsIn('.a { height:248px; }')).toEqual(['height: 248px']);
    expect(violationsIn('.a { height:calc(var(--lgi-space-1) * 62) }')).toEqual([]);
  });

  it('检查本身能识别违规写法', () => {
    const sample = path.join(sourceRoot, 'app', 'NotFoundPage.module.css');
    expect(violations(sample)).toEqual([]);
    expect(COLOR_LITERAL.test('#ef4f25')).toBe(true);
    expect(SIZED_PROPERTY.test('padding-left') && LITERAL_LENGTH.test('12px')).toBe(true);
    expect(LITERAL_LENGTH.test('var(--lgi-space-2)')).toBe(false);
  });
});
