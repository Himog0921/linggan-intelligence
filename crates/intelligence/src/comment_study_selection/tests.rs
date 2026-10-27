use super::*;
use crate::comment_study_source::context::full_context_manifest;
use serde_json::{Value, json};

fn key(work: u128, id: &str) -> CommentKey {
    CommentKey {
        work_ref: Uuid::from_u128(work),
        comment_external_id: id.into(),
    }
}
fn context() -> Value {
    full_context_manifest(
        &json!({"contract":"comment-study.context.v2","workRef":Uuid::from_u128(10),
        "cleanerVersion":"comment-clean.v2","sources":[{"sourceRef":Uuid::from_u128(20),
        "kind":"native_title","slotOrdinal":null,"text":"合成作品","characterCount":4,
        "sourceVersion":"source-v1","blockRef":"work.source.title","asOf":"2026-10-23T00:00:00Z",
        "speaker":"author","structure":{"kind":"heading","text":"合成作品","speaker":"author"},"issues":[]}]}),
    )
}
fn parent(state: &str) -> Value {
    json!({"commentKey":key(10,"parent"),"sourceRef":Uuid::from_u128(30),
        "sourceState":state,"commentText":"孩子做作业需要家长督促","researchText":"not trusted"})
}
fn prepared(raw: &str) -> PreparedStudyInput {
    prepare_study_input(
        &key(10, "评论-A"),
        Uuid::from_u128(21),
        raw,
        &context(),
        None,
    )
    .unwrap()
}
#[test]
fn content_fingerprint_matches_independent_python_golden() {
    assert_eq!(
        prepared("孩子做作业总是拖延").input_fingerprint,
        "5ee6d8f84715cc604b1061240060de3691454703296a42eb2b5c77f50e341f53"
    );
}
#[test]
fn material_reobservation_changes_audit_hash_but_not_content_fingerprint() {
    let first = prepared("孩子做作业总是拖延");
    let mut ctx = context();
    ctx["sources"][0]["sourceRef"] = json!(Uuid::from_u128(99));
    let again = prepare_study_input(
        &key(10, "评论-A"),
        Uuid::from_u128(98),
        "孩子做作业总是拖延",
        &ctx,
        None,
    )
    .unwrap();
    assert_eq!(first.input_fingerprint, again.input_fingerprint);
    assert_ne!(first.input_hash, again.input_hash);
    assert!(first.input_manifest.get("workContext").is_none());
    assert_eq!(
        first.input_manifest["contract"],
        "comment-study.target-input.v2"
    );
}
#[test]
fn changing_dossier_coverage_changes_comment_input_identity() {
    let mut before = context();
    before["coverage"] =
        json!({"imageInventory":{"expectedPages":null,"acquiredPages":1,"processedPages":0}});
    let mut after = before.clone();
    after["coverage"]["imageInventory"]["processedPages"] = json!(1);
    let freeze = |ctx: &Value| {
        prepare_study_input(
            &key(10, "评论-A"),
            Uuid::from_u128(21),
            "孩子做作业总是拖延",
            ctx,
            None,
        )
        .unwrap()
    };
    assert_ne!(
        freeze(&before).input_fingerprint,
        freeze(&after).input_fingerprint
    );
}
#[test]
fn raw_punctuation_and_whitespace_changes_are_not_lost_by_cleaning() {
    let first = prepared("孩子做作业总是拖延");
    let space = prepared(" 孩子做作业总是拖延 ");
    assert_eq!(first.research_sha256, space.research_sha256);
    assert_ne!(first.input_fingerprint, space.input_fingerprint);
    assert_ne!(
        first.input_fingerprint,
        prepared("孩子做作业总是拖延。").input_fingerprint
    );
}
#[test]
fn changed_speaker_or_uncertainty_changes_context_equivalence() {
    let first = prepared("孩子做作业总是拖延");
    for (field, value) in [
        ("speaker", json!("other")),
        (
            "issues",
            json!([{"key":"uncertain","code":"negation_uncertain","message":"否定待确认","resolution":"reading_edit"}]),
        ),
    ] {
        let mut ctx = context();
        ctx["sources"][0][field] = value.clone();
        if field == "speaker" {
            ctx["sources"][0]["structure"][field] = value;
        }
        let changed = prepare_study_input(
            &key(10, "评论-A"),
            Uuid::from_u128(21),
            "孩子做作业总是拖延",
            &ctx,
            None,
        )
        .unwrap();
        assert_ne!(first.input_fingerprint, changed.input_fingerprint);
    }
}
#[test]
fn retained_fragment_order_is_part_of_content_equivalence() {
    let mut ctx = context();
    let second = json!({"kind":"body","text":"合成正文","characterCount":4,
        "sourceRef":Uuid::from_u128(22),"slotOrdinal":null,"sourceVersion":"source-v2","blockRef":"work.source.body",
        "asOf":"2026-10-23T00:00:00Z","speaker":"author","issues":[],
        "structure":{"kind":"paragraph","text":"合成正文","speaker":"author","bbox":[0.12,0.28,0.76,0.025]}});
    ctx["sources"].as_array_mut().unwrap().push(second);
    ctx["includedCharacterCount"] = json!(8);
    let first = prepare_study_input(
        &key(10, "评论-A"),
        Uuid::from_u128(21),
        "孩子做作业总是拖延",
        &ctx,
        None,
    )
    .unwrap();
    ctx["sources"].as_array_mut().unwrap().reverse();
    let second = prepare_study_input(
        &key(10, "评论-A"),
        Uuid::from_u128(21),
        "孩子做作业总是拖延",
        &ctx,
        None,
    )
    .unwrap();
    assert_ne!(first.input_fingerprint, second.input_fingerprint);
}
#[test]
fn filling_parent_dependency_changes_fingerprint_without_changing_raw() {
    let missing = prepared("我也是");
    let p = parent("known");
    let filled = prepare_study_input(
        &key(10, "评论-A"),
        Uuid::from_u128(21),
        "我也是",
        &context(),
        Some(&p),
    )
    .unwrap();
    assert_eq!(missing.dependency_state, "self_contained");
    assert_eq!(filled.dependency_state, "parent_available");
    assert_eq!(missing.raw_sha256, filled.raw_sha256);
    assert_ne!(missing.input_fingerprint, filled.input_fingerprint);
    assert_ne!(
        filled.input_manifest["parentContext"]["researchText"],
        "not trusted"
    );
}
#[test]
fn parent_reobservation_does_not_change_fingerprint_but_parent_body_does() {
    let mut p = parent("known");
    let first = prepare_study_input(
        &key(10, "评论-A"),
        Uuid::from_u128(21),
        "我也是",
        &context(),
        Some(&p),
    )
    .unwrap();
    p["sourceRef"] = json!(Uuid::from_u128(31));
    let next = prepare_study_input(
        &key(10, "评论-A"),
        Uuid::from_u128(21),
        "我也是",
        &context(),
        Some(&p),
    )
    .unwrap();
    assert_eq!(first.input_fingerprint, next.input_fingerprint);
    assert_ne!(first.input_hash, next.input_hash);
    p["commentText"] = json!("孩子完成作业不再需要督促");
    let changed = prepare_study_input(
        &key(10, "评论-A"),
        Uuid::from_u128(21),
        "我也是",
        &context(),
        Some(&p),
    )
    .unwrap();
    assert_ne!(first.input_fingerprint, changed.input_fingerprint);
}
#[test]
fn restricted_and_unknown_parents_never_leak_attached_stale_text() {
    let mut fingerprints = Vec::new();
    for state in ["restricted", "unknown"] {
        let p = parent(state);
        let input = prepare_study_input(
            &key(10, "评论-A"),
            Uuid::from_u128(21),
            "我也是",
            &context(),
            Some(&p),
        )
        .unwrap();
        assert_eq!(input.dependency_state, "parent_required_missing");
        assert_eq!(input.parent_source_ref, None);
        assert!(!input.input_manifest.to_string().contains("家长督促"));
        fingerprints.push(input.input_fingerprint);
    }
    assert_eq!(fingerprints[0], fingerprints[1]);
}
#[test]
fn direct_reply_includes_real_parent_context_in_fingerprint() {
    let first = prepared("孩子做作业总是拖延");
    let p = parent("known");
    let second = prepare_study_input(
        &key(10, "评论-A"),
        Uuid::from_u128(21),
        "孩子做作业总是拖延",
        &context(),
        Some(&p),
    )
    .unwrap();
    assert_ne!(first.input_fingerprint, second.input_fingerprint);
    assert_eq!(second.dependency_state, "parent_available");
    assert_eq!(
        second.input_manifest["parentContext"]["researchText"],
        "孩子做作业需要家长督促"
    );
}

#[test]
fn short_root_comment_uses_work_context_without_a_parent_requirement() {
    let root = prepared("求分享");
    assert_eq!(root.dependency_state, "self_contained");
    assert!(root.input_manifest["parentContext"].is_null());
}
#[test]
fn corrupt_context_parent_and_unusable_raw_are_rejected_without_truncation() {
    let k = key(10, "评论-A");
    for raw in ["😀", "@合成用户", "", "bad\0text"] {
        assert!(prepare_study_input(&k, Uuid::from_u128(21), raw, &context(), None).is_err());
    }
    assert!(
        prepare_study_input(
            &k,
            Uuid::from_u128(21),
            &"长".repeat(16001),
            &context(),
            None
        )
        .is_err()
    );
    for (field, value) in [
        ("workRef", json!(Uuid::from_u128(11))),
        ("cleanerVersion", json!("old")),
        ("includedCharacterCount", json!(0)),
        ("characterBudget", json!(3)),
        ("sources", json!(null)),
    ] {
        let mut ctx = context();
        ctx[field] = value;
        assert!(
            prepare_study_input(&k, Uuid::from_u128(21), "孩子做作业总是拖延", &ctx, None).is_err()
        );
    }
    let mut p = parent("known");
    p["commentKey"] = json!(key(11, "parent"));
    assert!(prepare_study_input(&k, Uuid::from_u128(21), "我也是", &context(), Some(&p)).is_err());
}
