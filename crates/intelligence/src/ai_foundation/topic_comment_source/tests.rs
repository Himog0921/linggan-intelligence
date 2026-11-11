//! Actual isolated ledger/packet/publication/read path with explicitly synthetic saved semantics.
use super::*;
use crate::ai_foundation::{
    topic_research as owner, topic_research_automation as automation,
    topic_research_packet as packet,
};
use linggan_storage_postgres::Database;
#[allow(dead_code)]
#[path = "../../topic_map_research/tests/saved_source_fixture.rs"]
mod support;
#[tokio::test]
#[ignore = "owned isolated PostgreSQL; synthetic accepted semantics, no provider"]
async fn saved_comments_match_only_existing_topics_and_keep_unmatched() {
    let (db, model_config) =
        support::setup(&format!("comment_map_{}", Uuid::new_v4().simple())).await;
    let work = support::work(&db, "dossier-note", "作者材料只作原评论语义研究的上下文。").await;
    let raw = "孩子每天开始作业前需要提醒。我喜欢蓝色笔盒。";
    let source = support::comments::comment_with_author(
        &db,
        "dossier-note",
        "comment-one",
        raw,
        Some("reader"),
        "2026-10-14T00:00:00Z",
    )
    .await;
    // Seed selected configurations in this isolated fixture, not live settings.
    let settings: Vec<(String,Uuid)> = sqlx::query_as("SELECT task_key,current_config_ref FROM linggan_ai_task_setting WHERE task_key LIKE 'topic_research%' OR task_key='content_understanding'").fetch_all(db.pool()).await.unwrap();
    for (task, previous) in settings {
        let config = Uuid::new_v4();
        sqlx::query("INSERT INTO linggan_ai_task_config(task_config_ref,task_key,model_config_ref,execution_contract,material_columns,budget_mode,origin,prompt_version_ref) SELECT $1,task_key,$2,execution_contract,material_columns,'unlimited','seed',prompt_version_ref FROM linggan_ai_task_config WHERE task_config_ref=$3")
            .bind(config).bind(model_config).bind(previous).execute(db.pool()).await.unwrap();
        sqlx::query("UPDATE linggan_ai_task_setting SET current_config_ref=$1,revision=revision+1 WHERE task_key=$2").bind(config).bind(task).execute(db.pool()).await.unwrap();
    }
    use crate::ai_foundation::control_store::{Actor, ChangeControl, ControlScope, change_control};
    for scope in [
        ControlScope::Global,
        ControlScope::Executor,
        ControlScope::Task(crate::ai_foundation::TaskKey::CommentStudy),
    ]
    .into_iter()
    .chain(super::super::topic_research::TASKS.map(ControlScope::Task))
    {
        let revision =
            sqlx::query_scalar("SELECT revision FROM linggan_ai_control WHERE scope_key=$1")
                .bind(scope.key())
                .fetch_one(db.pool())
                .await
                .unwrap();
        change_control(
            &db,
            &ChangeControl {
                command_ref: Uuid::new_v4(),
                scope,
                expected_revision: revision,
                enabled: true,
                reason: "SYNTHETIC isolated fixture".into(),
            },
            Actor::Human,
        )
        .await
        .unwrap();
    }
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash,state,reserved_tokens,charged_tokens,result,finished_at) SELECT $1,m.connection_version_ref,m.model_ref,$2,'probe',$3,'succeeded',0,0,'{\"modelCallable\":true,\"synthetic\":true}',scope_001_now() FROM linggan_model_config c JOIN linggan_model_entry m USING(model_ref) WHERE c.config_ref=$2")
        .bind(Uuid::new_v4()).bind(model_config).bind("b".repeat(64)).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_egress_event(event_ref,endpoint_ref,task_key,material_class,allowed,actor_kind,reason) SELECT gen_random_uuid(),cap.endpoint_ref,t.task_key,material.class,true,'human','SYNTHETIC isolated fixture, no network' FROM linggan_ai_task t CROSS JOIN (VALUES('note_material'),('comment_material')) material(class) CROSS JOIN linggan_model_config c JOIN linggan_model_capability cap USING(capability_ref) WHERE c.config_ref=$1 AND t.task_key LIKE 'topic_research%'")
        .bind(model_config).execute(db.pool()).await.unwrap();

    let semantic = seed_semantic(&db, model_config, work, source, raw).await;
    let executor =
        crate::ai_foundation::executor::ExecutorSession::acquire(&db, "synthetic-comment-map", 4)
            .await
            .unwrap();
    let item:Uuid=sqlx::query_scalar("SELECT i.item_ref FROM linggan_ai_batch_item i JOIN linggan_comment_research_request r USING(batch_ref) WHERE r.request_ref=$1").bind(semantic).fetch_one(db.pool()).await.unwrap();
    settle_item(&db, item, model_config, &executor).await;
    let query = Query {
        domain_ref: support::D,
        work_ref: Some(work),
        request_ref: None,
        topic_ref: None,
    };
    let initial = read(&db, &query).await.unwrap();
    assert_eq!(initial["items"][0]["assignmentState"], "not_analyzed");
    let mut cursor = None;
    assert_eq!(automation::tick(&db, &mut cursor).await.unwrap(), 0);
    automation::change(
        &db,
        &automation::Change {
            command_ref: Uuid::new_v4(),
            domain_ref: support::D,
            expected_revision: 0,
            enabled: true,
        },
    )
    .await
    .unwrap();
    assert_eq!(automation::tick(&db, &mut cursor).await.unwrap(), 1);
    assert_eq!(automation::tick(&db, &mut cursor).await.unwrap(), 0);
    let window: Uuid = sqlx::query_scalar("SELECT window_ref FROM linggan_topic_research_window")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let stages: Vec<i32> = sqlx::query_scalar(
        "SELECT stage_number FROM linggan_topic_research_stage ORDER BY stage_number",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(stages, vec![2, 3]);
    let frozen = owner::frozen(&db, window).await.unwrap();
    assert_eq!(frozen.fragments.len(), 2);
    assert!(frozen.fragments.iter().all(|f| f.speaker == "commenter"));
    let (topic, definition) = seed_topic(&db).await;
    let mut tx = db.pool().begin().await.unwrap();
    let proposed = packet::prepare(&db, &mut tx, window, 2, &frozen)
        .await
        .unwrap();
    assert_eq!(
        proposed.schema["properties"]["decisions"]["items"]["properties"]["status"]["enum"],
        json!(["matched", "uncertain", "out_of_scope"])
    );
    let units = crate::topic_map_core::units_for_discussions(&frozen, &discussions(&frozen));
    let output = json!({"contract":crate::topic_map_research_analysis::RESOLVE_CONTRACT,"decisions":[
        {"unitId":units[0].0,"status":"matched","matches":[{"topicRef":topic,"definitionRef":definition,"reason":"SYNTHETIC 同为启动任务"}],"proposedTopic":null,"relations":[],"reason":"SYNTHETIC 启动任务"},
        {"unitId":units[1].0,"status":"uncertain","matches":[],"proposedTopic":null,"relations":[],"reason":"SYNTHETIC 现有主题不含笔盒偏好"}]});
    packet::validate(&output.to_string(), 2, &frozen, &proposed.context, None).unwrap();
    let mut forbidden = output.clone();
    forbidden["decisions"][1]["status"] = json!("new");
    assert_eq!(
        packet::validate(&forbidden.to_string(), 2, &frozen, &proposed.context, None).unwrap_err(),
        "comment_topic_creation_forbidden"
    );
    let inv = invocation(&db, model_config).await;
    sqlx::query("INSERT INTO linggan_topic_research_dispatch(item_ref,invocation_ref,context) SELECT item_ref,$2,$3 FROM linggan_topic_research_stage WHERE window_ref=$1 AND stage_number=2").bind(window).bind(inv).bind(&proposed.context).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_topic_research_stage_result(item_ref,invocation_ref,output) SELECT item_ref,$2,$3 FROM linggan_topic_research_stage WHERE window_ref=$1 AND stage_number=2").bind(window).bind(inv).bind(output).execute(&mut *tx).await.unwrap();
    let review = packet::prepare(&db, &mut tx, window, 3, &frozen)
        .await
        .unwrap();
    let verification = json!({"contract":"topic-map.verify.v1","checks":review.context["checks"].as_array().unwrap().iter().map(|c|json!({"checkId":c["checkId"],"verdict":"pass","reason":"SYNTHETIC exact existing topic"})).collect::<Vec<_>>()});
    packet::validate(&verification.to_string(), 3, &frozen, &review.context, None).unwrap();
    packet::publish_in(
        &mut tx,
        window,
        3,
        invocation(&db, model_config).await,
        &frozen,
        &review.context,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let result = read(&db, &query).await.unwrap();
    assert_eq!(result["items"][0]["commentExternalId"], "comment-one");
    assert_eq!(
        result["items"][0]["signals"][0]["assignment"]["matches"][0]["topicRef"],
        json!(topic)
    );
    assert_eq!(
        result["items"][0]["signals"][1]["assignment"]["status"],
        "uncertain"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_topic_workspace")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_topic_material_member")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
    let map = crate::topic_map::read_topic_map(
        &db,
        &crate::topic_map::TopicMapQuery {
            domain_ref: Some(support::D),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(
        map.works
            .iter()
            .filter(|w| w.work_ref == work)
            .all(|w| w.research.is_none())
    );
    // Actual definition change hides historical assignment and admits only one two-stage repair.
    let items: Vec<Uuid> =
        sqlx::query_scalar("SELECT item_ref FROM linggan_topic_research_stage WHERE window_ref=$1")
            .bind(window)
            .fetch_all(db.pool())
            .await
            .unwrap();
    for item in items {
        settle_item(&db, item, model_config, &executor).await;
    }
    let replacement = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_topic_definition(definition_ref,topic_ref,version,display_name,definition_text,lifecycle_state) VALUES($1,$2,2,'任务启动','开始任务前需要提醒并迟迟不行动','provisional')").bind(replacement).bind(topic).execute(db.pool()).await.unwrap();
    let stale = read(&db, &query).await.unwrap();
    assert!(stale["items"][0]["signals"][0]["assignment"].is_null());
    assert_eq!(stale["items"][0]["assignmentState"], "unavailable");
    let repaired = automation::tick(&db, &mut cursor).await.unwrap()
        + automation::tick(&db, &mut cursor).await.unwrap();
    assert_eq!(repaired, 1);
    for _ in 0..3 {
        assert_eq!(automation::tick(&db, &mut cursor).await.unwrap(), 0);
    }
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_topic_research_stage WHERE stage_number<2",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(count, 0);
    sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason) VALUES($1,'comment-one','synthetic withdrawal')").bind(work).execute(db.pool()).await.unwrap();
    assert!(owner::frozen(&db, window).await.is_err());
    let withdrawn = read(&db, &query).await.unwrap();
    assert_eq!(withdrawn["items"][0]["sourceAvailable"], false);
    assert_eq!(withdrawn["items"][0]["signals"], json!([]));
}

async fn invocation(db: &Database, model: Uuid) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash,state,reserved_tokens,charged_tokens,result,finished_at) SELECT $1,m.connection_version_ref,m.model_ref,$2,'analyze',$3,'succeeded',0,0,'{\"synthetic\":true}',scope_001_now() FROM linggan_model_config c JOIN linggan_model_entry m USING(model_ref) WHERE c.config_ref=$2")
 .bind(id).bind(model).bind(linggan_evidence::creator_discovery::hash(&id.to_string())).execute(db.pool()).await.unwrap();
    id
}

async fn settle_item(
    db: &Database,
    item: Uuid,
    model: Uuid,
    owner: &crate::ai_foundation::executor::ExecutorSession,
) {
    let inv = invocation(db, model).await;
    let mut tx = owner.fence().begin(db, true).await.unwrap();
    sqlx::query("UPDATE linggan_ai_batch_item_control SET state='running',lease_generation=1,lease_token=$2,lease_instance_ref=$3,lease_instance_generation=$4,lease_expires_at=clock_timestamp()+interval '1 minute' WHERE item_ref=$1")
   .bind(item).bind(Uuid::new_v4()).bind(owner.fence().instance_ref()).bind(owner.fence().generation()).execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE linggan_ai_batch_item_control ctl SET invocation_ref=$2,attempt_count=1,executed_config_ref=cfg.task_config_ref,executed_skill_version_ref=cfg.skill_version_ref,execution_contract=cfg.execution_contract FROM linggan_ai_batch_item i JOIN linggan_ai_batch b USING(batch_ref) JOIN linggan_ai_task_config cfg ON cfg.task_config_ref=b.task_config_ref WHERE ctl.item_ref=i.item_ref AND i.item_ref=$1")
   .bind(item).bind(inv).execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE linggan_ai_batch_item_control SET state='succeeded',usable=true,output='{\"synthetic\":true}',finished_at=scope_001_now(),lease_token=NULL,lease_expires_at=NULL,lease_instance_ref=NULL,lease_instance_generation=NULL WHERE item_ref=$1").bind(item).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
}

async fn seed_semantic(db: &Database, model: Uuid, work: Uuid, source: Uuid, raw: &str) -> Uuid {
    let preview = super::super::comment_research::preview(
        db,
        support::D,
        work,
        source,
        crate::comment_study_source::StudySourceRole::Primary,
    )
    .await
    .unwrap();
    let selected = crate::comment_study_source::selected_source_in(
        &mut db.pool().begin().await.unwrap(),
        support::D,
        work,
        source,
        crate::comment_study_source::StudySourceRole::Primary,
        preview["knowledgeCutoff"].as_str().unwrap(),
    )
    .await
    .unwrap()
    .unwrap();
    let prepared = crate::comment_study_selection::prepare_study_input(
        &crate::comment_study_selection::CommentKey {
            work_ref: work,
            comment_external_id: "comment-one".into(),
        },
        source,
        raw,
        &selected.source.context_manifest,
        selected.parent.as_ref(),
    )
    .unwrap();
    let (request, command, batch, item) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    let config: Uuid = serde_json::from_value(preview["taskConfigRef"].clone()).unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    sqlx::query("INSERT INTO linggan_ai_command(command_ref,operation,payload,receipt) VALUES($1,'SYNTHETIC_SAVED_COMMENT','{}','{}')").bind(command).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_ai_batch(batch_ref,task_key,task_config_ref,kind,inputs_sha256,item_count,command_ref) VALUES($1,'comment_study',$2,'interactive',$3,1,$4)").bind(batch).bind(config).bind(&prepared.input_hash).bind(command).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_ai_batch_item(item_ref,batch_ref,position,item_key,input_ref) VALUES($1,$2,0,'synthetic',$3)").bind(item).bind(batch).bind(json!({"type":"comment_research","requestRef":request})).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_comment_research_request(request_ref,command_ref,domain_ref,work_ref,source_ref,observation_role,knowledge_cutoff,input_hash,input_fingerprint,task_config_ref,batch_ref,method_version) VALUES($1,$2,$3,$4,$5,'primary',$6::timestamptz,$7,$8,$9,$10,'comment-research-semantic.v1')")
        .bind(request).bind(command).bind(support::D).bind(work).bind(source).bind(preview["knowledgeCutoff"].as_str().unwrap()).bind(prepared.input_hash).bind(prepared.input_fingerprint).bind(config).bind(batch).execute(&mut *tx).await.unwrap();
    let first = "孩子每天开始作业前需要提醒。".chars().count();
    let result = json!({"contract":"comment-research-semantic.v1","signals":[
        {"kind":"need","proposition":"孩子开始作业需要提醒","sourceRef":source,"evidenceSpan":{"start":0,"end":first},"problemFrame":null,"eligibilityState":"eligible","eligibilityReason":null},
        {"kind":"experience","proposition":"评论者喜欢蓝色笔盒","sourceRef":source,"evidenceSpan":{"start":first,"end":raw.chars().count()},"problemFrame":null,"eligibilityState":"not_applicable","eligibilityReason":null}]});
    sqlx::query("INSERT INTO linggan_comment_research_result(request_ref,invocation_ref,result) VALUES($1,$2,$3)").bind(request).bind(invocation(db,model).await).bind(result).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    request
}
async fn seed_topic(db: &Database) -> (Uuid, Uuid) {
    let (topic, definition, receipt) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    sqlx::query("INSERT INTO linggan_topic_workspace(topic_ref,domain_key,canonical_key) VALUES($1,'synthetic','synthetic-existing-start')").bind(topic).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_topic_definition(definition_ref,topic_ref,version,display_name,definition_text,lifecycle_state) VALUES($1,$2,1,'任务启动','开始任务前需要提醒','provisional')").bind(definition).bind(topic).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_topic_map_receipt(receipt_ref,idempotency_key,request_sha256,action) VALUES($1,$2,$3,'synthetic')").bind(receipt).bind(receipt.to_string()).bind("a".repeat(64)).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_topic_map_binding(binding_ref,topic_ref,domain_ref,version,receipt_ref) VALUES($1,$2,$3,1,$4)").bind(Uuid::new_v4()).bind(topic).bind(support::D).bind(receipt).execute(db.pool()).await.unwrap();
    (topic, definition)
}

#[test]
fn comment_dependency_scope_is_actual_matches_or_catalog_for_unmatched() {
    let a = Uuid::new_v4();
    let ad = Uuid::new_v4();
    let b = Uuid::new_v4();
    let bd = Uuid::new_v4();
    let original = json!([{"topicRef":a,"definitionRef":ad}]);
    let expanded = json!([{"topicRef":a,"definitionRef":ad},{"topicRef":b,"definitionRef":bd}]);
    let matched = json!({"status":"matched","assignments":[{"topicRef":a,"definitionRef":ad}]});
    let unmatched = json!({"status":"uncertain","assignments":[]});
    assert!(assignment_current(
        &matched,
        &catalog_versions(&original),
        &expanded
    ));
    assert!(!assignment_current(
        &unmatched,
        &catalog_versions(&original),
        &expanded
    ));
    assert!(assignment_current(
        &unmatched,
        &catalog_versions(&expanded),
        &expanded
    ));
    assert!(!assignment_current(
        &matched,
        &catalog_versions(&original),
        &json!([])
    ));
}
