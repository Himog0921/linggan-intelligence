use super::*;

fn fixture() -> (Value, Snapshot, Generation) {
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
fn text_only() -> (Value, Snapshot, Generation) {
    let (mut draft, mut snapshot, generation) = fixture();
    snapshot.images.clear();
    snapshot.included_kinds = vec!["body".into()];
    draft["imageContributions"] = json!([]);
    draft["evidenceCandidates"]["vision"] = json!([]);
    draft["core"]["knowledgeUnits"][2]["evidenceRefs"] = json!(["t1"]);
    (draft, snapshot, generation)
}
fn reject_at(pointer: &str, value: Value) {
    let (mut draft, snapshot, generation) = fixture();
    *draft.pointer_mut(pointer).expect("synthetic pointer") = value;
    assert!(
        accept(draft, &snapshot, &generation).is_err(),
        "accepted invalid {pointer}"
    );
}

#[test]
fn mixed_card_binds_all_identity_and_keeps_only_union_of_used_evidence() {
    let (draft, snapshot, generation) = fixture();
    let unchanged = draft.clone();
    let result = accept(draft.clone(), &snapshot, &generation).unwrap();
    let card = &result["card"];
    assert_eq!(draft, unchanged);
    assert_eq!(card["schemaVersion"], "content-card.v0.2");
    assert_eq!(card["workRef"], snapshot.work_ref.to_string());
    assert_eq!(card["analysisRef"], generation.analysis_ref.to_string());
    assert_eq!(card["deep"], Value::Null);
    assert_eq!(card["generation"]["methodVersion"], METHOD);
    assert_eq!(card["generation"]["createdAt"], generation.created_at);
    assert_eq!(
        card["generation"]["skillVersionRef"],
        generation.skill_version_ref
    );
    assert_eq!(
        card["generation"]["promptVersionRef"],
        generation.prompt_version_ref
    );
    assert_eq!(
        card["generation"]["modelConfigRef"],
        generation.model_config_ref
    );
    assert_eq!(
        card["generation"]["providerReturnedModel"],
        generation.provider_returned_model
    );
    assert_eq!(
        card["sourceSnapshot"]["materialSnapshotIdentity"],
        snapshot.material_snapshot_identity
    );
    assert_eq!(
        card["sourceSnapshot"]["materialAsOf"],
        snapshot.material_as_of
    );
    assert_eq!(card["sourceSnapshot"]["coverageMode"], "partial");
    assert_eq!(
        card["sourceSnapshot"]["includedKinds"],
        json!(["body", "image"])
    );
    let evidence = card["evidence"].as_array().unwrap();
    assert_eq!(
        evidence
            .iter()
            .map(|e| e["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["t1", "v1", "v2"]
    );
    assert!(
        !draft["core"]["knowledgeUnits"]
            .as_array()
            .unwrap()
            .iter()
            .any(|u| u["evidenceRefs"].as_array().unwrap().contains(&json!("v2")))
    );
    assert_eq!(result["imageContributions"], draft["imageContributions"]);
    assert_eq!(
        card["core"]["candidateLabels"], draft["core"]["candidateLabels"],
        "cross-type labels are not a semantic gate"
    );
    assert_eq!(
        evidence[1]["speakerRole"], "unknown",
        "no forced correspondence with the unit speaker"
    );
    assert_eq!(
        evidence[1]["sourceRef"],
        snapshot.images[0].source_ref.to_string()
    );
    assert_eq!(
        evidence[1]["sourceVersion"],
        snapshot.images[0].source_version
    );
    assert_eq!(evidence[1]["blobSha256"], snapshot.images[0].blob_sha256);
    assert_eq!(evidence[1]["verification"], "model_read");
    assert_eq!(evidence[1]["anchorKind"], "image");
    assert_eq!(evidence[1]["sourceKind"], "material");
    assert!(evidence[1].get("start").is_none() && evidence[1].get("quote").is_none());
}

#[test]
fn explicit_main_topic_is_not_the_first_discussion_and_is_required() {
    let (draft, snapshot, generation) = fixture();
    let result = accept(draft.clone(), &snapshot, &generation).unwrap();
    assert_eq!(result["card"]["core"]["mainTopicUnitId"], "u2");
    for id in ["u3", "missing", "1bad"] {
        reject_at("/core/mainTopicUnitId", json!(id));
    }
    let mut missing = draft;
    missing["core"]
        .as_object_mut()
        .unwrap()
        .remove("mainTopicUnitId");
    assert_eq!(
        accept(missing, &snapshot, &generation).unwrap_err(),
        "invalid_draft"
    );
}

#[test]
fn category_projections_are_derived_from_unit_types_in_order() {
    let (mut draft, snapshot, generation) = fixture();
    let units = draft["core"]["knowledgeUnits"].as_array_mut().unwrap();
    let template = units[2].clone();
    for (id, kind) in [
        ("u4", "audience_context"),
        ("u5", "author_position"),
        ("u6", "reader_value"),
        ("u7", "provision"),
    ] {
        let mut unit = template.clone();
        unit["id"] = json!(id);
        unit["unitType"] = json!(kind);
        units.push(unit);
    }
    let result = accept(draft, &snapshot, &generation).unwrap();
    let core = &result["card"]["core"];
    assert_eq!(core["audienceUnitIds"], json!(["u4"]));
    assert_eq!(core["authorPositionUnitIds"], json!(["u5"]));
    assert_eq!(core["provisionUnitIds"], json!(["u3", "u7"]));
    assert_eq!(core["readerValueUnitIds"], json!(["u6"]));
}

#[test]
fn text_offsets_are_unicode_scalars_with_slice_origin_and_unique_exact_quote() {
    let (draft, snapshot, generation) = fixture();
    let result = accept(draft, &snapshot, &generation).unwrap();
    let text = &result["card"]["evidence"][0];
    assert_eq!(text["start"], 11);
    assert_eq!(text["end"], 15);
    assert_eq!(text["sourceRef"], snapshot.texts[0].source_ref.to_string());
    assert_eq!(text["sourceVersion"], "text-v1");
    assert_eq!(text["quote"], "合成材料");
    for (source_text, quote) in [
        ("aaa", "aa"),
        ("🙂🙂", "🙂"),
        ("合成材料", "合成 材料"),
        ("e\u{301}", "é"),
        ("合成标题的“方法", "合成标题的“方法”"),
        ("把剩下的情绪转化成动力。", "把剩余的情绪转化成动力。"),
    ] {
        let (mut draft, mut snapshot, generation) = text_only();
        snapshot.texts[0].text = source_text.into();
        draft["evidenceCandidates"]["text"] =
            json!([{"id":"t1","inputCitationId":"text-1","quote":quote}]);
        assert_eq!(
            accept(draft, &snapshot, &generation).unwrap_err(),
            "invalid_text_quote"
        );
    }
    let (draft, mut snapshot, generation) = text_only();
    snapshot.texts[0].start = usize::MAX;
    assert_eq!(
        accept(draft, &snapshot, &generation).unwrap_err(),
        "invalid_text_quote"
    );
}

#[test]
fn pure_text_and_pure_image_inputs_are_supported() {
    let (draft, snapshot, generation) = text_only();
    let result = accept(draft, &snapshot, &generation).unwrap();
    assert_eq!(result["imageContributions"], json!([]));
    assert_eq!(result["card"]["evidence"].as_array().unwrap().len(), 1);
    let (mut draft, mut snapshot, generation) = fixture();
    snapshot.texts.clear();
    snapshot.included_kinds = vec!["image".into()];
    draft["evidenceCandidates"]["text"] = json!([]);
    for unit in draft["core"]["knowledgeUnits"].as_array_mut().unwrap() {
        unit["evidenceRefs"] = json!(["v1"]);
    }
    let result = accept(draft, &snapshot, &generation).unwrap();
    assert!(
        result["card"]["evidence"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["anchorKind"] == "image")
    );
}

#[test]
fn image_aliases_with_identical_bytes_are_not_deduplicated_or_fabricated() {
    let (mut draft, mut snapshot, generation) = fixture();
    let mut second = snapshot.images[0].clone();
    second.citation_id = "image-2".into();
    snapshot.images.push(second);
    draft["imageContributions"].as_array_mut().unwrap().push(json!({"inputCitationId":"image-2","status":"no_new_information","summary":"与首图相同的合成图片","unitIds":[],"evidenceRefs":[]}));
    let result = accept(draft.clone(), &snapshot, &generation).unwrap();
    assert_eq!(result["imageContributions"].as_array().unwrap().len(), 2);
    let mut duplicate = draft.clone();
    duplicate["imageContributions"][1]["inputCitationId"] = json!("image-1");
    assert!(accept(duplicate, &snapshot, &generation).is_err());
    let mut missing = draft.clone();
    missing["imageContributions"].as_array_mut().unwrap().pop();
    assert!(accept(missing, &snapshot, &generation).is_err());
    let mut fabricated = draft.clone();
    fabricated["imageContributions"][1]["inputCitationId"] = json!("image-3");
    assert!(accept(fabricated, &snapshot, &generation).is_err());
    draft["imageContributions"][1]["evidenceRefs"] = json!(["v1"]);
    assert!(
        accept(draft, &snapshot, &generation).is_err(),
        "cross-image contribution accepted"
    );
}

#[test]
fn contribution_statuses_are_explicit_and_failed_processing_is_not_a_final_answer() {
    let (draft, snapshot, generation) = fixture();
    for status in ["used", "no_new_information", "unclear"] {
        let mut v = draft.clone();
        v["imageContributions"][0]["status"] = json!(status);
        assert!(accept(v, &snapshot, &generation).is_ok());
    }
    for status in ["failed", "ok", "unknown"] {
        reject_at("/imageContributions/0/status", json!(status));
    }
    reject_at("/imageContributions/0/evidenceRefs", json!([]));
    for status in ["no_new_information", "unclear"] {
        let mut v = draft.clone();
        v["imageContributions"][0]["status"] = json!(status);
        v["imageContributions"][0]["unitIds"] = json!([]);
        v["imageContributions"][0]["evidenceRefs"] = json!([]);
        assert!(accept(v, &snapshot, &generation).is_ok());
    }
}

#[test]
fn references_ids_and_duplicates_are_rejected_in_every_reference_surface() {
    for pointer in [
        "/core/summary/unitIds",
        "/core/candidateLabels/0/unitIds",
        "/imageContributions/0/unitIds",
    ] {
        reject_at(pointer, json!(["missing"]));
        reject_at(pointer, json!(["u2", "u2"]));
    }
    for pointer in [
        "/core/knowledgeUnits/0/evidenceRefs",
        "/imageContributions/0/evidenceRefs",
    ] {
        reject_at(pointer, json!(["missing"]));
        reject_at(pointer, json!(["v1", "v1"]));
        reject_at(pointer, json!(["bad id"]));
    }
    reject_at("/imageContributions/0/evidenceRefs", json!(["t1"]));
    for id in ["", "1bad", "bad id", "a.b", "é", &"x".repeat(33)] {
        reject_at("/core/knowledgeUnits/0/id", json!(id));
    }
    reject_at("/core/knowledgeUnits/1/id", json!("u1"));
    reject_at("/evidenceCandidates/vision/0/id", json!("t1"));
    reject_at("/evidenceCandidates/vision/1/id", json!("v1"));
    reject_at("/evidenceCandidates/text/1/id", json!("t1"));
}

#[test]
fn source_aliases_are_exact_for_referenced_evidence() {
    for pointer in [
        "/evidenceCandidates/text/0/inputCitationId",
        "/evidenceCandidates/vision/0/inputCitationId",
        "/evidenceCandidates/vision/1/inputCitationId",
    ] {
        reject_at(pointer, json!("not-sent"));
    }
    reject_at(
        "/evidenceCandidates/text/0/inputCitationId",
        json!("image-1"),
    );
    reject_at(
        "/evidenceCandidates/vision/0/inputCitationId",
        json!("text-1"),
    );
    reject_at("/evidenceCandidates/text/0/quote", json!("not in input"));
}

#[test]
fn unused_candidates_are_pruned_before_source_binding_without_rewriting_the_draft() {
    let (draft, snapshot, generation) = fixture();
    let expected = accept(draft.clone(), &snapshot, &generation).unwrap();
    for (pointer, value, reference_pointer, id, error) in [
        (
            "/evidenceCandidates/text/1/quote",
            json!(format!("{}”", snapshot.texts[0].text)),
            "/core/knowledgeUnits/2/evidenceRefs",
            "unusedText",
            "invalid_text_quote",
        ),
        (
            "/evidenceCandidates/text/1/inputCitationId",
            json!("not-sent"),
            "/core/knowledgeUnits/2/evidenceRefs",
            "unusedText",
            "invalid_evidence_source",
        ),
        (
            "/evidenceCandidates/vision/2/inputCitationId",
            json!("not-sent"),
            "/imageContributions/0/evidenceRefs",
            "unusedVision",
            "invalid_evidence_source",
        ),
    ] {
        let mut value_draft = draft.clone();
        *value_draft.pointer_mut(pointer).unwrap() = value;
        let unchanged = value_draft.clone();
        assert_eq!(
            accept(value_draft.clone(), &snapshot, &generation).unwrap(),
            expected
        );
        assert_eq!(value_draft, unchanged);
        *value_draft.pointer_mut(reference_pointer).unwrap() = json!([id]);
        assert_eq!(
            accept(value_draft, &snapshot, &generation).unwrap_err(),
            error
        );
    }
    for (pointer, value) in [
        ("/evidenceCandidates/text/1/id", json!("t1")),
        ("/evidenceCandidates/text/1/quote", json!("")),
        ("/evidenceCandidates/vision/2/observation", json!("")),
        ("/evidenceCandidates/vision/2/speakerRole", json!("invalid")),
    ] {
        reject_at(pointer, value);
    }
}

#[test]
fn unknown_fields_cannot_smuggle_program_identity_offsets_or_projections() {
    let (draft, snapshot, generation) = fixture();
    for pointer in [
        "",
        "/core",
        "/core/summary",
        "/core/knowledgeUnits/0",
        "/core/candidateLabels/0",
        "/imageContributions/0",
        "/evidenceCandidates",
        "/evidenceCandidates/text/0",
        "/evidenceCandidates/vision/0",
    ] {
        let mut v = draft.clone();
        v.pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unexpected".into(), json!(true));
        assert_eq!(
            accept(v, &snapshot, &generation).unwrap_err(),
            "invalid_draft"
        );
    }
    for (pointer, key, value) in [
        ("", "analysisRef", json!(Uuid::from_u128(50))),
        ("/core", "audienceUnitIds", json!([])),
        ("/evidenceCandidates/text/0", "start", json!(0)),
        (
            "/evidenceCandidates/text/0",
            "sourceRef",
            json!(Uuid::from_u128(50)),
        ),
        (
            "/evidenceCandidates/vision/0",
            "blobSha256",
            json!("c".repeat(64)),
        ),
        (
            "/evidenceCandidates/vision/0",
            "verification",
            json!("verified"),
        ),
    ] {
        let mut v = draft.clone();
        v.pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(key.into(), value);
        assert!(accept(v, &snapshot, &generation).is_err());
    }
}

#[test]
fn nullable_vision_fields_must_be_present_and_substantive_when_not_null() {
    let (draft, snapshot, generation) = fixture();
    for field in ["modelQuote", "observation"] {
        let mut missing = draft.clone();
        missing["evidenceCandidates"]["vision"][0]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert_eq!(
            accept(missing, &snapshot, &generation).unwrap_err(),
            "invalid_draft"
        );
        for value in [
            json!(""),
            json!(" \n\t\u{3000}"),
            json!("字".repeat(501)),
            json!(12),
        ] {
            reject_at(&format!("/evidenceCandidates/vision/0/{field}"), value);
        }
    }
    let mut both_null = draft;
    both_null["evidenceCandidates"]["vision"][0]["observation"] = Value::Null;
    assert!(accept(both_null, &snapshot, &generation).is_err());
}

#[test]
fn type_enums_and_unicode_length_limits_are_checked_without_semantic_rewriting() {
    for (pointer, value) in [
        ("/core/contentType", "video"),
        ("/core/knowledgeUnits/0/unitType", "interpretation"),
        ("/core/knowledgeUnits/0/assertionKind", "discussion"),
        ("/core/knowledgeUnits/0/speakerRole", "commenter"),
        ("/core/candidateLabels/0/dimension", "topic"),
        ("/evidenceCandidates/vision/0/speakerRole", "verified"),
    ] {
        reject_at(pointer, json!(value));
    }
    for (pointer, max) in [
        ("/core/summary/text", 160),
        ("/core/knowledgeUnits/0/statement", 280),
        ("/core/candidateLabels/0/label", 80),
        ("/core/candidateLabels/0/retrievalUse", 180),
        ("/imageContributions/0/summary", 500),
    ] {
        let (mut draft, snapshot, generation) = fixture();
        *draft.pointer_mut(pointer).unwrap() = json!("🙂".repeat(max));
        assert!(
            accept(draft, &snapshot, &generation).is_ok(),
            "Unicode boundary {pointer}"
        );
        reject_at(pointer, json!("🙂".repeat(max + 1)));
        reject_at(pointer, json!(""));
        reject_at(pointer, Value::Null);
    }
    reject_at("/core/knowledgeUnits/0/evidenceRefs", json!([]));
    reject_at("/core/summary/unitIds", json!([]));
    reject_at("/core/candidateLabels/0/unitIds", json!([]));
}

#[test]
fn content_type_and_knowledge_type_all_documented_enum_values_are_accepted() {
    let (draft, snapshot, generation) = fixture();
    for kind in [
        "practical_guide",
        "checklist",
        "explanation",
        "personal_story",
        "opinion",
        "case_analysis",
        "interactive_challenge",
        "question",
        "product_intro",
        "mixed",
        "other",
    ] {
        let mut v = draft.clone();
        v["core"]["contentType"] = json!(kind);
        assert!(accept(v, &snapshot, &generation).is_ok());
    }
    for kind in [
        "discussion",
        "audience_context",
        "author_position",
        "provision",
        "reader_value",
        "answer_form",
        "expression",
        "participation",
    ] {
        let mut v = draft.clone();
        v["core"]["knowledgeUnits"][2]["unitType"] = json!(kind);
        assert!(accept(v, &snapshot, &generation).is_ok());
    }
}

#[test]
fn collection_and_projection_count_ceilings_reject_rather_than_truncate() {
    let (draft, snapshot, generation) = fixture();
    for (kind, max) in [
        ("audience_context", 4),
        ("author_position", 4),
        ("provision", 8),
        ("reader_value", 4),
    ] {
        for count in [max, max + 1] {
            let mut v = draft.clone();
            v["core"]["knowledgeUnits"][2]["unitType"] = json!("discussion");
            for index in 0..count {
                let mut unit = v["core"]["knowledgeUnits"][0].clone();
                unit["id"] = json!(format!("extra{index}"));
                unit["unitType"] = json!(kind);
                v["core"]["knowledgeUnits"]
                    .as_array_mut()
                    .unwrap()
                    .push(unit);
            }
            assert_eq!(
                accept(v, &snapshot, &generation).is_ok(),
                count == max,
                "{kind} count {count}"
            );
        }
    }
    for count in [12, 13] {
        let mut v = draft.clone();
        let label = v["core"]["candidateLabels"][0].clone();
        v["core"]["candidateLabels"] = json!(vec![label; count]);
        assert_eq!(
            accept(v, &snapshot, &generation).is_ok(),
            count == 12,
            "label count"
        );
        let mut v = draft.clone();
        let units = v["core"]["knowledgeUnits"].as_array_mut().unwrap();
        for index in 3..count {
            let mut unit = units[0].clone();
            unit["id"] = json!(format!("extra{index}"));
            units.push(unit);
        }
        assert_eq!(
            accept(v, &snapshot, &generation).is_ok(),
            count == 12,
            "unit count"
        );
    }
    reject_at("/core/knowledgeUnits", json!([]));
    for count in [64, 65] {
        let (mut v, snapshot, generation) = text_only();
        v["evidenceCandidates"]["text"] = json!((0..count).map(|i| json!({"id":if i == 0 {"t1".to_owned()} else {format!("t{i}x")},"inputCitationId":"text-1","quote":"合成材料"})).collect::<Vec<_>>());
        assert_eq!(
            accept(v, &snapshot, &generation).is_ok(),
            count == 64,
            "combined candidates count"
        );
    }
    for pointer in [
        "/core/summary/unitIds",
        "/core/candidateLabels/0/unitIds",
        "/core/knowledgeUnits/0/evidenceRefs",
        "/imageContributions/0/evidenceRefs",
    ] {
        reject_at(
            pointer,
            json!((0..9).map(|i| format!("id{i}")).collect::<Vec<_>>()),
        );
    }
    reject_at(
        "/imageContributions/0/unitIds",
        json!((0..13).map(|i| format!("id{i}")).collect::<Vec<_>>()),
    );
}

#[test]
fn metadata_and_snapshot_format_validation_rejects_invalid_program_inputs() {
    let (draft, snapshot, generation) = fixture();
    for timestamp in [
        "",
        "2026-11-10",
        "2026-02-29T00:00:00Z",
        "2026-11-10T24:00:00Z",
        "2026-11-10T00:00:00",
        "2026-11-10T00:00:00+25:00",
        "not-a-date",
    ] {
        let mut s = snapshot.clone();
        s.material_as_of = timestamp.into();
        assert!(accept(draft.clone(), &s, &generation).is_err());
        let mut g = generation.clone();
        g.created_at = timestamp.into();
        assert!(accept(draft.clone(), &snapshot, &g).is_err());
    }
    for bad_hash in ["a".repeat(63), "A".repeat(64), "g".repeat(64)] {
        let mut s = snapshot.clone();
        s.material_snapshot_identity = bad_hash.clone();
        assert!(accept(draft.clone(), &s, &generation).is_err());
        let mut s = snapshot.clone();
        s.images[0].blob_sha256 = bad_hash;
        assert!(accept(draft.clone(), &s, &generation).is_err());
    }
    for (pointer, value) in [
        ("/workRef", json!("not-uuid")),
        ("/texts/0/sourceRef", json!("not-uuid")),
        ("/images/0/sourceRef", json!("not-uuid")),
        ("/texts/0/start", json!(-1)),
        ("/texts/0/start", json!(1.5)),
        ("/texts/0/start", json!("0")),
        ("/extra", json!(true)),
    ] {
        let mut s = serde_json::to_value(&snapshot).unwrap();
        if pointer == "/extra" {
            s["extra"] = value;
        } else {
            *s.pointer_mut(pointer).unwrap() = value;
        }
        assert!(serde_json::from_value::<Snapshot>(s).is_err(), "{pointer}");
    }
    let mut g = serde_json::to_value(&generation).unwrap();
    g["analysisRef"] = json!("not-uuid");
    assert!(serde_json::from_value::<Generation>(g).is_err());
    for included in [
        vec![],
        vec!["vision".into()],
        vec!["body".into()],
        vec!["body".into(), "image".into(), "image".into()],
    ] {
        let mut s = snapshot.clone();
        s.included_kinds = included;
        assert!(accept(draft.clone(), &s, &generation).is_err());
    }
    let mut s = snapshot.clone();
    s.coverage_mode = "unknown".into();
    assert!(accept(draft.clone(), &s, &generation).is_err());
    let mut s = snapshot.clone();
    s.images[0].citation_id = "text-1".into();
    assert!(accept(draft.clone(), &s, &generation).is_err());
    let mut s = snapshot.clone();
    s.texts.push(s.texts[0].clone());
    assert!(accept(draft.clone(), &s, &generation).is_err());
    for pointer in [
        "/skillVersionRef",
        "/promptVersionRef",
        "/modelConfigRef",
        "/providerReturnedModel",
    ] {
        for value in ["".to_owned(), "x".repeat(161)] {
            let mut g = serde_json::to_value(&generation).unwrap();
            *g.pointer_mut(pointer).unwrap() = json!(value);
            assert!(
                accept(
                    draft.clone(),
                    &snapshot,
                    &serde_json::from_value(g).unwrap()
                )
                .is_err()
            );
        }
    }
}

#[test]
fn response_schema_uses_explicit_new_draft_without_identity_or_legacy_status() {
    let schema = response_schema();
    assert_eq!(schema["additionalProperties"], false);
    let core = &schema["properties"]["core"];
    assert!(
        core["required"]
            .as_array()
            .unwrap()
            .contains(&json!("mainTopicUnitId"))
    );
    assert_eq!(core["properties"].as_object().unwrap().len(), 5);
    assert_eq!(
        core["properties"]["summary"]["properties"]["text"]["maxLength"],
        160
    );
    assert!(schema["properties"].get("workRef").is_none());
    assert_eq!(
        schema["properties"]["imageContributions"]["items"]["properties"]["status"]["enum"],
        json!(["used", "no_new_information", "unclear"])
    );
    let candidates = &schema["properties"]["evidenceCandidates"]["properties"];
    assert_eq!(
        candidates["text"]["items"]["properties"]
            .as_object()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(
        candidates["vision"]["items"]["properties"]
            .as_object()
            .unwrap()
            .len(),
        5
    );
    assert!(
        candidates["vision"]["items"]["required"]
            .as_array()
            .unwrap()
            .contains(&json!("modelQuote"))
    );
    assert_eq!(candidates["text"]["maxItems"], 64);
}

#[test]
fn response_schema_explains_projection_limits_without_changing_field_bounds() {
    let schema = response_schema();
    let units = &schema["properties"]["core"]["properties"]["knowledgeUnits"];
    let storage: Value = serde_json::from_str(include_str!(
        "../../../../docs/data-contracts/content-card.v0.2.schema.json"
    ))
    .unwrap();
    let core = &storage["properties"]["core"]["properties"];
    let description = units["description"].as_str().unwrap();
    for (kind, projection) in [
        ("audience_context", "audienceUnitIds"),
        ("author_position", "authorPositionUnitIds"),
        ("provision", "provisionUnitIds"),
        ("reader_value", "readerValueUnitIds"),
    ] {
        assert!(description.contains(&format!("{kind}最多{}条", core[projection]["maxItems"])));
    }
    assert_eq!(units["minItems"], core["knowledgeUnits"]["minItems"]);
    assert_eq!(units["maxItems"], core["knowledgeUnits"]["maxItems"]);
    let kind = &units["items"]["properties"]["unitType"];
    assert_eq!(
        kind["enum"],
        core["knowledgeUnits"]["items"]["properties"]["unitType"]["enum"]
    );
    let description = kind["description"].as_str().unwrap();
    assert!(description.contains("assertionKind"));
    assert!(description.contains("作者主张不等于作者立场"));
    assert!(description.contains("provision"));
}

#[test]
fn response_schema_examples_keep_label_links_and_literal_truncated_quotes() {
    let schema = response_schema();
    let core = &schema["properties"]["core"]["properties"];
    let label = core["candidateLabels"]["items"]["examples"][0].clone();
    assert_eq!(label["unitIds"], json!(["u3"]));
    let mut definition = core["candidateLabels"].clone();
    definition.as_object_mut().unwrap().remove("description");
    definition["items"]
        .as_object_mut()
        .unwrap()
        .remove("examples");
    definition["items"]["properties"]["unitIds"]
        .as_object_mut()
        .unwrap()
        .remove("description");
    let storage: Value = serde_json::from_str(include_str!(
        "../../../../docs/data-contracts/content-card.v0.2.schema.json"
    ))
    .unwrap();
    assert_eq!(
        definition,
        storage["properties"]["core"]["properties"]["candidateLabels"]
    );
    let (mut draft, snapshot, generation) = fixture();
    draft["core"]["candidateLabels"] = json!([label]);
    assert!(accept(draft.clone(), &snapshot, &generation).is_ok());
    draft["core"]["candidateLabels"][0]
        .as_object_mut()
        .unwrap()
        .remove("unitIds");
    assert_eq!(
        accept(draft, &snapshot, &generation).unwrap_err(),
        "invalid_draft"
    );
    let quote = &schema["properties"]["evidenceCandidates"]["properties"]["text"]["items"]["properties"]
        ["quote"];
    assert!(
        quote["description"]
            .as_str()
            .unwrap()
            .contains("内部连续短语")
    );
    let (mut draft, mut snapshot, generation) = text_only();
    snapshot.texts[0].text = "合成标题：“分步练习".into();
    draft["evidenceCandidates"]["text"] = json!([{
        "id":"t1","inputCitationId":"text-1","quote":quote["examples"][0]
    }]);
    let result = accept(draft.clone(), &snapshot, &generation).unwrap();
    let evidence = &result["card"]["evidence"][0];
    assert_eq!(evidence["quote"], "分步练习");
    assert_eq!(evidence["start"], 13);
    assert_eq!(evidence["end"], 17);
    draft["evidenceCandidates"]["text"][0]["quote"] = json!("合成标题：“分步练习”");
    assert_eq!(
        accept(draft, &snapshot, &generation).unwrap_err(),
        "invalid_text_quote"
    );
}

#[test]
fn response_schema_explains_closed_json_literal_quotes_and_image_local_evidence() {
    let schema = response_schema();
    let description = schema["description"].as_str().unwrap();
    assert!(description.contains("一个完整的 JSON 对象"));
    assert!(description.contains("额外字段"));
    assert!(description.contains("括号"));
    let fields = &schema["properties"];
    let quote = fields["evidenceCandidates"]["properties"]["text"]["items"]["properties"]["quote"]
        ["description"]
        .as_str()
        .unwrap();
    for rule in ["逐字", "连续", "唯一", "标点", "改写", "拼接"] {
        assert!(quote.contains(rule));
    }
    let evidence =
        fields["imageContributions"]["items"]["properties"]["evidenceRefs"]["description"]
            .as_str()
            .unwrap();
    for rule in ["vision", "inputCitationId", "text", "其他图片"] {
        assert!(evidence.contains(rule));
    }
}
