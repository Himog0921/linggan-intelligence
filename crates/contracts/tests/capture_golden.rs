use linggan_contracts::{ContractError, parse_capture_package};

const F01_RFC8785_JCS_GOLDEN: &str =
    include_str!("fixtures/capture-v1/f01-rfc8785-jcs-golden.json");
const F01_RFC8785_JCS_GOLDEN_STALE_PACKAGE_HASH: &str =
    include_str!("fixtures/capture-v1/f01-rfc8785-jcs-golden-stale-package-hash.json");
const F01_MANIFEST: &str = include_str!("fixtures/capture-v1/manifest.json");

/// The hand-maintained F01 manifest. Every test below reads it through this one accessor so a
/// renamed or removed fixture fails loudly instead of silently skipping assertions.
fn f01_manifest() -> serde_json::Value {
    let manifest: serde_json::Value =
        serde_json::from_str(F01_MANIFEST).expect("the hand-maintained F01 manifest must be JSON");
    manifest["fixtures"][0].clone()
}

#[test]
fn f01_manifest_scope_boundary_excludes_f02_to_f10_explicitly() {
    let manifest: serde_json::Value =
        serde_json::from_str(F01_MANIFEST).expect("the hand-maintained F01 manifest must be JSON");

    assert_eq!(manifest["includedFixtures"], serde_json::json!(["F01"]));
    assert_eq!(
        manifest["fixtures"]
            .as_array()
            .expect("the manifest must contain a fixtures array")
            .len(),
        1,
        "the manifest must contain exactly one fixture while only F01 is in scope"
    );
    assert_eq!(
        manifest["scopeBoundary"],
        serde_json::json!({
            "statement": "This hand-maintained manifest currently contains only F01 static contract evidence. It does not claim that F02-F10 fixtures or any runtime pipeline are implemented.",
            "notIncluded": ["F02", "F03", "F04", "F05", "F06", "F07", "F08", "F09", "F10"]
        }),
        "the manifest must explicitly exclude F02-F10 rather than merely omit them"
    );
}

#[test]
fn f01_manifest_keeps_its_fixture_identity_frozen() {
    let f01 = f01_manifest();

    assert_eq!(f01["fixtureId"], "F01");
    assert_eq!(f01["contractVersion"], "content-detail.synthetic.v1");
    assert_eq!(f01["synthetic"], true);
    assert_eq!(f01["baseCaptureFixturePath"], "f01-complete-known-set.json");
    assert_eq!(f01["acceptedTestSeedName"], "f01-fresh-seed");
    assert_eq!(f01["proofNow"], "2026-08-20T09:00:00Z");
}

#[test]
fn f01_manifest_keeps_its_layered_outcomes_separate() {
    let f01 = f01_manifest();

    assert_eq!(
        f01["expectedLayeredState"],
        serde_json::json!({
            "ingressDelivery": "accepted",
            "packageAcceptance": "accepted",
            "attemptTerminal": {"state": "terminal", "reason": "target_reached"},
            "captureSatisfaction": "satisfied",
            "recordProcessing": [
                {"recordRef": "recordA", "runtime": "finalized", "businessOutcome": "observation_recorded"},
                {"recordRef": "recordB", "runtime": "finalized", "businessOutcome": "observation_recorded"}
            ],
            "observationFormation": [
                {"recordRef": "recordA", "state": "formed"},
                {"recordRef": "recordB", "state": "formed"}
            ],
            "currentFieldResolution": [
                {"contentRef": "contentA", "title": "selected", "body": "selected"},
                {"contentRef": "contentB", "title": "selected", "body": "selected"}
            ]
        }),
        "the F01 manifest must retain the SCOPE's separate layered outcomes"
    );
}

#[test]
fn f01_manifest_keeps_each_staged_table_count_separate() {
    let f01 = f01_manifest();

    assert_eq!(
        f01["expectedTableRows"],
        serde_json::json!([
            {
                "stage": "fresh_seed",
                "kind": "total",
                "work": 1,
                "attempt": 1,
                "workTarget": 2,
                "delivery": 0,
                "package": 0,
                "record": 0,
                "targetResult": 0,
                "coverage": 0,
                "processingWork": 0,
                "processingAttempt": 0,
                "sourceIdentity": 0,
                "sourceContent": 0,
                "observation": 0,
                "currentRevision": 0,
                "fieldSource": 0
            },
            {
                "stage": "accepted_ingress",
                "kind": "delta",
                "work": 0,
                "attempt": 0,
                "workTarget": 0,
                "delivery": 1,
                "package": 1,
                "record": 2,
                "targetResult": 2,
                "coverage": 1,
                "processingWork": 2,
                "processingAttempt": 0,
                "sourceIdentity": 0,
                "sourceContent": 0,
                "observation": 0,
                "currentRevision": 0,
                "fieldSource": 0
            },
            {
                "stage": "all_processing",
                "kind": "delta",
                "work": 0,
                "attempt": 0,
                "workTarget": 0,
                "delivery": 0,
                "package": 0,
                "record": 0,
                "targetResult": 0,
                "coverage": 0,
                "processingWork": 0,
                "processingAttempt": 2,
                "sourceIdentity": 2,
                "sourceContent": 2,
                "observation": 2,
                "currentRevision": 2,
                "fieldSource": 4
            },
            {
                "stage": "final",
                "kind": "total",
                "work": 1,
                "attempt": 1,
                "workTarget": 2,
                "delivery": 1,
                "package": 1,
                "record": 2,
                "targetResult": 2,
                "coverage": 1,
                "processingWork": 2,
                "processingAttempt": 2,
                "sourceIdentity": 2,
                "sourceContent": 2,
                "observation": 2,
                "currentRevision": 2,
                "fieldSource": 4
            }
        ]),
        "the F01 manifest must retain fresh, ingress, processing, and final table facts separately"
    );
}

#[test]
fn f01_manifest_keeps_its_negative_and_proof_boundaries() {
    let f01 = f01_manifest();

    assert_eq!(
        f01["negativeProhibitions"],
        serde_json::json!([
            "no_total_completed_ok_or_success_state",
            "accepted_package_does_not_automatically_create_observations",
            "accepted_ingress_does_not_create_source_identity_content_observation_current_or_field_source",
            "f01_does_not_prove_a_database_api_worker_cli_or_real_producer"
        ]),
        "the F01 manifest must preserve its negative and proof boundaries"
    );

    assert_eq!(
        f01["proofBoundary"]["doesNotProve"],
        serde_json::json!([
            "PostgreSQL migrations, rows, constraints, or side effects",
            "API, worker, CLI, or processor behavior",
            "real producer, Raw Artifact, plugin, AI Agent, or production behavior",
            "F02-F10 fixture coverage or a complete Evidence pipeline"
        ]),
        "the F01 manifest must retain every explicit not-proven boundary"
    );
}

#[test]
fn f01_manifest_provides_the_complete_fixed_reference_dictionary() {
    let f01 = f01_manifest();

    assert_eq!(
        f01["fixedRefs"],
        serde_json::json!({
            "workOrderRef": "00000000-0000-4000-8000-000000000101",
            "attemptRef": "00000000-0000-4000-8000-000000000102",
            "captureIdentityRef": "00000000-0000-4000-8000-000000000103",
            "deliveryRef": "00000000-0000-4000-8000-000000000201",
            "acceptedReceiptRef": "00000000-0000-4000-8000-000000000202",
            "packageRef": "00000000-0000-4000-8000-000000000203",
            "recordA": "00000000-0000-4000-8000-000000000204",
            "recordB": "00000000-0000-4000-8000-000000000205",
            "processingWorkA": "00000000-0000-4000-8000-000000000206",
            "processingWorkB": "00000000-0000-4000-8000-000000000207",
            "processingAttemptA": "00000000-0000-4000-8000-000000000208",
            "processingAttemptB": "00000000-0000-4000-8000-000000000209",
            "sourceIdentityA": "00000000-0000-4000-8000-000000000210",
            "sourceIdentityB": "00000000-0000-4000-8000-000000000211",
            "contentA": "00000000-0000-4000-8000-000000000212",
            "contentB": "00000000-0000-4000-8000-000000000213",
            "observationA": "00000000-0000-4000-8000-000000000214",
            "observationB": "00000000-0000-4000-8000-000000000215",
            "currentRevisionA": "00000000-0000-4000-8000-000000000216",
            "currentRevisionB": "00000000-0000-4000-8000-000000000217",
            "fieldSourceTitleA": "00000000-0000-4000-8000-000000000218",
            "fieldSourceBodyA": "00000000-0000-4000-8000-000000000219",
            "fieldSourceTitleB": "00000000-0000-4000-8000-000000000220",
            "fieldSourceBodyB": "00000000-0000-4000-8000-000000000221"
        }),
        "the F01 manifest must provide the complete, fixed, typed reference dictionary"
    );
}

#[test]
fn rfc8785_golden_payload_is_accepted_through_the_public_capture_contract() {
    let fixture: serde_json::Value = serde_json::from_str(F01_RFC8785_JCS_GOLDEN)
        .expect("golden fixture must remain valid JSON");
    let package_input = serde_json::to_string(&fixture["capturePackage"])
        .expect("golden fixture must contain a capture package");

    let package = parse_capture_package(&package_input)
        .expect("the hand-maintained RFC 8785 golden package must be accepted");

    assert_eq!(package.contract_version(), "content-detail.synthetic.v1");
    assert_eq!(package.records().len(), 2);
}

#[test]
fn rfc8785_golden_stale_package_hash_is_rejected_through_the_public_capture_contract() {
    let fixture: serde_json::Value =
        serde_json::from_str(F01_RFC8785_JCS_GOLDEN_STALE_PACKAGE_HASH)
            .expect("stale golden fixture must remain valid JSON");
    let package_input = serde_json::to_string(&fixture["capturePackage"])
        .expect("stale golden fixture must contain a capture package");

    let error = parse_capture_package(&package_input)
        .expect_err("a Package with only a stale packageHash must not be accepted");

    assert!(
        matches!(error, ContractError::PackageHashInvalid),
        "a Package with only a stale packageHash must be rejected specifically as PackageHashInvalid"
    );
}
