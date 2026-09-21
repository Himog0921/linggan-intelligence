use linggan_contracts::{
    EvidenceQuery, ProducerTaskSpec, parse_producer_attempt, parse_producer_submission,
    parse_producer_task_spec,
};
use linggan_evidence::{
    AccountEligibilityObservation, AuthorizationGrant, CheckInOutcome, DeliveryConclusion,
    DetailPageSessionNavigationError, DetailPageSessionProgress, DispatchDecision,
    DispatchFailureCode, DispatchFailureOutcome, InstallationCheckIn, MonitorCommandActor,
    MonitorCommandKind, MonitorRuleCommand, MonitorRuleDraft, MonitorRuleMode, PreparedLaneDelivery,
    ProducerRuntimeError, RuntimeAttemptOutcome, RuntimeSubmissionOutcome, RuntimeTaskOutcome,
    activate_installation_credential, apply_monitor_rule_command, bind_observation_account,
    check_in_installation, create_producer_task, decide_dispatch, dispatch_schema_is_ready,
    DetailPageSessionGrant, expire_lapsed_leases, grant_authorization, grant_detail_page_session,
    grant_detail_page_session_with_lane_deliveries, issue_work_order_lease,
    open_claim_window, read_collection_task_timeline, read_detail_delivery_reconciliation,
    read_work_resources, record_detail_page_session_progress, recover_released_orphaned_work_orders,
    report_account_eligibility, requeue_failed_dispatch,
    retire_materials, rotate_installation_credential, set_station_accepting, start_producer_attempt,
    submit_producer_package,
};
use linggan_storage_postgres::{Database, testing::isolated_proof_schema};
use sqlx::{AssertSqlSafe, Row};
use uuid::Uuid;

/// 迟到的页面读失败要证明的是「已经接纳的详情不因此消失」，而接纳详情只有一条正常路径
/// （Package 接纳）。这里复用材料夹具的那一条，不再写一份近似的替身。
#[allow(dead_code)]
#[path = "support/material_fixture.rs"]
mod material_fixture;

const MIGRATIONS: &str = concat!(
    include_str!("../../../database/migrations/0001_scope_001_capture_evidence.sql"),
    "\n",
    include_str!("../../../database/migrations/0002_local_001_discovery.sql"),
    "\n",
    include_str!("../../../database/migrations/0003_local_trusted_producer.sql"),
    "\n",
    include_str!("../../../database/migrations/0004_plugin_runtime_all_capabilities.sql"),
    "\n",
    include_str!("../../../database/migrations/0005_collection_observation_target.sql"),
    "\n",
    include_str!("../../../database/migrations/0006_collection_acquisition_chain.sql"),
    "\n",
    include_str!("../../../database/migrations/0007_execution_station.sql"),
    "\n",
    include_str!("../../../database/migrations/0008_collection_risk_pause.sql"),
    "\n",
    include_str!("../../../database/migrations/0009_work_order_station.sql"),
    "\n",
    include_str!("../../../database/migrations/0010_work_order_lease.sql"),
    "\n",
    include_str!("../../../database/migrations/0011_execution_gate.sql"),
    "\n",
    include_str!("../../../database/migrations/0012_target_monitor_schedule.sql"),
    "\n",
    include_str!("../../../database/migrations/0013_drop_execution_gate.sql"),
    "\n",
    include_str!("../../../database/migrations/0014_target_group.sql"),
    "\n",
    include_str!("../../../database/migrations/0015_material_projection.sql"),
    "\n",
    include_str!("../../../database/migrations/0016_material_social_lanes.sql"),
    "\n",
    include_str!("../../../database/migrations/0017_material_media_projection.sql"),
    "\n",
    include_str!("../../../database/migrations/0018_material_discovery_lane.sql"),
    "\n",
    include_str!("../../../database/migrations/0019_work_order_lease_task_sequence.sql"),
    "\n",
    include_str!("../../../database/migrations/0020_observation_runtime_automation.sql"),
    "\n",
    include_str!("../../../database/migrations/0021_discovery_cover_media_acquisition.sql"),
    "\n",
    include_str!("../../../database/migrations/0022_material_deepening_scope.sql"),
    "\n",
    include_str!("../../../database/migrations/0023_material_engagement_and_media_components.sql"),
    "\n",
    include_str!("../../../database/migrations/0024_media_processing_runtime.sql"),
    "\n",
    include_str!("../../../database/migrations/0025_comment_current_projection.sql"),
    "\n",
    include_str!("../../../database/migrations/0026_work_resource_read.sql"),
    "\n",
    include_str!("../../../database/migrations/0027_unified_media_resource.sql"),
    "\n",
    include_str!("../../../database/migrations/0029_author_avatar_media.sql"),
    "\n",
    include_str!("../../../database/migrations/0030_comment_image_media.sql"),
    "\n",
    include_str!("../../../database/migrations/0032_author_profile_avatar_media.sql"),
    "\n",
    include_str!("../../../database/migrations/0033_dispatch_failure_recovery.sql"),
    "\n",
    include_str!("../../../database/migrations/0034_collection_control_closure.sql"),
    "\n",
    include_str!("../../../database/migrations/0035_claimed_station_auto_acceptance.sql"),
    "\n",
    include_str!("../../../database/migrations/0036_monitor_scheduling_clarity.sql"),
    "\n",
    include_str!("../../../database/migrations/0037_collection_scheduler_scale.sql"),
    "\n",
    include_str!("../../../database/migrations/0038_detail_only_material_scope.sql"),
    "\n",
    include_str!("../../../database/migrations/0045_deep_archive_recovery.sql"),
    "\n",
    include_str!("../../../database/migrations/0046_keyword_sampling_policy.sql"),
    "\n",
    include_str!("../../../database/migrations/0047_collection_detail_failure_boundary.sql"),
    "\n",
    include_str!("../../../database/migrations/0052_work_order_expiry.sql"),
    "\n",
    include_str!("../../../database/migrations/0061_material_retirement.sql"),
    "\n",
    include_str!("../../../database/migrations/0062_human_moment.sql"),
    "\n",
    include_str!("../../../database/migrations/0063_content_author_attribution.sql"),
    "\n",
    include_str!("../../../database/migrations/0064_account_observation_normalization.sql"),
    "\n",
    include_str!("../../../database/migrations/0065_account_observation_bootstrap.sql"),
    "\n",
    include_str!("../../../database/migrations/0080_scheduler_admission_failure_reasons.sql"),
    "\n",
    include_str!("../../../database/migrations/0076_monitor_rule_slots.sql"),
    "\n",
    include_str!("../../../database/migrations/0078_monitor_rule_owns_its_schedule.sql"),
    "\n",
    include_str!("../../../database/migrations/0089_detail_page_session_replay_safety.sql"),
    "\n",
    include_str!(
        "../../../database/migrations/0090_detail_page_grant_recovery_and_risk_cooldown.sql"
    ),
    "\n",
    include_str!("../../../database/migrations/0091_ocr_content_layering.sql"),
    "\n",
    include_str!("../../../database/migrations/0092_detail_page_session_recovery_boundary.sql"),
    "\n",
    include_str!("../../../database/migrations/0093_capture_delivery_rejection.sql"),
    "\n",
    include_str!("../../../database/migrations/0094_corpus_evidence_read_recovery.sql"),
    "\n",
    include_str!("../../../database/migrations/0095_detail_page_url_rejection.sql"),
    "\n",
    include_str!(
        "../../../database/migrations/0096_detail_page_session_lane_delivery_identities.sql"
    ),
    "\n",
    include_str!("../../../database/migrations/0097_collection_execution_input_eligibility.sql"),
    "\n",
    include_str!(
        "../../../database/migrations/0098_scheduler_tick_steps_and_readiness.sql"
    ),
);

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn dispatch_schema_readiness_requires_relations_in_the_current_schema() {
    let database = proof_database_for("collection_dispatch_schema_readiness").await;
    let schema: String = sqlx::query_scalar("SELECT current_schema()")
        .fetch_one(database.pool())
        .await
        .expect("proof schema is identifiable");
    let shadow_schema = format!("{schema}_shadow");

    // A later search_path schema can contain a same-named relation.  Dispatch
    // readiness must still reject the incomplete active proof schema.
    sqlx::raw_sql(AssertSqlSafe(format!(
        "CREATE SCHEMA {shadow_schema}; \
         CREATE TABLE {shadow_schema}.collection_work_order_lease (placeholder text); \
         ALTER TABLE collection_work_order_lease RENAME TO collection_work_order_lease_missing; \
         SET search_path TO {schema},{shadow_schema};"
    )))
    .execute(database.pool())
    .await
    .expect("proof shadows only the missing dispatch relation outside the current schema");

    assert!(
        !dispatch_schema_is_ready(&database)
            .await
            .expect("dispatch readiness remains readable"),
        "a relation in a later search_path schema cannot make dispatch ready"
    );
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn a_target_without_any_rule_never_becomes_due_and_the_scheduler_stays_idle() {
    let database = proof_database_for("collection_scheduler_enabled_target").await;
    let target_ref = Uuid::new_v4();
    // `0036` 曾用一条 CHECK 挡住「在监控中却没有规则」：那时目标行上存着一份规则指针，
    // 指针为空而开关为真，调度就会照着一个不存在的规则跑。`0078` 把那份指针删掉之后，
    // 调度是**遍历规则**的——没有规则就没有到期的东西，这件事由下面的空转直接证明，
    // 不再需要一条数据库约束替它挡。
    sqlx::query(
        "INSERT INTO collection_observation_target \
             (target_ref,platform,target_kind,identity_key,display_name,source,lifecycle_state,monitoring_enabled) \
         VALUES ($1,'xhs','creator','creator-auto-proof','自动调度证明','manual','monitoring',true)",
    )
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("monitoring is a target-level fact; having rules is a rule-level one");
    let rule_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_monitor_rule WHERE target_ref=$1 AND retired_at IS NULL",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .expect("rule count is readable");
    assert_eq!(rule_count, 0, "前提：这个目标一条规则都没有");

    linggan_evidence::record_scheduler_started(&database, Uuid::new_v4())
        .await
        .unwrap();
    let first = linggan_evidence::run_due_patrols(&database).await.unwrap();
    assert!(first.queued.is_empty());
    assert!(first.dispatched.is_empty());
    assert!(first.skipped.is_empty());
    let first_heartbeat = linggan_evidence::read_scheduler_heartbeat(&database)
        .await
        .unwrap()
        .expect("scheduler heartbeat exists after an idle tick");
    assert_eq!(first_heartbeat.state, "running");
    assert_eq!(first_heartbeat.last_outcome, "idle");
    assert_eq!(first_heartbeat.dispatched_count, 0);
    assert_eq!(first_heartbeat.skipped_count, 0);
    let work_order_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order WHERE target_ref=$1 AND lane='deep_archive'",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(work_order_count, 0);
    let live_lease_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order w JOIN collection_work_order_lease l USING(work_order_ref) \
         WHERE w.target_ref=$1 AND l.released_at IS NULL",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(live_lease_count, 0);

    let second = linggan_evidence::run_due_patrols(&database).await.unwrap();
    assert!(second.queued.is_empty());
    assert!(second.dispatched.is_empty());
    let work_order_count_after: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order WHERE target_ref=$1 AND lane='deep_archive'",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(
        work_order_count_after, 0,
        "an enabled flag does not bypass the versioned rule and capacity gates"
    );
    let heartbeat = linggan_evidence::read_scheduler_heartbeat(&database)
        .await
        .unwrap()
        .expect("scheduler heartbeat exists");
    assert_eq!(heartbeat.state, "running");
    assert_eq!(heartbeat.last_outcome, "idle");
    assert_eq!(heartbeat.dispatched_count, 0);
    assert_eq!(heartbeat.skipped_count, 0);
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn creator_rule_queues_once_without_a_baseline_or_a_preassigned_station() {
    let database = proof_database_for("collection_scheduler_creator_queue").await;
    let target_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_observation_target \
             (target_ref,platform,target_kind,identity_key,display_name,source,lifecycle_state) \
         VALUES ($1,'xhs','creator','creator-rule-proof','规则入队证明','manual','pending_decision')",
    )
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("creator target is seeded without any archive receipt");
    grant_authorization(
        &database,
        &AuthorizationGrant {
            platform: "xhs",
            target_kind: "creator",
            lane: "patrol",
            purpose: "creator patrol proof",
            max_targets: Some(10),
            max_works_per_target: Some(30),
            valid_for_days: 1,
        },
    )
    .await
    .expect("structured patrol authorization is granted");
    let saved = apply_monitor_rule_command(
        &database,
        &MonitorRuleCommand {
            target_ref,
            expected_revision: 0,
            idempotency_key: Uuid::new_v4(),
            kind: MonitorCommandKind::SaveRule,
            actor: MonitorCommandActor::Person,
            source: "targets_ui",
            slot_key: None,
            draft: Some(MonitorRuleDraft {
                mode: MonitorRuleMode::Fixed,
                automatic_enabled: true,
                run_on_weekdays: true,
                run_on_weekends: true,
                all_day: true,
                window_start_minute: None,
                window_end_minute: None,
                fixed_interval_seconds: Some(21_600),
                fallback_interval_seconds: 21_600,
                surface_key: "creator_patrol".to_owned(),
                ranking_key: None,
                scroll_rounds: None,
                top_by_likes: None,
                published_within_days: None,
                task_contract_version: "linggan.producer.task-spec.v1".to_owned(),
            }),
        },
    )
    .await
    .expect("baseline completeness does not block creator rule save");
    assert_eq!(saved.reason_code, "rule_saved");
    // 排期状态住在规则上（`0076`）——改目标行不会让任何规则到期。
    sqlx::query(
        "UPDATE collection_monitor_rule \
         SET monitor_next_run_at=scope_001_now()-interval '1 second' WHERE target_ref=$1",
    )
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("the isolated clock makes the rule due");

    let tick = linggan_evidence::run_due_patrols(&database)
        .await
        .expect("due rule is queued");
    assert_eq!(tick.queued, vec![target_ref]);
    assert!(tick.dispatched.is_empty());
    assert!(tick.skipped.is_empty());
    let queued: (
        String,
        String,
        Option<Uuid>,
        Option<Uuid>,
        Option<Uuid>,
        Option<Uuid>,
    ) = sqlx::query_as(
        "SELECT queue_state,dispatch_lane,station_ref,installation_ref,account_ref, \
                    monitor_rule_revision_ref \
             FROM collection_work_order WHERE target_ref=$1",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .expect("the durable queued work order exists");
    assert_eq!(queued.0, "queued");
    assert_eq!(queued.1, "scheduled");
    assert_eq!((queued.2, queued.3, queued.4), (None, None, None));
    assert_eq!(queued.5, saved.applied_rule_revision_ref);
    // 排程住在规则上（`0078`）；目标只保留「在不在监控中」这一件它自己的事实。
    let schedule: (bool, Option<String>, i32) = sqlx::query_as(
        "SELECT target.monitoring_enabled,rule.monitor_next_run_at::text, \
                rule.monitor_missed_run_count \
         FROM collection_observation_target target \
         JOIN collection_monitor_rule rule \
           ON rule.target_ref=target.target_ref AND rule.retired_at IS NULL \
         WHERE target.target_ref=$1",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .expect("the next schedule advances atomically with queueing");
    assert!(schedule.0);
    assert!(schedule.1.is_some());
    assert_eq!(schedule.2, 0);
    let active_lease_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order_lease WHERE work_order_ref=( \
             SELECT work_order_ref FROM collection_work_order WHERE target_ref=$1)",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .expect("scheduler did not pre-lease browser work");
    assert_eq!(active_lease_count, 0);

    // A later plugin poll, not the scheduler tick, supplies the actual station
    // and account. The fixture's own legacy order stays unqueued and therefore
    // cannot be chosen instead of this scheduled Work Order.
    let claimant = seed_creator_work_order(&database).await;
    let claimed = decide_dispatch(
        &database,
        &claimant.install_key,
        &claimant.installation_credential,
    )
    .await
    .expect("eligible station claims the queued creator patrol");
    assert_eq!(capability(&claimed), "author_profile");
    let leased: (String, Option<Uuid>, Option<Uuid>, Option<Uuid>) = sqlx::query_as(
        "SELECT queue_state,station_ref,installation_ref,account_ref \
         FROM collection_work_order WHERE target_ref=$1",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .expect("claim freezes the actual station route only now");
    assert_eq!(leased.0, "leased");
    assert_eq!(leased.1, Some(claimant.station_ref));
    assert_eq!(leased.2, Some(claimant.installation_ref));
    assert!(leased.3.is_some());
}

/// 调度闸门只说一件事：**这次没推进排期，等 300 秒再来看**。它不替排期回答「下一次什么时候」。
///
/// 2026-09-14 adhd 那条关键词规则排期已经到点，却被自己上一次派发时留下的闸门挡住，下一次
/// 要晚 27 小时；界面上改周期、暂停恢复都只重算排期、不回写闸门，于是每一次调整都撞在同一面
/// 墙上。下面两组断言分别钉住修复的两半：**推进了排期的派发不留便条**（写侧），
/// **被拒的便条仍然挡得住**（读侧，冷却还在）。
#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn scheduler_gate_only_cools_down_rejected_rules_and_never_outranks_the_rule_schedule() {
    let database = proof_database_for("collection_scheduler_gate_scope").await;
    let dispatched_target =
        save_patrol_rule(&database, "creator", "creator-schedule-proof", None).await;
    grant_authorization(
        &database,
        &AuthorizationGrant {
            platform: "xhs",
            target_kind: "creator",
            lane: "patrol",
            purpose: "creator patrol gate proof",
            max_targets: Some(10),
            max_works_per_target: Some(200),
            valid_for_days: 1,
        },
    )
    .await
    .expect("creator patrol authorization is granted");

    make_rule_due(&database, dispatched_target).await;
    let first = linggan_evidence::run_due_patrols(&database)
        .await
        .expect("the due rule is queued");
    assert_eq!(first.queued, vec![dispatched_target]);
    assert!(first.skipped.is_empty());
    assert_eq!(
        latest_decision_gate(&database, dispatched_target, "queued").await,
        Some(None),
        "派发已经在同一笔事务里把排期翻到下一次，就不再留一张按别的算法算出来的便条"
    );

    // 第一单跑完了。真实系统里，人正是在这时候去界面上改周期的。
    sqlx::query("UPDATE collection_work_order SET queue_state='completed' WHERE target_ref=$1")
        .bind(dispatched_target)
        .execute(database.pool())
        .await
        .expect("the first patrol finishes before the schedule is rewritten");
    // 修复上线前那次派发留下的便条还在（审计记录不改写），它写着「12 小时后再来」。
    seed_pre_fix_dispatch_gate(&database, dispatched_target).await;
    make_rule_due(&database, dispatched_target).await;
    let reconsidered = linggan_evidence::run_due_patrols(&database)
        .await
        .expect("the rewritten schedule is honoured");
    assert_eq!(
        reconsidered.queued,
        vec![dispatched_target],
        "排期说到点了就得被考虑：历史派发留下的旧便条没有资格压过排期，只有被拒的才有"
    );

    // 上半段的授权只服务于这个已派发目标。撤销它后再验证未授权目标的冷却，避免同一类
    // 目标的宽授权意外覆盖拒绝样本。
    sqlx::query(
        "UPDATE collection_acquisition_authorization \
         SET revoked_at=scope_001_now(),revoke_reason='proof: isolate rejected creator rule' \
         WHERE target_kind='creator' AND lane='patrol' \
           AND purpose='creator patrol gate proof' AND revoked_at IS NULL",
    )
    .execute(database.pool())
    .await
    .expect("the dispatched-target authorization is retired before the rejection proof");

    // 另一半：被拒的规则仍然要等 300 秒。适用授权已撤销，准入必然拒绝。
    let rejected_target = save_patrol_rule(&database, "creator", "creator-gate-proof", None).await;
    make_rule_due(&database, rejected_target).await;
    let refused = linggan_evidence::run_due_patrols(&database)
        .await
        .expect("a rule without authorization is refused, not queued");
    assert_eq!(
        refused.skipped,
        vec![(
            (rejected_target),
            "authorization_expired_or_revoked".to_owned()
        )]
    );
    let waited: i32 = sqlx::query_scalar(
        "SELECT EXTRACT(EPOCH FROM (next_eligible_at-decided_at))::integer \
         FROM collection_scheduler_target_decision \
         WHERE target_ref=$1 AND outcome='rejected' \
         ORDER BY decided_at DESC, target_decision_ref DESC LIMIT 1",
    )
    .bind(rejected_target)
    .fetch_one(database.pool())
    .await
    .expect("the refusal leaves a cooling-down note");
    assert_eq!(waited, 300);

    let cooled = linggan_evidence::run_due_patrols(&database)
        .await
        .expect("a cooling rule is not retried yet");
    assert!(
        cooled.queued.is_empty() && cooled.skipped.is_empty(),
        "被拒之后 300 秒之内不再碰它：冷却的意义就是不每分钟重试同一件注定失败的事"
    );
    sqlx::query(
        "UPDATE collection_scheduler_target_decision SET next_eligible_at=scope_001_now()-interval '1 second' \
         WHERE target_ref=$1 AND outcome='rejected'",
    )
    .bind(rejected_target)
    .execute(database.pool())
    .await
    .expect("the cooling window passes");
    let after_cooldown = linggan_evidence::run_due_patrols(&database)
        .await
        .expect("the rule is considered again after the cooldown");
    assert_eq!(
        after_cooldown.skipped,
        vec![(
            (rejected_target),
            "authorization_expired_or_revoked".to_owned()
        )],
        "冷却过后如期再来一次：便条是冷却，不是封条"
    );
}

/// 未完成建档的关键词没有可排期的巡查规则。
///
/// 这是规则写入这一层的第一道门；即使有授权，也不能用它把尚未完成的关键词送进巡查。
#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn an_unarchived_keyword_rule_is_rejected_before_any_patrol_is_scheduled() {
    let database = proof_database_for("collection_keyword_patrol_expected_count").await;
    let target_ref = save_patrol_rule(&database, "keyword", "adhd::comprehensive", Some(20)).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM collection_monitor_rule WHERE target_ref=$1",
        )
        .bind(target_ref)
        .fetch_one(database.pool())
        .await
        .expect("the control rows are readable"),
        0,
        "未完成建档的关键词在规则写入前被拒绝，因而没有可排期的 patrol rule"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT reason_code FROM collection_monitor_rule_command_receipt \
             WHERE target_ref=$1 ORDER BY recorded_at DESC, command_receipt_ref DESC LIMIT 1",
        )
        .bind(target_ref)
        .fetch_one(database.pool())
        .await
        .expect("the rejection receipt is readable"),
        "baseline_not_ready",
        "拒绝原因保留在既有的持久化闭集词汇内，不以本次修复扩大共享数据库约束"
    );
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn creator_lease_claims_and_completes_two_scheduled_tasks_in_order() {
    let database = proof_database().await;
    let fixture = seed_creator_work_order(&database).await;
    let lease = issue_work_order_lease(&database, fixture.work_order_ref, 60)
        .await
        .expect("creator work order receives one ordered lease");
    assert_eq!(lease.task_ids.len(), 2);

    let legacy_task_id: Option<Uuid> =
        sqlx::query_scalar("SELECT task_id FROM collection_work_order_lease WHERE lease_ref = $1")
            .bind(lease.lease_ref)
            .fetch_one(database.pool())
            .await
            .expect("legacy compatibility column is readable");
    assert_eq!(
        legacy_task_id, None,
        "new writers do not dual-write task identity"
    );

    let rows = sqlx::query(
        "SELECT sequence_no, execution_state FROM collection_work_order_lease_task \
         WHERE lease_ref = $1 ORDER BY sequence_no",
    )
    .bind(lease.lease_ref)
    .fetch_all(database.pool())
    .await
    .expect("lease task sequence is readable");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].get::<i32, _>("sequence_no"), 1);
    assert_eq!(rows[1].get::<i32, _>("sequence_no"), 2);
    assert!(
        rows.iter()
            .all(|row| row.get::<String, _>("execution_state") == "pending")
    );

    let (left, right) = tokio::join!(
        decide_dispatch(
            &database,
            &fixture.install_key,
            &fixture.installation_credential
        ),
        decide_dispatch(
            &database,
            &fixture.install_key,
            &fixture.installation_credential
        ),
    );
    let (first, first_replay) = same_dispatch(
        left.expect("first concurrent dispatch decides"),
        right.expect("second concurrent dispatch decides"),
    );
    assert_eq!(capability(&first), "author_profile");
    assert_eq!(task_id(&first), task_id(&first_replay));

    let response_loss_replay = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("lost response retry decides");
    assert_eq!(task_id(&first), task_id(&response_loss_replay));

    let first_task = task_from_dispatch(&first);
    let wrong_attempt = attempt(first_task.task_id(), Uuid::new_v4());
    assert!(matches!(
        start_producer_attempt(&database, &wrong_attempt).await,
        Err(ProducerRuntimeError::ScheduledTaskNotClaimed)
    ));
    run_scheduled_task(&database, &first_task, fixture.producer_instance_id).await;
    assert_task_state(&database, first_task.task_id(), "completed").await;
    assert!(lease_is_live(&database, lease.lease_ref).await);

    let second = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("second sequence dispatch decides");
    assert_eq!(capability(&second), "profile_discovery");
    let second_replay = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("second response replay decides");
    assert_eq!(task_id(&second), task_id(&second_replay));
    let second_task = task_from_dispatch(&second);
    run_scheduled_task(&database, &second_task, fixture.producer_instance_id).await;
    assert_task_state(&database, second_task.task_id(), "completed").await;
    assert!(!lease_is_live(&database, lease.lease_ref).await);
    let lifecycle_state: String = sqlx::query_scalar(
        "SELECT lifecycle_state FROM collection_observation_target \
         WHERE identity_key='creator-fixture'",
    )
    .fetch_one(database.pool())
    .await
    .expect("completed baseline advances target lifecycle");
    assert_eq!(
        lifecycle_state, "archiving",
        "two accepted empty packages complete the lease but cannot manufacture a qualified baseline"
    );

    let manual_task = manual_task();
    assert!(matches!(
        create_producer_task(&database, &manual_task).await,
        Ok(RuntimeTaskOutcome::Created { .. })
    ));
    let manual_attempt = attempt(manual_task.task_id(), Uuid::new_v4());
    assert!(matches!(
        start_producer_attempt(&database, &manual_attempt).await,
        Ok(RuntimeAttemptOutcome::Started { .. })
    ));

    let timeline = read_collection_task_timeline(&database, 100)
        .await
        .expect("Task / Attempt / Package / Receipt read projection is available");
    assert_eq!(timeline.tasks.len(), 3);
    assert_eq!(timeline.accepted_count, 2);
    assert_eq!(
        timeline.active_count, 1,
        "manual Attempt has no Receipt yet"
    );
    assert_eq!(timeline.expired_lease_count, 0);
    let completed = timeline
        .tasks
        .iter()
        .find(|row| row.task_id == first_task.task_id())
        .expect("first scheduled task is listed");
    assert_eq!(completed.capabilities, "author_profile");
    assert_eq!(completed.queue_state.as_deref(), Some("completed"));
    assert_eq!(completed.has_live_lease, Some(false));
    assert_eq!(completed.material_admission.as_deref(), Some("ACCEPTED"));
    assert_eq!(
        completed.target_display_name.as_deref(),
        Some("顺序派发夹具"),
        "scheduled task retains its collection target instead of inventing an author"
    );
    let unreceived = timeline
        .tasks
        .iter()
        .find(|row| row.task_id == manual_task.task_id())
        .expect("manual task is listed");
    assert!(unreceived.attempt_id.is_some());
    assert!(unreceived.receipt_ref.is_none());
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn task_read_projection_keeps_queue_attempt_package_and_receipt_distinct() {
    let database = proof_database_for("collection_task_read_projection").await;
    let fixture = seed_creator_work_order(&database).await;
    let lease = issue_work_order_lease(&database, fixture.work_order_ref, 60)
        .await
        .expect("creator work order is leased");
    let first = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("first task is claimed");
    let first_task = task_from_dispatch(&first);
    run_scheduled_task(&database, &first_task, fixture.producer_instance_id).await;
    assert_task_state(&database, first_task.task_id(), "completed").await;
    assert!(lease_is_live(&database, lease.lease_ref).await);

    let timeline = read_collection_task_timeline(&database, 100)
        .await
        .expect("Task projection is readable");
    assert_eq!(timeline.tasks.len(), 2);
    assert_eq!(timeline.accepted_count, 1);
    assert_eq!(timeline.active_count, 1);
    assert_eq!(timeline.expired_lease_count, 0);

    let completed = timeline
        .tasks
        .iter()
        .find(|row| row.task_id == first_task.task_id())
        .expect("completed first step is present");
    assert_eq!(completed.capabilities, "author_profile");
    assert_eq!(completed.queue_state.as_deref(), Some("completed"));
    assert_eq!(completed.has_live_lease, Some(true));
    assert!(completed.attempt_id.is_some());
    assert!(completed.package_ref.is_some());
    assert!(completed.receipt_ref.is_some());
    assert_eq!(completed.material_admission.as_deref(), Some("ACCEPTED"));
    assert_eq!(
        completed.target_display_name.as_deref(),
        Some("顺序派发夹具")
    );

    let pending = timeline
        .tasks
        .iter()
        .find(|row| row.task_id != first_task.task_id())
        .expect("second ordered step is present");
    assert_eq!(pending.capabilities, "profile_discovery");
    assert_eq!(pending.queue_state.as_deref(), Some("pending"));
    assert_eq!(pending.has_live_lease, Some(true));
    assert!(pending.attempt_id.is_none());
    assert!(pending.package_ref.is_none());
    assert!(pending.receipt_ref.is_none());
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn expired_scheduled_lease_is_historical_not_active_in_task_projection() {
    let database = proof_database_for("collection_task_read_expired_lease").await;
    let fixture = seed_creator_work_order(&database).await;
    let lease = issue_work_order_lease(&database, fixture.work_order_ref, 60)
        .await
        .expect("creator work order is leased");
    let dispatch = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("first task is claimed before the lease expires");
    let claimed_task_id = task_id(&dispatch);
    assert_task_state(&database, claimed_task_id, "in_progress").await;

    // A deterministic isolated fixture: real expiry code must record this as
    // `released_at = expires_at`, rather than the read projection guessing from
    // a stale queue state.
    sqlx::query(
        "UPDATE collection_work_order_lease \
         SET issued_at = scope_001_now() - interval '2 seconds', \
             expires_at = scope_001_now() - interval '1 second' \
         WHERE lease_ref = $1",
    )
    .bind(lease.lease_ref)
    .execute(database.pool())
    .await
    .expect("fixture lease is made lapsed");
    assert_eq!(
        expire_lapsed_leases(&database)
            .await
            .expect("lapsed lease is recorded"),
        1
    );
    assert!(!lease_is_live(&database, lease.lease_ref).await);

    let timeline = read_collection_task_timeline(&database, 100)
        .await
        .expect("expired lease stays visible as historical task data");
    assert_eq!(timeline.tasks.len(), 2);
    assert_eq!(timeline.active_count, 0);
    assert_eq!(timeline.expired_lease_count, 2);
    let claimed = timeline
        .tasks
        .iter()
        .find(|row| row.task_id == claimed_task_id)
        .expect("claimed task remains in bounded history");
    assert_eq!(claimed.queue_state.as_deref(), Some("in_progress"));
    assert_eq!(claimed.has_live_lease, Some(false));
    assert!(claimed.attempt_id.is_none());
    assert!(claimed.receipt_ref.is_none());
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn failed_browser_start_is_audited_then_returns_work_order_to_shared_queue() {
    let database = proof_database_for("collection_dispatch_failure_recovery").await;
    let fixture = seed_creator_work_order(&database).await;
    let lease = issue_work_order_lease(&database, fixture.work_order_ref, 60)
        .await
        .expect("creator work order is leased");
    let first_dispatch = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("first task is claimed before page startup");
    let first_task_id = task_id(&first_dispatch);
    assert_task_state(&database, first_task_id, "in_progress").await;

    let failure_ref = Uuid::new_v4();
    let outcome = requeue_failed_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
        first_task_id,
        failure_ref,
        DispatchFailureCode::PageTimeout,
    )
    .await
    .expect("the owning installation can return a failed page start to the queue");
    assert_eq!(
        outcome,
        DispatchFailureOutcome::Requeued {
            retry_after_seconds: 60
        }
    );
    assert_task_state(&database, first_task_id, "pending").await;
    let claim_owner: Option<Uuid> = sqlx::query_scalar(
        "SELECT claimed_by_installation_ref FROM collection_work_order_lease_task WHERE task_id=$1",
    )
    .bind(first_task_id)
    .fetch_one(database.pool())
    .await
    .expect("requeued task remains readable");
    assert_eq!(claim_owner, None, "no installation owns a requeued task");
    let failure: (Uuid, Uuid, String) = sqlx::query_as(
        "SELECT task_id,installation_ref,failure_code \
         FROM collection_work_order_lease_task_dispatch_failure WHERE failure_ref=$1",
    )
    .bind(failure_ref)
    .fetch_one(database.pool())
    .await
    .expect("the page-start failure remains auditable");
    assert_eq!(failure.0, first_task_id);
    assert_eq!(failure.1, fixture.installation_ref);
    assert_eq!(failure.2, "page_timeout");
    let attempt_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_runtime_attempt WHERE task_id=$1")
            .bind(first_task_id)
            .fetch_one(database.pool())
            .await
            .expect("attempt count is readable");
    assert_eq!(
        attempt_count, 0,
        "a page-start failure is not a producer Attempt"
    );

    let timeline = read_collection_task_timeline(&database, 100)
        .await
        .expect("Task read projection keeps dispatch failures visible");
    assert_eq!(timeline.expired_lease_count, 2);
    let task = timeline
        .tasks
        .iter()
        .find(|row| row.task_id == first_task_id)
        .expect("requeued task is listed");
    assert_eq!(task.queue_state.as_deref(), Some("pending"));
    assert_eq!(task.has_live_lease, Some(false));
    assert_eq!(
        task.last_dispatch_failure_code.as_deref(),
        Some("page_timeout")
    );
    assert!(task.attempt_id.is_none());

    let replay = requeue_failed_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
        first_task_id,
        failure_ref,
        DispatchFailureCode::PageTimeout,
    )
    .await
    .expect("a lost failure response replays without writing a second failure");
    assert_eq!(
        replay,
        DispatchFailureOutcome::Replay {
            retry_after_seconds: 60
        }
    );
    let failure_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order_lease_task_dispatch_failure WHERE task_id=$1",
    )
    .bind(first_task_id)
    .fetch_one(database.pool())
    .await
    .expect("failure count is readable");
    assert_eq!(failure_count, 1, "same failure id is an idempotent replay");

    let cooling: bool = sqlx::query_scalar(
        "SELECT retry_not_before_at>scope_001_now() AND dispatch_failure_count=1 \
         FROM collection_work_order WHERE work_order_ref=$1",
    )
    .bind(fixture.work_order_ref)
    .fetch_one(database.pool())
    .await
    .expect("the retry delay is a persisted work-order fact");
    assert!(
        cooling,
        "a different station cannot immediately reopen the same failed page"
    );
    let during_cooling = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("the shared queue remains readable while cooling");
    assert!(matches!(during_cooling, DispatchDecision::NothingWaiting));
    sqlx::query(
        "UPDATE collection_work_order SET retry_not_before_at=scope_001_now()-interval '1 second' \
         WHERE work_order_ref=$1",
    )
    .bind(fixture.work_order_ref)
    .execute(database.pool())
    .await
    .expect("proof advances only the isolated retry clock");
    let retry = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("a fresh eligible claim is made after persistent cooling expires");
    assert_ne!(
        task_id(&retry),
        first_task_id,
        "a new Lease makes a fresh immutable RuntimeTask; the failed one stays history"
    );
    assert_task_state(&database, first_task_id, "pending").await;
    assert!(!lease_is_live(&database, lease.lease_ref).await);
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn unavailable_detail_is_audited_without_blocking_later_materials() {
    let database = proof_database_for("collection_dispatch_detail_session_recovery").await;
    let fixture = seed_creator_work_order(&database).await;
    for (ordinal, content_external_id) in [(1, "unavailable-first"), (2, "available-second")] {
        submit_profile_discovery(
            &database,
            content_external_id,
            &format!(
                "https://www.xiaohongshu.com/explore/{content_external_id}?xsec_token=SIGNED_FIXTURE&xsec_source=pc_user"
            ),
        )
        .await;
        let content_public_ref: Uuid = sqlx::query_scalar(
            "SELECT public_ref FROM linggan_material_content \
             WHERE platform='xhs' AND content_external_id=$1",
        )
        .bind(content_external_id)
        .fetch_one(database.pool())
        .await
        .expect("accepted discovery creates the stable material identity");
        sqlx::query(
            "INSERT INTO collection_work_order_material_target \
                 (work_order_ref,content_public_ref,ordinal,comment_limit,reply_expand_limit,acquire_media) \
             VALUES ($1,$2,$3,30,2,true)",
        )
        .bind(fixture.work_order_ref)
        .bind(content_public_ref)
        .bind(ordinal)
        .execute(database.pool())
        .await
        .expect("both exact materials are frozen by the same approved WorkOrder");
    }
    issue_work_order_lease(&database, fixture.work_order_ref, 60)
        .await
        .expect("deepening lease is issued");
    let first = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("first detail work is claimed");
    let first_task_id = task_id(&first);
    assert_eq!(
        task_from_dispatch(&first).raw()["target"]["contentExternalId"],
        "unavailable-first"
    );

    let unavailable_failure_ref = Uuid::new_v4();
    let outcome = requeue_failed_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
        first_task_id,
        unavailable_failure_ref,
        DispatchFailureCode::DetailPageSessionRecoveryRequired,
    )
    .await
    .expect("a producer-confirmed unavailable page is recorded");
    assert_eq!(outcome, DispatchFailureOutcome::Unavailable);
    let unavailable_lanes: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order_lease_task task \
         JOIN linggan_runtime_task runtime USING(task_id) \
         WHERE runtime.task_spec #>> '{target,contentExternalId}'='unavailable-first' \
           AND task.execution_state='unavailable'",
    )
    .fetch_one(database.pool())
    .await
    .expect("all dependent lanes remain readable");
    assert_eq!(
        unavailable_lanes, 4,
        "one unreadable work closes only its own lanes"
    );
    let attempts: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_runtime_attempt WHERE task_id=$1")
            .bind(first_task_id)
            .fetch_one(database.pool())
            .await
            .expect("attempt history is readable");
    assert_eq!(attempts, 0, "page unavailability is not a producer Attempt");

    let next = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("the next independent material is still eligible");
    assert_eq!(
        task_from_dispatch(&next).raw()["target"]["contentExternalId"],
        "available-second"
    );
    let failure_code: String = sqlx::query_scalar(
        "SELECT failure_code FROM collection_work_order_lease_task_dispatch_failure \
         WHERE task_id=$1",
    )
    .bind(first_task_id)
    .fetch_one(database.pool())
    .await
    .expect("unavailability remains an append-only dispatch fact");
    assert_eq!(failure_code, "detail_page_session_recovery_required");
    let replay = requeue_failed_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
        first_task_id,
        unavailable_failure_ref,
        DispatchFailureCode::DetailPageSessionRecoveryRequired,
    )
    .await
    .expect("a lost terminal acknowledgement remains idempotent");
    assert_eq!(replay, DispatchFailureOutcome::Unavailable);
}

/// T28 的第一个例子：生产方**明确**判定页面不可用，且上报的正是 `page_unavailable` 这个码。
///
/// 与相邻的 `unavailable_detail_is_audited_without_blocking_later_materials` 分工不同：那一条
/// 走的是「同页缓存身份无法校验、不再自动重开」的 `detail_page_session_recovery_required`
/// （插件在消费过导航许可之后对执行不确定性的如实上报），这一条走的是「这一页明确不可用」
/// 本身。两者在服务端判定表里同一次收束，但触发它们的是生产里两个不同的事实，T28 点名要求
/// 各有一例。
///
/// 这条同时守住「不伪造」：停止只落成这一篇自己的通道终态和一条追加式失败事实——
/// 不把「这次打不开」写成「作品被删除」（材料身份原样留着），也不把租约收成「整单耗尽」
/// （相邻作品照常可执行）。
#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn an_explicitly_unavailable_page_stops_only_its_lanes_without_fabricating_deletion() {
    let database = proof_database_for("collection_dispatch_explicit_page_unavailable").await;
    let fixture = seed_creator_work_order(&database).await;
    for (ordinal, content_external_id) in [(1, "explicit-unavailable"), (2, "still-available")] {
        submit_profile_discovery(
            &database,
            content_external_id,
            &format!(
                "https://www.xiaohongshu.com/explore/{content_external_id}?xsec_token=SIGNED_FIXTURE&xsec_source=pc_user"
            ),
        )
        .await;
        let content_public_ref: Uuid = sqlx::query_scalar(
            "SELECT public_ref FROM linggan_material_content \
             WHERE platform='xhs' AND content_external_id=$1",
        )
        .bind(content_external_id)
        .fetch_one(database.pool())
        .await
        .expect("accepted discovery creates the stable material identity");
        sqlx::query(
            "INSERT INTO collection_work_order_material_target \
                 (work_order_ref,content_public_ref,ordinal,comment_limit,reply_expand_limit,acquire_media) \
             VALUES ($1,$2,$3,30,2,true)",
        )
        .bind(fixture.work_order_ref)
        .bind(content_public_ref)
        .bind(ordinal)
        .execute(database.pool())
        .await
        .expect("both exact materials are frozen by the same approved WorkOrder");
    }
    let lease = issue_work_order_lease(&database, fixture.work_order_ref, 60)
        .await
        .expect("deepening lease is issued");
    let counts_before = package_and_receipt_counts(&database).await;
    let first = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("first detail work is claimed");
    let first_task_id = task_id(&first);
    assert_eq!(
        task_from_dispatch(&first).raw()["target"]["contentExternalId"],
        "explicit-unavailable"
    );

    let failure_ref = Uuid::new_v4();
    let outcome = requeue_failed_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
        first_task_id,
        failure_ref,
        DispatchFailureCode::PageUnavailable,
    )
    .await
    .expect("a producer-confirmed unavailable page is a bounded stop");
    assert_eq!(outcome, DispatchFailureOutcome::Unavailable);
    assert_task_state(&database, first_task_id, "unavailable").await;
    let claim_owner: Option<Uuid> = sqlx::query_scalar(
        "SELECT claimed_by_installation_ref FROM collection_work_order_lease_task WHERE task_id=$1",
    )
    .bind(first_task_id)
    .fetch_one(database.pool())
    .await
    .expect("the stopped lane remains readable");
    assert_eq!(claim_owner, None, "停止的通道不再由任何工位持有");

    let (code, disposition): (String, String) = sqlx::query_as(
        "SELECT failure_code,failure_disposition \
         FROM collection_work_order_lease_task_dispatch_failure WHERE failure_ref=$1",
    )
    .bind(failure_ref)
    .fetch_one(database.pool())
    .await
    .expect("the stop stays an append-only dispatch fact");
    assert_eq!(code, "page_unavailable");
    assert_eq!(
        disposition, "unavailable",
        "明确不可用是停止（unavailable），不是有界重试后的 blocked，也不是可再排的 requeued——\
         三种分类各归各的"
    );

    let stopped_lanes: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order_lease_task task \
         JOIN linggan_runtime_task runtime USING(task_id) \
         WHERE runtime.task_spec #>> '{target,contentExternalId}'='explicit-unavailable' \
           AND task.execution_state='unavailable'",
    )
    .fetch_one(database.pool())
    .await
    .expect("stopped lanes remain visible as current execution facts");
    assert_eq!(stopped_lanes, 4, "停的只有这一篇自己的四个通道");
    let neighbour_lanes: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order_lease_task task \
         JOIN linggan_runtime_task runtime USING(task_id) \
         WHERE runtime.task_spec #>> '{target,contentExternalId}'='still-available' \
           AND task.execution_state='pending'",
    )
    .fetch_one(database.pool())
    .await
    .expect("the neighbouring material keeps its own lanes");
    assert_eq!(neighbour_lanes, 4, "同一张工单里的相邻作品不被连坐");

    let attempts: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_runtime_attempt WHERE task_id=$1")
            .bind(first_task_id)
            .fetch_one(database.pool())
            .await
            .expect("attempt history is readable");
    assert_eq!(attempts, 0, "页面不可用不是一次 Attempt");
    assert_eq!(
        package_and_receipt_counts(&database).await,
        counts_before,
        "停止不制造 Package 或 Receipt"
    );
    let material_intact: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_material_content \
         WHERE platform='xhs' AND content_external_id='explicit-unavailable'",
    )
    .fetch_one(database.pool())
    .await
    .expect("the material identity is readable after the stop");
    assert_eq!(
        material_intact, 1,
        "「这一页这次打不开」不是「作品被删除」：材料身份原样留着"
    );
    assert!(
        lease_is_live(&database, lease.lease_ref).await,
        "还有可执行的相邻成员时，租约不因这一篇停止而收束"
    );
    let work_order_state: String = sqlx::query_scalar(
        "SELECT queue_state FROM collection_work_order WHERE work_order_ref=$1",
    )
    .bind(fixture.work_order_ref)
    .fetch_one(database.pool())
    .await
    .expect("the work order state is readable");
    assert_eq!(
        work_order_state, "leased",
        "一张工单里的一篇不可用不等于整单耗尽"
    );

    let next = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("the neighbouring material is still eligible");
    assert_eq!(
        task_from_dispatch(&next).raw()["target"]["contentExternalId"],
        "still-available"
    );

    let replay = requeue_failed_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
        first_task_id,
        failure_ref,
        DispatchFailureCode::PageUnavailable,
    )
    .await
    .expect("a lost terminal acknowledgement remains idempotent");
    assert_eq!(replay, DispatchFailureOutcome::Unavailable);
    let failure_rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order_lease_task_dispatch_failure WHERE task_id=$1",
    )
    .bind(first_task_id)
    .fetch_one(database.pool())
    .await
    .expect("failure rows are readable");
    assert_eq!(failure_rows, 1, "重放不追加第二条失败事实");
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn rejected_capture_package_ends_only_its_lane_without_reopening_the_page() {
    let database = proof_database_for("collection_dispatch_capture_delivery_rejected").await;
    let fixture = seed_creator_work_order(&database).await;
    let content_external_id = "delivery-rejected-note";
    submit_profile_discovery(
        &database,
        content_external_id,
        "https://www.xiaohongshu.com/explore/delivery-rejected-note?xsec_token=SIGNED_FIXTURE&xsec_source=pc_user",
    )
    .await;
    let content_public_ref: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE platform='xhs' AND content_external_id=$1",
    )
    .bind(content_external_id)
    .fetch_one(database.pool())
    .await
    .expect("accepted discovery creates the frozen material identity");
    sqlx::query(
        "INSERT INTO collection_work_order_material_target \
             (work_order_ref,content_public_ref,ordinal,comment_limit,reply_expand_limit,acquire_media) \
         VALUES ($1,$2,1,30,2,true)",
    )
    .bind(fixture.work_order_ref)
    .bind(content_public_ref)
    .execute(database.pool())
    .await
    .expect("the work order freezes every approved detail lane");
    issue_work_order_lease(&database, fixture.work_order_ref, 60)
        .await
        .expect("a deepening lease is issued");
    let first = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("the page-owning detail task is claimed");
    let first_task_id = task_id(&first);
    assert_eq!(
        task_from_dispatch(&first).raw()["capabilitiesRequested"][0],
        "content_detail"
    );
    assert_eq!(
        requeue_failed_dispatch(
            &database,
            &fixture.install_key,
            &fixture.installation_credential,
            first_task_id,
            Uuid::new_v4(),
            DispatchFailureCode::CaptureDeliveryRejected,
        )
        .await
        .expect("a rejected immutable package is terminal for its task"),
        DispatchFailureOutcome::Unavailable
    );
    let states: Vec<(String, String)> = sqlx::query_as(
        "SELECT runtime.task_spec #>> '{capabilitiesRequested,0}',task.execution_state \
         FROM collection_work_order_lease_task task \
         JOIN linggan_runtime_task runtime USING(task_id) \
         WHERE task.lease_ref=(SELECT lease_ref FROM collection_work_order_lease \
                               WHERE work_order_ref=$1 ORDER BY issued_at DESC LIMIT 1) \
         ORDER BY runtime.task_spec #>> '{capabilitiesRequested,0}'",
    )
    .bind(fixture.work_order_ref)
    .fetch_all(database.pool())
    .await
    .expect("every lane state remains auditable");
    assert_eq!(
        states
            .iter()
            .filter(|(_, state)| state == "unavailable")
            .count(),
        1,
        "only the rejected package lane closes"
    );
    assert_eq!(
        states
            .iter()
            .filter(|(_, state)| state == "pending")
            .count(),
        3,
        "other cached lanes remain deliverable without another page read"
    );
    let next = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("the next approved lane can use the retained page session");
    assert_ne!(task_id(&next), first_task_id);
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn repeated_detail_read_failure_becomes_blocked_without_stalling_later_materials() {
    let database = proof_database_for("collection_dispatch_page_read_boundary").await;
    let fixture = seed_creator_work_order(&database).await;
    for (ordinal, content_external_id) in [(1, "blocked-first"), (2, "eligible-second")] {
        submit_profile_discovery(
            &database,
            content_external_id,
            &format!(
                "https://www.xiaohongshu.com/explore/{content_external_id}?xsec_token=SIGNED_FIXTURE&xsec_source=pc_user"
            ),
        )
        .await;
        let content_public_ref: Uuid = sqlx::query_scalar(
            "SELECT public_ref FROM linggan_material_content \
             WHERE platform='xhs' AND content_external_id=$1",
        )
        .bind(content_external_id)
        .fetch_one(database.pool())
        .await
        .expect("accepted discovery creates the stable material identity");
        sqlx::query(
            "INSERT INTO collection_work_order_material_target \
                 (work_order_ref,content_public_ref,ordinal,comment_limit,reply_expand_limit,acquire_media) \
             VALUES ($1,$2,$3,30,2,true)",
        )
        .bind(fixture.work_order_ref)
        .bind(content_public_ref)
        .bind(ordinal)
        .execute(database.pool())
        .await
        .expect("both exact materials are frozen by the same approved WorkOrder");
    }

    issue_work_order_lease(&database, fixture.work_order_ref, 60)
        .await
        .expect("deepening lease is issued");
    let mut failure_task_ids = Vec::new();
    for retry in 1..=2 {
        let decision = decide_dispatch(
            &database,
            &fixture.install_key,
            &fixture.installation_credential,
        )
        .await
        .expect("the frozen first detail is eligible until its bounded retry limit");
        assert_eq!(
            task_from_dispatch(&decision).raw()["target"]["contentExternalId"],
            "blocked-first"
        );
        let task_id = task_id(&decision);
        failure_task_ids.push(task_id);
        let outcome = requeue_failed_dispatch(
            &database,
            &fixture.install_key,
            &fixture.installation_credential,
            task_id,
            Uuid::new_v4(),
            DispatchFailureCode::PageReadFailed,
        )
        .await
        .expect("the first two page-read failures are bounded recoveries");
        assert_eq!(
            outcome,
            DispatchFailureOutcome::Requeued {
                retry_after_seconds: 60 * (1 << (retry - 1))
            }
        );
        sqlx::query(
            "UPDATE collection_work_order SET retry_not_before_at=scope_001_now()-interval '1 second' \
             WHERE work_order_ref=$1",
        )
        .bind(fixture.work_order_ref)
        .execute(database.pool())
        .await
        .expect("only the isolated proof clock advances between recoveries");
    }

    let third = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("the exact same frozen material receives its final bounded attempt");
    assert_eq!(
        task_from_dispatch(&third).raw()["target"]["contentExternalId"],
        "blocked-first"
    );
    let third_task_id = task_id(&third);
    failure_task_ids.push(third_task_id);
    let blocked_failure_ref = Uuid::new_v4();
    let outcome = requeue_failed_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
        third_task_id,
        blocked_failure_ref,
        DispatchFailureCode::PageReadFailed,
    )
    .await
    .expect("the third identical detail read failure becomes an explicit terminal boundary");
    assert_eq!(outcome, DispatchFailureOutcome::Blocked);

    let blocked_lanes: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order_lease_task task \
         JOIN linggan_runtime_task runtime USING(task_id) \
         WHERE runtime.task_spec #>> '{target,contentExternalId}'='blocked-first' \
           AND task.execution_state='blocked'",
    )
    .fetch_one(database.pool())
    .await
    .expect("blocked detail lanes remain visible as current execution facts");
    assert_eq!(
        blocked_lanes, 4,
        "only this material's four frozen lanes stop"
    );

    let failure_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order_lease_task_dispatch_failure failure \
         JOIN linggan_runtime_task runtime ON runtime.task_id=failure.task_id \
         WHERE failure.failure_code='page_read_failed' \
           AND runtime.task_spec #>> '{target,contentExternalId}'='blocked-first'",
    )
    .fetch_one(database.pool())
    .await
    .expect("every pre-Attempt failure is retained in the ledger");
    assert_eq!(failure_count, 3);
    let disposition: String = sqlx::query_scalar(
        "SELECT failure_disposition FROM collection_work_order_lease_task_dispatch_failure \
         WHERE failure_ref=$1",
    )
    .bind(blocked_failure_ref)
    .fetch_one(database.pool())
    .await
    .expect("the terminal acknowledgement has a durable replay disposition");
    assert_eq!(disposition, "blocked");
    let attempts: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_runtime_attempt attempt \
         JOIN linggan_runtime_task runtime ON runtime.task_id=attempt.task_id \
         WHERE runtime.task_spec #>> '{target,contentExternalId}'='blocked-first'",
    )
    .fetch_one(database.pool())
    .await
    .expect("attempt ledger is readable");
    assert_eq!(
        attempts, 0,
        "a page-read boundary is not a producer Attempt"
    );

    let replay = requeue_failed_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
        third_task_id,
        blocked_failure_ref,
        DispatchFailureCode::PageReadFailed,
    )
    .await
    .expect("a lost terminal acknowledgement replays its real disposition");
    assert_eq!(replay, DispatchFailureOutcome::Blocked);

    let next = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("later independent frozen material is not held behind the blocked one");
    assert_eq!(
        task_from_dispatch(&next).raw()["target"]["contentExternalId"],
        "eligible-second"
    );
    assert_eq!(failure_task_ids.len(), 3);
}

/// 平台明确把某条签名详情链接带到了 404 时，停的是这一条 URL，不是作品身份。
/// 后来发现链路带回新的签名 URL 后，新工单仍可以正常领取；旧 URL 则绝不再回队打开。
#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn rejected_signed_detail_url_is_never_reopened_but_a_fresh_discovery_url_can_run() {
    let database = proof_database_for("collection_dispatch_rejected_detail_url").await;
    let fixture = seed_creator_work_order(&database).await;
    let content_external_id = "signed-url-rejection-proof";
    let rejected_url = format!(
        "https://www.xiaohongshu.com/explore/{content_external_id}?xsec_token=REJECTED_FIXTURE&xsec_source=pc_user"
    );
    submit_profile_discovery(&database, content_external_id, &rejected_url).await;
    let content_public_ref: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content \
         WHERE platform='xhs' AND content_external_id=$1",
    )
    .bind(content_external_id)
    .fetch_one(database.pool())
    .await
    .expect("accepted discovery creates the stable content identity");
    sqlx::query(
        "INSERT INTO collection_work_order_material_target \
             (work_order_ref,content_public_ref,ordinal,comment_limit,reply_expand_limit,acquire_media) \
         VALUES ($1,$2,1,30,2,true)",
    )
    .bind(fixture.work_order_ref)
    .bind(content_public_ref)
    .execute(database.pool())
    .await
    .expect("the original work order freezes the one material");

    issue_work_order_lease(&database, fixture.work_order_ref, 60)
        .await
        .expect("original detail work is leased");
    let first = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("the signed URL is dispatched once");
    let first_task_id = task_id(&first);
    let dispatched_url = match first {
        DispatchDecision::Dispatch {
            execution_source_url: Some(url),
            ..
        } => url,
        other => panic!("a signed detail URL is required for the first dispatch; got {other:?}"),
    };
    assert_eq!(dispatched_url, rejected_url);
    assert!(matches!(
        grant_detail_page_session(
            &database,
            &fixture.install_key,
            &fixture.installation_credential,
            first_task_id,
            Uuid::new_v4(),
            &dispatched_url,
        )
        .await,
        Ok(linggan_evidence::DetailPageSessionGrant::Authorized { .. })
    ));

    assert_eq!(
        requeue_failed_dispatch(
            &database,
            &fixture.install_key,
            &fixture.installation_credential,
            first_task_id,
            Uuid::new_v4(),
            DispatchFailureCode::DetailPageUrlInvalid,
        )
        .await
        .expect("the explicit platform dead-page fact terminalizes this URL"),
        DispatchFailureOutcome::Blocked
    );
    let stored_session: (String, Option<String>, String) = sqlx::query_as(
        "SELECT state,stop_reason,execution_source_url_sha256 \
         FROM collection_detail_page_session WHERE initial_lease_ref=( \
             SELECT lease_ref FROM collection_work_order_lease \
             WHERE work_order_ref=$1 ORDER BY issued_at DESC LIMIT 1)",
    )
    .bind(fixture.work_order_ref)
    .fetch_one(database.pool())
    .await
    .expect("the server records a terminal source fingerprint");
    assert_eq!(stored_session.0, "stopped");
    assert_eq!(stored_session.1.as_deref(), Some("detail_page_url_invalid"));
    assert_eq!(
        stored_session.2.len(),
        64,
        "only the SHA-256 fingerprint is stored on the session"
    );
    assert_ne!(
        stored_session.2, rejected_url,
        "the session never stores the raw signed URL"
    );
    assert!(matches!(
        decide_dispatch(
            &database,
            &fixture.install_key,
            &fixture.installation_credential,
        )
        .await,
        Ok(DispatchDecision::NothingWaiting)
    ));

    let fresh_url = format!(
        "https://www.xiaohongshu.com/explore/{content_external_id}?xsec_token=FRESH_FIXTURE&xsec_source=pc_user"
    );
    submit_profile_discovery(&database, content_external_id, &fresh_url).await;
    let new_work_order_ref = seed_second_queued_work_order(&database, &fixture).await;
    sqlx::query(
        "INSERT INTO collection_work_order_material_target \
             (work_order_ref,content_public_ref,ordinal,comment_limit,reply_expand_limit,acquire_media) \
         VALUES ($1,$2,1,30,2,true)",
    )
    .bind(new_work_order_ref)
    .bind(content_public_ref)
    .execute(database.pool())
    .await
    .expect("the later approved work order freezes the same durable content identity");
    let fresh = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("the later work order is eligible");
    assert!(matches!(
        fresh,
        DispatchDecision::Dispatch { execution_source_url: Some(url), .. } if url == fresh_url
    ));
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn released_orphaned_work_order_is_recovered_without_rewriting_old_lease_history() {
    let database = proof_database_for("collection_dispatch_orphaned_work_order").await;
    let fixture = seed_creator_work_order(&database).await;
    let lease = issue_work_order_lease(&database, fixture.work_order_ref, 60)
        .await
        .expect("fixture work is leased once");
    let claimed = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("first historical task is claimed");
    let historical_task_id = task_id(&claimed);
    sqlx::query(
        "UPDATE collection_work_order_lease \
         SET released_at=scope_001_now(),release_reason='station_unavailable' \
         WHERE lease_ref=$1",
    )
    .bind(lease.lease_ref)
    .execute(database.pool())
    .await
    .expect("simulate the pre-recovery release defect");
    assert_eq!(
        recover_released_orphaned_work_orders(&database)
            .await
            .expect("released orphan is safely returned to the queue"),
        1
    );
    let state: String =
        sqlx::query_scalar("SELECT queue_state FROM collection_work_order WHERE work_order_ref=$1")
            .bind(fixture.work_order_ref)
            .fetch_one(database.pool())
            .await
            .expect("recovered work order is readable");
    assert_eq!(state, "queued");
    assert_task_state(&database, historical_task_id, "in_progress").await;
    assert!(!lease_is_live(&database, lease.lease_ref).await);
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn late_scheduled_submission_keeps_material_without_advancing_revoked_execution() {
    let database = proof_database_for("collection_dispatch_submission_fence").await;
    let fixture = seed_creator_work_order(&database).await;
    let lease = issue_work_order_lease(&database, fixture.work_order_ref, 60)
        .await
        .expect("creator work order is leased");
    let dispatch = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("task is claimed");
    let task = task_from_dispatch(&dispatch);
    let attempt = attempt(task.task_id(), fixture.producer_instance_id);
    assert!(matches!(
        start_producer_attempt(&database, &attempt).await,
        Ok(RuntimeAttemptOutcome::Started { .. })
    ));
    sqlx::query(
        "UPDATE collection_work_order_lease \
         SET released_at = scope_001_now(), release_reason = 'revoked' WHERE lease_ref = $1",
    )
    .bind(lease.lease_ref)
    .execute(database.pool())
    .await
    .expect("lease is revoked after attempt start");

    let submission = scheduled_submission(&task, &attempt, fixture.producer_instance_id);
    let outcome = submit_producer_package(&database, &submission)
        .await
        .expect("already observed material is retained after authority loss");
    assert!(matches!(
        outcome,
        RuntimeSubmissionOutcome::Acknowledged {
            ref execution_effect,
            ref material_admission,
            ..
        } if execution_effect == "LOST_AUTHORITY" && material_admission == "ACCEPTED"
    ));
    let package_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_runtime_capture_package")
            .fetch_one(database.pool())
            .await
            .expect("package count is readable");
    let receipt_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_runtime_submission_receipt")
            .fetch_one(database.pool())
            .await
            .expect("receipt count is readable");
    assert_eq!((package_count, receipt_count), (1, 1));
    assert!(lease_is_live(&database, lease.lease_ref).await == false);
    assert_task_state(&database, task.task_id(), "in_progress").await;
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn a_lost_submission_response_still_replays_after_its_lease_closed() {
    let database = proof_database_for("collection_dispatch_lost_response_replay").await;
    let fixture = seed_creator_work_order(&database).await;
    let lease = issue_work_order_lease(&database, fixture.work_order_ref, 60)
        .await
        .expect("creator work order receives one ordered lease");
    let first = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("first task is claimed");
    let first_task = task_from_dispatch(&first);
    let first_attempt = attempt(first_task.task_id(), fixture.producer_instance_id);
    assert!(matches!(
        start_producer_attempt(&database, &first_attempt).await,
        Ok(RuntimeAttemptOutcome::Started { .. })
    ));
    let first_submission =
        scheduled_submission(&first_task, &first_attempt, fixture.producer_instance_id);
    let acknowledged = submit_producer_package(&database, &first_submission)
        .await
        .expect("first delivery is accepted");
    let RuntimeSubmissionOutcome::Acknowledged { receipt_ref, .. } = acknowledged else {
        panic!("first delivery must be acknowledged, got {acknowledged:?}");
    };

    // The browser never received that response, so its durable outbox row stays and will be
    // replayed. Meanwhile the remaining frozen lane finishes and its own delivery closes the
    // lease — the exact state in which the client has already earned a receipt it cannot see.
    let second = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("second sequence dispatch decides");
    run_scheduled_task(
        &database,
        &task_from_dispatch(&second),
        fixture.producer_instance_id,
    )
    .await;
    assert!(!lease_is_live(&database, lease.lease_ref).await);

    // Recovery must not depend on the closed lease. The Attempt is a durable fact, so an
    // identity-matched replay is owed even after its execution authority ends; a *new* Attempt
    // still requires live authority, which the ordered-completion proof keeps covering.
    assert!(matches!(
        start_producer_attempt(&database, &first_attempt).await,
        Ok(RuntimeAttemptOutcome::Replay { .. })
    ));
    let before = package_and_receipt_counts(&database).await;
    let replayed = submit_producer_package(&database, &first_submission)
        .await
        .expect("replay recovers the receipt the browser never received");
    assert!(
        matches!(
            replayed,
            RuntimeSubmissionOutcome::Replay { receipt_ref: ref recovered, .. }
                if *recovered == receipt_ref
        ),
        "replay must return the original receipt, got {replayed:?}"
    );
    assert_eq!(
        package_and_receipt_counts(&database).await,
        before,
        "replaying a delivered package must not mint a second package or receipt"
    );
}

/// 导航前登记的通道交付身份，必须让「页面已读完、当时服务端不可达」的包仍有身份可投递。
///
/// 只把重放顺序提前是不够的：身份是投递时才生成的，断网期间取回的包在租约关闭后连一个
/// 服务端认识的身份都没有。这条用例同时钉住两件事——身份在授权事务里一次登记、重放拿回
/// 同一个；登记**不**等于执行，通道任务不会被伪装成执行中，Attempt 也仍然只在真的投递时出现。
#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn navigation_time_lane_identities_outlive_a_closed_lease_without_impersonating_execution() {
    let database = proof_database_for("collection_dispatch_lane_preparation").await;
    let fixture = seed_creator_work_order(&database).await;
    let content_external_id = "note-lane-preparation";
    let signed_url = format!(
        "https://www.xiaohongshu.com/user/profile/creator-fixture/{content_external_id}?xsec_token=SIGNED_PREPARATION%3D&xsec_source=pc_user"
    );
    submit_profile_discovery(&database, content_external_id, &signed_url).await;
    let content_public_ref: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE platform='xhs' AND content_external_id=$1",
    )
    .bind(content_external_id)
    .fetch_one(database.pool())
    .await
    .expect("accepted discovery creates the stable content identity");
    sqlx::query(
        "INSERT INTO collection_work_order_material_target \
         (work_order_ref,content_public_ref,ordinal,comment_limit,reply_expand_limit,acquire_media) \
         VALUES ($1,$2,1,30,2,true)",
    )
    .bind(fixture.work_order_ref)
    .bind(content_public_ref)
    .execute(database.pool())
    .await
    .expect("the work order freezes all four lanes");
    let lease = issue_work_order_lease(&database, fixture.work_order_ref, 60)
        .await
        .expect("material deepening lease is issued");
    let dispatch = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("detail dispatch is decided");
    let task = task_from_dispatch(&dispatch);
    let execution_source_url = match &dispatch {
        DispatchDecision::Dispatch {
            execution_source_url: Some(url),
            ..
        } => url.clone(),
        other => panic!("signed discovery must produce a detail dispatch; got {other:?}"),
    };

    let grant_request_id = Uuid::new_v4();
    let first = grant_detail_page_session_with_lane_deliveries(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
        task.task_id(),
        grant_request_id,
        &execution_source_url,
    )
    .await
    .expect("the authorized page session is prepared before navigation");
    let (session_ref, plan, prepared) = match first {
        DetailPageSessionGrant::Authorized {
            session_ref,
            plan,
            prepared_lanes,
        } => (session_ref, plan, prepared_lanes),
        other => panic!("first preparation must authorize, got {other:?}"),
    };
    let planned_lanes: Vec<String> = plan["lanes"]
        .as_array()
        .expect("the frozen plan lists lanes")
        .iter()
        .map(|lane| lane.as_str().expect("lane is a capability name").to_owned())
        .collect();
    assert_eq!(
        prepared
            .iter()
            .map(|lane| lane.capability.clone())
            .collect::<Vec<_>>(),
        planned_lanes,
        "every frozen lane gets exactly one identity, in plan order"
    );
    let attempt_ids: std::collections::HashSet<Uuid> =
        prepared.iter().map(|lane| lane.attempt_id).collect();
    assert_eq!(
        attempt_ids.len(),
        prepared.len(),
        "one lane must never share another lane's delivery identity"
    );

    // 准备不是执行：这张租约下还没有任何通道 Attempt，通道任务也没有被说成「正在采集」。
    // （只统计本租约：夹具自己那条发现任务早就有 Attempt 了，全局计数会把两件事混在一起。）
    let lane_attempt_rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_runtime_attempt attempt \
         JOIN collection_work_order_lease_task lease_task ON lease_task.task_id = attempt.task_id \
         WHERE lease_task.lease_ref = $1",
    )
    .bind(lease.lease_ref)
    .fetch_one(database.pool())
    .await
    .expect("lease task attempts are readable");
    assert_eq!(
        lane_attempt_rows, 0,
        "registering delivery identities must not mint an Attempt"
    );
    for lane in &prepared {
        let expected_state = if lane.capability == "content_detail" {
            "in_progress"
        } else {
            "pending"
        };
        assert_task_state(&database, lane.task_id, expected_state).await;
    }

    // 准备响应丢失后，同一个 grant_request_id 必须拿回同一份身份。
    let replayed = grant_detail_page_session_with_lane_deliveries(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
        task.task_id(),
        grant_request_id,
        &execution_source_url,
    )
    .await
    .expect("a lost preparation response is recoverable");
    match replayed {
        DetailPageSessionGrant::Replay {
            session_ref: replay_session,
            prepared_lanes,
            ..
        } => {
            assert_eq!(replay_session, session_ref);
            assert_eq!(prepared_lanes, prepared, "replay owes the same identities");
        }
        other => panic!("repeating the same request must replay, got {other:?}"),
    }

    // 页面读完、服务端此后不可达：租约关闭，通道任务从未被单独领取。
    sqlx::query(
        "UPDATE collection_work_order_lease \
         SET released_at = scope_001_now(), release_reason = 'revoked' WHERE lease_ref = $1",
    )
    .bind(lease.lease_ref)
    .execute(database.pool())
    .await
    .expect("lease is revoked after the page was read");
    assert!(!lease_is_live(&database, lease.lease_ref).await);

    let media_lane = prepared
        .iter()
        .find(|lane| lane.capability == "media_slots")
        .expect("the frozen plan contains the media lane");
    let comments_lane = prepared
        .iter()
        .find(|lane| lane.capability == "comments")
        .expect("the frozen plan contains the comments lane");

    // 服务端不认识的身份仍然没有执行权：换一条通道、换一个工位、或这台安装已被替换。
    let foreign = parse_producer_attempt(
        &serde_json::json!({
            "contractVersion": "linggan.producer.attempt.v1",
            "producerInstanceId": Uuid::new_v4(),
            "taskId": comments_lane.task_id,
            "attemptId": media_lane.attempt_id,
        })
        .to_string(),
    )
    .expect("foreign attempt is well-formed");
    assert!(
        matches!(
            start_producer_attempt(&database, &foreign).await,
            Err(ProducerRuntimeError::ScheduledTaskNotClaimed)
        ),
        "a prepared identity is bound to its own task and its own installation"
    );
    let crossed = parse_producer_attempt(
        &serde_json::json!({
            "contractVersion": "linggan.producer.attempt.v1",
            "producerInstanceId": fixture.producer_instance_id,
            "taskId": comments_lane.task_id,
            "attemptId": media_lane.attempt_id,
        })
        .to_string(),
    )
    .expect("crossed attempt is well-formed");
    assert!(
        matches!(
            start_producer_attempt(&database, &crossed).await,
            Err(ProducerRuntimeError::ScheduledTaskNotClaimed)
        ),
        "the right installation with the wrong lane task is still not an execution right"
    );

    // 同一条通道、同一个工位、服务端铸造的身份：晚到的包仍然有身份可投递。
    let media_attempt = parse_producer_attempt(
        &serde_json::json!({
            "contractVersion": "linggan.producer.attempt.v1",
            "producerInstanceId": fixture.producer_instance_id,
            "taskId": media_lane.task_id,
            "attemptId": media_lane.attempt_id,
        })
        .to_string(),
    )
    .expect("prepared attempt is well-formed");
    assert!(
        matches!(
            start_producer_attempt(&database, &media_attempt).await,
            Ok(RuntimeAttemptOutcome::Started { .. })
        ),
        "a delivery identity registered before navigation survives its closed lease"
    );
    let media_task_spec: serde_json::Value =
        sqlx::query_scalar("SELECT task_spec FROM linggan_runtime_task WHERE task_id = $1")
            .bind(media_lane.task_id)
            .fetch_one(database.pool())
            .await
            .expect("the lane task spec is readable");
    let media_task = parse_producer_task_spec(&media_task_spec.to_string())
        .expect("the stored lane task remains valid");
    let submission = scheduled_submission(&media_task, &media_attempt, fixture.producer_instance_id);
    let outcome = submit_producer_package(&database, &submission)
        .await
        .expect("material observed under the prepared identity is retained");
    assert!(
        matches!(
            outcome,
            RuntimeSubmissionOutcome::Acknowledged {
                ref execution_effect,
                ref material_admission,
                ..
            } if execution_effect == "LOST_AUTHORITY" && material_admission == "ACCEPTED"
        ),
        "a closed lease costs the execution effect, never the material, got {outcome:?}"
    );

    // 安装被替换之后，同一份准备不再构成新的开始依据。取代是记下来的事实：
    // 必须同时写下 superseded_by（0007 的 CHECK 不允许只写一半）。
    let replacement_installation_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO plugin_installation (installation_ref, install_key, plugin_version, capabilities) \
         VALUES ($1, $2, '0.8.54', '[]'::jsonb)",
    )
    .bind(replacement_installation_ref)
    .bind(Uuid::new_v4().to_string())
    .execute(database.pool())
    .await
    .expect("a replacement installation registers");
    sqlx::query(
        "UPDATE plugin_installation \
         SET superseded_at = scope_001_now(), superseded_by = $2 WHERE installation_ref = $1",
    )
    .bind(fixture.installation_ref)
    .bind(replacement_installation_ref)
    .execute(database.pool())
    .await
    .expect("the installation is replaced after the fact");
    let superseded_attempt = parse_producer_attempt(
        &serde_json::json!({
            "contractVersion": "linggan.producer.attempt.v1",
            "producerInstanceId": fixture.producer_instance_id,
            "taskId": comments_lane.task_id,
            "attemptId": comments_lane.attempt_id,
        })
        .to_string(),
    )
    .expect("superseded installation attempt is well-formed");
    assert!(
        matches!(
            start_producer_attempt(&database, &superseded_attempt).await,
            Err(ProducerRuntimeError::ScheduledTaskNotClaimed)
        ),
        "a replaced installation cannot start from a preparation it no longer owns"
    );
}

/// 交付对账回答的是「冻结通道的包到服务端了没有」，不是「会话自己写着什么状态」。
///
/// 一条真实会话从「读完页面、包还在手上」走到「四个通道全部拿到回执」，中途没有写入任何
/// 交付事实：结论只能由回执数出来。已经终结的会话，晚到的交付进度既写不进去（写入侧的
/// 终态守卫），也改不动已经读出的结论（读侧不按通道数把终态降级）。
#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn delivery_reconciliation_counts_receipts_not_the_session_marker() {
    let database = proof_database_for("collection_dispatch_delivery_reconciliation").await;
    let fixture = seed_creator_work_order(&database).await;
    let content_external_id = "note-delivery-reconciliation";
    let signed_url = format!(
        "https://www.xiaohongshu.com/user/profile/creator-fixture/{content_external_id}?xsec_token=SIGNED_RECONCILE%3D&xsec_source=pc_user"
    );
    submit_profile_discovery(&database, content_external_id, &signed_url).await;
    let content_public_ref: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE platform='xhs' AND content_external_id=$1",
    )
    .bind(content_external_id)
    .fetch_one(database.pool())
    .await
    .expect("accepted discovery creates the stable content identity");
    sqlx::query(
        "INSERT INTO collection_work_order_material_target \
         (work_order_ref,content_public_ref,ordinal,comment_limit,reply_expand_limit,acquire_media) \
         VALUES ($1,$2,1,30,2,true)",
    )
    .bind(fixture.work_order_ref)
    .bind(content_public_ref)
    .execute(database.pool())
    .await
    .expect("the work order freezes all four lanes");
    issue_work_order_lease(&database, fixture.work_order_ref, 60)
        .await
        .expect("material deepening lease is issued");
    let dispatch = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("detail dispatch is decided");
    let task = task_from_dispatch(&dispatch);
    let execution_source_url = match &dispatch {
        DispatchDecision::Dispatch {
            execution_source_url: Some(url),
            ..
        } => url.clone(),
        other => panic!("signed discovery must produce a detail dispatch; got {other:?}"),
    };
    let (session_ref, prepared) = match grant_detail_page_session_with_lane_deliveries(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
        task.task_id(),
        Uuid::new_v4(),
        &execution_source_url,
    )
    .await
    .expect("the authorized page session is prepared before navigation")
    {
        DetailPageSessionGrant::Authorized {
            session_ref,
            prepared_lanes,
            ..
        } => (session_ref, prepared_lanes),
        other => panic!("first preparation must authorize, got {other:?}"),
    };
    assert_eq!(
        prepared.len(),
        4,
        "the frozen plan owes all four lanes, got {prepared:?}"
    );

    // 只拿到授权、还没消费导航：没有任何包可以交付，这一行不该出现在对账里，更不该
    // 被读成「欠着四个包」。
    assert!(
        read_detail_delivery_reconciliation(&database, 100)
            .await
            .expect("delivery reconciliation is readable")
            .is_empty(),
        "an authorized session that never consumed navigation owes no delivery"
    );

    report_session_progress(
        &database,
        &fixture,
        task.task_id(),
        session_ref,
        DetailPageSessionProgress::NavigationObserved,
    )
    .await;

    // 页面读完、包还在本地：四个通道一个回执都没有，依据是 0/4 而不是会话的措辞。
    let waiting = only_session(&database).await;
    assert_eq!(waiting.conclusion, DeliveryConclusion::AwaitingDelivery);
    assert_eq!((waiting.prepared_lanes, waiting.delivered_lanes), (4, 0));
    assert_eq!(
        waiting.state, "navigation_committed",
        "the read model does not rewrite the session's own state"
    );

    // 三个通道到岸、一个还在路上：结论不变，依据变成 3/4。
    for lane in prepared.iter().take(3) {
        deliver_prepared_lane(&database, &fixture, lane).await;
    }
    let partial = only_session(&database).await;
    assert_eq!(partial.conclusion, DeliveryConclusion::AwaitingDelivery);
    assert_eq!((partial.prepared_lanes, partial.delivered_lanes), (4, 3));

    // 会话自己说「已保存待交付」，四个通道却都拿到了回执：按回执归并，结论是已交付。
    // 会话的 delivery_pending 只是它自己的说法，不能直接顶替结论（T22）。
    report_session_progress(
        &database,
        &fixture,
        task.task_id(),
        session_ref,
        DetailPageSessionProgress::DeliveryPending,
    )
    .await;
    deliver_prepared_lane(&database, &fixture, &prepared[3]).await;
    let delivered = only_session(&database).await;
    assert_eq!(delivered.conclusion, DeliveryConclusion::Delivered);
    assert_eq!(delivered.state, "delivery_pending");

    // 平台风控停止：结论换成已终结，原因与终结时刻原样带出。
    report_session_progress(
        &database,
        &fixture,
        task.task_id(),
        session_ref,
        DetailPageSessionProgress::Stopped { reason: "risk_stop" },
    )
    .await;
    let closed = only_session(&database).await;
    assert_eq!(closed.conclusion, DeliveryConclusion::Closed);
    assert_eq!(closed.stop_reason.as_deref(), Some("risk_stop"));
    assert!(closed.closed_at.is_some(), "a terminal session records when");

    // 晚到的交付进度不能把终态降级：写入侧拒绝这条事实，读侧结论原封不动（T27）。
    let late = record_detail_page_session_progress(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
        task.task_id(),
        session_ref,
        DetailPageSessionProgress::DeliveryPending,
    )
    .await;
    assert!(
        matches!(late, Err(DetailPageSessionNavigationError::SessionNotHeld)),
        "已终结的会话必须拒绝晚到的交付进度，实际得到 {late:?}"
    );
    let after_late_progress = only_session(&database).await;
    assert_eq!(after_late_progress.conclusion, DeliveryConclusion::Closed);
    assert_eq!(after_late_progress.state, "stopped");

    // 五种终结原因都要能原样读出：读层不挑原因，页面的中文映射各自成立。
    for reason in [
        "navigation_state_unknown",
        "owner_unavailable",
        "page_unavailable",
        "risk_stop",
        "delivery_terminal",
    ] {
        sqlx::query("UPDATE collection_detail_page_session SET stop_reason = $2 WHERE session_ref = $1")
            .bind(session_ref)
            .bind(reason)
            .execute(database.pool())
            .await
            .expect("the terminal reason is restated");
        let row = only_session(&database).await;
        assert_eq!(row.conclusion, DeliveryConclusion::Closed);
        assert_eq!(row.stop_reason.as_deref(), Some(reason));
    }
}

/// 跨行业参照物的来源表不是每个 schema 都装：0089 为 `cross_industry_sample_ref` 写下的列注释
/// 点名了这件事，那一列也因此故意不带外键。于是「会话在、样本身份读不到」是一个真实状态——
/// 对账必须照常给出这行，缺的是一张参照物表，不是这条会话；更不能让整页变成「读取失败」。
///
/// 这条用例所在的证明 schema 恰好就是那个状态：装着 detail page session，没装跨行业来源表。
#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn a_session_without_its_cross_industry_table_still_reconciles() {
    let database = proof_database_for("collection_dispatch_cross_industry_absent").await;
    let fixture = seed_creator_work_order(&database).await;
    let lease = issue_work_order_lease(&database, fixture.work_order_ref, 60)
        .await
        .expect("lease is issued");
    let table_exists: bool =
        sqlx::query_scalar("SELECT to_regclass('cross_industry_sample') IS NOT NULL")
            .fetch_one(database.pool())
            .await
            .expect("the schema is inspected");
    assert!(!table_exists, "这条用例只在跨行业来源表缺席时才有意义");

    let session_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_detail_page_session \
         (session_ref,work_order_ref,content_public_ref,cross_industry_sample_ref, \
          owner_installation_ref,grant_request_id,initial_lease_ref,plan_snapshot,plan_hash,state) \
         VALUES ($1,$2,NULL,$3,$4,$5,$6,'{}'::jsonb,repeat('a',64),'navigation_started')",
    )
    .bind(session_ref)
    .bind(fixture.work_order_ref)
    .bind(Uuid::new_v4())
    .bind(fixture.installation_ref)
    .bind(Uuid::new_v4())
    .bind(lease.lease_ref)
    .execute(database.pool())
    .await
    .expect("a cross-industry session row is seeded");

    let rows = read_detail_delivery_reconciliation(&database, 100)
        .await
        .expect("缺一张参照物表不该让整页交付对账读取失败");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].session_ref, session_ref);
    assert_eq!(rows[0].conclusion, DeliveryConclusion::RecoveryUnverified);
    assert_eq!(rows[0].platform, None);
    assert_eq!(rows[0].content_external_id, None);
}

async fn only_session(database: &Database) -> linggan_evidence::DetailDeliveryReconciliation {
    let mut rows = read_detail_delivery_reconciliation(database, 100)
        .await
        .expect("delivery reconciliation is readable");
    assert_eq!(rows.len(), 1, "exactly one consumed session is in scope");
    rows.remove(0)
}

async fn report_session_progress(
    database: &Database,
    fixture: &Fixture,
    task_id: Uuid,
    session_ref: Uuid,
    progress: DetailPageSessionProgress,
) {
    record_detail_page_session_progress(
        database,
        &fixture.install_key,
        &fixture.installation_credential,
        task_id,
        session_ref,
        progress,
    )
    .await
    .expect("the owning installation reports its own session progress");
}

/// 用导航前登记好的身份投递一个通道的包，并证明它换回了回执。
async fn deliver_prepared_lane(database: &Database, fixture: &Fixture, lane: &PreparedLaneDelivery) {
    let attempt = parse_producer_attempt(
        &serde_json::json!({
            "contractVersion": "linggan.producer.attempt.v1",
            "producerInstanceId": fixture.producer_instance_id,
            "taskId": lane.task_id,
            "attemptId": lane.attempt_id,
        })
        .to_string(),
    )
    .expect("a prepared lane attempt is well-formed");
    let started = start_producer_attempt(database, &attempt)
        .await
        .expect("a prepared lane identity survives to its delivery");
    assert!(
        !matches!(started, RuntimeAttemptOutcome::Conflict { .. }),
        "the {} lane must not conflict, got {started:?}",
        lane.capability
    );
    let spec: serde_json::Value =
        sqlx::query_scalar("SELECT task_spec FROM linggan_runtime_task WHERE task_id = $1")
            .bind(lane.task_id)
            .fetch_one(database.pool())
            .await
            .expect("the lane task spec is readable");
    let task =
        parse_producer_task_spec(&spec.to_string()).expect("the stored lane task remains valid");
    let submission = scheduled_submission(&task, &attempt, fixture.producer_instance_id);
    let outcome = submit_producer_package(database, &submission)
        .await
        .expect("a lane package is retained under its prepared identity");
    assert!(
        matches!(outcome, RuntimeSubmissionOutcome::Acknowledged { .. }),
        "the {} lane must be acknowledged, got {outcome:?}",
        lane.capability
    );
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn replacement_installation_releases_stale_work_instead_of_adopting_it() {
    let database = proof_database_for("collection_dispatch_installation_takeover").await;
    let fixture = seed_creator_work_order(&database).await;
    let lease = issue_work_order_lease(&database, fixture.work_order_ref, 60)
        .await
        .expect("creator work order is leased");
    let old_dispatch = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("old installation claims the first task");
    let original_task_id = task_id(&old_dispatch);

    open_claim_window(&database, fixture.station_ref, 1)
        .await
        .expect("the same station accepts the replacement installation");
    let intermediate_instance_id = Uuid::new_v4();
    let intermediate_install_key = intermediate_instance_id.to_string();
    let intermediate = check_in_installation(
        &database,
        &InstallationCheckIn {
            install_key: &intermediate_install_key,
            plugin_version: "0.8.47",
            browser_label: Some("intermediate fixture"),
            capabilities: serde_json::json!(["author_profile", "profile_discovery"]),
            installation_credential: None,
        },
    )
    .await
    .expect("intermediate installation checks in");
    let intermediate_installation_ref = match intermediate {
        CheckInOutcome::Claimed {
            installation_ref,
            superseded,
            ..
        } => {
            assert_eq!(superseded, Some(fixture.installation_ref));
            installation_ref
        }
        other => panic!("intermediate install must claim the station; got {other:?}"),
    };
    assert!(
        !lease_is_live(&database, lease.lease_ref).await,
        "superseding an installation releases its old frozen lease instead of silently adopting it"
    );

    let replacement_instance_id = Uuid::new_v4();
    let replacement_install_key = replacement_instance_id.to_string();
    let outcome = check_in_installation(
        &database,
        &InstallationCheckIn {
            install_key: &replacement_install_key,
            plugin_version: "0.8.47",
            browser_label: Some("replacement fixture"),
            capabilities: serde_json::json!([
                "author_profile",
                "profile_discovery",
                "content_detail",
                "media_slots",
                "comments",
                "replies"
            ]),
            installation_credential: None,
        },
    )
    .await
    .expect("replacement installation checks in");
    let replacement_installation_ref = match &outcome {
        CheckInOutcome::Claimed {
            installation_ref,
            station_ref,
            superseded,
            ..
        } => {
            assert_eq!(*station_ref, fixture.station_ref);
            assert_eq!(*superseded, Some(intermediate_installation_ref));
            *installation_ref
        }
        other => panic!("replacement must claim the open station; got {other:?}"),
    };
    let owner: Option<Uuid> = sqlx::query_scalar(
        "SELECT claimed_by_installation_ref FROM collection_work_order_lease_task WHERE task_id=$1",
    )
    .bind(original_task_id)
    .fetch_one(database.pool())
    .await
    .expect("task owner is readable");
    assert_ne!(owner, Some(replacement_installation_ref));
    assert_eq!(task_id(&old_dispatch), original_task_id);
    assert!(
        !lease_is_live(&database, lease.lease_ref).await,
        "a replacement requires a fresh account binding, admission and lease"
    );
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn detail_dispatch_uses_the_latest_accepted_signed_discovery_url_outside_task_spec() {
    let database = proof_database_for("collection_dispatch_signed_source").await;
    let fixture = seed_creator_work_order(&database).await;
    let content_external_id = "note-signed-execution";
    let signed_url = format!(
        "https://www.xiaohongshu.com/user/profile/creator-fixture/{content_external_id}?xsec_token=SIGNED_FIXTURE%3D&xsec_source=pc_user"
    );
    submit_profile_discovery(&database, content_external_id, &signed_url).await;
    let query: EvidenceQuery = serde_json::from_value(serde_json::json!({
        "scope":"all_accepted_material","window":"latest_accepted_discovery","sort":"latest_discovery"
    }))
    .expect("work resource query is valid");
    let resources = read_work_resources(&database, &query)
        .await
        .expect("shared work resource interface reads the accepted discovery");
    let resource = resources
        .items
        .first()
        .expect("one work resource is projected");
    assert_eq!(
        resource.collection_context.target_display_name.as_deref(),
        Some("顺序派发夹具")
    );
    assert_eq!(
        resource.collection_context.relationship_state,
        "OBSERVED_ON_TARGET_SURFACE"
    );
    assert_eq!(
        resource.collection_context.author_identity_match_state,
        "NOT_VERIFIED"
    );
    let content_public_ref: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE platform='xhs' AND content_external_id=$1",
    )
    .bind(content_external_id)
    .fetch_one(database.pool())
    .await
    .expect("accepted discovery creates the stable content identity");
    sqlx::query(
        "INSERT INTO collection_work_order_material_target \
         (work_order_ref,content_public_ref,ordinal,comment_limit,reply_expand_limit,acquire_media) \
         VALUES ($1,$2,1,30,2,true)",
    )
    .bind(fixture.work_order_ref)
    .bind(content_public_ref)
    .execute(database.pool())
    .await
    .expect("the work order freezes one authorized material target");

    issue_work_order_lease(&database, fixture.work_order_ref, 60)
        .await
        .expect("material deepening lease is issued");
    let dispatch = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("detail dispatch is decided");
    match dispatch {
        DispatchDecision::Dispatch {
            task_spec,
            execution_source_url,
            page_session_plan,
            ..
        } => {
            assert_eq!(task_spec["capabilitiesRequested"][0], "content_detail");
            assert_eq!(
                task_spec["target"]["contentExternalId"],
                content_external_id
            );
            assert!(
                !task_spec.to_string().contains("xsec_token"),
                "short-lived execution credentials never enter immutable TaskSpec"
            );
            assert_eq!(execution_source_url.as_deref(), Some(signed_url.as_str()));
            let plan = page_session_plan.expect("fixed detail work exposes one same-page plan");
            assert_eq!(plan["contractVersion"], "linggan.detail-page-session.v1");
            assert_eq!(plan["contentExternalId"], content_external_id);
            assert_eq!(
                plan["lanes"],
                serde_json::json!(["content_detail", "media_slots", "comments", "replies"])
            );
            assert_eq!(plan["commentLimit"], 30);
            assert_eq!(plan["replyExpandLimit"], 2);
            assert!(
                plan["cacheTtlSeconds"]
                    .as_i64()
                    .is_some_and(|value| value > 0)
            );
        }
        other => panic!("signed discovery must produce a detail dispatch; got {other:?}"),
    }
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn detail_only_scope_claims_only_detail_and_rejects_orphaned_replies() {
    let database = proof_database_for("collection_dispatch_detail_only_scope").await;
    let fixture = seed_creator_work_order(&database).await;
    let content_external_id = "note-detail-only-execution";
    submit_profile_discovery(
        &database,
        content_external_id,
        "https://www.xiaohongshu.com/user/profile/creator-fixture/note-detail-only-execution?xsec_token=SIGNED_DETAIL_ONLY%3D&xsec_source=pc_user",
    )
    .await;
    let content_public_ref: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE platform='xhs' AND content_external_id=$1",
    )
    .bind(content_external_id)
    .fetch_one(database.pool())
    .await
    .expect("accepted discovery creates the stable content identity");
    sqlx::query(
        "INSERT INTO collection_work_order_material_target \
         (work_order_ref,content_public_ref,ordinal,comment_limit,reply_expand_limit,acquire_media) \
         VALUES ($1,$2,1,0,0,false)",
    )
    .bind(fixture.work_order_ref)
    .bind(content_public_ref)
    .execute(database.pool())
    .await
    .expect("detail-only is an accepted frozen scope");
    let invalid_reply_scope = sqlx::query(
        "INSERT INTO collection_work_order_material_target \
         (work_order_ref,content_public_ref,ordinal,comment_limit,reply_expand_limit,acquire_media) \
         VALUES ($1,$2,2,0,1,false)",
    )
    .bind(fixture.work_order_ref)
    .bind(content_public_ref)
    .execute(database.pool())
    .await;
    assert!(
        invalid_reply_scope.is_err(),
        "0038 refuses replies when the approved comment scope is zero"
    );

    issue_work_order_lease(&database, fixture.work_order_ref, 60)
        .await
        .expect("a content-detail-capable station can lease the narrow scope");
    let dispatch = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("one narrow task is claimed through the shared dispatcher");
    match dispatch {
        DispatchDecision::Dispatch {
            task_spec,
            page_session_plan,
            ..
        } => {
            assert_eq!(
                task_spec["capabilitiesRequested"],
                serde_json::json!(["content_detail"])
            );
            let plan = page_session_plan.expect("detail work carries its bounded same-page plan");
            assert_eq!(plan["lanes"], serde_json::json!(["content_detail"]));
            assert_eq!(plan["commentLimit"], 0);
            assert_eq!(plan["replyExpandLimit"], 0);
        }
        other => panic!("detail-only scope must dispatch detail first; got {other:?}"),
    }
}

/// 过了保质期的工单不再排队，也不再被当成「等待中」。
///
/// 一张昨天的工单回答的是昨天的问题：人可以再点一次，巡检下一轮会再来。只要它还在队列
/// 里，它就一直是最老的那一张，一直排在最前面。
#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn a_work_order_past_its_shelf_life_leaves_the_queue_instead_of_waiting_forever() {
    let database = proof_database_for("collection_dispatch_work_order_expiry").await;
    let fixture = seed_creator_work_order(&database).await;
    sqlx::query(
        "UPDATE collection_work_order \
         SET queue_state='queued',dispatch_lane='immediate', \
             scheduled_for=scope_001_now()-interval '2 days', \
             expires_at=scope_001_now()-interval '1 day' \
         WHERE work_order_ref=$1",
    )
    .bind(fixture.work_order_ref)
    .execute(database.pool())
    .await
    .expect("the stale order is the oldest waiting one in its lane");

    let runnable = seed_second_queued_work_order(&database, &fixture).await;

    let decision = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("the poll answers without tripping over the stale order");
    let dispatched_work_order: Uuid = sqlx::query_scalar(
        "SELECT lease.work_order_ref FROM collection_work_order_lease lease \
         JOIN collection_work_order_lease_task task USING(lease_ref) WHERE task.task_id=$1",
    )
    .bind(task_id(&decision))
    .fetch_one(database.pool())
    .await
    .expect("a dispatched task belongs to exactly one work order");
    assert_eq!(dispatched_work_order, runnable);

    let stale_state: String =
        sqlx::query_scalar("SELECT queue_state FROM collection_work_order WHERE work_order_ref=$1")
            .bind(fixture.work_order_ref)
            .fetch_one(database.pool())
            .await
            .expect("the stale order stays readable");
    assert_eq!(
        stale_state, "cancelled",
        "只把它从候选里滤掉是不够的：它会继续以「等待」的样子留在队列里说假话"
    );
}

/// 保质期只对排队中的工单成立。正在跑的活不能被一次派发扫描顺手取消。
#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn a_leased_work_order_is_not_cancelled_by_its_own_shelf_life() {
    let database = proof_database_for("collection_dispatch_leased_shelf_life").await;
    let fixture = seed_creator_work_order(&database).await;
    sqlx::query(
        "UPDATE collection_work_order \
         SET queue_state='queued',dispatch_lane='immediate', \
             scheduled_for=scope_001_now()-interval '1 hour' \
         WHERE work_order_ref=$1",
    )
    .bind(fixture.work_order_ref)
    .execute(database.pool())
    .await
    .expect("the order is claimable");
    decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("the station claims it");
    // 它现在真的在跑；此刻让保质期到期。
    sqlx::query(
        "UPDATE collection_work_order SET expires_at=scope_001_now()-interval '1 second' \
         WHERE work_order_ref=$1",
    )
    .bind(fixture.work_order_ref)
    .execute(database.pool())
    .await
    .expect("proof advances only the isolated shelf-life clock");

    decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("the next poll runs the expiry sweep");
    let state: String =
        sqlx::query_scalar("SELECT queue_state FROM collection_work_order WHERE work_order_ref=$1")
            .bind(fixture.work_order_ref)
            .fetch_one(database.pool())
            .await
            .expect("the leased order stays readable");
    assert_eq!(
        state, "leased",
        "正在执行的工单不能因为保质期到了就被取消——它的租约结束后才轮到这条规则"
    );
}

/// 授权失效的工单不得挡住排在它后面的活。
///
/// 2026-09-08 实测的停摆就长这样：一张 09-05 授权被撤销的建档工单排在 immediate 队首，
/// 派发扫描每一轮都停在它身上直接返回，当天两张人工观察工单一次都没有被看过。队列不空，
/// 工位在线，所有闸门都是开的——却什么都不发生。
#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn a_lapsed_authorization_at_the_queue_head_does_not_hide_runnable_work_behind_it() {
    let database = proof_database_for("collection_dispatch_lapsed_authorization").await;
    let fixture = seed_creator_work_order(&database).await;
    sqlx::query(
        "UPDATE collection_work_order \
         SET queue_state='queued',dispatch_lane='immediate', \
             scheduled_for=scope_001_now()-interval '2 hours' \
         WHERE work_order_ref=$1",
    )
    .bind(fixture.work_order_ref)
    .execute(database.pool())
    .await
    .expect("the poisoned order is the oldest waiting one in its lane");
    sqlx::query(
        "UPDATE collection_acquisition_authorization \
         SET revoked_at=scope_001_now(),revoke_reason='proof: replaced by a wider grant' \
         WHERE authorization_ref=( \
             SELECT decision.authorization_ref FROM collection_admission_decision decision \
             JOIN collection_work_order work_order \
               ON work_order.decision_ref=decision.decision_ref \
             WHERE work_order.work_order_ref=$1)",
    )
    .bind(fixture.work_order_ref)
    .execute(database.pool())
    .await
    .expect("a person withdrew the authorization this order was admitted under");

    let runnable = seed_second_queued_work_order(&database, &fixture).await;

    let decision = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("the same poll keeps looking past an order it can never run");
    let dispatched_work_order: Uuid = sqlx::query_scalar(
        "SELECT lease.work_order_ref FROM collection_work_order_lease lease \
         JOIN collection_work_order_lease_task task USING(lease_ref) WHERE task.task_id=$1",
    )
    .bind(task_id(&decision))
    .fetch_one(database.pool())
    .await
    .expect("a dispatched task belongs to exactly one work order");
    assert_eq!(
        dispatched_work_order, runnable,
        "the runnable order behind the lapsed one is what gets dispatched"
    );

    let poisoned_state: String =
        sqlx::query_scalar("SELECT queue_state FROM collection_work_order WHERE work_order_ref=$1")
            .bind(fixture.work_order_ref)
            .fetch_one(database.pool())
            .await
            .expect("the poisoned order stays readable");
    assert_eq!(
        poisoned_state, "cancelled",
        "an order whose authorization is gone is finished, not queued for a retry that can never succeed"
    );
}

/// 一条材料没有可用的执行地址时，只停它自己：不把同批其他作品拖回去重排，也不靠反复重建
/// 租约假装在推进。
///
/// 修前（`1ef5830c`）：pending 分支取到的第一条任务若没有签名地址，就走
/// `record_recoverable_dispatch_failure_in_transaction(…, "execution_locator_unavailable")`——
/// 释放的是**整张租约**、重排的是**整张工单**。于是同一张工单里另一篇地址完好的作品永远轮
/// 不到；而每重排一次就新建一张租约、一组运行任务和一条失败事件。共享库里那 3 张工单、
/// 35 条 `execution_locator_unavailable` 正是这条路径连转三小时的产物。
#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn a_missing_locator_stops_only_its_own_material_without_requeueing_forever() {
    let database = proof_database_for("collection_dispatch_missing_locator_stop").await;
    let fixture = seed_creator_work_order(&database).await;
    // 同一张工单上的两条材料：第一条的平台记录里没有签名地址，第二条是好的。
    for (ordinal, content_external_id, url) in [
        (
            1,
            "locator-less-note",
            "https://www.xiaohongshu.com/explore/locator-less-note",
        ),
        (
            2,
            "executable-note",
            "https://www.xiaohongshu.com/explore/executable-note?xsec_token=SIGNED_FIXTURE&xsec_source=pc_user",
        ),
    ] {
        submit_profile_discovery(&database, content_external_id, url).await;
        let content_public_ref: Uuid = sqlx::query_scalar(
            "SELECT public_ref FROM linggan_material_content \
             WHERE platform='xhs' AND content_external_id=$1",
        )
        .bind(content_external_id)
        .fetch_one(database.pool())
        .await
        .expect("accepted discovery creates the stable material identity");
        sqlx::query(
            "INSERT INTO collection_work_order_material_target \
                 (work_order_ref,content_public_ref,ordinal,comment_limit,reply_expand_limit,acquire_media) \
             VALUES ($1,$2,$3,30,2,true)",
        )
        .bind(fixture.work_order_ref)
        .bind(content_public_ref)
        .bind(ordinal)
        .execute(database.pool())
        .await
        .expect("the approved material scope is frozen on the work order");
    }
    issue_work_order_lease(&database, fixture.work_order_ref, 60)
        .await
        .expect("deepening lease is issued");

    let decision = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("一次派发请求仍要给出答案");
    assert_eq!(
        task_from_dispatch(&decision).raw()["target"]["contentExternalId"],
        "executable-note",
        "缺地址的成员停它自己，同一批里地址完好的作品照常执行"
    );

    let stopped: Vec<(String, String)> = sqlx::query_as(
        "SELECT task.execution_state,runtime.task_spec #>> '{capabilitiesRequested,0}' \
         FROM collection_work_order_lease_task task \
         JOIN linggan_runtime_task runtime USING(task_id) \
         WHERE runtime.task_spec #>> '{target,contentExternalId}'='locator-less-note' \
         ORDER BY 2",
    )
    .fetch_all(database.pool())
    .await
    .expect("stopped lanes stay readable");
    assert_eq!(stopped.len(), 4, "这一篇的四条通道都要有明确去向");
    assert!(
        stopped
            .iter()
            .all(|(state, _)| state == "input_blocked"),
        "缺输入是「从未执行」，既不是读过没读成的 blocked，也不是 completed：{stopped:?}"
    );
    let fabricated: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM linggan_runtime_attempt attempt \
                   JOIN linggan_runtime_task runtime USING(task_id) \
                 WHERE runtime.task_spec #>> '{target,contentExternalId}'='locator-less-note') \
              + (SELECT count(*) FROM linggan_runtime_capture_package package \
                   JOIN linggan_runtime_task runtime USING(task_id) \
                 WHERE runtime.task_spec #>> '{target,contentExternalId}'='locator-less-note')",
    )
    .fetch_one(database.pool())
    .await
    .expect("the fabricated-fact check is readable");
    assert_eq!(fabricated, 0, "停止不能伪造 Attempt 或 Package");
    let work_order_state: String =
        sqlx::query_scalar("SELECT queue_state FROM collection_work_order WHERE work_order_ref=$1")
            .bind(fixture.work_order_ref)
            .fetch_one(database.pool())
            .await
            .expect("the work order state stays readable");
    assert_ne!(
        work_order_state, "queued",
        "缺输入不是可重试的失败，不能把整张工单退回队列等下一轮"
    );
    assert_ne!(
        work_order_state, "completed",
        "同一批里还有没执行完的有效成员，不能提前总完成"
    );

    // 停止之后连续 100 轮调度：不得新建租约，不得重复记停止事实。
    let leases_at_stop: i64 =
        sqlx::query_scalar("SELECT count(*) FROM collection_work_order_lease")
            .fetch_one(database.pool())
            .await
            .expect("lease count is readable");
    let stop_events_at_stop: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order_lease_task_dispatch_failure \
         WHERE failure_code='execution_input_missing'",
    )
    .fetch_one(database.pool())
    .await
    .expect("stop events are readable");
    assert!(
        stop_events_at_stop > 0 && stop_events_at_stop <= 4,
        "停止事实按归属的通道各记一次，不按调度轮数增长：{stop_events_at_stop}"
    );
    let cooling_requeues: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order_lease_task_dispatch_failure \
         WHERE failure_code='execution_locator_unavailable'",
    )
    .fetch_one(database.pool())
    .await
    .expect("the cooldown-retry ledger is readable");
    assert_eq!(
        cooling_requeues, 0,
        "缺输入不能被记成「进冷却、过一会儿再来」——那条码属于页面读取失败，不属于从未开始的成员"
    );
    for round in 0..100 {
        // 退避到期的等价操作。修前正是这条路径让同一张工单每一轮都被重新领取。
        sqlx::query(
            "UPDATE collection_work_order SET retry_not_before_at=scope_001_now() \
             WHERE work_order_ref=$1 AND queue_state='queued'",
        )
        .bind(fixture.work_order_ref)
        .execute(database.pool())
        .await
        .expect("the requeued order is made claimable again");
        let _ = decide_dispatch(
            &database,
            &fixture.install_key,
            &fixture.installation_credential,
        )
        .await
        .unwrap_or_else(|error| panic!("第 {round} 轮调度仍要给出答案：{error:?}"));
    }
    let leases_after: i64 = sqlx::query_scalar("SELECT count(*) FROM collection_work_order_lease")
        .fetch_one(database.pool())
        .await
        .expect("lease count is readable");
    assert_eq!(
        leases_after, leases_at_stop,
        "停止之后 100 轮调度不得再新建租约"
    );
    let stop_events_after: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order_lease_task_dispatch_failure \
         WHERE failure_code='execution_input_missing'",
    )
    .fetch_one(database.pool())
    .await
    .expect("stop events are readable");
    assert_eq!(
        stop_events_after, stop_events_at_stop,
        "停止事实只记一次，不随调度轮数增长"
    );
    let gap_remains: bool = sqlx::query_scalar(
        "SELECT NOT EXISTS ( \
             SELECT 1 FROM linggan_material_content content \
             JOIN linggan_material_content_detail detail ON detail.content_public_ref=content.public_ref \
             WHERE content.platform='xhs' AND content.content_external_id='locator-less-note')",
    )
    .fetch_one(database.pool())
    .await
    .expect("the gap check is readable");
    assert!(gap_remains, "停下不等于取到详情，缺口要留着");
}

/// 一张工单上的成员**全部**没有可执行地址时：这一轮如实终结，不记成完成、也不退回队列。
///
/// 与上一条的分工：上一条是「一个坏、其余好」——坏的停下、好的照常执行，工单留着；这一条
/// 是「一个都没有可执行入口」。两者处置相反（前者继续、后者终结），所以要各钉一次。
///
/// 「没有入口」在这里要分开说清两件事：
///
///   * 租约是**因为成员缺输入**结束的（`release_reason='input_blocked'`）：它不是交付了一部分
///     （`partial`），也不是页面读失败进了冷却（`execution_locator_unavailable`）。
///   * 工单**取消**，不是完成。一篇都没取回却记成 `completed`，就是把「什么都没发生」说成
///     「这一单做完了」；记成 `queued` 更糟——每一轮调度都会重新领取、重新展开、再空一次。
///     「哪一篇缺的是哪个输入」留在执行资格台账上，新输入到了由准入另开一张后继工单。
#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn a_lease_whose_members_all_lack_execution_input_ends_as_a_stop_not_a_completion() {
    let database = proof_database_for("collection_dispatch_all_members_input_blocked").await;
    let fixture = seed_creator_work_order(&database).await;
    let content_external_id = "no-locator-note";
    submit_profile_discovery(
        &database,
        content_external_id,
        "https://www.xiaohongshu.com/explore/no-locator-note",
    )
    .await;
    let content_public_ref: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content \
         WHERE platform='xhs' AND content_external_id=$1",
    )
    .bind(content_external_id)
    .fetch_one(database.pool())
    .await
    .expect("accepted discovery creates the stable material identity");
    sqlx::query(
        "INSERT INTO collection_work_order_material_target \
             (work_order_ref,content_public_ref,ordinal,comment_limit,reply_expand_limit,acquire_media) \
         VALUES ($1,$2,1,30,2,true)",
    )
    .bind(fixture.work_order_ref)
    .bind(content_public_ref)
    .execute(database.pool())
    .await
    .expect("the approved material scope is frozen on the work order");
    let lease = issue_work_order_lease(&database, fixture.work_order_ref, 60)
        .await
        .expect("the lease is issued");
    assert!(
        lease_is_live(&database, lease.lease_ref).await,
        "前置：租约先要真的发出来，否则后面的释放断言是空转"
    );

    let decision = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("一次派发请求仍要给出答案");
    assert!(
        matches!(decision, DispatchDecision::NothingWaiting),
        "没有可执行入口的成员不该派给工位，实际是 {decision:?}"
    );

    let (release_reason, released): (Option<String>, bool) = sqlx::query_as(
        "SELECT release_reason,(released_at IS NOT NULL) \
         FROM collection_work_order_lease WHERE lease_ref=$1",
    )
    .bind(lease.lease_ref)
    .fetch_one(database.pool())
    .await
    .expect("the lease stays readable");
    assert!(
        released,
        "成员全停之后不能再占着执行权——到期才归还等于让工位空等一轮"
    );
    assert_eq!(
        release_reason.as_deref(),
        Some("input_blocked"),
        "这张租约是因为成员缺输入停的：不是交付了一部分（partial），也不是页面读失败进冷却\
         （execution_locator_unavailable）"
    );

    let work_order_state: String =
        sqlx::query_scalar("SELECT queue_state FROM collection_work_order WHERE work_order_ref=$1")
            .bind(fixture.work_order_ref)
            .fetch_one(database.pool())
            .await
            .expect("the work order state stays readable");
    assert_eq!(
        work_order_state, "cancelled",
        "一篇都没取回却记成 completed 就是制造成功；退回 queued 则每一轮都会被重新领取、\
         重新展开、再空一次"
    );

    let fabricated: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM linggan_runtime_attempt attempt \
                   JOIN linggan_runtime_task runtime USING(task_id) \
                 WHERE runtime.task_spec #>> '{target,contentExternalId}'=$1) \
              + (SELECT count(*) FROM linggan_runtime_capture_package package \
                   JOIN linggan_runtime_task runtime USING(task_id) \
                 WHERE runtime.task_spec #>> '{target,contentExternalId}'=$1)",
    )
    .bind(content_external_id)
    .fetch_one(database.pool())
    .await
    .expect("the fabricated-fact check is readable");
    assert_eq!(fabricated, 0, "停止不能伪造 Attempt 或 Package");

    // 停就停住：再调度 100 轮，不得新建租约、不得新增停止事实、也不得把工单退回队列。
    let leases_at_stop: i64 = sqlx::query_scalar("SELECT count(*) FROM collection_work_order_lease")
        .fetch_one(database.pool())
        .await
        .expect("lease count is readable");
    let stop_events_at_stop: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order_lease_task_dispatch_failure \
         WHERE failure_code='execution_input_missing'",
    )
    .fetch_one(database.pool())
    .await
    .expect("stop events are readable");
    assert_eq!(
        stop_events_at_stop, 1,
        "四条通道共用同一次页面打开，缺输入这件事只记一次；重复记账会让运维看板上的\
         「停了多少」随通道数虚增"
    );
    for round in 0..100 {
        sqlx::query(
            "UPDATE collection_work_order SET retry_not_before_at=scope_001_now() \
             WHERE work_order_ref=$1 AND queue_state='queued'",
        )
        .bind(fixture.work_order_ref)
        .execute(database.pool())
        .await
        .expect("a requeued order would be made claimable again");
        let _ = decide_dispatch(
            &database,
            &fixture.install_key,
            &fixture.installation_credential,
        )
        .await
        .unwrap_or_else(|error| panic!("第 {round} 轮调度仍要给出答案：{error:?}"));
    }
    let leases_after: i64 = sqlx::query_scalar("SELECT count(*) FROM collection_work_order_lease")
        .fetch_one(database.pool())
        .await
        .expect("lease count is readable");
    assert_eq!(leases_after, leases_at_stop, "停止之后 100 轮调度不得再新建租约");
    let stop_events_after: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order_lease_task_dispatch_failure \
         WHERE failure_code='execution_input_missing'",
    )
    .fetch_one(database.pool())
    .await
    .expect("stop events are readable");
    assert_eq!(stop_events_after, stop_events_at_stop, "停止事实不随调度轮数增长");
    let state_after: String =
        sqlx::query_scalar("SELECT queue_state FROM collection_work_order WHERE work_order_ref=$1")
            .bind(fixture.work_order_ref)
            .fetch_one(database.pool())
            .await
            .expect("the work order state stays readable");
    assert_eq!(state_after, "cancelled", "100 轮调度之后仍然是同一个结论");
}

/// 重放一条**已经交出去过**的任务时地址已不可用：保住现场——不重新导航、不释放租约、不销毁
/// 已有的 Attempt 与包。
///
/// 与上面两条「缺输入就停止」的分工：那两条说的是**还没开始**的成员，没有 Attempt、没有包，
/// 停下不损失任何东西。这一条说的是已经派发给浏览器的任务：派发应答可能在浏览器真正开始之后
/// 才丢，插件再问一次时任务仍是 `in_progress`。此时如果套用同一套停止逻辑，等于把一次可能正在
/// 进行的执行说成「从未开始」，连同它已经铸出的 Attempt 和随后的投递路径一起作废。
///
/// 所以这里只回一句「这一轮不能给地址」，其余什么都不动；已经开始的执行照常投递，投递落地后
/// 尚未开始的其它通道才按缺输入停下——同一条租约里两种事实各按各的办。
///
/// 「地址已不可用」用真实的状态转移制造：会话先经公开授权路径建立（它写下地址指纹），随后执行
/// `requeue_failed_dispatch` 判定地址失效时写的那一条更新（`0095`）。不直接走那条判定本身，是
/// 因为它会连带把**报告它的任务**终态化——这正是为什么这个组合在生产里只会来自**另一张租约**
/// 对同一条地址的判定；判定本身的形状由
/// `rejected_signed_detail_url_is_never_reopened_but_a_fresh_discovery_url_can_run` 证明。
#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn an_in_progress_replay_with_a_dead_locator_keeps_the_attempt_and_never_renavigates() {
    let database = proof_database_for("collection_dispatch_in_progress_dead_locator").await;
    let fixture = seed_creator_work_order(&database).await;
    let content_external_id = "held-note";
    let held_url = format!(
        "https://www.xiaohongshu.com/explore/{content_external_id}?xsec_token=HELD_FIXTURE&xsec_source=pc_user"
    );
    submit_profile_discovery(&database, content_external_id, &held_url).await;
    let content_public_ref: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content \
         WHERE platform='xhs' AND content_external_id=$1",
    )
    .bind(content_external_id)
    .fetch_one(database.pool())
    .await
    .expect("accepted discovery creates the stable material identity");
    sqlx::query(
        "INSERT INTO collection_work_order_material_target \
             (work_order_ref,content_public_ref,ordinal,comment_limit,reply_expand_limit,acquire_media) \
         VALUES ($1,$2,1,30,2,true)",
    )
    .bind(fixture.work_order_ref)
    .bind(content_public_ref)
    .execute(database.pool())
    .await
    .expect("the approved material scope is frozen on the work order");
    let lease = issue_work_order_lease(&database, fixture.work_order_ref, 60)
        .await
        .expect("the detail work is leased");

    let decided = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("the first poll hands the detail lane to the browser");
    assert_eq!(capability(&decided), "content_detail");
    let task = task_from_dispatch(&decided);
    assert_task_state(&database, task.task_id(), "in_progress").await;

    // 浏览器已经真正开始：服务端按任务身份铸出 Attempt。这一步之后，「从未执行」的说法不成立。
    let started_attempt = attempt(task.task_id(), fixture.producer_instance_id);
    assert!(matches!(
        start_producer_attempt(&database, &started_attempt).await,
        Ok(RuntimeAttemptOutcome::Started { .. })
    ));

    // 会话是真实的（公开授权路径写下这条地址的指纹），随后平台判定它已失效。
    assert!(matches!(
        grant_detail_page_session(
            &database,
            &fixture.install_key,
            &fixture.installation_credential,
            task.task_id(),
            Uuid::new_v4(),
            &held_url,
        )
        .await,
        Ok(DetailPageSessionGrant::Authorized { .. })
    ));
    let stopped = sqlx::query(
        "UPDATE collection_detail_page_session \
         SET state='stopped',stop_reason='detail_page_url_invalid', \
             finished_at=scope_001_now(),last_progress_at=scope_001_now() \
         WHERE initial_lease_ref=$1 AND execution_source_url_sha256 IS NOT NULL \
           AND state NOT IN ('finished','stopped')",
    )
    .bind(lease.lease_ref)
    .execute(database.pool())
    .await
    .expect("the platform's dead-address fact is recordable");
    assert_eq!(stopped.rows_affected(), 1, "这条地址的失效要落在它自己的会话上");

    let scene_before = execution_scene(&database, task.task_id(), lease.lease_ref).await;

    // 应答丢了，插件再问一次。
    let replay = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("a lost-response retry still gets an answer");
    assert!(
        matches!(replay, DispatchDecision::ExecutionLocatorUnavailable { .. }),
        "地址不可用就不再交给浏览器导航：{replay:?}"
    );
    assert_eq!(
        execution_scene(&database, task.task_id(), lease.lease_ref).await,
        scene_before,
        "重放不得改动任何执行事实：任务仍在进行、租约仍有效、Attempt 与包都还在"
    );

    // 插件会一直问下去。100 轮之后仍是同一句话，且事实数量不变——不新建租约、不新建 Attempt。
    for round in 0..100 {
        let replay = decide_dispatch(
            &database,
            &fixture.install_key,
            &fixture.installation_credential,
        )
        .await
        .unwrap_or_else(|error| panic!("第 {round} 轮重放仍要给出答案：{error:?}"));
        assert!(matches!(
            replay,
            DispatchDecision::ExecutionLocatorUnavailable { .. }
        ));
    }
    assert_eq!(
        execution_scene(&database, task.task_id(), lease.lease_ref).await,
        scene_before,
        "100 轮重放之后仍是同一个现场"
    );

    // 恢复对账：浏览器侧的投递不经过这条地址，已经开始的执行照常落地。
    let submission = scheduled_submission(&task, &started_attempt, fixture.producer_instance_id);
    assert!(matches!(
        submit_producer_package(&database, &submission).await,
        Ok(RuntimeSubmissionOutcome::Acknowledged { .. })
    ));
    assert_task_state(&database, task.task_id(), "completed").await;
    let (packages_after, receipts_after) = package_and_receipt_counts(&database).await;
    assert_eq!(
        (packages_after, receipts_after),
        (scene_before.packages + 1, scene_before.receipts + 1),
        "这一次投递只新增一条包与一条回执，历史事实不动"
    );

    // 尚未开始的其它通道：同一条死地址，但它们没有可损失的执行事实，按缺输入停下。
    let next = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("the poll after delivery still gets an answer");
    assert!(
        !matches!(next, DispatchDecision::Dispatch { .. }),
        "同一条死地址不得再交给浏览器导航：{next:?}"
    );
    let lane_states: Vec<String> = sqlx::query_scalar(
        "SELECT task.execution_state FROM collection_work_order_lease_task task \
         WHERE task.lease_ref=$1 ORDER BY task.sequence_no",
    )
    .bind(lease.lease_ref)
    .fetch_all(database.pool())
    .await
    .expect("every lane outcome stays readable");
    assert_eq!(lane_states.len(), 4, "这一篇的四条通道都要有明确去向");
    assert_eq!(lane_states[0], "completed", "已交付的通道保持完成");
    assert!(
        lane_states[1..].iter().all(|state| state == "input_blocked"),
        "没开始的通道如实记为缺输入，不改写成读过没读成的 blocked：{lane_states:?}"
    );
    let attempts_after: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_runtime_attempt WHERE task_id=$1")
            .bind(task.task_id())
            .fetch_one(database.pool())
            .await
            .expect("attempt history is readable");
    assert_eq!(
        attempts_after, scene_before.attempts_for_task,
        "停下未开始的通道不得伪造 Attempt"
    );
    let scene_after = execution_scene(&database, task.task_id(), lease.lease_ref).await;
    assert_eq!(
        scene_after.lease_tasks, scene_before.lease_tasks,
        "同一条租约里的通道不因重放或停止而增删"
    );

    // 收束之后继续轮询：不得再建租约、不得再派任务——其余通道缺的是同一个输入，
    // 这个结论不会因为多问几轮而改变。
    for round in 0..100 {
        let idle = decide_dispatch(
            &database,
            &fixture.install_key,
            &fixture.installation_credential,
        )
        .await
        .unwrap_or_else(|error| panic!("第 {round} 轮空转仍要给出答案：{error:?}"));
        assert!(
            !matches!(idle, DispatchDecision::Dispatch { .. }),
            "第 {round} 轮不得再派发：{idle:?}"
        );
    }
    let scene_idle = execution_scene(&database, task.task_id(), lease.lease_ref).await;
    assert_eq!(
        (scene_idle.leases, scene_idle.lease_tasks),
        (scene_before.leases, scene_before.lease_tasks),
        "空转不得靠新建租约或任务假装在推进"
    );
    let stop_events: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order_lease_task_dispatch_failure \
         WHERE failure_code='execution_input_missing'",
    )
    .fetch_one(database.pool())
    .await
    .expect("stop events are readable");
    assert!(
        stop_events > 0 && stop_events <= 3,
        "停止事实按归属的通道各记一次，不按轮数增长：{stop_events}"
    );
}

/// 详情页读取失败的预算记在**需求范围**上，不记在工单上。
///
/// 同一张工单里「最多三次」从前是新工单就能清零的软限制：共享库里同一篇作品进过 21、13、20、
/// 18 张带匹配任务的工单，每一张都从零开始，于是同一个缺口可以永远「再试三次」。这条用例把
/// 三件事一起钉住——换新工单不清零、刷新同一个地址的签名令牌不清零、用尽之后同批里别的作品
/// 照常执行。
#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn detail_read_failure_budget_follows_the_requirement_scope_across_new_work_orders() {
    let database = proof_database_for("collection_dispatch_detail_budget_scope").await;
    let fixture = seed_creator_work_order(&database).await;
    let (target_ref, ..) = fixture_control_tuple(&database, fixture.work_order_ref).await;
    let first_url = "https://www.xiaohongshu.com/explore/budget-scope-target?xsec_token=FIRST_TOKEN&xsec_source=pc_user";
    let content_public_ref = accept_material(&database, "budget-scope-target", first_url).await;
    freeze_material(&database, fixture.work_order_ref, content_public_ref, 1).await;
    issue_work_order_lease(&database, fixture.work_order_ref, 60)
        .await
        .expect("第一张工单发出租约");

    let first = fail_dispatched_detail(
        &database,
        &fixture,
        fixture.work_order_ref,
        "budget-scope-target",
    )
    .await;
    assert_eq!(
        first,
        DispatchFailureOutcome::Requeued {
            retry_after_seconds: 60
        },
        "这个范围上的第一次失败仍是 60 秒那一档"
    );

    // 平台后来又给了这一篇**另一条**签名地址。指纹不同——输入确实变了；预算键不含指纹，
    // 所以它不该因此从头再数。
    let second_url = "https://www.xiaohongshu.com/explore/budget-scope-target?xsec_token=SECOND_TOKEN&xsec_source=pc_user";
    accept_material(&database, "budget-scope-target", second_url).await;
    let (first_fingerprint, second_fingerprint): (String, String) = sqlx::query_as(
        "SELECT encode(sha256(convert_to($1,'UTF8')),'hex'), \
                encode(sha256(convert_to($2,'UTF8')),'hex')",
    )
    .bind(first_url)
    .bind(second_url)
    .fetch_one(database.pool())
    .await
    .expect("指纹就是候选解析用的那一段 SQL");
    assert_ne!(
        first_fingerprint, second_fingerprint,
        "刷新令牌确实换了一条地址，这不是同一份输入"
    );

    let second_order = seed_followup_frozen_work_order(&database, &fixture).await;
    freeze_material(&database, second_order, content_public_ref, 1).await;
    issue_work_order_lease(&database, second_order, 60)
        .await
        .expect("第二张工单发出租约");
    let second = fail_dispatched_detail(&database, &fixture, second_order, "budget-scope-target").await;
    assert_eq!(
        second,
        DispatchFailureOutcome::Requeued {
            retry_after_seconds: 120
        },
        "换工单、换地址都不是新事实：等待要接在这个范围累计的第二次上"
    );

    let third_order = seed_followup_frozen_work_order(&database, &fixture).await;
    freeze_material(&database, third_order, content_public_ref, 1).await;
    issue_work_order_lease(&database, third_order, 60)
        .await
        .expect("第三张工单发出租约");
    let third = fail_dispatched_detail(&database, &fixture, third_order, "budget-scope-target").await;
    assert_eq!(
        third,
        DispatchFailureOutcome::Blocked,
        "第三次是这个范围的停止，不是又一次重试"
    );

    let ledger: Vec<(i32, String, Option<String>, i32, i32)> = sqlx::query_as(
        "SELECT deduplicated_failure_count,state,reason_code,retry_epoch,prior_failures_unverified \
         FROM collection_execution_input_eligibility \
         WHERE target_ref=$1 AND domain_scope='own_domain' AND object_kind='material_content' \
           AND object_ref=$2 AND capability='content_detail'",
    )
    .bind(target_ref)
    .bind(content_public_ref)
    .fetch_all(database.pool())
    .await
    .expect("资格台账可读");
    assert_eq!(
        ledger,
        vec![(
            3,
            "budget_exhausted".to_owned(),
            Some("page_read_budget_exhausted".to_owned()),
            0,
            0
        )],
        "跨工单只有一条当前行：三次都记在它上面，刷新地址没有另开一条，旧失败也没有混进来"
    );

    let blocked_lanes: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order_lease_task task \
         JOIN linggan_runtime_task runtime ON runtime.task_id=task.task_id \
         WHERE runtime.task_spec #>> '{target,contentExternalId}'='budget-scope-target' \
           AND task.execution_state='blocked'",
    )
    .fetch_one(database.pool())
    .await
    .expect("通道状态可读");
    assert_eq!(
        blocked_lanes, 4,
        "停的是这一篇的四条通道；前两张工单里已经退队的任务不跟着变成停止"
    );

    // 第四张工单里既有预算用尽的那一篇，也有一篇完好的：展开任务时摘掉用尽的那一篇，
    // 好的那一篇当场就要能派——一个缺口不该让同一批里别的作品一起等。
    let healthy_ref = accept_material(
        &database,
        "budget-scope-healthy",
        "https://www.xiaohongshu.com/explore/budget-scope-healthy?xsec_token=HEALTHY_TOKEN&xsec_source=pc_user",
    )
    .await;
    let fourth_order = seed_followup_frozen_work_order(&database, &fixture).await;
    freeze_material(&database, fourth_order, content_public_ref, 1).await;
    freeze_material(&database, fourth_order, healthy_ref, 2).await;
    issue_work_order_lease(&database, fourth_order, 60)
        .await
        .expect("预算用尽的成员不该让这张工单发不出租约");
    let next = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("同批里完好的那一篇仍要派出去");
    assert_eq!(
        task_from_dispatch(&next).raw()["target"]["contentExternalId"].as_str(),
        Some("budget-scope-healthy")
    );
}

/// 同一篇作品被两个目标各要求一次详情时，两个需求范围的资格彼此独立。
///
/// 一个目标上读三次读不成，不该让另一个目标再也拿不到这一篇；一个目标上记下的「已失效」也只
/// 约束它自己。台账的键里含目标，这两件事才对得上。
#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn the_same_content_under_two_targets_keeps_two_independent_detail_budgets() {
    let database = proof_database_for("collection_dispatch_detail_budget_two_targets").await;
    let fixture = seed_creator_work_order(&database).await;
    let (target_a, ..) = fixture_control_tuple(&database, fixture.work_order_ref).await;
    let content_public_ref = accept_material(
        &database,
        "shared-content",
        "https://www.xiaohongshu.com/explore/shared-content?xsec_token=SHARED_TOKEN&xsec_source=pc_user",
    )
    .await;
    freeze_material(&database, fixture.work_order_ref, content_public_ref, 1).await;
    issue_work_order_lease(&database, fixture.work_order_ref, 60)
        .await
        .expect("目标 A 的工单发出租约");
    for attempt in 1..=3 {
        let outcome = fail_dispatched_detail(
            &database,
            &fixture,
            fixture.work_order_ref,
            "shared-content",
        )
        .await;
        if attempt < 3 {
            assert!(matches!(
                outcome,
                DispatchFailureOutcome::Requeued { .. }
            ));
            clear_work_order_backoff(&database, fixture.work_order_ref).await;
        } else {
            assert_eq!(outcome, DispatchFailureOutcome::Blocked);
        }
    }

    let retired = retire_materials(&database, target_a, &[content_public_ref], "page_gone")
        .await
        .expect("目标 A 上的结论写进台账");
    assert_eq!(retired, 1, "目标 A 记下这一篇已失效");

    // 第二个目标要的是同一篇作品的详情——换的是需求，不是作品。
    let target_b = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_observation_target \
             (target_ref, platform, target_kind, identity_key, display_name, source, lifecycle_state) \
         VALUES ($1, 'xhs', 'creator', 'second-fixture', '第二个目标', 'manual', 'archiving')",
    )
    .bind(target_b)
    .execute(database.pool())
    .await
    .expect("第二个目标已建档");
    let order_b = seed_followup_frozen_work_order_for_target(&database, &fixture, target_b).await;
    freeze_material(&database, order_b, content_public_ref, 1).await;
    issue_work_order_lease(&database, order_b, 60)
        .await
        .expect("同一台工位、同一篇作品，换一个需求范围照常发租约");
    let decision = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("目标 B 的工单可以派");
    assert_eq!(
        task_from_dispatch(&decision).raw()["target"]["contentExternalId"].as_str(),
        Some("shared-content"),
        "别的目标上读不成、还被记了失效，都不影响这个目标拿到这一篇"
    );
    let outcome = requeue_failed_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
        task_id(&decision),
        Uuid::new_v4(),
        DispatchFailureCode::PageReadFailed,
    )
    .await
    .expect("目标 B 的失败上报被接纳");
    assert_eq!(
        outcome,
        DispatchFailureOutcome::Requeued {
            retry_after_seconds: 60
        },
        "目标 B 自己才失败第一次：等待从它自己的范围起算"
    );

    let rows: Vec<(Uuid, i32, String)> = sqlx::query_as(
        "SELECT target_ref,deduplicated_failure_count,state \
         FROM collection_execution_input_eligibility \
         WHERE capability='content_detail' AND object_ref=$1",
    )
    .bind(content_public_ref)
    .fetch_all(database.pool())
    .await
    .expect("两个范围各有一条自己的资格行");
    let mut actual = rows;
    let mut expected = vec![
        (target_a, 3, "budget_exhausted".to_owned()),
        (target_b, 1, "eligible".to_owned()),
    ];
    actual.sort_by_key(|row| row.0);
    expected.sort_by_key(|row| row.0);
    assert_eq!(
        actual, expected,
        "键里含目标：两个需求各自的次数与状态互不覆盖"
    );
}

/// 本表建立之前的老工单：失败如实记下，但不算进预算。
///
/// 「试过」不等于「试的是同一份输入」。凭一批无法对齐输入的历史失败去停止一篇作品，等于按
/// content ID 把它封掉——所以旧失败只进待核实那一列：既不清零、也不触发停止，更不允许它们
/// 把一篇历史上常失败的作品在新规则下直接停掉。
#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn legacy_unfrozen_orders_record_unverified_prior_failures_without_spending_the_budget() {
    let database = proof_database_for("collection_dispatch_legacy_unfrozen_budget").await;
    let fixture = seed_creator_work_order(&database).await;
    let (target_ref, ..) = fixture_control_tuple(&database, fixture.work_order_ref).await;
    let content_public_ref = accept_material(
        &database,
        "legacy-content",
        "https://www.xiaohongshu.com/explore/legacy-content?xsec_token=LEGACY_TOKEN&xsec_source=pc_user",
    )
    .await;
    freeze_material(&database, fixture.work_order_ref, content_public_ref, 1).await;
    set_work_order_execution_input_frozen(&database, fixture.work_order_ref, false).await;
    issue_work_order_lease(&database, fixture.work_order_ref, 60)
        .await
        .expect("本表建立之前的老工单照常发租约");

    for (attempt, expected_seconds) in [(1_i32, 60_u32), (2, 120)] {
        let outcome = fail_dispatched_detail(
            &database,
            &fixture,
            fixture.work_order_ref,
            "legacy-content",
        )
        .await;
        assert_eq!(
            outcome,
            DispatchFailureOutcome::Requeued {
                retry_after_seconds: expected_seconds
            },
            "老工单里仍按已上线的「同一张工单最多三次」收尾（第 {attempt} 次）"
        );
        clear_work_order_backoff(&database, fixture.work_order_ref).await;
    }
    let third = fail_dispatched_detail(
        &database,
        &fixture,
        fixture.work_order_ref,
        "legacy-content",
    )
    .await;
    assert_eq!(
        third,
        DispatchFailureOutcome::Blocked,
        "已上线的同一工单三次仍然有效，新规则只收紧不放松"
    );

    let ledger: Vec<(i32, i32, String, Option<String>, String)> = sqlx::query_as(
        "SELECT deduplicated_failure_count,prior_failures_unverified,state,reason_code, \
                input_source_status \
         FROM collection_execution_input_eligibility \
         WHERE target_ref=$1 AND domain_scope='own_domain' AND object_kind='material_content' \
           AND object_ref=$2 AND capability='content_detail'",
    )
    .bind(target_ref)
    .bind(content_public_ref)
    .fetch_all(database.pool())
    .await
    .expect("资格台账可读");
    assert_eq!(
        ledger,
        vec![(
            0,
            3,
            "eligible".to_owned(),
            None,
            "legacy_input_unfrozen".to_owned()
        )],
        "三次旧失败一次都不进预算：次数记在待核实那一列，台账也不谎称这份输入当初冻过"
    );

    let fresh_order = seed_followup_frozen_work_order(&database, &fixture).await;
    freeze_material(&database, fresh_order, content_public_ref, 1).await;
    issue_work_order_lease(&database, fresh_order, 60)
        .await
        .expect("新工单发出租约");
    let outcome = fail_dispatched_detail(&database, &fixture, fresh_order, "legacy-content").await;
    assert_eq!(
        outcome,
        DispatchFailureOutcome::Requeued {
            retry_after_seconds: 60
        },
        "旧失败不进预算，也就不会把新工单直接推到停止"
    );
    let after: (i32, i32) = sqlx::query_as(
        "SELECT deduplicated_failure_count,prior_failures_unverified \
         FROM collection_execution_input_eligibility \
         WHERE target_ref=$1 AND object_ref=$2 AND capability='content_detail'",
    )
    .bind(target_ref)
    .bind(content_public_ref)
    .fetch_one(database.pool())
    .await
    .expect("仍然只有一条当前行");
    assert_eq!(
        after,
        (1, 3),
        "新工单的这次失败进了预算（1），旧失败仍留在待核实（3）"
    );
}

/// 已经接纳了详情的那一篇，后来又报一次页面读失败：不改写已经拿到的材料事实，也不动预算。
///
/// 预算回答的是「还要不要再试」，而这里已经没有要补的东西了。把这次失败也算进停止，等于让
/// 一次迟到的浏览器故障把已经拿到的正文作废。
#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn a_late_detail_read_failure_never_spends_the_budget_of_an_already_accepted_material() {
    let database = proof_database_for("collection_dispatch_late_detail_failure").await;
    let fixture = seed_creator_work_order(&database).await;
    let (target_ref, ..) = fixture_control_tuple(&database, fixture.work_order_ref).await;
    let content_public_ref = accept_material(
        &database,
        "late-accept-content",
        "https://www.xiaohongshu.com/explore/late-accept-content?xsec_token=LATE_TOKEN&xsec_source=pc_user",
    )
    .await;
    freeze_material(&database, fixture.work_order_ref, content_public_ref, 1).await;
    issue_work_order_lease(&database, fixture.work_order_ref, 60)
        .await
        .expect("这一篇的详情通道发出租约");
    let decision = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("这一篇的正文通道可以派");
    assert_eq!(capability(&decision), "content_detail");

    // 正文已经由一次正常交付接纳（夹具走的是与生产同一条 Package 接纳路径）。
    material_fixture::submit_package(
        &database,
        "content_detail",
        serde_json::json!({"contentExternalId":"late-accept-content"}),
        serde_json::json!({
            "kind":"content_detail",
            "sourceObject":{"platform":"xhs","type":"content","externalId":"late-accept-content"},
            "payload":{
                "title":"已经拿到的正文",
                "authorId":"creator-fixture",
                "publishedAt":1785542400000_i64,
                "publishedAtText":"1785542400",
                "publishedAtSourceField":"publishTime",
                "publishedAtSourceKind":"platform_epoch",
                "publishedAtPrecision":"second",
                "publishedAtParserVersion":"xhs-detail-time-v2"
            }
        }),
    )
    .await;

    let failure_ref = Uuid::new_v4();
    let outcome = requeue_failed_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
        task_id(&decision),
        failure_ref,
        DispatchFailureCode::PageReadFailed,
    )
    .await
    .expect("迟到的失败上报仍会被如实处理");
    assert_eq!(
        outcome,
        DispatchFailureOutcome::Requeued {
            retry_after_seconds: 60
        },
        "材料已经接纳：这次失败不进预算，也就不该触发任何停止"
    );

    let details: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_material_content_detail WHERE content_public_ref=$1",
    )
    .bind(content_public_ref)
    .fetch_one(database.pool())
    .await
    .expect("已接纳的详情可读");
    assert_eq!(details, 1, "已经拿到的正文不得因为一次迟到失败而消失");
    let ledger_rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_execution_input_eligibility \
         WHERE target_ref=$1 AND object_ref=$2 AND capability='content_detail'",
    )
    .bind(target_ref)
    .bind(content_public_ref)
    .fetch_one(database.pool())
    .await
    .expect("资格台账可读");
    assert_eq!(
        ledger_rows, 0,
        "没有要补的东西，就不该为它开一条预算行"
    );
    let stopped_lanes: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order_lease_task task \
         JOIN linggan_runtime_task runtime ON runtime.task_id=task.task_id \
         WHERE runtime.task_spec #>> '{target,contentExternalId}'='late-accept-content' \
           AND task.execution_state IN ('blocked','unavailable')",
    )
    .fetch_one(database.pool())
    .await
    .expect("通道状态可读");
    assert_eq!(stopped_lanes, 0, "这一篇的四条通道一条也不停");
    let failure_rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order_lease_task_dispatch_failure \
         WHERE failure_ref=$1 AND failure_disposition='requeued'",
    )
    .bind(failure_ref)
    .fetch_one(database.pool())
    .await
    .expect("失败事实可读");
    assert_eq!(failure_rows, 1, "这次失败本身仍然如实留痕");
}

/// 同一个失败被上报两次（插件重试，或两条连接同时到）：只算一次，只留一条当前行。
///
/// 上报路径是「先提交、再确认」，确认丢了这个动作一定会被重做；重做不该让预算往前走一格，
/// 也不该在台账上留下第二条并行的资格。两个调用先抢同一行安装记录，于是它们串行——一个真的
/// 记账，另一个读到那条已经落地的记录、如实回放同一个结论。
#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn duplicate_detail_failure_reports_count_once_and_keep_one_current_row() {
    let database = proof_database_for("collection_dispatch_detail_failure_dedupe").await;
    let fixture = seed_creator_work_order(&database).await;
    let (target_ref, ..) = fixture_control_tuple(&database, fixture.work_order_ref).await;
    let content_public_ref = accept_material(
        &database,
        "dedupe-content",
        "https://www.xiaohongshu.com/explore/dedupe-content?xsec_token=DEDUPE_TOKEN&xsec_source=pc_user",
    )
    .await;
    freeze_material(&database, fixture.work_order_ref, content_public_ref, 1).await;
    issue_work_order_lease(&database, fixture.work_order_ref, 60)
        .await
        .expect("这一篇的详情通道发出租约");
    let decision = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("这一篇的正文通道可以派");
    let task = task_id(&decision);
    let failure_ref = Uuid::new_v4();
    let (left, right) = tokio::join!(
        requeue_failed_dispatch(
            &database,
            &fixture.install_key,
            &fixture.installation_credential,
            task,
            failure_ref,
            DispatchFailureCode::PageReadFailed,
        ),
        requeue_failed_dispatch(
            &database,
            &fixture.install_key,
            &fixture.installation_credential,
            task,
            failure_ref,
            DispatchFailureCode::PageReadFailed,
        ),
    );
    let mut outcomes = vec![
        left.expect("并发的第一条上报被接纳"),
        right.expect("并发的第二条上报被接纳"),
    ];
    outcomes.sort_by_key(|outcome| match outcome {
        DispatchFailureOutcome::Requeued { .. } => 0_u8,
        _ => 1,
    });
    assert_eq!(
        outcomes,
        vec![
            DispatchFailureOutcome::Requeued {
                retry_after_seconds: 60
            },
            DispatchFailureOutcome::Replay {
                retry_after_seconds: 60
            },
        ],
        "一次记账、一次如实回放；回放说的还是同一条事实，不是第二次失败"
    );
    let failure_rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order_lease_task_dispatch_failure \
         WHERE failure_ref=$1",
    )
    .bind(failure_ref)
    .fetch_one(database.pool())
    .await
    .expect("失败事实可读");
    assert_eq!(failure_rows, 1, "同一个 failure_ref 只留一条事实");
    let ledger: Vec<(i32, i32)> = sqlx::query_as(
        "SELECT deduplicated_failure_count,prior_failures_unverified \
         FROM collection_execution_input_eligibility \
         WHERE target_ref=$1 AND object_ref=$2 AND capability='content_detail'",
    )
    .bind(target_ref)
    .bind(content_public_ref)
    .fetch_all(database.pool())
    .await
    .expect("资格台账可读");
    assert_eq!(
        ledger,
        vec![(1, 0)],
        "重复上报不让预算走两格，也不长出第二条当前行"
    );

    clear_work_order_backoff(&database, fixture.work_order_ref).await;
    let outcome = fail_dispatched_detail(
        &database,
        &fixture,
        fixture.work_order_ref,
        "dedupe-content",
    )
    .await;
    assert_eq!(
        outcome,
        DispatchFailureOutcome::Requeued {
            retry_after_seconds: 120
        },
        "真正新的一次失败照常累计"
    );
    let count: i32 = sqlx::query_scalar(
        "SELECT deduplicated_failure_count FROM collection_execution_input_eligibility \
         WHERE target_ref=$1 AND object_ref=$2 AND capability='content_detail'",
    )
    .bind(target_ref)
    .bind(content_public_ref)
    .fetch_one(database.pool())
    .await
    .expect("资格台账可读");
    assert_eq!(count, 2, "只有新事件才让预算往前走");
}

/// 在同一个目标、同一台工位上再排一张工单——授权是有效的，排在毒工单后面。
async fn seed_second_queued_work_order(database: &Database, fixture: &Fixture) -> Uuid {
    let authorization_ref = Uuid::new_v4();
    let request_ref = Uuid::new_v4();
    let decision_ref = Uuid::new_v4();
    let work_order_ref = Uuid::new_v4();
    let (target_ref, account_ref): (Uuid, Uuid) = sqlx::query_as(
        "SELECT target_ref,account_ref FROM collection_work_order WHERE work_order_ref=$1",
    )
    .bind(fixture.work_order_ref)
    .fetch_one(database.pool())
    .await
    .expect("the fixture froze its own control tuple");
    sqlx::query(
        "INSERT INTO collection_acquisition_authorization              (authorization_ref, platform, target_kind, lane, max_targets, max_works_per_target,               allowed_task_templates,allowed_dispatch_lanes,max_work_units,purpose,granted_by,expires_at)          VALUES ($1,'xhs','creator','deep_archive',1,10,                  ARRAY['creator_archive','material_deepening'],ARRAY['immediate','batch'],10,                  'proof: the grant that replaced the withdrawn one','person',                  scope_001_now() + interval '1 day')",
    )
    .bind(authorization_ref)
    .execute(database.pool())
    .await
    .expect("the replacement authorization is seeded");
    sqlx::query(
        "INSERT INTO collection_acquisition_request              (request_ref, target_ref, lane, purpose, requested_by)          VALUES ($1,$2,'deep_archive','proof: the request behind the lapsed one','person')",
    )
    .bind(request_ref)
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("the later request is seeded");
    sqlx::query(
        "INSERT INTO collection_admission_decision              (decision_ref, request_ref, outcome, reason_code, authorization_ref,               target_ref, station_ref, installation_ref, account_ref)          VALUES ($1,$2,'admitted','lapsed_authorization_proof',$3,$4,$5,$6,$7)",
    )
    .bind(decision_ref)
    .bind(request_ref)
    .bind(authorization_ref)
    .bind(target_ref)
    .bind(fixture.station_ref)
    .bind(fixture.installation_ref)
    .bind(account_ref)
    .execute(database.pool())
    .await
    .expect("the later admission is seeded");
    sqlx::query(
        "INSERT INTO collection_work_order              (work_order_ref, decision_ref, target_ref, lane, max_works, stop_conditions,               dispatch_lane, queue_state, scheduled_for)          VALUES ($1,$2,$3,'deep_archive',10,'[\"maximum_quota\",\"time_budget\"]'::jsonb,                  'immediate','queued',scope_001_now()-interval '1 hour')",
    )
    .bind(work_order_ref)
    .bind(decision_ref)
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("the runnable order waits behind the poisoned one");
    work_order_ref
}

/// 接纳一篇作品的发现地址，并回答它稳定的材料身份。
///
/// 这一步不能省：工单执行的是**冻过的已知作品**，而一篇作品要先有一条已接纳的签名地址，
/// 准入与派发才认它有执行入口。
async fn accept_material(database: &Database, content_external_id: &str, signed_url: &str) -> Uuid {
    submit_profile_discovery(database, content_external_id, signed_url).await;
    sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content \
         WHERE platform='xhs' AND content_external_id=$1",
    )
    .bind(content_external_id)
    .fetch_one(database.pool())
    .await
    .expect("accepted discovery creates the stable material identity")
}

/// 把一篇作品冻进这张工单要执行的范围——工单执行的是冻结过的已知集合，不是「再看一眼」。
///
/// 四通道同开（正文、媒体、评论、回复）：同一次页面打开服务这四条，停止判据也在四条上一致。
async fn freeze_material(
    database: &Database,
    work_order_ref: Uuid,
    content_public_ref: Uuid,
    ordinal: i32,
) {
    sqlx::query(
        "INSERT INTO collection_work_order_material_target \
             (work_order_ref,content_public_ref,ordinal,comment_limit,reply_expand_limit,acquire_media) \
         VALUES ($1,$2,$3,30,2,true)",
    )
    .bind(work_order_ref)
    .bind(content_public_ref)
    .bind(ordinal)
    .execute(database.pool())
    .await
    .expect("the frozen material scope is recorded");
}

/// 控制元组（目标、工位、安装、账号）读自**那次准入决定**。
///
/// 不读工单自己那几列：失败退回队列时它们会被清空（`station_ref`/`installation_ref`/
/// `account_ref` 置空），决定上的那一份不会——后续工单要绑的正是「当初选定的那套控制面」。
async fn fixture_control_tuple(
    database: &Database,
    work_order_ref: Uuid,
) -> (Uuid, Uuid, Uuid, Uuid) {
    sqlx::query_as(
        "SELECT decision.target_ref,decision.station_ref,decision.installation_ref, \
                decision.account_ref \
         FROM collection_work_order work_order \
         JOIN collection_admission_decision decision USING(decision_ref) \
         WHERE work_order.work_order_ref=$1",
    )
    .bind(work_order_ref)
    .fetch_one(database.pool())
    .await
    .expect("the fixture admission decision froze the control tuple")
}

/// 同一个需求范围上的**又一张**工单：换的是工单，不是需求。
async fn seed_followup_frozen_work_order(database: &Database, fixture: &Fixture) -> Uuid {
    let (target_ref, ..) = fixture_control_tuple(database, fixture.work_order_ref).await;
    seed_followup_frozen_work_order_for_target(database, fixture, target_ref).await
}

/// 建单该有的三样都在——一份仍然有效的授权、一条请求、一次准入决定（`decision_ref` 是唯一键，
/// 一张工单一份决定），以及建单那一刻写下的执行输入冻结标记。它不带作品：要执行哪几篇由调用
/// 方按这一条用例要证明的东西自己冻进去。
async fn seed_followup_frozen_work_order_for_target(
    database: &Database,
    fixture: &Fixture,
    target_ref: Uuid,
) -> Uuid {
    let (_, station_ref, installation_ref, account_ref) =
        fixture_control_tuple(database, fixture.work_order_ref).await;
    let authorization_ref = Uuid::new_v4();
    let request_ref = Uuid::new_v4();
    let decision_ref = Uuid::new_v4();
    let work_order_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_acquisition_authorization \
             (authorization_ref, platform, target_kind, lane, max_targets, max_works_per_target, \
              allowed_task_templates,allowed_dispatch_lanes,max_work_units,purpose,granted_by,expires_at) \
         VALUES ($1,'xhs','creator','deep_archive',1,10, \
                 ARRAY['creator_archive','material_deepening'],ARRAY['immediate','batch'],10, \
                 '需求范围上的又一张工单','person',scope_001_now() + interval '1 day')",
    )
    .bind(authorization_ref)
    .execute(database.pool())
    .await
    .expect("the follow-up authorization is seeded");
    sqlx::query(
        "INSERT INTO collection_acquisition_request \
             (request_ref, target_ref, lane, purpose, requested_by) \
         VALUES ($1,$2,'deep_archive','需求范围上的又一张工单','person')",
    )
    .bind(request_ref)
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("the follow-up request is seeded");
    sqlx::query(
        "INSERT INTO collection_admission_decision \
             (decision_ref, request_ref, outcome, reason_code, authorization_ref, \
              target_ref, station_ref, installation_ref, account_ref) \
         VALUES ($1,$2,'admitted','focused_sequence_proof',$3,$4,$5,$6,$7)",
    )
    .bind(decision_ref)
    .bind(request_ref)
    .bind(authorization_ref)
    .bind(target_ref)
    .bind(station_ref)
    .bind(installation_ref)
    .bind(account_ref)
    .execute(database.pool())
    .await
    .expect("the follow-up admission is seeded");
    sqlx::query(
        "INSERT INTO collection_work_order \
             (work_order_ref, decision_ref, target_ref, lane, max_works, stop_conditions, \
              station_ref,installation_ref,account_ref,dispatch_lane,queue_state,scheduled_for, \
              execution_input_frozen_at) \
         VALUES ($1,$2,$3,'deep_archive',10,'[\"maximum_quota\",\"time_budget\"]'::jsonb, \
                 $4,$5,$6,'immediate','queued',scope_001_now()-interval '1 hour',scope_001_now())",
    )
    .bind(work_order_ref)
    .bind(decision_ref)
    .bind(target_ref)
    .bind(station_ref)
    .bind(installation_ref)
    .bind(account_ref)
    .execute(database.pool())
    .await
    .expect("the follow-up work order waits in the immediate lane");
    work_order_ref
}

/// 把一张工单标成「出生时没有冻过输入」——本表建立之前的老工单形态。
///
/// 夹具默认按今天的建单方式写这一列；要覆盖旧分支的用例必须显式退回去，否则测的是另一条路。
async fn set_work_order_execution_input_frozen(
    database: &Database,
    work_order_ref: Uuid,
    frozen: bool,
) {
    sqlx::query(
        "UPDATE collection_work_order \
         SET execution_input_frozen_at=CASE WHEN $2 THEN scope_001_now() ELSE NULL END \
         WHERE work_order_ref=$1",
    )
    .bind(work_order_ref)
    .bind(frozen)
    .execute(database.pool())
    .await
    .expect("the fixture states whether this work order froze its execution input");
}

/// 把工单的退避时间拨到过去——只有隔离的证明时钟可以这样走。
async fn clear_work_order_backoff(database: &Database, work_order_ref: Uuid) {
    sqlx::query(
        "UPDATE collection_work_order SET retry_not_before_at=scope_001_now()-interval '1 second' \
         WHERE work_order_ref=$1",
    )
    .bind(work_order_ref)
    .execute(database.pool())
    .await
    .expect("the isolated proof clock steps past the backoff");
}

/// 在一张工单上派一次 `content_detail`、再如实上报一次页面读失败。
///
/// 报的是**这一次真的派出去的那条通道**：先认工单、再认能力，免得把失败记到别的工单或别的
/// 通道上，最后把台账的结论读成别人的。
async fn fail_dispatched_detail(
    database: &Database,
    fixture: &Fixture,
    expected_work_order: Uuid,
    content_external_id: &str,
) -> DispatchFailureOutcome {
    let decision = decide_dispatch(
        database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("the isolated proof database dispatches the frozen detail");
    assert_eq!(
        work_order_of_lease(database, lease_ref(&decision)).await,
        expected_work_order,
        "这一步要报的是这张工单上的失败"
    );
    assert_eq!(capability(&decision), "content_detail");
    let task = task_from_dispatch(&decision);
    assert_eq!(
        task.raw()["target"]["contentExternalId"].as_str(),
        Some(content_external_id)
    );
    requeue_failed_dispatch(
        database,
        &fixture.install_key,
        &fixture.installation_credential,
        task_id(&decision),
        Uuid::new_v4(),
        DispatchFailureCode::PageReadFailed,
    )
    .await
    .expect("the page-read failure report is accepted")
}

fn lease_ref(decision: &DispatchDecision) -> Uuid {
    match decision {
        DispatchDecision::Dispatch { lease_ref, .. } => *lease_ref,
        other => panic!("expected dispatch, got {other:?}"),
    }
}

async fn work_order_of_lease(database: &Database, lease_ref: Uuid) -> Uuid {
    sqlx::query_scalar("SELECT work_order_ref FROM collection_work_order_lease WHERE lease_ref=$1")
        .bind(lease_ref)
        .fetch_one(database.pool())
        .await
        .expect("every lease belongs to a work order")
}

struct Fixture {
    work_order_ref: Uuid,
    station_ref: Uuid,
    installation_ref: Uuid,
    producer_instance_id: Uuid,
    install_key: String,
    installation_credential: String,
}

async fn proof_database() -> Database {
    proof_database_for("collection_dispatch_sequence").await
}

async fn proof_database_for(schema: &str) -> Database {
    let url = std::env::var("COLLECTION_DISPATCH_PROOF_DATABASE_URL")
        .expect("proof database URL is supplied");
    isolated_proof_schema(&url, schema, MIGRATIONS)
        .await
        .expect("focused migrations apply")
}

async fn seed_creator_work_order(database: &Database) -> Fixture {
    let target_ref = Uuid::new_v4();
    let authorization_ref = Uuid::new_v4();
    let request_ref = Uuid::new_v4();
    let decision_ref = Uuid::new_v4();
    let work_order_ref = Uuid::new_v4();
    let station_ref = Uuid::new_v4();
    let installation_ref = Uuid::new_v4();
    let producer_instance_id = Uuid::new_v4();
    let install_key = producer_instance_id.to_string();

    sqlx::query(
        "INSERT INTO collection_observation_target \
             (target_ref, platform, target_kind, identity_key, display_name, source, lifecycle_state) \
         VALUES ($1, 'xhs', 'creator', 'creator-fixture', '顺序派发夹具', 'manual', 'archiving')",
    )
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("target is seeded");
    sqlx::query(
        "INSERT INTO collection_acquisition_authorization \
             (authorization_ref, platform, target_kind, lane, max_targets, max_works_per_target, \
              allowed_task_templates,allowed_dispatch_lanes,max_work_units,purpose,granted_by,expires_at) \
         VALUES ($1, 'xhs', 'creator', 'deep_archive', 1, 10, \
                 ARRAY['creator_archive','material_deepening'],ARRAY['immediate','batch'],10, \
                 'focused sequence proof', 'person',scope_001_now() + interval '1 day')",
    )
    .bind(authorization_ref)
    .execute(database.pool())
    .await
    .expect("authorization is seeded");
    sqlx::query(
        "INSERT INTO collection_acquisition_request \
             (request_ref, target_ref, lane, purpose, requested_by) \
         VALUES ($1, $2, 'deep_archive', 'focused sequence proof', 'person')",
    )
    .bind(request_ref)
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("request is seeded");
    sqlx::query(
        "INSERT INTO collection_admission_decision \
             (decision_ref, request_ref, outcome, reason_code, authorization_ref) \
         VALUES ($1, $2, 'admitted', 'focused_sequence_proof', $3)",
    )
    .bind(decision_ref)
    .bind(request_ref)
    .bind(authorization_ref)
    .execute(database.pool())
    .await
    .expect("admission is seeded");
    sqlx::query(
        "INSERT INTO execution_station (station_ref, display_name, daily_work_quota) \
         VALUES ($1, 'focused sequence station', 200)",
    )
    .bind(station_ref)
    .execute(database.pool())
    .await
    .expect("station is seeded");
    sqlx::query(
        "INSERT INTO plugin_installation \
             (installation_ref, install_key, station_ref, claim_kind, claimed_at, plugin_version, capabilities) \
         VALUES ($1, $2, $3, 'person', scope_001_now(), '0.8.47', \
                 '[\"author_profile\",\"profile_discovery\",\"content_detail\",\"media_slots\",\"comments\",\"replies\"]'::jsonb)",
    )
    .bind(installation_ref)
    .bind(&install_key)
    .bind(station_ref)
    .execute(database.pool())
    .await
    .expect("installation is seeded");
    set_station_accepting(database, station_ref, true, "person")
        .await
        .expect("fixture station explicitly accepts new work");
    let credential = rotate_installation_credential(database, installation_ref)
        .await
        .expect("fixture installation receives a high-entropy credential");
    let installation_credential = credential.raw_credential.expose_once().to_owned();
    activate_installation_credential(
        database,
        installation_ref,
        credential.credential_ref,
        &installation_credential,
    )
    .await
    .expect("fixture activates the pending credential before reporting account state");
    let account = report_account_eligibility(
        database,
        installation_ref,
        &installation_credential,
        AccountEligibilityObservation::Authenticated {
            raw_platform_account_id: "xhs-account-dispatch-fixture",
        },
        Some(b"collection-dispatch-fixture-digest-key-32-plus"),
    )
    .await
    .expect("fixture account observation is accepted");
    let account_ref = account.account_ref.expect("fixture account is projected");
    bind_observation_account(database, account_ref, installation_ref, "person")
        .await
        .expect("fixture account is explicitly bound");
    sqlx::query(
        "UPDATE collection_admission_decision \
         SET target_ref=$2,station_ref=$3,installation_ref=$4,account_ref=$5 \
         WHERE decision_ref=$1",
    )
    .bind(decision_ref)
    .bind(target_ref)
    .bind(station_ref)
    .bind(installation_ref)
    .bind(account_ref)
    .execute(database.pool())
    .await
    .expect("fixture admission freezes the selected control tuple");
    // `execution_input_frozen_at` 与 `write_work_order` 一样在建单那一刻写下：夹具直接写表，
    // 少写这一列就会造出一张「本表建立之前的老工单」，走的是另一条分支（旧失败不进预算，
    // 见 `legacy_unfrozen_orders_record_unverified_prior_failures_without_spending_the_budget`）。
    // 需要旧分支的用例显式调用 `set_work_order_execution_input_frozen(…, false)`。
    sqlx::query(
        "INSERT INTO collection_work_order \
             (work_order_ref, decision_ref, target_ref, lane, max_works, stop_conditions, \
              station_ref,installation_ref,account_ref,execution_input_frozen_at) \
         VALUES ($1, $2, $3, 'deep_archive', 10, '[\"maximum_quota\",\"time_budget\"]'::jsonb, $4,$5,$6, \
                 scope_001_now())",
    )
    .bind(work_order_ref)
    .bind(decision_ref)
    .bind(target_ref)
    .bind(station_ref)
    .bind(installation_ref)
    .bind(account_ref)
    .execute(database.pool())
    .await
    .expect("work order is seeded");

    Fixture {
        work_order_ref,
        station_ref,
        installation_ref,
        producer_instance_id,
        install_key,
        installation_credential,
    }
}

/// 造一个在观察中的目标，并给它存一条自动巡查规则（固定周期、全天、每天都跑）。
///
/// 关键词的规则身份就是排序（`monitor_rule_slot_key`）：同一个词盯两个榜是两条规则，
/// 所以排哪条榜要说清；创作者没有排序可言，固定 `primary`。
///
/// 到期与否不在这里定——那一步由调用方显式做，因为「界面把周期重算到什么时候」正是
/// 上面两条用例要证明的东西，不能藏进夹具。
async fn save_patrol_rule(
    database: &Database,
    target_kind: &str,
    identity_key: &str,
    top_by_likes: Option<i32>,
) -> Uuid {
    let target_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_observation_target \
             (target_ref,platform,target_kind,identity_key,display_name,source) \
         VALUES ($1,'xhs',$2,$3,$3,'manual')",
    )
    .bind(target_ref)
    .bind(target_kind)
    .bind(identity_key)
    .execute(database.pool())
    .await
    .expect("the patrol target is seeded without any archive receipt");
    let ranking_key = (target_kind == "keyword").then(|| "comprehensive".to_owned());
    // 关键词的搜索面叫 `keyword_search`（`0046` 的 CHECK 只允许它带采样口径），
    // 创作者主页叫 `creator_patrol`。
    let surface_key = if target_kind == "keyword" {
        "keyword_search"
    } else {
        "creator_patrol"
    };
    let saved = apply_monitor_rule_command(
        database,
        &MonitorRuleCommand {
            target_ref,
            expected_revision: 0,
            idempotency_key: Uuid::new_v4(),
            kind: MonitorCommandKind::SaveRule,
            actor: MonitorCommandActor::Person,
            source: "targets_ui",
            slot_key: ranking_key.clone(),
            draft: Some(MonitorRuleDraft {
                mode: MonitorRuleMode::Fixed,
                automatic_enabled: true,
                run_on_weekdays: true,
                run_on_weekends: true,
                all_day: true,
                window_start_minute: None,
                window_end_minute: None,
                fixed_interval_seconds: Some(43_200),
                fallback_interval_seconds: 43_200,
                surface_key: surface_key.to_owned(),
                ranking_key,
                // 采样口径只属于关键词搜索面（`0046` 的 CHECK）：创作者主页没有「排序」
                // 也没有「取前 N」可言，给它编一个会让复核基于一个不存在的事实。
                scroll_rounds: (target_kind == "keyword").then_some(3),
                top_by_likes,
                published_within_days: (target_kind == "keyword").then_some(7),
                task_contract_version: "linggan.producer.task-spec.v1".to_owned(),
            }),
        },
    )
    .await
    .expect("the patrol rule is saved");
    if target_kind == "keyword" {
        assert_eq!(saved.reason_code, "baseline_not_ready");
        assert_eq!(
            saved.outcome,
            linggan_evidence::MonitorCommandOutcomeKind::Rejected
        );
        return target_ref;
    }
    assert_eq!(saved.reason_code, "rule_saved");
    target_ref
}

/// 造一张**修复上线前那一版派发**留下的便条：推进了排期也照样写「派发时刻 + 周期」。
///
/// 生产库里每一条历史派发都长这样，而 `collection_scheduler_target_decision` 是审计记录，
/// 不改写。所以「旧便条不再有资格压排期」这件事只能在读取侧做，而读取侧正是靠这条形状
/// 才被证明真的挡得住它——不造这一行，用例就只在测新写的行（新写的行根本没有便条）。
async fn seed_pre_fix_dispatch_gate(database: &Database, target_ref: Uuid) {
    let (rule_ref, rule_revision_ref, interval_seconds): (Uuid, Uuid, i32) = sqlx::query_as(
        "SELECT rule.rule_ref,rule.active_revision_ref,revision.fixed_interval_seconds \
         FROM collection_monitor_rule rule \
         JOIN collection_monitor_rule_revision revision \
           ON revision.rule_revision_ref=rule.active_revision_ref \
         WHERE rule.target_ref=$1 AND rule.retired_at IS NULL",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .expect("the active rule revision is readable");
    // 便条来自**更早的那一次调度**，所以它得有自己的 run：`(scheduler_run_ref,target_ref)`
    // 是唯一的，复用本轮 run 会被约束挡住，而真实历史也正是这样——每次派发各归各的 run。
    let historical_run_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_scheduler_run \
             (scheduler_run_ref,scheduler_key,started_at,completed_at,outcome, \
              considered_count,dispatched_count) \
         VALUES ($1,'patrol',scope_001_now()-interval '2 hours', \
                 scope_001_now()-interval '2 hours', 'dispatched',1,1)",
    )
    .bind(historical_run_ref)
    .execute(database.pool())
    .await
    .expect("a historical scheduler run is stored");
    sqlx::query(
        "INSERT INTO collection_scheduler_target_decision \
             (target_decision_ref,scheduler_run_ref,target_ref,rule_ref,rule_revision_ref, \
              outcome,reason_code,cadence_source,effective_interval_seconds,next_eligible_at, \
              work_order_ref,lease_ref) \
         VALUES ($1,$2,$3,$4,$5,'queued','queued','fixed',$6, \
                 scope_001_now()+make_interval(secs=>$6),NULL,NULL)",
    )
    .bind(Uuid::new_v4())
    .bind(historical_run_ref)
    .bind(target_ref)
    .bind(rule_ref)
    .bind(rule_revision_ref)
    .bind(interval_seconds)
    .execute(database.pool())
    .await
    .expect("a pre-fix dispatch note is stored exactly as the old writer wrote it");
}

/// 把规则排期重算到「现在之前」——界面上改周期、暂停恢复之后，排期就是这样被重算的。
///
/// 这一步只动排期，不动调度闸门：两者不一致正是 2026-09-14 那条规则逾期 27 小时的原因。
async fn make_rule_due(database: &Database, target_ref: Uuid) {
    sqlx::query(
        "UPDATE collection_monitor_rule SET monitor_next_run_at=scope_001_now()-interval '1 second' \
         WHERE target_ref=$1 AND retired_at IS NULL",
    )
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("the rule schedule is rewritten to now");
}

/// 读某个目标最近一条决定留下的闸门（`next_eligible_at`）。
///
/// 外层 `None` 是「没有这条决定」，内层 `None` 是「这条决定没有留闸门」。两者必须分得开：
/// 「派发不留便条」一旦被「压根没查到行」冒充过去，断言就守不住它本该守的东西。
async fn latest_decision_gate(
    database: &Database,
    target_ref: Uuid,
    outcome: &str,
) -> Option<Option<String>> {
    sqlx::query_scalar::<_, Option<String>>(
        "SELECT next_eligible_at::text FROM collection_scheduler_target_decision \
         WHERE target_ref=$1 AND outcome=$2 \
         ORDER BY decided_at DESC, target_decision_ref DESC LIMIT 1",
    )
    .bind(target_ref)
    .bind(outcome)
    .fetch_optional(database.pool())
    .await
    .expect("the scheduler decision is readable")
}

async fn read_target_patrol_success(database: &Database, target_ref: Uuid) -> Option<String> {
    sqlx::query_scalar(
        "SELECT last_patrol_succeeded_at::text FROM collection_observation_target \
         WHERE target_ref=$1",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .expect("the target row is readable")
}

/// 走完一轮关键词巡查：建任务（调度器已经建过，这里是回放）、开尝试、交包。
///
/// `acquired` 条记录各自是一条内容卡（`discovery_card` + `content` 身份），落库判为
/// `accepted_for_library_discovery`——判据里那条「接纳条数等于 acquired」正是靠它成立。
/// `stop_reason` 写进**包的 `checkpoint`**：coverage 里那个同名的字段不携带信息。
async fn run_keyword_patrol_round(
    database: &Database,
    task: &ProducerTaskSpec,
    producer_instance_id: Uuid,
    acquired: i64,
    stop_reason: &str,
) {
    assert!(matches!(
        create_producer_task(database, task).await,
        Ok(RuntimeTaskOutcome::Replay { .. })
    ));
    let attempt = attempt(task.task_id(), producer_instance_id);
    assert!(matches!(
        start_producer_attempt(database, &attempt).await,
        Ok(RuntimeAttemptOutcome::Started { .. })
    ));
    let records: Vec<serde_json::Value> = (0..acquired)
        .map(|index| {
            let external_id = format!("keyword-patrol-work-{index}");
            serde_json::json!({
                "kind":"discovery_card",
                "resultPosition": index + 1,
                "sourceObject":{"platform":"xhs","type":"content","externalId":external_id},
                "payload":{
                    "noteId":external_id,
                    "title":format!("关键词巡查样本 {index}"),
                    "likes":"233",
                    "url":format!(
                        "https://www.xiaohongshu.com/search_result/{external_id}?xsec_token=ABkeyword{index}"
                    )
                }
            })
        })
        .collect();
    let submission = parse_producer_submission(
        &serde_json::json!({
            "contractVersion":"linggan.producer.capture-package.v1",
            "producerInstanceId":producer_instance_id,
            "taskId":task.task_id(),
            "attemptId":attempt.attempt_id(),
            "submissionId":Uuid::new_v4(),
            "capturePackage":{
                "contractVersion":"linggan.producer.capture-package.v1",
                "packageRef":Uuid::new_v4(),
                "packageKind":"discovery_search",
                "platform":"xhs",
                "observedAt":"2026-09-13T00:00:00Z",
                "capturedAt":"2026-09-13T00:00:01Z",
                "coverage":{
                    "target":{"basis":"current_visible_surface","surface":"target_driven_surface","query":"adhd"},
                    "layers":[{
                        "capability":"discovery_search",
                        "observed":acquired,"attempted":acquired,"acquired":acquired,
                        "verified":0,"failed":0,"notAttempted":0,"unknown":0,
                        "stoppedReason":"surface_read_complete"
                    }]
                },
                "checkpoint":{"surfaceReceipt":{"stopReason":stop_reason}},
                "records":records
            }
        })
        .to_string(),
    )
    .expect("keyword patrol submission is valid");
    assert!(matches!(
        submit_producer_package(database, &submission).await,
        Ok(RuntimeSubmissionOutcome::Acknowledged { .. })
    ));
}

async fn submit_profile_discovery(
    database: &Database,
    content_external_id: &str,
    signed_url: &str,
) {
    let task = parse_producer_task_spec(
        &serde_json::json!({
            "contractVersion":"linggan.producer.task-spec.v1",
            "taskId":Uuid::new_v4(),
            "source":"manual",
            "platform":"xhs",
            "pageType":"profile",
            "target":{"authorExternalId":"creator-fixture"},
            "capabilitiesRequested":["profile_discovery"],
            "maximumQuota":1,
            "commentLimit":"not_requested",
            "acquireMedia":"not_requested",
            "riskPolicy":"local_trusted_user_initiated",
            "stopConditions":["surface_ended","maximum_quota"]
        })
        .to_string(),
    )
    .expect("profile discovery task is valid");
    assert!(matches!(
        create_producer_task(database, &task).await,
        Ok(RuntimeTaskOutcome::Created { .. })
    ));
    let producer_instance_id = Uuid::new_v4();
    let attempt = attempt(task.task_id(), producer_instance_id);
    assert!(matches!(
        start_producer_attempt(database, &attempt).await,
        Ok(RuntimeAttemptOutcome::Started { .. })
    ));
    let submission = parse_producer_submission(
        &serde_json::json!({
            "contractVersion":"linggan.producer.capture-package.v1",
            "producerInstanceId":producer_instance_id,
            "taskId":task.task_id(),
            "attemptId":attempt.attempt_id(),
            "submissionId":Uuid::new_v4(),
            "capturePackage":{
                "contractVersion":"linggan.producer.capture-package.v1",
                "packageRef":Uuid::new_v4(),
                "packageKind":"profile_discovery",
                "platform":"xhs",
                "observedAt":"2026-08-30T00:00:00Z",
                "capturedAt":"2026-08-30T00:00:01Z",
                "coverage":{
                    "target":{"basis":"known_set","authorExternalId":"creator-fixture"},
                    "layers":[{
                        "capability":"profile_discovery","observed":1,"attempted":1,
                        "acquired":1,"verified":0,"failed":0,"notAttempted":0,
                        "unknown":0,"stoppedReason":"surface_ended"
                    }]
                },
                "records":[{
                    "kind":"profile_discovery_card",
                    "resultPosition":1,
                    "sourceObject":{
                        "platform":"xhs","type":"content","externalId":content_external_id
                    },
                    "payload":{"title":"signed fixture","url":signed_url}
                }]
            }
        })
        .to_string(),
    )
    .expect("signed profile discovery submission is valid");
    assert!(matches!(
        submit_producer_package(database, &submission).await,
        Ok(RuntimeSubmissionOutcome::Acknowledged { .. })
    ));
}

fn same_dispatch(
    left: DispatchDecision,
    right: DispatchDecision,
) -> (DispatchDecision, DispatchDecision) {
    match (left, right) {
        (left @ DispatchDecision::Dispatch { .. }, right @ DispatchDecision::Dispatch { .. }) => {
            (left, right)
        }
        pair => {
            panic!("concurrent callers must receive one idempotent task identity; got {pair:?}")
        }
    }
}

fn task_id(decision: &DispatchDecision) -> Uuid {
    match decision {
        DispatchDecision::Dispatch { task_id, .. } => *task_id,
        other => panic!("expected dispatch, got {other:?}"),
    }
}

fn capability(decision: &DispatchDecision) -> &str {
    match decision {
        DispatchDecision::Dispatch { task_spec, .. } => task_spec["capabilitiesRequested"][0]
            .as_str()
            .expect("scheduled capability is present"),
        other => panic!("expected dispatch, got {other:?}"),
    }
}

fn task_from_dispatch(decision: &DispatchDecision) -> ProducerTaskSpec {
    let task_spec = match decision {
        DispatchDecision::Dispatch { task_spec, .. } => task_spec,
        other => panic!("expected dispatch, got {other:?}"),
    };
    parse_producer_task_spec(&task_spec.to_string()).expect("dispatched task remains valid")
}

fn attempt(task_id: Uuid, producer_instance_id: Uuid) -> linggan_contracts::ProducerAttempt {
    parse_producer_attempt(
        &serde_json::json!({
            "contractVersion": "linggan.producer.attempt.v1",
            "producerInstanceId": producer_instance_id,
            "taskId": task_id,
            "attemptId": Uuid::new_v4(),
        })
        .to_string(),
    )
    .expect("attempt is valid")
}

async fn run_scheduled_task(
    database: &Database,
    task: &ProducerTaskSpec,
    producer_instance_id: Uuid,
) {
    assert!(matches!(
        create_producer_task(database, task).await,
        Ok(RuntimeTaskOutcome::Replay { .. })
    ));
    let attempt = attempt(task.task_id(), producer_instance_id);
    assert!(matches!(
        start_producer_attempt(database, &attempt).await,
        Ok(RuntimeAttemptOutcome::Started { .. })
    ));
    let submission = scheduled_submission(task, &attempt, producer_instance_id);
    assert!(matches!(
        submit_producer_package(database, &submission).await,
        Ok(RuntimeSubmissionOutcome::Acknowledged { .. })
    ));
}

fn scheduled_submission(
    task: &ProducerTaskSpec,
    attempt: &linggan_contracts::ProducerAttempt,
    producer_instance_id: Uuid,
) -> linggan_contracts::ProducerSubmission {
    let capability = task.raw()["capabilitiesRequested"][0]
        .as_str()
        .expect("capability is present");
    parse_producer_submission(
        &serde_json::json!({
            "contractVersion": "linggan.producer.capture-package.v1",
            "producerInstanceId": producer_instance_id,
            "taskId": task.task_id(),
            "attemptId": attempt.attempt_id(),
            "submissionId": Uuid::new_v4(),
            "capturePackage": {
                "contractVersion": "linggan.producer.capture-package.v1",
                "packageRef": Uuid::new_v4(),
                "packageKind": capability,
                "platform": "xhs",
                "observedAt": "2026-08-29T00:00:00Z",
                "capturedAt": "2026-08-29T00:00:01Z",
                "coverage": {
                    "target": {"basis": "known_set", "authorExternalId": "creator-fixture"},
                    "layers": [{
                        "capability": capability,
                        "observed": 0,
                        "attempted": 0,
                        "acquired": 0,
                        "verified": 0,
                        "failed": 0,
                        "notAttempted": 0,
                        "unknown": 0,
                        "stoppedReason": "surface_ended"
                    }]
                },
                "records": []
            }
        })
        .to_string(),
    )
    .expect("scheduled submission is valid")
}

async fn lease_is_live(database: &Database, lease_ref: Uuid) -> bool {
    sqlx::query_scalar(
        "SELECT released_at IS NULL FROM collection_work_order_lease WHERE lease_ref = $1",
    )
    .bind(lease_ref)
    .fetch_one(database.pool())
    .await
    .expect("lease is readable")
}

/// 交付重放必须不新增事实，所以断言它时得同时看 Package 和 Receipt 两张表。
async fn package_and_receipt_counts(database: &Database) -> (i64, i64) {
    let packages: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_runtime_capture_package")
        .fetch_one(database.pool())
        .await
        .expect("package count is readable");
    let receipts: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_runtime_submission_receipt")
            .fetch_one(database.pool())
            .await
            .expect("receipt count is readable");
    (packages, receipts)
}

async fn assert_task_state(database: &Database, task_id: Uuid, expected: &str) {
    let actual: String = sqlx::query_scalar(
        "SELECT execution_state FROM collection_work_order_lease_task WHERE task_id = $1",
    )
    .bind(task_id)
    .fetch_one(database.pool())
    .await
    .expect("lease task state is readable");
    assert_eq!(actual, expected);
}

/// 一条任务的执行现场：谁在执行、租约还作不作数、已经产生了哪些事实、停止过几次。
/// 「重放不得改动任何执行事实」对着这一组值一次比完，比逐个断言更难漏项。
#[derive(Debug, PartialEq)]
struct ExecutionScene {
    task_state: String,
    claimed_at: Option<String>,
    claimed_by: Option<Uuid>,
    lease_live: bool,
    release_reason: Option<String>,
    work_order_state: String,
    attempts_for_task: i64,
    lease_tasks: i64,
    leases: i64,
    packages: i64,
    receipts: i64,
    input_stop_events: i64,
    eligibility_rows: i64,
}

async fn execution_scene(database: &Database, task_id: Uuid, lease_ref: Uuid) -> ExecutionScene {
    let (task_state, claimed_at, claimed_by): (String, Option<String>, Option<Uuid>) = sqlx::query_as(
        "SELECT execution_state, claimed_at::text, claimed_by_installation_ref \
         FROM collection_work_order_lease_task WHERE task_id=$1",
    )
    .bind(task_id)
    .fetch_one(database.pool())
    .await
    .expect("the held task stays readable");
    let (lease_live, release_reason, work_order_state): (bool, Option<String>, String) =
        sqlx::query_as(
            "SELECT lease.released_at IS NULL, lease.release_reason, work_order.queue_state \
             FROM collection_work_order_lease lease \
             JOIN collection_work_order work_order USING(work_order_ref) \
             WHERE lease.lease_ref=$1",
        )
        .bind(lease_ref)
        .fetch_one(database.pool())
        .await
        .expect("the lease and its work order stay readable");
    let attempts_for_task: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_runtime_attempt WHERE task_id=$1")
            .bind(task_id)
            .fetch_one(database.pool())
            .await
            .expect("attempt history is readable");
    let lease_tasks: i64 =
        sqlx::query_scalar("SELECT count(*) FROM collection_work_order_lease_task WHERE lease_ref=$1")
            .bind(lease_ref)
            .fetch_one(database.pool())
            .await
            .expect("lease tasks are readable");
    let leases: i64 = sqlx::query_scalar("SELECT count(*) FROM collection_work_order_lease")
        .fetch_one(database.pool())
        .await
        .expect("leases are readable");
    let input_stop_events: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order_lease_task_dispatch_failure \
         WHERE failure_code='execution_input_missing'",
    )
    .fetch_one(database.pool())
    .await
    .expect("stop events are readable");
    let eligibility_rows: i64 =
        sqlx::query_scalar("SELECT count(*) FROM collection_execution_input_eligibility")
            .fetch_one(database.pool())
            .await
            .expect("the eligibility ledger is readable");
    let (packages, receipts) = package_and_receipt_counts(database).await;
    ExecutionScene {
        task_state,
        claimed_at,
        claimed_by,
        lease_live,
        release_reason,
        work_order_state,
        attempts_for_task,
        lease_tasks,
        leases,
        packages,
        receipts,
        input_stop_events,
        eligibility_rows,
    }
}

fn manual_task() -> ProducerTaskSpec {
    parse_producer_task_spec(
        &serde_json::json!({
            "contractVersion": "linggan.producer.task-spec.v1",
            "taskId": Uuid::new_v4(),
            "source": "manual",
            "platform": "xhs",
            "pageType": "note_detail",
            "target": {"contentExternalId": "manual-fixture"},
            "capabilitiesRequested": ["content_detail"],
            "maximumQuota": 1,
            "commentLimit": "not_requested",
            "acquireMedia": "not_requested",
            "riskPolicy": "local_trusted_user_initiated",
            "stopConditions": ["manual_stop", "maximum_quota"]
        })
        .to_string(),
    )
    .expect("manual task remains valid")
}
