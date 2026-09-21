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
    comment_study_run::{PrepareStudyRunRequest, prepare_study_run},
    comment_study_source::{ADHD_DOMAIN_REF, preview_sources},
};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/corpus/comments", get(page))
        .route("/assets/comment-study.css", get(stylesheet))
        .route("/assets/comment-study.js", get(script))
        .route("/api/local/comment-study/overview", get(read_overview))
        .route("/api/local/comment-study/runs", get(read_runs))
        .route("/api/local/comment-study/targets", get(read_targets))
        .route("/api/local/comment-study/signals", get(read_signals))
        .route("/api/local/comment-study/problems", get(read_problems))
        .route("/api/local/comment-study/setup", get(read_setup))
        .route("/api/local/comment-study/policy", post(save_policy))
        .route("/api/local/comment-study/runs", post(start_run))
        .route(
            "/api/local/comment-study/embedding-probe",
            post(run_embedding_probe),
        )
        .layer(middleware::from_fn(local_comment_study_guard))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SavePolicy {
    model_config_ref: Uuid,
    comment_budget: i32,
    context_character_budget: i32,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StartRun {
    content_public_refs: Vec<Uuid>,
}

async fn read_setup(State(state): State<LocalWebState>) -> Response {
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
    let Ok(preview) = preview_sources(
        database,
        Uuid::parse_str(ADHD_DOMAIN_REF).expect("static UUID"),
        &as_of,
    )
    .await
    else {
        return error(StatusCode::SERVICE_UNAVAILABLE, "comment_study_unavailable");
    };
    let eligible_works = preview.works.clone();
    Json(json!({
        "contract":"comment-study.setup.v2",
        "domainRef":ADHD_DOMAIN_REF,
        "modelConfigs":configs,
        "sourcePreview":preview,
        "eligibleWorks":eligible_works
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

async fn save_policy(
    State(state): State<LocalWebState>,
    Json(request): Json<SavePolicy>,
) -> Response {
    if !(1..=3000).contains(&request.comment_budget)
        || (1..=20000).contains(&request.context_character_budget) == false
    {
        return error(StatusCode::BAD_REQUEST, "invalid_comment_study_policy");
    };
    let database = match database(&state) {
        Ok(v) => v,
        Err(e) => return e,
    };
    let result:Result<Value,sqlx::Error>=async{let mut tx=database.pool().begin().await?;let valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_model_config config JOIN linggan_model_entry model USING(model_ref) JOIN linggan_model_connection_version version ON version.version_ref=model.connection_version_ref JOIN linggan_model_connection connection USING(connection_ref) WHERE config.config_ref=$1 AND connection.enabled)").bind(request.model_config_ref).fetch_one(&mut *tx).await?;if !valid{return Ok(json!({"error":"model_config_unavailable"}))};let policy=Uuid::new_v4();sqlx::query("INSERT INTO linggan_comment_study_policy(policy_ref,domain_ref,model_config_ref,contract,comment_budget,context_character_budget) VALUES($1,$2,$3,'comment-study.v1',$4,$5)").bind(policy).bind(Uuid::parse_str(ADHD_DOMAIN_REF).expect("static uuid")).bind(request.model_config_ref).bind(request.comment_budget).bind(request.context_character_budget).execute(&mut *tx).await?;sqlx::query("INSERT INTO linggan_comment_study_active_policy(singleton,policy_ref) VALUES(true,$1) ON CONFLICT(singleton) DO UPDATE SET policy_ref=EXCLUDED.policy_ref,updated_at=scope_001_now()").bind(policy).execute(&mut *tx).await?;tx.commit().await?;Ok(json!({"policyRef":policy,"saved":true}))}.await;
    match result {
        Ok(value) if value.get("error").is_none() => Json(value).into_response(),
        Ok(_) => error(StatusCode::CONFLICT, "model_config_unavailable"),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE, "comment_study_unavailable"),
    }
}
async fn start_run(State(state): State<LocalWebState>, Json(request): Json<StartRun>) -> Response {
    let database = match database(&state) {
        Ok(v) => v,
        Err(e) => return e,
    };
    match prepare_study_run(
        database,
        PrepareStudyRunRequest {
            content_public_refs: request.content_public_refs,
        },
    )
    .await
    {
        Ok(value) => Json(value).into_response(),
        Err(_) => error(StatusCode::CONFLICT, "comment_study_run_unavailable"),
    }
}

async fn page(State(state): State<LocalWebState>) -> Html<String> {
    let configured = matches!(state.database, LocalDatabaseState::Ready(_));
    let header = shell::global_header(
        shell::PrimarySurface::Corpus,
        "本机研究",
        "语料 <span class=\"v7-slash\">/</span> <b>评论研究</b>",
        "当前研究与证据",
        None,
    );
    let nav = shell::corpus_side_nav(
        shell::CorpusPage::Comments,
        None,
        "只读呈现新评论研究链路<br>自动排程保持关闭",
    );
    Html(
        include_str!("comment_study.html")
            .replace("{{HEADER}}", &header)
            .replace("{{SIDE_NAV}}", &nav)
            .replace(
                "{{DATABASE_STATE}}",
                if configured { "已连接" } else { "未连接" },
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
        | LocalDatabaseState::SchemaUnavailable => Err(error(
            StatusCode::SERVICE_UNAVAILABLE,
            "comment_study_unavailable",
        )),
    }
}

fn read_response(result: Result<serde_json::Value, CommentStudyReadError>) -> Response {
    match result {
        Ok(value) => Json(value).into_response(),
        Err(CommentStudyReadError::InvalidQuery) => error(StatusCode::BAD_REQUEST, "invalid_query"),
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
        assert!(script.contains("const selectedWorkRefs = new Set();"));
        assert!(script.contains("const visibleWorks = ()"));
        assert!(script.contains("visibleWorks().forEach(work =>"));
        assert!(script.contains("selectedWorkRefs.has(work.workRef)"));
        assert!(script.contains(
            "document.querySelector('#work-filter').addEventListener('input', renderWorks)"
        ));
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
    fn comment_study_page_offers_the_five_approved_review_tabs_with_no_leftover_mini_readout() {
        let page = include_str!("comment_study.html");
        assert!(page.contains("<nav class=\"study-tabs\" aria-label=\"评论研究视图\">"));
        for view in ["overview", "targets", "pending", "problems", "runs"] {
            assert!(
                page.contains(&format!("data-view=\"{view}\"")),
                "missing tab button for view={view}"
            );
        }
        assert!(page.contains("data-view=\"overview\" aria-current=\"page\""));
        assert!(page.contains("id=\"study-run-picker\" class=\"study-run-picker\" hidden"));
        assert!(page.contains("id=\"study-run-select\""));
        assert!(page.contains("id=\"study-tab-result\""));
        assert!(!page.contains("study-readouts"));
        assert!(!page.contains("id=\"states\""));
    }

    #[test]
    fn comment_study_script_renders_a_run_scoped_targets_tab_with_original_comment_text() {
        let script = include_str!("comment_study.js");
        assert!(script.contains("const RUN_SCOPED_VIEWS = new Set(['targets', 'pending']);"));
        assert!(script.contains("async function renderTargetsTab()"));
        assert!(
            script.contains("`targets?runRef=${encodeURIComponent(selectedRunRef)}&limit=100`")
        );
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
             the targets/pending tabs keep showing the previously selected run"
        );
    }

    #[test]
    fn comment_study_script_filters_pending_signals_by_resolution_state_not_by_kind() {
        let script = include_str!("comment_study.js");
        assert!(script.contains(
            "const PENDING_RESOLUTION_STATES = new Set(['pending', 'deferred_context', 'deferred_ambiguous', 'deferred_novel', 'retrieval_incomplete', 'budget_stopped']);"
        ));
        assert!(script.contains(
            "signal.resolutionState == null || PENDING_RESOLUTION_STATES.has(signal.resolutionState)"
        ));
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
    fn comment_study_script_explains_source_eligibility_and_budget_in_chinese() {
        let script = include_str!("comment_study.js");
        assert!(script.contains("function renderSourcePreview(preview)"));
        assert!(script.contains("评论作者身份未知"));
        assert!(script.contains("作品作者本人"));
        assert!(script.contains("本次最多冻结"));
        assert!(script.contains("此列表最多展示 100 篇"));
    }

    #[test]
    fn comment_study_script_hides_signal_evidence_once_its_source_is_restricted() {
        let script = include_str!("comment_study.js");
        assert!(
            script.contains("signal.sourceState === 'restricted'"),
            "a Signal's evidence is a literal quote of the original comment (see \
             comment_study_semantic.rs); the pending-merge tab must stop quoting it once the \
             backend reports the source as restricted, the same way the targets tab already does"
        );
        assert!(script.contains("来源已被限制，原声与摘要不再显示"));
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
        assert!(script.contains("if (token !== renderToken) return;"));
        for renderer in [
            "async function renderOverviewTab()",
            "async function renderTargetsTab()",
            "async function renderPendingTab()",
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
    fn comment_study_stylesheet_uses_the_text_tab_primitive_not_a_segmented_control() {
        let stylesheet = include_str!("comment_study.css");
        assert!(stylesheet.contains(
            ".study-tabs button[aria-current=\"page\"]{border-color:var(--lgi-signal);color:var(--lgi-ink);font-weight:var(--lgi-weight-semibold)}"
        ));
        assert!(stylesheet.contains(".study-review-table blockquote{"));
        assert!(stylesheet.contains("var(--lgi-font-evidence)"));
    }
}
