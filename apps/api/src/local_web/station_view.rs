//! COLLECTION-001 · 把工位与插件安装的现状注入执行工位页。
//!
//! 单独成文件，因为 `collection.rs` 已超模块行数上限，再长会让一个已知问题更糟。
//!
//! 这一页允许出现工程执行细节（工位、插件版本、心跳），但它能声称的东西很窄：
//! 一台工位在岗只说明「有个插件报到了」，不说明它跑过任何采集（INV-36）。

use linggan_evidence::{StationOverview, UnclaimedInstallation};

const EMPTY_STATE_OPEN: &str = "<section class=\"c-empty c-empty-engineering\">";
const EMPTY_STATE_CLOSE: &str = "</section>";

/// 用真实工位替换空态。都为空时原样返回：「还没接通」这个事实空态已经诚实答过了。
pub fn render_stations(
    base: &str,
    stations: &[StationOverview],
    unclaimed: &[UnclaimedInstallation],
) -> String {
    if stations.is_empty() && unclaimed.is_empty() {
        return base.to_owned();
    }
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
        let rows: String = unclaimed.iter().map(unclaimed_row).collect();
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
    format!(
        "{before}{sections}{after}",
        before = &base[..open],
        after = &base[close..],
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
                <div class="c-target-meta"><span>{history}</span><span>{window}</span><span>每日 {quota} 篇</span></div>
                <div class="c-target-state">{state}</div>
              </div>"#,
        name = escape(&station.display_name),
        plugin = escape(&plugin_line),
        history = escape(&history),
        window = escape(window),
        quota = station.daily_work_quota,
        state = escape(state_label),
    )
}

fn unclaimed_row(installation: &UnclaimedInstallation) -> String {
    let browser = installation
        .browser_label
        .as_deref()
        .unwrap_or("未知浏览器");
    format!(
        r#"<div class="c-target-row">
                <div class="c-target-kind">安装</div>
                <div class="c-target-name"><b>插件 {version}</b><span>{browser}</span></div>
                <div class="c-target-meta"><span>首次报到 {first_seen}</span></div>
                <div class="c-target-state">待认领</div>
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
        }
    }

    #[test]
    fn nothing_registered_leaves_the_honest_empty_state_alone() {
        // "还没接通" is already answered by the empty state; rendering a second empty list
        // would only add noise.
        let base = format!("before{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}after");
        assert_eq!(render_stations(&base, &[], &[]), base);
    }

    #[test]
    fn many_reinstalls_stay_one_station_with_a_visible_count() {
        // The failure this whole design exists to prevent: 内容工作台 turned 11 reinstalls
        // into 11 zombie stations. Here they must remain one station whose history is stated.
        let base = format!("{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}");
        let rendered = render_stations(&base, &[station(Some("2.0.101"), 11)], &[]);
        assert_eq!(rendered.matches("c-target-row").count(), 1);
        assert!(rendered.contains("换过 11 次插件"));
        assert!(rendered.contains("在岗"));
    }

    #[test]
    fn a_station_with_no_plugin_reads_as_vacant_not_broken() {
        let base = format!("{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}");
        let rendered = render_stations(&base, &[station(None, 0)], &[]);
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
        let rendered = render_stations(&base, &[], &[unclaimed]);
        assert!(rendered.contains("待认领"));
        assert!(rendered.contains("未知浏览器"));
    }
}
