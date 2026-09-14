//! One target, one read-time decision projection for the Collection inspector.
//!
//! This module does not create another target or archive truth. It combines target-scoped,
//! already-persisted execution, accepted material and patrol facts in one repeatable-read
//! snapshot. In particular, a queued Work Order is never reported as running, and the absence
//! of a qualified patrol result remains unknown rather than becoming a zero.

use linggan_storage_postgres::Database;
use serde::Serialize;
use sqlx::Row;
use uuid::Uuid;

#[path = "target_inspector_sql.rs"]
mod sql;
use sql::{ARCHIVE_SQL, EXECUTION_SQL, LATEST_PATROL_SQL};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "state", content = "value")]
pub enum TargetInspectorCount {
    Known(i64),
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetInspectorArchiveState {
    NotApplicable,
    NotStarted,
    Queued,
    Running,
    Partial,
    Blocked,
    Complete,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetInspectorDirectoryState {
    NotApplicable,
    NotStarted,
    Building,
    Ready,
    Historical,
    RebuildRequired,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetInspectorExecutionState {
    Idle,
    Queued,
    AwaitingProducer,
    Running,
    Blocked,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetInspectorPatrolState {
    Disabled,
    Waiting,
    Queued,
    AwaitingProducer,
    Running,
    Normal,
    Blocked,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetInspectorAction {
    NoActionHealthy,
    NoActionQueued,
    NoActionRunning,
    StartArchive,
    RebuildDirectory,
    ContinueArchive,
    HandleArchiveProblems,
    EnablePatrol,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetInspectorArchive {
    pub state: TargetInspectorArchiveState,
    pub directory_state: TargetInspectorDirectoryState,
    pub started: bool,
    pub attempted: bool,
    pub author_profile_captures: TargetInspectorCount,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetInspectorExecution {
    pub state: TargetInspectorExecutionState,
    pub queued_work_orders: i64,
    pub awaiting_producer_tasks: i64,
    pub running_attempts: i64,
    pub blocked_tasks: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetInspectorPatrol {
    pub state: TargetInspectorPatrolState,
    pub last_dispatched_at: Option<String>,
    pub last_succeeded_at: Option<String>,
    pub next_run_at: Option<String>,
    pub latest_hits: TargetInspectorCount,
    pub latest_new: TargetInspectorCount,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetInspectorCoverage {
    pub directory_works: TargetInspectorCount,
    pub captured_details: TargetInspectorCount,
    pub missing_details: TargetInspectorCount,
    pub quarantined_records: TargetInspectorCount,
    pub blocked_details: TargetInspectorCount,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetInspectorProjection {
    pub target_ref: Uuid,
    pub target_kind: String,
    pub as_of: String,
    pub archive: TargetInspectorArchive,
    pub execution: TargetInspectorExecution,
    pub patrol: TargetInspectorPatrol,
    pub coverage: TargetInspectorCoverage,
    pub required_action: TargetInspectorAction,
}

#[derive(Debug, thiserror::Error)]
pub enum TargetInspectorReadError {
    #[error("the target inspector projection schema is unavailable")]
    ProjectionUnavailable,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

struct TargetRow {
    target_ref: Uuid,
    target_kind: String,
    monitoring_enabled: bool,
    last_dispatched_at: Option<String>,
    last_succeeded_at: Option<String>,
    next_run_at: Option<String>,
}

#[derive(Default)]
struct LaneCounts {
    queued: i64,
    awaiting: i64,
    running: i64,
    blocked: i64,
}

struct ArchiveFacts {
    started: bool,
    attempted: bool,
    profiles: i64,
    works: i64,
    details: i64,
    /// 人已确认在平台上不存在的作品数。它们留在作品目录里，但不再是待补齐——
    /// 少了这一项，界面会一直催人去补一批永远补不到的作品。
    retired_works: i64,
    quarantined: i64,
    blocked_details: i64,
    standard_directory_ready: bool,
}

/// Read one target without acquiring material or persisting a derived status.
///
/// `None` means the target reference was not present at this snapshot. Schema drift is returned
/// separately so callers cannot turn an unreadable projection into a healthy or empty target.
pub async fn read_target_inspector(
    database: &Database,
    target_ref: Uuid,
) -> Result<Option<TargetInspectorProjection>, TargetInspectorReadError> {
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let as_of: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(&mut *tx)
        .await
        .map_err(map_schema_error)?;
    // 派发时刻与下次巡查住在**规则**上（`0078`）：一个关键词可以有几条口径各跑各的周期。
    // 检查器头部讲的是目标，所以这里把几条规则收成一句话——最近一次派发是它们里最近的，
    // 下次巡查是它们里最早的那个。
    let target = sqlx::query(
        "SELECT target.target_ref,target.target_kind,target.monitoring_enabled, \
                linggan_human_moment(rules.last_dispatched_at) AS last_patrol_dispatched_at, \
                linggan_human_moment(target.last_patrol_succeeded_at) AS last_patrol_succeeded_at, \
                CASE WHEN target.monitoring_enabled \
                     THEN linggan_human_moment(rules.next_run_at) END AS next_run_at \
         FROM collection_observation_target target \
         LEFT JOIN LATERAL ( \
             SELECT max(rule.last_patrol_dispatched_at) AS last_dispatched_at, \
                    min(rule.monitor_next_run_at) AS next_run_at \
               FROM collection_monitor_rule rule \
              WHERE rule.target_ref=target.target_ref AND rule.retired_at IS NULL) rules ON true \
         WHERE target.target_ref=$1 AND target.first_stored_at <= $2::timestamptz",
    )
    .bind(target_ref)
    .bind(&as_of)
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_schema_error)?
    .map(|row| TargetRow {
        target_ref: row.get("target_ref"),
        target_kind: row.get("target_kind"),
        monitoring_enabled: row.get("monitoring_enabled"),
        last_dispatched_at: row.get("last_patrol_dispatched_at"),
        last_succeeded_at: row.get("last_patrol_succeeded_at"),
        next_run_at: row.get("next_run_at"),
    });
    let Some(target) = target else {
        tx.commit().await?;
        return Ok(None);
    };

    let lane_rows = sqlx::query(EXECUTION_SQL)
        .bind(target.target_ref)
        .bind(&as_of)
        .fetch_all(&mut *tx)
        .await
        .map_err(map_schema_error)?;
    let mut archive_lane = LaneCounts::default();
    let mut patrol_lane = LaneCounts::default();
    for row in lane_rows {
        let counts = if row.get::<String, _>("lane") == "patrol" {
            &mut patrol_lane
        } else {
            &mut archive_lane
        };
        counts.queued = row.get("queued_work_orders");
        counts.awaiting = row.get("awaiting_producer_tasks");
        counts.running = row.get("running_attempts");
        counts.blocked = row.get("blocked_tasks");
    }
    let execution = execution_projection(&archive_lane);
    let facts = read_archive_facts(&mut tx, target.target_ref, &as_of)
        .await
        .map_err(map_schema_error)?;
    let (latest_hits, latest_new) =
        read_latest_patrol(&mut tx, target.target_ref, &target.target_kind, &as_of)
            .await
            .map_err(map_schema_error)?;
    tx.commit().await?;

    let (archive, coverage) = archive_projection(&target.target_kind, &facts, execution.state);
    let patrol = patrol_projection(&target, &patrol_lane, latest_hits, latest_new);
    let required_action =
        resolve_action(&archive, &coverage, &execution, target.monitoring_enabled);
    Ok(Some(TargetInspectorProjection {
        target_ref: target.target_ref,
        target_kind: target.target_kind,
        as_of,
        archive,
        execution,
        patrol,
        coverage,
        required_action,
    }))
}

fn execution_projection(counts: &LaneCounts) -> TargetInspectorExecution {
    let state = if counts.running > 0 {
        TargetInspectorExecutionState::Running
    } else if counts.awaiting > 0 {
        TargetInspectorExecutionState::AwaitingProducer
    } else if counts.queued > 0 {
        TargetInspectorExecutionState::Queued
    } else if counts.blocked > 0 {
        TargetInspectorExecutionState::Blocked
    } else {
        TargetInspectorExecutionState::Idle
    };
    TargetInspectorExecution {
        state,
        queued_work_orders: counts.queued,
        awaiting_producer_tasks: counts.awaiting,
        running_attempts: counts.running,
        blocked_tasks: counts.blocked,
    }
}

fn archive_projection(
    target_kind: &str,
    facts: &ArchiveFacts,
    execution: TargetInspectorExecutionState,
) -> (TargetInspectorArchive, TargetInspectorCoverage) {
    if target_kind != "creator" {
        return (
            TargetInspectorArchive {
                state: TargetInspectorArchiveState::NotApplicable,
                directory_state: TargetInspectorDirectoryState::NotApplicable,
                started: false,
                attempted: false,
                author_profile_captures: TargetInspectorCount::Unknown,
            },
            unknown_coverage(),
        );
    }
    let missing = facts
        .works
        .saturating_sub(facts.details)
        .saturating_sub(facts.retired_works);
    let historical = !facts.standard_directory_ready && facts.works > 0 && missing == 0;
    let directory_state = if facts.standard_directory_ready {
        TargetInspectorDirectoryState::Ready
    } else if historical {
        TargetInspectorDirectoryState::Historical
    } else if !facts.started && facts.works == 0 {
        TargetInspectorDirectoryState::NotStarted
    } else if matches!(
        execution,
        TargetInspectorExecutionState::Queued
            | TargetInspectorExecutionState::AwaitingProducer
            | TargetInspectorExecutionState::Running
    ) {
        TargetInspectorDirectoryState::Building
    } else {
        TargetInspectorDirectoryState::RebuildRequired
    };
    let state = match execution {
        TargetInspectorExecutionState::Running => TargetInspectorArchiveState::Running,
        TargetInspectorExecutionState::Queued | TargetInspectorExecutionState::AwaitingProducer => {
            TargetInspectorArchiveState::Queued
        }
        _ if facts.quarantined > 0 || facts.blocked_details > 0 => {
            TargetInspectorArchiveState::Blocked
        }
        _ if !facts.started && facts.works == 0 => TargetInspectorArchiveState::NotStarted,
        _ if matches!(
            directory_state,
            TargetInspectorDirectoryState::Ready | TargetInspectorDirectoryState::Historical
        ) && missing == 0 =>
        {
            TargetInspectorArchiveState::Complete
        }
        _ => TargetInspectorArchiveState::Partial,
    };
    (
        TargetInspectorArchive {
            state,
            directory_state,
            started: facts.started,
            attempted: facts.attempted,
            author_profile_captures: TargetInspectorCount::Known(facts.profiles),
        },
        TargetInspectorCoverage {
            directory_works: TargetInspectorCount::Known(facts.works),
            captured_details: TargetInspectorCount::Known(facts.details),
            missing_details: TargetInspectorCount::Known(missing),
            quarantined_records: TargetInspectorCount::Known(facts.quarantined),
            blocked_details: TargetInspectorCount::Known(facts.blocked_details),
        },
    )
}

fn unknown_coverage() -> TargetInspectorCoverage {
    TargetInspectorCoverage {
        directory_works: TargetInspectorCount::Unknown,
        captured_details: TargetInspectorCount::Unknown,
        missing_details: TargetInspectorCount::Unknown,
        quarantined_records: TargetInspectorCount::Unknown,
        blocked_details: TargetInspectorCount::Unknown,
    }
}

fn patrol_projection(
    target: &TargetRow,
    counts: &LaneCounts,
    latest_hits: TargetInspectorCount,
    latest_new: TargetInspectorCount,
) -> TargetInspectorPatrol {
    let state = if !target.monitoring_enabled {
        TargetInspectorPatrolState::Disabled
    } else if counts.running > 0 {
        TargetInspectorPatrolState::Running
    } else if counts.awaiting > 0 {
        TargetInspectorPatrolState::AwaitingProducer
    } else if counts.queued > 0 {
        TargetInspectorPatrolState::Queued
    } else if counts.blocked > 0 {
        TargetInspectorPatrolState::Blocked
    } else if matches!(latest_hits, TargetInspectorCount::Known(_)) {
        TargetInspectorPatrolState::Normal
    } else {
        TargetInspectorPatrolState::Waiting
    };
    TargetInspectorPatrol {
        state,
        last_dispatched_at: target.last_dispatched_at.clone(),
        last_succeeded_at: target.last_succeeded_at.clone(),
        next_run_at: target.next_run_at.clone(),
        latest_hits,
        latest_new,
    }
}

fn resolve_action(
    archive: &TargetInspectorArchive,
    coverage: &TargetInspectorCoverage,
    execution: &TargetInspectorExecution,
    monitoring_enabled: bool,
) -> TargetInspectorAction {
    // Execution ownership and archive incompleteness are independent facts. A running worker
    // cannot erase the human's route to quarantined or blocked material.
    if archive.state == TargetInspectorArchiveState::Blocked
        || matches!(coverage.quarantined_records, TargetInspectorCount::Known(value) if value > 0)
        || matches!(coverage.missing_details, TargetInspectorCount::Known(value) if value > 0)
    {
        return TargetInspectorAction::HandleArchiveProblems;
    }
    match execution.state {
        TargetInspectorExecutionState::Running => return TargetInspectorAction::NoActionRunning,
        TargetInspectorExecutionState::Queued | TargetInspectorExecutionState::AwaitingProducer => {
            return TargetInspectorAction::NoActionQueued;
        }
        _ => {}
    }
    match archive.directory_state {
        TargetInspectorDirectoryState::NotApplicable => TargetInspectorAction::NoActionHealthy,
        TargetInspectorDirectoryState::NotStarted => TargetInspectorAction::StartArchive,
        TargetInspectorDirectoryState::RebuildRequired => TargetInspectorAction::RebuildDirectory,
        _ if matches!(coverage.missing_details, TargetInspectorCount::Known(value) if value > 0) => {
            TargetInspectorAction::ContinueArchive
        }
        _ if !monitoring_enabled => TargetInspectorAction::EnablePatrol,
        _ => TargetInspectorAction::NoActionHealthy,
    }
}

fn map_schema_error(error: sqlx::Error) -> TargetInspectorReadError {
    match &error {
        sqlx::Error::Database(error)
            if matches!(error.code().as_deref(), Some("42P01" | "42703")) =>
        {
            TargetInspectorReadError::ProjectionUnavailable
        }
        _ => TargetInspectorReadError::Database(error),
    }
}

async fn read_archive_facts(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_ref: Uuid,
    as_of: &str,
) -> Result<ArchiveFacts, sqlx::Error> {
    let row = sqlx::query(ARCHIVE_SQL)
        .bind(target_ref)
        .bind(as_of)
        .fetch_one(&mut **tx)
        .await?;
    Ok(ArchiveFacts {
        started: row.get("started"),
        attempted: row.get("attempted"),
        profiles: row.get("profiles"),
        works: row.get("works"),
        details: row.get("details"),
        retired_works: row.get("retired_works"),
        quarantined: row.get("quarantined"),
        blocked_details: row.get("blocked_details"),
        standard_directory_ready: row.get("standard_directory_ready"),
    })
}

async fn read_latest_patrol(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_ref: Uuid,
    target_kind: &str,
    as_of: &str,
) -> Result<(TargetInspectorCount, TargetInspectorCount), sqlx::Error> {
    let row = sqlx::query(LATEST_PATROL_SQL)
        .bind(target_ref)
        .bind(target_kind)
        .bind(as_of)
        .fetch_optional(&mut **tx)
        .await?;
    Ok(match row {
        Some(row) => (
            TargetInspectorCount::Known(row.get("hits")),
            TargetInspectorCount::Known(row.get("newly_discovered")),
        ),
        None => (TargetInspectorCount::Unknown, TargetInspectorCount::Unknown),
    })
}

#[cfg(test)]
#[path = "target_inspector_tests.rs"]
mod tests;
