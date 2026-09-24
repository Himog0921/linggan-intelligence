//! 一轮 tick 的步骤账本与失败隔离（COLLECTION-UPGRADE-001 · S4b · 验收行 T30）。
//!
//! 这一组测的不是某个函数，而是**一轮 tick 在库里的形状**：一行 run、四行步骤、一行心跳。
//! 此前只有巡查写账本，另外三步只打日志——一步崩了，账本上留下的是「这一轮没派活」，
//! 与「没有到期的活」长成同一个样子。
//!
//! 四条用例各自钉一件事：
//!
//! | 用例 | 钉住的事实 |
//! |---|---|
//! | 一步真失败、其余照跑 | 四行步骤同属一个 run；失败行有自己的受限 `error_class`；整轮记失败；其余三步的行与它无关，各自照常收尾 |
//! | 一步自己的表不在 | 那是「没轮到」（`skipped` + `schema_unavailable`），不是失败——整轮不该因此变红 |
//! | 步骤表不在 | `begin` 返回 `None`：不记账、也不假装记了（跑了却记不上比不跑更坏） |
//! | 未就绪写心跳 | 就绪是心跳上的一列，写它**不动** tick 列；上一轮没跑完这件事仍然看得见 |
//!
//! 失败一律**注入在真实对象上**（改真表的列名、DROP 真表），不搭替身：一个手写的替身会把
//! 「真实迁移里到底建了什么」这一层缺陷一起藏起来。每个用例用独立 schema，互不干扰。

#[path = "support/material_fixture.rs"]
mod fixture;

use fixture::proof_database;
use linggan_evidence::{
    AuthorizationGrant, COLLECTION_RUNTIME_REQUIREMENTS, MonitorCommandActor, MonitorCommandKind,
    MonitorRuleCommand, MonitorRuleDraft, MonitorRuleMode, PatrolTickSummary, STEP_KEYWORD_DETAILS,
    STEP_MEDIA_ACQUISITION, STEP_PATROL, STEP_PROGRESSIVE_DOSSIERS, StepOutcome, StepReport,
    TICK_STEP_KEYS, TickLedger, apply_monitor_rule_command, ensure_discovery_cover_media_work,
    grant_authorization, probe_runtime_readiness, record_readiness, run_due_patrol_step,
    run_keyword_archive_details, run_progressive_archives, tick_outcome,
};
use linggan_storage_postgres::Database;
use uuid::Uuid;

/// 一轮 tick 的四步，与 `apps/worker/src/tick.rs` 逐行同形：同一步、同一份受限码。
///
/// 不把 worker 的四步搬进 `evidence`（那是组合入口的事），也不在这里另写一套判据——四处
/// 差异只允许有一处：这里用的是**同一个** `TickLedger::run_step` 与同一批步骤函数。
async fn media_step(database: &Database, ledger: &TickLedger) -> StepReport {
    let (report, _) = ledger
        .run_step(
            STEP_MEDIA_ACQUISITION,
            ensure_discovery_cover_media_work(database),
            |projected| StepOutcome::produced(i64::try_from(*projected).unwrap_or(i64::MAX)),
        )
        .await;
    report
}

async fn dossier_step(database: &Database, ledger: &TickLedger) -> StepReport {
    let (report, _) = ledger
        .run_step(
            STEP_PROGRESSIVE_DOSSIERS,
            run_progressive_archives(database),
            |summary| summary.step_outcome(),
        )
        .await;
    report
}

async fn keyword_step(database: &Database, ledger: &TickLedger) -> StepReport {
    let (report, _) = ledger
        .run_step(
            STEP_KEYWORD_DETAILS,
            run_keyword_archive_details(database, "关键词建档详情补采（证明）"),
            |summary| summary.step_outcome(),
        )
        .await;
    report
}

async fn patrol_step(database: &Database, ledger: &TickLedger) -> StepReport {
    let (report, _) = ledger
        .run_step(
            STEP_PATROL,
            run_due_patrol_step(database, ledger.run_ref()),
            PatrolTickSummary::step_outcome,
        )
        .await;
    report
}

/// 一轮完整的 tick：开账本、四步、收轮。交回 run 号与四份报告。
async fn run_one_tick(database: &Database) -> (Uuid, Vec<StepReport>) {
    let ledger = TickLedger::begin(database)
        .await
        .expect("the ledger is readable")
        .expect("the ledger tables are present");
    let reports = vec![
        media_step(database, &ledger).await,
        dossier_step(database, &ledger).await,
        keyword_step(database, &ledger).await,
        patrol_step(database, &ledger).await,
    ];
    ledger
        .finish(&reports)
        .await
        .expect("the tick closes in its ledger");
    (ledger.run_ref(), reports)
}

/// 步骤行，按 `step_key` 排好序：(step_key, outcome, skipped_reason, error_class, 三个计数)。
type StepRow = (
    String,
    String,
    Option<String>,
    Option<String>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
);

async fn step_rows(database: &Database, run_ref: Uuid) -> Vec<StepRow> {
    sqlx::query_as(
        "SELECT step_key,outcome,skipped_reason,error_class,considered_count,produced_count, \
             skipped_count \
         FROM collection_scheduler_run_step WHERE scheduler_run_ref=$1 ORDER BY step_key",
    )
    .bind(run_ref)
    .fetch_all(database.pool())
    .await
    .expect("step rows are readable")
}

/// 一行步骤，按步骤名取。**不按行号取**：行是按 `step_key` 排序出来的，第 0 行是
/// `keyword_details` 而不是第一步——按位置写断言，测的就是字母序了。
async fn step_row(database: &Database, run_ref: Uuid, step_key: &str) -> StepRow {
    step_rows(database, run_ref)
        .await
        .into_iter()
        .find(|row| row.0 == step_key)
        .unwrap_or_else(|| panic!("{step_key} 这一行必须在账本里"))
}

async fn heartbeat(database: &Database) -> (String, Option<String>, String) {
    sqlx::query_as(
        "SELECT last_outcome,last_error,readiness_state FROM collection_scheduler_heartbeat \
         WHERE scheduler_key='patrol'",
    )
    .fetch_one(database.pool())
    .await
    .expect("the heartbeat row exists after a tick")
}

async fn run_outcome(database: &Database, run_ref: Uuid) -> (String, bool) {
    let (outcome, completed): (String, bool) = sqlx::query_as(
        "SELECT outcome,completed_at IS NOT NULL FROM collection_scheduler_run \
         WHERE scheduler_run_ref=$1",
    )
    .bind(run_ref)
    .fetch_one(database.pool())
    .await
    .expect("the run row exists");
    (outcome, completed)
}

/// 真实失败：把媒体投影选的那一列改名。它不在该步自己的 schema 探针里（探针只看两张
/// 媒资表），所以这一步会拿到一个真的 SQLSTATE（`42703` undefined_column），不是「表不在」。
async fn break_the_media_projection(database: &Database) {
    sqlx::query(
        "ALTER TABLE linggan_material_discovery_finding \
         RENAME COLUMN cover_source_state TO cover_source_state_moved",
    )
    .execute(database.pool())
    .await
    .expect("the column is renamed for this proof");
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn one_failing_step_is_recorded_alone_while_the_other_three_finish() {
    let database = proof_database("tick_one_step_fails").await;
    break_the_media_projection(&database).await;

    let (run_ref, reports) = run_one_tick(&database).await;

    // 四步各自一份报告，同一个 run 号——一步失败不改写另外三步的结论。
    assert_eq!(reports.len(), TICK_STEP_KEYS.len());
    assert_eq!(reports[0].outcome.error_class(), Some("sqlstate_42703"));
    for report in &reports[1..] {
        assert_eq!(
            report.outcome.code(),
            "ok",
            "{} 不该被上一步的失败连坐",
            report.step_key
        );
    }

    let rows = step_rows(&database, run_ref).await;
    assert_eq!(rows.len(), TICK_STEP_KEYS.len(), "一个 run 下四行步骤");
    let keys: Vec<&str> = rows.iter().map(|row| row.0.as_str()).collect();
    let mut expected = TICK_STEP_KEYS.to_vec();
    expected.sort_unstable();
    assert_eq!(keys, expected, "步骤名与 0098 的 CHECK 逐字一致");

    let media = step_row(&database, run_ref, STEP_MEDIA_ACQUISITION).await;
    assert_eq!(media.1, "failed");
    assert_eq!(media.2, None, "失败不是跳过，没有跳过原因");
    assert_eq!(media.3.as_deref(), Some("sqlstate_42703"));
    assert_eq!(
        (media.4, media.5, media.6),
        (None, None, None),
        "没数过的计数留空，不写 0"
    );

    for step_key in [STEP_PROGRESSIVE_DOSSIERS, STEP_KEYWORD_DETAILS, STEP_PATROL] {
        let row = step_row(&database, run_ref, step_key).await;
        assert_eq!((row.1.as_str(), row.3.as_deref()), ("ok", None));
    }

    // 整轮记失败，且**只有失败的那一步**被点名：一步失败不会让这一轮长得像「什么都没发生」。
    let (outcome, completed) = run_outcome(&database, run_ref).await;
    assert_eq!(outcome, "failed");
    assert!(completed, "收轮写了完成时刻");
    let (last_outcome, last_error, readiness) = heartbeat(&database).await;
    assert_eq!(last_outcome, "failed");
    assert_eq!(
        last_error.as_deref(),
        Some("media_acquisition:sqlstate_42703"),
        "心跳只记受限的「步骤:分类」，不记原始报文"
    );
    assert_eq!(
        readiness, "unknown",
        "这一轮没写就绪判定，就不该冒充一个：刚启动与判过是两件事"
    );

    // 「不是第二套账本」：这一轮在既有 run 表里恰好一行，四步是它的子行。
    let run_rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_scheduler_run WHERE scheduler_run_ref=$1",
    )
    .bind(run_ref)
    .fetch_one(database.pool())
    .await
    .expect("the run table is readable");
    assert_eq!(run_rows, 1);
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn a_step_whose_own_tables_are_missing_is_skipped_rather_than_failed() {
    let database = proof_database("tick_step_skipped").await;
    sqlx::query("DROP TABLE linggan_discovery_cover_media_link CASCADE")
        .execute(database.pool())
        .await
        .expect("the media table is dropped for this proof");

    let (run_ref, reports) = run_one_tick(&database).await;

    assert_eq!(
        reports[0].outcome,
        StepOutcome::skipped("schema_unavailable"),
        "自己那张表不在是「没轮到」，不是故障"
    );
    assert_eq!(
        tick_outcome(&reports),
        "idle",
        "没轮到的步骤不该把整轮染红——那会让真故障淹没在假故障里"
    );

    let media = step_row(&database, run_ref, STEP_MEDIA_ACQUISITION).await;
    assert_eq!(media.1, "skipped");
    assert_eq!(media.2.as_deref(), Some("schema_unavailable"));
    assert_eq!(media.3, None, "跳过不是失败，没有错误分类");
    assert_eq!(
        (media.4, media.5, media.6),
        (None, None, None),
        "没轮到的步骤没有计数"
    );

    let (outcome, _) = run_outcome(&database, run_ref).await;
    assert_eq!(outcome, "idle");
    let (last_outcome, last_error, _) = heartbeat(&database).await;
    assert_eq!(last_outcome, "idle");
    assert_eq!(last_error, None, "心跳的约束：不是失败就没有错误");
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn an_absent_step_ledger_opens_nothing_rather_than_a_half_recorded_tick() {
    let database = proof_database("tick_ledger_absent").await;
    sqlx::query("DROP TABLE collection_scheduler_run_step")
        .execute(database.pool())
        .await
        .expect("the step table is dropped for this proof");

    let before: i64 = sqlx::query_scalar("SELECT count(*) FROM collection_scheduler_run")
        .fetch_one(database.pool())
        .await
        .expect("the run table is readable");

    let opened = TickLedger::begin(&database)
        .await
        .expect("a missing ledger is not an error, it is an answer");
    assert!(
        opened.is_none(),
        "账本不在时不开轮：跑了却记不上，比不跑更坏"
    );

    let after: i64 = sqlx::query_scalar("SELECT count(*) FROM collection_scheduler_run")
        .fetch_one(database.pool())
        .await
        .expect("the run table is readable");
    assert_eq!(after, before, "没有开轮就没有 run 行");
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn readiness_is_a_heartbeat_column_that_does_not_touch_the_tick_columns() {
    // 夹具没有登记 `0034`（就绪判据要求它在册），所以这里天然是一个「连得上、但判不了能接活」
    // 的库——正是要证明的那种状态。
    let database = proof_database("tick_readiness_landing").await;
    let ledger = TickLedger::begin(&database)
        .await
        .expect("the ledger is readable")
        .expect("the ledger tables are present");

    let readiness = probe_runtime_readiness(&database, &COLLECTION_RUNTIME_REQUIREMENTS).await;
    assert!(
        !readiness.is_ready(),
        "台账里缺本消费者要求的 migration id 时不该判成可接活"
    );
    record_readiness(&database, &readiness)
        .await
        .expect("the readiness write lands in the heartbeat");

    let (state, detail, checked_at): (String, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT readiness_state,readiness_detail,readiness_checked_at::text \
         FROM collection_scheduler_heartbeat WHERE scheduler_key='patrol'",
    )
    .fetch_one(database.pool())
    .await
    .expect("the heartbeat row is readable");
    assert_eq!(state, readiness.state.code());
    assert_eq!(detail, readiness.detail);
    assert!(checked_at.is_some(), "判定时刻来自数据库时钟");

    // 就绪写在心跳上，**不动** tick 的列：这一轮开了没跑完（`begin` 之后没有 `finish`），
    // 那件事仍然看得见——一次就绪判定不该把「上一轮没跑完」擦掉。
    let (last_outcome, last_completed_at): (String, Option<String>) = sqlx::query_as(
        "SELECT last_outcome,last_tick_completed_at::text FROM collection_scheduler_heartbeat \
         WHERE scheduler_key='patrol'",
    )
    .fetch_one(database.pool())
    .await
    .expect("the heartbeat row is readable");
    assert_eq!(last_outcome, "unknown", "开轮写了「还没结局」，就留着");
    assert_eq!(last_completed_at, None, "这一轮还没收轮");
    drop(ledger);
}

/// 一条到期的巡查规则 + 一份适用于它的授权，让巡查步在这一轮里真的排出一张工单。
///
/// 走的是与 `collection_dispatch_sequence_postgres` 相同的公开入口（改目标、存规则、把排期
/// 拨到过期），不另造一套「更简单的」夹具：夹具替系统干活，藏起来的正是系统真会怎么做。
async fn seed_a_due_patrol_rule(database: &Database) -> Uuid {
    let target_ref = Uuid::new_v4();
    // 领域是采集的**准入前提**，不是装饰：`0041` 之后，没归属领域的目标在申请工位之前就被
    // 挡下（`target_domain_unassigned`）——「这批材料该写进哪个库」必须在花掉第一次平台
    // 访问之前就有答案。所以这里像建档路径（`store_pending_target`）一样，显式把本领域写上。
    // 领域号从表里读，不抄 `0041` 里那串字面量：抄下来就多了一份会各自漂的副本。
    sqlx::query(
        "INSERT INTO collection_observation_target \
             (target_ref,platform,target_kind,identity_key,display_name,source,lifecycle_state) \
         VALUES ($1,'xhs','creator','tick-trace-proof','串证证明','manual','pending_decision')",
    )
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("the patrol target is seeded");
    sqlx::query(
        "INSERT INTO observation_domain_target(domain_ref,target_ref,role) \
         SELECT home.domain_ref,$1,'primary' FROM observation_domain home WHERE home.name='ADHD'",
    )
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("the target has an explicit Domain relation");
    grant_authorization(
        database,
        &AuthorizationGrant {
            platform: "xhs",
            target_kind: "creator",
            lane: "patrol",
            purpose: "tick trace proof",
            max_targets: Some(10),
            max_works_per_target: Some(30),
            valid_for_days: 1,
        },
    )
    .await
    .expect("creator patrol authorization is granted");
    let saved = apply_monitor_rule_command(
        database,
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
    .expect("the patrol rule is saved");
    assert_eq!(saved.reason_code, "rule_saved");
    // 排期住在规则上（`0076`/`0078`）：把它拨到过期，这一轮才会排活。
    sqlx::query(
        "UPDATE collection_monitor_rule \
         SET monitor_next_run_at=scope_001_now()-interval '1 second' WHERE target_ref=$1",
    )
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("the isolated clock makes the rule due");
    target_ref
}

/// S4 退出条件：**一次 tick 的一步失败，只用引用就能串出整个故事**（COLLECTION-UPGRADE-001 · S4d）。
///
/// 一条采集链断在哪里，此前要靠在几万行日志里往回翻：调度说「这一轮没派活」，插件说「没拿到
/// 活」，哪个都不指向同一个东西。现在一次 tick 有一个号（`tickRef`），库里四张表都挂着它或
/// 挂在挂着它的东西上，于是「谁失败了、失败在哪个类别、这一步排出了什么、排出给谁」是一条
/// JOIN 的事，不需要累计任何日志。
///
/// 反向也钉住：**日志里那一行的 `tickRef` 与库里串证的入口是同一个号**——两处各算一个号，
/// 就会各自都对、合起来对不上。这份报告也写进账本，两处的值出自同一份映射
/// （`StepReport::event`），本用例用的正是 `apps/worker` 发事件时走的那个方法。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn one_tick_ref_strings_the_failure_and_what_it_queued_without_reading_the_logs() {
    let database = proof_database("tick_trace_chain").await;
    let target_ref = seed_a_due_patrol_rule(&database).await;
    // 真的坏一处：媒体投影要读的那一列改名，这一步会拿到一个真实的 SQLSTATE。
    break_the_media_projection(&database).await;

    let (run_ref, reports) = run_one_tick(&database).await;

    // 只用 run 号——也是事件里的 `tickRef`。四张表走到底，中间没有一处读日志或累计计数。
    let trace: (String, Option<String>, String, String, Uuid, Uuid, String) = sqlx::query_as(
        "SELECT step.step_key, step.error_class, decision.outcome, decision.reason_code, \
                decision.target_ref, decision.work_order_ref, work_order.queue_state \
         FROM collection_scheduler_run_step step \
         JOIN collection_scheduler_target_decision decision \
           ON decision.scheduler_run_ref = step.scheduler_run_ref \
         JOIN collection_work_order work_order \
           ON work_order.work_order_ref = decision.work_order_ref \
         WHERE step.scheduler_run_ref = $1 AND step.outcome = 'failed'",
    )
    .bind(run_ref)
    .fetch_one(database.pool())
    .await
    .expect("从一个 run 号出发就能走到「哪一步失败了 + 这一步排出了什么」");

    assert_eq!(trace.0, STEP_MEDIA_ACQUISITION, "失败的是哪一步");
    assert_eq!(
        trace.1.as_deref(),
        Some("sqlstate_42703"),
        "失败在哪个受限类别（不是报文）"
    );
    assert_eq!(
        (trace.2.as_str(), trace.3.as_str()),
        ("queued", "queued"),
        "同一轮里巡查步排出的决定"
    );
    assert_eq!(trace.4, target_ref, "决定指向的目标");
    assert_eq!(trace.6, "queued", "决定指向的工单此刻的状态");

    // 工单确实挂在这个目标上：串证不是「两条各自成立的记录被摆在了一起」。
    let work_order_target: Uuid =
        sqlx::query_scalar("SELECT target_ref FROM collection_work_order WHERE work_order_ref=$1")
            .bind(trace.5)
            .fetch_one(database.pool())
            .await
            .expect("the queued work order is readable");
    assert_eq!(work_order_target, target_ref);

    // 日志那一侧：同一份报告发出去的那一行，号与库里串证的入口是同一个，分类码也同一个。
    let failed = reports
        .iter()
        .find(|report| report.step_key == STEP_MEDIA_ACQUISITION)
        .expect("这一轮四步各有报告");
    let line: serde_json::Value =
        serde_json::from_str(&failed.event(run_ref).to_json()).expect("一行事件就是一行 JSON");
    assert_eq!(line["tickRef"], run_ref.to_string());
    assert_eq!(line["stepKey"], STEP_MEDIA_ACQUISITION);
    assert_eq!(line["outcome"], "failed");
    assert_eq!(line["errorClass"], "sqlstate_42703");
    assert!(
        line.get("considered").is_none() && line.get("produced").is_none(),
        "失败的行不带计数：账本里这一步也没有数，两个读者说同一件事"
    );
    // 这一行**只能**带白名单上的字段（`runtime_event::EVENT_FIELD_WHITELIST`）：这一步是
    // 运行链上的最后一道口，任何一处「顺手把详情塞进日志」都会在这里露出来。
    for key in line.as_object().expect("一行事件就是一个对象").keys() {
        assert!(
            linggan_evidence::EVENT_FIELD_WHITELIST.contains(&key.as_str()),
            "事件漏出了白名单之外的字段：{key}"
        );
    }
}
