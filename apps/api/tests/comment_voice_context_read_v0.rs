use std::sync::Arc;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use linggan_api::comment_research_router_v0;
use linggan_contracts::CapturePackageV0;
use linggan_storage_postgres::CommentFactStore;
use serde_json::{Value, json};
use tokio_postgres::{Client, NoTls};
use tower::ServiceExt;

const CONTEXT_FIXTURE: &str =
    include_str!("../../../fixtures/xhs/comment-context-evidence-set-v0.json");
const PAIR_FIXTURE: &str = include_str!("../../../fixtures/xhs/comment-evidence-set-v1.json");
const WORKSPACE: &str = "voice-context-api-proof-workspace";

/// This proof creates a disposable PostgreSQL instance through its companion
/// script. It neither reads nor writes any configured local runtime database.
#[tokio::test]
#[ignore = "requires scripts/prove-comment-voice-context-read-api-v0.sh"]
async fn proves_user_voice_context_read_api_against_isolated_postgres() {
    let database_url = std::env::var("LINGGAN_COMMENT_VOICE_CONTEXT_API_TEST_DATABASE_URL")
        .expect("proof script must provide an isolated PostgreSQL database URL");

    let mut store = CommentFactStore::connect(&database_url)
        .await
        .expect("isolated proof database must connect");
    store
        .apply_comment_fact_storage_v0_migration()
        .await
        .expect("fact migration must apply");
    store
        .apply_comment_context_storage_v0_migration()
        .await
        .expect("context migration must apply after fact storage");
    let inspector = connect_inspector(&database_url).await;

    let (detail, comments, replies) =
        context_fixture_variant("available", TextFixtureMode::Observed);
    let available_note_id = source_text(&comments, 0, "noteId");
    let available_comment_id = source_text(&comments, 0, "commentId");
    let available_admission = store
        .admit_xhs_comment_context_capture_set_v0(WORKSPACE, detail, comments, replies)
        .await
        .expect("observed context fixture must admit");

    let (blank_detail, blank_comments, blank_replies) =
        context_fixture_variant("text-state", TextFixtureMode::BlankTitleUnavailableBody);
    let blank_admission = store
        .admit_xhs_comment_context_capture_set_v0(
            WORKSPACE,
            blank_detail,
            blank_comments,
            blank_replies,
        )
        .await
        .expect("three-state context fixture must admit");

    let (legacy_detail, legacy_comments) = legacy_pair_fixture("no-context");
    let no_context_admission = store
        .admit_xhs_comment_capture_pair_v0(WORKSPACE, legacy_detail, legacy_comments)
        .await
        .expect("legacy pair remains a current voice without a context capture set");

    let shared_store = Arc::new(store);
    let app = comment_research_router_v0(shared_store.clone());

    let before_context_reads = all_counts(&inspector).await;
    let available = get_json(
        &app,
        &context_uri(available_admission.comments_evidence.id, 0, WORKSPACE),
    )
    .await;
    assert_eq!(available.0, StatusCode::OK);
    assert_eq!(available.1["availability"], "available");
    assert_eq!(
        available.1["work_context"]["title"]["availability"],
        "observed"
    );
    assert!(available.1["work_context"]["title"]["text"].is_string());
    assert_eq!(
        available.1["work_context"]["body_text"]["availability"],
        "observed"
    );
    assert!(available.1["related_discussion"].is_array());
    assert_eq!(
        available.1["related_discussion"].as_array().map(Vec::len),
        Some(2)
    );
    assert!(
        available.1["related_discussion"]
            .as_array()
            .expect("related discussion array")
            .iter()
            .any(|item| {
                item["relationship"]["parent_comment"] == true
                    && item["relationship"]["reply_to_comment"] == true
            })
    );
    assert_context_response_has_no_forbidden_fields(&available.1);

    let blank = get_json(
        &app,
        &context_uri(blank_admission.comments_evidence.id, 0, WORKSPACE),
    )
    .await;
    assert_eq!(blank.0, StatusCode::OK);
    assert_eq!(blank.1["availability"], "available");
    assert_eq!(
        blank.1["work_context"]["title"],
        json!({"availability": "blank", "text": null})
    );
    assert_eq!(
        blank.1["work_context"]["body_text"],
        json!({"availability": "unavailable", "text": null})
    );
    assert_context_response_has_no_forbidden_fields(&blank.1);

    let no_context = get_json(
        &app,
        &context_uri(no_context_admission.comments_evidence.id, 0, WORKSPACE),
    )
    .await;
    assert_eq!(no_context.0, StatusCode::OK);
    assert_eq!(no_context.1, json!({"availability": "unavailable"}));

    for invalid_uri in [
        "/api/v0/comment-research/voices/context",
        "/api/v0/comment-research/voices/context?workspace_id=voice-context-api-proof-workspace",
        "/api/v0/comment-research/voices/context?workspace_id=voice-context-api-proof-workspace&evidence_id=not-a-uuid&record_index=0",
        "/api/v0/comment-research/voices/context?workspace_id=voice-context-api-proof-workspace&evidence_id=00000000-0000-0000-0000-000000000000",
        "/api/v0/comment-research/voices/context?workspace_id=voice-context-api-proof-workspace&evidence_id=00000000-0000-0000-0000-000000000000&record_index=-1",
        "/api/v0/comment-research/voices/context?workspace_id=voice-context-api-proof-workspace&evidence_id=00000000-0000-0000-0000-000000000000&record_index=not-an-integer",
    ] {
        let response = get_json(&app, invalid_uri).await;
        assert_eq!(response.0, StatusCode::BAD_REQUEST, "{invalid_uri}");
        assert_eq!(response.1["error"]["code"], "invalid_request");
    }

    let stale_or_forged = get_json(&app, &context_uri(uuid::Uuid::new_v4(), 0, WORKSPACE)).await;
    assert_eq!(stale_or_forged.0, StatusCode::NOT_FOUND);
    assert_eq!(
        stale_or_forged.1["error"]["code"],
        "comment_voice_not_found"
    );

    assert_eq!(
        all_counts(&inspector).await,
        before_context_reads,
        "all context HTTP reads, including invalid locators, must be read-only"
    );

    // Reclaim the store after consuming the first router, then advance the
    // direct comment body through the legacy fact path. The old context Evidence
    // locator must stop resolving as current, and the fresh direct locator must
    // report that no text-matching context set has been admitted.
    drop(app);
    let mut store = match Arc::try_unwrap(shared_store) {
        Ok(store) => store,
        Err(_) => panic!("router must release its only store reference before mutation"),
    };
    let (changed_detail, changed_comments) =
        legacy_pair_for_identity("changed-current", &available_note_id, &available_comment_id);
    let changed_admission = store
        .admit_xhs_comment_capture_pair_v0(WORKSPACE, changed_detail, changed_comments)
        .await
        .expect("changed direct source must advance current without a context set");
    let changed_evidence_id = changed_admission.comments_evidence.id;

    let app = comment_research_router_v0(Arc::new(store));
    let old_locator = get_json(
        &app,
        &context_uri(available_admission.comments_evidence.id, 0, WORKSPACE),
    )
    .await;
    assert_eq!(old_locator.0, StatusCode::NOT_FOUND);
    assert_eq!(old_locator.1["error"]["code"], "comment_voice_not_found");

    let changed_locator = get_json(&app, &context_uri(changed_evidence_id, 0, WORKSPACE)).await;
    assert_eq!(changed_locator.0, StatusCode::OK);
    assert_eq!(changed_locator.1, json!({"availability": "unavailable"}));
}

#[derive(Clone, Copy)]
enum TextFixtureMode {
    Observed,
    BlankTitleUnavailableBody,
}

fn context_fixture_variant(
    suffix: &str,
    text_mode: TextFixtureMode,
) -> (CapturePackageV0, CapturePackageV0, CapturePackageV0) {
    let fixture: Value = serde_json::from_str(CONTEXT_FIXTURE).expect("context fixture must parse");
    let packages = fixture["packages"]
        .as_object()
        .expect("context packages object");
    let mut detail: CapturePackageV0 =
        serde_json::from_value(packages["detailPackage"].clone()).expect("detail package");
    let mut comments: CapturePackageV0 =
        serde_json::from_value(packages["commentsPackage"].clone()).expect("comments package");
    let mut replies: CapturePackageV0 =
        serde_json::from_value(packages["repliesPackage"].clone()).expect("replies package");

    let note_id = format!("context-note-{suffix}");
    let comment_id = format!("context-comment-{suffix}");
    detail.records[0].payload["noteId"] = json!(note_id);
    comments.records[0].payload["noteId"] = json!(note_id);
    comments.records[0].payload["commentId"] = json!(comment_id);
    comments.records[0].payload["text"] = json!(format!("context direct voice {suffix}"));

    for (index, record) in replies.records.iter_mut().enumerate() {
        record.payload["noteId"] = json!(note_id);
        record.payload["commentId"] = json!(format!("context-reply-{suffix}-{index}"));
        record.payload["text"] = json!(format!("context related reply {suffix} {index}"));
        record.payload["rootCommentId"] = json!(comment_id);
        record.payload["parentCommentId"] = json!(comment_id);
        record.payload["replyToCommentId"] = json!(comment_id);
    }

    match text_mode {
        TextFixtureMode::Observed => {}
        TextFixtureMode::BlankTitleUnavailableBody => {
            detail.records[0].payload["title"] = json!("");
            detail.records[0]
                .payload
                .as_object_mut()
                .expect("detail payload object")
                .remove("bodyText");
        }
    }

    detail.extensions.insert(
        "packageRef".to_owned(),
        json!(format!("context-detail-{suffix}")),
    );
    comments.extensions.insert(
        "packageRef".to_owned(),
        json!(format!("context-comments-{suffix}")),
    );
    replies.extensions.insert(
        "packageRef".to_owned(),
        json!(format!("context-replies-{suffix}")),
    );
    (detail, comments, replies)
}

fn legacy_pair_fixture(suffix: &str) -> (CapturePackageV0, CapturePackageV0) {
    legacy_pair_for_identity(
        suffix,
        &format!("legacy-note-{suffix}"),
        &format!("legacy-comment-{suffix}"),
    )
}

fn legacy_pair_for_identity(
    suffix: &str,
    note_id: &str,
    comment_id: &str,
) -> (CapturePackageV0, CapturePackageV0) {
    let fixture: Value = serde_json::from_str(PAIR_FIXTURE).expect("pair fixture must parse");
    let packages = fixture["packages"]
        .as_object()
        .expect("pair packages object");
    let mut detail: CapturePackageV0 =
        serde_json::from_value(packages["detailPackage"].clone()).expect("detail package");
    let mut comments: CapturePackageV0 =
        serde_json::from_value(packages["commentsPackage"].clone()).expect("comments package");
    detail.records[0].payload["noteId"] = json!(note_id);
    comments.records[0].payload["noteId"] = json!(note_id);
    comments.records[0].payload["commentId"] = json!(comment_id);
    comments.records[0].payload["text"] = json!(format!("changed direct voice {suffix}"));
    detail.extensions.insert(
        "packageRef".to_owned(),
        json!(format!("legacy-detail-{suffix}")),
    );
    comments.extensions.insert(
        "packageRef".to_owned(),
        json!(format!("legacy-comments-{suffix}")),
    );
    (detail, comments)
}

fn source_text(package: &CapturePackageV0, record_index: usize, key: &str) -> String {
    package.records[record_index].payload[key]
        .as_str()
        .expect("fixture source field must remain a string")
        .to_owned()
}

fn context_uri(evidence_id: uuid::Uuid, record_index: i32, workspace_id: &str) -> String {
    format!(
        "/api/v0/comment-research/voices/context?workspace_id={workspace_id}&evidence_id={evidence_id}&record_index={record_index}"
    )
}

async fn get_json(app: &axum::Router, uri: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(uri)
                .body(Body::empty())
                .expect("request must build"),
        )
        .await
        .expect("router must respond");
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body must be readable");
    let body = serde_json::from_slice(&body).expect("response body must be JSON");
    (status, body)
}

async fn connect_inspector(database_url: &str) -> Client {
    let (client, connection) = tokio_postgres::connect(database_url, NoTls)
        .await
        .expect("inspector must connect to the isolated PostgreSQL database");
    tokio::spawn(async move {
        connection
            .await
            .expect("proof database connection must stay healthy");
    });
    client
}

async fn all_counts(inspector: &Client) -> Vec<i64> {
    let tables = [
        "source_evidence_v0",
        "source_capture_pair_v0",
        "comment_identity_v0",
        "evidence_comment_record_v0",
        "comment_observation_v0",
        "comment_current_v0",
        "context_work_record_v0",
        "context_discussion_record_v0",
        "source_context_capture_set_v0",
    ];
    let mut counts = Vec::with_capacity(tables.len());
    for table in tables {
        let row = inspector
            .query_one(&format!("SELECT count(*) FROM {table}"), &[])
            .await
            .expect("table count query must succeed");
        counts.push(row.get(0));
    }
    counts
}

fn assert_context_response_has_no_forbidden_fields(value: &Value) {
    const FORBIDDEN: &[&str] = &[
        "author",
        "author_id",
        "comment_id",
        "note_id",
        "url",
        "raw_url",
        "likes",
        "published_at",
        "published_time",
        "ocr",
        "asr",
        "media",
    ];
    match value {
        Value::Object(object) => {
            for (key, nested) in object {
                assert!(
                    !FORBIDDEN.contains(&key.as_str()),
                    "context DTO must not expose forbidden field {key}"
                );
                assert_context_response_has_no_forbidden_fields(nested);
            }
        }
        Value::Array(values) => {
            for nested in values {
                assert_context_response_has_no_forbidden_fields(nested);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}
