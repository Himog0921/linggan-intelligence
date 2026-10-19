import type { JSX, ReactNode } from "react";
import { OverviewReadouts } from "./OverviewReadouts";
import { OverviewDistribution } from "./OverviewDistribution";
import { attributes, inlineStyle } from "./elementProps";
import {
  comparisonScopeNote,
  materialSourceNote,
  researchCoverageSummary,
} from "./sourceViews";
import { c as cls } from "./styles";
import { badge, btn, empty, tab, withEmpty } from "./topicElements";
import {
  assignmentKey,
  authorUnavailable,
  citationText,
  discussionUnits,
  esc,
  fallbackStages,
  fragmentText,
  fragmentWorkRef,
  known,
  legacyConcept,
  list,
  locatedCitations,
  normalizeSearch,
  number,
  otherNames,
  pct,
  platformName,
  platformStats,
  publicationOrder,
  publishedTime,
  readableMaterial,
  readLabel,
  readoutNumber,
  sharesDiscussion,
  sourceKind,
  stageNames,
  title,
} from "./topicModel";
import type * as T from "./types";

/** Read-only page projection inputs; commands stay in the controller. */
export interface TopicOverviewContext {
  readonly state: Readonly<T.ViewState>;
  readonly selectedWorks: () => T.Work[];
  readonly topic: (ref: string | null | undefined) => T.Topic | undefined;
  readonly snapshot: T.Snapshot | null;
  readonly discussionsView: (ns: T.Work[]) => JSX.Element;
  readonly loading: boolean;
  readonly readError: string;
  readonly requireSnapshot: () => T.Snapshot;
  readonly views: string[][];
  readonly dialogState: T.DialogState | null;
  readonly readerCitationTarget: (
    w: T.Work,
    c: T.LocatedCitation,
    raw: string,
  ) => string;
  readonly anglesFrom: (ns: T.Work[]) => T.AnnotatedResearchItem[];
  readonly work: (ref: string | undefined | null) => T.Work | undefined;
  readonly topicName: (ref: string) => string;
}
export function createTopicOverview(context: TopicOverviewContext) {
  const select = (
    key: T.TextStateKey,
    label: string,
    options: readonly (readonly [string, string])[],
  ) => (
    <>
      <label className={cls("lgi-tm-control")}>
        <span>{esc(label)}</span>
        <select data-filter={key}>
          {options.map(([value, text]) => (
            <>
              <option
                value={esc(value)}
                {...attributes(
                  String(context.state[key]) === String(value)
                    ? "selected"
                    : "",
                )}
              >
                {esc(text)}
              </option>
            </>
          ))}
        </select>
      </label>
    </>
  );
  function matchesWork(w: T.Work, queryText = context.state.query) {
    const q = String(queryText || "")
      .normalize("NFKC")
      .toLocaleLowerCase()
      .trim();
    if (!q) return true;
    const text = [
      title(w),
      w.creatorDisplayName,
      typeof w.preview === "string" ? w.preview : w.preview?.text,
      w.evidenceFragment?.text,
      ...discussionUnits(w).flatMap((d) => [d.label, d.statement]),
      ...list(w.research?.output?.scenes).map((v) => v.label),
    ]
      .filter((v) => typeof v === "string")
      .join(" ")
      .normalize("NFKC")
      .toLocaleLowerCase();
    return text.includes(q);
  }
  function stageWorks(ns = context.selectedWorks()) {
    if (context.state.view !== "journey" || !context.state.stage) return ns;
    const journey =
      context.topic(context.state.topicRef)?.journey ||
      context.snapshot?.journey;
    const entry = journeyEntries(
      journey,
      stageNames[context.state.stage] ? context.state.metric : "main",
    ).find((e) => e.stage === context.state.stage);
    if (Array.isArray(entry?.workRefs)) {
      const refs = new Set(entry.workRefs);
      return ns.filter((w) => refs.has(w.workRef));
    }
    return ns
      .filter((w) => readableMaterial(w))
      .filter((w) =>
        context.state.metric === "involved" && stageNames[context.state.stage]
          ? list(w.involvedStages).includes(context.state.stage)
          : w.mainStage === context.state.stage,
      );
  }
  function workspaceWorks() {
    return sortedMaterials(stageWorks());
  }
  function scopeChips() {
    const chips = [];
    if (context.state.query)
      chips.push(
        btn(
          `关键词：${esc(context.state.query)} · 清除`,
          "clear-filter",
          "query",
          "lgi-tm-quiet",
        ),
      );
    if (context.state.view === "journey" && context.state.stage)
      chips.push(
        btn(
          `${esc(stageNames[context.state.stage] || otherNames[context.state.stage])} · 全部阶段`,
          "clear-filter",
          "stage",
          "lgi-tm-quiet",
        ),
      );
    return (
      <>
        <div className={cls("lgi-tm-scope-chips lgi-tm-row")}>
          {chips}
          {chips.length
            ? btn("清除全部浏览条件", "reset-scope", "", "lgi-tm-quiet")
            : ""}
        </div>
      </>
    );
  }
  function emptyWorkspace() {
    const stage =
      stageNames[context.state.stage] || otherNames[context.state.stage];
    return (
      <>
        {empty(
          context.state.query
            ? "当前条件没有关键词命中"
            : stage
              ? `当前没有合格归入「${stage}」的材料`
              : "当前条件没有对应作品",
          context.state.query
            ? "查找范围是标题、作者、已返回正文片段与讨论，不代表全文和全部评论。"
            : context.state.materialMode !== "latest"
              ? "高表现与常规作品按已保存平台规则区分；规则未知不计为低表现。"
              : "分类未覆盖或来源不足，不代表现实需求不存在。",
        )}
        <div className={cls("lgi-tm-row")}>
          {context.state.query
            ? btn("清除关键词", "clear-filter", "query")
            : ""}
          {context.state.stage
            ? btn("查看全部阶段", "clear-filter", "stage")
            : ""}
          {btn("查看待分析材料", "browse-pending")}
          {btn("返回全部材料", "reset-scope")}
        </div>
      </>
    );
  }
  function materialRows(ns: T.Work[]) {
    return (
      <>
        <div className={cls("lgi-tm-evidence-list")}>
          {ns.map((w) => {
            const cover = coverUrl(w);
            return (
              <>
                <article className={cls("lgi-tm-evidence-row")}>
                  <button
                    type={"button"}
                    className={cls("lgi-tm-evidence-cover")}
                    data-action={"reader"}
                    data-id={esc(w.workRef)}
                    aria-label={"打开" + String(esc(title(w)) ?? "")}
                  >
                    {cover ? (
                      <>
                        <img
                          src={esc(cover)}
                          loading={"lazy"}
                          alt={String(esc(title(w)) ?? "") + "的已取得封面"}
                        />
                      </>
                    ) : (
                      <>
                        <span>{"封面尚未采集"}</span>
                      </>
                    )}
                  </button>
                  <div className={cls("lgi-tm-evidence-copy")}>
                    <h3>
                      {btn(
                        esc(title(w)),
                        "reader",
                        w.workRef,
                        "lgi-tm-title-link",
                      )}
                    </h3>
                    <div className={cls("lgi-tm-material-meta")}>
                      <span>{esc(platformName(w.platform))}</span>
                      <span
                        className={cls("lgi-tm-material-author")}
                        title={esc(w.creatorDisplayName || "作者未知")}
                      >
                        {esc(w.creatorDisplayName || "作者未知")}
                      </span>
                      <span
                        className={cls("lgi-tm-material-date")}
                        title={esc(publishedTime(w))}
                      >
                        {esc(publishedTime(w))}
                      </span>
                      {w.own
                        ? badge(
                            w.ownBreakout ? "我方手动爆款" : "我方作品",
                            w.ownBreakout,
                          )
                        : ""}
                      {badge(
                        list(w.topicRefs).length ? "已有方向依据" : "待归方向",
                      )}
                    </div>
                    <p className={cls("lgi-tm-excerpt")}>
                      {esc(
                        (typeof w.preview === "string"
                          ? w.preview
                          : w.preview?.text) ||
                          fragmentText(w.evidenceFragment) ||
                          (authorUnavailable(w)
                            ? "作者原文当前不可用，可核对其他合格来源。"
                            : "当前没有可展示的正文片段。"),
                      )}
                    </p>
                    {materialSourceNote(w)}
                    <div className={cls("lgi-tm-material-footer")}>
                      <div className={cls("lgi-tm-material-numbers")}>
                        <span>
                          {"赞 "}
                          <b>{number(w.likes)}</b>
                        </span>
                        <span>
                          {"藏 "}
                          <b>{number(w.collects)}</b>
                        </span>
                        <span>
                          {"评 "}
                          <b>{number(w.comments)}</b>
                        </span>
                        <span>
                          {"分享 "}
                          <b>{number(w.shares)}</b>
                        </span>
                      </div>
                      <div className={cls("lgi-tm-row")}>
                        {btn(
                          readLabel(w, "阅读原文与讨论"),
                          "reader",
                          w.workRef,
                          "lgi-tm-quiet",
                        )}
                        {btn(
                          "人工保留备选",
                          "manual-save",
                          w.workRef,
                          "",
                          w.readable
                            ? ""
                            : 'disabled title="当前作者来源不足，已有评论仍可阅读"',
                        )}
                      </div>
                    </div>
                  </div>
                </article>
              </>
            );
          })}
        </div>
      </>
    );
  }
  function materialWorkspace() {
    const ns = workspaceWorks(),
      limit = 12,
      names = list(context.snapshot?.topics).filter(
        (t) =>
          t.lifecycleState !== "superseded" &&
          context.state.query &&
          normalizeSearch(t.displayName).includes(
            normalizeSearch(context.state.query),
          ),
      );
    return (
      <>
        <section
          className={cls("lgi-tm-workspace-content")}
          aria-label={"匹配材料"}
        >
          <div className={cls("lgi-tm-section-head")}>
            <div>
              <h3>{"材料与讨论"}</h3>
            <p>
              <strong>{[ns.length, " 篇当前匹配"].join("")}</strong>
              {context.snapshot?.scope?.totalWorkCount !== ns.length
                ? ` · ${number(context.snapshot?.scope?.totalWorkCount)} 篇服务端范围`
                : ""}
            </p>
            </div>
            <div className={cls("lgi-tm-row")}>
              {materialSearch()}
              {[
                ["latest", "最新作品"],
                ["high", "高表现"],
                ["ordinary", "常规作品"],
              ].map(([id, name]) =>
                tab(
                  name,
                  "material-mode",
                  id,
                  context.state.materialMode === id,
                ),
              )}
            </div>
          </div>
          {scopeChips()}
          {context.state.query ? (
            <>
              <details className={cls("lgi-tm-core-details")}>
                <summary>
                  {["方向名称另有 ", names.length, " 个命中"].join("")}
                </summary>
                <p className={cls("lgi-tm-note")}>
                  {
                    "关键词匹配当前返回的标题、作者、正文片段、讨论及场景；不改上方服务端统计。"
                  }
                </p>
                <div className={cls("lgi-tm-row")}>
                  {names.map((t) =>
                    btn(
                      esc(t.displayName),
                      "topic",
                      t.topicRef,
                      "lgi-tm-quiet",
                    ),
                  )}
                </div>
              </details>
            </>
          ) : (
            ""
          )}
          {context.state.materialMode !== "latest" ? (
            <>
              <p className={cls("lgi-tm-note")}>
                {
                  "高表现与常规作品按已保存的平台样本规则筛选，未知指标不计为0；不等于低粉高赞或我方手动爆款。"
                }
              </p>
            </>
          ) : (
            ""
          )}
          {ns.length ? (
            <>
              {materialRows(ns.slice(0, limit))}
              <div className={cls("lgi-tm-section-head")}>
                <span className={cls("lgi-tm-note")}>
                  {[
                    "展示 ",
                    Math.min(limit, ns.length),
                    " / ",
                    ns.length,
                    " 篇",
                  ].join("")}
                </span>
                {btn(
                  "查看全部匹配材料",
                  "subset",
                  ns.map((w) => w.workRef).join(","),
                  "lgi-tm-quiet",
                )}
              </div>
            </>
          ) : (
            emptyWorkspace()
          )}
          {context.state.view === "overview" ? (
            <>
              <details className={cls("lgi-tm-core-details")}>
                <summary>{"已有讨论与场景"}</summary>
                {context.discussionsView(ns)}
                {sceneList(ns)}
              </details>
            </>
          ) : (
            ""
          )}
        </section>
      </>
    );
  }
  function sidebar() {
    const busy = context.loading || Boolean(context.readError);
    return (
      <>
        <aside
          className={cls("lgi-tm-sidebar")}
          data-loaded={!busy}
          aria-label={"主题图谱导航"}
        >
          <div
            id={"topic-map-navigation"}
            className={cls(
              "lgi-tm-navigation-body " +
                String(context.state.navigationOpen ? "is-open" : ""),
            )}
          >
            {busy ? (
              <>
                <div className={cls("lgi-tm-tree")}>
                  <h2 className={cls("v7-sr-only")}>{"主题列表"}</h2>
                  <p className={cls("lgi-tm-note")}>{"当前范围待读取"}</p>
                </div>
              </>
            ) : (
              tree()
            )}
          </div>
          <div className={cls("lgi-tm-sidebar-tools")}>
          {btn(
            context.state.navigationOpen ? "收起主题树" : "展开主题树",
            "toggle-navigation",
            "",
            "lgi-tm-navigation-toggle",
            {
              "aria-expanded": context.state.navigationOpen,
              "aria-controls": "topic-map-navigation",
            },
          )}

            {btn(
              "我方矩阵",
              "identity",
              "",
              "lgi-tm-quiet",
              busy ? "disabled" : "",
            )}
            {btn(
              "计数与统计口径",
              "method",
              "",
              "lgi-tm-quiet",
              busy ? "disabled" : "",
            )}
          </div>
        </aside>
      </>
    );
  }
  function scopeText() {
    return `${list(context.snapshot?.domains).find((d) => (d.domainRef || d.id) === context.state.domainRef)?.displayName || list(context.snapshot?.domains).find((d) => (d.domainRef || d.id) === context.state.domainRef)?.name || "当前领域"} · ${context.state.platform || "全部平台"} · ${context.state.windowDays ? `近${context.state.windowDays}天发布` : "全部发布时间"}`;
  }
  function header() {
    return (
      <>
        <h1 className={cls("v7-sr-only")}>{"主题图谱"}</h1>
      </>
    );
  }
  function observedPlatforms(stats = context.snapshot?.statistics) {
    return platformStats(stats).filter(
      (p) => p && typeof p.platform === "string" && p.workCount !== 0,
    );
  }
  function platformControl() {
    const ids = [...new Set(observedPlatforms().map((p) => p.platform))];
    if (context.state.platform && !ids.includes(context.state.platform))
      ids.push(context.state.platform);
    if (ids.length < 2 && !context.state.platform)
      return ids.length ? (
        <>
          <span className={cls("lgi-tm-platform-context")}>
            {esc(platformName(ids[0]))}
          </span>
        </>
      ) : (
        ""
      );
    return select("platform", "平台", [
      ["", "全部平台"],
      ...ids.map((id) => [id, platformName(id)] as const),
    ]);
  }
  function controls() {
    return (
      <>
        <div className={cls("lgi-tm-toolbar")} aria-label={"当前统计范围"}>
          {platformControl()}
          {select(
            "windowDays",
            context.state.view === "journey" ? "笔记时间" : "发布时间",
            [
              ["", "全部时间"],
              ["7", "近7天"],
              ["30", "近30天"],
              ["90", "近90天"],
            ],
          )}
          {context.state.view === "journey" ? (
            <>
              {select("path", "经历", [
                ["", "全部经历"],
                ["family", "家庭支持"],
                ["adult", "成年自我管理"],
              ])}
              {select("overlay", "跨阶段状态", [
                ["", "全部状态"],
                ["obstruction_recurrence", "受阻与反复"],
                ["transition_handoff", "环境转换与支持交接"],
              ])}
            </>
          ) : (
            ""
          )}
        </div>
      </>
    );
  }
  function materialSearch() {
    return (
      <>
        <form className={cls("lgi-tm-search")} data-form={"search"}>
          <label className={cls("v7-sr-only")} htmlFor={"topic-map-search"}>
            {"查找材料与方向"}
          </label>
          <div className={cls("lgi-tm-row")}>
            <input
              id={"topic-map-search"}
              name={"query"}
              defaultValue={esc(context.state.query)}
              placeholder={"查找材料与方向"}
            />
            <button className={cls("lgi-tm-button")} type={"submit"}>
              {"查找"}
            </button>
          </div>
        </form>
      </>
    );
  }
  function summary() {
    const activeSnapshot = context.requireSnapshot();
    const selected = context.topic(context.state.topicRef),
      parents = [],
      visited = new Set();
    let current = selected;
    while (current && !visited.has(current.topicRef)) {
      visited.add(current.topicRef);
      parents.unshift(current);
      current = context.topic(current.parentTopicRef);
    }
    return (
      <>
        <section
          className={cls("lgi-tm-summary")}
          aria-label={"当前主题基础概览"}
        >
          <div className={cls("lgi-tm-summary-heading")}>
            <div className={cls("lgi-tm-summary-identity")}>
              
            <nav aria-label={"当前主题路径"}>
              {btn("全部方向", "topic", "", "lgi-tm-quiet")}
              {parents.map((t) => (
                <>
                  <span aria-hidden={"true"}>{"/"}</span>
                  {btn(
                    esc(t.displayName),
                    "topic",
                    t.topicRef,
                    "lgi-tm-quiet",
                    {
                      title: t.displayName,
                      "aria-current": t === selected ? "page" : undefined,
                    },
                  )}
                </>
              ))}
            </nav>
            </div>
            <div className={cls("lgi-tm-row")}>
              {selected ? (
                <>
                  {btn(
                    "定义与依据",
                    "definition",
                    selected.topicRef,
                    "lgi-tm-quiet",
                  )}
                  {btn("判断这个方向", "judge", selected.topicRef)}
                </>
              ) : (
                ""
              )}
              <span
                className={cls("lgi-tm-asof")}
                title={esc(context.snapshot?.scope?.asOf || "未知")}
              >
                {[
                  "指标截至 ",
                  esc(
                    String(context.snapshot?.scope?.asOf || "未知")
                      .replace("T", " ")
                      .slice(0, 16),
                  ),
                ].join("")}
              </span>
              {btn("统计口径", "method", "", "lgi-tm-quiet")}
              {controls()}
            </div>
          </div>
          <div className={cls("lgi-tm-overview-band")} aria-label="主题概览与样本表现">
            <OverviewReadouts
              statistics={selected?.statistics || activeSnapshot.statistics}
              scope={activeSnapshot.scope}
            />
            <OverviewDistribution
              compact
              works={context.selectedWorks().filter((w) => w.platform === plotPlatform(selected?.statistics || activeSnapshot.statistics))}
              platform={platformStats(selected?.statistics || activeSnapshot.statistics).find((p) => p.platform === plotPlatform(selected?.statistics || activeSnapshot.statistics))}
              statistics={selected?.statistics || activeSnapshot.statistics}
              scope={activeSnapshot.scope}
              headControls={observedPlatforms(selected?.statistics || activeSnapshot.statistics).length > 1 ? plotSelect(selected?.statistics || activeSnapshot.statistics) : null}
            />
          </div>
          {known(context.snapshot?.scope?.totalWorkCount) &&
          activeSnapshot.scope.totalWorkCount >
            list(activeSnapshot.works).length ? (
            <>
              <p className={cls("lgi-tm-note lgi-tm-partial")}>
                {[
                  "当前载入 ",
                  number(list(activeSnapshot.works).length),
                  " / ",
                  number(activeSnapshot.scope.totalWorkCount),
                  " 篇；上方统计为服务端完整范围。",
                ].join("")}
              </p>
            </>
          ) : (
            ""
          )}
        </section>
      </>
    );
  }
  function tree() {
    const topics = list(context.snapshot?.topics).filter(
        (t) => t.lifecycleState !== "superseded",
      ),
      roots = topics.filter(
        (t) =>
          !t.parentTopicRef ||
          !topics.some((x) => x.topicRef === t.parentTopicRef),
      );
    function branch(t: T.Topic, trail = new Set<string>()): ReactNode {
      if (trail.has(t.topicRef)) return "";
      const next = new Set(trail);
      next.add(t.topicRef);
      const children = topics.filter((c) => c.parentTopicRef === t.topicRef),
        expanded = !list(context.state.collapsed).includes(t.topicRef);
      return (
        <>
          <li>
            <div
              className={cls(
                "lgi-tm-tree-row " +
                  String(
                    context.state.topicRef === t.topicRef ? "is-active" : "",
                  ),
              )}
            >
              {children.length ? (
                btn(
                  expanded ? "−" : "+",
                  "collapse",
                  t.topicRef,
                  "lgi-tm-tree-toggle",
                  {
                    "aria-label": `${expanded ? "收起" : "展开"}${t.displayName}`,
                    "aria-expanded": expanded,
                  },
                )
              ) : (
                <>
                  <span className={cls("lgi-tm-tree-spacer")}></span>
                </>
              )}
              {btn(
                esc(t.displayName),
                "topic",
                t.topicRef,
                "lgi-tm-tree-name",
                {
                  title: t.displayName,
                  "aria-current":
                    context.state.topicRef === t.topicRef ? "page" : undefined,
                },
              )}
              {t.lifecycleState === "candidate" ? (
                <>
                  <span
                    className={cls("lgi-tm-candidate-dot")}
                    title={"候选方向"}
                  >
                    {"候选"}
                  </span>
                </>
              ) : (
                ""
              )}
              <span
                className={cls("lgi-tm-num")}
                title={String(number(t.statistics?.workCount) ?? "") + " 篇"}
              >
                {number(t.statistics?.workCount)}
              </span>
            </div>
            {children.length && expanded ? (
              <>
                <ul>{children.map((c) => branch(c, next))}</ul>
              </>
            ) : (
              ""
            )}
          </li>
        </>
      );
    }
    const pickerOptions: ReactNode[] = [];
    function optionBranch(t: T.Topic, path: string[] = []) {
      if (path.includes(t.topicRef)) return;
      const next = [...path, t.topicRef];
      pickerOptions.push(
        <>
          <option
            value={esc(t.topicRef)}
            {...attributes(
              context.state.topicRef === t.topicRef ? "selected" : "",
            )}
          >
            {["　".repeat(path.length), esc(t.displayName)].join("")}
          </option>
        </>,
      );
      topics
        .filter((c) => c.parentTopicRef === t.topicRef)
        .forEach((c) => optionBranch(c, next));
    }
    roots.forEach((t) => optionBranch(t));
    return (
      <>
        <label className={cls("lgi-tm-mobile-tree-picker lgi-tm-control")}>
          <span>{"主题层级 · 全部方向可选择"}</span>
          <select data-filter={"topicRef"}>
            <option
              value={""}
              {...attributes(!context.state.topicRef ? "selected" : "")}
            >
              {"全部方向"}
            </option>
            {pickerOptions}
          </select>
        </label>
        <div className={cls("lgi-tm-tree")} aria-label={"稳定主题结构"}>
          <div className={cls("lgi-tm-section-head")}>
            <h2 className={cls("v7-sr-only")}>{"主题列表"}</h2>
            {btn("调整归属", "move-topic", "", "lgi-tm-quiet")}
          </div>
          <div
            className={cls(
              "lgi-tm-tree-row " +
                String(!context.state.topicRef ? "is-active" : ""),
            )}
          >
            {btn("全部方向", "topic", "", "lgi-tm-tree-name")}
            <span className={cls("lgi-tm-num")}>
              {number(context.snapshot?.scope?.totalWorkCount)}
            </span>
          </div>
          <ul>
            {roots.filter((t) => !legacyConcept(t)).map((t) => branch(t))}
          </ul>
          {roots.some(legacyConcept) ? (
            <>
              <details
                className={cls("lgi-tm-core-details")}
                data-tree-legacy=""
                {...attributes(
                  context.state.legacyExpanded ||
                    roots.some(
                      (t) =>
                        legacyConcept(t) &&
                        t.topicRef === context.state.topicRef,
                    )
                    ? "open"
                    : "",
                )}
              >
                <summary>
                  {["旧方法候选 · ", roots.filter(legacyConcept).length].join(
                    "",
                  )}
                </summary>
                <p className={cls("lgi-tm-note")}>
                  {
                    "保留原声与历史归属，尚待稳定概念归纳；这些节点不代表已形成专业主题树。"
                  }
                </p>
                <ul>{roots.filter(legacyConcept).map((t) => branch(t))}</ul>
              </details>
            </>
          ) : (
            ""
          )}
          {topics.length ? (
            ""
          ) : (
            <>
              <p className={cls("lgi-tm-note")}>
                {"当前领域尚未建立主题定义，已入库材料仍然可读。"}
              </p>
            </>
          )}
          <div className={cls("lgi-tm-tree-row lgi-tm-tree-unassigned")}>
            {btn("尚未归入主题", "unassigned", "", "lgi-tm-tree-name")}
            <span className={cls("lgi-tm-num")}>
              {number(context.snapshot?.scope?.unclassifiedWorkCount)}
            </span>
          </div>
          <p className={cls("v7-sr-only")}>
            {"每个数字为去重作品，子主题之间可以重叠。"}
          </p>
        </div>
      </>
    );
  }
  function metrics(stats = context.snapshot?.statistics) {
    const platforms = observedPlatforms(stats),
      multiple = platforms.length > 1;
    const total = (
      label: string,
      value: unknown,
      unit: string,
      detail: string,
      action: string,
    ) => (
      <>
        <div className={cls("lgi-tm-total")}>
          <span className={cls("lgi-tm-metric-label")}>{label}</span>
          {action ? (
            btn(
              <>
                <strong>{readoutNumber(value)}</strong>
                <span>{unit}</span>
              </>,
              action,
              "",
              "lgi-tm-total-value",
              { "aria-label": `${label} ${number(value)} ${unit}，查看材料` },
            )
          ) : (
            <>
              <div className={cls("lgi-tm-total-value")}>
                <strong>{readoutNumber(value)}</strong>
                <span>{unit}</span>
              </div>
            </>
          )}
          <small className={cls("lgi-tm-metric-detail")}>{detail}</small>
        </div>
      </>
    );
    const rows = platforms.map((p) => (
      <>
        <tr data-platform={esc(p.platform)}>
          {multiple ? (
            <>
              <th scope={"row"}>{esc(platformName(p.platform))}</th>
            </>
          ) : (
            ""
          )}
          <td className={cls("lgi-tm-performance-value")}>
            {p.highPerformanceThreshold == null ? (
              <>
                <span className={cls("lgi-tm-unset")}>{"未设置"}</span>
              </>
            ) : (
              pct(p.highPerformanceCount, p.knownLikeCount)
            )}
          </td>
          <td>
            {p.highPerformanceThreshold == null
              ? "—"
              : readoutNumber(p.highPerformanceCount)}
            <span className={cls("lgi-tm-ratio-divider")}>{" / "}</span>
            {readoutNumber(p.knownLikeCount)}
          </td>
          <td className={cls("lgi-tm-performance-value")}>
            {readoutNumber(p.medianLikes)}
          </td>
          <td className={cls("lgi-tm-performance-value")}>
            {readoutNumber(p.p90Likes)}
          </td>
          <td>
            {readoutNumber(p.unknownLikeCount)} <small>{"篇"}</small>
          </td>
        </tr>
      </>
    ));
    return (
      <>
        <section className={cls("lgi-tm-metrics")} aria-label={"基础概览指标"}>
          <div className={cls("lgi-tm-totals")}>
            {total(
              "观察到的内容",
              stats?.workCount,
              "篇",
              `可读 ${number(context.snapshot?.scope?.readableWorkCount)} · 待归主题 ${number(context.snapshot?.scope?.unclassifiedWorkCount)}`,
              "materials",
            )}
            {total(
              "已观察创作者",
              stats?.authorCount,
              "位",
              `作者身份未知 ${number(stats?.unknownAuthorCount)} 篇`,
              "",
            )}
          </div>
          <div className={cls("lgi-tm-performance")}>
            <div className={cls("lgi-tm-performance-scroll")}>
              <table className={cls("lgi-tm-performance-table")}>
                <caption className={cls("v7-sr-only")}>
                  {[
                    multiple
                      ? "各平台"
                      : esc(platformName(platforms[0]?.platform)),
                    "样本表现；常规 / 高位表现对应点赞中位数 / P90",
                  ].join("")}
                </caption>
                <thead>
                  <tr>
                    {multiple ? (
                      <>
                        <th scope={"col"}>{"平台"}</th>
                      </>
                    ) : (
                      ""
                    )}
                    <th scope={"col"}>{"样本爆款率"}</th>
                    <th scope={"col"}>{"爆款 / 可判定"}</th>
                    <th scope={"col"}>{"点赞中位数"}</th>
                    <th scope={"col"}>{"P90 点赞"}</th>
                    <th scope={"col"}>{"点赞未知"}</th>
                  </tr>
                </thead>
                <tbody>
                  {rows || (
                    <>
                      <tr>
                        <td
                          colSpan={multiple ? 6 : 5}
                          className={cls("lgi-tm-performance-empty")}
                        >
                          {"当前范围暂无表现统计"}
                        </td>
                      </tr>
                    </>
                  )}
                </tbody>
              </table>
            </div>
            <div className={cls("lgi-tm-performance-basis")}>
              <span>
                {platforms.length
                  ? platforms
                      .map(
                        (p) =>
                          `${multiple ? esc(platformName(p.platform)) + " · " : ""}${p.highPerformanceThreshold == null ? "样本爆款规则未设置" : `样本爆款：点赞 ≥ ${number(p.highPerformanceThreshold)}`}`,
                      )
                      .join("；")
                  : "没有可判定样本，已有材料仍可阅读。"}
              </span>
              {btn(
                "规则",
                "rules",
                "",
                "lgi-tm-quiet",
                'aria-label="查看或调整样本爆款规则"',
              )}
            </div>
          </div>
        </section>
      </>
    );
  }
  function coverage() {
    const activeSnapshot = context.requireSnapshot();
    const s = context.snapshot?.scope;
    return (
      <>
        <div className={cls("lgi-tm-boundary")}>
          <span>
            {esc(
              context.snapshot?.sourceBoundary ||
                "统计引用后端读模型；来源缺口与研究判断分别表达。",
            )}
          </span>
          {context.snapshot?.scope?.readLimit ? (
            <>
              <span>
                {[
                  "读取上限 ",
                  number(activeSnapshot.scope.readLimit),
                  " · 当前返回 ",
                  list(activeSnapshot.works).length,
                  " 篇",
                ].join("")}
              </span>
            </>
          ) : (
            ""
          )}
          {s && s.totalWorkCount > list(activeSnapshot.works).length ? (
            <>
              <span>{"作品列表为部分读取；基础总量采用服务端统计。"}</span>
            </>
          ) : (
            ""
          )}
        </div>
      </>
    );
  }
  function overview() {
    return (
      <>
        <div className={cls("lgi-tm-view-toolbar")}>
          <nav className={cls("lgi-tm-work-tabs")} aria-label={"概览工作视角"}>
            {context.views.map(([id, name]) =>
              tab(name, "tab", id, context.state.tab === id),
            )}
          </nav>
          {btn("主题 × 旅程", "matrix", "", "lgi-tm-quiet")}
        </div>
        {context.state.tab === "content" ? (
          <div className={cls("lgi-tm-material-first")}>{materialWorkspace()}</div>
        ) : (
          <>
            {changesBar()}
            {context.state.tab === "structure"
              ? structure()
              : context.state.tab === "patterns"
                ? patterns()
                : context.state.tab === "sources"
                  ? sources()
                  : context.state.tab === "candidates"
                    ? candidates()
                    : actionView()}
          </>
        )}
      </>
    );
  }
  function children() {
    return list(context.snapshot?.topics)
      .filter((t) => t.lifecycleState !== "superseded")
      .filter((t) =>
        context.state.topicRef
          ? t.parentTopicRef === context.state.topicRef
          : !t.parentTopicRef,
      );
  }
  function plotPlatform(
    stats = context.topic(context.state.topicRef)?.statistics ||
      context.requireSnapshot().statistics,
  ) {
    const ids = observedPlatforms(stats).map((p) => p.platform);
    return ids.includes(context.state.plotPlatform)
      ? context.state.plotPlatform
      : ids.includes(context.state.platform)
        ? context.state.platform
        : ids[0] || "";
  }
  function plotSelect(
    stats = context.topic(context.state.topicRef)?.statistics ||
      context.requireSnapshot().statistics,
  ) {
    const activeSnapshot = context.requireSnapshot();
    const id = plotPlatform(stats),
      platforms = observedPlatforms(
        stats ||
          context.topic(context.state.topicRef)?.statistics ||
          activeSnapshot.statistics,
      );
    if (platforms.length < 2)
      return platforms.length ? (
        <>
          <span className={cls("lgi-tm-platform-context")}>
            {esc(platformName(id))}
          </span>
        </>
      ) : (
        ""
      );
    return (
      <>
        <label className={cls("lgi-tm-control")}>
          <span>{"表现图平台"}</span>
          <select data-filter={"plotPlatform"}>
            {platforms.map((p) => (
              <>
                <option
                  value={esc(p.platform)}
                  {...attributes(p.platform === id ? "selected" : "")}
                >
                  {esc(platformName(p.platform))}
                </option>
              </>
            ))}
          </select>
        </label>
      </>
    );
  }
  function changeRows() {
    return list(context.snapshot?.changes).filter(
      (c) =>
        c.structureChanged || c.notViewed || list(c.unviewedWorkRefs).length,
    );
  }
  function changesBar() {
    const changes = changeRows();
    if (!changes.length && !context.state.changesOnly) return null;
    return (
      <>
        <div className={cls("lgi-tm-changes-bar")}>
          <div>
            <strong>
              {changes.length
                ? `${changes.length} 个主题的结构或依据变化可查看`
                : "当前没有尚未查看的结构或依据变化"}
            </strong>
            <span className={cls("v7-sr-only")}>{" · 材料与分类变化，不直接解释为市场变化"}</span>
          </div>
          <div className={cls("lgi-tm-row")}>
            {tab("全部方向", "changes-mode", "all", !context.state.changesOnly)}
            {tab(
              "查看变化",
              "changes-mode",
              "changes",
              context.state.changesOnly,
            )}
            {btn("查看变更记录", "changes", "", "lgi-tm-quiet")}
          </div>
        </div>
      </>
    );
  }
  function structure() {
    const allRows = children(),
      changed = new Set(changeRows().map((c) => c.topicRef));
    const rows = allRows.filter(
      (t) => !context.state.changesOnly || changed.has(t.topicRef),
    );
    return (
      <>
        {allRows.length ? (
          <>
            <div className={cls("lgi-tm-section-head")}>
              <div>
                <h3>{"主题供给分布"}</h3>
                <p>{"点名称下钻，勾选最多三个方向并排比较。"}</p>
              </div>
              <div className={cls("lgi-tm-overview-tools")}>
                {tab(
                  "表格",
                  "summary-chart",
                  "table",
                  context.state.summaryChart !== "scatter",
                )}
                {tab(
                  "分布图",
                  "summary-chart",
                  "scatter",
                  context.state.summaryChart === "scatter",
                )}
                {btn(
                  "并排比较",
                  "compare",
                  "",
                  "",
                  list(context.state.compare).length
                    ? ""
                    : 'disabled title="先勾选方向"',
                )}
              </div>
            </div>
            {rows.length
              ? context.state.summaryChart === "scatter"
                ? topicScatter(rows)
                : structureTable(rows)
              : empty(
                  "当前子方向没有对应的未读变化",
                  "已有统计与材料继续可看，可切回全部方向。",
                )}
            <p className={cls("lgi-tm-note")}>
              {
                "子主题可以重叠，父主题按作品去重；比例描述已观察样本，不是市场份额。"
              }
            </p>
          </>
        ) : (
          <>
            <div className={cls("lgi-tm-section-head")}>
              <div>
                <h3>{"当前方向的内容与表现"}</h3>
                <p>{"叶主题继续展示完整统计与作品，不要求先启动研究。"}</p>
              </div>
            </div>
          </>
        )}
        {distribution()}
      </>
    );
  }
  function structureTable(rows: T.Topic[]) {
    const activeSnapshot = context.requireSnapshot();
    const parent =
        context.topic(context.state.topicRef)?.statistics ||
        activeSnapshot.statistics,
      max = Math.max(1, ...rows.map((t) => t.statistics?.workCount || 0));
    return (
      <>
        <div className={cls("lgi-tm-table-scroll")}>
          <table className={cls("lgi-tm-supply-table")}>
            <thead>
              <tr>
                <th>{"比较"}</th>
                <th>{"细分方向"}</th>
                <th>{"内容供给"}</th>
                <th>{"样本爆款率"}</th>
                <th>{"创作者"}</th>
                <th>{"中位赞 / P90"}</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((t) => (
                <>
                  <tr>
                    <td>
                      <input
                        type={"checkbox"}
                        data-compare={esc(t.topicRef)}
                        aria-label={"比较" + String(esc(t.displayName) ?? "")}
                        {...attributes(
                          list(context.state.compare).includes(t.topicRef)
                            ? "checked"
                            : "",
                        )}
                      />
                    </td>
                    <td>
                      {btn(
                        esc(t.displayName),
                        "topic",
                        t.topicRef,
                        "lgi-tm-title-link",
                      )}
                      <span className={cls("lgi-tm-subline")}>
                        {[
                          list(activeSnapshot.topics).filter(
                            (c) =>
                              c.parentTopicRef === t.topicRef &&
                              c.lifecycleState !== "superseded",
                          ).length
                            ? "含子方向"
                            : "细分主题",
                          legacyConcept(t)
                            ? " · 旧方法候选"
                            : t.lifecycleState === "candidate"
                              ? " · 新候选"
                              : "",
                        ].join("")}
                      </span>
                    </td>
                    <td>
                      <div className={cls("lgi-tm-countbar")}>
                        <div className={cls("lgi-tm-count-track")}>
                          <span
                            style={inlineStyle(
                              "width:" +
                                String(
                                  ((t.statistics?.workCount || 0) / max) * 100,
                                ) +
                                "%",
                            )}
                          ></span>
                        </div>
                        <b className={cls("lgi-tm-num")}>
                          {number(t.statistics?.workCount)}
                        </b>
                      </div>
                      <span className={cls("lgi-tm-subline")}>
                        {[
                          "占当前父级 ",
                          pct(t.statistics?.workCount, parent.workCount),
                        ].join("")}
                      </span>
                    </td>
                    <td>
                      {withEmpty(
                        platformStats(t.statistics).map((p) => (
                          <>
                            <p>
                              {[
                                esc(platformName(p.platform)),
                                " · ",
                                p.highPerformanceThreshold == null
                                  ? "规则未设置"
                                  : pct(
                                      p.highPerformanceCount,
                                      p.knownLikeCount,
                                    ),
                              ].join("")}
                              <span className={cls("lgi-tm-subline")}>
                                {p.highPerformanceThreshold == null
                                  ? "未判定"
                                  : `${number(p.highPerformanceCount)} / ${number(p.knownLikeCount)} 篇点赞已知`}
                              </span>
                            </p>
                          </>
                        )),
                        "表现未知",
                      )}
                    </td>
                    <td className={cls("lgi-tm-num")}>
                      {number(t.statistics?.authorCount)}
                      <span className={cls("lgi-tm-subline")}>
                        {[
                          number(t.statistics?.unknownAuthorCount),
                          " 篇作者未知",
                        ].join("")}
                      </span>
                    </td>
                    <td>
                      {withEmpty(
                        platformStats(t.statistics).map((p) => (
                          <>
                            <p>
                              {esc(platformName(p.platform))}
                              <br />
                              <span className={cls("lgi-tm-num")}>
                                {[
                                  number(p.medianLikes),
                                  " / ",
                                  number(p.p90Likes),
                                ].join("")}
                              </span>
                            </p>
                          </>
                        )),
                        "指标未知",
                      )}
                    </td>
                  </tr>
                </>
              ))}
            </tbody>
          </table>
        </div>
      </>
    );
  }
  function topicScatter(rows: T.Topic[]) {
    const platform = plotPlatform(),
      all = rows
        .map((t) => ({
          t,
          p: platformStats(t.statistics).find((p) => p.platform === platform),
        }))
        .filter((x) => x.p !== undefined && x.p.workCount > 0);
    const data = all.filter(
        (
          x,
        ): x is {
          t: T.Topic;
          p: T.PlatformStatistics & { highPerformanceCount: number };
        } =>
          x.p !== undefined &&
          known(x.p.highPerformanceCount) &&
          x.p.highPerformanceThreshold != null &&
          x.p.knownLikeCount > 0,
      ),
      unknown = all.length - data.length,
      max = Math.max(5, ...data.map((x) => x.p.workCount)) * 1.2;
    return (
      <>
        <div className={cls("lgi-tm-section-head")}>
          <p>
            {[
              "横轴：已观察作品数；纵轴：样本爆款率。仅对照同平台，不划“蓝海区”。",
              unknown
                ? `${unknown} 个主题表现不可判定，未画在0%位置，仍可从表格查看。`
                : "",
            ].join("")}
          </p>
          {plotSelect()}
        </div>
        {data.length ? (
          <>
            <div className={cls("lgi-tm-table-scroll")}>
              <svg
                className={cls("lgi-tm-scatter")}
                viewBox={"0 0 960 350"}
                role={"group"}
                aria-label={
                  String(esc(platformName(platform)) ?? "") +
                  "主题供给与表现分布"
                }
              >
                <path
                  className={cls("lgi-tm-scatter-axis")}
                  d={"M67 35V293H916"}
                ></path>
                {[0, 0.25, 0.5, 0.75, 1].map((v) => (
                  <>
                    <path
                      className={cls("lgi-tm-scatter-grid")}
                      d={"M67 " + String(293 - v * 240) + "H916"}
                    ></path>
                    <text x={"54"} y={297 - v * 240} textAnchor={"end"}>
                      {[v * 100, "%"].join("")}
                    </text>
                    <text x={67 + v * 800} y={"317"} textAnchor={"middle"}>
                      {Math.round(v * max)}
                    </text>
                  </>
                ))}
                <text x={"70"} y={"20"}>
                  {["样本爆款率 · ", esc(platformName(platform))].join("")}
                </text>
                <text x={"885"} y={"339"} textAnchor={"end"}>
                  {"已观察作品数"}
                </text>
                {data.map(({ t, p }, i) => {
                  const x = 67 + (p.workCount / max) * 800,
                    y = 293 - (p.highPerformanceCount / p.knownLikeCount) * 240;
                  return (
                    <>
                      <g
                        role={"button"}
                        tabIndex={0}
                        className={cls("lgi-tm-scatter-point")}
                        data-action={"topic"}
                        data-id={esc(t.topicRef)}
                        aria-label={
                          String(esc(t.displayName) ?? "") +
                          "，" +
                          String(p.workCount ?? "") +
                          "篇，样本爆款率" +
                          String(
                            pct(p.highPerformanceCount, p.knownLikeCount) ?? "",
                          )
                        }
                      >
                        <circle cx={x} cy={y} r={"8"}></circle>
                        <text x={x + 13} y={y + (i % 2 ? 20 : -13)}>
                          {esc(t.displayName)}
                        </text>
                        <title>
                          {[
                            esc(t.displayName),
                            " · ",
                            p.highPerformanceCount,
                            "/",
                            p.knownLikeCount,
                            " 篇可判定",
                          ].join("")}
                        </title>
                      </g>
                    </>
                  );
                })}
              </svg>
            </div>
          </>
        ) : (
          empty(
            "当前平台还没有可判定的主题表现",
            "规则未设置或点赞未知时，不把主题画在0%位置。",
          )
        )}
      </>
    );
  }
  function distribution() {
    const activeSnapshot = context.requireSnapshot();
    const stats =
        context.topic(context.state.topicRef)?.statistics ||
        activeSnapshot.statistics,
      platform = plotPlatform(stats),
      p = platformStats(stats).find((p) => p.platform === platform);
    if (!p) return empty("当前没有可绘制的点赞数据", "原始材料仍可查看。");
    const ns = context.selectedWorks().filter((w) => w.platform === platform),
      bins = (
        [
          [0, 20, "≤20赞"],
          [21, 99, "21–99赞"],
          [100, 499, "100–499赞"],
          [500, 999, "500–999赞"],
          [1000, Infinity, "≥1,000赞"],
          [null, null, "指标缺失"],
        ] as const
      ).map(([min, max, label]) => ({
        label,
        refs: ns
          .filter((w) =>
            min === null
              ? !known(w.likes)
              : known(w.likes) && w.likes >= min && w.likes <= max,
          )
          .map((w) => w.workRef),
      })),
      max = Math.max(1, ...bins.map((b) => b.refs.length));
    return (
      <>
        <section className={cls("lgi-tm-section")}>
          <div className={cls("lgi-tm-section-head")}>
            <div>
              <h3>{"表现结构"}</h3>
              <p>{"从整体分布检查常规与高位表现，不只看成功案例。"}</p>
            </div>
            {plotSelect(stats)}
          </div>
          <div className={cls("lgi-tm-distribution-layout")}>
            <div className={cls("lgi-tm-bin-list")}>
              <div className={cls("lgi-tm-section-head")}>
                <strong>
                  {[esc(platformName(platform)), " · 点赞分布"].join("")}
                </strong>
                <span>{[ns.length, " 篇当前读取作品"].join("")}</span>
              </div>
              {bins.map((b, i) => (
                <>
                  <button
                    type={"button"}
                    className={cls("lgi-tm-bin-row")}
                    data-action={"subset"}
                    data-id={esc(b.refs.join(","))}
                    {...attributes(b.refs.length ? "" : "disabled")}
                    aria-label={
                      String(b.label ?? "") +
                      "，" +
                      String(b.refs.length ?? "") +
                      "篇，查看作品"
                    }
                  >
                    <span>{b.label}</span>
                    <span className={cls("lgi-tm-bin-track")}>
                      <i
                        className={cls(
                          "lgi-tm-bin-fill " +
                            String(i === 5 ? "is-missing" : ""),
                        )}
                        style={inlineStyle(
                          "width:" + String((b.refs.length / max) * 100) + "%",
                        )}
                      ></i>
                    </span>
                    <b className={cls("lgi-tm-num")}>{b.refs.length}</b>
                  </button>
                </>
              ))}
            </div>
            <aside className={cls("lgi-tm-dist-aside")}>
              <h4>{"常规表现与高位表现，相差多少？"}</h4>
              <div className={cls("lgi-tm-row")}>
                {badge(`中位赞 ${number(p.medianLikes)}`)}
                {badge(`P90 ${number(p.p90Likes)}`)}
                {badge(
                  known(p.medianLikes) && p.medianLikes > 0 && known(p.p90Likes)
                    ? `${(p.p90Likes / p.medianLikes).toFixed(1)} 倍`
                    : "倍数未判定",
                )}
              </div>
              <p>
                {
                  "中位数描述样本中间位置，P90描述较高位置。比值用于检查分布差异，不预测再次发布的效果。"
                }
              </p>
              <p className={cls("lgi-tm-note")}>
                {[
                  "读数引用后端 ",
                  number(p.workCount),
                  " 篇统计范围；横条只描述当前返回的 ",
                  ns.length,
                  " 篇作品。",
                  number(p.unknownLikeCount),
                  " 篇点赞未知不计为0，低互动不等于内容失败。",
                ].join("")}
              </p>
              {btn("检查高表现参考来自谁", "tab", "sources", "lgi-tm-quiet")}
            </aside>
          </div>
        </section>
      </>
    );
  }
  function topicBoundary(
    t: T.Topic | undefined | null,
    source = context.snapshot,
  ) {
    if (!t) return "";
    if (t.core?.sourceState === "source_unavailable")
      return (
        <>
          <section
            className={cls("lgi-tm-topic-boundary")}
            aria-label={"主题边界来源受限"}
          >
            <h3>{"主题定义的来源当前受限"}</h3>
            <p>
              {
                "纳入、排除条件与定义关系暂不可展示。已有当前可读材料仍可核对，不能把来源受限解释为边界尚未定义。"
              }
            </p>
          </section>
        </>
      );
    const core = t.core,
      inclusion = list(core?.inclusionCriteria).filter(
        (v) => typeof v === "string" && v.trim(),
      ),
      exclusion = list(core?.exclusionCriteria).filter(
        (v) => typeof v === "string" && v.trim(),
      );
    const origin =
      Object.assign({
        machine_induced: "机器提炼的定义",
        manual: "人工维护的定义",
        legacy: "历史定义 · 边界未补齐",
      } as Record<string, string>)[core?.definitionSource || ""] ||
      (core ? "定义来源尚未取得" : "历史定义 · 边界未补齐");
    return (
      <>
        <section
          className={cls("lgi-tm-topic-boundary")}
          aria-label={"主题纳入与排除边界"}
        >
          <div className={cls("lgi-tm-section-head")}>
            <h3>{"这个主题包含什么？"}</h3>
            {badge(origin)}
          </div>
          <div className={cls("lgi-tm-criteria")}>
            <div>
              <h4>{"纳入条件"}</h4>
              {inclusion.length ? (
                <>
                  <ul>
                    {inclusion.map((v) => (
                      <>
                        <li>{esc(v)}</li>
                      </>
                    ))}
                  </ul>
                </>
              ) : (
                <>
                  <p className={cls("lgi-tm-note")}>
                    {"纳入边界尚未补齐，已有定义与材料继续可读。"}
                  </p>
                </>
              )}
            </div>
            <div>
              <h4>{"排除条件"}</h4>
              {exclusion.length ? (
                <>
                  <ul>
                    {exclusion.map((v) => (
                      <>
                        <li>{esc(v)}</li>
                      </>
                    ))}
                  </ul>
                </>
              ) : (
                <>
                  <p className={cls("lgi-tm-note")}>
                    {"排除边界尚未补齐，不能据名称认定范围。"}
                  </p>
                </>
              )}
            </div>
          </div>
          {core ? (
            <>
              <p className={cls("lgi-tm-note")}>
                {[
                  "当前范围 ",
                  number(core.discussionCount),
                  " 条独立讨论 · 支持 ",
                  number(core.evidenceRoles?.support),
                  " · 反例 ",
                  number(core.evidenceRoles?.challenge),
                  " · 背景 ",
                  number(core.evidenceRoles?.context),
                  "。按讨论计数，不代表作品数或人数；归入同一主题可以保留不同观点。",
                ].join("")}
              </p>
            </>
          ) : (
            ""
          )}
          {relationDetails(core?.relations, source, "本主题")}
        </section>
      </>
    );
  }
  function relationDetails(
    relations: T.Relation[] | undefined,
    source = context.snapshot,
    subject = "此讨论",
  ) {
    const rows = list(relations).filter((r) => r && typeof r === "object");
    if (!rows.length) return "";
    const names: Record<string, string> = {
      equivalent: "含义相同",
      broader: `${subject}范围更宽`,
      narrower: `${subject}范围更细`,
      related: "相互关联",
      distinct: "边界不同",
      uncertain: "关系未确定",
    };
    return (
      <>
        <details className={cls("lgi-tm-core-details")}>
          <summary>{["与已有主题的关系 · ", rows.length].join("")}</summary>
          {rows.map((r) => {
            const t = list(source?.topics).find(
                (t) => t.topicRef === r.topicRef,
              ),
              name = t?.displayName || r.displayName || r.label || "已有主题";
            return (
              <>
                <p>
                  {r.topicRef
                    ? btn(esc(name), "topic", r.topicRef, "lgi-tm-title-link")
                    : esc(name)}
                  {[" · ", esc(names[r.relation] || "关系未确定")].join("")}
                  {r.reason ? (
                    <>
                      <span className={cls("lgi-tm-subline")}>
                        {esc(r.reason)}
                      </span>
                    </>
                  ) : (
                    ""
                  )}
                </p>
              </>
            );
          })}
          <p className={cls("lgi-tm-note")}>
            {"研究关系供核对，主题树按已保存的归属显示。"}
          </p>
        </details>
      </>
    );
  }
  function discussionGroups(
    ns: T.Work[],
    source = context.dialogState?.snapshot || context.snapshot,
  ) {
    const groups = new Map<string, T.DiscussionInternalGroup>();
    ns.forEach((w) =>
      discussionUnits(w).forEach((d) =>
        (d.assignments.length ? d.assignments : [null]).forEach((a) => {
          const key = a
              ? assignmentKey(a)
              : JSON.stringify(["unassigned", w.workRef, d.unitKey]),
            t = list(source?.topics).find(
              (t) => a?.topicRef && t.topicRef === a.topicRef,
            );
          if (!groups.has(key))
            groups.set(key, {
              key,
              topicRef: a?.topicRef || "",
              label:
                t?.displayName ||
                a?.label ||
                d.label ||
                d.statement ||
                "尚未命名的讨论",
              canonical: Boolean(a),
              rows: [],
              seen: new Set(),
            });
          const group = groups.get(key);
          if (!group) return;
          const rowKey = JSON.stringify([w.workRef, d.unitKey]);
          if (group.seen.has(rowKey)) return;
          group.seen.add(rowKey);
          group.rows.push({ work: w, discussion: d, assignment: a });
        }),
      ),
    );
    return [...groups.values()].map((g) => ({
      ...g,
      works: [...new Map(g.rows.map((r) => [r.work.workRef, r.work])).values()],
      evidenceCount: new Set(
        g.rows.flatMap((r) =>
          locatedCitations(r.work, r.discussion.evidence).map((c) =>
            JSON.stringify([r.work.workRef, c.fragmentId, c.start, c.end]),
          ),
        ),
      ).size,
    }));
  }
  function conceptDecisionDetails(d: T.Discussion, source: T.Snapshot | null) {
    if (!["new", "matched"].includes(d.status || "")) return "";
    const p = d.proposedTopic,
      h = d.hierarchy;
    const parent = list(source?.topics).find(
      (t) =>
        t.topicRef === h?.parentTopicRef &&
        t.definitionRef === h?.parentDefinitionRef &&
        t.core?.sourceState !== "source_unavailable",
    );
    return (
      <>
        {p ? (
          <>
            <p className={cls("lgi-tm-note")}>
              {["领域相关性：", esc(p.domainReason || "尚未记录")].join("")}
            </p>
            <p className={cls("lgi-tm-note")}>
              {["主题抽象依据：", esc(p.abstractionReason || "尚未记录")].join(
                "",
              )}
            </p>
          </>
        ) : (
          ""
        )}
        {parent ? (
          <>
            <p className={cls("lgi-tm-note")}>
              {"归入上位主题："}
              {btn(
                esc(parent.displayName),
                "topic",
                parent.topicRef,
                "lgi-tm-title-link",
              )}
              <span className={cls("lgi-tm-subline")}>
                {["包含依据：", esc(h?.reason || "尚未记录")].join("")}
              </span>
            </p>
          </>
        ) : p?.parent?.kind === "root" ? (
          <>
            <p className={cls("lgi-tm-note")}>
              {[
                "保留为根方向的依据：",
                esc(p.parent.reason || "尚未记录"),
              ].join("")}
            </p>
          </>
        ) : (
          ""
        )}
      </>
    );
  }
  function discussionUnitView(
    row: Omit<T.DiscussionRow, "assignment">,
    reader: { raw: string } | null = null,
  ) {
    const w = row.work,
      d = row.discussion,
      source = context.dialogState?.snapshot || context.snapshot;
    const speaker =
      Object.assign({
        author: "作者表达",
        commenter: "评论者原声",
        quoted: "引用或转述",
        unknown: "来源角色未知",
      } as Record<string, string>)[d.speakerRole || ""] || "来源角色未知";
    const role =
      Object.assign({
        support: "支持材料",
        challenge: "反例或不同经验",
        context: "背景材料",
      } as Record<string, string>)[d.evidenceRole || ""] || "材料作用未判定";
    const status = d.legacy
      ? "旧研究 · 边界未补齐"
      : Object.assign({
          matched: "已判断归属",
          new: "新方向候选",
          uncertain: "归属尚未确定",
          out_of_scope: "当前领域外",
        } as Record<string, string>)[d.status || ""] || "归属尚未确定";
    const citations = locatedCitations(w, d.evidence),
      quote = citationText(w, d.evidence);
    return (
      <>
        <section className={cls("lgi-tm-discussion-unit")}>
          <div className={cls("lgi-tm-row")}>
            {badge(speaker)}
            {badge(role)}
            {badge(status)}
          </div>
          <h4>{esc(d.label || "具体讨论")}</h4>
          {d.statement ? (
            <>
              <p>{esc(d.statement)}</p>
            </>
          ) : (
            ""
          )}
          {d.rationale ? (
            <>
              <p className={cls("lgi-tm-note")}>
                {["材料依据：", esc(d.rationale)].join("")}
              </p>
            </>
          ) : (
            ""
          )}
          {d.assignments.length ? (
            <>
              <div className={cls("lgi-tm-unit-assignments")}>
                {d.assignments.map((a) => {
                  const t = list(source?.topics).find(
                    (t) => t.topicRef === a.topicRef,
                  );
                  return (
                    <>
                      <p>
                        {"归属主题："}
                        {a.topicRef
                          ? btn(
                              esc(t?.displayName || a.label || "已有主题"),
                              "topic",
                              a.topicRef,
                              "lgi-tm-title-link",
                            )
                          : esc(a.label || "已有主题")}
                        <span className={cls("lgi-tm-subline")}>
                          {[
                            "归属理由：",
                            esc(
                              a.reason ||
                                (d.legacy
                                  ? "旧研究未记录具体归属理由。"
                                  : "归属理由尚未取得。"),
                            ),
                          ].join("")}
                        </span>
                      </p>
                    </>
                  );
                })}
                {d.assignments.length > 1 ? (
                  <>
                    <p className={cls("lgi-tm-note")}>
                      {"同一条讨论涉及多个主题，讨论总量只计一次。"}
                    </p>
                  </>
                ) : (
                  ""
                )}
              </div>
            </>
          ) : d.legacy ? (
            <>
              <p className={cls("lgi-tm-note")}>
                {"尚无可核对的主题身份，同名讨论分别保留。"}
              </p>
            </>
          ) : (
            <>
              <p className={cls("lgi-tm-note")}>
                {"尚未归入主题，已有讨论与证据保留。"}
              </p>
            </>
          )}
          {d.reason ? (
            <>
              <p className={cls("lgi-tm-note")}>
                {["判断说明：", esc(d.reason)].join("")}
              </p>
            </>
          ) : (
            ""
          )}
          {conceptDecisionDetails(d, source)}
          {quote ? (
            <>
              <p className={cls("lgi-tm-note")}>
                {[
                  "原文片段 · ",
                  esc(
                    sourceKind(
                      citations[0]?.fragment.sourceKind ||
                        citations[0]?.fragment.field,
                    ),
                  ),
                ].join("")}
              </p>
              <blockquote>{esc(quote)}</blockquote>
            </>
          ) : (
            <>
              <p className={cls("lgi-tm-note")}>
                {"当前没有可展示的精确引用片段，可继续核对原作。"}
              </p>
            </>
          )}
          {reader ? (
            <>
              <div className={cls("lgi-tm-row")}>
                {citations.map((c) => (
                  <>
                    <a
                      className={cls("lgi-tm-button lgi-tm-quiet")}
                      href={
                        "#" +
                        String(
                          encodeURIComponent(
                            context.readerCitationTarget(w, c, reader.raw),
                          ) ?? "",
                        )
                      }
                    >
                      {[
                        esc(
                          sourceKind(c.fragment.sourceKind || c.fragment.field),
                        ),
                        " · 第 ",
                        number(c.start + 1),
                        "–",
                        number(c.end),
                        " 字",
                      ].join("")}
                    </a>
                  </>
                ))}
              </div>
            </>
          ) : (
            btn(
              `${esc(title(w))} · ${readLabel(w, "核对原文")}${citations.length ? `与 ${citations.length} 条依据` : ""}`,
              "reader",
              w.workRef,
              "lgi-tm-quiet",
            )
          )}
          {relationDetails(d.relations, source)}
        </section>
      </>
    );
  }
  function discussionGroupDetails(g: T.DiscussionGroup) {
    return (
      <>
        <details className={cls("lgi-tm-core-details")}>
          <summary>
            {["查看 ", g.rows.length, " 条具体讨论、来源与归属依据"].join("")}
          </summary>
          {g.rows.map((r) => discussionUnitView(r))}
        </details>
      </>
    );
  }
  function patterns() {
    const ns = context.selectedWorks(),
      groups = discussionGroups(ns),
      tasks = context
        .anglesFrom(ns)
        .filter((angle) => angle.answerTask)
        .map((angle) => ({
          work: ns.find((w) => w.workRef === angle.workRef),
          angle,
        }));
    return (
      <>
        <div className={cls("lgi-tm-section-head")}>
          <div>
            <h3>{"这些内容，实际在怎样讲？"}</h3>
            <p>
              {
                "当前研究保留具体讨论、场景与回答任务；经验自述、方法说明等讲法分类尚未形成，不用视频或图文形态替代。"
              }
            </p>
            {researchCoverageSummary(ns)}
          </div>
        </div>
        <div className={cls("lgi-tm-pattern-grid")}>
          {withEmpty(
            groups.map((g) => (
              <>
                <article className={cls("lgi-tm-panel")}>
                  <span className={cls("lgi-tm-eyebrow")}>
                    {g.canonical
                      ? "按已保存主题身份归组"
                      : "归属待核对的独立讨论"}
                  </span>
                  <h4>{esc(g.label)}</h4>
                  <p>
                    {[
                      g.rows.length,
                      " 条独立讨论 · ",
                      g.works.length,
                      " 篇当前读取作品 · ",
                      g.evidenceCount,
                      " 条去重定位依据",
                    ].join("")}
                  </p>
                  {discussionGroupDetails(g)}
                  {btn(
                    "查看这些作品",
                    "subset",
                    g.works.map((w) => w.workRef).join(","),
                    "lgi-tm-quiet",
                  )}
                </article>
              </>
            )),
            empty(
              "讲法分类与具体讨论尚未形成",
              "原文仍然可读；不推断未研究作品的表达方式。",
            ),
          )}
        </div>
        <section className={cls("lgi-tm-section")}>
          <h3>{"哪些具体问题已形成回答任务？"}</h3>
          <div className={cls("lgi-tm-pattern-grid")}>
            {withEmpty(
              tasks.slice(0, 6).map(({ angle }) => (
                <>
                  <article className={cls("lgi-tm-panel")}>
                    <h4>{esc(angle.label)}</h4>
                    <p>{esc(angle.answerTask)}</p>
                    {comparisonScopeNote(angle)}
                    {btn(
                      "核对原回答与依据",
                      "reader",
                      angle.workRef,
                      "lgi-tm-quiet",
                    )}
                  </article>
                </>
              )),
              empty(
                "当前尚无可引用的回答任务",
                "不能把用户评论问题自动当作原作已有回答。",
              ),
            )}
          </div>
        </section>
        <section className={cls("lgi-tm-section")}>
          <h3>{"具体讨论与场景"}</h3>
          {sceneList(ns)}
        </section>
      </>
    );
  }
  function sceneGroups(ns: T.Work[], queryText = "") {
    const groups = new Map<string, T.SceneGroup>();
    ns.forEach((w) =>
      list(w.research?.output?.scenes).forEach((scene) => {
        if (typeof scene.label !== "string" || !scene.label) return;
        if (!groups.has(scene.label))
          groups.set(scene.label, {
            label: scene.label,
            works: new Map(),
            citations: new Set(),
            excerpt: "",
          });
        const group = groups.get(scene.label);
        if (!group) return;
        list(scene.evidence).forEach((c) => {
          const f = list(w.research?.fragments).find(
              (f) => f.fragmentId === c.fragmentId,
            ),
            owner = f ? fragmentWorkRef(w, f) : w.workRef;
          if (owner)
            group.works.set(
              owner,
              ns.find((row) => row.workRef === owner) || context.work(owner),
            );
          else group.unknownOwner = true;
          group.citations.add(
            JSON.stringify([
              owner,
              f?.sourceRef || c.fragmentId,
              f?.field,
              c.start,
              c.end,
            ]),
          );
          if (!group.excerpt) group.excerpt = citationText(w, [c]);
        });
      }),
    );
    const q = normalizeSearch(queryText);
    return [...groups.values()].filter(
      (g) =>
        !q ||
        normalizeSearch(
          [
            g.label,
            g.excerpt,
            ...[...g.works.values()].map((w) => (w ? title(w) : "")),
          ].join(" "),
        ).includes(q),
    );
  }
  function sceneList(ns: T.Work[], queryText = "", limit = Infinity) {
    const groups = sceneGroups(ns, queryText);
    return groups.length ? (
      <>
        <div className={cls("lgi-tm-scene-grid")}>
          {groups.slice(0, limit).map((group) => (
            <>
              <article className={cls("lgi-tm-panel lgi-tm-scene-card")}>
                <h4 title={esc(group.label)}>{esc(group.label)}</h4>
                <p>
                  {[
                    group.works.size,
                    " 篇",
                    group.unknownOwner ? "已知" : "独立",
                    "来源作品 · ",
                    group.citations.size,
                    " 条去重定位依据",
                  ].join("")}
                </p>
                {group.excerpt ? (
                  <>
                    <blockquote>{esc(group.excerpt)}</blockquote>
                  </>
                ) : (
                  ""
                )}
                {group.unknownOwner ? (
                  <>
                    <p className={cls("lgi-tm-note")}>
                      {"部分原文所属作品未知，未计入作品数量。"}
                    </p>
                  </>
                ) : (
                  ""
                )}
                <details className={cls("lgi-tm-scene-sources")}>
                  <summary>{"阅读来源与上下文"}</summary>
                  <div className={cls("lgi-tm-row")}>
                    {[...group.works.values()].map((w) =>
                      w ? (
                        btn(esc(title(w)), "reader", w.workRef, "lgi-tm-quiet")
                      ) : (
                        <>
                          <span>{"比较中的另一篇作品（当前列表未展开）"}</span>
                        </>
                      ),
                    )}
                  </div>
                </details>
              </article>
            </>
          ))}
        </div>
      </>
    ) : (
      empty(
        "当前材料尚未形成可引用的场景分组",
        "阶段尚未确定不影响原作与已有评论的阅读。",
      )
    );
  }
  function commentFragmentCount(ns: T.Work[]) {
    return new Set(
      ns.flatMap((w) =>
        list(w.research?.fragments)
          .filter((f) => String(f.field).endsWith("comment"))
          .map((f) =>
            JSON.stringify([
              fragmentWorkRef(w, f),
              f.sourceRef || f.fragmentId,
              f.field,
              f.start,
              f.end,
            ]),
          ),
      ),
    ).size;
  }
  function sources() {
    const activeSnapshot = context.requireSnapshot();
    const rows = list(activeSnapshot.sources)
        .slice()
        .sort((a, b) => b.workCount - a.workCount),
      top3 = rows
        .filter((r) => r.authorExternalId)
        .slice(0, 3)
        .reduce((n, r) => n + r.workCount, 0),
      ps = platformStats(activeSnapshot.statistics),
      configured = ps.filter((p) => p.highPerformanceThreshold != null),
      high = configured.reduce(
        (n, p) =>
          n + (known(p.highPerformanceCount) ? p.highPerformanceCount : 0),
        0,
      ),
      highSources = rows.filter(
        (r) =>
          r.authorExternalId &&
          platformStats(r.statistics).some(
            (p) => known(p.highPerformanceCount) && p.highPerformanceCount > 0,
          ),
      ).length;
    return (
      <>
        <div className={cls("lgi-tm-section-head")}>
          <div>
            <h3>{"这些材料主要来自谁？"}</h3>
            <p>{"作者贡献引用当前服务端统计范围，作品按身份去重。"}</p>
          </div>
          {btn("我方与对标范围", "identity")}
        </div>
        <div className={cls("lgi-tm-source-summary")}>
          <div>
            <span>{"内容来源"}</span>
            <strong>{number(activeSnapshot.statistics.authorCount)}</strong>
            <span>
              {[
                "位已观察创作者 · ",
                number(activeSnapshot.statistics.unknownAuthorCount),
                " 篇作者未知",
              ].join("")}
            </span>
          </div>
          <div>
            <span>{"前三位已知来源贡献"}</span>
            <strong>{pct(top3, activeSnapshot.statistics.workCount)}</strong>
            <span>
              {[
                top3,
                " / ",
                number(activeSnapshot.statistics.workCount),
                " 篇当前样本",
              ].join("")}
            </span>
          </div>
          <div>
            <span>{"样本高表现参考"}</span>
            <strong>{configured.length ? number(high) : "规则未设置"}</strong>
            <span>
              {configured.length
                ? `涉及 ${highSources} 位已知作者 · ${configured.length < ps.length ? "部分平台已设规则" : "按各平台已设规则"}`
                : "不擅自判定高表现"}
            </span>
          </div>
        </div>
        <div className={cls("lgi-tm-table-scroll")}>
          <table>
            <thead>
              <tr>
                <th>{"创作者"}</th>
                <th>{"作品 / 样本占比"}</th>
                <th>{"主要方向"}</th>
                <th>{"样本表现 / 可判定分母"}</th>
                <th>{"作品"}</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((r) => (
                <>
                  <tr>
                    <td>
                      <div className={cls("lgi-tm-source-who")}>
                        {avatar(r.creatorDisplayName)}
                        <div>
                          <strong>
                            {esc(r.creatorDisplayName || "作者未知")}
                          </strong>
                          <p>{esc(platformName(r.platform))}</p>
                        </div>
                      </div>
                    </td>
                    <td>
                      <b className={cls("lgi-tm-num")}>{number(r.workCount)}</b>
                      {[
                        " 篇 · ",
                        pct(r.workCount, activeSnapshot.statistics.workCount),
                      ].join("")}
                    </td>
                    <td>
                      {list(r.topicRefs)
                        .map((ref) => esc(context.topicName(ref)))
                        .join("、") || "尚未归主题"}
                    </td>
                    <td>
                      {platformStats(r.statistics).map((p) => (
                        <>
                          <p>
                            {p.highPerformanceThreshold == null
                              ? "样本规则未设置"
                              : `${number(p.highPerformanceCount)} / ${number(p.knownLikeCount)} · ${pct(p.highPerformanceCount, p.knownLikeCount)}`}
                            <br />
                            {[number(p.unknownLikeCount), " 篇点赞未知"].join(
                              "",
                            )}
                          </p>
                        </>
                      ))}
                    </td>
                    <td>
                      {btn(
                        "查看作品",
                        "subset",
                        list(r.workRefs).join(","),
                        "lgi-tm-quiet",
                      )}
                    </td>
                  </tr>
                </>
              ))}
            </tbody>
          </table>
        </div>
        {rows.length ? "" : empty("当前范围没有作者作品")}
        <p className={cls("lgi-tm-note")}>
          {
            "作品可由多条采集路径取得，总量只计一次。不据来源集中程度判断作者能力。"
          }
        </p>
        {materialsSection()}
      </>
    );
  }
  function candidates() {
    const activeSnapshot = context.requireSnapshot();
    const rows = list(context.snapshot?.topics).filter(
      (t) => t.lifecycleState === "candidate",
    );
    return (
      <>
        <div className={cls("lgi-tm-section-head")}>
          <div>
            <h3>{"新方向与尚未归类的材料"}</h3>
            <p>{"候选有依据即可查看；新增结构不等于市场增长。"}</p>
          </div>
          {btn("结构与依据变更", "changes")}
          {btn("结构调整", "structure-change")}
        </div>
        {rows.length
          ? rows.map((t) => (
              <>
                <article className={cls("lgi-tm-candidate")}>
                  {badge("新方向候选", true)}
                  <h4>{esc(t.displayName)}</h4>
                  <p>{esc(t.definitionText)}</p>
                  <p>
                    {[
                      number(t.statistics?.workCount),
                      " 篇当前材料 · 定义 v",
                      esc(t.definitionVersion),
                    ].join("")}
                  </p>
                  <div className={cls("lgi-tm-row")}>
                    {btn("在概览中查看", "topic", t.topicRef)}
                    {btn("判断这个方向", "judge", t.topicRef, "lgi-tm-quiet")}
                  </div>
                </article>
              </>
            ))
          : empty(
              "当前没有新的候选方向",
              "已有主题和材料继续可用；没有新方向不表示观察完整。",
            )}
        {list(activeSnapshot.candidates).map((c) => (
          <>
            <article className={cls("lgi-tm-candidate")}>
              {badge("研究提出的候选方向", true)}
              <h4>{esc(c.label)}</h4>
              <p>
                {[
                  "由具体作品与 ",
                  list(c.evidence).length,
                  " 条引用提出，尚未正式发布定义。",
                ].join("")}
              </p>
              {btn("核对候选依据", "reader", c.workRef, "lgi-tm-quiet")}
            </article>
          </>
        ))}
        <section className={cls("lgi-tm-section")}>
          <h3>{"尚未被主题结构解释的材料"}</h3>
          <p>
            {[
              number(activeSnapshot.scope.unclassifiedWorkCount),
              " 篇待归主题；已有材料仍然可读。",
            ].join("")}
          </p>
          <div className={cls("lgi-tm-material-grid")}>
            {context
              .selectedWorks()
              .filter((w) => !list(w.topicRefs).length)
              .slice(0, 8)
              .map(card)}
          </div>
          {btn("查看未归类材料", "unassigned", "", "lgi-tm-quiet")}
        </section>
      </>
    );
  }
  function actionView() {
    const activeSnapshot = context.requireSnapshot();
    const stats =
      context.topic(context.state.topicRef)?.statistics ||
      activeSnapshot.statistics;
    const selectedTopic = context.topic(context.state.topicRef);
    const rows = children().length
      ? children()
      : selectedTopic
        ? [selectedTopic]
        : list(activeSnapshot.topics).filter(
            (t) => t.lifecycleState !== "superseded",
          );
    return (
      <>
        <div className={cls("lgi-tm-section-head")}>
          <div>
            <h3>{"从熟悉的方向，找到下一次切入"}</h3>
            <p>{"我方发布历史不随概览或外部近期窗口清空。"}</p>
          </div>
          {select("referenceWindowDays", "外部参考窗口", [
            ["7", "近7天"],
            ["30", "近30天"],
            ["90", "近90天"],
            ["0", "全部历史"],
          ])}
        </div>
        <div className={cls("lgi-tm-callout")}>
          {activeSnapshot.scope.ownIdentityState === "unknown"
            ? "我方矩阵尚未设置，不能据此判断整个账号矩阵没做过。"
            : `${number(activeSnapshot.scope.ownHistoryPublishedCount)} 篇我方已确认发布 · ${number(stats.ownBreakoutCount)} 篇手动爆款 · ${number(activeSnapshot.scope.ownHistoryPublicationUnknownCount)} 篇发布事实未知`}
        </div>
        <div className={cls("lgi-tm-table-scroll")}>
          <table>
            <thead>
              <tr>
                <th>{"主题与具体讨论"}</th>
                <th>{"我方发布覆盖 / 全部历史"}</th>
                <th>{"近期外部低粉高赞"}</th>
                <th>{"为什么打开"}</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((t) => (
                <>
                  <tr>
                    <td>
                      {btn(
                        esc(t.displayName),
                        "topic",
                        t.topicRef,
                        "lgi-tm-title-link",
                      )}
                      <p>{esc(t.definitionText)}</p>
                    </td>
                    <td>
                      {activeSnapshot.scope.ownIdentityState === "unknown" ? (
                        "我方范围未设置"
                      ) : (
                        <>
                          {[
                            number(t.statistics?.ownHistoryPublishedCount),
                            " 篇已发布",
                          ].join("")}
                          <br />
                          {[
                            number(t.statistics?.ownHistoryBreakoutCount),
                            " 篇手动爆款",
                          ].join("")}
                        </>
                      )}
                    </td>
                    <td>
                      {[
                        number(t.statistics?.recentLowFollowerHighLikeCount),
                        " 篇独立外部作品",
                      ].join("")}
                      <p className={cls("lgi-tm-note")}>
                        {"粉丝 ≤1,000 且点赞 ≥500，粉丝未知不计入"}
                      </p>
                    </td>
                    <td>
                      <p>
                        {
                          "先查看我方原作、外部参考与具体讨论，不把做过当作做完。"
                        }
                      </p>
                      {btn("判断这个方向", "judge", t.topicRef, "lgi-tm-quiet")}
                    </td>
                  </tr>
                </>
              ))}
            </tbody>
          </table>
        </div>
      </>
    );
  }
  function sortedMaterials(ns: T.Work[]) {
    const filtered = ns.filter((w) =>
      matchesWork(
        w,
        context.state.view === "journey" || context.state.tab === "content"
          ? context.state.query
          : "",
      ),
    );
    if (context.state.materialMode === "high")
      return filtered
        .filter((w) => {
          const p = platformStats(context.snapshot?.statistics).find(
            (p) => p.platform === w.platform,
          );
          return (
            known(p?.highPerformanceThreshold) &&
            known(w.likes) &&
            w.likes >= p.highPerformanceThreshold
          );
        })
        .sort(
          (a, b) =>
            a.platform.localeCompare(b.platform) ||
            (b.likes ?? -1) - (a.likes ?? -1),
        );
    if (context.state.materialMode === "ordinary")
      return filtered
        .filter((w) => {
          const p = platformStats(context.snapshot?.statistics).find(
            (p) => p.platform === w.platform,
          );
          return (
            known(p?.highPerformanceThreshold) &&
            known(w.likes) &&
            w.likes < p.highPerformanceThreshold
          );
        })
        .sort(
          (a, b) =>
            a.platform.localeCompare(b.platform) ||
            (b.likes ?? -1) - (a.likes ?? -1),
        );
    return filtered.sort((a, b) => publicationOrder(b) - publicationOrder(a));
  }
  function coverUrl(w: T.Work) {
    const value = w.media?.cover?.localAssetUrl;
    if (typeof value !== "string") return "";
    try {
      const url = new URL(value, location.origin);
      return url.origin === location.origin &&
        ["/api/local/media/", "/api/local/derivative/"].some((prefix) =>
          url.pathname.startsWith(prefix),
        )
        ? url.href
        : "";
    } catch {
      return "";
    }
  }
  function avatar(name: string | null | undefined) {
    const initials = Array.from(typeof name === "string" ? name.trim() : "")
      .slice(0, 2)
      .join("");
    return (
      <>
        <span className={cls("lgi-tm-avatar")} aria-hidden={"true"}>
          {esc(initials || "·")}
        </span>
      </>
    );
  }
  function card(w: T.Work) {
    const cover = coverUrl(w);
    return (
      <>
        <article className={cls("lgi-tm-material")}>
          <button
            type={"button"}
            className={cls("lgi-tm-cover")}
            data-action={"reader"}
            data-id={esc(w.workRef)}
            aria-label={"打开" + String(esc(title(w)) ?? "")}
          >
            {cover ? (
              <>
                <img
                  src={esc(cover)}
                  loading={"lazy"}
                  alt={String(esc(title(w)) ?? "") + "的已取得封面"}
                />
              </>
            ) : (
              <>
                <span>
                  {"▧"}
                  <br />
                  {"封面尚未采集"}
                </span>
              </>
            )}
          </button>
          <div className={cls("lgi-tm-row lgi-tm-card-meta")}>
            <span>{esc(platformName(w.platform))}</span>
            {w.own
              ? badge(
                  w.ownBreakout ? "我方手动爆款" : "我方作品",
                  w.ownBreakout,
                )
              : ""}
          </div>
          <h4>
            {btn(esc(title(w)), "reader", w.workRef, "lgi-tm-title-link")}
          </h4>
          {materialSourceNote(w)}
          <p
            className={cls("lgi-tm-author")}
            title={esc(w.creatorDisplayName || "作者未知")}
          >
            {avatar(w.creatorDisplayName)}
            {esc(w.creatorDisplayName || "作者未知")}
          </p>
          <p className={cls("lgi-tm-note")}>{esc(publishedTime(w))}</p>
          <div className={cls("lgi-tm-interactions")}>
            <span title={"点赞精确值 " + String(number(w.likes) ?? "")}>
              {"赞 "}
              <b>{number(w.likes)}</b>
            </span>
            <span title={"收藏精确值 " + String(number(w.collects) ?? "")}>
              {"藏 "}
              <b>{number(w.collects)}</b>
            </span>
            <span title={"评论精确值 " + String(number(w.comments) ?? "")}>
              {"评 "}
              <b>{number(w.comments)}</b>
            </span>
            <span title={"分享精确值 " + String(number(w.shares) ?? "")}>
              {"分享 "}
              <b>{number(w.shares)}</b>
            </span>
          </div>
        </article>
      </>
    );
  }
  function materialsSection() {
    const ns = sortedMaterials(context.selectedWorks());
    return (
      <>
        <section className={cls("lgi-tm-section")}>
          <div className={cls("lgi-tm-section-head")}>
            <div>
              <h3>{"把数字还原成内容"}</h3>
              <p>{"原作与全部材料直接可读，不受精选10篇上限裁切。"}</p>
            </div>
            <div className={cls("lgi-tm-row")}>
              {[
                ["latest", "最新作品"],
                ["high", "高表现"],
                ["ordinary", "常规作品"],
              ].map(([id, name]) =>
                tab(
                  name,
                  "material-mode",
                  id,
                  context.state.materialMode === id,
                ),
              )}
            </div>
          </div>
          {ns.length ? (
            <>
              <div className={cls("lgi-tm-material-grid")}>
                {ns.slice(0, 6).map(card)}
              </div>
            </>
          ) : (
            empty(
              "当前条件没有对应作品",
              context.state.materialMode === "latest"
                ? "可以调整范围；读取失败与没有材料分别表达。"
                : "未设置样本表现规则时，不擅自划分高表现与常规作品。",
            )
          )}
          <div className={cls("lgi-tm-section-head")}>
            <p>
              {[
                "展示 ",
                Math.min(6, ns.length),
                " / ",
                ns.length,
                " 篇当前读取集合",
              ].join("")}
            </p>
            {btn("查看全部材料", "materials", "", "lgi-tm-quiet")}
          </div>
        </section>
      </>
    );
  }
  function journeyEntries(
    journey: T.Journey | undefined | null,
    mode = context.state.metric,
  ) {
    return list(mode === "involved" ? journey?.involved : journey?.main);
  }
  function stages(journey: T.Journey | undefined | null) {
    return fallbackStages.map((s) => ({
      ...s,
      ...journeyEntries(journey).find((e) => e.stage === s.stage),
    }));
  }
  function journey() {
    const activeSnapshot = context.requireSnapshot();
    const j =
        context.topic(context.state.topicRef)?.journey ||
        activeSnapshot.journey,
      stations = stages(j);
    const questions = [
      "发生了什么？怎样理解？",
      "何时求助？如何评估？",
      "哪些支持适合当前处境？",
      "先试什么？怎样开始？",
      "如何调整并持续支持？",
    ];
    const glyphs = [
      "M10 3a7 7 0 1 0 0 14 7 7 0 0 0 0-14m5 12 6 6",
      "M5 3v18M5 5h10v10h5m-8-3 3 3 3-3",
      "M3 7l9-5 9 5-9 5-9-5m0 6 9 5 9-5m-18 5 9 5 9-5",
      "M7 3l14 9-14 9V3Z",
      "M19 7a8 8 0 1 0 1 9M19 3v5h-5",
    ];
    const stageRefs = (stage: string) =>
      journeyEntries(j, stageNames[stage] ? context.state.metric : "main").find(
        (e) => e.stage === stage,
      );
    const map = (
      <>
        <div
          className={cls("lgi-tm-map-shell")}
          aria-label={"五主阶段旅程地图"}
        >
          <div className={cls("lgi-tm-map-canvas")}>
            <svg
              className={cls("lgi-tm-map-road")}
              viewBox={"0 0 1080 344"}
              preserveAspectRatio={"none"}
              aria-hidden={"true"}
            >
              <path
                className={cls("lgi-tm-route-bed")}
                d={"M180 70H900C1055 70 1055 242 900 242H360"}
              ></path>
              <path
                className={cls("lgi-tm-route")}
                d={"M180 70H900C1055 70 1055 242 900 242H360"}
              ></path>
              <path
                className={cls("lgi-tm-route-dashes")}
                d={"M180 70H900C1055 70 1055 242 900 242H360"}
              ></path>
            </svg>
            <div className={cls("lgi-tm-map-nodes")}>
              {stations.map((station, i) => (
                <>
                  <button
                    type={"button"}
                    className={cls(
                      "lgi-tm-journey-node lgi-tm-station " +
                        String(
                          context.state.stage === station.stage
                            ? "is-active"
                            : "",
                        ),
                    )}
                    data-action={"stage"}
                    data-id={station.stage}
                    aria-pressed={context.state.stage === station.stage}
                    aria-label={
                      String(esc(station.label) ?? "") +
                      "，" +
                      String(number(station.count) ?? "") +
                      "篇，" +
                      String(pct(station.count, j?.denominator) ?? "") +
                      "；查看该阶段场景"
                    }
                  >
                    <svg
                      className={cls("lgi-tm-station-artboard")}
                      viewBox={"-80 -76 160 128"}
                      aria-hidden={"true"}
                    >
                      <path
                        className={cls("lgi-tm-station-base-side")}
                        d={"m-65 1 65 32 65-32v11l-65 34-65-34Z"}
                      ></path>
                      <path
                        className={cls("lgi-tm-station-base-top")}
                        d={"m-65 1 65-33 65 33-65 32Z"}
                      ></path>
                      <g
                        transform={"translate(-27,-62)"}
                        className={cls("lgi-tm-station-art")}
                      >
                        <path
                          className={cls("lgi-tm-station-art-top")}
                          d={"m0 17 31-16 24 13-31 16Z"}
                        ></path>
                        <path
                          className={cls("lgi-tm-station-art-side")}
                          d={"M0 17v32l24 14V30Z"}
                        ></path>
                        <path
                          className={cls("lgi-tm-station-art-front")}
                          d={"m24 30 31-16v32L24 63Z"}
                        ></path>
                        <path
                          className={cls("lgi-tm-station-art-detail")}
                          d={"m5 25 12 6m-12 1 12 6m22-30v-11"}
                        ></path>
                        <circle
                          className={cls("lgi-tm-station-art-pin")}
                          cx={"39"}
                          cy={"-4"}
                          r={"2.5"}
                        ></circle>
                        <svg
                          x={"29"}
                          y={"27"}
                          width={"18"}
                          height={"22"}
                          viewBox={"0 0 24 24"}
                          className={cls("lgi-tm-station-glyph")}
                        >
                          <path d={glyphs[i]}></path>
                        </svg>
                      </g>
                    </svg>
                    <span className={cls("lgi-tm-node-name")}>
                      <small className={cls("lgi-tm-node-index")}>
                        {["0", i + 1].join("")}
                      </small>
                      {esc(station.label)}
                    </span>
                    <span className={cls("lgi-tm-node-count")}>
                      <strong
                        title={String(number(station.count) ?? "") + " 篇"}
                      >
                        {readoutNumber(station.count)}
                      </strong>
                      <small>
                        {["篇 · ", pct(station.count, j?.denominator)].join("")}
                      </small>
                    </span>
                    <span className={cls("lgi-tm-node-question")}>
                      {questions[i]}
                    </span>
                  </button>
                </>
              ))}
            </div>
          </div>
        </div>
      </>
    );
    const table = (
      <>
        <div className={cls("lgi-tm-table-scroll")}>
          <table className={cls("lgi-tm-journey-table")}>
            <caption className={cls("v7-sr-only")}>
              {"旅程阶段列表；与地图使用相同材料和统计口径"}
            </caption>
            <thead>
              <tr>
                <th>{"阶段"}</th>
                <th>{"作品数"}</th>
                <th>
                  {context.state.metric === "main"
                    ? "主阶段占比"
                    : "阶段涉及率"}
                </th>
                <th>{"主要讨论"}</th>
                <th>{"材料"}</th>
              </tr>
            </thead>
            <tbody>
              {stations.map((e, i) => {
                const refs = new Set(list(e.workRefs)),
                  groups = discussionGroups(
                    context.selectedWorks().filter((w) => refs.has(w.workRef)),
                  ),
                  label = groups[0]?.label;
                return (
                  <>
                    <tr
                      className={cls(
                        context.state.stage === e.stage ? "is-active" : "",
                      )}
                      data-action={"stage"}
                      data-id={e.stage}
                    >
                      <td>
                        {btn(
                          <>
                            <small className={cls("lgi-tm-num")}>
                              {["0", i + 1].join("")}
                            </small>
                            {[" ", esc(e.label)].join("")}
                          </>,
                          "stage",
                          e.stage,
                          "lgi-tm-title-link",
                          { "aria-pressed": context.state.stage === e.stage },
                        )}
                      </td>
                      <td className={cls("lgi-tm-num")}>{number(e.count)}</td>
                      <td className={cls("lgi-tm-num")}>
                        {pct(e.count, j?.denominator)}
                      </td>
                      <td>
                        <span
                          className={cls("lgi-tm-table-topic")}
                          title={esc(label || "当前没有已形成的讨论")}
                        >
                          {esc(label || "—")}
                        </span>
                      </td>
                      <td>
                        {btn(
                          "查看材料",
                          "stage-materials",
                          e.stage,
                          "lgi-tm-quiet",
                        )}
                      </td>
                    </tr>
                  </>
                );
              })}
            </tbody>
          </table>
        </div>
      </>
    );
    const sceneWorks = stageWorks(),
      groups = sceneGroups(sceneWorks, context.state.sceneQuery);
    return (
      <>
        <div className={cls("lgi-tm-view-toolbar lgi-tm-journey-toolbar")}>
          <div
            className={cls("lgi-tm-row")}
            role={"group"}
            aria-label={"旅程统计口径"}
          >
            {tab(
              "主阶段分布",
              "metric",
              "main",
              context.state.metric === "main",
            )}
            {tab(
              "阶段涉及率",
              "metric",
              "involved",
              context.state.metric === "involved",
            )}
          </div>
          <div className={cls("lgi-tm-row")}>
            {btn("主题 × 旅程", "matrix", "", "lgi-tm-quiet")}
            {btn("统计口径", "method", "", "lgi-tm-quiet")}
            <div
              className={cls("lgi-tm-row lgi-tm-view-segment")}
              role={"group"}
              aria-label={"旅程呈现方式"}
            >
              {tab("地图", "map-mode", "map", context.state.mapMode === "map")}
              {tab(
                "列表",
                "map-mode",
                "list",
                context.state.mapMode === "list",
              )}
            </div>
          </div>
        </div>
        <div className={cls("lgi-tm-journey-basis")}>
          <span>
            {[
              "概念路径，非用户迁移 · 分母 ",
              number(j?.denominator),
              " 篇可读作品",
            ].join("")}
          </span>
          <span>
            {[
              context.state.metric === "involved"
                ? "同篇可涉及多个阶段，合计可超过100%"
                : "每篇一个主阶段，其他类别单列",
              context.state.overlay ? " · 已应用跨阶段状态" : "",
            ].join("")}
          </span>
        </div>
        {context.state.mapMode === "map" ? map : table}
        <div className={cls("lgi-tm-other-stages")} aria-label={"其他阶段分类"}>
          {Object.entries(otherNames).map(([key, name]) => {
            const e = stageRefs(key);
            return tab(
              `${name} ${number(e?.count)}`,
              "stage",
              key,
              context.state.stage === key,
            );
          })}
        </div>
        <section className={cls("lgi-tm-journey-scenes")}>
          <div className={cls("lgi-tm-section-head")}>
            <div>
              <h3>
                {[
                  context.state.stage
                    ? stageNames[context.state.stage] ||
                      otherNames[context.state.stage]
                    : "全部阶段",
                  " · 具体场景",
                ].join("")}
              </h3>
              <p>
                {[
                  sceneWorks.length,
                  " 篇当前材料 · ",
                  groups.length,
                  " 个场景",
                  context.state.sceneQuery ? "匹配" : "",
                ].join("")}
                {context.state.stage ? (
                  <> {btn("清除阶段选择", "stage", "", "lgi-tm-quiet")}</>
                ) : (
                  ""
                )}
              </p>
            </div>
            <div className={cls("lgi-tm-row")}>
              <form className={cls("lgi-tm-search")} data-form={"scene-search"}>
                <label
                  className={cls("v7-sr-only")}
                  htmlFor={"topic-map-scene-search"}
                >
                  {"搜索场景"}
                </label>
                <div className={cls("lgi-tm-row")}>
                  <input
                    id={"topic-map-scene-search"}
                    name={"query"}
                    defaultValue={esc(context.state.sceneQuery)}
                    placeholder={"搜索场景"}
                  />
                  <button type={"submit"} className={cls("lgi-tm-button")}>
                    {"搜索"}
                  </button>
                  {context.state.sceneQuery
                    ? btn("清除", "clear-scene-query", "", "lgi-tm-quiet")
                    : ""}
                </div>
              </form>
              {btn(
                "查看当前阶段材料",
                "stage-materials",
                context.state.stage,
                "lgi-tm-quiet",
              )}
            </div>
          </div>
          {groups.length
            ? sceneList(
                sceneWorks,
                context.state.sceneQuery,
                context.state.showAllScenes ? Infinity : 6,
              )
            : empty(
                context.state.sceneQuery
                  ? "没有匹配的已研究场景"
                  : "当前材料尚未形成可引用的场景分组",
                "原作与已有评论仍可查看，不代表这个阶段没有内容。",
              )}
          {groups.length > 6
            ? btn(
                context.state.showAllScenes
                  ? "收起更多场景"
                  : `展开其余 ${groups.length - 6} 个场景`,
                "scene-more",
                "",
                "lgi-tm-quiet",
              )
            : ""}
        </section>
        {materialWorkspace()}
      </>
    );
  }
  function curated(
    ref: string | undefined,
    source = context.snapshot,
  ): T.Work[] {
    if (!source) return [];
    const t = list(source.topics).find((t) => t.topicRef === ref);
    const candidates = list(source.works).filter(
      (w) =>
        readableMaterial(w) &&
        (!t ||
          list(w.topicRefs).includes(ref || "") ||
          list(t.workRefs).includes(w.workRef)),
    );
    const recent = new Set(list(source.scope.recentReferenceWorkRefs));
    const eligible = candidates.filter((w) =>
      w.own
        ? Boolean(w.publishedAt || w.publishedAtSourceText)
        : recent.has(w.workRef),
    );
    const high = eligible.filter((w) =>
      w.own
        ? w.ownBreakout
        : (known(w.followerCount) &&
            w.followerCount <= 1000 &&
            known(w.likes) &&
            w.likes >= 500) ||
          (known(
            platformStats(source.statistics).find(
              (p) => p.platform === w.platform,
            )?.highPerformanceThreshold,
          ) &&
            known(w.likes) &&
            w.likes >=
              (platformStats(source.statistics).find(
                (p) => p.platform === w.platform,
              )?.highPerformanceThreshold ?? Infinity)),
    );
    high.sort(
      (a, b) =>
        Number(b.ownBreakout) - Number(a.ownBreakout) ||
        a.platform.localeCompare(b.platform) ||
        (b.likes ?? -1) - (a.likes ?? -1),
    );
    const picked = high.slice(0, 10);
    const contrasts = eligible.filter(
      (w) =>
        !picked.some((p) => p.workRef === w.workRef) &&
        picked.some((p) => p.platform === w.platform && sharesDiscussion(p, w)),
    );
    if (!picked.length)
      return eligible
        .slice()
        .sort((a, b) => publicationOrder(b) - publicationOrder(a))
        .slice(0, 3);
    const result: T.Work[] = [],
      chosen = new Set<string>();
    const add = (w: T.Work) => {
      if (result.length < 10 && !chosen.has(w.workRef)) {
        result.push(w);
        chosen.add(w.workRef);
      }
    };
    for (const sample of picked) {
      add(sample);
      if (!sample.own) {
        contrasts
          .filter(
            (w) =>
              w.own &&
              w.platform === sample.platform &&
              sharesDiscussion(sample, w),
          )
          .forEach(add);
      }
    }
    contrasts.forEach(add);
    return result;
  }
  function ownTopicWorks(ref: string | undefined, source = context.snapshot) {
    const t = list(source?.topics).find((t) => t.topicRef === ref);
    return list(source?.works).filter(
      (w) =>
        w.own &&
        Boolean(w.publishedAt || w.publishedAtSourceText) &&
        (!t ||
          list(w.topicRefs).includes(ref || "") ||
          list(t.workRefs).includes(w.workRef)),
    );
  }
  function replacementCandidates(parent: T.DialogState) {
    const source = parent.snapshot || context.requireSnapshot(),
      recent = new Set(list(source.scope.recentReferenceWorkRefs)),
      t = list(source.topics).find((t) => t.topicRef === parent.topicRef),
      members = new Set(list(t?.workRefs));
    return list(source.works).filter(
      (w) =>
        readableMaterial(w) &&
        (members.has(w.workRef) ||
          list(w.topicRefs).includes(parent.topicRef || "")) &&
        (w.own
          ? Boolean(w.publishedAt || w.publishedAtSourceText)
          : recent.has(w.workRef)),
    );
  }
  return {
    select,
    matchesWork,
    stageWorks,
    workspaceWorks,
    scopeChips,
    emptyWorkspace,
    materialRows,
    materialWorkspace,
    sidebar,
    scopeText,
    header,
    observedPlatforms,
    platformControl,
    controls,
    materialSearch,
    summary,
    tree,
    metrics,
    coverage,
    overview,
    children,
    plotPlatform,
    plotSelect,
    changeRows,
    changesBar,
    structure,
    structureTable,
    topicScatter,
    distribution,
    topicBoundary,
    relationDetails,
    discussionGroups,
    conceptDecisionDetails,
    discussionUnitView,
    discussionGroupDetails,
    patterns,
    sceneGroups,
    sceneList,
    commentFragmentCount,
    sources,
    candidates,
    actionView,
    sortedMaterials,
    coverUrl,
    avatar,
    card,
    materialsSection,
    journeyEntries,
    stages,
    journey,
    curated,
    ownTopicWorks,
    replacementCandidates,
  };
}
