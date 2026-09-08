(() => {
  "use strict";
  const main = document.querySelector("[data-initial-view]");
  if (!main || main.dataset.initialView === "queries") return;
  const $ = (id) => document.getElementById(id),
    esc = (v) =>
      String(v ?? "").replace(
        /[&<>"']/g,
        (c) =>
          ({
            "&": "&amp;",
            "<": "&lt;",
            ">": "&gt;",
            '"': "&quot;",
            "'": "&#39;",
          })[c],
      );
  const api = "/api/local/comment-intelligence",
    old = "/api/local/comment-research";
  const lenses = {
    resonance: "高共鸣",
    conflict: "高冲突",
    need: "用户需求",
    solution: "解决方案",
    quote: "金句",
    story: "故事",
  };
  const status = {
    pending: "待分析",
    running: "分析中",
    succeeded: "已分析",
    failed: "分析失败",
    no_signal: "未提取研究信号",
    context: "需上下文",
    context_missing: "上下文待补",
    low_information: "低信息",
    anomaly: "异常",
    restricted: "当前受限",
    direct: "可分析",
    source_limit: "等待数量额度",
  };
  const num = (v) => (v == null ? "—" : Number(v).toLocaleString("zh-CN"));
  const date = (v, full = false) =>
    v && !Number.isNaN(new Date(v).valueOf())
      ? new Date(v).toLocaleString("zh-CN", {
          timeZone: "Asia/Shanghai",
          ...(full ? { year: "numeric" } : {}),
          month: "2-digit",
          day: "2-digit",
          hour: "2-digit",
          minute: "2-digit",
        })
      : "未知";
  const params = new URLSearchParams(location.search),
    domain = document.body.dataset.corpusDomain || "",
    canResearch = document.body.dataset.corpusDomainOwn !== "false";
  const state = {
    view: params.get("view") || "overview",
    domain,
    days: params.get("days") || "30",
    from: params.get("from") || "",
    to: params.get("to") || "",
    timeBasis: params.get("timeBasis") || "observed",
    text: params.get("text") || "",
    workRef: params.get("workRef") || "",
    lenses: params.get("lenses") || "",
    problemRef: params.get("problemRef") || "",
    term: params.get("term") || "",
    bookmarkedOnly: params.get("bookmarkedOnly") === "true",
    sort: params.get("sort") || "observed",
    offset: Number(params.get("offset")) || 0,
    limit: Number(params.get("limit")) === 50 ? 50 : 20,
    resultRevision: params.get("resultRevision") || "",
    batchRef: params.get("batchRef") || "",
    processingState: params.get("processingState") || "",
    sourceRefs: params.get("sourceRefs") || "",
  };
  if (state.view === "groups") state.view = "problems";
  if (state.view === "assets") {
    state.view = "voices";
    state.bookmarkedOnly = true;
  }
  if (!["overview", "voices", "problems", "daily"].includes(state.view))
    state.view = "overview";
  let chartLens = "",
    chartMode = "count",
    data = null,
    detail = null,
    generation = 0,
    detailGeneration = 0,
    abort = null,
    returnFocus = null,
    command = null,
    savedScroll = 0,
    busyAction = false,
    selectionNotice = "",
    batchDetail = null,
    batchDetailError = "",
    refreshTimer = null,
    requestGeneration = 0,
    voiceScope = null,
    resultsCurrent = true;
  const selected = new Set();
  const selectedSources = new Map();
  const voiceFilterReset = { text: "", workRef: "", lenses: "", problemRef: "", term: "", bookmarkedOnly: false, processingState: "", sourceRefs: "" };
  function rememberVoiceScope() {
    return Object.fromEntries([...Object.keys(voiceFilterReset), "batchRef", "sort", "offset", "limit", "from", "to", "days", "timeBasis"].map((key) => [key, state[key]]));
  }
  if (state.view === "daily") {
    voiceScope = rememberVoiceScope();
    Object.assign(state, voiceFilterReset, { offset: 0 });
  }
  const selectionScopeKeys = ["domain", "days", "from", "to", "timeBasis", "text", "workRef", "lenses", "problemRef", "term", "bookmarkedOnly", "batchRef", "processingState", "sourceRefs"];
  function clearSelectionForScope(patch) {
    if (selected.size && selectionScopeKeys.some((key) => Object.hasOwn(patch, key) && patch[key] !== state[key])) {
      selectionNotice = `筛选范围已改变，已清空先前选择的 ${num(selected.size)} 条评论。`;
      selected.clear();
      selectedSources.clear();
    }
  }
  function selectSource(source, enabled) {
    if (!resultsCurrent) return;
    if (enabled) {
      selected.add(source.sourceRef);
      selectedSources.set(source.sourceRef, source);
    } else {
      selected.delete(source.sourceRef);
      selectedSources.delete(source.sourceRef);
    }
  }
  const errors = {
    revision_conflict: "记录已更新，请保留输入并重新读取最新版本。",
    model_revision_conflict: "研究设置已被更新，请重新打开设置后保存。",
    invalid_model_command: "设置没有保存，请核对数量与预算范围。",
    model_source_unavailable: "相关来源当前不可读，无法展示或继续处理。",
    invalid_query: "查询条件不正确，请检查日期与筛选范围。",
    research_selection_limit:
      "本次手动研究最多100条，请缩小范围或选取试跑；每日持续研究请使用研究设置。",
    source_unavailable: "来源当前不可读，已停止展示正文与衍生结果。",
    comment_intelligence_schema_missing:
      "评论研究升级尚未应用，请完成数据库升级后重试。",
    model_not_qualified: "研究模型尚未通过当前合同测试，请先完成模型配置。",
    invalid_command: "请核对必填内容、选择范围及版本。",
    prepare_expired: "确认范围已经过期，请关闭后重新准备研究。",
  };
  async function request(url, body, signal) {
    const r = await fetch(url, {
      method: body ? "POST" : "GET",
      cache: "no-store",
      signal,
      headers: body ? { "Content-Type": "application/json" } : {},
      body: body ? JSON.stringify(body) : undefined,
    });
    const p = await r.json().catch(() => ({}));
    if (!r.ok)
      throw new Error(
        errors[p.error] || `请求未完成（${r.status}），请保留输入后重试。`,
      );
    return p;
  }
  function query(overrides = {}) {
    const q = new URLSearchParams();
    Object.entries({ ...state, ...overrides })
      .filter(([k]) => k !== "scroll")
      .forEach(([k, v]) => {
        if (v !== "" && v !== false && v != null) q.set(k, String(v));
      });
    return q;
  }
  function url() {
    history.replaceState(
      { ...state, scroll: main.scrollTop },
      "",
      `${location.pathname}?${query()}`,
    );
  }
  function feedback(text, error = false) {
    $("feedback").textContent = text;
    $("feedback").classList.toggle("lgi-research-error", error);
  }
  function empty(text) {
    return `<p class="lgi-research-empty">${esc(text)}</p>`;
  }
  const btn = (text, action, extra = "") =>
    `<button type="button" data-ci="${action}" ${extra}>${text}</button>`;
  const linkWork = (s) =>
    `/corpus/evidence?domain=${encodeURIComponent(domain)}&work=${encodeURIComponent(s.workRef)}`;
  function markedBody(body) {
    const text = String(body || "正文尚未取得");
    if (!state.term) return esc(text);
    const at = text.indexOf(state.term);
    return at < 0
      ? esc(text)
      : esc(text.slice(0, at)) +
          "<mark>" +
          esc(text.slice(at, at + state.term.length)) +
          "</mark>" +
          esc(text.slice(at + state.term.length));
  }
  function tags(items, compact = false) {
    const values = (items || []).filter((k) => lenses[k]);
    return values.length
      ? values
          .slice(0, compact ? 2 : values.length)
          .map((k) => `<span class="ci-tag">${lenses[k]}</span>`)
          .join(" ") +
          (compact && values.length > 2
            ? `<span class="ci-tag" title="${esc(values.map((k) => lenses[k]).join("、"))}">+${values.length - 2}</span>`
            : "")
      : "";
  }
  function bookmark(s) {
    return `<button type="button" class="ci-bookmark" data-ci="bookmark" data-ref="${esc(s.sourceRef)}" aria-label="${s.bookmarked ? "取消收藏" : "收藏评论"}" aria-pressed="${Boolean(s.bookmarked)}">${s.bookmarked ? "★" : "☆"}</button>`;
  }
  main.innerHTML = `${document.body.dataset.researchSynthetic === "true" ? '<p class="ci-notice">合成验收环境 · SYNTHETIC / NOT EVIDENCE，以下不是实际研究数据。</p>' : ""}<div class="lgi-research-heading"><h1>评论研究</h1><div class="ci-heading-actions"><label class="ci-sr" for="ci-days">时间范围</label><select id="ci-days"><option value="7">近7天</option><option value="30">近30天</option><option value="custom">自定义</option></select>${canResearch ? btn("研究设置", "settings") : ""}${btn("刷新", "refresh")}</div></div><nav class="lgi-research-tabs" aria-label="评论研究视图">${Object.entries(
    {
      overview: "概览",
      voices: "原声",
      problems: "用户问题",
      daily: "每日观察",
    },
  )
    .map(
      ([v, n]) =>
        `<a href="?${query({ view: v, problemRef: "" })}" data-view="${v}">${n}</a>`,
    )
    .join(
      "",
    )}</nav><div id="ci-scope" class="ci-scope"></div><div id="ci-tools"></div><p id="feedback" role="status" aria-live="polite"></p><div id="results" aria-busy="true"></div><div id="ci-selection" hidden></div><div id="ci-pagination"></div><p class="ci-footnote">${canResearch ? "" : "当前外部领域仅浏览，不提供模型研究。"}仅描述已接纳且当前可研究的观察样本，不代表平台总体或需求规模。研究标签须结合原声核验；高级观察在真实校准通过前保持试验状态。</p>`;
  $("ci-days").value = state.from ? "custom" : state.days;
  const inspector = document.createElement("aside");
  inspector.id = "ci-inspector";
  inspector.className = "ci-inspector";
  inspector.hidden = true;
  inspector.setAttribute("aria-labelledby", "ci-inspector-title");
  document.body.append(inspector);
  const modal = $("research-form-dialog");
  modal.innerHTML =
    '<form id="ci-command-form"><div class="lgi-research-dialog-heading"><h2 id="ci-command-title"></h2><button type="button" data-ci="close-modal">取消</button></div><div id="ci-command-fields"></div><p id="ci-command-feedback" role="status" aria-live="polite"></p><button type="submit" id="ci-command-submit">保存</button></form>';
  function alignInspector() {
    const anchor = document.querySelector(".v7-context-row");
    const top = anchor
      ? Math.max(0, anchor.getBoundingClientRect().bottom)
      : main.getBoundingClientRect().top;
    inspector.style.top = `${top}px`;
    inspector.style.setProperty(
      "--ci-content-width",
      `${main.getBoundingClientRect().width}px`,
    );
    inspector.style.setProperty(
      "--ci-content-left",
      `${main.getBoundingClientRect().left}px`,
    );
  }
  const observer = new ResizeObserver(alignInspector);
  observer.observe(main);
  const anchor = document.querySelector(".v7-context-row");
  if (anchor) observer.observe(anchor);
  window.addEventListener("resize", alignInspector);
  window.addEventListener("scroll", alignInspector, { passive: true });
  function closeInspector() {
    const ref = detail?.source.sourceRef;
    inspector.hidden = true;
    detailGeneration++;
    detail = null;
    const target = returnFocus?.isConnected
      ? returnFocus
      : ref
        ? main.querySelector(
            `[data-ci="source"][data-ref="${CSS.escape(ref)}"]`,
          )
        : null;
    target?.focus({ preventScroll: true });
  }
  function showModal(title, html, submit, callback) {
    command = callback;
    requestGeneration++;
    modal.classList.remove("ci-request-dialog");
    $("ci-command-title").textContent = title;
    $("ci-command-fields").innerHTML = html;
    $("ci-command-feedback").textContent = "";
    $("ci-command-submit").textContent = submit;
    $("ci-command-submit").hidden = !callback;
    if (!modal.open) modal.showModal();
  }
  const resultActions = new Set(["select-page", "prepare", "prepare-selected", "bookmark-selected", "candidate-create", "candidate-voices", "problem-create"]);
  function setResultsCurrent(current) {
    resultsCurrent = current;
    const selectors = ["[data-ci-select-page]", "[data-ci-select]", ...[...resultActions].map((a) => `[data-ci="${a}"]`)];
    document.querySelectorAll(selectors.join(",")).forEach((control) => {
      if (!current && !control.disabled) {
        control.dataset.ciLoadingDisabled = "true";
        control.disabled = true;
      } else if (current && control.dataset.ciLoadingDisabled === "true") {
        control.disabled = false;
        delete control.dataset.ciLoadingDisabled;
      }
    });
  }
  async function load() {
    const seq = ++generation;
    setResultsCurrent(false);
    clearTimeout(refreshTimer);
    abort?.abort();
    abort = new AbortController();
    $("results").setAttribute("aria-busy", "true");
    feedback("正在读取当前范围…");
    url();
    try {
      const endpoint =
        state.view === "problems" && state.problemRef
          ? `${api}/problems/${encodeURIComponent(state.problemRef)}`
          : api;
      const v = await request(
        `${endpoint}?${query()}`,
        undefined,
        abort.signal,
      );
      if (seq !== generation) return;
      if (!v.scope || !v.page || !v.summary)
        throw new Error("服务响应缺少范围或计数，未更新页面。");
      if (state.view === "daily" && !state.batchRef && v.daily?.items?.length) {
        clearSelectionForScope({ batchRef: v.daily.items[0].batchRef });
        state.batchRef = v.daily.items[0].batchRef;
        return load();
      }
      batchDetail = null;
      batchDetailError = "";
      if (state.view === "daily" && state.batchRef) {
        try {
          batchDetail = await request(`${old}/daily/${encodeURIComponent(state.batchRef)}?domain=${encodeURIComponent(domain)}`, undefined, abort.signal);
        } catch (e) {
          if (e.name === "AbortError") throw e;
          batchDetailError = e.message;
        }
      }
      if (seq !== generation) return;
      data = v;
      for (const source of v.page.items) if (selected.has(source.sourceRef)) selectedSources.set(source.sourceRef, source);
      state.resultRevision = v.scope.resultRevision || "";
      setResultsCurrent(true);
      render();
      feedback(selectionNotice || (v.scope.updated ? "结果已更新：本页显示最新聚合版本。" : ""));
      selectionNotice = "";
      const currentBatch = v.daily?.items?.find((b) => b.batchRef === state.batchRef);
      if (state.view === "daily" && currentBatch?.enabled && ((currentBatch.counts?.pending || 0) + (currentBatch.counts?.running || 0) > 0)) {
        const refreshWhenVisible = () => {
          if (state.view !== "daily") return;
          if (document.visibilityState === "visible" && !modal.open && inspector.hidden) load();
          else refreshTimer = setTimeout(refreshWhenVisible, 5000);
        };
        refreshTimer = setTimeout(refreshWhenVisible, 5000);
      }
    } catch (e) {
      if (seq !== generation || e.name === "AbortError") return;
      feedback(
        `${e.message}${data ? " 上次结果保留，截止 " + date(data.scope.asOf, true) + "。" : ""}`,
        true,
      );
      if (!data) $("results").innerHTML = empty("数据暂不可读，点击刷新重试。");
    } finally {
      if (seq === generation) $("results").setAttribute("aria-busy", "false");
    }
  }
  function navigate(patch) {
    if (patch.view === "daily" && state.view !== "daily") {
      voiceScope = rememberVoiceScope();
      patch = { ...patch, ...voiceFilterReset, offset: 0 };
    }
    closeInspector();
    savedScroll = main.scrollTop;
    if (
      data?.scope &&
      patch.from === undefined &&
      state.view !== "daily" &&
      patch.view !== "daily"
    ) {
      state.from = data.scope.from;
      state.to = data.scope.to;
    }
    history.replaceState(
      { ...state, scroll: savedScroll },
      "",
      `${location.pathname}?${query()}`,
    );
    clearSelectionForScope(patch);
    Object.assign(state, patch, { offset: patch.offset ?? 0 });
    history.pushState(
      { ...state, scroll: 0 },
      "",
      `${location.pathname}?${query()}`,
    );
    return load().then(() => {
      main.scrollTop = 0;
    });
  }
  function render() {
    main.dataset.view = state.view;
    document
      .querySelectorAll(".lgi-research-tabs a")
      .forEach((a) =>
        a.setAttribute(
          "aria-current",
          a.dataset.view === state.view ? "page" : "false",
        ),
      );
    $("ci-days").disabled = state.view === "daily";
    const s = data.scope;
    $("ci-scope").innerHTML =
      `<div>${esc(s.domainName || document.body.dataset.corpusDomainName || "当前领域")} · ${state.view === "daily" ? "批次范围" : state.timeBasis === "published" ? "按评论发表时间" : "按首次观察时间"}${state.workRef ? " · 已限定作品" : ""}${state.problemRef ? " · 已限定问题" : ""}${state.term ? " · 热词：" + esc(state.term) : ""}${state.sourceRefs ? " · 精确证据集" : ""}${state.text ? " · 检索：" + esc(state.text) : ""} ${state.workRef || state.problemRef || state.term || state.text || state.lenses || state.processingState || state.sourceRefs ? btn("清除筛选", "clear") : ""}<div class="lgi-research-meta">${state.view === "daily" ? "按本批冻结的评论成员查看，不受原声浏览日期筛选影响" : `${esc(date(s.from, true))} 至 ${esc(date(s.to, true))}（不含截止时刻）`}</div></div><div>${data.model?.modelConnected ? "研究模型已连接" : `<a href="/settings/models">${data.model?.modelState === "PAUSED" ? "模型连接已暂停" : data.model?.modelState === "NEEDS_SELECTION" ? "请选择已通过校验的默认模型" : data.model?.modelState === "NEEDS_QUALIFICATION" ? "模型需要测试评论格式" : "模型尚未配置"}</a>`} · 截止 ${esc(date(s.asOf))}<details><summary>范围与版本</summary><p>聚合版本 ${esc(s.resultRevision || "未知")} · ${esc(s.timeBasis || state.timeBasis)}</p><p>按发表时间筛选时排除 ${num(data.summary.excludedPublished)} 条无法可靠解析时间的评论。</p></details></div>`;
    renderTools();
    if (state.view === "overview") renderOverview();
    else if (state.view === "voices")
      $("results").innerHTML = voiceTable(data.page.items);
    else if (state.view === "problems") renderProblems();
    else renderDaily();
    renderSelection();
    renderPagination();
  }
  function renderTools() {
    const show = state.view === "voices" || state.view === "problems";
    $("ci-tools").innerHTML = show
      ? `<form id="ci-search" class="lgi-research-tools"><label class="lgi-research-grow"><span class="ci-sr">检索评论原文</span><input name="text" type="search" value="${esc(state.text)}" placeholder="搜索评论原文或具体表达（Enter 查询）" maxlength="200"></label><label><span class="ci-sr">作品</span><select name="workRef"><option value="">全部作品</option>${(data.works || []).map((w) => `<option value="${esc(w.workRef)}" ${w.workRef === state.workRef ? "selected" : ""}>${esc(w.title || w.workTitle || "标题未知")}</option>`).join("")}${state.workRef && !(data.works || []).some((w) => w.workRef === state.workRef) ? `<option value="${esc(state.workRef)}" selected>当前限定作品</option>` : ""}</select></label><button type="submit">查询</button>${btn("更多筛选", "filters")}${canResearch ? btn("研究当前结果", "prepare", 'class="ci-primary"') : ""}</form><div class="ci-lens-filters">${btn("全部", "lens", 'data-lens="" aria-pressed="' + !state.lenses + '"')}${Object.entries(
          lenses,
        )
          .map(([k, n]) =>
            btn(
              n,
              "lens",
              `data-lens="${k}" aria-pressed="${state.lenses.split(",").includes(k)}"`,
            ),
          )
          .join(
            "",
          )}<span class="ci-flex"></span>${btn("☆ 仅收藏", "bookmarked", `aria-pressed="${state.bookmarkedOnly}"`)}</div><div class="ci-result-line"><span>${num(data.page.total)} 条原声 · 完成分析 ${num(data.summary.analyzed)} / ${num(data.summary.eligible)}（含无信号结果）${state.processingState ? " · " + esc(status[state.processingState] || "待处理") : ""}</span><label>排序 <select id="ci-sort"><option value="observed">最近观察</option><option value="likes" ${state.sort === "likes" ? "selected" : ""}>点赞最多</option></select></label>${btn("保存查询", "save-query")}${state.bookmarkedOnly ? btn("历史收藏", "legacy-assets") : ""}</div>`
      : "";
  }
  function voiceTable(items) {
    if (!items.length)
      return empty(
        "当前范围没有匹配原声。可以清除筛选；尚未采到不代表现实中没有讨论。",
      );
    return `<div class="ci-table-wrap"><table class="ci-voices"><colgroup><col class="ci-col-select"><col><col class="ci-col-tags"><col class="ci-col-work"><col class="ci-col-likes"><col class="ci-col-time"></colgroup><thead><tr><th><input type="checkbox" data-ci-select-page aria-label="选择当前页评论" ${items.every((s) => selected.has(s.sourceRef)) ? "checked" : ""}></th><th>评论原声／回复</th><th>研究标签</th><th>所属作品</th><th class="lgi-research-number">赞</th><th>${state.timeBasis === "published" ? "发表时间" : "首次观察"}</th></tr></thead><tbody>${items.map((s) => `<tr data-row="${esc(s.sourceRef)}"><td><input type="checkbox" data-ci-select="${esc(s.sourceRef)}" aria-label="选择评论" ${selected.has(s.sourceRef) ? "checked" : ""}></td><td><button type="button" class="lgi-voice-open" data-ci="source" data-ref="${esc(s.sourceRef)}"><span class="lgi-voice-preview">${markedBody(s.body)}</span></button>${s.isReply ? `<div class="ci-parent">↳ ${esc(s.parentPreview || "父评论尚未采集")}</div>` : ""}<div class="ci-inline-tags">${tags(s.labels, true)}</div>${s.sourceChanged ? '<span class="lgi-research-meta">原文已变化，原人工判断待复核</span>' : ""}${!s.labels?.length ? `<span class="lgi-research-meta">${esc(status[s.analysisState] || status[s.cleanState] || "待分析")}</span>` : ""}${s.matchLocation ? `<span class="lgi-research-meta">${esc(s.matchLocation)}</span>` : ""}</td><td class="ci-tags-cell">${tags(s.labels, true)}${bookmark(s)}</td><td><a class="lgi-research-source-title" href="${linkWork(s)}">${esc(s.workTitle || "作品标题未知")}</a><div class="ci-author">${esc(s.creatorDisplayName || "作者未知")}</div></td><td class="lgi-research-number" title="${s.likes == null ? "尚未取得点赞" : esc(s.likes)}">${s.likes == null ? "—" : s.likes >= 10000 ? (s.likes / 10000).toFixed(1) + "万" : num(s.likes)}</td><td>${state.timeBasis === "published" ? esc(s.publishedAt ? date(s.publishedAt) : s.publishedAtText ? "采集时：" + s.publishedAtText : "未知") : esc(date(s.firstObservedAt))}</td></tr>`).join("")}</tbody></table></div>`;
  }
  function representative(s) {
    return `<article class="ci-representative"><button type="button" data-ci="source" data-ref="${esc(s.sourceRef)}" class="lgi-voice-open"><span class="lgi-voice-preview">${esc(s.body || "原声当前不可读")}</span></button><div class="ci-result-line"><span>${tags(s.labels) || `<span class="lgi-research-meta">${s.hasAnalysis ? '已分析' : '待分析'}</span>`}</span><span class="lgi-research-meta">${esc(s.creatorDisplayName || "作者未知")} · ${num(s.likes)}赞 ${bookmark(s)}</span></div></article>`;
  }
  function observations(items) {
    if (data.rules?.advancedReleaseEnabled === false)
      return '<div class="ci-empty-compact"><p>高级观察尚未启用</p><p class="lgi-research-meta">共鸣、冲突与变化判断还需校准。已提取的需求、故事和问题表达仍可查看。</p></div>';
    return items.length
      ? `<ol class="ci-observations">${items
          .slice(0, 3)
          .map(
            (o) =>
              `<li>${btn(esc(o.title), "observation", `data-ref="${esc(o.observationRef)}" class="ci-text-action"`)}<div class="lgi-research-meta">${num(o.comments)} 条原声 · ${num(o.works)} 篇作品${o.distinctExpressions == null ? "" : " · " + num(o.distinctExpressions) + " 种不同表达"} · ${esc(o.reason || "样本内观察")}</div><details><summary>依据与限制</summary><p>${esc(o.comparison?.reason || "没有可比基线，不作增长判断。")}</p><p>${esc(o.representative?.body || "请打开原声核对具体表达。")}</p><p>${esc(o.limitation || "仅描述当前范围内的样本。")}</p><p>规则版本 ${esc(o.ruleVersion || "未知")} · 截止 ${esc(date(o.asOf))}</p></details></li>`,
          )
          .join("")}</ol>`
      : empty(
          data.summary.analyzed
            ? "本次未发现符合规则的新观察。已有结果仍可查阅。"
            : "尚未形成研究观察。原声和词频仍可查看，完成小样本研究后再判断。",
        );
  }
  function problemList(items) {
    return items.length
      ? `<ul class="ci-problem-list">${items
          .slice(0, 5)
          .map(
            (p) =>
              `<li><div>${btn(esc(p.name), "problem", `data-ref="${esc(p.problemRef)}" class="ci-text-action"`)}<div class="lgi-research-meta lgi-voice-preview">${esc(p.representative?.body || p.meaning)}</div></div><span class="lgi-research-meta">${num(p.comments)} 条<br>${num(p.works)} 篇作品</span></li>`,
          )
          .join("")}</ul>`
      : empty("当前范围尚无用户问题。没有分析结果不代表用户没有问题。");
  }
  function chart() {
    const t = data.trend || [];
    if (!t.length) return empty("尚无可用的按日观察计数。");
    const values = t.map((d) => {
      const n = chartLens
        ? !d.researchAvailable && d.analyzed === 0
          ? null
          : d.lenses?.[chartLens]
        : d.comments;
      return n == null
        ? null
        : chartMode === "share"
          ? d.eligible
            ? (100 * n) / d.eligible
            : null
          : n;
    });
    const max = Math.max(1, ...values.filter((v) => v != null));
    return `<div class="ci-chart-controls"><label>声音类型 <select id="ci-chart-lens"><option value="">全部评论</option>${Object.entries(
      lenses,
    )
      .map(
        ([k, n]) =>
          `<option value="${k}" ${k === chartLens ? "selected" : ""}>${n}</option>`,
      )
      .join(
        "",
      )}</select></label><label>显示 <select id="ci-chart-mode"><option value="count">数量</option><option value="share" ${chartMode === "share" ? "selected" : ""} ${chartLens ? "" : "disabled"}>可分析样本内占比</option></select></label></div><div class="ci-chart" role="group" aria-label="用户声音按日计数">${t.map((d, i) => `<button type="button" data-ci="day" data-day="${esc(d.day)}" class="ci-chart-day ${d.partial ? "ci-partial" : ""}" style="--ci-bar:${values[i] == null ? 0 : Math.max(1, (100 * values[i]) / max)}%" aria-label="${esc(d.day)}，${values[i] == null ? "尚未分析" : chartMode === "share" ? values[i].toFixed(1) + "%" : num(values[i]) + "条"}${d.partial ? "，当日未结束" : ""}"><span class="ci-chart-count">${values[i] == null ? "—" : chartMode === "share" ? values[i].toFixed(0) + "%" : num(values[i])}</span><span class="ci-chart-bar"></span><span class="ci-chart-date">${esc(d.day.slice(5))}</span></button>`).join("")}</div><p class="lgi-research-meta">${chartMode === "share" ? "分母为当日当前可分析评论数，需结合分析覆盖解读。" : "本库首次观察计数。"}当日未完成部分以浅色标记，不解释为需求增长。</p>`;
  }

  function renderOverview() {
    const s = data.summary;
    $("results").innerHTML = `<div class="ci-metrics">${[
      ["评论原声", s.comments, "comments"],
      ["来源作品", s.works, "works"],
      ["已分析／可分析", `${num(s.analyzed)} / ${num(s.eligible)}`, "analyzed"],
      ["待分析", s.pending, "pending"],
    ]
      .map(
        ([n, v, k]) =>
          `<button type="button" data-ci="metric" data-metric="${k}"><span>${n}</span><strong>${typeof v === "string" ? v : num(v)}</strong><small>${k === "analyzed" ? (s.eligible ? ((s.analyzed / s.eligible) * 100).toFixed(0) + "% 分析覆盖" : "暂无可分析分母") : k === "comments" ? "按评论身份去重" : k === "works" ? "当前范围来源" : "清洗、分析与上下文"}</small></button>`,
      )
      .join(
        "",
      )}</div><div class="ci-grid"><section><h2>用户声音走势</h2>${chart()}</section><section><h2>值得注意</h2>${observations(data.observations || [])}</section></div><div class="ci-lens-strip">${Object.entries(
      lenses,
    )
      .map(
        ([k, n]) =>
          `<button type="button" data-ci="lens-drill" data-lens="${k}"><span>${n}</span><strong>${num(s.lenses?.[k])}</strong></button>`,
      )
      .join(
        "",
      )}</div>${Object.values(s.lenses || {}).every((v) => v == null) ? '<p class="lgi-research-meta">六类视角尚未分析，未知不计为零。</p>' : ""}<details class="lgi-research-meta"><summary>共鸣与冲突的判定依据</summary><p>共鸣需要同一明确判断有至少5条独立表达、覆盖3篇作品。冲突需要同一命题的支持与反对各有至少2条原声，并存在跨作品证据或真实回复中的反驳；担忧不等于反对。</p></details><div class="ci-grid"><section><div class="ci-section-head"><h2>用户在问什么</h2>${btn("全部问题 →", "all-problems", 'class="ci-text-action"')}</div>${problemList(data.problems || [])}</section><section><div class="ci-section-head"><h2>社区热词</h2>${btn("词频明细 →", "terms", 'class="ci-text-action"')}</div><div class="ci-cloud">${
      (data.terms || [])
        .slice(0, 30)
        .map((t) => {
          const max = Math.max(1, ...data.terms.map((x) => x.count));
          return `<button type="button" data-ci="term" data-term="${esc(t.term)}" style="font-size:${15 + Math.round((17 * t.count) / max)}px" title="${num(t.count)}条原声 · ${num(t.works)}篇作品">${esc(t.term)}</button>`;
        })
        .join("") || empty("当前范围尚无可展示词频。")
    }</div><p class="lgi-research-meta">字号表示包含该词的不同评论数。点击查看原声；没有可比基线时不标升温。</p></section></div><section class="ci-section"><div class="ci-section-head"><h2>代表原声</h2><span class="lgi-research-meta">按最近观察选取，可回溯，不代表总体</span></div><div class="ci-representatives">${(data.representatives || data.page.items.slice(0, 4)).map(representative).join("") || empty("尚无原声。采集接纳后的评论会进入这里。")}</div></section>`;
  }
  function renderProblems() {
    const p = data.problem;
    if (!p) {
      $("results").innerHTML = (data.problems || []).length
        ? `<table><thead><tr><th>用户问题</th><th>评论</th><th>作品</th><th>样本内变化</th><th>最近原声</th></tr></thead><tbody>${data.problems.map((p) => `<tr><td>${btn(esc(p.name), "problem", `data-ref="${esc(p.problemRef)}" class="ci-text-action"`)}</td><td>${num(p.comments)}</td><td>${num(p.works)}</td><td>${esc(p.change?.label || "无可比基线")}</td><td><span class="lgi-voice-preview">${esc(p.representative?.body || "暂无代表原声")}</span></td></tr>`).join("")}</tbody></table>`
        : `<div class="ci-empty-compact"><h2>还没有归并成形的用户问题</h2><p>已分析 ${num(data.summary.analyzed)} 条评论。提取出的具体问题先保留在下方，核对原声后可确认归属。</p>${btn("浏览原声", "voices")}</div>`;
      $("results").innerHTML += candidateList();
      return;
    }
    $("results").innerHTML =
      `${btn("← 返回用户问题", "all-problems")}<section class="ci-problem-header"><div class="ci-section-head"><h2>${esc(p.name)}</h2><div>${btn("收藏问题", "problem-bookmark")}${btn("更名／合并／拆分", "problem-edit")}</div></div><p>${esc(p.meaning)}</p><p class="lgi-research-meta">${num(data.page.total)} 条原声 · 修订 ${num(p.revision)} · ${esc(p.problemRef)}</p>${p.redirectRef ? `<p>本问题已合并。${btn("查看当前问题", "problem", `data-ref="${esc(p.redirectRef)}"`)}</p>` : ""}</section><div class="ci-grid"><section><h2>用户真实怎么说</h2>${(data.representatives || data.page.items.slice(0, 3)).map(representative).join("")}<h3>问题的具体差异</h3>${(data.differences || []).map((x) => `<p>${esc(x.title || x.name || x.label)} · ${num(x.comments)} 条</p>`).join("") || '<p class="lgi-research-meta">当前结果尚未形成可验证的细分差异。</p>'}${(data.stances || []).length ? "<h3>不同观点／经验</h3>" + data.stances.map((x) => `<div class="ci-stance"><strong>${esc({ support: "支持该命题", oppose: "反对该命题", mixed: "有条件的看法", uncertain: "立场不确定" }[x.position] || x.position || x.label)}</strong><p>${esc(x.target || x.meaning)}</p>${(x.sourceRefs || []).map((r) => btn("查看原声", "source", `data-ref="${esc(r)}"`)).join("")}</div>`).join("") : ""}</section><section><h2>样本内走势</h2>${chart()}<h3>来源作品</h3>${
        (data.distribution || [])
          .slice(0, 5)
          .map(
            (w) =>
              `<p>${btn(esc(w.title || "作品标题未知"), "work", `data-ref="${esc(w.workRef)}" class="ci-text-action"`)} · ${num(w.comments)}</p>`,
          )
          .join("") || '<p class="lgi-research-meta">暂无来源分布。</p>'
      }</section></div><details><summary>定义与变更记录</summary><p>${esc(p.definition?.boundary || p.meaning)}</p>${(data.history || []).map((h) => `<p>修订 ${num(h.revision)} · ${esc(h.reason || h.kind || "定义更新")} · ${esc(date(h.createdAt))}</p>`).join("") || "<p>暂无变更记录。</p>"}</details><section class="ci-section"><h2>相关原声</h2>${voiceTable(data.page.items)}</section>`;
  }
  function candidateList() {
    const candidates = data.problemCandidates || [];
    const total = data.candidateTotal;
    return `<section class="ci-section ci-candidates"><div class="ci-section-head"><h2>待整理的问题表达 <span class="lgi-research-meta">${num(total ?? candidates.length)} 个候选</span></h2></div><p class="lgi-research-meta">模型提取的具体问题，尚未确认为稳定问题。先看用户怎样说，再决定是否归入同一个问题。</p>${candidates.length ? `<ul class="ci-candidate-list">${candidates.map((p) => `<li><div><h3>${esc(p.name)}</h3><p>${esc(p.meaning)}</p>${p.evidence?.length ? `<blockquote>${esc(typeof p.evidence[0] === "string" ? p.evidence[0] : p.evidence[0].quote || p.evidence[0].body || "")}</blockquote>` : ""}<span class="lgi-research-meta">${num(p.comments)} 条原声 · ${num(p.works)} 篇作品 · 待确认归属</span></div><div class="ci-candidate-actions">${btn("核对原声", "candidate-voices", `data-ref="${esc(p.candidateRef)}"`)}${canResearch ? btn("确认为问题", "candidate-create", `data-ref="${esc(p.candidateRef)}"`) : ""}</div></li>`).join("")}</ul>${total > candidates.length ? `<p class="lgi-research-meta">当前展示 ${num(candidates.length)} / ${num(total)} 个候选，可通过作品或评论关键词缩小范围。</p>` : ""}` : `<p class="ci-empty-compact">${total == null ? "问题候选尚未提供，请刷新后重试。" : data.summary.analyzed ? "当前范围内尚未提取到未归并的问题表达。可查看已分析原声，核对标签与具体结果。" : "评论尚未分析。先在原声中选择一批评论进行研究，提取到的问题会在这里保留。"}</p>`}</section>`;
  }
  const failureText = (code) => ({
    item_schema_invalid: "返回字段不符合评论格式",
    json_invalid: "模型返回未能解析为完整 JSON",
    schema_invalid: "返回内容不符合约定结构",
    json_or_schema_invalid: "返回内容不符合约定结构",
    provider_failed: "供应商未交付完整结果",
    provider_request_rejected: "供应商拒绝了这次请求，请核对模型与接口配置",
    provider_unavailable: "供应商暂不可用，可稍后重试",
    provider_timeout: "等待供应商响应超时",
    output_limit: "输出达到本次上限",
    evidence_bounds: "证据引用数量不符合约束",
    interpretable_without_evidence: "声明有解释，但没有提供研究结果",
    missing_comment: "模型遗漏了这条评论",
    duplicate_comment: "同一评论被重复返回",
    quote_missing_or_ambiguous: "证据引用无法唯一对应原声",
    quote_redacted_or_empty: "证据引用为空或包含已遮盖内容",
    model_input_limit: "上下文超过输入预算，尚未调用模型",
    context_changed: "调用期间上下文变化，结果未接纳",
    worker_interrupted: "执行中断，供应商用量可能未知",
    source_or_plan_unavailable: "来源不可用或研究已暂停",
  })[code] || (code ? "此项未通过校验，打开详情核对具体原因" : "");
  const duration = (ms) => ms == null ? "耗时未知" : ms < 1000 ? `${num(ms)} 毫秒` : `${(ms / 1000).toFixed(1)} 秒`;
  function callTable(calls, batchRef) {
    return calls.length ? `<div class="ci-table-wrap"><table class="ci-call-table"><thead><tr><th>作品与评论</th><th>执行情况</th><th>用量</th><th>详情</th></tr></thead><tbody>${calls.map((c, i) => `<tr><td><span class="lgi-research-meta">调用 ${i + 1}</span><div class="lgi-voice-preview">${esc(c.workTitle || "作品标题未记录")}</div><span class="lgi-research-meta">${num(c.requestedComments)} 条评论 · ${esc(c.modelId || "模型未记录")}</span></td><td><span class="ci-operation" data-state="${esc(c.state)}">${c.state === "running" ? "等待模型返回" : c.state === "succeeded" ? "输出已接纳" : c.state === "partial" ? "部分输出已接纳" : c.state === "failed" ? "未接纳研究输出" : "状态未知"}</span><div class="lgi-research-meta">${esc(date(c.startedAt))} · ${duration(c.elapsedMs)}</div><div>${c.acceptedComments == null ? "" : `接纳 ${num(c.acceptedComments)} 条输出`}</div>${c.succeededComments != null ? `<div class="lgi-research-meta">有结果 ${num(c.succeededComments)} · 无信号 ${num(c.noSignalComments)} · 未接纳 ${num(c.failedComments)}</div>` : ""}${c.failureCode ? `<p class="ci-failure-text">${esc(failureText(c.failureCode))}</p>` : ""}</td><td><span>输入 ${num(c.inputTokens)} / 输出 ${num(c.outputTokens)}</span><div class="lgi-research-meta">${c.usageUnknown ? `用量待核对 · 保留预留 ${num(c.reservedTokens)}` : "供应商已报告用量"}</div></td><td>${btn("查看过程", "request-detail", `data-ref="${esc(batchRef)}" data-invocation="${esc(c.invocationRef)}"`)}</td></tr>`).join("")}</tbody></table></div>` : `<p class="ci-empty-compact">尚无模型请求记录。批次可能还在清洗、等待执行或已暂停。</p>`;
  }
  function renderDaily() {
    const daily = data.daily || { items: [], schedule: {} }, items = daily.items || [], b = items.find((x) => x.batchRef === state.batchRef);
    $("ci-tools").innerHTML = `<div class="ci-result-line"><label class="ci-batch-picker"><span>研究批次</span> <select id="ci-batch"><option value="">查看最近批次</option>${items.map((x) => `<option value="${esc(x.batchRef)}" ${x.batchRef === state.batchRef ? "selected" : ""}>${esc(date(x.end, true))} · ${x.kind === "daily" ? "每日新增" : "指定范围研究"}</option>`).join("")}</select></label><span>每日自动研究${daily.schedule?.enabled ? "已启用 · 北京时间 23:00 开始" : "未启用"}</span></div>`;
    if (!b) {
      $("results").innerHTML = `<div class="ci-empty-compact"><h2>${items.length ? "这个批次当前不在可见范围" : "从一批原声开始研究"}</h2><p>${items.length ? "请从上方选择已有批次，查看研究结果与执行情况。" : "在原声中勾选评论后开始研究。这里会保留每批的发现、未完成项和模型处理过程。"}</p>${btn("浏览原声", "voices")}</div>`;
      return;
    }
    const counts = b.counts || {}, calls = batchDetail?.calls || [];
    const unfinished = Object.entries(counts).filter(([k]) => !["succeeded", "no_signal", "failed"].includes(k));
    const knownInput = calls.reduce((n, c) => n + (c.inputTokens || 0), 0), knownOutput = calls.reduce((n, c) => n + (c.outputTokens || 0), 0);
    const unknownReserve = calls.filter((c) => c.usageUnknown).reduce((n, c) => n + (c.reservedTokens || 0), 0);
    $("results").innerHTML = `<section class="ci-batch-header"><div class="ci-section-head"><div><h2>这批评论告诉了我们什么</h2><p class="lgi-research-meta">${num(b.total)} 条原声 · ${num(b.works)} 篇作品 · ${esc(date(b.end, true))}</p></div><div class="lgi-research-actions">${btn("查看本批原声", "batch-voices")}${btn("查看运行过程", "show-run")}</div></div><div class="ci-batch-readouts">${[["提取到研究结果", counts.succeeded || 0, "succeeded"], ["未提取到信号", counts.no_signal || 0, "no_signal"], ["分析未完成", counts.failed || 0, "failed"]].map(([label, value, key]) => `<button type="button" data-ci="batch-result" data-state="${key}"><span>${label}</span><strong>${num(value)} <small>条</small></strong><span>${key === "succeeded" ? "查看标签与具体表达" : key === "no_signal" ? "原声保留，可继续核对" : "查看原因与处理方式"}</span></button>`).join("")}</div>${unfinished.length ? `<p class="ci-notice">${unfinished.map(([k, v]) => `${esc(status[k] || "其他待处理状态")} ${num(v)} 条`).join(" · ")}${counts.running ? " · 每 5 秒更新执行情况" : ""}</p>` : ""}${b.kind !== "daily" ? '<p class="lgi-research-meta">这是本批分析到的历史评论，不能据此判断今天新出现了哪些需求。</p>' : ""}</section><section class="ci-section"><div class="ci-section-head"><h2>本批研究结果</h2>${btn("逐条核对", "batch-records", `data-ref="${esc(b.batchRef)}"`)}</div><div class="ci-lens-strip">${["need", "solution", "story", "quote"].map((k) => `<button type="button" data-ci="lens-drill" data-lens="${k}"><span>${lenses[k]}</span><strong>${num(data.summary.lenses?.[k])}</strong></button>`).join("")}</div>${(data.representatives || []).length ? `<div class="ci-representatives">${data.representatives.slice(0, 4).map(representative).join("")}</div>` : '<p class="ci-empty-compact">代表原声尚未选出。可逐条核对已分析评论，查看提取结果。</p>'}${candidateList()}</section><div class="ci-grid"><section><h2>值得继续关注</h2>${observations(data.observations || [])}</section><section><h2>已有问题的新原声</h2>${problemList(data.problems || [])}</section></div><section class="ci-section"><div class="ci-section-head"><div><h2 id="ci-run-heading" tabindex="-1">模型怎样处理这一批</h2><p class="lgi-research-meta">结构化提取 · 思考关闭 · 无工具调用。展示实际请求与校验记录。</p></div><span class="lgi-research-meta">${batchDetail?.callTotal == null ? "" : `共 ${num(batchDetail.callTotal)} 次 · `}最近展示 ${num(batchDetail ? calls.length : null)} 次调用</span></div>${batchDetailError ? `<p class="ci-notice">运行记录暂不可读：${esc(batchDetailError)} ${btn("重试读取", "refresh")}</p>` : callTable(calls, b.batchRef)}<details class="ci-usage"><summary>用量与研究控制</summary><p>当前展示调用已报告：输入 ${num(batchDetail ? knownInput : null)} / 输出 ${num(batchDetail ? knownOutput : null)} Token；用量未知而保留的预留 ${num(batchDetail ? unknownReserve : null)} Token。</p><p>批次额度已计入 ${num(b.chargedTokens)} / ${num(b.tokenLimit)} Token。预留不等于已确认消耗；实际费用以供应商账单为准。</p><p>${b.enabled ? "后续请求允许执行" : "后续请求已暂停"}。暂停后已发出的调用仍可能返回并计费。</p><div class="lgi-research-actions">${btn(b.enabled ? "暂停批次" : "恢复批次", "batch-toggle", `data-ref="${esc(b.batchRef)}" data-enabled="${!b.enabled}"`)}${btn("重试失败", "batch-retry", `data-ref="${esc(b.batchRef)}" ${b.enabled && counts.failed ? "" : "disabled"}`)}${btn("调整本批额度", "batch-continue", `data-ref="${esc(b.batchRef)}"`)}</div></details></section>`;
  }
  function renderPagination() {
    const show = state.view === "voices" || (state.view === "problems" && data.problem);
    $("ci-pagination").hidden = !show;
    if (!show) return;
    const pages = Math.max(1, Math.ceil(data.page.total / state.limit));
    const current = Math.min(pages, Math.floor(state.offset / state.limit) + 1);
    const numbers = [...new Set([1, current - 1, current, current + 1, pages])].filter((p) => p > 0 && p <= pages).sort((a, b) => a - b);
    const pageButtons = numbers.map((p, i) => `${i && p - numbers[i - 1] > 1 ? '<span aria-hidden="true">…</span>' : ""}${btn(String(p), "page", `data-page="${p}" aria-label="第 ${p} 页" ${p === current ? 'aria-current="page"' : ""}`)}`).join("");
    $("ci-pagination").innerHTML = `<span>共 ${num(data.page.total)} 条</span>${btn("选择本页", "select-page", data.page.items.length ? "" : "disabled")}<span class="ci-flex"></span><nav class="ci-pages" aria-label="评论分页">${btn("‹", "previous", `aria-label="上一页" ${state.offset ? "" : "disabled"}`)}${pageButtons}${btn("›", "next", `aria-label="下一页" ${state.offset + state.limit < data.page.total ? "" : "disabled"}`)}</nav><form id="ci-page-jump"><label>跳至 <input name="page" type="number" min="1" max="${pages}" value="${current}" aria-label="跳转页码" required></label><button type="submit">跳转</button></form><label><span class="ci-sr">每页评论数</span><select id="ci-limit"><option value="20">20 条／页</option><option value="50" ${state.limit === 50 ? "selected" : ""}>50 条／页</option></select></label>`;
  }
  function renderSelection() {
    const el = $("ci-selection");
    el.hidden = !selected.size || !["voices", "problems"].includes(state.view);
    el.innerHTML = `<strong>已选 ${num(selected.size)} 条</strong><span class="lgi-research-meta">翻页保留选择</span><span class="ci-flex"></span>${btn("查看已选", "show-selection")}${canResearch ? btn(`研究已选 ${num(selected.size)} 条`, "prepare-selected", 'class="ci-primary"') : ""}${btn("收藏", "bookmark-selected")}${btn("清空选择", "clear-selection")}`;
    const pageBox = document.querySelector("[data-ci-select-page]");
    if (pageBox && data) {
      const count = data.page.items.filter((s) => selected.has(s.sourceRef)).length;
      pageBox.checked = count > 0 && count === data.page.items.length;
      pageBox.indeterminate = count > 0 && count < data.page.items.length;
    }
  }
  async function openSource(ref) {
    const seq = ++detailGeneration;
    if (inspector.hidden) returnFocus = document.activeElement;
    detail = null;
    inspector.hidden = false;
    alignInspector();
    inspector.innerHTML =
      '<header><h2 id="ci-inspector-title" tabindex="-1">评论详情</h2>' +
      btn("关闭", "close-inspector") +
      "</header>" +
      empty("正在读取原声与上下文…");
    $("ci-inspector-title").focus();
    try {
      const v = await request(
        `${api}/sources/${encodeURIComponent(ref)}?${query({ from: data.scope.from, to: data.scope.to })}`,
      );
      if (seq !== detailGeneration || inspector.hidden) return;
      detail = v;
      const s = v.source,
        idx = data.page.items.findIndex((x) => x.sourceRef === s.sourceRef),
        a = v.annotations || {};
      inspector.innerHTML = `<header><h2 id="ci-inspector-title" tabindex="-1">评论详情</h2><div>${btn("↑", "source-previous", `aria-label="上一条评论" ${idx > 0 ? "" : "disabled"}`)}${btn("↓", "source-next", `aria-label="下一条评论" ${idx >= 0 && idx < data.page.items.length - 1 ? "" : "disabled"}`)}${bookmark(s)}${btn("关闭", "close-inspector")}</div></header><div class="ci-inspector-content"><section><blockquote id="ci-source-body">${esc(s.body || "正文当前不可读")}</blockquote><p class="lgi-research-meta">${num(s.likes)} 赞 · ${s.isReply ? "回复评论" : "一级评论"} · ${esc({ author: "作者回复", platform_system: "平台系统评论", user: "用户评论" }[s.role] || "角色未知")} · 点赞快照 ${esc(date(s.lastObservedAt, true))}</p><p class="lgi-research-meta">发表：${esc(s.publishedAt ? date(s.publishedAt, true) : s.publishedAtText ? "采集时：" + s.publishedAtText : "未知")} · 首次观察 ${esc(date(s.firstObservedAt, true))}</p></section><section><h3>对话上下文</h3>${v.parent ? `<div class="ci-parent-context"><p>父评论</p><blockquote>${esc(v.parent.body || "父评论正文未知")}</blockquote></div>` : `<p>${s.isReply ? "父评论尚未采集或当前不可读，不补写缺失内容。" : "这是一条一级评论。"}</p>`}<details><summary>展开已采集的对话片段</summary>${(v.thread || []).map((t) => `<blockquote>${esc(t.body)}</blockquote>`).join("") || "<p>没有更多已存对话。</p>"}</details></section><section><h3>所属作品</h3><strong>${esc(v.work?.title?.value || s.workTitle || "作品标题未知")}</strong><p class="lgi-research-meta">${esc(s.creatorDisplayName || "作者未知")}</p><blockquote class="ci-context-body">${esc(v.work?.body?.value || "作品正文尚未取得。")}</blockquote>${window.CommentDaily.mediaHtml(v.derivatives)}<a href="${linkWork(s)}">查看完整作品 →</a></section><section><h3>研究结果</h3>${v.research?.sourceChanged || s.sourceChanged ? '<p class="lgi-research-meta">原文已变化，原人工判断待复核。</p>' : ""}${tags(v.research?.labels || s.labels) || "<p>尚无研究分类。</p>"}${(v.problems || []).map((p) => `<p>${btn(esc(p.name), "problem", `data-ref="${esc(p.problemRef)}" class="ci-text-action"`)}</p>`).join("")}${(
        a.analysis || []
      )
        .map((x) =>
          x.result
            ? (x.result.spans || [])
                .map(
                  (span) =>
                    `<blockquote>${esc(
                      span.quote ||
                        Array.from(s.body || "")
                          .slice(span.startChar, span.endChar)
                          .join(""),
                    )}</blockquote><p>${(span.facets || []).map((f) => `${esc(f.label)}（${f.basis === "explicit" ? "直接表达" : "研究推断"}）`).join(" · ")}</p>`,
                )
                .join("")
            : "",
        )
        .join(
          "",
        )}${v.research?.locked ? '<p class="lgi-research-meta">人工更正优先保留；自动分析不会静默覆盖。</p>' : ""}${v.research?.reason ? `<p>更正说明：${esc(v.research.reason)}</p>` : ""}<div class="lgi-research-actions">${btn("修改标签／问题归属", "correct")}${btn("收藏与备注", "bookmark-note")}${canResearch ? btn("研究这篇讨论", "prepare-work") : ""}${v.research?.revision ? btn("撤销最近操作", "undo") : ""}</div></section>${(v.bookmarks || []).length ? `<section><h3>历史收藏与备注</h3>${v.bookmarks.map((b) => `<blockquote>${esc(b.quote)}</blockquote><p>${esc(b.reason)}</p><p class="lgi-research-meta">${esc(date(b.createdAt, true))} · 修订 ${num(b.revision)} · 固定来源引用</p>`).join("")}</section>` : ""}<details><summary>处理详情与版本</summary>${window.CommentDaily.cleaningHtml(v.processing)}${(a.analysis || []).map((x) => `<p>${esc(status[x.state] || "状态未知")} · 规则 ${esc(x.ruleVersion || "未知")} · 模型 ${esc(x.modelVersion || "未知")}</p>${x.failureCode ? `<p>${esc(x.failureCode)}</p>` : ""}`).join("")}<p>上下文版本 ${esc(v.contextVersion || v.processing?.contextVersion || "尚未记录")} · 原声指纹 ${esc(v.sourceSha256 || "未知")}</p></details></div>`;
      $("ci-inspector-title").focus();
    } catch (e) {
      if (seq === detailGeneration) {
        inspector.innerHTML =
          '<header><h2 id="ci-inspector-title" tabindex="-1">评论详情</h2>' +
          btn("关闭", "close-inspector") +
          "</header>" +
          empty(e.message);
      }
    }
  }
  async function action(kind, opts = {}) {
    const shownSource =
      opts.sourceRef && detail?.source.sourceRef === opts.sourceRef
        ? { ...detail.source, sourceSha256: detail.sourceSha256 }
        : data?.page.items.find((s) => s.sourceRef === opts.sourceRef);
    const body = {
      commandRef: crypto.randomUUID(),
      domain,
      kind,
      expectedRevision: 0,
      payload: {},
      reason: "",
      ...opts,
      ...(shownSource?.sourceSha256
        ? { expectedSourceSha256: shownSource.sourceSha256 }
        : {}),
    };
    return request(`${api}/actions`, body);
  }
  async function toggleBookmark(ref) {
    const s =
      detail?.source.sourceRef === ref
        ? detail.source
        : data.page.items.find((x) => x.sourceRef === ref) ||
          data.representatives?.find((x) => x.sourceRef === ref);
    if (!s) throw new Error("评论版本已变化，请刷新后重试。");
    await action("bookmark", {
      sourceRef: ref,
      expectedRevision: s.researchRevision ?? 0,
      payload: { enabled: !s.bookmarked, note: "" },
    });
    await load();
    if (detail?.source.sourceRef === ref) await openSource(ref);
  }
  async function openPrepare(refs, scopeOverride = {}) {
    if (!canResearch)
      throw new Error("当前外部领域仅浏览原声，不向模型发送评论。");
    if (!refs && !scopeOverride.workRef && data.page.total > 100) {
      const frozen = {
        ...state,
        ...scopeOverride,
        from: data.scope.from,
        to: data.scope.to,
        offset: 0,
        limit: 50,
      };
      showModal(
        "选择研究范围",
        `<p>当前 ${num(data.page.total)} 条原声。单次手动研究最多 100 条，按当前排序选择前 100 条试跑，其余 ${num(data.page.total - 100)} 条不会自动进入此批次。</p><p>也可以取消后在原声表格中勾选具体评论。每日持续研究由研究设置控制。</p>`,
        "核对前100条试跑范围",
        async () => {
          const pages = await Promise.all([
            request(`${api}?${query(frozen)}`),
            request(`${api}?${query({ ...frozen, offset: 50 })}`),
          ]);
          if (pages[0].scope.resultRevision !== pages[1].scope.resultRevision)
            throw new Error("读取期间结果发生变化，请刷新后重新选择。");
          const refs = [
            ...new Set(
              pages.flatMap((p) => p.page.items.map((s) => s.sourceRef)),
            ),
          ];
          if (refs.length !== 100)
            throw new Error("当前可读范围已变化，请刷新后重新选择。");
          await openPrepare(refs, {
            ...scopeOverride,
            from: frozen.from,
            to: frozen.to,
          });
        },
      );
      return;
    }
    showModal("准备研究范围", empty("正在核对当前可读原声与模型…"), "", null);
    const captured = query(scopeOverride);
    captured.delete("offset");
    captured.delete("limit");
    const scope = {
      ...Object.fromEntries(captured),
      days: Number(state.days),
      limit: state.limit,
      offset: 0,
      bookmarkedOnly: state.bookmarkedOnly,
    };
    const [p, m] = await Promise.all([
      request(`${api}/prepare`, { scope, sourceRefs: refs, reanalyze: false }),
      request("/api/local/model-settings"),
    ]);
    if (!modal.open) return;
    const model = (m.models || []).find(
      (x) => x.modelRef === m.config?.modelRef,
    );
    const modelReady = Boolean(m.config && m.model?.modelConnected);
    const modelHint = ({NEEDS_SELECTION:"已有模型通过校验，尚未选择为默认模型",NEEDS_QUALIFICATION:"请在供应商弹窗中测试评论格式，无需先设置默认模型",PAUSED:"模型连接已暂停，请先启用"})[m.model?.modelState] || "尚未配置，请先添加供应商并测试";
    showModal(
      "确认评论研究",
      `<p>${num(p.count)} 条评论 · ${num(p.works)} 篇作品。范围已冻结，有效至 ${esc(date(p.expiresAt))}。</p><p>按所属作品组织上下文，不增加外部采集。</p><p>${Object.entries(
        p.states || {},
      )
        .filter(([, n]) => typeof n === "number")
        .map(
          ([k, n]) =>
            `${esc({ ...status, comments: "评论总数", works: "来源作品", analyzed: "已有分析", eligible: "可分析", excludedPublished: "发表时间未知" }[k] || "处理计数")} ${num(n)}`,
        )
        .join(
          " · ",
        )}</p><p>研究模型：${esc(modelReady ? model.modelId : modelHint)}。评论及已有作品、父评论文字经必要清洗后提交给该供应商。</p>${modelReady ? `<label>本批 Token 总上限<input name="tokenLimit" type="number" min="1024" max="10000000" value="100000" required></label><label class="ci-check"><input name="reanalyze" type="checkbox"> 重新分析已有结果（额外消耗额度，保留旧版本）</label>` : '<p><a href="/settings/models">配置研究模型</a></p>'}<p class="lgi-research-meta">只在确认后创建批次。取消不创建任务、不预留额度。实际费用取决于模型计费，当前不换算货币。</p>`,
      "确认并开始",
      modelReady && p.count
        ? async (f) => {
            let prep = p;
            if (f.get("reanalyze"))
              prep = await request(`${api}/prepare`, {
                scope,
                sourceRefs: p.sourceRefs,
                reanalyze: true,
              });
            const r = await request(`${api}/run`, {
              prepareRef: prep.prepareRef,
              configRef: m.config.configRef,
              tokenLimit: Number(f.get("tokenLimit")),
              reanalyze: Boolean(f.get("reanalyze")),
            });
            modal.close();
            navigate({ view: "daily", batchRef: r.batchRef });
          }
        : null,
    );
  }
  function openCorrection() {
    const d = detail;
    if (!d) return;
    const r = d.research || {};
    showModal(
      "修改研究标签与问题归属",
      `<p>只更正研究解释，原声保持不变。高共鸣与高冲突由群体证据计算。</p><fieldset><legend>研究标签（可多选）</legend>${["need", "solution", "quote", "story"].map((k) => `<label class="ci-check"><input name="labels" type="checkbox" value="${k}" ${(r.labels || []).includes(k) ? "checked" : ""}>${lenses[k]}</label>`).join("")}</fieldset><fieldset><legend>关联用户问题</legend>${(data.problemOptions || []).map((p) => `<label class="ci-check"><input name="problemRefs" type="checkbox" value="${esc(p.problemRef)}" ${(r.problemRefs || []).includes(p.problemRef) ? "checked" : ""}>${esc(p.name)}</label>`).join("") || "<p>尚无可选问题。可以先创建一个有证据的用户问题。</p>"}</fieldset><label class="ci-check"><input name="needsContext" type="checkbox" ${r.needsContext ? "checked" : ""}>含义需补充上下文</label><label>更正原因<textarea name="reason" required maxlength="1000"></textarea></label>${btn("以这条原声创建问题", "problem-create")}`,
      "保存更正",
      async (f) => {
        await action("correct", {
          sourceRef: d.source.sourceRef,
          expectedRevision: r.revision || 0,
          payload: {
            labels: f.getAll("labels"),
            problemRefs: f.getAll("problemRefs"),
            needsContext: Boolean(f.get("needsContext")),
          },
          reason: f.get("reason"),
        });
        modal.close();
        await load();
        await openSource(d.source.sourceRef);
      },
    );
  }
  function openProblemEdit() {
    const p = data.problem;
    if (!p) return;
    showModal(
      "整理用户问题",
      `<label>操作<select name="operation" id="ci-problem-operation"><option value="problem_rename">更名／修订定义</option><option value="problem_merge">合并到已有问题</option><option value="problem_split">拆分所选原声为新问题</option></select></label><label>问题名称<input name="name" maxlength="200" value="${esc(p.name)}"></label><label>定义与边界<textarea name="meaning" maxlength="2000">${esc(p.meaning)}</textarea></label><label>合并目标<select name="targetRef"><option value="">选择已有问题</option>${(
        data.problemOptions || []
      )
        .filter((x) => x.problemRef !== p.problemRef)
        .map(
          (x) => `<option value="${esc(x.problemRef)}">${esc(x.name)}</option>`,
        )
        .join(
          "",
        )}</select></label><p>拆分范围：当前明确勾选 ${selected.size} 条原声。请先在相关原声中勾选，未选中的归属不自动改变。</p><label>调整原因<textarea name="reason" required maxlength="1000"></textarea></label><p class="lgi-research-meta">更名保持 ID；合并保留旧 ID 重定向；拆分保留变更记录。</p>`,
      "确认调整",
      async (f) => {
        const kind = f.get("operation");
        if (kind === "problem_split" && !selected.size)
          throw new Error("先在相关原声中勾选拆分成员。");
        if (kind === "problem_merge" && !f.get("targetRef"))
          throw new Error("请选择合并目标。");
        const payload =
          kind === "problem_merge"
            ? { targetRef: f.get("targetRef") }
            : {
                name: f.get("name"),
                meaning: f.get("meaning"),
                ...(kind === "problem_split"
                  ? { sourceRefs: [...selected] }
                  : {}),
              };
        await action(kind, {
          problemRef: p.problemRef,
          expectedRevision: p.revision,
          payload,
          reason: f.get("reason"),
        });
        modal.close();
        await load();
      },
    );
  }
  function openFilters() {
    showModal(
      "筛选当前研究范围",
      `<label>时间口径<select name="timeBasis"><option value="observed">首次观察时间</option><option value="published" ${state.timeBasis === "published" ? "selected" : ""}>评论发表时间（仅可靠时间）</option></select></label><label>处理状态<select name="processingState"><option value="">全部状态</option>${["pending", "context", "failed", "low_information", "anomaly"].map((k) => `<option value="${k}" ${state.processingState === k ? "selected" : ""}>${status[k]}</option>`).join("")}</select></label><p>发表时间无法可靠解析的评论会被排除；不会以观察时间补充发表时间。</p>`,
      "应用筛选",
      async (f) => {
        modal.close();
        navigate({
          timeBasis: f.get("timeBasis"),
          processingState: f.get("processingState"),
        });
      },
    );
  }
  function openTerms() {
    showModal(
      "社区词频明细",
      `<p>按不同评论计数；屏蔽只影响当前领域词频，不改原文、不重跑模型。</p><table><thead><tr><th>词</th><th>评论</th><th>作品</th><th>词表操作</th></tr></thead><tbody>${(data.terms || []).map((t) => `<tr><td>${btn(esc(t.term), "term", `data-term="${esc(t.term)}"`)}</td><td>${num(t.count)}</td><td>${num(t.works)}</td><td>${btn("屏蔽", "term-hide", `data-term="${esc(t.term)}" data-revision="${t.revision || 0}"`)}</td></tr>`).join("")}</tbody></table><details><summary>已屏蔽词</summary>${(data.hiddenTerms || []).map((t) => `<p>${esc(t.term)} ${btn("恢复词频", "term-restore", `data-term="${esc(t.term)}" data-revision="${t.revision}"`)}</p>`).join("") || "<p>当前领域没有屏蔽词。</p>"}</details>`,
      "",
      null,
    );
  }
  async function openLegacyAssets(after = "") {
    const q = new URLSearchParams({ limit: "20" });
    if (params.get("collectionRef"))
      q.set("collectionRef", params.get("collectionRef"));
    if (after) q.set("cursor", after);
    const r = await request(`${old}/assets?${q}`);
    showModal(
      "历史收藏",
      `<p>保留原来的收存理由、出处和固定来源版本。历史记录不自动成为当前研究标签。</p>${r.items.map((a) => `<section><blockquote>${esc(a.quote || "来源当前不可读")}</blockquote><p>${esc(a.reason || "备注当前不可读")}</p><p class="lgi-research-meta">${esc(date(a.createdAt, true))} · 修订 ${num(a.revision)} · ${a.isCurrentSource ? "当前来源" : "历史来源版本"}</p>${a.eligibility === "READABLE" ? btn("查看出处", "source", `data-ref="${esc(a.sourceRef)}"`) : "<p>来源当前受限，停止展示原文。</p>"}</section>`).join("") || empty("暂无历史收藏。")}${r.nextCursor ? btn("下一页", "legacy-assets", `data-after="${esc(r.nextCursor)}"`) : ""}`,
      "",
      null,
    );
  }
  const contextLabels = { workBody: "作品正文", ocr: "图片文字", asr: "音视频转录", parent: "父评论", existingProblems: "已有问题召回" };
  async function openContextSettings() {
    const [settings, models] = await Promise.all([
      request(`${old}/daily/context-settings`),
      request("/api/local/model-settings"),
    ]);
    const policy = settings.policy;
    if (!policy) throw new Error("上下文设置尚未提供，请刷新后重试。");
    showModal("研究设置", `<p>决定下次研究可以使用哪些已有材料。每批会保存一份设置，修改不会改变正在运行的批次。</p><fieldset><legend>研究上下文</legend><div class="ci-setting-checks">${Object.entries(contextLabels).map(([key, label]) => `<label class="ci-check"><input name="${key}" type="checkbox" ${policy[key] ? "checked" : ""}>${label}</label>`).join("")}</div><p class="lgi-research-meta">开启表示允许纳入已取得的文字；缺失内容不会被补写，也不会自动发起采集。</p><div class="ci-setting-numbers"><label>最多召回已有问题<input name="recallLimit" type="number" min="1" max="10" value="${policy.recallLimit}" required></label><label>每次最多评论数<input name="maxComments" type="number" min="1" max="30" value="${policy.maxComments}" required></label></div><p class="lgi-research-meta">实际分包还会按模型输入和输出预算缩小。</p></fieldset><fieldset><legend>排查记录</legend><label class="ci-check"><input name="recordContent" type="checkbox" ${policy.recordContent ? "checked" : ""}>保留输入与返回 24 小时，便于排查</label><p class="lgi-research-meta">关闭后仍保留状态、用量和校验摘要。历史未记录的内容无法补回；原声本身仍保存在语料库。</p></fieldset><details><summary>模型与预算</summary><p>快速结构化提取 · 思考关闭 · 无工具调用。</p><p>当前输入预算 ${num(models.config?.inputTokenLimit)}；输出预算 ${num(models.config?.outputTokenLimit)} Token。</p><a href="/settings/models">前往模型设置修改预算</a></details><details><summary>每日自动研究</summary><p>自动运行有单独的启用开关与批次额度。保存这里的上下文设置不会开启自动研究。</p>${btn("设置每日运行与额度", "daily-settings")}</details>`, "保存上下文设置", async (f) => {
      const next = Object.fromEntries(Object.keys(contextLabels).map((key) => [key, f.has(key)]));
      Object.assign(next, { recallLimit: Number(f.get("recallLimit")), maxComments: Number(f.get("maxComments")), recordContent: f.has("recordContent") });
      await request(`${old}/daily/context-settings`, { expectedRevision: settings.revision, policy: next });
      modal.close();
      await load();
      feedback("上下文设置已保存，用于之后创建的批次；每日自动研究状态未改变。");
    });
  }
  function contextText(value) {
    if (typeof value === "string") return value;
    if (value == null) return "尚未取得";
    return value.value || value.text || value.body || JSON.stringify(value, null, 2);
  }
  function workContextHtml(work) {
    if (!work) return "<p>这次没有纳入作品文字。</p>";
    const sections = [
      ["作品标题", work.title], ["作品正文", work.body],
      ["图片文字", work.ocr], ["音视频转录", work.asr],
      ...(Array.isArray(work.mediaTexts) ? work.mediaTexts.map((item) => [({ ocr: "图片文字", asr: "音视频转录", transcript: "音视频转录" })[item.kind] || "媒体文字", item.text]) : []),
    ].filter(([, value]) => value != null);
    return `<div class="ci-input-block">${sections.map(([label, value]) => `<article class="ci-input-comment"><p class="lgi-research-meta">${label}</p><blockquote>${esc(contextText(value))}</blockquote></article>`).join("") || "<p>这次没有取得可展示的作品文字。</p>"}</div>`;
  }
  function diagnosticActual(value) {
    if (typeof value === "string") return value;
    if (!value || typeof value !== "object") return "未记录";
    const types = {string:"文本",object:"对象",array:"列表",null:"空值",boolean:"布尔值",number:"数字",missing:"字段缺失",invalid_json:"JSON 格式不正确",matches:"对应结果"};
    const labels = {characters:"字数",count:"数量",fields:"字段数",unexpectedFields:"多余字段数",bytes:"字节数",line:"行",column:"列",labels:"标签数",problems:"问题数",stances:"立场数"};
    return [types[value.type] || "返回结构", ...Object.entries(labels).filter(([key])=>value[key]!=null).map(([key,label])=>`${label} ${value[key]}`), value.category ? (value.category === "incomplete" ? "内容不完整" : "语法错误") : ""].filter(Boolean).join(" · ");
  }
  async function openRequest(batchRef, invocationRef) {
    showModal("模型处理过程", '<p>正在读取这一次调用的真实记录…</p>', "", null);
    const currentRequest = requestGeneration;
    const v = await request(`${old}/daily/${encodeURIComponent(batchRef)}/requests/${encodeURIComponent(invocationRef)}?domain=${encodeURIComponent(domain)}`);
    if (!modal.open || currentRequest !== requestGeneration) return;
    const call = v.call || batchDetail?.calls?.find((c) => c.invocationRef === invocationRef) || {}, input = v.input, output = v.output;
    const recordedPolicy = v.policy || input?.policy || call.contextPolicy;
    const unavailable = { NOT_RECORDED: "这次调用没有保留输入与返回，无法事后还原。状态和用量记录仍可核对。", EXPIRED: "输入与返回已超过 24 小时保留期限。原声仍在语料库，运行元数据继续保留。", RESTRICTED: "相关来源当前不可读，输入与返回已停止展示。请先核对来源权限。" }[v.availability];
    const events = { packet_prepared: "上下文已组装", request_started: "开始调用模型", provider_started: "供应商请求已发出", provider_returned: "已收到供应商返回", response_received: "已收到模型返回", validation_completed: "输出校验完成", validation_finished: "输出校验完成", results_saved: "合格结果已保存", request_failed: "请求未完成", queued: "已进入等待队列", started: "开始调用", finished: "调用结束" };
    const eventHtml = (v.events || []).map((event) => `<li><span>${esc(events[event.kind] || "已记录运行事件")}</span><time>${esc(date(event.at, true))}</time>${events[event.kind] ? "" : `<details><summary>事件代码</summary><code>${esc(event.kind)}</code></details>`}</li>`).join("");
    const validation = (v.validation || []).map((item) => `<li><strong>${esc(item.commentRef || "请求整体")} · ${esc(failureText(item.code) || "校验记录")}</strong>${item.path ? `<p>位置：<code>${esc(item.path)}</code></p>` : ""}${item.expected != null ? `<p>要求：${esc(contextText(item.expected))}</p>` : ""}${item.actual != null ? `<p>实际：${esc(diagnosticActual(item.actual))}</p>` : ""}<details><summary>校验代码</summary><code>${esc(item.code)}</code></details></li>`).join("");
    const work = input?.work;
    const context = input ? `<section><h3>模型实际拿到的材料</h3><p>评论合同 <code>${esc(input.contract || "未记录")}</code> · 本批冻结的上下文</p><div class="ci-context-flags">${Object.entries(contextLabels).map(([key, label]) => `<span>${recordedPolicy?.[key] === false ? "未纳入" : recordedPolicy?.[key] === true ? "允许纳入" : "设置未记录"} · ${label}</span>`).join("")}</div><p class="lgi-research-meta">允许纳入不代表材料已经取得；以下内容是本次实际输入。</p><details><summary>作品内容</summary>${workContextHtml(work)}</details><details open><summary>评论与父评论 · ${num(input.comments?.length)} 条</summary>${(input.comments || []).map((comment) => `<article class="ci-input-comment"><strong>${esc(comment.commentRef || "评论引用未记录")}</strong><blockquote>${esc(comment.text || comment.body || "正文未记录")}</blockquote>${comment.parent ? `<p class="lgi-research-meta">父评论</p><blockquote>${esc(contextText(comment.parent))}</blockquote>` : '<p class="lgi-research-meta">本次没有纳入父评论文字。</p>'}</article>`).join("")}</details><details><summary>召回的已有问题 · ${num(input.existingProblems?.length)} 个</summary>${(input.existingProblems || []).map((problem) => `<article><h4>${esc(problem.definition?.name || problem.name || problem.candidateRef || "问题")}</h4><p>${esc(problem.definition?.meaning || problem.meaning || problem.boundary || "定义未记录")}</p></article>`).join("") || "<p>本次没有召回已有问题。</p>"}</details><details><summary>研究约束与完整输入</summary><pre>${esc(input.system || "系统约束未记录")}</pre><details><summary>查看输入结构</summary><pre>${esc(JSON.stringify(input, null, 2))}</pre></details></details></section>` : "";
    showModal("模型处理过程", `<div class="ci-request-summary"><strong>${esc(call.workTitle || "作品标题未记录")}</strong><p>${num(call.requestedComments)} 条评论 · ${esc(call.modelId || "模型未记录")} · ${duration(call.elapsedMs)}</p><p class="lgi-research-meta">结构化提取 · 思考关闭 · 无工具调用；以下为真实请求记录。</p>${call.failureCode ? `<p class="ci-failure-text">${esc(failureText(call.failureCode))}</p>` : ""}</div>${eventHtml ? `<ol class="ci-event-list">${eventHtml}</ol>` : '<p class="lgi-research-meta">这次未记录分阶段事件，不能据此推断模型内部过程。</p>'}${unavailable ? `<p class="ci-notice">${esc(unavailable)}</p>` : ""}<div class="ci-request-details">${context}${output ? `<section><h3>模型实际返回 · 已脱敏</h3>${output.truncated ? '<p class="ci-notice">展示已截短，校验使用原始完整返回。展示长度限制不代表模型输出被截断。</p>' : ""}${output.received === false ? "<p>没有取得完整返回。</p>" : ""}<details><summary>查看脱敏返回</summary><pre>${esc(output.text || (output.json ? JSON.stringify(output.json, null, 2) : "返回正文未记录"))}</pre></details></section>` : ""}<section><h3>结果校验</h3>${validation ? `<ul class="ci-validation-list">${validation}</ul>` : `<p>${v.availability === "AVAILABLE" && call.state === "running" ? "正在等待模型返回，校验尚未完成。" : "这次没有可展示的逐字段校验记录。请结合接纳数量与评论结果核对，不能据此判断全部通过。"}</p>`}<details><summary>输入与输出约束</summary><p>输入为清洗后的评论与本批允许纳入的已有上下文。输出须为结构化评论结果，逐条对应输入；研究标签、问题和立场须有可定位的原声引用。缺失内容不补写，未提取信号与调用失败分别记录。</p></details></section><section><h3>用量</h3><p>供应商报告：输入 ${num(call.inputTokens)} / 输出 ${num(call.outputTokens)} Token。</p>${call.usageUnknown ? `<p>用量未知，仍保留 ${num(call.reservedTokens)} Token 预留。预留不代表实际消耗。</p>${call.state !== "running" ? btn("核对用量／允许重试", "usage-review", `data-ref="${esc(batchRef)}" data-invocation="${esc(invocationRef)}"`) : ""}` : ""}</section></div>`, "", null);
    modal.classList.add("ci-request-dialog");
  }
  async function openBatchRecords(ref, after = "") {
    const r = await request(
      `${old}/daily/${encodeURIComponent(ref)}${after ? "?after=" + encodeURIComponent(after) : ""}`,
    );
    showModal(
      "逐条运行记录",
      `<p>评论记录每页最多 50 条。运行用量与人工核对分别记录。</p><table><thead><tr><th>评论</th><th>处理状态</th></tr></thead><tbody>${r.items.map((x) => `<tr><td>${btn(esc(x.body || "来源受限"), "source", `data-ref="${esc(x.sourceRef)}" class="ci-text-action"`)}</td><td>${esc(status[x.state] || x.state)}<p class="lgi-research-meta">${esc(failureText(x.failureCode))}</p></td></tr>`).join("")}</tbody></table>${r.nextCursor ? btn("下一页评论记录", "batch-records", `data-ref="${esc(ref)}" data-after="${esc(r.nextCursor)}"`) : ""}<details><summary>模型调用与费用核对</summary>${(r.calls || []).map((c) => `<section><p>${esc(status[c.state] || c.state)} · ${esc(date(c.startedAt, true))}</p><p>输入 ${num(c.inputTokens)} · 输出 ${num(c.outputTokens)} · 预留 ${num(c.reservedTokens)} · 已计 ${num(c.chargedTokens)} Token</p>${c.usageUnknown ? "<p>供应商用量未知，预留仍保留；未核对前不自动重试。</p>" : ""}${c.usageUnknown && c.state !== "running" ? btn("核对用量／允许重试", "usage-review", `data-ref="${esc(ref)}" data-invocation="${esc(c.invocationRef)}"`) : ""}${c.review ? "<p>已有人工核对记录。</p>" : ""}</section>`).join("") || "<p>暂无模型调用。</p>"}</details><details><summary>人工调整记录</summary>${(r.adjustments || []).map((a) => `<p>${esc({ continue: "继续处理积压", usage_review: "人工核对用量" }[a.kind] || "调整")} · ${esc(date(a.createdAt, true))} · ${esc(a.request?.reason || "未记录说明")}</p>`).join("") || "<p>暂无调整。</p>"}</details>`,
      "",
      null,
    );
  }
  function openContinue(ref) {
    const b = data.daily.items.find((x) => x.batchRef === ref);
    showModal(
      "继续处理本批积压",
      `<p>仍使用本批冻结成员与模型版本，不把历史积压重新算成今天新增。当前 ${num(b.sourceLimit)} 条上限，${num(b.tokenLimit)} Token 总上限。</p><label>本批累计评论上限<input name="sourceLimit" type="number" min="${b.sourceLimit}" max="100000" value="${b.sourceLimit}" required></label><label>本批累计 Token 上限<input name="tokenLimit" type="number" min="${b.tokenLimit}" max="100000000" value="${b.tokenLimit}" required></label><label>继续处理说明<textarea name="reason" required maxlength="1000"></textarea></label>`,
      "确认继续处理",
      async (f) => {
        await request(`${old}/daily/${ref}/continue`, {
          commandRef: crypto.randomUUID(),
          sourceLimit: Number(f.get("sourceLimit")),
          tokenLimit: Number(f.get("tokenLimit")),
          reason: f.get("reason"),
        });
        modal.close();
        await load();
      },
    );
  }
  function openUsageReview(ref, invocation) {
    showModal(
      "人工核对模型用量",
      `<p>请依据供应商记录填写用量。无法确定时留空；保留预留，只有明确接受可能重复计费才允许后续重试。</p><label>核对输入 Token<input name="inputTokens" type="number" min="0"></label><label>核对输出 Token<input name="outputTokens" type="number" min="0"></label><label class="ci-check"><input name="acceptDuplicateCharge" type="checkbox"> 用量仍未知，我接受重试可能产生重复计费</label><label>核对依据与说明<textarea name="reason" required maxlength="1000"></textarea></label><p>此操作记录核对结果，不自行发起重试。</p>`,
      "保存核对",
      async (f) => {
        const input = f.get("inputTokens"),
          output = f.get("outputTokens");
        await request(`${old}/daily/${ref}/usage-review`, {
          commandRef: crypto.randomUUID(),
          invocationRef: invocation,
          inputTokens: input === "" ? null : Number(input),
          outputTokens: output === "" ? null : Number(output),
          acceptDuplicateCharge: Boolean(f.get("acceptDuplicateCharge")),
          reason: f.get("reason"),
        });
        modal.close();
        await load();
      },
    );
  }
  async function handle(a, el) {
    if (resultActions.has(a) && !resultsCurrent) {
      feedback("当前范围尚未读取完成，请等待结果更新后再选择或研究。", true);
      return;
    }
    if (a === "refresh") return load();
    if (a === "close-inspector") return closeInspector();
    if (a === "close-modal") return modal.close();
    if (a === "clear")
      return navigate({
        text: "",
        workRef: "",
        lenses: "",
        term: "",
        problemRef: "",
        sourceRefs: "",
        processingState: "",
        bookmarkedOnly: false,
      });
    if (a === "source") {
      modal.close();
      return openSource(el.dataset.ref);
    }
    if (a === "problem") {
      modal.close();
      return navigate({
        view: "problems",
        problemRef: el.dataset.ref,
        term: "",
        sourceRefs: "",
      });
    }
    if (a === "all-problems")
      return navigate({ view: "problems", problemRef: "" });
    if (a === "work")
      return navigate({ view: "voices", workRef: el.dataset.ref });
    if (a === "bookmarked")
      return navigate({ bookmarkedOnly: !state.bookmarkedOnly });
    if (a === "lens") {
      const values = new Set(state.lenses.split(",").filter(Boolean)),
        k = el.dataset.lens;
      if (!k) values.clear();
      else if (values.has(k)) values.delete(k);
      else values.add(k);
      return navigate({ lenses: [...values].join(",") });
    }
    if (a === "lens-drill")
      return navigate({ view: "voices", lenses: el.dataset.lens });
    if (a === "metric") {
      if (el.dataset.metric === "works") {
        navigate({ view: "voices" });
        return;
      }
      return navigate({
        view: "voices",
        processingState:
          el.dataset.metric === "pending"
            ? "pending"
            : el.dataset.metric === "analyzed"
              ? "analyzed"
              : "",
      });
    }
    if (a === "term") {
      modal.close();
      return navigate({ view: "voices", term: el.dataset.term });
    }
    if (a === "day") {
      const from = new Date(el.dataset.day + "T00:00:00+08:00");
      return navigate({
        view: "voices",
        lenses: chartLens,
        from: from.toISOString(),
        to: new Date(from.valueOf() + 86400000).toISOString(),
      });
    }
    if (a === "observation") {
      const o = data.observations.find(
        (x) => x.observationRef === el.dataset.ref,
      );
      if (o)
        return navigate(
          o.problemRef
            ? { view: "problems", problemRef: o.problemRef }
            : { view: "voices", sourceRefs: (o.sourceRefs || []).join(",") },
        );
    }
    if (a === "voices") return navigate(state.view === "daily" && voiceScope ? { ...voiceScope, view: "voices" } : { view: "voices", problemRef: "" });
    if (a === "page") return navigate({ offset: (Number(el.dataset.page) - 1) * state.limit });
    if (a === "select-page") {
      data.page.items.forEach((s) => selectSource(s, true));
      render();
      return;
    }
    if (a === "show-selection") {
      showModal("已选择的评论", `<p>共 ${num(selected.size)} 条，包含其他页面的选择。确认研究时会再次核对来源是否可读。</p><ul class="ci-problem-list">${[...selected].map((ref) => `<li><div>${btn(esc(selectedSources.get(ref)?.body || "评论正文未缓存"), "source", `data-ref="${esc(ref)}" class="ci-text-action"`)}<p class="lgi-research-meta">${esc(selectedSources.get(ref)?.workTitle || "作品标题未知")}</p></div></li>`).join("")}</ul>`, "", null);
      return;
    }
    if (a === "previous" || a === "next")
      return navigate({
        offset: Math.max(
          0,
          state.offset + (a === "next" ? state.limit : -state.limit),
        ),
      });
    if (a === "clear-selection") {
      selected.clear();
      selectedSources.clear();
      render();
      return;
    }
    if (a === "bookmark") return toggleBookmark(el.dataset.ref);
    if (a === "bookmark-selected") {
      for (const ref of selected) {
        const current = await request(`${api}/sources/${encodeURIComponent(ref)}?${query()}`);
        const s = current.source;
        if (s && !s.bookmarked)
          await action("bookmark", {
            sourceRef: ref,
            expectedRevision: s.researchRevision ?? 0,
            expectedSourceSha256: current.sourceSha256 || s.sourceSha256,
            payload: { enabled: true, note: "" },
          });
      }
      selected.clear();
      selectedSources.clear();
      return load();
    }
    if (a === "prepare") return openPrepare();
    if (a === "prepare-selected") return openPrepare([...selected]);
    if (a === "prepare-work")
      return openPrepare(undefined, {
        workRef: detail.source.workRef,
        text: "",
        lenses: "",
        problemRef: "",
        term: "",
        sourceRefs: "",
      });
    if (a === "settings") {
      if (!canResearch)
        throw new Error("当前外部领域提供原声浏览，尚未开放模型研究。");
      modal.close();
      return openContextSettings();
    }
    if (a === "correct") return openCorrection();
    if (a === "terms") return openTerms();
    if (a === "filters") return openFilters();
    if (a === "term-hide" || a === "term-restore") {
      await action("term_hide", {
        expectedRevision: Number(el.dataset.revision),
        payload: { term: el.dataset.term, hidden: a === "term-hide" },
      });
      modal.close();
      return load();
    }
    if (a === "source-previous" || a === "source-next") {
      const i = data.page.items.findIndex(
        (x) => x.sourceRef === detail?.source.sourceRef,
      );
      const s = data.page.items[i + (a === "source-next" ? 1 : -1)];
      if (s) return openSource(s.sourceRef);
    }
    if (a === "bookmark-note") {
      const d = detail;
      showModal(
        "收藏与备注",
        `<label>研究备注<textarea name="note" maxlength="1000">${esc(d.research?.bookmarkNote || "")}</textarea></label><p>记录这条表达值得再次研究的原因，关联原声，不复制新的评论身份。</p>`,
        "保存收藏与备注",
        async (f) => {
          await action("bookmark", {
            sourceRef: d.source.sourceRef,
            expectedRevision: d.source.researchRevision,
            payload: { enabled: true, note: f.get("note") },
          });
          modal.close();
          await load();
          await openSource(d.source.sourceRef);
        },
      );
      return;
    }
    if (a === "undo") {
      const d = detail;
      showModal(
        "撤销最近操作",
        '<p>恢复前一个人工研究记录版本，包括其中的标签、问题归属或收藏，并保留本次撤销记录。</p><label>撤销原因<textarea name="reason" required maxlength="1000"></textarea></label>',
        "确认撤销",
        async (f) => {
          await action("undo", {
            sourceRef: d.source.sourceRef,
            expectedRevision: d.research.revision,
            payload: { revision: d.research.revision },
            reason: f.get("reason"),
          });
          modal.close();
          await load();
          await openSource(d.source.sourceRef);
        },
      );
      return;
    }
    if (a === "problem-create") {
      const ref = detail?.source.sourceRef;
      showModal(
        "创建用户问题",
        '<label>具体问题<input name="name" required maxlength="200" placeholder="用户具体在问什么？"></label><label>含义与边界<textarea name="meaning" required maxlength="2000"></textarea></label><label>依据说明<textarea name="reason" required maxlength="1000"></textarea></label>',
        "创建问题",
        async (f) => {
          await action("problem_create", {
            payload: {
              name: f.get("name"),
              meaning: f.get("meaning"),
              sourceRefs: ref ? [ref] : [...selected],
            },
            reason: f.get("reason"),
          });
          modal.close();
          await load();
          if (ref) await openSource(ref);
        },
      );
      return;
    }
    if (a === "problem-edit") return openProblemEdit();
    if (a === "problem-bookmark") {
      await action("problem_bookmark", {
        problemRef: data.problem.problemRef,
        expectedRevision: data.problem.revision,
        payload: { enabled: !data.problem.bookmarked },
      });
      return load();
    }
    if (a === "candidate-voices") {
      const candidate = data.problemCandidates?.find((p) => p.candidateRef === el.dataset.ref);
      if (candidate) return navigate({ view: "voices", problemRef: "", sourceRefs: candidate.sourceRefs.join(",") });
    }
    if (a === "candidate-create") {
      const candidate = data.problemCandidates?.find((p) => p.candidateRef === el.dataset.ref);
      if (!candidate) throw new Error("候选已更新，请刷新后重试。");
      showModal("确认用户问题", `<p>将 ${num(candidate.comments)} 条原声确认为一个问题。请先核对这些表达的含义与边界。</p><label>具体问题<input name="name" required maxlength="200" value="${esc(candidate.name)}"></label><label>含义与边界<textarea name="meaning" required maxlength="2000">${esc(candidate.meaning)}</textarea></label><label>确认依据<textarea name="reason" required maxlength="1000"></textarea></label>`, "确认并创建问题", async (f) => {
        await action("problem_create", { payload: { name: f.get("name"), meaning: f.get("meaning"), sourceRefs: candidate.sourceRefs }, reason: f.get("reason") });
        modal.close();
        await load();
      });
      return;
    }
    if (a === "show-run") {
      $("ci-run-heading")?.scrollIntoView({ block: "start" });
      $("ci-run-heading")?.focus({ preventScroll: true });
      return;
    }
    if (a === "request-detail") return openRequest(el.dataset.ref, el.dataset.invocation);
    if (a === "daily-settings") {
      modal.close();
      return window.CommentDaily.openSettings();
    }
    if (a === "batch-result") return navigate({ view: "voices", processingState: el.dataset.state });
    if (a === "batch-voices") return navigate({ view: "voices" });
    if (a === "batch-toggle") {
      await request(`${old}/daily/${el.dataset.ref}`, {
        enabled: el.dataset.enabled === "true",
      });
      return load();
    }
    if (a === "batch-retry") {
      await request(`${old}/daily/${el.dataset.ref}/retry`, {
        commandRef: crypto.randomUUID(),
      });
      return load();
    }
    if (a === "batch-records")
      return openBatchRecords(el.dataset.ref, el.dataset.after || "");
    if (a === "batch-continue") return openContinue(el.dataset.ref);
    if (a === "usage-review")
      return openUsageReview(el.dataset.ref, el.dataset.invocation);
    if (a === "legacy-assets") return openLegacyAssets(el.dataset.after || "");
    if (a === "save-query") {
      showModal(
        "保存查询",
        `<label>查询名称<input name="name" required maxlength="100"></label><p>兼容现有已存查询：保存搜索文字和作品范围。时间与研究标签为当前浏览条件。</p>`,
        "保存查询",
        async (f) => {
          await request(`${old}/queries`, {
            queryRef: crypto.randomUUID(),
            name: f.get("name"),
            text: state.text,
            workRef: state.workRef || null,
          });
          modal.close();
          feedback("查询已保存，可从已存查询入口打开。");
        },
      );
    }
  }
  document.addEventListener("click", (e) => {
    const tab = e.target.closest(".lgi-research-tabs [data-view]");
    if (tab) {
      e.preventDefault();
      navigate(state.view === "daily" && tab.dataset.view === "voices" && voiceScope
        ? { ...voiceScope, view: "voices" }
        : { view: tab.dataset.view, problemRef: tab.dataset.view === "problems" ? state.problemRef : "", batchRef: "" });
      return;
    }
    const el = e.target.closest("[data-ci]");
    if (!el) return;
    e.preventDefault();
    if (busyAction) return;
    const mutations = [
      "bookmark",
      "bookmark-selected",
      "term-hide",
      "term-restore",
      "problem-bookmark",
      "batch-toggle",
      "batch-retry",
    ];
    const mutation = mutations.includes(el.dataset.ci);
    if (mutation) {
      busyAction = true;
      el.disabled = true;
    }
    Promise.resolve(handle(el.dataset.ci, el))
      .catch((err) => {
        if (modal.open) $("ci-command-feedback").textContent = err.message;
        else feedback(err.message, true);
      })
      .finally(() => {
        if (mutation) {
          busyAction = false;
          el.disabled = false;
        }
      });
  });
  document.addEventListener("submit", (e) => {
    if (e.target.id === "ci-page-jump") {
      e.preventDefault();
      const page = Number(new FormData(e.target).get("page"));
      const pages = Math.max(1, Math.ceil(data.page.total / state.limit));
      if (Number.isInteger(page) && page >= 1 && page <= pages) navigate({ offset: (page - 1) * state.limit });
      return;
    }
    if (e.target.id === "ci-search") {
      e.preventDefault();
      const f = new FormData(e.target);
      navigate({ text: f.get("text").trim(), workRef: f.get("workRef") });
    }
  });
  $("ci-command-form").addEventListener("submit", async (e) => {
    e.preventDefault();
    if (!command) return;
    const b = $("ci-command-submit");
    if (b.disabled) return;
    b.disabled = true;
    try {
      await command(new FormData(e.target));
    } catch (err) {
      $("ci-command-feedback").textContent = err.message;
    } finally {
      b.disabled = false;
    }
  });
  document.addEventListener("change", (e) => {
    const t = e.target;
    if ((t.dataset.ciSelect || t.hasAttribute("data-ci-select-page")) && !resultsCurrent) {
      if (t.dataset.ciSelect) t.checked = selected.has(t.dataset.ciSelect);
      else t.checked = data?.page.items.every((s) => selected.has(s.sourceRef)) || false;
      feedback("当前范围尚未读取完成，请等待结果更新后再选择。", true);
      return;
    }
    if (t.dataset.ciSelect) {
      const source = data.page.items.find((s) => s.sourceRef === t.dataset.ciSelect);
      if (source) selectSource(source, t.checked);
      renderSelection();
    } else if (t.hasAttribute("data-ci-select-page")) {
      data.page.items.forEach((s) =>
        selectSource(s, t.checked),
      );
      render();
    } else if (t.id === "ci-chart-lens") {
      chartLens = t.value;
      if (!chartLens) chartMode = "count";
      render();
    } else if (t.id === "ci-chart-mode") {
      chartMode = t.value;
      render();
    } else if (t.id === "ci-sort") navigate({ sort: t.value });
    else if (t.id === "ci-limit") navigate({ limit: Number(t.value) });
    else if (t.id === "ci-batch") navigate({ batchRef: t.value });
    else if (t.id === "ci-days") {
      if (t.value === "custom")
        showModal(
          "自定义时间范围",
          '<label>开始日期（北京时间）<input name="from" type="date" required></label><label>截止日期（不包含当天）<input name="to" type="date" required></label>',
          "应用范围",
          async (f) => {
            const from = new Date(f.get("from") + "T00:00:00+08:00"),
              to = new Date(f.get("to") + "T00:00:00+08:00");
            if (from >= to) throw new Error("截止日期必须晚于开始日期。");
            modal.close();
            navigate({ from: from.toISOString(), to: to.toISOString() });
          },
        );
      else navigate({ days: t.value, from: "", to: "" });
    }
  });
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && !modal.open && !inspector.hidden)
      closeInspector();
  });
  window.addEventListener("popstate", (e) => {
    if (e.state) {
      clearSelectionForScope(e.state);
      Object.assign(state, e.state);
      if (state.view === "daily") Object.assign(state, voiceFilterReset);
      closeInspector();
      load().then(() => {
        main.scrollTop = e.state.scroll || 0;
      });
    }
  });
  window.CommentDaily.onRefresh = () => load();
  load().then(() => {
    if (params.get("source")) openSource(params.get("source"));
  });
})();
