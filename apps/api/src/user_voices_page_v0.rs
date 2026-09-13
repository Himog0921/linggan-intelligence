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
        <p class="nav-note">当前只交付可回溯的评论事实。研究结论并未在此版本中生成。</p>
      </aside>

      <main class="workspace" id="voices-main" tabindex="-1">
        <header class="page-heading">
          <div>
            <p class="eyebrow">评论研究 · V0</p>
            <h1>用户原声</h1>
          </div>
          <p class="page-summary">从当前已接入的评论事实开始，逐条回到来源证据。研究结论、作品上下文和趋势尚未在此版本中生成。</p>
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
            <p class="field-help" id="workspace-help">初始状态不读取任何数据。系统只在你确认工作空间后，以只读方式加载当前已接入的评论事实。</p>
          </div>
          <button class="primary-button" id="load-voices" type="submit">加载用户原声</button>
        </form>

        <p class="status-message" id="status-message" role="status" aria-live="polite"></p>

        <section class="empty-state" id="initial-state" aria-labelledby="initial-state-title">
          <h2 id="initial-state-title">先确认要看的语料范围</h2>
          <p>输入工作空间 ID 后，页面会读取其中已接入、可回溯至来源证据的当前评论原声。这里不会假定作品标题、作者、点赞、发表时间、回复关系或研究结论。</p>
        </section>

        <section class="empty-state" id="empty-state" aria-labelledby="empty-state-title" hidden>
          <h2 id="empty-state-title">当前范围没有可显示的用户原声</h2>
          <p>这表示该工作空间尚未接入评论来源，或当前还没有已确认的评论事实；它不表示用户没有讨论，也不表示系统已经完成研究。</p>
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
                  <th scope="col">评论原声</th>
                  <th scope="col">来源作品 ID</th>
                  <th scope="col">系统接入时间</th>
                  <th scope="col">研究状态</th>
                  <th scope="col">作品上下文</th>
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
      <section class="drawer-section" aria-labelledby="voice-text-title">
        <h3 id="voice-text-title">当前原声</h3>
        <p class="drawer-copy" id="drawer-voice-text"></p>
      </section>
      <section class="drawer-section" aria-labelledby="source-evidence-title">
        <h3 id="source-evidence-title">来源证据关系</h3>
        <dl class="drawer-list">
          <div><dt>来源作品 ID</dt><dd class="mono" id="drawer-note-id"></dd></div>
          <div><dt>Evidence ID</dt><dd class="mono" id="drawer-evidence-id"></dd></div>
          <div><dt>来源记录序号</dt><dd class="mono" id="drawer-record-index"></dd></div>
          <div><dt>系统接入时间</dt><dd class="mono" id="drawer-admitted-at"></dd></div>
        </dl>
      </section>
      <section class="drawer-section" aria-labelledby="context-title">
        <h3 id="context-title">作品上下文</h3>
        <p class="context-notice">作品上下文尚未取得。本页面不以推断出的标题、作者、媒体或正文片段补齐这个缺口。</p>
      </section>
    </aside>

    <script>
      (() => {
        "use strict";

        const pageLimit = 25;
        const state = { workspaceId: "", offset: 0, total: 0, lastTrigger: null };
        const form = document.getElementById("workspace-form");
        const workspaceInput = document.getElementById("workspace-id");
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
            textCell.dataset.label = "评论原声";
            const text = document.createElement("span");
            text.className = "voice-text";
            text.textContent = voice.text;
            const detail = document.createElement("button");
            detail.className = "detail-button";
            detail.type = "button";
            detail.textContent = "查看原声与来源";
            detail.addEventListener("click", () => openDrawer(voice, detail));
            textCell.append(text, detail);
            row.append(textCell);
            appendCell(row, "来源作品 ID", voice.source_note_id, "mono metadata");
            appendCell(row, "系统接入时间", voice.current_admitted_at, "mono metadata");
            appendFactCell(row, "研究状态", "尚未研究", false);
            appendFactCell(row, "作品上下文", "作品上下文未取得", true);
            voicesBody.append(row);
          }
        }

        function updatePagination(pagination, voicesCount) {
          state.total = pagination.total;
          state.offset = pagination.offset;
          const start = voicesCount === 0 ? 0 : pagination.offset + 1;
          const end = pagination.offset + voicesCount;
          resultCount.textContent = `已接入 ${pagination.total} 条 · 当前显示 ${start}-${end}`;
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
            const parameters = new URLSearchParams({ workspace_id: workspaceId, limit: String(pageLimit), offset: String(offset) });
            const response = await fetch(`/api/v0/comment-research/voices?${parameters.toString()}`, {
              headers: { "Accept": "application/json" },
              credentials: "same-origin"
            });
            const payload = await response.json().catch(() => null);
            if (!response.ok || !payload || !payload.pagination || !Array.isArray(payload.voices)) {
              throw new Error(errorMessage(payload));
            }

            renderVoices(payload.voices);
            updatePagination(payload.pagination, payload.voices.length);
            if (payload.voices.length === 0) {
              setVisibleState("empty");
              setStatus("已读取当前范围；未发现可显示的用户原声。", "");
            } else {
              setVisibleState("results");
              setStatus("当前列表来自已接入的评论事实；打开任意一条可查看其来源证据关系。", "");
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

        function openDrawer(voice, trigger) {
          state.lastTrigger = trigger;
          document.getElementById("drawer-voice-text").textContent = voice.text;
          document.getElementById("drawer-note-id").textContent = voice.source_note_id;
          document.getElementById("drawer-evidence-id").textContent = voice.source_evidence.evidence_id;
          document.getElementById("drawer-record-index").textContent = String(voice.source_evidence.record_index);
          document.getElementById("drawer-admitted-at").textContent = voice.current_admitted_at;
          drawerBackdrop.hidden = false;
          drawer.hidden = false;
          closeDrawerButton.focus();
        }

        function closeDrawer() {
          drawer.hidden = true;
          drawerBackdrop.hidden = true;
          if (state.lastTrigger instanceof HTMLElement) {
            state.lastTrigger.focus();
          }
        }

        form.addEventListener("submit", (event) => {
          event.preventDefault();
          state.workspaceId = workspaceInput.value.trim();
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
        assert!(USER_VOICES_PAGE_V0_HTML.contains("尚未研究"));
        assert!(USER_VOICES_PAGE_V0_HTML.contains("作品上下文未取得"));
        assert!(USER_VOICES_PAGE_V0_HTML.contains("尚未具备来源事实"));
        assert!(!USER_VOICES_PAGE_V0_HTML.contains("data-label=\\\"点赞\\\""));
        assert!(!USER_VOICES_PAGE_V0_HTML.contains("data-label=\\\"作者\\\""));
        assert!(!USER_VOICES_PAGE_V0_HTML.contains("data-label=\\\"发表时间\\\""));
        assert!(!USER_VOICES_PAGE_V0_HTML.contains("data-label=\\\"趋势\\\""));
    }
}
