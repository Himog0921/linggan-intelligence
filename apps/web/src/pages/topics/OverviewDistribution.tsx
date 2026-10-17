import { useState } from "react";
import { btn } from "./topicElements";
import { number, platformName } from "./topicModel";
import type { PlatformStatistics, Snapshot, Statistics, Work } from "./types";
import s from "./Overview.module.css";
import { materialLikeBins } from "./materialLikeBins";

export function OverviewDistribution({ works, platform, statistics, scope }: {
  works: readonly Work[];
  platform: PlatformStatistics | undefined;
  statistics: Statistics;
  scope: Snapshot["scope"];
}) {
  const [table, setTable] = useState(false);
  const { bins, unknown } = materialLikeBins(works, platform?.highPerformanceThreshold ?? null);
  const max = Math.max(1, ...bins.map((b) => b.refs.length));
  const subset = (label: string, refs: string[]) => btn(label, "subset", refs.join(","), "lgi-tm-quiet", { disabled: !refs.length });
  return (
    <aside className={s.aside} aria-label="材料分布与来源边界">
      <section aria-label="已观察内容点赞分布">
        <div className={s.chartHead}>
          <div><h3>内容样本表现</h3><p>按点赞区间查看已有材料</p></div>
          <div className={s.segment} role="group" aria-label="材料表现视图">
            <button type="button" aria-pressed={!table} onClick={() => setTable(false)}>分布图</button>
            <button type="button" aria-pressed={table} onClick={() => setTable(true)}>表格</button>
          </div>
        </div>
        <p className={s.scope}>{platform ? platformName(platform.platform) : "暂无平台样本"} · 当前返回 {number(works.length)} 篇 · 点赞可判定 {number(works.length - unknown.length)} 篇</p>
        {platform?.highPerformanceThreshold != null ? (
          <p className={s.legend}><i /> 其他区间 <i className={s.signal} /> 整个区间均达到点赞 ≥ {number(platform.highPerformanceThreshold)} 的样本规则</p>
        ) : <p className={s.scope}>样本规则未设置，不判定高表现。</p>}
        {table ? (
          <table className={s.table}>
            <caption>当前返回材料的点赞区间</caption>
            <thead><tr><th scope="col">点赞区间</th><th scope="col">作品数</th><th scope="col">材料</th></tr></thead>
            <tbody>{bins.map((bin) => <tr key={bin.label}><th scope="row">{bin.label}</th><td>{number(bin.refs.length)}</td><td>{subset("查看", bin.refs)}</td></tr>)}</tbody>
          </table>
        ) : (
          <div className={s.plot}>
            <div className={s.axis} aria-hidden="true"><span>{number(max)}</span><span>{number(Math.round(max / 2))}</span><span>0</span></div>
            <div className={s.columns}>{bins.map((bin) => (
              <button type="button" key={bin.label} className={s.column} disabled={!bin.refs.length}
                data-action="subset" data-id={bin.refs.join(",")}
                aria-label={`${bin.label}赞，${number(bin.refs.length)}篇，查看作品`}>
                <span className={s.barSpace}><span className={s.bar + (bin.high ? ` ${s.signal}` : "")} style={{ height: `${bin.refs.length / max * 100}%` }}><b>{number(bin.refs.length)}</b></span></span>
                <span className={s.barLabel}>{bin.label}</span>
              </button>
            ))}</div>
          </div>
        )}
        <div className={s.chartFoot}>
          <span>点赞未知 {number(unknown.length)} 篇，单独保留</span>
          {subset("查看未知指标材料", unknown)}
        </div>
        <p className={s.scope}>图表限当前方向、平台的已返回材料，关键词与排序不改变分布。服务端该平台统计 {number(platform?.workCount)} 篇，点赞未知 {number(platform?.unknownLikeCount)} 篇；库内样本不代表市场趋势或再次发布效果。</p>
        {btn("统计说明 ↗", "method", "", "lgi-tm-quiet")}
      </section>
      <section className={s.boundaries} aria-label="当前资料边界">
        <h3>当前资料边界</h3>
        <dl>
          <div><dt>范围内可读作品</dt><dd>{number(scope.readableWorkCount)} / {number(scope.totalWorkCount)} 篇</dd></div>
          <div><dt>作者身份未知</dt><dd>{number(statistics.unknownAuthorCount)} 篇</dd></div>
          <div><dt>我方覆盖</dt><dd>{scope.ownIdentityState === "unknown" ? "账号范围未明确" : "以已保存账号范围为准"}</dd></div>
        </dl>
        <div className={s.boundaryActions}>{btn("我方矩阵 ↗", "identity", "", "lgi-tm-quiet")}{btn("检查高表现参考来自谁 ↗", "tab", "sources", "lgi-tm-quiet")}</div>
      </section>
    </aside>
  );
}
