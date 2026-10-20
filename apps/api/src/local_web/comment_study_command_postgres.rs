//! Retired writes and preserved historical read/stop contracts over owned PostgreSQL + Axum.
//! Five positive start/activation HTTP scenarios retired with their live owner; original Git preserves them.
//! Their transaction contracts remain in the intelligence historical/core suites. No provider I/O.
use super::tests::{app, send};
use super::*;
use linggan_intelligence::comment_study_batch::{PrepareStudyBatchRequest, prepare_study_batch};
use linggan_intelligence::comment_study_batch_acceptance::{
    BatchAcceptanceError, accept_study_batch_output,
};
use linggan_intelligence::comment_study_batch_worker::claim_next_study_batch;
use linggan_intelligence::comment_study_catalog::refresh_clean_cache;
use linggan_intelligence::comment_study_model_dispatch::{
    mark_study_batch_model_dispatch_started, reserve_study_batch_model_call,
};
use linggan_intelligence::comment_study_policy::{CreateStudyPolicyCommand, create_study_policy};
use linggan_intelligence::comment_study_selection::StartStudyRunCommand;
use linggan_intelligence::comment_study_source::ADHD_DOMAIN_REF;
use linggan_storage_postgres::Database;
use serde_json::Value;
use std::sync::Arc;

#[allow(dead_code)]
#[path = "../../../../crates/evidence/tests/support/material_fixture.rs"]
mod fixture;

struct Proof {
    db: Arc<Database>,
    application: Router,
    command: Value,
    connection: Uuid,
    config: Uuid,
    legacy: Uuid,
}

fn domain() -> Uuid {
    Uuid::parse_str(ADHD_DOMAIN_REF).unwrap()
}

async fn policy(db: &Database, config: Uuid, name: &str) -> Uuid {
    let command: CreateStudyPolicyCommand = serde_json::from_value(json!({
        "domainRef":domain(),"methodName":name,"parentPolicyRef":null,"modelConfigRef":config,
        "defaults":{"commentBudget":1,"contextCharacterBudget":1},
        "stageInstructions":{"semantic":"","resolution":"","pair":""}}))
    .unwrap();
    let saved = create_study_policy(db, command).await.unwrap();
    serde_json::from_value(saved["policy"]["policyRef"].clone()).unwrap()
}

async fn add_comment(db: &Database, id: &str) {
    fixture::submit_package(db,"comments",json!({"contentExternalId":"selected"}),json!({
        "kind":"comment","sourceObject":{"platform":"xhs","type":"content","externalId":"selected"},
        "payload":{"commentId":id,"noteId":"selected","text":"SYNTHETIC 用户评论","authorId":"reader"}
    })).await;
}

async fn setup(name: &str, count: usize) -> Proof {
    let db = Arc::new(fixture::proof_database(name).await);
    sqlx::raw_sql(include_str!(
        "../../../../database/bootstrap/comment-study-001.sql"
    ))
    .execute(db.pool())
    .await
    .unwrap();
    let legacy = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_comment_study_policy(policy_ref,domain_ref,contract,comment_budget,context_character_budget) \
        VALUES($1,$2,'comment-study.v1',1,1)").bind(legacy).bind(domain()).execute(db.pool()).await.unwrap();
    sqlx::query(
        "INSERT INTO linggan_comment_study_active_policy(domain_ref,policy_ref) VALUES($1,$2)",
    )
    .bind(domain())
    .bind(legacy)
    .execute(db.pool())
    .await
    .unwrap();
    for migration in [
        include_str!(
            "../../../../database/migrations/0107_comment_study_productization_schema.sql"
        ),
        include_str!("../../../../database/migrations/0108_comment_study_policy_constraints.sql"),
        include_str!("../../../../database/migrations/0109_comment_study_start_constraints.sql"),
        include_str!(
            "../../../../database/migrations/0110_comment_study_request_snapshot_constraints.sql"
        ),
        include_str!("../../../../database/migrations/0111_comment_study_pair_failure_state.sql"),
        include_str!("../../../../database/migrations/0112_comment_study_membership_revision.sql"),
        include_str!("../../../../database/migrations/0114_comment_study_effective_head.sql"),
    ] {
        sqlx::raw_sql(migration).execute(db.pool()).await.unwrap();
    }
    let (connection, version, model, config) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    sqlx::query(
        "INSERT INTO linggan_model_connection(connection_ref,enabled,revision) VALUES($1,true,1)",
    )
    .bind(connection)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO linggan_model_connection_version(version_ref,connection_ref,revision,name,api,base_url,local_endpoint,secret_ref) \
        VALUES($1,$2,1,'SYNTHETIC','openai-completions','http://127.0.0.1:18080',true,$3)")
        .bind(version).bind(connection).bind(Uuid::new_v4()).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_entry(model_ref,connection_version_ref,model_id,origin) VALUES($1,$2,'synthetic','manual')")
        .bind(model).bind(version).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_config(config_ref,model_ref,input_token_limit,output_token_limit,timeout_seconds,max_attempts) \
        VALUES($1,$2,8192,1024,30,1)").bind(config).bind(model).execute(db.pool()).await.unwrap();
    let method = policy(&db, config, "SYNTHETIC A").await;
    fixture::submit_package(&db,"content_detail",json!({"contentExternalId":"selected"}),json!({
        "kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"selected"},
        "payload":{"noteId":"selected","title":"SYNTHETIC selected","bodyText":"SYNTHETIC 作品语境","authorId":"creator","authorName":"合成作者"}
    })).await;
    // Directly accepted package fixtures do not pass through the production domain-admission
    // command. Attach this synthetic legacy work through the current append-only usage contract.
    sqlx::query(
        "INSERT INTO linggan_material_domain_usage( \
            usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref \
         ) SELECT gen_random_uuid(),public_ref,$1,'primary','legacy_domain_migration',first_package_ref \
             FROM linggan_material_content \
            WHERE platform='xhs' AND content_external_id='selected' \
         ON CONFLICT (content_public_ref,domain_ref) \
             WHERE basis_kind='legacy_domain_migration' DO NOTHING",
    )
    .bind(domain())
    .execute(db.pool())
    .await
    .unwrap();
    let work: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_domain_usage WHERE domain_ref=$1 LIMIT 1",
    )
    .bind(domain())
    .fetch_one(db.pool())
    .await
    .unwrap();
    for i in 0..count {
        add_comment(&db, &format!("c{i:04}")).await;
    }
    for _ in 0..(count / 128 + 1) {
        refresh_clean_cache(&db, domain(), 128).await.unwrap();
    }
    let command = json!({"requestRef":Uuid::new_v4(),"domainRef":domain(),"policyRef":method,
        "scope":{"kind":"works","workRefs":[work]},"mode":"new_only",
        "limits":{"commentBudget":100,"contextCharacterBudget":6000,"tokenLimit":100000},"reason":null});
    Proof {
        application: app(LocalDatabaseState::Ready(db.clone())),
        db,
        command,
        connection,
        config,
        legacy,
    }
}

async fn effects(db: &Database) -> Value {
    sqlx::query_scalar("SELECT jsonb_build_array((SELECT count(*) FROM linggan_comment_study_run), \
        (SELECT count(*) FROM linggan_comment_study_target),(SELECT count(*) FROM linggan_comment_study_start_request), \
        (SELECT count(*) FROM linggan_model_invocation))").fetch_one(db.pool()).await.unwrap()
}
async fn historical_start(p: &Proof) -> Uuid {
    linggan_intelligence::comment_study_run::start_study_run(
        &p.db,
        serde_json::from_value(p.command.clone()).unwrap(),
        linggan_intelligence::comment_study_run::TrustedStudyOrigin::Manual,
    )
    .await
    .unwrap()
    .run_ref
    .expect("synthetic historical Run")
}
fn preview_command(value: &Value) -> Value {
    let command: StartStudyRunCommand = serde_json::from_value(value.clone()).unwrap();
    json!(command.preview())
}

#[tokio::test]
#[ignore = "isolated command HTTP PostgreSQL proof; no shared database or model"]
async fn http_missing_scope_and_partial_schema_fail_without_partial_writes() {
    let p = setup("http_partial", 1).await;
    let mut missing = p.command.clone();
    missing["scope"]["workRefs"] = json!([Uuid::new_v4()]);
    let (status, body) = send(
        p.application.clone(),
        "/api/local/comment-study/runs",
        missing,
    )
    .await;
    assert_eq!(status, StatusCode::GONE);
    assert_eq!(body["error"], "ai_legacy_execution_retired");
    sqlx::query("ALTER TABLE linggan_comment_study_run DISABLE TRIGGER cs_run_insert_guard")
        .execute(p.db.pool())
        .await
        .unwrap();
    let (status, body) = send(
        p.application.clone(),
        "/api/local/comment-study/runs",
        p.command.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::GONE);
    assert_eq!(body["error"], "ai_legacy_execution_retired");
    let (status, body) = send(
        p.application.clone(),
        "/api/local/comment-study/selection-preview",
        preview_command(&p.command),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], "study_schema_unavailable");
    sqlx::query("ALTER TABLE linggan_comment_study_policy DISABLE TRIGGER cs_policy_immutable")
        .execute(p.db.pool())
        .await
        .unwrap();
    let path = format!(
        "/api/local/comment-study/policies/{}/activate?domain={}",
        p.command["policyRef"].as_str().unwrap(),
        domain()
    );
    let (status, body) = send(
        p.application.clone(),
        &path,
        json!({"expectedActivePolicyRef":p.legacy}),
    )
    .await;
    assert_eq!(status, StatusCode::GONE);
    assert_eq!(body["error"], "ai_legacy_execution_retired");
    assert_eq!(effects(&p.db).await, json!([0, 0, 0, 0]));
}

#[tokio::test]
#[ignore = "isolated command HTTP PostgreSQL proof; no shared database or model"]
async fn http_continue_ready_cannot_reenable_retired_start_but_preview_still_validates() {
    let p = setup("http_recovery_scope", 1).await;
    let mut command = p.command.clone();
    command["mode"] = json!("continue_ready");
    for (path, body) in [
        ("/api/local/comment-study/runs", command.clone()),
        (
            "/api/local/comment-study/selection-preview",
            preview_command(&command),
        ),
    ] {
        let (status, response) = send(p.application.clone(), path, body).await;
        if path.ends_with("/runs") {
            assert_eq!(status, StatusCode::GONE);
            assert_eq!(response["error"], "ai_legacy_execution_retired");
        } else {
            assert_eq!(status, StatusCode::BAD_REQUEST);
            assert_eq!(response["error"]["code"], "invalid_request");
        }
    }
    assert_eq!(effects(&p.db).await, json!([0, 0, 0, 0]));
}

#[tokio::test]
#[ignore = "isolated command HTTP PostgreSQL proof; no shared database or model"]
async fn http_historical_cancel_is_idempotent_and_recovery_cannot_create_new_targets() {
    let p = setup("http_cancel_restart", 2).await;
    let run_ref = historical_start(&p).await;
    let path = format!("/api/local/comment-study/runs/{run_ref}/cancel");
    let cancel = json!({"domain_ref":domain()});
    let (status, first) = send(p.application.clone(), &path, cancel.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first["data"]["runRef"], json!(run_ref));
    assert_eq!(first["data"]["state"], "cancelled");
    assert_eq!(first["data"]["dispatchState"], "stopped");
    assert_eq!(first["data"]["dispatchReason"], "user_stopped");
    assert_eq!(first["data"]["controlVersion"], 1);
    assert!(first["data"]["finishedAt"].is_string());
    let (status, second) = send(p.application.clone(), &path, cancel).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(second["data"]["controlVersion"], 1);
    assert_eq!(second["data"]["dispatchReason"], "user_stopped");
    let cancelled: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_study_target WHERE run_ref=$1 AND state='cancelled' \
         AND terminal_reason='user_stopped' AND finished_at IS NOT NULL",
    )
    .bind(run_ref)
    .fetch_one(p.db.pool())
    .await
    .unwrap();
    assert_eq!(cancelled, 2);

    add_comment(&p.db, "c0002").await;
    refresh_clean_cache(&p.db, domain(), 128).await.unwrap();

    let recover_path = format!("/api/local/comment-study/runs/{run_ref}/recover");
    let recover = json!({"domainRef":domain(),"requestRef":Uuid::new_v4()});
    let before = effects(&p.db).await;
    for _ in 0..2 {
        let (status, receipt) = send(p.application.clone(), &recover_path, recover.clone()).await;
        assert_eq!(status, StatusCode::GONE);
        assert_eq!(receipt["error"], "ai_legacy_execution_retired");
    }
    assert_eq!(effects(&p.db).await, before);
}

#[tokio::test]
#[ignore = "isolated command HTTP PostgreSQL proof; no shared database or model"]
async fn http_stop_rejects_unstarted_late_output_but_keeps_dispatched_output_authorized() {
    let p = setup("http_cancel_late", 2).await;
    let run_ref = historical_start(&p).await;
    let prepared = prepare_study_batch(
        &p.db,
        PrepareStudyBatchRequest {
            run_ref,
            maximum_targets: 2,
        },
    )
    .await
    .unwrap();
    let lease = claim_next_study_batch(&p.db, Uuid::new_v4(), 60)
        .await
        .unwrap()
        .unwrap();
    let reserved = reserve_study_batch_model_call(&p.db, lease.batch_ref, lease.lease_token)
        .await
        .unwrap();
    let path = format!("/api/local/comment-study/runs/{run_ref}/stop");
    let (status, stopped) = send(
        p.application.clone(),
        &path,
        json!({"expectedControlVersion":0}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(stopped["data"]["dispatchState"], "stopped");

    let late = accept_study_batch_output(
        &p.db,
        lease.batch_ref,
        lease.lease_token,
        json!({"contract":"comment-study.note-batch.v1","batchRef":lease.batch_ref,
            "contentPublicRef":prepared.content_public_ref,"results":[{"targetRef":prepared.target_refs[0],
                "outcome":"no_signal","reason":"合成回执","signals":[]}]}),
    )
    .await;
    assert!(matches!(late, Err(BatchAcceptanceError::BatchUnavailable)));
    let unstarted: Value=sqlx::query_scalar(
        "SELECT jsonb_build_object('invocation',(SELECT jsonb_build_array(state,charged_tokens,failure_code) \
             FROM linggan_model_invocation WHERE invocation_ref=$1), \
           'target',(SELECT jsonb_build_array(state,terminal_reason) FROM linggan_comment_study_target WHERE target_ref=$2), \
           'signals',(SELECT count(*) FROM linggan_comment_study_signal WHERE target_ref=$2))",
    )
    .bind(reserved.invocation_ref)
    .bind(prepared.target_refs[0])
    .fetch_one(p.db.pool())
    .await
    .unwrap();
    assert_eq!(
        unstarted["invocation"],
        json!(["failed", 0, "user_stopped"])
    );
    assert_eq!(unstarted["target"], json!(["cancelled", "user_stopped"]));
    assert_eq!(unstarted["signals"], 0);

    let p = setup("http_cancel_authorized", 2).await;
    let run_ref = historical_start(&p).await;
    let prepared = prepare_study_batch(
        &p.db,
        PrepareStudyBatchRequest {
            run_ref,
            maximum_targets: 2,
        },
    )
    .await
    .unwrap();
    let lease = claim_next_study_batch(&p.db, Uuid::new_v4(), 60)
        .await
        .unwrap()
        .unwrap();
    let reserved = reserve_study_batch_model_call(&p.db, lease.batch_ref, lease.lease_token)
        .await
        .unwrap();
    mark_study_batch_model_dispatch_started(
        &p.db,
        reserved.invocation_ref,
        lease.batch_ref,
        lease.lease_token,
    )
    .await
    .unwrap();
    let path = format!("/api/local/comment-study/runs/{run_ref}/stop");
    let (status, stopped) = send(
        p.application.clone(),
        &path,
        json!({"expectedControlVersion":0}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(stopped["data"]["dispatchState"], "stopped");
    let receipt = accept_study_batch_output(
        &p.db,
        lease.batch_ref,
        lease.lease_token,
        json!({"contract":"comment-study.note-batch.v1","batchRef":lease.batch_ref,
            "contentPublicRef":prepared.content_public_ref,"results":[{"targetRef":prepared.target_refs[0],
                "outcome":"no_signal","reason":"合成回执","signals":[]}]}),
    )
    .await
    .unwrap();
    assert_eq!(receipt.accepted_target_count, 1);
    assert_eq!(receipt.cancelled_target_count, 1);
    let settled: Value=sqlx::query_scalar(
        "SELECT jsonb_build_object('run',(SELECT jsonb_build_array(state,dispatch_state,dispatch_reason,control_version) \
             FROM linggan_comment_study_run WHERE run_ref=$1), \
           'invocation',(SELECT jsonb_build_array(state,charged_tokens,failure_code) \
             FROM linggan_model_invocation WHERE invocation_ref=$2), \
           'targets',jsonb_build_array( \
             (SELECT count(*) FROM linggan_comment_study_target WHERE run_ref=$1 AND state='no_signal'), \
             (SELECT count(*) FROM linggan_comment_study_target WHERE run_ref=$1 AND state='cancelled' \
                AND terminal_reason='user_stopped')))",
    )
    .bind(run_ref)
    .bind(reserved.invocation_ref)
    .fetch_one(p.db.pool())
    .await
    .unwrap();
    assert_eq!(
        settled["run"],
        json!(["completed_with_failures", "stopped", "user_stopped", 1])
    );
    assert_eq!(settled["invocation"][0], "succeeded");
    assert_eq!(settled["targets"], json!([1, 1]));

    let before_restart = effects(&p.db).await;
    let mut restart = p.command.clone();
    restart["requestRef"] = json!(Uuid::new_v4());
    restart["mode"] = json!("retry_failed");
    restart["reason"] = json!("只续做停止后未完成的目标");
    let (status, resumed) = send(
        p.application.clone(),
        "/api/local/comment-study/runs",
        restart,
    )
    .await;
    assert_eq!(status, StatusCode::GONE);
    assert_eq!(resumed["error"], "ai_legacy_execution_retired");
    assert_eq!(effects(&p.db).await, before_restart);
}

#[tokio::test]
#[ignore = "isolated command HTTP PostgreSQL proof; no shared database or model"]
async fn http_historical_pause_stop_preserve_cas_but_resume_never_reenables_execution() {
    let p = setup("http_run_control_cas", 2).await;
    let run = historical_start(&p).await;
    let pause_path = format!("/api/local/comment-study/runs/{run}/pause");
    let (a, b) = tokio::join!(
        send(
            p.application.clone(),
            &pause_path,
            json!({"expectedControlVersion":0})
        ),
        send(
            p.application.clone(),
            &pause_path,
            json!({"expectedControlVersion":0})
        )
    );
    let (accepted, rejected) = if a.0 == StatusCode::OK {
        (a, b)
    } else {
        (b, a)
    };
    assert_eq!(accepted.0, StatusCode::OK);
    assert_eq!(accepted.1["data"]["controlVersion"], 1);
    assert_eq!(rejected.0, StatusCode::CONFLICT);
    assert_eq!(rejected.1["error"]["code"], "control_version_conflict");
    for enabled in [false, true] {
        sqlx::query("UPDATE linggan_model_connection SET enabled=$2 WHERE connection_ref=$1")
            .bind(p.connection)
            .bind(enabled)
            .execute(p.db.pool())
            .await
            .unwrap();
        let (status, receipt) = send(
            p.application.clone(),
            &format!("/api/local/comment-study/runs/{run}/resume"),
            json!({"expectedControlVersion":1}),
        )
        .await;
        assert_eq!(status, StatusCode::GONE);
        assert_eq!(receipt["error"], "ai_legacy_execution_retired");
        let row: (String, i64) = sqlx::query_as(
            "SELECT dispatch_state,control_version FROM linggan_comment_study_run WHERE run_ref=$1",
        )
        .bind(run)
        .fetch_one(p.db.pool())
        .await
        .unwrap();
        assert_eq!(row, ("paused".into(), 1));
    }
    let (status, stopped) = send(
        p.application.clone(),
        &format!("/api/local/comment-study/runs/{run}/stop"),
        json!({"expectedControlVersion":1}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(stopped["data"]["controlVersion"], 2);
    assert_eq!(stopped["data"]["state"], "cancelled");
    let calls: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
        .fetch_one(p.db.pool())
        .await
        .unwrap();
    assert_eq!(calls, 0);
}

#[tokio::test]
#[ignore = "isolated disposable PostgreSQL + real Axum page/API + Playwright; no worker/provider"]
async fn browser_real_axum_postgres_retired_commands_preserve_reading_and_human_post_usage() {
    use std::path::Path;
    use std::process::Command;
    use tokio::net::TcpListener;

    assert_eq!(
        std::env::var("P1_BROWSER_PROOF").as_deref(),
        Ok("1"),
        "the isolated proof harness must explicitly enable browser tests"
    );

    let p = setup("browser_live_axum", 3).await;
    let work_ref: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_domain_usage WHERE domain_ref=$1 LIMIT 1",
    )
    .bind(domain())
    .fetch_one(p.db.pool())
    .await
    .unwrap();
    let target_policy_ref: Uuid = sqlx::query_scalar(
        "SELECT policy_ref FROM linggan_comment_study_policy \
         WHERE domain_ref=$1 AND method_name='SYNTHETIC A'",
    )
    .bind(domain())
    .fetch_one(p.db.pool())
    .await
    .unwrap();
    let initial_active_policy_ref: Uuid = sqlx::query_scalar(
        "SELECT policy_ref FROM linggan_comment_study_active_policy WHERE domain_ref=$1",
    )
    .bind(domain())
    .fetch_one(p.db.pool())
    .await
    .unwrap();

    policy(&p.db, p.config, "SYNTHETIC B").await;
    let before = effects(&p.db).await;
    let policies_before: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_comment_study_policy")
            .fetch_one(p.db.pool())
            .await
            .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let proof_token = Uuid::new_v4().to_string();
    let route_proof_token = proof_token.clone();
    let application = crate::local_web::comment_study::routes()
        .merge(crate::local_web::web_app::routes())
        .merge(crate::local_web::corpus_shell_api::routes())
        .merge(crate::local_web::ai_settings::routes())
        .route(
            "/__comment-study-browser-proof",
            axum::routing::get(move || {
                let token = route_proof_token.clone();
                async move { token }
            }),
        )
        .with_state(super::tests::state(LocalDatabaseState::Ready(p.db.clone())));
    let server = tokio::spawn(async move {
        axum::serve(listener, application).await.unwrap();
    });

    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("apps/web/test/e2e/comments-real-chain.mjs")
        .canonicalize()
        .unwrap();
    let api_base_url = format!("http://{address}");
    let domain_ref = domain().to_string();
    let work_ref_arg = work_ref.to_string();
    let target_policy_ref_arg = target_policy_ref.to_string();
    let initial_active_policy_ref_arg = initial_active_policy_ref.to_string();
    let proof_token_arg = proof_token;
    let node = std::env::var("WEB_APP_PROOF_NODE").unwrap_or_else(|_| "node".to_owned());
    let browser_result = tokio::task::spawn_blocking(move || {
        Command::new(node)
            .arg(script)
            .arg("--api-base-url")
            .arg(api_base_url)
            .arg("--domain-ref")
            .arg(domain_ref)
            .arg("--work-ref")
            .arg(work_ref_arg)
            .arg("--existing-policy-ref")
            .arg(target_policy_ref_arg)
            .arg("--initial-active-policy-ref")
            .arg(initial_active_policy_ref_arg)
            .arg("--proof-token")
            .arg(proof_token_arg)
            .output()
    })
    .await
    .unwrap();
    server.abort();
    let _ = server.await;
    let browser_result = browser_result.unwrap();
    assert!(
        browser_result.status.success(),
        "live API browser regression failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&browser_result.stdout),
        String::from_utf8_lossy(&browser_result.stderr)
    );

    assert_eq!(
        effects(&p.db).await,
        before,
        "retirement UI and human usage cannot create research or model calls"
    );
    let (count,active):(i64,Uuid)=sqlx::query_as("SELECT (SELECT count(*) FROM linggan_comment_study_policy),policy_ref FROM linggan_comment_study_active_policy WHERE domain_ref=$1").bind(domain()).fetch_one(p.db.pool()).await.unwrap();
    assert_eq!(
        (count, active),
        (policies_before, initial_active_policy_ref)
    );
}

/// Build one small, internally consistent read fixture. No worker or provider is started; the
/// browser still talks to real Axum handlers and PostgreSQL for every detail and navigation step.
async fn seed_browser_problem(p: &Proof) -> (Uuid, Uuid) {
    // The live start API is retired; these are synthetic historical fixture rows only.
    let run_ref = historical_start(p).await;
    let targets: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT target_ref FROM linggan_comment_study_target \
         WHERE run_ref=$1 ORDER BY created_at,target_ref LIMIT 3",
    )
    .bind(run_ref)
    .fetch_all(p.db.pool())
    .await
    .unwrap();
    assert_eq!(targets.len(), 3);

    let mut signal_refs = Vec::new();
    for (target_ref,) in targets.iter().take(2).copied() {
        let attempt_ref = Uuid::new_v4();
        let signal_ref = Uuid::new_v4();
        sqlx::query(
            "UPDATE linggan_comment_study_target \
             SET state='succeeded',finished_at=scope_001_now() WHERE target_ref=$1",
        )
        .bind(target_ref)
        .execute(p.db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO linggan_comment_study_semantic_attempt( \
               attempt_ref,target_ref,attempt_ordinal,request_hash,state,output_manifest,finished_at) \
             VALUES($1,$2,1,repeat('a',64),'accepted','{}'::jsonb,scope_001_now())",
        )
        .bind(attempt_ref)
        .bind(target_ref)
        .execute(p.db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO linggan_comment_study_signal( \
               signal_ref,target_ref,semantic_attempt_ref,kind,proposition,evidence, \
               evidence_start,evidence_end,problem_frame,eligibility_state,canonical_text,canonical_hash) \
             VALUES($1,$2,$3,'problem','用户需要外部催促才能开始作业','SYNTHETIC', \
               0,9,'{}'::jsonb,'eligible','用户无法自主启动家庭作业',repeat('b',64))",
        )
        .bind(signal_ref)
        .bind(target_ref)
        .bind(attempt_ref)
        .execute(p.db.pool())
        .await
        .unwrap();
        signal_refs.push(signal_ref);
    }
    let deferred_target_ref = targets[2].0;
    let deferred_attempt_ref = Uuid::new_v4();
    let deferred_signal_ref = Uuid::new_v4();
    sqlx::query(
        "UPDATE linggan_comment_study_target \
         SET state='succeeded',finished_at=scope_001_now() WHERE target_ref=$1",
    )
    .bind(deferred_target_ref)
    .execute(p.db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_comment_study_semantic_attempt( \
           attempt_ref,target_ref,attempt_ordinal,request_hash,state,output_manifest,finished_at) \
         VALUES($1,$2,1,repeat('a',64),'accepted','{}'::jsonb,scope_001_now())",
    )
    .bind(deferred_attempt_ref)
    .bind(deferred_target_ref)
    .execute(p.db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_comment_study_signal( \
           signal_ref,target_ref,semantic_attempt_ref,kind,proposition,evidence, \
           evidence_start,evidence_end,problem_frame,eligibility_state,canonical_text,canonical_hash) \
         VALUES($1,$2,$3,'problem','尚未建档的作业表达','SYNTHETIC', \
           0,9,'{}'::jsonb,'eligible','尚未建档的作业表达',repeat('e',64))",
    )
    .bind(deferred_signal_ref)
    .bind(deferred_target_ref)
    .bind(deferred_attempt_ref)
    .execute(p.db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_comment_study_resolution( \
           resolution_ref,signal_ref,domain_ref,state,candidate_manifest,decision_manifest,resolved_at) \
         VALUES($1,$2,$3,'deferred_novel','{}'::jsonb,'{}'::jsonb,scope_001_now())",
    )
    .bind(Uuid::new_v4())
    .bind(deferred_signal_ref)
    .bind(domain())
    .execute(p.db.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE linggan_comment_study_target \
         SET state='no_signal',finished_at=scope_001_now() \
         WHERE run_ref=$1 AND state='queued'",
    )
    .bind(run_ref)
    .execute(p.db.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE linggan_comment_study_run \
         SET state='completed',finished_at=scope_001_now() WHERE run_ref=$1",
    )
    .bind(run_ref)
    .execute(p.db.pool())
    .await
    .unwrap();

    let problem_ref = Uuid::new_v4();
    let revision_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_comment_study_problem( \
           problem_ref,domain_ref,identity_version,support_version,state) \
         VALUES($1,$2,1,2,'active')",
    )
    .bind(problem_ref)
    .bind(domain())
    .execute(p.db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_comment_study_problem_revision( \
           revision_ref,problem_ref,domain_ref,identity_version,title,definition,core_frame, \
           inclusions,exclusions,seed_signal_refs,canonical_text,canonical_hash,definition_hash,reason) \
         VALUES($1,$2,$3,1,'作业启动困难','孩子在家庭作业中需要外部催促才能开始', \
           '{}'::jsonb,'[\"需要外部催促\"]'::jsonb,'[]'::jsonb,$4, \
           '孩子在家庭作业中需要外部催促才能开始',repeat('c',64),repeat('d',64),'SYNTHETIC / NOT EVIDENCE')",
    )
    .bind(revision_ref)
    .bind(problem_ref)
    .bind(domain())
    .bind(&signal_refs)
    .execute(p.db.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE linggan_comment_study_problem SET current_revision_ref=$2 WHERE problem_ref=$1",
    )
    .bind(problem_ref)
    .bind(revision_ref)
    .execute(p.db.pool())
    .await
    .unwrap();
    let mut first_resolution_ref = None;
    for signal_ref in signal_refs {
        let resolution_ref = Uuid::new_v4();
        if first_resolution_ref.is_none() {
            first_resolution_ref = Some(resolution_ref);
        }
        sqlx::query(
            "INSERT INTO linggan_comment_study_resolution( \
               resolution_ref,signal_ref,domain_ref,state,candidate_manifest,decision_manifest, \
               resolved_problem_ref,resolved_at) \
             VALUES($1,$2,$3,'assigned','{}'::jsonb,$4,$5,scope_001_now())",
        )
        .bind(resolution_ref)
        .bind(signal_ref)
        .bind(domain())
        .bind(json!({"problemRevisionRef":revision_ref}))
        .bind(problem_ref)
        .execute(p.db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO linggan_comment_study_problem_membership( \
               membership_ref,signal_ref,problem_ref,resolution_ref,problem_revision_ref) \
             VALUES($1,$2,$3,$4,$5)",
        )
        .bind(Uuid::new_v4())
        .bind(signal_ref)
        .bind(problem_ref)
        .bind(resolution_ref)
        .bind(revision_ref)
        .execute(p.db.pool())
        .await
        .unwrap();
    }
    // A recorded request makes the protected detail screen testable without dispatching a model.
    let invocation_ref = Uuid::new_v4();
    let resolution_ref = first_resolution_ref.unwrap();
    let (model_ref, version_ref): (Uuid, Uuid) = sqlx::query_as(
        "SELECT model.model_ref,model.connection_version_ref \
         FROM linggan_model_config config JOIN linggan_model_entry model USING(model_ref) \
         WHERE config.config_ref=$1",
    )
    .bind(p.config)
    .fetch_one(p.db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_model_invocation( \
           invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash, \
           state,reserved_tokens,charged_tokens,input_tokens,output_tokens,finished_at) \
         VALUES($1,$2,$3,$4,'analyze',repeat('f',64),'succeeded',9216,14,10,4,scope_001_now())",
    )
    .bind(invocation_ref)
    .bind(version_ref)
    .bind(model_ref)
    .bind(p.config)
    .execute(p.db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_comment_study_model_request( \
           invocation_ref,run_ref,policy_ref,stage,resolution_ref,attempt_ordinal, \
           input_context_hash,request_manifest,request_hash,dispatch_started_at,deadline_at) \
         VALUES($1,$2,$3,'resolution',$4,1,repeat('c',64),$5,repeat('f',64), \
                scope_001_now(),scope_001_now()+interval '5 minutes')",
    )
    .bind(invocation_ref)
    .bind(run_ref)
    .bind(serde_json::from_value::<Uuid>(p.command["policyRef"].clone()).unwrap())
    .bind(resolution_ref)
    .bind(json!({
        "systemInstruction":"SYNTHETIC / NOT EVIDENCE",
        "prompt":serde_json::to_string(&json!({
            "input":{"candidates":[{"problemRevisionRef":revision_ref}]}
        })).unwrap()
    }))
    .execute(p.db.pool())
    .await
    .unwrap();
    (run_ref, problem_ref)
}

#[tokio::test]
#[ignore = "isolated PostgreSQL + real Axum browser read flow; no worker/provider"]
async fn browser_real_axum_postgres_traces_comment_run_signal_problem_evidence() {
    use std::path::Path;
    use std::process::Command;
    use tokio::net::TcpListener;

    assert_eq!(std::env::var("P1_BROWSER_PROOF").as_deref(), Ok("1"));
    let p = setup("browser_comment_problem_e2e", 3).await;
    let (run_ref, problem_ref) = seed_browser_problem(&p).await;
    let deferred = linggan_intelligence::comment_study_read::read_deferred_expressions(
        &p.db,
        &linggan_intelligence::comment_study_read::CommentStudyReadQuery {
            domain: Some(domain()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        deferred["expressions"].as_array().unwrap().len(),
        1,
        "{deferred}"
    );
    let existing_policy_ref = p.command["policyRef"].as_str().unwrap().to_owned();
    let initial_active_policy_ref = p.legacy.to_string();
    let work_ref: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_domain_usage WHERE domain_ref=$1 LIMIT 1",
    )
    .bind(domain())
    .fetch_one(p.db.pool())
    .await
    .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let proof_token = Uuid::new_v4().to_string();
    let route_proof_token = proof_token.clone();
    let application = crate::local_web::comment_study::routes()
        .merge(crate::local_web::web_app::routes())
        .merge(crate::local_web::corpus_shell_api::routes())
        .merge(crate::local_web::ai_settings::routes())
        .route(
            "/__comment-study-browser-proof",
            axum::routing::get(move || {
                let token = route_proof_token.clone();
                async move { token }
            }),
        )
        .with_state(super::tests::state(LocalDatabaseState::Ready(p.db.clone())));
    let server = tokio::spawn(async move { axum::serve(listener, application).await.unwrap() });
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("apps/web/test/e2e/comments-real-chain.mjs")
        .canonicalize()
        .unwrap();
    let node = std::env::var("WEB_APP_PROOF_NODE").unwrap_or_else(|_| "node".to_owned());
    let browser_result = tokio::task::spawn_blocking(move || {
        Command::new(node)
            .arg(script)
            .arg("--api-base-url")
            .arg(format!("http://{address}"))
            .arg("--domain-ref")
            .arg(domain().to_string())
            .arg("--work-ref")
            .arg(work_ref.to_string())
            .arg("--existing-policy-ref")
            .arg(existing_policy_ref)
            .arg("--initial-active-policy-ref")
            .arg(initial_active_policy_ref)
            .arg("--proof-token")
            .arg(proof_token)
            .arg("--e2e-run-ref")
            .arg(run_ref.to_string())
            .arg("--e2e-problem-ref")
            .arg(problem_ref.to_string())
            .output()
    })
    .await
    .unwrap();
    server.abort();
    let _ = server.await;
    let browser_result = browser_result.unwrap();
    assert!(
        browser_result.status.success(),
        "browser stdout: {}\nbrowser stderr: {}",
        String::from_utf8_lossy(&browser_result.stdout),
        String::from_utf8_lossy(&browser_result.stderr)
    );
}
