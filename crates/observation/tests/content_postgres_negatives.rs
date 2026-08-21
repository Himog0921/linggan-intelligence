//! Negatives for the F01 record processing proofs.
//!
//! These bypass the Rust facade with direct SQL, or race two runs against each other, so they
//! prove the database itself refuses the forged state rather than proving the application merely
//! happens not to write it.

mod support;

use linggan_observation::{
    BusinessOutcome, ProcessingError, ProcessingFault, ProcessingOptions, ProcessingOutcome,
    claim_one_ready_record, process_one_ready_record, run_claimed_record,
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
    // All three ready records run at once, each on its own task, held at a rendezvous until every
    // run has opened its transaction. Without the rendezvous this proof is probabilistic: it
    // passes whether or not identity resolution is actually serialized. The runs must be separate
    // tasks so they can actually overlap; the barrier is async so it never blocks a runtime
    // thread.
    let rendezvous = std::sync::Arc::new(tokio::sync::Barrier::new(3));
    let mut runs = Vec::new();
    for _ in 0..3 {
        let database = database.clone();
        let rendezvous = rendezvous.clone();
        runs.push(tokio::spawn(async move {
            let mut options = ProcessingOptions::default();
            options.synchronize_before_identity(rendezvous);
            process_one_ready_record(&database, &options).await
        }));
    }
    for run in runs {
        match run
            .await
            .expect("a concurrent run must not panic")
            .expect("a concurrent claim must not fail")
        {
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

    let ruled = processed_with(&database, &attempt_ref_only()).await;
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

// ---------------------------------------------------------------------------
// Second review round: a published fact must stay closed, and an outcome must keep the facts it
// depends on. Every case below is written with direct SQL, outside the Rust facade.
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
async fn database_refuses_appending_a_source_to_a_published_revision() {
    let database = accepted_f01_database("f01_append_source", F01_PACKAGE).await;
    processed(&database, 0).await;
    let observation_id = single_observation_id(&database).await;

    // A published revision already says title is `selected`. Adding a conflicting candidate after
    // the fact would rewrite what that revision means without creating a new revision.
    let appended = raw(
        &database,
        &format!(
            "INSERT INTO content_current_revision_field_source \
                 (field_source_ref, revision_id, source_content_id, field_kind, observation_id, role) \
             SELECT gen_random_uuid(), c.current_revision_id, c.id, 'title', {observation_id}, 'conflicting_candidate' \
             FROM source_content c WHERE c.current_revision_id IS NOT NULL"
        ),
    )
    .await;
    assert_refused(appended, "closed");

    assert_eq!(
        count(&database, "content_current_revision_field_source").await,
        2,
        "the published revision must still fix exactly its original two sources"
    );
}

#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
async fn database_refuses_observation_recorded_that_borrows_an_older_current() {
    let database = accepted_f01_database("f01_borrowed_current", F01_PACKAGE).await;
    processed(&database, 0).await;
    let content_id = single_content_id(&database).await;

    // A later record for the same content forms its own observation, but current is never
    // recomputed, so the published revision cannot possibly include it.
    let record_id = insert_extra_capture_record_and_work(
        &database,
        3,
        "synthetic-note-a",
        Some("synthetic-note-a"),
        "2026-08-20T10:00:00Z",
        "Newer title A",
        "Newer body A",
    )
    .await;
    raw(
        &database,
        &format!(
            "INSERT INTO content_observation \
                 (observation_ref, source_content_id, capture_record_id, observed_at, observed_at_precision, \
                  parser_version, title_observed, title_value, body_observed, body_value) \
             SELECT gen_random_uuid(), {content_id}, r.id, r.observed_at, r.observed_at_precision, \
                    'content-detail-processor-v1', true, 'Newer title A', true, 'Newer body A' \
             FROM capture_record r WHERE r.id = {record_id}"
        ),
    )
    .await
    .expect("an observation on its own is legal");
    raw(
        &database,
        &format!(
            "INSERT INTO record_processing_attempt \
                 (processing_attempt_ref, processing_work_id, epoch, lease_expires_at, finalized_at) \
             SELECT gen_random_uuid(), w.id, 1, scope_001_now() + interval '5 minutes', scope_001_now() \
             FROM record_processing_work w WHERE w.capture_record_id = {record_id}"
        ),
    )
    .await
    .expect("an attempt row on its own is legal");

    let borrowed = raw(
        &database,
        &format!(
            "UPDATE record_processing_work SET business_outcome = 'observation_recorded' \
             WHERE capture_record_id = {record_id}"
        ),
    )
    .await;
    assert_refused(borrowed, "current revision to include");
}

#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
async fn database_refuses_removing_the_facts_a_finished_outcome_depends_on() {
    let database = accepted_f01_database("f01_remove_support", F01_PACKAGE).await;
    processed(&database, 0).await;

    let deleted_attempt = raw(&database, "DELETE FROM record_processing_attempt").await;
    assert_refused(deleted_attempt, "cannot be removed or reopened");

    let reopened_attempt = raw(
        &database,
        "UPDATE record_processing_attempt SET finalized_at = NULL",
    )
    .await;
    assert_refused(reopened_attempt, "cannot be removed or reopened");

    let cleared_pointer = raw(
        &database,
        "UPDATE source_content SET current_revision_id = NULL WHERE current_revision_id IS NOT NULL",
    )
    .await;
    assert_refused(cleared_pointer, "cannot be cleared");

    assert_eq!(count(&database, "record_processing_attempt").await, 1);
    assert_eq!(
        scalar(
            &database,
            "SELECT count(*) FROM source_content WHERE current_revision_id IS NOT NULL"
        )
        .await,
        1
    );
}

// ---------------------------------------------------------------------------
// Durable lease and epoch. A claim is persisted before the work runs, so other readers can see
// the work is leased, a crash leaves a takeable state, and a late epoch cannot write back.
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
async fn an_expired_lease_is_taken_over_by_a_higher_epoch_and_the_late_one_is_refused() {
    let database = accepted_f01_database("f01_lease_takeover", F01_PACKAGE).await;
    processed(&database, 0).await; // finish record A so only record B is claimable

    let first = claim_one_ready_record(&database, &attempt_ref_only())
        .await
        .expect("claiming must succeed")
        .expect("record B is ready");
    assert_eq!(first.epoch(), 1);
    assert_eq!(first.record_ref(), manifest_ref("recordB"));

    // The claim is durable: another worker sees the lease and finds nothing else to take.
    assert!(
        claim_one_ready_record(&database, &attempt_ref_only())
            .await
            .expect("a second claim attempt must not error")
            .is_none(),
        "a live lease must be visible to other readers, not hidden inside a transaction"
    );

    raw(
        &database,
        "UPDATE record_processing_attempt SET lease_expires_at = scope_001_now() - interval '1 minute' WHERE finalized_at IS NULL",
    )
    .await
    .expect("expiring a lease is legal");

    let second = claim_one_ready_record(&database, &attempt_ref_only())
        .await
        .expect("claiming must succeed")
        .expect("an expired lease must be takeable");
    assert_eq!(second.epoch(), 2, "a takeover increments the epoch");
    assert_eq!(second.record_ref(), first.record_ref());

    // The superseded epoch may not write its result back.
    let late = run_claimed_record(&database, first, &f01_run_refs("B")).await;
    assert!(
        matches!(late, Err(ProcessingError::LeaseLost)),
        "a late epoch must be refused, got {late:?}"
    );
    assert_eq!(
        scalar(
            &database,
            "SELECT count(*) FROM record_processing_work WHERE business_outcome IS NOT NULL"
        )
        .await,
        1,
        "only record A is finished; the refused epoch must not have finalized record B"
    );

    let taken_over = run_claimed_record(&database, second, &f01_run_refs("B"))
        .await
        .expect("the current epoch must be able to finish");
    assert_eq!(
        taken_over.business_outcome,
        BusinessOutcome::ObservationRecorded
    );

    // Both attempts survive as history; only the winning epoch is finalized.
    assert_eq!(count(&database, "record_processing_attempt").await, 3);
    assert_eq!(
        scalar(
            &database,
            "SELECT count(*) FROM record_processing_attempt WHERE epoch = 1 AND finalized_at IS NULL AND run_error IS NOT NULL"
        )
        .await,
        1,
        "the superseded attempt keeps an honest, unfinalized run record"
    );
}

#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
async fn a_failed_run_leaves_a_traceable_attempt_instead_of_vanishing() {
    let database = accepted_f01_database("f01_failed_run", F01_PACKAGE).await;

    let claim = claim_one_ready_record(&database, &attempt_ref_only())
        .await
        .expect("claiming must succeed")
        .expect("record A is ready");
    let mut faulty = f01_run_refs("A");
    faulty.inject_fault(ProcessingFault::AfterObservation);

    let failed = run_claimed_record(&database, claim, &faulty).await;
    assert!(
        failed.is_err(),
        "an injected fault must not report a business outcome"
    );

    // Business writes rolled back, but the run itself is on the record.
    assert_eq!(count(&database, "content_observation").await, 0);
    assert_eq!(count(&database, "content_current_revision").await, 0);
    assert_eq!(count(&database, "record_processing_attempt").await, 1);
    assert_eq!(
        scalar(
            &database,
            "SELECT count(*) FROM record_processing_attempt WHERE finalized_at IS NULL AND run_error IS NOT NULL"
        )
        .await,
        1,
        "a crashed run must leave a traceable attempt, not disappear"
    );
    assert_eq!(
        scalar(
            &database,
            "SELECT count(*) FROM record_processing_work WHERE business_outcome IS NOT NULL"
        )
        .await,
        0
    );

    // After the lease expires the work is takeable again and completes normally.
    raw(
        &database,
        "UPDATE record_processing_attempt SET lease_expires_at = scope_001_now() - interval '1 minute' WHERE finalized_at IS NULL",
    )
    .await
    .expect("expiring a lease is legal");
    let retried = process_one_ready_record(&database, &f01_processing_options(0))
        .await
        .expect("the work must be recoverable");
    match retried {
        ProcessingOutcome::Processed(record) => {
            assert_eq!(record.epoch, 2);
            assert_eq!(
                record.business_outcome,
                BusinessOutcome::ObservationRecorded
            );
        }
        ProcessingOutcome::NothingReady => panic!("an expired lease must make the work takeable"),
    }
}

// ---------------------------------------------------------------------------
// Least-privilege runtime role. Triggers state what is true; permissions decide who may even
// attempt to write. Both layers are proved here, separately.
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
async fn the_runtime_role_runs_the_chain_but_cannot_rewrite_history_or_change_the_schema() {
    let schema = "f01_runtime_role";
    let admin = accepted_f01_database(schema, F01_PACKAGE).await;
    let runtime = runtime_role_database(schema).await;

    // The minimum privileges are enough to finish real work.
    let processed = process_one_ready_record(&runtime, &f01_processing_options(0))
        .await
        .expect("the runtime role must be able to process a record");
    match processed {
        ProcessingOutcome::Processed(record) => assert_eq!(
            record.business_outcome,
            BusinessOutcome::ObservationRecorded
        ),
        ProcessingOutcome::NothingReady => panic!("record A was ready"),
    }

    // They are not enough to rewrite history or to touch the schema.
    for (statement, what) in [
        ("DELETE FROM content_observation", "delete an observation"),
        (
            "UPDATE content_current_revision SET title_value = 'FORGED'",
            "rewrite a current revision",
        ),
        (
            "DELETE FROM content_current_revision_field_source",
            "delete a field source",
        ),
        ("DELETE FROM capture_record", "delete a capture record"),
        ("DELETE FROM capture_package", "delete a package"),
        (
            "ALTER TABLE capture_record ADD COLUMN forged text",
            "alter a table",
        ),
        ("CREATE TABLE forged (id bigint)", "create a table"),
    ] {
        let error = raw(&runtime, statement)
            .await
            .expect_err(&format!("the runtime role must not be able to {what}"))
            .to_string();
        assert!(
            error.contains("permission denied") || error.contains("must be owner"),
            "expected a privilege refusal when trying to {what}, got: {error}"
        );
    }

    // Nothing above changed the facts the admin connection can still read.
    assert_eq!(count(&admin, "content_observation").await, 1);
    assert_eq!(count(&admin, "capture_record").await, 2);
    assert_eq!(count(&admin, "content_current_revision").await, 1);
}
