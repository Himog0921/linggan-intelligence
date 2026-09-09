//! COLLECTION-001 · 工位的读投影与每日配额。
//!
//! 与登记/认领分开，因为它们回答的是不同的问题：那边写「谁是谁」，这边只读「现在是什么
//! 样子」。配额判据也放在这里——展示与准入读同一段 SQL，两处口径才不会漂移。

use crate::execution_station::{StationError, station_schema_is_ready};
use linggan_storage_postgres::Database;
use uuid::Uuid;

/// 一台工位当天碰过的**笔记篇数**。
///
/// **配额的权威实现只有这一个函数**，准入判定与页面展示都读它。旧项目有两份 200——一份
/// 管派单硬编码、一份管页面显示读插件上报值，判据还不同，于是出现「页面显示已达上限但
/// 仍在派单」。两处口径必然漂移，只留一处才不会。
///
/// 计数单位是**去重后的笔记篇数**：一篇笔记当天无论被读过几次、读回多少评论与回复，都
/// 只占一格。按记录条数计会被评论吞掉——见 [`DAILY_NOTE_USAGE_SQL`] 的说明。隔离与未解释
/// 的记录不计入：它们没有成为可用材料，占用配额就等于让失败的采集吃掉当天的额度。
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

/// 配额的唯一判据。每个入口共用它，口径才不会漂移。
///
/// 单位是**笔记篇数**，不是记录条数。一篇笔记当天无论被读过几次、读回多少评论与回复，
/// 都只占一格。
///
/// 按条数计会让配额被评论吞掉：实测本库 accepted 记录里回复占 54%、评论占 26%，正文详情
/// 只有 4%。一次「三篇带评论」的建档，正文算 3 条，评论与回复可能算掉一两百条 —— 于是
/// 「今天最多两百篇」实际变成「今天最多两三篇」。配额守的是平台访问，而读评论发生在
/// 那篇笔记已经打开的页面上，不是又一次访问。
///
/// 只统计需要打开笔记详情页的采集（`content_detail` / `comments` / `replies`）。发现面
/// （`discovery_search` / `profile_discovery`）一次列表访问就能带回几十篇，把它按篇计费
/// 等于用一次访问的风险扣掉几十格；作者资料同理，它读的是作者页。
///
/// `sourceObject.type='content'` 是笔记身份的判据：评论记录的 sourceObject 指向它所属的
/// 那篇笔记而非评论自身，因此按 externalId 去重天然得到「碰过几篇笔记」。
const DAILY_NOTE_USAGE_SQL: &str = "SELECT count(DISTINCT record.value -> 'sourceObject' ->> 'externalId') \
     FROM linggan_runtime_record_disposition d \
     JOIN linggan_runtime_capture_package p ON p.package_ref = d.package_ref \
     JOIN plugin_installation i ON i.install_key = p.producer_instance_id::text \
     CROSS JOIN LATERAL jsonb_array_elements(p.payload -> 'records') \
          WITH ORDINALITY AS record(value, ordinality) \
     WHERE i.station_ref = $1 \
       AND record.ordinality = d.record_ordinal + 1 \
       AND d.disposition IN \
           ('accepted_for_library_discovery', 'accepted_for_library_content') \
       AND p.payload ->> 'packageKind' IN ('content_detail', 'comments', 'replies') \
       AND record.value -> 'sourceObject' ->> 'type' = 'content' \
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
    /// 当天碰过的笔记篇数（去重）。与准入判定读同一段 SQL——旧项目两处口径不同，出现过
    /// 「页面显示已达上限但仍在派单」。
    pub daily_notes_used: i64,
    /// 上一次这台工位来问活时，服务端给出的回答。三项一起为空表示它还从来没问过。
    ///
    /// 这不是「现在能不能接活」——那是 `RuntimeCapacityOverview` 的判定。这里是**实际
    /// 发生过的一次对话**：工位问了，我们答了什么。2026-09-08 那一整天的答案都是
    /// 「被拦住了」，而页面上一个字也看不到。
    pub last_dispatch_answer_at: Option<String>,
    pub last_dispatch_answer_code: Option<String>,
    /// 回答是「被拦住」时，拦住它的那条具体原因码。
    pub last_dispatch_answer_reason: Option<String>,
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
                linggan_human_moment(active.last_seen_at) AS active_last_seen_at, \
                (SELECT count(*) FROM plugin_installation h \
                 WHERE h.station_ref = s.station_ref AND h.superseded_at IS NOT NULL) \
                    AS superseded_count, \
                linggan_human_moment(s.last_dispatch_answer_at) \
                    AS last_dispatch_answer_at, \
                s.last_dispatch_answer_code, s.last_dispatch_answer_reason \
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
                linggan_human_moment(first_seen_at) AS first_seen_at \
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
    Option<String>,
    Option<String>,
    Option<String>,
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
            last_dispatch_answer_at: row.8,
            last_dispatch_answer_code: row.9,
            last_dispatch_answer_reason: row.10,
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

// ---------------------------------------------------------------------------
// 能力矩阵：这台工位的每一项能力，现在是什么状态
// ---------------------------------------------------------------------------

/// 判定窗口。七天是当前全部运行数据的跨度量级；窗口必须显式，因为「就绪」的意思
/// 是「最近还在跑」，不是「历史上跑成过一次」。
const CAPABILITY_WINDOW_DAYS: i32 = 7;

/// 一项能力在一台工位上的当前状态。
///
/// 四态而不是三态，因为「声明支持但窗口内没跑过」既不是就绪也不是降级——它是**没有证据**。
/// 把它显示成降级，就是把「没派过这种活」说成「这台机器坏了」；显示成就绪，则是拿
/// 一次都没验证过的东西当已验证。两种都是用视觉便利改写事实。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityState {
    /// 窗口内有成功产出，且没有能力级失败。
    Ready,
    /// 窗口内出现过 `capability_not_executable_here`——插件明确说这里跑不了这项。
    Degraded,
    /// 插件声明支持，但窗口内既无成功也无能力级失败。没有证据，不下结论。
    Unverified,
    /// 当前在岗插件没有声明这项能力。
    NotDeclared,
}

/// 一台工位的一项能力读数。
///
/// `execution_failures` 与 `capability_failures` 分开计，因为两者含义完全不同：
/// 页面超时、标签页丢失是**这一次**执行没成，能力本身没问题；
/// `capability_not_executable_here` 才是插件在说「这项我干不了」。
/// 合成一个「失败数」会让一次网络抖动看起来像能力缺陷。
#[derive(Debug, Clone)]
pub struct StationCapability {
    /// 机器名，如 `author_profile`。它是合同里的字面取值，界面上按 LIDS-LANG-001
    /// 的 Mono 预算可以原样显示，但必须有中文名相邻。
    pub capability: String,
    pub declared: bool,
    pub successes: i64,
    pub last_success_at: Option<String>,
    /// 能力级失败：插件声明这里跑不了这一项。
    pub capability_failures: i64,
    /// 执行级失败：超时、标签页不可用等。不翻转能力状态。
    pub execution_failures: i64,
    pub last_failure_at: Option<String>,
}

impl StationCapability {
    pub fn state(&self) -> CapabilityState {
        if !self.declared {
            return CapabilityState::NotDeclared;
        }
        if self.capability_failures > 0 {
            return CapabilityState::Degraded;
        }
        if self.successes > 0 {
            return CapabilityState::Ready;
        }
        CapabilityState::Unverified
    }
}

/// 读一台工位的能力矩阵。纯读。
///
/// 三个来源合成，每一个都有它自己的口径问题，都在这里一次说清：
///
/// 1. **声明**来自当前在岗安装的 `capabilities`。它回答「这个插件版本支持什么」，
///    与跑得成跑不成无关；所有安装报的是同一份清单。
/// 2. **成功**来自 `linggan_runtime_capture_package`。它与工位的连接靠
///    `producer_instance_id = plugin_installation.install_key`——**这是约定不是外键**，
///    库里现有约 6% 的包连不上（那些插件从未被认领到任何工位）。连不上的不计入任何
///    工位，而不是摊到某一台头上。
/// 3. **失败**来自 `collection_work_order_lease_task_dispatch_failure`，经 task 反查
///    `capabilitiesRequested`。这条链有正经外键。
pub async fn read_station_capabilities(
    database: &Database,
    station_ref: Uuid,
) -> Result<Vec<StationCapability>, StationError> {
    if !station_schema_is_ready(database).await? {
        return Err(StationError::SchemaUnavailable);
    }
    let rows = sqlx::query_as::<_, CapabilityRow>(
        "WITH active_declared AS ( \
             SELECT jsonb_array_elements_text(pi.capabilities) AS capability \
             FROM plugin_installation pi \
             WHERE pi.station_ref = $1 AND pi.superseded_at IS NULL \
         ), \
         successes AS ( \
             SELECT p.package_kind AS capability, count(*) AS successes, \
                    max(p.accepted_at) AS last_success_at \
             FROM linggan_runtime_capture_package p \
             JOIN plugin_installation pi ON pi.install_key = p.producer_instance_id::text \
             WHERE pi.station_ref = $1 \
               AND p.accepted_at > scope_001_now() - make_interval(days => $2) \
             GROUP BY 1 \
         ), \
         failures AS ( \
             SELECT tk.task_spec->'capabilitiesRequested'->>0 AS capability, \
                    count(*) FILTER (WHERE f.failure_code = 'capability_not_executable_here') \
                        AS capability_failures, \
                    count(*) FILTER (WHERE f.failure_code <> 'capability_not_executable_here') \
                        AS execution_failures, \
                    max(f.occurred_at) AS last_failure_at \
             FROM collection_work_order_lease_task_dispatch_failure f \
             JOIN plugin_installation pi ON pi.installation_ref = f.installation_ref \
             JOIN linggan_runtime_task tk ON tk.task_id = f.task_id \
             WHERE pi.station_ref = $1 \
               AND f.occurred_at > scope_001_now() - make_interval(days => $2) \
             GROUP BY 1 \
         ) \
         SELECT COALESCE(d.capability, s.capability, fl.capability) AS capability, \
                (d.capability IS NOT NULL) AS declared, \
                COALESCE(s.successes, 0) AS successes, \
                linggan_human_moment(s.last_success_at) AS last_success_at, \
                COALESCE(fl.capability_failures, 0) AS capability_failures, \
                COALESCE(fl.execution_failures, 0) AS execution_failures, \
                linggan_human_moment(fl.last_failure_at) AS last_failure_at \
         FROM active_declared d \
         FULL OUTER JOIN successes s ON s.capability = d.capability \
         FULL OUTER JOIN failures fl ON fl.capability = COALESCE(d.capability, s.capability) \
         ORDER BY 1",
    )
    .bind(station_ref)
    .bind(CAPABILITY_WINDOW_DAYS)
    .fetch_all(database.pool())
    .await?;

    Ok(rows.into_iter().map(StationCapability::from).collect())
}

type CapabilityRow = (
    Option<String>,
    Option<bool>,
    i64,
    Option<String>,
    i64,
    i64,
    Option<String>,
);

impl From<CapabilityRow> for StationCapability {
    fn from(row: CapabilityRow) -> Self {
        Self {
            capability: row.0.unwrap_or_default(),
            declared: row.1.unwrap_or(false),
            successes: row.2,
            last_success_at: row.3,
            capability_failures: row.4,
            execution_failures: row.5,
            last_failure_at: row.6,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 配额守的是「碰过几篇笔记」，不是「入库了几条记录」。
    ///
    /// 按条数计会被评论吞掉——实测本库 accepted 记录里回复 54%、评论 26%，正文详情只有 4%。
    /// 这三条断言分别钉住：按笔记去重、只算详情页上的采集、只认笔记身份。
    #[test]
    fn daily_quota_counts_notes_not_records() {
        assert!(
            DAILY_NOTE_USAGE_SQL
                .contains("count(DISTINCT record.value -> 'sourceObject' ->> 'externalId')"),
            "配额必须按笔记去重，一篇笔记读回多少评论都只占一格",
        );
        assert!(
            DAILY_NOTE_USAGE_SQL.contains("'content_detail', 'comments', 'replies'"),
            "只统计需要打开笔记详情页的采集；发现面一次列表访问带回几十篇，不该按篇计费",
        );
        assert!(
            DAILY_NOTE_USAGE_SQL.contains("'sourceObject' ->> 'type' = 'content'"),
            "评论记录的 sourceObject 指向它所属的笔记，按 type 收窄才不会把作者页算进来",
        );
    }
}
