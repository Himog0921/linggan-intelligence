import { Fragment, type ReactNode } from "react";
import { btn } from "./topicElements";
import { number, pct, platformName, readoutNumber } from "./topicModel";
import type { Snapshot, Statistics } from "./types";
import s from "./Overview.module.css";

/**
 * One readout = label row + figure (value, then its facts). The band aligns the figure with the
 * like-distribution baseline. No yesterday column: the topic-map API is a current snapshot only,
 * so a permanent "—" placeholder carried no information (Mog 2026-11-27 redesign).
 */
function Readout({
  label, value, valueText, unit, detail, drill = false,
}: {
  label: string;
  value: ReactNode;
  valueText?: string;
  unit?: string;
  detail: ReactNode;
  drill?: boolean;
}) {
  return (
    <div className={s.readout}>
      <div className={s.label}>{label}</div>
      <div className={s.figure}>
        <div className={s.value}>{drill
          ? btn(
              <><strong>{value}</strong>{unit ? <small>{unit}</small> : null}</>,
              "materials", "", s.valueAction,
              { "aria-label": label + " " + (valueText || "") + " " + (unit || "") + "，查看材料" },
            )
          : <><strong>{value}</strong>{unit ? <small>{unit}</small> : null}</>
        }</div>
        <div className={s.detail}>{detail}</div>
      </div>
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
        <Readout label="观察到的内容" drill
          value={readoutNumber(stats.workCount)} valueText={number(stats.workCount)} unit="篇"
          detail={<><span>可读 <b>{number(scope.readableWorkCount)}</b></span></>}
        />
        <Readout label="已观察创作者"
          value={readoutNumber(stats.authorCount)} unit="位"
          detail={<span>身份未知 <b>{number(stats.unknownAuthorCount)}</b> 篇</span>}
        />
        {platforms.map((p) => (
          <Fragment key={p.platform}>
            <Readout
              label={(multiple ? platformName(p.platform) + " · " : "") + "样本爆款率"}
              value={p.highPerformanceThreshold == null ? "未设置" : pct(p.highPerformanceCount, p.knownLikeCount)}
              detail={<><span>达标 <b>{p.highPerformanceThreshold == null ? "—" : readoutNumber(p.highPerformanceCount)}</b></span><span>可判定 <b>{readoutNumber(p.knownLikeCount)}</b></span>
                <span className={s.visuallyHidden}>{p.highPerformanceThreshold == null ? "样本爆款规则未设置" : "样本爆款：点赞 ≥ " + number(p.highPerformanceThreshold)}</span>
              </>}
            />
            <Readout
              label={(multiple ? platformName(p.platform) + " · " : "") + "点赞中位数"}
              value={readoutNumber(p.medianLikes)}
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
