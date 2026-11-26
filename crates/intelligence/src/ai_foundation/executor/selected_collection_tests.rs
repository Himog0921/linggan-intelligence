use super::*;
use crate::ai_foundation::{
    batch_store::{self, BatchInput, BatchKind},
    content_understanding::topic_tests::configure_tasks_for_test,
    executor::ExecutorSession,
    store::Command,
};
#[allow(dead_code)]
#[path = "../../topic_map_research/tests/saved_source_fixture.rs"]
mod support;

#[tokio::test]
#[ignore = "owned isolated PostgreSQL; no model dispatch"]
async fn selected_collection_claim_skips_backlog_and_survives_worker_restart() {
    let (db, model) = support::setup("selected_collection_ai_claim").await;
    let model_config = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_config(config_ref,model_ref,capability_ref,input_token_limit,output_token_limit,timeout_seconds,max_attempts,thinking_level) SELECT $1,model_ref,capability_ref,input_token_limit,output_token_limit,timeout_seconds,max_attempts,'high' FROM linggan_model_config WHERE config_ref=$2")
        .bind(model_config).bind(model).execute(db.pool()).await.unwrap();
    configure_tasks_for_test(&db, model_config).await;
    let outside = support::work(&db, "old-queued-work", "SYNTHETIC unrelated source").await;
    let selected = support::work(&db, "selected-work", "SYNTHETIC selected source").await;
    let collection_run = Uuid::new_v4();
    let old_l1 = queue_l1(&db, outside, None).await;
    let old = queue_topic(&db, outside, None, "[]").await;
    let old_selected = queue_topic(&db, selected, None, "[]").await;
    let mislabeled = queue_topic(&db, outside, Some(collection_run), "[]").await;
    let comment = queue_topic(&db, selected, Some(collection_run), "[{}]").await;
    let approved = queue_topic(&db, selected, Some(collection_run), "[]").await;
    let selected_l1 = queue_l1(&db, selected, Some(collection_run)).await;
    let mut owner = ExecutorSession::acquire(
        &db,
        "synthetic-before-restart",
        AI_LIMITS.default_concurrency,
    )
    .await
    .unwrap();
    let fence = owner.fence();
    let mut filter = selected_collection_scope::Filter {
        collection_run_ref: Some(collection_run),
        domain: Some(support::D),
        works: Some(vec![]),
    };
    assert!(
        claim_in(fence.begin(&db, true).await.unwrap(), fence, false, &filter)
            .await
            .unwrap()
            .is_none(),
        "without a fresh accepted work no item can claim"
    );
    filter.works = Some(vec![selected]);
    let item = claim_in(fence.begin(&db, true).await.unwrap(), fence, false, &filter)
        .await
        .unwrap()
        .expect("selected author Core remains claimable after higher-priority exclusions");
    assert_eq!(item.item_ref, approved);
    let mut tx = fence.begin(&db, true).await.unwrap();
    for denied in [old_l1, old, old_selected, mislabeled, comment] {
        assert!(
            !selected_item_allowed_in(&mut tx, denied, &filter)
                .await
                .unwrap()
        );
        let state: String =
            sqlx::query_scalar("SELECT state FROM linggan_ai_batch_item_control WHERE item_ref=$1")
                .bind(denied)
                .fetch_one(&mut *tx)
                .await
                .unwrap();
        assert_eq!(state, "pending");
    }
    assert!(
        selected_item_allowed_in(&mut tx, approved, &filter)
            .await
            .unwrap()
    );
    assert!(
        selected_item_allowed_in(&mut tx, selected_l1, &filter)
            .await
            .unwrap()
    );
    assert!(
        !selected_item_allowed_in(
            &mut tx,
            approved,
            &selected_collection_scope::Filter::default()
        )
        .await
        .unwrap(),
        "unscoped restart cannot send a selected run's leftover jobs"
    );
    tx.commit().await.unwrap();
    item.defer(&db, "executor_draining").await.unwrap();
    owner.request_drain().await.unwrap();
    owner.complete_drain().await.unwrap();
    drop(owner);

    // Wait out the production deferral instead of changing job timestamps.
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    let owner = ExecutorSession::acquire(
        &db,
        "synthetic-after-restart",
        AI_LIMITS.default_concurrency,
    )
    .await
    .unwrap();
    let next_fence = owner.fence();
    assert!(next_fence.generation() > fence.generation());
    let retried = claim_in(
        next_fence.begin(&db, true).await.unwrap(),
        next_fence,
        false,
        &filter,
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(retried.item_ref, approved);
    assert!(retried.lease_generation > item.lease_generation);
}

async fn queue_topic(
    db: &Database,
    work: Uuid,
    collection_run: Option<Uuid>,
    comments: &str,
) -> Uuid {
    let command_ref = Uuid::new_v4();
    let mut command = Command::begin(
        db,
        command_ref,
        "synthetic_selected_topic",
        &json!({"work":work}),
    )
    .await
    .unwrap();
    let config: Uuid = sqlx::query_scalar(
        "SELECT current_config_ref FROM linggan_ai_task_setting WHERE task_key='topic_research'",
    )
    .fetch_one(&mut *command.tx)
    .await
    .unwrap();
    let run = Uuid::new_v4();
    let window = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_topic_research_run(run_ref,command_ref,domain_ref,work_refs,knowledge_cutoff,configurations) VALUES($1,$2,$3,$4,scope_001_now(),$5)")
        .bind(run).bind(command_ref).bind(support::D).bind(vec![work]).bind(json!([config])).execute(&mut *command.tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_topic_research_window(window_ref,run_ref,work_ref,input_hash,input_refs) VALUES($1,$2,$3,repeat('0',64),$4)")
        .bind(window).bind(run).bind(work).bind(json!({"executionMode":"single_pass","inputContract":"topic-map.core-source.v1","commentStudy":serde_json::from_str::<serde_json::Value>(comments).unwrap()})).execute(&mut *command.tx).await.unwrap();
    let inputs = vec![BatchInput {
        item_key: window.to_string(),
        input: InputRef::TopicResearch {
            window_ref: window,
            stage_number: 0,
            collection_run_ref: collection_run,
        },
    }];
    let hash = batch_store::input_hash(TaskKey::TopicResearch, &inputs, false).unwrap();
    let batch = batch_store::insert_batch(
        &mut command.tx,
        command_ref,
        TaskKey::TopicResearch,
        config,
        BatchKind::Automatic,
        &inputs,
        &hash,
    )
    .await
    .unwrap();
    let item: Uuid =
        sqlx::query_scalar("SELECT item_ref FROM linggan_ai_batch_item WHERE batch_ref=$1")
            .bind(batch)
            .fetch_one(&mut *command.tx)
            .await
            .unwrap();
    sqlx::query("INSERT INTO linggan_topic_research_stage(item_ref,window_ref,stage_number) VALUES($1,$2,0)")
        .bind(item).bind(window).execute(&mut *command.tx).await.unwrap();
    command.finish(json!({"itemRef":item})).await.unwrap();
    item
}

async fn queue_l1(db: &Database, work: Uuid, collection_run: Option<Uuid>) -> Uuid {
    let command_ref = Uuid::new_v4();
    let mut command = Command::begin(
        db,
        command_ref,
        "synthetic_selected_l1",
        &json!({"work":work}),
    )
    .await
    .unwrap();
    let config: Uuid = sqlx::query_scalar(
        "SELECT current_config_ref FROM linggan_ai_task_setting WHERE task_key='content_understanding'",
    )
    .fetch_one(&mut *command.tx)
    .await
    .unwrap();
    let request = Uuid::new_v4();
    let inputs = vec![BatchInput {
        item_key: request.to_string(),
        input: InputRef::ContentUnderstanding {
            request_ref: request,
            collection_run_ref: collection_run,
        },
    }];
    let task = TaskKey::ContentUnderstanding;
    let hash = batch_store::input_hash(task, &inputs, false).unwrap();
    let batch = batch_store::insert_batch(
        &mut command.tx,
        command_ref,
        task,
        config,
        BatchKind::Automatic,
        &inputs,
        &hash,
    )
    .await
    .unwrap();
    let item: Uuid =
        sqlx::query_scalar("SELECT item_ref FROM linggan_ai_batch_item WHERE batch_ref=$1")
            .bind(batch)
            .fetch_one(&mut *command.tx)
            .await
            .unwrap();
    sqlx::query("INSERT INTO linggan_content_understanding_request(request_ref,command_ref,domain_ref,work_ref,material_snapshot_identity,method_version,knowledge_cutoff,input_hash,input_manifest,task_config_ref,batch_ref,item_ref) VALUES($1,$2,$3,$4,$5,$6,scope_001_now(),$5,'{}',$7,$8,$9)")
        .bind(request).bind(command_ref).bind(support::D).bind(work).bind(hash).bind(crate::content_understanding::METHOD)
        .bind(config).bind(batch).bind(item).execute(&mut *command.tx).await.unwrap();
    command.finish(json!({"itemRef":item})).await.unwrap();
    item
}
