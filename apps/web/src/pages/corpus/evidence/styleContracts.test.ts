import { readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

/**
 * 样式里「必须在」的几条契约。写死值与 Token 引用由 `src/test/tokenDiscipline.test.ts` 管；这里管的是旧页用字符串
 * 断言守的那几条行为性样式：悬停不上浮、减少动态效果、移动断点、封面 3:4。jsdom 不计算层叠，真实呈现由四个视口的
 * 静止态截图与计算样式探针（e2e）核对。测试在 apps/web 目录下运行。
 */
const directory = path.resolve(process.cwd(), 'src/pages/corpus/evidence');
const styles = new Map(
  readdirSync(directory)
    .filter(file => file.endsWith('.module.css'))
    .map(file => [file, readFileSync(path.join(directory, file), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '')] as const),
);

/** 选择器里带 :hover 的规则体。 */
function hoverBodies(css: string): { selector: string; body: string }[] {
  return [...css.matchAll(/([^{}]*:hover[^{}]*)\{([^{}]*)\}/g)].map(match => ({ selector: (match[1] ?? '').trim(), body: match[2] ?? '' }));
}

describe('样式契约', () => {
  it('悬停不上浮：悬停规则里没有位移、缩放；封面翻转卡的旋转是唯一的悬停形变（EVIDENCE-COVER-FLIP-CARD-001）', () => {
    const offenders: string[] = [];
    for (const [file, css] of styles) {
      for (const { selector, body } of hoverBodies(css)) {
        const moves = /translate|scale|(^|[;\s])(top|bottom|left|right|margin[a-z-]*)\s*:/.test(body);
        const rotation = /rotateY\(/.test(body) && file === 'CoverCard.module.css';
        if (moves && !rotation) offenders.push(`${file}: ${selector}`);
      }
    }
    expect(offenders).toEqual([]);
  });

  it('封面翻转是旋转；减少动态效果时直接切换（不做过渡）', () => {
    const cover = styles.get('CoverCard.module.css') ?? '';
    expect(cover).toMatch(/rotateY\(180deg\)/);
    expect(cover).toMatch(/backface-visibility:\s*hidden/);
    expect(cover).toMatch(/@media \(prefers-reduced-motion: reduce\)[\s\S]*transition:\s*none/);
    expect(cover).toMatch(/@media \(hover: hover\)\s*\{\s*\.visual:hover \.flip/);
  });

  it('检查器：抽屉断点 1180px、窄屏 900px；减少动态效果', () => {
    const page = styles.get('EvidencePage.module.css') ?? '';
    const inspector = styles.get('Inspector.module.css') ?? '';
    expect(page).toMatch(/@media \(max-width: 1180px\)/);
    expect(page).toMatch(/@media \(max-width: 900px\)/);
    expect(inspector).toMatch(/@media \(max-width: 1180px\)/);
    expect(inspector).toMatch(/@media \(prefers-reduced-motion: reduce\)/);
  });

  it('列表在窄屏有断点；媒体轨道与查看器有减少动态效果的分支；查看器有 640px 断点', () => {
    expect(styles.get('WorkList.module.css')).toMatch(/@media \(max-width: 640px\)/);
    expect(styles.get('MediaRail.module.css')).toMatch(/@media \(prefers-reduced-motion: reduce\)/);
    expect(styles.get('Lightbox.module.css')).toMatch(/@media \(max-width: 640px\)/);
  });

  it('封面预览是 3:4：研读行与表格里的预览（Media）、封面卡的翻转舞台与背面（CoverCard）都按小红书竖封面比例', () => {
    const ratios = ['Media.module.css', 'CoverCard.module.css'].map(file => (styles.get(file) ?? '').match(/aspect-ratio:\s*3\s*\/\s*4/g)?.length ?? 0);
    expect(ratios[0]).toBeGreaterThanOrEqual(1);
    expect(ratios[1]).toBeGreaterThanOrEqual(2);
  });

  it('封面排版是多列栅格：列宽不小于 248px 自适应列数；≤640px 两列、≤420px 一列（缺了它封面卡会退化成一列通栏）', () => {
    const list = styles.get('WorkList.module.css') ?? '';
    expect(list).toMatch(/\.list\[data-layout='cover'\] \.items\s*\{[^}]*display:\s*grid[^}]*repeat\(auto-fill, minmax\(calc\(var\(--lgi-space-24\) \* 2 \+ var\(--lgi-space-12\) \+ var\(--lgi-space-2\)\), 1fr\)\)/);
    expect(list).toMatch(/@media \(max-width: 640px\)\s*\{\s*\.list\[data-layout='cover'\] \.items\s*\{\s*grid-template-columns:\s*repeat\(2, minmax\(0, 1fr\)\)/);
    expect(list).toMatch(/@media \(max-width: 420px\)\s*\{\s*\.list\[data-layout='cover'\] \.items\s*\{\s*grid-template-columns:\s*1fr/);
  });

  it('关闭检查器是让这一栏塌缩，不是隐藏元素：栏宽是一个变量，三档加关闭', () => {
    const page = styles.get('EvidencePage.module.css') ?? '';
    expect(page).toMatch(/--ev-inspector-w/);
    for (const state of ['wide', 'focus', 'closed']) expect(page).toContain(`data-inspector='${state}'`);
  });
});
