//! Disposable PostgreSQL + actual Axum router. No mock transaction or provider I/O.
use super::tests::{app, send};
use super::*;
use linggan_intelligence::comment_study_batch::{PrepareStudyBatchRequest, prepare_study_batch};
use linggan_intelligence::comment_study_batch_acceptance::{
    BatchAcceptanceError, accept_study_batch_output,
};
use linggan_intelligence::comment_study_batch_worker::claim_next_study_batch;
use linggan_intelligence::comment_study_catalog::refresh_clean_cache;
use linggan_intelligence::comment_study_model_dispatch::{
    StudyModelDispatchError, mark_study_batch_model_dispatch_started,
    reserve_study_batch_model_call,
};
use linggan_intelligence::comment_study_policy::{CreateStudyPolicyCommand, create_study_policy};
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
            "../../../../database/migrations/0105_comment_study_productization_schema.sql"
        ),
        include_str!("../../../../database/migrations/0106_comment_study_policy_constraints.sql"),
        include_str!("../../../../database/migrations/0107_comment_study_start_constraints.sql"),
        include_str!(
            "../../../../database/migrations/0108_comment_study_request_snapshot_constraints.sql"
        ),
        include_str!("../../../../database/migrations/0109_comment_study_pair_failure_state.sql"),
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
fn next(value: &Value) -> Value {
    let mut next = value.clone();
    next["requestRef"] = json!(Uuid::new_v4());
    next
}
fn preview_command(value: &Value) -> Value {
    let command: StartStudyRunCommand = serde_json::from_value(value.clone()).unwrap();
    json!(command.preview())
}

#[tokio::test]
#[ignore = "isolated command HTTP PostgreSQL proof; no shared database or model"]
async fn http_preview_start_replay_conflict_bind_the_explicit_method_and_limits() {
    let p = setup("http_start", 3).await;
    let (status, preview) = send(
        p.application.clone(),
        "/api/local/comment-study/selection-preview",
        preview_command(&p.command),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(preview["targetCount"], 3);
    assert_eq!(effects(&p.db).await, json!([0, 0, 0, 0]));
    let (a, b) = tokio::join!(
        send(
            p.application.clone(),
            "/api/local/comment-study/runs",
            p.command.clone()
        ),
        send(
            p.application.clone(),
            "/api/local/comment-study/runs",
            p.command.clone()
        )
    );
    assert!(matches!(
        (a.0, b.0),
        (StatusCode::CREATED, StatusCode::OK) | (StatusCode::OK, StatusCode::CREATED)
    ));
    assert_eq!(a.1["runRef"], b.1["runRef"]);
    assert_ne!(a.1["idempotentReplay"], b.1["idempotentReplay"]);
    assert_eq!(a.1["limits"], p.command["limits"]);
    assert_eq!(a.1["dispatchState"], "enabled");
    assert_eq!(effects(&p.db).await, json!([1, 3, 1, 0]));
    let stored: Value=sqlx::query_scalar("SELECT jsonb_build_array(policy_ref,comment_budget,context_character_budget,token_limit) FROM linggan_comment_study_run")
        .fetch_one(p.db.pool()).await.unwrap();
    assert_eq!(stored, json!([p.command["policyRef"], 100, 6000, 100000]));
    for field in ["limits", "policyRef"] {
        let mut changed = p.command.clone();
        changed[field] = if field == "limits" {
            json!({"commentBudget":1,"contextCharacterBudget":6000,"tokenLimit":100000})
        } else {
            json!(Uuid::new_v4())
        };
        let (status, body) = send(
            p.application.clone(),
            "/api/local/comment-study/runs",
            changed,
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["error"]["code"], "idempotency_conflict");
        assert_eq!(body["requestRef"], p.command["requestRef"]);
    }
    assert_eq!(effects(&p.db).await, json!([1, 3, 1, 0]));
}

#[tokio::test]
#[ignore = "isolated command HTTP PostgreSQL proof; no shared database or model"]
async fn http_distinct_concurrent_requests_do_not_reserve_a_comment_twice() {
    let mut p = setup("http_race", 3).await;
    p.command["limits"]["commentBudget"] = json!(2);
    let (a, b) = tokio::join!(
        send(
            p.application.clone(),
            "/api/local/comment-study/runs",
            p.command.clone()
        ),
        send(
            p.application.clone(),
            "/api/local/comment-study/runs",
            next(&p.command)
        )
    );
    assert_eq!(a.0, StatusCode::CREATED);
    assert_eq!(b.0, StatusCode::CREATED);
    assert_eq!(
        a.1["targetCount"].as_u64().unwrap() + b.1["targetCount"].as_u64().unwrap(),
        3
    );
    let distinct: i64=sqlx::query_scalar("SELECT count(DISTINCT (content_public_ref,comment_external_id)) FROM linggan_comment_study_target")
        .fetch_one(p.db.pool()).await.unwrap();
    assert_eq!(distinct, 3);
    assert_eq!(effects(&p.db).await, json!([2, 3, 2, 0]));
}

#[tokio::test]
#[ignore = "isolated command HTTP PostgreSQL proof; no shared database or model"]
async fn http_empty_and_index_pending_are_terminal_receipts_not_errors_or_empty_runs() {
    let p = setup("http_empty", 0).await;
    let (status, empty) = send(
        p.application.clone(),
        "/api/local/comment-study/runs",
        p.command.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(empty["outcome"], "no_work");
    assert!(empty["runRef"].is_null());
    add_comment(&p.db, "late").await;
    let pending = next(&p.command);
    let (status, body) = send(
        p.application.clone(),
        "/api/local/comment-study/runs",
        pending.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["outcome"], "index_pending");
    assert!(body["runRef"].is_null());
    refresh_clean_cache(&p.db, domain(), 128).await.unwrap();
    for (command, outcome) in [(p.command.clone(), "no_work"), (pending, "index_pending")] {
        let (status, body) = send(
            p.application.clone(),
            "/api/local/comment-study/runs",
            command,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["outcome"], outcome);
        assert_eq!(body["idempotentReplay"], true);
    }
    assert_eq!(effects(&p.db).await, json!([0, 0, 2, 0]));
    let (status, body) = send(
        p.application.clone(),
        "/api/local/comment-study/runs",
        next(&p.command),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["targetCount"], 1);
}

#[tokio::test]
#[ignore = "isolated command HTTP PostgreSQL proof; no shared database or model"]
async fn http_activation_cas_preserves_method_rows_and_frozen_runs() {
    let p = setup("http_activate", 1).await;
    let second = policy(&p.db, p.config, "SYNTHETIC B").await;
    let (status, _) = send(
        p.application.clone(),
        "/api/local/comment-study/runs",
        p.command.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let before: Value=sqlx::query_scalar("SELECT jsonb_build_object('policies',(SELECT jsonb_agg(to_jsonb(p) ORDER BY policy_ref) FROM linggan_comment_study_policy p), \
        'runs',(SELECT jsonb_agg(to_jsonb(r) ORDER BY run_ref) FROM linggan_comment_study_run r))").fetch_one(p.db.pool()).await.unwrap();
    let path = format!(
        "/api/local/comment-study/policies/{second}/activate?domain={}",
        domain()
    );
    let expected = json!({"expectedActivePolicyRef":p.legacy});
    let (status, body) = send(p.application.clone(), &path, expected.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["policyRef"], json!(second));
    assert_eq!(body["isActive"], true);
    let (status, body) = send(p.application.clone(), &path, expected).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "control_version_conflict");
    let after: Value=sqlx::query_scalar("SELECT jsonb_build_object('policies',(SELECT jsonb_agg(to_jsonb(p) ORDER BY policy_ref) FROM linggan_comment_study_policy p), \
        'runs',(SELECT jsonb_agg(to_jsonb(r) ORDER BY run_ref) FROM linggan_comment_study_run r))").fetch_one(p.db.pool()).await.unwrap();
    assert_eq!(before, after);
    assert_eq!(effects(&p.db).await, json!([1, 1, 1, 0]));
}

#[tokio::test]
#[ignore = "isolated command HTTP PostgreSQL proof; no shared database or model"]
async fn http_first_default_is_atomic_and_invalid_methods_never_change_it() {
    let p = setup("http_first_default", 0).await;
    sqlx::query("DELETE FROM linggan_comment_study_active_policy")
        .execute(p.db.pool())
        .await
        .unwrap();
    let second = policy(&p.db, p.config, "SYNTHETIC B").await;
    let path_a = format!(
        "/api/local/comment-study/policies/{}/activate?domain={}",
        p.command["policyRef"].as_str().unwrap(),
        domain()
    );
    let path_b = format!(
        "/api/local/comment-study/policies/{second}/activate?domain={}",
        domain()
    );
    let (a, b) = tokio::join!(
        send(
            p.application.clone(),
            &path_a,
            json!({"expectedActivePolicyRef":null})
        ),
        send(
            p.application.clone(),
            &path_b,
            json!({"expectedActivePolicyRef":null})
        )
    );
    assert!(matches!(
        (a.0, b.0),
        (StatusCode::OK, StatusCode::CONFLICT) | (StatusCode::CONFLICT, StatusCode::OK)
    ));
    let current: Uuid = sqlx::query_scalar(
        "SELECT policy_ref FROM linggan_comment_study_active_policy WHERE domain_ref=$1",
    )
    .bind(domain())
    .fetch_one(p.db.pool())
    .await
    .unwrap();
    for (reference, code, status) in [
        (p.legacy, "policy_unrecorded", StatusCode::CONFLICT),
        (Uuid::new_v4(), "resource_not_found", StatusCode::NOT_FOUND),
    ] {
        let path = format!(
            "/api/local/comment-study/policies/{reference}/activate?domain={}",
            domain()
        );
        let (actual, body) = send(
            p.application.clone(),
            &path,
            json!({"expectedActivePolicyRef":current}),
        )
        .await;
        assert_eq!(actual, status);
        assert_eq!(body["error"]["code"], code);
    }
    sqlx::query("UPDATE linggan_model_connection SET enabled=false WHERE connection_ref=$1")
        .bind(p.connection)
        .execute(p.db.pool())
        .await
        .unwrap();
    let (status, body) = send(
        p.application.clone(),
        &path_b,
        json!({"expectedActivePolicyRef":current}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "policy_unavailable");
    let after: Uuid = sqlx::query_scalar(
        "SELECT policy_ref FROM linggan_comment_study_active_policy WHERE domain_ref=$1",
    )
    .bind(domain())
    .fetch_one(p.db.pool())
    .await
    .unwrap();
    assert_eq!(after, current);
    assert_eq!(effects(&p.db).await, json!([0, 0, 0, 0]));
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
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "resource_not_found");
    sqlx::query("ALTER TABLE linggan_comment_study_run DISABLE TRIGGER cs_run_insert_guard")
        .execute(p.db.pool())
        .await
        .unwrap();
    for (path, command) in [
        ("/api/local/comment-study/runs", p.command.clone()),
        (
            "/api/local/comment-study/selection-preview",
            preview_command(&p.command),
        ),
    ] {
        let (status, body) = send(p.application.clone(), path, command).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["error"]["code"], "study_schema_unavailable");
    }
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
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], "study_schema_unavailable");
    assert_eq!(effects(&p.db).await, json!([0, 0, 0, 0]));
}

#[tokio::test]
#[ignore = "isolated command HTTP PostgreSQL proof; no shared database or model"]
async fn http_cancel_is_idempotent_and_restart_selects_only_explicitly_retryable_targets() {
    let p = setup("http_cancel_restart", 2).await;
    let (status, started) = send(
        p.application.clone(),
        "/api/local/comment-study/runs",
        p.command.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let run_ref: Uuid = serde_json::from_value(started["runRef"].clone()).unwrap();
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

    let mut restart = next(&p.command);
    restart["mode"] = json!("retry_failed");
    restart["reason"] = json!("明确续做已取消目标");
    let (status, resumed) = send(
        p.application.clone(),
        "/api/local/comment-study/runs",
        restart,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_ne!(resumed["runRef"], json!(run_ref));
    assert_eq!(resumed["targetCount"], 2);
}

#[tokio::test]
#[ignore = "isolated command HTTP PostgreSQL proof; no shared database or model"]
async fn http_stop_rejects_unstarted_late_output_but_keeps_dispatched_output_authorized() {
    let p = setup("http_cancel_late", 2).await;
    let (status, started) = send(
        p.application.clone(),
        "/api/local/comment-study/runs",
        p.command.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let run_ref: Uuid = serde_json::from_value(started["runRef"].clone()).unwrap();
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
    let (status, started) = send(
        p.application.clone(),
        "/api/local/comment-study/runs",
        p.command.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let run_ref: Uuid = serde_json::from_value(started["runRef"].clone()).unwrap();
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

    let mut restart = next(&p.command);
    restart["mode"] = json!("retry_failed");
    restart["reason"] = json!("只续做停止后未完成的目标");
    let (status, resumed) = send(
        p.application.clone(),
        "/api/local/comment-study/runs",
        restart,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(resumed["targetCount"], 1);
}

#[tokio::test]
#[ignore = "isolated command HTTP PostgreSQL proof; no shared database or model"]
async fn http_run_control_cas_pauses_resumes_and_stops_without_losing_safe_state() {
    let p = setup("http_run_control_cas", 2).await;
    let (status, started) = send(
        p.application.clone(),
        "/api/local/comment-study/runs",
        p.command.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let run_ref: Uuid = serde_json::from_value(started["runRef"].clone()).unwrap();
    let prepared = prepare_study_batch(
        &p.db,
        PrepareStudyBatchRequest {
            run_ref,
            maximum_targets: 2,
        },
    )
    .await
    .unwrap();

    // Two commands racing on the same version cannot both win. The loser receives the current
    // safe state so the caller can refresh without guessing which transition committed.
    let pause_path = format!("/api/local/comment-study/runs/{run_ref}/pause");
    let pause = json!({"expectedControlVersion":0});
    let (a, b) = tokio::join!(
        send(p.application.clone(), &pause_path, pause.clone()),
        send(p.application.clone(), &pause_path, pause)
    );
    let (paused, conflict) = if a.0 == StatusCode::OK {
        (a, b)
    } else {
        (b, a)
    };
    assert_eq!(paused.0, StatusCode::OK);
    assert_eq!(paused.1["data"]["dispatchState"], "paused");
    assert_eq!(paused.1["data"]["dispatchReason"], "user_paused");
    assert_eq!(paused.1["data"]["controlVersion"], 1);
    assert_eq!(conflict.0, StatusCode::CONFLICT);
    assert_eq!(conflict.1["error"]["code"], "control_version_conflict");
    assert_eq!(
        conflict.1["error"]["details"]["current"]["controlVersion"],
        1
    );
    assert_eq!(
        conflict.1["error"]["details"]["current"]["dispatchState"],
        "paused"
    );

    assert!(
        claim_next_study_batch(&p.db, Uuid::new_v4(), 60)
            .await
            .unwrap()
            .is_none()
    );
    let paused_batch_state: String =
        sqlx::query_scalar("SELECT state FROM linggan_comment_study_batch WHERE batch_ref=$1")
            .bind(prepared.batch_ref)
            .fetch_one(p.db.pool())
            .await
            .unwrap();
    assert_eq!(paused_batch_state, "prepared");

    // A disabled frozen model makes resume unavailable. The control version and paused state
    // remain unchanged so the caller can retry only after the dependency is restored.
    sqlx::query("UPDATE linggan_model_connection SET enabled=false WHERE connection_ref=$1")
        .bind(p.connection)
        .execute(p.db.pool())
        .await
        .unwrap();
    let resume_path = format!("/api/local/comment-study/runs/{run_ref}/resume");
    let (status, unavailable) = send(
        p.application.clone(),
        &resume_path,
        json!({"expectedControlVersion":1}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(unavailable["error"]["code"], "policy_unavailable");
    assert_eq!(
        unavailable["error"]["details"]["current"]["dispatchState"],
        "paused"
    );
    assert_eq!(
        unavailable["error"]["details"]["current"]["controlVersion"],
        1
    );
    sqlx::query("UPDATE linggan_model_connection SET enabled=true WHERE connection_ref=$1")
        .bind(p.connection)
        .execute(p.db.pool())
        .await
        .unwrap();

    let (status, resumed) = send(
        p.application.clone(),
        &resume_path,
        json!({"expectedControlVersion":1}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(resumed["data"]["dispatchState"], "enabled");
    assert_eq!(resumed["data"]["dispatchReason"], Value::Null);
    assert_eq!(resumed["data"]["controlVersion"], 2);

    // The batch stays prepared while paused and is claimable as soon as the same Run resumes.
    let resumed_lease = claim_next_study_batch(&p.db, Uuid::new_v4(), 60)
        .await
        .unwrap()
        .expect("resumed Run should release its frozen batch");
    assert_eq!(resumed_lease.batch_ref, prepared.batch_ref);

    // If pause wins after claim but before reservation, the worker returns the lease without
    // creating an invocation. This avoids holding a prepared batch for the timeout window.
    let (status, paused_again) = send(
        p.application.clone(),
        &pause_path,
        json!({"expectedControlVersion":2}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(paused_again["data"]["controlVersion"], 3);
    assert!(matches!(
        reserve_study_batch_model_call(&p.db, resumed_lease.batch_ref, resumed_lease.lease_token)
            .await,
        Err(StudyModelDispatchError::PreDispatchDeferred)
    ));
    let (state, invocation): (String, Option<Uuid>) = sqlx::query_as(
        "SELECT state,model_invocation_ref FROM linggan_comment_study_batch WHERE batch_ref=$1",
    )
    .bind(prepared.batch_ref)
    .fetch_one(p.db.pool())
    .await
    .unwrap();
    assert_eq!(state, "prepared");
    assert_eq!(invocation, None);

    let (status, resumed_again) = send(
        p.application.clone(),
        &resume_path,
        json!({"expectedControlVersion":3}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(resumed_again["data"]["controlVersion"], 4);
    let resumed_lease = claim_next_study_batch(&p.db, Uuid::new_v4(), 60)
        .await
        .unwrap()
        .expect("resumed Run should reclaim its frozen batch");
    assert_eq!(resumed_lease.batch_ref, prepared.batch_ref);

    // A reservation is still not a provider call. If pause wins the Run lock before the
    // dispatch fence, retire that immutable request at zero charge and put its targets back
    // into the queue without recording a semantic attempt.
    let reserved =
        reserve_study_batch_model_call(&p.db, resumed_lease.batch_ref, resumed_lease.lease_token)
            .await
            .unwrap();
    let (status, paused_before_fence) = send(
        p.application.clone(),
        &pause_path,
        json!({"expectedControlVersion":4}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(paused_before_fence["data"]["controlVersion"], 5);
    assert!(matches!(
        mark_study_batch_model_dispatch_started(
            &p.db,
            reserved.invocation_ref,
            prepared.batch_ref,
            resumed_lease.lease_token,
        )
        .await,
        Err(StudyModelDispatchError::PreDispatchDeferred)
    ));
    let (batch_state, invocation_state, charged, failure, call_started, dispatch_started): (
        String,
        String,
        i64,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = sqlx::query_as(
        "SELECT batch.state,invocation.state,invocation.charged_tokens,invocation.failure_code, \
                invocation.result->>'callStarted',request.dispatch_started_at::text \
         FROM linggan_comment_study_batch batch \
         JOIN linggan_model_invocation invocation ON invocation.invocation_ref=batch.model_invocation_ref \
         JOIN linggan_comment_study_model_request request USING(invocation_ref) \
         WHERE batch.batch_ref=$1",
    )
    .bind(prepared.batch_ref)
    .fetch_one(p.db.pool())
    .await
    .unwrap();
    assert_eq!(batch_state, "cancelled");
    assert_eq!(invocation_state, "failed");
    assert_eq!(charged, 0);
    assert_eq!(failure.as_deref(), Some("user_paused_before_dispatch"));
    assert_eq!(call_started.as_deref(), Some("false"));
    assert_eq!(dispatch_started, None);
    let target_states: Vec<String> = sqlx::query_scalar(
        "SELECT state FROM linggan_comment_study_target WHERE run_ref=$1 ORDER BY target_ref",
    )
    .bind(run_ref)
    .fetch_all(p.db.pool())
    .await
    .unwrap();
    assert_eq!(target_states, vec!["queued", "queued"]);
    let semantic_attempts: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_study_semantic_attempt WHERE batch_ref=$1",
    )
    .bind(prepared.batch_ref)
    .fetch_one(p.db.pool())
    .await
    .unwrap();
    assert_eq!(semantic_attempts, 0);

    let (status, resumed_third) = send(
        p.application.clone(),
        &resume_path,
        json!({"expectedControlVersion":5}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(resumed_third["data"]["controlVersion"], 6);
    let replacement = prepare_study_batch(
        &p.db,
        PrepareStudyBatchRequest {
            run_ref,
            maximum_targets: 2,
        },
    )
    .await
    .unwrap();
    assert_ne!(replacement.batch_ref, prepared.batch_ref);
    let replacement_lease = claim_next_study_batch(&p.db, Uuid::new_v4(), 60)
        .await
        .unwrap()
        .expect("resumed queued targets should be frozen and claimable again");
    assert_eq!(replacement_lease.batch_ref, replacement.batch_ref);

    let stop_path = format!("/api/local/comment-study/runs/{run_ref}/stop");
    let (status, stopped) = send(
        p.application.clone(),
        &stop_path,
        json!({"expectedControlVersion":6}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(stopped["data"]["dispatchState"], "stopped");
    assert_eq!(stopped["data"]["dispatchReason"], "user_stopped");
    assert_eq!(stopped["data"]["controlVersion"], 7);
    assert_eq!(stopped["data"]["state"], "cancelled");

    let (status, terminal) = send(
        p.application.clone(),
        &resume_path,
        json!({"expectedControlVersion":7}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(terminal["error"]["code"], "run_stopped");
    assert_eq!(terminal["error"]["details"]["current"]["controlVersion"], 7);

    let targets: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_study_target \
         WHERE run_ref=$1 AND state='cancelled' AND terminal_reason='user_stopped' \
           AND finished_at IS NOT NULL",
    )
    .bind(run_ref)
    .fetch_one(p.db.pool())
    .await
    .unwrap();
    assert_eq!(targets, 2);
}

#[tokio::test]
#[ignore = "isolated disposable PostgreSQL + real Axum page/API + Playwright; no worker/provider"]
async fn browser_real_axum_postgres_previews_starts_and_stops_without_provider_dispatch() {
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

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let proof_token = Uuid::new_v4().to_string();
    let route_proof_token = proof_token.clone();
    let application = crate::local_web::comment_study::routes()
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
        .join("scripts/test-comment-study-productization-ui.py")
        .canonicalize()
        .unwrap();
    let api_base_url = format!("http://{address}");
    let domain_ref = domain().to_string();
    let work_ref_arg = work_ref.to_string();
    let target_policy_ref_arg = target_policy_ref.to_string();
    let initial_active_policy_ref_arg = initial_active_policy_ref.to_string();
    let proof_token_arg = proof_token;
    let python = std::env::var("PYTHON").unwrap_or_else(|_| "python3".to_owned());
    let browser_result = tokio::task::spawn_blocking(move || {
        Command::new(python)
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

    assert_eq!(effects(&p.db).await, json!([1, 2, 1, 0]));
    let (dispatch_state, run_state, control_version): (String, String, i64) = sqlx::query_as(
        "SELECT dispatch_state,state,control_version FROM linggan_comment_study_run LIMIT 1",
    )
    .fetch_one(p.db.pool())
    .await
    .unwrap();
    assert_eq!(dispatch_state, "stopped");
    assert_eq!(run_state, "cancelled");
    assert_eq!(control_version, 1);

    let (created_policy_ref, manifest): (Uuid, Value) = sqlx::query_as(
        "SELECT policy_ref,method_manifest FROM linggan_comment_study_policy \
         WHERE domain_ref=$1 AND method_name='隔离浏览器方法'",
    )
    .bind(domain())
    .fetch_one(p.db.pool())
    .await
    .unwrap();
    let active_policy_ref: Uuid = sqlx::query_scalar(
        "SELECT policy_ref FROM linggan_comment_study_active_policy WHERE domain_ref=$1",
    )
    .bind(domain())
    .fetch_one(p.db.pool())
    .await
    .unwrap();
    assert_eq!(created_policy_ref, active_policy_ref);
    assert_ne!(created_policy_ref, target_policy_ref);
    assert!(
        manifest["stages"]["semantic"]["systemInstruction"]
            .as_str()
            .unwrap()
            .contains("仅用于真实 Axum 与隔离 PostgreSQL 浏览器回归")
    );
    let run_policy_ref: Uuid =
        sqlx::query_scalar("SELECT policy_ref FROM linggan_comment_study_run LIMIT 1")
            .fetch_one(p.db.pool())
            .await
            .unwrap();
    assert_eq!(run_policy_ref, created_policy_ref);
}
