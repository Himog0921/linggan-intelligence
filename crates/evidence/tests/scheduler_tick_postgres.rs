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
    COLLECTION_RUNTIME_REQUIREMENTS, PatrolTickSummary, STEP_KEYWORD_DETAILS, STEP_MEDIA_ACQUISITION,
    STEP_PATROL, STEP_PROGRESSIVE_DOSSIERS, StepOutcome, StepReport, TICK_STEP_KEYS, TickLedger,
    ensure_discovery_cover_media_work, probe_runtime_readiness, record_readiness,
    run_due_patrol_step, run_keyword_archive_details, run_progressive_archives, tick_outcome,
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

    for step_key in [
        STEP_PROGRESSIVE_DOSSIERS,
        STEP_KEYWORD_DETAILS,
        STEP_PATROL,
    ] {
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
