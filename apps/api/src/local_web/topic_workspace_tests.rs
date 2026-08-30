use super::*;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use linggan_storage_postgres::testing::isolated_proof_schema;
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

#[tokio::test]
async fn topic_page_names_its_provisional_and_not_connected_boundaries() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/topics/task-initiation-difficulty")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();
    for required in [
        "暂定主题",
        "PROVISIONAL TOPIC",
        "人工裁定",
        "来源边界",
        "当前未读取 Topic",
        "data-topic-workspace",
    ] {
        assert!(
            html.contains(required),
            "missing Topic boundary: {required}"
        );
    }
    assert!(!html.contains("已验证趋势"));
    assert!(!html.contains("正式主题"));
}

#[tokio::test]
async fn topic_assets_are_served_as_dedicated_lids_consumers() {
    for (path, content_type, required) in [
        (
            "/assets/topic-workspace.css",
            "text/css; charset=utf-8",
            ".topic-workspace",
        ),
        (
            "/assets/topic-workspace.js",
            "text/javascript; charset=utf-8",
            "data-topic-workspace",
        ),
    ] {
        let response = app()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            content_type
        );
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert!(String::from_utf8(body.to_vec()).unwrap().contains(required));
    }
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn topic_import_and_read_compose_exact_work_resources_without_copying_them() {
    let database = proof_database("topic_api_exact_pack").await;
    let support_ref = admit_work_identity(&database, "topic-api-support", 'a').await;
    let challenge_ref = admit_work_identity(&database, "topic-api-challenge", 'b').await;
    let application = app_with_database(database);
    let request = topic_request(support_ref, challenge_ref);

    let response = application
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/local/topic-workspaces")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(request.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let receipt: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(receipt.pointer("/definitionVersion"), Some(&json!(1)));

    let response = application
        .oneshot(
            Request::builder()
                .uri("/api/local/topic-workspaces/task-initiation-difficulty")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        payload.pointer("/topic/definition/lifecycleState"),
        Some(&json!("provisional"))
    );
    assert_eq!(
        payload.pointer("/topic/classificationRun/runKind"),
        Some(&json!("human_adjudicated"))
    );
    assert_eq!(
        payload.pointer("/materials/0/classification/workPublicRef"),
        Some(&json!(support_ref))
    );
    assert_eq!(
        payload.pointer("/materials/1/classification/workPublicRef"),
        Some(&json!(challenge_ref))
    );
    assert_eq!(
        payload.pointer("/materials/0/workResource/identity/publicRef"),
        Some(&json!(support_ref))
    );
    assert!(
        payload
            .pointer("/materials/0/workResource/display/bodyText")
            .is_none()
    );
}

async fn proof_database(schema: &str) -> Database {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL")
        .expect("test script must provide the isolated proof database URL");
    isolated_proof_schema(&url, schema, full_schema_fixture::FULL_MIGRATIONS)
        .await
        .expect("isolated migration applies")
}

async fn admit_work_identity(database: &Database, external_id: &str, seed: char) -> Uuid {
    let task_id = Uuid::new_v4();
    let attempt_id = Uuid::new_v4();
    let package_ref = Uuid::new_v4();
    let public_ref = Uuid::new_v4();
    let hash: String = std::iter::repeat_n(seed, 64).collect();
    sqlx::query(
        "INSERT INTO linggan_runtime_task \
         (task_id,task_spec_hash,task_spec,source,platform,page_type) \
         VALUES($1,$2,'{}','manual','xhs','topic_proof')",
    )
    .bind(task_id)
    .bind(&hash)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_runtime_attempt(attempt_id,task_id,producer_instance_id) \
         VALUES($1,$2,$3)",
    )
    .bind(attempt_id)
    .bind(task_id)
    .bind(Uuid::new_v4())
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_runtime_capture_package \
         (package_ref,attempt_id,task_id,producer_instance_id,package_kind,platform,package_hash, \
          observed_at,captured_at,coverage,payload) \
         SELECT $1,$2,$3,producer_instance_id,'content_detail','xhs',$4, \
          '2026-08-31T00:00:00Z','2026-08-31T00:00:00Z','{}','{}' \
         FROM linggan_runtime_attempt WHERE attempt_id=$2",
    )
    .bind(package_ref)
    .bind(attempt_id)
    .bind(task_id)
    .bind(&hash)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_material_content(platform,content_external_id,public_ref,first_package_ref) \
         VALUES('xhs',$1,$2,$3)",
    )
    .bind(external_id)
    .bind(public_ref)
    .bind(package_ref)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_runtime_record_disposition \
         (package_ref,record_ordinal,disposition,reason) \
         VALUES($1,0,'accepted_for_library_content','topic proof fixture')",
    )
    .bind(package_ref)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_material_content_detail \
         (material_ref,content_public_ref,package_ref,record_ordinal,observed_at,title,title_state, \
          body_text,body_state,creator_display_name,creator_display_name_state, \
          published_at_source_text,published_at_source_text_state,searchable_text,author_external_id) \
         VALUES($1,$2,$3,0,'2026-08-31T00:00:00Z',$4,'KNOWN',NULL,'UNKNOWN', \
          '研究样本','KNOWN',NULL,'UNKNOWN',$4,'topic-proof-author')",
    )
    .bind(Uuid::new_v4())
    .bind(public_ref)
    .bind(package_ref)
    .bind(format!("Topic proof {external_id}"))
    .execute(database.pool())
    .await
    .unwrap();
    public_ref
}

fn topic_request(support_ref: Uuid, challenge_ref: Uuid) -> Value {
    json!({
        "idempotencyKey":"topic-api:exact-pack:0001",
        "domainKey":"adhd-family",
        "canonicalKey":"task-initiation-difficulty",
        "displayName":"任务启动困难",
        "definitionText":"在有明确意图时仍难以开始第一步的可观察困难。",
        "expectedVersion":null,
        "adjudicationNote":"研究者按定义逐条裁定。",
        "sourceBoundary":"仅含两条已接纳 Work Resource，不代表总体分布。",
        "members":[
            {"workPublicRef":support_ref,"role":"support","rationale":"支持材料"},
            {"workPublicRef":challenge_ref,"role":"challenge","rationale":"挑战材料"}
        ]
    })
}
