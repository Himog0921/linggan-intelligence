use linggan_contracts::{CapturePackage, ContractError, parse_capture_package};

const F01_COMPLETE_KNOWN_SET: &str =
    include_str!("fixtures/capture-v1/f01-complete-known-set.json");
const F01_COMPLETE_KNOWN_SET_WITH_PAYLOAD_EXTENSION: &str =
    include_str!("fixtures/capture-v1/f01-complete-known-set-payload-extension.json");
const F01_PACKAGE_VALID_RECORD_HASH_INVALID: &str =
    include_str!("fixtures/capture-v1/f01-package-valid-record-hash-invalid.json");
const F01_DUPLICATE_KEY_SCHEMA_INVALID: &str =
    include_str!("fixtures/capture-v1/f01-duplicate-key-schema-invalid.json");
const F01_UNSAFE_INTEGER_SCHEMA_INVALID: &str =
    include_str!("fixtures/capture-v1/f01-unsafe-integer-schema-invalid.json");
const F01_UNPAIRED_SURROGATE_SCHEMA_INVALID: &str =
    include_str!("fixtures/capture-v1/f01-unpaired-surrogate-schema-invalid.json");
const F01_SOURCE_EXTERNAL_ID_NULL: &str =
    include_str!("fixtures/capture-v1/f01-source-external-id-null.json");
const F01_SOURCE_EXTERNAL_ID_MISSING: &str =
    include_str!("fixtures/capture-v1/f01-source-external-id-missing.json");
const F01_OBSERVED_AT_INVALID: &str =
    include_str!("fixtures/capture-v1/f01-observed-at-invalid.json");

#[test]
fn f01_complete_known_set_is_read_through_the_public_contract() {
    let package = parse_capture_package(F01_COMPLETE_KNOWN_SET).expect("F01 must be valid");

    assert_eq!(
        (
            package.contract_version(),
            package.target().known_target_count(),
            package.records().len(),
            (
                package.coverage().attempted(),
                package.coverage().emitted(),
                package.coverage().failed(),
                package.coverage().known_not_attempted(),
            ),
            package.terminal().reason().as_str(),
        ),
        (
            "content-detail.synthetic.v1",
            2,
            2,
            (2, 2, 0, Some(0)),
            "target_reached",
        )
    );
}

#[test]
fn record_envelope_rejects_unknown_fields_while_payload_remains_open_for_processor() {
    let base: serde_json::Value =
        serde_json::from_str(F01_COMPLETE_KNOWN_SET).expect("F01 fixture must be JSON");

    let mut envelope_extension = base.clone();
    envelope_extension["records"][0]["unexpectedEnvelopeField"] = serde_json::json!(true);
    let envelope_extension =
        serde_json::to_string(&envelope_extension).expect("mutation must remain JSON");

    let envelope_error = parse_capture_package(&envelope_extension)
        .expect_err("an undeclared Record envelope field must fail ingress parsing");
    assert!(
        matches!(envelope_error, ContractError::PackageSchemaInvalid(_)),
        "a valid-JSON Record envelope extension must be a package schema error"
    );

    assert!(
        parse_capture_package(F01_COMPLETE_KNOWN_SET_WITH_PAYLOAD_EXTENSION).is_ok(),
        "a hand-maintained payload extension fixture with a valid Package hash must remain accepted"
    );
}

#[test]
fn package_hash_rejects_a_schema_valid_stale_package() {
    let mut stale_package: serde_json::Value =
        serde_json::from_str(F01_COMPLETE_KNOWN_SET).expect("F01 fixture must be JSON");
    stale_package["terminal"]["reason"] = serde_json::json!("risk_control");
    let stale_package = serde_json::to_string(&stale_package).expect("mutation must remain JSON");

    let error = parse_capture_package(&stale_package).expect_err(
        "a schema-valid Package mutation must not be accepted with its stale packageHash",
    );
    assert!(
        matches!(error, ContractError::PackageHashInvalid),
        "a schema-valid Package mutation must be rejected specifically as PackageHashInvalid"
    );
}

#[test]
fn record_hash_rejects_a_package_that_has_already_passed_package_hash_validation() {
    let error = parse_capture_package(F01_PACKAGE_VALID_RECORD_HASH_INVALID).expect_err(
        "a Package with a correct packageHash must reject a Record whose literal recordHash is stale",
    );
    assert!(
        matches!(error, ContractError::RecordHashInvalid),
        "a Package that passes Package hash validation must reject a stale Record hash specifically"
    );
}

#[test]
fn duplicate_key_precedes_package_schema_validation() {
    let error = parse_capture_package(F01_DUPLICATE_KEY_SCHEMA_INVALID).expect_err(
        "a syntactically valid JSON input with a duplicate key must fail before envelope schema validation",
    );
    assert!(
        matches!(error, ContractError::CanonicalizationInvalid(_)),
        "a duplicate JSON key must be rejected specifically as CanonicalizationInvalid"
    );
}

#[test]
fn unsafe_integer_precedes_package_schema_validation() {
    let error = parse_capture_package(F01_UNSAFE_INTEGER_SCHEMA_INVALID).expect_err(
        "a syntax-valid JSON input with an unsafe integer must fail before envelope schema validation",
    );
    assert!(
        matches!(error, ContractError::CanonicalizationInvalid(_)),
        "an unsafe JSON integer must be rejected specifically as CanonicalizationInvalid"
    );
}

#[test]
fn unpaired_surrogate_precedes_package_schema_validation() {
    let error = parse_capture_package(F01_UNPAIRED_SURROGATE_SCHEMA_INVALID).expect_err(
        "a syntax-valid JSON input with an unpaired surrogate must fail before envelope schema validation",
    );
    assert!(
        matches!(error, ContractError::CanonicalizationInvalid(_)),
        "an unpaired surrogate must be rejected specifically as CanonicalizationInvalid; got {error:?}"
    );
}

#[test]
fn lone_low_surrogate_precedes_package_schema_validation() {
    let error = parse_capture_package(r#"{"note":"\uDC00","records":"not-an-array"}"#).expect_err(
        "a syntax-valid JSON input with a lone low surrogate must fail before envelope schema validation",
    );
    assert!(
        matches!(error, ContractError::CanonicalizationInvalid(_)),
        "a lone low surrogate must be rejected specifically as CanonicalizationInvalid"
    );
}

#[test]
fn malformed_json_with_an_unpaired_surrogate_remains_invalid_json() {
    let error = parse_capture_package(r#"{"note":"\uD800",}"#)
        .expect_err("a trailing object comma must remain malformed JSON");
    assert!(
        matches!(error, ContractError::InvalidJson(_)),
        "a malformed JSON input must not be reclassified as a canonicalization error"
    );
}

#[test]
fn legal_surrogate_pair_and_escaped_surrogate_spelling_remain_schema_errors() {
    for input in [
        r#"{"note":"\uD83D\uDE00","records":"not-an-array"}"#,
        r#"{"note":"\\uD800","records":"not-an-array"}"#,
    ] {
        let error = parse_capture_package(input)
            .expect_err("each syntax-valid input must fail the incomplete Package envelope");
        assert!(
            matches!(error, ContractError::PackageSchemaInvalid(_)),
            "{input} must not be misclassified as an unpaired-surrogate canonicalization error"
        );
    }
}

#[test]
fn negative_unsafe_integer_precedes_package_schema_validation() {
    let input = r#"{"leaseEpoch":-9007199254740992,"records":"not-an-array"}"#;
    let error = parse_capture_package(input).expect_err(
        "a syntax-valid JSON input with a negative unsafe integer must fail before envelope schema validation",
    );
    assert!(
        matches!(error, ContractError::CanonicalizationInvalid(_)),
        "a negative unsafe JSON integer must be rejected specifically as CanonicalizationInvalid"
    );
}

#[test]
fn safe_integers_and_legal_non_integer_numbers_remain_schema_errors() {
    for literal in [
        "9007199254740991",
        "-9007199254740991",
        "1e20",
        "1.0",
        "-1.5",
    ] {
        let input = format!(r#"{{"leaseEpoch":{literal},"records":"not-an-array"}}"#);
        let error = parse_capture_package(&input)
            .expect_err("each syntax-valid input must fail the incomplete Package envelope");
        assert!(
            matches!(error, ContractError::PackageSchemaInvalid(_)),
            "{literal} must not be misclassified as an unsafe-integer canonicalization error"
        );
    }
}

#[test]
fn record_envelope_exposes_every_verified_field_through_the_public_contract() {
    let package = parse_capture_package(F01_COMPLETE_KNOWN_SET).expect("F01 must be valid");
    let record = &package.records()[0];

    assert_eq!(
        (
            record.ordinal(),
            record.record_kind(),
            record.target_external_id(),
            record.source_system(),
            record.source_namespace(),
            record.source_object_type(),
            record.source_external_id(),
            record.source_channel(),
            record.observed_at_value(),
            record.observed_at_precision(),
            record.observed_at_basis(),
        ),
        (
            1,
            "content_detail",
            "synthetic-note-a",
            "synthetic",
            "scope-001",
            "content",
            Some("synthetic-note-a"),
            "synthetic_page",
            "2026-08-20T08:00:00Z",
            "exact",
            "fixture",
        )
    );
}

#[test]
fn explicit_null_source_external_id_is_a_legal_envelope_value() {
    let package =
        parse_capture_package(F01_SOURCE_EXTERNAL_ID_NULL).expect("an explicit null is legal");
    let record = &package.records()[0];

    assert_eq!(record.source_external_id(), None);
    assert_eq!(
        record.target_external_id(),
        "synthetic-note-a",
        "the target statement stays readable but never substitutes for the source statement"
    );
    assert_eq!(
        record.payload()["sourceExternalId"].as_str(),
        Some("synthetic-note-a"),
        "the payload still states an id, so any fallback would be visible downstream"
    );
}

#[test]
fn missing_source_external_id_field_is_a_package_schema_error_not_a_null() {
    let error = parse_capture_package(F01_SOURCE_EXTERNAL_ID_MISSING)
        .expect_err("a mandatory envelope field must not be optional");

    assert!(
        matches!(error, ContractError::PackageSchemaInvalid(_)),
        "an absent mandatory field must fail the schema rather than become an explicit null: {error:?}"
    );
}

#[test]
fn observed_at_outside_the_frozen_utc_form_fails_closed() {
    let error = parse_capture_package(F01_OBSERVED_AT_INVALID)
        .expect_err("a non-UTC RFC 3339 instant is a different lexical contract");

    assert!(
        matches!(error, ContractError::PackageSchemaInvalid(_)),
        "an offset instant must fail closed rather than be normalized to UTC: {error:?}"
    );
}

#[test]
fn observed_at_and_source_fixed_values_are_closed_enumerations() {
    let base: serde_json::Value =
        serde_json::from_str(F01_COMPLETE_KNOWN_SET).expect("F01 fixture must be JSON");

    for (path, replacement) in [
        (["observedAt", "precision"], "approximate"),
        (["observedAt", "basis"], "inferred"),
        (["source", "system"], "xiaohongshu"),
        (["source", "namespace"], "scope-002"),
        (["source", "objectType"], "comment"),
        (["source", "channel"], "search_page"),
    ] {
        let mut mutated = base.clone();
        mutated["records"][0][path[0]][path[1]] = serde_json::json!(replacement);
        let mutated = serde_json::to_string(&mutated).expect("mutation must serialize");

        let error = parse_capture_package(&mutated)
            .unwrap_err_or_else_message(&format!("{}.{} = {replacement}", path[0], path[1]));
        assert!(
            matches!(error, ContractError::PackageSchemaInvalid(_)),
            "{}.{} = {replacement} must fail the schema, not be accepted as a free string",
            path[0],
            path[1]
        );
    }
}

/// Small helper so a failing mutation names itself in the panic message.
trait UnwrapErrOrElseMessage<T> {
    fn unwrap_err_or_else_message(self, context: &str) -> ContractError;
}

impl UnwrapErrOrElseMessage<CapturePackage> for Result<CapturePackage, ContractError> {
    fn unwrap_err_or_else_message(self, context: &str) -> ContractError {
        match self {
            Ok(_) => panic!("{context} must not be accepted"),
            Err(error) => error,
        }
    }
}
