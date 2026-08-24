use axum::{
    Json, Router,
    http::{HeaderValue, header},
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
};
use serde_json::{Value, json};
use std::net::{Ipv4Addr, SocketAddr};

const LOCAL_HOST: Ipv4Addr = Ipv4Addr::LOCALHOST;
const LOCAL_PORT: u16 = 3000;
const LIDS_TOKENS: &str = include_str!("local_web/lids_tokens.css");
const EVIDENCE_LIBRARY_CSS: &str = include_str!("local_web/evidence_library.css");
#[cfg(test)]
const LIDS_TOKEN_DOCUMENT: &str = include_str!("../../../docs/design/lids/tokens.md");

pub fn app() -> Router {
    Router::new()
        .route("/", get(local_entry))
        .route("/health", get(health))
        .route("/corpus/evidence", get(evidence_library))
        .route("/assets/evidence-library.css", get(stylesheet))
}

async fn local_entry() -> Redirect {
    Redirect::temporary("/corpus/evidence")
}

pub async fn serve() -> Result<(), std::io::Error> {
    let address = SocketAddr::from((LOCAL_HOST, LOCAL_PORT));
    let listener = tokio::net::TcpListener::bind(address).await?;
    println!("Linggan local host listening on http://localhost:{LOCAL_PORT}");
    axum::serve(listener, app()).await
}

async fn health() -> Json<Value> {
    Json(json!({
        "service": "linggan-local-web",
        "listener": "loopback-only",
        "dataState": "SOURCE_INCOMPLETE",
        "evidenceReadModel": "NOT_CONNECTED",
        "routes": {
            "evidenceLibrary": "/corpus/evidence"
        }
    }))
}

async fn evidence_library() -> Html<&'static str> {
    Html(evidence_library_html())
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
    <header class="lgi-shell-header">
      <div class="lgi-brand" aria-label="Linggan Intelligence">LINGGAN<span>INTELLIGENCE</span></div>
      <nav class="lgi-primary-nav" aria-label="产品导航">
        <span>雷达</span><span>主题地图</span><span class="lgi-nav-current" aria-current="page">语料</span><span>洞察</span><span>采集</span>
      </nav>
      <div class="lgi-header-state"><span aria-hidden="true"></span>LOCAL / LOOPBACK</div>
    </header>

    <main class="lgi-workbench" aria-labelledby="page-title">
      <aside class="lgi-corpus-rail" aria-label="语料导航">
        <p class="lgi-rail-kicker">CORPUS / 01</p>
        <h2>语料</h2>
        <nav class="lgi-secondary-nav" aria-label="语料子导航">
          <span class="lgi-secondary-current" aria-current="page">Evidence Library</span>
          <span>评论</span><span>创作者</span><span>保存的查询</span><span>来源</span>
        </nav>
        <div class="lgi-rail-boundary">
          <p class="lgi-text-label">CURRENT LOCAL SCOPE</p>
          <p>页面尚未连接材料读投影。此处不使用历史工作台或示例内容。</p>
        </div>
      </aside>

      <section class="lgi-evidence-workspace" aria-labelledby="page-title">
        <div class="lgi-page-intro">
          <p class="lgi-text-label">EVIDENCE LIBRARY / LOCAL-001A</p>
          <h1 id="page-title">找到材料，理解它的边界。</h1>
          <p class="lgi-intro-copy">这里将承接 Linggan 已接纳的本地材料；当前没有可展示的本地已接纳材料。</p>
        </div>

        <section class="lgi-boundary-strip" aria-label="当前材料状态">
          <div><span class="lgi-text-label">TRUTH</span><strong>SOURCE_INCOMPLETE</strong></div>
          <div><span class="lgi-text-label">READ MODEL</span><strong>NOT CONNECTED</strong></div>
          <div><span class="lgi-text-label">COVERAGE</span><strong>UNKNOWN</strong></div>
        </section>

        <section class="lgi-empty-state" aria-labelledby="empty-title">
          <p class="lgi-empty-index">01 / MATERIALS</p>
          <h2 id="empty-title">没有可展示的本地材料</h2>
          <p>这只说明当前页面没有连接到可读的本地材料投影；它不能说明世界中不存在相关内容，也不能把未观察或未接入写成 0。</p>
          <dl>
            <div><dt>现在知道什么</dt><dd>本地 Web host 正在提供页面；材料读取合同尚未接通。</dd></div>
            <div><dt>现在不知道什么</dt><dd>本机是否已有可显示材料、其来源、观察时间、Coverage 或有效性。</dd></div>
            <div><dt>下一步由谁负责</dt><dd>001B 建立受控只读投影；001C 才会在独立授权下建立真实 discovery ingress。</dd></div>
          </dl>
        </section>
      </section>

      <aside class="lgi-provenance-inspector" aria-labelledby="inspector-title">
        <p class="lgi-text-label">PROVENANCE INSPECTOR</p>
        <h2 id="inspector-title">尚未选择材料</h2>
        <p>选择 ContentItem 后，这里才会显示它的来源、观察、采集与 Coverage 边界。</p>
        <dl class="lgi-inspector-list">
          <div><dt>CONTENT ITEM</dt><dd>UNKNOWN</dd></div>
          <div><dt>OBSERVATION</dt><dd>UNKNOWN</dd></div>
          <div><dt>CAPTURE RUN</dt><dd>UNKNOWN</dd></div>
          <div><dt>LOCAL DISPLAY</dt><dd>NO ACCEPTED MATERIAL AVAILABLE</dd></div>
        </dl>
      </aside>
    </main>
  </body>
</html>"#
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode, header},
    };
    use std::collections::BTreeSet;
    use tower::ServiceExt;

    #[tokio::test]
    async fn health_route_returns_machine_readable_local_state() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/json"
        );
    }

    #[tokio::test]
    async fn local_entry_redirects_to_the_evidence_library() {
        let response = app()
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
        assert_eq!(
            response.headers().get(header::LOCATION).unwrap(),
            "/corpus/evidence"
        );
    }

    #[tokio::test]
    async fn evidence_route_returns_the_honest_empty_state() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/corpus/evidence")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(evidence_library_html().contains("SOURCE_INCOMPLETE"));
        assert!(evidence_library_html().contains("没有可展示的本地材料"));
        assert!(!evidence_library_html().contains("LIVE"));
    }

    #[test]
    fn evidence_page_does_not_replace_unknown_with_zero() {
        assert!(evidence_library_html().contains("COVERAGE</span><strong>UNKNOWN"));
        assert!(!evidence_library_html().contains("评论 0"));
    }

    #[test]
    fn runtime_token_source_matches_the_full_lids_baseline() {
        let runtime = declared_token_names(LIDS_TOKENS);
        let documented = declared_token_names(LIDS_TOKEN_DOCUMENT);

        assert_eq!(runtime.len(), 107);
        assert_eq!(runtime, documented);
        assert!(declared_token_names(EVIDENCE_LIBRARY_CSS).is_empty());
    }

    fn declared_token_names(stylesheet: &str) -> BTreeSet<&str> {
        stylesheet
            .lines()
            .filter_map(|line| line.trim().strip_prefix("--lgi-"))
            .filter_map(|line| line.split_once(':').map(|(name, _)| name))
            .collect()
    }
}
