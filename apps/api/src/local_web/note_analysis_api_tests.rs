//! Real application routing with an owned PostgreSQL schema and synthetic source text.
//! This proves blocked/manual HTTP behavior, not successful model execution or quality.
use super::*;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use tower::ServiceExt;
use uuid::Uuid;

#[path = "../../../../crates/evidence/tests/support/research_dossier_fixture.rs"]
mod dossier;

async fn call(
    app: Router,
    uri: &str,
    body: Option<serde_json::Value>,
    cross_site: bool,
) -> (StatusCode, serde_json::Value) {
    let mut builder = Request::builder().uri(uri).header("Host", "127.0.0.1:4185");
    if cross_site {
        builder = builder.header("Origin", "https://untrusted.example");
    }
    if body.is_some() {
        builder = builder
            .method("POST")
            .header("Content-Type", "application/json");
    }
    let response = app
        .oneshot(
            builder
                .body(body.map_or_else(Body::empty, |value| Body::from(value.to_string())))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    if !cross_site {
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-store"
        );
    }
    let bytes = to_bytes(response.into_body(), 1 << 20).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
#[ignore = "owned PostgreSQL proof; synthetic material, no model dispatch"]
async fn note_analysis_http_blocked_preview_and_local_guard() {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").expect("owned proof URL");
    let db = linggan_storage_postgres::testing::isolated_proof_schema(
        &url,
        "note_analysis_http_a",
        full_schema_fixture::FULL_MIGRATIONS,
    )
    .await
    .unwrap();
    let work = dossier::work(&db).await;
    let app = app_with_database(db.clone());
    let root = "/api/local/note-analysis";
    let selection = format!("domainRef={}&workRef={work}&scope=note", dossier::DOMAIN);
    let preview_path = format!("{root}/preview?{selection}");
    let latest_path = format!("{root}/latest?{selection}");
    let request_path = format!("{root}/{}?domainRef={}", Uuid::new_v4(), dossier::DOMAIN);

    let (status, preview) = call(app.clone(), &preview_path, None, false).await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["contract"], "note-analysis.preview.v1");
    assert_eq!(preview["domainRef"], json!(dossier::DOMAIN));
    assert_eq!(preview["workRef"], json!(work));
    assert_eq!(preview["scope"], "note");
    assert_eq!(preview["canStart"], false);
    assert!(!preview["reasons"].as_array().unwrap().is_empty());
    assert!(
        preview["reasons"]
            .as_array()
            .unwrap()
            .iter()
            .all(Value::is_string)
    );
    assert_eq!(preview["coverage"]["includedCommentCount"], 0);
    assert_eq!(preview["coverage"]["authorCommentCount"], 0);
    assert!(preview["coverage"]["platformCommentCount"].is_null());
    assert!(preview["inputHash"].is_string());
    assert!(preview.get("task_config_ref").is_none());
    let request = json!({
        "commandRef": Uuid::new_v4(), "domainRef": dossier::DOMAIN, "workRef": work,
        "scope": "note", "knowledgeCutoff": preview["knowledgeCutoff"],
        "inputHash": preview["inputHash"], "taskConfigRef": preview["taskConfigRef"],
        "taskRevision": preview["taskRevision"],
    });
    let (status, rejected) = call(app.clone(), root, Some(request.clone()), false).await;
    assert_eq!(status, StatusCode::CONFLICT, "{rejected}");
    assert_eq!(rejected["error"], "ai_admission_denied");
    assert!(rejected.get("requestRef").is_none());

    let mut stale = request.clone();
    stale["commandRef"] = json!(Uuid::new_v4());
    stale["inputHash"] = json!("changed-input");
    assert_eq!(
        call(app.clone(), root, Some(stale), false).await.0,
        StatusCode::CONFLICT
    );
    let (status, latest) = call(app.clone(), &latest_path, None, false).await;
    assert_eq!(status, StatusCode::OK, "{latest}");
    assert_eq!(
        latest,
        json!({"contract":"note-analysis.latest.v1","latestRun":null,"latestSuccessful":null})
    );
    let (status, missing) = call(app.clone(), &request_path, None, false).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{missing}");
    assert!(missing["error"].is_string());

    for (uri, body) in [
        (preview_path, None),
        (latest_path, None),
        (request_path, None),
        (root.to_owned(), Some(request)),
    ] {
        let (status, rejected) = call(app.clone(), &uri, body, true).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{uri}: {rejected}");
        assert_eq!(rejected["error"], "local_comment_study_origin_rejected");
    }
    let requests: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_note_analysis_request")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let invocations: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!((requests, invocations), (0, 0));
}
