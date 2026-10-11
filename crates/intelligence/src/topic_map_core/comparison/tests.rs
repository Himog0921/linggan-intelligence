use super::*;
use linggan_evidence::creator_discovery::{self, DiscoveryWork};

fn topics() -> Value {
    json!([
        {"topicRef":Uuid::from_u128(100),"definitionRef":Uuid::from_u128(101),"label":"同名讨论"},
        {"topicRef":Uuid::from_u128(200),"definitionRef":Uuid::from_u128(201),"label":"同名讨论"}
    ])
}

fn source(work: u128, id: u128, field: &str, text: &str) -> Fragment {
    let work = Uuid::from_u128(work);
    let source_ref = Uuid::from_u128(id);
    Fragment {
        fragment_id: format!("{work}.{field}.{source_ref}"),
        source_ref,
        field: field.into(),
        source_version: creator_discovery::hash(text),
        start: 0,
        end: text.chars().count(),
        text: text.into(),
    }
}

fn input(work: u128, fragments: Vec<Fragment>) -> ResearchInput {
    let work_ref = Uuid::from_u128(work);
    let comments: Vec<_> = fragments.iter().filter(|f|f.field=="unresearched_comment").map(|comment|json!({
        "workRef":work_ref,"sourceRef":comment.source_ref,"fragmentId":comment.fragment_id,
        "sourceFragmentId":comment.fragment_id,"role":"eligible_user_comment","contextOnly":false,
        "researchState":"accepted","observationRole":"primary","parentFragmentId":null,
        "parentSourceRef":null,"parentContextOnly":true,"signals":[]
    })).collect();
    let mut input = ResearchInput {
        work: DiscoveryWork {
            work_ref,
            usage_roles: vec!["primary".into()],
            creator_key: None,
            platform: if work == 1 { "xhs" } else { "douyin" }.into(),
            author_external_id: Some(format!("synthetic-author-{work}")),
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
            fragments: fragments
                .iter()
                .filter(|f| matches!(f.field.as_str(), "title" | "body" | "ocr" | "transcript"))
                .cloned()
                .collect(),
            relevance: "unknown".into(),
            traits: Vec::new(),
            topic_hints: Vec::new(),
            analysis: Value::Null,
            analysis_state: "not_analyzed".into(),
            observation: Value::Null,
            profile: Value::Null,
            focus: Value::Null,
            author_analysis: Value::Null,
            acquisition_kind: "synthetic".into(),
        },
        fragments,
        topics: topics(),
        domain: json!({"domainRef":Uuid::from_u128(30),"name":"合成领域","description":"仅测试","researchGoal":"比较讨论边界"}),
        hash: String::new(),
        context_work_refs: Vec::new(),
        comment_study: json!(comments),
        role_metadata: json!({"workRef":work_ref,"platform":if work==1{"xhs"}else{"douyin"},"usageRoles":["primary"],
            "own":work==1,"ownScopeConfigured":true,"manualOwnBreakout":false,"likes":work*10,
            "likesObservedAt":"2026-01-01T00:00:00Z","followerCount":null,"publishedAt":null}),
        coverage: json!({"inputContract":"topic-map.source-windows.v1","configRef":Uuid::from_u128(40)}),
    };
    input.comment_study = research::selected_comment_study(&input, &input.fragments, &json!({}));
    input.hash = research::input_identity(&input);
    input
}

fn assignment(topic: u128) -> Value {
    json!({"topicRef":Uuid::from_u128(topic),"definitionRef":Uuid::from_u128(topic+1),
        "label":"同名讨论","reason":"原文符合当前边界"})
}

fn unit(
    id: &str,
    fragment: &Fragment,
    start: usize,
    end: usize,
    speaker: &str,
    role: &str,
    topic: u128,
) -> Value {
    json!({"unitId":id,"label":"同名讨论","statement":format!("合成讨论{id}的陈述"),"status":"matched",
        "speakerRole":speaker,"evidenceRole":role,"assignments":[assignment(topic)],
        "evidence":[{"fragmentId":fragment.fragment_id,"start":start,"end":end}]})
}

fn task(window: &ResearchInput, task: u128, units: Vec<Value>) -> TaskEvidence {
    TaskEvidence {
        task: Uuid::from_u128(task),
        work: window.work.work_ref,
        manifest: research::reference_manifest(window),
        resolutions: json!(units),
    }
}

fn pair() -> (Vec<ResearchInput>, Vec<TaskEvidence>, Vec<Uuid>) {
    let a = input(1, vec![source(1, 11, "body", "作者第一篇观点与边界")]);
    let b = input(
        2,
        vec![source(
            2,
            22,
            "unresearched_comment",
            "评论者提供具体不同经历",
        )],
    );
    let tasks = vec![
        task(
            &a,
            1001,
            vec![unit("a", &a.fragments[0], 0, 5, "author", "support", 100)],
        ),
        task(
            &b,
            1002,
            vec![unit(
                "b",
                &b.fragments[0],
                0,
                6,
                "commenter",
                "challenge",
                100,
            )],
        ),
    ];
    let requested = vec![a.work.work_ref, b.work.work_ref];
    (vec![a, b], tasks, requested)
}

#[path = "tests/scheduling.rs"]
mod scheduling_tests;
#[path = "tests/selection.rs"]
mod selection_tests;
#[path = "tests/snapshot.rs"]
mod snapshot_tests;
#[path = "tests/sources.rs"]
mod source_tests;

#[path = "tests/context.rs"]
mod context_tests;
