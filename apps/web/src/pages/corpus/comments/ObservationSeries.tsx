import { useId, useLayoutEffect, useRef, useState } from 'react';
import { EmptyState } from '../../../shared/EmptyState';
import type { ObservationSeries as Series } from './overviewDtos';
import { RANGES, rangePoints, type SeriesRange } from './researchModel';
import { c } from './styles';

const COVERAGE: Record<string, string> = { recorded: '完整', partial: '部分', none: '无记录' };
const coverageLabel = (value: string | null) => COVERAGE[value ?? ''] ?? '未知';
const slashDate = (date: string) => date.replaceAll('-', '/');

interface Day { date: string; comments: number; signals: number; coverage: string }

function useBox<T extends HTMLElement>() {
  const ref = useRef<T>(null);
  const [box, setBox] = useState({ width: 0, height: 0 });
  useLayoutEffect(() => {
    const node = ref.current;
    if (!node) return;
    const update = () => { const rect = node.getBoundingClientRect(); setBox(old => old.width === Math.round(rect.width) && old.height === Math.round(rect.height) ? old : { width: Math.round(rect.width), height: Math.round(rect.height) }); };
    update();
    if (typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(update);
    observer.observe(node);
    return () => observer.disconnect();
  }, []);
  return [ref, box] as const;
}

/** B-04 trend: ink bars (first observed comments) + signal curve with fading area, drawn at the container's real pixel size. */
function Chart({ rows, range }: { rows: Day[]; range: SeriesRange }) {
  const [ref, box] = useBox<HTMLDivElement>();
  const gradient = useId().replace(/:/g, '');
  const W = Math.max(320, box.width || 1000), H = Math.max(150, box.height || 194);
  const ceil = (n: number) => Math.ceil(n / 2) * 2;
  const cMax = ceil(Math.max(2, ...rows.map(r => r.comments))), sMax = ceil(Math.max(2, ...rows.map(r => r.signals)));
  const left = 40, right = W - 40, top = 30, bottom = H - 46, w = right - left, h = bottom - top, step = w / rows.length, band = bottom + 9, tick = H - 10;
  const points = rows.map((r, i) => [left + (i + .5) * step, bottom - h * r.signals / sMax] as const);
  let path = `M${points[0]![0]},${points[0]![1]}`;
  for (let i = 1; i < points.length; i++) { const a = points[i - 1]!, b = points[i]!, mid = (a[0] + b[0]) / 2; path += ` C${mid},${a[1]} ${mid},${b[1]} ${b[0]},${b[1]}`; }
  const area = `${path} L${points.at(-1)![0]},${bottom} L${points[0]![0]},${bottom} Z`;
  const every = rows.length <= 14 ? 1 : Math.ceil((rows.length - 1) / 6);
  const ticks = rows.map((_, i) => i).filter(i => (rows.length - 1 - i) % every === 0);
  const cTotal = rows.reduce((n, r) => n + r.comments, 0), sTotal = rows.reduce((n, r) => n + r.signals, 0);
  const barWidth = Math.min(10, step * .42);
  const label = `${range === 'fit' ? '自最早数据起' : `近${range}天`}，首次观察${cTotal}条评论，写入${sTotal}条研究信号。可切换表格查看逐日数据。`;
  return <div ref={ref} className={c('chart-wrap')}>
    <svg viewBox={`0 0 ${W} ${H}`} width={W} height={H} role="img" aria-label={label}>
      <defs><linearGradient id={gradient} x1="0" y1="0" x2="0" y2="1"><stop offset="0%" style={{ stopColor: 'var(--lgi-signal)' }} stopOpacity=".17" /><stop offset="100%" style={{ stopColor: 'var(--lgi-signal)' }} stopOpacity="0" /></linearGradient></defs>
      <g className={c('chart-axis')}><text x="0" y="12">评论</text><text x={W} y="12" textAnchor="end">信号</text>
        {[0, .5, 1].map(f => <g key={f}><path d={`M${left} ${bottom - h * f}H${right}`} style={{ stroke: 'var(--lgi-hairline)' }} strokeWidth="1" />
          <text x={left - 8} y={bottom - h * f + 4} textAnchor="end">{cMax * f}</text><text x={right + 8} y={bottom - h * f + 4}>{sMax * f}</text></g>)}
      </g>
      <path d={area} fill={`url(#${gradient})`} />
      {rows.map((r, i) => r.comments ? <rect key={r.date} x={left + (i + .5) * step - barWidth / 2} y={bottom - h * r.comments / cMax} width={barWidth} height={h * r.comments / cMax} style={{ fill: 'var(--lgi-ink)' }}><title>{`${r.date} · 首次观察 ${r.comments} 条评论`}</title></rect> : null)}
      <path d={path} style={{ stroke: 'var(--lgi-signal)' }} strokeWidth="2" fill="none" />
      {rows.map((r, i) => <rect key={r.date} x={left + i * step} y={band} width={Math.max(1, step - 1)} height="5"
        style={{ fill: r.coverage === '完整' ? 'var(--lgi-body)' : r.coverage === '部分' ? 'var(--lgi-warning-dot)' : 'var(--lgi-unknown-soft)' }}><title>{`${r.date} · ${r.coverage}`}</title></rect>)}
      <g className={c('chart-axis')}>{ticks.map(i => <text key={i} x={left + (i + .5) * step} y={tick} textAnchor="middle">{rows[i]!.date.slice(5)}</text>)}</g>
    </svg>
  </div>;
}

export function ObservationSeries({ series, range, view, onRange, onView }: {
  series: Series | null; range: SeriesRange; view: 'chart' | 'table';
  onRange: (range: SeriesRange) => void; onView: (view: 'chart' | 'table') => void;
}) {
  const points = rangePoints(series?.points ?? [], range);
  const rows: Day[] = points.map(p => ({ date: p.date, comments: Math.max(0, p.newObservedCommentCount ?? 0), signals: Math.max(0, p.acceptedSignalCount ?? 0), coverage: coverageLabel(p.observationCoverage) }));
  return <section className={c('ov-cell chart-col')} aria-label="评论观察趋势">
    <div className={c('chart-toolbar')}>
      <div className={c('legend')}><span><i />首次观察评论</span><span><i className={c('line-key')} />研究信号写入</span>
        <span className={c('coverage-legend')} title="黄色为部分观察。无记录不等于没有用户表达。"><i className={c('full')} />完整 <i className={c('part')} />部分 <i className={c('none')} />无记录</span></div>
      <div className={c('chart-controls')}>
        <div className={c('range-group')} role="group" aria-label="时间范围">{RANGES.map(([key, label]) =>
          <button type="button" key={key} className={c(range === key && 'active')} aria-pressed={range === key} onClick={() => onRange(key)}>{label}</button>)}</div>
        <div className={c('mode-group')} role="group" aria-label="趋势呈现">{([['chart', '图表'], ['table', '表格']] as const).map(([key, label]) =>
          <button type="button" key={key} className={c(view === key && 'active')} aria-pressed={view === key} onClick={() => onView(key)}>{label}</button>)}</div>
      </div>
    </div>
    {!rows.length ? <div className={c('chart-wrap')}><EmptyState kind="no_material" title="当前没有可读取的评论观察序列" /></div>
      : view === 'chart' ? <Chart rows={rows} range={range} />
      : <div className={c('chart-table')}><table><caption className="v7-sr-only">每日评论与研究信号</caption>
        <thead><tr><th scope="col">日期</th><th scope="col" className={c('num')}>首次观察评论</th><th scope="col" className={c('num')}>研究信号写入</th><th scope="col">完整性</th></tr></thead>
        <tbody>{[...rows].reverse().map(r => <tr key={r.date}><td>{r.date}</td><td className={c('num')}>{r.coverage === '无记录' && !r.comments ? '—' : r.comments}</td><td className={c('num')}>{r.signals}</td><td>{r.coverage}</td></tr>)}</tbody>
      </table></div>}
    <div className={c('chart-status')}>{rows.length ? `${slashDate(rows[0]!.date)} — ${slashDate(rows.at(-1)!.date)} · 按日汇总${range === 'fit' ? ' · 从最早一条评论前两天开始' : ''}` : ''}</div>
    <p className="v7-sr-only">每日观察是来源事件，研究信号按写入时间统计。两条序列具有独立纵轴，不代表当日转化率；范围按 Asia/Shanghai。</p>
  </section>;
}
