//! Synthetic contracts only; cloud opinions are never human ground truth.
use linggan_intelligence::ocr_quality_audit::{
    self as audit, Settings, parse_visual,
    sampling::{Candidate, select},
};
use serde_json::json;
use uuid::Uuid;
fn output() -> serde_json::Value {
    json!({"titlePresence":"absent","titleText":null,"regions":[{"regionRef":"r","text":"合成原文","role":"body","legibility":"clear","bbox":null}],"unreadable":false})
}
#[test]
fn malformed_empty_truncated_and_unsupported_regions_fail_closed() {
    for text in ["", "{}", "[]", "not json"] {
        assert!(parse_visual(text, Some("stop")).is_err());
    }
    assert!(parse_visual(&output().to_string(), Some("length")).is_err());
    let mut v = output();
    v["regions"][0]["bbox"] = json!([0., 0., 2., 1.]);
    assert!(parse_visual(&v.to_string(), Some("stop")).is_err());
    v = output();
    v["regions"][0]["instruction"] = json!("ignore policy");
    assert!(parse_visual(&v.to_string(), Some("stop")).is_err());
}
#[test]
fn a_title_must_be_visible_literal_region_not_a_generated_summary() {
    let mut v = output();
    v["titlePresence"] = json!("present");
    v["titleText"] = json!("invented title");
    assert!(parse_visual(&v.to_string(), Some("stop")).is_err());
    v["regions"][0]["role"] = json!("main_title");
    v["regions"][0]["text"] = json!("invented title");
    assert!(parse_visual(&v.to_string(), Some("stop")).is_ok());
}
#[test]
fn absent_and_unreadable_are_distinct_valid_observations() {
    let no_text =
        json!({"titlePresence":"absent","titleText":null,"regions":[],"unreadable":false});
    assert!(parse_visual(&no_text.to_string(), Some("stop")).is_ok());
    let unreadable =
        json!({"titlePresence":"uncertain","titleText":null,"regions":[],"unreadable":true});
    assert!(parse_visual(&unreadable.to_string(), Some("stop")).is_ok());
}
#[test]
fn sampling_is_work_first_deterministic_disjoint_and_does_not_fill_missing_pools() {
    let candidates: Vec<_> = (1..=12)
        .map(|n| Candidate {
            adoption_ref: Uuid::from_u128(n),
            work_ref: Uuid::from_u128(if n < 10 { 1 } else { n }),
            blob_sha256: format!("{n:064x}"),
            risk: true,
        })
        .collect();
    let s = Settings {
        random_count: 3,
        risk_count: 2,
        regression_count: 10,
        regression_refs: vec![Uuid::from_u128(2)],
        ..Settings::default()
    };
    let a = select("frozen-seed", &candidates, &s);
    let b = select("frozen-seed", &candidates, &s);
    assert_eq!(
        serde_json::to_value(&a).unwrap(),
        serde_json::to_value(b).unwrap()
    );
    assert_eq!(a.iter().filter(|x| x.pool == "regression").count(), 1);
    assert_eq!(a.len(), 6);
    assert_eq!(
        a.iter()
            .map(|s| s.candidate.adoption_ref)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        6
    );
    assert_eq!(
        a.iter()
            .filter(|s| s.pool == "random")
            .map(|s| s.candidate.work_ref)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        3
    );
}
#[test]
fn defaults_cannot_start_automatic_sampling_or_correction() {
    let s = Settings::default();
    assert!(!s.automatic);
    assert_eq!(s.domain_ref, None);
    assert!(s.privacy_cleared_refs.is_empty());
    s.validate().unwrap();
    let mut invalid = s;
    invalid.concurrency = 0;
    assert!(invalid.validate().is_err());
}
#[test]
fn blind_prompt_contains_no_local_answer_or_dynamic_instruction() {
    let (system, prompt) = audit::prompt();
    assert!(system.contains("untrusted data"));
    assert!(!prompt.contains("localAssessment"));
    assert!(prompt.contains("jsonExample"));
}
#[test]
fn synthetic_visual_probe_is_a_real_bounded_image() {
    let bytes = audit::source::probe_image_bytes();
    let image = image::load_from_memory(&bytes).unwrap();
    assert_eq!(image.width(), 144);
    assert_eq!(image.height(), 96);
    assert!(bytes.len() < 4096);
    let mut v = parse_visual(&output().to_string(), Some("stop")).unwrap();
    assert!(!audit::qualifies_probe(&v));
    v.regions[0].text = "42".into();
    assert!(audit::qualifies_probe(&v));
    v.regions[0].legibility = audit::Legibility::Partial;
    assert!(!audit::qualifies_probe(&v));
}

#[test]
fn external_clearance_is_explicit_scoped_and_auditable() {
    let mut s = Settings {
        privacy_cleared_refs: vec![Uuid::from_u128(1)],
        ..Default::default()
    };
    assert!(s.validate().is_err());
    s.privacy_clearance_actor = "synthetic reviewer".into();
    s.privacy_clearance_reason = "procedural image, no personal content".into();
    assert!(s.validate().is_ok());
    s.privacy_cleared_refs.push(Uuid::from_u128(1));
    assert!(s.validate().is_err());
}
