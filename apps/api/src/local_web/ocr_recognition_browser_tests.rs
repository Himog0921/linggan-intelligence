//! Browser proof uses only the owned schema seeded by the worker integration test.
use std::path::PathBuf;
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "owned preseeded PostgreSQL, loopback mock and built React only"]
async fn owned_manual_ocr_real_ui() {
    let uri = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    assert!(uri.contains("@127.0.0.1:") && uri.contains("/linggan_intelligence_ocr_"));
    let directory = PathBuf::from(std::env::var_os("OCR_AUDIT_PROOF_OUTPUT").unwrap());
    let mut fixture: serde_json::Value =
        serde_json::from_slice(&std::fs::read(directory.join("recognition-fixture.json")).unwrap())
            .unwrap();
    assert_eq!(fixture["schema"], "ocr_recognition_owned");
    let db =
        linggan_storage_postgres::Database::connect_within_schema(&uri, "ocr_recognition_owned")
            .await
            .unwrap();
    let release = directory.join("release-second");
    let app = super::app_with_database(db).route(
        "/__ocr-proof/release",
        axum::routing::post(move || {
            let path = release.clone();
            async move {
                std::fs::write(path, b"explicit browser observation complete").unwrap();
                axum::Json(serde_json::json!({"released":true}))
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    fixture["origin"] = serde_json::json!(format!("http://{}", listener.local_addr().unwrap()));
    std::fs::write(
        directory.join("recognition-fixture.json"),
        fixture.to_string(),
    )
    .unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let mut node =
        tokio::process::Command::new(std::env::var_os("AI_FOUNDATION_PROOF_NODE").unwrap())
            .arg(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../web/test/e2e/ocr-recognition-real-chain.mjs"),
            )
            .kill_on_drop(true)
            .spawn()
            .unwrap();
    let status = tokio::time::timeout(std::time::Duration::from_secs(75), node.wait()).await;
    if status.is_err() {
        let _ = node.kill().await;
        let _ = node.wait().await;
    }
    server.abort();
    let _ = server.await;
    assert!(
        status
            .expect("owned browser proof timed out")
            .unwrap()
            .success()
    );
}
