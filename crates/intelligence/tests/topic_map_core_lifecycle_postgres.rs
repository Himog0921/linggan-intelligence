//! Native concurrency and authorization proofs; every model response is synthetic.
#[path = "support/comment_research_fixture.rs"]
mod comments;
#[path = "support/topic_map_core_fixture.rs"]
mod core_fixture;
use core_fixture::*;
use linggan_intelligence::{
    model_secrets::SyntheticModelSecrets,
    pi_adapter::PiAdapter,
    topic_map_research::{ResearchCommand, apply_research_command},
    topic_map_research_worker::run_once,
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use std::time::Duration;
use uuid::Uuid;

#[path = "topic_map_core_lifecycle/comparison_snapshot.rs"]
mod comparison_snapshot;

async fn run_state(db: &Database, run: Uuid) -> String {
    sqlx::query_scalar("SELECT state FROM linggan_topic_map_research_run WHERE run_ref=$1")
        .bind(run)
        .fetch_one(db.pool())
        .await
        .unwrap()
}

async fn request_counts(db: &Database, run: Uuid) -> (i64, i64, i64) {
    sqlx::query_as("SELECT count(*) FILTER(WHERE phase='extract'),count(*) FILTER(WHERE phase='resolve'),count(*) FILTER(WHERE phase='compare') FROM linggan_topic_map_research_request WHERE run_ref=$1")
        .bind(run).fetch_one(db.pool()).await.unwrap()
}

async fn initial_definition(db: &Database) -> Uuid {
    sqlx::query_scalar("SELECT definition_ref FROM linggan_topic_map_concept_rule ORDER BY created_at,definition_ref LIMIT 1")
        .fetch_one(db.pool()).await.unwrap()
}

async fn revision_in(tx: &mut Transaction<'_, Postgres>, previous: Uuid) -> Uuid {
    let definition = Uuid::new_v4();
    // This fixture changes the version/name while retaining the same complete
    // boundary. A semantic match must choose the new definition UUID exactly.
    sqlx::query("INSERT INTO linggan_topic_definition(definition_ref,topic_ref,version,display_name,definition_text,lifecycle_state) SELECT $1,topic_ref,version+1,display_name||'修订',definition_text,lifecycle_state FROM linggan_topic_definition WHERE definition_ref=$2")
        .bind(definition).bind(previous).execute(&mut **tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_topic_map_concept_rule(definition_ref,domain_ref,identity_hash,inclusion_criteria,exclusion_criteria,method_version,invocation_ref) SELECT $1,domain_ref,identity_hash,inclusion_criteria,exclusion_criteria,method_version,invocation_ref FROM linggan_topic_map_concept_rule WHERE definition_ref=$2")
        .bind(definition).bind(previous).execute(&mut **tx).await.unwrap();
    definition
}

async fn revision(db: &Database, previous: Uuid) -> Uuid {
    let mut tx = db.pool().begin().await.unwrap();
    let definition = revision_in(&mut tx, previous).await;
    tx.commit().await.unwrap();
    definition
}

#[tokio::test]
#[ignore = "isolated native PostgreSQL; synthetic adapter only"]
async fn explicit_definition_reassignment_reuses_original_run_budget_without_permanent_auto_permission()
 {
    let (db, config, adapter) = setup("topic_core_explicit_backfill").await;
    let work = work(
        &db,
        "explicit-definition-work",
        "SYNTHETIC 开始练习前需要明确第一步。",
    )
    .await;
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    let receipt = apply_research_command(&db, &start(vec![work]))
        .await
        .unwrap();
    let run: Uuid = receipt["runRef"].as_str().unwrap().parse().unwrap();
    finish_pending(&db, &adapter).await;
    assert_eq!(run_state(&db, run).await, "completed");
    assert_eq!(request_counts(&db, run).await, (1, 1, 0));
    let original = sqlx::query(
        "SELECT task_ref,distilled_json FROM linggan_topic_map_research_task WHERE run_ref=$1",
    )
    .bind(run)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let source: Uuid = original.get("task_ref");
    let distilled: Value = original.get("distilled_json");
    let budget: i64 = sqlx::query_scalar(
        "SELECT token_limit FROM linggan_topic_map_research_run WHERE run_ref=$1",
    )
    .bind(run)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let current = revision(&db, initial_definition(&db).await).await;
    finish_pending(&db, &adapter).await;
    assert_eq!(
        request_counts(&db, run).await,
        (1, 1, 0),
        "a completed on-demand run is not permanent automatic permission"
    );
    assert_eq!(run_state(&db, run).await, "completed");
    assert_eq!(sqlx::query_scalar::<_,String>("SELECT state FROM linggan_topic_map_backfill_queue WHERE source_task_ref=$1 AND definition_ref=$2")
        .bind(source).bind(current).fetch_one(db.pool()).await.unwrap(),"pending");

    let command = start(vec![work]);
    let ResearchCommand::Start { request_ref, .. } = &command else {
        unreachable!()
    };
    let receipt = apply_research_command(&db, &command).await.unwrap();
    assert_eq!(
        receipt["runRef"],
        json!(run),
        "explicit Start reuses the original budget owner"
    );
    let scope: Value = sqlx::query_scalar(
        "SELECT input_scope FROM linggan_topic_map_research_run WHERE run_ref=$1",
    )
    .bind(run)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let permission = scope["backfillAuthorizations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|permission| permission["requestRef"] == json!(request_ref))
        .unwrap();
    assert_eq!(permission["workRefs"], json!([work]));
    assert_eq!(permission["definitionRefs"], json!([current]));
    finish_pending(&db, &adapter).await;
    assert_eq!(
        request_counts(&db, run).await,
        (1, 2, 0),
        "definition changes never re-extract unchanged source text"
    );
    assert_eq!(run_state(&db, run).await, "completed");
    let reassigned = sqlx::query("SELECT t.distilled_json,t.resolutions_json,t.recall_manifest FROM linggan_topic_map_research_task t WHERE t.run_ref=$1 AND t.task_ref<>$2")
        .bind(run).bind(source).fetch_one(db.pool()).await.unwrap();
    assert_eq!(reassigned.get::<Value, _>("distilled_json"), distilled);
    assert_eq!(
        reassigned.get::<Value, _>("resolutions_json")[0]["assignments"][0]["definitionRef"],
        json!(current)
    );
    assert_eq!(
        reassigned.get::<Value, _>("recall_manifest")["backfill"]["authorizationRequestRef"],
        json!(request_ref)
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT token_limit FROM linggan_topic_map_research_run WHERE run_ref=$1"
        )
        .bind(run)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        budget
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_topic_map_research_run")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        1
    );
    let future = revision(&db, current).await;
    finish_pending(&db, &adapter).await;
    assert_eq!(
        request_counts(&db, run).await,
        (1, 2, 0),
        "an earlier explicit scope cannot authorize a later definition version"
    );
    assert_ne!(future, current);
    assert_eq!(run_state(&db, run).await, "completed");
}

async fn mark_definition_scanned(tx: &mut Transaction<'_, Postgres>, definition: Uuid) {
    sqlx::query("INSERT INTO linggan_topic_map_definition_scan(definition_ref,domain_ref,topic_ref,reason,recall_manifest) SELECT definition_ref,$2,topic_ref,CASE WHEN version=1 THEN 'new_topic' ELSE 'definition_changed' END,'{}'::jsonb FROM linggan_topic_definition WHERE definition_ref=$1 ON CONFLICT DO NOTHING")
        .bind(definition).bind(D).execute(&mut **tx).await.unwrap();
}

async fn append_resolution_in(
    tx: &mut Transaction<'_, Postgres>,
    run: Uuid,
    source: Uuid,
    definition: Uuid,
) -> Uuid {
    // Hold the same gate -> policy -> run order as the real maintenance writer.
    sqlx::query(
        "SELECT domain_ref FROM linggan_topic_map_research_policy WHERE domain_ref=$1 FOR UPDATE",
    )
    .bind(D)
    .execute(&mut **tx)
    .await
    .unwrap();
    let state: String = sqlx::query_scalar(
        "SELECT state FROM linggan_topic_map_research_run WHERE run_ref=$1 FOR UPDATE",
    )
    .bind(run)
    .fetch_one(&mut **tx)
    .await
    .unwrap();
    assert!(
        matches!(state.as_str(), "queued" | "running"),
        "a busy maintenance owner must retain an open run"
    );
    let keys: Vec<String> = sqlx::query_scalar("SELECT unit_key FROM linggan_topic_map_discussion_unit unit JOIN linggan_topic_map_unit_resolution resolution USING(unit_ref) WHERE resolution.task_ref=$1 ORDER BY unit_key")
        .bind(source).fetch_all(&mut **tx).await.unwrap();
    assert_eq!(keys.len(), 1);
    let task = Uuid::new_v4();
    let recall = json!({"backfill":{"unitKeys":keys,"forcedDefinitionRefs":[definition],"sourceTaskRef":source,"reason":"definition_changed"}});
    sqlx::query("INSERT INTO linggan_topic_map_research_task(task_ref,run_ref,domain_ref,work_public_ref,input_hash,input_refs,phase,distilled_json,resolutions_json,recall_manifest) SELECT $1,run_ref,domain_ref,work_public_ref,input_hash,input_refs,'resolve',distilled_json,'[]'::jsonb,$3 FROM linggan_topic_map_research_task WHERE task_ref=$2")
        .bind(task).bind(source).bind(recall).execute(&mut **tx).await.unwrap();
    mark_definition_scanned(tx, definition).await;
    sqlx::query("INSERT INTO linggan_topic_map_backfill_queue(definition_ref,source_task_ref,unit_keys,state,queued_task_ref,reason)VALUES($1,$2,$3,'active',$4,'definition_changed')")
        .bind(definition).bind(source).bind(keys).bind(task).execute(&mut **tx).await.unwrap();
    task
}

#[tokio::test]
#[ignore = "isolated native PostgreSQL maintenance sessions; synthetic adapter only"]
async fn busy_maintenance_gate_cannot_complete_a_run_before_its_new_resolution_commits() {
    let (db, config, adapter) = setup("topic_core_maintenance_finish_race").await;
    let work = work(
        &db,
        "maintenance-window",
        "SYNTHETIC 我需要开始练习的提醒。",
    )
    .await;
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    let receipt = apply_research_command(&db, &start(vec![work]))
        .await
        .unwrap();
    let run: Uuid = receipt["runRef"].as_str().unwrap().parse().unwrap();
    assert!(
        run_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap()
    );
    assert!(
        run_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap()
    );
    let source: Uuid = sqlx::query_scalar("SELECT task_ref FROM linggan_topic_map_research_task WHERE run_ref=$1 AND state='succeeded'")
        .bind(run).fetch_one(db.pool()).await.unwrap();
    let original = initial_definition(&db).await;
    let mut scanned = db.pool().begin().await.unwrap();
    mark_definition_scanned(&mut scanned, original).await;
    scanned.commit().await.unwrap();
    let mut owner = db.pool().begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('topic-map-backfill',120))")
        .execute(&mut *owner)
        .await
        .unwrap();
    // The worker must distinguish a gate held by another connection from an
    // empty maintenance queue; no sleep or scheduling assumption is involved.
    run_once(&db, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    assert!(matches!(
        run_state(&db, run).await.as_str(),
        "queued" | "running"
    ));
    assert_eq!(request_counts(&db, run).await, (1, 1, 0));
    let changed = revision_in(&mut owner, original).await;
    let followup = append_resolution_in(&mut owner, run, source, changed).await;
    owner.commit().await.unwrap();
    finish_pending(&db, &adapter).await;
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_topic_map_research_task WHERE task_ref=$1"
        )
        .bind(followup)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        "succeeded"
    );
    assert_eq!(request_counts(&db, run).await, (1, 2, 0));
    assert_eq!(run_state(&db, run).await, "completed");
}

fn launch(db: &Database, adapter: &PiAdapter) -> tokio::task::JoinHandle<bool> {
    let db = db.clone();
    let adapter = adapter.clone();
    tokio::spawn(async move {
        run_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap()
    })
}

async fn wait_for_policy_reservation(db: &Database, blocker: i32) {
    for _ in 0..500 {
        let blocked: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity a WHERE $1=ANY(pg_blocking_pids(a.pid)) AND a.wait_event_type='Lock' AND a.query LIKE '%SELECT p.*%' AND a.query LIKE '%FOR UPDATE OF p%')")
            .bind(blocker).fetch_one(db.pool()).await.unwrap();
        if blocked {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("worker never reached the reservation policy lock after preparing current source");
}

#[tokio::test]
#[ignore = "isolated native PostgreSQL row locks; synthetic adapter only"]
async fn withdrawal_while_reserve_waits_for_policy_never_marks_dispatch_or_charges() {
    let (db, config, adapter) = setup("topic_core_policy_wait_withdrawal").await;
    let work = work(&db, "policy-wait-comment", "").await;
    let comment = comments::comment_with_author(
        &db,
        "policy-wait-comment",
        "wait-withdrawal",
        "SYNTHETIC 开始练习时需要解释第一步。",
        Some("synthetic-reader"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    let receipt = apply_research_command(&db, &start(vec![work]))
        .await
        .unwrap();
    let run: Uuid = receipt["runRef"].as_str().unwrap().parse().unwrap();
    let mut policy = db.pool().begin().await.unwrap();
    let blocker: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *policy)
        .await
        .unwrap();
    sqlx::query(
        "SELECT domain_ref FROM linggan_topic_map_research_policy WHERE domain_ref=$1 FOR UPDATE",
    )
    .bind(D)
    .execute(&mut *policy)
    .await
    .unwrap();
    let worker = launch(&db, &adapter);
    wait_for_policy_reservation(&db, blocker).await;
    assert!(!worker.is_finished());
    assert_eq!(request_counts(&db, run).await, (0, 0, 0));
    sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason)SELECT content_public_ref,comment_external_id,'synthetic withdrawal after fresh preparation' FROM linggan_material_comment WHERE material_ref=$1")
        .bind(comment).execute(db.pool()).await.unwrap();
    policy.commit().await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_secs(10), worker)
            .await
            .unwrap()
            .unwrap()
    );
    let requests = sqlx::query("SELECT q.dispatch_started_at IS NULL AS unsent,i.charged_tokens,i.failure_code FROM linggan_topic_map_research_request q JOIN linggan_model_invocation i USING(invocation_ref) WHERE q.run_ref=$1")
        .bind(run).fetch_all(db.pool()).await.unwrap();
    assert!(
        requests.len() <= 1,
        "the blocked source can reserve at most its prepared attempt"
    );
    for request in requests {
        assert!(request.get::<bool, _>("unsent"));
        assert_eq!(request.get::<i64, _>("charged_tokens"), 0);
        assert_eq!(
            request.get::<String, _>("failure_code"),
            "pre_dispatch_source_changed"
        );
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_topic_map_research_result WHERE work_public_ref=$1"
        )
        .bind(work)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_topic_map_research_task WHERE run_ref=$1"
        )
        .bind(run)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        "stale"
    );
}
