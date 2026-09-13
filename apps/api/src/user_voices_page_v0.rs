//! Static, read-only browser for current Comment Research User Voices V0.
//!
//! The page deliberately starts without a workspace and therefore has no
//! source range to request. Its script only calls the read API after a user
//! supplies a workspace ID and submits the form or changes an already-loaded
//! page. It never creates research work or writes source facts.

use axum::{
    http::header,
    response::{Html, IntoResponse},
};

const USER_VOICES_PAGE_V0_CSS: &str = include_str!("../assets/comment-research-voices-v0.css");

pub async fn user_voices_page_v0() -> Html<&'static str> {
    Html(USER_VOICES_PAGE_V0_HTML)
}

pub async fn user_voices_page_v0_stylesheet() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        USER_VOICES_PAGE_V0_CSS,
    )
}

const USER_VOICES_PAGE_V0_HTML: &str = r##"<!doctype html>
<html lang="zh-CN">
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta name="color-scheme" content="light">
    <title>用户原声 · Linggan Intelligence</title>
    <link rel="stylesheet" href="/comment-research/voices/styles.css">
  </head>
  <body>
    <a class="skip-link" href="#voices-main">跳到用户原声</a>
    <div class="app-shell">
      <aside class="rail" aria-label="当前模块">
        <span class="brand">Linggan Intelligence</span>
        <p class="section-label">评论研究</p>
        <div class="research-nav">
          <span class="nav-item" aria-current="page"><span class="nav-index">02</span>用户原声</span>
        </div>
        <p class="nav-note">当前交付可回溯的原始原声与确定性清洗表达。研究结论并未在此版本中生成。</p>
      </aside>

      <main class="workspace" id="voices-main" tabindex="-1">
        <header class="page-heading">
          <div>
            <p class="eyebrow" id="page-eyebrow">评论研究 · 清洗语料 V1</p>
            <h1 id="page-title">用户原声</h1>
          </div>
          <div class="page-heading-actions">
            <button class="quiet-button" id="plan-preview-button" type="button" disabled aria-describedby="plan-preview-help">查看自动研究范围</button>
            <p class="page-summary" id="plan-preview-help">列表呈现确定性清洗后的可研究表达；打开详情可同时核对原始采集原声、来源证据与已采到的讨论语境。研究结论和趋势尚未生成。</p>
          </div>
        </header>

        <div class="view-tabs" role="tablist" aria-label="评论研究视图">
          <button class="view-tab" type="button" role="tab" aria-selected="false" disabled>概览</button>
          <button class="view-tab" id="voices-tab" type="button" role="tab" aria-selected="true">用户原声</button>
          <button class="view-tab" type="button" role="tab" aria-selected="false" disabled>用户问题</button>
          <button class="view-tab" type="button" role="tab" aria-selected="false" disabled>变化观察</button>
          <button class="view-tab" id="run-records-tab" type="button" role="tab" aria-selected="false">运行记录</button>
        </div>
        <p class="nav-note" id="unavailable-views-note">概览、用户问题和变化观察尚未具备来源事实，本页不会用空白仪表盘代替。</p>

        <form class="scope-panel" id="workspace-form" novalidate>
          <div>
            <label class="field-label" for="workspace-id">工作空间 ID</label>
            <input class="workspace-input" id="workspace-id" name="workspace_id" type="text" autocomplete="off" spellcheck="false" placeholder="输入工作空间 ID 后加载用户原声" aria-describedby="workspace-help status-message">
            <p class="field-help" id="workspace-help">初始状态不读取任何数据。系统只在你确认工作空间后，以只读方式加载当前可用的清洗语料。</p>
          </div>
          <div class="filter-field">
            <label class="field-label" for="voice-filter">语料状态</label>
            <select class="workspace-input" id="voice-filter" name="filter" aria-describedby="filter-help">
              <option value="available">全部可用</option>
              <option value="ready">可直接研究</option>
              <option value="needs_context">需要上下文</option>
            </select>
            <p class="field-help" id="filter-help">纯 emoji、纯 @ 和异常内容不会进入用户原声。需要上下文的短表达仍保留。</p>
          </div>
          <button class="primary-button" id="load-voices" type="submit">加载用户原声</button>
        </form>

        <p class="status-message" id="status-message" role="status" aria-live="polite"></p>

        <section class="empty-state" id="initial-state" aria-labelledby="initial-state-title">
          <h2 id="initial-state-title">先确认要看的语料范围</h2>
          <p>输入工作空间 ID 后，页面会读取其中已接入、完成确定性清洗且可回溯至来源证据的当前表达。这里不会假定作品标题、作者、点赞、发表时间、回复关系或研究结论。</p>
        </section>

        <section class="empty-state" id="empty-state" aria-labelledby="empty-state-title" hidden>
          <h2 id="empty-state-title">当前筛选没有可显示的用户原声</h2>
          <p>这可能是当前没有可用清洗表达、所选状态没有匹配结果，或仍有原声等待本地清洗物化；它不表示用户没有讨论，也不表示系统已经完成研究。</p>
        </section>

        <section class="results" id="results" aria-labelledby="results-title" hidden>
          <div class="results-heading">
            <h2 id="results-title">当前用户原声</h2>
            <p class="result-count" id="result-count"></p>
          </div>
          <div class="table-scroller" tabindex="0" aria-label="用户原声表格，可横向滚动">
            <table class="voice-table">
              <thead>
                <tr>
                  <th scope="col">可研究表达</th>
                  <th scope="col">来源作品 ID</th>
                  <th scope="col">系统接入时间</th>
                  <th scope="col">语料状态</th>
                  <th scope="col">来源与上下文</th>
                </tr>
              </thead>
              <tbody id="voices-body"></tbody>
            </table>
          </div>
          <nav class="pagination" aria-label="用户原声分页">
            <button class="quiet-button" id="previous-page" type="button" disabled>上一页</button>
            <span class="pagination-label" id="page-summary" aria-live="polite">未加载</span>
            <button class="quiet-button" id="next-page" type="button" disabled>下一页</button>
          </nav>
        </section>

        <section class="run-records-view" id="run-records-view" aria-labelledby="run-records-title" hidden>
          <div class="results-heading">
            <div>
              <h2 id="run-records-title">本地研究运行记录</h2>
              <p class="run-records-intro">这里只记录本次怎样冻结输入与目前是否具备执行条件；它不展示评论结论或用户问题。</p>
              <p class="run-records-intro">当前未配置执行器，尚未开始分析。</p>
            </div>
            <p class="result-count" id="run-records-count"></p>
          </div>
          <p class="status-message" id="run-records-status" role="status" aria-live="polite"></p>
          <section class="empty-state run-records-empty" id="run-records-empty" aria-labelledby="run-records-empty-title" hidden>
            <h3 id="run-records-empty-title">尚无待执行研究</h3>
            <p>当前工作空间还没有通过明确确认创建的本地 Run。查看自动研究范围不会创建记录；当前版本也没有执行器。</p>
          </section>
          <div class="table-scroller" id="run-records-table-wrap" tabindex="0" aria-label="研究运行记录表格，可横向滚动" hidden>
            <table class="run-records-table">
              <thead>
                <tr>
                  <th scope="col">创建时间</th>
                  <th scope="col">当前状态</th>
                  <th scope="col">冻结输入</th>
                  <th scope="col">来源作品</th>
                  <th scope="col">查看</th>
                </tr>
              </thead>
              <tbody id="run-records-body"></tbody>
            </table>
          </div>
          <nav class="pagination" id="run-records-pagination" aria-label="运行记录分页" hidden>
            <button class="quiet-button" id="previous-run-records-page" type="button" disabled>上一页</button>
            <span class="pagination-label" id="run-records-page-summary" aria-live="polite">未加载</span>
            <button class="quiet-button" id="next-run-records-page" type="button" disabled>下一页</button>
          </nav>
        </section>
      </main>
    </div>

    <div class="backdrop" id="drawer-backdrop" hidden></div>
    <aside class="drawer" id="voice-drawer" role="dialog" aria-modal="true" aria-labelledby="drawer-title" hidden>
      <div class="drawer-header">
        <h2 class="drawer-title" id="drawer-title">原声与来源证据</h2>
        <button class="icon-button" id="close-drawer" type="button" aria-label="关闭原声详情">×</button>
      </div>
      <section class="drawer-section" aria-labelledby="research-expression-title">
        <h3 id="research-expression-title">清洗后研究表达</h3>
        <p class="drawer-copy" id="drawer-research-expression"></p>
        <p class="context-field-note">仅移除确定性无意义成分；它不替代原始采集原声。</p>
      </section>
      <section class="drawer-section" aria-labelledby="voice-text-title">
        <h3 id="voice-text-title">原始采集原声</h3>
        <p class="drawer-copy" id="drawer-original-voice"></p>
      </section>
      <section class="drawer-section" aria-labelledby="discussion-title">
        <h3 id="discussion-title">已采到的相关讨论</h3>
        <div id="drawer-related-discussion" aria-live="polite"></div>
      </section>
      <section class="drawer-section" aria-labelledby="context-title">
        <h3 id="context-title">作品上下文</h3>
        <div id="drawer-work-context" aria-live="polite"></div>
      </section>
      <section class="drawer-section" aria-labelledby="context-pack-title">
        <h3 id="context-pack-title">研究输入预览</h3>
        <p class="context-field-note">查看未来研究可能接收的已采到文本。当前评论的清洗表达单独作为直接证据；回复和作品内容只用于理解语境。这里不是实际 Prompt，不会执行研究。</p>
        <button class="quiet-button context-pack-button" id="load-context-pack" type="button">查看研究输入预览</button>
        <div class="context-pack-preview" id="drawer-context-pack" aria-live="polite" hidden></div>
      </section>
      <section class="drawer-section" aria-labelledby="source-evidence-title">
        <h3 id="source-evidence-title">来源证据</h3>
        <dl class="drawer-list">
          <div><dt>来源作品 ID</dt><dd class="mono" id="drawer-note-id"></dd></div>
          <div><dt>Evidence ID</dt><dd class="mono" id="drawer-evidence-id"></dd></div>
          <div><dt>来源记录序号</dt><dd class="mono" id="drawer-record-index"></dd></div>
          <div><dt>系统接入时间</dt><dd class="mono" id="drawer-admitted-at"></dd></div>
        </dl>
      </section>
    </aside>

    <aside class="drawer plan-drawer" id="plan-preview-drawer" role="dialog" aria-modal="true" aria-labelledby="plan-preview-title" hidden>
      <div class="drawer-header">
        <h2 class="drawer-title" id="plan-preview-title">自动研究范围</h2>
        <button class="icon-button" id="close-plan-preview" type="button" aria-label="关闭自动研究范围">×</button>
      </div>
      <section class="drawer-section" aria-labelledby="plan-preview-meaning-title">
        <h3 id="plan-preview-meaning-title">这是范围预览，不是开始研究</h3>
        <p class="drawer-copy">查看预览时，它只读取当前清洗语料，按来源作品轮换展示可能进入后续研究的一小段范围。确认前不会创建任务、锁定样本、调用模型、组装上下文或执行研究。</p>
      </section>
      <form class="plan-controls" id="plan-preview-form" novalidate>
        <div>
          <label class="field-label" for="plan-preview-scope">查看范围</label>
          <select class="workspace-input" id="plan-preview-scope" name="scope">
            <option value="available">全部可用</option>
            <option value="ready">可直接研究</option>
            <option value="needs_context">需要上下文</option>
          </select>
        </div>
        <div>
          <label class="field-label" for="plan-preview-limit">显示上限</label>
          <select class="workspace-input" id="plan-preview-limit" name="limit">
            <option value="20">20 条</option>
            <option value="50" selected>50 条</option>
            <option value="100">100 条</option>
          </select>
        </div>
        <button class="quiet-button" id="refresh-plan-preview" type="submit">更新预览</button>
      </form>
      <p class="status-message" id="plan-preview-status" role="status" aria-live="polite"></p>
      <section class="drawer-section" aria-labelledby="plan-preparation-title">
        <h3 id="plan-preparation-title">当前语料准备情况</h3>
        <dl class="plan-totals" id="plan-preparation"></dl>
      </section>
      <section class="drawer-section" aria-labelledby="plan-sources-title">
        <h3 id="plan-sources-title">本次预览中的来源分布</h3>
        <p class="context-field-note">只列出本次实时预览实际选到的来源。它不是全量作品覆盖，也不是优先级或研究价值判断。</p>
        <div class="drawer-table-scroller">
          <table class="drawer-table">
            <thead><tr><th scope="col">来源作品 ID</th><th scope="col">本范围可用</th><th scope="col">本次显示</th></tr></thead>
            <tbody id="plan-sources-body"></tbody>
          </table>
        </div>
      </section>
      <section class="drawer-section" aria-labelledby="plan-candidates-title">
        <h3 id="plan-candidates-title">当前候选原声</h3>
        <p class="context-field-note" id="plan-candidates-note">候选按来源作品轮换；同一来源的第 1 条先于第 2 条出现。范围在每次读取时重新计算，尚未冻结。</p>
        <div class="drawer-table-scroller">
          <table class="drawer-table">
            <thead><tr><th scope="col">清洗后研究表达</th><th scope="col">来源轮次</th><th scope="col">语料状态</th></tr></thead>
            <tbody id="plan-candidates-body"></tbody>
          </table>
        </div>
        <div class="context-notice" id="plan-preview-empty" hidden></div>
      </section>
      <section class="drawer-section plan-preparation-action" aria-labelledby="prepare-run-title">
        <div>
          <h3 id="prepare-run-title">准备本次研究</h3>
          <p class="drawer-copy">确认时会按当前语料重新核对范围，不使用表格中的逐条选择；本次只创建待执行研究。</p>
        </div>
        <button class="primary-button prepare-run-button" id="prepare-run-button" type="button" disabled>准备本次研究</button>
        <div class="run-confirmation" id="run-confirmation" role="region" aria-labelledby="run-confirmation-title" hidden>
          <h4 id="run-confirmation-title">核对后冻结输入</h4>
          <p class="drawer-copy" id="run-confirmation-summary"></p>
          <p class="run-confirmation-note">会创建“待执行研究”。当前版本没有执行器：确认不会调用模型、不生成结论、不扣费。</p>
          <div class="run-confirmation-actions">
            <button class="quiet-button" id="cancel-run-confirmation" type="button">返回预览</button>
            <button class="primary-button" id="confirm-run-button" type="button">确认并冻结输入</button>
          </div>
        </div>
        <div class="run-preparation-result" id="run-preparation-result" role="status" aria-live="polite" tabindex="-1" hidden></div>
      </section>
    </aside>

    <aside class="drawer run-record-drawer" id="run-record-drawer" role="dialog" aria-modal="true" aria-labelledby="run-record-title" hidden>
      <div class="drawer-header">
        <h2 class="drawer-title" id="run-record-title">本次运行记录</h2>
        <button class="icon-button" id="close-run-record" type="button" aria-label="关闭运行记录详情">×</button>
      </div>
      <section class="drawer-section" aria-labelledby="run-record-state-title">
        <h3 id="run-record-state-title">当前状态</h3>
        <p class="drawer-copy" id="run-record-state"></p>
        <p class="context-field-note" id="run-record-execution-note"></p>
      </section>
      <section class="drawer-section" aria-labelledby="run-record-formation-title">
        <h3 id="run-record-formation-title">这批怎样形成</h3>
        <p class="drawer-copy" id="run-record-formation"></p>
        <dl class="run-detail-totals" id="run-record-totals"></dl>
      </section>
      <section class="drawer-section" aria-labelledby="run-record-blocks-title">
        <h3 id="run-record-blocks-title">阻断与未继续执行</h3>
        <p class="context-field-note">需要补足上下文表示输入不能进入未来执行器，不是模型或执行失败。</p>
        <div class="run-record-reasons" id="run-record-reasons" aria-live="polite"></div>
      </section>
      <section class="drawer-section" aria-labelledby="run-record-reference-title">
        <h3 id="run-record-reference-title">Run 回执</h3>
        <dl class="drawer-list">
          <div><dt>本地运行参考</dt><dd class="mono" id="run-record-ref"></dd></div>
          <div><dt>创建时间</dt><dd class="mono" id="run-record-created-at"></dd></div>
        </dl>
      </section>
    </aside>

    <script>
      (() => {
        "use strict";

        const pageLimit = 25;
        const state = {
          workspaceId: "",
          filter: "available",
          activeView: "voices",
          offset: 0,
          total: 0,
          runRecordsOffset: 0,
          runRecordsTotal: 0,
          currentVoice: null,
          lastTrigger: null,
          planLastTrigger: null,
          runRecordLastTrigger: null,
          contextRequestToken: 0,
          contextPackRequestToken: 0,
          planRequestToken: 0,
          runPreparationRequestToken: 0,
          runRecordsRequestToken: 0,
          runRecordDetailRequestToken: 0,
          planPreviewPayload: null,
          runPreparedForCurrentPreview: false
        };
        const form = document.getElementById("workspace-form");
        const workspaceInput = document.getElementById("workspace-id");
        const voiceFilter = document.getElementById("voice-filter");
        const loadButton = document.getElementById("load-voices");
        const pageEyebrow = document.getElementById("page-eyebrow");
        const pageTitle = document.getElementById("page-title");
        const unavailableViewsNote = document.getElementById("unavailable-views-note");
        const voicesTab = document.getElementById("voices-tab");
        const runRecordsTab = document.getElementById("run-records-tab");
        const previousButton = document.getElementById("previous-page");
        const nextButton = document.getElementById("next-page");
        const statusMessage = document.getElementById("status-message");
        const initialState = document.getElementById("initial-state");
        const emptyState = document.getElementById("empty-state");
        const results = document.getElementById("results");
        const voicesBody = document.getElementById("voices-body");
        const resultCount = document.getElementById("result-count");
        const pageSummary = document.getElementById("page-summary");
        const runRecordsView = document.getElementById("run-records-view");
        const runRecordsStatus = document.getElementById("run-records-status");
        const runRecordsCount = document.getElementById("run-records-count");
        const runRecordsEmpty = document.getElementById("run-records-empty");
        const runRecordsTableWrap = document.getElementById("run-records-table-wrap");
        const runRecordsBody = document.getElementById("run-records-body");
        const runRecordsPagination = document.getElementById("run-records-pagination");
        const previousRunRecordsButton = document.getElementById("previous-run-records-page");
        const nextRunRecordsButton = document.getElementById("next-run-records-page");
        const runRecordsPageSummary = document.getElementById("run-records-page-summary");
        const drawer = document.getElementById("voice-drawer");
        const drawerBackdrop = document.getElementById("drawer-backdrop");
        const closeDrawerButton = document.getElementById("close-drawer");
        const relatedDiscussion = document.getElementById("drawer-related-discussion");
        const workContext = document.getElementById("drawer-work-context");
        const contextPackButton = document.getElementById("load-context-pack");
        const contextPackPreview = document.getElementById("drawer-context-pack");
        const planPreviewButton = document.getElementById("plan-preview-button");
        const planPreviewDrawer = document.getElementById("plan-preview-drawer");
        const closePlanPreviewButton = document.getElementById("close-plan-preview");
        const planPreviewForm = document.getElementById("plan-preview-form");
        const planPreviewScope = document.getElementById("plan-preview-scope");
        const planPreviewLimit = document.getElementById("plan-preview-limit");
        const refreshPlanPreviewButton = document.getElementById("refresh-plan-preview");
        const planPreviewStatus = document.getElementById("plan-preview-status");
        const planPreparation = document.getElementById("plan-preparation");
        const planSourcesBody = document.getElementById("plan-sources-body");
        const planCandidatesBody = document.getElementById("plan-candidates-body");
        const planPreviewEmpty = document.getElementById("plan-preview-empty");
        const prepareRunButton = document.getElementById("prepare-run-button");
        const runConfirmation = document.getElementById("run-confirmation");
        const runConfirmationSummary = document.getElementById("run-confirmation-summary");
        const cancelRunConfirmationButton = document.getElementById("cancel-run-confirmation");
        const confirmRunButton = document.getElementById("confirm-run-button");
        const runPreparationResult = document.getElementById("run-preparation-result");
        const runRecordDrawer = document.getElementById("run-record-drawer");
        const closeRunRecordButton = document.getElementById("close-run-record");
        const runRecordState = document.getElementById("run-record-state");
        const runRecordExecutionNote = document.getElementById("run-record-execution-note");
        const runRecordFormation = document.getElementById("run-record-formation");
        const runRecordTotals = document.getElementById("run-record-totals");
        const runRecordReasons = document.getElementById("run-record-reasons");
        const runRecordRef = document.getElementById("run-record-ref");
        const runRecordCreatedAt = document.getElementById("run-record-created-at");

        function setStatus(message, kind) {
          statusMessage.textContent = message;
          statusMessage.dataset.state = kind || "";
        }

        function clearRows() {
          voicesBody.replaceChildren();
        }

        function setLoading(loading) {
          loadButton.disabled = loading;
          loadButton.textContent = loading
            ? "正在加载…"
            : state.activeView === "runs" ? "加载运行记录" : "加载用户原声";
          previousButton.disabled = loading || state.offset === 0;
          nextButton.disabled = true;
          planPreviewButton.disabled = loading || state.activeView !== "voices" || !state.workspaceId.trim();
        }

        function appendCell(row, label, content, className) {
          const cell = document.createElement("td");
          cell.dataset.label = label;
          const element = document.createElement("span");
          element.className = className || "";
          element.textContent = content;
          cell.append(element);
          row.append(cell);
        }

        function appendFactCell(row, label, text, context) {
          const cell = document.createElement("td");
          cell.dataset.label = label;
          const fact = document.createElement("span");
          fact.className = context ? "fact-state context" : "fact-state";
          fact.textContent = text;
          cell.append(fact);
          row.append(cell);
        }

        function renderVoices(voices) {
          clearRows();
          for (const voice of voices) {
            const row = document.createElement("tr");
            const textCell = document.createElement("td");
            textCell.dataset.label = "可研究表达";
            const text = document.createElement("span");
            text.className = "voice-text";
            text.textContent = voice.research_text;
            const detail = document.createElement("button");
            detail.className = "detail-button";
            detail.type = "button";
            detail.textContent = "查看原声与来源";
            detail.addEventListener("click", () => openDrawer(voice, detail));
            textCell.append(text, detail);
            row.append(textCell);
            appendCell(row, "来源作品 ID", voice.source_note_id, "mono metadata");
            appendCell(row, "系统接入时间", voice.current_admitted_at, "mono metadata");
            appendFactCell(row, "语料状态", readinessLabel(voice.readiness), voice.readiness === "needs_context");
            appendFactCell(row, "来源与上下文", "打开详情后按来源读取", true);
            voicesBody.append(row);
          }
        }

        function readinessLabel(readiness) {
          return readiness === "needs_context" ? "需要上下文" : "可直接研究";
        }

        function updatePagination(pagination, preparation, voicesCount) {
          state.total = pagination.total;
          state.offset = pagination.offset;
          const start = voicesCount === 0 ? 0 : pagination.offset + 1;
          const end = pagination.offset + voicesCount;
          const awaiting = preparation && Number.isInteger(preparation.awaiting_cleaning_total)
            ? preparation.awaiting_cleaning_total
            : 0;
          resultCount.textContent = awaiting > 0
            ? `可用 ${pagination.total} 条 · 等待清洗 ${awaiting} 条 · 当前显示 ${start}-${end}`
            : `可用 ${pagination.total} 条 · 当前显示 ${start}-${end}`;
          pageSummary.textContent = voicesCount === 0 ? "没有更多原声" : `${start}-${end} / ${pagination.total}`;
          previousButton.disabled = pagination.offset === 0;
          nextButton.disabled = pagination.offset + voicesCount >= pagination.total;
        }

        function setVisibleState(kind) {
          initialState.hidden = kind !== "initial";
          emptyState.hidden = kind !== "empty";
          results.hidden = kind !== "results";
        }

        function showRunRecordsView() {
          initialState.hidden = true;
          emptyState.hidden = true;
          results.hidden = true;
          runRecordsView.hidden = false;
        }

        function setRunRecordsStatus(message, kind) {
          runRecordsStatus.textContent = message;
          runRecordsStatus.dataset.state = kind || "";
        }

        function clearRunRecords() {
          runRecordsBody.replaceChildren();
          runRecordsCount.textContent = "";
          runRecordsEmpty.hidden = true;
          runRecordsTableWrap.hidden = true;
          runRecordsPagination.hidden = true;
          runRecordsPageSummary.textContent = "未加载";
          previousRunRecordsButton.disabled = true;
          nextRunRecordsButton.disabled = true;
        }

        function setRunRecordsLoading(loading) {
          loadButton.disabled = loading;
          loadButton.textContent = loading ? "正在读取…" : "加载运行记录";
          previousRunRecordsButton.disabled = loading || state.runRecordsOffset === 0;
          nextRunRecordsButton.disabled = true;
        }

        function isRunRecordsPayload(payload) {
          return payload
            && payload.pagination
            && Number.isInteger(payload.pagination.total)
            && Number.isInteger(payload.pagination.limit)
            && Number.isInteger(payload.pagination.offset)
            && Array.isArray(payload.runs)
            && typeof payload.read_note === "string";
        }

        function isRunRecord(payload) {
          return payload
            && typeof payload.run_ref === "string"
            && typeof payload.created_at === "string"
            && typeof payload.run_state === "string"
            && Number.isInteger(payload.frozen_input_total)
            && Number.isInteger(payload.awaiting_execution_total)
            && Number.isInteger(payload.blocked_needs_context_total)
            && Number.isInteger(payload.execution_excluded_total)
            && Number.isInteger(payload.finalized_conclusion_total)
            && Number.isInteger(payload.source_coverage_total);
        }

        function runInputSummary(run) {
          const parts = [`冻结 ${run.frozen_input_total} 条`];
          if (run.awaiting_execution_total > 0) parts.push(`待执行 ${run.awaiting_execution_total} 条`);
          if (run.blocked_needs_context_total > 0) parts.push(`上下文阻断 ${run.blocked_needs_context_total} 条`);
          if (run.execution_excluded_total > 0) parts.push(`未继续执行 ${run.execution_excluded_total} 条`);
          if (run.finalized_conclusion_total > 0) parts.push(`已记录最终状态 ${run.finalized_conclusion_total} 条`);
          return parts.join(" · ");
        }

        function renderRunRecords(payload) {
          clearRunRecords();
          state.runRecordsTotal = payload.pagination.total;
          state.runRecordsOffset = payload.pagination.offset;
          const runs = payload.runs.filter(isRunRecord);
          runRecordsCount.textContent = payload.pagination.total === 0
            ? "尚无本地 Run"
            : `共 ${payload.pagination.total} 个本地 Run`;
          if (payload.pagination.total === 0) {
            runRecordsEmpty.hidden = false;
            setRunRecordsStatus("尚无待执行研究。自动研究范围预览不会创建记录。", "");
            return;
          }
          for (const run of runs) {
            const row = document.createElement("tr");
            appendCell(row, "创建时间", run.created_at, "mono metadata");
            appendCell(row, "当前状态", run.run_state, "fact-state context");
            appendCell(row, "冻结输入", runInputSummary(run), "metadata");
            appendCell(row, "来源作品", `${run.source_coverage_total} 个`, "metadata");
            const actionCell = document.createElement("td");
            actionCell.dataset.label = "查看";
            const action = document.createElement("button");
            action.className = "detail-button";
            action.type = "button";
            action.textContent = "查看本次记录";
            action.addEventListener("click", () => openRunRecordDrawer(run.run_ref, action));
            actionCell.append(action);
            row.append(actionCell);
            runRecordsBody.append(row);
          }
          runRecordsTableWrap.hidden = false;
          runRecordsPagination.hidden = false;
          const start = runs.length === 0 ? 0 : payload.pagination.offset + 1;
          const end = payload.pagination.offset + runs.length;
          runRecordsPageSummary.textContent = runs.length === 0 ? "没有更多记录" : `${start}-${end} / ${payload.pagination.total}`;
          previousRunRecordsButton.disabled = payload.pagination.offset === 0;
          nextRunRecordsButton.disabled = payload.pagination.offset + runs.length >= payload.pagination.total;
          setRunRecordsStatus(payload.read_note, "");
        }

        async function loadRunRecords(offset, trigger) {
          const workspaceId = state.workspaceId.trim();
          showRunRecordsView();
          if (!workspaceId) {
            clearRunRecords();
            setRunRecordsStatus("请先输入并确认工作空间 ID。当前没有请求运行记录。", "error");
            workspaceInput.focus();
            return;
          }
          state.runRecordLastTrigger = trigger || document.activeElement;
          const requestToken = state.runRecordsRequestToken + 1;
          state.runRecordsRequestToken = requestToken;
          setRunRecordsLoading(true);
          setRunRecordsStatus("正在只读加载本地运行记录；不会启动分析…", "loading");
          try {
            const parameters = new URLSearchParams({
              workspace_id: workspaceId,
              limit: String(pageLimit),
              offset: String(offset)
            });
            const response = await fetch(`/api/v0/comment-research/runs?${parameters.toString()}`, {
              headers: { "Accept": "application/json" },
              credentials: "same-origin"
            });
            const payload = await response.json().catch(() => null);
            if (requestToken !== state.runRecordsRequestToken || state.activeView !== "runs") return;
            if (!response.ok || !isRunRecordsPayload(payload) || !payload.runs.every(isRunRecord)) {
              throw new Error(errorMessage(payload));
            }
            renderRunRecords(payload);
          } catch (error) {
            if (requestToken !== state.runRecordsRequestToken || state.activeView !== "runs") return;
            clearRunRecords();
            setRunRecordsStatus(
              error instanceof Error ? error.message : "无法读取本地运行记录。请稍后重试。",
              "error"
            );
          } finally {
            if (requestToken === state.runRecordsRequestToken) setRunRecordsLoading(false);
          }
        }

        function isRunRecordDetailPayload(payload) {
          return payload
            && isRunRecord(payload.run)
            && typeof payload.formation_note === "string"
            && typeof payload.execution_note === "string"
            && Array.isArray(payload.blocked_or_failure_reasons);
        }

        function appendRunDetailTotal(label, value, detail) {
          const item = document.createElement("div");
          const term = document.createElement("dt");
          const definition = document.createElement("dd");
          term.textContent = label;
          definition.textContent = String(value);
          item.append(term, definition);
          if (detail) {
            const note = document.createElement("p");
            note.className = "plan-total-note";
            note.textContent = detail;
            item.append(note);
          }
          runRecordTotals.append(item);
        }

        function renderRunRecordDetail(payload) {
          const run = payload.run;
          runRecordState.textContent = run.run_state;
          runRecordExecutionNote.textContent = payload.execution_note;
          runRecordFormation.textContent = payload.formation_note;
          runRecordRef.textContent = run.run_ref;
          runRecordCreatedAt.textContent = run.created_at;
          runRecordTotals.replaceChildren();
          appendRunDetailTotal("冻结输入", run.frozen_input_total);
          appendRunDetailTotal("待执行", run.awaiting_execution_total, "当前没有执行器，尚未开始分析");
          appendRunDetailTotal("上下文阻断", run.blocked_needs_context_total, "阻断不是执行失败");
          appendRunDetailTotal("未继续执行", run.execution_excluded_total);
          appendRunDetailTotal("已记录最终状态", run.finalized_conclusion_total);
          appendRunDetailTotal("来源作品覆盖", run.source_coverage_total);
          runRecordReasons.replaceChildren();
          if (payload.blocked_or_failure_reasons.length === 0) {
            const note = document.createElement("p");
            note.className = "context-field-note";
            note.textContent = "当前没有阻断或未继续执行原因。";
            runRecordReasons.append(note);
            return;
          }
          for (const reason of payload.blocked_or_failure_reasons) {
            if (!reason || typeof reason.reason !== "string" || !Number.isInteger(reason.item_total)) continue;
            const item = document.createElement("p");
            item.className = "run-record-reason";
            item.textContent = `${reason.reason}：${reason.item_total} 条`;
            runRecordReasons.append(item);
          }
        }

        async function loadRunRecordDetail(runRef, requestToken) {
          const parameters = new URLSearchParams({ workspace_id: state.workspaceId.trim() });
          try {
            const response = await fetch(`/api/v0/comment-research/runs/${encodeURIComponent(runRef)}?${parameters.toString()}`, {
              headers: { "Accept": "application/json" },
              credentials: "same-origin"
            });
            const payload = await response.json().catch(() => null);
            if (requestToken !== state.runRecordDetailRequestToken || runRecordDrawer.hidden) return;
            if (!response.ok || !isRunRecordDetailPayload(payload)) {
              throw new Error(errorMessage(payload));
            }
            renderRunRecordDetail(payload);
          } catch (error) {
            if (requestToken !== state.runRecordDetailRequestToken || runRecordDrawer.hidden) return;
            runRecordState.textContent = "无法读取本次运行记录";
            runRecordExecutionNote.textContent = error instanceof Error ? error.message : "请返回运行记录后重试。";
            runRecordFormation.textContent = "当前没有写入、重试或执行任何研究。";
            runRecordTotals.replaceChildren();
            runRecordReasons.replaceChildren();
            runRecordRef.textContent = runRef;
            runRecordCreatedAt.textContent = "未取得";
          }
        }

        function openRunRecordDrawer(runRef, trigger) {
          state.runRecordLastTrigger = trigger;
          if (!drawer.hidden) {
            state.contextRequestToken += 1;
            drawer.hidden = true;
          }
          if (!planPreviewDrawer.hidden) {
            state.planRequestToken += 1;
            planPreviewDrawer.hidden = true;
          }
          drawerBackdrop.hidden = false;
          runRecordDrawer.hidden = false;
          runRecordState.textContent = "正在读取本次冻结记录…";
          runRecordExecutionNote.textContent = "这不会启动分析或调用模型。";
          runRecordFormation.textContent = "";
          runRecordTotals.replaceChildren();
          runRecordReasons.replaceChildren();
          runRecordRef.textContent = runRef;
          runRecordCreatedAt.textContent = "正在读取…";
          const requestToken = state.runRecordDetailRequestToken + 1;
          state.runRecordDetailRequestToken = requestToken;
          closeRunRecordButton.focus();
          loadRunRecordDetail(runRef, requestToken);
        }

        function closeRunRecordDrawer() {
          state.runRecordDetailRequestToken += 1;
          runRecordDrawer.hidden = true;
          if (drawer.hidden && planPreviewDrawer.hidden) drawerBackdrop.hidden = true;
          if (state.runRecordLastTrigger instanceof HTMLElement) {
            state.runRecordLastTrigger.focus();
          }
        }

        function activateView(view, trigger) {
          if (state.activeView === view) return;
          state.activeView = view;
          state.runRecordsRequestToken += 1;
          voicesTab.setAttribute("aria-selected", view === "voices" ? "true" : "false");
          runRecordsTab.setAttribute("aria-selected", view === "runs" ? "true" : "false");
          voiceFilter.disabled = view === "runs";
          pageEyebrow.textContent = view === "runs" ? "评论研究 · 本地 Run V1" : "评论研究 · 清洗语料 V1";
          pageTitle.textContent = view === "runs" ? "运行记录" : "用户原声";
          unavailableViewsNote.textContent = view === "runs"
            ? "概览、用户问题和变化观察尚未具备来源事实；运行记录只读取本地冻结批次。"
            : "概览、用户问题和变化观察尚未具备来源事实，本页不会用空白仪表盘代替。";
          planPreviewButton.disabled = view !== "voices" || !state.workspaceId.trim();
          if (view === "runs") {
            loadRunRecords(0, trigger);
          } else {
            runRecordsView.hidden = true;
            if (state.workspaceId.trim()) {
              loadVoices(0, trigger);
            } else {
              setVisibleState("initial");
            }
          }
        }

        function errorMessage(response) {
          if (response && response.error && typeof response.error.message === "string") {
            return response.error.message;
          }
          return "无法读取用户原声。请检查工作空间 ID 和本地数据库连接后重试。";
        }

        async function loadVoices(offset, trigger) {
          const workspaceId = state.workspaceId.trim();
          if (!workspaceId) {
            setVisibleState("initial");
            setStatus("请先输入工作空间 ID。尚未请求任何用户原声。", "error");
            workspaceInput.focus();
            return;
          }

          state.lastTrigger = trigger || document.activeElement;
          setLoading(true);
          setStatus("正在以只读方式加载当前已接入的用户原声…", "loading");
          try {
            const parameters = new URLSearchParams({ workspace_id: workspaceId, filter: state.filter, limit: String(pageLimit), offset: String(offset) });
            const response = await fetch(`/api/v0/comment-research/voices?${parameters.toString()}`, {
              headers: { "Accept": "application/json" },
              credentials: "same-origin"
            });
            const payload = await response.json().catch(() => null);
            if (!response.ok || !payload || !payload.pagination || !payload.preparation || !Array.isArray(payload.voices)) {
              throw new Error(errorMessage(payload));
            }

            renderVoices(payload.voices);
            updatePagination(payload.pagination, payload.preparation, payload.voices.length);
            if (payload.voices.length === 0) {
              setVisibleState("empty");
              const awaiting = Number.isInteger(payload.preparation.awaiting_cleaning_total)
                ? payload.preparation.awaiting_cleaning_total
                : 0;
              setStatus(awaiting > 0
                ? `当前筛选暂无可显示表达；仍有 ${awaiting} 条当前原声等待本地清洗物化。`
                : "已读取当前范围；未发现符合当前语料状态的用户原声。", "");
            } else {
              setVisibleState("results");
              setStatus("列表显示确定性清洗后的研究表达；打开任意一条可核对原始采集原声与来源证据。", "");
            }
            planPreviewButton.disabled = false;
          } catch (error) {
            clearRows();
            setVisibleState("initial");
            setStatus(error instanceof Error ? error.message : "无法读取用户原声。请稍后重试。", "error");
            pageSummary.textContent = "读取失败";
          } finally {
            setLoading(false);
          }
        }

        function replaceContextMessage(target, message, stateName) {
          target.replaceChildren();
          const notice = document.createElement("p");
          notice.className = stateName === "error" ? "context-notice context-notice-error" : "context-notice";
          notice.textContent = message;
          target.append(notice);
        }

        function renderContextLoading() {
          const message = "正在按当前来源证据读取已采到的讨论与作品上下文…";
          replaceContextMessage(relatedDiscussion, message, "loading");
          replaceContextMessage(workContext, message, "loading");
        }

        function renderContextUnavailable() {
          const message = "尚未取得与当前原声正文相匹配的完整来源上下文。它不表示平台没有作品或其他讨论。";
          replaceContextMessage(relatedDiscussion, message, "unavailable");
          replaceContextMessage(workContext, message, "unavailable");
        }

        function renderContextError(message) {
          const safeMessage = message || "无法读取已采到的上下文。当前原声与其来源证据仍可查看，请稍后重试。";
          document.getElementById("drawer-original-voice").textContent = "无法读取当前来源证据中的原始采集原声；清洗后研究表达仍保留在上方。";
          replaceContextMessage(relatedDiscussion, safeMessage, "error");
          replaceContextMessage(workContext, safeMessage, "error");
        }

        function appendDefinition(root, label, value, className) {
          const item = document.createElement("div");
          const term = document.createElement("dt");
          const definition = document.createElement("dd");
          term.textContent = label;
          definition.className = className || "";
          definition.textContent = value;
          item.append(term, definition);
          root.append(item);
        }

        function appendContextText(root, label, value) {
          const item = document.createElement("div");
          item.className = "context-field";
          const heading = document.createElement("p");
          heading.className = "context-field-label";
          heading.textContent = label;
          const text = document.createElement("p");
          text.className = "drawer-copy";
          if (value && value.availability === "observed" && typeof value.text === "string") {
            text.textContent = value.text;
          } else if (value && value.availability === "blank") {
            text.className = "context-field-note";
            text.textContent = `来源包已提供${label}字段，但内容为空。`;
          } else {
            text.className = "context-field-note";
            text.textContent = `本组来源未提供${label}。`;
          }
          item.append(heading, text);
          root.append(item);
        }

        function relationshipLabels(relationship) {
          const labels = [];
          if (relationship && relationship.root_comment === true) labels.push("根评论");
          if (relationship && relationship.parent_comment === true) labels.push("父评论");
          if (relationship && relationship.reply_to_comment === true) labels.push("被回复对象");
          return labels;
        }

        function renderRelatedDiscussion(items) {
          relatedDiscussion.replaceChildren();
          if (!Array.isArray(items) || items.length === 0) {
            replaceContextMessage(
              relatedDiscussion,
              "当前已接入的这组来源中，没有以根评论、父评论或被回复对象关系指向当前原声的回复。它不表示平台没有其他讨论。",
              "unavailable"
            );
            return;
          }
          const list = document.createElement("div");
          list.className = "discussion-list";
          for (const item of items) {
            const entry = document.createElement("article");
            entry.className = "discussion-entry";
            const text = document.createElement("p");
            text.className = "drawer-copy";
            text.textContent = typeof item.text === "string" ? item.text : "来源回复正文不可读取。";
            const relation = document.createElement("p");
            relation.className = "discussion-meta";
            const labels = relationshipLabels(item.relationship);
            relation.textContent = labels.length > 0
              ? `与当前原声的已观察关系：${labels.join("、")}`
              : "与当前原声的已观察关系未取得。";
            const evidence = document.createElement("p");
            evidence.className = "discussion-meta mono";
            const source = item.source_evidence || {};
            evidence.textContent = `Evidence ${typeof source.evidence_id === "string" ? source.evidence_id : "未取得"} · 记录 ${Number.isInteger(source.record_index) ? source.record_index : "未取得"}`;
            entry.append(text, relation, evidence);
            list.append(entry);
          }
          relatedDiscussion.append(list);
        }

        function renderWorkContext(context) {
          workContext.replaceChildren();
          if (!context || !context.source_evidence) {
            renderContextUnavailable();
            return;
          }
          const fields = document.createElement("div");
          fields.className = "context-fields";
          appendContextText(fields, "作品标题", context.title);
          appendContextText(fields, "作品正文", context.body_text);
          const evidence = document.createElement("dl");
          evidence.className = "drawer-list context-source";
          appendDefinition(
            evidence,
            "作品上下文来源 Evidence",
            typeof context.source_evidence.evidence_id === "string" ? context.source_evidence.evidence_id : "未取得",
            "mono"
          );
          appendDefinition(
            evidence,
            "作品上下文来源记录序号",
            Number.isInteger(context.source_evidence.record_index) ? String(context.source_evidence.record_index) : "未取得",
            "mono"
          );
          workContext.append(fields, evidence);
        }

        function clearContextPackPreview() {
          state.contextPackRequestToken += 1;
          contextPackButton.disabled = false;
          contextPackButton.textContent = "查看研究输入预览";
          contextPackPreview.hidden = true;
          contextPackPreview.replaceChildren();
        }

        function setContextPackLoading(loading) {
          contextPackButton.disabled = loading;
          contextPackButton.textContent = loading ? "正在读取预览…" : "查看研究输入预览";
        }

        function appendContextPackText(root, title, value, unavailableMessage) {
          const section = document.createElement("section");
          section.className = "context-pack-subsection";
          const heading = document.createElement("h4");
          heading.textContent = title;
          const text = document.createElement("p");
          text.className = "drawer-copy";
          if (value && typeof value.text === "string") {
            text.textContent = value.text;
          } else {
            text.className = "context-field-note";
            text.textContent = unavailableMessage;
          }
          section.append(heading, text);
          if (value && value.truncated === true) {
            const note = document.createElement("p");
            note.className = "context-field-note";
            note.textContent = "此处只保留了来源文本的前段，完整内容没有在预览中展示。";
            section.append(note);
          }
          root.append(section);
        }

        function appendContextPackDiscussion(root, discussion) {
          const section = document.createElement("section");
          section.className = "context-pack-subsection";
          const heading = document.createElement("h4");
          heading.textContent = "讨论语境";
          section.append(heading);
          const excerpts = discussion && Array.isArray(discussion.excerpts) ? discussion.excerpts : [];
          if (!discussion || discussion.availability !== "available") {
            const note = document.createElement("p");
            note.className = "context-field-note";
            note.textContent = "当前没有正文匹配的已采到讨论语境。这不表示平台没有其他讨论。";
            section.append(note);
          } else if (excerpts.length === 0) {
            const note = document.createElement("p");
            note.className = "context-field-note";
            note.textContent = "当前已采到的这组来源中，没有关联到这条原声的回复可放入预览。它不表示完整评论树为空。";
            section.append(note);
          } else {
            const list = document.createElement("div");
            list.className = "discussion-list";
            for (const excerpt of excerpts) {
              const entry = document.createElement("article");
              entry.className = "discussion-entry";
              const text = document.createElement("p");
              text.className = "drawer-copy";
              text.textContent = excerpt && excerpt.text && typeof excerpt.text.text === "string"
                ? excerpt.text.text
                : "已采到的讨论文本不可读取。";
              const relation = document.createElement("p");
              relation.className = "discussion-meta";
              const labels = relationshipLabels(excerpt && excerpt.relationship);
              relation.textContent = labels.length > 0
                ? `与当前原声的已观察关系：${labels.join("、")}`
                : "与当前原声的已观察关系未取得。";
              entry.append(text, relation);
              if (excerpt && excerpt.text && excerpt.text.truncated === true) {
                const note = document.createElement("p");
                note.className = "context-field-note";
                note.textContent = "此处只保留了这条讨论的前段。";
                entry.append(note);
              }
              list.append(entry);
            }
            section.append(list);
          }
          root.append(section);
        }

        function appendContextPackWork(root, work) {
          const section = document.createElement("section");
          section.className = "context-pack-subsection";
          const heading = document.createElement("h4");
          heading.textContent = "作品语境";
          section.append(heading);
          if (!work || work.availability !== "available") {
            const note = document.createElement("p");
            note.className = "context-field-note";
            note.textContent = "当前没有正文匹配的已采到作品语境。这不表示平台没有作品正文。";
            section.append(note);
          } else {
            const fields = document.createElement("div");
            fields.className = "context-fields";
            appendContextText(fields, "作品标题", work.title);
            appendContextText(fields, "作品正文", work.body_text);
            section.append(fields);
          }
          root.append(section);
        }

        function appendContextPackBoundary(root, payload) {
          const section = document.createElement("section");
          section.className = "context-pack-subsection";
          const heading = document.createElement("h4");
          heading.textContent = "本次省略与边界";
          const budget = payload.budget || {};
          const budgetNote = document.createElement("p");
          budgetNote.className = "context-field-note";
          const included = Number.isInteger(budget.included_characters) ? budget.included_characters : "未取得";
          const total = Number.isInteger(budget.total_character_limit) ? budget.total_character_limit : "未取得";
          budgetNote.textContent = `本预览纳入 ${included} / ${total} 个字符。当前原声、讨论和作品字段各有固定上限；这里不包含任何执行信息。`;
          section.append(heading, budgetNote);
          const omissions = Array.isArray(payload.omissions) ? payload.omissions : [];
          if (omissions.length > 0) {
            const list = document.createElement("ul");
            list.className = "context-pack-omissions";
            for (const omission of omissions) {
              if (typeof omission !== "string") continue;
              const item = document.createElement("li");
              item.textContent = omission;
              list.append(item);
            }
            section.append(list);
          } else {
            const note = document.createElement("p");
            note.className = "context-field-note";
            note.textContent = "当前已读取的字段没有因本预览上限被省略；这不代表来源覆盖完整。";
            section.append(note);
          }
          if (typeof payload.future_execution_note === "string") {
            const executionNote = document.createElement("p");
            executionNote.className = "context-notice";
            executionNote.textContent = payload.future_execution_note;
            section.append(executionNote);
          }
          root.append(section);
        }

        function isContextPackPayload(payload) {
          return payload
            && payload.preview_state === "source_backed_read_only"
            && (payload.readiness === "ready" || payload.readiness === "needs_context")
            && payload.direct_comment_evidence
            && typeof payload.direct_comment_evidence.text === "string"
            && payload.discussion_context
            && payload.work_context
            && payload.budget
            && Array.isArray(payload.omissions);
        }

        function renderContextPackPreview(payload) {
          contextPackPreview.replaceChildren();
          appendContextPackText(
            contextPackPreview,
            "当前评论的直接证据",
            payload.direct_comment_evidence,
            "当前清洗后研究表达不可读取。"
          );
          appendContextPackDiscussion(contextPackPreview, payload.discussion_context);
          appendContextPackWork(contextPackPreview, payload.work_context);
          appendContextPackBoundary(contextPackPreview, payload);
          contextPackPreview.hidden = false;
        }

        async function loadContextPack() {
          const currentVoice = state.currentVoice;
          if (!currentVoice || drawer.hidden) return;
          const requestToken = state.contextPackRequestToken + 1;
          state.contextPackRequestToken = requestToken;
          setContextPackLoading(true);
          try {
            const parameters = new URLSearchParams({
              workspace_id: state.workspaceId,
              evidence_id: currentVoice.source_evidence.evidence_id,
              record_index: String(currentVoice.source_evidence.record_index)
            });
            const response = await fetch(`/api/v0/comment-research/voices/context-pack?${parameters.toString()}`, {
              headers: { "Accept": "application/json" },
              credentials: "same-origin"
            });
            const payload = await response.json().catch(() => null);
            if (requestToken !== state.contextPackRequestToken || drawer.hidden) return;
            if (!response.ok || !isContextPackPayload(payload)) {
              throw new Error("无法读取研究输入预览。当前原声与来源详情仍可查看，请稍后重试。");
            }
            renderContextPackPreview(payload);
          } catch (error) {
            if (requestToken !== state.contextPackRequestToken || drawer.hidden) return;
            contextPackPreview.replaceChildren();
            const notice = document.createElement("p");
            notice.className = "context-notice context-notice-error";
            notice.textContent = error instanceof Error ? error.message : "无法读取研究输入预览。";
            contextPackPreview.append(notice);
            contextPackPreview.hidden = false;
          } finally {
            if (requestToken === state.contextPackRequestToken) setContextPackLoading(false);
          }
        }

        function setPlanPreviewStatus(message, kind) {
          planPreviewStatus.textContent = message;
          planPreviewStatus.dataset.state = kind || "";
        }

        function setPlanPreviewLoading(loading) {
          refreshPlanPreviewButton.disabled = loading;
          refreshPlanPreviewButton.textContent = loading ? "正在读取…" : "更新预览";
          planPreviewScope.disabled = loading;
          planPreviewLimit.disabled = loading;
          prepareRunButton.disabled = loading || state.runPreparedForCurrentPreview || !state.planPreviewPayload || state.planPreviewPayload.candidates.length === 0;
        }

        function clearRunPreparationState() {
          state.runPreparationRequestToken += 1;
          state.runPreparedForCurrentPreview = false;
          runConfirmation.hidden = true;
          runConfirmationSummary.textContent = "";
          runPreparationResult.hidden = true;
          runPreparationResult.removeAttribute("data-state");
          runPreparationResult.replaceChildren();
          confirmRunButton.disabled = false;
          confirmRunButton.textContent = "确认并冻结输入";
          cancelRunConfirmationButton.disabled = false;
          planPreviewScope.disabled = false;
          planPreviewLimit.disabled = false;
          refreshPlanPreviewButton.disabled = false;
        }

        function clearPlanPreviewState() {
          state.planPreviewPayload = null;
          clearRunPreparationState();
          prepareRunButton.disabled = true;
          prepareRunButton.textContent = "准备本次研究";
        }

        function setRunPreparationLoading(loading) {
          const hasCandidates = state.planPreviewPayload && state.planPreviewPayload.candidates.length > 0;
          prepareRunButton.disabled = loading || state.runPreparedForCurrentPreview || !hasCandidates;
          confirmRunButton.disabled = loading;
          cancelRunConfirmationButton.disabled = loading;
          confirmRunButton.textContent = loading ? "正在冻结输入…" : "确认并冻结输入";
        }

        function appendPlanTotal(label, value, detail) {
          const item = document.createElement("div");
          const term = document.createElement("dt");
          const definition = document.createElement("dd");
          term.textContent = label;
          definition.textContent = String(value);
          if (detail) {
            const note = document.createElement("p");
            note.className = "plan-total-note";
            note.textContent = detail;
            item.append(term, definition, note);
          } else {
            item.append(term, definition);
          }
          planPreparation.append(item);
        }

        function clearPlanPreviewRows() {
          planPreparation.replaceChildren();
          planSourcesBody.replaceChildren();
          planCandidatesBody.replaceChildren();
          planPreviewEmpty.hidden = true;
          planPreviewEmpty.replaceChildren();
          clearPlanPreviewState();
        }

        function planScopeLabel(scope) {
          if (scope === "ready") return "可直接研究";
          if (scope === "needs_context") return "需要上下文";
          return "全部可用";
        }

        function renderPlanPreparation(preparation) {
          appendPlanTotal("当前原声", preparation.current_total, "当前 Comment Current 投影的数量");
          appendPlanTotal("可用语料", preparation.available_total, "完成 V1 清洗、可进入范围预览");
          appendPlanTotal("可直接研究", preparation.ready_total);
          appendPlanTotal("需要上下文", preparation.needs_context_total, "后续研究需要另行读取已采上下文；本页没有组装它");
          appendPlanTotal("等待清洗", preparation.awaiting_cleaning_total, "不会由预览自动物化");
          appendPlanTotal("确定性排除", preparation.excluded_total, "纯无效或异常语料，不进入候选");
        }

        function appendPlanTableCell(row, text, className) {
          const cell = document.createElement("td");
          const content = document.createElement("span");
          content.className = className || "";
          content.textContent = text;
          cell.append(content);
          row.append(cell);
        }

        function renderPlanSources(sources) {
          planSourcesBody.replaceChildren();
          for (const source of sources) {
            const row = document.createElement("tr");
            appendPlanTableCell(row, source.source_note_id, "mono");
            appendPlanTableCell(row, `${source.eligible_total} 条`, "metadata");
            appendPlanTableCell(row, `${source.selected_total} 条`, "metadata");
            planSourcesBody.append(row);
          }
        }

        function renderPlanCandidates(candidates) {
          planCandidatesBody.replaceChildren();
          for (const candidate of candidates) {
            const row = document.createElement("tr");
            appendPlanTableCell(row, candidate.research_text, "plan-expression");
            appendPlanTableCell(row, `该来源第 ${candidate.source_rotation_turn} 轮`, "metadata");
            const status = candidate.readiness === "needs_context"
              ? "需要上下文"
              : "可直接研究";
            appendPlanTableCell(row, status, candidate.readiness === "needs_context" ? "plan-caution" : "");
            planCandidatesBody.append(row);
          }
        }

        function renderPlanEmpty(preparation, scope) {
          const message = document.createElement("p");
          if (preparation.current_total === 0) {
            message.textContent = "当前工作空间还没有已接入的用户原声，因此没有可预览的范围。";
          } else if (preparation.available_total === 0 && preparation.awaiting_cleaning_total > 0) {
            message.textContent = "当前原声尚在等待本地清洗物化。范围预览不会自行写入或触发清洗，因此暂不显示候选。";
          } else if (preparation.available_total === 0 && preparation.excluded_total > 0) {
            message.textContent = "当前原声均被确定性规则排除为无效或异常语料，因此没有进入候选范围。它不代表没有用户讨论。";
          } else {
            message.textContent = `“${planScopeLabel(scope)}”范围当前没有候选。可切换到其他语料状态查看；这不表示系统已经研究或拒绝了这些评论。`;
          }
          planPreviewEmpty.append(message);
          planPreviewEmpty.hidden = false;
        }

        function isPlanPreviewPayload(payload) {
          return payload
            && payload.preview_state === "live_read_only"
            && (payload.scope === "available" || payload.scope === "ready" || payload.scope === "needs_context")
            && Number.isInteger(payload.limit)
            && payload.preparation
            && Array.isArray(payload.source_distribution)
            && Array.isArray(payload.candidates);
        }

        function renderPlanPreview(payload) {
          clearPlanPreviewRows();
          state.planPreviewPayload = payload;
          prepareRunButton.disabled = payload.candidates.length === 0;
          renderPlanPreparation(payload.preparation);
          renderPlanSources(payload.source_distribution);
          renderPlanCandidates(payload.candidates);
          if (payload.candidates.length === 0) {
            renderPlanEmpty(payload.preparation, payload.scope);
          }
        }

        function showRunConfirmation() {
          const preview = state.planPreviewPayload;
          if (!preview || preview.candidates.length === 0) {
            setPlanPreviewStatus("当前范围没有可冻结的输入。可更新预览或切换范围后再试。", "error");
            return;
          }
          const sourceTotal = preview.source_distribution.filter((source) => source.selected_total > 0).length;
          runConfirmationSummary.textContent = `当前预览有 ${preview.candidates.length} 条候选，覆盖 ${sourceTotal} 个来源作品。确认时会重新核对当前语料和来源轮换，预览可能更新；系统将冻结最终范围并创建待执行研究。`;
          runPreparationResult.hidden = true;
          runConfirmation.hidden = false;
          planPreviewScope.disabled = true;
          planPreviewLimit.disabled = true;
          refreshPlanPreviewButton.disabled = true;
          confirmRunButton.focus();
        }

        function closeRunConfirmation() {
          clearRunPreparationState();
          if (!planPreviewDrawer.hidden) prepareRunButton.focus();
        }

        function isRunPreparationPayload(payload) {
          return payload
            && typeof payload.run_ref === "string"
            && (payload.run_state === "prepared_for_execution" || payload.run_state === "blocked_by_context")
            && typeof payload.scope_refreshed === "boolean"
            && Number.isInteger(payload.frozen_item_total)
            && Number.isInteger(payload.prepared_for_execution_total)
            && Number.isInteger(payload.blocked_needs_context_total)
            && Array.isArray(payload.source_distribution);
        }

        function appendRunReceiptLine(root, label, value, className) {
          const item = document.createElement("div");
          const term = document.createElement("dt");
          const definition = document.createElement("dd");
          term.textContent = label;
          definition.className = className || "";
          definition.textContent = value;
          item.append(term, definition);
          root.append(item);
        }

        function renderRunPreparationResult(payload) {
          const sourceTotal = payload.source_distribution.filter((source) => source.frozen_total > 0).length;
          const hasPreparedInput = payload.prepared_for_execution_total > 0;
          const resultTitle = document.createElement("h4");
          resultTitle.textContent = hasPreparedInput
            ? "待执行，尚未开始分析"
            : "已冻结，但因上下文不足暂不执行";
          const resultCopy = document.createElement("p");
          resultCopy.className = "drawer-copy";
          resultCopy.textContent = `已冻结 ${payload.frozen_item_total} 条输入，覆盖 ${sourceTotal} 个来源作品；其中 ${payload.prepared_for_execution_total} 条待执行，${payload.blocked_needs_context_total} 条因上下文不足被阻断。`;
          const refresh = document.createElement("p");
          refresh.className = "run-receipt-note";
          refresh.textContent = payload.scope_refreshed
            ? "创建时范围已刷新，回执按重新核对后的当前语料生成。"
            : "创建时已重新核对当前语料；本次没有提交候选清单，服务端不会按页面逐条创建。";
          const receipt = document.createElement("dl");
          receipt.className = "run-receipt";
          appendRunReceiptLine(receipt, "Run 回执", payload.run_ref, "mono");
          appendRunReceiptLine(receipt, "最终来源覆盖", `${sourceTotal} 个来源作品`);
          appendRunReceiptLine(receipt, "范围刷新", payload.scope_refreshed ? "已刷新" : "已重新核对");
          runPreparationResult.replaceChildren(resultTitle, resultCopy, refresh, receipt);
          runPreparationResult.dataset.state = "success";
          runPreparationResult.hidden = false;
          state.runPreparedForCurrentPreview = true;
          prepareRunButton.disabled = true;
          prepareRunButton.textContent = "本次输入已冻结";
          planPreviewScope.disabled = false;
          planPreviewLimit.disabled = false;
          refreshPlanPreviewButton.disabled = false;
          runPreparationResult.focus();
        }

        function renderRunPreparationError(message) {
          const heading = document.createElement("h4");
          heading.textContent = "未能准备本次研究";
          const copy = document.createElement("p");
          copy.className = "drawer-copy";
          copy.textContent = message;
          const action = document.createElement("button");
          action.className = "quiet-button";
          action.type = "button";
          action.textContent = "回到预览";
          action.addEventListener("click", () => {
            clearRunPreparationState();
            prepareRunButton.focus();
          });
          runPreparationResult.replaceChildren(heading, copy, action);
          runPreparationResult.dataset.state = "error";
          runPreparationResult.hidden = false;
          runPreparationResult.focus();
        }

        async function submitRunPreparation() {
          const workspaceId = state.workspaceId.trim();
          const preview = state.planPreviewPayload;
          if (!workspaceId || !preview || preview.candidates.length === 0) {
            renderRunPreparationError("当前预览没有可冻结输入。请回到预览并重新读取当前范围。");
            return;
          }
          const requestToken = state.runPreparationRequestToken + 1;
          state.runPreparationRequestToken = requestToken;
          setRunPreparationLoading(true);
          try {
            const response = await fetch("/api/v0/comment-research/runs", {
              method: "POST",
              headers: { "Accept": "application/json", "Content-Type": "application/json" },
              credentials: "same-origin",
              body: JSON.stringify({
                workspace_id: workspaceId,
                scope: planPreviewScope.value,
                limit: Number(planPreviewLimit.value)
              })
            });
            const payload = await response.json().catch(() => null);
            if (requestToken !== state.runPreparationRequestToken || planPreviewDrawer.hidden) return;
            if (!response.ok || !isRunPreparationPayload(payload)) {
              throw new Error(errorMessage(payload));
            }
            runConfirmation.hidden = true;
            renderRunPreparationResult(payload);
          } catch (error) {
            if (requestToken !== state.runPreparationRequestToken || planPreviewDrawer.hidden) return;
            runConfirmation.hidden = true;
            renderRunPreparationError(error instanceof Error ? error.message : "无法准备本次研究。请回到预览后重试。");
          } finally {
            if (requestToken === state.runPreparationRequestToken) setRunPreparationLoading(false);
          }
        }

        async function loadPlanPreview() {
          const workspaceId = state.workspaceId.trim();
          if (!workspaceId) {
            setPlanPreviewStatus("请先输入并加载工作空间 ID。当前没有读取自动研究范围。", "error");
            return;
          }
          const requestToken = state.planRequestToken + 1;
          state.planRequestToken = requestToken;
          setPlanPreviewLoading(true);
          setPlanPreviewStatus("正在读取当前范围；这不会创建任务或调用模型…", "loading");
          try {
            const parameters = new URLSearchParams({
              workspace_id: workspaceId,
              scope: planPreviewScope.value,
              limit: planPreviewLimit.value
            });
            const response = await fetch(`/api/v0/comment-research/plan-preview?${parameters.toString()}`, {
              headers: { "Accept": "application/json" },
              credentials: "same-origin"
            });
            const payload = await response.json().catch(() => null);
            if (requestToken !== state.planRequestToken || planPreviewDrawer.hidden) return;
            if (!response.ok || !isPlanPreviewPayload(payload)) {
              throw new Error(errorMessage(payload));
            }
            renderPlanPreview(payload);
            const shown = payload.candidates.length;
            setPlanPreviewStatus(
              shown > 0
                ? `实时范围显示 ${shown} 条候选；离开或再次更新后范围可能变化，当前没有冻结样本。`
                : "已读取实时范围；当前没有符合该范围的候选。",
              ""
            );
          } catch (error) {
            if (requestToken !== state.planRequestToken || planPreviewDrawer.hidden) return;
            clearPlanPreviewRows();
            setPlanPreviewStatus(
              error instanceof Error ? error.message : "无法读取自动研究范围。请稍后重试。",
              "error"
            );
          } finally {
            if (requestToken === state.planRequestToken) setPlanPreviewLoading(false);
          }
        }

        function closePlanPreview() {
          state.planRequestToken += 1;
          state.runPreparationRequestToken += 1;
          planPreviewDrawer.hidden = true;
          if (drawer.hidden && runRecordDrawer.hidden) drawerBackdrop.hidden = true;
          if (state.planLastTrigger instanceof HTMLElement) {
            state.planLastTrigger.focus();
          }
        }

        function openPlanPreview(trigger) {
          const workspaceId = state.workspaceId.trim();
          if (!workspaceId) {
            setStatus("请先输入工作空间 ID 并加载用户原声，再查看自动研究范围。", "error");
            workspaceInput.focus();
            return;
          }
          state.planLastTrigger = trigger;
          if (!drawer.hidden) {
            state.contextRequestToken += 1;
            drawer.hidden = true;
          }
          if (!runRecordDrawer.hidden) {
            state.runRecordDetailRequestToken += 1;
            runRecordDrawer.hidden = true;
          }
          planPreviewScope.value = state.filter;
          drawerBackdrop.hidden = false;
          planPreviewDrawer.hidden = false;
          closePlanPreviewButton.focus();
          loadPlanPreview();
        }

        function isContextPayload(payload) {
          return payload && (payload.availability === "available" || payload.availability === "unavailable");
        }

        async function loadContext(voice, requestToken) {
          const parameters = new URLSearchParams({
            workspace_id: state.workspaceId,
            evidence_id: voice.source_evidence.evidence_id,
            record_index: String(voice.source_evidence.record_index)
          });
          try {
            const response = await fetch(`/api/v0/comment-research/voices/context?${parameters.toString()}`, {
              headers: { "Accept": "application/json" },
              credentials: "same-origin"
            });
            const payload = await response.json().catch(() => null);
            if (requestToken !== state.contextRequestToken || drawer.hidden) return;
            if (!response.ok || !isContextPayload(payload) || typeof payload.original_voice_text !== "string") {
              throw new Error("无法读取已采到的上下文。当前原声与其来源证据仍可查看，请稍后重试。");
            }
            document.getElementById("drawer-original-voice").textContent = payload.original_voice_text;
            if (payload.availability === "unavailable") {
              renderContextUnavailable();
              return;
            }
            renderRelatedDiscussion(payload.related_discussion);
            renderWorkContext(payload.work_context);
          } catch (error) {
            if (requestToken !== state.contextRequestToken || drawer.hidden) return;
            renderContextError(error instanceof Error ? error.message : null);
          }
        }

        function openDrawer(voice, trigger) {
          state.lastTrigger = trigger;
          state.currentVoice = voice;
          clearContextPackPreview();
          if (!planPreviewDrawer.hidden) {
            state.planRequestToken += 1;
            planPreviewDrawer.hidden = true;
          }
          if (!runRecordDrawer.hidden) {
            state.runRecordDetailRequestToken += 1;
            runRecordDrawer.hidden = true;
          }
          document.getElementById("drawer-research-expression").textContent = voice.research_text;
          document.getElementById("drawer-original-voice").textContent = "正在按当前来源证据读取原始采集原声…";
          document.getElementById("drawer-note-id").textContent = voice.source_note_id;
          document.getElementById("drawer-evidence-id").textContent = voice.source_evidence.evidence_id;
          document.getElementById("drawer-record-index").textContent = String(voice.source_evidence.record_index);
          document.getElementById("drawer-admitted-at").textContent = voice.current_admitted_at;
          drawerBackdrop.hidden = false;
          drawer.hidden = false;
          const requestToken = state.contextRequestToken + 1;
          state.contextRequestToken = requestToken;
          renderContextLoading();
          closeDrawerButton.focus();
          loadContext(voice, requestToken);
        }

        function closeDrawer() {
          state.contextRequestToken += 1;
          clearContextPackPreview();
          state.currentVoice = null;
          drawer.hidden = true;
          if (planPreviewDrawer.hidden && runRecordDrawer.hidden) drawerBackdrop.hidden = true;
          if (state.lastTrigger instanceof HTMLElement) {
            state.lastTrigger.focus();
          }
        }

        form.addEventListener("submit", (event) => {
          event.preventDefault();
          state.workspaceId = workspaceInput.value.trim();
          state.filter = voiceFilter.value;
          if (state.activeView === "runs") {
            loadRunRecords(0, loadButton);
          } else {
            loadVoices(0, loadButton);
          }
        });
        previousButton.addEventListener("click", () => loadVoices(Math.max(0, state.offset - pageLimit), previousButton));
        nextButton.addEventListener("click", () => loadVoices(state.offset + pageLimit, nextButton));
        previousRunRecordsButton.addEventListener("click", () => loadRunRecords(Math.max(0, state.runRecordsOffset - pageLimit), previousRunRecordsButton));
        nextRunRecordsButton.addEventListener("click", () => loadRunRecords(state.runRecordsOffset + pageLimit, nextRunRecordsButton));
        voicesTab.addEventListener("click", () => activateView("voices", voicesTab));
        runRecordsTab.addEventListener("click", () => activateView("runs", runRecordsTab));
        planPreviewButton.addEventListener("click", () => openPlanPreview(planPreviewButton));
        contextPackButton.addEventListener("click", loadContextPack);
        planPreviewForm.addEventListener("submit", (event) => {
          event.preventDefault();
          loadPlanPreview();
        });
        prepareRunButton.addEventListener("click", showRunConfirmation);
        cancelRunConfirmationButton.addEventListener("click", closeRunConfirmation);
        confirmRunButton.addEventListener("click", submitRunPreparation);
        closeDrawerButton.addEventListener("click", closeDrawer);
        closePlanPreviewButton.addEventListener("click", closePlanPreview);
        closeRunRecordButton.addEventListener("click", closeRunRecordDrawer);
        drawerBackdrop.addEventListener("click", () => {
          if (!planPreviewDrawer.hidden) {
            closePlanPreview();
          } else if (!runRecordDrawer.hidden) {
            closeRunRecordDrawer();
          } else {
            closeDrawer();
          }
        });
        document.addEventListener("keydown", (event) => {
          if (event.key === "Escape") {
            if (!planPreviewDrawer.hidden) {
              closePlanPreview();
            } else if (!runRecordDrawer.hidden) {
              closeRunRecordDrawer();
            } else if (!drawer.hidden) {
              closeDrawer();
            }
          }
        });
      })();
    </script>
  </body>
</html>
"##;

#[cfg(test)]
mod tests {
    use super::USER_VOICES_PAGE_V0_HTML;

    #[test]
    fn page_starts_without_a_workspace_request_or_synthetic_research_data() {
        assert!(USER_VOICES_PAGE_V0_HTML.contains("初始状态不读取任何数据"));
        assert!(USER_VOICES_PAGE_V0_HTML.contains("const workspaceId = state.workspaceId.trim()"));
        assert!(USER_VOICES_PAGE_V0_HTML.contains("if (!workspaceId)"));
        assert!(!USER_VOICES_PAGE_V0_HTML.contains("DOMContentLoaded"));
        assert!(!USER_VOICES_PAGE_V0_HTML.contains("window.onload"));
        assert!(USER_VOICES_PAGE_V0_HTML.contains("清洗后研究表达"));
        assert!(USER_VOICES_PAGE_V0_HTML.contains("原始采集原声"));
        assert!(USER_VOICES_PAGE_V0_HTML.contains("全部可用"));
        assert!(USER_VOICES_PAGE_V0_HTML.contains("打开详情后按来源读取"));
        assert!(USER_VOICES_PAGE_V0_HTML.contains("已采到的相关讨论"));
        assert!(USER_VOICES_PAGE_V0_HTML.contains("研究输入预览"));
        assert!(USER_VOICES_PAGE_V0_HTML.contains("当前评论的直接证据"));
        assert!(USER_VOICES_PAGE_V0_HTML.contains("本次省略与边界"));
        assert!(USER_VOICES_PAGE_V0_HTML.contains("/api/v0/comment-research/voices/context"));
        assert!(USER_VOICES_PAGE_V0_HTML.contains("/api/v0/comment-research/voices/context-pack"));
        assert!(USER_VOICES_PAGE_V0_HTML.contains("查看自动研究范围"));
        assert!(USER_VOICES_PAGE_V0_HTML.contains("/api/v0/comment-research/plan-preview"));
        assert!(USER_VOICES_PAGE_V0_HTML.contains("确认前不会创建任务、锁定样本、调用模型"));
        assert!(USER_VOICES_PAGE_V0_HTML.contains("范围在每次读取时重新计算，尚未冻结"));
        assert!(USER_VOICES_PAGE_V0_HTML.contains("当前原声与其来源证据仍可查看"));
        assert!(USER_VOICES_PAGE_V0_HTML.contains("等待本地清洗物化"));
        assert!(USER_VOICES_PAGE_V0_HTML.contains("准备本次研究"));
        assert!(USER_VOICES_PAGE_V0_HTML.contains("确认并冻结输入"));
        assert!(USER_VOICES_PAGE_V0_HTML.contains("待执行，尚未开始分析"));
        assert!(USER_VOICES_PAGE_V0_HTML.contains("不会调用模型、不生成结论、不扣费"));
        assert!(USER_VOICES_PAGE_V0_HTML.contains("/api/v0/comment-research/runs"));
        assert!(USER_VOICES_PAGE_V0_HTML.contains("本地研究运行记录"));
        assert!(USER_VOICES_PAGE_V0_HTML.contains("尚无待执行研究"));
        assert!(USER_VOICES_PAGE_V0_HTML.contains("待执行，尚未开始分析"));
        assert!(USER_VOICES_PAGE_V0_HTML.contains("未配置执行器，尚未开始分析"));
        assert!(
            USER_VOICES_PAGE_V0_HTML
                .contains("/api/v0/comment-research/runs/${encodeURIComponent(runRef)}")
        );
        let run_submission = USER_VOICES_PAGE_V0_HTML
            .split("async function submitRunPreparation()")
            .nth(1)
            .expect("page has run submission")
            .split("function closePlanPreview()")
            .next()
            .expect("run submission ends before close");
        assert!(!run_submission.contains("evidence_id"));
        assert!(!run_submission.contains("evidence_ids"));
        assert!(!run_submission.contains("preview:"));
        assert!(!USER_VOICES_PAGE_V0_HTML.contains("data-label=\\\"点赞\\\""));
        assert!(!USER_VOICES_PAGE_V0_HTML.contains("data-label=\\\"作者\\\""));
        assert!(!USER_VOICES_PAGE_V0_HTML.contains("data-label=\\\"发表时间\\\""));
        assert!(!USER_VOICES_PAGE_V0_HTML.contains("data-label=\\\"趋势\\\""));
        assert!(!USER_VOICES_PAGE_V0_HTML.contains(">开始研究<"));
        assert!(!USER_VOICES_PAGE_V0_HTML.contains("确认执行"));
        assert!(!USER_VOICES_PAGE_V0_HTML.contains("type=\"checkbox\""));
    }
}
