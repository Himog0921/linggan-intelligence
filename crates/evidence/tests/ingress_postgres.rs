use sqlx::{Connection, PgConnection, Row};

const CAPTURE_MIGRATION: &str = "database/migrations/0001_scope_001_capture_evidence.sql";

#[tokio::test]
#[ignore = "requires ./scripts/test-scope-001-postgres.sh and an isolated PostgreSQL proof database"]
async fn isolated_f01_database_accepts_its_minimum_ingress_shape_and_rejects_cross_parent_rows() {
    let database_url = std::env::var("SCOPE_001_PROOF_DATABASE_URL")
        .expect("test-scope-001-postgres.sh must provide an isolated proof database URL");
    let migration = std::fs::read_to_string(workspace_path(CAPTURE_MIGRATION))
        .unwrap_or_else(|error| panic!("required F01 capture migration is unavailable: {error}"));
    let mut connection = PgConnection::connect(&database_url)
        .await
        .expect("proof database must accept a PostgreSQL connection");

    sqlx::raw_sql(&migration)
        .execute(&mut connection)
        .await
        .expect("F01 capture migration must apply to an empty proof database");

    seed_f01_work_attempt_and_targets(&mut connection).await;

    assert_eq!(count(&mut connection, "capture_work_order").await, 1);
    assert_eq!(count(&mut connection, "capture_attempt").await, 1);
    assert_eq!(count(&mut connection, "capture_work_order_target").await, 2);
    for table in [
        "capture_ingress_delivery",
        "capture_package",
        "capture_record",
        "capture_package_target_result",
        "capture_package_coverage",
        "record_processing_work",
    ] {
        assert_eq!(count(&mut connection, table).await, 0, "fresh seed {table}");
    }

    insert_accepted_f01_ingress_shape(&mut connection).await;

    assert_eq!(count(&mut connection, "capture_ingress_delivery").await, 1);
    assert_eq!(count(&mut connection, "capture_package").await, 1);
    assert_eq!(count(&mut connection, "capture_record").await, 2);
    assert_eq!(
        count(&mut connection, "capture_package_target_result").await,
        2
    );
    assert_eq!(count(&mut connection, "capture_package_coverage").await, 1);
    assert_eq!(count(&mut connection, "record_processing_work").await, 2);

    let no_observation_or_current_tables = sqlx::query(
        "SELECT to_regclass('public.source_identity') IS NULL AS source_identity_absent, \
                to_regclass('public.source_content') IS NULL AS source_content_absent, \
                to_regclass('public.content_observation') IS NULL AS observation_absent, \
                to_regclass('public.content_current_revision') IS NULL AS current_absent",
    )
    .fetch_one(&mut connection)
    .await
    .expect("proof database must report the F01 foundation table boundary");
    assert!(no_observation_or_current_tables.get::<bool, _>("source_identity_absent"));
    assert!(no_observation_or_current_tables.get::<bool, _>("source_content_absent"));
    assert!(no_observation_or_current_tables.get::<bool, _>("observation_absent"));
    assert!(no_observation_or_current_tables.get::<bool, _>("current_absent"));

    sqlx::query(
        "INSERT INTO capture_work_order (work_order_ref, contract_version, target_basis, target_unit, target_manifest_hash, known_target_count, quota_limit) \
         VALUES ('00000000-0000-0000-0000-000000000099', 'content-detail.synthetic.v1', 'known_set', 'content_detail', 'sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff', 1, NULL)",
    )
    .execute(&mut connection)
    .await
    .expect("second work is only a constraint-negative fixture");
    sqlx::query(
        "INSERT INTO capture_attempt (attempt_ref, capture_identity, work_order_id, lease_epoch, authority_valid_until) \
         SELECT '00000000-0000-0000-0000-000000000199', '00000000-0000-0000-0000-000000000299', id, 1, scope_001_now() + interval '1 hour' \
         FROM capture_work_order WHERE work_order_ref = '00000000-0000-0000-0000-000000000099'",
    )
    .execute(&mut connection)
    .await
    .expect("second attempt is only a constraint-negative fixture");

    let cross_parent_delivery = sqlx::query(
        "INSERT INTO capture_ingress_delivery (audit_kind, delivery_ref, work_order_id, attempt_id, capture_identity, outcome, external_code) \
         SELECT 'public_delivery', '00000000-0000-0000-0000-000000000399', first_work.id, second_attempt.id, second_attempt.capture_identity, 'rejected', 'lease_epoch_mismatch' \
         FROM capture_work_order first_work \
         JOIN capture_attempt second_attempt ON second_attempt.attempt_ref = '00000000-0000-0000-0000-000000000199' \
         WHERE first_work.work_order_ref = '00000000-0000-0000-0000-000000000001'",
    )
    .execute(&mut connection)
    .await;
    assert!(
        cross_parent_delivery.is_err(),
        "cross-parent delivery must be rejected by the database"
    );
}

fn workspace_path(relative_path: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative_path)
}

async fn seed_f01_work_attempt_and_targets(connection: &mut PgConnection) {
    sqlx::raw_sql(
        "INSERT INTO capture_work_order (work_order_ref, contract_version, target_basis, target_unit, target_manifest_hash, known_target_count, quota_limit) \
         VALUES ('00000000-0000-0000-0000-000000000001', 'content-detail.synthetic.v1', 'known_set', 'content_detail', 'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', 2, NULL); \
         INSERT INTO capture_attempt (attempt_ref, capture_identity, work_order_id, lease_epoch, authority_valid_until) \
         SELECT '00000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000003', id, 1, scope_001_now() + interval '1 hour' \
         FROM capture_work_order WHERE work_order_ref = '00000000-0000-0000-0000-000000000001'; \
         INSERT INTO capture_work_order_target (work_order_id, ordinal, external_id) \
         SELECT id, 1, 'synthetic-note-001' FROM capture_work_order WHERE work_order_ref = '00000000-0000-0000-0000-000000000001'; \
         INSERT INTO capture_work_order_target (work_order_id, ordinal, external_id) \
         SELECT id, 2, 'synthetic-note-002' FROM capture_work_order WHERE work_order_ref = '00000000-0000-0000-0000-000000000001';",
    )
    .execute(connection)
    .await
    .expect("F01 fresh seed must create one work, one attempt, and two frozen targets");
}

async fn insert_accepted_f01_ingress_shape(connection: &mut PgConnection) {
    sqlx::raw_sql(
        "INSERT INTO capture_ingress_delivery (audit_kind, delivery_ref, work_order_id, attempt_id, capture_identity, outcome, external_code) \
         SELECT 'public_delivery', '00000000-0000-0000-0000-000000000004', work.id, attempt.id, attempt.capture_identity, 'accepted', NULL \
         FROM capture_work_order work JOIN capture_attempt attempt ON attempt.work_order_id = work.id \
         WHERE work.work_order_ref = '00000000-0000-0000-0000-000000000001'; \
         INSERT INTO capture_package (package_ref, work_order_id, attempt_id, capture_identity, package_hash, accepted_delivery_id, accepted_receipt_ref) \
         SELECT '00000000-0000-0000-0000-000000000005', delivery.work_order_id, delivery.attempt_id, delivery.capture_identity, 'sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', delivery.id, '00000000-0000-0000-0000-000000000006' \
         FROM capture_ingress_delivery delivery WHERE delivery.delivery_ref = '00000000-0000-0000-0000-000000000004'; \
         INSERT INTO capture_record (record_ref, package_id, ordinal, target_external_id, record_hash, payload) \
         SELECT '00000000-0000-0000-0000-000000000007', id, 1, 'synthetic-note-001', 'sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc', '{\"synthetic\":true}'::jsonb FROM capture_package; \
         INSERT INTO capture_record (record_ref, package_id, ordinal, target_external_id, record_hash, payload) \
         SELECT '00000000-0000-0000-0000-000000000008', id, 2, 'synthetic-note-002', 'sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd', '{\"synthetic\":true}'::jsonb FROM capture_package; \
         INSERT INTO capture_package_target_result (package_id, work_order_id, target_id, outcome, record_id, reason) \
         SELECT package.id, package.work_order_id, target.id, 'emitted', record.id, NULL \
         FROM capture_package package JOIN capture_work_order_target target ON target.work_order_id = package.work_order_id \
         JOIN capture_record record ON record.package_id = package.id AND record.ordinal = target.ordinal; \
         INSERT INTO capture_package_coverage (package_id, unit, attempted, emitted, failed, known_not_attempted, remaining_scope) \
         SELECT id, 'content_detail', 2, 2, 0, 0, 'known_members' FROM capture_package; \
         INSERT INTO record_processing_work (processing_work_ref, capture_record_id, processor_version) \
         SELECT CASE ordinal WHEN 1 THEN '00000000-0000-0000-0000-000000000009'::uuid ELSE '00000000-0000-0000-0000-000000000010'::uuid END, id, 'content-detail-processor-v1' \
         FROM capture_record;",
    )
    .execute(connection)
    .await
    .expect("minimum accepted ingress facts must satisfy the F01 foundation constraints");
}

async fn count(connection: &mut PgConnection, table: &str) -> i64 {
    let query = format!("SELECT count(*) AS count FROM {table}");
    sqlx::query(&query)
        .fetch_one(connection)
        .await
        .expect("proof table must be queryable")
        .get("count")
}
