import { Fragment, useEffect, useId, useRef, useState } from 'react';
import type { ObservationPoint, ObservationSeries as Series } from './overviewDtos';
import { num } from './format';
import { c } from './styles';

const coverageLabel = (value: string | null) =>
  ({ recorded: '完整观察', partial: '部分观察', none: '无观察记录' }[value ?? ''] ?? '观察状态未知');

function ceiling(max: number) {
  if (max <= 1) return 1;
  const step = 10 ** Math.floor(Math.log10(max / 3));
  return Math.ceil(max / step) * step;
}
function smooth(points: { x: number; y: number }[]) {
  if (!points.length) return '';
  let result = `M ${points[0]!.x.toFixed(2)} ${points[0]!.y.toFixed(2)}`;
  for (let i = 1; i < points.length; i++) {
    const before = points[i - 1]!, current = points[i]!;
    const mid = (before.x + current.x) / 2;
    result += ` C ${mid.toFixed(2)} ${before.y.toFixed(2)} ${mid.toFixed(2)} ${current.y.toFixed(2)} ${current.x.toFixed(2)} ${current.y.toFixed(2)}`;
  }
  return result;
}

/** Black observations + red smooth signal curve with a downward-to-baseline gradient.
 * Both series stay above the common x axis and retain independent y scales. */
export function ObservationChart({ points }: { points: ObservationPoint[] }) {
  const container = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(960);
  const gradientId = useId().replace(/:/g, '');
  useEffect(() => {
    const node = container.current;
    if (!node) return;
    const update = () => { const measured = Math.floor(node.getBoundingClientRect().width); if (measured > 0) setWidth(measured); };
    update();
    if (typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(update);
    observer.observe(node);
    return () => observer.disconnect();
  }, []);
  if (!points.length) return <p className={c('study-observation-empty')}>当前没有可读取的评论观察序列。</p>;

  const h = 196, left = 48, right = 46, top = 22, base = 148, coverageY = 157, coverageH = 6, dateY = 190;
  const plotWidth = Math.max(1, width - left - right), band = plotWidth / points.length;
  const bw = Math.min(10, Math.max(2.4, band * .34));
  const x = (i: number) => left + band * (i + .5);
  const comments = points.map(p => Math.max(0, Number(p.newObservedCommentCount) || 0));
  const signals = points.map(p => Math.max(0, Number(p.acceptedSignalCount) || 0));
  const maxComments = ceiling(Math.max(...comments)), maxSignals = ceiling(Math.max(...signals));
  const yComment = (v: number) => base - (v / maxComments) * (base - top);
  const ySignal = (v: number) => base - (v / maxSignals) * (base - top);
  const steps = new Set([0, Math.round(maxComments / 2), maxComments]);
  const dates = new Set<number>();
  const shown = points.length <= 7 ? points.length : points.length <= 28 ? 5 : 7;
  for (let i = 0; i < shown; i++) dates.add(Math.round(i * (points.length - 1) / Math.max(1, shown - 1)));
  const peakComment = comments.indexOf(Math.max(...comments));
  const peakSignal = signals.indexOf(Math.max(...signals));
  const signalGeometry = points.map((_, i) => ({ x: x(i), y: ySignal(signals[i]!) }));
  const line = smooth(signalGeometry);
  const fill = line + ` L ${signalGeometry.at(-1)!.x.toFixed(2)} ${base} L ${signalGeometry[0]!.x.toFixed(2)} ${base} Z`;

  return <div className={c('study-series-chart')}><div ref={container} className={c('study-series-svg-wrap')}>
    <svg role="img" aria-label="评论柱状与研究信号渐变曲线，共同横轴及采集覆盖" viewBox={`0 0 ${width} ${h}`}>
      <title>评论黑色柱状、研究信号橙红曲线与向下渐变，均位于日期横轴上方；左右独立刻度</title>
      <defs><linearGradient id={gradientId} x1="0" y1={top} x2="0" y2={base} gradientUnits="userSpaceOnUse">
        <stop offset="0%" stopColor="var(--lgi-signal)" stopOpacity=".27"/>
        <stop offset="100%" stopColor="var(--lgi-signal)" stopOpacity=".01"/>
      </linearGradient></defs>
      {[...steps].sort((a,b) => b - a).map(v => <Fragment key={v}>
        <line className={c('study-series-grid-line')} x1={left} x2={width - right} y1={yComment(v)} y2={yComment(v)}/>
        <text className={c('study-series-tick')} x={left - 8} y={yComment(v) + 4} textAnchor="end">{num(v)}</text>
      </Fragment>)}
      {[0, Math.round(maxSignals/2), maxSignals].filter((value, i, values) => values.indexOf(value) === i).map(value =>
        <text key={value} className={c('study-series-tick')} x={width - right + 8} y={ySignal(value) + 4}>{num(value)}</text>)}
      <text className={c('study-series-axis-caption')} x="0" y="14">评论</text>
      <text className={c('study-series-axis-caption')} x={width} y="14" textAnchor="end">信号</text>
      <line className={c('study-series-zero-line')} x1={left} x2={width - right} y1={base} y2={base}/>
      <path className={c('study-signal-area')} d={fill} fill={`url(#${gradientId})`}/>
      <path className={c('study-signal-curve')} d={line}/>
      {points.map((point, i) => {
        const value = comments[i]!, signal = signals[i]!;
        const state = point.observationCoverage ?? 'none';
        const fullLabel = `${point.date}：首次观察 ${state === 'none' && value === 0 ? '未记录' : num(value) + ' 条'}，写入信号 ${num(signal)} 个，${coverageLabel(state)}`;
        return <Fragment key={point.date}>
          {value > 0 ? <rect className={c('study-series-comment-bar')} x={x(i) - bw / 2} y={yComment(value)} width={bw} height={base - yComment(value)}/> : null}
          <rect className={c('study-series-coverage-tick')} data-state={state} x={left + i * band + .5} y={coverageY} width={Math.max(1, band - 1)} height={coverageH}/>
          {dates.has(i) ? <text className={c('study-series-tick')} x={i === 0 ? left : i === points.length - 1 ? width - right : x(i)}
            y={dateY} textAnchor={i === 0 ? 'start' : i === points.length - 1 ? 'end' : 'middle'}>{point.date.slice(5)}</text> : null}
          <rect className={c('study-series-day-hit')} role="img" tabIndex={0} aria-label={fullLabel}
            x={left + i * band} y={top} width={band} height={coverageY + coverageH - top}><title>{fullLabel}</title></rect>
        </Fragment>;
      })}
      {signals[peakSignal]! > 0 ? <circle className={c('study-signal-point')} cx={x(peakSignal)} cy={ySignal(signals[peakSignal]!)} r="2.5"/> : null}
      {comments[peakComment]! > 0 ? <text className={c('study-series-peak')} x={x(peakComment)} y={Math.max(top - 5, yComment(comments[peakComment]!) - 7)}
        textAnchor="middle">{num(comments[peakComment])}</text> : null}
      {signals[peakSignal]! > 0 ? <text className={c('study-series-peak-signal')} x={x(peakSignal)} y={Math.max(top - 5, ySignal(signals[peakSignal]!) - 9)}
        textAnchor="middle">{num(signals[peakSignal])}</text> : null}
    </svg>
  </div></div>;
}

export function ObservationSeries({ series, days, view, onDays, onView }: {
  series: Series | null; days: number; view: 'chart' | 'table';
  onDays: (days: number) => void; onView: (view: 'chart' | 'table') => void;
}) {
  const points = series?.points.slice(-days) ?? [];
  return <section className={c('study-observation')} aria-label="评论观察趋势">
    <div className={c('study-observation-toolbar')}>
      <div className={c('study-series-legend')} aria-label="图例">
        <span><i data-kind="comments"/>首次观察评论</span>
        <span><i data-kind="signals"/>研究信号写入</span>
        <span className={c('study-series-legend-separator')} aria-hidden="true"/>
        <span className={c('study-legend-coverage')} title="黄色为部分观察。无观察记录不等于没有用户表达。">
          <i data-state="recorded"/>完整 <i data-state="partial"/>部分 <i data-state="none"/>无记录
        </span>
      </div>
      <div className={c('study-observation-controls')}>
        <div className={c('study-text-switcher')} role="group" aria-label="观察时间范围">
          {[7,28,56].map(n => <button type="button" key={n} aria-pressed={days===n} onClick={() => onDays(n)}>近 {n} 天</button>)}
        </div>
        <div className={c('study-text-switcher')} role="group" aria-label="观察数据视图">
          <button type="button" aria-pressed={view==='chart'} onClick={() => onView('chart')}>图表</button>
          <button type="button" aria-pressed={view==='table'} onClick={() => onView('table')}>表格</button>
        </div>
      </div>
    </div>
    {!series || !points.length ? <p className={c('study-observation-empty')}>当前没有可读取的评论观察序列。</p>
      : view === 'chart' ? <ObservationChart points={points}/> : <div className={c('study-series-table-wrap')}>
        <table className={c('study-series-table')}><thead><tr>
          {['日期','首次观察评论','进入研究评论','接纳信号','覆盖作品','观察覆盖'].map(name=><th scope="col" key={name}>{name}</th>)}
        </tr></thead><tbody>
          {[...points].reverse().map(p=><tr key={p.date}>
            <td><time dateTime={p.date}>{p.date}</time></td>
            <td>{p.observationCoverage === 'none' && !p.newObservedCommentCount ? '—' : num(p.newObservedCommentCount)}</td>
            <td>{num(p.studiedCommentCount)}</td><td>{num(p.acceptedSignalCount)}</td>
            <td>{p.observationCoverage === 'none' && !p.coveredWorkCount ? '—' : num(p.coveredWorkCount)}</td>
            <td>{coverageLabel(p.observationCoverage)}</td>
          </tr>)}
        </tbody></table>
      </div>}
    <p className="v7-sr-only">每日观察是来源事件，研究信号按写入时间统计。两条序列具有独立纵轴，不代表当日转化率；范围按 Asia/Shanghai。</p>
  </section>;
}
