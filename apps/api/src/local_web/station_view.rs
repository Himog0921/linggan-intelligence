//! COLLECTION-001 · 把工位与插件安装的现状注入执行工位页。
//!
//! 单独成文件，因为 `collection.rs` 已超模块行数上限，再长会让一个已知问题更糟。
//!
//! 这一页允许出现工程执行细节（工位、插件版本、心跳），但它能声称的东西很窄：
//! 一台工位在岗只说明「有个插件报到了」，不说明它跑过任何采集（INV-36）。

use linggan_evidence::{StationOverview, UnclaimedInstallation};

/// 登记工位与认领安装都不消耗任何平台访问——它们只是在本地记下「这台机器是谁」。
/// 因此这一页允许出现真实可点的按钮，而会触发平台访问的控件仍然不存在。
const NO_PLATFORM_ACCESS_NOTE: &str =
    "登记工位、开认领窗口与认领安装都只写本地记录，不访问任何平台，也不会让任何采集开始。";

const EMPTY_STATE_OPEN: &str = "<section class=\"c-empty c-empty-engineering\">";
const EMPTY_STATE_CLOSE: &str = "</section>";

/// 用工位面板替换空态。
///
/// 与观察目标那一栏不同，这里**即使一台工位都没有也要替换**：空态说的是「等工程」，
/// 而登记工位现在真的可以做了，继续显示「等工程」就是在说假话。
pub fn render_stations(
    base: &str,
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

    let mut sections = String::new();
    if !stations.is_empty() {
        let rows: String = stations.iter().map(station_row).collect();
        sections.push_str(&format!(
            r#"<section class="c-targets">
              <div class="c-targets-head">
                <div class="c-targets-count"><b>{count}</b><span>已登记工位</span></div>
                <p class="c-targets-note">工位由人登记，插件安装由插件自报。重装插件换的是安装，不是工位——授权、名字与每日额度都挂在工位上，不会被重装清掉。</p>
              </div>
              <div class="c-targets-rows">{rows}</div>
            </section>"#,
            count = stations.len(),
        ));
    }
    if !unclaimed.is_empty() {
        let rows: String = unclaimed
            .iter()
            .map(|installation| unclaimed_row(installation, stations))
            .collect();
        sections.push_str(&format!(
            r#"<section class="c-targets">
              <div class="c-targets-head">
                <div class="c-targets-count"><b>{count}</b><span>待认领安装</span></div>
                <p class="c-targets-note">这些插件报到了，但还没人说它们是哪台工位。它们不会被派活。认领窗口开着时新安装会自动归位，关着就停在这里等你指认。</p>
              </div>
              <div class="c-targets-rows">{rows}</div>
            </section>"#,
            count = unclaimed.len(),
        ));
    }
    let console = format!(
        r#"<section class="c-station-console">
              <h2>工位</h2>
              {failure}
              <p class="c-station-lede">工位由你登记，插件安装由插件自报。重装插件换的是安装，不是工位——名字、每日额度与授权都挂在工位上，不会被重装清掉。</p>
              <p class="c-station-note">{note}</p>
              <div class="c-station-forms">
                <form class="c-station-form" method="post" action="/collection/runtime/stations">
                  <label for="station-name">登记一台工位</label>
                  <input id="station-name" name="display_name" required maxlength="60"
                         placeholder="例如：MacBook Chrome" />
                  <button class="c-btn-primary" type="submit">登记</button>
                </form>
                {window_form}
              </div>
            </section>"#,
        failure = failure_markup(error),
        note = NO_PLATFORM_ACCESS_NOTE,
        window_form = claim_window_form(stations),
    );
    // 工位面板替换了原来的空态，而空态里「调度器未接通」那个事实必须留下来：
    // 一台工位在岗只说明有个插件报到了，不说明任何采集会开始（INV-36）。
    let pending = r#"<section class="c-station-pending">
              <h3>还没有接通的部分</h3>
              <dl class="c-empty-grid">
                <div><dt>调度器</dt><dd>未接通。登记工位、认领插件都不会让任何采集开始——工位在岗只说明有个插件报到了。</dd></div>
                <div><dt>准入第 5 问</dt><dd>「是否有兼容工位、账号、预算与风险余量」尚未接上工位，因此深度建档申请仍会停在这一问。</dd></div>
                <div><dt>队列、租约、回执</dt><dd>尚不存在。这一页是唯一允许出现这些工程细节的地方，接通后会长在这里。</dd></div>
              </dl>
            </section>"#;
    format!(
        "{before}{console}{sections}{pending}{after}",
        before = &base[..open],
        after = &base[close..],
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
        r#"<p class="c-station-failure"><b>没有完成</b>{explanation}</p>"#,
        explanation = escape(explanation),
    )
}

/// 开认领窗口的表单。没有工位时不渲染——没有可开窗口的对象。
fn claim_window_form(stations: &[StationOverview]) -> String {
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
    format!(
        r#"<form class="c-station-form" method="post" action="/collection/runtime/claim-window">
                  <label for="claim-station">开认领窗口（开发期用）</label>
                  <select id="claim-station" name="station_ref">{options}</select>
                  <select name="valid_for_hours" aria-label="窗口时长">
                    <option value="24">24 小时</option>
                    <option value="8">8 小时</option>
                    <option value="1">1 小时</option>
                  </select>
                  <button class="c-btn-secondary" type="submit">开窗口</button>
                </form>{close}"#,
        close = close_window_form(stations),
    )
}

/// 关窗口。只在确实有窗口开着时出现——没有开着的窗口就没有可关的东西。
fn close_window_form(stations: &[StationOverview]) -> String {
    let open: Vec<&StationOverview> = stations
        .iter()
        .filter(|station| station.claim_window_open)
        .collect();
    if open.is_empty() {
        return String::new();
    }
    let options: String = open
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
        r#"<form class="c-station-form" method="post" action="/collection/runtime/close-window">
                  <label for="close-station">提前关窗口</label>
                  <select id="close-station" name="station_ref">{options}</select>
                  <button class="c-btn-secondary" type="submit">关窗口</button>
                </form>"#
    )
}

fn station_row(station: &StationOverview) -> String {
    // 在岗与空缺是两件不同的事，不能都渲染成一片灰。
    let (state_label, plugin_line) = match station.active_plugin_version.as_deref() {
        Some(version) => {
            let browser = station
                .active_browser_label
                .as_deref()
                .unwrap_or("未知浏览器");
            let seen = station.active_last_seen_at.as_deref().unwrap_or("UNKNOWN");
            (
                "在岗",
                format!("插件 {version} · {browser} · 最后报到 {seen}"),
            )
        }
        // 没有在岗安装不等于工位坏了，只是现在没插件连着它。
        None => ("空缺", "当前没有插件安装认领这台工位".to_owned()),
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

    format!(
        r#"<div class="c-target-row">
                <div class="c-target-kind">工位</div>
                <div class="c-target-name"><b>{name}</b><span>{plugin}</span></div>
                <div class="c-target-meta"><span>{history}</span><span>{window}</span><span>今日 {used}/{quota} 篇</span></div>
                <div class="c-target-state">{state}</div>
                <form class="c-target-retire" method="post" action="/collection/runtime/retire">
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
        r#"<div class="c-target-state">待认领</div>"#.to_owned()
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
            r#"<form class="c-target-claim" method="post" action="/collection/runtime/claims">
                  <input type="hidden" name="installation_ref" value="{installation}" />
                  <select name="station_ref" aria-label="认领到哪台工位">{options}</select>
                  <button class="c-btn-quiet" type="submit">认领</button>
                </form>"#,
            installation = installation.installation_ref,
        )
    };
    format!(
        r#"<div class="c-target-row">
                <div class="c-target-kind">安装</div>
                <div class="c-target-name"><b>插件 {version}</b><span>{browser}</span></div>
                <div class="c-target-meta"><span>首次报到 {first_seen}</span></div>
                {claim}
              </div>"#,
        version = escape(&installation.plugin_version),
        browser = escape(browser),
        first_seen = escape(&installation.first_seen_at),
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
    use uuid::Uuid;

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

    #[test]
    fn nothing_registered_still_offers_the_one_thing_the_person_can_do() {
        // The empty state this replaces says "这一栏不需要你做任何事". Registering a station
        // is now a real action, so leaving that text in place would be a lie.
        let base =
            format!("before{EMPTY_STATE_OPEN}这一栏不需要你做任何事{EMPTY_STATE_CLOSE}after");
        let rendered = render_stations(&base, &[], &[], None);
        assert!(rendered.contains("/collection/runtime/stations"));
        assert!(!rendered.contains("这一栏不需要你做任何事"));
        // Nothing exists to open a window on or claim yet, so neither control is offered.
        assert!(!rendered.contains("/collection/runtime/claim-window"));
        assert!(!rendered.contains("c-target-claim"));
    }

    #[test]
    fn the_scheduler_being_absent_stays_visible_next_to_the_real_controls() {
        // A station being 在岗 must never read as "capture will now happen" (INV-36).
        let base = format!("{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}");
        let rendered = render_stations(&base, &[station(Some("2.0.101"), 0)], &[], None);
        assert!(rendered.contains("c-station-pending"));
        assert!(rendered.contains("调度器"));
    }

    #[test]
    fn many_reinstalls_stay_one_station_with_a_visible_count() {
        // The failure this whole design exists to prevent: 内容工作台 turned 11 reinstalls
        // into 11 zombie stations. Here they must remain one station whose history is stated.
        let base = format!("{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}");
        let rendered = render_stations(&base, &[station(Some("2.0.101"), 11)], &[], None);
        assert_eq!(rendered.matches("c-target-row").count(), 1);
        assert!(rendered.contains("换过 11 次插件"));
        assert!(rendered.contains("在岗"));
    }

    #[test]
    fn a_failed_action_says_so_instead_of_looking_like_it_worked() {
        // 表单失败后只是跳转回来、什么都不说，会让人以为动作成功了——那比「点了没反应」
        // 更糟：它会让人以为系统里有一台并不存在的工位。
        let base = format!("{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}");
        let rendered = render_stations(&base, &[], &[], Some("station_rejected"));
        assert!(rendered.contains("没有完成"));
        assert!(rendered.contains("重复"));
        // 没有失败时不留任何痕迹。
        assert!(!render_stations(&base, &[], &[], None).contains("没有完成"));
    }

    #[test]
    fn a_station_with_no_plugin_reads_as_vacant_not_broken() {
        let base = format!("{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}");
        let rendered = render_stations(&base, &[station(None, 0)], &[], None);
        assert!(rendered.contains("空缺"));
        assert!(rendered.contains("当前没有插件安装认领这台工位"));
        // A vacant station must never be dressed up with an invented plugin version.
        assert!(!rendered.contains("最后报到"));
    }

    #[test]
    fn an_unclaimed_install_is_listed_without_a_station() {
        let base = format!("{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}");
        let unclaimed = UnclaimedInstallation {
            installation_ref: Uuid::new_v4(),
            plugin_version: "2.0.89".to_owned(),
            browser_label: None,
            first_seen_at: "2026-08-27 02:58".to_owned(),
        };
        let rendered = render_stations(&base, &[], &[unclaimed], None);
        assert!(rendered.contains("待认领"));
        assert!(rendered.contains("未知浏览器"));
    }
}
