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
use linggan_evidence::observation_domain::ObservationDomain;
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
        .route("/api/local/comment-research/works", get(work_options))
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
        .merge(super::comment_daily::routes())
        .merge(super::comment_intelligence::routes())
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

/// 语料页每个子页都在某一个当前观察领域下工作。领域由地址带来，解析仍在服务端做
/// 一次——与证据库同一个 `resolve_current_domain`，写了非法值或已停用的领域一律回落
/// 本领域，页面不会因为一个读不出来的参数而空着。
#[derive(Deserialize)]
struct CorpusDomainQuery {
    domain: Option<String>,
}

async fn page(
    State(state): State<LocalWebState>,
    Query(params): Query<CorpusDomainQuery>,
) -> Html<String> {
    Html(corpus_page_html(&state, false, params.domain.as_deref()).await)
}
async fn queries_page(
    State(state): State<LocalWebState>,
    Query(params): Query<CorpusDomainQuery>,
) -> Html<String> {
    Html(corpus_page_html(&state, true, params.domain.as_deref()).await)
}

async fn corpus_page_html(state: &LocalWebState, queries: bool, domain: Option<&str>) -> String {
    // 读不出领域时给空列表：选择器随之隐藏，页面照常以本领域呈现。与证据库同一处置——
    // 缺一个切换器远好过显示一个点不动的假控件。
    let domains = match state.database.database() {
        Some(database) => linggan_evidence::observation_domain::read_observation_domains(database)
            .await
            .unwrap_or_default(),
        None => Vec::new(),
    };
    let current = linggan_evidence::observation_domain::resolve_current_domain(&domains, domain);
    page_html(queries, &domains, current)
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
struct ResearchSearchQuery {
    #[serde(default)]
    text: String,
    work_ref: Option<Uuid>,
    cursor: Option<String>,
    clean_state: Option<String>,
}
async fn work_options(State(state): State<LocalWebState>) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    match read_comment_work_options(db).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => read_error(e),
    }
}
async fn search(
    State(state): State<LocalWebState>,
    Query(query): Query<ResearchSearchQuery>,
) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    let members = if let Some(clean) = &query.clean_state {
        if !linggan_intelligence::comment_daily::schema_ready(db)
            .await
            .unwrap_or(false)
        {
            return unavailable();
        }
        match linggan_intelligence::comment_daily::cleaning_members(db, clean).await {
            Ok(v) => Some(v),
            Err(_) => return error(StatusCode::BAD_REQUEST, "invalid_query"),
        }
    } else {
        None
    };
    let source_query = CommentResearchQuery {
        text: query.text,
        work_ref: query.work_ref,
        cursor: query.cursor,
    };
    let mut page = match read_comment_research_subset(
        db,
        &source_query,
        members.as_deref(),
        query.clean_state.as_deref().unwrap_or(""),
    )
    .await
    {
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
    let research_states = if linggan_intelligence::comment_daily::schema_ready(db)
        .await
        .unwrap_or(false)
    {
        let refs: Vec<_> = page.items.iter().map(|s| s.source_ref).collect();
        linggan_intelligence::comment_daily::source_states(db, &refs)
            .await
            .ok()
            .map(|mut v| {
                if let Some(items) = v.as_array_mut() {
                    for item in items {
                        item.as_object_mut().map(|v| v.remove("cleaning"));
                    }
                }
                v
            })
    } else {
        None
    };
    Json(json!({"researchStates":research_states,"page":page,"works":works,"modelConnected":model["modelConnected"],"modelState":model["modelState"],"scope":"ACCEPTED_READABLE_COMMENT_SAMPLE"})).into_response()
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
    if linggan_intelligence::comment_daily::schema_ready(db)
        .await
        .unwrap_or(false)
    {
        context["processing"] =
            linggan_intelligence::comment_daily::source_states(db, &[source_ref])
                .await
                .ok()
                .and_then(|v| v.as_array().and_then(|v| v.first()).cloned())
                .unwrap_or(Value::Null);
    }
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

fn page_html(
    queries: bool,
    domains: &[ObservationDomain],
    current: Option<&ObservationDomain>,
) -> String {
    let title = if queries {
        "已存查询"
    } else {
        "评论研究"
    };
    let page = if queries {
        shell::CorpusPage::Queries
    } else {
        shell::CorpusPage::Comments
    };
    let action = if queries {
        "/corpus/queries"
    } else {
        "/corpus/comments"
    };
    // 换领域留在当前子页：换的是观察对象，不是把人送回证据库。
    let picker = super::corpus_domain_picker(domains, current, action, None);
    let crumb = if picker.is_empty() {
        format!("语料 <span class=\"v7-slash\">/</span> <b>{title}</b>")
    } else {
        format!(
            "语料 <span class=\"v7-slash\">/</span> {picker} <span class=\"v7-slash\">/</span> <b>{title}</b>"
        )
    };
    let header = shell::global_header(
        shell::PrimarySurface::Corpus,
        "本机研究",
        &crumb,
        "<span>内部研究</span><span>样本范围</span>",
        None,
    );
    let side_nav = shell::corpus_side_nav(
        page,
        super::corpus_nav_domain(domains, current).as_deref(),
        "材料到达即可研究<br>内部受限研究入口",
    );
    // 视图 tab 是写死的相对链接，点击被 JS 接管，但脚本没跑起来时它们会原样生效并把
    // 领域从查询串里冲掉。根因与侧栏那处同源：任何一个写死的链接都得自己把领域带上。
    let domain_qs = match super::corpus_nav_domain(domains, current) {
        Some(domain_ref) => format!("domain={domain_ref}&amp;"),
        None => String::new(),
    };
    let html = include_str!("comment_research.html")
        .replace("{{HEADER}}", &header)
        .replace("{{SIDE_NAV}}", &side_nav)
        .replace("{{TITLE}}", title)
        .replace("{{VIEW}}", if queries { "queries" } else { "overview" })
        .replace("{{DOMAIN_QS}}", &domain_qs)
        .replace(
            "{{CORPUS_DOMAIN_REF}}",
            &current
                .map(|domain| domain.domain_ref.to_string())
                .unwrap_or_default(),
        )
        .replace(
            "{{CORPUS_DOMAIN_OWN}}",
            if current.is_none_or(|domain| domain.is_own_domain) {
                "true"
            } else {
                "false"
            },
        )
        .replace(
            "{{CORPUS_DOMAIN_NAME}}",
            &current
                .map(|domain| super::html_escape(&domain.name))
                .unwrap_or_default(),
        );
    if std::env::var("LINGGAN_MODEL_SYNTHETIC_PREVIEW").as_deref() == Ok("SYNTHETIC-NOT-EVIDENCE") {
        html.replace("<body ", "<body data-research-synthetic=\"true\" ")
            .replace(
                "<p class=\"lgi-research-boundary\">",
                "<p class=\"lgi-research-boundary\">合成验收环境，非真实研究材料。 ",
            )
    } else {
        html
    }
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
    fn domain(name: &str, own: bool) -> ObservationDomain {
        ObservationDomain {
            domain_ref: Uuid::new_v4(),
            name: name.to_owned(),
            is_own_domain: own,
            status: "active".to_owned(),
            sample_count: None,
        }
    }

    /// 这一条守的是本页此前真实存在的缺陷：选了外部领域，点一下评论研究就掉回本领域。
    /// 根因是侧栏链接写死成裸路径，领域在跳转那一刻被丢掉。导航现在由 shell 一处生成，
    /// 这条断言保证它继续把当前领域带上——包括切换器提交回的是本页而不是证据库。
    #[test]
    fn every_corpus_link_carries_the_current_domain() {
        let domains = vec![domain("ADHD", true), domain("考研自习", false)];
        let current = &domains[1];
        let html = page_html(false, &domains, Some(current));
        let domain_ref = current.domain_ref;
        for page in ["/corpus/evidence", "/corpus/comments", "/corpus/queries"] {
            assert!(
                html.contains(&format!("href=\"{page}?domain={domain_ref}\"")),
                "{page} 丢掉了当前领域"
            );
        }
        // 切换器早已不是提交表单，而是一个链接列表；它同样得指回本页，换的是观察领域，
        // 不是把人送回证据库。用非当前领域来验：当前领域的链接侧栏也会出，区分不开。
        let other = domains[0].domain_ref;
        assert!(html.contains(&format!("href=\"/corpus/comments?domain={other}\"")));
        // 视图 tab 同样不许把领域冲掉：脚本没跑起来时它们会原样生效。
        assert!(html.contains(&format!(
            "href=\"/corpus/comments?domain={domain_ref}&amp;view=voices\""
        )));
        assert!(html.contains("data-corpus-domain-own=\"false\""));
        assert!(html.contains("data-corpus-domain=\"") && html.contains(&domain_ref.to_string()));
    }

    /// 少于两个领域时页面上没有可切换的东西，链接不该带一个参数假装有得选。
    #[test]
    fn a_single_domain_leaves_the_links_bare() {
        let domains = vec![domain("ADHD", true)];
        let html = page_html(false, &domains, Some(&domains[0]));
        assert!(html.contains("href=\"/corpus/evidence\""));
        // 只查链接上的领域参数：body 的 data-corpus-domain 是页面下发的事实，始终该在。
        assert!(!html.contains("?domain="));
        assert!(!html.contains("&amp;view="));
        assert!(html.contains("href=\"/corpus/comments?view=voices\""));
        assert!(!html.contains("v7-domain-picker"));
    }

    #[test]
    fn research_defaults_to_overview_with_four_research_views() {
        let html = page_html(false, &[], None);
        for expected in [
            "概览",
            "原声",
            "用户问题",
            "每日观察",
            "/corpus/queries",
            "data-initial-view=\"overview\"",
        ] {
            assert!(html.contains(expected));
        }
        assert!(!html.contains("{{"));
        assert!(!html.contains("选题库"));
    }
}
