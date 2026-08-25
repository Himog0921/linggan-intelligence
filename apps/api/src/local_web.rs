mod evidence_page;

use axum::{
    Json, Router,
    body::Bytes,
    extract::{Query, State},
    http::{HeaderValue, header},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use linggan_contracts::EvidenceQuery;
use linggan_evidence::{
    DiscoveryIngressError, ingest_discovery_package, local_discovery_schema_is_ready,
    read_discovery_library,
};
use linggan_storage_postgres::Database;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    io::{Error, ErrorKind},
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
};

const LOCAL_HOST: Ipv4Addr = Ipv4Addr::LOCALHOST;
const LOCAL_PORT: u16 = 3000;
const LIDS_TOKENS: &str = include_str!("local_web/lids_tokens.css");
const EVIDENCE_LIBRARY_CSS: &str = include_str!("local_web/evidence_library.css");
#[cfg(test)]
const LIDS_TOKEN_DOCUMENT: &str = include_str!("../../../docs/design/lids/tokens.md");

#[derive(Clone)]
struct LocalWebState {
    database: LocalDatabaseState,
}

#[derive(Clone)]
enum LocalDatabaseState {
    NotConfigured,
    DatabaseUnavailable,
    SchemaUnavailable,
    Ready(Arc<Database>),
}

impl LocalDatabaseState {
    fn database(&self) -> Option<&Database> {
        match self {
            Self::Ready(database) => Some(database),
            Self::NotConfigured | Self::DatabaseUnavailable | Self::SchemaUnavailable => None,
        }
    }

    async fn health_state(&self) -> (&'static str, &'static str, &'static str, &'static str) {
        match self {
            Self::NotConfigured => (
                "SOURCE_INCOMPLETE",
                "NOT_CONNECTED",
                "NOT_CONFIGURED",
                "NOT_CHECKED",
            ),
            Self::DatabaseUnavailable => (
                "SOURCE_INCOMPLETE",
                "NOT_CONNECTED",
                "CONFIGURED_UNAVAILABLE",
                "LOCAL_001_DATABASE_UNAVAILABLE",
            ),
            Self::SchemaUnavailable => (
                "SOURCE_INCOMPLETE",
                "NOT_CONNECTED",
                "CONFIGURED_UNAVAILABLE",
                "LOCAL_001_SCHEMA_UNAVAILABLE",
            ),
            Self::Ready(database) => match local_discovery_schema_is_ready(database).await {
                Ok(true) => (
                    "LOCAL_DISCOVERY_READ_PROJECTION",
                    "DISCOVERY_ONLY",
                    "READY",
                    "LOCAL_001_SCHEMA_READY",
                ),
                Ok(false) => (
                    "SOURCE_INCOMPLETE",
                    "NOT_CONNECTED",
                    "CONFIGURED_UNAVAILABLE",
                    "LOCAL_001_SCHEMA_UNAVAILABLE",
                ),
                Err(_) => (
                    "SOURCE_INCOMPLETE",
                    "NOT_CONNECTED",
                    "CONFIGURED_UNAVAILABLE",
                    "LOCAL_001_DATABASE_UNAVAILABLE",
                ),
            },
        }
    }
}

#[derive(Debug, Deserialize)]
struct EvidenceLibraryParams {
    q: Option<String>,
    window: Option<String>,
}

#[cfg(test)]
fn app() -> Router {
    router(LocalWebState {
        database: LocalDatabaseState::NotConfigured,
    })
}

#[cfg(test)]
fn app_with_database(database: Database) -> Router {
    router(LocalWebState {
        database: LocalDatabaseState::Ready(Arc::new(database)),
    })
}

fn router(state: LocalWebState) -> Router {
    Router::new()
        .route("/", get(local_entry))
        .route("/health", get(health))
        .route("/api/local/discovery-packages", post(discovery_ingress))
        .route("/api/local/evidence-library", get(evidence_library_json))
        .route("/corpus/evidence", get(evidence_library))
        .route("/assets/evidence-library.css", get(stylesheet))
        .with_state(state)
}

async fn local_entry() -> Redirect {
    Redirect::temporary("/corpus/evidence")
}

pub async fn serve() -> Result<(), std::io::Error> {
    let application = router(LocalWebState {
        database: configured_database_state().await,
    });
    let local_port = configured_local_port()?;
    let address = SocketAddr::from((LOCAL_HOST, local_port));
    let listener = tokio::net::TcpListener::bind(address).await?;
    println!("Linggan local host listening on http://localhost:{local_port}");
    axum::serve(listener, application).await
}

fn configured_local_port() -> Result<u16, std::io::Error> {
    match std::env::var("LINGGAN_LOCAL_PORT") {
        Ok(value) => match value.parse::<u16>() {
            Ok(port) if port > 0 => Ok(port),
            _ => Err(Error::new(
                ErrorKind::InvalidInput,
                "LINGGAN_LOCAL_PORT must be a valid non-zero loopback TCP port",
            )),
        },
        Err(_) => Ok(LOCAL_PORT),
    }
}

async fn health(State(state): State<LocalWebState>) -> Json<Value> {
    let (data_state, evidence_read_model, database_state, schema_state) =
        state.database.health_state().await;
    Json(json!({
        "service": "linggan-local-web",
        "listener": "loopback-only",
        "dataState": data_state,
        "evidenceReadModel": evidence_read_model,
        "database": {
            "state": database_state,
            "schema": schema_state
        },
        "routes": {
            "evidenceLibrary": "/corpus/evidence",
            "discoveryIngress": "/api/local/discovery-packages"
        }
    }))
}

async fn evidence_library(
    State(state): State<LocalWebState>,
    Query(params): Query<EvidenceLibraryParams>,
) -> Html<String> {
    match state.database.database() {
        None => Html(evidence_library_html().to_owned()),
        Some(database) => match local_query(&params) {
            Ok(query) => match read_discovery_library(database, &query).await {
                Ok(projection) => Html(evidence_page::render_read_projection(
                    evidence_library_html(),
                    &projection,
                    params.q.as_deref(),
                )),
                Err(_) => Html(evidence_read_unavailable_html()),
            },
            Err(()) => Html(evidence_query_invalid_html()),
        },
    }
}

async fn evidence_library_json(
    State(state): State<LocalWebState>,
    Query(params): Query<EvidenceLibraryParams>,
) -> Response {
    let Some(database) = state.database.database() else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    let Ok(query) = local_query(&params) else {
        return local_read_json_error(
            axum::http::StatusCode::BAD_REQUEST,
            "invalid_local_evidence_query",
        );
    };
    match read_discovery_library(database, &query).await {
        Ok(projection) => Json(projection).into_response(),
        Err(_) => local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "read_projection_unavailable",
        ),
    }
}

async fn discovery_ingress(State(state): State<LocalWebState>, body: Bytes) -> Response {
    let Some(database) = state.database.database() else {
        return ingress_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "ingress_not_connected",
        );
    };
    let Ok(body) = std::str::from_utf8(&body) else {
        return ingress_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "discovery_contract_invalid",
        );
    };
    match ingest_discovery_package(database, body).await {
        Ok(outcome) => Json(outcome).into_response(),
        Err(DiscoveryIngressError::Contract(_)) => ingress_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "discovery_contract_invalid",
        ),
        Err(DiscoveryIngressError::Internal(_)) => ingress_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "ingress_not_committed",
        ),
    }
}

async fn configured_database_state() -> LocalDatabaseState {
    let Ok(url) = std::env::var("LINGGAN_LOCAL_DATABASE_URL") else {
        return LocalDatabaseState::NotConfigured;
    };
    let database = match Database::connect(&url).await {
        Ok(database) => database,
        Err(_) => return LocalDatabaseState::DatabaseUnavailable,
    };
    if local_discovery_schema_is_ready(&database)
        .await
        .unwrap_or(false)
    {
        LocalDatabaseState::Ready(Arc::new(database))
    } else {
        LocalDatabaseState::SchemaUnavailable
    }
}

fn local_query(params: &EvidenceLibraryParams) -> Result<EvidenceQuery, ()> {
    let window = match params.window.as_deref().unwrap_or("last_30_days") {
        "last_7_days" => "last_7_days",
        "last_30_days" => "last_30_days",
        _ => return Err(()),
    };
    serde_json::from_value(json!({
        "text": params.q,
        "scope": "all_accepted_material",
        "window": window,
        "sort": "latest_discovery"
    }))
    .map_err(|_| ())
}

fn ingress_json_error(status: axum::http::StatusCode, code: &'static str) -> Response {
    (
        status,
        Json(json!({ "admission": "not_accepted", "code": code })),
    )
        .into_response()
}

fn local_read_json_error(status: axum::http::StatusCode, code: &'static str) -> Response {
    (
        status,
        Json(json!({ "operation": "local_read", "outcome": "unavailable", "code": code })),
    )
        .into_response()
}

fn evidence_read_unavailable_html() -> String {
    evidence_library_html()
        .replace("SOURCE_INCOMPLETE", "READ_PROJECTION_UNAVAILABLE")
        .replace(
            "页面还没有连接到受控的本地材料读投影",
            "页面无法从受控本地读投影读取卡片；没有显示任何旧系统或远程数据",
        )
        .to_owned()
}

fn evidence_query_invalid_html() -> String {
    evidence_library_html()
        .replace("SOURCE_INCOMPLETE", "LOCAL_QUERY_INVALID")
        .replace(
            "页面还没有连接到受控的本地材料读投影",
            "当前只接受本地 EvidenceQuery；没有触发平台搜索或补采",
        )
        .to_owned()
}

async fn stylesheet() -> Response {
    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/css; charset=utf-8"),
        )],
        format!("{LIDS_TOKENS}\n{EVIDENCE_LIBRARY_CSS}"),
    )
        .into_response()
}

fn evidence_library_html() -> &'static str {
    r#"<!doctype html>
<html lang="zh-CN" data-theme="linggan-intelligence">
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta name="color-scheme" content="light">
    <title>Evidence Library · Linggan Intelligence</title>
    <link rel="stylesheet" href="/assets/evidence-library.css">
  </head>
  <body>
    <div class="v7-app">
      <header class="v7-global-header">
        <div class="v7-global-row">
          <div class="v7-global-brand" aria-label="Linggan Intelligence">
            <div class="v7-li-mark">LI</div>
            <div class="v7-global-brand-copy"><div class="v7-global-brand-name">Linggan Intelligence</div><div class="v7-global-brand-sub">EDITORIAL INTELLIGENCE TERMINAL</div></div>
          </div>
          <nav class="v7-primary-nav" aria-label="一级导航">
            <button disabled aria-disabled="true">Radar <span class="v7-nav-readout">—</span></button>
            <button disabled aria-disabled="true">Topic Map <span class="v7-nav-readout">—</span></button>
            <button disabled aria-disabled="true" aria-current="page">Corpus <span class="v7-nav-readout">UNKNOWN</span></button>
            <button disabled aria-disabled="true">Insights <span class="v7-nav-readout">—</span></button>
            <button disabled aria-disabled="true">Collection <span class="v7-nav-readout">—</span></button>
          </nav>
          <div class="v7-global-flex" aria-hidden="true"></div>
          <div class="v7-global-system">
            <div class="v7-system-boundary">LOCAL HOST / NO READ MODEL</div>
            <button class="v7-global-command" disabled aria-disabled="true"><span>&gt; 输入命令</span><kbd>/</kbd></button>
          </div>
        </div>
        <div class="v7-context-row">
          <div aria-hidden="true"></div>
          <div class="v7-context-main">
            <div class="v7-context-crumb">CORPUS <span class="v7-slash">/</span> <b>EVIDENCE LIBRARY</b> <span class="v7-slash">/</span> <span class="v7-context-current">材料状态</span></div>
            <div class="v7-context-meta"><span class="v7-query-meta">READ MODEL NOT CONNECTED</span><span>SOURCE INCOMPLETE</span><span>UTC+08</span></div>
          </div>
        </div>
      </header>

      <div class="v7-shell">
        <aside class="v7-side" aria-label="语料导航">
          <div class="v7-nav-label">CORPUS</div>
          <button class="v7-side-nav" disabled aria-disabled="true" aria-current="page"><i>01</i><span>Evidence Library</span></button>
          <button class="v7-side-nav" disabled aria-disabled="true"><i>02</i><span>Comments</span></button>
          <button class="v7-side-nav" disabled aria-disabled="true"><i>03</i><span>Creators</span></button>
          <button class="v7-side-nav" disabled aria-disabled="true"><i>04</i><span>Saved Queries</span></button>
          <button class="v7-side-nav" disabled aria-disabled="true"><i>05</i><span>Sources</span></button>
          <div class="v7-side-foot"><span class="v7-side-dot"></span>SOURCE INCOMPLETE<br><span class="v7-mono">local presentation only · no material read</span></div>
        </aside>

        <main class="v7-main" aria-labelledby="page-title">
          <section class="v7-page-header">
            <div class="v7-header-row">
              <div><div class="v7-eyebrow">CORPUS / 事实资产工作台</div><h1 class="v7-title" id="page-title">Evidence Library</h1><div class="v7-sub">检索、核验、追溯系统已经观察到的真实内容；事实与推导保持分层。</div></div>
              <div class="v7-header-right">
                <div class="v7-stats"><div class="v7-stat"><span>01 / CONTENT</span><b>UNKNOWN</b></div><div class="v7-stat"><span>02 / COMMENTS</span><b>UNKNOWN</b></div><div class="v7-stat"><span>03 / CREATORS</span><b>UNKNOWN</b></div></div>
                <div class="v7-page-actions"><button class="v7-btn" disabled aria-disabled="true">复制查询</button><button class="v7-btn" disabled aria-disabled="true">保存当前视图</button><button class="v7-btn v7-primary" disabled aria-disabled="true">发起研究</button></div>
              </div>
            </div>
            <form class="v7-search-row" method="get"><label class="v7-search"><span class="v7-cmd">⌘ FIND</span><!-- EVIDENCE_SEARCH_INPUT_START --><input disabled aria-disabled="true" placeholder="等待受控材料读投影接通……"><!-- EVIDENCE_SEARCH_INPUT_END --><kbd>⌘ K</kbd></label><div class="v7-scope" aria-label="搜索范围"><button disabled aria-current="true">全部</button><button disabled>作品</button><button disabled>评论</button><button disabled>转录</button><button disabled>作者</button></div></form>
            <div class="v7-views-bar">
              <div class="v7-view-group"><span class="v7-view-label">SYSTEM VIEWS / 系统视图</span><div class="v7-view-strip"><button class="v7-view-pill" disabled aria-current="true"><i>01</i><span>最新发现</span><span class="v7-n">—</span></button><button class="v7-view-pill" disabled><i>02</i><span>待补采</span><span class="v7-n">—</span></button><button class="v7-view-pill" disabled><i>03</i><span>高互动</span><span class="v7-n">—</span></button><button class="v7-view-pill" disabled><i>04</i><span>评论密集</span><span class="v7-n">—</span></button><button class="v7-view-pill" disabled><i>05</i><span>最近异常</span><span class="v7-n">—</span></button><button class="v7-view-pill v7-more" disabled><i>+</i><span>更多</span><span class="v7-n">⌄</span></button></div></div>
              <div class="v7-view-group v7-my"><span class="v7-view-label">MY VIEWS / 我的视图</span><div class="v7-view-strip"><button class="v7-view-pill" disabled><i>A</i><span>ADHD 作业</span></button><button class="v7-view-pill" disabled><i>B</i><span>低粉爆文</span></button><button class="v7-view-pill" disabled><i>C</i><span>家长原声研究</span></button></div></div>
            </div>
            <div class="v7-controls-row"><div class="v7-filters"><span class="v7-filter-lead">FILTER /</span><button class="v7-chip" disabled><em>PLATFORM:</em> ALL</button><button class="v7-chip" disabled><em>WINDOW:</em> UNKNOWN</button><button class="v7-chip" disabled><em>TYPE:</em> ALL</button><button class="v7-chip" disabled><em>SOURCE:</em> UNKNOWN</button><button class="v7-chip" disabled><em>STATUS:</em> UNKNOWN</button></div><div class="v7-controls"><button class="v7-control" disabled><small>GROUP /</small><strong>不分组</strong></button><button class="v7-control" disabled><small>SORT /</small><strong>UNKNOWN</strong></button><button class="v7-control" disabled><small>DENSITY /</small><strong>标准</strong></button><div class="v7-seg"><span class="v7-seg-label">VIEW /</span><button disabled aria-current="true">RESEARCH</button><button disabled>TABLE</button><button disabled>COVER</button></div></div></div>
            <div class="v7-query-line"><div>页面结构已就绪 · 材料读投影尚未接通</div><div><b>SOURCE_INCOMPLETE</b> · <span>NO QUERY AVAILABLE</span></div></div>
          </section>

          <section class="v7-workspace" aria-label="Evidence Library 工作区">
            <section class="v7-results" aria-label="事实材料列表">
              <div class="v7-fact-strap"><span>FACT LAYER / EVIDENCE</span><i aria-hidden="true"></i><b>原始内容资产</b></div>
              <div class="v7-results-head"><div class="v7-results-left"><input class="v7-check" type="checkbox" disabled aria-label="选择全部材料"><span>NO ACCEPTED MATERIAL AVAILABLE</span></div><div>READ MODEL NOT CONNECTED</div></div>
              <!-- EVIDENCE_RESULTS_START --><div class="v7-results-empty">
                <article class="v7-empty-row"><input class="v7-check" type="checkbox" disabled aria-label="无材料"><div class="v7-empty-mark">?</div><div class="v7-empty-main"><div class="v7-empty-title">SOURCE_INCOMPLETE</div><div class="v7-empty-copy">页面还没有连接到受控的本地材料读投影，因此不能列出 Content、评论、转录或来源对象。</div><div class="v7-empty-boundary"><b>NO_ACCEPTED_MATERIAL_AVAILABLE</b>这不是世界中不存在内容，也不是库内数量为零。</div><div class="v7-empty-facts"><span>TRUTH <strong>UNKNOWN</strong></span><span>COVERAGE <strong>UNKNOWN</strong></span><span>DISPLAY <strong>NOT CONNECTED</strong></span></div></div><div class="v7-empty-metric"><div><b>—</b><span>ITEMS</span></div><div><b>—</b><span>OBS</span></div><div><b>—</b><span>SOURCE</span></div></div></article>
                <section class="v7-empty-panel" aria-labelledby="empty-title"><h2 id="empty-title">没有可展示的本地材料</h2><p>当前本地 host 只提供此页面的视觉和信息边界；它没有读取数据库、历史内容工作台或插件结果。</p><dl class="v7-empty-grid"><div><dt>现在知道什么</dt><dd>页面可被本地 host 提供；材料读取合同未接通。</dd></div><div><dt>现在不知道什么</dt><dd>材料、来源、观察时间、Capture 与 Coverage 均为未知。</dd></div><div><dt>下一步</dt><dd>001B 另立范围后才能建立受控只读投影。</dd></div></dl></section>
              </div><!-- EVIDENCE_RESULTS_END -->
            </section>
            <aside class="v7-inspect" aria-labelledby="inspector-title">
              <div class="v7-inspector-head"><div class="v7-inspector-identity"><div class="v7-iid">#NO_SELECTION</div><h2 class="v7-ititle" id="inspector-title">尚未选择材料</h2><div class="v7-imeta"><span>CONTENT ITEM UNKNOWN</span><span>·</span><span>OBSERVATION UNKNOWN</span><span>·</span><span>CAPTURE UNKNOWN</span></div></div><div class="v7-inspector-ops"><div class="v7-inspector-primary"><button disabled aria-disabled="true">↗ 原文</button><button disabled aria-disabled="true">⟳ 补采</button><button disabled aria-disabled="true">＋ 研究</button></div><div class="v7-inspector-window"><button disabled aria-disabled="true">PIN</button><button disabled aria-disabled="true">WIDE</button><button disabled aria-disabled="true">×</button></div></div><div class="v7-tabs" aria-label="材料详情页签"><button disabled aria-current="page">Overview</button><button disabled>Content</button><button disabled>Comments</button><button disabled>History</button><button disabled>Provenance</button><button disabled>Relations</button></div></div>
              <div class="v7-inspector-body"><section class="v7-section"><h3>RAW CONTENT / 原始内容 <span>UNKNOWN</span></h3><div class="v7-body-copy">没有选中 ContentItem，也没有可显示的受限材料。此处不能推断标题、正文、作者或平台状态。</div></section><section class="v7-section"><h3>STRONGEST MATCH / 最强命中 <span>NO MATERIAL</span></h3><div class="v7-quote">没有材料可供匹配或引用。<small>不显示示例原文、评论或转录。</small></div></section><section class="v7-section"><h3>PLATFORM / LOCAL FACTS <span>UNKNOWN</span></h3><div class="v7-readout"><div><b>—</b><span>PLATFORM LIKES</span></div><div><b>—</b><span>PLATFORM COMMENTS</span></div><div><b>—</b><span>LOCAL COMMENTS</span></div><div><b>—</b><span>OBSERVATIONS</span></div></div></section><section class="v7-section"><h3>STATE MATRIX</h3><div class="v7-matrix"><div class="v7-matrix-row"><div class="v7-k">FACT SOURCE</div><div class="v7-v"><i class="v7-dot"></i>SOURCE_INCOMPLETE</div></div><div class="v7-matrix-row"><div class="v7-k">READ MODEL</div><div class="v7-v"><i class="v7-dot"></i>NOT CONNECTED</div></div><div class="v7-matrix-row"><div class="v7-k">COVERAGE</div><div class="v7-v"><i class="v7-dot"></i>UNKNOWN</div></div></div></section></div>
            </aside>
          </section>
        </main>
      </div>
    </div>
  </body>
</html>"#
}

#[cfg(test)]
mod tests;
