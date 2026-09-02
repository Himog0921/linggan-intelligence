use super::*;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
#[cfg(any())]
use linggan_evidence::{DiscoveryLibraryCard, DiscoveryLibraryProjection};
use linggan_storage_postgres::testing::isolated_proof_schema;
use sqlx::Row;
use std::collections::BTreeMap;
use tower::ServiceExt;

const LOCAL_001_MIGRATIONS: &str = full_schema_fixture::FULL_MIGRATIONS;

#[tokio::test]
async fn health_route_returns_machine_readable_local_state() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/json"
    );
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        payload.pointer("/routes/localProducer"),
        Some(&serde_json::Value::Null),
        "a non-ready local host must not publish a full Producer delivery bundle"
    );
}

#[tokio::test]
async fn local_entry_redirects_to_the_evidence_library() {
    let response = app()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        response.headers().get(header::LOCATION).unwrap(),
        "/corpus/evidence"
    );
}

#[tokio::test]
#[cfg(any())] // superseded server-rendered discovery page
async fn evidence_route_returns_the_honest_empty_state() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/corpus/evidence")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(evidence_library_html(None).contains("来源材料尚未完整接通"));
    assert!(evidence_library_html(None).contains("SOURCE INCOMPLETE"));
    assert!(evidence_library_html(None).contains("没有可展示的本地材料"));
    assert!(!evidence_library_html(None).contains(&["SYSTEM", "LIVE"].join(" ")));
}

#[test]
#[cfg(any())] // superseded server-rendered discovery page
fn evidence_page_does_not_replace_unknown_with_zero() {
    assert!(evidence_library_html(None).contains("覆盖情况 <strong>未知"));
    assert!(!evidence_library_html(None).contains("评论 0"));
}

#[test]
#[cfg(any())] // superseded server-rendered discovery page
fn evidence_library_uses_chinese_for_user_meaning_and_english_only_as_technical_keys() {
    let base = evidence_library_html(None);

    for required in [
        "事实层 / 证据",
        "本机服务 / 读投影未接通 <span class=\"v7-tech-key\">LOCAL HOST / NO READ MODEL</span>",
        "本机时区 <span class=\"v7-tech-key\">UTC+08</span>",
        "当前没有已接纳材料 <span class=\"v7-tech-key\">NO ACCEPTED MATERIAL AVAILABLE</span>",
        "来源材料尚未完整接通",
        "固定面板 <span class=\"v7-tech-key\">PIN</span>",
        "加宽面板 <span class=\"v7-tech-key\">WIDE</span>",
        "平台点赞 <span class=\"v7-tech-key\">PLATFORM LIKES</span>",
        "状态矩阵 <em>STATE MATRIX</em>",
        "仅展示本机页面结构</span><span class=\"v7-tech-key\">LOCAL PRESENTATION</span>",
        "当前尚未读取任何材料</span><span class=\"v7-tech-key\">NO MATERIAL READ</span>",
    ] {
        assert!(
            base.contains(required),
            "missing Chinese-first copy: {required}"
        );
    }

    let projection = DiscoveryLibraryProjection {
        cards: vec![DiscoveryLibraryCard {
            platform: "xhs".to_owned(),
            platform_content_id: "local-language-card".to_owned(),
            title: Some("原始标题保持不翻译".to_owned()),
            creator_display_name: None,
            published_at_source_text: None,
            published_at: None,
            published_at_state: "UNKNOWN",
            first_discovered_at: "2026-08-26 00:00:00+00".to_owned(),
            observed_at: "2026-08-26 00:00:00+00".to_owned(),
            result_position: 17,
            coverage_visible_cards: 20,
            coverage_maximum_quota: 20,
            coverage_stopped_reason: "surface_read_complete".to_owned(),
            cover_presentation_state: "MEDIA_NOT_ACQUIRED",
            cover_local_asset_url: None,
        }],
        excluded_unknown_published_at: 0,
        time_view: "latest_accepted_discovery",
    };
    let html = evidence_page::render_read_projection(&base, &projection, None);

    for required in [
        "创作者未知",
        "发布时间未知 <span class=\"v7-tech-key\">PUBLISHED_AT UNKNOWN</span>",
        "媒体<br>尚未采集<span class=\"v7-tech-key\">MEDIA NOT ACQUIRED</span>",
        "搜索位置 #17</span><span class=\"v7-tech-key\">POSITION #17</span>",
        "已接纳的本机发现材料 <span class=\"v7-tech-key\">ACCEPTED RUNTIME MATERIAL</span>",
        "观察到 / 配额 <span class=\"v7-tech-key\">OBSERVED / QUOTA</span>",
        "停止原因：当前页面读取完成 <span class=\"v7-tech-key\">surface_read_complete</span>",
        "已接纳的发现卡片 <span class=\"v7-tech-key\">ACCEPTED DISCOVERY</span>",
        "当前显示最新已接纳的发现卡片；其中部分卡片的发布时间仍可能未知。",
        "<span class=\"v7-tech-key\">LATEST ACCEPTED DISCOVERY · PUBLISHED_AT UNKNOWN</span>",
        "只读取 Linggan 已接纳的发现卡片；不触发平台采集 <span class=\"v7-tech-key\">DISCOVERY ONLY</span>",
        "本机服务 / 已接纳发现材料 <span class=\"v7-tech-key\">LOCAL HOST / ACCEPTED DISCOVERY</span>",
        "<span class=\"v7-kpi\"><em>内容</em><b>1</b></span>",
    ] {
        assert!(
            html.contains(required),
            "missing Chinese-first dynamic copy: {required}"
        );
    }

    for prohibited in [
        "本机服务 / 读投影未接通（LOCAL HOST / NO READ MODEL）",
        "本机服务 / 读投影未接通",
        "本地读投影未接通",
        "READ MODEL NOT CONNECTED",
        "aria-label=\"MEDIA NOT ACQUIRED\"",
        "<span class=\"v7-published-unknown\">PUBLISHED",
        "<span>POSITION #17</span>",
        "<b>ACCEPTED RUNTIME MATERIAL</b>",
        "<span>OBSERVED / QUOTA</span>",
        "LOCAL PRESENTATION ONLY / NO MATERIAL READ",
        ">ACCEPTED DISCOVERY CARDS<",
        "VIEW = LATEST ACCEPTED DISCOVERY / PUBLISHED_AT MAY BE UNKNOWN",
        "只读取 Linggan 已接纳的 discovery 卡片",
        "本机服务 / 已接纳发现材料（LOCAL HOST / ACCEPTED DISCOVERY）",
        "<span>UTC+08</span>",
    ] {
        assert!(
            !html.contains(prohibited),
            "English technical key must not carry this user meaning alone: {prohibited}"
        );
    }

    let stylesheet = evidence_page_stylesheet();
    assert!(
        stylesheet.contains(".v7-tech-key{display:inline;color:var(--v7-ghost);font:500 .82em/1.2"),
        "technical annotations must be visually smaller than their Chinese primary expression"
    );
    assert!(
        stylesheet.contains(".v7-media-pending .v7-tech-key{display:block;max-width:54px;margin-top:4px;font-size:.82em"),
        "the media-state technical annotation must also remain subordinate on discovery cards"
    );
}

#[test]
#[cfg(any())] // superseded server-rendered discovery page
fn base_no_db_header_and_nested_technical_keys_remain_chinese_first() {
    let base = evidence_library_html(None);
    assert!(base.contains("<!-- EVIDENCE_HEADER_BOUNDARY_START -->"));
    assert!(base.contains("<!-- EVIDENCE_HEADER_META_STATE_START -->"));
    assert!(base.contains(
        "本机服务 / 读投影未接通 <span class=\"v7-tech-key\">LOCAL HOST / NO READ MODEL</span>"
    ));
    assert!(
        base.contains(
            "本地读投影未接通 <span class=\"v7-tech-key\">READ MODEL NOT CONNECTED</span>"
        )
    );
    assert!(
        !base.contains("本机服务 / 读投影未接通（LOCAL HOST / NO READ MODEL）"),
        "the English no-read-model sentence must not become the header state"
    );

    let stylesheet = evidence_page_stylesheet();
    for selector in [
        ".v7-fact-strap span .v7-tech-key",
        ".v7-empty-metric span .v7-tech-key",
        ".v7-readout span .v7-tech-key",
    ] {
        assert!(
            stylesheet.contains(selector),
            "a nested technical key needs a selector stronger than its label: {selector}"
        );
    }
    for rule in [
        ".v7-view-label em,\n.v7-filter-lead em { margin-left:7px; color:var(--v7-ghost); font:500 .82em/1.2 var(--lgi-font-mono); font-style:normal; letter-spacing:.06em; }",
        ".v7-section h3 em { margin-right:auto; margin-left:8px; color:var(--v7-ghost); font:500 .82em/1.2 var(--lgi-font-mono); font-style:normal; letter-spacing:.06em; }",
    ] {
        assert!(
            stylesheet.contains(rule),
            "English annotation must not outweigh its Chinese label: {rule}"
        );
    }
}

#[test]
#[cfg(any())] // superseded server-rendered discovery page
fn invalid_and_unavailable_reads_render_the_actual_chinese_state_without_source_incomplete() {
    for (html, primary, code, detail) in [
        (
            evidence_query_invalid_html(),
            "当前查询参数无效",
            "LOCAL_QUERY_INVALID",
            "当前未读取任何材料，也未触发平台搜索或补采。",
        ),
        (
            evidence_read_unavailable_html(),
            "本机发现材料读取暂时不可用",
            "READ_PROJECTION_UNAVAILABLE",
            "当前未读取任何材料；没有显示旧系统或远程数据。",
        ),
    ] {
        assert!(
            html.contains(primary),
            "missing Chinese primary state: {primary}"
        );
        assert!(
            html.contains(&format!("<span class=\"v7-tech-key\">{code}</span>")),
            "the raw code must remain an adjacent technical key: {code}"
        );
        assert!(
            html.contains(detail),
            "missing exact state boundary: {detail}"
        );
        assert!(html.contains("当前未读取材料"));
        assert!(html.contains("当前未读取材料，来源状态未知"));
        assert!(
            !html.contains("来源信息尚未完整接通"),
            "a query/read failure must not be misdescribed as source incompleteness"
        );
        assert!(
            !html.contains("SOURCE INCOMPLETE"),
            "a query/read failure must not inherit the base SOURCE_INCOMPLETE code"
        );
        assert!(
            !html.contains("读投影未接通"),
            "a query/read failure must not be misdescribed as a disconnected read model"
        );
        assert!(
            !html.contains("当前没有已接纳材料"),
            "a failed read must not be misdescribed as an empty accepted-material set"
        );
        assert!(
            !html.contains("当前没有可用查询"),
            "the old generic query copy must not survive a state-specific failure"
        );
    }
}

#[test]
fn discovery_stop_reasons_keep_raw_codes_but_lead_with_truthful_chinese_meaning() {
    for (raw_reason, expected_meaning) in [
        ("quota_reached", "已达到本次配额"),
        ("surface_ended", "当前页面内容已结束"),
        ("risk_control", "平台风险控制导致停止"),
        ("manual_stop", "用户手动停止"),
        ("unknown", "停止原因未知"),
        ("future_reason", "停止原因未归类"),
    ] {
        let markup = evidence_page::stopped_reason_markup(raw_reason);
        assert!(
            markup.contains(expected_meaning),
            "missing meaning for {raw_reason}"
        );
        assert!(
            markup.contains(&format!("<span class=\"v7-tech-key\">{raw_reason}</span>")),
            "the stored code must remain an adjacent technical annotation for {raw_reason}"
        );
    }
}

#[test]
#[cfg(any())] // superseded server-rendered discovery page
fn evidence_page_keeps_the_v7_shell_and_three_column_geometry() {
    let html = evidence_library_html(None);

    for required in [
        "v7-global-header",
        "v7-context-row",
        "v7-side",
        "v7-page-header",
        "v7-workspace",
        "v7-results",
        "v7-inspect",
        "FACT LAYER / EVIDENCE",
    ] {
        assert!(html.contains(required), "missing V7 structure: {required}");
    }

    assert!(
        !evidence_page_stylesheet().contains("#e8003f"),
        "the second signature colour must stay retired: DESIGN-003 collapsed the palette onto --lgi-signal"
    );
    assert!(
        !evidence_page_stylesheet().contains('#'),
        "page CSS must not author raw colour: every value resolves through a LIDS token"
    );

    for required_css in [
        "html,body { height:100%; overflow:hidden; }",
        ".v7-app { height:100vh; min-height:0;",
        "--v7-header-height:128px",
        "--v7-side-width:216px",
        "--v7-inspector-width:440px",
        "--v7-red:var(--lgi-signal)",
        "--v7-black:var(--lgi-ink)",
        "@media(max-width:900px){html,body{height:auto;min-height:100%;overflow:auto}.v7-app{height:auto;min-height:100vh;grid-template-rows:auto minmax(0,1fr);overflow:visible}",
    ] {
        assert!(
            evidence_page_stylesheet().contains(required_css),
            "missing V7 page-local visual constant: {required_css}"
        );
    }
}

#[test]
#[cfg(any())] // superseded server-rendered discovery page
fn evidence_page_has_no_fabricated_v7_runtime_material_or_actions() {
    let html = evidence_library_html(None);

    let prohibited = [
        ["SYSTEM", "LIVE"].join(" "),
        ["INDEX", "FRESH"].join(" "),
        ["12", "482"].join(","),
        ["327", "9K"].join("."),
        ["XHS", "78F2A"].join("-"),
        ["为什么 ADHD 孩子", "每天写作业都像打仗？"].concat(),
        ["已创建", "补采任务"].concat(),
        ["已保存为", "个人视图"].concat(),
    ];

    for prohibited in &prohibited {
        assert!(
            !html.contains(prohibited),
            "fabricated V7 value: {prohibited}"
        );
    }

    assert!(html.contains("disabled aria-disabled=\"true\""));
    assert!(html.contains(
        "当前没有已接纳材料 <span class=\"v7-tech-key\">NO ACCEPTED MATERIAL AVAILABLE</span>"
    ));
}

#[test]
fn runtime_token_source_matches_the_full_lids_baseline() {
    let runtime = declared_token_values(LIDS_TOKENS);
    let documented = declared_token_values(LIDS_TOKEN_DOCUMENT);

    assert_eq!(runtime.len(), 131);
    assert_eq!(documented.len(), 131);
    assert_eq!(runtime, documented);
    assert!(declared_token_values(SHELL_CSS).is_empty());
    assert!(declared_token_values(EVIDENCE_LIBRARY_CSS).is_empty());
}

#[test]
#[cfg(any())] // superseded server-rendered discovery page
fn read_projection_escapes_source_text_and_never_emits_a_remote_cover_url() {
    let projection = DiscoveryLibraryProjection {
        cards: vec![DiscoveryLibraryCard {
            platform: "xhs".to_owned(),
            platform_content_id: "note-a".to_owned(),
            title: Some("<script>not a cover</script>".to_owned()),
            creator_display_name: Some("A娃 & 家长".to_owned()),
            published_at_source_text: Some("2026-08-25T00:00:00Z".to_owned()),
            published_at: Some("2026-08-25 00:00:00+00".to_owned()),
            published_at_state: "KNOWN",
            first_discovered_at: "2026-08-25 00:00:00+00".to_owned(),
            observed_at: "2026-08-25 00:00:00+00".to_owned(),
            result_position: 1,
            coverage_visible_cards: 1,
            coverage_maximum_quota: 20,
            coverage_stopped_reason: "risk_control".to_owned(),
            cover_presentation_state: "MEDIA_NOT_ACQUIRED",
            cover_local_asset_url: None,
        }],
        excluded_unknown_published_at: 0,
        time_view: "last_30_days",
    };
    let html = evidence_page::render_read_projection(
        &evidence_library_html(None),
        &projection,
        Some("A娃"),
    );

    assert!(html.contains("&lt;script&gt;not a cover&lt;/script&gt;"));
    assert!(html.contains("媒体<br>尚未采集"));
    assert!(html.contains("MEDIA NOT ACQUIRED"));
    assert!(!html.contains("<script>not a cover</script>"));
    assert!(!html.contains("https://"));
    assert!(!html.contains("xhscdn"));
}

#[test]
#[cfg(any())] // superseded server-rendered discovery page
fn default_read_view_surfaces_unknown_published_time_without_a_surrogate_date() {
    let projection = DiscoveryLibraryProjection {
        cards: vec![DiscoveryLibraryCard {
            platform: "xhs".to_owned(),
            platform_content_id: "synthetic-unknown-publication".to_owned(),
            title: Some("synthetic accepted discovery".to_owned()),
            creator_display_name: Some("synthetic creator".to_owned()),
            published_at_source_text: None,
            published_at: None,
            published_at_state: "UNKNOWN",
            first_discovered_at: "2026-08-26 00:00:00+00".to_owned(),
            observed_at: "2026-08-26 00:00:00+00".to_owned(),
            result_position: 1,
            coverage_visible_cards: 1,
            coverage_maximum_quota: 20,
            coverage_stopped_reason: "surface_read_complete".to_owned(),
            cover_presentation_state: "MEDIA_NOT_ACQUIRED",
            cover_local_asset_url: None,
        }],
        excluded_unknown_published_at: 0,
        time_view: "latest_accepted_discovery",
    };

    let html =
        evidence_page::render_read_projection(&evidence_library_html(None), &projection, None);

    assert!(html.contains("PUBLISHED_AT UNKNOWN"));
    assert!(html.contains("未用首次发现、观察或接收时间替代"));
    assert!(html.contains("当前显示最新已接纳的发现卡片；其中部分卡片的发布时间仍可能未知。"));
    assert!(html.contains("LATEST ACCEPTED DISCOVERY · PUBLISHED_AT UNKNOWN"));
    assert!(!html.contains("VIEW = LATEST ACCEPTED DISCOVERY / PUBLISHED_AT MAY BE UNKNOWN"));
    assert!(html.contains("<em>视角</em> <span class=\"v7-zh-value\">最新已接纳</span>"));
    assert!(!html.contains("2026-08-26 00:00:00+00"));
}

#[test]
#[cfg(any())] // superseded server-rendered discovery page
fn strict_published_window_reports_unknown_exclusions_even_with_visible_cards() {
    let known_card = || DiscoveryLibraryCard {
        platform: "xhs".to_owned(),
        platform_content_id: "synthetic-known-publication".to_owned(),
        title: Some("synthetic known discovery".to_owned()),
        creator_display_name: Some("synthetic creator".to_owned()),
        published_at_source_text: Some("2026-08-26T00:00:00Z".to_owned()),
        published_at: Some("2026-08-26 00:00:00+00".to_owned()),
        published_at_state: "KNOWN",
        first_discovered_at: "2026-08-26 00:00:00+00".to_owned(),
        observed_at: "2026-08-26 00:00:00+00".to_owned(),
        result_position: 1,
        coverage_visible_cards: 1,
        coverage_maximum_quota: 20,
        coverage_stopped_reason: "surface_read_complete".to_owned(),
        cover_presentation_state: "MEDIA_NOT_ACQUIRED",
        cover_local_asset_url: None,
    };
    let strict_html = evidence_page::render_read_projection(
        &evidence_library_html(None),
        &DiscoveryLibraryProjection {
            cards: vec![known_card()],
            excluded_unknown_published_at: 1,
            time_view: "last_30_days",
        },
        None,
    );
    assert!(strict_html.contains("当前查询有 1 个对象因来源发布时间未知而未进入此发布时间窗口"));
    assert!(strict_html.contains("synthetic known discovery"));

    let default_html = evidence_page::render_read_projection(
        &evidence_library_html(None),
        &DiscoveryLibraryProjection {
            cards: vec![known_card()],
            excluded_unknown_published_at: 0,
            time_view: "latest_accepted_discovery",
        },
        None,
    );
    assert!(!default_html.contains("未进入此发布时间窗口"));
}

#[test]
#[cfg(any())] // superseded server-rendered discovery page
fn default_empty_read_view_is_not_misdescribed_as_an_empty_published_window() {
    let projection = DiscoveryLibraryProjection {
        cards: vec![],
        excluded_unknown_published_at: 0,
        time_view: "latest_accepted_discovery",
    };

    let html =
        evidence_page::render_read_projection(&evidence_library_html(None), &projection, None);

    assert!(html.contains("当前视角没有可展示卡片"));
    assert!(html.contains("最新已接纳不是发布时间窗口"));
    assert!(!html.contains("当前窗口没有可展示卡片"));
}

#[test]
fn default_local_query_is_latest_accepted_discovery_and_explicit_windows_remain_published_only() {
    let default = material_projection::local_query(&material_projection::EvidenceLibraryParams {
        cursor: None,
        q: None,
        window: None,
        sort: None,
        lane: None,
        lane_state: None,
        media_kind: None,
        restriction: None,
    })
    .expect("an omitted URL window selects the explicit default discovery view");
    assert_eq!(
        default.time_view(),
        linggan_contracts::EvidenceTimeView::LatestAcceptedDiscovery
    );
    assert_eq!(default.published_window(), None);

    let explicit = material_projection::local_query(&material_projection::EvidenceLibraryParams {
        cursor: None,
        q: None,
        window: Some("last_30_days".to_owned()),
        sort: None,
        lane: None,
        lane_state: None,
        media_kind: None,
        restriction: None,
    })
    .expect("an explicit published window remains valid");
    assert_eq!(
        explicit.time_view(),
        linggan_contracts::EvidenceTimeView::PublishedLast30Days
    );
    assert_eq!(
        explicit.published_window(),
        Some(linggan_contracts::PublishedWindow::Last30Days)
    );
}

#[test]
fn resumable_media_temp_bytes_are_never_published_until_the_final_promotion() {
    let root = std::env::temp_dir().join(format!("linggan-media-helper-{}", uuid::Uuid::new_v4()));
    let temporary = root.join("uploads/fixture/bytes.part");
    let final_path = root.join("blobs/88/fixture");

    write_media_chunk(&temporary, 0, b"ab").expect("first synthetic chunk writes");
    assert!(
        !final_path.exists(),
        "an interrupted upload has no public blob path"
    );
    assert!(
        write_media_chunk(&temporary, 0, b"cd").is_err(),
        "offset replay cannot overwrite the partial file"
    );
    write_media_chunk(&temporary, 2, b"cd").expect("second synthetic chunk resumes exactly");
    assert!(
        atomically_promote_media_upload(&temporary, &final_path).expect("final promotion succeeds")
    );
    assert_eq!(
        std::fs::read(&final_path).expect("published synthetic bytes"),
        b"abcd"
    );
    std::fs::remove_dir_all(root).expect("synthetic media root cleans up");
}

#[tokio::test]
#[ignore = "requires ./scripts/test-local-001-discovery-postgres.sh and an isolated PostgreSQL proof database"]
#[cfg(any())] // superseded server-rendered discovery page
async fn loopback_ingress_then_library_page_only_returns_locally_accepted_discovery_cards() {
    let database = proof_database("local_api_ingress").await;
    let observed_at = producer_fixture_observed_at(&database).await;
    let application = app_with_database(database);

    assert_discovery_package_is_accepted(application.clone(), discovery_package(&observed_at))
        .await;

    let body = get_successful_utf8_response(
        application.clone(),
        "/api/local/evidence-library/legacy?q=ADHD&window=last_30_days",
    )
    .await;
    assert!(body.contains("note-api-known"));
    assert!(!body.contains("note-api-unknown"));
    assert!(body.contains("\"timeView\":\"last_30_days\""));
    assert!(body.contains("\"excludedUnknownPublishedAt\":1"));
    assert!(!body.contains("https://"));
    assert!(!body.contains("xhscdn"));

    let body = get_successful_utf8_response(
        application.clone(),
        "/api/local/evidence-library/legacy?q=ADHD",
    )
    .await;
    assert!(body.contains("note-api-known"));
    assert!(body.contains("note-api-unknown"));
    assert!(body.contains("\"timeView\":\"latest_accepted_discovery\""));
    assert!(body.contains("\"excludedUnknownPublishedAt\":0"));

    let html = get_successful_utf8_response(
        application.clone(),
        "/corpus/evidence?q=ADHD&window=last_30_days",
    )
    .await;
    assert!(html.contains("API 接纳卡片"));
    assert!(html.contains("媒体<br>尚未采集"));
    assert!(html.contains("MEDIA NOT ACQUIRED"));
    assert!(!html.contains("https://"));
    assert!(!html.contains("xhscdn"));

    let html = get_successful_utf8_response(application, "/corpus/evidence?q=ADHD").await;
    assert!(html.contains("PUBLISHED_AT UNKNOWN"));
    assert!(html.contains("未用首次发现、观察或接收时间替代"));
}

#[tokio::test]
#[ignore = "requires ./scripts/test-local-001-discovery-postgres.sh and an isolated PostgreSQL proof database"]
#[cfg(any())] // superseded server-rendered discovery page
async fn loopback_projection_counts_unknown_published_time_by_content_item_identity() {
    let database = proof_database("local_api_identity_unknown").await;
    let observed_at = producer_fixture_observed_at(&database).await;
    let application = app_with_database(database);

    let response =
        post_discovery_package(application.clone(), discovery_package(&observed_at)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let response = post_discovery_package(
        application.clone(),
        shared_api_content_unknown_package(&observed_at),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let response = application
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/local/evidence-library/legacy?q=API&window=last_30_days")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(body.contains("note-api-known"));
    assert!(body.contains("\"excludedUnknownPublishedAt\":0"));

    let response = application
        .oneshot(
            Request::builder()
                .uri("/corpus/evidence?q=API&window=last_30_days")
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
    assert!(html.contains("API 接纳卡片"));
    assert!(!html.contains("当前查询候选中 1 个对象因发布时间未知而未进入窗口"));
}

#[tokio::test]
#[ignore = "requires ./scripts/test-local-001-discovery-postgres.sh and an isolated PostgreSQL proof database"]
#[cfg(any())] // superseded server-rendered discovery page
async fn loopback_7_day_query_keeps_api_and_page_window_metadata_in_sync() {
    let database = proof_database("local_api_window_metadata").await;
    let observed_at = producer_fixture_observed_at(&database).await;
    let application = app_with_database(database);

    let response = application
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/local/discovery-packages")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(discovery_package(&observed_at)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let response = application
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/local/evidence-library/legacy?q=ADHD&window=last_7_days")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(body.contains("\"timeView\":\"last_7_days\""));
    assert!(!body.contains("\"timeView\":\"last_30_days\""));

    let response = application
        .oneshot(
            Request::builder()
                .uri("/corpus/evidence?q=ADHD&window=last_7_days")
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
    assert!(html.contains("<em>窗口</em> <span class=\"v7-zh-value\">近 7 天</span>"));
    assert!(html.contains("<span class=\"v7-tech-key\">PUBLISHED_AT / 7D</span>"));
    assert!(!html.contains("PUBLISHED_AT / 30D"));
}

#[tokio::test]
#[ignore = "requires ./scripts/test-local-001-discovery-postgres.sh and an isolated PostgreSQL proof database"]
async fn loopback_rejects_conflicting_quota_reached_and_keeps_truthful_partial_cards() {
    let database = proof_database("local_api_quota_consistency").await;
    let observed_at = producer_fixture_observed_at(&database).await;
    let application = app_with_database(database);
    let conflicting = discovery_package(&observed_at).replace(
        "\"stoppedReason\":\"risk_control\"",
        "\"stoppedReason\":\"quota_reached\"",
    );

    let response = post_discovery_package(application.clone(), conflicting).await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(body.contains("\"admission\":\"not_accepted\""));

    let response = post_discovery_package(application, discovery_package(&observed_at)).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
#[ignore = "requires ./scripts/test-local-001-discovery-postgres.sh and an isolated PostgreSQL proof database"]
async fn loopback_local_producer_acknowledges_one_partial_package_and_replays_timeout_submission() {
    let database = proof_database("local_api_producer").await;
    let observed_at = producer_fixture_observed_at(&database).await;
    let application = app_with_database(database);
    let task = local_task_spec();
    let attempt = local_attempt();
    let submission = local_submission(&observed_at);
    for (path, body, expected) in [
        (
            "/api/local/producer/manual-tasks",
            task.clone(),
            "\"outcome\":\"created\"",
        ),
        (
            "/api/local/producer/attempts",
            attempt.clone(),
            "\"outcome\":\"started\"",
        ),
        (
            "/api/local/producer/submissions",
            submission.clone(),
            "\"delivery\":\"acknowledged\"",
        ),
        (
            "/api/local/producer/submissions",
            submission,
            "\"delivery\":\"replay\"",
        ),
    ] {
        let response = application
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(path)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert!(String::from_utf8_lossy(&body).contains(expected));
    }
    let conflicting_attempt = attempt.replace(
        "22222222-2222-4222-8222-222222222222",
        "88888888-8888-4888-8888-888888888888",
    );
    let response = application
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/local/producer/attempts")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(conflicting_attempt))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert!(String::from_utf8_lossy(&body).contains("attempt_identity_conflict"));
    let terminal_conflict = local_submission(&observed_at)
        .replace(
            "44444444-4444-4444-8444-444444444444",
            "55555555-5555-4555-8555-555555555555",
        )
        .replace("note-api-known", "note-api-conflict");
    let response = application
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/local/producer/submissions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(terminal_conflict))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert!(String::from_utf8_lossy(&body).contains("attempt_terminal_submission_conflict"));
    let invalid_scheduler = task.replace("\"manual\"", "\"scheduler\"");
    let response = application
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/local/producer/manual-tasks")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(invalid_scheduler))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
#[ignore = "requires ./scripts/test-local-001-discovery-postgres.sh and an isolated PostgreSQL proof database"]
async fn loopback_runtime_producer_uses_the_routes_published_by_health() {
    let database = proof_database("local_api_runtime_route_contract").await;
    let application = app_with_database(database);
    let health_response = application
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(health_response.status(), StatusCode::OK);
    let health_body = to_bytes(health_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let health: serde_json::Value = serde_json::from_slice(&health_body).unwrap();
    assert_eq!(
        health.pointer("/dataState"),
        Some(&serde_json::Value::String(
            "LINGGAN_BROWSER_PRODUCER_RUNTIME".to_owned()
        ))
    );
    assert_eq!(
        health.pointer("/database/schema"),
        Some(&serde_json::Value::String(
            "PLUGIN_RUNTIME_002_SCHEMA_READY".to_owned()
        ))
    );
    assert_eq!(
        health
            .pointer("/routes/workResources")
            .and_then(Value::as_str),
        Some("/api/local/work-resources")
    );
    assert_eq!(
        health
            .pointer("/routes/dispatch/claim")
            .and_then(Value::as_str),
        Some("/api/local/dispatch/claim"),
        "a ready local runtime publishes the task-claim route"
    );
    assert_eq!(
        health
            .pointer("/routes/dispatch/failure")
            .and_then(Value::as_str),
        Some("/api/local/dispatch/failures"),
        "a ready local runtime publishes the failure-recovery route with claim"
    );
    let task_path = health
        .pointer("/routes/localProducer/taskCreation")
        .and_then(serde_json::Value::as_str)
        .expect("health publishes task creation route")
        .to_owned();
    let attempt_path = health
        .pointer("/routes/localProducer/attemptStart")
        .and_then(serde_json::Value::as_str)
        .expect("health publishes attempt start route")
        .to_owned();
    let submission_path = health
        .pointer("/routes/localProducer/submission")
        .and_then(serde_json::Value::as_str)
        .expect("health publishes submission route")
        .to_owned();
    let task = runtime_producer_task_spec();
    let attempt = runtime_producer_attempt();
    let submission = runtime_producer_submission();
    for (path, body, expected) in [
        (task_path, task, "\"outcome\":\"created\""),
        (attempt_path, attempt, "\"outcome\":\"started\""),
        (
            submission_path.clone(),
            submission.clone(),
            "\"delivery\":\"acknowledged\"",
        ),
        (submission_path, submission, "\"delivery\":\"replay\""),
    ] {
        let response = application
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(&path)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert!(String::from_utf8_lossy(&body).contains(expected));
    }
}

#[tokio::test]
#[ignore = "requires ./scripts/test-local-001-discovery-postgres.sh and an isolated PostgreSQL proof database"]
async fn accepted_author_profile_creates_a_deduplicated_observation_target_with_target_receipt() {
    let database = proof_database("author_profile_target_receipt").await;
    let application = app_with_database(database.clone());
    for (path, body) in [
        (
            "/api/local/producer/tasks",
            runtime_author_producer_task_spec(),
        ),
        (
            "/api/local/producer/runtime-attempts",
            runtime_author_producer_attempt(),
        ),
    ] {
        let response = application
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(path)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
    let response = application
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/local/producer/runtime-submissions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(runtime_author_producer_submission()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let receipt: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        receipt.pointer("/delivery").and_then(Value::as_str),
        Some("acknowledged")
    );
    assert_eq!(
        receipt.pointer("/targetSync/state").and_then(Value::as_str),
        Some("created")
    );
    assert_eq!(
        receipt
            .pointer("/targetSync/targetRef")
            .and_then(Value::as_str)
            .is_some(),
        true,
    );
    let target: (String, String, serde_json::Value) = sqlx::query_as(
        "SELECT identity_key,display_name,identity_facts \
         FROM collection_observation_target WHERE platform='xhs' AND target_kind='creator'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(target.0, "author-route-1");
    assert_eq!(target.1, "路由作者");
    assert_eq!(
        target.2["avatar"],
        "https://media.example/author-route-1.jpg"
    );
}

async fn proof_database(schema: &str) -> Database {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL")
        .expect("test script must provide the isolated proof database URL");
    isolated_proof_schema(&url, schema, LOCAL_001_MIGRATIONS)
        .await
        .expect("isolated migration applies")
}

async fn producer_fixture_observed_at(database: &Database) -> String {
    sqlx::query(
        "SELECT to_char(scope_001_now() AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS observed_at",
    )
        .fetch_one(database.pool())
        .await
        .expect("database returns a timestamp")
        .get("observed_at")
}

async fn post_discovery_package(application: Router, package: String) -> axum::response::Response {
    application
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/local/discovery-packages")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(package))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn assert_discovery_package_is_accepted(application: Router, package: String) {
    let response = post_discovery_package(application, package).await;
    assert_eq!(response.status(), StatusCode::OK);
}

async fn get_successful_utf8_response(application: Router, uri: &str) -> String {
    let response = application
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap()
}

fn discovery_package(observed_at: &str) -> String {
    format!(
        r#"{{
          "contractVersion":"xhs.discovery.visible-card.v1",
          "acquisitionSpec":{{"platform":"xhs","query":"ADHD","sort":"comprehensive","target":{{"basis":"maximum_quota","unit":"visible_search_card","maximumQuota":20}}}},
          "observedAt":"{observed_at}",
          "coverage":{{"unit":"visible_search_card","visibleCards":2,"stoppedReason":"risk_control"}},
          "cards":[
            {{"content":{{"platformContentId":"note-api-known","title":"API 接纳卡片 ADHD","creatorDisplayName":"A娃家长","publishedAtSourceText":"{observed_at}","coverCandidate":{{"observedExternalUri":"https://xhscdn.example/api-cover"}}}},"occurrence":{{"query":"ADHD","sort":"comprehensive","observedAt":"{observed_at}","resultPosition":1}}}},
            {{"content":{{"platformContentId":"note-api-unknown","title":"ADHD 未知发布时间","creatorDisplayName":"另一位家长"}},"occurrence":{{"query":"ADHD","sort":"comprehensive","observedAt":"{observed_at}","resultPosition":2}}}}
          ]
        }}"#,
    )
}

fn shared_api_content_unknown_package(observed_at: &str) -> String {
    format!(
        r#"{{
          "contractVersion":"xhs.discovery.visible-card.v1",
          "acquisitionSpec":{{"platform":"xhs","query":"ADHD","sort":"comprehensive","target":{{"basis":"maximum_quota","unit":"visible_search_card","maximumQuota":20}}}},
          "observedAt":"{observed_at}",
          "coverage":{{"unit":"visible_search_card","visibleCards":1,"stoppedReason":"risk_control"}},
          "cards":[
            {{"content":{{"platformContentId":"note-api-known","title":"API 接纳卡片 ADHD","creatorDisplayName":"A娃家长"}},"occurrence":{{"query":"ADHD","sort":"comprehensive","observedAt":"{observed_at}","resultPosition":1}}}}
          ]
        }}"#,
    )
}

fn local_task_spec() -> String {
    r#"{"contractVersion":"linggan.task-spec.v1","taskId":"11111111-1111-4111-8111-111111111111","source":"manual","platform":"xhs","pageType":"search_results","target":"current_visible_search_surface","capabilitiesRequested":["discover_visible_cards"],"maximumQuota":20,"commentLimit":"not_requested","acquireMedia":"not_requested","riskPolicy":"local_trusted_user_initiated","stopConditions":["current_surface_read_once","maximum_quota"]}"#.to_owned()
}

fn local_attempt() -> String {
    r#"{"contractVersion":"linggan.local-trusted.attempt.v1","producerInstanceId":"22222222-2222-4222-8222-222222222222","taskId":"11111111-1111-4111-8111-111111111111","attemptId":"33333333-3333-4333-8333-333333333333"}"#.to_owned()
}

fn local_submission(observed_at: &str) -> String {
    format!(
        r#"{{"contractVersion":"linggan.local-trusted.submission.v1","producerInstanceId":"22222222-2222-4222-8222-222222222222","taskId":"11111111-1111-4111-8111-111111111111","attemptId":"33333333-3333-4333-8333-333333333333","submissionId":"44444444-4444-4444-8444-444444444444","discoveryPackage":{}}}"#,
        discovery_package(observed_at)
    )
}

#[test]
fn collection_serves_all_five_sub_surfaces_from_the_shared_shell() {
    for (section, name) in [
        (collection::Section::Targets, "观察目标"),
        (collection::Section::Operations, "生产流"),
        (collection::Section::Attention, "待处理"),
        (collection::Section::Tasks, "采集任务"),
        (collection::Section::Runtime, "执行工位"),
    ] {
        let html = collection::render(section, collection::OperationsMode::Now, None, None, None);
        // The breadcrumb, not a title block, is where a surface states which page this is.
        assert!(
            html.contains(&format!("<b>{name}</b>")),
            "missing surface breadcrumb: {name}"
        );
        // One header implementation for the whole product, rendered from shell.rs.
        assert!(html.contains("v7-global-header"));
        assert!(html.contains("v7-context-row"));
        assert!(html.contains("data-readout=\"COLLECTION\""));
    }
}

#[test]
fn every_surface_reclaims_its_header_instead_of_restating_its_own_name() {
    // DESIGN-003: breadcrumb and rail already name the page, so the h1 survives only for
    // assistive tech and the vertical space returns to the content. A page that grows a
    // visible title block back is restating its name for the third time.
    let mut pages = vec![evidence_library_html(None)];
    for section in [
        collection::Section::Targets,
        collection::Section::Operations,
        collection::Section::Attention,
        collection::Section::Tasks,
        collection::Section::Runtime,
    ] {
        pages.push(collection::render(
            section,
            collection::OperationsMode::Now,
            None,
            None,
            None,
        ));
    }

    for html in &pages {
        assert!(
            html.contains("<h1 class=\"v7-sr-only\" id=\"page-title\">"),
            "the page title must survive as the visually hidden h1"
        );
        for restated in ["c-title", "c-eyebrow", "c-head", "v7-title", "v7-eyebrow"] {
            assert!(
                !html.contains(restated),
                "reclaimed header must not return as {restated}"
            );
        }
    }

    // The counts the title block used to carry now read from the context row, and each one
    // stays a separate figure rather than a single invented total.
    let targets = collection::render(
        collection::Section::Targets,
        collection::OperationsMode::Now,
        None,
        None,
        None,
    );
    let context_row = targets
        .split_once("v7-context-meta")
        .expect("collection pages render the context row")
        .1;
    for kpi in [
        "<span class=\"v7-kpi\"><em>巡检已开</em><b><span class=\"v7-status-main\">未知</span><small class=\"v7-tech-key\">UNKNOWN</small></b></span>",
        "<span class=\"v7-kpi\"><em>建档中</em><b><span class=\"v7-status-main\">未知</span><small class=\"v7-tech-key\">UNKNOWN</small></b></span>",
    ] {
        assert!(
            context_row.contains(kpi),
            "count missing from context row: {kpi}"
        );
    }
}

#[test]
fn connected_collection_surfaces_render_real_targets_and_scheduler_state() {
    let running = collection::SurfaceState {
        vacant_stations: Some(0),
        unclaimed_installations: Some(1),
        total_targets: Some(2),
        monitoring_targets: Some(1),
        archiving_targets: Some(1),
        scheduler_state: collection::SchedulerState::Running,
    };
    let counts = linggan_evidence::TargetCounts {
        total: 2,
        creator: 1,
        keyword: 1,
        archiving: 1,
        monitoring: 1,
    };
    let targets = collection::render(
        collection::Section::Targets,
        collection::OperationsMode::Now,
        None,
        Some(&counts),
        Some(&running),
    );

    assert!(targets.contains("调度运行中"));
    assert!(targets.contains("SCHEDULER RUNNING"));
    assert!(targets.contains("<em>巡检已开</em><b>1</b>"));
    assert!(targets.contains("<em>建档中</em><b>1</b>"));
    assert!(targets.contains("<span class=\"v7-nav-state\">观察中</span>"));
    assert!(!targets.contains("调度器未接通"));
    assert!(!targets.contains("<span class=\"v7-status-main\">暂无观察目标</span>"));

    let operations = collection::render(
        collection::Section::Operations,
        collection::OperationsMode::Now,
        None,
        None,
        Some(&running),
    );
    assert!(operations.contains("调度运行中"));
    assert!(!operations.contains("调度器未接通"));
}

#[test]
fn scheduler_stale_and_unreadable_remain_distinct_facts() {
    let stale = collection::SurfaceState {
        vacant_stations: None,
        unclaimed_installations: None,
        total_targets: None,
        monitoring_targets: None,
        archiving_targets: None,
        scheduler_state: collection::SchedulerState::Stale,
    };
    let stale_html = collection::render(
        collection::Section::Runtime,
        collection::OperationsMode::Now,
        None,
        None,
        Some(&stale),
    );
    assert!(stale_html.contains("调度心跳已过期"));
    assert!(stale_html.contains("SCHEDULER STALE"));
    assert!(stale_html.contains("<span class=\"v7-nav-state\">状态未知</span>"));

    let unreadable = collection::SurfaceState {
        scheduler_state: collection::SchedulerState::Unreadable,
        ..stale
    };
    let unreadable_html = collection::render(
        collection::Section::Runtime,
        collection::OperationsMode::Now,
        None,
        None,
        Some(&unreadable),
    );
    assert!(unreadable_html.contains("调度心跳读不到"));
    assert!(unreadable_html.contains("SCHEDULER HEARTBEAT UNREADABLE"));
}

#[test]
fn corpus_header_can_reflect_the_connected_collection_read_model() {
    let html = evidence_library_html(Some("观察中"));
    let collection_entry = html
        .split_once("v7-tech-key\">COLLECTION")
        .expect("the collection primary entry exists")
        .1;
    assert!(collection_entry.contains("<span class=\"v7-nav-state\">观察中</span>"));
}

#[test]
fn corpus_header_can_preserve_an_unreadable_collection_state() {
    let html = evidence_library_html(Some("状态未知"));
    let collection_entry = html
        .split_once("v7-tech-key\">COLLECTION")
        .expect("the collection primary entry exists")
        .1;
    assert!(collection_entry.contains("<span class=\"v7-nav-state\">状态未知</span>"));
}

#[test]
fn context_row_counts_read_in_chinese_so_one_row_holds_one_english_scale() {
    // `.v7-kpi em` is the 11px Sans reading slot; the system words beside it are 9px Mono
    // caps. An English label in the Sans slot puts two English scales in one row, which is
    // exactly what the Corpus surface does not do. Every count label must read in Chinese.
    let mut labels = Vec::new();
    for section in [
        collection::Section::Targets,
        collection::Section::Operations,
        collection::Section::Attention,
        collection::Section::Tasks,
        collection::Section::Runtime,
    ] {
        let html = collection::render(section, collection::OperationsMode::Now, None, None, None);
        for fragment in html.split("<span class=\"v7-kpi\"><em>").skip(1) {
            labels.push(
                fragment
                    .split_once("</em>")
                    .expect("a kpi label closes its em")
                    .0
                    .to_owned(),
            );
        }
    }
    assert_eq!(labels.len(), 10, "each surface carries two counts");

    for label in &labels {
        assert!(
            label
                .chars()
                .any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)),
            "count label must read in Chinese, not in the Mono slot's English: {label}"
        );
    }
}

#[test]
fn the_selected_rail_entry_keeps_its_inked_block_under_the_pointer() {
    // Hover invites you to leave the page you are on; it must never repaint the entry that
    // marks where you already are. The selected block is only readable while it stays inked.
    let shell = SHELL_CSS;
    for rule in [
        ".v7-side-nav:not([disabled]):not([aria-current=\"page\"]):hover",
        ".v7-primary-nav a:not([aria-current=\"page\"]):hover",
    ] {
        assert!(
            shell.contains(rule),
            "hover state must exclude the current page: {rule}"
        );
    }
    assert!(
        !shell.contains(".v7-side-nav:not([disabled]):hover {"),
        "the unqualified rail hover rule outranks the selected state and must not return"
    );
}

#[test]
fn the_primary_nav_readouts_all_share_one_type_scale_and_one_colour() {
    // At 8px a colour change reads as a size change, so darkening the current entry's readout
    // makes one shared header look like it carries two type standards. The current entry is
    // marked by the signal rule beneath it; the readouts stay one row of one scale.
    assert!(
        !SHELL_CSS.contains("[aria-current=\"page\"] .v7-nav-readout"),
        "the current entry must not restyle its readout: the signal rule already marks it"
    );

    let readout_declarations = SHELL_CSS
        .match_indices(".v7-nav-readout")
        .filter(|(index, _)| SHELL_CSS[*index..].starts_with(".v7-nav-readout {"))
        .count();
    assert_eq!(
        readout_declarations, 1,
        "one readout scale means exactly one rule owns it"
    );

    // Both surfaces render the same five entries from the one shared header.
    let mut pages = vec![evidence_library_html(None)];
    pages.push(collection::render(
        collection::Section::Targets,
        collection::OperationsMode::Now,
        None,
        None,
        None,
    ));
    for html in &pages {
        assert_eq!(
            html.matches("class=\"v7-nav-readout\"").count(),
            5,
            "every page renders the same five primary entries"
        );
    }
}

#[test]
fn collection_never_publishes_prototype_material_or_a_fake_zero() {
    let surfaces = [
        collection::Section::Targets,
        collection::Section::Operations,
        collection::Section::Attention,
        collection::Section::Tasks,
        collection::Section::Runtime,
    ];
    // Figures lifted straight from the V4 Gold Master's mock data. None of them may reach a
    // real route: the page has no observation targets, tasks, workers or events at all.
    let fabricated = [
        "146",
        "07/08",
        "@小北妈妈",
        "ADHD 作业拖延",
        "T-CR-019",
        "W06",
        "6.2K",
        "8.3K",
    ];
    for section in surfaces {
        for mode in [
            collection::OperationsMode::Now,
            collection::OperationsMode::Trace,
            collection::OperationsMode::Review,
        ] {
            let html = collection::render(section, mode, None, None, None);
            for figure in fabricated {
                assert!(
                    !html.contains(figure),
                    "prototype material leaked into a real route: {figure}"
                );
            }
            // Unknown must never be flattened into a confirmed zero.
            assert!(!html.contains(">0<"));
        }
    }
}

#[test]
fn collection_states_why_each_surface_is_empty_rather_than_looking_broken() {
    let confirmed_empty = collection::SurfaceState {
        vacant_stations: None,
        unclaimed_installations: None,
        total_targets: Some(0),
        monitoring_targets: Some(0),
        archiving_targets: Some(0),
        scheduler_state: collection::SchedulerState::Unreadable,
    };
    let targets = collection::render(
        collection::Section::Targets,
        collection::OperationsMode::Now,
        None,
        None,
        Some(&confirmed_empty),
    );
    // 加入观察只写本机记录，因此它是真实可点的动作；但页面必须把「加进来」与「开始采集」
    // 分清楚，否则会让人以为点一下就开始采了。
    assert!(targets.contains("不访问任何平台"));
    assert!(targets.contains("/collection/targets/new"));
    assert!(targets.contains("还没有观察目标"));
    // 真正消耗平台访问的那一步仍然要走完整条链并由人开闸。
    assert!(targets.contains("最后还要人开闸"));

    let unknown_targets = collection::render(
        collection::Section::Targets,
        collection::OperationsMode::Now,
        None,
        None,
        None,
    );
    assert!(unknown_targets.contains("观察目标当前未知"));
    assert!(!unknown_targets.contains("还没有观察目标"));
    assert!(!unknown_targets.contains("本机采集运行时已接通"));
    assert!(!unknown_targets.contains("SCHEDULER NOT CONNECTED"));
    assert!(!unknown_targets.contains("NO OBSERVATION TARGETS"));
    assert!(unknown_targets.contains("<span class=\"v7-nav-state\">状态未知</span>"));

    let attention = collection::render(
        collection::Section::Attention,
        collection::OperationsMode::Now,
        None,
        None,
        None,
    );
    assert!(attention.contains("待处理状态当前未知"));
    assert!(attention.contains("待处理读模型尚未接入"));
    assert!(!attention.contains("没有待处理事项"));

    let tasks = collection::render(
        collection::Section::Tasks,
        collection::OperationsMode::Now,
        None,
        None,
        None,
    );
    assert!(tasks.contains("采集任务当前未知"));
    assert!(tasks.contains("观察目标数量不能排除历史或在途任务"));
    assert!(!tasks.contains("没有采集任务"));

    let runtime = collection::render(
        collection::Section::Runtime,
        collection::OperationsMode::Now,
        None,
        None,
        None,
    );
    assert!(runtime.contains("执行工位状态当前未知"));
    assert!(runtime.contains("采集状态未知"));
    assert!(!runtime.contains("调度器未接通"));
}

#[test]
fn operations_modes_are_addressable_and_the_stream_stays_honest() {
    let now = collection::render(
        collection::Section::Operations,
        collection::OperationsMode::Now,
        None,
        None,
        None,
    );
    assert!(now.contains("LIVE OBSERVATION"));
    assert!(now.contains("采集状态读不到"));
    assert!(now.contains("COLLECTION STATE UNAVAILABLE"));
    assert!(!now.contains("SCHEDULER NOT CONNECTED"));
    // The prototype invented an event every seven seconds. Production must not.
    assert!(now.contains("不会用计时器伪造事件"));
    assert!(now.contains("暂停只停止画面跟随，永远不会暂停真实的采集调度"));

    let trace = collection::render(
        collection::Section::Operations,
        collection::OperationsMode::Trace,
        None,
        None,
        None,
    );
    assert!(trace.contains("观察历史当前未知"));
    assert!(!trace.contains("没有可回放的观察历史"));
    let review = collection::render(
        collection::Section::Operations,
        collection::OperationsMode::Review,
        None,
        None,
        None,
    );
    assert!(review.contains("周期复盘当前未知"));
    assert!(!review.contains("没有可复盘的周期"));
    assert!(review.contains("观察盲区"));

    assert!(now.contains("href=\"/collection/operations?mode=trace\""));
}

#[test]
fn target_drawer_is_owned_by_the_url_and_escapes_its_identifier() {
    let closed = collection::render(
        collection::Section::Targets,
        collection::OperationsMode::Now,
        None,
        None,
        None,
    );
    assert!(!closed.contains("c-drawer"));

    let open = collection::render_unreadable_target_drawer(Some("T-CR-019"));
    assert!(open.contains("id=\"c-drawer\""));
    assert!(open.contains("#T-CR-019"));
    assert!(open.contains("观察目标读取状态当前未知"));
    for false_empty in [
        "未找到该观察目标",
        "没有建档基线",
        "没有巡逻策略",
        "没有关联证据",
        "没有观察史",
    ] {
        assert!(!open.contains(false_empty));
    }

    let injected = collection::render_unreadable_target_drawer(Some("<script>alert(1)</script>"));
    assert!(!injected.contains("<script>alert(1)</script>"));
    assert!(injected.contains("&lt;script&gt;"));
}

#[test]
fn collection_stylesheet_authors_no_colour_of_its_own() {
    let sheet = format!("{SHELL_CSS}\n{COLLECTION_WORKSPACE_CSS}");
    assert!(
        !sheet.contains('#'),
        "page CSS must not author raw colour: every value resolves through a LIDS token"
    );
    assert!(declared_token_values(COLLECTION_WORKSPACE_CSS).is_empty());
    // The dark stream is the one approved exception and it lives in the token source.
    assert!(LIDS_TOKENS.contains("--lgi-stream-bg:"));
    assert!(COLLECTION_WORKSPACE_CSS.contains("var(--lgi-stream-bg)"));
}

/// The stylesheet the Evidence Library route actually serves: shared shell first, then the
/// page layer. Assertions run against this so moving a rule between the two files cannot
/// silently drop it from the page.
fn evidence_page_stylesheet() -> String {
    format!("{SHELL_CSS}\n{EVIDENCE_LIBRARY_CSS}")
}

fn runtime_producer_task_spec() -> String {
    r#"{"contractVersion":"linggan.producer.task-spec.v1","taskId":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa","source":"manual","platform":"xhs","pageType":"note_detail","target":{"contentExternalId":"note-a"},"capabilitiesRequested":["media_slots"],"maximumQuota":1,"commentLimit":"not_requested","acquireMedia":"slots","riskPolicy":"local_trusted_user_initiated","stopConditions":["manual_stop","maximum_quota"]}"#.to_owned()
}

fn runtime_producer_attempt() -> String {
    r#"{"contractVersion":"linggan.producer.attempt.v1","producerInstanceId":"bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb","taskId":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa","attemptId":"cccccccc-cccc-4ccc-8ccc-cccccccccccc"}"#.to_owned()
}

fn runtime_producer_submission() -> String {
    r#"{"contractVersion":"linggan.producer.capture-package.v1","producerInstanceId":"bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb","taskId":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa","attemptId":"cccccccc-cccc-4ccc-8ccc-cccccccccccc","submissionId":"dddddddd-dddd-4ddd-8ddd-dddddddddddd","capturePackage":{"contractVersion":"linggan.producer.capture-package.v1","packageRef":"eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee","packageKind":"media_slots","platform":"xhs","observedAt":"2026-08-25T00:00:00Z","capturedAt":"2026-08-25T00:00:01Z","coverage":{"target":{"basis":"known_set","contentExternalId":"note-a"},"layers":[{"capability":"media_slots","observed":2,"attempted":2,"acquired":0,"verified":0,"failed":0,"notAttempted":0,"unknown":0,"stoppedReason":"media_acquisition_not_started"}]},"records":[{"kind":"media_slot","slotKey":"xhs:note-a:image:1","slot":{"role":"image","ordinal":1},"sourceObject":{"externalId":"note-a"},"observation":{"externalUri":"https://fixture.invalid/one.jpg"},"observationRef":"ffffffff-ffff-4fff-8fff-ffffffffffff"},{"kind":"media_slot","slotKey":"xhs:note-a:image:2","slot":{"role":"image","ordinal":2},"sourceObject":{"externalId":"note-a"},"observation":{"externalUri":"https://fixture.invalid/two.jpg"},"observationRef":"11111111-2222-4333-8444-555555555555"}]}}"#.to_owned()
}

fn runtime_author_producer_task_spec() -> String {
    r#"{"contractVersion":"linggan.producer.task-spec.v1","taskId":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaab","source":"manual","platform":"xhs","pageType":"profile","target":{"authorExternalId":"author-route-1"},"capabilitiesRequested":["author_profile"],"maximumQuota":1,"commentLimit":"not_requested","acquireMedia":"not_requested","riskPolicy":"local_trusted_user_initiated","stopConditions":["manual_stop","maximum_quota"]}"#.to_owned()
}

fn runtime_author_producer_attempt() -> String {
    r#"{"contractVersion":"linggan.producer.attempt.v1","producerInstanceId":"bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbc","taskId":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaab","attemptId":"cccccccc-cccc-4ccc-8ccc-cccccccccccd"}"#.to_owned()
}

fn runtime_author_producer_submission() -> String {
    r#"{"contractVersion":"linggan.producer.capture-package.v1","producerInstanceId":"bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbc","taskId":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaab","attemptId":"cccccccc-cccc-4ccc-8ccc-cccccccccccd","submissionId":"dddddddd-dddd-4ddd-8ddd-ddddddddddde","capturePackage":{"contractVersion":"linggan.producer.capture-package.v1","packageRef":"eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeef","packageKind":"author_profile","platform":"xhs","observedAt":"2026-09-02T10:00:00Z","capturedAt":"2026-09-02T10:00:01Z","coverage":{"target":{"basis":"known_set","authorExternalId":"author-route-1"},"layers":[{"capability":"author_profile","observed":1,"attempted":1,"acquired":1,"verified":0,"failed":0,"notAttempted":0,"unknown":0,"stoppedReason":"profile_read_complete"}]},"records":[{"kind":"author_profile","sourceObject":{"platform":"xhs","type":"author","externalId":"author-route-1"},"payload":{"userId":"author-route-1","name":"路由作者","avatar":"https://media.example/author-route-1.jpg","description":"从已接纳包建立观察目标"}}]}}"#.to_owned()
}

fn declared_token_values(stylesheet: &str) -> BTreeMap<&str, &str> {
    stylesheet
        .lines()
        .filter_map(|line| line.trim().strip_prefix("--lgi-"))
        .filter_map(|line| {
            line.split_once(':')
                .map(|(name, value)| (name, value.trim().trim_end_matches(';')))
        })
        .collect()
}

#[test]
fn served_primary_surfaces_link_to_each_other_and_unserved_ones_stay_disabled() {
    // A responsibility whose route this binary actually serves must be reachable from every
    // other served surface. An operator standing on either page can always leave it.
    let evidence = evidence_library_html(None);
    let collection = collection::render(
        collection::Section::Targets,
        collection::OperationsMode::Now,
        None,
        None,
        None,
    );

    for (html, page, own_href, other_hrefs) in [
        (&evidence, "corpus", "/corpus", ["/collection", "/topics"]),
        (
            &collection,
            "collection",
            "/collection",
            ["/corpus", "/topics"],
        ),
    ] {
        assert!(
            html.contains(&format!("<a href=\"{own_href}\" aria-current=\"page\">")),
            "{page} must mark its own primary entry as the current page"
        );
        for other_href in other_hrefs {
            assert!(
                html.contains(&format!("<a href=\"{other_href}\">")),
                "{page} must offer a working link to {other_href}"
            );
        }

        // Naming a responsibility is not the same as serving it: unconnected entries
        // stay disabled buttons rather than becoming dead links.
        for unserved in ["雷达", "洞察"] {
            assert!(
                html.contains(&format!(
                    "<button disabled aria-disabled=\"true\"><b class=\"v7-nav-zh\">{unserved}</b>"
                )),
                "{page} must keep the unconnected {unserved} entry disabled"
            );
        }
    }
}

/// Every class the shell declares. `shell.css` styles what `shell.rs` renders for all
/// pages, so these names are the shell's to own.
fn shell_owned_classes() -> std::collections::BTreeSet<String> {
    let mut classes = std::collections::BTreeSet::new();
    let mut rest = SHELL_CSS;
    while let Some(start) = rest.find(".v7-") {
        rest = &rest[start + 1..];
        let end = rest
            .find(|c: char| !c.is_ascii_alphanumeric() && c != '-')
            .unwrap_or(rest.len());
        classes.insert(rest[..end].to_owned());
    }
    classes
}

/// Selectors a page stylesheet declares, including the ones inside `@media` blocks.
fn declared_selectors(stylesheet: &str) -> Vec<String> {
    let mut selectors = Vec::new();
    let mut cursor = stylesheet;
    while let Some(brace) = cursor.find('{') {
        let head = cursor[..brace].trim();
        cursor = &cursor[brace + 1..];
        // An @media head opens a nested block; its inner selectors are read on the next pass.
        let selector = head.rsplit(['}', '\n']).next().unwrap_or(head).trim();
        if selector.is_empty() || selector.starts_with('@') || selector.starts_with("/*") {
            continue;
        }
        selectors.push(selector.to_owned());
    }
    selectors
}

#[test]
fn no_page_stylesheet_restyles_a_component_the_shell_owns() {
    // One header, one implementation. While the primary nav's typography lived in the
    // Evidence Library stylesheet, Corpus rendered its readouts at 11px and Collection at
    // 8px from the very same markup — one shared header wearing two type standards. A page
    // stylesheet may style its own components; the shell's are not its to restyle.
    let shell_classes = shell_owned_classes();

    for (page, stylesheet) in [
        ("evidence_library.css", EVIDENCE_LIBRARY_CSS),
        ("collection_workspace.css", COLLECTION_WORKSPACE_CSS),
    ] {
        let mut violations = Vec::new();
        for selector in declared_selectors(stylesheet) {
            // Only the parts that redefine the component itself count. A page may position a
            // shell element inside its own component (".v7-fact-strap .v7-tech-key") — that
            // is the page styling its own context, not restyling the shell. What it may not
            // do is redeclare the component: a second base definition means the same element
            // renders differently depending on which page loaded last.
            let redefines_component = selector.split(',').any(|part| {
                let part = part.trim();
                shell_classes.iter().any(|class| {
                    let needle = format!(".{class}");
                    part.starts_with(&needle)
                        && part[needle.len()..]
                            .chars()
                            .next()
                            .is_none_or(|c| !c.is_ascii_alphanumeric() && c != '-')
                })
            });
            if redefines_component {
                violations.push(selector);
            }
        }
        assert!(
            violations.is_empty(),
            "{page} restyles shell-owned components; move these rules into shell.css:\n  {}",
            violations.join("\n  ")
        );
    }
}

#[test]
fn both_surfaces_render_the_header_at_one_type_scale() {
    // The regression this locks: same markup, same stylesheet layer, same declarations.
    let corpus = evidence_library_html(None);
    let collection = collection::render(
        collection::Section::Targets,
        collection::OperationsMode::Now,
        None,
        None,
        None,
    );

    // Neither page may carry its own copy of the nav typography.
    for (page, stylesheet) in [
        ("evidence_library.css", EVIDENCE_LIBRARY_CSS),
        ("collection_workspace.css", COLLECTION_WORKSPACE_CSS),
    ] {
        for owned in [".v7-nav-readout", ".v7-nav-zh", ".v7-primary-nav"] {
            assert!(
                !stylesheet.contains(owned),
                "{page} must not declare {owned}: the shell renders the primary nav"
            );
        }
    }

    // And both pages must actually emit that markup, so the shared rule reaches both.
    for (page, html) in [("corpus", &corpus), ("collection", &collection)] {
        assert_eq!(
            html.matches("class=\"v7-nav-zh\"").count(),
            5,
            "{page} renders five primary entries"
        );
        assert_eq!(
            html.matches("class=\"v7-nav-readout\"").count(),
            5,
            "{page} renders five primary readouts"
        );
    }
}

#[test]
fn narrow_primary_navigation_wraps_instead_of_hiding_a_responsibility_offscreen() {
    // Topic made the fifth first-level responsibility reachable. The prior narrow-shell
    // rule still kept every item at 86px inside a horizontally scrolling strip, so a 390px
    // viewport clipped Collection. This regression guard keeps visibility a shell property:
    // five entries share one row at 390px, while narrower widths or future entries wrap.
    assert!(
        SHELL_CSS.contains("grid-template-columns:repeat(auto-fit,minmax(72px,1fr))"),
        "narrow primary navigation must distribute entries across the available width"
    );
    assert!(
        SHELL_CSS.contains(".v7-primary-nav{grid-column:1/-1;height:auto;display:grid"),
        "the narrow primary navigation must become a grid rather than stay a horizontal strip"
    );
    assert!(
        SHELL_CSS.contains(
            ".v7-global-row{display:grid;grid-template-columns:minmax(0,1fr) auto;overflow:visible}"
        ),
        "the narrow shared row must not retain the parent horizontal scroller"
    );
    assert!(
        SHELL_CSS.contains(
            ".v7-primary-nav button,.v7-primary-nav a{width:100%;min-width:0;min-height:58px"
        ),
        "each narrow navigation entry must shrink within its grid cell while keeping a usable target"
    );
    assert!(
        SHELL_CSS.contains(".v7-primary-nav .v7-tech-key{display:none}"),
        "narrow navigation may remove only the technical annotation, never the Chinese responsibility or state"
    );
    assert!(
        !SHELL_CSS.contains(".v7-primary-nav{grid-column:1/-1;overflow-x:auto"),
        "a hidden horizontal swipe must not be required to reach a first-level responsibility"
    );
}

#[test]
fn the_larger_hard_shadow_only_marks_hover_displacement() {
    // Two resting depths would be two standards. --v7-brutal-lg exists for the 2px hover
    // displacement that DESIGN-003 permits; anything that sits still wears --v7-brutal.
    for (name, stylesheet) in [
        ("shell.css", SHELL_CSS),
        ("evidence_library.css", EVIDENCE_LIBRARY_CSS),
        ("collection_workspace.css", COLLECTION_WORKSPACE_CSS),
    ] {
        for (index, _) in stylesheet.match_indices("var(--v7-brutal-lg)") {
            let selector = stylesheet[..index]
                .rfind('}')
                .map(|end| &stylesheet[end + 1..index])
                .unwrap_or(&stylesheet[..index]);
            assert!(
                selector.contains(":hover") || selector.contains(":active"),
                "{name}: the larger hard shadow is for hover displacement, but it is applied at rest here:\n  {}",
                selector.trim().lines().last().unwrap_or(selector).trim()
            );
        }
    }
}

#[test]
fn collection_orders_its_surfaces_by_urgency_and_opens_on_the_one_that_expires() {
    // DESIGN-006 Q7. Attention is the only surface whose contents go stale; the other four
    // read the same next week. The Collection Control pattern already required NEEDS
    // ATTENTION first — the pipeline order this page shipped with never matched it.
    let html = collection::render(
        collection::Section::Attention,
        collection::OperationsMode::Now,
        None,
        None,
        None,
    );

    // Search inside the rail only: the global header's Collection entry is also a
    // /collection/* link and it renders before the rail.
    let rail = html
        .split_once("aria-label=\"采集导航\"")
        .expect("collection renders its rail")
        .1
        .split_once("</aside>")
        .expect("the rail closes")
        .0;

    let mut positions = Vec::new();
    for (index, slug) in [
        ("01", "attention"),
        ("02", "targets"),
        ("03", "operations"),
        ("04", "tasks"),
        ("05", "runtime"),
    ] {
        let entry = format!("href=\"/collection/{slug}\"");
        let at = rail
            .find(&entry)
            .unwrap_or_else(|| panic!("rail is missing {slug}"));
        assert!(
            rail[at..].contains(&format!("<i>{index}</i>")),
            "{slug} must be numbered {index}"
        );
        positions.push(at);
    }
    assert!(
        positions.windows(2).all(|pair| pair[0] < pair[1]),
        "rail order must follow urgency, not pipeline stage"
    );
}

#[test]
fn every_empty_surface_says_whether_it_is_waiting_on_you() {
    // DESIGN-006 Q9. Five empty surfaces, and only one confirmed-empty surface is the
    // reader's to act on. Rendered at equal weight they answered everything except "so what
    // do I do".
    let surface =
        |section| collection::render(section, collection::OperationsMode::Now, None, None, None);

    // Exactly one surface may claim the reader's attention, and it must name the action.
    let confirmed_empty = collection::SurfaceState {
        vacant_stations: None,
        unclaimed_installations: None,
        total_targets: Some(0),
        monitoring_targets: Some(0),
        archiving_targets: Some(0),
        scheduler_state: collection::SchedulerState::Unreadable,
    };
    let targets = collection::render(
        collection::Section::Targets,
        collection::OperationsMode::Now,
        None,
        None,
        Some(&confirmed_empty),
    );
    assert!(targets.contains("c-empty-you"));
    assert!(targets.contains("c-empty-action"));

    for (name, html) in [
        ("attention", surface(collection::Section::Attention)),
        ("tasks", surface(collection::Section::Tasks)),
        ("operations", surface(collection::Section::Operations)),
        ("runtime", surface(collection::Section::Runtime)),
    ] {
        assert!(
            !html.contains("c-empty-you"),
            "{name} must not claim the reader's attention: only one surface is waiting on them"
        );
    }

    // Surfaces waiting on engineering must say so, so nobody hunts for an action.
    // Operations' default mode is not an empty state — it renders the pipeline itself — so
    // its awaiting-engineering wording lives on the two modes that are empty.
    for (name, html) in [
        ("attention", surface(collection::Section::Attention)),
        ("tasks", surface(collection::Section::Tasks)),
        (
            "operations/trace",
            collection::render(
                collection::Section::Operations,
                collection::OperationsMode::Trace,
                None,
                None,
                None,
            ),
        ),
        (
            "operations/review",
            collection::render(
                collection::Section::Operations,
                collection::OperationsMode::Review,
                None,
                None,
                None,
            ),
        ),
        ("runtime", surface(collection::Section::Runtime)),
    ] {
        assert!(
            html.contains("c-empty-engineering"),
            "{name} must identify the missing engineering read model"
        );
    }
}

#[test]
fn structure_survives_without_data_but_placeholder_counters_do_not() {
    // DESIGN-006 Q11. The six stage names are the domain model and carry information; a
    // counter reading UNKNOWN six times over carries none, and teaches the eye to skip the
    // one word on this page that must never become invisible.
    let operations = collection::render(
        collection::Section::Operations,
        collection::OperationsMode::Now,
        None,
        None,
        None,
    );

    for stage in [
        "目标接入",
        "首次建档",
        "日常巡逻",
        "触发式深采",
        "事实资产保留",
        "恢复与重排",
    ] {
        assert!(
            operations.contains(stage),
            "stage name must survive: {stage}"
        );
    }
    for retired in ["c-flow-metrics", "阶段计数", "c-stream-readout"] {
        assert!(
            !operations.contains(retired),
            "counter cells must wait for data that exists: {retired}"
        );
    }

    // The reason is still on the page — once, in the shared context row.
    assert!(operations.contains("采集状态读不到"));
    assert!(operations.contains("COLLECTION STATE UNAVAILABLE"));
    assert!(operations.contains("来源读不到"));
    assert!(operations.contains("SOURCE UNREADABLE"));
    assert!(!operations.contains("COLLECTION STATE <span"));
    assert!(!operations.contains("SOURCE <span"));
    assert!(!operations.contains("SCHEDULER NOT CONNECTED"));

    // Filter tabs keep their names and lose their placeholder counts.
    let targets = collection::render(
        collection::Section::Targets,
        collection::OperationsMode::Now,
        None,
        None,
        None,
    );
    for tab in ["创作者", "关键词", "建档中", "巡逻中"] {
        assert!(targets.contains(tab), "filter tab must survive: {tab}");
    }
    assert!(!targets.contains("<small>UNKNOWN</small>"));
}

#[test]
fn the_primary_nav_links_to_entry_routes_never_to_a_sub_surface() {
    // The default surface of a responsibility is decided in exactly one place: its entry
    // route. A header that links straight to a sub-surface is a second copy of that
    // decision, and the two drift the first time the default moves — which is exactly what
    // happened when Collection's default became /collection/attention while the header
    // still pointed at /collection/targets.
    let html = evidence_library_html(None);
    let nav = html
        .split_once("v7-primary-nav")
        .expect("every page renders the primary nav")
        .1
        .split_once("</nav>")
        .expect("the nav closes")
        .0;

    for href in nav.split("href=\"").skip(1) {
        let href = href.split('"').next().expect("href closes");
        assert_eq!(
            href.matches('/').count(),
            1,
            "primary nav must link to an entry route, not a sub-surface: {href}"
        );
    }

    // And both served responsibilities must actually be reachable that way.
    for entry in [
        "href=\"/corpus\"",
        "href=\"/collection\"",
        "href=\"/topics\"",
    ] {
        assert!(nav.contains(entry), "primary nav is missing {entry}");
    }
}

#[test]
fn evidence_runtime_uses_material_projection_as_its_only_default_read_source() {
    let html = evidence_library_html(None);

    assert!(html.contains("/assets/evidence-observation.js"));
    assert!(html.contains("/assets/evidence-library.js"));
    assert!(html.contains("id=\"ev-work-list\""));
    assert!(html.contains("data-ev-panel=\"trace\""));
    for layout in ["research", "table", "cover"] {
        assert!(html.contains(&format!("data-ev-layout=\"{layout}\"")));
    }
    assert!(EVIDENCE_LIBRARY_JS.contains("const API_ROOT = '/api/local/work-resources'"));
    assert!(EVIDENCE_LIBRARY_JS.contains("params.set('layout', model.activeLayout)"));
    assert!(EVIDENCE_LIBRARY_JS.contains("params.set('view', model.activeView)"));
    assert!(EVIDENCE_OBSERVATION_JS.contains("createController"));
    assert!(EVIDENCE_OBSERVATION_JS.contains("立即复观测"));
    assert!(EVIDENCE_OBSERVATION_JS.contains("TARGET-LINKED AUTHORIZATION"));
    assert!(EVIDENCE_OBSERVATION_JS.contains("commentsCoverageHistory"));
    assert!(EVIDENCE_LIBRARY_JS.contains("LATEST KNOWN PER METRIC"));
    assert!(EVIDENCE_LIBRARY_JS.contains("refreshSelectedDetailAfterReobservation"));
    assert!(EVIDENCE_OBSERVATION_JS.contains("await onTerminal?.()"));
    assert!(EVIDENCE_LIBRARY_JS.contains("readJson(detailUrl, model.detailController.signal)"));
    assert!(EVIDENCE_OBSERVATION_JS.contains("已合并到覆盖当前作品的在途工作"));
    // V9 owns the reading surface, media rail, lightbox and filter mechanics; Issue #133 must
    // keep its operation state in the dedicated module instead of growing that shared shell.
    assert!(EVIDENCE_OBSERVATION_JS.contains("function reobservationOperationBlock"));
    assert!(!EVIDENCE_LIBRARY_JS.contains("reobservationOperation"));
    assert!(EVIDENCE_LIBRARY_CSS.contains(".ev-work-list[data-layout=\"cover\"]"));
    assert!(EVIDENCE_LIBRARY_CSS.contains(".ev-work-list[data-layout=\"table\"]"));
    assert!(!EVIDENCE_LIBRARY_JS.contains("/api/local/evidence-library/legacy"));
    assert!(!EVIDENCE_LIBRARY_JS.contains("fetch('http"));
}

#[test]
fn evidence_runtime_restores_system_and_personal_view_strategy_without_faking_saved_views() {
    let html = evidence_library_html(None);

    assert!(html.contains("aria-label=\"按材料状态快速筛选\""));
    assert!(html.contains("SYSTEM VIEWS"));
    assert!(html.contains("系统视图"));
    assert!(html.contains("MY VIEWS"));
    assert!(html.contains("我的视图"));
    assert!(html.contains("暂无已保存视图"));
    assert!(html.contains("SAVED VIEWS NOT CONNECTED"));
    assert!(html.contains("data-ev-view=\"partial\""));
    assert!(html.contains("data-ev-view=\"risk\""));
    assert!(html.contains("data-ev-view=\"cleaned\""));
    assert!(html.contains("data-ev-view=\"restricted\""));
    assert!(!html.contains("data-ev-saved-view"));
    assert!(!html.contains("保存当前视图"));
    assert!(!html.contains("状态视图"));
    assert!(!html.contains("以作品为顶层的多材料证据库"));
    assert!(!html.contains("作品级材料集合"));
    assert!(!html.contains("多通道真实状态"));
    assert!(!html.contains("ev-context-count"));

    assert!(EVIDENCE_LIBRARY_JS.contains("partial: { laneState: 'PARTIAL' }"));
    assert!(EVIDENCE_LIBRARY_JS.contains("risk: { laneState: 'RISK_CONTROL' }"));
    assert!(
        EVIDENCE_LIBRARY_JS
            .contains("cleaned: { lane: 'media_bytes', laneState: 'BYTES_CLEANED' }")
    );
    assert!(EVIDENCE_LIBRARY_JS.contains("params.set('layout', model.activeLayout)"));
    assert!(EVIDENCE_LIBRARY_JS.contains("params.set('view', model.activeView)"));
    // The table layout drops the cover and reads as fixed high-density columns. EVIDENCE-V9-001
    // replaced the per-column classes with one cell primitive; what must survive is that the
    // table row is denser than the research row and still names its work and its context.
    assert!(EVIDENCE_LIBRARY_CSS.contains(".ev-table-cell"));
    assert!(EVIDENCE_LIBRARY_CSS.contains(".ev-work-list[data-layout=\"table\"] .ev-preview"));
    assert!(EVIDENCE_LIBRARY_JS.contains("function tableCell(primary, secondary)"));

    // The layout switch marks the current view with a Signal underline. It must never become a
    // filled black button: solid Ink is reserved for the one primary action on the surface, and
    // a black tab would read as the page's main control rather than as a view toggle.
    assert!(
        EVIDENCE_LIBRARY_CSS
            .contains(".ev-layout-switch button[aria-pressed=\"true\"]{border-bottom-color:var(--v7-red)")
            || EVIDENCE_LIBRARY_CSS.contains(
                ".ev-quickviews button[aria-pressed=\"true\"],.ev-layout-switch button[aria-pressed=\"true\"]{border-bottom-color:var(--v7-red)"
            )
    );
    assert!(
        !EVIDENCE_LIBRARY_CSS
            .contains(".ev-layout-switch button[aria-pressed=\"true\"]{background:var(--v7-black)")
    );
    assert!(EVIDENCE_LIBRARY_CSS.contains(".ev-layout-switch button:focus-visible"));
}

#[test]
fn evidence_runtime_renders_only_controlled_media_handles() {
    assert!(!EVIDENCE_LIBRARY_JS.contains("function observedCoverUrl"));
    assert!(!EVIDENCE_LIBRARY_JS.contains("observedSourceUrl"));
    assert!(EVIDENCE_LIBRARY_JS.contains("sameOriginPath(cover.localAssetUrl"));
    assert!(EVIDENCE_LIBRARY_JS.contains("renderMaterials(item, inspector, channels)"));
    assert!(!EVIDENCE_LIBRARY_JS.contains("item.preview?.localAssetUrl"));
    assert!(EVIDENCE_LIBRARY_JS.contains("node('img')"));
    assert!(EVIDENCE_LIBRARY_JS.contains("本地副本"));
    assert!(EVIDENCE_LIBRARY_CSS.contains(".ev-preview img"));
    assert!(EVIDENCE_LIBRARY_CSS.contains("aspect-ratio:3/4"));
}

#[test]
fn evidence_runtime_separates_creator_and_monitoring_target_and_renders_avatar_from_media() {
    assert!(EVIDENCE_LIBRARY_JS.contains("ev-creator-fact"));
    assert!(EVIDENCE_LIBRARY_JS.contains("ev-target-fact"));
    assert!(EVIDENCE_LIBRARY_JS.contains("作品作者"));
    assert!(EVIDENCE_LIBRARY_JS.contains("监控目标"));
    assert!(EVIDENCE_LIBRARY_JS.contains("media.avatar"));
    assert!(EVIDENCE_LIBRARY_JS.contains("sameOriginPath(avatar.localAssetUrl"));
    assert!(EVIDENCE_LIBRARY_CSS.contains(".ev-author-avatar"));
    assert!(!EVIDENCE_LIBRARY_JS.contains("identity_facts.avatar"));
}

#[test]
fn evidence_runtime_uses_the_xhs_portrait_cover_ratio_in_visual_layouts() {
    assert!(EVIDENCE_LIBRARY_JS.contains(
        "row.dataset.platform = String(item.identity?.platform || 'unknown').toLowerCase()"
    ));
    assert!(
        EVIDENCE_LIBRARY_CSS.contains(
            ".ev-work-row[data-platform=\"xhs\"] .ev-preview{height:auto;aspect-ratio:3/4}"
        )
    );
}

#[test]
fn evidence_runtime_preserves_unknown_partial_and_restricted_states() {
    for state in [
        "UNKNOWN",
        "SOURCE_TEXT_ONLY",
        "PARTIAL",
        "RISK_CONTROL",
        "BYTES_CLEANED",
        "WITHDRAWN_OR_RESTRICTED",
    ] {
        assert!(
            EVIDENCE_LIBRARY_JS.contains(state),
            "runtime must render honest state {state}"
        );
    }
    assert!(EVIDENCE_LIBRARY_JS.contains("数量未知"));
    assert!(EVIDENCE_LIBRARY_JS.contains("SOURCE INCOMPLETE"));
    assert!(EVIDENCE_LIBRARY_JS.contains("这不表示作品没有媒体"));
    assert!(!EVIDENCE_LIBRARY_JS.contains("ACK 完整"));
}

#[test]
fn evidence_runtime_keeps_sensitive_text_and_media_inside_controlled_detail_reads() {
    assert!(EVIDENCE_OBSERVATION_JS.contains("LOCAL AUTHORIZED RESEARCH"));
    assert!(EVIDENCE_OBSERVATION_JS.contains("IDENTITY WITHHELD"));
    assert!(EVIDENCE_OBSERVATION_JS.contains("comment.body"));
    assert!(!EVIDENCE_LIBRARY_JS.contains("authorExternalId"));
    assert!(!EVIDENCE_OBSERVATION_JS.contains("authorExternalId"));
    assert!(EVIDENCE_LIBRARY_JS.contains("blob?.deliveryState === 'INLINE_SAFE'"));
    assert!(EVIDENCE_LIBRARY_JS.contains("'/api/local/media/'"));
    assert!(EVIDENCE_LIBRARY_JS.contains("'/api/local/derivative/'"));
    assert!(!EVIDENCE_LIBRARY_JS.contains("innerHTML"));
}

#[test]
fn evidence_runtime_has_bounded_continuation_keyboard_and_mobile_contracts() {
    for marker in [
        "payload.nextCursor",
        "payload.truncated",
        "ArrowDown",
        "ArrowRight",
        "prefers-reduced-motion",
        "max-width:900px",
        "max-width:640px",
        "min-height:40px",
    ] {
        assert!(
            EVIDENCE_LIBRARY_JS.contains(marker)
                || EVIDENCE_OBSERVATION_JS.contains(marker)
                || EVIDENCE_LIBRARY_CSS.contains(marker),
            "runtime contract marker missing: {marker}"
        );
    }
}
