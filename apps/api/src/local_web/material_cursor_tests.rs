use super::*;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use linggan_storage_postgres::testing::isolated_proof_schema;
use tower::ServiceExt;

const MIGRATIONS: &str = concat!(
    "CREATE TABLE linggan_local_schema_migration (migration_id text PRIMARY KEY, migration_sha256 text NOT NULL, applied_at timestamptz NOT NULL DEFAULT clock_timestamp());\n",
    include_str!("../../../../database/migrations/0001_scope_001_capture_evidence.sql"),
    "\n",
    include_str!("../../../../database/migrations/0002_local_001_discovery.sql"),
    "\n",
    include_str!("../../../../database/migrations/0003_local_trusted_producer.sql"),
    "\n",
    include_str!("../../../../database/migrations/0004_plugin_runtime_all_capabilities.sql"),
    "\n",
    include_str!("../../../../database/migrations/0005_collection_observation_target.sql"),
    "\n",
    include_str!("../../../../database/migrations/0006_collection_acquisition_chain.sql"),
    "\n",
    include_str!("../../../../database/migrations/0007_execution_station.sql"),
    "\n",
    include_str!("../../../../database/migrations/0015_material_projection.sql"),
    "\n",
    include_str!("../../../../database/migrations/0016_material_social_lanes.sql"),
    "\n",
    include_str!("../../../../database/migrations/0017_material_media_projection.sql"),
    "\n",
    include_str!("../../../../database/migrations/0018_material_discovery_lane.sql"),
    "\n",
    include_str!("../../../../database/migrations/0020_observation_runtime_automation.sql"),
    "\n",
    include_str!("../../../../database/migrations/0021_discovery_cover_media_acquisition.sql"),
    "\n",
    include_str!(
        "../../../../database/migrations/0023_material_engagement_and_media_components.sql"
    ),
    "\n",
    include_str!("../../../../database/migrations/0024_media_processing_runtime.sql"),
    "\n",
    include_str!("../../../../database/migrations/0025_work_resource_read.sql"),
    "\n",
    "INSERT",
    " INTO linggan_local_schema_migration (migration_id,migration_sha256) VALUES \
      ('0001_scope_001_capture_evidence','1'),('0002_local_001_discovery','2'), \
      ('0003_local_trusted_producer','3'),('0004_plugin_runtime_all_capabilities','4'), \
      ('0015_material_projection','15'),('0016_material_social_lanes','16'), \
      ('0017_material_media_projection','17'),('0018_material_discovery_lane','18'),('0020_observation_runtime_automation','20');\n",
);

const LEGACY_MIGRATIONS: &str = concat!(
    "CREATE TABLE linggan_local_schema_migration (migration_id text PRIMARY KEY, migration_sha256 text NOT NULL, applied_at timestamptz NOT NULL DEFAULT clock_timestamp());\n",
    include_str!("../../../../database/migrations/0001_scope_001_capture_evidence.sql"),
    "\n",
    include_str!("../../../../database/migrations/0002_local_001_discovery.sql"),
    "\n",
    include_str!("../../../../database/migrations/0003_local_trusted_producer.sql"),
    "\n",
    include_str!("../../../../database/migrations/0004_plugin_runtime_all_capabilities.sql"),
    "\n",
    "INSERT",
    " INTO linggan_local_schema_migration (migration_id,migration_sha256) VALUES ('0001_scope_001_capture_evidence','1'),('0002_local_001_discovery','2'),('0003_local_trusted_producer','3'),('0004_plugin_runtime_all_capabilities','4');\n",
);

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn legacy_schema_validates_cursor_before_refusing_an_unfulfillable_page() {
    let current = proof_database("material_cursor_source").await;
    seed_details(&current, 0, 55, false).await;
    let first = get_json(&current, "/api/local/work-resources").await;
    let cursor = first.pointer("/cursor").and_then(Value::as_str).unwrap();
    let future = cursor_with_future_as_of(cursor);
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").expect("proof URL is supplied");
    let legacy = isolated_proof_schema(&url, "material_cursor_legacy", LEGACY_MIGRATIONS)
        .await
        .expect("legacy migrations apply");

    for uri in [
        "/api/local/work-resources?cursor=garbage".to_owned(),
        format!("/api/local/work-resources?q=mismatch&cursor={cursor}"),
        format!("/api/local/work-resources?cursor={future}"),
        "/api/local/work-resources?sort=relevance".to_owned(),
    ] {
        assert_eq!(get_status(&legacy, &uri).await, StatusCode::BAD_REQUEST);
    }
    let response = app_with_database(legacy)
        .oneshot(
            Request::builder()
                .uri(format!("/api/local/work-resources?cursor={cursor}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        payload.pointer("/operation").and_then(Value::as_str),
        Some("local_read")
    );
    assert_eq!(
        payload.pointer("/outcome").and_then(Value::as_str),
        Some("unavailable")
    );
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn cursor_freezes_as_of_rejects_corruption_and_avoids_cross_page_duplicates() {
    let database = proof_database("material_cursor_watermark").await;
    seed_details(&database, 0, 55, false).await;
    let first = get_json(&database, "/api/local/work-resources").await;
    assert_eq!(
        first
            .pointer("/items")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(50)
    );
    assert_eq!(
        first.pointer("/truncated").and_then(Value::as_bool),
        Some(true)
    );
    let cursor = first.pointer("/cursor").and_then(Value::as_str).unwrap();
    let as_of = first.pointer("/asOf").and_then(Value::as_str).unwrap();
    let first_ids = item_ids(&first);

    seed_details(&database, 100, 1, false).await;
    let second = get_json(
        &database,
        &format!("/api/local/work-resources?cursor={cursor}"),
    )
    .await;
    assert_eq!(second.pointer("/asOf").and_then(Value::as_str), Some(as_of));
    assert!(
        second.get("cards").is_none(),
        "Material cursor pages must never contain legacy cards"
    );
    assert_eq!(
        second
            .pointer("/items")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(5)
    );
    assert_eq!(
        second.pointer("/truncated").and_then(Value::as_bool),
        Some(false)
    );
    let second_ids = item_ids(&second);
    assert!(first_ids.is_disjoint(&second_ids));
    assert!(!second_ids.contains("note-cursor-100"));

    let mut corrupted = cursor.to_owned();
    let last = corrupted.pop().unwrap();
    corrupted.push(if last == '0' { '1' } else { '0' });
    assert_eq!(
        get_status(
            &database,
            &format!("/api/local/work-resources?cursor={corrupted}")
        )
        .await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        get_status(
            &database,
            &format!("/api/local/work-resources?q=different&cursor={cursor}")
        )
        .await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        get_status(&database, "/api/local/work-resources?sort=relevance").await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        get_status(
            &database,
            "/api/local/work-resources?window=last_7_days&cursor=garbage"
        )
        .await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        get_status(
            &database,
            &format!("/api/local/work-resources?window=last_7_days&cursor={cursor}")
        )
        .await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        get_status(
            &database,
            &format!(
                "/api/local/work-resources?cursor={}",
                cursor_with_future_as_of(cursor)
            )
        )
        .await,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn post_enrichment_filter_scans_past_non_matches_until_the_page_is_exhausted() {
    let database = proof_database("material_cursor_filter_fill").await;
    seed_details(&database, 0, 58, true).await;
    let payload = get_json(&database, "/api/local/work-resources?lane=author").await;
    assert_eq!(
        payload
            .pointer("/items")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(
        payload.pointer("/truncated").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        item_ids(&payload),
        ["note-cursor-000".to_owned(), "note-cursor-001".to_owned()].into()
    );
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn a_late_accepted_older_observation_does_not_replace_the_latest_observed_detail() {
    let database = proof_database("material_observation_order").await;
    seed_details(&database, 0, 1, false).await;
    seed_older_detail_version(&database).await;
    let payload = get_json(&database, "/api/local/work-resources").await;
    assert_eq!(
        payload
            .pointer("/items/0/display/title")
            .and_then(Value::as_str),
        Some("cursor item 0")
    );
    assert_eq!(
        payload
            .pointer("/items/0/summary/lastObservedAt")
            .and_then(Value::as_str),
        Some("2026-08-28T10:00:00Z")
    );
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn scan_budget_returns_an_honest_continuation_without_rescanning_non_matches() {
    let database = proof_database("material_cursor_scan_budget").await;
    seed_details(&database, 0, 205, false).await;
    let first = get_json(
        &database,
        "/api/local/work-resources?restriction=WITHDRAWN_OR_RESTRICTED",
    )
    .await;
    assert_eq!(
        first.pointer("/scanLimited").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        first.pointer("/scannedCount").and_then(Value::as_u64),
        Some(200)
    );
    assert_eq!(
        first
            .pointer("/items")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );
    let cursor = first.pointer("/cursor").and_then(Value::as_str).unwrap();
    let second = get_json(
        &database,
        &format!("/api/local/work-resources?restriction=WITHDRAWN_OR_RESTRICTED&cursor={cursor}"),
    )
    .await;
    assert_eq!(
        second.pointer("/scanLimited").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        second.pointer("/scannedCount").and_then(Value::as_u64),
        Some(5)
    );
    assert_eq!(second.pointer("/cursor"), Some(&Value::Null));
}

fn cursor_with_future_as_of(cursor: &str) -> String {
    let mut parts = cursor.split('.');
    assert_eq!(parts.next(), Some("m1"));
    let encoded = parts.next().unwrap();
    let bytes = (0..encoded.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&encoded[index..index + 2], 16).unwrap())
        .collect::<Vec<_>>();
    let mut payload: Value = serde_json::from_slice(&bytes).unwrap();
    payload["asOf"] = json!("2999-01-01T00:00:00Z");
    let encoded = serde_json::to_vec(&payload)
        .unwrap()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let checksum = Sha256::digest(format!("material-keyset-v1:{encoded}").as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("m1.{encoded}.{checksum}")
}

async fn seed_older_detail_version(database: &Database) {
    submit_detail(
        database,
        "note-cursor-000",
        "late accepted older title",
        "2026-08-28T09:00:00Z",
        None,
    )
    .await;
}

async fn seed_details(database: &Database, start: usize, count: usize, authors_on_first_two: bool) {
    for index in start..start + count {
        let content_id = format!("note-cursor-{index:03}");
        let observed_at = format!("2026-08-28T10:{:02}:{:02}Z", (index / 60) % 60, index % 60);
        let author_id = (authors_on_first_two && index < 2).then(|| format!("author-{index}"));
        submit_detail(
            database,
            &content_id,
            &format!("cursor item {index}"),
            &observed_at,
            author_id.as_deref(),
        )
        .await;
        if let Some(author_id) = author_id.as_deref() {
            submit_package(
                database,
                "author_profile",
                json!({"authorExternalId":author_id}),
                json!({
                    "kind":"author_profile",
                    "sourceObject":{"platform":"xhs","type":"author","externalId":author_id},
                    "payload":{"userId":author_id,"nickname":author_id}
                }),
                &observed_at,
            )
            .await;
        }
    }
}

async fn submit_detail(
    database: &Database,
    content_id: &str,
    title: &str,
    observed_at: &str,
    author_id: Option<&str>,
) {
    let mut payload = json!({"title":title});
    if let Some(author_id) = author_id {
        payload["authorId"] = json!(author_id);
    }
    submit_package(
        database,
        "content_detail",
        json!({"contentExternalId":content_id}),
        json!({
            "kind":"content_detail",
            "sourceObject":{"platform":"xhs","type":"content","externalId":content_id},
            "payload":payload
        }),
        observed_at,
    )
    .await;
}

async fn submit_package(
    database: &Database,
    capability: &str,
    target: Value,
    record: Value,
    observed_at: &str,
) {
    let task_id = uuid::Uuid::new_v4();
    let producer_instance_id = uuid::Uuid::new_v4();
    let attempt_id = uuid::Uuid::new_v4();
    let task = json!({
        "contractVersion":"linggan.producer.task-spec.v1","taskId":task_id,"source":"manual",
        "platform":"xhs","pageType":"cursor_proof","target":target.clone(),
        "capabilitiesRequested":[capability],"maximumQuota":1,
        "commentLimit":"not_requested","acquireMedia":"not_requested",
        "riskPolicy":"local_trusted_user_initiated","stopConditions":["maximum_quota"]
    });
    create_producer_task(
        database,
        &parse_producer_task_spec(&task.to_string()).unwrap(),
    )
    .await
    .unwrap();
    let attempt = json!({
        "contractVersion":"linggan.producer.attempt.v1",
        "producerInstanceId":producer_instance_id,"taskId":task_id,"attemptId":attempt_id
    });
    start_producer_attempt(
        database,
        &parse_producer_attempt(&attempt.to_string()).unwrap(),
    )
    .await
    .unwrap();
    let package = json!({
        "contractVersion":"linggan.producer.capture-package.v1",
        "packageRef":uuid::Uuid::new_v4(),"packageKind":capability,"platform":"xhs",
        "observedAt":observed_at,"capturedAt":observed_at,
        "coverage":{"target":target,"layers":[{
            "capability":capability,"observed":1,"attempted":1,"acquired":1,
            "verified":0,"failed":0,"notAttempted":0,"unknown":0,"stoppedReason":"fixture_complete"
        }]},
        "records":[record]
    });
    let submission = json!({
        "contractVersion":"linggan.producer.capture-package.v1",
        "producerInstanceId":producer_instance_id,"taskId":task_id,"attemptId":attempt_id,
        "submissionId":uuid::Uuid::new_v4(),"capturePackage":package
    });
    submit_producer_package(
        database,
        &parse_producer_submission(&submission.to_string()).unwrap(),
    )
    .await
    .unwrap();
}

async fn get_json(database: &Database, uri: &str) -> Value {
    let response = app_with_database(database.clone())
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
}

async fn get_status(database: &Database, uri: &str) -> StatusCode {
    app_with_database(database.clone())
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap()
        .status()
}

fn item_ids(payload: &Value) -> std::collections::BTreeSet<String> {
    payload
        .pointer("/items")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .filter_map(|item| {
            item.pointer("/identity/contentExternalId")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .collect()
}

async fn proof_database(schema: &str) -> Database {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").expect("proof URL is supplied");
    isolated_proof_schema(&url, schema, MIGRATIONS)
        .await
        .expect("migrations apply")
}
