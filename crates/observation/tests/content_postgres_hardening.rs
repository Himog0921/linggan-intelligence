//! Round-4 F01 database hardening proofs.
//!
//! These direct PostgreSQL attacks cover the repair guards that must hold even
//! when a caller bypasses the Rust processing facade.

mod support;

use linggan_observation::{
    BusinessOutcome, ProcessingFault, ProcessingOptions, ProcessingOutcome,
    process_one_ready_record,
};
use support::*;

#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
async fn database_refuses_clearing_a_finished_outcome_before_deleting_its_attempt() {
    let database = accepted_f01_database("f01_outcome_is_fixed", F01_PACKAGE).await;
    processed(&database, 0).await;

    let cleared = raw(
        &database,
        "UPDATE record_processing_work SET business_outcome = NULL WHERE business_outcome IS NOT NULL",
    )
    .await;
    assert_refused(cleared, "business outcome");

    let deleted = raw(&database, "DELETE FROM record_processing_attempt").await;
    assert_refused(deleted, "cannot be removed or reopened");
    assert_eq!(count(&database, "record_processing_attempt").await, 1);
}

#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
async fn database_refuses_rolling_current_back_to_an_older_revision() {
    let database = accepted_f01_database("f01_current_never_rolls_back", F01_PACKAGE).await;
    processed(&database, 0).await;
    let content_id = single_content_id(&database).await;
    let observation_id = single_observation_id(&database).await;
    let old_revision: i64 =
        sqlx::query_scalar("SELECT current_revision_id FROM source_content WHERE id = $1")
            .bind(content_id)
            .fetch_one(database.pool())
            .await
            .expect("the first current revision must exist");

    insert_revision_with_sources(
        &database,
        content_id,
        &[
            ("title", "Synthetic title A", "selected"),
            ("body", "Synthetic body A", "selected"),
        ],
        &[
            ("title", observation_id, "selected_support"),
            ("body", observation_id, "selected_support"),
        ],
    )
    .await
    .expect("a complete newer revision may be prepared");
    let newer_revision: i64 = sqlx::query_scalar(
        "SELECT max(id) FROM content_current_revision WHERE source_content_id = $1",
    )
    .bind(content_id)
    .fetch_one(database.pool())
    .await
    .expect("the prepared revision must exist");
    raw(
        &database,
        &format!(
            "UPDATE content_current_revision SET published_at = scope_001_now() WHERE id = {newer_revision}"
        ),
    )
    .await
    .expect("the prepared revision may be marked for publication");
    raw(
        &database,
        &format!("UPDATE source_content SET current_revision_id = {newer_revision} WHERE id = {content_id}"),
    )
    .await
    .expect("a complete newer revision may publish");

    let rollback = raw(
        &database,
        &format!(
            "UPDATE source_content SET current_revision_id = {old_revision} WHERE id = {content_id}"
        ),
    )
    .await;
    assert_refused(rollback, "cannot roll back");
}

#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
async fn publication_revalidates_a_revision_contaminated_after_its_initial_insert() {
    let database = accepted_f01_database("f01_publish_revalidates", F01_PACKAGE).await;
    processed(&database, 0).await;
    let content_id = single_content_id(&database).await;
    let first = single_observation_id(&database).await;

    insert_revision_with_sources(
        &database,
        content_id,
        &[
            ("title", "Synthetic title A", "selected"),
            ("body", "Synthetic body A", "selected"),
        ],
        &[
            ("title", first, "selected_support"),
            ("body", first, "selected_support"),
        ],
    )
    .await
    .expect("a complete revision may be staged before publication");
    let staged: i64 = sqlx::query_scalar(
        "SELECT max(id) FROM content_current_revision WHERE source_content_id = $1",
    )
    .bind(content_id)
    .fetch_one(database.pool())
    .await
    .expect("the staged revision must exist");
    let later = seed_identity_content_and_observation_for(
        &database,
        content_id,
        3,
        "2026-08-20T09:00:00Z",
        "Later title A",
        "Later body A",
    )
    .await;
    raw(
        &database,
        &format!(
            "INSERT INTO content_current_revision_field_source \
             (field_source_ref, revision_id, source_content_id, field_kind, observation_id, role) \
             VALUES (gen_random_uuid(), {staged}, {content_id}, 'title', {later}, 'conflicting_candidate')"
        ),
    )
    .await
    .expect("the pre-publication attack must be observable before the repair");
    raw(
        &database,
        &format!(
            "UPDATE content_current_revision SET published_at = scope_001_now() WHERE id = {staged}"
        ),
    )
    .await
    .expect("the staged revision may still be marked before its publication guard runs");

    let publish = raw(
        &database,
        &format!(
            "UPDATE source_content SET current_revision_id = {staged} WHERE id = {content_id}"
        ),
    )
    .await;
    assert_refused(publish, "latest");
}

#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
async fn database_refuses_rebinding_identity_or_content_anchor_after_observation_use() {
    let database = accepted_f01_database("f01_immutable_anchors", F01_PACKAGE).await;
    processed(&database, 0).await;

    let identity = raw(
        &database,
        "UPDATE source_identity SET external_id = 'forged-external-id'",
    )
    .await;
    assert_refused(identity, "cannot be rebound");

    let content = raw(
        &database,
        "UPDATE source_content SET content_ref = gen_random_uuid()",
    )
    .await;
    assert_refused(content, "cannot be rebound");
}

#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
async fn a_multi_record_current_watermark_has_lexically_sorted_distinct_refs() {
    let database = accepted_f01_database("f01_watermark_order", F01_PACKAGE).await;
    processed(&database, 0).await;
    processed(&database, 1).await;
    insert_extra_capture_record_and_work(
        &database,
        3,
        "synthetic-note-a",
        Some("synthetic-note-a"),
        "2026-08-20T09:00:00Z",
        "Later title A",
        "Later body A",
    )
    .await;
    match process_one_ready_record(&database, &ProcessingOptions::default())
        .await
        .expect("the third record must process")
    {
        ProcessingOutcome::Processed(record) => assert_eq!(
            record.business_outcome,
            BusinessOutcome::ObservationRecorded
        ),
        ProcessingOutcome::NothingReady => panic!("the third record must remain ready"),
    }

    let watermark: serde_json::Value = sqlx::query_scalar(
        "SELECT rev.watermark FROM content_current_revision rev \
         JOIN source_content c ON c.current_revision_id = rev.id \
         JOIN source_identity i ON i.id = c.source_identity_id \
         WHERE i.external_id = 'synthetic-note-a'",
    )
    .fetch_one(database.pool())
    .await
    .expect("the current watermark must be readable");
    let entry = watermark
        .as_array()
        .and_then(|entries| entries.first())
        .expect("one package must be represented");
    for key in ["recordRefs", "observationRefs"] {
        let refs = entry[key]
            .as_array()
            .expect("watermark arrays must be explicit arrays")
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .expect("a watermark ref must be a string")
                    .to_owned()
            })
            .collect::<Vec<_>>();
        let mut expected = refs.clone();
        expected.sort();
        expected.dedup();
        assert_eq!(refs, expected, "{key} must be distinct and lexical");
        assert_eq!(refs.len(), 2, "both records must remain in the watermark");
    }
}

#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
async fn a_run_error_that_cannot_be_persisted_is_returned_as_an_explicit_failure() {
    let schema = "f01_run_error_observable";
    let admin = accepted_f01_database(schema, F01_PACKAGE).await;
    let runtime = runtime_role_database(schema).await;
    let role = format!("scope_001_runtime_{schema}");
    raw(
        &admin,
        &format!("REVOKE UPDATE (run_error) ON record_processing_attempt FROM {role}"),
    )
    .await
    .expect("the proof may remove only run-error persistence from the runtime role");

    let mut faulty = f01_processing_options(0);
    faulty.inject_fault(ProcessingFault::AfterObservation);
    let failed = process_one_ready_record(&runtime, &faulty)
        .await
        .expect_err("a run with no durable error receipt must not look like an ordinary failure");
    assert!(
        failed.to_string().contains("run-error persistence"),
        "the caller must see that the failure audit itself was not recorded: {failed}"
    );
    assert_eq!(count(&admin, "record_processing_attempt").await, 1);
    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*) FROM record_processing_work WHERE business_outcome IS NOT NULL"
        )
        .await,
        0
    );
}

#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
async fn runtime_role_cannot_direct_insert_a_fabricated_fact_chain() {
    let schema = "f01_runtime_fake_chain";
    let admin = accepted_f01_database(schema, F01_PACKAGE).await;
    let runtime = runtime_role_database(schema).await;

    let fabricated_identity = raw(
        &runtime,
        "INSERT INTO source_identity \
         (source_identity_ref, source_system, namespace, object_type, external_id) \
         VALUES (gen_random_uuid(), 'synthetic', 'scope-001', 'content', 'forged-runtime-object')",
    )
    .await;
    assert_refused(fabricated_identity, "accepted record envelope");

    let fabricated_work = raw(
        &runtime,
        "INSERT INTO record_processing_work \
         (processing_work_ref, capture_record_id, processor_version) \
         SELECT gen_random_uuid(), id, 'content-detail-processor-v1' \
         FROM capture_record WHERE ordinal = 2",
    )
    .await
    .expect_err("the runtime role must not create a processing work outside ingress");
    assert!(
        fabricated_work.to_string().contains("permission denied"),
        "only ingress may create processing work: {fabricated_work}"
    );
    assert_eq!(count(&admin, "source_identity").await, 0);
    assert_eq!(count(&admin, "record_processing_work").await, 2);
}
