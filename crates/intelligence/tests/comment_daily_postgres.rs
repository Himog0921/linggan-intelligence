#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
#[path = "support/comment_research_fixture.rs"]
mod research_fixture;
use linggan_evidence::comment_research_read::*;
use linggan_intelligence::{
    comment_analysis::CommentAnalysisInput,
    comment_daily::*,
    comment_intelligence::{Prepare, ResearchScope, Run, prepare, read, run},
    comment_research::comment_source_hash,
    comment_research_fingerprint::{
        CleanState, EligibilityCategory, EligibilityRequest, ExecutionState, RecoveryPolicy,
        ResultState, RuleFingerprint, SourceIdentity, SourceQualification, build_semantic_input,
        evaluate_eligibility,
    },
    comment_research_projection::*,
    comment_runtime::*,
    model_invocation::*,
    model_secrets::*,
    model_settings::*,
    pi_adapter::*,
};
use linggan_storage_postgres::Database;
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;
#[path = "support/comment_daily_fixture.rs"]
mod daily_fixture;
use daily_fixture::*;
#[path = "support/comment_field_repair_cases.rs"]
mod field_repair_cases;
#[path = "support/comment_local_recovery_cases.rs"]
mod local_recovery_cases;
#[path = "support/comment_partial_recovery_cases.rs"]
mod partial_recovery_cases;
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
            .filter(|i| i["state"] == "dropped")
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
            auto_policy: AutoPolicy::new_intake_only(100000),
        },
    )
    .await
    .unwrap();
    let first = source(&db, "first", "same", "今天首次入库的评论").await;
    clock(&db, "2026-09-07 15:00:00Z").await;
    let late = source(&db, "late", "late", "23点整入库，进入下批").await;
    assert_eq!(seal_all_due(&db).await.unwrap(), 1);
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
    assert_eq!(seal_all_due(&db).await.unwrap(), 2);
    assert!(!seal_due(&db).await.unwrap());
    let refs: Vec<Uuid> = sqlx::query_scalar(
        "SELECT DISTINCT source_ref FROM linggan_comment_daily_item ORDER BY source_ref",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(refs.len(), 2);
    assert!(refs.contains(&late));
    clean_pending(&db).await.unwrap();
    let n:i64=sqlx::query_scalar("SELECT count(DISTINCT i.source_ref) FROM linggan_comment_daily_item i JOIN linggan_comment_clean c USING(source_ref)").fetch_one(db.pool()).await.unwrap();
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
    let members = cleaning_members(&db, "dropped").await.unwrap();
    let page = read_comment_research_subset(
        &db,
        &CommentResearchQuery::default(),
        Some(&members),
        "dropped",
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
        read_comment_research_subset(&db, &q, Some(&members), "dropped")
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
        auto_policy: AutoPolicy::new_intake_only(100000),
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

async fn runtime_trace_packet(db: &Database, config: Uuid) -> (Uuid, Uuid, Uuid, Uuid) {
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
    (a, b, batch, invocation)
}

async fn assert_runtime_trace_access(db: &Database, batch: Uuid, invocation: Uuid, own: Uuid) {
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
}

fn runtime_trace_scope(own: Uuid) -> ResearchScope {
    ResearchScope {
        domain: Some(own),
        from: Some("2020-01-01T00:00:00Z".into()),
        to: Some("2099-01-01T00:00:00Z".into()),
        ..Default::default()
    }
}

async fn assert_runtime_trace_index(
    db: &Database,
    scope: &ResearchScope,
    batch: Uuid,
    source_ref: Uuid,
) {
    linggan_intelligence::comment_intelligence_problems::reconcile_problem_index(&db, 100)
        .await
        .unwrap();
    let view = read(&db, scope).await.unwrap();
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
    assert_eq!(
        view["problemCandidates"][0]["sourceRefs"],
        json!([source_ref])
    );
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
}

async fn assert_runtime_trace_reuse_and_restriction(
    db: &Database,
    config: Uuid,
    source_ref: Uuid,
    restricted_source_ref: Uuid,
    batch: Uuid,
    invocation: Uuid,
    own: Uuid,
    scope: &ResearchScope,
) {
    let later: String = sqlx::query_scalar("SELECT (scope_001_now()+interval '1 second')::text")
        .fetch_one(db.pool())
        .await
        .unwrap();
    clock(&db, &later).await;
    let pending_batch = selected(&db, config, vec![source_ref], 100000).await;
    assert_eq!(
        batch_detail(&db, pending_batch, None).await.unwrap()["callTotal"],
        0,
        "reused coverage is not a new model invocation"
    );
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
        1,
        "a newly selected scope may reuse an existing current result without a new call"
    );
    restrict_comment_research_source(&db, restricted_source_ref, "synthetic restriction")
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
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn runtime_trace_reports_exact_packet_outcomes_candidates_and_source_access() {
    let db = fixture::proof_database("runtime_trace_outcomes").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    let own = Uuid::parse_str("00000000-0000-4000-8000-000000000001").unwrap();
    let (a, b, batch, invocation) = runtime_trace_packet(&db, config).await;
    assert_runtime_trace_access(&db, batch, invocation, own).await;
    let scope = runtime_trace_scope(own);
    assert_runtime_trace_index(&db, &scope, batch, a).await;
    assert_runtime_trace_reuse_and_restriction(&db, config, a, b, batch, invocation, own, &scope)
        .await;
    server.kill().await.unwrap();
}

fn frozen_runtime_context_policy() -> ContextPolicy {
    ContextPolicy {
        work_body: false,
        parent: false,
        ocr: false,
        asr: false,
        existing_problems: false,
        record_content: false,
        max_comments: 1,
        ..Default::default()
    }
}

async fn prepare_with_frozen_context(
    db: &Database,
    own: Uuid,
    source_ref: Uuid,
    policy: &ContextPolicy,
) -> Uuid {
    save_settings(
        db,
        &SaveContext {
            expected_revision: 0,
            policy: policy.clone(),
        },
    )
    .await
    .unwrap();
    let prepared = prepare(
        db,
        &Prepare {
            scope: ResearchScope {
                domain: Some(own),
                from: Some("2020-01-01T00:00:00Z".into()),
                to: Some("2099-01-01T00:00:00Z".into()),
                ..Default::default()
            },
            source_refs: Some(vec![source_ref]),
            reanalyze: false,
        },
    )
    .await
    .unwrap();
    Uuid::parse_str(prepared["prepareRef"].as_str().unwrap()).unwrap()
}

async fn assert_frozen_runtime_trace(
    db: &Database,
    batch: Uuid,
    own: Uuid,
    policy: &ContextPolicy,
) {
    let detail = batch_detail(db, batch, None).await.unwrap();
    assert_eq!(detail["calls"][0]["contextPolicy"], json!(policy));
    let invocation =
        Uuid::parse_str(detail["calls"][0]["invocationRef"].as_str().unwrap()).unwrap();
    let trace = request_detail(db, batch, invocation, own).await.unwrap();
    assert_eq!(trace["availability"], "NOT_RECORDED");
    assert!(trace["input"].is_null());
    assert!(trace["output"].is_null());
    assert_eq!(trace["events"].as_array().unwrap().len(), 4);
}

async fn save_test_schedule(
    db: &Database,
    expected_revision: i32,
    enabled: bool,
    config_ref: Uuid,
    auto_policy: AutoPolicy,
) {
    save_schedule(
        db,
        &DailySchedule {
            expected_revision,
            enabled,
            config_ref,
            source_limit: 100,
            token_limit: 100000,
            auto_policy,
        },
    )
    .await
    .unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn runtime_context_is_frozen_by_preparation_and_recording_can_be_disabled() {
    let db = fixture::proof_database("runtime_context_freeze").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    research_fixture::detail(&db, "runtime-policy", "合成冻结上下文").await;
    let a = source(&db, "runtime-policy", "a", "希望减少监督成本").await;
    let own = Uuid::parse_str("00000000-0000-4000-8000-000000000001").unwrap();
    let policy = frozen_runtime_context_policy();
    let batch = prepare_with_frozen_context(&db, own, a, &policy).await;
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
    assert_frozen_runtime_trace(&db, batch, own, &policy).await;
    let automatic: bool =
        sqlx::query_scalar("SELECT enabled FROM linggan_comment_daily_schedule WHERE singleton")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert!(!automatic);
    clock(&db, "2026-09-08 14:00:00Z").await;
    save_test_schedule(&db, 0, true, config, AutoPolicy::new_intake_only(100000)).await;
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

fn p1_semantic_input(
    source_ref: Uuid,
    work_ref: Uuid,
    comment_external_id: &str,
    body: &str,
    rule_hash: &str,
) -> linggan_intelligence::comment_research_fingerprint::SemanticInput {
    build_semantic_input(
        &CommentAnalysisInput {
            work_ref,
            lease_ref: Uuid::nil(),
            source_ref,
            source_sha256: comment_source_hash(body),
            body: body.into(),
            context: json!({
                "role":"unknown",
                "workIdentity":work_ref,
                "researchFragments":[],
            }),
            rule_version: "comment-research.v4".into(),
            model_version: "synthetic.execution.provenance".into(),
            instruction: "synthetic P1 eligibility input",
            limitations: vec![],
        },
        &SourceIdentity {
            work_ref,
            comment_external_id: comment_external_id.into(),
        },
        &RuleFingerprint::new(
            Uuid::from_u128(0x100),
            rule_hash,
            "comment-extraction.schema.v4",
            "comment-context.lexical-excerpts.v1",
            "comment-research.v4",
        ),
    )
}

async fn p1_source_shape(db: &Database, source_ref: Uuid) -> (Uuid, String, String) {
    let row = sqlx::query(
        "SELECT content_public_ref,comment_external_id,body_text FROM linggan_material_comment WHERE material_ref=$1",
    )
    .bind(source_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    (
        row.get("content_public_ref"),
        row.get("comment_external_id"),
        row.get("body_text"),
    )
}

async fn p1_observe_with_likes(
    db: &Database,
    note: &str,
    comment_id: &str,
    body: &str,
    likes: i64,
    observed_at: &str,
) -> Uuid {
    let package = fixture::submit_package_at(
        db,
        "comments",
        json!({"contentExternalId":note}),
        json!({
            "kind":"comment",
            "sourceObject":{"platform":"xhs","type":"content","externalId":note},
            "payload":{
                "commentId":comment_id,
                "noteId":note,
                "text":body,
                "authorId":"synthetic-hidden-author",
                "likes":likes,
            },
        }),
        observed_at,
    )
    .await;
    sqlx::query_scalar("SELECT material_ref FROM linggan_material_comment WHERE package_ref=$1")
        .bind(package)
        .fetch_one(db.pool())
        .await
        .unwrap()
}

async fn p1_insert_accepted_analysis(db: &Database, source_ref: Uuid) -> Uuid {
    let analysis_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_comment_analysis_work(work_ref,source_ref,rule_version,model_version,state,result) VALUES($1,$2,'comment-research.v4','synthetic-p1','succeeded',$3)",
    )
    .bind(analysis_ref)
    .bind(source_ref)
    .bind(json!({"contextRefs":{"researchSourceRefs":[source_ref]}}))
    .execute(db.pool())
    .await
    .unwrap();
    analysis_ref
}

async fn p1_insert_invocation(
    db: &Database,
    config_ref: Uuid,
    connection_version_ref: Uuid,
    known_usage: bool,
) -> Uuid {
    let invocation_ref = Uuid::new_v4();
    let result = json!({"callStarted":true});
    let (input_tokens, output_tokens) = if known_usage {
        (Some(17_i64), Some(5_i64))
    } else {
        (None, None)
    };
    sqlx::query(
        "INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash,state,reserved_tokens,charged_tokens,input_tokens,output_tokens,result,finished_at) SELECT $1,$2,m.model_ref,$3,'analyze',repeat('0',64),'failed',100,100,$4,$5,$6,scope_001_now() FROM linggan_model_config c JOIN linggan_model_entry m USING(model_ref) WHERE c.config_ref=$3",
    )
    .bind(invocation_ref)
    .bind(connection_version_ref)
    .bind(config_ref)
    .bind(input_tokens)
    .bind(output_tokens)
    .bind(result)
    .execute(db.pool())
    .await
    .unwrap();
    invocation_ref
}

#[tokio::test]
#[ignore = "isolated PostgreSQL"]
async fn p1_semantic_identity_ignores_new_observation_and_likes() {
    let db = fixture::proof_database("p1_stable_comment_identity").await;
    let first = p1_observe_with_likes(
        &db,
        "stable-identity",
        "same-comment",
        "同一条评论，正文没有变化",
        0,
        "2026-09-07T01:00:00Z",
    )
    .await;
    let replay = p1_observe_with_likes(
        &db,
        "stable-identity",
        "same-comment",
        "同一条评论，正文没有变化",
        99,
        "2026-09-08T01:00:00Z",
    )
    .await;
    assert_ne!(first, replay);
    let (first_work, first_id, first_body) = p1_source_shape(&db, first).await;
    let (replay_work, replay_id, replay_body) = p1_source_shape(&db, replay).await;
    assert_eq!(first_work, replay_work);
    assert_eq!(first_id, replay_id);
    let initial = p1_semantic_input(first, first_work, &first_id, &first_body, "a");
    let observed_again = p1_semantic_input(replay, replay_work, &replay_id, &replay_body, "a");
    assert_eq!(initial.source_identity, observed_again.source_identity);
    assert_eq!(initial.fingerprint, observed_again.fingerprint);
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn p1_legacy_alias_reuses_accepted_analysis_without_rewriting_its_key() {
    let db = fixture::proof_database("p1_legacy_alias_reuse").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    let source_ref = source(&db, "legacy-alias", "a", "我想知道如何坚持下去").await;
    let (work_ref, comment_id, body) = p1_source_shape(&db, source_ref).await;
    let old = p1_semantic_input(source_ref, work_ref, &comment_id, &body, "old-rule");
    let current = p1_semantic_input(source_ref, work_ref, &comment_id, &body, "new-rule");
    assert_ne!(old.fingerprint, current.fingerprint);
    let workspace_ref: Uuid =
        sqlx::query_scalar("SELECT workspace_ref FROM linggan_model_workspace WHERE singleton")
            .fetch_one(db.pool())
            .await
            .unwrap();
    let analysis_ref = p1_insert_accepted_analysis(&db, source_ref).await;
    let semantic_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_comment_semantic_work(semantic_ref,workspace_ref,identity_key,fingerprint,source_ref,config_ref,state,analysis_ref,attempts,input_manifest) VALUES($1,$2,$3,$4,$5,$6,'succeeded',$7,1,$8)",
    )
    .bind(semantic_ref)
    .bind(workspace_ref)
    .bind(&old.source_identity)
    .bind(&old.fingerprint)
    .bind(source_ref)
    .bind(config)
    .bind(analysis_ref)
    .bind(&old.input_manifest)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_comment_legacy_fingerprint_alias(source_identity,old_fingerprint,new_fingerprint,analysis_ref,proof_hash) VALUES($1,$2,$3,$4,$5)",
    )
    .bind(&old.source_identity)
    .bind(&old.fingerprint)
    .bind(&current.fingerprint)
    .bind(analysis_ref)
    .bind(comment_source_hash("synthetic compatibility proof"))
    .execute(db.pool())
    .await
    .unwrap();
    let eligibility = evaluate_eligibility(
        &db,
        &EligibilityRequest {
            workspace_ref,
            source_ref,
            input: current.clone(),
            qualification: SourceQualification::eligible(CleanState::Direct),
            explicit_generation: None,
            recovery: RecoveryPolicy::default(),
        },
    )
    .await
    .unwrap();
    assert_eq!(eligibility.category, EligibilityCategory::Reusable);
    assert_eq!(eligibility.result_state, ResultState::Studied);
    assert_eq!(eligibility.last_accepted_analysis_ref, Some(analysis_ref));
    assert!(!eligibility.eligible_to_dispatch);
    let persisted: String = sqlx::query_scalar(
        "SELECT fingerprint FROM linggan_comment_semantic_work WHERE semantic_ref=$1",
    )
    .bind(semantic_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(persisted, old.fingerprint);
    server.kill().await.unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn p1_unknown_charge_allows_exactly_one_separate_recovery() {
    let db = fixture::proof_database("p1_unknown_recovery").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, connection_version) = configured(&db, &url, "synthetic-good", None).await;
    let source_ref = source(&db, "unknown-retry", "a", "这条调用需要保留未知费用").await;
    let (work_ref, comment_id, body) = p1_source_shape(&db, source_ref).await;
    let input = p1_semantic_input(source_ref, work_ref, &comment_id, &body, "active-rule");
    let workspace_ref: Uuid =
        sqlx::query_scalar("SELECT workspace_ref FROM linggan_model_workspace WHERE singleton")
            .fetch_one(db.pool())
            .await
            .unwrap();
    let invocation_ref = p1_insert_invocation(&db, config, connection_version, false).await;
    let semantic_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_comment_semantic_work(semantic_ref,workspace_ref,identity_key,fingerprint,source_ref,config_ref,state,invocation_ref,failure_code,attempts,unknown_retry_attempts,input_manifest) VALUES($1,$2,$3,$4,$5,$6,'failed',$7,'worker_interrupted',1,0,$8)",
    )
    .bind(semantic_ref)
    .bind(workspace_ref)
    .bind(&input.source_identity)
    .bind(&input.fingerprint)
    .bind(source_ref)
    .bind(config)
    .bind(invocation_ref)
    .bind(&input.input_manifest)
    .execute(db.pool())
    .await
    .unwrap();
    let request = EligibilityRequest {
        workspace_ref,
        source_ref,
        input,
        qualification: SourceQualification::eligible(CleanState::Direct),
        explicit_generation: None,
        recovery: RecoveryPolicy {
            retry_max_attempts: 3,
            unknown_retry_max_attempts: 1,
            retry_cooldown_seconds: 0,
        },
    };
    let first = evaluate_eligibility(&db, &request).await.unwrap();
    assert_eq!(first.category, EligibilityCategory::FailedRecovery);
    assert_eq!(first.execution_state, ExecutionState::WaitingRecovery);
    assert!(first.eligible_to_dispatch);
    assert_eq!(first.recovery.unwrap().unknown_retry_attempts, 0);
    sqlx::query(
        "UPDATE linggan_comment_semantic_work SET unknown_retry_attempts=1 WHERE semantic_ref=$1",
    )
    .bind(semantic_ref)
    .execute(db.pool())
    .await
    .unwrap();
    let exhausted = evaluate_eligibility(&db, &request).await.unwrap();
    assert_eq!(exhausted.category, EligibilityCategory::FailedRecovery);
    assert_eq!(exhausted.execution_state, ExecutionState::Failed);
    assert!(!exhausted.eligible_to_dispatch);
    assert_eq!(exhausted.recovery.unwrap().unknown_retry_attempts, 1);
    server.kill().await.unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn p1_old_accepted_result_remains_visible_when_current_update_fails() {
    let db = fixture::proof_database("p1_result_execution_axes").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, connection_version) = configured(&db, &url, "synthetic-good", None).await;
    let source_ref = source(&db, "axes", "a", "更新前后都需要可解释").await;
    let (work_ref, comment_id, body) = p1_source_shape(&db, source_ref).await;
    let old = p1_semantic_input(source_ref, work_ref, &comment_id, &body, "old-rule");
    let current = p1_semantic_input(source_ref, work_ref, &comment_id, &body, "new-rule");
    let workspace_ref: Uuid =
        sqlx::query_scalar("SELECT workspace_ref FROM linggan_model_workspace WHERE singleton")
            .fetch_one(db.pool())
            .await
            .unwrap();
    let accepted_ref = p1_insert_accepted_analysis(&db, source_ref).await;
    sqlx::query(
        "INSERT INTO linggan_comment_semantic_work(semantic_ref,workspace_ref,identity_key,fingerprint,source_ref,config_ref,state,analysis_ref,attempts,input_manifest) VALUES($1,$2,$3,$4,$5,$6,'succeeded',$7,1,$8)",
    )
    .bind(Uuid::new_v4())
    .bind(workspace_ref)
    .bind(&old.source_identity)
    .bind(&old.fingerprint)
    .bind(source_ref)
    .bind(config)
    .bind(accepted_ref)
    .bind(&old.input_manifest)
    .execute(db.pool())
    .await
    .unwrap();
    let invocation_ref = p1_insert_invocation(&db, config, connection_version, true).await;
    sqlx::query(
        "INSERT INTO linggan_comment_semantic_work(semantic_ref,workspace_ref,identity_key,fingerprint,source_ref,config_ref,state,invocation_ref,failure_code,attempts,input_manifest) VALUES($1,$2,$3,$4,$5,$6,'failed',$7,'invalid_output',1,$8)",
    )
    .bind(Uuid::new_v4())
    .bind(workspace_ref)
    .bind(&current.source_identity)
    .bind(&current.fingerprint)
    .bind(source_ref)
    .bind(config)
    .bind(invocation_ref)
    .bind(&current.input_manifest)
    .execute(db.pool())
    .await
    .unwrap();
    let eligibility = evaluate_eligibility(
        &db,
        &EligibilityRequest {
            workspace_ref,
            source_ref,
            input: current,
            qualification: SourceQualification::eligible(CleanState::Direct),
            explicit_generation: None,
            recovery: RecoveryPolicy::default(),
        },
    )
    .await
    .unwrap();
    assert_eq!(eligibility.result_state, ResultState::Outdated);
    assert_eq!(eligibility.execution_state, ExecutionState::Failed);
    assert_eq!(eligibility.last_accepted_analysis_ref, Some(accepted_ref));
    assert_eq!(eligibility.current_attempt_ref, Some(invocation_ref));
    assert!(!eligibility.eligible_to_dispatch);
    server.kill().await.unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn p1_23_cutoff_and_pause_resume_keep_first_enable_history_boundary() {
    let db = fixture::proof_database("p1_daily_cutoff_pause_history").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    clock(&db, "2026-09-07 13:00:00Z").await;
    let before_enable = source(&db, "p1-history", "before", "首次启用前的历史评论").await;
    let policy = AutoPolicy {
        historical_enabled: true,
        history_start: Some("2026-09-01T00:00:00Z".into()),
        ..AutoPolicy::new_intake_only(100000)
    };
    clock(&db, "2026-09-07 14:59:59Z").await;
    save_test_schedule(&db, 0, true, config, policy.clone()).await;
    let enabled_at: String = sqlx::query_scalar(
        "SELECT auto_enabled_at::text FROM linggan_comment_daily_schedule WHERE singleton",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let before_cutoff = source(&db, "p1-history", "before-cutoff", "22:59:59属于当前窗口").await;
    clock(&db, "2026-09-07 15:00:00Z").await;
    let at_cutoff = source(&db, "p1-history", "at-cutoff", "23:00整属于下一窗口").await;
    assert_eq!(seal_all_due(&db).await.unwrap(), 1);
    let first_window: Vec<Uuid> =
        sqlx::query_scalar("SELECT source_ref FROM linggan_comment_daily_item ORDER BY source_ref")
            .fetch_all(db.pool())
            .await
            .unwrap();
    assert_eq!(first_window, vec![before_cutoff]);
    let first_policy: serde_json::Value = sqlx::query_scalar(
        "SELECT request->'autoPolicy' FROM linggan_comment_daily_batch WHERE kind='daily'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(first_policy["historyStart"], "2026-09-01T00:00:00Z");
    assert!(
        first_window
            .iter()
            .all(|source_ref| *source_ref != before_enable)
    );

    save_test_schedule(&db, 1, false, config, policy.clone()).await;
    clock(&db, "2026-09-07 18:00:00Z").await;
    let while_paused = source(&db, "p1-history", "paused", "暂停期间只等待恢复").await;
    assert_eq!(seal_all_due(&db).await.unwrap(), 0);
    clock(&db, "2026-09-08 14:00:00Z").await;
    save_test_schedule(&db, 2, true, config, policy).await;
    let resumed_at: String = sqlx::query_scalar(
        "SELECT auto_enabled_at::text FROM linggan_comment_daily_schedule WHERE singleton",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(resumed_at, enabled_at);
    clock(&db, "2026-09-08 15:00:00Z").await;
    assert_eq!(seal_all_due(&db).await.unwrap(), 1);
    let resumed_window: Vec<Uuid> = sqlx::query_scalar(
        "SELECT source_ref FROM linggan_comment_daily_item WHERE source_ref=ANY($1) ORDER BY source_ref",
    )
    .bind(vec![at_cutoff, while_paused])
    .fetch_all(db.pool())
    .await
    .unwrap();
    let mut expected_resumed = vec![at_cutoff, while_paused];
    expected_resumed.sort();
    assert_eq!(resumed_window, expected_resumed);
    let historical_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_comment_daily_item WHERE source_ref=$1")
            .bind(before_enable)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(historical_count, 0);
    server.kill().await.unwrap();
}
