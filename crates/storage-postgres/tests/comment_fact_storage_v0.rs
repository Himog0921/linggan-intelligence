use std::sync::Arc;

use linggan_contracts::CapturePackageV0;
use linggan_storage_postgres::{CommentFactStore, CommentProjectionDispositionV0, StorageError};
use serde_json::{Value, json};
use tokio::sync::Barrier;
use tokio_postgres::{Client, NoTls, error::SqlState};

const FIXTURE: &str = include_str!("../../../fixtures/xhs/comment-evidence-set-v1.json");
const WORKSPACE: &str = "proof-workspace";

/// This test has no fallback database. Run it through
/// scripts/prove-comment-fact-storage-v0.sh, which creates and removes an
/// isolated PostgreSQL container with a random local endpoint.
#[tokio::test]
#[ignore = "requires scripts/prove-comment-fact-storage-v0.sh to create an isolated PostgreSQL database"]
async fn proves_comment_fact_v0_against_an_isolated_postgres_database() {
    let database_url = std::env::var("LINGGAN_COMMENT_STORAGE_TEST_DATABASE_URL")
        .expect("proof script must provide an isolated PostgreSQL database URL");

    let mut store = CommentFactStore::connect(&database_url)
        .await
        .expect("isolated proof database must connect");
    store
        .apply_comment_fact_storage_v0_migration()
        .await
        .expect("greenfield migration must apply");

    let inspector = connect_inspector(&database_url).await;

    let (detail, mut comments) = fixture_packages();
    set_comment_text(&mut comments, "   ");
    let invalid = store
        .admit_xhs_comment_capture_pair_v0(WORKSPACE, detail, comments)
        .await
        .expect_err("source contract must reject blank text before storage");
    assert!(matches!(
        invalid,
        StorageError::Preparation(linggan_evidence::EvidencePreparationError::Source(_))
    ));
    assert_all_counts(&inspector, [0, 0, 0, 0, 0, 0]).await;

    let (mut detail, comments) = fixture_packages();
    detail
        .extensions
        .insert("unknownExtension".into(), json!("unsafe\u{0000}value"));
    let nul_error = store
        .admit_xhs_comment_capture_pair_v0(WORKSPACE, detail, comments)
        .await
        .expect_err("PostgreSQL-incompatible JSON must be rejected before a write");
    assert!(matches!(
        nul_error,
        StorageError::Preparation(linggan_evidence::EvidencePreparationError::PostgresJsonbNul)
    ));
    assert_all_counts(&inspector, [0, 0, 0, 0, 0, 0]).await;

    let (detail, comments) = fixture_packages();
    let initial = store
        .admit_xhs_comment_capture_pair_v0(WORKSPACE, detail.clone(), comments.clone())
        .await
        .expect("validated fixture must admit");
    assert!(initial.detail_evidence.inserted);
    assert!(initial.comments_evidence.inserted);
    let initial_identity = initial.comments[0].identity.clone();
    let initial_observation_id = created_observation_id(&initial.comments[0].disposition);
    assert!(initial.source_capture_pair.inserted);
    assert_all_counts(&inspector, [2, 1, 1, 1, 1, 1]).await;

    let exact_replay = store
        .admit_xhs_comment_capture_pair_v0(WORKSPACE, detail.clone(), comments.clone())
        .await
        .expect("same immutable source pair must replay idempotently");
    assert!(!exact_replay.detail_evidence.inserted);
    assert!(!exact_replay.comments_evidence.inserted);
    assert!(!exact_replay.source_capture_pair.inserted);
    assert!(matches!(
        exact_replay.comments[0].disposition,
        CommentProjectionDispositionV0::PreviouslyAdmittedSource { observation_id }
            if observation_id == initial_observation_id
    ));
    assert_all_counts(&inspector, [2, 1, 1, 1, 1, 1]).await;

    let same_body_new_source = change_comments_package_marker(comments.clone(), "second-source");
    let same_body = store
        .admit_xhs_comment_capture_pair_v0(WORKSPACE, detail.clone(), same_body_new_source)
        .await
        .expect("new source package with same body must preserve current");
    assert!(!same_body.detail_evidence.inserted);
    assert!(same_body.comments_evidence.inserted);
    assert!(same_body.source_capture_pair.inserted);
    assert!(matches!(
        same_body.comments[0].disposition,
        CommentProjectionDispositionV0::ReplayUnchanged
    ));
    assert_all_counts(&inspector, [3, 2, 1, 2, 1, 1]).await;

    let mut changed_body_new_source =
        change_comments_package_marker(comments.clone(), "third-source");
    set_comment_text(&mut changed_body_new_source, "changed fixture comment text");
    let changed = store
        .admit_xhs_comment_capture_pair_v0(WORKSPACE, detail.clone(), changed_body_new_source)
        .await
        .expect("changed body must append an observation and advance current");
    let changed_observation_id = match changed.comments[0].disposition {
        CommentProjectionDispositionV0::Advanced {
            previous_observation_id,
            observation_id,
        } => {
            assert_eq!(previous_observation_id, initial_observation_id);
            observation_id
        }
        ref other => panic!("expected current advance, received {other:?}"),
    };
    assert_ne!(changed_observation_id, initial_observation_id);
    assert_all_counts(&inspector, [4, 3, 1, 3, 2, 1]).await;
    assert_eq!(
        current_observation_id(
            &inspector,
            WORKSPACE,
            &initial_identity.note_id,
            &initial_identity.comment_id
        )
        .await,
        changed_observation_id
    );

    let different_detail_same_comments =
        change_detail_package_marker(detail.clone(), "different-detail");
    let traced_pair = store
        .admit_xhs_comment_capture_pair_v0(
            WORKSPACE,
            different_detail_same_comments,
            comments.clone(),
        )
        .await
        .expect("same comments with a distinct detail package must preserve pair provenance");
    assert!(traced_pair.detail_evidence.inserted);
    assert!(!traced_pair.comments_evidence.inserted);
    assert!(traced_pair.source_capture_pair.inserted);
    assert!(matches!(
        traced_pair.comments[0].disposition,
        CommentProjectionDispositionV0::PreviouslyAdmittedSource { .. }
    ));
    assert_all_counts(&inspector, [5, 4, 1, 3, 2, 1]).await;

    let old_source_after_change = store
        .admit_xhs_comment_capture_pair_v0(WORKSPACE, detail.clone(), comments.clone())
        .await
        .expect("old immutable source must not roll current backward");
    assert!(matches!(
        old_source_after_change.comments[0].disposition,
        CommentProjectionDispositionV0::PreviouslyAdmittedSource { observation_id }
            if observation_id == initial_observation_id
    ));
    assert_eq!(
        current_observation_id(
            &inspector,
            WORKSPACE,
            &initial_identity.note_id,
            &initial_identity.comment_id
        )
        .await,
        changed_observation_id
    );

    let mut other_detail = detail.clone();
    let mut other_comments = change_comments_package_marker(comments.clone(), "other-note-source");
    replace_note_id(&mut other_detail, &mut other_comments, "noteId-fixture-002");
    let cross_work = store
        .admit_xhs_comment_capture_pair_v0(WORKSPACE, other_detail, other_comments)
        .await
        .expect("same comment id under a different note must be a different identity");
    assert!(matches!(
        cross_work.comments[0].disposition,
        CommentProjectionDispositionV0::Created { .. }
    ));
    assert_all_counts(&inspector, [7, 5, 2, 4, 3, 2]).await;

    let concurrent_one = capture_pair_with_text(
        detail.clone(),
        comments.clone(),
        "concurrent-detail-one",
        "concurrent-comments-one",
        "concurrent body one",
    );
    let concurrent_two = capture_pair_with_text(
        detail,
        comments,
        "concurrent-detail-two",
        "concurrent-comments-two",
        "concurrent body two",
    );
    let barrier = Arc::new(Barrier::new(3));
    let first = tokio::spawn(admit_after_barrier(
        database_url.clone(),
        barrier.clone(),
        concurrent_one.0,
        concurrent_one.1,
    ));
    let second = tokio::spawn(admit_after_barrier(
        database_url.clone(),
        barrier.clone(),
        concurrent_two.0,
        concurrent_two.1,
    ));
    barrier.wait().await;
    let first = first
        .await
        .expect("first concurrent task must not panic")
        .expect("first concurrent admission");
    let second = second
        .await
        .expect("second concurrent task must not panic")
        .expect("second concurrent admission");
    assert!(matches!(
        first.comments[0].disposition,
        CommentProjectionDispositionV0::Advanced { .. }
    ));
    assert!(matches!(
        second.comments[0].disposition,
        CommentProjectionDispositionV0::Advanced { .. }
    ));
    assert_all_counts(&inspector, [11, 7, 2, 6, 5, 2]).await;
    assert_current_is_latest_local_admission(
        &inspector,
        WORKSPACE,
        &initial_identity.note_id,
        &initial_identity.comment_id,
    )
    .await;

    let pair_relationship_violation = inspector
        .execute(
            "INSERT INTO source_capture_pair_v0 (id, workspace_id, detail_evidence_id, comments_evidence_id) \
             SELECT $1, 'other-workspace', detail.id, comments.id \
             FROM source_evidence_v0 AS detail CROSS JOIN source_evidence_v0 AS comments \
             WHERE detail.package_kind = 'content_detail' AND comments.package_kind = 'comments' \
             LIMIT 1",
            &[&uuid::Uuid::new_v4()],
        )
        .await
        .expect_err("Source pair must reference Evidence from its declared workspace");
    assert_protected_history_rejection(pair_relationship_violation);

    let evidence_mutation = inspector
        .execute(
            "UPDATE source_evidence_v0 SET package_kind = 'comments' WHERE id = (SELECT id FROM source_evidence_v0 LIMIT 1)",
            &[],
        )
        .await
        .expect_err("Evidence must reject UPDATE");
    assert_protected_history_rejection(evidence_mutation);

    let evidence_delete = inspector
        .execute(
            "DELETE FROM source_evidence_v0 WHERE id = (SELECT id FROM source_evidence_v0 LIMIT 1)",
            &[],
        )
        .await
        .expect_err("Evidence must reject DELETE");
    assert_protected_history_rejection(evidence_delete);

    let pair_mutation = inspector
        .execute(
            "UPDATE source_capture_pair_v0 SET workspace_id = 'tampered' \
             WHERE id = (SELECT id FROM source_capture_pair_v0 LIMIT 1)",
            &[],
        )
        .await
        .expect_err("Source capture pair must reject UPDATE");
    assert_protected_history_rejection(pair_mutation);

    let observation_mutation = inspector
        .execute(
            "DELETE FROM comment_observation_v0 WHERE id = (SELECT id FROM comment_observation_v0 LIMIT 1)",
            &[],
        )
        .await
        .expect_err("CommentObservation must reject DELETE");
    assert_protected_history_rejection(observation_mutation);

    let observation_update = inspector
        .execute(
            "UPDATE comment_observation_v0 SET source_text = 'tampered' \
             WHERE id = (SELECT id FROM comment_observation_v0 LIMIT 1)",
            &[],
        )
        .await
        .expect_err("CommentObservation must reject UPDATE");
    assert_protected_history_rejection(observation_update);

    let initial_text_sha256 = observation_text_sha256(&inspector, initial_observation_id).await;
    let backwards_current = inspector
        .execute(
            "UPDATE comment_current_v0 SET current_observation_id = $1, text_sha256 = $2 \
             WHERE workspace_id = $3 AND platform = 'xhs' AND note_id = $4 AND comment_id = $5",
            &[
                &initial_observation_id,
                &initial_text_sha256,
                &WORKSPACE,
                &initial_identity.note_id,
                &initial_identity.comment_id,
            ],
        )
        .await
        .expect_err("Current must not move back to an earlier local admission");
    assert_protected_history_rejection(backwards_current);

    let invalid_current_mutation = inspector
        .execute(
            "UPDATE comment_current_v0 SET text_sha256 = repeat('0', 64) \
             WHERE workspace_id = $1 AND platform = 'xhs' AND note_id = $2 AND comment_id = $3",
            &[
                &WORKSPACE,
                &initial_identity.note_id,
                &initial_identity.comment_id,
            ],
        )
        .await
        .expect_err("Current must materialize its referenced immutable observation");
    assert_protected_history_rejection(invalid_current_mutation);
    assert_all_counts(&inspector, [11, 7, 2, 6, 5, 2]).await;
}

fn assert_protected_history_rejection(error: tokio_postgres::Error) {
    let database_error = error
        .as_db_error()
        .expect("PostgreSQL trigger must return a database error");
    assert_eq!(
        database_error.code(),
        &SqlState::OBJECT_NOT_IN_PREREQUISITE_STATE
    );
}

async fn connect_inspector(database_url: &str) -> Client {
    let (client, connection) = tokio_postgres::connect(database_url, NoTls)
        .await
        .expect("inspector must connect to isolated proof database");
    tokio::spawn(async move {
        connection
            .await
            .expect("proof database connection must stay healthy");
    });
    client
}

fn fixture_packages() -> (CapturePackageV0, CapturePackageV0) {
    let fixture: Value = serde_json::from_str(FIXTURE).expect("fixture must parse");
    let packages = fixture["packages"].as_object().expect("fixture packages");
    (
        serde_json::from_value(packages["detailPackage"].clone()).expect("detail package"),
        serde_json::from_value(packages["commentsPackage"].clone()).expect("comments package"),
    )
}

fn change_comments_package_marker(
    mut comments: CapturePackageV0,
    marker: &str,
) -> CapturePackageV0 {
    comments.extensions.insert(
        "observedAt".to_owned(),
        json!(format!("2026-09-13T00:00:00Z::{marker}")),
    );
    comments
}

fn change_detail_package_marker(mut detail: CapturePackageV0, marker: &str) -> CapturePackageV0 {
    detail.extensions.insert(
        "observedAt".to_owned(),
        json!(format!("2026-09-13T00:00:00Z::{marker}")),
    );
    detail
}

fn capture_pair_with_text(
    detail: CapturePackageV0,
    comments: CapturePackageV0,
    detail_marker: &str,
    comments_marker: &str,
    text: &str,
) -> (CapturePackageV0, CapturePackageV0) {
    let detail = change_detail_package_marker(detail, detail_marker);
    let mut comments = change_comments_package_marker(comments, comments_marker);
    set_comment_text(&mut comments, text);
    (detail, comments)
}

async fn admit_after_barrier(
    database_url: String,
    barrier: Arc<Barrier>,
    detail: CapturePackageV0,
    comments: CapturePackageV0,
) -> Result<linggan_storage_postgres::CommentFactAdmissionOutcomeV0, StorageError> {
    let mut store = CommentFactStore::connect(&database_url).await?;
    barrier.wait().await;
    store
        .admit_xhs_comment_capture_pair_v0(WORKSPACE, detail, comments)
        .await
}

fn set_comment_text(comments: &mut CapturePackageV0, text: &str) {
    comments.records[0].payload["text"] = json!(text);
}

fn replace_note_id(detail: &mut CapturePackageV0, comments: &mut CapturePackageV0, note_id: &str) {
    detail.records[0].payload["noteId"] = json!(note_id);
    comments.records[0].payload["noteId"] = json!(note_id);
}

fn created_observation_id(disposition: &CommentProjectionDispositionV0) -> uuid::Uuid {
    match disposition {
        CommentProjectionDispositionV0::Created { observation_id } => *observation_id,
        other => panic!("expected initial current creation, received {other:?}"),
    }
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

async fn current_observation_id(
    inspector: &Client,
    workspace_id: &str,
    note_id: &str,
    comment_id: &str,
) -> uuid::Uuid {
    let row = inspector
        .query_one(
            "SELECT current_observation_id FROM comment_current_v0 \
             WHERE workspace_id = $1 AND platform = 'xhs' AND note_id = $2 AND comment_id = $3",
            &[&workspace_id, &note_id, &comment_id],
        )
        .await
        .expect("current projection row");
    row.get(0)
}

async fn observation_text_sha256(inspector: &Client, observation_id: uuid::Uuid) -> String {
    let row = inspector
        .query_one(
            "SELECT text_sha256 FROM comment_observation_v0 WHERE id = $1",
            &[&observation_id],
        )
        .await
        .expect("immutable observation row");
    row.get(0)
}

async fn assert_current_is_latest_local_admission(
    inspector: &Client,
    workspace_id: &str,
    note_id: &str,
    comment_id: &str,
) {
    let row = inspector
        .query_one(
            "SELECT current_observation.admission_sequence, max_observation.max_sequence \
             FROM comment_current_v0 AS current_projection \
             JOIN comment_observation_v0 AS current_observation \
               ON current_observation.id = current_projection.current_observation_id \
             CROSS JOIN LATERAL ( \
               SELECT max(admission_sequence) AS max_sequence \
               FROM comment_observation_v0 \
               WHERE workspace_id = $1 AND platform = 'xhs' AND note_id = $2 AND comment_id = $3 \
             ) AS max_observation \
             WHERE current_projection.workspace_id = $1 AND current_projection.platform = 'xhs' \
               AND current_projection.note_id = $2 AND current_projection.comment_id = $3",
            &[&workspace_id, &note_id, &comment_id],
        )
        .await
        .expect("current projection must point to an observation");
    let current_sequence: i64 = row.get(0);
    let maximum_sequence: i64 = row.get(1);
    assert_eq!(
        current_sequence, maximum_sequence,
        "Current must point to the last locally admitted observation for its identity"
    );
}
