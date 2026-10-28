import { parseTopicProgress, type TopicProgress } from "./topicResearchModel";
import type { ChangeEvent, FormEvent, KeyboardEvent, ToggleEvent } from "react";
import { Fragment } from "react";
import { ApiError, isRecord } from "../../shared/api";
import { attributes } from "./elementProps";
import { researchCoverage } from "./sourceViews";
import { c as cls } from "./styles";
import { createTopicCommands } from "./topicCommands";
import { createTopicDialogs } from "./topicDialogs";
import {
  decodeAlternative,
  decodeCommand,
  decodeComments,
  decodeCollectionProgress,
  decodeEvidence,
  decodePreview,
  decodeCollectionReceipt,
  decodeResource,
  decodeSearch,
  decodeSnapshot,
  decodeStructure,
} from "./topicDtos";
import { badge, btn, empty, tab } from "./topicElements";
import {
  analysisOutputs,
  applyQualifiedReader,
  caught,
  discussionUnits,
  errorStatus,
  esc,
  evidenceWorks,
  fragmentWorkRef,
  labelState,
  list,
  number,
  otherNames,
  platformStats,
  readableMaterial,
  readerFragmentInBody,
  readerTitleFragment,
  researchQualificationUrl,
  sourceKind,
  stageNames,
  title,
  unreadResearchWork,
} from "./topicModel";
import { createTopicOverview } from "./topicOverview";
import { topicRequest } from "./topicRequest";
import type * as T from "./types";
export function createTopicEngine() {
  let disposed = false,
    revision = 0,
    dialogFeedbackText = "";
  const readControllers = new Set<AbortController>();
  let bootstrapDomain = "";
  const listeners = new Set<() => void>(),
    controllers = new Set<AbortController>();
  let root: HTMLDivElement | null = null,
    dialog: HTMLDialogElement | null = null;
  const surfaces = () =>
    [
      root,
      document.getElementById("topic-map-view-tabs"),
      document.getElementById("topic-map-context-actions"),
    ].filter((node): node is HTMLElement => node !== null);
  const inWorkspace = (node: Node) =>
    surfaces().some((surface) => surface.contains(node));
  let dialogRevision = 0,
    dialogScroll: number | null = null;
  let mainRevision = 0,
    mainScrollRequest: number | null = null;
  let resetMainScroll = false;

  const uuid = () => crypto.randomUUID();

  const views = [
    ["content", "材料与讨论"],
    ["structure", "结构与表现"],
    ["patterns", "内容格局"],
    ["sources", "来源分布"],
    ["candidates", "新方向"],
    ["action", "我方对照与再做"],
  ];
  const defaults: T.ViewState = {
    domainRef: "",
    topicRef: "",
    platform: "",
    windowDays: "",
    referenceWindowDays: "30",
    view: "overview",
    tab: "content",
    path: "",
    overlay: "",
    stage: "",
    metric: "main",
    mapMode: "map",
    materialMode: "latest",
    summaryChart: "table",
    plotPlatform: "",
    changesOnly: false,
    query: "",
    collapsed: [],
    compare: [],
    navigationOpen: false,
    overviewWindowDays: "",
    journeyWindowDays: "",
    sceneQuery: "",
    showAllScenes: false,
    legacyExpanded: false,
    layoutVersion: 3,
  };
  const restored: Partial<T.ViewState> = {};
  try {
    const value: unknown = JSON.parse(
      sessionStorage.getItem("linggan.topic-map.view") || "{}",
    );
    if (isRecord(value)) {
      for (const key of Object.keys(defaults) as (keyof T.ViewState)[]) {
        const candidate = value[key];
        if (typeof defaults[key] === "string" && typeof candidate === "string")
          Object.assign(restored, { [key]: candidate });
        else if (
          typeof defaults[key] === "boolean" &&
          typeof candidate === "boolean"
        )
          Object.assign(restored, { [key]: candidate });
        else if (
          Array.isArray(defaults[key]) &&
          Array.isArray(candidate) &&
          candidate.every((v) => typeof v === "string")
        )
          Object.assign(restored, { [key]: candidate });
        else if (key === "layoutVersion" && typeof candidate === "number")
          restored.layoutVersion = candidate;
      }
    }
  } catch {
    /* Session preferences are optional. */
  }
  const state = { ...defaults, ...restored };
  if (restored.layoutVersion !== 3) Object.assign(state, { layoutVersion: 3 });
  if (!views.some(([id]) => id === state.tab)) state.tab = "content";
  if (!["overview", "journey"].includes(state.view)) state.view = "overview";
  const params = new URLSearchParams(location.search);
  // The shared picker links only the new domain; former scope must not follow it.
  const incomingDomain = params.get("domain") ?? params.get("domainRef");
  if (
    params.has("domain") ||
    (incomingDomain !== null && incomingDomain !== state.domainRef)
  )
    Object.assign(state, defaults, { domainRef: incomingDomain });
  const urlKeys: T.TextStateKey[] = [
    "domainRef",
    "topicRef",
    "platform",
    "windowDays",
    "referenceWindowDays",
    "view",
    "tab",
    "path",
    "overlay",
    "stage",
    "metric",
    "mapMode",
    "materialMode",
    "query",
    "sceneQuery",
    "overviewWindowDays",
    "journeyWindowDays",
  ];
  if (params.has("tm"))
    Object.assign(state, defaults, {
      collapsed: list(state.collapsed),
      navigationOpen: state.navigationOpen === true,
      legacyExpanded: state.legacyExpanded === true,
    });
  urlKeys.forEach((key) => {
    if (params.has(key)) state[key] = params.get(key) || "";
  });
  function normalizeState() {
    if (!["overview", "journey"].includes(state.view)) state.view = "overview";
    if (!views.some(([key]) => key === state.tab)) state.tab = "content";
    if (
      ![...Object.keys(stageNames), ...Object.keys(otherNames)].includes(
        state.stage,
      )
    )
      state.stage = "";
    if (!["main", "involved"].includes(state.metric)) state.metric = "main";
    if (!["latest", "high", "ordinary"].includes(state.materialMode))
      state.materialMode = "latest";
    if (!["", "xhs", "douyin"].includes(state.platform)) state.platform = "";
    for (const key of [
      "windowDays",
      "overviewWindowDays",
      "journeyWindowDays",
    ] as const)
      if (!["", "7", "30", "90"].includes(state[key])) state[key] = "";
    if (!["map", "list"].includes(state.mapMode)) state.mapMode = "map";
    for (const key of ["query", "sceneQuery"] as const)
      if (typeof state[key] !== "string") state[key] = "";
    state[
      state.view === "journey" ? "journeyWindowDays" : "overviewWindowDays"
    ] = state.windowDays;
    if (!["", "family", "adult"].includes(state.path)) state.path = "";
    if (
      !["", "obstruction_recurrence", "transition_handoff"].includes(
        state.overlay,
      )
    )
      state.overlay = "";
  }
  normalizeState();
  // The page supplies the Topic API's name-ordered default before an expensive map read.
  // Default domain is supplied by the server page context before activate().
  const extraWorks = new Map<string, T.Work>();
  let progressError = "",
    progressLoading = false,
    progressEpoch = 0;
  let snapshot: T.Snapshot | null = null,
    progress: TopicProgress | null = null,
    loading = false,
    materialsLoading = false,
    materialsError = "",
    readError = "",
    notice = "",
    pending = false;
  let dialogState: T.DialogState | null = null,
    dialogStack: T.DialogState[] = [],
    triggerElement: HTMLElement | null = null,
    triggerAction: { action?: string; id?: string } | null = null,
    fetchEpoch = 0;
  function requireSnapshot() {
    if (!snapshot) throw new Error("图谱尚未读取");
    return snapshot;
  }
  function isReader(
    value: T.DialogState | T.Reader | undefined,
  ): value is T.Reader & T.DialogState {
    return Boolean(
      value?.work && value.domainRef && typeof value.loading === "boolean",
    );
  }
  function requireDialog() {
    if (!dialogState) throw new Error("对话框已关闭");
    return dialogState;
  }

  const topic = (ref: string | null | undefined) =>
    list(snapshot?.topics).find((t) => t.topicRef === ref);
  const work = (ref: string | undefined | null) =>
    list(snapshot?.works).find((w) => w.workRef === ref) ||
    extraWorks.get(ref || "");
  const topicName = (ref: string) => topic(ref)?.displayName || "主题";

  function persist(replace = false) {
    try {
      sessionStorage.setItem("linggan.topic-map.view", JSON.stringify(state));
    } catch {
      /* Session preferences are optional. */
    }
    const url = new URL(location.href);
    url.searchParams.delete("domain");
    url.searchParams.set("tm", "1");
    urlKeys.forEach((key) =>
      state[key]
        ? url.searchParams.set(key, state[key])
        : url.searchParams.delete(key),
    );
    if (url.href !== location.href)
      history[replace ? "replaceState" : "pushState"](null, "", url);
  }
  function switchView(view: string) {
    if (!["overview", "journey"].includes(view) || state.view === view)
      return false;
    state[
      state.view === "journey" ? "journeyWindowDays" : "overviewWindowDays"
    ] = state.windowDays;
    state.view = view;
    state.windowDays =
      state[view === "journey" ? "journeyWindowDays" : "overviewWindowDays"];
    resetMainScroll = true;
    return true;
  }
  function query(overrides: Partial<T.ViewState> = {}) {
    const values = {
      ...state,
      ...(state.view === "overview" ? { path: "", overlay: "" } : {}),
      ...overrides,
    };
    const q = new URLSearchParams();
    (
      [
        "domainRef",
        "topicRef",
        "platform",
        "windowDays",
        "referenceWindowDays",
        "path",
        "overlay",
      ] as const
    ).forEach((key) => {
      if (values[key]) q.set(key, values[key]);
    });
    return q.toString();
  }
  async function request<V>(
    url: string,
    parse: (value: unknown) => V,
    options?: { method?: "POST"; body?: string },
  ) {
    const controller = new AbortController();
    controllers.add(controller);
    if (!options?.method) readControllers.add(controller);
    try {
      const value = await topicRequest(url, parse, options, controller.signal);
      const domain = new URL(url, location.origin).searchParams.get(
        "domainRef",
      );
      if (domain && isRecord(value)) {
        const scope = isRecord(value.scope) ? value.scope : value;
        if ("domainRef" in scope && scope.domainRef !== domain)
          throw new ApiError({ kind: "invalid_response", status: 200 });
      }
      return value;
    } finally {
      controllers.delete(controller);
      readControllers.delete(controller);
    }
  }
  const topicOverview = createTopicOverview({
    get state() {
      return state;
    },
    get selectedWorks() {
      return selectedWorks;
    },
    get topic() {
      return topic;
    },
    get snapshot() {
      return snapshot;
    },
    get discussionsView() {
      return topicDialogs.discussionsView;
    },
    get loading() {
      return loading;
    },
    get readError() {
      return readError;
    },
    get requireSnapshot() {
      return requireSnapshot;
    },
    get views() {
      return views;
    },
    get dialogState() {
      return dialogState;
    },
    get readerCitationTarget() {
      return readerCitationTarget;
    },
    get anglesFrom() {
      return topicDialogs.anglesFrom;
    },
    get work() {
      return work;
    },
    get topicName() {
      return topicName;
    },
  });
  const topicDialogs = createTopicDialogs({
    get requireDialog() {
      return requireDialog;
    },
    get requireSnapshot() {
      return requireSnapshot;
    },
    get matchesWork() {
      return topicOverview.matchesWork;
    },
    get card() {
      return topicOverview.card;
    },
    get avatar() {
      return topicOverview.avatar;
    },
    get fragmentSourceLabel() {
      return fragmentSourceLabel;
    },
    get state() {
      return state;
    },
    get readerDiscussions() {
      return readerDiscussions;
    },
    get topic() {
      return topic;
    },
    get curated() {
      return topicOverview.curated;
    },
    get ownTopicWorks() {
      return topicOverview.ownTopicWorks;
    },
    get topicBoundary() {
      return topicOverview.topicBoundary;
    },
    get commentFragmentCount() {
      return topicOverview.commentFragmentCount;
    },
    get discussionGroups() {
      return topicOverview.discussionGroups;
    },
    get discussionGroupDetails() {
      return topicOverview.discussionGroupDetails;
    },
    get sceneList() {
      return topicOverview.sceneList;
    },
    get dialogState() {
      return dialogState;
    },
    get replacementCandidates() {
      return topicOverview.replacementCandidates;
    },
    get scopeText() {
      return topicOverview.scopeText;
    },
    get journeyEntries() {
      return topicOverview.journeyEntries;
    },
    get pending() {
      return pending;
    },
    get observedPlatforms() {
      return topicOverview.observedPlatforms;
    },
    get work() {
      return work;
    },
    get selectedWorks() {
      return selectedWorks;
    },
    get alternativeAvailable() {
      return alternativeAvailable;
    },
    get topicName() {
      return topicName;
    },
  });
  const mutations = createTopicCommands(
    (path, body, parse) => request(path, parse, { method: "POST", body }),
    notify,
  );
  async function load() {
    const epoch = ++fetchEpoch;
    readControllers.forEach((controller) => controller.abort());
    readControllers.clear();
    progress = null;
    progressError = "";
    progressLoading = Boolean(state.domainRef);
    if (state.domainRef) void refreshReadiness(epoch, state.domainRef);
    loading = true;
    materialsLoading = false;
    materialsError = "";
    readError = "";
    render();
    try {
      const result = await request(
        `/api/local/topic-map/summary?${query()}`,
        decodeSnapshot,
      );
      if (epoch !== fetchEpoch) return;
      if (
        !result ||
        !Array.isArray(result.topics) ||
        !Array.isArray(result.works) ||
        !result.scope ||
        !result.statistics
      )
        throw new Error("主题图谱读取合同不完整，原有结果继续保留");
      snapshot = result;
      if (!state.domainRef && result.scope.domainRef)
        state.domainRef = result.scope.domainRef;
      if (!state.domainRef && result.domains.length) {
        state.domainRef = result.domains[0]?.domainRef || "";
        loading = false;
        persist();
        return await load();
      }
      if (state.topicRef && !topic(state.topicRef)) state.topicRef = "";
      persist(true);
      loading = false;
      materialsLoading = true;
      render();

      const materials = await request(
        `/api/local/topic-map/overview?${query()}`,
        decodeSnapshot,
      );
      if (epoch !== fetchEpoch) return;
      if (
        !materials ||
        !Array.isArray(materials.works) ||
        !materials.scope ||
        !materials.statistics
      )
        throw new Error("材料读取合同不完整");
      // Replace the whole version if authority changed between the two reads.
      snapshot = materials;
      materialsLoading = false;
    } catch (caughtError) {
      const error = caught(caughtError);
      if (
        epoch === fetchEpoch &&
        errorStatus(caughtError) === 404 &&
        state.topicRef
      ) {
        state.topicRef = "";
        notice =
          "原主题引用当前不可用，已返回该领域概览；旧备选的定义引用仍保留。";
        persist();
        return await load();
      }
      if (epoch === fetchEpoch) {
        if (materialsLoading) materialsError = error.message;
        else readError = error.message;
        materialsLoading = false;
      }
    } finally {
      if (epoch === fetchEpoch) {
        loading = false;
        render();
      }
    }
  }
  async function command(
    payload: Record<string, unknown>,
    successText: string,
  ) {
    if (pending) return null;
    pending = true;
    notice = "";
    renderNotice();
    const owner = dialogState,
      domain = state.domainRef;
    try {
      const result = await mutations.submit(
        "/api/local/topic-map/commands",
        {
          ...payload,
          idempotencyKey: payload.idempotencyKey || uuid(),
          domainRef: payload.domainRef || domain,
        },
        decodeCommand,
        async (result) => {
          notice =
            state.domainRef === domain
              ? successText || "操作已获得后端回执"
              : "原范围操作已获得后端回执，当前范围未更新";
          if (state.domainRef !== domain || disposed) return;
          if (owner?.type === "save" && payload.action === "saveAlternative")
            owner.savedAlternativeRef = result.subjectRef || undefined;
          await load();
          if (dialogState === owner && owner)
            dialogFeedback(`已保存 · 回执 ${result.receiptRef}`);
        },
      );
      return !disposed && state.domainRef === domain && dialogState === owner
        ? result
        : null;
    } catch (caughtError) {
      const error = caught(caughtError);
      notice = `操作未完成：${error.message}`;
      return null;
    } finally {
      pending = false;
      renderNotice();
    }
  }
  function selectedWorks() {
    return list(snapshot?.works);
  }
  async function readResearchProgress(
    domainRef: string,
    scopeEpoch = fetchEpoch,
  ) {
    const epoch = ++progressEpoch;
    const current = () =>
      epoch === progressEpoch &&
      scopeEpoch === fetchEpoch &&
      domainRef === state.domainRef;
    progressLoading = true;
    renderReadiness();
    try {
      const value = await request(
        `/api/local/topic-research/runs?domainRef=${encodeURIComponent(domainRef)}`,
        parseTopicProgress,
      );
      if (current()) {
        progress = value;
        progressError = "";
      }
      return current() ? value : null;
    } catch (caughtError) {
      const error = caught(caughtError);
      if (current()) {
        progressError = error.message;
        throw error;
      }
      return null;
    } finally {
      if (current()) {
        progressLoading = false;
        renderReadiness();
      }
    }
  }
  async function refreshReadiness(epoch: number, domainRef: string) {
    progress = null;
    progressError = "";
    try {
      await readResearchProgress(domainRef, epoch);
    } catch {
      /* Shared reader records the visible failure. */
    }
  }
  function renderReadiness() {
    notify();
  }
  function readiness() {
    const counts = progress?.summary;
    const operation = progressLoading ? "研究进度读取中" : progressError ? "研究进度读取失败" : "主题研究";
    const summary = counts
      ? `接纳 ${number(counts.succeededStageCount)} · 失败 ${number(counts.failedStageCount)} · 待完成 ${number(counts.pendingStageCount)}`
      : "当前方法统计尚未取得";
    return btn(
      <>
        {esc(operation)}
        {counts ? (
          <>
            <small className={cls("lgi-tm-operation-counts")}>
              {[
                "失败 ",
                number(counts.failedStageCount),
                " · 待完成 ",
                number(counts.pendingStageCount),
              ].join("")}
            </small>
          </>
        ) : (
          ""
        )}
      </>,
      "research",
      "",
      "lgi-tm-quiet lgi-tm-research-status",
      {
        title: `${summary}；打开主题研究与阶段回执`,
        "aria-label": `${operation}；${summary}；打开主题研究与阶段回执`,
      },
    );
  }

  function renderNotice() {
    notify();
  }

  function notify() {
    if (disposed) return;
    revision++;
    listeners.forEach((listener) => listener());
  }
  function render() {
    mainScrollRequest = resetMainScroll
      ? 0
      : root?.querySelector("[data-topic-main]")?.scrollTop || 0;
    mainRevision++;
    notify();
  }
  function mutationRetry() {
    return mutations.unconfirmed ? (
      <span role="status">
        上一次动作结果未确认；当前修改尚未提交。
        {btn("重试原请求", "mutation-retry")}
      </span>
    ) : null;
  }
  function getContent(noticeText = notice) {
    const busy = loading || Boolean(readError) || !snapshot;
    const body = readError ? (
      <>
        <div className={cls("lgi-tm-read-error")} role={"alert"}>
          <strong>{"当前读取未成功"}</strong>
          <p>{esc(readError)}</p>
          {btn("重新读取", "refresh")}
          {snapshot ? (
            <>
              <p>{"旧结果保留在会话中，不混入当前未成功读取的范围。"}</p>
            </>
          ) : (
            ""
          )}
        </div>
      </>
    ) : loading ? (
      <>
        <div className={cls("lgi-tm-summary-loading")} role={"status"}>
          {"正在读取当前范围的已有材料与统计…"}
        </div>
      </>
    ) : snapshot ? (
      <>
        {topicOverview.summary()}
        {materialsLoading ? (
          <>
            <section aria-label={"首批材料"}>
              <p role={"status"}>
                {[
                  "完整统计已读取，已载入 ",
                  list(snapshot.works).length,
                  " 篇首批材料；正在载入其余材料与讨论…",
                ].join("")}
              </p>
              {topicOverview.materialRows(
                topicOverview.workspaceWorks().slice(0, 12),
              )}
            </section>
          </>
        ) : materialsError ? (
          <>
            <div role={"alert"}>
              <p>{["材料读取未成功：", esc(materialsError)].join("")}</p>
              {btn("重新读取", "refresh")}
            </div>
          </>
        ) : state.view === "overview" ? (
          topicOverview.overview()
        ) : (
          topicOverview.journey()
        )}
        <footer className={cls("lgi-tm-footer")}>
          <span>{esc(snapshot.methodVersion || "方法尚未读取")}</span>
          <details>
            <summary>{"数据范围与来源边界"}</summary>
            {topicOverview.coverage()}
          </details>
        </footer>
      </>
    ) : (
      empty("当前图谱尚未读取", "读取页面不启动模型研究或平台采集。")
    );
    return (
      <>
        {topicOverview.header()}
        <div
          data-notice=""
          className={cls("lgi-tm-feedback")}
          role={"status"}
          {...attributes(noticeText ? "" : "hidden")}
        >
          {esc(noticeText)}
        </div>
        <div className={cls("lgi-tm-workbench")}>
          {topicOverview.sidebar()}
          <main
            className={cls("lgi-tm-main")}
            data-topic-main=""
            aria-label={"主题内容"}
            data-loaded={!busy}
            aria-busy={loading}
          >
            {body}
          </main>
        </div>
        {list(state.compare).length ? (
          <>
            <div className={cls("lgi-tm-compare-dock")}>
              <span>
                {[
                  "已选 ",
                  state.compare.length,
                  " 个方向 · ",
                  state.compare.map(topicName).map(esc).join(" / "),
                ].join("")}
              </span>
              {btn("并排比较", "compare", "", "lgi-tm-primary")}
              {btn("清空", "compare-clear", "", "lgi-tm-quiet")}
            </div>
          </>
        ) : (
          ""
        )}
      </>
    );
  }
  function openDialog(
    type: T.DialogKind,
    data: Omit<T.DialogState, "type"> = {},
    push = true,
  ) {
    if (dialogState && push)
      dialogStack.push({
        ...dialogState,
        scroll:
          dialog?.querySelector(".lgi-tm-dialog-body, [data-topic-dialog-body]")
            ?.scrollTop || 0,
      });
    if (!dialogState) {
      triggerElement =
        document.activeElement instanceof HTMLElement
          ? document.activeElement
          : null;
      triggerAction = triggerElement?.dataset?.action
        ? {
            action: triggerElement.dataset.action,
            id: triggerElement.dataset.id,
          }
        : null;
    }
    dialogFeedbackText = "";
    dialogState = { type, ...data };
    dialogRevision++;
    dialogScroll = 0;
    notify();
  }
  function drawDialog() {
    dialogScroll = dialogState?.scroll || 0;
    dialogRevision++;
    notify();
  }
  function getDialogContent() {
    if (!dialogState) return null;
    const { type } = dialogState;
    const names = {
      definition: "主题定义与证据边界",
      capture: "有界补充材料",
      alternative: "保存的原版本与依据",
      move: "调整主题归属",
      structure: "主题结构调整",
      matrix: "主题 × 旅程",
      compare: "并排比较",
      materials: "全部材料",
      reader: "原作与讨论",
      judge: "选题判断",
      identity: "我方矩阵与观察范围",
      saved: "我的备选",
      save: "保留角度与依据",
      research: "主题研究",
      rules: "样本表现规则",
      method: "方法与统计边界",
      changes: "结构与依据变化",
      product: "产品机会：需求与支持假设",
      replace: "替换本次判断样本",
    };
    return (
      <Fragment key={dialogRevision}>
        <div className={cls("lgi-tm-dialog-head")}>
          <div>
            <span className={cls("lgi-tm-eyebrow")}>
              {esc(topicOverview.scopeText())}
            </span>
            <h2 id={"topic-map-dialog-title"}>
              {esc(names[type] || "查看材料")}
            </h2>
          </div>
          <div className={cls("lgi-tm-row")}>
            {dialogStack.length ? btn("返回上一层", "dialog-back") : ""}
            {btn(
              "关闭",
              "dialog-close",
              "",
              "",
              'aria-label="关闭对话框，保留研究任务"',
            )}
          </div>
        </div>
        <div data-topic-dialog-body="" className={cls("lgi-tm-dialog-body")}>
          {dialogBody()}
        </div>
        <div
          className={cls("lgi-tm-dialog-feedback")}
          role={"status"}
          data-dialog-feedback=""
        >
          {dialogFeedbackText}
          {mutationRetry()}
        </div>
      </Fragment>
    );
  }
  function closeDialog() {
    dialogState = null;
    dialogStack = [];
    dialogFeedbackText = "";
    notify();
    queueMicrotask(() => {
      if (disposed) return;
      (triggerElement?.isConnected
        ? triggerElement
        : surfaces()
            .flatMap((surface) =>
              Array.from(
                surface.querySelectorAll<HTMLElement>("[data-action]"),
              ),
            )
            .find(
              (node) =>
                node.dataset.action === triggerAction?.action &&
                node.dataset.id === triggerAction?.id,
            )
      )?.focus({ preventScroll: true });
    });
  }
  function dialogBack() {
    if (!dialogStack.length) return closeDialog();
    dialogState = dialogStack.pop() || null;
    drawDialog();
    dialogScroll = dialogState?.scroll || 0;
  }
  function dialogFeedback(text: string) {
    dialogFeedbackText = text;
    notify();
  }
  function dialogBody() {
    const activeDialog = requireDialog();
    if (activeDialog.error)
      return (
        <>
          {empty("此区域读取未成功", activeDialog.error)}
          {btn("重试", "dialog-retry")}
        </>
      );
    const type = activeDialog.type;
    if (type === "research") return topicDialogs.researchDialog();
    const activeSnapshot = requireSnapshot();
    if (type === "definition") {
      const original = topic(activeDialog.topicRef),
        t =
          original?.core?.sourceState === "source_unavailable"
            ? null
            : original;
      return (
        <>
          {t ? (
            <>
              <h3>{esc(t.displayName)}</h3>
              <p>{esc(t.definitionText || "定义文字尚未取得")}</p>
              <p>
                {[
                  "定义 v",
                  esc(t.definitionVersion),
                  " · ",
                  esc(labelState(t.lifecycleState)),
                ].join("")}
              </p>
              <div className={cls("lgi-tm-row")}>
                {list(t.aliases).map((alias) => badge(alias))}
              </div>
            </>
          ) : (
            ""
          )}
          {topicOverview.topicBoundary(original)}
        </>
      );
    }
    if (type === "materials") return topicDialogs.materialsDialog();
    if (type === "reader")
      return isReader(activeDialog)
        ? topicDialogs.readerDialog(activeDialog)
        : empty("当前阅读器尚未取得材料身份");
    if (type === "judge") return topicDialogs.judgmentDialog();
    if (type === "replace") return topicDialogs.replaceSamplesDialog();
    if (type === "matrix") return topicDialogs.matrixDialog();
    if (type === "compare") return topicDialogs.compareDialog();
    if (type === "identity")
      return activeDialog.loading ? (
        <>
          <p role={"status"}>{"正在读取领域账号与完整我方范围。"}</p>
        </>
      ) : (
        topicDialogs.identityDialog()
      );
    if (type === "rules") return topicDialogs.rulesDialog();
    if (type === "saved") return topicDialogs.savedDialog();
    if (type === "alternative") return topicDialogs.alternativeDialog();
    if (type === "save") return topicDialogs.saveDialog();
    if (type === "product") return topicDialogs.productDialog();
    if (type === "changes") return topicDialogs.changesDialog();
    if (type === "structure") return topicDialogs.structureDialog();
    if (type === "move") return topicDialogs.moveDialog();
    if (type === "capture") return topicDialogs.captureDialog();
    return (
      <>
        <h3>{"当前方法与范围"}</h3>
        <p>{esc(activeSnapshot.methodVersion)}</p>
        <p>{esc(activeSnapshot.sourceBoundary)}</p>
        <div className={cls("lgi-tm-callout")}>
          {
            "主阶段：发现与理解、求助与评估、选择支持、开始实践、长期管理。受阻与转换为覆盖层，不能加进主阶段分布。"
          }
        </div>
        <p>
          {
            "主阶段分母为当前范围的全部可读独立作品；涉及率可以重叠，合计可以超过100%。五阶段之外保留跨阶段综合、通用背景、阶段不明和待分析。"
          }
        </p>
        <p>
          {
            "样本爆款依据已保存的平台规则，低粉高赞固定粉丝≤1,000且点赞≥500，我方爆款由手动标记决定，三者互不替代。"
          }
        </p>
        <p>
          {
            "已有来源材料不直接证明需求、趋势、因果或产品价值；原文与模型解释分开。"
          }
        </p>
      </>
    );
  }

  async function readWork(ref: string) {
    const w =
      work(ref) || dialogState?.snapshot?.works?.find((w) => w.workRef === ref);
    if (!w) return;
    const domainRef = state.domainRef;
    openDialog("reader", {
      work: unreadResearchWork(w),
      domainRef,
      loading: true,
    });
    const current = requireDialog();
    if (!isReader(current)) return;
    const domain = encodeURIComponent(domainRef),
      values = await Promise.allSettled([
        request(
          `/api/local/work-resources/${encodeURIComponent(ref)}?domain=${domain}`,
          decodeResource,
        ),
        request(
          `/api/local/work-resources/${encodeURIComponent(ref)}/comments?domain=${domain}`,
          decodeComments,
        ),
        request(researchQualificationUrl(w, domainRef), decodeEvidence),
      ]);
    applyQualifiedReader(current, values);
    if (dialogState === current) drawDialog();
  }

  function fragmentSourceLabel(w: T.Work, f: T.Fragment): string {
    const context = String(f.field || "").startsWith("work_context:"),
      kind = context
        ? `${sourceKind(String(f.field).slice(13))}上下文（不单独计证据）`
        : sourceKind(f.sourceKind || f.field),
      owner = fragmentWorkRef(w, f);
    if (!owner) return `${kind} · 原文所属作品未知`;
    if (owner === w.workRef) return kind;
    const source =
      work(owner) ||
      dialogState?.snapshot?.works?.find((row) => row.workRef === owner);
    return `${kind} · 来自${source ? "《" + title(source) + "》" : "比较中的另一篇作品"}`;
  }
  function readerCitationTarget(w: T.Work, c: T.LocatedCitation, raw: string) {
    return readerFragmentInBody(w, c.fragment, raw)
      ? `lgi-tm-source-body-${w.workRef}`
      : readerTitleFragment(w, c.fragment)
        ? `lgi-tm-source-title-${w.workRef}`
        : `lgi-tm-source-fragment-${c.fragmentId}`;
  }
  function readerDiscussions(w: T.Work, raw: string) {
    if (!w.research) return "";
    const units = discussionUnits(w);
    return (
      <>
        {researchCoverage(w)}
        <section className={cls("lgi-tm-reader-discussions")}>
          <h4>{"具体讨论与主题归属"}</h4>
          {units.length
            ? units.map((d) =>
                topicOverview.discussionUnitView(
                  { work: w, discussion: d },
                  { raw },
                ),
              )
            : empty(
                Array.isArray(w.research.core?.units)
                  ? "当前已处理范围未形成具体讨论"
                  : "旧研究尚无具体讨论",
                "已有原文与其他研究结果继续可读。",
              )}
        </section>
      </>
    );
  }

  async function readSample(ref: string) {
    const current = requireDialog();
    if (
      current?.type !== "judge" ||
      !list(current.works).some((w) => w.workRef === ref)
    )
      return;
    const selected = list(current.works).find((w) => w.workRef === ref);
    if (!selected) return;
    const domainRef = state.domainRef,
      reader = { work: unreadResearchWork(selected), domainRef, loading: true };
    current.sampleRef = ref;
    current.inlineReader = reader;
    drawDialog();
    const domain = encodeURIComponent(domainRef),
      values = await Promise.allSettled([
        request(
          `/api/local/work-resources/${encodeURIComponent(ref)}?domain=${domain}`,
          decodeResource,
        ),
        request(
          `/api/local/work-resources/${encodeURIComponent(ref)}/comments?domain=${domain}`,
          decodeComments,
        ),
        request(researchQualificationUrl(selected, domainRef), decodeEvidence),
      ]);
    if (current.inlineReader !== reader) return;
    applyQualifiedReader(reader, values);
    if (dialogState?.type === "judge" && dialogState.inlineReader === reader)
      drawDialog();
  }
  async function openMatrix() {
    openDialog("matrix", {
      loading: true,
      rows: "leaves",
      metric: state.metric,
    });
    const current = requireDialog();
    try {
      current.snapshot = await request(
        `/api/local/topic-map?${query({ topicRef: "" })}`,
        decodeSnapshot,
      );
    } catch (caughtError) {
      const error = caught(caughtError);
      current.error = error.message;
    }
    if (dialogState === current) {
      current.loading = false;
      drawDialog();
    }
  }

  async function openCompare() {
    const refs = list(state.compare).slice(0, 3);
    if (!refs.length) return;
    openDialog("compare", { loading: true, refs });
    const current = requireDialog();
    try {
      current.snapshot = await request(
        `/api/local/topic-map?${query({ topicRef: "" })}`,
        decodeSnapshot,
      );
    } catch (caughtError) {
      const error = caught(caughtError);
      current.error = error.message;
    }
    if (dialogState === current) {
      current.loading = false;
      drawDialog();
    }
  }

  async function openIdentity() {
    openDialog("identity", { loading: true });
    await refreshIdentity();
  }
  async function refreshIdentity() {
    const current = requireDialog();
    try {
      const full = await request(
        `/api/local/topic-map?${query({ topicRef: "", windowDays: "", platform: "", path: "", overlay: "" })}`,
        decodeSnapshot,
      );
      if (dialogState === current) {
        current.snapshot = full;
        current.loading = false;
        drawDialog();
      }
    } catch (caughtError) {
      const error = caught(caughtError);
      if (dialogState === current) {
        current.error = error.message;
        current.loading = false;
        drawDialog();
      }
    }
  }

  async function openSaved() {
    openDialog("saved", { loading: true });
    const current = requireDialog();
    try {
      current.snapshot = await request(
        `/api/local/topic-map?${query({ topicRef: "", windowDays: "", platform: "", overlay: "", path: "" })}`,
        decodeSnapshot,
      );
    } catch (caughtError) {
      const error = caught(caughtError);
      current.error = error.message;
    }
    if (dialogState === current) {
      current.loading = false;
      drawDialog();
    }
  }

  const alternativeAvailable = (
    value: T.Alternative | undefined,
  ): value is T.ReadableAlternative =>
    Boolean(
      value &&
        ["available", "version_changed_available"].includes(
          value.sourceState || "",
        ) &&
        typeof value.title === "string" &&
        typeof value.angle === "string" &&
        typeof value.rationale === "string",
    );
  async function readAlternative(ref: string) {
    return request(
      `/api/local/topic-map/alternative?domainRef=${encodeURIComponent(state.domainRef)}&alternativeRef=${encodeURIComponent(ref)}`,
      decodeAlternative,
    );
  }
  async function openAlternative(ref: string) {
    openDialog("alternative", { alternativeRef: ref, loading: true });
    const current = requireDialog();
    try {
      const value = await readAlternative(ref);
      if (dialogState === current) {
        current.alternative = value;
        current.loading = false;
        drawDialog();
      }
    } catch (caughtError) {
      const error = caught(caughtError);
      if (dialogState === current) {
        current.error = error.message;
        current.loading = false;
        drawDialog();
      }
    }
  }

  async function exportAlternative(ref: string) {
    if (pending) return;
    pending = true;
    dialogFeedback("正在重新复核保存记录与来源资格，复核通过后才生成本地导出…");
    try {
      const a = await readAlternative(ref);
      if (
        dialogState?.type === "alternative" &&
        dialogState.alternativeRef === ref
      ) {
        dialogState.alternative = a;
        drawDialog();
      }
      if (!alternativeAvailable(a)) {
        dialogFeedback(
          "来源当前不可用，已停止导出；不输出旧文本或以当前材料补位。",
        );
        return;
      }
      const blob = new Blob([topicDialogs.alternativeMarkdown(a)], {
          type: "text/markdown;charset=utf-8",
        }),
        url = URL.createObjectURL(blob),
        anchor = document.createElement("a");
      anchor.href = url;
      anchor.download = `Linggan_${Array.from(a.title)
        .slice(0, 60)
        .join("")
        .split("")
        .map((ch) =>
          ch.charCodeAt(0) < 32 || '/\\:*?"<>|'.includes(ch) ? "_" : ch,
        )
        .join("")}.md`;
      anchor.hidden = true;
      root?.appendChild(anchor);
      anchor.click();
      anchor.remove();
      setTimeout(() => URL.revokeObjectURL(url), 1000);
      dialogFeedback(
        "来源资格复核通过，已生成本地Markdown导出；没有平台发布或外发模型。",
      );
    } catch (caughtError) {
      const error = caught(caughtError);
      if (dialogState?.type === "alternative") {
        delete dialogState.alternative;
        dialogState.error = error.message;
        drawDialog();
      }
      dialogFeedback(`导出未完成：${error.message}`);
    } finally {
      pending = false;
      notify();
    }
  }

  async function openResearch() {
    if (!state.domainRef) { notice="先选择实际领域。"; renderNotice(); return; }
    openDialog("research",{});
  }

  async function previewStructure(form: HTMLFormElement, data: FormData) {
    const activeDialog = requireDialog();
    const activeSnapshot = requireSnapshot();
    const sources = data.getAll("sources").map((ref) => {
      const t = topic(String(ref));
      if (!t) throw new Error("所选主题已不在当前范围");
      return { topicRef: t.topicRef, definitionRef: t.definitionRef };
    });
    const kind = activeDialog.kind || "merge",
      count = kind === "merge" ? 1 : activeDialog.destinationCount || 2;
    if (
      (kind === "merge" && sources.length < 2) ||
      (kind === "split" && sources.length !== 1)
    )
      return dialogFeedback("合并至少选择两个旧主题，拆分只选择一个旧主题。");
    const destinations: T.StructureDestination[] = Array.from(
      { length: count },
      (_, i) => ({
        displayName: String(data.get(`title-${i}`)).trim(),
        definitionText: String(data.get(`definition-${i}`)).trim(),
        parentTopicRef: String(data.get(`parent-${i}`)) || null,
        members: [],
      }),
    );
    for (const w of activeSnapshot.works) {
      const index = data.get(`member-${w.workRef}`);
      if (index === null || index === "") continue;
      const rationale = String(data.get(`reason-${w.workRef}`) || "").trim();
      if (!rationale) return dialogFeedback(`请说明“${title(w)}”的分配理由。`);
      const destination = destinations[Number(index)];
      if (!destination) return dialogFeedback("分配的方向编号无效");
      destination.members.push({
        workPublicRef: w.workRef,
        role: String(data.get(`role-${w.workRef}`)),
        rationale,
      });
    }
    if (destinations.some((d) => !d.members.length))
      return dialogFeedback("每个新方向需要至少一篇你显式分配的实际材料。");
    const plan = {
      requestRef: uuid(),
      domainRef: state.domainRef,
      kind,
      sources,
      destinations,
      rationale: String(data.get("rationale")).trim(),
    };
    try {
      await mutations.submit(
        "/api/local/topic-map/structure/preview",
        plan,
        decodePreview,
        async (preview) => {
          activeDialog.plan = plan;
          activeDialog.preview = preview;
          if (dialogState === activeDialog) drawDialog();
        },
      );
    } catch (caughtError) {
      const error = caught(caughtError);
      dialogFeedback(`预览未完成：${error.message}`);
    }
  }

  async function submitDialog(event: FormEvent) {
    const submissionDialog = dialogState,
      submissionDomain = state.domainRef;
    const feedback = (text: string) => {
      if (
        dialogState === submissionDialog &&
        state.domainRef === submissionDomain
      )
        dialogFeedback(text);
    };

    const activeDialog = requireDialog();
    event.preventDefault();
    const form = event.target;
    if (!(form instanceof HTMLFormElement)) return;
    const data = new FormData(form),
      name = form.dataset.form;
    if (name === "replace-samples") {
      const refs = [...new Set(data.getAll("workRefs").map(String))],
        parent = activeDialog.judgeState;
      if (!parent) return;
      if (refs.length > 10) return feedback("本次最多10篇独立作品。");
      const allowed = new Map(
        topicOverview.replacementCandidates(parent).map((w) => [w.workRef, w]),
      );
      if (refs.some((ref) => !allowed.has(ref)))
        return feedback("所选作品不属于当前主题的合格判断范围，请重新核对。");
      const works = refs
        .map((ref) => allowed.get(ref))
        .filter((w): w is T.Work => Boolean(w));
      parent.works = works;
      delete parent.inlineReader;
      delete parent.sampleRef;
      parent.tab = "samples";
      dialogBack();
      if (works[0]) await readSample(works[0].workRef);
      return;
    }
    if (name === "material-search" || name === "saved-search") {
      activeDialog.query = String(data.get("query") || "").trim();
      drawDialog();
      return;
    }
    if (name === "own-creator") {
      const [platform, authorExternalId] = String(data.get("creator")).split(
        "|",
      );
      const result = await command(
        { action: "ownCreator", platform, authorExternalId, active: true },
        "我方账号范围已保存",
      );
      if (result) {
        await refreshIdentity();
        feedback(`已保存 · 回执 ${result.receiptRef}`);
      } else feedback(notice);
      return;
    }
    if (name === "rule") {
      const platform = form.dataset.platform,
        current = platformStats(requireSnapshot().statistics).find(
          (p) => p.platform === platform,
        );
      const result = await command(
        {
          action: "performanceRule",
          platform,
          likeThreshold: Number(data.get("threshold")),
          expectedVersion: current?.highPerformanceRuleVersion ?? null,
        },
        "平台样本规则已保存",
      );
      if (result) {
        drawDialog();
        feedback(`规则已保存 · 版本 ${result.revision}`);
      } else feedback(notice);
      return;
    }
    if (name === "save") {
      const current = activeDialog,
        a = current.angle,
        t = topic(current.topicRef);
      if (!a || !a.workRef) return feedback("保存依据尚未取得");
      if (!t && !current.manual)
        return feedback("原主题定义尚未读取，当前不能保存。");
      const product = activeDialog.kind === "product_research";
      const savedAngle = product
        ? String(data.get("angle")).trim()
        : `切入角度：${String(data.get("angle")).trim()}\n\n复用意图：${data.getAll("reuse").join("、") || "未指定"}`;
      const savedRationale = product
        ? String(data.get("rationale")).trim()
        : `这篇要回答的问题：\n${String(data.get("rationale")).trim()}\n\n回答边界：\n${String(data.get("boundary") || "").trim() || "尚未明确，需要核对原作与适用条件"}\n\n备注：\n${String(data.get("note") || "").trim() || "未填写"}`;
      if (
        Array.from(savedAngle).length > 2000 ||
        Array.from(savedRationale).length > 2000
      )
        return feedback(
          "完整角度或研究说明超过保存上限，请缩短问题、边界或备注后再保存。",
        );
      const result = await command(
        {
          action: "saveAlternative",
          idempotencyKey:
            current.saveRequestKey || (current.saveRequestKey = uuid()),
          topicRef: t?.topicRef || null,
          definitionRef: t?.definitionRef || null,
          title: String(data.get("title")).trim(),
          angle: savedAngle,
          rationale: savedRationale,
          evidenceWorkRefs: evidenceWorks(a, a.workRef),
          methodVersion:
            a.researchMethodVersion || requireSnapshot().methodVersion,
          researchResultRef: a.researchResultRef || null,
          researchAngleIndex:
            a.researchResultRef && activeDialog.kind !== "product_research"
              ? a.researchAngleIndex
              : null,
          researchOpportunityIndex:
            a.researchResultRef && activeDialog.kind === "product_research"
              ? a.researchOpportunityIndex
              : null,
        },
        "备选已保存到后端，不启用跟踪",
      );
      if (result) {
        current.savedAlternativeRef = result.subjectRef || undefined;
        if (dialogState === current) {
          drawDialog();
          feedback(`已保存 · 回执 ${result.receiptRef}`);
        }
      } else feedback(notice);
      return;
    }

    if (name === "freeze-details") {
      const refs = data.getAll("workRefs");
      if (!refs.length || refs.length > 10)
        return feedback("请从本轮候选显式选择1到10篇独立作品。");
      const result = await collectionCommand(
        {
          action: "freeze_search_details",
          roundRef: form.dataset.round,
          workRefs: refs,
        },
        true,
      );
      if (result) await refreshCapture();
      return;
    }
    if (name === "capture") {
      await submitCapture(form, data);
      return;
    }
    if (name === "move-topic") {
      const t = topic(String(data.get("topicRef")));
      if (!t) return feedback("先选中本域主题。");
      const result = await command(
        {
          action: "bindTopic",
          topicRef: t.topicRef,
          parentTopicRef: String(data.get("parentTopicRef")) || null,
          expectedVersion: t.bindingVersion ?? null,
        },
        "主题导航归属已保存",
      );
      if (result) {
        drawDialog();
        feedback(`归属已保存 · 版本 ${result.revision}`);
      } else feedback(notice);
      return;
    }
    if (name === "structure") {
      await previewStructure(form, data);
      return;
    }
  }
  async function refreshResearch() {
    if (!dialogState) return;
    const current = requireDialog();
    try {
      const value = await readResearchProgress(state.domainRef);
      if (value && dialogState === current) {
        progress = value;
        delete current.error;
        drawDialog();
      }
    } catch (caughtError) {
      const error = caught(caughtError);
      if (dialogState === current)
        dialogFeedback(`进度读取未成功：${error.message}`);
    }
  }

  async function collectionCommand(
    payload: Record<string, unknown>,
    search = false,
  ) {
    if (pending) return null;
    pending = true;
    const owner = dialogState,
      domain = state.domainRef;
    const current = () =>
      !disposed && dialogState === owner && state.domainRef === domain;
    const feedback = (text: string) => {
      if (current()) dialogFeedback(text);
    };
    feedback("正在提交有界动作…");
    try {
      const result = await mutations.submit(
        search
          ? "/api/local/topic-map/search/commands"
          : "/api/local/topic-map/collection/commands",
        { ...payload, requestRef: uuid(), domainRef: domain },
        decodeCollectionReceipt,
        async (result) => {
          feedback(
            `已获得后端回执：${labelState(result.state)}。实际取得材料与研究结果不由提交成功代替。`,
          );
        },
      );
      return current() ? result : null;
    } catch (caughtError) {
      const error = caught(caughtError);
      feedback(`动作未完成：${error.message}`);
      return null;
    } finally {
      pending = false;
      notify();
    }
  }
  async function submitCapture(form: HTMLFormElement, data: FormData) {
    const activeDialog = requireDialog();
    const purpose = String(data.get("purpose")).trim(),
      newRound = data.has("newRound");
    const payload = activeDialog.workRef
      ? {
          action: "deepen_comments",
          topicRef: activeDialog.topicRef || null,
          targetRef: String(data.get("targetRef")),
          workRef: activeDialog.workRef,
          purpose,
          newRound,
        }
      : {
          action: "start_search",
          topicRef: activeDialog.topicRef || null,
          authorizationRef: String(data.get("authorizationRef")),
          keywords: String(data.get("keywords"))
            .split(/[,，、\n]/)
            .map((s) => s.trim())
            .filter(Boolean),
          purpose,
          newRound,
        };
    if (
      payload.keywords &&
      (payload.keywords.length < 1 || payload.keywords.length > 3)
    )
      return dialogFeedback("本轮须明确1到3个搜索词。");
    const result = await collectionCommand(payload, !activeDialog.workRef);
    if (result) await refreshCapture();
  }
  async function refreshCapture() {
    const current = requireDialog();
    try {
      const results = await Promise.allSettled([
        request(
          `/api/local/topic-map/collection?domainRef=${encodeURIComponent(state.domainRef)}`,
          decodeCollectionProgress,
        ),
        request(
          `/api/local/topic-map/search?domainRef=${encodeURIComponent(state.domainRef)}`,
          decodeSearch,
        ),
        request(
          `/api/local/topic-map?${query({ topicRef: "", windowDays: "", platform: "", path: "", overlay: "" })}`,
          decodeSnapshot,
        ),
      ]);
      if (dialogState !== current) return;
      if (results[0].status === "fulfilled")
        current.collection = results[0].value;
      else current.collectionError = results[0].reason.message;
      if (results[1].status === "fulfilled") current.search = results[1].value;
      else current.searchError = results[1].reason.message;
      if (results[2].status === "fulfilled")
        results[2].value.works.forEach((w) => extraWorks.set(w.workRef, w));
      current.loading = false;
      drawDialog();
      if (current.collectionError || current.searchError)
        dialogFeedback(
          `部分进度读取未成功：${current.collectionError || current.searchError}`,
        );
    } catch (caughtError) {
      const error = caught(caughtError);
      if (dialogState === current) {
        current.loading = false;
        current.error = error.message;
        drawDialog();
      }
    }
  }
  async function openCapture(
    ref: string | undefined,
    workRef: string | null = null,
  ) {
    openDialog("capture", { topicRef: ref, workRef, loading: true });
    await refreshCapture();
  }
  async function dispatchClick(event: { target: EventTarget | null }) {
    if (!(event.target instanceof Element)) return;
    const target = event.target.closest<HTMLElement>("[data-action]");
    if (!target || !inWorkspace(target)) return;
    const action = target.dataset.action || "",
      id = target.dataset.id || "";
    if (action === "mutation-retry") {
      const owner = dialogState,
        domain = state.domainRef;
      try {
        await mutations.retry();
        renderNotice();
        if (dialogState !== owner || state.domainRef !== domain) return;
        if (dialogState?.type === "identity") await refreshIdentity();
        else if (dialogState?.type === "research") await refreshResearch();
        else if (dialogState?.type === "capture") await refreshCapture();
      } catch (error) {
        if (dialogState === owner && state.domainRef === domain)
          dialogFeedback(`原请求未完成：${caught(error).message}`);
      }
      return;
    }
    if (target instanceof HTMLButtonElement && target.disabled) return;
    if (action === "refresh") {
      await load();
      return;
    }
    if (action === "research") {
      await openResearch();
      return;
    }
    if (action === "research-refresh") {
      await refreshResearch();
      return;
    }
    if (action === "dialog-close") {
      closeDialog();
      return;
    }
    if (action === "dialog-back") {
      dialogBack();
      return;
    }
    if (action === "dialog-retry" && dialogState?.type === "research") {
      await openResearch();
      return;
    }
    const currentSnapshot = snapshot;
    if (!currentSnapshot) return;
    if (action === "toggle-navigation") {
      state.navigationOpen = !state.navigationOpen;
      persist();
      render();
      return;
    }
    if (action === "clear-filter") {
      if (!["topicRef", "stage", "path", "overlay", "query"].includes(id))
        return;
      if (
        id === "topicRef" ||
        id === "stage" ||
        id === "path" ||
        id === "overlay" ||
        id === "query"
      )
        state[id] = "";
      state.tab = "content";
      persist();
      if (["topicRef", "path", "overlay"].includes(id)) await load();
      else render();
      return;
    }
    if (action === "reset-scope" || action === "browse-pending") {
      if (action === "browse-pending") switchView("journey");
      Object.assign(state, {
        topicRef: "",
        platform: "",
        windowDays: "",
        path: "",
        overlay: "",
        query: "",
        stage: action === "browse-pending" ? "pending" : "",
        materialMode: "latest",
        tab: "content",
      });
      state[
        state.view === "journey" ? "journeyWindowDays" : "overviewWindowDays"
      ] = "";
      persist();
      await load();
      return;
    }
    if (action === "manual-save") {
      const w = work(id);
      if (!w?.readable) return;
      openDialog("save", {
        manual: true,
        topicRef: "",
        kind: "angle",
        angle: {
          workRef: id,
          evidenceWorkRefs: [id],
          title: title(w),
          label: "",
          answerTask: "",
          researchMethodVersion: "topic-map.manual.v1",
        },
      });
      return;
    }
    if (action === "view") {
      if (!switchView(id)) return;
      persist();
      await load();
      return;
    }
    if (action === "summary-chart") {
      state.summaryChart = id;
      persist();
      render();
      return;
    }
    if (action === "changes-mode") {
      state.changesOnly = id === "changes";
      const viewChanged = state.changesOnly && switchView("overview");
      if (state.changesOnly) state.tab = "structure";
      persist();
      if (viewChanged) await load();
      else render();
      return;
    }
    if (action === "tab") {
      state.tab = id;
      persist();
      render();
      return;
    }
    if (action === "topic") {
      state.topicRef = id;
      state.stage = "";
      resetMainScroll = true;
      persist();
      if (dialog) closeDialog();
      await load();
      return;
    }
    if (action === "collapse") {
      const values = new Set(list(state.collapsed));
      if (values.has(id)) values.delete(id);
      else values.add(id);
      state.collapsed = [...values];
      persist();
      render();
      return;
    }
    if (action === "material-mode") {
      state.materialMode = id;
      persist();
      render();
      return;
    }
    if (action === "stage") {
      state.stage = state.stage === id ? "" : id;
      persist();
      render();
      return;
    }
    if (action === "overlay") {
      state.overlay = id;
      state.stage = "";
      persist();
      await load();
      return;
    }
    if (action === "metric") {
      state.metric = id;
      persist();
      render();
      return;
    }
    if (action === "map-mode") {
      state.mapMode = id;
      persist();
      render();
      return;
    }
    if (action === "definition") {
      openDialog("definition", { topicRef: id });
      return;
    }
    if (action === "scene-more") {
      state.showAllScenes = !state.showAllScenes;
      persist();
      render();
      return;
    }
    if (action === "clear-scene-query") {
      state.sceneQuery = "";
      persist();
      render();
      return;
    }
    if (action === "scope-materials") {
      openDialog("materials", { query: "", label: "当前统计范围的材料" });
      return;
    }
    if (action === "stage-materials") {
      const j = topic(state.topicRef)?.journey || currentSnapshot.journey,
        e = topicOverview
          .journeyEntries(j, stageNames[id] ? state.metric : "main")
          .find((e) => e.stage === id);
      openDialog("materials", {
        refs: id ? list(e?.workRefs) : selectedWorks().map((w) => w.workRef),
        query: "",
        label: id ? stageNames[id] || otherNames[id] : "全部阶段材料",
      });
      return;
    }
    if (action === "matrix") {
      await openMatrix();
      return;
    }
    if (action === "compare") {
      await openCompare();
      return;
    }
    if (action === "compare-clear") {
      state.compare = [];
      persist();
      render();
      return;
    }
    if (action === "materials") {
      openDialog("materials", {
        snapshot: dialogState?.snapshot,
        query: state.query,
      });
      return;
    }
    if (action === "subset") {
      openDialog("materials", { refs: id ? id.split(",") : [] });
      return;
    }
    if (action === "unassigned") {
      openDialog("materials", { unassigned: true });
      return;
    }
    if (action === "materials-more") {
      requireDialog().limit = (requireDialog().limit || 60) + 60;
      drawDialog();
      return;
    }
    if (action === "reader") {
      await readWork(id);
      return;
    }
    if (action === "comments-more") {
      const owner = requireDialog(),
        current = owner.type === "judge" ? owner.inlineReader : owner;
      if (!current?.commentResource?.nextCursor || !isReader(current)) return;
      try {
        const next = await request(
          `/api/local/work-resources/${encodeURIComponent(current.work.workRef)}/comments?domain=${encodeURIComponent(state.domainRef)}&cursor=${encodeURIComponent(current.commentResource.nextCursor)}`,
          decodeComments,
        );
        if (
          dialogState === owner &&
          (owner.type !== "judge" || owner.inlineReader === current)
        ) {
          current.commentResource = {
            ...next,
            items: [
              ...list(current.commentResource.items),
              ...list(next.items),
            ],
          };
          drawDialog();
        }
      } catch (caughtError) {
        const error = caught(caughtError);
        if (dialogState === owner)
          dialogFeedback(`已有评论读取未成功：${error.message}`);
      }
      return;
    }

    if (action === "judge") {
      openDialog("judge", {
        topicRef: id,
        works: topicOverview.curated(id),
        tab: "discussion",
        loading: true,
      });
      const current = requireDialog();
      try {
        const full = await request(
          `/api/local/topic-map?${query({ topicRef: id, windowDays: "", platform: "" })}`,
          decodeSnapshot,
        );
        full.works.forEach((w) => extraWorks.set(w.workRef, w));
        if (dialogState === current) {
          current.works = topicOverview.curated(id, full);
          current.snapshot = full;
          current.loading = false;
          drawDialog();
        }
      } catch (caughtError) {
        const error = caught(caughtError);
        if (dialogState === current) {
          current.loading = false;
          current.readError = error.message;
          drawDialog();
        }
      }
      return;
    }
    if (action === "judge-tab") {
      requireDialog().tab = id;
      drawDialog();
      if (id === "samples" && !requireDialog().inlineReader) {
        const first = requireDialog().works?.[0];
        if (first) await readSample(first.workRef);
      }
      return;
    }
    if (action === "sample") {
      await readSample(id);
      return;
    }
    if (action === "replace-samples") {
      openDialog("replace", { judgeState: requireDialog() });
      requireDialog().judgeState = dialogStack[dialogStack.length - 1];
      drawDialog();
      return;
    }
    if (action === "judge-product") {
      openDialog("product", {
        topicRef: requireDialog().topicRef,
        works: requireDialog().works,
      });
      return;
    }
    if (action === "judge-sources") {
      openDialog("materials", {
        snapshot: requireDialog().snapshot || currentSnapshot,
        refs: list(requireDialog().works)
          .filter((w) => !w.own)
          .map((w) => w.workRef),
        label: "当前精选外部参考来源",
      });
      return;
    }
    if (action === "judge-to-journey") {
      state.topicRef = id;
      switchView("journey");
      state.stage = "";
      closeDialog();
      persist();
      await load();
      return;
    }
    if (action === "own-reuse") {
      const w = work(id);
      if (!w) return;
      if (!readableMaterial(w))
        return dialogFeedback(
          "当前没有可引用的来源，暂不能保留这篇作品的复用依据。",
        );
      openDialog("save", {
        topicRef: requireDialog().topicRef,
        kind: "angle",
        angle: {
          workRef: w.workRef,
          title: title(w),
          label: `复用我方已发布作品：${title(w)}`,
          answerTask: "",
          researchMethodVersion: currentSnapshot.methodVersion,
        },
      });
      return;
    }
    if (action === "judge-own-all") {
      const source = requireDialog().snapshot || currentSnapshot,
        refs = topicOverview
          .ownTopicWorks(requireDialog().topicRef, source)
          .map((w) => w.workRef);
      openDialog("materials", {
        snapshot: source,
        refs,
        label: "本主题全部已确认我方发布原作",
      });
      return;
    }
    if (action === "identity") {
      await openIdentity();
      return;
    }
    if (action === "structure-add-destination") {
      requireDialog().destinationCount = Math.min(
        10,
        (requireDialog().destinationCount || 2) + 1,
      );
      drawDialog();
      return;
    }
    if (action === "structure-edit") {
      delete requireDialog().preview;
      requireDialog().structureApplied = false;
      drawDialog();
      return;
    }
    if (action === "structure-apply") {
      const active = requireDialog();
      if (!active.plan || !active.preview) return;
      try {
        await mutations.submit(
          "/api/local/topic-map/structure/apply",
          { plan: active.plan, previewHash: active.preview.previewHash },
          decodeStructure,
          async (result) => {
            active.structureApplied = true;
            await load();
            if (dialogState === active)
              dialogFeedback(
                `结构调整已保存 · ${result.destinations.length} 个新候选定义。旧历史与备选引用保留。`,
              );
          },
        );
      } catch (error) {
        dialogFeedback(`结构调整未完成：${caught(error).message}`);
      }
      return;
    }
    if (action === "move-topic") {
      openDialog("move", { moveTopicRef: state.topicRef });
      return;
    }
    if (action === "structure-change") {
      openDialog("structure", { kind: "merge", destinationCount: 1 });
      return;
    }
    if (action === "viewed") {
      const t = topic(id);
      if (!t) return;
      const result = await command(
        {
          action: "viewed",
          topicRef: id,
          definitionRef: t.definitionRef,
          workRefs:
            list(currentSnapshot.changes).find((c) => c.topicRef === id)
              ?.lastViewedWorkRefs || [],
        },
        "已记录此主题定义的查看",
      );
      if (result) drawDialog();
      return;
    }
    if (action === "rules" || action === "method" || action === "changes") {
      openDialog(action);
      return;
    }
    if (action === "saved") {
      await openSaved();
      return;
    }
    if (action === "saved-materials") {
      await openAlternative(id);
      return;
    }
    if (action === "export-alternative") {
      await exportAlternative(id);
      return;
    }
    if (action === "matrix-cell") {
      const [ref, stage] = id.split("|"),
        source = requireDialog().snapshot;
      if (!source) return;
      const t = list(source.topics).find((t) => t.topicRef === ref);
      if (!t) return;
      const refs =
        stage === "other"
          ? list(t.journey.main)
              .filter((e) => !stageNames[e.stage])
              .flatMap((e) => list(e.workRefs))
          : list(
              requireDialog().metric === "involved"
                ? t.journey.involved
                : t.journey.main,
            ).find((e) => e.stage === stage)?.workRefs || [];
      openDialog("materials", {
        refs: [...new Set(refs)],
        snapshot: source,
        label: `${t.displayName} · ${stageNames[stage || ""] || "其他 / 未归主阶段"}`,
      });
      return;
    }
    if (action === "save-angle") {
      const angles = topicDialogs.anglesFrom(list(requireDialog().works));
      const a = angles[Number(id)];
      if (a)
        openDialog("save", {
          angle: a,
          topicRef: requireDialog().topicRef,
          kind: "angle",
        });
      return;
    }
    if (action === "save-opportunity") {
      const opportunities = topicDialogs.opportunitiesFrom(
          analysisOutputs(requireDialog().works || selectedWorks()),
        ),
        o = opportunities[Number(id)];
      if (!o) return;
      const angle = {
        ...o,
        title: Array.from(o.need || "")
          .slice(0, 120)
          .join(""),
        label: `具体需求：${o.need}\n\n支持假设：${o.hypothesis}`,
        answerTask: `最先验证：${o.verificationQuestion}\n\n替代解释：${o.alternativeExplanation}`,
      };
      openDialog("save", {
        angle,
        topicRef: requireDialog().topicRef || state.topicRef,
        kind: "product_research",
      });
      return;
    }
    if (action === "breakout") {
      const current = requireDialog(),
        w = work(id);
      if (!w) return;
      const result = await command(
        { action: "breakout", workRef: id, marked: !w.ownBreakout },
        "我方爆款标记已保存",
      );
      if (result && dialogState === current) {
        current.work = work(id);
        const refreshed = work(id);
        if (
          current.type === "judge" &&
          current.inlineReader?.work.workRef === id &&
          refreshed
        )
          current.inlineReader.work = refreshed;
        drawDialog();
        dialogFeedback("手动标记已获得后端回执。未标记不等于表现低。");
      } else if (dialogState === current) dialogFeedback(notice);
      return;
    }
    if (action === "own-remove") {
      const owner = dialogState;
      const [platform, authorExternalId] = id.split("|");
      const result = await command(
        { action: "ownCreator", platform, authorExternalId, active: false },
        "我方账号范围已更新",
      );
      if (result) await refreshIdentity();
      else if (dialogState === owner) dialogFeedback(notice);
      return;
    }
    if (action === "capture-configure") {
      const policy=requireDialog().collection?.policy;
      if (!policy) return;
      const result=await collectionCommand({action:'configure',expectedRevision:policy.revision,collectionEnabled:!policy.enabled});
      if (result) await refreshCapture();
      return;
    }
    if (action === "capture-refresh") {
      await refreshCapture();
      return;
    }
    if (action === "search-stop") {
      const result = await collectionCommand(
        { action: "stop_search", roundRef: id },
        true,
      );
      if (result) await refreshCapture();
      return;
    }
    if (action === "collection-stop") {
      const result = await collectionCommand({
        action: "stop_collection",
        roundRef: id,
      });
      if (result) await refreshCapture();
      return;
    }
    if (action === "capture") {
      await openCapture(id);
      return;
    }
    if (action === "deep-comments") {
      await openCapture(state.topicRef, id);
      return;
    }
    if (action === "dialog-retry") {
      const current = requireDialog();
      if (current.type === "matrix") await openMatrix();
      else if (current.type === "saved") await openSaved();
      else if (current.type === "reader" && current.work)
        await readWork(current.work.workRef);
      else if (current.type === "capture")
        await openCapture(current.topicRef, current.workRef || null);
      else if (current.type === "alternative" && current.alternativeRef)
        await openAlternative(current.alternativeRef);
      return;
    }
  }

  function onChange(event: ChangeEvent) {
    const target = event.target;
    if (
      !(
        target instanceof HTMLInputElement ||
        target instanceof HTMLSelectElement
      )
    )
      return;
    if (dialogState && dialog?.contains(target)) {
      if (
        target instanceof HTMLInputElement &&
        target.hasAttribute("data-sample-choice") &&
        dialog.querySelectorAll("[data-sample-choice]:checked").length > 10
      ) {
        target.checked = false;
        dialogFeedback("本次精选最多10篇独立作品，可先取消现有样本再替换。");
        return;
      }
      if (
        target instanceof HTMLInputElement &&
        target.dataset.detailChoice &&
        dialog.querySelectorAll(
          '[data-detail-choice="' + target.dataset.detailChoice + '"]:checked',
        ).length > 10
      ) {
        target.checked = false;
        dialogFeedback("一轮最多冻结10篇独立作品详情。");
        return;
      }
      if (
        target instanceof HTMLInputElement &&
        target.dataset.structureSource
      ) {
        const set = new Set(list(dialogState.sources));
        if (target.checked) set.add(target.dataset.structureSource);
        else set.delete(target.dataset.structureSource);
        dialogState.sources = [...set];
        drawDialog();
        return;
      }
      if (target.dataset.dialogFilter) {
        const key = target.dataset.dialogFilter;
        if (
          key === "query" ||
          key === "rows" ||
          key === "metric" ||
          key === "kind" ||
          key === "moveTopicRef"
        )
          dialogState[key] = target.value;
        else if (
          key === "sort" &&
          ["latest", "likes", "collects", "comments", "shares"].includes(
            target.value,
          )
        )
          dialogState.sort = target.value as T.DialogState["sort"];
        drawDialog();
        return;
      }
    }
    if (target instanceof HTMLInputElement && target.dataset.compare) {
      const values = new Set(list(state.compare));
      if (target.checked) values.add(target.dataset.compare);
      else values.delete(target.dataset.compare);
      if (values.size > 3) {
        target.checked = false;
        notice = "一次最多比较三个方向，保留可读的并排信息。";
        renderNotice();
        return;
      }
      state.compare = [...values];
      persist();
      render();
      return;
    }
    if (target.dataset.filter === "plotPlatform") {
      state.plotPlatform = target.value;
      persist();
      render();
      return;
    }
    if (
      target.dataset.filter &&
      urlKeys.includes(target.dataset.filter as T.TextStateKey)
    ) {
      state[target.dataset.filter as T.TextStateKey] = target.value;
      if (target.dataset.filter === "windowDays")
        state[
          state.view === "journey" ? "journeyWindowDays" : "overviewWindowDays"
        ] = target.value;
      if (
        ["topicRef", "platform", "windowDays", "path", "overlay"].includes(
          target.dataset.filter,
        )
      ) {
        state.stage = "";
        resetMainScroll = true;
      }
      if (target.dataset.filter === "domainRef") {
        state.topicRef = "";
        state.compare = [];
      }
      persist();
      void load();
    }
  }
  function onSubmit(event: FormEvent) {
    const form = event.target;
    if (!(form instanceof HTMLFormElement)) return;
    if (form.closest("dialog")) {
      void submitDialog(event);
      return;
    }
    if (form.dataset.form === "scene-search") {
      event.preventDefault();
      state.sceneQuery = String(new FormData(form).get("query") || "").trim();
      state.showAllScenes = false;
      persist();
      render();
      return;
    }
    if (form.dataset.form === "search") {
      event.preventDefault();
      state.query = String(new FormData(form).get("query") || "").trim();
      state.tab = "content";
      persist();
      render();
    }
  }
  return {
    subscribe(listener: () => void) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    getVersion: () => revision,
    getMainVersion: () => mainRevision,
    getNotice: () => notice,
    isMutationBusy: () => pending || mutations.busy,
    consumeMainScroll() {
      const value = mainScrollRequest;
      mainScrollRequest = null;
      return value;
    },
    getContent,
    getDialogContent,
    getViewTabs: () => (
      <>
        {tab("主题概览", "view", "overview", state.view === "overview")}
        {tab("用户生命旅程", "view", "journey", state.view === "journey")}
      </>
    ),
    getContextActions: () => (
      <>
        {mutationRetry()}
        {btn(
          "我的备选",
          "saved",
          "",
          "lgi-tm-quiet",
          !snapshot ? "disabled" : "",
        )}
        <div
          data-readiness=""
          className={cls("lgi-tm-readiness")}
          role={"status"}
        >
          {loading
            ? btn("研究进度", "research", "", "lgi-tm-quiet")
            : readiness()}
        </div>
        {btn("刷新已有结果", "refresh", "", "", loading ? "disabled" : "")}
      </>
    ),
    getState: () => state,
    getSnapshot: () => snapshot,
    getDialog: () => dialogState,
    attachRoot(node: HTMLDivElement | null) {
      root = node;
    },
    attachDialog(node: HTMLDialogElement | null) {
      dialog = node;
    },
    onClick: dispatchClick,
    onChange,
    onSubmit,
    closeDialog,
    onToggle(event: ToggleEvent) {
      if (
        event.target instanceof HTMLDetailsElement &&
        event.target.hasAttribute("data-tree-legacy")
      ) {
        state.legacyExpanded = event.target.open;
        persist(true);
      }
    },
    onKeyDown(event: KeyboardEvent) {
      if (!(event.target instanceof Element)) return;
      const target = event.target.closest("g[data-action]");
      if (target && ["Enter", " "].includes(event.key)) {
        event.preventDefault();
        void dispatchClick({ target });
      }
    },
    async activate(defaultDomain: string) {
      disposed = false;
      bootstrapDomain = defaultDomain;
      if (!state.domainRef) state.domainRef = defaultDomain;
      await load();
    },
    onLocation() {
      const params = new URLSearchParams(location.search);
      Object.assign(state, defaults, {
        collapsed: list(state.collapsed),
        navigationOpen: state.navigationOpen,
        legacyExpanded: state.legacyExpanded,
      });
      resetMainScroll = true;
      urlKeys.forEach((key) => {
        if (params.has(key)) state[key] = params.get(key) || "";
      });
      if (params.has("domain")) state.domainRef = params.get("domain") || "";
      if (!state.domainRef) state.domainRef = bootstrapDomain;
      normalizeState();
      if (dialogState) closeDialog();
      void load();
    },
    consumeDialogScroll() {
      const scroll = dialogScroll;
      dialogScroll = null;
      return scroll;
    },
    consumeScrollReset() {
      const value = resetMainScroll;
      resetMainScroll = false;
      return value;
    },
    dispose() {
      disposed = true;
      fetchEpoch++;
      progressEpoch++;
      controllers.forEach((c) => c.abort());
      controllers.clear();
      listeners.clear();
    },
  };
}
