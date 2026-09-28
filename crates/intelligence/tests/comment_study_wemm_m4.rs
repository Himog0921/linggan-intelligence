//! Opt-in M4 qualification. Run only through scripts/test-comment-study-wemm-m4.sh: it uses
//! the fixed local WeMM weights, synthetic sentences, and a disposable PostgreSQL schema.
#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;

use linggan_intelligence::{
    comment_study_embedding::{ProbeOutcome, probe_and_register_embedding_profile},
    pi_adapter::PiAdapter,
};
use serde_json::{Value, json};
use sqlx::Row;

const STUDY_SCHEMA_SQL: &str = include_str!("../../../database/bootstrap/comment-study-001.sql");

#[tokio::test]
#[ignore = "loads the pinned local WeMM model; isolated M4 qualification only"]
async fn rust_adapter_qualifies_the_local_m4_wemm_runtime_on_synthetic_text() {
    let database = fixture::proof_database("comment_study_m4_wemm_probe").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();

    let outcome = probe_and_register_embedding_profile(&database, &PiAdapter::configured())
        .await
        .expect("the Rust adapter must complete the local WeMM qualification probe");
    let (profile_ref, evidence) = match outcome {
        ProbeOutcome::Qualified {
            profile_ref,
            evidence,
        } => (profile_ref, evidence),
        ProbeOutcome::Refused {
            failing_check,
            evidence,
        } => panic!("M4 WeMM qualification refused at {failing_check}: {evidence}"),
    };

    assert_eq!(evidence["backend"], "mps", "the M4 probe must use MPS");
    assert_eq!(evidence["dimension"], 512);
    assert_eq!(
        evidence["encodeMeasurements"].as_array().map(Vec::len),
        Some(3)
    );
    for measurement in evidence["encodeMeasurements"].as_array().unwrap() {
        assert!(
            measurement["elapsedMs"]
                .as_i64()
                .is_some_and(|value| value > 0)
        );
        assert!(
            measurement["peakRssBytes"]
                .as_i64()
                .is_some_and(|value| value > 0)
        );
        assert!(
            measurement["mpsAllocatedBytes"]
                .as_u64()
                .is_some_and(|value| value > 0)
        );
        assert!(
            measurement["mpsDriverBytes"]
                .as_u64()
                .is_some_and(|value| value > 0)
        );
    }
    assert!(
        evidence["repeatMeasurement"]["elapsedMs"]
            .as_i64()
            .is_some_and(|value| value > 0)
    );
    assert_eq!(
        evidence["notCaptured"],
        json!(["systemMemoryPressure", "swapActivity"])
    );

    let profile: Value = sqlx::query(
        "SELECT jsonb_build_object( \
           'profileRef',profile_ref::text,'state',state,'modelRevision',model_revision, \
           'runtimeManifest',runtime_manifest,'qualification',qualification) AS profile \
         FROM linggan_comment_study_embedding_profile WHERE profile_ref=$1",
    )
    .bind(profile_ref)
    .fetch_one(database.pool())
    .await
    .unwrap()
    .get("profile");
    assert_eq!(profile["state"], "qualified");
    assert_eq!(
        profile["modelRevision"],
        "bbd6cd4bf52cfc6716f752a2df80b2706720bd95"
    );
    assert_eq!(profile["qualification"], evidence);
    println!(
        "M4_WEMM_RUST_ADAPTER_QUALIFICATION={}",
        serde_json::to_string(&profile).unwrap()
    );
}
