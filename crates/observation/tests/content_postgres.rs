//! F01 record processing proof against a real, isolated PostgreSQL schema. Expected row counts
//! come from the hand-maintained fixture manifest; the implementation never supplies them.

mod support;

use linggan_observation::{BusinessOutcome, ProcessingOutcome, process_one_ready_record};
use serde_json::Value;
use sqlx::Row;
use support::*;
use uuid::Uuid;

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

    let bad = processed_with(&database, &attempt_ref_only()).await;
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
