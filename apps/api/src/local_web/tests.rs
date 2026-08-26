use super::*;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use linggan_evidence::{DiscoveryLibraryCard, DiscoveryLibraryProjection};
use linggan_storage_postgres::testing::isolated_proof_schema;
use sqlx::Row;
use std::collections::BTreeMap;
use tower::ServiceExt;

const LOCAL_001_MIGRATIONS: &str = concat!(
    "CREATE TABLE linggan_local_schema_migration (\n",
    "  migration_id text PRIMARY KEY,\n",
    "  migration_sha256 text NOT NULL CHECK (migration_sha256 ~ '^[0-9a-f]{64}$'),\n",
    "  applied_at timestamptz NOT NULL DEFAULT clock_timestamp()\n",
    ");\n",
    include_str!("../../../../database/migrations/0001_scope_001_capture_evidence.sql"),
    "\n",
    include_str!("../../../../database/migrations/0002_local_001_discovery.sql"),
    "\n",
    include_str!("../../../../database/migrations/0003_local_trusted_producer.sql"),
    "\n",
    include_str!("../../../../database/migrations/0004_plugin_runtime_all_capabilities.sql"),
    "\n",
    "INSERT INTO linggan_local_schema_migration (migration_id, migration_sha256) VALUES\n",
    "('0001_scope_001_capture_evidence', '0000000000000000000000000000000000000000000000000000000000000001'),\n",
    "('0002_local_001_discovery', '0000000000000000000000000000000000000000000000000000000000000002'),\n",
    "('0003_local_trusted_producer', '0000000000000000000000000000000000000000000000000000000000000003'),\n",
    "('0004_plugin_runtime_all_capabilities', '0000000000000000000000000000000000000000000000000000000000000004');\n",
);

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
    assert!(evidence_library_html().contains("SOURCE_INCOMPLETE"));
    assert!(evidence_library_html().contains("没有可展示的本地材料"));
    assert!(!evidence_library_html().contains(&["SYSTEM", "LIVE"].join(" ")));
}

#[test]
fn evidence_page_does_not_replace_unknown_with_zero() {
    assert!(evidence_library_html().contains("COVERAGE <strong>UNKNOWN</strong>"));
    assert!(!evidence_library_html().contains("评论 0"));
}

#[test]
fn evidence_page_keeps_the_v7_shell_and_three_column_geometry() {
    let html = evidence_library_html();

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
fn evidence_page_has_no_fabricated_v7_runtime_material_or_actions() {
    let html = evidence_library_html();

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
    assert!(html.contains("NO_ACCEPTED_MATERIAL_AVAILABLE"));
}

#[test]
fn runtime_token_source_matches_the_full_lids_baseline() {
    let runtime = declared_token_values(LIDS_TOKENS);
    let documented = declared_token_values(LIDS_TOKEN_DOCUMENT);

    assert_eq!(runtime.len(), 127);
    assert_eq!(documented.len(), 127);
    assert_eq!(runtime, documented);
    assert!(declared_token_values(SHELL_CSS).is_empty());
    assert!(declared_token_values(EVIDENCE_LIBRARY_CSS).is_empty());
}

#[test]
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
    let html =
        evidence_page::render_read_projection(&evidence_library_html(), &projection, Some("A娃"));

    assert!(html.contains("&lt;script&gt;not a cover&lt;/script&gt;"));
    assert!(html.contains("MEDIA<br>NOT ACQUIRED"));
    assert!(!html.contains("<script>not a cover</script>"));
    assert!(!html.contains("https://"));
    assert!(!html.contains("xhscdn"));
}

#[test]
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

    let html = evidence_page::render_read_projection(&evidence_library_html(), &projection, None);

    assert!(html.contains("PUBLISHED_AT UNKNOWN"));
    assert!(html.contains("未用首次发现、观察或接收时间替代"));
    assert!(html.contains("VIEW = LATEST ACCEPTED DISCOVERY / PUBLISHED_AT MAY BE UNKNOWN"));
    assert!(html.contains("<em>视角</em> 最新已接纳"));
    assert!(!html.contains("2026-08-26 00:00:00+00"));
}

#[test]
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
        &evidence_library_html(),
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
        &evidence_library_html(),
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
fn default_empty_read_view_is_not_misdescribed_as_an_empty_published_window() {
    let projection = DiscoveryLibraryProjection {
        cards: vec![],
        excluded_unknown_published_at: 0,
        time_view: "latest_accepted_discovery",
    };

    let html = evidence_page::render_read_projection(&evidence_library_html(), &projection, None);

    assert!(html.contains("当前视角没有可展示卡片"));
    assert!(html.contains("最新已接纳不是发布时间窗口"));
    assert!(!html.contains("当前窗口没有可展示卡片"));
}

#[test]
fn default_local_query_is_latest_accepted_discovery_and_explicit_windows_remain_published_only() {
    let default = local_query(&EvidenceLibraryParams {
        q: None,
        window: None,
    })
    .expect("an omitted URL window selects the explicit default discovery view");
    assert_eq!(
        default.time_view(),
        linggan_contracts::EvidenceTimeView::LatestAcceptedDiscovery
    );
    assert_eq!(default.published_window(), None);

    let explicit = local_query(&EvidenceLibraryParams {
        q: None,
        window: Some("last_30_days".to_owned()),
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
async fn loopback_ingress_then_library_page_only_returns_locally_accepted_discovery_cards() {
    let database = proof_database("local_api_ingress").await;
    let observed_at = producer_fixture_observed_at(&database).await;
    let package = discovery_package(&observed_at);
    let application = app_with_database(database);

    let response = application
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/local/discovery-packages")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(package))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let response = application
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/local/evidence-library?q=ADHD&window=last_30_days")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();
    assert!(body.contains("note-api-known"));
    assert!(!body.contains("note-api-unknown"));
    assert!(body.contains("\"timeView\":\"last_30_days\""));
    assert!(body.contains("\"excludedUnknownPublishedAt\":1"));
    assert!(!body.contains("https://"));
    assert!(!body.contains("xhscdn"));

    let response = application
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/local/evidence-library?q=ADHD")
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
    assert!(body.contains("note-api-unknown"));
    assert!(body.contains("\"timeView\":\"latest_accepted_discovery\""));
    assert!(body.contains("\"excludedUnknownPublishedAt\":0"));

    let response = application
        .clone()
        .oneshot(
            Request::builder()
                .uri("/corpus/evidence?q=ADHD&window=last_30_days")
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
    assert!(html.contains("MEDIA<br>NOT ACQUIRED"));
    assert!(!html.contains("https://"));
    assert!(!html.contains("xhscdn"));

    let response = application
        .clone()
        .oneshot(
            Request::builder()
                .uri("/corpus/evidence?q=ADHD")
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
    assert!(html.contains("PUBLISHED_AT UNKNOWN"));
    assert!(html.contains("未用首次发现、观察或接收时间替代"));
}

#[tokio::test]
#[ignore = "requires ./scripts/test-local-001-discovery-postgres.sh and an isolated PostgreSQL proof database"]
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
                .uri("/api/local/evidence-library?q=API&window=last_30_days")
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
                .uri("/api/local/evidence-library?q=ADHD&window=last_7_days")
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
    assert!(html.contains("<em>窗口</em> 7D"));
    assert!(html.contains("WINDOW = PUBLISHED_AT / 7D"));
    assert!(!html.contains("WINDOW = PUBLISHED_AT / 30D"));
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
async fn loopback_runtime_producer_uses_the_three_routes_published_by_health() {
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
            "PLUGIN_RUNTIME_001_SCHEMA_READY".to_owned()
        ))
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
        let html = collection::render(section, collection::OperationsMode::Now, None);
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
    let mut pages = vec![evidence_library_html()];
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
    );
    let context_row = targets
        .split_once("v7-context-meta")
        .expect("collection pages render the context row")
        .1;
    for kpi in [
        "<span class=\"v7-kpi\"><em>巡逻中断</em><b>UNKNOWN</b></span>",
        "<span class=\"v7-kpi\"><em>建档未完成</em><b>UNKNOWN</b></span>",
    ] {
        assert!(
            context_row.contains(kpi),
            "count missing from context row: {kpi}"
        );
    }
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
        let html = collection::render(section, collection::OperationsMode::Now, None);
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
    let mut pages = vec![evidence_library_html()];
    pages.push(collection::render(
        collection::Section::Targets,
        collection::OperationsMode::Now,
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
            let html = collection::render(section, mode, None);
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
    let targets = collection::render(
        collection::Section::Targets,
        collection::OperationsMode::Now,
        None,
    );
    assert!(targets.contains("采集授权链尚未存在"));
    assert!(targets.contains("需要采集授权"));
    // The create action must be visibly unavailable, not a button that silently does nothing.
    assert!(targets.contains("＋ 新建观察目标"));
    assert!(targets.contains("disabled aria-disabled=\"true\">＋ 新建观察目标"));

    let attention = collection::render(
        collection::Section::Attention,
        collection::OperationsMode::Now,
        None,
    );
    // Q9 moved this out of its own column and into the opening line, but the claim it guards
    // is unchanged: an empty queue must never read as "confirmed zero faults".
    assert!(attention.contains("这不是「已确认零故障」"));
    // Upstream-empty surfaces must hand the reader the step that is actually stopped.
    assert!(attention.contains("/collection/targets"));

    let runtime = collection::render(
        collection::Section::Runtime,
        collection::OperationsMode::Now,
        None,
    );
    assert!(runtime.contains("调度器未接通"));
}

#[test]
fn operations_modes_are_addressable_and_the_stream_stays_honest() {
    let now = collection::render(
        collection::Section::Operations,
        collection::OperationsMode::Now,
        None,
    );
    assert!(now.contains("LIVE OBSERVATION"));
    assert!(now.contains("NOT CONNECTED"));
    // The prototype invented an event every seven seconds. Production must not.
    assert!(now.contains("不会用计时器伪造事件"));
    assert!(now.contains("暂停只停止画面跟随，永远不会暂停真实的采集调度"));

    let trace = collection::render(
        collection::Section::Operations,
        collection::OperationsMode::Trace,
        None,
    );
    assert!(trace.contains("没有可回放的观察历史"));
    let review = collection::render(
        collection::Section::Operations,
        collection::OperationsMode::Review,
        None,
    );
    assert!(review.contains("观察盲区"));

    assert!(now.contains("href=\"/collection/operations?mode=trace\""));
}

#[test]
fn target_drawer_is_owned_by_the_url_and_escapes_its_identifier() {
    let closed = collection::render(
        collection::Section::Targets,
        collection::OperationsMode::Now,
        None,
    );
    assert!(!closed.contains("c-drawer"));

    let open = collection::render(
        collection::Section::Targets,
        collection::OperationsMode::Now,
        Some("T-CR-019"),
    );
    assert!(open.contains("id=\"c-drawer\""));
    assert!(open.contains("#T-CR-019"));
    assert!(open.contains("未找到该观察目标"));

    let injected = collection::render(
        collection::Section::Targets,
        collection::OperationsMode::Now,
        Some("<script>alert(1)</script>"),
    );
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
    let evidence = evidence_library_html();
    let collection = collection::render(
        collection::Section::Targets,
        collection::OperationsMode::Now,
        None,
    );

    for (html, page, own_href, other_href) in [
        (&evidence, "corpus", "/corpus", "/collection"),
        (&collection, "collection", "/collection", "/corpus"),
    ] {
        assert!(
            html.contains(&format!("<a href=\"{own_href}\" aria-current=\"page\">")),
            "{page} must mark its own primary entry as the current page"
        );
        assert!(
            html.contains(&format!("<a href=\"{other_href}\">")),
            "{page} must offer a working link to the other served surface"
        );

        // Naming a responsibility is not the same as serving it: the four unconnected
        // entries must stay disabled buttons rather than become dead links.
        for unserved in ["雷达", "主题图谱", "洞察"] {
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
            for class in shell_classes.iter() {
                let needle = format!(".{class}");
                let matched = selector.match_indices(&needle).any(|(index, _)| {
                    // ".v7-side" must not match ".v7-side-nav"
                    selector[index + needle.len()..]
                        .chars()
                        .next()
                        .is_none_or(|c| !c.is_ascii_alphanumeric() && c != '-')
                });
                if matched {
                    violations.push(format!("{selector}  (shell owns .{class})"));
                    break;
                }
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
    let corpus = evidence_library_html();
    let collection = collection::render(
        collection::Section::Targets,
        collection::OperationsMode::Now,
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
    // DESIGN-006 Q9. Five empty surfaces, three kinds of empty — and only one of them is the
    // reader's to act on. Rendered at equal weight they answered everything except "so what
    // do I do".
    let surface = |section| collection::render(section, collection::OperationsMode::Now, None);

    // Exactly one surface may claim the reader's attention, and it must name the action.
    let targets = surface(collection::Section::Targets);
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

    // Surfaces empty only because their upstream is must hand over a way out.
    for (name, html, href) in [
        (
            "attention",
            surface(collection::Section::Attention),
            "/collection/targets",
        ),
        (
            "tasks",
            surface(collection::Section::Tasks),
            "/collection/targets",
        ),
    ] {
        assert!(
            html.contains("c-empty-upstream"),
            "{name} is upstream-empty"
        );
        assert!(
            html.contains(&format!("class=\"c-empty-pointer\" href=\"{href}\"")),
            "{name} must point at the step that is actually stopped"
        );
    }

    // Surfaces waiting on engineering must say so, so nobody hunts for an action.
    // Operations' default mode is not an empty state — it renders the pipeline itself — so
    // its awaiting-engineering wording lives on the two modes that are empty.
    for (name, html) in [
        (
            "operations/trace",
            collection::render(
                collection::Section::Operations,
                collection::OperationsMode::Trace,
                None,
            ),
        ),
        (
            "operations/review",
            collection::render(
                collection::Section::Operations,
                collection::OperationsMode::Review,
                None,
            ),
        ),
        ("runtime", surface(collection::Section::Runtime)),
    ] {
        assert!(
            html.contains("这一栏不需要你做任何事"),
            "{name} must state that it needs nothing from the reader"
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
    assert!(operations.contains("SCHEDULER NOT CONNECTED"));

    // Filter tabs keep their names and lose their placeholder counts.
    let targets = collection::render(
        collection::Section::Targets,
        collection::OperationsMode::Now,
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
    let html = evidence_library_html();
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
    for entry in ["href=\"/corpus\"", "href=\"/collection\""] {
        assert!(nav.contains(entry), "primary nav is missing {entry}");
    }
}
