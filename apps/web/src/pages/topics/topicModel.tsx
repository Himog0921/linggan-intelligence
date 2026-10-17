import type { ReactNode } from "react";
import { ApiError } from "../../shared/api";
import { c as cls } from "./styles";
import type * as T from "./types";

// Pure presentation and source-identity algorithms; no page or dialog state.
export const esc = (value: unknown) => String(value ?? "");
export const list = <V,>(value: V[] | null | undefined): V[] =>
  Array.isArray(value) ? value : [];
export const known = (value: unknown): value is number =>
  typeof value === "number" && Number.isFinite(value);
export const number = (value: unknown) =>
  known(value) ? value.toLocaleString("zh-CN") : "—";
export const pct = (n: unknown, d: unknown) =>
  known(n) && known(d) && d > 0 ? `${((100 * n) / d).toFixed(1)}%` : "—";
export const normalizeSearch = (text: unknown) =>
  String(text || "")
    .normalize("NFKC")
    .toLocaleLowerCase()
    .trim();
export const stageNames: Record<string, string> = {
  discover_understand: "发现与理解",
  seek_assessment: "求助与评估",
  choose_support: "选择支持",
  begin_practice: "开始实践",
  long_term_manage: "长期管理",
};
export const fallbackStages = Object.entries(stageNames).map(
  ([stage, label]) => ({ stage, label }),
);
export const otherNames: Record<string, string> = {
  cross_stage: "跨阶段综合",
  general_background: "通用背景",
  unclear: "已分析但阶段不明",
  pending: "待分析 / 材料不足",
};
export function caught(error: unknown) {
  return error instanceof Error ? error : new Error("读取未成功");
}
export function errorStatus(error: unknown) {
  return error instanceof ApiError && error.failure.kind !== "network"
    ? error.failure.status
    : undefined;
}
export const title = (work: Pick<T.Work, "title"> | undefined) =>
  work?.title || "标题尚未取得";
export const readableMaterial = (w: T.Work | undefined) =>
  Boolean(w?.readable || w?.research?.readable);
export const commentOnlyResearch = (w: T.Work | undefined) =>
  w?.research?.readable === true &&
  w.research.authorSourceState === "source_unavailable" &&
  w.research.commentSourceState === "available";
export const authorUnavailable = (w: T.Work | undefined) =>
  w?.readable === false ||
  w?.research?.authorSourceState === "source_unavailable";
export const readLabel = (w: T.Work, label = "核对原文与讨论") =>
  authorUnavailable(w)
    ? commentOnlyResearch(w)
      ? "查看已有评论与讨论"
      : "核对当前可用材料"
    : label;
export const labelState = (value: string | undefined | null) =>
  Object.assign({
    provisional: "暂定定义",
    pending: "待分析",
    blocked: "当前被阻塞",
    active: "使用中",
    persisted: "已保存",
    candidate: "候选方向",
    published: "已发布定义",
    paused: "已暂停",
    running: "正在处理",
    queued: "待处理",
    completed: "已处理",
    completed_with_failures: "部分处理",
    failed: "处理失败",
    stopped: "已停止",
    cancelled: "已停止",
    daily_limit: "日额度已用尽",
    budget_exhausted: "额度已用尽",
    deferred: "额度延后",
    unknown: "未知",
    not_configured: "尚未设置",
    ready: "已就绪",
  } as Record<string, string>)[value || ""] ||
  value ||
  "未知";
export function publicationOrder(w: T.Work) {
  const value = String(w.publishedAt || "");
  const stamp = /^\d{13}$/.test(value) ? Number(value) : Date.parse(value);
  return Number.isFinite(stamp) ? stamp : 0;
}
export function publishedTime(w: T.Work) {
  if (w.publishedAtSourceText && !/^\d{13}$/.test(w.publishedAtSourceText))
    return w.publishedAtSourceText;
  if (!w.publishedAt && !w.publishedAtSourceText) return "发布时间未知";
  const raw = String(w.publishedAtSourceText || w.publishedAt),
    date = new Date(/^\d{13}$/.test(raw) ? Number(raw) : raw);
  if (Number.isNaN(date.getTime())) return raw;
  return `${date.toLocaleDateString("zh-CN")} · ${w.publishedAtPrecision === "exact" ? "发布时间" : "时间精度见来源"}`;
}
export function legacyConcept(t: T.Topic | undefined | null) {
  return (
    t?.lifecycleState === "candidate" &&
    t?.core?.qualityState === "legacy_candidate"
  );
}
export function platformStats(stats: T.Statistics | undefined | null) {
  return list(stats?.platforms);
}
export function readoutNumber(value: unknown) {
  const compact =
    known(value) && Math.abs(value) >= 10000
      ? `${(value / (Math.abs(value) >= 100000000 ? 100000000 : 10000)).toLocaleString("zh-CN", { maximumFractionDigits: 1 })}${Math.abs(value) >= 100000000 ? "亿" : "万"}`
      : number(value);
  return (
    <>
      <span
        className={cls("lgi-tm-reading-number")}
        title={number(value)}
        aria-label={number(value)}
      >
        {compact}
      </span>
    </>
  );
}
export function platformName(id: string | undefined | null) {
  return { xhs: "小红书", douyin: "抖音" }[id || ""] || id || "平台未知";
}
export function coverageState(w: T.Work | undefined) {
  const c = w?.research?.core?.coverage;
  if (!c) return "unknown";
  const counts = [
      c.sourceChars,
      c.coveredChars,
      c.totalWindows,
      c.completedWindows,
    ],
    valid = counts.every((v) => Number.isInteger(v) && v >= 0);
  return c.state === "complete" &&
    valid &&
    c.coveredChars === c.sourceChars &&
    c.completedWindows === c.totalWindows
    ? "complete"
    : c.state === "partial" || valid
      ? "partial"
      : "unknown";
}
export function discussionUnits(w: T.Work | undefined) {
  if (!w) return [];
  const units = w.research?.core?.units,
    hasCore = Array.isArray(units),
    rows = hasCore ? units : list(w.research?.output?.discussions),
    seen = new Set<string>();
  return rows.flatMap((d, index) => {
    if (!d || typeof d !== "object") return [];
    const unitKey =
      hasCore && typeof d.unitId === "string" && d.unitId
        ? d.unitId
        : JSON.stringify([w.workRef, "legacy", index]);
    if (seen.has(unitKey)) return [];
    seen.add(unitKey);
    const candidates = hasCore
        ? ["matched", "new"].includes(d.status || "")
          ? list(d.assignments)
          : []
        : d.topicRef
          ? [
              {
                topicRef: d.topicRef,
                definitionRef: d.definitionRef,
                label: d.label,
              },
            ]
          : [],
      assigned = new Set();
    const assignments = candidates.filter((a) => {
      if (!a || typeof a !== "object") return false;
      const key = assignmentKey(a);
      if (!key || assigned.has(key)) return false;
      assigned.add(key);
      return true;
    });
    return [
      {
        ...d,
        unitKey,
        assignments,
        legacy: !hasCore,
        evidence: list(d.evidence),
      },
    ];
  });
}
export function assignmentKey(a: T.Assignment | null | undefined) {
  return typeof a?.topicRef === "string" && a.topicRef
    ? `topic:${a.topicRef}`
    : typeof a?.definitionRef === "string" && a.definitionRef
      ? `definition:${a.definitionRef}`
      : "";
}
export function discussionKeys(w: T.Work) {
  return new Set(
    discussionUnits(w).flatMap((d) => d.assignments.map(assignmentKey)),
  );
}
export function sharesDiscussion(a: T.Work, b: T.Work) {
  const keys = discussionKeys(a);
  return [...discussionKeys(b)].some((key) => keys.has(key));
}
export function locatedCitations(
  w: T.Work | undefined,
  citations: T.Citation[] | undefined,
) {
  const fragments = new Map(
      list(w?.research?.fragments).map((f) => [f.fragmentId, f]),
    ),
    seen = new Set();
  return list(citations).flatMap((c) => {
    if (!c || typeof c !== "object") return [];
    const f = fragments.get(c.fragmentId),
      chars = Array.from(f?.text || ""),
      base =
        typeof f?.start === "number" &&
        typeof f.start === "number" &&
        Number.isInteger(f.start)
          ? f.start
          : 0;
    if (
      !f ||
      !Number.isInteger(c.start) ||
      !Number.isInteger(c.end) ||
      c.start < base ||
      c.end <= c.start ||
      c.end > base + chars.length
    )
      return [];
    const key = JSON.stringify([c.fragmentId, c.start, c.end]);
    if (seen.has(key)) return [];
    seen.add(key);
    return [
      { ...c, fragment: f, localStart: c.start - base, localEnd: c.end - base },
    ];
  });
}
export function citationText(w: T.Work, citations: T.Citation[] | undefined) {
  const c = locatedCitations(w, citations)[0];
  return c
    ? Array.from(c.fragment.text).slice(c.localStart, c.localEnd).join("")
    : "";
}
export function unreadResearchWork(w: T.Work) {
  return {
    ...w,
    research: null,
    annotation: null,
    evidenceFragment: null,
    mainStage: null,
    involvedStages: [],
    overlays: [],
  };
}
export function researchQualificationUrl(w: T.Work, domainRef: string) {
  return `/api/local/topic-map/evidence?${new URLSearchParams({ domainRef, workRef: w.workRef })}`;
}
export function applyQualifiedReader(
  reader: T.Reader,
  values: readonly [
    PromiseSettledResult<T.Resource>,
    PromiseSettledResult<T.Comments>,
    PromiseSettledResult<T.Evidence>,
  ],
) {
  reader.resource = values[0].status === "fulfilled" ? values[0].value : null;
  reader.resourceError =
    values[0].status === "rejected" ? caught(values[0].reason).message : "";
  reader.commentResource =
    values[1].status === "fulfilled" ? values[1].value : null;
  reader.commentError =
    values[1].status === "rejected" ? caught(values[1].reason).message : "";
  const qualified =
    values[2].status === "fulfilled"
      ? list(values[2].value?.works).find(
          (w) => w.workRef === reader.work.workRef,
        )
      : null;
  reader.qualificationError =
    values[2].status === "rejected"
      ? caught(values[2].reason).message
      : !qualified
        ? "当前范围未返回这篇作品的来源资格，旧研究不继续展示。"
        : "";
  reader.work = qualified
    ? { ...qualified, annotation: null }
    : unreadResearchWork(reader.work);
  if (
    reader.qualificationError ||
    (reader.resourceError && !commentOnlyResearch(reader.work))
  )
    reader.work = unreadResearchWork(reader.work);
  reader.loading = false;
}
export function fragmentText(
  fragment: { text: string } | string | null | undefined,
) {
  return typeof fragment === "string" ? fragment : fragment?.text || "";
}
export function sourceKind(kind: string | undefined) {
  return (
    Object.assign({
      detail_body: "作者正文",
      ocr_text: "图片OCR",
      image_substantive_text: "图片原文",
      frame_ocr_text: "视频帧OCR",
      asr_text: "音视频转写",
      ocr: "图片OCR",
      transcript: "音视频转写",
      title: "作品标题",
      comment_body: "评论原声",
      comment: "评论原声",
      clean_comment: "评论原声",
      unresearched_comment: "评论原声",
      studied_comment: "评论原声",
      parent_comment_context: "父评论上下文",
      body: "作者正文",
    } as Record<string, string>)[kind || ""] || "来源片段"
  );
}
export function highlighted(text: string, spans: T.Span[] = []) {
  const chars = Array.from(text || ""),
    valid = list(spans)
      .filter(
        (s) =>
          Number.isInteger(s.start) &&
          Number.isInteger(s.end) &&
          s.start >= 0 &&
          s.end > s.start &&
          s.end <= chars.length,
      )
      .sort((a, b) => a.start - b.start);
  const merged: T.Span[] = [];
  valid.forEach((s) => {
    const previous = merged[merged.length - 1];
    if (previous && s.start <= previous.end)
      previous.end = Math.max(previous.end, s.end);
    else merged.push({ start: s.start, end: s.end });
  });
  let position = 0;
  const out: ReactNode[] = [];
  merged.forEach((s) => {
    out.push(
      <>
        {esc(chars.slice(position, s.start).join(""))}
        {
          <>
            <mark>{esc(chars.slice(s.start, s.end).join(""))}</mark>
          </>
        }
      </>,
    );
    position = s.end;
  });
  return (
    <>
      {out}
      {esc(chars.slice(position).join(""))}
    </>
  );
}
export function fragmentWorkRef(w: T.Work, f: T.Fragment) {
  return Object.prototype.hasOwnProperty.call(f, "workRef")
    ? typeof f.workRef === "string" && f.workRef
      ? f.workRef
      : null
    : w.workRef;
}
export function readerFragmentInBody(w: T.Work, f: T.Fragment, raw: string) {
  if (
    fragmentWorkRef(w, f) !== w.workRef ||
    f.field !== "body" ||
    !String(f.fragmentId).startsWith(`${w.workRef}.body.`) ||
    !raw
  )
    return false;
  const base =
      typeof f.start === "number" && Number.isInteger(f.start) ? f.start : 0,
    text = fragmentText(f);
  return (
    Boolean(text) &&
    Array.from(raw)
      .slice(base, base + Array.from(text).length)
      .join("") === text
  );
}
export function readerTitleFragment(w: T.Work, f: T.Fragment) {
  return (
    fragmentWorkRef(w, f) === w.workRef &&
    f.field === "title" &&
    String(f.fragmentId).startsWith(`${w.workRef}.title.`) &&
    fragmentText(f) === title(w)
  );
}
export function analysisOutputs(ns: T.Work[]) {
  return ns.map((w) => ({ work: w, output: w.research?.output || {} }));
}
export function evidenceWorks(item: T.ResearchItem, workRef: string) {
  return Array.isArray(item.evidenceWorkRefs)
    ? [
        ...new Set(
          item.evidenceWorkRefs.filter((ref) => typeof ref === "string" && ref),
        ),
      ]
    : [workRef];
}
export function researchItems(
  outputs: T.ResearchOutputs,
  key: "angles" | "productOpportunities",
  indexField: "researchAngleIndex" | "researchOpportunityIndex",
) {
  const seen = new Set();
  return outputs
    .flatMap(({ work, output }) =>
      list(output[key]).map((item, index) => ({
        ...item,
        workRef: work.workRef,
        evidenceWorkRefs: evidenceWorks(item, work.workRef),
        researchResultRef: item.researchResultRef || work.research?.resultRef,
        researchMethodVersion:
          item.researchMethodVersion || work.research?.methodVersion,
        [indexField]: Number.isInteger(item[indexField])
          ? item[indexField]
          : index,
        limitations: list(output.limitations),
      })),
    )
    .filter((item) => {
      if (typeof item.researchResultRef !== "string" || !item.researchResultRef)
        return false;
      const identity = JSON.stringify([
        item.researchResultRef,
        item[indexField],
      ]);
      if (seen.has(identity)) return false;
      seen.add(identity);
      return true;
    });
}
export function markdownFence(text: unknown) {
  const longest = Math.max(
    2,
    ...(String(text).match(/`+/g) || []).map((value) => value.length),
  );
  const fence = "`".repeat(longest + 1);
  return `${fence}\n${text}\n${fence}`;
}
export function allCitations(w: T.Work) {
  const o = w.research?.output || {};
  return [
    ...discussionUnits(w).flatMap((d) => d.evidence),
    ...list(o.journey?.evidence),
    ...(
      ["scenes", "responseMatches", "angles", "productOpportunities"] as const
    ).flatMap((key) => list(o[key]).flatMap((v) => list(v.evidence))),
  ];
}
export function citationSpans(w: T.Work, fragmentId: string) {
  return locatedCitations(w, allCitations(w))
    .filter((c) => c.fragmentId === fragmentId)
    .map((c) => ({ start: c.localStart, end: c.localEnd }));
}
export function bodyCitations(w: T.Work, raw: string) {
  return locatedCitations(w, allCitations(w))
    .filter((c) => readerFragmentInBody(w, c.fragment, raw))
    .map((c) => ({ start: c.start, end: c.end }));
}
