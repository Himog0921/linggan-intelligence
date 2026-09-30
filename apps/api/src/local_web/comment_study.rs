//! Read-only HTTP projection for the clean comment-study lifecycle.
//!
//! These routes deliberately use a new namespace while the retired surface is still present in
//! the tree.  The final page replacement can therefore be verified against the clean layer
//! before the old command routes are removed; no route here reads or writes V1 relations.

use super::{LocalDatabaseState, LocalWebState, shell};
use axum::{
    Json, Router,
    extract::{Query, Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use linggan_intelligence::comment_study_read::{
    self as read, CommentStudyReadError, CommentStudyReadQuery,
};
use linggan_intelligence::{
    comment_study_embedding::EmbeddingError,
    comment_study_source::{StudySourceRole, preview_sources_for_roles},
};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

#[path = "comment_study_api.rs"]
mod catalog_api;

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/corpus/comments", get(page))
        .route("/assets/comment-study.css", get(stylesheet))
        .route("/assets/comment-study.js", get(script))
        .route("/api/local/comment-study/overview", get(read_overview))
        .route(
            "/api/local/comment-study/runs",
            get(read_runs).post(catalog_api::start),
        )
        .route("/api/local/comment-study/targets", get(read_targets))
        .route("/api/local/comment-study/signals", get(read_signals))
        .route("/api/local/comment-study/problems", get(read_problems))
        .route("/api/local/comment-study/setup", get(read_setup))
        .route(
            "/api/local/comment-study/policy",
            post(catalog_api::retired),
        )
        .route(
            "/api/local/comment-study/embedding-probe",
            post(run_embedding_probe),
        )
        .merge(catalog_api::command_routes())
        .merge(catalog_api::routes())
        .layer(middleware::from_fn(local_comment_study_guard))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DomainQuery {
    domain: Option<Uuid>,
}

/// The page's browser-owned route state is part of its URL and is read after the HTML loads.
/// Keep the page query strict while accepting the documented navigation fields; API query
/// structs remain limited to their own contracts.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CommentStudyPageQuery {
    domain: Option<Uuid>,
    #[serde(rename = "q")]
    _q: Option<String>,
    #[serde(rename = "workRef")]
    _work_ref: Option<String>,
    #[serde(rename = "view")]
    _view: Option<String>,
    #[serde(rename = "runRef")]
    _run_ref: Option<String>,
    #[serde(rename = "problemRef")]
    _problem_ref: Option<String>,
    #[serde(rename = "commentExternalId")]
    _comment_external_id: Option<String>,
    #[serde(rename = "state")]
    _state: Option<String>,
    #[serde(rename = "cursor")]
    _cursor: Option<String>,
    #[serde(rename = "detail")]
    _detail: Option<String>,
    #[serde(rename = "panel")]
    _panel: Option<String>,
}

async fn read_setup(
    State(state): State<LocalWebState>,
    Query(query): Query<DomainQuery>,
) -> Response {
    let database = match database(&state) {
        Ok(v) => v,
        Err(e) => return e,
    };
    let configs: Result<Vec<Value>, sqlx::Error> = sqlx::query_scalar(
        "SELECT jsonb_build_object( \
           'configRef',config.config_ref,'modelId',model.model_id, \
           'inputTokenLimit',config.input_token_limit,'outputTokenLimit',config.output_token_limit, \
           'enabled',connection.enabled \
         ) \
         FROM linggan_model_config config \
         JOIN linggan_model_entry model USING(model_ref) \
         JOIN linggan_model_connection_version version \
           ON version.version_ref=model.connection_version_ref \
         JOIN linggan_model_connection connection USING(connection_ref) \
         WHERE connection.enabled ORDER BY config.created_at",
    )
    .fetch_all(database.pool())
    .await;
    let Ok(configs) = configs else {
        return error(StatusCode::SERVICE_UNAVAILABLE, "comment_study_unavailable");
    };
    let as_of: Result<String, sqlx::Error> = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(database.pool())
        .await;
    let Ok(as_of) = as_of else {
        return error(StatusCode::SERVICE_UNAVAILABLE, "comment_study_unavailable");
    };
    let Some(domain_ref) = query.domain else {
        return error(StatusCode::BAD_REQUEST, "domain_required");
    };
    let domain_state: Option<String> =
        sqlx::query_scalar("SELECT status FROM observation_domain WHERE domain_ref=$1")
            .bind(domain_ref)
            .fetch_optional(database.pool())
            .await
            .unwrap_or(None);
    let Some(domain_state) = domain_state else {
        return error(StatusCode::NOT_FOUND, "domain_not_found");
    };
    let Ok(mut previews) = preview_sources_for_roles(
        database,
        domain_ref,
        &as_of,
        &[StudySourceRole::Primary, StudySourceRole::Reference],
    )
    .await
    else {
        return error(StatusCode::SERVICE_UNAVAILABLE, "comment_study_unavailable");
    };
    let Some(primary_preview) = previews.remove(&StudySourceRole::Primary) else {
        return error(StatusCode::SERVICE_UNAVAILABLE, "comment_study_unavailable");
    };
    let Some(reference_preview) = previews.remove(&StudySourceRole::Reference) else {
        return error(StatusCode::SERVICE_UNAVAILABLE, "comment_study_unavailable");
    };
    let eligible_works = primary_preview.works.clone();
    let reference_eligible_works = reference_preview.works.clone();
    Json(json!({
        "contract":"comment-study.setup.v2",
        "domainRef":domain_ref,
        "domainStatus":domain_state,
        "modelConfigs":configs,
        "sourcePreview":primary_preview,
        "eligibleWorks":eligible_works,
        "referenceSourcePreview":reference_preview,
        "referenceEligibleWorks":reference_eligible_works
    }))
    .into_response()
}
/// Runs the local qualification probe and, only if it passes, makes its profile the one every
/// comparison happens in. An operator action rather than tick work: it loads the model and states
/// what this machine can do, and nothing entitles the system to assert that on its own.
async fn run_embedding_probe(State(state): State<LocalWebState>) -> Response {
    let database = match database(&state) {
        Ok(v) => v,
        Err(e) => return e,
    };
    match linggan_intelligence::comment_study_embedding::probe_and_register_embedding_profile(
        database,
        &linggan_intelligence::pi_adapter::PiAdapter::configured(),
    )
    .await
    {
        Ok(outcome) => Json(json!(outcome)).into_response(),
        // A runtime failure is distinct from a failure to retain the qualification fact.  Both
        // remain deliberately closed, safe codes rather than exposing an adapter or database
        // error string to the local browser.
        Err(probe_error) => error(
            StatusCode::SERVICE_UNAVAILABLE,
            embedding_probe_error_code(&probe_error),
        ),
    }
}

fn embedding_probe_error_code(error: &EmbeddingError) -> &'static str {
    match error {
        // A runtime failure is distinct from a failure to retain the qualification fact. Both
        // remain deliberately closed, safe codes rather than exposing adapter or database text.
        EmbeddingError::Database(_) => "embedding_probe_storage_unavailable",
        EmbeddingError::Model(_) | EmbeddingError::InvalidVector => "embedding_runtime_unavailable",
    }
}

async fn page(
    State(state): State<LocalWebState>,
    Query(query): Query<CommentStudyPageQuery>,
) -> Html<String> {
    let configured = matches!(state.database, LocalDatabaseState::Ready(_));
    let domains = state.database.database().map(|database| async move {
        linggan_evidence::observation_domain::read_observation_domains(database)
            .await
            .unwrap_or_default()
    });
    let domains = match domains {
        Some(read) => read.await,
        None => Vec::new(),
    };
    let domain_value = query.domain.map(|value| value.to_string());
    let selected = linggan_evidence::observation_domain::resolve_current_domain(
        &domains,
        domain_value.as_deref(),
    );
    let selected_name = selected
        .map(|domain| domain.name.as_str())
        .unwrap_or("选择领域");
    let selected_status = selected
        .map(|domain| domain.status.as_str())
        .unwrap_or("unknown");
    let picker = super::corpus_domain_picker(&domains, selected, "/corpus/comments", None);
    let crumb = if picker.is_empty() {
        "语料 <span class=\"v7-slash\">/</span> <b>评论研究</b>".to_owned()
    } else {
        format!(
            "语料 <span class=\"v7-slash\">/</span> {picker} <span class=\"v7-slash\">/</span> <b>评论研究</b>"
        )
    };
    let header = shell::global_header(
        shell::PrimarySurface::Corpus,
        "本机研究",
        &crumb,
        &format!("{selected_name} · 状态 {selected_status}"),
        None,
    );
    let domain_nav = domain_value.as_deref();
    let nav = shell::corpus_side_nav(
        shell::CorpusPage::Comments,
        domain_nav,
        "只读呈现新评论研究链路<br>自动排程保持关闭",
    );
    Html(
        include_str!("comment_study.html")
            .replace("{{HEADER}}", &header)
            .replace("{{SIDE_NAV}}", &nav)
            .replace(
                "{{DATABASE_STATE}}",
                if configured { "已连接" } else { "未连接" },
            )
            .replace("ADHD", selected_name)
            .replace(
                "<body>",
                &format!(
                    "<body data-domain-name=\"{}\">",
                    super::html_escape(selected_name)
                ),
            ),
    )
}

async fn stylesheet() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        format!(
            "{}\n{}\n{}",
            super::LIDS_TOKENS,
            super::SHELL_CSS,
            include_str!("comment_study.css")
        ),
    )
}

async fn script() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("comment_study.js"),
    )
}

pub(super) async fn local_comment_study_guard(request: Request, next: Next) -> Response {
    if !allowed_origin(request.headers()) {
        return error(StatusCode::FORBIDDEN, "local_comment_study_origin_rejected");
    }
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        header::HeaderValue::from_static("nosniff"),
    );
    response
}

fn allowed_origin(headers: &HeaderMap) -> bool {
    let Some(host) = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };
    let Some((name, port)) = host.rsplit_once(':') else {
        return false;
    };
    if !matches!(name, "127.0.0.1" | "localhost") || port.parse::<u16>().is_err() {
        return false;
    }
    if headers
        .get("sec-fetch-site")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| !matches!(value, "same-origin" | "none"))
    {
        return false;
    }
    headers.get(header::ORIGIN).is_none_or(|origin| {
        origin
            .to_str()
            .is_ok_and(|value| value == format!("http://{host}"))
    })
}

async fn read_overview(
    State(state): State<LocalWebState>,
    Query(query): Query<CommentStudyReadQuery>,
) -> Response {
    let database = match database(&state) {
        Ok(database) => database,
        Err(response) => return response,
    };
    read_response(read::read_overview(database, &query).await)
}

async fn read_runs(
    State(state): State<LocalWebState>,
    Query(query): Query<CommentStudyReadQuery>,
) -> Response {
    let database = match database(&state) {
        Ok(database) => database,
        Err(response) => return response,
    };
    read_response(read::read_runs(database, &query).await)
}

async fn read_targets(
    State(state): State<LocalWebState>,
    Query(query): Query<CommentStudyReadQuery>,
) -> Response {
    let database = match database(&state) {
        Ok(database) => database,
        Err(response) => return response,
    };
    read_response(read::read_targets(database, &query).await)
}

async fn read_signals(
    State(state): State<LocalWebState>,
    Query(query): Query<CommentStudyReadQuery>,
) -> Response {
    let database = match database(&state) {
        Ok(database) => database,
        Err(response) => return response,
    };
    read_response(read::read_signals(database, &query).await)
}

async fn read_problems(
    State(state): State<LocalWebState>,
    Query(query): Query<CommentStudyReadQuery>,
) -> Response {
    let database = match database(&state) {
        Ok(database) => database,
        Err(response) => return response,
    };
    read_response(read::read_problems(database, &query).await)
}

fn database(state: &LocalWebState) -> Result<&linggan_storage_postgres::Database, Response> {
    match &state.database {
        LocalDatabaseState::Ready(database) => Ok(database),
        LocalDatabaseState::NotConfigured
        | LocalDatabaseState::DatabaseUnavailable
        | LocalDatabaseState::SchemaUnavailable(_) => Err(error(
            StatusCode::SERVICE_UNAVAILABLE,
            "comment_study_unavailable",
        )),
    }
}

fn read_response(result: Result<serde_json::Value, CommentStudyReadError>) -> Response {
    match result {
        Ok(value) => Json(value).into_response(),
        Err(CommentStudyReadError::InvalidQuery) => error(StatusCode::BAD_REQUEST, "invalid_query"),
        Err(CommentStudyReadError::InvalidCursor) => {
            error(StatusCode::BAD_REQUEST, "invalid_cursor")
        }
        Err(CommentStudyReadError::CursorScopeMismatch) => {
            error(StatusCode::BAD_REQUEST, "cursor_scope_mismatch")
        }
        Err(CommentStudyReadError::RunUnavailable) => {
            error(StatusCode::NOT_FOUND, "comment_study_run_unavailable")
        }
        Err(CommentStudyReadError::SchemaUnavailable | CommentStudyReadError::Database(_)) => {
            error(StatusCode::SERVICE_UNAVAILABLE, "comment_study_unavailable")
        }
    }
}

fn error(status: StatusCode, code: &str) -> Response {
    (status, Json(json!({"error":code}))).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_read_routes_reject_cross_site_requests() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, "127.0.0.1:3000".parse().unwrap());
        headers.insert(header::ORIGIN, "https://other.example".parse().unwrap());
        assert!(!allowed_origin(&headers));
        headers.insert(header::ORIGIN, "http://127.0.0.1:3000".parse().unwrap());
        assert!(allowed_origin(&headers));
    }

    #[test]
    fn embedding_probe_never_reports_an_invalid_runtime_vector_as_a_storage_failure() {
        assert_eq!(
            embedding_probe_error_code(&EmbeddingError::InvalidVector),
            "embedding_runtime_unavailable"
        );
    }

    #[test]
    fn comment_study_page_uses_the_shared_workspace_and_a_table_for_work_selection() {
        let page = include_str!("comment_study.html");
        assert!(page.contains("<div class=\"v7-app\">"));
        assert!(page.contains("<div class=\"v7-shell\">"));
        assert!(page.contains("class=\"v7-sr-only\""));
        assert!(
            page.contains("<table class=\"study-table\" aria-labelledby=\"work-picker-title\"")
        );
        assert!(page.contains("id=\"work-filter\""));
        assert!(page.contains("id=\"select-visible-works\""));
        assert!(!page.contains("study-hero"));
        assert!(!page.contains("work-option"));
    }

    #[test]
    fn work_selection_keeps_its_canonical_set_when_the_visible_table_is_filtered() {
        let script = include_str!("comment_study.js");
        assert!(script.contains("const selectedWorkRoles = new Map();"));
        assert!(script.contains("const visibleWorks = ()"));
        assert!(script.contains("visibleWorks().forEach(work =>"));
        assert!(script.contains("selectedWorkRoles.has(work.workRef)"));
        assert!(script.contains("workCatalogPath(cursor=null)"));
        assert!(script.contains("workCatalogState.observationRole"));
        assert!(script.contains("comments/history"));
        assert!(script.contains("继续读取研究历史"));
        assert!(script.contains("loadWorksPage(workCatalogState.nextCursor)"));
    }

    #[test]
    fn comment_study_table_keeps_desktop_scroll_local_and_releases_it_on_narrow_screens() {
        let stylesheet = include_str!("comment_study.css");
        assert!(stylesheet.contains(
            ".study-table-wrap{max-block-size:calc(var(--lgi-space-24) * 5);overflow:auto"
        ));
        assert!(stylesheet.contains(".study-table th{position:sticky"));
        assert!(stylesheet.contains(
            ".study-table input[type=\"checkbox\"]{inline-size:var(--lgi-space-6);block-size:var(--lgi-space-6)"
        ));
        assert!(stylesheet.contains(
            "#study-dialog #save-policy:not(:disabled),#study-dialog #start-run:not(:disabled){border:2px solid var(--lgi-ink);background:var(--lgi-signal-ink);box-shadow:var(--lgi-shadow-brutal)"
        ));
        assert!(stylesheet.contains("@media(max-width:900px){.study-main{overflow:visible"));
        assert!(stylesheet.contains(".study-table-wrap{max-block-size:none;overflow:visible"));
        assert!(stylesheet.contains(
            ".study-table th:first-child,.study-table td:first-child{width:var(--lgi-space-10)}"
        ));
    }

    #[test]
    fn comment_study_page_puts_the_five_tabs_immediately_after_the_toolbar_not_below_a_giant_form()
    {
        let page = include_str!("comment_study.html");
        let main_start = page
            .find("<main class=\"study-main\"")
            .expect("main present");
        let main_end = page.find("</main>").expect("main closes");
        let main = &page[main_start..main_end];
        // Mog's complaint: the tabs were pushed to the very bottom of the page, below the policy
        // form and a ~100-row work picker table, so the reviewable content (the whole point of the
        // page) was invisible without scrolling past a giant setup form first. The setup form must
        // now live inside <dialog id="study-dialog">, which is a sibling of <main>, not inside it.
        assert!(
            !main.contains("id=\"policy-form\""),
            "the policy form must no longer live directly inside <main>; it belongs in the dialog"
        );
        assert!(
            !main.contains("id=\"works\""),
            "the 100-row work picker table must no longer live directly inside <main>"
        );
        let toolbar_index = main
            .find("class=\"study-toolbar\"")
            .expect("toolbar present");
        let tabs_index = main
            .find("<nav class=\"study-tabs\" aria-label=\"评论研究视图\">")
            .expect("tabs present");
        assert!(
            toolbar_index < tabs_index,
            "toolbar must come before the tabs"
        );
        assert!(
            tabs_index - toolbar_index < 700,
            "the tabs must sit right after the toolbar with nothing bulky in between; got {} \
             characters of markup separating them",
            tabs_index - toolbar_index
        );
    }

    #[test]
    fn comment_study_dialog_headings_keep_their_lids_token_styling_outside_the_main_shell() {
        let stylesheet = include_str!("comment_study.css");
        // <dialog id="study-dialog"> is a sibling of <main class="study-main">, not a descendant,
        // so the old `.study-main h2{...}` rule silently stopped matching once the setup section's
        // headings moved into the dialog (and were demoted to h3). Without an explicit rule the
        // work-picker heading fell back to the browser's default h3 styling.
        assert!(stylesheet.contains(
            "#study-dialog h3{margin:0;color:var(--lgi-ink);font:var(--lgi-weight-bold) var(--lgi-text-section)/var(--lgi-lh-heading) var(--lgi-font-sans)}"
        ));
    }

    #[test]
    fn comment_study_page_moves_the_research_setup_into_a_button_triggered_dialog() {
        let page = include_str!("comment_study.html");
        assert!(
            page.contains("<dialog id=\"study-dialog\" aria-labelledby=\"study-dialog-title\">")
        );
        assert!(page.contains("id=\"open-study-dialog\""));
        assert!(page.contains("id=\"study-dialog-close\""));
        let dialog_start = page
            .find("<dialog id=\"study-dialog\"")
            .expect("dialog present");
        let dialog = &page[dialog_start..];
        assert!(
            dialog.contains("id=\"policy-form\""),
            "the policy form must live inside the dialog"
        );
        assert!(
            dialog.contains("id=\"works\""),
            "the work picker table must live inside the dialog"
        );
        assert!(dialog.contains("id=\"start-run\""));
    }

    #[test]
    fn method_editor_starts_compact_and_saves_one_immutable_child_version() {
        let page = include_str!("comment_study.html");
        let script = include_str!("comment_study.js");
        assert!(page.contains("id=\"edit-policy\""));
        assert!(page.contains("id=\"policy-form\" class=\"policy-form\" hidden"));
        assert!(page.contains("id=\"save-policy\" type=\"submit\" disabled>保存新版本"));
        assert!(page.contains("id=\"cancel-policy-edit\""));
        assert!(script.contains("async function openPolicyEditor()"));
        assert!(script.contains("get(`policies/${encodeURIComponent(reference)}`)"));
        assert!(script.contains("parentPolicyRef,"));
        assert!(script.contains("保存会生成新的不可变方法版本"));
    }

    #[test]
    fn work_catalog_failure_is_reported_without_marking_setup_unavailable() {
        let script = include_str!("comment_study.js");
        assert!(script.contains("作品目录加载失败：${error.message}"));
        assert!(script.contains("准备信息已加载；作品目录暂时不可用"));
        assert!(script.contains("Promise.allSettled([loadWorksPage(null),loadPolicies()])"));
        assert!(script.contains("retry-work-catalog"));
    }

    #[test]
    fn comment_study_script_wires_the_dialog_open_close_and_auto_switches_to_runs_after_creating_one()
     {
        let script = include_str!("comment_study.js");
        assert!(script.contains("studyDialog.showModal()"));
        assert!(script.contains("studyDialog.close()"));
        // Creating a Run is the whole point of opening the dialog; once it succeeds the user should
        // land back on the tabs (Mog also asked why the runs tab looked like nothing had happened —
        // landing on it after creating a run makes the fresh row immediately visible).
        let start_run_handler = script
            .split("document.querySelector('#start-run').addEventListener")
            .nth(1)
            .expect("start-run click handler is present");
        assert!(start_run_handler.contains("studyDialog.close();"));
        assert!(start_run_handler.contains("activeView = 'runs';"));
    }

    #[test]
    fn comment_study_page_exposes_user_comments_without_restoring_the_old_target_tab() {
        let page = include_str!("comment_study.html");
        assert!(page.contains("<nav class=\"study-tabs\" aria-label=\"评论研究视图\">"));
        for view in ["overview", "comments", "runs", "problems"] {
            assert!(
                page.contains(&format!("data-view=\"{view}\"")),
                "missing tab button for view={view}"
            );
        }
        assert!(!page.contains("data-view=\"results\""));
        assert!(page.contains("data-view=\"overview\" aria-current=\"page\""));
        assert!(page.contains("id=\"study-run-picker\" class=\"study-run-picker\" hidden"));
        assert!(page.contains("id=\"study-run-select\""));
        assert!(page.contains("id=\"study-tab-result\""));
        assert!(!page.contains("study-readouts"));
        assert!(!page.contains("id=\"states\""));
    }

    #[test]
    fn comment_study_results_render_targets_and_signals_for_the_selected_run() {
        let script = include_str!("comment_study.js");
        assert!(script.contains("const RUN_SCOPED_VIEWS = new Set(['runs']);"));
        assert!(script.contains("function renderTargetsPanel(targets)"));
        assert!(script.contains("async function renderSelectedRunPanel()"));
        assert!(script.contains("await renderSelectedRunPanel()"));
        assert!(script.contains("async function loadRunPanelPage(panel, reset = false)"));
        assert!(script.contains("state.nextCursor = response.page?.nextCursor || null"));
        assert!(script.contains("runListNextCursor = response.page?.nextCursor || null"));
        assert!(script.contains("data-run-list-load-more"));
        assert!(script.contains("list(state.items, signalCard"));
        assert!(script.contains("data-run-panel=\"targets\""));
        assert!(script.contains("data-run-panel=\"signals\""));
        assert!(script.contains("target.commentText"));
        assert!(script.contains("sourceStateLabel[target.sourceState]"));
        assert!(script.contains(
            "restricted: '来源已被限制，原文不再显示', unknown: '原文未知（来源未采集到正文）'"
        ));
        assert!(script.contains(
            "const contextStateLabel = { ready: '语境完整', partial: '语境部分（有截断）', missing: '缺少语境' };"
        ));
    }

    #[test]
    fn comment_study_script_selects_the_newly_created_run_instead_of_keeping_the_old_selection() {
        let script = include_str!("comment_study.js");
        let start_run_handler = script
            .split("document.querySelector('#start-run').addEventListener")
            .nth(1)
            .expect("start-run click handler is present");
        assert!(
            start_run_handler
                .find("selectedRunRef = response.runRef;")
                .is_some_and(|selection_index| {
                    start_run_handler
                        .find("await loadProjection();")
                        .is_some_and(|reload_index| selection_index < reload_index)
                }),
            "creating a run must select it before reloading the review tabs, otherwise \
             the selected Run panel keeps showing the previously selected run"
        );
    }

    #[test]
    fn comment_study_results_include_signals_in_every_resolution_state() {
        let script = include_str!("comment_study.js");
        assert!(script.contains("async function renderSelectedRunPanel()"));
        assert!(script.contains("list(state.items, signalCard"));
        assert!(!script.contains("PENDING_RESOLUTION_STATES"));
    }

    #[test]
    fn comment_study_script_explains_incomplete_resolution_states_in_chinese() {
        let script = include_str!("comment_study.js");
        assert!(
            script.contains("retrieval_incomplete: '候选目录未查全，当前不能判定是否为新问题'")
        );
        assert!(script.contains("budget_stopped: '归并预算已到上限，当前未完成判断'"));
    }

    #[test]
    fn comment_study_script_explains_primary_pair_outcomes_in_chinese_without_exposing_model_output()
     {
        let script = include_str!("comment_study.js");
        assert!(script.contains("const pairDecisionLabel = {"));
        assert!(script.contains("关键维度不同，当前不是同一用户问题"));
        assert!(script.contains("模型输出结构不符合约定，未接纳"));
        assert!(script.contains("历史配对未记录候选选择信息"));
        assert!(script.contains("function pairOutcomeSummary(outcome)"));
        assert!(!script.contains("proposedProblem"));
    }

    #[test]
    fn comment_study_script_explains_source_eligibility_and_budget_in_chinese() {
        let script = include_str!("comment_study.js");
        assert!(script.contains("function renderSourcePreview(preview, targetId, roleLabel)"));
        assert!(script.contains("作者身份未知"));
        assert!(script.contains("不作为独立用户计数"));
        assert!(script.contains("作品作者本人"));
        assert!(script.contains("本次最多冻结"));
        assert!(script.contains("服务端作品目录"));
        assert!(script.contains("MAX_SELECTED_WORKS = 100"));
    }

    #[test]
    fn comment_study_script_hides_signal_evidence_once_its_source_is_restricted() {
        let script = include_str!("comment_study.js");
        assert!(
            script.contains("signal.sourceState === 'restricted'"),
            "a Signal's evidence is a literal quote of the original comment (see \
             comment_study_semantic.rs); the result panel must stop quoting it once the \
             backend reports the source as restricted, the same way the targets tab already does"
        );
        assert!(script.contains("本条或父语境已受限，研究衍生文本不再显示。"));
    }

    #[test]
    fn comment_study_script_discards_a_stale_tab_render_instead_of_overwriting_a_newer_one() {
        let script = include_str!("comment_study.js");
        assert!(
            script.contains("let renderToken = 0;"),
            "a slow fetch from an abandoned tab/run selection must not be allowed to overwrite \
             whatever the user switched to in the meantime"
        );
        assert!(script.contains("const token = ++renderToken;"));
        assert!(script.contains("if (token !== renderToken) return false;"));
        for renderer in [
            "async function renderOverviewTab()",
            "async function renderSelectedRunPanel()",
            "async function renderProblemsTab()",
            "async function renderRunsTab()",
        ] {
            assert!(
                script.contains(renderer),
                "{renderer} must return its HTML instead of writing to the DOM itself, so the \
                 generation check in renderActiveTab is the only place allowed to apply it"
            );
        }
    }

    #[test]
    fn comment_study_overview_stays_a_read_only_projection_of_existing_facts() {
        // 页面脚本的排版（换行、空格）不是契约：断言前统一去掉空白，只比 token 序列，
        // 这样把同一段脚本压成一行或重新换行都不会让这道守门失效。
        let compact = |source: &str| -> String {
            source
                .chars()
                .filter(|character| !character.is_whitespace())
                .collect()
        };
        let page = compact(include_str!("comment_study.html"));
        assert!(
            page.contains("TAB_RENDERERS.overview=renderIntelligenceOverviewTab;"),
            "情报总览必须是默认工作面的渲染器，否则首屏仍会落回旧的工程报告"
        );
        assert!(
            page.contains("if(activeView!=='overview'||!overviewState.cache)return;"),
            "总览的本地重渲染只能作用于当前工作面；没有这道判断，一次旧筛选的 innerHTML \
             写入会覆盖用户已经切到的其它 Tab"
        );
        assert!(
            page.contains(
                "constRUN_RECORD_RESOLUTION_STATES=newSet(['protocol_rejected','failed']);"
            ),
            "协议拒绝与失败不在「待归并」页的状态集合里；它们必须指向运行记录，否则\
             「尚未看清」里的「查看」会落到一个必定为空的列表上"
        );
        let loader = page
            .split("asyncfunctionloadIntelligenceOverview()")
            .nth(1)
            .and_then(|rest| {
                rest.split("asyncfunctionrenderIntelligenceOverviewTab()")
                    .next()
            })
            .expect("总览必须有独立的加载函数，组合既有读取端点");
        for read in [
            "get(overviewPath('overview'))",
            "get(overviewPath('problems',{limit:100}))",
            "get(overviewPath('targets',{runRef:overview.latestRun.runRef,limit:100}))",
            "get(overviewPath('signals',{runRef:overview.latestRun.runRef,limit:100}))",
        ] {
            assert!(loader.contains(read), "总览只组合既有读取事实：{read}");
        }
        assert!(
            !loader.contains("post("),
            "总览的加载路径必须只读：浏览、筛选与下钻不得创建 Run、保存策略或调用模型"
        );
    }

    #[test]
    fn comment_study_stylesheet_uses_the_text_tab_primitive_not_a_segmented_control() {
        let stylesheet = include_str!("comment_study.css");
        assert!(stylesheet.contains(
            ".study-tabs button[aria-current=\"page\"]{border-color:var(--lgi-signal);color:var(--lgi-ink);font-weight:var(--lgi-weight-semibold)}"
        ));
        assert!(stylesheet.contains(".study-review-table blockquote{"));
        assert!(stylesheet.contains("var(--lgi-font-evidence)"));
    }
    #[test]
    fn p1_comments_use_catalog_detail_and_server_side_work_pagination() {
        let page = include_str!("comment_study.html");
        let script = include_str!("comment_study.js");
        assert!(page.contains("data-view=\"comments\">用户评论"));
        assert!(page.contains("id=\"comment-detail-dialog\""));
        for token in [
            "catalogQuery('comments',params)",
            "catalogQuery('catalog-summary',summaryParams)",
            "catalogQuery('comments/detail',{workRef,commentExternalId})",
            "catalogQuery('works',{q:query,limit:20})",
            "workCatalogPath(cursor)",
        ] {
            assert!(
                script.contains(token),
                "missing P1 client contract: {token}"
            );
        }
        assert!(!script.contains("setup.eligibleWorks || []"));
        assert!(!page.contains("筛选已加载作品"));
    }
    #[test]
    fn p1_comment_detail_keeps_raw_context_cleaning_and_history_visibly_distinct() {
        let page = include_str!("comment_study.html");
        let script = include_str!("comment_study.js");
        for label in ["原声证据", "所属作品", "父评论语境", "清洗文本", "研究历史"]
        {
            assert!(
                page.contains(label) || script.contains(label),
                "missing detail layer: {label}"
            );
        }
        assert!(script.contains("仅作为语境，不作为当前评论的独立证据"));
        assert!(script.contains("历史未记录"));
    }
}
