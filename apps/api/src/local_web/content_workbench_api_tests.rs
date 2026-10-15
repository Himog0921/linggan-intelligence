//! Real guarded HTTP composition against synthetic, isolated PostgreSQL.
use super::*;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use linggan_intelligence::content_workbench::{self as core, types::*};
use tower::ServiceExt;
use uuid::Uuid;
#[path = "../../../../crates/evidence/tests/support/research_dossier_fixture.rs"]
mod dossier;
async fn proof(name: &str) -> Database {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    linggan_storage_postgres::testing::isolated_proof_schema(
        &url,
        name,
        full_schema_fixture::FULL_MIGRATIONS,
    )
    .await
    .unwrap()
}
async fn call(
    app: Router,
    uri: &str,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let mut b = Request::builder().uri(uri).header("Host", "127.0.0.1:4185");
    if body.is_some() {
        b = b.method("POST").header("Content-Type", "application/json");
    }
    let response = app
        .oneshot(
            b.body(body.map_or_else(Body::empty, |v| Body::from(v.to_string())))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 3 * 1024 * 1024)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or_else(|_| json!({"rawLength":bytes.len()})),
    )
}
#[tokio::test]
#[ignore = "isolated PostgreSQL proof; no shared database"]
async fn content_workbench_http_routes_save_conflict_history_and_guard() {
    let db = proof("creation_http").await;
    let app = app_with_database(db);
    let domain = dossier::DOMAIN;
    let request =
        json!({"requestId":Uuid::new_v4(),"domainRef":domain,"document":WorkDocument::default()});
    let (status, first) = call(
        app.clone(),
        "/ops/api/creation/works",
        Some(request.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let id = first["workId"].as_str().unwrap();
    let uri = format!("/ops/api/creation/works/{id}");
    let (status, read) = call(app.clone(), &uri, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(read["domainRef"], json!(domain));
    let mut doc = read["revision"]["document"].clone();
    doc["title"] = json!("真实HTTP保存🙂");
    let command = json!({"requestId":Uuid::new_v4(),"expectedRevision":read["revision"]["id"],"document":doc});
    let (status, saved) = call(app.clone(), &format!("{uri}/save"), Some(command.clone())).await;
    assert_eq!(status, StatusCode::OK);
    let (status, receipt) = call(
        app.clone(),
        &format!(
            "/ops/api/creation/requests/{}",
            command["requestId"].as_str().unwrap()
        ),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(receipt["kind"], "work");
    assert_eq!(
        receipt["receipt"]["revision"]["id"],
        saved["revision"]["id"]
    );
    let (status, _) = call(
        app.clone(),
        &format!("/ops/api/creation/requests/{}", Uuid::new_v4()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, replay) = call(app.clone(), &format!("{uri}/save"), Some(command.clone())).await;
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["revision"]["id"], saved["revision"]["id"]);
    let mut stale = command;
    stale["requestId"] = json!(Uuid::new_v4());
    let (status, conflict) = call(app.clone(), &format!("{uri}/save"), Some(stale)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        conflict["receipt"]["revision"]["id"],
        saved["revision"]["id"]
    );
    let (status, history) = call(app.clone(), &format!("{uri}/revisions"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(history["items"].as_array().unwrap().len(), 2);
    let (status, list) = call(
        app.clone(),
        &format!("/ops/api/creation/works?domainRef={domain}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list["items"].as_array().unwrap().len(), 1);
    let bad = Request::builder()
        .uri("/ops/api/creation/works")
        .header("Host", "127.0.0.1:4185")
        .header("Origin", "https://evil.example")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app.oneshot(bad).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof; no shared database"]
async fn content_workbench_http_denies_unknown_fields_and_nonexistent_domain() {
    let db = proof("creation_http_contract").await;
    let app = app_with_database(db);
    let document = WorkDocument::default();
    let request =
        json!({"requestId":Uuid::new_v4(),"domainRef":Uuid::new_v4(),"document":document});
    let (status, _) = call(app.clone(), "/ops/api/creation/works", Some(request)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let request = json!({"requestId":Uuid::new_v4(),"domainRef":dossier::DOMAIN,"document":document,"serverOverride":true});
    let (status, _) = call(app, "/ops/api/creation/works", Some(request)).await;
    assert!(status.is_client_error());
}
