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
export function TrendChart({
  points,
  granularity,
}: {
  points: TrendPoint[];
  granularity: string;
}) {
  const { ref, width } = useWidth();
  const followTop = 18,
    followHeight = 92,
    hitTop = 132,
    hitHeight = 22;
  const slot = width / Math.max(points.length, 1);
  const follow = f.scale(
      points.map((p) => p.followersGained),
      followHeight,
    ),
    hit = f.scale(
      points.map((p) => p.hits),
      hitHeight,
    );
  const every = Math.max(1, Math.ceil(points.length / 8));
  return (
    <div
      ref={ref}
      className={c("ops-chart")}
      id="ops-trend"
      role="img"
      aria-label={f.trendAccessibleLabel(points, granularity)}
    >
      {points.length ? (
        <>
          <svg viewBox={`0 0 ${width} ${hitTop + hitHeight + 18}`}>
            <text x={0} y={10} textAnchor="start" className={c("ops-axis")}>
              加微
            </text>
            <text
              x={0}
              y={hitTop - 6}
              textAnchor="start"
              className={c("ops-axis")}
            >
              爆款
            </text>
            <line
              x1={0}
              x2={width}
              y1={followTop + followHeight}
              y2={followTop + followHeight}
              className={c("ops-grid")}
            />
            <line
              x1={0}
              x2={width}
              y1={hitTop + hitHeight}
              y2={hitTop + hitHeight}
              className={c("ops-grid")}
            />
            {points.map((p, i) => {
              const x = i * slot + slot * 0.15,
                w = slot * 0.7,
                fh = follow(p.followersGained);
              const suffix =
                p.coverage === "partial" ? ` · ${f.trendPointNote(p)}` : "";
              return (
                <g key={p.start}>
                  {p.followersGained === null ? (
                    <rect
                      x={x}
                      y={followTop + followHeight - 2}
                      width={w}
                      height={2}
                      className={c("ops-gap")}
                    >
                      <title>
                        {p.label} 没有迁入的每日数据{suffix}
                      </title>
                    </rect>
                  ) : fh > 0 ? (
                    <rect
                      x={x}
                      y={followTop + followHeight - fh}
                      width={w}
                      height={fh}
                      className={c("ops-followers")}
                    >
                      <title>
                        {p.label} 加微 {f.number(p.followersGained)}
                        {suffix}
                      </title>
                    </rect>
                  ) : null}
                  {p.hits === null ? (
                    <rect
                      x={x}
                      y={hitTop + hitHeight - 2}
                      width={w}
                      height={2}
                      className={c("ops-gap")}
                    >
                      <title>{p.label} 爆款：还没迁入</title>
                    </rect>
                  ) : p.hits > 0 ? (
                    <rect
                      x={x}
                      y={hitTop + hitHeight - Math.max(hit(p.hits), 3)}
                      width={w}
                      height={Math.max(hit(p.hits), 3)}
                      className={c("ops-hits")}
                    >
                      <title>
                        {p.label} 爆款 {p.hits} 篇{suffix}
                      </title>
                    </rect>
                  ) : null}
                  {i % every === 0 ? (
                    <Axis
                      center={x + w / 2}
                      y={hitTop + hitHeight + 14}
                      value={p.label}
                      width={width}
                    />
                  ) : null}
                </g>
              );
            })}
          </svg>
          {points
            .filter((p) => p.coverage === "partial")
            .map((p) => (
              <p key={p.start} className={c("ops-note-line")}>
                {p.label} · {f.trendPointNote(p)}
              </p>
            ))}
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
  const width = 84,
    height = 20,
    max = Math.max(1, ...values.filter((v): v is number => v !== null));
  const segments: string[] = [];
  let segment: string[] = [];
  const flush = () => {
    if (segment.length > 1) segments.push(segment.join(" "));
    segment = [];
  };
  values.forEach((v, i) => {
    if (v === null) {
      flush();
      return;
    }
    segment.push(
      `${(i / 6) * (width - 2) + 1},${height - 1 - (v / max) * (height - 2)}`,
    );
  });
  flush();
  return (
    <svg
      viewBox={`0 0 ${width} ${height}`}
      className={c("ops-spark")}
      role="img"
      aria-label={`近 7 天加微 ${values.map(f.number).join("、")}`}
    >
      {segments.map((points, i) => (
        <polyline key={i} points={points} />
      ))}
    </svg>
  );
}
