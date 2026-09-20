use linggan_contracts::{
    EvidenceQuery, ProducerTaskSpec, parse_producer_attempt, parse_producer_submission,
    parse_producer_task_spec,
};
use linggan_evidence::{
    AccountEligibilityObservation, AuthorizationGrant, CheckInOutcome, DispatchDecision,
    DispatchFailureCode, DispatchFailureOutcome, InstallationCheckIn, MonitorCommandActor,
    MonitorCommandKind, MonitorRuleCommand, MonitorRuleDraft, MonitorRuleMode,
    ProducerRuntimeError, RuntimeAttemptOutcome, RuntimeSubmissionOutcome, RuntimeTaskOutcome,
    activate_installation_credential, apply_monitor_rule_command, bind_observation_account,
    check_in_installation, create_producer_task, decide_dispatch, dispatch_schema_is_ready,
    expire_lapsed_leases, grant_authorization, issue_work_order_lease, open_claim_window,
    read_collection_task_timeline, read_work_resources, recover_released_orphaned_work_orders,
    report_account_eligibility, requeue_failed_dispatch, rotate_installation_credential,
    set_station_accepting, start_producer_attempt, submit_producer_package,
};
use linggan_storage_postgres::{Database, testing::isolated_proof_schema};
use sqlx::{AssertSqlSafe, Row};
use uuid::Uuid;

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
    include_str!("../../../database/migrations/0090_detail_page_grant_recovery_and_risk_cooldown.sql"),
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
    let database = proof_database_for("collection_dispatch_page_unavailable").await;
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
        DispatchFailureCode::PageUnavailable,
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
    assert_eq!(failure_code, "page_unavailable");
    let replay = requeue_failed_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
        first_task_id,
        unavailable_failure_ref,
        DispatchFailureCode::PageUnavailable,
    )
    .await
    .expect("a lost terminal acknowledgement remains idempotent");
    assert_eq!(replay, DispatchFailureOutcome::Unavailable);
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
    sqlx::query(
        "INSERT INTO collection_work_order \
             (work_order_ref, decision_ref, target_ref, lane, max_works, stop_conditions, \
              station_ref,installation_ref,account_ref) \
         VALUES ($1, $2, $3, 'deep_archive', 10, '[\"maximum_quota\",\"time_budget\"]'::jsonb, $4,$5,$6)",
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
