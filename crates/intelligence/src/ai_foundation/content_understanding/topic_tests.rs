//! Saved L1 → real topic queue → accepted synthetic resolution → production read.
//! Provider output is explicitly seeded; no model network request is made.
use super::*;
#[allow(dead_code)]
#[path = "../../topic_map_research/tests/saved_source_fixture.rs"]
mod support;

#[tokio::test]
#[ignore = "owned isolated PostgreSQL, synthetic saved Core; no provider"]
async fn saved_core_drives_topic_queue_and_persisted_membership() {
    roundtrip(false).await;
    roundtrip(true).await;
}
async fn roundtrip(image_only: bool) {
    let (db, model_config) = support::setup(&format!("tm_core_{}", Uuid::new_v4().simple())).await;
    let work = support::work(
        &db,
        "dossier-note",
        if image_only {
            ""
        } else {
            "合成作者内容：先摆出材料，再把作业拆成几个短步骤。"
        },
    )
    .await;
    if image_only {
        support::dossier_fixture::acquired_image_source(&db).await;
    }
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
        ControlScope::Task(TASK),
    ]
    .into_iter()
    .chain(super::super::topic_research::CURRENT_TASKS.map(ControlScope::Task))
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
    sqlx::query("INSERT INTO linggan_model_egress_event(event_ref,endpoint_ref,task_key,material_class,allowed,actor_kind,reason) SELECT gen_random_uuid(),cap.endpoint_ref,t.task_key,'note_material',true,'human','SYNTHETIC isolated fixture, no network' FROM linggan_ai_task t CROSS JOIN linggan_model_config c JOIN linggan_model_capability cap USING(capability_ref) WHERE c.config_ref=$1 AND (t.task_key LIKE 'topic_research%' OR t.task_key='content_understanding')")
        .bind(model_config).execute(db.pool()).await.unwrap();
    let missing = super::super::topic_research::start(
        &db,
        &super::super::topic_research::Start {
            command_ref: Uuid::new_v4(),
            domain_ref: support::D,
            work_refs: vec![work],
        },
    )
    .await;
    assert!(
        matches!(missing, Err(FoundationError::AdmissionDenied(ref reasons)) if reasons.contains(&"content_understanding_required".into()))
    );
    let cutoff: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(
        scope_coverage(&db, &[work], &cutoff).await.unwrap(),
        json!({"available":0,"missing":1,"stale":0,"unknown":0})
    );
    let reference = seed_core(&db, work, model_config, image_only).await;
    let cutoff: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    let packet = source::load(&mut tx, support::D, work, &cutoff)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let ready = packet.coverage["materialReadiness"]["state"] == "ready";
    let coverage = scope_coverage(&db, &[work], &cutoff).await.unwrap();
    assert_eq!(coverage["available"], json!(usize::from(ready)));
    assert_eq!(coverage["stale"], json!(usize::from(!ready)));
    assert_eq!(coverage["missing"], 0);

    use crate::ai_foundation::topic_research_automation as automation;
    let run = if image_only {
        automation::tests::assert_first_queue(&db, support::D, work, reference).await
    } else {
        let receipt = super::super::topic_research::start(
            &db,
            &super::super::topic_research::Start {
                command_ref: Uuid::new_v4(),
                domain_ref: support::D,
                work_refs: vec![work],
            },
        )
        .await
        .unwrap();
        assert_eq!(receipt.window_count, 1);
        assert_eq!(receipt.stage_count, 1);
        automation::tests::assert_manual_not_requeued(&db, support::D).await;
        receipt.run_ref
    };
    let window: Uuid =
        sqlx::query_scalar("SELECT window_ref FROM linggan_topic_research_window WHERE run_ref=$1")
            .bind(run)
            .fetch_one(db.pool())
            .await
            .unwrap();
    let input = super::super::topic_research::frozen(&db, window)
        .await
        .unwrap();
    assert_eq!(input.coverage["coreRef"], json!(reference));
    assert_eq!(input.comment_study, json!([]));
    support::seed_accepted_source_task(&db, window).await;
    let projected = support::projected_research(&db, work).await;
    assert!(
        !projected["output"]["discussions"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let memberships: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_topic_map_unit_assignment")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert!(memberships > 0);
    let catalog = crate::topic_map_core::catalog(&db, support::D)
        .await
        .unwrap();
    let example = catalog
        .as_array()
        .unwrap()
        .iter()
        .find_map(|candidate| {
            candidate["supportExamples"]
                .as_array()
                .and_then(|items| items.first())
        })
        .expect("qualified catalog has a frozen definition-creation example");
    assert_eq!(example["coreRef"], json!(reference));
    assert_eq!(example["workRef"], json!(work));
    assert_eq!(
        example["evidence"][0]["field"],
        if image_only {
            "core_image_evidence"
        } else {
            "core_text_evidence"
        }
    );
    assert!(example["evidence"][0]["knowledgeUnits"].is_array());
    let saved = support::save_task_angle(&db, window, work, "core-saved-reopen").await;
    let reopened = crate::topic_map::read_saved_alternative(&db, support::D, saved)
        .await
        .unwrap();
    assert_ne!(reopened.source_state, "source_unavailable");
    assert!(reopened.fragments.iter().any(|f| f.field
        == if image_only {
            "core_image_evidence"
        } else {
            "core_text_evidence"
        }));
    let egress: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_model_egress_event WHERE material_class='comment_material'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(egress, 0);

    if !image_only {
        automation::definition_tests::assert_definition_repair(
            &db,
            support::D,
            window,
            model_config,
            false,
        )
        .await;
    }
    if image_only {
        // The first automation helper left this domain disabled. A new saved Core
        // stays idle until explicit enable, then queues exactly once.
        support::work_at(
            &db,
            "dossier-note",
            "合成更新标题",
            "",
            10,
            "2026-11-11T00:00:00Z",
        )
        .await;
        let cutoff: String = sqlx::query_scalar("SELECT scope_001_now()::text")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(
            scope_coverage(&db, &[work], &cutoff).await.unwrap()["stale"],
            1
        );
        let next = seed_core(&db, work, model_config, image_only).await;
        let mut cursor = None;
        assert_eq!(automation::tick(&db, &mut cursor).await.unwrap(), 0);
        let state = automation::read(&db, support::D).await.unwrap();
        automation::change(
            &db,
            &automation::Change {
                command_ref: Uuid::new_v4(),
                domain_ref: support::D,
                expected_revision: state.revision,
                enabled: true,
            },
        )
        .await
        .unwrap();
        assert_eq!(automation::tick(&db, &mut cursor).await.unwrap(), 1);
        let newest: Value = sqlx::query_scalar("SELECT input_refs FROM linggan_topic_research_window WHERE input_refs#>>'{coverage,coreRef}'=$1").bind(next.to_string()).fetch_one(db.pool()).await.unwrap();
        assert_eq!(newest["coverage"]["coreRef"], json!(next));
        assert_eq!(automation::tick(&db, &mut cursor).await.unwrap(), 0);
        // Freezing the newer Core must not rewrite the existing saved selection.
        let old = crate::topic_map::read_saved_alternative(&db, support::D, saved)
            .await
            .unwrap();
        assert_ne!(old.source_state, "source_unavailable");
        let latest_window:Uuid=sqlx::query_scalar("SELECT window_ref FROM linggan_topic_research_window WHERE input_refs#>>'{coverage,coreRef}'=$1").bind(next.to_string()).fetch_one(db.pool()).await.unwrap();
        support::seed_accepted_source_task(&db, latest_window).await;
        automation::definition_tests::assert_definition_repair(
            &db,
            support::D,
            latest_window,
            model_config,
            true,
        )
        .await;
    }
}

async fn seed_core(db: &Database, work: Uuid, model_config: Uuid, image_only: bool) -> Uuid {
    let reference = Uuid::new_v4();
    let mut command = Command::begin(
        &db,
        reference,
        "SYNTHETIC_CORE_FIXTURE",
        &json!({"work":work}),
    )
    .await
    .unwrap();
    let cutoff = linggan_evidence::research_dossier::cutoff_in(&mut command.tx, None)
        .await
        .unwrap();
    let packet = source::load(&mut command.tx, support::D, work, &cutoff)
        .await
        .unwrap();
    let evidence = if image_only {
        assert!(
            packet
                .snapshot
                .texts
                .iter()
                .all(|source| source.kind != "body")
        );
        json!({"text":[],"vision":[{"id":"e1","inputCitationId":packet.snapshot.images[0].citation_id,"modelQuote":null,"observation":"合成图片展示按短步骤组织作业","speakerRole":"author"}]})
    } else {
        let text = packet
            .snapshot
            .texts
            .iter()
            .find(|text| text.kind == "body")
            .unwrap();
        json!({"text":[{"id":"e1","inputCitationId":text.citation_id,"quote":text.text}],"vision":[]})
    };
    let images = if image_only {
        json!([{"inputCitationId":packet.snapshot.images[0].citation_id,"status":"used","summary":"合成步骤图","unitIds":["u1"],"evidenceRefs":["e1"]}])
    } else {
        json!([])
    };
    let draft = json!({"core":{"summary":{"text":"合成任务拆解方法","unitIds":["u1"]},"contentType":"practical_guide","mainTopicUnitId":"u1","knowledgeUnits":[{"id":"u1","unitType":"discussion","statement":"用短步骤组织作业","assertionKind":"author_claim","speakerRole":"author","evidenceRefs":["e1"]}],"candidateLabels":[]},"imageContributions":images,"evidenceCandidates":evidence});
    let accepted = contract::accept(
        draft,
        &packet.snapshot,
        &contract::Generation {
            analysis_ref: reference,
            skill_version_ref: "synthetic".into(),
            prompt_version_ref: "synthetic".into(),
            model_config_ref: model_config.to_string(),
            provider_returned_model: "synthetic-no-network".into(),
            created_at: cutoff.clone(),
        },
    )
    .unwrap();
    let (task_config, _) = configuration(&mut command.tx).await.unwrap();
    let inputs = vec![BatchInput {
        item_key: reference.to_string(),
        input: InputRef::ContentUnderstanding {
            request_ref: reference,
        },
    }];
    let batch = batch_store::insert_batch(
        &mut command.tx,
        reference,
        TASK,
        task_config,
        BatchKind::Interactive,
        &inputs,
        &batch_store::input_hash(TASK, &inputs, false).unwrap(),
    )
    .await
    .unwrap();
    let item: Uuid =
        sqlx::query_scalar("SELECT item_ref FROM linggan_ai_batch_item WHERE batch_ref=$1")
            .bind(batch)
            .fetch_one(&mut *command.tx)
            .await
            .unwrap();
    sqlx::query("INSERT INTO linggan_content_understanding_request(request_ref,command_ref,domain_ref,work_ref,material_snapshot_identity,method_version,knowledge_cutoff,input_hash,input_manifest,task_config_ref,batch_ref,item_ref) VALUES($1,$1,$2,$3,$4,$5,$6::timestamptz,$7,$8,$9,$10,$11)")
        .bind(reference).bind(support::D).bind(work).bind(&packet.snapshot.material_snapshot_identity).bind(METHOD).bind(&cutoff).bind(packet.hash()).bind(json!(packet)).bind(task_config).bind(batch).bind(item).execute(&mut *command.tx).await.unwrap();
    let invocation = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash,state,reserved_tokens,charged_tokens,result,finished_at) SELECT $1,m.connection_version_ref,m.model_ref,$2,'analyze',$3,'succeeded',0,0,'{\"synthetic\":true}',scope_001_now() FROM linggan_model_config c JOIN linggan_model_entry m USING(model_ref) WHERE c.config_ref=$2")
        .bind(invocation).bind(model_config).bind(linggan_evidence::creator_discovery::hash(&reference.to_string())).execute(&mut *command.tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_content_understanding_result(request_ref,work_ref,material_snapshot_identity,method_version,invocation_ref,result) VALUES($1,$2,$3,$4,$5,$6)")
        .bind(reference).bind(work).bind(&packet.snapshot.material_snapshot_identity).bind(METHOD).bind(invocation).bind(accepted).execute(&mut *command.tx).await.unwrap();
    command.finish(json!({"seeded":reference})).await.unwrap();
    reference
}
