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
    include_str!("../../../../database/migrations/0001_scope_001_capture_evidence.sql"),
    "\n",
    include_str!("../../../../database/migrations/0002_local_001_discovery.sql"),
    "\n",
    include_str!("../../../../database/migrations/0003_local_trusted_producer.sql"),
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
        !EVIDENCE_LIBRARY_CSS.contains("#e8003f"),
        "the second signature colour must stay retired: DESIGN-003 collapsed the palette onto --lgi-signal"
    );
    assert!(
        !EVIDENCE_LIBRARY_CSS.contains('#'),
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
            EVIDENCE_LIBRARY_CSS.contains(required_css),
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

    assert_eq!(runtime.len(), 117);
    assert_eq!(documented.len(), 117);
    assert_eq!(runtime, documented);
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
            published_at: "2026-08-25 00:00:00+00".to_owned(),
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
        window: "last_30_days",
    };
    let html =
        evidence_page::render_read_projection(evidence_library_html(), &projection, Some("A娃"));

    assert!(html.contains("&lt;script&gt;not a cover&lt;/script&gt;"));
    assert!(html.contains("MEDIA<br>NOT ACQUIRED"));
    assert!(!html.contains("<script>not a cover</script>"));
    assert!(!html.contains("https://"));
    assert!(!html.contains("xhscdn"));
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
    assert!(!body.contains("https://"));
    assert!(!body.contains("xhscdn"));

    let response = application
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
                .uri("/api/local/evidence-library?q=ADHD&window=last_30_days")
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
    assert!(body.contains("\"window\":\"last_7_days\""));
    assert!(!body.contains("\"window\":\"last_30_days\""));

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
            {{"content":{{"platformContentId":"note-api-unknown","title":"未知发布时间","creatorDisplayName":"另一位家长"}},"occurrence":{{"query":"ADHD","sort":"comprehensive","observedAt":"{observed_at}","resultPosition":2}}}}
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
