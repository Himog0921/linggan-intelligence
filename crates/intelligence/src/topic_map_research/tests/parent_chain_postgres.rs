//! Isolated PostgreSQL proof that a new parent referenced only through its child
//! still persists its own parent. Synthetic decisions only; no provider or worker.
#[path = "saved_source_fixture.rs"]
mod support;
use crate::topic_map::{self, TopicMapCandidateRequest, TopicMapCitation};
use crate::topic_map_research as research;
use crate::topic_map_research_analysis as analysis;
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use support::*;
use uuid::Uuid;

async fn latest_parent(db: &Database, label: &str) -> (Uuid, i32, Option<Uuid>) {
    sqlx::query_as("SELECT d.topic_ref,b.version,b.parent_topic_ref FROM linggan_topic_definition d JOIN LATERAL(SELECT version,parent_topic_ref FROM linggan_topic_map_binding WHERE topic_ref=d.topic_ref ORDER BY version DESC LIMIT 1)b ON true WHERE d.display_name=$1 ORDER BY d.created_at,d.topic_ref LIMIT 1")
        .bind(label)
        .fetch_one(db.pool())
        .await
        .unwrap()
}

#[tokio::test]
#[ignore = "isolated native PostgreSQL; synthetic accepted records, no provider or worker"]
async fn a_parent_referenced_only_by_its_child_persists_its_own_parent() {
    let (db, config) = setup("topic_parent_chain").await;
    let work = work(&db, "parent-chain", "知道要做却无法开始，反复如此。").await;
    seed_research_inputs(&db, config, &[work]).await;
    let (task, manifest): (Uuid, Value) = sqlx::query_as(
        "SELECT window_ref,input_refs FROM linggan_topic_research_window WHERE work_ref=$1 ORDER BY created_at,window_ref LIMIT 1",
    )
    .bind(work)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let inputs = research::load_inputs_for_works_at(
        &db,
        D,
        config,
        &[work],
        research::manifest_as_of(&manifest).unwrap(),
    )
    .await
    .unwrap();
    let input = research::restore_scoped_window(&inputs, work, &manifest)
        .expect("canonical preparation restores the frozen window");
    let primary = input
        .fragments
        .iter()
        .find(|fragment| research::is_research_evidence(fragment))
        .unwrap();
    let citation = analysis::Citation {
        fragment_id: primary.fragment_id.clone(),
        start: primary.start,
        end: primary.end,
    };
    let invocation = Uuid::new_v4();
    let request_hash =
        linggan_evidence::creator_discovery::hash(&format!("synthetic:{invocation}"));
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash,state,reserved_tokens,charged_tokens,result,finished_at) SELECT $1,model.connection_version_ref,model.model_ref,$2,'analyze',$3,'succeeded',0,0,$4,scope_001_now() FROM linggan_model_config config JOIN linggan_model_entry model USING(model_ref) WHERE config.config_ref=$2")
        .bind(invocation).bind(config).bind(&request_hash)
        .bind(json!({"synthetic":true,"fixture":"SYNTHETIC / NOT EVIDENCE; no provider call"}))
        .execute(db.pool()).await.unwrap();
    // Existing upper topic C: A -> new B -> existing C.
    let mut tx = db.pool().begin().await.unwrap();
    let existing = topic_map::accept_topic_map_candidates_in(
        &mut tx,
        D,
        work,
        &[TopicMapCandidateRequest {
            is_parent: true,
            label: "合成自我调节困难".into(),
            topic_ref: None,
            evidence_citations: vec![TopicMapCitation {
                fragment_id: primary.fragment_id.clone(),
                source_ref: primary.source_ref,
                field: primary.field.clone(),
            }],
            evidence_role: "support".into(),
            definition_text: "跨场景的自我调节资源不足，影响计划、启动与持续".into(),
            inclusion_criteria: vec!["跨场景的自我调节资源不足".into()],
            exclusion_criteria: vec!["只涉及单一场景的技能不足".into()],
            invocation_ref: invocation,
            reuse_definition_refs: vec![],
        }],
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let existing_topic = existing[0];
    let existing_definition: Uuid = sqlx::query_scalar(
        "SELECT definition_ref FROM linggan_topic_definition WHERE topic_ref=$1 ORDER BY version DESC LIMIT 1",
    )
    .bind(existing_topic)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let unit_id = "a".repeat(64);
    let discussion = analysis::Discussion {
        label: "合成启动困难".into(),
        topic_ref: None,
        evidence: vec![citation],
        statement: "知道要做却无法开始".into(),
        definition: "知道要做却无法开始第一步".into(),
        inclusion_criteria: vec!["已决定要做但迟迟无法开始".into()],
        exclusion_criteria: vec!["已经开始后的持续分心".into()],
        speaker_role: "author".into(),
        evidence_role: "support".into(),
        rationale: "SYNTHETIC / NOT EVIDENCE".into(),
    };
    let units = vec![
        (unit_id.clone(), discussion.clone()),
        ("b".repeat(64), discussion),
    ];
    let decision = json!({
            "unitId":unit_id,"status":"new","matches":[],
            "proposedTopic":{"label":"合成启动困难","definition":"知道要做却无法开始第一步",
                "inclusionCriteria":["已决定要做但迟迟无法开始"],
                "exclusionCriteria":["已经开始后的持续分心"],
                "domainFit":"in_scope","domainReason":"SYNTHETIC / NOT EVIDENCE",
                "abstractionReason":"合成父级链持久化测试",
                "parent":{"kind":"proposed","topicRef":null,"definitionRef":null,
                    "proposal":{"label":"合成执行功能困难","definition":"跨场景的执行功能不足，影响启动与持续",
                        "inclusionCriteria":["跨场景的执行功能不足"],
                        "exclusionCriteria":["只涉及单一场景的技能不足"],
                        "domainFit":"in_scope","domainReason":"SYNTHETIC / NOT EVIDENCE",
                        "abstractionReason":"合成父级链持久化测试"},
                    "ancestor":{"kind":"existing","topicRef":existing_topic,
                        "definitionRef":existing_definition,"proposal":null,
                        "reason":"上层自我调节困难"},
                    "reason":"属于执行功能困难"}},
            "relations":[],"reason":"SYNTHETIC parent chain fixture"});
    let mut repeated = decision.clone();
    repeated["unitId"] = json!("b".repeat(64));
    let resolution: analysis::ResolutionOutput = serde_json::from_value(json!({
        "contract":analysis::RESOLVE_CONTRACT,
        "decisions":[decision,repeated]}))
    .unwrap();
    let candidates = json!([{"topicRef":existing_topic,"definitionRef":existing_definition}]);
    analysis::validate_resolution(&resolution, &units, &candidates).unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    let accepted = crate::topic_map_core::accept_in(
        &mut tx,
        crate::topic_map_core::Acceptance {
            input: &input,
            task,
            invocation,
            units: &units,
            resolution: &resolution,
            candidates: &candidates,
            recall: &json!({"synthetic": true}),
            semantic_verification: &json!({"contract":analysis::verification::VERIFY_CONTRACT,"passed":true,"fixture":"SYNTHETIC acceptance, not semantic review"}),
            eligible_definitions: &[existing_definition],
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let (child, child_version, child_parent) = latest_parent(&db, "合成启动困难").await;
    let (parent, parent_version, parent_parent) = latest_parent(&db, "合成执行功能困难").await;
    let (upper, upper_version, upper_parent) = latest_parent(&db, "合成自我调节困难").await;
    assert!(child != parent && parent != upper && child != upper);
    assert_eq!(upper, existing_topic);
    assert_eq!((child_version, child_parent), (2, Some(parent)));
    assert_eq!((parent_version, parent_parent), (2, Some(existing_topic)));
    assert_eq!((upper_version, upper_parent), (1, None));
    assert_eq!(accepted[0]["hierarchy"]["parentTopicRef"], json!(parent));
    assert_eq!(accepted[1]["hierarchy"]["parentTopicRef"], json!(parent));
}
