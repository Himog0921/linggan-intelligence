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
    busyAction = false;
  const selected = new Set();
  const errors = {
    revision_conflict: "记录已更新，请保留输入并重新读取最新版本。",
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
    $("ci-command-title").textContent = title;
    $("ci-command-fields").innerHTML = html;
    $("ci-command-feedback").textContent = "";
    $("ci-command-submit").textContent = submit;
    $("ci-command-submit").hidden = !callback;
    if (!modal.open) modal.showModal();
  }
  async function load() {
    const seq = ++generation;
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
      data = v;
      state.resultRevision = v.scope.resultRevision || "";
      render();
      feedback(v.scope.updated ? "结果已更新：本页显示最新聚合版本。" : "");
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
    closeInspector();
    savedScroll = main.scrollTop;
    history.replaceState(
      { ...state, scroll: savedScroll },
      "",
      `${location.pathname}?${query()}`,
    );
    if (
      data?.scope &&
      patch.from === undefined &&
      state.view !== "daily" &&
      patch.view !== "daily"
    ) {
      state.from = data.scope.from;
      state.to = data.scope.to;
    }
    Object.assign(state, patch, { offset: patch.offset ?? 0 });
    selected.clear();
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
      `<div>${esc(s.domainName || document.body.dataset.corpusDomainName || "当前领域")} · ${state.view === "daily" ? "批次范围" : state.timeBasis === "published" ? "按评论发表时间" : "按首次观察时间"}${state.workRef ? " · 已限定作品" : ""}${state.problemRef ? " · 已限定问题" : ""}${state.term ? " · 热词：" + esc(state.term) : ""}${state.sourceRefs ? " · 精确证据集" : ""}${state.text ? " · 检索：" + esc(state.text) : ""} ${state.workRef || state.problemRef || state.term || state.text || state.lenses || state.processingState || state.sourceRefs ? btn("清除筛选", "clear") : ""}<div class="lgi-research-meta">${esc(date(s.from, true))} 至 ${esc(date(s.to, true))}（不含截止时刻）</div></div><div>${data.model?.modelConnected ? "研究模型已连接" : `<a href="/settings/models">${data.model?.modelState === "PAUSED" ? "模型连接已暂停" : data.model?.modelState === "NEEDS_QUALIFICATION" ? "模型需要重新测试评论合同" : "模型尚未配置"}</a>`} · 截止 ${esc(date(s.asOf))}<details><summary>范围与版本</summary><p>聚合版本 ${esc(s.resultRevision || "未知")} · ${esc(s.timeBasis || state.timeBasis)}</p><p>按发表时间筛选时排除 ${num(data.summary.excludedPublished)} 条无法可靠解析时间的评论。</p></details></div>`;
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
          )}<span class="ci-flex"></span>${btn("☆ 仅收藏", "bookmarked", `aria-pressed="${state.bookmarkedOnly}"`)}</div><div class="ci-result-line"><span>${num(data.page.total)} 条原声 · 已分析 ${num(data.summary.analyzed)} / ${num(data.summary.eligible)}${state.processingState ? " · " + esc(status[state.processingState] || "待处理") : ""}</span><label>排序 <select id="ci-sort"><option value="observed">最近观察</option><option value="likes" ${state.sort === "likes" ? "selected" : ""}>点赞最多</option></select></label>${btn("保存查询", "save-query")}${state.bookmarkedOnly ? btn("历史收藏", "legacy-assets") : ""}</div>`
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
    return `<article class="ci-representative"><button type="button" data-ci="source" data-ref="${esc(s.sourceRef)}" class="lgi-voice-open"><span class="lgi-voice-preview">${esc(s.body || "原声当前不可读")}</span></button><div class="ci-result-line"><span>${tags(s.labels) || '<span class="lgi-research-meta">待分析</span>'}</span><span class="lgi-research-meta">${esc(s.creatorDisplayName || "作者未知")} · ${num(s.likes)}赞 ${bookmark(s)}</span></div></article>`;
  }
  function observations(items) {
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
        : empty("尚未形成稳定的用户问题。可以从原声中研究或纠正问题归属。");
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
  function renderDaily() {
    const daily = data.daily || { items: [], schedule: {} },
      items = daily.items || [],
      b = items.find((x) => x.batchRef === state.batchRef);
    $("ci-tools").innerHTML =
      `<div class="ci-result-line"><label>研究批次 <select id="ci-batch"><option value="">选择已有批次</option>${items.map((x) => `<option value="${esc(x.batchRef)}" ${x.batchRef === state.batchRef ? "selected" : ""}>${esc(date(x.end, true))} · ${x.kind === "daily" ? "每日新增" : "指定范围研究"}</option>`).join("")}</select></label><span class="lgi-research-meta">每日观察${daily.schedule?.enabled ? "已启用" : "尚未启用或已暂停"} · 北京时间 23:00 冻结批次</span></div>`;
    $("results").innerHTML = b
      ? `<section class="ci-problem-header"><h2>本次评论观察</h2><p>${esc(date(b.start, true))} → ${esc(date(b.end, true))} · 北京时间</p><p>${num(b.total)} 条原声 · ${num(b.works)} 篇作品 · ${Object.entries(
          b.counts || {},
        )
          .map(([k, v]) => esc(status[k] || k) + " " + num(v))
          .join(
            " · ",
          )}</p>${b.kind !== "daily" ? '<p class="ci-notice">本次新分析到的历史评论，不表示今天新出现的用户需求。</p>' : ""}${btn("查看本批原声", "batch-voices")}</section><div class="ci-grid"><section><h2>本次值得注意</h2>${observations(data.observations || [])}</section><section><h2>已有问题的本批原声</h2>${problemList(data.problems || [])}</section></div><details class="ci-section"><summary>运行记录与控制</summary><p>当前筛选包含 ${num(b.total)} 条；完整批次 ${num(b.batchTotals?.total ?? b.total)} 条。完整批次已计用量 ${num(b.chargedTokens)} / ${num(b.tokenLimit)} Token。包含用量未知时保留的预留，无法据此推算实际货币费用。</p><p>${b.enabled ? "允许执行" : "已暂停"} · 暂停停止后续派发，已发出的调用仍可能返回并计费。</p><div class="lgi-research-actions">${btn(b.enabled ? "暂停批次" : "恢复批次", "batch-toggle", `data-ref="${esc(b.batchRef)}" data-enabled="${!b.enabled}"`)}${btn("重试失败", "batch-retry", `data-ref="${esc(b.batchRef)}" ${b.enabled && b.counts?.failed ? "" : "disabled"}`)}${btn("继续处理积压／调整额度", "batch-continue", `data-ref="${esc(b.batchRef)}"`)}${btn("逐条运行记录", "batch-records", `data-ref="${esc(b.batchRef)}"`)}</div></details>`
      : empty(
          items.length
            ? "选择一个批次查看本次观察。原声范围与批次范围分别保留。"
            : "尚无研究批次。先对少量原声试跑，确认结果后在研究设置中启用每日观察。",
        );
  }
  function renderPagination() {
    const show =
      state.view === "voices" || (state.view === "problems" && data.problem);
    $("ci-pagination").hidden = !show;
    if (!show) return;
    $("ci-pagination").innerHTML =
      `<label>每页 <select id="ci-limit"><option value="20">20 条</option><option value="50" ${state.limit === 50 ? "selected" : ""}>50 条</option></select></label><span>${num(data.page.total)} 条 · 第 ${Math.floor(state.offset / state.limit) + 1} 页</span><span class="ci-flex"></span>${btn("上一页", "previous", state.offset ? "" : "disabled")}${btn("下一页", "next", state.offset + state.limit < data.page.total ? "" : "disabled")}`;
  }
  function renderSelection() {
    const el = $("ci-selection");
    el.hidden = !selected.size;
    el.innerHTML = `<span>已选 ${selected.size} 条（明确评论 ID）</span>${canResearch ? btn("分析所选", "prepare-selected") : ""}${btn("收藏", "bookmark-selected")}${btn("取消选择", "clear-selection")}`;
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
        )}</p><p>研究模型：${esc(model?.modelId || "尚未配置")}。评论及已有作品、父评论文字经必要清洗后提交给该供应商。</p>${m.config ? `<label>本批 Token 总上限<input name="tokenLimit" type="number" min="1024" max="10000000" value="100000" required></label><label class="ci-check"><input name="reanalyze" type="checkbox"> 重新分析已有结果（额外消耗额度，保留旧版本）</label>` : '<p><a href="/settings/models">配置研究模型</a></p>'}<p class="lgi-research-meta">只在确认后创建批次。取消不创建任务、不预留额度。实际费用取决于模型计费，当前不换算货币。</p>`,
      "确认并开始",
      m.config && p.count
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
  async function openBatchRecords(ref, after = "") {
    const r = await request(
      `${old}/daily/${encodeURIComponent(ref)}${after ? "?after=" + encodeURIComponent(after) : ""}`,
    );
    showModal(
      "逐条运行记录",
      `<p>评论记录每页最多 50 条。运行用量与人工核对分别记录。</p><table><thead><tr><th>评论</th><th>处理状态</th></tr></thead><tbody>${r.items.map((x) => `<tr><td>${btn(esc(x.body || "来源受限"), "source", `data-ref="${esc(x.sourceRef)}" class="ci-text-action"`)}</td><td>${esc(status[x.state] || x.state)}<p class="lgi-research-meta">${esc(x.failureCode || "")}</p></td></tr>`).join("")}</tbody></table>${r.nextCursor ? btn("下一页评论记录", "batch-records", `data-ref="${esc(ref)}" data-after="${esc(r.nextCursor)}"`) : ""}<details><summary>模型调用与费用核对</summary>${(r.calls || []).map((c) => `<section><p>${esc(status[c.state] || c.state)} · ${esc(date(c.startedAt, true))}</p><p>输入 ${num(c.inputTokens)} · 输出 ${num(c.outputTokens)} · 预留 ${num(c.reservedTokens)} · 已计 ${num(c.chargedTokens)} Token</p>${c.usageUnknown ? "<p>供应商用量未知，预留仍保留；未核对前不自动重试。</p>" : ""}${c.usageUnknown && c.state !== "running" ? btn("核对用量／允许重试", "usage-review", `data-ref="${esc(ref)}" data-invocation="${esc(c.invocationRef)}"`) : ""}${c.review ? "<p>已有人工核对记录。</p>" : ""}</section>`).join("") || "<p>暂无模型调用。</p>"}</details><details><summary>人工调整记录</summary>${(r.adjustments || []).map((a) => `<p>${esc({ continue: "继续处理积压", usage_review: "人工核对用量" }[a.kind] || "调整")} · ${esc(date(a.createdAt, true))} · ${esc(a.request?.reason || "未记录说明")}</p>`).join("") || "<p>暂无调整。</p>"}</details>`,
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
    if (a === "previous" || a === "next")
      return navigate({
        offset: Math.max(
          0,
          state.offset + (a === "next" ? state.limit : -state.limit),
        ),
      });
    if (a === "clear-selection") {
      selected.clear();
      render();
      return;
    }
    if (a === "bookmark") return toggleBookmark(el.dataset.ref);
    if (a === "bookmark-selected") {
      for (const ref of selected) {
        const s = data.page.items.find((x) => x.sourceRef === ref);
        if (s && !s.bookmarked)
          await action("bookmark", {
            sourceRef: ref,
            expectedRevision: s.researchRevision ?? 0,
            payload: { enabled: true, note: "" },
          });
      }
      selected.clear();
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
      return window.CommentDaily.openSettings();
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
      navigate({
        view: tab.dataset.view,
        problemRef: tab.dataset.view === "problems" ? state.problemRef : "",
        batchRef: "",
      });
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
    if (t.dataset.ciSelect) {
      if (t.checked) selected.add(t.dataset.ciSelect);
      else selected.delete(t.dataset.ciSelect);
      renderSelection();
    } else if (t.hasAttribute("data-ci-select-page")) {
      data.page.items.forEach((s) =>
        t.checked ? selected.add(s.sourceRef) : selected.delete(s.sourceRef),
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
      Object.assign(state, e.state);
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
