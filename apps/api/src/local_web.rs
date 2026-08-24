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
            <div class="v7-search-row"><label class="v7-search"><span class="v7-cmd">⌘ FIND</span><input disabled aria-disabled="true" placeholder="等待受控材料读投影接通……"><kbd>⌘ K</kbd></label><div class="v7-scope" aria-label="搜索范围"><button disabled aria-current="true">全部</button><button disabled>作品</button><button disabled>评论</button><button disabled>转录</button><button disabled>作者</button></div></div>
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
              <div class="v7-results-empty">
                <article class="v7-empty-row"><input class="v7-check" type="checkbox" disabled aria-label="无材料"><div class="v7-empty-mark">?</div><div class="v7-empty-main"><div class="v7-empty-title">SOURCE_INCOMPLETE</div><div class="v7-empty-copy">页面还没有连接到受控的本地材料读投影，因此不能列出 Content、评论、转录或来源对象。</div><div class="v7-empty-boundary"><b>NO_ACCEPTED_MATERIAL_AVAILABLE</b>这不是世界中不存在内容，也不是库内数量为零。</div><div class="v7-empty-facts"><span>TRUTH <strong>UNKNOWN</strong></span><span>COVERAGE <strong>UNKNOWN</strong></span><span>DISPLAY <strong>NOT CONNECTED</strong></span></div></div><div class="v7-empty-metric"><div><b>—</b><span>ITEMS</span></div><div><b>—</b><span>OBS</span></div><div><b>—</b><span>SOURCE</span></div></div></article>
                <section class="v7-empty-panel" aria-labelledby="empty-title"><h2 id="empty-title">没有可展示的本地材料</h2><p>当前本地 host 只提供此页面的视觉和信息边界；它没有读取数据库、历史内容工作台或插件结果。</p><dl class="v7-empty-grid"><div><dt>现在知道什么</dt><dd>页面可被本地 host 提供；材料读取合同未接通。</dd></div><div><dt>现在不知道什么</dt><dd>材料、来源、观察时间、Capture 与 Coverage 均为未知。</dd></div><div><dt>下一步</dt><dd>001B 另立范围后才能建立受控只读投影。</dd></div></dl></section>
              </div>
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
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode, header},
    };
    use std::collections::BTreeMap;
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
        assert!(!evidence_library_html().contains(&["SYSTEM", "LIVE"].join(" ")));
    }

    #[test]
    fn evidence_page_does_not_replace_unknown_with_zero() {
        assert!(evidence_library_html().contains("COVERAGE <strong>UNKNOWN</strong>"));
        assert!(!evidence_library_html().contains("评论 0"));
    }

    #[test]
    fn evidence_page_keeps_the_v7_shell_and_three_column_geometry() {
        let html = evidence_library_html();

        for required in [
            "v7-global-header",
            "v7-context-row",
            "v7-side",
            "v7-page-header",
            "v7-workspace",
            "v7-results",
            "v7-inspect",
            "FACT LAYER / EVIDENCE",
        ] {
            assert!(html.contains(required), "missing V7 structure: {required}");
        }

        for required_css in [
            "html,body { height:100%; overflow:hidden; }",
            ".v7-app { height:100vh; min-height:0;",
            "--v7-header-height:128px",
            "--v7-side-width:216px",
            "--v7-inspector-width:440px",
            "--v7-brand-red:#e8003f",
            "--v7-red:#ef4f25",
            "@media(max-width:900px){html,body{height:auto;min-height:100%;overflow:auto}.v7-app{height:auto;min-height:100vh;overflow:visible}",
        ] {
            assert!(
                EVIDENCE_LIBRARY_CSS.contains(required_css),
                "missing V7 page-local visual constant: {required_css}"
            );
        }
    }

    #[test]
    fn evidence_page_has_no_fabricated_v7_runtime_material_or_actions() {
        let html = evidence_library_html();

        let prohibited = [
            ["SYSTEM", "LIVE"].join(" "),
            ["INDEX", "FRESH"].join(" "),
            ["12", "482"].join(","),
            ["327", "9K"].join("."),
            ["XHS", "78F2A"].join("-"),
            ["为什么 ADHD 孩子", "每天写作业都像打仗？"].concat(),
            ["已创建", "补采任务"].concat(),
            ["已保存为", "个人视图"].concat(),
        ];

        for prohibited in &prohibited {
            assert!(
                !html.contains(prohibited),
                "fabricated V7 value: {prohibited}"
            );
        }

        assert!(html.contains("disabled aria-disabled=\"true\""));
        assert!(html.contains("NO_ACCEPTED_MATERIAL_AVAILABLE"));
    }

    #[test]
    fn runtime_token_source_matches_the_full_lids_baseline() {
        let runtime = declared_token_values(LIDS_TOKENS);
        let documented = declared_token_values(LIDS_TOKEN_DOCUMENT);

        assert_eq!(runtime.len(), 107);
        assert_eq!(documented.len(), 107);
        assert_eq!(runtime, documented);
        assert!(declared_token_values(EVIDENCE_LIBRARY_CSS).is_empty());
    }

    fn declared_token_values(stylesheet: &str) -> BTreeMap<&str, &str> {
        stylesheet
            .lines()
            .filter_map(|line| line.trim().strip_prefix("--lgi-"))
            .filter_map(|line| {
                line.split_once(':')
                    .map(|(name, value)| (name, value.trim().trim_end_matches(';')))
            })
            .collect()
    }
}
