//! 「这台机器现在能不能接活」的真实证明（COLLECTION-UPGRADE-001 · S4a · 验收行 T29）。
//!
//! 四类故障逐个注入，每类都断言**分类**与**受限 detail**：
//!
//! | 注入 | 期望分类 |
//! |---|---|
//! | 连不上（本机一个必然没人监听的端口） | `database_unreachable` |
//! | 台账表不存在 | `migration_ledger_unreadable` |
//! | 台账在、缺本消费者要求的 migration id | `migrations_not_applied`（detail = 声明序里第一个缺的） |
//! | 台账齐、缺要求的表 | `schema_incompatible`（detail = 声明序里第一个缺的） |
//!
//! 注入一律作用在**真实对象**上（DROP 真表、DELETE 真台账行），不搭替身表：一个手写的替身
//! 会把「真实迁移里到底建了什么」这一层缺陷一起藏起来。每个用例用独立 schema，互不干扰。
//!
//! 另外证明两件容易说不清的事：判定**只读**（探完不会顺手把缺的表或台账补上），以及连不上的
//! 时候**不编造判定时刻**（`checked_at` 是数据库时钟给的，问不到就是 `None`）。

#[path = "support/material_fixture.rs"]
mod fixture;

use fixture::proof_database;
use linggan_evidence::{
    COLLECTION_RUNTIME_REQUIREMENTS, ReadinessState, connect_runtime_readiness,
    probe_runtime_readiness,
};
use linggan_storage_postgres::Database;

/// 与 `scripts/local-runtime.sh migrate` 写进台账的同一个摘要（`shasum -a 256` 该迁移文件）。
const CONTROL_CLOSURE_MIGRATION_ID: &str = "0034_collection_control_closure";
const CONTROL_CLOSURE_MIGRATION_SHA256: &str =
    "a45cfab5a9e9ee31ab221655ba662bacf9794fdb148ee9da0e54a5e7baed9a0c";

/// 夹具应用了全部迁移文件，但台账里只登记了各读取器探针点名的那些 id，`0034` 不在其中。
/// 就绪判据要求它登记在册，所以这里补的是一行**已经应用过、还没登记**的真实记账，不是造账。
async fn record_control_closure(database: &Database) {
    sqlx::query(
        "INSERT INTO linggan_local_schema_migration (migration_id, migration_sha256) \
         VALUES ($1, $2)",
    )
    .bind(CONTROL_CLOSURE_MIGRATION_ID)
    .bind(CONTROL_CLOSURE_MIGRATION_SHA256)
    .execute(database.pool())
    .await
    .expect("the applied migration is recorded in its own ledger");
}

/// 一个「应该被判成可接活」的 schema。
async fn ready_database(schema: &str) -> Database {
    let database = proof_database(schema).await;
    record_control_closure(&database).await;
    database
}

async fn object_exists(database: &Database, object_name: &str) -> bool {
    sqlx::query_scalar("SELECT to_regclass(format('%I.%I', current_schema(), $1)) IS NOT NULL")
        .bind(object_name)
        .fetch_one(database.pool())
        .await
        .expect("catalog probe succeeds")
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn ready_requires_the_ledger_row_and_the_required_tables() {
    let database = ready_database("readiness_ready").await;

    let readiness = probe_runtime_readiness(&database, &COLLECTION_RUNTIME_REQUIREMENTS).await;

    assert_eq!(readiness.state, ReadinessState::Ready);
    assert_eq!(readiness.detail, None, "可接活没有受限原因");
    let checked_at = readiness.checked_at.expect("判定时刻来自数据库时钟");
    assert_eq!(
        checked_at.len(),
        20,
        "判定时刻是 UTC RFC3339 秒精度：{checked_at}"
    );
    assert!(checked_at.ends_with('Z'), "判定时刻带 Z 后缀：{checked_at}");
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn unreachable_database_is_classified_and_never_invents_a_check_time() {
    // 127.0.0.1:1 上不会有 PostgreSQL。用关闭端口而不是不可路由地址：前者立刻拒绝，
    // 后者会让连接挂到池超时，测试就变成了在测超时。
    let (database, readiness) = connect_runtime_readiness(
        "postgresql://readiness_probe:readiness_probe@127.0.0.1:1/readiness_probe",
        &COLLECTION_RUNTIME_REQUIREMENTS,
    )
    .await;

    assert!(database.is_none(), "连不上就不该交出一个连接");
    assert_eq!(readiness.state, ReadinessState::DatabaseUnreachable);
    assert_eq!(
        readiness.detail.as_deref(),
        Some("connect_failed"),
        "detail 只写类别，不带连接串"
    );
    assert_eq!(
        readiness.checked_at, None,
        "问不到数据库时钟就不写一个本地时间冒充判定时刻"
    );
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn missing_ledger_table_is_unreadable_rather_than_unapplied() {
    let database = ready_database("readiness_no_ledger").await;
    sqlx::query("DROP TABLE linggan_local_schema_migration")
        .execute(database.pool())
        .await
        .expect("the ledger table is dropped for this proof");

    let readiness = probe_runtime_readiness(&database, &COLLECTION_RUNTIME_REQUIREMENTS).await;

    assert_eq!(readiness.state, ReadinessState::MigrationLedgerUnreadable);
    assert_eq!(readiness.detail, None);
    assert!(readiness.checked_at.is_some(), "连得上就有数据库时钟");
    assert!(
        !object_exists(&database, "linggan_local_schema_migration").await,
        "判定只读：探一次不会把缺掉的台账补上（应用迁移永远是 local-runtime.sh migrate 的事）"
    );
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn unapplied_migration_names_the_first_missing_id_in_declaration_order() {
    // 夹具原样：0034 的 SQL 应用了，台账里没有它；再造一个同样缺登记的 0036。
    let database = proof_database("readiness_missing_ids").await;
    sqlx::query(
        "DELETE FROM linggan_local_schema_migration \
         WHERE migration_id = '0036_monitor_scheduling_clarity'",
    )
    .execute(database.pool())
    .await
    .expect("the ledger row is removed for this proof");

    let readiness = probe_runtime_readiness(&database, &COLLECTION_RUNTIME_REQUIREMENTS).await;

    assert_eq!(readiness.state, ReadinessState::MigrationsNotApplied);
    assert_eq!(
        readiness.detail.as_deref(),
        Some(CONTROL_CLOSURE_MIGRATION_ID),
        "detail 只写声明序里第一个缺的 id，不是随便一个"
    );
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn misplaced_table_is_schema_incompatible_and_is_not_created_by_the_probe() {
    let database = ready_database("readiness_missing_table").await;
    sqlx::query("DROP TABLE collection_scheduler_run CASCADE")
        .execute(database.pool())
        .await
        .expect("the required table is dropped for this proof");

    let readiness = probe_runtime_readiness(&database, &COLLECTION_RUNTIME_REQUIREMENTS).await;

    assert_eq!(readiness.state, ReadinessState::SchemaIncompatible);
    assert_eq!(
        readiness.detail.as_deref(),
        Some("collection_scheduler_run"),
        "detail 只写声明序里第一个缺的对象"
    );
    assert!(
        !object_exists(&database, "collection_scheduler_run").await,
        "判定只读：探一次不会把缺掉的表补上"
    );
}
