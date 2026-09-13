use std::sync::Arc;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use linggan_api::comment_research_router_v0;
use linggan_contracts::CapturePackageV0;
use linggan_storage_postgres::{
    CommentFactStore, CommentProjectionDispositionV0, CurrentCommentVoicesPageRequestV0,
};
use serde_json::{Value, json};
use tokio_postgres::{Client, NoTls, error::SqlState};
use tower::ServiceExt;
use uuid::Uuid;

const FIXTURE: &str = include_str!("../../../fixtures/xhs/comment-evidence-set-v1.json");
const WORKSPACE: &str = "derivation-voices-v1-proof-workspace";

/// This proof is intentionally run only by the companion script. The script
/// starts an empty postgres:16 container on a random loopback port; no shared
/// database, model provider, vector service, or network source is consulted.
#[tokio::test]
#[ignore = "requires scripts/prove-comment-derivation-user-voices-v1.sh"]
async fn proves_deterministic_comment_derivation_and_filtered_user_voices_against_isolated_postgres()
 {
    let database_url = std::env::var("LINGGAN_COMMENT_DERIVATION_V1_TEST_DATABASE_URL")
        .expect("proof script must provide an isolated PostgreSQL database URL");
    let mut store = CommentFactStore::connect(&database_url)
        .await
        .expect("isolated proof database must connect");
    store
        .apply_comment_fact_storage_v0_migration()
        .await
        .expect("fact baseline migration must apply");
    let inspector = connect_inspector(&database_url).await;

    // A current observation that predates 0003 has no derivation until the
    // explicitly invoked, bounded local materializer handles it.
    let legacy_observation_id = seed_legacy_current_without_derivation(&inspector).await;
    store
        .apply_comment_derivation_v1_migration()
        .await
        .expect("derivation migration must apply after its fact baseline");

    let before_materialization = store
        .list_current_comment_voices_v0(
            WORKSPACE,
            CurrentCommentVoicesPageRequestV0::new(50, 0).expect("bounded page"),
        )
        .await
        .expect("pre-materialization read must succeed");
    assert_eq!(before_materialization.total, 0);
    assert_eq!(before_materialization.awaiting_cleaning_total, 1);

    let materialized = store
        .materialize_missing_current_comment_derivations_v1(WORKSPACE, 1)
        .await
        .expect("one legacy current observation must materialize");
    assert_eq!(materialized.materialized_current_observations, 1);
    let idempotent = store
        .materialize_missing_current_comment_derivations_v1(WORKSPACE, 1)
        .await
        .expect("second materialization must be idempotent");
    assert_eq!(idempotent.materialized_current_observations, 0);
    assert_eq!(
        derivation_count_for_observation(&inspector, legacy_observation_id).await,
        1
    );

    // V1 supports a future contract beside V1, but the visible read is pinned
    // to V1 and therefore cannot mix a later replay into the current corpus.
    inspector
        .execute(
            "INSERT INTO comment_derivation_v1 \
             (id, comment_observation_id, cleaning_contract, research_state, research_text, reason_codes) \
             VALUES ($1, $2, 'comment-cleaning.v2', 'analyzable', 'future contract text', ARRAY[]::TEXT[])",
            &[&Uuid::new_v4(), &legacy_observation_id],
        )
        .await
        .expect("a later contract must coexist for one immutable observation");
    assert_eq!(
        derivation_count_for_observation(&inspector, legacy_observation_id).await,
        2
    );

    let (needs_detail, needs_comments) =
        fixture_pair("needs-context", "needs-note", "needs-comment", "同问");
    store
        .admit_xhs_comment_capture_pair_v0(WORKSPACE, needs_detail, needs_comments)
        .await
        .expect("short semantic comment must be retained as needs-context");

    let (dropped_detail, dropped_comments) =
        fixture_pair("dropped", "dropped-note", "dropped-comment", "😂❤️");
    store
        .admit_xhs_comment_capture_pair_v0(WORKSPACE, dropped_detail, dropped_comments)
        .await
        .expect("pure emoji still admits the immutable fact and derives a drop");

    let (anomaly_detail, anomaly_comments) = fixture_pair(
        "anomaly",
        "anomaly-note",
        "anomaly-comment",
        "保留原始事实\u{0007}",
    );
    store
        .admit_xhs_comment_capture_pair_v0(WORKSPACE, anomaly_detail, anomaly_comments)
        .await
        .expect("control-character source text stores an anomaly derivation without mutation");

    let (version_one_detail, version_one_comments) = fixture_pair(
        "version-one",
        "version-note",
        "version-comment",
        "@author 😀 版本相同",
    );
    let initial = store
        .admit_xhs_comment_capture_pair_v0(
            WORKSPACE,
            version_one_detail.clone(),
            version_one_comments.clone(),
        )
        .await
        .expect("initial versioned raw body must admit");
    let initial_observation_id = created_observation_id(&initial.comments[0].disposition);
    let initial_evidence_id = initial.comments_evidence.id;
    assert_eq!(
        derived_research_text(&inspector, initial_observation_id).await,
        "版本相同"
    );

    let replay = store
        .admit_xhs_comment_capture_pair_v0(
            WORKSPACE,
            version_one_detail,
            version_one_comments.clone(),
        )
        .await
        .expect("exact immutable source replay must not derive again");
    assert!(matches!(
        replay.comments[0].disposition,
        CommentProjectionDispositionV0::PreviouslyAdmittedSource { observation_id }
            if observation_id == initial_observation_id
    ));
    assert_eq!(
        derivation_count_for_observation(&inspector, initial_observation_id).await,
        1
    );

    let (same_body_detail, same_body_comments) = fixture_pair(
        "same-raw-new-evidence",
        "version-note",
        "version-comment",
        "@author 😀 版本相同",
    );
    let same_body = store
        .admit_xhs_comment_capture_pair_v0(WORKSPACE, same_body_detail, same_body_comments)
        .await
        .expect("new Evidence with the same raw body must not duplicate an observation");
    assert!(matches!(
        same_body.comments[0].disposition,
        CommentProjectionDispositionV0::ReplayUnchanged
    ));
    assert_eq!(
        derivation_count_for_observation(&inspector, initial_observation_id).await,
        1
    );

    let (changed_detail, changed_comments) = fixture_pair(
        "changed-raw-same-cleaned",
        "version-note",
        "version-comment",
        "版本相同",
    );
    let changed = store
        .admit_xhs_comment_capture_pair_v0(WORKSPACE, changed_detail, changed_comments)
        .await
        .expect(
            "a changed raw body must append an observation even when cleaning yields equal text",
        );
    let changed_observation_id = advanced_observation_id(&changed.comments[0].disposition);
    assert_ne!(changed_observation_id, initial_observation_id);
    assert_eq!(
        derived_research_text(&inspector, changed_observation_id).await,
        "版本相同"
    );
    assert_eq!(
        derivation_count_for_observation(&inspector, initial_observation_id).await,
        1
    );
    assert_eq!(
        derivation_count_for_observation(&inspector, changed_observation_id).await,
        1
    );

    let derivation_update = inspector
        .execute(
            "UPDATE comment_derivation_v1 SET research_text = 'tampered' WHERE comment_observation_id = $1 AND cleaning_contract = 'comment-cleaning.v1'",
            &[&changed_observation_id],
        )
        .await
        .expect_err("derived text history must be append-only");
    assert_protected_history_rejection(derivation_update);
    let derivation_delete = inspector
        .execute(
            "DELETE FROM comment_derivation_v1 WHERE comment_observation_id = $1 AND cleaning_contract = 'comment-cleaning.v1'",
            &[&changed_observation_id],
        )
        .await
        .expect_err("derivation history must reject deletion");
    assert_protected_history_rejection(derivation_delete);
    let integrity = inspector
        .query_one(
            "SELECT research_text_integrity_md5 = md5(research_text) \
               FROM comment_derivation_v1 \
              WHERE comment_observation_id = $1 AND cleaning_contract = 'comment-cleaning.v1'",
            &[&changed_observation_id],
        )
        .await
        .expect("integrity check row");
    assert!(integrity.get::<_, bool>(0));
    let invalid_derivation = inspector
        .execute(
            "INSERT INTO comment_derivation_v1 \
             (id, comment_observation_id, cleaning_contract, research_state, research_text, reason_codes) \
             VALUES ($1, $2, 'comment-cleaning.v3', 'analyzable', NULL, ARRAY[]::TEXT[])",
            &[&Uuid::new_v4(), &changed_observation_id],
        )
        .await
        .expect_err("migration must reject a state without its required derived text");
    assert_eq!(
        invalid_derivation
            .as_db_error()
            .expect("constraint rejection must be PostgreSQL")
            .code(),
        &SqlState::CHECK_VIOLATION
    );

    let app = comment_research_router_v0(Arc::new(store));
    let counts_before_reads = all_counts(&inspector).await;

    let available = get_json(
        &app,
        "/api/v0/comment-research/voices?workspace_id=derivation-voices-v1-proof-workspace&limit=50&offset=0",
    )
    .await;
    assert_eq!(available.0, StatusCode::OK);
    assert_eq!(
        available.1["pagination"],
        json!({"total": 3, "limit": 50, "offset": 0})
    );
    assert_eq!(
        available.1["preparation"],
        json!({"filter": "available", "awaiting_cleaning_total": 0})
    );
    let available_voices = available.1["voices"].as_array().expect("available voices");
    assert_eq!(available_voices.len(), 3);
    assert!(
        available_voices
            .iter()
            .any(|voice| voice["readiness"] == "ready")
    );
    assert!(
        available_voices
            .iter()
            .any(|voice| voice["readiness"] == "needs_context")
    );
    assert!(
        available_voices
            .iter()
            .any(|voice| voice["research_text"] == "版本相同")
    );
    assert!(
        available_voices
            .iter()
            .all(|voice| voice["research_text"] != "@author 😀 版本相同")
    );
    let version_voice = available_voices
        .iter()
        .find(|voice| voice["source_note_id"] == "version-note")
        .expect("changed current comment must remain visible through its new Evidence locator");
    assert_eq!(
        version_voice["source_evidence"]["evidence_id"],
        changed.comments_evidence.id.to_string(),
        "the old raw body derivation must not appear as the current voice"
    );
    for voice in available_voices {
        assert_user_voices_dto_is_private(voice);
    }

    let ready = get_json(
        &app,
        "/api/v0/comment-research/voices?workspace_id=derivation-voices-v1-proof-workspace&filter=ready&limit=50&offset=0",
    )
    .await;
    assert_eq!(ready.0, StatusCode::OK);
    assert_eq!(ready.1["pagination"]["total"], 2);
    assert!(
        ready.1["voices"]
            .as_array()
            .expect("ready rows")
            .iter()
            .all(|voice| voice["readiness"] == "ready")
    );

    let needs_context = get_json(
        &app,
        "/api/v0/comment-research/voices?workspace_id=derivation-voices-v1-proof-workspace&filter=needs_context&limit=50&offset=0",
    )
    .await;
    assert_eq!(needs_context.0, StatusCode::OK);
    assert_eq!(needs_context.1["pagination"]["total"], 1);
    assert_eq!(needs_context.1["voices"][0]["research_text"], "同问");

    let bad_filter = get_json(
        &app,
        "/api/v0/comment-research/voices?workspace_id=derivation-voices-v1-proof-workspace&filter=dropped",
    )
    .await;
    assert_eq!(bad_filter.0, StatusCode::BAD_REQUEST);
    assert_eq!(bad_filter.1["error"]["code"], "invalid_request");

    assert_eq!(
        all_counts(&inspector).await,
        counts_before_reads,
        "all filtered API reads must be pure: no admission, derivation, model, or network side effect"
    );
    assert_ne!(initial_evidence_id, changed.comments_evidence.id);
}

fn fixture_pair(
    suffix: &str,
    note_id: &str,
    comment_id: &str,
    text: &str,
) -> (CapturePackageV0, CapturePackageV0) {
    let fixture: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    let packages = fixture["packages"].as_object().expect("fixture packages");
    let mut detail: CapturePackageV0 =
        serde_json::from_value(packages["detailPackage"].clone()).expect("detail package");
    let mut comments: CapturePackageV0 =
        serde_json::from_value(packages["commentsPackage"].clone()).expect("comments package");
    detail.records[0].payload["noteId"] = json!(note_id);
    comments.records[0].payload["noteId"] = json!(note_id);
    comments.records[0].payload["commentId"] = json!(comment_id);
    comments.records[0].payload["text"] = json!(text);
    detail
        .extensions
        .insert("packageRef".to_owned(), json!(format!("detail-{suffix}")));
    comments
        .extensions
        .insert("packageRef".to_owned(), json!(format!("comments-{suffix}")));
    (detail, comments)
}

async fn seed_legacy_current_without_derivation(inspector: &Client) -> Uuid {
    let evidence_id = Uuid::new_v4();
    let observation_id = Uuid::new_v4();
    let text = "@legacy 😀 待物化表达";
    let text_sha = "a".repeat(64);
    inspector
        .execute(
            "INSERT INTO source_evidence_v0 \
             (id, workspace_id, platform, source_contract, package_kind, payload_sha256, source_payload) \
             VALUES ($1, $2, 'xhs', 'xhs.comment-capture-source.v0', 'comments', repeat('b', 64), $3)",
            &[&evidence_id, &WORKSPACE, &json!({"legacy": true})],
        )
        .await
        .expect("legacy source Evidence seed");
    inspector
        .execute(
            "INSERT INTO comment_identity_v0 (workspace_id, platform, note_id, comment_id) \
             VALUES ($1, 'xhs', 'legacy-note', 'legacy-comment')",
            &[&WORKSPACE],
        )
        .await
        .expect("legacy identity seed");
    inspector
        .execute(
            "INSERT INTO evidence_comment_record_v0 \
             (source_evidence_id, source_record_index, workspace_id, platform, note_id, comment_id, text_sha256) \
             VALUES ($1, 0, $2, 'xhs', 'legacy-note', 'legacy-comment', $3)",
            &[&evidence_id, &WORKSPACE, &text_sha],
        )
        .await
        .expect("legacy source record seed");
    inspector
        .execute(
            "INSERT INTO comment_observation_v0 \
             (id, workspace_id, platform, note_id, comment_id, source_evidence_id, source_record_index, text_sha256, source_text) \
             VALUES ($1, $2, 'xhs', 'legacy-note', 'legacy-comment', $3, 0, $4, $5)",
            &[&observation_id, &WORKSPACE, &evidence_id, &text_sha, &text],
        )
        .await
        .expect("legacy observation seed");
    inspector
        .execute(
            "INSERT INTO comment_current_v0 \
             (workspace_id, platform, note_id, comment_id, current_observation_id, text_sha256) \
             VALUES ($1, 'xhs', 'legacy-note', 'legacy-comment', $2, $3)",
            &[&WORKSPACE, &observation_id, &text_sha],
        )
        .await
        .expect("legacy current seed");
    observation_id
}

async fn derivation_count_for_observation(inspector: &Client, observation_id: Uuid) -> i64 {
    inspector
        .query_one(
            "SELECT count(*) FROM comment_derivation_v1 WHERE comment_observation_id = $1",
            &[&observation_id],
        )
        .await
        .expect("derivation count")
        .get(0)
}

async fn derived_research_text(inspector: &Client, observation_id: Uuid) -> String {
    inspector
        .query_one(
            "SELECT research_text FROM comment_derivation_v1 \
              WHERE comment_observation_id = $1 AND cleaning_contract = 'comment-cleaning.v1'",
            &[&observation_id],
        )
        .await
        .expect("V1 derivation")
        .get(0)
}

fn created_observation_id(disposition: &CommentProjectionDispositionV0) -> Uuid {
    match disposition {
        CommentProjectionDispositionV0::Created { observation_id } => *observation_id,
        other => panic!("expected initial current observation, got {other:?}"),
    }
}

fn advanced_observation_id(disposition: &CommentProjectionDispositionV0) -> Uuid {
    match disposition {
        CommentProjectionDispositionV0::Advanced { observation_id, .. } => *observation_id,
        other => panic!("expected advanced current observation, got {other:?}"),
    }
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
    (
        status,
        serde_json::from_slice(&body).expect("JSON response"),
    )
}

async fn connect_inspector(database_url: &str) -> Client {
    let (client, connection) = tokio_postgres::connect(database_url, NoTls)
        .await
        .expect("inspector must connect");
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

fn assert_user_voices_dto_is_private(value: &Value) {
    let object = value.as_object().expect("voice DTO object");
    let mut keys = object.keys().map(String::as_str).collect::<Vec<_>>();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "current_admitted_at",
            "readiness",
            "research_text",
            "source_evidence",
            "source_note_id",
        ],
        "list DTO must not leak raw source text, cleaning reasons, derivation IDs, fingerprints, or unproven platform fields"
    );
    assert_no_forbidden_user_voice_field(value);
}

fn assert_no_forbidden_user_voice_field(value: &Value) {
    const FORBIDDEN: &[&str] = &[
        "author",
        "author_id",
        "comment_id",
        "derivation",
        "derivation_id",
        "fingerprint",
        "likes",
        "ocr",
        "asr",
        "media",
        "published_at",
        "published_time",
        "raw_text",
        "reason_codes",
        "source_text",
        "url",
    ];
    match value {
        Value::Object(object) => {
            for (key, nested) in object {
                assert!(
                    !FORBIDDEN.contains(&key.as_str()),
                    "User Voices DTO must not expose {key}"
                );
                assert_no_forbidden_user_voice_field(nested);
            }
        }
        Value::Array(values) => {
            for nested in values {
                assert_no_forbidden_user_voice_field(nested);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn assert_protected_history_rejection(error: tokio_postgres::Error) {
    let database_error = error
        .as_db_error()
        .expect("append-only rejection must come from PostgreSQL");
    assert_eq!(
        database_error.code(),
        &SqlState::OBJECT_NOT_IN_PREREQUISITE_STATE
    );
}
