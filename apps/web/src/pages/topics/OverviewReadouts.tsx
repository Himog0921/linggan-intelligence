import { Fragment, type ReactNode } from "react";
import { btn } from "./topicElements";
import { number, pct, platformName, readoutNumber } from "./topicModel";
import type { Snapshot, Statistics } from "./types";
import s from "./Overview.module.css";

type Glyph = "papers" | "people" | "signal" | "distribution";
const glyphPaths: Record<Glyph, string> = {
  papers: "M4 5h11v15H4zM9 2h11v15M7 9h5M7 13h5",
  people: "M8 11a3 3 0 1 0 0-6 3 3 0 0 0 0 6M2 20c0-4 2-6 6-6s6 2 6 6M17 6c3 0 4 2 4 5M16 15c3 0 5 2 5 5",
  signal: "m12 2 3 7 7 3-7 3-3 7-3-7-7-3 7-3z",
  distribution: "M3 20V4M3 20h18M6 16l5-6 3 3 5-8M15 5h4v4",
};

/** The topic-map API is a current snapshot, not a yesterday-change series. */
function UnavailableDelta({ label }: { label: string }) {
  return (
    <span
      className={s.delta}
      title="当前主题统计接口不提供上一完整自然日的可核对增量；未知不能填成零，也不能复用原型数值。"
      aria-label={label + "未知，暂无可核对的昨日数据"}
    >
      <span aria-hidden="true">—</span><small>{label}</small>
    </span>
  );
}
function GlyphMark({ kind }: { kind: Glyph }) {
  return (
    <svg className={s.glyph} viewBox="0 0 24 24" fill="none" aria-hidden="true" focusable="false">
      <path d={glyphPaths[kind]} />
    </svg>
  );
}
function Readout({
  glyph, label, value, valueText, unit, detail, deltaLabel, drill = false,
}: {
  glyph: Glyph;
  label: string;
  value: ReactNode;
  valueText?: string;
  unit?: string;
  detail: ReactNode;
  deltaLabel: string;
  drill?: boolean;
}) {
  return (
    <div className={s.readout}>
      <div className={s.label}><GlyphMark kind={glyph} /><span>{label}</span></div>
      <div className={s.figure}>
        <div className={s.value}>{drill
          ? btn(
              <><strong>{value}</strong>{unit ? <small>{unit}</small> : null}</>,
              "materials", "", s.valueAction,
              { "aria-label": label + " " + (valueText || "") + " " + (unit || "") + "，查看材料" },
            )
          : <><strong>{value}</strong>{unit ? <small>{unit}</small> : null}</>
        }</div>
        <UnavailableDelta label={deltaLabel} />
      </div>
      <div className={s.detail}>{detail}</div>
    </div>
  );
}
/** All values are server scope; histogram bins in the adjacent panel use loaded works. */
export function OverviewReadouts({
  statistics: stats,
  scope,
  controls,
}: {
  statistics: Statistics;
  scope: Snapshot["scope"];
  controls?: ReactNode;
}) {
  const platforms = stats.platforms.filter((p) => p.workCount !== 0);
  const multiple = platforms.length > 1;
  return (
    <section className={s.readouts} aria-label="基础概览指标">
      <div className={s.metrics}>
        <Readout
          glyph="papers" label="观察到的内容" drill
          value={readoutNumber(stats.workCount)} valueText={number(stats.workCount)} unit="篇" deltaLabel="昨日新增"
          detail={<><span>可读 <b>{number(scope.readableWorkCount)}</b></span><span>待归主题 <b>{number(scope.unclassifiedWorkCount)}</b></span></>}
        />
        <Readout
          glyph="people" label="已观察创作者"
          value={readoutNumber(stats.authorCount)} unit="位" deltaLabel="昨日新增"
          detail={<span>身份未知 <b>{number(stats.unknownAuthorCount)}</b> 篇</span>}
        />
        {platforms.map((p) => (
          <Fragment key={p.platform}>
            <Readout
              glyph="signal"
              label={(multiple ? platformName(p.platform) + " · " : "") + "样本爆款率"}
              value={p.highPerformanceThreshold == null ? "未设置" : pct(p.highPerformanceCount, p.knownLikeCount)}
              deltaLabel="昨日达标"
              detail={<><span>达标 <b>{p.highPerformanceThreshold == null ? "—" : readoutNumber(p.highPerformanceCount)}</b></span><span>可判定 <b>{readoutNumber(p.knownLikeCount)}</b></span>
                <span className={s.visuallyHidden}>{p.highPerformanceThreshold == null ? "样本爆款规则未设置" : "样本爆款：点赞 ≥ " + number(p.highPerformanceThreshold)}</span>
              </>}
            />
            <Readout
              glyph="distribution"
              label={(multiple ? platformName(p.platform) + " · " : "") + "点赞中位数"}
              value={readoutNumber(p.medianLikes)} deltaLabel="较昨日"
              detail={<><span>P90 <b>{readoutNumber(p.p90Likes)}</b></span><span>点赞未知 <b>{readoutNumber(p.unknownLikeCount)}</b> 篇</span></>}
            />
          </Fragment>
        ))}
      </div>
      {platforms.length === 0 ? (
        <div className={s.noPlatform}>当前范围暂无可判定的平台表现，已有材料仍可阅读。</div>
      ) : null}
      {controls ? <div className={s.controls}>{controls}</div> : null}
    </section>
  );
}
