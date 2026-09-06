//! Local research composition; no business SQL and no implicit external model access.
use super::{LocalWebState, shell};
use axum::{
    Json, Router,
    extract::{Path, Query, Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use linggan_evidence::comment_research_read::*;
use linggan_intelligence::{
    comment_analysis::*, comment_research::*, comment_research_management::*,
    comment_research_projection::*,
};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/corpus/comments", get(page))
        .route("/corpus/queries", get(queries_page))
        .route("/assets/comment-research.css", get(stylesheet))
        .route("/assets/comment-research.js", get(script))
        .route("/api/local/comment-research", get(search))
        .route(
            "/api/local/comment-research/sources/{source_ref}",
            get(source),
        )
        .route(
            "/api/local/comment-research/queries",
            get(queries).post(save_query),
        )
        .route(
            "/api/local/comment-research/collections",
            get(collections).post(save_collection),
        )
        .route(
            "/api/local/comment-research/assets",
            get(assets).post(save_asset),
        )
        .route(
            "/api/local/comment-research/annotations",
            post(correct_annotation),
        )
        .route(
            "/api/local/comment-research/assets/revisions",
            post(revise_asset),
        )
        .route(
            "/api/local/comment-research/assets/{asset_ref}/history",
            get(asset_history),
        )
        .route(
            "/api/local/comment-research/queries/revisions",
            post(revise_query),
        )
        .route("/api/local/comment-research/groups", get(groups))
        .route(
            "/api/local/comment-research/analysis/{work_ref}/retry",
            post(retry),
        )
        .layer(middleware::from_fn(local_research_guard))
}

pub(super) async fn local_research_guard(request: Request, next: Next) -> Response {
    if !allowed_origin(request.headers()) {
        return error(StatusCode::FORBIDDEN, "local_research_origin_rejected");
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
    let Some(host) = headers.get(header::HOST).and_then(|v| v.to_str().ok()) else {
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
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| !matches!(v, "same-origin" | "none"))
    {
        return false;
    }
    let Some(origin) = headers.get(header::ORIGIN) else {
        return true;
    };
    origin.to_str().is_ok_and(|o| o == format!("http://{host}"))
        && (host.starts_with("127.0.0.1:") || host.starts_with("localhost:"))
}

async fn page() -> Html<String> {
    Html(page_html(false))
}
async fn queries_page() -> Html<String> {
    Html(page_html(true))
}
async fn stylesheet() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        format!(
            "{}\n{}\n{}",
            super::LIDS_TOKENS,
            super::SHELL_CSS,
            include_str!("comment_research.css")
        ),
    )
}
async fn script() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("comment_research.js"),
    )
}

async fn search(
    State(state): State<LocalWebState>,
    Query(query): Query<CommentResearchQuery>,
) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    let mut page = match read_comment_research(db, &query).await {
        Ok(p) => p,
        Err(e) => return read_error(e),
    };
    let refs = page.items.iter().map(|s| s.work_ref).collect::<Vec<_>>();
    let works = match read_comment_work_contexts(db, &refs).await {
        Ok(w) => w,
        Err(e) => return read_error(e),
    };
    for item in &mut page.items {
        item.body_truncated = item
            .body
            .as_ref()
            .is_some_and(|body| body.chars().count() > 350);
        item.body = item
            .body
            .as_ref()
            .map(|body| body.chars().take(350).collect());
    }
    let model = linggan_intelligence::model_settings_read::current_comment_model_state(db)
        .await
        .unwrap_or_else(|_| json!({"modelConnected":false,"modelState":"UNAVAILABLE"}));
    Json(json!({"page":page,"works":works,"modelConnected":model["modelConnected"],"modelState":model["modelState"],"scope":"ACCEPTED_READABLE_COMMENT_SAMPLE"})).into_response()
}

async fn source(State(state): State<LocalWebState>, Path(source_ref): Path<Uuid>) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    let mut context = match read_comment_research_context(db, source_ref).await {
        Ok(c) => c,
        Err(e) => return read_error(e),
    };
    if let Some(body) = context
        .pointer("/source/body")
        .and_then(Value::as_str)
        .map(str::to_owned)
    {
        context["sourceSha256"] = json!(comment_source_hash(&body));
        context["source"]["bodyTruncated"] = json!(body.chars().count() > 4000);
        context["source"]["body"] = json!(body.chars().take(4000).collect::<String>());
    }
    if let Some(body) = context
        .pointer("/parent/body")
        .and_then(Value::as_str)
        .map(str::to_owned)
    {
        context["parent"]["bodyTruncated"] = json!(body.chars().count() > 4000);
        context["parent"]["body"] = json!(body.chars().take(4000).collect::<String>());
    }
    let annotations = match read_comment_research_annotations(db, source_ref).await {
        Ok(a) => a,
        Err(e) => return domain_error(e),
    };
    context["annotations"] = annotations;
    Json(context).into_response()
}

async fn queries(State(state): State<LocalWebState>) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    respond(read_research_queries(db).await)
}
async fn save_query(
    State(state): State<LocalWebState>,
    Json(request): Json<SaveResearchQuery>,
) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    respond(save_research_query(db, &request).await)
}
async fn collections(State(state): State<LocalWebState>) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    respond(read_research_collections(db).await)
}
async fn save_collection(
    State(state): State<LocalWebState>,
    Json(request): Json<SaveResearchCollection>,
) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    respond(save_research_collection(db, &request).await)
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AssetQuery {
    collection_ref: Option<Uuid>,
    cursor: Option<Uuid>,
}
async fn assets(State(state): State<LocalWebState>, Query(query): Query<AssetQuery>) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    respond(read_comment_assets(db, query.collection_ref, query.cursor).await)
}
async fn save_asset(
    State(state): State<LocalWebState>,
    Json(request): Json<SaveCommentAsset>,
) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    respond(save_comment_asset(db, &request).await)
}
async fn revise_asset(
    State(state): State<LocalWebState>,
    Json(request): Json<ReviseCommentAsset>,
) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    respond(revise_comment_asset(db, &request).await)
}
async fn asset_history(
    State(state): State<LocalWebState>,
    Path(asset_ref): Path<Uuid>,
) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    respond(read_comment_asset_history(db, asset_ref).await)
}
async fn revise_query(
    State(state): State<LocalWebState>,
    Json(request): Json<ReviseResearchQuery>,
) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    respond(revise_research_query(db, &request).await)
}
async fn correct_annotation(
    State(state): State<LocalWebState>,
    Json(request): Json<CorrectResearchAnnotation>,
) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    respond(correct_research_annotation(db, &request).await)
}
async fn groups(State(state): State<LocalWebState>) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    let version = linggan_intelligence::model_settings_read::current_model_version(db)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| UNCONFIGURED_MODEL.into());
    respond(read_comment_problem_groups(db, &version).await)
}
async fn retry(State(state): State<LocalWebState>, Path(work_ref): Path<Uuid>) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    respond(
        retry_comment_analysis(db, work_ref)
            .await
            .map(|()| json!({"state":"pending","modelConnected":false})),
    )
}

fn respond(result: Result<Value, CommentResearchError>) -> Response {
    match result {
        Ok(v) => Json(v).into_response(),
        Err(e) => domain_error(e),
    }
}
fn unavailable() -> Response {
    error(
        StatusCode::SERVICE_UNAVAILABLE,
        "comment_research_unavailable",
    )
}
fn error(status: StatusCode, code: &str) -> Response {
    (status, Json(json!({"error":code}))).into_response()
}
fn read_error(e: CommentResearchReadError) -> Response {
    match e {
        CommentResearchReadError::InvalidQuery => error(StatusCode::BAD_REQUEST, "invalid_query"),
        CommentResearchReadError::SourceUnavailable => {
            error(StatusCode::NOT_FOUND, "source_unavailable")
        }
        CommentResearchReadError::Database(_) => unavailable(),
    }
}
fn domain_error(e: CommentResearchError) -> Response {
    match e {
        CommentResearchError::InvalidCommand => error(StatusCode::BAD_REQUEST, "invalid_command"),
        CommentResearchError::IdempotencyConflict => {
            error(StatusCode::CONFLICT, "idempotency_conflict")
        }
        CommentResearchError::RevisionConflict => error(StatusCode::CONFLICT, "revision_conflict"),
        CommentResearchError::SourceUnavailable => {
            error(StatusCode::NOT_FOUND, "source_unavailable")
        }
        CommentResearchError::Database(_) => unavailable(),
    }
}

fn page_html(queries: bool) -> String {
    let title = if queries {
        "已存查询"
    } else {
        "评论研究"
    };
    let header = shell::global_header(
        shell::PrimarySurface::Corpus,
        "本机研究",
        &format!("语料 <span class=\"v7-slash\">/</span> <b>{title}</b>"),
        "<span>内部研究</span><span>样本范围</span>",
        None,
    );
    include_str!("comment_research.html")
        .replace("{{HEADER}}", &header)
        .replace("{{TITLE}}", title)
        .replace("{{VIEW}}", if queries { "queries" } else { "voices" })
        .replace(
            "{{COMMENT_CURRENT}}",
            if queries { "" } else { "aria-current=\"page\"" },
        )
        .replace(
            "{{QUERY_CURRENT}}",
            if queries { "aria-current=\"page\"" } else { "" },
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_mutations_reject_foreign_origin_and_cross_site_fetches() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, "127.0.0.1:3000".parse().unwrap());
        headers.insert(header::ORIGIN, "https://attacker.test".parse().unwrap());
        assert!(!allowed_origin(&headers));
        headers.insert(header::ORIGIN, "http://127.0.0.1:3000".parse().unwrap());
        assert!(allowed_origin(&headers));
        headers.insert("sec-fetch-site", "cross-site".parse().unwrap());
        assert!(!allowed_origin(&headers));
    }
    #[test]
    fn research_is_a_corpus_page_with_three_internal_views() {
        let html = page_html(false);
        for expected in [
            "原声浏览",
            "问题分组",
            "语料资产",
            "/corpus/queries",
            "data-initial-view=\"voices\"",
        ] {
            assert!(html.contains(expected));
        }
        assert!(!html.contains("{{"));
        assert!(!html.contains("选题库"));
    }
}
