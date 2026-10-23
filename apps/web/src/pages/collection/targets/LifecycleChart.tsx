import { Link } from 'react-router';
import type { Lifecycle } from './api';
import { listHref } from './useWorkspace';
import { compact, metricLabel, percentile, smoothPath, type Point, type Bucket } from './chartMath';
const coordinate = (value: number) => value.toFixed(1);
function ChartSwitch({
  params,
  distribution,
  week,
}: {
  params: URLSearchParams;
  distribution: boolean;
  week: boolean;
}) {
  return (
    <div className="life-chart-head-actions">
      <nav className="life-chart-switch" aria-label="图表视图">
        {[
          ['trend', '趋势'],
          ['distribution', '分布'],
        ].map(([key, label]) => (
          <Link
            key={key}
            className="life-chart-switch-tab"
            aria-current={(distribution ? 'distribution' : 'trend') === key ? 'page' : undefined}
            to={listHref(params, { life_chart: key ?? 'trend' }, 'creator-lifecycle')}
          >
            {label}
          </Link>
        ))}
      </nav>
      {distribution ? (
        <span className="life-trend-grain">复核信号 · 高讨论率 &gt; 20%</span>
      ) : (
        <nav className="life-grain-switch" aria-label="趋势粒度">
          <span>粒度</span>
          {[
            ['month', '按月'],
            ['week', '按周'],
          ].map(([key, label]) => (
            <Link
              key={key}
              aria-current={(week ? 'week' : 'month') === key ? 'page' : undefined}
              to={listHref(params, { life_grain: key ?? 'month' }, 'creator-lifecycle')}
            >
              {label}
            </Link>
          ))}
        </nav>
      )}
    </div>
  );
}
function Grid({ max }: { max: number }) {
  return (
    <>
      {Array.from({ length: 5 }, (_, i) => {
        const y = 28 + (388 / 4) * i,
          value = max * (1 - i / 4);
        return (
          <g key={i}>
            <line className="life-trend-grid" x1={62} y1={y} x2={952} y2={y} />
            <text
              className="life-trend-axis"
              x={50}
              y={y + 4}
              textAnchor="end"
              aria-label={`精确数值 ${value}`}
            >
              <title>精确数值 {value}</title>
              {compact(value)}
            </text>
          </g>
        );
      })}
    </>
  );
}
export function Chart({
  p,
  params,
  distribution,
  week,
  buckets,
}: {
  p: Lifecycle;
  params: URLSearchParams;
  distribution: boolean;
  week: boolean;
  buckets: Bucket[];
}) {
  const metric = metricLabel(p.metric),
    selected = params.get('life_work');
  if (distribution) {
    const start = p.points[0],
      end = p.points[p.points.length - 1],
      min = start?.epochMs ?? 0,
      max = end?.epochMs ?? min,
      maxY = Math.max(1, ...p.points.map((v) => v.value)),
      x = (v: Point) => (min === max ? 507 : 62 + ((v.epochMs - min) / Math.max(max - min, 1)) * 890),
      y = (v: Point) => 28 + (1 - v.value / maxY) * 388;
    return (
      <>
        <div className="life-trend-head">
          <div>
            <div className="life-trend-eyebrow">作品复核</div>
            <h2>作品表现分布</h2>
            <p>每个点是一篇当前可分析作品；横轴为发布时间，纵轴保持当前{metric}原始数值。</p>
          </div>
          <ChartSwitch params={params} distribution week={week} />
        </div>
        <div className="life-trend-legend" aria-label="分布图图例">
          <span>
            <i className="life-legend-dot" />
            每个点代表一篇作品
          </span>
          <span>
            <i className="life-legend-ring" />
            高讨论率 &gt; 20%
          </span>
          <span>
            <i className="life-legend-new" />
            最近巡查新增
          </span>
        </div>
        <figure className="life-trend-figure">
          <svg
            className="life-trend-chart life-distribution-chart"
            viewBox="0 0 980 470"
            role="group"
            aria-labelledby="life-chart-title life-chart-desc"
          >
            <title id="life-chart-title">创作者作品表现分布图</title>
            <desc id="life-chart-desc">
              横轴为当前时间窗内合格作品的发布时间，纵轴为当前指标{metric}
              的原始数值。每个可交互点代表一篇作品。橙红外圈表示评论／点赞超过 20%
              的复核信号；评论或点赞未知、或点赞为零的作品不参与该信号。
            </desc>
            <Grid max={maxY} />
            <line className="life-axis" x1={62} y1={416} x2={952} y2={416} />
            {p.points.map((v) => (
              <a
                key={v.publicRef}
                className={`life-point${v.new ? ' life-point-new' : ''}${v.discussionRate !== null && v.discussionRate > 0.2 ? ' life-point-high-discussion' : ''}${selected === v.publicRef ? ' life-point-selected' : ''}`}
                href={listHref(params, { life_work: v.publicRef }, 'creator-lifecycle')}
                aria-label={`${v.title ?? '标题未知'}，${v.date}，${metric} ${v.value}，${v.association === 'AUTHOR_CONFIRMED' ? '作者已确认' : '来自主页作品目录，详情作者待确认'}${v.new ? '，最近一次巡查新增' : ''}${v.discussionRate !== null && v.discussionRate > 0.2 ? `，评论／点赞 ${(v.discussionRate * 100).toFixed(0)}%（高讨论率复核信号）` : ''}`}
                aria-current={selected === v.publicRef ? 'true' : undefined}
              >
                {v.discussionRate !== null && v.discussionRate > 0.2 && (
                  <circle
                    className="life-point-high-discussion-ring"
                    cx={coordinate(x(v))}
                    cy={coordinate(y(v))}
                    r={10}
                    aria-hidden="true"
                  />
                )}
                {v.new && (
                  <rect
                    className="life-point-new-mark"
                    x={coordinate(x(v) + 6)}
                    y={coordinate(y(v) - 11)}
                    width={6}
                    height={6}
                    aria-hidden="true"
                  />
                )}
                <circle className="life-point-hit" cx={coordinate(x(v))} cy={coordinate(y(v))} r={7} aria-hidden="true" />
                <circle className="life-point-visible" cx={coordinate(x(v))} cy={coordinate(y(v))} r={4.6} aria-hidden="true" />
              </a>
            ))}
            <text className="life-axis-copy" x={62} y={446}>
              {start?.date ?? '—'}
            </text>
            <text className="life-axis-copy" x={952} y={446} textAnchor="end">
              {end?.date ?? '—'}
            </text>
            <text
              className="life-axis-copy"
              x={17}
              y={222}
              transform="rotate(-90 17 222)"
              textAnchor="middle"
            >
              {metric}
            </text>
          </svg>
        </figure>
      </>
    );
  }
  const medians = buckets.map(
      (b) =>
        percentile(
          b.points.map((v) => v.value),
          0.5,
        ) ?? 0,
    ),
    max = Math.max(1, ...medians) * 1.18,
    maxPosts = Math.max(1, ...buckets.map((b) => b.points.length)),
    step = buckets.length > 1 ? 890 / (buckets.length - 1) : 445,
    x = (i: number) => (buckets.length === 1 ? 507 : 62 + step * i),
    y = (v: number) => 416 - (v / max) * 388,
    points: [number, number][] = medians.map((v, i) => [x(i), y(v)]),
    line = smoothPath(points),
    area = `${line} L ${points[points.length - 1]?.[0].toFixed(1)},416.0 L ${points[0]?.[0].toFixed(1)},416.0 Z`,
    median =
      percentile(
        p.points.map((v) => v.value),
        0.5,
      ) ?? 0;
  return (
    <>
      <div className="life-trend-head">
        <div>
          <div className="life-trend-eyebrow">作品复核</div>
          <h2>作品表现趋势</h2>
          <p>柱形表示发布密度，主线使用同一时间桶的{metric}中位数；橙红只标记最近巡查新增。</p>
        </div>
        <ChartSwitch params={params} distribution={false} week={week} />
      </div>
      <div className="life-trend-legend" aria-label="趋势图图例">
        <span>
          <i className="life-legend-bar" />
          发布作品数
        </span>
        <span>
          <i className="life-legend-line" />
          {metric}中位数
        </span>
        <span>
          <i className="life-legend-dash" />
          当前窗口中位基准
        </span>
        <span>
          <i className="life-legend-ring" />
          巡查新增
        </span>
      </div>
      <figure className="life-trend-figure">
        <svg
          className="life-trend-chart"
          viewBox="0 0 980 470"
          role="group"
          aria-labelledby="life-trend-chart-title life-trend-chart-desc"
        >
          <title id="life-trend-chart-title">创作者作品表现趋势</title>
          <desc id="life-trend-chart-desc">
            横轴为当前时间窗口的{week ? '按周' : '按月'}，柱形表示可分析作品数，墨线表示同一时间桶的{metric}
            中位数，虚线表示当前窗口所有已纳入作品的{metric}中位数。橙红圆环仅表示该桶包含最近巡查新增作品。
          </desc>
          <defs>
            <linearGradient id="life-trend-area-gradient" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor="var(--lgi-signal-soft)" stopOpacity={0.92} />
              <stop offset="100%" stopColor="var(--lgi-canvas)" stopOpacity={0} />
            </linearGradient>
          </defs>
          <Grid max={max} />
          {buckets.map((b, i) => {
            const h = (b.points.length / maxPosts) * 72,
              w = Math.max(12, Math.min(30, step * 0.42));
            return (
              <rect
                key={b.label}
                className="life-trend-bar"
                x={coordinate(x(i) - w / 2)}
                y={coordinate(416 - h)}
                width={coordinate(w)}
                height={coordinate(h)}
                rx={1}
              />
            );
          })}
          <path className="life-trend-area" d={area} />
          <line className="life-trend-benchmark" x1={62} y1={coordinate(y(median))} x2={952} y2={coordinate(y(median))} />
          <text
            className="life-trend-benchmark-copy"
            x={952}
            y={coordinate(y(median) - 7)}
            textAnchor="end"
            aria-label={`当前窗口精确中位数 ${median}`}
          >
            <title>当前窗口精确中位数 {median}</title>当前窗口中位 · {compact(median)}
          </text>
          <path className="life-trend-line" d={line} />
          {buckets.map((b, i) => (
            <a key={b.label} href={listHref(params,{life_bucket:b.label,life_work:null},'creator-lifecycle')}
              aria-label={`${b.label}，${b.points.length} 篇当前可分析作品，${metric}中位数 ${medians[i] ?? 0}。选择后查看本桶作品并加入对照。`}
              aria-current={params.get('life_bucket')===b.label?'true':undefined}
              className={params.get('life_bucket')===b.label?'life-trend-bucket-selected':undefined}>
              {b.points.some((v) => v.new) && (
                <circle className="life-trend-new-ring" cx={coordinate(x(i))} cy={coordinate(y(medians[i] ?? 0))} r={9.5} />
              )}
              <circle
                className={`life-trend-dot${b.points.some((v) => v.new) ? ' life-trend-dot-new' : ''}`}
                cx={coordinate(x(i))}
                cy={coordinate(y(medians[i] ?? 0))}
                r={4.4}
              />
              <circle className="life-trend-bucket-hit" cx={coordinate(x(i))} cy={coordinate(y(medians[i] ?? 0))} r={14} fill="transparent" />
            </a>
          ))}
          {buckets.map((b, i) =>
            buckets.length <= 7 || i === 0 || i + 1 === buckets.length || i % 2 === 0 ? (
              <text
                key={b.label}
                className="life-trend-axis"
                x={coordinate(x(i))}
                y={446}
                textAnchor={i === 0 ? 'start' : i + 1 === buckets.length ? 'end' : 'middle'}
              >
                {b.label}
              </text>
            ) : null,
          )}
          <text className="life-trend-axis" x={17} y={222} transform="rotate(-90 17 222)" textAnchor="middle">
            {metric}中位数
          </text>
        </svg>
      </figure>
    </>
  );
}
