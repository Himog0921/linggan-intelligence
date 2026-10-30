//! Public producer parsers preserve execution budgets and comment-completion boundaries.
use linggan_contracts::{parse_producer_capture_package, parse_producer_task_spec};
use serde_json::{Value, json};

fn task(capability: &str) -> Value {
    json!({"contractVersion":"linggan.producer.task-spec.v2",
        "taskId":"11111111-1111-4111-8111-111111111111", "source":"scheduled",
        "platform":"xhs", "pageType":"note_detail", "target":{"contentExternalId":"note-1"},
        "capabilitiesRequested":[capability], "maximumQuota":30, "commentLimit":30,
        "acquireMedia":"not_requested", "riskPolicy":"server_authorized_leased",
        "stopConditions":["maximum_quota"]})
}

#[test]
fn incremental_search_bounds_stay_coupled_to_the_task() {
    let mut valid = task("discovery_search");
    valid["target"] = json!({"query":"synthetic"});
    valid["commentLimit"] = json!("not_requested");
    valid["incrementalSearchBudget"] =
        json!({"maxScrollRounds":3,"maxDurationSeconds":180,"candidateQuota":30});
    assert!(parse_producer_task_spec(&valid.to_string()).is_ok());
    for (field, value) in [
        ("maxScrollRounds", 0),
        ("maxScrollRounds", 4),
        ("maxDurationSeconds", 0),
        ("maxDurationSeconds", 181),
        ("candidateQuota", 31),
    ] {
        let mut invalid = valid.clone();
        invalid["incrementalSearchBudget"][field] = json!(value);
        assert!(
            parse_producer_task_spec(&invalid.to_string()).is_err(),
            "{field}={value}"
        );
    }
    valid["source"] = json!("manual");
    valid["riskPolicy"] = json!("local_trusted_user_initiated");
    assert!(parse_producer_task_spec(&valid.to_string()).is_err());
}

#[test]
fn incremental_comment_bounds_reject_duplicates_and_unqualified_reply_counts() {
    let mut valid = task("comments");
    valid["commentMode"] = json!("sample_30");
    valid["collectionRunRef"] = json!("22222222-2222-4222-8222-222222222222");
    valid["incrementalCommentBudget"] = json!({"knownCommentIds":["one"],
        "knownReplyCounts":{"one":2},"newUniqueLimit":30,"maxScrollRounds":50,"maxDurationSeconds":600});
    assert!(parse_producer_task_spec(&valid.to_string()).is_ok());
    for (field, value) in [
        ("knownCommentIds", json!(["one", "one"])),
        ("knownReplyCounts", json!({"one":3})),
        ("newUniqueLimit", json!(31)),
        ("maxScrollRounds", json!(0)),
        ("maxDurationSeconds", json!(601)),
    ] {
        let mut invalid = valid.clone();
        invalid["incrementalCommentBudget"][field] = value;
        assert!(
            parse_producer_task_spec(&invalid.to_string()).is_err(),
            "{field}"
        );
    }
    valid.as_object_mut().unwrap().remove("collectionRunRef");
    assert!(parse_producer_task_spec(&valid.to_string()).is_err());
}

fn capture(receipt: Value) -> Value {
    json!({"contractVersion":"linggan.producer.capture-package.v1",
        "packageRef":"11111111-1111-4111-8111-111111111111", "packageKind":"comments",
        "platform":"xhs", "observedAt":"2026-08-25T00:00:00Z", "capturedAt":"2026-08-25T00:00:01Z",
        "coverage":{"target":{"basis":"known_set","contentExternalId":"note-1","commentCollection":receipt},
        "layers":[{"capability":"comments","observed":30,"attempted":30,"acquired":30,
            "verified":0,"failed":0,"notAttempted":0,"unknown":0,"stoppedReason":"comment_area_end"}]},"records":[]})
}

#[test]
fn all_public_completion_requires_valid_complete_traversal_when_present() {
    let valid = json!({"version":1,"noteId":"note-1","scope":"all_public_comments",
        "pageCommentCount":null,"expectedCount":null,"uniqueCollectedCount":30,
        "state":"complete","analysisUsability":"usable","targetIdentity":"matched",
        "stopReason":"comment_area_end", "traversal":{"mainExhausted":true,"repliesExhausted":true,"identityReliable":true}});
    assert!(parse_producer_capture_package(&capture(valid.clone()).to_string()).is_ok());
    for field in ["mainExhausted", "repliesExhausted", "identityReliable"] {
        let mut invalid = valid.clone();
        invalid["traversal"][field] = json!(false);
        assert!(
            parse_producer_capture_package(&capture(invalid).to_string()).is_err(),
            "{field}"
        );
    }
    let mut invalid = valid.clone();
    invalid["traversal"]["extra"] = json!(true);
    assert!(parse_producer_capture_package(&capture(invalid).to_string()).is_err());
    let mut invalid = valid;
    invalid.as_object_mut().unwrap().remove("traversal");
    assert!(parse_producer_capture_package(&capture(invalid).to_string()).is_err());
}
