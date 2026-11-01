#[path = "support/material_fixture.rs"]
mod fixture;
use linggan_contracts::creator_discovery::creator_key;
use linggan_evidence::read_creator_observation_record;
use uuid::Uuid;

const D: Uuid = Uuid::from_u128(0x00000000000040008000000000000001);
const OTHER: Uuid = Uuid::from_u128(0x00000000000040008000000000000002);

#[tokio::test]
#[ignore = "isolated PostgreSQL proof only"]
async fn observation_record_reports_join_patrol_and_last_change() {
    let db = fixture::proof_database("observation_record").await;
    sqlx::query(
        "INSERT INTO observation_domain(domain_ref,name,status) VALUES($1,'ADHD','active') ON CONFLICT (domain_ref) DO NOTHING",
    )
    .bind(D)
    .execute(db.pool())
    .await
    .unwrap();
    let target = Uuid::new_v4();
    let rule = Uuid::new_v4();
    let revision = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_observation_target(target_ref,platform,target_kind,identity_key,display_name,source,lifecycle_state,first_stored_at,lifecycle_changed_at,monitoring_enabled,last_patrol_succeeded_at) \
         VALUES($1,'xhs','creator','author-1','观察名','manual','paused','2026-09-22 03:12+08','2026-10-15 16:26+08',false,'2026-10-14 14:12+08')",
    )
    .bind(target)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO observation_domain_target(domain_ref,target_ref,role,created_at) VALUES($1,$2,'primary','2026-09-24 12:23+08')")
        .bind(D)
        .bind(target)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO collection_monitor_rule(rule_ref,target_ref,slot_key,monitor_next_run_at,monitor_missed_run_count,last_patrol_dispatched_at) \
         VALUES($1,$2,'primary','2026-10-20 09:00+08',2,'2026-10-14 14:11+08')",
    )
    .bind(rule)
    .bind(target)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO collection_monitor_rule_revision(rule_revision_ref,target_ref,rule_ref,revision,mode,automatic_enabled,timezone,run_on_weekdays,run_on_weekends,all_day,fixed_interval_seconds,fallback_interval_seconds,surface_key,task_contract_version,rule_payload_digest,created_by) \
         VALUES($1,$2,$3,1,'fixed',false,'Asia/Shanghai',true,true,true,86400,86400,'profile_discovery','linggan.producer.task-spec.v2',repeat('a',64),'person')",
    )
    .bind(revision)
    .bind(target)
    .bind(rule)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query("UPDATE collection_monitor_rule SET active_revision_ref=$1 WHERE rule_ref=$2")
        .bind(revision)
        .bind(rule)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO collection_observation_target_transition(transition_ref,target_ref,from_state,to_state,actor,reason_code,occurred_at) \
         VALUES($1,$2,'monitoring','paused','person','monitor_paused','2026-10-15 16:26+08')",
    )
    .bind(Uuid::new_v4())
    .bind(target)
    .execute(db.pool())
    .await
    .unwrap();

    let record = read_creator_observation_record(&db, D, &creator_key("xhs", "author-1"))
        .await
        .unwrap()
        .expect("joined creator has a record");
    assert_eq!(record.usage_role, "primary");
    assert_eq!(record.joined_at, "2026-09-24 12:23");
    assert_eq!(record.first_stored_at.as_deref(), Some("2026-09-22 03:12"));
    assert_eq!(record.lifecycle_state, "paused");
    assert_eq!(
        record.lifecycle_changed_at.as_deref(),
        Some("2026-10-15 16:26")
    );
    assert_eq!(record.monitoring_state, "paused");
    assert!(!record.automatic_enabled);
    assert_eq!(
        record.last_dispatched_at.as_deref(),
        Some("2026-10-14 14:11")
    );
    assert_eq!(
        record.last_succeeded_at.as_deref(),
        Some("2026-10-14 14:12")
    );
    // 巡查关着时不读「下一次」，否则会显示一个永远不会到来的时间。
    assert_eq!(record.next_run_at, None);
    assert_eq!(record.missed_runs, 2);
    assert_eq!(record.patrol_state, "disabled");
    assert_eq!(record.latest_hits, None);
    let transition = record.last_transition.expect("last change is recorded");
    assert_eq!(transition.occurred_at, "2026-10-15 16:26");
    assert_eq!(transition.from_state.as_deref(), Some("monitoring"));
    assert_eq!(transition.to_state, "paused");
    assert_eq!(transition.reason_code, "monitor_paused");

    // 同一目标在别的领域不是本领域的观察记录；不存在的作者同样没有记录。
    assert!(
        read_creator_observation_record(&db, OTHER, &creator_key("xhs", "author-1"))
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        read_creator_observation_record(&db, D, &creator_key("xhs", "missing"))
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof only"]
async fn enabled_patrol_shows_next_run_and_waiting_state() {
    let db = fixture::proof_database("observation_record_running").await;
    sqlx::query(
        "INSERT INTO observation_domain(domain_ref,name,status) VALUES($1,'ADHD','active') ON CONFLICT (domain_ref) DO NOTHING",
    )
    .bind(D)
    .execute(db.pool())
    .await
    .unwrap();
    let target = Uuid::new_v4();
    let rule = Uuid::new_v4();
    let revision = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_observation_target(target_ref,platform,target_kind,identity_key,display_name,source,lifecycle_state,monitoring_enabled) \
         VALUES($1,'xhs','creator','author-2','观察名','manual','monitoring',true)",
    )
    .bind(target)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO observation_domain_target(domain_ref,target_ref,role) VALUES($1,$2,'reference')")
        .bind(D)
        .bind(target)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO collection_monitor_rule(rule_ref,target_ref,slot_key,monitor_next_run_at) \
         VALUES($1,$2,'primary','2026-11-01 09:00+08')",
    )
    .bind(rule)
    .bind(target)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO collection_monitor_rule_revision(rule_revision_ref,target_ref,rule_ref,revision,mode,automatic_enabled,timezone,run_on_weekdays,run_on_weekends,all_day,fixed_interval_seconds,fallback_interval_seconds,surface_key,task_contract_version,rule_payload_digest,created_by) \
         VALUES($1,$2,$3,1,'fixed',true,'Asia/Shanghai',true,true,true,86400,86400,'profile_discovery','linggan.producer.task-spec.v2',repeat('a',64),'person')",
    )
    .bind(revision)
    .bind(target)
    .bind(rule)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query("UPDATE collection_monitor_rule SET active_revision_ref=$1 WHERE rule_ref=$2")
        .bind(revision)
        .bind(rule)
        .execute(db.pool())
        .await
        .unwrap();

    let record = read_creator_observation_record(&db, D, &creator_key("xhs", "author-2"))
        .await
        .unwrap()
        .expect("joined creator has a record");
    assert_eq!(record.usage_role, "reference");
    assert_eq!(record.monitoring_state, "monitoring");
    assert!(record.automatic_enabled);
    assert_eq!(record.next_run_at.as_deref(), Some("2026-11-01 09:00"));
    // 还没有任何成功巡查：计数是未知，不是 0。
    assert_eq!(record.latest_hits, None);
    assert_eq!(record.latest_new, None);
    assert_eq!(record.patrol_state, "waiting");
    assert!(record.last_transition.is_none());
}
