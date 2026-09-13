use std::sync::Arc;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use linggan_api::comment_research_router_v0;
use linggan_contracts::CapturePackageV0;
use linggan_storage_postgres::CommentFactStore;
use serde_json::{Value, json};
use tokio_postgres::{Client, NoTls};
use tower::ServiceExt;

const FIXTURE: &str = include_str!("../../../fixtures/xhs/comment-evidence-set-v1.json");
const WORKSPACE: &str = "voices-api-proof-workspace";

/// This proof has no fallback database. The companion script creates a fresh
/// postgres:16 container on a random loopback port and removes it afterwards.
#[tokio::test]
#[ignore = "requires scripts/prove-comment-voices-read-api-v0.sh"]
async fn proves_user_voices_v0_read_api_against_isolated_postgres() {
    let database_url = std::env::var("LINGGAN_COMMENT_VOICES_API_TEST_DATABASE_URL")
        .expect("proof script must provide an isolated PostgreSQL database URL");

    let mut store = CommentFactStore::connect(&database_url)
        .await
        .expect("isolated proof database must connect");
    store
        .apply_comment_fact_storage_v0_migration()
        .await
        .expect("greenfield migration must apply");
    let inspector = connect_inspector(&database_url).await;

    for identity_suffix in ["001", "002", "003"] {
        let (detail, comments) = fixture_pair(identity_suffix);
        store
            .admit_xhs_comment_capture_pair_v0(WORKSPACE, detail, comments)
            .await
            .expect("fixture-derived source pair must admit");
    }
    assert_all_counts(&inspector, [6, 3, 3, 3, 3, 3]).await;

    let app = comment_research_router_v0(Arc::new(store));

    let (page_status, page_content_type, page) = get_text(&app, "/comment-research/voices").await;
    assert_eq!(page_status, StatusCode::OK);
    assert_eq!(
        page_content_type.as_deref(),
        Some("text/html; charset=utf-8")
    );
    assert!(page.contains("用户原声"));
    assert!(page.contains("初始状态不读取任何数据"));
    assert!(page.contains("尚未具备来源事实"));
    assert!(page.contains("打开详情后按来源读取"));
    assert!(page.contains("已采到的相关讨论"));
    assert!(page.contains("const workspaceId = state.workspaceId.trim()"));

    let (css_status, css_content_type, stylesheet) =
        get_text(&app, "/comment-research/voices/styles.css").await;
    assert_eq!(css_status, StatusCode::OK);
    assert_eq!(css_content_type.as_deref(), Some("text/css; charset=utf-8"));
    assert!(stylesheet.contains("--canvas: oklch(0.965 0.008 85)"));
    assert!(stylesheet.contains("@media (prefers-reduced-motion: reduce)"));

    // Static page and stylesheet requests do not call the write admission path.
    assert_all_counts(&inspector, [6, 3, 3, 3, 3, 3]).await;

    let first_page = get_json(
        &app,
        "/api/v0/comment-research/voices?workspace_id=voices-api-proof-workspace&limit=2&offset=0",
    )
    .await;
    assert_eq!(first_page.0, StatusCode::OK);
    assert_eq!(
        first_page.1["pagination"],
        json!({"total": 3, "limit": 2, "offset": 0})
    );
    let first_voices = first_page.1["voices"]
        .as_array()
        .expect("voices must be an array");
    assert_eq!(first_voices.len(), 2);
    for voice in first_voices {
        assert_eq!(voice["research_status"], "not_researched");
        assert_eq!(
            voice
                .as_object()
                .expect("voice must be an object")
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
            vec![
                "current_admitted_at",
                "research_status",
                "source_evidence",
                "source_note_id",
                "text",
            ],
            "read DTO must not leak unproven author, engagement, platform time, or reply fields"
        );
        assert!(voice["source_evidence"]["evidence_id"].is_string());
        assert_eq!(voice["source_evidence"]["record_index"], 0);
        assert!(
            voice["current_admitted_at"]
                .as_str()
                .expect("local admission time must be text")
                .ends_with('Z')
        );
    }

    let second_page = get_json(
        &app,
        "/api/v0/comment-research/voices?workspace_id=voices-api-proof-workspace&limit=2&offset=2",
    )
    .await;
    assert_eq!(second_page.0, StatusCode::OK);
    assert_eq!(
        second_page.1["pagination"],
        json!({"total": 3, "limit": 2, "offset": 2})
    );
    let second_voices = second_page.1["voices"]
        .as_array()
        .expect("voices must be an array");
    assert_eq!(second_voices.len(), 1);
    assert_ne!(
        first_voices[0]["source_note_id"], second_voices[0]["source_note_id"],
        "offset page must not repeat a source note from the first page"
    );
    assert_ne!(
        first_voices[1]["source_note_id"], second_voices[0]["source_note_id"],
        "offset page must not repeat a source note from the first page"
    );

    let exhausted = get_json(
        &app,
        "/api/v0/comment-research/voices?workspace_id=voices-api-proof-workspace&limit=2&offset=3",
    )
    .await;
    assert_eq!(exhausted.0, StatusCode::OK);
    assert_eq!(
        exhausted.1,
        json!({"pagination": {"total": 3, "limit": 2, "offset": 3}, "voices": []})
    );

    let empty = get_json(
        &app,
        "/api/v0/comment-research/voices?workspace_id=empty-workspace&limit=2&offset=0",
    )
    .await;
    assert_eq!(empty.0, StatusCode::OK);
    assert_eq!(
        empty.1,
        json!({"pagination": {"total": 0, "limit": 2, "offset": 0}, "voices": []})
    );

    for invalid_uri in [
        "/api/v0/comment-research/voices",
        "/api/v0/comment-research/voices?workspace_id=%20%20",
        "/api/v0/comment-research/voices?workspace_id=voices-api-proof-workspace&limit=0",
        "/api/v0/comment-research/voices?workspace_id=voices-api-proof-workspace&limit=101",
        "/api/v0/comment-research/voices?workspace_id=voices-api-proof-workspace&limit=not-a-number",
        "/api/v0/comment-research/voices?workspace_id=voices-api-proof-workspace&offset=-1",
    ] {
        let invalid = get_json(&app, invalid_uri).await;
        assert_eq!(invalid.0, StatusCode::BAD_REQUEST, "{invalid_uri}");
        assert_eq!(invalid.1["error"]["code"], "invalid_request");
    }

    // Every request above, including invalid input, only used the V0 SELECT
    // route. No Evidence, identity, Observation, or Current row may change.
    assert_all_counts(&inspector, [6, 3, 3, 3, 3, 3]).await;
}

fn fixture_pair(identity_suffix: &str) -> (CapturePackageV0, CapturePackageV0) {
    let fixture: Value = serde_json::from_str(FIXTURE).expect("fixture must parse");
    let packages = fixture["packages"].as_object().expect("fixture packages");
    let mut detail: CapturePackageV0 =
        serde_json::from_value(packages["detailPackage"].clone()).expect("detail package");
    let mut comments: CapturePackageV0 =
        serde_json::from_value(packages["commentsPackage"].clone()).expect("comments package");

    let note_id = format!("noteId-fixture-{identity_suffix}");
    let comment_id = format!("commentId-fixture-{identity_suffix}");
    detail.records[0].payload["noteId"] = json!(note_id);
    comments.records[0].payload["noteId"] = json!(note_id);
    comments.records[0].payload["commentId"] = json!(comment_id);
    comments.records[0].payload["text"] = json!(format!("fixture comment text {identity_suffix}"));
    detail.extensions.insert(
        "packageRef".to_owned(),
        json!(format!("detailPackageRef-fixture-{identity_suffix}")),
    );
    comments.extensions.insert(
        "packageRef".to_owned(),
        json!(format!("commentsPackageRef-fixture-{identity_suffix}")),
    );

    (detail, comments)
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

async fn get_text(app: &axum::Router, uri: &str) -> (StatusCode, Option<String>, String) {
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
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body must be readable");
    let body = String::from_utf8(body.to_vec()).expect("page body must be UTF-8");
    (status, content_type, body)
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

async fn assert_all_counts(inspector: &Client, expected: [i64; 6]) {
    let tables = [
        "source_evidence_v0",
        "source_capture_pair_v0",
        "comment_identity_v0",
        "evidence_comment_record_v0",
        "comment_observation_v0",
        "comment_current_v0",
    ];

    for (table, expected_count) in tables.iter().zip(expected) {
        let row = inspector
            .query_one(&format!("SELECT count(*) FROM {table}"), &[])
            .await
            .expect("count query");
        assert_eq!(
            row.get::<_, i64>(0),
            expected_count,
            "unexpected row count for {table}"
        );
    }
}
