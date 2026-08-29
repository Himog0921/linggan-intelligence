//! COLLECTION-001 · 执行工位页的读投影：准入第 5 问**现在**答什么。
//!
//! 这一页此前用写死的句子回答「接通了没有」——`调度器未接通`、`准入第 5 问尚未接上工位`、
//! `队列、租约、回执尚不存在`。三句话写下时都是真的，之后三样全部接通，而句子一个字
//! 没变。**一个不会随系统状态改变的状态区块，等于一个永远不会响的警报器。**
//!
//! 因此这里不存任何文案，只读事实。页面能说的话由这些事实推出来，接通与否不再由人手写。
//!
//! 这个模块只读，不建立也不改变任何东西。

use crate::acquisition_chain::read_capacity;
use crate::execution_station::StationError;
use linggan_contracts::Capacity;
use linggan_storage_postgres::Database;

/// 本项目当前唯一的平台与目标类型。写成常量而不是散在各处的字面量，
/// 是为了在第二个平台出现时能一次找齐。
const PLATFORM: &str = "xhs";
const TARGET_KIND: &str = "creator";

/// 第 5 问「账号」那一半的当前处置。
///
/// **这不是遗漏，是一个已记录的决定。** `DECISION-04 · 工位与产能` 明写平台账号对象
/// 独立立项、不进本规则实施范围——它回答的是「以哪个观察身份看到」，与配额无关。
/// 于是 `establish_capacity` 检查的是风险、工位、能力、预算四样，**账号不在其中**。
///
/// 页面必须把这件事说出来。四项里有一项从未被检查，却显示成「四项齐备」，
/// 就是用视觉便利改写了资格——正是设计治理里「状态/语义」类变更禁止的事。
pub const ACCOUNT_CHECK_NOT_CONNECTED: &str = "账号这一项当前不参与判定。平台账号是独立立项的对象（DECISION-04），\
     它回答「以哪个观察身份看到」，与配额无关；因此第 5 问的其余三项答「是」时，\
     并不代表账号已经被检查过。";

/// 一条 lane 的产能判定。
#[derive(Debug, Clone)]
pub struct LaneVerdict {
    /// 给人看的 lane 名。
    pub zh: &'static str,
    /// 数据库里的 lane 取值。
    pub lane: &'static str,
    /// 这条 lane 需要工位具备什么能力，说给人听的版本。
    pub needs: &'static str,
    pub capacity: Capacity,
}

impl LaneVerdict {
    /// 能接活时为 `None`；接不了时是准入自己会写下的那句原因，不是页面另编的一句。
    pub fn blocking_reason(&self) -> Option<String> {
        self.capacity.blocking_reason()
    }

    pub fn is_available(&self) -> bool {
        matches!(self.capacity, Capacity::Available { .. })
    }

    /// 判定的短标签。与 `Capacity` 的变体一一对应，页面不另造第二套状态词。
    pub fn verdict_label(&self) -> &'static str {
        match self.capacity {
            Capacity::Available { .. } => "可接活",
            Capacity::NoStaffedStation => "无在岗工位",
            Capacity::MissingCapabilities { .. } => "能力不匹配",
            Capacity::DailyQuotaCommitted { .. } => "今日额度已满",
            Capacity::RiskPaused { .. } => "风险暂停中",
        }
    }
}

/// 一条仍在生效的风险暂停。
#[derive(Debug, Clone)]
pub struct ActiveRiskPause {
    pub platform: Option<String>,
    pub lane: Option<String>,
    pub reason: String,
    pub paused_by: String,
    pub paused_at: String,
}

/// 一份还活着的租约：某张工单被允许在某台工位上执行，到某个时刻为止。
#[derive(Debug, Clone)]
pub struct LiveLease {
    pub station_name: String,
    pub lane: String,
    pub target_label: String,
    pub expires_at: String,
    /// 租约是否已经展开成插件可领的任务。为假时租约是许可，还没有活。
    pub has_task: bool,
}

/// 巡检当前的样子。
///
/// **这里没有「调度器是否活着」。** 系统没有任何进程心跳记录，因此那件事页面读不到；
/// 编一个「运行中」出来，就是用视觉便利改写事实。能读到的只有下面这些真实痕迹。
#[derive(Debug, Clone)]
pub struct PatrolOutlook {
    pub total_targets: i64,
    pub monitoring_targets: i64,
    pub due_now: i64,
    /// 全系统最近一次真的派出巡检的时间。
    pub last_dispatched_at: Option<String>,
    /// 全系统最近一次真的拿回材料的时间。与上一条分开，因为「派过」与「成了」是两个事实。
    pub last_succeeded_at: Option<String>,
}

impl PatrolOutlook {
    /// 巡检没有对象时，调度器即使正在跑也不会有任何动静。
    ///
    /// 这个区别必须说出来：把「在跑但没活可派」和「根本没在跑」都显示成「未接通」，
    /// 会让人去修一个没有坏的东西。
    pub fn silent_because_nothing_to_patrol(&self) -> bool {
        self.monitoring_targets == 0
    }
}

/// 执行工位页需要的全部事实。
#[derive(Debug, Clone)]
pub struct RuntimeCapacityOverview {
    pub lanes: Vec<LaneVerdict>,
    pub risk_pauses: Vec<ActiveRiskPause>,
    pub registered_stations: i64,
    pub staffed_stations: i64,
    pub patrol: PatrolOutlook,
    pub live_leases: Vec<LiveLease>,
}

impl RuntimeCapacityOverview {
    /// 任意一条 lane 能接活，系统就还能干事。
    pub fn any_lane_available(&self) -> bool {
        self.lanes.iter().any(LaneVerdict::is_available)
    }

    /// 全部 lane 都接不了活时，返回它们各自的原因。
    ///
    /// 不合并成一句：两条 lane 可能因为**不同的**原因停下（一条缺能力、一条额度满），
    /// 压成一句「资源不足」正是 `Capacity` 当初从布尔换成枚举要避免的事。
    pub fn blocking_reasons(&self) -> Vec<(&'static str, String)> {
        self.lanes
            .iter()
            .filter_map(|lane| lane.blocking_reason().map(|reason| (lane.zh, reason)))
            .collect()
    }
}

/// 读出执行工位页要回答的全部问题。纯读。
pub async fn read_runtime_capacity(
    database: &Database,
) -> Result<RuntimeCapacityOverview, StationError> {
    // 两条 lane 的能力要求不同，因此必须分别问。合并成一次判定会让「深度建档接不了、
    // 巡检还能跑」这种真实情况消失。
    let mut lanes = Vec::new();
    for (zh, lane, needs) in [
        ("观察基线", "deep_archive", "作者档案 · 有界作品清单"),
        ("巡检", "patrol", "作品清单"),
    ] {
        lanes.push(LaneVerdict {
            zh,
            lane,
            needs,
            capacity: read_capacity(database, PLATFORM, TARGET_KIND, lane).await?,
        });
    }

    let risk_pauses = sqlx::query_as::<_, RiskPauseRow>(
        "SELECT platform, lane, reason, paused_by, \
                to_char(paused_at, 'YYYY-MM-DD HH24:MI') AS paused_at \
         FROM collection_risk_pause \
         WHERE lifted_at IS NULL \
         ORDER BY paused_at DESC",
    )
    .fetch_all(database.pool())
    .await?
    .into_iter()
    .map(ActiveRiskPause::from)
    .collect();

    let (registered_stations, staffed_stations) = sqlx::query_as::<_, (i64, i64)>(
        "SELECT count(*), \
                count(*) FILTER (WHERE i.installation_ref IS NOT NULL) \
         FROM execution_station s \
         LEFT JOIN plugin_installation i \
                ON i.station_ref = s.station_ref AND i.superseded_at IS NULL \
         WHERE s.retired_at IS NULL",
    )
    .fetch_one(database.pool())
    .await?;

    // 到期判据与 `run_due_patrols` 用同一个式子：派出时间 + 间隔 <= 现在，从未派过即到期。
    // 页面另写一份「差不多的」判据，会让人看到「显示 1 个到期」却一轮都没派出去。
    let patrol = sqlx::query_as::<_, PatrolRow>(
        "SELECT count(*), \
                count(*) FILTER (WHERE monitoring_enabled), \
                count(*) FILTER (WHERE monitoring_enabled AND ( \
                    last_patrol_dispatched_at IS NULL \
                    OR last_patrol_dispatched_at \
                       + make_interval(secs => patrol_interval_seconds) <= scope_001_now())), \
                to_char(max(last_patrol_dispatched_at), 'YYYY-MM-DD HH24:MI'), \
                to_char(max(last_patrol_succeeded_at), 'YYYY-MM-DD HH24:MI') \
         FROM collection_observation_target \
         WHERE lifecycle_state <> 'dismissed'",
    )
    .fetch_one(database.pool())
    .await
    .map(PatrolOutlook::from)?;

    let live_leases = sqlx::query_as::<_, LeaseRow>(
        "SELECT s.display_name, o.lane, \
                coalesce(t.display_name, t.identity_key), \
                to_char(l.expires_at, 'YYYY-MM-DD HH24:MI'), \
                (l.task_id IS NOT NULL) \
         FROM collection_work_order_lease l \
         JOIN collection_work_order o ON o.work_order_ref = l.work_order_ref \
         JOIN collection_observation_target t ON t.target_ref = o.target_ref \
         JOIN execution_station s ON s.station_ref = l.station_ref \
         WHERE l.released_at IS NULL AND l.expires_at > scope_001_now() \
         ORDER BY l.expires_at",
    )
    .fetch_all(database.pool())
    .await?
    .into_iter()
    .map(LiveLease::from)
    .collect();

    Ok(RuntimeCapacityOverview {
        lanes,
        risk_pauses,
        registered_stations,
        staffed_stations,
        patrol,
        live_leases,
    })
}

type RiskPauseRow = (
    Option<String>,
    Option<String>,
    String,
    String,
    Option<String>,
);

impl From<RiskPauseRow> for ActiveRiskPause {
    fn from(row: RiskPauseRow) -> Self {
        Self {
            platform: row.0,
            lane: row.1,
            reason: row.2,
            paused_by: row.3,
            paused_at: row.4.unwrap_or_else(|| "UNKNOWN".to_owned()),
        }
    }
}

type PatrolRow = (i64, i64, i64, Option<String>, Option<String>);

impl From<PatrolRow> for PatrolOutlook {
    fn from(row: PatrolRow) -> Self {
        Self {
            total_targets: row.0,
            monitoring_targets: row.1,
            due_now: row.2,
            last_dispatched_at: row.3,
            last_succeeded_at: row.4,
        }
    }
}

type LeaseRow = (String, String, String, Option<String>, Option<bool>);

impl From<LeaseRow> for LiveLease {
    fn from(row: LeaseRow) -> Self {
        Self {
            station_name: row.0,
            lane: row.1,
            target_label: row.2,
            expires_at: row.3.unwrap_or_else(|| "UNKNOWN".to_owned()),
            has_task: row.4.unwrap_or(false),
        }
    }
}
