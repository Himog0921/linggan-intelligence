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

    let mut rows = String::new();
    for (index, target) in targets.iter().enumerate() {
        rows.push_str(&target_row(
            target,
            index,
            avatars.get(&target.target_ref),
            completeness.get(&target.identity_key),
        ));
    }

    // 表头六列与内容工作台「监控来源」一致。它在真实环境用了数月，列的取舍有依据：
    // 「档案健康度」与「状态」分开，因为「有没有资料」和「在不在监控」是两件独立的事。
    let list = format!(
        r#"<section class="c-tg-workspace">
              {failure}
              <div class="c-tg-list-head">
                <span>{count} 个观察目标</span>
                <span class="c-tg-list-hint">点击一行 → 打开宽幅研究抽屉</span>
              </div>
              <form class="c-tg-form" method="post" action="/collection/targets/batch">
                <div class="c-tg-list">{rows}</div>
                <div class="c-tg-batch">
                  <span class="c-tg-batch-label">对勾选的目标：</span>
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
    replace_target_state(base, &list)
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

/// 一行观察目标。
///
/// 五列布局照 `linggan-collection-workspace-final-v4` 稿子：选择轨 / 编号 + 身份 /
/// 内容量 / 信号 / 时间 / 基线摘要。
///
/// **稿子上有而系统里没有的读数，一律写「未采集」或「—」，不填假数**（Mog 于
/// 2026-08-28 选定方案 A）。这样等评论、转录、爆款判定接通时，结构不用再改一次；
/// 而在那之前，页面不会声称系统做得到它做不到的事。
fn target_row(
    target: &ObservationTarget,
    index: usize,
    avatar: Option<&ObservationTargetAvatar>,
    archive: Option<&ArchiveCompleteness>,
) -> String {
    let is_creator = target.target_kind == "creator";
    let name = target
        .display_name
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&target.identity_key);

    format!(
        r#"<article class="c-tg-item">
                <div class="c-tg-pick"><input type="checkbox" name="target_ref" value="{target_ref}" aria-label="选择 {name}" /></div>
                <div class="c-tg-index">{index:03}</div>
                <div class="c-tg-object">
                  {avatar}
                  <div class="c-tg-object-text">
                    <a class="c-tg-title" href="/collection/targets?drawer={target_ref}">{name}</a>
                    <div class="c-tg-meta">{kind} / {platform} · {handle}</div>
                    <div class="c-tg-states">{states}</div>
                  </div>
                </div>
                <div class="c-tg-metrics">{metrics}</div>
                <div class="c-tg-signals">{signals}</div>
                <div class="c-tg-times">{times}</div>
                <div class="c-tg-baseline">{baseline}</div>
                <div class="c-tg-actions">{actions}</div>
              </article>"#,
        target_ref = target.target_ref,
        index = index + 1,
        avatar = avatar_markup(avatar),
        name = escape(name),
        kind = escape(if is_creator { "创作者" } else { "关键词" }),
        platform = escape(&target.platform.to_uppercase()),
        handle = escape(&identity_display(target)),
        states = state_chips(target),
        metrics = metrics_cell(target, archive, is_creator),
        signals = signals_cell(),
        times = times_cell(target),
        baseline = baseline_cell(archive, is_creator),
        actions = row_actions(target, is_creator, archive),
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

/// 状态徽记。稿子是 `● BASELINE READY` / `PATROLLING` 这类，此处转中文。
fn state_chips(target: &ObservationTarget) -> String {
    let (archive_tone, archive_label) = match target.lifecycle_state.as_str() {
        "pending_decision" => ("neutral", "尚未建档"),
        "archiving" => ("warn", "▲ 建档中"),
        "monitoring" => ("ok", "● 基线就绪"),
        _ => ("neutral", "状态未知"),
    };
    let (patrol_tone, patrol_label) = if target.monitoring_enabled {
        ("info", "巡检中")
    } else {
        ("neutral", "未开启巡检")
    };
    format!(
        r#"<span class="c-tg-truth c-tg-{archive_tone}">{archive_label}</span>
           <span class="c-tg-truth c-tg-{patrol_tone}">{patrol_label}</span>
           <span class="c-tg-truth c-tg-neutral">{group}</span>"#,
        group = escape(target.group_name.as_deref().unwrap_or("未分组")),
    )
}

/// 内容量。稿子是 `184 CONTENT / 6.2K COMMENTS`。
///
/// **评论数系统里没有**——评论从来没有被采过。写「未采」而不是 0：0 会被读成
/// 「这个博主没有评论」。
fn metrics_cell(
    target: &ObservationTarget,
    archive: Option<&ArchiveCompleteness>,
    is_creator: bool,
) -> String {
    if !is_creator {
        return readout_pairs(&[("—", "作品"), ("—", "评论")]);
    }
    let works = archive
        .map(|value| value.works_listed)
        .filter(|count| *count > 0)
        .map(|count| count.to_string())
        .unwrap_or_else(|| "未采".to_owned());
    let _ = target;
    readout_pairs(&[(&works, "作品"), ("未采", "评论")])
}

/// 信号。稿子是 `01 NEW / 01 BURST`。
///
/// **两项系统里都没有**：没有新增检测，也没有爆款判定。整格写「未接通」比写两个 0 诚实
/// ——0 会被读成「查过了，没有新增也没有爆款」。
fn signals_cell() -> String {
    readout_pairs(&[("—", "新增"), ("—", "爆款")])
}

/// 时间。稿子是 `32m LAST / 28m NEXT`。
///
/// 上次派出是真实记录；下次时间由「上次 + 巡检间隔」算得出来，因此可以给。巡检没开时
/// 下次写「—」，因为确实没有下一次。
fn times_cell(target: &ObservationTarget) -> String {
    let last = target
        .last_patrol_dispatched_at
        .as_deref()
        .unwrap_or("未派过");
    let next = if target.monitoring_enabled {
        target.next_patrol_at.as_deref().unwrap_or("待定")
    } else {
        "—"
    };
    readout_pairs(&[(last, "上次"), (next, "下次")])
}

/// 一格两行的读数（稿子的 `<b>值</b><span>标签</span>` 结构）。
fn readout_pairs(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(value, label)| {
            format!(
                "<b>{value}</b><span>{label}</span>",
                value = escape(value),
                label = escape(label),
            )
        })
        .collect()
}

/// 基线摘要：标题 + 健康条 + 一行读数。
fn baseline_cell(archive: Option<&ArchiveCompleteness>, is_creator: bool) -> String {
    if !is_creator {
        return r#"<div class="c-tg-baseline-title">搜索基线</div>
                  <small>关键词来源不生成博主档案</small>"#
            .to_owned();
    }
    format!(
        r#"<div class="c-tg-baseline-title">档案 / 基线</div>{health}"#,
        health = archive_health(archive, is_creator),
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

/// 行尾操作：深度建档 + 巡检开关。
///
/// 深度建档已建过就不再显示按钮——重复全量建档只会把当天额度吃光，增量是巡检在做的事。
/// 两者都**不绕过授权链**：走的是与定时巡检、与 API 完全相同的一条路。
fn row_actions(
    target: &ObservationTarget,
    is_creator: bool,
    archive: Option<&ArchiveCompleteness>,
) -> String {
    if !is_creator {
        return r#"<span class="c-tg-muted">—</span>"#.to_owned();
    }
    let archived = archive.is_some_and(|value| value.works_listed > 0);
    let archive_button = if archived {
        String::new()
    } else {
        format!(
            r#"<button class="c-btn-primary c-tg-btn" type="submit"
                  formaction="/collection/targets/archive" name="row_target_ref" value="{target_ref}">深度建档</button>"#,
            target_ref = target.target_ref,
        )
    };
    format!(
        r#"{archive_button}<button class="c-btn-quiet c-tg-btn" type="submit"
                  formaction="/collection/targets/monitoring" name="row_target_ref" value="{target_ref}">{action}</button>"#,
        target_ref = target.target_ref,
        action = if target.monitoring_enabled {
            "暂停巡检"
        } else {
            "开启巡检"
        },
    )
}

/// 小红书号优先/// 小红书号优先/// 小红书号优先/// 小红书号优先，采不到才退回平台 ID——小红书号是人能对上的那个。
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
            let html = render_stored_targets(&base, &[], &HashMap::new(), &HashMap::new(), None);
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
    fn a_successful_list_replaces_an_unreadable_count_empty_state() {
        let base =
            "before<section class=\"c-empty c-empty-engineering\">count unreadable</section>after";
        let html = render_stored_targets(
            base,
            &[target("creator", Some("真实目标"))],
            &HashMap::new(),
            &HashMap::new(),
            None,
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

        let html = render_stored_targets(&base, &[creator], &HashMap::new(), &HashMap::new(), None);

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
        let html = render_stored_targets(&base, &[creator], &avatars, &HashMap::new(), None);
        assert!(html.contains("/api/local/media/11111111-1111-4111-8111-111111111111/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
        assert!(html.contains("<img class=\"c-tg-avatar\""));
        assert!(!html.contains("http://"));
        assert!(!html.contains("https://"));
    }
}
