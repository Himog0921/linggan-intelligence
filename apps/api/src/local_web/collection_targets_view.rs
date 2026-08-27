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
pub fn render_stored_targets(base: &str, targets: &[ObservationTarget]) -> String {
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
    for target in targets {
        rows.push_str(&target_row(target));
    }

    let list = format!(
        r#"<section class="c-targets">
              <div class="c-targets-head">
                <div class="c-targets-count"><b>{count}</b><span>待决观察目标</span></div>
                <p class="c-targets-note">这些目标已可靠保存，仅此而已。它们尚未建档、未获采集授权、也没有任何采集发生过——「已保存」不等于「已开始观察」。</p>
              </div>
              <div class="c-targets-rows">{rows}</div>
            </section>"#,
        count = targets.len(),
    );
    format!(
        "{before}{list}{after}",
        before = &base[..open],
        after = &base[close..],
    )
}

fn target_row(target: &ObservationTarget) -> String {
    let kind_label = match target.target_kind.as_str() {
        "creator" => "创作者",
        "keyword" => "关键词",
        other => other,
    };
    let source_label = match target.source.as_str() {
        "plugin_push" => "插件推送",
        "manual" => "手动添加",
        other => other,
    };
    // 没有真名时用平台标识，而不是编一个占位名。
    let name = target
        .display_name
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&target.identity_key);
    let facts = target.identity_facts.as_ref();

    format!(
        r#"<div class="c-target-row">
                <div class="c-target-kind">{kind}</div>
                {avatar}
                <div class="c-target-name"><b>{name}</b><span>{identity}</span>{bio}</div>
                <div class="c-target-meta">{counts}<span>{source}</span><span>{stored}</span></div>
                <div class="c-target-state">待决</div>
              </div>"#,
        kind = escape(kind_label),
        avatar = avatar_markup(facts),
        name = escape(name),
        identity = escape(&handle_or_identity(target)),
        bio = bio_markup(facts),
        counts = count_markup(facts),
        source = escape(source_label),
        stored = escape(&target.first_stored_at),
    )
}

/// 头像。没采到就不占位——一个灰方块会让人以为「这个博主没有头像」，而事实是还没采过。
fn avatar_markup(facts: Option<&Value>) -> String {
    let Some(url) = fact_text(facts, "avatar") else {
        return String::new();
    };
    format!(
        r#"<img class="c-target-avatar" src="{url}" alt="" loading="lazy" referrerpolicy="no-referrer" />"#,
        url = escape(&url),
    )
}

/// 小红书号优先，没采到就退回平台 ID。小红书号是人能对上的那个，平台 ID 不是。
fn handle_or_identity(target: &ObservationTarget) -> String {
    fact_text(target.identity_facts.as_ref(), "redId")
        .map(|red_id| format!("小红书号 {red_id}"))
        .unwrap_or_else(|| target.identity_key.clone())
}

fn bio_markup(facts: Option<&Value>) -> String {
    let Some(description) = fact_text(facts, "description") else {
        return String::new();
    };
    format!(
        r#"<span class="c-target-bio">{description}</span>"#,
        description = escape(&description),
    )
}

/// 粉丝、关注、赞藏。
///
/// **只显示采到的**：缺的字段整个不出现，而不是显示「粉丝 0」。能力登记表明确记着
/// `userPageData` 可能整个拿不到，那时粉丝数是真的「不知道」——把未知显示成 0，会让人
/// 据此判断这个博主不值得看。
fn count_markup(facts: Option<&Value>) -> String {
    [
        ("fans", "粉丝"),
        ("follows", "关注"),
        ("interactions", "赞藏"),
    ]
    .iter()
    .filter_map(|(key, label)| {
        facts
            .and_then(|value| value.get(*key))
            .and_then(Value::as_i64)
            .map(|count| format!("<span>{label} {count}</span>"))
    })
    .collect()
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
        }
    }

    #[test]
    fn an_empty_list_leaves_the_honest_empty_state_alone() {
        let base = format!("before{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}after");
        assert_eq!(render_stored_targets(&base, &[]), base);
    }

    #[test]
    fn stored_targets_never_claim_more_than_being_stored() {
        let base = format!("before{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}after");
        let html = render_stored_targets(&base, &[target("creator", Some("孩悦"))]);

        assert!(html.contains("孩悦"));
        assert!(html.contains("待决"));
        // The page must keep saying that storing is not observing.
        assert!(html.contains("未获采集授权"));
        // And it must not have grown a claim about archiving or running.
        assert!(!html.contains("已建档"));
        assert!(!html.contains("监控中"));
    }

    #[test]
    fn a_target_without_a_name_shows_its_identity_rather_than_an_invented_one() {
        let base = format!("{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}");
        let html = render_stored_targets(&base, &[target("keyword", None)]);
        assert!(html.contains("5ebe6d21"));
        assert!(!html.contains("未命名"));
    }

    #[test]
    fn target_text_is_escaped() {
        let base = format!("{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}");
        let html = render_stored_targets(&base, &[target("creator", Some("<script>x</script>"))]);
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>x"));
    }
}
