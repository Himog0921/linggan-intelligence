#[allow(dead_code)]
#[path = "support/material_fixture.rs"]
mod fixture;

use fixture::proof_database;
use linggan_evidence::{
    TargetInspectorAction, TargetInspectorCount, TargetInspectorExecutionState,
    TargetInspectorPatrolState, read_target_inspector,
};
use linggan_storage_postgres::Database;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL 16 proof database"]
async fn inspector_separates_queued_from_running_and_keeps_reads_side_effect_free() {
    let database = proof_database("target_inspector_execution_truth").await;
    sqlx::query(
        "CREATE OR REPLACE FUNCTION scope_001_now() RETURNS timestamptz LANGUAGE sql VOLATILE \
         AS $$ SELECT timestamptz '2026-09-08T08:00:00Z' $$",
    )
    .execute(database.pool())
    .await
    .unwrap();
    let fixture = seed_queued_archive(&database).await;
    let before = fact_counts(&database).await;

    let queued = read_target_inspector(&database, fixture.target_ref)
        .await
        .unwrap()
        .expect("the target exists");
    assert_eq!(
        queued.execution.state,
        TargetInspectorExecutionState::Queued
    );
    assert_eq!(queued.execution.queued_work_orders, 1);
    assert_eq!(queued.execution.running_attempts, 0);
    assert_eq!(
        queued.required_action,
        TargetInspectorAction::NoActionQueued
    );
    assert_eq!(
        queued.coverage.directory_works,
        TargetInspectorCount::Known(0)
    );
    assert_eq!(
        queued.coverage.missing_details,
        TargetInspectorCount::Known(0)
    );
    assert_eq!(queued.patrol.latest_hits, TargetInspectorCount::Unknown);
    assert_eq!(queued.patrol.state, TargetInspectorPatrolState::Disabled);
    assert_ne!(queued.coverage.directory_works, queued.patrol.latest_hits);
    assert_eq!(
        fact_counts(&database).await,
        before,
        "the read projection must not write"
    );

    start_attempt(&database, &fixture).await;
    let running_before = fact_counts(&database).await;
    let running = read_target_inspector(&database, fixture.target_ref)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        running.execution.state,
        TargetInspectorExecutionState::Running
    );
    assert_eq!(running.execution.queued_work_orders, 0);
    assert_eq!(running.execution.running_attempts, 1);
    assert_eq!(
        running.required_action,
        TargetInspectorAction::NoActionRunning
    );
    assert_eq!(
        fact_counts(&database).await,
        running_before,
        "the read projection must remain read-only"
    );
}

struct Fixture {
    target_ref: Uuid,
    work_order_ref: Uuid,
    station_ref: Uuid,
}

async fn seed_queued_archive(database: &Database) -> Fixture {
    let target_ref = Uuid::new_v4();
    let authorization_ref = Uuid::new_v4();
    let request_ref = Uuid::new_v4();
    let decision_ref = Uuid::new_v4();
    let work_order_ref = Uuid::new_v4();
    let station_ref = Uuid::new_v4();
    sqlx::query("INSERT INTO collection_observation_target (target_ref,platform,target_kind,identity_key,display_name,source,lifecycle_state,domain_ref) VALUES ($1,'xhs','creator','inspector-author','Inspector fixture','manual','archiving','00000000-0000-4000-8000-000000000001')")
        .bind(target_ref).execute(database.pool()).await.unwrap();
    sqlx::query("INSERT INTO collection_acquisition_authorization (authorization_ref,platform,target_kind,lane,max_targets,max_works_per_target,allowed_task_templates,allowed_dispatch_lanes,max_work_units,purpose,granted_by,expires_at) VALUES ($1,'xhs','creator','deep_archive',1,200,ARRAY['creator_archive'],ARRAY['batch'],200,'inspector proof','person',scope_001_now()+interval '1 day')")
        .bind(authorization_ref).execute(database.pool()).await.unwrap();
    sqlx::query("INSERT INTO collection_acquisition_request (request_ref,target_ref,lane,purpose,requested_by) VALUES ($1,$2,'deep_archive','inspector proof','person')")
        .bind(request_ref).bind(target_ref).execute(database.pool()).await.unwrap();
    sqlx::query("INSERT INTO collection_admission_decision (decision_ref,request_ref,outcome,reason_code,authorization_ref,target_ref) VALUES ($1,$2,'admitted','inspector_proof',$3,$4)")
        .bind(decision_ref).bind(request_ref).bind(authorization_ref).bind(target_ref).execute(database.pool()).await.unwrap();
    sqlx::query("INSERT INTO execution_station (station_ref,display_name,daily_work_quota) VALUES ($1,'inspector proof station',200)")
        .bind(station_ref).execute(database.pool()).await.unwrap();
    sqlx::query("INSERT INTO collection_work_order (work_order_ref,decision_ref,target_ref,lane,max_works,stop_conditions,dispatch_lane,queue_state,scheduled_for,estimated_work_units,station_ref) VALUES ($1,$2,$3,'deep_archive',200,'{\"progressiveArchive\":{\"version\":1}}'::jsonb,'batch','queued',scope_001_now(),1,$4)")
        .bind(work_order_ref).bind(decision_ref).bind(target_ref).bind(station_ref).execute(database.pool()).await.unwrap();
    Fixture {
        target_ref,
        work_order_ref,
        station_ref,
    }
}

async fn start_attempt(database: &Database, fixture: &Fixture) {
    let lease_ref = Uuid::new_v4();
    let task_id = Uuid::new_v4();
    let producer_instance_id = Uuid::new_v4();
    sqlx::query("UPDATE collection_work_order SET queue_state='leased' WHERE work_order_ref=$1")
        .bind(fixture.work_order_ref)
        .execute(database.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO linggan_runtime_task (task_id,task_spec_hash,task_spec,source,platform,page_type) VALUES ($1,$2,$3,'scheduled','xhs','profile')")
        .bind(task_id).bind(format!("{:064x}", 1)).bind(serde_json::json!({"capabilitiesRequested":["author_profile"],"target":{"authorExternalId":"inspector-author"}})).execute(database.pool()).await.unwrap();
    sqlx::query("INSERT INTO collection_work_order_lease (lease_ref,work_order_ref,station_ref,capture_identity,expires_at) VALUES ($1,$2,$3,'{}'::jsonb,scope_001_now()+interval '1 hour')")
        .bind(lease_ref).bind(fixture.work_order_ref).bind(fixture.station_ref).execute(database.pool()).await.unwrap();
    sqlx::query("INSERT INTO collection_work_order_lease_task (lease_ref,task_id,sequence_no,execution_state,claimed_at) VALUES ($1,$2,1,'in_progress',scope_001_now())")
        .bind(lease_ref).bind(task_id).execute(database.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_runtime_attempt (attempt_id,task_id,producer_instance_id) VALUES ($1,$2,$3)")
        .bind(Uuid::new_v4()).bind(task_id).bind(producer_instance_id).execute(database.pool()).await.unwrap();
}

async fn fact_counts(database: &Database) -> (i64, i64, i64, i64, i64) {
    sqlx::query_as(
        "SELECT (SELECT count(*) FROM collection_work_order), \
                (SELECT count(*) FROM collection_work_order_lease), \
                (SELECT count(*) FROM collection_work_order_lease_task), \
                (SELECT count(*) FROM linggan_runtime_attempt), \
                (SELECT count(*) FROM linggan_runtime_capture_package)",
    )
    .fetch_one(database.pool())
    .await
    .unwrap()
}
