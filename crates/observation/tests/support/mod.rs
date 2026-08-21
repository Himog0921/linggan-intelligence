//! Shared fixtures and assertions for the F01 record processing proofs.
//!
//! Integration test binaries each compile this module separately, so items used by only one
//! of them would otherwise look dead.
#![allow(dead_code)]

use linggan_evidence::{IngressOptions, IngressOutcome, ingest_capture_package_with};
use linggan_observation::{
    ProcessedRecord, ProcessingOptions, ProcessingOutcome, process_one_ready_record,
};
use linggan_storage_postgres::{Database, testing::isolated_proof_schema};
use serde_json::Value;
use sqlx::{AssertSqlSafe, Row};
use uuid::Uuid;

pub const CAPTURE_MIGRATION: &str = "database/migrations/0001_scope_001_capture_evidence.sql";
pub const OBSERVATION_MIGRATION: &str =
    "database/migrations/0002_scope_001_content_observation.sql";
pub const F01_PACKAGE: &str =
    include_str!("../../../contracts/tests/fixtures/capture-v1/f01-complete-known-set.json");
pub const F01_PAYLOAD_EXTENSION: &str = include_str!(
    "../../../contracts/tests/fixtures/capture-v1/f01-complete-known-set-payload-extension.json"
);
pub const F01_MANIFEST: &str =
    include_str!("../../../contracts/tests/fixtures/capture-v1/manifest.json");
pub const F01_SOURCE_EXTERNAL_ID_NULL: &str =
    include_str!("../../../contracts/tests/fixtures/capture-v1/f01-source-external-id-null.json");

pub const DOWNSTREAM_TABLES: [(&str, &str); 6] = [
    ("processingAttempt", "record_processing_attempt"),
    ("sourceIdentity", "source_identity"),
    ("sourceContent", "source_content"),
    ("observation", "content_observation"),
    ("currentRevision", "content_current_revision"),
    ("fieldSource", "content_current_revision_field_source"),
];

pub async fn processed(database: &Database, index: usize) -> ProcessedRecord {
    match process_one_ready_record(database, &f01_processing_options(index))
        .await
        .expect("a ready record must be processable")
    {
        ProcessingOutcome::Processed(record) => record,
        ProcessingOutcome::NothingReady => panic!("record {index} should have been ready"),
    }
}

/// Frozen refs for one processing run, in the order the implementation mints them.
pub fn f01_processing_options(index: usize) -> ProcessingOptions {
    let suffix = if index == 0 { "A" } else { "B" };
    let mut options = ProcessingOptions::default();
    options.use_fixed_refs(vec![
        manifest_ref(&format!("processingAttempt{suffix}")),
        manifest_ref(&format!("sourceIdentity{suffix}")),
        manifest_ref(&format!("content{suffix}")),
        manifest_ref(&format!("observation{suffix}")),
        manifest_ref(&format!("currentRevision{suffix}")),
        manifest_ref(&format!("fieldSourceTitle{suffix}")),
        manifest_ref(&format!("fieldSourceBody{suffix}")),
    ]);
    options
}

pub async fn assert_current_matches_its_record(
    database: &Database,
    external_id: &str,
    suffix: &str,
) {
    let row = sqlx::query(
        "SELECT rev.title_state, rev.title_value, rev.body_state, rev.body_value \
         FROM content_current_revision rev \
         JOIN source_content c ON c.current_revision_id = rev.id \
         JOIN source_identity i ON i.id = c.source_identity_id \
         WHERE i.external_id = $1",
    )
    .bind(external_id)
    .fetch_one(database.pool())
    .await
    .unwrap_or_else(|error| panic!("{external_id} must have a published revision: {error}"));

    assert_eq!(row.get::<String, _>("title_state"), "selected");
    assert_eq!(row.get::<String, _>("body_state"), "selected");
    assert_eq!(
        row.get::<Option<String>, _>("title_value"),
        Some(format!("Synthetic title {suffix}"))
    );
    assert_eq!(
        row.get::<Option<String>, _>("body_value"),
        Some(format!("Synthetic body {suffix}"))
    );
}

pub async fn assert_downstream_rows(database: &Database, stage: &str, kind: &str) {
    let expected = manifest_stage(stage, kind);
    for (manifest_name, table) in DOWNSTREAM_TABLES {
        assert_eq!(
            count(database, table).await,
            expected[manifest_name].as_i64().expect("frozen row count"),
            "stage {stage} freezes {table}"
        );
    }
}

/// Each processing work's record ordinal and its finalized business outcome.
pub async fn finalized_outcomes(database: &Database) -> Vec<(i32, Option<String>)> {
    sqlx::query(
        "SELECT r.ordinal, w.business_outcome FROM record_processing_work w \
         JOIN capture_record r ON r.id = w.capture_record_id ORDER BY r.ordinal",
    )
    .fetch_all(database.pool())
    .await
    .expect("processing works must be readable")
    .iter()
    .map(|row| (row.get("ordinal"), row.get("business_outcome")))
    .collect()
}

pub async fn accepted_f01_database(schema: &str, package: &str) -> Database {
    let proof_database_url = std::env::var("SCOPE_001_PROOF_DATABASE_URL")
        .expect("test-scope-001-postgres.sh must provide an isolated proof database URL");
    let migrations = format!(
        "{}\n{}",
        read_workspace_file(CAPTURE_MIGRATION),
        read_workspace_file(OBSERVATION_MIGRATION)
    );
    let database = isolated_proof_schema(&proof_database_url, schema, &migrations)
        .await
        .expect("both F01 migrations must apply to an isolated empty schema");
    seed_f01_work_attempt_and_targets(&database).await;

    let mut options = IngressOptions::default();
    options.use_fixed_refs(vec![
        manifest_ref("deliveryRef"),
        manifest_ref("acceptedReceiptRef"),
        manifest_ref("packageRef"),
        manifest_ref("recordA"),
        manifest_ref("recordB"),
        manifest_ref("processingWorkA"),
        manifest_ref("processingWorkB"),
    ]);
    let accepted = ingest_capture_package_with(&database, package, &options)
        .await
        .expect("the proof package must be accepted before processing starts");
    assert!(matches!(accepted, IngressOutcome::Accepted { .. }));
    database
}

pub async fn seed_f01_work_attempt_and_targets(database: &Database) {
    sqlx::query(
        "INSERT INTO capture_work_order (work_order_ref, contract_version, target_basis, target_unit, target_manifest_hash, known_target_count, quota_limit) \
         VALUES ($1, 'content-detail.synthetic.v1', 'known_set', 'content_detail', $2, 2, NULL)",
    )
    .bind(manifest_ref("workOrderRef"))
    .bind(
        fixture_package(F01_PACKAGE)["target"]["targetManifestHash"]
            .as_str()
            .expect("the fixture freezes its target manifest hash"),
    )
    .execute(database.pool())
    .await
    .expect("the fresh seed must create the frozen work order");

    sqlx::query(
        "INSERT INTO capture_attempt (attempt_ref, capture_identity, work_order_id, lease_epoch, authority_valid_until) \
         SELECT $1, $2, id, 1, scope_001_now() + interval '1 hour' FROM capture_work_order WHERE work_order_ref = $3",
    )
    .bind(manifest_ref("attemptRef"))
    .bind(manifest_ref("captureIdentityRef"))
    .bind(manifest_ref("workOrderRef"))
    .execute(database.pool())
    .await
    .expect("the fresh seed must create the single authorized attempt");

    for (ordinal, external_id) in ["synthetic-note-a", "synthetic-note-b"]
        .into_iter()
        .enumerate()
    {
        sqlx::query(
            "INSERT INTO capture_work_order_target (work_order_id, ordinal, external_id) \
             SELECT id, $1, $2 FROM capture_work_order WHERE work_order_ref = $3",
        )
        .bind(i32::try_from(ordinal + 1).expect("fixture ordinals stay small"))
        .bind(external_id)
        .bind(manifest_ref("workOrderRef"))
        .execute(database.pool())
        .await
        .expect("the fresh seed must freeze both known targets");
    }
}

pub async fn count(database: &Database, table: &str) -> i64 {
    sqlx::query(AssertSqlSafe(format!(
        "SELECT count(*) AS count FROM {table}"
    )))
    .fetch_one(database.pool())
    .await
    .unwrap_or_else(|error| panic!("proof table {table} must be queryable: {error}"))
    .get("count")
}

pub fn manifest_stage(stage: &str, kind: &str) -> Value {
    f01_manifest()["expectedTableRows"]
        .as_array()
        .expect("the manifest must freeze staged row counts")
        .iter()
        .find(|row| row["stage"] == stage && row["kind"] == kind)
        .unwrap_or_else(|| panic!("the manifest must freeze the {kind} of stage {stage}"))
        .clone()
}

pub fn manifest_ref(name: &str) -> Uuid {
    Uuid::parse_str(
        f01_manifest()["fixedRefs"][name]
            .as_str()
            .unwrap_or_else(|| panic!("the manifest must freeze {name}")),
    )
    .expect("a frozen manifest ref must be a UUID")
}

pub fn f01_manifest() -> Value {
    let manifest: Value =
        serde_json::from_str(F01_MANIFEST).expect("the F01 manifest must remain valid JSON");
    manifest["fixtures"]
        .as_array()
        .expect("the manifest must list fixtures")
        .iter()
        .find(|fixture| fixture["fixtureId"] == "F01")
        .expect("the manifest must contain F01")
        .clone()
}

pub fn fixture_package(source: &str) -> Value {
    serde_json::from_str(source).expect("a capture fixture must remain valid JSON")
}

pub fn read_workspace_file(relative_path: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative_path);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("required file {relative_path} is unavailable: {error}"))
}

// Helpers for the database-level negatives
// ---------------------------------------------------------------------------

/// Runs one statement outside the Rust facade, in its own implicit transaction.
pub async fn raw(database: &Database, sql: &str) -> Result<(), sqlx::Error> {
    sqlx::raw_sql(AssertSqlSafe(sql.to_owned()))
        .execute(database.pool())
        .await
        .map(|_| ())
}

/// Asserts the database itself refused the write, and that it refused it for the stated reason.
pub fn assert_refused(result: Result<(), sqlx::Error>, expected_reason: &str) {
    let error = result.expect_err("the database must refuse this write");
    let message = error.to_string();
    assert!(
        message.contains(expected_reason),
        "expected a refusal mentioning {expected_reason:?}, got: {message}"
    );
}

pub async fn single_content_id(database: &Database) -> i64 {
    scalar(
        database,
        "SELECT id FROM source_content ORDER BY id LIMIT 1",
    )
    .await
}

pub async fn single_observation_id(database: &Database) -> i64 {
    scalar(
        database,
        "SELECT id FROM content_observation ORDER BY id LIMIT 1",
    )
    .await
}

pub async fn unfinalized_attempts(database: &Database) -> i64 {
    scalar(
        database,
        "SELECT count(*) FROM record_processing_attempt WHERE finalized_at IS NULL",
    )
    .await
}

pub async fn scalar(database: &Database, sql: &str) -> i64 {
    sqlx::query_scalar(AssertSqlSafe(sql.to_owned()))
        .fetch_one(database.pool())
        .await
        .unwrap_or_else(|error| panic!("proof query must succeed: {error}"))
}

pub async fn published_title(database: &Database, external_id: &str) -> String {
    sqlx::query(
        "SELECT rev.title_value FROM content_current_revision rev \
         JOIN source_content c ON c.current_revision_id = rev.id \
         JOIN source_identity i ON i.id = c.source_identity_id WHERE i.external_id = $1",
    )
    .bind(external_id)
    .fetch_one(database.pool())
    .await
    .expect("the content must have a published revision")
    .get::<Option<String>, _>("title_value")
    .expect("the published title must be selected")
}

/// Adds one more capture record and the observation it supports to an existing content.
pub async fn seed_identity_content_and_observation_for(
    database: &Database,
    content_id: i64,
    record_ordinal: i32,
    observed_at: &str,
    title: &str,
    body: &str,
) -> i64 {
    let record_id = insert_extra_capture_record(
        database,
        record_ordinal,
        "synthetic-note-a",
        Some("synthetic-note-a"),
        observed_at,
        title,
        body,
    )
    .await;
    sqlx::query(
        "INSERT INTO content_observation \
             (observation_ref, source_content_id, capture_record_id, observed_at, observed_at_precision, \
              parser_version, title_observed, title_value, body_observed, body_value) \
         SELECT gen_random_uuid(), $1, r.id, r.observed_at, r.observed_at_precision, \
                'content-detail-processor-v1', true, $3, true, $4 \
         FROM capture_record r WHERE r.id = $2 RETURNING id",
    )
    .bind(content_id)
    .bind(record_id)
    .bind(title)
    .bind(body)
    .fetch_one(database.pool())
    .await
    .expect("seeding an observation must succeed")
    .get("id")
}

/// Inserts one additional record into the accepted package, with its own processing work.
pub async fn insert_extra_capture_record_and_work(
    database: &Database,
    ordinal: i32,
    target_external_id: &str,
    source_external_id: Option<&str>,
    observed_at: &str,
    title: &str,
    body: &str,
) -> i64 {
    let record_id = insert_extra_capture_record(
        database,
        ordinal,
        target_external_id,
        source_external_id,
        observed_at,
        title,
        body,
    )
    .await;
    sqlx::query(
        "INSERT INTO record_processing_work (processing_work_ref, capture_record_id, processor_version) \
         VALUES (gen_random_uuid(), $1, 'content-detail-processor-v1')",
    )
    .bind(record_id)
    .execute(database.pool())
    .await
    .expect("the extra record needs its own processing work");
    record_id
}

pub async fn insert_extra_capture_record(
    database: &Database,
    ordinal: i32,
    target_external_id: &str,
    source_external_id: Option<&str>,
    observed_at: &str,
    title: &str,
    body: &str,
) -> i64 {
    let payload = serde_json::json!({
        "schemaVersion": "content-detail.synthetic.v1",
        "sourceExternalId": source_external_id,
        "fields": {
            "title": {"observed": true, "value": title},
            "body": {"observed": true, "value": body},
        }
    });
    sqlx::query(
        "INSERT INTO capture_record \
             (record_ref, package_id, ordinal, target_external_id, record_hash, payload, record_kind, \
              source_system, source_namespace, source_object_type, source_external_id, source_channel, \
              observed_at, observed_at_precision, observed_at_basis) \
         SELECT gen_random_uuid(), p.id, $1, $2, \
                'sha256:0000000000000000000000000000000000000000000000000000000000000000', $3, \
                'content_detail', 'synthetic', 'scope-001', 'content', $4, 'synthetic_page', \
                $5::timestamptz, 'exact', 'fixture' \
         FROM capture_package p RETURNING id",
    )
    .bind(ordinal)
    .bind(target_external_id)
    .bind(&payload)
    .bind(source_external_id)
    .bind(observed_at)
    .fetch_one(database.pool())
    .await
    .expect("the extra capture record must satisfy the envelope contract")
    .get("id")
}

/// Inserts a revision and its field sources in one transaction, exactly as a bypassing writer
/// would, so the deferred constraint triggers decide the outcome.
pub async fn insert_revision_with_sources(
    database: &Database,
    content_id: i64,
    fields: &[(&str, &str, &str)],
    sources: &[(&str, i64, &str)],
) -> Result<(), sqlx::Error> {
    let mut transaction = database.pool().begin().await?;
    let title = fields
        .iter()
        .find(|(kind, _, _)| *kind == "title")
        .expect("title state");
    let body = fields
        .iter()
        .find(|(kind, _, _)| *kind == "body")
        .expect("body state");
    let watermark: Value = sqlx::query_scalar(
        "SELECT coalesce(jsonb_agg(entry ORDER BY entry ->> 'packageRef'), '[]'::jsonb) FROM ( \
             SELECT jsonb_build_object( \
                 'packageRef', p.package_ref, 'originalAcceptedDeliveryRef', d.delivery_ref, \
                 'acceptedReceiptRef', p.accepted_receipt_ref, \
                 'recordRefs', jsonb_agg(DISTINCT to_jsonb(r.record_ref::text)), \
                 'observationRefs', jsonb_agg(DISTINCT to_jsonb(o.observation_ref::text))) AS entry \
             FROM content_observation o \
             JOIN capture_record r ON r.id = o.capture_record_id \
             JOIN capture_package p ON p.id = r.package_id \
             JOIN capture_ingress_delivery d ON d.id = p.accepted_delivery_id \
             WHERE o.source_content_id = $1 \
             GROUP BY p.package_ref, d.delivery_ref, p.accepted_receipt_ref) grouped",
    )
    .bind(content_id)
    .fetch_one(&mut *transaction)
    .await?;

    let revision_id: i64 = sqlx::query_scalar(
        "INSERT INTO content_current_revision \
             (revision_ref, source_content_id, policy_version, title_state, title_value, body_state, body_value, watermark) \
         VALUES (gen_random_uuid(), $1, 'content-current-policy-v1', $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(content_id)
    .bind(title.2)
    .bind(if title.2 == "selected" { Some(title.1) } else { None })
    .bind(body.2)
    .bind(if body.2 == "selected" { Some(body.1) } else { None })
    .bind(&watermark)
    .fetch_one(&mut *transaction)
    .await?;

    for (field_kind, observation_id, role) in sources {
        sqlx::query(
            "INSERT INTO content_current_revision_field_source \
                 (field_source_ref, revision_id, source_content_id, field_kind, observation_id, role) \
             VALUES (gen_random_uuid(), $1, $2, $3, $4, $5)",
        )
        .bind(revision_id)
        .bind(content_id)
        .bind(field_kind)
        .bind(observation_id)
        .bind(role)
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await
}
