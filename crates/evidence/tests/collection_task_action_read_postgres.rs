//! Real PostgreSQL proofs for the two task-action read projections.
//!
//! Fixtures write only inside the disposable proof schema. They do not run admission,
//! dispatch, a producer or a plugin; these tests prove reads, not collection execution.

#[allow(dead_code)]
#[path = "support/material_fixture.rs"]
mod fixture;

use fixture::proof_database;
use linggan_evidence::{read_manual_observation_primary_domains, read_manual_observation_receipt};
use linggan_storage_postgres::Database;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn active_primary_purposes_are_target_scoped_and_bounded_at_two() {
    let database = proof_database("task_action_primary_purposes").await;
    let target_ref = seed_target(&database, "primary-purpose-target").await;
    let other_target_ref = seed_target(&database, "other-purpose-target").await;
    seed_purpose(
        &database,
        target_ref,
        40,
        "仅参照用途",
        "active",
        "reference",
        "2026-09-01",
    )
    .await;
    seed_purpose(
        &database,
        target_ref,
        50,
        "暂停用途",
        "paused",
        "primary",
        "2026-09-01",
    )
    .await;
    seed_purpose(
        &database,
        other_target_ref,
        60,
        "另一目标用途",
        "active",
        "primary",
        "2026-09-01",
    )
    .await;
    assert!(
        read_manual_observation_primary_domains(&database, target_ref)
            .await
            .unwrap()
            .is_empty(),
        "reference, paused and another target's primary purposes must not enter this read result"
    );

    seed_purpose(
        &database,
        target_ref,
        20,
        "后建用途",
        "active",
        "primary",
        "2026-09-02",
    )
    .await;
    assert_eq!(
        read_manual_observation_primary_domains(&database, target_ref)
            .await
            .unwrap(),
        vec!["后建用途"]
    );
    seed_purpose(
        &database,
        target_ref,
        30,
        "先建用途",
        "active",
        "primary",
        "2026-09-01",
    )
    .await;
    assert_eq!(
        read_manual_observation_primary_domains(&database, target_ref)
            .await
            .unwrap(),
        vec!["先建用途", "后建用途"],
        "two active primary purposes must remain ambiguous, in creation order"
    );
    seed_purpose(
        &database,
        target_ref,
        10,
        "同刻优先用途",
        "active",
        "primary",
        "2026-09-01",
    )
    .await;
    assert_eq!(
        read_manual_observation_primary_domains(&database, target_ref)
            .await
            .unwrap(),
        vec!["同刻优先用途", "先建用途"],
        "ambiguity is bounded at two, with the domain UUID breaking timestamp ties"
    );
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn receipt_requires_the_exact_target_id_and_manual_command() {
    let database = proof_database("task_action_exact_manual_receipt").await;
    let target_ref = seed_target(&database, "manual-receipt-target").await;
    let other_target_ref = seed_target(&database, "other-receipt-target").await;
    let domain_ref = seed_purpose(
        &database,
        target_ref,
        100,
        "回执用途",
        "active",
        "primary",
        "2026-09-01",
    )
    .await;
    let work_order_ref = seed_work_order(&database, target_ref, domain_ref).await;
    let selected = seed_receipt(
        &database,
        target_ref,
        "manual_observe",
        "applied",
        "manual_observe_created",
        Some(work_order_ref),
        "2026-10-10T02:05:06Z",
    )
    .await;
    let newer = seed_receipt(
        &database,
        target_ref,
        "manual_observe",
        "rejected",
        "authorization_missing",
        None,
        "2026-10-11T02:05:06Z",
    )
    .await;
    let wrong_kind = seed_receipt(
        &database,
        target_ref,
        "pause",
        "applied",
        "monitor_paused",
        None,
        "2026-10-12T02:05:06Z",
    )
    .await;
    let other_receipt = seed_receipt(
        &database,
        other_target_ref,
        "manual_observe",
        "rejected",
        "authorization_missing",
        None,
        "2026-10-12T02:05:06Z",
    )
    .await;

    let receipt = read_manual_observation_receipt(&database, target_ref, selected)
        .await
        .unwrap()
        .expect("the exact older manual receipt is readable");
    assert_eq!(receipt.receipt_ref, selected);
    assert_eq!(receipt.outcome, "applied");
    assert_eq!(receipt.reason, "manual_observe_created");
    assert_eq!(receipt.work_order_ref, Some(work_order_ref));
    assert_eq!(receipt.recorded_at, "2026-10-10 10:05");

    let rejected = read_manual_observation_receipt(&database, target_ref, newer)
        .await
        .unwrap()
        .expect("the later rejection remains a separate durable result");
    assert_eq!(rejected.receipt_ref, newer);
    assert_eq!(rejected.outcome, "rejected");
    assert_eq!(rejected.reason, "authorization_missing");
    assert_eq!(rejected.work_order_ref, None);
    for (requested_target, requested_receipt) in [
        (other_target_ref, selected),
        (target_ref, other_receipt),
        (target_ref, wrong_kind),
        (target_ref, Uuid::new_v4()),
    ] {
        assert!(
            read_manual_observation_receipt(&database, requested_target, requested_receipt)
                .await
                .unwrap()
                .is_none(),
            "a mismatched or absent receipt must not fall back to another command"
        );
    }
}

async fn seed_target(database: &Database, identity_key: &str) -> Uuid {
    let target_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_observation_target \
             (target_ref,platform,target_kind,identity_key,display_name,source) \
         VALUES ($1,'xhs','creator',$2,$2,'manual')",
    )
    .bind(target_ref)
    .bind(identity_key)
    .execute(database.pool())
    .await
    .expect("the synthetic target is stored");
    target_ref
}

async fn seed_purpose(
    database: &Database,
    target_ref: Uuid,
    id: u128,
    name: &str,
    status: &str,
    role: &str,
    created_at: &str,
) -> Uuid {
    let domain_ref = Uuid::from_u128(id);
    sqlx::query(
        "INSERT INTO observation_domain (domain_ref,name,status,created_at) \
         VALUES ($1,$2,$3,$4::timestamptz)",
    )
    .bind(domain_ref)
    .bind(name)
    .bind(status)
    .bind(created_at)
    .execute(database.pool())
    .await
    .expect("the synthetic purpose is stored");
    sqlx::query(
        "INSERT INTO observation_domain_target (domain_ref,target_ref,role) VALUES ($1,$2,$3)",
    )
    .bind(domain_ref)
    .bind(target_ref)
    .bind(role)
    .execute(database.pool())
    .await
    .expect("the purpose has an explicit target relation");
    domain_ref
}

async fn seed_work_order(database: &Database, target_ref: Uuid, domain_ref: Uuid) -> Uuid {
    let request_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_acquisition_request \
             (request_ref,target_ref,domain_ref,observation_role,lane,purpose,requested_by) \
         VALUES ($1,$2,$3,'primary','patrol','manual receipt projection proof','person')",
    )
    .bind(request_ref)
    .bind(target_ref)
    .bind(domain_ref)
    .execute(database.pool())
    .await
    .expect("the Work Order has a real request");
    let authorization_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_acquisition_authorization \
             (authorization_ref,platform,target_kind,lane,purpose,granted_by,expires_at, \
              allowed_task_templates,allowed_dispatch_lanes,max_work_units) \
         VALUES ($1,'xhs','creator','patrol','manual receipt projection proof','person', \
                 scope_001_now()+interval '1 day',ARRAY['creator_patrol'],ARRAY['immediate'],20)",
    )
    .bind(authorization_ref)
    .execute(database.pool())
    .await
    .expect("the synthetic admission names an explicit authorization");
    let decision_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_admission_decision \
             (decision_ref,request_ref,outcome,reason_code,authorization_ref,target_ref) \
         VALUES ($1,$2,'admitted','within_authorization',$3,$4)",
    )
    .bind(decision_ref)
    .bind(request_ref)
    .bind(authorization_ref)
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("the Work Order has an admitted decision");
    let work_order_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_work_order \
             (work_order_ref,decision_ref,target_ref,lane,max_works,stop_conditions, \
              dispatch_lane,queue_state,scheduled_for,dedupe_key) \
         VALUES ($1,$2,$3,'patrol',20,'[\"maximum_quota\"]'::jsonb,'immediate','queued', \
                 scope_001_now(),$1::text)",
    )
    .bind(work_order_ref)
    .bind(decision_ref)
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("the receipt can reference a real, unclaimed Work Order");
    sqlx::query(
        "INSERT INTO collection_work_order_domain_usage \
             (work_order_ref,request_ref,domain_ref,role,basis_kind) \
         VALUES ($1,$2,$3,'primary','admitted')",
    )
    .bind(work_order_ref)
    .bind(request_ref)
    .bind(domain_ref)
    .execute(database.pool())
    .await
    .expect("the Work Order freezes the request purpose");
    work_order_ref
}

async fn seed_receipt(
    database: &Database,
    target_ref: Uuid,
    command_kind: &str,
    outcome: &str,
    reason: &str,
    work_order_ref: Option<Uuid>,
    recorded_at: &str,
) -> Uuid {
    let receipt_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_monitor_rule_command_receipt \
             (command_receipt_ref,target_ref,idempotency_key,command_kind,expected_revision, \
              payload_digest,actor,source,outcome,reason_code,work_order_ref,recorded_at) \
         VALUES ($1,$2,$3,$4,0,repeat('a',64),'person','targets_ui',$5,$6,$7,$8::timestamptz)",
    )
    .bind(receipt_ref)
    .bind(target_ref)
    .bind(Uuid::new_v4())
    .bind(command_kind)
    .bind(outcome)
    .bind(reason)
    .bind(work_order_ref)
    .bind(recorded_at)
    .execute(database.pool())
    .await
    .expect("the exact synthetic command receipt is stored");
    receipt_ref
}
