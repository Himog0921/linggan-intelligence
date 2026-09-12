//! 一个目标几条规则：口径、首要位与到期口径的真实证明。
//!
//! 这三件事的共同点是**跑起来全都自洽**：工单发得出去、回执正常、材料落库、覆盖度完整，
//! 只有采回来的东西按了错误的口径。没有断言就等于没修。

#[path = "support/material_fixture.rs"]
mod fixture;

use fixture::proof_database;
use linggan_evidence::{
    MonitorCommandActor, MonitorCommandKind, MonitorCommandOutcomeKind, MonitorRuleCommand,
    MonitorRuleDraft, MonitorRuleMode, apply_monitor_rule_command, read_target_monitor_rules,
    retire_monitor_rule,
};
use linggan_storage_postgres::Database;
use uuid::Uuid;

async fn seed_keyword(database: &Database, identity_key: &str) -> Uuid {
    let target_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_observation_target \
             (target_ref,platform,target_kind,identity_key,display_name,source,lifecycle_state, \
              domain_ref) \
         VALUES ($1,'xhs','keyword',$2,$2,'manual','pending_decision', \
                 '00000000-0000-4000-8000-000000000001')",
    )
    .bind(target_ref)
    .bind(identity_key)
    .execute(database.pool())
    .await
    .expect("keyword target is seeded");
    target_ref
}

fn save_rule(
    target_ref: Uuid,
    ranking: &str,
    interval_seconds: i32,
    expected_revision: i32,
) -> MonitorRuleCommand {
    MonitorRuleCommand {
        target_ref,
        expected_revision,
        kind: MonitorCommandKind::SaveRule,
        actor: MonitorCommandActor::Person,
        source: "targets_ui",
        idempotency_key: Uuid::new_v4(),
        draft: Some(MonitorRuleDraft {
            mode: MonitorRuleMode::Fixed,
            automatic_enabled: true,
            run_on_weekdays: true,
            run_on_weekends: true,
            all_day: true,
            window_start_minute: None,
            window_end_minute: None,
            fixed_interval_seconds: Some(interval_seconds),
            fallback_interval_seconds: interval_seconds,
            surface_key: "keyword_search".to_owned(),
            ranking_key: Some(ranking.to_owned()),
            scroll_rounds: Some(3),
            top_by_likes: Some(20),
            published_within_days: Some(7),
            task_contract_version: "linggan.producer.task-spec.v1".to_owned(),
        }),
    }
}

async fn primary_pointer(database: &Database, target_ref: Uuid) -> Option<Uuid> {
    sqlx::query_scalar(
        "SELECT active_monitor_rule_revision_ref FROM collection_observation_target \
         WHERE target_ref=$1",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .expect("the target row is readable")
}

/// **加第二条规则不该顶掉首要位。**
///
/// 目标行上那一列的含义是「首要规则的当前版本」，全项目还有五处把它当作「这个目标的规则」
/// 在读（发租的取样口径、派发的规则闸、运行产能、管理巡查面板）。给一个关键词存第二条规则时
/// 若照旧覆盖它，那五处会集体开始读一条**非首要**规则——最直接的后果是点「暂停巡检」只停掉了
/// 其中一条，另一条继续按自己的周期跑，而界面上看不出任何异常。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn a_second_rule_does_not_hijack_the_primary_pointer() {
    let database = proof_database("monitor_rule_second_slot").await;
    let target_ref = seed_keyword(&database, "考研自习").await;

    let first = apply_monitor_rule_command(
        &database,
        &save_rule(target_ref, "comprehensive", 86_400, 0),
    )
    .await
    .expect("the first rule is saved");
    assert_eq!(first.outcome, MonitorCommandOutcomeKind::Applied);
    let primary_revision = primary_pointer(&database, target_ref).await;
    assert_eq!(
        primary_revision, first.applied_rule_revision_ref,
        "第一条规则就是首要规则，目标行指着它"
    );

    let second =
        apply_monitor_rule_command(&database, &save_rule(target_ref, "most_liked", 21_600, 1))
            .await
            .expect("the second rule is saved");
    assert_eq!(second.outcome, MonitorCommandOutcomeKind::Applied);
    assert_ne!(
        second.applied_rule_revision_ref, first.applied_rule_revision_ref,
        "两条口径是两条规则，不是同一条的两个版本"
    );
    assert_eq!(
        primary_pointer(&database, target_ref).await,
        primary_revision,
        "加第二条规则不得改写首要位——那一列还有五处在当作「这个目标的规则」读"
    );

    let rules = read_target_monitor_rules(&database, target_ref)
        .await
        .expect("rules are readable")
        .expect("the schema has rules");
    assert_eq!(rules.len(), 2);
    assert_eq!(
        rules.iter().filter(|rule| rule.is_primary).count(),
        1,
        "首要规则只能有一条"
    );
    // 两条各自的周期都留着——这正是「各有各的时间周期」那句话的意思。
    let mut intervals = rules
        .iter()
        .filter_map(|rule| rule.interval_seconds)
        .collect::<Vec<_>>();
    intervals.sort_unstable();
    assert_eq!(intervals, vec![21_600, 86_400]);
}

/// **同一个口径存两次是改那一条，不是再开一条。**
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn saving_the_same_ranking_twice_edits_one_rule() {
    let database = proof_database("monitor_rule_same_slot").await;
    let target_ref = seed_keyword(&database, "学不进去").await;
    apply_monitor_rule_command(&database, &save_rule(target_ref, "most_liked", 86_400, 0))
        .await
        .expect("the rule is saved");
    apply_monitor_rule_command(&database, &save_rule(target_ref, "most_liked", 21_600, 1))
        .await
        .expect("the same slot is saved again");

    let rules = read_target_monitor_rules(&database, target_ref)
        .await
        .expect("rules are readable")
        .expect("the schema has rules");
    assert_eq!(rules.len(), 1, "同一个口径只该有一条在用的规则");
    assert_eq!(
        rules[0].interval_seconds,
        Some(21_600),
        "改的是周期，不是新开一条"
    );
}

/// **停用首要规则必须把首要位交出去。**
///
/// 留下一个没有首要规则的目标，那五处读取会一直读着一条已经停用的规则的版本——而它已经
/// 不在界面的规则列表里了，没人看得出它还在影响什么。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn retiring_the_primary_hands_the_primary_seat_to_a_survivor() {
    let database = proof_database("monitor_rule_retire_primary").await;
    let target_ref = seed_keyword(&database, "图书馆打卡").await;
    let first = apply_monitor_rule_command(
        &database,
        &save_rule(target_ref, "comprehensive", 86_400, 0),
    )
    .await
    .expect("the first rule is saved");
    let second =
        apply_monitor_rule_command(&database, &save_rule(target_ref, "most_liked", 21_600, 1))
            .await
            .expect("the second rule is saved");

    let primary_rule_ref: Uuid = sqlx::query_scalar(
        "SELECT rule_ref FROM collection_monitor_rule \
         WHERE target_ref=$1 AND is_primary AND retired_at IS NULL",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .expect("a primary rule exists");

    retire_monitor_rule(&database, target_ref, primary_rule_ref)
        .await
        .expect("the primary rule is retired");

    let rules = read_target_monitor_rules(&database, target_ref)
        .await
        .expect("rules are readable")
        .expect("the schema has rules");
    assert_eq!(rules.len(), 1, "停用的那条不再出现在可管理的规则里");
    assert!(rules[0].is_primary, "还在的那条必须接过首要位");
    assert_eq!(
        primary_pointer(&database, target_ref).await,
        second.applied_rule_revision_ref,
        "目标行必须跟着指向接手的那条，而不是继续指着已停用的规则"
    );
    let _ = first;
}

/// **到期的是哪条规则，工单就得冻哪条规则的口径。**
///
/// 这是「一个关键词可以同时盯综合榜和点赞榜」这句话的全部意义。准入此前一律从目标行上读
/// 「当前规则」——那在一个目标一条规则的年代成立；现在点赞榜那一轮到期时，冻进工单的可能
/// 是综合榜的口径，插件于是按错误的排序、错误的取样上限去采，**而回执、覆盖度、材料全都
/// 自洽，没有任何一处看得出来**。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn a_due_rule_freezes_its_own_sampling_into_the_work_order() {
    let database = proof_database("monitor_rule_due_freezes_own").await;
    let target_ref = seed_keyword(&database, "考研自习::due").await;
    let primary = apply_monitor_rule_command(
        &database,
        &save_rule(target_ref, "comprehensive", 172_800, 0),
    )
    .await
    .expect("the primary rule is saved");
    let secondary =
        apply_monitor_rule_command(&database, &save_rule(target_ref, "most_liked", 21_600, 1))
            .await
            .expect("the secondary rule is saved");

    // 让目标进入监控，并签一份关键词巡检授权——没有授权时准入会拒绝，工单根本不会产生。
    sqlx::query(
        "INSERT INTO collection_acquisition_authorization \
             (authorization_ref,platform,target_kind,lane,purpose,granted_by,expires_at, \
              allowed_task_templates,allowed_dispatch_lanes,max_work_units) \
         VALUES (gen_random_uuid(),'xhs','keyword','patrol','定时巡检','person', \
                 scope_001_now()+interval '1 day', \
                 ARRAY['keyword_patrol'],ARRAY['immediate','scheduled'],200)",
    )
    .execute(database.pool())
    .await
    .expect("keyword patrol authorization is granted");
    sqlx::query(
        "UPDATE collection_observation_target \
         SET lifecycle_state='monitoring',monitoring_enabled=true WHERE target_ref=$1",
    )
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("the target is monitoring");

    // **只让点赞榜那条到期**，综合榜那条留在未来。
    let secondary_rule_ref: Uuid = sqlx::query_scalar(
        "SELECT rule_ref FROM collection_monitor_rule \
         WHERE target_ref=$1 AND slot_key='most_liked' AND retired_at IS NULL",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .expect("the secondary rule exists");
    sqlx::query(
        "UPDATE collection_monitor_rule \
         SET monitor_next_run_at=CASE WHEN rule_ref=$2 \
               THEN scope_001_now()-interval '1 second' \
               ELSE scope_001_now()+interval '1 day' END \
         WHERE target_ref=$1",
    )
    .bind(target_ref)
    .bind(secondary_rule_ref)
    .execute(database.pool())
    .await
    .expect("only the secondary rule is due");

    let state: (String, bool, Option<String>) = sqlx::query_as(
        "SELECT lifecycle_state,monitoring_enabled, \
                (SELECT slot_key FROM collection_monitor_rule \
                  WHERE rule_ref=$2) \
         FROM collection_observation_target WHERE target_ref=$1",
    )
    .bind(target_ref)
    .bind(secondary_rule_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(
        (state.0.as_str(), state.1, state.2.as_deref()),
        ("monitoring", true, Some("most_liked")),
        "前提：目标在监控中，到期的是点赞榜那条"
    );
    let tick = linggan_evidence::run_due_patrols(&database)
        .await
        .expect("the scheduler tick completes");
    assert!(
        tick.skipped.is_empty(),
        "这一轮本该把到期的那条规则排进队列，实际被跳过：{:?}",
        tick.skipped
    );

    let frozen: Option<Uuid> = sqlx::query_scalar(
        "SELECT monitor_rule_revision_ref FROM collection_work_order \
         WHERE target_ref=$1 AND lane='patrol' ORDER BY created_at DESC LIMIT 1",
    )
    .bind(target_ref)
    .fetch_optional(database.pool())
    .await
    .expect("the queued work order is readable")
    .flatten();
    assert_eq!(
        frozen, secondary.applied_rule_revision_ref,
        "到期的是点赞榜那条，冻进工单的就必须是它的版本——冻成首要那条，插件会按综合排序去采"
    );
    assert_ne!(
        frozen, primary.applied_rule_revision_ref,
        "不得回落到目标行上的「当前规则」"
    );
}
