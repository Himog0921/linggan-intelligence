//! F01 ingress proof against a real, isolated PostgreSQL schema. Every expected row count comes
//! from the hand-maintained fixture manifest so neither the test nor the implementation can
//! derive a second version of the truth.

use linggan_evidence::{
    IngressError, IngressFault, IngressOptions, IngressOutcome, PreRoutingCode,
    ingest_capture_package_with,
};
use linggan_storage_postgres::{Database, testing::isolated_proof_schema};
use serde_json::Value;
use sqlx::{AssertSqlSafe, Row};
use uuid::Uuid;

const CAPTURE_MIGRATION: &str = "database/migrations/0001_scope_001_capture_evidence.sql";
const OBSERVATION_MIGRATION: &str = "database/migrations/0002_scope_001_content_observation.sql";
const F01_PACKAGE: &str =
    include_str!("../../contracts/tests/fixtures/capture-v1/f01-complete-known-set.json");
const F01_PACKAGE_OTHER_HASH: &str = include_str!(
    "../../contracts/tests/fixtures/capture-v1/f01-complete-known-set-payload-extension.json"
);
const F01_MANIFEST: &str = include_str!("../../contracts/tests/fixtures/capture-v1/manifest.json");
const F01_SOURCE_EXTERNAL_ID_NULL: &str =
    include_str!("../../contracts/tests/fixtures/capture-v1/f01-source-external-id-null.json");
const F01_SOURCE_EXTERNAL_ID_MISSING: &str =
    include_str!("../../contracts/tests/fixtures/capture-v1/f01-source-external-id-missing.json");
const F01_OBSERVED_AT_INVALID: &str =
    include_str!("../../contracts/tests/fixtures/capture-v1/f01-observed-at-invalid.json");

/// The tables the F01 foundation migration owns, keyed by their manifest name.
const FOUNDATION_TABLES: [(&str, &str); 9] = [
    ("work", "capture_work_order"),
    ("attempt", "capture_attempt"),
    ("workTarget", "capture_work_order_target"),
    ("delivery", "capture_ingress_delivery"),
    ("package", "capture_package"),
    ("record", "capture_record"),
    ("targetResult", "capture_package_target_result"),
    ("coverage", "capture_package_coverage"),
    ("processingWork", "record_processing_work"),
];

/// Downstream tables the F01 main chain must not create during ingress. They belong to the
/// record-processing migration, so at this stage "zero rows" means "no table at all".
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
async fn f01_valid_package_is_accepted_atomically_and_forms_no_observation_or_current() {
    let database = fresh_f01_database("f01_accepted").await;

    assert_stage_rows(&database, "fresh_seed", "total").await;

    let outcome = ingest_capture_package_with(&database, F01_PACKAGE, &f01_fixed_options())
        .await
        .expect("a valid F01 package must be accepted");

    let IngressOutcome::Accepted {
        delivery_ref,
        package_ref,
        accepted_receipt_ref,
    } = outcome
    else {
        panic!("a first valid delivery must be accepted, not {outcome:?}");
    };
    assert_eq!(delivery_ref, manifest_ref("deliveryRef"));
    assert_eq!(package_ref, manifest_ref("packageRef"));
    assert_eq!(accepted_receipt_ref, manifest_ref("acceptedReceiptRef"));
    assert_ne!(
        accepted_receipt_ref, delivery_ref,
        "the accepted receipt must never be the same value as the delivery ref"
    );

    assert_stage_rows(&database, "final", "total").await;
    assert_downstream_tables_empty(&database).await;

    let package = sqlx::query(
        "SELECT accepted_outcome, package_hash, accepted_receipt_ref, accepted_delivery_id, \
                (SELECT delivery_ref FROM capture_ingress_delivery d WHERE d.id = p.accepted_delivery_id) AS delivery_ref \
         FROM capture_package p",
    )
    .fetch_one(database.pool())
    .await
    .expect("the accepted package must be readable");
    assert_eq!(package.get::<String, _>("accepted_outcome"), "accepted");
    assert_eq!(
        package.get::<String, _>("package_hash"),
        declared_package_hash(F01_PACKAGE)
    );
    assert_eq!(
        package.get::<Uuid, _>("accepted_receipt_ref"),
        accepted_receipt_ref
    );
    assert_eq!(package.get::<Uuid, _>("delivery_ref"), delivery_ref);

    let attempt = sqlx::query("SELECT terminal_reason FROM capture_attempt")
        .fetch_one(database.pool())
        .await
        .expect("the attempt must be readable");
    assert_eq!(
        attempt
            .get::<Option<String>, _>("terminal_reason")
            .as_deref(),
        Some("target_reached")
    );
}

#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
async fn f01_ingress_faults_before_commit_leave_no_half_written_rows() {
    for (index, fault) in [
        IngressFault::AfterRefsGenerated,
        IngressFault::AfterAcceptedDelivery,
        IngressFault::AfterPackage,
        IngressFault::DuringRecords,
        IngressFault::BeforeCoverage,
        IngressFault::DuringProcessingWork,
    ]
    .into_iter()
    .enumerate()
    {
        let database = fresh_f01_database(&format!("f01_fault_{index}")).await;
        let mut options = f01_fixed_options();
        options.inject_fault(fault);

        let error = ingest_capture_package_with(&database, F01_PACKAGE, &options)
            .await
            .expect_err("an injected transaction fault must not report acceptance");
        assert!(
            error.is_internal(),
            "a transaction fault is an internal failure, not a client rejection: {error:?}"
        );

        assert_stage_rows(&database, "fresh_seed", "total").await;
        assert_downstream_tables_empty(&database).await;
    }
}

#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
async fn f01_same_hash_replay_reuses_the_original_receipt_and_adds_only_a_delivery() {
    let database = fresh_f01_database("f01_replay").await;
    let accepted = ingest_capture_package_with(&database, F01_PACKAGE, &f01_fixed_options())
        .await
        .expect("the first valid delivery must be accepted");
    let IngressOutcome::Accepted {
        delivery_ref: original_delivery_ref,
        package_ref: original_package_ref,
        accepted_receipt_ref,
    } = accepted
    else {
        panic!("the first valid delivery must be accepted");
    };

    let mut replay_options = IngressOptions::default();
    replay_options.use_fixed_refs(vec![replay_delivery_ref()]);
    let replay = ingest_capture_package_with(&database, F01_PACKAGE, &replay_options)
        .await
        .expect("re-delivering the same hash must be a safe replay");

    let IngressOutcome::Replay {
        delivery_ref,
        package_ref,
        accepted_receipt_ref: replayed_receipt_ref,
        original_accepted_delivery_ref,
    } = replay
    else {
        panic!("the same package hash must replay, not {replay:?}");
    };
    assert_eq!(delivery_ref, replay_delivery_ref());
    assert_ne!(delivery_ref, original_delivery_ref);
    assert_eq!(package_ref, original_package_ref);
    assert_eq!(replayed_receipt_ref, accepted_receipt_ref);
    assert_eq!(original_accepted_delivery_ref, original_delivery_ref);

    assert_delivery_outcomes(&database, &["accepted", "replay"]).await;
    assert_business_rows_match_stage(&database, "final").await;
    assert_downstream_tables_empty(&database).await;
}

#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
async fn f01_different_hash_conflict_does_not_overwrite_the_accepted_package() {
    let database = fresh_f01_database("f01_conflict").await;
    let accepted = ingest_capture_package_with(&database, F01_PACKAGE, &f01_fixed_options())
        .await
        .expect("the first valid delivery must be accepted");
    let IngressOutcome::Accepted { package_ref, .. } = accepted else {
        panic!("the first valid delivery must be accepted");
    };

    let mut conflict_options = IngressOptions::default();
    conflict_options.use_fixed_refs(vec![conflict_delivery_ref()]);
    let conflict =
        ingest_capture_package_with(&database, F01_PACKAGE_OTHER_HASH, &conflict_options)
            .await
            .expect("a different hash for the same capture identity must be a closed conflict");

    let IngressOutcome::Conflict {
        delivery_ref,
        package_ref: existing_package_ref,
    } = conflict
    else {
        panic!("a different package hash must conflict, not {conflict:?}");
    };
    assert_eq!(delivery_ref, conflict_delivery_ref());
    assert_eq!(existing_package_ref, package_ref);

    assert_delivery_outcomes(&database, &["accepted", "conflict"]).await;
    assert_business_rows_match_stage(&database, "final").await;
    let stored_hash = sqlx::query("SELECT package_hash FROM capture_package")
        .fetch_one(database.pool())
        .await
        .expect("the original package must still be readable")
        .get::<String, _>("package_hash");
    assert_eq!(
        stored_hash,
        declared_package_hash(F01_PACKAGE),
        "a conflicting delivery must never overwrite the accepted package hash"
    );
}

#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
async fn f01_accepted_records_persist_the_whole_typed_envelope() {
    let database = fresh_f01_database("f01_envelope").await;
    ingest_capture_package_with(&database, F01_PACKAGE, &f01_fixed_options())
        .await
        .expect("a valid F01 package must be accepted");

    let rows = sqlx::query(
        "SELECT ordinal, record_kind, source_system, source_namespace, source_object_type, \
                source_external_id, source_channel, \
                to_char(observed_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS observed_at_utc, \
                observed_at_precision, observed_at_basis, target_external_id, record_hash, payload \
         FROM capture_record ORDER BY ordinal",
    )
    .fetch_all(database.pool())
    .await
    .expect("accepted records must expose their typed envelope");
    assert_eq!(rows.len(), 2);

    for (index, row) in rows.iter().enumerate() {
        let expected = &fixture_package(F01_PACKAGE)["records"][index];
        assert_eq!(
            row.get::<i32, _>("ordinal"),
            expected["ordinal"].as_i64().unwrap() as i32
        );
        assert_eq!(row.get::<String, _>("record_kind"), expected["recordKind"]);
        assert_eq!(
            row.get::<String, _>("source_system"),
            expected["source"]["system"]
        );
        assert_eq!(
            row.get::<String, _>("source_namespace"),
            expected["source"]["namespace"]
        );
        assert_eq!(
            row.get::<String, _>("source_object_type"),
            expected["source"]["objectType"]
        );
        assert_eq!(
            row.get::<Option<String>, _>("source_external_id")
                .as_deref(),
            expected["source"]["externalId"].as_str(),
            "the envelope's own external id must be stored verbatim"
        );
        assert_eq!(
            row.get::<String, _>("source_channel"),
            expected["source"]["channel"]
        );
        assert_eq!(
            row.get::<String, _>("observed_at_utc"),
            expected["observedAt"]["value"],
            "observedAt must be the producer's observation instant, not a receive or insert time"
        );
        assert_eq!(
            row.get::<String, _>("observed_at_precision"),
            expected["observedAt"]["precision"]
        );
        assert_eq!(
            row.get::<String, _>("observed_at_basis"),
            expected["observedAt"]["basis"]
        );
        assert_eq!(
            row.get::<String, _>("target_external_id"),
            expected["targetExternalId"]
        );
        assert_eq!(row.get::<String, _>("record_hash"), expected["recordHash"]);
        assert_eq!(row.get::<Value, _>("payload"), expected["payload"]);
    }
}

#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
async fn f01_explicit_null_source_external_id_is_accepted_and_stored_as_null() {
    let database = fresh_f01_database("f01_external_id_null").await;
    ingest_capture_package_with(&database, F01_SOURCE_EXTERNAL_ID_NULL, &f01_fixed_options())
        .await
        .expect("an explicit null source external id is a legal envelope value");

    let stored: Option<String> =
        sqlx::query("SELECT source_external_id FROM capture_record WHERE ordinal = 1")
            .fetch_one(database.pool())
            .await
            .expect("the record must be readable")
            .get("source_external_id");
    assert_eq!(
        stored, None,
        "an explicit null must stay null; it must not be back-filled from the target or payload"
    );

    let payload_states_an_id: Option<String> = sqlx::query(
        "SELECT payload->>'sourceExternalId' AS payload_source_external_id FROM capture_record WHERE ordinal = 1",
    )
    .fetch_one(database.pool())
    .await
    .expect("the record must be readable")
    .get("payload_source_external_id");
    assert!(
        payload_states_an_id.is_some(),
        "this fixture only proves the no-fallback rule while the payload still states an id"
    );
}

#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
async fn f01_missing_source_external_id_field_fails_the_contract_instead_of_reaching_the_database()
{
    let database = fresh_f01_database("f01_external_id_missing").await;
    let error = ingest_capture_package_with(
        &database,
        F01_SOURCE_EXTERNAL_ID_MISSING,
        &f01_fixed_options(),
    )
    .await
    .expect_err("a missing mandatory envelope field must not be accepted");

    assert!(matches!(
        error,
        IngressError::PreRouting(PreRoutingCode::PackageSchemaInvalid)
    ));
    assert_stage_rows(&database, "fresh_seed", "total").await;
}

#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
async fn f01_non_utc_observed_at_fails_the_contract_instead_of_being_normalized() {
    let database = fresh_f01_database("f01_observed_at_invalid").await;
    let error =
        ingest_capture_package_with(&database, F01_OBSERVED_AT_INVALID, &f01_fixed_options())
            .await
            .expect_err(
                "a non-UTC observedAt is a different lexical contract and must fail closed",
            );

    assert!(matches!(
        error,
        IngressError::PreRouting(PreRoutingCode::PackageSchemaInvalid)
    ));
    assert_stage_rows(&database, "fresh_seed", "total").await;
}

fn f01_fixed_options() -> IngressOptions {
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
    options
}

fn replay_delivery_ref() -> Uuid {
    Uuid::parse_str("00000000-0000-4000-8000-000000000301").expect("fixed proof ref must parse")
}

fn conflict_delivery_ref() -> Uuid {
    Uuid::parse_str("00000000-0000-4000-8000-000000000302").expect("fixed proof ref must parse")
}

async fn fresh_f01_database(schema: &str) -> Database {
    let proof_database_url = std::env::var("SCOPE_001_PROOF_DATABASE_URL")
        .expect("test-scope-001-postgres.sh must provide an isolated proof database URL");
    let migration = std::fs::read_to_string(workspace_path(CAPTURE_MIGRATION))
        .unwrap_or_else(|error| panic!("required F01 capture migration is unavailable: {error}"));
    let observation_migration = std::fs::read_to_string(workspace_path(OBSERVATION_MIGRATION))
        .unwrap_or_else(|error| {
            panic!("required F01 observation migration is unavailable: {error}")
        });
    let database = isolated_proof_schema(
        &proof_database_url,
        schema,
        &format!("{migration}\n{observation_migration}"),
    )
    .await
    .expect("both F01 migrations must apply in order to an isolated empty schema");
    seed_f01_work_attempt_and_targets(&database).await;
    database
}

async fn seed_f01_work_attempt_and_targets(database: &Database) {
    sqlx::query(
        "INSERT INTO capture_work_order (work_order_ref, contract_version, target_basis, target_unit, target_manifest_hash, known_target_count, quota_limit) \
         VALUES ($1, 'content-detail.synthetic.v1', 'known_set', 'content_detail', $2, 2, NULL)",
    )
    .bind(manifest_ref("workOrderRef"))
    .bind(fixture_target_manifest_hash())
    .execute(database.pool())
    .await
    .expect("the F01 fresh seed must create the frozen work order");

    sqlx::query(
        "INSERT INTO capture_attempt (attempt_ref, capture_identity, work_order_id, lease_epoch, authority_valid_until) \
         SELECT $1, $2, id, 1, scope_001_now() + interval '1 hour' FROM capture_work_order WHERE work_order_ref = $3",
    )
    .bind(manifest_ref("attemptRef"))
    .bind(manifest_ref("captureIdentityRef"))
    .bind(manifest_ref("workOrderRef"))
    .execute(database.pool())
    .await
    .expect("the F01 fresh seed must create the single authorized attempt");

    for (ordinal, external_id) in fixture_target_external_ids().into_iter().enumerate() {
        sqlx::query(
            "INSERT INTO capture_work_order_target (work_order_id, ordinal, external_id) \
             SELECT id, $1, $2 FROM capture_work_order WHERE work_order_ref = $3",
        )
        .bind(i32::try_from(ordinal + 1).expect("fixture ordinals stay small"))
        .bind(external_id)
        .bind(manifest_ref("workOrderRef"))
        .execute(database.pool())
        .await
        .expect("the F01 fresh seed must freeze both known targets");
    }
}

async fn assert_stage_rows(database: &Database, stage: &str, kind: &str) {
    let expected = manifest_stage(stage, kind);
    for (manifest_name, table) in FOUNDATION_TABLES {
        let expected_rows = expected[manifest_name]
            .as_i64()
            .unwrap_or_else(|| panic!("manifest stage {stage} must freeze {manifest_name}"));
        assert_eq!(
            count(database, table).await,
            expected_rows,
            "stage {stage} expects {expected_rows} rows in {table}"
        );
    }
}

async fn assert_business_rows_match_stage(database: &Database, stage: &str) {
    let expected = manifest_stage(stage, "total");
    for (manifest_name, table) in FOUNDATION_TABLES {
        if manifest_name == "delivery" {
            continue;
        }
        assert_eq!(
            count(database, table).await,
            expected[manifest_name].as_i64().expect("frozen row count"),
            "a replay or conflict must not change {table}"
        );
    }
}

/// The downstream tables exist once 0002 is applied, so acceptance must leave them empty rather
/// than merely absent. The manifest freezes every one of these as zero for accepted ingress.
async fn assert_downstream_tables_empty(database: &Database) {
    let expected = manifest_stage("accepted_ingress", "delta");
    for (manifest_name, table) in DOWNSTREAM_TABLES {
        assert_eq!(
            count(database, table).await,
            expected[manifest_name].as_i64().expect("frozen row count"),
            "accepted ingress must not create {manifest_name} ({table}); it belongs to record processing"
        );
    }
}

async fn assert_delivery_outcomes(database: &Database, expected: &[&str]) {
    let rows = sqlx::query("SELECT outcome FROM capture_ingress_delivery ORDER BY id")
        .fetch_all(database.pool())
        .await
        .expect("delivery audit rows must be readable");
    let outcomes: Vec<String> = rows
        .iter()
        .map(|row| {
            row.get::<Option<String>, _>("outcome")
                .expect("a public delivery always records its outcome")
        })
        .collect();
    assert_eq!(outcomes, expected);
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

fn declared_package_hash(source: &str) -> String {
    fixture_package(source)["packageHash"]
        .as_str()
        .expect("a capture fixture must declare its package hash")
        .to_owned()
}

fn fixture_target_manifest_hash() -> String {
    fixture_package(F01_PACKAGE)["target"]["targetManifestHash"]
        .as_str()
        .expect("the F01 fixture must declare its frozen target manifest hash")
        .to_owned()
}

fn fixture_target_external_ids() -> Vec<String> {
    fixture_package(F01_PACKAGE)["knownTargetResults"]
        .as_array()
        .expect("the F01 fixture must declare its known target results")
        .iter()
        .map(|result| {
            result["targetExternalId"]
                .as_str()
                .expect("each known target result names its target")
                .to_owned()
        })
        .collect()
}

fn workspace_path(relative_path: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative_path)
}
