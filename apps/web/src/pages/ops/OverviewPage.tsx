import { useOpsLeave } from './useOpsLeave';
import { ActionButton } from '../../shared/ActionButton';
import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useSearchParams } from "react-router";
import * as api from "./api";
import * as f from "./format";
import { Dialog, Status, Tabs } from "./ui";
import { c, peopleTabs } from "./styles";
import { TargetDialog } from "./TargetDialog";
import {
  Delta,
  Matrix,
  OpsGlyph,
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
  const activeAccountCount = data?.matrix.covered
    ? new Set(data.matrix.cells.filter((cell) => cell.published > 0).map((cell) => cell.account)).size
    : null;
  const accountPublishedCount = data?.matrix.covered
    ? data.matrix.cells.reduce((sum, cell) => sum + cell.published, 0)
    : null;
  const perActiveAccount = activeAccountCount && accountPublishedCount !== null
    ? (accountPublishedCount / activeAccountCount).toFixed(1)
    : null;
  const platformTotal = data?.platformShare
    ? data.platformShare.xhs + data.platformShare.douyin
    : null;
  const xhsShare = platformTotal && data?.platformShare
    ? (data.platformShare.xhs / platformTotal) * 100
    : null;
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
        className={c("ops-overview", "ops-overview-v2")}
        data-ops-overview
        aria-busy={result.isFetching}
        hidden={result.isError}
      >
        {data ? (
          <>
            <section className={c("ops-hero-v2")} aria-label="经营结果与加微目标">
              <div className={c("ops-results-v2")}>
                <div className={c("ops-lead-readout-v2")}>
                  <div className={c("ops-readout-label-v2")}>
                    <OpsGlyph name="lead" />
                    <h2 id="ops-hero-title">{f.sliceTitle[data.window.slice]}</h2>
                  </div>
                  <div className={c("ops-hero-number-v2")}>
                    <strong
                      data-state={data.kpi.current.followersGained === null ? "unknown" : undefined}
                      title={data.kpi.current.followersGained === null ? "这段时间未迁入每日数据，未知不等于零" : ""}
                    >
                      {f.number(data.kpi.current.followersGained)}
                    </strong>
                    <small>人</small>
                  </div>
                  <div className={c("ops-readout-footer-v2")}>
                    <span>
                      {data.window.previous
                        ? `上期同天数 ${f.number(data.kpi.previous.followersGained)}`
                        : "本期尚无完整迁入日，不比上期"}
                    </span>
                    <Delta value={data.kpi.delta.followersGainedPct} />
                  </div>
                </div>
                <div className={c("ops-hero-secondary-v2")}>
                  <div className={c("ops-secondary-readout-v2")}>
                    <div className={c("ops-readout-label-v2")}>
                      <OpsGlyph name="open" />
                      <span>链接打开</span>
                    </div>
                    <div className={c("ops-submetric-value-v2")}>
                      <strong>{f.number(data.kpi.current.linksOpened)}</strong>
                      <small>次</small>
                    </div>
                    <div className={c("ops-readout-footer-v2")}>
                      <span>每日记录 · 不分业务</span>
                      <Delta value={data.kpi.delta.linksOpenedPct} />
                    </div>
                  </div>
                  <div className={c("ops-secondary-readout-v2")}>
                    <div className={c("ops-readout-label-v2")}>
                      <OpsGlyph name="ratio" />
                      <span>加微 / 打开</span>
                    </div>
                    <div className={c("ops-submetric-value-v2")}>
                      <strong>{f.percent(data.kpi.current.conversionRate)}</strong>
                    </div>
                    <div className={c("ops-readout-footer-v2")}>
                      <span title="同周期已记录加微除以链接打开；不是逐次点击的转化归因">
                        同期比值 · 非逐篇归因
                      </span>
                    </div>
                  </div>
                </div>
              </div>
              <div className={c("ops-goal-v2")}>
                <Target data={data} open={setTarget} />
              </div>
            </section>

            <section className={c("ops-efficiency-v2")} aria-label="内容规模与账号效率">
              <div className={c("ops-section-kicker-v2")}>
                <span>内容规模与效率</span>
                <span>发布与阅读量按所选业务 · 加微按全部业务</span>
              </div>
              <div className={c("ops-efficiency-grid-v2")}>
                <div className={c("ops-efficiency-item-v2")}>
                  <div className={c("ops-readout-label-v2")}>
                    <OpsGlyph name="publish" /><span>发布作品</span>
                  </div>
                  <strong>{f.number(data.kpi.current.published)}</strong>
                  <div className={c("ops-readout-footer-v2")}>
                    <span>去重后的主发布记录</span>
                    <Delta value={data.kpi.delta.publishedPct} />
                  </div>
                </div>
                <div className={c("ops-efficiency-item-v2")}>
                  <div className={c("ops-readout-label-v2")}>
                    <OpsGlyph name="accounts" /><span>有发布账号</span>
                  </div>
                  <strong>{f.number(activeAccountCount)}</strong>
                  <div className={c("ops-readout-footer-v2")}>
                    <span>该期账号矩阵中发布 &gt; 0</span>
                  </div>
                </div>
                <div className={c("ops-efficiency-item-v2")}>
                  <div className={c("ops-readout-label-v2")}>
                    <OpsGlyph name="yield" /><span>单活跃账号产出</span>
                  </div>
                  <strong>{perActiveAccount ?? "—"}</strong>
                  <div className={c("ops-readout-footer-v2")}>
                    <span>已归属作品 / 有发布账号</span>
                  </div>
                </div>
                <div className={c("ops-efficiency-item-v2")}>
                  <div className={c("ops-readout-label-v2")}>
                    <OpsGlyph name="hits" /><span>小红书爆款</span>
                  </div>
                  <strong>{f.number(data.kpi.current.hits)}</strong>
                  <div className={c("ops-readout-footer-v2")}>
                    <span title="已填写的最新阅读量达到 5000；无阅读量不等于非爆款">
                      ≥ 5000 阅读 · 样本非完整
                    </span>
                    <Delta value={data.kpi.delta.hitsPct} />
                  </div>
                </div>
                <div className={c("ops-efficiency-item-v2")}>
                  <div className={c("ops-readout-label-v2")}>
                    <OpsGlyph name="platform" /><span>小红书发布占比</span>
                  </div>
                  <strong>{f.percent(xhsShare)}</strong>
                  <div className={c("ops-readout-footer-v2")}>
                    <span>
                      小红书 {f.number(data.platformShare?.xhs)} · 抖音 {f.number(data.platformShare?.douyin)}
                    </span>
                  </div>
                </div>
              </div>
            </section>

            <section className={c("ops-analytics-v2")} aria-label="长期经营趋势及本期产出">
              <div className={c("ops-analysis-panel-v2")}>
                <div className={c("ops-section-head-v2")}>
                  <div className={c("ops-section-heading-v2")}>
                    <OpsGlyph name="trend" />
                    <h2 id="ops-trend-title">长期经营趋势</h2>
                  </div>
                  <Tabs
                    label="趋势粒度" id="ops-granularity" compact
                    values={[[ "day", "按日" ], [ "week", "按周" ], [ "month", "按月" ]]}
                    current={granularity}
                    onSelect={(value) => {
                      if (value === granularity) void trend.refetch();
                      else setGranularity(value);
                    }}
                  />
                </div>
                <div className={c("ops-analysis-legend-v2")}>
                  <span><i data-kind="followers" />加微</span>
                  <span><i data-kind="hits" />爆款</span>
                  <span className={c("ops-analysis-hint-v2")}>
                    近 90 日 / 12 周 / 6 月 · 与顶部周期独立
                  </span>
                </div>
                {trend.isError ? (
                  <div className={c("ops-chart")}>
                    <p className={c("ops-chart-empty")}>
                      趋势读取失败（{api.reason(trend.error)}），可重新点选时间粒度重试
                    </p>
                  </div>
                ) : trend.data ? (
                  <TrendChart points={trend.data.points} granularity={granularity} />
                ) : (
                  <div className={c("ops-chart")} />
                )}
              </div>
              <div className={c("ops-analysis-panel-v2")}>
                <div className={c("ops-section-head-v2")}>
                  <div className={c("ops-section-heading-v2")}>
                    <OpsGlyph name="publish" />
                    <h2>本期每日产出</h2>
                  </div>
                  <Tabs
                    label="产出指标" id="ops-output-metric" compact
                    values={[[ "published", "发布" ], [ "followers", "加微" ]]}
                    current={metric} onSelect={setMetric}
                  />
                </div>
                <div className={c("ops-output-people-v2")}>
                  <Tabs
                    label="运营人" id="ops-output-people" compact
                    values={peopleTabs(data.dailyOutput.operators)}
                    current={data.dailyOutput.operators.includes(person) ? person : ""}
                    onSelect={setPerson}
                  />
                </div>
                <OutputChart
                  output={data.dailyOutput}
                  metric={metric}
                  person={data.dailyOutput.operators.includes(person) ? person : ""}
                />
              </div>
            </section>

            <Ranking data={data} />

            <section className={c("ops-platform-v2")} aria-labelledby="ops-platform-title">
              <div className={c("ops-section-head-v2")}>
                <div className={c("ops-section-heading-v2")}>
                  <OpsGlyph name="platform" />
                  <h2 id="ops-platform-title">平台发布结构</h2>
                </div>
                <span className={c("ops-section-aside-v2")}>仅统计发布，无法将加微归属平台</span>
              </div>
              <Share data={data} />
            </section>

            <section className={c("ops-matrix-section-v2")} aria-labelledby="ops-matrix-title">
              <div className={c("ops-section-head-v2")}>
                <div className={c("ops-section-heading-v2")}>
                  <OpsGlyph name="matrix" />
                  <h2 id="ops-matrix-title">运营人 × 账号矩阵</h2>
                </div>
                <span className={c("ops-section-aside-v2")}>
                  单元格为发布数，括号内为爆款数 · 横向滚动
                </span>
              </div>
              <Matrix data={data} />
            </section>
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
export function CustomDialog({
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
  const [draftFrom, setDraftFrom] = useState(from);
  const [draftTo, setDraftTo] = useState(to);
  const guard = useOpsLeave({ dirty: draftFrom !== from || draftTo !== to, onClose: close });
  return (
    <><Dialog title="ops-custom-title" onClose={guard.requestClose}>
      <form
        id="ops-custom-form"
        onInvalid={event => { event.preventDefault(); const input = event.target; if (input instanceof HTMLInputElement) setError(input.validity.valueMissing ? "请选择开始和结束日期" : input.validity.badInput ? "请输入完整有效的日期" : "开始日期不能晚于结束日期，最长 366 天"); }}
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
            <input type="date" name="from" required value={draftFrom} onChange={event => { setDraftFrom(event.target.value); setError(''); }} max={draftTo || undefined} />
          </label>
          <label>
            结束日期
            <input type="date" name="to" required value={draftTo} onChange={event => { setDraftTo(event.target.value); setError(''); }} min={draftFrom || undefined} />
          </label>
          <p className={c("ops-form-error")} role="alert">
            {error}
          </p>
        </div>
        <footer>
          <ActionButton type="button" onClick={guard.requestClose}>
            取消
          </ActionButton>
          <ActionButton type="submit" variant="primary">
            确认
          </ActionButton>
        </footer>
      </form>
    </Dialog>{guard.confirmation}</>
  );
}
