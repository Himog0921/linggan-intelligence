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
            Capacity::Queueable => "可入队，待工位认领",
            Capacity::Available { .. } => "可接活",
            Capacity::NoStaffedStation => "无在岗工位",
            Capacity::MissingCapabilities { .. } => "能力不匹配",
            Capacity::DailyQuotaCommitted { .. } => "今日额度已满",
            Capacity::RiskPaused { .. } => "风险暂停中",
            Capacity::Unavailable { .. } => "控制闸门关闭",
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
    pub started_at: String,
    pub expires_at: String,
    pub estimated_work_units: i32,
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
    /// 当前已经越过持久 next_run_at 的规则数；不从最近完成时间反推。
    pub overdue_rules: i64,
    pub oldest_due_at: Option<String>,
    /// 全系统最近一次真的派出巡检的时间。
    pub last_dispatched_at: Option<String>,
    /// 全系统最近一次真的拿回材料的时间。与上一条分开，因为「派过」与「成了」是两个事实。
    pub last_succeeded_at: Option<String>,
}

/// 平台全局并发是第三层硬上限，和单工位、单账号的资格不同。
#[derive(Debug, Clone)]
pub struct PlatformDispatchCapacity {
    pub platform: String,
    pub concurrent_cap: i32,
    pub live_leases: i64,
}

impl PlatformDispatchCapacity {
    pub fn remaining(&self) -> i64 {
        (i64::from(self.concurrent_cap) - self.live_leases).max(0)
    }
}

/// 一个技术 lane 的队列事实。它不按业务模块分队列。
#[derive(Debug, Clone)]
pub struct DispatchLaneBacklog {
    pub dispatch_lane: String,
    pub queued_work_orders: i64,
    pub leased_work_orders: i64,
    pub retry_cooling_work_orders: i64,
    pub oldest_ready_at: Option<String>,
    pub concurrent_cap: Option<i32>,
}

/// 有效观察 Rule 的调度痕迹。所有字段都来自 Rule、WorkOrder、Task、Attempt 或 Receipt。
#[derive(Debug, Clone)]
pub struct MonitorRuleSchedule {
    pub target_label: String,
    pub interval_seconds: i32,
    pub last_scheduled_for: Option<String>,
    pub last_attempt_started_at: Option<String>,
    pub latest_work_order_state: Option<String>,
    pub next_run_at: Option<String>,
    pub last_receipt_at: Option<String>,
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
    pub platform_dispatch: Vec<PlatformDispatchCapacity>,
    pub dispatch_backlog: Vec<DispatchLaneBacklog>,
    pub monitor_rule_schedules: Vec<MonitorRuleSchedule>,
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
        ("批量建档", "deep_archive", "作者档案 · 有界作品清单"),
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
                linggan_human_moment(paused_at) AS paused_at \
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

    // 到期判据与 `run_due_patrols` 读同一个已持久化的 `monitor_next_run_at`。页面不能
    // 用上次派出时间和旧间隔反推，否则规则修订、一次 catch-up 或暂停恢复后会显示另一套事实。
    let patrol = sqlx::query_as::<_, PatrolRow>(
        // 单位仍是**目标**（页面讲的是目标），但排期住在规则上（`0078`）：一个目标
        // 「到期了」的意思是它有任一条在用规则到期了；时间取规则里最早／最近的那个。
        "SELECT count(*), \
                count(*) FILTER (WHERE target.monitoring_enabled AND rules.automatic_any), \
                count(*) FILTER (WHERE target.monitoring_enabled AND rules.automatic_any \
                    AND rules.next_run_at <= scope_001_now()), \
                count(*) FILTER (WHERE target.monitoring_enabled AND rules.automatic_any \
                    AND rules.next_run_at < scope_001_now()), \
                linggan_human_moment(min(rules.next_run_at) FILTER (WHERE target.monitoring_enabled \
                    AND rules.automatic_any AND rules.next_run_at <= scope_001_now())), \
                linggan_human_moment(max(rules.last_dispatched_at)), \
                linggan_human_moment(max(target.last_patrol_succeeded_at)) \
         FROM collection_observation_target target \
         LEFT JOIN LATERAL ( \
           SELECT COALESCE(bool_or(COALESCE(revision.automatic_enabled,false)),false) \
                AS automatic_any, \
                  min(rule.monitor_next_run_at) AS next_run_at, \
                  max(rule.last_patrol_dispatched_at) AS last_dispatched_at \
           FROM collection_monitor_rule rule \
           LEFT JOIN collection_monitor_rule_revision revision \
                  ON revision.rule_revision_ref=rule.active_revision_ref \
           WHERE rule.target_ref=target.target_ref AND rule.retired_at IS NULL) rules ON true \
         WHERE target.lifecycle_state <> 'dismissed'",
    )
    .fetch_one(database.pool())
    .await
    .map(PatrolOutlook::from)?;

    let live_leases = sqlx::query_as::<_, LeaseRow>(
        "SELECT s.display_name, o.lane, \
                coalesce(t.display_name, t.identity_key), \
                linggan_human_moment(COALESCE((SELECT min(task.claimed_at) \
                                  FROM collection_work_order_lease_task task \
                                  WHERE task.lease_ref=l.lease_ref),l.issued_at)), \
                linggan_human_moment(l.expires_at), \
                o.estimated_work_units, \
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

    let platform_dispatch = sqlx::query_as::<_, PlatformDispatchRow>(
        "SELECT policy.platform,policy.concurrent_cap, \
                (SELECT count(*) FROM collection_work_order_lease lease \
                 JOIN collection_work_order work_order USING(work_order_ref) \
                 JOIN collection_observation_target target USING(target_ref) \
                 WHERE target.platform=policy.platform AND lease.released_at IS NULL \
                   AND lease.expires_at>scope_001_now()) \
         FROM collection_platform_dispatch_policy policy ORDER BY policy.platform",
    )
    .fetch_all(database.pool())
    .await?
    .into_iter()
    .map(PlatformDispatchCapacity::from)
    .collect();

    let dispatch_backlog = sqlx::query_as::<_, DispatchLaneRow>(
        "SELECT policy.dispatch_lane,policy.concurrent_cap, \
                count(work_order.work_order_ref) FILTER (WHERE work_order.queue_state='queued'), \
                count(work_order.work_order_ref) FILTER (WHERE work_order.queue_state='leased'), \
                count(work_order.work_order_ref) FILTER (WHERE work_order.queue_state='queued' \
                    AND work_order.retry_not_before_at>scope_001_now()), \
                linggan_human_moment(min(work_order.scheduled_for) FILTER (WHERE work_order.queue_state='queued' \
                    AND work_order.retry_not_before_at<=scope_001_now() \
                    AND work_order.scheduled_for<=scope_001_now())) \
         FROM collection_dispatch_lane_fairness policy \
         LEFT JOIN collection_work_order work_order \
           ON work_order.dispatch_lane=policy.dispatch_lane \
         GROUP BY policy.dispatch_lane,policy.concurrent_cap,policy.virtual_finish \
         ORDER BY CASE policy.dispatch_lane WHEN 'immediate' THEN 1 WHEN 'scheduled' THEN 2 ELSE 3 END",
    )
    .fetch_all(database.pool())
    .await?
    .into_iter()
    .map(DispatchLaneBacklog::from)
    .collect();

    let monitor_rule_schedules = sqlx::query_as::<_, MonitorRuleScheduleRow>(
        // 一条规则一行。一个关键词可以同时盯几个榜，各有各的周期——按目标一行会把其中
        // 两条藏起来。名字带上口径，否则同一个词的三行在页面上分不出谁是谁。
        "SELECT coalesce(target.display_name,target.identity_key) \
                  || CASE WHEN rule_identity.slot_key='primary' THEN '' \
                          ELSE ' · ' || rule_identity.slot_key END, \
                rule.fixed_interval_seconds, \
                linggan_human_moment((SELECT max(work_order.scheduled_for) FROM collection_work_order work_order \
                         WHERE work_order.monitor_rule_revision_ref=rule.rule_revision_ref)), \
                linggan_human_moment((SELECT max(attempt.started_at) FROM linggan_runtime_attempt attempt \
                         JOIN collection_work_order_lease_task lease_task \
                           ON lease_task.task_id=attempt.task_id \
                         JOIN collection_work_order_lease lease USING(lease_ref) \
                         JOIN collection_work_order work_order USING(work_order_ref) \
                         WHERE work_order.monitor_rule_revision_ref=rule.rule_revision_ref)), \
                (SELECT work_order.queue_state FROM collection_work_order work_order \
                 WHERE work_order.monitor_rule_revision_ref=rule.rule_revision_ref \
                 ORDER BY work_order.scheduled_for DESC NULLS LAST,work_order.created_at DESC \
                 LIMIT 1), \
                linggan_human_moment(rule_identity.monitor_next_run_at), \
                linggan_human_moment((SELECT max(receipt.received_at) \
                         FROM linggan_runtime_submission_receipt receipt \
                         JOIN linggan_runtime_capture_package package USING(package_ref) \
                         JOIN collection_work_order_lease_task lease_task \
                           ON lease_task.task_id=package.task_id \
                         JOIN collection_work_order_lease lease USING(lease_ref) \
                         JOIN collection_work_order work_order USING(work_order_ref) \
                         WHERE work_order.monitor_rule_revision_ref=rule.rule_revision_ref)) \
         FROM collection_monitor_rule rule_identity \
         JOIN collection_observation_target target USING(target_ref) \
         JOIN collection_monitor_rule_revision rule \
           ON rule.rule_revision_ref=rule_identity.active_revision_ref \
         WHERE rule_identity.retired_at IS NULL \
           AND target.monitoring_enabled AND rule.automatic_enabled AND rule.mode='fixed' \
         ORDER BY rule_identity.monitor_next_run_at,rule_identity.rule_ref LIMIT 100",
    )
    .fetch_all(database.pool())
    .await?
    .into_iter()
    .map(MonitorRuleSchedule::from)
    .collect();

    Ok(RuntimeCapacityOverview {
        lanes,
        risk_pauses,
        registered_stations,
        staffed_stations,
        patrol,
        live_leases,
        platform_dispatch,
        dispatch_backlog,
        monitor_rule_schedules,
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

type PatrolRow = (
    i64,
    i64,
    i64,
    i64,
    Option<String>,
    Option<String>,
    Option<String>,
);

impl From<PatrolRow> for PatrolOutlook {
    fn from(row: PatrolRow) -> Self {
        Self {
            total_targets: row.0,
            monitoring_targets: row.1,
            due_now: row.2,
            overdue_rules: row.3,
            oldest_due_at: row.4,
            last_dispatched_at: row.5,
            last_succeeded_at: row.6,
        }
    }
}

type LeaseRow = (
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    i32,
    Option<bool>,
);

impl From<LeaseRow> for LiveLease {
    fn from(row: LeaseRow) -> Self {
        Self {
            station_name: row.0,
            lane: row.1,
            target_label: row.2,
            started_at: row.3.unwrap_or_else(|| "暂无".to_owned()),
            expires_at: row.4.unwrap_or_else(|| "UNKNOWN".to_owned()),
            estimated_work_units: row.5,
            has_task: row.6.unwrap_or(false),
        }
    }
}

type PlatformDispatchRow = (String, i32, i64);

impl From<PlatformDispatchRow> for PlatformDispatchCapacity {
    fn from(row: PlatformDispatchRow) -> Self {
        Self {
            platform: row.0,
            concurrent_cap: row.1,
            live_leases: row.2,
        }
    }
}

type DispatchLaneRow = (String, Option<i32>, i64, i64, i64, Option<String>);

impl From<DispatchLaneRow> for DispatchLaneBacklog {
    fn from(row: DispatchLaneRow) -> Self {
        Self {
            dispatch_lane: row.0,
            concurrent_cap: row.1,
            queued_work_orders: row.2,
            leased_work_orders: row.3,
            retry_cooling_work_orders: row.4,
            oldest_ready_at: row.5,
        }
    }
}

type MonitorRuleScheduleRow = (
    String,
    Option<i32>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

impl From<MonitorRuleScheduleRow> for MonitorRuleSchedule {
    fn from(row: MonitorRuleScheduleRow) -> Self {
        Self {
            target_label: row.0,
            interval_seconds: row.1.unwrap_or(0),
            last_scheduled_for: row.2,
            last_attempt_started_at: row.3,
            latest_work_order_state: row.4,
            next_run_at: row.5,
            last_receipt_at: row.6,
        }
    }
}
