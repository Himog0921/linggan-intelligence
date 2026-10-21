import { ActionButton } from '../../shared/ActionButton';
import { useState, type CSSProperties } from "react";
import type { Overview, PeriodType } from "./api";
import * as f from "./format";
import { c } from "./styles";
import { Sparkline } from "./Charts";
type OpsGlyphName =
  | "lead"
  | "open"
  | "ratio"
  | "publish"
  | "accounts"
  | "yield"
  | "hits"
  | "platform"
  | "trend"
  | "matrix";

/** Lightweight LIDS-style geometric glyphs; decorative only, never encode status alone. */
export function OpsGlyph({ name }: { name: OpsGlyphName }) {
  const paths: Record<OpsGlyphName, readonly [string, string]> = {
    lead: ["M3 17V5h5M21 7v12h-5M5 15l5-5 4 3 6-7", "M16 6h4v4"],
    open: ["M5 3h6M5 3v18h14v-6M9 9h3M9 13h3", "M12 12l8-8m-5 0h5v5"],
    ratio: ["M4 5h6v6H4zM14 13h6v6h-6zM9 15l6-6", "M14 5h6v6h-6z"],
    publish: ["M3 5h8v8H3zM14 5h7M14 9h7M3 17h18M3 21h13", "M16 21h5"],
    accounts: ["M4 5h6v6H4zM14 5h6v6h-6zM4 15h6v6H4z", "M14 15h6v6h-6z"],
    yield: ["M4 20V14h4v6M10 20v-9h4v9M16 20V4h4v16", "M3 6h7m-3-3 3 3-3 3"],
    hits: ["M12 2l2.8 6.6L22 12l-7.2 3.4L12 22l-2.8-6.6L2 12l7.2-3.4z", "M9 12h6"],
    platform: ["M3 5h18M3 10h18M3 15h18M3 20h18", "M3 10h12"],
    trend: ["M3 19V4M3 19h18M5 15c3-6 5-6 8-3s4 1 7-7", "M16 5h4v4"],
    matrix: ["M3 3h18v18H3zM9 3v18M15 3v18M3 9h18M3 15h18", "M9 9h6v6H9z"],
  };
  const [frame, signal] = paths[name];
  return (
    <svg
      className={c("ops-glyph-v2")}
      viewBox="0 0 24 24"
      fill="none"
      aria-hidden="true"
      focusable="false"
    >
      <path className={c("ops-glyph-frame-v2")} d={frame} />
      <path className={c("ops-glyph-signal-v2")} d={signal} />
    </svg>
  );
}

export function Delta({ value }: { value: number | null }) {
  const delta = f.delta(value);
  return (
    <span className={c("ops-delta")} data-tone={delta.tone || undefined}>
      {delta.text}
    </span>
  );
}
export function Target({
  data,
  open,
}: {
  data: Overview;
  open: (type: PeriodType) => void;
}) {
  const t = data.target;
  const rev = t.revision;
  const period = t.periodType === "month" ? "本月目标" : "本周目标";
  const known = t.periodFollowers !== null && t.elapsedDays !== null;
  return (
    <div className={c("ops-target")} id="ops-target" aria-label="加微目标">
      {t.state === "not_applicable" ? (
        <>
          <div className={c("ops-target-head")}>
            <span>加微目标</span>
            <b data-state="unknown">自定义时段不对应目标</b>
          </div>
          <p className={c("ops-note-line")}>
            目标按周、按月设；切到本周或本月可以查看和设定。
          </p>
        </>
      ) : (
        <>
          <div className={c("ops-target-head")}>
            <div className={c("ops-target-label")}>
              <span>{`${period}（${t.periodKey}）${t.pace === "achieved" ? " · 目标已达成" : ""}`}</span>
              <button type="button" className={c("ops-target-edit")}
                aria-label={t.state === "set" ? "修改目标" : "设定目标"}
                title={t.state === "set" ? "修改目标" : "设定目标"}
                onClick={() => open(t.periodType === "month" ? "month" : "week")}>
                <svg viewBox="0 0 24 24" fill="none" aria-hidden="true" focusable="false">
                  <path d="m15 5 4 4M4 20l5-1L20 8l-4-4L5 15l-1 5Z" />
                </svg>
              </button>
            </div>
            <b data-state={t.state === "not_set" ? "unknown" : undefined}>
              {t.state === "set" ? f.number(rev?.targetValue) : "未设定"}
            </b>
          </div>
          {t.state === "not_set" ? (
            <p className={c("ops-note-line")}>没设目标时不估算、不判断节奏。</p>
          ) : (
            <>
              <div
                className={c("ops-progress")}
                role="img"
                aria-label={`完成 ${f.percent(t.achievedPct)}，时间进度 ${f.percent(t.timeProgressPct)}`}
                style={
                  {
                    "--ops-fill": `${Math.min(t.achievedPct || 0, 100)}%`,
                    "--ops-time": `${Math.min(t.timeProgressPct || 0, 100)}%`,
                  } as CSSProperties
                }
              >
                <i />
                <b />
              </div>
              <dl className={c("ops-target-grid")}>
                {[
                  [
                    "本期已完成",
                    known
                      ? `${f.number(t.periodFollowers)}（${f.percent(t.achievedPct)}）`
                      : "还没有迁入的数据",
                  ],
                  [
                    "时间进度",
                    known
                      ? `${f.percent(t.timeProgressPct)}（按已迁入 ${t.elapsedDays} 天）`
                      : "—",
                  ],
                  ["每日还需", f.number(t.dailyNeeded)],
                  ["预计周期末", f.number(t.projectedEnd)],
                ].map(([label, value]) => (
                  <div key={label}>
                    <dt>{label}</dt>
                    <dd>{value}</dd>
                  </div>
                ))}
              </dl>
            </>
          )}

        </>
      )}
    </div>
  );
}
export function Funnel({ data }: { data: Overview }) {
  const { kpi: k, funnel: fn } = data;
  const openRate = fn.publishToOpenAvailable
    ? `发布 → 打开 ${f.percent(fn.current.publishToOpen)}`
    : "发布 → 打开：选了业务时不计算（打开不分业务）";
  const stages: [string, string, number | null, string, boolean][] = [
    ["发布", f.number(k.current.published), k.delta.publishedPct, "", false],
    ["有效发布", "未接入", null, "自动发现我方帖子上线后接入", true],
    [
      "链接打开",
      f.number(k.current.linksOpened),
      k.delta.linksOpenedPct,
      `${openRate} · ${f.delta(k.delta.linksOpenedPct).text}`,
      false,
    ],
    [
      "加微",
      f.number(k.current.followersGained),
      k.delta.followersGainedPct,
      `打开 → 加微 ${f.percent(fn.current.openToFollow)} · ${f.delta(k.delta.followersGainedPct).text}`,
      false,
    ],
  ];
  const watched = [
    k.delta.publishedPct,
    k.delta.linksOpenedPct,
    k.delta.followersGainedPct,
  ].some((v) => v !== null && v <= -20);
  return (
    <section className={c("ops-block")} aria-labelledby="ops-funnel-title">
      <h2 id="ops-funnel-title" className={c("ops-block-title")}>
        转化漏斗诊断
      </h2>
      <div className={c("ops-funnel-row")}>
        <ol className={c("ops-funnel")} id="ops-funnel" aria-label="转化漏斗">
          {stages.map(([label, value, delta, note, unknown]) => {
            const watch = delta !== null && delta <= -20;
            return (
              <li key={label} data-watch={String(watch)}>
                <span>{watch ? `${label} · 观察中` : label}</span>
                <b data-state={unknown ? "unknown" : undefined}>{value}</b>
                <small>{note || f.delta(delta).text}</small>
              </li>
            );
          })}
        </ol>
        <dl className={c("ops-stats")}>
          <div>
            <dt title="加微 ÷ 链接打开">转化率</dt>
            <dd>{f.percent(k.current.conversionRate)}</dd>
          </div>
          <div>
            <dt title="所选时段发布的小红书帖子中，最新阅读量不少于 5000 的篇数">
              爆款数
            </dt>
            <dd>
              <b>{f.number(k.current.hits)}</b>
              <Delta value={k.delta.hitsPct} />
            </dd>
          </div>
        </dl>
      </div>
      <p className={c("ops-note-line")}>
        {watched
          ? "标「观察中」的环节比上期下降 20% 以上。"
          : "各环节转化平稳，无明显掉点。"}
      </p>
    </section>
  );
}
export function Attention({ data }: { data: Overview }) {
  const [open, setOpen] = useState(false);
  const { items, rest } = data.attention;
  return (
    <section
      className={c("ops-block", "ops-attention")}
      aria-labelledby="ops-attention-title"
    >
      <h2 id="ops-attention-title" className={c("ops-block-title")}>
        今日必须处理
      </h2>
      <ol className={c("ops-attention-list")} id="ops-attention">
        {items.length ? (
          [...items, ...rest].map((item, i) => {
            const t = f.attention(item);
            return (
              <li
                key={i}
                hidden={i >= items.length && !open}
                data-severity={item.severity}
              >
                <i aria-hidden="true" />
                <b>{t.text}</b>
                <small>{t.action}</small>
              </li>
            );
          })
        ) : (
          <li className={c("ops-attention-empty")}>各项指标在正常波动范围内</li>
        )}
      </ol>
      <p className={c("ops-note-line")} id="ops-attention-more">
        {items.length && rest.length ? (
          <ActionButton
            type="button"
            aria-expanded={open}
            aria-controls="ops-attention"
            onClick={() => setOpen(!open)}
          >
            {open ? "收起" : `另有 ${rest.length} 项，展开`}
          </ActionButton>
        ) : null}
      </p>
    </section>
  );
}
export function Ranking({ data }: { data: Overview }) {
  const headers = ["运营人", "负责账号", "发布", "爆款", "链接打开", "加微", "加微 / 打开", "近 7 日"];
  return (
    <section className={c("ops-ranking-v2")} aria-labelledby="ops-ranking-title">
      <div className={c("ops-section-head-v2")}>
        <h2 id="ops-ranking-title">运营人表现</h2>
        <span className={c("ops-section-aside-v2")}>按加微排序 · 每日数据不分业务</span>
      </div>
      <div className={c("ops-table-wrap", "ops-ranking-scroll-v2")} tabIndex={0}
           aria-label="运营人表现排行，可横向滚动">
        <table className={c("ops-table", "ops-ranking", "ops-ranking-table-v2")}>
          <thead>
            <tr>{headers.map((title, i) => (
              <th key={title} scope="col"
                  className={i > 0 && i < 7 ? c("ops-number") : undefined}
                  title={title === "加微 / 打开" ? "按运营人同周期记账数计算，不表示逐篇作品归因" : undefined}>
                {title}
              </th>
            ))}</tr>
          </thead>
          <tbody>
            {data.ranking.length ? data.ranking.map((row) => (
              <tr key={row.operator}>
                <th scope="row" className={c("ops-ranking-person-v2")}>
                  <b title={row.operator}>{row.operator}</b>
                  <small>{row.platform ? f.platformLabel[row.platform] : "平台未知"}</small>
                </th>
                <td className={c("ops-number")}>{f.number(row.responsibleAccounts)}</td>
                <td className={c("ops-number")}>{f.number(row.published)}</td>
                <td className={c("ops-number")}>{f.number(row.hits)}</td>
                <td className={c("ops-number")}>{f.number(row.linksOpened)}</td>
                <td className={c("ops-number", "ops-ranking-leads-v2")}>{f.number(row.followersGained)}</td>
                <td className={c("ops-number")}>{f.percent(row.conversionRate)}</td>
                <td><Sparkline values={row.sparkline} /></td>
              </tr>
            )) : (
              <tr><td colSpan={headers.length} className={c("ops-empty")}>
                {f.blockEmptyText(data.window, "当期没有运营数据")}
              </td></tr>
            )}
          </tbody>
        </table>
      </div>
    </section>
  );
}

export function Share({ data }: { data: Overview }) {
  const share = data.platformShare;
  const total = share ? share.xhs + share.douyin : 0;
  return (
    <div id="ops-platform-share">
      {!data.window.covered || !share ? (
        <p className={c("ops-chart-empty")}>
          {f.blockEmptyText(data.window, "当期无发布")}
        </p>
      ) : !total ? (
        <p className={c("ops-chart-empty")}>当期无发布</p>
      ) : (
        <>
          <div
            className={c("ops-share")}
            role="img"
            aria-label={`小红书 ${share.xhs} 篇，抖音 ${share.douyin} 篇`}
          >
            <span
              className={c("ops-xhs")}
              title={`${share.xhs} 篇`}
              style={{ flex: share.xhs }}
            />
            <span
              className={c("ops-douyin")}
              title={`${share.douyin} 篇`}
              style={{ flex: share.douyin }}
            />
          </div>
          <p className={c("ops-share-labels")}>
            {(
              [
                ["xhs", share.xhs],
                ["douyin", share.douyin],
              ] as const
            ).map(([p, value]) => (
              <span key={p}>
                <i className={c(`ops-${p}`)} />
                {f.platformLabel[p]} {f.number(Number(value))} 篇（
                {f.percent((Number(value) / total) * 100)}）
              </span>
            ))}
          </p>
        </>
      )}
    </div>
  );
}
export function Matrix({ data }: { data: Overview }) {
  const m = data.matrix,
    max = Math.max(...m.cells.map((cell) => cell.published));
  return (
    <div
      className={c(
        "ops-table-wrap",
        "ops-inline-table",
        ...(m.cells.length ? ["ops-matrix-wrap"] : []),
      )}
      id="ops-matrix"
      tabIndex={0}
      aria-label="运营人与账号矩阵，可滚动"
    >
      {m.cells.length ? (
        <table className={c("ops-table", "ops-matrix")}>
          <thead>
            <tr>
              <th scope="col">账号</th>
              <th scope="col">平台</th>
              {m.operators.map((name) => (
                <th key={name} scope="col" className={c("ops-number")}>
                  {name}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {m.accounts.map((account) => (
              <tr key={account.ref}>
                <th scope="row">
                  <span className={c("ops-matrix-name")} title={account.name}>
                    {account.name}
                  </span>
                  {account.retired ? (
                    <>
                      {" "}
                      <span
                        className={c("ops-tag")}
                        title="这个账号已在工作台删除"
                      >
                        已停用
                      </span>
                    </>
                  ) : null}
                </th>
                <td>{f.platformLabel[account.platform]}</td>
                {m.operators.map((operator) => {
                  const cell = m.cells.find(
                    (item) =>
                      item.account === account.ref &&
                      item.operator === operator,
                  );
                  return cell ? (
                    <td
                      key={operator}
                      data-heat=""
                      style={
                        {
                          "--ops-heat": String(cell.published / max),
                        } as CSSProperties
                      }
                      title={`${operator} 在 ${account.name} 发布 ${cell.published} 篇，爆款 ${cell.hits} 篇`}
                    >
                      {cell.hits
                        ? `${cell.published}（${cell.hits}）`
                        : cell.published}
                    </td>
                  ) : (
                    <td key={operator} className={c("ops-matrix-empty")}>
                      ·
                    </td>
                  );
                })}
              </tr>
            ))}
          </tbody>
        </table>
      ) : (
        <p className={c("ops-chart-empty")}>
          {f.blockEmptyText(data.window, "当期无矩阵数据")}
        </p>
      )}
    </div>
  );
}
