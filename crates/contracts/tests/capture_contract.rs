use linggan_contracts::{ContractError, parse_capture_package};

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
