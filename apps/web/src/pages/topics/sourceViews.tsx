import { c as cls } from "./styles";
import { badge, btn, empty } from "./topicElements";
import {
  assignmentKey,
  authorUnavailable,
  citationText,
  commentOnlyResearch,
  coverageState,
  discussionKeys,
  discussionUnits,
  esc,
  known,
  number,
  platformName,
  publishedTime,
  readLabel,
  sharesDiscussion,
  title,
} from "./topicModel";
import type * as T from "./types";
export const materialSourceNote = (w: T.Work) =>
  authorUnavailable(w) ? (
    <>
      <p className={cls("lgi-tm-note")}>
        {[
          "作者原文当前未取得或不可用。",
          commentOnlyResearch(w)
            ? "已有合格评论与讨论继续可读。"
            : "其他来源的可读范围以当前复核结果为准。",
        ].join("")}
      </p>
    </>
  ) : (
    ""
  );
export function researchCoverage(w: T.Work) {
  if (!w?.research) return "";
  const c = w.research.core?.coverage,
    status = coverageState(w);
  if (!c)
    return (
      <>
        <p className={cls("lgi-tm-note")}>
          {Array.isArray(w.research.core?.units)
            ? "当前研究的处理范围尚未取得。"
            : "旧研究的处理范围未记录，讨论边界尚未补齐。"}
        </p>
      </>
    );
  return (
    <>
      <p
        className={cls(
          "lgi-tm-research-coverage " +
            String(status === "partial" ? "is-partial" : ""),
        )}
      >
        <strong>
          {status === "complete"
            ? "当前可读来源已处理"
            : status === "partial"
              ? "部分来源已处理"
              : "已处理范围尚未确认"}
        </strong>
        <span>
          {[
            "已处理 ",
            number(c.coveredChars),
            " / ",
            number(c.sourceChars),
            " 字符 · ",
            number(c.completedWindows),
            " / ",
            number(c.totalWindows),
            " 段来源",
          ].join("")}
        </span>
        {status !== "complete" ? (
          <>
            <span>{"已有结果可读，未处理部分的讨论与归属仍未知。"}</span>
          </>
        ) : (
          <>
            <span>
              {"覆盖范围限于本次可读来源，不代表平台全文或领域观察完整。"}
            </span>
          </>
        )}
      </p>
    </>
  );
}
export function researchCoverageSummary(ns: T.Work[]) {
  const rows = [...new Map(ns.map((w) => [w.workRef, w])).values()].filter(
      (w) => w.research,
    ),
    counts = { complete: 0, partial: 0, unknown: 0 };
  rows.forEach((w) => counts[coverageState(w)]++);
  return rows.length ? (
    <>
      <p className={cls("lgi-tm-note")}>
        {[
          rows.length,
          " 篇当前材料有研究结果：",
          counts.complete,
          " 篇当前可读来源已处理 · ",
          counts.partial,
          " 篇仅处理部分 · ",
          counts.unknown,
          " 篇处理范围未确认。一个讨论可涉及多个主题，各组数量不相加为总量。",
        ].join("")}
      </p>
    </>
  ) : (
    ""
  );
}
export function performancePair(ns: T.Work[]) {
  const high = ns.find(
    (w) =>
      !w.own &&
      known(w.followerCount) &&
      w.followerCount <= 1000 &&
      known(w.likes) &&
      w.likes >= 500,
  );
  const low =
    high &&
    ns.find(
      (w) =>
        !w.own &&
        w.workRef !== high.workRef &&
        w.platform === high.platform &&
        known(w.likes) &&
        w.likes <= 20 &&
        sharesDiscussion(high, w),
    );
  if (!high || !low)
    return empty(
      "当前同讨论、同平台的高低对照尚不充分",
      "先看已有材料，不强凑避坑结论。低互动不等于失败。",
    );
  const common = new Set(
    [...discussionKeys(high)].filter((key) => discussionKeys(low).has(key)),
  );
  return (
    <>
      <div className={cls("lgi-tm-performance-pair")}>
        {[high, low].map((w, i) => {
          const d = discussionUnits(w).find((d) =>
              d.assignments.some((a) => common.has(assignmentKey(a))),
            ),
            quote = citationText(w, d?.evidence);
          return (
            <>
              <article className={cls("lgi-tm-panel")}>
                {badge(i ? "低互动对照 · 非失败判定" : "低粉高赞参考", !i)}
                <h4>{esc(title(w))}</h4>
                <p>
                  {[
                    esc(platformName(w.platform)),
                    " · ",
                    number(w.likes),
                    "赞 · 粉丝",
                    number(w.followerCount),
                    " · ",
                    esc(publishedTime(w)),
                  ].join("")}
                </p>
                {quote ? (
                  <>
                    <blockquote>{esc(quote)}</blockquote>
                  </>
                ) : (
                  <>
                    <p>{"当前没有可展示的精确引用，请核对原作。"}</p>
                  </>
                )}
                {btn(
                  readLabel(w, "打开原文与已有评论"),
                  "reader",
                  w.workRef,
                  "lgi-tm-quiet",
                )}
              </article>
            </>
          );
        })}
      </div>
    </>
  );
}
export function comparisonScopeNote(item: T.ResearchItem | undefined) {
  const scope = item?.comparisonScope;
  if (!scope) return "";
  const selected = Array.isArray(scope.selectedWorkRefs)
      ? scope.selectedWorkRefs.length
      : null,
    requested = Array.isArray(scope.scopeWorkRefs)
      ? scope.scopeWorkRefs.length
      : null;
  return (
    <>
      <p className={cls("lgi-tm-note")}>
        {[
          "比较依据：本次选取 ",
          number(selected),
          " / ",
          number(requested),
          " 篇作品的讨论片段。",
          esc(scope.boundary || "结论仅覆盖所列依据，未选入的材料仍未比较。"),
        ].join("")}
      </p>
    </>
  );
}
