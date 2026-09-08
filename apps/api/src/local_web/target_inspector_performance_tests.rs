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

    assert!(html.contains("6 个观察目标"));
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
    assert!(TARGET_DRAWER_CSS.contains("min-width:1120px"));
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
fn performance_view_keeps_known_zero_excludes_unknown_and_preserves_context() {
    use super::target_drawer::{
        LifecycleView, TargetCatalogView, TargetDrawerTab, TargetInspectorView, TargetListContext,
        TargetWorksView,
    };

    let target = sample_target(10, "creator");
    let projection = lifecycle_projection(target.target_ref);
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
        TargetListContext {
            filter: Some("creator"),
            sort: Some("last"),
            domain: Some("adhd-family"),
        },
    );

    assert!(html.contains(r#"aria-label="作品视图""#));
    assert!(html.contains("wview=list"));
    assert!(html.contains("wview=performance"));
    assert!(html.contains("domain=adhd-family"));
    assert!(html.contains("filter=creator"));
    assert!(html.contains("sort=last"));
    assert!(html.contains("life_window=recent_90_days"));
    assert!(html.contains("life_metric=likes"));
    assert!(html.contains("零互动作品，2026-09-01，点赞 0"));
    assert_eq!(html.matches(r#"class="life-point-hit""#).count(), 1);
    assert!(html.contains("指标未知 1"));
    assert!(html.contains("尚未建立内容分类"));
    assert!(!html.contains("主题表现"));
    assert!(!html.contains("内容结构"));
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
    assert!(!TARGET_DRAWER_CSS.contains("gradient"));
    assert!(!TARGET_DRAWER_CSS.contains("outline:none"));
    assert!(!TARGET_DRAWER_CSS.contains("!important"));
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
            linked_work_count: Some(2),
            linked_work_count_lower_bound: 2,
            confirmed_author_work_count: 1,
            eligible_point_count: 1,
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
            probed_count: 2,
            scanned_count: 2,
            returned_count: 1,
            truncated: false,
        },
        points: vec![CreatorLifecyclePoint {
            work_public_ref: uuid::Uuid::from_u128(20_001),
            title: Some("零互动作品".to_owned()),
            title_state: "KNOWN",
            published_at: "2026-09-01 00:00:00+08".to_owned(),
            published_local_date: "2026-09-01".to_owned(),
            published_at_epoch_ms: 1_788_192_000_000,
            metric_value: 0,
            association_state: CreatorLifecycleAssociation::AuthorConfirmed,
            new_in_latest_patrol: false,
        }],
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
