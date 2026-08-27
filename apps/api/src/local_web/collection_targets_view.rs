//! COLLECTION-001 · injecting stored observation targets into the Targets surface.
//!
//! The page is rendered synchronously as an honest empty state; this module replaces that
//! empty state once the read side actually has targets. It lives apart from `collection.rs`
//! because that file already exceeds the module size limit — growing it further would make a
//! known problem worse.
//!
//! What this view may claim is narrow: a row here proves the target was **stored**. It says
//! nothing about archiving, authorisation or any capture ever running (INV-36).

use linggan_evidence::ObservationTarget;
use serde_json::Value;

/// The marker `collection.rs` leaves in the Targets page so the read side can find the empty
/// state without re-parsing the whole document.
const EMPTY_STATE_OPEN: &str = "<section class=\"c-empty c-empty-you\">";
const EMPTY_STATE_CLOSE: &str = "</section>";

/// Replace the empty state with the stored targets. An empty list leaves the page untouched:
/// "no targets yet" is already answered honestly by the empty state, and a second empty
/// rendering of the same fact would only add noise.
pub fn render_stored_targets(
    base: &str,
    targets: &[ObservationTarget],
    filter: Option<&str>,
) -> String {
    if targets.is_empty() {
        return base.to_owned();
    }
    let Some(open) = base.find(EMPTY_STATE_OPEN) else {
        return base.to_owned();
    };
    let Some(close_offset) = base[open..].find(EMPTY_STATE_CLOSE) else {
        return base.to_owned();
    };
    let close = open + close_offset + EMPTY_STATE_CLOSE.len();

    let mut rows = String::new();
    for (index, target) in targets.iter().enumerate() {
        rows.push_str(&target_row(target, index));
    }

    // 表头六列与内容工作台「监控来源」一致。它在真实环境用了数月，列的取舍有依据：
    // 「档案健康度」与「状态」分开，因为「有没有资料」和「在不在监控」是两件独立的事。
    let list = format!(
        r#"<section class="c-sources">
              <div class="c-sources-bar">
                <div class="c-sources-count"><b>{count}</b><span>个来源</span></div>
                <div class="c-sources-filters">{filters}</div>
              </div>
              <p class="c-sources-note">这里是「我在长期看谁」。加入观察只写本机记录，不访问任何平台；真正开始采集要另走一遍申请 → 授权 → 准入 → 工单 → 租约。</p>
              <div class="c-src-head">
                <div>编号</div><div>博主信息</div><div>平台 / 分组</div>
                <div>状态</div><div>档案健康度</div><div>更新时间</div><div>操作</div>
              </div>
              <div class="c-src-rows">{rows}</div>
            </section>"#,
        count = targets.len(),
        filters = filter_chips(filter),
    );
    format!(
        "{before}{list}{after}",
        before = &base[..open],
        after = &base[close..],
    )
}

/// 筛选。与内容工作台同一组维度：全部 / 平台上的类型 / 生命周期。
fn filter_chips(active: Option<&str>) -> String {
    const FILTERS: &[(&str, &str)] = &[
        ("", "全部来源"),
        ("creator", "创作者"),
        ("keyword", "关键词"),
        ("archiving", "建档中"),
        ("monitoring", "已建档"),
    ];
    FILTERS
        .iter()
        .map(|(value, label)| {
            let current = active.unwrap_or("");
            let class = if current == *value {
                " c-src-chip-on"
            } else {
                ""
            };
            let href = if value.is_empty() {
                "/collection/targets".to_owned()
            } else {
                format!("/collection/targets?filter={value}")
            };
            format!(r#"<a class="c-src-chip{class}" href="{href}">{label}</a>"#)
        })
        .collect()
}

/// 一行来源。
///
/// 六列与内容工作台「监控来源」一致：编号 / 博主信息 / 平台·分组 / 状态 / 档案健康度 /
/// 更新时间 / 操作。它在真实环境用了数月，列的取舍是有依据的——博主信息里放的是「人能
/// 认出这是谁」所需的最少四样（名字、ID、简介、粉丝与赞藏），而不是把所有采到的字段
/// 都摊开。
fn target_row(target: &ObservationTarget, index: usize) -> String {
    let facts = target.identity_facts.as_ref();
    let is_creator = target.target_kind == "creator";
    let name = target
        .display_name
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&target.identity_key);

    format!(
        r#"<div class="c-src-row">
                <div class="c-src-index">{index:02}</div>
                <div class="c-src-identity">{avatar}
                  <div class="c-src-identity-text">
                    <b>{name}</b>
                    <span class="c-src-id">ID {identity}</span>
                    {bio}
                    {counts}
                  </div>
                </div>
                <div class="c-src-platform"><span class="c-src-tag">{platform}</span>{kind}</div>
                <div class="c-src-status">{status}</div>
                <div class="c-src-health">{health}</div>
                <div class="c-src-updated">{stored}</div>
                <div class="c-src-actions">{actions}</div>
              </div>"#,
        index = index + 1,
        avatar = avatar_markup(facts),
        name = escape(name),
        identity = escape(&identity_display(target)),
        bio = bio_markup(facts),
        counts = count_markup(facts, is_creator),
        platform = escape(&target.platform.to_uppercase()),
        kind = escape(if is_creator { "创作者" } else { "关键词" }),
        status = status_lines(target),
        health = archive_health(target, is_creator),
        stored = escape(&target.first_stored_at),
        actions = row_actions(target, is_creator),
    )
}

/// 状态三行，与内容工作台同构：档案 / 监控 / 分组。
///
/// 分成三行而不是压成一个词，是因为它们会各自独立变化——一个博主可以「已建档 + 监控已
/// 暂停」，压成一个状态就分不清是没建档还是被人停了。
fn status_lines(target: &ObservationTarget) -> String {
    let (archive_tone, archive_label) = match target.lifecycle_state.as_str() {
        "pending_decision" => ("neutral", "尚未建档"),
        "archiving" => ("warning", "建档中"),
        "monitoring" => ("ready", "已建档"),
        _ => ("neutral", "状态未知"),
    };
    // 巡检与建档是两个独立事实：一个博主可以「已建档 + 巡检已暂停」。压成一个状态就
    // 分不清「没在跑」是因为被停了还是因为还没建档。
    let (patrol_tone, patrol_label) = if target.monitoring_enabled {
        ("ready", "巡检中")
    } else {
        ("neutral", "未开启巡检")
    };
    format!(
        r#"<span class="c-src-line c-src-{archive_tone}">{archive_label}</span>
           <span class="c-src-line c-src-{patrol_tone}">{patrol_label}</span>
           <span class="c-src-line c-src-dash">未分组</span>"#
    )
}

/// 档案健康度。
///
/// **不采到就写「未采集」，不给一个看起来像已知的读数。**内容工作台在这一列踩过的坑是
/// 把「读不到」显示成正常值，人据此以为档案是好的。
fn archive_health(target: &ObservationTarget, is_creator: bool) -> String {
    if !is_creator {
        return r#"<span class="c-src-muted">关键词来源不生成博主档案</span>"#.to_owned();
    }
    match target.identity_facts.as_ref() {
        Some(_) => r#"<span class="c-src-line c-src-ready">公开资料已采</span>"#.to_owned(),
        None => r#"<span class="c-src-line c-src-neutral">公开资料未采集</span>"#.to_owned(),
    }
}

/// 行内操作。**只放真实存在的动作**：深度建档与巡检开关都已接通，因此是真按钮。
fn row_actions(target: &ObservationTarget, is_creator: bool) -> String {
    if !is_creator {
        return r#"<span class="c-src-muted">—</span>"#.to_owned();
    }
    format!(
        r#"<form method="post" action="/collection/targets/monitoring">
                  <input type="hidden" name="target_ref" value="{target_ref}" />
                  <button class="c-btn-quiet" type="submit">{action}</button>
                </form>"#,
        target_ref = target.target_ref,
        action = if target.monitoring_enabled {
            "暂停巡检"
        } else {
            "开启巡检"
        },
    )
}

/// 小红书号优先，采不到才退回平台 ID——小红书号是人能对上的那个。
fn identity_display(target: &ObservationTarget) -> String {
    fact_text(target.identity_facts.as_ref(), "redId")
        .unwrap_or_else(|| target.identity_key.clone())
}

/// 头像。没采到就不占位——一个灰方块会让人以为「这个博主没有头像」，而事实是还没采过。
fn avatar_markup(facts: Option<&Value>) -> String {
    let Some(url) = fact_text(facts, "avatar") else {
        return String::new();
    };
    format!(
        r#"<img class="c-src-avatar" src="{url}" alt="" loading="lazy" referrerpolicy="no-referrer" />"#,
        url = escape(&url),
    )
}

fn bio_markup(facts: Option<&Value>) -> String {
    let Some(description) = fact_text(facts, "description") else {
        return String::new();
    };
    format!(
        r#"<span class="c-src-bio">{description}</span>"#,
        description = escape(&description),
    )
}

/// 粉丝与赞藏。
///
/// **缺的字段整个不出现，不显示 0**：能力登记表记着 `userPageData` 可能整个拿不到，
/// 那时粉丝数是真的「不知道」。一个写着「粉丝 0」的档案会让人直接判定这个博主不值得看。
fn count_markup(facts: Option<&Value>, is_creator: bool) -> String {
    if !is_creator {
        return String::new();
    }
    let parts: Vec<String> = [("fans", "粉丝"), ("interactions", "赞藏")]
        .iter()
        .filter_map(|(key, label)| {
            facts
                .and_then(|value| value.get(*key))
                .and_then(Value::as_i64)
                .map(|count| format!("{label} {count}"))
        })
        .collect();
    if parts.is_empty() {
        return r#"<span class="c-src-counts c-src-muted">粉丝与赞藏未采集</span>"#.to_owned();
    }
    format!(
        r#"<span class="c-src-counts">{}</span>"#,
        escape(&parts.join(" · "))
    )
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
        }
    }

    #[test]
    fn an_empty_list_leaves_the_honest_empty_state_alone() {
        let base = format!("before{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}after");
        assert_eq!(render_stored_targets(&base, &[], None), base);
    }

    #[test]
    fn stored_targets_never_claim_more_than_being_stored() {
        let base = format!("before{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}after");
        let html = render_stored_targets(&base, &[target("creator", Some("孩悦"))], None);

        assert!(html.contains("孩悦"));
        // 词表换成了内容工作台那套（档案/巡检/分组三行），但断言的意图不变：
        // **列表不得声称比「已保存」更多**。
        assert!(html.contains("尚未建档"));
        assert!(html.contains("未开启巡检"));
        // 「加进来」与「开始采集」必须一直分得清。
        assert!(html.contains("不访问任何平台"));
        // 一个待决目标不得看起来像已经建过档或正在跑。
        //
        // 断言盯住**状态行的标记形态**而不是任意出现：「已建档」也是一个合法的筛选页签
        // 标签，按裸字符串断言会把筛选项误当成对这一行的声称。
        assert!(!html.contains(r#"c-src-ready">已建档"#));
        assert!(!html.contains(r#"c-src-ready">巡检中"#));
    }

    #[test]
    fn a_target_without_a_name_shows_its_identity_rather_than_an_invented_one() {
        let base = format!("{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}");
        let html = render_stored_targets(&base, &[target("keyword", None)], None);
        assert!(html.contains("5ebe6d21"));
        assert!(!html.contains("未命名"));
    }

    #[test]
    fn target_text_is_escaped() {
        let base = format!("{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}");
        let html = render_stored_targets(
            &base,
            &[target("creator", Some("<script>x</script>"))],
            None,
        );
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>x"));
    }
}
