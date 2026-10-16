//! Bounded collection runtime presentation facts for React.
//! Business availability remains owned by the existing capacity and control read models.
use super::collection::collection_control_surface_view::{
    RuntimeLaneControlView, RuntimeResourceView,
};
use super::collection_targets_view::{
    beijing_now_minutes, minutes_since_epoch, moment_without_year,
};
use linggan_evidence::{
    CONTROL_FRESHNESS_MINUTES, CapabilityState, MINIMUM_PLUGIN_VERSION, RuntimeCapacityOverview,
    StationCapability, StationOverview, StationSelectorHealth, UnclaimedInstallation,
};
use std::collections::BTreeMap;
use uuid::Uuid;

pub type CapabilityMatrix = BTreeMap<Uuid, Vec<StationCapability>>;
pub const RECENT_INSTALLATION_DAYS: i64 = 7;

pub struct RuntimeControl<'a> {
    pub lanes: &'a [RuntimeLaneControlView],
    pub resources: &'a [RuntimeResourceView],
    /// 本机是否配置了账号身份摘要。没配置时认证身份不会上报，但明确的登录或限制
    /// 仍然会阻断接活——这两件事必须分开说，否则「不上报」会被读成「不检查」。
    pub account_observation_available: bool,
}

struct LaneRow {
    name: String,
    needs: &'static str,
    available: bool,
    /// 准入允许入队、但还没有工位能领——这既不是「能接活」也不是「坏了」。
    queueable: bool,
    reason: Option<String>,
    reason_code: Option<String>,
}

struct CapabilityCounts {
    readable: bool,
    ready: usize,
    degraded: usize,
    unverified: usize,
    undeclared: usize,
}

impl RuntimeControl<'_> {
    fn resource(&self, station_ref: Uuid) -> Option<&RuntimeResourceView> {
        self.resources
            .iter()
            .find(|resource| resource.station_ref == station_ref)
    }
}

enum Verdict {
    All,
    Some,
    None,
}

pub fn recent_installation_cutoff() -> i64 {
    beijing_now_minutes()
}

pub fn installation_is_recent(first_seen_at: &str, now_minutes: i64) -> bool {
    minutes_since_epoch(first_seen_at)
        .is_none_or(|seen| now_minutes - seen <= RECENT_INSTALLATION_DAYS * 24 * 60)
}

fn verdict_state(rows: Option<&Vec<LaneRow>>) -> (&'static str, &'static str) {
    let Some(rows) = rows else {
        return ("读不到", "unknown");
    };
    if !rows.is_empty()
        && rows
            .iter()
            .all(|row| row.reason_code.as_deref() == Some("collection_upgrade_recovery_only"))
    {
        return ("采集暂停", "blocked");
    }
    if !rows.is_empty() && rows.iter().all(is_capacity_wait) {
        return ("等待执行容量", "partial");
    }
    match verdict_of(rows) {
        Verdict::All => ("能接活", "ok"),
        Verdict::Some => ("部分接不了活", "partial"),
        Verdict::None => ("接不了活", "blocked"),
    }
}

fn is_capacity_wait(row: &LaneRow) -> bool {
    !row.available
        && matches!(
            row.reason_code.as_deref(),
            Some("station_busy" | "account_busy" | "platform_concurrency_reached")
        )
}

fn verdict_of(rows: &[LaneRow]) -> Verdict {
    if rows.iter().all(|row| row.available) {
        Verdict::All
    } else if rows.iter().any(|row| row.available) {
        Verdict::Some
    } else {
        Verdict::None
    }
}

fn lane_needs(target_kind: &str, lane: &str) -> &'static str {
    match (target_kind, lane) {
        (_, "deep_archive") => "按作者档案建一份有界作品清单",
        ("keyword", "patrol") => "查一遍搜索结果有没有新命中",
        (_, "patrol") => "查一遍作品清单有没有新作品",
        _ => "这条通道的用途尚未登记说明",
    }
}

fn lane_rows(
    control: Option<&RuntimeControl<'_>>,
    overview: Option<&RuntimeCapacityOverview>,
) -> Option<Vec<LaneRow>> {
    if let Some(control) = control.filter(|control| !control.lanes.is_empty()) {
        return Some(
            control
                .lanes
                .iter()
                .map(|lane| LaneRow {
                    name: lane.label.to_owned(),
                    needs: lane_needs(lane.target_kind, lane.lane),
                    available: lane.available,
                    queueable: lane.queueable,
                    reason: lane.reason.clone(),
                    reason_code: lane.reason_code.map(str::to_owned),
                })
                .collect(),
        );
    }
    let overview = overview?;
    Some(
        overview
            .lanes
            .iter()
            .map(|lane| LaneRow {
                name: lane.zh.to_owned(),
                needs: lane_needs("", lane.lane),
                available: lane.is_available(),
                queueable: false,
                reason: lane.blocking_reason(),
                reason_code: None,
            })
            .collect(),
    )
}

fn lane_state(row: &LaneRow) -> (&'static str, &'static str) {
    if row.available {
        ("c-lane-ok", "可接活")
    } else if is_capacity_wait(row) {
        ("c-lane-queueable", "等当前任务完成")
    } else if row.queueable {
        ("c-lane-queueable", "可排队，等工位")
    } else if row.reason_code.as_deref() == Some("collection_upgrade_recovery_only") {
        ("c-lane-blocked", "暂不派发")
    } else {
        ("c-lane-blocked", "接不了")
    }
}

fn capacity_state(row: &LaneRow) -> &'static str {
    if row.available {
        "available"
    } else if is_capacity_wait(row) {
        "waiting"
    } else if row.queueable {
        "queueable"
    } else {
        "blocked"
    }
}

fn selector_health_detail(record: &StationSelectorHealth) -> String {
    let mut lines = vec![
        format!(
            "这次检查：{}（{} 页面 · {}）",
            or_unknown(&record.checked_at),
            or_unknown(&record.page_type),
            or_unknown(&record.capability),
        ),
        format!("选择器验证日期：{}", verification_date(&record.verified_at)),
    ];
    if !record.checked_categories.is_empty() {
        lines.push(format!(
            "检查了 {} 类：{}",
            record.checked_categories.len(),
            record.checked_categories.join("、"),
        ));
    }
    if !record.missing_categories.is_empty() {
        lines.push(format!("缺：{}", record.missing_categories.join("、")));
    }
    if !record.stale_categories.is_empty() {
        lines.push(format!(
            "验证日期陈旧：{}",
            record.stale_categories.join("、")
        ));
    }
    if !record.failure_counts.is_empty() {
        let counts: Vec<String> = record
            .failure_counts
            .iter()
            .map(|(category, count)| format!("{category} 连续 {count} 次"))
            .collect();
        lines.push(format!("连续缺失：{}", counts.join("、")));
    }
    lines.join("\n")
}

fn or_unknown(value: &str) -> &str {
    if value.trim().is_empty() {
        "未知"
    } else {
        value
    }
}

fn verification_date(value: &str) -> &str {
    let Some((date, _time)) = value.split_once(' ') else {
        return value;
    };
    let shaped = date.len() == 10
        && date.char_indices().all(|(i, c)| {
            if i == 4 || i == 7 {
                c == '-'
            } else {
                c.is_ascii_digit()
            }
        });
    if shaped { date } else { value }
}

fn platform_label(platform: &str) -> &str {
    match platform {
        "xhs" => "小红书",
        "douyin" => "抖音",
        other => other,
    }
}

fn station_state(station: &StationOverview, now_minutes: i64) -> (&'static str, &'static str) {
    if station.active_plugin_version.is_none() {
        // 没有在岗安装不等于工位坏了，只是现在没插件连着它。
        return ("空缺", "c-tg-neutral");
    }
    let Some(seen) = station
        .active_last_seen_at
        .as_deref()
        .and_then(minutes_since_epoch)
    else {
        return ("在岗", "c-tg-neutral");
    };
    if now_minutes - seen > i64::from(CONTROL_FRESHNESS_MINUTES) {
        ("失联", "c-tg-warn")
    } else {
        ("在岗", "c-tg-ok")
    }
}

fn account_binding_label(state: &str) -> (&'static str, &'static str) {
    match state {
        "current" => ("c-tg-ok", "已确认"),
        "unconfirmed" => ("c-tg-warn", "待确认"),
        "changed" => ("c-tg-warn", "已变化"),
        "bound_without_eligibility" => ("c-tg-warn", "缺资格信号"),
        _ => ("c-tg-neutral", "未绑定"),
    }
}

fn short_ref(value: Option<Uuid>) -> String {
    value.map_or_else(
        || "—".to_owned(),
        |value| value.to_string().chars().take(8).collect(),
    )
}

fn capability_counts(capabilities: Option<&Vec<StationCapability>>) -> CapabilityCounts {
    let Some(rows) = capabilities else {
        return CapabilityCounts {
            readable: false,
            ready: 0,
            degraded: 0,
            unverified: 0,
            undeclared: 0,
        };
    };
    let mut counts = CapabilityCounts {
        readable: true,
        ready: 0,
        degraded: 0,
        unverified: 0,
        undeclared: 0,
    };
    for row in rows {
        match row.state() {
            CapabilityState::Ready => counts.ready += 1,
            CapabilityState::Degraded => counts.degraded += 1,
            CapabilityState::Unverified => counts.unverified += 1,
            CapabilityState::NotDeclared => counts.undeclared += 1,
        }
    }
    counts
}

fn capability_label(capability: &str) -> &str {
    match capability {
        "discovery_search" => "搜索发现",
        "profile_discovery" => "主页发现",
        "author_profile" => "作者档案",
        "content_detail" => "内容详情",
        "comments" => "评论",
        "replies" => "评论回复",
        "media_slots" => "媒体",
        "xhs.pageAccess" => "页面访问",
        other => other,
    }
}

fn station_dispatch_answer(station: &StationOverview) -> (&'static str, String) {
    let Some(code) = station.last_dispatch_answer_code.as_deref() else {
        // 从未问过与「问了但没活」是两件事。前者说明这台工位根本没来敲过门，
        // 该查的是插件那一侧，不是队列。
        return (
            "从未问过活",
            "这台工位还没有向服务端要过任务。先确认插件已安装，并且已经归位到这台工位。".to_owned(),
        );
    };
    dispatch_answer_label(code, station.last_dispatch_answer_reason.as_deref())
}

fn dispatch_answer_label(code: &str, reason: Option<&str>) -> (&'static str, String) {
    if let Some((_, label, hint)) = DISPATCH_ANSWER_EXPLANATIONS
        .iter()
        .find(|(key, _, _)| *key == code)
    {
        return ((*label), dispatch_answer_hint(code, hint));
    }
    if code == "capacity_unknown" {
        return (
            "原因还没有中文说明",
            reason.map_or_else(
                || "服务端只说了「资格未知」，没有给出更具体的原因。".to_owned(),
                |reason| format!("服务端给出的原因码是 {reason}。"),
            ),
        );
    }
    (
        "这条回答还没有中文说明",
        format!("服务端给出的判定是 {code}。"),
    )
}

fn dispatch_answer_hint(code: &str, hint: &str) -> String {
    if code == "plugin_version_unsupported" {
        format!("{hint}当前最低版本是 {MINIMUM_PLUGIN_VERSION}。")
    } else {
        hint.to_owned()
    }
}

fn moment_or_never(value: Option<&str>) -> String {
    value.map_or_else(
        || "从未".to_owned(),
        |value| moment_without_year(value).to_owned(),
    )
}

fn work_order_state_label(value: Option<&str>) -> &'static str {
    match value {
        Some("queued") => "等待领取",
        Some("leased") => "执行中",
        Some("completed") => "已完成",
        Some("cancelled") => "已取消",
        Some("legacy") => "历史状态",
        _ => "还没排过",
    }
}

fn interval_label(seconds: i32) -> String {
    match seconds {
        21_600 => "6 小时".to_owned(),
        43_200 => "12 小时".to_owned(),
        86_400 => "24 小时".to_owned(),
        172_800 => "2 天".to_owned(),
        604_800 => "7 天".to_owned(),
        _ => "未知间隔".to_owned(),
    }
}

const DISPATCH_ANSWER_EXPLANATIONS: &[(&str, &str, &str)] = &[
    ("dispatch", "派出了一单", "工位正在执行它；这是正常状态。"),
    (
        "nothing_waiting",
        "队列里没有活",
        "这不是故障。要立刻采一次，去观察目标页发起人工观察。",
    ),
    (
        "installation_not_claimed",
        "插件还没归位",
        "在这一页把它认领到一台工位上，或者开一个认领窗口。",
    ),
    (
        "risk_paused",
        "风险暂停生效中",
        "有人按下了刹车。解除之前不会派出任何活。",
    ),
    (
        "daily_quota_reached",
        "今天额度用完了",
        "工位没有离线，明天窗口重置后自然恢复。",
    ),
    (
        "station_daily_budget_reached",
        "今天额度用完了",
        "工位没有离线，明天窗口重置后自然恢复。",
    ),
    (
        "execution_locator_unavailable",
        "缺少可用的签名链接",
        "这篇作品当前没有带签名的已接纳发现链接，已进入冷却重试。",
    ),
    (
        "station_not_accepting",
        "被人暂停了接活",
        "在这一行展开，把「恢复这台工位自动接活」点开。",
    ),
    (
        "station_unavailable",
        "工位或安装已不在岗",
        "确认那台工位还在、插件还连着。",
    ),
    (
        "installation_credential_missing",
        "在岗安装没有有效凭据",
        "让插件重新报到一次，服务端会补发凭据。",
    ),
    (
        "plugin_version_unsupported",
        "插件版本过低",
        "升级这台机器上的插件。",
    ),
    (
        "installation_stale",
        "插件超过 20 分钟没报到",
        "按失联处理。确认那台机器的浏览器还开着、插件还启用着。",
    ),
    (
        "capability_missing",
        "缺这条活要的能力",
        "在这一行展开看「能力」缺哪一项；升级插件或换一台工位。",
    ),
    (
        "account_unbound",
        "还没绑定观察账号",
        "在这一行展开，把这个安装绑定到一个平台账号上。",
    ),
    (
        "account_binding_changed",
        "账号绑定与工单对不上",
        "这一单是按旧绑定发出的；重新发起一次即可。",
    ),
    (
        "account_binding_expired",
        "账号绑定超过 30 天没确认",
        "重新确认一次这个安装绑定的是哪个账号。",
    ),
    (
        "account_eligibility_stale",
        "账号观察已过诊断窗口",
        "这是历史兼容状态；当前不会仅因观察时间阻断接活。实际登录、限制或绑定问题仍会阻断。",
    ),
    (
        "account_cooling",
        "账号正在冷却",
        "平台恢复后，在正常平台页面形成新的账号观察再恢复接活。",
    ),
    (
        "account_needs_login",
        "账号需要重新登录",
        "在这台机器的浏览器里重新登录平台账号。",
    ),
    (
        "account_restricted",
        "账号被平台限制",
        "先在浏览器里处理平台的验证或限制，再让它接活。",
    ),
    (
        "account_unknown",
        "账号资格未知",
        "尚未形成可判定的账号事实，按关闭处理；页面暂时读不到标记不会覆盖已有状态。",
    ),
    (
        "account_busy",
        "这个账号已有活在跑",
        "等它跑完自然轮到下一单。",
    ),
    (
        "station_busy",
        "这台工位已有活在跑",
        "等当前这一份跑完；未观察账号的首单也只允许这一份并发。",
    ),
    (
        "platform_concurrency_reached",
        "平台并发已满",
        "等任一执行许可释放后自然恢复。",
    ),
    (
        "rule_revision_changed",
        "规则改过了",
        "这一单是按旧规则发出的，不会再执行；新规则会自己排下一单。",
    ),
    (
        "monitoring_paused",
        "这个目标的自动观察被暂停了",
        "去观察目标页把它的巡查重新打开。",
    ),
    (
        "domain_paused_or_unscoped",
        "工单的领域已暂停或没有冻结领域用途",
        "到领域管理核对工单涉及的全部领域；暂停的领域恢复运行后，重新检查这张工单。",
    ),
    (
        "authorization_expired_or_revoked",
        "这一单的授权已撤销或过期",
        "工单已终结，不会再重试。需要的话在观察目标页重新发起一次。",
    ),
    (
        "collection_upgrade_recovery_only",
        "采集暂停",
        "系统处于恢复阶段，暂不派发新任务。",
    ),
    (
        "target_not_requestable",
        "这个观察目标已被弃置",
        "弃置的目标不能再发起采集。要用它就先恢复这个目标。",
    ),
];

/// Same bounded read facts and labels as the retired station renderer, without HTML.
pub(super) fn runtime_projection(
    overview: Option<&RuntimeCapacityOverview>,
    roster: Option<(&[StationOverview], &[UnclaimedInstallation])>,
    capabilities: &CapabilityMatrix,
    control: Option<&RuntimeControl<'_>>,
    now_minutes: i64,
) -> serde_json::Value {
    use serde_json::json;
    let lanes = lane_rows(control, overview);
    let station_rows = roster.map(|(stations, _)| {
        stations
            .iter()
            .map(|station| {
                station_projection(
                    station,
                    capabilities.get(&station.station_ref),
                    control.and_then(|c| c.resource(station.station_ref)),
                    now_minutes,
                )
            })
            .collect::<Vec<_>>()
    });
    let unclaimed = roster.map(|(_, rows)| {
        rows.iter()
            .map(|row| {
                json!({
                    "installationRef": row.installation_ref,
                    "pluginVersion": row.plugin_version,
                    "browser": row.browser_label.as_deref().unwrap_or("未知浏览器"),
                    "firstSeen": moment_without_year(&row.first_seen_at),
                    "recent": installation_is_recent(&row.first_seen_at, now_minutes)
                })
            })
            .collect::<Vec<_>>()
    });
    let running = overview.map(|overview| {
        overview
            .live_leases
            .iter()
            .filter(|lease| lease.has_task)
            .map(|lease| {
                json!({
                    "target": lease.target_label,
                    "station": lease.station_name,
                    "started": moment_without_year(&lease.started_at),
                    "expires": moment_without_year(&lease.expires_at),
                    "units": lease.estimated_work_units
                })
            })
            .collect::<Vec<_>>()
    });
    let schedule = overview.map(|overview| overview.monitor_rule_schedules.iter().map(|rule| json!({
        "target": rule.target_label,
        "interval": interval_label(rule.interval_seconds),
        "state": work_order_state_label(rule.latest_work_order_state.as_deref()),
        "last": moment_or_never(rule.last_receipt_at.as_deref().or(rule.last_attempt_started_at.as_deref())),
        "next": moment_or_never(rule.next_run_at.as_deref())
    })).collect::<Vec<_>>());
    json!({
        "meta": {
            "vacantStations": overview.map(|overview| overview.registered_stations - overview.staffed_stations),
            "unclaimedInstallations": roster.map(|(_,rows)| rows.iter().filter(|row| installation_is_recent(&row.first_seen_at, now_minutes)).count())
        },
        "board": board_projection(overview, lanes.as_ref()),
        "lanes": lanes.as_ref().map(|rows| rows.iter().map(lane_projection).collect::<Vec<_>>()),
        "stations": station_rows,
        "unclaimed": unclaimed,
        "accountObservationAvailable": control.is_none_or(|control| control.account_observation_available),
        "staffedCount": roster.map(|(stations,_)| stations.iter().filter(|station| station.active_plugin_version.is_some()).count()),
        "running": running,
        "schedule": schedule,
        "drawer": drawer_projection(overview, roster.map(|(stations,_)|stations), capabilities)
    })
}

fn board_projection(
    overview: Option<&RuntimeCapacityOverview>,
    lanes: Option<&Vec<LaneRow>>,
) -> serde_json::Value {
    use serde_json::json;
    let (state, tone) = verdict_state(lanes);
    let blocked = lanes.map(|rows| {
        rows.iter()
            .filter(|row| !row.available && !row.queueable && !is_capacity_wait(row))
            .count()
    });
    let waiting = lanes.map(|rows| {
        rows.iter()
            .filter(|row| !row.available && (row.queueable || is_capacity_wait(row)))
            .count()
    });
    let badge = if state == "采集暂停" {
        "恢复阶段".to_owned()
    } else if let Some(count) = blocked.filter(|count| *count > 0) {
        format!("{count} 项阻塞")
    } else if let Some(count) = waiting.filter(|count| *count > 0) {
        format!("{count} 项等容量")
    } else if lanes.is_some() {
        "全部通畅".to_owned()
    } else {
        "读不到".to_owned()
    };
    let backlog = |pick: fn(&linggan_evidence::DispatchLaneBacklog) -> i64| {
        overview
            .filter(|overview| !overview.dispatch_backlog.is_empty())
            .map(|overview| {
                overview
                    .dispatch_backlog
                    .iter()
                    .map(pick)
                    .sum::<i64>()
                    .to_string()
            })
            .unwrap_or_else(|| "—".to_owned())
    };
    let metrics = [
        (
            "在线工位",
            overview
                .map(|overview| {
                    format!(
                        "{}/{}",
                        overview.staffed_stations, overview.registered_stations
                    )
                })
                .unwrap_or_else(|| "读不到".to_owned()),
        ),
        ("执行中", backlog(|row| row.leased_work_orders)),
        ("排队等待", backlog(|row| row.queued_work_orders)),
        ("失败重试", backlog(|row| row.retry_cooling_work_orders)),
        (
            "自动观察",
            overview
                .map(|overview| {
                    format!(
                        "{}/{}",
                        overview.patrol.monitoring_targets, overview.patrol.total_targets
                    )
                })
                .unwrap_or_else(|| "读不到".to_owned()),
        ),
        (
            "并发上限",
            overview
                .and_then(|overview| overview.platform_dispatch.first())
                .map(|capacity| format!("{}/{}", capacity.live_leases, capacity.concurrent_cap))
                .unwrap_or_else(|| "—".to_owned()),
        ),
    ];
    let head_note = lanes
        .map(
            |rows| match overview.map_or(0, |overview| overview.risk_pauses.len()) {
                0 => format!("{} 条通道", rows.len()),
                count => format!("{} 条通道 · {count} 条风险暂停", rows.len()),
            },
        )
        .unwrap_or_else(|| "通道数未知".to_owned());
    json!({
        "state": if lanes.is_some() {state} else {"运行状态读不到"},
        "tone": tone,
        "badge": badge,
        "headNote": head_note,
        "metrics": metrics.iter().map(|(label,value)|json!({"label":label,"value":value})).collect::<Vec<_>>()
    })
}

fn lane_projection(row: &LaneRow) -> serde_json::Value {
    let (class, label) = lane_state(row);
    serde_json::json!({
        "name": row.name,
        "needs": row.needs,
        "detail": row.reason.as_deref().unwrap_or(row.needs),
        "label": label,
        "className": class,
        "capacityState": capacity_state(row),
        "reasonCode": row.reason_code,
        "available": row.available
    })
}

fn station_projection(
    station: &StationOverview,
    capabilities: Option<&Vec<StationCapability>>,
    resource: Option<&RuntimeResourceView>,
    now_minutes: i64,
) -> serde_json::Value {
    let (state, tone) = station_state(station, now_minutes);
    let (dispatch, hint) = station_dispatch_answer(station);
    let quota = i64::from(station.daily_work_quota);
    let lit = if quota > 0 {
        (station.daily_notes_used.clamp(0, quota) * 12 + quota - 1) / quota
    } else {
        0
    };
    let (cap_summary, cap_tone) = capability_summary(capabilities);
    serde_json::json!({
        "stationRef": station.station_ref,
        "name": station.display_name,
        "stateLabel": state,
        "stateTone": tone,
        "pluginVersion": station.active_plugin_version,
        "lastSeen": station.active_last_seen_at.as_deref().map(moment_without_year),
        "dailyUsed": station.daily_notes_used,
        "dailyQuota": station.daily_work_quota,
        "quotaExhausted": station.daily_notes_used >= quota,
        "litSegments": lit,
        "claimWindowOpen": station.claim_window_open,
        "dispatchLabel": dispatch,
        "dispatchHint": hint,
        "dispatchAt": station.last_dispatch_answer_at.as_deref().map(moment_without_year),
        "capSummary": cap_summary,
        "capTone": cap_tone,
        "capabilities": capabilities.map(|rows| rows.iter().map(capability_projection).collect::<Vec<_>>()),
        "account": resource.map(account_projection),
        "selectorHealth": health_projection(station)
    })
}

fn capability_summary(capabilities: Option<&Vec<StationCapability>>) -> (String, &'static str) {
    let counts = capability_counts(capabilities);
    if !counts.readable {
        return ("读不到".to_owned(), "c-tg-neutral");
    }
    if counts.ready + counts.degraded + counts.unverified + counts.undeclared == 0 {
        return ("尚无记录".to_owned(), "c-tg-neutral");
    }
    let mut parts = vec![format!("{} 就绪", counts.ready)];
    if counts.degraded > 0 {
        parts.push(format!("{} 降级", counts.degraded));
    }
    if counts.unverified > 0 {
        parts.push(format!("{} 未验证", counts.unverified));
    }
    (
        parts.join(" · "),
        if counts.degraded > 0 {
            "c-tg-warn"
        } else {
            "c-tg-ok"
        },
    )
}

fn capability_projection(row: &StationCapability) -> serde_json::Value {
    let (word, class, detail) = match row.state() {
        CapabilityState::Ready => (
            "就绪",
            "c-cap-ready",
            row.last_success_at
                .as_deref()
                .map(|at| format!("{} 次 · 最后 {}", row.successes, moment_without_year(at)))
                .unwrap_or_else(|| format!("{} 次", row.successes)),
        ),
        CapabilityState::Degraded => (
            "降级",
            "c-cap-degraded",
            row.last_failure_at
                .as_deref()
                .map(|at| format!("插件回报跑不了 · 最后 {}", moment_without_year(at)))
                .unwrap_or_else(|| "插件回报跑不了".to_owned()),
        ),
        CapabilityState::Unverified => (
            "未验证",
            "c-cap-unverified",
            "近 7 天没派过这种活".to_owned(),
        ),
        CapabilityState::NotDeclared => (
            "不支持",
            "c-cap-undeclared",
            "在岗插件没有声明这一项".to_owned(),
        ),
    };
    serde_json::json!({
        "capability": row.capability,
        "label": capability_label(&row.capability),
        "stateWord": word,
        "className": class,
        "detail": detail,
        "executionFailures": row.execution_failures
    })
}

fn health_projection(station: &StationOverview) -> Vec<serde_json::Value> {
    use serde_json::json;
    if station.active_plugin_version.is_none() {
        return vec![];
    }
    if station.active_selector_health.is_empty() {
        return vec![json!({
            "label": "未上报自检",
            "tone": "c-tg-neutral",
            "detail": "这台安装还没有可用的页面结构自检记录：没报到过，或报来的那份没被收下。没有记录不等于页面结构正常。"
        })];
    }
    station.active_selector_health.iter().map(|record| {
        let missing = record.missing_categories.len(); let stale = record.stale_categories.len();
        let marker = if missing > 0 { format!("自检缺 {missing} 类") }
            else if stale > 0 { "自检待重验".to_owned() } else { "自检无缺失".to_owned() };
        json!({
            "label": if station.active_selector_health.len()>1 {format!("{} {marker}",platform_label(&record.platform))} else {marker},
            "tone": if missing > 0 || stale > 0 {"c-tg-warn"} else {"c-tg-ok"},
            "detail": selector_health_detail(record)
        })
    }).collect()
}

fn account_projection(resource: &RuntimeResourceView) -> serde_json::Value {
    let (tone, label) = account_binding_label(&resource.binding_state);
    serde_json::json!({
        "accepting": resource.accepting_tasks,
        "installationRef": resource.installation_ref,
        "installationLabel": short_ref(resource.installation_ref),
        "accountRef": resource.account_ref,
        "credentialValid": resource.has_valid_credential,
        "bindingState": resource.binding_state,
        "bindingLabel": label,
        "bindingTone": tone,
        "observed": resource.eligibility_observed_at.as_deref().map(moment_without_year),
        "eligibilityReason": resource.eligibility_reason_code.as_deref().unwrap_or("account_unknown"),
        "hasLiveLease": resource.account_has_live_lease
    })
}

fn drawer_projection(
    overview: Option<&RuntimeCapacityOverview>,
    stations: Option<&[StationOverview]>,
    capabilities: &CapabilityMatrix,
) -> serde_json::Value {
    use serde_json::json;
    let (mut read, mut picked, mut capability_failures, mut execution_failures) =
        (0usize, 0i64, 0i64, 0i64);
    for rows in capabilities.values() {
        read += 1;
        for row in rows {
            picked += row.successes;
            capability_failures += row.capability_failures;
            execution_failures += row.execution_failures;
        }
    }
    let coverage = stations
        .map(|stations| format!("{read} / {} 台工位有读数", stations.len()))
        .unwrap_or_else(|| format!("{read} 台有读数，工位总数此刻读不到"));
    let oldest = overview.map(|overview| {
        overview
            .dispatch_backlog
            .iter()
            .filter_map(|row| row.oldest_ready_at.as_deref())
            .min()
            .map(moment_without_year)
            .unwrap_or("没有在等的一单")
    });
    let pauses = overview.map(|overview| {
        overview
            .risk_pauses
            .iter()
            .map(|pause| {
                json!({
                    "reason":pause.reason,
                    "by":pause.paused_by,
                    "at":moment_without_year(&pause.paused_at)
                })
            })
            .collect::<Vec<_>>()
    });
    json!({
        "coverage": coverage,
        "readCount": read,
        "picked": picked,
        "capabilityFailures": capability_failures,
        "executionFailures": execution_failures,
        "oldest": oldest,
        "pauses": pauses,
        "dispatched": overview.map(|overview|moment_or_never(overview.patrol.last_dispatched_at.as_deref())).unwrap_or_else(||"读不到".to_owned()),
        "succeeded": overview.map(|overview|moment_or_never(overview.patrol.last_succeeded_at.as_deref())).unwrap_or_else(||"读不到".to_owned())
    })
}

#[cfg(test)]
mod projection_tests {
    use super::*;

    fn station() -> StationOverview {
        StationOverview {
            station_ref: Uuid::new_v4(),
            display_name: "合成工位".to_owned(),
            daily_work_quota: 200,
            claim_window_open: false,
            active_plugin_version: Some("synthetic".to_owned()),
            active_browser_label: None,
            active_last_seen_at: Some("2026-10-16 09:00".to_owned()),
            superseded_count: 7,
            daily_notes_used: 0,
            active_selector_health: vec![],
            last_dispatch_answer_at: None,
            last_dispatch_answer_code: None,
            last_dispatch_answer_reason: None,
        }
    }

    #[test]
    fn unavailable_reads_and_confirmed_empty_rosters_remain_distinct_in_json() {
        let unreadable = runtime_projection(None, None, &CapabilityMatrix::new(), None, 0);
        let empty = runtime_projection(None, Some((&[], &[])), &CapabilityMatrix::new(), None, 0);
        assert!(unreadable["stations"].is_null());
        assert!(unreadable["unclaimed"].is_null());
        assert!(unreadable["meta"]["unclaimedInstallations"].is_null());
        assert_eq!(empty["stations"], serde_json::json!([]));
        assert_eq!(empty["meta"]["unclaimedInstallations"], 0);
        assert!(empty["meta"]["vacantStations"].is_null());
        assert_eq!(unreadable["board"]["state"], "运行状态读不到");
    }

    #[test]
    fn recovery_gate_is_not_lost_when_capacity_is_unreadable() {
        let lanes =
            super::super::collection::collection_control_surface_view::recovery_runtime_lanes();
        let control = RuntimeControl {
            lanes: &lanes,
            resources: &[],
            account_observation_available: false,
        };
        let value = runtime_projection(None, None, &CapabilityMatrix::new(), Some(&control), 0);
        assert_eq!(value["board"]["state"], "采集暂停");
        assert_eq!(value["board"]["badge"], "恢复阶段");
        for lane in value["lanes"].as_array().expect("recovery lanes") {
            assert_eq!(lane["available"], false);
            assert_eq!(lane["label"], "暂不派发");
        }
    }

    #[test]
    fn connection_freshness_and_unreported_selector_health_keep_their_original_boundaries() {
        let mut station = station();
        let seen = minutes_since_epoch("2026-10-16 09:00").expect("synthetic time");
        let at_boundary = station_projection(
            &station,
            None,
            None,
            seen + i64::from(CONTROL_FRESHNESS_MINUTES),
        );
        let expired = station_projection(
            &station,
            None,
            None,
            seen + i64::from(CONTROL_FRESHNESS_MINUTES) + 1,
        );
        assert_eq!(at_boundary["stateLabel"], "在岗");
        assert_eq!(expired["stateLabel"], "失联");
        assert_eq!(expired["selectorHealth"][0]["label"], "未上报自检");
        assert!(expired["account"].is_null());
        assert!(expired["capabilities"].is_null());
        assert_eq!(expired["dispatchLabel"], "从未问过活");
        station.active_plugin_version = None;
        let vacant = station_projection(&station, Some(&vec![]), None, seen);
        assert_eq!(vacant["stateLabel"], "空缺");
        assert_eq!(vacant["selectorHealth"], serde_json::json!([]));
        assert_eq!(vacant["capSummary"], "尚无记录");
    }

    #[test]
    fn execution_failure_does_not_become_capability_degradation() {
        let mut row = StationCapability {
            capability: "author_profile".to_owned(),
            declared: true,
            successes: 0,
            last_success_at: None,
            capability_failures: 0,
            execution_failures: 3,
            last_failure_at: None,
        };
        let value = capability_projection(&row);
        assert_eq!(value["stateWord"], "未验证");
        assert_eq!(value["executionFailures"], 3);
        row.successes = 1;
        assert_eq!(capability_projection(&row)["stateWord"], "就绪");
        row.capability_failures = 1;
        assert_eq!(capability_projection(&row)["stateWord"], "降级");
    }
}
