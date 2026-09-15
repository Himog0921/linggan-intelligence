use super::{COLLECTION_WORKSPACE_JS, TARGET_DRAWER_CSS};
use linggan_evidence::{
    CreatorLifecycleAssociation, CreatorLifecycleExclusions, CreatorLifecycleMetric,
    CreatorLifecyclePoint, CreatorLifecycleProjection, CreatorLifecycleReceipt,
    CreatorLifecycleStatus, CreatorLifecycleSummary, CreatorLifecycleWindow, ObservationTarget,
    TargetInspectorAction, TargetInspectorArchive, TargetInspectorArchiveState,
    TargetInspectorCount, TargetInspectorCoverage, TargetInspectorDirectoryState,
    TargetInspectorExecution, TargetInspectorExecutionState, TargetInspectorPatrol,
    TargetInspectorPatrolState, TargetInspectorProjection,
};
use std::collections::HashMap;

#[test]
fn six_targets_keep_batch_selection_without_the_redundant_row_instruction() {
    let targets = (0..6)
        .map(|index| sample_target(index, if index < 3 { "creator" } else { "keyword" }))
        .collect::<Vec<_>>();
    let base = r#"<main><section class="c-empty">loading</section></main>"#;
    let html = super::collection_targets_view::render_stored_targets(
        base,
        &targets,
        &HashMap::new(),
        Some(&HashMap::new()),
        None,
        super::target_drawer::TargetListContext::default(),
    );

    // 顶部 tab 已经写着「全部来源 6」，表格上方再数一遍是同一件事说两次。
    assert!(!html.contains("个观察目标"));
    assert!(!html.contains("c-tg-list-head"));
    assert_eq!(html.matches(" data-target-select ").count(), 6);
    assert_eq!(html.matches(" data-target-select-all ").count(), 2);
    assert_eq!(html.matches(r#"form="target-batch-modal-form""#).count(), 6);
    assert!(html.contains(r#"id="target-batch-modal-form""#));
    assert!(html.contains("批量编辑观察目标"));
    assert!(!html.contains("勾选后可在顶部批量编辑"));
    assert!(!html.contains("点击一行查看详情"));

    // Selection and navigation remain separate: checking a target only changes the batch
    // command state, while the target name remains the explicit drawer link.
    assert!(COLLECTION_WORKSPACE_JS.contains("function selectedTargetCount()"));
    assert!(COLLECTION_WORKSPACE_JS.contains("targetBatchOpen.disabled = selected === 0"));
    let toolbar = super::collection::render(
        super::collection::Section::Targets,
        super::collection::OperationsMode::Now,
        None,
        None,
        None,
    );
    assert!(toolbar.contains("data-target-batch-open"));
    assert!(toolbar.contains("data-target-selected-count hidden>0"));
    for redundant_runtime_copy in [
        "巡检已开",
        "当前观察目标投影",
        "调度运行中",
        "SCHEDULER RUNNING",
        "PATROL ARMED",
        "中国标准时间",
        "UTC+08",
    ] {
        assert!(
            !toolbar.contains(redundant_runtime_copy),
            "targets header repeated runtime copy: {redundant_runtime_copy}"
        );
    }
    assert!(COLLECTION_WORKSPACE_JS.contains("targetSelectedCount.textContent = String(selected)"));
    assert!(COLLECTION_WORKSPACE_JS.contains("targetSelectedCount.hidden = selected === 0"));
    assert!(
        COLLECTION_WORKSPACE_JS
            .contains("targetBatchCount.textContent = \"已选择 \" + selected + \" 个目标\"")
    );
    assert!(COLLECTION_WORKSPACE_JS.contains("a,button,input,select,textarea,label,summary"));
    // 此前这里要求表格保持 1120px 最小宽度、窄窗口交给横向滚动。改为一屏排满：
    // 需要左右拖动才能看全的目录，等于要求人记住左边看过什么。
    assert!(!TARGET_DRAWER_CSS.contains("min-width:1120px"));
    assert!(!TARGET_DRAWER_CSS.contains("calc(var(--lgi-space-24) * 57)"));
    assert!(!TARGET_DRAWER_CSS.contains("position:sticky;right:0"));
    assert!(!TARGET_DRAWER_CSS.contains("border-left:5px solid var(--lgi-signal)"));
    assert!(super::COLLECTION_WORKSPACE_CSS.contains(".c-tg-field[hidden] { display:none; }"));
}

#[test]
fn works_view_is_url_restorable_and_legacy_lifecycle_links_open_performance() {
    use super::target_drawer::TargetWorksView;

    assert_eq!(
        TargetWorksView::parse(Some("list"), false),
        TargetWorksView::List
    );
    assert_eq!(
        TargetWorksView::parse(Some("performance"), false),
        TargetWorksView::Performance
    );
    assert_eq!(
        TargetWorksView::parse(None, true),
        TargetWorksView::Performance
    );
    assert_eq!(TargetWorksView::parse(None, false), TargetWorksView::List);
    assert_eq!(
        TargetWorksView::parse(Some("unsupported"), false),
        TargetWorksView::List
    );
}

#[test]
fn blocked_materials_get_a_real_decision_instead_of_a_dead_end() {
    use super::target_drawer::{
        LifecycleView, TargetCatalogView, TargetDrawerTab, TargetInspectorView, TargetListContext,
        TargetWorksView,
    };
    use linggan_evidence::BlockedMaterial;

    let target = sample_target(31, "creator");
    let projection = inspector_projection(
        &target,
        TargetInspectorExecutionState::Idle,
        TargetInspectorAction::HandleArchiveProblems,
        known_coverage(13, 10),
    );
    let retirable = [BlockedMaterial {
        content_public_ref: uuid::Uuid::from_u128(77),
        content_external_id: "gone-work-one".to_owned(),
        title: Some("这篇已经被作者删了".to_owned()),
    }];
    let render = |retirable: &[BlockedMaterial]| {
        super::target_drawer::render_with_catalog_view(
            Some(&target),
            None,
            Some(&HashMap::new()),
            Some(&target.target_ref.to_string()),
            TargetDrawerTab::Overview,
            LifecycleView::NotRead {
                window: linggan_evidence::CreatorLifecycleWindow::Recent90Days,
                metric: linggan_evidence::CreatorLifecycleMetric::Likes,
            },
            TargetInspectorView::Projection(&projection),
            TargetWorksView::List,
            TargetCatalogView::Unavailable,
            None,
            None,
            None,
            None,
            retirable,
            TargetListContext::default(),
        )
    };

    // 有待判断的作品时，「处理异常」不再只是一句陈述：它给出可以做的那个决定，
    // 并把作品的标题与平台 id 一起摆出来——人是照着这些去平台上核对的。
    let html = render(&retirable);
    assert!(html.contains(r#"action="/collection/targets/retire-materials""#));
    assert!(html.contains("确认这些作品已失效"));
    assert!(html.contains("这篇已经被作者删了"));
    assert!(html.contains("gone-work-one"));
    assert!(html.contains(r#"value="00000000-0000-0000-0000-00000000004d""#));

    // 没有待判断的对象就不摆按钮：请人对空气做一个决定，不是一个动作。
    let empty = render(&[]);
    assert!(!empty.contains("确认这些作品已失效"));
    assert!(!empty.contains(r#"action="/collection/targets/retire-materials""#));
}

#[test]
fn performance_view_keeps_known_zero_excludes_unknown_and_preserves_context() {
    use super::target_drawer::{
        KeywordArchiveRead, LifeChartView, LifeTrendGrain, LifecycleView, TargetCatalogView,
        TargetDrawerTab, TargetInspectorView, TargetListContext, TargetWorksView,
    };

    let target = sample_target(10, "creator");
    let projection = lifecycle_projection(target.target_ref);
    let render = |chart_view| {
        super::target_drawer::render_with_catalog_view_with_chart(
            Some(&target),
            None,
            Some(&HashMap::new()),
            Some(&target.target_ref.to_string()),
            TargetDrawerTab::Baseline,
            LifecycleView::Projection(&projection),
            TargetInspectorView::NotRead,
            TargetWorksView::Performance,
            chart_view,
            LifeTrendGrain::Month,
            TargetCatalogView::Unavailable,
            None,
            None,
            None,
            None,
            &[],
            KeywordArchiveRead::Unavailable,
            TargetListContext {
                filter: Some("creator"),
                sort: Some("last"),
                domain: Some("adhd-family"),
            },
        )
    };
    let html = render(LifeChartView::Trend);

    assert!(html.contains(r#"aria-label="作品视图""#));
    assert!(html.contains("wview=list"));
    assert!(html.contains("wview=performance"));
    assert!(html.contains("domain=adhd-family"));
    assert!(html.contains("filter=creator"));
    assert!(html.contains("sort=last"));
    assert!(html.contains("life_window=recent_90_days"));
    assert!(html.contains("life_metric=likes"));
    assert!(html.contains("life_chart=trend"));
    assert!(html.contains("life_chart=distribution"));
    assert!(html.contains("life_grain=month"));
    assert!(html.contains(r#"class="life-trend-chart""#));
    assert!(html.contains(r#"id="life-trend-area-gradient""#));
    assert_eq!(html.matches("<linearGradient").count(), 1);
    assert_eq!(html.matches("life-distribution-chart").count(), 0);
    assert!(html.contains(r#"aria-label="图表视图""#));
    assert!(html.contains(r#"aria-label="趋势粒度""#));
    assert!(html.contains(r#"aria-label="复核摘要""#));
    assert!(html.contains("近期作品证据"));
    let trend_position = html.find("作品表现趋势").expect("trend heading");
    let evidence_position = html.find("近期作品证据").expect("evidence heading");
    assert!(trend_position < evidence_position);
    assert!(!html.contains(r#"class="life-point-hit""#));

    let distribution = render(LifeChartView::Distribution);
    assert!(distribution.contains("作品表现分布"));
    assert!(distribution.contains("life-distribution-chart"));
    assert_eq!(distribution.matches("<linearGradient").count(), 0);
    assert!(distribution.contains("纵轴保持当前点赞原始数值"));
    assert!(!distribution.contains("纵轴压缩"));
    assert!(distribution.contains("零互动作品，2026-09-01，点赞 0"));
    assert_eq!(distribution.matches(r#"class="life-point-hit""#).count(), 3);
    assert_eq!(
        distribution
            .matches("life-point-high-discussion-ring")
            .count(),
        1
    );
    assert!(distribution.contains("高讨论率"));
    assert!(distribution.contains("讨论率未可判"));
    assert!(html.contains("指标未知 1"));
    assert!(html.contains("尚未建立内容分类"));
    assert!(!html.contains("主题表现"));
    assert!(!html.contains("内容结构"));
}

#[test]
fn performance_view_keeps_unknown_out_of_the_review_when_no_points_qualify() {
    use super::target_drawer::{
        LifecycleView, TargetCatalogView, TargetDrawerTab, TargetInspectorView, TargetListContext,
        TargetWorksView,
    };

    let target = sample_target(11, "creator");
    let mut projection = lifecycle_projection(target.target_ref);
    projection.points.clear();
    projection.summary.eligible_point_count = 0;
    projection.receipt.returned_count = 0;
    let html = super::target_drawer::render_with_catalog_view(
        Some(&target),
        None,
        Some(&HashMap::new()),
        Some(&target.target_ref.to_string()),
        TargetDrawerTab::Baseline,
        LifecycleView::Projection(&projection),
        TargetInspectorView::NotRead,
        TargetWorksView::Performance,
        TargetCatalogView::Unavailable,
        None,
        None,
        None,
        None,
        &[],
        TargetListContext::default(),
    );

    assert!(html.contains("观察不足，暂时无法成图"));
    assert!(html.contains("指标未知 1"));
    assert!(html.contains("life-performance-grid-empty"));
    assert!(!html.contains(r#"aria-label="复核摘要""#));
    assert!(!html.contains("当前窗口中位点赞"));
    assert!(!html.contains("发布密度"));
}

#[test]
fn drawer_observation_badge_does_not_collapse_stopped_or_unstarted_into_paused() {
    use super::target_drawer::{
        LifecycleView, TargetCatalogView, TargetDrawerTab, TargetInspectorView, TargetListContext,
        TargetWorksView,
    };

    let render = |lifecycle_state: &str, monitoring_enabled: bool| {
        let mut target = sample_target(12, "creator");
        target.lifecycle_state = lifecycle_state.to_owned();
        target.monitoring_enabled = monitoring_enabled;
        super::target_drawer::render_with_catalog_view(
            Some(&target),
            None,
            Some(&HashMap::new()),
            Some(&target.target_ref.to_string()),
            TargetDrawerTab::Overview,
            LifecycleView::NotRead {
                window: CreatorLifecycleWindow::Recent90Days,
                metric: CreatorLifecycleMetric::Likes,
            },
            TargetInspectorView::NotRead,
            TargetWorksView::List,
            TargetCatalogView::Unavailable,
            None,
            None,
            None,
            None,
            &[],
            TargetListContext::default(),
        )
    };

    let stopped = render("dismissed", false);
    assert!(stopped.contains(r#"data-state="stopped""#));
    assert!(stopped.contains("已停止观察"));
    assert!(!stopped.contains(r#"data-state="paused""#));

    let paused = render("paused", false);
    assert!(paused.contains(r#"data-state="paused""#));
    assert!(paused.contains("已暂停"));

    let unstarted = render("pending_decision", false);
    assert!(unstarted.contains(r#"data-state="inactive""#));
    assert!(unstarted.contains("未开启观察"));
}

#[test]
fn overview_orders_identity_system_facts_and_decision_as_four_distinct_layers() {
    use super::target_drawer::{
        LifecycleView, TargetCatalogView, TargetDrawerTab, TargetInspectorView, TargetListContext,
        TargetWorksView,
    };

    let target = sample_target(20, "creator");
    let inspector = inspector_projection(
        &target,
        TargetInspectorExecutionState::Queued,
        TargetInspectorAction::NoActionQueued,
        known_coverage(42, 41),
    );
    let html = super::target_drawer::render_with_catalog_view(
        Some(&target),
        None,
        Some(&HashMap::new()),
        Some(&target.target_ref.to_string()),
        TargetDrawerTab::Overview,
        LifecycleView::NotRead {
            window: CreatorLifecycleWindow::Recent90Days,
            metric: CreatorLifecycleMetric::Likes,
        },
        TargetInspectorView::Projection(&inspector),
        TargetWorksView::List,
        TargetCatalogView::Unavailable,
        None,
        None,
        None,
        None,
        &[],
        TargetListContext::default(),
    );

    let identity_at = html.find("合成目标 20").expect("identity is rendered");
    let system_at = html
        .find("系统现在在做什么")
        .expect("system state layer is rendered");
    let facts_at = html
        .find("已经取得什么")
        .expect("acquired facts layer is rendered");
    let decision_at = html
        .find("是否需要处理")
        .expect("required-action layer is rendered");
    assert!(identity_at < system_at && system_at < facts_at && facts_at < decision_at);
    assert!(html.contains("42"));
    assert!(html.contains("41"));
    assert!(html.contains("当前无需处理"));
    assert!(!html.contains(r#"action="/collection/targets/archive""#));
}

#[test]
fn queued_and_running_are_not_the_same_execution_claim() {
    use super::target_drawer::{
        LifecycleView, TargetCatalogView, TargetDrawerTab, TargetInspectorView, TargetListContext,
        TargetWorksView,
    };

    let target = sample_target(21, "creator");
    let render = |inspector: &TargetInspectorProjection| {
        super::target_drawer::render_with_catalog_view(
            Some(&target),
            None,
            Some(&HashMap::new()),
            Some(&target.target_ref.to_string()),
            TargetDrawerTab::Overview,
            LifecycleView::NotRead {
                window: CreatorLifecycleWindow::Recent90Days,
                metric: CreatorLifecycleMetric::Likes,
            },
            TargetInspectorView::Projection(inspector),
            TargetWorksView::List,
            TargetCatalogView::Unavailable,
            None,
            None,
            None,
            None,
            &[],
            TargetListContext::default(),
        )
    };
    let queued = inspector_projection(
        &target,
        TargetInspectorExecutionState::Queued,
        TargetInspectorAction::NoActionQueued,
        known_coverage(1, 0),
    );
    let running = inspector_projection(
        &target,
        TargetInspectorExecutionState::Running,
        TargetInspectorAction::NoActionRunning,
        known_coverage(1, 0),
    );

    let queued_html = render(&queued);
    let running_html = render(&running);
    assert!(queued_html.contains("等待调度"));
    assert!(!queued_html.contains("正在执行"));
    assert!(running_html.contains("正在执行"));
    assert!(!running_html.contains("等待调度"));
    assert!(queued_html.contains("当前无需处理"));
    assert!(running_html.contains("当前无需处理"));
}

#[test]
fn unknown_coverage_never_falls_back_to_zero() {
    use super::target_drawer::{
        LifecycleView, TargetCatalogView, TargetDrawerTab, TargetInspectorView, TargetListContext,
        TargetWorksView,
    };

    let target = sample_target(22, "creator");
    let inspector = inspector_projection(
        &target,
        TargetInspectorExecutionState::Idle,
        TargetInspectorAction::NoActionHealthy,
        TargetInspectorCoverage {
            directory_works: TargetInspectorCount::Unknown,
            captured_details: TargetInspectorCount::Unknown,
            missing_details: TargetInspectorCount::Unknown,
            quarantined_records: TargetInspectorCount::Unknown,
            blocked_details: TargetInspectorCount::Unknown,
        },
    );
    let html = super::target_drawer::render_with_catalog_view(
        Some(&target),
        None,
        None,
        Some(&target.target_ref.to_string()),
        TargetDrawerTab::Overview,
        LifecycleView::NotRead {
            window: CreatorLifecycleWindow::Recent90Days,
            metric: CreatorLifecycleMetric::Likes,
        },
        TargetInspectorView::Projection(&inspector),
        TargetWorksView::List,
        TargetCatalogView::Unavailable,
        None,
        None,
        None,
        None,
        &[],
        TargetListContext::default(),
    );

    assert!(html.contains("<b>尚未取得</b><span>作品目录</span>"));
    assert!(html.contains("<b>尚未取得</b><span>详情进度</span>"));
    assert!(html.contains("<b>尚未取得</b><span>待取得详情</span>"));
    assert!(!html.contains("0 / 0"));
}

#[test]
fn unavailable_inspector_does_not_fall_back_to_a_healthy_or_empty_state() {
    use super::target_drawer::{
        LifecycleView, TargetCatalogView, TargetDrawerTab, TargetInspectorView, TargetListContext,
        TargetWorksView,
    };

    let target = sample_target(23, "creator");
    let html = super::target_drawer::render_with_catalog_view(
        Some(&target),
        None,
        Some(&HashMap::new()),
        Some(&target.target_ref.to_string()),
        TargetDrawerTab::Overview,
        LifecycleView::NotRead {
            window: CreatorLifecycleWindow::Recent90Days,
            metric: CreatorLifecycleMetric::Likes,
        },
        TargetInspectorView::ReadUnavailable,
        TargetWorksView::List,
        TargetCatalogView::Unavailable,
        None,
        None,
        None,
        None,
        &[],
        TargetListContext::default(),
    );

    assert!(html.contains("目标状态暂时读不到"));
    assert!(html.contains("当前无法判断"));
    assert!(!html.contains("当前无需处理"));
    assert!(!html.contains("无已知阻塞"));
    assert!(!html.contains(r#"action="/collection/targets/archive""#));
}

#[test]
fn inspector_css_keeps_desktop_controls_and_accessible_motion_boundaries() {
    for required in [
        "min-height:40px",
        "@media (prefers-reduced-motion:reduce)",
        ".life-point:focus-visible .life-point-visible",
        "stroke:var(--lgi-focus)",
        ".c-dw-view-tab",
    ] {
        assert!(
            TARGET_DRAWER_CSS.contains(required),
            "target inspector CSS is missing {required}"
        );
    }
    assert!(TARGET_DRAWER_CSS.contains(".life-chart-switch"));
    assert!(TARGET_DRAWER_CSS.contains(".life-grain-switch"));
    assert!(TARGET_DRAWER_CSS.contains("fill:url(#life-trend-area-gradient)"));
    assert!(TARGET_DRAWER_CSS.contains("stroke:var(--lgi-ink)"));
    assert!(
        TARGET_DRAWER_CSS
            .contains(".life-trend-line{fill:none;stroke:var(--lgi-ink);stroke-width:2;")
    );
    assert!(TARGET_DRAWER_CSS.contains(
        ".life-trend-new-ring{fill:var(--lgi-canvas);stroke:var(--lgi-signal);stroke-width:2}"
    ));
    assert!(
        !TARGET_DRAWER_CSS
            .contains(".life-trend-line{fill:none;stroke:var(--lgi-ink);stroke-width:2.15")
    );
    assert!(!TARGET_DRAWER_CSS.contains(
        ".life-trend-new-ring{fill:var(--lgi-canvas);stroke:var(--lgi-signal);stroke-width:1.5}"
    ));
    assert!(!TARGET_DRAWER_CSS.contains("life-trend-line-gradient"));
    assert!(!TARGET_DRAWER_CSS.contains("outline:none"));
    assert!(!TARGET_DRAWER_CSS.contains("!important"));

    let review_css = TARGET_DRAWER_CSS
        .split("TARGET-INSPECTOR-REVIEW-001")
        .nth(1)
        .expect("performance-review scope is present");
    for forbidden in [
        "font-size:9px",
        "font-size:10px",
        "font-size:12px",
        "font:650",
        "life-strip-height",
        "margin-bottom:12px",
    ] {
        assert!(
            !review_css.contains(forbidden),
            "performance-review CSS reintroduced a non-token visual value: {forbidden}"
        );
    }
    assert!(review_css.contains("font-size:var(--lgi-text-label)"));
    assert!(review_css.contains(".life-performance-grid-empty"));
    assert!(
        review_css.contains(".life-trend-legend{display:flex;align-items:center;flex-wrap:wrap")
    );
}

fn sample_target(index: u128, target_kind: &str) -> ObservationTarget {
    ObservationTarget {
        target_ref: uuid::Uuid::from_u128(10_000 + index),
        platform: "xhs".to_owned(),
        target_kind: target_kind.to_owned(),
        identity_key: format!("target-{index}"),
        display_name: Some(format!("合成目标 {index}")),
        identity_facts: None,
        source: "synthetic_test".to_owned(),
        lifecycle_state: "monitoring".to_owned(),
        first_stored_at: "2026-09-01 00:00:00+08".to_owned(),
        monitoring_enabled: true,
        group_name: None,
        last_patrol_dispatched_at: None,
        last_patrol_succeeded_at: None,
        next_patrol_at: None,
        domain_name: Some("ADHD 家庭".to_owned()),
        domain_is_own: Some(true),
    }
}

fn lifecycle_projection(target_ref: uuid::Uuid) -> CreatorLifecycleProjection {
    CreatorLifecycleProjection {
        target_ref,
        target_kind: "creator".to_owned(),
        status: CreatorLifecycleStatus::Ready,
        as_of: "2026-09-08 12:00:00+08".to_owned(),
        window: CreatorLifecycleWindow::Recent90Days,
        metric: CreatorLifecycleMetric::Likes,
        summary: CreatorLifecycleSummary {
            linked_work_count: Some(4),
            linked_work_count_lower_bound: 4,
            confirmed_author_work_count: 3,
            eligible_point_count: 3,
        },
        exclusions: CreatorLifecycleExclusions {
            author_not_verified: 0,
            author_mismatch: 0,
            published_at_not_qualified: 0,
            outside_window: 0,
            metric_unknown: 1,
            scan_truncated: false,
        },
        receipt: CreatorLifecycleReceipt {
            scan_limit: 2_000,
            probed_count: 4,
            scanned_count: 4,
            returned_count: 3,
            truncated: false,
        },
        points: vec![
            CreatorLifecyclePoint {
                work_public_ref: uuid::Uuid::from_u128(20_001),
                title: Some("零互动作品".to_owned()),
                title_state: "KNOWN",
                published_at: "2026-09-01 00:00:00+08".to_owned(),
                published_local_date: "2026-09-01".to_owned(),
                published_at_epoch_ms: 1_788_192_000_000,
                metric_value: 0,
                discussion_rate: None,
                association_state: CreatorLifecycleAssociation::AuthorConfirmed,
                new_in_latest_patrol: false,
            },
            CreatorLifecyclePoint {
                work_public_ref: uuid::Uuid::from_u128(20_002),
                title: Some("高讨论作品".to_owned()),
                title_state: "KNOWN",
                published_at: "2026-09-03 00:00:00+08".to_owned(),
                published_local_date: "2026-09-03".to_owned(),
                published_at_epoch_ms: 1_788_364_800_000,
                metric_value: 20,
                discussion_rate: Some(0.25),
                association_state: CreatorLifecycleAssociation::AuthorConfirmed,
                new_in_latest_patrol: true,
            },
            CreatorLifecyclePoint {
                work_public_ref: uuid::Uuid::from_u128(20_003),
                title: Some("常规作品".to_owned()),
                title_state: "KNOWN",
                published_at: "2026-09-04 00:00:00+08".to_owned(),
                published_local_date: "2026-09-04".to_owned(),
                published_at_epoch_ms: 1_788_451_200_000,
                metric_value: 8,
                discussion_rate: Some(0.10),
                association_state: CreatorLifecycleAssociation::DirectoryLinked,
                new_in_latest_patrol: false,
            },
        ],
    }
}

fn known_coverage(directory: i64, details: i64) -> TargetInspectorCoverage {
    TargetInspectorCoverage {
        directory_works: TargetInspectorCount::Known(directory),
        captured_details: TargetInspectorCount::Known(details),
        missing_details: TargetInspectorCount::Known(directory.saturating_sub(details)),
        quarantined_records: TargetInspectorCount::Known(0),
        blocked_details: TargetInspectorCount::Known(0),
    }
}

fn inspector_projection(
    target: &ObservationTarget,
    execution_state: TargetInspectorExecutionState,
    action: TargetInspectorAction,
    coverage: TargetInspectorCoverage,
) -> TargetInspectorProjection {
    let (queued_work_orders, awaiting_producer_tasks, running_attempts) = match execution_state {
        TargetInspectorExecutionState::Queued => (1, 0, 0),
        TargetInspectorExecutionState::AwaitingProducer => (0, 1, 0),
        TargetInspectorExecutionState::Running => (0, 0, 1),
        TargetInspectorExecutionState::Idle | TargetInspectorExecutionState::Blocked => (0, 0, 0),
    };
    TargetInspectorProjection {
        target_ref: target.target_ref,
        target_kind: target.target_kind.clone(),
        as_of: "2026-09-08 12:00:00+08".to_owned(),
        archive: TargetInspectorArchive {
            state: match execution_state {
                TargetInspectorExecutionState::Queued
                | TargetInspectorExecutionState::AwaitingProducer => {
                    TargetInspectorArchiveState::Queued
                }
                TargetInspectorExecutionState::Running => TargetInspectorArchiveState::Running,
                TargetInspectorExecutionState::Idle | TargetInspectorExecutionState::Blocked => {
                    TargetInspectorArchiveState::Complete
                }
            },
            directory_state: TargetInspectorDirectoryState::Ready,
            started: true,
            attempted: execution_state == TargetInspectorExecutionState::Running,
            author_profile_captures: TargetInspectorCount::Known(1),
        },
        execution: TargetInspectorExecution {
            state: execution_state,
            queued_work_orders,
            awaiting_producer_tasks,
            running_attempts,
            blocked_tasks: 0,
        },
        patrol: TargetInspectorPatrol {
            state: TargetInspectorPatrolState::Normal,
            last_dispatched_at: Some("2026-09-08 11:30:00+08".to_owned()),
            last_succeeded_at: Some("2026-09-08 11:45:00+08".to_owned()),
            next_run_at: Some("2026-09-09 11:45:00+08".to_owned()),
            latest_hits: TargetInspectorCount::Known(0),
            latest_new: TargetInspectorCount::Known(0),
        },
        coverage,
        required_action: action,
    }
}
