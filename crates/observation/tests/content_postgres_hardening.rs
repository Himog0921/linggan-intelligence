//! Round-4 F01 database hardening proofs.
//!
//! These direct PostgreSQL attacks cover the repair guards that must hold even
//! when a caller bypasses the Rust processing facade.

mod support;

use linggan_observation::{
    BusinessOutcome, ProcessingFault, ProcessingOptions, ProcessingOutcome, claim_one_ready_record,
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

// This constraint attack belongs with the other database-owned hardening proofs. Moving it from
// the ingress happy-path target keeps both proof binaries below the repository warning limit
// without changing the assertion, fixture, or covered boundary.
#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
async fn database_refuses_cross_parent_ingress_rows_written_around_the_rust_facade() {
    let database = accepted_f01_database("f01_cross_parent", F01_PACKAGE).await;

    raw(
        &database,
        "INSERT INTO capture_work_order \
             (work_order_ref, contract_version, target_basis, target_unit, target_manifest_hash, known_target_count, quota_limit) \
         VALUES \
             ('00000000-0000-4000-8000-000000000901', 'content-detail.synthetic.v1', 'known_set', 'content_detail', \
              'sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff', 1, NULL); \
         INSERT INTO capture_attempt \
             (attempt_ref, capture_identity, work_order_id, lease_epoch, authority_valid_until) \
         SELECT '00000000-0000-4000-8000-000000000902', '00000000-0000-4000-8000-000000000903', id, 1, \
                scope_001_now() + interval '1 hour' \
         FROM capture_work_order \
         WHERE work_order_ref = '00000000-0000-4000-8000-000000000901'",
    )
    .await
    .expect("the second work and attempt are only constraint-negative fixtures");

    let cross_parent_delivery = sqlx::query(
        "INSERT INTO capture_ingress_delivery \
             (audit_kind, delivery_ref, work_order_id, attempt_id, capture_identity, outcome, external_code) \
         SELECT 'public_delivery', '00000000-0000-4000-8000-000000000904', first_work.id, \
                second_attempt.id, second_attempt.capture_identity, 'rejected', 'lease_epoch_mismatch' \
         FROM capture_work_order first_work \
         JOIN capture_attempt second_attempt \
           ON second_attempt.attempt_ref = '00000000-0000-4000-8000-000000000902' \
         WHERE first_work.work_order_ref = $1",
    )
    .bind(manifest_ref("workOrderRef"))
    .execute(database.pool())
    .await;
    assert!(
        cross_parent_delivery.is_err(),
        "a delivery that mixes one work with another work's attempt must be rejected by the database"
    );
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
        &format!("REVOKE EXECUTE ON FUNCTION scope_001_record_processing_run_error(uuid, integer, text) FROM {role}"),
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

// Least-privilege runtime role. Triggers state what is true; permissions decide who may even
// attempt to write. Both layers are proved here, separately.
#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
async fn the_runtime_role_runs_the_chain_but_cannot_rewrite_history_or_change_the_schema() {
    let schema = "f01_runtime_role";
    let admin = accepted_f01_database(schema, F01_PACKAGE).await;
    let runtime = runtime_role_database(schema).await;

    // The database-owned operations are sufficient to finish real work without any table grant.
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

    // They are not enough to rewrite history, read protected tables or touch the schema.
    for (statement, what) in [
        (
            "SELECT * FROM capture_record",
            "read an accepted record directly",
        ),
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
    assert_refused(fabricated_identity, "permission denied");

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

// Round-5: a runtime writer must not be able to reuse an accepted external id and turn it into
// an independent Source -> Observation -> Current -> outcome chain. This starts RED against the
// broad direct-table grants: the old runtime role can create the identity and content anchors
// directly, even though it does not invent a new external id.
#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
async fn runtime_cannot_reuse_an_accepted_external_id_to_forge_a_fact_chain() {
    let schema = "f01_runtime_reused_identity";
    let admin = accepted_f01_database(schema, F01_PACKAGE).await;
    let runtime = runtime_role_database(schema).await;

    let forged = raw(
        &runtime,
        "WITH identity AS (\
             INSERT INTO source_identity \
                 (source_identity_ref, source_system, namespace, object_type, external_id) \
             VALUES (gen_random_uuid(), 'synthetic', 'scope-001', 'content', 'synthetic-note-a') \
             RETURNING id\
         ), content AS (\
             INSERT INTO source_content (content_ref, source_identity_id) \
             SELECT gen_random_uuid(), id FROM identity RETURNING id\
         ), observation AS (\
             INSERT INTO content_observation \
                 (observation_ref, source_content_id, capture_record_id, observed_at, \
                  observed_at_precision, parser_version, title_observed, title_value, \
                  body_observed, body_value) \
             SELECT gen_random_uuid(), content.id, r.id, r.observed_at, r.observed_at_precision, \
                    'content-detail-processor-v1', true, 'FORGED', true, 'FORGED' \
             FROM content, capture_record r WHERE r.ordinal = 1 RETURNING id, source_content_id\
         ), revision AS (\
             INSERT INTO content_current_revision \
                 (revision_ref, source_content_id, policy_version, title_state, title_value, body_state, body_value, watermark) \
             SELECT gen_random_uuid(), source_content_id, 'content-current-policy-v1', 'selected', 'FORGED', 'selected', 'FORGED', '[]'::jsonb \
             FROM observation RETURNING id, source_content_id\
         ), sources AS (\
             INSERT INTO content_current_revision_field_source \
                 (field_source_ref, revision_id, source_content_id, field_kind, observation_id, role) \
             SELECT gen_random_uuid(), revision.id, revision.source_content_id, 'title', observation.id, 'selected_support' \
             FROM revision JOIN observation USING (source_content_id) RETURNING revision_id\
         ), published AS (\
             UPDATE content_current_revision revision SET published_at = scope_001_now() \
             FROM sources WHERE revision.id = sources.revision_id RETURNING revision.id, revision.source_content_id\
         ), current AS (\
             UPDATE source_content content SET current_revision_id = published.id \
             FROM published WHERE content.id = published.source_content_id RETURNING content.id\
         ), finalized_attempt AS (\
             INSERT INTO record_processing_attempt \
                 (processing_attempt_ref, processing_work_id, epoch, lease_expires_at, finalized_at) \
             SELECT gen_random_uuid(), work.id, 1, scope_001_now() + interval '5 minutes', scope_001_now() \
             FROM record_processing_work work CROSS JOIN current WHERE work.capture_record_id = (SELECT id FROM capture_record WHERE ordinal = 1) \
             RETURNING processing_work_id\
         )\
         UPDATE record_processing_work work SET business_outcome = 'observation_recorded' \
         FROM finalized_attempt WHERE work.id = finalized_attempt.processing_work_id",
    )
    .await;
    assert_refused(forged, "permission denied");
    assert_eq!(count(&admin, "source_identity").await, 0);
    assert_eq!(count(&admin, "source_content").await, 0);
    assert_eq!(count(&admin, "content_observation").await, 0);
    assert_eq!(count(&admin, "content_current_revision").await, 0);
    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*) FROM record_processing_work WHERE business_outcome IS NOT NULL"
        )
        .await,
        0
    );
}

// Round-6 RED: SECURITY DEFINER must resolve the trusted proof schema, never a runtime
// session's TEMP objects. The temporary relations are intentionally malformed: an insecure
// function either reads an empty fake queue or fails on a fake relation. A secure function must
// still claim and process the real accepted record.
#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
async fn runtime_temp_shadow_relations_cannot_change_claim_or_processing_facts() {
    let schema = "f01_runtime_temp_shadow";
    let admin = accepted_f01_database(schema, F01_PACKAGE).await;
    let runtime = runtime_role_database(schema).await;

    raw(
        &runtime,
        "CREATE TEMP TABLE record_processing_work (id bigint, processing_work_ref uuid, capture_record_id bigint, business_outcome text); \
         CREATE TEMP TABLE capture_record (id bigint, record_ref uuid, target_external_id text, source_external_id text, payload jsonb); \
         CREATE TEMP TABLE record_processing_attempt (id bigint, processing_work_id bigint, epoch integer, finalized_at timestamptz, lease_expires_at timestamptz); \
         CREATE TEMP TABLE source_identity (id bigint); \
         CREATE TEMP TABLE source_content (id bigint); \
         CREATE TEMP TABLE content_observation (id bigint); \
         CREATE TEMP TABLE content_current_revision (id bigint); \
         CREATE TEMP TABLE content_current_revision_field_source (id bigint); \
         CREATE TEMP TABLE capture_package (id bigint); \
         CREATE TEMP TABLE capture_ingress_delivery (id bigint); \
         CREATE OR REPLACE FUNCTION pg_temp.scope_001_now() RETURNS timestamptz LANGUAGE sql AS $$ SELECT '2000-01-01T00:00:00Z'::timestamptz $$; \
         SET search_path = pg_temp, f01_runtime_temp_shadow",
    )
    .await
    .expect("the runtime may create session-local shadows for this security proof");

    let processed = process_one_ready_record(&runtime, &f01_processing_options(0))
        .await
        .expect("TEMP shadows must not change the real function-owned chain");
    assert!(
        matches!(processed, ProcessingOutcome::Processed(record) if record.business_outcome == BusinessOutcome::ObservationRecorded)
    );
    assert_eq!(count(&admin, "record_processing_attempt").await, 1);
    assert_eq!(count(&admin, "content_observation").await, 1);
    assert_eq!(count(&admin, "content_current_revision").await, 1);
}

// Round-7 RED: `run_error` is an independently callable SECURITY DEFINER entry. It must retain
// the same trusted-schema guarantee as claim and processing even after a real claim has created
// an open attempt. This invokes the narrow entry directly on the attack session: no Rust facade
// may hide a temporary-relation, temporary-function or caller search-path resolution mistake.
#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
async fn runtime_temp_shadow_cannot_redirect_a_claimed_run_error() {
    let schema = "f01_run_error_temp_shadow";
    let admin = accepted_f01_database(schema, F01_PACKAGE).await;
    let runtime = runtime_role_database(schema).await;
    let claim = claim_one_ready_record(&runtime, &attempt_ref_only())
        .await
        .expect("the runtime role must claim a real accepted record")
        .expect("record A must be ready for the run-error attack");
    let error_text = "run-error must stay on the real claimed attempt";

    // Hold one session while creating the attacker-controlled TEMP objects and invoking the
    // function. A vulnerable SECURITY DEFINER function would either update the malformed fake
    // relations or dispatch to the fake same-signature function below.
    let mut session = runtime
        .pool()
        .acquire()
        .await
        .expect("the runtime attack session must be available");
    sqlx::raw_sql(
        "CREATE TEMP TABLE record_processing_work (decoy text); \
         CREATE TEMP TABLE record_processing_attempt (decoy text); \
         CREATE TEMP TABLE scope_001_run_error_decoy (id bigint GENERATED ALWAYS AS IDENTITY); \
         CREATE OR REPLACE FUNCTION pg_temp.scope_001_record_processing_run_error(uuid, integer, text) \
         RETURNS void LANGUAGE plpgsql AS $$ \
         BEGIN INSERT INTO scope_001_run_error_decoy DEFAULT VALUES; END; $$; \
         SET search_path = pg_temp, f01_run_error_temp_shadow",
    )
    .execute(&mut *session)
    .await
    .expect("the runtime may create session-local shadows for this security proof");

    sqlx::query("SELECT scope_001_record_processing_run_error($1, $2, $3)")
        .bind(claim.processing_work_ref())
        .bind(claim.epoch())
        .bind(error_text)
        .execute(&mut *session)
        .await
        .expect("the trusted run-error entry must update the real claim despite TEMP shadows");
    let decoy_calls: i64 = sqlx::query_scalar("SELECT count(*) FROM scope_001_run_error_decoy")
        .fetch_one(&mut *session)
        .await
        .expect("the temporary decoy marker must remain queryable in the same session");
    assert_eq!(
        decoy_calls, 0,
        "the caller's fake same-signature function must never receive the run-error call"
    );
    drop(session);

    let stored_error: Option<String> = sqlx::query_scalar(
        "SELECT a.run_error \
         FROM record_processing_attempt a \
         JOIN record_processing_work w ON w.id = a.processing_work_id \
         WHERE w.processing_work_ref = $1 AND a.epoch = $2",
    )
    .bind(claim.processing_work_ref())
    .bind(claim.epoch())
    .fetch_one(admin.pool())
    .await
    .expect("the real claimed attempt must remain readable to the proof admin");
    assert_eq!(stored_error.as_deref(), Some(error_text));
    assert_eq!(count(&admin, "content_observation").await, 0);
    assert_eq!(count(&admin, "content_current_revision").await, 0);
    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*) FROM record_processing_work WHERE business_outcome IS NOT NULL",
        )
        .await,
        0,
        "run-error persistence must not create a business outcome"
    );
}

// Round-6 RED: delivery is ingress audit history. This attacks it as the trusted admin rather
// than relying on the runtime role's absence of table privileges.
#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
async fn accepted_delivery_is_append_only_even_for_direct_owner_sql() {
    let database = accepted_f01_database("f01_delivery_append_only", F01_PACKAGE).await;
    let before = count(&database, "capture_ingress_delivery").await;
    for statement in [
        "UPDATE capture_ingress_delivery SET delivery_ref = gen_random_uuid()",
        "UPDATE capture_ingress_delivery SET work_order_id = NULL",
        "UPDATE capture_ingress_delivery SET attempt_id = NULL",
        "UPDATE capture_ingress_delivery SET capture_identity = NULL",
        "UPDATE capture_ingress_delivery SET outcome = 'conflict'",
        "UPDATE capture_ingress_delivery SET external_code = 'authority_expired'",
        "UPDATE capture_ingress_delivery SET received_at = scope_001_now() + interval '1 second'",
        "DELETE FROM capture_ingress_delivery",
    ] {
        assert_refused(raw(&database, statement).await, "append-only");
        assert_eq!(count(&database, "capture_ingress_delivery").await, before);
        assert_eq!(count(&database, "capture_package").await, 1);
        assert_eq!(count(&database, "capture_record").await, 2);
        assert_eq!(count(&database, "record_processing_work").await, 2);
        assert_eq!(count(&database, "content_observation").await, 0);
    }
}

// Round-5: valid accepted material remains available, but it is no longer editable or deletable
// merely because an administrator can issue SQL. Each attempted overwrite must roll back without
// changing any protected evidence-side row.
#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
async fn accepted_evidence_side_rows_are_append_only_after_acceptance() {
    let database = accepted_f01_database("f01_accepted_evidence_append_only", F01_PACKAGE).await;
    processed(&database, 0).await;
    let before = (
        count(&database, "capture_package").await,
        count(&database, "capture_record").await,
        count(&database, "capture_package_target_result").await,
        count(&database, "capture_package_coverage").await,
        count(&database, "content_observation").await,
        count(&database, "content_current_revision").await,
        scalar(
            &database,
            "SELECT count(*) FROM record_processing_work WHERE business_outcome IS NOT NULL",
        )
        .await,
    );

    for statement in [
        "UPDATE capture_package SET package_hash = decode(repeat('00', 32), 'hex')",
        "DELETE FROM capture_package",
        "UPDATE capture_record SET payload = '{}'::jsonb",
        "DELETE FROM capture_record",
        "UPDATE capture_package_target_result SET outcome = 'failed'",
        "DELETE FROM capture_package_target_result",
        "UPDATE capture_package_coverage SET emitted = 0",
        "DELETE FROM capture_package_coverage",
    ] {
        let rejected = raw(&database, statement).await;
        assert_refused(rejected, "accepted");
        assert_eq!(
            (
                count(&database, "capture_package").await,
                count(&database, "capture_record").await,
                count(&database, "capture_package_target_result").await,
                count(&database, "capture_package_coverage").await,
                count(&database, "content_observation").await,
                count(&database, "content_current_revision").await,
                scalar(
                    &database,
                    "SELECT count(*) FROM record_processing_work WHERE business_outcome IS NOT NULL",
                )
                .await,
            ),
            before,
            "a refused accepted-evidence attack must leave no partial side effect: {statement}"
        );
    }
}
