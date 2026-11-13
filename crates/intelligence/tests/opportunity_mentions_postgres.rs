//! T1.1 proof against a disposable PostgreSQL schema and synthetic source memberships.
#[path = "../../../apps/api/src/local_web/full_schema_fixture.rs"]
mod full_schema_fixture;
#[path = "support/opportunity_dataset.rs"]
mod opportunity_dataset;

use linggan_intelligence::opportunity::{
    CommenterDigest, MentionSyncError, check_invariants, sync_comment_mentions,
};
use linggan_storage_postgres::{Database, testing::isolated_proof_schema};
use sqlx::Row;

async fn database(schema: &str) -> Database {
    let url = std::env::var("OPPORTUNITY_PROOF_DATABASE_URL").expect("disposable proof URL");
    isolated_proof_schema(&url, schema, full_schema_fixture::FULL_MIGRATIONS)
        .await
        .expect("isolated schema")
}

async fn seed_memberships(db: &Database) {
    opportunity_dataset::build(db).await.unwrap();
    sqlx::raw_sql(
        r#"
ALTER TABLE linggan_model_entry DISABLE TRIGGER ALL;
INSERT INTO linggan_model_entry(model_ref, connection_version_ref, model_id, origin)
VALUES ('00000000-0000-4000-8000-000000004001',
        '00000000-0000-4000-8000-000000004002', 'synthetic-model', 'manual');
ALTER TABLE linggan_model_entry ENABLE TRIGGER ALL;

ALTER TABLE linggan_model_invocation DISABLE TRIGGER ALL;
INSERT INTO linggan_model_invocation(
 invocation_ref, connection_version_ref, model_ref, operation, request_hash, state,
 reserved_tokens, charged_tokens, task_key, task_config_ref, budget_day, usage_state,
 dispatch_state, finished_at
)
SELECT '00000000-0000-4000-8000-000000004003',
       '00000000-0000-4000-8000-000000004002',
       '00000000-0000-4000-8000-000000004001', 'analyze', repeat('a',64), 'succeeded',
       1, 1, 'comment_problem_pair_review', current_config_ref, '2026-11-01',
       'unknown_reserved', 'started', '2026-11-04T00:00:00Z'
FROM linggan_ai_task_setting WHERE task_key = 'comment_problem_pair_review';
ALTER TABLE linggan_model_invocation ENABLE TRIGGER ALL;

ALTER TABLE linggan_comment_research_request DISABLE TRIGGER ALL;
INSERT INTO linggan_comment_research_request(
 request_ref, command_ref, domain_ref, work_ref, source_ref, observation_role,
 knowledge_cutoff, input_hash, input_fingerprint, task_config_ref, batch_ref, method_version,
 created_at
)
SELECT md5('request-' || n)::uuid, md5('command-' || n)::uuid,
       '00000000-0000-4000-8000-000000000001',
       '00000000-0000-4000-8000-000000000100',
       md5('opportunity-comment:00000000-0000-4000-8000-000000000100:' || n)::uuid,
       'primary', '2026-11-05T00:00:00Z', repeat('b',64), repeat('c',64),
       setting.current_config_ref, md5('batch-' || n)::uuid, 'comment-research-semantic.v1',
       '2026-11-05T00:00:00Z'
FROM (VALUES (1), (2), (3), (1000)) seed(n)
CROSS JOIN linggan_ai_task_setting setting WHERE setting.task_key = 'comment_study';
ALTER TABLE linggan_comment_research_request ENABLE TRIGGER ALL;

ALTER TABLE linggan_comment_problem_definition DISABLE TRIGGER ALL;
INSERT INTO linggan_comment_problem_definition(
 definition_ref, problem_ref, domain_ref, identity_hash, label, actor, goal, barrier,
 source_request_ref, source_signal_ordinal, review_invocation_ref, created_at
)
VALUES ('00000000-0000-4000-8000-000000002001',
        '00000000-0000-4000-8000-000000001001',
        '00000000-0000-4000-8000-000000000001', repeat('d',64), 'synthetic problem',
        'actor', 'goal', 'barrier', md5('request-1')::uuid, 0,
        '00000000-0000-4000-8000-000000004003', '2026-11-05T00:00:00Z');
ALTER TABLE linggan_comment_problem_definition ENABLE TRIGGER ALL;

INSERT INTO linggan_comment_problem_membership(request_ref, signal_ordinal, definition_ref,
 review_invocation_ref, created_at)
SELECT md5('request-' || n)::uuid, 0, '00000000-0000-4000-8000-000000002001',
       '00000000-0000-4000-8000-000000004003', '2026-11-05T00:00:00Z'
FROM (VALUES (1), (2), (1000)) seed(n);
INSERT INTO linggan_comment_problem_membership(request_ref, signal_ordinal, definition_ref,
 review_invocation_ref, created_at)
VALUES (md5('request-1')::uuid, 1, '00000000-0000-4000-8000-000000002001',
        '00000000-0000-4000-8000-000000004003', '2026-11-05T00:00:00Z');
"#,
    )
    .execute(db.pool())
    .await
    .unwrap();
}

#[tokio::test]
#[ignore = "requires the disposable opportunity PostgreSQL proof"]
async fn existing_and_incremental_memberships_are_idempotent_and_pseudonymous() {
    let db = database("opportunity_t11_mentions").await;
    seed_memberships(&db).await;
    let digest = CommenterDigest::from_key(vec![42; 32]).unwrap();
    let first = sync_comment_mentions(&db, &digest).await.unwrap();
    assert_eq!(first.source_memberships, 4);
    assert_eq!(first.unique_comment_problem_pairs, 3);
    assert_eq!(first.inserted, 3);
    assert_eq!(first.unknown_commenters, 1);
    assert_eq!(first.reconciliations[0].source_mentioners, 2);
    assert_eq!(first.reconciliations[0].projected_mentioners, 2);
    assert!(first.reconciliations[0].reasons.is_empty());
    assert_eq!(first.reconciliations[0].eligible_mentioners, 2);
    assert!(!first.reconciliations[0].currently_published);
    let unknown = first.reconciliations[0]
        .eligibility
        .iter()
        .find(|bucket| bucket.reason == "commenter_unknown")
        .expect("unknown identity remains explicit");
    assert_eq!(unknown.comments, 1);
    assert_eq!(unknown.mentioners, 0);

    let again = sync_comment_mentions(&db, &digest).await.unwrap();
    assert_eq!(again.inserted, 0);
    assert_eq!(again.updated, 0);
    assert_eq!(again.already_present, 3);

    let wrong_key = CommenterDigest::from_key(vec![43; 32]).unwrap();
    let error = sync_comment_mentions(&db, &wrong_key)
        .await
        .expect_err("a different key under the same digest version must not mix hashes");
    assert!(matches!(
        error,
        MentionSyncError::DigestKeyOrSourceMismatch { .. }
    ));

    sqlx::query("INSERT INTO linggan_comment_problem_membership(request_ref, signal_ordinal, definition_ref, review_invocation_ref) VALUES (md5('request-3')::uuid,0,'00000000-0000-4000-8000-000000002001','00000000-0000-4000-8000-000000004003')")
        .execute(db.pool()).await.unwrap();
    let incremental = sync_comment_mentions(&db, &digest).await.unwrap();
    assert_eq!(incremental.inserted, 1);
    assert_eq!(incremental.reconciliations[0].source_mentioners, 3);
    assert_eq!(incremental.reconciliations[0].projected_mentioners, 3);
    let states = sqlx::query(
        "SELECT assigned_by, count(*) AS n FROM linggan_opportunity_comment_mention GROUP BY assigned_by ORDER BY assigned_by",
    ).fetch_all(db.pool()).await.unwrap();
    assert_eq!(states[0].get::<String, _>("assigned_by"), "ai");
    assert_eq!(states[0].get::<i64, _>("n"), 1);
    assert_eq!(states[1].get::<String, _>("assigned_by"), "migrated");
    assert_eq!(states[1].get::<i64, _>("n"), 3);
    let report = serde_json::to_string(&incremental).unwrap();
    assert!(!report.contains("commenter-work-0"));
    let invariants = check_invariants(&db).await.unwrap();
    for id in ["I6", "I6b", "I8", "I11"] {
        let check = invariants
            .checks
            .iter()
            .find(|check| check.id == id)
            .unwrap();
        assert!(check.available, "{id}");
        assert_eq!(check.violations, 0, "{id}");
    }
}

#[tokio::test]
#[ignore = "requires the disposable opportunity PostgreSQL proof"]
async fn a_new_canonical_observation_updates_the_existing_derived_hash() {
    let db = database("opportunity_t11_canonical_change").await;
    seed_memberships(&db).await;
    let digest = CommenterDigest::from_key(vec![42; 32]).unwrap();
    sync_comment_mentions(&db, &digest).await.unwrap();
    sqlx::raw_sql(
        r#"
INSERT INTO linggan_runtime_record_disposition(package_ref, record_ordinal, disposition, reason)
SELECT package_ref, 4, 'accepted_for_library_content', 'synthetic canonical change'
FROM opportunity_seed_package WHERE work_key='work-0' AND kind='late';
INSERT INTO linggan_material_comment(
 material_ref, content_public_ref, package_ref, record_ordinal, comment_external_id,
 root_comment_external_id, is_reply, body_text, body_state, author_external_id, observed_at
)
SELECT '00000000-0000-4000-8000-000000008001',
       '00000000-0000-4000-8000-000000000100', package_ref, 4, 'c-work-0-1',
       'c-work-0-1', false, NULL, 'UNKNOWN', 'synthetic-new-author', '2026-11-10T01:00:00Z'
FROM opportunity_seed_package WHERE work_key='work-0' AND kind='late';
"#,
    )
    .execute(db.pool())
    .await
    .unwrap();
    let unchanged = sync_comment_mentions(&db, &digest).await.unwrap();
    assert_eq!(unchanged.updated, 0);
    assert_eq!(unchanged.identity_conflicts(), 1);
    assert!(unchanged.reconciliations[0].reasons.is_empty());

    sqlx::raw_sql(
        r#"
ALTER TABLE linggan_material_comment_first_admission DISABLE TRIGGER USER;
UPDATE linggan_material_comment_first_admission
SET first_package_ref = (SELECT package_ref FROM opportunity_seed_package WHERE work_key='work-0' AND kind='late')
WHERE content_public_ref='00000000-0000-4000-8000-000000000100' AND comment_external_id='c-work-0-1';
ALTER TABLE linggan_material_comment_first_admission ENABLE TRIGGER USER;
"#,
    )
    .execute(db.pool())
    .await
    .unwrap();
    let report = sync_comment_mentions(&db, &digest).await.unwrap();
    assert_eq!(report.updated, 1);
    assert_eq!(report.reconciliations[0].identity_conflicts, 1);
    assert_eq!(
        report.reconciliations[0].reasons,
        vec!["commenter_identity_conflict"]
    );
    let row = sqlx::query("SELECT commenter_hash, hash_changed_at IS NOT NULL AS changed FROM linggan_opportunity_comment_mention WHERE comment_external_id='c-work-0-1'")
        .fetch_one(db.pool()).await.unwrap();
    assert_eq!(
        row.get::<String, _>("commenter_hash"),
        digest.hash(Some("synthetic-new-author")).unwrap()
    );
    assert!(row.get::<bool, _>("changed"));
}

#[tokio::test]
#[ignore = "requires the disposable opportunity PostgreSQL proof"]
async fn missing_source_provenance_rejects_the_whole_projection() {
    let db = database("opportunity_t11_missing_provenance").await;
    seed_memberships(&db).await;
    sqlx::raw_sql(
        "ALTER TABLE linggan_model_invocation DISABLE TRIGGER USER; \
         UPDATE linggan_model_invocation SET model_ref = NULL \
         WHERE invocation_ref = '00000000-0000-4000-8000-000000004003'; \
         ALTER TABLE linggan_model_invocation ENABLE TRIGGER USER;",
    )
    .execute(db.pool())
    .await
    .unwrap();
    let digest = CommenterDigest::from_key(vec![42; 32]).unwrap();
    let error = sync_comment_mentions(&db, &digest)
        .await
        .expect_err("missing model provenance must reject the projection");
    assert!(matches!(
        error,
        MentionSyncError::SourceProvenanceMissing { rows: 4 }
    ));
    let stored: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_opportunity_comment_mention")
            .fetch_one(db.pool())
            .await
            .unwrap();
    let registered: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_opportunity_problem")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!((stored, registered), (0, 0));
}

#[tokio::test]
#[ignore = "requires the disposable opportunity PostgreSQL proof"]
async fn taxonomy_history_preserves_the_version_at_each_known_time() {
    let db = database("opportunity_t11_taxonomy").await;
    seed_memberships(&db).await;
    let digest = CommenterDigest::from_key(vec![42; 32]).unwrap();
    sync_comment_mentions(&db, &digest).await.unwrap();
    sqlx::query("UPDATE linggan_opportunity_taxonomy_version SET version=version+1, updated_at=updated_at+interval '1 second' WHERE domain_ref=$1::uuid")
        .bind(opportunity_dataset::DOMAIN).execute(db.pool()).await.unwrap();
    let versions: Vec<i64> = sqlx::query_scalar(
        "SELECT linggan_opportunity_taxonomy_version_at(domain_ref, created_at)
         FROM linggan_opportunity_taxonomy_version_history ORDER BY version",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(versions, vec![1, 2]);
    let missing = sqlx::query_scalar::<_, i64>(
        "SELECT linggan_opportunity_taxonomy_version_at($1::uuid, '2000-01-01T00:00:00Z')",
    )
    .bind(opportunity_dataset::DOMAIN)
    .fetch_one(db.pool())
    .await
    .expect_err("unknown history must not be invented");
    assert!(
        missing
            .to_string()
            .contains("opportunity_taxonomy_version_unavailable_as_of")
    );

    let mut earlier = db.pool().begin().await.unwrap();
    sqlx::query("SELECT now()")
        .execute(&mut *earlier)
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(2)).await;
    let mut later = db.pool().begin().await.unwrap();
    sqlx::query("UPDATE linggan_opportunity_taxonomy_version SET version=version+1, updated_at=now() WHERE domain_ref=$1::uuid")
        .bind(opportunity_dataset::DOMAIN).execute(&mut *later).await.unwrap();
    later.commit().await.unwrap();
    sqlx::query("UPDATE linggan_opportunity_taxonomy_version SET version=version+1, updated_at=now() WHERE domain_ref=$1::uuid")
        .bind(opportunity_dataset::DOMAIN).execute(&mut *earlier).await.unwrap();
    earlier.commit().await.unwrap();
    let backwards: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM (SELECT created_at, lag(created_at) OVER (ORDER BY version) previous
         FROM linggan_opportunity_taxonomy_version_history) versions WHERE created_at <= previous",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(backwards, 0);
}
