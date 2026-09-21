//! 这台机器现在能不能接活。
//!
//! 「连不上数据库」与「连得上，但这套代码要求的东西不在」是两件不同的事：前者会自愈
//! （等数据库回来），后者要人跑迁移，不会自己好。把它们压成同一句话（或者干脆什么都不说）
//! 的后果是：一台迁移没跑完的机器看起来和一台空闲的机器一模一样——巡检照常报「本轮 0 个」。
//!
//! 判定顺序固定，先因后果：
//!
//!   1. 迁移台账在不在、读不读得动（`linggan_local_schema_migration`）；
//!   2. 本消费者要求的 migration id 是否都在台账里 → 不齐是**未迁移**；
//!   3. 本消费者要求的表是否都在当前 schema 里 → 不在是 **schema 不兼容**；
//!   4. 都满足 → **可接活**。
//!
//! 任何一步的查询本身失败（连接断、池超时、库在恢复中）都归到 `database_unreachable`：
//! 那是环境问题，不是这套代码的问题，退避重试即可。
//!
//! 这里只读三样东西（台账、`pg_catalog`、`information_schema` 的等价物），不写任何行、
//! 不应用迁移——`scripts/local-runtime.sh migrate` 仍然是唯一应用迁移的人。每个消费者只
//! 声明自己真的会用到的要求：断言开得越宽，越容易把一台健康机器判成未就绪。
//!
//! 要求清单是**下限**，不是完整依赖清单。它只回答「这台机器还能不能干这件事」；某个步骤
//! 缺了它自己那张表，是那一步失败（有步骤结果可查），不该把整台机器判成未就绪——否则
//! 一个步骤的缺口会连坐其它还能跑的步骤。

use std::time::Duration;

use linggan_storage_postgres::Database;

/// 未就绪时重新判定的节奏：1 秒起步、翻倍、封顶 60 秒。
///
/// 起点短，是因为数据库常常只是还在启动，几秒就回来，不该让它等到下一分钟；上限等于扫描
/// 节奏，所以持续故障时每分钟只问一次。两个 worker 共用这一份节奏，不各写一套。
pub const READINESS_RETRY_START: Duration = Duration::from_secs(1);
pub const READINESS_RETRY_CAP: Duration = Duration::from_secs(60);

/// 退避序列按 1、2、4……翻倍，到这里封顶。
pub fn next_readiness_retry(current: Duration) -> Duration {
    (current * 2).min(READINESS_RETRY_CAP)
}

/// 一台运行时（worker / API）要接活，必须满足的物理条件。
#[derive(Debug, Clone, Copy)]
pub struct RuntimeRequirements {
    /// 必须已经在迁移台账里登记的 migration id（按声明的先后报告缺失项）。
    pub migration_ids: &'static [&'static str],
    /// 必须在当前 schema 里存在的表名。
    pub tables: &'static [&'static str],
}

/// 采集运行时（巡检 worker 与本地 API 的采集读口）的最小要求：
/// 巡检账本由 `0034` 建立、`0036` 定下结果词表，派发前要读的资格台账是 `0097`，
/// 每轮 tick 要写的步骤明细是 `0098`。
pub const COLLECTION_RUNTIME_REQUIREMENTS: RuntimeRequirements = RuntimeRequirements {
    migration_ids: &[
        "0034_collection_control_closure",
        "0036_monitor_scheduling_clarity",
        "0097_collection_execution_input_eligibility",
        "0098_scheduler_tick_steps_and_readiness",
    ],
    tables: &[
        "collection_scheduler_run",
        "collection_scheduler_run_step",
        "collection_scheduler_heartbeat",
        "collection_scheduler_target_decision",
        "collection_monitor_rule_revision",
        "collection_execution_input_eligibility",
    ],
};

/// 就绪分级。互斥、可判定，日志、心跳、页面与 `/health` 共用同一组码。
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ReadinessState {
    /// 被要求接活，但连数据库的地址都没给。这是配置错误，等多久都不会好。
    NotConfigured,
    /// 连不上，或探针查询失败（连接池超时、库在恢复中……）。
    DatabaseUnreachable,
    /// 连得上，但迁移台账不存在或读不动。不知道迁移跑到哪了，就不能假设它是齐的。
    MigrationLedgerUnreadable,
    /// 台账可读，但本消费者要求的 migration id 不在。
    MigrationsNotApplied,
    /// 台账齐，但要求的表不在（旧库、半迁移库）。
    SchemaIncompatible,
    /// 可以接活。
    Ready,
}

impl ReadinessState {
    /// 机器可读的码。中文说法由各表面自己给，含义以这里为准。
    pub fn code(self) -> &'static str {
        match self {
            Self::NotConfigured => "not_configured",
            Self::DatabaseUnreachable => "database_unreachable",
            Self::MigrationLedgerUnreadable => "migration_ledger_unreadable",
            Self::MigrationsNotApplied => "migrations_not_applied",
            Self::SchemaIncompatible => "schema_incompatible",
            Self::Ready => "ready",
        }
    }

    pub fn is_ready(self) -> bool {
        matches!(self, Self::Ready)
    }
}

/// 一次就绪判定的结论。
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeReadiness {
    pub state: ReadinessState,
    /// 受限原因：只写第一个缺失的 migration id、第一个缺失的表名，或探针失败的 SQLSTATE。
    /// 不写原始错误报文——它可能带着连接串。
    pub detail: Option<String>,
    /// 判定时刻来自**数据库时钟**（UTC，RFC3339）；连不上时是 `None`：不知道就不写一个
    /// 本地时间冒充。
    pub checked_at: Option<String>,
}

impl RuntimeReadiness {
    pub fn not_configured() -> Self {
        Self {
            state: ReadinessState::NotConfigured,
            detail: None,
            checked_at: None,
        }
    }

    pub fn is_ready(&self) -> bool {
        self.state.is_ready()
    }

    fn report(state: ReadinessState, detail: Option<String>, checked_at: Option<String>) -> Self {
        Self {
            state,
            detail,
            checked_at,
        }
    }

    /// 从外面判定出来的「够不着」：调用方自己给上限（比如探针超时），detail 写它给的类别码。
    ///
    /// 连接池有它自己的耐心（sqlx 默认 `acquire_timeout` 30 秒，期间反复重试）。那是连接池
    /// 该有的，不是操作员该等的——一台连不上数据库的机器要半分钟之后才第一次说出「不可达」，
    /// 在这半分钟里它和一台健康空闲的机器长得一模一样。所以判定上限由调用方给，形状仍由这里定。
    pub fn unreachable(detail: &'static str) -> Self {
        Self::report(
            ReadinessState::DatabaseUnreachable,
            Some(detail.to_owned()),
            None,
        )
    }

    /// 探针本身失败：归到「数据库不可达」。detail 只保留 SQLSTATE（五个字符，数据库给的
    /// 错误类别），原始报文不落日志。
    fn probe_failed(error: &sqlx::Error) -> Self {
        let detail = error
            .as_database_error()
            .and_then(|database_error| database_error.code())
            .map(|code| code.to_string());
        Self::report(
            ReadinessState::DatabaseUnreachable,
            Some(detail.unwrap_or_else(|| "probe_failed".to_owned())),
            None,
        )
    }
}

/// 连接一次并判定。连不上就返回 `(None, database_unreachable)`——调用方据此进入退避重试，
/// 而不是把「连不上」当成可以继续跑的常态。
pub async fn connect_runtime_readiness(
    url: &str,
    requirements: &RuntimeRequirements,
) -> (Option<Database>, RuntimeReadiness) {
    match Database::connect(url).await {
        Ok(database) => {
            let readiness = probe_runtime_readiness(&database, requirements).await;
            (Some(database), readiness)
        }
        // 数据库自己给的报文可能带着连接串，这里只记类别。
        Err(_) => (
            None,
            RuntimeReadiness::report(
                ReadinessState::DatabaseUnreachable,
                Some("connect_failed".to_owned()),
                None,
            ),
        ),
    }
}

/// 已经有连接时只做判定。每轮 tick 都用它；代价是三条只读查询。
pub async fn probe_runtime_readiness(
    database: &Database,
    requirements: &RuntimeRequirements,
) -> RuntimeReadiness {
    let checked_at = match sqlx::query_scalar::<_, String>(
        "SELECT to_char(now() AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')",
    )
    .fetch_one(database.pool())
    .await
    {
        Ok(clock) => clock,
        Err(error) => return RuntimeReadiness::probe_failed(&error),
    };

    let ledger_present: bool = match sqlx::query_scalar(
        "SELECT to_regclass(format('%I.%I', current_schema(), 'linggan_local_schema_migration')) \
                IS NOT NULL",
    )
    .fetch_one(database.pool())
    .await
    {
        Ok(present) => present,
        Err(error) => return RuntimeReadiness::probe_failed(&error),
    };
    if !ledger_present {
        return RuntimeReadiness::report(
            ReadinessState::MigrationLedgerUnreadable,
            None,
            Some(checked_at),
        );
    }

    let missing_migration: Option<String> = match sqlx::query_scalar(
        "SELECT probe.migration_id \
         FROM unnest($1::text[]) WITH ORDINALITY AS probe(migration_id, position) \
         WHERE NOT EXISTS ( \
             SELECT 1 FROM linggan_local_schema_migration applied \
             WHERE applied.migration_id = probe.migration_id) \
         ORDER BY probe.position LIMIT 1",
    )
    .bind(requirements.migration_ids)
    .fetch_optional(database.pool())
    .await
    {
        Ok(missing) => missing,
        Err(error) => return RuntimeReadiness::probe_failed(&error),
    };
    if let Some(migration_id) = missing_migration {
        return RuntimeReadiness::report(
            ReadinessState::MigrationsNotApplied,
            Some(migration_id),
            Some(checked_at),
        );
    }

    let missing_table: Option<String> = match sqlx::query_scalar(
        "SELECT probe.object_name \
         FROM unnest($1::text[]) WITH ORDINALITY AS probe(object_name, position) \
         WHERE to_regclass(format('%I.%I', current_schema(), probe.object_name)) IS NULL \
         ORDER BY probe.position LIMIT 1",
    )
    .bind(requirements.tables)
    .fetch_optional(database.pool())
    .await
    {
        Ok(missing) => missing,
        Err(error) => return RuntimeReadiness::probe_failed(&error),
    };
    if let Some(object_name) = missing_table {
        return RuntimeReadiness::report(
            ReadinessState::SchemaIncompatible,
            Some(object_name),
            Some(checked_at),
        );
    }

    RuntimeReadiness::report(ReadinessState::Ready, None, Some(checked_at))
}
