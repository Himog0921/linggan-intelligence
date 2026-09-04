//! COLLECTION-001 · 观察目标的宽幅研究抽屉。
//!
//! 三个职责 tab（概览 / 档案 / 巡查）留在 Collection；作品正文、评论和媒体结果只在
//! Corpus。创作者概览的视觉核心是作品生命周期分布，不是工程回执或监控价值评分。
//!
//! **用 URL 参数驱动，不用 JS**：`?drawer=<target_ref>&dtab=archive`。稿子那 142 行
//! 脚本换来的是「点击不刷新」，代价是刷新即丢状态、链接分享不过去。URL 版两样都不丢，
//! 而这一页的用途正是「打开一个目标细看，然后把它发给别人」。
//!
//! 稿子上有而系统里没有的读数，一律如实写「未采集」或「尚未接通」（Mog 于 2026-08-28
//! 选定方案 A），不填假数。

use linggan_evidence::{
    ArchiveCompleteness, CreatorLifecycleAssociation, CreatorLifecycleMetric,
    CreatorLifecyclePoint, CreatorLifecycleProjection, CreatorLifecycleStatus,
    CreatorLifecycleWindow, ObservationTarget,
};

/// Collection 目标抽屉的三个职责。Evidence 已退回唯一的 Corpus 表面。退役或未知
/// 值统一归一到 Overview；读取守卫和渲染都只消费这个闭集，不能各自猜一次。
const TABS: &[(TargetDrawerTab, &str)] = &[
    (TargetDrawerTab::Overview, "概览"),
    (TargetDrawerTab::Baseline, "档案"),
    (TargetDrawerTab::Patrol, "巡查"),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetDrawerTab {
    Overview,
    Baseline,
    Patrol,
}

impl TargetDrawerTab {
    pub fn parse(value: Option<&str>) -> Self {
        match value {
            Some("archive" | "baseline") => Self::Baseline,
            Some("patrol") => Self::Patrol,
            // `baseline` remains a compatibility alias. Retired `evidence` / `trace` and any
            // future spelling open the safe default Overview and actually read it.
            None | Some("overview") | Some("evidence") | Some("trace") | Some(_) => Self::Overview,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Overview => "overview",
            Self::Baseline => "archive",
            Self::Patrol => "patrol",
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TargetListContext<'a> {
    pub filter: Option<&'a str>,
    pub sort: Option<&'a str>,
}

impl<'a> TargetListContext<'a> {
    fn pairs(self) -> Vec<(&'static str, &'a str)> {
        let mut pairs = Vec::new();
        if let Some(filter @ ("creator" | "keyword" | "archiving" | "monitoring")) = self.filter {
            pairs.push(("filter", filter));
        }
        if let Some(sort @ "last") = self.sort {
            pairs.push(("sort", sort));
        }
        pairs
    }

    pub fn list_href(self, focus_id: Option<&str>) -> String {
        let pairs = self.pairs();
        let mut href = "/collection/targets".to_owned();
        if !pairs.is_empty() {
            href.push('?');
            href.push_str(
                &pairs
                    .iter()
                    .map(|(key, value)| format!("{key}={value}"))
                    .collect::<Vec<_>>()
                    .join("&amp;"),
            );
        }
        if let Some(focus_id) = focus_id {
            href.push('#');
            href.push_str(focus_id);
        }
        href
    }

    pub fn drawer_href(
        self,
        target_ref: uuid::Uuid,
        params: &[(&str, &str)],
        fragment: Option<&str>,
    ) -> String {
        let mut pairs = self
            .pairs()
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .collect::<Vec<_>>();
        pairs.push(("drawer".to_owned(), target_ref.to_string()));
        pairs.extend(
            params
                .iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned())),
        );
        let mut href = format!(
            "/collection/targets?{}",
            pairs
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect::<Vec<_>>()
                .join("&amp;")
        );
        if let Some(fragment) = fragment {
            href.push('#');
            href.push_str(fragment);
        }
        href
    }

    /// Open the target-scoped monitoring rule overlay without carrying drawer state into it.
    /// The list filter and sort remain in the URL so closing the overlay returns to the same
    /// working set. `opener_id` is a fragment only: it restores keyboard focus and never
    /// participates in target identity.
    pub fn monitor_rule_href(self, target_ref: uuid::Uuid, opener_id: &str) -> String {
        let mut pairs = self
            .pairs()
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .collect::<Vec<_>>();
        pairs.push(("rule".to_owned(), target_ref.to_string()));
        format!(
            "/collection/targets?{}#{}",
            pairs
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect::<Vec<_>>()
                .join("&amp;"),
            opener_id,
        )
    }
}

#[derive(Clone, Copy)]
pub enum LifecycleView<'a> {
    Projection(&'a CreatorLifecycleProjection),
    ReadUnavailable {
        window: CreatorLifecycleWindow,
        metric: CreatorLifecycleMetric,
    },
    NotRead {
        window: CreatorLifecycleWindow,
        metric: CreatorLifecycleMetric,
    },
    QueryInvalid,
}

/// Collection's base renderer owns the closing document and script tag. Mount the drawer before
/// that script so the behavior module can bind Escape on first parse; appending after `</html>`
/// produces invalid markup and makes the drawer invisible to the script at execution time.
pub fn attach_to_collection_document(document: &str, drawer: &str) -> String {
    if drawer.is_empty() {
        return document.to_owned();
    }
    const SCRIPT: &str = r#"<script src="/assets/collection-workspace.js"></script>"#;
    let Some(script_at) = document.rfind(SCRIPT) else {
        return document.to_owned();
    };
    format!(
        "{}{}{}",
        &document[..script_at],
        drawer,
        &document[script_at..]
    )
}

/// 渲染抽屉。`drawer` 为空时整块不渲染——没有选中目标时不该有一个空壳挂在那里。
pub fn render(
    target: Option<&ObservationTarget>,
    completeness: &std::collections::HashMap<String, ArchiveCompleteness>,
    drawer: Option<&str>,
    active_tab: TargetDrawerTab,
    lifecycle: LifecycleView<'_>,
    selected_work: Option<&str>,
    list_context: TargetListContext<'_>,
) -> String {
    let Some(drawer) = drawer else {
        return String::new();
    };
    // 找不到就如实说找不到，而不是静默关掉抽屉——链接失效与「我没点开」是两回事。
    //
    // 目标由调用方**独立查询**得到，不从当前列表里找：列表是筛过的，一个被筛掉的目标
    // 会让这里说「未找到」，而它其实好好地在库里——那是在撒谎。
    let Some(target) = target else {
        let return_focus = uuid::Uuid::parse_str(drawer)
            .ok()
            .map(|target_ref| format!("target-{target_ref}"));
        let return_href = list_context.list_href(return_focus.as_deref());
        let return_focus_attr = return_focus
            .as_deref()
            .map(|focus| format!(r#" data-return-focus="{focus}""#))
            .unwrap_or_default();
        return format!(
            r#"<aside id="c-drawer" class="c-dw" aria-label="观察目标工作区" data-return-url="{return_url}"{return_focus_attr}>
                 <div class="c-dw-head">
                   <div class="c-dw-kicker">观察档案</div>
                   <div class="c-dw-title-row">
                     <div><div class="c-dw-title">未找到该观察目标</div>
                       <div class="c-dw-meta">#{drawer}</div></div>
                     <div class="c-dw-actions"><a class="c-btn-quiet" href="{return_href}">关闭</a></div>
                   </div>
                 </div>
                 <div class="c-dw-body"><p class="c-dw-empty">这个标识没有对应的观察目标。它可能已被删除，或链接来自另一台机器的库。</p></div>
               </aside>"#,
            drawer = escape(drawer),
            return_url = list_context.list_href(None),
        );
    };

    let archive = completeness.get(&target.identity_key);
    let is_creator = target.target_kind == "creator";
    let tab = if !is_creator && active_tab == TargetDrawerTab::Baseline {
        TargetDrawerTab::Overview
    } else {
        active_tab
    };
    let return_focus = format!("target-{}", target.target_ref);
    let return_href = list_context.list_href(Some(&return_focus));

    format!(
        r#"<aside id="c-drawer" class="c-dw" aria-label="观察目标工作区" data-return-url="{return_url}" data-return-focus="{return_focus}">
             <div class="c-dw-head">
               <div class="c-dw-kicker">{workspace}</div>
               <div class="c-dw-title-row">
                 <div>
                   <div class="c-dw-title">{name}</div>
                   <div class="c-dw-meta">{platform}{handle}</div>
                 </div>
                 <div class="c-dw-actions">
                   {source_link}
                   <a class="c-btn-quiet" href="{return_href}">关闭</a>
                 </div>
               </div>
               <div class="c-dw-statusline">{statusline}</div>
             </div>
             <nav class="c-dw-tabs">{tabs}</nav>
             <div class="c-dw-body">{body}</div>
           </aside>"#,
        workspace = escape(if is_creator {
            "创作者档案"
        } else {
            "关键词观察"
        }),
        name = escape(display_name(target)),
        platform = escape(platform_label(&target.platform)),
        handle = identity_handle(target)
            .map(|handle| format!(" · {}", escape(handle)))
            .unwrap_or_default(),
        return_url = list_context.list_href(None),
        source_link = source_link(target, is_creator),
        statusline = statusline(target, archive),
        tabs = tab_bar(target, tab, lifecycle, selected_work, list_context),
        body = body(
            target,
            archive,
            is_creator,
            tab,
            lifecycle,
            selected_work,
            list_context,
        ),
    )
}

fn display_name(target: &ObservationTarget) -> &str {
    target
        .display_name
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&target.identity_key)
}

fn identity_handle(target: &ObservationTarget) -> Option<&str> {
    target
        .identity_facts
        .as_ref()
        .and_then(|facts| facts.get("redId"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn platform_label(platform: &str) -> &str {
    match platform {
        "xhs" => "小红书",
        other => other,
    }
}

/// One kind-aware lifecycle vocabulary is shared by the target ledger and drawer. Keyword
/// targets never run creator baseline archiving, so a monitoring keyword must not inherit the
/// creator-only "baseline ready" claim merely because the state spelling is shared.
pub(crate) fn lifecycle_primary_copy(
    target_kind: &str,
    lifecycle_state: &str,
) -> (&'static str, &'static str) {
    match (target_kind, lifecycle_state) {
        ("creator", "pending_decision") => ("neutral", "尚未建档"),
        ("creator", "archiving") => ("warn", "建档中"),
        ("creator", "archived" | "monitoring" | "paused") => ("ok", "档案已建立"),
        ("creator", "dismissed") => ("neutral", "已停止观察"),
        ("keyword", "pending_decision") => ("neutral", "等待决定"),
        ("keyword", "monitoring") => ("ok", "规则已生效"),
        ("keyword", "paused") => ("warn", "规则已暂停"),
        ("keyword", "dismissed") => ("neutral", "已停止观察"),
        ("keyword", "archiving" | "archived") => ("warn", "状态与目标类型冲突"),
        _ => ("neutral", "状态未知"),
    }
}

pub(crate) fn lifecycle_patrol_copy(target: &ObservationTarget) -> (&'static str, &'static str) {
    match target.lifecycle_state.as_str() {
        "paused" => ("warn", "巡查已暂停"),
        "dismissed" => ("neutral", "不再调度"),
        _ if target.monitoring_enabled => ("info", "巡查中"),
        _ => ("neutral", "未开启巡查"),
    }
}

/// 打开平台原页。只有创作者给得出链接——关键词没有一个「主页」。
fn source_link(target: &ObservationTarget, is_creator: bool) -> String {
    if !is_creator {
        return String::new();
    }
    format!(
        r#"<a class="c-btn-quiet" href="https://www.xiaohongshu.com/user/profile/{id}" target="_blank" rel="noreferrer">打开原页</a>"#,
        id = escape(&target.identity_key),
    )
}

fn statusline(target: &ObservationTarget, completeness: Option<&ArchiveCompleteness>) -> String {
    let archive = if target.target_kind != "creator" {
        lifecycle_primary_copy(&target.target_kind, &target.lifecycle_state).1
    } else if completeness.is_some_and(|value| value.work_in_progress) {
        "建档中"
    } else if completeness.is_none_or(ArchiveCompleteness::is_untouched) {
        "尚未建档"
    } else {
        "档案已建立"
    };
    let (_, patrol) = lifecycle_patrol_copy(target);
    format!(
        "{archive} · {patrol} · {group}",
        group = escape(target.group_name.as_deref().unwrap_or("未分组")),
    )
}

fn tab_bar(
    target: &ObservationTarget,
    active: TargetDrawerTab,
    lifecycle: LifecycleView<'_>,
    selected_work: Option<&str>,
    list_context: TargetListContext<'_>,
) -> String {
    let lifecycle_state = lifecycle_query_state(lifecycle, selected_work);
    TABS.iter()
        .filter(|(key, _)| target.target_kind == "creator" || *key != TargetDrawerTab::Baseline)
        .map(|(key, label)| {
            let class = if *key == active { " c-dw-tab-on" } else { "" };
            let current = if *key == active {
                r#" aria-current="page""#
            } else {
                ""
            };
            let lifecycle_state = lifecycle_state
                .iter()
                .map(|(key, value)| (*key, value.as_str()))
                .collect::<Vec<_>>();
            let mut params = vec![("dtab", key.as_str())];
            params.extend(lifecycle_state);
            let href = list_context.drawer_href(target.target_ref, &params, None);
            format!(r#"<a class="c-dw-tab{class}" href="{href}"{current}>{label}</a>"#,)
        })
        .collect()
}

fn body(
    target: &ObservationTarget,
    archive: Option<&ArchiveCompleteness>,
    is_creator: bool,
    tab: TargetDrawerTab,
    lifecycle: LifecycleView<'_>,
    selected_work: Option<&str>,
    list_context: TargetListContext<'_>,
) -> String {
    match tab {
        TargetDrawerTab::Baseline => archive_tab(target, archive, is_creator, list_context),
        TargetDrawerTab::Patrol => patrol_tab(target, list_context),
        TargetDrawerTab::Overview => overview_tab(
            target,
            archive,
            is_creator,
            lifecycle,
            selected_work,
            list_context,
        ),
    }
}

/// 概览只回答对象是谁、最近发生了什么、作品如何分布、档案还缺什么。
/// 缺失的最近变化统计直接表达为尚未取得，不用零值或工程诊断语言填充。
fn overview_tab(
    target: &ObservationTarget,
    archive: Option<&ArchiveCompleteness>,
    is_creator: bool,
    lifecycle: LifecycleView<'_>,
    selected_work: Option<&str>,
    list_context: TargetListContext<'_>,
) -> String {
    if !is_creator {
        return format!(
            r#"<section class="c-dw-section">
                  <div class="c-dw-section-head"><b>关键词观察</b><span>持续搜索</span></div>
                  <p class="c-dw-note">这里用于查看关键词巡查是否运行、何时有结果，以及后续接通的新命中与数据更新。关键词不建立创作者作品档案。</p>
                </section>{}"#,
            recent_activity(target),
        );
    }
    let facts = target.identity_facts.as_ref();
    let bio = facts
        .and_then(|value| value.get("description"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");

    format!(
        r#"{bio_block}{recent}{lifecycle}{gaps}"#,
        bio_block = if bio.is_empty() {
            String::new()
        } else {
            format!(
                r#"<section class="c-dw-section c-dw-profile"><div class="c-dw-section-head"><b>创作者简介</b><span>平台公开资料</span></div><p class="c-dw-bio">{bio}</p></section>"#,
                bio = escape(bio),
            )
        },
        recent = recent_activity(target),
        lifecycle = lifecycle_overview(target, is_creator, lifecycle, selected_work, list_context,),
        gaps = archive_gap_overview(archive, is_creator, lifecycle),
    )
}

fn recent_activity(target: &ObservationTarget) -> String {
    let (headline, note) = match target.last_patrol_succeeded_at.as_deref() {
        Some(last) => (
            format!("最近一次有效巡查完成于 {}", escape(last)),
            "当前还没有可显示的本轮新增作品和数据更新汇总。已接纳的新作品和指标会自动进入下面的作品生命周期。",
        ),
        None if target.monitoring_enabled => (
            "尚无巡查结果".to_owned(),
            "巡查已经开启，但还没有成功结果；这里不会把未知写成零变化。",
        ),
        None if target.target_kind == "keyword" => (
            "尚未开始关键词巡查".to_owned(),
            "设置巡查后，这里会显示最近一次取得有效结果的时间；没有结果时不会写成零命中。",
        ),
        None => (
            "尚未开始持续巡查".to_owned(),
            "当前档案只会在手动建立或继续完善时更新。",
        ),
    };
    format!(
        r#"<section class="c-dw-section c-dw-activity">
              <div class="c-dw-section-head"><b>最近变化</b><span>巡查结果</span></div>
              <strong>{headline}</strong><p>{note}</p>
            </section>"#,
    )
}

fn lifecycle_query_state(
    lifecycle: LifecycleView<'_>,
    selected_work: Option<&str>,
) -> Vec<(&'static str, String)> {
    let (window, metric, validate_selection) = match lifecycle {
        LifecycleView::Projection(projection) => (
            projection.window,
            projection.metric,
            Some(&projection.points),
        ),
        LifecycleView::ReadUnavailable { window, metric }
        | LifecycleView::NotRead { window, metric } => (window, metric, None),
        LifecycleView::QueryInvalid => return Vec::new(),
    };
    let selected = selected_work
        .filter(|candidate| {
            validate_selection
                .map(|points| {
                    points
                        .iter()
                        .any(|point| point.work_public_ref.to_string() == *candidate)
                })
                .unwrap_or(true)
        })
        .map(str::to_owned);
    let mut params = vec![
        ("life_window", window.as_str().to_owned()),
        ("life_metric", metric.as_str().to_owned()),
    ];
    if let Some(selected) = selected {
        params.push(("life_work", selected));
    }
    params
}

fn lifecycle_overview(
    target: &ObservationTarget,
    is_creator: bool,
    lifecycle: LifecycleView<'_>,
    selected_work: Option<&str>,
    list_context: TargetListContext<'_>,
) -> String {
    if matches!(lifecycle, LifecycleView::QueryInvalid) {
        return lifecycle_state(
            "作品分布的查询条件无效",
            "地址里的时间范围或指标不受支持。页面没有改用默认条件，也没有把失败读取成空数据。",
        );
    }
    if !is_creator {
        return lifecycle_state(
            "不适用于关键词目标",
            "关键词按搜索命中持续观察，不拥有创作者作品档案，因此这里不显示创作者生命周期图。",
        );
    }
    let projection = match lifecycle {
        LifecycleView::Projection(projection) => projection,
        LifecycleView::ReadUnavailable { .. }
        | LifecycleView::NotRead { .. }
        | LifecycleView::QueryInvalid => {
            return lifecycle_state(
                "作品分布当前读不到",
                "当前读取状态未知，不表示创作者没有作品，也不表示互动数据为零。",
            );
        }
    };
    if projection.status == CreatorLifecycleStatus::NotApplicable {
        return lifecycle_state(
            "不适用于当前目标",
            "只有已确认身份的创作者目标才能生成作品生命周期。",
        );
    }

    let controls = lifecycle_controls(target, projection, list_context);
    let (linked, linked_label) = match projection.summary.linked_work_count {
        Some(count) => (count.to_string(), "作品目录"),
        None => (
            format!("≥{}", projection.summary.linked_work_count_lower_bound),
            "作品目录下限",
        ),
    };
    let summary = format!(
        r#"<div class="life-summary" aria-label="生命周期覆盖摘要">
              <div><b>{linked}</b><span>{linked_label}</span></div>
              <div><b>{confirmed}</b><span>详情已确认</span></div>
              <div><b>{eligible}</b><span>当前可分析</span></div>
            </div>"#,
        confirmed = projection.summary.confirmed_author_work_count,
        eligible = projection.summary.eligible_point_count,
    );
    let legend = r#"<div class="life-legend" aria-label="散点含义">
          <span><i class="life-legend-dot life-legend-directory"></i>主页目录，详情待确认</span>
          <span><i class="life-legend-dot life-legend-confirmed"></i>详情作者已确认</span>
          <span><i class="life-legend-dot life-legend-new"></i>最近巡查新增</span>
        </div>"#;
    let chart = if projection.points.is_empty() {
        lifecycle_state(
            "观察不足，暂时无法成图",
            "作品必须同时具备可确认的作者归属、真实发布时间和当前互动数据。未知值不会按零计算。",
        )
    } else {
        lifecycle_chart(target, projection, selected_work, list_context)
    };
    let selected = selected_work
        .and_then(|selected| {
            projection
                .points
                .iter()
                .find(|point| point.work_public_ref.to_string() == selected)
        })
        .map(|point| selected_work_summary(point, projection.metric))
        .unwrap_or_default();

    format!(
        r#"<section class="c-dw-section life-panel" id="creator-lifecycle">
              <div class="life-heading">
                <div><h2>作品生命周期</h2><p>横轴是作品发布时间，纵轴是所选互动数据</p></div>
                <p>数据截至 {as_of}</p>
              </div>
              {controls}{summary}{legend}{chart}
              {selected}
            </section>"#,
        as_of = escape(&projection.as_of),
    )
}

fn lifecycle_controls(
    target: &ObservationTarget,
    projection: &CreatorLifecycleProjection,
    list_context: TargetListContext<'_>,
) -> String {
    let windows = [
        (CreatorLifecycleWindow::Recent90Days, "近 90 天"),
        (CreatorLifecycleWindow::All, "全部周期"),
    ]
    .into_iter()
    .map(|(window, label)| {
        let current = if window == projection.window {
            r#" aria-current="true""#
        } else {
            ""
        };
        let href = list_context.drawer_href(
            target.target_ref,
            &[
                ("life_window", window.as_str()),
                ("life_metric", projection.metric.as_str()),
            ],
            Some("creator-lifecycle"),
        );
        format!(r#"<a class="life-chip"{current} href="{href}">{label}</a>"#)
    })
    .collect::<String>();
    let metrics = [
        (CreatorLifecycleMetric::Likes, "点赞"),
        (CreatorLifecycleMetric::Comments, "评论"),
        (CreatorLifecycleMetric::Collects, "收藏"),
        (CreatorLifecycleMetric::Shares, "转发"),
    ]
    .into_iter()
    .map(|(metric, label)| {
        let current = if metric == projection.metric {
            r#" aria-current="true""#
        } else {
            ""
        };
        let href = list_context.drawer_href(
            target.target_ref,
            &[
                ("life_window", projection.window.as_str()),
                ("life_metric", metric.as_str()),
            ],
            Some("creator-lifecycle"),
        );
        format!(r#"<a class="life-chip"{current} href="{href}">{label}</a>"#)
    })
    .collect::<String>();
    format!(
        r#"<div class="life-controls" aria-label="生命周期口径">
              <div class="life-control-row"><span class="life-control-label">时间</span>{windows}</div>
              <div class="life-control-row"><span class="life-control-label">指标</span>{metrics}</div>
            </div>"#,
    )
}

fn lifecycle_chart(
    target: &ObservationTarget,
    projection: &CreatorLifecycleProjection,
    selected_work: Option<&str>,
    list_context: TargetListContext<'_>,
) -> String {
    const WIDTH: f64 = 800.0;
    const HEIGHT: f64 = 300.0;
    const LEFT: f64 = 54.0;
    // Keep the non-scaling hit ring inside the plot at the supported desktop drawer width.
    const RIGHT: f64 = 36.0;
    const TOP: f64 = 20.0;
    const BOTTOM: f64 = 40.0;
    let min_x = projection
        .points
        .first()
        .map(|point| point.published_at_epoch_ms)
        .unwrap_or(0);
    let max_x = projection
        .points
        .last()
        .map(|point| point.published_at_epoch_ms)
        .unwrap_or(min_x);
    let x_span = (max_x - min_x).max(1) as f64;
    let max_y = projection
        .points
        .iter()
        .map(|point| (point.metric_value as f64).ln_1p())
        .fold(0.0_f64, f64::max)
        .max(1.0);
    let x_for = |point: &CreatorLifecyclePoint| {
        LEFT + (point.published_at_epoch_ms - min_x) as f64 / x_span * (WIDTH - LEFT - RIGHT)
    };
    let y_for = |value: f64| TOP + (1.0 - value.ln_1p() / max_y) * (HEIGHT - TOP - BOTTOM);
    let points = projection
        .points
        .iter()
        .map(|point| {
            let selected_class = if selected_work
                .map(|selected| point.work_public_ref.to_string() == selected)
                .unwrap_or(false)
            {
                " life-point-selected"
            } else {
                ""
            };
            let (association_class, association_label) = match point.association_state {
                CreatorLifecycleAssociation::DirectoryLinked => {
                    (" life-point-directory", "来自主页作品目录，详情作者待确认")
                }
                CreatorLifecycleAssociation::AuthorConfirmed => {
                    (" life-point-confirmed", "详情作者已确认")
                }
            };
            let (new_class, new_label, new_ring) = if point.new_in_latest_patrol {
                (
                    " life-point-new",
                    "，最近一次巡查新增",
                    format!(
                        r#"<circle class="life-point-new-ring" cx="{:.1}" cy="{:.1}" r="9" aria-hidden="true"/>"#,
                        x_for(point),
                        y_for(point.metric_value as f64),
                    ),
                )
            } else {
                ("", "", String::new())
            };
            let title = point.title.as_deref().unwrap_or("标题未知");
            let work = point.work_public_ref.to_string();
            let href = list_context.drawer_href(
                target.target_ref,
                &[
                    ("life_window", projection.window.as_str()),
                    ("life_metric", projection.metric.as_str()),
                    ("life_work", work.as_str()),
                ],
                Some("creator-lifecycle"),
            );
            format!(
                r#"<a class="life-point{association_class}{new_class}{selected_class}" href="{href}" aria-label="{title}，{published}，{metric_label} {value}，{association_label}{new_label}">{new_ring}<circle class="life-point-hit" cx="{x:.1}" cy="{y:.1}" r="6" aria-hidden="true"/><circle class="life-point-visible" cx="{x:.1}" cy="{y:.1}" r="5" aria-hidden="true"/></a>"#,
                title = escape(title),
                published = escape(&point.published_local_date),
                metric_label = metric_label(projection.metric),
                value = point.metric_value,
                x = x_for(point),
                y = y_for(point.metric_value as f64),
            )
        })
        .collect::<String>();
    let start = projection
        .points
        .first()
        .map(|point| point.published_local_date.as_str())
        .unwrap_or("—");
    let end = projection
        .points
        .last()
        .map(|point| point.published_local_date.as_str())
        .unwrap_or("—");
    let window_caption = match projection.window {
        CreatorLifecycleWindow::Recent90Days => "UTC+08 近 90 个日历日（含首尾）",
        CreatorLifecycleWindow::All => "UTC+08 全部合格历史",
    };
    format!(
        r#"<figure class="life-figure">
              <svg class="life-chart" viewBox="0 0 800 300" role="img" aria-labelledby="life-chart-title life-chart-desc">
                <title id="life-chart-title">创作者作品生命周期散点图</title>
                <desc id="life-chart-desc">横轴为合格作品发布时间，纵轴压缩互动量差距。每个点代表一篇当前可分析作品。</desc>
                <line class="life-axis" x1="54" y1="260" x2="782" y2="260"/>
                <line class="life-axis" x1="54" y1="20" x2="54" y2="260"/>
                {points}
                <text class="life-axis-copy" x="54" y="284">{start}</text>
                <text class="life-axis-copy" x="782" y="284" text-anchor="end">{end}</text>
                <text class="life-axis-copy" x="14" y="142" transform="rotate(-90 14 142)" text-anchor="middle">互动量</text>
              </svg>
              <figcaption class="life-caption"><span>作品发布时间 · {window_caption}</span><span>纵轴压缩差距，选中作品仍显示原始数值</span></figcaption>
            </figure>"#,
        start = escape(start),
        end = escape(end),
    )
}

fn lifecycle_exclusions(projection: &CreatorLifecycleProjection) -> String {
    let exclusion_items = [
        ("作者未确认", projection.exclusions.author_not_verified),
        ("作者不匹配", projection.exclusions.author_mismatch),
        (
            "发布时间不合格",
            projection.exclusions.published_at_not_qualified,
        ),
        ("不在当前时间窗", projection.exclusions.outside_window),
        ("指标未知", projection.exclusions.metric_unknown),
    ]
    .into_iter()
    .filter(|(_, count)| *count > 0)
    .map(|(label, count)| format!("<li>{label} {count}</li>"))
    .collect::<String>();
    if exclusion_items.is_empty() && !projection.receipt.truncated {
        return String::new();
    }
    let truncated = if projection.receipt.truncated {
        "<li>当前作品范围仍有后续内容</li>"
    } else {
        ""
    };
    format!(
        r#"<ul class="life-exclusions" aria-label="未进入当前曲线的原因">{exclusion_items}{truncated}</ul>"#,
    )
}

fn selected_work_summary(point: &CreatorLifecyclePoint, metric: CreatorLifecycleMetric) -> String {
    format!(
        r#"<section class="life-selected" aria-label="选中作品摘要">
              <h3>{title}</h3>
              <dl>
                <div><dt>发布时间</dt><dd>{published}</dd></div>
                <div><dt>{metric}</dt><dd>{value}</dd></div>
              </dl>
              <a class="life-corpus-link" href="/corpus/evidence?work={work}">到语料页查看这篇作品</a>
            </section>"#,
        title = escape(point.title.as_deref().unwrap_or("标题未知")),
        published = escape(&point.published_local_date),
        metric = metric_label(metric),
        value = point.metric_value,
        work = point.work_public_ref,
    )
}

fn lifecycle_state(title: &str, body: &str) -> String {
    format!(
        r#"<section class="c-dw-section life-panel"><div class="life-heading"><h2>作品生命周期</h2></div><div class="life-state"><b>{title}</b><p>{body}</p></div></section>"#,
        title = escape(title),
        body = escape(body),
    )
}

fn metric_label(metric: CreatorLifecycleMetric) -> &'static str {
    match metric {
        CreatorLifecycleMetric::Likes => "点赞",
        CreatorLifecycleMetric::Comments => "评论",
        CreatorLifecycleMetric::Collects => "收藏",
        CreatorLifecycleMetric::Shares => "转发",
    }
}

fn archive_gap_overview(
    archive: Option<&ArchiveCompleteness>,
    is_creator: bool,
    lifecycle: LifecycleView<'_>,
) -> String {
    if !is_creator {
        return String::new();
    }
    let directory = archive
        .filter(|value| !value.is_untouched())
        .map(|value| value.works_listed.to_string())
        .unwrap_or_else(|| "—".to_owned());
    let detail = archive
        .filter(|value| value.works_listed > 0)
        .map(|value| format!("{} / {}", value.details_captured, value.works_listed))
        .unwrap_or_else(|| "—".to_owned());
    let analyzable = match lifecycle {
        LifecycleView::Projection(projection) => {
            projection.summary.eligible_point_count.to_string()
        }
        _ => "—".to_owned(),
    };
    let mut gaps = match lifecycle {
        LifecycleView::Projection(projection) => lifecycle_exclusions(projection),
        _ => String::new(),
    };
    if let Some(count) = archive
        .map(|value| value.quarantined)
        .filter(|count| *count > 0)
    {
        gaps.push_str(&format!(
            r#"<ul class="life-exclusions" aria-label="档案待处理项"><li>待处理记录 {count}</li></ul>"#
        ));
    }
    if gaps.is_empty() {
        gaps = r#"<p class="c-dw-note">当前读取没有给出额外缺口；这只描述已建立的作品目录，不代表平台全部作品。</p>"#.to_owned();
    }
    format!(
        r#"<section class="c-dw-section c-dw-archive-summary" id="target-archive">
              <div class="c-dw-section-head"><b>档案缺口</b><span>分别看，不合成总分</span></div>
              <div class="c-dw-readouts c-dw-archive-readouts">
                <div><b>{directory}</b><span>作品目录</span></div>
                <div><b>{detail}</b><span>详情进度</span></div>
                <div><b>{analyzable}</b><span>当前可分析</span></div>
                <div><b>语料页</b><span>评论与深层材料</span></div>
              </div>
              {gaps}
            </section>"#,
    )
}

/// 档案把目录、详情、可分析和深层材料分开，不能把不同分母压成一个完成百分比。
fn archive_tab(
    target: &ObservationTarget,
    archive: Option<&ArchiveCompleteness>,
    is_creator: bool,
    list_context: TargetListContext<'_>,
) -> String {
    if !is_creator {
        return lifecycle_state(
            "关键词不建立创作者档案",
            "关键词观察记录搜索命中与巡查变化，不扫描某个创作者的作品目录，也不显示创作者作品分布。",
        );
    }
    let untouched = archive.is_none_or(ArchiveCompleteness::is_untouched);
    let directory = archive
        .filter(|value| !value.is_untouched())
        .map(|value| value.works_listed.to_string())
        .unwrap_or_else(|| "—".to_owned());
    let detail = archive
        .filter(|value| value.works_listed > 0)
        .map(|value| format!("{} / {}", value.details_captured, value.works_listed))
        .unwrap_or_else(|| "—".to_owned());
    let missing = archive
        .filter(|value| value.works_listed > value.details_captured)
        .map(|value| value.works_listed - value.details_captured)
        .unwrap_or(0);
    let action = if archive.is_some_and(|value| value.work_in_progress) {
        r#"<span class="c-dw-action-note">档案正在建立，完成范围和待补缺口以实际采集结果为准。</span>"#
            .to_owned()
    } else if untouched || missing > 0 || archive.is_some_and(|value| value.works_listed == 0) {
        let label = if untouched {
            "建立档案"
        } else {
            "继续完善"
        };
        format!(
            r#"<form class="c-dw-primary-form" method="post" action="/collection/targets/archive">
                  <button class="c-btn-primary" type="submit" name="row_target_ref" value="{target_ref}">{label}</button>
                </form>"#,
            target_ref = target.target_ref,
        )
    } else {
        let href = list_context.drawer_href(
            target.target_ref,
            &[("dtab", "overview")],
            Some("creator-lifecycle"),
        );
        format!(r#"<a class="c-btn-secondary" href="{href}">查看作品分布</a>"#)
    };
    format!(
        r#"<section class="c-dw-section">
             <div class="c-dw-section-head"><b>作品档案</b><span>当前已取得范围</span></div>
             <div class="c-dw-readouts c-dw-archive-readouts">
               <div><b>{directory}</b><span>作品目录</span></div>
               <div><b>{detail}</b><span>详情进度</span></div>
               <div><b>—</b><span>当前可分析</span></div>
               <div><b>语料页</b><span>评论与深层材料</span></div>
             </div>
             <p class="c-dw-note">建立档案会读取创作者主页当前可见的前 200 篇作品链接作为上限，先建立去重目录，再逐步补齐详情与已授权的数据化处理。200 不是平台总作品数。</p>
             {action}
           </section>"#,
    )
}

/// 巡查只展示当前可读的开关与时间，完整规则通过已有版本化规则入口管理。
fn patrol_tab(target: &ObservationTarget, list_context: TargetListContext<'_>) -> String {
    let opener_id = format!("drawer-monitor-rule-{}", target.target_ref);
    let rule_href = list_context.monitor_rule_href(target.target_ref, &opener_id);
    format!(
        r#"<section class="c-dw-section">
             <div class="c-dw-section-head"><b>持续巡查</b><span>当前规则与时间</span></div>
             <div class="c-dw-readouts">
               <div><b>{enabled}</b><span>巡查</span></div>
               <div><b>{last}</b><span>上次巡查</span></div>
               <div><b>{next}</b><span>下次巡查</span></div>
               <div><b>尚未取得</b><span>最近结果</span></div>
             </div>
             <p class="c-dw-note">巡查发现的新作品和后续互动数据，在接纳后会进入同一作品目录与生命周期。当前还没有可显示的本轮新增和数据更新汇总。</p>
             <a id="{opener_id}" class="c-btn-secondary" data-monitor-rule-trigger="{target_ref}" href="{rule_href}">管理巡查</a>
           </section>"#,
        enabled = if target.monitoring_enabled {
            "已开启"
        } else {
            "未开启"
        },
        last = escape(
            target
                .last_patrol_succeeded_at
                .as_deref()
                .unwrap_or("尚未巡查")
        ),
        next = escape(target.next_patrol_at.as_deref().unwrap_or("—")),
        target_ref = target.target_ref,
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

    fn target(state: &str) -> ObservationTarget {
        ObservationTarget {
            target_ref: uuid::Uuid::new_v4(),
            platform: "xhs".to_owned(),
            target_kind: "creator".to_owned(),
            identity_key: "creator-lifecycle-test".to_owned(),
            display_name: Some("生命周期作者".to_owned()),
            identity_facts: None,
            source: "manual".to_owned(),
            lifecycle_state: state.to_owned(),
            first_stored_at: "2026-09-04T09:00:00+08".to_owned(),
            monitoring_enabled: state == "monitoring",
            group_name: None,
            last_patrol_dispatched_at: None,
            last_patrol_succeeded_at: None,
            next_patrol_at: None,
        }
    }

    #[test]
    fn drawer_statusline_uses_durable_archive_progress() {
        let target = target("archiving");
        let untouched = statusline(&target, None);
        assert!(untouched.contains("尚未建档"));
        assert!(!untouched.contains("建档中"));

        let in_progress = ArchiveCompleteness {
            work_in_progress: true,
            ..ArchiveCompleteness::default()
        };
        assert!(statusline(&target, Some(&in_progress)).contains("建档中"));

        let established = ArchiveCompleteness {
            work_in_progress: false,
            author_profile_captures: 1,
            works_listed: 12,
            details_captured: 5,
            quarantined: 0,
        };
        assert!(statusline(&target, Some(&established)).contains("档案已建立"));
    }
}
