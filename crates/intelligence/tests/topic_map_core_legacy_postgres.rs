//! Native PostgreSQL compatibility proofs using the actual pre-0120 candidate shape.
//! Synthetic source text and a local deterministic adapter; no provider requests.
#[path = "support/topic_map_core_fixture.rs"]
mod core_fixture;
use core_fixture::*;
use linggan_intelligence::{
    TopicMaterialMemberImport, TopicMaterialRole, TopicWorkspaceImport, import_topic_workspace,
    topic_map::{self, TopicMapCommand, TopicMapQuery},
    topic_map_research::apply_research_command,
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use uuid::Uuid;

const PRIVATE_LABEL: &str = "SYNTHETIC_LEGACY_PRIVATE_LABEL";
const PRIVATE_DEFINITION: &str = "SYNTHETIC_LEGACY_PRIVATE_DEFINITION";
const MANUAL_LABEL: &str = "人工裁定的合成实践启动";
const PRIVATE_ANGLE: &str = "SYNTHETIC_LEGACY_DERIVED_PRIVATE_ANGLE";
const VISIBLE_ANGLE: &str = "SYNTHETIC_LEGACY_MANUAL_VISIBLE_ANGLE";

fn query() -> TopicMapQuery {
    TopicMapQuery {
        domain_ref: Some(D),
        reference_window_days: Some(0),
        ..Default::default()
    }
}

async fn legacy_machine_candidate(db: &Database, work: Uuid) -> (Uuid, Uuid) {
    // Mirrors a88ff387 topic_map/store.rs::accept_topic_map_candidates_in:
    // workspace -> definition -> machine classification -> pack/member -> map
    // receipt/binding/membership. That code recorded no creation invocation.
    let (topic, definition, classification, receipt) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    let (source, body): (Uuid, String) = sqlx::query_as(
        "SELECT material_ref,body_text FROM linggan_material_content_detail WHERE content_public_ref=$1",
    )
    .bind(work)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let fragment = linggan_evidence::creator_discovery::fragment(
        work,
        "body",
        Some(&body),
        Some(source),
        None,
    )
    .unwrap();
    let hash = linggan_evidence::creator_discovery::hash(&format!("{D}:{PRIVATE_LABEL}"));
    let mut tx = db.pool().begin().await.unwrap();
    sqlx::query(
        "INSERT INTO linggan_topic_workspace(topic_ref,domain_key,canonical_key)VALUES($1,$2,$3)",
    )
    .bind(topic)
    .bind(format!("domain-{}", D.simple()))
    .bind(format!("candidate-{}", &hash[..24]))
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query("INSERT INTO linggan_topic_definition(definition_ref,topic_ref,version,display_name,definition_text,lifecycle_state)VALUES($1,$2,1,$3,$4,'candidate')")
        .bind(definition).bind(topic).bind(PRIVATE_LABEL)
        .bind(format!("依据已取得材料提出的讨论候选：{PRIVATE_DEFINITION}。尚未经人工定义裁定。"))
        .execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_topic_classification_run(classification_run_ref,definition_ref,run_kind,run_state,adjudication_note)VALUES($1,$2,'machine_proposed','completed','引用有效材料的机器候选；未作为人工裁定或正式定义发布。')")
        .bind(classification).bind(definition).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_topic_material_pack(material_pack_ref,classification_run_ref,source_boundary)VALUES($1,$2,'仅当前已接纳作品；少量样本可形成候选，不能外推市场。')")
        .bind(Uuid::new_v4()).bind(classification).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_topic_material_member(classification_run_ref,work_public_ref,role,rationale,ordinal)VALUES($1,$2,'support','本次接受的源片段支持此讨论候选，详见机器分类引用。',1)")
        .bind(classification).bind(work).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_topic_map_receipt(receipt_ref,idempotency_key,request_sha256,action,subject_ref,revision)VALUES($1,$2,$3,'candidate',$4,1)")
        .bind(receipt).bind(format!("topic-map-candidate:{hash}"))
        .bind(&hash).bind(topic).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_topic_map_binding(binding_ref,topic_ref,domain_ref,parent_topic_ref,version,receipt_ref)VALUES($1,$2,$3,NULL,1,$4)")
        .bind(Uuid::new_v4()).bind(topic).bind(D).bind(receipt).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_topic_map_membership(membership_ref,domain_ref,topic_ref,definition_ref,work_public_ref,method_version,evidence_citations)VALUES($1,$2,$3,$4,$5,'topic-map.research.v1.1',$6)")
        .bind(Uuid::new_v4()).bind(D).bind(topic).bind(definition).bind(work)
        .bind(json!([{"fragmentId":fragment.fragment_id,"sourceRef":source,"field":"body"}]))
        .execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    (topic, definition)
}

async fn manual_definition(db: &Database, support: Uuid, boundary: Uuid) -> (Uuid, Uuid) {
    let imported = import_topic_workspace(
        db,
        &TopicWorkspaceImport {
            idempotency_key: "legacy-proof:manual-import".into(),
            domain_key: "adhd-family".into(),
            canonical_key: "legacy-proof-manual-practice".into(),
            display_name: MANUAL_LABEL.into(),
            definition_text: "人工裁定的合成定义：开始练习的步骤、安排与困难；排除长期效果。"
                .into(),
            expected_version: None,
            adjudication_note: "SYNTHETIC 人工裁定对照；不表示真实研究".into(),
            source_boundary: "合成已接纳作品与明确边界材料".into(),
            members: vec![
                TopicMaterialMemberImport {
                    work_public_ref: support,
                    role: TopicMaterialRole::Support,
                    rationale: "合成开始练习材料".into(),
                },
                TopicMaterialMemberImport {
                    work_public_ref: boundary,
                    role: TopicMaterialRole::Boundary,
                    rationale: "合成长期效果边界材料".into(),
                },
            ],
        },
    )
    .await
    .unwrap();
    topic_map::save_topic_map_command(
        db,
        &TopicMapCommand::BindTopic {
            idempotency_key: "legacy-proof:manual-bind".into(),
            domain_ref: D,
            topic_ref: imported.topic_ref,
            parent_topic_ref: None,
            expected_version: None,
        },
    )
    .await
    .unwrap();
    (imported.topic_ref, imported.definition_ref)
}

#[tokio::test]
#[ignore = "isolated native PostgreSQL; synthetic adapter only"]
async fn unverifiable_legacy_machine_definition_is_hidden_and_excluded_from_future_prompts() {
    let (db, config, adapter) = setup("topic_core_legacy_qualification").await;
    let source = work(
        &db,
        "legacy-source",
        "SYNTHETIC 已取得的正文：开始练习需要拆解步骤。",
    )
    .await;
    let boundary = work(
        &db,
        "legacy-boundary",
        "SYNTHETIC 只观察已开始练习之后的长期效果。",
    )
    .await;
    let (machine_topic, machine_definition) = legacy_machine_candidate(&db, source).await;
    let (manual_topic, manual_definition) = manual_definition(&db, source, boundary).await;
    let rules: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_topic_map_concept_rule WHERE definition_ref=ANY($1)",
    )
    .bind(vec![machine_definition, manual_definition])
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        rules, 0,
        "no invented 0120 creation lineage for either definition"
    );
    let kinds: Vec<String> = sqlx::query_scalar("SELECT run_kind FROM linggan_topic_classification_run WHERE definition_ref=ANY($1) ORDER BY run_kind")
        .bind(vec![machine_definition, manual_definition]).fetch_all(db.pool()).await.unwrap();
    assert_eq!(kinds, vec!["human_adjudicated", "machine_proposed"]);

    let snapshot = topic_map::read_topic_map(&db, &query()).await.unwrap();
    let machine = snapshot
        .topics
        .iter()
        .find(|t| t.topic_ref == machine_topic)
        .unwrap();
    assert_eq!(machine.core["sourceState"], "source_unavailable");
    assert!(machine.direct_work_refs.is_empty());
    let manual = snapshot
        .topics
        .iter()
        .find(|t| t.topic_ref == manual_topic)
        .unwrap();
    assert_eq!(manual.display_name, MANUAL_LABEL);
    assert!(manual.definition_text.contains("人工裁定的合成定义"));
    assert_eq!(manual.core["sourceState"], "available");
    assert!(snapshot.works.iter().any(|w| w.work_ref == source));
    let visible = serde_json::to_string(&snapshot).unwrap();
    assert!(!visible.contains(PRIVATE_LABEL));
    assert!(!visible.contains(PRIVATE_DEFINITION));

    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    let next = work(
        &db,
        "legacy-future-work",
        "SYNTHETIC 开始练习前我先安排具体步骤。",
    )
    .await;
    apply_research_command(&db, &start(vec![next]))
        .await
        .unwrap();
    finish_pending(&db, &adapter).await;
    let manifests: Vec<Value> = sqlx::query_scalar("SELECT q.request_manifest FROM linggan_topic_map_research_request q JOIN linggan_topic_map_research_task t USING(task_ref) WHERE t.work_public_ref=$1 AND q.phase='resolve' AND q.dispatch_started_at IS NOT NULL")
        .bind(next).fetch_all(db.pool()).await.unwrap();
    assert!(
        !manifests.is_empty(),
        "a real synthetic resolve dispatch must have occurred"
    );
    let serialized = serde_json::to_string(&manifests).unwrap();
    assert!(!serialized.contains(PRIVATE_LABEL));
    assert!(!serialized.contains(PRIVATE_DEFINITION));
    assert!(!serialized.contains(&machine_definition.to_string()));
    assert!(
        manifests
            .iter()
            .any(|m| m["topics"].as_array().is_some_and(|topics| topics
                .iter()
                .any(|t| t["definitionRef"] == json!(manual_definition)))),
        "an eligible, relevant manual definition remains in actual resolution candidates"
    );
}

fn legacy_output(
    fragment: &linggan_evidence::creator_discovery::Fragment,
    topic: Uuid,
    restricted: bool,
) -> Value {
    let citation =
        json!([{"fragmentId":fragment.fragment_id,"start":fragment.start,"end":fragment.end}]);
    let label = if restricted {
        PRIVATE_ANGLE
    } else {
        VISIBLE_ANGLE
    };
    json!({
        "contract":"topic-map.research.v1","outcome":"analyzed",
        "discussions":[{"label":"合成开始练习讨论","topicRef":topic,"evidence":citation}],
        "scenes":[{"label":"合成练习场景","evidence":citation}],
        "journey":{"mainStage":"begin_practice","involvedStages":["begin_practice"],
            "overlays":[],"path":"unknown","rationale":"合成材料提及开始练习，身份未知。","evidence":citation},
        "responseMatches":[],
        "angles":[{"label":label,"title":label,"answerTask":"解释合成练习的开始步骤","evidence":citation}],
        "productOpportunities":[],"limitations":["SYNTHETIC / NOT EVIDENCE"]
    })
}

async fn definition_reference(db: &Database, definition: Uuid) -> Value {
    sqlx::query_scalar("SELECT jsonb_build_object('topicRef',topic_ref,'definitionRef',definition_ref,'version',version,'label',display_name,'definition',definition_text) FROM linggan_topic_definition WHERE definition_ref=$1")
        .bind(definition).fetch_one(db.pool()).await.unwrap()
}

async fn legacy_result(
    db: &Database,
    config: Uuid,
    source: (Uuid, &str),
    manual: (Uuid, Uuid),
    dependency: Option<(Uuid, Uuid)>,
    wrapped: bool,
) -> Uuid {
    let (work, body) = source;
    let (material, version): (Uuid, String) = sqlx::query_as(
        "SELECT material_ref,linggan_human_moment(created_at) FROM linggan_material_content_detail WHERE content_public_ref=$1",
    )
    .bind(work)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let fragment = linggan_evidence::creator_discovery::fragment(
        work,
        "body",
        Some(body),
        Some(material),
        Some(&version),
    )
    .unwrap();
    let (run, task, invocation, result) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    let manual_ref = definition_reference(db, manual.1).await;
    let mut manifest = json!({"workRef":work,"contextWorkRefs":[],"domain":D,
        "methodVersion":"topic-map.research.v1.1","topics":[manual_ref],
        "commentStudy":[],"roleMetadata":{"workRef":work,"usageRoles":["primary"],"own":false},
        "fragments":[{"fragmentId":fragment.fragment_id,"sourceRef":fragment.source_ref,
            "sourceVersion":fragment.source_version,"field":fragment.field,
            "start":fragment.start,"end":fragment.end,"textHash":linggan_evidence::creator_discovery::hash(&fragment.text)}]});
    let restricted_topic = match dependency {
        Some((_, definition)) => Some(definition_reference(db, definition).await),
        None => None,
    };
    // Base v1 stored its source manifest directly. Exercise that exact shape and
    // the supported source wrapper, with its dependency only outside `source`.
    let request = if wrapped {
        json!({"source":manifest,"topics":restricted_topic.into_iter().collect::<Vec<_>>()})
    } else {
        manifest["topics"]
            .as_array_mut()
            .unwrap()
            .extend(restricted_topic);
        manifest.clone()
    };
    let hash = linggan_evidence::creator_discovery::hash(&request.to_string());
    let output = legacy_output(&fragment, manual.0, dependency.is_some());
    let mut tx = db.pool().begin().await.unwrap();
    sqlx::query("INSERT INTO linggan_topic_map_research_run(run_ref,domain_ref,request_ref,trigger,config_ref,method_version,token_limit,state)VALUES($1,$2,$3,'on_demand',$4,'topic-map.research.v1.1',100000,'completed')")
        .bind(run).bind(D).bind(Uuid::new_v4()).bind(config).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_topic_map_research_task(task_ref,run_ref,domain_ref,work_public_ref,input_hash,input_refs,state,attempt_count)VALUES($1,$2,$3,$4,$5,$6,'succeeded',1)")
        .bind(task).bind(run).bind(D).bind(work).bind(&hash).bind(manifest).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash,state,reserved_tokens,charged_tokens,result,finished_at) SELECT $1,m.connection_version_ref,m.model_ref,c.config_ref,'analyze',$3,'succeeded',399,399,$4,scope_001_now() FROM linggan_model_config c JOIN linggan_model_entry m USING(model_ref) WHERE c.config_ref=$2")
        .bind(invocation).bind(config).bind(&hash)
        .bind(json!({"purpose":"topic_map","domainRef":D,"runRef":run,"taskRef":task,"methodVersion":"topic-map.research.v1.1"}))
        .execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_topic_map_research_request(invocation_ref,task_ref,run_ref,attempt_ordinal,request_hash,request_manifest,budget_day,dispatch_started_at,deadline_at)VALUES($1,$2,$3,1,$4,$5,(scope_001_now()AT TIME ZONE 'Asia/Shanghai')::date,scope_001_now(),scope_001_now()+interval '1 minute')")
        .bind(invocation).bind(task).bind(run).bind(&hash).bind(request).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_topic_map_research_result(result_ref,domain_ref,work_public_ref,input_hash,output_json,method_version,invocation_ref)VALUES($1,$2,$3,$4,$5,'topic-map.research.v1.1',$6)")
        .bind(result).bind(D).bind(work).bind(hash).bind(output).bind(invocation).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    result
}

fn legacy_save(manual: (Uuid, Uuid), source: (Uuid, Uuid)) -> TopicMapCommand {
    TopicMapCommand::SaveAlternative {
        idempotency_key: format!("legacy-proof:save:{}", source.1),
        domain_ref: D,
        topic_ref: manual.0,
        definition_ref: manual.1,
        title: "保存合成旧版角度".into(),
        angle: "沿用旧版精确引用的合成练习角度".into(),
        rationale: "SYNTHETIC 旧版兼容资格测试".into(),
        evidence_work_refs: vec![source.0],
        method_version: "topic-map.research.v1.1".into(),
        research_result_ref: Some(source.1),
        research_angle_index: Some(0),
        research_opportunity_index: None,
    }
}

#[tokio::test]
#[ignore = "isolated native PostgreSQL; synthetic v1 ledger only"]
async fn legacy_direct_and_wrapped_definition_dependencies_hide_angles_and_block_saving() {
    let (db, config, _) = setup("topic_core_legacy_derived").await;
    let body = "SYNTHETIC 合格正文：开始练习之前先拆解步骤。";
    let source = work(&db, "legacy-safe-source", body).await;
    let boundary = work(
        &db,
        "legacy-safe-boundary",
        "SYNTHETIC 练习之后的长期效果边界。",
    )
    .await;
    let machine = legacy_machine_candidate(&db, source).await;
    let manual = manual_definition(&db, source, boundary).await;
    let safe_result = legacy_result(&db, config, (source, body), manual, None, false).await;
    let mut restricted = Vec::new();
    for (id, wrapped) in [("legacy-direct", false), ("legacy-wrapped", true)] {
        let work = work(&db, id, body).await;
        let result = legacy_result(&db, config, (work, body), manual, Some(machine), wrapped).await;
        restricted.push((work, result));
    }
    let snapshot = topic_map::read_topic_map(&db, &query()).await.unwrap();
    let visible = serde_json::to_string(&snapshot).unwrap();
    assert!(
        visible.contains(VISIBLE_ANGLE),
        "valid v1 source and manual context remain readable"
    );
    assert!(!visible.contains(PRIVATE_ANGLE));
    assert!(!visible.contains(PRIVATE_DEFINITION));
    for (work, _) in &restricted {
        assert!(
            snapshot
                .works
                .iter()
                .any(|w| w.work_ref == *work && w.readable),
            "definition withdrawal must not hide an independently qualified raw work"
        );
    }
    let saved = topic_map::save_topic_map_command(&db, &legacy_save(manual, (source, safe_result)))
        .await
        .unwrap()
        .subject_ref
        .unwrap();
    let reopened = topic_map::read_saved_alternative(&db, D, saved)
        .await
        .unwrap();
    assert_ne!(reopened.source_state, "source_unavailable");
    assert!(reopened.fragments.iter().any(|f| f.text == body));
    for source in restricted {
        let result = topic_map::save_topic_map_command(&db, &legacy_save(manual, source)).await;
        assert!(
            matches!(result, Err(topic_map::TopicMapError::Invalid(_))),
            "a current manual destination cannot launder unavailable legacy input definitions: {result:?}"
        );
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_topic_map_alternative")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(
        count, 1,
        "only the independent manual-context angle is saved"
    );
}
