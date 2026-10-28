import { TopicResearchFlow } from './TopicResearchFlow';
import { TopicButton } from "./TopicButton";
import type { JSX } from "react";
import { attributes } from "./elementProps";
import {
  comparisonScopeNote,
  materialSourceNote,
  performancePair,
  researchCoverageSummary,
} from "./sourceViews";
import { c as cls } from "./styles";
import { badge, btn, empty, tab, withEmpty } from "./topicElements";
import {
  analysisOutputs,
  bodyCitations,
  citationSpans,
  commentOnlyResearch,
  esc,
  fallbackStages,
  fragmentText,
  highlighted,
  known,
  labelState,
  list,
  markdownFence,
  number,
  otherNames,
  pct,
  platformName,
  platformStats,
  publicationOrder,
  publishedTime,
  readableMaterial,
  readerFragmentInBody,
  readerTitleFragment,
  readLabel,
  researchItems,
  sourceKind,
  stageNames,
  title,
} from "./topicModel";
import type * as T from "./types";

/** Read-only dialog projection inputs; commands stay in the controller. */
export interface TopicDialogsContext {
  readonly requireDialog: () => T.DialogState;
  readonly requireSnapshot: () => T.Snapshot;
  readonly matchesWork: (w: T.Work, queryText?: string) => boolean;
  readonly card: (w: T.Work) => JSX.Element;
  readonly avatar: (name: string | null | undefined) => JSX.Element;
  readonly fragmentSourceLabel: (w: T.Work, f: T.Fragment) => string;
  readonly state: Readonly<T.ViewState>;
  readonly readerDiscussions: (w: T.Work, raw: string) => JSX.Element | "";
  readonly topic: (ref: string | null | undefined) => T.Topic | undefined;
  readonly curated: (
    ref: string | undefined,
    source?: T.Snapshot | null,
  ) => T.Work[];
  readonly ownTopicWorks: (
    ref: string | undefined,
    source?: T.Snapshot | null,
  ) => T.Work[];
  readonly topicBoundary: (
    t: T.Topic | undefined | null,
    source?: T.Snapshot | null,
  ) => JSX.Element | "";
  readonly commentFragmentCount: (ns: T.Work[]) => number;
  readonly discussionGroups: (
    ns: T.Work[],
    source?: T.Snapshot | null,
  ) => {
    works: T.Work[];
    evidenceCount: number;
    key: string;
    topicRef: string;
    label: string;
    canonical: boolean;
    rows: T.DiscussionRow[];
    seen: Set<string>;
  }[];
  readonly discussionGroupDetails: (g: T.DiscussionGroup) => JSX.Element;
  readonly sceneList: (
    ns: T.Work[],
    queryText?: string,
    limit?: number,
  ) => JSX.Element;
  readonly dialogState: T.DialogState | null;
  readonly replacementCandidates: (parent: T.DialogState) => T.Work[];
  readonly scopeText: () => string;
  readonly journeyEntries: (
    journey: T.Journey | undefined | null,
    mode?: string,
  ) => T.JourneyEntry[];
  readonly pending: boolean;
  readonly observedPlatforms: (
    stats?: T.Statistics | undefined,
  ) => T.PlatformStatistics[];
  readonly work: (ref: string | undefined | null) => T.Work | undefined;
  readonly selectedWorks: () => T.Work[];
  readonly alternativeAvailable: (
    value: T.Alternative | undefined,
  ) => value is T.ReadableAlternative;
  readonly topicName: (ref: string) => string;
}
export function createTopicDialogs(context: TopicDialogsContext) {
  function materialsDialog() {
    const activeDialog = context.requireDialog();
    const source = activeDialog.snapshot || context.requireSnapshot();
    let ns = list(source.works);
    const refs = activeDialog.refs;
    if (refs) ns = ns.filter((w) => refs.includes(w.workRef));
    if (activeDialog.unassigned)
      ns = ns.filter((w) => !list(w.topicRefs).length);
    const q = activeDialog.query || "",
      sort = activeDialog.sort || "latest";
    ns = ns.filter((w) => context.matchesWork(w, q));
    if (sort !== "latest")
      ns.sort(
        (a, b) =>
          a.platform.localeCompare(b.platform) ||
          (b[sort] ?? -1) - (a[sort] ?? -1),
      );
    else ns.sort((a, b) => publicationOrder(b) - publicationOrder(a));
    const limit = activeDialog.limit || 60;
    return (
      <>
        <p>
          {[
            esc(activeDialog.label || "当前范围的全部作品"),
            " · ",
            ns.length,
            " 篇已读取作品。原始材料不受精选10篇上限限制。",
          ].join("")}
        </p>
        <div className={cls("lgi-tm-toolbar")}>
          <label className={cls("lgi-tm-control")}>
            <span>{"材料排序 / 同平台分组"}</span>
            <select data-dialog-filter={"sort"}>
              {[
                ["latest", "最新发布时间"],
                ["likes", "点赞"],
                ["collects", "收藏"],
                ["comments", "评论"],
                ["shares", "分享"],
              ].map(([v, t]) => (
                <>
                  <option
                    value={v}
                    {...attributes(sort === v ? "selected" : "")}
                  >
                    {t}
                  </option>
                </>
              ))}
            </select>
          </label>
          <form data-form={"material-search"} className={cls("lgi-tm-row")}>
            <input
              name={"query"}
              defaultValue={esc(q)}
              placeholder={"查找标题 / 作者 / 片段 / 讨论"}
              aria-label={"查找材料"}
            />
            <TopicButton type={"submit"} className={cls("lgi-tm-button")}>
              {"查找"}
            </TopicButton>
          </form>
        </div>
        <div className={cls("lgi-tm-material-grid")}>
          {ns.slice(0, limit).map(context.card)}
        </div>
        {ns.length > limit
          ? btn(`展开其余 ${ns.length - limit} 篇`, "materials-more")
          : ""}
        {ns.length ? "" : empty("当前条件没有匹配的已读取作品")}
        <p className={cls("lgi-tm-note")}>
          {"未知数字显示—；跨平台互动不混合作统一排名。"}
        </p>
      </>
    );
  }
  function readerDialog(reader: T.Reader) {
    const w = reader.work,
      r = reader.resource;
    const item = r?.item,
      research =
        !reader.loading &&
        !reader.qualificationError &&
        (!reader.resourceError || commentOnlyResearch(w))
          ? w.research
          : null,
      body = item?.inspector?.detailCurrent?.body?.value;
    const raw =
      w.readable === false ? "" : body || fragmentText(item?.evidenceFragment);
    const fragments = list(research?.fragments).filter(
      (f) => !(readerFragmentInBody(w, f, raw) || readerTitleFragment(w, f)),
    );
    const comments = list(reader.commentResource?.items);
    const source = "";
    const analysis = research?.output || {},
      qualifiedWork = { ...w, research };
    return (
      <>
        <div className={cls("lgi-tm-reader-head")}>
          <div>
            <h3 id={"lgi-tm-source-title-" + String(esc(w.workRef) ?? "")}>
              {esc(title(w))}
            </h3>
            {materialSourceNote(w)}
            <p>
              {context.avatar(w.creatorDisplayName)}
              {[
                esc(w.creatorDisplayName || "作者未知"),
                " · ",
                esc(platformName(w.platform)),
                " · ",
                esc(publishedTime(w)),
              ].join("")}
            </p>
            <div className={cls("lgi-tm-row")}>
              {badge(
                w.titleSource === "cover_ocr" ? "标题来自封面OCR" : "作品标题",
              )}
              {!reader.loading && !reader.resourceError && w.readable
                ? btn("人工保留备选", "manual-save", w.workRef)
                : ""}
              {w.own
                ? btn(
                    w.ownBreakout ? "取消我方爆款标记" : "标记我方爆款",
                    "breakout",
                    w.workRef,
                    "",
                    "data-mutation",
                  )
                : ""}
              {source ? (
                <>
                  <a
                    className={cls("lgi-tm-button")}
                    href={esc(source)}
                    target={"_blank"}
                    rel={"noopener noreferrer"}
                  >
                    {"打开来源页面"}
                  </a>
                </>
              ) : (
                ""
              )}
            </div>
          </div>
          <div className={cls("lgi-tm-interactions")}>
            {[
              "赞 ",
              number(w.likes),
              " · 藏 ",
              number(w.collects),
              " · 评 ",
              number(w.comments),
              " · 分享 ",
              number(w.shares),
            ].join("")}
          </div>
        </div>
        <div className={cls("lgi-tm-reader-layout")}>
          <section>
            <h4>{"原文与可定位片段"}</h4>
            {reader.loading ? (
              <>
                <p role={"status"}>
                  {
                    "正在复核原文、评论与研究来源资格；上次结果的片段和解释暂不展示。"
                  }
                </p>
              </>
            ) : (
              ""
            )}
            {reader.resourceError ? (
              <>
                <p className={cls("lgi-tm-read-error")}>
                  {["原文读取未成功：", esc(reader.resourceError)].join("")}
                </p>
              </>
            ) : (
              ""
            )}
            {raw ? (
              <>
                <div
                  className={cls("lgi-tm-original")}
                  id={"lgi-tm-source-body-" + String(esc(w.workRef) ?? "")}
                >
                  {highlighted(raw, bodyCitations(qualifiedWork, raw))}
                </div>
                {!body ? (
                  <>
                    <p className={cls("lgi-tm-note")}>
                      {
                        "当前为有界来源片段；完整原文读取与来源资格以材料检查器为准。"
                      }
                    </p>
                  </>
                ) : (
                  ""
                )}
              </>
            ) : (
              empty("当前没有可读取的正文片段", "缺少正文不等于原作没有回应。")
            )}
            {fragments.map((f) => (
              <>
                <article
                  className={cls("lgi-tm-fragment")}
                  id={
                    "lgi-tm-source-fragment-" + String(esc(f.fragmentId) ?? "")
                  }
                >
                  <span>{esc(context.fragmentSourceLabel(w, f))}</span>
                  <blockquote>
                    {highlighted(
                      fragmentText(f),
                      citationSpans(qualifiedWork, f.fragmentId),
                    )}
                  </blockquote>
                </article>
              </>
            ))}
            <a
              className={cls("lgi-tm-button")}
              href={
                "/corpus/evidence?work=" +
                String(encodeURIComponent(w.workRef) ?? "") +
                "&domain=" +
                String(encodeURIComponent(context.state.domainRef) ?? "")
              }
            >
              {"在材料检查器定位"}
            </a>
          </section>
          <aside className={cls("lgi-tm-analysis")}>
            {context.readerDiscussions(qualifiedWork, raw)}
            <h4>{"模型解释 / 已保存版本"}</h4>
            {reader.qualificationError ? (
              <>
                <p className={cls("lgi-tm-read-error")}>
                  {[
                    "研究来源资格复核未完成：",
                    esc(reader.qualificationError),
                  ].join("")}
                </p>
              </>
            ) : (
              ""
            )}
            {analysis.journey?.rationale ? (
              <>
                <p>{esc(analysis.journey.rationale)}</p>
              </>
            ) : (
              <>
                <p>
                  {
                    "当前没有通过来源资格检查的可引用研究解释，不用旧摘要代替原声。"
                  }
                </p>
              </>
            )}
            <p>
              {[
                esc(
                  stageNames[analysis.journey?.mainStage || ""] ||
                    otherNames[analysis.journey?.mainStage || ""] ||
                    "阶段解释未取得",
                ),
                " · ",
                esc(
                  list(analysis.journey?.overlays)
                    .map((v) =>
                      v === "obstruction_recurrence"
                        ? "受阻与反复"
                        : "环境转换与交接",
                    )
                    .join("、"),
                ),
              ].join("")}
            </p>
            {list(analysis.limitations).map((t) => (
              <>
                <p className={cls("lgi-tm-note")}>{esc(t)}</p>
              </>
            ))}
            <h4>{"已有回应关系"}</h4>
            {withEmpty(
              list(analysis.responseMatches).map((m) => (
                <>
                  <p>
                    {esc(
                      Object.assign({
                        direct: "有直接回应依据",
                        partial: "部分回应",
                        unmatched: "未找到匹配回应",
                        unknown: "回应未知",
                        not_applicable: "当前不适用",
                      } as Record<string, string>)[m.status || ""] || m.status,
                    )}
                  </p>
                  {comparisonScopeNote(m)}
                </>
              )),
              <>
                <p>{"回应尚未研究，不能把评论问题自动当原作回答。"}</p>
              </>,
            )}
          </aside>
        </div>
        <section className={cls("lgi-tm-section")}>
          <div className={cls("lgi-tm-section-head")}>
            <div>
              <h3>{"已有评论原声"}</h3>
              <p>{"评论条数不等于人数，作者观点与评论分别保留来源。"}</p>
            </div>
            {btn(
              "深入看这篇的讨论",
              "deep-comments",
              w.workRef,
              "",
              "data-mutation",
            )}
          </div>
          {reader.commentError ? (
            <>
              <p className={cls("lgi-tm-read-error")}>
                {["评论读取未成功：", esc(reader.commentError)].join("")}
              </p>
            </>
          ) : (
            ""
          )}
          <p className={cls("lgi-tm-note")}>
            {[
              "当前展示 ",
              comments.length,
              " / ",
              number(reader.commentResource?.total),
              " 条当前可读评论。",
            ].join("")}
          </p>
          {comments.length
            ? comments.map((c) => (
                <>
                  <article className={cls("lgi-tm-comment")}>
                    <blockquote>{esc(c.body || "正文未知")}</blockquote>
                    <p>
                      {[
                        esc("评论作者身份已隐去"),
                        " · ",
                        esc(c.sourceRef || ""),
                      ].join("")}
                    </p>
                    {c.parentText ? (
                      <>
                        <details>
                          <summary>{"父评论上下文"}</summary>
                          <blockquote>{esc(c.parentText)}</blockquote>
                        </details>
                      </>
                    ) : (
                      ""
                    )}
                  </article>
                </>
              ))
            : empty(
                reader.loading ? "评论正在读取" : "当前没有可展示的已有评论",
                "不会为了填满原声区域发起采集。",
              )}
          {reader.commentResource?.nextCursor
            ? btn("读取下一页已有评论", "comments-more")
            : ""}
        </section>
      </>
    );
  }
  function judgmentDialog() {
    const activeDialog = context.requireDialog();
    const ref = activeDialog.topicRef,
      source = activeDialog.snapshot || context.requireSnapshot(),
      t =
        list(source.topics).find((t) => t.topicRef === ref) ||
        context.topic(ref),
      ns = activeDialog.works || context.curated(ref),
      allOwn = context.ownTopicWorks(ref, source),
      mode = activeDialog.tab || "discussion";
    const external = list(source.works).filter(
      (w) =>
        !w.own &&
        list(source.scope.recentReferenceWorkRefs).includes(w.workRef),
    );
    const tabs = [
      ["discussion", "方向与讨论"],
      ["samples", `精选样本 · ${ns.length}/10`],
      ["own", "我方对照与复用"],
      ["angles", "角度与标题"],
    ];
    return (
      <>
        <div className={cls("lgi-tm-object-head")}>
          <div>
            <span className={cls("lgi-tm-eyebrow")}>
              {["方向判断 · 定义 v", number(t?.definitionVersion)].join("")}
            </span>
            <h3>{esc(t?.displayName || "当前方向")}</h3>
            <p>{esc(t?.definitionText)}</p>
            <p>
              {[
                "我方全部发布历史 / 外部",
                context.state.referenceWindowDays === "0"
                  ? "全部历史"
                  : "近" + context.state.referenceWindowDays + "天发布",
                "参考；独立于概览统计窗口。",
              ].join("")}
            </p>
          </div>
          {btn("查看全部材料", "materials")}
        </div>
        {context.topicBoundary(t, source)}
        {activeDialog.loading ? (
          <>
            <p role={"status"}>
              {"正在补齐全历史我方作品与独立外部参考范围，已有材料先显示。"}
            </p>
          </>
        ) : (
          ""
        )}
        {activeDialog.readError ? (
          <>
            <p className={cls("lgi-tm-read-error")}>
              {["完整范围读取未成功：", esc(activeDialog.readError)].join("")}
            </p>
          </>
        ) : (
          ""
        )}
        <div className={cls("lgi-tm-judge-facts")}>
          <div>
            <strong>{list(source.works).length}</strong>
            <span>{"篇当前相关材料"}</span>
          </div>
          <div>
            <strong>{allOwn.length}</strong>
            <span>{"篇已确认我方发布"}</span>
          </div>
          <div>
            <strong>
              {
                external.filter(
                  (w) =>
                    known(w.followerCount) &&
                    w.followerCount <= 1000 &&
                    known(w.likes) &&
                    w.likes >= 500,
                ).length
              }
            </strong>
            <span>{"篇近期低粉高赞参考"}</span>
          </div>
        </div>
        <nav className={cls("lgi-tm-work-tabs")} aria-label={"选题判断内容"}>
          {tabs.map(([id, name]) => tab(name, "judge-tab", id, mode === id))}
        </nav>
        <div className={cls("lgi-tm-judgment-grid")}>
          <div className={cls("lgi-tm-judgment-main")}>
            {mode === "samples"
              ? sampleView(ns)
              : mode === "own"
                ? ownView(ref, allOwn, ns)
                : mode === "angles"
                  ? anglesView(ns)
                  : discussionsView(ns)}
          </div>
          <aside className={cls("lgi-tm-judgment-rail")}>
            <section className={cls("lgi-tm-rail-box")}>
              <h3>{"这一轮，先判断什么？"}</h3>
              <p>
                {
                  "从同题高表现中找借鉴，用同讨论对照检查条件。先看原文，再决定角度。"
                }
              </p>
              <div className={cls("lgi-tm-rail-stats")}>
                <div>
                  <b>{ns.length}</b>
                  <span>{"精选作品 / 最多10"}</span>
                </div>
                <div>
                  <b>{context.commentFragmentCount(ns)}</b>
                  <span>{"可定位的评论来源片段"}</span>
                </div>
              </div>
              {btn(
                mode === "angles" ? "回到材料对照" : "找角度与标题",
                "judge-tab",
                mode === "angles" ? "samples" : "angles",
                "lgi-tm-primary",
              )}
              {btn("查看全部相关材料", "materials", "", "lgi-tm-quiet")}
            </section>
            <section className={cls("lgi-tm-rail-box")}>
              <h3>{"依据与资料补充"}</h3>
              <div className={cls("lgi-tm-rail-links")}>
                {btn("高表现参考来自谁", "judge-sources")}
                {btn("本次补充与真实进度", "capture", ref)}
                {btn("放回旅程地图", "judge-to-journey", ref)}
                {btn("查看有限产品机会", "judge-product", ref)}
              </div>
              <p>
                {"保存只保留方向与依据，不新建监控。反例和材料不足都继续保留。"}
              </p>
            </section>
            <section className={cls("lgi-tm-rail-box")}>
              <h3>{"继续研究"}</h3>
              <p>{"选择作品后开始四阶段研究，方法与额度在 AI 设置管理。"}</p>

              {btn("主题研究与阶段回执", "research", "", "lgi-tm-quiet")}
            </section>
          </aside>
        </div>
      </>
    );
  }
  function discussionsView(ns: T.Work[]) {
    const groups = context.discussionGroups(ns);
    return (
      <>
        <div className={cls("lgi-tm-section-head")}>
          <div>
            <h3>{"同一主题，先分清几种具体讨论"}</h3>
            <p>
              {
                "按已保存的主题身份归组；同名但边界不同的讨论分别保留。支持、反例和背景可以属于同一主题。"
              }
            </p>
            {researchCoverageSummary(ns)}
          </div>
        </div>
        {withEmpty(
          groups.map((g, i) => (
            <>
              <article className={cls("lgi-tm-discussion")}>
                <div className={cls("lgi-tm-discussion-head")}>
                  <span className={cls("lgi-tm-discussion-index")}>
                    {String(i + 1).padStart(2, "0")}
                  </span>
                  <div>
                    <h3>{esc(g.label)}</h3>
                    <p>
                      {[
                        g.rows.length,
                        " 条独立讨论 · ",
                        g.works.length,
                        " 篇精选作品 · ",
                        g.evidenceCount,
                        " 条去重定位依据",
                      ].join("")}
                    </p>
                    {g.canonical ? (
                      ""
                    ) : (
                      <>
                        <p className={cls("lgi-tm-note")}>
                          {"主题身份尚未确认，不与其他同名材料自动合组。"}
                        </p>
                      </>
                    )}
                  </div>
                </div>
                {context.discussionGroupDetails(g)}
              </article>
            </>
          )),
          empty("具体讨论关系尚待研究", "原作可以先读，不用等分析完成。"),
        )}
        <section className={cls("lgi-tm-section")}>
          <h3>{"高低表现：先看差异，不先下归因"}</h3>
          {performancePair(ns)}
          <p className={cls("lgi-tm-note")}>
            {
              "同题不代表曝光、作者条件与发布时间可比。这里是材料对照，不是写法导致爆款的结论。"
            }
          </p>
        </section>
        <section className={cls("lgi-tm-section")}>
          <h3>{"实际场景与来源"}</h3>
          {context.sceneList(ns)}
        </section>
      </>
    );
  }
  function anglesFrom(ns: T.Work[]) {
    return researchItems(analysisOutputs(ns), "angles", "researchAngleIndex");
  }
  function anglesView(ns: T.Work[]) {
    const angles = anglesFrom(ns);
    return (
      <>
        <div className={cls("lgi-tm-section-head")}>
          <div>
            <h3>{"先选不同角度，再打磨标题"}</h3>
            <p>
              {
                "角度与回答任务来自当前已保存的研究版本，差异与回答边界需要继续核对原作。"
              }
            </p>
          </div>
        </div>
        {angles.length
          ? angles.slice(0, 6).map((a, i) => (
              <>
                <article className={cls("lgi-tm-angle")}>
                  {badge(`${String(i + 1).padStart(2, "0")} · ${a.label}`)}
                  <h3>{esc(a.title)}</h3>
                  <p>{["这篇要回答：", esc(a.answerTask)].join("")}</p>
                  {comparisonScopeNote(a)}
                  <p className={cls("lgi-tm-note")}>
                    {"与我方已有内容的差别尚待核对，不由标题相似推断覆盖。"}
                  </p>
                  {a.limitations.map((value) => (
                    <>
                      <p className={cls("lgi-tm-note")}>
                        {["研究限制：", esc(value)].join("")}
                      </p>
                    </>
                  ))}
                  <div className={cls("lgi-tm-row")}>
                    {btn("核对原作与引用", "reader", a.workRef)}
                    {btn(
                      "选这个角度，打磨标题",
                      "save-angle",
                      String(i),
                      "lgi-tm-primary",
                    )}
                  </div>
                </article>
              </>
            ))
          : empty(
              "当前没有已研究、可引用的不同角度",
              "已有作品可继续读；可选择作品发起主题研究。",
            )}
      </>
    );
  }
  function ownView(ref: string | undefined, own: T.Work[], ns: T.Work[]) {
    const activeSnapshot = context.requireSnapshot();
    if (activeSnapshot.scope.ownIdentityState === "unknown")
      return (
        <>
          {empty("我方矩阵未设置", "不能推断全部主题都没有做过。")}
          {btn("设置我方矩阵", "identity")}
        </>
      );
    return (
      <>
        <div className={cls("lgi-tm-section-head")}>
          <div>
            <h3>{"我方原来讲了什么，外部还有什么？"}</h3>
            <p>
              {"整个已确认矩阵的发布历史；标题相近不等于具体问题已经覆盖。"}
            </p>
          </div>
          {btn("展开全部我方原作", "judge-own-all")}
        </div>
        {withEmpty(
          own.map((w) => (
            <>
              <article className={cls("lgi-tm-own-work")}>
                {badge(
                  w.ownBreakout ? "我方手动爆款" : "我方已发布",
                  w.ownBreakout,
                )}
                <h3>{esc(title(w))}</h3>
                <p>
                  {esc(fragmentText(w.evidenceFragment)) ||
                    "当前没有安全可展示的原文片段，请打开原作核对。"}
                </p>
                <p className={cls("lgi-tm-note")}>
                  {[
                    esc(w.creatorDisplayName || "作者未知"),
                    " · ",
                    esc(w.publishedAtSourceText || w.publishedAt),
                    " · ",
                    number(w.likes),
                    "赞",
                  ].join("")}
                </p>
                <div className={cls("lgi-tm-row")}>
                  {btn(readLabel(w, "看原题、封面与正文"), "reader", w.workRef)}
                  {btn(
                    "保留复用意图",
                    "own-reuse",
                    w.workRef,
                    "lgi-tm-quiet",
                    readableMaterial(w)
                      ? ""
                      : 'disabled title="当前没有可引用来源"',
                  )}
                </div>
              </article>
            </>
          )),
          empty(
            "当前已入库发布历史中尚未观察到本主题我方原作",
            "部分覆盖不代表全矩阵从未做过。",
          ),
        )}
        <section className={cls("lgi-tm-section")}>
          <h3>{"相对我方，继续检查哪些差别？"}</h3>
          <p>
            {
              "按同一主题身份下的讨论并列查看。是否回答了不同问题、遗漏哪些条件，由原作和实际回应共同核对；同名未知归属分别保留。"
            }
          </p>
          {withEmpty(
            context.discussionGroups(ns).map((g) => {
              const mine = g.works.filter((w) => w.own),
                others = g.works.filter((w) => !w.own);
              return (
                <>
                  <article className={cls("lgi-tm-panel")}>
                    <h4>{esc(g.label)}</h4>
                    <p className={cls("lgi-tm-note")}>
                      {[
                        g.rows.length,
                        " 条独立讨论 · ",
                        g.evidenceCount,
                        " 条去重定位依据",
                      ].join("")}
                    </p>
                    <div className={cls("lgi-tm-own-comparison")}>
                      <div>
                        <h5>{"我方原作"}</h5>
                        {mine.map((w) =>
                          btn(
                            esc(title(w)),
                            "reader",
                            w.workRef,
                            "lgi-tm-quiet",
                          ),
                        ) || (
                          <>
                            <p>{"当前精选无对应原作。"}</p>
                          </>
                        )}
                      </div>
                      <div>
                        <h5>{"外部参考"}</h5>
                        {others.map((w) =>
                          btn(
                            esc(title(w)),
                            "reader",
                            w.workRef,
                            "lgi-tm-quiet",
                          ),
                        ) || (
                          <>
                            <p>{"当前精选无对应外部参考。"}</p>
                          </>
                        )}
                      </div>
                    </div>
                    {context.discussionGroupDetails(g)}
                  </article>
                </>
              );
            }),
            empty(
              "当前尚无可引用的讨论差异分组",
              "可先核对原文与已有评论，不补写差异结论。",
            ),
          )}
        </section>
      </>
    );
  }
  function sampleReason(w: T.Work) {
    const activeSnapshot = context.requireSnapshot();
    if (w.own) return w.ownBreakout ? "我方手动爆款" : "我方已发布原作";
    if (
      known(w.followerCount) &&
      w.followerCount <= 1000 &&
      known(w.likes) &&
      w.likes >= 500
    )
      return "外部低粉高赞参考";
    const p = platformStats(
      context.dialogState?.snapshot?.statistics || activeSnapshot.statistics,
    ).find((p) => p.platform === w.platform);
    return known(w.likes) &&
      known(p?.highPerformanceThreshold) &&
      w.likes >= p.highPerformanceThreshold
      ? "同平台高表现参考"
      : "同题材料 / 待核对条件";
  }
  function sampleView(ns: T.Work[]) {
    const activeDialog = context.requireDialog();
    const selected =
        ns.find((w) => w.workRef === activeDialog.sampleRef) || ns[0],
      reader = activeDialog.inlineReader;
    return (
      <>
        <div className={cls("lgi-tm-section-head")}>
          <div>
            <h3>{"最多10篇，先读有用的对照"}</h3>
            <p>
              {
                "我方原作也计入精选上限。不为凑数加入无关内容，替换只改变本次阅读集合。"
              }
            </p>
          </div>
          {btn(
            "替换当前精选",
            "replace-samples",
            "",
            "",
            activeDialog.loading ? "disabled" : "",
          )}
        </div>
        {ns.length ? (
          <>
            <div className={cls("lgi-tm-sample-grid")}>
              <aside
                className={cls("lgi-tm-sample-list")}
                aria-label={"精选判断样本"}
              >
                {ns.map((w, i) => (
                  <>
                    <TopicButton
                      type={"button"}
                      className={cls(
                        "lgi-tm-sample-item " +
                          String(
                            w.workRef === selected?.workRef ? "is-active" : "",
                          ),
                      )}
                      data-action={"sample"}
                      data-id={esc(w.workRef)}
                      aria-pressed={w.workRef === selected?.workRef}
                    >
                      <span className={cls("lgi-tm-sample-num")}>
                        {String(i + 1).padStart(2, "0")}
                      </span>
                      <div>
                        <h4>{esc(title(w))}</h4>
                        <small>
                          {[
                            esc(platformName(w.platform)),
                            " · ",
                            number(w.likes),
                            "赞 · ",
                            esc(publishedTime(w)),
                          ].join("")}
                        </small>
                        {badge(sampleReason(w))}
                        {materialSourceNote(w)}
                      </div>
                    </TopicButton>
                  </>
                ))}
              </aside>
              <article className={cls("lgi-tm-inline-reader")}>
                {reader && reader.work.workRef === selected?.workRef
                  ? readerDialog(reader)
                  : empty(
                      "选择左侧作品，读取当前可用材料",
                      "来源资格通过读取接口复核，不用模型摘要冒充正文。",
                    )}
                {selected && !reader
                  ? btn(
                      readLabel(selected, "读取这篇原文"),
                      "sample",
                      selected.workRef,
                    )
                  : ""}
              </article>
            </div>
          </>
        ) : (
          empty(
            "当前没有精选材料",
            "可以查看已入库材料，或显式选择当前有界材料集合。",
          )
        )}
      </>
    );
  }
  function replaceSamplesDialog() {
    const activeDialog = context.requireDialog();
    const parent = activeDialog.judgeState;
    if (!parent) return empty("判断范围已关闭");
    const rows = context.replacementCandidates(parent);
    return (
      <>
        <h3>{"调整当前最多10篇判断材料"}</h3>
        <p>
          {
            "每个独立作品只计一次；我方原作也计入10篇。替换不启动新研究或补采，已开启研究保留原冻结集合。"
          }
        </p>
        <form
          data-form={"replace-samples"}
          className={cls("lgi-tm-replacement-list")}
        >
          {withEmpty(
            rows.map((w) => (
              <>
                <label className={cls("lgi-tm-check-label")}>
                  <input
                    type={"checkbox"}
                    name={"workRefs"}
                    defaultValue={esc(w.workRef)}
                    data-sample-choice=""
                    {...attributes(
                      list(parent.works).some((p) => p.workRef === w.workRef)
                        ? "checked"
                        : "",
                    )}
                  />
                  <span>
                    <strong>{esc(title(w))}</strong>
                    <br />
                    {[
                      esc(platformName(w.platform)),
                      " · ",
                      number(w.likes),
                      "赞 · ",
                      w.own ? "我方已发布" : "当前外部参考",
                    ].join("")}
                  </span>
                </label>
              </>
            )),
            empty("当前范围没有可加入的可读作品", "不从其他主题自动填充。"),
          )}
          <TopicButton
            type={"submit"}
            className={cls("lgi-tm-button lgi-tm-primary")}
          >
            {"更新本次精选"}
          </TopicButton>
        </form>
      </>
    );
  }
  function matrixDialog() {
    const activeDialog = context.requireDialog();
    if (activeDialog.loading)
      return (
        <>
          <p role={"status"}>
            {"正在展开全部主题作对照，保留平台、时间、经历路径与覆盖层。"}
          </p>
        </>
      );
    const source = activeDialog.snapshot,
      rowMode = activeDialog.rows || "leaves",
      metric = activeDialog.metric || "main";
    if (!source) return empty("矩阵尚未读取");
    const rows = list(source.topics)
      .filter((t) => t.lifecycleState !== "superseded")
      .filter((t) =>
        rowMode === "roots"
          ? !t.parentTopicRef
          : !source.topics.some((c) => c.parentTopicRef === t.topicRef),
      );
    return (
      <>
        <p>
          {[
            "展开全部主题作对照 · ",
            esc(context.scopeText()),
            " · ",
            metric === "main"
              ? "每格独立作品数 ÷ 本行主题全部可读作品"
              : "每格涉及作品数 ÷ 本行主题全部可读作品，允许重叠",
            context.state.overlay ? " · 已应用跨阶段覆盖筛选" : "",
          ].join("")}
        </p>
        <div className={cls("lgi-tm-toolbar")}>
          <label className={cls("lgi-tm-control")}>
            <span>{"主题行"}</span>
            <select data-dialog-filter={"rows"}>
              <option
                value={"leaves"}
                {...attributes(rowMode === "leaves" ? "selected" : "")}
              >
                {"全部细分主题"}
              </option>
              <option
                value={"roots"}
                {...attributes(rowMode === "roots" ? "selected" : "")}
              >
                {"一级主题"}
              </option>
            </select>
          </label>
          <label className={cls("lgi-tm-control")}>
            <span>{"统计口径"}</span>
            <select data-dialog-filter={"metric"}>
              <option
                value={"main"}
                {...attributes(metric === "main" ? "selected" : "")}
              >
                {"主阶段占比"}
              </option>
              <option
                value={"involved"}
                {...attributes(metric === "involved" ? "selected" : "")}
              >
                {"涉及率"}
              </option>
            </select>
          </label>
        </div>
        <div className={cls("lgi-tm-table-scroll")}>
          <table className={cls("lgi-tm-matrix")}>
            <thead>
              <tr>
                <th>{"主题 / 行分母"}</th>
                {fallbackStages.map((s) => (
                  <>
                    <th>{esc(s.label)}</th>
                  </>
                ))}
                <th>{"其他 / 未归主阶段"}</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((t) => {
                const j = t.journey,
                  entries = context.journeyEntries(j, metric),
                  otherEntries = list(j?.main).filter(
                    (e) => !stageNames[e.stage],
                  );
                const otherCount = otherEntries.reduce(
                  (sum, e) => sum + (known(e.count) ? e.count : 0),
                  0,
                );
                return (
                  <>
                    <tr
                      className={cls(
                        t.topicRef === context.state.topicRef
                          ? "is-active"
                          : "",
                      )}
                    >
                      <th>
                        <strong>{esc(t.displayName)}</strong>
                        <p>
                          {[number(j?.denominator), " 篇可读作品"].join("")}
                        </p>
                      </th>
                      {fallbackStages.map((s) => {
                        const e = entries.find((e) => e.stage === s.stage);
                        return (
                          <>
                            <td>
                              {btn(
                                <>
                                  <b>{number(e?.count)}</b>
                                  <small>{pct(e?.count, j?.denominator)}</small>
                                </>,
                                "matrix-cell",
                                `${t.topicRef}|${s.stage}`,
                                "lgi-tm-matrix-cell",
                                `style="--topic-cell-strength:${known(e?.count) && j?.denominator > 0 ? Math.min(1, e.count / j.denominator) : 0}"`,
                              )}
                            </td>
                          </>
                        );
                      })}
                      <td>
                        {btn(
                          <>
                            <b>{number(otherCount)}</b>
                            <small>{pct(otherCount, j?.denominator)}</small>
                          </>,
                          "matrix-cell",
                          `${t.topicRef}|other`,
                          "lgi-tm-matrix-cell",
                          `style="--topic-cell-strength:${j?.denominator > 0 ? Math.min(1, otherCount / j.denominator) : 0}"`,
                        )}
                        <details>
                          <summary>{"类别明细"}</summary>
                          {otherEntries.map((e) => (
                            <>
                              <p>
                                {[
                                  esc(otherNames[e.stage] || e.stage),
                                  " ",
                                  number(e.count),
                                ].join("")}
                              </p>
                            </>
                          ))}
                        </details>
                      </td>
                    </tr>
                  </>
                );
              })}
            </tbody>
          </table>
        </div>
        {rows.length
          ? ""
          : empty("当前领域尚无可展开的主题行", "已读取材料仍可浏览。")}
        <p className={cls("lgi-tm-note")}>
          {
            "主阶段模式包括五阶段与其他。涉及率可能超过100%，其他仍展示尚未归入五主阶段的材料。空格不表示现实中没有需求。"
          }
        </p>
      </>
    );
  }
  function compareDialog() {
    const activeDialog = context.requireDialog();
    if (activeDialog.loading)
      return (
        <>
          <p role={"status"}>{"正在读取所选方向的同一范围统计。"}</p>
        </>
      );
    const source = activeDialog.snapshot,
      topics = list(source?.topics).filter((t) =>
        list(activeDialog.refs).includes(t.topicRef),
      );
    return (
      <>
        <p>
          {[
            esc(context.scopeText()),
            " · 各平台分别比较，统计引用同一服务端集合。",
          ].join("")}
        </p>
        <div className={cls("lgi-tm-compare-grid")}>
          {topics.map((t) => (
            <>
              <article className={cls("lgi-tm-panel")}>
                <h3>{esc(t.displayName)}</h3>
                <p>{esc(t.definitionText)}</p>
                {context.topicBoundary(t, source)}
                <dl className={cls("lgi-tm-facts")}>
                  <div>
                    <dt>{"去重作品"}</dt>
                    <dd>{number(t.statistics?.workCount)}</dd>
                  </div>
                  <div>
                    <dt>{"已观察作者"}</dt>
                    <dd>{number(t.statistics?.authorCount)}</dd>
                  </div>
                  {platformStats(t.statistics).map((p) => (
                    <>
                      <div>
                        <dt>
                          {[esc(platformName(p.platform)), " 样本爆款"].join(
                            "",
                          )}
                        </dt>
                        <dd>
                          {p.highPerformanceThreshold == null
                            ? "未设置规则"
                            : `${number(p.highPerformanceCount)} / ${number(p.knownLikeCount)} · ${pct(p.highPerformanceCount, p.knownLikeCount)}`}
                        </dd>
                      </div>
                      <div>
                        <dt>
                          {[
                            esc(platformName(p.platform)),
                            " 点赞中位数 / P90",
                          ].join("")}
                        </dt>
                        <dd>
                          {[
                            number(p.medianLikes),
                            " / ",
                            number(p.p90Likes),
                          ].join("")}
                        </dd>
                      </div>
                    </>
                  ))}
                </dl>
                <h4>{"主阶段分布"}</h4>
                {list(t.journey?.main).map((e) => (
                  <>
                    <p>
                      {[
                        esc(
                          stageNames[e.stage] || otherNames[e.stage] || e.stage,
                        ),
                        " ",
                      ].join("")}
                      <span className={cls("lgi-tm-num")}>
                        {[
                          number(e.count),
                          " · ",
                          pct(e.count, t.journey.denominator),
                        ].join("")}
                      </span>
                    </p>
                  </>
                ))}
                <h4>{"我方与外部参考"}</h4>
                <p>
                  {[
                    "已确认我方发布：",
                    number(t.statistics?.ownHistoryPublishedCount),
                    " · 手动爆款：",
                    number(t.statistics?.ownHistoryBreakoutCount),
                  ].join("")}
                </p>
                <p>
                  {[
                    "独立外部近期低粉高赞 ",
                    number(t.statistics?.recentLowFollowerHighLikeCount),
                    " 篇",
                  ].join("")}
                </p>
                {btn("打开判断", "judge", t.topicRef, "lgi-tm-quiet")}
              </article>
            </>
          ))}
        </div>
      </>
    );
  }
  function identityDialog() {
    const activeDialog = context.requireDialog();
    const identitySource = activeDialog.snapshot || context.requireSnapshot();
    const rows = list(identitySource.ownCreators),
      authors = list(identitySource.sources).filter((s) => s.authorExternalId);
    return (
      <>
        <div className={cls("lgi-tm-callout")}>
          {
            "我方身份由你显式指定，仅已确认发布的作品计入覆盖。当前材料仅是已观察范围，不能据此断言整个矩阵没做过。"
          }
        </div>
        <form data-form={"own-creator"} className={cls("lgi-tm-form")}>
          <h3>{"指定我方账号"}</h3>
          <label>
            <span>{"已有观察账号"}</span>
            <select name={"creator"} required={true}>
              <option value={""}>{"请选择账号"}</option>
              {authors.map((a) => (
                <>
                  <option
                    value={
                      String(esc(a.platform) ?? "") +
                      "|" +
                      String(esc(a.authorExternalId) ?? "")
                    }
                  >
                    {[
                      esc(platformName(a.platform)),
                      " · ",
                      esc(a.creatorDisplayName || a.authorExternalId),
                    ].join("")}
                  </option>
                </>
              ))}
            </select>
          </label>
          <p className={cls("lgi-tm-note")}>
            {"只有实际观察且身份已知的账号进入选择；不会自动推断账号归属。"}
          </p>
          <TopicButton
            type={"submit"}
            className={cls("lgi-tm-button lgi-tm-primary")}
            data-mutation=""
            {...attributes(context.pending ? "disabled" : "")}
          >
            {"加入我方矩阵"}
          </TopicButton>
        </form>
        <section className={cls("lgi-tm-section")}>
          <h3>{"当前指定范围"}</h3>
          {withEmpty(
            rows
              .filter((r) => r.active)
              .map((r) => (
                <>
                  <div className={cls("lgi-tm-list-row")}>
                    <span>
                      {[
                        esc(platformName(r.platform)),
                        " · ",
                        esc(
                          authors.find(
                            (a) =>
                              a.platform === r.platform &&
                              a.authorExternalId === r.authorExternalId,
                          )?.creatorDisplayName || r.authorExternalId,
                        ),
                      ].join("")}
                    </span>
                    {btn(
                      "移出我方矩阵",
                      "own-remove",
                      `${r.platform}|${r.authorExternalId}`,
                      "",
                      "data-mutation",
                    )}
                  </div>
                </>
              )),
            empty("尚未指定我方矩阵"),
          )}
          <p className={cls("lgi-tm-note")}>
            {
              "对标来源沿用采集目标的领域用途，不在这里自动建立监控或深度作者档案。"
            }
          </p>
        </section>
      </>
    );
  }
  function rulesDialog() {
    const activeSnapshot = context.requireSnapshot();
    return (
      <>
        <p>
          {
            "样本爆款规则按平台独立保存；未知点赞不进入可判定分母。外部低粉高赞与我方手动标签不随本规则改变。"
          }
        </p>
        {[
          ...new Set([
            "xhs",
            ...context.observedPlatforms().map((p) => p.platform),
          ]),
        ].map((p) => {
          const stat = platformStats(activeSnapshot.statistics).find(
            (s) => s.platform === p,
          );
          return (
            <>
              <form
                data-form={"rule"}
                data-platform={p}
                className={cls("lgi-tm-form")}
              >
                <h3>{platformName(p)}</h3>
                <p>
                  {[
                    "当前规则：",
                    stat?.highPerformanceThreshold == null
                      ? "未设置"
                      : `点赞 ≥ ${number(stat.highPerformanceThreshold)} · 版本 ${esc(stat.highPerformanceRuleVersion)}`,
                  ].join("")}
                </p>
                <label>
                  <span>{"点赞阈值"}</span>
                  <input
                    name={"threshold"}
                    type={"number"}
                    min={"1"}
                    step={"1"}
                    required={true}
                    defaultValue={stat?.highPerformanceThreshold ?? ""}
                    placeholder={"输入明确的正整数阈值"}
                  />
                </label>
                <TopicButton
                  type={"submit"}
                  className={cls("lgi-tm-button")}
                  data-mutation=""
                >
                  {"保存本平台规则"}
                </TopicButton>
              </form>
            </>
          );
        })}
      </>
    );
  }
  function savedDialog() {
    const activeDialog = context.requireDialog();
    if (activeDialog.loading)
      return (
        <>
          <p role={"status"}>{"正在取回后端已保存的备选与来源资格。"}</p>
        </>
      );
    const rows = list(activeDialog.snapshot?.alternatives).filter(
      (a) =>
        !activeDialog.query ||
        [a.title, a.angle, a.rationale].join(" ").includes(activeDialog.query),
    );
    return (
      <>
        <p>
          {
            "人工备选与历史研究说明保留引用版本，默认不跟踪。运营选题池尚未接通；可复核来源并导出交接，保存不代表已采纳或发布。"
          }
        </p>
        <form data-form={"saved-search"} className={cls("lgi-tm-row")}>
          <input
            name={"query"}
            defaultValue={esc(activeDialog.query || "")}
            placeholder={"查找已保存标题 / 角度 / 研究说明"}
            aria-label={"查找备选"}
          />
          <TopicButton type={"submit"} className={cls("lgi-tm-button")}>
            {"查找"}
          </TopicButton>
        </form>
        {rows.length
          ? rows.map((a) => (
              <>
                <article className={cls("lgi-tm-saved")}>
                  <div className={cls("lgi-tm-row")}>
                    {badge(
                      a.methodVersion === "topic-map.manual.v1"
                        ? a.topicRef
                          ? "人工备选"
                          : "人工备选 · 待归方向"
                        : a.kind === "product_research"
                          ? "产品研究说明"
                          : "内容角度",
                    )}
                    {badge(
                      a.sourceState === "available"
                        ? "来源当前可用"
                        : "读取原版本时复核来源资格",
                    )}
                    {badge("不持续跟踪")}
                  </div>
                  <h3>{esc(a.title || "保存记录的来源资格已变化")}</h3>
                  <p>{esc(a.angle || "原保存文本需经来源资格复核后读取。")}</p>
                  <p>{esc(a.rationale)}</p>
                  <p className={cls("lgi-tm-note")}>
                    {[
                      esc(a.createdAt),
                      " · ",
                      esc(a.methodVersion),
                      " · ",
                      list(a.evidenceWorkRefs).length,
                      " 篇冻结作品引用",
                    ].join("")}
                  </p>
                  {btn("取回保存的原版本", "saved-materials", a.alternativeRef)}
                  {btn(
                    "导出引用与说明",
                    "export-alternative",
                    a.alternativeRef,
                    "lgi-tm-quiet",
                  )}
                </article>
              </>
            ))
          : empty(
              "当前没有匹配的已保存备选",
              "从材料列表选择“人工保留备选”，或保存已有研究角度；保存后可在这里取回依据。",
            )}
      </>
    );
  }
  function saveDialog() {
    const activeDialog = context.requireDialog();
    const a = activeDialog.angle;
    if (!a) return empty("角度与来源尚未取得");
    const product = activeDialog.kind === "product_research",
      manual = activeDialog.manual === true;
    if (activeDialog.savedAlternativeRef)
      return (
        <>
          <div className={cls("lgi-tm-callout")}>
            {[
              product ? "产品研究说明" : "内容角度",
              "已获得后端保存回执。保存不启动监控、采集或业务实验。",
            ].join("")}
          </div>
          <div className={cls("lgi-tm-row")}>
            {btn(
              "取回刚保存的原版本",
              "saved-materials",
              activeDialog.savedAlternativeRef,
            )}
            {btn(
              "复核来源并导出",
              "export-alternative",
              activeDialog.savedAlternativeRef,
            )}
          </div>
        </>
      );
    return (
      <>
        <h3>
          {manual
            ? "人工保留材料与选题想法"
            : product
              ? "保留需求、支持假设与最先验证的问题"
              : "这个方向，先收下来"}
        </h3>
        {manual ? (
          <>
            <p className={cls("lgi-tm-analysis-note")}>
              {
                "人工提出 · 待归方向。保存不改变作品归属，不依赖模型研究；这里保留交接材料，尚未进入运营选题池。"
              }
            </p>
          </>
        ) : (
          ""
        )}
        <p>
          {[
            product
              ? "修改是人工研究说明，不改变模型原研究结果。"
              : "内容备选不是已发布作品，不计入我方发布覆盖。",
            manual
              ? "保存人工说明与冻结的作者来源；无主题定义和模型结果引用。"
              : "保存保留当前主题定义、研究版本与引用。",
            "不自动持续跟踪。",
          ].join("")}
        </p>
        <form data-form={"save"} className={cls("lgi-tm-form")}>
          <label>
            <span>{product ? "研究说明标题" : "备选标题 · 可继续打磨"}</span>
            <input
              name={"title"}
              defaultValue={esc(a.title)}
              maxLength={120}
              required={true}
            />
          </label>
          <label>
            <span>{product ? "具体需求与支持假设" : "切入角度"}</span>
            <textarea
              name={"angle"}
              maxLength={2000}
              required={true}
              defaultValue={esc(a.label)}
            ></textarea>
          </label>
          <label>
            <span>
              {product ? "可编辑研究说明 / 最先验证什么" : "这篇要回答的问题"}
            </span>
            <textarea
              name={"rationale"}
              maxLength={2000}
              required={true}
              defaultValue={esc(a.answerTask)}
            ></textarea>
          </label>
          {product ? (
            ""
          ) : (
            <>
              <fieldset className={cls("lgi-tm-reuse-options")}>
                <legend>{"复用意图 · 可组合"}</legend>
                {[
                  "沿用标题",
                  "保留核心观点",
                  "调整封面",
                  "切换场景",
                  "重新组织内容",
                ].map((value) => (
                  <>
                    <label className={cls("lgi-tm-check-label")}>
                      <input
                        type={"checkbox"}
                        name={"reuse"}
                        defaultValue={value}
                      />
                      <span>{value}</span>
                    </label>
                  </>
                ))}
              </fieldset>
              <label>
                <span>{"回答边界 · 没有依据时保留未知"}</span>
                <textarea
                  name={"boundary"}
                  maxLength={800}
                  placeholder={"明确这篇不回答什么、适用条件以及还缺少哪些材料"}
                  defaultValue={esc(list(a.limitations).join("；"))}
                ></textarea>
              </label>
              <label>
                <span>{"备注（可不填）"}</span>
                <textarea
                  name={"note"}
                  maxLength={400}
                  placeholder={"以后打开时，帮助自己想起为什么保留。"}
                ></textarea>
              </label>
            </>
          )}
          <div className={cls("lgi-tm-callout")}>
            <strong>{"原始依据与人工说明一起保存"}</strong>
            <p>
              {[
                esc(title(context.work(a.workRef))),
                " · ",
                manual
                  ? "保留已取得的标题、正文和媒体文本版本（每个字段最多前4,000字）；评论不会自动作为人工备选依据"
                  : `${list(a.evidence).length} 条定位引用`,
              ].join("")}
            </p>
            {comparisonScopeNote(a)}
            <p>
              {
                "复用意图、回答边界与备注以明确字段标题保存在现有角度和说明中；不改变原始作品或原模型结论。"
              }
            </p>
          </div>
          <TopicButton
            type={"submit"}
            className={cls("lgi-tm-button lgi-tm-primary")}
            data-mutation=""
          >
            {product ? "保存研究说明与引用" : "保存到我的备选"}
          </TopicButton>
        </form>
      </>
    );
  }
  function opportunitiesFrom(outputs: T.ResearchOutputs) {
    return researchItems(
      outputs,
      "productOpportunities",
      "researchOpportunityIndex",
    );
  }
  function productContent(outputs: T.ResearchOutputs) {
    const opportunities = opportunitiesFrom(outputs);
    return (
      <>
        <p>
          {
            "首版只形成需求、支持假设与先验证的问题，可以编辑并保存研究说明及引用；不启动业务实验。"
          }
        </p>
        {opportunities.length
          ? opportunities.map((o, index) => (
              <>
                <article className={cls("lgi-tm-panel")}>
                  <h4>{"具体需求"}</h4>
                  <p>{esc(o.need)}</p>
                  <h4>{"可能的支持方式"}</h4>
                  <p>{esc(o.hypothesis)}</p>
                  <h4>{"最先验证什么"}</h4>
                  <p>{esc(o.verificationQuestion)}</p>
                  <h4>{"替代解释与限制"}</h4>
                  <p>{esc(o.alternativeExplanation)}</p>
                  {comparisonScopeNote(o)}
                  <div className={cls("lgi-tm-row")}>
                    {btn("核对依据", "reader", o.workRef, "lgi-tm-quiet")}
                    {btn(
                      "编辑并保存研究说明",
                      "save-opportunity",
                      String(index),
                    )}
                  </div>
                </article>
              </>
            ))
          : empty(
              "当前证据不足以提出可引用的产品机会",
              "先核对具体需求与现有回答，不补造付费意愿、市场规模或产品结论。",
            )}
      </>
    );
  }
  function productDialog() {
    const activeDialog = context.requireDialog();
    return productContent(
      analysisOutputs(activeDialog.works || context.selectedWorks()),
    );
  }
  function alternativeDialog() {
    const activeDialog = context.requireDialog();
    if (activeDialog.loading)
      return (
        <>
          <p role={"status"}>
            {"正在复核当前来源资格，并读取保存时的定义与引用版本…"}
          </p>
        </>
      );
    const a = activeDialog.alternative;
    if (!context.alternativeAvailable(a))
      return (
        <>
          {empty(
            "保存记录的来源当前不可用",
            "原文本与引用片段不继续展示。历史引用记录保留，不能用当前作品替代原依据。",
          )}
          {
            <>
              <p className={cls("lgi-tm-note")}>
                {[
                  "记录 ",
                  esc(a?.alternativeRef || activeDialog.alternativeRef),
                  " · 方法 ",
                  esc(a?.methodVersion || "未知"),
                ].join("")}
              </p>
            </>
          }
        </>
      );
    const d = a.originalDefinition,
      product = a.kind === "product_research";
    return (
      <>
        {a.sourceState === "version_changed_available" ? (
          <>
            <div className={cls("lgi-tm-callout")}>
              {
                "来源或研究版本后来有变化。下方展示保存时的原定义、说明和已接纳原版本片段；当前来源限制已重新复核。原版本不代表当前判断。"
              }
            </div>
          </>
        ) : (
          <>
            <div className={cls("lgi-tm-callout")}>
              {
                "下方为保存时的版本，当前来源资格已复核。查看与导出不启动研究、监控或采集。"
              }
            </div>
          </>
        )}
        <div className={cls("lgi-tm-object-head")}>
          <div>
            <div className={cls("lgi-tm-row")}>
              {badge(product ? "产品研究说明" : "内容角度")}
              {badge("保存的原版本")}
            </div>
            <h3>{esc(a.title)}</h3>
            <p>{esc(a.methodVersion)}</p>
          </div>
          {btn(
            "复核来源并导出Markdown",
            "export-alternative",
            a.alternativeRef,
          )}
        </div>
        <section className={cls("lgi-tm-section")}>
          <h4>{product ? "具体需求与支持假设" : "保存的角度"}</h4>
          <div className={cls("lgi-tm-original")}>{esc(a.angle)}</div>
          <h4>{product ? "人工编辑的研究说明" : "回答任务与保留理由"}</h4>
          <div className={cls("lgi-tm-original")}>{esc(a.rationale)}</div>
        </section>
        <section className={cls("lgi-tm-section")}>
          <h3>{"保存时的主题定义"}</h3>
          {d ? (
            <>
              <h4>
                {[esc(d.displayName), " · 定义 v", number(d.version)].join("")}
              </h4>
              <p>{esc(d.definitionText)}</p>
              <small className={cls("lgi-tm-num")}>
                {esc(d.definitionRef)}
              </small>
            </>
          ) : a.topicRef == null ? (
            <>
              <p>{"人工备选，保存时未归方向；未生成主题定义或材料归属。"}</p>
            </>
          ) : (
            empty("原定义尚未返回", "不以当前主题定义代替原版本。")
          )}
        </section>
        <section className={cls("lgi-tm-section")}>
          <h3>{"保存时的正文与原声依据"}</h3>
          <p>
            {
              "原文和机器读图 / 转写分别保留来源；高亮来自保存时的精确引用范围。"
            }
          </p>
          {withEmpty(
            list(a.fragments).map((f) => (
              <>
                <article className={cls("lgi-tm-frozen-fragment")}>
                  <div className={cls("lgi-tm-row")}>
                    {badge(sourceKind(f.field))}
                    <span className={cls("lgi-tm-num")}>{esc(f.workRef)}</span>
                  </div>
                  <blockquote>
                    {highlighted(f.text, list(f.citedRanges))}
                  </blockquote>
                  <p className={cls("lgi-tm-note")}>
                    {[
                      "来源 ",
                      esc(f.sourceRef),
                      " · ",
                      esc(f.sourceVersion || "来源版本未记录"),
                      " · 字符 ",
                      number(f.start),
                      "–",
                      number(f.end),
                    ].join("")}
                  </p>
                </article>
              </>
            )),
            empty("本记录没有可展示的冻结来源片段", "保留记录不补造来源内容。"),
          )}
        </section>
        <p className={cls("lgi-tm-note")}>
          {[
            "记录 ",
            esc(a.alternativeRef),
            " · 原定义 ",
            esc(a.definitionRef),
          ].join("")}
        </p>
      </>
    );
  }
  function alternativeMarkdown(a: T.ReadableAlternative) {
    const d = a.originalDefinition,
      product = a.kind === "product_research";
    const lines = [
      `# ${a.title}`,
      "",
      `类型：${product ? "产品研究说明" : "内容角度"}`,
      `保存记录：${a.alternativeRef}`,
      `领域：${a.domainRef}`,
      `主题：${a.topicRef || "人工备选，待归方向"}`,
      `原定义：${a.definitionRef || "保存时未绑定定义"}`,
      `研究方法：${a.methodVersion}`,
      `来源复核：${a.sourceState === "version_changed_available" ? "原版本当前仍可读，后来版本已有变化" : "原版本当前可读"}`,
      "",
      "保存不开启监控、采集或业务实验。此说明及依据保留当时版本，不代表当前事实、市场趋势或产品价值已得到验证。",
      "",
      `## ${product ? "具体需求与支持假设" : "保存的角度"}`,
      "",
      a.angle,
      "",
      `## ${product ? "人工编辑的研究说明" : "回答任务与保留理由"}`,
      "",
      a.rationale,
      "",
      "## 保存时的主题定义",
      "",
    ];
    if (d)
      lines.push(
        `${d.displayName} · v${d.version}`,
        d.definitionText,
        `定义引用：${d.definitionRef}`,
      );
    else
      lines.push(
        a.topicRef == null
          ? "人工备选，保存时未归方向；没有生成主题或材料归属。"
          : "原定义未取得，未以当前定义替代。",
      );
    lines.push("", "## 保存时的原文引用", "");
    list(a.fragments).forEach((f, index) => {
      lines.push(
        `### 引用 ${index + 1} · ${sourceKind(f.field)}`,
        "",
        `作品：${f.workRef}`,
        `来源：${f.sourceRef}`,
        `来源版本：${f.sourceVersion || "未知"}`,
        `片段身份：${f.fragmentId}`,
        `字符范围：${f.start}–${f.end}`,
        `高亮引用范围：${
          list(f.citedRanges)
            .map((r) => `${r.start}–${r.end}`)
            .join("、") || "无额外高亮范围"
        }`,
        "",
        markdownFence(f.text),
        "",
      );
    });
    return lines.join("\n");
  }
  function changesDialog() {
    const activeSnapshot = context.requireSnapshot();
    const changes = list(activeSnapshot.changes).filter(
      (c) =>
        c.structureChanged || c.notViewed || list(c.unviewedWorkRefs).length,
    );
    return (
      <>
        <p>
          {
            "当前按主题定义版本与查看回执比较；研究依据变化和历史补录保留各自来源，不混称市场变化。"
          }
        </p>
        {changes.length
          ? changes.map((c) => (
              <>
                <article className={cls("lgi-tm-list-row")}>
                  <div>
                    <strong>{esc(context.topicName(c.topicRef))}</strong>
                    <p>
                      {[
                        c.notViewed
                          ? "首次查看此主题定义"
                          : "定义版本与上次查看不同",
                        " · 定义 v",
                        esc(c.definitionVersion),
                      ].join("")}
                    </p>
                    <p>
                      {[
                        "上次查看：",
                        esc(c.lastViewedAt || "尚无记录"),
                        " · ",
                        list(c.unviewedWorkRefs).length,
                        " 篇尚未记录为已读",
                      ].join("")}
                    </p>
                    <p className={cls("lgi-tm-note")}>
                      {"材料补录或分类差异不等于市场变化。"}
                    </p>
                  </div>
                  {btn("查看主题", "topic", c.topicRef)}
                  {btn(
                    "记为已查看本定义",
                    "viewed",
                    c.topicRef,
                    "lgi-tm-quiet",
                  )}
                </article>
              </>
            ))
          : empty("当前没有尚未查看的定义变化", "已有主题与历史材料继续可用。")}
      </>
    );
  }
  function researchDialog() {
    return <TopicResearchFlow domain={context.state.domainRef} works={context.selectedWorks().map(work => ({ workRef: work.workRef, title: work.title }))} />;
  }
  function moveDialog() {
    const activeDialog = context.requireDialog();
    const activeSnapshot = context.requireSnapshot();
    const ref = activeDialog.moveTopicRef || context.state.topicRef,
      t = context.topic(ref),
      choices = activeSnapshot.topics.filter(
        (t) => t.lifecycleState !== "superseded",
      );
    const descendants = new Set([ref]);
    let changed = true;
    while (changed) {
      changed = false;
      choices.forEach((c) => {
        if (
          descendants.has(c.parentTopicRef || "") &&
          !descendants.has(c.topicRef)
        ) {
          descendants.add(c.topicRef);
          changed = true;
        }
      });
    }
    return (
      <>
        <p>{"只调整本领域导航归属，不改写主题定义、来源材料与已有判断。"}</p>
        <form data-form={"move-topic"} className={cls("lgi-tm-form")}>
          <label>
            <span>{"要调整的主题"}</span>
            <select
              name={"topicRef"}
              data-dialog-filter={"moveTopicRef"}
              required={true}
            >
              <option value={""}>{"选择主题"}</option>
              {choices.map((c) => (
                <>
                  <option
                    value={esc(c.topicRef)}
                    {...attributes(c.topicRef === ref ? "selected" : "")}
                  >
                    {esc(c.displayName)}
                  </option>
                </>
              ))}
            </select>
          </label>
          <label>
            <span>{"新的父主题"}</span>
            <select name={"parentTopicRef"}>
              <option value={""}>{"作为一级主题"}</option>
              {choices
                .filter((c) => !descendants.has(c.topicRef))
                .map((c) => (
                  <>
                    <option
                      value={esc(c.topicRef)}
                      {...attributes(
                        t?.parentTopicRef === c.topicRef ? "selected" : "",
                      )}
                    >
                      {esc(c.displayName)}
                    </option>
                  </>
                ))}
            </select>
          </label>
          <p>
            {[
              "当前父主题：",
              t?.parentTopicRef
                ? esc(context.topicName(t.parentTopicRef))
                : "一级主题",
              " · 归属版本 ",
              t?.bindingVersion == null ? "尚未绑定" : number(t.bindingVersion),
            ].join("")}
          </p>
          <TopicButton
            type={"submit"}
            className={cls("lgi-tm-button lgi-tm-primary")}
            data-mutation=""
            {...attributes(t ? "" : "disabled")}
          >
            {"保存本次归属调整"}
          </TopicButton>
        </form>
      </>
    );
  }
  function structureDialog() {
    const activeDialog = context.requireDialog();
    const activeSnapshot = context.requireSnapshot();
    const kind = activeDialog.kind || "merge",
      count = kind === "merge" ? 1 : activeDialog.destinationCount || 2;
    const selected = list(activeDialog.sources),
      eligible = list(activeSnapshot.topics)
        .filter((t) => t.lifecycleState !== "superseded")
        .filter(
          (t) =>
            !activeSnapshot.topics.some((c) => c.parentTopicRef === t.topicRef),
        );
    const refs = [
        ...new Set(
          selected.flatMap((ref) => list(context.topic(ref)?.workRefs)),
        ),
      ],
      ns = activeSnapshot.works.filter((w) => refs.includes(w.workRef));
    if (activeDialog.preview) {
      const { plan, preview } = activeDialog;
      if (!plan || !preview) return empty("结构预览尚未取得");
      return (
        <>
          <div className={cls("lgi-tm-callout")}>
            {
              "确认后，旧主题保留历史引用，新主题仅采用你显式分配的作品；旧角度、判断与统计不会自动继承。新定义仍是候选。"
            }
          </div>
          <h3>
            {[plan.kind === "merge" ? "合并" : "拆分", "影响预览"].join("")}
          </h3>
          {plan.sources.map((s) => (
            <>
              <p>
                {[
                  "旧主题：",
                  esc(context.topicName(s.topicRef)),
                  " · 定义 ",
                  esc(s.definitionRef),
                ].join("")}
              </p>
            </>
          ))}
          {plan.destinations.map((d) => (
            <>
              <article className={cls("lgi-tm-panel")}>
                <h4>{esc(d.displayName)}</h4>
                <p>{esc(d.definitionText)}</p>
                <p>{[d.members.length, " 篇显式材料分配"].join("")}</p>
              </article>
            </>
          ))}
          <p>
            {[
              list(preview.impact?.unassignedWorkRefs).length,
              " 篇未重新分配的材料保留旧历史归属。",
            ].join("")}
          </p>
          <p>{esc(plan.rationale)}</p>
          <div className={cls("lgi-tm-row")}>
            {btn("返回修改", "structure-edit")}
            {btn(
              "确认应用此结构调整",
              "structure-apply",
              "",
              "lgi-tm-primary",
              activeDialog.structureApplied
                ? "disabled data-mutation"
                : "data-mutation",
            )}
          </div>
        </>
      );
    }
    return (
      <>
        <p>
          {
            "先选择旧叶主题，显式分配每篇作品，再看后端影响预览。拆分与合并不会自动继承旧判断或研究。"
          }
        </p>
        <div className={cls("lgi-tm-toolbar")}>
          <label className={cls("lgi-tm-control")}>
            <span>{"调整方式"}</span>
            <select data-dialog-filter={"kind"}>
              <option
                value={"merge"}
                {...attributes(kind === "merge" ? "selected" : "")}
              >
                {"合并多个主题"}
              </option>
              <option
                value={"split"}
                {...attributes(kind === "split" ? "selected" : "")}
              >
                {"拆分一个主题"}
              </option>
            </select>
          </label>
          {kind === "split"
            ? btn("再增加一个新方向", "structure-add-destination")
            : ""}
        </div>
        <form data-form={"structure"} className={cls("lgi-tm-form")}>
          <fieldset>
            <legend>
              {kind === "merge" ? "选择至少两个旧叶主题" : "选择一个旧叶主题"}
            </legend>
            {eligible.map((t) => (
              <>
                <label className={cls("lgi-tm-check-label")}>
                  <input
                    type={"checkbox"}
                    data-structure-source={esc(t.topicRef)}
                    name={"sources"}
                    defaultValue={esc(t.topicRef)}
                    {...attributes(
                      selected.includes(t.topicRef) ? "checked" : "",
                    )}
                  />
                  <span>
                    {[
                      esc(t.displayName),
                      " · ",
                      number(t.statistics?.workCount),
                      " 篇",
                    ].join("")}
                  </span>
                </label>
              </>
            ))}
          </fieldset>
          {Array.from({ length: count }, (_, i) => (
            <>
              <fieldset>
                <legend>{["新方向 ", i + 1].join("")}</legend>
                <label>
                  <span>{"名称"}</span>
                  <input
                    name={"title-" + String(i ?? "")}
                    maxLength={120}
                    required={true}
                  />
                </label>
                <label>
                  <span>{"定义与边界"}</span>
                  <textarea
                    name={"definition-" + String(i ?? "")}
                    maxLength={2000}
                    required={true}
                  ></textarea>
                </label>
                <label>
                  <span>{"父主题"}</span>
                  <select name={"parent-" + String(i ?? "")}>
                    <option value={""}>{"作为一级主题"}</option>
                    {activeSnapshot.topics
                      .filter(
                        (t) =>
                          !selected.includes(t.topicRef) &&
                          t.lifecycleState !== "superseded",
                      )
                      .map((t) => (
                        <>
                          <option value={esc(t.topicRef)}>
                            {esc(t.displayName)}
                          </option>
                        </>
                      ))}
                  </select>
                </label>
              </fieldset>
            </>
          ))}
          <h3>{"逐篇显式分配"}</h3>
          <p>
            {
              "不分配的作品保留旧历史，不会自动带入新方向。每个新方向至少选择一篇实际作品。"
            }
          </p>
          {ns.map((w) => (
            <>
              <fieldset>
                <legend>{esc(title(w))}</legend>
                <label>
                  <span>{"进入哪个新方向"}</span>
                  <select name={"member-" + String(w.workRef ?? "")}>
                    <option value={""}>{"本次不重新分配"}</option>
                    {Array.from({ length: count }, (_, i) => (
                      <>
                        <option value={i}>{["新方向 ", i + 1].join("")}</option>
                      </>
                    ))}
                  </select>
                </label>
                <label>
                  <span>{"材料作用"}</span>
                  <select name={"role-" + String(w.workRef ?? "")}>
                    <option value={"support"}>{"支持"}</option>
                    <option value={"challenge"}>{"反例"}</option>
                    <option value={"boundary"}>{"边界"}</option>
                  </select>
                </label>
                <label>
                  <span>{"此分配的具体理由"}</span>
                  <input
                    name={"reason-" + String(w.workRef ?? "")}
                    maxLength={1000}
                    placeholder={"分配后须说明具体依据"}
                  />
                </label>
              </fieldset>
            </>
          ))}
          <label>
            <span>{"整体结构调整理由"}</span>
            <textarea
              name={"rationale"}
              maxLength={2000}
              required={true}
            ></textarea>
          </label>
          <TopicButton
            type={"submit"}
            className={cls("lgi-tm-button lgi-tm-primary")}
            data-mutation=""
          >
            {"预览实际影响"}
          </TopicButton>
        </form>
      </>
    );
  }
  function captureDialog() {
    const activeDialog = context.requireDialog();
    if (activeDialog.loading)
      return (
        <>
          <p role={"status"}>{"正在读取已有执行范围与补采轮次…"}</p>
        </>
      );
    const p = activeDialog.collection,
      search = activeDialog.search,
      deep = Boolean(activeDialog.workRef);
    return (
      <>
        <div className={cls("lgi-tm-callout")}>
          {
            "详情每轮最多读取10篇新作品；临时搜索最多3个显式词、200篇候选，不建立长期监控。深入评论仅由显式点击追加，新增评论与回复合计最多30条。关闭弹窗不取消本轮。"
          }
        </div>
        <div className={cls("lgi-tm-row")}><span>本领域主题补采：{p?.policy.enabled ? '已允许' : '未允许'}；每轮仍需明确选择范围和已有采集授权。</span>{btn(p?.policy.enabled ? '暂停主题补采' : '允许本领域主题补采', 'capture-configure')}</div>
        <form data-form={"capture"} className={cls("lgi-tm-form")}>
          {deep ? (
            <>
              <label>
                <span>{"现有受控采集目标"}</span>
                <select name={"targetRef"} required={true}>
                  <option value={""}>{"选择已有执行目标"}</option>
                  {list(p?.targets).map((t) => (
                    <>
                      <option value={esc(t.targetRef)}>
                        {[
                          esc(t.displayName),
                          " · ",
                          t.kind === "keyword" ? "关键词目标" : "创作者目标",
                        ].join("")}
                      </option>
                    </>
                  ))}
                </select>
              </label>
              <p>
                {[
                  "深入讨论：",
                  esc(title(context.work(activeDialog.workRef))),
                  " · 最多新增30条评论与回复",
                ].join("")}
              </p>
            </>
          ) : (
            <>
              <label>
                <span>{"现有关键词执行授权"}</span>
                <select name={"authorizationRef"} required={true}>
                  <option value={""}>{"选择已有授权范围"}</option>
                  {list(search?.authorizations).map((a) => (
                    <>
                      <option value={esc(a.authorizationRef)}>
                        {[
                          esc(a.purpose),
                          " · 每目标最多 ",
                          number(a.maxWorksPerTarget),
                          " 篇 · 到期 ",
                          esc(a.expiresAt),
                        ].join("")}
                      </option>
                    </>
                  ))}
                </select>
              </label>
              <label>
                <span>{"本主题临时搜索词"}</span>
                <input
                  name={"keywords"}
                  maxLength={300}
                  required={true}
                  placeholder={"最多3个明确词，用逗号分隔"}
                />
              </label>
            </>
          )}
          <label>
            <span>{"本轮补充目的"}</span>
            <input
              name={"purpose"}
              maxLength={120}
              required={true}
              placeholder={"说明当前具体缺项"}
            />
          </label>
          <label className={cls("lgi-tm-check-label")}>
            <input type={"checkbox"} name={"newRound"} />
            <span>{"明确再开启新一轮；默认取回同范围已有轮次"}</span>
          </label>
          <TopicButton
            type={"submit"}
            className={cls("lgi-tm-button lgi-tm-primary")}
            data-mutation=""
          >
            {deep ? "明确追加讨论样本" : "明确开启一轮临时搜索"}
          </TopicButton>
        </form>
        <div className={cls("lgi-tm-row")}>
          {btn("刷新真实进度", "capture-refresh")}
        </div>
        <section className={cls("lgi-tm-section")}>
          <h3>{"临时搜索与详情补充"}</h3>
          {withEmpty(
            list(search?.rounds).map((r) => (
              <>
                <article className={cls("lgi-tm-panel lgi-tm-section")}>
                  <div className={cls("lgi-tm-section-head")}>
                    <div>
                      <h4>
                        {[
                          esc(labelState(r.state)),
                          " · ",
                          r.phase === "search" ? "搜索阶段" : "详情阶段",
                        ].join("")}
                      </h4>
                      <p>
                        {[
                          list(r.searches)
                            .map((s) => esc(s.keyword))
                            .join("、"),
                          " · 已发现候选 ",
                          number(r.candidateCount),
                          " / ",
                          number(r.candidateLimit),
                        ].join("")}
                      </p>
                      <p>
                        {[
                          "详情预留 ",
                          number(r.detailSlotsReserved),
                          " / 10 · 已取得合格详情 ",
                          number(r.actualDetailsAcquired),
                          " · 待取得 ",
                          list(r.pendingDetailWorkRefs).length,
                        ].join("")}
                      </p>
                    </div>
                    {btn(
                      "明确停止本轮",
                      "search-stop",
                      r.roundRef,
                      "lgi-tm-danger",
                      "data-mutation",
                    )}
                  </div>
                  {r.phase === "search" &&
                  list(r.candidateWorkRefs).length &&
                  ["active", "partial"].includes(r.state) ? (
                    <>
                      <form
                        data-form={"freeze-details"}
                        data-round={esc(r.roundRef)}
                      >
                        <h4>{"从本轮真实候选中选择最多10篇详情"}</h4>
                        <p>
                          {
                            "选择后停止继续搜索并冻结本轮详情身份；已有合格详情复用，不额外计新详情。"
                          }
                        </p>
                        {list(r.candidateWorkRefs).map((ref) => {
                          const w = context.work(ref);
                          return (
                            <>
                              <label className={cls("lgi-tm-check-label")}>
                                <input
                                  type={"checkbox"}
                                  name={"workRefs"}
                                  defaultValue={esc(ref)}
                                  data-detail-choice={esc(r.roundRef)}
                                />
                                <span>
                                  {[
                                    esc(w ? title(w) : "作品标题尚未读取"),
                                    " · ",
                                    esc(ref),
                                  ].join("")}
                                </span>
                                {w
                                  ? btn(
                                      "先看已有材料",
                                      "reader",
                                      ref,
                                      "lgi-tm-quiet",
                                    )
                                  : ""}
                              </label>
                            </>
                          );
                        })}
                        <TopicButton
                          type={"submit"}
                          className={cls("lgi-tm-button")}
                          data-mutation=""
                        >
                          {"冻结所选详情集合"}
                        </TopicButton>
                      </form>
                    </>
                  ) : (
                    ""
                  )}
                  <p className={cls("lgi-tm-note")}>
                    {
                      "搜索为空不证明现实没有需求，平台执行与分析结果更新分别以真实回执为准。"
                    }
                  </p>
                </article>
              </>
            )),
            empty(
              "当前没有临时搜索轮次",
              "已有材料继续可读，不会为填满页面自动补采。",
            ),
          )}
        </section>
        <section className={cls("lgi-tm-section")}>
          <h3>{"已有详情 / 评论补采轮次"}</h3>
          {withEmpty(
            list(p?.collectionRounds)
              .filter((r) => r.kind !== "search")
              .map((r) => (
                <>
                  <article className={cls("lgi-tm-run")}>
                    <div>
                      <h4>
                        {[
                          esc(labelState(r.state)),
                          " · ",
                          r.kind === "comments" ? "评论追加" : "详情补充",
                        ].join("")}
                      </h4>
                      <p>
                        {[
                          "预留详情身份 ",
                          number(r.detailSlotsReserved),
                          " / 10",
                        ].join("")}
                      </p>
                      <p>
                        {[esc(r.reason || ""), " · ", esc(r.roundRef)].join("")}
                      </p>
                    </div>
                    {btn(
                      "明确停止本轮",
                      "collection-stop",
                      r.roundRef,
                      "lgi-tm-danger",
                      "data-mutation",
                    )}
                  </article>
                </>
              )),
            empty(
              "当前没有已记录的详情或评论补采",
              "操作回执、真实平台取得与研究结果更新分别表达。",
            ),
          )}
        </section>
      </>
    );
  }
  return {
    materialsDialog,
    readerDialog,
    judgmentDialog,
    discussionsView,
    anglesFrom,
    anglesView,
    ownView,
    sampleReason,
    sampleView,
    replaceSamplesDialog,
    matrixDialog,
    compareDialog,
    identityDialog,
    rulesDialog,
    savedDialog,
    saveDialog,
    opportunitiesFrom,
    productContent,
    productDialog,
    alternativeDialog,
    alternativeMarkdown,
    changesDialog,
    researchDialog,
    moveDialog,
    structureDialog,
    captureDialog,
  };
}
