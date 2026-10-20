//! The actual app with NotConfigured database state: no DSN, SQL, provider or worker.
use super::*;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use tower::ServiceExt;
use uuid::Uuid;

async fn call(body: Option<Value>, origin: Option<&str>) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .uri("/api/local/ai-offline-preview")
        .header("Host", "127.0.0.1:4100");
    if let Some(origin) = origin {
        request = request.header("Origin", origin);
    }
    if body.is_some() {
        request = request
            .method("POST")
            .header("Content-Type", "application/json");
    }
    let response = app()
        .oneshot(
            request
                .body(body.map_or_else(Body::empty, |value| Body::from(value.to_string())))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response
            .headers()
            .get("cache-control")
            .map(|v| v.to_str().unwrap()),
        Some("no-store")
    );
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1048576).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn offline_preview_real_router_is_stateless_without_database_and_rejects_untrusted_scope() {
    let (status, baseline) = call(None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(baseline["origin"], "offline_mock");
    assert_eq!(baseline["summary"]["total"], 6);
    let judgment = json!({"origin":"offline_mock", "identity":baseline["notes"][0]["input"]["identity"], "relevance":"related", "primaryDirection":baseline["catalogue"]["directions"][0]["directionRef"], "alsoRelated":null, "reason":null});
    let (status, carried) = call(
        Some(json!({"scenario":"candidate", "judgments":[judgment.clone()]})),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(carried["notes"][0]["result"]["needsReview"], true);
    assert_eq!(carried["notes"][0]["result"]["judgment"], judgment);
    assert_eq!(carried["notes"][0]["result"]["manualApplied"], false);
    assert_eq!(
        call(None, None).await.1,
        baseline,
        "POST calculations have no persisted state"
    );
    let mut forged = judgment.clone();
    forged["identity"]["workRef"] = Uuid::new_v4().to_string().into();
    for body in [
        json!({"scenario":"candidate","judgments":[forged]}),
        json!({"scenario":"baseline","judgments":[judgment.clone(), judgment]}),
        json!({"scenario":"provider","judgments":[]}),
        json!({"scenario":"baseline","judgments":[],"run":true}),
    ] {
        assert_eq!(call(Some(body), None).await.0, StatusCode::BAD_REQUEST);
    }
    // The guard rejects before no-store is added, so exercise it directly here.
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/api/local/ai-offline-preview")
                .header("Host", "127.0.0.1:4100")
                .header("Origin", "https://untrusted.example")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "built React and owned loopback browser only; no database or provider"]
async fn offline_preview_browser_main_path() {
    let directory = PathBuf::from(std::env::var("C0_PREVIEW_PROOF_DIR").unwrap());
    fs::create_dir_all(&directory).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app()).await.unwrap() });
    fs::write(
        directory.join("fixture.json"),
        json!({"origin":origin,"database":"NotConfigured","provider":"none","worker":"none"})
            .to_string(),
    )
    .unwrap();
    let result = tokio::task::spawn_blocking(move || {
        std::process::Command::new(std::env::var("C0_PREVIEW_NODE").unwrap())
            .arg(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../web/test/e2e/offline-preview.mjs"),
            )
            .env("C0_PREVIEW_PROOF_DIR", &directory)
            .status()
    })
    .await;
    server.abort();
    let _ = server.await;
    assert!(
        result.unwrap().unwrap().success(),
        "inspect the C0 browser report"
    );
}
