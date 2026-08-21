//! Negatives for the F01 record processing proofs.
//!
//! These bypass the Rust facade with direct SQL, or race two runs against each other, so they
//! prove the database itself refuses the forged state rather than proving the application merely
//! happens not to write it.

mod support;

use linggan_observation::{
    BusinessOutcome, ProcessingOptions, ProcessingOutcome, process_one_ready_record,
};
use support::*;

// ---------------------------------------------------------------------------
// Database-level negatives: these bypass the Rust facade entirely, so they prove the database
// itself refuses the forged state rather than proving the application happens not to write it.
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
async fn database_refuses_observation_recorded_without_its_supporting_facts() {
    let database = accepted_f01_database("f01_forged_outcome", F01_PACKAGE).await;

    // 1. No attempt, no observation, no current at all.
    let no_facts = raw(&database, "UPDATE record_processing_work SET business_outcome = 'observation_recorded' WHERE id = (SELECT min(id) FROM record_processing_work)").await;
    assert_refused(no_facts, "processing attempt");

    // 2. A finalized attempt exists, but nothing was observed.
    raw(&database, "INSERT INTO record_processing_attempt (processing_attempt_ref, processing_work_id, epoch, lease_expires_at, finalized_at) SELECT gen_random_uuid(), min(id), 1, scope_001_now() + interval '5 minutes', scope_001_now() FROM record_processing_work")
        .await
        .expect("an attempt row on its own is legal");
    let no_observation = raw(&database, "UPDATE record_processing_work SET business_outcome = 'observation_recorded' WHERE id = (SELECT min(id) FROM record_processing_work)").await;
    assert_refused(no_observation, "observation");

    // 3. An observation exists for that very record, but no current revision was published.
    raw(
        &database,
        "WITH identity AS ( \
             INSERT INTO source_identity (source_identity_ref, source_system, namespace, object_type, external_id) \
             VALUES (gen_random_uuid(), 'synthetic', 'scope-001', 'content', 'synthetic-note-a') RETURNING id \
         ), content AS ( \
             INSERT INTO source_content (content_ref, source_identity_id) \
             SELECT gen_random_uuid(), id FROM identity RETURNING id \
         ) \
         INSERT INTO content_observation \
             (observation_ref, source_content_id, capture_record_id, observed_at, observed_at_precision, \
              parser_version, title_observed, title_value, body_observed, body_value) \
         SELECT gen_random_uuid(), content.id, r.id, r.observed_at, r.observed_at_precision, \
                'content-detail-processor-v1', true, 'Synthetic title A', true, 'Synthetic body A' \
         FROM content, capture_record r \
         WHERE r.id = (SELECT capture_record_id FROM record_processing_work ORDER BY id LIMIT 1)",
    )
    .await
    .expect("an observation without a current revision is legal on its own");
    let no_current = raw(&database, "UPDATE record_processing_work SET business_outcome = 'observation_recorded' WHERE id = (SELECT min(id) FROM record_processing_work)").await;
    assert_refused(no_current, "current revision");

    assert_eq!(
        count(&database, "content_current_revision").await,
        0,
        "none of the forged updates may leave a current revision behind"
    );
}

#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
async fn database_refuses_rewriting_or_deleting_a_published_current_revision() {
    let database = accepted_f01_database("f01_immutable_current", F01_PACKAGE).await;
    processed(&database, 0).await;

    let rewritten = raw(
        &database,
        "UPDATE content_current_revision SET title_value = 'FORGED'",
    )
    .await;
    assert_refused(rewritten, "append-only");

    let deleted = raw(&database, "DELETE FROM content_current_revision").await;
    assert_refused(deleted, "append-only");

    let rewritten_observation = raw(
        &database,
        "UPDATE content_observation SET title_value = 'FORGED'",
    )
    .await;
    assert_refused(rewritten_observation, "append-only");

    assert_current_matches_its_record(&database, "synthetic-note-a", "A").await;
}

#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
async fn database_refuses_a_field_source_that_does_not_support_the_published_value() {
    let database = accepted_f01_database("f01_unsupported_source", F01_PACKAGE).await;
    processed(&database, 0).await;
    let content_id = single_content_id(&database).await;
    let observation_id = single_observation_id(&database).await;

    // The support points at the real observation, but the revision publishes a different value.
    let forged = insert_revision_with_sources(
        &database,
        content_id,
        &[
            ("title", "FORGED", "selected"),
            ("body", "Synthetic body A", "selected"),
        ],
        &[
            ("title", observation_id, "selected_support"),
            ("body", observation_id, "selected_support"),
        ],
    )
    .await;
    assert_refused(forged, "does not observe");
}

#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
async fn database_refuses_unresolved_fabricated_from_same_value_candidates() {
    let database = accepted_f01_database("f01_fake_unresolved", F01_PACKAGE).await;
    processed(&database, 0).await;
    let content_id = single_content_id(&database).await;
    let first = single_observation_id(&database).await;
    // A second observation at the same instant carrying the very same values.
    let second = seed_identity_content_and_observation_for(
        &database,
        content_id,
        3,
        "2026-08-20T08:00:00Z",
        "Synthetic title A",
        "Synthetic body A",
    )
    .await;

    let forged = insert_revision_with_sources(
        &database,
        content_id,
        &[("title", "", "unresolved"), ("body", "", "unresolved")],
        &[
            ("title", first, "conflicting_candidate"),
            ("title", second, "conflicting_candidate"),
            ("body", first, "conflicting_candidate"),
            ("body", second, "conflicting_candidate"),
        ],
    )
    .await;
    assert_refused(forged, "distinct values");
}

#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
async fn database_refuses_a_stale_observation_supporting_the_latest_current() {
    let database = accepted_f01_database("f01_stale_support", F01_PACKAGE).await;
    processed(&database, 0).await;
    let content_id = single_content_id(&database).await;
    let stale = single_observation_id(&database).await;
    // A strictly newer observation exists, so the older one can no longer support current.
    seed_identity_content_and_observation_for(
        &database,
        content_id,
        3,
        "2026-08-20T09:00:00Z",
        "Newer title A",
        "Newer body A",
    )
    .await;

    let forged = insert_revision_with_sources(
        &database,
        content_id,
        &[
            ("title", "Synthetic title A", "selected"),
            ("body", "Synthetic body A", "selected"),
        ],
        &[
            ("title", stale, "selected_support"),
            ("body", stale, "selected_support"),
        ],
    )
    .await;
    assert_refused(forged, "latest");
}

// ---------------------------------------------------------------------------
// Concurrency
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
async fn concurrent_records_for_one_identity_converge_and_keep_the_newest_observation() {
    let database = accepted_f01_database("f01_concurrent_identity", F01_PACKAGE).await;
    // A third record naming the same source object, observed strictly later than record A.
    insert_extra_capture_record_and_work(
        &database,
        3,
        "synthetic-note-a",
        Some("synthetic-note-a"),
        "2026-08-20T09:00:00Z",
        "Newer title A",
        "Newer body A",
    )
    .await;
    // All three ready records run at once. Two of them resolve the same identity, so one must
    // lose that race and still read the winner rather than failing or returning no row.
    let options = [
        ProcessingOptions::default(),
        ProcessingOptions::default(),
        ProcessingOptions::default(),
    ];
    let (first, second, third) = tokio::join!(
        process_one_ready_record(&database, &options[0]),
        process_one_ready_record(&database, &options[1]),
        process_one_ready_record(&database, &options[2]),
    );
    for outcome in [
        first.expect("a concurrent claim must not fail"),
        second.expect("a concurrent claim must not fail"),
        third.expect("a concurrent claim must not fail"),
    ] {
        match outcome {
            ProcessingOutcome::Processed(record) => assert_eq!(
                record.business_outcome,
                BusinessOutcome::ObservationRecorded,
                "a losing race must still read the winning identity and continue"
            ),
            ProcessingOutcome::NothingReady => panic!("all three records were ready"),
        }
    }

    assert_eq!(
        count(&database, "source_identity").await,
        2,
        "one per distinct source object"
    );
    assert_eq!(count(&database, "source_content").await, 2);
    assert_eq!(count(&database, "content_observation").await, 3);
    assert_eq!(
        published_title(&database, "synthetic-note-a").await,
        "Newer title A",
        "the newest observation must reach current no matter which record committed last"
    );
    assert_eq!(
        count(&database, "record_processing_attempt").await,
        3,
        "a rolled-back run must not leave an unfinalized attempt behind"
    );
    assert_eq!(unfinalized_attempts(&database).await, 0);
}

// ---------------------------------------------------------------------------
// Identity ruling, end to end
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
async fn explicit_null_source_external_id_conflicts_instead_of_falling_back() {
    let database = accepted_f01_database("f01_null_no_fallback", F01_SOURCE_EXTERNAL_ID_NULL).await;

    let ruled = processed(&database, 0).await;
    assert_eq!(
        ruled.business_outcome,
        BusinessOutcome::SourceIdentityConflict,
        "target=A, source=null, payload=A must conflict; neither statement may fill in for source"
    );
    assert_eq!(count(&database, "source_identity").await, 0);
    assert_eq!(count(&database, "source_content").await, 0);
    assert_eq!(count(&database, "content_observation").await, 0);
    assert_eq!(count(&database, "content_current_revision").await, 0);
    assert_eq!(
        count(&database, "content_current_revision_field_source").await,
        0
    );
}

// ---------------------------------------------------------------------------
// Frozen proof references
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
#[should_panic(expected = "frozen proof reference sequence")]
async fn exhausted_frozen_refs_fail_closed_instead_of_minting_a_random_uuid() {
    let database = accepted_f01_database("f01_refs_exhausted", F01_PACKAGE).await;
    let mut options = ProcessingOptions::default();
    // One short of what a full observation_recorded run consumes.
    options.use_fixed_refs(vec![
        manifest_ref("processingAttemptA"),
        manifest_ref("sourceIdentityA"),
        manifest_ref("contentA"),
        manifest_ref("observationA"),
        manifest_ref("currentRevisionA"),
        manifest_ref("fieldSourceTitleA"),
    ]);
    let _ = process_one_ready_record(&database, &options).await;
}

#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
#[should_panic(expected = "unconsumed")]
async fn unconsumed_frozen_refs_fail_closed_at_the_end_of_a_run() {
    let database = accepted_f01_database("f01_refs_unconsumed", F01_PACKAGE).await;
    let mut options = ProcessingOptions::default();
    // One more than a full run consumes: a leftover reference must not pass silently.
    options.use_fixed_refs(vec![
        manifest_ref("processingAttemptA"),
        manifest_ref("sourceIdentityA"),
        manifest_ref("contentA"),
        manifest_ref("observationA"),
        manifest_ref("currentRevisionA"),
        manifest_ref("fieldSourceTitleA"),
        manifest_ref("fieldSourceBodyA"),
        manifest_ref("processingAttemptB"),
    ]);
    process_one_ready_record(&database, &options)
        .await
        .expect("the run itself succeeds");
    options.assert_frozen_refs_fully_consumed();
}
