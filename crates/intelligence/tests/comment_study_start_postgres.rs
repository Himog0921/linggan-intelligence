//! P2 start/dispatch proofs: disposable PostgreSQL and synthetic materials only. No model I/O.
#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
#[path = "support/comment_research_fixture.rs"]
mod research_fixture;
use linggan_intelligence::{
    comment_study_batch::{PrepareStudyBatchRequest, next_run_needing_batch, prepare_study_batch},
    comment_study_batch_acceptance::accept_study_batch_output,
    comment_study_batch_worker::{claim_next_study_batch, recover_expired_study_batch_leases},
    comment_study_catalog::refresh_clean_cache,
    comment_study_model_dispatch::{
        StudyModelDispatchError, mark_study_batch_model_dispatch_started,
        reserve_study_batch_model_call,
    },
    comment_study_model_runner::{StudyModelRunnerError, call_study_batch_model},
    comment_study_policy::{
        CreateStudyPolicyCommand, StudyPolicyDefaults, StudyStageInstructions, create_study_policy,
    },
    comment_study_run::{
        StudyStartError, TrustedStudyOrigin, preview_study_selection, start_study_run,
    },
    comment_study_selection::{StartStudyRunCommand, StudySelectionMode, study_domain_lock_key},
    model_invocation::checkpoint_invocation_usage,
    model_secrets::SyntheticModelSecrets,
    pi_adapter::PiAdapter,
    pi_adapter::{PiResponse, PiUsage},
};
use linggan_storage_postgres::Database;
use research_fixture::{comment_with_author, detail_with_author, reply_with_author};
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;
const GUARDS: &str =
    include_str!("../../../database/migrations/0107_comment_study_start_constraints.sql");
const REQUEST_GUARDS: &str = include_str!(
    "../../../database/migrations/0108_comment_study_request_snapshot_constraints.sql"
);
fn domain() -> Uuid {
    Uuid::parse_str(linggan_intelligence::comment_study_source::ADHD_DOMAIN_REF).unwrap()
}

async fn setup(name: &str, count: usize) -> (Database, StartStudyRunCommand, Uuid) {
    setup_with_model_input_limit(name, count, 8192).await
}

async fn setup_with_model_input_limit(
    name: &str,
    count: usize,
    model_input_limit: i32,
) -> (Database, StartStudyRunCommand, Uuid) {
    let db = fixture::proof_database(name).await;
    sqlx::raw_sql(include_str!(
        "../../../database/bootstrap/comment-study-001.sql"
    ))
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../../../database/migrations/0105_comment_study_productization_schema.sql"
    ))
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../../../database/migrations/0106_comment_study_policy_constraints.sql"
    ))
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::raw_sql(GUARDS).execute(db.pool()).await.unwrap();
    sqlx::raw_sql(REQUEST_GUARDS)
        .execute(db.pool())
        .await
        .unwrap();
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
        VALUES($1,$2,$3,1024,30,1)").bind(config).bind(model).bind(model_input_limit).execute(db.pool()).await.unwrap();
    let policy: CreateStudyPolicyCommand = serde_json::from_value(
        json!({"domainRef":domain(),"methodName":"SYNTHETIC", "parentPolicyRef":null,
        "modelConfigRef":config,"defaults":{"commentBudget":1,"contextCharacterBudget":1},
        "stageInstructions":{"semantic":"","resolution":"","pair":""}}),
    )
    .unwrap();
    let policy = create_study_policy(&db, policy).await.unwrap();
    detail_with_author(&db, "selected", "SYNTHETIC selected", Some("creator")).await;
    let work: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_domain_usage WHERE domain_ref=$1 LIMIT 1",
    )
    .bind(domain())
    .fetch_one(db.pool())
    .await
    .unwrap();
    for index in 0..count {
        add(&db, &format!("c{index:04}")).await;
    }
    for _ in 0..(count / 128 + 1) {
        refresh_clean_cache(&db, domain(), 128).await.unwrap();
    }
    let command=serde_json::from_value(json!({"requestRef":Uuid::new_v4(),"domainRef":domain(),"policyRef":policy["policy"]["policyRef"],
        "scope":{"kind":"works","workRefs":[work]},"mode":"new_only",
        "limits":{"commentBudget":100,"contextCharacterBudget":6000,"tokenLimit":100000},"reason":null})).unwrap();
    (db, command, connection)
}

async fn policy_with_semantic_instruction(
    db: &Database,
    command: &StartStudyRunCommand,
    semantic_instruction: String,
) -> Uuid {
    let model_config_ref: Uuid = sqlx::query_scalar(
        "SELECT model_config_ref FROM linggan_comment_study_policy WHERE policy_ref=$1",
    )
    .bind(command.policy_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let policy = create_study_policy(
        db,
        CreateStudyPolicyCommand {
            domain_ref: domain(),
            method_name: "INPUT_LIMIT_FIXTURE".to_owned(),
            parent_policy_ref: Some(command.policy_ref),
            model_config_ref,
            defaults: StudyPolicyDefaults {
                comment_budget: 2,
                context_character_budget: 6000,
            },
            stage_instructions: StudyStageInstructions {
                semantic: semantic_instruction,
                resolution: String::new(),
                pair: String::new(),
            },
        },
    )
    .await
    .unwrap();
    serde_json::from_value(policy["policy"]["policyRef"].clone()).unwrap()
}
async fn add(db: &Database, id: &str) -> Uuid {
    comment_with_author(
        db,
        "selected",
        id,
        "SYNTHETIC 用户评论",
        Some("reader"),
        "2026-09-21T08:00:00Z",
    )
    .await
}
async fn effects(db: &Database) -> Value {
    sqlx::query_scalar("SELECT jsonb_build_array((SELECT count(*) FROM linggan_comment_study_run), \
        (SELECT count(*) FROM linggan_comment_study_target),(SELECT count(*) FROM linggan_comment_study_start_request), \
        (SELECT count(*) FROM linggan_model_invocation))").fetch_one(db.pool()).await.unwrap()
}
fn next(command: &StartStudyRunCommand) -> StartStudyRunCommand {
    let mut c = command.clone();
    c.request_ref = Uuid::new_v4();
    c
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn preview_and_two_runs_cover_different_comments_from_688() {
    let (db, c, _) = setup("start_688", 688).await;
    let preview = preview_study_selection(&db, c.preview()).await.unwrap();
    assert_eq!(preview["scopeCommentCount"], 688);
    assert_eq!(preview["targetCount"], 100);
    assert_eq!(effects(&db).await, json!([0, 0, 0, 0]));
    let first = start_study_run(&db, c.clone(), TrustedStudyOrigin::Manual)
        .await
        .unwrap();
    sqlx::query("UPDATE linggan_comment_study_target SET state='no_signal',finished_at=scope_001_now() WHERE run_ref=$1")
        .bind(first.run_ref).execute(db.pool()).await.unwrap();
    let second = start_study_run(&db, next(&c), TrustedStudyOrigin::Manual)
        .await
        .unwrap();
    assert_eq!(second.target_count, 100);
    assert_eq!(second.exclusion_counts["notSelectedByMode"], 100);
    let overlap:i64=sqlx::query_scalar("SELECT count(*) FROM linggan_comment_study_target a JOIN linggan_comment_study_target b \
        ON a.content_public_ref=b.content_public_ref AND a.comment_external_id=b.comment_external_id WHERE a.run_ref=$1 AND b.run_ref=$2")
        .bind(first.run_ref).bind(second.run_ref).fetch_one(db.pool()).await.unwrap();
    assert_eq!(overlap, 0);
    assert_eq!(effects(&db).await, json!([2, 200, 2, 0]));
    let limits:Value=sqlx::query_scalar("SELECT jsonb_build_array(comment_budget,context_character_budget,token_limit) FROM linggan_comment_study_run WHERE run_ref=$1")
        .bind(first.run_ref).fetch_one(db.pool()).await.unwrap();
    assert_eq!(
        limits,
        json!([100, 6000, 100000]),
        "no active/default budget substitution"
    );
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn concurrent_identical_request_replays_one_run_and_conflicting_command_fails() {
    let (db, c, _) = setup("start_replay", 3).await;
    let (a, b) = tokio::join!(
        start_study_run(&db, c.clone(), TrustedStudyOrigin::Manual),
        start_study_run(&db, c.clone(), TrustedStudyOrigin::Manual)
    );
    let (a, b) = (a.unwrap(), b.unwrap());
    assert_eq!(a.run_ref, b.run_ref);
    assert_ne!(a.idempotent_replay, b.idempotent_replay);
    assert_eq!(effects(&db).await, json!([1, 3, 1, 0]));
    let mut changed = c;
    changed.limits.comment_budget = 2;
    assert!(matches!(
        start_study_run(&db, changed, TrustedStudyOrigin::Manual).await,
        Err(StudyStartError::IdempotencyConflict)
    ));
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn concurrent_distinct_requests_never_reserve_the_same_comment() {
    let (db, c, _) = setup("start_race", 150).await;
    let (a, b) = tokio::join!(
        start_study_run(&db, c.clone(), TrustedStudyOrigin::Manual),
        start_study_run(&db, next(&c), TrustedStudyOrigin::Manual)
    );
    let (a, b) = (a.unwrap(), b.unwrap());
    assert_eq!(a.target_count + b.target_count, 150);
    assert_ne!(a.run_ref, b.run_ref);
    let duplicates:i64=sqlx::query_scalar("SELECT count(*) FROM (SELECT content_public_ref,comment_external_id FROM linggan_comment_study_target \
        GROUP BY 1,2 HAVING count(*)>1) d").fetch_one(db.pool()).await.unwrap();
    assert_eq!(duplicates, 0);
    assert_eq!(effects(&db).await, json!([2, 150, 2, 0]));
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn empty_and_index_pending_receipts_remain_terminal_after_material_changes() {
    let (db, c, connection) = setup("start_empty", 0).await;
    let empty = start_study_run(&db, c.clone(), TrustedStudyOrigin::Manual)
        .await
        .unwrap();
    assert_eq!(empty.outcome, "no_work");
    add(&db, "late").await;
    let pending_command = next(&c);
    let pending = start_study_run(&db, pending_command.clone(), TrustedStudyOrigin::Manual)
        .await
        .unwrap();
    assert_eq!(pending.outcome, "index_pending");
    refresh_clean_cache(&db, domain(), 128).await.unwrap();
    sqlx::query("UPDATE linggan_model_connection SET enabled=false WHERE connection_ref=$1")
        .bind(connection)
        .execute(db.pool())
        .await
        .unwrap();
    assert_eq!(
        start_study_run(&db, c, TrustedStudyOrigin::Manual)
            .await
            .unwrap()
            .outcome,
        "no_work"
    );
    assert_eq!(
        start_study_run(&db, pending_command, TrustedStudyOrigin::Manual)
            .await
            .unwrap()
            .outcome,
        "index_pending"
    );
    assert_eq!(effects(&db).await, json!([0, 0, 2, 0]));
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn missing_parent_settles_without_model_and_reobservation_is_not_changed_input() {
    let (db, c, _) = setup("start_context", 0).await;
    reply_with_author(
        &db,
        "selected",
        "child",
        "parent",
        "我也是",
        Some("reader"),
        "2026-09-21T08:00:00Z",
    )
    .await;
    refresh_clean_cache(&db, domain(), 128).await.unwrap();
    let first = start_study_run(&db, c.clone(), TrustedStudyOrigin::Manual)
        .await
        .unwrap();
    assert_eq!(first.needs_context_count, 1);
    let state: String =
        sqlx::query_scalar("SELECT state FROM linggan_comment_study_run WHERE run_ref=$1")
            .bind(first.run_ref)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(state, "completed");
    let mut changed = next(&c);
    changed.mode = StudySelectionMode::InputChanged;
    changed.reason = Some("SYNTHETIC parent repair".into());
    assert_eq!(
        start_study_run(&db, changed.clone(), TrustedStudyOrigin::Manual)
            .await
            .unwrap()
            .outcome,
        "no_work"
    );
    comment_with_author(
        &db,
        "selected",
        "parent",
        "SYNTHETIC 父语境",
        Some("creator"),
        "2026-09-21T08:00:00Z",
    )
    .await;
    refresh_clean_cache(&db, domain(), 128).await.unwrap();
    let repaired = start_study_run(&db, next(&changed), TrustedStudyOrigin::Manual)
        .await
        .unwrap();
    assert_eq!(repaired.queued_count, 1);
    sqlx::query("UPDATE linggan_comment_study_target SET state='no_signal',finished_at=scope_001_now() WHERE run_ref=$1")
        .bind(repaired.run_ref).execute(db.pool()).await.unwrap();
    reply_with_author(
        &db,
        "selected",
        "child",
        "parent",
        "我也是",
        Some("reader"),
        "2026-09-22T08:00:00Z",
    )
    .await;
    refresh_clean_cache(&db, domain(), 128).await.unwrap();
    assert_eq!(
        start_study_run(&db, next(&changed), TrustedStudyOrigin::Manual)
            .await
            .unwrap()
            .outcome,
        "no_work"
    );
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn failed_receipt_insert_rolls_back_run_targets_and_can_retry_the_same_request() {
    let (db, c, _) = setup("start_rollback", 2).await;
    sqlx::raw_sql("CREATE FUNCTION cs_proof_fail() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'SYNTHETIC'; END $$; \
        CREATE TRIGGER cs_proof_failure BEFORE INSERT ON linggan_comment_study_start_request FOR EACH ROW EXECUTE FUNCTION cs_proof_fail();")
        .execute(db.pool()).await.unwrap();
    assert!(
        start_study_run(&db, c.clone(), TrustedStudyOrigin::Manual)
            .await
            .is_err()
    );
    assert_eq!(effects(&db).await, json!([0, 0, 0, 0]));
    sqlx::raw_sql("DROP TRIGGER cs_proof_failure ON linggan_comment_study_start_request")
        .execute(db.pool())
        .await
        .unwrap();
    assert_eq!(
        start_study_run(&db, c, TrustedStudyOrigin::Manual)
            .await
            .unwrap()
            .target_count,
        2
    );
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn new_run_is_frozen_and_method_bound_dispatch_accepts_only_the_valid_target() {
    let (db, c, _) = setup("start_guards", 2).await;
    let receipt = start_study_run(&db, c.clone(), TrustedStudyOrigin::Manual)
        .await
        .unwrap();
    let run = receipt.run_ref.unwrap();
    assert!(
        sqlx::query("UPDATE linggan_comment_study_run SET token_limit=200000 WHERE run_ref=$1")
            .bind(run)
            .execute(db.pool())
            .await
            .is_err()
    );
    assert!(
        sqlx::query("DELETE FROM linggan_comment_study_start_request WHERE request_ref=$1")
            .bind(c.request_ref)
            .execute(db.pool())
            .await
            .is_err()
    );
    assert!(
        sqlx::raw_sql("TRUNCATE linggan_comment_study_start_request")
            .execute(db.pool())
            .await
            .is_err()
    );
    assert_eq!(next_run_needing_batch(&db, &[]).await.unwrap(), Some(run));
    let prepared = prepare_study_batch(
        &db,
        PrepareStudyBatchRequest {
            run_ref: run,
            maximum_targets: 12,
        },
    )
    .await
    .unwrap();
    let claimed = claim_next_study_batch(&db, Uuid::new_v4(), 60)
        .await
        .unwrap()
        .unwrap();
    let reservation = reserve_study_batch_model_call(&db, prepared.batch_ref, claimed.lease_token)
        .await
        .unwrap();
    assert_eq!(reservation.run_ref, run);
    let request: Value = sqlx::query_scalar(
        "SELECT request_manifest FROM linggan_comment_study_model_request WHERE invocation_ref=$1",
    )
    .bind(reservation.invocation_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(
        request["systemInstruction"]
            .as_str()
            .unwrap()
            .contains("补充研究说明只影响研究侧重点")
    );
    assert_eq!(
        request["outputSchema"]["properties"]["contract"]["enum"],
        json!(["comment-study.note-batch.v1"])
    );
    let (snapshot_hash, invocation_hash): (String, String) = sqlx::query_as(
        "SELECT request.request_hash,invocation.request_hash \
         FROM linggan_comment_study_model_request request \
         JOIN linggan_model_invocation invocation USING(invocation_ref) \
         WHERE request.invocation_ref=$1",
    )
    .bind(reservation.invocation_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(snapshot_hash, invocation_hash);
    let call_started: bool = sqlx::query_scalar(
        "SELECT result->>'callStarted'='false' FROM linggan_model_invocation WHERE invocation_ref=$1",
    )
    .bind(reservation.invocation_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(
        call_started,
        "reservation alone must not claim provider I/O"
    );
    mark_study_batch_model_dispatch_started(
        &db,
        reservation.invocation_ref,
        prepared.batch_ref,
        claimed.lease_token,
    )
    .await
    .unwrap();
    let target_refs: Vec<Uuid> = sqlx::query_scalar(
        "SELECT target_ref FROM linggan_comment_study_batch_target WHERE batch_ref=$1 ORDER BY ordinal",
    )
    .bind(prepared.batch_ref)
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(target_refs.len(), 2);
    let synthetic_output = json!({
        "contract":"comment-study.note-batch.v1",
        "batchRef":prepared.batch_ref,
        "contentPublicRef":prepared.content_public_ref,
        "results":[{"targetRef":target_refs[0],"outcome":"no_signal","reason":"SYNTHETIC no_signal result","signals":[]}]
    });
    let accepted = accept_study_batch_output(
        &db,
        prepared.batch_ref,
        claimed.lease_token,
        synthetic_output,
    )
    .await
    .unwrap();
    assert_eq!(accepted.accepted_target_count, 1);
    assert_eq!(accepted.retried_target_count, 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_comment_study_target WHERE run_ref=$1 AND state='no_signal'",
        )
        .bind(run)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        1
    );
    assert!(sqlx::query(
        "UPDATE linggan_comment_study_model_request SET request_hash=repeat('0',64) WHERE invocation_ref=$1",
    )
    .bind(reservation.invocation_ref)
    .execute(db.pool())
    .await
    .is_err());
    assert!(sqlx::query(
        "UPDATE linggan_comment_study_model_request SET dispatch_started_at=NULL WHERE invocation_ref=$1",
    )
    .bind(reservation.invocation_ref)
    .execute(db.pool())
    .await
    .is_err());
    assert!(
        sqlx::query("DELETE FROM linggan_comment_study_model_request WHERE invocation_ref=$1")
            .bind(reservation.invocation_ref)
            .execute(db.pool())
            .await
            .is_err()
    );
    assert!(
        sqlx::raw_sql("TRUNCATE linggan_comment_study_model_request")
            .execute(db.pool())
            .await
            .is_err()
    );
    let v: Value = sqlx::query_scalar(
        "SELECT execution_manifest FROM linggan_comment_study_run WHERE run_ref=$1",
    )
    .bind(run)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(v["engineRevision"].as_str().unwrap().len(), 40);
    assert_eq!(effects(&db).await, json!([1, 2, 1, 1]));
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn insufficient_run_token_budget_stops_the_undispatched_run_without_an_invocation() {
    let (db, mut command, _) = setup("start_token_budget", 2).await;
    command.limits.token_limit = 1024;
    let receipt = start_study_run(&db, command, TrustedStudyOrigin::Manual)
        .await
        .unwrap();
    let run_ref = receipt.run_ref.unwrap();
    let prepared = prepare_study_batch(
        &db,
        PrepareStudyBatchRequest {
            run_ref,
            maximum_targets: 12,
        },
    )
    .await
    .unwrap();
    let claimed = claim_next_study_batch(&db, Uuid::new_v4(), 60)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        reserve_study_batch_model_call(&db, prepared.batch_ref, claimed.lease_token).await,
        Err(StudyModelDispatchError::BudgetExhausted)
    ));
    let run: Value = sqlx::query_scalar(
        "SELECT jsonb_build_object('state',state,'dispatchState',dispatch_state, \
         'dispatchReason',dispatch_reason,'controlVersion',control_version) \
         FROM linggan_comment_study_run WHERE run_ref=$1",
    )
    .bind(run_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(run["dispatchState"], "stopped");
    assert_eq!(run["dispatchReason"], "budget_exhausted");
    assert_eq!(run["controlVersion"], 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_comment_study_target \
             WHERE run_ref=$1 AND state='cancelled' AND terminal_reason='budget_exhausted' \
               AND finished_at IS NOT NULL",
        )
        .bind(run_ref)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_model_invocation")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0,
        "budget rejection must precede any provider ledger reservation"
    );
    assert_eq!(next_run_needing_batch(&db, &[]).await.unwrap(), None);
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn method_aware_batch_packing_splits_targets_before_the_input_limit() {
    let (db, mut command, _) = setup("start_method_input_split", 2).await;
    let baseline = start_study_run(&db, command.clone(), TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();
    let rows = sqlx::query(
        "SELECT target.target_ref,target.source_ref,target.research_text, \
                target.dependency_state,target.input_manifest,work.content_public_ref,work.context_manifest \
         FROM linggan_comment_study_target target \
         JOIN linggan_comment_study_work work USING(run_ref,content_public_ref) \
         WHERE target.run_ref=$1 ORDER BY target.created_at,target.target_ref",
    )
    .bind(baseline)
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(rows.len(), 2);
    let work_ref: Uuid = rows[0].get("content_public_ref");
    let context: Value = rows[0].get("context_manifest");
    let target_values = rows
        .iter()
        .map(|row| {
            let input: Value = row.get("input_manifest");
            json!({
                "targetRef":row.get::<Uuid,_>("target_ref"),
                "sourceRef":row.get::<Uuid,_>("source_ref"),
                "researchText":row.get::<String,_>("research_text"),
                "dependencyState":row.get::<String,_>("dependency_state"),
                "parentContext":input["parentContext"],
            })
        })
        .collect::<Vec<_>>();
    let build_input = |targets: &[Value]| {
        json!({
            "contract":"comment-study.note-batch.v1",
            "batchRef":Uuid::nil(),
            "runRef":baseline,
            "workRef":work_ref,
            "workContext":context,
            "targets":targets,
        })
    };
    let baseline_method: Value = sqlx::query_scalar(
        "SELECT to_jsonb(policy)->'method_manifest' FROM linggan_comment_study_policy policy WHERE policy_ref=$1",
    )
    .bind(command.policy_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let schema = baseline_method["stages"]["semantic"]["outputSchema"].clone();
    let system_instruction = baseline_method["stages"]["semantic"]["systemInstruction"]
        .as_str()
        .unwrap();
    let request_size = |input: &Value, instruction: &str| {
        let prompt = serde_json::to_string(&json!({
            "contract":"comment-study.note-batch.v1",
            "batchRef":Uuid::nil(),
            "runRef":baseline,
            "input":input,
            "outputSchema":schema,
        }))
        .unwrap();
        let manifest = json!({
            "stage":"semantic",
            "systemInstruction":instruction,
            "prompt":prompt,
            "outputSchema":schema,
        });
        let mut estimate = 256_i64;
        let mut ascii = 0_i64;
        for character in manifest.to_string().chars() {
            if character.is_ascii() {
                ascii += 1;
            } else {
                estimate += 2;
            }
        }
        estimate + (ascii + 3) / 4
    };
    let single_input = build_input(&target_values[..1]);
    let pair_input = build_input(&target_values);
    let base_single = request_size(&single_input, system_instruction);
    let mut extension_chars = usize::try_from((8192 - base_single).max(1) / 2).unwrap_or(1);
    let mut fitting_policy_ref = None;
    for _ in 0..8 {
        let policy_ref = policy_with_semantic_instruction(
            &db,
            &command,
            "界".repeat(extension_chars.clamp(1, 7900)),
        )
        .await;
        let method: Value = sqlx::query_scalar(
            "SELECT to_jsonb(policy)->'method_manifest' FROM linggan_comment_study_policy policy WHERE policy_ref=$1",
        )
        .bind(policy_ref)
        .fetch_one(db.pool())
        .await
        .unwrap();
        let instruction = method["stages"]["semantic"]["systemInstruction"]
            .as_str()
            .unwrap();
        let single = request_size(&single_input, instruction);
        let pair = request_size(&pair_input, instruction);
        if single <= 8192 && pair > 8192 {
            fitting_policy_ref = Some(policy_ref);
            break;
        }
        if single > 8192 {
            extension_chars = extension_chars
                .saturating_sub(usize::try_from((single - 8192 + 1) / 2).unwrap_or(1).max(1));
        } else {
            extension_chars = extension_chars
                .saturating_add(usize::try_from((8192 - pair + 1) / 2).unwrap_or(1).max(1));
        }
    }
    let fitting_policy_ref =
        fitting_policy_ref.expect("a single-target-only input boundary exists");
    sqlx::query(
        "UPDATE linggan_comment_study_target SET state='failed',finished_at=scope_001_now(), \
         terminal_reason='attempts_exhausted' WHERE run_ref=$1 AND state='queued'",
    )
    .bind(baseline)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE linggan_comment_study_run SET state='completed_with_failures',finished_at=scope_001_now() WHERE run_ref=$1",
    )
    .bind(baseline)
    .execute(db.pool())
    .await
    .unwrap();
    command.policy_ref = fitting_policy_ref;
    command.request_ref = Uuid::new_v4();
    command.mode = StudySelectionMode::RetryFailed;
    command.reason = Some("synthetic input fitting boundary".to_owned());
    let run_ref = start_study_run(&db, command, TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();

    let first = prepare_study_batch(
        &db,
        PrepareStudyBatchRequest {
            run_ref,
            maximum_targets: 12,
        },
    )
    .await
    .unwrap();
    assert_eq!(first.target_refs.len(), 1);
    let second = prepare_study_batch(
        &db,
        PrepareStudyBatchRequest {
            run_ref,
            maximum_targets: 12,
        },
    )
    .await
    .unwrap();
    assert_eq!(second.target_refs.len(), 1);
    assert_ne!(first.target_refs[0], second.target_refs[0]);
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn one_target_over_the_frozen_method_input_limit_is_terminal_without_invocation() {
    let (db, mut command, _) = setup("start_method_input_reject", 1).await;
    command.policy_ref = policy_with_semantic_instruction(&db, &command, "界".repeat(4000)).await;
    let run_ref = start_study_run(&db, command, TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();

    assert!(matches!(
        prepare_study_batch(
            &db,
            PrepareStudyBatchRequest {
                run_ref,
                maximum_targets: 12,
            },
        )
        .await,
        Err(linggan_intelligence::comment_study_batch::StudyBatchError::InputLimitExceeded)
    ));
    let target: Value = sqlx::query_scalar(
        "SELECT jsonb_build_object('state',state,'reason',terminal_reason,'finished',finished_at IS NOT NULL) \
         FROM linggan_comment_study_target WHERE run_ref=$1",
    )
    .bind(run_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(target["state"], "failed");
    assert_eq!(target["reason"], "input_limit_exceeded");
    assert_eq!(target["finished"], true);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT (SELECT count(*) FROM linggan_comment_study_batch WHERE run_ref=$1) + \
                    (SELECT count(*) FROM linggan_model_invocation)",
        )
        .bind(run_ref)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        0
    );
    let run_state: String =
        sqlx::query_scalar("SELECT state FROM linggan_comment_study_run WHERE run_ref=$1")
            .bind(run_ref)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(run_state, "completed_with_failures");
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn valid_1024_model_context_terminalizes_the_default_instruction_overflow() {
    let (db, command, _) =
        setup_with_model_input_limit("start_default_instruction_overflow", 1, 1024).await;
    let run_ref = start_study_run(&db, command, TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();

    assert!(matches!(
        prepare_study_batch(
            &db,
            PrepareStudyBatchRequest {
                run_ref,
                maximum_targets: 12,
            },
        )
        .await,
        Err(linggan_intelligence::comment_study_batch::StudyBatchError::InputLimitExceeded)
    ));
    let target_reason: Option<String> = sqlx::query_scalar(
        "SELECT terminal_reason FROM linggan_comment_study_target WHERE run_ref=$1",
    )
    .bind(run_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(target_reason.as_deref(), Some("input_limit_exceeded"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_model_invocation")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn measured_running_usage_replaces_the_reservation_for_future_budget_checks() {
    let (db, mut command, _) = setup("start_measured_usage_budget", 2).await;
    command.limits.token_limit = 10_000;
    let run_ref = start_study_run(&db, command, TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();
    let first = prepare_study_batch(
        &db,
        PrepareStudyBatchRequest {
            run_ref,
            maximum_targets: 1,
        },
    )
    .await
    .unwrap();
    let second = prepare_study_batch(
        &db,
        PrepareStudyBatchRequest {
            run_ref,
            maximum_targets: 1,
        },
    )
    .await
    .unwrap();
    let first_lease = claim_next_study_batch(&db, Uuid::new_v4(), 60)
        .await
        .unwrap()
        .unwrap();
    let first_reservation =
        reserve_study_batch_model_call(&db, first.batch_ref, first_lease.lease_token)
            .await
            .unwrap();
    let (reserved_tokens, charged_before_call): (i64, i64) = sqlx::query_as(
        "SELECT reserved_tokens,charged_tokens FROM linggan_model_invocation WHERE invocation_ref=$1",
    )
    .bind(first_reservation.invocation_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(reserved_tokens > 0);
    assert_eq!(charged_before_call, 0);
    assert!(reserved_tokens.saturating_mul(2) <= 10_000);

    mark_study_batch_model_dispatch_started(
        &db,
        first_reservation.invocation_ref,
        first.batch_ref,
        first_lease.lease_token,
    )
    .await
    .unwrap();
    let actual_charge = 10_001 - reserved_tokens;
    let input_tokens = actual_charge.min(8192);
    let output_tokens = actual_charge - input_tokens;
    assert!(input_tokens <= 8192 && output_tokens <= 1024);
    checkpoint_invocation_usage(
        &db,
        first_reservation.invocation_ref,
        Some(&PiResponse {
            version: "synthetic".to_owned(),
            ok: true,
            text: Some("synthetic usage checkpoint".to_owned()),
            failure_code: None,
            model_ids: None,
            model_list_origin: None,
            usage: PiUsage {
                input_tokens: Some(input_tokens),
                output_tokens: Some(output_tokens),
                cost_usd: None,
            },
            elapsed_ms: Some(10),
            diagnostic: None,
        }),
    )
    .await
    .unwrap();
    let charged_after_checkpoint: i64 = sqlx::query_scalar(
        "SELECT charged_tokens FROM linggan_model_invocation WHERE invocation_ref=$1",
    )
    .bind(first_reservation.invocation_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(charged_after_checkpoint, actual_charge);

    let second_lease = claim_next_study_batch(&db, Uuid::new_v4(), 60)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(second_lease.batch_ref, second.batch_ref);
    assert!(matches!(
        reserve_study_batch_model_call(&db, second.batch_ref, second_lease.lease_token).await,
        Err(StudyModelDispatchError::BudgetDeferred)
    ));
    let second_state: String =
        sqlx::query_scalar("SELECT state FROM linggan_comment_study_batch WHERE batch_ref=$1")
            .bind(second.batch_ref)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(second_state, "prepared");
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_comment_study_model_request WHERE run_ref=$1",
        )
        .bind(run_ref)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        1
    );

    let first_target: Uuid = sqlx::query_scalar(
        "SELECT target_ref FROM linggan_comment_study_batch_target WHERE batch_ref=$1",
    )
    .bind(first.batch_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    accept_study_batch_output(
        &db,
        first.batch_ref,
        first_lease.lease_token,
        json!({
            "contract":"comment-study.note-batch.v1",
            "batchRef":first.batch_ref,
            "contentPublicRef":first.content_public_ref,
            "results":[{"targetRef":first_target,"outcome":"no_signal","reason":"SYNTHETIC","signals":[]}]
        }),
    )
    .await
    .unwrap();
    let second_lease = claim_next_study_batch(&db, Uuid::new_v4(), 60)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        reserve_study_batch_model_call(&db, second.batch_ref, second_lease.lease_token).await,
        Err(StudyModelDispatchError::BudgetExhausted)
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_comment_study_model_request WHERE run_ref=$1",
        )
        .bind(run_ref)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        1
    );
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL and local Pi child; no provider call"]
async fn v2_frozen_method_and_batch_reach_the_configured_worker_adapter() {
    let (db, command, _) = setup("start_v2_adapter_boundary", 1).await;
    let run_ref = start_study_run(&db, command, TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();
    let prepared = prepare_study_batch(
        &db,
        PrepareStudyBatchRequest {
            run_ref,
            maximum_targets: 12,
        },
    )
    .await
    .unwrap();
    let claim = claim_next_study_batch(&db, Uuid::new_v4(), 60)
        .await
        .unwrap()
        .unwrap();
    let adapter = PiAdapter::configured_with_test_command(
        std::path::PathBuf::from("/bin/sh"),
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/support/comment_study_settlement_adapter.sh"),
    );
    let error = call_study_batch_model(
        &db,
        &SyntheticModelSecrets,
        &adapter,
        prepared.batch_ref,
        claim.lease_token,
    )
    .await
    .expect_err("the local child verifies the frozen v2 request then returns a synthetic failure");
    assert!(matches!(error, StudyModelRunnerError::ProviderFailure));
    let (request_hash, dispatch_started, state, failure_code, call_started): (
        String,
        bool,
        String,
        Option<String>,
        bool,
    ) = sqlx::query_as(
        "SELECT request.request_hash,request.dispatch_started_at IS NOT NULL, \
                invocation.state,invocation.failure_code,invocation.result->>'callStarted'='true' \
         FROM linggan_comment_study_model_request request \
         JOIN linggan_model_invocation invocation USING(invocation_ref) \
         WHERE request.run_ref=$1 AND request.stage='semantic'",
    )
    .bind(run_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(request_hash.len(), 64);
    assert!(dispatch_started);
    assert_eq!(state, "failed");
    assert_eq!(
        failure_code.as_deref(),
        Some("v2_request_snapshot_verified")
    );
    assert!(call_started);
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_comment_study_target WHERE run_ref=$1",
        )
        .bind(run_ref)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        "queued"
    );
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn expired_dispatched_invocation_keeps_conservative_budget_charge_without_usage() {
    let (db, command, _) = setup("start_expired_dispatched_budget", 1).await;
    let run_ref = start_study_run(&db, command, TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();
    let prepared = prepare_study_batch(
        &db,
        PrepareStudyBatchRequest {
            run_ref,
            maximum_targets: 12,
        },
    )
    .await
    .unwrap();
    let lease = claim_next_study_batch(&db, Uuid::new_v4(), 60)
        .await
        .unwrap()
        .unwrap();
    let reservation = reserve_study_batch_model_call(&db, prepared.batch_ref, lease.lease_token)
        .await
        .unwrap();
    let reserved_tokens: i64 = sqlx::query_scalar(
        "SELECT reserved_tokens FROM linggan_model_invocation WHERE invocation_ref=$1",
    )
    .bind(reservation.invocation_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(reserved_tokens > 0);

    mark_study_batch_model_dispatch_started(
        &db,
        reservation.invocation_ref,
        prepared.batch_ref,
        lease.lease_token,
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE linggan_comment_study_batch \
         SET lease_expires_at=scope_001_now()-interval '1 second' WHERE batch_ref=$1",
    )
    .bind(prepared.batch_ref)
    .execute(db.pool())
    .await
    .unwrap();

    assert_eq!(recover_expired_study_batch_leases(&db).await.unwrap(), 1);
    let (state, charged_tokens): (String, i64) = sqlx::query_as(
        "SELECT state,charged_tokens FROM linggan_model_invocation WHERE invocation_ref=$1",
    )
    .bind(reservation.invocation_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(state, "failed");
    assert_eq!(charged_tokens, reserved_tokens);
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn expired_undispatched_invocation_releases_its_budget_reservation() {
    let (db, command, _) = setup("start_expired_undispatched_budget", 1).await;
    let run_ref = start_study_run(&db, command, TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();
    let prepared = prepare_study_batch(
        &db,
        PrepareStudyBatchRequest {
            run_ref,
            maximum_targets: 12,
        },
    )
    .await
    .unwrap();
    let lease = claim_next_study_batch(&db, Uuid::new_v4(), 60)
        .await
        .unwrap()
        .unwrap();
    let reservation = reserve_study_batch_model_call(&db, prepared.batch_ref, lease.lease_token)
        .await
        .unwrap();
    let reserved_tokens: i64 = sqlx::query_scalar(
        "SELECT reserved_tokens FROM linggan_model_invocation WHERE invocation_ref=$1",
    )
    .bind(reservation.invocation_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(reserved_tokens > 0);
    sqlx::query(
        "UPDATE linggan_comment_study_batch \
         SET lease_expires_at=scope_001_now()-interval '1 second' WHERE batch_ref=$1",
    )
    .bind(prepared.batch_ref)
    .execute(db.pool())
    .await
    .unwrap();

    assert_eq!(recover_expired_study_batch_leases(&db).await.unwrap(), 1);
    let (state, charged_tokens): (String, i64) = sqlx::query_as(
        "SELECT state,charged_tokens FROM linggan_model_invocation WHERE invocation_ref=$1",
    )
    .bind(reservation.invocation_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(state, "failed");
    assert_eq!(charged_tokens, 0);
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn start_acquires_lock_before_choosing_and_sees_the_post_wait_committed_material() {
    let (db, c, _) = setup("start_wait_snapshot", 0).await;
    let mut holder = db.pool().begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(study_domain_lock_key(domain()).unwrap())
        .execute(&mut *holder)
        .await
        .unwrap();
    let other = db.clone();
    let task =
        tokio::spawn(async move { start_study_run(&other, c, TrustedStudyOrigin::Manual).await });
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            let waiting: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM pg_locks WHERE locktype='advisory' AND NOT granted)",
            )
            .fetch_one(db.pool())
            .await
            .unwrap();
            if waiting {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    add(&db, "after-wait").await;
    refresh_clean_cache(&db, domain(), 128).await.unwrap();
    holder.commit().await.unwrap();
    assert_eq!(task.await.unwrap().unwrap().target_count, 1);
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn missing_scope_or_incomplete_guards_never_create_partial_work() {
    let (db, mut c, _) = setup("start_scope", 1).await;
    c.scope = serde_json::from_value(json!({"kind":"works","workRefs":[Uuid::new_v4()]})).unwrap();
    assert!(matches!(
        start_study_run(&db, c.clone(), TrustedStudyOrigin::Manual).await,
        Err(StudyStartError::NotFound)
    ));
    assert_eq!(effects(&db).await, json!([0, 0, 0, 0]));
    sqlx::raw_sql("ALTER TABLE linggan_comment_study_target DISABLE TRIGGER cs_target_input_guard")
        .execute(db.pool())
        .await
        .unwrap();
    assert!(matches!(
        start_study_run(&db, c, TrustedStudyOrigin::Manual).await,
        Err(StudyStartError::SchemaUnavailable)
    ));
}
