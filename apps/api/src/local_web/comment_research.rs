//! Local composition for the single COMMENT-RESEARCH-RESET-001 user surface.
//!
//! It exposes only V1 Run commands and independently frozen read models.  The retired comment
//! search, saved-query, asset, daily, Task-B and comment-intelligence endpoints deliberately do
//! not have routes here.
use super::{LocalWebState, shell};
use axum::{
    Json, Router,
    extract::{Query, Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use linggan_evidence::observation_domain::ObservationDomain;
use linggan_intelligence::{
    comment_research_kernel::{self as kernel, SaveResearchPolicy},
    comment_research_read_v1::{
        self as read_v1, CommentResearchV1ReadError, CommentResearchV1ReadQuery,
    },
    embedding_settings,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/corpus/comments", get(page))
        .route("/assets/comment-research.css", get(stylesheet))
        .route("/assets/comment-research.js", get(script))
        .route("/api/local/comment-research/setup", get(read_setup))
        .route(
            "/api/local/comment-research/policy",
            post(save_research_kernel_policy),
        )
        .route(
            "/api/local/comment-research/runs",
            post(start_research_kernel_run),
        )
        .route(
            "/api/local/comment-research/runs/preview",
            get(preview_research_kernel_run),
        )
        .route(
            "/api/local/comment-research/overview",
            get(read_v1_overview),
        )
        .route("/api/local/comment-research/voices", get(read_v1_voices))
        .route(
            "/api/local/comment-research/problems",
            get(read_v1_problems),
        )
        .route("/api/local/comment-research/changes", get(read_v1_changes))
        .route("/api/local/comment-research/runs", get(read_v1_runs))
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

#[derive(Deserialize)]
struct CorpusDomainQuery {
    domain: Option<String>,
}

async fn page(
    State(state): State<LocalWebState>,
    Query(params): Query<CorpusDomainQuery>,
) -> Html<String> {
    Html(corpus_page_html(&state, params.domain.as_deref()).await)
}

async fn corpus_page_html(state: &LocalWebState, domain: Option<&str>) -> String {
    let domains = match state.database.database() {
        Some(database) => linggan_evidence::observation_domain::read_observation_domains(database)
            .await
            .unwrap_or_default(),
        None => Vec::new(),
    };
    let current = linggan_evidence::observation_domain::resolve_current_domain(&domains, domain);
    page_html(&domains, current)
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StartResearchKernelRun {}

fn kernel_response<T: Serialize>(
    result: Result<T, kernel::CommentResearchKernelError>,
) -> Response {
    match result {
        Ok(value) => Json(value).into_response(),
        Err(error_value) => {
            let (status, code) = match error_value {
                kernel::CommentResearchKernelError::InvalidPolicy => {
                    (StatusCode::BAD_REQUEST, "invalid_research_request")
                }
                kernel::CommentResearchKernelError::PolicyMissing => {
                    (StatusCode::CONFLICT, "research_policy_missing")
                }
                kernel::CommentResearchKernelError::PolicyInputContractStale => {
                    (StatusCode::CONFLICT, "research_policy_input_contract_stale")
                }
                kernel::CommentResearchKernelError::NoEligibleDerivations => {
                    (StatusCode::CONFLICT, "no_eligible_research_comments")
                }
                kernel::CommentResearchKernelError::EmbeddingNotReady => {
                    (StatusCode::CONFLICT, "embedding_not_ready")
                }
                kernel::CommentResearchKernelError::ModelNotReady => {
                    (StatusCode::CONFLICT, "research_model_not_ready")
                }
                kernel::CommentResearchKernelError::DevelopmentResetBlocked { .. } => {
                    (StatusCode::CONFLICT, "comment_research_reset_blocked")
                }
                kernel::CommentResearchKernelError::DerivationPrewarmIncomplete
                | kernel::CommentResearchKernelError::Database(_)
                | kernel::CommentResearchKernelError::Serialization => (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "comment_research_unavailable",
                ),
            };
            error(status, code)
        }
    }
}

async fn save_research_kernel_policy(
    State(state): State<LocalWebState>,
    Json(request): Json<SaveResearchPolicy>,
) -> Response {
    let database = match v1_database(&state).await {
        Ok(database) => database,
        Err(response) => return response,
    };
    kernel_response(kernel::save_active_policy(database, request).await)
}

async fn start_research_kernel_run(
    State(state): State<LocalWebState>,
    Json(request): Json<StartResearchKernelRun>,
) -> Response {
    let database = match v1_database(&state).await {
        Ok(database) => database,
        Err(response) => return response,
    };
    let _ = request;
    kernel_response(kernel::start_run(database).await)
}

/// This endpoint only explains the server's current automatic boundary. It accepts no request
/// body and returns no source IDs, so a browser cannot select comments for a later Run. The
/// confirm action remains the existing, gated `POST /runs` command.
async fn preview_research_kernel_run(State(state): State<LocalWebState>) -> Response {
    let database = match v1_database(&state).await {
        Ok(database) => database,
        Err(response) => return response,
    };
    kernel_response(kernel::preview_run(database).await)
}

async fn read_setup(State(state): State<LocalWebState>) -> Response {
    let database = match v1_database(&state).await {
        Ok(database) => database,
        Err(response) => return response,
    };
    let policy: Option<Value> = match sqlx::query_scalar(
        "SELECT jsonb_build_object( \
             'revision',active.revision, \
             'policyRevisionRef',policy.policy_revision_ref, \
             'configRef',policy.config_ref,'sourceLimit',policy.source_limit,'tokenLimit',policy.token_limit \
         ) FROM linggan_comment_research_policy_active active \
         JOIN linggan_comment_research_policy_revision policy \
           ON policy.policy_revision_ref=active.policy_revision_ref \
         WHERE active.singleton",
    )
    .fetch_optional(database.pool())
    .await
    {
        Ok(policy) => policy,
        Err(_) => return unavailable(),
    };
    let config: Option<Value> = match sqlx::query_scalar(
        "SELECT jsonb_build_object( \
             'configRef',config.config_ref,'modelId',model.model_id, \
             'inputTokenLimit',config.input_token_limit,'outputTokenLimit',config.output_token_limit, \
             'timeoutSeconds',config.timeout_seconds,'connectionEnabled',connection.enabled, \
             'semanticReady',COALESCE(( \
                 SELECT invocation.state='succeeded' \
                    AND invocation.result->>'ok'='true' \
                    AND invocation.result->>'modelCallable'='true' \
                    AND invocation.result->>'semanticQualified'='true' \
                 FROM linggan_model_invocation invocation \
                 WHERE invocation.model_ref=model.model_ref \
                   AND invocation.connection_version_ref=version.version_ref \
                   AND invocation.operation='probe' \
                 ORDER BY invocation.created_at DESC,invocation.invocation_ref DESC LIMIT 1 \
             ),false) \
         ) FROM linggan_model_workspace workspace \
         JOIN linggan_model_config config ON config.config_ref=workspace.default_config_ref \
         JOIN linggan_model_entry model ON model.model_ref=config.model_ref \
         JOIN linggan_model_connection_version version ON version.version_ref=model.connection_version_ref \
         JOIN linggan_model_connection connection USING(connection_ref) \
         WHERE workspace.singleton",
    )
    .fetch_optional(database.pool())
    .await
    {
        Ok(config) => config,
        Err(_) => return unavailable(),
    };
    // This existing setup projection must read the same singleton used by Run admission.
    // The retired generic provider config is intentionally not a fallback for LOCAL-EMBEDDING-001.
    let embedding = match embedding_settings::read(&database).await {
        Ok(embedding) => embedding,
        Err(_) => return unavailable(),
    };
    let worker: Option<Value> = sqlx::query_scalar(
        "SELECT jsonb_build_object( \
             'lastSeenAt',worker_last_seen_at,'state',worker_state,'lastError',worker_last_error \
         ) FROM linggan_model_workspace WHERE singleton",
    )
    .fetch_optional(database.pool())
    .await
    .unwrap_or(None);
    Json(json!({
        "policy":policy,
        "defaultConfig":config,
        "embedding":embedding,
        "worker":worker,
        "continuousScheduleEnabled":false,
    }))
    .into_response()
}

async fn read_v1_overview(
    State(state): State<LocalWebState>,
    Query(query): Query<CommentResearchV1ReadQuery>,
) -> Response {
    let database = match v1_database(&state).await {
        Ok(database) => database,
        Err(response) => return response,
    };
    v1_read_response(read_v1::read_overview(database, &query).await)
}

async fn read_v1_voices(
    State(state): State<LocalWebState>,
    Query(query): Query<CommentResearchV1ReadQuery>,
) -> Response {
    let database = match v1_database(&state).await {
        Ok(database) => database,
        Err(response) => return response,
    };
    v1_read_response(read_v1::read_voices(database, &query).await)
}

async fn read_v1_problems(
    State(state): State<LocalWebState>,
    Query(query): Query<CommentResearchV1ReadQuery>,
) -> Response {
    let database = match v1_database(&state).await {
        Ok(database) => database,
        Err(response) => return response,
    };
    v1_read_response(read_v1::read_problems(database, &query).await)
}

async fn read_v1_changes(
    State(state): State<LocalWebState>,
    Query(query): Query<CommentResearchV1ReadQuery>,
) -> Response {
    let database = match v1_database(&state).await {
        Ok(database) => database,
        Err(response) => return response,
    };
    v1_read_response(read_v1::read_changes(database, &query).await)
}

async fn read_v1_runs(
    State(state): State<LocalWebState>,
    Query(query): Query<CommentResearchV1ReadQuery>,
) -> Response {
    let database = match v1_database(&state).await {
        Ok(database) => database,
        Err(response) => return response,
    };
    v1_read_response(read_v1::read_runs(database, &query).await)
}

async fn v1_database(
    state: &LocalWebState,
) -> Result<&linggan_storage_postgres::Database, Response> {
    let Some(database) = state.database.database() else {
        return Err(unavailable());
    };
    match read_v1::schema_ready(database).await {
        Ok(true) => Ok(database),
        Ok(false) | Err(_) => Err(error(
            StatusCode::SERVICE_UNAVAILABLE,
            "comment_research_v1_schema_missing",
        )),
    }
}

fn v1_read_response(result: Result<Value, CommentResearchV1ReadError>) -> Response {
    match result {
        Ok(value) => Json(value).into_response(),
        Err(error_value) => v1_read_error(error_value),
    }
}

fn v1_read_error(error_value: CommentResearchV1ReadError) -> Response {
    match error_value {
        CommentResearchV1ReadError::InvalidQuery => error(StatusCode::BAD_REQUEST, "invalid_query"),
        CommentResearchV1ReadError::ResultUnavailable => {
            error(StatusCode::NOT_FOUND, "comment_research_result_unavailable")
        }
        CommentResearchV1ReadError::SchemaUnavailable | CommentResearchV1ReadError::Database(_) => {
            error(
                StatusCode::SERVICE_UNAVAILABLE,
                "comment_research_v1_unavailable",
            )
        }
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

fn page_html(domains: &[ObservationDomain], current: Option<&ObservationDomain>) -> String {
    let picker = super::corpus_domain_picker(domains, current, "/corpus/comments", None);
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
        "<span>当前研究与证据</span>",
        None,
    );
    let side_nav = shell::corpus_side_nav(
        shell::CorpusPage::Comments,
        super::corpus_nav_domain(domains, current).as_deref(),
        "评论证据进入研究<br>自动排程保持关闭",
    );
    include_str!("comment_research.html")
        .replace("{{HEADER}}", &header)
        .replace("{{SIDE_NAV}}", &side_nav)
        .replace(
            "{{DOMAIN_QS}}",
            &super::corpus_nav_domain(domains, current)
                .map(|domain| format!("domain={domain}&amp;"))
                .unwrap_or_default(),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use linggan_intelligence::comment_research_kernel::derive_current_sources;
    use serde_json::json;
    use tower::ServiceExt;

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
    fn run_confirmation_does_not_accept_client_comment_selection() {
        assert!(serde_json::from_value::<StartResearchKernelRun>(json!({})).is_ok());
        assert!(
            serde_json::from_value::<StartResearchKernelRun>(json!({
                "derivationRefs":["00000000-0000-0000-0000-000000000000"]
            }))
            .is_err()
        );
    }

    #[test]
    fn page_has_one_v1_surface_and_no_retired_query_or_daily_entry() {
        let page = page_html(&[], None);
        for label in ["概览", "用户原声", "用户问题", "变化观察", "运行记录"] {
            assert!(page.contains(label));
        }
        assert!(page.contains("前往模型与向量设置"));
        assert!(page.contains("准备本轮研究"));
        assert!(page.contains("确认系统将处理的范围"));
        for retired in ["每日观察", "保存查询", "分析所选", "评论研究设置"] {
            assert!(!page.contains(retired));
        }
        assert!(!page.contains("{{"));
    }

    #[tokio::test]
    #[ignore = "requires the isolated PostgreSQL 16 proof harness"]
    async fn voices_api_is_readable_without_a_published_result() {
        let database = super::super::material_projection_tests::proof_database(
            "comment_research_voices_api_without_result",
        )
        .await;
        let source_ref = seed_ordinary_voice(&database).await;
        assert_eq!(derive_current_sources(&database, 10).await.unwrap(), 1);
        let source_author_display_name: Option<String> = sqlx::query_scalar(
            "SELECT author_display_name FROM linggan_material_comment WHERE material_ref=$1",
        )
        .bind(source_ref)
        .fetch_one(database.pool())
        .await
        .unwrap();
        assert_eq!(source_author_display_name.as_deref(), Some("合成读者昵称"));
        let application = super::super::app_with_database(database);

        // This deliberately has a derivation but no ResultRevision.  The source fact contains a
        // display name so the HTTP proof can assert that the voices DTO does not expose it.
        let voices = application
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/local/comment-research/voices?limit=1&offset=0")
                    .header(header::HOST, "127.0.0.1:3000")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(voices.status(), StatusCode::OK);
        let voices: Value =
            serde_json::from_slice(&to_bytes(voices.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(voices["view"], "voices");
        assert_eq!(voices["source"]["kind"], "current_readable_ordinary_user");
        assert!(voices.get("result").is_none());
        assert_eq!(voices["page"]["limit"], 1);
        assert_eq!(voices["page"]["offset"], 0);
        assert_eq!(voices["page"]["total"], 1);
        assert_eq!(
            voices["page"]["items"][0]["sourceRef"],
            source_ref.to_string()
        );
        assert!(
            voices["page"]["items"][0]
                .get("authorDisplayName")
                .is_none()
        );
        assert!(voices["page"]["items"][0].get("atomKinds").is_none());

        let overview = application
            .oneshot(
                Request::builder()
                    .uri("/api/local/comment-research/overview")
                    .header(header::HOST, "127.0.0.1:3000")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(overview.status(), StatusCode::NOT_FOUND);
        let overview: Value =
            serde_json::from_slice(&to_bytes(overview.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(overview["error"], "comment_research_result_unavailable");
    }

    async fn seed_ordinary_voice(database: &linggan_storage_postgres::Database) -> uuid::Uuid {
        let note_id = "voices-api-note";
        super::super::comment_research_api_fixture::submit_package(
            database,
            "content_detail",
            json!({"contentExternalId":note_id}),
            json!({
                "kind":"content_detail",
                "sourceObject":{"platform":"xhs","type":"content","externalId":note_id},
                "payload":{
                    "noteId":note_id,
                    "title":"合成作品",
                    "bodyText":"SYNTHETIC / NOT EVIDENCE · 作品上下文",
                    "authorId":"creator-1",
                    "authorName":"合成作品作者"
                }
            }),
        )
        .await;
        let package_ref = super::super::comment_research_api_fixture::submit_package(
            database,
            "comments",
            json!({"contentExternalId":note_id}),
            json!({
                "kind":"comment",
                "sourceObject":{"platform":"xhs","type":"content","externalId":note_id},
                "payload":{
                    "commentId":"voices-api-comment",
                    "noteId":note_id,
                    "text":"我想知道怎么开始做作业",
                    "authorId":"reader-1",
                    "authorName":"合成读者昵称"
                }
            }),
        )
        .await;
        sqlx::query_scalar("SELECT material_ref FROM linggan_material_comment WHERE package_ref=$1")
            .bind(package_ref)
            .fetch_one(database.pool())
            .await
            .unwrap()
    }
}
