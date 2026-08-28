//! COLLECTION-001 · 观察目标的宽幅研究抽屉。
//!
//! 结构照 `linggan-collection-workspace-final-v4` 稿子：五个 tab（概览 / 基线 /
//! 巡检策略 / 证据 / 追踪），左侧列表保持可见。
//!
//! **用 URL 参数驱动，不用 JS**：`?drawer=<target_ref>&dtab=baseline`。稿子那 142 行
//! 脚本换来的是「点击不刷新」，代价是刷新即丢状态、链接分享不过去。URL 版两样都不丢，
//! 而这一页的用途正是「打开一个目标细看，然后把它发给别人」。
//!
//! 稿子上有而系统里没有的读数，一律如实写「未采集」或「尚未接通」（Mog 于 2026-08-28
//! 选定方案 A），不填假数。

use linggan_evidence::{ArchiveCompleteness, ObservationTarget};

/// 抽屉的五个 tab。顺序照稿子。
const TABS: &[(&str, &str)] = &[
    ("overview", "概览"),
    ("baseline", "基线"),
    ("patrol", "巡检策略"),
    ("evidence", "证据"),
    ("trace", "追踪"),
];

/// 渲染抽屉。`drawer` 为空时整块不渲染——没有选中目标时不该有一个空壳挂在那里。
pub fn render(
    target: Option<&ObservationTarget>,
    completeness: &std::collections::HashMap<String, ArchiveCompleteness>,
    drawer: Option<&str>,
    active_tab: Option<&str>,
) -> String {
    let Some(drawer) = drawer else {
        return String::new();
    };
    // 找不到就如实说找不到，而不是静默关掉抽屉——链接失效与「我没点开」是两回事。
    //
    // 目标由调用方**独立查询**得到，不从当前列表里找：列表是筛过的，一个被筛掉的目标
    // 会让这里说「未找到」，而它其实好好地在库里——那是在撒谎。
    let Some(target) = target else {
        return format!(
            r#"<aside class="c-dw" aria-label="观察目标工作区">
                 <div class="c-dw-head">
                   <div class="c-dw-kicker">目标工作区</div>
                   <div class="c-dw-title-row">
                     <div><div class="c-dw-title">未找到该观察目标</div>
                       <div class="c-dw-meta">#{drawer}</div></div>
                     <div class="c-dw-actions"><a class="c-btn-quiet" href="/collection/targets">关闭 ×</a></div>
                   </div>
                 </div>
                 <div class="c-dw-body"><p class="c-dw-empty">这个标识没有对应的观察目标。它可能已被删除，或链接来自另一台机器的库。</p></div>
               </aside>"#,
            drawer = escape(drawer),
        );
    };

    let tab = active_tab.filter(|value| TABS.iter().any(|(key, _)| key == value));
    let tab = tab.unwrap_or("overview");
    let archive = completeness.get(&target.identity_key);
    let is_creator = target.target_kind == "creator";

    format!(
        r#"<aside class="c-dw" aria-label="观察目标工作区">
             <div class="c-dw-head">
               <div class="c-dw-kicker">目标工作区 / {kind}</div>
               <div class="c-dw-title-row">
                 <div>
                   <div class="c-dw-title">{name}</div>
                   <div class="c-dw-meta">{platform} · {handle}</div>
                 </div>
                 <div class="c-dw-actions">
                   {source_link}
                   <a class="c-btn-quiet" href="/collection/targets">关闭 ×</a>
                 </div>
               </div>
               <div class="c-dw-statusline">{statusline}</div>
             </div>
             <nav class="c-dw-tabs">{tabs}</nav>
             <div class="c-dw-body">{body}</div>
           </aside>"#,
        kind = escape(if is_creator { "创作者" } else { "关键词" }),
        name = escape(display_name(target)),
        platform = escape(&target.platform.to_uppercase()),
        handle = escape(&target.identity_key),
        source_link = source_link(target, is_creator),
        statusline = statusline(target),
        tabs = tab_bar(target, tab),
        body = body(target, archive, is_creator, tab),
    )
}

fn display_name(target: &ObservationTarget) -> &str {
    target
        .display_name
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&target.identity_key)
}

/// 打开平台原页。只有创作者给得出链接——关键词没有一个「主页」。
fn source_link(target: &ObservationTarget, is_creator: bool) -> String {
    if !is_creator {
        return String::new();
    }
    format!(
        r#"<a class="c-btn-quiet" href="https://www.xiaohongshu.com/user/profile/{id}" target="_blank" rel="noreferrer">打开原页 ↗</a>"#,
        id = escape(&target.identity_key),
    )
}

fn statusline(target: &ObservationTarget) -> String {
    let archive = match target.lifecycle_state.as_str() {
        "pending_decision" => "尚未建档",
        "archiving" => "建档中",
        "monitoring" => "基线就绪",
        _ => "状态未知",
    };
    let patrol = if target.monitoring_enabled {
        "巡检中"
    } else {
        "未开启巡检"
    };
    format!(
        "{archive} · {patrol} · {group}",
        group = escape(target.group_name.as_deref().unwrap_or("未分组")),
    )
}

fn tab_bar(target: &ObservationTarget, active: &str) -> String {
    TABS.iter()
        .map(|(key, label)| {
            let class = if *key == active { " c-dw-tab-on" } else { "" };
            format!(
                r#"<a class="c-dw-tab{class}" href="/collection/targets?drawer={id}&amp;dtab={key}">{label}</a>"#,
                id = target.target_ref,
            )
        })
        .collect()
}

fn body(
    target: &ObservationTarget,
    archive: Option<&ArchiveCompleteness>,
    is_creator: bool,
    tab: &str,
) -> String {
    match tab {
        "baseline" => baseline_tab(target, archive, is_creator),
        "patrol" => patrol_tab(target),
        // 这三个 tab 背后暂时没有东西。**如实说尚未接通，而不是画一个空壳**——
        // 一个有标题、有格子、没有数的面板，会让人以为「查过了，是空的」。
        "evidence" => not_connected(
            "证据",
            "按目标看证据需要一条从观察目标到语料库的读投影，它还不存在。语料库本身有数据，可从「语料」进入。",
        ),
        "trace" => not_connected(
            "追踪",
            "语义事件流尚未实现。执行留痕目前只到工单与租约这一层，可在「执行工位」查看。",
        ),
        _ => overview_tab(target, archive, is_creator),
    }
}

/// 概览。稿子有粉丝／内容量／中位表现／峰值四格与两张图表。
///
/// **中位表现与峰值需要逐篇互动数据，系统里没有**；图表同理。因此只给有真实来源的那些，
/// 缺的整格写「未采集」——留一个空图表框比不画更糟，它看起来像「数据为零」。
fn overview_tab(
    target: &ObservationTarget,
    archive: Option<&ArchiveCompleteness>,
    is_creator: bool,
) -> String {
    let facts = target.identity_facts.as_ref();
    let followers = facts
        .and_then(|value| value.get("fans"))
        .and_then(serde_json::Value::as_i64)
        .map(|count| count.to_string())
        .unwrap_or_else(|| "未采集".to_owned());
    let works = archive
        .map(|value| value.works_listed)
        .filter(|count| *count > 0)
        .map(|count| count.to_string())
        .unwrap_or_else(|| "未采集".to_owned());
    let bio = facts
        .and_then(|value| value.get("description"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");

    format!(
        r#"{bio_block}
           <section class="c-dw-section">
             <div class="c-dw-section-head"><b>基线 + 长期观察</b><span>只报有分母的读数</span></div>
             <div class="c-dw-readouts">
               <div><b>{followers}</b><span>粉丝</span></div>
               <div><b>{works}</b><span>作品清单</span></div>
               <div><b>未采集</b><span>中位表现</span></div>
               <div><b>未采集</b><span>峰值</span></div>
             </div>
             <p class="c-dw-note">中位表现与峰值需要逐篇互动数据。详情采集尚未跑过，因此这两项没有分母，不给数字。{keyword_note}</p>
           </section>"#,
        bio_block = if bio.is_empty() {
            String::new()
        } else {
            format!(
                r#"<section class="c-dw-section"><div class="c-dw-section-head"><b>简介</b><span>平台公开资料</span></div><p class="c-dw-bio">{bio}</p></section>"#,
                bio = escape(bio),
            )
        },
        keyword_note = if is_creator {
            ""
        } else {
            "关键词来源不生成博主档案，粉丝一项对它不适用。"
        },
    )
}

/// 基线。稿子四格：上次观察 / 下次到期 / 状态 / 缺口。
fn baseline_tab(
    target: &ObservationTarget,
    archive: Option<&ArchiveCompleteness>,
    is_creator: bool,
) -> String {
    let (state, gap) = match archive {
        Some(value) if value.works_listed > 0 => {
            let missing = value.works_listed - value.details_captured.min(value.works_listed);
            ("有作品清单".to_owned(), format!("缺详情 {missing}"))
        }
        Some(value) if value.author_profile_captures > 0 => {
            ("仅作者档案".to_owned(), "无作品清单".to_owned())
        }
        _ => ("尚未采集".to_owned(), "—".to_owned()),
    };
    format!(
        r#"<section class="c-dw-section">
             <div class="c-dw-section-head"><b>基线 ≠ 平台全量</b><span>基线只覆盖已观察到的部分</span></div>
             <div class="c-dw-readouts">
               <div><b>{last}</b><span>上次观察</span></div>
               <div><b>{next}</b><span>下次到期</span></div>
               <div><b>{state}</b><span>状态</span></div>
               <div><b>{gap}</b><span>缺口</span></div>
             </div>
             <p class="c-dw-note">{note}</p>
           </section>"#,
        last = escape(
            target
                .last_patrol_dispatched_at
                .as_deref()
                .unwrap_or("未派过")
        ),
        next = escape(target.next_patrol_at.as_deref().unwrap_or("—")),
        state = escape(&state),
        gap = escape(&gap),
        note = if is_creator {
            "基线是「我们看到过什么」，不是「平台上有什么」。缺口指的是清单里已知、但还没取到详情的那些。"
        } else {
            "关键词的基线是一次搜索面的可见范围，平台上的其余部分始终未知。"
        },
    )
}

/// 巡检策略。系统里目前只有「开关 + 间隔」两项，稿子上是一整套策略面板。
fn patrol_tab(target: &ObservationTarget) -> String {
    format!(
        r#"<section class="c-dw-section">
             <div class="c-dw-section-head"><b>巡检策略</b><span>当前只有开关与间隔两项</span></div>
             <div class="c-dw-readouts">
               <div><b>{enabled}</b><span>巡检</span></div>
               <div><b>{last}</b><span>上次派出</span></div>
               <div><b>{next}</b><span>下次到期</span></div>
               <div><b>未接通</b><span>动态频率</span></div>
             </div>
             <p class="c-dw-note">产品规则定的是「频率由新内容出现的速率决定」，那需要连续几轮的采集结果才算得出来。在有真实速率之前，间隔是一个固定值，不是一个算出来的值——这里如实写「未接通」而不是显示一个看起来像算过的数字。</p>
           </section>"#,
        enabled = if target.monitoring_enabled {
            "已开启"
        } else {
            "未开启"
        },
        last = escape(
            target
                .last_patrol_dispatched_at
                .as_deref()
                .unwrap_or("未派过")
        ),
        next = escape(target.next_patrol_at.as_deref().unwrap_or("—")),
    )
}

/// 尚未接通的 tab。**说清楚缺什么、以及现在能去哪看**，而不是一句「暂无数据」。
fn not_connected(title: &str, reason: &str) -> String {
    format!(
        r#"<section class="c-dw-section">
             <div class="c-dw-section-head"><b>{title}</b><span>尚未接通</span></div>
             <p class="c-dw-note">{reason}</p>
           </section>"#,
        title = escape(title),
        reason = escape(reason),
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
