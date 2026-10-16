import { useEffect, useRef, useState, type ReactNode } from "react";
import type { Overview, TrendPoint } from "./api";
import * as f from "./format";
import { c } from "./styles";
function useWidth() {
  const ref = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(640);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const update = () =>
      setWidth(Math.max(Math.floor(el.clientWidth) || 640, 240));
    update();
    const observer = new ResizeObserver(update);
    observer.observe(el);
    return () => observer.disconnect();
  }, []);
  return { ref, width };
}
function Axis({
  center,
  y,
  value,
  width,
}: {
  center: number;
  y: number;
  value: string;
  width: number;
}) {
  const half = value.length * 3.5;
  const anchor =
    center - half < 0 ? "start" : center + half > width ? "end" : "middle";
  return (
    <text
      x={anchor === "start" ? 0 : anchor === "end" ? width : center}
      y={y}
      textAnchor={anchor}
      className={c("ops-axis")}
    >
      {value}
    </text>
  );
}
type CurvePoint = { x: number; y: number };

/** Smooth measured values without bridging missing data or overshooting adjacent values. */
export function smoothSeriesPath(points: Array<CurvePoint | null>): string {
  const segments: CurvePoint[][] = [];
  let segment: CurvePoint[] = [];
  const flush = () => { if (segment.length) segments.push(segment); segment = []; };
  for (const point of points) {
    if (point === null) flush();
    else segment.push(point);
  }
  flush();
  const fmt = (value: number) => value.toFixed(2);
  return segments.map((line) => {
    if (!line.length) return "";
    let d = "M" + fmt(line[0]!.x) + "," + fmt(line[0]!.y);
    for (let i = 0; i < line.length - 1; i++) {
      const a = line[i]!, b = line[i + 1]!;
      const before = line[Math.max(0, i - 1)]!;
      const after = line[Math.min(line.length - 1, i + 2)]!;
      const dx = (b.x - a.x) / 3;
      const min = Math.min(a.y, b.y), max = Math.max(a.y, b.y);
      const clamp = (value: number) => Math.max(min, Math.min(max, value));
      const firstY = clamp(a.y + (b.y - before.y) / 6);
      const secondY = clamp(b.y - (after.y - a.y) / 6);
      d += " C" + fmt(a.x + dx) + "," + fmt(firstY) + " " +
        fmt(b.x - dx) + "," + fmt(secondY) + " " + fmt(b.x) + "," + fmt(b.y);
    }
    return d;
  }).filter(Boolean).join(" ");
}

export function TrendChart({
  points,
  granularity,
}: {
  points: TrendPoint[];
  granularity: string;
}) {
  const { ref, width } = useWidth();
  const left = 42, right = 12;
  const plotWidth = Math.max(1, width - left - right);
  const top = 25, followerHeight = 142, followerBaseline = top + followerHeight;
  const hitsTop = 193, hitHeight = 62, hitBaseline = hitsTop + hitHeight;
  const coverageTop = hitBaseline + 9, coverageHeight = 6;
  const labelY = coverageTop + coverageHeight + 20, height = labelY + 12;
  const slot = plotWidth / Math.max(points.length, 1);
  const x = (i: number) => left + (i + 0.5) * slot;
  const knownMax = (selector: (p: TrendPoint) => number | null) =>
    Math.max(1, ...points.map(selector).filter((v): v is number => v !== null && Number.isFinite(v)));
  const followMax = knownMax((p) => p.followersGained);
  const hitMax = knownMax((p) => p.hits);
  const fy = (value: number) => followerBaseline - (Math.max(0, value) / followMax) * followerHeight;
  const hy = (value: number) => hitBaseline - (Math.max(0, value) / hitMax) * hitHeight;
  const followersPath = smoothSeriesPath(points.map((p, i) =>
    p.followersGained === null ? null : { x: x(i), y: fy(p.followersGained) },
  ));
  const hitsPath = smoothSeriesPath(points.map((p, i) =>
    p.hits === null ? null : { x: x(i), y: hy(p.hits) },
  ));
  const showEvery = Math.max(1, Math.ceil(points.length / 7));
  const partial = points.filter((p) => p.coverage === "partial");
  return (
    <div
      ref={ref}
      className={c("ops-chart", "ops-trend-chart-v2")}
      id="ops-trend"
      role="img"
      aria-label={f.trendAccessibleLabel(points, granularity)}
    >
      {points.length ? (
        <>
          <svg viewBox={[0, 0, width, height].join(" ")}>
            <text x={0} y={13} textAnchor="start" className={c("ops-axis")}>加微</text>
            <text x={0} y={hitsTop - 7} textAnchor="start" className={c("ops-axis")}>爆款</text>
            {[0.25, 0.5, 0.75, 1].map((step) => (
              <line
                key={step}
                x1={left} x2={width - right}
                y1={followerBaseline - step * followerHeight}
                y2={followerBaseline - step * followerHeight}
                className={c("ops-grid")}
              />
            ))}
            <line x1={left} x2={width - right} y1={followerBaseline} y2={followerBaseline}
                  className={c("ops-trend-baseline-v2")} />
            <line x1={left} x2={width - right} y1={hitBaseline} y2={hitBaseline}
                  className={c("ops-trend-baseline-v2")} />
            {followersPath ? <path d={followersPath} className={c("ops-trend-followers-v2")} /> : null}
            {hitsPath ? <path d={hitsPath} className={c("ops-trend-hits-v2")} /> : null}
            {points.map((p, i) => {
              const summary = p.label + " · 加微 " + f.number(p.followersGained) +
                " · 爆款 " + f.number(p.hits) + " · " + f.trendPointNote(p);
              const dayFocus = i === 0 || i === points.length - 1 || i % showEvery === 0;
              return (
                <g key={p.start}>
                  {p.followersGained !== null ? (
                    <circle cx={x(i)} cy={fy(p.followersGained)} r={points.length > 28 ? 1.5 : 2.5}
                            className={c("ops-trend-dot-v2")} />
                  ) : null}
                  {p.hits !== null && p.hits > 0 ? (
                    <circle cx={x(i)} cy={hy(p.hits)} r={points.length > 28 ? 1.5 : 2.5}
                            className={c("ops-trend-hit-dot-v2")} />
                  ) : null}
                  <rect
                    x={left + slot * i + 0.5}
                    y={coverageTop}
                    width={Math.max(1, slot - 1)}
                    height={coverageHeight}
                    className={c("ops-trend-coverage-v2")}
                    data-coverage={p.coverage}
                  />
                  {(i === 0 || i === points.length - 1 || i % showEvery === 0) ? (
                    <Axis center={x(i)} y={labelY} value={p.label} width={width} />
                  ) : null}
                  <rect
                    className={c("ops-trend-hit-area-v2")}
                    x={left + slot * i}
                    y={top}
                    width={slot}
                    height={coverageTop + coverageHeight - top}
                    tabIndex={dayFocus ? 0 : -1}
                    role="img"
                    aria-label={summary}
                  >
                    <title>{summary}</title>
                  </rect>
                </g>
              );
            })}
          </svg>
          {partial.length ? (
            <p className={c("ops-trend-footnote-v2")}>
              部分周期仅统计已迁入的完整日；悬停或聚焦日期查看实际截止时间。
            </p>
          ) : null}
        </>
      ) : (
        <p className={c("ops-chart-empty")}>暂无趋势数据</p>
      )}
    </div>
  );
}

interface Column {
  label: string;
  unknown: string;
  parts: { height: number; className: string; title: string }[];
}
function Bars({
  width,
  columns,
  height = 120,
}: {
  width: number;
  columns: Column[];
  height?: number;
}) {
  const top = 8,
    slot = width / Math.max(columns.length, 1);
  return (
    <svg viewBox={`0 0 ${width} ${height + 24}`}>
      <line
        x1={0}
        x2={width}
        y1={height + top}
        y2={height + top}
        className={c("ops-grid")}
      />
      {columns.map((column, i) => {
        const x = i * slot + slot * 0.15;
        let base = height + top;
        return (
          <g key={i}>
            {column.parts.map((part, j) => {
              if (part.height <= 0) return null;
              base -= part.height;
              return (
                <rect
                  key={j}
                  x={x}
                  y={base}
                  width={slot * 0.7}
                  height={part.height}
                  className={c(part.className)}
                >
                  <title>{part.title}</title>
                </rect>
              );
            })}
            {column.unknown ? (
              <rect
                x={x}
                y={height + top - 2}
                width={slot * 0.7}
                height={2}
                className={c("ops-gap")}
              >
                <title>{column.unknown}</title>
              </rect>
            ) : null}
            {column.label ? (
              <Axis
                center={x + slot * 0.35}
                y={height + top + 14}
                value={column.label}
                width={width}
              />
            ) : null}
          </g>
        );
      })}
    </svg>
  );
}
export function OutputChart({
  output,
  metric,
  person,
}: {
  output: Overview["dailyOutput"];
  metric: string;
  person: string;
}) {
  const { ref, width } = useWidth();
  const mine = (row: { operator: string }) =>
    !person || person === row.operator;
  const every = Math.max(1, Math.ceil(output.days.length / 8));
  const migrated = (day: string) =>
    output.coveredTo !== null && day <= output.coveredTo;
  let content: ReactNode;
  if (metric === "published") {
    const totals = output.days.map((day) =>
      output.published
        .filter((row) => row.date === day && mine(row))
        .reduce(
          (sum, row) => ({
            xhs: sum.xhs + row.xhs,
            douyin: sum.douyin + row.douyin,
          }),
          { xhs: 0, douyin: 0 },
        ),
    );
    const height = f.scale(
      totals.map((t) => t.xhs + t.douyin),
      120,
    );
    content =
      output.coveredTo === null ? (
        <p className={c("ops-chart-empty")}>这段时间还没有迁入的数据</p>
      ) : !totals.some((t) => t.xhs + t.douyin > 0) ? (
        <p className={c("ops-chart-empty")}>当期无发布</p>
      ) : (
        <Bars
          width={width}
          columns={output.days.map((day, i) => ({
            label: i % every === 0 ? day.slice(5) : "",
            unknown: migrated(day) ? "" : `${day} 还没有迁入`,
            parts: [
              {
                height: height(totals[i]?.xhs ?? null),
                className: "ops-xhs",
                title: `${day} 小红书 ${totals[i]?.xhs} 篇`,
              },
              {
                height: height(totals[i]?.douyin ?? null),
                className: "ops-douyin",
                title: `${day} 抖音 ${totals[i]?.douyin} 篇`,
              },
            ],
          }))}
        />
      );
  } else {
    const values = output.days.map((day) => {
      const rows = output.followers.filter(
        (row) => row.date === day && mine(row),
      );
      return rows.length ? rows.reduce((sum, row) => sum + row.value, 0) : null;
    });
    const height = f.scale(values, 120);
    content = (
      <Bars
        width={width}
        columns={output.days.map((day, i) => ({
          label: i % every === 0 ? day.slice(5) : "",
          unknown: !migrated(day)
            ? `${day} 还没有迁入`
            : values[i] == null
              ? `${day} 没有每日数据`
              : "",
          parts: [
            {
              height: height(values[i] ?? null),
              className: "ops-followers",
              title: `${day} 加微 ${f.number(values[i])}`,
            },
          ],
        }))}
      />
    );
  }
  return (
    <>
      <div
        ref={ref}
        className={c("ops-chart")}
        id="ops-daily-output"
        role="img"
        aria-label="每日产出"
      >
        {content}
      </div>
      <p className={c("ops-legend")} id="ops-output-legend">
        {metric === "published" ? (
          <>
            <span>
              <i className={c("ops-xhs")} />
              小红书
            </span>
            <span>
              <i className={c("ops-douyin")} />
              抖音
            </span>
          </>
        ) : (
          <span>
            <i className={c("ops-followers")} />
            加微（按人记，不分平台）
          </span>
        )}
      </p>
    </>
  );
}
export function Sparkline({ values }: { values: (number | null)[] }) {
  const width = 84, height = 20;
  const max = Math.max(1, ...values.filter((v): v is number => v !== null));
  const path = smoothSeriesPath(values.map((value, i) => value === null ? null : ({
    x: values.length > 1 ? 1 + (i / (values.length - 1)) * (width - 2) : width / 2,
    y: height - 1 - (value / max) * (height - 2),
  })));
  return (
    <svg
      viewBox={[0, 0, width, height].join(" ")}
      className={c("ops-spark")}
      role="img"
      aria-label={"近 7 天加微 " + values.map(f.number).join("、")}
    >
      {path ? <path d={path} /> : null}
    </svg>
  );
}
