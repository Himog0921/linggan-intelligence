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
    assert_eq!(
        payload
            .pointer("/analysis/rollingMedianVersion")
            .and_then(Value::as_str),
        Some("trailing-5-work-median-v1")
    );
    assert_eq!(
        payload
            .pointer("/analysis/rollingMedianWindow")
            .and_then(Value::as_u64),
        Some(5)
    );
    for forbidden in [
        "bodyText",
        "commentId",
        "commentExternalId",
        "media",
        "evidence",
        "monitoringValue",
        "opportunityScore",
        "selectedWork",
    ] {
        assert_no_json_key(&payload, forbidden);
    }

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
fn creator_drawer_defaults_to_the_server_owned_lifecycle_in_four_tabs() {
    use linggan_evidence::{
        CreatorLifecycleAnalysis, CreatorLifecycleExclusions, CreatorLifecycleMetric,
        CreatorLifecyclePoint, CreatorLifecycleProjection, CreatorLifecycleReceipt,
        CreatorLifecycleStatus, CreatorLifecycleSummary, CreatorLifecycleWindow, ObservationTarget,
    };
    let target = ObservationTarget {
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
        last_patrol_dispatched_at: None,
        next_patrol_at: None,
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
            linked_work_count: 3,
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
            scanned_count: 3,
            truncated: false,
        },
        analysis: CreatorLifecycleAnalysis {
            percentile_version: "creator-percentile-v1",
            rolling_median_version: "trailing-5-work-median-v1",
            rolling_median_window: 5,
            composite_version: "composite-v1",
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
                creator_percentile: 50.0,
                rolling_median: 10.0,
            },
            CreatorLifecyclePoint {
                work_public_ref: selected_ref,
                title: Some("第二篇".to_owned()),
                title_state: "KNOWN",
                published_at: "2026-08-02 00:00:00+00".to_owned(),
                published_local_date: "2026-08-02".to_owned(),
                published_at_epoch_ms: 1_785_628_800_000,
                metric_value: 100,
                creator_percentile: 100.0,
                rolling_median: 55.0,
            },
        ],
    };
    let html = target_drawer::render(
        Some(&target),
        &std::collections::HashMap::new(),
        Some(&target.target_ref.to_string()),
        None,
        target_drawer::LifecycleView::Projection(&projection),
        Some(&selected_ref.to_string()),
    );

    for tab in ["概览", "基线", "巡检策略", "追踪"] {
        assert!(html.contains(tab));
    }
    assert!(!html.contains("dtab=evidence"));
    assert!(!html.contains(">证据</a>"));
    assert!(html.contains("创作者生命周期"));
    assert!(html.contains("role=\"img\""));
    assert!(html.contains("纵轴为 log(1 + 指标)"));
    assert!(html.contains("trailing-5-work-median-v1"));
    assert!(html.contains("发布时间</dt><dd>2026-08-02</dd>"));
    assert!(html.contains(r#"aria-current="page""#));
    assert!(html.contains(&format!("life_work={selected_ref}")));
    assert!(html.contains(&format!("/corpus/evidence?work={selected_ref}")));
    assert!(html.contains("第二篇"));
    for forbidden in ["监控价值", "机会评分", "产出分", "稀缺分", "趋势预测"] {
        assert!(!html.contains(forbidden));
    }
}

#[test]
fn target_drawer_lifecycle_styles_are_lids_bounded_and_mobile_safe() {
    assert!(TARGET_DRAWER_CSS.contains("@media (max-width: 640px)"));
    assert!(TARGET_DRAWER_CSS.contains("@media (prefers-reduced-motion: reduce)"));
    assert!(TARGET_DRAWER_CSS.contains("min-height:40px"));
    assert!(TARGET_DRAWER_CSS.contains(".life-control-row{min-width:0;"));
    assert!(TARGET_DRAWER_CSS.contains(".life-control-label{flex:0 0 100%}"));
    assert!(TARGET_DRAWER_CSS.contains(".life-receipt{grid-template-columns:minmax(0,1fr)}"));
    assert!(TARGET_DRAWER_CSS.contains(".life-figure{min-width:0;"));
    assert!(TARGET_DRAWER_CSS.contains("overflow-wrap:anywhere"));
    assert!(!TARGET_DRAWER_CSS.contains("gradient"));
    assert!(!TARGET_DRAWER_CSS.contains("#fff"));
    assert!(!TARGET_DRAWER_CSS.contains("#000"));
}

#[test]
fn corpus_work_deep_link_does_not_fall_back_when_the_work_is_off_page() {
    assert!(EVIDENCE_LIBRARY_JS.contains("function directWorkItem(publicRef)"));
    assert!(EVIDENCE_LIBRARY_JS.contains("await selectItem(directWorkItem(requestedRef), false)"));
    assert!(EVIDENCE_LIBRARY_JS.contains("const requestedRef = restoreRef || model.selectedRef"));
    assert!(!EVIDENCE_LIBRARY_JS.contains(
        "model.items.find((item) => item.identity?.publicRef === (restoreRef || model.selectedRef))\n          || model.items[0]"
    ));
}

#[test]
fn collection_reads_lifecycle_only_for_the_creator_overview() {
    let mut target = sample_target("creator");
    assert!(should_read_target_lifecycle(Some(&target), None));
    assert!(should_read_target_lifecycle(
        Some(&target),
        Some("overview")
    ));
    assert!(!should_read_target_lifecycle(
        Some(&target),
        Some("baseline")
    ));
    assert!(!should_read_target_lifecycle(Some(&target), Some("patrol")));
    assert!(!should_read_target_lifecycle(Some(&target), Some("trace")));
    target.target_kind = "keyword".to_owned();
    assert!(!should_read_target_lifecycle(Some(&target), None));
    assert!(!should_read_target_lifecycle(None, None));
}

#[test]
fn keyword_drawer_names_lifecycle_as_not_applicable_without_a_chart() {
    let target = sample_target("keyword");
    let html = target_drawer::render(
        Some(&target),
        &std::collections::HashMap::new(),
        Some(&target.target_ref.to_string()),
        None,
        target_drawer::LifecycleView::NotRead {
            window: linggan_evidence::CreatorLifecycleWindow::Recent90Days,
            metric: linggan_evidence::CreatorLifecycleMetric::Likes,
        },
        None,
    );
    assert!(html.contains("不适用于关键词目标"));
    assert!(!html.contains("class=\"life-chart\""));
    assert!(!html.contains("class=\"life-point"));
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
        next_patrol_at: None,
    }
}
