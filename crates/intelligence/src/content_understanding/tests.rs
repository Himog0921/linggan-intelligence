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

fn accepted_with(pointer: &str, value: Value) -> Value {
    let (mut draft, snapshot, generation) = fixture();
    *draft.pointer_mut(pointer).expect("synthetic pointer") = value;
    accept(draft, &snapshot, &generation).unwrap_or_else(|code| panic!("{pointer}: {code}"))
}
fn has_note(result: &Value, reason: &str) -> bool {
    result["admissionNotes"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|note| note["reason"] == reason)
}
fn unit_ids(result: &Value) -> Vec<String> {
    result["card"]["core"]["knowledgeUnits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|unit| unit["id"].as_str().unwrap().to_owned())
        .collect()
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
fn main_topic_keeps_the_declared_discussion_and_otherwise_falls_back() {
    let (draft, snapshot, generation) = fixture();
    let result = accept(draft.clone(), &snapshot, &generation).unwrap();
    assert_eq!(result["card"]["core"]["mainTopicUnitId"], "u2");
    assert!(result.get("admissionNotes").is_none());
    for id in ["u3", "missing", "1bad"] {
        let result = accepted_with("/core/mainTopicUnitId", json!(id));
        assert_eq!(result["card"]["core"]["mainTopicUnitId"], "u1", "{id}");
        assert!(has_note(&result, "main_topic_reassigned"));
    }
    let mut missing = draft;
    missing["core"]
        .as_object_mut()
        .unwrap()
        .remove("mainTopicUnitId");
    let result = accept(missing, &snapshot, &generation).unwrap();
    assert_eq!(result["card"]["core"]["mainTopicUnitId"], "u1");
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
fn text_offsets_are_unicode_scalars_and_stored_quotes_are_literal_source_slices() {
    let (draft, snapshot, generation) = fixture();
    let result = accept(draft, &snapshot, &generation).unwrap();
    let text = &result["card"]["evidence"][0];
    assert_eq!(text["start"], 11);
    assert_eq!(text["end"], 15);
    assert_eq!(text["sourceRef"], snapshot.texts[0].source_ref.to_string());
    assert_eq!(text["sourceVersion"], "text-v1");
    assert_eq!(text["quote"], "合成材料");
    let quoted = |source_text: &str, quote: &str| {
        let (mut draft, mut snapshot, generation) = text_only();
        snapshot.texts[0].text = source_text.into();
        draft["evidenceCandidates"]["text"] =
            json!([{"id":"t1","inputCitationId":"text-1","quote":quote}]);
        accept(draft, &snapshot, &generation)
    };
    // A repeated phrase is still literal material; the first occurrence anchors it.
    for (source_text, quote, start) in [("aaa", "aa", 7), ("🙂🙂", "🙂", 7)] {
        let result = quoted(source_text, quote).unwrap();
        assert_eq!(result["card"]["evidence"][0]["start"], start);
        assert!(has_note(&result, "quote_repeated_first_used"));
    }
    // Spacing and punctuation differences keep the source's own characters.
    for (source_text, quote) in [
        ("合成材料", "合成 材料"),
        ("合成标题的“方法", "合成标题的“方法”"),
    ] {
        let result = quoted(source_text, quote).unwrap();
        assert_eq!(result["card"]["evidence"][0]["quote"], source_text);
        assert!(has_note(&result, "quote_punctuation_ignored"));
    }
    // Changed characters are not in the material: the evidence and its units go.
    for (source_text, quote) in [
        ("e\u{301}", "é"),
        ("把剩下的情绪转化成动力。", "把剩余的情绪转化成动力。"),
    ] {
        assert_eq!(quoted(source_text, quote).unwrap_err(), "invalid_draft");
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
    let contributions = |draft: Value| {
        let result = accept(draft, &snapshot, &generation).unwrap();
        result["imageContributions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["inputCitationId"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>()
    };
    let mut duplicate = draft.clone();
    duplicate["imageContributions"][1]["inputCitationId"] = json!("image-1");
    assert_eq!(contributions(duplicate), ["image-1"]);
    let mut missing = draft.clone();
    missing["imageContributions"].as_array_mut().unwrap().pop();
    assert_eq!(contributions(missing), ["image-1"]);
    let mut fabricated = draft.clone();
    fabricated["imageContributions"][1]["inputCitationId"] = json!("image-3");
    assert_eq!(contributions(fabricated), ["image-1"]);
    draft["imageContributions"][1]["evidenceRefs"] = json!(["v1"]);
    let result = accept(draft, &snapshot, &generation).unwrap();
    assert_eq!(
        result["imageContributions"][1]["evidenceRefs"],
        json!([]),
        "evidence from another image is not attached"
    );
}

#[test]
fn contribution_statuses_are_explicit_and_unknown_statuses_become_unclear() {
    let (draft, snapshot, generation) = fixture();
    for status in ["used", "no_new_information", "unclear"] {
        let result = accepted_with("/imageContributions/0/status", json!(status));
        assert_eq!(result["imageContributions"][0]["status"], status);
    }
    for status in ["failed", "ok", "unknown"] {
        let result = accepted_with("/imageContributions/0/status", json!(status));
        assert_eq!(result["imageContributions"][0]["status"], "unclear");
    }
    for status in ["no_new_information", "unclear"] {
        let mut v = draft.clone();
        v["imageContributions"][0]["status"] = json!(status);
        v["imageContributions"][0]["unitIds"] = json!([]);
        v["imageContributions"][0]["evidenceRefs"] = json!([]);
        assert!(accept(v, &snapshot, &generation).is_ok());
    }
}

#[test]
fn references_are_filtered_and_malformed_ids_are_renamed_consistently() {
    for pointer in [
        "/core/summary/unitIds",
        "/core/candidateLabels/0/unitIds",
        "/imageContributions/0/unitIds",
    ] {
        let result = accepted_with(pointer, json!(["u2", "u2", "missing"]));
        let kept = &result
            .pointer(&pointer.replace("/core/", "/card/core/"))
            .cloned()
            .unwrap_or(result["imageContributions"][0]["unitIds"].clone());
        assert_eq!(kept, &json!(["u2"]), "{pointer}");
    }
    let result = accepted_with("/core/knowledgeUnits/0/evidenceRefs", json!(["missing"]));
    assert_eq!(unit_ids(&result), ["u2", "u3"]);
    assert!(has_note(&result, "no_supported_evidence"));
    let result = accepted_with(
        "/imageContributions/0/evidenceRefs",
        json!(["t1", "v1", "v1"]),
    );
    assert_eq!(
        result["imageContributions"][0]["evidenceRefs"],
        json!(["v1"])
    );
    for id in ["", "1bad", "bad id", "a.b", "é", &"x".repeat(33)] {
        let result = accepted_with("/core/knowledgeUnits/0/id", json!(id));
        assert_eq!(unit_ids(&result), ["ux1", "u2", "u3"], "{id:?}");
        assert!(has_note(&result, "id_renamed"));
    }
    let result = accepted_with("/core/knowledgeUnits/1/id", json!("u1"));
    assert_eq!(unit_ids(&result), ["u1", "u3"]);
    assert!(has_note(&result, "duplicate_id"));
    let result = accepted_with("/evidenceCandidates/vision/0/id", json!("t1"));
    assert_eq!(
        unit_ids(&result),
        ["u1", "u2"],
        "u3 loses its only image evidence"
    );
}

#[test]
fn text_evidence_anchors_where_the_quote_is_and_image_evidence_needs_a_sent_image() {
    for value in [json!("not-sent"), json!("image-1")] {
        let result = accepted_with("/evidenceCandidates/text/0/inputCitationId", value);
        assert_eq!(result["card"]["evidence"][0]["inputCitationId"], "text-1");
        assert!(has_note(&result, "quote_reanchored"));
    }
    for value in [json!("not-sent"), json!("text-1")] {
        let result = accepted_with("/evidenceCandidates/vision/0/inputCitationId", value);
        assert_eq!(unit_ids(&result), ["u1", "u2"]);
        assert!(has_note(&result, "invalid_image_evidence"));
    }
    let result = accepted_with("/evidenceCandidates/text/0/quote", json!("not in input"));
    assert_eq!(unit_ids(&result), ["u3"]);
    assert!(has_note(&result, "quote_not_in_material"));
}

#[test]
fn unused_candidates_are_pruned_before_source_binding_without_rewriting_the_draft() {
    let (draft, snapshot, generation) = fixture();
    let expected = accept(draft.clone(), &snapshot, &generation).unwrap();
    for (pointer, value) in [
        (
            "/evidenceCandidates/text/1/quote",
            json!(format!("{}”", snapshot.texts[0].text)),
        ),
        (
            "/evidenceCandidates/text/1/inputCitationId",
            json!("not-sent"),
        ),
        (
            "/evidenceCandidates/vision/2/inputCitationId",
            json!("not-sent"),
        ),
        ("/evidenceCandidates/text/1/quote", json!("")),
        ("/evidenceCandidates/vision/2/observation", json!("")),
    ] {
        let mut value_draft = draft.clone();
        *value_draft.pointer_mut(pointer).unwrap() = value;
        let unchanged = value_draft.clone();
        assert_eq!(
            accept(value_draft.clone(), &snapshot, &generation).unwrap(),
            expected,
            "{pointer}"
        );
        assert_eq!(value_draft, unchanged);
    }
}

#[test]
fn unknown_fields_are_ignored_and_cannot_change_identity_offsets_or_projections() {
    let (draft, snapshot, generation) = fixture();
    let expected = accept(draft.clone(), &snapshot, &generation).unwrap();
    for (pointer, key, value) in [
        ("", "unexpected", json!(true)),
        ("/core/summary", "unexpected", json!(true)),
        ("/imageContributions/0", "unexpected", json!(true)),
        ("", "analysisRef", json!(Uuid::from_u128(50))),
        ("/core", "audienceUnitIds", json!(["u1"])),
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
        assert_eq!(
            accept(v, &snapshot, &generation).unwrap(),
            expected,
            "{pointer}/{key}"
        );
    }
}

#[test]
fn blank_vision_fields_count_as_null_and_empty_image_evidence_is_dropped() {
    let (draft, snapshot, generation) = fixture();
    let expected = accept(draft.clone(), &snapshot, &generation).unwrap();
    let mut missing = draft.clone();
    missing["evidenceCandidates"]["vision"][0]
        .as_object_mut()
        .unwrap()
        .remove("modelQuote");
    assert_eq!(accept(missing, &snapshot, &generation).unwrap(), expected);
    for value in [json!(""), json!(" \n\t\u{3000}"), json!("字".repeat(501))] {
        let result = accepted_with("/evidenceCandidates/vision/0/modelQuote", value);
        assert_eq!(result, expected);
    }
    for value in [json!(""), json!(12), Value::Null] {
        let result = accepted_with("/evidenceCandidates/vision/0/observation", value);
        assert_eq!(unit_ids(&result), ["u1", "u2"]);
    }
}

#[test]
fn unknown_enum_values_and_oversized_items_drop_only_that_item() {
    let result = accepted_with("/core/contentType", json!("video"));
    assert_eq!(result["card"]["core"]["contentType"], "other");
    for pointer in [
        "/core/knowledgeUnits/0/unitType",
        "/core/knowledgeUnits/0/assertionKind",
        "/core/knowledgeUnits/0/statement",
        "/core/knowledgeUnits/0/evidenceRefs",
    ] {
        let value = if pointer.ends_with("statement") {
            json!("🙂".repeat(281))
        } else if pointer.ends_with("evidenceRefs") {
            json!([])
        } else {
            json!("interpretation_or_discussion")
        };
        assert_eq!(
            unit_ids(&accepted_with(pointer, value)),
            ["u2", "u3"],
            "{pointer}"
        );
    }
    for pointer in [
        "/core/knowledgeUnits/0/speakerRole",
        "/evidenceCandidates/vision/0/speakerRole",
    ] {
        let result = accepted_with(pointer, json!("commenter"));
        assert!(
            result.to_string().contains("\"speakerRole\":\"unknown\""),
            "{pointer}"
        );
    }
    for (pointer, value) in [
        ("/core/candidateLabels/0/dimension", json!("topic")),
        ("/core/candidateLabels/0/label", json!("🙂".repeat(81))),
        (
            "/core/candidateLabels/0/retrievalUse",
            json!("🙂".repeat(181)),
        ),
        ("/core/candidateLabels/0/unitIds", json!([])),
    ] {
        let result = accepted_with(pointer, value);
        assert_eq!(
            result["card"]["core"]["candidateLabels"],
            json!([]),
            "{pointer}"
        );
    }
    let result = accepted_with("/imageContributions/0/summary", json!("🙂".repeat(501)));
    assert_eq!(result["imageContributions"], json!([]));
    let result = accepted_with("/core/summary/unitIds", json!([]));
    assert_eq!(result["card"]["core"]["summary"]["unitIds"], json!(["u2"]));
    // The summary has no fallback, so its bounds still decide the answer.
    let (draft, snapshot, generation) = fixture();
    let mut boundary = draft.clone();
    boundary["core"]["summary"]["text"] = json!("🙂".repeat(160));
    assert!(accept(boundary, &snapshot, &generation).is_ok());
    for value in [json!("🙂".repeat(161)), json!(""), Value::Null] {
        reject_at("/core/summary/text", value);
    }
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
fn collection_and_projection_ceilings_keep_the_first_items() {
    let (draft, snapshot, generation) = fixture();
    for (kind, projection, max) in [
        ("audience_context", "audienceUnitIds", 4),
        ("author_position", "authorPositionUnitIds", 4),
        ("reader_value", "readerValueUnitIds", 4),
    ] {
        let mut v = draft.clone();
        for index in 0..max + 1 {
            let mut unit = v["core"]["knowledgeUnits"][0].clone();
            unit["id"] = json!(format!("extra{index}"));
            unit["unitType"] = json!(kind);
            v["core"]["knowledgeUnits"]
                .as_array_mut()
                .unwrap()
                .push(unit);
        }
        let result = accept(v, &snapshot, &generation).unwrap();
        assert_eq!(
            result["card"]["core"][projection].as_array().unwrap().len(),
            max
        );
        assert!(has_note(&result, "over_limit"), "{kind}");
    }
    let mut v = draft.clone();
    let label = v["core"]["candidateLabels"][0].clone();
    v["core"]["candidateLabels"] = json!(vec![label; 13]);
    let units = v["core"]["knowledgeUnits"].as_array_mut().unwrap();
    for index in 3..13 {
        let mut unit = units[0].clone();
        unit["id"] = json!(format!("extra{index}"));
        units.push(unit);
    }
    let result = accept(v, &snapshot, &generation).unwrap();
    assert_eq!(
        result["card"]["core"]["candidateLabels"]
            .as_array()
            .unwrap()
            .len(),
        12
    );
    assert_eq!(
        result["card"]["core"]["knowledgeUnits"]
            .as_array()
            .unwrap()
            .len(),
        12
    );
    reject_at("/core/knowledgeUnits", json!([]));
    let (mut v, snapshot, generation) = text_only();
    v["evidenceCandidates"]["text"] = json!((0..65).map(|i| json!({"id":if i == 0 {"t1".to_owned()} else {format!("t{i}x")},"inputCitationId":"text-1","quote":"合成材料"})).collect::<Vec<_>>());
    assert!(
        accept(v, &snapshot, &generation).is_ok(),
        "unused candidates do not count"
    );
    let result = accepted_with(
        "/core/knowledgeUnits/2/evidenceRefs",
        json!(["v1", "t1", "v2", "v1"]),
    );
    assert_eq!(
        result["card"]["core"]["knowledgeUnits"][2]["evidenceRefs"],
        json!(["v1", "t1", "v2"])
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
    let fields = &schema["properties"];
    let sources = &fields["evidenceCandidates"]["properties"];
    let image = &fields["imageContributions"]["items"]["properties"];
    assert_eq!(
        sources["text"]["items"]["properties"]["inputCitationId"]["pattern"],
        "^s[1-9][0-9]*$"
    );
    assert_eq!(
        image["inputCitationId"],
        sources["vision"]["items"]["properties"]["inputCitationId"]
    );
    assert_ne!(image["inputCitationId"]["pattern"], "^s[1-9][0-9]*$");
    assert_eq!(
        image["evidenceRefs"]["items"]["pattern"],
        "^ev[A-Za-z0-9_-]*$"
    );
    assert_ne!(
        image["unitIds"]["items"]["pattern"],
        image["evidenceRefs"]["items"]["pattern"]
    );
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
    let result = accept(draft, &snapshot, &generation).unwrap();
    assert_eq!(result["card"]["core"]["candidateLabels"], json!([]));
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
    let result = accept(draft, &snapshot, &generation).unwrap();
    assert_eq!(
        result["card"]["evidence"][0]["quote"],
        "合成标题：“分步练习"
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

#[test]
fn missing_transcript_line_break_restores_the_exact_source_slice_and_anchor() {
    let (mut draft, mut snapshot, generation) = text_only();
    let source = &mut snapshot.texts[0];
    source.kind = "transcript".into();
    source.text = "前🙂，合成材料，\n继续复核。".into();
    source.structure = json!({"time":{"startMs":12000,"endMs":18000}});
    snapshot.included_kinds = vec!["transcript".into()];
    draft["evidenceCandidates"]["text"][0]["quote"] = json!("合成材料，继续复核");

    let result = accept(draft, &snapshot, &generation).unwrap();
    let evidence = &result["card"]["evidence"][0];
    let anchored = snapshot
        .texts
        .iter()
        .find(|text| text.citation_id == evidence["inputCitationId"].as_str().unwrap())
        .unwrap();
    assert_eq!(evidence["sourceRef"], anchored.source_ref.to_string());
    assert_eq!(evidence["sourceVersion"], anchored.source_version);
    assert_eq!(evidence["start"], 10);
    assert_eq!(evidence["end"], 20);
    assert_eq!(evidence["quote"], "合成材料，\n继续复核");
    assert_eq!(anchored.structure["time"]["startMs"], 12000);
}

#[test]
fn transcript_quotes_anchor_to_the_segment_that_holds_them_literally() {
    let quote = "合成材料，继续复核";
    for (kind, cited_text, anchored) in [
        ("transcript", "合成材料,\n继续复核", "text-2"),
        ("transcript", "合成材料， 继续复核", "text-2"),
        (
            "transcript",
            "合成材料，\n继续复核，合成材料，\n继续复核",
            "text-1",
        ),
        (
            "transcript",
            "合成材料，\n继续复核。另：合成材料，继续复核",
            "text-1",
        ),
        ("transcript", "合成材料，\n\n继续复核", "text-2"),
        ("transcript", "其他片段", "text-2"),
        ("frame_text", "合成材料，\n继续复核", "text-2"),
    ] {
        let (mut draft, mut snapshot, generation) = text_only();
        snapshot.texts[0].kind = kind.into();
        snapshot.texts[0].text = cited_text.into();
        snapshot.texts[0].structure = json!({"time":{"startMs":1000,"endMs":2000}});
        let mut other = snapshot.texts[0].clone();
        other.citation_id = "text-2".into();
        other.source_ref = Uuid::from_u128(9);
        other.text = quote.into();
        other.structure = json!({"time":{"startMs":9000,"endMs":10000}});
        snapshot.texts.push(other);
        snapshot.included_kinds = vec![kind.into()];
        draft["evidenceCandidates"]["text"][0]["quote"] = json!(quote);
        let result = accept(draft, &snapshot, &generation).unwrap();
        let evidence = &result["card"]["evidence"][0];
        assert_eq!(
            evidence["inputCitationId"], anchored,
            "{kind}: {cited_text}"
        );
        let source = snapshot
            .texts
            .iter()
            .find(|t| t.citation_id == anchored)
            .unwrap();
        assert!(source.text.contains(evidence["quote"].as_str().unwrap()));
    }
}

#[test]
fn draft_metadata_and_unsupported_optional_label_recover_without_changing_core() {
    let (draft, snapshot, generation) = fixture();
    let expected = accept(draft.clone(), &snapshot, &generation).unwrap();
    let mut draft = draft;
    draft["type"] = json!("json_object");
    draft["core"]["knowledgeUnitsNote"] = Value::Null;
    draft["core"]["knowledgeUnits_note"] = Value::Null;
    draft["evidenceCandidates"]["vision"][0]["speakerRole"] = json!("unconfirmed");
    let mut extra_label = draft["core"]["candidateLabels"][0].clone();
    extra_label["dimension"] = json!("author_position");
    let labels = draft["core"]["candidateLabels"].as_array_mut().unwrap();
    labels.push(extra_label);
    assert_eq!(accept(draft, &snapshot, &generation).unwrap(), expected);
}

#[test]
fn draft_metadata_and_unlinked_labels_do_not_block_the_core() {
    let (draft, snapshot, generation) = fixture();
    let units =
        accept(draft.clone(), &snapshot, &generation).unwrap()["card"]["core"]["knowledgeUnits"]
            .clone();
    for (pointer, key, value) in [
        ("/core", "knowledgeUnits_note", json!("额外主张")),
        ("", "type", json!("unsupported")),
    ] {
        let mut v = draft.clone();
        v.pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(key.into(), value);
        let result = accept(v, &snapshot, &generation).unwrap();
        assert_eq!(result["card"]["core"]["knowledgeUnits"], units);
    }
    let mut v = draft;
    v["core"]["candidateLabels"][0]["dimension"] = json!("author_position");
    v["core"]["candidateLabels"][0]["unitIds"] = json!(["missing"]);
    let result = accept(v, &snapshot, &generation).unwrap();
    assert_eq!(result["card"]["core"]["candidateLabels"], json!([]));
    assert!(has_note(&result, "invalid_label"));
}

#[test]
fn transcript_layout_recovery_keeps_the_source_line_break_and_unique_position() {
    let source = "前🙂，合成材料\n接着核对，\n最后确认。";
    let quote = "合成材料,\n接着核对，最后确认。";
    let (mut draft, mut snapshot, generation) = text_only();
    snapshot.texts[0].kind = "transcript".into();
    snapshot.texts[0].text = source.into();
    snapshot.texts[0].structure = json!({"time":{"startMs":12000,"endMs":18000}});
    snapshot.included_kinds = vec!["transcript".into()];
    draft["evidenceCandidates"]["text"][0]["quote"] = json!(quote);

    let result = accept(draft, &snapshot, &generation).unwrap();
    let evidence = &result["card"]["evidence"][0];
    assert_eq!(evidence["inputCitationId"], snapshot.texts[0].citation_id);
    assert_eq!(
        evidence["sourceRef"],
        snapshot.texts[0].source_ref.to_string()
    );
    assert_eq!(evidence["sourceVersion"], snapshot.texts[0].source_version);
    assert_eq!(evidence["quote"], "合成材料\n接着核对，\n最后确认。");
    assert_eq!(evidence["start"], snapshot.texts[0].start + 3);
    assert_eq!(
        evidence["end"],
        snapshot.texts[0].start + source.chars().count()
    );
    assert_eq!(snapshot.texts[0].structure["time"]["startMs"], 12000);
}

#[test]
fn transcript_layout_recovery_keeps_first_repeat_and_rejects_content_changes() {
    for source in ["材料\n核对。另：材料,\n核对", "材料\n核对。另：材料\n核对"]
    {
        assert_eq!(
            quote_in_source(source, "材料,\n核对", true),
            Some((0, "材料\n核对", true)),
            "{source:?}"
        );
    }
    for (source, quote, wrap) in [
        ("材料\n核对", "材料,\n核对", false),
        ("材料\n核对", "材料,核对", true),
        ("材料\n核对", "材料。\n核对", true),
        ("金额1\n开始", "金额1,\n开始", true),
        ("材料\n不核对", "材料,\n核对", true),
        ("材料\n核对", "材料,\n核对。", true),
        ("材料\n\n核对", "材料,\n核对", true),
    ] {
        assert_eq!(
            quote_in_source(source, quote, wrap),
            None,
            "{source:?} {quote:?}"
        );
    }
}

#[test]
fn model_draft_keeps_canonical_source_and_requires_direct_book_title_support() {
    let (mut draft, mut snapshot, generation) = text_only();
    snapshot.texts[0].text = "课程使用两本教辅。\n步骤要先对版本。".into();
    let mut segment = snapshot.texts[0].clone();
    segment.citation_id = "canonical-segment".into();
    segment.kind = "transcript".into();
    segment.start = 11;
    segment.text = "🙂教材全练 题例".into();
    segment.speaker = "unknown".into();
    segment.structure = json!({"ref":"canonical-segment","time":{"startMs":120000,"endMs":120000}});
    snapshot.texts.push(segment);
    snapshot.included_kinds.push("transcript".into());
    draft["evidenceCandidates"]["text"] = json!([
        {"id":"ev1","inputCitationId":"s1","quote":"课程使用两本教辅"},
        {"id":"ev2","inputCitationId":"s2","quote":"教材全练"}
    ]);
    for unit in draft["core"]["knowledgeUnits"].as_array_mut().unwrap() {
        unit["evidenceRefs"] = json!(["ev1"]);
    }
    draft["core"]["knowledgeUnits"][2]["statement"] = json!("作者使用《教材全练》。");
    let verify = |draft: Value| model_draft::accept_model_draft(draft, &snapshot, &generation);
    // A unit naming a book its evidence does not show is dropped; the rest stays.
    let result = verify(draft.clone()).unwrap();
    assert_eq!(unit_ids(&result), ["u1", "u2"]);
    assert!(has_note(&result, "unsupported_literal"));
    // The summary has no fallback: an unsupported title there fails the answer.
    let mut summary = draft.clone();
    summary["core"]["summary"]["text"] = json!("作者介绍《教材全练》。");
    summary["core"]["summary"]["unitIds"] = json!(["u2"]);
    assert_eq!(verify(summary), Err("unsupported_literal"));
    draft["core"]["knowledgeUnits"][2]["evidenceRefs"] = json!(["ev1", "ev2"]);
    draft["core"]["summary"]["text"] = json!("作者介绍《教材全练》。");

    let material = model_draft::material(&snapshot);
    assert_eq!(material["texts"][0]["citationId"], "s1");
    assert_eq!(material["texts"][1]["citationId"], "s2");
    assert_eq!(snapshot.texts[1].citation_id, "canonical-segment");
    assert!(material["texts"][1]["structure"].get("ref").is_none());
    assert_eq!(
        material["texts"][1]["structure"]["time"],
        snapshot.texts[1].structure["time"]
    );

    let result = verify(draft.clone()).unwrap();
    let bound = &result["card"]["evidence"][1];
    assert_eq!(bound["inputCitationId"], "canonical-segment");
    assert_eq!(bound["start"], 12);
    assert_eq!(bound["end"], 16);
    assert_eq!(bound["quote"], "教材全练");

    let mut wrong_channel = draft.clone();
    wrong_channel["core"]["knowledgeUnits"][2]["statement"] = json!("画面原文「教材全练」。");
    wrong_channel["core"]["summary"]["text"] = json!("作者介绍教辅。");
    let result = verify(wrong_channel).unwrap();
    assert_eq!(unit_ids(&result), ["u1", "u2"]);
    let mut literal_from_speech = draft.clone();
    literal_from_speech["core"]["knowledgeUnits"][2]["statement"] = json!("口播原话「教材全练」。");
    assert_eq!(
        unit_ids(&verify(literal_from_speech).unwrap()),
        ["u1", "u2", "u3"]
    );

    // Citing the canonical id, the wrong segment, or the segment itself still
    // resolves to the transcript segment that literally holds the quote.
    let mut original_id = draft.clone();
    original_id["evidenceCandidates"]["text"][1]["inputCitationId"] = json!("canonical-segment");
    let mut wrong_source = draft.clone();
    wrong_source["evidenceCandidates"]["text"][1]["inputCitationId"] = json!("s1");
    let mut segment_ref = draft;
    segment_ref["core"]["knowledgeUnits"][2]["evidenceRefs"] = json!(["ev1", "s2"]);
    for variant in [original_id, wrong_source, segment_ref] {
        let result = verify(variant).unwrap();
        assert_eq!(
            result["card"]["evidence"][1]["inputCitationId"],
            "canonical-segment"
        );
        assert_eq!(unit_ids(&result), ["u1", "u2", "u3"]);
    }
}

#[test]
fn model_draft_keeps_image_reads_distinct_from_literal_transcriptions() {
    let (mut draft, mut snapshot, generation) = fixture();
    draft["evidenceCandidates"]["text"] = json!([
        {"id":"ev1","inputCitationId":"s1","quote":"合成材料"}
    ]);
    draft["evidenceCandidates"]["vision"] = json!([
        {"id":"ev2","inputCitationId":"image-1","modelQuote":null,"observation":"图片上写有合成步骤","speakerRole":"unknown"}
    ]);
    let units = draft["core"]["knowledgeUnits"].as_array_mut().unwrap();
    units[0]["evidenceRefs"] = json!(["ev1"]);
    units[1]["evidenceRefs"] = json!(["ev1"]);
    units[2]["evidenceRefs"] = json!(["ev2"]);
    units[2]["statement"] = json!("图片原文「合成步骤」。");
    draft["imageContributions"][0]["evidenceRefs"] = json!(["ev2"]);
    let verify =
        |draft: Value, snap: &Snapshot| model_draft::accept_model_draft(draft, snap, &generation);
    let kept = |draft: Value, snap: &Snapshot| unit_ids(&verify(draft, snap).unwrap());
    assert_eq!(kept(draft.clone(), &snapshot), ["u1", "u2"]);
    draft["evidenceCandidates"]["vision"][0]["modelQuote"] = json!("合成步骤");
    let result = verify(draft.clone(), &snapshot).unwrap();
    assert_eq!(result["card"]["evidence"][1]["verification"], "model_read");
    assert_eq!(unit_ids(&result), ["u1", "u2", "u3"]);

    // On-screen quotes are image reads now that video frame text is retired.
    let mut on_screen = draft.clone();
    on_screen["core"]["knowledgeUnits"][2]["statement"] = json!("画面原文：「合成步骤」。");
    assert_eq!(kept(on_screen, &snapshot), ["u1", "u2", "u3"]);
    for unsupported in [
        "口播原话: “合成步骤”。",
        "图片原文「合成步骤",
        "图片原文「 」",
        "图片原文「 合成步骤 」",
        "图片原文「说明「合成步骤」。",
    ] {
        let mut variant = draft.clone();
        variant["core"]["knowledgeUnits"][2]["statement"] = json!(unsupported);
        assert_eq!(kept(variant, &snapshot), ["u1", "u2"], "{unsupported}");
    }

    let mut wrong_source = draft.clone();
    wrong_source["core"]["knowledgeUnits"][2]["statement"] = json!("图片 原文：「合成材料」。");
    wrong_source["core"]["knowledgeUnits"][2]["evidenceRefs"] = json!(["ev1"]);
    assert_eq!(kept(wrong_source, &snapshot), ["u1", "u2"]);

    let mut unqualified = draft;
    unqualified["core"]["knowledgeUnits"][2]["statement"] = json!("原文「合成步骤」。");
    assert_eq!(kept(unqualified, &snapshot), ["u1", "u2", "u3"]);
}
