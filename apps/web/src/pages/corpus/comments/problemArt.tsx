/** Decorative banner art for problem archive cards (B-04). Everything here is aria-hidden and carries no data. */

const PIXELS: Record<string, readonly string[]> = {
  '0': ['11111', '10001', '10011', '10101', '11001', '10001', '11111'], '1': ['00100', '01100', '00100', '00100', '00100', '00100', '01110'],
  '2': ['11110', '00001', '00001', '01110', '10000', '10000', '11111'], '3': ['11110', '00001', '00001', '01110', '00001', '00001', '11110'],
  '4': ['10010', '10010', '10010', '11111', '00010', '00010', '00010'], '5': ['11111', '10000', '10000', '11110', '00001', '00001', '11110'],
  '6': ['01111', '10000', '10000', '11110', '10001', '10001', '01110'], '7': ['11111', '00001', '00010', '00100', '01000', '01000', '01000'],
  '8': ['01110', '10001', '10001', '01110', '10001', '10001', '01110'], '9': ['01110', '10001', '10001', '01111', '00001', '00001', '11110'],
};

export function PixelCode({ code }: { code: string }) {
  const cell = 6.1, rects: { x: number; y: number }[] = [];
  [...code].forEach((ch, ci) => (PIXELS[ch] ?? PIXELS['0']!).forEach((row, ri) => [...row].forEach((bit, bi) => {
    if (bit === '1') rects.push({ x: ci * 37.8 + bi * cell, y: ri * cell });
  })));
  const width = code.length * 37.8 - 7.3;
  return <svg viewBox={`0 0 ${width} 44`} aria-hidden="true">
    <g style={{ fill: 'var(--lgi-cr-banner-pixel)' }} opacity=".96">
      {rects.map(r => <rect key={`${r.x}:${r.y}`} x={r.x} y={r.y} width={cell - 0.7} height={cell - 0.7} rx=".3" />)}
    </g>
  </svg>;
}

const DARK = 'var(--lgi-cr-particle-dark)', LIGHT = 'var(--lgi-cr-particle-light)', MARK = 'var(--lgi-signal-mark)';
const DOTS: readonly [number, number, number, string, number][] = [
  [250, 26, 1.0, DARK, .46], [253, 34, .8, MARK, .48], [256, 20, .75, LIGHT, .45],
  [261, 30, .95, DARK, .44], [266, 23, .75, DARK, .38], [269, 38, .8, LIGHT, .38],
  [275, 18, .85, DARK, .42], [278, 31, 1.05, DARK, .48], [284, 25, .8, MARK, .42],
  [290, 37, .75, DARK, .36], [296, 20, .9, DARK, .4], [301, 29, .95, LIGHT, .36],
  [307, 17, .8, DARK, .36], [312, 34, .85, DARK, .42], [318, 25, .75, MARK, .4],
  [324, 39, .8, DARK, .32], [330, 22, .85, LIGHT, .34], [336, 31, .75, DARK, .34],
  [342, 26, .7, DARK, .28], [348, 36, .65, LIGHT, .28],
  [258, 43, .65, LIGHT, .3], [272, 13, .65, DARK, .28], [287, 44, .7, MARK, .32],
  [304, 43, .65, DARK, .27], [321, 13, .65, DARK, .28], [339, 17, .6, LIGHT, .27],
];

export function ParticleWake({ className }: { className: string }) {
  return <svg className={className} viewBox="0 0 420 56" preserveAspectRatio="none" aria-hidden="true">
    <g>{DOTS.map(([x, y, r, fill, opacity]) => <circle key={`${x}:${y}`} cx={x} cy={y} r={r} style={{ fill }} opacity={opacity} />)}</g>
  </svg>;
}
