//! COLLECTION-001 · 观察目标的宽幅研究抽屉。
//!
//! 四个职责 tab（概览 / 基线 / 巡检策略 / 追踪）留在 Collection；Evidence 结果只在
//! Corpus。创作者概览的视觉核心是 target-scoped Work 生命周期，不是监控价值评分。
//!
//! **用 URL 参数驱动，不用 JS**：`?drawer=<target_ref>&dtab=baseline`。稿子那 142 行
//! 脚本换来的是「点击不刷新」，代价是刷新即丢状态、链接分享不过去。URL 版两样都不丢，
//! 而这一页的用途正是「打开一个目标细看，然后把它发给别人」。
//!
//! 稿子上有而系统里没有的读数，一律如实写「未采集」或「尚未接通」（Mog 于 2026-08-28
//! 选定方案 A），不填假数。

use linggan_evidence::{
    ArchiveCompleteness, CreatorLifecycleMetric, CreatorLifecyclePoint, CreatorLifecycleProjection,
    CreatorLifecycleStatus, CreatorLifecycleWindow, ObservationTarget,
};

/// Collection 目标抽屉的四个职责。Evidence 已退回唯一的 Corpus 表面。退役或未知
/// 值统一归一到 Overview；读取守卫和渲染都只消费这个闭集，不能各自猜一次。
const TABS: &[(TargetDrawerTab, &str)] = &[
    (TargetDrawerTab::Overview, "概览"),
    (TargetDrawerTab::Baseline, "基线"),
    (TargetDrawerTab::Patrol, "巡检策略"),
    (TargetDrawerTab::Trace, "追踪"),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetDrawerTab {
    Overview,
    Baseline,
    Patrol,
    Trace,
}

impl TargetDrawerTab {
    pub fn parse(value: Option<&str>) -> Self {
        match value {
            Some("baseline") => Self::Baseline,
            Some("patrol") => Self::Patrol,
            Some("trace") => Self::Trace,
            // `evidence` is retired and any future/unknown spelling has the same explicit
            // compatibility policy: open the safe default Overview and actually read it.
            None | Some("overview") | Some("evidence") | Some(_) => Self::Overview,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Overview => "overview",
            Self::Baseline => "baseline",
            Self::Patrol => "patrol",
            Self::Trace => "trace",
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
                   <div class="c-dw-kicker">目标工作区</div>
                   <div class="c-dw-title-row">
                     <div><div class="c-dw-title">未找到该观察目标</div>
                       <div class="c-dw-meta">#{drawer}</div></div>
                     <div class="c-dw-actions"><a class="c-btn-quiet" href="{return_href}">关闭 ×</a></div>
                   </div>
                 </div>
                 <div class="c-dw-body"><p class="c-dw-empty">这个标识没有对应的观察目标。它可能已被删除，或链接来自另一台机器的库。</p></div>
               </aside>"#,
            drawer = escape(drawer),
            return_url = list_context.list_href(None),
        );
    };

    let tab = active_tab;
    let archive = completeness.get(&target.identity_key);
    let is_creator = target.target_kind == "creator";
    let return_focus = format!("target-{}", target.target_ref);
    let return_href = list_context.list_href(Some(&return_focus));

    format!(
        r#"<aside id="c-drawer" class="c-dw" aria-label="观察目标工作区" data-return-url="{return_url}" data-return-focus="{return_focus}">
             <div class="c-dw-head">
               <div class="c-dw-kicker">目标工作区 / {kind}</div>
               <div class="c-dw-title-row">
                 <div>
                   <div class="c-dw-title">{name}</div>
                   <div class="c-dw-meta">{platform} · {handle}</div>
                 </div>
                 <div class="c-dw-actions">
                   {source_link}
                   <a class="c-btn-quiet" href="{return_href}">关闭 ×</a>
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
        return_url = list_context.list_href(None),
        source_link = source_link(target, is_creator),
        statusline = statusline(target),
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
        ("creator", "archived" | "monitoring" | "paused") => ("ok", "基线就绪"),
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
        "paused" => ("warn", "巡检已暂停"),
        "dismissed" => ("neutral", "不再调度"),
        _ if target.monitoring_enabled => ("info", "巡检中"),
        _ => ("neutral", "未开启巡检"),
    }
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
    let (_, archive) = lifecycle_primary_copy(&target.target_kind, &target.lifecycle_state);
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
        TargetDrawerTab::Baseline => baseline_tab(target, archive, is_creator),
        TargetDrawerTab::Patrol => patrol_tab(target),
        // 追踪背后暂时没有东西。**如实说尚未接通，而不是画一个空壳**——
        // 一个有标题、有格子、没有数的面板，会让人以为「查过了，是空的」。
        TargetDrawerTab::Trace => not_connected(
            "追踪",
            "语义事件流尚未实现。执行留痕目前只到工单与租约这一层，可在「执行工位」查看。",
        ),
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

/// 概览。稿子有粉丝／内容量／中位表现／峰值四格与两张图表。
///
/// **中位表现与峰值需要逐篇互动数据，系统里没有**；图表同理。因此只给有真实来源的那些，
/// 缺的整格写「未采集」——留一个空图表框比不画更糟，它看起来像「数据为零」。
fn overview_tab(
    target: &ObservationTarget,
    _archive: Option<&ArchiveCompleteness>,
    is_creator: bool,
    lifecycle: LifecycleView<'_>,
    selected_work: Option<&str>,
    list_context: TargetListContext<'_>,
) -> String {
    let facts = target.identity_facts.as_ref();
    let bio = facts
        .and_then(|value| value.get("description"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");

    format!(
        r#"{lifecycle}{bio_block}"#,
        lifecycle = lifecycle_overview(target, is_creator, lifecycle, selected_work, list_context,),
        bio_block = if bio.is_empty() {
            String::new()
        } else {
            format!(
                r#"<section class="c-dw-section"><div class="c-dw-section-head"><b>简介</b><span>平台公开资料</span></div><p class="c-dw-bio">{bio}</p></section>"#,
                bio = escape(bio),
            )
        },
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
            "生命周期查询无效 / QUERY_INVALID",
            "URL 中的时间窗或指标不在闭集合同内；本页没有回落到默认口径，也没有发起生命周期读取。",
        );
    }
    if !is_creator {
        return lifecycle_state(
            "不适用于关键词目标",
            "创作者生命周期按稳定作者身份组织作品；关键词观察面不能伪装成作者曲线。",
        );
    }
    let projection = match lifecycle {
        LifecycleView::Projection(projection) => projection,
        LifecycleView::ReadUnavailable { .. }
        | LifecycleView::NotRead { .. }
        | LifecycleView::QueryInvalid => {
            return lifecycle_state(
                "生命周期当前读不到",
                "这是读取状态未知，不表示创作者没有作品，也不表示表现为零。",
            );
        }
    };
    if projection.status == CreatorLifecycleStatus::NotApplicable {
        return lifecycle_state(
            "不适用于当前目标",
            "只有稳定 creator target 能生成创作者生命周期。",
        );
    }

    let controls = lifecycle_controls(target, projection, list_context);
    let (linked, linked_label) = match projection.summary.linked_work_count {
        Some(count) => (count.to_string(), "关联作品"),
        None => (
            format!("≥{}", projection.summary.linked_work_count_lower_bound),
            "关联作品下限",
        ),
    };
    let summary = format!(
        r#"<div class="life-summary" aria-label="生命周期覆盖摘要">
              <div><b>{linked}</b><span>{linked_label}</span></div>
              <div><b>{confirmed}</b><span>扫描内作者确认</span></div>
              <div><b>{eligible}</b><span>返回可绘制</span></div>
            </div>"#,
        confirmed = projection.summary.confirmed_author_work_count,
        eligible = projection.summary.eligible_point_count,
    );
    let chart = if projection.points.is_empty() {
        lifecycle_state(
            "观察不足，暂时无法成图",
            "作品必须同时具备稳定作者归属、合格真实发布时间和当前指标的 KNOWN 值。UNKNOWN 不会被当成 0。",
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
                <div><h2>创作者生命周期</h2><p>按作品真实发布时间，不是账号粉丝增长曲线</p></div>
                <p>截至 {as_of}</p>
              </div>
              {controls}{summary}{chart}
              <div class="life-receipt">
                <span>探测 {probed} · 扫描 {scanned}/{limit} · 返回 {returned}；{receipt_state}</span>
                <span class="life-version">{median_version} · {percentile_version}</span>
              </div>
              {exclusions}{selected}
            </section>"#,
        as_of = escape(&projection.as_of),
        probed = projection.receipt.probed_count,
        scanned = projection.receipt.scanned_count,
        limit = projection.receipt.scan_limit,
        returned = projection.receipt.returned_count,
        receipt_state = if projection.receipt.truncated {
            "已触及扫描上限，当前不是完整历史"
        } else {
            "本次有界扫描未截断"
        },
        median_version = escape(projection.analysis.rolling_median_version),
        percentile_version = escape(projection.analysis.percentile_version),
        exclusions = lifecycle_exclusions(projection),
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
        (CreatorLifecycleMetric::CompositeV1, "复合指标 v1"),
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
    // At the 390px drawer width, 36 viewBox units leave just over 16 CSS pixels between
    // the last point's centre and the SVG edge. That keeps the non-scaling 24px hit ring
    // inside the plot instead of clipping its right half.
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
    let median = projection
        .points
        .iter()
        .map(|point| format!("{:.1},{:.1}", x_for(point), y_for(point.rolling_median)))
        .collect::<Vec<_>>()
        .join(" ");
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
                r#"<a class="life-point{selected_class}" href="{href}" aria-label="{title}，{published}，{metric_label} {value}，创作者内分位 {percentile:.1}%"><circle class="life-point-hit" cx="{x:.1}" cy="{y:.1}" r="6" aria-hidden="true"/><circle class="life-point-visible" cx="{x:.1}" cy="{y:.1}" r="5" aria-hidden="true"/></a>"#,
                title = escape(title),
                published = escape(&point.published_local_date),
                metric_label = metric_label(projection.metric),
                value = point.metric_value,
                percentile = point.creator_percentile,
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
        CreatorLifecycleWindow::Recent90Days => "Asia/Shanghai 近 90 个日历日（含首尾）",
        CreatorLifecycleWindow::All => "Asia/Shanghai 全部合格历史",
    };
    format!(
        r#"<figure class="life-figure">
              <svg class="life-chart" viewBox="0 0 800 300" role="img" aria-labelledby="life-chart-title life-chart-desc">
                <title id="life-chart-title">创作者作品生命周期散点图</title>
                <desc id="life-chart-desc">横轴为合格真实发布时间，纵轴为 log(1 + 指标)。黑点是作品，橙线是服务端 trailing 5 work median。</desc>
                <line class="life-axis" x1="54" y1="260" x2="782" y2="260"/>
                <line class="life-axis" x1="54" y1="20" x2="54" y2="260"/>
                <polyline class="life-median" points="{median}"/>
                {points}
                <text class="life-axis-copy" x="54" y="284">{start}</text>
                <text class="life-axis-copy" x="782" y="284" text-anchor="end">{end}</text>
                <text class="life-axis-copy" x="14" y="142" transform="rotate(-90 14 142)" text-anchor="middle">log(1 + 指标)</text>
              </svg>
              <figcaption class="life-caption"><span>横轴：合格真实发布时间（{window_caption}）</span><span>纵轴为 log(1 + 指标)；数值摘要仍显示原值</span></figcaption>
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
        "<li>扫描已截断</li>"
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
                <div><dt>创作者内分位</dt><dd>{percentile:.1}%</dd></div>
              </dl>
              <a class="life-corpus-link" href="/corpus/evidence?work={work}">到语料页查看这篇作品 →</a>
            </section>"#,
        title = escape(point.title.as_deref().unwrap_or("标题未知")),
        published = escape(&point.published_local_date),
        metric = metric_label(metric),
        value = point.metric_value,
        percentile = point.creator_percentile,
        work = point.work_public_ref,
    )
}

fn lifecycle_state(title: &str, body: &str) -> String {
    format!(
        r#"<section class="c-dw-section life-panel"><div class="life-heading"><h2>创作者生命周期</h2></div><div class="life-state"><b>{title}</b><p>{body}</p></div></section>"#,
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
        CreatorLifecycleMetric::CompositeV1 => "复合指标 v1",
    }
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
            next_patrol_at: None,
        }
    }

    #[test]
    fn drawer_statusline_names_every_legal_lifecycle_state() {
        for (state, label) in [
            ("pending_decision", "尚未建档"),
            ("archiving", "建档中"),
            ("archived", "基线就绪"),
            ("monitoring", "基线就绪"),
            ("paused", "巡检已暂停"),
            ("dismissed", "已停止观察"),
        ] {
            let line = statusline(&target(state));
            assert!(line.contains(label), "{state} must read as {label}");
            assert!(!line.contains("状态未知"), "{state} is a legal state");
            for forbidden in ['▲', '●'] {
                assert!(
                    !line.contains(forbidden),
                    "lifecycle copy must use text and semantic styling, not {forbidden}"
                );
            }
        }
    }
}
