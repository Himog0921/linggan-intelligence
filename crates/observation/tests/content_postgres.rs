//! F01 record processing proof against a real, isolated PostgreSQL schema. Expected row counts
//! come from the hand-maintained fixture manifest; the implementation never supplies them.

use linggan_evidence::{IngressOptions, IngressOutcome, ingest_capture_package_with};
use linggan_observation::{
    BusinessOutcome, ProcessedRecord, ProcessingOptions, ProcessingOutcome,
    process_one_ready_record,
};
use linggan_storage_postgres::{Database, testing::isolated_proof_schema};
use serde_json::Value;
use sqlx::{AssertSqlSafe, Row};
use uuid::Uuid;

const CAPTURE_MIGRATION: &str = "database/migrations/0001_scope_001_capture_evidence.sql";
const OBSERVATION_MIGRATION: &str = "database/migrations/0002_scope_001_content_observation.sql";
const F01_PACKAGE: &str =
    include_str!("../../contracts/tests/fixtures/capture-v1/f01-complete-known-set.json");
const F01_PAYLOAD_EXTENSION: &str = include_str!(
    "../../contracts/tests/fixtures/capture-v1/f01-complete-known-set-payload-extension.json"
);
const F01_MANIFEST: &str = include_str!("../../contracts/tests/fixtures/capture-v1/manifest.json");

const DOWNSTREAM_TABLES: [(&str, &str); 6] = [
    ("processingAttempt", "record_processing_attempt"),
    ("sourceIdentity", "source_identity"),
    ("sourceContent", "source_content"),
    ("observation", "content_observation"),
    ("currentRevision", "content_current_revision"),
    ("fieldSource", "content_current_revision_field_source"),
];

#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
async fn f01_records_are_processed_independently_into_observation_and_current() {
    let database = accepted_f01_database("f01_processing", F01_PACKAGE).await;

    // Acceptance alone forms nothing downstream.
    assert_downstream_rows(&database, "accepted_ingress", "delta").await;

    let first = processed(&database, 0).await;
    assert_eq!(first.business_outcome, BusinessOutcome::ObservationRecorded);
    assert_eq!(first.epoch, 1);
    assert_eq!(first.record_ref, manifest_ref("recordA"));

    // Record B has not been touched: exactly one of everything downstream exists.
    for (_, table) in DOWNSTREAM_TABLES {
        let expected = if table == "content_current_revision_field_source" {
            2
        } else {
            1
        };
        assert_eq!(
            count(&database, table).await,
            expected,
            "after only Record A, {table} must hold Record A's facts and nothing else"
        );
    }
    assert_eq!(
        finalized_outcomes(&database).await,
        vec![(1_i32, Some("observation_recorded".to_owned())), (2, None)],
        "Record B's work must still be unfinalized rather than presumed complete"
    );

    let second = processed(&database, 1).await;
    assert_eq!(
        second.business_outcome,
        BusinessOutcome::ObservationRecorded
    );
    assert_eq!(second.epoch, 1, "each record gets its own epoch sequence");
    assert_eq!(second.record_ref, manifest_ref("recordB"));

    assert_downstream_rows(&database, "final", "total").await;
    assert_eq!(
        finalized_outcomes(&database).await,
        vec![
            (1_i32, Some("observation_recorded".to_owned())),
            (2, Some("observation_recorded".to_owned()))
        ]
    );
    assert!(
        matches!(
            process_one_ready_record(&database, &f01_processing_options(2)).await,
            Ok(ProcessingOutcome::NothingReady)
        ),
        "a finalized work must not be claimable again"
    );

    assert_current_matches_its_record(&database, "synthetic-note-a", "A").await;
    assert_current_matches_its_record(&database, "synthetic-note-b", "B").await;
}

#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
async fn f01_bad_record_does_not_revoke_the_qualified_record_in_the_same_package() {
    let database = accepted_f01_database("f01_bad_record", F01_PAYLOAD_EXTENSION).await;

    let bad = processed(&database, 0).await;
    assert_eq!(
        bad.business_outcome,
        BusinessOutcome::RecordContractInvalid,
        "an undeclared payload field is a per-record contract failure, not an ingress failure"
    );
    for (_, table) in DOWNSTREAM_TABLES {
        if table == "record_processing_attempt" {
            continue;
        }
        assert_eq!(
            count(&database, table).await,
            0,
            "a contract-invalid record must not create an empty {table}"
        );
    }

    let good = processed(&database, 1).await;
    assert_eq!(good.business_outcome, BusinessOutcome::ObservationRecorded);

    assert_eq!(count(&database, "source_identity").await, 1);
    assert_eq!(count(&database, "content_observation").await, 1);
    assert_eq!(count(&database, "content_current_revision").await, 1);
    assert_eq!(
        count(&database, "content_current_revision_field_source").await,
        2
    );
    assert_eq!(
        finalized_outcomes(&database).await,
        vec![
            (1_i32, Some("record_contract_invalid".to_owned())),
            (2, Some("observation_recorded".to_owned()))
        ]
    );

    // The package and both processing works survive the bad member untouched.
    assert_eq!(count(&database, "capture_package").await, 1);
    assert_eq!(count(&database, "capture_record").await, 2);
    assert_eq!(count(&database, "capture_package_coverage").await, 1);
    assert_eq!(count(&database, "record_processing_work").await, 2);
    assert_current_matches_its_record(&database, "synthetic-note-b", "B").await;
}

#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
async fn f01_current_field_provenance_reaches_its_observation_record_and_package() {
    let database = accepted_f01_database("f01_provenance", F01_PACKAGE).await;
    processed(&database, 0).await;

    let rows = sqlx::query(
        "SELECT fs.field_kind, fs.role, o.observation_ref, r.record_ref, p.package_ref, \
                rev.watermark, rev.policy_version \
         FROM content_current_revision_field_source fs \
         JOIN content_current_revision rev ON rev.id = fs.revision_id \
         JOIN content_observation o ON o.id = fs.observation_id \
         JOIN capture_record r ON r.id = o.capture_record_id \
         JOIN capture_package p ON p.id = r.package_id \
         ORDER BY fs.field_kind",
    )
    .fetch_all(database.pool())
    .await
    .expect("every current field must reach its own observation, record and package");
    assert_eq!(rows.len(), 2, "title and body each fix their own source");

    let expected_watermark = serde_json::json!([{
        "packageRef": manifest_ref("packageRef").to_string(),
        "originalAcceptedDeliveryRef": manifest_ref("deliveryRef").to_string(),
        "acceptedReceiptRef": manifest_ref("acceptedReceiptRef").to_string(),
        "recordRefs": [manifest_ref("recordA").to_string()],
        "observationRefs": [manifest_ref("observationA").to_string()],
    }]);
    for row in &rows {
        assert_eq!(row.get::<String, _>("role"), "selected_support");
        assert_eq!(
            row.get::<Uuid, _>("observation_ref"),
            manifest_ref("observationA")
        );
        assert_eq!(row.get::<Uuid, _>("record_ref"), manifest_ref("recordA"));
        assert_eq!(
            row.get::<Uuid, _>("package_ref"),
            manifest_ref("packageRef")
        );
        assert_eq!(
            row.get::<String, _>("policy_version"),
            "content-current-policy-v1"
        );
        assert_eq!(
            row.get::<Value, _>("watermark"),
            expected_watermark,
            "the watermark must name the exact inputs this recomputation locked"
        );
    }
    assert_eq!(
        rows.iter()
            .map(|row| row.get::<String, _>("field_kind"))
            .collect::<Vec<_>>(),
        vec!["body".to_owned(), "title".to_owned()]
    );
}

#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
async fn f01_observation_uses_the_producer_observed_at_not_a_processing_time() {
    let database = accepted_f01_database("f01_observed_at", F01_PACKAGE).await;
    processed(&database, 0).await;

    let row = sqlx::query(
        "SELECT to_char(o.observed_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS observed_at_utc, \
                o.observed_at_precision, o.parser_version, o.title_observed, o.title_value, \
                o.body_observed, o.body_value \
         FROM content_observation o",
    )
    .fetch_one(database.pool())
    .await
    .expect("the observation must be readable");

    let record = &fixture_package(F01_PACKAGE)["records"][0];
    assert_eq!(
        row.get::<String, _>("observed_at_utc"),
        record["observedAt"]["value"]
    );
    assert_eq!(row.get::<String, _>("observed_at_precision"), "exact");
    assert_eq!(
        row.get::<String, _>("parser_version"),
        "content-detail-processor-v1"
    );
    assert!(row.get::<bool, _>("title_observed"));
    assert!(row.get::<bool, _>("body_observed"));
    assert_eq!(
        row.get::<Option<String>, _>("title_value").as_deref(),
        record["payload"]["fields"]["title"]["value"].as_str()
    );
    assert_eq!(
        row.get::<Option<String>, _>("body_value").as_deref(),
        record["payload"]["fields"]["body"]["value"].as_str()
    );
}

async fn processed(database: &Database, index: usize) -> ProcessedRecord {
    match process_one_ready_record(database, &f01_processing_options(index))
        .await
        .expect("a ready record must be processable")
    {
        ProcessingOutcome::Processed(record) => record,
        ProcessingOutcome::NothingReady => panic!("record {index} should have been ready"),
    }
}

/// Frozen refs for one processing run, in the order the implementation mints them.
fn f01_processing_options(index: usize) -> ProcessingOptions {
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

async fn assert_current_matches_its_record(database: &Database, external_id: &str, suffix: &str) {
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

async fn assert_downstream_rows(database: &Database, stage: &str, kind: &str) {
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
async fn finalized_outcomes(database: &Database) -> Vec<(i32, Option<String>)> {
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

async fn accepted_f01_database(schema: &str, package: &str) -> Database {
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

async fn seed_f01_work_attempt_and_targets(database: &Database) {
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

async fn count(database: &Database, table: &str) -> i64 {
    sqlx::query(AssertSqlSafe(format!(
        "SELECT count(*) AS count FROM {table}"
    )))
    .fetch_one(database.pool())
    .await
    .unwrap_or_else(|error| panic!("proof table {table} must be queryable: {error}"))
    .get("count")
}

fn manifest_stage(stage: &str, kind: &str) -> Value {
    f01_manifest()["expectedTableRows"]
        .as_array()
        .expect("the manifest must freeze staged row counts")
        .iter()
        .find(|row| row["stage"] == stage && row["kind"] == kind)
        .unwrap_or_else(|| panic!("the manifest must freeze the {kind} of stage {stage}"))
        .clone()
}

fn manifest_ref(name: &str) -> Uuid {
    Uuid::parse_str(
        f01_manifest()["fixedRefs"][name]
            .as_str()
            .unwrap_or_else(|| panic!("the manifest must freeze {name}")),
    )
    .expect("a frozen manifest ref must be a UUID")
}

fn f01_manifest() -> Value {
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

fn fixture_package(source: &str) -> Value {
    serde_json::from_str(source).expect("a capture fixture must remain valid JSON")
}

fn read_workspace_file(relative_path: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative_path);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("required file {relative_path} is unavailable: {error}"))
}
