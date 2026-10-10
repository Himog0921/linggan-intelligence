//! Pure projection proofs; PostgreSQL acceptance and source gates have separate fixtures.
use super::*;
use crate::topic_map_research_analysis::{Citation, Discussion, ResearchOutput};
use linggan_evidence::creator_discovery::{DiscoveryWork, hash};

#[path = "tests/comparison_projection.rs"]
mod comparison_projection;
#[path = "tests/partial_projection.rs"]
mod partial_projection;
#[path = "tests/scope_and_identity.rs"]
mod scope_and_identity;

fn topic(id: u128) -> TopicMapTopic {
    let mut topic = TopicMapTopic {
        topic_ref: Uuid::from_u128(id),
        canonical_key: format!("synthetic:{id}"),
        domain_ref: Some(Uuid::from_u128(3)),
        definition_ref: Uuid::from_u128(id + 1),
        definition_version: 1,
        display_name: "同名主题".into(),
        definition_text: format!("概念边界{id}"),
        lifecycle_state: "candidate".into(),
        parent_topic_ref: None,
        binding_version: Some(1),
        direct_work_refs: vec![],
        work_refs: vec![],
        statistics: json!({}),
        journey: json!({}),
        core: json!({}),
    };
    units::apply_rule(
        &mut topic,
        Some((vec!["明确纳入标准".into()], vec!["明确排除标准".into()])),
        false,
    );
    topic
}

fn work(id: u128, readable: bool) -> TopicMapWork {
    TopicMapWork {
        work_ref: Uuid::from_u128(id),
        platform: "xhs".into(),
        content_external_id: format!("synthetic:{id}"),
        title: None,
        title_source: "unavailable".into(),
        author_external_id: None,
        creator_display_name: None,
        published_at: None,
        published_at_source_text: None,
        published_at_precision: "unknown".into(),
        likes: None,
        comments: None,
        collects: None,
        shares: None,
        follower_count: None,
        own: false,
        own_breakout: false,
        readable,
        usage_roles: vec!["primary".into()],
        topic_refs: vec![],
        main_stage: "pending".into(),
        involved_stages: vec![],
        overlays: vec![],
        path: "unknown".into(),
        annotation: None,
        research: None,
        media: json!({}),
        preview: json!({}),
        evidence_fragment: None,
        source_manifest: json!({}),
    }
}

fn input(field: &str) -> ResearchInput {
    let work_ref = Uuid::from_u128(1);
    let text = "我一直难以开始任务。🙂开始以后又容易分心。";
    let fragment = Fragment {
        fragment_id: format!("{work_ref}.{field}.synthetic"),
        source_ref: Uuid::from_u128(5),
        source_version: hash(text),
        field: field.into(),
        start: 10_001,
        end: 10_001 + text.chars().count(),
        text: text.into(),
    };
    let author = ["title", "body", "ocr", "transcript"].contains(&field);
    ResearchInput {
        work: DiscoveryWork {
            work_ref,
            usage_roles: vec!["primary".into()],
            creator_key: None,
            platform: "xhs".into(),
            author_external_id: None,
            display_name: None,
            title: None,
            published_date: None,
            published_epoch: None,
            likes: None,
            likes_observed_at: None,
            likes_observed_display: None,
            comments: None,
            collects: None,
            first_added: String::new(),
            first_added_display: String::new(),
            fingerprint: String::new(),
            search_text: String::new(),
            fragments: if author {
                vec![fragment.clone()]
            } else {
                vec![]
            },
            relevance: "unknown".into(),
            traits: vec![],
            topic_hints: vec![],
            analysis: Value::Null,
            analysis_state: "not_analyzed".into(),
            observation: Value::Null,
            profile: Value::Null,
            focus: Value::Null,
            author_analysis: Value::Null,
            acquisition_kind: "synthetic".into(),
        },
        fragments: vec![fragment],
        topics: json!([]),
        domain: json!({"domainRef":Uuid::from_u128(3)}),
        hash: "synthetic-window".into(),
        context_work_refs: vec![],
        comment_study: json!([]),
        role_metadata: json!({"workRef":work_ref,"own":false}),
        coverage: json!({"sourceChars":text.chars().count(),"windowCount":1,"windowKey":"synthetic-window"}),
    }
}

fn draft(input: &ResearchInput) -> ResearchOutput {
    let source = &input.fragments[0];
    let evidence = vec![Citation {
        fragment_id: source.fragment_id.clone(),
        start: source.start,
        end: source.end,
    }];
    let discussions: Vec<_> = ["我一直难以开始任务", "开始以后又容易分心"]
        .iter()
        .map(|statement| Discussion {
            label: (*statement).into(),
            topic_ref: None,
            evidence: evidence.clone(),
            statement: (*statement).into(),
            definition: format!("材料中描述的困难：{statement}"),
            inclusion_criteria: vec!["存在明确任务困难的陈述".into()],
            exclusion_criteria: vec!["只讨论与任务无关的内容".into()],
            speaker_role: if input.work.fragments.is_empty() {
                "commenter"
            } else {
                "author"
            }
            .into(),
            evidence_role: "support".into(),
            rationale: "原文直接陈述该困难。".into(),
        })
        .collect();
    serde_json::from_value(json!({"contract":"topic-map.research.v2","outcome":"analyzed","discussions":discussions,
        "journey":{"mainStage":"begin_practice","involvedStages":["begin_practice"],"overlays":[],"path":"adult",
            "rationale":"原文描述尝试开展任务。","evidence":evidence},
        "scenes":[{"label":"执行任务","evidence":evidence}],"responseMatches":[],
        "angles":[{"label":"任务困难","title":"尚未接纳的草稿角度","answerTask":"说明如何开始","evidence":evidence}],
        "productOpportunities":[],"limitations":["合成材料，只验证读取逻辑。"]})).unwrap()
}

fn accepted(input: &ResearchInput, draft: &ResearchOutput, index: usize, topic: u128) -> Value {
    let (id, discussion) = &crate::topic_map_core::units(input, draft)[index];
    json!({"unitId":id,"unitRef":Uuid::parse_str(&id[..32]).unwrap(),"resolutionRef":Uuid::from_u128(20+index as u128),
        "label":discussion.label,"statement":discussion.statement,"speakerRole":discussion.speaker_role,
        "evidenceRole":discussion.evidence_role,"rationale":discussion.rationale,"evidence":discussion.evidence,
        "status":"matched","reason":"原文满足纳入标准。","assignments":[assignment(topic)],"relations":[],
        "comparedDefinitionRefs":[Uuid::from_u128(topic+1)]})
}

fn assignment(topic: u128) -> Value {
    json!({"topicRef":Uuid::from_u128(topic),"definitionRef":Uuid::from_u128(topic+1),"label":"同名主题","reason":"逐项符合概念边界。"})
}

fn window(
    input: ResearchInput,
    output: Value,
    core: Value,
    result: Option<u128>,
    time: &str,
) -> Window {
    Window {
        manifest: json!({"coverage":input.coverage}),
        input,
        output,
        core,
        result: result.map(Uuid::from_u128),
        task: Uuid::from_u128(result.unwrap_or(99)),
        method: "topic-map.research.v2".into(),
        time: time.into(),
    }
}
