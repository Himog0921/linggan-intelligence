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
use uuid::Uuid;

const FIXTURE: &str = include_str!("../../../fixtures/xhs/comment-evidence-set-v1.json");
const WORKSPACE: &str = "plan-preview-proof-workspace";
const NEEDS_ONLY_WORKSPACE: &str = "plan-preview-needs-only-workspace";
const EXCLUDED_ONLY_WORKSPACE: &str = "plan-preview-excluded-only-workspace";
const AWAITING_ONLY_WORKSPACE: &str = "plan-preview-awaiting-only-workspace";

/// This proof runs only through its companion script, which gives it an empty
/// disposable PostgreSQL database. It does not contact a model, vector service,
/// worker, network source, or configured local runtime database.
#[tokio::test]
#[ignore = "requires scripts/prove-comment-research-plan-preview-v0.sh"]
async fn proves_read_only_source_rotated_comment_research_plan_preview_v0() {
    let database_url = std::env::var("LINGGAN_COMMENT_RESEARCH_PLAN_PREVIEW_TEST_DATABASE_URL")
        .expect("proof script must supply an isolated PostgreSQL URL");
    let mut store = CommentFactStore::connect(&database_url)
        .await
        .expect("isolated store connects");
    store
        .apply_comment_fact_storage_v0_migration()
        .await
        .expect("fact migration applies");
    store
        .apply_comment_derivation_v1_migration()
        .await
        .expect("derivation migration applies");
    let inspector = connect_inspector(&database_url).await;

    // Three source works have usable current expressions. Source A has two, so
    // a four-row preview must show A1, B1, C1, then A2 instead of consuming A.
    let first_a = admit_fixture_comment(
        &mut store,
        WORKSPACE,
        "source-a-first",
        "source-a",
        "source-a-comment-1",
        "孩子每天开始写作业都很困难",
    )
    .await;
    admit_fixture_comment(
        &mut store,
        WORKSPACE,
        "source-a-second",
        "source-a",
        "source-a-comment-2",
        "一催就发脾气，不知道怎么沟通",
    )
    .await;
    admit_fixture_comment(
        &mut store,
        WORKSPACE,
        "source-b-first",
        "source-b",
        "source-b-comment-1",
        "十岁孩子现在开始训练还来得及吗",
    )
    .await;
    admit_fixture_comment(
        &mut store,
        WORKSPACE,
        "source-c-context",
        "source-c",
        "source-c-comment-1",
        "同问",
    )
    .await;
    admit_fixture_comment(
        &mut store,
        WORKSPACE,
        "source-d-dropped",
        "source-d",
        "source-d-comment-1",
        "😂❤️",
    )
    .await;
    admit_fixture_comment(
        &mut store,
        WORKSPACE,
        "source-e-anomaly",
        "source-e",
        "source-e-comment-1",
        "保留原始事实\u{0007}",
    )
    .await;

    // A current observation without a V1 derivation is deliberately visible in
    // preparation totals but is never materialized by this GET preview.
    seed_current_without_derivation(
        &inspector,
        WORKSPACE,
        "awaiting-note",
        "awaiting-comment",
        "awaiting current source",
        'a',
    )
    .await;

    admit_fixture_comment(
        &mut store,
        NEEDS_ONLY_WORKSPACE,
        "needs-only",
        "needs-only-note",
        "needs-only-comment",
        "同问",
    )
    .await;
    admit_fixture_comment(
        &mut store,
        EXCLUDED_ONLY_WORKSPACE,
        "excluded-dropped",
        "excluded-note-a",
        "excluded-comment-a",
        "😂",
    )
    .await;
    admit_fixture_comment(
        &mut store,
        EXCLUDED_ONLY_WORKSPACE,
        "excluded-anomaly",
        "excluded-note-b",
        "excluded-comment-b",
        "控制字符\u{0007}",
    )
    .await;
    seed_current_without_derivation(
        &inspector,
        AWAITING_ONLY_WORKSPACE,
        "awaiting-only-note",
        "awaiting-only-comment",
        "only awaiting source",
        'b',
    )
    .await;

    let shared_store = Arc::new(store);
    let app = comment_research_router_v0(shared_store.clone());
    let counts_before_reads = all_counts(&inspector).await;

    let preview_uri = plan_preview_uri(WORKSPACE, "available", 4);
    let first_preview = get_json(&app, &preview_uri).await;
    assert_eq!(first_preview.0, StatusCode::OK);
    assert_eq!(first_preview.1["preview_state"], "live_read_only");
    assert_eq!(first_preview.1["scope"], "available");
    assert_eq!(first_preview.1["limit"], 4);
    assert_eq!(
        first_preview.1["preparation"],
        json!({
            "current_total": 7,
            "available_total": 4,
            "ready_total": 3,
            "needs_context_total": 1,
            "awaiting_cleaning_total": 1,
            "excluded_total": 2
        })
    );
    assert_candidate_note_order(
        &first_preview.1,
        &["source-a", "source-b", "source-c", "source-a"],
    );
    assert_eq!(
        first_preview.1["candidates"]
            .as_array()
            .expect("candidate list")
            .iter()
            .map(|candidate| candidate["source_rotation_turn"].as_i64())
            .collect::<Option<Vec<_>>>(),
        Some(vec![1, 1, 1, 2])
    );
    assert_eq!(
        first_preview.1["source_distribution"],
        json!([
            {"source_note_id": "source-a", "eligible_total": 2, "selected_total": 2},
            {"source_note_id": "source-b", "eligible_total": 1, "selected_total": 1},
            {"source_note_id": "source-c", "eligible_total": 1, "selected_total": 1}
        ])
    );
    assert_no_forbidden_plan_preview_field(&first_preview.1);

    let second_preview = get_json(&app, &preview_uri).await;
    assert_eq!(
        second_preview.1, first_preview.1,
        "two live read-only previews over unchanged current facts must be deterministic"
    );

    let ready = get_json(&app, &plan_preview_uri(WORKSPACE, "ready", 100)).await;
    assert_eq!(ready.0, StatusCode::OK);
    assert_eq!(ready.1["candidates"].as_array().map(Vec::len), Some(3));
    assert!(
        ready.1["candidates"]
            .as_array()
            .expect("ready candidates")
            .iter()
            .all(|candidate| candidate["readiness"] == "ready")
    );

    let needs_context = get_json(&app, &plan_preview_uri(WORKSPACE, "needs_context", 50)).await;
    assert_eq!(needs_context.0, StatusCode::OK);
    assert_eq!(
        needs_context.1["candidates"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(
        needs_context.1["candidates"][0]["source_note_id"],
        "source-c"
    );
    assert_eq!(
        needs_context.1["candidates"][0]["readiness"],
        "needs_context"
    );

    let scope_has_no_candidates =
        get_json(&app, &plan_preview_uri(NEEDS_ONLY_WORKSPACE, "ready", 20)).await;
    assert_eq!(scope_has_no_candidates.0, StatusCode::OK);
    assert_eq!(
        scope_has_no_candidates.1["preparation"],
        json!({
            "current_total": 1,
            "available_total": 1,
            "ready_total": 0,
            "needs_context_total": 1,
            "awaiting_cleaning_total": 0,
            "excluded_total": 0
        })
    );
    assert_eq!(scope_has_no_candidates.1["candidates"], json!([]));
    assert_eq!(scope_has_no_candidates.1["source_distribution"], json!([]));

    let only_excluded = get_json(
        &app,
        &plan_preview_uri(EXCLUDED_ONLY_WORKSPACE, "available", 20),
    )
    .await;
    assert_eq!(only_excluded.0, StatusCode::OK);
    assert_eq!(
        only_excluded.1["preparation"],
        json!({
            "current_total": 2,
            "available_total": 0,
            "ready_total": 0,
            "needs_context_total": 0,
            "awaiting_cleaning_total": 0,
            "excluded_total": 2
        })
    );
    assert_eq!(only_excluded.1["candidates"], json!([]));

    let only_awaiting = get_json(
        &app,
        &plan_preview_uri(AWAITING_ONLY_WORKSPACE, "available", 20),
    )
    .await;
    assert_eq!(only_awaiting.0, StatusCode::OK);
    assert_eq!(
        only_awaiting.1["preparation"],
        json!({
            "current_total": 1,
            "available_total": 0,
            "ready_total": 0,
            "needs_context_total": 0,
            "awaiting_cleaning_total": 1,
            "excluded_total": 0
        })
    );
    assert_eq!(only_awaiting.1["candidates"], json!([]));

    let empty = get_json(
        &app,
        &plan_preview_uri("empty-plan-preview-workspace", "available", 50),
    )
    .await;
    assert_eq!(empty.0, StatusCode::OK);
    assert_eq!(
        empty.1,
        json!({
            "preview_state": "live_read_only",
            "scope": "available",
            "limit": 50,
            "preparation": {
                "current_total": 0,
                "available_total": 0,
                "ready_total": 0,
                "needs_context_total": 0,
                "awaiting_cleaning_total": 0,
                "excluded_total": 0
            },
            "source_distribution": [],
            "candidates": []
        })
    );

    for invalid_uri in [
        "/api/v0/comment-research/plan-preview",
        "/api/v0/comment-research/plan-preview?workspace_id=%20%20",
        "/api/v0/comment-research/plan-preview?workspace_id=plan-preview-proof-workspace&scope=invalid",
        "/api/v0/comment-research/plan-preview?workspace_id=plan-preview-proof-workspace&limit=0",
        "/api/v0/comment-research/plan-preview?workspace_id=plan-preview-proof-workspace&limit=101",
        "/api/v0/comment-research/plan-preview?workspace_id=plan-preview-proof-workspace&limit=not-a-number",
    ] {
        let invalid = get_json(&app, invalid_uri).await;
        assert_eq!(invalid.0, StatusCode::BAD_REQUEST, "{invalid_uri}");
        assert_eq!(invalid.1["error"]["code"], "invalid_request");
    }

    assert_eq!(
        all_counts(&inspector).await,
        counts_before_reads,
        "all plan preview HTTP paths must stay read-only and never materialize cleaning, persist a plan, or create work"
    );

    // A changed current source body invalidates the former current Evidence
    // locator. The new current source appears in the live preview; the old one
    // cannot remain a candidate merely because its identity is unchanged.
    drop(app);
    let mut store = match Arc::try_unwrap(shared_store) {
        Ok(store) => store,
        Err(_) => panic!("router must release its only store reference before mutation"),
    };
    let changed_a = admit_fixture_comment(
        &mut store,
        WORKSPACE,
        "source-a-first-changed",
        "source-a",
        "source-a-comment-1",
        "孩子每天开始写作业都很困难，现在需要新的办法",
    )
    .await;
    let changed_evidence_id = changed_a.comments_evidence.id;
    let app = comment_research_router_v0(Arc::new(store));
    let changed_preview = get_json(&app, &plan_preview_uri(WORKSPACE, "available", 4)).await;
    assert_eq!(changed_preview.0, StatusCode::OK);
    assert!(
        changed_preview.1["candidates"]
            .as_array()
            .expect("changed candidates")
            .iter()
            .all(|candidate| candidate["source_evidence"]["evidence_id"]
                != first_a.comments_evidence.id.to_string()),
        "an old current source Evidence locator cannot survive a changed current observation"
    );
    assert!(
        changed_preview.1["candidates"]
            .as_array()
            .expect("changed candidates")
            .iter()
            .any(|candidate| candidate["source_evidence"]["evidence_id"]
                == changed_evidence_id.to_string()),
        "the advanced current observation must appear through its new Evidence locator"
    );
}

async fn admit_fixture_comment(
    store: &mut CommentFactStore,
    workspace_id: &str,
    suffix: &str,
    note_id: &str,
    comment_id: &str,
    text: &str,
) -> linggan_storage_postgres::CommentFactAdmissionOutcomeV0 {
    let fixture: Value = serde_json::from_str(FIXTURE).expect("fixture parses");
    let packages = fixture["packages"].as_object().expect("fixture packages");
    let mut detail: CapturePackageV0 =
        serde_json::from_value(packages["detailPackage"].clone()).expect("detail package");
    let mut comments: CapturePackageV0 =
        serde_json::from_value(packages["commentsPackage"].clone()).expect("comments package");
    detail.records[0].payload["noteId"] = json!(note_id);
    comments.records[0].payload["noteId"] = json!(note_id);
    comments.records[0].payload["commentId"] = json!(comment_id);
    comments.records[0].payload["text"] = json!(text);
    detail.extensions.insert(
        "packageRef".to_owned(),
        json!(format!("plan-detail-{suffix}")),
    );
    comments.extensions.insert(
        "packageRef".to_owned(),
        json!(format!("plan-comments-{suffix}")),
    );
    store
        .admit_xhs_comment_capture_pair_v0(workspace_id, detail, comments)
        .await
        .expect("fixture comment must admit")
}

async fn seed_current_without_derivation(
    inspector: &Client,
    workspace_id: &str,
    note_id: &str,
    comment_id: &str,
    source_text: &str,
    hash_character: char,
) {
    let evidence_id = Uuid::new_v4();
    let observation_id = Uuid::new_v4();
    let text_sha256 = hash_character.to_string().repeat(64);
    let payload_sha256 = match hash_character {
        'a' => "b".repeat(64),
        'b' => "c".repeat(64),
        _ => "d".repeat(64),
    };
    inspector
        .execute(
            "INSERT INTO source_evidence_v0 \
             (id, workspace_id, platform, source_contract, package_kind, payload_sha256, source_payload) \
             VALUES ($1, $2, 'xhs', 'xhs.comment-capture-source.v0', 'comments', $3, $4)",
            &[&evidence_id, &workspace_id, &payload_sha256, &json!({"legacy": true, "note": note_id})],
        )
        .await
        .expect("legacy Evidence seed");
    inspector
        .execute(
            "INSERT INTO comment_identity_v0 (workspace_id, platform, note_id, comment_id) \
             VALUES ($1, 'xhs', $2, $3)",
            &[&workspace_id, &note_id, &comment_id],
        )
        .await
        .expect("legacy identity seed");
    inspector
        .execute(
            "INSERT INTO evidence_comment_record_v0 \
             (source_evidence_id, source_record_index, workspace_id, platform, note_id, comment_id, text_sha256) \
             VALUES ($1, 0, $2, 'xhs', $3, $4, $5)",
            &[&evidence_id, &workspace_id, &note_id, &comment_id, &text_sha256],
        )
        .await
        .expect("legacy source record seed");
    inspector
        .execute(
            "INSERT INTO comment_observation_v0 \
             (id, workspace_id, platform, note_id, comment_id, source_evidence_id, source_record_index, text_sha256, source_text) \
             VALUES ($1, $2, 'xhs', $3, $4, $5, 0, $6, $7)",
            &[&observation_id, &workspace_id, &note_id, &comment_id, &evidence_id, &text_sha256, &source_text],
        )
        .await
        .expect("legacy observation seed");
    inspector
        .execute(
            "INSERT INTO comment_current_v0 \
             (workspace_id, platform, note_id, comment_id, current_observation_id, text_sha256) \
             VALUES ($1, 'xhs', $2, $3, $4, $5)",
            &[
                &workspace_id,
                &note_id,
                &comment_id,
                &observation_id,
                &text_sha256,
            ],
        )
        .await
        .expect("legacy current seed");
}

fn plan_preview_uri(workspace_id: &str, scope: &str, limit: i64) -> String {
    format!(
        "/api/v0/comment-research/plan-preview?workspace_id={workspace_id}&scope={scope}&limit={limit}"
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
        .expect("router responds");
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body reads");
    (
        status,
        serde_json::from_slice(&body).expect("JSON response"),
    )
}

async fn connect_inspector(database_url: &str) -> Client {
    let (client, connection) = tokio_postgres::connect(database_url, NoTls)
        .await
        .expect("inspector connects");
    tokio::spawn(async move {
        connection
            .await
            .expect("inspector connection stays healthy");
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
        "comment_derivation_v1",
    ];
    let mut counts = Vec::with_capacity(tables.len());
    for table in tables {
        counts.push(
            inspector
                .query_one(&format!("SELECT count(*) FROM {table}"), &[])
                .await
                .expect("count query")
                .get(0),
        );
    }
    counts
}

fn assert_candidate_note_order(preview: &Value, expected: &[&str]) {
    let actual = preview["candidates"]
        .as_array()
        .expect("candidate array")
        .iter()
        .map(|candidate| candidate["source_note_id"].as_str())
        .collect::<Option<Vec<_>>>()
        .expect("candidate source IDs are strings");
    assert_eq!(actual, expected);
}

fn assert_no_forbidden_plan_preview_field(value: &Value) {
    const FORBIDDEN: &[&str] = &[
        "author",
        "author_id",
        "comment_id",
        "derivation",
        "derivation_id",
        "fingerprint",
        "likes",
        "model",
        "ocr",
        "asr",
        "media",
        "published_at",
        "published_time",
        "raw_text",
        "original_text",
        "original_voice_text",
        "reason_codes",
        "source_text",
        "url",
    ];
    match value {
        Value::Object(object) => {
            for (key, nested) in object {
                assert!(
                    !FORBIDDEN.contains(&key.as_str()),
                    "plan preview DTO must not expose {key}"
                );
                assert_no_forbidden_plan_preview_field(nested);
            }
        }
        Value::Array(values) => {
            for nested in values {
                assert_no_forbidden_plan_preview_field(nested);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}
