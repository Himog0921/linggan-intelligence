//! WEB-REACT-001 阶段 B：React 页面入口、静态资源、版本闸门与旧设置页退役的回归，
//! 以及一条在隔离 PostgreSQL 上用真实浏览器走完设置页的证明（默认忽略）。
use super::web_app::{WebAppConfig, routes_for, test_support};
use super::*;
use axum::{
    body::{Body, to_bytes},
    http::{HeaderMap, Request, StatusCode, header},
};
use tower::ServiceExt;

const ENTRY: &str = "<!doctype html><html><body><div id=\"root\"></div><script type=\"module\" src=\"/ui-assets/index-AbC_123.js\"></script></body></html>";

/// 一份合成的前端产物目录。`revision` 为 None 时不写构建信息。
fn synthetic_dist(revision: Option<&str>, with_entry: bool) -> PathBuf {
    let directory = std::env::temp_dir().join(format!("linggan-web-dist-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&directory).unwrap();
    if with_entry {
        fs::write(directory.join("index.html"), ENTRY).unwrap();
    }
    if let Some(revision) = revision {
        fs::write(
            directory.join("build-info.json"),
            json!({"revision": revision, "builtAt": "2026-10-15T00:00:00Z"}).to_string(),
        )
        .unwrap();
    }
    fs::write(
        directory.join("index-AbC_123.js"),
        "console.log('synthetic');",
    )
    .unwrap();
    directory
}

async fn send(router: Router, uri: &str, host: &str) -> (StatusCode, HeaderMap, String) {
    let response = router
        .oneshot(
            Request::builder()
                .uri(uri)
                .header(header::HOST, host)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = to_bytes(response.into_body(), 1 << 20).await.unwrap();
    (status, headers, String::from_utf8_lossy(&body).into_owned())
}

fn web(dist: PathBuf, runtime_revision: &str) -> Router {
    routes_for(WebAppConfig::new(dist, runtime_revision))
}

#[tokio::test]
async fn registered_page_returns_the_react_entry_with_local_guard_headers() {
    let router = web(synthetic_dist(Some("284cd3a9"), true), "284cd3a9");
    let (status, headers, body) = send(router, "/settings/models", "127.0.0.1:3000").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        headers[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("text/html")
    );
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    assert!(body.contains("<div id=\"root\">"));
}

#[tokio::test]
async fn settings_entry_redirects_to_the_models_page() {
    let router = web(synthetic_dist(None, true), "unknown");
    let (status, headers, _) = send(router, "/settings", "localhost:3000").await;
    assert_eq!(status, StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(headers[header::LOCATION], "/settings/models");
}

#[tokio::test]
async fn page_is_refused_to_a_foreign_host() {
    let router = web(synthetic_dist(None, true), "unknown");
    let (status, _, _) = send(router, "/settings/models", "evil.example:3000").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn missing_frontend_build_is_reported_instead_of_served_blank() {
    let router = web(synthetic_dist(None, false), "unknown");
    let (status, headers, body) = send(router, "/settings/models", "127.0.0.1:3000").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(
        headers[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("text/plain")
    );
    assert!(body.contains("web_dist_missing"));
}

#[tokio::test]
async fn mismatched_frontend_and_backend_revisions_refuse_the_page() {
    let dist = synthetic_dist(Some("aaaaaaa1"), true);
    let config = WebAppConfig::new(dist.clone(), "bbbbbbb2");
    assert_eq!(test_support::state_of(&config), "revision_mismatch");
    let health = test_support::health_of(&config);
    assert_eq!(health["state"], "revision_mismatch");
    assert_eq!(health["revision"], "aaaaaaa1");
    assert_eq!(health["runtimeRevision"], "bbbbbbb2");
    let (status, _, body) = send(web(dist, "bbbbbbb2"), "/settings/models", "127.0.0.1:3000").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(body.contains("web_revision_mismatch"));
}

#[test]
fn revisions_compare_only_when_both_sides_are_known() {
    assert!(test_support::revisions_match("284cd3a9", "284cd3a"));
    assert!(test_support::revisions_match("284CD3A9", "284cd3a9"));
    assert!(test_support::revisions_match("unknown", "284cd3a9"));
    assert!(test_support::revisions_match("284cd3a9", "unknown"));
    assert!(test_support::revisions_match("", "284cd3a9"));
    assert!(!test_support::revisions_match("284cd3a9", "473980cb"));
}

#[tokio::test]
async fn hashed_assets_are_long_cached_and_nothing_else_is_reachable() {
    let dist = synthetic_dist(Some("284cd3a9"), true);
    let (status, headers, body) = send(
        web(dist.clone(), "unknown"),
        "/ui-assets/index-AbC_123.js",
        "127.0.0.1:3000",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers[header::CONTENT_TYPE],
        "text/javascript; charset=utf-8"
    );
    assert_eq!(
        headers[header::CACHE_CONTROL],
        "public, max-age=31536000, immutable"
    );
    assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    assert!(body.contains("synthetic"));
    for blocked in [
        "/ui-assets/index.html",
        "/ui-assets/build-info.json",
        "/ui-assets/..%2Findex.html",
        "/ui-assets/%2E%2E%2Fbuild-info.json",
        "/ui-assets/missing-file.js",
    ] {
        let (status, _, _) = send(web(dist.clone(), "unknown"), blocked, "127.0.0.1:3000").await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{blocked}");
    }
    for rejected in [
        "a..js", ".js", "index", "index.JS", "a/b.js", "a\\b.js", "x.map",
    ] {
        assert_eq!(test_support::content_type_of(rejected), None, "{rejected}");
    }
    assert_eq!(
        test_support::content_type_of("font-Ab_1.woff2"),
        Some("font/woff2")
    );
}

#[tokio::test]
async fn legacy_settings_page_assets_are_retired_and_unknown_paths_stay_404() {
    for retired in [
        "/assets/model-settings.js",
        "/assets/model-settings.css",
        "/assets/collection-workspace.css",
        "/assets/collection-workspace.js",
        "/collection/operations",
        "/collection/attention?domain=all",
        "/collection/targets/new",
        "/collection/domains/save",
        "/collection/runtime/stations",
        "/settings/unknown",
    ] {
        let (status, _, _) = send(app(), retired, "127.0.0.1:3000").await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{retired}");
    }
    // 接口没有被页面入口吞掉：仍是接口自己的 JSON 回答。
    let (status, headers, body) = send(app(), "/api/local/model-settings", "127.0.0.1:3000").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(
        headers[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("application/json")
    );
    assert!(body.contains("model_database_unavailable"));
}

/// 阶段 D1：创作者页迁到 React。登记路径返回前端入口；旧页面的样式、脚本已删除（不留双轨）；
/// 接口仍由 Rust 精确提供，不被页面入口吞掉。
#[tokio::test]
async fn creators_page_is_a_registered_react_entry_and_its_native_assets_are_retired() {
    let domain = "00000000-0000-4000-8000-000000000001";
    let router = web(synthetic_dist(Some("284cd3a9"), true), "284cd3a9");
    let (status, headers, body) = send(
        router,
        &format!("/corpus/creators?domain={domain}&page=1"),
        "127.0.0.1:3000",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        headers[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("text/html")
    );
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    assert!(body.contains("<div id=\"root\">"));

    let foreign = web(synthetic_dist(None, true), "unknown");
    let (status, _, _) = send(foreign, "/corpus/creators", "evil.example:3000").await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // 真实路由表里同样登记了该路径（没有构建产物时是 503 说明，而不是 404）。
    let (status, _, _) = send(app(), "/corpus/creators", "127.0.0.1:3000").await;
    assert!(matches!(
        status,
        StatusCode::OK | StatusCode::SERVICE_UNAVAILABLE
    ));
    for retired in [
        "/assets/creators.css",
        "/assets/creators.js",
        "/corpus/creators/unknown",
    ] {
        let (status, _, _) = send(app(), retired, "127.0.0.1:3000").await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{retired}");
    }
    // 接口没有被页面入口吞掉：仍是接口自己的 JSON 回答。
    let (status, headers, body) = send(
        app(),
        &format!("/api/local/creators?domain={domain}"),
        "127.0.0.1:3000",
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(
        headers[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("application/json")
    );
    assert!(body.contains("read_model_not_connected"));
}

/// 阶段 D2：证据库迁到 React。登记路径返回前端入口（经本机来源闸门，插件与书签的直接打开仍可用）；
/// 旧页面的样式、脚本与底稿脚本全部退役（不留双轨）；`/corpus` 仍跳转过来；接口不被页面入口吞掉。
#[tokio::test]
async fn evidence_page_is_a_registered_react_entry_and_its_native_assets_are_retired() {
    let domain = "00000000-0000-4000-8000-000000000001";
    let work = "10000000-0000-4000-8000-000000000001";
    let uri = format!("/corpus/evidence?domain={domain}&work={work}&tab=dossier");
    let router = web(synthetic_dist(Some("284cd3a9"), true), "284cd3a9");
    let (status, headers, body) = send(router.clone(), &uri, "127.0.0.1:3000").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        headers[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("text/html")
    );
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    assert!(body.contains("<div id=\"root\">"));
    // 没有任何参数的入口（插件、登录页、书签）同样返回入口，不改写地址。
    let (status, _, _) = send(router, "/corpus/evidence", "localhost:3000").await;
    assert_eq!(status, StatusCode::OK);

    // 来源闸门：其他主机名、跨站发起的导航被拒；用户直接打开（插件 chrome.tabs.create、书签、地址栏）
    // 的 Sec-Fetch-Site 是 none，同源跳转是 same-origin，都放行。
    let foreign = web(synthetic_dist(None, true), "unknown");
    let (status, _, _) = send(foreign, "/corpus/evidence", "evil.example:3000").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    for (fetch_site, expected) in [
        ("cross-site", StatusCode::FORBIDDEN),
        ("same-site", StatusCode::FORBIDDEN),
        ("none", StatusCode::OK),
        ("same-origin", StatusCode::OK),
    ] {
        let response = web(synthetic_dist(Some("284cd3a9"), true), "284cd3a9")
            .oneshot(
                Request::builder()
                    .uri("/corpus/evidence")
                    .header(header::HOST, "127.0.0.1:3000")
                    .header("Sec-Fetch-Site", fetch_site)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), expected, "Sec-Fetch-Site: {fetch_site}");
    }

    // 真实路由表里同样登记了该路径（没有构建产物时是 503 说明，而不是 404）；`/corpus` 仍跳转到它。
    let (status, _, _) = send(app(), "/corpus/evidence", "127.0.0.1:3000").await;
    assert!(matches!(
        status,
        StatusCode::OK | StatusCode::SERVICE_UNAVAILABLE
    ));
    let (status, headers, _) = send(app(), "/corpus", "127.0.0.1:3000").await;
    assert_eq!(status, StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(headers[header::LOCATION], "/corpus/evidence");
    for retired in [
        "/assets/evidence-library.css",
        "/assets/evidence-library.js",
        "/assets/evidence-observation.js",
        "/assets/research-dossier.js",
        "/corpus/evidence/unknown",
    ] {
        let (status, _, _) = send(app(), retired, "127.0.0.1:3000").await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{retired}");
    }
    // 接口没有被页面入口吞掉：仍是接口自己的 JSON 回答。
    for api in [
        format!("/api/local/work-resources?domain={domain}"),
        format!("/api/local/work-resources/{work}?domain={domain}"),
    ] {
        let (status, headers, body) = send(app(), &api, "127.0.0.1:3000").await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{api}");
        assert!(
            headers[header::CONTENT_TYPE]
                .to_str()
                .unwrap()
                .starts_with("application/json")
        );
        assert!(body.contains("read_model_not_connected"), "{api}");
    }
}

#[tokio::test]
async fn health_reports_the_frontend_build() {
    let (status, _, body) = send(app(), "/health", "127.0.0.1:3000").await;
    assert_eq!(status, StatusCode::OK);
    let health: Value = serde_json::from_str(&body).unwrap();
    assert!(matches!(
        health["webApp"]["state"].as_str(),
        Some("ready" | "missing" | "revision_mismatch")
    ));
    assert_eq!(
        health["webApp"]["pages"],
        json!([
            "/settings/models",
            "/ops/creation",
            "/corpus/creators",
            "/corpus/evidence",
            "/collection/targets",
            "/collection/tasks",
            "/collection/domains",
            "/collection/runtime",
            "/ops/overview",
            "/ops/publications",
            "/ops/accounts",
            "/ops/daily",
            "/ops/io",
            "/topics",
            "/topics/{canonical_key}"
        ])
    );
}

#[tokio::test]
async fn workbench_entry_is_registered_and_ops_preserves_explicit_domain() {
    let domain = "00000000-0000-4000-8000-000000000001";
    let (status, headers, _) =
        send(app(), &format!("/ops?domain={domain}"), "127.0.0.1:3000").await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(
        headers[header::LOCATION],
        format!("/ops/creation?domain={domain}")
    );
    let (status, headers, _) = send(app(), "/ops", "127.0.0.1:3000").await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(headers[header::LOCATION], "/ops/creation");
    let (status, _, _) = send(app(), "/ops/creation", "127.0.0.1:3000").await;
    assert!(matches!(
        status,
        StatusCode::OK | StatusCode::SERVICE_UNAVAILABLE
    ));
    let (status, _, _) = send(app(), "/ops/creation/unknown", "127.0.0.1:3000").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, headers, _) = send(app(), "/ops/api/creation/works", "127.0.0.1:3000").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(
        headers[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("application/json")
    );
}

/// 真实链路：隔离 PostgreSQL + 本机 Axum 路由 + 已构建的前端 + 无界面浏览器走完设置页。
/// 只用合成模型凭据（`LINGGAN_MODEL_SYNTHETIC_PREVIEW`），连接地址固定在本机回环，不外发请求。
// 多线程：浏览器子进程运行期间，服务器任务必须能在其他线程上继续响应。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "需要隔离 PostgreSQL、已构建的 apps/web/dist 与 Node；由 scripts/test-web-app-postgres.sh 运行"]
async fn settings_page_real_chain_browser_proof() {
    assert_eq!(std::env::var("WEB_APP_BROWSER_PROOF").as_deref(), Ok("1"));
    assert_eq!(
        std::env::var("LINGGAN_MODEL_SYNTHETIC_PREVIEW").as_deref(),
        Ok("SYNTHETIC-NOT-EVIDENCE"),
        "浏览器证明只允许合成凭据"
    );
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").expect("proof URL is supplied");
    let schema = format!("web_app_proof_{}", uuid::Uuid::new_v4().simple());
    let database = linggan_storage_postgres::testing::isolated_proof_schema(
        &url,
        &schema,
        full_schema_fixture::FULL_MIGRATIONS,
    )
    .await
    .expect("migrations apply");
    let application = router(LocalWebState {
        database: LocalDatabaseState::Ready(Arc::new(database)),
        active_media_sessions: Arc::new(tokio::sync::Mutex::new(BTreeSet::new())),
        account_digest_key: None,
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, application).await.unwrap() });
    let directory = std::env::var("WEB_APP_PROOF_DIR").expect("evidence directory is supplied");
    fs::create_dir_all(&directory).unwrap();
    fs::write(
        FsPath::new(&directory).join("fixture.json"),
        json!({"origin": origin}).to_string(),
    )
    .unwrap();
    let node = std::env::var("WEB_APP_PROOF_NODE").expect("node binary is supplied");
    let script =
        FsPath::new(env!("CARGO_MANIFEST_DIR")).join("../web/test/e2e/settings-real-chain.mjs");
    let result = std::process::Command::new(node)
        .arg(script)
        .env("WEB_APP_PROOF_DIR", &directory)
        .status()
        .unwrap();
    server.abort();
    assert!(
        result.success(),
        "React settings browser proof failed; see the evidence directory"
    );
}

#[tokio::test]
async fn operations_five_pages_are_react_and_native_assets_are_retired() {
    for page in ["overview", "publications", "accounts", "daily", "io"] {
        let (status, _, body) = send(app(), &format!("/ops/{page}"), "127.0.0.1:3000").await;
        assert!(matches!(
            status,
            StatusCode::OK | StatusCode::SERVICE_UNAVAILABLE
        ));
        assert!(!body.contains("data-ops-page"));
    }
    for asset in ["ops.css", "ops.js", "ops-overview.js"] {
        let (status, _, _) = send(app(), &format!("/assets/{asset}"), "127.0.0.1:3000").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
    for uri in ["/ops/unknown", "/api/local/ops/unknown"] {
        let (status, _, _) = send(app(), uri, "127.0.0.1:3000").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}

#[tokio::test]
async fn topic_pages_use_react_keep_safe_keys_and_retire_native_assets() {
    let dist = synthetic_dist(Some("82d48a72"), true);
    for path in [
        "/topics",
        "/topics?domainRef=00000000-0000-4000-8000-000000000001",
        "/topics/task-initiation-difficulty",
    ] {
        let (status, headers, body) =
            send(web(dist.clone(), "82d48a72"), path, "127.0.0.1:3000").await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert!(body.contains("<div id=\"root\">"));
        assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    }
    for key in ["a", "Bad", "a.b", "中文", "a%2Fb"] {
        let (status, _, _) = send(
            web(dist.clone(), "unknown"),
            &format!("/topics/{key}"),
            "localhost:3000",
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{key}");
    }
    for retired in [
        "/assets/topic-map.css",
        "/assets/topic-map.js",
        "/assets/topic-workspace.css",
        "/assets/topic-workspace.js",
        "/topics/key/unregistered",
        "/api/local/topic-map/unknown",
    ] {
        let (status, _, _) = send(app(), retired, "localhost:3000").await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{retired}");
    }
    let (status, _, _) = send(web(dist, "unknown"), "/topics", "evil.example:3000").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, headers, body) = send(
        app(),
        "/api/local/topic-workspaces/task-initiation-difficulty",
        "localhost:3000",
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(
        headers[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("application/json")
    );
    assert!(body.contains("topic_read_model_not_connected"));
}
