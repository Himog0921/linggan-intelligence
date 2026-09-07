use super::*;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use linggan_storage_postgres::testing::isolated_proof_schema;
use tower::ServiceExt;

#[tokio::test]
async fn lifecycle_api_keeps_a_closed_query_contract() {
    let invalid_enum = app()
        .oneshot(
            Request::builder()
                .uri("/api/local/collection/targets/00000000-0000-0000-0000-000000000001/lifecycle?window=last_2160_hours")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_enum.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let unused_selection = app()
        .oneshot(
            Request::builder()
                .uri("/api/local/collection/targets/00000000-0000-0000-0000-000000000001/lifecycle?selected_work=00000000-0000-0000-0000-000000000002")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unused_selection.status(), StatusCode::BAD_REQUEST);

    let not_connected = app()
        .oneshot(
            Request::builder()
                .uri("/api/local/collection/targets/00000000-0000-0000-0000-000000000001/lifecycle?window=all&metric=likes")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(not_connected.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn lifecycle_api_returns_a_minimal_target_projection() {
    let database = proof_database("creator_lifecycle_api").await;
    let target_ref = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_observation_target \
         (target_ref,platform,target_kind,identity_key,display_name,source) \
         VALUES ($1,'xhs','keyword','ADHD','ADHD','manual')",
    )
    .bind(target_ref)
    .execute(database.pool())
    .await
    .unwrap();

    let response = app_with_database(database.clone())
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/local/collection/targets/{target_ref}/lifecycle?window=all&metric=comments"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        payload.pointer("/status").and_then(Value::as_str),
        Some("NOT_APPLICABLE")
    );
    assert!(payload.pointer("/analysis").is_none());
    for forbidden in [
        "bodyText",
        "commentId",
        "commentExternalId",
        "media",
        "evidence",
        "monitoringValue",
        "opportunityScore",
        "selectedWork",
        "title",
        "publishedAt",
        "publishedLocalDate",
        "publishedAtEpochMs",
        "metricValue",
        "authorExternalId",
    ] {
        assert_no_json_key(&payload, forbidden);
    }

    let creator_ref = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_observation_target \
         (target_ref,platform,target_kind,identity_key,display_name,source) \
         VALUES ($1,'xhs','creator','creator-tab-policy','Creator tab policy','manual')",
    )
    .bind(creator_ref)
    .execute(database.pool())
    .await
    .unwrap();
    for normalized_tab in ["evidence", "future-tab"] {
        let response = app_with_database(database.clone())
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/collection/targets?drawer={creator_ref}&dtab={normalized_tab}"
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let html = String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap();
        assert!(html.contains("观察不足，暂时无法成图"));
        assert!(!html.contains("生命周期当前读不到"));
        assert!(html.contains(r#"class="c-dw-tab c-dw-tab-on" href="#));
    }

    let invalid_html = app_with_database(database.clone())
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/collection/targets?drawer={creator_ref}&life_window=last_2160_hours"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_html.status(), StatusCode::OK);
    let invalid_html = String::from_utf8(
        to_bytes(invalid_html.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(invalid_html.contains("作品分布的查询条件无效"));
    assert!(!invalid_html.contains("QUERY_INVALID"));
    assert!(!invalid_html.contains("aria-current=\"true\""));

    let missing = app_with_database(database)
        .oneshot(
            Request::builder()
                .uri("/api/local/collection/targets/00000000-0000-0000-0000-000000000003/lifecycle")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
}

async fn proof_database(schema: &str) -> Database {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").expect("proof URL is supplied");
    isolated_proof_schema(&url, schema, full_schema_fixture::FULL_MIGRATIONS)
        .await
        .expect("migrations apply")
}

fn assert_no_json_key(value: &Value, forbidden: &str) {
    match value {
        Value::Object(object) => {
            assert!(
                !object.contains_key(forbidden),
                "lifecycle response must not copy Corpus facts or invent value fields: {forbidden}"
            );
            object
                .values()
                .for_each(|child| assert_no_json_key(child, forbidden));
        }
        Value::Array(values) => values
            .iter()
            .for_each(|child| assert_no_json_key(child, forbidden)),
        _ => {}
    }
}

#[test]
fn creator_drawer_uses_three_business_tabs_and_two_level_work_association() {
    use linggan_evidence::{
        CreatorLifecycleAssociation, CreatorLifecycleExclusions, CreatorLifecycleMetric,
        CreatorLifecyclePoint, CreatorLifecycleProjection, CreatorLifecycleReceipt,
        CreatorLifecycleStatus, CreatorLifecycleSummary, CreatorLifecycleWindow, ObservationTarget,
        ObservationTargetAvatar,
    };
    let mut target = ObservationTarget {
        target_ref: uuid::Uuid::from_u128(11),
        platform: "xhs".to_owned(),
        target_kind: "creator".to_owned(),
        identity_key: "creator-stable-id".to_owned(),
        display_name: Some("生命周期作者".to_owned()),
        identity_facts: None,
        source: "manual".to_owned(),
        lifecycle_state: "monitoring".to_owned(),
        first_stored_at: "2026-08-01 00:00:00+00".to_owned(),
        monitoring_enabled: true,
        group_name: None,
        last_patrol_dispatched_at: Some("2026-09-04 15:00:00+08".to_owned()),
        last_patrol_succeeded_at: Some("2026-09-04 15:30:00+08".to_owned()),
        next_patrol_at: None,
    };
    target.identity_facts = Some(serde_json::json!({
        "redId": "creator-red-id",
        "description": "记录神经多样性的真实创作日常"
    }));
    let avatar = ObservationTargetAvatar::Local {
        local_asset_path: "/api/local/media/11111111-1111-4111-8111-111111111111/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
    };
    let selected_ref = uuid::Uuid::from_u128(101);
    let projection = CreatorLifecycleProjection {
        target_ref: target.target_ref,
        target_kind: "creator".to_owned(),
        status: CreatorLifecycleStatus::Ready,
        as_of: "2026-09-04 15:59:59+00".to_owned(),
        window: CreatorLifecycleWindow::Recent90Days,
        metric: CreatorLifecycleMetric::Likes,
        summary: CreatorLifecycleSummary {
            linked_work_count: Some(3),
            linked_work_count_lower_bound: 3,
            confirmed_author_work_count: 2,
            eligible_point_count: 2,
        },
        exclusions: CreatorLifecycleExclusions {
            author_not_verified: 1,
            author_mismatch: 0,
            published_at_not_qualified: 0,
            outside_window: 0,
            metric_unknown: 0,
            scan_truncated: false,
        },
        receipt: CreatorLifecycleReceipt {
            scan_limit: 2_000,
            probed_count: 3,
            scanned_count: 3,
            returned_count: 2,
            truncated: false,
        },
        points: vec![
            CreatorLifecyclePoint {
                work_public_ref: uuid::Uuid::from_u128(100),
                title: Some("第一篇".to_owned()),
                title_state: "KNOWN",
                published_at: "2026-08-01 00:00:00+00".to_owned(),
                published_local_date: "2026-08-01".to_owned(),
                published_at_epoch_ms: 1_785_542_400_000,
                metric_value: 10,
                association_state: CreatorLifecycleAssociation::DirectoryLinked,
                new_in_latest_patrol: true,
            },
            CreatorLifecyclePoint {
                work_public_ref: selected_ref,
                title: Some("第二篇".to_owned()),
                title_state: "KNOWN",
                published_at: "2026-08-02 00:00:00+00".to_owned(),
                published_local_date: "2026-08-02".to_owned(),
                published_at_epoch_ms: 1_785_628_800_000,
                metric_value: 100,
                association_state: CreatorLifecycleAssociation::AuthorConfirmed,
                new_in_latest_patrol: false,
            },
        ],
    };
    let api_payload = serde_json::to_value(creator_lifecycle_api::api_projection(&projection))
        .expect("the public lifecycle DTO serializes");
    assert_eq!(
        api_payload
            .pointer("/points/1/workPublicRef")
            .and_then(Value::as_str),
        Some(selected_ref.to_string().as_str())
    );
    assert_eq!(
        api_payload
            .pointer("/points/0/associationState")
            .and_then(Value::as_str),
        Some("DIRECTORY_LINKED")
    );
    assert_eq!(
        api_payload
            .pointer("/points/0/newInLatestPatrol")
            .and_then(Value::as_bool),
        Some(true)
    );
    for work_fact in [
        "title",
        "publishedAt",
        "publishedLocalDate",
        "publishedAtEpochMs",
        "metricValue",
        "authorExternalId",
    ] {
        assert_no_json_key(&api_payload, work_fact);
    }
    let html = target_drawer::render(
        Some(&target),
        Some(&avatar),
        Some(&std::collections::HashMap::new()),
        Some(&target.target_ref.to_string()),
        target_drawer::TargetDrawerTab::Overview,
        target_drawer::LifecycleView::Projection(&projection),
        Some(&selected_ref.to_string()),
        target_drawer::TargetListContext::default(),
    );

    for tab in ["概览", "档案", "巡查"] {
        assert!(html.contains(tab));
    }
    assert_eq!(html.matches(r#"<a class="c-dw-tab"#).count(), 3);
    assert_eq!(html.matches(r#"<a class="c-dw-tab c-dw-tab-on"#).count(), 1);
    assert!(!html.contains("追踪"));
    assert!(!html.contains("巡检策略"));
    assert!(!html.contains("dtab=evidence"));
    assert!(!html.contains(">证据</a>"));
    assert!(html.contains("作品生命周期"));
    assert!(html.contains(r#"id="c-drawer-title""#));
    assert!(html.contains("data-drawer-initial-focus"));
    assert!(html.contains(r#"class="c-dw-avatar""#));
    assert!(html.contains("creator-red-id"));
    assert!(!html.contains("记录神经多样性的真实创作日常"));
    assert!(html.contains("作品目录"));
    assert!(html.contains("作者已确认"));
    assert!(html.contains("当前可分析"));
    assert!(html.contains("role=\"group\""));
    assert!(html.contains("纵轴压缩互动量差距"));
    assert!(html.contains("主页目录，详情待确认"));
    assert!(html.contains("作者已确认"));
    assert!(html.contains("最近巡查新增"));
    assert!(html.contains("life-point-directory"));
    assert!(html.contains("life-point-confirmed"));
    assert!(html.contains("life-point-new"));
    assert_eq!(html.matches("life-point-new-ring").count(), 1);
    assert!(html.contains("发布时间</dt><dd>2026-08-02</dd>"));
    assert!(html.contains(r#"aria-current="true""#));
    assert!(html.contains(r#"life-point-confirmed life-point-selected""#));
    assert!(html.contains(&format!("life_work={selected_ref}")));
    assert!(html.contains(&format!("/corpus/evidence?work={selected_ref}")));
    assert!(html.contains("第二篇"));
    assert_eq!(html.matches(r#"class="life-point-hit""#).count(), 2);
    assert_eq!(html.matches(r#"class="life-point-visible""#).count(), 2);
    assert_eq!(html.matches(r#"class="life-point-hit" cx="#).count(), 2);
    assert_eq!(html.matches(r#"r="6" aria-hidden="true""#).count(), 2);
    assert_eq!(html.matches(r#"r="5" aria-hidden="true""#).count(), 2);
    let recent_at = html.find("最近变化").expect("recent activity is shown");
    let chart_at = html
        .find("id=\"creator-lifecycle\"")
        .expect("lifecycle is shown");
    let gaps_at = html.find("档案缺口").expect("archive gaps are shown");
    assert!(recent_at < chart_at && chart_at < gaps_at);
    assert!(
        html.contains(r#"class="life-point-hit" cx="764.0""#),
        "the right plot inset must leave the hit target inside the desktop plot"
    );
    for forbidden in [
        "监控价值",
        "机会评分",
        "产出分",
        "稀缺分",
        "趋势预测",
        "ARCHIVE HEALTH",
        "NO WORK SET",
        "扫描内作者确认",
        "返回可绘制",
        "复合指标",
        "创作者内分位",
        "creator-percentile",
        "trailing-5-work-median-v1",
    ] {
        assert!(!html.contains(forbidden));
    }

    let mut all_projection = projection.clone();
    all_projection.window = CreatorLifecycleWindow::All;
    let all_html = target_drawer::render(
        Some(&target),
        None,
        Some(&std::collections::HashMap::new()),
        Some(&target.target_ref.to_string()),
        target_drawer::TargetDrawerTab::Overview,
        target_drawer::LifecycleView::Projection(&all_projection),
        None,
        target_drawer::TargetListContext::default(),
    );
    assert!(all_html.contains("UTC+08 全部合格历史"));
    assert!(!all_html.contains("UTC+08 近 90 个日历日"));

    let mut truncated_projection = projection.clone();
    truncated_projection.summary.linked_work_count = None;
    truncated_projection.summary.linked_work_count_lower_bound = 2_001;
    truncated_projection.receipt.probed_count = 2_001;
    truncated_projection.receipt.scanned_count = 2_000;
    truncated_projection.receipt.returned_count = 2;
    truncated_projection.receipt.truncated = true;
    let truncated_html = target_drawer::render(
        Some(&target),
        None,
        Some(&std::collections::HashMap::new()),
        Some(&target.target_ref.to_string()),
        target_drawer::TargetDrawerTab::Overview,
        target_drawer::LifecycleView::Projection(&truncated_projection),
        None,
        target_drawer::TargetListContext::default(),
    );
    assert!(truncated_html.contains("≥2001</b><span>作品目录下限"));
    assert!(truncated_html.contains("当前作品范围仍有后续内容"));
    assert!(!truncated_html.contains("探测 2001"));
    assert!(!truncated_html.contains("返回 2"));
}

#[test]
fn target_drawer_styles_are_lids_bounded_for_the_desktop_workspace() {
    assert!(TARGET_DRAWER_CSS.contains("@media (max-width:900px)"));
    assert!(TARGET_DRAWER_CSS.contains("@media (prefers-reduced-motion:reduce)"));
    assert!(TARGET_DRAWER_CSS.contains("min-height:40px"));
    assert!(TARGET_DRAWER_CSS.contains(".c-tg-table-head.c-tg-creator-grid"));
    assert!(
        !TARGET_DRAWER_CSS.contains("--lgi-space-64"),
        "the creator table must only use declared LIDS spacing tokens"
    );
    assert!(TARGET_DRAWER_CSS.contains(".c-tg-table-head.c-tg-keyword-grid"));
    assert!(
        !TARGET_DRAWER_CSS.contains(".c-tg-table-head.c-tg-keyword-grid,.c-tg-item.c-tg-keyword-grid{grid-template-columns:calc(var(--lgi-space-6) + var(--lgi-space-4)) minmax")
    );
    assert!(TARGET_DRAWER_CSS.contains("overflow-x:auto"));
    assert!(TARGET_DRAWER_CSS.contains("white-space:nowrap"));
    assert!(
        TARGET_DRAWER_CSS.contains(".c-tg-cell{min-width:0;overflow:hidden;text-overflow:ellipsis")
    );
    assert!(
        TARGET_DRAWER_CSS
            .contains(".c-tg-actions{display:flex;align-items:center;justify-content:flex-end")
    );
    assert!(TARGET_DRAWER_CSS.contains(".c-tg-actions form{width:auto;min-width:0;margin:0}"));
    assert!(!TARGET_DRAWER_CSS.contains(".c-tg-btn{width:100%"));
    assert!(
        TARGET_DRAWER_CSS
            .contains(".c-tg-creator-grid>:nth-child(8){margin-left:var(--lgi-space-3)}")
    );
    assert!(!TARGET_DRAWER_CSS.contains(".c-tg-row-open"));
    assert!(TARGET_DRAWER_CSS.contains(".life-control-row{min-width:0;"));
    assert!(TARGET_DRAWER_CSS.contains(".life-figure{min-width:0;"));
    assert!(!TARGET_DRAWER_CSS.contains("gradient"));
    assert!(!TARGET_DRAWER_CSS.contains("#fff"));
    assert!(!TARGET_DRAWER_CSS.contains("#000"));
    assert!(LIDS_TOKENS.contains("--lgi-focus: #335e72"));
    assert!(SHELL_CSS.contains("--v7-focus:var(--lgi-focus)"));
    assert!(SHELL_CSS.contains(
        "[data-theme=\"linggan-intelligence\"] :is(a,button,input,select,textarea,summary,[tabindex]):focus-visible"
    ));
    assert!(!SHELL_CSS.contains("[data-theme=\"linggan-intelligence\"] .v7-app :focus-visible"));
    assert!(SHELL_CSS.contains("outline:2px solid var(--v7-focus); outline-offset:2px"));
    assert!(!SHELL_CSS.contains("!important; outline"));
    assert!(!TARGET_DRAWER_CSS.contains("outline:none"));
    assert!(!TARGET_DRAWER_CSS.contains("!important"));
    assert!(!TARGET_DRAWER_CSS.contains("outline:2px solid var(--lgi-signal)"));
    assert!(TARGET_DRAWER_CSS.contains(
        ".life-point-hit{fill:transparent;stroke:transparent;stroke-width:24;vector-effect:non-scaling-stroke;pointer-events:all}"
    ));
    assert!(TARGET_DRAWER_CSS.contains(".life-point:focus-visible .life-point-visible"));
    assert!(TARGET_DRAWER_CSS.contains("stroke:var(--lgi-focus)"));
    assert!(TARGET_DRAWER_CSS.contains("stroke-width:4"));
    assert!(!TARGET_DRAWER_CSS.contains(".life-point circle"));
}

#[test]
fn invalid_lifecycle_query_is_visible_and_never_claims_defaults() {
    let target = sample_target("creator");
    let html = target_drawer::render(
        Some(&target),
        None,
        Some(&std::collections::HashMap::new()),
        Some(&target.target_ref.to_string()),
        target_drawer::TargetDrawerTab::Overview,
        target_drawer::LifecycleView::QueryInvalid,
        None,
        target_drawer::TargetListContext::default(),
    );
    assert!(html.contains("作品分布的查询条件无效"));
    assert!(!html.contains("QUERY_INVALID"));
    assert!(!html.contains("aria-current=\"true\""));
    assert!(!html.contains("近 90 天口径"));
}

#[test]
fn corpus_work_deep_link_does_not_fall_back_when_the_work_is_off_page() {
    assert!(EVIDENCE_LIBRARY_JS.contains("function directWorkItem(publicRef)"));
    assert!(EVIDENCE_LIBRARY_JS.contains("revealUrlSelection = false"));
    assert!(
        EVIDENCE_LIBRARY_JS.contains(
            "const selectionSource = revealUrlSelection && requestedRef ? 'url' : 'auto'"
        )
    );
    assert!(
        EVIDENCE_LIBRARY_JS
            .contains("await selectItem(directWorkItem(requestedRef), selectionSource)")
    );
    assert!(
        EVIDENCE_LIBRARY_JS.contains(
            "if (selectionSource === 'user' || selectionSource === 'url') openInspector()"
        )
    );
    assert_eq!(
        EVIDENCE_LIBRARY_JS
            .matches("loadList({ keepSelection: true, revealUrlSelection: true })")
            .count(),
        2,
        "initial URL restoration and popstate must reveal the addressed Work without adding history"
    );
    assert!(EVIDENCE_LIBRARY_JS.contains("const requestedRef = restoreRef || model.selectedRef"));
    assert!(!EVIDENCE_LIBRARY_JS.contains(
        "model.items.find((item) => item.identity?.publicRef === (restoreRef || model.selectedRef))\n          || model.items[0]"
    ));
}

#[test]
fn collection_reads_lifecycle_only_for_creator_tabs_that_show_analyzable_counts() {
    let mut target = sample_target("creator");
    assert!(should_read_target_lifecycle(
        Some(&target),
        target_drawer::TargetDrawerTab::parse(None)
    ));
    assert!(should_read_target_lifecycle(
        Some(&target),
        target_drawer::TargetDrawerTab::parse(Some("overview"))
    ));
    assert!(should_read_target_lifecycle(
        Some(&target),
        target_drawer::TargetDrawerTab::parse(Some("baseline"))
    ));
    assert!(!should_read_target_lifecycle(
        Some(&target),
        target_drawer::TargetDrawerTab::parse(Some("patrol"))
    ));
    assert!(should_read_target_lifecycle(
        Some(&target),
        target_drawer::TargetDrawerTab::parse(Some("trace"))
    ));
    assert_eq!(
        target_drawer::TargetDrawerTab::parse(Some("evidence")),
        target_drawer::TargetDrawerTab::Overview
    );
    assert_eq!(
        target_drawer::TargetDrawerTab::parse(Some("unknown")),
        target_drawer::TargetDrawerTab::Overview
    );
    assert!(should_read_target_lifecycle(
        Some(&target),
        target_drawer::TargetDrawerTab::parse(Some("evidence"))
    ));
    assert!(should_read_target_lifecycle(
        Some(&target),
        target_drawer::TargetDrawerTab::parse(Some("unknown"))
    ));
    target.target_kind = "keyword".to_owned();
    assert!(!should_read_target_lifecycle(
        Some(&target),
        target_drawer::TargetDrawerTab::Overview
    ));
    assert!(!should_read_target_lifecycle(
        None,
        target_drawer::TargetDrawerTab::Overview
    ));
}

#[test]
fn target_drawer_escape_and_focus_return_keep_list_context() {
    assert!(COLLECTION_WORKSPACE_JS.contains("drawer.dataset.returnUrl"));
    assert!(COLLECTION_WORKSPACE_JS.contains("drawer.dataset.returnFocus"));
    assert!(COLLECTION_WORKSPACE_JS.contains("window.location.assign(returnUrl)"));
    assert!(COLLECTION_WORKSPACE_JS.contains("window.location.hash.slice(1)"));
    assert!(COLLECTION_WORKSPACE_JS.contains("[data-target-row]"));
    assert!(COLLECTION_WORKSPACE_JS.contains("[data-row-opener]"));
    assert!(COLLECTION_WORKSPACE_JS.contains("[data-row-no-open]"));
    assert!(COLLECTION_WORKSPACE_JS.contains("[data-drawer-initial-focus]"));
    assert!(COLLECTION_WORKSPACE_JS.contains("if (!window.location.hash)"));
    assert!(!COLLECTION_WORKSPACE_JS.contains("window.location.href = \"/collection/targets\""));

    let target = sample_target("creator");
    let context = target_drawer::TargetListContext {
        filter: Some("creator"),
        sort: Some("last"),
    };
    let html = target_drawer::render(
        Some(&target),
        None,
        Some(&std::collections::HashMap::new()),
        Some(&target.target_ref.to_string()),
        target_drawer::TargetDrawerTab::Overview,
        target_drawer::LifecycleView::NotRead {
            window: linggan_evidence::CreatorLifecycleWindow::Recent90Days,
            metric: linggan_evidence::CreatorLifecycleMetric::Likes,
        },
        None,
        context,
    );
    assert!(html.contains("id=\"c-drawer\""));
    assert!(html.contains("data-return-url=\"/collection/targets?filter=creator&amp;sort=last\""));
    assert!(html.contains(&format!(
        "data-return-focus=\"target-{}\"",
        target.target_ref
    )));
    assert!(html.contains(&format!(
        "href=\"/collection/targets?filter=creator&amp;sort=last#target-{}\"",
        target.target_ref
    )));
    assert!(html.contains("filter=creator&amp;sort=last&amp;drawer="));

    let document = target_drawer::attach_to_collection_document(
        r#"<html><body><main>list</main><script src="/assets/collection-workspace.js"></script></body></html>"#,
        &html,
    );
    let drawer_at = document.find("<aside id=\"c-drawer\"").unwrap();
    let script_at = document
        .find(r#"<script src="/assets/collection-workspace.js">"#)
        .unwrap();
    let body_end = document.find("</body>").unwrap();
    assert!(drawer_at < script_at && script_at < body_end);
    assert!(document.ends_with("</html>"));
}

#[test]
fn missing_target_drawer_keeps_return_focus_and_list_context() {
    let target_ref = uuid::Uuid::from_u128(777);
    let context = target_drawer::TargetListContext {
        filter: Some("creator"),
        sort: Some("last"),
    };
    let html = target_drawer::render(
        None,
        None,
        Some(&std::collections::HashMap::new()),
        Some(&target_ref.to_string()),
        target_drawer::TargetDrawerTab::Overview,
        target_drawer::LifecycleView::NotRead {
            window: linggan_evidence::CreatorLifecycleWindow::Recent90Days,
            metric: linggan_evidence::CreatorLifecycleMetric::Likes,
        },
        None,
        context,
    );

    assert!(html.contains("class=\"c-dw\""));
    assert!(html.contains("data-return-url=\"/collection/targets?filter=creator&amp;sort=last\""));
    assert!(html.contains(&format!("data-return-focus=\"target-{target_ref}\"")));
    assert!(html.contains(&format!(
        "href=\"/collection/targets?filter=creator&amp;sort=last#target-{target_ref}\""
    )));
}

#[test]
fn invalid_selected_work_is_dropped_and_drawer_identity_stays_encoded() {
    let mut target = sample_target("creator");
    target.identity_key = "creator\"><svg onload=alert(1)>".to_owned();
    let html = target_drawer::render(
        Some(&target),
        None,
        Some(&std::collections::HashMap::new()),
        Some(&target.target_ref.to_string()),
        target_drawer::TargetDrawerTab::Overview,
        target_drawer::LifecycleView::NotRead {
            window: linggan_evidence::CreatorLifecycleWindow::Recent90Days,
            metric: linggan_evidence::CreatorLifecycleMetric::Likes,
        },
        Some("\"><svg onload=alert(1)>"),
        target_drawer::TargetListContext::default(),
    );

    assert!(html.contains("creator%22%3E%3Csvg%20onload%3Dalert%281%29%3E"));
    assert!(!html.contains("life_work="));
    assert!(!html.contains("\"><svg"));
    assert!(!html.contains("onload=alert"));
}

#[tokio::test]
async fn target_route_never_reflects_an_invalid_selected_work() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/collection/targets?drawer=00000000-0000-0000-0000-000000000001&life_work=%22%3E%3Csvg%20onload%3Dalert%281%29%3E")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();
    assert!(!html.contains("life_work="));
    assert!(!html.contains("onload=alert"));
    assert!(!html.contains("\"><svg onload"));
}

#[test]
fn archive_return_path_drops_untrusted_navigation_fields() {
    let target_ref = uuid::Uuid::from_u128(606);
    let unsafe_form = TargetArchiveForm {
        row_target_ref: target_ref,
        return_filter: Some("\"><svg onload=alert(1)>".to_owned()),
        return_sort: Some("https://example.invalid".to_owned()),
    };
    assert_eq!(
        target_archive_return_path(&unsafe_form, Some("archive_not_requestable")),
        format!(
            "/collection/targets?drawer={target_ref}&dtab=archive&error=archive_not_requestable#target-archive"
        )
    );

    let valid_form = TargetArchiveForm {
        row_target_ref: target_ref,
        return_filter: Some("creator".to_owned()),
        return_sort: Some("last".to_owned()),
    };
    assert_eq!(
        target_archive_return_path(&valid_form, None),
        format!(
            "/collection/targets?filter=creator&sort=last&drawer={target_ref}&dtab=archive#target-archive"
        )
    );
}

#[test]
fn unreadable_drawer_archive_never_offers_a_write_action() {
    let target = sample_target("creator");
    let html = target_drawer::render(
        Some(&target),
        None,
        None,
        Some(&target.target_ref.to_string()),
        target_drawer::TargetDrawerTab::Baseline,
        target_drawer::LifecycleView::NotRead {
            window: linggan_evidence::CreatorLifecycleWindow::Recent90Days,
            metric: linggan_evidence::CreatorLifecycleMetric::Likes,
        },
        None,
        target_drawer::TargetListContext::default(),
    );

    assert!(html.contains("档案状态暂时无法读取"));
    assert!(html.contains("当前读不到"));
    assert!(html.contains("未知状态下发起写操作"));
    assert!(!html.contains(r#"action="/collection/targets/archive""#));
    assert!(!html.contains(">建立档案</button>"));
    assert!(!html.contains(">继续完善</button>"));
}

#[test]
fn drawer_archive_post_preserves_validated_list_and_focus_context() {
    let target = sample_target("creator");
    let html = target_drawer::render(
        Some(&target),
        None,
        Some(&std::collections::HashMap::new()),
        Some(&target.target_ref.to_string()),
        target_drawer::TargetDrawerTab::Baseline,
        target_drawer::LifecycleView::NotRead {
            window: linggan_evidence::CreatorLifecycleWindow::Recent90Days,
            metric: linggan_evidence::CreatorLifecycleMetric::Likes,
        },
        None,
        target_drawer::TargetListContext {
            filter: Some("creator"),
            sort: Some("last"),
        },
    );

    assert!(html.contains(r#"name="return_filter" value="creator""#));
    assert!(html.contains(r#"name="return_sort" value="last""#));
    assert!(html.contains(&format!(
        r#"name="return_drawer" value="{}""#,
        target.target_ref
    )));
    assert!(html.contains(r#"name="return_dtab" value="archive""#));
    assert!(html.contains(r#"name="return_focus" value="target-archive""#));
}

#[test]
fn keyword_drawer_has_only_keyword_overview_and_patrol_without_a_creator_chart_shell() {
    let target = sample_target("keyword");
    let html = target_drawer::render(
        Some(&target),
        None,
        Some(&std::collections::HashMap::new()),
        Some(&target.target_ref.to_string()),
        target_drawer::TargetDrawerTab::Overview,
        target_drawer::LifecycleView::NotRead {
            window: linggan_evidence::CreatorLifecycleWindow::Recent90Days,
            metric: linggan_evidence::CreatorLifecycleMetric::Likes,
        },
        None,
        target_drawer::TargetListContext::default(),
    );
    assert!(html.contains("关键词观察"));
    assert!(html.contains("关键词不建立创作者作品档案"));
    assert!(html.contains(">概览</a>"));
    assert!(html.contains(">巡查</a>"));
    assert!(!html.contains(">档案</a>"));
    assert_eq!(html.matches(r#"<a class="c-dw-tab"#).count(), 2);
    assert!(!html.contains("class=\"life-chart\""));
    assert!(!html.contains("class=\"life-point"));
    assert!(!html.contains("作品生命周期"));
}

#[test]
fn archive_tab_explains_the_first_two_hundred_boundary_without_a_fake_score() {
    let target = sample_target("creator");
    let mut completeness = std::collections::HashMap::new();
    completeness.insert(
        target.identity_key.clone(),
        linggan_evidence::ArchiveCompleteness {
            started: true,
            attempted: true,
            work_in_progress: false,
            author_profile_captures: 1,
            works_listed: 12,
            details_captured: 5,
            quarantined: 1,
            directory_baseline: linggan_evidence::ArchiveDirectoryBaseline::Ready,
        },
    );
    let html = target_drawer::render(
        Some(&target),
        None,
        Some(&completeness),
        Some(&target.target_ref.to_string()),
        target_drawer::TargetDrawerTab::Baseline,
        target_drawer::LifecycleView::NotRead {
            window: linggan_evidence::CreatorLifecycleWindow::Recent90Days,
            metric: linggan_evidence::CreatorLifecycleMetric::Likes,
        },
        None,
        target_drawer::TargetListContext::default(),
    );

    assert!(html.contains("前 200 篇作品链接作为上限"));
    assert!(html.contains(r#"id="target-archive""#));
    assert!(html.contains(r#"id="archive-problems""#));
    assert!(html.contains("1 条记录需要处理"));
    assert!(html.contains("当前先处理上面的隔离记录"));
    assert!(html.contains("5 / 12"));
    assert!(html.contains("当前读不到</b><span>当前可分析"));
    assert!(!html.contains("语料页</b><span>评论与深层材料"));
    assert!(!html.contains(">继续完善</button>"));
    assert!(!html.contains(">建立档案</button>"));
    assert!(!html.contains('%'));
    assert!(!html.contains("ARCHIVE HEALTH"));
    assert!(!html.contains("持久回执"));
}

#[test]
fn patrol_tab_uses_the_last_successful_result_not_the_last_dispatch() {
    let mut target = sample_target("creator");
    target.monitoring_enabled = true;
    target.last_patrol_dispatched_at = Some("2026-09-04 09:00:00+08".to_owned());
    target.last_patrol_succeeded_at = Some("2026-09-04 08:30:00+08".to_owned());
    target.next_patrol_at = Some("2026-09-05 09:00:00+08".to_owned());
    let html = target_drawer::render(
        Some(&target),
        None,
        Some(&std::collections::HashMap::new()),
        Some(&target.target_ref.to_string()),
        target_drawer::TargetDrawerTab::Patrol,
        target_drawer::LifecycleView::NotRead {
            window: linggan_evidence::CreatorLifecycleWindow::Recent90Days,
            metric: linggan_evidence::CreatorLifecycleMetric::Likes,
        },
        None,
        target_drawer::TargetListContext::default(),
    );

    assert!(html.contains("2026-09-04 08:30:00+08"));
    assert!(html.contains("2026-09-05 09:00:00+08"));
    assert!(!html.contains("2026-09-04 09:00:00+08"));
    assert!(html.contains("尚未取得</b><span>最近结果"));
}

fn sample_target(target_kind: &str) -> linggan_evidence::ObservationTarget {
    linggan_evidence::ObservationTarget {
        target_ref: uuid::Uuid::from_u128(501),
        platform: "xhs".to_owned(),
        target_kind: target_kind.to_owned(),
        identity_key: "sample-target".to_owned(),
        display_name: Some("示例目标".to_owned()),
        identity_facts: None,
        source: "manual".to_owned(),
        lifecycle_state: "monitoring".to_owned(),
        first_stored_at: "2026-09-01 00:00:00+00".to_owned(),
        monitoring_enabled: false,
        group_name: None,
        last_patrol_dispatched_at: None,
        last_patrol_succeeded_at: None,
        next_patrol_at: None,
    }
}
