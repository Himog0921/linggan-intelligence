import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useSearchParams } from "react-router";
import * as api from "./api";
import * as f from "./format";
import { Dialog, Status, Tabs } from "./ui";
import { c, peopleTabs } from "./styles";
import { TargetDialog } from "./TargetDialog";
import {
  Attention,
  Delta,
  Funnel,
  Matrix,
  Ranking,
  Share,
  Target,
} from "./OverviewBlocks";
import { OutputChart, TrendChart } from "./Charts";
export function OverviewPage({ business }: { business: string }) {
  const [params, setParams] = useSearchParams();
  const slice = params.get("slice") || "week";
  const client = useQueryClient();
  const [granularity, setGranularity] = useState("day");
  const [metric, setMetric] = useState("published");
  const [person, setPerson] = useState("");
  const [custom, setCustom] = useState(false);
  const [target, setTarget] = useState<api.PeriodType | null>(null);
  const [note, setNote] = useState("");
  const result = useQuery({
    queryKey: [
      "ops",
      "overview",
      business,
      slice,
      params.get("from"),
      params.get("to"),
    ],
    queryFn: ({ signal }) =>
      api.get(
        api.url("overview", {
          domain: business,
          slice,
          ...(slice === "custom"
            ? { from: params.get("from") || "", to: params.get("to") || "" }
            : {}),
        }),
        api.overview,
        signal,
      ),
  });
  const trend = useQuery({
    queryKey: ["ops", "trend", business, granularity],
    queryFn: ({ signal }) =>
      api.get(
        api.url("overview/trend", { domain: business, granularity }),
        api.trend,
        signal,
      ),
  });
  const data = result.isError ? undefined : result.data;
  function select(nextSlice: string, from = "", to = "") {
    const next = new URLSearchParams(params);
    next.set("slice", nextSlice);
    for (const [key, value] of [
      ["from", from],
      ["to", to],
    ]) {
      if (value) next.set(key || "", value);
      else next.delete(key || "");
    }
    setNote("");
    setParams(next, { replace: true });
  }
  return (
    <>
      <div className={c("ops-toolbar")}>
        <Tabs
          label="时间段"
          id="ops-slices"
          values={[
            ["yesterday", "昨日"],
            ["week", "本周"],
            ["month", "本月"],
            ["custom", "自定义"],
          ]}
          current={slice}
          onSelect={(value) =>
            value === "custom" ? setCustom(true) : select(value)
          }
        />
        <div className={c("ops-tools", "ops-window-tools")}>
          <span id="ops-window" className={c("ops-counts")} aria-live="polite">
            {data ? f.windowLine(data.window, data.asOf) : ""}
          </span>
        </div>
      </div>
      <p
        id="ops-scope-note"
        className={c("ops-scope-note")}
        hidden={!data?.businessSelected}
      >
        链接打开、加微、转化率和目标来自每日数据，不分业务。
      </p>
      <Status
        error={result.isError || Boolean(data && !data.asOf)}
        retry={result.isError ? () => void result.refetch() : undefined}
      >
        {result.isError
          ? `总览读取失败（${api.reason(result.error)}），可以重试。`
          : data && !data.asOf
            ? "还没有迁入数据。"
            : note}
      </Status>
      <div
        className={c("ops-overview")}
        data-ops-overview
        aria-busy={result.isFetching}
        hidden={result.isError}
      >
        {data ? (
          <>
            <section
              className={c("ops-block", "ops-hero")}
              aria-labelledby="ops-hero-title"
            >
              <div className={c("ops-hero-main")}>
                <h2 id="ops-hero-title" className={c("ops-block-title")}>
                  {f.sliceTitle[data.window.slice]}
                </h2>
                <p className={c("ops-hero-figure")}>
                  <b
                    data-state={
                      data.kpi.current.followersGained === null
                        ? "unknown"
                        : undefined
                    }
                    title={
                      data.kpi.current.followersGained === null
                        ? "这段时间没有迁入的每日数据：未知，不是 0"
                        : ""
                    }
                  >
                    {f.number(data.kpi.current.followersGained)}
                  </b>
                  <Delta value={data.kpi.delta.followersGainedPct} />
                </p>
                <p className={c("ops-hero-sub")}>
                  {data.window.previous
                    ? `上期（${f.span(data.window.previous)}）实际 ${f.number(data.kpi.previous.followersGained)}`
                    : "这段时间还没有迁入的数据，不比上期"}
                </p>
                <ul
                  className={c("ops-insights")}
                  id="ops-insights"
                  aria-label="自动结论"
                >
                  {data.insights.map((item, i) => {
                    const t = f.insight(item);
                    return (
                      <li key={i} data-tone={t.tone}>
                        {t.text}
                      </li>
                    );
                  })}
                </ul>
              </div>
              <Target data={data} open={setTarget} />
            </section>
            <Funnel data={data} />
            <div className={c("ops-split")}>
              <section
                className={c("ops-block")}
                aria-labelledby="ops-trend-title"
              >
                <div className={c("ops-block-head")}>
                  <h2 id="ops-trend-title" className={c("ops-block-title")}>
                    加微 · 爆款长趋势
                  </h2>
                  <Tabs
                    label="趋势粒度"
                    id="ops-granularity"
                    compact
                    values={[
                      ["day", "按日"],
                      ["week", "按周"],
                      ["month", "按月"],
                    ]}
                    current={granularity}
                    onSelect={(value) => {
                      if (value === granularity) void trend.refetch();
                      else setGranularity(value);
                    }}
                  />
                </div>
                <p className={c("ops-note-line")}>
                  按日近 90 天 · 按周近 12 周 · 按月近 6
                  月；不随上面的时间段变化。加微不分业务。
                </p>
                {trend.isError ? (
                  <div className={c("ops-chart")}>
                    <p className={c("ops-chart-empty")}>
                      趋势读取失败（{api.reason(trend.error)}
                      ），可以重新点粒度重试
                    </p>
                  </div>
                ) : trend.data ? (
                  <TrendChart
                    points={trend.data.points}
                    granularity={granularity}
                  />
                ) : (
                  <div className={c("ops-chart")} />
                )}
                <div className={c("ops-block-head", "ops-subhead")}>
                  <h2 className={c("ops-block-title")}>
                    每日产出 · 按人按平台
                  </h2>
                  <Tabs
                    label="产出指标"
                    id="ops-output-metric"
                    compact
                    values={[
                      ["published", "发布量"],
                      ["followers", "加微"],
                    ]}
                    current={metric}
                    onSelect={setMetric}
                  />
                </div>
                <Tabs
                  label="运营人"
                  id="ops-output-people"
                  compact
                  values={peopleTabs(data.dailyOutput.operators)}
                  current={
                    data.dailyOutput.operators.includes(person) ? person : ""
                  }
                  onSelect={setPerson}
                />
                <OutputChart
                  output={data.dailyOutput}
                  metric={metric}
                  person={
                    data.dailyOutput.operators.includes(person) ? person : ""
                  }
                />
              </section>
              <Attention
                key={`${business}:${slice}:${params.get("from")}:${params.get("to")}`}
                data={data}
              />
            </div>
            <Ranking data={data} />
            <div className={c("ops-split", "ops-split-even")}>
              <section
                className={c("ops-block")}
                aria-labelledby="ops-platform-title"
              >
                <h2 id="ops-platform-title" className={c("ops-block-title")}>
                  平台占比
                </h2>
                <Share data={data} />
                <p className={c("ops-note-line")}>
                  按每条发布登记的平台统计。加微按人记，分不到平台，所以不显示加微占比。
                </p>
              </section>
              <section
                className={c("ops-block")}
                aria-labelledby="ops-matrix-title"
              >
                <h2 id="ops-matrix-title" className={c("ops-block-title")}>
                  运营人 × 账号矩阵
                </h2>
                <p className={c("ops-note-line")}>
                  格子是所选时段的发布篇数，括号里是爆款数；颜色深浅只表示篇数多少。加微分不到账号，不显示转化率。
                </p>
                <Matrix data={data} />
              </section>
            </div>
          </>
        ) : null}
      </div>
      {custom ? (
        <CustomDialog
          from={params.get("from") || ""}
          to={params.get("to") || ""}
          close={() => setCustom(false)}
          save={(from, to) => {
            setCustom(false);
            select("custom", from, to);
          }}
        />
      ) : null}
      {target && data ? (
        <TargetDialog
          data={data}
          initial={target}
          close={() => setTarget(null)}
          saved={(message) => {
            setTarget(null);
            setNote(message);
            void client.invalidateQueries({ queryKey: ["ops", "overview"] });
          }}
        />
      ) : null}
    </>
  );
}
function CustomDialog({
  from,
  to,
  close,
  save,
}: {
  from: string;
  to: string;
  close: () => void;
  save: (from: string, to: string) => void;
}) {
  const [error, setError] = useState("");
  return (
    <Dialog title="ops-custom-title" onClose={close}>
      <form
        id="ops-custom-form"
        onSubmit={(event) => {
          event.preventDefault();
          const form = new FormData(event.currentTarget);
          const a = String(form.get("from") || ""),
            b = String(form.get("to") || "");
          const days =
            (Date.parse(`${b}T00:00:00Z`) - Date.parse(`${a}T00:00:00Z`)) /
              86400000 +
            1;
          if (!(days >= 1 && days <= 366)) {
            setError("开始日期不能晚于结束日期，最长 366 天");
            return;
          }
          save(a, b);
        }}
      >
        <header>
          <h2 id="ops-custom-title">自定义时段</h2>
          <small>按北京时间的业务日，含首尾两天，最长 366 天</small>
        </header>
        <div className={c("ops-dialog-body")}>
          <label>
            开始日期
            <input type="date" name="from" required defaultValue={from} />
          </label>
          <label>
            结束日期
            <input type="date" name="to" required defaultValue={to} />
          </label>
          <p className={c("ops-form-error")} role="alert">
            {error}
          </p>
        </div>
        <footer>
          <button type="button" onClick={close}>
            取消
          </button>
          <button type="submit" className={c("ops-primary")}>
            确认
          </button>
        </footer>
      </form>
    </Dialog>
  );
}
