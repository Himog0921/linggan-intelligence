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

#[test]
fn recovery_control_read_failure_still_blocks_the_runtime_page() {
    let recovery_lanes = collection::collection_control_surface_view::recovery_runtime_lanes();
    let control = runtime_control_for_page(None, Some(&recovery_lanes), false)
        .expect("the known recovery gate supplies a control projection");
    assert_eq!(control.lanes.len(), 3);
    assert!(control.lanes.iter().all(|lane| {
        !lane.available
            && !lane.queueable
            && lane.reason_code == Some("collection_upgrade_recovery_only")
    }));
    assert!(runtime_control_for_page(None, None, false).is_none());
}

#[tokio::test]
async fn legacy_evidence_route_cannot_bypass_domain_scoped_work_resources() {
    for uri in [
        "/api/local/evidence-library/legacy",
        "/api/local/evidence-library/legacy?domain=00000000-0000-4000-8000-000000000001",
    ] {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .header(header::HOST, "127.0.0.1:8080")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}

#[tokio::test]
async fn evidence_library_rejects_unknown_url_filters_before_reading() {
    let domain = "00000000-0000-4000-8000-000000000001";
    let invalid = app()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/local/work-resources?domain={domain}&creatorKye=typo"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    let valid = app().oneshot(Request::builder()
        .uri(format!("/api/local/work-resources?domain={domain}&creatorKey=xhs%3A41&creatorFilter=%7B%22domain%22%3A%22{domain}%22%7D&returnTo=%2Fcorpus%2Fcreators%3Fdomain%3D{domain}"))
        .body(Body::empty()).unwrap()).await.unwrap();
    assert_ne!(valid.status(), StatusCode::BAD_REQUEST);
    let parsed: material_projection::EvidenceLibraryParams = serde_json::from_value(json!({
        "domain": domain,
        "creatorKey": "xhs:41",
        "creatorFilter": format!(r#"{{"domain":"{domain}"}}"#),
        "returnTo": format!("/corpus/creators?domain={domain}"),
    }))
    .unwrap();
    assert!(material_projection::local_query(&parsed).is_ok());
}

#[test]
fn lease_refusal_keeps_the_paused_domain_reason() {
    assert_eq!(
        lease_error_code(&LeaseError::ControlBlocked {
            reason_code: "domain_paused_or_unscoped".to_owned(),
        }),
        "domain_paused_or_unscoped"
    );
}

#[path = "../../../../crates/evidence/tests/support/material_fixture.rs"]
mod domain_management_fixture;

#[tokio::test]
#[ignore = "requires ./scripts/test-local-001-discovery-postgres.sh and an isolated PostgreSQL proof database"]
async fn domain_management_page_queries_real_lane_receipts_without_english_or_false_zero() {
    let database = domain_management_fixture::proof_database("domain_page_lane_receipts").await;
    let external_id = "domain-page-lane-proof";
    let package_ref = domain_management_fixture::submit_package(
        &database,
        "profile_discovery",
        serde_json::json!({"authorExternalId":"domain-page-author"}),
        serde_json::json!({
            "kind":"profile_discovery_card",
            "resultPosition":1,
            "sourceObject":{"platform":"xhs","type":"content","externalId":external_id},
            "payload":{"title":"领域页通道回执证明"}
        }),
    )
    .await;
    let content_ref: uuid::Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content \
         WHERE platform='xhs' AND content_external_id=$1",
    )
    .bind(external_id)
    .fetch_one(database.pool())
    .await
    .expect("the submitted producer record creates canonical material");
    sqlx::query(
        "INSERT INTO linggan_material_domain_usage \
            (usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) \
         VALUES ($1,$2,'00000000-0000-4000-8000-000000000001','primary', \
                 'legacy_domain_migration',$3)",
    )
    .bind(uuid::Uuid::new_v4())
    .bind(content_ref)
    .bind(package_ref)
    .execute(database.pool())
    .await
    .expect("the proof attaches an explicit legacy Domain usage to accepted material");
    let discovery_facts: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_material_discovery_finding \
         WHERE package_ref=$1 AND content_public_ref=$2",
    )
    .bind(package_ref)
    .bind(content_ref)
    .fetch_one(database.pool())
    .await
    .expect("the accepted package retains its typed discovery fact");
    assert_eq!(discovery_facts, 1);

    let response=app_with_database(database).oneshot(Request::builder().uri("/api/collection/workspace/domains?domain=00000000-0000-4000-8000-000000000001").header(header::HOST,"127.0.0.1:8080").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(response.status(),StatusCode::OK);
    let body=to_bytes(response.into_body(),usize::MAX).await.unwrap();
    let result:Value=serde_json::from_slice(&body).unwrap();
    let domain=result["domains"].as_array().unwrap().iter().find(|d|d["domainRef"]=="00000000-0000-4000-8000-000000000001").unwrap();
    let lanes=domain["lanes"].as_array().unwrap();
    assert!(lanes.iter().any(|lane|lane["label"]=="发现" && lane["summary"]=="可检索 1 篇"));
    assert!(lanes.iter().any(|lane|lane["label"]=="评论" && lane["summary"]=="状态未知"));

}

#[tokio::test]
#[ignore = "requires ./scripts/test-local-001-discovery-postgres.sh and an isolated PostgreSQL proof database"]
async fn retired_policy_endpoint_cannot_write_even_for_a_paused_domain() {
    let database = domain_management_fixture::proof_database("comment_policy_paused_domain").await;
    sqlx::raw_sql(include_str!(
        "../../../../database/bootstrap/comment-study-001.sql"
    ))
    .execute(database.pool())
    .await
    .expect("the Comment Study derived schema is initialized for the API route proof");
    let domain_ref = uuid::Uuid::parse_str("00000000-0000-4000-8000-000000000001").unwrap();
    sqlx::query("UPDATE observation_domain SET status='paused' WHERE domain_ref=$1")
        .bind(domain_ref)
        .execute(database.pool())
        .await
        .expect("the test Domain is paused before the policy write");
    let model_config_ref = uuid::Uuid::new_v4();
    let response = app_with_database(database.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/local/comment-study/policy")
                .header(header::HOST, "127.0.0.1:8080")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "domainRef":domain_ref,
                        "modelConfigRef":model_config_ref,
                        "commentBudget":100,
                        "contextCharacterBudget":12000
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::GONE);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["error"]["code"], "policy_endpoint_retired");
    let policy_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_study_policy \
         WHERE domain_ref=$1 AND model_config_ref=$2",
    )
    .bind(domain_ref)
    .bind(model_config_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(policy_count, 0, "a paused Domain retains no new policy");
}

#[test]
fn check_in_payload_returns_the_server_canonical_station_name_and_pause_state() {
    let station_ref = uuid::Uuid::new_v4();
    let payload = check_in_payload(&CheckInOutcome::Heartbeat {
        installation_ref: uuid::Uuid::new_v4(),
        station_ref: Some(station_ref),
        station_display_name: Some("本机 Chrome".to_owned()),
        accepting_tasks: Some(false),
        credential: None,
    });
    assert_eq!(
        payload.pointer("/stationRef"),
        Some(&serde_json::Value::String(station_ref.to_string()))
    );
    assert_eq!(
        payload.pointer("/stationDisplayName"),
        Some(&serde_json::Value::String("本机 Chrome".to_owned()))
    );
    assert_eq!(
        payload.pointer("/stationAccepting"),
        Some(&serde_json::Value::Bool(false))
    );
}

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
    assert_eq!(
        payload.pointer("/readiness/state").and_then(Value::as_str),
        Some("not_configured"),
        "没给数据库地址是配置错误，与「连不上」不是同一件事：{payload}"
    );
    assert_eq!(payload.pointer("/readiness/detail"), Some(&Value::Null));
    assert_eq!(
        payload.pointer("/readiness/checkedAt"),
        Some(&Value::Null),
        "问不到数据库时钟就不编造判定时刻"
    );
}

#[tokio::test]
async fn health_readiness_separates_an_unreachable_database_from_a_missing_configuration() {
    let application = router(LocalWebState {
        database: LocalDatabaseState::DatabaseUnavailable,
        active_media_sessions: Arc::new(tokio::sync::Mutex::new(BTreeSet::new())),
        account_digest_key: None,
    });
    let payload = health_payload(application).await;

    assert_eq!(
        payload.pointer("/readiness/state").and_then(Value::as_str),
        Some("database_unreachable"),
        "连不上是环境问题，等它回来就会好：{payload}"
    );
    assert_eq!(
        payload.pointer("/readiness/detail").and_then(Value::as_str),
        Some("connect_failed"),
        "detail 只写类别，不带连接串"
    );
    assert_eq!(payload.pointer("/readiness/checkedAt"), Some(&Value::Null));
}

#[tokio::test]
async fn local_entry_redirects_to_the_login_page() {
    let response = app()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(response.headers().get(header::LOCATION).unwrap(), "/login");
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
    assert!(evidence_library_html(None, &[], None).contains("来源材料尚未完整接通"));
    assert!(evidence_library_html(None, &[], None).contains("SOURCE INCOMPLETE"));
    assert!(evidence_library_html(None, &[], None).contains("没有可展示的本地材料"));
    assert!(!evidence_library_html(None, &[], None).contains(&["SYSTEM", "LIVE"].join(" ")));
}

#[test]
#[cfg(any())] // superseded server-rendered discovery page
fn evidence_page_does_not_replace_unknown_with_zero() {
    assert!(evidence_library_html(None, &[], None).contains("覆盖情况 <strong>未知"));
    assert!(!evidence_library_html(None, &[], None).contains("评论 0"));
}

#[test]
#[cfg(any())] // superseded server-rendered discovery page
fn evidence_library_uses_chinese_for_user_meaning_and_english_only_as_technical_keys() {
    let base = evidence_library_html(None, &[], None);

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
    let base = evidence_library_html(None, &[], None);
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
    let html = evidence_library_html(None, &[], None);

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
    let html = evidence_library_html(None, &[], None);

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

    assert_eq!(runtime.len(), 132);
    assert_eq!(documented.len(), 132);
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
        &evidence_library_html(None, &[], None),
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

    let html = evidence_page::render_read_projection(
        &evidence_library_html(None, &[], None),
        &projection,
        None,
    );

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
        &evidence_library_html(None, &[], None),
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
        &evidence_library_html(None, &[], None),
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

    let html = evidence_page::render_read_projection(
        &evidence_library_html(None, &[], None),
        &projection,
        None,
    );

    assert!(html.contains("当前视角没有可展示卡片"));
    assert!(html.contains("最新已接纳不是发布时间窗口"));
    assert!(!html.contains("当前窗口没有可展示卡片"));
}

#[test]
fn default_local_query_is_latest_accepted_discovery_and_explicit_windows_remain_published_only() {
    let default = material_projection::local_query(&material_projection::EvidenceLibraryParams {
        creator_key: None,
        creator_filter: None,
        _return_to: None,
        public_refs: None,
        cursor: None,
        q: None,
        window: None,
        sort: None,
        lane: None,
        lane_state: None,
        media_kind: None,
        restriction: None,
        domain: None,
        collection_run_ref: None,
    })
    .expect("an omitted URL window selects the explicit default discovery view");
    assert_eq!(
        default.time_view(),
        linggan_contracts::EvidenceTimeView::LatestAcceptedDiscovery
    );
    assert_eq!(default.published_window(), None);

    let explicit = material_projection::local_query(&material_projection::EvidenceLibraryParams {
        creator_key: None,
        creator_filter: None,
        _return_to: None,
        public_refs: None,
        cursor: None,
        q: None,
        window: Some("last_30_days".to_owned()),
        sort: None,
        lane: None,
        lane_state: None,
        media_kind: None,
        restriction: None,
        domain: None,
        collection_run_ref: None,
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

/// `/health` 的就绪回答与 worker 用的是同一套判据（`runtime_readiness`），同一次故障在这里
/// 和在 worker 日志里必须是同一句话。注入用「台账少一行」：迁移文件全部应用过，只是记账少了
/// `0034`。这类故障最容易糊过去——库是好的、表都在，只有记账不齐。
#[tokio::test]
#[ignore = "requires ./scripts/test-local-001-discovery-postgres.sh and an isolated PostgreSQL proof database"]
async fn health_readiness_speaks_the_same_verdict_as_the_worker() {
    let database = proof_database("local_api_readiness").await;
    let application = app_with_database(database.clone());

    let ready = health_payload(application.clone()).await;
    assert_eq!(
        ready.pointer("/readiness/state").and_then(Value::as_str),
        Some("ready"),
        "迁移与表都齐就该判成可接活：{ready}"
    );
    assert_eq!(ready.pointer("/readiness/detail"), Some(&Value::Null));
    let checked_at = ready
        .pointer("/readiness/checkedAt")
        .and_then(Value::as_str)
        .expect("连得上就有数据库时钟给的判定时刻");
    assert!(
        checked_at.ends_with('Z'),
        "判定时刻用数据库时钟：{checked_at}"
    );

    sqlx::query(
        "DELETE FROM linggan_local_schema_migration \
         WHERE migration_id = '0034_collection_control_closure'",
    )
    .execute(database.pool())
    .await
    .expect("the ledger row is removed for this proof");

    let not_applied = health_payload(application).await;
    assert_eq!(
        not_applied
            .pointer("/readiness/state")
            .and_then(Value::as_str),
        Some("migrations_not_applied"),
        "连得上、表在、记账缺一行，就该报未迁移：{not_applied}"
    );
    assert_eq!(
        not_applied
            .pointer("/readiness/detail")
            .and_then(Value::as_str),
        Some("0034_collection_control_closure"),
        "detail 写声明序里第一个缺的 id，与 worker 报的是同一句"
    );
}

/// 本地读取面用不了时（`SchemaUnavailable`）仍然拿得住连接，所以 `/health` 能说出**为什么**
/// 不能接活，而不是在这里替它猜一个原因。这里把台账表整个删掉：读取面判死，而就绪判定报的
/// 是「台账读不到」——两者是不同的问题，第二个回答更具体。
#[tokio::test]
#[ignore = "requires ./scripts/test-local-001-discovery-postgres.sh and an isolated PostgreSQL proof database"]
async fn a_schema_unavailable_api_still_says_why_it_cannot_take_work() {
    let database = proof_database("local_api_readiness_schema").await;
    sqlx::query("DROP TABLE linggan_local_schema_migration")
        .execute(database.pool())
        .await
        .expect("the ledger table is dropped for this proof");

    let state = classify_database(database).await;
    assert!(
        matches!(state, LocalDatabaseState::SchemaUnavailable(_)),
        "基础 schema 不在时读取面判死"
    );
    let application = router(LocalWebState {
        database: state,
        active_media_sessions: Arc::new(tokio::sync::Mutex::new(BTreeSet::new())),
        account_digest_key: None,
    });
    let payload = health_payload(application).await;

    assert_eq!(
        payload.pointer("/database/state").and_then(Value::as_str),
        Some("CONFIGURED_UNAVAILABLE"),
        "读取面的状态不变：{payload}"
    );
    assert_eq!(
        payload.pointer("/readiness/state").and_then(Value::as_str),
        Some("migration_ledger_unreadable"),
        "就绪回答要说出确切原因，不是笼统的「schema 不行」：{payload}"
    );
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

/// 采集准入按**状态**拒绝时，回执说的必须是「状态前置」，不是「稍后可以重试」。
///
/// 这条阶梯两段共用。此前它写在两个分支里、只有第一段分了类，于是第二段遇到同一种拒绝
/// （关键词被停掉之后，详情补采同样不被准入接受）仍然说「稍后可以重试」——一句让人反复
/// 做同一件永远不会成的事的话。这张卡要修的正是回执说谎，所以分类只留一处。
///
/// 另一半同样要钉住：**真的没送出去**（目标已不在、schema 没就绪、数据库故障）必须继续
/// 说「可以稍后重试」。分不清这两类，只是把「一律说重试」翻成「一律说别重试」，一样是
/// 让人照着错的话做决定。
#[test]
fn a_refusal_by_state_is_not_told_to_retry_later_while_a_genuine_miss_is() {
    assert_eq!(
        keyword_archive_error_receipt(&AcquisitionChainError::TargetNotRequestable {
            state: "dismissed".to_owned(),
        }),
        "keyword_archive_not_requestable",
        "准入按状态拒绝是永久前置，说成「稍后可以重试」会让人一直点"
    );
    assert_eq!(
        keyword_archive_error_receipt(&AcquisitionChainError::TargetDomainUnassigned),
        "target_domain_unassigned",
        "缺领域要如实说成缺领域，不能落进信息量最低的那条兜底"
    );
    assert_eq!(
        keyword_archive_error_receipt(&AcquisitionChainError::SchemaUnavailable),
        "archive_unavailable",
        "schema 没就绪是真的可以稍后重试，不能和状态前置混成一句"
    );
}

/// 「建立档案」按下去要真的开始建档：一个**从没建过档**的关键词必须发出第一段
/// （翻搜索面拿链接），而不是回一句「详情已补齐」然后什么都不做。
///
/// 这个入口此前是「先试第二段，挑不出候选就当已完成」：一个还没有作品链接的词，第二段
/// 当然挑不出候选——空的候选集被读成了「都补完了」，于是它回一句「详情已补齐」就结束，
/// **第一段永远发不出去**，库里一张工单都没有。行上写着「尚未建立」，界面上却没有任何
/// 入口能把它建起来（2026-09-15 在「a 娃」这个词上就是整个页面没有任何反应）。
///
/// 判据只有一条是别人替不了的：**库里真的多了一张待翻搜索面的工单**。只断言回执文案
/// 不够——那句话本来就是错的，错的地方正在于它说的话和库里的状态对不上。
#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn nul_submission_is_terminal_422_without_partial_package_or_receipt() {
    let database = proof_database("nul_submission_rejection").await;
    let application = app_with_database(database.clone());
    let mut submission: Value =
        serde_json::from_str(&runtime_author_producer_submission()).unwrap();
    submission["capturePackage"]["records"][0]["payload"]["description"] =
        serde_json::json!("前\0后");
    for _ in 0..2 {
        let response = application
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/local/producer/runtime-submissions")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(submission.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert!(String::from_utf8_lossy(&body).contains("submission_invalid"));
    }
    let counts: (i64,i64) = sqlx::query_as("SELECT (SELECT count(*) FROM linggan_runtime_capture_package), (SELECT count(*) FROM linggan_runtime_submission_receipt)")
        .fetch_one(database.pool()).await.unwrap();
    assert_eq!(counts, (0, 0));
}

#[tokio::test]
#[ignore = "requires ./scripts/test-local-001-discovery-postgres.sh and an isolated PostgreSQL proof database"]
async fn a_never_archived_keyword_starts_its_first_stage_when_asked_to_archive() {
    let database = proof_database("keyword_archive_first_stage").await;
    let application = app_with_database(database.clone());

    // 从页面加入一个关键词：领域当场选定——缺领域的目标发不出任何采集，准入会先拦住它，
    // 那样这条用例就绕开了真正要证明的那一段。
    let created = collection_json_command(&application, "new", json!({"target_kind":"keyword","identity":"archive-first-stage-proof","domain":"00000000-0000-4000-8000-000000000001"})).await;
    assert_eq!(created["outcome"], "applied");
    let target_ref: uuid::Uuid = sqlx::query_scalar(
        "SELECT target_ref FROM collection_observation_target \
         WHERE target_kind='keyword' AND identity_key='archive-first-stage-proof'",
    )
    .fetch_one(database.pool())
    .await
    .expect("页面新建的关键词目标落库");

    // 授权走产品自己的入口，与人在授权页上按下的那一次是同一条路。
    let granted = application
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/local/collection/authorizations")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "targetKind": "keyword",
                        "lane": "deep_archive",
                        "purpose": "从观察目标页发起关键词历史建档",
                        "maxTargets": 200,
                        "maxWorksPerTarget": 200,
                        "validForDays": 30,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(granted.status(), StatusCode::OK);

    // 人在列表上按下的那一下。
    let archived = collection_json_command(&application, "archive", json!({"row_target_ref":target_ref})).await;
    assert_eq!(archived["outcome"], "applied");
    let location = archived["location"].as_str().expect("回执地址");
    assert!(
        location.contains("archive_requested"),
        "一个从没建过档的词，这一下应当发出第一段建档，实际回执：{location}"
    );
    assert!(
        !location.contains("keyword_detail_complete"),
        "链接都还没有的词不可能「详情已补齐」：{location}"
    );

    let orders: Vec<String> = sqlx::query_scalar(
        "SELECT lane FROM collection_work_order WHERE target_ref=$1 ORDER BY created_at",
    )
    .bind(target_ref)
    .fetch_all(database.pool())
    .await
    .expect("工单可查");
    assert_eq!(
        orders,
        vec!["deep_archive".to_owned()],
        "这一下应当只发出翻搜索面那一段，实际工单：{orders:?}"
    );
}

/// 历史上已经在观察、但未建档的关键词必须能补建第一阶段档案。
///
/// 巡查准入仍被关键词完成门槛阻断；这里仅允许修复旧状态留下的缺失底座，不能把一次
/// `deep_archive` 成功误报成巡查已恢复。
#[tokio::test]
#[ignore = "requires ./scripts/test-local-001-discovery-postgres.sh and an isolated PostgreSQL proof database"]
async fn a_patrolling_keyword_can_repair_its_missing_first_stage_archive() {
    let database = proof_database("keyword_archive_patrolling_refusal").await;
    let application = app_with_database(database.clone());
    let created = collection_json_command(&application, "new", json!({"target_kind":"keyword","identity":"archive-patrolling-refusal","domain":"00000000-0000-4000-8000-000000000001"})).await;
    assert_eq!(created["outcome"], "applied");
    let target_ref: uuid::Uuid = sqlx::query_scalar(
        "SELECT target_ref FROM collection_observation_target \
         WHERE target_kind='keyword' AND identity_key='archive-patrolling-refusal'",
    )
    .fetch_one(database.pool())
    .await
    .expect("页面新建的关键词目标落库");

    let granted = application
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/local/collection/authorizations")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "targetKind": "keyword",
                        "lane": "deep_archive",
                        "purpose": "从观察目标页发起关键词历史建档",
                        "maxTargets": 200,
                        "maxWorksPerTarget": 200,
                        "validForDays": 30,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(granted.status(), StatusCode::OK);

    // 线上 a 娃 与 adhd 的真实状态：正在按周观察。
    sqlx::query(
        "UPDATE collection_observation_target SET lifecycle_state='monitoring' WHERE target_ref=$1",
    )
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("置为观察中");

    let archived = collection_json_command(&application, "archive", json!({"row_target_ref":target_ref})).await;
    assert_eq!(archived["outcome"], "applied");
    let location = archived["location"].as_str().expect("回执地址");
    assert!(
        location.contains("archive_requested"),
        "历史巡查状态不得把关键词困在未建档状态，实际回执：{location}"
    );
    assert!(
        !location.contains("keyword_archive_not_requestable"),
        "修复性建档已被接受，不能再报成不可请求：{location}"
    );

    let orders: Vec<String> = sqlx::query_scalar(
        "SELECT lane FROM collection_work_order WHERE target_ref=$1 ORDER BY created_at",
    )
    .bind(target_ref)
    .fetch_all(database.pool())
    .await
    .expect("工单可数");
    assert_eq!(orders, vec!["deep_archive"], "修复性建档只创建第一阶段工单");
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn creator_gap_route_never_falls_back_to_a_homepage_rescan() {
    let database = proof_database("creator_gap_route").await;
    let application = app_with_database(database.clone());
    let created = collection_json_command(&application, "new", json!({"target_kind":"creator","identity":"69aad16e000000003201b111","domain":"00000000-0000-4000-8000-000000000001"})).await;
    assert_eq!(created["outcome"], "applied");
    let target_ref: uuid::Uuid = sqlx::query_scalar("SELECT target_ref FROM collection_observation_target WHERE identity_key='69aad16e000000003201b111'")
        .fetch_one(database.pool()).await.unwrap();
    let response = collection_json_command(&application, "archive", json!({"row_target_ref":target_ref,"archive_action":"gaps"})).await;
    assert_eq!(response["outcome"], "rejected");
    assert_eq!(response["code"], "archive_nothing_to_continue");
    let work_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM collection_work_order WHERE target_ref=$1")
            .bind(target_ref)
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert_eq!(work_count, 0);
}

// Exercise the canonical React JSON transport while retaining the original owner oracles.
async fn collection_json_command(application: &Router, action: &str, fields: Value) -> Value {
    let context = application.clone().oneshot(Request::builder()
        .uri("/api/collection/commands/context").header(header::HOST, "127.0.0.1:8080")
        .body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(context.status(), StatusCode::OK);
    let context: Value = serde_json::from_slice(&to_bytes(context.into_body(), usize::MAX).await.unwrap()).unwrap();
    let response = application.clone().oneshot(Request::builder().method("POST")
        .uri(format!("/api/collection/commands/{action}"))
        .header(header::HOST, "127.0.0.1:8080").header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({"requestRef":uuid::Uuid::new_v4(),"epoch":context["epoch"],"fields":fields}).to_string())).unwrap()).await.unwrap();
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
}

async fn proof_database(schema: &str) -> Database {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL")
        .expect("test script must provide the isolated proof database URL");
    isolated_proof_schema(&url, schema, LOCAL_001_MIGRATIONS)
        .await
        .expect("isolated migration applies")
}

/// 走真实路由取一次 `/health` 的回答。
async fn health_payload(application: Router) -> Value {
    let response = application
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&body).unwrap()
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
fn corpus_header_can_reflect_the_connected_collection_read_model() {
    let html = evidence_library_html(Some("观察中"), &[], None);
    let collection_entry = html
        .split_once("v7-tech-key\">COLLECTION")
        .expect("the collection primary entry exists")
        .1;
    assert!(collection_entry.contains("<span class=\"v7-nav-state\">观察中</span>"));
}

#[test]
fn corpus_header_can_preserve_an_unreadable_collection_state() {
    let html = evidence_library_html(Some("状态未知"), &[], None);
    let collection_entry = html
        .split_once("v7-tech-key\">COLLECTION")
        .expect("the collection primary entry exists")
        .1;
    assert!(collection_entry.contains("<span class=\"v7-nav-state\">状态未知</span>"));
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

#[tokio::test]
async fn unreadable_target_json_never_substitutes_empty_html() {
    let response = app().oneshot(Request::builder().uri("/api/collection/workspace/target/00000000-0000-4000-8000-000000000001?domain=all").header(header::HOST,"127.0.0.1:8080").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(response.status(),StatusCode::SERVICE_UNAVAILABLE);
    let body=to_bytes(response.into_body(),usize::MAX).await.unwrap();
    let result:Value=serde_json::from_slice(&body).unwrap();
    assert_eq!(result["code"],"collection_targets_unavailable");
}

/// DESIGN-010 · LIDS v7 on the Operations surface.
///
/// Two things this locks. First, the English descriptive labels that used to sit under every
/// Chinese one are gone and must stay gone: LANG-05 budgets English to machine facts, closed-set
/// state words and structural numbering, and none of these were any of those. The event-kind
/// filters are the control case — they are contract literals, so they stay.
///
/// Second, the sampling lattice on the dark stream stays clipped to its top strip. An earlier
/// build let the sweep travel the full panel height and it crossed the body copy, which is the
/// one thing `materials.md` rules out outright: edges only, never behind running text.
#[test]
fn station_capability_matrix_separates_unverified_from_degraded() {
    use linggan_evidence::{CapabilityState, StationCapability};

    let base = |capability: &str| StationCapability {
        capability: capability.to_owned(),
        declared: true,
        successes: 0,
        last_success_at: None,
        capability_failures: 0,
        execution_failures: 0,
        last_failure_at: None,
    };

    // 声明支持、窗口内没有任何证据 —— 未验证，不是降级。
    assert_eq!(
        base("discovery_search").state(),
        CapabilityState::Unverified
    );

    // 有成功产出 —— 就绪。
    let ready = StationCapability {
        successes: 3,
        ..base("comments")
    };
    assert_eq!(ready.state(), CapabilityState::Ready);

    // 超时一堆但插件从没说过跑不了 —— 仍然就绪。执行失败不是能力缺陷。
    let timing_out = StationCapability {
        successes: 3,
        execution_failures: 11,
        ..base("author_profile")
    };
    assert_eq!(
        timing_out.state(),
        CapabilityState::Ready,
        "页面超时是这一次没成，不能把能力判成坏的"
    );

    // 插件明确回报跑不了 —— 降级，且即使有成功记录也降级。
    let degraded = StationCapability {
        successes: 3,
        capability_failures: 1,
        ..base("media_slots")
    };
    assert_eq!(degraded.state(), CapabilityState::Degraded);

    // 没声明 —— 不支持，与「声明了但没验证」是两件事。
    let undeclared = StationCapability {
        declared: false,
        ..base("replies")
    };
    assert_eq!(undeclared.state(), CapabilityState::NotDeclared);
}

/// v7 的两条可量化排版规则在**执行工位这一页**的落地：字号下限 11px（ADR-03）、
/// 字重只有 400/600/700（ADR-02）。
///
/// 范围刻意只到本页的类名。同一张样式表里，目标页、抽屉与检视面板仍有 10px 与 800/850
/// 字重——那是 DESIGN-011 没有迁移的部分，如实留作欠账，而不是让这条测试假装全表已经合规。
/// 它们各自迁移时把前缀加进下面这张表即可。
fn evidence_page_stylesheet() -> String {
    format!("{SHELL_CSS}\n{EVIDENCE_LIBRARY_CSS}")
}

fn runtime_producer_task_spec() -> String {
    r#"{"contractVersion":"linggan.producer.task-spec.v2","taskId":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa","source":"manual","platform":"xhs","pageType":"note_detail","target":{"contentExternalId":"note-a"},"capabilitiesRequested":["media_slots"],"maximumQuota":1,"commentLimit":"not_requested","acquireMedia":"slots","riskPolicy":"local_trusted_user_initiated","stopConditions":["manual_stop","maximum_quota"]}"#.to_owned()
}

fn runtime_producer_attempt() -> String {
    r#"{"contractVersion":"linggan.producer.attempt.v1","producerInstanceId":"bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb","taskId":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa","attemptId":"cccccccc-cccc-4ccc-8ccc-cccccccccccc"}"#.to_owned()
}

fn runtime_producer_submission() -> String {
    r#"{"contractVersion":"linggan.producer.capture-package.v1","producerInstanceId":"bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb","taskId":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa","attemptId":"cccccccc-cccc-4ccc-8ccc-cccccccccccc","submissionId":"dddddddd-dddd-4ddd-8ddd-dddddddddddd","capturePackage":{"contractVersion":"linggan.producer.capture-package.v1","packageRef":"eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee","packageKind":"media_slots","platform":"xhs","observedAt":"2026-08-25T00:00:00Z","capturedAt":"2026-08-25T00:00:01Z","coverage":{"target":{"basis":"known_set","contentExternalId":"note-a"},"layers":[{"capability":"media_slots","observed":2,"attempted":2,"acquired":0,"verified":0,"failed":0,"notAttempted":0,"unknown":0,"stoppedReason":"media_acquisition_not_started"}]},"records":[{"kind":"media_slot","slotKey":"xhs:note-a:image:1","slot":{"role":"image","ordinal":1},"sourceObject":{"externalId":"note-a"},"observation":{"externalUri":"https://fixture.invalid/one.jpg"},"observationRef":"ffffffff-ffff-4fff-8fff-ffffffffffff"},{"kind":"media_slot","slotKey":"xhs:note-a:image:2","slot":{"role":"image","ordinal":2},"sourceObject":{"externalId":"note-a"},"observation":{"externalUri":"https://fixture.invalid/two.jpg"},"observationRef":"11111111-2222-4333-8444-555555555555"}]}}"#.to_owned()
}

fn runtime_author_producer_task_spec() -> String {
    r#"{"contractVersion":"linggan.producer.task-spec.v2","taskId":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaab","source":"manual","platform":"xhs","pageType":"profile","target":{"authorExternalId":"author-route-1"},"capabilitiesRequested":["author_profile"],"maximumQuota":1,"commentLimit":"not_requested","acquireMedia":"not_requested","riskPolicy":"local_trusted_user_initiated","stopConditions":["manual_stop","maximum_quota"]}"#.to_owned()
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
fn six_primary_entries_keep_the_personal_menu_on_screen_at_laptop_widths() {
    // 运营 made six first-level entries (OPS-MIGRATION-001 D2). At the old 1500/1180 steps the
    // header needed 1362px all the way down to 1181px, so a 1280px laptop clipped 个人菜单. The
    // header now narrows in its own steps; the browser sweep over 901–1600px is recorded in the
    // D2 acceptance record, this guard only keeps the steps from being dropped.
    for step in [
        "@media(max-width:1560px){.v7-global-brand{width:285px",
        "@media(max-width:1380px){.v7-global-brand{width:230px;flex-basis:230px}.v7-global-brand-sub,.v7-global-flex{display:none}",
        "@media(max-width:1240px){.v7-primary-nav .v7-tech-key{display:none}}",
        "@media(max-width:960px){.v7-primary-nav button,.v7-primary-nav a{min-width:80px;padding:0 var(--lgi-space-2)}}",
    ] {
        assert!(SHELL_CSS.contains(step), "missing header step: {step}");
    }
    let laptop_step = SHELL_CSS
        .split("@media(max-width:1380px){")
        .nth(1)
        .and_then(|rest| rest.split("}\n").next())
        .unwrap_or_default();
    assert!(
        !laptop_step.contains("v7-system-boundary"),
        "the laptop step must keep the service-status capsule"
    );
}

#[tokio::test]
async fn collection_default_resolves_to_the_daily_target_page() {
    let response = app().oneshot(Request::builder().uri("/collection").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(response.headers().get(header::LOCATION).unwrap(), "/collection/targets");
}

#[tokio::test]
async fn collection_task_return_scope_only_applies_to_manual_observation() {
    for (command, expected_page) in [
        ("manual_observe", "/collection/tasks?"),
        ("save_rule", "/collection/targets?"),
    ] {
        let form:MonitorRuleWire=serde_json::from_value(json!({"target_ref":"00000000-0000-4000-8000-000000000001","expected_revision":0,"idempotency_key":"00000000-0000-4000-8000-000000000002","command_kind":command,"return_scope":"tasks","return_domain":"all"})).unwrap();
        let response=collection_target_rule_command(State(LocalWebState {database:LocalDatabaseState::NotConfigured, active_media_sessions:Arc::new(tokio::sync::Mutex::new(BTreeSet::new())),account_digest_key:None}),axum::extract::Form(form)).await.into_response();
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        let location = response
            .headers()
            .get(header::LOCATION)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(location.starts_with(expected_page));
        assert!(location.contains("error=read_model_not_connected"));
        if command == "manual_observe" {
            assert!(location.contains("domain=all"));
        }
    }
}

#[test]
fn the_primary_nav_links_to_entry_routes_never_to_a_sub_surface() {
    // The default surface of a responsibility is decided in exactly one place: its entry
    // route. A header that links straight to a sub-surface is a second copy of that
    // decision, and the two drift the first time the default moves — which is exactly what
    // happened when Collection's default became /collection/attention while the header
    // still pointed at /collection/targets.
    let html = evidence_library_html(None, &[], None);
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
    let html = evidence_library_html(None, &[], None);

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
    // The authorization requirement is stated where it is actionable information: when the
    // action cannot run. Once it can, saying what it needed is history.
    assert!(EVIDENCE_OBSERVATION_JS.contains("尚未具备监控目标关联的有效深度归档授权"));
    assert!(EVIDENCE_OBSERVATION_JS.contains("commentsCoverageHistory"));
    assert!(EVIDENCE_LIBRARY_JS.contains("当前互动状态与变化"));
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
fn evidence_library_requires_a_domain_and_uses_work_resources_only() {
    assert!(EVIDENCE_LIBRARY_JS.contains("const API_ROOT = '/api/local/work-resources'"));
    assert!(EVIDENCE_LIBRARY_JS.contains("return item?.identity?.publicRef || null;"));
    assert!(EVIDENCE_LIBRARY_JS.contains("if (!CORPUS_DOMAIN.hasDomain) {"));
    assert!(EVIDENCE_LIBRARY_JS.contains("setFeedback('empty', '请选择研究领域'"));
    assert!(EVIDENCE_LIBRARY_JS.contains("item.domainUsage?.role === 'reference'"));
    assert!(!EVIDENCE_LIBRARY_JS.contains("/api/local/cross-industry/samples"));
    assert!(!EVIDENCE_LIBRARY_JS.contains("crossIndustryListItem"));
    assert!(!EVIDENCE_LIBRARY_JS.contains("selectCrossIndustrySample"));
    assert!(!EVIDENCE_LIBRARY_JS.contains("LIST LEVEL ONLY"));
    assert!(!EVIDENCE_LIBRARY_JS.contains("CORPUS_DOMAIN.isOwn"));
    assert!(!EVIDENCE_LIBRARY_JS.contains("[data-public-ref]"));
}

#[test]
fn corpus_domain_picker_is_a_lids_owned_link_menu_not_a_native_select() {
    let domains = vec![
        ObservationDomain {
            domain_ref: uuid::Uuid::new_v4(),
            name: "ADHD".to_owned(),
            description: None,
            research_goal: None,
            status: "active".to_owned(),
            sample_count: None,
            target_count: 0,
            primary_target_count: 0,
            reference_target_count: 0,
        },
        ObservationDomain {
            domain_ref: uuid::Uuid::new_v4(),
            name: "考研自习".to_owned(),
            description: None,
            research_goal: None,
            status: "active".to_owned(),
            sample_count: Some(21),
            target_count: 0,
            primary_target_count: 0,
            reference_target_count: 0,
        },
    ];
    let html = evidence_library_html(None, &domains, Some(&domains[1]));

    assert!(html.contains("<details class=\"v7-domain-picker\">"));
    assert!(html.contains("<nav aria-label=\"可选观察领域\">"));
    assert!(html.contains("考研自习</span><small>0 个目标 · 21 篇作品</small>"));
    assert!(html.contains("aria-label=\"考研自习，0 个目标 · 21 篇作品\""));
    assert!(html.contains(&format!(
        "href=\"/corpus/evidence?domain={}\" aria-current=\"page\"",
        domains[1].domain_ref
    )));
    assert!(!html.contains("<select id=\"corpus-domain\""));
    assert!(
        SHELL_CSS.contains(".v7-domain-picker[open] summary{box-shadow:var(--lgi-shadow-brutal)}")
    );
    assert!(!SHELL_CSS.contains(".v7-domain-picker select"));
    assert!(!html.contains("id=\"ev-domain-dialog\""));
}

#[test]
fn corpus_without_a_selected_domain_keeps_the_shell_and_offers_a_modal_choice() {
    let domains = vec![
        ObservationDomain {
            domain_ref: uuid::Uuid::new_v4(),
            name: "考研自习".to_owned(),
            description: None,
            research_goal: None,
            status: "active".to_owned(),
            sample_count: None,
            target_count: 2,
            primary_target_count: 2,
            reference_target_count: 0,
        },
        ObservationDomain {
            domain_ref: uuid::Uuid::new_v4(),
            name: "已暂停领域".to_owned(),
            description: None,
            research_goal: None,
            status: "paused".to_owned(),
            sample_count: Some(5),
            target_count: 1,
            primary_target_count: 1,
            reference_target_count: 0,
        },
    ];
    let html = evidence_library_html(None, &domains, None);
    assert!(html.contains("data-theme=\"linggan-intelligence\""));
    assert!(html.contains("class=\"v7-app\""));
    assert!(html.contains("data-corpus-domain-selected=\"false\""));
    assert!(html.contains("<dialog class=\"ev-domain-dialog\""));
    assert!(html.contains("id=\"ev-domain-dialog-title\">选择研究领域"));
    assert!(html.contains("2 个目标 · 作品数未知"));
    assert!(html.contains("已暂停 · 历史材料可读"));
    assert!(html.contains(&format!(
        "href=\"/corpus/evidence?domain={}\"",
        domains[0].domain_ref
    )));
    assert!(!html.contains("<h1>先选择一个领域</h1>"));

    let empty = evidence_library_html(None, &[], None);
    assert!(empty.contains("还没有可选择的研究领域"));
    assert!(empty.contains("href=\"/collection/domains\""));
    assert!(empty.contains("class=\"ev-domain-manage ev-domain-manage--inline\""));
    assert!(!empty.contains("class=\"v7-domain-picker\""));
    assert!(EVIDENCE_LIBRARY_JS.contains("dialog.showModal()"));
    assert!(EVIDENCE_LIBRARY_JS.contains("if (!CORPUS_DOMAIN.hasDomain)"));
    assert!(SHELL_CSS.contains("z-index:var(--lgi-z-header)"));

    let only_one = evidence_library_html(None, &domains[..1], Some(&domains[0]));
    assert!(only_one.contains(&format!(
        "href=\"/corpus/comments?domain={}\"",
        domains[0].domain_ref
    )));
}

#[test]
fn evidence_runtime_restores_system_and_personal_view_strategy_without_faking_saved_views() {
    let html = evidence_library_html(None, &[], None);

    assert!(html.contains("aria-label=\"按材料状态快速筛选\""));
    assert!(html.contains("系统视图"));
    assert!(html.contains("我的视图"));
    assert!(html.contains("暂无已保存视图"));
    // LANG-05 (CORPUS-COPY-001): descriptive English labels and the legacy-fallback policy line
    // must not come back next to the Chinese ones.
    for english in [
        "SYSTEM VIEWS",
        "MY VIEWS",
        "SAVED VIEWS NOT CONNECTED",
        "NO LEGACY FALLBACK",
    ] {
        assert!(
            !html.contains(english),
            "descriptive English label returned: {english}"
        );
    }
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

/* DEC-作者合并 (2026-09-04): one author field on every surface, the Inspector included. */
#[test]
fn evidence_runtime_shows_one_author_field_and_renders_avatar_from_media() {
    assert!(EVIDENCE_LIBRARY_JS.contains("ev-author-fact"));
    assert!(EVIDENCE_LIBRARY_JS.contains("function effectiveAuthor"));
    assert!(EVIDENCE_LIBRARY_JS.contains("media.avatar"));
    assert!(EVIDENCE_LIBRARY_JS.contains("sameOriginPath(avatar.localAssetUrl"));
    assert!(EVIDENCE_LIBRARY_CSS.contains(".ev-author-avatar"));
    assert!(!EVIDENCE_LIBRARY_JS.contains("identity_facts.avatar"));
    assert!(!EVIDENCE_LIBRARY_JS.contains("ev-target-fact"));
    assert!(!EVIDENCE_LIBRARY_JS.contains("作品作者"));
    assert!(!EVIDENCE_LIBRARY_CSS.contains(".ev-target-fact"));
    assert!(!EVIDENCE_LIBRARY_CSS.contains(".ev-identity-fact"));
}

/* A creator target only fills the author when the detail has not delivered one, a reported
 * MISMATCH still blocks that fallback, and a keyword target never fills it. The lineage the row
 * no longer shows lives in the Inspector as a single 来源监控 row. */
#[test]
fn evidence_runtime_keeps_collection_lineage_in_the_inspector_only() {
    assert!(EVIDENCE_LIBRARY_JS.contains("来源与溯源"));
    assert!(EVIDENCE_LIBRARY_JS.contains("来源监控"));
    assert!(EVIDENCE_LIBRARY_JS.contains("keyword: '关键词监控'"));
    assert!(EVIDENCE_LIBRARY_JS.contains("context.authorIdentityMatchState !== 'MISMATCH'"));
    assert!(!EVIDENCE_LIBRARY_JS.contains("作者与监控目标"));
    assert!(!EVIDENCE_LIBRARY_JS.contains("identityRelationLabels"));
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
fn evidence_cover_layout_keeps_a_stationary_data_plate_and_one_shared_flip_stage() {
    // Cover mode is a presentation of the existing Work Resource row, not another card model:
    // the row retains its selection handler, while only `.ev-cover-visual` becomes interactive.
    for marker in [
        "function coverVisualBlock(item, ordinal)",
        "function coverMetaBlock(item, material, detailState, observedAt)",
        "function coverGeometryKind(item)",
        "coverCard.append(",
        "visual.addEventListener('click'",
        "window.matchMedia('(hover: none)').matches",
        "event.stopPropagation()",
        "sameOriginPath(cover.localAssetUrl",
    ] {
        assert!(
            EVIDENCE_LIBRARY_JS.contains(marker),
            "missing cover-card contract: {marker}"
        );
    }
    for marker in [
        ".ev-cover-visual",
        ".ev-cover-meta",
        ".ev-cover-stage-frame",
        "aspect-ratio:3/4",
        ".ev-cover-face--back",
        "backface-visibility:hidden",
        ".ev-cover-visual[data-flipped=\"true\"] .ev-cover-flip",
        "@media(hover:hover){.ev-cover-visual:hover .ev-cover-flip",
        ".ev-cover-status-tooltip",
        "column-gap:var(--lgi-space-2);row-gap:var(--lgi-space-8)",
        "aspect-ratio:7/2",
        "inline-size:100%;max-inline-size:100%;box-sizing:border-box",
        "grid-template-rows:var(--lgi-space-6) var(--lgi-space-6) minmax(var(--lgi-space-6),1fr)",
        "min-height:calc(var(--lgi-space-16) + var(--lgi-space-6) + var(--lgi-space-4) + var(--lgi-space-2))",
        "grid-template-columns:minmax(0,1fr) minmax(0,1fr)",
        "grid-template-columns:minmax(0,1fr) minmax(0,10ch)",
    ] {
        assert!(
            EVIDENCE_LIBRARY_CSS.contains(marker),
            "missing cover-card visual contract: {marker}"
        );
    }
    assert!(!EVIDENCE_LIBRARY_CSS.contains(".ev-cover-cross-boundary"));
    assert!(
        !EVIDENCE_LIBRARY_JS.contains("facts.append(material.rail, status)"),
        "the cover data plate must not reintroduce the large multi-colour material rail"
    );
    for marker in [
        "['grid', 'cone', 'frame'][index]",
        "kind === 'stack'",
        "kind === 'cluster'",
    ] {
        assert!(
            EVIDENCE_LIBRARY_JS.contains(marker),
            "the generic catalogue mark family must remain stable and visually differentiated: {marker}"
        );
    }
    assert!(
        EVIDENCE_LIBRARY_JS.contains("function coverObservationMoment(value)"),
        "the cover plate must use a compact, non-overflowing observation timestamp"
    );
    assert!(
        EVIDENCE_LIBRARY_JS.contains("function coverPublishedCopy(item)"),
        "the cover plate must reserve a stable, legible date slot beside the author"
    );
    assert!(
        EVIDENCE_LIBRARY_JS.contains("function coverStateLine(state)"),
        "the cover plate must keep technical state codes out of its compact reading line"
    );
    assert!(
        EVIDENCE_LIBRARY_JS.contains("line.querySelector('.v7-tech-key')?.remove()"),
        "the cover state line must retain the real state label while omitting its technical enum"
    );
    assert!(
        EVIDENCE_LIBRARY_JS.contains(
            "if (item.display?.publishedAtState === 'SOURCE_TEXT_ONLY') return '来源时间';"
        ),
        "a source-text-only publish time must not overrun the compact cover date slot"
    );
}

/* Surface hierarchy, card material, the measuring field and the title step are one contract: the
 * four levels of the cover card must stay distinguishable without a frame around each of them,
 * the grain must stay off every text layer, must stay a state of the shared 8px lattice that
 * `LIDS-MAT-001` builds its materials on *without lining up into a rule*, and the stage lattice
 * must be a different pattern
 * from the grain and must actually cover the whole 3:4 stage. The title step is part of it: a
 * 277px card is a caption surface over its 3:4 stage, so it takes the card step and never the
 * page-title step that made six cards shout over their own material. */
#[test]
fn evidence_cover_card_layers_by_surface_and_keeps_its_materials_apart() {
    for marker in [
        // Four levels, all existing tokens: page / card / stage / plate.
        "--ev-cover-title-size:var(--lgi-text-longform)",
        "border:1px solid var(--lgi-border-strong);background:var(--lgi-canvas-sunken)",
        ".ev-cover-stage-frame{position:relative;display:grid;max-width:100%;height:100%;aspect-ratio:3/4;overflow:hidden;border:1px solid var(--lgi-border-strong);background:var(--lgi-canvas-hi)",
        "padding:var(--lgi-space-3);aspect-ratio:7/2;border-top:1px solid var(--lgi-border-strong);border-bottom:1px solid var(--lgi-hairline);background:var(--lgi-canvas-low)",
        // The card's own material, and the layer that keeps it out of the text.
        ".ev-cover-face::after{content:\"\";position:absolute;inset:0;z-index:1;pointer-events:none;background-image:radial-gradient",
        ".ev-cover-face-inner{position:relative;z-index:2;",
        "background-size:24px 24px",
        // The selected card keeps the same material on orange, denser.
        ".ev-work-row[aria-selected=\"true\"] .ev-cover-face::after{background-image:radial-gradient",
        // The measuring field: a real 8px / 1px lattice, painted by CSS over the whole stage.
        ".ev-cover-stage-frame--diagram::before{content:\"\";position:absolute;inset:0;z-index:0;pointer-events:none;background-image:radial-gradient(circle,var(--lgi-ink) 1px,transparent 1.2px);background-size:8px 8px",
        ".ev-cover-geometry{position:relative;z-index:1;width:100%;height:100%;color:var(--v7-black)}",
        // The original cover fills the very same stage box as the diagram. The frame's height
        // comes from the shared sizer, so a percentage height on the cover resolved only
        // sometimes and an off-ratio cover hung below the frame instead of being cropped.
        ".ev-cover-stage-frame--cover img{position:absolute;inset:0;width:100%;height:100%;object-fit:cover}",
    ] {
        assert!(
            EVIDENCE_LIBRARY_CSS.contains(marker),
            "missing cover-card surface contract: {marker}"
        );
    }
    // The card grain is registered as `LIDS-MAT-001` `M-06`, and what keeps it in that family is
    // that it is a *state of the same 8px lattice* rather than a second texture system: every node
    // it draws, and the tile it repeats on, must be a whole number of lattice steps. That is
    // checked as a property, because a pinned pixel string can be re-pinned to a number that has
    // left the lattice and stay green.
    //
    // Lattice membership alone is not enough, and the first version of this material is the proof:
    // it kept three of the nine nodes of a 24px cell that formed a *subgroup* of the lattice
    // (y = 2x mod 3), and a subgroup makes every dot the first of an endless run — the field was
    // a 45 degree ruling at 11.31px, which is the "texture you can see" that this material must
    // never be. So the residue is also checked for what makes it a residue: no three of its dots
    // may be evenly spaced along one line at a spacing under 17px — the tile holds no three-term
    // arithmetic progression shorter than that. *Evenly* is the whole of the condition, not a
    // softening of it: on a 24px cell any three nodes either put two of them in one row or column
    // — which the repeat turns into a chain of alternating 8px and 16px gaps, two gaps of
    // different length and so no single rhythm to read as a line — or they are one per column,
    // and every such arrangement is y = ax + b, a coset of a subgroup, i.e. the ruled version
    // again. No three-node residue can be perfectly isotropic; this is the strongest condition
    // the density admits.
    //
    // Density is capped the same way, and it is the criterion most likely to be edited by hand
    // ("make it a little stronger"), so it is asserted rather than left to taste: the family's own
    // ink, the alpha carrying the density, and the dot under a sub-perceptible radius.
    const LATTICE_STEP: usize = 8;
    const MIN_RUN_SPACING: f64 = 17.0;
    const MAX_DENSITY: f64 = 0.20;
    const MAX_DOT_RADIUS: f64 = 0.6;
    assert_eq!(
        EVIDENCE_LIBRARY_CSS
            .matches(".ev-cover-face::after{")
            .count(),
        2,
        "the card grain is one material with one selected state; a third rule would override it \
         outside every check below"
    );
    for (rule, expected_nodes, tile) in [
        (".ev-cover-face::after{content:", 3usize, 24usize),
        (
            ".ev-work-row[aria-selected=\"true\"] .ev-cover-face::after{background-image:",
            4usize,
            24usize,
        ),
    ] {
        let start = EVIDENCE_LIBRARY_CSS
            .find(rule)
            .unwrap_or_else(|| panic!("the M-06 grain rule is missing: {rule}"));
        let rule_body = &EVIDENCE_LIBRARY_CSS[start..];
        let rule_body = &rule_body[..rule_body.find('}').expect("the M-06 rule must be closed")];
        assert_eq!(
            rule_body.matches("circle at ").count(),
            expected_nodes,
            "M-06 {rule} must draw {expected_nodes} nodes of the lattice"
        );
        let mut drawn: Vec<(usize, usize)> = Vec::new();
        for node in rule_body.split("circle at ").skip(1) {
            let (coordinates, ink_and_radius) = node
                .split_once(',')
                .expect("an M-06 node declares its coordinates, then its ink");
            let ink_end = ink_and_radius
                .find(')')
                .expect("an M-06 node declares its ink as a colour function");
            let ink = &ink_and_radius[..=ink_end];
            let alpha: f64 = ink
                .strip_prefix("rgba(17,19,21,")
                .and_then(|rest| rest.strip_suffix(')'))
                .unwrap_or_else(|| {
                    panic!("an M-06 node draws in the family ink (17,19,21), never a second colour: {ink}")
                })
                .parse()
                .unwrap_or_else(|_| panic!("an M-06 density is a number: {ink}"));
            assert!(
                (0.0..=MAX_DENSITY).contains(&alpha),
                "M-06 {rule} paints at alpha {alpha}, past the sub-perceptible cap {MAX_DENSITY}"
            );
            let radius: f64 = ink_and_radius[ink_end + 1..]
                .split(',')
                .next()
                .expect("an M-06 node declares the radius of its dot")
                .trim()
                .strip_suffix("px")
                .and_then(|number| number.parse().ok())
                .unwrap_or_else(|| panic!("an M-06 dot radius is a px length: {node}"));
            assert!(
                (0.0..=MAX_DOT_RADIUS).contains(&radius),
                "M-06 {rule} paints a {radius}px dot, past the sub-perceptible cap \
                 {MAX_DOT_RADIUS}px"
            );
            let parts: Vec<&str> = coordinates.split_whitespace().collect();
            assert_eq!(
                parts.len(),
                2,
                "an M-06 node is two coordinates, x then y, in space-separated px: {coordinates}"
            );
            let mut pair = (0usize, 0usize);
            for (axis, part) in parts.iter().enumerate() {
                let part = part.to_ascii_lowercase();
                let length: f64 = match part.strip_suffix("px") {
                    Some(number) => number.parse().unwrap_or_else(|_| {
                        panic!("an M-06 node coordinate is not a length: {node}")
                    }),
                    // CSS lets a bare zero stand for a length. Any other unitless number makes
                    // the whole declaration invalid and the layer would vanish without a word,
                    // so it has to fail here instead of quietly painting nothing.
                    None => {
                        let bare: f64 = part.parse().unwrap_or_else(|_| {
                            panic!("an M-06 node coordinate must carry its px unit: {node}")
                        });
                        assert_eq!(
                            bare, 0.0,
                            "an M-06 node coordinate must carry its px unit: {node}"
                        );
                        bare
                    }
                };
                assert!(
                    (0.0..tile as f64).contains(&length),
                    "M-06 {rule} draws a node outside its own tile, where the repeat hides it: \
                     {coordinates}"
                );
                assert_eq!(
                    length % LATTICE_STEP as f64,
                    0.0,
                    "M-06 {rule} draws off the {LATTICE_STEP}px lattice: {coordinates}"
                );
                let step = (length / LATTICE_STEP as f64) as usize;
                if axis == 0 {
                    pair.0 = step;
                } else {
                    pair.1 = step;
                }
            }
            drawn.push(pair);
        }
        assert_eq!(
            rule_body.matches("background-size:").count(),
            1,
            "M-06 {rule} must declare the tile it repeats on exactly once"
        );
        let size = rule_body
            .split("background-size:")
            .nth(1)
            .expect("M-06 must declare the tile it repeats on");
        assert_eq!(
            size.split(';')
                .next()
                .expect("the tile ends the declaration")
                .trim(),
            format!("{tile}px {tile}px"),
            "M-06 {rule} must repeat on a whole number of lattice steps"
        );
        // A dot that has a companion one step away and another two steps away is the head of a
        // run; at this spacing that run is what a ruled pattern is made of. One probe per step
        // vector is enough *because the tile is three lattice steps wide*: a shorter run than that
        // would have to close at a smaller multiple of the same vector and is caught by the same
        // probe. A material on a wider tile would need the general test — any three collinear
        // nodes, not only evenly spaced ones — so this loop must not be carried over unchanged.
        let span = tile / LATTICE_STEP;
        let reach = (MIN_RUN_SPACING / LATTICE_STEP as f64).floor() as i32;
        for dx in -reach..=reach {
            for dy in -reach..=reach {
                if (dx, dy) == (0, 0) {
                    continue;
                }
                let spacing = LATTICE_STEP as f64 * ((dx * dx + dy * dy) as f64).sqrt();
                if spacing >= MIN_RUN_SPACING {
                    continue;
                }
                for &(x, y) in &drawn {
                    let on_line = |multiple: i32| {
                        let px = (x as i32 + multiple * dx).rem_euclid(span as i32) as usize;
                        let py = (y as i32 + multiple * dy).rem_euclid(span as i32) as usize;
                        drawn.contains(&(px, py))
                    };
                    assert!(
                        !(on_line(1) && on_line(2)),
                        "M-06 {rule} draws three dots on one line {spacing:.2}px apart, which \
                         reads as ruled paper instead of a residue: {drawn:?}"
                    );
                }
            }
        }
    }
    // One type step for every card, and a plate that is exactly two lines of it.
    assert!(
        EVIDENCE_LIBRARY_CSS.contains(
            "--ev-cover-title-height:calc(var(--ev-cover-title-size) * var(--lgi-lh-title) * 2)"
        ),
        "the title plate must be exactly two lines of the card's own title step"
    );
    assert!(
        !EVIDENCE_LIBRARY_CSS.contains("data-two-line"),
        "a per-card title downgrade would put two different type sizes on one row of cards"
    );
    assert!(
        !EVIDENCE_LIBRARY_CSS.contains(".ev-cover-dot-field")
            && !EVIDENCE_LIBRARY_CSS.contains(".ev-cover-corner"),
        "the stage field is a CSS tile and the stage frame carries no corner ticks"
    );
    assert!(
        !EVIDENCE_LIBRARY_JS.contains("ev-cover-dot-field")
            && !EVIDENCE_LIBRARY_JS.contains("coverCorners"),
        "a ratio-derived inline SVG cannot measure the whole 3:4 stage, so the field must not go back to one"
    );
    // Every template is drawn inside the one safe area of the 300x400 viewBox — x 48..252
    // (68%) and y 80..320 (60%). The band is only as good as the coordinates that draw it, so
    // every literal the six templates use is pinned: pinning a subset leaves the rest free to
    // drift out of the band unnoticed (raising the stack's rect from 160x170 to 260x260 puts it
    // at x 70..330 / y 150..410 and a subset pin stays green).
    for marker in [
        "[24, 48, 72, 96].forEach((radius) => append('circle', { cx: 150, cy: 200, r: radius }))",
        "append('path', { d: 'M54 200h192M150 104v192' })",
        "[[50, 90], [60, 120], [70, 150]].forEach(([x, y]) => append('rect', {",
        "x, y, width: 160, height: 170,",
        "append('path', { d: 'M80 140h100M80 180h100M80 220h100' })",
        "[[110, 180, 60], [190, 180, 60], [150, 240, 60]].forEach(([cx, cy, r]) => append('circle', { cx, cy, r }))",
        "append('path', { d: 'M125 260 175 300M140 222l60 56' })",
        "append('rect', { x: 48, y: 80, width: 204, height: 240 })",
        "[99, 150, 201].forEach((position) => append('path', { d: `M${position} 80v240` }))",
        "[140, 200, 260].forEach((position) => append('path', { d: `M48 ${position}h204` }))",
        "append('path', { d: 'M48 80 150 200 252 80M48 320 150 200 252 320' })",
        "append('rect', { x: 85, y: 123, width: 130, height: 154 })",
        "append('path', { d: 'M85 174h130M85 226h130M150 123v154' })",
        "append('path', { d: 'M110 148 150 180 190 148M110 252 150 220 190 252' })",
        "append('path', { d: 'M150 80 48 320h204Z' })",
        "[96, 144, 192].forEach((position) => append('path', { d: `M150 80 ${position} 320` }))",
        "append('path', { d: 'M78 246h144' })",
    ] {
        assert!(
            EVIDENCE_LIBRARY_JS.contains(marker),
            "every catalogue mark must share one safe area so visual weight does not jump: {marker}"
        );
    }
    // The comment is the only place the 68% / 60% claim itself is written down.
    assert!(
        EVIDENCE_LIBRARY_JS.contains("x 48..252 (68% of the")
            && EVIDENCE_LIBRARY_JS.contains("y 80..320 (60% of its height)"),
        "the geometry comment must keep naming the safe area the templates above are drawn in"
    );
}

/* The results band that carried the layout legend and the inspector reopen button is gone; the
 * read timestamp it used to hold hangs on the work list instead of losing its checkable home. */
#[test]
fn evidence_results_band_stays_removed() {
    let html = evidence_library_html(None, &[], None);
    for gone in [
        "ev-results-head",
        "ev-results-legend",
        "ev-reopen-inspector",
    ] {
        assert!(
            !html.contains(gone),
            "the results band was removed on request and must not return: {gone}"
        );
    }
    // The CSS ships as its own asset, so an HTML-only check would let a stylesheet rule for the
    // removed band come back as dead code under a passing test.
    for gone in [
        "ev-inspector-reopen",
        "ev-results-head",
        "ev-results-legend",
    ] {
        assert!(
            !EVIDENCE_LIBRARY_CSS.contains(gone),
            "a stylesheet rule for the removed results band is dead code: {gone}"
        );
    }
    assert!(!EVIDENCE_LIBRARY_JS.contains("reopenInspector"));
    assert!(
        EVIDENCE_LIBRARY_JS.contains("refs.list.title = `读取时间"),
        "the read timestamp must stay checkable after its header row was removed"
    );
}

#[test]
fn evidence_runtime_preserves_unknown_partial_and_restricted_states() {
    for state in [
        "UNKNOWN",
        "SOURCE_TEXT_ONLY",
        "PARTIAL",
        "RETIRED",
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
fn evidence_runtime_exposes_ocr_layering_without_reintroducing_invalid_tokens() {
    for marker in [
        "function appendOcrLayering",
        "image_substantive_text: '图片实质文字'",
        "cleanCorpusEligible",
        "dataset.evOption = 'RETIRED'",
    ] {
        assert!(
            EVIDENCE_LIBRARY_JS.contains(marker),
            "OCR-layering runtime contract marker missing: {marker}"
        );
    }
    for invalid_token in ["lgi-ink-muted", "lgi-text-micro", "lgi-ls-label"] {
        assert!(
            !EVIDENCE_LIBRARY_CSS.contains(invalid_token),
            "OCR-layering CSS must not refer to the removed token {invalid_token}"
        );
    }
}

#[test]
fn evidence_runtime_keeps_sensitive_text_and_media_inside_controlled_detail_reads() {
    assert!(EVIDENCE_OBSERVATION_JS.contains("本机授权评论研究"));
    assert!(EVIDENCE_OBSERVATION_JS.contains("作者只显示匿名上下文"));
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
