import { Fragment } from 'react';
import type { ObservationPoint, ObservationSeries as Series } from './overviewDtos';
import { num } from './format';
import { c } from './styles';

const coverageLabel = (state: string | null) => ({ recorded: '有观察记录', partial: '部分观察', none: '无观察记录' }[state ?? ''] ?? '观察状态未知');
const scale = (max: number, n: number) => {
  const raw = Math.max(1, max) / n, pow = 10 ** Math.floor(Math.log10(raw));
  const step = [1, 2, 2.5, 5, 10].map(k => k * pow).filter(Number.isInteger).find(k => k >= raw) || Math.max(1, 10 * pow);
  return { max: Math.ceil(Math.max(1, max) / step) * step, step };
};

/** Original dual-track geometry, expressed as SVG nodes owned by React. */
export function ObservationChart({ points }: { points: ObservationPoint[] }) {
  if (!points.length) return <p className={c('study-observation-empty')}>当前没有可读取的评论观察序列。</p>;
  const w = 1000, l = 51, rr = 10, top = 24, topH = 190, signalBase = 329, signalH = 70, covY = signalBase + 8, covH = 6, dateY = covY + covH + 20, h = dateY + 16;
  const band = (w - l - rr) / points.length, bw = points.length <= 7 ? Math.min(42, band * .34) : Math.max(4, Math.min(19, band * .54)), cx = (i: number) => l + band * (i + .5);
  const obs = points.map(p => Math.max(0, Number(p.newObservedCommentCount) || 0)), sig = points.map(p => Math.max(0, Number(p.acceptedSignalCount) || 0));
  const oc = scale(Math.max(...obs), 3), sc = scale(Math.max(...sig), 2), oy = (v: number) => top + topH - v / oc.max * topH, sy = (v: number) => signalBase - v / sc.max * signalH;
  const ticks = new Set<number>(), n = points.length <= 7 ? points.length : points.length <= 28 ? 7 : 8;
  for (let i = 0; i < n; i++) ticks.add(Math.round(i * (points.length - 1) / (n - 1)));
  const oi = obs.indexOf(Math.max(...obs)), si = sig.indexOf(Math.max(...sig));
  const grid = [];
  for (let value = 0; value <= oc.max + oc.step / 2; value += oc.step) grid.push(value);
  return <div className={c('study-series-chart')}><div className={c('study-series-svg-wrap')}><svg viewBox={`0 0 ${w} ${h}`} role="img" aria-label="上下双轨每日柱状图及紧贴信号轴的采集覆盖">
    <title>首次观察评论与研究信号写入，底部短刻度表示采集覆盖</title>
    {grid.map(value => <Fragment key={value}><line className={c('study-series-grid-line')} x1={l} x2={w - rr} y1={oy(value)} y2={oy(value)} /><text x={l - 8} y={oy(value) + 4} textAnchor="end">{num(Math.round(value))}</text></Fragment>)}
    <text className={c('study-series-axis-caption')} x="0" y="14">评论</text><text className={c('study-series-axis-caption')} x="0" y={signalBase - signalH - 10}>信号</text><line className={c('study-series-zero-line')} x1={l} x2={w - rr} y1={signalBase} y2={signalBase} /><text x={l - 8} y={signalBase + 4} textAnchor="end">0</text>
    {points.map((point, i) => {
      const x = cx(i), left = x - bw / 2, a = obs[i]!, b = sig[i]!;
      const state = ['recorded', 'partial', 'none'].includes(point.observationCoverage ?? '') ? point.observationCoverage : 'none';
      const alt = `${point.date}：首次观察 ${num(a)} 条、进入研究事件 ${num(point.studiedCommentCount)} 条、写入信号 ${num(b)} 个、${coverageLabel(state)}`;
      return <Fragment key={point.date}>
        {a > 0 ? <rect className={c('study-series-comment-bar')} x={left.toFixed(2)} y={oy(a).toFixed(2)} width={bw.toFixed(2)} height={(top + topH - oy(a)).toFixed(2)} /> : null}
        {b > 0 ? <rect className={c('study-series-signal-bar')} x={left.toFixed(2)} y={sy(b).toFixed(2)} width={bw.toFixed(2)} height={(signalBase - sy(b)).toFixed(2)} /> : null}
        <rect className={c('study-series-coverage-tick')} data-state={state} x={(l + i * band + .8).toFixed(2)} y={covY} width={Math.max(1.5, band - 1.6).toFixed(2)} height={covH} />
        {ticks.has(i) ? <text x={(i === 0 ? l : i === points.length - 1 ? w - rr : x).toFixed(2)} y={dateY} textAnchor={i === 0 ? 'start' : i === points.length - 1 ? 'end' : 'middle'}>{point.date.slice(5)}</text> : null}
        <rect className={c('study-series-day-hit')} role="img" tabIndex={0} aria-label={alt} x={(l + i * band).toFixed(2)} y={top} width={band.toFixed(2)} height={covY + covH - top}><title>{alt}</title></rect>
      </Fragment>;
    })}
    {obs[oi]! > 0 ? <text className={c('study-series-peak')} x={Math.max(l + 23, Math.min(w - rr - 25, cx(oi))).toFixed(2)} y={Math.max(18, oy(obs[oi]!) - 9).toFixed(2)} textAnchor="middle">{num(obs[oi])}</text> : null}
    {sig[si]! > 0 ? <text className={c('study-series-peak-signal')} x={Math.max(l + 23, Math.min(w - rr - 25, cx(si))).toFixed(2)} y={Math.max(signalBase - signalH - 5, sy(sig[si]!) - 8).toFixed(2)} textAnchor="middle">{num(sig[si])}</text> : null}
  </svg></div></div>;
}

export function ObservationSeries({ series, days, view, onDays, onView }: { series: Series | null; days: number; view: 'chart' | 'table'; onDays: (days: number) => void; onView: (view: 'chart' | 'table') => void }) {
  const points = series?.points.slice(-days) ?? [];
  return <section className={c('study-observation')} aria-labelledby="study-observation-title">
    <h2 id="study-observation-title" className="v7-sr-only">评论观察与研究写入</h2>
    <div className={c('study-observation-toolbar')}>
      <div className={c('study-series-legend')} aria-label="图例"><span><i data-kind="comments" />首次观察评论</span><span><i data-kind="signals" />研究信号写入</span><span className={c('study-series-legend-separator')} aria-hidden="true" /><span className={c('study-legend-coverage')} title="采集覆盖紧贴信号轴。无记录不等于没有表达。"><i data-state="recorded" />完整 <i data-state="partial" />部分 <i data-state="none" />无记录</span></div>
      <div className={c('study-observation-controls')}><div className={c('study-text-switcher')} role="group" aria-label="观察时间范围">{[7, 28, 56].map(n => <button key={n} type="button" data-series-days={n} aria-pressed={days === n} onClick={() => onDays(n)}>近 {n} 天</button>)}</div><div className={c('study-text-switcher')} role="group" aria-label="每日数据视图"><button type="button" data-series-view="chart" aria-pressed={view === 'chart'} onClick={() => onView('chart')}>图表</button><button type="button" data-series-view="table" aria-pressed={view === 'table'} onClick={() => onView('table')}>表格</button></div></div>
    </div>
    {!series ? <p className={c('study-observation-empty')}>当前领域没有可读取的评论观察序列。</p> : !points.length ? <p className={c('study-observation-empty')}>当前没有可读取的评论观察序列。</p> : view === 'chart' ? <ObservationChart points={points} /> : <div className={c('study-series-table-wrap')}><table className={c('study-series-table')}><thead><tr>{['日期', '首次观察评论', '进入研究评论', '接纳信号', '覆盖作品', '观察覆盖'].map(title => <th scope="col" key={title}>{title}</th>)}</tr></thead><tbody>{[...points].reverse().map(point => <tr key={point.date}><td><time dateTime={point.date}>{point.date}</time></td><td>{num(point.newObservedCommentCount)}</td><td>{num(point.studiedCommentCount)}</td><td>{num(point.acceptedSignalCount)}</td><td>{num(point.coveredWorkCount)}</td><td><span className={c('study-coverage-state')} data-state={point.observationCoverage}>{coverageLabel(point.observationCoverage)}</span></td></tr>)}</tbody></table></div>}
    <p className="v7-sr-only">没有观察记录不等于用户没有表达。首次观察按评论身份去重，进入研究按目标来源去重，研究信号按写入日期统计；时间按 {series?.timezone || 'Asia/Shanghai'}。</p>
  </section>;
}
