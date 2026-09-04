//! COLLECTION-001 · injecting stored observation targets into the Targets surface.
//!
//! The page is rendered synchronously as an honest empty state; this module replaces that
//! empty state once the read side actually has targets. It lives apart from `collection.rs`
//! because that file already exceeds the module size limit — growing it further would make a
//! known problem worse.
//!
//! What this view may claim is narrow: a row here proves the target was **stored**. It says
//! nothing about archiving, authorisation or any capture ever running (INV-36).

use linggan_evidence::{ArchiveCompleteness, ObservationTarget, ObservationTargetAvatar};
use serde_json::Value;
use std::collections::HashMap;

/// The marker `collection.rs` leaves in the Targets page so the read side can find either a
/// confirmed-empty or unreadable empty state. A successful non-empty list is stronger than
/// a failed count query and must replace either form without changing unrelated header facts.
const EMPTY_STATE_OPEN: &str = "<section class=\"c-empty";
const EMPTY_STATE_CLOSE: &str = "</section>";

/// Replace the provisional target state with the successful list result. An empty successful
/// result is also a fact: it must replace both "list unreadable" variants without claiming
/// that the entire unfiltered target collection is empty.
pub fn render_stored_targets(
    base: &str,
    targets: &[ObservationTarget],
    avatars: &HashMap<uuid::Uuid, ObservationTargetAvatar>,
    completeness: &HashMap<String, ArchiveCompleteness>,
    error: Option<&str>,
    list_context: super::target_drawer::TargetListContext<'_>,
) -> String {
    if targets.is_empty() {
        return replace_target_state(
            base,
            r#"<section class="c-empty c-empty-known-view">
                 <div class="c-empty-rule"></div>
                 <h2>当前列表范围没有匹配的观察目标</h2>
                 <p>目标列表读取成功，当前筛选范围返回零项；这不表示其他筛选范围为空，也不表示平台没有可观察对象。</p>
                 <div class="c-empty-foot"></div>
               </section>"#,
        );
    }

    let creator_table = target_table(
        "creator",
        "创作者档案",
        targets,
        avatars,
        completeness,
        list_context,
    );
    let keyword_table = target_table(
        "keyword",
        "关键词观察",
        targets,
        avatars,
        completeness,
        list_context,
    );

    // Creator 与 keyword 的问题不同，不能再共享一套含糊表头。默认目录不显示选择框或
    // 批量操作；用户在这里先判断对象状态和下一步，而不是先进入管理模式。
    let list = format!(
        r#"<section class="c-tg-workspace">
              {failure}
              <div class="c-tg-list-head">
                <span>{count} 个观察目标</span>
                <span class="c-tg-list-hint">点击一行查看完整档案</span>
              </div>
              <div class="c-tg-directory">{creator_table}{keyword_table}</div>
            </section>"#,
        count = targets.len(),
        failure = failure_markup(error),
    );
    replace_target_state(base, &list)
}

fn target_table(
    kind: &str,
    title: &str,
    targets: &[ObservationTarget],
    avatars: &HashMap<uuid::Uuid, ObservationTargetAvatar>,
    completeness: &HashMap<String, ArchiveCompleteness>,
    list_context: super::target_drawer::TargetListContext<'_>,
) -> String {
    let matching = targets
        .iter()
        .enumerate()
        .filter(|(_, target)| target.target_kind == kind)
        .collect::<Vec<_>>();
    if matching.is_empty() {
        return String::new();
    }
    let is_creator = kind == "creator";
    let mut rows = String::new();
    for (index, target) in matching.iter().copied() {
        rows.push_str(&target_row(
            target,
            index,
            avatars.get(&target.target_ref),
            completeness.get(&target.identity_key),
            list_context,
        ));
    }
    let columns = if is_creator {
        r#"<span role="columnheader">编号</span><span role="columnheader">创作者</span><span role="columnheader">平台</span><span role="columnheader">分组</span><span role="columnheader">档案状态</span><span role="columnheader">作品目录</span><span role="columnheader">详情进度</span><span role="columnheader">巡查状态</span><span role="columnheader">最近变化</span><span role="columnheader">上次巡查</span><span role="columnheader">下次巡查</span><span role="columnheader">操作</span>"#
    } else {
        r#"<span role="columnheader">编号</span><span role="columnheader">关键词</span><span role="columnheader">平台</span><span role="columnheader">分组</span><span role="columnheader">巡查状态</span><span role="columnheader">最近命中</span><span role="columnheader">数据更新</span><span role="columnheader">上次巡查</span><span role="columnheader">下次巡查</span><span role="columnheader">操作</span>"#
    };
    let grid = if is_creator {
        "c-tg-creator-grid"
    } else {
        "c-tg-keyword-grid"
    };
    format!(
        r#"<section class="c-tg-kind" aria-labelledby="target-kind-{kind}">
              <div class="c-tg-kind-head"><h2 id="target-kind-{kind}">{title}</h2><span>{count} 项</span></div>
              <div class="c-tg-table-scroll" role="table" aria-label="{title}">
                <div class="c-tg-table-head {grid}" role="row">{columns}</div>
                <div class="c-tg-list" role="rowgroup">{rows}</div>
              </div>
            </section>"#,
        count = matching.len(),
    )
}

fn replace_target_state(base: &str, replacement: &str) -> String {
    let Some(open) = base.find(EMPTY_STATE_OPEN) else {
        return base.to_owned();
    };
    let Some(close_offset) = base[open..].find(EMPTY_STATE_CLOSE) else {
        return base.to_owned();
    };
    let close = open + close_offset + EMPTY_STATE_CLOSE.len();
    format!("{}{}{}", &base[..open], replacement, &base[close..])
}

/// 上一次动作失败时说明原因。
///
/// 失败必须看得见。跳转回来却什么都不说，会让人以为动作成功了——那比「点了没反应」
/// 更糟，因为它会让人以为系统里正在跑一件其实没跑的事。
fn failure_markup(error: Option<&str>) -> String {
    let Some(code) = error else {
        return String::new();
    };
    let explanation = match code {
        "identity_unrecognised" => {
            "认不出这是谁。创作者请粘主页链接（里面带平台 ID），关键词直接写词就行。"
        }
        "store_failed" => "没有保存成功。这个目标可能已经在观察列表里了。",
        "archive_not_requestable" => {
            "现在不能建立或继续完善档案。目标可能已有同类工作在进行，或当前状态不允许再次发起。"
        }
        "archive_refuse" => "深度建档被拒绝：没有覆盖「创作者 · 深度建档」的有效采集授权。",
        "archive_defer" => {
            "深度建档暂缓：资源不够（没有在岗工位、能力不匹配、当天额度已满，或风险暂停生效中）。"
        }
        "archive_merge" => {
            "深度建档暂缓：已经有一份在途的工作覆盖同一目标，等它跑完而不是再开一个。"
        }
        "archive_lease_failed" => "工单已建立但没能发出租约。工位可能刚刚掉线。",
        "archive_authorization_below_200" => {
            "当前采集授权不足以支持前 200 篇作品的有界建档。本次没有缩小范围后静默开始。"
        }
        "batch_nothing_selected" => "没有勾选任何来源。先在左侧勾上要操作的行，再点批量动作。",
        "batch_unknown_action" => "这个批量动作系统不认识。",
        "batch_failed" => "批量操作没有完成，没有任何来源被改动。",
        "monitoring_toggle_failed" => "巡查开关没有切换成功。",
        "read_model_not_connected" => "本机读投影未接通，这次没有写入任何东西。",
        _ => "上一次动作没有完成。",
    };
    format!(
        r#"<p class="c-src-failure"><b>没有完成</b>{explanation}</p>"#,
        explanation = escape(explanation),
    )
}

/// 一行观察目标。creator 与 keyword 只共享对象身份和巡查时间；档案、作品分布与详情
/// 只属于 creator。当前读模型没有「最近新增/数据更新」计数时直接写尚未取得，不用 0
/// 冒充已巡查后的零变化。
fn target_row(
    target: &ObservationTarget,
    index: usize,
    avatar: Option<&ObservationTargetAvatar>,
    archive: Option<&ArchiveCompleteness>,
    list_context: super::target_drawer::TargetListContext<'_>,
) -> String {
    let is_creator = target.target_kind == "creator";
    let name = target
        .display_name
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&target.identity_key);
    let opener_href = list_context.drawer_href(target.target_ref, &[], None);
    let opener_id = format!("target-{}", target.target_ref);

    let shared_start = format!(
        r#"<div class="c-tg-number" role="cell"><span class="c-tg-index">{index:03}</span></div>
            <div class="c-tg-object" role="cell">{avatar}<div class="c-tg-object-text"><span class="c-tg-title">{name}</span>{identity}</div></div>
            <div class="c-tg-cell c-tg-platform" role="cell">{platform}</div>
            <div class="c-tg-cell c-tg-group" role="cell" title="{group}">{group}</div>"#,
        index = index + 1,
        avatar = if is_creator {
            avatar_markup(avatar)
        } else {
            String::new()
        },
        name = escape(name),
        identity = if is_creator {
            format!(
                r#"<span class="c-tg-meta" title="{}">{}</span>"#,
                escape(&identity_display(target)),
                escape(&identity_display(target))
            )
        } else {
            String::new()
        },
        platform = escape(platform_label(&target.platform)),
        group = escape(target.group_name.as_deref().unwrap_or("未分组")),
    );
    let last = target
        .last_patrol_succeeded_at
        .as_deref()
        .unwrap_or("尚未巡查");
    let next = if target.monitoring_enabled {
        target.next_patrol_at.as_deref().unwrap_or("待排定")
    } else {
        "—"
    };
    let cells = if is_creator {
        format!(
            r#"<div class="c-tg-cell" role="cell">{archive_state}</div>
                <div class="c-tg-cell c-tg-number-value" role="cell">{works}</div>
                <div class="c-tg-cell c-tg-number-value" role="cell">{details}</div>
                <div class="c-tg-cell" role="cell">{patrol}</div>
                <div class="c-tg-cell c-tg-unknown" role="cell" title="当前尚未取得最近巡查的新作品或数据更新统计">尚未取得</div>
                <time class="c-tg-cell c-tg-time" role="cell">{last}</time>
                <time class="c-tg-cell c-tg-time" role="cell">{next}</time>
                <div class="c-tg-actions" role="cell">{actions}</div>"#,
            archive_state = archive_state(archive),
            works = archive_count(archive),
            details = detail_count(archive),
            patrol = patrol_state(target),
            last = escape(last),
            next = escape(next),
            actions = row_action(target, true, archive, list_context),
        )
    } else {
        format!(
            r#"<div class="c-tg-cell" role="cell">{patrol}</div>
                <div class="c-tg-cell c-tg-unknown" role="cell" title="当前尚未取得最近一次关键词巡查的命中统计">尚未取得</div>
                <div class="c-tg-cell c-tg-unknown" role="cell" title="当前尚未取得命中作品的数据更新统计">尚未取得</div>
                <time class="c-tg-cell c-tg-time" role="cell">{last}</time>
                <time class="c-tg-cell c-tg-time" role="cell">{next}</time>
                <div class="c-tg-actions" role="cell">{actions}</div>"#,
            patrol = patrol_state(target),
            last = escape(last),
            next = escape(next),
            actions = row_action(target, false, archive, list_context),
        )
    };
    let grid = if is_creator {
        "c-tg-creator-grid"
    } else {
        "c-tg-keyword-grid"
    };
    format!(
        r#"<article class="c-tg-item {grid}" role="row">
              <a id="{opener_id}" class="c-tg-row-open" data-drawer-trigger="{target_ref}" href="{opener_href}" aria-label="打开{name}的观察档案"></a>
              {shared_start}{cells}
            </article>"#,
        target_ref = target.target_ref,
        name = escape(name),
    )
}

fn avatar_markup(avatar: Option<&ObservationTargetAvatar>) -> String {
    match avatar.unwrap_or(&ObservationTargetAvatar::NotObserved) {
        ObservationTargetAvatar::Local { local_asset_path } => format!(
            r#"<img class="c-tg-avatar" src="{}" alt="博主头像" referrerpolicy="no-referrer" />"#,
            escape(local_asset_path),
        ),
        ObservationTargetAvatar::Pending => {
            r#"<span class="c-tg-avatar c-tg-avatar-state" title="头像已观察，等待本机媒体物化">头像<br/>物化中</span>"#.to_owned()
        }
        ObservationTargetAvatar::Unavailable => {
            r#"<span class="c-tg-avatar c-tg-avatar-state" title="头像本机物化未完成或已不可用">头像<br/>不可用</span>"#.to_owned()
        }
        ObservationTargetAvatar::NotObserved => {
            r#"<span class="c-tg-avatar c-tg-avatar-state" title="本次作者资料未观察到头像">头像<br/>未观察</span>"#.to_owned()
        }
    }
}

fn platform_label(platform: &str) -> &str {
    match platform {
        "xhs" => "小红书",
        other => other,
    }
}

fn archive_state(archive: Option<&ArchiveCompleteness>) -> String {
    let (tone, label) = if archive.is_some_and(|value| value.work_in_progress) {
        ("warn", "建档中")
    } else if archive.is_none_or(ArchiveCompleteness::is_untouched) {
        ("neutral", "尚未建立")
    } else if archive.is_some_and(|value| value.quarantined > 0) {
        ("warn", "档案有问题")
    } else if archive
        .is_some_and(|value| value.works_listed > 0 && value.details_captured < value.works_listed)
    {
        ("warn", "待完善")
    } else if archive.is_some_and(|value| value.works_listed > 0) {
        ("ok", "当前范围已齐")
    } else {
        ("neutral", "等待作品目录")
    };
    format!(r#"<span class="c-tg-truth c-tg-{tone}">{label}</span>"#)
}

fn patrol_state(target: &ObservationTarget) -> String {
    let (tone, label) = super::target_drawer::lifecycle_patrol_copy(target);
    format!(r#"<span class="c-tg-truth c-tg-{tone}">{label}</span>"#)
}

fn archive_count(archive: Option<&ArchiveCompleteness>) -> String {
    archive
        .filter(|value| !value.is_untouched())
        .map(|value| format!("{} 篇", value.works_listed))
        .unwrap_or_else(|| "—".to_owned())
}

fn detail_count(archive: Option<&ArchiveCompleteness>) -> String {
    archive
        .filter(|value| value.works_listed > 0)
        .map(|value| {
            let missing = (value.works_listed - value.details_captured).max(0);
            format!(
                "{} / {} · 缺 {}",
                value.details_captured, value.works_listed, missing
            )
        })
        .unwrap_or_else(|| "—".to_owned())
}

/// 每行只提供一个主要动作。详情抽屉和巡查规则只是导航；建档/继续完善仍走同一条受控
/// 请求入口，真实结果由后端 durable receipt 决定，按钮本身不声称已经执行。
fn row_action(
    target: &ObservationTarget,
    is_creator: bool,
    archive: Option<&ArchiveCompleteness>,
    list_context: super::target_drawer::TargetListContext<'_>,
) -> String {
    let opener_id = format!("monitor-rule-{}", target.target_ref);
    let rule_href = list_context.monitor_rule_href(target.target_ref, &opener_id);
    let rule_label = if target.lifecycle_state == "paused" {
        "恢复巡查"
    } else if target.monitoring_enabled {
        "管理巡查"
    } else if is_creator {
        "开启巡查"
    } else {
        "设置巡查"
    };
    let rule_entry = format!(
        r#"<a id="{opener_id}" class="c-btn-secondary c-tg-btn" data-monitor-rule-trigger="{target_ref}" href="{rule_href}">{rule_label}</a>"#,
        target_ref = target.target_ref,
    );
    if !is_creator {
        if target.monitoring_enabled && target.lifecycle_state != "paused" {
            let drawer_href = list_context.drawer_href(target.target_ref, &[], None);
            return format!(
                r#"<a class="c-btn-secondary c-tg-btn" href="{drawer_href}">查看观察</a>"#
            );
        }
        return rule_entry;
    }
    let drawer_href = list_context.drawer_href(
        target.target_ref,
        &[("dtab", "archive")],
        Some("target-archive"),
    );
    if archive.is_some_and(|value| value.work_in_progress) {
        return format!(r#"<a class="c-btn-secondary c-tg-btn" href="{drawer_href}">查看进度</a>"#);
    }
    if archive.is_some_and(|value| value.quarantined > 0) {
        return format!(
            r#"<a class="c-btn-secondary c-tg-btn" href="{drawer_href}">查看档案问题</a>"#
        );
    }
    if target.lifecycle_state == "dismissed" {
        return format!(r#"<a class="c-btn-secondary c-tg-btn" href="{drawer_href}">查看档案</a>"#);
    }
    let untouched = archive.is_none_or(ArchiveCompleteness::is_untouched);
    let needs_details = archive.is_some_and(|value| {
        value.works_listed == 0 || value.details_captured < value.works_listed
    });
    if untouched || needs_details {
        let label = if untouched {
            "建立档案"
        } else {
            "继续完善"
        };
        return format!(
            r#"<form method="post" action="/collection/targets/archive"><button class="c-btn-primary c-tg-btn" type="submit" name="row_target_ref" value="{target_ref}">{label}</button></form>"#,
            target_ref = target.target_ref,
        );
    }
    if !target.monitoring_enabled {
        return rule_entry;
    }
    format!(r#"<a class="c-btn-secondary c-tg-btn" href="{drawer_href}">查看档案</a>"#)
}

/// 小红书号优先，采不到才退回平台 ID——小红书号是人能对上的那个。
fn identity_display(target: &ObservationTarget) -> String {
    fact_text(target.identity_facts.as_ref(), "redId")
        .unwrap_or_else(|| target.identity_key.clone())
}

fn fact_text(facts: Option<&Value>, key: &str) -> Option<String> {
    facts
        .and_then(|value| value.get(key))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
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
    use super::super::target_drawer::TargetListContext;
    use super::*;
    use uuid::Uuid;

    fn target(kind: &str, name: Option<&str>) -> ObservationTarget {
        ObservationTarget {
            target_ref: Uuid::new_v4(),
            platform: "xhs".to_owned(),
            target_kind: kind.to_owned(),
            identity_key: "5ebe6d21".to_owned(),
            display_name: name.map(str::to_owned),
            identity_facts: None,
            source: "plugin_push".to_owned(),
            lifecycle_state: "pending_decision".to_owned(),
            first_stored_at: "2026-08-26T20:00:00+08".to_owned(),
            monitoring_enabled: false,
            group_name: None,
            last_patrol_dispatched_at: None,
            last_patrol_succeeded_at: None,
            next_patrol_at: None,
        }
    }

    #[test]
    fn a_successful_empty_list_replaces_each_provisional_unreadable_state() {
        for provisional in ["观察目标列表暂时读不到", "目标计数与列表当前都不可读"]
        {
            let base = format!(
                "before<section class=\"c-empty c-empty-engineering\">{provisional}</section>after"
            );
            let html = render_stored_targets(
                &base,
                &[],
                &HashMap::new(),
                &HashMap::new(),
                None,
                TargetListContext::default(),
            );
            assert!(html.contains("当前列表范围没有匹配的观察目标"));
            assert!(html.contains("目标列表读取成功"));
            assert!(!html.contains(provisional));
        }
    }

    #[test]
    fn stored_targets_never_claim_more_than_being_stored() {
        let base = format!("before{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}after");
        let html = render_stored_targets(
            &base,
            &[target("creator", Some("孩悦"))],
            &HashMap::new(),
            &HashMap::new(),
            None,
            TargetListContext::default(),
        );

        assert!(html.contains("孩悦"));
        // 存下目标不等于已经建立档案或跑过巡查。
        assert!(html.contains("尚未建立"));
        assert!(html.contains("未开启巡查"));
        assert!(html.contains("建立档案"));
        assert!(!html.contains("档案已建立"));
        assert!(!html.contains("巡查中"));
    }

    #[test]
    fn a_successful_list_replaces_an_unreadable_count_empty_state() {
        let base =
            "before<section class=\"c-empty c-empty-engineering\">count unreadable</section>after";
        let html = render_stored_targets(
            base,
            &[target("creator", Some("真实目标"))],
            &HashMap::new(),
            &HashMap::new(),
            None,
            TargetListContext::default(),
        );

        assert!(html.contains("真实目标"));
        assert!(!html.contains("count unreadable"));
        assert_eq!(html.matches("c-tg-workspace").count(), 1);
    }

    #[test]
    fn a_target_without_a_name_shows_its_identity_rather_than_an_invented_one() {
        let base = format!("{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}");
        let html = render_stored_targets(
            &base,
            &[target("keyword", None)],
            &HashMap::new(),
            &HashMap::new(),
            None,
            TargetListContext::default(),
        );
        assert!(html.contains("5ebe6d21"));
        assert!(!html.contains("未命名"));
    }

    #[test]
    fn target_text_is_escaped() {
        let base = format!("{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}");
        let html = render_stored_targets(
            &base,
            &[target("creator", Some("<script>x</script>"))],
            &HashMap::new(),
            &HashMap::new(),
            None,
            TargetListContext::default(),
        );
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>x"));
    }

    #[test]
    fn stored_target_does_not_render_a_remote_identity_avatar() {
        let base = format!("{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}");
        let mut creator = target("creator", Some("真实创作者"));
        creator.identity_facts = Some(serde_json::json!({
            "avatar": "https://sns-avatar-qc.xhscdn.com/remote-avatar.jpg",
            "redId": "creator-001"
        }));

        let html = render_stored_targets(
            &base,
            &[creator],
            &HashMap::new(),
            &HashMap::new(),
            None,
            TargetListContext::default(),
        );

        assert!(html.contains("creator-001"));
        assert!(!html.contains("remote-avatar.jpg"));
        assert!(!html.contains("<img"));
    }

    #[test]
    fn stored_target_renders_only_a_qualified_local_avatar_asset() {
        let base = format!("{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}");
        let creator = target("creator", Some("本机头像作者"));
        let mut avatars = HashMap::new();
        avatars.insert(
            creator.target_ref,
            ObservationTargetAvatar::Local {
                local_asset_path: "/api/local/media/11111111-1111-4111-8111-111111111111/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
            },
        );
        let html = render_stored_targets(
            &base,
            &[creator],
            &avatars,
            &HashMap::new(),
            None,
            TargetListContext::default(),
        );
        assert!(html.contains("/api/local/media/11111111-1111-4111-8111-111111111111/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
        assert!(html.contains("<img class=\"c-tg-avatar\""));
        assert!(!html.contains("http://"));
        assert!(!html.contains("https://"));
    }

    #[test]
    fn target_opener_keeps_the_filtered_list_context_and_has_a_focus_return_anchor() {
        let base = format!("{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}");
        let creator = target("creator", Some("筛选内作者"));
        let html = render_stored_targets(
            &base,
            std::slice::from_ref(&creator),
            &HashMap::new(),
            &HashMap::new(),
            None,
            TargetListContext {
                filter: Some("creator"),
                sort: Some("last"),
            },
        );
        assert!(html.contains(&format!("id=\"target-{}\"", creator.target_ref)));
        assert!(html.contains(&format!("data-drawer-trigger=\"{}\"", creator.target_ref)));
        assert!(html.contains(&format!(
            "href=\"/collection/targets?filter=creator&amp;sort=last&amp;drawer={}\"",
            creator.target_ref
        )));
    }

    #[test]
    fn creator_and_keyword_use_distinct_columns_and_one_primary_action_per_row() {
        let base = format!("{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}");
        let creator = target("creator", Some("作者"));
        let keyword = target("keyword", Some("关键词"));
        let html = render_stored_targets(
            &base,
            &[creator, keyword],
            &HashMap::new(),
            &HashMap::new(),
            None,
            TargetListContext::default(),
        );
        assert!(html.contains("创作者档案"));
        assert!(html.contains("关键词观察"));
        assert!(html.contains("c-tg-creator-grid"));
        assert!(html.contains("c-tg-keyword-grid"));
        assert!(html.contains(r#"作品目录</span><span role="columnheader">详情进度"#));
        assert!(html.contains(r#"最近命中</span><span role="columnheader">数据更新"#));
        assert_eq!(html.matches("c-tg-actions").count(), 2);
        assert_eq!(html.matches("c-tg-btn").count(), 2);
        assert_eq!(html.matches("data-monitor-rule-trigger").count(), 1);
        assert_eq!(html.matches(">建立档案</button>").count(), 1);
        assert_eq!(html.matches(">设置巡查</a>").count(), 1);
        assert!(!html.contains("type=\"checkbox\""));
        assert!(!html.contains("c-tg-batch"));
        assert!(!html.contains("/collection/targets/monitoring"));
        assert!(!html.contains("name=\"action\" value=\"monitor_on\""));
        assert!(!html.contains("name=\"action\" value=\"monitor_off\""));
    }

    #[test]
    fn creator_row_separates_archive_progress_and_successful_patrol_times() {
        let base = format!("{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}");
        let mut creator = target("creator", Some("档案进度作者"));
        creator.monitoring_enabled = true;
        creator.lifecycle_state = "monitoring".to_owned();
        creator.last_patrol_dispatched_at = Some("2026-09-04 09:00".to_owned());
        creator.last_patrol_succeeded_at = Some("2026-09-04 08:30".to_owned());
        creator.next_patrol_at = Some("2026-09-05 09:00".to_owned());
        let mut completeness = HashMap::new();
        completeness.insert(
            creator.identity_key.clone(),
            ArchiveCompleteness {
                work_in_progress: false,
                author_profile_captures: 1,
                works_listed: 12,
                details_captured: 5,
                quarantined: 0,
            },
        );
        let html = render_stored_targets(
            &base,
            &[creator],
            &HashMap::new(),
            &completeness,
            None,
            TargetListContext::default(),
        );

        assert!(html.contains("待完善"));
        assert!(html.contains("12 篇"));
        assert!(html.contains("5 / 12 · 缺 7"));
        assert!(html.contains("继续完善"));
        assert!(html.contains("2026-09-04 08:30"));
        assert!(html.contains("2026-09-05 09:00"));
        assert!(!html.contains("2026-09-04 09:00"));
        assert!(!html.contains("ARCHIVE HEALTH"));
        assert!(!html.contains('%'));
    }
}
