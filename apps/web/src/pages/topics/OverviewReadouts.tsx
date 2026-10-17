import { Fragment, type ReactNode } from "react";
import { btn } from "./topicElements";
import { number, pct, platformName, readoutNumber } from "./topicModel";
import type { Snapshot, Statistics } from "./types";
import s from "./Overview.module.css";

/** Compact presentation of server statistics; local material filters never feed these values. */
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
  return (
    <section className={s.readouts} aria-label="基础概览指标">
      <div className={s.readout}>
        <span>观察到的内容</span>
        {btn(
          <><strong>{readoutNumber(stats.workCount)}</strong><small>篇</small></>,
          "materials", "", s.value,
          { "aria-label": `观察到的内容 ${number(stats.workCount)} 篇，查看材料` },
        )}
        <small>范围内可读 {number(scope.readableWorkCount)} · 待归主题 {number(scope.unclassifiedWorkCount)}</small>
      </div>
      <div className={s.readout}>
        <span>已观察创作者</span>
        <div className={s.value}><strong>{readoutNumber(stats.authorCount)}</strong><small>位</small></div>
        <small>作者身份未知 {number(stats.unknownAuthorCount)} 篇</small>
      </div>
      {platforms.map((p) => (
        <Fragment key={p.platform}>
          <div className={s.readout}>
            <span>{platforms.length > 1 ? `${platformName(p.platform)} · ` : ""}样本爆款率</span>
            <div className={s.value}><strong>{p.highPerformanceThreshold == null ? "未设置" : pct(p.highPerformanceCount, p.knownLikeCount)}</strong></div>
            <small>爆款 / 可判定：{p.highPerformanceThreshold == null ? "—" : readoutNumber(p.highPerformanceCount)} / {readoutNumber(p.knownLikeCount)}</small>
          </div>
          <div className={s.readout}>
            <span>{platforms.length > 1 ? `${platformName(p.platform)} · ` : ""}点赞中位数</span>
            <div className={s.value}><strong>{readoutNumber(p.medianLikes)}</strong></div>
            <small>P90 点赞 {readoutNumber(p.p90Likes)} · 点赞未知 {readoutNumber(p.unknownLikeCount)} 篇</small>
          </div>
        </Fragment>
      ))}
      <div className={s.basis}>
        <div className={s.rule}>
        <span>{platforms.length ? platforms.map((p) => `${platforms.length > 1 ? platformName(p.platform) + " · " : ""}${p.highPerformanceThreshold == null ? "样本爆款规则未设置" : `样本爆款：点赞 ≥ ${number(p.highPerformanceThreshold)}`}`).join("；") : "当前范围暂无表现统计，已有材料仍可阅读。"}</span>
        {btn("规则", "rules", "", "lgi-tm-quiet", { "aria-label": "查看或调整样本爆款规则" })}
        </div>
        {controls}
      </div>
    </section>
  );
}
