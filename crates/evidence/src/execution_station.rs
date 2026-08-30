//! COLLECTION-001 · 工位登记与插件安装认领。
//!
//! 核心分工：**工位由人登记，安装由插件自报。** 授权、额度、名字都挂在工位上，
//! 因此插件重装不会让它们消失，也不需要重做。
//!
//! 这里没有任何平台访问。一条在岗安装只说明「有个插件报到了」，不说明它跑过任何东西。

use linggan_storage_postgres::Database;
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum StationError {
    #[error("execution station schema is not applied")]
    SchemaUnavailable,
    #[error("no execution station with that reference")]
    UnknownStation,
    #[error("no plugin installation with that reference")]
    UnknownInstallation,
    #[error("that station is retired")]
    StationRetired,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// 一次安装报到的结果。
#[derive(Debug, PartialEq, Eq)]
pub enum CheckInOutcome {
    /// 认领窗口开着，自动绑到工位；同工位上一个安装被取代。
    Claimed {
        installation_ref: Uuid,
        station_ref: Uuid,
        superseded: Option<Uuid>,
    },
    /// 插件在，但还没人说它是哪台工位。它不会被派活。
    AwaitingClaim { installation_ref: Uuid },
    /// 同一个安装再次报到，只更新心跳。
    Heartbeat { installation_ref: Uuid },
}

pub async fn station_schema_is_ready(database: &Database) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, bool>(
        "SELECT to_regclass('execution_station') IS NOT NULL \
             AND to_regclass('plugin_installation') IS NOT NULL",
    )
    .fetch_one(database.pool())
    .await
}

/// 人登记一台工位。这是唯一创建工位的入口——插件永远不能创建工位。
pub async fn register_station(
    database: &Database,
    display_name: &str,
    daily_work_quota: i32,
) -> Result<Uuid, StationError> {
    if !station_schema_is_ready(database).await? {
        return Err(StationError::SchemaUnavailable);
    }
    let station_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO execution_station (station_ref, display_name, daily_work_quota) \
         VALUES ($1, $2, $3)",
    )
    .bind(station_ref)
    .bind(display_name)
    .bind(daily_work_quota)
    .execute(database.pool())
    .await?;
    Ok(station_ref)
}

/// 开一个认领窗口。窗口期内新安装自动绑到这台工位。
///
/// 必须给时长：开发期开一天，比每次重装点一次确认省事；而不会过期的窗口在公网域名下
/// 就是一扇一直开着的门，跟没有认领控制没区别。
pub async fn open_claim_window(
    database: &Database,
    station_ref: Uuid,
    valid_for_hours: i32,
) -> Result<(), StationError> {
    if !station_schema_is_ready(database).await? {
        return Err(StationError::SchemaUnavailable);
    }
    let affected = sqlx::query(
        "UPDATE execution_station \
         SET claim_window_opens_at = scope_001_now(), \
             claim_window_expires_at = scope_001_now() + make_interval(hours => $2) \
         WHERE station_ref = $1 AND retired_at IS NULL",
    )
    .bind(station_ref)
    .bind(valid_for_hours)
    .execute(database.pool())
    .await?
    .rows_affected();
    if affected == 0 {
        return Err(StationError::UnknownStation);
    }
    Ok(())
}

/// 提前关掉认领窗口。
///
/// 窗口本来就会自己过期，但「开了想撤」必须有退路：一个能开不能关的控制只是半个控制。
pub async fn close_claim_window(
    database: &Database,
    station_ref: Uuid,
) -> Result<(), StationError> {
    if !station_schema_is_ready(database).await? {
        return Err(StationError::SchemaUnavailable);
    }
    let affected = sqlx::query(
        "UPDATE execution_station \
         SET claim_window_opens_at = NULL, claim_window_expires_at = NULL \
         WHERE station_ref = $1 AND retired_at IS NULL",
    )
    .bind(station_ref)
    .execute(database.pool())
    .await?
    .rows_affected();
    if affected == 0 {
        return Err(StationError::UnknownStation);
    }
    Ok(())
}

/// 停用一台工位。
///
/// 不是删除：停用要留下时间与原因，否则「这台工位为什么不见了」以后没人答得上来。
/// 停用会同时把它上面的在岗安装标记为被取代——否则那条安装会挂在一台已经不存在的
/// 工位上，而唯一索引仍认为它在岗。
pub async fn retire_station(
    database: &Database,
    station_ref: Uuid,
    reason: &str,
) -> Result<(), StationError> {
    if !station_schema_is_ready(database).await? {
        return Err(StationError::SchemaUnavailable);
    }
    let mut transaction = database.pool().begin().await?;
    let affected = sqlx::query(
        "UPDATE execution_station \
         SET retired_at = scope_001_now(), retire_reason = $2, \
             claim_window_opens_at = NULL, claim_window_expires_at = NULL \
         WHERE station_ref = $1 AND retired_at IS NULL",
    )
    .bind(station_ref)
    .bind(reason)
    .execute(&mut *transaction)
    .await?
    .rows_affected();
    if affected == 0 {
        return Err(StationError::UnknownStation);
    }
    // 安装记录保留，只解除在岗关系：插件历史不因工位停用而消失。
    sqlx::query(
        "UPDATE plugin_installation \
         SET superseded_at = scope_001_now(), superseded_by = installation_ref \
         WHERE station_ref = $1 AND superseded_at IS NULL",
    )
    .bind(station_ref)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(())
}

/// 插件报到自述。全部是插件自报的兼容性事实（合同 §5），不含任何它能改变的配额或范围。
#[derive(Debug, Clone)]
pub struct InstallationCheckIn<'a> {
    /// 插件端自己生成的标识。**它变了只说明插件重装过，不说明这是另一台机器**——
    /// 因此这里既不唯一也不作为工位身份。
    pub install_key: &'a str,
    pub plugin_version: &'a str,
    pub browser_label: Option<&'a str>,
    pub capabilities: Value,
}

/// 插件报到。
///
/// 三条路径：已在岗的同一安装 → 心跳；有开着的认领窗口 → 绑上并取代旧安装；
/// 否则 → 待认领，不派活。
pub async fn check_in_installation(
    database: &Database,
    check_in: &InstallationCheckIn<'_>,
) -> Result<CheckInOutcome, StationError> {
    if !station_schema_is_ready(database).await? {
        return Err(StationError::SchemaUnavailable);
    }
    let mut transaction = database.pool().begin().await?;

    // 同一个 install_key 且仍在岗 —— 插件只是又报了一次到。
    let existing: Option<(Uuid, Option<Uuid>)> = sqlx::query_as(
        "SELECT installation_ref, station_ref FROM plugin_installation \
         WHERE install_key = $1 AND superseded_at IS NULL \
         ORDER BY first_seen_at DESC LIMIT 1 FOR UPDATE",
    )
    .bind(check_in.install_key)
    .fetch_optional(&mut *transaction)
    .await?;
    if let Some((installation_ref, station_ref)) = existing {
        sqlx::query(
            "UPDATE plugin_installation \
             SET last_seen_at = scope_001_now(), plugin_version = $2, capabilities = $3 \
             WHERE installation_ref = $1",
        )
        .bind(installation_ref)
        .bind(check_in.plugin_version)
        .bind(&check_in.capabilities)
        .execute(&mut *transaction)
        .await?;
        if let Some(station_ref) = station_ref {
            adopt_predecessor_live_claims(&mut transaction, station_ref, installation_ref).await?;
        }
        // 已归位的安装只更新心跳。**未归位的必须再试一次认领**：它上次报到时窗口可能
        // 还关着，之后人才把窗口打开。不重试的话，这个安装会永远停在待认领——而使用者
        // 看到的是「窗口开着，插件却始终不归位」，无从判断哪里出了问题。
        if station_ref.is_none()
            && let Some(open_station) = open_claim_station(&mut transaction).await?
        {
            let superseded =
                supersede_active_installation(&mut transaction, open_station, installation_ref)
                    .await?;
            sqlx::query(
                "UPDATE plugin_installation \
                 SET station_ref = $2, claim_kind = 'claim_window', claimed_at = scope_001_now() \
                 WHERE installation_ref = $1",
            )
            .bind(installation_ref)
            .bind(open_station)
            .execute(&mut *transaction)
            .await?;
            adopt_predecessor_live_claims(&mut transaction, open_station, installation_ref).await?;
            transaction.commit().await?;
            return Ok(CheckInOutcome::Claimed {
                installation_ref,
                station_ref: open_station,
                superseded,
            });
        }
        transaction.commit().await?;
        return Ok(CheckInOutcome::Heartbeat { installation_ref });
    }

    let open_station = open_claim_station(&mut transaction).await?;

    let installation_ref = Uuid::new_v4();
    let outcome = match open_station {
        Some(station_ref) => {
            let superseded =
                supersede_active_installation(&mut transaction, station_ref, installation_ref)
                    .await?;
            insert_installation(
                &mut transaction,
                installation_ref,
                check_in,
                Some(station_ref),
            )
            .await?;
            adopt_predecessor_live_claims(&mut transaction, station_ref, installation_ref).await?;
            CheckInOutcome::Claimed {
                installation_ref,
                station_ref,
                superseded,
            }
        }
        None => {
            insert_installation(&mut transaction, installation_ref, check_in, None).await?;
            CheckInOutcome::AwaitingClaim { installation_ref }
        }
    };
    transaction.commit().await?;
    Ok(outcome)
}

/// 找一台认领窗口还开着的工位。窗口是人开的，过期自动关上。
async fn open_claim_station(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT station_ref FROM execution_station \
         WHERE retired_at IS NULL AND claim_window_expires_at > scope_001_now() \
         ORDER BY claim_window_opens_at DESC LIMIT 1 FOR UPDATE",
    )
    .fetch_optional(&mut **transaction)
    .await
}

/// 人手动把一个待认领的安装指到某台工位。窗口关着时走这条路。
pub async fn claim_installation(
    database: &Database,
    installation_ref: Uuid,
    station_ref: Uuid,
) -> Result<Option<Uuid>, StationError> {
    if !station_schema_is_ready(database).await? {
        return Err(StationError::SchemaUnavailable);
    }
    let mut transaction = database.pool().begin().await?;

    let retired: Option<bool> = sqlx::query_scalar(
        "SELECT retired_at IS NOT NULL FROM execution_station WHERE station_ref = $1 FOR UPDATE",
    )
    .bind(station_ref)
    .fetch_optional(&mut *transaction)
    .await?;
    match retired {
        None => return Err(StationError::UnknownStation),
        Some(true) => return Err(StationError::StationRetired),
        Some(false) => {}
    }

    let superseded =
        supersede_active_installation(&mut transaction, station_ref, installation_ref).await?;
    let affected = sqlx::query(
        "UPDATE plugin_installation \
         SET station_ref = $2, claim_kind = 'person', claimed_at = scope_001_now() \
         WHERE installation_ref = $1 AND superseded_at IS NULL",
    )
    .bind(installation_ref)
    .bind(station_ref)
    .execute(&mut *transaction)
    .await?
    .rows_affected();
    if affected == 0 {
        return Err(StationError::UnknownInstallation);
    }
    adopt_predecessor_live_claims(&mut transaction, station_ref, installation_ref).await?;
    transaction.commit().await?;
    Ok(superseded)
}

/// 把该工位当前在岗的安装标记为被取代。
///
/// 取代是记下来的事实，不是删除：「这台工位换过 12 次插件」必须看得见，否则它就会以
/// 12 台僵尸工位的形式呈现，而那正是内容工作台踩过的坑。
async fn supersede_active_installation(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    station_ref: Uuid,
    replacement: Uuid,
) -> Result<Option<Uuid>, sqlx::Error> {
    let previous: Option<Uuid> = sqlx::query_scalar(
        "UPDATE plugin_installation \
         SET superseded_at = scope_001_now(), superseded_by = $2 \
         WHERE station_ref = $1 AND superseded_at IS NULL \
         RETURNING installation_ref",
    )
    .bind(station_ref)
    .bind(replacement)
    .fetch_optional(&mut **transaction)
    .await?;
    Ok(previous)
}

/// 新安装已经在同一工位取代旧安装时，承接这个工位所有已被取代安装尚在有效租约内的
/// 执行权。不能只看直接前任：连续重载时，任务所有者可能仍停在更早一代安装。
/// 旧安装已标记 superseded，Attempt/Submission 闸门会拒绝它继续改任务状态；这里只转移
/// 所有权，不重建 Task，不改已完成步骤。
async fn adopt_predecessor_live_claims(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    station_ref: Uuid,
    replacement: Uuid,
) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query(
        "UPDATE collection_work_order_lease_task task \
         SET claimed_by_installation_ref=$2,claimed_at=scope_001_now() \
         FROM plugin_installation previous,collection_work_order_lease lease \
         WHERE task.claimed_by_installation_ref=previous.installation_ref \
           AND previous.station_ref=$1 AND previous.superseded_at IS NOT NULL \
           AND previous.installation_ref<>$2 \
           AND task.lease_ref=lease.lease_ref \
           AND task.execution_state='in_progress' \
           AND lease.released_at IS NULL AND lease.expires_at>scope_001_now()",
    )
    .bind(station_ref)
    .bind(replacement)
    .execute(&mut **transaction)
    .await?
    .rows_affected())
}

async fn insert_installation(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    installation_ref: Uuid,
    check_in: &InstallationCheckIn<'_>,
    station_ref: Option<Uuid>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO plugin_installation \
             (installation_ref, install_key, station_ref, claim_kind, claimed_at, \
              plugin_version, browser_label, capabilities) \
         VALUES ($1, $2, $3, \
                 CASE WHEN $3::uuid IS NULL THEN NULL ELSE 'claim_window' END, \
                 CASE WHEN $3::uuid IS NULL THEN NULL ELSE scope_001_now() END, \
                 $4, $5, $6)",
    )
    .bind(installation_ref)
    .bind(check_in.install_key)
    .bind(station_ref)
    .bind(check_in.plugin_version)
    .bind(check_in.browser_label)
    .bind(&check_in.capabilities)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}
