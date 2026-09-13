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

const CONTEXT_FIXTURE: &str =
    include_str!("../../../fixtures/xhs/comment-context-evidence-set-v0.json");
const PAIR_FIXTURE: &str = include_str!("../../../fixtures/xhs/comment-evidence-set-v1.json");
const WORKSPACE: &str = "voice-context-pack-proof-workspace";

/// This proof creates a disposable PostgreSQL instance through its companion
/// script. It validates the real Axum route, but never calls a model, creates a
/// plan, freezes an input snapshot, or reads a configured runtime database.
#[tokio::test]
#[ignore = "requires scripts/prove-comment-voice-context-pack-preview-v1.sh"]
async fn proves_source_backed_comment_research_context_pack_preview_v1() {
    let database_url = std::env::var("LINGGAN_COMMENT_CONTEXT_PACK_PREVIEW_TEST_DATABASE_URL")
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
    store
        .apply_comment_derivation_v1_migration()
        .await
        .expect("derivation migration must apply after fact baseline");
    let inspector = connect_inspector(&database_url).await;

    let (observed_detail, observed_comments, observed_replies) =
        context_fixture_variant("observed", ContextFixtureMode::Observed);
    let observed_note_id = source_text(&observed_comments, 0, "noteId");
    let observed_comment_id = source_text(&observed_comments, 0, "commentId");
    let observed_admission = store
        .admit_xhs_comment_context_capture_set_v0(
            WORKSPACE,
            observed_detail,
            observed_comments,
            observed_replies,
        )
        .await
        .expect("observed context fixture must admit");

    let (blank_detail, blank_comments, blank_replies) = context_fixture_variant(
        "blank-unavailable",
        ContextFixtureMode::BlankTitleUnavailableBody,
    );
    let blank_admission = store
        .admit_xhs_comment_context_capture_set_v0(
            WORKSPACE,
            blank_detail,
            blank_comments,
            blank_replies,
        )
        .await
        .expect("three-state context fixture must admit");

    let (long_detail, long_comments, long_replies) =
        context_fixture_variant("long-unicode", ContextFixtureMode::LongUnicode);
    let long_admission = store
        .admit_xhs_comment_context_capture_set_v0(
            WORKSPACE,
            long_detail,
            long_comments,
            long_replies,
        )
        .await
        .expect("long source-backed context fixture must admit");

    let (needs_context_detail, needs_context_comments) = legacy_pair_for_identity(
        "needs-context",
        "needs-context-note",
        "needs-context-comment",
        "同问",
    );
    let needs_context_admission = store
        .admit_xhs_comment_capture_pair_v0(WORKSPACE, needs_context_detail, needs_context_comments)
        .await
        .expect("needs-context direct voice must admit without context capture");

    let shared_store = Arc::new(store);
    let app = comment_research_router_v0(shared_store.clone());
    let before_reads = all_counts(&inspector).await;

    let observed_uri = context_pack_uri(observed_admission.comments_evidence.id, 0, WORKSPACE);
    let observed = get_json(&app, &observed_uri).await;
    assert_eq!(observed.0, StatusCode::OK);
    assert_eq!(observed.1["preview_state"], "source_backed_read_only");
    assert_eq!(observed.1["readiness"], "ready");
    assert_eq!(
        observed.1["direct_comment_evidence"]["text"],
        "context direct voice observed"
    );
    assert_eq!(
        observed.1["discussion_context"]["availability"],
        "available"
    );
    assert_eq!(
        observed.1["discussion_context"]["excerpts"]
            .as_array()
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(observed.1["work_context"]["availability"], "available");
    assert_eq!(
        observed.1["work_context"]["title"]["availability"],
        "observed"
    );
    assert!(observed.1["work_context"]["title"]["text"].is_string());
    assert!(
        observed.1["discussion_context"]["excerpts"]
            .as_array()
            .expect("observed excerpt array")
            .iter()
            .any(|item| {
                item["relationship"]["parent_comment"] == true
                    && item["relationship"]["reply_to_comment"] == true
            })
    );
    assert_ne!(
        observed.1["direct_comment_evidence"]["text"],
        observed.1["discussion_context"]["excerpts"][0]["text"]["text"],
        "discussion context must not be placed in direct comment evidence"
    );
    assert_context_pack_response_has_no_forbidden_fields(&observed.1);

    let observed_again = get_json(&app, &observed_uri).await;
    assert_eq!(
        observed_again.1, observed.1,
        "unchanged source facts must assemble an identical read-only preview"
    );

    let blank = get_json(
        &app,
        &context_pack_uri(blank_admission.comments_evidence.id, 0, WORKSPACE),
    )
    .await;
    assert_eq!(blank.0, StatusCode::OK);
    assert_eq!(blank.1["work_context"]["availability"], "available");
    assert_eq!(
        blank.1["work_context"]["title"],
        json!({"availability": "blank", "text": null, "truncated": false})
    );
    assert_eq!(
        blank.1["work_context"]["body_text"],
        json!({"availability": "unavailable", "text": null, "truncated": false})
    );
    assert_context_pack_response_has_no_forbidden_fields(&blank.1);

    let needs_context = get_json(
        &app,
        &context_pack_uri(needs_context_admission.comments_evidence.id, 0, WORKSPACE),
    )
    .await;
    assert_eq!(needs_context.0, StatusCode::OK);
    assert_eq!(needs_context.1["readiness"], "needs_context");
    assert_eq!(
        needs_context.1["direct_comment_evidence"],
        json!({"text": "同问", "truncated": false})
    );
    assert_eq!(
        needs_context.1["discussion_context"],
        json!({"availability": "unavailable", "excerpts": []})
    );
    assert_eq!(
        needs_context.1["work_context"]["availability"],
        "unavailable"
    );
    assert!(
        needs_context.1["future_execution_note"]
            .as_str()
            .expect("needs-context note")
            .contains("后续实际研究仍必须单独检查上下文是否充足")
    );
    assert!(
        needs_context.1["omissions"]
            .as_array()
            .expect("omissions")
            .iter()
            .any(|item| item == "当前原声尚无正文匹配的来源上下文；这不表示平台没有其他文本。")
    );
    assert_context_pack_response_has_no_forbidden_fields(&needs_context.1);

    let long = get_json(
        &app,
        &context_pack_uri(long_admission.comments_evidence.id, 0, WORKSPACE),
    )
    .await;
    assert_eq!(long.0, StatusCode::OK);
    assert_eq!(long.1["budget"]["total_character_limit"], 1_800);
    assert_eq!(
        long.1["budget"]["direct_comment_evidence_character_limit"],
        480
    );
    assert_eq!(long.1["budget"]["related_discussion_item_limit"], 3);
    assert!(
        long.1["budget"]["included_characters"]
            .as_u64()
            .expect("included char count")
            <= 1_800
    );
    let direct = long.1["direct_comment_evidence"]["text"]
        .as_str()
        .expect("direct unicode text");
    assert_eq!(direct.chars().count(), 480);
    assert_eq!(long.1["direct_comment_evidence"]["truncated"], true);
    assert_eq!(
        long.1["discussion_context"]["excerpts"]
            .as_array()
            .map(Vec::len),
        Some(3)
    );
    assert!(
        long.1["discussion_context"]["excerpts"]
            .as_array()
            .expect("truncated discussion")
            .iter()
            .all(|item| {
                item["text"]["text"]
                    .as_str()
                    .expect("excerpt text")
                    .is_char_boundary(item["text"]["text"].as_str().expect("excerpt text").len())
            })
    );
    assert_eq!(long.1["work_context"]["title"]["truncated"], true);
    assert_eq!(long.1["work_context"]["body_text"]["truncated"], true);
    assert!(
        long.1["omissions"]
            .as_array()
            .expect("long omissions")
            .iter()
            .any(|item| item == "本次最多纳入 3 条已读取的相关讨论；其余已读取内容未放入预览。")
    );
    assert_context_pack_response_has_no_forbidden_fields(&long.1);

    for invalid_uri in [
        "/api/v0/comment-research/voices/context-pack",
        "/api/v0/comment-research/voices/context-pack?workspace_id=voice-context-pack-proof-workspace",
        "/api/v0/comment-research/voices/context-pack?workspace_id=voice-context-pack-proof-workspace&evidence_id=not-a-uuid&record_index=0",
        "/api/v0/comment-research/voices/context-pack?workspace_id=voice-context-pack-proof-workspace&evidence_id=00000000-0000-0000-0000-000000000000",
        "/api/v0/comment-research/voices/context-pack?workspace_id=voice-context-pack-proof-workspace&evidence_id=00000000-0000-0000-0000-000000000000&record_index=-1",
        "/api/v0/comment-research/voices/context-pack?workspace_id=voice-context-pack-proof-workspace&evidence_id=00000000-0000-0000-0000-000000000000&record_index=not-an-integer",
    ] {
        let invalid = get_json(&app, invalid_uri).await;
        assert_eq!(invalid.0, StatusCode::BAD_REQUEST, "{invalid_uri}");
        assert_eq!(invalid.1["error"]["code"], "invalid_request");
    }

    let forged = get_json(&app, &context_pack_uri(Uuid::new_v4(), 0, WORKSPACE)).await;
    assert_eq!(forged.0, StatusCode::NOT_FOUND);
    assert_eq!(forged.1["error"]["code"], "comment_voice_not_found");

    assert_eq!(
        all_counts(&inspector).await,
        before_reads,
        "all Context Pack preview reads, including invalid locators, must remain zero-write"
    );

    // Advance the same identity through the direct comment source path. The old
    // locator must stop resolving even though its older source-backed context
    // still exists; the fresh locator gets a clean current derivation but no
    // text-matching context capture.
    drop(app);
    let mut store = match Arc::try_unwrap(shared_store) {
        Ok(store) => store,
        Err(_) => panic!("router must release its only store reference before mutation"),
    };
    let (changed_detail, changed_comments) = legacy_pair_for_identity(
        "changed-current",
        &observed_note_id,
        &observed_comment_id,
        "当前正文已经改变",
    );
    let changed_admission = store
        .admit_xhs_comment_capture_pair_v0(WORKSPACE, changed_detail, changed_comments)
        .await
        .expect("changed source must advance current direct observation");
    let app = comment_research_router_v0(Arc::new(store));

    let old_locator = get_json(&app, &observed_uri).await;
    assert_eq!(old_locator.0, StatusCode::NOT_FOUND);
    assert_eq!(old_locator.1["error"]["code"], "comment_voice_not_found");

    let changed = get_json(
        &app,
        &context_pack_uri(changed_admission.comments_evidence.id, 0, WORKSPACE),
    )
    .await;
    assert_eq!(changed.0, StatusCode::OK);
    assert_eq!(
        changed.1["direct_comment_evidence"],
        json!({"text": "当前正文已经改变", "truncated": false})
    );
    assert_eq!(
        changed.1["discussion_context"]["availability"],
        "unavailable"
    );
    assert_eq!(changed.1["work_context"]["availability"], "unavailable");
}

#[derive(Clone, Copy)]
enum ContextFixtureMode {
    Observed,
    BlankTitleUnavailableBody,
    LongUnicode,
}

fn context_fixture_variant(
    suffix: &str,
    mode: ContextFixtureMode,
) -> (CapturePackageV0, CapturePackageV0, CapturePackageV0) {
    let fixture: Value = serde_json::from_str(CONTEXT_FIXTURE).expect("context fixture parses");
    let packages = fixture["packages"]
        .as_object()
        .expect("context packages object");
    let mut detail: CapturePackageV0 =
        serde_json::from_value(packages["detailPackage"].clone()).expect("detail package");
    let mut comments: CapturePackageV0 =
        serde_json::from_value(packages["commentsPackage"].clone()).expect("comments package");
    let mut replies: CapturePackageV0 =
        serde_json::from_value(packages["repliesPackage"].clone()).expect("replies package");

    let note_id = format!("context-pack-note-{suffix}");
    let comment_id = format!("context-pack-comment-{suffix}");
    detail.records[0].payload["noteId"] = json!(note_id);
    comments.records[0].payload["noteId"] = json!(note_id);
    comments.records[0].payload["commentId"] = json!(comment_id);
    comments.records[0].payload["text"] = json!(format!("context direct voice {suffix}"));

    for (index, record) in replies.records.iter_mut().enumerate() {
        record.payload["noteId"] = json!(note_id);
        record.payload["commentId"] = json!(format!("context-pack-reply-{suffix}-{index}"));
        record.payload["text"] = json!(format!("context related reply {suffix} {index}"));
        record.payload["rootCommentId"] = json!(comment_id);
        record.payload["parentCommentId"] = json!(comment_id);
        record.payload["replyToCommentId"] = json!(comment_id);
    }

    match mode {
        ContextFixtureMode::Observed => {}
        ContextFixtureMode::BlankTitleUnavailableBody => {
            detail.records[0].payload["title"] = json!("");
            detail.records[0]
                .payload
                .as_object_mut()
                .expect("detail payload object")
                .remove("bodyText");
        }
        ContextFixtureMode::LongUnicode => {
            comments.records[0].payload["text"] = json!("中🚀".repeat(600));
            detail.records[0].payload["title"] = json!("题🚀".repeat(300));
            detail.records[0].payload["bodyText"] = json!("正文🙂".repeat(300));
            for (index, record) in replies.records.iter_mut().enumerate() {
                record.payload["text"] = json!(format!("讨论{index}{}", "语🙂".repeat(250)));
            }
            let extra_template = replies.records[0].clone();
            for index in 2..5 {
                let mut extra = extra_template.clone();
                extra.payload["commentId"] = json!(format!("context-pack-reply-{suffix}-{index}"));
                extra.payload["text"] = json!(format!("额外讨论{index}{}", "语🙂".repeat(250)));
                extra.payload["rootCommentId"] = json!(comment_id);
                extra.payload["parentCommentId"] = json!(comment_id);
                extra.payload["replyToCommentId"] = json!(comment_id);
                replies.records.push(extra);
            }
        }
    }

    detail.extensions.insert(
        "packageRef".to_owned(),
        json!(format!("context-pack-detail-{suffix}")),
    );
    comments.extensions.insert(
        "packageRef".to_owned(),
        json!(format!("context-pack-comments-{suffix}")),
    );
    replies.extensions.insert(
        "packageRef".to_owned(),
        json!(format!("context-pack-replies-{suffix}")),
    );
    (detail, comments, replies)
}

fn legacy_pair_for_identity(
    suffix: &str,
    note_id: &str,
    comment_id: &str,
    text: &str,
) -> (CapturePackageV0, CapturePackageV0) {
    let fixture: Value = serde_json::from_str(PAIR_FIXTURE).expect("pair fixture parses");
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
    comments.records[0].payload["text"] = json!(text);
    detail.extensions.insert(
        "packageRef".to_owned(),
        json!(format!("context-pack-legacy-detail-{suffix}")),
    );
    comments.extensions.insert(
        "packageRef".to_owned(),
        json!(format!("context-pack-legacy-comments-{suffix}")),
    );
    (detail, comments)
}

fn source_text(package: &CapturePackageV0, record_index: usize, key: &str) -> String {
    package.records[record_index].payload[key]
        .as_str()
        .expect("fixture source field must be a string")
        .to_owned()
}

fn context_pack_uri(evidence_id: Uuid, record_index: i32, workspace_id: &str) -> String {
    format!(
        "/api/v0/comment-research/voices/context-pack?workspace_id={workspace_id}&evidence_id={evidence_id}&record_index={record_index}"
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
        "context_work_record_v0",
        "context_discussion_record_v0",
        "source_context_capture_set_v0",
    ];
    let mut counts = Vec::with_capacity(tables.len());
    for table in tables {
        let row = inspector
            .query_one(&format!("SELECT count(*) FROM {table}"), &[])
            .await
            .expect("table count query succeeds");
        counts.push(row.get(0));
    }
    counts
}

fn assert_context_pack_response_has_no_forbidden_fields(value: &Value) {
    const FORBIDDEN: &[&str] = &[
        "author",
        "author_id",
        "author_name",
        "comment_id",
        "note_id",
        "url",
        "raw_url",
        "likes",
        "published_at",
        "published_time",
        "admitted_at",
        "ocr",
        "asr",
        "media",
        "raw_text",
        "original_text",
        "original_voice_text",
        "source_evidence",
        "source_payload",
        "reason_codes",
        "derivation",
        "derivation_id",
        "fingerprint",
        "prompt",
        "system",
        "model",
        "tokens",
        "cost",
    ];
    match value {
        Value::Object(object) => {
            for (key, nested) in object {
                assert!(
                    !FORBIDDEN.contains(&key.as_str()),
                    "Context Pack DTO must not expose forbidden field {key}"
                );
                assert_context_pack_response_has_no_forbidden_fields(nested);
            }
        }
        Value::Array(values) => {
            for nested in values {
                assert_context_pack_response_has_no_forbidden_fields(nested);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}
