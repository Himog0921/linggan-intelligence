use super::*;
use serde_json::{Value, json};

mod concept_quality;
use concept_quality::new_proposal;

fn fragment(id: &str, field: &str, start: usize, value: &str) -> Fragment {
    Fragment {
        fragment_id: id.into(),
        source_ref: Uuid::new_v4(),
        field: field.into(),
        source_version: "synthetic-v1".into(),
        start,
        end: start + value.chars().count(),
        text: value.into(),
    }
}
fn citation(f: &Fragment) -> Citation {
    Citation {
        fragment_id: f.fragment_id.clone(),
        start: f.start,
        end: f.end,
    }
}
fn discussion(f: &Fragment) -> Discussion {
    Discussion {
        label: "合成任务启动".into(),
        topic_ref: None,
        evidence: vec![citation(f)],
        statement: "合成原声描述开始任务时的困难".into(),
        definition: "开始一项已决定执行的任务时遇到的具体困难".into(),
        inclusion_criteria: vec!["已决定做但无法开始第一步".into()],
        exclusion_criteria: vec!["已经开始后的中途分心".into()],
        speaker_role: "author".into(),
        evidence_role: "support".into(),
        rationale: "合成材料明确涉及开始第一步".into(),
    }
}
fn sample() -> (ResearchOutput, Vec<Fragment>) {
    let f = fragment("synthetic.body", "body", 7, "A中😀e\u{301}B");
    let output = ResearchOutput {
        contract: EXTRACT_CONTRACT.into(),
        outcome: "analyzed".into(),
        discussions: vec![discussion(&f)],
        scenes: vec![],
        journey: Journey {
            main_stage: "unclear".into(),
            involved_stages: vec![],
            overlays: vec![],
            path: "unknown".into(),
            rationale: "没有经历阶段依据".into(),
            evidence: vec![],
        },
        response_matches: vec![],
        angles: vec![],
        product_opportunities: vec![],
        limitations: vec!["合成测试，不是现实证据".into()],
    };
    (output, vec![f])
}
#[test]
fn saved_v1_output_remains_readable_without_v2_fields_or_two_sided_checks() {
    let (mut o, f) = sample();
    o.contract = "topic-map.research.v1".into();
    o.response_matches.push(ResponseMatch {
        status: "direct".into(),
        unanswered: vec![],
        evidence: vec![citation(&f[0])],
    });
    let mut value = serde_json::to_value(o).unwrap();
    for field in [
        "statement",
        "definition",
        "inclusionCriteria",
        "exclusionCriteria",
        "speakerRole",
        "evidenceRole",
        "rationale",
    ] {
        value["discussions"][0]
            .as_object_mut()
            .unwrap()
            .remove(field);
    }
    let legacy: ResearchOutput = serde_json::from_value(value).unwrap();
    assert!(validate_output(&legacy, &f, &[]).is_ok());
}
#[test]
fn v2_requires_explicit_meaning_boundaries_and_keeps_unknown_low_frequency() {
    let (o, f) = sample();
    assert!(validate_output(&o, &f, &[]).is_ok());
    for field in [
        "statement",
        "definition",
        "inclusionCriteria",
        "exclusionCriteria",
        "speakerRole",
        "evidenceRole",
        "rationale",
    ] {
        let mut value = serde_json::to_value(&o).unwrap();
        value["discussions"][0]
            .as_object_mut()
            .unwrap()
            .remove(field);
        let missing: ResearchOutput = serde_json::from_value(value).unwrap();
        assert!(
            validate_output(&missing, &f, &[]).is_err(),
            "missing {field}"
        );
    }
    let mut unknown = o;
    unknown.discussions[0].speaker_role = "unknown".into();
    unknown.discussions[0].evidence_role = "challenge".into();
    assert!(validate_output(&unknown, &f, &[]).is_ok());
}
#[test]
fn extraction_cannot_preassign_even_a_known_topic() {
    let (mut o, f) = sample();
    let topic = Uuid::new_v4();
    o.discussions[0].topic_ref = Some(topic);
    assert!(validate_output(&o, &f, &[topic]).is_err());
}
#[test]
fn unicode_citations_use_original_scalar_window_coordinates() {
    let (mut o, f) = sample();
    assert_eq!(f[0].end, 13); // nonzero origin; emoji and combining mark are each scalars.
    o.discussions[0].evidence[0].start = 9;
    o.discussions[0].evidence[0].end = 10;
    assert!(validate_output(&o, &f, &[]).is_ok());
    for (start, end) in [(6, 8), (9, 9), (9, 14), (0, 1)] {
        o.discussions[0].evidence[0].start = start;
        o.discussions[0].evidence[0].end = end;
        assert!(validate_output(&o, &f, &[]).is_err());
    }
    let (o, mut malformed) = sample();
    malformed[0].end = malformed[0].start + malformed[0].text.len();
    assert_eq!(
        validate_output(&o, &malformed, &[]),
        Err("invalid_unicode_fragment")
    );
}
#[test]
fn foreign_or_ambiguous_fragment_ids_are_rejected() {
    let (mut o, mut f) = sample();
    o.discussions[0].evidence[0].fragment_id = "not-in-input".into();
    assert!(validate_output(&o, &f, &[]).is_err());
    let (o, _) = sample();
    f.push(f[0].clone());
    assert_eq!(
        validate_output(&o, &f, &[]),
        Err("invalid_unicode_fragment")
    );
}
#[test]
fn a_comment_cannot_be_presented_as_an_author_statement() {
    let (mut o, mut f) = sample();
    let c = fragment("synthetic.comment", "unresearched_comment", 0, "合成主评论");
    o.discussions[0].evidence = vec![citation(&c)];
    f.push(c);
    assert!(validate_output(&o, &f, &[]).is_err());
    o.discussions[0].speaker_role = "commenter".into();
    assert!(validate_output(&o, &f, &[]).is_ok());
    o.discussions[0].evidence = vec![citation(&f[0])];
    assert!(validate_output(&o, &f, &[]).is_err());
}
#[test]
fn parent_context_does_not_become_an_independent_comment_experience() {
    let (mut o, mut f) = sample();
    let c = fragment("synthetic.comment", "studied_comment", 0, "合成主评论");
    let p = fragment(
        "synthetic.parent",
        "parent_comment_context",
        0,
        "合成父评论",
    );
    o.discussions[0].speaker_role = "commenter".into();
    o.discussions[0].evidence = vec![citation(&p)];
    f.extend([c.clone(), p]);
    assert!(validate_output(&o, &f, &[]).is_err());
    o.discussions[0].evidence.push(citation(&c));
    assert!(validate_output(&o, &f, &[]).is_ok());
    o.discussions[0].speaker_role = "unknown".into();
    o.discussions[0].evidence.pop();
    assert!(validate_output(&o, &f, &[]).is_err());
    o.discussions[0].evidence_role = "context".into();
    assert!(validate_output(&o, &f, &[]).is_ok());
}
#[test]
fn response_comparisons_need_real_author_and_primary_comment_sides() {
    let (mut o, mut f) = sample();
    let c = fragment("synthetic.comment", "studied_comment", 0, "合成主评论");
    let p = fragment(
        "synthetic.parent",
        "parent_comment_context",
        0,
        "合成父评论",
    );
    f.extend([c.clone(), p.clone()]);
    for status in ["direct", "partial", "unmatched"] {
        o.response_matches = vec![ResponseMatch {
            status: status.into(),
            unanswered: vec![],
            evidence: vec![citation(&f[0])],
        }];
        assert!(validate_output(&o, &f, &[]).is_err());
        o.response_matches[0].evidence.push(citation(&p));
        assert!(validate_output(&o, &f, &[]).is_err());
        o.response_matches[0].evidence.push(citation(&c));
        assert!(validate_output(&o, &f, &[]).is_ok());
        o.response_matches[0].evidence.remove(0);
        assert!(validate_output(&o, &f, &[]).is_err());
    }
    o.response_matches[0].status = "unknown".into();
    o.response_matches[0].evidence.clear();
    assert!(validate_output(&o, &f, &[]).is_ok());
}
#[test]
fn rejects_hidden_authority_and_duplicate_angle_tasks() {
    let (mut o, f) = sample();
    let mut extra = serde_json::to_value(&o).unwrap();
    extra["tools"] = json!(["fetch"]);
    assert!(serde_json::from_value::<ResearchOutput>(extra).is_err());
    let a = Angle {
        label: "合成A".into(),
        title: "合成标题A".into(),
        answer_task: "SYN task".into(),
        evidence: vec![citation(&f[0])],
    };
    o.angles = vec![
        a.clone(),
        Angle {
            label: "合成B".into(),
            title: "合成标题B".into(),
            answer_task: " syn TASK ".into(),
            ..a
        },
    ];
    assert_eq!(validate_output(&o, &f, &[]), Err("duplicate_angle_task"));
}

fn resolution_case() -> (Vec<(String, Discussion)>, Value, ResolutionOutput) {
    let (o, _) = sample();
    let units = vec![("synthetic-unit-1".into(), o.discussions[0].clone())];
    let topic = Uuid::new_v4();
    let definition = Uuid::new_v4();
    let topics = json!([{"topicRef":topic,"definitionRef":definition,"label":"合成主题"}]);
    let output = ResolutionOutput {
        contract: RESOLVE_CONTRACT.into(),
        decisions: vec![UnitDecision {
            unit_id: "synthetic-unit-1".into(),
            status: "matched".into(),
            matches: vec![TopicMatch {
                topic_ref: topic,
                definition_ref: definition,
                reason: "含义与候选的启动边界一致".into(),
            }],
            proposed_topic: None,
            relations: vec![],
            reason: "原声涉及候选规定的开始阶段".into(),
        }],
    };
    (units, topics, output)
}
#[test]
fn resolution_covers_every_unit_exactly_once() {
    let (mut units, topics, mut o) = resolution_case();
    assert!(validate_resolution(&o, &units, &topics).is_ok());
    units.push(("synthetic-unit-2".into(), units[0].1.clone()));
    assert!(validate_resolution(&o, &units, &topics).is_err());
    o.decisions.push(o.decisions[0].clone());
    assert!(validate_resolution(&o, &units, &topics).is_err());
    o.decisions[1].unit_id = "foreign-unit".into();
    assert!(validate_resolution(&o, &units, &topics).is_err());
    o.decisions[1].unit_id = "synthetic-unit-2".into();
    assert!(validate_resolution(&o, &units, &topics).is_ok());
}

#[test]
fn resolution_requires_exact_candidate_and_definition_versions() {
    let (units, topics, mut o) = resolution_case();
    o.decisions[0].matches[0].definition_ref = Uuid::new_v4();
    assert_eq!(
        validate_resolution(&o, &units, &topics),
        Err("invalid_resolution_match")
    );
    o.decisions[0].matches[0].topic_ref = Uuid::new_v4();
    assert!(validate_resolution(&o, &units, &topics).is_err());
}

#[test]
fn low_frequency_and_multiple_memberships_are_preserved_without_count_gate() {
    let (mut units, mut topics, mut o) = resolution_case();
    units[0].1.speaker_role = "unknown".into();
    for _ in 0..3 {
        let topic = Uuid::new_v4();
        let definition = Uuid::new_v4();
        topics
            .as_array_mut()
            .unwrap()
            .push(json!({"topicRef":topic,"definitionRef":definition}));
        o.decisions[0].matches.push(TopicMatch {
            topic_ref: topic,
            definition_ref: definition,
            reason: "合成独立归属依据".into(),
        });
    }
    assert!(validate_resolution(&o, &units, &topics).is_ok());
    let duplicate = o.decisions[0].matches[0].clone();
    o.decisions[0].matches.push(duplicate);
    assert!(validate_resolution(&o, &units, &topics).is_err());
}

#[test]
fn new_topic_needs_explicit_boundaries_and_comparison_to_every_candidate() {
    let (units, topics, mut o) = resolution_case();
    let matched = o.decisions[0].matches.pop().unwrap();
    o.decisions[0].status = "new".into();
    o.decisions[0].proposed_topic = Some(new_proposal());
    assert!(validate_resolution(&o, &units, &topics).is_err());
    o.decisions[0].relations.push(TopicRelation {
        topic_ref: matched.topic_ref,
        definition_ref: matched.definition_ref,
        relation: "distinct".into(),
        reason: "持续过程与开始第一步不同".into(),
    });
    assert!(validate_resolution(&o, &units, &topics).is_ok());
    o.decisions[0].relations[0].relation = "equivalent".into();
    assert!(validate_resolution(&o, &units, &topics).is_err());
    o.decisions[0].relations[0].relation = "distinct".into();
    o.decisions[0]
        .proposed_topic
        .as_mut()
        .unwrap()
        .exclusion_criteria
        .clear();
    assert!(validate_resolution(&o, &units, &topics).is_err());
}

#[test]
fn new_topic_without_candidates_is_valid_and_not_blocked_by_frequency() {
    let (units, _, mut o) = resolution_case();
    o.decisions[0].status = "new".into();
    o.decisions[0].matches.clear();
    o.decisions[0].proposed_topic = Some(new_proposal());
    assert!(validate_resolution(&o, &units, &json!([])).is_ok());
}

#[test]
fn uncertain_and_out_of_scope_cannot_create_membership_or_new_topic() {
    let (units, topics, mut o) = resolution_case();
    for status in ["uncertain", "out_of_scope"] {
        o.decisions[0].status = status.into();
        assert!(validate_resolution(&o, &units, &topics).is_err());
    }
    o.decisions[0].matches.clear();
    for status in ["uncertain", "out_of_scope"] {
        o.decisions[0].status = status.into();
        assert!(validate_resolution(&o, &units, &topics).is_ok());
        o.decisions[0].proposed_topic = Some(new_proposal());
        assert!(validate_resolution(&o, &units, &topics).is_err());
        o.decisions[0].proposed_topic = None;
    }
}

#[test]
fn resolution_cannot_rewrite_frozen_roles_or_omit_nullable_contract_field() {
    let (_, _, o) = resolution_case();
    let mut raw = serde_json::to_value(&o).unwrap();
    raw["decisions"][0]["speakerRole"] = json!("author");
    assert!(serde_json::from_value::<ResolutionOutput>(raw).is_err());
    let mut raw = serde_json::to_value(o).unwrap();
    raw["decisions"][0]
        .as_object_mut()
        .unwrap()
        .remove("proposedTopic");
    assert!(serde_json::from_value::<ResolutionOutput>(raw).is_err());
}

fn assert_complete_schema(schema: &Value) {
    if schema["type"] == "object" {
        let properties = schema["properties"].as_object().unwrap();
        let required: HashSet<_> = schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(required, properties.keys().map(String::as_str).collect());
        assert_eq!(schema["additionalProperties"], false);
        for nested in properties.values() {
            assert_complete_schema(nested);
        }
    }
    if schema["type"] == "array" {
        assert_complete_schema(&schema["items"]);
    }
    if let Some(variants) = schema["anyOf"].as_array() {
        for variant in variants {
            assert_complete_schema(variant);
        }
    }
}
fn assert_serialized_shape(value: &Value, schema: &Value) {
    if value.is_null() {
        return;
    }
    if let Some(variants) = schema["anyOf"].as_array() {
        assert_serialized_shape(value, &variants[0]);
        return;
    }
    if let Some(fields) = value.as_object() {
        let properties = schema["properties"].as_object().unwrap();
        assert_eq!(
            fields.keys().collect::<Vec<_>>(),
            properties.keys().collect::<Vec<_>>()
        );
        for (key, nested) in fields {
            assert_serialized_shape(nested, &properties[key]);
        }
    }
    if let Some(items) = value.as_array() {
        for item in items {
            assert_serialized_shape(item, &schema["items"]);
        }
    }
}

#[test]
fn complete_schemas_match_serialized_rust_contracts() {
    let (mut o, f) = sample();
    o.scenes.push(Scene {
        label: "合成场景".into(),
        evidence: vec![citation(&f[0])],
    });
    o.angles.push(Angle {
        label: "合成角度".into(),
        title: "合成标题".into(),
        answer_task: "合成回答任务".into(),
        evidence: vec![citation(&f[0])],
    });
    o.response_matches.push(ResponseMatch {
        status: "unknown".into(),
        unanswered: vec![],
        evidence: vec![],
    });
    o.product_opportunities.push(ProductOpportunity {
        need: "合成需求".into(),
        hypothesis: "合成假设".into(),
        verification_question: "合成验证问题".into(),
        alternative_explanation: "合成替代解释".into(),
        evidence: vec![citation(&f[0])],
    });
    let (_, _, mut r) = resolution_case();
    let m = r.decisions[0].matches[0].clone();
    r.decisions[0].relations.push(TopicRelation {
        topic_ref: m.topic_ref,
        definition_ref: m.definition_ref,
        relation: "related".into(),
        reason: "合成关系".into(),
    });
    r.decisions[0].proposed_topic = Some(new_proposal());
    for (value, schema) in [
        (serde_json::to_value(o).unwrap(), output_schema()),
        (serde_json::to_value(r).unwrap(), resolution_schema()),
    ] {
        assert_complete_schema(&schema);
        assert_serialized_shape(&value, &schema);
        assert!(schema.to_string().len() < 32768);
    }
    assert_eq!(
        output_schema()["properties"]["discussions"]["items"]["properties"]["topicRef"]["type"],
        "null"
    );
}

#[test]
fn token_estimation_vectors_match_adapter_contract() {
    for (system, prompt, expected) in [
        ("", "", 128),
        ("", "abc", 129),
        ("", "abcd", 130),
        ("A", "BC", 130),
        ("中", "😀", 130),
        ("", "a_b", 131),
        ("", "甲e\u{301}😀", 132),
    ] {
        assert_eq!(estimate_input_tokens(system, prompt), expected);
    }
    assert_eq!(estimate_input_tokens("", &"甲".repeat(12000)), 12128);
}
