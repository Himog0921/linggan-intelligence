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
            <p class="eyebrow">评论研究 · 清洗语料 V1</p>
            <h1>用户原声</h1>
          </div>
          <p class="page-summary">列表呈现确定性清洗后的可研究表达；打开详情可同时核对原始采集原声、来源证据与已采到的讨论语境。研究结论和趋势尚未生成。</p>
        </header>

        <div class="view-tabs" role="tablist" aria-label="评论研究视图">
          <button class="view-tab" type="button" role="tab" aria-selected="false" disabled>概览</button>
          <button class="view-tab" type="button" role="tab" aria-selected="true">用户原声</button>
          <button class="view-tab" type="button" role="tab" aria-selected="false" disabled>用户问题</button>
          <button class="view-tab" type="button" role="tab" aria-selected="false" disabled>变化观察</button>
          <button class="view-tab" type="button" role="tab" aria-selected="false" disabled>运行记录</button>
        </div>
        <p class="nav-note">其余视图尚未具备来源事实，本页不会用空白仪表盘代替。</p>

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

    <script>
      (() => {
        "use strict";

        const pageLimit = 25;
        const state = { workspaceId: "", filter: "available", offset: 0, total: 0, lastTrigger: null, contextRequestToken: 0 };
        const form = document.getElementById("workspace-form");
        const workspaceInput = document.getElementById("workspace-id");
        const voiceFilter = document.getElementById("voice-filter");
        const loadButton = document.getElementById("load-voices");
        const previousButton = document.getElementById("previous-page");
        const nextButton = document.getElementById("next-page");
        const statusMessage = document.getElementById("status-message");
        const initialState = document.getElementById("initial-state");
        const emptyState = document.getElementById("empty-state");
        const results = document.getElementById("results");
        const voicesBody = document.getElementById("voices-body");
        const resultCount = document.getElementById("result-count");
        const pageSummary = document.getElementById("page-summary");
        const drawer = document.getElementById("voice-drawer");
        const drawerBackdrop = document.getElementById("drawer-backdrop");
        const closeDrawerButton = document.getElementById("close-drawer");
        const relatedDiscussion = document.getElementById("drawer-related-discussion");
        const workContext = document.getElementById("drawer-work-context");

        function setStatus(message, kind) {
          statusMessage.textContent = message;
          statusMessage.dataset.state = kind || "";
        }

        function clearRows() {
          voicesBody.replaceChildren();
        }

        function setLoading(loading) {
          loadButton.disabled = loading;
          loadButton.textContent = loading ? "正在加载…" : "加载用户原声";
          previousButton.disabled = loading || state.offset === 0;
          nextButton.disabled = true;
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
          drawer.hidden = true;
          drawerBackdrop.hidden = true;
          if (state.lastTrigger instanceof HTMLElement) {
            state.lastTrigger.focus();
          }
        }

        form.addEventListener("submit", (event) => {
          event.preventDefault();
          state.workspaceId = workspaceInput.value.trim();
          state.filter = voiceFilter.value;
          loadVoices(0, loadButton);
        });
        previousButton.addEventListener("click", () => loadVoices(Math.max(0, state.offset - pageLimit), previousButton));
        nextButton.addEventListener("click", () => loadVoices(state.offset + pageLimit, nextButton));
        closeDrawerButton.addEventListener("click", closeDrawer);
        drawerBackdrop.addEventListener("click", closeDrawer);
        document.addEventListener("keydown", (event) => {
          if (event.key === "Escape" && !drawer.hidden) {
            closeDrawer();
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
        assert!(USER_VOICES_PAGE_V0_HTML.contains("/api/v0/comment-research/voices/context"));
        assert!(USER_VOICES_PAGE_V0_HTML.contains("当前原声与其来源证据仍可查看"));
        assert!(USER_VOICES_PAGE_V0_HTML.contains("等待本地清洗物化"));
        assert!(!USER_VOICES_PAGE_V0_HTML.contains("data-label=\\\"点赞\\\""));
        assert!(!USER_VOICES_PAGE_V0_HTML.contains("data-label=\\\"作者\\\""));
        assert!(!USER_VOICES_PAGE_V0_HTML.contains("data-label=\\\"发表时间\\\""));
        assert!(!USER_VOICES_PAGE_V0_HTML.contains("data-label=\\\"趋势\\\""));
    }
}
