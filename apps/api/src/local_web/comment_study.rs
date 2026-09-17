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
    comment_study_run::{PrepareStudyRunRequest, prepare_study_run},
    comment_study_source::ADHD_DOMAIN_REF,
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
    let result:Result<Value,sqlx::Error>=async { let configs:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('configRef',config.config_ref,'modelId',model.model_id,'inputTokenLimit',config.input_token_limit,'outputTokenLimit',config.output_token_limit,'enabled',connection.enabled) FROM linggan_model_config config JOIN linggan_model_entry model USING(model_ref) JOIN linggan_model_connection_version version ON version.version_ref=model.connection_version_ref JOIN linggan_model_connection connection USING(connection_ref) WHERE connection.enabled ORDER BY config.created_at").fetch_all(database.pool()).await?;let works:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('workRef',comment.content_public_ref,'eligibleCommentCount',count(*),'title',COALESCE((SELECT detail.title FROM linggan_material_content_detail detail JOIN linggan_runtime_capture_package package USING(package_ref) WHERE detail.content_public_ref=comment.content_public_ref AND detail.title_state='KNOWN' ORDER BY detail.observed_at DESC LIMIT 1),'未命名作品')) FROM linggan_material_comment comment JOIN linggan_material_content content ON content.public_ref=comment.content_public_ref WHERE content.domain_ref=$1 AND comment.body_state='KNOWN' GROUP BY comment.content_public_ref ORDER BY count(*) DESC,comment.content_public_ref LIMIT 100").bind(Uuid::parse_str(ADHD_DOMAIN_REF).expect("static uuid")).fetch_all(database.pool()).await?;Ok(json!({"contract":"comment-study.setup.v1","domainRef":ADHD_DOMAIN_REF,"modelConfigs":configs,"eligibleWorks":works}))}.await;
    match result {
        Ok(value) => Json(value).into_response(),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE, "comment_study_unavailable"),
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
            "#save-policy:not(:disabled),.study-main #start-run:not(:disabled){border:2px solid var(--lgi-ink);background:var(--lgi-signal-ink);box-shadow:var(--lgi-shadow-brutal)"
        ));
        assert!(stylesheet.contains("@media(max-width:900px){.study-main{overflow:visible"));
        assert!(stylesheet.contains(".study-table-wrap{max-block-size:none;overflow:visible"));
        assert!(stylesheet.contains(
            ".study-table th:first-child,.study-table td:first-child{width:var(--lgi-space-10)}"
        ));
    }
}
