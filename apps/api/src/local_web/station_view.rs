//! COLLECTION-001 · 执行工位页。
//!
//! 单独成文件，因为 `collection.rs` 已超模块行数上限，再长会让一个已知问题更糟。
//!
//! 这一页允许出现工程执行细节（工位、租约、心跳、配额），但它回答的不是「有哪些机器」。
//! 它回答**准入第 5 问**——「是否有兼容工位、账号、预算与风险余量」。前四问的答案在别的
//! 页面，第 5 问的答案只在这里。因此页面按「能不能接活 → 缺哪一样 → 是谁 → 按什么边界跑」
//! 四层组织，而不是按对象清单。
//!
//! **这一页此前用写死的句子回答接通状态**：`调度器未接通`、`准入第 5 问尚未接上工位`、
//! `队列、租约、回执尚不存在`。三句写下时都是真的，之后三样全部接通，句子一个字没变——
//! 一个不会随系统状态改变的状态区块，等于一个永远不会响的警报器。现在没有一句接通状态
//! 是手写的，全部由 `read_runtime_capacity` 的事实推出。

use linggan_evidence::{
    ACCOUNT_CHECK_NOT_CONNECTED, LaneVerdict, RuntimeCapacityOverview, StationOverview,
    UnclaimedInstallation,
};

/// 登记工位与认领安装都不消耗任何平台访问——它们只是在本地记下「这台机器是谁」。
/// 因此这一页允许出现真实可点的按钮，而会触发平台访问的控件仍然不存在。
const NO_PLATFORM_ACCESS_NOTE: &str =
    "登记工位、开认领窗口与认领安装都只写本地记录，不访问任何平台。";

const EMPTY_STATE_OPEN: &str = "<section class=\"c-empty c-empty-engineering\">";
const EMPTY_STATE_CLOSE: &str = "</section>";

/// 渲染整页工作区，替换掉原来的空态。
///
/// 即使一台工位都没有也要替换：空态说的是「等工程」，而登记工位现在真的可以做了，
/// 继续显示「等工程」就是在说假话。
pub fn render_runtime(
    base: &str,
    overview: Option<&RuntimeCapacityOverview>,
    stations: &[StationOverview],
    unclaimed: &[UnclaimedInstallation],
    error: Option<&str>,
) -> String {
    let Some(open) = base.find(EMPTY_STATE_OPEN) else {
        return base.to_owned();
    };
    let Some(close_offset) = base[open..].find(EMPTY_STATE_CLOSE) else {
        return base.to_owned();
    };
    let close = open + close_offset + EMPTY_STATE_CLOSE.len();

    let body = format!(
        r#"<div class="c-runtime">
              {verdict}
              <div class="c-runtime-split">
                <div class="c-runtime-main">{roster}</div>
                <aside class="c-runtime-aside">{console}{bounds}</aside>
              </div>
            </div>"#,
        verdict = verdict_markup(overview, stations),
        roster = roster_markup(stations, unclaimed),
        console = console_markup(stations, error),
        bounds = bounds_markup(overview),
    );

    format!(
        "{before}{body}{after}",
        before = &base[..open],
        after = &base[close..],
    )
}

// ---------------------------------------------------------------------------
// 第一层与第二层：能不能接活，缺哪一样
// ---------------------------------------------------------------------------

/// 第一层是一个**判断**，不是一串读数。
///
/// 数字（20/200、1 台在岗）是支撑判断的证据，不是主角；把它们当主角，读的人得自己在脑子里
/// 做一次判定，而那次判定服务端已经做过了——`Capacity` 枚举就是它的结论。
fn verdict_markup(
    overview: Option<&RuntimeCapacityOverview>,
    stations: &[StationOverview],
) -> String {
    let Some(overview) = overview else {
        // 读不到与「没有」是两个不同的说法，绝不能合并。
        return r#"<section class="c-verdict c-verdict-unknown">
                <div class="c-verdict-head">
                  <b class="c-verdict-state">读不到</b>
                  <p class="c-verdict-why">现在读不到产能判定。这不表示系统接不了活，只表示这一页此刻答不出——两者的处置不同。</p>
                </div>
              </section>"#
            .to_owned();
    };

    let blocked = overview.blocking_reasons();
    let (variant, state) = if blocked.is_empty() {
        ("c-verdict-ok", "能接活")
    } else if overview.any_lane_available() {
        ("c-verdict-partial", "部分接不了活")
    } else {
        ("c-verdict-blocked", "接不了活")
    };

    // 接不了活时，逐条说明是哪条 lane、缺哪一样。两条 lane 可能因为不同原因停下
    // （一条缺能力、一条额度满），压成一句「资源不足」正是 Capacity 从布尔换成枚举
    // 要避免的事：一个说不出缺哪一样的拒绝，会让人翻遍四个子系统。
    let why = if blocked.is_empty() {
        "<p class=\"c-verdict-why\">准入第 5 问的可判定项全部答「是」。这不代表现在有活在跑，只代表申请到这一问不会被挡下。</p>".to_owned()
    } else {
        let items: String = blocked
            .iter()
            .map(|(lane, reason)| {
                format!(
                    "<li><b>{lane}</b>{reason}</li>",
                    lane = escape(lane),
                    reason = escape(reason),
                )
            })
            .collect();
        format!("<ul class=\"c-verdict-why-list\">{items}</ul>")
    };

    format!(
        r#"<section class="c-verdict {variant}">
              <div class="c-verdict-head">
                <b class="c-verdict-state">{state}</b>
                {why}
              </div>
              {lanes}
              {factors}
            </section>"#,
        lanes = lane_markup(&overview.lanes),
        factors = factor_markup(overview, stations),
    )
}

/// 两条 lane 分别判定。合并成一次会让「深度建档接不了、巡检还能跑」这种真实情况消失。
fn lane_markup(lanes: &[LaneVerdict]) -> String {
    let rows: String = lanes
        .iter()
        .map(|lane| {
            let state_class = if lane.is_available() {
                "c-lane-ok"
            } else {
                "c-lane-blocked"
            };
            format!(
                r#"<div class="c-lane">
                    <div class="c-lane-name"><b>{zh}</b><span>{needs}</span></div>
                    <div class="c-lane-state {state_class}">{verdict}</div>
                  </div>"#,
                zh = escape(lane.zh),
                needs = escape(lane.needs),
                verdict = escape(lane.verdict_label()),
            )
        })
        .collect();
    format!(r#"<div class="c-lanes">{rows}</div>"#)
}

/// 第二层：第 5 问的四个分项各自的当前值。
///
/// **账号那一项永远显示「不参与判定」**，因为 `establish_capacity` 检查的是风险、工位、
/// 能力、预算四样，账号不在其中（DECISION-04 独立立项）。四项里有一项从未被检查，
/// 却显示成四项齐备，就是用视觉便利改写资格。
fn factor_markup(overview: &RuntimeCapacityOverview, stations: &[StationOverview]) -> String {
    let station_value = format!(
        "{staffed}/{registered}",
        staffed = overview.staffed_stations,
        registered = overview.registered_stations,
    );
    // 配额按工位计（Mog 于 2026-08-27 确认）。多台工位时不加总成一个数——各自的额度是
    // 各自的边界，加总出来的「400」不对应任何一个真实上限。
    let budget_value = match stations {
        [] => "—".to_owned(),
        [single] => format!(
            "{used}/{quota}",
            used = single.daily_notes_used,
            quota = single.daily_work_quota
        ),
        many => format!("{} 台各计", many.len()),
    };
    let risk_value = match overview.risk_pauses.len() {
        0 => "无".to_owned(),
        count => format!("{count} 条生效"),
    };

    let cells = [
        ("工位", station_value, "在岗 / 已登记", true),
        (
            "账号",
            "不参与判定".to_owned(),
            "独立立项 · DECISION-04",
            false,
        ),
        ("今日预算", budget_value, "已入库 / 每日上限", true),
        ("风险余量", risk_value, "生效中的风险暂停", true),
    ];

    let rendered: String = cells
        .iter()
        .map(|(label, value, note, checked)| {
            let unchecked = if *checked { "" } else { " c-factor-unchecked" };
            format!(
                r#"<div class="c-factor{unchecked}">
                    <dt>{label}</dt>
                    <dd>{value}</dd>
                    <p>{note}</p>
                  </div>"#,
                label = escape(label),
                value = escape(value),
                note = escape(note),
            )
        })
        .collect();

    format!(
        r#"<dl class="c-factors">{rendered}</dl>
           <p class="c-factors-caveat">{caveat}</p>"#,
        caveat = escape(ACCOUNT_CHECK_NOT_CONNECTED),
    )
}

// ---------------------------------------------------------------------------
// 第三层：构成这些值的对象
// ---------------------------------------------------------------------------

fn roster_markup(stations: &[StationOverview], unclaimed: &[UnclaimedInstallation]) -> String {
    let mut sections = String::new();

    sections.push_str(&group(
        "工位",
        stations.len(),
        // 这一页存在的第一性理由就写在这句话里：让「这台机器」这个身份活得比插件长。
        // 内容工作台把工位与安装混成一个对象，13 台注册变成 11 台僵尸。
        None,
        &if stations.is_empty() {
            r#"<p class="c-roster-empty">还没有登记任何工位。这不是「等工程」——登记一台是你现在就能做的事，右边就是入口。</p>"#.to_owned()
        } else {
            stations.iter().map(station_row).collect::<String>()
        },
    ));

    if !unclaimed.is_empty() {
        sections.push_str(&group(
            "待认领安装",
            unclaimed.len(),
            Some("这些插件报到了，但还没人说它们是哪台工位。它们不会被派活。认领窗口开着时新安装会自动归位，关着就停在这里等你指认。"),
            &unclaimed
                .iter()
                .map(|installation| unclaimed_row(installation, stations))
                .collect::<String>(),
        ));
    }

    sections
}

/// 一组对象。说明句只在这一处出现一次。
///
/// 此前「工位由人登记，插件安装由插件自报……」这段话在页面上出现了两次，几乎一字不差：
/// 标题下一次、组标题下再一次。它真正解释的是「为什么一个插件报到了不等于一台工位」，
/// 所以它属于待认领安装那一组，不属于页面开头。
fn group(title: &str, count: usize, note: Option<&str>, rows: &str) -> String {
    let note = note.map_or_else(String::new, |text| {
        format!(
            r#"<p class="c-roster-note">{text}</p>"#,
            text = escape(text)
        )
    });
    format!(
        r#"<section class="c-roster">
              <div class="c-roster-head">
                <div class="c-roster-count"><b>{count}</b><span>{title}</span></div>
                {note}
              </div>
              <div class="c-roster-rows">{rows}</div>
            </section>"#,
        title = escape(title),
    )
}

fn station_row(station: &StationOverview) -> String {
    // 在岗与空缺是两件不同的事，不能都渲染成一片灰。
    let (state_label, state_class, plugin_line) = match station.active_plugin_version.as_deref() {
        Some(version) => {
            let browser = station
                .active_browser_label
                .as_deref()
                .unwrap_or("未知浏览器");
            let seen = station.active_last_seen_at.as_deref().unwrap_or("UNKNOWN");
            (
                "在岗",
                "c-state-staffed",
                format!("插件 {version} · {browser} · 最后报到 {seen}"),
            )
        }
        // 没有在岗安装不等于工位坏了，只是现在没插件连着它。
        None => (
            "空缺",
            "c-state-vacant",
            "当前没有插件安装认领这台工位".to_owned(),
        ),
    };
    // 换过几次插件要看得见。内容工作台正是让这个数字隐身，才变成 11 台僵尸工位。
    let history = match station.superseded_count {
        0 => "首次安装".to_owned(),
        count => format!("换过 {count} 次插件"),
    };
    let window = if station.claim_window_open {
        "认领窗口开着"
    } else {
        "认领窗口已关"
    };
    // 额度是这一行唯一能告诉人「今天还能干多少活」的数，因此它是第二视觉重量，
    // 不与「换过几次插件」同档。用尽时它也是准入 DailyQuotaCommitted 的来源。
    let exhausted = station.daily_notes_used >= i64::from(station.daily_work_quota);
    let quota_class = if exhausted {
        "c-quota c-quota-full"
    } else {
        "c-quota"
    };

    format!(
        r#"<div class="c-station-row">
                <div class="c-station-state {state_class}">{state}</div>
                <div class="c-station-name"><b>{name}</b><span>{plugin}</span></div>
                <div class="{quota_class}"><b>{used}</b><span>/{quota} 篇</span><em>今日已入库</em></div>
                <div class="c-station-meta"><span>{history}</span><span>{window}</span></div>
                <form class="c-station-retire" method="post" action="/collection/runtime/retire">
                  <input type="hidden" name="station_ref" value="{station_ref}" />
                  <button class="c-btn-quiet" type="submit">停用</button>
                </form>
              </div>"#,
        station_ref = station.station_ref,
        name = escape(&station.display_name),
        plugin = escape(&plugin_line),
        history = escape(&history),
        window = escape(window),
        used = station.daily_notes_used,
        quota = station.daily_work_quota,
        state = escape(state_label),
    )
}

fn unclaimed_row(installation: &UnclaimedInstallation, stations: &[StationOverview]) -> String {
    let browser = installation
        .browser_label
        .as_deref()
        .unwrap_or("未知浏览器");
    // 没有已登记的工位时不给认领控件：没有可认领到的去处，给了也是点了没反应。
    let claim = if stations.is_empty() {
        r#"<div class="c-station-state c-state-waiting">待认领</div>"#.to_owned()
    } else {
        let options: String = stations
            .iter()
            .map(|station| {
                format!(
                    r#"<option value="{value}">{name}</option>"#,
                    value = station.station_ref,
                    name = escape(&station.display_name),
                )
            })
            .collect();
        format!(
            r#"<form class="c-station-claim" method="post" action="/collection/runtime/claims">
                  <input type="hidden" name="installation_ref" value="{installation}" />
                  <select name="station_ref" aria-label="认领到哪台工位">{options}</select>
                  <button class="c-btn-quiet" type="submit">认领</button>
                </form>"#,
            installation = installation.installation_ref,
        )
    };
    format!(
        r#"<div class="c-station-row c-station-row-unclaimed">
                <div class="c-station-state c-state-waiting">未归位</div>
                <div class="c-station-name"><b>插件 {version}</b><span>{browser} · 首次报到 {first_seen}</span></div>
                {claim}
              </div>"#,
        version = escape(&installation.plugin_version),
        browser = escape(browser),
        first_seen = escape(&installation.first_seen_at),
    )
}

// ---------------------------------------------------------------------------
// 右栏：主动作与开发期工具
// ---------------------------------------------------------------------------

fn console_markup(stations: &[StationOverview], error: Option<&str>) -> String {
    format!(
        r#"<section class="c-console">
              <h2 class="c-console-title">登记一台工位</h2>
              {failure}
              <form class="c-console-form" method="post" action="/collection/runtime/stations">
                <label for="station-name">名字要能让你一眼认出是哪台机器</label>
                <input id="station-name" name="display_name" required maxlength="60"
                       placeholder="例如：MacBook Chrome" />
                <button class="c-btn-primary" type="submit">登记</button>
              </form>
              <p class="c-console-note">{note}</p>
              {dev}
            </section>"#,
        failure = failure_markup(error),
        note = escape(NO_PLATFORM_ACCESS_NOTE),
        dev = dev_tools_markup(stations),
    )
}

/// 上一次动作失败时说明原因。
///
/// 失败必须看得见。跳转回来却什么都不说，会让人以为动作成功了——那比「点了没反应」更糟，
/// 因为它会让人以为系统里有一台并不存在的工位。
fn failure_markup(error: Option<&str>) -> String {
    let Some(code) = error else {
        return String::new();
    };
    let explanation = match code {
        "station_rejected" => {
            "工位没有登记成功。最常见的原因是名字与某台在册工位重复——名字要能让你一眼认出是哪台机器，因此不允许重名。"
        }
        "claim_window_rejected" => "认领窗口没有打开。该工位可能已被停用。",
        "close_window_rejected" => "认领窗口没有关闭。它可能已经自己过期了。",
        "retire_rejected" => "工位没有停用成功。它可能已经处于停用状态。",
        "claim_rejected" => "认领没有成功。这台工位可能已有在岗安装，或该安装已被取代。",
        _ => "上一次动作没有完成。",
    };
    format!(
        r#"<p class="c-console-failure"><b>没有完成</b>{explanation}</p>"#,
        explanation = escape(explanation),
    )
}

/// 开认领窗口与提前关窗口是开发期用的东西，不该与主动作同级。
///
/// 默认折叠。没有工位时整块不渲染——没有可开窗口的对象。
fn dev_tools_markup(stations: &[StationOverview]) -> String {
    if stations.is_empty() {
        return String::new();
    }
    let options: String = stations
        .iter()
        .map(|station| {
            format!(
                r#"<option value="{value}">{name}</option>"#,
                value = station.station_ref,
                name = escape(&station.display_name),
            )
        })
        .collect();
    let open: Vec<&StationOverview> = stations
        .iter()
        .filter(|station| station.claim_window_open)
        .collect();
    // 关窗口只在确实有窗口开着时出现——没有开着的窗口就没有可关的东西。
    let close = if open.is_empty() {
        String::new()
    } else {
        let open_options: String = open
            .iter()
            .map(|station| {
                format!(
                    r#"<option value="{value}">{name}</option>"#,
                    value = station.station_ref,
                    name = escape(&station.display_name),
                )
            })
            .collect();
        format!(
            r#"<form class="c-console-form c-console-form-quiet" method="post" action="/collection/runtime/close-window">
                  <label for="close-station">提前关窗口</label>
                  <select id="close-station" name="station_ref">{open_options}</select>
                  <button class="c-btn-secondary" type="submit">关窗口</button>
                </form>"#
        )
    };

    format!(
        r#"<details class="c-dev-tools">
              <summary>开发期工具</summary>
              <form class="c-console-form c-console-form-quiet" method="post" action="/collection/runtime/claim-window">
                <label for="claim-station">开认领窗口</label>
                <select id="claim-station" name="station_ref">{options}</select>
                <select name="valid_for_hours" aria-label="窗口时长">
                  <option value="24">24 小时</option>
                  <option value="8">8 小时</option>
                  <option value="1">1 小时</option>
                </select>
                <button class="c-btn-secondary" type="submit">开窗口</button>
              </form>
              {close}
            </details>"#
    )
}

// ---------------------------------------------------------------------------
// 第四层：正在按什么边界跑
// ---------------------------------------------------------------------------

/// 租约与巡检：系统在无人值守时按什么边界跑。
///
/// **这里没有「调度器是否在运行」。** 系统没有任何进程心跳记录，那件事这一页读不到；
/// 写一个「运行中」出来就是编的。能说的只有真实痕迹：派过没有、成了没有、下面还有几个到期。
fn bounds_markup(overview: Option<&RuntimeCapacityOverview>) -> String {
    let Some(overview) = overview else {
        return String::new();
    };

    let leases = if overview.live_leases.is_empty() {
        r#"<p class="c-bounds-empty">当前没有活着的租约。没有任何工位被允许执行任何工单。</p>"#
            .to_owned()
    } else {
        overview
            .live_leases
            .iter()
            .map(|lease| {
                // 「已展开成任务」与「只是许可」必须分开：租约是许可，不是活。
                let stage = if lease.has_task {
                    "已展开成任务"
                } else {
                    "尚未展开成任务"
                };
                format!(
                    r#"<div class="c-bound-row">
                        <div class="c-bound-name"><b>{target}</b><span>{lane} · {station}</span></div>
                        <div class="c-bound-meta"><span>{stage}</span><span>到期 {expires}</span></div>
                      </div>"#,
                    target = escape(&lease.target_label),
                    lane = escape(&lease.lane),
                    station = escape(&lease.station_name),
                    stage = escape(stage),
                    expires = escape(&lease.expires_at),
                )
            })
            .collect::<String>()
    };

    let patrol = &overview.patrol;
    // 「在跑但没活可派」与「根本没在跑」处置完全不同，页面必须把这个区别说出来，
    // 而不是把两者都写成「未接通」——那会让人去修一个没有坏的东西。
    let patrol_note = if patrol.silent_because_nothing_to_patrol() {
        "没有任何目标开着巡检，因此调度即使正在运行也不会有动静。这一页读不到调度进程本身的心跳，只能看见它留下的痕迹。"
    } else {
        "到期判据与调度自己用的是同一个式子。这一页读不到调度进程本身的心跳，只能看见它留下的痕迹。"
    };

    format!(
        r#"<section class="c-bounds">
              <h2 class="c-bounds-title">正在按什么边界跑</h2>
              <div class="c-bounds-group">
                <h3>活着的租约</h3>
                {leases}
              </div>
              <div class="c-bounds-group">
                <h3>巡检</h3>
                <dl class="c-bounds-grid">
                  <div><dt>开着巡检</dt><dd>{monitoring}/{total}</dd></div>
                  <div><dt>此刻到期</dt><dd>{due}</dd></div>
                  <div><dt>最近派出</dt><dd>{dispatched}</dd></div>
                  <div><dt>最近成功</dt><dd>{succeeded}</dd></div>
                </dl>
                <p class="c-bounds-note">{patrol_note}</p>
              </div>
            </section>"#,
        monitoring = patrol.monitoring_targets,
        total = patrol.total_targets,
        due = patrol.due_now,
        // 从未派出与派出过是两个事实，前者不是 0 也不是未知，是「从未」。
        dispatched = escape(patrol.last_dispatched_at.as_deref().unwrap_or("从未")),
        succeeded = escape(patrol.last_succeeded_at.as_deref().unwrap_or("从未")),
        patrol_note = escape(patrol_note),
    )
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use linggan_contracts::Capacity;
    use linggan_evidence::{ActiveRiskPause, LiveLease, PatrolOutlook};
    use uuid::Uuid;

    fn base() -> String {
        format!("before{EMPTY_STATE_OPEN}这一栏不需要你做任何事{EMPTY_STATE_CLOSE}after")
    }

    fn station(active_version: Option<&str>, superseded: i64) -> StationOverview {
        StationOverview {
            station_ref: Uuid::new_v4(),
            display_name: "MacBook Chrome".to_owned(),
            daily_work_quota: 200,
            claim_window_open: false,
            active_plugin_version: active_version.map(str::to_owned),
            active_browser_label: Some("Chrome".to_owned()),
            active_last_seen_at: Some("2026-08-27 02:58".to_owned()),
            superseded_count: superseded,
            daily_notes_used: 0,
        }
    }

    fn overview(lanes: Vec<LaneVerdict>) -> RuntimeCapacityOverview {
        RuntimeCapacityOverview {
            lanes,
            risk_pauses: Vec::new(),
            registered_stations: 1,
            staffed_stations: 1,
            patrol: PatrolOutlook {
                total_targets: 0,
                monitoring_targets: 0,
                due_now: 0,
                last_dispatched_at: None,
                last_succeeded_at: None,
            },
            live_leases: Vec::new(),
        }
    }

    fn lane(zh: &'static str, capacity: Capacity) -> LaneVerdict {
        LaneVerdict {
            zh,
            lane: "deep_archive",
            needs: "作品清单",
            capacity,
        }
    }

    fn available() -> Capacity {
        Capacity::Available {
            station_ref: "s".to_owned(),
        }
    }

    #[test]
    fn nothing_registered_still_offers_the_one_thing_the_person_can_do() {
        // The empty state this replaces says "这一栏不需要你做任何事". Registering a station
        // is now a real action, so leaving that text in place would be a lie.
        let rendered = render_runtime(&base(), None, &[], &[], None);
        assert!(rendered.contains("/collection/runtime/stations"));
        assert!(!rendered.contains("这一栏不需要你做任何事"));
        // Nothing exists to open a window on or claim yet, so neither control is offered.
        assert!(!rendered.contains("/collection/runtime/claim-window"));
        assert!(!rendered.contains("c-station-claim"));
    }

    #[test]
    fn the_page_never_writes_a_connection_state_by_hand() {
        // 这一页的整个存在理由：接通状态必须来自事实，不能是写死的句子。三句旧文案
        // 都曾是真的，之后三样全部接通而句子一个字没变。
        let rendered = render_runtime(
            &base(),
            Some(&overview(vec![lane("巡检", available())])),
            &[station(Some("0.5.2"), 0)],
            &[],
            None,
        );
        for frozen in [
            "调度器未接通",
            "尚未接上工位",
            "队列、租约、回执",
            "尚不存在",
        ] {
            assert!(
                !rendered.contains(frozen),
                "「{frozen}」是写死的接通状态，必须由事实推出"
            );
        }
    }

    #[test]
    fn a_blocked_lane_names_the_missing_resource() {
        // 一个说不出「缺哪一样」的拒绝，会让人翻遍四个子系统——Capacity 从布尔换成
        // 枚举就是为了这个。
        let rendered = render_runtime(
            &base(),
            Some(&overview(vec![lane(
                "深度建档",
                Capacity::MissingCapabilities {
                    missing: vec!["content_detail".to_owned()],
                },
            )])),
            &[station(Some("0.5.2"), 0)],
            &[],
            None,
        );
        assert!(rendered.contains("接不了活"));
        assert!(rendered.contains("content_detail"));
    }

    #[test]
    fn one_lane_running_is_not_reported_as_total_failure() {
        // 深度建档接不了、巡检还能跑，是真实情况。合并成一句会让它消失。
        let rendered = render_runtime(
            &base(),
            Some(&overview(vec![
                lane("深度建档", Capacity::NoStaffedStation),
                lane("巡检", available()),
            ])),
            &[],
            &[],
            None,
        );
        assert!(rendered.contains("部分接不了活"));
    }

    #[test]
    fn the_account_half_of_question_five_is_never_shown_as_checked() {
        // establish_capacity 检查风险、工位、能力、预算四样，账号不在其中
        // （DECISION-04 独立立项）。显示成四项齐备就是用视觉便利改写资格。
        let rendered = render_runtime(
            &base(),
            Some(&overview(vec![lane("巡检", available())])),
            &[],
            &[],
            None,
        );
        assert!(rendered.contains("不参与判定"));
        assert!(rendered.contains("c-factor-unchecked"));
    }

    #[test]
    fn unreadable_capacity_is_not_reported_as_no_capacity() {
        // 「读不到」与「接不了活」是两个不同的说法，处置也不同。
        let rendered = render_runtime(&base(), None, &[], &[], None);
        // 断言判定词本身，不是文案里出现过哪个词：说明句里正写着「这不表示系统接不了活」。
        assert!(rendered.contains(r#"c-verdict-state">读不到"#));
        assert!(!rendered.contains("c-verdict-blocked"));
        assert!(!rendered.contains("c-verdict-ok"));
    }

    #[test]
    fn many_reinstalls_stay_one_station_with_a_visible_count() {
        // The failure this whole design exists to prevent: 内容工作台 turned 11 reinstalls
        // into 11 zombie stations. Here they must remain one station whose history is stated.
        let rendered = render_runtime(&base(), None, &[station(Some("0.5.2"), 11)], &[], None);
        assert_eq!(rendered.matches("c-station-row").count(), 1);
        assert!(rendered.contains("换过 11 次插件"));
        assert!(rendered.contains("在岗"));
    }

    #[test]
    fn a_failed_action_says_so_instead_of_looking_like_it_worked() {
        // 表单失败后只是跳转回来、什么都不说，会让人以为动作成功了。
        let rendered = render_runtime(&base(), None, &[], &[], Some("station_rejected"));
        assert!(rendered.contains("没有完成"));
        assert!(rendered.contains("重复"));
        assert!(!render_runtime(&base(), None, &[], &[], None).contains("没有完成"));
    }

    #[test]
    fn a_station_with_no_plugin_reads_as_vacant_not_broken() {
        let rendered = render_runtime(&base(), None, &[station(None, 0)], &[], None);
        assert!(rendered.contains("空缺"));
        assert!(rendered.contains("当前没有插件安装认领这台工位"));
        // A vacant station must never be dressed up with an invented plugin version.
        assert!(!rendered.contains("最后报到"));
    }

    #[test]
    fn an_unclaimed_install_is_listed_without_a_station() {
        let unclaimed = UnclaimedInstallation {
            installation_ref: Uuid::new_v4(),
            plugin_version: "0.5.0".to_owned(),
            browser_label: None,
            first_seen_at: "2026-08-27 02:58".to_owned(),
        };
        let rendered = render_runtime(&base(), None, &[], &[unclaimed], None);
        assert!(rendered.contains("未归位"));
        assert!(rendered.contains("未知浏览器"));
    }

    #[test]
    fn the_explanation_of_station_versus_installation_appears_once() {
        // 这段话此前出现两次，几乎一字不差。它解释的是「插件报到不等于一台工位」，
        // 所以它属于待认领那一组，页面开头不再重复一遍。
        let unclaimed = UnclaimedInstallation {
            installation_ref: Uuid::new_v4(),
            plugin_version: "0.5.0".to_owned(),
            browser_label: None,
            first_seen_at: "2026-08-27 02:58".to_owned(),
        };
        let rendered = render_runtime(
            &base(),
            None,
            &[station(Some("0.5.2"), 0)],
            &[unclaimed],
            None,
        );
        assert_eq!(rendered.matches("它们不会被派活").count(), 1);
    }

    #[test]
    fn a_lease_is_never_reported_as_work_in_progress() {
        // 租约是许可，不是活。展开成任务与否必须分得开。
        let mut data = overview(vec![lane("巡检", available())]);
        data.live_leases = vec![LiveLease {
            station_name: "MacBook Chrome".to_owned(),
            lane: "patrol".to_owned(),
            target_label: "某创作者".to_owned(),
            expires_at: "2026-08-29 09:00".to_owned(),
            has_task: false,
        }];
        let rendered = render_runtime(&base(), Some(&data), &[], &[], None);
        assert!(rendered.contains("尚未展开成任务"));
    }

    #[test]
    fn a_silent_scheduler_with_nothing_to_do_is_not_reported_as_broken() {
        // 「在跑但没活可派」与「根本没在跑」处置相反，都写成「未接通」会让人去修一个
        // 没有坏的东西。页面也绝不声称读得到进程心跳。
        let rendered = render_runtime(
            &base(),
            Some(&overview(vec![lane("巡检", available())])),
            &[],
            &[],
            None,
        );
        assert!(rendered.contains("没有任何目标开着巡检"));
        assert!(rendered.contains("读不到调度进程本身的心跳"));
    }

    #[test]
    fn never_patrolled_reads_as_never_not_as_zero() {
        // 从未派出不是 0，也不是未知，是「从未」。
        let rendered = render_runtime(
            &base(),
            Some(&overview(vec![lane("巡检", available())])),
            &[],
            &[],
            None,
        );
        assert!(rendered.contains("从未"));
    }

    #[test]
    fn an_active_risk_pause_is_counted_in_the_factor_row() {
        let mut data = overview(vec![lane("巡检", available())]);
        data.risk_pauses = vec![ActiveRiskPause {
            platform: Some("xhs".to_owned()),
            lane: None,
            reason: "演练".to_owned(),
            paused_by: "person".to_owned(),
            paused_at: "2026-08-29 07:00".to_owned(),
        }];
        let rendered = render_runtime(&base(), Some(&data), &[], &[], None);
        assert!(rendered.contains("1 条生效"));
    }
}
