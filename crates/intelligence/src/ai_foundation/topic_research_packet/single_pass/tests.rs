use super::*;
use crate::topic_map_research_analysis::{Citation, Discussion, ResearchOutput};
use linggan_evidence::creator_discovery::{DiscoveryWork, Fragment};
use uuid::Uuid;

fn fragment(id: &str, text: &str) -> Fragment {
    Fragment {
        fragment_id: id.into(),
        source_ref: Uuid::from_u128(5),
        source_version: "synthetic-version".into(),
        field: "body".into(),
        start: 10_000,
        end: 10_000 + text.chars().count(),
        text: text.into(),
        speaker: "author".into(),
        ..Fragment::default()
    }
}

fn input(fragments: Vec<Fragment>) -> ResearchInput {
    let work_ref = Uuid::from_u128(1);
    ResearchInput {
        work: DiscoveryWork {
            reading_title: Value::Null,
            dossier_revision: String::new(),
            dossier_material_revision: String::new(),
            dossier_coverage: Value::Null,
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
            fragments: vec![],
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
        fragments,
        topics: json!([]),
        domain: json!({"domainRef":Uuid::from_u128(3)}),
        hash: "synthetic-window".into(),
        context_work_refs: vec![],
        comment_study: json!([]),
        role_metadata: json!({}),
        coverage: json!({"inputContract":crate::topic_map_research::CORE_INPUT_CONTRACT,"core":{}}),
    }
}

fn discussion(label: &str, statement: &str, ids: &[&str], fragments: &[Fragment]) -> Discussion {
    Discussion {
        label: label.into(),
        topic_ref: None,
        evidence: ids
            .iter()
            .map(|id| {
                let fragment = fragments
                    .iter()
                    .find(|fragment| fragment.fragment_id == *id)
                    .unwrap();
                Citation {
                    fragment_id: (*id).into(),
                    start: fragment.start,
                    end: fragment.end,
                }
            })
            .collect(),
        statement: statement.into(),
        definition: "知道要做却无法启动第一步".into(),
        inclusion_criteria: vec!["已决定要做但迟迟无法开始".into()],
        exclusion_criteria: vec!["已经开始后的持续分心".into()],
        speaker_role: "author".into(),
        evidence_role: "support".into(),
        rationale: "材料明确描述了启动前的停滞".into(),
    }
}

fn compact(fragments: &[Fragment], discussions: &[Discussion]) -> Value {
    let discussions = discussions
        .iter()
        .map(|discussion| {
            json!({
                "l":discussion.label,"s":discussion.statement,"d":discussion.definition,
                "i":discussion.inclusion_criteria,"x":discussion.exclusion_criteria,
                "sp":discussion.speaker_role,"er":discussion.evidence_role,"r":discussion.rationale,"t":Value::Null,
                "e":discussion.evidence.iter().map(|citation|{
                    let index = fragments.iter().position(|f|f.fragment_id==citation.fragment_id).unwrap();
                    let fragment = &fragments[index];
                    let quote: String = fragment.text.chars()
                        .skip(citation.start - fragment.start)
                        .take(citation.end - citation.start)
                        .collect();
                    json!({"f":index,"q":quote})
                }).collect::<Vec<_>>()
            })
        })
        .collect::<Vec<_>>();
    json!({"contract":"topic-map.extract.quotes.v1","o":"analyzed","d":discussions,"s":[],
        "j":{"m":"unclear","s":[],"o":[],"p":"unknown","r":"没有阶段依据","e":[]},
        "rm":[],"a":[],"p":[],"l":["合成测试"]})
}

fn new_topic(id: &str, parent: Value) -> Value {
    json!({"newTopicId":id,"label":"任务启动困难","definition":"知道要做却无法开始第一步",
        "inclusionCriteria":["已决定要做但迟迟无法开始"],"exclusionCriteria":["已经开始后的持续分心"],
        "domainFit":"in_scope","domainReason":"材料属于当前领域","abstractionReason":"可跨作品复用的启动问题",
        "parent":parent})
}

fn root_parent() -> Value {
    json!({"kind":"root","topicRef":Value::Null,"definitionRef":Value::Null,"newTopicId":Value::Null,"reason":"顶层问题"})
}

fn wire(projection: Value, new_topics: Vec<Value>, decisions: Vec<Value>) -> Value {
    json!({"contract":CONTRACT,"projection":projection,"newTopics":new_topics,"decisions":decisions})
}

fn author_context() -> Value {
    json!({"mode":"author","candidates":[],"recall":{"method":"complete_catalog","catalogTopics":0,"similarityIsMembership":false}})
}

#[test]
fn one_new_topic_definition_serves_multiple_units() {
    let fragments = vec![
        fragment("f1", "知道要做却开始不了"),
        fragment("f2", "计划了任务但无法启动"),
    ];
    let discussions = vec![
        discussion("启动一", "知道要做却开始不了", &["f1"], &fragments),
        discussion("启动二", "计划了任务但无法启动", &["f2"], &fragments),
    ];
    let payload = wire(
        compact(&fragments, &discussions),
        vec![new_topic("n1", root_parent())],
        vec![
            json!({"discussionIndex":0,"status":"new","matches":[],"newTopicId":"n1","relations":[],"reason":"独立启动问题"}),
            json!({"discussionIndex":1,"status":"new","matches":[],"newTopicId":"n1","relations":[],"reason":"同一启动问题"}),
        ],
    );
    let parsed = validate(
        &payload.to_string(),
        &input(fragments.clone()),
        &author_context(),
    )
    .unwrap();
    let decisions = parsed["resolution"]["decisions"].as_array().unwrap();
    assert_eq!(decisions.len(), 2);
    assert_eq!(decisions[0]["proposedTopic"], decisions[1]["proposedTopic"]);
    assert_eq!(decisions[0]["proposedTopic"]["label"], "任务启动困难");
    assert!(
        parsed["resolution"]["decisions"][0]
            .get("newTopicId")
            .is_none()
    );
}

#[test]
fn duplicate_projection_discussions_keep_original_indices_aligned() {
    let fragments = vec![
        fragment("f1", "知道要做却开始不了"),
        fragment("f2", "开始后总是分心"),
    ];
    let first = discussion("启动", "知道要做却开始不了", &["f1"], &fragments);
    let second = discussion("分心", "开始后总是分心", &["f2"], &fragments);
    let discussions = vec![first.clone(), second.clone(), first];
    let payload = wire(
        compact(&fragments, &discussions),
        vec![
            new_topic("n1", root_parent()),
            new_topic("n2", root_parent()),
        ],
        vec![
            json!({"discussionIndex":0,"status":"new","matches":[],"newTopicId":"n1","relations":[],"reason":"启动问题"}),
            json!({"discussionIndex":1,"status":"new","matches":[],"newTopicId":"n2","relations":[],"reason":"持续注意问题"}),
            json!({"discussionIndex":2,"status":"new","matches":[],"newTopicId":"n1","relations":[],"reason":"启动问题"}),
        ],
    );
    let parsed = validate(&payload.to_string(), &input(fragments), &author_context()).unwrap();
    let decisions = parsed["resolution"]["decisions"].as_array().unwrap();
    assert_eq!(decisions.len(), 2);
    assert_eq!(decisions[1]["reason"], "持续注意问题");
    assert_eq!(decisions[1]["proposedTopic"]["label"], "任务启动困难");
}

#[test]
fn contradictory_replies_for_the_same_unit_are_rejected() {
    let fragments = vec![fragment("f1", "知道要做却开始不了")];
    let discussions = vec![discussion(
        "启动",
        "知道要做却开始不了",
        &["f1"],
        &fragments,
    )];
    let payload = wire(
        compact(&fragments, &discussions),
        vec![
            new_topic("n1", root_parent()),
            new_topic("n2", root_parent()),
        ],
        vec![
            json!({"discussionIndex":0,"status":"new","matches":[],"newTopicId":"n1","relations":[],"reason":"启动问题"}),
            json!({"discussionIndex":0,"status":"new","matches":[],"newTopicId":"n2","relations":[],"reason":"另一个问题"}),
        ],
    );
    assert_eq!(
        validate(&payload.to_string(), &input(fragments), &author_context()).unwrap_err(),
        "resolution_unit_decision_conflict"
    );
}

#[test]
fn local_new_topic_references_must_exist_and_be_unique() {
    let fragments = vec![fragment("f1", "知道要做却开始不了")];
    let discussions = vec![discussion(
        "启动",
        "知道要做却开始不了",
        &["f1"],
        &fragments,
    )];
    let unknown = wire(
        compact(&fragments, &discussions),
        vec![new_topic("n1", root_parent())],
        vec![
            json!({"discussionIndex":0,"status":"new","matches":[],"newTopicId":"missing","relations":[],"reason":"启动问题"}),
        ],
    );
    assert_eq!(
        validate(
            &unknown.to_string(),
            &input(fragments.clone()),
            &author_context()
        )
        .unwrap_err(),
        "new_topic_reference_unknown"
    );
    let duplicate = wire(
        compact(&fragments, &discussions),
        vec![
            new_topic("n1", root_parent()),
            new_topic("n1", root_parent()),
        ],
        vec![
            json!({"discussionIndex":0,"status":"new","matches":[],"newTopicId":"n1","relations":[],"reason":"启动问题"}),
        ],
    );
    assert_eq!(
        validate(&duplicate.to_string(), &input(fragments), &author_context()).unwrap_err(),
        "new_topic_id_duplicate"
    );
}

#[test]
fn declared_existing_parent_adds_the_required_containment_relation() {
    let fragments = vec![fragment("f1", "知道要做却开始不了")];
    let discussions = vec![discussion(
        "启动",
        "知道要做却开始不了",
        &["f1"],
        &fragments,
    )];
    let topic = Uuid::from_u128(20);
    let definition = Uuid::from_u128(21);
    let parent = json!({"kind":"existing","topicRef":topic,"definitionRef":definition,"newTopicId":Value::Null,"reason":"属于执行困难"});
    let payload = wire(
        compact(&fragments, &discussions),
        vec![new_topic("n1", parent)],
        vec![
            json!({"discussionIndex":0,"status":"new","matches":[],"newTopicId":"n1","relations":[],"reason":"启动问题"}),
        ],
    );
    let context = json!({"mode":"author","candidates":[{"topicRef":topic,"definitionRef":definition}],
        "recall":{"method":"complete_catalog","catalogTopics":1,"similarityIsMembership":false}});
    let parsed = validate(&payload.to_string(), &input(fragments), &context).unwrap();
    let relations = parsed["resolution"]["decisions"][0]["relations"]
        .as_array()
        .unwrap();
    assert_eq!(relations.len(), 1);
    assert_eq!(relations[0]["relation"], "narrower");
    assert_eq!(relations[0]["topicRef"], json!(topic));
}

fn named_topic(id: &str, label: &str, definition: &str, parent: Value) -> Value {
    json!({"newTopicId":id,"label":label,"definition":definition,
        "inclusionCriteria":[definition],"exclusionCriteria":["不属于该主题"],
        "domainFit":"in_scope","domainReason":"材料属于当前领域","abstractionReason":"可跨作品复用的执行问题",
        "parent":parent})
}

#[test]
fn a_new_parent_referenced_only_by_its_child_keeps_its_own_parent() {
    let fragments = vec![fragment("f1", "知道要做却开始不了")];
    let discussions = vec![discussion(
        "启动",
        "知道要做却开始不了",
        &["f1"],
        &fragments,
    )];
    let existing = Uuid::from_u128(50);
    let existing_definition = Uuid::from_u128(51);
    let via_new = json!({"kind":"newTopic","topicRef":Value::Null,"definitionRef":Value::Null,
        "newTopicId":"n2","reason":"属于执行功能困难"});
    let via_existing = json!({"kind":"existing","topicRef":existing,"definitionRef":existing_definition,
        "newTopicId":Value::Null,"reason":"属于执行功能困难"});
    let payload = wire(
        compact(&fragments, &discussions),
        vec![
            new_topic("n1", via_new),
            named_topic("n2", "执行功能困难", "跨场景的执行功能不足", via_existing),
        ],
        vec![
            json!({"discussionIndex":0,"status":"new","matches":[],"newTopicId":"n1","relations":[],"reason":"启动问题"}),
        ],
    );
    let context = json!({"mode":"author","candidates":[{"topicRef":existing,"definitionRef":existing_definition}],
        "recall":{"method":"complete_catalog","catalogTopics":1,"similarityIsMembership":false}});
    let parsed = validate(&payload.to_string(), &input(fragments), &context).unwrap();
    let parent = &parsed["resolution"]["decisions"][0]["proposedTopic"]["parent"];
    assert_eq!(parent["kind"], "proposed");
    assert_eq!(parent["proposal"]["label"], "执行功能困难");
    let ancestor = &parent["ancestor"];
    assert_eq!(ancestor["kind"], "existing");
    assert_eq!(ancestor["topicRef"], json!(existing));
    assert_eq!(ancestor["definitionRef"], json!(existing_definition));
}

#[test]
fn a_parent_cycle_is_rejected_instead_of_recursing_forever() {
    let fragments = vec![fragment("f1", "知道要做却开始不了")];
    let discussions = vec![discussion(
        "启动",
        "知道要做却开始不了",
        &["f1"],
        &fragments,
    )];
    let to_n2 = json!({"kind":"newTopic","topicRef":Value::Null,"definitionRef":Value::Null,"newTopicId":"n2","reason":"上层"});
    let to_n1 = json!({"kind":"newTopic","topicRef":Value::Null,"definitionRef":Value::Null,"newTopicId":"n1","reason":"上层"});
    let payload = wire(
        compact(&fragments, &discussions),
        vec![
            new_topic("n1", to_n2),
            named_topic("n2", "执行功能困难", "跨场景的执行功能不足", to_n1),
        ],
        vec![
            json!({"discussionIndex":0,"status":"new","matches":[],"newTopicId":"n1","relations":[],"reason":"启动问题"}),
        ],
    );
    assert_eq!(
        validate(&payload.to_string(), &input(fragments), &author_context()).unwrap_err(),
        "new_topic_parent_cycle"
    );
}

#[test]
fn author_payload_carries_the_core_once_and_other_modes_carry_only_saved_units() {
    let fragments = vec![fragment("f1", "知道要做却开始不了")];
    let mut source = input(fragments.clone());
    source.coverage = json!({"inputContract":crate::topic_map_research::CORE_INPUT_CONTRACT,
        "semanticRequestRef":Uuid::from_u128(60),"core":{"marker":"only-one-copy","premises":["合成前提"]}});
    let definitions =
        vec![json!({"topicRef":Uuid::from_u128(61),"definitionRef":Uuid::from_u128(62)})];
    let author = input_payload("author", &source, &definitions, &[]);
    assert_eq!(author["contentCore"]["marker"], "only-one-copy");
    assert_eq!(
        author["coverage"]["inputContract"],
        crate::topic_map_research::CORE_INPUT_CONTRACT
    );
    assert!(author["coverage"].get("core").is_none());
    assert_eq!(author.to_string().matches("only-one-copy").count(), 1);

    let discussions = vec![discussion(
        "启动",
        "知道要做却开始不了",
        &["f1"],
        &fragments,
    )];
    let units = crate::topic_map_core::units_for_discussions(&source, &discussions);
    let reassignment = input_payload("reassignment", &source, &definitions, &units);
    let mut keys: Vec<_> = reassignment.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(keys, ["definitions", "domain", "mode", "units", "workRef"]);
    assert_eq!(reassignment["units"][0]["discussionIndex"], 0);
    assert_eq!(
        reassignment["units"][0]["discussion"]["statement"],
        "知道要做却开始不了"
    );
    assert!(reassignment.get("contentCore").is_none());
    assert!(reassignment.get("fragments").is_none());
    assert!(!reassignment.to_string().contains("only-one-copy"));
}

#[test]
fn comment_mode_cannot_create_topics() {
    let mut comment = fragment("c1", "需要提醒");
    comment.field = "studied_comment".into();
    comment.speaker = "commenter".into();
    comment.structure = json!({"signal":{"proposition":"需要提醒","kind":"need"}});
    let mut frozen = input(vec![comment]);
    frozen.coverage =
        json!({"inputContract":comments::CONTRACT,"semanticRequestRef":Uuid::from_u128(30)});
    let units =
        crate::topic_map_core::units_for_discussions(&frozen, &comments::discussions(&frozen));
    let topic = Uuid::from_u128(31);
    let definition = Uuid::from_u128(32);
    let context = json!({"mode":"comment","unitIds":[units[0].0],"candidates":[{"topicRef":topic,"definitionRef":definition}],
        "recall":{"method":"complete_catalog","catalogTopics":1,"similarityIsMembership":false}});
    let matched = wire(
        Value::Null,
        vec![],
        vec![json!({"discussionIndex":0,"status":"matched",
        "matches":[{"topicRef":topic,"definitionRef":definition,"reason":"同一提醒问题"}],
        "newTopicId":Value::Null,"relations":[],"reason":"已有主题"})],
    );
    let parsed = validate(&matched.to_string(), &frozen, &context).unwrap();
    assert_eq!(
        parsed["resolution"]["decisions"][0]["unitId"],
        json!(units[0].0)
    );

    let creates = wire(
        Value::Null,
        vec![new_topic("n1", root_parent())],
        vec![json!({"discussionIndex":0,
        "status":"new","matches":[],"newTopicId":"n1","relations":[],"reason":"评论需求"})],
    );
    assert_eq!(
        validate(&creates.to_string(), &frozen, &context).unwrap_err(),
        "comment_topic_creation_forbidden"
    );
    let creates_via_decision = wire(
        Value::Null,
        vec![],
        vec![json!({"discussionIndex":0,
        "status":"new","matches":[],"newTopicId":Value::Null,"relations":[],"reason":"评论需求"})],
    );
    assert_eq!(
        validate(&creates_via_decision.to_string(), &frozen, &context).unwrap_err(),
        "comment_topic_creation_forbidden"
    );
}

#[test]
fn reassignment_reuses_the_saved_projection_without_regenerating_it() {
    let fragments = vec![
        fragment("f1", "知道要做却开始不了"),
        fragment("f2", "开始后总是分心"),
    ];
    let first = discussion("启动", "知道要做却开始不了", &["f1"], &fragments);
    let second = discussion("分心", "开始后总是分心", &["f2"], &fragments);
    let discussions = vec![first.clone(), second.clone(), first];
    let projection: ResearchOutput = serde_json::from_value(json!({
        "contract":"topic-map.extract.quotes.v1","outcome":"analyzed","discussions":discussions,
        "scenes":[],"journey":{"mainStage":"unclear","involvedStages":[],"overlays":[],"path":"unknown","rationale":"没有阶段依据","evidence":[]},
        "responseMatches":[],"angles":[],"productOpportunities":[],"limitations":["合成测试"]}))
    .unwrap();
    let units = crate::topic_map_core::units_for_discussions(
        &input(fragments.clone()),
        &projection.discussions,
    );
    assert_eq!(units.len(), 2);
    let topic = Uuid::from_u128(40);
    let definition = Uuid::from_u128(41);
    let context = json!({"mode":"reassignment","projection":projection,"unitIds":[units[0].0,units[1].0],
        "candidates":[{"topicRef":topic,"definitionRef":definition}],
        "recall":{"method":"complete_catalog","catalogTopics":1,"similarityIsMembership":false}});
    let payload = wire(
        Value::Null,
        vec![],
        vec![
            json!({"discussionIndex":0,"status":"matched",
            "matches":[{"topicRef":topic,"definitionRef":definition,"reason":"定义仍然相符"}],
            "newTopicId":Value::Null,"relations":[],"reason":"复用已保存讨论"}),
            json!({"discussionIndex":1,"status":"matched",
            "matches":[{"topicRef":topic,"definitionRef":definition,"reason":"定义仍然相符"}],
            "newTopicId":Value::Null,"relations":[],"reason":"复用已保存讨论"}),
        ],
    );
    let parsed = validate(&payload.to_string(), &input(fragments), &context).unwrap();
    assert!(parsed["projection"].is_object());
    let decisions = parsed["resolution"]["decisions"].as_array().unwrap();
    assert_eq!(decisions.len(), 2);
    assert_eq!(decisions[0]["unitId"], json!(units[0].0));
    assert_eq!(decisions[1]["unitId"], json!(units[1].0));
    assert_eq!(decisions[1]["status"], "matched");
}

fn chain_topic(id: &str, label: &str, definition: &str, inclusion: &str, exclusion: &str, parent: Value) -> Value {
    json!({"newTopicId":id,"label":label,"definition":definition,
        "inclusionCriteria":[inclusion],"exclusionCriteria":[exclusion],
        "domainFit":"in_scope","domainReason":"材料属于当前领域","abstractionReason":"可跨作品复用的执行问题",
        "parent":parent})
}

fn new_topic_parent(id: &str) -> Value {
    json!({"kind":"newTopic","topicRef":Value::Null,"definitionRef":Value::Null,
        "newTopicId":id,"reason":"上层执行范围"})
}

#[test]
fn a_three_layer_local_chain_is_accepted_with_every_parent_edge() {
    let fragments = vec![fragment("f1", "知道要做却开始不了")];
    let discussions = vec![discussion("启动", "知道要做却开始不了", &["f1"], &fragments)];
    let payload = wire(
        compact(&fragments, &discussions),
        vec![
            new_topic("n1", new_topic_parent("n2")),
            chain_topic(
                "n2",
                "任务执行支持",
                "帮助开始并维持已选定任务的支持过程",
                "任务开始或执行中的具体障碍与支持",
                "仅讨论选择哪些任务",
                new_topic_parent("n3"),
            ),
            chain_topic(
                "n3",
                "执行支持范围",
                "覆盖多类执行困难的更广支持范围",
                "执行中的障碍",
                "选择任务",
                root_parent(),
            ),
        ],
        vec![json!({"discussionIndex":0,"status":"new","matches":[],"newTopicId":"n1","relations":[],"reason":"启动问题"})],
    );
    let parsed = validate(&payload.to_string(), &input(fragments), &author_context()).unwrap();
    let parent = &parsed["resolution"]["decisions"][0]["proposedTopic"]["parent"];
    assert_eq!(parent["proposal"]["label"], "任务执行支持");
    assert_eq!(parent["ancestor"]["proposal"]["label"], "执行支持范围");
    assert_eq!(parent["ancestor"]["ancestor"], Value::Null);
}

#[test]
fn an_unreferenced_grandparent_repeating_criteria_is_rejected() {
    let fragments = vec![fragment("f1", "知道要做却开始不了")];
    let discussions = vec![discussion("启动", "知道要做却开始不了", &["f1"], &fragments)];
    let payload = wire(
        compact(&fragments, &discussions),
        vec![
            new_topic("n1", new_topic_parent("n2")),
            chain_topic(
                "n2",
                "任务执行支持",
                "帮助开始并维持已选定任务的支持过程",
                "任务开始或执行中的具体障碍与支持",
                "仅讨论选择哪些任务",
                new_topic_parent("n3"),
            ),
            chain_topic(
                "n3",
                "执行支持范围",
                "覆盖多类执行困难的更广支持范围",
                "任务开始或执行中的具体障碍与支持",
                "仅讨论选择哪些任务",
                root_parent(),
            ),
        ],
        vec![json!({"discussionIndex":0,"status":"new","matches":[],"newTopicId":"n1","relations":[],"reason":"启动问题"})],
    );
    assert_eq!(
        validate(&payload.to_string(), &input(fragments), &author_context()).unwrap_err(),
        "topic_parent_criteria_repeated"
    );
}
