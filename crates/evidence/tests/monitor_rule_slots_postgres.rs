//! 一个目标几条规则：口径各自的版本号、互不干扰与到期口径的真实证明。
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
        slot_key: None,
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

/// 这条口径当前在用的是哪一版。**问的是规则，不是目标**——目标行上再没有这份副本。
async fn active_revision_of(database: &Database, target_ref: Uuid, slot_key: &str) -> Option<Uuid> {
    sqlx::query_scalar(
        "SELECT active_revision_ref FROM collection_monitor_rule \
         WHERE target_ref=$1 AND slot_key=$2 AND retired_at IS NULL",
    )
    .bind(target_ref)
    .bind(slot_key)
    .fetch_one(database.pool())
    .await
    .expect("the rule row is readable")
}

/// **版本号属于规则，不属于目标。**
///
/// 第二条口径是一条崭新的规则，它的第一版就是第 1 版；提交时报的「我看到的是第 0 版」说的
/// 是这条规则，不是这个目标。此前版本号按目标全局递增，于是存第二条必须报 1、第三条必须报
/// 2——而页面上那条规则明明写着「第 1 版」。结果是配到第三条时必然 `stale_revision`，
/// 再改任何一条都会撞唯一约束。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn each_rule_numbers_its_own_versions() {
    let database = proof_database("monitor_rule_second_slot").await;
    let target_ref = seed_keyword(&database, "考研自习").await;

    let first = apply_monitor_rule_command(
        &database,
        &save_rule(target_ref, "comprehensive", 86_400, 0),
    )
    .await
    .expect("the first rule is saved");
    assert_eq!(first.outcome, MonitorCommandOutcomeKind::Applied);
    assert_eq!(first.current_revision, 1, "一条新规则的第一版就是第 1 版");
    let first_revision = active_revision_of(&database, target_ref, "comprehensive").await;
    assert_eq!(first_revision, first.applied_rule_revision_ref);

    // 第二条口径同样报 0：它是另一条规则的第一版。
    let second =
        apply_monitor_rule_command(&database, &save_rule(target_ref, "most_liked", 21_600, 0))
            .await
            .expect("the second rule is saved");
    assert_eq!(
        (second.outcome, second.reason_code),
        (MonitorCommandOutcomeKind::Applied, "rule_saved"),
        "第二条口径的第一版不该被判成过期版本"
    );
    assert_eq!(second.current_revision, 1);
    assert_ne!(
        second.applied_rule_revision_ref, first.applied_rule_revision_ref,
        "两条口径是两条规则，不是同一条的两个版本"
    );
    assert_eq!(
        active_revision_of(&database, target_ref, "comprehensive").await,
        first_revision,
        "存第二条不得动到第一条在用的版本"
    );

    let rules = read_target_monitor_rules(&database, target_ref)
        .await
        .expect("rules are readable")
        .expect("the schema has rules");
    assert_eq!(rules.len(), 2);
    // 两条各自的周期都留着——这正是「各有各的时间周期」那句话的意思。
    let mut intervals = rules
        .iter()
        .filter_map(|rule| rule.interval_seconds)
        .collect::<Vec<_>>();
    intervals.sort_unstable();
    assert_eq!(intervals, vec![21_600, 86_400]);
}

/// **停用一条口径不得碰到另一条。**
///
/// 规则之间没有主次（`0078` 取消了 `is_primary`）。停掉点赞榜那条，综合榜那条的在用版本
/// 和周期必须原封不动——否则界面上看不出任何异常，采回来的东西却换了口径。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn retiring_one_rule_leaves_the_others_alone() {
    let database = proof_database("monitor_rule_retire_primary").await;
    let target_ref = seed_keyword(&database, "图书馆打卡").await;
    let first = apply_monitor_rule_command(
        &database,
        &save_rule(target_ref, "comprehensive", 86_400, 0),
    )
    .await
    .expect("the first rule is saved");
    apply_monitor_rule_command(&database, &save_rule(target_ref, "most_liked", 21_600, 0))
        .await
        .expect("the second rule is saved");
    let survivor_revision = active_revision_of(&database, target_ref, "comprehensive").await;
    assert_eq!(survivor_revision, first.applied_rule_revision_ref);

    let retired_rule_ref: Uuid = sqlx::query_scalar(
        "SELECT rule_ref FROM collection_monitor_rule \
         WHERE target_ref=$1 AND slot_key='most_liked' AND retired_at IS NULL",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .expect("the most_liked rule exists");

    retire_monitor_rule(&database, target_ref, retired_rule_ref)
        .await
        .expect("the rule is retired");

    let rules = read_target_monitor_rules(&database, target_ref)
        .await
        .expect("rules are readable")
        .expect("the schema has rules");
    assert_eq!(rules.len(), 1, "停用的那条不再出现在可管理的规则里");
    assert_eq!(rules[0].slot_key, "comprehensive");
    assert_eq!(
        rules[0].interval_seconds,
        Some(86_400),
        "留下的那条保持自己的周期"
    );
    assert_eq!(
        active_revision_of(&database, target_ref, "comprehensive").await,
        survivor_revision,
        "停用另一条不得改写这一条在用的版本"
    );
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
        apply_monitor_rule_command(&database, &save_rule(target_ref, "most_liked", 21_600, 0))
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
        "到期的是点赞榜那条，冻进工单的就必须是它的版本——冻成另一条，插件会按综合排序去采"
    );
    assert_ne!(
        frozen, primary.applied_rule_revision_ref,
        "不得回落到另一条口径"
    );
}

/// **一个关键词配三条规则，再各改一次周期。**
///
/// 这正是 Mog 说的用法：7 天综合前 20、7 天点赞前 20、7 天评论前 20，各设各的周期。
/// 版本号按目标全局递增时，这条路一定走不通——第二条报 `stale_revision`，第三条直接撞
/// `(target_ref, revision)` 唯一约束报数据库错误。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn three_rules_can_each_be_edited_afterwards() {
    let database = proof_database("audit_three_rules").await;
    let target_ref = seed_keyword(&database, "审计词").await;
    let rankings = ["comprehensive", "most_liked", "most_commented"];

    // 先配三条：每条都是自己那条规则的第 1 版。
    for ranking in rankings {
        let receipt =
            apply_monitor_rule_command(&database, &save_rule(target_ref, ranking, 604_800, 0))
                .await
                .unwrap_or_else(|error| panic!("{ranking} 这条规则存不进去：{error}"));
        assert_eq!(
            (receipt.outcome, receipt.reason_code),
            (MonitorCommandOutcomeKind::Applied, "rule_saved"),
            "{ranking} 这条口径的第一版被拒了"
        );
        assert_eq!(receipt.current_revision, 1);
    }
    let rules = read_target_monitor_rules(&database, target_ref)
        .await
        .expect("rules are readable")
        .expect("the schema has rules");
    assert_eq!(rules.len(), 3, "三个口径就是三条规则");

    // 再各改一次周期。人会这么做：先配好三条，再调其中一条的频率。
    for ranking in rankings {
        let receipt =
            apply_monitor_rule_command(&database, &save_rule(target_ref, ranking, 86_400, 1))
                .await
                .unwrap_or_else(|error| panic!("{ranking} 这条规则改不了：{error}"));
        assert_eq!(
            (receipt.outcome, receipt.reason_code),
            (MonitorCommandOutcomeKind::Applied, "rule_saved"),
            "{ranking} 改周期被拒了"
        );
        assert_eq!(
            receipt.current_revision, 2,
            "{ranking} 应该走到自己的第 2 版"
        );
    }

    let rules = read_target_monitor_rules(&database, target_ref)
        .await
        .expect("rules are readable")
        .expect("the schema has rules");
    let mut slots = rules
        .iter()
        .map(|rule| (rule.slot_key.as_str(), rule.interval_seconds))
        .collect::<Vec<_>>();
    slots.sort_unstable();
    assert_eq!(
        slots,
        vec![
            ("comprehensive", Some(86_400)),
            ("most_commented", Some(86_400)),
            ("most_liked", Some(86_400)),
        ],
        "三条各自改到的新周期都要落在自己那条上"
    );
}

/// **单独暂停一条口径，不能把整个目标标成已暂停。**
///
/// 目标行上的「在不在被观察」是**所有规则的或**。此前它写的是这条命令那一版的
/// `automatic_enabled`：一个关键词盯三个榜、只停点赞那一条，目标会被标成已暂停、生命周期
/// 掉到 `paused`，而另外两条规则继续按自己的周期跑——列表说「已暂停」，队列里却一直出活。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn pausing_one_rule_leaves_the_target_and_the_other_rules_observing() {
    let database = proof_database("monitor_rule_pause_one").await;
    let target_ref = seed_keyword(&database, "考研自习::pause-one").await;
    for ranking in ["comprehensive", "most_liked"] {
        apply_monitor_rule_command(&database, &save_rule(target_ref, ranking, 86_400, 0))
            .await
            .unwrap_or_else(|error| panic!("{ranking} 存不进去：{error}"));
    }
    assert_target_observing(&database, target_ref, true, "monitoring").await;

    // 只停点赞榜那一条。
    let paused = apply_monitor_rule_command(
        &database,
        &MonitorRuleCommand {
            target_ref,
            expected_revision: 1,
            idempotency_key: Uuid::new_v4(),
            kind: MonitorCommandKind::Pause,
            actor: MonitorCommandActor::Person,
            source: "targets_ui",
            draft: None,
            slot_key: Some("most_liked".to_owned()),
        },
    )
    .await
    .expect("the pause command runs");
    assert_eq!(
        (paused.outcome, paused.reason_code),
        (MonitorCommandOutcomeKind::Applied, "monitor_paused"),
        "指定口径的暂停必须被接受"
    );

    let states = rule_states(&database, target_ref).await;
    assert_eq!(
        states,
        vec![
            ("comprehensive".to_owned(), true),
            ("most_liked".to_owned(), false),
        ],
        "停的必须是点赞榜那一条，综合榜那条不受影响"
    );
    assert_target_observing(&database, target_ref, true, "monitoring").await;

    // 把最后一条也停掉，这时目标才真的不再被观察。
    apply_monitor_rule_command(
        &database,
        &MonitorRuleCommand {
            target_ref,
            expected_revision: 1,
            idempotency_key: Uuid::new_v4(),
            kind: MonitorCommandKind::Pause,
            actor: MonitorCommandActor::Person,
            source: "targets_ui",
            draft: None,
            slot_key: Some("comprehensive".to_owned()),
        },
    )
    .await
    .expect("the second pause runs");
    assert_target_observing(&database, target_ref, false, "paused").await;
}

/// **列表上那个开关一起翻全部规则。**
///
/// 那一列讲的是目标，「巡查中」显示的是几条规则的或。此前它只翻最早那条：点「暂停」之后
/// 另外两条继续跑，而那一列仍然显示「巡查中」——看不出点了没有。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn the_list_toggle_switches_every_rule_of_the_target() {
    let database = proof_database("monitor_rule_list_toggle_all").await;
    let target_ref = seed_keyword(&database, "考研自习::toggle-all").await;
    for ranking in ["comprehensive", "most_liked", "most_commented"] {
        apply_monitor_rule_command(&database, &save_rule(target_ref, ranking, 86_400, 0))
            .await
            .unwrap_or_else(|error| panic!("{ranking} 存不进去：{error}"));
    }

    linggan_evidence::toggle_target_patrol(&database, target_ref, false)
        .await
        .expect("the list pause runs");
    assert!(
        rule_states(&database, target_ref)
            .await
            .iter()
            .all(|(_, automatic)| !automatic),
        "列表上关掉，三条规则必须全部停下——留一条在跑，那一列却显示已暂停"
    );
    assert_target_observing(&database, target_ref, false, "paused").await;

    linggan_evidence::toggle_target_patrol(&database, target_ref, true)
        .await
        .expect("the list resume runs");
    assert!(
        rule_states(&database, target_ref)
            .await
            .iter()
            .all(|(_, automatic)| *automatic),
        "列表上打开，三条规则必须全部恢复"
    );
    assert_target_observing(&database, target_ref, true, "monitoring").await;
}

/// 每条规则的口径与「自动巡查开着没有」，按建立顺序。
async fn rule_states(database: &Database, target_ref: Uuid) -> Vec<(String, bool)> {
    sqlx::query_as(
        "SELECT rule.slot_key,COALESCE(revision.automatic_enabled,false) \
         FROM collection_monitor_rule rule \
         LEFT JOIN collection_monitor_rule_revision revision \
                ON revision.rule_revision_ref=rule.active_revision_ref \
         WHERE rule.target_ref=$1 AND rule.retired_at IS NULL \
         ORDER BY rule.created_at,rule.rule_ref",
    )
    .bind(target_ref)
    .fetch_all(database.pool())
    .await
    .expect("rule states are readable")
}

async fn assert_target_observing(
    database: &Database,
    target_ref: Uuid,
    expected_enabled: bool,
    expected_lifecycle: &str,
) {
    let state: (bool, String) = sqlx::query_as(
        "SELECT monitoring_enabled,lifecycle_state FROM collection_observation_target \
         WHERE target_ref=$1",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .expect("the target row is readable");
    assert_eq!(
        (state.0, state.1.as_str()),
        (expected_enabled, expected_lifecycle),
        "目标级的「在不在被观察」是所有规则的或"
    );
}

/// **「停止观察」不接受口径。**
///
/// 它把生命周期推到 `dismissed`，而那一支不看还有没有别的规则在跑。允许「只停一条口径」
/// 走这条命令，整个目标会变成已停止观察，另外几条却还在按自己的周期出活。当前没有界面能
/// 提交这个组合，但端点收得下——挡在这里，而不是等哪天加个按钮才发现。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn stopping_observation_does_not_accept_a_single_slot() {
    let database = proof_database("monitor_rule_stop_with_slot").await;
    let target_ref = seed_keyword(&database, "考研自习::stop-slot").await;
    for ranking in ["comprehensive", "most_liked"] {
        apply_monitor_rule_command(&database, &save_rule(target_ref, ranking, 86_400, 0))
            .await
            .unwrap_or_else(|error| panic!("{ranking} 存不进去：{error}"));
    }

    let refused = apply_monitor_rule_command(
        &database,
        &MonitorRuleCommand {
            target_ref,
            expected_revision: 1,
            idempotency_key: Uuid::new_v4(),
            kind: MonitorCommandKind::Stop,
            actor: MonitorCommandActor::Person,
            source: "targets_ui",
            draft: None,
            slot_key: Some("most_liked".to_owned()),
        },
    )
    .await
    .expect("the command is decided, not an infrastructure failure");
    assert_eq!(
        (refused.outcome, refused.reason_code),
        (MonitorCommandOutcomeKind::Rejected, "invalid_mode"),
        "带口径的「停止观察」必须被拒"
    );
    assert_target_observing(&database, target_ref, true, "monitoring").await;
    assert!(
        rule_states(&database, target_ref)
            .await
            .iter()
            .all(|(_, automatic)| *automatic),
        "被拒的命令不该改动任何规则"
    );
}
