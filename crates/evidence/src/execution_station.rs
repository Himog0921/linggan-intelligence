//! COLLECTION-001 · 工位登记与插件安装认领。
//!
//! 核心分工：**工位由人登记，安装由插件自报。** 授权、额度、名字都挂在工位上，
//! 因此插件重装不会让它们消失，也不需要重做。
//!
//! 这里没有任何平台访问。一条在岗安装只说明「有个插件报到了」，不说明它跑过任何东西。

use crate::collection_control::{
    IssuedInstallationCredential, MINIMUM_PLUGIN_VERSION, authenticate_installation_check_in_in,
    issue_installation_credential_if_absent_in, version_at_least,
};
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
    #[error("station display name must not be blank")]
    InvalidDisplayName,
    #[error(transparent)]
    Control(#[from] crate::collection_control::CollectionControlError),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// 一次安装报到的结果。
#[derive(Debug)]
pub enum CheckInOutcome {
    /// 认领窗口开着，自动绑到工位；同工位上一个安装被取代。
    Claimed {
        installation_ref: Uuid,
        station_ref: Uuid,
        station_display_name: String,
        accepting_tasks: bool,
        superseded: Option<Uuid>,
        credential: Option<IssuedInstallationCredential>,
    },
    /// 插件在，但还没人说它是哪台工位。它不会被派活。
    AwaitingClaim {
        installation_ref: Uuid,
        credential: Option<IssuedInstallationCredential>,
    },
    /// 同一个安装再次报到，只更新心跳。
    Heartbeat {
        installation_ref: Uuid,
        station_ref: Option<Uuid>,
        station_display_name: Option<String>,
        accepting_tasks: Option<bool>,
        credential: Option<IssuedInstallationCredential>,
    },
}

/// The server-confirmed outcome of a person matching one waiting installation
/// to one station. The plugin never supplies this name and cannot use it as an
/// identity key; it is the durable station name that both surfaces display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallationClaimOutcome {
    pub superseded: Option<Uuid>,
    pub station_display_name: String,
    pub accepting_tasks: bool,
}

pub async fn station_schema_is_ready(database: &Database) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, bool>(
        "SELECT to_regclass('execution_station') IS NOT NULL \
             AND to_regclass('plugin_installation') IS NOT NULL \
             AND to_regclass('installation_credential') IS NOT NULL",
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
    let display_name = display_name.trim();
    if display_name.is_empty() {
        return Err(StationError::InvalidDisplayName);
    }
    let station_ref = Uuid::new_v4();
    let mut transaction = database.pool().begin().await?;
    sqlx::query(
        "INSERT INTO execution_station (station_ref, display_name, daily_work_quota) \
         VALUES ($1, $2, $3)",
    )
    .bind(station_ref)
    .bind(display_name)
    .bind(daily_work_quota)
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        "INSERT INTO execution_station_acceptance_transition \
             (transition_ref,station_ref,from_accepting,to_accepting,actor,reason_code) \
         VALUES ($1,$2,NULL,false,'person','registered_awaiting_claim')",
    )
    .bind(Uuid::new_v4())
    .bind(station_ref)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
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
    let previous_accepting: Option<bool> = sqlx::query_scalar(
        "SELECT accepting_tasks FROM execution_station \
         WHERE station_ref=$1 AND retired_at IS NULL FOR UPDATE",
    )
    .bind(station_ref)
    .fetch_optional(&mut *transaction)
    .await?;
    let Some(previous_accepting) = previous_accepting else {
        return Err(StationError::UnknownStation);
    };
    let affected = sqlx::query(
        "UPDATE execution_station \
         SET retired_at = scope_001_now(), retire_reason = $2, \
             claim_window_opens_at = NULL, claim_window_expires_at = NULL, \
             accepting_tasks=false \
         WHERE station_ref = $1 AND retired_at IS NULL",
    )
    .bind(station_ref)
    .bind(reason)
    .execute(&mut *transaction)
    .await?
    .rows_affected();
    debug_assert_eq!(affected, 1);
    sqlx::query(
        "INSERT INTO execution_station_acceptance_transition \
             (transition_ref,station_ref,from_accepting,to_accepting,actor,reason_code) \
         VALUES ($1,$2,$3,false,'person','station_retired')",
    )
    .bind(Uuid::new_v4())
    .bind(station_ref)
    .bind(previous_accepting)
    .execute(&mut *transaction)
    .await?;
    // 安装记录保留，只解除在岗关系：插件历史不因工位停用而消失。
    let retired_installations: Vec<Uuid> = sqlx::query_scalar(
        "UPDATE plugin_installation \
         SET superseded_at = scope_001_now(), superseded_by = installation_ref \
         WHERE station_ref = $1 AND superseded_at IS NULL RETURNING installation_ref",
    )
    .bind(station_ref)
    .fetch_all(&mut *transaction)
    .await?;
    for installation_ref in retired_installations {
        teardown_installation(&mut transaction, installation_ref, "station_retired").await?;
    }
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
    /// Required once this installation has an activated credential. It is omitted only for a
    /// first bootstrap or replacement of an unactivated pending issuance.
    pub installation_credential: Option<&'a str>,
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
        if !authenticate_installation_check_in_in(
            &mut transaction,
            installation_ref,
            check_in.installation_credential,
        )
        .await?
        {
            return Err(
                crate::collection_control::CollectionControlError::InvalidCredential.into(),
            );
        }
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
        let credential = issue_compatible_credential(
            &mut transaction,
            installation_ref,
            check_in.plugin_version,
        )
        .await?;
        // 已归位的安装只更新心跳。**未归位的必须再试一次认领**：它上次报到时窗口可能
        // 还关着，之后人才把窗口打开。不重试的话，这个安装会永远停在待认领——而使用者
        // 看到的是「窗口开着，插件却始终不归位」，无从判断哪里出了问题。
        if station_ref.is_none()
            && let Some(mut open_station) = open_claim_station(&mut transaction).await?
        {
            let superseded = supersede_active_installation(
                &mut transaction,
                open_station.station_ref,
                installation_ref,
            )
            .await?;
            sqlx::query(
                "UPDATE plugin_installation \
                 SET station_ref = $2, claim_kind = 'claim_window', claimed_at = scope_001_now() \
                 WHERE installation_ref = $1",
            )
            .bind(installation_ref)
            .bind(open_station.station_ref)
            .execute(&mut *transaction)
            .await?;
            open_station.accepting_tasks =
                enable_default_acceptance_after_claim(&mut transaction, open_station.station_ref)
                    .await?;
            transaction.commit().await?;
            return Ok(CheckInOutcome::Claimed {
                installation_ref,
                station_ref: open_station.station_ref,
                station_display_name: open_station.display_name,
                accepting_tasks: open_station.accepting_tasks,
                superseded,
                credential,
            });
        }
        let station = match station_ref {
            Some(station_ref) => Some(read_active_station(&mut transaction, station_ref).await?),
            None => None,
        };
        transaction.commit().await?;
        return Ok(CheckInOutcome::Heartbeat {
            installation_ref,
            station_ref: station.as_ref().map(|station| station.station_ref),
            station_display_name: station.as_ref().map(|station| station.display_name.clone()),
            accepting_tasks: station.as_ref().map(|station| station.accepting_tasks),
            credential,
        });
    }

    let open_station = open_claim_station(&mut transaction).await?;

    let installation_ref = Uuid::new_v4();
    let outcome = match open_station {
        Some(mut station) => {
            let superseded = supersede_active_installation(
                &mut transaction,
                station.station_ref,
                installation_ref,
            )
            .await?;
            insert_installation(
                &mut transaction,
                installation_ref,
                check_in,
                Some(station.station_ref),
            )
            .await?;
            let credential = issue_compatible_credential(
                &mut transaction,
                installation_ref,
                check_in.plugin_version,
            )
            .await?;
            station.accepting_tasks =
                enable_default_acceptance_after_claim(&mut transaction, station.station_ref)
                    .await?;
            CheckInOutcome::Claimed {
                installation_ref,
                station_ref: station.station_ref,
                station_display_name: station.display_name,
                accepting_tasks: station.accepting_tasks,
                superseded,
                credential,
            }
        }
        None => {
            insert_installation(&mut transaction, installation_ref, check_in, None).await?;
            let credential = issue_compatible_credential(
                &mut transaction,
                installation_ref,
                check_in.plugin_version,
            )
            .await?;
            CheckInOutcome::AwaitingClaim {
                installation_ref,
                credential,
            }
        }
    };
    transaction.commit().await?;
    Ok(outcome)
}

async fn issue_compatible_credential(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    installation_ref: Uuid,
    plugin_version: &str,
) -> Result<Option<IssuedInstallationCredential>, StationError> {
    if !version_at_least(plugin_version, MINIMUM_PLUGIN_VERSION) {
        return Ok(None);
    }
    Ok(issue_installation_credential_if_absent_in(transaction, installation_ref).await?)
}

#[derive(Debug, Clone)]
struct ActiveStation {
    station_ref: Uuid,
    display_name: String,
    accepting_tasks: bool,
}

/// 找一台认领窗口还开着的工位。窗口是人开的，过期自动关上。
async fn open_claim_station(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<Option<ActiveStation>, sqlx::Error> {
    let row: Option<(Uuid, String, bool)> = sqlx::query_as(
        "SELECT station_ref,display_name,accepting_tasks FROM execution_station \
         WHERE retired_at IS NULL AND claim_window_expires_at > scope_001_now() \
         ORDER BY claim_window_opens_at DESC LIMIT 1 FOR UPDATE",
    )
    .fetch_optional(&mut **transaction)
    .await?;
    Ok(row.map(
        |(station_ref, display_name, accepting_tasks)| ActiveStation {
            station_ref,
            display_name,
            accepting_tasks,
        },
    ))
}

/// 人手动把一个待认领的安装指到某台工位。窗口关着时走这条路。
pub async fn claim_installation(
    database: &Database,
    installation_ref: Uuid,
    station_ref: Uuid,
) -> Result<InstallationClaimOutcome, StationError> {
    if !station_schema_is_ready(database).await? {
        return Err(StationError::SchemaUnavailable);
    }
    let mut transaction = database.pool().begin().await?;

    let station: Option<(String, bool, bool)> = sqlx::query_as(
        "SELECT display_name,accepting_tasks,retired_at IS NOT NULL \
         FROM execution_station WHERE station_ref = $1 FOR UPDATE",
    )
    .bind(station_ref)
    .fetch_optional(&mut *transaction)
    .await?;
    let (display_name, accepting_tasks, _retired) = match station {
        None => return Err(StationError::UnknownStation),
        Some((_, _, true)) => return Err(StationError::StationRetired),
        Some(station) => station,
    };

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
    let accepting_tasks = if accepting_tasks {
        true
    } else {
        enable_default_acceptance_after_claim(&mut transaction, station_ref).await?
    };
    transaction.commit().await?;
    Ok(InstallationClaimOutcome {
        superseded,
        station_display_name: display_name,
        accepting_tasks,
    })
}

/// A server-owned station name may be changed by a person so that the Runtime
/// page and its matched plugin panel stay recognisable. The installation is
/// deliberately not involved: it has no authority over station identity.
pub async fn rename_station(
    database: &Database,
    station_ref: Uuid,
    display_name: &str,
) -> Result<(), StationError> {
    if !station_schema_is_ready(database).await? {
        return Err(StationError::SchemaUnavailable);
    }
    let display_name = display_name.trim();
    if display_name.is_empty() {
        return Err(StationError::InvalidDisplayName);
    }
    let affected = sqlx::query(
        "UPDATE execution_station SET display_name=$2 \
         WHERE station_ref=$1 AND retired_at IS NULL",
    )
    .bind(station_ref)
    .bind(display_name)
    .execute(database.pool())
    .await?
    .rows_affected();
    if affected == 0 {
        return Err(StationError::UnknownStation);
    }
    Ok(())
}

/// Enables the automatic default only when the last acceptance fact is the
/// pre-claim default. `person_disabled` stays false across every later
/// heartbeat, replacement or reinstallation.
async fn enable_default_acceptance_after_claim(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    station_ref: Uuid,
) -> Result<bool, sqlx::Error> {
    let state: Option<(bool, Option<String>)> = sqlx::query_as(
        "SELECT station.accepting_tasks,( \
             SELECT transition.reason_code \
             FROM execution_station_acceptance_transition transition \
             WHERE transition.station_ref=station.station_ref \
             ORDER BY transition.occurred_at DESC,transition.transition_ref DESC LIMIT 1 \
         ) AS latest_reason \
         FROM execution_station station \
         WHERE station.station_ref=$1 AND station.retired_at IS NULL FOR UPDATE",
    )
    .bind(station_ref)
    .fetch_optional(&mut **transaction)
    .await?;
    let Some((accepting_tasks, latest_reason)) = state else {
        return Ok(false);
    };
    if accepting_tasks {
        return Ok(true);
    }
    if !matches!(
        latest_reason.as_deref(),
        Some("registered_closed") | Some("registered_awaiting_claim") | Some("migration_closed")
    ) {
        return Ok(false);
    }
    sqlx::query("UPDATE execution_station SET accepting_tasks=true WHERE station_ref=$1")
        .bind(station_ref)
        .execute(&mut **transaction)
        .await?;
    sqlx::query(
        "INSERT INTO execution_station_acceptance_transition \
             (transition_ref,station_ref,from_accepting,to_accepting,actor,reason_code) \
         VALUES ($1,$2,false,true,'system','installation_claimed_auto_enabled')",
    )
    .bind(Uuid::new_v4())
    .bind(station_ref)
    .execute(&mut **transaction)
    .await?;
    Ok(true)
}

async fn read_active_station(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    station_ref: Uuid,
) -> Result<ActiveStation, sqlx::Error> {
    let (station_ref, display_name, accepting_tasks): (Uuid, String, bool) = sqlx::query_as(
        "SELECT station_ref,display_name,accepting_tasks FROM execution_station \
         WHERE station_ref=$1 AND retired_at IS NULL",
    )
    .bind(station_ref)
    .fetch_one(&mut **transaction)
    .await?;
    Ok(ActiveStation {
        station_ref,
        display_name,
        accepting_tasks,
    })
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
    if let Some(previous) = previous {
        // A replacement never inherits account identity or live execution ownership. The old
        // Lease is ended and ordinary recovery must establish a fresh Admission/Lease against
        // a newly person-confirmed account binding.
        teardown_installation(transaction, previous, "installation_superseded").await?;
    }
    Ok(previous)
}

async fn teardown_installation(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    installation_ref: Uuid,
    reason_code: &str,
) -> Result<(), sqlx::Error> {
    let credential_reason = if reason_code == "station_retired" {
        "station_retired"
    } else {
        "installation_superseded"
    };
    sqlx::query(
        "UPDATE installation_credential SET revoked_at=scope_001_now(), \
                revoke_reason_code=$2 \
         WHERE installation_ref=$1 AND revoked_at IS NULL",
    )
    .bind(installation_ref)
    .bind(credential_reason)
    .execute(&mut **transaction)
    .await?;
    sqlx::query(
        "UPDATE platform_observation_account_binding \
         SET ended_at=scope_001_now(),end_reason_code=$2 \
         WHERE installation_ref=$1 AND ended_at IS NULL",
    )
    .bind(installation_ref)
    .bind(reason_code)
    .execute(&mut **transaction)
    .await?;
    // 释放租约与交还工单必须一起做。只释放租约会把工单永久留在 `leased` 且名下无活租约，
    // 共享 claim 只找 `queued`，于是没有任何工位能再接手它，也没有巡检会回收它。
    let released: Vec<Uuid> = sqlx::query_scalar(
        "UPDATE collection_work_order_lease lease \
         SET released_at=scope_001_now(),release_reason='station_unavailable' \
         FROM collection_work_order work_order \
         WHERE lease.work_order_ref=work_order.work_order_ref \
           AND work_order.installation_ref=$1 \
           AND lease.released_at IS NULL \
         RETURNING lease.work_order_ref",
    )
    .bind(installation_ref)
    .fetch_all(&mut **transaction)
    .await?;
    crate::work_order_lease::requeue_work_orders_after_release(transaction, &released).await?;
    Ok(())
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
