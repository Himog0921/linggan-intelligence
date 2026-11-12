use super::*;
use crate::content_understanding::{Generation, ImageSource, Snapshot, TextSource, accept};
fn core_fixture() -> (Value, Snapshot, Generation) {
    let snapshot = Snapshot {
        work_ref: Uuid::from_u128(1),
        material_snapshot_identity: "a".repeat(64),
        material_as_of: "2026-11-10T09:10:11.123+08:00".into(),
        included_kinds: vec!["body".into(), "image".into()],
        coverage_mode: "partial".into(),
        texts: vec![TextSource {
            citation_id: "text-1".into(),
            source_ref: Uuid::from_u128(2),
            source_version: "text-v1".into(),
            start: 7,
            text: "前🙂段，合成材料支持步骤。第二条依据".into(),
            kind: "body".into(),
            speaker: "author".into(),
            structure: json!({"kind":"paragraph"}),
            issues: vec![],
        }],
        images: vec![ImageSource {
            citation_id: "image-1".into(),
            source_ref: Uuid::from_u128(3),
            source_version: "image-v1".into(),
            blob_sha256: "b".repeat(64),
        }],
    };
    let generation = Generation {
        analysis_ref: Uuid::from_u128(4),
        skill_version_ref: "skill-v1".into(),
        prompt_version_ref: "prompt-v1".into(),
        model_config_ref: "model-v1".into(),
        provider_returned_model: "synthetic-model".into(),
        created_at: "2026-11-10T01:11:12Z".into(),
    };
    let draft = json!({
        "core":{
            "summary":{"text":"合成图文内容摘要","unitIds":["u2","u3"]},
            "contentType":"practical_guide","mainTopicUnitId":"u2",
            "knowledgeUnits":[
                {"id":"u1","unitType":"discussion","statement":"合成背景","assertionKind":"observation","speakerRole":"author","evidenceRefs":["t1"]},
                {"id":"u2","unitType":"discussion","statement":"模型明确选择的主议题","assertionKind":"interpretation","speakerRole":"unknown","evidenceRefs":["t1"]},
                {"id":"u3","unitType":"provision","statement":"合成图片提供一个步骤","assertionKind":"author_claim","speakerRole":"author","evidenceRefs":["v1"]}
            ],
            "candidateLabels":[{"dimension":"audience","label":"合成检索词","retrievalUse":"查找合成步骤","unitIds":["u3"]}]
        },
        "imageContributions":[{"inputCitationId":"image-1","status":"used","summary":"图片中的合成步骤","unitIds":["u3"],"evidenceRefs":["v1","v2"]}],
        "evidenceCandidates":{
            "text":[{"id":"t1","inputCitationId":"text-1","quote":"合成材料"},{"id":"unusedText","inputCitationId":"text-1","quote":"第二条依据"}],
            "vision":[
                {"id":"v1","inputCitationId":"image-1","modelQuote":null,"observation":"合成步骤图","speakerRole":"unknown"},
                {"id":"v2","inputCitationId":"image-1","modelQuote":"合成补充","observation":null,"speakerRole":"quoted"},
                {"id":"unusedVision","inputCitationId":"image-1","modelQuote":null,"observation":"冗余图像依据","speakerRole":"unknown"}
            ]
        }
    });
    (draft, snapshot, generation)
}

fn with_cards() -> ResearchInput {
    let mut input = input(vec![
        source("body", "不应重复送入模型的原材料", 21),
        source("unresearched_comment", "仅评论提出的新方向", 22),
    ]);
    let (draft, snapshot, generation) = core_fixture();
    let accepted = accept(draft, &snapshot, &generation).unwrap();
    input.coverage["availableCores"] = json!([accepted["card"]]);
    input
}
#[test]
fn core_input_keeps_real_text_offsets_and_derived_image_anchor() {
    let original = with_cards();
    let core = from_core(&original, None).unwrap();
    assert_eq!(core.fragments.len(), 2);
    assert_eq!(core.comment_study, json!([]));
    assert!(!core.fragments.iter().any(|f| f.text.contains("仅评论")));
    let text = &core.fragments[0];
    assert_eq!(text.text, "合成材料");
    assert_eq!(text.start, 11); // canonical source start 7 + 前🙂段，
    assert_eq!(text.end, 15);
    let image = &core.fragments[1];
    assert_eq!(image.field, "core_image_evidence");
    assert_eq!(image.source_ref, Uuid::from_u128(4));
    assert_eq!(image.start, 0);
    assert_eq!(
        image.structure["anchor"]["sourceRef"],
        json!(Uuid::from_u128(3))
    );
    assert_eq!(image.structure["anchor"]["verification"], "model_read");
    assert_eq!(image.speaker, "unconfirmed"); // image anchor overrides author knowledge unit
    let windows = research_windows(&core);
    assert_eq!(windows.len(), 1);
    let manifest = reference_manifest(&windows[0]);
    assert_eq!(manifest["inputContract"], CORE_INPUT_CONTRACT);
    assert_eq!(manifest["coverage"]["coreRef"], json!(Uuid::from_u128(4)));
    let restored = restore_scoped_window(&[original], Uuid::from_u128(1), &manifest).unwrap();
    assert_eq!(restored.hash, windows[0].hash);
}
#[test]
fn frozen_core_is_not_replaced_by_latest_and_missing_core_never_falls_back() {
    let mut input = with_cards();
    let windows = research_windows(&from_core(&input, None).unwrap());
    let manifest = reference_manifest(&windows[0]);
    let mut new = input.coverage["availableCores"][0].clone();
    new["analysisRef"] = json!(Uuid::from_u128(99));
    input.coverage["availableCores"]
        .as_array_mut()
        .unwrap()
        .insert(0, new.clone());
    assert_eq!(
        from_core(&input, None).unwrap().coverage["coreRef"],
        new["analysisRef"]
    );
    let restored = restore_scoped_window(&[input.clone()], Uuid::from_u128(1), &manifest).unwrap();
    assert_eq!(restored.coverage["coreRef"], json!(Uuid::from_u128(4)));
    input.coverage["availableCores"] = json!([new]);
    assert!(restore_scoped_window(&[input.clone()], Uuid::from_u128(1), &manifest).is_none());
    input.coverage["availableCores"] = json!([]);
    assert!(from_core(&input, None).is_none());
}
#[test]
fn core_summary_changes_identity_and_legacy_windows_still_restore() {
    let mut original = with_cards();
    let first = from_core(&original, None).unwrap();
    original.coverage["availableCores"][0]["core"]["summary"]["text"] = json!("更新摘要");
    let second = from_core(&original, None).unwrap();
    assert_ne!(first.hash, second.hash);
    let old = input(vec![source("body", "历史原文", 31)]);
    let window = research_windows(&old).remove(0);
    assert!(
        restore_scoped_window(&[old], Uuid::from_u128(1), &reference_manifest(&window)).is_some()
    );
}
