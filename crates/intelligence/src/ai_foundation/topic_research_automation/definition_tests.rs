//! Real database reuse proof with explicitly synthetic accepted model output; no network.
use super::*;
use crate::ai_foundation::{topic_research as owner, topic_research_packet as packet};
use serde_json::{Value, json};
use sqlx::Row;

async fn invocation(db: &Database, model: Uuid) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash,state,reserved_tokens,charged_tokens,result,finished_at) SELECT $1,m.connection_version_ref,m.model_ref,$2,'analyze',$3,'succeeded',0,0,'{\"synthetic\":true}',scope_001_now() FROM linggan_model_config c JOIN linggan_model_entry m USING(model_ref) WHERE c.config_ref=$2")
 .bind(id).bind(model).bind(linggan_evidence::creator_discovery::hash(&id.to_string())).execute(db.pool()).await.unwrap();
    id
}
async fn approve_extraction(db: &Database, window: Uuid, model: Uuid) {
    let output: Value = sqlx::query_scalar(
        "SELECT output_json FROM linggan_topic_research_result WHERE window_ref=$1",
    )
    .bind(window)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let frozen = owner::frozen(db, window).await.unwrap();
    let draft = serde_json::from_value(output.clone()).unwrap();
    let review = crate::topic_map_research_analysis::verification::extraction_review(
        &draft,
        &frozen.fragments,
    )
    .unwrap();
    let verified = json!({"contract":"topic-map.verify.v1","checks":review["checks"].as_array().unwrap().iter().map(|c|json!({"checkId":c["checkId"],"verdict":"pass","reason":"SYNTHETIC fixture"})).collect::<Vec<_>>()});
    crate::topic_map_research_analysis::verification::validate_verification(
        &serde_json::from_value(verified.clone()).unwrap(),
        &review["checks"],
    )
    .unwrap();
    for stage in [0] {
        let inv = invocation(db, model).await;
        sqlx::query("INSERT INTO linggan_topic_research_stage_result(item_ref,invocation_ref,output) SELECT item_ref,$3,$4 FROM linggan_topic_research_stage WHERE window_ref=$1 AND stage_number=$2")
   .bind(window).bind(stage).bind(inv).bind(if stage==0{output.clone()}else{verified.clone()}).execute(db.pool()).await.unwrap();
    }
}
async fn change_definition(db: &Database, topic: Uuid) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_topic_definition(definition_ref,topic_ref,version,display_name,definition_text,lifecycle_state) SELECT $1,topic_ref,version+1,display_name,definition_text,lifecycle_state FROM linggan_topic_definition WHERE topic_ref=$2 ORDER BY version DESC LIMIT 1")
 .bind(id).bind(topic).execute(db.pool()).await.unwrap();
    id
}
async fn poll(db: &Database) -> usize {
    let mut cursor = None;
    tick(db, &mut cursor).await.unwrap()
}

pub(crate) async fn assert_definition_repair(
    db: &Database,
    domain: Uuid,
    window: Uuid,
    model: Uuid,
    withdraw_image: bool,
) {
    let executor = crate::ai_foundation::executor::ExecutorSession::acquire(
        db,
        "synthetic-definition-proof",
        4,
    )
    .await
    .unwrap();
    approve_extraction(db, window, model).await;
    settle_window(db, window, model, &executor).await;
    let state = read(db, domain).await.unwrap();
    if !state.enabled {
        change(
            db,
            &Change {
                command_ref: Uuid::new_v4(),
                domain_ref: domain,
                expected_revision: state.revision,
                enabled: true,
            },
        )
        .await
        .unwrap();
    }
    let topic:Uuid=sqlx::query_scalar("SELECT (core_json#>>'{units,0,assignments,0,topicRef}')::uuid FROM linggan_topic_research_result WHERE window_ref=$1").bind(window).fetch_one(db.pool()).await.unwrap();
    assert_eq!(poll(db).await, 0);
    // A new unrelated topic must not invalidate prior judgments.
    let extra = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_topic_workspace(topic_ref,domain_key,canonical_key) VALUES($1,'synthetic-domain',$2)").bind(extra).bind(format!("synthetic-{}",extra.simple())).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_topic_definition(definition_ref,topic_ref,version,display_name,definition_text,lifecycle_state) VALUES($1,$2,1,'无关新增方向','合成新增，与前次定义无关','candidate')").bind(Uuid::new_v4()).bind(extra).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_topic_map_binding(binding_ref,topic_ref,domain_ref,parent_topic_ref,version,receipt_ref) SELECT $1,$2,domain_ref,NULL,1,receipt_ref FROM linggan_topic_map_binding WHERE topic_ref=$3 ORDER BY version DESC LIMIT 1").bind(Uuid::new_v4()).bind(extra).bind(topic).execute(db.pool()).await.unwrap();
    assert_eq!(poll(db).await, 0);
    let mut previous = window;
    for round in 0..2 {
        let definition = change_definition(db, topic).await;
        let work: Uuid = sqlx::query_scalar(
            "SELECT work_ref FROM linggan_topic_research_window WHERE window_ref=$1",
        )
        .bind(window)
        .fetch_one(db.pool())
        .await
        .unwrap();
        let manual = Uuid::new_v4();
        sqlx::query("INSERT INTO linggan_topic_classification_run(classification_run_ref,definition_ref,run_kind,run_state,adjudication_note) VALUES($1,$2,'human_adjudicated','completed','SYNTHETIC manual authority')").bind(manual).bind(definition).execute(db.pool()).await.unwrap();
        sqlx::query("INSERT INTO linggan_topic_material_member(classification_run_ref,work_public_ref,role,rationale,ordinal) VALUES($1,$2,'support','SYNTHETIC manual membership',1)").bind(manual).bind(work).execute(db.pool()).await.unwrap();
        if round == 0 {
            use crate::ai_foundation::{control_store::ControlScope, topic_research::CURRENT_TASKS};
            for scope in [
                ControlScope::Global,
                ControlScope::Executor,
                ControlScope::Task(CURRENT_TASKS[0]),
            ] {
                switch(db, scope.clone(), false).await;
                assert_eq!(poll(db).await, 0);
                switch(db, scope, true).await;
            }
        }

        if round == 0 {
            let state = read(db, domain).await.unwrap();
            change(
                db,
                &Change {
                    command_ref: Uuid::new_v4(),
                    domain_ref: domain,
                    expected_revision: state.revision,
                    enabled: false,
                },
            )
            .await
            .unwrap();
            assert_eq!(poll(db).await, 0);
            let state = read(db, domain).await.unwrap();
            change(
                db,
                &Change {
                    command_ref: Uuid::new_v4(),
                    domain_ref: domain,
                    expected_revision: state.revision,
                    enabled: true,
                },
            )
            .await
            .unwrap();
        }
        assert_eq!(poll(db).await, 1);
        assert_eq!(poll(db).await, 0);
        let repaired:Uuid=sqlx::query_scalar("SELECT window_ref FROM linggan_topic_research_window WHERE input_refs#>>'{definitionReassessment,sourceWindow}'=$1 ORDER BY created_at DESC LIMIT 1").bind(previous.to_string()).fetch_one(db.pool()).await.unwrap();
        let stages:Vec<i32>=sqlx::query_scalar("SELECT stage_number FROM linggan_topic_research_stage WHERE window_ref=$1 ORDER BY stage_number").bind(repaired).fetch_all(db.pool()).await.unwrap();
        assert_eq!(stages, vec![0]);
        let original = owner::frozen(db, window).await.unwrap();
        let frozen = owner::frozen(db, repaired).await.unwrap();
        assert_eq!(frozen.hash, original.hash);
        let mut tx = db.pool().begin().await.unwrap();
        assert_eq!(
            owner::previous_in(&mut tx, repaired, 0).await.unwrap(),
            owner::previous_in(&mut tx, window, 0).await.unwrap()
        );
        tx.rollback().await.unwrap();
        // Feed an explicitly synthetic matched resolution through actual production publication.
        let candidates = crate::topic_map_core::catalog(db, domain).await.unwrap();
        let mut tx = db.pool().begin().await.unwrap();
        let draft = packet::draft_in(&mut tx, repaired).await.unwrap();
        tx.rollback().await.unwrap();
        let units = crate::topic_map_core::units(&frozen, &draft);
        let output = json!({"contract":crate::topic_map_research_analysis::RESOLVE_CONTRACT,"decisions":[{"unitId":units[0].0,"status":"matched","matches":[{"topicRef":topic,"definitionRef":definition,"reason":"合成重判保留边界"}],"proposedTopic":null,"relations":[],"reason":"合成重判"}]});
        let typed = serde_json::from_value(output.clone()).unwrap();
        crate::topic_map_research_analysis::validate_resolution(&typed, &units, &candidates)
            .unwrap();
        let parsed = json!({"contract":"topic-map.single-pass.v1","projection":serde_json::to_value(&draft).unwrap(),"resolution":output});
        let inv = invocation(db, model).await;
        let accepted = invocation(db, model).await;
        let mut tx = db.pool().begin().await.unwrap();
        sqlx::query("INSERT INTO linggan_topic_research_stage_result(item_ref,invocation_ref,output) SELECT item_ref,$2,$3 FROM linggan_topic_research_stage WHERE window_ref=$1 AND stage_number=0").bind(repaired).bind(inv).bind(&parsed).execute(&mut *tx).await.unwrap();
        packet::publish_in(
            &mut tx,
            repaired,
            0,
            accepted,
            &frozen,
            &json!({"candidates":candidates,"recall":{"method":"synthetic-proof"}}),
            &parsed,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        settle_window(db, repaired, model, &executor).await;
        assert_eq!(poll(db).await, 0);
        let assigned:Uuid=sqlx::query_scalar("SELECT (core_json#>>'{units,0,assignments,0,definitionRef}')::uuid FROM linggan_topic_research_result WHERE window_ref=$1").bind(repaired).fetch_one(db.pool()).await.unwrap();
        assert_eq!(assigned, definition);
        let retained:String=sqlx::query_scalar("SELECT rationale FROM linggan_topic_material_member WHERE classification_run_ref=$1 AND work_public_ref=$2").bind(manual).bind(work).fetch_one(db.pool()).await.unwrap();
        assert_eq!(retained, "SYNTHETIC manual membership");
        previous = repaired;
    }
    let outputs:i64=sqlx::query_scalar("SELECT count(*) FROM linggan_topic_research_stage s JOIN linggan_topic_research_stage_result r USING(item_ref) WHERE s.window_ref=$1 AND stage_number=0").bind(window).fetch_one(db.pool()).await.unwrap();
    assert_eq!(outputs, 1);
    if withdraw_image {
        change_definition(db, topic).await;
        let material:Uuid=sqlx::query_scalar("SELECT m.materialization_ref FROM linggan_material_media_origin o JOIN linggan_media_download_attempt a ON a.media_observation_ref=o.observation_ref JOIN linggan_media_materialization m USING(download_attempt_ref) WHERE o.content_public_ref=(SELECT work_ref FROM linggan_topic_research_window WHERE window_ref=$1) ORDER BY m.verified_at DESC LIMIT 1").bind(window).fetch_one(db.pool()).await.unwrap();
        sqlx::query("INSERT INTO linggan_material_media_disposition_event(event_ref,materialization_ref,state,authority_ref,reason,effective_at) VALUES($1,$2,'WITHDRAWN_OR_RESTRICTED','synthetic-proof','definition repair withdrawn image',scope_001_now())").bind(Uuid::new_v4()).bind(material).execute(db.pool()).await.unwrap();
        assert!(owner::frozen(db, previous).await.is_err());
        assert_eq!(poll(db).await, 0);
    }
}

async fn settle_window(
    db: &Database,
    window: Uuid,
    model: Uuid,
    owner: &crate::ai_foundation::executor::ExecutorSession,
) {
    let items=sqlx::query("SELECT s.item_ref,COALESCE(sr.invocation_ref,CASE WHEN s.stage_number=3 THEN r.invocation_ref END) AS invocation FROM linggan_topic_research_stage s LEFT JOIN linggan_topic_research_stage_result sr USING(item_ref) LEFT JOIN linggan_topic_research_result r USING(window_ref) WHERE s.window_ref=$1").bind(window).fetch_all(db.pool()).await.unwrap();
    for row in items {
        let item: Uuid = row.get("item_ref");
        let inv = match row.get::<Option<Uuid>, _>("invocation") {
            Some(id) => id,
            None => invocation(db, model).await,
        };
        let mut tx = owner.fence().begin(db, true).await.unwrap();
        sqlx::query("UPDATE linggan_ai_batch_item_control SET state='running',lease_generation=1,lease_token=$2,lease_instance_ref=$3,lease_instance_generation=$4,lease_expires_at=clock_timestamp()+interval '1 minute' WHERE item_ref=$1")
   .bind(item).bind(Uuid::new_v4()).bind(owner.fence().instance_ref()).bind(owner.fence().generation()).execute(&mut *tx).await.unwrap();
        sqlx::query("UPDATE linggan_ai_batch_item_control ctl SET invocation_ref=$2,attempt_count=1,executed_config_ref=cfg.task_config_ref,executed_skill_version_ref=cfg.skill_version_ref,execution_contract=cfg.execution_contract FROM linggan_ai_batch_item i JOIN linggan_ai_batch b USING(batch_ref) JOIN linggan_ai_task_config cfg ON cfg.task_config_ref=b.task_config_ref WHERE ctl.item_ref=i.item_ref AND i.item_ref=$1")
   .bind(item).bind(inv).execute(&mut *tx).await.unwrap();
        sqlx::query("UPDATE linggan_ai_batch_item_control SET state='succeeded',usable=true,output='{\"synthetic\":true}',finished_at=scope_001_now(),lease_token=NULL,lease_expires_at=NULL,lease_instance_ref=NULL,lease_instance_generation=NULL WHERE item_ref=$1").bind(item).execute(&mut *tx).await.unwrap();
        tx.commit().await.unwrap();
    }
}

async fn switch(
    db: &Database,
    scope: crate::ai_foundation::control_store::ControlScope,
    enabled: bool,
) {
    use crate::ai_foundation::control_store::{Actor, ChangeControl, change_control};
    let revision = sqlx::query_scalar("SELECT revision FROM linggan_ai_control WHERE scope_key=$1")
        .bind(scope.key())
        .fetch_one(db.pool())
        .await
        .unwrap();
    change_control(
        db,
        &ChangeControl {
            command_ref: Uuid::new_v4(),
            scope,
            expected_revision: revision,
            enabled,
            reason: "SYNTHETIC definition repair switch".into(),
        },
        Actor::Human,
    )
    .await
    .unwrap();
}
