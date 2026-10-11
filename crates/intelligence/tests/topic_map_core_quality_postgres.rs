//! Handwritten quality scheduling proofs against disposable PostgreSQL only.
#[path = "support/topic_map_core_fixture.rs"]
mod core_fixture;
use core_fixture::*;
use linggan_intelligence::{
    model_secrets::SyntheticModelSecrets,
    topic_map_research::{apply_research_command, read_research_progress},
    topic_map_research_worker::run_once,
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

async fn count(db: &Database, query: &'static str) -> i64 {
    sqlx::query_scalar(query)
        .fetch_one(db.pool())
        .await
        .unwrap()
}

async fn prepared_source(
    db: &Database,
    config: Uuid,
    adapter: &linggan_intelligence::pi_adapter::PiAdapter,
) -> Uuid {
    let source = work(
        db,
        "quality-original",
        "SYNTHETIC 开始练习前需要明确第一步。",
    )
    .await;
    apply_research_command(db, &configure(config, 100000, false))
        .await
        .unwrap();
    apply_research_command(db, &start(vec![source]))
        .await
        .unwrap();
    finish_pending(db, adapter).await;
    sqlx::query_scalar("SELECT task_ref FROM linggan_topic_map_research_task WHERE work_public_ref=$1 AND state='succeeded' AND NOT (recall_manifest ? 'backfill')")
        .bind(source).fetch_one(db.pool()).await.unwrap()
}

/// These are independently available human definitions, not invented machine lineage.
async fn add_definitions(db: &Database, amount: usize) -> Vec<Uuid> {
    add_definitions_at(db, amount, None).await
}

async fn add_definitions_at(db: &Database, amount: usize, created_at: Option<String>) -> Vec<Uuid> {
    let base = sqlx::query("SELECT d.definition_ref,b.receipt_ref FROM linggan_topic_definition d JOIN linggan_topic_map_binding b USING(topic_ref) LEFT JOIN linggan_topic_map_concept_rule rule USING(definition_ref) WHERE COALESCE(rule.method_version,'')<>'topic-map.core.parent.v1' ORDER BY d.created_at LIMIT 1")
        .fetch_one(db.pool()).await.unwrap();
    let mut definitions = Vec::new();
    for index in 0..amount {
        let topic = Uuid::new_v4();
        let definition = Uuid::new_v4();
        sqlx::query("INSERT INTO linggan_topic_workspace(topic_ref,domain_key,canonical_key)VALUES($1,$2,$3)")
            .bind(topic).bind(format!("domain-{}",D.simple())).bind(format!("quality-{}",topic.simple())).execute(db.pool()).await.unwrap();
        sqlx::query("INSERT INTO linggan_topic_definition(definition_ref,topic_ref,version,display_name,definition_text,lifecycle_state,created_at) SELECT $1,$2,1,display_name||$4,definition_text||$4,'candidate',COALESCE($5::timestamptz,scope_001_now()) FROM linggan_topic_definition WHERE definition_ref=$3")
            .bind(definition).bind(topic).bind(base.get::<Uuid,_>("definition_ref")).bind(format!(" 合成边界{index}")).bind(created_at.clone()).execute(db.pool()).await.unwrap();
        sqlx::query("INSERT INTO linggan_topic_map_binding(binding_ref,topic_ref,domain_ref,version,receipt_ref)VALUES($1,$2,$3,1,$4)")
            .bind(Uuid::new_v4()).bind(topic).bind(D).bind(base.get::<Uuid,_>("receipt_ref")).execute(db.pool()).await.unwrap();
        definitions.push(definition);
    }
    definitions
}

#[tokio::test]
#[ignore = "isolated native PostgreSQL; synthetic adapter only"]
async fn several_definitions_share_one_reassessment_and_never_inflate_initial_coverage() {
    let (db, config, adapter) = setup("topic_core_quality_coalescing").await;
    let source = prepared_source(&db, config, &adapter).await;
    let before = read_research_progress(&db, D).await.unwrap()["summary"].clone();
    let definitions = add_definitions(&db, 3).await;
    apply_research_command(&db, &configure(config, 100000, true))
        .await
        .unwrap();
    run_once(&db, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    let maintenance = sqlx::query("SELECT task_ref,recall_manifest,state FROM linggan_topic_map_research_task WHERE recall_manifest ? 'backfill'").fetch_all(db.pool()).await.unwrap();
    assert_eq!(
        maintenance.len(),
        1,
        "three definition events reuse one source task"
    );
    let recall: Value = maintenance[0].get("recall_manifest");
    assert_eq!(recall["backfill"]["sourceTaskRef"], json!(source));
    let refs = recall["backfill"]["forcedDefinitionRefs"]
        .as_array()
        .unwrap();
    assert_eq!(refs.len(), definitions.len());
    assert!(definitions.iter().all(|id| refs.contains(&json!(id))));
    assert_eq!(maintenance[0].get::<String, _>("state"), "succeeded");
    run_once(&db, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    assert_eq!(count(&db,"SELECT count(DISTINCT queued_task_ref) FROM linggan_topic_map_backfill_queue WHERE state='completed'").await, 1);
    assert_eq!(count(&db,"SELECT count(*) FROM linggan_topic_map_research_request request JOIN linggan_topic_map_research_task task USING(task_ref) WHERE task.recall_manifest ? 'backfill'").await, 1);
    let after = read_research_progress(&db, D).await.unwrap()["summary"].clone();
    for field in [
        "initialWindowCount",
        "completedInitialWindowCount",
        "coveredWorkCount",
        "extractedWindowCount",
    ] {
        assert_eq!(
            before[field],
            json!(1),
            "one original source window: {field}"
        );
        assert_eq!(
            after[field], before[field],
            "maintenance does not create new coverage: {field}"
        );
    }
    assert_eq!(after["reassessmentCompletedCount"], json!(1));
}

#[tokio::test]
#[ignore = "isolated native PostgreSQL; synthetic adapter only"]
async fn definition_maintenance_and_fresh_material_both_receive_real_dispatch_slots() {
    let (db, config, adapter) = setup("topic_core_quality_fairness").await;
    prepared_source(&db, config, &adapter).await;
    add_definitions(&db, 3).await;
    let fresh = work(
        &db,
        "quality-new-material",
        &"SYNTHETIC 开始练习的新材料。".repeat(400),
    )
    .await;
    apply_research_command(&db, &configure(config, 100000, true))
        .await
        .unwrap();
    apply_research_command(&db, &start(vec![fresh]))
        .await
        .unwrap();
    run_once(&db, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    let newest = sqlx::query("SELECT t.work_public_ref,t.recall_manifest,q.phase FROM linggan_topic_map_research_request q JOIN linggan_topic_map_research_task t USING(task_ref) ORDER BY q.created_at DESC LIMIT 1")
        .fetch_one(db.pool()).await.unwrap();
    assert_eq!(
        newest.get::<Uuid, _>("work_public_ref"),
        fresh,
        "maintenance must not win every resolve-priority slot"
    );
    assert_eq!(newest.get::<String, _>("phase"), "extract");
    // Events are old enough to enter a maintenance slot while fresh work remains.
    sqlx::query("UPDATE linggan_topic_map_backfill_queue SET created_at=scope_001_now()-interval '16 minutes' WHERE state='pending'")
        .execute(db.pool()).await.unwrap();
    for _ in 0..3 {
        run_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap();
    }
    assert!(count(&db,"SELECT count(*) FROM linggan_topic_map_research_request q JOIN linggan_topic_map_research_task t USING(task_ref) WHERE t.recall_manifest ? 'backfill'").await > 0, "maintenance gets a bounded slot too");
    assert!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM linggan_topic_map_research_request q JOIN linggan_topic_map_research_task t USING(task_ref) WHERE t.work_public_ref=$1")
        .bind(fresh).fetch_one(db.pool()).await.unwrap()>1, "new source understanding continues after maintenance");
    assert!(count(&db,"SELECT count(*) FROM linggan_topic_map_research_task WHERE state IN ('queued','running') AND recall_manifest ? 'backfill'").await <=2);
}

#[tokio::test]
#[ignore = "isolated native PostgreSQL; synthetic adapter only"]
async fn coalesced_maintenance_cannot_bypass_the_original_run_budget() {
    let (db, config, adapter) = setup("topic_core_quality_budget").await;
    let source = prepared_source(&db, config, &adapter).await;
    add_definitions(&db, 3).await;
    let charges = count(&db,"SELECT sum(charged_tokens)::bigint FROM linggan_model_invocation WHERE operation='analyze'").await;
    let calls = count(
        &db,
        "SELECT count(*) FROM linggan_topic_map_research_request",
    )
    .await;
    // Keep the original known fees. The 0117 minimum of 1,024 tokens leaves only 226
    // tokens left, insufficient for another bounded resolve reservation.
    assert_eq!(charges, 798);
    sqlx::query("UPDATE linggan_topic_map_research_run SET token_limit=1024 WHERE run_ref=(SELECT run_ref FROM linggan_topic_map_research_task WHERE task_ref=$1)")
        .bind(source).execute(db.pool()).await.unwrap();
    apply_research_command(&db, &configure(config, 100000, true))
        .await
        .unwrap();
    run_once(&db, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    assert_eq!(
        count(
            &db,
            "SELECT count(*) FROM linggan_topic_map_research_request"
        )
        .await,
        calls
    );
    assert_eq!(count(&db,"SELECT sum(charged_tokens)::bigint FROM linggan_model_invocation WHERE operation='analyze'").await, charges);
    assert_eq!(
        count(
            &db,
            "SELECT count(*) FROM linggan_topic_map_research_run WHERE state='run_budget_exhausted'"
        )
        .await,
        1
    );
}

#[tokio::test]
#[ignore = "isolated native PostgreSQL; synthetic adapter only"]
async fn an_unknown_source_dispatch_blocks_all_coalesced_definition_resends() {
    let (db, config, adapter) = setup("topic_core_quality_unknown").await;
    let source = prepared_source(&db, config, &adapter).await;
    add_definitions(&db, 3).await;
    let calls = count(
        &db,
        "SELECT count(*) FROM linggan_topic_map_research_request",
    )
    .await;
    sqlx::query("INSERT INTO linggan_topic_map_research_task(task_ref,run_ref,domain_ref,work_public_ref,input_hash,input_refs,state,last_reason) SELECT $1,run_ref,domain_ref,work_public_ref,input_hash,input_refs,'unknown_dispatch','unknown_dispatch' FROM linggan_topic_map_research_task WHERE task_ref=$2")
        .bind(Uuid::new_v4()).bind(source).execute(db.pool()).await.unwrap();
    apply_research_command(&db, &configure(config, 100000, true))
        .await
        .unwrap();
    run_once(&db, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    assert_eq!(
        count(
            &db,
            "SELECT count(*) FROM linggan_topic_map_research_request"
        )
        .await,
        calls
    );
    assert_eq!(count(&db,"SELECT count(*) FROM linggan_topic_map_backfill_queue WHERE state='skipped' AND last_reason='unknown_dispatch'").await, 3);
    assert_eq!(
        count(
            &db,
            "SELECT count(*) FROM linggan_topic_map_research_task WHERE state='unknown_dispatch'"
        )
        .await,
        1
    );
}

#[tokio::test]
#[ignore = "isolated native PostgreSQL; synthetic adapter only"]
async fn coalescing_cannot_borrow_live_permission_for_an_older_ungranted_definition() {
    let (db, config, adapter) = setup("topic_core_quality_scope").await;
    let source = prepared_source(&db, config, &adapter).await;
    // Freeze the older timestamp on insertion: definitions are append-only.
    let older_at: String = sqlx::query_scalar("SELECT (run.created_at-interval '1 second')::text FROM linggan_topic_map_research_run run JOIN linggan_topic_map_research_task task USING(run_ref) WHERE task.task_ref=$1")
        .bind(source).fetch_one(db.pool()).await.unwrap();
    let mut definitions = add_definitions_at(&db, 1, Some(older_at)).await;
    definitions.extend(add_definitions(&db, 1).await);
    // Model a still-open on-demand owner with a newly created definition and
    // an unrelated older event. No explicit reassessment grant exists.
    sqlx::query("UPDATE linggan_topic_map_research_run SET state='queued' WHERE run_ref=(SELECT run_ref FROM linggan_topic_map_research_task WHERE task_ref=$1)")
        .bind(source).execute(db.pool()).await.unwrap();
    run_once(&db, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    let recall: Value = sqlx::query_scalar("SELECT recall_manifest FROM linggan_topic_map_research_task WHERE recall_manifest ? 'backfill'")
        .fetch_one(db.pool()).await.unwrap();
    assert_eq!(
        recall["backfill"]["forcedDefinitionRefs"],
        json!([definitions[1]])
    );
    let state: String = sqlx::query_scalar("SELECT state FROM linggan_topic_map_backfill_queue WHERE source_task_ref=$1 AND definition_ref=$2")
        .bind(source).bind(definitions[0]).fetch_one(db.pool()).await.unwrap();
    assert_eq!(
        state, "pending",
        "an absent grant is not matching permission"
    );
}

#[tokio::test]
#[ignore = "isolated native PostgreSQL; synthetic adapter only"]
async fn sequential_new_definitions_accumulate_before_one_bounded_reassessment() {
    let (db, config, adapter) = setup("topic_core_quality_debounce").await;
    let source = prepared_source(&db, config, &adapter).await;
    let fresh = work(
        &db,
        "quality-many-new-windows",
        &"SYNTHETIC 开始练习的新材料。".repeat(2000),
    )
    .await;
    apply_research_command(&db, &configure(config, 100000, true))
        .await
        .unwrap();
    apply_research_command(&db, &start(vec![fresh]))
        .await
        .unwrap();
    let mut expected = Vec::new();
    for index in 0..6 {
        expected.extend(add_definitions(&db, 1).await);
        run_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap();
        let calls = count(&db,"SELECT count(*) FROM linggan_topic_map_research_request q JOIN linggan_topic_map_research_task t USING(task_ref) WHERE t.recall_manifest ? 'backfill'").await;
        if index < 5 {
            assert_eq!(
                calls, 0,
                "one new event each tick is retained instead of sent immediately"
            );
            assert_eq!(count(&db,"SELECT count(*) FROM linggan_topic_map_research_task WHERE recall_manifest ? 'backfill'").await, 0);
        } else {
            assert_eq!(
                calls, 1,
                "six accumulated definition events share one actual call"
            );
        }
    }
    let recall: Value = sqlx::query_scalar("SELECT recall_manifest FROM linggan_topic_map_research_task WHERE recall_manifest ? 'backfill'")
        .fetch_one(db.pool()).await.unwrap();
    assert_eq!(recall["backfill"]["sourceTaskRef"], json!(source));
    let refs = recall["backfill"]["forcedDefinitionRefs"]
        .as_array()
        .unwrap();
    assert_eq!(refs.len(), 6);
    assert!(
        expected
            .iter()
            .all(|definition| refs.contains(&json!(definition)))
    );
    assert!(count(&db,"SELECT count(*) FROM linggan_topic_map_research_task WHERE state='queued' AND NOT (recall_manifest ? 'backfill')").await >0, "the debounce ends before the whole first-coverage backlog drains");
}
