#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
#[path = "support/comment_research_fixture.rs"]
mod research_fixture;
use linggan_evidence::comment_research_read::*;
use linggan_intelligence::{
    comment_daily::*, comment_research_projection::*, model_invocation::*, model_secrets::*,
    model_settings::*, pi_adapter::*,
};
use linggan_storage_postgres::Database;
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;
#[path = "support/comment_daily_fixture.rs"]
mod daily_fixture;
use daily_fixture::*;
#[path = "support/comment_daily_semantic_cases.rs"]
mod semantic_cases;

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn work_packet_is_one_call_with_exact_results_and_restriction_propagation() {
    let db = fixture::proof_database("daily_work_packet").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    research_fixture::detail(&db, "work-one", "合成作品").await;
    let a = source(&db, "work-one", "a", "没有效果  ，我每天提醒也没有用").await;
    let b = source(&db, "work-one", "b", "我尝试过陪写，还是每天很晚").await;
    let low = source(&db, "work-one", "low", "😭😭😭").await;
    let batch = selected(&db, config, vec![a, b, low], 100000).await;
    assert!(
        run_daily_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
    assert!(
        !run_daily_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_comment_daily_packet WHERE batch_ref=$1")
            .bind(batch)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(count, 1);
    let data = batch_detail(&db, batch, None).await.unwrap();
    assert_eq!(data["items"].as_array().unwrap().len(), 3);
    assert_eq!(
        data["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| i["state"] == "succeeded")
            .count(),
        2
    );
    assert_eq!(
        data["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| i["state"] == "low_information")
            .count(),
        1
    );
    let annotations = read_comment_research_annotations(&db, a).await.unwrap();
    assert_eq!(annotations["analysis"][0]["state"], "succeeded");
    assert_eq!(
        annotations["analysis"][0]["result"]["modelVersion"],
        annotations["analysis"][0]["modelVersion"]
    );
    restrict_comment_research_source(&db, b, "synthetic restriction")
        .await
        .unwrap();
    assert!(
        read_comment_research_annotations(&db, a).await.unwrap()["analysis"][0]["result"].is_null()
    );
    server.kill().await.unwrap();
}
#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn daily_windows_are_gapless_deduplicate_reobservations_and_allow_delayed_cleaning() {
    let db = fixture::proof_database("daily_windows").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    clock(&db, "2026-09-07 14:00:00Z").await;
    save_schedule(
        &db,
        &DailySchedule {
            expected_revision: 0,
            enabled: true,
            config_ref: config,
            source_limit: 100,
            token_limit: 100000,
        },
    )
    .await
    .unwrap();
    let first = source(&db, "first", "same", "今天首次入库的评论").await;
    clock(&db, "2026-09-07 15:00:00Z").await;
    let late = source(&db, "late", "late", "23点整入库，进入下批").await;
    assert!(seal_due(&db).await.unwrap());
    assert!(!seal_due(&db).await.unwrap());
    let rows = sqlx::query("SELECT source_ref FROM linggan_comment_daily_item")
        .fetch_all(db.pool())
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<Uuid, _>("source_ref"), first);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_comment_clean")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
    clock(&db, "2026-09-08 14:00:00Z").await;
    source(&db, "first", "same", "今天首次入库的评论").await;
    clock(&db, "2026-09-09 16:00:00Z").await;
    assert!(seal_due(&db).await.unwrap());
    assert!(seal_due(&db).await.unwrap());
    assert!(!seal_due(&db).await.unwrap());
    let refs: Vec<Uuid> =
        sqlx::query_scalar("SELECT source_ref FROM linggan_comment_daily_item ORDER BY source_ref")
            .fetch_all(db.pool())
            .await
            .unwrap();
    assert_eq!(refs.len(), 2);
    assert!(refs.contains(&late));
    clean_pending(&db).await.unwrap();
    let n:i64=sqlx::query_scalar("SELECT count(*) FROM linggan_comment_daily_item i JOIN linggan_comment_clean c USING(source_ref)").fetch_one(db.pool()).await.unwrap();
    assert_eq!(n, 2);
    server.kill().await.unwrap();
}
#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn selection_replay_pause_and_budget_do_not_duplicate_dispatch() {
    let db = fixture::proof_database("daily_budget").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    let a = source(&db, "a", "a", "一条需要研究的评论").await;
    let b = source(&db, "b", "b", "另一篇作品的评论").await;
    let id = Uuid::new_v4();
    let request = SelectedBatch {
        reanalyze: false,
        batch_ref: id,
        config_ref: config,
        source_refs: vec![a, b],
        token_limit: 18000,
    };
    create_selected(&db, &request).await.unwrap();
    assert_eq!(
        create_selected(&db, &request).await.unwrap()["replayed"],
        true
    );
    set_batch_enabled(&db, id, false).await.unwrap();
    assert!(
        !run_daily_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
    set_batch_enabled(&db, id, true).await.unwrap();
    assert!(
        run_daily_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
    assert!(
        !run_daily_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
    let detail = batch_detail(&db, id, None).await.unwrap();
    assert_eq!(
        detail["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| i["state"] == "pending")
            .count(),
        1
    );
    server.kill().await.unwrap();
}
#[tokio::test]
#[ignore = "isolated PostgreSQL"]
async fn comment_facts_keep_zero_and_unknown_separate() {
    let db = fixture::proof_database("daily_comment_facts").await;
    let package=fixture::submit_package(&db,"comments",json!({"contentExternalId":"facts"}),json!({"kind":"comment","sourceObject":{"platform":"xhs","type":"content","externalId":"facts"},"payload":{"commentId":"zero","noteId":"facts","text":"零点赞的完整表达","likes":0,"publishedAt":1788750000000_i64}})).await;
    let id: Uuid = sqlx::query_scalar(
        "SELECT material_ref FROM linggan_material_comment WHERE package_ref=$1",
    )
    .bind(package)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let s = read_comment_research_source(&db, id).await.unwrap();
    assert_eq!(s.likes, Some(0));
    assert_eq!(s.published_at, Some(1788750000000));
    assert!(s.accepted_at.is_some());
    let unknown = source(&db, "facts", "unknown", "没有附带点赞的表达").await;
    assert_eq!(
        read_comment_research_source(&db, unknown)
            .await
            .unwrap()
            .likes,
        None
    );
}
#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn partial_packet_keeps_good_comments_and_concurrent_ticks_dispatch_once() {
    let db = fixture::proof_database("daily_partial").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    let a = source(&db, "partial", "a", "这条评论有完整的表达").await;
    let b = source(&db, "partial", "b", "[BAD_QUOTE] 不接受虚构引用").await;
    let c = source(&db, "partial", "c", "[MISSING] 模型没有回答此条").await;
    let d = source(&db, "partial", "d", "[NO_SIGNAL] 这条没有信号").await;
    let batch = selected(&db, config, vec![a, b, c, d], 100000).await;
    let adapter = PiAdapter::configured();
    let (x, y) = tokio::join!(
        run_daily_once(&db, &SyntheticModelSecrets, &adapter),
        run_daily_once(&db, &SyntheticModelSecrets, &adapter)
    );
    assert_ne!(x.unwrap(), y.unwrap());
    let data = batch_detail(&db, batch, None).await.unwrap();
    let items = data["items"].as_array().unwrap();
    assert_eq!(items.iter().filter(|i| i["state"] == "failed").count(), 2);
    assert_eq!(
        items.iter().filter(|i| i["state"] == "succeeded").count(),
        1
    );
    assert_eq!(
        items.iter().filter(|i| i["state"] == "no_signal").count(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_comment_daily_packet WHERE batch_ref=$1"
        )
        .bind(batch)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        1
    );
    server.kill().await.unwrap();
}
#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn interrupted_packet_keeps_unknown_reservation_and_does_not_silently_rebill() {
    let db = fixture::proof_database("daily_interrupted").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    let a = source(&db, "interrupt", "a", "[HANG_PROVIDER] 模拟进程中断").await;
    let batch = selected(&db, config, vec![a], 100000).await;
    let db2 = db.clone();
    let handle = tokio::spawn(async move {
        run_daily_once(&db2, &SyntheticModelSecrets, &PiAdapter::configured()).await
    });
    for _ in 0..100 {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM linggan_comment_daily_packet WHERE batch_ref=$1)",
        )
        .bind(batch)
        .fetch_one(db.pool())
        .await
        .unwrap();
        if exists {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    handle.abort();
    let _ = handle.await;
    sqlx::query("UPDATE linggan_comment_daily_packet SET lease_until=scope_001_now()-interval '1 second' WHERE batch_ref=$1").bind(batch).execute(db.pool()).await.unwrap();
    assert!(
        !run_daily_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
    let row=sqlx::query("SELECT v.charged_tokens,v.input_tokens,v.failure_code FROM linggan_comment_daily_packet p JOIN linggan_model_invocation v USING(invocation_ref) WHERE p.batch_ref=$1").bind(batch).fetch_one(db.pool()).await.unwrap();
    assert_eq!(row.get::<i64, _>("charged_tokens"), 18000);
    assert!(row.get::<Option<i64>, _>("input_tokens").is_none());
    assert_eq!(row.get::<String, _>("failure_code"), "worker_interrupted");
    let command = Uuid::new_v4();
    assert_eq!(
        retry_failed(&db, batch, command).await.unwrap()["queued"],
        0
    );
    assert_eq!(
        retry_failed(&db, batch, command).await.unwrap()["queued"],
        0
    );
    let invocation: Uuid = sqlx::query_scalar(
        "SELECT invocation_ref FROM linggan_comment_daily_packet WHERE batch_ref=$1",
    )
    .bind(batch)
    .fetch_one(db.pool())
    .await
    .unwrap();
    review_usage(
        &db,
        batch,
        &ReviewUsage {
            command_ref: Uuid::new_v4(),
            invocation_ref: invocation,
            input_tokens: None,
            output_tokens: None,
            accept_duplicate_charge: true,
            reason: "合成验收明确接受重复计费风险".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        retry_failed(&db, batch, Uuid::new_v4()).await.unwrap()["queued"],
        1
    );
    let charged:i64=sqlx::query_scalar("SELECT sum(v.charged_tokens)::bigint FROM linggan_comment_daily_packet p JOIN linggan_model_invocation v USING(invocation_ref) WHERE p.batch_ref=$1").bind(batch).fetch_one(db.pool()).await.unwrap();
    assert_eq!(charged, 18000);
    server.kill().await.unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL"]
async fn cleaning_filter_searches_all_sources_and_binds_cursor() {
    let db = fixture::proof_database("daily_filter").await;
    for i in 0..25 {
        source(&db, "filter", &format!("low-{i}"), "😭😭😭").await;
    }
    source(&db, "filter", "normal", "详细描述一个具体问题").await;
    clean_pending(&db).await.unwrap();
    let members = cleaning_members(&db, "low_information").await.unwrap();
    let page = read_comment_research_subset(
        &db,
        &CommentResearchQuery::default(),
        Some(&members),
        "low_information",
    )
    .await
    .unwrap();
    assert_eq!(page.total, 25);
    assert_eq!(page.items.len(), 20);
    let q = CommentResearchQuery {
        cursor: page.next_cursor,
        ..Default::default()
    };
    assert_eq!(
        read_comment_research_subset(&db, &q, Some(&members), "low_information")
            .await
            .unwrap()
            .items
            .len(),
        5
    );
    assert!(
        read_comment_research_subset(&db, &q, Some(&members), "direct")
            .await
            .is_err()
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn daily_permission_cannot_be_duplicated_by_legacy_auto_and_pauses_without_connection() {
    use linggan_intelligence::model_plans::{StartModelPlan, start_model_plan};
    let db = fixture::proof_database("daily_permissions").await;
    let (mut server, url) = fixture_server().await;
    let (config, connection, _) = configured(&db, &url, "synthetic-good", None).await;
    let mut schedule = DailySchedule {
        expected_revision: 0,
        enabled: true,
        config_ref: config,
        source_limit: 10,
        token_limit: 100000,
    };
    save_schedule(&db, &schedule).await.unwrap();
    let automatic = StartModelPlan {
        plan_ref: Uuid::new_v4(),
        config_ref: config,
        kind: "automatic".into(),
        source_refs: vec![],
        source_limit: 10,
        token_limit: 100000,
        expected_auto_plan_ref: None,
    };
    assert!(matches!(
        start_model_plan(&db, &automatic).await,
        Err(ModelError::ResearchPlanRetired)
    ));
    set_model_connection_enabled(
        &db,
        &SetModelConnectionEnabled {
            connection_ref: connection,
            expected_revision: 1,
            enabled: false,
        },
    )
    .await
    .unwrap();
    schedule.expected_revision = 1;
    schedule.enabled = false;
    save_schedule(&db, &schedule).await.unwrap();
    assert_eq!(overview(&db).await.unwrap()["schedule"]["enabled"], false);
    server.kill().await.unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn runtime_trace_reports_exact_packet_outcomes_candidates_and_source_access() {
    use linggan_intelligence::{
        comment_intelligence::{ResearchScope, read},
        comment_runtime::*,
    };
    let db = fixture::proof_database("runtime_trace_outcomes").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    research_fixture::detail(&db, "runtime-work", "合成运行详情").await;
    let a = source(&db, "runtime-work", "good", "我需要更省精力的办法").await;
    let b = source(&db, "runtime-work", "bad", "[BAD_QUOTE] 没有真正引用我的话").await;
    let c = source(&db, "runtime-work", "none", "[NO_SIGNAL] 合成无信号评论").await;
    let batch = selected(&db, config, vec![a, b, c], 100000).await;
    assert!(
        run_daily_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
    let detail = batch_detail(&db, batch, None).await.unwrap();
    assert_eq!(detail["callTotal"], 1);
    let call = &detail["calls"][0];
    assert_eq!(call["requestedComments"], 3);
    assert_eq!(call["acceptedComments"], 2);
    assert_eq!(call["succeededComments"], 1);
    assert_eq!(call["noSignalComments"], 1);
    assert_eq!(call["failedComments"], 1);
    assert_eq!(call["state"], "partial");
    assert_eq!(call["modelId"], "synthetic-good");
    assert_eq!(call["workTitle"], "合成运行详情");
    let invocation = Uuid::parse_str(call["invocationRef"].as_str().unwrap()).unwrap();
    let own = Uuid::parse_str("00000000-0000-4000-8000-000000000001").unwrap();
    let trace = request_detail(&db, batch, invocation, own).await.unwrap();
    assert_eq!(trace["availability"], "AVAILABLE");
    assert_eq!(trace["input"]["comments"].as_array().unwrap().len(), 3);
    assert!(trace["input"]["outputSchema"].is_object());
    assert_eq!(trace["validation"].as_array().unwrap().len(), 1);
    assert!(
        trace["validation"][0]["path"]
            .as_str()
            .unwrap()
            .contains("quote")
    );
    assert_eq!(trace["events"].as_array().unwrap().len(), 4);
    assert!(
        request_detail(&db, Uuid::new_v4(), invocation, own)
            .await
            .is_err()
    );
    assert!(
        request_detail(&db, batch, invocation, Uuid::new_v4())
            .await
            .is_err()
    );
    let scope = ResearchScope {
        domain: Some(own),
        from: Some("2020-01-01T00:00:00Z".into()),
        to: Some("2099-01-01T00:00:00Z".into()),
        ..Default::default()
    };
    linggan_intelligence::comment_intelligence_problems::reconcile_problem_index(&db, 100)
        .await
        .unwrap();
    let view = read(&db, &scope).await.unwrap();
    assert_eq!(view["candidateTotal"], 1);
    let daily = read(
        &db,
        &ResearchScope {
            view: Some("daily".into()),
            batch_ref: Some(batch),
            from: Some("2020-01-01T00:00:00Z".into()),
            to: Some("2020-02-01T00:00:00Z".into()),
            ..scope.clone()
        },
    )
    .await
    .unwrap();
    assert_eq!(daily["summary"]["comments"], 3);
    assert_eq!(daily["daily"]["items"][0]["total"], 3);
    assert_eq!(
        view["problemCandidates"][0]["evidence"][0]["quote"],
        "我需要更省精力的办法"
    );
    assert_eq!(view["representatives"].as_array().unwrap().len(), 2);
    assert_eq!(view["problems"].as_array().unwrap().len(), 0);
    assert_eq!(view["problemCandidates"][0]["sourceRefs"], json!([a]));
    assert_eq!(
        read(
            &db,
            &ResearchScope {
                processing_state: Some("no_signal".into()),
                ..scope.clone()
            }
        )
        .await
        .unwrap()["page"]["total"],
        1
    );
    let later: String = sqlx::query_scalar("SELECT (scope_001_now()+interval '1 second')::text")
        .fetch_one(db.pool())
        .await
        .unwrap();
    clock(&db, &later).await;
    let pending_batch = selected(&db, config, vec![a], 100000).await;
    let original = read(
        &db,
        &ResearchScope {
            batch_ref: Some(batch),
            processing_state: Some("succeeded".into()),
            ..scope.clone()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        original["page"]["total"], 1,
        "later pending run must not erase the original batch result"
    );
    assert_eq!(
        read(
            &db,
            &ResearchScope {
                batch_ref: Some(pending_batch),
                ..scope.clone()
            }
        )
        .await
        .unwrap()["summary"]["analyzed"],
        0,
        "unexecuted batch must not claim another batch result"
    );
    restrict_comment_research_source(&db, b, "synthetic restriction")
        .await
        .unwrap();
    assert_eq!(
        request_detail(&db, batch, invocation, own).await.unwrap()["availability"],
        "RESTRICTED"
    );
    assert_eq!(read(&db, &scope).await.unwrap()["candidateTotal"], 0);
    expire_content(&db).await.unwrap();
    let retained:bool=sqlx::query_scalar("SELECT input_content IS NULL AND output_content IS NULL FROM linggan_comment_request_trace WHERE invocation_ref=$1").bind(invocation).fetch_one(db.pool()).await.unwrap();
    assert!(retained);
    server.kill().await.unwrap();
}
#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn runtime_context_is_frozen_by_preparation_and_recording_can_be_disabled() {
    use linggan_intelligence::{
        comment_intelligence::{Prepare, ResearchScope, Run, prepare, run},
        comment_runtime::*,
    };
    let db = fixture::proof_database("runtime_context_freeze").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    research_fixture::detail(&db, "runtime-policy", "合成冻结上下文").await;
    let a = source(&db, "runtime-policy", "a", "希望减少监督成本").await;
    let own = Uuid::parse_str("00000000-0000-4000-8000-000000000001").unwrap();
    let policy = ContextPolicy {
        work_body: false,
        parent: false,
        ocr: false,
        asr: false,
        existing_problems: false,
        record_content: false,
        max_comments: 1,
        ..Default::default()
    };
    save_settings(
        &db,
        &SaveContext {
            expected_revision: 0,
            policy: policy.clone(),
        },
    )
    .await
    .unwrap();
    let prepared = prepare(
        &db,
        &Prepare {
            scope: ResearchScope {
                domain: Some(own),
                from: Some("2020-01-01T00:00:00Z".into()),
                to: Some("2099-01-01T00:00:00Z".into()),
                ..Default::default()
            },
            source_refs: Some(vec![a]),
            reanalyze: false,
        },
    )
    .await
    .unwrap();
    let batch = Uuid::parse_str(prepared["prepareRef"].as_str().unwrap()).unwrap();
    save_settings(
        &db,
        &SaveContext {
            expected_revision: 1,
            policy: ContextPolicy::default(),
        },
    )
    .await
    .unwrap();
    assert!(
        save_settings(
            &db,
            &SaveContext {
                expected_revision: 1,
                policy: policy.clone()
            }
        )
        .await
        .is_err()
    );
    run(
        &db,
        &Run {
            prepare_ref: batch,
            config_ref: config,
            token_limit: 100000,
            reanalyze: false,
        },
    )
    .await
    .unwrap();
    assert!(
        run_daily_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
    let d = batch_detail(&db, batch, None).await.unwrap();
    assert_eq!(d["calls"][0]["contextPolicy"], json!(policy));
    let inv = Uuid::parse_str(d["calls"][0]["invocationRef"].as_str().unwrap()).unwrap();
    let trace = request_detail(&db, batch, inv, own).await.unwrap();
    assert_eq!(trace["availability"], "NOT_RECORDED");
    assert!(trace["input"].is_null());
    assert!(trace["output"].is_null());
    assert_eq!(trace["events"].as_array().unwrap().len(), 4);
    let automatic: bool =
        sqlx::query_scalar("SELECT enabled FROM linggan_comment_daily_schedule WHERE singleton")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert!(!automatic);
    clock(&db, "2026-09-08 14:00:00Z").await;
    save_schedule(
        &db,
        &DailySchedule {
            expected_revision: 0,
            enabled: true,
            config_ref: config,
            source_limit: 100,
            token_limit: 100000,
        },
    )
    .await
    .unwrap();
    save_settings(
        &db,
        &SaveContext {
            expected_revision: 2,
            policy: policy.clone(),
        },
    )
    .await
    .unwrap();
    clock(&db, "2026-09-08 15:00:00Z").await;
    assert!(seal_due(&db).await.unwrap());
    let daily_policy: serde_json::Value = sqlx::query_scalar(
        "SELECT context_policy FROM linggan_comment_daily_batch WHERE kind='daily'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(daily_policy, json!(policy));
    server.kill().await.unwrap();
}
#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn runtime_expiry_and_historical_absence_are_distinct_without_reconstructing_content() {
    use linggan_intelligence::comment_runtime::*;
    let db = fixture::proof_database("runtime_expiry").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    research_fixture::detail(&db, "runtime-expiry", "合成保留期限").await;
    let a = source(&db, "runtime-expiry", "a", "希望有人解释操作步骤").await;
    let batch = selected(&db, config, vec![a], 100000).await;
    run_daily_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
        .await
        .unwrap();
    let own = Uuid::parse_str("00000000-0000-4000-8000-000000000001").unwrap();
    let inv: Uuid = sqlx::query_scalar(
        "SELECT invocation_ref FROM linggan_comment_daily_packet WHERE batch_ref=$1",
    )
    .bind(batch)
    .fetch_one(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE linggan_comment_request_trace SET expires_at=scope_001_now()-interval '1 second'",
    )
    .execute(db.pool())
    .await
    .unwrap();
    let expired = request_detail(&db, batch, inv, own).await.unwrap();
    assert_eq!(expired["availability"], "EXPIRED");
    assert!(expired["input"].is_null());
    expire_content(&db).await.unwrap();
    sqlx::query("DELETE FROM linggan_comment_request_trace WHERE invocation_ref=$1")
        .bind(inv)
        .execute(db.pool())
        .await
        .unwrap();
    assert_eq!(
        request_detail(&db, batch, inv, own).await.unwrap()["availability"],
        "NOT_RECORDED"
    );
    server.kill().await.unwrap();
}

#[tokio::test]
#[ignore = "isolated synthetic browser preview, never production"]
async fn prepare_runtime_browser_preview() {
    let db = fixture::proof_database("comment_runtime_preview").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    for (id, sql) in [
        (
            "0001_scope_001_capture_evidence",
            include_str!("../../../database/migrations/0001_scope_001_capture_evidence.sql"),
        ),
        (
            "0002_local_001_discovery",
            include_str!("../../../database/migrations/0002_local_001_discovery.sql"),
        ),
    ] {
        sqlx::query("INSERT INTO linggan_local_schema_migration(migration_id,migration_sha256) VALUES($1,$2) ON CONFLICT DO NOTHING").bind(id).bind(linggan_intelligence::comment_research::comment_source_hash(sql)).execute(db.pool()).await.unwrap();
    }
    research_fixture::detail(&db, "preview-runtime", "合成材料 · 陪写作业的时间与精力").await;
    let mut refs = vec![];
    for i in 0..123 {
        let text = match i {
            0 => "[BAD_QUOTE] 每天提醒却没有效果，希望知道怎样安排。",
            1 => "[NO_SIGNAL] 我先留下记录，后面继续看看。",
            _ => "我下班还要做饭，每天坐在旁边提醒真的做不到，希望有省精力的办法。",
        };
        let r = source(
            &db,
            "preview-runtime",
            &format!("runtime-{i}"),
            &format!("SYNTHETIC / NOT EVIDENCE {i}：{text}"),
        )
        .await;
        if i < 3 {
            refs.push(r);
        }
    }
    selected(&db, config, refs, 100000).await;
    run_daily_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
        .await
        .unwrap();
    linggan_intelligence::comment_intelligence_problems::reconcile_problem_index(&db, 500)
        .await
        .unwrap();
    server.kill().await.unwrap();
}
