//! COLLECTION-001 · injecting stored observation targets into the Targets surface.
//!
//! The page is rendered synchronously as an honest empty state; this module replaces that
//! empty state once the read side actually has targets. It lives apart from `collection.rs`
//! because that file already exceeds the module size limit — growing it further would make a
//! known problem worse.
//!
//! What this view may claim is narrow: a row here proves the target was **stored**. It says
//! nothing about archiving, authorisation or any capture ever running (INV-36).

use linggan_evidence::{ArchiveCompleteness, ObservationTarget};
use serde_json::Value;
use std::collections::HashMap;

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
    completeness: &HashMap<String, ArchiveCompleteness>,
    error: Option<&str>,
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
        rows.push_str(&target_row(
            target,
            index,
            completeness.get(&target.identity_key),
        ));
    }

    // 表头六列与内容工作台「监控来源」一致。它在真实环境用了数月，列的取舍有依据：
    // 「档案健康度」与「状态」分开，因为「有没有资料」和「在不在监控」是两件独立的事。
    let list = format!(
        r#"<section class="c-sources">
              {failure}
              <form class="c-src-form" method="post" action="/collection/targets/batch">
                <div class="c-src-head">
                  <div>选择</div><div>编号</div><div>博主信息</div><div>平台 / 分组</div>
                  <div>状态</div><div>档案健康度</div><div>深度建档</div><div>更新时间</div>
                  <div class="c-src-head-count">{count} 个来源</div>
                </div>
                <div class="c-src-rows">{rows}</div>
                <div class="c-src-batch">
                  <span class="c-src-batch-label">对勾选的来源：</span>
                  <button class="c-btn-quiet" type="submit" name="action" value="monitor_on">开启巡检</button>
                  <button class="c-btn-quiet" type="submit" name="action" value="monitor_off">暂停巡检</button>
                  <input name="group_name" maxlength="40" placeholder="分组名（留空取消分组）" />
                  <button class="c-btn-quiet" type="submit" name="action" value="set_group">设置分组</button>
                </div>
              </form>
            </section>"#,
        count = targets.len(),
        failure = failure_markup(error),
    );
    format!(
        "{before}{list}{after}",
        before = &base[..open],
        after = &base[close..],
    )
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
            "现在不能发起深度建档。深度建档是一次性的：目标已经在建档中或已建过档，增量由巡检负责。"
        }
        "archive_refuse" => "深度建档被拒绝：没有覆盖「创作者 · 深度建档」的有效采集授权。",
        "archive_defer" => {
            "深度建档暂缓：资源不够（没有在岗工位、能力不匹配、当天额度已满，或风险暂停生效中）。"
        }
        "archive_merge" => {
            "深度建档暂缓：已经有一份在途的工作覆盖同一目标，等它跑完而不是再开一个。"
        }
        "archive_lease_failed" => "工单已建立但没能发出租约。工位可能刚刚掉线。",
        "batch_nothing_selected" => "没有勾选任何来源。先在左侧勾上要操作的行，再点批量动作。",
        "batch_unknown_action" => "这个批量动作系统不认识。",
        "batch_failed" => "批量操作没有完成，没有任何来源被改动。",
        "monitoring_toggle_failed" => "巡检开关没有切换成功。",
        "read_model_not_connected" => "本机读投影未接通，这次没有写入任何东西。",
        _ => "上一次动作没有完成。",
    };
    format!(
        r#"<p class="c-src-failure"><b>没有完成</b>{explanation}</p>"#,
        explanation = escape(explanation),
    )
}

/// 一行来源。
///
/// 六列与内容工作台「监控来源」一致：编号 / 博主信息 / 平台·分组 / 状态 / 档案健康度 /
/// 更新时间 / 操作。它在真实环境用了数月，列的取舍是有依据的——博主信息里放的是「人能
/// 认出这是谁」所需的最少四样（名字、ID、简介、粉丝与赞藏），而不是把所有采到的字段
/// 都摊开。
fn target_row(
    target: &ObservationTarget,
    index: usize,
    archive: Option<&ArchiveCompleteness>,
) -> String {
    let facts = target.identity_facts.as_ref();
    let is_creator = target.target_kind == "creator";
    let name = target
        .display_name
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&target.identity_key);

    format!(
        r#"<div class="c-src-row">
                <div class="c-src-pick"><input type="checkbox" name="target_ref" value="{target_ref}" aria-label="选择 {name}" /></div>
                <div class="c-src-index">{index:02}</div>
                <div class="c-src-identity">{avatar}
                  <div class="c-src-identity-text">
                    <b>{name}</b>
                    <span class="c-src-id">ID {identity}</span>
                    {bio}
                    {counts}
                  </div>
                </div>
                <div class="c-src-platform"><span class="c-src-tag">{platform}</span>{kind}<span class="c-src-group">{group}</span></div>
                <div class="c-src-status">{status}</div>
                <div class="c-src-health">{health}</div>
                <div class="c-src-archive">{archive_action}</div>
                <div class="c-src-updated">{stored}</div>
                <div class="c-src-actions">{actions}</div>
              </div>"#,
        target_ref = target.target_ref,
        index = index + 1,
        avatar = avatar_markup(facts),
        name = escape(name),
        identity = escape(&identity_display(target)),
        bio = bio_markup(facts),
        counts = count_markup(facts, is_creator),
        platform = escape(&target.platform.to_uppercase()),
        kind = escape(if is_creator { "创作者" } else { "关键词" }),
        group = escape(target.group_name.as_deref().unwrap_or("未分组")),
        status = status_lines(target),
        health = archive_health(archive, is_creator),
        stored = escape(&target.first_stored_at),
        archive_action = archive_action(target, is_creator, archive),
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

/// 档案健康度：进度条 + 逐项读数 + 缺口。
///
/// 算法照内容工作台 `monitor-archive-health.tsx`：**分母为 0 的维度不参与百分比**
/// （`applicableMetrics.filter(total > 0)`），完全没有作品清单时**干脆不画进度条**——
/// 那边对未建档的来源显示的是「ARCHIVE NOT CREATED」，不是一个 0%。
///
/// 这一条很要紧：一个还没采过作品清单的博主，画一根 0% 的条会让人以为「采过了但什么
/// 都没有」，而事实是根本没采。**没有分母就没有百分比。**
fn archive_health(archive: Option<&ArchiveCompleteness>, is_creator: bool) -> String {
    if !is_creator {
        return r#"<span class="c-src-muted">关键词来源不生成博主档案</span>"#.to_owned();
    }
    let Some(archive) = archive.filter(|value| !value.is_untouched()) else {
        return r#"<div class="c-hp-none"><span class="c-hp-key">ARCHIVE NOT CREATED</span>
                  <span class="c-hp-hint">当前来源尚未采集。</span></div>"#
            .to_owned();
    };
    // 作品清单是所有逐篇指标的分母。没有它就只报已知的事实，不给百分比。
    if archive.works_listed == 0 {
        return format!(
            r#"<div class="c-hp-none"><span class="c-hp-key">NO WORK SET</span>
               <span class="c-hp-hint">已取得作者档案 {profile}，但还没有作品清单——逐篇进度没有分母。</span></div>"#,
            profile = archive.author_profile_captures,
        );
    }

    let total = archive.works_listed;
    let done = archive.details_captured.min(total);
    let percent = (done * 100 / total).clamp(0, 100);
    let missing = total - done;

    format!(
        r#"<div class="c-hp">
              <div class="c-hp-top">
                <span class="c-hp-key">ARCHIVE HEALTH</span>
                <span class="c-hp-state">{state}</span>
                <span class="c-hp-percent">{percent}%</span>
              </div>
              <div class="c-hp-bar" role="img" aria-label="档案完成度 {percent}%">{ticks}</div>
              <div class="c-hp-metrics"><span>详情 {done}/{total}</span></div>
              <div class="c-hp-foot"><span>作品 {total}</span>{quarantined}{gap}</div>
            </div>"#,
        state = if percent >= 100 {
            "COMPLETE"
        } else {
            "NEEDS COMPLETION"
        },
        ticks = health_ticks(percent),
        quarantined = if archive.quarantined > 0 {
            format!("<span>已隔离 {}</span>", archive.quarantined)
        } else {
            String::new()
        },
        gap = if missing > 0 {
            format!(r#"<span class="c-hp-gap">缺详情 {missing}</span>"#)
        } else {
            String::new()
        },
    )
}

/// 分段进度条。分段而不是一根实心条，是为了让「还差多少格」可数——一根渐变条只能看出
/// 大概，数格子能看出确切进度。
fn health_ticks(percent: i64) -> String {
    const TICKS: i64 = 24;
    let filled = (percent * TICKS / 100).clamp(0, TICKS);
    (0..TICKS)
        .map(|index| {
            if index < filled {
                r#"<i class="c-hp-tick c-hp-tick-on"></i>"#
            } else {
                r#"<i class="c-hp-tick"></i>"#
            }
        })
        .collect()
}

/// 深度建档列。
///
/// 从「操作」里拆出来单独成列（Mog 于 2026-08-28 要求）：它与巡检开关不是同一类动作
/// ——巡检是长期节奏的开关，深度建档是一次性的、重的、会吃掉当天大半额度的动作。
/// 混在一列里，一个日常操作和一个重动作会长得一样。
///
/// **不绕过授权链**：它走的是与定时巡检、与 API 完全相同的那条路。
fn archive_action(
    target: &ObservationTarget,
    is_creator: bool,
    archive: Option<&ArchiveCompleteness>,
) -> String {
    if !is_creator {
        return r#"<span class="c-src-muted">—</span>"#.to_owned();
    }
    // 已经建过档就不再显示按钮：重复全量建档只会把当天额度吃光，增量是巡检在做的事。
    if archive.is_some_and(|value| value.works_listed > 0) {
        return r#"<span class="c-src-line c-src-ready">已建档</span>"#.to_owned();
    }
    format!(
        r#"<button class="c-btn-primary c-src-btn" type="submit"
                  formaction="/collection/targets/archive" name="row_target_ref" value="{target_ref}">深度建档</button>"#,
        target_ref = target.target_ref,
    )
}

/// 行内操作：只剩巡检开关。
fn row_actions(target: &ObservationTarget, is_creator: bool) -> String {
    if !is_creator {
        return r#"<span class="c-src-muted">—</span>"#.to_owned();
    }
    format!(
        r#"<button class="c-btn-quiet" type="submit"
                  formaction="/collection/targets/monitoring" name="row_target_ref" value="{target_ref}">{action}</button>"#,
        target_ref = target.target_ref,
        action = if target.monitoring_enabled {
            "暂停巡检"
        } else {
            "开启巡检"
        },
    )
}

/// 小红书号优先/// 小红书号优先/// 小红书号优先，采不到才退回平台 ID——小红书号是人能对上的那个。
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
            group_name: None,
        }
    }

    #[test]
    fn an_empty_list_leaves_the_honest_empty_state_alone() {
        let base = format!("before{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}after");
        assert_eq!(
            render_stored_targets(&base, &[], &HashMap::new(), None),
            base
        );
    }

    #[test]
    fn stored_targets_never_claim_more_than_being_stored() {
        let base = format!("before{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}after");
        let html = render_stored_targets(
            &base,
            &[target("creator", Some("孩悦"))],
            &HashMap::new(),
            None,
        );

        assert!(html.contains("孩悦"));
        // 词表换成了内容工作台那套（档案/巡检/分组三行），但断言的意图不变：
        // **列表不得声称比「已保存」更多**。
        assert!(html.contains("尚未建档"));
        assert!(html.contains("未开启巡检"));
        // 「加进来」与「开始采集」必须一直分得清——那句说明已从列表上方移除（与页面
        // 自带的 tab 条重复），改由状态列的「尚未建档 / 未开启巡检」承担同一件事。
        assert!(html.contains("尚未建档"));
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
        let html = render_stored_targets(&base, &[target("keyword", None)], &HashMap::new(), None);
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
            None,
        );
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>x"));
    }
}
