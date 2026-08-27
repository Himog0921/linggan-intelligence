//! COLLECTION-001 · 工位的读投影与每日配额。
//!
//! 与登记/认领分开，因为它们回答的是不同的问题：那边写「谁是谁」，这边只读「现在是什么
//! 样子」。配额判据也放在这里——展示与准入读同一段 SQL，两处口径才不会漂移。

use crate::execution_station::{StationError, station_schema_is_ready};
use linggan_storage_postgres::Database;
use uuid::Uuid;

/// 一台工位当天已入库的笔记条数。
///
/// **配额的权威实现只有这一个函数**，准入判定与页面展示都读它。旧项目有两份 200——一份
/// 管派单硬编码、一份管页面显示读插件上报值，判据还不同，于是出现「页面显示已达上限但
/// 仍在派单」。两处口径必然漂移，只留一处才不会。
///
/// 计数单位是**实际入库的笔记条数**，不是任务数：一次建档可能入库 200 条却只算一个任务，
/// 按任务数计会算错（规则文档 §单工位每日上限）。隔离与未解释的记录不计入——它们没有
/// 成为可用材料，占用配额就等于让失败的采集吃掉当天的额度。
///
/// 窗口按 **Asia/Shanghai 自然日**，与规则文档一致。
pub async fn station_daily_note_usage(
    database: &Database,
    station_ref: Uuid,
) -> Result<i64, StationError> {
    if !station_schema_is_ready(database).await? {
        return Err(StationError::SchemaUnavailable);
    }
    let used: Option<i64> = sqlx::query_scalar(DAILY_NOTE_USAGE_SQL)
        .bind(station_ref)
        .fetch_one(database.pool())
        .await?;
    Ok(used.unwrap_or(0))
}

/// 同一段查询的事务内入口。准入判定在事务里跑，不能另开连接读到不同的快照。
pub(crate) async fn station_daily_note_usage_in(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    station_ref: Uuid,
) -> Result<i64, sqlx::Error> {
    let used: Option<i64> = sqlx::query_scalar(DAILY_NOTE_USAGE_SQL)
        .bind(station_ref)
        .fetch_one(&mut **transaction)
        .await?;
    Ok(used.unwrap_or(0))
}

/// 配额的唯一判据。两个入口共用它，口径才不会漂移。
const DAILY_NOTE_USAGE_SQL: &str = "SELECT count(*) \
     FROM linggan_runtime_record_disposition d \
     JOIN linggan_runtime_capture_package p ON p.package_ref = d.package_ref \
     JOIN plugin_installation i ON i.install_key = p.producer_instance_id::text \
     WHERE i.station_ref = $1 \
       AND d.disposition IN \
           ('accepted_for_library_discovery', 'accepted_for_library_content') \
       AND d.created_at >= \
           date_trunc('day', scope_001_now() AT TIME ZONE 'Asia/Shanghai') \
               AT TIME ZONE 'Asia/Shanghai'";

/// 一台工位在界面上的样子：人登记的事实 + 当前在岗的那个插件安装。
#[derive(Debug, Clone)]
pub struct StationOverview {
    pub station_ref: Uuid,
    pub display_name: String,
    pub daily_work_quota: i32,
    /// 认领窗口是否还开着。开着意味着新装的插件会自动绑到这台工位。
    pub claim_window_open: bool,
    pub active_plugin_version: Option<String>,
    pub active_browser_label: Option<String>,
    pub active_last_seen_at: Option<String>,
    /// 这台工位换过几次插件。内容工作台正是把这个数字变成了 11 台僵尸工位。
    pub superseded_count: i64,
    /// 当天已入库的笔记条数。与准入判定读同一段 SQL——旧项目两处口径不同，出现过
    /// 「页面显示已达上限但仍在派单」。
    pub daily_notes_used: i64,
}

/// 一个报到了但还没人认领的插件安装。它不会被派活。
#[derive(Debug, Clone)]
pub struct UnclaimedInstallation {
    pub installation_ref: Uuid,
    pub plugin_version: String,
    pub browser_label: Option<String>,
    pub first_seen_at: String,
}

/// 读工位与插件安装的现状。纯读，不建立也不改变任何东西。
pub async fn read_station_overview(
    database: &Database,
) -> Result<(Vec<StationOverview>, Vec<UnclaimedInstallation>), StationError> {
    if !station_schema_is_ready(database).await? {
        return Err(StationError::SchemaUnavailable);
    }
    let station_rows = sqlx::query_as::<_, StationRow>(
        "SELECT s.station_ref, s.display_name, s.daily_work_quota, \
                (s.claim_window_expires_at > scope_001_now()) AS claim_window_open, \
                active.plugin_version AS active_plugin_version, \
                active.browser_label AS active_browser_label, \
                to_char(active.last_seen_at, 'YYYY-MM-DD HH24:MI') AS active_last_seen_at, \
                (SELECT count(*) FROM plugin_installation h \
                 WHERE h.station_ref = s.station_ref AND h.superseded_at IS NOT NULL) \
                    AS superseded_count \
         FROM execution_station s \
         LEFT JOIN plugin_installation active \
                ON active.station_ref = s.station_ref AND active.superseded_at IS NULL \
         WHERE s.retired_at IS NULL \
         ORDER BY s.registered_at",
    )
    .fetch_all(database.pool())
    .await?;

    let unclaimed_rows = sqlx::query_as::<_, UnclaimedRow>(
        "SELECT installation_ref, plugin_version, browser_label, \
                to_char(first_seen_at, 'YYYY-MM-DD HH24:MI') AS first_seen_at \
         FROM plugin_installation \
         WHERE station_ref IS NULL AND superseded_at IS NULL \
         ORDER BY first_seen_at DESC",
    )
    .fetch_all(database.pool())
    .await?;

    let mut stations: Vec<StationOverview> = station_rows
        .into_iter()
        .map(StationOverview::from)
        .collect();
    // 配额只有一个判据。这里每台工位多跑一次查询，换的是页面与准入永远读同一段 SQL——
    // 旧项目两处口径不同，出现过「页面显示已达上限但仍在派单」。工位数是个位数，代价可忽略。
    for station in &mut stations {
        station.daily_notes_used = station_daily_note_usage(database, station.station_ref).await?;
    }

    Ok((
        stations,
        unclaimed_rows
            .into_iter()
            .map(UnclaimedInstallation::from)
            .collect(),
    ))
}

type StationRow = (
    Uuid,
    String,
    i32,
    Option<bool>,
    Option<String>,
    Option<String>,
    Option<String>,
    i64,
);

impl From<StationRow> for StationOverview {
    fn from(row: StationRow) -> Self {
        Self {
            station_ref: row.0,
            display_name: row.1,
            daily_work_quota: row.2,
            // NULL means no window was ever opened, which reads the same as a closed one.
            claim_window_open: row.3.unwrap_or(false),
            active_plugin_version: row.4,
            active_browser_label: row.5,
            active_last_seen_at: row.6,
            superseded_count: row.7,
            // 由 read_station_overview 调用配额权威函数补齐，不在这条查询里另写一份 SQL。
            daily_notes_used: 0,
        }
    }
}

type UnclaimedRow = (Uuid, String, Option<String>, String);

impl From<UnclaimedRow> for UnclaimedInstallation {
    fn from(row: UnclaimedRow) -> Self {
        Self {
            installation_ref: row.0,
            plugin_version: row.1,
            browser_label: row.2,
            first_seen_at: row.3,
        }
    }
}
