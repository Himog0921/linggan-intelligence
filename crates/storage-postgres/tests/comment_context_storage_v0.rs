use linggan_contracts::CapturePackageV0;
use linggan_storage_postgres::{CommentFactStore, StorageError};
use serde_json::{Value, json};
use tokio_postgres::{Client, NoTls, error::SqlState};

const FIXTURE: &str = include_str!("../../../fixtures/xhs/comment-context-evidence-set-v0.json");
const WORKSPACE: &str = "context-proof-workspace";

/// This test has no fallback database. Run it only through
/// scripts/prove-comment-context-storage-v0.sh, which creates and removes an
/// isolated PostgreSQL container with a random local endpoint.
#[tokio::test]
#[ignore = "requires scripts/prove-comment-context-storage-v0.sh to create an isolated PostgreSQL database"]
async fn proves_comment_context_storage_v0_against_an_isolated_postgres_database() {
    let database_url = std::env::var("LINGGAN_COMMENT_CONTEXT_STORAGE_TEST_DATABASE_URL")
        .expect("proof script must provide an isolated PostgreSQL database URL");

    let mut store = CommentFactStore::connect(&database_url)
        .await
        .expect("isolated proof database must connect");
    store
        .apply_comment_fact_storage_v0_migration()
        .await
        .expect("fact baseline migration must apply");
    store
        .apply_comment_context_storage_v0_migration()
        .await
        .expect("context migration must apply after fact baseline");
    store
        .apply_comment_derivation_v1_migration()
        .await
        .expect("derivation migration must apply after fact baseline");
    let inspector = connect_inspector(&database_url).await;

    let (detail, comments, mut replies) = fixture_packages();
    replies.records[0].payload["noteId"] = json!("other-note");
    let invalid = store
        .admit_xhs_comment_context_capture_set_v0(WORKSPACE, detail, comments, replies)
        .await
        .expect_err("mismatched context packages must reject before storage");
    assert!(matches!(
        invalid,
        StorageError::Preparation(linggan_evidence::EvidencePreparationError::ContextSource(_))
    ));
    assert_all_counts(&inspector, [0, 0, 0, 0, 0, 0, 0, 0, 0]).await;

    let (detail, comments, replies) = fixture_packages();
    let note_id = comments.records[0].payload["noteId"]
        .as_str()
        .expect("fixture note id")
        .to_owned();
    let comment_id = comments.records[0].payload["commentId"]
        .as_str()
        .expect("fixture comment id")
        .to_owned();
    let initial = store
        .admit_xhs_comment_context_capture_set_v0(
            WORKSPACE,
            detail.clone(),
            comments.clone(),
            replies.clone(),
        )
        .await
        .expect("validated context fixture must admit atomically");
    assert!(initial.detail_evidence.inserted);
    assert!(initial.comments_evidence.inserted);
    assert!(initial.replies_evidence.inserted);
    assert!(initial.source_context_capture_set.inserted);
    assert_all_counts(&inspector, [3, 0, 1, 1, 1, 1, 1, 3, 1]).await;

    let before_read = all_counts(&inspector).await;
    let context = store
        .get_current_comment_context_detail_v0(WORKSPACE, &note_id, &comment_id)
        .await
        .expect("context read must succeed")
        .expect("matching context set must be available");
    assert_eq!(context.work_context.source_evidence.record_index, 0);
    assert_eq!(context.related_replies.len(), 2);
    assert!(
        context.related_replies.iter().any(|reply| {
            reply.parent_comment_id.is_some() && reply.reply_to_comment_id.is_some()
        })
    );
    assert_eq!(
        all_counts(&inspector).await,
        before_read,
        "context read must not write"
    );

    let replay = store
        .admit_xhs_comment_context_capture_set_v0(
            WORKSPACE,
            detail.clone(),
            comments.clone(),
            replies,
        )
        .await
        .expect("exact context replay must be idempotent");
    assert!(!replay.detail_evidence.inserted);
    assert!(!replay.comments_evidence.inserted);
    assert!(!replay.replies_evidence.inserted);
    assert!(!replay.source_context_capture_set.inserted);
    assert_all_counts(&inspector, [3, 0, 1, 1, 1, 1, 1, 3, 1]).await;

    // A legacy pair can still advance the direct Comment fact. It intentionally
    // has no Context Capture Set, so the old context hash must not attach to
    // this new current body merely because the comment ID stayed the same.
    let mut changed_comments = comments;
    changed_comments.extensions.insert(
        "contextProofChanged".to_owned(),
        json!("different-direct-observation"),
    );
    changed_comments.records[0].payload["text"] = json!("changed direct comment body");
    let changed = store
        .admit_xhs_comment_capture_pair_v0(WORKSPACE, detail, changed_comments)
        .await
        .expect("legacy pair must remain compatible after 0002");
    assert!(matches!(
        changed.comments[0].disposition,
        linggan_storage_postgres::CommentProjectionDispositionV0::Advanced { .. }
    ));
    assert!(
        store
            .get_current_comment_context_detail_v0(WORKSPACE, &note_id, &comment_id)
            .await
            .expect("stale context lookup must succeed")
            .is_none(),
        "an old context body must never attach to a changed current comment"
    );

    let legacy_pair_context_violation = inspector
        .execute(
            "INSERT INTO source_capture_pair_v0 (id, workspace_id, detail_evidence_id, comments_evidence_id) \
             VALUES ($1, $2, $3, $4)",
            &[
                &uuid::Uuid::new_v4(),
                &WORKSPACE,
                &initial.detail_evidence.id,
                &initial.comments_evidence.id,
            ],
        )
        .await
        .expect_err("legacy pair must reject context-contract Evidence");
    assert_protected_history_rejection(legacy_pair_context_violation);

    let legacy_context_set_violation = inspector
        .execute(
            "INSERT INTO source_context_capture_set_v0 \
             (id, workspace_id, platform, note_id, detail_evidence_id, comments_evidence_id, replies_evidence_id, comments_record_count, replies_record_count) \
             VALUES ($1, $2, 'xhs', $3, $4, $5, $6, 1, 2)",
            &[
                &uuid::Uuid::new_v4(),
                &WORKSPACE,
                &note_id,
                &changed.detail_evidence.id,
                &changed.comments_evidence.id,
                &initial.replies_evidence.id,
            ],
        )
        .await
        .expect_err("context set must reject legacy-contract Evidence");
    assert_protected_history_rejection(legacy_context_set_violation);

    let capture_set_workspace_violation = inspector
        .execute(
            "INSERT INTO source_context_capture_set_v0 \
             (id, workspace_id, platform, note_id, detail_evidence_id, comments_evidence_id, replies_evidence_id, comments_record_count, replies_record_count) \
             SELECT $1, 'other-workspace', 'xhs', $2, $3, $4, $5, 1, 2",
            &[
                &uuid::Uuid::new_v4(),
                &note_id,
                &initial.detail_evidence.id,
                &initial.comments_evidence.id,
                &initial.replies_evidence.id,
            ],
        )
        .await
        .expect_err("context set must reject a different workspace");
    assert_protected_history_rejection(capture_set_workspace_violation);

    for statement in [
        "UPDATE context_work_record_v0 SET note_id = 'tampered'",
        "DELETE FROM context_discussion_record_v0",
        "UPDATE source_context_capture_set_v0 SET note_id = 'tampered'",
    ] {
        let error = inspector
            .execute(statement, &[])
            .await
            .expect_err("context history must be append-only");
        assert_protected_history_rejection(error);
    }
}

fn fixture_packages() -> (CapturePackageV0, CapturePackageV0, CapturePackageV0) {
    let fixture: Value = serde_json::from_str(FIXTURE).expect("fixture must parse");
    let packages = fixture["packages"].as_object().expect("fixture packages");
    (
        serde_json::from_value(packages["detailPackage"].clone()).expect("detail package"),
        serde_json::from_value(packages["commentsPackage"].clone()).expect("comments package"),
        serde_json::from_value(packages["repliesPackage"].clone()).expect("replies package"),
    )
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

async fn all_counts(inspector: &Client) -> [i64; 9] {
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
    let mut counts = [0; 9];
    for (index, table) in tables.iter().enumerate() {
        let row = inspector
            .query_one(&format!("SELECT count(*) FROM {table}"), &[])
            .await
            .expect("count query");
        counts[index] = row.get(0);
    }
    counts
}

async fn assert_all_counts(inspector: &Client, expected: [i64; 9]) {
    assert_eq!(all_counts(inspector).await, expected);
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
